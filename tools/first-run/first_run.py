"""First-run test: today's launcher download on an empty data folder, through to in the game (docs/FIRST-RUN-TEST.md,
owner 3 Nov: "everything makes sense on new machine"; handoff section 8 item 2: "fresh install, through to in the game"
proven).

    python -I tools/first-run/first_run.py [--hold 150] [--no-den] [--keep]

What it does, on the owner's PC (Control step first-run-test), PASS / FAIL per phase with the fix:
  1. download: the newest Leyline.exe into .launcher\\first-run (a new copy, not the owner's .launcher\\Leyline.exe);
     over 1 MB, a Windows program, with play-test in it;
  2. fresh folders: %TEMP%\\leyline-first-run-<time>\\data and \\webview (LEYLINE_APP_DIR, WEBVIEW2_USER_DATA_FOLDER):
     nothing of %APPDATA%\\Leyline is read but the game folder's path, and nothing there is written (hashed before
     and after);
  3. a test account from the website's own test-account API (account/new {audit: true}); its key goes only into the
     launcher's environment (LEYLINE_TEST_HEROKEY), never a command line, a line here or a file;
  4. a test character on it (the headless player, tools/audit/play335.py, with the account's one-start game login);
  5. `Leyline.exe play-test --official --fresh --game <your Wrath folder> --world --hold <s>`: the folder added, the key
     signed in, PLAY, the game signs in by itself; the website's audit/presence says when it is in the world;
     screenshots of the game window meanwhile;
  6. the Den test (tools/den-check den_check.py test) on the same fresh data folder and download;
  7. clean up, always: audit/close (the account and its sign-ins gone), the temp folders removed, the game folder's
     Config.wtf, realmlist and login archive put back as they were.

Logs: tools\\first-run\\logs\\first-run-latest.txt, play-test.json (no secrets), NN-*.png. Standard library only.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
LOGS = HERE / "logs"
EXE_URL = "https://github.com/orionopenwow/Leyline/releases/download/launcher-latest/Leyline.exe"
SITES = ("https://leylinelauncher.com", "https://play.openvanilla.app")
REALMLISTS = ("leylinelauncher.com", "play.openvanilla.app")
API = "/wrath/api/social/"
CLI_MARK = b"leyline-play-test-v1"
DEN_MARK = b"leyline-den-play-v1"
REPORT = "play-test.json"
KEY_ENV = "LEYLINE_TEST_HEROKEY"
UA = "LeylineFirstRun/1"
RACE, CLASS = 1, 1            # human warrior (docs/FIRST-RUN-TEST.md 3.3)

sys.path.insert(0, str(ROOT / "tools" / "den-check"))


def plain(t, n=300):
    return "".join(c for c in str(t) if c.isprintable() and c != "﻿")[:n]


class Lines:
    """PASS / FAIL / INFO lines; every line goes through hide() so a secret can never be printed."""

    def __init__(self, out=print):
        self.rows, self.secrets, self.out = [], [], out

    def secret(self, s):
        if s and isinstance(s, str) and len(s) >= 4:
            self.secrets.append(s)

    def hide(self, text):
        text = str(text)
        for s in self.secrets:
            text = text.replace(s, "(hidden)")
        return text

    def add(self, ok, what, detail="", fix=""):
        kind = "PASS" if ok is True else ("FAIL" if ok is False else "INFO")
        detail = self.hide(plain(detail))
        fix = self.hide(plain(fix)) if kind == "FAIL" else ""
        self.rows.append((kind, what, detail, fix))
        self.out("%-4s %s%s%s" % (kind, what, (": " + detail) if detail else "", ("  -> " + fix) if fix else ""))
        return ok

    def failed(self):
        return any(r[0] == "FAIL" for r in self.rows)

    def text(self):
        return "\n".join("%-4s %s%s%s" % (k, w, (": " + d) if d else "", ("  -> " + f) if f else "") for k, w, d, f in self.rows) + "\n"


def sha(p: Path):
    try:
        return hashlib.sha256(p.read_bytes()).hexdigest()
    except OSError:
        return None


def owner_app_dir() -> Path:
    """The owner's own launcher data folder (never LEYLINE_APP_DIR: this tool sets that only for its child)."""
    if os.name == "nt":
        return Path(os.environ.get("APPDATA", str(Path.home()))) / "Leyline"
    return Path(os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local" / "share"))) / "Leyline"


def wrath_folder(app: Path):
    """The owner's first 3.3.5a game folder from his own settings (read only): {name, path} or None."""
    try:
        s = json.loads((app / "settings.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    cl = [c for c in s.get("clients", []) if isinstance(c, dict) and c.get("build") == "3.3.5a" and c.get("path")]
    sel = s.get("selected_client")
    return next((c for c in cl if c.get("id") == sel), cl[0] if cl else None)


def game_files(game: Path):
    """The files PLAY may change in the game folder (put back after): Config.wtf, each realmlist.wtf, patch-Z.MPQ."""
    def ci(d: Path, name: str):
        try:
            for e in os.listdir(d):
                if e.lower() == name.lower():
                    return d / e
        except OSError:
            pass
        return d / name
    wtf = ci(game, "WTF")
    data = ci(game, "Data")
    out = [ci(wtf, "Config.wtf"), ci(data, "patch-Z.MPQ"), ci(data, "realmlist.wtf"), ci(game, "realmlist.wtf")]
    try:
        for e in sorted(os.listdir(data)):
            if len(e) == 4 and e[:2].isalpha() and e[2:].isalpha() and (data / e).is_dir():   # a locale folder: enUS
                out.append(ci(data / e, "realmlist.wtf"))
                out.append(ci(data / e, "realmlist.wtf").with_name("patch-" + e + "-Z.MPQ"))
    except OSError:
        pass
    return out


def stash(paths):
    out = {}
    for p in paths:
        try:
            out[p] = p.read_bytes()
        except OSError:
            out[p] = None
    return out


def unstash(saved) -> list:
    """Put each file back as it was (None: it wasn't there: removed). The paths that couldn't be put back."""
    bad = []
    for p, data in saved.items():
        try:
            if data is None:
                if p.exists():
                    p.unlink()
            else:
                if not p.exists() or p.read_bytes() != data:
                    p.write_bytes(data)
        except OSError:
            bad.append(str(p))
    return bad


# ---------------------------------------------------------------- the outside world (each one replaced by a fake in the tests)
class Real:
    def __init__(self):
        self.site = None

    def http(self, method, path, js=None, token=""):
        """(status, json dict) from the website; tries the new name, then the old. (0, {}) when nothing answers."""
        sites = [self.site] if self.site else list(SITES)
        last = (0, {})
        for base in sites:
            h = {"User-Agent": UA, "Accept": "application/json"}
            data = None
            if js is not None:
                data, h["Content-Type"] = json.dumps(js).encode(), "application/json"
                h["Sec-Fetch-Site"] = "same-origin"
            if token:
                h["Authorization"] = "Bearer " + token
            req = urllib.request.Request(base + API + path, data=data, method=method, headers=h)
            try:
                r = urllib.request.urlopen(req, timeout=30)
            except urllib.error.HTTPError as e:
                r = e
            except (urllib.error.URLError, OSError, ValueError):
                continue
            try:
                body = json.loads(r.read(1 << 20) or b"{}")
            except (OSError, ValueError):
                body = {}
            self.site = base
            return (r.status if hasattr(r, "status") else r.code), (body if isinstance(body, dict) else {})
        return last

    def fetch(self, url, dest: Path):
        dest.parent.mkdir(parents=True, exist_ok=True)
        tmp = dest.with_name(dest.name + ".new")
        req = urllib.request.Request(url, headers={"User-Agent": UA})
        with urllib.request.urlopen(req, timeout=300) as r, open(tmp, "wb") as f:
            shutil.copyfileobj(r, f)
        os.replace(tmp, dest)

    def spawn(self, cmd, env, cwd):
        return subprocess.Popen(cmd, env=env, cwd=cwd)

    def screenshot(self, pid, out: Path):
        import den_check as dc
        return dc.screenshot(pid, out)

    def den_test(self, exe: Path, data: Path, env):
        r = subprocess.run([sys.executable, "-I", str(ROOT / "tools" / "den-check" / "den_check.py"), "test", "--app-dir", str(data)],
                           env=env, cwd=str(ROOT), capture_output=True, text=True, timeout=1200, errors="replace")
        return r.returncode, (r.stdout or "") + (r.stderr or "")

    def make_character(self, user, password, name):
        """A character on the official realm with the account's game login (the headless player). (ok, detail)."""
        import asyncio
        sys.path.insert(0, str(ROOT / "tools" / "audit"))
        sys.path.insert(0, str(ROOT / "hosting" / "kit" / "tools"))
        import play335 as p
        from wunet import wire335 as w

        async def go():
            last = ""
            for host in REALMLISTS:
                try:
                    s, realm, _ = await p.connect(host, 3724, user, password, realm_pick=lambda r: "wrath" in r["name"].lower())
                except Exception as e:   # noqa: BLE001: the next name
                    last = "%s: %s" % (host, type(e).__name__)
                    continue
                try:
                    chars = await s.char_enum()
                    if chars:
                        return True, "%s already has one" % realm["name"]
                    code, tried = 0, name
                    for n in range(4):   # a name in use (0x31, 0x32): the same with other letters on the end
                        tried = name if n == 0 else name[:9] + "".join(random.choice("aeioulnrst") for _ in range(3))
                        code = await s.char_create(tried, RACE, CLASS)
                        if code not in (0x31, 0x32):
                            break
                    return code == w.CHAR_CREATE_SUCCESS, "%s: character create answered %#x" % (tried, code)
                finally:
                    s.close()
            return False, last or "no login server answered"
        return asyncio.run(go())

    def sleep(self, s):
        time.sleep(s)

    def now(self):
        return time.time()


def char_name(made: str) -> str:
    """A character name from the account's made-up name: letters only, 2 to 12, first upper."""
    letters = "".join(c for c in (made or "") if c.isascii() and c.isalpha())[:12] or "Firstrun"
    return letters[:1].upper() + letters[1:].lower() if len(letters) >= 2 else "Firstrun"


# ---------------------------------------------------------------- the run
def run(a, io=None, lines=None, temp_base=None, app_owner=None, root=None) -> int:
    io = io or Real()
    ln = lines or Lines()
    root = Path(root) if root else ROOT
    logs = root / "tools" / "first-run" / "logs"
    if logs.exists():
        shutil.rmtree(logs, ignore_errors=True)
    logs.mkdir(parents=True, exist_ok=True)
    print("FIRST-RUN TEST (%s)" % time.strftime("%Y-%m-%d %H:%M"), flush=True)
    owner = Path(app_owner) if app_owner else owner_app_dir()
    watch = [owner / "settings.json", owner / "social-session.json"]
    before = {str(p): sha(p) for p in watch}
    tmp = Path(tempfile.mkdtemp(prefix="leyline-first-run-", dir=temp_base))
    data, webview = tmp / "data", tmp / "webview"
    st8 = {"token": "", "saved": {}, "report": {}}

    def body():
        # 1. the download
        exe = root / ".launcher" / "first-run" / ("Leyline.exe")
        try:
            io.fetch(EXE_URL, exe)
            raw = exe.read_bytes()
        except (OSError, urllib.error.URLError, ValueError) as e:
            raw = b""
            ln.add(False, "1. the download", type(e).__name__, "Check the internet connection, then run first-run-test again.")
        if raw:
            good = len(raw) > 1_000_000 and raw[:2] == b"MZ"
            ln.add(good, "1. the download", "%.1f MB" % (len(raw) / 1e6), "The release file looks wrong: check the leyline launcher build on GitHub.")
            if not ln.add(CLI_MARK in raw, "1. the download has play-test", "", "Ship, wait for the leyline launcher build, then run first-run-test again."):
                return
        else:
            return

        # 2. fresh folders
        data.mkdir(parents=True)
        webview.mkdir(parents=True)
        ln.add(not any(data.iterdir()), "2. fresh start: an empty data folder", str(tmp))

        # the owner's game folder (read only from his settings)
        game = wrath_folder(owner)
        if not ln.add(game is not None, "3. your Wrath game folder", game and game.get("path") or "",
                      "Add your Wrath folder in your own launcher first (Game folders), then run first-run-test again."):
            return
        gpath = Path(game["path"])
        st8["saved"] = stash(game_files(gpath))

        # 4. a test account (the website's own test-account API)
        st, d = io.http("POST", "account/new", js={"audit": True})
        token, key = str(d.get("token") or ""), str(d.get("key") or "")
        st8["token"] = token
        ln.secret(token)
        ln.secret(key)
        me = d.get("me") or {}
        made = "%s#%s" % (me.get("name", ""), me.get("tag", "")) if me.get("tag") else me.get("name", "")
        if not ln.add(st == 200 and bool(token) and key.startswith("ovk1."), "4. a test account", made or "HTTP %s" % st,
                      "Too many new accounts this hour (3 a network) or the website is down: wait an hour, or run check."):
            return

        # 5. a test character (its one-start game login: never printed)
        st, g = io.http("POST", "game/login", js={}, token=token)
        user, pw = str(g.get("user") or ""), str(g.get("pass") or "")
        ln.secret(pw)
        if st == 200 and user and pw:
            name = char_name(me.get("name", ""))
            try:
                ok, why = io.make_character(user, pw, name)
            except Exception as e:   # noqa: BLE001
                ok, why = False, type(e).__name__
            ln.add(ok, "5. a test character", "%s, %s" % (name, why), "The realm may be down or full: run check, then try again.")
        else:
            ln.add(False, "5. a test character", "game/login HTTP %s %s" % (st, g.get("error", "")), "Run signin (the website's sign-in), then try again.")

        # 6. PLAY from the fresh launcher
        env = dict(os.environ)
        env.update({"LEYLINE_APP_DIR": str(data), "WEBVIEW2_USER_DATA_FOLDER": str(webview), KEY_ENV: key})
        env.pop("LEYLINE_EXE", None)
        hold = max(30, min(int(a.hold), 600))
        cmd = [str(exe), "play-test", "--official", "--fresh", "--game", str(gpath), "--world", "--hold", str(hold)]
        t0 = io.now()
        proc = io.spawn(cmd, env, str(exe.parent))
        env.pop(KEY_ENV, None)
        ln.add(None, "6. started", " ".join(cmd[1:]))
        pid, shots, seen, world, given, phase, pres = 0, 0, "", None, False, "", set()
        deadline = t0 + hold + 120
        next_shot = 0.0
        while io.now() < deadline:
            report = st8["report"] = read_json(data / REPORT) or {}
            phase = report.get("phase", "")
            if phase and phase != seen:
                seen = phase
                ln.add(None, "   launcher phase", "%s at %.0fs" % (phase, io.now() - t0))
            if report.get("pid") and not pid:
                pid = int(report["pid"])
            if pid and phase == "running":
                ps, pd = io.http("GET", "audit/presence", token=token)
                pres.add(ps)
                if ps == 200:
                    given = given or bool(pd.get("login_given"))
                    if pd.get("in_world") and not world:
                        world = {"character": pd.get("character"), "map": pd.get("map"), "at": io.now() - t0}
                if io.now() >= next_shot and shots < 40:
                    ok, _ = io.screenshot(pid, logs / ("%02d-%03ds.png" % (shots, int(io.now() - t0))))
                    shots += 1 if ok else 0
                    next_shot = io.now() + (3 if not world else 10)
            if proc.poll() is not None and phase == "done":
                break
            io.sleep(2)
        try:
            code = proc.wait(timeout=60)
        except subprocess.TimeoutExpired:
            proc.kill()
            code = -1
        report = st8["report"] = read_json(data / REPORT) or {}
        if pres and 200 not in pres:
            ln.add(None, "7. the website's presence check didn't answer", "HTTP %s" % ",".join(str(x) for x in sorted(pres)),
                   )
            if 404 in pres:
                ln.add(False, "7. the website has audit/presence", "404", "Run the signin step (it deploys herokeys.py), then run first-run-test again.")
        play_lines(ln, report, settings=read_json(data / "settings.json"), pid=pid, given=given, world=world, shots=shots, code=code)

        # 7. the Den test on the same fresh folder
        if a.no_den:
            ln.add(None, "8. Your Den test", "skipped (--no-den)")
        elif not (DEN_MARK in raw):
            ln.add(False, "8. Your Den test", "this launcher has no den-play", "Ship, wait for the build, then run again.")
        else:
            denv = dict(env)
            denv["LEYLINE_EXE"] = str(exe)
            dcode, dtext = io.den_test(exe, data, denv)
            last = [x for x in dtext.splitlines() if x.startswith(("FAIL", "PASS"))]
            ln.add(dcode == 0, "8. Your Den test (walk in, build, save, invite a friend)", (last[-1] if last else "exit %s" % dcode),
                   "Read tools\\den-check\\logs\\den-test-latest.txt.")
    try:
        body()
    finally:
        cleanup(ln, io, st8["token"], tmp, st8["saved"], a.keep)
        after = {str(p): sha(p) for p in watch}
        ln.add(after == before, "9. your own launcher untouched", "settings and sign-in the same as before",
               "Your own launcher's settings changed during the test: tell Claude this line.")
    return finish(ln, logs, st8["report"])


def read_json(p: Path):
    try:
        v = json.loads(p.read_text(encoding="utf-8"))
        return v if isinstance(v, dict) else None
    except (OSError, ValueError):
        return None


def play_lines(ln, r, settings, pid, given, world, shots, code):
    """The PASS / FAIL lines for play-test's report and what the website saw."""
    fr = r.get("fresh") or {}
    ln.add(bool(fr.get("asked")) and bool(fr.get("empty")), "6. the launcher started on an empty data folder", json.dumps(fr),
           "The launcher found settings in the new folder: tell Claude this line.")
    added = r.get("added") or {}
    ln.add(added.get("build") == "3.3.5a" and bool(added.get("complete")), "6. your game folder added (as Find your game files does)",
           "build %s, missing %s" % (added.get("build"), ", ".join(added.get("missing") or []) or "nothing"),
           "Your Wrath folder looks incomplete: open it in your own launcher, Game folders, Check.")
    clients = (settings or {}).get("clients") or []
    ln.add(len(clients) == 1, "6. the new launcher has exactly that one game folder", "%d" % len(clients), "Tell Claude this line.")
    ks = r.get("key_signin") or {}
    ln.add(bool(ks.get("ok")) and bool(r.get("signed_in")), "6. logged in with the Hero Key", ks.get("name", ""),
           "The website's Hero Key sign-in refused the key: run signin, then try again.")
    wl = r.get("website_login") or {}
    ln.add(bool(wl.get("ok")), "6. the website gave the game login for PLAY", wl.get("why", ""), "Run signin, then try again.")
    ln.add(bool(pid), "7. PLAY started the game", "pid %s" % pid if pid else r.get("error", "no game process"),
           "The game didn't start: run client-smoke and read its log.")
    hy, keys = r.get("hybrid") or {}, r.get("keys") or {}
    by_itself = bool(hy.get("ok")) or keys.get("typed") is True
    ln.add(by_itself, "7. the game logged in by itself", "login screen hand-over" if hy.get("ok") else ("typed by the launcher" if by_itself else hy.get("why", "")),
           "The login screen waited for typing: run client-smoke and send its screenshots.")
    ln.add(given or by_itself, "7. character select (the game got its login)", "" if given else "seen from the launcher's report only")
    ln.add(bool(world), "7. in the world", ("%s on map %s after %.0fs" % (world["character"], world["map"], world["at"])) if world else "not seen by the website",
           "The game stopped before the world: look at the last screenshots in tools\\first-run\\logs.")
    ln.add(shots > 0, "7. screenshots of the game", "%d" % shots, "Screenshots need Windows and the game window: see the den-test step's notes.")
    ln.add(bool(r.get("scrubbed")) and not r.get("session_left"), "7. the one-time login taken back out of the game", "",
           "A one-time login was left in the game folder's archive: run client-smoke once to clear it.")
    if r.get("error"):
        ln.add(False, "7. the launcher stopped", r["error"], "Tell Claude this line.")
    ln.add(code == 0, "7. play-test's exit code", str(code), "See the lines above.")


def cleanup(ln, io, token, tmp: Path, saved, keep: bool):
    if token:
        st, d = io.http("POST", "audit/close", js={}, token=token)
        ln.add(st == 200, "10. the test account closed", "" if st == 200 else "HTTP %s %s" % (st, d.get("error", "")),
               "Nothing to do: test accounts can only close themselves within a day; the next run makes a new one.")
        st2, _ = io.http("GET", "me", token=token)
        ln.add(st2 == 401, "10. its sign-in no longer works", "HTTP %s" % st2, "Tell Claude this line.")
    bad = unstash(saved) if saved else []
    ln.add(not bad, "10. your game folder put back (Config.wtf, realmlist, login archive)", ", ".join(bad),
           "Close the game, then run first-run-test again (it puts them back first).")
    if keep:
        ln.add(None, "10. the temp folders kept (--keep)", str(tmp))
    else:
        shutil.rmtree(tmp, ignore_errors=True)
        ln.add(not tmp.exists(), "10. the temp folders removed", str(tmp), "Delete it by hand.")


def finish(ln, logs: Path, report) -> int:
    if report:
        safe = {k: v for k, v in report.items() if k not in ("pass", "password", "key", "token")}
        (logs / REPORT).write_text(ln.hide(json.dumps(safe, indent=2)), encoding="utf-8")
    text = ln.text()
    leaked = any(s in text for s in ln.secrets) or "ovk1." in text
    if leaked:   # never: hide() runs on every line; this is the belt to its braces
        ln.add(False, "11. no secrets in the log", "", "Tell Claude this line.")
    else:
        ln.add(True, "11. no secrets in the log")
    verdict = "FAIL" if ln.failed() else "PASS"
    out = "FIRST-RUN TEST (%s)\n%s%s\n" % (time.strftime("%Y-%m-%d %H:%M"), ln.hide(ln.text()), verdict)
    (logs / "first-run-latest.txt").write_text(out, encoding="utf-8")
    print(verdict, flush=True)
    print("Saved %s" % (logs / "first-run-latest.txt"), flush=True)
    return 0 if verdict == "PASS" else 1


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--hold", type=int, default=150, help="seconds play-test keeps the game open (start, log in, the world)")
    ap.add_argument("--no-den", action="store_true", help="skip the Den test")
    ap.add_argument("--keep", action="store_true", help="keep the temp folders (to open the fresh launcher by hand after)")
    a = ap.parse_args(argv)
    for f in (sys.stdout, sys.stderr):
        try:
            f.reconfigure(errors="replace")
        except (AttributeError, ValueError):
            pass
    return run(a)


if __name__ == "__main__":
    sys.exit(main())
