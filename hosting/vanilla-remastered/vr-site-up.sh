#!/usr/bin/env bash
# Vanilla Remastered in the browser: the OG 1.12 site and game page again (owner 2026-10-31: "the style we had in the
# og project"), at its own address, playing the levels 1-20 slice on the Vanilla Remastered realm.
# Run as root by "Vanilla Remastered browser page.bat" (one SSH login; this folder, gamedata-caddy.py and the kit's
# wunet arrive as a tarball over stdin).
#
#   bash vr-site-up.sh <source dir> --check   read-only: what it would do, the address (DNS), what is ready
#   bash vr-site-up.sh <source dir> --go      do it (backup, start, self-check, roll back on failure)
#   bash vr-site-up.sh <source dir> --test    the self-check only, with a fresh TEST account (made, logged in, deleted)
#   bash vr-site-up.sh <source dir> --off     the page off again (container stopped and removed, Caddy block out;
#                                             its copied volumes are kept)
#
# What it runs: vr-site, a COPY of the old 1.12 site container (wenilla-realm-realm-1: the same image id, i.e. the
# client build and site that were live before, never the private leyline-client-112 merge), with three changes:
#   * its game connection goes to the Vanilla Remastered realm on vr-112 (REALMD_HOST=vr-realmd,
#     MANGOSD_HOST=vr-mangosd) instead of the old names, which on the website's network now are nobody and the
#     Wrath bridge;
#   * its game data (CLIENT_DATA) is /srv/vr112/slice/Data, the levels 1-20 slice (vr-data.sh), read-only;
#   * its own volumes are copies (vr-site-<name>), so the old site container, which keeps running untouched, and
#     the copy never write the same files. The game accounts themselves are in the 1.12 database, shared as before.
# and a Caddy site for its address (VR_SITE_HOST, default vanilla.openvanilla.app): the page reads its game files at
# /data and its game connection at /ws from the root of its own address, so it can't live under a path of
# play.openvanilla.app (where /data, /ws and / belong to other pages). The address needs a DNS A record pointing at
# this server: the check says whether it has one, and the --go run stops before touching Caddy when it doesn't.
# The game data killswitch (gamedata-switch.sh) covers it: when the switch is on, the site gets its 410 block at once.
#
# Last line: "VR SITE ON" / "VR SITE DRY" / "VR SITE TEST OK" / "VR SITE OFF", or "VR SITE FAILED: ..." (exit 1).
# Never prints the environment (it holds the database password), a SOAP login or a test password.
set -uo pipefail
SRC="${1:?usage: vr-site-up.sh <source dir> --check|--go|--test|--off}"
MODE="${2:---check}"
OLD="${VR_OLD_SITE:-wenilla-realm-realm-1}"
SITE=vr-site
NET="${VR_NET:-vr-112}"
SITE_NET="${VR_SITE_NET:-wenilla-realm_default}"
CF="${VR_CADDYFILE:-/opt/openvanilla-classicplus/Caddyfile}"
CADDY="${VR_CADDY:-wenilla-realm-caddy-1}"
VR_ROOT="${VR_ROOT:-/srv/vr112}"
SLICE="$VR_ROOT/slice/Data"
HOST="${VR_SITE_HOST:-$(tr -cd 'a-z0-9.-' < "$VR_ROOT/site-host" 2>/dev/null)}"   # the address it was put up at
HOST="${HOST:-vanilla.openvanilla.app}"
PORT="${VR_SITE_PORT:-8090}"
PORTAL_DIR="${WU_PORTAL_DIR:-/srv/wrath/portal}"
GD_FLAG="${WU_GAMEDATA_FLAG_HOST:-$PORTAL_DIR/gamedata-off}"
ENV_DIR="${VR_ENV_DIR:-/root/vr112-env}"
MEM="${VR_SITE_MEM:-256m}"
LOGON=vr-realmd
WORLD=vr-mangosd
BRAND_RE='world *of *warcraft|(^|[^a-z])wow([^a-z]|$)|warcraft|lich *king|blizzard'
SITE_BEGIN="# >>> Vanilla Remastered site (vr-site-up.sh)"
SITE_END="# <<< Vanilla Remastered site"
log() { printf '\n== %s\n' "$*"; }
note() { printf '  %s\n' "$*"; }
die() { printf '\nVR SITE FAILED: %s\n' "$*" >&2; exit 1; }
running() { docker ps --format '{{.Names}}' | grep -x "$1" >/dev/null; }
exists() { docker ps -a --format '{{.Names}}' | grep -x "$1" >/dev/null; }
code() { curl -s -o /dev/null -m 15 -w '%{http_code}' "$1" 2>/dev/null || echo 000; }
case "$MODE" in --check|--go|--test|--off) ;; *) die "usage: vr-site-up.sh <source dir> --check|--go|--test|--off";; esac
[[ "$HOST" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]] || die "VR_SITE_HOST is not a host name"
find "$SRC" -type f \( -name '*.sh' -o -name '*.py' \) -exec sed -i 's/\r$//' {} + 2>/dev/null || true
PROBE="$SRC/hosting/vanilla-remastered/probe112.py"
GCADDY="$SRC/stream/server/gamedata-caddy.py"
[ -f "$PROBE" ] && [ -f "$GCADDY" ] && [ -f "$SRC/hosting/kit/tools/wunet/srp6.py" ] || die "the upload is incomplete"
[ "${VR_TEST:-0}" = 1 ] || [ "$(id -u)" = 0 ] || die "run as root"
command -v docker >/dev/null || die "docker not found"
PY_IMG=""
for i in python:3.12-alpine wu-hangout-tools:1; do docker image inspect "$i" >/dev/null 2>&1 && { PY_IMG=$i; break; }; done
probe() {  # probe NETWORK [--env-file F] ARGS...
  local net="$1"; shift
  local extra=()
  if [ "${1:-}" = --env-file ]; then extra=(--env-file "$2"); shift 2; fi
  [ -n "$PY_IMG" ] || { echo "no python image on this box"; return 1; }
  docker run --rm --network "$net" "${extra[@]}" --memory 64m --cap-drop ALL --security-opt no-new-privileges --read-only \
    -v "$SRC/hosting/kit/tools/wunet:/p/wunet:ro" -v "$PROBE:/p/probe112.py:ro" --entrypoint python3 "$PY_IMG" \
    -I /p/probe112.py "$@"
}
envval() { docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$1" 2>/dev/null | sed -n "s/^$2=//p" | head -1; }

caddy_site() {  # caddy_site add|remove|has: our site block in the Caddyfile (the file is bind-mounted into Caddy)
  python3 -I - "$1" "$CF" "$HOST" "$SITE" "$PORT" "$SITE_BEGIN" "$SITE_END" <<'PY'
import sys
cmd, path, host, up, port, begin, end = sys.argv[1:8]
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
    t = strip(s)
    import re
    print("yes" if re.search(r"(?m)^\s*(https?://)?" + re.escape(host) + r"(:\d+)?\s*(,|\{)", t) else "no"); sys.exit(0)
new = strip(s)
if cmd == "add":
    new = new.rstrip("\n") + "\n\n" + (
        f"{begin}: the OG 1.12 site and game page for Vanilla Remastered (levels 1-20 slice).\n"
        f"{host} {{\n"
        "\tencode zstd gzip\n"
        f"\treverse_proxy {up}:{port}\n"
        "}\n"
        f"{end}\n")
if new != s:
    open(path, "w", encoding="utf-8").write(new)
PY
}
caddy_reload() {
  docker exec "$CADDY" caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1 \
    && docker exec "$CADDY" caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1
}
box_ips() {   # this server's public addresses: what play.openvanilla.app resolves to, and the box's own
  { getent ahostsv4 play.openvanilla.app | awk '{print $1}'; hostname -I 2>/dev/null | tr ' ' '\n'; } | grep -E '^[0-9.]+$' | sort -u
}
dns_ok() { local ip; ip=$(getent ahostsv4 "$HOST" | awk 'NR==1{print $1}'); [ -n "$ip" ] && box_ips | grep -qx "$ip"; }

# ---------------------------------------------------------------------------------------------- self-check
self_test() {   # the page answers, the slice is served behind it, brand words, then a fresh TEST account end to end
  local bad=0 out f
  log "Self-check"
  out=$(probe "$SITE_NET" http "http://$SITE:$PORT/" 2>&1) && note "page inside: $out" || { note "page inside: FAILED ($out)"; bad=1; }
  note "https://$HOST/ -> $(code "https://$HOST/")"
  c=$(code "https://$HOST/data/__index")
  case "$c" in 401|403|302|303) note "https://$HOST/data/__index without sign-in -> $c (behind the sign-in)";;
    410) note "https://$HOST/data/__index -> 410 (the game data killswitch is on)";;
    200) note "!! https://$HOST/data/__index without sign-in -> 200: the old site serves its game files to anyone (as it did before); the slice is only levels 1-20";;
    *) note "https://$HOST/data/__index -> $c";; esac
  local pages="" p
  for p in / /play /game /key /login; do
    pages+=$(curl -s -m 15 "https://$HOST$p" 2>/dev/null | head -c 400000 | tr 'A-Z' 'a-z' | grep -oE "$BRAND_RE" | sort -u | tr '\n' ' ')
  done
  if [ -n "${pages// /}" ]; then note "!! brand words on the old site's pages: $(echo "$pages" | tr ' ' '\n' | sort -u | tr '\n' ' ') (the page is the OG site as it was; see the runbook)"
  else note "brand words on /, /play, /game, /key, /login: none"; fi
  # the test account: the SOAP login is the one the site itself uses (its environment), handed over in an env file
  f="$ENV_DIR/vr-soap-test.env"; ( umask 077; mkdir -p "$ENV_DIR"; : > "$f" )
  local su sp src=$SITE; exists "$SITE" || src=$OLD
  su=$(docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$src" 2>/dev/null | grep -iE '^[A-Z0-9_]*SOAP[A-Z0-9_]*USER[A-Z0-9_]*=' | head -1 | cut -d= -f2-)
  sp=$(docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$src" 2>/dev/null | grep -iE '^[A-Z0-9_]*SOAP[A-Z0-9_]*PASS[A-Z0-9_]*=' | head -1 | cut -d= -f2-)
  printf 'VR_SOAP_USER=%s\nVR_SOAP_PASS=%s\n' "$su" "$sp" > "$f"
  local relay=() rp
  # on Leyline (vr-beacon): the relay's own login port for this realm, from the agent's state
  rp=$(python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1])).get("auth_port",""))' \
       "${VR_BEACON_DIR:-/srv/wrath/vr-beacon}/state/relay-state.json" 2>/dev/null || true)
  [[ "$rp" =~ ^[0-9]+$ ]] && relay=(relay.openvanilla.app "$rp")
  local soap_port; soap_port=$(envval "$src" SOAP_PORT); [[ "$soap_port" =~ ^[0-9]+$ ]] || soap_port=7878
  # inside the page's own network namespace when it runs: the realm's names (vr-112) and the relay (outside) both resolve
  local pnet="$NET"; if running "$SITE"; then pnet="container:$SITE"; else relay=(); fi
  out=$(probe "$pnet" --env-file "$f" account "$WORLD" "$soap_port" "$LOGON" 3724 "$WORLD" 8085 "${relay[@]}" 2>&1); rc=$?
  rm -f "$f"
  printf '%s\n' "$out" | sed 's/^/  /'
  case "$rc" in 0) ;; 3) note "!! the account check was skipped: no SOAP login in the site's settings (new sign-ups need it too)";;
    *) bad=1;; esac
  return $bad
}

