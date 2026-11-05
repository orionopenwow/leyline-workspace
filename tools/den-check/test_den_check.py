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
import math
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
        # the friend's other ways in (launcher host/denways.rs): kept out of the log too
        ways = good + "~~lan=192.168.1.20:3724~~at=home.example.net:3724"
        self.assertTrue(den_check.INVITE_RE.match(ways))
        self.assertEqual(den_check.redact(ways), r)
        for bad in (good + "~~evil=1", good + "~~lan=1.2.3.4 x", good + "~~lan="):
            self.assertIsNone(den_check.INVITE_RE.match(bad), bad)

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
        # 3 Nov: the game's own grass by name, the Elwynn sky (Light.dbc), the zone's own name
        good = {"grass": True, "texture_name": den_check.GRASS, "open": 256, "island_models": 0, "models": 2,
                "sky": "the Elwynn sky", "area": "the game's own", "area_id": 876, "hidden_by": []}
        rep = den_check.Report()
        quiet(den_check.ground_lines, rep, good)
        text = rep.text()
        self.assertIn("PASS grass ground: the game's own texture: Tileset\\Elwynn\\ElwynnGrassBase.blp, from your own game", text)
        self.assertIn("PASS no buildings/trees on the Den tile: the island's models are gone, 2 of yours stay", text)
        self.assertIn("INFO sky/area: sky: the Elwynn sky; zone: the game's own", text)
        self.assertIn("PASS sky: the Elwynn sky: Elwynn Forest's blue sky over the Den", text)
        self.assertEqual(rep.failed(), [])
        # the sky left the area's own: said why, failed
        rep = den_check.Report()
        quiet(den_check.ground_lines, rep, dict(good, sky="the area's own (your game has no Light.dbc)"))
        self.assertIn("FAIL sky: the Elwynn sky: the sky is the area's own (your game has no Light.dbc)", rep.text())
        # a launcher from before the Elwynn sky (owner's PC 3 Nov 11:20): said so, with the fix
        rep = den_check.Report()
        quiet(den_check.ground_lines, rep, dict(good, sky="the area's own"))
        self.assertIn("FAIL sky: the Elwynn sky: the sky is the area's own: this launcher is from before the Elwynn sky", rep.text())
        self.assertIn("Update the launcher", rep.text())
        # the launcher says so, but patch-T has no Light.dbc; then with it; then hidden by a later archive
        real = den_check.mpqlite.Archive

        class Fake:
            names_ = []

            def __init__(self, path):
                pass

            def names(self):
                return Fake.names_
        try:
            den_check.mpqlite.Archive = Fake
            Fake.names_ = ["World\\Maps\\Kalimdor\\Kalimdor_1_1.adt"]
            rep = den_check.Report()
            quiet(den_check.ground_lines, rep, good, "patch-T.MPQ")
            self.assertIn("FAIL sky: the Elwynn sky: the launcher says the Elwynn sky, but Data\\patch-T.MPQ has no Light.dbc", rep.text())
            Fake.names_.append("DBFilesClient\\Light.dbc")
            rep = den_check.Report()
            quiet(den_check.ground_lines, rep, good, "patch-T.MPQ")
            self.assertIn("PASS sky: the Elwynn sky: Elwynn Forest's blue sky over the Den (your own Light.dbc, in Data\\patch-T.MPQ)", rep.text())
            rep = den_check.Report()
            quiet(den_check.ground_lines, rep, dict(good, hidden_by=["patch-Z.MPQ: DBFilesClient\\Light.dbc"]), "patch-T.MPQ")
            self.assertIn("FAIL sky: the Elwynn sky: an archive read after the Den's has its own Light.dbc: patch-Z.MPQ", rep.text())
            # a language whose own patches win (ruRU) with a Light.dbc: said
            with tempfile.TemporaryDirectory() as t:
                (Path(t) / "ruRU").mkdir()
                (Path(t) / "ruRU" / "patch-ruRU-3.MPQ").write_bytes(b"x")
                (Path(t) / "enUS").mkdir()
                self.assertEqual(den_check.locale_light(t), "ruRU\\patch-ruRU-3.MPQ")
                rep = den_check.Report()
                quiet(den_check.ground_lines, rep, good, "patch-T.MPQ", t)
                self.assertIn("FAIL sky: the Elwynn sky: your game's language reads its own Light.dbc after the Den's", rep.text())
                os.rename(Path(t) / "ruRU", Path(t) / "deDE")
                self.assertIsNone(den_check.locale_light(t))
        finally:
            den_check.mpqlite.Archive = real
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
            self.assertIn("FAIL zone: the game's own, no other table: Data\\patch-T.MPQ couldn't be read", rep.text())

    def test_patch_t_holds_no_table_or_texture_of_the_games(self):
        self.assertEqual(den_check.patch_extras(["World\\Maps\\Kalimdor\\Kalimdor_1_1.adt", "Leyline\\DenGround.txt"]), [])
        self.assertEqual(den_check.patch_extras(["World\\Maps\\Kalimdor\\Kalimdor_1_1.adt", "DBFilesClient\\Light.dbc"]), [], "the Elwynn sky's table")
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
        self.views = {"tile": [0.0, 1.0], "preview": [0.0, 1.0]}
        self.guide = [False, 1]

    def view(self, which):
        turn, zoom = self.views[which]
        frame = "Model" if "topdown" in self.broken else "PlayerModel"
        return "%s,1,%.2f,%.2f,%.2f" % (frame, turn, zoom, 0.6 + turn)

    def ev(self, name, detail=""):
        with open(self.dens / "den-lite.events", "a", encoding="utf-8") as f:
            f.write(json.dumps({"t": den_check.now_ms(), "ev": name, "who": "", "detail": detail}) + "\n")

    def report(self, lines):
        with open(self.dens / "den-lite.ui-report", "a", encoding="utf-8") as f:
            for l in lines:
                f.write(json.dumps({"t": den_check.now_ms(), "report": l}) + "\n")

    def send(self, cmd, wait=10.0):
        w = cmd.split()
        if w[:3] == [".den", "jaina", "build"]:
            if "stamps" in self.broken:
                return {"props": self.props, "said": []}
            if "stamps-part" in self.broken:
                self.stamped = 4
            else:
                self.stamped = den_check.stamp_pieces(w[3]) or 20
            self.props += self.stamped
            self.ev("den_change", "+%d -0" % self.stamped)
            self.ev("dojo", "built %d" % self.stamped)
            return {"props": self.props, "said": []}
        if w[:2] == [".build", "undo"]:
            self.props -= getattr(self, "stamped", 0)
            self.stamped = 0
            return {"props": self.props, "said": []}
        if cmd == ".den size":
            return {"props": self.props, "said": ["Your Den: a 16 yard room."]}
        if cmd == ".den help":
            if "guide" not in self.broken:
                self.guide = [True, 1]
            return {"props": self.props, "said": ["The Den guide is open (Guide on the Den bar). .den commands lists the commands."]}
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
                         "view tile=%s preview=%s previewShown=1 spin=1" % (self.view("tile"), self.view("preview")),
                         "guide open=%d page=%d pages=11 key=x" % (1 if self.guide[0] else 0, self.guide[1]),
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
        elif act == "guide" and "guide" not in self.broken:
            if a[1:2] == ["close"]:
                self.guide[0] = False
            elif a[1:2] == ["next"]:
                self.guide[1] += 1
            else:
                self.guide = [True, 1]
        elif act == "view" and "view" not in self.broken:
            v = self.views[a[1]]
            if a[2] == "turn":
                v[0] = (v[0] + math.radians(float(a[3]))) % (2 * math.pi)
            else:
                v[1] = min(3.0, v[1] * 1.15 ** int(a[3]))
        elif act == "play":
            self.play = True
        elif act == "build":
            self.play = False
        return {"props": self.props, "said": []}


