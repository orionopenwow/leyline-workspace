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
        # the Den bar's hello (WrathUnleashedUI DenBar.lua): Den Lite's own lines are toasts in it from now on
        self.w.addon("Wren", "OVMOD", "wuden\thello\t1/1\t1")
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
                     "PASS Forge: the answer reached your game: shown in the Den bar", "PASS share: a Den file the site takes: the site's own check passes (1 props)",
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
            # built on this PC (the den-lite step): the test keeps that build, no download
            exe.write_bytes(b"built here")
            (exe.parent / "den-lite.built-here").write_text("built on this PC")
            calls.clear()
            rep = den_check.Report()
            quiet(den_check.den_lite_update, rep, root, fetch)
            self.assertIn("built on this PC", rep.text())
            self.assertEqual((exe.read_bytes(), calls), (b"built here", []))
            # a newer Den Lite replaced it later (the launcher's download): the mark no longer counts
            t = (exe.parent / "den-lite.built-here").stat().st_mtime
            os.utime(exe, (t + 60, t + 60))
            self.assertFalse(den_check.built_here(exe))
        finally:
            shutil.rmtree(d, ignore_errors=True)


class Milestones(unittest.TestCase):
    def test_a_report_reads_as_pass_fail_with_times(self):
        start = 1_000_000
        evs = [{"t": start + 9000, "ev": "login_challenge", "who": "DEN"}, {"t": start + 9050, "ev": "auth_ok", "who": "DEN"},
               {"t": start + 9060, "ev": "realm_list"}, {"t": start + 9100, "ev": "world_auth"}, {"t": start + 9200, "ev": "char_enum"},
               {"t": start + 12000, "ev": "player_login"}, {"t": start + 15000, "ev": "in_world"}, {"t": start + 15500, "ev": "first_move"},
               {"t": start + 16000, "ev": "builder_ready"}, {"t": start + 16010, "ev": "den_ui"}, {"t": start + 1, "ev": "auth_ok", "detail": "remote"}]
        actions = [(4000, "the game window is open"), (9000, "send Password: login screen: the local password"), (11500, "send Enter: character select: Enter World")]
        rep = den_check.Report()
        quiet(den_check.milestones, rep, evs, start, actions)
        text = rep.text()
        self.assertIn("PASS the game window opened: 4.0 s", text)
        self.assertIn("PASS the game asked to sign in: 9.0 s, by the launcher's keys", text)
        self.assertIn("PASS in the world (your Den): 15.0 s", text)
        self.assertIn("PASS the Den bar opened (Build, Shape ground, Play, Save, Invite): 16.0 s", text)
        self.assertIn("send Password: login screen: the local password; send Enter", text)
        self.assertEqual(rep.failed(), [])
        rep = den_check.Report()
        quiet(den_check.milestones, rep, [dict(evs[0], who="DENAUTO")], start, actions)
        self.assertIn("by Leyline's login screen", rep.text())
        self.assertIn("FAIL in the world (your Den): not seen", rep.text())