# ---------------------------------------------------------------------------------------------- off
if [ "$MODE" = --off ]; then
  log "Off: the Vanilla Remastered page"
  if [ -f "$CF" ] && [ "$(caddy_site has)" = yes ]; then
    BK="/root/vr-site-off-$(date +%Y%m%d-%H%M%S)"; ( umask 077; mkdir -p "$BK" ); cp -p "$CF" "$BK/Caddyfile"
    python3 -I "$GCADDY" remove-vr "$CF" >/dev/null 2>&1 || true
    caddy_site remove
    if caddy_reload; then note "Caddy: the $HOST site is out (backup $BK/Caddyfile)"
    else cat "$BK/Caddyfile" > "$CF"; caddy_reload || true; die "Caddy refused; the Caddyfile is as it was"; fi
  fi
  if exists "$SITE"; then docker rm -f "$SITE" >/dev/null && note "$SITE removed (its copied volumes vr-site-* are kept)"; fi
  rm -f "$PORTAL_DIR/vr-site.url" "$VR_ROOT/site-host"
  note "the old site ($OLD), the realm and the slice are not touched"
  echo; echo "VR SITE OFF"; exit 0
fi

if [ "$MODE" = --test ]; then
  running "$LOGON" && running "$WORLD" || die "the realm is not running (the ov112-on step first)"
  self_test || die "the self-check did not pass (see above)"
  echo; echo "VR SITE TEST OK"; exit 0
