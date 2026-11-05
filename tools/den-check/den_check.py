#!/usr/bin/env python3
"""den-check and den-test: is My Den one click into the world on this PC's own game?

    py -3 tools/den-check/den_check.py check [--game <folder>]     read-only: the game folder, Leyline's login
                                                                    archive, Forever UI, Config.wtf, Den Lite and a
                                                                    sign-in through the exact steps (a Den Lite of
                                                                    its own on spare ports, a scratch folder)
    py -3 tools/den-check/den_check.py features                    `test`, then the Den editor's feature test (den_features,
                                                                    below): every editor action through the Leyline UI's own
                                                                    buttons, a screenshot after each (logs/den-features/);
                                                                    then the Dojo's combat (dojo_features: a class and
                                                                    spec fill the bars, a gear set, a practice run's
                                                                    number, the boss's circle, the range), then the
                                                                    placement aids (aids_features) and, on
                                                                    Windows, the build camera's hover calibration
                                                                    (the real cursor moved: leave the mouse alone)
    py -3 tools/den-check/den_check.py test                        the real thing, unattended: the newest Den Lite,
                                                                    then the launcher's own My Den (Leyline.exe
                                                                    den-play --test) starts Den Lite and the game,
                                                                    signs in, waits for the world, then the Den's
                                                                    smoke test (below), a screenshot, and closes the
                                                                    game and Den Lite

First, walking (walk_lines; owner 3 Nov: the character floated above its shadow and wouldn't walk while every line
passed): the launcher steps forward until Den Lite sees the character walk, then the test checks from Den Lite's
events that the game took its character, that the step walked it, that nothing holds it up or still (root, no
gravity, hover), and from `.den where` that its feet are within half a yard of the ground; the Leyline UI's own view
of the character (its speed, falling, control) is shown too. Then (keys_check, 3 Nov: no movement at all, on posted
keys or the owner's own W) W is pressed a second way, as a keyboard does (SendInput, the game in front), with the
Leyline UI's view before and after (Den Lite `.den diag`), what the game sent in the world and its time sync answer:
the report says which way, if any, moved the character.

The Den's smoke test, after "in the world", in order (den_steps; the commands go through Den Lite's own command files,
labs/den-lite testctl.rs, so they never depend on the game window having the keyboard; typed into the game window by
the launcher only when Den Lite is too old for that):
    1. build      .den place chair (Den Lite logs the change), .build undo takes it back
    2. save/load  .den save smoketest, the file is there and is a Den file, .den load smoketest replace, the test
                  save removed again (and the Den's own backup put back as it was)
    3. room       .den room white (the white room is there), .den size answers
    4. Forge      .den forge: the launcher makes Forge's files for the Den (the tile, its ground, the one tile Forge
                  may save) without opening Forge; whether Forge is installed is INFO only
    5. share      the saved Den passes the site's own check for a shared Den (stream/listings.py den_problems); an
                  upload from Leyline itself is TODO until the launcher has `den-share --dry-run` (SHARE_MARK)
    6. invite     .den invite: a link https://<relay>/join/<id>#<secret>~~<account>.<password> (never printed: the
                  log shows it with the secret and the password hidden), into the Den panel, a friend (this test's
                  own headless game) signs in with it through Den Lite's friends' listener and your game sees them;
                  the friend's login taken back after; the secret is in no log
    7. autosave   Den Lite wrote autosave.den.json (its event, or the file's time)
The Den is left as it was: the piece is undone, the load puts back what was saved, the room's look is put back.

Prints PASS / FAIL / INFO (and TODO for what isn't built yet) per step and, at the end, the one fix. `test` writes
logs/den-test-latest.txt and logs/den-test-latest.png next to this file (logs/ is not in git). Never needs a key
press; safe to run again. Nothing here prints a password or an invite's secret or reads game files beyond archive
headers and Leyline's own archive.
"""
import argparse
import hashlib
import json
import math
import os
import re
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
import urllib.request
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
# the launcher answers the Den's asks as a test (denclick.rs TEST_MARK): no relay tunnel, Forge not opened
TEST_MARK = b"leyline-den-test-v2"
# the launcher can upload a Den to the site (`Leyline den-share --dry-run --file <x.den.json>` prints one JSON line
# {"ok": true, ...} without uploading anything): not built yet, so the share check says TODO until it is
SHARE_MARK = b"leyline-den-share-v1"
SAVE = "smoketest"
# the invite link (host/litenet.rs invite_link, new_secret; labs/den-lite guests.rs): relay, realm id, the Den's
# secret, the friend's account and password
# the friend's other ways in may follow (launcher host/denways.rs with_ways: ~~lan=<ip:port>, ~~at=<host[:port]>)
INVITE_RE = re.compile(r"^https://([a-z0-9.-]{3,80})/join/([a-z2-7]{16})#([A-Z2-9]{24})~~(G[A-Z0-9]{3,15})\.([A-Z0-9]{16})((?:~~(?:lan|at)=[A-Za-z0-9.:\[\]-]{1,120})*)$")
DEN_LITE_RELEASE = "https://github.com/orionopenwow/leyline-workspace/releases/download/den-lite-latest/"
NO_WINDOW = 0x08000000 if os.name == "nt" else 0
MILESTONES = [
    ("login_challenge", "the game asked to sign in"),
    ("auth_ok", "signed in"),
    ("realm_list", "realm list sent"),
    ("world_auth", "joined Your Den"),
    ("char_enum", "character select"),
    ("player_login", "Enter World"),
    ("in_world", "in the world (your Den)"),
    ("first_move", "first movement from the game (landing, turning or walking)"),
    ("builder_ready", "the Leyline UI's Den panel and Builder answered"),
    ("den_ui", "the Den bar opened (Build, Shape ground, Play, Save, Invite)"),
]


class Report:
    def __init__(self):
        self.rows = []

    def add(self, kind, step, detail="", fix=""):
        """kind: PASS, FAIL (counts), INFO, SKIP (not run because an earlier step failed), TODO (not built yet)."""
        self.rows.append((kind, step, detail, fix))
        line = "%-4s %s" % (kind, step) + (": " + detail if detail else "")
        print(line, flush=True)
        if fix and kind in ("FAIL", "TODO"):
            print("     fix: " + fix, flush=True)

    def ok(self, cond, step, detail="", fix=""):
        self.add("PASS" if cond else "FAIL", step, detail, fix)
        return cond

    def failed(self):
        return [r for r in self.rows if r[0] == "FAIL"]

    def end(self, title):
        f = self.failed()
        print("")
        todo = [r for r in self.rows if r[0] == "TODO"]
        if todo:
            print("Not built yet (doesn't fail the test): " + "; ".join(r[1] for r in todo))
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
            if fix and kind in ("FAIL", "TODO"):
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


def loose_stock_addons(game):
    """Folders in <game>/Interface/AddOns named like the game's own built-in addons (Blizzard_*). The game expects its
    own addons from its archives; a loose copy is the likeliest cause of "Couldn't load Blizzard_CombatLog: Banned"
    at login (some UI packs and repacks leave such copies behind). Not proven on a real game yet: INFO only."""
    addons = find_ci(find_ci(game, "Interface") or game / "Interface", "AddOns")
    try:
        return sorted(e for e in os.listdir(addons) if e.lower().startswith("blizzard_") and (Path(addons) / e).is_dir()) if addons else []
    except OSError:
        return []


def stock_addons(rep, game):
    loose = loose_stock_addons(game)
    if loose:
        rep.add("INFO", "copies of the game's own addons in Interface\\AddOns", ", ".join(loose[:8]) + (" and %d more" % (len(loose) - 8) if len(loose) > 8 else "")
                + ": the likeliest cause of \"Couldn't load ...: Banned\" at login; moving them out of Interface\\AddOns should stop that message")
    else:
        rep.add("INFO", "copies of the game's own addons in Interface\\AddOns", "none")


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
    stock_addons(rep, game)
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
    "den_ui": "The Den bar didn't open (an older Leyline UI: Den Lite's lines went to the chat): update the Leyline addons in the launcher (Mods), then run den-test again.",
}


# ------------------------------------------------------------------------------------------------ walking
# movement flags that hold a character up or still (labs/den-lite movewatch.rs)
HELD = ("ROOT", "DISABLE_GRAVITY", "HOVER", "FLYING")
# the feet within this many yards of the ground: standing on it (movewatch.rs ON_GROUND)
ON_GROUND = 0.5


def walk_lines(rep, evs, where=None):
    """Does the game move its character, did the step forward walk it, and does it stand on the ground? (Owner 3 Nov:
    the character floated above its shadow and wouldn't walk, and the den-test passed: "first movement" was any
    packet.) From Den Lite's events (labs/den-lite movewatch.rs: spawn, mover, first_move, move_flags, walked) and
    its answer to `.den where` (`where`: the test-out line, with pos and ground; None when there was none)."""
    first = {}
    for e in evs:
        if e.get("detail") == "remote":
            continue
        first.setdefault(e.get("ev"), e)
    diag = [str(e.get("detail", ""))[7:] for e in evs if e.get("ev") == "addon" and str(e.get("detail", "")).startswith("wudiag ")]
    sp = first.get("spawn")
    if not sp:
        rep.add("INFO", "walking", "Den Lite doesn't say whether the character walked (an older Den Lite)")
        for d in diag[:6]:
            rep.add("INFO", "the game says", d[:200])
        return
    rep.add("INFO", "where you came in", str(sp.get("detail", ""))[:200])
    mover = first.get("mover")
    fm = first.get("first_move")
    rep.ok(mover is not None, "the game took its character",
           "it said so; Den Lite told it once more that it has control" if mover else "it never said so (no SET_ACTIVE_MOVER)",
           "The game never took its character, so it can neither walk nor fall: send logs\\den-test-latest.txt and the screenshot.")
    rep.add("INFO", "the game's first move", str(fm.get("detail", "")) if fm else "none: the game sent no movement at all")
    for e in evs:
        if e.get("ev") == "move_flags":
            d = str(e.get("detail", ""))
            held = any(h in d.split() for h in HELD)
            rep.add("FAIL" if held else "INFO", "held up or still" if held else "movement flags", d[:200],
                    "The game holds the character up or still (%s): send logs\\den-test-latest.txt." % d.split(" in ")[0] if held else "")
    walked = first.get("walked")
    if walked:
        why = ""
    elif fm:
        why = "The game moves its character but the step forward didn't walk it: see the keys line (the game says) below, then send logs\\den-test-latest.txt."
    else:
        why = "The game sent no movement at all: it isn't moving its character. Send logs\\den-test-latest.txt and the screenshot."
    # 4 Nov den-move: Your Den now opens in build mode (Den Lite's state is asked for at once), where W pans the build
    # view (Gizmo.lua LeylineGizmoPanW), so the launcher's steps forward moved the view, not the character
    pans = [e for e in evs if e.get("ev") == "addon" and str(e.get("detail", "")).startswith(("wubuild aid cam pan", "wubuild aid cam fly"))]
    built = any(e.get("ev") == "build_mode" and e.get("detail") == "build" for e in evs)
    if walked is None and built and pans and any("LeylineGizmoPan" in d for d in diag):
        rep.add("INFO", "walking: not checked", "the Den opened in build mode, where W flies the build view: the steps "
                "forward moved it %d times (play mode walks; den-test checks walking)" % len(pans))
    else:
        rep.ok(walked is not None, "walking: the step forward walked the character",
               str(walked.get("detail", "")) if walked else "it never got a yard from where it came in", why)
    if isinstance(where, dict) and isinstance(where.get("pos"), list) and len(where["pos"]) == 3 and where.get("ground") is not None:
        z, g = float(where["pos"][2]), float(where["ground"])
        ok = abs(z - g) <= ON_GROUND
        detail = "z %.2f, the ground %.2f (%s)" % (z, g, "on it" if ok else "%.2f %s it" % (abs(z - g), "above" if z > g else "under"))
    else:
        src = walked or sp
        ok = "on the ground" in str(src.get("detail", ""))
        detail = ("after walking: " if walked else "where you came in: ") + str(src.get("detail", ""))[:160]
    rep.ok(ok, "standing on the ground (feet within half a yard)", detail,
           "The character isn't on the ground: send logs\\den-test-latest.txt and the screenshot.")
    for d in diag[:6]:
        rep.add("INFO", "the game says", d[:200])


# ------------------------------------------------------------------------------------------------ the Den's smoke test

def wait_for(fn, seconds, step=0.2):
    end = time.time() + seconds
    while True:
        v = fn()
        if v or time.time() >= end:
            return v
        time.sleep(step)


def event_after(den_dir, name, since, detail=None):
    """The first Den Lite event `name` at or after `since` (unix ms), with that detail when given."""
    for e in events(den_dir, since):
        if e.get("ev") == name and (detail is None or e.get("detail") == detail):
            return e
    return None


def now_ms():
    return int(time.time() * 1000)


def shown_where(den_dir, since):
    """Where Den Lite showed a line it wasn't typed for (labs/den-lite notice.rs, its "notice" event): a toast in the
    Den bar, or the chat when the game has no Den bar (an older Leyline UI)."""
    e = wait_for(lambda: event_after(den_dir, "notice", since), 2, 0.1)
    if not e:
        return "a line in the game's chat (an older Den Lite)"
    return "shown in the Den bar" if str(e.get("detail", "")).startswith("toast") else "a line in the game's chat (no Den bar: update the Leyline addons)"


class FileChannel:
    """The test's commands through Den Lite's own files (labs/den-lite testctl.rs): never the keyboard."""
    kind = "Den Lite's command file"

    def __init__(self, den_dir):
        self.dir = Path(den_dir)
        self.n = now_ms()

    def send(self, cmd, wait=10.0):
        self.n += 1
        cid = self.n
        tmp = self.dir / "den-lite.test-cmd.tmp"
        tmp.write_text(json.dumps({"id": cid, "cmd": cmd}), encoding="utf-8")
        os.replace(tmp, self.dir / "den-lite.test-cmd")

        def answer():
            try:
                lines = (self.dir / "den-lite.test-out").read_text(encoding="utf-8").splitlines()
            except OSError:
                return None
            for line in lines:
                try:
                    v = json.loads(line)
                except ValueError:
                    continue
                if v.get("id") == cid:
                    return v
            return None
        got = wait_for(answer, wait, 0.1)
        if got is None:
            # not taken: it mustn't run later, after the test moved on
            try:
                (self.dir / "den-lite.test-cmd").unlink()
            except OSError:
                pass
        return got


class KeyChannel:
    """The fallback for a Den Lite without test commands: the launcher types the line into the game window
    (denclick.rs TYPE_FILE). What it did is only seen in Den Lite's events and files."""
    kind = "typed into the game window"

    def __init__(self, root):
        self.root = Path(root)
        self.n = now_ms()

    def send(self, cmd, wait=10.0):
        self.n += 1
        cid = self.n
        tmp = self.root / "den-click.type.tmp"
        tmp.write_text(json.dumps({"id": cid, "line": cmd}), encoding="utf-8")
        os.replace(tmp, self.root / "den-click.type")
        done = wait_for(lambda: (read_json(self.root / "den-click.typed") or {}).get("id") == cid, wait)
        if not done:
            return None
        time.sleep(1.5)  # the game sends it, Den Lite answers
        return {"keyboard": True, "typed": bool((read_json(self.root / "den-click.typed") or {}).get("ok")), "said": []}


def redact(link):
    """An invite link as the log may show it: the relay and the start of the realm id; never the secret or the
    friend's password."""
    m = INVITE_RE.match(link or "")
    if not m:
        return "(not an invite link: %d letters)" % len(link or "")
    return "https://%s/join/%s...#(secret hidden)~~%s...(password hidden)" % (m.group(1), m.group(2)[:4], m.group(4)[:2])


def said(r):
    return " / ".join((r or {}).get("said") or [])[:160]


def stash(paths):
    """The files the test may write over, as they are now (None: not there)."""
    out = {}
    for p in paths:
        try:
            out[p] = p.read_bytes()
        except OSError:
            out[p] = None
    return out


def unstash(saved):
    for p, data in saved.items():
        try:
            if data is None:
                p.unlink()
            else:
                p.write_bytes(data)
        except OSError:
            pass


def site_den_check(text):
    """The site's own check for a shared Den (stream/listings.py den_problems): (problems, props) or None when the
    site's code isn't in this folder."""
    sys.path.insert(0, str(ROOT / "stream"))
    try:
        import listings  # noqa: E402
    except Exception:  # noqa: BLE001
        return None
    finally:
        sys.path.pop(0)
    probs, props, _ = listings.den_problems(text)
    return probs, props


def den_lite_bin_for(root):
    if os.environ.get("DEN_LITE_BIN"):
        return Path(os.environ["DEN_LITE_BIN"])
    return Path(root) / "bin" / exe_name("den-lite")