class FakeDojo:
    """A stand-in for Den Lite's sandbox for dojo_features: its commands write the events Den Lite writes."""

    def __init__(self, dens, broken=()):
        self.dens, self.broken = Path(dens), set(broken)
        self.sent = []

    def ev(self, name, detail):
        with open(self.dens / "den-lite.events", "a", encoding="utf-8") as f:
            f.write(json.dumps({"t": den_check.now_ms(), "ev": name, "who": "", "detail": detail}) + "\n")

    def send(self, cmd, wait=10.0):
        self.sent.append(cmd)
        w = cmd.split()
        if cmd == ".den size":
            return {"props": 3, "said": []}
        if w[:2] == [".den", "class"] and len(w) >= 4:
            n, filled = (2, 3) if "tables" in self.broken else (64, 58)
            self.ev("class", "%s %s %d %s %d 71" % (w[2], w[-1], n, w[3], filled))
        elif w[:2] == [".den", "spec"]:
            self.ev("spec", "mage %s 60 58 71" % w[2])
        elif w[:2] == [".den", "bars"] and w[2] in ("save", "use"):
            self.ev("bars", "%s %s" % (w[2], w[3]))
        elif w[:2] == [".den", "gearset"] and w[2].startswith("t") and "gear" not in self.broken and w[2] == "t7":
            self.ev("gearset", "t7 7 17")
        elif w[:3] == [".den", "dojo", "demo"] and "demo" not in self.broken:
            if w[-1] == "boss":
                self.ev("dojo", "boss cast")
                self.ev("dojo", "boss circle")
            else:
                self.ev("demo", "%s 41250 2750" % w[3])
        elif w[:3] == [".den", "dojo", "range"]:
            self.ev("dojo", "range 5")
        return {"props": 3, "said": []}


