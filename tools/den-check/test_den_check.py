#!/usr/bin/env python3
"""Tests for den-check / den-test (no game, no Windows needed):

    python3 -I tools/den-check/test_den_check.py

* the MPQ reader opens the archive the launcher's writer makes (fixtures/patch-Z-sample.bin, kept equal to the
  writer's output by launcher/src-tauri/src/hybrid.rs), finds its login screen list and the empty handoff;
* `check` on a stand-in game folder: what it says with our archive, with a sign-in left in it, with another mod's;
* the headless client against a real Den Lite, when one is built here (DEN_LITE_BIN, or labs/den-lite's build in
  $CARGO_TARGET_DIR): sign in, realm list, world, character list, enter the world, the Den panel's answer;
* den-test's milestones read from a launcher report; the control steps are registered.
"""
import ast
import contextlib
import io
import json
import os
import shutil
import sys
import tempfile
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import den_check  # noqa: E402
import mpqlite  # noqa: E402

FIXTURE = HERE / "fixtures" / "patch-Z-sample.bin"


def quiet(fn, *a):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        r = fn(*a)
    return r, buf.getvalue()


def den_lite_bin():
    if os.environ.get("DEN_LITE_BIN"):
        return Path(os.environ["DEN_LITE_BIN"])
    for t in filter(None, [os.environ.get("CARGO_TARGET_DIR"), str(HERE.parent.parent / "labs" / "den-lite" / "target")]):
        for kind in ("debug", "release"):
            p = Path(t) / kind / "den-lite"
            if p.is_file():
                return p
    return None


class Mpq(unittest.TestCase):
    def test_the_writers_archive_opens(self):
        a = mpqlite.Archive(str(FIXTURE))
        self.assertEqual(a.version, 0)
        names = a.names()
        for n in ("Interface\\GlueXML\\GlueXML.toc", "Interface\\GlueXML\\LeylineHybrid.xml", den_check.OUR_LUA, den_check.SESSION,
                  "Interface\\Glues\\Leyline\\LeylineMark.tga"):
            self.assertIn(n, names)
            self.assertTrue(a.has(n.lower()), "names without regard to case")
        toc = a.read(den_check.TOC).decode()
        self.assertIn("ForeverUIGlue.xml\r\nLeylineHybrid.xml\r\n", toc)
        self.assertIn(den_check.NO_SESSION, a.read(den_check.SESSION))
        lua = a.read(den_check.OUR_LUA)
        self.assertEqual(lua, (HERE.parent.parent / "launcher/src-tauri/glue/LeylineHybrid.lua").read_bytes(), "the glue, byte for byte")
        self.assertIsNone(a.read("Interface\\GlueXML\\AccountLogin.lua"))
        with self.assertRaises(mpqlite.MpqError):
            mpqlite.Archive("x", data=b"not an archive" * 10)

    def test_an_independent_reader_agrees_when_installed(self):
        try:
            import mpyq  # noqa: F401
        except ImportError:
            self.skipTest("mpyq isn't installed here")
        import mpyq
        m = mpyq.MPQArchive(str(FIXTURE), listfile=True)
        a = mpqlite.Archive(str(FIXTURE))
        for n in a.names():
            self.assertEqual(m.read_file(n), a.read(n), n)

    def test_the_clients_order(self):
        names = ["patch-Z.MPQ", "patch.MPQ", "common.MPQ", "patch-3.MPQ", "patch-4.MPQ", "lichking.MPQ", "patch-T.MPQ", "patch-2.MPQ", "expansion.MPQ", "common-2.MPQ"]
        self.assertEqual(sorted(names, key=den_check.patch_key),
                         ["common.MPQ", "common-2.MPQ", "expansion.MPQ", "lichking.MPQ", "patch.MPQ", "patch-2.MPQ", "patch-3.MPQ", "patch-4.MPQ", "patch-T.MPQ", "patch-Z.MPQ"])
        loc = ["patch-enUS-Z.MPQ", "locale-enUS.MPQ", "patch-enUS-3.MPQ", "patch-enUS.MPQ"]
        self.assertEqual(sorted(loc, key=lambda n: den_check.patch_key(n, "enUS")), ["locale-enUS.MPQ", "patch-enUS.MPQ", "patch-enUS-3.MPQ", "patch-enUS-Z.MPQ"])


