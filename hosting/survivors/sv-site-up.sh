#!/usr/bin/env bash
# Azeroth Survivors at its own address (docs/SURVIVORS.md): the unmodified upstream browser build and the slice of
# game files the owner exported from his own client on his PC, served by sv-play behind the Leyline sign-in.
# Run as root by "Put Survivors on Leyline.bat" (one SSH login; this folder, gamedata-caddy.py, survivors_check.py, the
# lock and the upload arrive as a tarball over stdin) and its "- check" / "- off" twins.
#
#   bash sv-site-up.sh <source dir> --check   read-only: what it would do, the address (DNS), what is ready
#   bash sv-site-up.sh <source dir> --go      do it: check the upload, install it, start sv-play, the Caddy site, the
#                                             self-check, and only then the Play button (rolls back on a failure)
#   bash sv-site-up.sh <source dir> --off     Play off, the Caddy site out, sv-play stopped (the files are kept)
#
# <source dir>/payload/ (optional: without it --go re-serves what is installed):
#   www/            the browser build, file for file as stream/survivors/upstream.lock.json pins it, + licenses/
#   slice/          the game files (Record Survivors files.bat on the owner's PC), as names.sha256 lists them
#   names.sha256    "<sha256>  <name>" per file
# Installed to $SV_ROOT (/srv/survivors): www/, slice/ (the last ones kept as *.prev), names.sha256, docker-compose.yml,
# site-host. The address: SV_SITE_HOST (the bat sends hosting/survivors/address.txt), else the one it was put up at.
# The game data killswitch (stream/server/gamedata-switch.sh) covers this site: when it is on, the site goes up with
# its "resting" block (gamedata-caddy.py add-sv) and the page on the site says Resting.
#
# Last line: "SV SITE ON" / "SV SITE DRY" / "SV SITE OFF", or "SV SITE FAILED: ..." (exit 1). Prints no secret.
set -uo pipefail
SRC="${1:?usage: sv-site-up.sh <source dir> --check|--go|--off}"
MODE="${2:---check}"
SV_ROOT="${SV_ROOT:-/srv/survivors}"
WU="${WU_DIR:-/srv/wrath}"
CF="${SV_CADDYFILE:-/opt/openvanilla-classicplus/Caddyfile}"
CADDY="${SV_CADDY:-wenilla-realm-caddy-1}"
NET="${SV_NET:-wenilla-realm_default}"
PORTAL_DIR="${WU_PORTAL_DIR:-$WU/portal}"
GD_FLAG="${WU_GAMEDATA_FLAG_HOST:-$PORTAL_DIR/gamedata-off}"
HOST_BIN="${SV_HOST_BIN:-$WU/stream/bin/wenilla-host}"
PUBLIC="${SV_PUBLIC_HOST:-play.openvanilla.app}"
HOST="${SV_SITE_HOST:-$(tr -cd 'a-z0-9.-' < "$SV_ROOT/site-host" 2>/dev/null)}"
HOST=$(printf '%s' "$HOST" | tr 'A-Z' 'a-z' | tr -d ' \r\n')
SITE=sv-play
SITE_BEGIN="# >>> Azeroth Survivors site (sv-site-up.sh)"
SITE_END="# <<< Azeroth Survivors site"
BRAND_RE='world *of *warcraft|(^|[^a-z])wow([^a-z]|$)|warcraft|lich *king|blizzard'
TS=$(date -u +%Y%m%d-%H%M%S)
BK="${SV_BACKUPS:-/root/sv-site-backups}/$TS"
log() { printf '\n== %s\n' "$*"; }
note() { printf '  %s\n' "$*"; }
die() { printf '\nSV SITE FAILED: %s\n' "$*" >&2; exit 1; }
running() { docker ps --format '{{.Names}}' | grep -x "$1" >/dev/null; }
code() { curl -s -o /dev/null -m 15 -w '%{http_code}' "$1" 2>/dev/null || echo 000; }
compose() { docker compose -f "$SV_ROOT/docker-compose.yml" "$@"; }
case "$MODE" in --check|--go|--off) ;; *) die "usage: sv-site-up.sh <source dir> --check|--go|--off";; esac
find "$SRC" -maxdepth 4 -type f \( -name '*.sh' -o -name '*.py' -o -name '*.caddy' -o -name '*.yml' \) -not -path '*/payload/*' \
  -exec sed -i 's/\r$//' {} + 2>/dev/null || true