def friend_joins(rep, den_dir, user, password, name, since):
    """The friend's game (the headless one) signs in with the invite's login through Den Lite's friends' listener on
    this PC (what the relay tunnel connects to), enters the Den, and Den Lite shows them to the host's game."""
    try:
        lport, wport = (int(x) for x in (Path(den_dir) / "den-lite.remote").read_text().split())
    except (OSError, ValueError):
        rep.add("FAIL", "invite: a friend signs in with it", "Den Lite has no friends' listener (den-lite.remote)",
                "Update Leyline (Den Lite with friends), then run den-test again.")
        return
    w = None
    try:
        K, _ = wowclient.login("127.0.0.1", lport, user, password)
        w = wowclient.World("127.0.0.1:%d" % wport, user, K)
        b = w.until(wowclient.SMSG_AUTH_RESPONSE)
        if b[0] != 0x0C:
            raise RuntimeError("the world server said no (code %d)" % b[0])
        w.send(wowclient.CMSG_CHAR_ENUM)
        b = w.until(wowclient.SMSG_CHAR_ENUM)
        w.send(wowclient.CMSG_PLAYER_LOGIN, struct.pack("<Q", struct.unpack_from("<Q", b, 1)[0]))
        w.until(wowclient.SMSG_LOGIN_VERIFY_WORLD)
        rep.ok(True, "invite: a friend signs in with it", "as %s, through the friends' listener" % (name or "the friend"))
        seen = wait_for(lambda: next((e for e in events(den_dir, since) if e["ev"] == "peer_seen" and e.get("detail") == name and e.get("who") != name), None), 6)
        rep.ok(seen is not None, "invite: your game was shown the friend", "seen by %s" % seen["who"] if seen else "not seen",
               "Den Lite didn't show the friend to your game: send logs\\den-test-latest.txt.")
        time.sleep(1)
    except Exception as e:  # noqa: BLE001 - reported in plain words
        rep.add("FAIL", "invite: a friend signs in with it", str(e)[:200].replace(password, "(password)"),
                "Den Lite didn't take the friend's login from the link: send logs\\den-test-latest.txt.")
    finally:
        if w:
            w.close()


def den_steps(rep, ch, den_dir, root, launcher=None, start=None):
    """The Den's smoke test after "in the world" (module doc, steps 1 to 7). `ch`: how the commands get there
    (FileChannel; KeyChannel only after FileChannel didn't answer). `launcher`: the launcher program's bytes' marks
    (TEST_MARK, SHARE_MARK) as a set; `start`: unix ms of this start. Returns the invite's secrets, so the caller can
    check that no log holds them."""
    den_dir, root = Path(den_dir), Path(root)
    launcher = launcher or set()
    start = start or now_ms()
    secrets = []

    def skip_all(why, steps):
        for s in steps:
            rep.add("SKIP", s, why)

    STEPS = ["build: place a piece", "build: .build undo takes it back", "save and load", "room: the white room",
             "Forge: the editor's files", "share: a Den file the site takes", "invite: a link for one friend", "autosave"]
    state = ch.send(".den size")
    if state is None and isinstance(ch, FileChannel):
        if TEST_MARK in launcher and os.name == "nt":
            rep.add("INFO", "the test's commands", "Den Lite didn't take them from its folder (an older Den Lite): the launcher types them into the game instead")
            ch = KeyChannel(root)
            state = ch.send(".den size")
        if state is None:
            rep.add("FAIL", "the test's commands", "Den Lite didn't answer the test's first command (.den size)",
                    "Close your game, then run den-test again (it gets the newest Den Lite first). If it fails again, send logs\\den-test-latest.txt.")
            skip_all("the test's commands didn't reach Den Lite", STEPS)
            return secrets
    keys = isinstance(ch, KeyChannel)
    rep.add("INFO", "the test's commands", "through %s" % ch.kind)
    orig = {"look": state.get("look"), "props": state.get("props"), "size": state.get("size")}
    if not keys:
        rep.add("INFO", "your Den before the test", "%s props, the %s room, size %s" % (orig["props"], orig["look"], orig["size"]))

    # 1. build
    t = now_ms()
    r = ch.send(".den place chair")
    ev = wait_for(lambda: event_after(den_dir, "den_change", t), 4)
    if keys:
        rep.ok(ev is not None, "build: place a piece", ".den place chair, Den Lite logged it (%s)" % (ev or {}).get("detail", "-"),
               "Den Lite didn't place the chair: send logs\\den-test-latest.txt.")
    else:
        placed = bool(r and r.get("created", 0) >= 1 and r.get("props") == orig["props"] + 1)
        rep.ok(placed, "build: place a piece", ".den place chair: %s (%s more prop)" % (said(r) or "no answer", (r or {}).get("created", 0)),
               "Den Lite didn't place the chair: send logs\\den-test-latest.txt.")
        rep.ok(ev is not None, "build: Den Lite logged it", "den_change %s" % ev["detail"] if ev else "no den_change event",
               "Den Lite didn't log the change: update Leyline, then run den-test again.")
    # the piece stays until the autosave (every 2 seconds when the Den changed) has kept it: step 7
    placed_at = t
    auto_ev = wait_for(lambda: event_after(den_dir, "autosave", placed_at), 6)
    t = now_ms()
    r = ch.send(".build undo")
    if keys:
        ev = wait_for(lambda: event_after(den_dir, "den_change", t), 4)
        rep.ok(ev is not None and ev.get("detail", "").endswith("-1"), "build: .build undo takes it back", (ev or {}).get("detail", "no change seen"),
               "Undo didn't take the chair back: send logs\\den-test-latest.txt.")
    else:
        rep.ok(bool(r and r.get("destroyed", 0) >= 1 and r.get("props") == orig["props"]), "build: .build undo takes it back", said(r) or "no answer",
               "Undo didn't take the chair back: send logs\\den-test-latest.txt.")

    # 2. save and load (the test's save and the Den's own backup are put back as they were)
    save_file = den_dir / ("%s.den.json" % SAVE)
    kept = stash([save_file, den_dir / "backup.den.json"])
    text = None
    try:
        t = now_ms()
        r = ch.send(".den save %s" % SAVE)
        if keys:
            wait_for(lambda: save_file.is_file() and save_file.stat().st_mtime * 1000 >= t - 2000, 5)
        try:
            text = save_file.read_text(encoding="utf-8")
            doc = json.loads(text)
        except (OSError, ValueError):
            doc = None
        fresh = save_file.is_file() and save_file.stat().st_mtime * 1000 >= t - 2000
        good = bool(doc and doc.get("format") == "leyline-den" and isinstance(doc.get("props"), list) and fresh)
        rep.ok(good, "save: .den save %s" % SAVE, "%s, %d props" % (save_file.name, len(doc["props"])) if good else (said(r) or "no file"),
               "Den Lite didn't write the Den file: check that %s can be written, then run den-test again." % den_dir)
        if good:
            t = now_ms()
            r = ch.send(".den load %s replace" % SAVE)
            if keys:
                ev = wait_for(lambda: event_after(den_dir, "den_load", t), 5)
                rep.ok(ev is not None, "load: .den load %s replace" % SAVE, (ev or {}).get("detail", "not seen"), "Den Lite didn't open the file: send logs\\den-test-latest.txt.")
            else:
                ok = bool(r and r.get("props") == len(doc["props"]) == orig["props"])
                rep.ok(ok, "load: .den load %s replace" % SAVE, "%s (%s props, as before)" % (said(r) or "no answer", (r or {}).get("props")),
                       "Den Lite didn't open the file it saved: send logs\\den-test-latest.txt.")
        else:
            rep.add("SKIP", "load: .den load %s replace" % SAVE, "nothing was saved")
    finally:
        unstash(kept)
    rep.ok(kept[save_file] is not None or not save_file.exists(), "save: the test save removed", "your own Saved Dens are as they were",
           "Delete %s by hand." % save_file)

    # 3. the white room
    r = ch.send(".den room white")
    if keys:
        rep.add("INFO", "room: the white room", "typed .den room white (its answer is in the game's chat)")
    else:
        rep.ok(bool(r and r.get("look") == "white" and any("white" in s for s in r.get("said", []))), "room: the white room", said(r) or "no answer",
               "Den Lite didn't build the white room: send logs\\den-test-latest.txt.")
        sz = ch.send(".den size")
        line = said(sz)
        rep.ok("yard room" in line, "room: its size", line or "no answer", "Den Lite didn't say how big the Den is: send logs\\den-test-latest.txt.")

    # 4. Forge: the editor's files made by the launcher, Forge itself never opened by a test
    if TEST_MARK not in launcher:
        rep.add("FAIL", "Forge: the editor's files", "the launcher is from before this test (it would open Forge for real)",
                "Run den-test from \"Den test.bat\" (it gets the newest launcher first).")
    else:
        rep_file = root / "den-click.forge.json"
        try:
            rep_file.unlink()
        except OSError:
            pass
        t = now_ms()
        ch.send(".den forge")
        asked = wait_for(lambda: event_after(den_dir, "ask", t, "forge"), 5)
        rep.ok(asked is not None, "Forge: the Den asked the launcher", ".den forge" if asked else "no ask from Den Lite",
               "Den Lite didn't pass Shape the ground to the launcher: send logs\\den-test-latest.txt.")
        fr = wait_for(lambda: read_json(rep_file), 40, 0.5) if asked else None
        if fr is None:
            rep.add("FAIL", "Forge: the editor's files", "the launcher didn't answer", "Send logs\\den-test-latest.txt (the launcher's part of the test).")
        else:
            rep.ok(bool(fr.get("ok")) and not fr.get("opened"), "Forge: the editor's files",
                   "your Den's tile %s (%s bytes), its ground and the one tile Forge may save, %s" % (fr.get("tile", "?"), fr.get("tile_bytes", 0),
                                                                                                     "with %s drawn in" % fr["den_file"] if fr.get("den_file") else "no saved Den to draw")
                   if fr.get("ok") else str(fr.get("error", "not made"))[:200],
                   "Forge's files for your Den couldn't be made: check your game folder in the launcher, then run den-test again.")
            rep.add("INFO", "Forge (the editor)", "installed (not opened by the test)" if fr.get("installed") else
                    "not installed: Your Den > Shape the ground > Get Forge (the test doesn't need it)")
        ans = wait_for(lambda: event_after(den_dir, "answer", t, "say"), 6) if fr is not None else None
        rep.ok(ans is not None, "Forge: the answer reached your game", shown_where(den_dir, ans["t"]) if ans else "not seen",
               "Den Lite didn't pass the launcher's answer into the game: send logs\\den-test-latest.txt.")
        try:
            rep_file.unlink()
        except OSError:
            pass

    # 5. share: the saved Den is what the site takes; the upload from Leyline itself isn't built yet
    if text is None:
        rep.add("SKIP", "share: a Den file the site takes", "nothing was saved")
    else:
        got = site_den_check(text)
        if got is None:
            rep.add("INFO", "share: a Den file the site takes", "the site's check isn't in this folder (stream/listings.py)")
        else:
            probs, props = got
            rep.ok(not probs, "share: a Den file the site takes", "the site's own check passes (%d props)" % props if not probs else "; ".join(probs)[:200],
                   "The site would refuse this Den file: send logs\\den-test-latest.txt.")
    exe = launcher_exe()
    if SHARE_MARK in launcher and exe and text is not None:
        tmpd = Path(tempfile.mkdtemp(prefix="den-share-"))
        try:
            f = tmpd / ("%s.den.json" % SAVE)
            f.write_text(text, encoding="utf-8")
            r = subprocess.run([str(exe), "den-share", "--dry-run", "--file", str(f)], capture_output=True, text=True, timeout=60, creationflags=NO_WINDOW)
            try:
                out = json.loads(r.stdout.strip().splitlines()[-1])
            except (ValueError, IndexError):
                out = {}
            rep.ok(r.returncode == 0 and out.get("ok") is True, "share: upload a Den from Leyline (dry run)", str(out.get("message", r.stderr.strip()))[:200],
                   "Leyline's Den upload didn't pass its dry run: send logs\\den-test-latest.txt.")
        finally:
            shutil.rmtree(tmpd, ignore_errors=True)
    else:
        rep.add("TODO", "share: upload a Den from Leyline", "not built yet",
                "Today: put the .den.json on a GitHub release and list it on the site (List your mod). This check turns into PASS / FAIL once the launcher has den-share.")

    # 6. invite: the link (never printed), into the Den panel, a friend signs in with it, your game sees them
    if TEST_MARK not in launcher:
        rep.add("FAIL", "invite: a link for one friend", "the launcher is from before this test (it would open the Den to the network)",
                "Run den-test from \"Den test.bat\" (it gets the newest launcher first).")
    else:
        inv_file = root / "den-click.invite"
        try:
            inv_file.unlink()
        except OSError:
            pass
        t = now_ms()
        ch.send(".den invite")
        inv = wait_for(lambda: read_json(inv_file), 30, 0.5)
        try:
            inv_file.unlink()
        except OSError:
            pass
        link = (inv or {}).get("link", "")
        m = INVITE_RE.match(link)
        if m:
            secrets.extend([m.group(3), m.group(5)])
        rep.ok(m is not None, "invite: a link for one friend", redact(link) if inv else "no link from the launcher",
               "The launcher didn't make an invite link: open Your Den in the launcher once, then run den-test again."
               if not inv else "The invite link isn't in the right shape: send logs\\den-test-latest.txt.")
        ans = wait_for(lambda: event_after(den_dir, "answer", t, "invite"), 6) if inv else None
        rep.ok(ans is not None, "invite: the link went into your Den panel", ("passed into the game, " + shown_where(den_dir, ans["t"])) if ans else "not seen",
               "Den Lite didn't pass the link into the game: send logs\\den-test-latest.txt.")
        if m:
            name = guest_name(den_dir, m.group(4))
            friend_joins(rep, den_dir, m.group(4), m.group(5), name, t)
        else:
            rep.add("SKIP", "invite: a friend signs in with it", "no link")
        if m:
            den = den_lite_bin_for(root)
            r = subprocess.run([str(den), "guest", "revoke", "all", "--dens", str(den_dir)], capture_output=True, text=True, timeout=20,
                               creationflags=NO_WINDOW) if den.is_file() else None
            rep.ok(bool(r and r.returncode == 0), "invite: the friend's login taken back", (r.stdout.strip() if r else "no Den Lite program at %s" % den)[:80],
                   "Press Stop sharing on Your Den in the launcher (it takes every friend's login back).")

    # 7. autosave
    auto = den_dir / "autosave.den.json"
    ev = auto_ev or event_after(den_dir, "autosave", start)
    fresh = auto.is_file() and auto.stat().st_mtime * 1000 >= placed_at - 1000
    rep.ok(ev is not None or fresh, "autosave", ("Den Lite kept the Den by itself (%s)" % ev["detail"]) if ev else "autosave.den.json written" if fresh else "not seen",
           "Den Lite didn't keep the Den by itself: check that %s can be written, then send logs\\den-test-latest.txt." % den_dir)

    # the Den as it was: the room's look
    if orig.get("look") and orig["look"] != "white":
        ch.send(".den room %s" % orig["look"])
    return secrets


# ------------------------------------------------------------------------------------------------ the editor's feature test

UI_REPORT = "den-lite.ui-report"
# five props of different kinds from Den Lite's catalog (labs/den-lite objects.rs): a chair, a barrel, a campfire,
# flowers, a bed
FEATURE_PROPS = [(2413, "Wooden Chair"), (1670, "Barrel of Powder"), (1953, "Campfire"), (2109, "Fragrant Flowers"), (13948, "Fancy Bed")]
FEATURE_SAVE = "featuretest"
# the stamps (realm/custom/lua_scripts/den-stamps, made by tools/den-stamps/compose.py): den-features builds each and
# takes a picture so the owner can judge them (docs/DEN-STAMPS.md)
STAMP_NAMES = ["campfire", "inn", "market", "garden", "dock", "house", "smithy", "nook", "shrine"]
STAMP_DIR = ROOT / "realm" / "custom" / "lua_scripts" / "den-stamps"


def stamp_pieces(name, folder=STAMP_DIR):
    """How many pieces a built-in stamp has (its den-stamps file; Den Lite builds the same in), else None."""
    try:
        return len(json.loads((Path(folder) / ("%s.den.json" % name)).read_text(encoding="utf-8"))["props"])
    except (OSError, ValueError, KeyError, TypeError):
        return None


def stamp_pictures(rep, ch, den_dir, shot, wait=6.0, names=STAMP_NAMES, build_wait=60.0):
    """Each stamp built in front of you (`.den jaina build <name>`: Jaina builds it piece by piece when she is out, else
    it goes down at once), a picture once it is all there (logs/den-features/NN-stamp-<name>.png), then one undo."""
    def size():
        return ((ch.send(".den size") or {}).get("props")) or 0
    for name in names:
        p0 = size()
        t = now_ms()
        r = ch.send(".den jaina build %s" % name) or {}
        if wait_for(lambda: event_after(den_dir, "den_change", t), wait) is None:
            said = " ".join(str(x) for x in r.get("said", []))[:160]
            rep.add("FAIL", "stamp: %s" % name, "nothing was placed" + (": " + said if said else ""),
                    "Update Den Lite (stamps are moved inside the room since 3 Nov), then send logs\\den-features-latest.txt.")
            continue
        if not any("Call your companion" in x for x in r.get("said", [])):
            # she is out: wait for her to finish (the dojo event "built <n>")
            wait_for(lambda: next((e for e in events(den_dir, t) if e.get("ev") == "dojo" and str(e.get("detail", "")).startswith("built")), None),
                     build_wait, 0.5)
        n = size() - p0
        pic = shot("stamp-%s" % name)
        whole = stamp_pieces(name)
        # all of it inside the room (Den Lite fits a stamp in: den.rs fit_stamp); a part left out fails
        rep.ok(n > 0 and (whole is None or n >= whole), "stamp: %s" % name,
               "%d pieces%s%s" % (n, (" of %d" % whole) if whole else "", (", " + pic) if pic else ""),
               "The stamp didn't go down: send logs\\den-features-latest.txt." if n <= 0 else
               "Only part of it is inside your Den: update Den Lite (it moves a stamp inside the room), then send logs\\den-features-latest.txt.")
        ch.send(".build undo")
        wait_for(lambda: size() <= p0, wait, 0.5)


def ui_lines(den_dir, since):
    """The Leyline UI's report lines (Den Lite writes them to den-lite.ui-report: testctl.rs write_ui) since `since`."""
    out = []
    try:
        for line in (Path(den_dir) / UI_REPORT).read_text(encoding="utf-8").splitlines():
            try:
                v = json.loads(line)
            except ValueError:
                continue
            if v.get("t", 0) >= since:
                out.append(str(v.get("report", "")))
    except OSError:
        pass
    return out


