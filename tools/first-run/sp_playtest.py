"""Single player playtest on this PC, nobody at the keyboard (owner 11 Nov: "make sure you do your own playtests on my
pc and level up before releasing and then release"). Control step `sp-playtest`.

With --bossrush (Control step `sp-playtest-boss`; owner 12 Nov: "tested by you at least a full boss fight run
through"): Boss Rush instead of single player, in the same fresh data folder with the same guards, puts and clean-up:
`Leyline play-test --fresh --bossrush --boss <start> --checks bossstart,bossfight`: the Boss Rush install set up from
your game folder, the start picker's one click (the hero, the game signed in), then `bossstart` (the raid in front of
the boss, a full companion raid, on the realm's own word) and `bossfight` (the hero on the bots' own AI, the fight
begun as a player would, watched to its end: the engage, the boss's phase casts, a kill or a wipe as it was, then
Retry back in front of the boss). Results in logs\boss-playtest\<time>\ and logs\boss-playtest\latest.txt.

    python -I tools/first-run/sp_playtest.py run --bossrush [--boss frozen-throne] [--size 10|25] [--diff normal|heroic]
                                                [--hero mage-fire] [--fight-max 1500] [--checks bossstart,bossfight]

    python -I tools/first-run/sp_playtest.py [run] [--kind single|vanilla] [--both] [--game <Wrath folder>]
                                                [--hold 20] [--level 10] [--checks levelup,companions,stoponexit,repair]
                                                [--no-key] [--keep] [--work <folder>] [--timeout 230]

What it does:
  1. refuses to start when it could touch your own things: the test folder is (or holds, or sits inside) your own
     launcher's or Solo's data; the local world's ports (3724, 8085, 13306, 17878) are in use (your own Single player
     world running: Stop it first); the drive has less than 12 GB free; not Windows;
  2. downloads the test build of the launcher (the `launcher-test` prerelease of leyline-workspace, built from a test
     branch by the launcher check workflow) and checks it against its .sha256 and for the single player playtest;
  3. your Wrath game folder from your launcher's saved game folders (read only), else tools\\first-run\\folder.txt,
     else --game; its files the game start changes are copied aside first (Config.wtf, realmlist.wtf, Leyline's
     patch archives) and put back after, byte for byte;
  4. a test Hero Key from the website's own test-account API (as first-run-test does) so the world plays on the Hero
     Key's own login; without one (--no-key, or the website says no) the world's own login, as offline;
  5. `Leyline play-test --fresh --single single --checks ...` in a brand-new data folder: world_find_folder, the disk
     check, setup (the engine, the maps from your folder, the database), Play with a random Quick start, in the world,
     then: companions in the group within 30 s; server-driven levelling to level 10 on the test's own local world (not
     hand play: said so in the report); the game closed and the world stopping by itself; a deleted engine file put
     back by Repair and Play into the world again. Screenshots of the game window at each phase;
  6. always: the test's world stopped (only programs inside the test folder), the game it started closed, your game
     folder put back, the test account closed, the test folder removed (--keep keeps it), your own launcher checked
     untouched, no secret in any file.
Results: logs\\sp-playtest\\<time>\\ (the PASS / FAIL table, each run's sp-play-test.json, its lines, the world's logs,
the screenshots) and logs\\sp-playtest\\latest.txt. Standard library only; tests: tools/first-run/test_sp_playtest.py.
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
import urllib.error
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import first_run as fr  # noqa: E402
from first_run import Lines  # noqa: E402
from solo_first_run import owner_dirs, refuse_reason  # noqa: E402

RELEASE = "https://github.com/orionopenwow/leyline-workspace/releases/download/launcher-test/"
EXE_URL = RELEASE + "Leyline.exe"
SHA_URL = RELEASE + "Leyline.exe.sha256"
INFO_URL = RELEASE + "launcher-test.json"
SP_MARK = b"leyline-play-test-single-v1"      # launcher/src-tauri/src/sptest.rs SP_MARK
REPORT = "sp-play-test.json"                  # sptest.rs REPORT
BR_MARK = b"leyline-play-test-bossrush-v1"    # sptest.rs BR_MARK
BR_REPORT = "br-play-test.json"               # sptest.rs BR_REPORT
BR_CHECKS = "bossstart,bossfight"
# Boss Rush's report phases (app.rs bossrush_play_test_cli) the game window is photographed at
BR_SHOTS = {"entering": "entering", "in_world": "in-world", "boss_ready": "in-front-of-the-boss", "fighting": "fighting",
            "fight_end": "fight-end", "retried": "retried"}
BR_WORLD_PHASES = ("boss_ready", "fighting")
CLI_LOG = "sp-play-test.log"                  # sptest.rs LOG
KEY_ENV = fr.KEY_ENV
PORTS = {"login": 3724, "world": 8085, "database": 13306, "soap": 17878}   # host/engine.rs
NEED_GB = 12          # the engine, the maps and the database of a fresh world, with room
CHECKS = "levelup,companions,stoponexit,repair"
KINDS = ("single", "vanilla")
TIMEOUT_MIN = 230     # Control gives the step 240
# the report's phases the game window is photographed at (sptest.rs / app.rs single_play_test_cli)
SHOTS = {"entering": "entering", "in_world": "in-world", "levelled": "levelled", "entering_again": "entering-again",
         "repaired_in_world": "after-repair"}
WORLD_PHASES = ("in_world", "levelled", "repaired_in_world")
SHOT_EVERY_S = 60
MAX_SHOTS = 40
FIX = {
    "Hero Key": "The website refused the test key: run signin, then try again (or --no-key).",
    "game folder": "Add your Wrath folder in your launcher (Game folders), or write its path in tools\\first-run\\folder.txt.",
    "preflight": "Free some space on the drive the test folder is on, or pass --work <a folder on a bigger drive>.",
    "setup": "Read setup in the run's sp-play-test.log and the world's logs in this run's folder; tell Claude the line.",
    "play": "Tell Claude this line (app.rs world_play).",
    "signed in by itself": "The game waited at its login screen: send the entering screenshot to Claude.",
    "in the world": "Look at the screenshots in this run's folder; tell Claude this line.",
    "companions": "Tell Claude this line (realm/custom/lua_scripts/ov_companions.lua, single player).",
    "levelup": "Tell Claude this line (the companions' level keeping, or the world's console).",
    "stoponexit": "Tell Claude this line (app.rs stop_world_when_closed).",
    "repair": "Tell Claude this line (host/engine.rs begin_repair).",
    "clean up": "Stop the world from the launcher's Single player page if it still runs; tell Claude this line.",
    "in the world": "Look at the screenshots in this run's folder; tell Claude this line.",
    "bossstart": "Send the in-front-of-the-boss screenshot and this line to Claude (the module's start, BossfightsStarts.h).",
    "at the start": "Send the in-front-of-the-boss screenshot and this line to Claude.",
    "bossfight": "Send the fighting and fight-end screenshots and this line to Claude (bossrush.rs, the module's run).",
}


# ------------------------------------------------------------------------------------------------ where things are
def game_folder(arg, owner: Path, root: Path = ROOT):
    """(folder, where from): --game, else your launcher's 3.3.5a game folder (read only), else folder.txt."""
    if arg:
        return Path(arg), "--game"
    w = fr.wrath_folder(owner)
    if w and w.get("path"):
        return Path(w["path"]), "your launcher's game folders"
    try:
        line = (root / "tools" / "first-run" / "folder.txt").read_text(encoding="utf-8").strip().splitlines()[0].strip().strip('"')
        if line:
            return Path(line), "tools\\first-run\\folder.txt"
    except (OSError, IndexError):
        pass
    return None, ""