class Check(unittest.TestCase):
    def setUp(self):
        self.d = Path(tempfile.mkdtemp())
        self.game = self.d / "Game"
        (self.game / "Data" / "enUS").mkdir(parents=True)
        (self.game / "WTF").mkdir()
        (self.game / "Wow.exe").write_bytes(b"MZ stand-in")
        (self.game / "Data" / "common.MPQ").write_bytes(b"not a real archive")
        (self.game / "Data" / "enUS" / "realmlist.wtf").write_text("set realmlist 127.0.0.1\n")
        (self.game / "WTF" / "Config.wtf").write_text('SET accountName "DEN"\nSET realmName "Your Den"\nSET realmList "127.0.0.1"\nSET movie "0"\n')
        self.app = self.d / "app"
        (self.app / "den-lite" / "bin").mkdir(parents=True)
        (self.app / "settings.json").write_text(json.dumps({"clients": [{"id": "c1", "name": "Mine", "path": str(self.game), "build": "3.3.5a"}],
                                                            "selected_client": "c1"}))

    def tearDown(self):
        shutil.rmtree(self.d, ignore_errors=True)

    def run_check(self, *extra):
        a = den_check.argparse.Namespace(mode="check", game=None, app_dir=str(self.app), no_login=True, invite=False, timeout=5)
        rep = den_check.Report()
        _, out = quiet(den_check.check, a, rep)
        return rep, out

    def test_a_stock_folder_without_forever_ui(self):
        rep, out = self.run_check()
        self.assertIn("none: Forever UI's login screens aren't in this folder, so the launcher doesn't write it; the launcher's keys sign in instead", out)
        self.assertIn("PASS Config.wtf is set for Your Den", out)
        self.assertIn("archives in Data: common.MPQ", out)
        fails = [r[1] for r in rep.failed()]
        self.assertEqual(fails, ["Den Lite program"], out)

    def test_our_archive_is_read_and_a_left_sign_in_fails(self):
        shutil.copy(FIXTURE, self.game / "Data" / "patch-Z.MPQ")
        shutil.copy(FIXTURE, self.game / "Data" / "enUS" / "patch-enUS-Z.MPQ")
        (self.app / "den-lite" / "bin" / den_check.exe_name("den-lite")).write_bytes(b"x")
        rep, out = self.run_check()
        self.assertIn("PASS Leyline's login archive opens: 7 files", out)
        self.assertIn("PASS no sign-in left in the archive", out)
        self.assertIn("PASS Leyline's archive in Data\\enUS", out)
        self.assertIn("patch-Z.MPQ [login screen list (GlueXML.toc) inside]", out)
        self.assertEqual(rep.failed(), [])
        # a sign-in left in it: the stand-in for "the game didn't close yet / the launcher crashed"
        a = mpqlite.Archive(str(FIXTURE))
        sess = a.read(den_check.SESSION)
        raw = FIXTURE.read_bytes()
        orig = den_check.our_archive
        den_check.our_archive = lambda p: (True, True, True, a.names())
        try:
            rep, out = self.run_check()
        finally:
            den_check.our_archive = orig
        self.assertIn("FAIL no sign-in left in the archive", out)
        self.assertIn("The one fix", quiet(rep.end, "DEN CHECK")[1])
        self.assertTrue(sess and raw)

    def test_another_mods_archive_and_forever_ui(self):
        (self.game / "Data" / "patch-Z.MPQ").write_bytes(b"MPQ\x1a" + b"\0" * 60)
        g = self.game / "Interface" / "GlueXML"
        g.mkdir(parents=True)
        (g / "GlueXML.toc").write_text("ForeverUIGlue.xml\n")
        (g / "ForeverUIGlue.xml").write_text("<Ui/>")
        (self.game / "Leyline").mkdir()
        (self.game / "Leyline" / "glue-refused").write_text("x")
        rep, out = self.run_check()
        self.assertIn("another mod's archive", out)
        self.assertIn("INFO Forever UI's login screens: in Interface\\GlueXML", out)
        self.assertIn("this game refused Leyline's login screen once", out)

    def test_loose_copies_of_the_games_own_addons_are_named(self):
        rep, out = self.run_check()
        self.assertIn("INFO copies of the game's own addons in Interface\\AddOns: none", out)
        for n in ("Blizzard_CombatLog", "AzerothAdmin", "MiniGM"):
            (self.game / "Interface" / "AddOns" / n).mkdir(parents=True)
        rep, out = self.run_check()
        self.assertIn("INFO copies of the game's own addons in Interface\\AddOns: Blizzard_CombatLog: the likeliest cause", out)
        self.assertNotIn("AzerothAdmin", out, "other addons are no business of this line")

    def test_no_launcher_settings(self):
        shutil.rmtree(self.app)
        rep, out = self.run_check()
        self.assertEqual([r[1] for r in rep.failed()], ["launcher settings"])