def parse_report(lines):
    """The report's lines (DenBar.lua D.Report) as {first word: {key: value}} plus the raw text under "_text"."""
    out = {}
    for l in lines:
        head, _, rest = l.partition(" ")
        kv = {k: v for k, v in re.findall(r"(\w+)=(\S*)", rest)}
        kv["_text"] = rest
        out[head] = kv
        # a head that comes more than once: every line, in order
        out.setdefault("_lines", {}).setdefault(head, []).append(rest)
    return out


# The gallery's tiles and the selection's preview show props from the SIDE: a PlayerModel framed by its own camera
# (the owner's PC sweep of 3 Nov, picture "fit-5-playermodel-own"; Builder.lua B.VIEW_FRAME). In either, a right-drag
# turns the model and the wheel zooms (owner 3 Nov). The report's "view" line: tile=<frame>,<has model>,<turn>,<zoom>,
# <facing> and the same for the preview.
VIEW_FRAME = "PlayerModel"


def parse_view(text):
    """A view's part of the report ("PlayerModel,1,1.57,1.32,2.17") as a dict, or {}."""
    parts = str(text or "").split(",")
    if len(parts) < 5:
        return {}
    try:
        return {"frame": parts[0], "model": parts[1] == "1", "turn": float(parts[2]), "zoom": float(parts[3]), "facing": float(parts[4])}
    except ValueError:
        return {}


def view_checks(rep, report, ui, shot, which, label):
    """The model view `which` ("tile" / "preview"): from the side, then turned 90 degrees and zoomed in two wheel clicks
    by the addon's own handlers (DenBar.lua `view` action), a picture of it."""
    v = parse_view(report(which).get("view", {}).get(which))
    if not v:
        rep.add("FAIL", "%s: from the side" % label, "no view in the addon's report", "Update the Leyline addons, then run den-features again.")
        return
    rep.ok(v["frame"] == VIEW_FRAME and v["model"], "%s: from the side" % label,
           "a %s with its own framing%s" % (v["frame"], "" if v["model"] else ", but no model in it"),
           "Send the pictures in logs\\den-features (the tiles should show props from the side).")
    ui("view %s turn 90" % which)
    ui("view %s zoom 2" % which)
    after = parse_view(report(which + "-turned").get("view", {}).get(which))
    pic = shot("%s-turned-zoomed" % which)
    ok = bool(after) and abs(((after["turn"] - v["turn"]) % 6.2832) - 1.5708) < 0.05 and after["zoom"] > v["zoom"] * 1.2
    rep.ok(ok, "%s: right-drag turns it, the wheel zooms" % label,
           ("turned to %.2f rad, zoom %.2f%s" % (after["turn"], after["zoom"], (", " + pic) if pic else "")) if after else "no view after",
           "The model didn't turn or zoom: send logs\\den-features-latest.txt and the pictures.")


GUIDE_PAGES = 10   # at least: Build and Play, gallery, placing, selecting, stamps, Dojo, class, invite, keys (Guide.lua)


def guide_checks(rep, ch, report, ui, shot):
    """The Den guide (WrathUnleashedUI Guide.lua): Guide on the bar opens it at its first page, Next turns it, and
    `.den help` in the chat opens it (Den Lite answers `wuden guide open` when the game has the Leyline UI)."""
    ui("guide close")
    ui("guide")
    g = report("guide").get("guide", {})
    pic = shot("guide")
    rep.ok(g.get("open") == "1" and int(g.get("pages", "0") or 0) >= GUIDE_PAGES, "guide: Guide on the Den bar opens it",
           "page %s of %s%s" % (g.get("page"), g.get("pages"), (", " + pic) if pic else "") if g else "no guide in the addon's report",
           "Update the Leyline addons, then run den-features again.")
    ui("guide next")
    g2 = report("guide-next").get("guide", {})
    rep.ok(g2.get("page") == "2", "guide: Next turns the page", "page %s (%s)" % (g2.get("page"), g2.get("key")), "Send logs\\den-features-latest.txt.")
    ui("guide close")
    said = " ".join(str(x) for x in ((ch.send(".den help") or {}).get("said") or []))
    g3 = report("guide-help").get("guide", {})
    rep.ok(g3.get("open") == "1", "guide: .den help opens it", said[:120] or "nothing said",
           "Update Den Lite and the Leyline addons, then run den-features again.")
    ui("guide close")


def den_features(rep, ch, den_dir, shoot, outdir, settle=1.0, wait=6.0):
    """The Den editor, feature by feature, through the Leyline UI's own buttons (Den Lite's `.den ui <action>` asks the
    addon to press them: DenBar.lua D.TestAction), a screenshot after each into `outdir`, PASS / FAIL from Den Lite's
    events and counts and the addon's own report (`.den ui report`). Your Den is saved first and put back after.
    `shoot(path)` takes a picture (den_check.screenshot on Windows)."""
    den_dir, outdir = Path(den_dir), Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    for old in outdir.glob("*.png"):
        try:
            old.unlink()
        except OSError:
            pass
    n = [0]

    def shot(name):
        n[0] += 1
        out = outdir / ("%02d-%s.png" % (n[0], name))
        ok, _ = shoot(out)
        return out.name if ok else None

    def ui(action):
        r = ch.send(".den ui %s" % action)
        time.sleep(settle)
        return r

    def report(mark=""):
        t = now_ms()
        ui("report %s" % mark if mark else "report")
        lines = wait_for(lambda: (lambda ls: ls if any(x.startswith("screen ") for x in ls) else None)(ui_lines(den_dir, t)), wait, 0.2) or ui_lines(den_dir, t)
        return parse_report(lines)

    def props():
        r = ch.send(".den size")
        return (r or {}).get("props")

    def changed(since):
        return wait_for(lambda: event_after(den_dir, "den_change", since), wait)

    first = ch.send(".den size")
    if first is None:
        rep.add("SKIP", "the editor's feature test", "Den Lite didn't take the test's commands")
        return False
    before = den_dir / ("%s-before.den.json" % FEATURE_SAVE)
    save_file = den_dir / ("%s.den.json" % FEATURE_SAVE)
    kept = stash([before, save_file, den_dir / "backup.den.json"])
    ch.send(".den save %s-before" % FEATURE_SAVE)
    try:
        # 0. the addon answers at all (a Leyline UI from before this test doesn't)
        r = report("start")
        if "state" not in r:
            rep.add("FAIL", "editor: the Leyline UI answers the test", "no report from the addon",
                    "Update the Leyline addons (Ship, then the launcher puts them in), then run den-features again.")
            return False
        rep.add("INFO", "editor: the screen", r.get("screen", {}).get("_text", "?"))
        ov = r.get("overlaps", {}).get("_text", "?")
        rep.ok(ov == "none", "editor: nothing over the minimap, chat, bars or player frame", ov,
               "Send logs\\den-features-latest.txt and the pictures in logs\\den-features.")

        # 1. the gallery
        ui("gallery")
        r = report("gallery")
        g, c = r.get("gallery", {}), r.get("catalog", {})
        pic = shot("gallery")
        rep.ok(g.get("shown") == "1" and int(g.get("tiles", "0")) > 0, "gallery: opens with tiles",
               "%s tiles (%s x %s of %s)%s" % (g.get("tiles"), g.get("cols"), g.get("rows"), g.get("tile"), (", " + pic) if pic else ""),
               "The gallery didn't open: send logs\\den-features-latest.txt.")
        rep.ok(g.get("empty") == "0", "gallery: every tile a model or an icon on purpose", "%s with a model, %s with an icon, %s empty" % (
            g.get("model"), g.get("icon"), g.get("empty")), "Some tiles are blank: send the gallery picture in logs\\den-features.")
        models = int(c.get("model", "0") or 0)
        table = (den_dir / "GameObjectDisplayInfo.dbc").is_file()
        known = event_after(den_dir, "models", 0)
        rep.ok(models > 0, "gallery: the props' models from your game files",
               "%s of %s props have a model (%s sized, %s buildings, %s without); Den Lite knows %s models" % (
                   c.get("model"), c.get("n"), c.get("boxed"), c.get("wmo"), c.get("none"), (known or {}).get("detail", "?")),
               "The gallery has no models: the launcher copies GameObjectDisplayInfo.dbc from your game into the Dens folder (%s) at the Den's start. "
               "Check your 3.3.5a game folder in the launcher, then run den-features again." % ("it is there" if table else "it is missing"))
        if "nomodel" in r:
            rep.add("INFO", "gallery: props without a model in your files", r["nomodel"]["_text"][:180])
        # the tiles: from the side, and a right-drag / the wheel on one
        view_checks(rep, report, ui, shot, "tile", "gallery tiles")
        ui("page 1")
        shot("gallery-page2")
        ui("tab all")

        # 2. five props placed from the gallery (pick, then Place)
        start_props = props() or 0
        placed = 0
        for pid, name in FEATURE_PROPS:
            t = now_ms()
            ui("place %d" % pid)
            ev = changed(t)
            ok = ev is not None and str(ev.get("detail", "")).startswith("+1")
            placed += 1 if ok else 0
            rep.ok(ok, "place: %s from the gallery" % name, (ev or {}).get("detail", "no change"),
                   "Den Lite didn't place %s (%d): send logs\\den-features-latest.txt." % (name, pid))
        now_props = props() or 0
        rep.ok(now_props == start_props + placed and placed == len(FEATURE_PROPS), "place: five different props", "%s props now (were %s)" % (now_props, start_props),
               "Not every prop was placed: send logs\\den-features-latest.txt.")
        pic = shot("placed")
        r = report("placed")
        rep.add("INFO", "the status pill", r.get("pill", {}).get("_text", "-")[:160] + ((", " + pic) if pic else ""))

        # 3. select, move, turn (the popover's buttons and its keys)
        ui("nogallery")
        ui("select")
        r = report("select")
        sel = r.get("state", {}).get("sel", "-")
        rep.ok(sel not in ("-", ""), "select: the prop nearest you", sel, "Select picked nothing: send logs\\den-features-latest.txt.")
        ui("props")
        shot("selected")
        view_checks(rep, report, ui, shot, "preview", "the selection's preview")
        for action, what in (("nudge z 1", "move: up by the step (the popover's +)"), ("nudge x -1", "move: back along x"),
                             ("key q", "turn: Q"), ("nudge turn 1", "turn: the popover's +"), ("nudge scale 1", "size: the popover's +")):
            t = now_ms()
            ui(action)
            ev = changed(t)
            rep.ok(ev is not None, what, (ev or {}).get("detail", "no change"), "Den Lite didn't change the prop: send logs\\den-features-latest.txt.")
        shot("moved")

        # 4. copy, paste, duplicate, floor, delete, undo, redo (the menu's rows and the strip's buttons)
        for action, what, delta in (("menu copy", "copy", 0), ("menu paste", "paste: one more", 1), ("menu duplicate", "duplicate: one more", 1),
                                    ("menu drop to the floor", "floor", 0), ("menu delete", "delete: one fewer", -1),
                                    ("undo", "undo: it is back", 1), ("redo", "redo: deleted again", -1)):
            p0 = props() or 0
            t = now_ms()
            ui(action)
            if delta:
                got = wait_for(lambda: (props() or 0) == p0 + delta, wait, 0.5)
                rep.ok(bool(got), what, "%s -> %s props" % (p0, props()), "Den Lite didn't %s: send logs\\den-features-latest.txt." % what.split(":")[0])
            else:
                r = report(action)
                st = r.get("state", {})
                ok = st.get("clip") == "1" if action == "menu copy" else changed(t) is not None
                rep.ok(ok, what, st.get("_text", "")[:120] if action == "menu copy" else "the prop moved down", "Send logs\\den-features-latest.txt.")
        r = report("counts")
        st = r.get("state", {})
        rep.add("INFO", "undo / redo counts", "Undo %s, Redo %s" % (st.get("undo"), st.get("redo")))
        shot("edited")

        # 5. save and open by name (Files > This Den)
        t = now_ms()
        ui("save %s" % FEATURE_SAVE)
        ok = wait_for(lambda: save_file.is_file() and save_file.stat().st_mtime * 1000 >= t - 2000, wait)
        rep.ok(bool(ok), "files: Save by name", save_file.name if ok else "no file", "Den Lite didn't write the Den file: send logs\\den-features-latest.txt.")
        pic = shot("files")
        t = now_ms()
        ui("open %s" % FEATURE_SAVE)
        ev = wait_for(lambda: event_after(den_dir, "den_load", t), wait)
        rep.ok(ev is not None, "files: Open by name", (ev or {}).get("detail", "not opened"), "Den Lite didn't open the save: send logs\\den-features-latest.txt.")

        # 6. every panel, a picture of each
        for action, name in (("close", None), ("menu", "menu"), ("close", None), ("snap", "snap"), ("close", None), ("keys", "keys"),
                             ("close", None), ("rail stamps", "stamps"), ("close", None), ("play", "play"), ("build", "building")):
            ui(action)
            if name:
                pic = shot(name)
                r = report(name)
                opened = r.get("open", {}).get("_text", "")
                rep.add("INFO", "panel: %s" % name, (opened or "-")[:150] + ((", " + pic) if pic else ""))
        # the Den guide: the bar's Guide button opens it, its pages turn, `.den help` opens it too
        guide_checks(rep, ch, report, ui, shot)
        # 7. every stamp, built by Jaina (or put down at once without her), a picture of each, then taken back
        stamp_pictures(rep, ch, den_dir, shot, wait)

        r = report("end")
        rep.ok(r.get("screen", {}).get("play") == "false", "play mode: and back to building", r.get("screen", {}).get("_text", "?"),
               "Back to building didn't come back: send logs\\den-features-latest.txt.")
        ov = r.get("overlaps", {}).get("_text", "?")
        rep.ok(ov == "none", "editor: still nothing over the game's own frames", ov, "Send the pictures in logs\\den-features.")
        ui("close")
        return True
    finally:
        # your Den as it was
        if before.is_file():
            ch.send(".den load %s-before replace" % FEATURE_SAVE)
        unstash(kept)
        for f in (before, save_file):
            if kept.get(f) is None:
                try:
                    f.unlink()
                except OSError:
                    pass
        rep.add("INFO", "your Den after the feature test", "put back as it was (%s props)" % props())


# ------------------------------------------------------------------------------------------------ the Dojo's combat
# Owner 3 Nov: "spellbook placement loadout instant on bars, gear sets selector ... better combat and test range dojo
# features". dojo_features drives Den Lite's sandbox (labs/den-lite sandbox.rs, combat.rs) through its own commands
# and reads its events: a class and spec fill the spellbook and every bar, a bar loadout is kept and put back, a gear
# set changes the look, a practice run (Den Lite casts your main bar at a dummy) ends with a number, the boss marks the
# ground and casts, the range goes up. A picture after each. Your class, bars and look are put back after.

DOJO_CLASS = ("mage", "fire", 80)
DOJO_TIERS = ("t10", "t7", "t3", "t1")


def event_like(den_dir, name, since, prefix=""):
    """The first Den Lite event `name` at or after `since` whose detail starts with `prefix`."""
    for e in events(den_dir, since):
        if e.get("ev") == name and str(e.get("detail", "")).startswith(prefix):
            return e
    return None