def _ci(d: Path, name: str) -> Path:
    try:
        for e in os.listdir(d):
            if e.lower() == name.lower():
                return d / e
    except OSError:
        pass
    return d / name


def touched_files(game: Path) -> list:
    """The files a Play may change in the game folder: first-run's (Config.wtf, the realmlists, patch-Z) and Leyline's
    other patch archives (patch-S, -T, -Y: Your Den's ground, the race and class tables)."""
    out = list(fr.game_files(game))
    data = _ci(game, "Data")
    for letter in "STY":
        out.append(_ci(data, "patch-%s.MPQ" % letter))
    try:
        for e in sorted(os.listdir(data)):
            if len(e) == 4 and e.isalpha() and (data / e).is_dir():
                for letter in "STY":
                    out.append(_ci(data / e, "patch-%s-%s.MPQ" % (e, letter)))
    except OSError:
        pass
    seen, uniq = set(), []
    for p in out:
        k = os.path.normcase(str(p))
        if k not in seen:
            seen.add(k)
            uniq.append(p)
    return uniq


def keep_aside(paths, dest: Path) -> dict:
    """Each file copied to dest (on disk: the archives can be large). {path: copy or None (it wasn't there)}."""
    dest.mkdir(parents=True, exist_ok=True)
    out = {}
    for i, p in enumerate(paths):
        if p.is_file():
            c = dest / ("%02d-%s" % (i, p.name))
            shutil.copy2(p, c)
            out[p] = c
        else:
            out[p] = None
    return out


