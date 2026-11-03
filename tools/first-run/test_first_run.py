#!/usr/bin/env python3
"""The first-run test's tool (tools/first-run/first_run.py) with every outside thing faked: the download, the website
(account/new, game/login, audit/presence, audit/close), the launcher's play-test, the screenshots and the Den test.

    python3 -I tools/first-run/test_first_run.py
"""
import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import first_run as fr  # noqa: E402

KEY = "ovk1.AbCdEf1234." + "k" * 43
TOKEN = "t" * 43
GAMEPASS = "GamePass123"


class Args:
    hold = 60
    no_den = False
    keep = False


class FakeProc:
    def __init__(self, io):
        self.io = io

    def poll(self):
        return 0 if self.io.done else None

    def wait(self, timeout=None):
        return self.io.exit_code

    def kill(self):
        pass


class FakeIO:
    """The world as the tool sees it. `play` writes play-test.json the way the launcher does, in steps."""

    def __init__(self, root, exe=b"MZ" + b"\0" * 1_100_000 + fr.CLI_MARK + fr.DEN_MARK, in_world_after=3, presence=200,
                 report=None, den_code=0, closes=200):
        self.root, self.exe, self.t = Path(root), exe, 1000.0
        self.calls, self.env, self.cmd = [], None, None
        self.in_world_after, self.presence, self.den_code, self.closes = in_world_after, presence, den_code, closes
        self.report = report
        self.done, self.exit_code, self.polls, self.closed = False, 0, 0, False
        self.data = None

    # the website
    def http(self, method, path, js=None, token=""):
        self.calls.append((method, path, bool(token)))
        if path == "account/new":
            assert js == {"audit": True}
            return 200, {"ok": True, "token": TOKEN, "key": KEY, "me": {"name": "Brightfeather", "tag": "1234"}}
        if path == "game/login":
            return 200, {"ok": True, "user": "ABC123", "pass": GAMEPASS}
        if path == "audit/presence":
            self.polls += 1
            if self.presence != 200:
                return self.presence, {"error": "x"}
            w = self.polls >= self.in_world_after
            return 200, {"account_online": w, "in_world": w, "character": "Brightfeather" if w else None, "map": 0 if w else None, "login_given": True}
        if path == "audit/close":
            self.closed = self.closes == 200
            return self.closes, {"ok": True} if self.closes == 200 else {"error": "too old"}
        if path == "me":
            return (401 if self.closed else 200), {}
        return 404, {}

    def fetch(self, url, dest):
        assert url == fr.EXE_URL
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(self.exe)

    def spawn(self, cmd, env, cwd):
        self.cmd, self.env = cmd, dict(env)
        self.data = Path(env["LEYLINE_APP_DIR"])
        assert not any(KEY in c for c in cmd), "the key never on the command line"
        return FakeProc(self)

    def screenshot(self, pid, out):
        out.write_bytes(b"png")
        return True, str(out)

    def den_test(self, exe, data, env):
        self.den_env = env
        return self.den_code, "PASS den-play\nPASS" if self.den_code == 0 else "FAIL the white island"

    def make_character(self, user, password, name):
        self.char = (user, name)
        return True, "made"

    def sleep(self, s):
        self.t += s
        # the launcher's report, step by step
        if self.data is None:
            return
        n = int(self.t - 1000)
        r = self.report or {"mark": "leyline-play-test-v1", "fresh": {"asked": True, "empty": True},
                            "added": {"build": "3.3.5a", "complete": True, "missing": []}, "key_signin": {"ok": True, "name": "Brightfeather"},
                            "signed_in": True, "website_login": {"ok": True, "account": "ABC123"}, "hybrid": {"ok": True}, "pid": 4242}
        r = dict(r)
        if n < 4:
            r["phase"] = "login"
            r.pop("pid", None)
        elif n < 30:
            r["phase"] = "running"
        else:
            r.update(phase="done", scrubbed=True, session_left=False)
            self.done = True
        (self.data / "play-test.json").write_text(json.dumps(r), encoding="utf-8")
        (self.data / "settings.json").write_text(json.dumps({"clients": [{"id": "c1", "build": "3.3.5a"}]}), encoding="utf-8")

    def now(self):
        return self.t