fi

# ---------------------------------------------------------------------------------------------- checks
log "1/5 Checks"
exists "$OLD" || die "$OLD (the old 1.12 site) is not on this box: there is nothing to copy the page from"
IMG=$(docker inspect -f '{{.Image}}' "$OLD"); note "$OLD: image $(docker inspect -f '{{.Config.Image}}' "$OLD") ($IMG), $(running "$OLD" && echo running || echo stopped)"
CD=$(envval "$OLD" CLIENT_DATA); CD=${CD:-/client/Data}
note "its game data setting: CLIENT_DATA=$CD; its game connection: REALMD_HOST=$(envval "$OLD" REALMD_HOST) MANGOSD_HOST=$(envval "$OLD" MANGOSD_HOST)"
note "address settings that name the old address (names only): $(docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$OLD" | grep -E '^[A-Z0-9_]*(URL|DOMAIN|HOST|ORIGIN|BASE)[A-Z0-9_]*=.*play\.openvanilla\.app' | cut -d= -f1 | tr '\n' ' ')"
BLOCKED=""
gate() { if [ "$MODE" = --check ]; then note "WOULD STOP: $1"; BLOCKED=1; else die "$1"; fi; }
running "$LOGON" && running "$WORLD" || gate "the realm is not running (vr-realmd, vr-mangosd): the ov112-on step first"
[ -n "$(find "$SLICE" -maxdepth 1 -iname 'dbc.mpq' 2>/dev/null)" ] || gate "no slice in $SLICE: the vr-data step first"
[ -n "$PY_IMG" ] || gate "no python image for the self-check"
[ -f "$CF" ] || gate "no Caddyfile at $CF"
running "$CADDY" || gate "Caddy ($CADDY) is not running"
ip=$(getent ahostsv4 "$HOST" | awk 'NR==1{print $1}')
if dns_ok; then note "address: $HOST -> $ip (this server)"
else gate "the address $HOST has no DNS record pointing at this server (it says: ${ip:-nothing}). The owner adds an A record: $HOST -> $(box_ips | head -1) (or set VR_SITE_HOST to a name that has one)"; fi
[ -f "$CF" ] && [ "$(caddy_site other)" = yes ] && gate "the Caddyfile already has a site for $HOST that is not ours"
note "memory available: $(awk '/MemAvailable/ {print int($2/1024)}' /proc/meminfo) MB (the page takes up to $MEM)"
[ -f "$GD_FLAG" ] && note "the game data killswitch is ON: the page goes up with /data answering 410"

