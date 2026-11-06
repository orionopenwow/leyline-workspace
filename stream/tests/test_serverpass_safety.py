"""realm/serverpass.sh's way back (fix round 4 Nov, realm review 1): a failed restart puts the previous scripts and
profile back, step 2 writes its exact undo SQL before it changes the dungeon levels, and the Lua error check reads the
log from the world server's start, not only the last 60 s.

    python3 -I stream/tests/test_serverpass_safety.py
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SP = os.path.join(ROOT, "realm", "serverpass.sh")
src = open(SP, encoding="utf-8").read()
ok = bad = 0


def check(name, cond):
    global ok, bad
    print(("PASS " if cond else "FAIL ") + name)
    ok, bad = ok + bool(cond), bad + (not cond)


check("bash syntax", subprocess.run(["bash", "-n", SP]).returncode == 0)
body = src[src.index("put_back() {"):src.index("\n}\n", src.index("put_back() {"))]
check("put_back restores the backed-up scripts and the profile, and moves this pass's new scripts aside",
      '"$BK"/*.lua' in body and 'cp -p "$BK/profile.env" "$PF"' in body and "for b in $ADDED" in body)
check("brand-new scripts are recorded in ADDED", 'ADDED="$ADDED $b"' in src)
for what in ("did not report ready", "did not restart"):
    line = next((l for l in src.splitlines() if what in l and "die" in l), "")
    check(f"restart '{what}': put_back runs before the step fails", "put_back;" in line)
i, j = src.index("UNDO-dungeon-levels.sql"), src.index("UPDATE dungeon_access_template SET min_level = 10")
check("step 2: the undo SQL is written before the update", i < j)
check("Lua errors are read from the server's start after a restart",
      re.search(r'docker logs --since "\$LOG_SINCE"', src) is not None and "State.StartedAt" in src)
print(f"\n{ok}/{ok + bad} passed")
sys.exit(1 if bad else 0)