def _same(a: Path, b: Path) -> bool:
    try:
        if a.stat().st_size != b.stat().st_size:
            return False
        return fr.sha(a) == fr.sha(b)
    except OSError:
        return False


def put_back(saved: dict) -> list:
    """Every file as it was (None: it wasn't there, so it goes). The ones that couldn't be put back."""
    bad = []
    for p, c in saved.items():
        try:
            if c is None:
                if p.exists():
                    p.unlink()
            elif not p.is_file() or not _same(p, c):
                shutil.copy2(c, p)
        except OSError:
            bad.append(str(p))
    return bad


def names_in(d: Path) -> set:
    try:
        return set(os.listdir(d))
    except OSError:
        return set()


def new_dirs(game: Path) -> dict:
    """The folders a Play may add to (each one's names now): WTF\\Account (the login's settings) and Leyline\\."""
    return {d: names_in(d) for d in (_ci(_ci(game, "WTF"), "Account"), _ci(game, "Leyline"))}


def remove_new(before: dict) -> list:
    """What the test added there taken away (only names that weren't there before)."""
    gone = []
    for d, had in before.items():
        for n in sorted(names_in(d) - had):
            p = d / n
            try:
                shutil.rmtree(p) if p.is_dir() else p.unlink()
                gone.append(str(p))
            except OSError:
                pass
        if not had and d.is_dir() and not names_in(d):
            try:
                d.rmdir()
            except OSError:
                pass
    return gone


def sha256_line(text: str):
    """The hex of a `sha256sum` line ("<hex>  Leyline.exe"), or None."""
    m = re.match(r"\s*([0-9a-fA-F]{64})\b", text or "")
    return m.group(1).lower() if m else None


def build_cmd(exe: Path, game: Path, kind: str, first: bool, a) -> list:
    """The launcher's command line: never a key or a password (those go through the environment)."""
    cmd = [str(exe), "play-test"]
    if first:
        cmd.append("--fresh")
    if kind == "bossrush":
        return cmd + ["--game", str(game), "--bossrush", "--boss", a.boss, "--size", str(a.size), "--diff", a.diff, "--hero", a.hero,
                      "--hold", str(max(8, min(int(a.hold), 600))), "--fight-max", str(max(60, min(int(a.fight_max), 3600))),
                      "--checks", a.checks or BR_CHECKS]
    cmd += ["--game", str(game), "--single", kind, "--hold", str(max(8, min(int(a.hold), 600))),
            "--level", str(max(2, min(int(a.level), 80))), "--checks", a.checks]
    return cmd


