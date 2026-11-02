#!/usr/bin/env bash
# Server fix (overnight 2 Nov, owner OK for fixes): the two streamed-file folders no longer writable by everyone,
# and a read-only look at why the in-game .bug command didn't answer the release audit. No restart, no secrets.
set -uo pipefail
cd /srv/wrath || { echo "no /srv/wrath"; exit 1; }
echo "== 1. folder rights"
for d in /srv/wrath/stream /srv/wrath/stream/slices; do
  [ -d "$d" ] && chmod go-w "$d" && echo "  $d: $(stat -c %a "$d")"
done
echo "== 2. .bug (read only)"
echo "  ov_reports.lua on the server: $(ls -l custom/lua_scripts/ov_reports.lua 2>/dev/null | awk '{print $5" bytes "$6" "$7" "$8}')"
for k in AC_ALLOW_PLAYER_COMMANDS OV_REPORTS AC_ELUNA_ENABLED AC_ALE_ENABLED; do
  echo "  $k=$(docker exec ov-ac-worldserver printenv "$k" 2>/dev/null || echo '(not set)')"
done
grep -E '^(AC_ALLOW_PLAYER_COMMANDS|OV_REPORTS)=' profiles/main.env .env 2>/dev/null | sed 's/^/  file: /'
echo "  world server log, script lines (last 24 h):"
docker logs --since 24h ov-ac-worldserver 2>&1 | grep -iE 'ov_reports|lua.*error|eluna|ale\b|reload' | tail -n 25 | sed 's/^/    /'
echo "SERVER FIX DONE"
