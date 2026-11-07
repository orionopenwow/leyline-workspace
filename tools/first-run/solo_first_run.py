"""Leyline Solo's fresh-machine test (docs/FIRST-RUN-TEST.md, part 5): a brand-new Solo profile on this PC, your own
first-era game folder pointed at, setup, the world started, the game logging in by itself, character select reached,
then the owner's part watched (Quick start, .bot group), and everything put back. Control step `solo-first-run`.

    python -I tools/first-run/solo_first_run.py [run] [--game <first-era folder>] [--zip <engine zip>]
                                                [--installer <Leyline Solo setup .exe>] [--hold 600] [--keep]
                                                [--work <new empty folder>]

What runs by itself (PASS / FAIL, numbered as in docs/FIRST-RUN-TEST.md 5.2):
  S1  a fresh profile: a new empty folder for the app's data (LEYLINE_APP_DIR) and the world's (LEYLINE_SOLO_DIR);
      REFUSED when it is, holds or sits inside your own (%APPDATA%\\Leyline Solo, %APPDATA%\\Leyline,
      %LOCALAPPDATA%\\Leyline and its solo-proof, %LOCALAPPDATA%\\Leyline Solo), when the Solo ports are in use (your
      own world running) or when your game folder already has a sign-in archive in it (yours: solo-signin-off first)
  S2  install (only with --installer): the setup program installed silently into the test folder, its program there;
      uninstalled again at the end. Skipped when Leyline Solo is installed for real on this PC.
  S3  the folder: the first-era client found (tools/solo/solo_proof.py check)
  S4  setup: the engine (checked), the maps read from your folder, the database, the local account (up to an hour)
  S5  start: the world up on 127.0.0.1 ("World initialized")
  S6  sign-in: realmlist.wtf, the account name, our login-screen archive, backed up first
  S7  the game: started from your folder, alive, and character select reached: the world server's log shows the
      account SOLO logging in, or its login database marks SOLO on the realm (either one counts; both are reported)
What the test cannot do (it never drives the game's own screens): Quick start, typing .bot group, fighting. During
--hold it WATCHES for them in the world's database and logs (a character made, a character in the world, companions
in the world) and reports each as PASS (seen) or INFO (not seen: the owner's checklist, docs/FIRST-RUN-TEST.md 5.3).
  S8  clean up, always: the game closed (only the one this test started), the world stopped, sign-in off and your
      game folder byte for byte as it was (put back by this test if not), your own Solo and launcher folders
      unchanged, no password in any log, the test folder removed (--keep keeps it).

The generated passwords live only in the test folder's state.json (solo_proof.py's); they are never printed (every
line goes through Lines.hide). Logs: tools\\first-run\\logs\\solo-first-run-latest.txt and solo-first-run.json.
Standard library only; Windows (the tests run anywhere with fakes: tools/first-run/test_solo_first_run.py).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
LOGS = HERE / "logs"
SOLO_TOOL = ROOT / "tools" / "solo" / "solo_proof.py"
sys.path.insert(0, str(HERE))
from first_run import Lines, game_files, stash, unstash  # noqa: E402

ACCOUNT = "SOLO"
PORTS = {"login": 3724, "world": 8085, "database": 13336, "soap": 7878}   # solo_proof.py PORTS
SLOT = "patch-Z.MPQ"
EXE_NAMES = ("WoW.exe", "Wow.exe")   # the game's program in its folder (solo_proof.py EXE_NAMES)
SETUP_MIN = 120          # setup can take an hour; two before it counts as stuck
START_S = 900
CHARSEL_S = 300          # from the game starting to character select
REPORT = "solo-first-run.json"
LATEST = "solo-first-run-latest.txt"
# The world server's lines for the account reaching character select and a character entering the world. CMaNGOS
# writes the first from its world socket when a client's session is accepted (it reached the character list), the
# second to Char.log when a character logs in. TO CONFIRM on the first real run (the log level of the Solo profile
# decides which of them are written): the login database is the second signal, so a missing line alone is not a FAIL.
CHARSEL_RE = re.compile(r"(?i)(?:HandleAuthSession|session).{0,80}'?\b" + ACCOUNT + r"\b'?.{0,60}(?:authenticated|accepted|logged)"
                        r"|\baccount\b.{0,20}'?\b" + ACCOUNT + r"\b'?.{0,40}(?:logged in|online)")
WORLD_RE = re.compile(r"Login Character:\s*\[([A-Za-z]{2,12})\]")
WORLD_LOGS = ("Server.log", "mangosd.out", "Char.log", "World.log")
SECRET_KEYS = re.compile(r"(?i)(pw|pass|password|secret|token)$")


# ------------------------------------------------------------------------------------------------ where things are
def owner_dirs(env=None) -> list:
    """The owner's own folders this test must never be, hold or sit inside: the Solo app's data, the launcher's, and
    the Phase 0 tool's world (solo_proof.py home()); as they are on this PC, not as LEYLINE_* says."""
    env = os.environ if env is None else env
    out = []
    appdata = env.get("APPDATA")
    local = env.get("LOCALAPPDATA")
    if appdata:
        out += [Path(appdata) / "Leyline Solo", Path(appdata) / "Leyline"]
    if local:
        out += [Path(local) / "Leyline", Path(local) / "Leyline" / "solo-proof", Path(local) / "Leyline Solo"]
    home = Path.home()
    out += [home / ".local" / "share" / "Leyline", home / ".local" / "share" / "Leyline Solo"]
    return out


