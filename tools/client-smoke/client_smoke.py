"""Client smoke test: the real desktop game, started the way the launcher's PLAY starts it on the official realm, with a
screenshot at every phase (owner 2 Nov 23:33: "do a robust smoke test with screenshots each phase of the way ... you
should have access to my client for testing and debug, create that ability if you can't").

    python -I tools/client-smoke/client_smoke.py [--hold 75] [--every 3] [--world]

What it does, on the owner's PC (Control step client-smoke):
  1. before: the game folder the launcher plays with, its realmlist and Config.wtf account line (never a password),
     Leyline's login screen archive (Data/patch-Z.MPQ) and whether a sign-in is left in it;
  2. `Leyline.exe play-test --official --hold <s>` (launcher/src-tauri/src/playtest.rs, app.rs play_test_cli): the
     launcher's own sign-in asks the website for this player's game login, hands it to the login screen for one start
     and starts the game, with no launcher window;
  3. while the game runs: a screenshot of its window every few seconds (logs/client-smoke/NN-<s>s.png), and the archive
     checked (the sign-in is in it, for this one start);
  4. after: the game closed by the launcher, the sign-in taken back out (checked), the launcher's report
     (play-test.json, never a password) and a summary (logs/client-smoke/summary.txt) with PASS / FAIL per step.

It changes nothing else: the launcher's own settings and sign-in are used as they are. Windows only (the screenshots).
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
sys.path.insert(0, str(ROOT / "tools" / "den-check"))
import den_check as dc  # noqa: E402  (its helpers: where things are, the archive, the window screenshot)

OUT = ROOT / "logs" / "client-smoke"
REPORT = "play-test.json"
CLI_MARK = b"leyline-play-test-v1"


def plain(t):
    """One line for the log: no byte-order mark or other invisible characters, at most 300 characters."""
    return "".join(c for c in str(t) if c.isprintable() and c != "\ufeff")[:300]


class Steps:
    def __init__(self):
        self.rows = []

    def add(self, ok, what, detail=""):
        kind = "PASS" if ok is True else ("FAIL" if ok is False else "INFO")
        detail = plain(detail)
        self.rows.append((kind, what, detail))
        print("%-4s %s%s" % (kind, what, (": " + detail) if detail else ""), flush=True)
        return ok

    def text(self):
        return "\n".join("%-4s %s%s" % (k, w, (": " + d) if d else "") for k, w, d in self.rows) + "\n"

    def failed(self):
        return any(k == "FAIL" for k, _, _ in self.rows)


def config_account(game):
    """The account name in Config.wtf (the name only; Config.wtf never holds a password)."""
    _, vals = dc.read_config(Path(game))
    return next((v for k, v in vals.items() if k.lower() == "accountname"), "")


def realmlist(game):
    data = dc.find_ci(Path(game), "Data")
    places = [data / l for l in dc.locales(data)] + [data] if data else []
    for d in places + [Path(game)]:
        p = dc.find_ci(d, "realmlist.wtf")
        if p and p.is_file():
            return plain(p.read_text(encoding="utf-8-sig", errors="replace").strip())[:200]
    return ""


def archive_state(game):
    data = dc.find_ci(Path(game), "Data")
    p = dc.find_ci(data, "patch-Z.MPQ") if data else None
    if not p:
        return {"there": False}
    ours, left, toc, names = dc.our_archive(p)
    return {"there": True, "ours": ours, "session_left": left, "toc_has_ours": toc, "files": len(names),
            "glue_files": sorted(n for n in names if n.lower().startswith("interface\\gluexml\\"))[:12]}

# The game's message boxes (a dialog window, class #32770, of the game's process) and their text: "Your login interface
# files are corrupt" is a FAIL the screenshots alone didn't flag (3 Nov 00:15).
DIALOG_PS = r"""
Add-Type @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public class LD {
  public delegate bool EP(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EP f, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EP f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  public static List<string> Find(uint want) {
    var o = new List<string>();
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p != want || !IsWindowVisible(h)) return true;
      var c = new StringBuilder(64); GetClassName(h, c, 64); if (c.ToString() != "#32770") return true;
      var t = new StringBuilder(512); EnumChildWindows(h, (k, m) => { var cc = new StringBuilder(64); GetClassName(k, cc, 64);
        if (cc.ToString() == "Static") { var s = new StringBuilder(512); GetWindowText(k, s, 512); if (s.Length > 0) t.Append(s.ToString()).Append(' '); }
        return true; }, IntPtr.Zero);
      o.Add(t.ToString().Trim()); return true; }, IntPtr.Zero);
    return o; } }