GCADDY="$SRC/stream/server/gamedata-caddy.py"
CHECK="$SRC/tools/survivors/survivors_check.py"
LOCK="$SRC/stream/survivors/upstream.lock.json"
SNIP="$SRC/hosting/survivors/survivors.caddy"
COMPOSE="$SRC/hosting/survivors/docker-compose.yml"
for f in "$GCADDY" "$CHECK" "$LOCK" "$SNIP" "$COMPOSE"; do [ -f "$f" ] || die "the upload is incomplete (no ${f#"$SRC"/})"; done
[ "${SV_TEST:-0}" = 1 ] || [ "$(id -u)" = 0 ] || die "run as root"
command -v docker >/dev/null || die "docker not found"
[ -n "$HOST" ] || die "no address: hosting/survivors/address.txt (SV_SITE_HOST) says none"
[[ "$HOST" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]] || die "the address '$HOST' is not a host name"
printf '%s' "$HOST" | grep -qiE 'wow|warcraft|blizzard|lich-?king' && die "the address '$HOST' has a franchise name in it (the brand rule): pick another"
case "$HOST" in   # the other sites' own addresses (old and new domain)
  "$PUBLIC"|play.openvanilla.app|wrath.openvanilla.app|relay.openvanilla.app|leylinelauncher.com|www.leylinelauncher.com|play.leylinelauncher.com|relay.leylinelauncher.com)
    die "the address '$HOST' belongs to another site";;
esac
# ... and every other name in the one list (tools/domain/addresses.json, uploaded with this script; docs/DOMAIN-MOVE.md)
if [ -f "$SRC/tools/domain/addresses.json" ] && python3 -I - "$SRC/tools/domain/addresses.json" "$HOST" <<'PY'
import json, sys
a = json.load(open(sys.argv[1], encoding="utf-8"))
taken = {v.get(k) for p, v in a.items() if isinstance(v, dict) and p != "survivors" for k in ("new", "old", "to")}
sys.exit(0 if sys.argv[2] in taken else 1)
PY
then die "the address '$HOST' belongs to another site"; fi

caddy_site() {  # caddy_site add|remove|has|other: our site block in the Caddyfile (bind-mounted into Caddy)
  python3 -I - "$1" "$CF" "$HOST" "$SNIP" "$SITE_BEGIN" "$SITE_END" <<'PY'
import re, sys
cmd, path, host, snip, begin, end = sys.argv[1:7]
s = open(path, encoding="utf-8").read()
def strip(s):
    while begin in s:
        i = s.index(begin); j = s.find(end, i)
        if j < 0:
            sys.exit("broken: start marker without end marker")
        j = s.find("\n", j)
        s = s[:i] + (s[j + 1:] if j >= 0 else "")
    return s
if cmd == "has":
    print("yes" if begin in s else "no"); sys.exit(0)
if cmd == "other":   # another site block already answers for this host (not ours)
    print("yes" if re.search(r"(?m)^\s*(https?://)?" + re.escape(host) + r"(:\d+)?\s*(,|\{)", strip(s)) else "no"); sys.exit(0)
new = strip(s)
if cmd == "add":
    block = open(snip, encoding="utf-8").read().replace("__SV_HOST__", host)
    if not block.startswith(begin) or not block.rstrip("\n").endswith(end) or "__SV_HOST__" in block:
        sys.exit("survivors.caddy is not the site block (markers)")
    new = new.rstrip("\n") + "\n\n" + block.rstrip("\n") + "\n"
if new != s:
    open(path, "w", encoding="utf-8").write(new)
PY
}
caddy_reload() {
  docker exec "$CADDY" caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1 \
    && docker exec "$CADDY" caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1
}
box_ips() {   # this server's public addresses: what the website resolves to, and the box's own
  { getent ahostsv4 "$PUBLIC" | awk '{print $1}'; hostname -I 2>/dev/null | tr ' ' '\n'; } | grep -E '^[0-9.]+$' | sort -u
}
dns_ok() { local ip; ip=$(getent ahostsv4 "$HOST" | awk 'NR==1{print $1}'); [ -n "$ip" ] && box_ips | grep -qx "$ip"; }
inside() {   # inside: the answer code of http://sv-play:8090<path> on the Docker network (wu-portal asks)
  docker exec wu-portal python3 -c 'import sys,urllib.request,urllib.error
try: print(urllib.request.urlopen("http://sv-play:8090"+sys.argv[1],timeout=10).status)
except urllib.error.HTTPError as e: print(e.code)
except Exception: print("000")' "$1" 2>/dev/null || echo 000
}
play_off() { rm -f "$PORTAL_DIR/survivors.url"; }
play_on() { ( umask 022; printf 'https://%s/\n' "$HOST" > "$PORTAL_DIR/survivors.url.tmp" && mv -f "$PORTAL_DIR/survivors.url.tmp" "$PORTAL_DIR/survivors.url" ); }