def _norm(p: Path) -> str:
    return os.path.normcase(os.path.abspath(str(p))).rstrip("\\/")


def _inside(a: str, b: str) -> bool:
    return a == b or a.startswith(b + os.sep) or a.startswith(b + "/")


def refuse_reason(work: Path, owners) -> str:
    """Why this work folder may not be used ("" when it may): it must be new or empty and apart from every one of the
    owner's own folders (not one of them, not inside one, not holding one)."""
    w = _norm(work)
    for o in owners:
        n = _norm(o)
        if _inside(w, n) or _inside(n, w):
            return "the test folder %s is (or holds, or sits inside) your own %s" % (work, o)
    if work.exists() and (not work.is_dir() or any(work.iterdir())):
        return "the test folder %s is not new and empty" % work
    return ""


def fingerprint(dirs) -> dict:
    """What we watch in the owner's own folders: each one's key files (sha256 and size) and its top level's names."""
    out = {}
    for d in dirs:
        d = Path(d)
        if not d.is_dir():
            out[str(d)] = None
            continue
        rec = {}
        for n in ("state.json", "settings.json", "SOLO password.txt"):
            p = d / n
            try:
                b = p.read_bytes()
                rec[n] = (hashlib.sha256(b).hexdigest(), len(b))
            except OSError:
                pass
        try:
            rec["names"] = sorted(os.listdir(d))
        except OSError:
            rec["names"] = None
        out[str(d)] = rec
    return out


def game_folder(arg, env=None):
    """(folder, where from): the argument, else tools/solo/folder.txt, else the owner's launcher's first first-era
    game folder (read only)."""
    env = os.environ if env is None else env
    if arg:
        return Path(arg), "the argument"
    f = ROOT / "tools" / "solo" / "folder.txt"
    try:
        line = f.read_text(encoding="utf-8").strip().splitlines()[0].strip().strip('"')
        if line:
            return Path(line), "tools\\solo\\folder.txt"
    except (OSError, IndexError):
        pass
    appdata = env.get("APPDATA")
    if appdata:
        try:
            s = json.loads((Path(appdata) / "Leyline" / "settings.json").read_text(encoding="utf-8"))
            for c in s.get("clients", []):
                if isinstance(c, dict) and str(c.get("build", "")).startswith("1.12") and c.get("path"):
                    return Path(c["path"]), "your launcher's game folders"
        except (OSError, ValueError, AttributeError):
            pass
    return None, ""


