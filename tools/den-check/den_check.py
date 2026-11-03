#!/usr/bin/env python3
"""den-check and den-test: is My Den one click into the world on this PC's own game?

    py -3 tools/den-check/den_check.py check [--game <folder>]     read-only: the game folder, Leyline's login
                                                                    archive, Forever UI, Config.wtf, Den Lite and a
                                                                    sign-in through the exact steps (a Den Lite of
                                                                    its own on spare ports, a scratch folder)
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
of the character (its speed, falling, control) is shown too.

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
INVITE_RE = re.compile(r"^https://([a-z0-9.-]{3,80})/join/([a-z2-7]{16})#([A-Z2-9]{24})~~(G[A-Z0-9]{3,15})\.([A-Z0-9]{16})$")
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
# game's own grass by name (launcher terrain crate pad.rs PAD_TEXTURE), the area's own sky, no table of the game's
GRASS = "Tileset\\Elwynn\\ElwynnGrassBase.blp"


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
    """Of patch-T's file names, what isn't the tile or our mark: the game's tables (the old white sky and zone name)
    and textures (the old white ground). Since 3 Nov there should be none."""
    out = []
    for n in names:
        low = n.lower()
        if low.startswith("dbfilesclient\\") or low.endswith(".blp"):
            out.append(n)
    return out


def ground_lines(rep, detail, patch_t=None):
    """The Den's world, from the launcher's report (`ground_detail`, host/terrain.rs GroundStatus) and our patch-T:
    the ground painted the game's own grass (only its name in the tile), nothing of the island on the tile, nothing
    of the game's tables or textures in patch-T (the area's own sky and zone name). Only the picture shows what the
    game drew."""
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
    if patch_t is not None:
        try:
            extra = patch_extras(mpqlite.Archive(str(patch_t)).names())
            err = None
        except Exception as e:  # noqa: BLE001 - said below
            extra, err = [], str(e)[:120]
        if err:
            rep.add("FAIL", "sky/area: the game's own", "Data\\patch-T.MPQ couldn't be read: %s" % err, "Start My Den once more; still not: send this log.")
        else:
            rep.ok(not extra, "sky/area: the game's own",
                   "the area's normal sky and zone name: no table or texture of the game's in Data\\patch-T.MPQ" if not extra else
                   "Data\\patch-T.MPQ still has %s" % ", ".join(extra[:4]),
                   "An older launcher made it: update the launcher and start My Den once more.")
    else:
        rep.add("INFO", "sky/area", "sky: %s; zone: %s" % (detail.get("sky", "?"), detail.get("area", "?")))
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
        try:
            keys_check(steps, den_dir, pid, rpt["start"])
        except Exception as e:  # noqa: BLE001 - the smoke test still runs
            steps.add("INFO", "keys: pressed (SendInput)", "the check stopped: %s" % str(e)[:200])
        try:
            secrets = den_steps(steps, FileChannel(den_dir), den_dir, root, marks, rpt["start"])
        except Exception as e:  # noqa: BLE001 - the game still gets its picture and is closed
            steps.add("FAIL", "the Den's smoke test", "stopped: %s" % str(e)[:200], "Send logs\\den-test-latest.txt.")
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
        ground_lines(rep, rpt.get("ground_detail"), find_ci(data, "patch-T.MPQ") if data else None)
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


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", choices=["check", "test"])
    ap.add_argument("--game")
    ap.add_argument("--app-dir")
    ap.add_argument("--no-login", action="store_true")
    ap.add_argument("--invite", action="store_true", help="kept for the den-test-invite step: the invite is always tested now")
    ap.add_argument("--no-update", action="store_true", help="test: use the Den Lite on this PC as it is")
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