def dojo_features(rep, ch, den_dir, shoot, outdir, settle=1.0, wait=6.0, run_secs=15):
    den_dir, outdir = Path(den_dir), Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    n = [0]

    def shot(name):
        n[0] += 1
        out = outdir / ("dojo-%02d-%s.png" % (n[0], name))
        ok, _ = shoot(out)
        return out.name if ok else None

    def ev(name, since, prefix="", secs=None):
        return wait_for(lambda: event_like(den_dir, name, since, prefix), wait if secs is None else secs)

    def cmd(line):
        t = now_ms()
        r = ch.send(line)
        time.sleep(settle)
        return t, r

    if ch.send(".den size") is None:
        rep.add("SKIP", "the Dojo's combat test", "Den Lite didn't take the test's commands")
        return False
    before = read_json(den_dir / "den-lite.class.json") or {}
    kept = stash([den_dir / "den-lite.class.json", den_dir / "den-lite.gear.json", den_dir / "den-lite.gearsets.json", den_dir / "dojo-best.json"])
    cmd(".den gearset save dojo-before")
    try:
        # 1. a class and spec: the spellbook and every bar at once
        cls, spec, level = DOJO_CLASS
        t, _ = cmd(".den class %s %s %d" % (cls, spec, level))
        e = ev("class", t, cls)
        bits = str((e or {}).get("detail", "")).split()
        spells = int(bits[2]) if len(bits) > 2 and bits[2].isdigit() else 0
        filled = int(bits[4]) if len(bits) > 4 and bits[4].isdigit() else 0
        points = bits[5] if len(bits) > 5 else "?"
        pic = shot("class")
        rep.ok(e is not None and spells >= 10 and filled >= 20, "dojo: a class and spec fill the spellbook and every bar",
               "%s %s %s: %d spells, %d buttons, %s talent points%s" % (cls, spec, level, spells, filled, points, (", " + pic) if pic else ""),
               "No spells or bars: start Your Den from the launcher (it copies your game's spell tables), then run den-features again." if e else
               "Den Lite didn't take .den class: send logs\\den-features-latest.txt.")
        if points in ("0", "?"):
            rep.add("INFO", "dojo: talents", "no talent points: the launcher copies Talent.dbc and TalentTab.dbc when Your Den starts (an older launcher doesn't)")
        t, _ = cmd(".den spec frost")
        e = ev("spec", t, cls)
        rep.ok(e is not None, "dojo: another spec at once", (e or {}).get("detail", "no change"), "Den Lite didn't take .den spec: send logs\\den-features-latest.txt.")
        shot("spec-frost")
        # 2. a bar loadout kept and put back
        t, _ = cmd(".den bars save dojotest")
        saved = ev("bars", t, "save")
        t, _ = cmd(".den bars use dojotest")
        used = ev("bars", t, "use")
        rep.ok(saved is not None and used is not None, "dojo: a bar loadout saved and put back", "dojotest",
               "Den Lite didn't keep the bar: send logs\\den-features-latest.txt.")
        cmd(".den bars delete dojotest")
        # 3. a gear set: the look changes for everyone
        got = None
        for tier in DOJO_TIERS:
            t, _ = cmd(".den gearset %s" % tier)
            got = ev("gearset", t, tier, secs=wait / 2)
            if got:
                break
        bits = str((got or {}).get("detail", "")).split()
        slots = int(bits[2]) if len(bits) > 2 and bits[2].isdigit() else 0
        pic = shot("gearset")
        rep.ok(got is not None and slots >= 6, "dojo: a gear set changes the look", "%s: %d slots%s" % ((got or {}).get("detail", "none"), slots, (", " + pic) if pic else ""),
               "No gear set: the launcher copies your game's Item.dbc and ItemSet.dbc when Your Den starts; update the launcher, then run den-features again.")
        # 4. a practice run: a number
        cmd(".den dojo dummies 1")
        t, _ = cmd(".den dojo demo %d target" % run_secs)
        time.sleep(min(run_secs / 2.0, 8))
        pic = shot("practice-run")
        e = ev("demo", t, "", secs=run_secs + 10)
        bits = str((e or {}).get("detail", "")).split()
        dps = int(bits[2]) if len(bits) > 2 and bits[2].isdigit() else 0
        rep.ok(e is not None and dps > 0, "dojo: a timed practice run ends with a number",
               "%s per second over %s s%s" % (dps, bits[0] if bits else "?", (", " + pic) if pic else ""),
               "No number: the spells didn't land (send logs\\den-features-latest.txt and the picture).")
        # 5. the boss: its circle (and its cast)
        cmd(".den dojo boss")
        t, _ = cmd(".den dojo demo 12 boss")
        circle = ev("dojo", t, "boss circle", secs=14)
        pic = shot("boss-circle")
        cast = event_like(den_dir, "dojo", t, "boss cast")
        rep.ok(circle is not None, "dojo: the boss marks the ground under you", "%s%s" % ("and casts at you" if cast else "no cast seen", (", " + pic) if pic else ""),
               "The boss didn't fight: send logs\\den-features-latest.txt.")
        # 6. the range
        t, _ = cmd(".den dojo range")
        e = ev("dojo", t, "range")
        pic = shot("range")
        rep.ok(e is not None, "dojo: the range (a dummy 40 yards out, markers)", "%s%s" % ((e or {}).get("detail", "none"), (", " + pic) if pic else ""),
               "The range didn't go up: send logs\\den-features-latest.txt.")
        return True
    finally:
        # as you were: the dummies down, your class, spec and look
        ch.send(".den dojo clear")
        if before.get("class"):
            spec_before = (before.get("specs") or {}).get(before["class"], "")
            ch.send(".den class %s %s %s" % (before["class"], spec_before, before.get("level", 80)))
        ch.send(".den gearset dojo-before")
        ch.send(".den gearset delete dojo-before")
        unstash(kept)
        rep.add("INFO", "your class and look after the Dojo test", "put back as they were")


# ------------------------------------------------------------------------------------------------ the placement aids
# Owner 3 Nov 10:29 - 10:36: a live preview of the prop under the mouse, the three arrows, select by hovering and
# clicking. Den Lite labs/den-lite src/gizmo.rs, the Leyline UI's Gizmo.lua. aids_features drives them through the
# addon's test actions (`.den ui gizmo ...`, DenBar.lua D.TestAction); hover_calibration moves the real cursor.

AIDS_SAVE = "aidstest"
# gizmo.rs Camera::default: the build camera's first guesses (diagonal field of view, pivot over the feet, distance
# behind the pivot, looking down)
CAMERA = {"fov": math.pi / 2, "pivot": 2.0, "dist": 12.0, "pitch": 0.5}
CAMERA_FILE = "den-lite.camera.json"
SAMPLES_FILE = "den-lite.camera-samples"
# where hover_calibration puts the cursor: fractions of the game window (from the left, from the top), all below the
# middle (the ground in front of the character), spread across
CAL_POINTS = [(0.5, 0.62), (0.3, 0.72), (0.7, 0.72), (0.5, 0.85), (0.38, 0.9), (0.62, 0.66)]
SCAN_G = 0x22


def project(cam, player, u, v, aspect, floor):
    """gizmo.rs `project`, the same numbers: where the cursor at (u, v) (-1 .. 1, right and up) points on the ground at
    height `floor`, the build camera `cam` behind the player at (x, y, z, facing). None: no ground within 60 yards past
    the camera's own distance (gizmo.rs REACH + dist: the far view looks from further back)."""
    x, y, z, o = player
    so, co = math.sin(o), math.cos(o)
    fwd, right = (co, so, 0.0), (so, -co, 0.0)
    sp, cp = math.sin(cam["pitch"]), math.cos(cam["pitch"])
    pivot = (x, y, z + cam["pivot"])
    eye = (pivot[0] - cam["dist"] * cp * fwd[0], pivot[1] - cam["dist"] * cp * fwd[1], pivot[2] + cam["dist"] * sp)
    look = (cp * fwd[0], cp * fwd[1], -sp)
    up = (right[1] * look[2] - right[2] * look[1], right[2] * look[0] - right[0] * look[2], right[0] * look[1] - right[1] * look[0])
    a = min(max(aspect, 0.2), 5.0)
    tv = math.tan(cam["fov"] / 2) / math.sqrt(1 + a * a)
    u, v = min(max(u, -1.0), 1.0), min(max(v, -1.0), 1.0)
    d = [look[i] + right[i] * u * tv * a + up[i] * v * tv for i in range(3)]
    if d[2] >= -1e-4:
        return None
    t = (floor - eye[2]) / d[2]
    if t <= 0:
        return None
    px, py = eye[0] + d[0] * t, eye[1] + d[1] * t
    return (px, py) if math.hypot(px - x, py - y) <= 60.0 + cam["dist"] else None


def camera_error(cam, samples):
    """The root mean square, in yards, of where the camera says each sample's cursor lands against where the exact
    click landed (a cursor whose ray misses counts 30 yards)."""
    if not samples:
        return float("inf")
    tot = 0.0
    for sm in samples:
        # the zoom the sample was taken at (gizmo.rs Sample.dist): Den Lite's ray uses the addon's zoom over the
        # camera's own distance (Aids::view), so a fit must too, or samples at two zooms can't agree (review 4 Nov)
        # (a far-view sample's distance is the zoom plus the view's pull-back: up to FAR_MAX past the game's 50)
        c = cam
        if isinstance(sm.get("dist"), (int, float)) and 0.0 < sm["dist"] <= SAMPLE_DIST_MAX:
            c = dict(cam, dist=float(sm["dist"]))
        hit = project(c, sm["player"], sm["u"], sm["v"], sm["aspect"], sm["dest"][2])
        tot += 900.0 if hit is None else (hit[0] - sm["dest"][0]) ** 2 + (hit[1] - sm["dest"][1]) ** 2
    return math.sqrt(tot / len(samples))


SAMPLE_DIST_MAX = 1560.0   # gizmo.rs FAR_MAX (1500) + the game's own zoom limit, with room

FIT_KEYS = ("fov", "pivot", "dist", "pitch")
FIT_BOUNDS = {"fov": (0.3, 2.8), "pivot": (0.0, 5.0), "dist": (1.0, 40.0), "pitch": (-0.3, 1.5)}


def fit_camera(samples, start=None, rounds=3000):
    """The build camera that best fits the samples (Nelder-Mead on the four numbers, kept within FIT_BOUNDS):
    (camera, its error in yards)."""
    start = dict(start or CAMERA)

    def cam_of(p):
        return {k: min(max(p[i], FIT_BOUNDS[k][0]), FIT_BOUNDS[k][1]) for i, k in enumerate(FIT_KEYS)}

    def f(p):
        return camera_error(cam_of(p), samples)
    pts = [[start[k] for k in FIT_KEYS]]
    for i, k in enumerate(FIT_KEYS):
        p = list(pts[0])
        p[i] += 0.3 if k in ("fov", "pitch") else 1.5
        pts.append(p)
    vals = [f(p) for p in pts]
    for _ in range(rounds):
        order = sorted(range(len(pts)), key=lambda i: vals[i])
        pts, vals = [pts[i] for i in order], [vals[i] for i in order]
        if vals[-1] - vals[0] < 1e-9:
            break
        c = [sum(p[j] for p in pts[:-1]) / (len(pts) - 1) for j in range(len(FIT_KEYS))]
        r = [c[j] + (c[j] - pts[-1][j]) for j in range(len(FIT_KEYS))]
        fr = f(r)
        if fr < vals[0]:
            e = [c[j] + 2 * (c[j] - pts[-1][j]) for j in range(len(FIT_KEYS))]
            fe = f(e)
            pts[-1], vals[-1] = (e, fe) if fe < fr else (r, fr)
        elif fr < vals[-2]:
            pts[-1], vals[-1] = r, fr
        else:
            k = [c[j] + 0.5 * (pts[-1][j] - c[j]) for j in range(len(FIT_KEYS))]
            fk = f(k)
            if fk < vals[-1]:
                pts[-1], vals[-1] = k, fk
            else:
                for i in range(1, len(pts)):
                    pts[i] = [pts[0][j] + 0.5 * (pts[i][j] - pts[0][j]) for j in range(len(FIT_KEYS))]
                    vals[i] = f(pts[i])
    best = min(range(len(pts)), key=lambda i: vals[i])
    return cam_of(pts[best]), vals[best]


def read_samples(den_dir, skip=0):
    """The exact clicks' calibration samples Den Lite wrote (gizmo.rs Sample), from line `skip` on."""
    out = []
    try:
        lines = (Path(den_dir) / SAMPLES_FILE).read_text(encoding="utf-8").splitlines()
    except OSError:
        return out
    for line in lines[skip:]:
        try:
            v = json.loads(line)
        except ValueError:
            continue
        if all(k in v for k in ("u", "v", "aspect", "player", "dest")):
            out.append(v)
    return out


def aids_of(answer):
    """The host's placement aids from a test command's answer (test-out "aids": gizmo.rs `view`)."""
    a = answer.get("aids") if isinstance(answer, dict) else None
    return a if isinstance(a, dict) else {}


def mouse_steps(rep, ui, aids, shot, with_pic, den_dir, wait):
    """aids_features' mouse checks: a click on the sky clears the selection; the cursor on the prop glows it; a click
    selects it; pressed, moved and let go, it is dropped where the cursor took it (yards moved)."""
    a = aids()
    sel, uv, at0 = a.get("sel"), a.get("sel_uv"), a.get("sel_at")
    if sel is None or not uv or not at0 or not all(-0.95 < c < 0.95 for c in uv):
        rep.add("SKIP", "the mouse: hover, click and drag", "the selection (%s) isn't on the screen (%s)" % (sel, uv))
        return False
    u, v = uv
    note = aspect_note(den_dir, a.get("cursor"))
    if note:
        rep.add("INFO", "mouse: the camera's window shape", note)
    ui("gizmo click 0 0.95")
    cleared = aids().get("sel") is None
    ui("gizmo hover %.4f %.4f" % (u, v))
    a = aids()
    pic = shot("mouse-hover")
    rep.ok(a.get("hovered") == sel, "mouse: the prop under the cursor glows",
           with_pic("prop %s under the cursor at (%.2f, %.2f)" % (a.get("hovered"), u, v), pic),
           "Den Lite didn't find prop %s under the cursor (it found %s): send logs\\den-features-latest.txt and the picture." % (sel, a.get("hovered")))
    ui("gizmo click %.4f %.4f" % (u, v))
    a = aids()
    pic = shot("mouse-click")
    rep.ok(cleared and a.get("sel") == sel and a.get("glow") == sel, "mouse: a click selects it (a click on the sky clears)",
           with_pic("the sky cleared it: %s; then prop %s selected, glowing %s" % ("yes" if cleared else "no", a.get("sel"), a.get("glow")), pic),
           "Cleared %s, selected %s, glow %s: send logs\\den-features-latest.txt and the picture." % (cleared, a.get("sel"), a.get("glow")))
    t = now_ms()
    ui("gizmo press %.4f %.4f" % (u, v))
    ui("gizmo moveto %.4f %.4f" % (u + 0.2, v - 0.05))
    held = aids().get("grip") is True
    pic = shot("mouse-dragging")
    ui("gizmo release")
    ev = wait_for(lambda: event_after(den_dir, "den_change", t), wait)
    a = aids()
    pic2 = shot("mouse-dropped")
    at1 = a.get("sel_at") or at0
    yd = math.hypot(at1[0] - at0[0], at1[1] - at0[1])
    rep.ok(held and ev is not None and yd > 0.5, "mouse: press and drag moves it",
           "moved %.2f yd%s%s" % (yd, (", " + pic) if pic else "", (", " + pic2) if pic2 else ""),
           "Held %s, dropped %s, moved %.2f yd: send logs\\den-features-latest.txt and the pictures." % (held, ev is not None, yd))
    # click to carry (owner 4 Nov: "select with left click, immediately start moving mouse and the object follows the
    # mouse ... across the terrain, clicking places it"): a click picks it up, the cursor moves it with no button held,
    # the next click puts it down (one undo); it stays selected with its arrows
    a = aids()
    if "draw_ms" not in a:
        rep.add("INFO", "mouse: click to carry", "this Den Lite is from before carrying (4 Nov): run den-lite, then den-move again")
        return True
    sel, uv, at0 = a.get("sel"), a.get("sel_uv"), a.get("sel_at")
    if sel is None or not uv or not at0:
        rep.add("SKIP", "mouse: click to carry", "the selection isn't on the screen (%s)" % (uv,))
        return True
    u, v = uv
    t = now_ms()
    ui("gizmo carry %.4f %.4f" % (u, v))
    a = aids()
    carried = str(a.get("armed", "")).startswith("move,") and a.get("grip") is True
    # picked up where it stands (review 4 Nov: it jumped about 2.5 yd to the ground behind the spot clicked)
    jump = carry_jump(at0, a.get("at"))
    if jump is not None:
        rep.ok(jump <= 0.6, "mouse: picked up without a jump", "%.2f yd" % jump,
               "It jumped %.2f yd when picked up: send logs\\den-move-latest.txt." % jump)
    ui("gizmo moveto %.4f %.4f" % (u - 0.15, v - 0.04))
    g1 = aids().get("ghost")
    ui("gizmo moveto %.4f %.4f" % (u - 0.2, v - 0.06))
    a = aids()
    g2 = a.get("ghost")
    pic = shot("mouse-carrying")
    ui("gizmo commit")
    ev = wait_for(lambda: event_after(den_dir, "den_change", t), wait)
    a = aids()
    pic2 = shot("mouse-carried-down")
    at1 = a.get("sel_at") or at0
    yd = math.hypot(at1[0] - at0[0], at1[1] - at0[1])
    still = a.get("sel") == sel and int(a.get("gizmo") or 0) >= 3
    rep.ok(carried and g1 and g2 and g1 != g2 and ev is not None and yd > 0.5 and still, "mouse: a click picks it up, it follows, a click puts it down",
           "moved %.2f yd, still selected with its arrows%s%s" % (yd, (", " + pic) if pic else "", (", " + pic2) if pic2 else ""),
           "Carried %s, followed %s, put down %s, moved %.2f yd, selected with arrows %s: send logs\\den-move-latest.txt and the pictures."
           % (carried, bool(g1 and g2 and g1 != g2), ev is not None, yd, still))
    ui("gizmo debug")
    dbg = [x for x in ui_lines(den_dir, now_ms() - 5000) if "draw_ms=" in x]
    rep.add("INFO", "debug: what the editor sees", (dbg[-1][:300] if dbg else "the debug line comes in the game (/wudebug)"))
    ui("gizmo debug off")
    return True


NOT_SELECTABLE = 0x10   # gizmo.rs: GO_FLAG_NOT_SELECTABLE, what Den Lite tells the game for props in build mode


