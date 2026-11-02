#!/usr/bin/env python3
"""The release audit (docs/AUDIT.md): its parsers, the headless player's packets, the report and its manual list
(the same as docs/AUDIT.md's), secrets kept out of every line, and the whole run with --fake (the site's own code,
a fake relay, GitHub and realm, the server script's output). The Den section runs too when DEN_LITE_BIN points
at a built Den Lite (labs/den-lite).

    python3 -I tools/audit/tests/test_audit.py
"""
import asyncio
import io
import json
import os
import re
import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
AUDIT = HERE.parent
ROOT = AUDIT.parents[1]
sys.path.insert(0, str(AUDIT))

import common  # noqa: E402
import fakes  # noqa: E402
import pc  # noqa: E402
import play335 as p  # noqa: E402
import report  # noqa: E402
import server  # noqa: E402
import web  # noqa: E402


class ServerOutput(unittest.TestCase):
    def test_lines_secret_and_done(self):
        rows, secret, done = server.parse(fakes.SSH_CHECKS + "noise\r\nAUDIT relay bad wu-relay is not running\r\n")
        self.assertTrue(done)
        self.assertEqual(secret, "")
        self.assertIn(("world", "ok", "running for 30 h 2 min; restarts since it was made: 3"), rows)
        self.assertEqual(rows[-1], ("relay", "bad", "wu-relay is not running"))
        rows, secret, _ = server.parse(fakes.SSH_ACCOUNT)
        self.assertEqual(secret, fakes.FAKE_PASSWORD)
        self.assertEqual(rows[0][:2], ("account", "ok"))

    def test_every_key_the_script_prints_has_a_name_and_an_action(self):
        sh = (AUDIT / "server" / "audit-server.sh").read_text()
        keys = set(re.findall(r'\bsay ([a-z_]+) (?:ok|bad|skip)', sh))
        self.assertGreater(len(keys), 10)
        self.assertEqual(keys - set(server.CHECKS), set())
        for k, (name, action) in server.CHECKS.items():
            self.assertTrue(name and action, k)

    def test_the_server_script_is_read_only_but_for_the_audit_account(self):
        sh = (AUDIT / "server" / "audit-server.sh").read_text()
        soap = re.findall(r'soap "([^"]+)"', sh)
        self.assertEqual(sorted(soap), sorted(["account create $ACCOUNT $PW", "account set password $ACCOUNT $PW $PW",
                                               "account set gmlevel $ACCOUNT 0 -1", "server info"]))
        self.assertIn('ACCOUNT="AUDIT"', sh)
        for bad in ("docker restart", "docker stop", "docker rm", "rm -rf", "systemctl", "DROP ", "DELETE ", "reload ale"):
            self.assertNotIn(bad, sh)
        self.assertIn("chmod 600 /root/.leyline-audit/password", sh)
        self.assertNotIn("Chumpton", sh)
        self.assertEqual(sh.count("AUDIT_SECRET"), 2)   # the header comment and the one line that prints it

    def test_payload_carries_the_slice_check(self):
        pl = server.payload("checks")
        self.assertTrue(pl.startswith("export SLICE_HEALTH_B64='"))
        self.assertNotIn("SLICE_HEALTH_B64='", server.payload("account"))


