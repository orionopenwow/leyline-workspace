#!/usr/bin/env python3
"""The kill switch (tools/killswitch/killswitch.py, server/ks_server.py; docs/KILL-SWITCH.md), every part against fakes:
the message checks, previews that only read, Pause (the message first, then everything off), its dry run, Undo, the
gates in front of Delete (pause first, the wait, the right gh account, the delete_repo scope), the backup (real git
bundles, the Discord export, the website's data with its sha256) and that nothing is deleted when any of it fails,
the server half on its own, the real Discord and gh adapters against fakes, and no token in any log.

    python3 -I tools/killswitch/test_killswitch.py
"""
import json, sys, tempfile, time, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import killswitch as ks  # noqa: E402
import fakes  # noqa: E402

GOOD_MSG = "Thank you for every adventure.\nThe realms close tonight. Leyline is not affiliated with or endorsed by Blizzard Entertainment."


class Base(unittest.TestCase):
    def setUp(self):
        self.root, self.gh, self.dc, self.srv, self.steps, self.desk = fakes.world()
        self.logpath = self.root / "logs" / "killswitch-test.log"
        self.clock = [time.time()]

    def ctx(self, parts=ks.PARTS, private=False, dry=False, message=GOOD_MSG, title="Leyline is closing", **kw):
        log = ks.Log(self.logpath)
        args = dict(title=title, message=message, parts=parts, options={"private": private, "dry": dry}, log=log,
                    github=self.gh, discord=self.dc, server=self.srv, run_step=self.steps, backup_root=self.desk,
                    now=lambda: self.clock[0])
        args.update(kw)
        return ks.Ctx(**args)

    def pause(self, **kw):
        c = self.ctx(**kw)
        return ks.run_pause(c, ks.new_id())

    def later(self, hours=25):
        self.clock[0] += hours * 3600

    def lj(self, rel):
        return json.loads((self.srv.data / rel).read_text())


class Message(unittest.TestCase):
    def test_checks(self):
        self.assertTrue(ks.message_problems("t", ks.DEFAULT_MESSAGE))            # the TODO(owner) default
        self.assertTrue(ks.message_problems("t", ""))
        self.assertTrue(ks.message_problems("t", "x" * (ks.MESSAGE_MAX + 1)))
        self.assertTrue(ks.message_problems("x" * 81, "fine"))
        for bad in ("Thanks, WoW fans", "our Warcraft days", "Lich King forever", "World of  Warcraft", "blizzard"):
            self.assertTrue(ks.message_problems("t", bad), bad)
        self.assertEqual(ks.message_problems("Leyline is closing", GOOD_MSG), [])   # the legal line is allowed
        self.assertTrue(ks.message_problems("Bye WoW", "fine"))

    def test_phrases_and_keys(self):
        self.assertEqual(ks.PHRASES, {"pause": "SHUT DOWN LEYLINE", "delete": "DELETE LEYLINE FOREVER",
                                      "resume": "BRING LEYLINE BACK"})
        k = ks.preview_key("pause", "t", "m", ["github", "site"], {"dry": False})
        self.assertEqual(k, ks.preview_key("pause", "t", "m", ["site", "github", "nope"], {"private": False}))
        self.assertNotEqual(k, ks.preview_key("pause", "t", "m2", ["site", "github"], {}))
        self.assertNotEqual(k, ks.preview_key("delete", "t", "m", ["site", "github"], {}))
        self.assertNotEqual(k, ks.preview_key("pause", "t", "m", ["site", "github"], {"private": True}))