class Dojo(unittest.TestCase):
    """dojo_features against FakeDojo: every step PASS with a picture; no tables or a run without hits FAIL with the
    fix; your class and look put back."""

    def run_it(self, broken=()):
        d = Path(tempfile.mkdtemp(prefix="dojo-features-"))
        self.addCleanup(shutil.rmtree, d, True)
        dens = d / "dens"
        dens.mkdir()
        (dens / "den-lite.class.json").write_text(json.dumps({"class": "rogue", "level": 70, "specs": {"rogue": "combat"}}))
        shots = []

        def shoot(out):
            shots.append(out.name)
            out.write_bytes(b"png")
            return True, str(out)
        rep = den_check.Report()
        ch = FakeDojo(dens, broken)
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.dojo_features(rep, ch, dens, shoot, d / "shots", settle=0, wait=0.3, run_secs=0)
        return rep, shots, ch

    def test_every_step_passes_with_a_picture_and_your_class_comes_back(self):
        rep, shots, ch = self.run_it()
        fails = [r for r in rep.rows if r[0] == "FAIL"]
        self.assertEqual(fails, [])
        text = rep.text()
        for step in ("a class and spec fill the spellbook and every bar", "another spec at once", "a bar loadout saved and put back",
                     "a gear set changes the look", "a timed practice run ends with a number", "the boss marks the ground under you",
                     "the range (a dummy 40 yards out, markers)"):
            self.assertIn("PASS dojo: " + step, text)
        self.assertIn("2750 per second", text)
        self.assertIn("t7 7 17: 17 slots", text)
        self.assertIn("and casts at you", text)
        self.assertGreaterEqual(len(shots), 6)
        self.assertTrue(all(s.startswith("dojo-") for s in shots))
        self.assertIn(".den class rogue combat 70", ch.sent, "your class and spec back")
        self.assertIn(".den gearset dojo-before", ch.sent)
        self.assertEqual(ch.sent[-1], ".den gearset delete dojo-before")

    def test_no_tables_or_no_hits_fail_with_the_fix(self):
        rep, _, _ = self.run_it(broken=("tables", "demo", "gear"))
        text = rep.text()
        self.assertIn("FAIL dojo: a class and spec fill the spellbook and every bar", text)
        self.assertIn("copies your game's spell tables", text)
        self.assertIn("FAIL dojo: a timed practice run ends with a number", text)
        self.assertIn("FAIL dojo: a gear set changes the look", text)
        self.assertIn("ItemSet.dbc", text)


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
        # every stamp built and pictured, then taken back
        for name in den_check.STAMP_NAMES:
            self.assertTrue(any(x.endswith("-stamp-%s.png" % name) for x in shots), name)
            self.assertIn(("PASS", "stamp: %s" % name), [(r[0], r[1]) for r in rep.rows])
        self.assertEqual(len(den_check.STAMP_NAMES), len(set(den_check.STAMP_NAMES)))
        self.assertFalse((dens / "featuretest.den.json").exists() or (dens / "featuretest-before.den.json").exists(), "the test's saves removed")

    def test_tiles_and_preview_from_the_side_turned_and_zoomed(self):
        rep, shots, _ = self.run_it()
        rows = {r[1]: r for r in rep.rows}
        for label in ("gallery tiles", "the selection's preview"):
            self.assertEqual(rows["%s: from the side" % label][0], "PASS")
            self.assertEqual(rows["%s: right-drag turns it, the wheel zooms" % label][0], "PASS", rows["%s: right-drag turns it, the wheel zooms" % label])
        self.assertTrue(any(x.endswith("-tile-turned-zoomed.png") for x in shots) and any(x.endswith("-preview-turned-zoomed.png") for x in shots))
        self.assertFalse(any("-fit-" in x for x in shots), "the sweep is gone")
        self.assertFalse(hasattr(den_check, "FIT_SWEEP"))
        self.assertEqual(den_check.parse_view("PlayerModel,1,1.57,1.32,2.17")["zoom"], 1.32)
        self.assertEqual(den_check.parse_view("nope"), {})
        # top-down tiles, or turn/zoom that doesn't work: failed
        rep, _, _ = self.run_it(broken=("topdown", "view"))
        fails = {r[1] for r in rep.rows if r[0] == "FAIL"}
        self.assertIn("gallery tiles: from the side", fails)
        self.assertIn("the selection's preview: right-drag turns it, the wheel zooms", fails)

    def test_the_guide_opens_turns_and_answers_den_help(self):
        rep, shots, _ = self.run_it()
        rows = {r[1]: r for r in rep.rows}
        for name in ("guide: Guide on the Den bar opens it", "guide: Next turns the page", "guide: .den help opens it"):
            self.assertEqual(rows[name][0], "PASS", rows[name])
        self.assertTrue(any(x.endswith("-guide.png") for x in shots))
        rep, _, _ = self.run_it(broken=("guide",))
        fails = {r[1] for r in rep.rows if r[0] == "FAIL"}
        self.assertIn("guide: Guide on the Den bar opens it", fails)
        self.assertIn("guide: .den help opens it", fails)

    def test_the_stamp_list_is_the_shipped_stamps(self):
        shipped = sorted(p.name.split(".")[0] for p in (HERE.parent.parent / "realm" / "custom" / "lua_scripts" / "den-stamps").glob("*.den.json"))
        self.assertEqual(sorted(den_check.STAMP_NAMES), shipped)

    def test_a_stamp_that_doesnt_go_down_fails(self):
        rep, _, _ = self.run_it(broken=("stamps",))
        fails = {r[1] for r in rep.rows if r[0] == "FAIL"}
        self.assertIn("stamp: campfire", fails)
        # only part of it inside the room: failed, with the count
        rep, _, _ = self.run_it(broken=("stamps-part",))
        fails = {r[1]: r for r in rep.rows if r[0] == "FAIL"}
        self.assertIn("stamp: inn", fails)
        self.assertIn("4 pieces of 34", fails["stamp: inn"][2])
        self.assertEqual(den_check.stamp_pieces("nook"), 15)
        self.assertIsNone(den_check.stamp_pieces("no-such-stamp"))

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