if [ "$MODE" = --check ]; then
  note "slice: $(du -sh "$SLICE" 2>/dev/null | cut -f1 || echo none)"
  note "already: $SITE $(exists "$SITE" && (running "$SITE" && echo running || echo stopped) || echo 'not made'); Caddy block $( [ -f "$CF" ] && caddy_site has || echo -)"
  echo; echo "Would: copy $OLD into $SITE (same image, its game connection to $LOGON/$WORLD on $NET, CLIENT_DATA = the slice,"
  echo "copied volumes), add a Caddy site for https://$HOST, self-check with a fresh TEST account, and write the page's"
  echo "address for the website ($PORTAL_DIR/vr-site.url)."
  [ -n "$BLOCKED" ] && echo "The --go run would stop at the WOULD STOP lines above."
  echo; echo "VR SITE DRY"; exit 0
fi

# ---------------------------------------------------------------------------------------------- backup
log "2/5 Backup"
BK="/root/vr-site-on-$(date +%Y%m%d-%H%M%S)"; ( umask 077; mkdir -p "$BK" ) || die "could not make $BK"
cp -p "$CF" "$BK/Caddyfile"
SITE_WAS=0; exists "$SITE" && SITE_WAS=1
note "backup: $BK (the Caddyfile)"
NEWVOLS=()
rollback() {
  echo "  rolling back" >&2
  cat "$BK/Caddyfile" > "$CF"; caddy_reload || true
  [ "$SITE_WAS" = 0 ] && docker rm -f "$SITE" >/dev/null 2>&1
  local v; for v in "${NEWVOLS[@]}"; do docker volume rm "$v" >/dev/null 2>&1; done
  rm -f "$PORTAL_DIR/vr-site.url" "$VR_ROOT/site-host"
}
fail() { rollback; die "$1; rolled back (backup $BK)"; }