def kind_list(a) -> list:
    if getattr(a, "bossrush", False):
        return ["bossrush"]
    return list(KINDS) if a.both else [a.kind]


def boss(a) -> bool:
    return bool(getattr(a, "bossrush", False))


# ------------------------------------------------------------------------------------------------ the outside world
class Real(fr.Real):
    """The PC and the network (fr.Real's downloads and website, plus this test's own). The tests swap it for a fake."""

    windows = os.name == "nt"

    def port_busy(self, port):
        s = socket.socket()
        s.settimeout(0.5)
        try:
            return s.connect_ex(("127.0.0.1", port)) == 0
        finally:
            s.close()

    def free_gb(self, path: Path):
        p = path
        while not p.exists() and p != p.parent:
            p = p.parent
        try:
            return shutil.disk_usage(p).free / 1e9
        except OSError:
            return None

    def spawn_logged(self, cmd, env, cwd, out: Path):
        f = open(out, "ab")
        try:
            return subprocess.Popen(cmd, env=env, cwd=cwd, stdout=f, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        finally:
            f.close()

    def kill_under(self, folder: Path) -> list:
        """Every program running from inside the test folder stopped (the test's own world: its database, login and
        world servers). Nothing else. The ids stopped."""
        if os.name != "nt":
            return []
        root = str(folder).replace("'", "''")
        ps = ("$r='%s'; Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith($r, [StringComparison]::OrdinalIgnoreCase) }"
              " | ForEach-Object { $_.Id; Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }") % root
        try:
            r = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps], capture_output=True,
                               text=True, timeout=60)
            return [x.strip() for x in (r.stdout or "").splitlines() if x.strip().isdigit()]
        except (OSError, subprocess.TimeoutExpired):
            return []

    def close_game(self, pid: int, game: Path) -> bool:
        """The game this test started closed: only that process id, and only while it is a program of the game folder."""
        if os.name != "nt" or not pid:
            return False
        g = str(game).replace("'", "''")
        ps = ("$p = Get-Process -Id %d -ErrorAction SilentlyContinue; if ($p -and $p.Path -and $p.Path.StartsWith('%s', "
              "[StringComparison]::OrdinalIgnoreCase)) { Stop-Process -Id %d -Force; 'closed' }") % (int(pid), g, int(pid))
        try:
            r = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps], capture_output=True,
                               text=True, timeout=60)
            return "closed" in (r.stdout or "")
        except (OSError, subprocess.TimeoutExpired):
            return False

    def now(self):
        return time.monotonic()