class Preview(Base):
    def test_pause_preview_lists_everything_and_changes_nothing(self):
        before = self.srv.snapshot()
        p = ks.preview(self.ctx(private=True), "pause")
        self.assertEqual(p["problems"], [])
        text = "\n".join(f"{i['part']}|{i['kind']}|{i['text']}" for i in p["items"])
        for s in ("message|do|Discord: post the message in #📣︱announcements", "Website: a banner",
                  "top News post", "message of the day", "Stop every node", "all 2 realm(s)",
                  "hangout-off, vr-beacon-off, build-realm-off, ov112-realm-off, sv-site-off, gamedata-off",
                  "site|do|Every page", "DNS is yours", "all 7 channels read-only",
                  "orionopenwow/leyline-workspace (open, public): archive it (read-only) and make it private",
                  "orionopenwow/Leyline-Launcher"):
            self.assertIn(s, text)
        self.assertEqual(self.srv.snapshot(), before)
        self.assertEqual(self.gh.writes(), [])
        self.assertEqual(self.dc.ops, [])
        self.assertEqual([c for c, _ in self.srv.calls], ["status"])
        self.assertEqual(self.steps.ran, [])
        self.assertIn("Stop every node", self.logpath.read_text())

    def test_problems(self):
        p = ks.preview(self.ctx(message=ks.DEFAULT_MESSAGE), "pause")
        self.assertTrue(any("TODO(owner)" in x for x in p["problems"]))
        self.gh._login = "Chumpton"
        p = ks.preview(self.ctx(parts=["github"]), "pause")
        self.assertTrue(any("signed in as Chumpton" in x for x in p["problems"]), p["problems"])
        self.assertFalse(any(i["part"] == "github" and i["kind"] == "do" for i in p["items"]))
        p = ks.preview(self.ctx(parts=["discord"], discord=None), "pause")
        self.assertTrue(any("No Discord bot token" in x for x in p["problems"]))
        p = ks.preview(self.ctx(parts=["message"], discord=None), "pause")
        self.assertEqual(p["problems"], [])
        self.assertTrue(any("not posted on Discord" in i["text"] for i in p["items"]))
        self.srv.fail.add("status")
        p = ks.preview(self.ctx(parts=["site"]), "pause")
        self.assertTrue(any("server didn't answer" in x for x in p["problems"]))
        self.assertTrue(ks.preview(self.ctx(parts=[]), "pause")["problems"])

    def test_delete_preview_gates(self):
        p = ks.preview(self.ctx(), "delete", None)
        self.assertTrue(any("only after a finished Pause" in x for x in p["problems"]))
        rec = self.pause(parts=["site", "github"])
        p = ks.preview(self.ctx(parts=["site", "github", "discord"]), "delete", rec)
        self.assertTrue(any("hours after the Pause" in x for x in p["problems"]))
        self.assertTrue(any("Discord: This part wasn't taken down" in x for x in p["problems"]))
        self.assertTrue(any("delete_repo" in x for x in p["problems"]))
        self.later()
        self.gh._scopes.add("delete_repo")
        p = ks.preview(self.ctx(parts=["site", "github"]), "delete", rec)
        self.assertEqual(p["problems"], [])
        text = "\n".join(i["text"] for i in p["items"])
        self.assertIn("DELETE it", text)
        self.assertIn("wipe it on the server", text)
        self.assertIn("First a full backup", text)
        self.assertIn("DNS", text)