class Walking(unittest.TestCase):
    """walk_lines: the den-test's walk check from Den Lite's events (labs/den-lite movewatch.rs) and `.den where`."""
    SPAWN = {"t": 1, "ev": "spawn", "detail": "z 13.25, ground 13.20 (the Den's ground file), on the ground (+0.05), nearest prop none"}

    def lines(self, evs, where=None):
        rep = den_check.Report()
        quiet(den_check.walk_lines, rep, evs, where)
        return rep

    def test_a_character_that_walks_on_the_ground_passes(self):
        evs = [self.SPAWN, {"t": 2, "ev": "mover", "detail": "the game moves its character"},
               {"t": 3, "ev": "first_move", "detail": "FALL_LAND, z 13.20, on the ground (+0.00)"},
               {"t": 4, "ev": "walked", "detail": "2.8 yards by HEARTBEAT, z 13.21, on the ground (+0.01)"},
               {"t": 5, "ev": "addon", "detail": "wudiag me:speed=7 falling=false dead=false control=true"}]
        rep = self.lines(evs, {"id": 1, "pos": [16229.0, 16257.0, 13.21], "ground": 13.2, "on_ground": True})
        text = rep.text()
        self.assertEqual(rep.failed(), [], text)
        self.assertIn("PASS the game took its character", text)
        self.assertIn("PASS walking: the step forward walked the character: 2.8 yards by HEARTBEAT", text)
        self.assertIn("PASS standing on the ground (feet within half a yard): z 13.21, the ground 13.20 (on it)", text)
        self.assertIn("INFO the game says: me:speed=7 falling=false dead=false control=true", text)
        self.assertIn("INFO where you came in: z 13.25", text)

    def test_the_owners_floating_character_fails_with_the_reason(self):
        # 3 Nov: no SET_ACTIVE_MOVER, no movement, a yard above the ground; and one held by a root
        rep = self.lines([dict(self.SPAWN, detail="z 14.20, ground 13.20 (the Den's ground file), 1.00 above the ground, nearest prop none")],
                         {"id": 1, "pos": [16226.2, 16257.0, 14.2], "ground": 13.2})
        text = rep.text()
        self.assertIn("FAIL the game took its character: it never said so", text)
        self.assertIn("INFO the game's first move: none: the game sent no movement at all", text)
        self.assertIn("FAIL walking: the step forward walked the character: it never got a yard from where it came in", text)
        self.assertIn("The game sent no movement at all", text)
        self.assertIn("FAIL standing on the ground (feet within half a yard): z 14.20, the ground 13.20 (1.00 above it)", text)
        evs = [self.SPAWN, {"t": 2, "ev": "mover"}, {"t": 3, "ev": "first_move", "detail": "HEARTBEAT [ROOT], z 13.25, on the ground (+0.05)"},
               {"t": 3, "ev": "move_flags", "detail": "ROOT in HEARTBEAT"}, {"t": 4, "ev": "move_flags", "detail": "SWIMMING in START_SWIM"}]
        text = self.lines(evs).text()
        self.assertIn("FAIL held up or still: ROOT in HEARTBEAT", text)
        self.assertIn("INFO movement flags: SWIMMING in START_SWIM", text)
        self.assertIn("the step forward didn't walk it", text)
        # no `.den where` answer: the ground line from where the character came in
        self.assertIn("PASS standing on the ground (feet within half a yard): where you came in: z 13.25", text)

    def test_an_older_den_lite_says_so_without_failing(self):
        rep = self.lines([{"t": 1, "ev": "first_move", "detail": "raw"}, {"t": 2, "ev": "addon", "detail": "wudiag keys:forward=W"}])
        self.assertEqual(rep.failed(), [])
        self.assertIn("INFO walking: Den Lite doesn't say", rep.text())
        self.assertIn("INFO the game says: keys:forward=W", rep.text())


def white_blp(size=64):
    """The launcher's white texture, the same layout (terrain crate blp.rs `solid`)."""
    import struct
    levels = size.bit_length()
    offs, sizes, at = [], [], 1172
    for lv in range(levels):
        s = max(size >> lv, 1)
        offs.append(at)
        sizes.append(s * s)
        at += s * s
    head = b"BLP2" + struct.pack("<IBBBBII", 1, 1, 0, 0, 1, size, size)
    head += struct.pack("<16I", *(offs + [0] * (16 - levels))) + struct.pack("<16I", *(sizes + [0] * (16 - levels)))
    return head + b"\xff\xff\xff\xff" * 256 + bytes(at - 1172)


