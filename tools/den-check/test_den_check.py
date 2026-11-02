#!/usr/bin/env python3
"""Tests for den-check / den-test (no game, no Windows needed):

    python3 -I tools/den-check/test_den_check.py

* the MPQ reader opens the archive the launcher's writer makes (fixtures/patch-Z-sample.MPQ, kept equal to the
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
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import den_check  # noqa: E402
import mpqlite  # noqa: E402

FIXTURE = HERE / "fixtures" / "patch-Z-sample.MPQ"


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