class Packets(unittest.TestCase):
    def test_packed_guid_round_trip(self):
        for g in (0, 1, 0xF130000000000000 | (2031 << 24) | 77, 0xFFFFFFFFFFFFFFFF):
            b = p.Buf(p.pack_guid(g) + b"x")
            self.assertEqual(b.pguid(), g)
            self.assertEqual(b.left(), 1)

    def test_update_object_creates_values_movement_and_out_of_range(self):
        body = fakes.update(
            fakes.create(5, p.TYPE_PLAYER, fakes.living(1.0, 2.0, 3.0, 0.5, self_=True), {p.U_LEVEL: 7, 0x139: 12282}),
            fakes.create(fakes.MOB, p.TYPE_UNIT, fakes.living(4.0, 5.0, 6.0, 1.0, spline=True), {p.F_ENTRY: 2031, p.U_FACTION: 7}),
            b"\x00" + p.pack_guid(5) + fakes.values({p.U_MOUNT: 6080}),
            b"\x04" + struct.pack("<I", 1) + p.pack_guid(fakes.VENDOR))
        blocks = p.parse_update(body)
        self.assertEqual([b["kind"] for b in blocks], ["create", "create", "values", "out"])
        self.assertEqual(blocks[0]["fields"], {p.U_LEVEL: 7, 0x139: 12282})
        self.assertEqual(blocks[0]["pos"][:3], (1.0, 2.0, 3.0))
        self.assertEqual(blocks[1]["pos"][:2], (4.0, 5.0), "the spline after the speeds is read past")
        self.assertEqual(blocks[1]["fields"][p.F_ENTRY], 2031)
        self.assertEqual(blocks[3]["guids"], [fakes.VENDOR])
        comp = struct.pack("<I", len(body)) + zlib.compress(body)
        self.assertEqual(p.uncompress_update(comp), body)
        with self.assertRaises(p.ParseError):
            p.parse_update(body[:-3])

    def test_stationary_and_position_blocks(self):
        mv = struct.pack("<H", p.UF_STATIONARY | p.UF_ROTATION) + struct.pack("<ffff", 9, 8, 7, 1) + struct.pack("<q", 0)
        self.assertEqual(p.read_movement(p.Buf(mv))["pos"], (9.0, 8.0, 7.0, 1.0))
        mv = struct.pack("<H", p.UF_POSITION) + p.pack_guid(0) + struct.pack("<8f", 1, 2, 3, 1, 2, 3, 0.5, 0.5)
        self.assertEqual(p.read_movement(p.Buf(mv))["pos"], (1.0, 2.0, 3.0, 0.5))

    def test_chat_lines(self):
        c = p.parse_chat(fakes.chat(p.CHAT_SYSTEM, 0, "Welcome to your Den: 1 props."))
        self.assertEqual((c["type"], c["text"]), (p.CHAT_SYSTEM, "Welcome to your Den: 1 props."))
        c = p.parse_chat(fakes.chat(p.CHAT_MONSTER_SAY, 0xF130000000000001, "Try a stamp.", name="Jaina"))
        self.assertEqual((c["name"], c["text"]), ("Jaina", "Try a stamp."))
        self.assertEqual(p.strip_colours("|cff66ccff[Wrath]|r Type .bug"), "[Wrath] Type .bug")

    def test_gossip_menu(self):
        g = p.parse_gossip(fakes.gossip(5, 60001, ["Begin your adventure at level 1", "Boost to level 80"]))
        self.assertEqual((g["guid"], g["menu"]), (5, 60001))
        self.assertEqual([i["text"] for i in g["items"]], ["Begin your adventure at level 1", "Boost to level 80"])

    def test_client_movement_is_what_the_core_reads(self):
        b = p.movement_info(p.MOVE_FORWARD, 1234, 1.0, 2.0, 3.0, 0.25)
        self.assertEqual(len(b), 4 + 2 + 4 + 16 + 4)
        self.assertEqual(struct.unpack("<IHIffffI", b), (1, 0, 1234, 1.0, 2.0, 3.0, 0.25, 0))


class Pages(unittest.TestCase):
    def test_version_chips_are_not_sentences(self):
        page = ('<p>Play with your game folder, any one works <span class="chip">3.3.5a</span> today.</p>'
                '<p>Drop your own 3.3.5a game folder on the page to go anywhere.</p><code>set realmlist 3.3.5a x y z</code>')
        self.assertEqual(web.page_problems(page)["version"], ["Drop your own 3.3.5a game folder on the page to go anywhere."])

    def test_banned_words_and_server_errors(self):
        pr = web.page_problems('<h1>Leyline</h1><img alt="a WoW logo"><p>Traceback (most recent call last):</p>')
        self.assertTrue(pr["banned"] and pr["error"])
        self.assertEqual(web.page_problems("<p>Wrath Unleashed, nowhere to go</p>")["banned"], [])

    def test_links_and_pictures(self):
        hrefs, srcs = web.links_of('<a href="/wrath/looks">x</a><a href="mailto:a@b">m</a><a href="#top">t</a>'
                                   '<img src="/wrath/art/a.webp"><a href="leyline://install/x">i</a>', "https://site.test/wrath")
        self.assertEqual(hrefs, {"https://site.test/wrath/looks"})
        self.assertEqual(srcs, {"https://site.test/wrath/art/a.webp"})
        self.assertTrue(web.NO_FETCH.search("/wrath/play?mini=x"))
        self.assertTrue(web.NO_FETCH.search("/wrath?go=w:12"))
        self.assertFalse(web.NO_FETCH.search("/wrath/leyline/forum"))

    def test_launcher_presets_read_from_the_source(self):
        self.assertTrue({"low", "classic", "balanced", "ultra"} <= web.launcher_presets())


class Settings(unittest.TestCase):
    def test_sane_and_not(self):
        d = tempfile.mkdtemp()
        ok = {"schema": 1, "clients": [{"id": "a", "path": d}], "selected_client": "a"}
        self.assertEqual(pc.settings_problems(ok), [])
        bad = {"schema": 2, "clients": [{"id": "a", "path": d + "/gone"}], "selected_client": "b"}
        self.assertEqual(len(pc.settings_problems(bad)), 3)
        self.assertEqual(pc.settings_problems([]), ["not a JSON object"])