class DenWorld(unittest.TestCase):
    def test_the_dens_world_lines(self):
        # 3 Nov: the game's own grass by name, the area's own sky, no table of the game's
        good = {"grass": True, "texture_name": den_check.GRASS, "open": 256, "island_models": 0, "models": 2,
                "sky": "the area's own", "area": "the game's own", "area_id": 876, "hidden_by": []}
        rep = den_check.Report()
        quiet(den_check.ground_lines, rep, good)
        text = rep.text()
        self.assertIn("PASS grass ground: the game's own texture: Tileset\\Elwynn\\ElwynnGrassBase.blp, from your own game", text)
        self.assertIn("PASS no buildings/trees on the Den tile: the island's models are gone, 2 of yours stay", text)
        self.assertIn("INFO sky/area: sky: the area's own; zone: the game's own", text)
        self.assertEqual(rep.failed(), [])
        # no grass in this game; something read after our archive; a ground from before
        bad = dict(good, grass=False, island_models=3, models=3, hidden_by=["patch-Z.MPQ: World\\Maps\\x.adt"], whitened="256 parts painted grass")
        rep = den_check.Report()
        quiet(den_check.ground_lines, rep, bad)
        text = rep.text()
        self.assertIn("FAIL grass ground: the game's own texture: your game hasn't a grass texture Leyline knows", text)
        self.assertIn("FAIL no buildings/trees on the Den tile: 3 of the island's models still stand", text)
        self.assertIn("INFO an archive read after the Den's hides part of it: patch-Z.MPQ", text)
        self.assertIn("INFO your Den's ground from before: made grass: 256 parts painted grass", text)
        # an older launcher (the white ground's report): said, not failed
        rep = den_check.Report()
        quiet(den_check.ground_lines, rep, {"texture_ours": True, "island_models": 0, "models": 0, "sky": "white", "area": "Your Den"})
        self.assertIn("INFO the Den's ground: an older launcher: the white ground (sky: white, area: Your Den)", rep.text())
        self.assertEqual(rep.failed(), [])
        # no report at all: one line, no failure
        rep = den_check.Report()
        quiet(den_check.ground_lines, rep, None)
        self.assertEqual([r[0] for r in rep.rows], ["INFO"])
        # patch-T not readable: the sky/area line fails
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "patch-T.MPQ"
            p.write_bytes(b"not an archive")
            rep = den_check.Report()
            quiet(den_check.ground_lines, rep, good, p)
            self.assertIn("FAIL sky/area: the game's own: Data\\patch-T.MPQ couldn't be read", rep.text())

    def test_patch_t_holds_no_table_or_texture_of_the_games(self):
        self.assertEqual(den_check.patch_extras(["World\\Maps\\Kalimdor\\Kalimdor_1_1.adt", "Leyline\\DenGround.txt"]), [])
        self.assertEqual(den_check.patch_extras(["World\\Maps\\Kalimdor\\Kalimdor_1_1.adt", "DBFilesClient\\AreaTable.dbc", den_check.DEN_TEXTURE]),
                         ["DBFilesClient\\AreaTable.dbc", den_check.DEN_TEXTURE])

    def test_our_texture_is_told_from_others(self):
        self.assertTrue(den_check.blp_is_ours(white_blp(64)))
        self.assertTrue(den_check.blp_is_ours(white_blp(256)))
        # since 2 Nov: any whole BLP2 at our name counts (the launcher may put the player's own snow texture there)
        dxt = bytearray(white_blp(64))
        dxt[8] = 2
        self.assertTrue(den_check.blp_is_ours(bytes(dxt)))
        self.assertFalse(den_check.blp_is_ours(bytes(white_blp(64))[:1200]), "cut short")
        self.assertFalse(den_check.blp_is_ours(b"BLP1" + bytes(2000)))
        self.assertFalse(den_check.blp_is_ours(None))


