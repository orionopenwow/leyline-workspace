#!/usr/bin/env python3
"""Leyline Solo's fresh-machine test (tools/first-run/solo_first_run.py) with everything outside faked: the Solo
engine tool (check, setup, start, sign-in on and off, stop), the ports, the game, the world's logs and databases, the
installer. The refusals (your own folders, a used folder, the ports, your own sign-in archive), the clean up after a
FAIL, your game folder put back, no password in a log, and the Control step and its bat.

    python3 -I tools/first-run/test_solo_first_run.py
"""
import ast
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import solo_first_run as sf  # noqa: E402

PW = {"root_pw": "RootPass-123456", "db_pw": "DbPass-987654", "solo_pw": "SoloPass-abcdef", "stop_pw": "StopPass-zyxwvu"}
REALMLIST = b"set realmlist logon.example\r\n"
CONFIG = b'SET accountName "someone"\r\n'


class Args:
    game = None
    zip = None
    installer = None
    hold = 60
    keep = False
    work = None


class Game:
    def __init__(self):
        self.open = True


class Fake:
    windows = True

    def __init__(self, t, **kw):
        self.t = t
        self.calls, self.envs, self.clock = [], [], 0.0
        self.busy = kw.get("busy", set())
        self.fail = kw.get("fail")              # the solo_proof command that fails
        self.charsel_log = kw.get("charsel_log", True)
        self.on_realm = kw.get("on_realm", True)
        self.chars = kw.get("chars", [("Tester", "1"), ("Bota", "1"), ("Botb", "1")])
        self.off_restores = kw.get("off_restores", True)
        self.leak = kw.get("leak", False)
        self.print_pw = kw.get("print_pw", False)
        self.real_install = kw.get("real_install", False)
        self.games, self.closed, self.runs = [], [], []

    # solo_proof.py in the fresh profile
    def solo(self, args, env, timeout):
        self.calls.append(" ".join(a for a in args if not a.startswith(str(self.t))))
        self.envs.append(env)
        home = Path(env["LEYLINE_SOLO_DIR"])
        cmd = args[0] + (" --off" if "--off" in args else "")
        if cmd == self.fail:
            return 1, "FAIL the %s went wrong" % cmd
        if cmd == "check":
            return 0, "PASS the first-era client\nINFO disk"
        if cmd == "setup":
            (home / "logs").mkdir(parents=True, exist_ok=True)
            (home / "state.json").write_text(json.dumps(dict(PW, setup_done=True)), encoding="utf-8")
            return 0, "SETUP OK in 41 min: the local account SOLO is made" + ((" " + PW["solo_pw"]) if self.print_pw else "")
        if cmd == "start":
            return 0, "START OK: the Solo world is up"
        if cmd == "signin":
            game = Path(args[1])
            self.backup = {p: p.read_bytes() for p in (game / "realmlist.wtf", game / "WTF" / "Config.wtf")}
            (game / "Data" / "patch-Z.MPQ").write_bytes(b"MPQ ours")
            (game / "realmlist.wtf").write_bytes(b"set realmlist 127.0.0.1\r\n")
            (game / "WTF" / "Config.wtf").write_bytes(b'SET accountName "SOLO"\r\n')
            return 0, "SIGNIN ON: open your game"
        if cmd == "signin --off":
            game = self.t / "game"
            if self.off_restores and hasattr(self, "backup"):
                for p, b in self.backup.items():
                    p.write_bytes(b)
                (game / "Data" / "patch-Z.MPQ").unlink(missing_ok=True)
            return 0, "SIGNIN OFF: your game folder is as it was."
        if cmd == "stop":
            return 0, "STOP OK"
        return 2, "unknown"

    def port_free(self, port):
        return port not in self.busy

    def installed_for_real(self, env):
        return self.real_install

    def run(self, args, timeout):
        self.runs.append(args)
        if "/S" in args and any(a.startswith("/D=") for a in args):
            dest = Path(next(a for a in args if a.startswith("/D="))[3:])
            dest.mkdir(parents=True)
            (dest / "Leyline Solo.exe").write_bytes(b"MZ" + b"\0" * 1_100_000)
            (dest / "uninstall.exe").write_bytes(b"MZ")
        return 0

    def start_game(self, exe, env):
        g = Game()
        self.games.append(g)
        home = Path(self.envs[-1]["LEYLINE_SOLO_DIR"])
        lines = []
        if self.charsel_log:
            lines.append("WorldSocket::HandleAuthSession: Client 'SOLO' authenticated successfully from 127.0.0.1.")
        if self.leak:
            lines.append("debug " + PW["db_pw"])
        (home / "logs" / "Server.log").write_text("\n".join(lines) + "\n", encoding="utf-8")
        return g

    def alive(self, g):
        return g is not None and g.open

    def close(self, g):
        g.open = False
        self.closed.append(g)

    def query(self, home, sql, db):
        if "FROM account" in sql:
            return "1\t%d\n" % (1 if self.on_realm else 0)
        if "FROM characters" in sql:
            return "".join("%s\t%s\n" % r for r in self.chars)
        return None

    def sleep(self, s):
        self.clock += s

    def now(self):
        return self.clock