# ---------------------------------------------------------------------------------------------- off
if [ "$MODE" = --off ]; then
  log "Off: Azeroth Survivors"
  play_off && note "Play is off on /wrath/survivors (Coming soon again)"
  if [ -f "$CF" ] && [ "$(caddy_site has)" = yes ]; then
    ( umask 077; mkdir -p "$BK" ); cp -p "$CF" "$BK/Caddyfile"
    python3 -I "$GCADDY" remove-sv "$CF" >/dev/null 2>&1 || true
    caddy_site remove
    if caddy_reload; then note "Caddy: the $HOST site is out (backup $BK/Caddyfile)"
    else cat "$BK/Caddyfile" > "$CF"; caddy_reload || true; die "Caddy refused; the Caddyfile is as it was (Play stays off)"; fi
  fi
  if [ -f "$SV_ROOT/docker-compose.yml" ]; then compose down >/dev/null 2>&1 && note "sv-play stopped and removed"; fi
  docker rm -f "$SITE" >/dev/null 2>&1 || true
  note "the files in $SV_ROOT are kept (www, slice, names.sha256)"
  echo; echo "SV SITE OFF"; exit 0
fi

# ---------------------------------------------------------------------------------------------- checks
log "1/6 Checks"
BLOCKED=""
gate() { if [ "$MODE" = --check ]; then note "WOULD STOP: $1"; BLOCKED=1; else die "$1"; fi; }
PAY="$SRC/payload"
if [ -d "$PAY/www" ] || [ -d "$PAY/slice" ]; then
  [ -d "$PAY/www" ] && [ -d "$PAY/slice" ] && [ -f "$PAY/names.sha256" ] || gate "the upload has only part of payload/ (www, slice and names.sha256 go together)"
  python3 -I "$CHECK" all "$PAY/www" "$PAY/slice" "$PAY/names.sha256" "$LOCK" | sed 's/^/  /'
  [ "${PIPESTATUS[0]}" = 0 ] || gate "survivors-check did not pass on the upload (above): nothing is installed"
  note "upload: $(du -sh "$PAY/www" | cut -f1) build, $(du -sh "$PAY/slice" | cut -f1) slice"
else
  PAY=""
  if [ -d "$SV_ROOT/www" ] && [ -d "$SV_ROOT/slice" ] && [ -f "$SV_ROOT/names.sha256" ]; then
    python3 -I "$CHECK" all "$SV_ROOT/www" "$SV_ROOT/slice" "$SV_ROOT/names.sha256" "$LOCK" | sed 's/^/  /'
    [ "${PIPESTATUS[0]}" = 0 ] || gate "survivors-check did not pass on what is installed in $SV_ROOT (above)"
    note "no upload: serving what is installed in $SV_ROOT"
  else
    gate "no upload and nothing installed in $SV_ROOT: the build and the slice come from Put Survivors on Leyline.bat"
  fi
