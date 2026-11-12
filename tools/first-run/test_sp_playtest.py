#!/usr/bin/env python3
"""The single player playtest's tool (tools/first-run/sp_playtest.py) with every outside thing faked: the launcher-test
download and its .sha256, the website's test account, the ports and the disk, the launcher's `play-test --single`
(its report written phase by phase, as app.rs single_play_test_cli does), the screenshots and the clean-up. Also the
Control step and its bat.

    python3 -I tools/first-run/test_sp_playtest.py
"""
import ast
import hashlib
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import sp_playtest as sp  # noqa: E402
from first_run import Lines  # noqa: E402

KEY = "ovk1.AbCdEf1234." + "k" * 43
TOKEN = "t" * 43
EXE = b"MZ" + b"\0" * 2000 + sp.SP_MARK


class Args:
    kind = "single"
    both = False
    game = None
    hold = 20
    level = 10
    checks = sp.CHECKS
    no_key = False
    keep = False
    work = None
    timeout = 230


def steps(ok=True):
    names = ["Hero Key", "game folder", "preflight", "setup", "play", "signed in by itself", "in the world", "companions",
             "levelup", "stoponexit", "repair", "clean up"]
    return [{"name": n, "ok": (False if (not ok and n == "companions") else True), "ms": 1000, "why": "seen"} for n in names]


def phases(ok=True, kind="single"):
    """The report as the launcher writes it, one state per poll."""
    base = {"mark": sp.SP_MARK.decode(), "kind": kind, "pid": 0, "steps": []}
    out = []
    for ph in ("start", "setup", "entering", "in_world", "levelled", "closing", "entering_again", "repaired_in_world", "done"):
        r = dict(base, phase=ph)
        if ph not in ("start", "setup"):
            r["pid"] = 4242
        if ph == "done":
            r["steps"] = steps(ok)
            r["passed"] = ok
        out.append(r)
    return out


class Proc:
    def __init__(self, io, data, code):
        self.io, self.data, self.code, self.i = io, data, code, 0

    def poll(self):
        script = self.io.script
        if self.i < len(script):
            (self.data / sp.REPORT).write_text(json.dumps(script[self.i]), encoding="utf-8")
            self.i += 1
            return None
        return self.code

    def wait(self, timeout=None):
        return self.code

    def kill(self):
        self.io.killed = True


class Fake:
    windows = True

    def __init__(self, t: Path, exe=EXE, sha=None, busy=(), free=100.0, ok=True, code=0, account=200, mutate=True, script=None):
        self.t, self.exe, self.busy, self.free, self.code, self.account = t, exe, set(busy), free, code, account
        self.sha = hashlib.sha256(exe).hexdigest() if sha is None else sha
        self.script = phases(ok) if script is None else script
        self.ok, self.mutate = ok, mutate
        self.spawned, self.envs, self.shots, self.http_calls, self.killed_under, self.closed = [], [], [], [], [], []
        self.killed = False
        self.clock = 0.0

    def port_busy(self, port):
        return port in self.busy

    def free_gb(self, path):
        return self.free

    def fetch(self, url, dest):
        assert url == sp.EXE_URL
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(self.exe)

    def get_text(self, url, limit=1 << 16):
        if url == sp.SHA_URL:
            return 200, "%s  Leyline.exe\n" % self.sha
        if url == sp.INFO_URL:
            return 200, json.dumps({"commit": "abc1234def", "branch": "feat-sp-playtest"})
        return 404, ""

    def http(self, method, path, js=None, token=""):
        self.http_calls.append((method, path, token))
        if path == "account/new":
            return (self.account, {"token": TOKEN, "key": KEY}) if self.account == 200 else (self.account, {})
        if path == "audit/close":
            return 200, {}
        return 404, {}

    def spawn_logged(self, cmd, env, cwd, out):
        self.spawned.append(list(cmd))
        self.envs.append(dict(env))
        data = Path(env["LEYLINE_APP_DIR"])
        out.write_text("12:00:00 setup: step 1/5 Getting the engine\n12:00:01 the key " + env.get(sp.KEY_ENV, "") + "\n", encoding="utf-8")
        if self.mutate:   # what a Play does to the game folder
            game = Path(cmd[cmd.index("--game") + 1])
            (game / "WTF" / "Config.wtf").write_text("SET realmlist \"127.0.0.1\"\n", encoding="utf-8")
            (game / "Data" / "patch-Z.MPQ").write_bytes(b"MPQ session")
            (game / "WTF" / "Account" / "HK2ABCDEFGHIJKLM").mkdir(parents=True, exist_ok=True)
            # the engine's logs, one with the key in it (it must come out hidden)
            logs = data / "engine" / "logs"
            logs.mkdir(parents=True, exist_ok=True)
            (logs / "setup.log").write_text("done " + env.get(sp.KEY_ENV, "") + "\n", encoding="utf-8")
        (data / sp.CLI_LOG).write_text("lines\n", encoding="utf-8")
        return Proc(self, data, self.code)

    def screenshot(self, pid, out):
        self.shots.append(out.name)
        out.write_bytes(b"\x89PNG")
        return True, str(out)

    def kill_under(self, folder):
        self.killed_under.append(folder)
        return ["9001"]

    def close_game(self, pid, game):
        self.closed.append(pid)
        return False

    def sleep(self, s):
        self.clock += s

    def now(self):
        return self.clock