'@
[LD]::Find(__PID__) | ForEach-Object { "DIALOG:" + $_ }
"""


def game_dialogs(pid):
    """The text of each message box the game's process shows now ([] off Windows or when there is none)."""
    if os.name != "nt" or not pid:
        return []
    try:
        r = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", DIALOG_PS.replace("__PID__", str(int(pid)))],
                           capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return []
    return [plain(l[7:]) for l in r.stdout.splitlines() if l.startswith("DIALOG:")]



def read_report(app):
    try:
        return json.loads((app / REPORT).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--hold", type=int, default=75, help="seconds the game stays open (screenshots meanwhile)")
    ap.add_argument("--every", type=float, default=3.0, help="seconds between screenshots")
    ap.add_argument("--world", action="store_true", help="enter the world with the last character, not only character select")
    a = ap.parse_args(argv)
    for f in (sys.stdout, sys.stderr):   # the Windows console's code page can't print everything: never a crash over it
        try:
            f.reconfigure(errors="replace")
        except (AttributeError, ValueError):
            pass
    st = Steps()
    print("CLIENT SMOKE TEST (%s)" % time.strftime("%Y-%m-%d %H:%M"), flush=True)
    if OUT.exists():
        shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True, exist_ok=True)

    # 1. before
    app = dc.app_dir()
    s = dc.settings(app)
    st.add(s is not None, "the launcher's settings", str(app / "settings.json"))
    game = dc.den_game(s)
    if not st.add(game is not None, "a 3.3.5a game folder in the launcher", game and ("%s (%s)" % (game.get("name"), game.get("path"))) or "add one in the launcher"):
        return finish(st, None, None)
    gpath = game["path"]
    st.add(None, "realmlist before", realmlist(gpath) or "(none)")
    st.add(None, "Config.wtf account before", config_account(gpath) or "(none)")
    before = archive_state(gpath)
    st.add(None, "Leyline's login screen archive before", json.dumps(before))
    if before.get("there") and before.get("ours"):
        st.add(not before.get("session_left"), "no sign-in left in it from an earlier start")
    exe = dc.launcher_exe()
    if not st.add(exe is not None, "the launcher program", str(exe) if exe else "run Open Leyline Launcher.bat once"):
        return finish(st, None, None)
    try:
        has_cli = CLI_MARK in exe.read_bytes()
    except OSError:
        has_cli = False
    if not st.add(has_cli, "this launcher has play-test", "" if has_cli else "the newest launcher isn't in .launcher yet: ship, wait for its build, run again"):
        return finish(st, None, None)

    # 2. PLAY on the official realm, no launcher window
    try:
        (app / REPORT).unlink()
    except OSError:
        pass
    cmd = [str(exe), "play-test", "--official", "--hold", str(max(10, min(a.hold, 600)))] + (["--world"] if a.world else [])
    t0 = time.time()
    proc = subprocess.Popen(cmd, cwd=str(exe.parent))
    st.add(None, "started", " ".join(cmd[1:]))

    # 3. while it runs: the report's phases, the game window every few seconds
    shots, pid, seen_phase, seen_session, dialogs = 0, 0, "", False, []
    deadline = t0 + max(10, min(a.hold, 600)) + 90
    next_shot = 0.0
    while time.time() < deadline:
        r = read_report(app) or {}
        ph = r.get("phase", "")
        if ph and ph != seen_phase:
            seen_phase = ph
            st.add(None, "launcher phase", "%s at %.1fs" % (ph, time.time() - t0))
        if r.get("pid") and not pid:
            pid = int(r["pid"])
            st.add(True, "the game started", "pid %d at %.1fs" % (pid, time.time() - t0))
        if pid and ph == "running" and time.time() >= next_shot:
            name = OUT / ("%02d-%03ds.png" % (shots, int(time.time() - t0)))
            ok, detail = dc.screenshot(pid, name)
            if ok:
                shots += 1
            elif shots == 0:
                st.add(None, "screenshot", detail)
            if not seen_session:
                seen_session = bool(archive_state(gpath).get("session_left"))
            for d in game_dialogs(pid):
                if d not in dialogs:
                    dialogs.append(d)
                    st.add(None, "a message box from the game", "%s (at %.0fs)" % (d or "(no text)", time.time() - t0))
            next_shot = time.time() + a.every
        if proc.poll() is not None and ph == "done":
            break
        time.sleep(0.25)
    try:
        code = proc.wait(timeout=60)
    except subprocess.TimeoutExpired:
        proc.kill()
        code = -1

    # 4. after
    r = read_report(app) or {}
    return finish(st, r, {"code": code, "shots": shots, "pid": pid, "gpath": gpath, "seen_session": seen_session, "t": time.time() - t0, "dialogs": dialogs})