class AgainstDenLite(unittest.TestCase):
    def test_the_exact_sign_in_steps(self):
        b = den_lite_bin()
        if not b:
            self.skipTest("no Den Lite built here (cargo build in labs/den-lite, or DEN_LITE_BIN)")
        steps = den_check.den_lite_login(b)
        self.assertEqual([s[0] for s in steps], ["sign in", "realm list", "world server", "character list", "enter the world", "Den panel answers"], steps)
        self.assertTrue(all(s[1] for s in steps), steps)
        self.assertIn("Your Den at 127.0.0.1:", steps[1][2])
        self.assertIn("role=owner", steps[5][2])
        bad = den_check.den_lite_login(b, "DEN", "den", typed="wrong")
        self.assertEqual((bad[0][0], bad[0][1]), ("sign in", False))
        self.assertIn("wrong password", bad[0][2])
        self.assertEqual(len(bad), 1, "it stops at the first failure")


def free_port():
    import socket
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


class DenSteps(unittest.TestCase):
    """The Den's smoke test (den_check.den_steps) against a real Den Lite started for a test, with this test's
    headless game as the host in the world and a stand-in for the launcher's answers (an invite link made with
    `den-lite guest add`, Forge's files reported as made). Everything but the real game window."""

    def setUp(self):
        self.bin = den_lite_bin()
        if not self.bin:
            self.skipTest("no Den Lite built here (cargo build in labs/den-lite, or DEN_LITE_BIN)")
        self.env = os.environ.get("DEN_LITE_BIN")
        os.environ["DEN_LITE_BIN"] = str(self.bin)
        self.d = Path(tempfile.mkdtemp(prefix="den-steps-"))
        self.root = self.d / "den-lite"
        self.dens = self.root / "state" / "dens"
        self.dens.mkdir(parents=True)
        self.lp, self.wp = free_port(), free_port()
        self.p = den_check.subprocess.Popen([str(self.bin), "--dens", str(self.dens), "--bind", "127.0.0.1", "--login-port", str(self.lp),
                                             "--world-port", str(self.wp), "--password", "den", "--no-companion", "--test-control"],
                                            stdin=den_check.subprocess.DEVNULL, stdout=den_check.subprocess.DEVNULL, stderr=den_check.subprocess.DEVNULL)
        den_check.wait_for(lambda: den_check.port_open(self.lp), 5, 0.1)
        import threading
        import wowclient
        K, realms = wowclient.login("127.0.0.1", self.lp, "DEN", "den")
        self.w = wowclient.World(realms[0][1], "DEN", K)
        self.w.until(wowclient.SMSG_AUTH_RESPONSE)
        self.w.send(wowclient.CMSG_CHAR_ENUM)
        b = self.w.until(wowclient.SMSG_CHAR_ENUM)
        self.w.send(wowclient.CMSG_PLAYER_LOGIN, den_check.struct.pack("<Q", den_check.struct.unpack_from("<Q", b, 1)[0]))
        self.w.until(wowclient.SMSG_LOGIN_VERIFY_WORLD)
        self.w.s.settimeout(None)
        self.stop = threading.Event()

        def drain():   # the host's game reads what Den Lite sends it
            try:
                while True:
                    self.w.frame()
            except Exception:  # noqa: BLE001 - the socket closed
                pass

        def launcher():  # the launcher's test answers (app.rs den_watch with a test)
            while not self.stop.is_set():
                ask = den_check.read_json(self.dens / "den-lite.ask")
                if ask:
                    (self.dens / "den-lite.ask").unlink()
                    if ask["ask"] == "invite":
                        r = den_check.subprocess.run([str(self.bin), "guest", "add", "--dens", str(self.dens), "--name", "Rowan"], capture_output=True, text=True)
                        g = json.loads(r.stdout)
                        self.link = "https://relay.example.test/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~%s.%s" % (g["user"], g["pass"])
                        (self.root / "den-click.invite").write_text(json.dumps({"link": self.link}))
                        (self.dens / "den-lite.answer").write_text(json.dumps({"id": ask["id"], "invite": self.link}))
                    else:
                        (self.root / "den-click.forge.json").write_text(json.dumps({"ok": True, "installed": False, "opened": False, "tile": "32,32",
                                                                                     "tile_bytes": 4096, "den_file": "smoketest.den.json"}))
                        (self.dens / "den-lite.answer").write_text(json.dumps({"id": ask["id"], "say": "Test: Forge's files are ready."}))
                time.sleep(0.1)

        threading.Thread(target=drain, daemon=True).start()
        threading.Thread(target=launcher, daemon=True).start()

    def tearDown(self):
        if not getattr(self, "bin", None):
            return
        self.stop.set()
        try:
            self.w.s.shutdown(2)
        except OSError:
            pass
        self.w.close()
        self.p.terminate()
        self.p.wait(5)
        if self.env is None:
            os.environ.pop("DEN_LITE_BIN", None)
        else:
            os.environ["DEN_LITE_BIN"] = self.env
        shutil.rmtree(self.d, ignore_errors=True)

    def test_every_step_passes_and_the_den_is_left_as_it_was(self):
        ch = den_check.FileChannel(self.dens)
        # the owner's Den before the test: one lantern, the stone room, and a backup of their own
        self.assertEqual(ch.send(".den place lantern")["props"], 1)
        ch.send(".den room stone")
        (self.dens / "backup.den.json").write_text("my own backup")
        rep = den_check.Report()
        _, out = quiet(den_check.den_steps, rep, ch, self.dens, self.root, {den_check.TEST_MARK}, den_check.now_ms() - 1000)
        text = rep.text()
        self.assertEqual(rep.failed(), [], out)
        for line in ("PASS build: place a piece: .den place chair", "PASS build: Den Lite logged it: den_change +1 -0", "PASS build: .build undo takes it back",
                     "PASS save: .den save smoketest: smoketest.den.json, 1 props", "PASS load: .den load smoketest replace", "PASS save: the test save removed",
                     "PASS room: the white room: The room is white now.", "PASS room: its size: Your Den is size 1 of 4",
                     "PASS Forge: the Den asked the launcher", "PASS Forge: the editor's files: your Den's tile 32,32", "INFO Forge (the editor): not installed",
                     "PASS Forge: the answer reached your game", "PASS share: a Den file the site takes: the site's own check passes (1 props)",
                     "TODO share: upload a Den from Leyline: not built yet",
                     "PASS invite: a link for one friend: https://relay.example.test/join/kzdv...#(secret hidden)~~G",
                     "PASS invite: the link went into your Den panel", "PASS invite: a friend signs in with it: as Rowan", "PASS invite: your game was shown the friend: seen by Wren",
                     "PASS invite: the friend's login taken back: {\"revoked\":1}",
                     "PASS autosave: Den Lite kept the Den by itself"):
            self.assertIn(line, text)
        m = den_check.INVITE_RE.match(self.link)
        self.assertNotIn(m.group(3), text, "never the Den's secret")
        self.assertNotIn(m.group(5), out, "never the friend's password")
        # left as it was: the lantern, the stone room, the owner's backup, no test save, no friend's login
        state = ch.send(".den size")
        self.assertEqual((state["props"], state["look"]), (1, "stone"))
        self.assertEqual((self.dens / "backup.den.json").read_text(), "my own backup")
        self.assertFalse((self.dens / "smoketest.den.json").exists())
        self.assertFalse((self.root / "den-click.invite").exists())
        live = den_check.subprocess.run([str(self.bin), "guest", "list", "--dens", str(self.dens)], capture_output=True, text=True).stdout
        self.assertEqual(json.loads(live), [], "the friend's login is taken back")
        # and the log check finds a secret where it would be
        rep2 = den_check.Report()
        (self.d / "leak.log").write_text("link " + self.link)
        quiet(den_check.no_secrets_in_logs, rep2, [m.group(3)], [self.d / "leak.log"], "")
        self.assertIn("FAIL invite: its secret is in no log: leak.log", rep2.text())

    def test_an_older_launcher_never_gets_the_asks_and_a_silent_den_lite_stops_the_steps(self):
        rep = den_check.Report()
        _, out = quiet(den_check.den_steps, rep, den_check.FileChannel(self.dens), self.dens, self.root, set(), den_check.now_ms())
        fails = [r[1] for r in rep.failed()]
        self.assertEqual(fails, ["Forge: the editor's files", "invite: a link for one friend"], out)
        self.assertIn("PASS autosave: Den Lite kept the Den by itself (1 props)", rep.text(), "kept while the piece was down")
        self.assertFalse((self.root / "den-click.forge.json").exists())
        # a Den Lite that doesn't take the test's commands: one FAIL, the rest SKIP
        quiet_ch = den_check.FileChannel(self.d / "nowhere")
        (self.d / "nowhere").mkdir()
        orig = den_check.FileChannel.send
        den_check.FileChannel.send = lambda self, cmd, wait=10.0: orig(self, cmd, 0.3)
        try:
            rep = den_check.Report()
            quiet(den_check.den_steps, rep, quiet_ch, self.d / "nowhere", self.root, {den_check.TEST_MARK}, den_check.now_ms())
        finally:
            den_check.FileChannel.send = orig
        self.assertEqual([r[1] for r in rep.failed()], ["the test's commands"])
        self.assertTrue(all(r[0] == "SKIP" for r in rep.rows[1:]), rep.text())