class Out:
    def __init__(self):
        self.lines = []

    def __call__(self, s):
        self.lines.append(s)


class SoloFirstRun(unittest.TestCase):
    def setUp(self):
        self.t = Path(tempfile.mkdtemp(prefix="solo-fr-"))
        self.addCleanup(shutil.rmtree, self.t, True)
        game = self.t / "game"
        (game / "Data").mkdir(parents=True)
        (game / "WTF").mkdir()
        (game / "Wow.exe").write_bytes(b"MZ")
        (game / "realmlist.wtf").write_bytes(REALMLIST)
        (game / "WTF" / "Config.wtf").write_bytes(CONFIG)
        self.game = game
        self.env = {"APPDATA": str(self.t / "Roaming"), "LOCALAPPDATA": str(self.t / "Local")}
        for d in (self.t / "Roaming" / "Leyline", self.t / "Roaming" / "Leyline Solo", self.t / "Local" / "Leyline" / "solo-proof"):
            d.mkdir(parents=True)
        (self.t / "Roaming" / "Leyline Solo" / "settings.json").write_text('{"mine": 1}', encoding="utf-8")
        (self.t / "Local" / "Leyline" / "solo-proof" / "state.json").write_text('{"solo_pw": "OwnersOwnPass"}', encoding="utf-8")
        self.owners = sf.owner_dirs(self.env)
        self.before = sf.fingerprint(self.owners)
        self.logs = self.t / "logs"
        self._logs = sf.LOGS
        sf.LOGS = self.logs
        self.addCleanup(setattr, sf, "LOGS", self._logs)

    def go(self, io, **kw):
        a = Args()
        a.game = str(self.game)
        for k, v in kw.items():
            setattr(a, k, v)
        out = Out()
        ln = sf.Lines(out=out)
        rc = sf.run(a, io=io, lines=ln, temp_base=self.t / "tmp", env=self.env)
        self.text = (self.logs / sf.LATEST).read_text(encoding="utf-8")
        self.report = json.loads((self.logs / sf.REPORT).read_text(encoding="utf-8"))
        self.out = out.lines
        return rc

    def assertGameAsItWas(self):
        self.assertEqual((self.game / "realmlist.wtf").read_bytes(), REALMLIST)
        self.assertEqual((self.game / "WTF" / "Config.wtf").read_bytes(), CONFIG)
        self.assertFalse((self.game / "Data" / "patch-Z.MPQ").exists())

    # ------------------------------------------------------------------ the whole way
    def test_the_whole_way(self):
        io = Fake(self.t)
        (self.t / "tmp").mkdir()
        rc = self.go(io)
        self.assertEqual(rc, 0, self.text)
        self.assertEqual(io.calls, ["check", "setup", "start --wait 900", "signin", "stop", "signin --off"])
        for env in io.envs:   # every step in the fresh profile, never the owner's
            for k in ("LEYLINE_SOLO_DIR", "LEYLINE_APP_DIR"):
                self.assertTrue(env[k].startswith(str(self.t / "tmp" / "leyline-solo-first-run-")), env[k])
            self.assertNotEqual(env["LEYLINE_SOLO_DIR"], env["LEYLINE_APP_DIR"])
        for want in ("PASS S1 a fresh profile", "PASS S3", "PASS S4", "PASS S5", "PASS S6", "PASS S7 the game is running",
                     "PASS S7 character select reached by itself", "the world log: yes", "the login database: yes",
                     "PASS OWNER a character made", "PASS OWNER a character in the world: Tester",
                     "PASS OWNER companions in the world (.bot group): 2 with you", "PASS S8 the test world stopped",
                     "PASS S8 sign-in off", "PASS S8 your game folder as it was", "PASS S8 your own Solo and launcher folders unchanged",
                     "PASS S8 no password in any log", "PASS S8 the test folder removed"):
            self.assertIn(want, self.text)
        self.assertIn("INFO S2 install: skipped", self.text)
        self.assertEqual(self.out[-1], "SOLO FIRST RUN OK")
        self.assertTrue(self.report["ok"])
        self.assertGameAsItWas()
        self.assertEqual(sf.fingerprint(self.owners), self.before)
        self.assertEqual(len(io.closed), 1)
        self.assertEqual(list((self.t / "tmp").iterdir()), [])
        for pw in PW.values():
            self.assertNotIn(pw, self.text + json.dumps(self.report))

    def test_keep_keeps_the_test_folder(self):
        work = self.t / "mywork"
        self.assertEqual(self.go(Fake(self.t), keep=True, work=str(work)), 0, self.text)
        self.assertTrue((work / "solo" / "state.json").exists())
        self.assertIn("INFO S8 the test folder kept", self.text)

    def test_the_install_into_the_test_folder_and_out_again(self):
        inst = self.t / "Leyline Solo setup.exe"
        inst.write_bytes(b"MZ")
        io = Fake(self.t)
        self.assertEqual(self.go(io, installer=str(inst)), 0, self.text)
        self.assertIn("PASS S2 install", self.text)
        self.assertIn("PASS S8 uninstalled", self.text)
        self.assertEqual(io.runs[0][1], "/S")
        self.assertTrue(io.runs[0][2].startswith("/D=" + str(self.t / "tmp" / "leyline-solo-first-run-")))
        self.assertEqual(io.runs[1][1:], ["/S"])

    def test_no_install_over_a_real_install(self):
        inst = self.t / "setup.exe"
        inst.write_bytes(b"MZ")
        io = Fake(self.t, real_install=True)
        self.assertEqual(self.go(io, installer=str(inst)), 0, self.text)
        self.assertIn("INFO S2 install: skipped: Leyline Solo is installed for real", self.text)
        self.assertEqual(io.runs, [])

    # ------------------------------------------------------------------ the refusals: nothing runs
    def refused(self, io, rc, words):
        self.assertEqual(rc, 1)
        self.assertIn(words, self.text)
        self.assertEqual(io.calls, [])
        self.assertEqual(io.games, [])
        self.assertEqual(sf.fingerprint(self.owners), self.before)
        self.assertEqual((self.game / "realmlist.wtf").read_bytes(), REALMLIST)

    def test_refuses_your_own_folders(self):
        for work in (self.t / "Roaming" / "Leyline Solo", self.t / "Roaming" / "Leyline Solo" / "x",
                     self.t / "Local" / "Leyline" / "solo-proof", self.t / "Local" / "Leyline" / "new",
                     self.t / "Roaming", self.t / "Roaming" / "Leyline"):
            io = Fake(self.t)
            self.refused(io, self.go(io, work=str(work)), "REFUSED")

    def test_refuses_a_used_folder(self):
        work = self.t / "used"
        work.mkdir()
        (work / "x").write_text("x")
        io = Fake(self.t)
        self.refused(io, self.go(io, work=str(work)), "is not new and empty")

    def test_refuses_when_the_solo_ports_are_in_use(self):
        io = Fake(self.t, busy={3724})
        self.refused(io, self.go(io), "REFUSED: in use: login 3724")
        self.assertIn("solo-stop", self.text)

    def test_refuses_your_own_sign_in_archive(self):
        (self.game / "Data" / "patch-Z.MPQ").write_bytes(b"the owner's")
        io = Fake(self.t)
        self.refused(io, self.go(io), "REFUSED: Data\\patch-Z.MPQ is there")
        self.assertEqual((self.game / "Data" / "patch-Z.MPQ").read_bytes(), b"the owner's")

    def test_refuses_without_a_game_folder(self):
        io = Fake(self.t)
        self.refused(io, self.go(io, game=str(self.t / "nope")), "S1 your first-era game folder: not found")

    def test_not_windows(self):
        io = Fake(self.t)
        io.windows = False
        self.refused(io, self.go(io), "not Windows")

    def test_refuse_reason(self):
        o = [self.t / "own"]
        self.assertTrue(sf.refuse_reason(self.t / "own", o))
        self.assertTrue(sf.refuse_reason(self.t / "own" / "in", o))
        self.assertTrue(sf.refuse_reason(self.t, o))   # it would hold the owner's folder
        self.assertEqual(sf.refuse_reason(self.t / "owner-not", o), "")   # a name that starts the same is apart
        self.assertEqual(sf.refuse_reason(self.t / "fresh", o), "")

    def test_owner_dirs_are_the_real_places_not_the_env_overrides(self):
        env = dict(self.env, LEYLINE_APP_DIR=str(self.t / "elsewhere"), LEYLINE_SOLO_DIR=str(self.t / "elsewhere2"))
        names = [str(p) for p in sf.owner_dirs(env)]
        for want in (self.t / "Roaming" / "Leyline Solo", self.t / "Roaming" / "Leyline", self.t / "Local" / "Leyline" / "solo-proof"):
            self.assertIn(str(want), names)

    # ------------------------------------------------------------------ a FAIL: stop there, clean up all the same
    def test_setup_fails_cleanup_still_runs(self):
        io = Fake(self.t, fail="setup")
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL S4 setup", self.text)
        self.assertIn("FAIL the setup went wrong", self.text)
        self.assertEqual(io.calls, ["check", "setup", "stop", "signin --off"])
        self.assertEqual(io.games, [])
        self.assertIn("PASS S8 your game folder as it was", self.text)
        self.assertIn("SOLO FIRST RUN FAILED", self.out[-1])

    def test_character_select_not_reached(self):
        io = Fake(self.t, charsel_log=False, on_realm=False)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL S7 character select reached by itself", self.text)
        self.assertIn("Simple sign-in", self.text)
        self.assertNotIn("OWNER now", self.text)
        self.assertEqual(len(io.closed), 1)   # the game this test started is closed
        self.assertGameAsItWas()

    def test_either_signal_counts_for_character_select(self):
        io = Fake(self.t, charsel_log=False, on_realm=True)
        self.assertEqual(self.go(io), 0, self.text)
        self.assertIn("the world log: no line (to confirm, see 5.4); the login database: yes", self.text)
        io = Fake(self.t, charsel_log=True, on_realm=False)
        self.assertEqual(self.go(io), 0, self.text)

    def test_the_owners_part_not_seen_is_info_not_fail(self):
        io = Fake(self.t, chars=[])
        self.assertEqual(self.go(io), 0, self.text)
        for what in ("a character made", "a character in the world", "companions in the world"):
            self.assertIn("INFO OWNER " + what, self.text)
        self.assertIn("the owner's checklist", self.text)

    def test_sign_in_off_not_restoring_is_a_fail_and_the_test_puts_it_back(self):
        io = Fake(self.t, off_restores=False)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL S8 your game folder as it was: differed", self.text)
        self.assertIn("put back by this test", self.text)
        self.assertGameAsItWas()

    def test_a_password_in_a_server_log_is_a_fail(self):
        io = Fake(self.t, leak=True)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL S8 no password in any log: Server.log", self.text)
        self.assertNotIn(PW["db_pw"], self.text)

    def test_a_password_printed_by_the_tool_is_hidden_and_a_fail(self):
        io = Fake(self.t, print_pw=True)
        self.assertEqual(self.go(io), 1)
        self.assertIn("FAIL S8 no password in any log: the Solo tool's own lines", self.text)
        self.assertNotIn(PW["solo_pw"], self.text)

    def test_the_log_patterns(self):
        self.assertTrue(sf.CHARSEL_RE.search("WorldSocket::HandleAuthSession: Client 'SOLO' authenticated successfully from 127.0.0.1."))
        self.assertFalse(sf.CHARSEL_RE.search("WorldSocket::HandleAuthSession: Client 'SOLOSTOP' authenticated successfully"))
        self.assertEqual(sf.WORLD_RE.findall("Account: 1 (IP: 127.0.0.1) Login Character:[Tester] (guid: 1)"), ["Tester"])

    def test_secrets_of(self):
        self.assertEqual(sorted(sf.secrets_of({"root_pw": "abcdef", "x": {"solo_password": "ghijkl"}, "name": "SOLO", "pw": "ab"})),
                         ["abcdef", "ghijkl"])

    # ------------------------------------------------------------------ the Control step and its bat
    def test_control_step(self):
        src = (ROOT / "tools" / "control" / "control.py").read_text(encoding="utf-8")
        tree = ast.parse(src)
        val = {t.id: n.value for n in tree.body if isinstance(n, ast.Assign) for t in n.targets if hasattr(t, "id")}
        steps = ast.literal_eval(val["STEPS"])
        ro = ast.literal_eval(val["READ_ONLY"])
        to = ast.literal_eval(val["STEP_TIMEOUT_MIN"])
        bat, title, interactive = steps["solo-first-run"]
        self.assertEqual(bat.replace("\\", "/"), "tools/first-run/Solo first run test.bat")
        self.assertFalse(interactive)
        self.assertNotIn("solo-first-run", ro)   # it starts the game and changes the game folder (put back after)
        self.assertGreaterEqual(to["solo-first-run"], 150)
        self.assertIn("refuses your own", title)
        raw = (ROOT / bat).read_bytes()
        self.assertEqual(raw.count(b"\n"), raw.count(b"\r\n"), "CRLF")
        self.assertIn(b'solo_first_run.py" run %*', raw)
        self.assertIn(b"-I", raw)


if __name__ == "__main__":
    unittest.main()
