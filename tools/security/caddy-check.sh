#!/usr/bin/env bash
# Read-only: prints the LIVE Caddyfile and the compose files (secrets redacted by redact.py) plus what is
# listening, so they can be reviewed on the PC ("Caddy check.bat" saves this output to tools\security\live\).
# The live Caddyfile (/opt/openvanilla-classicplus/Caddyfile) is not in git. Changes nothing. Safe to run any time.
set -uo pipefail
cd "$(dirname "$0")" || exit 1
CF="${WU_CADDYFILE:-/opt/openvanilla-classicplus/Caddyfile}"
CADDY_DIR="${WU_CADDY_DIR:-/opt/wenilla-realm}"
ROOTS="${WU_COMPOSE_ROOTS:-/opt /srv/wrath}"
# Speed (read-only): why the server sends slowly. Every network test is bounded (-m) and sends nothing private.
speed_section() {
  local CADDY WP WPIP f t1 t2 r1 r2 dev
  echo "## Speed"
  echo "### CPU (steal = the host giving our VM less CPU than it pays for)"
  head -1 /proc/loadavg; top -bn1 2>/dev/null | sed -n '3p'
  docker stats --no-stream --format '{{.Name}}\t{{.CPUPerc}}\t{{.NetIO}}' 2>/dev/null | grep -E 'caddy|wu-play|wu-portal' || true
  echo "### Kernel network settings"
  sysctl -n net.ipv4.tcp_congestion_control net.core.default_qdisc net.core.rmem_max net.core.wmem_max 2>/dev/null | paste -sd' ' -
  dev=$(ip route show default 2>/dev/null | awk '{print $5; exit}')
  echo "default device: $dev"; ip -o link show "$dev" 2>/dev/null | grep -o 'mtu [0-9]*'
  tc -s qdisc show dev "$dev" 2>/dev/null | head -8
  echo "### Sending right now (5 s)"
  r1=$(awk -v d="$dev:" '$1==d{print $10}' /proc/net/dev); t1=$(date +%s.%N); sleep 5
  r2=$(awk -v d="$dev:" '$1==d{print $10}' /proc/net/dev); t2=$(date +%s.%N)
  awk -v a="$r1" -v b="$r2" -v s="$t1" -v e="$t2" 'BEGIN{printf "tx %.0f KB/s\n", (b-a)/1024/(e-s)}'
  command -v vnstat >/dev/null && vnstat -m 2>/dev/null | tail -6 || echo "(vnstat not installed: no monthly total)"
  echo "### Web connections (TCP 443: rtt, cwnd, retransmits, delivery rate)"
  ss -tin state established '( sport = :443 )' 2>/dev/null | grep -oE 'rtt:[^ ]+|cwnd:[0-9]+|retrans:[^ ]+|delivery_rate [^ ]+|bytes_acked:[0-9]+' | paste -sd' ' - | fold -w 220 | head -12
  echo "UDP 443 sockets: $(ss -uan '( sport = :443 )' 2>/dev/null | tail -n +2 | wc -l)"
  echo "### Caddy: HTTP/3 and buffer warnings (last 6 h)"
  CADDY=$(docker ps --format '{{.Names}}' | grep -m1 caddy)
  docker logs --since 6h "$CADDY" 2>&1 | grep -iE 'buffer size|quic|http/3|h3' | cut -c1-220 | sort | uniq -c | sort -rn | head -6
  grep -nE 'protocols|servers' "$CF" 2>/dev/null || echo "(no protocols line: Caddy serves h1, h2 and h3)"
  echo "### Local speed (no network: Caddy and wu-play on this box)"
  WP=$(docker ps --format '{{.Names}}' | grep -m1 '^wu-play$')
  WPIP=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' "$WP" 2>/dev/null | awk '{print $1}')
  [ -n "$WPIP" ] && curl -s -m 30 -o /dev/null -w "wu-play direct: %{http_code} %{size_download} bytes at %{speed_download} B/s\n" "http://$WPIP:8090/wenilla_bg.wasm" || echo "(wu-play address not found)"
  for v in --http1.1 --http2; do
    curl -s -m 30 $v -o /dev/null --resolve play.leylinelauncher.com:443:127.0.0.1 \
      -w "via Caddy $v: %{http_code} %{http_version} %{size_download} bytes at %{speed_download} B/s\n" \
      https://play.leylinelauncher.com/art/leyline-swirl-256.webp || true
  done
  curl -s -m 20 -D - -o /dev/null --resolve play.leylinelauncher.com:443:127.0.0.1 -H 'Accept-Encoding: zstd, br, gzip' \
    https://play.leylinelauncher.com/wenilla_bg.wasm 2>/dev/null | grep -iE '^(HTTP|content-encoding|content-length|alt-svc|cache-control)' || true
  echo "### Out to the internet (this box -> Cloudflare's speed test, 10 MB of zeros)"
  f=$(mktemp); head -c 10000000 /dev/zero > "$f"
  curl -s -m 60 -o /dev/null -w "upload: %{speed_upload} B/s in %{time_total} s\n" --data-binary @"$f" https://speed.cloudflare.com/__up || echo "(upload test failed)"
  curl -s -m 60 -o /dev/null -w "download: %{speed_download} B/s\n" "https://speed.cloudflare.com/__down?bytes=10000000" || echo "(download test failed)"
  rm -f "$f"
}

echo "# caddy-check $(date -u +%Y-%m-%dT%H:%M:%SZ) on $(hostname)"
echo "# secrets are replaced by <redacted>; .env files are listed by name only, never printed"
echo
echo "## Caddy version"
(cd "$CADDY_DIR" 2>/dev/null && docker compose exec -T caddy caddy version 2>/dev/null) || echo "(caddy container not reachable from $CADDY_DIR)"
echo
echo "## The live Caddyfile"
python3 -I redact.py "$CF"
echo
echo "## Compose files"
mapfile -t files < <(find $ROOTS -maxdepth 3 \( -name 'docker-compose*.yml' -o -name 'docker-compose*.yaml' -o -name 'compose*.yml' -o -name 'compose*.yaml' \) -type f 2>/dev/null | sort)
if [ "${#files[@]}" -gt 0 ]; then python3 -I redact.py "${files[@]}"; else echo "(none under $ROOTS)"; fi
echo
echo "## .env files (names only)"
find $ROOTS -maxdepth 3 -type f \( -name '*.env' -o -name '.env' \) 2>/dev/null | sort
echo
echo "## Published ports (docker)"
docker ps --format '{{.Names}}\t{{.Image}}\t{{.Ports}}' 2>/dev/null || echo "(docker not reachable)"
echo
echo "## Listening sockets"
ss -ltnup 2>/dev/null | awk '{print $1, $5, $7}' | sort -u || true
echo
echo "## Firewall (DOCKER-USER chain, ufw)"
iptables -S DOCKER-USER 2>/dev/null || echo "(no DOCKER-USER chain)"
ufw status 2>/dev/null | head -30 || true
echo
echo "## fail2ban jails"
fail2ban-client status 2>/dev/null || echo "(fail2ban not running)"
echo
speed_section || echo "(speed section failed)"
echo
echo "CADDY CHECK OK"
