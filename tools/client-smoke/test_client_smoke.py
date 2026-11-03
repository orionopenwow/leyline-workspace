"""client_smoke.py with a fake launcher (a script that writes play-test.json as play_test_cli does) and a fake game folder.

    python3 tools/client-smoke/test_client_smoke.py
"""
import json
import os
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
FAILS = []


def check(what, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + what + ("" if ok else "  " + str(detail)[:400]))
    if not ok:
        FAILS.append(what)


FAKE = r'''#!/usr/bin/env python3
# leyline-play-test-v1
import json, os, sys, time
app = os.environ["LEYLINE_APP_DIR"]
rep = {"mark": "leyline-play-test-v1", "phase": "login", "signed_in": True}
def save():
    open(os.path.join(app, "play-test.json"), "w").write(json.dumps(rep))
save(); time.sleep(0.3)
mode = os.environ.get("FAKE_MODE", "ok")
if mode == "signed_out":
    rep.update(signed_in=False, website_login={"ok": False, "why": "signed_out"}, hybrid={"ok": False, "why": "no sign-in"})
else:
    rep.update(website_login={"ok": True, "account": "WR000123"}, hybrid={"ok": True}, glue={"foreverui": False, "slot_ok": True})
rep.update(phase="running", pid=os.getpid()); save(); time.sleep(1.2)
rep.update(phase="done", scrubbed=True, session_left=False, closed_itself=False); save()
sys.exit(0 if mode == "ok" else 1)
'''


def run(mode):
    d = Path(tempfile.mkdtemp())
    app, game = d / "app", d / "game"
    (game / "Data" / "enUS").mkdir(parents=True)
    (game / "WTF").mkdir()
    (game / "WTF" / "Config.wtf").write_text('SET accountName "WR000123"\r\nSET gxWindow "1"\r\n')
    (game / "Data" / "enUS" / "realmlist.wtf").write_text("set realmlist leylinelauncher.com\r\n")
    app.mkdir()
    (app / "settings.json").write_text(json.dumps({"clients": [{"id": "c1", "name": "Wrath 3.3.5a", "path": str(game), "build": "3.3.5a"}],
                                                   "selected_client": "c1"}))
    exe = d / "Leyline.exe"
    exe.write_text(FAKE)
    exe.chmod(exe.stat().st_mode | stat.S_IEXEC)
    env = dict(os.environ, LEYLINE_APP_DIR=str(app), LEYLINE_EXE=str(exe), FAKE_MODE=mode)
    r = subprocess.run([sys.executable, "-I", str(HERE / "client_smoke.py"), "--hold", "10", "--every", "0.5"], env=env,
                       capture_output=True, text=True, timeout=200)
    summary = (ROOT / "logs" / "client-smoke" / "summary.txt").read_text()
    return r, summary


r, summary = run("ok")
out = r.stdout
check("ok: the launcher's phases are followed (login, running, done)", "phase: login" in out and "phase: running" in out and "phase: done" in out, out)
check("ok: the game started is seen by its pid", "PASS the game started" in out, out)
check("ok: the website login, the hand-over and the clean-up pass", "PASS the website gave this player's game login: WR000123" in out
      and "PASS the sign-in handed to the login screen" in out and "PASS the sign-in taken back out after" in out, out)
check("ok: the before and after state is written (account name, realmlist; never a password)", "Config.wtf account before: WR000123" in out
      and "realmlist before: set realmlist leylinelauncher.com" in out and "password" not in summary.lower().replace("never a password", ""), out)
check("ok: the summary file", summary.startswith("CLIENT SMOKE TEST") and "play-test's exit code: 0" in summary, summary)
check("no screenshots off Windows: said as a FAIL, not a crash", "FAIL screenshots of the game" in out and r.returncode == 1, (r.returncode, r.stderr[-400:]))

r, summary = run("signed_out")
check("signed out: said plainly (Log In first), FAIL", "FAIL the launcher is signed in: Log In in the launcher first" in r.stdout and r.returncode == 1, r.stdout)

print("%d/7 checks passed" % (7 - len(FAILS)))
sys.exit(1 if FAILS else 0)