def owner_flow_steps(rep, ui, aids, size, shot, with_pic, den_dir, wait):
    """The owner's live test of 5 Nov, as he does it, through the Leyline UI's own code paths (Gizmo.lua's frame tick
    and its press and let-go, not the test's own hovers): props are sent so the game never takes a click on them
    ("You are too far away."); a gallery card picked, its copy glides under the cursor as it moves over the world and a
    click puts it down, selected; with nothing in hand a click on a placed prop picks it up (it glides), the next click
    puts it down. Returns False when this Den Lite is from before."""
    first = size() or {}
    if "prop_flags" not in first:
        rep.add("INFO", "5 Nov: the editor's click fixes", "this Den Lite is from before them: run den-lite, then den-move again")
        return False
    flags = first.get("prop_flags")
    rep.ok(isinstance(flags, int) and flags & NOT_SELECTABLE, "build mode: props never take the game's click",
           "the game is told flags 0x%x (not selectable): a click on a prop is the editor's" % (flags or 0),
           "Den Lite told the game flags %s for a prop in build mode (want 0x10): a click would use the prop. Run den-lite." % flags)
    # 1. a card from the gallery: its copy glides with the cursor (each move a new place), a click puts it down
    ui("gizmo cancel")
    p0 = int(first.get("props") or 0)
    before = aids()
    t = now_ms()
    ui("pick 2413")
    if not str(aids().get("armed", "")).startswith("place,2413"):
        # what the UI said (5 Nov: the gallery's cards hadn't come, the pick did nothing)
        said = [x for x in ui_lines(den_dir, t) if "pick" in x or "missing" in x]
        rep.add("INFO", "card: the pick", "; ".join(said) or "the UI said nothing about the pick")
    spots = []
    for u, v in ((0.25, -0.35), (0.05, -0.3), (-0.15, -0.4)):
        ui("gizmo glide %.2f %.2f" % (u, v))
        spots.append(aids().get("at"))
    glide_check(rep, before, aids(), "card")
    armed = str(aids().get("armed", ""))
    pic = shot("card-glides")
    moved = all(spots) and len({tuple(round(c, 2) for c in sp[:2]) for sp in spots}) == 3
    rep.ok(armed.startswith("place,2413") and moved, "card: the copy glides under the cursor",
           with_pic("in hand %s, at %s" % (armed, " -> ".join(str([round(c, 1) for c in sp[:2]]) if sp else "-" for sp in spots)), pic),
           "In hand %s; the copy's places %s: the cursor's moves didn't reach Den Lite. Send logs\\den-move-latest.txt and the picture." % (armed, spots))
    t = now_ms()
    ui("gizmo mouse -0.15 -0.4")
    ev = wait_for(lambda: event_after(den_dir, "den_change", t), wait)
    ans = size() or {}
    a = aids_of(ans)
    pic = shot("card-placed")
    placed = ev is not None and int(ans.get("props") or 0) == p0 + 1
    rep.ok(placed and a.get("armed") == "off" and a.get("sel") is not None and a.get("glow") == a.get("sel"), "card: a click puts it down, selected",
           with_pic("prop %s, glowing %s" % (a.get("sel"), a.get("glow")), pic),
           "Placed %s, in hand %s, selected %s, glow %s: send logs\\den-move-latest.txt and the picture." % (placed, a.get("armed"), a.get("sel"), a.get("glow")))
    if placed and (a.get("sel") is None or a.get("glow") != a.get("sel")):
        why_unselected(rep, ui, den_dir, t, a, "card")
    # 2. nothing in hand (the Select tool): a click on the placed prop picks it up, it glides, the next click puts it down
    # (5 Nov: when step 1 left something in hand, this click put it down instead: each step starts empty-handed)
    if aids().get("armed") not in (None, "off"):
        ui("gizmo cancel")
    a = aids()
    sel, uv, at0 = a.get("sel"), a.get("sel_uv"), a.get("sel_at")
    if sel is None or not uv or not at0 or not all(-0.95 < c < 0.95 for c in uv):
        rep.add("SKIP", "select: a click picks it up", "the selection (%s) isn't on the screen (%s)" % (sel, uv))
        return True
    u, v = uv
    t = now_ms()
    before = aids()
    ui("gizmo mouse %.4f %.4f" % (u, v))
    a = aids()
    carried = str(a.get("armed", "")).startswith("move,%s," % sel) and a.get("grip") is True
    ui("gizmo glide %.4f %.4f" % (u - 0.12, v - 0.03))
    ui("gizmo glide %.4f %.4f" % (u - 0.06, v - 0.05))
    ui("gizmo glide %.4f %.4f" % (u - 0.12, v - 0.03))
    g1 = aids().get("at")
    glide_check(rep, before, aids(), "select")
    pic = shot("select-carried")
    ui("gizmo mouse %.4f %.4f" % (u - 0.12, v - 0.03))
    ev = wait_for(lambda: event_after(den_dir, "den_change", t), wait)
    a = aids()
    at1 = a.get("sel_at") or at0
    yd = math.hypot(at1[0] - at0[0], at1[1] - at0[1])
    rep.ok(carried and g1 and ev is not None and yd > 0.3 and a.get("armed") == "off", "select: a click on a prop picks it up, the next puts it down",
           with_pic("moved %.2f yd" % yd, pic),
           "Picked up %s, glided %s, put down %s, moved %.2f yd: send logs\\den-move-latest.txt and the picture." % (carried, bool(g1), ev is not None, yd))
    return True


# what Den Lite writes when the editor's commands come (world.rs): the verbs, the selection's changes, the modes
SEL_EVENTS = ("sel", "build_mode", "test_cmd", "aid_use", "aid_click", "panic")


def sel_story(den_dir, since, limit=14):
    """Den Lite's events since `since` that bear on the selection (the aids' verbs, the Builder's commands, `sel`
    changes and their cause, the modes), oldest first, as one short line each; hovers are never written."""
    out = []
    for e in events(Path(den_dir), since):
        ev, det = e.get("ev"), str(e.get("detail", ""))
        if ev in SEL_EVENTS or (ev == "addon" and det.startswith("wubuild")):
            if ev == "test_cmd" and det.startswith(".den size"):
                continue
            out.append("%s %s" % (ev, det[:80]))
    return out[-limit:]


def why_unselected(rep, ui, den_dir, since, aids_now, label, wait=4.0):
    """A prop placed and selected that isn't selected a moment later (5 Nov den-move, step 3: "prop None"): what
    cleared it, from both sides. Den Lite: `sel_by` (the verb that last changed the selection: world.rs
    `sel_changed`) and its events since `since`; the Leyline UI: `gizmo report` (its `seltrace`: a click on the
    world, Esc, Play, each with the time)."""
    rep.add("INFO", "%s: what changed the selection (Den Lite)" % label, str((aids_now or {}).get("sel_by") or "nothing said (a Den Lite from before 5 Nov evening)"))
    story = sel_story(den_dir, since)
    rep.add("INFO", "%s: Den Lite's events since" % label, "; ".join(story) or "none")
    t = now_ms()
    ui("gizmo report")
    lines = wait_for(lambda: [x for x in ui_lines(den_dir, t) if x.startswith("seltrace") or x.startswith("gizmo ")], wait, 0.2) or []
    rep.add("INFO", "%s: what the UI says" % label, "; ".join(x[:240] for x in lines) or "no report from the UI")


def layout_stamps(den_dir, since):
    """The stamps Den Lite kept from the layout (its `layout_stamps` event: `name:props:new|kept ...`), [(name, n, new)]."""
    out = []
    for e in events(Path(den_dir), since):
        if e.get("ev") == "layout_stamps":
            for part in str(e.get("detail", "")).split():
                bits = part.split(":")
                if len(bits) == 3 and bits[1].isdigit():
                    out.append((bits[0], int(bits[1]), bits[2] == "new"))
    return out


def empty_den(rep, ch, den_dir, owners, wait=4.0):
    """The Den emptied for the editor's test (owner 5 Nov), after its layout is kept as stamps (Den Lite
    layoutstamps.rs, `.den stamps from-layout`: once, a stamp already there isn't written again); then one of those
    stamps put down in the empty Den (its props come) and one undo takes it away. Returns the props cleared."""
    if owners <= 0:
        rep.add("INFO", "the test's empty Den", "your Den had no props")
        return 0
    t = now_ms()
    ch.send(".den stamps from-layout")
    made = wait_for(lambda: layout_stamps(den_dir, t), wait, 0.2) or []
    if made:
        rep.ok(True, "your layout kept as stamps (Stamps > Mine)",
               ", ".join("%s (%d%s)" % (n, k, ", new" if new else "") for n, k, new in made), "")
    else:
        rep.add("INFO", "your layout kept as stamps", "this Den Lite is from before `.den stamps from-layout`: your Den is saved as %s-before and put back after" % AIDS_SAVE)
    r = ch.send(".den clear") or {}
    left = r.get("props")
    if left is None or int(left) != 0:
        rep.add("INFO", "the test's empty Den", "this Den Lite can't clear the Den (%s props there): the test runs among them" % left)
        return 0
    rep.ok(True, "the test's empty Den", "%d props cleared (kept in %s-before, put back after)" % (owners, AIDS_SAVE), "")
    # one of the new stamps put down here, and one undo takes it back (the smallest dressed group: it fits)
    groups = sorted([m for m in made if not m[0].startswith("my-den-")], key=lambda m: m[1]) or sorted(made, key=lambda m: m[1])
    if groups:
        name, n, _ = groups[0]
        got = (ch.send(".den stamp %s" % name) or {}).get("props")
        undone = (ch.send(".build undo") or {}).get("props")
        rep.ok(got is not None and int(got) > 0 and undone is not None and int(undone) == 0, "a kept stamp goes down, one undo takes it back",
               "%s: %s props down, then %s" % (name, got, undone),
               "The stamp %s put %s props down and undo left %s (want some, then 0): send logs\\den-move-latest.txt." % (name, got, undone))
    return owners


def glide_check(rep, before, after, label):
    """Owner 5 Nov: "dynamically move instead of disappearing, reappearing". The copy in hand (an M2 prop) is a creature
    made once and glided (Den Lite glide.rs): between `before` and `after` (Den Lite's aid views) one copy was made
    and it glided at least twice, with no gameobject copy made anew. A Den Lite from before, or a game without the
    Den patch's creature rows (glide "none"), says so instead."""
    name = "%s: the copy glides as one creature (no destroy / create)" % label
    if "copies_made" not in after:
        rep.add("INFO", name, "this Den Lite is from before the glide: run den-lite, then den-move again")
        return None
    if after.get("glide") != "on":
        rep.add("INFO", name, "the copy is a gameobject made anew at each spot (glide %s): %s" % (after.get("glide"),
                "start Your Den from the updated launcher (its patch gives the props' models creature rows), then den-move again"
                if after.get("glide") == "none" else "turned off with gizmo glide off"))
        return None
    made = int(after.get("copies_made") or 0) - int(before.get("copies_made") or 0)
    glided = int(after.get("copies_glided") or 0) - int(before.get("copies_glided") or 0)
    ok = after.get("copy") == "creature" and made == 1 and glided >= 2
    rep.ok(ok, name, "made once, glided %d times" % glided,
           "The copy was %s, made %d times, glided %d times (want a creature made once, gliding): send logs\\den-move-latest.txt."
           % (after.get("copy"), made, glided))
    return ok