class Base(unittest.TestCase):
    def setUp(self):
        self.t = Path(tempfile.mkdtemp())
        self.root = self.t / "root"
        (self.root / "tools" / "first-run").mkdir(parents=True)
        self.game = self.t / "Games" / "Wrath"
        (self.game / "Data" / "enUS").mkdir(parents=True)
        (self.game / "WTF" / "Account" / "MYOWN").mkdir(parents=True)
        (self.game / "WTF" / "Config.wtf").write_text("SET realmlist \"leylinelauncher.com\"\n", encoding="utf-8")
        (self.game / "Data" / "patch-T.MPQ").write_bytes(b"den ground")
        self.owner = self.t / "AppData" / "Leyline"
        self.owner.mkdir(parents=True)
        (self.owner / "settings.json").write_text(json.dumps({"clients": [{"id": "w", "build": "3.3.5a", "path": str(self.game)}],
                                                              "selected_client": "w"}), encoding="utf-8")
        self.work = self.t / "work"
        self.work.mkdir()
        self.out_lines = []

    def tearDown(self):
        shutil.rmtree(self.t, ignore_errors=True)

    def go(self, io, **kw):
        a = Args()
        a.work = str(self.work)
        for k, v in kw.items():
            setattr(a, k, v)
        ln = Lines(out=self.out_lines.append)
        code = sp.run(a, io=io, lines=ln, owner=self.owner, owners=[self.owner], root=self.root, stamp="20261111-120000")
        self.ln = ln
        self.text = "\n".join(self.out_lines)
        self.run_dir = self.root / "logs" / "sp-playtest" / "20261111-120000"
        return code