fi
[ -x "$HOST_BIN" ] || gate "the slice host $HOST_BIN is not on this box (the Wrath stream's wenilla-host)"
[ -f "$CF" ] || gate "no Caddyfile at $CF"
running "$CADDY" || gate "Caddy ($CADDY) is not running"
running wu-portal || gate "wu-portal (the sign-in gate) is not running"
grep -q 'site_auth' "$PORTAL_DIR/survivors.py" 2>/dev/null || gate "the sign-in gate here predates Azeroth Survivors: the \"signin\" step first (it ships stream/survivors.py)"
docker network inspect "$NET" >/dev/null 2>&1 || gate "no Docker network $NET (Caddy's)"
ip=$(getent ahostsv4 "$HOST" | awk 'NR==1{print $1}')
if dns_ok; then note "address: $HOST -> $ip (this server)"
else gate "the address $HOST has no DNS record pointing at this server (it says: ${ip:-nothing}). Add an A record: $HOST -> $(box_ips | head -1)"; fi
[ -f "$CF" ] && [ "$(caddy_site other)" = yes ] && gate "the Caddyfile already has a site for $HOST that is not ours"
note "already: sv-play $(running "$SITE" && echo running || echo 'not running'); Caddy block $( [ -f "$CF" ] && caddy_site has || echo -); Play $( [ -f "$PORTAL_DIR/survivors.url" ] && cat "$PORTAL_DIR/survivors.url" || echo off)"
[ -f "$GD_FLAG" ] && note "the game data killswitch is ON: the site goes up resting (/data 410, pages a resting page)"
if [ "$MODE" = --check ]; then
  echo
  if [ -n "$BLOCKED" ]; then echo "Not ready: see WOULD STOP above."; else echo "Ready: --go would put Azeroth Survivors on https://$HOST/."; fi
  echo "SV SITE DRY"; exit 0
fi

# ---------------------------------------------------------------------------------------------- install
( umask 077; mkdir -p "$BK" )
mkdir -p "$SV_ROOT"
NEW_FILES=""
log "2/6 Files: $SV_ROOT"
if [ -n "$PAY" ]; then
  for d in www slice; do
    rm -rf "$SV_ROOT/$d.new" && cp -a "$PAY/$d" "$SV_ROOT/$d.new" || die "could not copy $d"
  done
  cp -p "$PAY/names.sha256" "$SV_ROOT/names.sha256.new"
  for d in www slice; do
    if [ -d "$SV_ROOT/$d" ]; then rm -rf "$SV_ROOT/$d.prev"; mv "$SV_ROOT/$d" "$SV_ROOT/$d.prev"; fi
    mv "$SV_ROOT/$d.new" "$SV_ROOT/$d"
  done
  [ -f "$SV_ROOT/names.sha256" ] && mv -f "$SV_ROOT/names.sha256" "$SV_ROOT/names.sha256.prev"
  mv -f "$SV_ROOT/names.sha256.new" "$SV_ROOT/names.sha256"
  chmod -R a+rX "$SV_ROOT/www" "$SV_ROOT/slice"
  NEW_FILES=1
  note "installed (the last ones kept as www.prev, slice.prev)"
fi
undo_files() {
  [ -n "$NEW_FILES" ] || return 0
  for d in www slice; do [ -d "$SV_ROOT/$d.prev" ] && { rm -rf "$SV_ROOT/$d"; mv "$SV_ROOT/$d.prev" "$SV_ROOT/$d"; }; done
  [ -f "$SV_ROOT/names.sha256.prev" ] && mv -f "$SV_ROOT/names.sha256.prev" "$SV_ROOT/names.sha256"
  return 0
}
fail() {
  printf '\n  rolling back\n' >&2
  play_off
  if [ -f "$BK/Caddyfile" ]; then cat "$BK/Caddyfile" > "$CF"; caddy_reload || true; fi
  undo_files
  if [ -n "${WAS_RUNNING:-}" ]; then compose up -d --force-recreate >/dev/null 2>&1 || true; else compose down >/dev/null 2>&1 || true; fi
  die "$1"
}