class Pieces(unittest.TestCase):
    def test_invite_links_are_checked_and_never_shown_whole(self):
        good = "https://relay.leylinelauncher.com/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~GAB12.ABCDEFGH23456789"
        self.assertTrue(den_check.INVITE_RE.match(good))
        r = den_check.redact(good)
        self.assertEqual(r, "https://relay.leylinelauncher.com/join/kzdv...#(secret hidden)~~GA...(password hidden)")
        for bad in ("", "https://relay/join/x#y", good.replace("~~", "~"), good + " ", "http" + good[5:], good.replace("kzdv", "KZDV")):
            self.assertIsNone(den_check.INVITE_RE.match(bad), bad)
            self.assertNotIn("ABCDEFGHJK", den_check.redact(bad))

    def test_todo_lines_dont_fail_the_test(self):
        rep = den_check.Report()
        quiet(rep.add, "TODO", "share: upload a Den from Leyline", "not built yet", "Today: ...")
        code, out = quiet(rep.end, "DEN TEST")
        self.assertEqual(code, 0)
        self.assertIn("Not built yet (doesn't fail the test): share: upload a Den from Leyline", out)
        self.assertIn("DEN TEST PASS", out)

    def test_the_newest_den_lite_only_when_its_checksum_matches(self):
        import hashlib
        d = Path(tempfile.mkdtemp())
        try:
            root = d / "den-lite"
            exe = root / "bin" / den_check.exe_name("den-lite")
            new = b"the newest den lite"
            sha = hashlib.sha256(new).hexdigest()
            calls = []

            def fetch(url, limit):
                calls.append(url)
                if url.endswith(".sha256"):
                    return ("%s  %s\n" % (sha, den_check.den_lite_asset())).encode()
                return served[0]
            served = [new]
            rep = den_check.Report()
            quiet(den_check.den_lite_update, rep, root, fetch)
            self.assertEqual(exe.read_bytes(), new)
            self.assertIn("put in place", rep.text())
            rep = den_check.Report()
            quiet(den_check.den_lite_update, rep, root, fetch)
            self.assertIn("already here", rep.text())
            self.assertEqual(len(calls), 3, "no second download when it's the same")
            sha = "0" * 64
            served = [b"something else"]
            rep = den_check.Report()
            quiet(den_check.den_lite_update, rep, root, fetch)
            self.assertIn("doesn't match its checksum", rep.text())
            self.assertEqual(exe.read_bytes(), new, "a bad download is never put in place")

            def offline(url, limit):
                raise OSError("no network")
            rep = den_check.Report()
            quiet(den_check.den_lite_update, rep, root, offline)
            self.assertIn("couldn't check", rep.text())
            self.assertEqual(rep.failed(), [])
        finally:
            shutil.rmtree(d, ignore_errors=True)


