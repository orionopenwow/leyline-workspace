#!/usr/bin/env python3
"""The fresh computer test's tool (tools/first-run/first_run.py) with every outside thing faked: the download and its
checksum, this computer (WebView2, the Visual C++ runtime), the website (account/new, game/login, audit/presence,
audit/close, the Hero Key page and its cookies, /logout), the launcher's play-test with its feature checks, the
screenshots and the Den test. Also the Windows Sandbox files and the bats and Control steps.

    python3 -I tools/first-run/test_first_run.py
"""
import hashlib
import json
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
SESSION = "S" * 40
EXE = b"MZ" + b"\0" * 1_100_000 + fr.CLI_MARK + fr.CHECKS_MARK + fr.DEN_MARK
TORRENT_NO = "Leyline can't download game clients from torrents. Drop your own client folder or zip instead."


class Args:
    hold = 60
    no_den = False
    keep = False
    no_game = False
    den_features = False
    game = None
    work = None


def drops(with_game=True, ok=True):
    rows = [{"name": "a zip of your game folder", "expect": "zip", "got": "zip", "ok": True},
            {"name": "a link to your own backup", "expect": "link", "got": "link", "ok": True},
            {"name": "a magnet link", "expect": "refused", "got": TORRENT_NO if ok else "magnet (taken: the testing switch is on)", "ok": ok},
            {"name": "a .torrent file", "expect": "refused", "got": TORRENT_NO, "ok": True},
            {"name": "a link to a .torrent", "expect": "refused", "got": TORRENT_NO, "ok": True},
            {"name": "a browser shortcut to a magnet link", "expect": "refused", "got": TORRENT_NO, "ok": True},
            {"name": "a 7z archive", "expect": "not_a_zip", "got": "Leyline can open zip files only.", "ok": True}]
    if with_game:
        rows.insert(0, {"name": "your game folder", "expect": "folder", "got": "folder", "ok": True})
    return rows