# ------------------------------------------------------------------------------------------------ the run
def run(a, io=None, lines=None, owner=None, owners=None, root=None, stamp=None) -> int:
    io = io or Real()
    ln = lines or Lines()
    root = Path(root) if root else ROOT
    stamp = stamp or time.strftime("%Y%m%d-%H%M%S")
    out = root / "logs" / ("boss-playtest" if boss(a) else "sp-playtest") / stamp
    out.mkdir(parents=True, exist_ok=True)
    owner = Path(owner) if owner else fr.owner_app_dir()
    owners = owner_dirs() if owners is None else owners
    print("%s (%s)" % (title(a), time.strftime("%Y-%m-%d %H:%M")), flush=True)
    st8 = {"tmp": None, "saved": {}, "dirs": {}, "token": "", "pids": set(), "game": None, "reports": {},
           "owner_before": owner_state(owner)}

    def body():
        if not io.windows:
            ln.add(False, "this PC", "not Windows", "The single player engine is a Windows build: run sp-playtest on the PC.")
            return
        game, src = game_folder(a.game, owner, root)
        if not ln.add(game is not None and _ci(game, "Data").is_dir(), "your Wrath game folder", "%s (%s)" % (game, src) if game else "none found",
                      FIX["game folder"]):
            return
        st8["game"] = game
        base = Path(a.work) if a.work else Path(tempfile.gettempdir())
        tmp = base / ("leyline-sp-playtest-%s" % stamp)
        why = refuse_reason(tmp, list(owners) + [game])
        if not ln.add(not why, "a fresh test folder (none of your own data)", why or str(tmp), "Pass --work <an empty folder of its own>."):
            return
        busy = [n for n, p in PORTS.items() if io.port_busy(p)]
        if not ln.add(not busy, "the local world's ports are free", ("in use: " + ", ".join("%s %d" % (n, PORTS[n]) for n in busy)) if busy else "",
                      "Your own Single player world (or another program) is using them: stop it in the launcher, then run again."):
            return
        free = io.free_gb(base)
        if not ln.add(free is None or free >= NEED_GB, "room for a fresh world", "%.1f GB free, %d needed" % (free, NEED_GB) if free is not None else "free space unknown",
                      FIX["preflight"]):
            return
        tmp.mkdir(parents=True)
        st8["tmp"] = tmp
        # the test build of the launcher
        exe = tmp / "launcher" / "Leyline.exe"
        try:
            io.fetch(EXE_URL, exe)
            raw = exe.read_bytes()
        except (OSError, urllib.error.URLError, ValueError) as e:
            ln.add(False, "the test launcher downloaded", type(e).__name__, "Is the launcher-test prerelease there? Tell Claude this line.")
            return
        st, sums = io.get_text(SHA_URL)
        want, got = sha256_line(sums) if st == 200 else None, hashlib.sha256(raw).hexdigest()
        if not ln.add(bool(want) and want == got, "the test launcher matches its .sha256", "%.1f MB%s" % (len(raw) / 1e6, "" if want else ", no .sha256 (HTTP %s)" % st),
                      "Download again (a new build may have landed meanwhile), or tell Claude this line."):
            return
        if not ln.add(SP_MARK in raw, "the test launcher has the single player playtest", "", "The launcher-test build is older: tell Claude."):
            return
        if boss(a) and not ln.add(BR_MARK in raw, "the test launcher has the Boss Rush playtest", "",
                                  "The launcher-test build is older than start anywhere: a new test build is needed (tell Claude)."):
            return
        st, info = io.get_text(INFO_URL, 4096)
        try:
            meta = json.loads(info) if st == 200 else {}
        except ValueError:
            meta = {}
        ln.add(None, "built from", "%s %s" % (meta.get("branch", "?"), str(meta.get("commit", "?"))[:10]))
        # the test Hero Key
        key = ""
        if boss(a):
            ln.add(None, "test Hero Key", "not needed: Boss Rush plays on its own hero login")
        elif a.no_key:
            ln.add(None, "test Hero Key", "--no-key: the world's own login (as offline)")
        else:
            st, d = io.http("POST", "account/new", js={"audit": True})
            st8["token"], key = str(d.get("token") or ""), str(d.get("key") or "")
            ln.secret(st8["token"])
            ln.secret(key)
            if st == 200 and key.startswith("ovk1."):
                ln.add(None, "test Hero Key", "a test account from the website (closed at the end)")
            else:
                key = ""
                ln.add(None, "test Hero Key", "the website said HTTP %s: the world's own login (as offline)" % st)
        # the game folder's files aside
        st8["saved"] = keep_aside(touched_files(game), tmp / "game-files")
        st8["dirs"] = new_dirs(game)
        ln.add(None, "your game folder's files kept aside", "%d there" % sum(1 for c in st8["saved"].values() if c))
        data, webview = tmp / "data", tmp / "webview"
        data.mkdir()
        webview.mkdir()
        deadline = io.now() + max(10, int(a.timeout)) * 60
        for n, kind in enumerate(kind_list(a)):
            if not one_run(a, io, ln, exe, game, kind, n == 0, data, webview, key, out, st8, deadline):
                break

    try:
        body()
    finally:
        cleanup(a, io, ln, st8, out)
        after = owner_state(owner)
        ln.add(after == st8.get("owner_before", after), "your own launcher untouched", "settings and sign-in as before",
               "Your own launcher's settings changed during the test: tell Claude this line.")
    return finish(ln, out, root)