class FakeAids:
    """Den Lite and the Leyline UI's Gizmo.lua for aids_features / hover_calibration: `.den ui gizmo ...` does what the
    addon's test action would, `.den size` answers with the aids (gizmo.rs `view`), the events as Den Lite writes them.
    `true_cam`: the camera the game really has (the exact clicks land by it); `cam`: the one Den Lite's preview uses."""

    def __init__(self, dens, true_cam=None, cam=None, broken=()):
        self.dens, self.props, self.broken = Path(dens), 2, set(broken)
        self.true_cam, self.cam = true_cam or dict(den_check.CAMERA), cam or dict(den_check.CAMERA)
        self.player = [16226.2, 16257.0, 13.2, 0.0]
        self.aids = {"armed": "off", "ghost": None, "at": None, "cursor": None, "axis": None, "gizmo": 0, "sel": None, "mode": "play",
                     "glow": None, "drag": 0.0, "cam": "near", "far_at": None, "zoom": 12,
                     "hovered": None, "sel_at": None, "sel_uv": None, "grip": False,
                     # the copy in hand (glide.rs): a creature made once and glided; "flicker": a gameobject made anew
                     "copy": None, "copies_made": 0, "copies_glided": 0, "glide": "on"}
        self.zoom, self.back = 12, 0.0
        self.screen = (0.0, 0.0)
        # the props the mouse can find: id -> (u, v on the screen, x, y, z)
        self.where = {7: [-0.3, -0.2, 16227.0, 16259.0, 13.2], 9: [0.1, -0.3, 16229.0, 16257.0, 13.2]}

    def under(self, u, v):
        if "mouse" in self.broken:
            return None
        for pid, w in self.where.items():
            if abs(w[0] - u) < 0.05 and abs(w[1] - v) < 0.05:
                return pid
        return None

    def mouse(self, act, a):
        if act == "click":
            hit = self.under(float(a[2]), float(a[3]))
            self.aids.update(sel=hit, glow=hit, gizmo=4 if hit else 0)
        elif act == "press":
            hit = self.under(float(a[2]), float(a[3]))
            if hit:
                self.aids.update(sel=hit, grip=True, armed="move,%s,1543" % hit)
        elif act == "moveto" and self.aids["grip"]:
            self.moved = (float(a[2]), float(a[3]))
        elif act == "release" and self.aids["grip"]:
            w = self.where[self.aids["sel"]]
            du, dv = self.moved
            w[2], w[3] = w[2] + (du - w[0]) * 10, w[3] + (dv - w[1]) * 10
            w[0], w[1] = du, dv
            self.ev("den_change", "+1 -1")
            self.aids.update(grip=False, armed="off", glow=self.aids["sel"])

    def draw(self):
        """The copy shown at a new spot: made the first time (a creature), glided after; broken "flicker": made anew."""
        if self.aids["copy"] is None or "flicker" in self.broken:
            self.aids["copies_made"] += 1
            self.aids["copy"] = "object" if "flicker" in self.broken else "creature"
        else:
            self.aids["copies_glided"] += 1

    def sync(self):
        w = self.where.get(self.aids["sel"])
        self.aids.update(sel_uv=w[:2] if w else None, sel_at=w[2:] if w else None)

    def ev(self, name, detail=""):
        with open(self.dens / "den-lite.events", "a", encoding="utf-8") as f:
            f.write(json.dumps({"t": den_check.now_ms(), "ev": name, "who": "", "detail": detail}) + "\n")

    def hover(self, u, v):
        if self.aids["armed"] == "off":
            if self.aids["mode"] == "build":
                self.aids["hovered"] = self.under(u, v)
            return
        self.aids["cursor"] = [u, v, 16 / 9]
        if self.aids["armed"].startswith("axis,"):
            self.aids["drag"] = round((u - 0.1) * 12, 2)
            return
        hit = den_check.project(self.cam, self.player, u, v, 16 / 9, self.player[2])
        if hit:
            self.aids["ghost"] = [hit[0], hit[1], self.player[2], 0.0]

    def send(self, cmd, wait=10.0):
        w = cmd.split()
        if cmd == ".den size":
            self.sync()
            look = "build" if self.aids["mode"] == "build" and "look" not in self.broken else "play"
            r = {"props": self.props, "aids": dict(self.aids), "look_mode": look, "twin_queries": getattr(self, "twins", 0)}
            if getattr(self, "before_glide", False):
                for k in ("copy", "copies_made", "copies_glided", "glide"):
                    r["aids"].pop(k, None)
            if not getattr(self, "before_5nov", False):
                r["prop_flags"] = 0x10 if look == "build" and "flags" not in self.broken else 0
            return r
        if w[:2] in ([".den", "save"], [".den", "load"]):
            if w[1] == "save":
                (self.dens / ("%s.den.json" % w[2])).write_text("{}")
            return {"props": self.props}
        a = w[2:]
        if a and a[0] == "select" and "select" not in self.broken:
            self.aids.update(sel=7, gizmo=3)
        if a[:2] == ["key", "insert"] and "swap" not in self.broken and self.aids["sel"]:
            self.ev("den_change", "+1 -1")
            return {"props": self.props, "said": []}
        if a[:1] == ["pick"]:
            # the gallery card (Builder.lua B.Choose, Gizmo.lua's hook): in hand, nothing shown yet
            self.aids.update(armed="place,%s,1543" % a[1], ghost=None, at=None)
            return {"props": self.props, "said": []}
        if a[:1] != ["gizmo"] or (len(a) > 1 and a[1] in self.broken):
            return {"props": self.props}
        act = a[1]
        if act == "build":
            self.aids["mode"] = "play" if a[2:] == ["off"] else "build"
            self.ev("build_mode", self.aids["mode"])
            if "sticky" in self.broken:
                # 5 Nov: the UI's half-second check turned build mode on again after the test's "build off"
                self.aids["mode"] = "build"
            if self.aids["mode"] == "build":
                self.twins = getattr(self, "twins", 0) + 3
        elif act == "arm":
            # in hand: nothing shown until the cursor is on the ground (since 5 Nov: shown at once, `shows_at_once`)
            self.aids.update(armed="place,%s,1543" % a[2], ghost=[16229.0, 16257.0, 13.2, 0.0] if "shows_at_once" in self.broken else None)
        elif act == "hover":
            self.hover(float(a[2]), float(a[3]))
        elif act == "commit":
            if self.aids["armed"].startswith("axis,"):
                self.ev("den_change", "+1 -1")
            else:
                self.props += 1
                self.ev("den_change", "+1 -0")
                if "glow" not in self.broken:
                    self.aids.update(sel=9, glow=9, gizmo=4)
            self.aids.update(armed="off", ghost=None, cursor=None)
        elif act == "grab":
            self.aids.update(armed="axis,%s,%s" % (self.aids["sel"], a[2]), drag=0.0)
        elif act == "snap":
            if self.aids["cam"] == "near":
                self.aids.update(cam="far,35.0,1,0", far_at=[16200.0, 16257.0, 32.0, 0.0])
            else:
                self.aids.update(cam="near", far_at=None)
        elif act == "zoom":
            steps = int(a[2])
            for _ in range(min(40, abs(steps))):
                if steps > 0:
                    if self.zoom + 2 > 50 or self.back > 0:
                        self.back = min(1500.0, self.back * 1.25 + 8)
                    else:
                        self.zoom += 2
                elif self.back > 0:
                    self.back = max(0.0, (self.back - 8) / 1.25)
                else:
                    self.zoom = max(2, self.zoom - 2)
            self.aids["cam"] = ("far,%.1f,0,0" % self.back) if self.back >= 0.5 else "near"
            self.back = self.back if self.back >= 0.5 else 0.0
        elif act == "cancel":
            self.aids.update(armed="off", ghost=None, cursor=None, copy=None)
        elif act == "axis":
            self.aids["axis"] = None if a[2] == "off" else a[2]
        elif act == "wheel":
            self.ev("den_change", "+1 -1")
        elif act == "glide":
            # Gizmo.lua's frame tick at (u, v): a hover; the copy (or the carried prop) where it lands
            u, v = float(a[2]), float(a[3])
            if self.aids["armed"].startswith("move,") and self.aids["grip"]:
                self.moved = (u, v)
                self.aids["at"] = [16227.0 + u * 10, 16259.0 + v * 10, 13.2, 0.0]
                self.draw()
            else:
                self.hover(u, v)
                if self.aids.get("ghost"):
                    self.aids["at"] = self.aids["ghost"]
                    if self.aids["armed"] != "off":
                        self.draw()
        elif act == "mouse":
            # Gizmo.lua's own press and let-go: in hand, put down; else what is under it picked up
            u, v = float(a[2]), float(a[3])
            if self.aids["armed"].startswith("place,"):
                self.props += 1
                self.ev("den_change", "+1 -0")
                self.where[11] = [u, v, 16230.0, 16258.0, 13.2]
                self.aids.update(armed="off", ghost=None, at=None, sel=11, glow=11, gizmo=4, copy=None)
            elif self.aids["armed"].startswith("move,") and self.aids["grip"]:
                self.mouse("release", ["gizmo", "release"])
                self.aids["copy"] = None
            else:
                hit = self.under(u, v)
                if hit:
                    self.moved = (u, v)
                    self.aids.update(sel=hit, grip=True, armed="move,%s,1543" % hit)
                    self.draw()
        elif act in ("click", "press", "moveto", "release"):
            self.mouse(act, a)
        elif act == "menu" and a[2] == "2":
            self.props += 1
            self.ev("den_change", "+1 -0")
        return {"props": self.props, "said": []}