class TestRun(Base):
    def test_a_whole_run_passes_and_puts_everything_back(self):
        io = Fake(self.t)
        before = (self.game / "WTF" / "Config.wtf").read_bytes()
        self.assertEqual(self.go(io), 0, self.text)
        # the launcher's command line: fresh, the game folder, single, every check; never the key
        cmd = io.spawned[0]
        self.assertEqual(cmd[1:3], ["play-test", "--fresh"])
        self.assertEqual(cmd[cmd.index("--single") + 1], "single")
        self.assertEqual(cmd[cmd.index("--game") + 1], str(self.game))
        self.assertEqual(cmd[cmd.index("--checks") + 1], sp.CHECKS)
        self.assertEqual(cmd[cmd.index("--level") + 1], "10")
        self.assertNotIn(KEY, " ".join(cmd))
        env = io.envs[0]
        self.assertEqual(env[sp.KEY_ENV], KEY, "the key only through the environment")
        self.assertTrue(env["LEYLINE_APP_DIR"].startswith(str(self.work / "leyline-sp-playtest-")))
        # the screenshots at the phases
        self.assertEqual([s.split("-", 2)[2] for s in io.shots[:5]], ["entering.png", "in-world.png", "levelled.png", "entering-again.png", "after-repair.png"])
        # every step of the report is a line
        for n in ("setup", "in the world", "companions", "levelup", "stoponexit", "repair"):
            self.assertIn("PASS single: %s" % n, self.text)
        self.assertIn("PASS single: exit code", self.text)
        # put back: Config.wtf and patch-Z as they were, the login folder the test made gone, the owner's own kept
        self.assertEqual((self.game / "WTF" / "Config.wtf").read_bytes(), before)
        self.assertFalse((self.game / "Data" / "patch-Z.MPQ").exists())
        self.assertEqual((self.game / "Data" / "patch-T.MPQ").read_bytes(), b"den ground")
        self.assertFalse((self.game / "WTF" / "Account" / "HK2ABCDEFGHIJKLM").exists())
        self.assertTrue((self.game / "WTF" / "Account" / "MYOWN").is_dir())
        # the test's world stopped, the account closed, the temp folder gone
        self.assertEqual(len(io.killed_under), 1)
        self.assertIn(("POST", "audit/close", TOKEN), io.http_calls)
        self.assertEqual(list(self.work.iterdir()), [])
        # the results folder: the table, the report, the lines, the world's logs, the pictures; no secret anywhere
        names = {f.name for f in self.run_dir.iterdir()}
        for n in ("sp-playtest.txt", "single-sp-play-test.json", "single-lines.txt", "world-engine-setup.log", "sp-play-test.log"):
            self.assertIn(n, names)
        self.assertTrue((self.root / "logs" / "sp-playtest" / "latest.txt").is_file())
        for f in self.run_dir.iterdir():
            if f.suffix != ".png":
                body = f.read_text(encoding="utf-8")
                self.assertNotIn(KEY, body, f.name)
                self.assertNotIn(TOKEN, body, f.name)
        self.assertNotIn(KEY, self.text)
        self.assertIn("PASS no secrets in the log", self.text)
        self.assertIn("PASS your own launcher untouched", self.text)

    def test_a_failed_check_fails_the_run(self):
        io = Fake(self.t, ok=False, code=1)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL single: companions", self.text)
        self.assertIn("FAIL single: exit code", self.text)
        self.assertEqual(list(self.work.iterdir()), [], "cleaned up after a failure too")

    def test_both_worlds(self):
        io = Fake(self.t)
        self.assertEqual(self.go(io, both=True), 0, self.text)
        self.assertEqual(len(io.spawned), 2)
        self.assertIn("--fresh", io.spawned[0])
        self.assertNotIn("--fresh", io.spawned[1], "the second world on the same data folder (shared engine and maps)")
        self.assertEqual(io.spawned[1][io.spawned[1].index("--single") + 1], "vanilla")
        self.assertEqual(io.envs[0]["LEYLINE_APP_DIR"], io.envs[1]["LEYLINE_APP_DIR"])

    def test_without_a_key(self):
        io = Fake(self.t)
        self.assertEqual(self.go(io, no_key=True), 0, self.text)
        self.assertNotIn(sp.KEY_ENV, io.envs[0])
        self.assertFalse(any(p == "account/new" for _m, p, _t in io.http_calls))
        io2 = Fake(self.t, account=429)
        self.out_lines.clear()
        self.assertEqual(self.go(io2), 0, self.text)
        self.assertNotIn(sp.KEY_ENV, io2.envs[0])
        self.assertIn("as offline", self.text)

    def test_no_report_is_a_fail(self):
        io = Fake(self.t, script=[{"phase": "start"}], code=3)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL single: the launcher's report", self.text)

    def test_a_run_past_its_time_is_stopped(self):
        io = Fake(self.t, script=[dict(phases()[1])] * 10000)
        self.assertEqual(self.go(io, timeout=10), 1)
        self.assertTrue(io.killed)
        self.assertIn("FAIL single: finished in time", self.text)


class TestRefusals(Base):
    def test_not_windows(self):
        io = Fake(self.t)
        io.windows = False
        self.assertEqual(self.go(io), 1)
        self.assertEqual(io.spawned, [])

    def test_your_own_world_running(self):
        io = Fake(self.t, busy=[3724, 13306])
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL the local world's ports are free: in use: login 3724, database 13306", self.text)
        self.assertEqual(io.spawned, [])
        self.assertEqual(list(self.work.iterdir()), [], "nothing made")

    def test_too_little_room(self):
        io = Fake(self.t, free=5.0)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL room for a fresh world", self.text)
        self.assertEqual(io.spawned, [])

    def test_the_work_folder_inside_your_own(self):
        io = Fake(self.t)
        self.assertEqual(self.go(io, work=str(self.owner)), 1)
        self.assertIn("FAIL a fresh test folder", self.text)
        self.assertEqual(io.spawned, [])
        self.assertEqual(sorted(p.name for p in self.owner.iterdir()), ["settings.json"])

    def test_a_download_that_doesnt_match(self):
        io = Fake(self.t, sha="0" * 64)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL the test launcher matches its .sha256", self.text)
        self.assertEqual(io.spawned, [])
        self.assertEqual(list(self.work.iterdir()), [])

    def test_an_older_build(self):
        io = Fake(self.t, exe=b"MZ" + b"\0" * 100)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL the test launcher has the single player playtest", self.text)
        self.assertEqual(io.spawned, [])

    def test_no_game_folder(self):
        (self.owner / "settings.json").write_text("{}", encoding="utf-8")
        io = Fake(self.t)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL your Wrath game folder", self.text)