def calibrated_aspect(samples):
    """The window's width over its height the calibration samples were taken at (their middle value), or None."""
    a = sorted(float(s["aspect"]) for s in samples if isinstance(s.get("aspect"), (int, float)) and 0.2 < s["aspect"] < 5.0)
    return round(a[len(a) // 2], 3) if a else None


def aspect_note(den_dir, cursor):
    """A line when the build camera was measured at another window shape than the game's now (None: same, or not
    known). The ray's field of view is fitted as a diagonal one; if the game keeps it another way, a new shape shifts
    every hover a little."""
    try:
        cal = float(json.loads((Path(den_dir) / CAMERA_FILE).read_text(encoding="utf-8")).get("aspect"))
        now = float(cursor[2])
    except (OSError, ValueError, TypeError, AttributeError, IndexError):
        return None
    if abs(cal - now) <= 0.05:
        return None
    return "the build camera was measured at a window %.2f wide to 1 high; the game is %.2f now: run the hover calibration again if hovers are off" % (cal, now)


def carry_jump(at0, at):
    """How far the copy in hand is from where the prop stood, the moment a click picked it up (yards; None: unknown)."""
    try:
        return math.hypot(float(at[0]) - float(at0[0]), float(at[1]) - float(at0[1]))
    except (TypeError, ValueError, IndexError):
        return None


def aids_features(rep, ch, den_dir, shoot, outdir, settle=1.0, wait=6.0, only_move=False):
    """The editor's flow as the owner's video shows it (3 Nov: "it should function like this"), through the Leyline UI's
    own test actions, a picture after each step: build mode (props shown as things the game highlights); a gallery
    card picked (in hand, nothing in front of you); the real prop following the cursor; one click places it and it is
    selected at once (its glow, its handles and the halo); a handle dragged along X (moved N yd) and dropped; the wheel
    along a picked axis; Swap with another card; the menu's Duplicate; the mouse (the prop under the cursor glows, a
    click selects it, a press and drag moves it: mouse_steps); snap out and back; the wheel past the game's zoom limit
    and back; play mode. Your Den is saved first and put back after."""
    den_dir, outdir = Path(den_dir), Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    n = [0]

    def shot(name):
        n[0] += 1
        out = outdir / ("aids-%02d-%s.png" % (n[0], name))
        ok, _ = shoot(out)
        return out.name if ok else None

    def with_pic(text, pic):
        return text + ((", " + pic) if pic else "")

    def ui(action):
        r = ch.send(".den ui %s" % action)
        time.sleep(settle)
        return r

    def aids():
        return aids_of(ch.send(".den size"))

    def props():
        return (ch.send(".den size") or {}).get("props")

    first = ch.send(".den size")
    if first is None:
        rep.add("SKIP", "the placement aids", "Den Lite didn't take the test's commands")
        return False
    if "aids" not in first:
        rep.add("SKIP", "the placement aids", "this Den Lite is from before them: update Den Lite (the launcher puts the newest in)")
        return False
    newer = "cam" in (aids_of(first) or {})
    before = den_dir / ("%s-before.den.json" % AIDS_SAVE)
    kept = stash([before, den_dir / "backup.den.json"])
    ch.send(".den save %s-before" % AIDS_SAVE)
    owners = int(first.get("props") or 0)
    try:
        # an empty Den for the test (owner 5 Nov: "something blocking the view in your tests you can remove everything
        # from den for these tests"): its layout kept as stamps first, then every prop away (one undo); the save above
        # puts it back at the end
        empty_den(rep, ch, den_dir, owners)
        # build mode: Den Lite says the look (an event every time it is asked: 3 Nov the editor was already in build
        # mode, so none came); since 3 Nov its props go out under their build twins (gizmo.rs `twin`), which the
        # game asks for once each, as goobers (a glow under the mouse, right-click uses them)
        q0 = int((ch.send(".den size") or {}).get("twin_queries") or 0)
        t = now_ms()
        ui("gizmo build on")
        ev = wait_for(lambda: event_after(den_dir, "build_mode", t), wait)
        pic = shot("build-mode")
        ans = ch.send(".den size") or {}
        look = ans.get("look_mode")
        asked = int(ans.get("twin_queries") or 0) - q0
        ok = ev is not None and ev.get("detail") == "build" and look in (None, "build")
        rep.ok(ok, "build mode: props light up under the mouse",
               ("your props are shown as things the game highlights and lets you right-click" +
                ("" if look is None else (", the game asked for %d of their goober looks" % asked) if asked > 0 else ", the game had their goober looks kept from before")
                + ((", " + pic) if pic else "")) if ok else
               ("Den Lite's look: %s; the event: %s" % (look or "?", (ev or {}).get("detail", "none"))),
               "The Leyline UI didn't start build mode: update the Leyline addons and Den Lite, then run den-features again.")
        if not ok:
            # why the UI left build mode (Gizmo.lua: it follows G.Wanted() every half second)
            t2 = now_ms()
            ui("gizmo report")
            why = wait_for(lambda: [x for x in ui_lines(den_dir, t2) if x.startswith("why ")], wait, 0.2) or []
            rep.add("INFO", "build mode: what the UI says", "; ".join(why) or "no report from the UI")
        if look is None:
            rep.add("INFO", "build mode: the props' goober looks", "this Den Lite is from before them (3 Nov): update Den Lite for the glow under the mouse")
        if not newer:
            rep.add("INFO", "the editor's new flow", "this Den Lite is from before one-click placing, the handles' drag and snap out (3 Nov): update Den Lite")
        # 1. a card picked: in hand (the panel says so), nothing in front of you
        ui("gizmo arm 2413")
        a = aids()
        pic = shot("pick")
        # since 5 Nov (owner: "it does not appear on mouse") the copy shows at once when the cursor is over the world
        # (or 5 yd in front of the view); with the cursor off the world nothing shows yet. Either is right: the pick
        # is what this checks (the first den-move of 5 Nov failed it for showing the copy at once)
        rep.ok(str(a.get("armed", "")).startswith("place,2413"), "pick: the card in hand",
               with_pic("armed %s, %s" % (a.get("armed"), "its copy shown at once at %s" % ([round(c, 1) for c in a["ghost"][:3]],)
                        if a.get("ghost") else "nothing shown until the cursor is over the world"), pic),
               "Den Lite didn't take the pick: send logs\\den-features-latest.txt.")
        # 2. the real prop follows the cursor on the ground
        ui("gizmo hover 0.25 -0.35")
        a = aids()
        cur = a.get("cursor") or []
        g1 = a.get("ghost")
        ui("gizmo hover 0.1 -0.3")
        a = aids()
        pic = shot("follow")
        rep.ok(len(cur) >= 2 and abs(cur[0] - 0.25) < 0.01 and abs(cur[1] + 0.35) < 0.01 and g1 and a.get("ghost") and a.get("ghost") != g1,
               "follow: the real prop goes where the cursor points", with_pic("at %s, then %s" % (g1, a.get("ghost")), pic),
               "The prop didn't follow the cursor: send logs\\den-features-latest.txt.")
        if not (g1 and a.get("ghost")):
            # what Den Lite saw (4 Nov: "at None, then None"): the cursor it got, what is armed, where the copy should
            # go, the camera and its zoom, the far view
            rep.add("INFO", "follow: what Den Lite saw", "cursor %s, armed %s, at %s, cam %s, zoom %s, far %s, build %s, draw %s ms" % (
                a.get("cursor"), a.get("armed"), a.get("at"), a.get("cam"), a.get("zoom"), a.get("far_at"), a.get("build"), a.get("draw_ms")))
            camf = den_dir / "den-lite.camera.json"
            try:
                cam_text = camf.read_text(encoding="utf-8")[:300] if camf.is_file() else "none (Den Lite's first guesses)"
            except OSError as e:
                cam_text = "unreadable (%s)" % e
            rep.add("INFO", "follow: the calibrated camera", "%s: %s" % (camf.name, cam_text))
            rep.add("INFO", "follow: Den Lite's whole state", json.dumps(a, sort_keys=True)[:900])
        # 3. one click: placed and selected at once (glow, handles, halo), placing over
        p0 = props() or 0
        t = now_ms()
        ui("gizmo commit")
        ev = wait_for(lambda: event_after(den_dir, "den_change", t), wait)
        a = aids()
        pic = shot("placed-selected")
        placed = ev is not None and (props() or 0) == p0 + 1
        if newer:
            ok = placed and a.get("armed") == "off" and a.get("sel") is not None and a.get("glow") == a.get("sel") and int(a.get("gizmo") or 0) >= 3
            rep.ok(ok, "click: placed, and selected at once (it glows, its handles)",
                   with_pic("prop %s, glow on %s, %s handles and the halo, placing over (%s)" % (a.get("sel"), a.get("glow"), max(0, int(a.get("gizmo") or 0) - 1), a.get("armed")), pic),
                   "Placed %s; armed %s, glow %s, gizmo %s: send logs\\den-features-latest.txt and the picture." % (placed, a.get("armed"), a.get("glow"), a.get("gizmo")))
            if placed and not ok:
                why_unselected(rep, ui, den_dir, t, a, "click")
        else:
            rep.ok(placed, "click: placed there", (ev or {}).get("detail", "nothing placed"), "Den Lite didn't place the prop: send logs\\den-features-latest.txt.")
            ui("gizmo cancel")
        # den-move (owner 3 Nov: "limit the den test to one test: moving and selecting objects") skips 4, 5 and the menu
        if not only_move:
            # 4. a handle dragged along X: the prop follows the cursor along it; a click drops it
            if newer:
                ui("gizmo grab x")
                a = aids()
                grabbed = str(a.get("armed", "")).startswith("axis,")
                ui("gizmo hover 0.4 -0.3")
                a = aids()
                pic = shot("dragging")
                moved = float(a.get("drag") or 0.0)
                t = now_ms()
                ui("gizmo commit")
                ev = wait_for(lambda: event_after(den_dir, "den_change", t), wait)
                pic2 = shot("drag-dropped")
                rep.ok(grabbed and abs(moved) > 0.05 and ev is not None, "drag: a handle moves it along X, a click drops it",
                       "moved %+.2f yd along X%s%s" % (moved, (", " + pic) if pic else "", (", " + pic2) if pic2 else ""),
                       "Grabbed %s, moved %.2f, dropped %s: send logs\\den-features-latest.txt and the pictures." % (grabbed, moved, ev is not None))
            else:
                ui("select")
            # the wheel along a picked axis (a step at a time)
            ui("gizmo axis z")
            t = now_ms()
            ui("gizmo wheel 1")
            ev = wait_for(lambda: event_after(den_dir, "den_change", t), wait)
            pic = shot("wheel-step")
            rep.ok(ev is not None and aids().get("axis") == "z", "gizmo: the wheel moves it along the picked axis",
                   with_pic("axis z, %s" % (ev or {}).get("detail", "no change"), pic),
                   "The wheel didn't move the prop: send logs\\den-features-latest.txt.")
            ui("gizmo axis off")
            # 5. Swap with: another card picked, Ins
            if newer:
                ui("pick 1670")
                t = now_ms()
                ui("key insert")
                ev = wait_for(lambda: event_after(den_dir, "den_change", t), wait)
                pic = shot("swap")
                rep.ok(ev is not None, "swap: the selection becomes the gallery's pick", with_pic((ev or {}).get("detail", "no change"), pic),
                       "Swap did nothing: send logs\\den-features-latest.txt.")
                ui("gizmo cancel")
            # the right-click menu's Duplicate (the menu opens at the cursor on a right-click in the game)
            p0 = props() or 0
            ui("gizmo menu 2")
            got = wait_for(lambda: (props() or 0) == p0 + 1, wait, 0.5)
            rep.ok(bool(got), "menu: Duplicate", "%s -> %s props" % (p0, props()), "The menu's Duplicate did nothing: send logs\\den-features-latest.txt.")
        # the mouse, picked by Den Lite (gizmo.rs `click`, `drag`; owner 3 Nov: "clicking objects, having them
        # highlighted, moving them around easily"): the prop under the cursor glows, a click selects it, a press and
        # drag moves it. The selection's own place on the screen comes from Den Lite (`sel_uv`), so the test points
        # the cursor where it truly is.
        a = aids()
        if "sel_uv" not in a:
            rep.add("INFO", "the mouse: hover, click and drag", "this Den Lite is from before clicking and dragging props (3 Nov): update Den Lite")
        else:
            mouse_steps(rep, ui, aids, shot, with_pic, den_dir, wait)
        # the owner's own way, 5 Nov (his Gizmo.lua paths: the frame tick and the press and let-go)
        owner_flow_steps(rep, ui, aids, lambda: ch.send(".den size"), shot, with_pic, den_dir, wait)
        # the free flight (owner 4 Nov: "fly like a typical 3d model engine"): W held for a second glides the build
        # view on (about 16 yards a second, no stops), the let-go eases it out
        ui("gizmo cam build")
        a0 = (aids() or {}).get("far_at")
        if a0:
            ui("gizmo fly W down")
            time.sleep(1.0)
            mid = (aids() or {}).get("far_at")
            ui("gizmo fly W up")
            time.sleep(0.4)
            a1 = (aids() or {}).get("far_at")
            pic = shot("fly")
            dist = lambda p, q: ((p[0] - q[0]) ** 2 + (p[1] - q[1]) ** 2) ** 0.5 if p and q else 0.0
            moved, still = dist(a0, a1), dist(mid, a1) > 0.5
            rep.ok(8.0 <= moved <= 40.0 and still, "free flight: W held glides the view on, the let-go eases out",
                   with_pic("moved %.1f yd in about a second (%s -> %s)" % (moved, a0, a1), pic),
                   "The view didn't fly (moved %.1f yd): send logs\\den-move-latest.txt." % moved)
        else:
            rep.add("SKIP", "free flight", "the build view wasn't on (no camera object)")
        if newer:
            # 6. snap out: the view up and back over you (Den Lite's camera object), and back
            ui("gizmo snap")
            a = aids()
            pic = shot("snap-out")
            out = str(a.get("cam", "")).startswith("far,") and a.get("far_at")
            rep.ok(bool(out), "snap out: the view from up high", with_pic("%s, the camera object at %s" % (a.get("cam"), a.get("far_at")), pic),
                   "The view didn't go out: send logs\\den-features-latest.txt and the picture.")
            ui("gizmo snap")
            a = aids()
            pic = shot("snap-back")
            rep.ok(a.get("cam") == "near", "snap out: and back to you", with_pic(str(a.get("cam")), pic),
                   "The view stayed out: press the snap-out key (or View on the rail) and send logs\\den-features-latest.txt.")
            # 7. the wheel past the game's own zoom limit: Den Lite's far view; and back in
            ui("gizmo zoom 40")
            a = aids()
            pic = shot("zoom-far")
            far = str(a.get("cam", "")).split(",")
            back = float(far[1]) if len(far) > 1 and far[0] == "far" else 0.0
            rep.ok(back > 0, "zoom: out past the game's limit", with_pic("%.0f yd further than the game's own camera goes" % back, pic),
                   "The zoom stopped at the game's limit: send logs\\den-features-latest.txt.")
            ui("gizmo zoom -60")
            a = aids()
            pic = shot("zoom-back")
            rep.ok(a.get("cam") == "near", "zoom: and back to you", with_pic(str(a.get("cam")), pic),
                   "The far view stayed: send logs\\den-features-latest.txt.")
        # play mode: plain again
        t = now_ms()
        ui("gizmo build off")
        ev = wait_for(lambda: event_after(den_dir, "build_mode", t), wait)
        rep.ok(ev is not None and ev.get("detail") == "play", "play mode: props plain again", (ev or {}).get("detail", "no change"),
               "Play mode didn't come back: send logs\\den-features-latest.txt.")
        pf = (ch.send(".den size") or {}).get("prop_flags", "-")
        if pf != "-":
            rep.ok(pf == 0, "play mode: props usable again (a chair is sat on)", "flags %s" % pf,
                   "Den Lite still tells the game a prop isn't selectable in play mode (flags %s): send logs\\den-move-latest.txt." % pf)
        return True
    finally:
        if before.is_file():
            back = ch.send(".den load %s-before replace" % AIDS_SAVE) or {}
            n = back.get("props")
            if n is not None and int(n) != owners and kept.get(before) is None:
                # not put back: the saved Den stays for the owner (never taken away with the test's files)
                kept.pop(before, None)
            if n is not None:
                rep.ok(int(n) == owners, "your Den put back as it was", "%d props, as before the test" % owners,
                       "Your Den has %s props after the test, %d before: .den load %s-before replace puts it back (the file is kept)." % (n, owners, AIDS_SAVE))
        unstash(kept)


def win_mouse(pid):
    """The real cursor over the game's window (Windows): move(fx, fy) puts it at that fraction of the window's client
    area; aim_click() presses G (the exact click's key while armed) and clicks the left button where the cursor is.
    Each returns "" when done, else why not. None off Windows."""
    if os.name != "nt":
        return None
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32

    class KEYBDINPUT(ctypes.Structure):
        _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD), ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class MOUSEINPUT(ctypes.Structure):
        _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG), ("mouseData", wintypes.DWORD), ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class _U(ctypes.Union):
        _fields_ = [("ki", KEYBDINPUT), ("mi", MOUSEINPUT)]

    class INPUT(ctypes.Structure):
        _fields_ = [("type", wintypes.DWORD), ("u", _U)]

    def key(scan, flags):
        i = INPUT(type=1, u=_U(ki=KEYBDINPUT(wVk=0, wScan=scan, dwFlags=flags, time=0, dwExtraInfo=0)))
        return u.SendInput(1, ctypes.byref(i), ctypes.sizeof(INPUT))

    def button(flags):
        i = INPUT(type=0, u=_U(mi=MOUSEINPUT(dx=0, dy=0, mouseData=0, dwFlags=flags, time=0, dwExtraInfo=0)))
        return u.SendInput(1, ctypes.byref(i), ctypes.sizeof(INPUT))

    class Mouse:
        def front(self):
            h = game_window(pid)
            if not h:
                return None
            key(0x38, KEYEVENTF_SCANCODE)
            key(0x38, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP)
            u.ShowWindow(h, 9)
            u.SetForegroundWindow(h)
            time.sleep(0.3)
            return h if u.GetForegroundWindow() == h else None

        def move(self, fx, fy):
            h = self.front()
            if not h:
                return "the game's window couldn't be brought to the front"
            r = wintypes.RECT()
            u.GetClientRect(h, ctypes.byref(r))
            pt = wintypes.POINT(int(r.right * fx), int(r.bottom * fy))
            u.ClientToScreen(h, ctypes.byref(pt))
            return "" if u.SetCursorPos(pt.x, pt.y) else "SetCursorPos didn't move the cursor"

        def aim_click(self):
            if not self.front():
                return "the game's window couldn't be brought to the front"
            n = key(SCAN_G, KEYEVENTF_SCANCODE) + key(SCAN_G, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP)
            time.sleep(0.5)
            n += button(0x0002)
            time.sleep(0.08)
            n += button(0x0004)
            return "" if n == 4 else "SendInput took %d of 4 events" % n
    return Mouse()


def hover_calibration(rep, ch, den_dir, shoot, outdir, mouse, settle=0.8, wait=4.0, points=CAL_POINTS):
    """The build camera measured on this PC: the real cursor put at a few points of the game window (`mouse.move`);
    at each, where Den Lite's ray put the prop's copy (the live preview) and, pressing G and clicking there
    (`mouse.aim_click`: the game's own ground-target cursor), where the game itself says the point is (Den Lite's
    calibration sample), with a picture. The camera (field of view, pivot, distance, pitch) fitted to those points is
    printed and written to Den Lite's `den-lite.camera.json` (used from the next Den start)."""
    den_dir, outdir = Path(den_dir), Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    if mouse is None:
        rep.add("SKIP", "hover calibration", "the real cursor is moved on Windows only")
        return None
    first = ch.send(".den size")
    if first is None or "aids" not in first:
        rep.add("SKIP", "hover calibration", "Den Lite didn't take the test's commands, or is from before the placement aids")
        return None
    before = den_dir / ("%s-before.den.json" % AIDS_SAVE)
    kept = stash([before, den_dir / "backup.den.json"])
    ch.send(".den save %s-before" % AIDS_SAVE)
    skip = len(read_samples(den_dir))
    try:
        cam_now = dict(CAMERA)
        try:
            cam_now.update({k: float(v) for k, v in json.loads((den_dir / CAMERA_FILE).read_text(encoding="utf-8")).items() if k in CAMERA})
        except (OSError, ValueError, AttributeError):
            pass
        ch.send(".den ui gizmo build on")
        time.sleep(settle)
        ch.send(".den ui gizmo arm 2413")
        time.sleep(settle)
        previews = []
        for i, (fx, fy) in enumerate(points, 1):
            why = mouse.move(fx, fy)
            if why:
                rep.add("FAIL", "hover calibration: point %d" % i, why, "Leave the game window alone during the test, then run it again.")
                continue
            time.sleep(settle)
            a = aids_of(ch.send(".den size"))
            out = outdir / ("calibrate-%d.png" % i)
            ok, _ = shoot(out)
            have = len(read_samples(den_dir, skip))
            why = mouse.aim_click()
            got = wait_for(lambda: len(read_samples(den_dir, skip)) > have or None, wait, 0.2)
            rep.add("INFO", "hover calibration: point %d" % i, "window %.2f, %.2f: cursor %s, the copy at %s, %s%s" % (
                fx, fy, a.get("cursor"), a.get("ghost"), "the click measured" if got else ("no click measured" + (" (%s)" % why if why else "")),
                (", " + out.name) if ok else ""))
            previews.append(a.get("ghost"))
        samples = read_samples(den_dir, skip)
        if len(samples) < 4:
            rep.add("FAIL", "hover calibration", "%d of %d clicks measured: too few to fit the camera" % (len(samples), len(points)),
                    "G must start the exact-click cursor (the Leyline UI's build mode, armed): send logs\\den-features-latest.txt and the pictures.")
            return None
        err0 = camera_error(cam_now, samples)
        cam, err = fit_camera(samples, cam_now)
        rep.add("INFO", "hover calibration: before", "the preview was %.2f yd off the real point on average (camera %s)" % (
            err0, ", ".join("%s %.3f" % (k, cam_now[k]) for k in FIT_KEYS)))
        rep.ok(err <= 0.6, "hover calibration: the camera fitted",
               "fov %.3f rad (diagonal), pivot %.2f yd, distance %.2f yd, pitch %.3f rad: %.2f yd off on average over %d points" % (
                   cam["fov"], cam["pivot"], cam["dist"], cam["pitch"], err, len(samples)),
               "No camera fits within 0.6 yd (the camera moved during the test?): send logs\\den-features-latest.txt and the pictures.")
        if err <= 0.6 and err < err0:
            try:
                saved = {k: round(cam[k], 4) for k in FIT_KEYS}
                # the window's shape it was measured at (Den Lite ignores it): the field of view is fitted as a
                # diagonal one, which may not be how the game keeps it, so another shape is worth measuring again
                asp = calibrated_aspect(samples)
                if asp:
                    saved["aspect"] = asp
                (den_dir / CAMERA_FILE).write_text(json.dumps(saved), encoding="utf-8")
                rep.add("INFO", "hover calibration: saved", "%s (Den Lite uses it from the next Den start)" % CAMERA_FILE)
            except OSError as e:
                rep.add("INFO", "hover calibration: not saved", str(e)[:120])
        return cam, err
    finally:
        try:
            ch.send(".den ui gizmo cancel")
            ch.send(".den ui gizmo build off")
        finally:
            if before.is_file():
                ch.send(".den load %s-before replace" % AIDS_SAVE)
            unstash(kept)
            if kept.get(before) is None:
                try:
                    before.unlink()
                except OSError:
                    pass