def find_ci(d: Path, name: str):
    try:
        for e in os.listdir(d):
            if e.lower() == name.lower():
                return d / e
    except OSError:
        pass
    return None


def game_exe(game: Path):
    for n in EXE_NAMES:
        p = find_ci(game, n)
        if p and p.is_file():
            return p
    return None


def slot_taken(game: Path) -> bool:
    data = find_ci(game, "Data") or game / "Data"
    return find_ci(data, SLOT) is not None


def secrets_of(state: dict) -> list:
    """Every password-like string in the test world's state.json (solo_proof.py keeps them there)."""
    out = []

    def walk(o, key=""):
        if isinstance(o, dict):
            for k, v in o.items():
                walk(v, str(k))
        elif isinstance(o, list):
            for v in o:
                walk(v, key)
        elif isinstance(o, str) and SECRET_KEYS.search(key) and len(o) >= 4:
            out.append(o)
    walk(state)
    return out


# ------------------------------------------------------------------------------------------------ the real PC
class Real:
    """Everything outside this file. The tests swap it for a fake."""

    windows = os.name == "nt"

    def __init__(self):
        self.games = []

    def solo(self, args, env, timeout):
        """solo_proof.py <args> in the fresh profile: (exit code, its output)."""
        try:
            p = subprocess.run([sys.executable, "-I", str(SOLO_TOOL)] + list(args), env=env, capture_output=True,
                               timeout=timeout)
        except subprocess.TimeoutExpired as e:
            return 124, ((e.stdout or b"").decode("utf-8", "replace") + "\n(stopped: took longer than %d s)" % timeout)
        except OSError as e:
            return 127, str(e)
        return p.returncode, (p.stdout + p.stderr).decode("utf-8", "replace")

    def port_free(self, port):
        s = socket.socket()
        try:
            s.bind(("127.0.0.1", port))
            return True
        except OSError:
            return False
        finally:
            s.close()

    def installed_for_real(self, env):
        """Leyline Solo installed for real on this PC (its per-user install folder or its uninstall entry)."""
        local = env.get("LOCALAPPDATA")
        if local and (Path(local) / "Leyline Solo" / "Leyline Solo.exe").exists():
            return True
        if not self.windows:
            return False
        try:
            import winreg   # noqa: PLC0415
            winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Leyline Solo").Close()
            return True
        except OSError:
            return False

    def run(self, args, timeout):
        try:
            return subprocess.run(list(args), capture_output=True, timeout=timeout).returncode
        except (OSError, subprocess.TimeoutExpired):
            return -1

    def start_game(self, exe: Path, env):
        p = subprocess.Popen([str(exe)], cwd=str(exe.parent), env=env)
        self.games.append(p)
        return p

    def alive(self, p):
        return p is not None and p.poll() is None

    def close(self, p):
        if p is not None and p.poll() is None:
            p.terminate()
            try:
                p.wait(20)
            except subprocess.TimeoutExpired:
                p.kill()

    def query(self, solo_home: Path, sql: str, db: str):
        """One read-only query on the TEST world's database (solo_proof.py's own client and login), or None."""
        code = ("import os,sys;sys.path.insert(0,sys.argv[1]);os.environ['LEYLINE_SOLO_DIR']=sys.argv[2];"
                "import solo_proof as s;print(s.mysql(s.load_state(),sys.stdin.read(),db=sys.argv[3],check=False),end='')")
        try:
            p = subprocess.run([sys.executable, "-I", "-c", code, str(SOLO_TOOL.parent), str(solo_home), db],
                               input=sql.encode("utf-8"), capture_output=True, timeout=30)
        except (OSError, subprocess.TimeoutExpired):
            return None
        return p.stdout.decode("utf-8", "replace") if p.returncode == 0 else None

    def sleep(self, s):
        time.sleep(s)

    def now(self):
        return time.monotonic()


