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
import mobile  # noqa: E402
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


class Guides(unittest.TestCase):
    """The guides (/wrath/docs, stream/docs.py) and the audit keep one rule: no game version in a sentence."""

    def test_same_rule_as_test_docs(self):
        src = (ROOT / "stream" / "tests" / "test_docs.py").read_text(encoding="utf-8")
        self.assertIn(f'VERSION = re.compile(r"{common.VERSION.pattern}")', src)

    def test_every_guide_passes_the_audits_check(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location("wu_docs_audit", ROOT / "stream" / "docs.py")
        d = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(d)
        for name, _ in d.PAGES:
            st, body = d.page("/wrath/docs" + ("" if name == "index" else "/" + name))
            self.assertEqual(st, 200, name)
            self.assertEqual(web.page_problems(body.decode())["version"], [], name)


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


class HeroKey(unittest.TestCase):
    """The Hero Key section (tools/audit/herokey.py): secrets masked, the switch, and a cleanup the site refuses is red."""

    def test_mask(self):
        import herokey
        self.assertEqual(herokey.mask("ovk1.abcdefghij." + "x" * 43), "ovk1.ab…")
        self.assertEqual(herokey.mask(""), "(none)")

    def test_switch_off(self):
        import herokey
        out = io.StringIO()
        rec = common.Recorder(out=out)
        ctx = common.Ctx({"addresses": {}}, rec, {"site": "http://127.0.0.1:9"})
        os.environ["LEYLINE_AUDIT_HEROKEY"] = "0"
        try:
            herokey.run(ctx, rec)
        finally:
            os.environ.pop("LEYLINE_AUDIT_HEROKEY")
        self.assertEqual([(r.status, r.detail) for r in rec.results], [("SKIP", "LEYLINE_AUDIT_HEROKEY=0")])

    def test_a_refused_cleanup_is_red_and_nothing_stays_signed_in(self):
        import herokey
        fk = fakes.Fakes()
        os.environ["WU_AUDIT_CLOSE"] = "0"
        try:
            out = io.StringIO()
            rec = common.Recorder(out=out)
            fake, rewrite = fk.ctx_args()
            herokey.run(common.Ctx({"addresses": {}}, rec, fake, rewrite), rec)
        finally:
            os.environ.pop("WU_AUDIT_CLOSE")
            fk.close()
        last = rec.results[-1]
        self.assertEqual((last.check, last.status), ("The test account is cleaned up", "FAIL"))
        self.assertIn("403", last.detail)
        self.assertEqual([r.check for r in rec.results if r.status == "FAIL"], ["The test account is cleaned up"], out.getvalue())
        self.assertIsNone(re.search(r"ovk1\.[A-Za-z0-9]{3}", out.getvalue()))


class FakeRun(unittest.TestCase):
    """The whole audit against the fakes: green, and the audit account's password in no line or report."""

    def test_the_fake_run(self):
        sys.path.insert(0, str(AUDIT))
        import audit
        logs = Path(tempfile.mkdtemp())
        only = "site,downloads,server,hosting,wrath,herokey,launcher" + (",den" if os.environ.get("DEN_LITE_BIN") else "")
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
        self.assertEqual(fails, [], "\n".join(fails))
        for must in ("PASS  Wrath Unleashed: Target a creature and start melee", "PASS  Wrath Unleashed: Mount up",
                     "PASS  Wrath Unleashed: Boost to level 80", "PASS  Wrath Unleashed: The level-1 test character is deleted again",
                     "PASS  Bug reports: In game", "PASS  Forum: Categories load", "PASS  Looks: Looks in the catalog",
                     "PASS  Downloads: Host pack", "PASS  Hosting: Wrath Unleashed on the network list",
                     "PASS  Server: Bug manifest reachable", "PASS  Site: Every public page answers",
                     "PASS  Hero Key: Get a Hero Key", "PASS  Hero Key: Status: the key while new, then saved",
                     "PASS  Hero Key: Sign in with the key", "PASS  Hero Key: Sign in on the website's page",
                     "PASS  Hero Key: Signed out on the website: the old cookie gets the signed-out card",
                     "PASS  Hero Key: A new key: the old one stops working",
                     "PASS  Hero Key: A pasted key with spaces, a line break and OVK1. still signs in",
                     "PASS  Hero Key: Change your name", "PASS  Hero Key: A refused name gets plain words",
                     "PASS  Hero Key: Password: set, sign in, remove", "PASS  Hero Key: Sign out: the token stops working",
                     "PASS  Hero Key: A wrong key and a garbage body get plain refusals",
                     "PASS  Hero Key: The test account is cleaned up"):
            self.assertIn(must, text)
        for blob in (text, report_txt, report_html):   # a key is never more than "ovk1.ab…" in a line or the report
            self.assertIsNone(re.search(r"ovk1\.[A-Za-z0-9]{3}", blob))
        if os.environ.get("DEN_LITE_BIN"):
            self.assertIn("PASS  Your Den: The Den is kept over a restart", text)
            self.assertIn("PASS  Your Den: Host and friend see each other", text)
        self.assertEqual(rc, 1 if fails else 0)
        self.assertIn("Only you can check these", report_html)


def png(w: int, h: int, rgb=(200, 30, 30)) -> bytes:
    """A solid PNG (standard library only)."""
    def chunk(t, b):
        return struct.pack(">I", len(b)) + t + b + struct.pack(">I", zlib.crc32(t + b) & 0xffffffff)
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


class Mobile(unittest.TestCase):
    """The phone and screenshot sweep (tools/audit/mobile.py, mobile_sweep.cjs): the checks, the lines, the switch,
    no Playwright, the report's changed list; and, where node and Playwright are here, the sweep against the fakes."""

    def ok(self, **kw):
        r = {"page": "home", "device": "iphone13", "status": 200, "scrollWidth": 390, "innerWidth": 390, "small": [], "cut": [],
             "past": [], "errors": [], "text": "Leyline. Not affiliated with or endorsed by Blizzard Entertainment.", "attrs": "", "shot": "mobile/home-iphone13.png"}
        r.update(kw)
        return r

    def test_problems(self):
        self.assertEqual(mobile.problems(self.ok()), [], "the legal line is allowed")
        self.assertEqual(mobile.problems(self.ok(status=404)), ["HTTP 404"])
        self.assertIn("scrolls sideways", mobile.problems(self.ok(scrollWidth=420))[0])
        self.assertEqual(mobile.problems(self.ok(scrollWidth=391)), [], "one pixel of rounding is fine")
        self.assertIn("buttons under 40 px", mobile.problems(self.ok(small=["Log in 92x38"]))[0])
        self.assertIn("banned words", mobile.problems(self.ok(text="Play " + "War" + "craft here"))[0])
        self.assertIn("banned words", mobile.problems(self.ok(attrs="a " + "W" + "oW logo"))[0])
        self.assertIn("errors", mobile.problems(self.ok(errors=["page error: x is not defined"]))[0])
        self.assertIn("text cut off", mobile.problems(self.ok(cut=[".title: Long words"]))[0])
        self.assertIn("key shows", mobile.problems(self.ok(leak=True))[0])
        # the desktop pass: only the answer, errors and words (a desktop layout isn't judged as a phone's)
        self.assertEqual(mobile.problems(self.ok(scrollWidth=2000, small=["x 10x10"]), desktop=True), [])

    def test_one_line_per_page_over_the_phones(self):
        rs = [self.ok(device=d) for d in mobile.DEVICES] + [self.ok(page="forum", device="pixel7", small=["Post 80x30"])]
        lines = mobile.page_lines(rs, [("home", "Home", "/wrath/leyline"), ("forum", "Forum", "/f"), ("forge", "Forge", "/g")])
        self.assertEqual([(t, st) for t, st, _, _ in lines], [("Home on phones", "PASS"), ("Forum on phones", "FAIL"), ("Forge on phones", "SKIP")])
        self.assertIn("iPhone 13, Pixel 7, 360x640", lines[0][2])
        self.assertIn("Pixel 7: buttons under 40 px: Post 80x30", lines[1][2])

    def test_switch_off_and_no_playwright(self):
        rec = common.Recorder(out=io.StringIO())
        ctx = common.Ctx({"addresses": {}}, rec, {"site": "http://127.0.0.1:9"})
        os.environ["LEYLINE_AUDIT_MOBILE"] = "0"
        try:
            mobile.run(ctx, rec, Path(tempfile.mkdtemp()))
        finally:
            os.environ.pop("LEYLINE_AUDIT_MOBILE")
        self.assertEqual([(r.status, r.detail) for r in rec.results], [("SKIP", "LEYLINE_AUDIT_MOBILE=0")])
        rec = common.Recorder(out=io.StringIO())
        real = mobile.shutil.which
        mobile.shutil.which = lambda name: None
        try:
            mobile.run(ctx, rec, Path(tempfile.mkdtemp()))
        finally:
            mobile.shutil.which = real
        self.assertEqual([r.status for r in rec.results], ["SKIP", "SKIP"])
        self.assertIn("npm install -g playwright", rec.results[0].detail, "the one-line fix")

    def test_rotate_keeps_one_run_back(self):
        shots = Path(tempfile.mkdtemp())
        self.assertIsNone(mobile.rotate(shots))
        (shots / "desktop").mkdir()
        (shots / "desktop" / "home.png").write_bytes(b"1")
        prev = mobile.rotate(shots)
        self.assertEqual((prev / "desktop" / "home.png").read_bytes(), b"1")
        self.assertFalse((shots / "desktop").exists())
        (shots / "desktop").mkdir()
        (shots / "desktop" / "home.png").write_bytes(b"2")
        mobile.rotate(shots)
        self.assertEqual((prev / "desktop" / "home.png").read_bytes(), b"2", "the run before that is dropped")

    def test_report_lists_the_changed_screenshots(self):
        R = common.Result
        rs = [R("Mobile", "Home on phones", "PASS", "/wrath/leyline"), R("Screenshots", "Desktop screenshots at 1440x900", "PASS", "16 saved")]
        meta = {"when": "2026-11-03 10:00", "secs": 90, "mode": "live", "shots": {"dir": "C:/x/logs/audit-shots", "previous": "C:/x/p",
                "changed": [{"shot": "desktop/home.png", "share": 12.5}, {"shot": "mobile/<b>.png", "share": 1}], "fresh": ["desktop/forge.png"], "same": 40, "count": 68}}
        t = report.text(rs, meta)
        self.assertIn("== Screenshots that changed", t)
        self.assertIn("desktop/home.png  (12.5 % of the picture)", t)
        self.assertIn("New this run: desktop/forge.png", t)
        h = report.page(rs, meta)
        self.assertIn('<a href="audit-shots/desktop/home.png">', h)
        self.assertIn('audit-shots/previous/desktop/home.png', h)
        self.assertIn("mobile/&lt;b&gt;.png", h)
        self.assertLess(h.index("Features"), h.index("Screenshots that changed"))
        self.assertLess(report.ORDER.index("Hero Key"), report.ORDER.index("Mobile"))
        first = report.text(rs, dict(meta, shots=dict(meta["shots"], previous="", changed=[])))
        self.assertIn("Nothing to compare with yet", first)
        self.assertNotIn("== Screenshots that changed", report.text(rs, {"when": "x"}))

    def test_the_section_is_in_the_audit(self):
        import audit
        self.assertIn("mobile", audit.SECTIONS)
        doc = (ROOT / "docs" / "AUDIT.md").read_text(encoding="utf-8")
        for key, _, path in mobile.PAGES:
            self.assertIn(path, doc, key)

    @unittest.skipUnless(mobile.node_env()[0], "node and Playwright are not here")
    def test_the_sweep_against_the_fakes(self):
        """A short sweep (three pages) on the fake site: green, the Log in journey on a phone, the test account closed,
        the screenshots saved, a changed one listed, and the key in no line, report or job file."""
        logs = Path(tempfile.mkdtemp())
        shots = logs / "audit-shots"
        for d, name in (("desktop", "home.png"), ("mobile", "home-pixel7.png")):
            (shots / d).mkdir(parents=True)
            (shots / d / name).write_bytes(png(40, 30))      # "the last run": a picture that differs from today's
        fk = fakes.Fakes()
        pages = mobile.PAGES
        mobile.PAGES = [p for p in pages if p[0] in ("home", "login", "submit")]
        out = io.StringIO()
        rec = common.Recorder(out=out)
        try:
            fake, rewrite = fk.ctx_args()
            ctx = common.Ctx({"addresses": {}}, rec, fake, rewrite)
            mobile.run(ctx, rec, logs)
        finally:
            mobile.PAGES = pages
            fk.close()
        text = out.getvalue()
        fails = [l for l in text.splitlines() if l.startswith("FAIL")]
        self.assertEqual(fails, [], text)
        for must in ("PASS  Mobile: Home on phones", "PASS  Mobile: Log in (Hero Key) on phones", "PASS  Mobile: Submit a link on phones",
                     "PASS  Mobile: Log in on a phone with a pasted Hero Key", "PASS  Mobile: Your profile shows on a phone, with Log out",
                     "PASS  Mobile: Submit a link while logged in, on a phone", "PASS  Mobile: Log out on a phone",
                     "PASS  Mobile: Test account: The test account is cleaned up", "PASS  Screenshots: Desktop screenshots at 1440x900"):
            self.assertIn(must, text)
        for d in mobile.DEVICES:
            self.assertTrue((shots / "mobile" / f"home-{d}.png").stat().st_size > 1000, d)
        self.assertTrue((shots / "mobile" / "login-signed-in-iphone13.png").exists())
        self.assertTrue((shots / "desktop" / "submit.png").exists())
        sh = ctx.cache["shots"]
        self.assertEqual(sorted(c["shot"] for c in sh["changed"]), ["desktop/home.png", "mobile/home-pixel7.png"])
        self.assertIn("mobile/home-iphone13.png", sh["fresh"])
        self.assertIsNone(re.search(r"ovk1\.[A-Za-z0-9]{3}", text))
        report.write(rec.results, {"shots": sh}, logs)
        for f in ("audit-latest.txt", "audit-latest.html"):
            self.assertIsNone(re.search(r"ovk1\.[A-Za-z0-9]{3}", (logs / f).read_text()))
        self.assertIn("desktop/home.png", (logs / "audit-latest.txt").read_text())


if __name__ == "__main__":
    unittest.main()