class Milestones(unittest.TestCase):
    def test_a_report_reads_as_pass_fail_with_times(self):
        start = 1_000_000
        evs = [{"t": start + 9000, "ev": "login_challenge", "who": "DEN"}, {"t": start + 9050, "ev": "auth_ok", "who": "DEN"},
               {"t": start + 9060, "ev": "realm_list"}, {"t": start + 9100, "ev": "world_auth"}, {"t": start + 9200, "ev": "char_enum"},
               {"t": start + 12000, "ev": "player_login"}, {"t": start + 15000, "ev": "in_world"}, {"t": start + 15500, "ev": "first_move"},
               {"t": start + 16000, "ev": "builder_ready"}, {"t": start + 1, "ev": "auth_ok", "detail": "remote"}]
        actions = [(4000, "the game window is open"), (9000, "send Password: login screen: the local password"), (11500, "send Enter: character select: Enter World")]
        rep = den_check.Report()
        quiet(den_check.milestones, rep, evs, start, actions)
        text = rep.text()
        self.assertIn("PASS the game window opened: 4.0 s", text)
        self.assertIn("PASS the game asked to sign in: 9.0 s, by the launcher's keys", text)
        self.assertIn("PASS in the world (the white island): 15.0 s", text)
        self.assertIn("send Password: login screen: the local password; send Enter", text)
        self.assertEqual(rep.failed(), [])
        rep = den_check.Report()
        quiet(den_check.milestones, rep, [dict(evs[0], who="DENAUTO")], start, actions)
        self.assertIn("by Leyline's login screen", rep.text())
        self.assertIn("FAIL in the world (the white island): not seen", rep.text())


class Control(unittest.TestCase):
    def test_the_steps_are_registered(self):
        src = (HERE.parent / "control" / "control.py").read_text(encoding="utf-8")
        tree = ast.parse(src)
        vals = {}
        for node in tree.body:
            if isinstance(node, ast.Assign) and len(node.targets) == 1 and getattr(node.targets[0], "id", "") in ("STEPS", "READ_ONLY"):
                vals[node.targets[0].id] = ast.literal_eval(node.value)
        for step in ("den-check", "den-test", "den-test-invite"):
            bat = vals["STEPS"][step][0]
            self.assertTrue((HERE.parent.parent / bat).is_file(), bat)
            self.assertFalse(bat.startswith("labs/"))
        self.assertIn("den-check", vals["READ_ONLY"])
        self.assertNotIn("den-test", vals["READ_ONLY"], "it starts the game: not for Autopilot on its own")
        for bat in ("Den check.bat", "Den test.bat", "Den test - invite.bat"):
            self.assertIn(b"\r\n", (HERE / bat).read_bytes(), bat + " has Windows line ends")


if __name__ == "__main__":
    unittest.main(verbosity=1)