# ------------------------------------------------------------------------------------------------ what we watch
def world_text(solo_home: Path) -> str:
    out = []
    for n in WORLD_LOGS:
        try:
            out.append((solo_home / "logs" / n).read_text(encoding="utf-8", errors="replace"))
        except OSError:
            pass
    return "\n".join(out)


def presence(io, solo_home: Path) -> dict:
    """What the test world's databases say about SOLO: on the realm (character select or later), its characters, the
    one in the world and companions in the world. Missing pieces stay None."""
    got = {"on_realm": None, "characters": None, "in_world": None, "companions": None}
    t = io.query(solo_home, "SELECT id, active_realm_id FROM account WHERE username='%s';" % ACCOUNT, "classicrealmd")
    acc = None
    if t:
        parts = t.strip().split()
        if len(parts) >= 2 and parts[0].isdigit():
            acc = parts[0]
            got["on_realm"] = parts[1] not in ("0", "NULL")
    if acc:
        t = io.query(solo_home, "SELECT name, online FROM characters WHERE account=%s;" % acc, "classiccharacters")
        if t is not None:
            rows = [r.split("\t") for r in t.strip().splitlines() if "\t" in r]
            got["characters"] = len(rows)
            online = [n for n, o in rows if o.strip() == "1"]
            got["in_world"] = online[0] if online else ""
            got["companions"] = max(0, len(online) - 1)
    return got


# ------------------------------------------------------------------------------------------------ the test
def run(a, io=None, lines=None, temp_base=None, owners=None, env=None) -> int:
    io = io or Real()
    ln = lines or Lines()
    env = dict(os.environ if env is None else env)
    owners = owner_dirs(env) if owners is None else owners
    LOGS.mkdir(parents=True, exist_ok=True)
    report = {"tool": "solo-first-run", "phases": {}}
    ln.add(None, "Leyline Solo fresh-machine test", "nothing of your own Solo or launcher is used; your game folder is put back")
    if not io.windows:
        ln.add(False, "S1 this PC", "not Windows", "Leyline Solo is a Windows app: run solo-first-run on the PC")
        return finish(ln, report)

    # S1: the fresh profile, and the refusals
    base = Path(temp_base or tempfile.gettempdir())
    work = Path(a.work) if a.work else base / ("leyline-solo-first-run-%d" % int(time.time()))
    why = refuse_reason(work, owners)
    if why:
        ln.add(False, "S1 a fresh profile", "REFUSED: " + why, "give --work a new empty folder of its own (or none)")
        return finish(ln, report)
    busy = [f"{k} {p}" for k, p in PORTS.items() if not io.port_free(p)]
    if busy:
        ln.add(False, "S1 the Solo ports are free", "REFUSED: in use: " + ", ".join(busy),
               "your own Solo world (or another server) is running: press solo-stop, then run this again")
        return finish(ln, report)
    game, where = game_folder(a.game, env)
    if not game or not game.is_dir():
        ln.add(False, "S1 your first-era game folder", "not found",
               "put its path in tools\\solo\\folder.txt, or run with --game <folder>")
        return finish(ln, report)
    if any(_inside(_norm(game), _norm(o)) for o in owners):
        ln.add(False, "S1 your first-era game folder", "REFUSED: it is inside your own data folders", "use the game folder itself")
        return finish(ln, report)
    if slot_taken(game):
        ln.add(False, "S1 your game folder has no sign-in archive", "REFUSED: Data\\%s is there (your own Solo sign-in?)" % SLOT,
               "press solo-signin-off (or remove that archive), then run this again")
        return finish(ln, report)
    app_dir, solo_home = work / "app", work / "solo"
    app_dir.mkdir(parents=True)
    solo_home.mkdir()
    child = dict(env, LEYLINE_APP_DIR=str(app_dir), LEYLINE_SOLO_DIR=str(solo_home))
    before = fingerprint(owners)
    saved = stash(game_files(game))
    ln.add(True, "S1 a fresh profile", "%s (app data and world inside, both empty); game folder %s (from %s)" % (work, game, where))
    report["phases"]["S1"] = True

    held = {"game": None, "installed": None}
    try:
        _phases(a, io, ln, env, child, game, solo_home, work, report, held)
    finally:
        cleanup(ln, io, child, game, held["game"], saved, before, owners, work, solo_home, held["installed"], a.keep, report)
    return finish(ln, report)