class FakeMouse:
    """The real cursor for hover_calibration: move() puts it at a window point (the addon then sends its hover:
    u from the left, v up), aim_click() is the game's exact click: Den Lite writes the sample where the true camera
    says the point is."""

    def __init__(self, den, dens):
        self.den, self.dens = den, Path(dens)

    def move(self, fx, fy):
        self.uv = (2 * fx - 1, 1 - 2 * fy)
        self.den.hover(*self.uv)
        return ""

    def aim_click(self):
        d = den_check.project(self.den.true_cam, self.den.player, self.uv[0], self.uv[1], 16 / 9, 13.2)
        if d:
            with open(self.dens / den_check.SAMPLES_FILE, "a", encoding="utf-8") as f:
                f.write(json.dumps({"u": self.uv[0], "v": self.uv[1], "aspect": 16 / 9, "dist": 12.0, "player": self.den.player,
                                    "dest": [d[0], d[1], 13.2]}) + "\n")
        return ""


class PlacementAids(unittest.TestCase):
    def setUp(self):
        self.d = Path(tempfile.mkdtemp(prefix="den-aids-"))
        self.addCleanup(shutil.rmtree, self.d, True)
        self.dens = self.d / "dens"
        self.dens.mkdir()
        (self.dens / "den-lite.events").write_text("")
        self.shots = []

    def shoot(self, out):
        self.shots.append(out.name)
        out.write_bytes(b"png")
        return True, str(out)

    def test_the_ray_is_den_lites(self):
        # gizmo.rs the_ray_through_the_middle_lands_ahead_and_the_sides_mirror: the same numbers
        cam = den_check.CAMERA
        x, y = den_check.project(cam, (100.0, 50.0, 10.0, 0.0), 0.0, 0.0, 16 / 9, 10.0)
        self.assertAlmostEqual(x, 100.0 + cam["pivot"] / math.tan(cam["pitch"]), places=3)
        self.assertAlmostEqual(y, 50.0, places=3)
        _, yr = den_check.project(cam, (100.0, 50.0, 10.0, 0.0), 0.5, 0.0, 16 / 9, 10.0)
        _, yl = den_check.project(cam, (100.0, 50.0, 10.0, 0.0), -0.5, 0.0, 16 / 9, 10.0)
        self.assertTrue(yr < 50.0 < yl and abs((50 - yr) - (yl - 50)) < 1e-6)
        self.assertIsNone(den_check.project(cam, (100.0, 50.0, 10.0, 0.0), 0.0, 1.0, 16 / 9, 10.0), "above the horizon")

    def test_the_camera_is_fitted_from_clicks(self):
        true = {"fov": 1.2, "pivot": 1.6, "dist": 9.0, "pitch": 0.42}
        samples = []
        for (fx, fy) in den_check.CAL_POINTS:
            u, v = 2 * fx - 1, 1 - 2 * fy
            d = den_check.project(true, (0.0, 0.0, 5.0, 0.7), u, v, 1.6, 5.0)
            samples.append({"u": u, "v": v, "aspect": 1.6, "player": [0.0, 0.0, 5.0, 0.7], "dest": [d[0], d[1], 5.0]})
        self.assertGreater(den_check.camera_error(den_check.CAMERA, samples), 0.5, "the guesses are off")
        cam, err = den_check.fit_camera(samples)
        self.assertLess(err, 0.05, "fitted: %s %.3f" % (cam, err))

    def test_samples_at_two_zooms_fit_the_same_camera(self):
        # review 4 Nov: each sample keeps the zoom it was taken at (gizmo.rs Sample.dist), and Den Lite's ray uses
        # that zoom (Aids::view): the fit must use it too (it found fov 0.39 and a 1.3 yd error before)
        true = {"fov": 1.2, "pivot": 1.6, "dist": 9.0, "pitch": 0.42}
        samples = []
        for i, (fx, fy) in enumerate(den_check.CAL_POINTS * 2):
            dist = 8.0 if i % 2 else 16.0
            u, v = 2 * fx - 1, 1 - 2 * fy
            d = den_check.project(dict(true, dist=dist), (0.0, 0.0, 5.0, 0.7), u, v, 1.6, 5.0)
            samples.append({"u": u, "v": v, "aspect": 1.6, "dist": dist, "player": [0.0, 0.0, 5.0, 0.7], "dest": [d[0], d[1], 5.0]})
        cam, err = den_check.fit_camera(samples)
        self.assertLess(err, 0.05, "fitted: %s %.3f" % (cam, err))
        self.assertAlmostEqual(cam["fov"], 1.2, places=2)
        self.assertAlmostEqual(cam["pitch"], 0.42, places=2)
        # a sample with no zoom, or a zoom that isn't one, falls back to the camera's own distance
        self.assertEqual(den_check.camera_error(true, [dict(samples[0], dist=None)]),
                         den_check.camera_error(dict(true, dist=9.0), [dict(samples[0], dist=None)]))
        self.assertEqual(den_check.camera_error({}, []), float("inf"))

    def test_a_far_view_sample_uses_its_own_long_distance(self):
        # placement review 4 Nov: a build-view sample is cast from the view's focus at the zoom plus the pull-back
        # (gizmo.rs sample), past the game's own 50 yd: the fit uses that distance, not the camera's own
        true = {"fov": 1.3, "pivot": 1.8, "dist": 12.0, "pitch": 0.5}
        samples = []
        for (fx, fy) in den_check.CAL_POINTS:
            u, v = 2 * fx - 1, 1 - 2 * fy
            d = den_check.project(dict(true, dist=26.0), (3.0, 1.0, 5.0, 0.4), u, v, 1.78, 5.0)
            if d:
                samples.append({"u": u, "v": v, "aspect": 1.78, "dist": 26.0, "player": [3.0, 1.0, 5.0, 0.4], "dest": [d[0], d[1], 5.0]})
        self.assertTrue(samples)
        self.assertLess(den_check.camera_error(true, samples), 1e-3)
        self.assertGreater(den_check.camera_error(true, [dict(s, dist=None) for s in samples]), 0.5)

    def test_the_calibrated_window_shape_is_kept_and_another_shape_is_noted(self):
        self.assertEqual(den_check.calibrated_aspect([{"aspect": 1.6}, {"aspect": 1.78}, {"aspect": 1.78}, {"aspect": "x"}]), 1.78)
        self.assertIsNone(den_check.calibrated_aspect([]))
        (self.dens / den_check.CAMERA_FILE).write_text(json.dumps({"fov": 1.3, "aspect": 1.778}), encoding="utf-8")
        self.assertIsNone(den_check.aspect_note(self.dens, [0, 0, 1.7778]))
        self.assertIn("1.33", den_check.aspect_note(self.dens, [0, 0, 1.3333]))
        self.assertIsNone(den_check.aspect_note(self.dens, None))
        (self.dens / den_check.CAMERA_FILE).unlink()
        self.assertIsNone(den_check.aspect_note(self.dens, [0, 0, 1.3333]))

    def test_a_carry_jump_is_measured_from_where_the_prop_stood(self):
        self.assertAlmostEqual(den_check.carry_jump([1.0, 2.0, 0.0], [4.0, 6.0, 0.0, 0.0]), 5.0)
        self.assertIsNone(den_check.carry_jump([1.0, 2.0], None))
        self.assertIsNone(den_check.carry_jump(None, [1.0, 2.0]))

    def test_project_turns_with_the_player_and_reads_the_same_from_any_facing(self):
        cam = den_check.CAMERA
        for o in (0.0, 0.7, math.pi / 2, math.pi, -2.5):
            p = den_check.project(cam, (10.0, -4.0, 3.0, o), 0.0, 0.0, 16 / 9, 3.0)
            ahead = math.hypot(p[0] - 10.0, p[1] + 4.0)
            q = den_check.project(cam, (10.0, -4.0, 3.0, 0.0), 0.0, 0.0, 16 / 9, 3.0)
            self.assertAlmostEqual(ahead, math.hypot(q[0] - 10.0, q[1] + 4.0), places=4)
            self.assertAlmostEqual(math.atan2(p[1] + 4.0, p[0] - 10.0) % (2 * math.pi), o % (2 * math.pi), places=4)

    def test_every_aid_passes_with_a_picture(self):
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, FakeAids(self.dens), self.dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        self.assertEqual([r for r in rep.rows if r[0] == "FAIL"], [])
        names = [r[1] for r in rep.rows if r[0] == "PASS"]
        for want in ("build mode: props light up under the mouse", "pick: the card in hand", "follow: the real prop goes where the cursor points",
                     "click: placed, and selected at once (it glows, its handles)", "drag: a handle moves it along X, a click drops it",
                     "gizmo: the wheel moves it along the picked axis", "swap: the selection becomes the gallery's pick", "menu: Duplicate",
                     "snap out: the view from up high", "snap out: and back to you", "zoom: out past the game's limit", "zoom: and back to you",
                     "play mode: props plain again", "mouse: the prop under the cursor glows", "mouse: a click selects it (a click on the sky clears)",
                     "mouse: press and drag moves it", "build mode: props never take the game's click", "card: the copy glides under the cursor",
                     "card: a click puts it down, selected", "select: a click on a prop picks it up, the next puts it down",
                     "card: the copy glides as one creature (no destroy / create)", "select: the copy glides as one creature (no destroy / create)",
                     "play mode: props usable again (a chair is sat on)"):
            self.assertIn(want, names)
        moved = [r for r in rep.rows if r[1] == "mouse: press and drag moves it"][0][2]
        self.assertTrue(moved.startswith("moved 2.06 yd") and moved.endswith("-mouse-dropped.png"), moved)
        self.assertIn("the game asked for 3 of their goober looks", [r for r in rep.rows if r[1] == "build mode: props light up under the mouse"][0][2])
        drag = [r for r in rep.rows if r[1] == "drag: a handle moves it along X, a click drops it"][0][2]
        self.assertTrue(drag.startswith("moved +3.60 yd along X"), drag)
        far = [r for r in rep.rows if r[1] == "zoom: out past the game's limit"][0][2]
        self.assertTrue(far.startswith("1500 yd") and far.endswith("-zoom-far.png") and "yd further than the game's own camera goes" in far, far)
        for s in ("build-mode", "pick", "follow", "placed-selected", "dragging", "drag-dropped", "wheel-step", "swap", "snap-out", "snap-back",
                  "zoom-far", "zoom-back", "mouse-hover", "mouse-click", "mouse-dragging", "mouse-dropped"):
            self.assertTrue(any(x.endswith("-%s.png" % s) for x in self.shots), s)
        self.assertFalse((self.dens / "aidstest-before.den.json").exists(), "the test's save removed")

    def test_a_broken_aid_fails_with_its_fix(self):
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, FakeAids(self.dens, broken={"hover", "glow"}), self.dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        fails = {r[1] for r in rep.rows if r[0] == "FAIL"}
        # no hover: nothing follows the cursor and nothing can be dragged; no glow: placed but not shown selected (and so
        # no swap; in this fake the wheel's change a moment before can stand in for it, as both land in one millisecond)
        must = {"follow: the real prop goes where the cursor points", "click: placed, and selected at once (it glows, its handles)",
                "drag: a handle moves it along X, a click drops it"}
        self.assertTrue(must <= fails <= must | {"swap: the selection becomes the gallery's pick"}, fails)
        # the mouse finds nothing: its three checks fail, each with what to send
        rep = den_check.Report()
        dens = self.d / "dens-mouse"
        dens.mkdir()
        (dens / "den-lite.events").write_text("")
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, FakeAids(dens, broken={"mouse"}), dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        fails = {r[1] for r in rep.rows if r[0] == "FAIL"}
        self.assertEqual(fails, {"mouse: the prop under the cursor glows", "mouse: a click selects it (a click on the sky clears)", "mouse: press and drag moves it",
                                 "card: a click puts it down, selected"})
        # a Den Lite from before the mouse: said, not failed
        class NoMouse(FakeAids):
            def send(self, cmd, wait=10.0):
                r = FakeAids.send(self, cmd, wait)
                if isinstance(r, dict) and "aids" in r:
                    for k in ("sel_uv", "sel_at", "hovered", "grip"):
                        r["aids"].pop(k, None)
                return r
        rep = den_check.Report()
        dens = self.d / "dens-nomouse"
        dens.mkdir()
        (dens / "den-lite.events").write_text("")
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, NoMouse(dens), dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        self.assertTrue(any(r[0] == "INFO" and r[1] == "the mouse: hover, click and drag" for r in rep.rows))
        self.assertEqual([r for r in rep.rows if r[0] == "FAIL"], [])
        # Den Lite never took the build look: failed, with what it said
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, FakeAids(self.dens, broken={"look"}), self.dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        row = [r for r in rep.rows if r[1] == "build mode: props light up under the mouse"][0]
        self.assertEqual(row[0], "FAIL")
        self.assertTrue(row[2].startswith("Den Lite's look: play; the event: "), row[2])
        # a Den Lite from before the aids: skipped, said
        class Old(FakeAids):
            def send(self, cmd, wait=10.0):
                return {"props": 1}
        rep = den_check.Report()
        den_check.aids_features(rep, Old(self.dens), self.dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        self.assertEqual(rep.rows[0][0], "SKIP")
        # a Den Lite from before the new flow (no cam in its aids): the old checks, and said
        class Before(FakeAids):
            def send(self, cmd, wait=10.0):
                r = FakeAids.send(self, cmd, wait)
                if isinstance(r, dict) and "aids" in r:
                    r["aids"].pop("cam", None)
                return r
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, Before(self.dens), self.dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        self.assertTrue(any(r[0] == "INFO" and r[1] == "the editor's new flow" for r in rep.rows))
        self.assertNotIn("snap out: the view from up high", [r[1] for r in rep.rows])

    def test_the_owners_5_nov_flow_fails_with_its_fix_and_an_older_den_lite_is_said(self):
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, FakeAids(self.dens, broken={"flags", "glide"}), self.dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        fails = {r[1] for r in rep.rows if r[0] == "FAIL"}
        self.assertIn("build mode: props never take the game's click", fails)
        self.assertIn("card: the copy glides under the cursor", fails)
        row = [r for r in rep.rows if r[1] == "build mode: props never take the game's click"][0]
        self.assertIn("want 0x10", row[3])
        dens = self.d / "dens-before-5nov"
        dens.mkdir()
        (dens / "den-lite.events").write_text("")
        old = FakeAids(dens)
        old.before_5nov = True
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, old, dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        self.assertTrue(any(r[0] == "INFO" and r[1] == "5 Nov: the editor's click fixes" for r in rep.rows))
        self.assertEqual([r for r in rep.rows if r[0] == "FAIL"], [])

    def test_a_copy_shown_at_once_passes_and_build_mode_coming_back_fails_play_mode(self):
        # the owner's PC 5 Nov: "pick" failed for showing the copy at once (the right thing since that morning), and
        # "play mode: props usable again" read flags 16 (build mode came back half a second after the test's "build off")
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, FakeAids(self.dens, broken={"shows_at_once", "sticky"}), self.dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        rows = {r[1]: r for r in rep.rows}
        self.assertEqual(rows["pick: the card in hand"][0], "PASS")
        self.assertIn("its copy shown at once", rows["pick: the card in hand"][2])
        self.assertEqual(rows["play mode: props usable again (a chair is sat on)"][0], "FAIL")

    def test_the_carried_copy_must_glide_not_flicker(self):
        # owner 5 Nov: "dynamically move instead of disappearing, reappearing": a copy made anew at each spot fails
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            den_check.aids_features(rep, FakeAids(self.dens, broken={"flicker"}), self.dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
        fails = {r[1]: r for r in rep.rows if r[0] == "FAIL"}
        self.assertIn("card: the copy glides as one creature (no destroy / create)", fails)
        self.assertIn("select: the copy glides as one creature (no destroy / create)", fails)
        self.assertIn("The copy was object, made", fails["card: the copy glides as one creature (no destroy / create)"][3])
        # a game without the Den patch's creature rows, and a Den Lite from before the glide: said, not failed
        for kind, setup in (("none", lambda f: f.aids.update(glide="none")), ("before", lambda f: setattr(f, "before_glide", True))):
            dens = self.d / ("dens-glide-" + kind)
            dens.mkdir()
            (dens / "den-lite.events").write_text("")
            den = FakeAids(dens)
            setup(den)
            rep = den_check.Report()
            with contextlib.redirect_stdout(io.StringIO()):
                den_check.aids_features(rep, den, dens, self.shoot, self.d / "shots", settle=0, wait=0.3)
            rows = [r for r in rep.rows if r[1] == "card: the copy glides as one creature (no destroy / create)"]
            self.assertEqual(rows[0][0], "INFO", kind)
            self.assertIn("updated launcher" if kind == "none" else "from before the glide", rows[0][2])
            self.assertEqual([r for r in rep.rows if r[0] == "FAIL"], [], kind)

    def test_hover_calibration_fits_and_saves_the_camera(self):
        true = {"fov": 1.25, "pivot": 1.7, "dist": 10.0, "pitch": 0.45}
        den = FakeAids(self.dens, true_cam=true)
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            got = den_check.hover_calibration(rep, den, self.dens, self.shoot, self.d / "shots", FakeMouse(den, self.dens), settle=0, wait=0.3)
        self.assertIsNotNone(got)
        cam, err = got
        self.assertLess(err, 0.1)
        text = rep.text()
        self.assertIn("PASS hover calibration: the camera fitted: fov", text)
        self.assertIn("INFO hover calibration: before: the preview was", text)
        saved = json.loads((self.dens / den_check.CAMERA_FILE).read_text())
        self.assertEqual(set(saved), {"fov", "pivot", "dist", "pitch", "aspect"}, "the fitted camera and the window shape it was measured at")
        self.assertEqual(sum(1 for s in self.shots if s.startswith("calibrate-")), len(den_check.CAL_POINTS))
        self.assertEqual(den.aids["mode"], "play", "build mode left again")
        # clicks not measured (G didn't start the cursor): failed, nothing saved
        (self.dens / den_check.CAMERA_FILE).unlink()

        class Deaf(FakeMouse):
            def aim_click(self):
                return ""
        rep = den_check.Report()
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertIsNone(den_check.hover_calibration(rep, FakeAids(self.dens), self.dens, self.shoot, self.d / "shots", Deaf(den, self.dens), settle=0, wait=0.2))
        self.assertIn("FAIL hover calibration: 0 of 6 clicks measured", rep.text())
        self.assertFalse((self.dens / den_check.CAMERA_FILE).exists())
        # off Windows: skipped
        rep = den_check.Report()
        den_check.hover_calibration(rep, den, self.dens, self.shoot, self.d / "shots", None)
        self.assertEqual(rep.rows[0][0], "SKIP")

if __name__ == "__main__":
    unittest.main(verbosity=1)
