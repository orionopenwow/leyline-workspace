#!/usr/bin/env bash
# Remove the old 1.12 game files from the server (owner 2026-10-31: "remove old game files, keep new slice").
#   bash data-remove.sh          dry run: says what it would do, changes nothing
#   bash data-remove.sh --go     do it
# 1. The old play site stops serving them: the live Caddyfile gets `handle /data/* { respond 410 }` in the
#    play.openvanilla.app block (before the catch-all that proxies to the old 1.12 site), validated and reloaded
#    with `docker exec` into the running Caddy (never `docker compose up` in /opt/wenilla-realm). A Caddy that
#    refuses it gets the old file back. Checked: /data/__index answers 410, /wrath/status.json still 200.
# 2. The files go (contents only; the folders and volumes stay, empty, so no container definition changes):
#      /srv/client/Data                         the 1.12 client archives the old play site served
#      /opt/openvanilla-duels/client-data       the old Duels copy of the same
#      volume wenilla-realm_gamedata            the old 1.12 server's extracted maps (vr-/wenilla-realm mangosd)
#      volume owduels_gamedata                  the old Duels server's extracted maps
#    Every container that mounts one of them must be stopped (exited) except the old site's web front, which
#    only reads /client/Data and gets the 410 in front of it first.
# Never touched: /srv/wrath (the Wrath stream and realm), the ov-client-data volume (the Wrath server's own data),
# databases, accounts, Caddy itself, the Azeroth Survivors slice (it does not exist yet; it will live elsewhere).
set -u
GO=""; [ "${1:-}" = "--go" ] && GO=1
CF=/opt/openvanilla-classicplus/Caddyfile
CADDY=wenilla-realm-caddy-1
SITE=play.openvanilla.app
die() { echo "OV112 DATA REMOVE FAILED: $*"; exit 1; }
vol() { docker volume inspect -f '{{.Mountpoint}}' "$1" 2>/dev/null; }
TARGETS=(/srv/client/Data /opt/openvanilla-duels/client-data)
for v in wenilla-realm_gamedata owduels_gamedata; do mp=$(vol "$v"); [ -n "$mp" ] && TARGETS+=("$mp"); done

echo "== what goes"
for t in "${TARGETS[@]}"; do
  case "$t" in /srv/wrath*|*ov-client-data*|*ac-database*|/|/srv|/opt|/var|/var/lib/docker/volumes) die "refusing $t";; esac
  [ -d "$t" ] && printf '  %-7s %s\n' "$(du -sh "$t" 2>/dev/null | cut -f1)" "$t" || echo "  (gone)  $t"
done

echo "== containers that mount them (must be stopped, except the old site's web front behind the 410)"
for c in $(docker ps --format '{{.Names}}'); do
  m=$(docker inspect -f '{{range .Mounts}}{{.Source}} {{end}}' "$c")
  for t in "${TARGETS[@]}"; do
    case " $m" in *" $t "*|*" $t/"*)
      if [ "$c" = wenilla-realm-realm-1 ] && [ "$t" = /srv/client/Data ]; then echo "  $c reads $t (web front: 410 in front of it)"
      else die "$c is RUNNING and mounts $t: stop it first"; fi;;
    esac
  done
done
echo "  ok"

echo "== Caddy: /data/* answers 410 on $SITE"
TMP=$(mktemp); trap 'rm -f "$TMP"' EXIT
python3 - "$CF" "$TMP" "$SITE" <<'PY' || die "could not edit the Caddyfile"
import sys,re
src,dst,site=sys.argv[1:4]; t=open(src).read()
mark="# Owner 2026-10-31: the old 1.12 game files are gone"
if mark in t: open(dst,'w').write(t); sys.exit(0)
i=t.find(site)
if i<0: sys.exit("no "+site+" block")
j=t.find("\thandle {", i)   # the catch-all to the old 1.12 site
if j<0: sys.exit("no catch-all handle in the "+site+" block")
add=("\t"+mark+" (tools/ov112-data/data-remove.sh): nothing is served from /data here.\n"
     "\thandle /data/* {\n\t\theader Cache-Control no-store\n\t\trespond \"Gone\" 410\n\t}\n\n")
open(dst,'w').write(t[:j]+add+t[j:])
PY
if cmp -s "$CF" "$TMP"; then echo "  already in place"; else diff -u "$CF" "$TMP" | sed -n '1,30p'; fi

if [ -z "$GO" ]; then echo; echo "Dry run: nothing changed. Run with --go to do it."; echo "OV112 DATA REMOVE DRY"; exit 0; fi

BK=/root/ov112-data-removed-$(date +%Y%m%d-%H%M%S); mkdir -p "$BK"; cp -p "$CF" "$BK/Caddyfile"
if ! cmp -s "$CF" "$TMP"; then
  cat "$TMP" > "$CF"
  if ! docker exec "$CADDY" caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1 || \
     ! docker exec "$CADDY" caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1; then
    cat "$BK/Caddyfile" > "$CF"; docker exec "$CADDY" caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1 || true
    die "Caddy refused the change; the old Caddyfile is back"
  fi
  sleep 2
fi
d=$(curl -s -m 15 -o /dev/null -w '%{http_code}' "https://$SITE/data/__index")
w=$(curl -s -m 15 -o /dev/null -w '%{http_code}' "https://$SITE/wrath/status.json")
echo "  https://$SITE/data/__index -> $d (want 410); /wrath/status.json -> $w (want 200)"
if [ "$w" != 200 ]; then cat "$BK/Caddyfile" > "$CF"; docker exec "$CADDY" caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1 || true; die "the Wrath site stopped answering; the old Caddyfile is back"; fi
[ "$d" = 410 ] || die "/data still answers $d; files kept"

echo "== removing the files"
for t in "${TARGETS[@]}"; do
  [ -d "$t" ] || continue
  find "$t" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
  echo "  emptied $t"
done
echo "== disk"; df -h / | tail -1
echo "Backup of the Caddyfile: $BK/Caddyfile"
echo "OV112 DATA REMOVE OK"