def _phases(a, io, ln, env, child, game, solo_home, work, report, held):
    """S2 to S7 and the owner's part; returns at the first FAIL (the caller cleans up, always)."""
    # S2: install (optional)
    if a.installer:
        held["installed"] = install(ln, io, Path(a.installer), work, env)
        report["phases"]["S2"] = bool(held["installed"])
    else:
        ln.add(None, "S2 install", "skipped (no --installer): the world is set up by the Solo engine tool, as the app does")

    # S3 .. S6: the folder, setup, start, sign-in
    steps = [("S3 your folder is the first-era client", ["check", str(game)], 120, None),
             ("S4 setup: the engine, your maps, the database, the account", ["setup", str(game)] + (["--zip", a.zip] if a.zip else []),
              SETUP_MIN * 60, "SETUP OK"),
             ("S5 start: the world on 127.0.0.1", ["start", "--wait", str(START_S)], START_S + 60, "START OK"),
             ("S6 sign-in: the game logs in by itself", ["signin", str(game)], 300, "SIGNIN ON")]
    for what, args, limit, want in steps:
        t0 = io.now()
        rc, text = io.solo(args, child, limit)
        for s in secrets_of(read_state(solo_home)):
            ln.secret(s)
        fails = [l.strip() for l in text.splitlines() if l.strip().startswith("FAIL")]
        ok = rc == 0 and not fails and (want is None or want in text)
        mins = (io.now() - t0) / 60
        detail = ("in %.0f min" % mins) if mins >= 1 else "done"
        ln.add(ok, what, detail if ok else (fails[0] if fails else "exit %d%s" % (rc, (", no " + want) if want else "")),
               "the step's own lines are in %s; fix and run again" % LATEST)
        report["phases"][what[:2]] = ok
        log_child(ln, what[:2], text)
        if not ok:
            return

    # S7: the game, to character select
    exe = game_exe(game)
    if not exe:
        ln.add(False, "S7 the game starts", "no game program in the folder", "point --game at the folder with the game's program")
        return
    game_p = held["game"] = io.start_game(exe, env)
    seen = watch(io, ln, solo_home, game_p, CHARSEL_S, lambda s: s["log_charsel"] or s["on_realm"])
    alive = io.alive(game_p)
    ln.add(alive, "S7 the game is running", exe.name, "the game closed by itself: open it from the folder once by hand and see what it says")
    ok = bool(seen and (seen["log_charsel"] or seen["on_realm"]))
    how = []
    if seen:
        how.append("the world log: " + ("yes" if seen["log_charsel"] else "no line (to confirm, see 5.4)"))
        how.append("the login database: " + ({True: "yes", False: "no", None: "not read"}[seen["on_realm"]]))
    ln.add(ok and alive, "S7 character select reached by itself", "; ".join(how) or "nothing seen in %d s" % CHARSEL_S,
           "look at the game: stopped at the login screen, or \"interface files are corrupt\"? Our sign-in archive "
           "was not taken: the fallback is Simple sign-in (solo-signin --no-addon; docs/LEYLINE-SOLO-PHASE0.md)")
    report["phases"]["S7"] = ok and alive
    if not (ok and alive):
        return

    # the owner's part, watched: Quick start, .bot group
    ln.add(None, "OWNER now", "in the game: Quick start (or make a character), enter the world, type .bot group, "
           "fight a little. This test watches for %d s and never presses anything." % a.hold)
    got = watch(io, ln, solo_home, game_p, a.hold, lambda s: (s["companions"] or 0) >= 1)
    owner_lines(ln, got, report)