def title(a) -> str:
    return "BOSS RUSH PLAYTEST" if boss(a) else "SINGLE PLAYER PLAYTEST"


def owner_state(owner: Path) -> dict:
    return {str(p): fr.sha(p) for p in (owner / "settings.json", owner / "social-session.json")}


def one_run(a, io, ln, exe, game, kind, first, data, webview, key, out, st8, deadline) -> bool:
    """One `play-test --single <kind>`: started, followed (its lines, the screenshots at its phases), its report read."""
    env = dict(os.environ)
    env.update({"LEYLINE_APP_DIR": str(data), "WEBVIEW2_USER_DATA_FOLDER": str(webview)})
    env.pop("LEYLINE_EXE", None)
    env.pop(KEY_ENV, None)
    if key:
        env[KEY_ENV] = key
    cmd = build_cmd(exe, game, kind, first, a)
    cli_out = st8["tmp"] / ("cli-%s.txt" % kind)
    report_path = data / (BR_REPORT if kind == "bossrush" else REPORT)
    shots_at, world_at = (BR_SHOTS, BR_WORLD_PHASES) if kind == "bossrush" else (SHOTS, WORLD_PHASES)
    if report_path.exists():
        report_path.unlink()
    proc = io.spawn_logged(cmd, env, str(exe.parent), cli_out)
    env.pop(KEY_ENV, None)
    ln.add(None, "%s: started" % kind, " ".join(cmd[1:]))
    seen, said, shots, next_shot, pid, rep = "", 0, 0, 0.0, 0, {}
    timed_out = False
    while True:
        rep = fr.read_json(report_path) or rep
        phase = rep.get("phase", "")
        if rep.get("pid"):
            pid = int(rep["pid"])
            st8["pids"].add(pid)
        said = echo_lines(ln, cli_out, said)
        if phase and phase != seen:
            seen = phase
            print("     phase: %s" % phase, flush=True)
            if phase in shots_at and pid and shots < MAX_SHOTS:
                ok, _ = io.screenshot(pid, out / ("%02d-%s-%s.png" % (shots, kind, shots_at[phase])))
                shots += 1 if ok else 0
                next_shot = io.now() + SHOT_EVERY_S
        elif phase in world_at and pid and io.now() >= next_shot and shots < MAX_SHOTS:
            ok, _ = io.screenshot(pid, out / ("%02d-%s-%s.png" % (shots, kind, phase.replace("_", "-"))))
            shots += 1 if ok else 0
            next_shot = io.now() + SHOT_EVERY_S
        if proc.poll() is not None:
            break
        if io.now() > deadline:
            timed_out = True
            proc.kill()
            break
        io.sleep(2)
    try:
        code = proc.wait(timeout=60)
    except subprocess.TimeoutExpired:
        proc.kill()
        code = -1
    echo_lines(ln, cli_out, said)
    rep = fr.read_json(report_path) or rep
    st8["reports"][kind] = rep
    if rep.get("pid"):
        st8["pids"].add(int(rep["pid"]))
    save_report(ln, rep, out / ("%s-%s" % (kind, BR_REPORT if kind == "bossrush" else REPORT)))
    for src, name in ((cli_out, "%s-lines.txt" % kind),):
        copy_hidden(ln, src, out / name)
    if timed_out:
        ln.add(False, "%s: finished in time" % kind, "stopped after %d minutes" % int(a.timeout), "Run again with --timeout <more minutes>, or tell Claude.")
    if rep.get("mark") != SP_MARK.decode():
        ln.add(False, "%s: the launcher's report" % kind, "none (exit %s)" % code, "Read %s-lines.txt in this run's folder; tell Claude." % kind)
        return False
    for s in rep.get("steps") or []:
        name = str(s.get("name", "?"))
        ok = s.get("ok")
        ln.add(ok if ok in (True, False) else None, "%s: %s" % (kind, name), "%s (%.0f s)" % (s.get("why", ""), (s.get("ms") or 0) / 1000),
               FIX.get(name, "Tell Claude this line."))
    ln.add(shots > 0, "%s: screenshots of the game" % kind, "%d" % shots, "Screenshots need Windows and the game window: see den-test's notes.")
    ln.add(code == 0, "%s: exit code" % kind, str(code), "See the lines above.")
    return code == 0