def guest_name(den_dir, user):
    """The friend's character name for a guest account (Den Lite's guests.json; never its verifier)."""
    try:
        doc = json.loads((Path(den_dir) / "guests.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    items = doc if isinstance(doc, list) else doc.get("guests", []) if isinstance(doc, dict) else []
    for g in items:
        if isinstance(g, dict) and str(g.get("user", "")).upper() == user.upper():
            return g.get("name")
    return None


def no_secrets_in_logs(rep, secrets, files, text):
    """The invite's secret and the friend's password are in none of the test's logs."""
    if not secrets:
        return
    hits = []
    for f in files:
        try:
            t = Path(f).read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        if any(s in t for s in secrets):
            hits.append(Path(f).name)
    if any(s in text for s in secrets):
        hits.append("this report")
    rep.ok(not hits, "invite: its secret is in no log", "checked %d logs" % (len(files) + 1) if not hits else ", ".join(hits),
           "An invite's secret was written to a log: send this report (not the logs) to the developers.")


# ------------------------------------------------------------------------------------------------ the newest Den Lite

def _fetch(url, limit):
    req = urllib.request.Request(url, headers={"User-Agent": "leyline-den-check"})
    with urllib.request.urlopen(req, timeout=60) as r:
        data = r.read(limit + 1)
    if len(data) > limit:
        raise RuntimeError("too big")
    return data


def sha_in(text):
    t = (text or "").split()
    t = t[0].lower() if t else ""
    return t if len(t) == 64 and all(c in "0123456789abcdef" for c in t) else None


def den_lite_asset():
    return "den-lite-windows-x64.exe" if os.name == "nt" else "den-lite-linux-x64"


GLIDE_MARKER = "den-creature-models.json"   # labs/den-lite glide.rs MARKER (launcher host/creaturemodels.rs)


def glide_ready(den_dir):
    """Whether Den Lite will glide the copy in hand: the marker is there and, when it names the archive holding the
    rows (Den Lite's own glide-rows), that archive is still in the game's Data folder. (ok, why)."""
    m = read_json(Path(den_dir) / GLIDE_MARKER)
    if not isinstance(m, dict) or not m.get("displays"):
        return False, "no creature rows marked yet"
    a = m.get("archive")
    if a and not Path(a).is_file():
        return False, "the rows' archive %s is gone" % a
    return True, "%d prop models can glide%s" % (len(m["displays"]), (" (%s)" % Path(a).name) if a else "")


def glide_rows(rep, den_exe, game, den_dir, undo=False, run=subprocess.run):
    """Den Lite's `glide-rows` (labs/den-lite gliderows.rs): the launcher's creature rows, from this PC's own game
    tables, into Data/patch-S.MPQ (backup first; `undo` takes them away), so the copy in hand glides before a launcher
    with them is out. A line in the report either way; True when the rows are there."""
    name = "the gliding copy's rows (your own game's tables)"
    if not Path(den_exe).is_file():
        rep.add("INFO", name, "no Den Lite at %s: run den-lite first" % den_exe)
        return False
    cmd = [str(den_exe), "glide-rows", "--game", str(game), "--dens", str(den_dir)] + (["--undo"] if undo else [])
    try:
        r = run(cmd, capture_output=True, text=True, timeout=120)
    except (OSError, subprocess.SubprocessError) as e:
        rep.add("INFO", name, "Den Lite couldn't be run: %s" % e)
        return False
    if r.returncode == 2:
        rep.add("INFO", name, "this Den Lite is from before glide-rows: run den-lite, then again")
        return False
    try:
        out = json.loads((r.stdout or "").strip().splitlines()[-1])
    except (ValueError, IndexError):
        out = {"ok": False, "said": (r.stderr or r.stdout or "no answer").strip()[:300]}
    rep.ok(bool(out.get("ok")), name, str(out.get("said", "")), "%s: the copy stays a gameobject made anew (no glide)." % out.get("said", "it didn't work"))
    return bool(out.get("ok"))


def built_here(den):
    """True when Den Lite here was built on this PC by the den-lite step (tools/den-lite/Build Den Lite.bat writes
    den-lite.built-here beside it), and not replaced since: the test then runs that build, not the release's."""
    mark = den.with_name("den-lite.built-here")
    try:
        return den.is_file() and mark.is_file() and den.stat().st_mtime <= mark.stat().st_mtime + 5
    except OSError:
        return False


def den_lite_update(rep, root, fetch=_fetch):
    """The newest Den Lite from its release (the one the launcher gets on first use), when the one here differs: the
    release's sha256 checked before it is put in place. A Den Lite left running is asked to stop first (the test
    starts its own)."""
    den = Path(root) / "bin" / exe_name("den-lite")
    if built_here(den):
        rep.add("INFO", "the newest Den Lite", "the one built on this PC (den-lite step) is used, not the release's")
        return
    try:
        want = sha_in(fetch(DEN_LITE_RELEASE + den_lite_asset() + ".sha256", 4096).decode("utf-8", "replace"))
    except Exception as e:  # noqa: BLE001 - offline is fine: the one here is used
        rep.add("INFO", "the newest Den Lite", "couldn't check (%s): the one here is used" % str(e)[:80])
        return
    if not want:
        rep.add("INFO", "the newest Den Lite", "the release's checksum doesn't look right: the one here is used")
        return
    have = hashlib.sha256(den.read_bytes()).hexdigest() if den.is_file() else None
    if have == want:
        rep.add("INFO", "the newest Den Lite", "already here")
        return
    try:
        data = fetch(DEN_LITE_RELEASE + den_lite_asset(), 64 << 20)
    except Exception as e:  # noqa: BLE001
        rep.add("INFO", "the newest Den Lite", "didn't download (%s): the one here is used" % str(e)[:80])
        return
    if hashlib.sha256(data).hexdigest() != want:
        rep.add("INFO", "the newest Den Lite", "the download doesn't match its checksum: the one here is used")
        return
    dens = Path(root) / "state" / "dens"
    if port_open(3724) and (dens / "den-lite.alive").is_file():
        try:
            (dens / "den-lite.stop").write_text("")
        except OSError:
            pass
        wait_for(lambda: not port_open(3724), 8, 0.3)
    den.parent.mkdir(parents=True, exist_ok=True)
    new = den.with_name(den.name + ".new")
    new.write_bytes(data)
    if os.name != "nt":
        os.chmod(new, 0o755)
    try:
        os.replace(new, den)
    except OSError as e:
        new.unlink()
        rep.add("INFO", "the newest Den Lite", "couldn't be put in place (%s): the one here is used" % str(e)[:80])
        return
    try:
        (den.parent / "den-lite.json").write_text(json.dumps({"sha256": want, "from": DEN_LITE_RELEASE + den_lite_asset()}, indent=2), encoding="utf-8")
    except OSError:
        pass
    rep.add("INFO", "the newest Den Lite", "put in place (sha256 %s)" % want[:12])


# ------------------------------------------------------------------------------------------------ test

DEN_TEXTURE = "Tileset\\Leyline\\DenWhite.blp"
# the Den's ground since 3 Nov (owner 08:41: "a default Elwynn ground color grass texture and ... a realm skybox"): the
# game's own grass by name (launcher terrain crate pad.rs PAD_TEXTURE); 10:29 "do blue forest sky not grey green": the
# Elwynn sky (the player's own Light.dbc, its Den lights pointed at Elwynn's: terrain crate dbc.rs elwynn_sky), the one
# table of the game's in patch-T
GRASS = "Tileset\\Elwynn\\ElwynnGrassBase.blp"
SKY = "the Elwynn sky"
# what a launcher from before the Elwynn sky (commit bf14145, 3 Nov 10:48) says: the owner's den-features of 11:20 ran
# one ("FAIL sky: the sky is the area's own", no reason in brackets: the newer launcher always gives one)
OLD_SKY = "the area's own"
LIGHT_TABLE = "DBFilesClient\\Light.dbc"
# game languages whose own patches are read AFTER Data\patch-T.MPQ (wotlkdev.github.io wiki/theory/client: on
# these the locale folder's patches win; on the others Data's lettered patches win)
LOCALE_FIRST = ("ruru", "zhcn", "zhtw")


def blp_is_ours(b):
    """Our old white texture (launcher terrain crate blp.rs): BLP2, type 1, palette, no alpha, mips; the palette's
    first colour white and every pixel using it."""
    if not b or len(b) < 1172 or b[:4] != b"BLP2":
        return False
    kind, comp, depth, _atype, mips, w, h = struct.unpack_from("<IBBBBII", b, 4)
    off0, size0 = struct.unpack_from("<I", b, 20)[0], struct.unpack_from("<I", b, 84)[0]
    # our made one (palette or DXT1), or the light snow texture from the player's own archives the launcher put at
    # our name (2 Nov): any whole BLP2 with mips
    return kind == 1 and comp in (1, 2, 3) and mips in (1, 2) and w > 0 and h > 0 and off0 >= 148 and len(b) >= off0 + size0


def patch_extras(names):
    """Of patch-T's file names, what isn't the tile, our mark or the Elwynn sky's Light.dbc: the game's other tables
    (the old white sky and zone name) and textures (the old white ground). Since 3 Nov there should be none."""
    out = []
    for n in names:
        low = n.lower()
        if low == LIGHT_TABLE.lower():
            continue
        if low.startswith("dbfilesclient\\") or low.endswith(".blp"):
            out.append(n)
    return out


def locale_light(data):
    """For a game whose language's own patches are read after Data\\patch-T.MPQ (LOCALE_FIRST): the first of those
    that has its own Light.dbc (it would hide the Elwynn sky), else None."""
    if not data:
        return None
    try:
        dirs = [d for d in Path(data).iterdir() if d.is_dir() and d.name.lower() in LOCALE_FIRST]
    except OSError:
        return None
    for d in dirs:
        for a in sorted(d.glob("*.[Mm][Pp][Qq]"), reverse=True):
            if not a.name.lower().startswith("patch-"):
                continue
            try:
                if any(n.lower() == LIGHT_TABLE.lower() for n in mpqlite.Archive(str(a)).names()):
                    return "%s\\%s" % (d.name, a.name)
            except Exception:  # noqa: BLE001 - an archive that doesn't read hides nothing we can tell
                continue
    return None


def ground_lines(rep, detail, patch_t=None, data=None):
    """The Den's world, from the launcher's report (`ground_detail`, host/terrain.rs GroundStatus) and our patch-T:
    the ground painted the game's own grass (only its name in the tile), nothing of the island on the tile, the
    Elwynn sky (Light.dbc the only table of the game's in patch-T; the zone's own name). Only the picture shows what
    the game drew."""
    if not isinstance(detail, dict) or not detail:
        rep.add("INFO", "the Den's world", "the launcher doesn't say (an older launcher)")
        return
    if "grass" not in detail:
        rep.add("INFO", "the Den's ground", "an older launcher: the white ground (sky: %s, area: %s); update the launcher for the grass" %
                (detail.get("sky", "?"), detail.get("area", "?")))
    else:
        name = detail.get("texture_name") or GRASS
        rep.ok(bool(detail.get("grass")), "grass ground: the game's own texture",
               "%s, from your own game (only its name is in the Den's tile)" % name if detail.get("grass") else
               "your game hasn't a grass texture Leyline knows (the tile names %s)" % name,
               "Send this log: it says which grass name your game has.")
    left = int(detail.get("island_models", -1))
    mine = int(detail.get("models", 0)) - max(left, 0)
    rep.ok(left == 0, "no buildings/trees on the Den tile",
           ("the island's models are gone" + (", %d of yours stay" % mine if mine > 0 else "")) if left == 0 else "%s of the island's models still stand" % left,
           "Send this log (the launcher should take them off at the start).")
    names = None
    if patch_t is not None:
        try:
            names = mpqlite.Archive(str(patch_t)).names()
            extra = patch_extras(names)
            err = None
        except Exception as e:  # noqa: BLE001 - said below
            extra, err = [], str(e)[:120]
        if err:
            rep.add("FAIL", "zone: the game's own, no other table", "Data\\patch-T.MPQ couldn't be read: %s" % err, "Start My Den once more; still not: send this log.")
        else:
            rep.ok(not extra, "zone: the game's own, no other table",
                   "the zone's own name: Light.dbc (the sky) is the only table of the game's in Data\\patch-T.MPQ, no texture" if not extra else
                   "Data\\patch-T.MPQ still has %s" % ", ".join(extra[:4]),
                   "An older launcher made it: update the launcher and start My Den once more.")
    else:
        rep.add("INFO", "sky/area", "sky: %s; zone: %s" % (detail.get("sky", "?"), detail.get("area", "?")))
    sky = str(detail.get("sky", "?"))
    if "grass" in detail:
        hidden = [h for h in detail.get("hidden_by") or [] if LIGHT_TABLE.lower() in str(h).lower()]
        loc = locale_light(data) if sky == SKY else None
        if sky == OLD_SKY:
            rep.add("FAIL", "sky: the Elwynn sky", "the sky is the area's own: this launcher is from before the Elwynn sky",
                    "Update the launcher (Ship, then wait for its build; den-test gets the newest), then run den-features again.")
        elif sky != SKY:
            rep.add("FAIL", "sky: the Elwynn sky", "the sky is %s" % sky, "Send this log: it says why your game's Light.dbc wasn't used.")
        elif names is not None and not any(n.lower() == LIGHT_TABLE.lower() for n in names):
            rep.add("FAIL", "sky: the Elwynn sky", "the launcher says the Elwynn sky, but Data\\patch-T.MPQ has no Light.dbc",
                    "Start My Den once more; still not: send this log.")
        elif hidden:
            rep.add("FAIL", "sky: the Elwynn sky", "an archive read after the Den's has its own Light.dbc: %s" % hidden[0][:120],
                    "Move that archive out of Data (it hides the Den's sky), then start My Den once more.")
        elif loc:
            rep.add("FAIL", "sky: the Elwynn sky", "your game's language reads its own Light.dbc after the Den's: %s" % loc,
                    "Send this log: Leyline has to put the Den's sky in your language's folder too.")
        else:
            rep.add("PASS", "sky: the Elwynn sky", "Elwynn Forest's blue sky over the Den (your own Light.dbc%s)" % (", in Data\\patch-T.MPQ" if names else ""))
    for h in detail.get("hidden_by") or []:
        rep.add("INFO", "an archive read after the Den's hides part of it", str(h)[:200])
    if detail.get("whitened"):
        rep.add("INFO", "your Den's ground from before", "made grass: %s" % str(detail["whitened"])[:200])


# ------------------------------------------------------------------------------------------------ keys: two ways
# 3 Nov: the game sent no movement at all, on the launcher's posted keys (denclick.rs PostMessage WM_KEYDOWN) and on
# the owner's own W. The test now also presses W the way a keyboard does (SendInput, scan codes, the game in front),
# asks the Leyline UI for its view before and after (Den Lite `.den diag`), and says which way moved the character.
SCAN_W = 0x11
SCAN_UP = 0x48
KEYEVENTF_EXTENDEDKEY = 0x0001
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_SCANCODE = 0x0008


def key_events(scan, extended=False):
    """One key press as SendInput events: (scan code, flags) down, then up."""
    base = KEYEVENTF_SCANCODE | (KEYEVENTF_EXTENDEDKEY if extended else 0)
    return [(scan, base), (scan, base | KEYEVENTF_KEYUP)]


def diag_blocks(evs):
    """The Leyline UI's answers (addon events `wudiag ...`), grouped by the tag of the ask they answer (`at:<tag>`;
    lines before any tag: "login")."""
    out, tag = {}, "login"
    for e in evs:
        d = str(e.get("detail", ""))
        if e.get("ev") != "addon" or not d.startswith("wudiag "):
            continue
        d = d[7:]
        if d.startswith("at:"):
            tag = d[3:].strip() or "?"
            out.setdefault(tag, [])
            continue
        out.setdefault(tag, []).append(d)
    return out


def moves_of(answer):
    """How many movement packets Den Lite has seen from the game, from a `.den where` answer (test-out's `me`)."""
    me = answer.get("me") if isinstance(answer, dict) else None
    return int(me.get("moves", 0)) if isinstance(me, dict) else None


def method_lines(rep, posted_moved, sent, before, after, blocks, evs):
    """Which way of pressing W moved the character, and the UI's view before and after.
    posted_moved: the launcher's posted keys moved it (Den Lite saw movement before this check); sent: what the
    SendInput press did ("" when it couldn't run, with why); before/after: Den Lite's movement count around it."""
    rep.add("INFO", "keys: posted to the window (the launcher)", "the game moved" if posted_moved else "no movement from the game")
    if sent:
        rep.add("INFO", "keys: pressed (SendInput, the game in front)", sent)
    elif before is None or after is None:
        rep.add("INFO", "keys: pressed (SendInput, the game in front)", "Den Lite didn't answer `.den where` around it")
    else:
        moved = after > before
        rep.ok(moved or posted_moved, "keys: pressed (SendInput, the game in front)",
               "the game moved: %d movement packets" % (after - before) if moved else "no movement from the game either",
               "Neither way of pressing W moves the character: the game itself holds it. Send logs\\den-test-latest.txt "
               "and the screenshot (the lines below say what the game sent and what the Leyline UI saw).")
    for tag in ("before", "after"):
        for line in blocks.get(tag, [])[:6]:
            rep.add("INFO", "the game says (%s)" % tag, line[:200])
    ops = [str(e.get("detail", "")) for e in evs if e.get("ev") == "client_op"]
    if ops:
        rep.add("INFO", "what the game sent in the world", ", ".join(ops)[:300])
    ts = [e for e in evs if e.get("ev") == "time_sync"]
    rep.add("INFO", "time sync", str(ts[0].get("detail")) if ts else "the game never answered the time sync")


def game_window(pid):
    """The game's main window (class GxWindowClass*) by process id (Windows), or None."""
    if os.name != "nt":
        return None
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    found = []
    proto = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def each(h, _lp):
        p = wintypes.DWORD()
        u.GetWindowThreadProcessId(h, ctypes.byref(p))
        buf = ctypes.create_unicode_buffer(64)
        u.GetClassNameW(h, buf, 64)
        if p.value == int(pid) and u.IsWindowVisible(h) and buf.value.startswith("GxWindowClass"):
            found.append(h)
            return False
        return True
    u.EnumWindows(proto(each), 0)
    return found[0] if found else None


def send_input_keys(pid, hold_ms=900):
    """W, then the up arrow, as a keyboard sends them (SendInput, scan codes), with the game's window brought to the
    front first. Returns "" when the presses went in, else why they couldn't."""
    if os.name != "nt":
        return "SendInput is on Windows only"
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    h = game_window(pid)
    if not h:
        return "the game's window wasn't found"

    class KEYBDINPUT(ctypes.Structure):
        _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD), ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class MOUSEINPUT(ctypes.Structure):
        _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG), ("mouseData", wintypes.DWORD), ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class _U(ctypes.Union):
        _fields_ = [("ki", KEYBDINPUT), ("mi", MOUSEINPUT)]

    class INPUT(ctypes.Structure):
        _fields_ = [("type", wintypes.DWORD), ("u", _U)]

    def press(events):
        n = 0
        for scan, flags in events:
            i = INPUT(type=1, u=_U(ki=KEYBDINPUT(wVk=0, wScan=scan, dwFlags=flags, time=0, dwExtraInfo=0)))
            n += u.SendInput(1, ctypes.byref(i), ctypes.sizeof(INPUT))
        return n

    # Windows lets a program take the foreground only after a key press of its own: a bare Alt first
    alt = 0x38
    press([(alt, KEYEVENTF_SCANCODE), (alt, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP)])
    u.ShowWindow(h, 9)
    u.SetForegroundWindow(h)
    time.sleep(0.4)
    if u.GetForegroundWindow() != h:
        return "the game's window couldn't be brought to the front"
    sent = 0
    for scan, ext in ((SCAN_W, False), (SCAN_UP, True)):
        down, up = key_events(scan, ext)
        sent += press([down])
        time.sleep(hold_ms / 1000)
        sent += press([up])
        time.sleep(0.3)
    return "" if sent == 4 else "SendInput took %d of 4 key events (another program may block input)" % sent