def read_state(solo_home: Path) -> dict:
    try:
        s = json.loads((solo_home / "state.json").read_text(encoding="utf-8"))
        return s if isinstance(s, dict) else {}
    except (OSError, ValueError):
        return {}


def log_child(ln, tag, text):
    """The step's own lines into the log file (hidden through Lines), not on the screen."""
    ln.child = getattr(ln, "child", [])
    ln.child += ["[%s] %s" % (tag, ln.hide(l)) for l in text.splitlines()[-60:]]
    ln.raw = getattr(ln, "raw", "") + text   # only to check below that the tool itself never printed a password


def watch(io, ln, solo_home, game_p, seconds, done):
    """Poll the test world every 5 s for up to `seconds`: its log lines and its database. The last state seen."""
    end = io.now() + seconds
    st = None
    while True:
        text = world_text(solo_home)
        st = dict(presence(io, solo_home), log_charsel=bool(CHARSEL_RE.search(text)),
                  log_world=WORLD_RE.findall(text))
        if done(st) or io.now() >= end or not io.alive(game_p):
            return st
        io.sleep(5)


def owner_lines(ln, s, report):
    s = s or {}
    made = s.get("characters")
    inw = s.get("in_world") or (s.get("log_world") or [None])[0]
    comp = s.get("companions")
    for what, seen, detail in (
            ("a character made (Quick start or the creation screen)", bool(made), "%s on the account" % made if made else ""),
            ("a character in the world", bool(inw), inw or ""),
            ("companions in the world (.bot group)", bool(comp), "%s with you" % comp if comp else "")):
        ln.add(True if seen else None, "OWNER " + what, detail if seen else "not seen: the owner's checklist (docs/FIRST-RUN-TEST.md 5.3)")
        report["phases"]["owner: " + what] = bool(seen)


def install(ln, io, installer: Path, work: Path, env):
    """The setup program installed silently into the test folder (NSIS /S /D=), its program checked. The folder, or None."""
    if io.installed_for_real(env):
        ln.add(None, "S2 install", "skipped: Leyline Solo is installed for real on this PC (a second install would take over "
               "its Start menu entry); use Windows Sandbox for the installer")
        return None
    if not installer.is_file():
        ln.add(False, "S2 install", "no file %s" % installer, "download the setup program from the leyline-solo releases")
        return None
    dest = work / "install"
    rc = io.run([str(installer), "/S", "/D=" + str(dest)], 600)
    exe = dest / "Leyline Solo.exe"
    try:
        ok = rc == 0 and exe.is_file() and exe.stat().st_size > 1_000_000 and exe.read_bytes()[:2] == b"MZ"
    except OSError:
        ok = False
    ln.add(ok, "S2 install", "silent install into the test folder: %s" % ("Leyline Solo.exe there" if ok else "exit %d" % rc),
           "run the setup program by hand once and see what it says")
    return dest if ok else None


