#!/usr/bin/env python3
"""den-check and den-test: is My Den one click into the world on this PC's own game?

    py -3 tools/den-check/den_check.py check [--game <folder>]     read-only: the game folder, Leyline's login
                                                                    archive, Forever UI, Config.wtf, Den Lite and a
                                                                    sign-in through the exact steps (a Den Lite of
                                                                    its own on spare ports, a scratch folder)
    py -3 tools/den-check/den_check.py test [--invite]             the real thing, unattended: the launcher's own
                                                                    My Den (Leyline.exe den-play --test) starts Den
                                                                    Lite and the game, signs in, waits for the world,
                                                                    takes a screenshot, closes the game and Den Lite

Prints PASS / FAIL / INFO per step and, at the end, the one fix. `test` writes logs/den-test-latest.txt and
logs/den-test-latest.png next to this file (logs/ is not in git). Never needs a key press; safe to run again.
Nothing here prints a password or reads game files beyond archive headers and Leyline's own archive.
"""
import argparse
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import mpqlite  # noqa: E402
import wowclient  # noqa: E402

ROOT = HERE.parent.parent
OUR_LUA = "Interface\\GlueXML\\LeylineHybrid.lua"
SESSION = "Interface\\GlueXML\\LeylineSession.lua"
TOC = "Interface\\GlueXML\\GlueXML.toc"
NO_SESSION = b"LEYLINE_SESSION = nil"
CLI_MARK = b"leyline-den-play-v1"
MILESTONES = [
    ("login_challenge", "the game asked to sign in"),
    ("auth_ok", "signed in"),
    ("realm_list", "realm list sent"),
    ("world_auth", "joined Your Den"),
    ("char_enum", "character select"),
    ("player_login", "Enter World"),
    ("in_world", "in the world (the white island)"),
    ("first_move", "first movement"),
    ("builder_ready", "the Leyline UI's Den panel and Builder answered"),
]


class Report:
    def __init__(self):
        self.rows = []

    def add(self, kind, step, detail="", fix=""):
        self.rows.append((kind, step, detail, fix))
        line = "%-4s %s" % (kind, step) + (": " + detail if detail else "")
        print(line, flush=True)
        if fix and kind == "FAIL":
            print("     fix: " + fix, flush=True)

    def ok(self, cond, step, detail="", fix=""):
        self.add("PASS" if cond else "FAIL", step, detail, fix)
        return cond

    def failed(self):
        return [r for r in self.rows if r[0] == "FAIL"]

    def end(self, title):
        f = self.failed()
        print("")
        if f:
            print("The one fix: " + (f[0][3] or f[0][1]))
            print("%s FAIL (%d failed)" % (title, len(f)))
        else:
            print("%s PASS" % title)
        return 0 if not f else 1

    def text(self):
        out = []
        for kind, step, detail, fix in self.rows:
            out.append("%-4s %s%s" % (kind, step, (": " + detail) if detail else ""))
            if fix and kind == "FAIL":
                out.append("     fix: " + fix)
        return "\n".join(out) + "\n"


# ------------------------------------------------------------------------------------------------ where things are

