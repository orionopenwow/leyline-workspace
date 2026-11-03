#!/usr/bin/env bash
# The bugs manifest, rebuilt on the server and summed up (run as root by "Pull bug manifest.bat", the Control step
# "bugs"). It asks the GM console (wu-gmconsole, 127.0.0.1:8099) to rebuild $WU/bugs/backlog.json and
# BUGS-MANIFEST.md from every source (gmconsole/server/bugs_manifest.py); the .bat then copies the two files to the PC.
# The GM token is read here and sent only in a header from a file descriptor: never printed, never in argv.
# Last line: "BUGS OK: <n> open, <m> in all" or "BUGS FAILED: <why>".
set -uo pipefail
WU="${WU_DIR:-/srv/wrath}"
PORT="${GM_PORT:-8099}"
TOK=$(sed -n 's/^GM_TOKEN=//p' "$WU/gmconsole.env" 2>/dev/null | tail -1)
[ -n "$TOK" ] || { echo "BUGS FAILED: no GM console token in $WU/gmconsole.env (install the GM console: the gm step)"; exit 1; }
if [ -f "$WU/bugs/status.json.new" ]; then
  if python3 -I -c 'import json,sys; d=json.load(open(sys.argv[1])); assert isinstance(d, dict)' "$WU/bugs/status.json.new" 2>/dev/null; then
    mv -f "$WU/bugs/status.json.new" "$WU/bugs/status.json"; chmod 600 "$WU/bugs/status.json"
    echo "statuses from the repo uploaded"
  else
    rm -f "$WU/bugs/status.json.new"; echo "tools/bugs/status.json is not a JSON object: skipped"
  fi
fi
out=$(curl -fsS -m 180 -H @<(printf 'Authorization: Bearer %s\n' "$TOK") \
  "http://127.0.0.1:$PORT/api/export/bugs-manifest?build=1") \
  || { echo "BUGS FAILED: the GM console did not answer (is wu-gmconsole running? the gm step installs it)"; exit 1; }
unset TOK
printf '%s' "$out" | python3 -I -c '
import json, sys
c = json.load(sys.stdin)["counts"]
s = c["status"]
print("by status: " + ", ".join("%s %d" % kv for kv in s.items()))
print("by source: " + ", ".join("%s %d" % kv for kv in c["source"].items()))
print("BUGS OK: %d open, %d in all" % (c["open"], c["total"]))' || { echo "BUGS FAILED: the GM console answered something else"; exit 1; }
