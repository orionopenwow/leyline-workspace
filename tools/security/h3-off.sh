#!/usr/bin/env bash
# Turn HTTP/3 (QUIC over UDP) off in the live Caddy, so browsers use HTTP/2 over TCP.
# Why: on 4 Nov 2026 every download from the server crawled at ~20-40 KB/s over HTTP/3 while the same PC
# downloaded at 4-24 MB/s elsewhere. QUIC through Docker's UDP port mapping is a known slow path.
#   bash h3-off.sh           back up the Caddyfile, add "servers { protocols h1 h2 }", validate, reload, self-check;
#                            anything wrong puts the old file back
#   bash h3-off.sh --undo    take out the lines it added (HTTP/3 on again): validate, reload, self-check
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
  # review 4 Nov: take out only the lines this script added. Putting the whole old file back would also undo every
  # Caddyfile change made since (the sign-in redirects, the streaming block...).
  grep -q "tools/security/h3-off.sh" "$CF" || { echo "The Caddyfile has no h3-off lines: HTTP/3 is already on. Nothing changed."; echo "H3 UNDO OK"; exit 0; }
  BK="$BKROOT/$(date +%Y%m%d-%H%M%S)-h3undo"; mkdir -p "$BK"; cp -p "$CF" "$BK/Caddyfile"
  TMP=$(mktemp); trap 'rm -f "$TMP"' EXIT
  python3 -I - "$CF" "$TMP" <<'PY' || { echo "The h3-off lines were changed by hand; nothing changed (put them back yourself, or restore a backup in $BKROOT)."; exit 1; }
import sys
lines = open(sys.argv[1], encoding="utf-8").read().split("\n")
i = next(n for n, l in enumerate(lines) if "tools/security/h3-off.sh" in l)
want = ["servers {", "protocols h1 h2", "}"]
if [l.strip() for l in lines[i + 1:i + 4]] != want:
    sys.exit(1)
del lines[i:i + 4]
# the global block this script made ("{" then "}" with nothing left between): gone too
j = i - 1
if 0 <= j and lines[j].strip() == "{" and i < len(lines) and lines[i].strip() == "}":
    del lines[j:i + 1]
    if j < len(lines) and not lines[j].strip():
        del lines[j]
open(sys.argv[2], "w", encoding="utf-8").write("\n".join(lines))
PY
  diff -u "$CF" "$TMP" || true
  cat "$TMP" > "$CF"
  if ! caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1 || ! reload; then
    cat "$BK/Caddyfile" > "$CF"; reload || true
    echo "Caddy refused it; the Caddyfile is as it was ($BK/Caddyfile)."; exit 1
  fi
  sleep "${WU_H3_WAIT:-3}"
  if ! selfcheck; then
    cat "$BK/Caddyfile" > "$CF"; reload || true
    echo "The site did not answer after the change; the Caddyfile is as it was."; exit 1
  fi
  echo "HTTP/3 is on again. Backup: $BK/Caddyfile."; echo "H3 UNDO OK"; exit 0
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
sleep "${WU_H3_WAIT:-3}"
if ! selfcheck; then
  cat "$BK/Caddyfile" > "$CF"; reload || true
  echo "The site did not answer after the change; the old Caddyfile is back."; exit 1
fi
alt=$(curl -s -m 15 -D - -o /dev/null https://leylinelauncher.com/ | grep -i '^alt-svc' || true)
echo "  alt-svc now: ${alt:-(none: browsers stop trying HTTP/3)}"
echo "HTTP/3 is off. Backup: $BK/Caddyfile. Undo: the h3-on step (bash h3-off.sh --undo)."
echo "H3 OFF OK"
