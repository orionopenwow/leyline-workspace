"""Fresh computer test: today's launcher download on an empty data folder, every feature a new player meets, through to
in the game (docs/FRESH-COMPUTER-TEST.md; owner 3 Nov: "a clean test from a fresh computer to test every feature with a
bat: like login, select client, torrent drag and drop, dens instant sign in and more, all needed assets on fresh
computer and cookies"). It grew from the first-run test (docs/FIRST-RUN-TEST.md), whose Control step runs it too.

    python -I tools/first-run/first_run.py [run] [--hold 150] [--no-den] [--den-features] [--keep] [--no-game]
                                                 [--game <Wrath folder>] [--work <folder>]
    python -I tools/first-run/first_run.py sandbox [--game <Wrath folder>] [--open]

`run` (the default), PASS / FAIL per numbered check (the numbers are the plan's, docs/FRESH-COMPUTER-TEST.md part B):
  A  this computer: the download (and its checksum), WebView2, the Visual C++ runtime, the addons pack and Den Lite
     downloads, a fresh data folder;
  B  game folders: yours added in the fresh launcher, your other versions too, and every kind of drop on the game
     folder box (a folder, a zip, a link; a magnet link, a .torrent and a shortcut to one refused);
  C  log in: a test account from the website's own test-account API (account/new {audit: true}), the key file, log out,
     the key pasted; the website: log in with the key in a browser's way (the cookie's flags), log out;
  D  play: a test character, PLAY on Wrath Unleashed, logged in by itself, character select, the world (the website's
     audit/presence), screenshots, the one-time login taken out; a community realm and back;
  E  Your Den: the Den test (instant sign-in, build, save, invite) on the same fresh launcher (--den-features: the
     editor's feature test as well);
  F  loadouts: Clean, then back to what was on (the game folder's mods and loadouts file put back exactly);
  G  clean up, always: audit/close, the game folder's files put back, the temp folders removed, your own launcher
     untouched, no secrets in any log.
The test key goes only into the launcher's environment (LEYLINE_TEST_HEROKEY), never a command line, a line or a file
here. --no-game: no game folder (a PC without one, GitHub's Windows runner): A, C and the drops only.

`sandbox`: writes tools\\first-run\\logs\\sandbox\\Leyline fresh computer.wsb and its run.cmd, and with --open starts
Windows Sandbox on it: a brand-new Windows each time, with this PC's Python and your game folder mapped read-only (the
game is copied inside, so the sandbox changes nothing of yours); the report lands in logs\\sandbox\\out.

Logs: tools\\first-run\\logs\\first-run-latest.txt, play-test.json (no secrets), NN-*.png. Standard library only.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import random
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
LOGS = HERE / "logs"
EXE_URL = "https://github.com/orionopenwow/Leyline/releases/download/launcher-latest/Leyline.exe"
SUMS_URL = "https://github.com/orionopenwow/Leyline/releases/download/launcher-latest/SHA256SUMS.txt"
# the launcher's own download addresses (addons.rs DESCRIPTOR_URL, host/lite.rs RELEASE + asset_for)
ADDONS_URL = "https://github.com/orionopenwow/Leyline/releases/download/leyline-addons/addonpack.json"
DEN_LITE_URL = "https://github.com/orionopenwow/leyline-workspace/releases/download/den-lite-latest/den-lite-windows-x64.exe.sha256"
SITES = ("https://leylinelauncher.com", "https://play.openvanilla.app")
REALMLISTS = ("leylinelauncher.com", "play.openvanilla.app")
API = "/wrath/api/social/"
KEY_PAGE = "/wrath/key"
CLI_MARK = b"leyline-play-test-v1"
CHECKS_MARK = b"leyline-play-test-checks-v1"
DEN_MARK = b"leyline-den-play-v1"
REPORT = "play-test.json"
KEY_ENV = "LEYLINE_TEST_HEROKEY"
UA = "LeylineFirstRun/1"
RACE, CLASS = 1, 1            # human warrior (docs/FIRST-RUN-TEST.md 3.3)
WEBVIEW2_GUID = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
VC_DLLS = ("vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll")
NO_GAME_ERROR = "no 3.3.5a game folder"

sys.path.insert(0, str(ROOT / "tools" / "den-check"))


def plain(t, n=300):
    return "".join(c for c in str(t) if c.isprintable() and c != "\ufeff")[:n]


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


def owner_clients(app: Path):
    """(the game folders, the selected one's id) from the owner's own settings (read only), or None."""
    try:
        s = json.loads((app / "settings.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    if not isinstance(s, dict):
        return None
    return [c for c in s.get("clients", []) if isinstance(c, dict) and c.get("path")], s.get("selected_client")


def wrath_folder(app: Path):
    """The owner's first 3.3.5a game folder from his own settings (read only): {name, path} or None."""
    got = owner_clients(app)
    if not got:
        return None
    clients, sel = got
    cl = [c for c in clients if c.get("build") == "3.3.5a"]
    return next((c for c in cl if c.get("id") == sel), cl[0] if cl else None)


def other_folders(app: Path, wrath: str) -> list:
    """The owner's other game folders (other versions; read only: the fresh launcher only looks at them)."""
    got = owner_clients(app)
    if not got:
        return []
    seen, out = {os.path.normcase(os.path.normpath(wrath))}, []
    for c in got[0]:
        k = os.path.normcase(os.path.normpath(c["path"]))
        if k not in seen and Path(c["path"]).is_dir():
            seen.add(k)
            out.append(c["path"])
    return out[:6]


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


def merge_cookies(jar: dict, set_cookies) -> dict:
    """The cookies after Set-Cookie lines (an empty value or Max-Age=0 removes one)."""
    jar = dict(jar)
    for sc in set_cookies or []:
        first = sc.split(";", 1)[0]
        if "=" not in first:
            continue
        k, v = first.split("=", 1)
        k, v = k.strip(), v.strip()
        gone = not v or re.search(r"(?i);\s*max-age=0\b", sc)
        if gone:
            jar.pop(k, None)
        else:
            jar[k] = v
    return jar


def cookie_header(jar: dict) -> str:
    return "; ".join("%s=%s" % kv for kv in jar.items())


def cookie_flags(set_cookies, name: str) -> dict:
    """The flags of the Set-Cookie line that sets `name` (not one that clears it): {httponly, secure, samesite}."""
    for sc in set_cookies or []:
        first = sc.split(";", 1)[0]
        if first.split("=", 1)[0].strip() == name and first.split("=", 1)[-1].strip():
            attrs = [a.strip().lower() for a in sc.split(";")[1:]]
            same = next((a.split("=", 1)[1] for a in attrs if a.startswith("samesite=")), "")
            return {"httponly": "httponly" in attrs, "secure": "secure" in attrs, "samesite": same}
    return {}


FORM_T = re.compile(r'id="hk-login"><input type="hidden" name="t" value="([A-Za-z0-9_.-]+)"')
SIGNED_OUT_MARK = 'id="hk-out"'


# ---------------------------------------------------------------- the outside world (each one replaced by a fake in the tests)
class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):   # a 303 is read, not followed (its Set-Cookie lines are the point)
        return None


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

    def web(self, method, path, form=None, cookie=""):
        """A page as a browser gets it, redirects not followed: (status, Set-Cookie lines, body text)."""
        base = self.site or SITES[0]
        h = {"User-Agent": UA, "Accept": "text/html"}
        data = None
        if form is not None:
            data = urllib.parse.urlencode(form).encode()
            h.update({"Content-Type": "application/x-www-form-urlencoded", "Origin": base, "Sec-Fetch-Site": "same-origin"})
        if cookie:
            h["Cookie"] = cookie
        req = urllib.request.Request(base + path, data=data, method=method, headers=h)
        try:
            r = urllib.request.build_opener(_NoRedirect).open(req, timeout=30)
        except urllib.error.HTTPError as e:
            r = e
        except (urllib.error.URLError, OSError, ValueError):
            return 0, [], ""
        try:
            body = r.read(1 << 20).decode("utf-8", "replace")
        except OSError:
            body = ""
        return getattr(r, "status", None) or r.code, (r.headers.get_all("Set-Cookie") if r.headers else None) or [], body

    def fetch(self, url, dest: Path):
        dest.parent.mkdir(parents=True, exist_ok=True)
        tmp = dest.with_name(dest.name + ".new")
        req = urllib.request.Request(url, headers={"User-Agent": UA})
        with urllib.request.urlopen(req, timeout=300) as r, open(tmp, "wb") as f:
            shutil.copyfileobj(r, f)
        os.replace(tmp, dest)

    def get_text(self, url, limit=1 << 16):
        """(status, text) of a small file anyone can download (no sign-in, as a fresh computer has none)."""
        req = urllib.request.Request(url, headers={"User-Agent": UA})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.status, r.read(limit).decode("utf-8", "replace")
        except urllib.error.HTTPError as e:
            return e.code, ""
        except (urllib.error.URLError, OSError, ValueError):
            return 0, ""

    def computer(self):
        """What this Windows has: WebView2's version, the Visual C++ runtime's files, a sandbox or not."""
        out = {"os": platform.platform(), "windows": os.name == "nt",
               "sandbox": os.environ.get("USERNAME", "").upper() == "WDAGUTILITYACCOUNT"}
        if os.name != "nt":
            return out
        import winreg
        wv = ""
        for hive, key in ((winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\\" + WEBVIEW2_GUID),
                          (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\EdgeUpdate\Clients\\" + WEBVIEW2_GUID),
                          (winreg.HKEY_CURRENT_USER, r"Software\Microsoft\EdgeUpdate\Clients\\" + WEBVIEW2_GUID)):
            try:
                with winreg.OpenKey(hive, key) as k:
                    v = str(winreg.QueryValueEx(k, "pv")[0])
                if v and v != "0.0.0.0":
                    wv = v
                    break
            except OSError:
                continue
        out["webview2"] = wv
        sys32 = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
        out["vc_missing"] = [d for d in VC_DLLS if not (sys32 / d).is_file()]
        return out

    def spawn(self, cmd, env, cwd):
        return subprocess.Popen(cmd, env=env, cwd=cwd)

    def screenshot(self, pid, out: Path):
        import den_check as dc
        return dc.screenshot(pid, out)

    def den_test(self, exe: Path, data: Path, env, features=False):
        r = subprocess.run([sys.executable, "-I", str(ROOT / "tools" / "den-check" / "den_check.py"), "features" if features else "test",
                            "--app-dir", str(data)], env=env, cwd=str(ROOT), capture_output=True, text=True, timeout=2400, errors="replace")
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


def sums_for(text: str, name: str):
    """The sha256 SHA256SUMS.txt gives for `name` (sha256sum's "<hex>  <name>" or "<hex> *<name>"), or None."""
    for line in (text or "").splitlines():
        parts = line.strip().split()
        if len(parts) == 2 and re.fullmatch(r"[0-9a-fA-F]{64}", parts[0]) and parts[1].lstrip("*") == name:
            return parts[0].lower()
    return None


# ---------------------------------------------------------------- the run
def run(a, io=None, lines=None, temp_base=None, app_owner=None, root=None) -> int:
    io = io or Real()
    ln = lines or Lines()
    root = Path(root) if root else ROOT
    work = Path(a.work) if getattr(a, "work", None) else root
    logs = root / "tools" / "first-run" / "logs"
    if logs.exists():   # the last run's lines and pictures (a sandbox run's results stay)
        for e in logs.iterdir():
            if e.name != "sandbox":
                shutil.rmtree(e, ignore_errors=True) if e.is_dir() else e.unlink()
    logs.mkdir(parents=True, exist_ok=True)
    print("FRESH COMPUTER TEST (%s)" % time.strftime("%Y-%m-%d %H:%M"), flush=True)
    owner = Path(app_owner) if app_owner else owner_app_dir()
    watch = [owner / "settings.json", owner / "social-session.json"]
    before = {str(p): sha(p) for p in watch}
    tmp = Path(tempfile.mkdtemp(prefix="leyline-first-run-", dir=temp_base))
    data, webview = tmp / "data", tmp / "webview"
    st8 = {"token": "", "saved": {}, "report": {}, "ran": False, "vc_missing": []}

    def body():
        # ---- A. this computer
        pc = io.computer()
        ln.add(None, "A. this computer", "%s%s" % (pc.get("os", ""), ", Windows Sandbox" if pc.get("sandbox") else ""))
        exe = work / ".launcher" / "first-run" / "Leyline.exe"
        try:
            io.fetch(EXE_URL, exe)
            raw = exe.read_bytes()
        except (OSError, urllib.error.URLError, ValueError) as e:
            raw = b""
            ln.add(False, "1. the download", type(e).__name__, "Check the internet connection, then run the test again.")
        if not raw:
            return
        good = len(raw) > 1_000_000 and raw[:2] == b"MZ"
        ln.add(good, "1. the download", "%.1f MB" % (len(raw) / 1e6), "The release file looks wrong: check the leyline launcher build on GitHub.")
        st, sums = io.get_text(SUMS_URL)
        want = sums_for(sums, "Leyline.exe") if st == 200 else None
        got = hashlib.sha256(raw).hexdigest()
        ln.add(want == got if want else None, "1. the download matches its checksum",
               "SHA256SUMS.txt" if want else "no SHA256SUMS.txt (HTTP %s)" % st,
               "The download and its checksum differ: a new build may have landed meanwhile. Run the test again.")
        if not ln.add(CLI_MARK in raw, "1. the download has play-test", "", "Ship, wait for the leyline launcher build, then run the test again."):
            return
        checks = CHECKS_MARK in raw
        if not checks:
            ln.add(False, "1. the download has the feature checks", "", "Ship, wait for the leyline launcher build, then run the test again.")
        if pc.get("windows"):
            ln.add(bool(pc.get("webview2")), "2. WebView2 (the launcher's window)", pc.get("webview2") or "not found",
                   "Install the Microsoft Edge WebView2 Runtime (Evergreen), then run the test again. Tell Claude: the "
                   "download page should say so.")
            st8["vc_missing"] = pc.get("vc_missing") or []
            if not st8["vc_missing"]:
                ln.add(True, "3. the Visual C++ runtime", "there")
        else:
            ln.add(None, "2. WebView2 (the launcher's window)", "not Windows: not checked")
        for n, url, what in (("4. the Leyline addons pack can be downloaded", ADDONS_URL, "addonpack.json"),
                             ("4. Den Lite (Your Den's server) can be downloaded", DEN_LITE_URL, "den-lite .sha256")):
            st, _ = io.get_text(url, 4096)
            ln.add(st == 200, n, "%s HTTP %s" % (what, st), "A fresh computer can't get it without signing in to GitHub: "
                   "its release must be public. Tell Claude this line.")

        data.mkdir(parents=True)
        webview.mkdir(parents=True)
        ln.add(not any(data.iterdir()), "5. fresh start: an empty data folder", str(tmp))

        # ---- B. game folders (read only from the owner's settings, or --game)
        gpath, more = None, []
        if not a.no_game:
            if getattr(a, "game", None):
                g = {"path": a.game}
            else:
                g = wrath_folder(owner)
            if not ln.add(g is not None and Path(g["path"]).is_dir(), "6. your Wrath game folder", g and g.get("path") or "",
                          "Add your Wrath folder in your own launcher first (Game folders), or pass --game <folder>."):
                return
            gpath = Path(g["path"])
            more = [] if getattr(a, "game", None) else other_folders(owner, str(gpath))
            st8["saved"] = stash(game_files(gpath))
        else:
            ln.add(None, "6. your Wrath game folder", "--no-game: none (PLAY, Your Den and loadouts are left out)")

        # ---- C. log in: a test account (the website's own test-account API)
        st, d = io.http("POST", "account/new", js={"audit": True})
        token, key = str(d.get("token") or ""), str(d.get("key") or "")
        st8["token"] = token
        ln.secret(token)
        ln.secret(key)
        me = d.get("me") or {}
        made = "%s#%s" % (me.get("name", ""), me.get("tag", "")) if me.get("tag") else me.get("name", "")
        if not ln.add(st == 200 and bool(token) and key.startswith("ovk1."), "10. a test account", made or "HTTP %s" % st,
                      "Too many new accounts this hour (3 a network) or the website is down: wait an hour, or run check."):
            return
        web_lines(ln, io, key)

        # ---- D. a test character (its one-start game login: never printed)
        if gpath is not None:
            st, g = io.http("POST", "game/login", js={}, token=token)
            user, pw = str(g.get("user") or ""), str(g.get("pass") or "")
            ln.secret(pw)
            if st == 200 and user and pw:
                name = char_name(me.get("name", ""))
                try:
                    ok, why = io.make_character(user, pw, name)
                except Exception as e:   # noqa: BLE001
                    ok, why = False, type(e).__name__
                ln.add(ok, "15. a test character", "%s, %s" % (name, why), "The realm may be down or full: run check, then try again.")
            else:
                ln.add(False, "15. a test character", "game/login HTTP %s %s" % (st, g.get("error", "")), "Run signin (the website's sign-in), then try again.")

        # ---- the fresh launcher: play-test (PLAY and the feature checks)
        env = dict(os.environ)
        env.update({"LEYLINE_APP_DIR": str(data), "WEBVIEW2_USER_DATA_FOLDER": str(webview), KEY_ENV: key})
        env.pop("LEYLINE_EXE", None)
        hold = max(30, min(int(a.hold), 600))
        cmd = [str(exe), "play-test", "--official", "--fresh"]
        if gpath is not None:
            cmd += ["--game", str(gpath)]
            for m in more:
                cmd += ["--game", m]
            cmd += ["--world", "--hold", str(hold)]
        if checks:
            cmd.append("--checks")
        t0 = io.now()
        proc = io.spawn(cmd, env, str(exe.parent))
        env.pop(KEY_ENV, None)
        st8["ran"] = True
        ln.add(None, "   started", " ".join(cmd[1:]))
        pid, shots, seen, world, given, phase, pres = 0, 0, "", None, False, "", set()
        deadline = t0 + (hold + 180 if gpath is not None else 120)
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
        settings = read_json(data / "settings.json")
        launcher_lines(ln, report, settings, gpath, more)
        drop_lines(ln, report.get("checks") or {}, gpath is not None, checks)
        signin_lines(ln, report, (report.get("checks") or {}).get("signin"), checks)
        if gpath is None:
            if NO_GAME_ERROR not in str(report.get("error", "")):
                ln.add(False, "   play-test without a game folder", report.get("error", "exit %s" % code), "Tell Claude this line.")
            return
        if pres and 200 not in pres:
            ln.add(None, "18. the website's presence check didn't answer", "HTTP %s" % ",".join(str(x) for x in sorted(pres)))
            if 404 in pres:
                ln.add(False, "18. the website has audit/presence", "404", "Run the signin step (it deploys herokeys.py), then run the test again.")
        play_lines(ln, report, pid=pid, given=given, world=world, shots=shots, code=code)
        after_lines(ln, report.get("checks") or {}, checks)

        # ---- E. Your Den on the same fresh folder
        if a.no_den:
            ln.add(None, "23. Your Den test", "skipped (--no-den)")
        elif DEN_MARK not in raw:
            ln.add(False, "23. Your Den test", "this launcher has no den-play", "Ship, wait for the build, then run again.")
        else:
            denv = dict(env)
            denv["LEYLINE_EXE"] = str(exe)
            feats = bool(getattr(a, "den_features", False))
            dcode, dtext = io.den_test(exe, data, denv, feats) if feats else io.den_test(exe, data, denv)
            last = [x for x in dtext.splitlines() if x.startswith(("FAIL", "PASS"))]
            label = "24. Your Den: instant sign-in, the editor's features" if feats else "23. Your Den: instant sign-in, walk, build, save, invite a friend"
            ln.add(dcode == 0, label, (last[-1] if last else "exit %s" % dcode),
                   "Read tools\\den-check\\logs\\den-test-latest.txt.")
    try:
        body()
    finally:
        cleanup(ln, io, st8["token"], tmp, st8["saved"], a.keep)
        if st8["vc_missing"]:   # missing, but did the launcher run without it?
            ran = bool(st8["report"].get("mark"))
            ln.add(True if ran else False, "3. the Visual C++ runtime",
                   ("not on this PC (%s); the launcher ran without it" if ran else "not on this PC (%s), and the launcher didn't start")
                   % ", ".join(st8["vc_missing"]),
                   "Install the Microsoft Visual C++ Redistributable (x64), then run again. Tell Claude: the launcher "
                   "should carry it (static runtime) or the download page say so.")
        after = {str(p): sha(p) for p in watch}
        ln.add(after == before, "29. your own launcher untouched", "settings and sign-in the same as before",
               "Your own launcher's settings changed during the test: tell Claude this line.")
    return finish(ln, logs, st8["report"])


def read_json(p: Path):
    try:
        v = json.loads(p.read_text(encoding="utf-8"))
        return v if isinstance(v, dict) else None
    except (OSError, ValueError):
        return None


def web_lines(ln, io, key):
    """14. The website as a browser uses it: the Log in page, log in with the key (its form, as a paste does), the
    session cookie's flags, signed in, log out, and the old cookie no longer signed in. Cookie values are never shown."""
    st, sc, page = io.web("GET", KEY_PAGE)
    jar = merge_cookies({}, sc)
    m = FORM_T.search(page or "")
    if not ln.add(st == 200 and SIGNED_OUT_MARK in (page or "") and bool(m), "14. website: the Log in page", "HTTP %s" % st,
                  "Run signin (it deploys the website's Hero Key page), then run the test again."):
        return
    st, sc, _ = io.web("POST", KEY_PAGE + "/signin", form={"t": m.group(1), "key": key}, cookie=cookie_header(jar))
    jar = merge_cookies(jar, sc)
    for v in jar.values():
        ln.secret(v)
    f = cookie_flags(sc, "wr_session")
    ok = st in (302, 303) and bool(f)
    ln.add(ok, "14. website: log in with the Hero Key", "HTTP %s" % st, "The website refused the test key: run signin, then try again.")
    if not ok:
        return
    ln.add(f.get("httponly") and f.get("secure") and f.get("samesite") in ("lax", "strict"), "14. website: the sign-in cookie is HttpOnly, Secure, SameSite",
           "HttpOnly %s, Secure %s, SameSite %s" % (f.get("httponly"), f.get("secure"), f.get("samesite") or "none"),
           "Tell Claude this line (stream/herokeys.py / the realm's session cookie).")
    st, sc, page = io.web("GET", KEY_PAGE, cookie=cookie_header(jar))
    ln.add(st == 200 and SIGNED_OUT_MARK not in (page or ""), "14. website: signed in on the next page", "HTTP %s" % st, "Tell Claude this line.")
    old = cookie_header(jar)
    st, sc, _ = io.web("POST", "/logout", form={}, cookie=old)
    st2, _, page = io.web("GET", KEY_PAGE, cookie=old)
    ln.add(st2 == 200 and SIGNED_OUT_MARK in (page or ""), "14. website: log out ends the session", "logout HTTP %s" % st,
           "The old cookie still signs in after Log out: tell Claude this line.")


def launcher_lines(ln, r, settings, gpath, more):
    fr = r.get("fresh") or {}
    ln.add(bool(fr.get("asked")) and bool(fr.get("empty")), "5. the launcher started on an empty data folder", json.dumps(fr),
           "The launcher found settings in the new folder: tell Claude this line.")
    if gpath is None:
        return
    added = r.get("added") or {}
    ln.add(added.get("build") == "3.3.5a" and bool(added.get("complete")), "7. your game folder added (as Find your game files does)",
           "build %s, missing %s" % (added.get("build"), ", ".join(added.get("missing") or []) or "nothing"),
           "Your Wrath folder looks incomplete: open it in your own launcher, Game folders, Check.")
    clients = (settings or {}).get("clients") or []
    ln.add(len(clients) == 1 + len(more), "7. the new launcher has exactly your game folders", "%d" % len(clients), "Tell Claude this line.")
    if more:
        rows = r.get("more_added") or []
        bad = [x.get("why", "") for x in rows if not x.get("ok")]
        ln.add(not bad and len(rows) == len(more), "8. your other game folders added too",
               ", ".join("%s (%s)" % (x.get("name", ""), x.get("build", "")) for x in rows if x.get("ok")) or "; ".join(bad),
               "One of your other folders wasn't taken: open it in your own launcher, Game folders, Check.")
    else:
        ln.add(None, "8. your other game folders", "none in your launcher")


def drop_lines(ln, checks, with_game, asked):
    if not asked:
        return
    if checks.get("torrent_switch"):
        ln.add(False, "9. drag and drop: Settings › Testing is off in a fresh launcher", "on",
               "A fresh launcher must start with torrent downloads off: tell Claude this line.")
    rows = checks.get("drops")
    if not isinstance(rows, list) or not rows:
        ln.add(False, "9. drag and drop", "the launcher didn't report its drops", "Tell Claude this line.")
        return
    for x in rows:
        ln.add(bool(x.get("ok")), "9. drop %s" % x.get("name", "?"), "expect %s, got: %s" % (x.get("expect", ""), x.get("got", "")),
               "Tell Claude this line (launcher clientimport.rs).")
    if not with_game:
        ln.add(None, "9. drop your game folder", "--no-game: not tried")


def signin_lines(ln, r, si, asked):
    ks = r.get("key_signin") or {}
    if asked and isinstance(si, dict):
        ln.add(bool(si.get("file")), "11. launcher: log in with the Hero Key file", si.get("file_why", ""),
               "The key file wasn't taken: tell Claude this line (social.rs find_key_file).")
        ln.add(bool(si.get("logout")), "12. launcher: log out", "", "The launcher kept its sign-in after Log out: tell Claude this line.")
        ln.add(bool(si.get("paste")), "13. launcher: log in with the key pasted", si.get("paste_why", ""),
               "The website's Hero Key sign-in refused the key: run signin, then try again.")
    elif asked:
        ln.add(False, "11. launcher: log in with the Hero Key file", r.get("error", "not reached"), "See the lines above.")
    else:
        ln.add(bool(ks.get("ok")) and bool(r.get("signed_in")), "13. launcher: log in with the Hero Key", ks.get("name", ""),
               "The website's Hero Key sign-in refused the key: run signin, then try again.")


def play_lines(ln, r, pid, given, world, shots, code):
    """The PASS / FAIL lines for play-test's PLAY and what the website saw."""
    wl = r.get("website_login") or {}
    ln.add(bool(wl.get("ok")), "16. the website gave the game login for PLAY", wl.get("why", ""), "Run signin, then try again.")
    ln.add(bool(pid), "16. PLAY started the game", "pid %s" % pid if pid else r.get("error", "no game process"),
           "The game didn't start: run client-smoke and read its log.")
    hy, keys = r.get("hybrid") or {}, r.get("keys") or {}
    by_itself = bool(hy.get("ok")) or keys.get("typed") is True
    ln.add(by_itself, "17. the game logged in by itself", "login screen hand-over" if hy.get("ok") else ("typed by the launcher" if by_itself else hy.get("why", "")),
           "The login screen waited for typing: run client-smoke and send its screenshots.")
    ln.add(given or by_itself, "18. character select (the game got its login)", "" if given else "seen from the launcher's report only")
    ln.add(bool(world), "19. in the world", ("%s on map %s after %.0fs" % (world["character"], world["map"], world["at"])) if world else "not seen by the website",
           "The game stopped before the world: look at the last screenshots in tools\\first-run\\logs.")
    ln.add(shots > 0, "20. screenshots of the game", "%d" % shots, "Screenshots need Windows and the game window: see the den-test step's notes.")
    ln.add(bool(r.get("scrubbed")) and not r.get("session_left"), "21. the one-time login taken back out of the game", "",
           "A one-time login was left in the game folder's archive: run client-smoke once to clear it.")
    if r.get("error"):
        ln.add(False, "16. the launcher stopped", r["error"], "Tell Claude this line.")
    ln.add(code == 0, "16. play-test's exit code", str(code), "See the lines above.")


def after_lines(ln, checks, asked):
    """22 (a community realm and back) and 25 (loadouts), done by the launcher after the game closed."""
    if not asked:
        return
    rs = checks.get("realm_switch")
    if not isinstance(rs, dict):
        ln.add(False, "22. a community realm and back", "not reported", "Tell Claude this line.")
    elif rs.get("ok") is None:
        ln.add(None, "22. a community realm and back", rs.get("why", ""))
    else:
        ln.add(bool(rs.get("ok")), "22. a community realm and back", "%s: there %s, back %s %s" % (rs.get("realm", ""), rs.get("there"), rs.get("back"), rs.get("why", "")),
               "Tell Claude this line (app.rs point_and_play).")
    lo = checks.get("loadouts")
    if not isinstance(lo, dict):
        ln.add(False, "25. loadouts: Clean and back", "not reported", "Tell Claude this line.")
        return
    if lo.get("ok") is None:
        ln.add(None, "25. loadouts: Clean and back", lo.get("why", ""))
    else:
        ln.add(bool(lo.get("ok")), "25. loadouts: Clean and back", "%s Leyline mods on" % lo.get("mods_on", "?"),
               "Tell Claude this line (loadouts.rs switch).")
    ln.add(bool(lo.get("put_back")), "25. loadouts: your mods and loadouts as they were", "",
           "Open Mods & Addons › Installed in your launcher and check what is on; tell Claude this line.")


def cleanup(ln, io, token, tmp: Path, saved, keep: bool):
    if token:
        st, d = io.http("POST", "audit/close", js={}, token=token)
        ln.add(st == 200, "26. the test account closed", "" if st == 200 else "HTTP %s %s" % (st, d.get("error", "")),
               "Nothing to do: test accounts can only close themselves within a day; the next run makes a new one.")
        st2, _ = io.http("GET", "me", token=token)
        ln.add(st2 == 401, "26. its sign-in no longer works", "HTTP %s" % st2, "Tell Claude this line.")
    if saved:
        bad = unstash(saved)
        ln.add(not bad, "27. your game folder put back (Config.wtf, realmlist, login archive)", ", ".join(bad),
               "Close the game, then run the test again (it puts them back first).")
    if keep:
        ln.add(None, "28. the temp folders kept (--keep)", str(tmp))
    else:
        shutil.rmtree(tmp, ignore_errors=True)
        ln.add(not tmp.exists(), "28. the temp folders removed", str(tmp), "Delete it by hand.")


def finish(ln, logs: Path, report) -> int:
    if report:
        safe = {k: v for k, v in report.items() if k not in ("pass", "password", "key", "token")}
        (logs / REPORT).write_text(ln.hide(json.dumps(safe, indent=2)), encoding="utf-8")
    text = ln.text()
    leaked = any(s in text for s in ln.secrets) or "ovk1." in text
    if leaked:   # never: hide() runs on every line; this is the belt to its braces
        ln.add(False, "30. no secrets in the log", "", "Tell Claude this line.")
    else:
        ln.add(True, "30. no secrets in the log")
    verdict = "FAIL" if ln.failed() else "PASS"
    out = "FRESH COMPUTER TEST (%s)\n%s%s\n" % (time.strftime("%Y-%m-%d %H:%M"), ln.hide(ln.text()), verdict)
    (logs / "first-run-latest.txt").write_text(out, encoding="utf-8")
    print(verdict, flush=True)
    print("Saved %s" % (logs / "first-run-latest.txt"), flush=True)
    return 0 if verdict == "PASS" else 1


# ---------------------------------------------------------------- Windows Sandbox
WSB_NAME = "Leyline fresh computer.wsb"
SB_ROOT, SB_PY, SB_GAME, SB_OUT = r"C:\leyline", r"C:\py", r"C:\game-ro", r"C:\out"
# only what the test runs (never the whole workspace: tools\ also holds the owner's saved tokens)
SB_COPY = ("tools\\first-run", "tools\\den-check", "tools\\audit", "hosting\\kit\\tools")


def xml_esc(s: str) -> str:
    return str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")


def wsb_text(root: Path, python_home: Path, game, out: Path) -> str:
    """The sandbox's configuration: the workspace, Python and the game folder read-only, the out folder writable,
    networking on (the download, the website), the clipboard and printers off, and run.cmd at logon."""
    maps = [(root, SB_ROOT, True), (python_home, SB_PY, True), (out, SB_OUT, False)]
    if game:
        maps.append((Path(game), SB_GAME, True))
    folders = "".join(
        "    <MappedFolder>\n      <HostFolder>%s</HostFolder>\n      <SandboxFolder>%s</SandboxFolder>\n      <ReadOnly>%s</ReadOnly>\n    </MappedFolder>\n"
        % (xml_esc(h), xml_esc(sb), "true" if ro else "false") for h, sb, ro in maps)
    return ("<Configuration>\n  <VGpu>Enable</VGpu>\n  <Networking>Enable</Networking>\n  <ClipboardRedirection>Disable</ClipboardRedirection>\n"
            "  <PrinterRedirection>Disable</PrinterRedirection>\n  <MemoryInMB>8192</MemoryInMB>\n  <MappedFolders>\n%s  </MappedFolders>\n"
            "  <LogonCommand>\n    <Command>cmd.exe /c %s\\run.cmd</Command>\n  </LogonCommand>\n</Configuration>\n" % (folders, SB_OUT))


def run_cmd_text(game: bool) -> str:
    """run.cmd inside the sandbox: copy the test's own folders (and the game) to the sandbox's disk, run the test with
    this PC's Python, copy the logs out. CRLF."""
    q = "/E /NFL /NDL /NJH /NJS /R:1 /W:1"
    lines = ["@echo off", "title Leyline - fresh computer test (Windows Sandbox)", "set PYTHONUTF8=1",
             "if exist %s\\DONE.txt del %s\\DONE.txt" % (SB_OUT, SB_OUT), "mkdir C:\\fresh\\repo 2>nul"]
    for rel in SB_COPY:
        lines.append("robocopy \"%s\\%s\" \"C:\\fresh\\repo\\%s\" %s /XD logs __pycache__ >nul" % (SB_ROOT, rel, rel, q))
    args = "run"
    if game:
        lines.append("echo Copying your game folder into the sandbox (a few minutes)...")
        lines.append("robocopy \"%s\" \"C:\\fresh\\Wrath\" %s >nul" % (SB_GAME, q))
        args += " --game C:\\fresh\\Wrath"
    else:
        args += " --no-game"
    lines += ["%s\\python.exe -I C:\\fresh\\repo\\tools\\first-run\\first_run.py %s > %s\\console.txt 2>&1" % (SB_PY, args, SB_OUT),
              "robocopy C:\\fresh\\repo\\tools\\first-run\\logs %s\\logs %s >nul" % (SB_OUT, q),
              "robocopy C:\\fresh\\repo\\tools\\den-check\\logs %s\\den-logs %s >nul" % (SB_OUT, q),
              "echo done> %s\\DONE.txt" % SB_OUT,
              "type %s\\logs\\first-run-latest.txt" % SB_OUT,
              "pause"]
    return "\r\n".join(lines) + "\r\n"


def sandbox(a, root=None, opener=None, python_home=None, owner=None, windows=None) -> int:
    root = Path(root) if root else ROOT
    windows = (os.name == "nt") if windows is None else windows
    where = root / "tools" / "first-run" / "logs" / "sandbox"
    out = where / "out"
    if out.exists():
        shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True, exist_ok=True)
    game = a.game
    if not game and not a.no_game:
        g = wrath_folder(Path(owner) if owner else owner_app_dir())
        game = g and g.get("path")
    py = Path(python_home or sys.base_prefix)
    if not (py / ("python.exe" if windows else "bin")).exists() or "windowsapps" in str(py).lower():
        print("FAIL this PC's Python isn't a normal install (%s): Windows Sandbox needs one it can map" % py)
        return 1
    (where / WSB_NAME).write_text(wsb_text(root, py, game, out), encoding="utf-8")
    (out / "run.cmd").write_bytes(run_cmd_text(bool(game)).encode("ascii", "replace"))
    print("Wrote %s" % (where / WSB_NAME))
    print("Game folder: %s" % (game or "none (--no-game)"))
    exe = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32" / "WindowsSandbox.exe"
    if not a.open:
        print("Open it with: double-click the .wsb file (Windows Sandbox must be on: Windows Features).")
        return 0
    if not windows or not (exe.is_file() or opener):
        print("FAIL Windows Sandbox isn't on here. Turn it on: Windows Features, Windows Sandbox (Pro, Enterprise or "
              "Education; a restart), or use a second Windows user (docs/FRESH-COMPUTER-TEST.md part C).")
        return 1
    (opener or os.startfile)(str(where / WSB_NAME))
    print("Windows Sandbox is starting. The test runs by itself; its report lands in %s" % (out / "logs" / "first-run-latest.txt"))
    return 0


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    mode = argv.pop(0) if argv and argv[0] in ("run", "sandbox") else "run"
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--hold", type=int, default=150, help="seconds play-test keeps the game open (start, log in, the world)")
    ap.add_argument("--no-den", action="store_true", help="skip the Den test")
    ap.add_argument("--den-features", action="store_true", help="the Den editor's feature test instead of the Den test")
    ap.add_argument("--keep", action="store_true", help="keep the temp folders (to open the fresh launcher by hand after)")
    ap.add_argument("--no-game", action="store_true", help="no game folder: the computer, log in, the website, the drops")
    ap.add_argument("--game", help="the Wrath game folder (default: the first one in your own launcher)")
    ap.add_argument("--work", help="where the download goes (default: the workspace's .launcher\\first-run)")
    ap.add_argument("--open", action="store_true", help="sandbox: start Windows Sandbox on the new .wsb")
    a = ap.parse_args(argv)
    for f in (sys.stdout, sys.stderr):
        try:
            f.reconfigure(errors="replace")
        except (AttributeError, ValueError):
            pass
    if mode == "sandbox":
        return sandbox(a)
    return run(a)


if __name__ == "__main__":
    sys.exit(main())