log "3/6 sv-play (the build and the slice, nothing else)"
WAS_RUNNING=""; running "$SITE" && WAS_RUNNING=1
install -m 644 "$COMPOSE" "$SV_ROOT/docker-compose.yml"
SV_ROOT="$SV_ROOT" SV_HOST_BIN="$HOST_BIN" compose up -d --force-recreate >/dev/null || fail "sv-play did not start"
ok=""
for _ in $(seq 1 30); do [ "$(inside /index.html)" = 200 ] && { ok=1; break; }; sleep 1; done
[ -n "$ok" ] || { docker logs --tail 20 "$SITE" >&2 2>&1 || true; fail "sv-play does not serve the page"; }
c=$(inside /data/__index); [ "$c" = 200 ] || fail "sv-play does not list the slice (/data/__index -> $c)"
c=$(inside "/data/Interface%5CGlues%5CCommon%5CGlues-WoW-Logo.blp"); [ "$c" = 404 ] || fail "sv-play serves the franchise logo ($c): the slice must not hold it"
note "sv-play serves the page and the slice; anything else is a 404"

log "4/6 Caddy: https://$HOST"
cp -p "$CF" "$BK/Caddyfile"
caddy_site add || fail "could not edit the Caddyfile"
if [ -f "$GD_FLAG" ]; then python3 -I "$GCADDY" add-sv "$CF" "$HOST" || fail "could not add the killswitch block"; note "killswitch on: the site rests"; fi
caddy_reload || fail "Caddy refused the new site"
[ "$(code "https://$PUBLIC/wrath/status.json")" = 200 ] || fail "the Wrath site stopped answering"
echo "$HOST" > "$SV_ROOT/site-host"; chmod 644 "$SV_ROOT/site-host"
ok=""
for _ in $(seq 1 30); do   # the first certificate takes a few seconds
  c=$(code "https://$HOST/licenses/LICENSE-MIT"); [ "$c" = 200 ] && { ok=1; break; }; sleep 3
done
[ -n "$ok" ] || fail "https://$HOST/licenses/LICENSE-MIT answers $c (certificate or DNS?)"
note "https://$HOST/licenses/ -> 200 (for anyone)"

log "5/6 Self-check"
bad=0
if [ -f "$GD_FLAG" ]; then
  c=$(code "https://$HOST/data/__index"); [ "$c" = 410 ] && note "/data/__index -> 410 (resting)" || { note "!! /data/__index -> $c (want 410)"; bad=1; }
  c=$(code "https://$HOST/"); [ "$c" = 503 ] && note "/ -> 503 resting page" || { note "!! / -> $c (want the resting page)"; bad=1; }
else
  c=$(code "https://$HOST/data/__index"); case "$c" in 401|403) note "/data/__index without sign-in -> $c (behind the sign-in)";;
    *) note "!! /data/__index without sign-in -> $c (want 403)"; bad=1;; esac
  c=$(code "https://$HOST/"); case "$c" in 302|303) note "/ without sign-in -> $c (to Play on $PUBLIC)";;
    *) note "!! / without sign-in -> $c (want a 302 to the sign-in)"; bad=1;; esac
fi
c=$(code "https://$HOST/licenses/"); [ "$c" = 200 ] && note "/licenses/ -> 200 (the credit and the upstream licence files)" \
  || { note "!! /licenses/ -> $c (want 200)"; bad=1; }
w=$(curl -s -m 15 "https://$HOST/licenses/" 2>/dev/null | head -c 200000 | tr 'A-Z' 'a-z' | grep -oE "$BRAND_RE" | sort -u | tr '\n' ' ')
[ -z "${w// /}" ] || { note "!! brand words on /licenses/: $w"; bad=1; }
[ "$bad" = 0 ] || fail "the self-check did not pass (above)"

log "6/6 Play on"
play_on
note "/wrath/survivors shows Play, to https://$HOST/ ($PORTAL_DIR/survivors.url)"
echo
echo "Azeroth Survivors plays at https://$HOST/ (signed in through https://$PUBLIC/wrath/survivors)."
echo "Off again: Put Survivors on Leyline - off.bat. Backup: $BK"
echo "SV SITE ON"