class Keys(unittest.TestCase):
    """The second way of pressing W (SendInput) and the Leyline UI's view before and after (3 Nov)."""

    def test_a_key_press_as_a_keyboard_sends_it(self):
        self.assertEqual(den_check.key_events(den_check.SCAN_W), [(0x11, 0x8), (0x11, 0xA)])
        self.assertEqual(den_check.key_events(den_check.SCAN_UP, True), [(0x48, 0x9), (0x48, 0xB)])

    def test_the_uis_answers_grouped_by_ask(self):
        evs = [{"ev": "addon", "detail": "wudiag me:speed=0 falling=nil dead=nil control=1"},
               {"ev": "addon", "detail": "wudiag at:before"}, {"ev": "addon", "detail": "wudiag me2:mounted=nil W*=MOVEFORWARD"},
               {"ev": "mover", "detail": "x"}, {"ev": "addon", "detail": "wudiag at:after"},
               {"ev": "addon", "detail": "wudiag blocked:BLOCKED:X:MoveForwardStart()"}]
        b = den_check.diag_blocks(evs)
        self.assertEqual(b["login"], ["me:speed=0 falling=nil dead=nil control=1"])
        self.assertEqual(b["before"], ["me2:mounted=nil W*=MOVEFORWARD"])
        self.assertEqual(b["after"], ["blocked:BLOCKED:X:MoveForwardStart()"])

    def test_den_lites_movement_count(self):
        self.assertEqual(den_check.moves_of({"me": {"moves": 4}}), 4)
        self.assertIsNone(den_check.moves_of({"said": []}))
        self.assertIsNone(den_check.moves_of(None))

    def test_which_way_moved_the_character(self):
        evs = [{"ev": "client_op", "detail": "SET_ACTIVE_MOVER"}, {"ev": "client_op", "detail": "TIME_SYNC_RESP"},
               {"ev": "time_sync", "detail": "the game answered the time sync (counter 0, its clock 9 ms)"}]
        blocks = {"before": ["me:speed=0"], "after": ["me:speed=7"]}
        rep = den_check.Report()
        quiet(den_check.method_lines, rep, False, "", 0, 5, blocks, evs)
        t = rep.text()
        self.assertIn("INFO keys: posted to the window (the launcher): no movement from the game", t)
        self.assertIn("PASS keys: pressed (SendInput, the game in front): the game moved: 5 movement packets", t)
        self.assertIn("INFO the game says (before): me:speed=0", t)
        self.assertIn("INFO the game says (after): me:speed=7", t)
        self.assertIn("INFO what the game sent in the world: SET_ACTIVE_MOVER, TIME_SYNC_RESP", t)
        self.assertIn("INFO time sync: the game answered", t)
        # neither way: one FAIL that says so
        rep = den_check.Report()
        quiet(den_check.method_lines, rep, False, "", 0, 0, {}, [])
        self.assertIn("FAIL keys: pressed (SendInput, the game in front): no movement from the game either", rep.text())
        self.assertIn("INFO time sync: the game never answered the time sync", rep.text())
        # SendInput couldn't run (not Windows, no window): said, not failed
        rep = den_check.Report()
        quiet(den_check.method_lines, rep, True, "SendInput is on Windows only", None, None, {}, [])
        self.assertEqual(rep.failed(), [])
        self.assertIn("INFO keys: posted to the window (the launcher): the game moved", rep.text())

    def test_the_check_asks_before_presses_and_asks_after(self):
        sent = []

        class Ch:
            n = 0

            def send(self, cmd, wait=10.0):
                sent.append(cmd)
                if cmd == ".den where":
                    Ch.n += 1
                    return {"me": {"moves": 0 if Ch.n == 1 else 3}}
                return {"said": []}
        pressed = []
        with tempfile.TemporaryDirectory() as d:
            rep = den_check.Report()
            orig = den_check.time.sleep
            den_check.time.sleep = lambda s: None
            try:
                quiet(den_check.keys_check, rep, Path(d), 42, 0, Ch(), lambda pid: pressed.append(pid) or "", 0)
            finally:
                den_check.time.sleep = orig
        self.assertEqual(sent, [".den diag before", ".den where", ".den where", ".den diag after"])
        self.assertEqual(pressed, [42])
        self.assertIn("PASS keys: pressed (SendInput, the game in front): the game moved: 3 movement packets", rep.text())