class Pause(Base):
    def test_pause_does_every_part_in_order(self):
        rec = self.pause(private=True)
        self.assertTrue(rec["ok"], rec["problems"])
        # Discord: the message posted and pinned, first, then every channel read-only
        self.assertEqual(self.dc.ops[0][0], "post")
        self.assertEqual(self.dc.ops[0][1], "12")
        self.assertIn("**Leyline is closing**", self.dc.ops[0][2])
        self.assertEqual(self.dc.ops[1][0], "pin")
        for c in self.dc.chans:
            ow = next(o for o in c["permission_overwrites"] if o["id"] == fakes.FakeDiscord.GID)
            self.assertEqual(int(ow["deny"]) & ks.LOCK_BITS, ks.LOCK_BITS, c["name"])
            self.assertEqual(int(ow["allow"]) & ks.LOCK_BITS, 0)
        # the server: the farewell page (site ticked), News, the update notice, SOAP, the relay
        sd = self.lj("leyline/shutdown.json")
        self.assertEqual((sd["mode"], sd["title"]), ("closed", "Leyline is closing"))
        news = self.lj("leyline/news.json")["posts"]
        self.assertTrue(news[0]["id"].startswith("farewell-"))
        self.assertEqual(news[0]["items"][0], "Thank you for every adventure.")
        self.assertEqual(news[1]["id"], "n1")
        self.assertTrue(self.lj("leyline/version-policy.json")["notice"].startswith("Thank you"))
        soap = self.srv.soap_log.read_text()
        self.assertIn("announce Thank you", soap)
        self.assertIn("server set motd Thank you", soap)
        rb = self.lj("admin/relay-block.json")
        self.assertTrue(rb["nodes_off"])
        self.assertEqual(sorted(rb["blocked"]), ["aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb", "cccccccccccccccc"])
        # the existing Control steps, all of them
        self.assertEqual(self.steps.ran, list(ks.REALM_STEPS))
        # GitHub: archived and private
        for full in ks.GH_REPOS:
            self.assertTrue(self.gh.repos[full]["archived"] and self.gh.repos[full]["private"])
        self.assertFalse(any(o[0] == "delete" for o in self.gh.ops + self.dc.ops))
        self.assertEqual(sorted(rec["results"]["github"]["archived"]), sorted(ks.GH_REPOS))

    def test_message_only_is_a_banner(self):
        rec = self.pause(parts=["message"])
        self.assertTrue(rec["ok"], rec["problems"])
        self.assertEqual(self.lj("leyline/shutdown.json")["mode"], "banner")
        self.assertFalse(self.lj("admin/relay-block.json")["nodes_off"])
        self.assertEqual(self.steps.ran, [])

    def test_a_failed_step_goes_on(self):
        self.steps.fail.add("build-realm-off")
        rec = self.pause(parts=["realms"])
        self.assertEqual(self.steps.ran, list(ks.REALM_STEPS))
        self.assertFalse(rec["ok"])
        self.assertIn("build-realm-off", " ".join(rec["problems"]))

    def test_dry_run_changes_nothing(self):
        before = self.srv.snapshot()
        rec = ks.run_pause(self.ctx(dry=True, private=True), ks.new_id())
        self.assertFalse(rec["ok"])
        self.assertTrue(rec["dry"])
        self.assertEqual(self.srv.snapshot(), before)
        self.assertFalse(self.srv.soap_log.exists())
        self.assertFalse((self.srv.home / "active.json").exists())
        self.assertEqual(self.gh.writes(), [])
        self.assertEqual(self.dc.ops, [])
        self.assertEqual(self.steps.ran, [])
        log = self.logpath.read_text()
        self.assertIn("(dry run) would post and pin", log)
        self.assertIn("(dry run) would run the step hangout-off", log)
        self.assertIn("(dry run) would archive orionopenwow/Leyline", log)

    def test_refuses_before_anything(self):
        before = self.srv.snapshot()
        for kw, gh_login in (({"message": ks.DEFAULT_MESSAGE}, "orionopenwow"), ({}, "Chumpton"), ({}, None)):
            self.gh._login = gh_login
            with self.assertRaises(ks.Refused):
                ks.run_pause(self.ctx(**kw), ks.new_id())
        self.assertEqual(self.srv.snapshot(), before)
        self.assertEqual((self.gh.writes(), self.dc.ops, self.steps.ran), ([], [], []))


