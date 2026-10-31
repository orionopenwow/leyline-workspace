#!/usr/bin/env bash
# Read-only: why did the world server restart? Its container state and restart count, the last 40 minutes of its
# output (crash lines, Lua errors, the restart notice), and the newest crash dump names. Changes nothing.
set -u
C=ov-ac-worldserver
echo "== container"
docker inspect -f 'started {{.State.StartedAt}}  restarts {{.RestartCount}}  status {{.State.Status}}  exit {{.State.ExitCode}}  oom {{.State.OOMKilled}}' "$C" 2>&1
echo "== memory now"; free -m | head -2
echo "== events (last 60 min)"
docker events --since 60m --until 0s --filter container=$C --format '{{.Time}} {{.Action}}' 2>/dev/null | tail -20
echo "== crash dumps"
ls -lt /srv/wrath/logs/world 2>/dev/null | grep -i -E 'crash|dump|\.txt' | head -5
echo "== world server output (last 40 min, crash/error lines first)"
docker logs --since 40m "$C" 2>&1 | grep -i -E 'crash|segfault|signal|assert|fatal|error|lua|shutdown|restart' | grep -v -i 'password' | tail -60
echo "== the 80 lines before the newest start (what it was doing when it stopped)"
docker logs --since 90m "$C" 2>&1 | grep -v -i 'password' > /tmp/wu-wl.txt
n=$(grep -n "Can't set process priority" /tmp/wu-wl.txt | tail -1 | cut -d: -f1)
[ -n "$n" ] && [ "$n" -gt 1 ] && head -n $((n-1)) /tmp/wu-wl.txt | grep -v 'exec_' | tail -80
rm -f /tmp/wu-wl.txt
echo "== crash files inside the container"
docker exec "$C" sh -c 'ls -lt /azerothcore/env/dist/bin/Crashes /azerothcore/env/dist/logs/Crashes 2>/dev/null | head -5; ls -lt /azerothcore/env/dist/bin 2>/dev/null | grep -i -E "crash|core" | head -5' 2>&1
echo "== last 40 lines"
docker logs --tail 40 "$C" 2>&1 | grep -v -i 'password'
echo "WORLDLOG OK"