class FakeDenEditor:
    """A stand-in for Den Lite with the Leyline UI in the game, for den_features: `.den ui <action>` does what the
    addon's button would (DenBar.lua D.TestAction) and writes Den Lite's events, counts and the addon's report."""

    def __init__(self, dens, broken=()):
        self.dens, self.props, self.play, self.clip, self.broken = Path(dens), 3, False, False, set(broken)
        self.kind = "fake"

    def ev(self, name, detail=""):
        with open(self.dens / "den-lite.events", "a", encoding="utf-8") as f:
            f.write(json.dumps({"t": den_check.now_ms(), "ev": name, "who": "", "detail": detail}) + "\n")

    def report(self, lines):
        with open(self.dens / "den-lite.ui-report", "a", encoding="utf-8") as f:
            for l in lines:
                f.write(json.dumps({"t": den_check.now_ms(), "report": l}) + "\n")

    def send(self, cmd, wait=10.0):
        w = cmd.split()
        if cmd == ".den size":
            return {"props": self.props, "said": ["Your Den: a 16 yard room."]}
        if w[:2] == [".den", "save"]:
            (self.dens / ("%s.den.json" % w[2])).write_text('{"format": "leyline-den", "props": []}')
            return {"props": self.props}
        if w[:2] == [".den", "load"]:
            self.ev("den_load", w[2] + " replace")
            return {"props": self.props}
        a = w[2:]
        act = a[0] if a else ""
        if act in self.broken:
            return {"props": self.props}
        if act == "report":
            ov = "LeylineDenBar/MinimapCluster" if "overlap" in self.broken else "none"
            self.report(["gallery shown=1 tiles=20 model=18 icon=2 empty=0 rows=4 cols=5 tile=92",
                         "catalog n=180 model=150 boxed=150 wmo=25 none=5", "nomodel Baked Bread,Bottle",
                         "modelapi Model Pitch=0 Roll=0 Camera=1", "modelapi PlayerModel Pitch=0 Camera=1 PortraitZ=1",
                         "fit frame=Model pitch=-1.57 view=auto fit=box cam=nil pitched=false",
                         "state sel=Barrel n=1 undo=4 redo=0 clip=%d grid=0 lite=1 mode=place tab=all pick=-" % self.clip,
                         "open LeylineDenBar,LeylineDenRail", "overlaps " + ov, "pill Place · snap off · 15°",
                         "screen 1923x1082 uiscale=0.71 play=%s rail=labelled" % str(self.play).lower()])
        elif act == "place":
            self.props += 1
            self.ev("den_change", "+1 -0")
        elif act in ("nudge", "key") or a[:2] == ["menu", "drop"]:
            self.ev("den_change", "+1 -1")
        elif a[:2] == ["menu", "copy"]:
            self.clip = True
        elif a[:2] in (["menu", "paste"], ["menu", "duplicate"]) or act == "undo":
            self.props += 1
            self.ev("den_change", "+1 -0")
        elif a[:2] == ["menu", "delete"] or act == "redo":
            self.props -= 1
            self.ev("den_change", "+0 -1")
        elif act == "save":
            (self.dens / ("%s.den.json" % a[1])).write_text("{}")
        elif act == "open":
            self.ev("den_load", a[1] + " replace")
        elif act == "play":
            self.play = True
        elif act == "build":
            self.play = False
        return {"props": self.props, "said": []}