def full_report():
    return {"mark": "leyline-play-test-v1", "fresh": {"asked": True, "empty": True},
            "added": {"build": "3.3.5a", "complete": True, "missing": []}, "key_signin": {"ok": True, "name": "Brightfeather"},
            "more_added": [{"ok": True, "build": "1.12.1", "name": "My Vanilla", "complete": True}],
            "signed_in": True, "website_login": {"ok": True, "account": "ABC123"}, "hybrid": {"ok": True}, "pid": 4242,
            "checks": {"mark": "leyline-play-test-checks-v1", "torrent_switch": False, "drops": drops(),
                       "signin": {"file": True, "file_why": "", "logout": True, "paste": True, "paste_why": ""},
                       "realm_switch": {"ok": True, "realm": "Friendly Realm", "there": True, "back": True, "why": ""},
                       "loadouts": {"ok": True, "mods_on": 3, "clean": {"ok": True}, "back": {"ok": True}, "put_back": True}}}


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
    """The world as the tool sees it. `sleep` writes play-test.json the way the launcher does, in steps."""

    def __init__(self, root, exe=EXE, in_world_after=3, presence=200, report=None, den_code=0, closes=200,
                 computer=None, sums=True, assets=200, web_ok=True, logout_ends=True, clients=2):
        self.root, self.exe, self.t = Path(root), exe, 1000.0
        self.calls, self.env, self.cmd, self.pages = [], None, None, []
        self.in_world_after, self.presence, self.den_code, self.closes = in_world_after, presence, den_code, closes
        self.report = report
        self.done, self.exit_code, self.polls, self.closed = False, 0, 0, False
        self.data = None
        self.pc = computer or {"os": "Windows-11", "windows": True, "sandbox": False, "webview2": "131.0.1", "vc_missing": []}
        self.sums, self.assets, self.web_ok, self.logout_ends, self.clients = sums, assets, web_ok, logout_ends, clients
        self.logged_out = False
        self.den_features = None

    # the website: JSON
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

    # the website: pages, as a browser
    def web(self, method, path, form=None, cookie=""):
        self.pages.append((method, path, dict(form or {}), cookie))
        signed = ("wr_session=" + SESSION) in cookie and not self.logged_out
        if method == "GET" and path == fr.KEY_PAGE:
            if signed:
                return 200, [], '<section class="hk-card"><h1>Your Hero Key</h1>'
            return 200, ["ly_hk=anon123anon123anon123anon; Path=/wrath/key; Max-Age=3600; HttpOnly; Secure; SameSite=Strict"], \
                '<section class="hk-card" id="hk-out"><form method="post" action="/wrath/key/signin" id="hk-login"><input type="hidden" name="t" value="form.Token_1">'
        if method == "POST" and path == fr.KEY_PAGE + "/signin":
            assert form == {"t": "form.Token_1", "key": KEY} and "ly_hk=" in cookie
            if not self.web_ok:
                return 401, [], "refused"
            return 303, ["wr_session=%s; Path=/; HttpOnly; Secure; SameSite=Lax" % SESSION], ""
        if method == "POST" and path == "/logout":
            self.logged_out = self.logout_ends
            return 303, ["wr_session=; Path=/; Max-Age=0"], ""
        return 404, [], ""

    def get_text(self, url, limit=1 << 16):
        if url == fr.SUMS_URL:
            if not self.sums:
                return 404, ""
            return 200, "%s  Leyline.exe\n%s  Leyline-portable.zip\n" % (hashlib.sha256(self.exe).hexdigest() if self.sums is True else "0" * 64, "1" * 64)
        if url in (fr.ADDONS_URL, fr.DEN_LITE_URL):
            return self.assets, "{}"
        return 404, ""

    def computer(self):
        return dict(self.pc)

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

    def den_test(self, exe, data, env, features=False):
        self.den_env, self.den_features = env, features
        return self.den_code, "PASS den-play\nPASS" if self.den_code == 0 else "FAIL the white island"

    def make_character(self, user, password, name):
        self.char = (user, name)
        return True, "made"

    def sleep(self, s):
        self.t += s
        if self.data is None:
            return
        n = int(self.t - 1000)
        r = dict(self.report if self.report is not None else full_report())
        if n < 4:
            r["phase"] = "login"
            r.pop("pid", None)
        elif n < 30:
            r["phase"] = "running"
        else:
            r.update(phase="done", scrubbed=True, session_left=False)
            self.done = True
        (self.data / "play-test.json").write_text(json.dumps(r), encoding="utf-8")
        cl = [{"id": "c%d" % i, "build": "3.3.5a"} for i in range(self.clients)]
        (self.data / "settings.json").write_text(json.dumps({"clients": cl}), encoding="utf-8")

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
        self.vanilla = self.tmp / "Vanilla"
        self.vanilla.mkdir()
        (self.game / "WTF").mkdir(parents=True)
        (self.game / "Data" / "enUS").mkdir(parents=True)
        (self.game / "WTF" / "Config.wtf").write_text('SET accountName "ME"\n')
        (self.game / "Data" / "enUS" / "realmlist.wtf").write_text("set realmlist mine\n")
        (self.owner / "settings.json").write_text(json.dumps({"clients": [{"id": "w", "build": "3.3.5a", "path": str(self.game)},
                                                                          {"id": "v", "build": "1.12.1", "path": str(self.vanilla)},
                                                                          {"id": "gone", "build": "2.4.3", "path": "/not/there"}],
                                                              "selected_client": "v"}))
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

    def test_every_feature_all_the_way_into_the_world(self):
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
        for want in ("PASS 1. the download", "PASS 1. the download matches its checksum", "PASS 2. WebView2", "PASS 3. the Visual C++ runtime",
                     "PASS 4. the Leyline addons pack", "PASS 4. Den Lite", "PASS 5. fresh start", "PASS 6. your Wrath game folder",
                     "PASS 7. your game folder added", "PASS 7. the new launcher has exactly your game folders",
                     "PASS 8. your other game folders added too: My Vanilla (1.12.1)",
                     "PASS 9. drop your game folder", "PASS 9. drop a magnet link", "PASS 9. drop a .torrent file", "PASS 9. drop a 7z archive",
                     "PASS 10. a test account: Brightfeather#1234", "PASS 11. launcher: log in with the Hero Key file",
                     "PASS 12. launcher: log out", "PASS 13. launcher: log in with the key pasted",
                     "PASS 14. website: the Log in page", "PASS 14. website: log in with the Hero Key",
                     "PASS 14. website: the sign-in cookie is HttpOnly, Secure, SameSite", "PASS 14. website: signed in on the next page",
                     "PASS 14. website: log out ends the session", "PASS 15. a test character", "PASS 16. PLAY started the game",
                     "PASS 17. the game logged in by itself", "PASS 18. character select", "PASS 19. in the world: Brightfeather on map 0",
                     "PASS 20. screenshots", "PASS 21. the one-time login", "PASS 22. a community realm and back: Friendly Realm",
                     "PASS 23. Your Den", "PASS 25. loadouts: Clean and back", "PASS 25. loadouts: your mods and loadouts as they were",
                     "PASS 26. the test account closed", "PASS 26. its sign-in no longer works", "PASS 27. your game folder put back",
                     "PASS 28. the temp folders removed", "PASS 29. your own launcher untouched", "PASS 30. no secrets"):
            self.assertIn(want, log)
        # the fresh launcher: its own data and window folders, the key only in its environment, the flags
        self.assertEqual(io.env[fr.KEY_ENV], KEY)
        self.assertTrue(io.env["LEYLINE_APP_DIR"].startswith(str(self.temp_base)))
        self.assertIn("WEBVIEW2_USER_DATA_FOLDER", io.env)
        self.assertEqual(io.cmd[1:6], ["play-test", "--official", "--fresh", "--game", str(self.game)])
        self.assertEqual(io.cmd[6:8], ["--game", str(self.vanilla)], "the other version too; a folder that's gone is left out")
        self.assertNotIn("/not/there", io.cmd)
        self.assertIn("--world", io.cmd)
        self.assertIn("--checks", io.cmd)
        self.assertEqual(io.den_env["LEYLINE_EXE"], str(self.root / ".launcher" / "first-run" / "Leyline.exe"))
        self.assertFalse(io.den_features)
        # put back, removed, closed; the website session logged out
        self.assertEqual((self.game / "WTF" / "Config.wtf").read_text(), 'SET accountName "ME"\n')
        self.assertFalse((self.game / "Data" / "patch-Z.MPQ").exists())
        self.assertEqual(list(self.temp_base.iterdir()), [])
        self.assertTrue(io.closed)
        self.assertIn(("POST", "audit/close", True), io.calls)
        self.assertTrue(io.logged_out)
        logs = self.root / "tools" / "first-run" / "logs"
        self.assertTrue(list(logs.glob("*.png")))
        self.assertTrue((logs / "play-test.json").is_file())

    def test_no_secret_anywhere(self):
        r = full_report()
        r.update(hybrid={"ok": False, "why": "no glue " + GAMEPASS}, keys={"typed": True}, error="boom " + TOKEN)
        r["checks"]["signin"]["paste_why"] = "said " + KEY
        io = FakeIO(self.root, report=r)
        code, log, _ = self.go(io)
        logs = self.root / "tools" / "first-run" / "logs"
        everything = log + "\n".join(self.out) + (logs / "play-test.json").read_text()
        for s in (KEY, TOKEN, GAMEPASS, SESSION):
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
        self.assertIn("29. your own launcher untouched", log)

    def test_launcher_without_checks_still_plays_but_fails_the_line(self):
        r = full_report()
        del r["checks"]
        io = FakeIO(self.root, exe=b"MZ" + b"\0" * 1_100_000 + fr.CLI_MARK + fr.DEN_MARK, report=r)
        code, log, _ = self.go(io)
        self.assertEqual(code, 1)
        self.assertIn("FAIL 1. the download has the feature checks", log)
        self.assertNotIn("--checks", io.cmd)
        self.assertIn("PASS 13. launcher: log in with the Hero Key", log)
        self.assertIn("PASS 19. in the world", log)

    def test_checksum_and_assets_and_webview(self):
        pc = {"os": "Windows-10", "windows": True, "sandbox": True, "webview2": "", "vc_missing": []}
        code, log, _ = self.go(FakeIO(self.root, sums="wrong", assets=404, computer=pc))
        self.assertEqual(code, 1)
        self.assertIn("FAIL 1. the download matches its checksum", log)
        self.assertIn("FAIL 2. WebView2", log)
        self.assertIn("Windows Sandbox", log)
        self.assertIn("FAIL 4. the Leyline addons pack can be downloaded: addonpack.json HTTP 404", log)
        self.assertIn("FAIL 4. Den Lite", log)
        code, log, _ = self.go(FakeIO(self.root, sums=False))
        self.assertIn("INFO 1. the download matches its checksum: no SHA256SUMS.txt", log)
        self.assertEqual(code, 0, log)

    def test_vc_runtime_missing_but_launcher_ran_passes(self):
        pc = {"os": "Windows-11", "windows": True, "sandbox": True, "webview2": "131", "vc_missing": ["vcruntime140_1.dll"]}
        code, log, _ = self.go(FakeIO(self.root, computer=pc))
        self.assertEqual(code, 0, log)
        self.assertIn("PASS 3. the Visual C++ runtime: not on this PC (vcruntime140_1.dll); the launcher ran without it", log)

    def test_vc_runtime_missing_and_launcher_silent_fails(self):
        pc = {"os": "Windows-11", "windows": True, "sandbox": True, "webview2": "131", "vc_missing": ["vcruntime140.dll"]}
        io = FakeIO(self.root, computer=pc, report={})
        code, log, _ = self.go(io)
        self.assertEqual(code, 1)
        self.assertIn("FAIL 3. the Visual C++ runtime: not on this PC (vcruntime140.dll), and the launcher didn't start", log)

    def test_torrent_drop_taken_is_a_fail(self):
        r = full_report()
        r["checks"]["drops"] = drops(ok=False)
        r["checks"]["torrent_switch"] = True
        code, log, _ = self.go(FakeIO(self.root, report=r))
        self.assertEqual(code, 1)
        self.assertIn("FAIL 9. drag and drop: Settings › Testing is off in a fresh launcher", log)
        self.assertIn("FAIL 9. drop a magnet link: expect refused, got: magnet (taken", log)

    def test_website_cookie_and_logout(self):
        code, log, _ = self.go(FakeIO(self.root, logout_ends=False))
        self.assertEqual(code, 1)
        self.assertIn("FAIL 14. website: log out ends the session", log)
        io = FakeIO(self.root, web_ok=False)
        code, log, _ = self.go(io)
        self.assertIn("FAIL 14. website: log in with the Hero Key", log)
        self.assertNotIn("14. website: signed in on the next page", log)
        self.assertTrue(io.closed, "the test account still closed")

    def test_cookie_flags_are_read_strictly(self):
        self.assertEqual(fr.cookie_flags(["wr_session=x; Path=/; HttpOnly; Secure; SameSite=Lax"], "wr_session"),
                         {"httponly": True, "secure": True, "samesite": "lax"})
        self.assertEqual(fr.cookie_flags(["wr_session=; Path=/; Max-Age=0"], "wr_session"), {}, "a clearing line isn't the cookie")
        self.assertEqual(fr.cookie_flags(["wr_session=x; Path=/"], "wr_session"), {"httponly": False, "secure": False, "samesite": ""})
        jar = fr.merge_cookies({"a": "1"}, ["b=2; Path=/", "a=; Max-Age=0"])
        self.assertEqual(jar, {"b": "2"})
        self.assertEqual(fr.cookie_header({"a": "1", "b": "2"}), "a=1; b=2")

    def test_realm_switch_and_loadouts_lines(self):
        r = full_report()
        r["checks"]["realm_switch"] = {"ok": None, "why": "no community realm for this game folder on the list right now"}
        r["checks"]["loadouts"] = {"ok": False, "mods_on": 2, "put_back": False}
        code, log, _ = self.go(FakeIO(self.root, report=r))
        self.assertEqual(code, 1)
        self.assertIn("INFO 22. a community realm and back: no community realm", log)
        self.assertIn("FAIL 25. loadouts: Clean and back", log)
        self.assertIn("FAIL 25. loadouts: your mods and loadouts as they were", log)

    def test_no_game(self):
        r = {"mark": "leyline-play-test-v1", "fresh": {"asked": True, "empty": True}, "key_signin": {"ok": True, "name": "B"},
             "signed_in": True, "error": "no 3.3.5a game folder in the launcher (or not the one --game names)",
             "checks": {"torrent_switch": False, "drops": drops(with_game=False),
                        "signin": {"file": True, "logout": True, "paste": True}}}
        a = Args()
        a.no_game = True
        io = FakeIO(self.root, report=r)
        code, log, _ = self.go(io, a)
        self.assertEqual(code, 0, log)
        self.assertEqual(io.cmd[1:], ["play-test", "--official", "--fresh", "--checks"])
        self.assertIn("INFO 6. your Wrath game folder: --no-game", log)
        self.assertIn("INFO 9. drop your game folder: --no-game", log)
        self.assertIn("PASS 13. launcher: log in with the key pasted", log)
        self.assertNotIn("game/login", [c[1] for c in io.calls])
        self.assertNotIn("23. Your Den", log)
        self.assertNotIn("27. your game folder", log)
        self.assertTrue(io.closed)

    def test_game_given_and_den_features(self):
        a = Args()
        a.game = str(self.game)
        a.den_features = True
        io = FakeIO(self.root, clients=1)
        code, log, _ = self.go(io, a)
        self.assertEqual(code, 0, log)
        self.assertNotIn(str(self.vanilla), io.cmd, "--game: only that folder")
        self.assertTrue(io.den_features)
        self.assertIn("PASS 24. Your Den: instant sign-in, the editor's features", log)

    def test_not_in_the_world_and_presence_missing(self):
        io = FakeIO(self.root, presence=404)
        code, log, _ = self.go(io)
        self.assertEqual(code, 1)
        self.assertIn("FAIL 19. in the world", log)
        self.assertIn("FAIL 18. the website has audit/presence", log)
        self.assertIn("Run the signin step", log)
        self.assertTrue(io.closed, "cleaned up on a failure too")

    def test_den_failure_and_skip(self):
        code, log, _ = self.go(FakeIO(self.root, den_code=1))
        self.assertEqual(code, 1)
        self.assertIn("FAIL 23. Your Den", log)
        a = Args()
        a.no_den = True
        code, log, _ = self.go(FakeIO(self.root), a)
        self.assertEqual(code, 0, log)
        self.assertIn("INFO 23. Your Den test: skipped", log)

    def test_no_wrath_folder(self):
        (self.owner / "settings.json").write_text(json.dumps({"clients": []}))
        io = FakeIO(self.root)
        code, log, _ = self.go(io)
        self.assertEqual(code, 1)
        self.assertIn("FAIL 6. your Wrath game folder", log)
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
        self.assertIn("FAIL 29. your own launcher untouched", log)

    def test_a_sandbox_result_survives_the_next_run(self):
        keep = self.root / "tools" / "first-run" / "logs" / "sandbox" / "out" / "console.txt"
        keep.parent.mkdir(parents=True)
        keep.write_text("x")
        old = self.root / "tools" / "first-run" / "logs" / "00-old.png"
        old.write_bytes(b"x")
        self.go(FakeIO(self.root))
        self.assertTrue(keep.is_file())
        self.assertFalse(old.exists())

    def test_helpers(self):
        self.assertEqual(fr.char_name("Brightfeather42"), "Brightfeather"[:12].capitalize())
        self.assertEqual(fr.char_name("é"), "Firstrun")
        self.assertEqual(fr.wrath_folder(self.owner)["path"], str(self.game))
        self.assertEqual(fr.other_folders(self.owner, str(self.game)), [str(self.vanilla)])
        self.assertIsNone(fr.wrath_folder(self.tmp / "nobody"))
        files = [str(p) for p in fr.game_files(self.game)]
        self.assertTrue(any(f.endswith("Config.wtf") for f in files) and any("enUS" in f and f.endswith("realmlist.wtf") for f in files))
        h = "a" * 64
        self.assertEqual(fr.sums_for("%s  Leyline.exe\n%s *Other.zip\n" % (h, "b" * 64), "Leyline.exe"), h)
        self.assertEqual(fr.sums_for("%s *Leyline.exe" % h.upper(), "Leyline.exe"), h)
        self.assertIsNone(fr.sums_for("nonsense", "Leyline.exe"))