class Resume(Base):
    def test_undo_puts_it_all_back(self):
        before = self.srv.snapshot()
        chans = json.dumps(self.dc.chans, sort_keys=True)
        rec = self.pause(private=True)
        self.assertNotEqual(self.srv.snapshot(), before)
        p = ks.preview(self.ctx(), "resume", rec)
        self.assertEqual(p["problems"], [])
        self.assertTrue(any("hangout" in i["text"] for i in p["items"]))
        out = ks.run_resume(self.ctx(), rec)
        self.assertTrue(out["ok"], out["problems"])
        after = self.srv.snapshot()
        self.assertNotIn("leyline/shutdown.json", after)
        self.assertEqual(json.loads(after["leyline/news.json"])["posts"], json.loads(before["leyline/news.json"])["posts"])
        self.assertEqual(json.loads(after["leyline/version-policy.json"])["notice"], "")
        rb = json.loads(after["admin/relay-block.json"])
        self.assertEqual((rb["nodes_off"], sorted(rb["blocked"])), (False, ["cccccccccccccccc"]))
        self.assertEqual(json.dumps(self.dc.chans, sort_keys=True), chans)
        for full in ks.GH_REPOS:
            self.assertFalse(self.gh.repos[full]["archived"] or self.gh.repos[full]["private"])
        self.assertEqual(out["turn_back_on"], [ks.REALM_STEPS_BACK[s] for s in ks.REALM_STEPS])
        with self.assertRaises(ks.Refused):
            ks.run_resume(self.ctx(), dict(rec, deleted=True))