class Features(unittest.TestCase):
    """den_features against FakeDenEditor: every feature PASS with a picture each; a broken one FAILs with its fix."""

    def run_it(self, broken=()):
        d = Path(tempfile.mkdtemp(prefix="den-features-"))
        self.addCleanup(shutil.rmtree, d, True)
        dens = d / "dens"
        dens.mkdir()
        (dens / "den-lite.events").write_text(json.dumps({"t": 1, "ev": "models", "who": "", "detail": "9120"}) + "\n")
        shots = []

        def shoot(out):
            shots.append(out.name)
            out.write_bytes(b"png")
            return True, str(out)
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.den_features(rep, FakeDenEditor(dens, broken), dens, shoot, d / "shots", settle=0, wait=0.3)
        return rep, shots, dens

    def test_every_feature_passes_with_a_picture(self):
        rep, shots, dens = self.run_it()
        fails = [r for r in rep.rows if r[0] == "FAIL"]
        self.assertEqual(fails, [])
        names = [r[1] for r in rep.rows]
        for want in ("gallery: opens with tiles", "gallery: every tile a model or an icon on purpose", "place: five different props",
                     "select: the prop nearest you", "turn: Q", "paste: one more", "delete: one fewer", "undo: it is back", "redo: deleted again",
                     "files: Save by name", "files: Open by name", "play mode: and back to building", "editor: nothing over the minimap, chat, bars or player frame"):
            self.assertIn(want, names)
        self.assertGreaterEqual(sum(1 for r in rep.rows if r[1].startswith("place: ") and r[0] == "PASS"), 6)
        for s in ("gallery", "placed", "selected", "moved", "edited", "files", "menu", "snap", "keys", "stamps", "play"):
            self.assertTrue(any(x.endswith("-%s.png" % s) for x in shots), s)
        self.assertFalse((dens / "featuretest.den.json").exists() or (dens / "featuretest-before.den.json").exists(), "the test's saves removed")

    def test_the_side_view_sweep_settles_in_one_run(self):
        rep, shots, _ = self.run_it()
        fits = [x for x in shots if "-fit-" in x]
        self.assertLessEqual(len(den_check.FIT_SWEEP), 8)
        self.assertEqual(len(fits), len(den_check.FIT_SWEEP), fits)
        self.assertEqual(len({l for l, _ in den_check.FIT_SWEEP}), len(den_check.FIT_SWEEP))
        self.assertTrue(any("frame=PlayerModel" in a for _, a in den_check.FIT_SWEEP))
        api = [r[2] for r in rep.rows if r[1] == "gallery: a model frame's methods"]
        self.assertEqual(api, ["Model Pitch=0 Roll=0 Camera=1", "PlayerModel Pitch=0 Camera=1 PortraitZ=1"])

    def test_a_broken_feature_fails_with_the_fix(self):
        rep, _, _ = self.run_it(broken=("place", "overlap"))
        fails = {r[1]: r for r in rep.rows if r[0] == "FAIL"}
        self.assertIn("place: Wooden Chair from the gallery", fails)
        self.assertIn("editor: nothing over the minimap, chat, bars or player frame", fails)
        self.assertIn("Den Lite didn't place", fails["place: Wooden Chair from the gallery"][3])


class Control(unittest.TestCase):
    def test_the_steps_are_registered(self):
        src = (HERE.parent / "control" / "control.py").read_text(encoding="utf-8")
        tree = ast.parse(src)
        vals = {}
        for node in tree.body:
            if isinstance(node, ast.Assign) and len(node.targets) == 1 and getattr(node.targets[0], "id", "") in ("STEPS", "READ_ONLY"):
                vals[node.targets[0].id] = ast.literal_eval(node.value)
        for step in ("den-check", "den-test", "den-test-invite", "den-features"):
            bat = vals["STEPS"][step][0]
            self.assertTrue((HERE.parent.parent / bat).is_file(), bat)
            self.assertFalse(bat.startswith("labs/"))
        self.assertIn("den-check", vals["READ_ONLY"])
        self.assertNotIn("den-test", vals["READ_ONLY"], "it starts the game: not for Autopilot on its own")
        self.assertNotIn("den-features", vals["READ_ONLY"], "it starts the game")
        for bat in ("Den check.bat", "Den test.bat", "Den test - invite.bat", "Den features.bat"):
            self.assertIn(b"\r\n", (HERE / bat).read_bytes(), bat + " has Windows line ends")


if __name__ == "__main__":
    unittest.main(verbosity=1)
