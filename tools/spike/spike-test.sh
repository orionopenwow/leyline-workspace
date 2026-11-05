#!/usr/bin/env bash
# Spike test, on the server (tools/spike/spike_test.py): about a minute of moderate load on the live services, then
# what each one took and what the box looked like meanwhile. Writes nothing, uses no account.
set -uo pipefail
cd "$(dirname "$0")" || exit 1
ip_of() { docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' "$1" 2>/dev/null | awk '{print $1}'; }
PORTAL=$(ip_of wu-portal); PLAY=$(ip_of wu-play)
[ -n "$PORTAL" ] && [ -n "$PLAY" ] || { echo "wu-portal or wu-play not found"; exit 1; }
SITE="${WU_SPIKE_SITE:-play.leylinelauncher.com}"
echo "== before"; head -1 /proc/loadavg; free -m | sed -n 2p
docker stats --no-stream --format '{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}' 2>/dev/null | grep -E 'caddy|wu-play|wu-portal|worldserver' || true
( sleep 20; echo "== during (part A/B)"; head -1 /proc/loadavg
  docker stats --no-stream --format '{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}' 2>/dev/null | grep -E 'caddy|wu-play|wu-portal|worldserver' || true ) &
python3 -I spike_test.py --portal "http://$PORTAL:8091" --play "http://$PLAY:8090" --site "$SITE" \
  --workers "${WU_SPIKE_WORKERS:-40}" --seconds "${WU_SPIKE_SECONDS:-15}"
rc=$?
wait
echo "== after"; head -1 /proc/loadavg
echo "== the website program's busy answers (503) in the last 5 min"
docker logs --since 5m wu-portal 2>&1 | grep -ci "busy" || true
exit $rc