def cleanup(ln, io, child, game, game_p, saved, before, owners, work, solo_home, installed, keep, report):
    secrets = list(ln.secrets) + secrets_of(read_state(solo_home))
    if game_p is not None:
        io.close(game_p)
        io.sleep(3)   # the game lets go of the archive
    rc, text = io.solo(["stop"], child, 300)
    log_child(ln, "S8", text)
    ln.add(rc == 0, "S8 the test world stopped", "", "press solo-status; the test world's folder is %s" % solo_home)
    rc, text = io.solo(["signin", "--off"], child, 300)
    log_child(ln, "S8", text)
    ln.add(rc == 0 and "SIGNIN OFF" in text, "S8 sign-in off", "", "the game may still be open: close it, then solo-signin-off")
    now = stash(list(saved))
    diff = [str(p) for p in saved if now.get(p) != saved[p]]
    if diff:
        bad = unstash(saved)
        ln.add(False, "S8 your game folder as it was", "differed: %s; %s" % (", ".join(Path(d).name for d in diff),
               "put back by this test" if not bad else "COULD NOT put back: " + ", ".join(bad)),
               "tell Claude: sign-in off should restore these byte for byte")
    else:
        ln.add(True, "S8 your game folder as it was", "byte for byte (%d files checked)" % len(saved))
    if installed:
        unins = installed / "uninstall.exe"
        rc = io.run([str(unins), "/S"], 300) if unins.exists() else -1
        ln.add(rc == 0, "S8 uninstalled", "", "remove %s by hand" % installed)
    after = fingerprint(owners)
    changed = [d for d in before if before[d] != after.get(d)]
    ln.add(not changed, "S8 your own Solo and launcher folders unchanged", ", ".join(changed) if changed else "",
           "tell Claude: this test must never touch them")
    leaked = scan_secrets(secrets, [solo_home / "logs"], getattr(ln, "raw", ""))
    ln.raw = ""
    ln.add(not leaked, "S8 no password in any log", ", ".join(leaked), "tell Claude, and delete those logs")
    if keep:
        ln.add(None, "S8 the test folder kept", str(work))
    else:
        shutil.rmtree(work, ignore_errors=True)
        ln.add(not work.exists(), "S8 the test folder removed", str(work), "delete it by hand")
    report["phases"]["S8"] = not ln_failed_since(ln, "S8")


def ln_failed_since(ln, tag):
    return any(k == "FAIL" and w.startswith(tag) for k, w, _d, _f in ln.rows)


def scan_secrets(secrets, dirs, own_text) -> list:
    """Where a password shows: this test's own lines, and every file in the test world's logs."""
    hits = []
    secrets = [s for s in secrets if s and len(s) >= 4]
    if any(s in own_text for s in secrets):
        hits.append("the Solo tool's own lines")
    for d in dirs:
        for p in Path(d).rglob("*") if Path(d).is_dir() else []:
            try:
                b = p.read_bytes()
            except OSError:
                continue
            if any(s.encode("utf-8") in b for s in secrets):
                hits.append(p.name)
    return hits


def finish(ln, report) -> int:
    report["ok"] = not ln.failed()
    LOGS.mkdir(parents=True, exist_ok=True)
    body = ln.text() + ("\n-- the steps' own lines --\n" + "\n".join(getattr(ln, "child", [])) + "\n" if getattr(ln, "child", None) else "")
    (LOGS / LATEST).write_text(ln.hide(body), encoding="utf-8")
    (LOGS / REPORT).write_text(ln.hide(json.dumps(report, indent=1)), encoding="utf-8")
    ln.out("SOLO FIRST RUN %s" % ("OK" if report["ok"] else "FAILED"))
    return 0 if report["ok"] else 1


def main(argv=None):
    ap = argparse.ArgumentParser(prog="solo_first_run.py", description="Leyline Solo's fresh-machine test")
    sub = ap.add_subparsers(dest="cmd")
    r = sub.add_parser("run")
    for p in (ap, r):
        p.add_argument("--game", help="your first-era game folder (else tools\\solo\\folder.txt, else the launcher's)")
        p.add_argument("--zip", help="a local engine zip instead of the download (its .sha256 beside it is checked)")
        p.add_argument("--installer", help="the Leyline Solo setup program, to test the install too")
        p.add_argument("--hold", type=int, default=600, help="seconds to watch for the owner's part (default 600)")
        p.add_argument("--keep", action="store_true", help="keep the test folder")
        p.add_argument("--work", help="the test folder (new and empty; default under %%TEMP%%)")
    a = ap.parse_args(argv)
    return run(a)


if __name__ == "__main__":
    sys.exit(main())