def echo_lines(ln, f: Path, at: int) -> int:
    """The launcher's new lines printed (secrets hidden); where it read to."""
    try:
        with open(f, "rb") as h:
            h.seek(at)
            chunk = h.read()
    except OSError:
        return at
    if chunk:
        for line in chunk.decode("utf-8", "replace").splitlines():
            if line.strip():
                print("     " + ln.hide(fr.plain(line, 400)), flush=True)
    return at + len(chunk)


def save_report(ln, rep: dict, dest: Path):
    if rep:
        safe = {k: v for k, v in rep.items() if k not in ("pass", "password", "key", "token")}
        dest.write_text(ln.hide(json.dumps(safe, indent=2)), encoding="utf-8")


def copy_hidden(ln, src: Path, dest: Path, tail=400_000):
    """A text file copied with every secret hidden (its last `tail` bytes)."""
    try:
        b = src.read_bytes()
    except OSError:
        return False
    dest.write_text(ln.hide(b[-tail:].decode("utf-8", "replace")), encoding="utf-8")
    return True


def world_logs(ln, tmp: Path, out: Path) -> int:
    """The test worlds' own logs (setup, the servers) into the run's folder, secrets hidden."""
    n = 0
    data = tmp / "data"
    for logdir in sorted(data.glob("engine*/logs")):
        for f in sorted(logdir.glob("*.log"))[:20]:
            if copy_hidden(ln, f, out / ("world-%s-%s" % (logdir.parent.name, f.name))):
                n += 1
    if copy_hidden(ln, data / CLI_LOG, out / CLI_LOG):
        n += 1
    return n


def cleanup(a, io, ln, st8, out: Path):
    tmp, game = st8.get("tmp"), st8.get("game")
    for pid in sorted(st8["pids"]):
        if game and io.close_game(pid, game):
            ln.add(None, "the test's game closed", "pid %d (it was still open)" % pid)
    if tmp:
        stopped = io.kill_under(tmp)
        ln.add(None if not stopped else True, "the test's world stopped", "%d program(s) of the test folder stopped" % len(stopped) if stopped else "nothing left running")
        world_logs(ln, tmp, out)
    if st8.get("saved"):
        bad = put_back(st8["saved"])
        ln.add(not bad, "your game folder put back (Config.wtf, realmlist, Leyline's archives)", ", ".join(bad),
               "Close the game, then run the test again (it puts them back first).")
    if st8.get("dirs"):
        gone = remove_new(st8["dirs"])
        ln.add(None, "the test's login folders taken out of your game folder", ", ".join(Path(g).name for g in gone) or "none made")
    if st8.get("token"):
        st, d = io.http("POST", "audit/close", js={}, token=st8["token"])
        ln.add(st == 200, "the test account closed", "" if st == 200 else "HTTP %s %s" % (st, d.get("error", "")),
               "Nothing to do: test accounts close themselves within a day.")
    if tmp:
        if a.keep:
            ln.add(None, "the test folder kept (--keep)", str(tmp))
        else:
            for _ in range(6):
                shutil.rmtree(tmp, ignore_errors=True)
                if not tmp.exists():
                    break
                io.sleep(5)
            ln.add(not tmp.exists(), "the test folder removed", str(tmp), "Delete it by hand (a program may have held a file).")