class Delete(Base):
    def ready(self, parts=("site", "discord", "github")):
        rec = self.pause()
        self.assertTrue(rec["ok"], rec["problems"])
        self.later()
        self.gh._scopes.add("delete_repo")
        return rec

    def test_gates(self):
        with self.assertRaises(ks.Refused):
            ks.run_delete(self.ctx(), ks.new_id(), None)
        rec = self.pause(parts=["message", "site"])
        self.gh._scopes.add("delete_repo")
        with self.assertRaisesRegex(ks.Refused, "hours after"):
            ks.run_delete(self.ctx(parts=["site"]), ks.new_id(), rec)
        self.later()
        with self.assertRaisesRegex(ks.Refused, "Not paused first: GitHub"):
            ks.run_delete(self.ctx(parts=["site", "github"]), ks.new_id(), rec)
        rec = self.pause()
        self.later()
        self.gh._login = "Chumpton"
        with self.assertRaisesRegex(ks.Refused, "orionopenwow"):
            ks.run_delete(self.ctx(parts=["github"]), ks.new_id(), rec)
        self.gh._login = "orionopenwow"
        self.gh._scopes.discard("delete_repo")
        with self.assertRaisesRegex(ks.Refused, "delete_repo"):
            ks.run_delete(self.ctx(parts=["github"]), ks.new_id(), rec)
        self.assertFalse(any(o[0] == "delete" for o in self.gh.ops + self.dc.ops))
        self.assertTrue((self.srv.data / "profiles" / "accounts.json").exists())
        self.assertEqual(list(self.desk.glob("Leyline-killswitch-backup-*")), [])

    def test_backup_then_delete(self):
        rec = self.ready()
        rid = ks.new_id()
        out = ks.run_delete(self.ctx(parts=["site", "discord", "github"]), rid, rec)
        self.assertTrue(out["ok"])
        bdir = Path(out["backup"])
        man = json.loads((bdir / "MANIFEST.json").read_text())
        self.assertTrue(man["ok"])
        files = {f["file"] for f in man["files"]}
        for full in ks.GH_REPOS:
            name = full.split("/")[1]
            self.assertIn(f"github/{name}.bundle", files)
            self.assertIn(f"github/{name}-releases.json", files)
            self.assertIn(f"github/{name}-issues.json", files)
            # the bundle really holds the repo: clone it back, every branch and tag there
            back = self.root / "restore" / name
            fakes._git("clone", "-q", "--mirror", str(bdir / "github" / f"{name}.bundle"), str(back))
            refs = fakes._git("for-each-ref", "--format=%(refname)", cwd=back).split()
            self.assertEqual(sorted(refs), ["refs/heads/feat", "refs/heads/main", "refs/tags/v1"])
        self.assertFalse((bdir / "github" / "mirrors").exists())
        self.assertIn("discord/channels.json", files)
        gen = json.loads((bdir / "discord" / "messages" / "general-21.json").read_text())
        self.assertEqual(len(gen["messages"]), 150)                       # two pages
        self.assertEqual(json.loads((bdir / "discord" / "skipped.json").read_text())[0]["channel"], "secret")
        tar = [f for f in files if f.startswith("portal/portal-data-")]
        self.assertEqual(len(tar), 1)
        # deletions: only after the backup's manifest was there
        dels = [o for o in self.gh.ops if o[0] == "delete"]
        self.assertEqual(sorted(o[1] for o in dels), sorted(ks.GH_REPOS))
        self.assertTrue(all(o[2] for o in dels))
        self.assertEqual(self.gh.repos, {})
        cdels = [o for o in self.dc.ops if o[0] == "delete"]
        self.assertTrue(cdels and all(o[2] for o in cdels))
        self.assertEqual(sorted(c["id"] for c in self.dc.chans), ["10", "12"])   # the farewell and its category
        self.assertEqual([o[1] for o in cdels][-1], "20")                         # categories after their channels
        # the website: wiped but the farewell, the news and the notice
        left = sorted(self.srv.snapshot())
        self.assertEqual(left, ["leyline/news.json", "leyline/shutdown.json", "leyline/version-policy.json"])
        self.assertIn("delete the server yourself", "\n".join(out["manual"]) + self.logpath.read_text())
        self.assertIn("DNS", "\n".join(out["manual"]))
        # Undo can't come back from that
        with self.assertRaises(ks.Refused):
            ks.run_resume(self.ctx(), dict(rec, deleted=True))

    def assertNothingDeleted(self):
        self.assertFalse(any(o[0] == "delete" for o in self.gh.ops + self.dc.ops))
        self.assertEqual(len(self.gh.repos), 3)
        self.assertTrue((self.srv.data / "profiles" / "accounts.json").exists())

    def test_a_failed_backup_deletes_nothing(self):
        rec = self.ready()
        cases = [("mirror", lambda: setattr(self.gh, "fail_mirror", "orionopenwow/Leyline")),
                 ("verify", lambda: setattr(self.gh, "fail_verify", True)),
                 ("server", lambda: self.srv.fail.add("backup")),
                 ("fetch", lambda: setattr(self.srv, "mangle_fetch", True))]
        for name, breakit in cases:
            with self.subTest(name):
                self.gh.fail_mirror, self.gh.fail_verify, self.srv.mangle_fetch = None, False, False
                self.srv.fail.discard("backup")
                breakit()
                with self.assertRaises(ks.Refused):
                    ks.run_delete(self.ctx(parts=["site", "discord", "github"]), ks.new_id(), rec)
                self.assertNothingDeleted()
        self.gh.fail_mirror = None
        self.gh.fail_verify = False
        self.srv.mangle_fetch = False
        self.srv.fail.discard("backup")
        with self.assertRaisesRegex(ks.Refused, "Discord is ticked"):
            ks.run_delete(self.ctx(parts=["discord"], discord=None), ks.new_id(), rec)
        self.assertNothingDeleted()

    def test_dry_run_delete(self):
        rec = self.pause()
        self.gh._scopes.add("delete_repo")
        before = self.srv.snapshot()
        out = ks.run_delete(self.ctx(parts=["site", "discord", "github"], dry=True), ks.new_id(), rec)   # no wait in a dry run
        self.assertFalse(out["ok"])
        self.assertNothingDeleted()
        self.assertEqual(self.srv.snapshot(), before)
        log = self.logpath.read_text()
        self.assertIn("(dry run) would delete orionopenwow/Leyline", log)
        self.assertIn("(dry run) would wipe", log)