# ---------------------------------------------------------------------------------------------- the container
log "3/5 The page: $SITE"
if running "$SITE"; then
  note "$SITE already runs: restarted on the current slice"
  docker restart "$SITE" >/dev/null || fail "could not restart $SITE"
else
  exists "$SITE" && docker rm -f "$SITE" >/dev/null
  ( umask 077; mkdir -p "$ENV_DIR" )
  ( umask 077; docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$OLD" | grep -v '^$' | python3 -I -c '
import re, sys
host = sys.argv[1]
for line in sys.stdin:
    line = line.rstrip("\n")
    k, _, v = line.partition("=")
    if k == "REALMD_HOST":
        v = "vr-realmd"
    elif k == "MANGOSD_HOST":
        v = "vr-mangosd"
    elif re.search(r"(URL|DOMAIN|HOST|ORIGIN|BASE)", k) and "play.openvanilla.app" in v:
        v = v.replace("play.openvanilla.app", host)
    print(f"{k}={v}")
' "$HOST" > "$ENV_DIR/$SITE.env" ) || fail "could not write the page's settings"
  chmod 600 "$ENV_DIR/$SITE.env"
  run=(docker run -d --name "$SITE" --restart unless-stopped --network "$NET" --env-file "$ENV_DIR/$SITE.env"
       --label leyline.vr=1 --label "leyline.vr.from=$OLD" --security-opt no-new-privileges --memory "$MEM"
       --log-opt max-size=20m --log-opt max-file=3)
  wd=$(docker inspect -f '{{.Config.WorkingDir}}' "$OLD"); [ -n "$wd" ] && run+=(-w "$wd")
  user=$(docker inspect -f '{{.Config.User}}' "$OLD"); [ -n "$user" ] && run+=(--user "$user")
  have_data=0
  while IFS='|' read -r typ name srcp dst rw; do
    [ -n "$dst" ] || continue
    if [ "$dst" = "$CD" ] || [ "$dst" = "$(dirname "$CD")" ]; then
      run+=(--mount "type=bind,src=$SLICE,dst=$CD,readonly"); have_data=1; continue
    fi
    case "$typ" in
      volume)
        nv="vr-site-$(printf '%s' "$name" | sed 's/[^A-Za-z0-9_.-]/_/g' | cut -c1-50)"
        if ! docker volume inspect "$nv" >/dev/null 2>&1; then
          docker volume create --label leyline.vr=1 "$nv" >/dev/null || fail "could not make the volume $nv"
          NEWVOLS+=("$nv")
          docker run --rm --network none -v "$name:/a:ro" -v "$nv:/b" --entrypoint sh "$IMG" -c 'cp -a /a/. /b/' >/dev/null 2>&1 \
            || docker run --rm --network none -v "$name:/a:ro" -v "$nv:/b" --entrypoint sh "${PY_IMG:-$IMG}" -c 'cp -a /a/. /b/' >/dev/null \
            || fail "could not copy the volume $name"
          note "volume $name copied to $nv ($dst)"
        else
          note "volume $nv (copied before) at $dst"
        fi
        m="type=volume,src=$nv,dst=$dst" ;;
      bind) m="type=bind,src=$srcp,dst=$dst"; note "bind $srcp at $dst (shared with $OLD$([ "$rw" = false ] && echo ', read-only'))" ;;
      *) continue ;;
    esac
    [ "$rw" = false ] && m="$m,readonly"
    run+=(--mount "$m")
  done < <(docker inspect -f '{{range .Mounts}}{{.Type}}|{{.Name}}|{{.Source}}|{{.Destination}}|{{.RW}}{{println}}{{end}}' "$OLD")
  [ "$have_data" = 1 ] || run+=(--mount "type=bind,src=$SLICE,dst=$CD,readonly")
  mapfile -d '' ep < <(docker inspect -f '{{json .Config.Entrypoint}}' "$OLD" | python3 -I -c 'import json,sys;v=json.load(sys.stdin) or [];sys.stdout.write("".join(a+"\0" for a in v))')
  mapfile -d '' cmd < <(docker inspect -f '{{json .Config.Cmd}}' "$OLD" | python3 -I -c 'import json,sys;v=json.load(sys.stdin) or [];sys.stdout.write("".join(a+"\0" for a in v))')
  if [ "${#ep[@]}" -gt 0 ]; then run+=(--entrypoint "${ep[0]}" "$IMG" "${ep[@]:1}" "${cmd[@]}"); else run+=("$IMG" "${cmd[@]}"); fi
  "${run[@]}" >/dev/null || fail "$SITE did not start"
  docker network connect --alias "$SITE" "$SITE_NET" "$SITE" >/dev/null || fail "could not join $SITE to $SITE_NET (Caddy's network)"
  note "$SITE: made from $OLD (game connection: $LOGON / $WORLD on $NET; game data: the slice, read-only; memory $MEM)"
