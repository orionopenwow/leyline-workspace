#!/usr/bin/env python3
"""The reel kit's capture side, on the owner's Windows PC (Control steps reel-record and reel-record-hand): scripted
takes of the real launcher, the real game and the website, recorded with ffmpeg's screen grabber (gdigrab) into
tools/reels/takes/<take>.mp4 with <take>.json beside it (its length and its named moments, "marks", for the edit).

    py -3 tools/reels/record.py [take ...]          the automatic takes (game, launcher-fake, live site)
    py -3 tools/reels/record.py --hand [take ...]   the hand takes: a box says what to do, then the screen is recorded
    py -3 tools/reels/record.py --list              what each take is and whether it is recorded yet

What each source does (scripts/takes.json):
  game           the launcher's own test starts, as the real buttons do, with nothing typed by hand:
                   official-world  `Leyline.exe play-test --official --world` (tools/client-smoke): PLAY on Wrath
                                   Unleashed, logged in by itself, character select, into the world
                   den-open        `Leyline.exe den-play --test` (tools/den-check): Den Lite and the game, in the Den
                   den-carry       then, in the Den, the Leyline UI's own test actions through Den Lite's command files
                                   (DenBar.lua / Gizmo.lua TestAction): build mode, a gallery piece in hand following
                                   the cursor, placed, picked up and carried, set down, turned
                   den-fly         build view, free flight (W held), snap out, zoom far out and back
                 The game window alone is recorded. Your Den is saved first and put back after; the piece is removed.
  launcher-fake  takes.cjs (node + Playwright, if this PC has them; the cloud makes these too)
  site / phone   offline ones come from takes.cjs; a live one opens Edge on leylinelauncher.com in its own window
                 (a fresh profile, nothing logged in), scrolled with the keyboard, the window recorded
  hand           a box says what to do and how long; OK starts the recording (the whole screen, or the game or
                 launcher window), it stops by itself, a second box says it's done

Marks: the moments the driver knows (it pressed something, the launcher said "in the world") are measured; the ones it
can't see (character select) are takes.json "guess" seconds, and the log says which. The edit's contact sheet shows
where they fell; the scripts' offsets are easy to nudge.

Never types a password or a Hero Key, never makes an account, never uploads or posts anything. Needs ffmpeg (the .bat
gets it with winget if it is missing) and Python 3; Windows for everything but --list.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
TAKES = HERE / "takes"
SCRIPTS = HERE / "scripts"
FPS = 30
SITE = "https://leylinelauncher.com"
sys.path.insert(0, str(ROOT / "tools" / "den-check"))


def catalog():
    return json.loads((SCRIPTS / "takes.json").read_text(encoding="utf-8"))["takes"]


def log(*a):
    print(*a, flush=True)


# ---------------------------------------------------------------- ffmpeg gdigrab
def ffmpeg():
    """ffmpeg on PATH, or where winget put it (edit.tool)."""
    sys.path.insert(0, str(HERE))
    import edit  # noqa: PLC0415
    return edit.tool("ffmpeg")


def even_rect(rect):
    """(x, y, w, h) with an even width and height (H.264 wants them even), never smaller than 2 x 2."""
    x, y, w, h = (int(v) for v in rect)
    return x, y, max(2, w - w % 2), max(2, h - h % 2)


def gdigrab_cmd(ff, out, rect=None, fps=FPS, mouse=True):
    """The ffmpeg command recording the desktop, or one rectangle of it (a window's), to out (H.264, quick to encode:
    the edit encodes again)."""
    cmd = [ff, "-y", "-hide_banner", "-loglevel", "error", "-f", "gdigrab", "-framerate", str(fps), "-draw_mouse", "1" if mouse else "0"]
    if rect:
        x, y, w, h = even_rect(rect)
        cmd += ["-offset_x", str(x), "-offset_y", str(y), "-video_size", "%dx%d" % (w, h)]
    cmd += ["-i", "desktop", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "18", "-pix_fmt", "yuv420p", str(out)]
    return cmd


class Recorder:
    """One take being recorded: start() begins, mark() notes a moment, stop() ends it ('q' to ffmpeg, as a person
    would) and writes <take>.json. popen and clock are the test's to replace."""

    def __init__(self, take, out_dir=TAKES, popen=subprocess.Popen, clock=time.monotonic, ff=None):
        self.take, self.dir = take, Path(out_dir)
        self.popen, self.clock = popen, clock
        self.ff = ff or ffmpeg()
        self.proc, self.t0, self.marks, self.guessed = None, None, {}, []
        self.mp4 = self.dir / (take + ".part.mp4")

    def start(self, rect=None, source=""):
        if not self.ff:
            raise RuntimeError("ffmpeg isn't installed (winget install Gyan.FFmpeg)")
        self.dir.mkdir(parents=True, exist_ok=True)
        self.source, self.rect = source, rect
        self.proc = self.popen(gdigrab_cmd(self.ff, self.mp4, rect), stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        self.t0 = self.clock()
        return self

    def now(self):
        return 0.0 if self.t0 is None else self.clock() - self.t0

    def mark(self, label, at=None):
        self.marks[label] = round(self.now() if at is None else at, 3)
        log("     mark %-12s %6.2fs" % (label, self.marks[label]))

    def guess(self, label, at):
        """A moment the driver can't see: takes.json's guess, noted as such."""
        self.marks[label] = float(at)
        self.guessed.append(label)

    def stop(self, ok=True):
        secs = self.now()
        if self.proc is not None:
            try:
                self.proc.stdin.write(b"q")
                self.proc.stdin.flush()
                self.proc.stdin.close()
            except (OSError, ValueError):
                pass
            try:
                self.proc.wait(timeout=30)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        final = self.dir / (self.take + ".mp4")
        if not ok:   # never a half take in a reel
            for p in (self.mp4,):
                try:
                    p.unlink()
                except OSError:
                    pass
            return None
        if self.mp4.is_file():
            os.replace(self.mp4, final)
        meta = {"seconds": round(secs, 3), "marks": self.marks, "guessed": self.guessed, "source": self.source,
                "rect": list(even_rect(self.rect)) if self.rect else None, "recorded": time.strftime("%Y-%m-%d %H:%M")}
        (self.dir / (self.take + ".json")).write_text(json.dumps(meta, indent=1), encoding="utf-8")
        return meta


# ---------------------------------------------------------------- windows (Windows only)
def dpi_aware():
    """Rectangles in real pixels, as gdigrab sees them (a scaled display would otherwise cut the window short)."""
    if os.name != "nt":
        return
    import ctypes
    try:
        ctypes.windll.shcore.SetProcessDpiAwareness(2)
    except (AttributeError, OSError):
        try:
            ctypes.windll.user32.SetProcessDPIAware()
        except (AttributeError, OSError):
            pass


def window_rect(hwnd):
    """A window's rectangle on screen without its invisible resize border (DWM's frame bounds), as (x, y, w, h)."""
    import ctypes
    from ctypes import wintypes
    r = wintypes.RECT()
    try:
        ctypes.windll.dwmapi.DwmGetWindowAttribute(wintypes.HWND(hwnd), 9, ctypes.byref(r), ctypes.sizeof(r))
    except (AttributeError, OSError):
        ctypes.windll.user32.GetWindowRect(wintypes.HWND(hwnd), ctypes.byref(r))
    if r.right - r.left < 50:
        ctypes.windll.user32.GetWindowRect(wintypes.HWND(hwnd), ctypes.byref(r))
    return clamp_rect((r.left, r.top, r.right - r.left, r.bottom - r.top), screen_size())


def screen_size():
    import ctypes
    u = ctypes.windll.user32
    return u.GetSystemMetrics(0), u.GetSystemMetrics(1)


def clamp_rect(rect, screen):
    """A rectangle kept on the (main) screen: gdigrab fails outright on one that reaches past it."""
    x, y, w, h = rect
    sw, sh = screen
    x, y = max(0, x), max(0, y)
    w, h = min(w, sw - x), min(h, sh - y)
    return even_rect((x, y, w, h))


def front(hwnd):
    import ctypes
    u = ctypes.windll.user32
    u.ShowWindow(hwnd, 9)   # SW_RESTORE
    u.SetForegroundWindow(hwnd)


def windows_of(match):
    """Visible top-level windows whose (title, class, pid) match(...) says yes: [(hwnd, title, class, pid)]."""
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    out = []
    proto = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def each(h, _lp):
        if not u.IsWindowVisible(h):
            return True
        p = wintypes.DWORD()
        u.GetWindowThreadProcessId(h, ctypes.byref(p))
        t = ctypes.create_unicode_buffer(256)
        u.GetWindowTextW(h, t, 256)
        c = ctypes.create_unicode_buffer(128)
        u.GetClassNameW(h, c, 128)
        if match(t.value, c.value, p.value):
            out.append((h, t.value, c.value, p.value))
        return True
    u.EnumWindows(proto(each), 0)
    return out


def wait_window(find, seconds):
    end = time.time() + seconds
    while time.time() < end:
        h = find()
        if h:
            return h
        time.sleep(0.25)
    return None


def message(text, title="Leyline reels", ok_cancel=False):
    """A box on screen (from a Control step too: it has no console). True for OK."""
    import ctypes
    flags = 0x40 | 0x10000 | 0x1000 | (0x1 if ok_cancel else 0)   # information, foreground, system modal
    return ctypes.windll.user32.MessageBoxW(None, text, title, flags) == 1


# ---------------------------------------------------------------- the game takes
class Game:
    """The launcher's own test starts (den_check's and client_smoke's), recorded."""

    def __init__(self):
        import den_check as dc
        self.dc = dc
        self.app = dc.app_dir()
        self.exe = dc.launcher_exe()
        s = dc.settings(self.app)
        self.client = dc.den_game(s)

    def ready(self):
        if self.exe is None or not self.exe.is_file():
            return "no launcher in .launcher (run Open Leyline Launcher.bat once)"
        if self.client is None:
            return "no 3.3.5a game folder in the launcher (add one first)"
        return ""

    def game_rect(self, pid, seconds=90):
        h = wait_window(lambda: self.dc.game_window(pid), seconds)
        if not h:
            return None, None
        time.sleep(0.5)
        front(h)
        return h, window_rect(h)

    # official-world: PLAY on Wrath Unleashed into the world (client_smoke's start)
    def official_world(self, rec, take):
        hold = int(take.get("seconds", 60)) + 15
        report = self.app / "play-test.json"
        try:
            report.unlink()
        except OSError:
            pass
        proc = subprocess.Popen([str(self.exe), "play-test", "--official", "--world", "--hold", str(hold)], cwd=str(self.exe.parent))
        pid = 0
        end = time.time() + 90
        while time.time() < end and not pid:
            r = self.dc.read_json(report) or {}
            pid = int(r.get("pid") or 0)
            if proc.poll() is not None:
                break
            time.sleep(0.25)
        if not pid:
            proc.kill()
            raise RuntimeError("the launcher didn't start the game (play-test.json: %s)" % (self.dc.read_json(report) or {}).get("error", "no report"))
        _h, rect = self.game_rect(pid)
        if not rect:
            raise RuntimeError("the game's window didn't show")
        rec.start(rect, "game")
        rec.mark("started")
        for label, at in (take.get("guess") or {}).items():
            rec.guess(label, at)
        time.sleep(max(5, float(take.get("seconds", 60))))
        rec.stop()
        try:
            proc.wait(timeout=hold + 60)
        except subprocess.TimeoutExpired:
            proc.kill()
        return True

    # the Den: den-play --test (den_check test's start), then what the take does in it
    def den(self, rec, take, inside):
        dc = self.dc
        root = self.app / "den-lite"
        den_dir = root / "state" / "dens"
        for p in (root / "den-click.json", root / "den-click.shot"):
            try:
                p.unlink()
            except OSError:
                pass
        start = int(time.time() * 1000)
        proc = subprocess.Popen([str(self.exe), "den-play", "--test", "--client", self.client["id"], "--hold", "300"],
                                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        pid, rpt = 0, None
        end = time.time() + 150
        recording = False
        while time.time() < end:
            rpt = dc.read_json(root / "den-click.json")
            if rpt and rpt.get("start", 0) >= start - 5000:
                pid = int(rpt.get("pid") or 0)
                if pid and not recording and inside is None:
                    # den-open records from the game's first window to the world
                    _h, rect = self.game_rect(pid, 60)
                    if rect:
                        rec.start(rect, "game")
                        rec.mark("started")
                        recording = True
                if rpt.get("phase") in ("in_world", "stopped", "done"):
                    break
            if proc.poll() is not None:
                break
            time.sleep(0.4)
        try:
            if not rpt or rpt.get("phase") != "in_world":
                raise RuntimeError("the Den didn't reach the world (%s)" % ((rpt or {}).get("outcome") or "no report"))
            for _ in range(80):   # the Leyline UI's hello (den_check test waits for the same)
                if "builder_ready" in {e["ev"] for e in dc.events(den_dir, rpt["start"])}:
                    break
                time.sleep(0.25)
            if inside is None:
                rec.mark("world")
                time.sleep(4)
                rec.stop()
            else:
                _h, rect = self.game_rect(pid, 10)
                ch = dc.FileChannel(den_dir)
                kept = dc.stash([den_dir / "reel-before.den.json", den_dir / "backup.den.json"])
                ch.send(".den save reel-before")
                try:
                    rec.start(rect, "game")
                    time.sleep(1.0)
                    inside(rec, ch)
                    time.sleep(1.0)
                    rec.stop()
                finally:
                    ch.send(".den ui gizmo build off")
                    ch.send(".den load reel-before replace")
                    dc.unstash(kept)
        except Exception:
            if rec.t0 is not None and rec.proc is not None:
                rec.stop(ok=False)
            raise
        finally:
            try:
                (root / "den-click.shot").write_text("taken")   # the launcher's test closes the game and Den Lite
            except OSError:
                pass
            try:
                proc.wait(timeout=330)
            except subprocess.TimeoutExpired:
                proc.kill()
        return True


def den_carry(rec, ch, pause=0.9):
    """In the Den (Gizmo.lua TestAction): build mode, a gallery piece in hand that follows the cursor, placed, picked
    up and carried somewhere else, set down, turned. The points are den_check's (aids_features)."""
    ui = lambda a: ch.send(".den ui " + a)   # noqa: E731
    ui("gizmo build on")
    rec.mark("build")
    time.sleep(pause * 2)
    ui("gizmo arm 2413")
    rec.mark("place")
    for u, v in path_points((0.35, -0.45), (0.25, -0.35), 8):
        ui("gizmo hover %.3f %.3f" % (u, v))
        time.sleep(0.12)
    time.sleep(pause)
    ui("gizmo commit")
    time.sleep(pause * 1.5)
    ui("gizmo carry 0.25 -0.35")
    rec.mark("carry")
    for u, v in path_points((0.25, -0.35), (0.05, -0.28), 14):
        ui("gizmo hover %.3f %.3f" % (u, v))
        time.sleep(0.12)
    time.sleep(pause * 0.6)
    ui("gizmo commit")
    rec.mark("drop")
    time.sleep(pause * 1.5)
    rec.mark("turn")
    for _ in range(3):
        ui("gizmo turn 1")
        time.sleep(pause * 0.7)
    time.sleep(pause)


def den_fly(rec, ch, pause=0.9):
    """In the Den: the build view, free flight with W held, snap out to the whole Den, zoom far out and back."""
    ui = lambda a: ch.send(".den ui " + a)   # noqa: E731
    ui("gizmo build on")
    ui("gizmo cam build")
    rec.mark("build")
    time.sleep(pause * 2)
    ui("gizmo fly W down")
    rec.mark("fly")
    time.sleep(3.0)
    ui("gizmo fly W up")
    time.sleep(pause)
    ui("gizmo snap")
    rec.mark("snap")
    time.sleep(pause * 3)
    ui("gizmo zoom 40")
    rec.mark("zoomout")
    time.sleep(pause * 3)
    ui("gizmo zoom -60")
    time.sleep(pause * 2)
    ui("gizmo snap")
    rec.mark("back")
    time.sleep(pause * 2)


def path_points(a, b, n):
    """n eased points from a to b (the carried piece glides, it doesn't jump)."""
    out = []
    for i in range(1, n + 1):
        t = i / n
        e = t * t * (3 - 2 * t)
        out.append((a[0] + (b[0] - a[0]) * e, a[1] + (b[1] - a[1]) * e))
    return out


# ---------------------------------------------------------------- a live site take (Edge, its own window)
def edge():
    for p in (os.path.expandvars(r"%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe"),
              os.path.expandvars(r"%ProgramFiles%\Microsoft\Edge\Application\msedge.exe")):
        if os.path.isfile(p):
            return p
    return shutil.which("msedge")


def edge_cmd(exe, url, profile, size):
    """Edge as an app window (no tabs, no address bar) on a fresh profile: nothing logged in, nothing of the owner's."""
    w, h = size
    cmd = [exe, "--app=" + url, "--user-data-dir=" + str(profile), "--no-first-run", "--no-default-browser-check",
           "--window-position=40,40", "--window-size=%d,%d" % (w, h), "--disable-features=Translate"]
    return cmd


def live_site(rec, take):
    import tempfile
    exe = edge()
    if not exe:
        raise RuntimeError("Edge wasn't found")
    mobile = take.get("source") == "phone"
    size = (430, 900) if mobile else (1280, 760)
    prof = Path(tempfile.mkdtemp(prefix="leyline-reel-edge-"))
    proc = subprocess.Popen(edge_cmd(exe, SITE + take.get("url", "/wrath"), prof, size))
    try:
        h = wait_window(lambda: next((w[0] for w in windows_of(lambda t, c, p: c.startswith("Chrome_WidgetWin") and "Leyline" in t)), None), 30)
        if not h:
            raise RuntimeError("Edge's window with the site didn't show")
        time.sleep(2.5)
        front(h)
        rec.start(window_rect(h), take.get("source", "site"))
        rec.mark("top")
        time.sleep(2)
        for _ in range(6):   # a slow scroll: the arrow key, as a person would
            press_key(h, 0x28, 6)
            time.sleep(0.6)
        rec.mark("scrolled")
        time.sleep(max(1.0, float(take.get("seconds", 12)) - rec.now()))
        rec.stop()
    finally:
        proc.terminate()
        time.sleep(1)
        shutil.rmtree(prof, ignore_errors=True)
    return True


def press_key(hwnd, vk, times=1):
    import ctypes
    u = ctypes.windll.user32
    for _ in range(times):
        u.PostMessageW(hwnd, 0x0100, vk, 0)   # WM_KEYDOWN
        u.PostMessageW(hwnd, 0x0101, vk, 0)   # WM_KEYUP
        time.sleep(0.04)


# ---------------------------------------------------------------- hand takes
def hand_text(take_id, take):
    secs = int(take.get("seconds", 30))
    where = {"game": "the game's window", "launcher": "the launcher's window"}.get(take.get("window"), "the whole screen")
    return ("Take %s (%d seconds, %s)\n\n%s\n\nGet it ready, then press OK: the recording starts at once and stops by "
            "itself after %d seconds. Cancel skips this take.") % (take_id, secs, where, take.get("prompt", ""), secs)


def hand(rec, take_id, take):
    if not message(hand_text(take_id, take), ok_cancel=True):
        log("     skipped")
        return False
    rect = None
    if take.get("window") == "game":
        h = next((w[0] for w in windows_of(lambda t, c, p: c.startswith("GxWindowClass"))), None)
        rect = window_rect(h) if h else None
    elif take.get("window") == "launcher":
        h = next((w[0] for w in windows_of(lambda t, c, p: t.strip() == "Leyline" or t.startswith("Leyline "))), None)
        rect = window_rect(h) if h else None
    rec.start(rect, "hand")
    time.sleep(float(take.get("seconds", 30)))
    rec.stop()
    message("Take %s is recorded. Run reel-make to cut the reels." % take_id)
    return True


# ---------------------------------------------------------------- the plan
def chosen(cat, names, hand_mode):
    """The takes this run records: the named ones, or every one of this kind (hand, or automatic: game and live site;
    launcher-fake and offline site/phone takes are takes.cjs's)."""
    out = []
    for tid, t in cat.items():
        if names and tid not in names:
            continue
        src = t.get("source")
        if hand_mode != (src == "hand"):
            continue
        if src == "launcher-fake" or (src in ("site", "phone") and t.get("offline")):
            continue
        out.append(tid)
    return out


def takes_cjs(names):
    """The launcher and offline site takes with takes.cjs, when this PC has node and Playwright."""
    node = shutil.which("node")
    if not node:
        log("INFO launcher takes: node isn't installed here (the cloud makes them: tools/reels/takes.cjs)")
        return None
    r = subprocess.run([node, "-e", "require('playwright')"], cwd=str(ROOT), capture_output=True)
    if r.returncode != 0:
        log("INFO launcher takes: Playwright isn't installed here (npm i -g playwright, then npx playwright install chromium)")
        return None
    return subprocess.run([node, str(HERE / "takes.cjs"), "--out", str(TAKES)] + list(names), cwd=str(ROOT)).returncode == 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("takes", nargs="*")
    ap.add_argument("--hand", action="store_true", help="the hand takes (a box says what to do)")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--no-launcher", action="store_true", help="skip takes.cjs (the launcher-fake takes)")
    a = ap.parse_args(argv)
    for f in (sys.stdout, sys.stderr):
        try:
            f.reconfigure(errors="replace")
        except (AttributeError, ValueError):
            pass
    cat = catalog()
    unknown = [t for t in a.takes if t not in cat]
    if unknown:
        log("No take called: %s (scripts/takes.json has %s)" % (", ".join(unknown), ", ".join(sorted(cat))))
        return 2
    if a.list:
        for tid, t in cat.items():
            got = (TAKES / (tid + ".mp4")).is_file() or (TAKES / tid / "frames.json").is_file()
            log("%-22s %-13s %-9s %s" % (tid, t["source"], "recorded" if got else "-", t.get("what", "")))
        return 0
    if os.name != "nt":
        log("Recording needs the owner's Windows PC (gdigrab). Here: --list, and takes.cjs for the launcher takes.")
        return 1
    dpi_aware()
    if not ffmpeg():
        log("FAIL ffmpeg isn't installed: winget install Gyan.FFmpeg (Reel record.bat does it for you)")
        return 1
    log("REEL RECORD (%s)%s" % (time.strftime("%Y-%m-%d %H:%M"), " - hand takes" if a.hand else ""))
    fails = 0
    if not a.hand and not a.no_launcher:
        lf = [t for t in (a.takes or cat) if cat[t]["source"] == "launcher-fake" or (cat[t]["source"] in ("site", "phone") and cat[t].get("offline"))]
        if lf:
            ok = takes_cjs(lf if a.takes else [])
            if ok is False:
                fails += 1
                log("FAIL launcher takes (takes.cjs): see above")
    game = None
    for tid in chosen(cat, a.takes, a.hand):
        t = cat[tid]
        log("TAKE %s (%s): %s" % (tid, t["source"], t.get("what", "")))
        rec = Recorder(tid)
        try:
            if t["source"] == "hand":
                ok = hand(rec, tid, t)
            elif t["source"] == "game":
                game = game or Game()
                why = game.ready()
                if why:
                    raise RuntimeError(why)
                flow = t.get("flow")
                if flow == "official-world":
                    ok = game.official_world(rec, t)
                elif flow == "den-open":
                    ok = game.den(rec, t, None)
                elif flow == "den-carry":
                    ok = game.den(rec, t, den_carry)
                elif flow == "den-fly":
                    ok = game.den(rec, t, den_fly)
                else:
                    raise RuntimeError("no flow called %s" % flow)
            elif t["source"] in ("site", "phone"):
                ok = live_site(rec, t)
            else:
                raise RuntimeError("this driver doesn't record %s takes" % t["source"])
            if ok:
                note = (" (guessed: %s)" % ", ".join(rec.guessed)) if rec.guessed else ""
                log("PASS %s: %.1fs%s" % (tid, rec.now(), note))
        except Exception as e:  # noqa: BLE001 - one take failing doesn't stop the others
            fails += 1
            if rec.proc is not None:
                rec.stop(ok=False)
            log("FAIL %s: %s" % (tid, str(e)[:300]))
    log("REEL RECORD %s: takes in %s; next: reel-make" % ("OK" if not fails else "FAILED (%d)" % fails, TAKES))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