def finish(ln, out: Path, root: Path) -> int:
    text = ln.text()
    leaked = any(s in text for s in ln.secrets) or "ovk1." in text
    ln.add(not leaked, "no secrets in the log", "", "Tell Claude this line.")
    for f in out.glob("*"):
        if f.is_file() and f.suffix in (".txt", ".json", ".log"):
            body = f.read_text(encoding="utf-8", errors="replace")
            if any(s in body for s in ln.secrets) or "ovk1." in body:
                f.write_text(ln.hide(body).replace("ovk1.", "(hidden)"), encoding="utf-8")
    verdict = "FAIL" if ln.failed() else "PASS"
    rows = [r for r in ln.rows if r[0] in ("PASS", "FAIL")]
    table = "\n".join("%-4s  %s" % (k, w) for k, w, _d, _f in rows)
    name = "BOSS RUSH PLAYTEST" if out.parent.name == "boss-playtest" else "SINGLE PLAYER PLAYTEST"
    full = "%s (%s)\n%s\n%s\n" % (name, time.strftime("%Y-%m-%d %H:%M"), ln.hide(ln.text()), verdict)
    (out / ("boss-playtest.txt" if out.parent.name == "boss-playtest" else "sp-playtest.txt")).write_text(full, encoding="utf-8")
    (out.parent / "latest.txt").write_text(full + "Folder: %s\n" % out, encoding="utf-8")
    print("\n%s\n%s" % (table, verdict), flush=True)
    print("Saved %s" % out, flush=True)
    return 0 if verdict == "PASS" else 1


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "run":
        argv.pop(0)
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--kind", choices=KINDS, default="single", help="which world (default: single)")
    ap.add_argument("--both", action="store_true", help="single, then the original world (vanilla) on the same engine and maps")
    ap.add_argument("--game", help="your Wrath game folder (default: the one in your launcher)")
    ap.add_argument("--hold", type=int, default=20, help="seconds in the world before the checks (screenshots)")
    ap.add_argument("--level", type=int, default=10, help="the level the levelup check goes to")
    ap.add_argument("--checks", default=None, help="which checks (default: all four; with --bossrush both of its own)")
    ap.add_argument("--bossrush", action="store_true", help="Boss Rush instead: start anywhere and a full boss fight")
    ap.add_argument("--boss", default="frozen-throne", help="with --bossrush: the boss, wing or whole raid to start at")
    ap.add_argument("--size", type=int, choices=(10, 25), default=10, help="with --bossrush: 10 or 25 players")
    ap.add_argument("--diff", choices=("normal", "heroic"), default="normal", help="with --bossrush: the difficulty")
    ap.add_argument("--hero", default="mage-fire", help="with --bossrush: the hero, <class>-<spec>")
    ap.add_argument("--fight-max", type=int, default=1500, help="with --bossrush: seconds the fight is watched at most")
    ap.add_argument("--no-key", action="store_true", help="no test Hero Key: the world's own login")
    ap.add_argument("--keep", action="store_true", help="keep the test folder")
    ap.add_argument("--work", help="where the test folder goes (default: the system's temp folder)")
    ap.add_argument("--timeout", type=int, default=TIMEOUT_MIN, help="minutes for all of it")
    a = ap.parse_args(argv)
    if a.checks is None:
        a.checks = BR_CHECKS if a.bossrush else CHECKS
    if not re.fullmatch(r"[a-z0-9-]+", a.boss or "") or not re.fullmatch(r"[a-z]+-[a-z]+", a.hero or ""):
        ap.error("--boss and --hero: words, digits and dashes only")
    if not re.fullmatch(r"[a-z,]+", a.checks or ""):
        ap.error("--checks: words and commas only")
    for f in (sys.stdout, sys.stderr):
        try:
            f.reconfigure(errors="replace")
        except (AttributeError, ValueError):
            pass
    return run(a)


if __name__ == "__main__":
    sys.exit(main())