fi
ok=""
for _ in $(seq 1 30); do
  if probe "$SITE_NET" http "http://$SITE:$PORT/" >/dev/null 2>&1; then ok=1; break; fi
  running "$SITE" || break
  sleep 2
done
[ -n "$ok" ] || { docker logs --tail 30 "$SITE" >&2 2>&1 || true; fail "$SITE does not answer on port $PORT"; }

# ---------------------------------------------------------------------------------------------- Caddy
log "4/5 Caddy: https://$HOST"
caddy_site add || fail "could not edit the Caddyfile"
if [ -f "$GD_FLAG" ]; then python3 -I "$GCADDY" add-vr "$CF" "$HOST" || fail "could not add the killswitch block"; note "killswitch on: /data answers 410 here"; fi
caddy_reload || fail "Caddy refused the new site"
w=$(code "https://play.openvanilla.app/wrath/status.json")
[ "$w" = 200 ] || fail "the Wrath site stopped answering ($w)"
echo "$HOST" > "$VR_ROOT/site-host"; chmod 644 "$VR_ROOT/site-host"
ok=""
for _ in $(seq 1 30); do   # the first certificate takes a few seconds
  c=$(code "https://$HOST/"); case "$c" in 200|301|302|303|307|308) ok=1; break;; esac; sleep 3
done
[ -n "$ok" ] || fail "https://$HOST/ answers $c (certificate or DNS?)"
note "https://$HOST/ -> $c"

# ---------------------------------------------------------------------------------------------- check
log "5/5 Self-check"
self_test || fail "the self-check did not pass"
( umask 022; printf 'https://%s/\n' "$HOST" > "$PORTAL_DIR/vr-site.url.tmp" && mv -f "$PORTAL_DIR/vr-site.url.tmp" "$PORTAL_DIR/vr-site.url" )
note "the website's realm card links to it ($PORTAL_DIR/vr-site.url)"
echo
echo "Vanilla Remastered plays in the browser at https://$HOST/ (levels 1-20 streamed; the old site look)."
echo "Off again: the vr-site-off step. Backup: $BK"
echo "VR SITE ON"
