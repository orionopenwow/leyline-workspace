#!/usr/bin/env bash
# Turn HTTP/3 (QUIC over UDP) off in the live Caddy, so browsers use HTTP/2 over TCP.
# Why: on 4 Nov 2026 every download from the server crawled at ~20-40 KB/s over HTTP/3 while the same PC
# downloaded at 4-24 MB/s elsewhere. QUIC through Docker's UDP port mapping is a known slow path.
#   bash h3-off.sh           back up the Caddyfile, add "servers { protocols h1 h2 }", validate, reload, self-check;
#                            anything wrong puts the old file back
#   bash h3-off.sh --undo    put back the newest backup this script made (HTTP/3 on again)
set -euo pipefail
CF="${WU_CADDYFILE:-/opt/openvanilla-classicplus/Caddyfile}"
CADDY_DIR="${WU_CADDY_DIR:-/opt/wenilla-realm}"
BKROOT=/root/caddy-backups
caddy() { (cd "$CADDY_DIR" && docker compose exec -T caddy caddy "$@"); }
reload() { caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile; }
selfcheck() {
  local u code
  for u in https://leylinelauncher.com/ https://play.leylinelauncher.com/wu-diag.js; do
    code=$(curl -s -L -m 20 -o /dev/null -w '%{http_code}' "$u" || true)
    echo "  $u -> $code"
    case "$code" in 2??|401|403) ;; *) return 1 ;; esac
  done
}
[ -f "$CF" ] || { echo "No Caddyfile at $CF. Nothing changed."; exit 1; }

if [ "${1:-}" = "--undo" ]; then
  last=$(ls -1d "$BKROOT"/*-h3off 2>/dev/null | tail -1 || true)
  [ -n "$last" ] || { echo "No h3-off backup to put back."; exit 1; }
  cat "$last/Caddyfile" > "$CF"; reload
  echo "Put back $last/Caddyfile (HTTP/3 on again)."; echo "H3 UNDO OK"; exit 0
fi

if grep -qE '^\s*protocols\b' "$CF"; then
  echo "The Caddyfile already sets protocols:"; grep -nE '^\s*protocols\b' "$CF"
  echo "Nothing changed."; echo "H3 OFF OK"; exit 0
fi

BK="$BKROOT/$(date +%Y%m%d-%H%M%S)-h3off"; mkdir -p "$BK"; cp -p "$CF" "$BK/Caddyfile"
TMP=$(mktemp); trap 'rm -f "$TMP"' EXIT
python3 -I - "$CF" "$TMP" <<'PY'
import sys
src = open(sys.argv[1], encoding="utf-8").read()
lines = src.split("\n")
ins = ["\t# HTTP/2 over TCP only: HTTP/3 through Docker's UDP mapping crawled (tools/security/h3-off.sh)",
       "\tservers {", "\t\tprotocols h1 h2", "\t}"]
# The global options block is a bare "{" before any site block (comments and blank lines may come first).
i = 0
while i < len(lines) and (not lines[i].strip() or lines[i].lstrip().startswith("#")):
    i += 1
if i < len(lines) and lines[i].strip() == "{":
    lines[i + 1:i + 1] = ins
else:
    lines[i:i] = ["{"] + ins + ["}", ""]
open(sys.argv[2], "w", encoding="utf-8").write("\n".join(lines))
PY
diff -u "$CF" "$TMP" || true
cat "$TMP" > "$CF"   # in place: the file is bind-mounted into the caddy container
if ! caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1 || ! reload; then
  cat "$BK/Caddyfile" > "$CF"; reload || true
  echo "Caddy refused it; the old Caddyfile is back ($BK/Caddyfile)."; exit 1
fi
sleep 3
if ! selfcheck; then
  cat "$BK/Caddyfile" > "$CF"; reload || true
  echo "The site did not answer after the change; the old Caddyfile is back."; exit 1
fi
alt=$(curl -s -m 15 -D - -o /dev/null https://leylinelauncher.com/ | grep -i '^alt-svc' || true)
echo "  alt-svc now: ${alt:-(none: browsers stop trying HTTP/3)}"
echo "HTTP/3 is off. Backup: $BK/Caddyfile. Undo: the h3-on step (bash h3-off.sh --undo)."
echo "H3 OFF OK"
