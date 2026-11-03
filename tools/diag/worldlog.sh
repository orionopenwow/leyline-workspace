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
echo "== Lua (ALE) lines, last 3 h (2 Nov: the in-game .bug didn't answer the audit)"
docker logs --since 3h "$C" 2>&1 | grep -i -E 'lua|ale\b|ov_reports|\.lua:' | grep -v -i -E 'password|Missing property ALE' | tail -40
echo "  since the world server started (the load lines: errors, and which scripts ALE loaded):"
docker logs "$C" 2>&1 | grep -i -E 'lua|ale\b|eluna|\.lua' | grep -v -i -E 'password|Missing property ALE' | grep -i -E 'error|fail|loaded|ov_reports|unable|attempt|expected|near' | head -40
echo "  where ALE reads scripts (env, mounts, and what each lua_scripts folder holds):"
docker exec "$C" sh -c 'env | grep -i -E "^AC_ALE|^AC_ELUNA" | sed "s/=.*PASS.*/=(hidden)/"' 2>&1 | sed 's/^/    /'
docker inspect -f '{{range .Mounts}}{{.Source}} -> {{.Destination}}{{"\n"}}{{end}}' "$C" 2>/dev/null | grep -i lua | sed 's/^/    mount: /'
docker exec "$C" sh -c 'for d in /azerothcore/lua_scripts /azerothcore/env/dist/bin/lua_scripts /azerothcore/env/dist/lua_scripts; do [ -d "$d" ] && echo "    $d: $(ls "$d" | tr "\n" " ")"; done' 2>&1
echo "  Lua errors in the world server's own log files (ALE may write only there):"
docker exec "$C" sh -c 'ls /azerothcore/env/dist/logs 2>/dev/null | head -20 | tr "\n" " "; echo; grep -h -i -E "\.lua:[0-9]+|lua.*error|ov_reports" /azerothcore/env/dist/logs/*.log 2>/dev/null | grep -v -i password | tail -25' 2>&1 | sed 's/^/    /'
grep -h -i -E "\.lua:[0-9]+|ov_reports" /srv/wrath/logs/world/*.log 2>/dev/null | tail -15 | sed 's/^/    host: /'
echo "  ov_reports.lua in the container: $(docker exec "$C" sh -c 'ls -l /azerothcore/env/dist/bin/lua_scripts/ov_reports.lua /azerothcore/lua_scripts/ov_reports.lua 2>/dev/null' 2>&1 | head -2)"
echo "== out-of-memory kills, last 24 h (names only)"
(journalctl -k --since "-24h" 2>/dev/null || dmesg 2>/dev/null) | grep -iE 'killed process|memory cgroup out of memory' | sed -E 's/.*(Killed process [0-9]+ \([^)]*\)).*/\1/; s/.*(Memory cgroup out of memory).*/\1/' | sort | uniq -c | tail -10
echo "== last 40 lines"
docker logs --tail 40 "$C" 2>&1 | grep -v -i 'password'
echo "WORLDLOG OK"