class TestPieces(Base):
    def test_game_folder_order(self):
        g, src = sp.game_folder(None, self.owner, self.root)
        self.assertEqual((g, src), (self.game, "your launcher's game folders"))
        (self.owner / "settings.json").write_text(json.dumps({"clients": [{"build": "1.12.1", "path": "C:/V"}]}), encoding="utf-8")
        (self.root / "tools" / "first-run" / "folder.txt").write_text('"D:\\Games\\Wrath"\n', encoding="utf-8")
        self.assertEqual(sp.game_folder(None, self.owner, self.root), (Path("D:\\Games\\Wrath"), "tools\\first-run\\folder.txt"))
        self.assertEqual(sp.game_folder("E:/W", self.owner, self.root)[0], Path("E:/W"))

    def test_touched_files_and_put_back(self):
        files = sp.touched_files(self.game)
        names = {p.name.lower() for p in files}
        for n in ("config.wtf", "patch-z.mpq", "patch-t.mpq", "patch-y.mpq", "realmlist.wtf", "patch-enus-z.mpq", "patch-enus-t.mpq"):
            self.assertIn(n, names)
        saved = sp.keep_aside(files, self.t / "aside")
        (self.game / "Data" / "patch-T.MPQ").write_bytes(b"changed")
        (self.game / "Data" / "patch-Y.MPQ").write_bytes(b"new")
        self.assertEqual(sp.put_back(saved), [])
        self.assertEqual((self.game / "Data" / "patch-T.MPQ").read_bytes(), b"den ground")
        self.assertFalse((self.game / "Data" / "patch-Y.MPQ").exists())

    def test_sha_line(self):
        self.assertEqual(sp.sha256_line("A" * 64 + "  Leyline.exe"), "a" * 64)
        self.assertIsNone(sp.sha256_line("nope"))
        self.assertIsNone(sp.sha256_line(""))

    def test_build_cmd_clamps(self):
        a = Args()
        a.hold, a.level = 99999, 1
        cmd = sp.build_cmd(Path("L.exe"), Path("G"), "vanilla", False, a)
        self.assertEqual(cmd[cmd.index("--hold") + 1], "600")
        self.assertEqual(cmd[cmd.index("--level") + 1], "2")
        self.assertNotIn("--fresh", cmd)

    def test_main_refuses_odd_checks(self):
        with self.assertRaises(SystemExit):
            sp.main(["run", "--checks", "levelup;rm"])

    def test_the_marks_match_the_launcher(self):
        src = (ROOT / "launcher" / "src-tauri" / "src" / "sptest.rs").read_text(encoding="utf-8")
        self.assertIn('pub const SP_MARK: &str = "%s";' % sp.SP_MARK.decode(), src)
        self.assertIn('pub const REPORT: &str = "%s";' % sp.REPORT, src)
        self.assertIn('pub const LOG: &str = "%s";' % sp.CLI_LOG, src)
        wf = (ROOT / ".github" / "workflows" / "launcher-check.yml").read_text(encoding="utf-8")
        self.assertIn(sp.SP_MARK.decode(), wf, "the test build checks the mark is in it")
        self.assertIn("launcher-test", wf)
        self.assertIn("Leyline.exe.sha256", wf)
        self.assertNotIn("refs/heads/main'", wf.split("launcher-test-windows:")[1].split("steps:")[0])
        for word in ("secrets.", "PAT", "TOKEN }}"):
            self.assertNotIn(word, wf.split("launcher-test-windows:")[1])


class TestControl(unittest.TestCase):
    def test_control_step(self):
        src = (ROOT / "tools" / "control" / "control.py").read_text(encoding="utf-8")
        tree = ast.parse(src)
        val = {t.id: n.value for n in tree.body if isinstance(n, ast.Assign) for t in n.targets if hasattr(t, "id")}
        steps = ast.literal_eval(val["STEPS"])
        ro = ast.literal_eval(val["READ_ONLY"])
        to = ast.literal_eval(val["STEP_TIMEOUT_MIN"])
        bat, title, interactive = steps["sp-playtest"]
        self.assertEqual(bat.replace("\\", "/"), "tools/first-run/Single player playtest.bat")
        self.assertFalse(interactive)
        self.assertNotIn("sp-playtest", ro)   # it starts the game and a world; the game folder is put back after
        self.assertGreaterEqual(to["sp-playtest"], 240)
        for word in ("World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard"):
            self.assertNotIn(word, title)
        raw = (ROOT / bat).read_bytes()
        self.assertEqual(raw.count(b"\n"), raw.count(b"\r\n"), "CRLF")
        self.assertIn(b'sp_playtest.py" run %*', raw)
        self.assertIn(b"-I", raw)


if __name__ == "__main__":
    unittest.main()