def finish(st, r, run):
    if r is not None and run is not None:
        wl = r.get("website_login") or {}
        st.add(bool(r.get("signed_in")), "the launcher is signed in", "" if r.get("signed_in") else "Log In in the launcher first")
        st.add(bool(wl.get("ok")), "the website gave this player's game login", wl.get("account") or wl.get("why", ""))
        glue = r.get("glue") or {}
        st.add(None, "login screen archive status", json.dumps(glue))
        hy = r.get("hybrid") or {}
        st.add(bool(hy.get("ok")), "the sign-in handed to the login screen (it skips the login by itself)", hy.get("why", ""))
        st.add(run["seen_session"] or not hy.get("ok"), "the sign-in was in the archive while the game started")
        st.add(run["shots"] > 0, "screenshots of the game", "%d in %s" % (run["shots"], OUT))
        st.add(not run["dialogs"], "no message box from the game (a refused login screen, a crash)", "; ".join(run["dialogs"]))
        st.add(bool(r.get("scrubbed")) and not r.get("session_left"), "the sign-in taken back out after", "left in the archive!" if r.get("session_left") else "")
        after = archive_state(run["gpath"])
        st.add(not after.get("session_left"), "nothing of it on disk now", json.dumps(after))
        st.add(None, "realmlist after", realmlist(run["gpath"]))
        st.add(None, "Config.wtf account after", config_account(run["gpath"]))
        if r.get("error"):
            st.add(False, "the launcher stopped", r["error"])
        st.add(not r.get("closed_itself"), "the game stayed open until the launcher closed it", "it closed by itself: see the last screenshots" if r.get("closed_itself") else "")
        st.add(run["code"] == 0, "play-test's exit code", str(run["code"]))
        safe = {k: v for k, v in r.items() if k not in ("pass", "password")}
        (OUT / "play-test.json").write_text(json.dumps(safe, indent=2), encoding="utf-8")
    verdict = "FAIL" if st.failed() else "PASS"
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "summary.txt").write_text("CLIENT SMOKE TEST (%s)\n%s\n%s\n" % (time.strftime("%Y-%m-%d %H:%M"), st.text(), verdict), encoding="utf-8")
    print(verdict, flush=True)
    print("Saved %s" % (OUT / "summary.txt"), flush=True)
    return 0 if verdict == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