class Report(unittest.TestCase):
    def results(self):
        R = common.Result
        return [R("Site", "Looks page", "PASS", "/wrath/looks"), R("Your Den", "Jaina answers", "FAIL", "silent", "Tell Claude."),
                R("Server", "Relay running", "FAIL", "not running", "Run the relay step."), R("Launcher", "x", "SKIP", "not Windows")]

    def test_next_action_is_the_most_basic_failure(self):
        self.assertEqual(report.next_action(self.results()), "Server: Relay running failed. Run the relay step.")
        self.assertTrue(report.next_action(self.results()[:1]).startswith("Nothing failed"))

    def test_text_and_html(self):
        rs = self.results() + [common.Result("Site", "<script>alert(1)</script>", "PASS", "a & b")]
        meta = {"when": "2026-11-01 17:00", "secs": 61, "mode": "live", "where": "site: leylinelauncher.com"}
        t = report.text(rs, meta)
        self.assertIn("Result: 2 FAILED", t)
        self.assertIn("Next: Server: Relay running failed.", t)
        self.assertIn("10. Azeroth Survivors", t)
        h = report.page(rs, meta)
        self.assertIn("<title>Release audit</title>", h)
        self.assertIn("&lt;script&gt;", h)
        self.assertNotIn("<script>", h)
        self.assertLess(h.index("Only you can check these"), h.index("Every check"), "the manual list is at the top")
        self.assertEqual(h.count('<li><b>'), len(report.MANUAL))
        self.assertIn('prefers-color-scheme:dark', h)
        self.assertIsNone(common.BANNED.search(h + t))

    def test_manual_list_is_short_and_the_same_as_the_doc(self):
        self.assertLessEqual(len(report.MANUAL), 10)
        doc = (ROOT / "docs" / "AUDIT.md").read_text(encoding="utf-8")
        listed = re.findall(r"^(\d+)\. \*\*(.+?)\*\*  \n   Where: (.+?)  \n   Good: (.+?)$", doc, re.M)
        self.assertEqual([(t, w, g) for _, t, w, g in listed], report.MANUAL)
        for t, w, g in report.MANUAL:
            self.assertTrue(w and g, t)

    def test_secrets_never_in_a_line(self):
        out = io.StringIO()
        rec = common.Recorder(out=out)
        rec.hide("SECRETPW123")
        r = rec.add("X", "y", "FAIL", "login with SECRETPW123 failed", "retry SECRETPW123")
        self.assertNotIn("SECRETPW123", out.getvalue() + r.detail + r.action)


class Config(unittest.TestCase):
    def test_every_address_has_a_new_and_an_old_name(self):
        cfg = common.load_config()
        for k in ("site", "game", "relay", "realmlist"):
            new, old = cfg["addresses"][k]["new"], cfg["addresses"][k]["old"]
            self.assertTrue(new.endswith("leylinelauncher.com"), new)
            self.assertTrue(old.endswith("openvanilla.app"), old)

    def test_no_address_is_written_anywhere_else(self):
        for f in AUDIT.glob("*.py"):
            src = f.read_text(encoding="utf-8")
            self.assertNotRegex(src, r"(?:play|wrath|relay)\.openvanilla\.app|leylinelauncher\.com", f.name)

    def test_the_new_name_first_then_the_old(self):
        ctx = common.Ctx({"addresses": {"site": {"new": "new.invalid", "old": "old.invalid"}}}, common.Recorder(out=io.StringIO()))
        self.assertEqual(ctx.host("site"), "old.invalid")
        self.assertEqual(ctx.base("site"), "https://old.invalid")


class FakeRun(unittest.TestCase):
    """The whole audit against the fakes: green, and the audit account's password in no line or report."""

    def test_the_fake_run(self):
        sys.path.insert(0, str(AUDIT))
        import audit
        logs = Path(tempfile.mkdtemp())
        only = "site,downloads,server,hosting,wrath,launcher" + (",den" if os.environ.get("DEN_LITE_BIN") else "")
        out = io.StringIO()
        old = sys.stdout
        sys.stdout = out
        try:
            rc = audit.main(["--fake", "--only", only, "--logs", str(logs)])
        finally:
            sys.stdout = old
        text = out.getvalue()
        report_txt = (logs / "audit-latest.txt").read_text()
        report_html = (logs / "audit-latest.html").read_text()
        for blob in (text, report_txt, report_html):
            self.assertNotIn(fakes.FAKE_PASSWORD, blob)
        fails = [l for l in text.splitlines() if l.startswith("FAIL")]
        # the guides (/wrath/docs) still name client versions in sentences: a real finding, reported on its own line
        allowed = [l for l in fails if "the guides, /wrath/docs" in l]
        self.assertEqual(fails, allowed, "\n".join(fails))
        for must in ("PASS  Wrath Unleashed: Target a creature and start melee", "PASS  Wrath Unleashed: Mount up",
                     "PASS  Wrath Unleashed: Boost to level 80", "PASS  Wrath Unleashed: The level-1 test character is deleted again",
                     "PASS  Bug reports: In game", "PASS  Forum: Categories load", "PASS  Looks: Looks in the catalog",
                     "PASS  Downloads: Host pack", "PASS  Hosting: Wrath Unleashed on the network list",
                     "PASS  Server: Bug manifest reachable", "PASS  Site: Every public page answers"):
            self.assertIn(must, text)
        if os.environ.get("DEN_LITE_BIN"):
            self.assertIn("PASS  Your Den: The Den is kept over a restart", text)
            self.assertIn("PASS  Your Den: Host and friend see each other", text)
        self.assertEqual(rc, 1 if fails else 0)
        self.assertIn("Only you can check these", report_html)


if __name__ == "__main__":
    unittest.main()