class FirstRun(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.root = self.tmp / "root"
        (self.root / "tools" / "first-run").mkdir(parents=True)
        self.owner = self.tmp / "owner"
        self.owner.mkdir()
        self.game = self.tmp / "Wrath"
        (self.game / "WTF").mkdir(parents=True)
        (self.game / "Data" / "enUS").mkdir(parents=True)
        (self.game / "WTF" / "Config.wtf").write_text('SET accountName "ME"\n')
        (self.game / "Data" / "enUS" / "realmlist.wtf").write_text("set realmlist mine\n")
        (self.owner / "settings.json").write_text(json.dumps({"clients": [{"id": "w", "build": "3.3.5a", "path": str(self.game)},
                                                                          {"id": "v", "build": "1.12.1", "path": "/x"}], "selected_client": "v"}))
        (self.owner / "social-session.json").write_text('{"token":"owner"}')
        self.temp_base = self.tmp / "temp"
        self.temp_base.mkdir()
        self.out = []

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def go(self, io, args=None):
        lines = fr.Lines(out=self.out.append)
        code = fr.run(args or Args(), io=io, lines=lines, temp_base=str(self.temp_base), app_owner=str(self.owner), root=str(self.root))
        log = (self.root / "tools" / "first-run" / "logs" / "first-run-latest.txt").read_text()
        return code, log, lines

    def test_all_the_way_into_the_world(self):
        io = FakeIO(self.root)
        # the "game" changes its files while it runs: they are put back after
        orig = io.sleep
        def sleep(s):
            orig(s)
            (self.game / "WTF" / "Config.wtf").write_text('SET accountName "ABC123"\n')
            (self.game / "Data" / "patch-Z.MPQ").write_bytes(b"MPQ")
        io.sleep = sleep
        code, log, _ = self.go(io)
        self.assertEqual(code, 0, log)
        self.assertTrue(log.rstrip().endswith("PASS"), log)
        for want in ("1. the download", "4. a test account: Brightfeather#1234", "5. a test character", "6. logged in with the Hero Key",
                     "7. in the world: Brightfeather on map 0", "8. Your Den test", "9. your own launcher untouched",
                     "10. the test account closed", "10. its sign-in no longer works", "10. the temp folders removed", "11. no secrets"):
            self.assertIn(want, log)
        # the fresh launcher: its own data and window folders, the key only in its environment, the flags
        self.assertEqual(io.env[fr.KEY_ENV], KEY)
        self.assertTrue(io.env["LEYLINE_APP_DIR"].startswith(str(self.temp_base)))
        self.assertIn("WEBVIEW2_USER_DATA_FOLDER", io.env)
        self.assertEqual(io.cmd[1:6], ["play-test", "--official", "--fresh", "--game", str(self.game)])
        self.assertIn("--world", io.cmd)
        self.assertEqual(io.den_env["LEYLINE_EXE"], str(self.root / ".launcher" / "first-run" / "Leyline.exe"))
        # put back, removed, closed
        self.assertEqual((self.game / "WTF" / "Config.wtf").read_text(), 'SET accountName "ME"\n')
        self.assertFalse((self.game / "Data" / "patch-Z.MPQ").exists())
        self.assertEqual(list(self.temp_base.iterdir()), [])
        self.assertTrue(io.closed)
        self.assertIn(("POST", "audit/close", True), io.calls)
        # screenshots, the report kept without secrets
        logs = self.root / "tools" / "first-run" / "logs"
        self.assertTrue(list(logs.glob("*.png")))
        self.assertTrue((logs / "play-test.json").is_file())

    def test_no_secret_anywhere(self):
        io = FakeIO(self.root, report={"fresh": {"asked": True, "empty": True}, "added": {"build": "3.3.5a", "complete": True},
                                       "key_signin": {"ok": True, "name": "x"}, "signed_in": True, "website_login": {"ok": True},
                                       "hybrid": {"ok": False, "why": "no glue " + GAMEPASS}, "keys": {"typed": True}, "pid": 1,
                                       "error": "boom " + TOKEN})
        code, log, _ = self.go(io)
        logs = self.root / "tools" / "first-run" / "logs"
        everything = log + "\n".join(self.out) + (logs / "play-test.json").read_text()
        for s in (KEY, TOKEN, GAMEPASS):
            self.assertNotIn(s, everything)
        self.assertIn("(hidden)", everything)
        self.assertEqual(code, 1)   # the launcher said error

    def test_old_launcher_stops_early_and_still_cleans_up(self):
        io = FakeIO(self.root, exe=b"MZ" + b"\0" * 1_100_000)
        code, log, _ = self.go(io)
        self.assertEqual(code, 1)
        self.assertIn("FAIL 1. the download has play-test", log)
        self.assertIn("Ship, wait for the leyline launcher build", log)
        self.assertNotIn(("POST", "account/new", False), io.calls, "no test account made for an old launcher")
        self.assertEqual(list(self.temp_base.iterdir()), [])
        self.assertIn("9. your own launcher untouched", log)

    def test_not_in_the_world_and_presence_missing(self):
        io = FakeIO(self.root, presence=404)
        code, log, _ = self.go(io)
        self.assertEqual(code, 1)
        self.assertIn("FAIL 7. in the world", log)
        self.assertIn("FAIL 7. the website has audit/presence", log)
        self.assertIn("Run the signin step", log)
        self.assertTrue(io.closed, "cleaned up on a failure too")

    def test_den_failure_and_skip(self):
        code, log, _ = self.go(FakeIO(self.root, den_code=1))
        self.assertEqual(code, 1)
        self.assertIn("FAIL 8. Your Den test", log)
        a = Args()
        a.no_den = True
        code, log, _ = self.go(FakeIO(self.root), a)
        self.assertEqual(code, 0, log)
        self.assertIn("INFO 8. Your Den test: skipped", log)

    def test_no_wrath_folder(self):
        (self.owner / "settings.json").write_text(json.dumps({"clients": []}))
        io = FakeIO(self.root)
        code, log, _ = self.go(io)
        self.assertEqual(code, 1)
        self.assertIn("FAIL 3. your Wrath game folder", log)
        self.assertFalse(any(c[1] == "account/new" for c in io.calls))

    def test_owner_launcher_changed_is_a_fail(self):
        io = FakeIO(self.root)
        orig = io.sleep
        def sleep(s):
            orig(s)
            (self.owner / "settings.json").write_text("{}")
        io.sleep = sleep
        code, log, _ = self.go(io)
        self.assertEqual(code, 1)
        self.assertIn("FAIL 9. your own launcher untouched", log)

    def test_helpers(self):
        self.assertEqual(fr.char_name("Brightfeather42"), "Brightfeather"[:12].capitalize())
        self.assertEqual(fr.char_name("é"), "Firstrun")
        self.assertEqual(fr.wrath_folder(self.owner)["path"], str(self.game))
        files = [str(p) for p in fr.game_files(self.game)]
        self.assertTrue(any(f.endswith("Config.wtf") for f in files) and any("enUS" in f and f.endswith("realmlist.wtf") for f in files))


class StepAndBat(unittest.TestCase):
    def test_bat_and_control_step(self):
        root = HERE.parent.parent
        bat = (HERE / "First run test.bat").read_bytes()
        self.assertIn(b"\r\n", bat)
        self.assertNotIn(b"\n", bat.replace(b"\r\n", b""), "CRLF only")
        self.assertIn(b"first_run.py", bat)
        self.assertIn(b"-I", bat)
        text = (root / "tools" / "control" / "control.py").read_text(encoding="utf-8")
        self.assertIn('"first-run-test": ("tools/first-run/First run test.bat"', text)


if __name__ == "__main__":
    unittest.main(verbosity=1)