def app_dir():
    if os.environ.get("LEYLINE_APP_DIR"):
        return Path(os.environ["LEYLINE_APP_DIR"])
    if os.name == "nt":
        return Path(os.environ.get("APPDATA", str(Path.home()))) / "Leyline"
    return Path(os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local" / "share"))) / "Leyline"


def settings(app):
    try:
        return json.loads((app / "settings.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def den_game(s):
    """The game folder My Den plays with: the selected client when it is a 3.3.5a one, else the first such."""
    clients = [c for c in (s or {}).get("clients", []) if c.get("build") == "3.3.5a"]
    sel = (s or {}).get("selected_client")
    for c in clients:
        if c.get("id") == sel:
            return c
    return clients[0] if clients else None


def find_ci(d, name):
    try:
        for e in os.listdir(d):
            if e.lower() == name.lower():
                return Path(d) / e
    except OSError:
        pass
    return None


def exe_name(base):
    return base + (".exe" if os.name == "nt" else "")


def launcher_exe():
    if os.environ.get("LEYLINE_EXE"):
        return Path(os.environ["LEYLINE_EXE"])
    for p in (ROOT / ".launcher" / "Leyline.exe", ROOT / ".launcher" / "leyline.exe"):
        if p.is_file():
            return p
    return None


def locales(data):
    try:
        return sorted(e for e in os.listdir(data) if len(e) == 4 and e[:2].islower() and e[2:].isupper() and (Path(data) / e).is_dir())
    except OSError:
        return []


def patch_key(name, loc=None):
    """The client's order (later wins): the base archives, then patch, patch-2, patch-3, then 4..9 and A..Z."""
    n = name.lower()
    base = ["common.mpq", "common-2.mpq", "expansion.mpq", "lichking.mpq"]
    if loc:
        l = loc.lower()
        base = ["locale-%s.mpq" % l, "speech-%s.mpq" % l, "expansion-locale-%s.mpq" % l, "lichking-locale-%s.mpq" % l,
                "expansion-speech-%s.mpq" % l, "lichking-speech-%s.mpq" % l]
        pre = "patch-%s" % l
    else:
        pre = "patch"
    if n in base:
        return (0, base.index(n))
    if n == pre + ".mpq":
        return (1, 0)
    if n.startswith(pre + "-") and n.endswith(".mpq"):
        c = n[len(pre) + 1:-4]
        if len(c) == 1 and c.isalnum():
            return (1, "0123456789abcdefghijklmnopqrstuvwxyz".index(c) + 1)
    return (2, n)


def archive_info(p):
    try:
        a = mpqlite.Archive(str(p))
    except Exception as e:  # noqa: BLE001
        return None, "unreadable (%s)" % e
    try:
        has_toc = a.has(TOC)
    except Exception:  # noqa: BLE001
        has_toc = False
    return a, "login screen list (GlueXML.toc) inside" if has_toc else ""


def read_config(game):
    cfg = find_ci(find_ci(game, "WTF") or game / "WTF", "Config.wtf")
    vals = {}
    if cfg and cfg.is_file():
        for line in cfg.read_text(encoding="utf-8", errors="replace").splitlines():
            parts = line.strip().split(None, 2)
            if len(parts) == 3 and parts[0].upper() == "SET":
                vals[parts[1]] = parts[2].strip().strip('"')
    return cfg, vals


def port_open(port):
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.4):
            return True
    except OSError:
        return False


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def our_archive(p):
    """(ours, session_left, toc_has_ours, names) for Leyline's archive at p."""
    a, _ = archive_info(p)
    if a is None or not a.has(OUR_LUA):
        return False, False, False, []
    sess = a.read(SESSION) or b""
    toc = (a.read(TOC) or b"").decode("utf-8", "replace")
    return True, NO_SESSION not in sess, "LeylineHybrid.xml" in toc, a.names()


# ------------------------------------------------------------------------------------------------ check

def check(args, rep):
    app = Path(args.app_dir) if args.app_dir else app_dir()
    s = settings(app)
    game = Path(args.game) if args.game else None
    if game is None:
        c = den_game(s)
        rep.ok(c is not None, "launcher settings", "%s, game folder %s" % (app / "settings.json", c.get("path") if c else "-"),
               "Open Leyline and add your 3.3.5a game folder in the play bar (or pass --game <folder>).")
        if c is None:
            return
        game = Path(c["path"])
    exe = find_ci(game, "Wow.exe")
    data = find_ci(game, "Data")
    if not rep.ok(bool(exe and data and find_ci(data, "common.MPQ")), "game folder", str(game),
                  "Pick the folder with Wow.exe and Data\\common.MPQ in the launcher."):
        return
    raw = exe.read_bytes()
    rep.add("INFO", "game program", "%s, %d bytes, sha256 %s" % (exe.name, len(raw), hashlib.sha256(raw).hexdigest()[:16]))

    # the archives, in the client's order, and which hold a login screen list
    locs = locales(data)
    groups = [(None, data)] + [(l, data / l) for l in locs]
    for loc, d in groups:
        names = sorted((e for e in os.listdir(d) if e.lower().endswith(".mpq")), key=lambda n: patch_key(n, loc))
        parts = []
        for n in names:
            _, note = archive_info(d / n) if (n.lower().startswith("patch") or "locale" in n.lower()) else (None, "")
            parts.append(n + (" [%s]" % note if note else ""))
        rep.add("INFO", "archives in Data%s" % ("\\" + loc if loc else ""), ", ".join(parts) or "none")

    # Leyline's login archive (crate::hybrid), root and locale copies
    z = find_ci(data, "patch-Z.MPQ")
    fui_toc = find_ci(find_ci(game, "Interface") or game, "GlueXML")
    fui_loose = bool(fui_toc and find_ci(fui_toc, "GlueXML.toc") and find_ci(fui_toc, "ForeverUIGlue.xml"))
    refused = (game / "Leyline" / "glue-refused").exists()
    if z is None:
        why = ("Forever UI's login screens aren't in this folder, so the launcher doesn't write it" if not fui_loose
               else "not written yet (My Den writes it at the next start)")
        rep.add("INFO", "Leyline's login archive (Data\\patch-Z.MPQ)", "none: %s; the launcher's keys sign in instead" % why)
    else:
        ours, left, toc_ok, names = our_archive(z)
        if not ours:
            rep.add("INFO", "Data\\patch-Z.MPQ", "another mod's archive (%d MB; Forever UI's installer puts one there): Leyline leaves it alone and its keys sign in"
                    % (z.stat().st_size // (1 << 20)))
        else:
            rep.ok(toc_ok and len(names) >= 6, "Leyline's login archive opens", "%d files, its login screen list names Leyline's" % len(names),
                   "Start My Den once: the launcher writes it again.")
            rep.ok(not left, "no sign-in left in the archive", "the handoff is gone after the game closed" if not left else "a sign-in is still in it",
                   "Close the game, then start and close My Den once (it takes the sign-in out), or delete Data\\patch-Z.MPQ.")
        for loc in locs:
            lz = find_ci(data / loc, "patch-%s-Z.MPQ" % loc)
            if lz is not None:
                o, l2, _, _ = our_archive(lz)
                if o:
                    rep.ok(not l2, "Leyline's archive in Data\\%s" % loc, "the same archive (whichever folder wins)",
                           "Close the game, then start and close My Den once.")
    rep.add("INFO", "Forever UI's login screens", "in Interface\\GlueXML" if fui_loose else "not in this folder")
    if refused:
        rep.add("INFO", "this game refused Leyline's login screen once", "My Den signs in with the launcher's keys (Leyline\\glue-refused)")

    cfg, vals = read_config(game)
    if cfg:
        acct, realm, rl = vals.get("accountName", ""), vals.get("realmName", ""), vals.get("realmList", "")
        rep.add("INFO", "Config.wtf", "accountName %s, realmName %s, realmList %s, movie %s, readEULA %s" %
                (acct or "-", realm or "-", rl or "-", vals.get("movie", "-"), vals.get("readEULA", "-")))
        if acct or realm:
            rep.ok(acct.upper() in ("DEN", "DENAUTO") and realm == "Your Den", "Config.wtf is set for Your Den",
                   "", "Press My Den once in the launcher (it writes the account DEN and the realm Your Den).")
    else:
        rep.add("INFO", "Config.wtf", "not there yet (the game writes it at its first start)")
    for loc in locs:
        rl = find_ci(data / loc, "realmlist.wtf")
        if rl:
            rep.add("INFO", "realmlist.wtf (%s)" % loc, rl.read_text(encoding="utf-8", errors="replace").strip()[:80])

    # Den Lite, and a sign-in through the exact steps on a Den Lite of our own
    den = app / "den-lite" / "bin" / exe_name("den-lite")
    if os.environ.get("DEN_LITE_BIN"):
        den = Path(os.environ["DEN_LITE_BIN"])
    if not rep.ok(den.is_file(), "Den Lite program", str(den), "Press My Den once in the launcher (it gets Den Lite), or run the den-lite step."):
        return
    rep.add("INFO", "Den Lite running now", "yes (login port answers)" if port_open(3724) else "no")
    if args.no_login:
        return
    for name, ok, detail in den_lite_login(den):
        rep.ok(ok, "sign-in step: " + name, detail, "Den Lite didn't answer this step: update Leyline (Den Lite comes with it), then run den-check again.")


def den_lite_login(den, account="DEN", password="den", typed=None):
    """Den Lite on spare ports with a scratch Dens folder, and the headless client's sign-in (`typed`: the password
    the client gives, when a test wants a wrong one)."""
    scratch = Path(tempfile.mkdtemp(prefix="den-check-"))
    lp, wp = free_port(), free_port()
    p = subprocess.Popen([str(den), "--dens", str(scratch), "--bind", "127.0.0.1", "--login-port", str(lp), "--world-port", str(wp),
                          "--password", password, "--no-friends", "--no-companion"],
                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                         creationflags=0x08000000 if os.name == "nt" else 0)
    try:
        for _ in range(50):
            if port_open(lp):
                break
            time.sleep(0.1)
        return wowclient.run_login("127.0.0.1", lp, account, typed if typed is not None else password)
    finally:
        p.terminate()
        try:
            p.wait(5)
        except subprocess.TimeoutExpired:
            p.kill()
        shutil.rmtree(scratch, ignore_errors=True)


# ------------------------------------------------------------------------------------------------ test

SHOT_PS = r"""
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public class LW { [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
public struct R { public int L, T, Rt, B; } }
'@
$p = Get-Process -Id __PID__ -ErrorAction Stop
$h = $p.MainWindowHandle
$r = New-Object LW+R
[LW]::GetWindowRect($h, [ref]$r) | Out-Null
$w = [Math]::Max(1, $r.Rt - $r.L); $hh = [Math]::Max(1, $r.B - $r.T)
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc(); [LW]::PrintWindow($h, $dc, 2) | Out-Null; $g.ReleaseHdc($dc)
$bmp.Save('__OUT__', [System.Drawing.Imaging.ImageFormat]::Png)
$g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
$bmp.Save('__OUT2__', [System.Drawing.Imaging.ImageFormat]::Png)
"""


def screenshot(pid, out):
    if os.name != "nt":
        return False, "screenshots are taken on Windows only"
    ps = SHOT_PS.replace("__PID__", str(int(pid))).replace("__OUT__", str(out)).replace("__OUT2__", str(out.with_name(out.stem + "-screen.png")))
    r = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps], capture_output=True, text=True, timeout=60)
    return out.is_file(), (r.stderr.strip()[:200] or str(out))


def read_json(p):
    try:
        return json.loads(Path(p).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def events(den_dir, since):
    out = []
    try:
        for line in (den_dir / "den-lite.events").read_text(encoding="utf-8").splitlines():
            try:
                e = json.loads(line)
            except ValueError:
                continue
            if e.get("t", 0) >= since:
                out.append(e)
    except OSError:
        pass
    return out


def milestones(rep, evs, start, actions):
    """PASS / FAIL per milestone with its time from the start, and which way the game signed in."""
    first = {}
    for e in evs:
        if e.get("detail") == "remote":
            continue
        first.setdefault(e["ev"], e)
    win = next((t for t, a in actions if "window is open" in a), None)
    rep.ok(win is not None, "the game window opened", "%.1f s" % (win / 1000) if win is not None else "", "The game didn't start: check the game folder in the launcher.")
    for ev, what in MILESTONES:
        e = first.get(ev)
        detail = "%.1f s" % ((e["t"] - start) / 1000) if e else "not seen"
        if e and ev == "login_challenge":
            detail += ", by %s" % ("Leyline's login screen" if e.get("who") == "DENAUTO" else "the launcher's keys" if e.get("who") == "DEN" else e.get("who", "?"))
        rep.ok(e is not None, what, detail, FIXES.get(ev, ""))
    keys = [a for _, a in actions if a.startswith("send ")]
    rep.add("INFO", "keys the launcher sent", "; ".join(keys) if keys else "none (the game did every step by itself)")


FIXES = {
    "login_challenge": "The login screen didn't sign in: run den-check, then send logs\\den-test-latest.txt.",
    "auth_ok": "Den Lite refused the password: update Leyline and Den Lite, then run den-test again.",
    "realm_list": "The realm list wasn't asked for: send logs\\den-test-latest.txt.",
    "world_auth": "The game stayed on the realm list: send logs\\den-test-latest.txt.",
    "char_enum": "Character select didn't load: send logs\\den-test-latest.txt.",
    "player_login": "Character select didn't enter the world: send logs\\den-test-latest.txt.",
    "in_world": "Entering the world didn't finish: send logs\\den-test-latest.txt and the screenshot.",
    "first_move": "The game didn't take the test's step forward (it may not have had the keys): send the log.",
    "builder_ready": "The Leyline UI didn't talk to Den Lite: install the Leyline addons in the launcher (Mods), then run den-test again.",
}


def invite_friend(rep, den, den_dir):
    """A friend through Den Lite's friends' listener (loopback): their login made, their game (the headless one)
    enters, and Den Lite shows them to the real game."""
    r = subprocess.run([str(den), "guest", "add", "--dens", str(den_dir), "--name", "Rowan"], capture_output=True, text=True, timeout=20,
                       creationflags=0x08000000 if os.name == "nt" else 0)
    try:
        g = json.loads(r.stdout)
    except ValueError:
        rep.add("FAIL", "a friend's login", (r.stderr or r.stdout).strip()[:200], "Update Leyline (Den Lite with friends), then run den-test --invite again.")
        return
    try:
        lport, wport = (int(x) for x in (den_dir / "den-lite.remote").read_text().split())
    except (OSError, ValueError):
        rep.add("FAIL", "the friends' listener", "den-lite.remote missing", "Update Leyline (Den Lite with friends).")
        return
    since = int(time.time() * 1000)
    try:
        K, _ = wowclient.login("127.0.0.1", lport, g["user"], g["pass"])
        w = wowclient.World("127.0.0.1:%d" % wport, g["user"], K)
        w.until(wowclient.SMSG_AUTH_RESPONSE)
        w.send(wowclient.CMSG_CHAR_ENUM)
        b = w.until(wowclient.SMSG_CHAR_ENUM)
        import struct
        w.send(wowclient.CMSG_PLAYER_LOGIN, struct.pack("<Q", struct.unpack_from("<Q", b, 1)[0]))
        w.until(wowclient.SMSG_LOGIN_VERIFY_WORLD)
        rep.ok(True, "the friend entered your Den", "as %s" % g["name"])
        seen = None
        for _ in range(20):
            seen = next((e for e in events(den_dir, since) if e["ev"] == "peer_seen" and e.get("detail") == g["name"]), None)
            if seen:
                break
            time.sleep(0.25)
        rep.ok(seen is not None, "your game was shown the friend", "seen by %s" % seen["who"] if seen else "not seen",
               "Den Lite didn't show the friend to your game: send logs\\den-test-latest.txt.")
        time.sleep(2)
        w.close()
    except Exception as e:  # noqa: BLE001
        rep.add("FAIL", "the friend entered your Den", str(e)[:200], "Send logs\\den-test-latest.txt.")
    finally:
        subprocess.run([str(den), "guest", "revoke", "all", "--dens", str(den_dir)], capture_output=True, timeout=20,
                       creationflags=0x08000000 if os.name == "nt" else 0)


def test(args, rep):
    app = Path(args.app_dir) if args.app_dir else app_dir()
    root = app / "den-lite"
    den_dir = root / "state" / "dens"
    logs = HERE / "logs"
    logs.mkdir(exist_ok=True)
    shot = logs / "den-test-latest.png"
    for p in (shot, root / "den-click.json", root / "den-click.shot"):
        try:
            p.unlink()
        except OSError:
            pass
    exe = launcher_exe()
    if not rep.ok(exe is not None and exe.is_file(), "the launcher program", str(exe), "Run \"Open Leyline Launcher.bat\" once (it puts .launcher\\Leyline.exe in place)."):
        return
    if not rep.ok(CLI_MARK in exe.read_bytes(), "the launcher has My Den's test start (den-play)", "", "Update the launcher (the release with den-play), then run den-test again."):
        return
    c = den_game(settings(app))
    if not rep.ok(c is not None, "game folder for Your Den", c.get("path") if c else "", "Add your 3.3.5a game folder in the launcher."):
        return
    hold = 60 if args.invite else 30
    start = int(time.time() * 1000)
    proc = subprocess.Popen([str(exe), "den-play", "--test", "--client", c["id"], "--hold", str(hold)],
                            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    rep.add("INFO", "started", "%s den-play --test (the same code as the My Den button)" % exe.name)
    rpt = None
    end = time.time() + args.timeout
    while time.time() < end:
        rpt = read_json(root / "den-click.json")
        if rpt and rpt.get("start", 0) >= start - 5000 and rpt.get("phase") in ("in_world", "stopped", "done"):
            break
        if proc.poll() is not None:
            rpt = read_json(root / "den-click.json")
            break
        time.sleep(0.5)
    if not rpt or rpt.get("start", 0) < start - 5000:
        rep.add("FAIL", "My Den's start", "no report from the launcher", "Send %s\\den-click.log if it exists." % root)
        proc.kill()
        return
    pid = rpt.get("pid", 0)
    if rpt.get("phase") == "in_world":
        # the step forward and the Leyline UI's hello, then the friend (optional), then the picture
        for _ in range(60):
            evs = events(den_dir, rpt["start"])
            if {"first_move", "builder_ready"} <= {e["ev"] for e in evs}:
                break
            time.sleep(0.25)
        if args.invite:
            den = root / "bin" / exe_name("den-lite")
            invite_friend(rep, den, den_dir)
        time.sleep(1.5)
        ok, detail = screenshot(pid, shot)
        rep.add("INFO" if ok else "FAIL" if os.name == "nt" else "INFO", "screenshot", detail, "PowerShell couldn't capture the game window.")
    try:
        (root / "den-click.shot").write_text("taken")
    except OSError:
        pass
    try:
        proc.wait(timeout=hold + 30)
    except subprocess.TimeoutExpired:
        proc.kill()
    rpt = read_json(root / "den-click.json") or rpt
    milestones(rep, events(den_dir, rpt["start"]), rpt["start"], [tuple(a) for a in rpt.get("actions", [])])
    rep.add("INFO", "outcome", str(rpt.get("outcome")))
    rep.ok(bool(rpt.get("closed")), "the game and the test closed", "Den Lite stopped too" if rpt.get("den_lite_stopped") else "Den Lite was already running: left running",
           "The launcher didn't finish the test: close the game by hand and run den-test again.")
    game = Path(c["path"])
    z = find_ci(find_ci(game, "Data") or game, "patch-Z.MPQ")
    left = bool(z and our_archive(z)[1])
    rep.ok(not left, "the handoff is gone", "no sign-in left in the game folder" if not left else "still in Data\\patch-Z.MPQ",
           "Start and close My Den once; it takes the sign-in out.")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", choices=["check", "test"])
    ap.add_argument("--game")
    ap.add_argument("--app-dir")
    ap.add_argument("--no-login", action="store_true")
    ap.add_argument("--invite", action="store_true")
    ap.add_argument("--timeout", type=int, default=150)
    a = ap.parse_args(argv)
    rep = Report()
    title = "DEN CHECK" if a.mode == "check" else "DEN TEST"
    print("%s (%s)" % (title, time.strftime("%Y-%m-%d %H:%M")))
    (check if a.mode == "check" else test)(a, rep)
    code = rep.end(title)
    if a.mode == "test":
        logs = HERE / "logs"
        logs.mkdir(exist_ok=True)
        app = Path(a.app_dir) if a.app_dir else app_dir()
        extra = ""
        for f in ("den-click.log",):
            p = app / "den-lite" / f
            if p.is_file():
                extra += "\n--- %s ---\n%s" % (f, p.read_text(encoding="utf-8", errors="replace"))
        (logs / "den-test-latest.txt").write_text("%s (%s)\n%s\n%s%s" % (title, time.strftime("%Y-%m-%d %H:%M"), rep.text(),
                                                   "PASS" if code == 0 else "FAIL", extra), encoding="utf-8")
        print("Saved %s" % (logs / "den-test-latest.txt"))
    return code


if __name__ == "__main__":
    sys.exit(main())