class ServerHalf(Base):
    def test_wipe_refuses(self):
        ok, doc, _ = self.srv.call("wipe", {"sha256": "0" * 64})
        self.assertFalse(ok)
        self.assertIn("not paused", doc["error"])
        self.pause(parts=["site"])
        ok, doc, _ = self.srv.call("wipe", {"sha256": "0" * 64})
        self.assertIn("no backup", doc["error"])
        ok, b, _ = self.srv.call("backup")
        self.assertTrue(ok)
        self.assertTrue(ks.sha256_file(b["path"]) == b["sha256"])
        ok, doc, _ = self.srv.call("wipe", {"sha256": "1" * 64})
        self.assertFalse(ok)
        ok, doc, _ = self.srv.call("wipe", {"sha256": b["sha256"]}, dry=True)
        self.assertTrue(ok and doc["dry"] and doc["removed"] > 0)
        self.assertTrue((self.srv.data / "profiles" / "accounts.json").exists())
        ok, doc, _ = self.srv.call("wipe", {})
        self.assertFalse(ok)

    def test_second_pause_keeps_the_first_before_state(self):
        self.pause(parts=["message"])
        self.pause(parts=["message", "site", "realms"])
        out = ks.run_resume(self.ctx(), {"id": "x" * 8, "parts": ["message", "site", "realms"], "results": {}})
        self.assertTrue(out["ok"], out["problems"])
        self.assertEqual(self.lj("leyline/version-policy.json")["notice"], "")
        self.assertFalse(self.lj("admin/relay-block.json")["nodes_off"])
        self.assertEqual([p["id"] for p in self.lj("leyline/news.json")["posts"]], ["n1"])

    def test_status_and_bad_input(self):
        ok, st, _ = self.srv.call("status")
        self.assertTrue(ok)
        self.assertEqual(st["realms"], ["aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb"])
        self.assertIsNone(st["active"])
        self.assertFalse(self.srv.call("pause", {"id": "bad id!", "message": "x", "parts": ["site"]})[0])
        self.assertFalse(self.srv.call("pause", {"id": "abcdef12", "message": "", "parts": ["site"]})[0])
        self.assertFalse(self.srv.call("nuke")[0])
        self.assertFalse(self.srv.call("resume", {"id": "abcdef12"})[0])     # nothing paused

    def test_real_server_adapter_commands(self):
        """killswitch.Server builds ssh commands: the owner's key, BatchMode, the upload once, --dry-run passed."""
        class Sh:
            def __init__(s):
                s.calls = []

            def run(s, argv, input=None, timeout=0, cwd=None, env=None):
                s.calls.append((argv, input))
                return 0, 'KS-JSON {"x": 1}\nKS OK\n', ""
        sh = Sh()
        srv = ks.Server(sh, host="root@example.invalid", key="/k")
        ok, doc, _ = srv.call("status")
        ok2, _, _ = srv.call("pause", {"id": "abcdef12"}, dry=True)
        self.assertTrue(ok and ok2 and doc == {"x": 1})
        self.assertEqual(len(sh.calls), 3)                                  # one upload, two calls
        self.assertEqual(sh.calls[0][1], ks.SERVER_SCRIPT.read_bytes())
        self.assertIn("BatchMode=yes", sh.calls[1][0])
        self.assertTrue(sh.calls[2][0][-1].endswith("ks_server.py pause --dry-run"))
        with self.assertRaises(ks.Refused):
            srv.fetch("/etc/passwd", "/tmp/x")
        self.assertEqual(ks.parse_ks(1, "KS-JSON {\"error\": \"no\"}\nKS FAILED: no", "")[1]["error"], "no")
        self.assertFalse(ks.parse_ks(0, "no ok line", "")[0])