def keys_check(rep, den_dir, pid, start, ch=None, press=send_input_keys, wait=2.5):
    """The second way of pressing W, Den Lite's movement count around it and the UI's view before and after."""
    ch = ch or FileChannel(den_dir)
    evs = events(den_dir, start)
    posted_moved = any(e.get("ev") in ("first_move", "walked") for e in evs)
    ch.send(".den diag before", wait=5.0)
    time.sleep(1.0)
    before = moves_of(ch.send(".den where", wait=5.0))
    why = press(pid)
    time.sleep(wait)
    after = moves_of(ch.send(".den where", wait=5.0))
    ch.send(".den diag after", wait=5.0)
    time.sleep(1.5)
    evs = events(den_dir, start)
    method_lines(rep, posted_moved, why, before, after, diag_blocks(evs), evs)


def test(args, rep):
    app = Path(args.app_dir) if args.app_dir else app_dir()
    root = app / "den-lite"
    den_dir = root / "state" / "dens"
    logs = HERE / "logs"
    logs.mkdir(exist_ok=True)
    shot = logs / "den-test-latest.png"
    for p in (shot, root / "den-click.json", root / "den-click.shot", root / "den-click.invite", root / "den-click.forge.json",
              root / "den-click.type", root / "den-click.typed", den_dir / "den-lite.test-cmd", den_dir / "den-lite.test-out"):
        try:
            p.unlink()
        except OSError:
            pass
    exe = launcher_exe()
    if not rep.ok(exe is not None and exe.is_file(), "the launcher program", str(exe), "Run \"Open Leyline Launcher.bat\" once (it puts .launcher\\Leyline.exe in place)."):
        return
    raw = exe.read_bytes()
    if not rep.ok(CLI_MARK in raw, "the launcher has My Den's test start (den-play)", "", "Update the launcher (the release with den-play), then run den-test again."):
        return
    marks = {m for m in (TEST_MARK, SHARE_MARK) if m in raw}
    c = den_game(settings(app))
    if not rep.ok(c is not None, "game folder for Your Den", c.get("path") if c else "", "Add your 3.3.5a game folder in the launcher."):
        return
    stock_addons(rep, Path(c["path"]))
    if not args.no_update:
        den_lite_update(rep, root)
    # the gliding copy (owner 5 Nov): its creature rows on this PC before the test, when they aren't there yet (the
    # installed launcher doesn't write them; Den Lite's glide-rows does, from this PC's own tables)
    if getattr(args, "move", False) or getattr(args, "features", False):
        ok, why = glide_ready(den_dir)
        if ok:
            rep.add("INFO", "the gliding copy's rows", why)
        else:
            rep.add("INFO", "the gliding copy's rows", why + ": made now")
            glide_rows(rep, root / "bin" / exe_name("den-lite"), Path(c["path"]), den_dir)
    hold = 300
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
    secrets = []
    steps = Report()
    if rpt.get("phase") == "in_world":
        # the step forward and the Leyline UI's hello (the launcher steps up to three times, 3 s apart, until Den Lite
        # saw the character walk), then where the character stands, the Den's smoke test, then the picture
        for _ in range(100):
            evs = events(den_dir, rpt["start"])
            if {"walked", "builder_ready"} <= {e["ev"] for e in evs}:
                break
            time.sleep(0.25)
        # the steps' lines go after the sign-in's milestones (below), in the order they ran
        steps.add = _quiet_add(steps)
        try:
            where = FileChannel(den_dir).send(".den where", wait=5.0)
            walk_lines(steps, events(den_dir, rpt["start"]), where)
        except Exception as e:  # noqa: BLE001 - the smoke test still runs
            steps.add("FAIL", "walking", "the check stopped: %s" % str(e)[:200], "Send logs\\den-test-latest.txt.")
        if not getattr(args, "move", False):   # den-move: only select and move (owner 3 Nov)
            try:
                keys_check(steps, den_dir, pid, rpt["start"])
            except Exception as e:  # noqa: BLE001 - the smoke test still runs
                steps.add("INFO", "keys: pressed (SendInput)", "the check stopped: %s" % str(e)[:200])
            try:
                secrets = den_steps(steps, FileChannel(den_dir), den_dir, root, marks, rpt["start"])
            except Exception as e:  # noqa: BLE001 - the game still gets its picture and is closed
                steps.add("FAIL", "the Den's smoke test", "stopped: %s" % str(e)[:200], "Send logs\\den-test-latest.txt.")
        if getattr(args, "move", False):
            try:
                aids_features(steps, FileChannel(den_dir), den_dir, lambda out: screenshot(pid, out), logs / "den-move", only_move=True)
            except Exception as e:  # noqa: BLE001 - the game still gets its picture and is closed
                steps.add("FAIL", "select and move", "stopped: %s" % str(e)[:200], "Send logs\\den-move-latest.txt.")
        elif getattr(args, "features", False):
            try:
                den_features(steps, FileChannel(den_dir), den_dir, lambda out: screenshot(pid, out), logs / "den-features")
            except Exception as e:  # noqa: BLE001 - the game still gets its picture and is closed
                steps.add("FAIL", "the editor's feature test", "stopped: %s" % str(e)[:200], "Send logs\\den-test-latest.txt.")
            try:
                dojo_features(steps, FileChannel(den_dir), den_dir, lambda out: screenshot(pid, out), logs / "den-features")
            except Exception as e:  # noqa: BLE001 - the game still gets its picture and is closed
                steps.add("FAIL", "the Dojo's combat test", "stopped: %s" % str(e)[:200], "Send logs\\den-test-latest.txt.")
            try:
                aids_features(steps, FileChannel(den_dir), den_dir, lambda out: screenshot(pid, out), logs / "den-features")
                hover_calibration(steps, FileChannel(den_dir), den_dir, lambda out: screenshot(pid, out), logs / "den-features", win_mouse(pid))
            except Exception as e:  # noqa: BLE001 - the game still gets its picture and is closed
                steps.add("FAIL", "the placement aids", "stopped: %s" % str(e)[:200], "Send logs\\den-test-latest.txt.")
        time.sleep(1.5)
        ok, detail = screenshot(pid, shot)
        steps.add("INFO" if ok else "FAIL" if os.name == "nt" else "INFO", "screenshot", detail, "PowerShell couldn't capture the game window.")
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
    if "launch" in rpt:
        rep.add("INFO", "the launcher said", str(rpt.get("launch"))[:300])
        if rpt.get("addons"):
            rep.add("INFO", "Leyline addons", str(rpt.get("addons"))[:200])
        rep.ok(bool(rpt.get("ground")), "the Den's ground (the open grass plane) was put in the game",
               "your Den's ground in Data/patch-T.MPQ",
               "The launcher couldn't make your Den's ground: see 'the launcher said' above and send this log.")
        data = find_ci(Path(c["path"]), "Data")
        ground_lines(rep, rpt.get("ground_detail"), find_ci(data, "patch-T.MPQ") if data else None, data)
    crcs = [e for e in events(den_dir, rpt["start"]) if e.get("ev") == "addon_crcs"]
    if crcs:
        rep.add("INFO", "the game's addon list (key checks)", str(crcs[-1].get("detail"))[:300])
    starts = [e for e in events(den_dir, rpt["start"] - 120_000) if e.get("ev") == "start"]
    rep.add("INFO", "Den Lite's start", (str(starts[-1].get("detail")) + (" (with the test's channel)" if str(starts[-1].get("detail", "")).endswith(" test") else " (WITHOUT the test's channel)")) if starts else "no start in its log in the last 2 minutes (one was already running)")
    if rpt.get("phase") == "in_world" or steps.rows:
        print("")
        print("In your Den:")
        for kind, step, detail, fix in steps.rows:
            Report.add(rep, kind, step, detail, fix)
    elif rpt.get("phase") != "in_world":
        rep.add("SKIP", "the Den's smoke test", "the game didn't reach the world")
    for p in (root / "den-click.invite", root / "den-click.forge.json"):
        try:
            p.unlink()
        except OSError:
            pass
    rep.ok(bool(rpt.get("closed")), "the game and the test closed",
           "Den Lite stopped too" + (" (it was running: restarted for the test; My Den starts it again)" if rpt.get("den_lite_restarted") else "")
           if rpt.get("den_lite_stopped") else "Den Lite was already running: left running",
           "The launcher didn't finish the test: close the game by hand and run den-test again.")
    game = Path(c["path"])
    z = find_ci(find_ci(game, "Data") or game, "patch-Z.MPQ")
    left = bool(z and our_archive(z)[1])
    rep.ok(not left, "the handoff is gone", "no sign-in left in the game folder" if not left else "still in Data\\patch-Z.MPQ",
           "Start and close My Den once; it takes the sign-in out.")
    no_secrets_in_logs(rep, secrets, [root / "den-click.log", den_dir / "den-lite.events", den_dir / "den-lite.test-out", den_dir / "companion-in.json"],
                       rep.text())


def _quiet_add(r):
    """The steps' lines are printed as they happen (the owner sees progress) and kept for the report's own order."""
    def add(kind, step, detail="", fix=""):
        r.rows.append((kind, step, detail, fix))
        print("     ... %s %s" % (kind, step), flush=True)
    return add


# ------------------------------------------------------------------------------------------------ the manual pass
# Owner 4 Nov: "give me the test to run as a bat i can do manually in den mode". Your Den is started the way the My Den
# button starts it (no test keys, the game stays open); then each check is shown here, you try it in the game and
# answer. What Den Lite heard during each check goes in the log with your answer.
MANUAL_STEPS = [
    ("debug box", "Type /wudebug in the game's chat. A box opens at the top left: the editor's state and Den Lite's.",
     "the box shows build=on (press Build on the Den bar if it says off)"),
    ("hover glow", "Move the mouse over a prop (place one from the Builder first if your Den is empty), then off it.",
     "the prop under the mouse glows; it stops glowing when the mouse leaves; only one glows at a time"),
    ("click picks up", "Left-click a prop once and let go of the button. Move the mouse.",
     "it comes up at once and its foot follows the mouse over the ground, no button held, no lag"),
    ("over the ground", "While carrying, sweep the mouse over raised ground or a slope (Shape ground first if it is flat).",
     "the prop rides the ground: never sunk into it, never floating"),
    ("the edge", "While carrying, move the mouse past your Den's edge.",
     "the prop stops at the edge and waits there; back inside it follows again"),
    ("click puts down", "Left-click again.",
     "it is put down where you saw it, stays selected (glow and halo) with three flags: red X, green Y, blue Z"),
    ("the arrows", "Press and drag the red flag, then the green, then the blue. Let go each time.",
     "red moves it along X only, green along Y only, blue up and down; it stays where you let go"),
    ("turn", "With it selected (or carried), press Q and E.",
     "it turns left and right"),
    ("Esc", "Click it to pick it up again, move it, then press Esc.",
     "it goes back where it was, nothing changed"),
    ("a card", "Pick a prop in the Builder (the gallery), move the mouse over the ground, click.",
     "the real model follows the mouse; one click places it and selects it"),
    ("Shift adds", "Shift-click a second prop.",
     "both are selected"),
    ("the views", "Press the snap key (or the wheel past the game's own zoom), then back.",
     "the view goes up high and back to you"),
    ("free flight", "In the high view, hold W, then W with A or D, then Space and X; turn with the right mouse button while "
     "flying; hold Shift. Watch the 'flight:' line in the /wudebug box.",
     "the view glides smoothly with no stops or jumps back; diagonals are smooth; Space rises, X sinks; it flies the way "
     "the camera faces after a turn; Shift is faster; it eases to a stop when you let go"),
    ("play mode", "Press F7 (or Play on the Den bar). Walk with W.",
     "props are plain again (no glow), you walk; F7 brings Build back"),
    ("debug box off", "Type /wudebug again to hide the box.",
     "the box goes; nothing else changed"),
]


def manual(args, rep):
    app = Path(args.app_dir) if args.app_dir else app_dir()
    root = app / "den-lite"
    den_dir = root / "state" / "dens"
    exe = launcher_exe()
    if not rep.ok(exe is not None and exe.is_file(), "the launcher program", str(exe), "Run \"Open Leyline Launcher.bat\" once (it puts .launcher\\Leyline.exe in place)."):
        return
    c = den_game(settings(app))
    if not rep.ok(c is not None, "game folder for Your Den", c.get("path") if c else "", "Add your 3.3.5a game folder in the launcher."):
        return
    if not args.no_update:
        den_lite_update(rep, root)
    start = now_ms()
    subprocess.Popen([str(exe), "den-play", "--client", c["id"]], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    rep.add("INFO", "started", "%s den-play (the My Den button; the game stays open)" % exe.name)
    print("Starting Your Den: the game signs in by itself. Wait until you stand in your Den...")
    end = time.time() + args.timeout
    inw = None
    while time.time() < end and inw is None:
        inw = event_after(den_dir, "in_world", start)
        time.sleep(0.5)
    if not rep.ok(inw is not None, "in your Den", str((inw or {}).get("detail", "")), "The game didn't get into your Den: run den-move and send its log."):
        return
    print()
    print("In your Den. Each check: try it in the game, then answer here.")
    print("  Enter = it works   n = it doesn't   s = skip   (after n you can type what you saw)")
    for i, (name, do, want) in enumerate(MANUAL_STEPS, 1):
        print()
        print("%d/%d  %s" % (i, len(MANUAL_STEPS), name.upper()))
        print("  Do:     " + do)
        print("  Expect: " + want)
        t = now_ms()
        try:
            ans = input("  Works? [Enter/n/s] ").strip().lower()
        except EOFError:
            ans = "s"
        note = ""
        if ans.startswith("n"):
            try:
                note = input("  What did you see? ").strip()
            except EOFError:
                note = ""
        heard = [str(e.get("detail", "")) for e in events(den_dir, t) if e.get("ev") == "addon" and "wubuild" in str(e.get("detail", ""))]
        said = [e.get("ev") + " " + str(e.get("detail", "")) for e in events(den_dir, t) if e.get("ev") in ("den_change", "build_mode", "error", "refused")]
        heard_txt = "; ".join(heard[-12:] + said[-6:])[:600] or "nothing from the editor"
        if ans.startswith("s"):
            rep.add("SKIP", "manual: " + name, "skipped")
        else:
            rep.ok(not ans.startswith("n"), "manual: " + name, want if not ans.startswith("n") else (note or "it didn't"),
                   "Den Lite heard: " + heard_txt)
        rep.add("INFO", "manual: %s: Den Lite heard" % name, heard_txt)
    print()
    print("Done: you can close the game.")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", choices=["check", "test", "features", "move", "manual", "glide", "glide-undo"])
    ap.add_argument("--game")
    ap.add_argument("--app-dir")
    ap.add_argument("--no-login", action="store_true")
    ap.add_argument("--invite", action="store_true", help="kept for the den-test-invite step: the invite is always tested now")
    ap.add_argument("--no-update", action="store_true", help="test: use the Den Lite on this PC as it is")
    ap.add_argument("--timeout", type=int, default=150)
    a = ap.parse_args(argv)
    rep = Report()
    if a.mode in ("glide", "glide-undo"):
        # the Control step den-glide: the gliding copy's creature rows on this PC (or taken away)
        app = Path(a.app_dir) if a.app_dir else app_dir()
        c = {"path": a.game} if a.game else den_game(settings(app))
        title = "DEN GLIDE ROWS"
        print("%s (%s)" % (title, time.strftime("%Y-%m-%d %H:%M")))
        if rep.ok(c is not None, "game folder for Your Den", c.get("path") if c else "", "Add your 3.3.5a game folder in the launcher."):
            den_dir = app / "den-lite" / "state" / "dens"
            glide_rows(rep, app / "den-lite" / "bin" / exe_name("den-lite"), Path(c["path"]), den_dir, undo=a.mode == "glide-undo")
            rep.add("INFO", "Den Lite will glide", glide_ready(den_dir)[1])
        return rep.end(title)
    a.move = a.mode == "move"
    a.features = a.mode == "features"
    if a.features or a.move:
        a.mode = "test"
        a.timeout = max(a.timeout, 150)
    if a.mode == "manual":
        title = "DEN MANUAL PASS"
        print("%s (%s)" % (title, time.strftime("%Y-%m-%d %H:%M")))
        manual(a, rep)
        code = rep.end(title)
        logs = HERE / "logs"
        logs.mkdir(exist_ok=True)
        (logs / "den-manual-latest.txt").write_text("%s (%s)\n%s\n%s" % (title, time.strftime("%Y-%m-%d %H:%M"), rep.text(),
                                                    "PASS" if code == 0 else "FAIL"), encoding="utf-8")
        print("Saved %s" % (logs / "den-manual-latest.txt"))
        return code
    title = "DEN CHECK" if a.mode == "check" else ("DEN MOVE TEST" if a.move else "DEN FEATURE TEST" if a.features else "DEN TEST")
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
        latest = "den-move-latest.txt" if a.move else "den-features-latest.txt" if a.features else "den-test-latest.txt"
        (logs / latest).write_text("%s (%s)\n%s\n%s%s" % (title, time.strftime("%Y-%m-%d %H:%M"), rep.text(),
                                                   "PASS" if code == 0 else "FAIL", extra), encoding="utf-8")
        print("Saved %s" % (logs / latest))
    return code


if __name__ == "__main__":
    sys.exit(main())