class Sandbox(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.root = self.tmp / "root"
        (self.root / "tools" / "first-run").mkdir(parents=True)
        self.py = self.tmp / "Python312"
        self.py.mkdir()
        (self.py / "python.exe").write_bytes(b"MZ")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def args(self, game=None, no_game=False, open_=False):
        a = Args()
        a.game, a.no_game, a.open = game, no_game, open_
        return a

    def test_wsb_and_run_cmd(self):
        opened = []
        code = fr.sandbox(self.args(game=r"D:\Games\Wrath & Co", open_=True), root=self.root, opener=opened.append,
                          python_home=self.py, windows=True)
        self.assertEqual(code, 0)
        where = self.root / "tools" / "first-run" / "logs" / "sandbox"
        wsb = (where / fr.WSB_NAME).read_text()
        self.assertEqual(opened, [str(where / fr.WSB_NAME)])
        self.assertIn("<HostFolder>D:\\Games\\Wrath &amp; Co</HostFolder>", wsb)
        self.assertIn("<Networking>Enable</Networking>", wsb)
        self.assertIn("<ClipboardRedirection>Disable</ClipboardRedirection>", wsb)
        # the workspace, Python and the game read-only; only the out folder writable
        blocks = wsb.split("<MappedFolder>")[1:]
        self.assertEqual(len(blocks), 4)
        for b in blocks:
            self.assertIn("<ReadOnly>%s</ReadOnly>" % ("false" if "<SandboxFolder>C:\\out<" in b else "true"), b)
        run = (where / "out" / "run.cmd").read_bytes()
        self.assertNotIn(b"\n", run.replace(b"\r\n", b""), "CRLF only")
        text = run.decode()
        self.assertIn("--game C:\\fresh\\Wrath", text)
        self.assertIn("C:\\py\\python.exe -I", text)
        for rel in fr.SB_COPY:
            self.assertIn(rel, text)
        self.assertNotIn("C:\\leyline\\tools\" ", text, "never the whole tools folder (it holds saved tokens)")
        self.assertNotIn("discord", text.lower())

    def test_no_game_and_no_sandbox(self):
        code = fr.sandbox(self.args(no_game=True), root=self.root, python_home=self.py, windows=True)
        self.assertEqual(code, 0)
        text = (self.root / "tools" / "first-run" / "logs" / "sandbox" / "out" / "run.cmd").read_text()
        self.assertIn("run --no-game", text)
        self.assertNotIn("game-ro", text)
        (self.py / "bin").mkdir()
        out = []
        sys.stdout, real = type("W", (), {"write": lambda _s, t: out.append(t), "flush": lambda _s: None})(), sys.stdout
        try:
            self.assertEqual(fr.sandbox(self.args(no_game=True, open_=True), root=self.root, python_home=self.py, windows=False), 1)
        finally:
            sys.stdout = real
        self.assertIn("Windows Sandbox isn't on here", "".join(out))
        store = self.tmp / "WindowsApps" / "Python"
        store.mkdir(parents=True)
        (store / "python.exe").write_bytes(b"MZ")
        self.assertEqual(fr.sandbox(self.args(no_game=True), root=self.root, python_home=store, windows=True), 1,
                         "the Store's Python can't be mapped into a sandbox")


class StepAndBat(unittest.TestCase):
    def bat(self, name):
        b = (HERE / name).read_bytes()
        self.assertIn(b"\r\n", b)
        self.assertNotIn(b"\n", b.replace(b"\r\n", b""), "CRLF only")
        self.assertIn(b"first_run.py", b)
        self.assertIn(b"-I", b)
        return b.decode()

    def test_bats_and_control_steps(self):
        root = HERE.parent.parent
        self.bat("First run test.bat")
        fresh = self.bat("Fresh computer test.bat")
        self.assertIn("%*", fresh)
        sb = self.bat("Fresh computer test - sandbox.bat")
        self.assertIn("first_run.py\" sandbox --open", sb)
        text = (root / "tools" / "control" / "control.py").read_text(encoding="utf-8")
        self.assertIn('"first-run-test": ("tools/first-run/First run test.bat"', text)
        self.assertIn('"fresh-test": ("tools/first-run/Fresh computer test.bat"', text)
        self.assertIn('"fresh-sandbox": ("tools/first-run/Fresh computer test - sandbox.bat"', text)


if __name__ == "__main__":
    unittest.main(verbosity=1)