class RealAdapters(unittest.TestCase):
    def test_gh_adapter_parsing(self):
        class Sh:
            def __init__(s):
                s.calls = []

            def run(s, argv, input=None, timeout=0, cwd=None, env=None):
                s.calls.append(argv)
                a = " ".join(argv)
                if a == "gh api user --jq .login":
                    return 0, "orionopenwow\n", ""
                if a == "gh api -i user":
                    return 0, "HTTP/2.0 200 OK\nX-Oauth-Scopes: repo, delete_repo, read:org\n\n{}", ""
                if a == "gh api repos/orionopenwow/gone":
                    return 1, '{"message":"Not Found"}', "gh: Not Found (HTTP 404)"
                if a.startswith("gh api repos/o/r/releases?per_page=100&page=1"):
                    return 0, json.dumps([{"tag_name": f"v{i}", "assets": []} for i in range(100)]), ""
                if a.startswith("gh api repos/o/r/releases?per_page=100&page=2"):
                    return 0, json.dumps([{"tag_name": "last", "assets": [{"name": "a", "size": 1}]}]), ""
                if a.startswith("gh api -X PATCH repos/o/r"):
                    return 0, "{}", ""
                return 1, "", "unexpected"
        sh = Sh()
        g = ks.GitHub(sh)
        self.assertEqual(g.login(), "orionopenwow")
        self.assertEqual(g.scopes(), {"repo", "delete_repo", "read:org"})
        self.assertIsNone(g.repo("orionopenwow/gone"))
        rel = g.releases("o/r")
        self.assertEqual(len(rel), 101)
        self.assertEqual(rel[-1]["assets"][0]["name"], "a")
        g.edit("o/r", archived=True, private=False)
        self.assertEqual(sh.calls[-1], ["gh", "api", "-X", "PATCH", "repos/o/r", "-F", "archived=true", "-F", "private=false"])
        with self.assertRaises(ks.Refused):
            g.delete("o/x")
        for c in sh.calls:
            self.assertNotIn("auth", c[:3])        # it never asks gh for its token

    def test_discord_adapter_against_the_fake_discord_and_no_token_anywhere(self):
        sys.path.insert(0, str(HERE.parent / "discord-leyline"))
        import test_discord_leyline as fdl
        tmp = Path(tempfile.mkdtemp(prefix="ks-dc-"))
        fake = fdl.Fake(fdl.base_world(), tmp)
        srv = fdl.serve(fake)
        try:
            api = ks.DiscordAPI(fdl.TOKEN, api_base=f"http://127.0.0.1:{srv.server_address[1]}/api", sleep=lambda s: None)
            root, gh, _dc, lsrv, steps, desk = fakes.world()
            log = ks.Log(root / "logs" / "killswitch-x.log")
            log.hide(fdl.TOKEN)
            ctx = ks.Ctx(title="Leyline is closing", message=GOOD_MSG + "\nkey " + fdl.TOKEN, parts=["message", "discord"],
                         options={}, log=log, github=gh, discord=api, server=lsrv, run_step=steps, backup_root=desk)
            with self.assertRaises(RuntimeError):     # no writes outside a run
                api.post("1", "x")
            p = ks.preview(ctx, "pause")
            self.assertFalse([x for x in fake.log if x[0] != "GET"], "the preview wrote")
            self.assertTrue(any(i["kind"] == "do" and i["part"] == "discord" for i in p["items"]), p)
            ctx.message = GOOD_MSG
            rec = ks.run_pause(ctx, ks.new_id())
            self.assertTrue(rec["ok"], rec["problems"])
            writes = [x for x in fake.log if x[0] != "GET"]
            self.assertEqual(writes[0][0], "POST")
            self.assertIn("/messages", writes[0][1])
            self.assertTrue(any(w[0] == "PUT" and "/pins/" in w[1] for w in writes))
            self.assertTrue(any(w[0] == "PUT" and "/permissions/" in w[1] for w in writes))
            ks.run_resume(ctx, rec)
            log("a line with the token " + fdl.TOKEN)
            text = (root / "logs" / "killswitch-x.log").read_text() + json.dumps(rec)
            self.assertNotIn(fdl.TOKEN, text)
            for part in fdl.TOKEN.split("."):
                self.assertNotIn(part, text)
        finally:
            srv.shutdown()
            srv.server_close()


if __name__ == "__main__":
    unittest.main(verbosity=1)
