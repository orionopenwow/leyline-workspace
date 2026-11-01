#!/usr/bin/env bash
# The Builders' Realm (TODO(owner): the final name): an official realm just for building, open to approved builders
# (owner 2026-10-31; docs/BUILD-REALM.md). Run as root on the realm's box by the Leyline Control steps (one SSH login;
# this folder, the realm's new files and the kit's tools arrive as a tarball over stdin):
#
#   bash build-realm.sh <source dir> --check    read-only: memory, disk, names, what --go would do   (build-realm-check)
#   bash build-realm.sh <source dir> --go       backup, databases, start, self-check, list it; roll back on failure
#   bash build-realm.sh <source dir> --off      off the list, a 5-minute warning in game, stopped (data kept)
#
# What runs (three small containers next to the main realm; nothing published, no new port):
#   ov-ac-build-world  a second world server: the main realm's own image, world database (no script writes it) and
#                      client data; its own characters database (acore_characters_build); realm id 2; the Lua
#                      scripts folder read-only with realm/profiles/build.env's switches (the Builder and a big Den on;
#                      events, duels, bots, boosts, Discord off), no playerbots, no autobalance, nothing preloaded,
#                      short sight, 20 players at most, a hard memory cap. Its save files: /srv/wrath/build-realm/dens.
#   wu-build-auth      a login server for it (like the hangout's): the same accounts, passwords, bans and GM rights
#                      (acore_auth_build: views of acore_auth's tables), but its own realm list with this realm only,
#                      so the main realm's list keeps exactly one realm (the browser game connects by itself only
#                      when it sees one). No IP bans (every relay player comes from the agent; the relay counts).
#   wu-build-agent     the host kit's agent ("official": never makes accounts or GM rights): the tunnel to the
#                      relay on this box and the signed listing heartbeat, so the realm is on the Leyline list,
#                      reachable from the browser game (hop) and the launcher (the relay's port pair).
# The main realm is never restarted, and its databases' tables are never changed by this script (a backup of the
# login database is made anyway: the build realm's logins write to it through the views, as any login does).
#
#   1 checks   root, Docker, the main realm, the database, the relay, wu-portal, the networks, memory, disk, names
#   2 backup   $DIR/backups/<time>/: acore_auth and the build databases (gzip, 0600), the files this run replaces
#   3 data     acore_auth_build (views + its realm list), acore_characters_build and acore_playerbots_build (the
#              main ones' tables, empty), the database user's rights on them
#   4 files    profiles/build.env, custom/lua_scripts/ov_buildrealm.lua and ov_den.lua on the box (the scripts step
#              doesn't carry ov_den.lua; it returns at once on the main realm); dens/ and logs/
#   5 start    wu-build-auth, then ov-ac-build-world
#   6 health   the login server answers a logon challenge; the world server greets and stays up; the main realm's
#              world and login servers still answer and the world server did not restart
#   7 list     the agent: tunnel up, the relay sees it online, the listing shows it; the realm list address is the
#              relay's; then the id files (play page, in-game list). Any failure: everything this run made goes.
#
# Last line: "BUILD REALM ON" / "BUILD REALM OFF" / "BUILD REALM DRY", or an error (exit 1).
# Never prints the environment (database and SOAP logins), never deletes a volume or the main databases, never
# publishes a port. Everything can be moved for tests (hosting/tests/test_build_realm.py).
set -uo pipefail
SRC="${1:?usage: build-realm.sh <source dir> [--check|--go|--off]}"
MODE="${2:---check}"
WRATH_DIR="${WU_WRATH_DIR:-/srv/wrath}"
DIR="${WU_BUILD_DIR:-$WRATH_DIR/build-realm}"
PORTAL_DIR="${WU_PORTAL_DIR:-$WRATH_DIR/portal}"
RELAY_DIR="${WU_RELAY_DIR:-/opt/wu-relay}"
DB_C="${WU_DB:-ov-ac-database}"
MAIN_WORLD="${WU_MAIN_WORLD:-ov-ac-worldserver}"
MAIN_AUTH="${WU_MAIN_AUTH:-ov-ac-authserver}"
BACKEND_NET="${WU_BACKEND_NET:-ov-wotlk_backend}"
EDGE_NET="${WU_EDGE_NET:-ov-wotlk_edge}"
SITE_NET="${WU_SITE_NET:-wenilla-realm_default}"
DATA_VOL="${WU_DATA_VOL:-ov-client-data}"
WORLD_C=ov-ac-build-world
AUTH_C=wu-build-auth
AGENT_C=wu-build-agent
IMG=wu-build-tools:1
UIDGID=10001:10001                                    # the tools image's "wu" user
REALM_ID="${WU_BUILD_REALM_ID:-2}"
AUTH_DB=acore_auth_build
CHARS_DB=acore_characters_build
BOTS_DB=acore_playerbots_build
DEFAULT_NAME="Builders' Realm"                          # TODO(owner): the final name (docs/WORDS.md has none yet)
NAME="${WU_BUILD_NAME:-$DEFAULT_NAME}"
TAGLINE="${WU_BUILD_TAGLINE:-For approved builders: build together, save, share.}"
SUMMARY="${WU_BUILD_SUMMARY:-The official realm for building. Approved builders sign in with their website login and build in one big shared room; their work saves and can go to a Den or to the main realm.}"
WORLD_MEM="${WU_BUILD_WORLD_MEM:-1536m}"   # 2026-10-31 live: at 1024m it loaded to ~935 MB and kept restarting
AUTH_MEM="${WU_BUILD_AUTH_MEM:-96m}"
MIN_FREE_MB="${WU_BUILD_MIN_FREE_MB:-1800}"
MIN_DISK_MB="${WU_BUILD_MIN_DISK_MB:-2000}"
WORLD_WAIT="${WU_BUILD_WORLD_WAIT:-900}"
WARN_SECS="${WU_BUILD_WARN_SECS:-300}"
TRIES="${WU_HEALTH_TRIES:-30}"
PAUSE="${WU_HEALTH_PAUSE:-2}"
SETTLE="${WU_BUILD_SETTLE:-20}"
log() { printf '\n== %s\n' "$*"; }
note() { printf '  %s\n' "$*"; }
die() { printf '\nBUILD REALM FAILED: %s\n' "$*" >&2; exit 1; }
BLOCKED=""
gate() { if [ "$MODE" = --check ]; then note "WOULD STOP: $1"; BLOCKED=1; else die "$1"; fi; }
running() { docker ps --format '{{.Names}}' | grep -x "$1" >/dev/null; }
exists() { docker ps -a --format '{{.Names}}' | grep -x "$1" >/dev/null; }
envget() { sed -n "s/^$2=\([^ #]*\).*/\1/p" "$1" 2>/dev/null | tail -1; }
jget() {  # jget FILE KEY: a top-level value of a small JSON file, or nothing
  python3 -I - "$1" "$2" <<'PY' 2>/dev/null || true
import json, sys
try:
    v = json.load(open(sys.argv[1])).get(sys.argv[2])
except (OSError, ValueError, AttributeError):
    sys.exit(0)
print("" if v is None else (str(v).lower() if isinstance(v, bool) else v))
PY
}
# SQL as root inside the database container (its root password stays in its own environment; nothing is printed)
sql() { docker exec -i "$DB_C" sh -c 'MYSQL_PWD="${MYSQL_ROOT_PASSWORD:-}" exec mysql -uroot -N -B' ; }
dump() { docker exec "$DB_C" sh -c 'MYSQL_PWD="${MYSQL_ROOT_PASSWORD:-}" exec mysqldump -uroot --single-transaction --skip-triggers "$@"' dump "$@"; }
db_exists() { [ "$(echo "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = '$1';" | sql 2>/dev/null)" = 1 ]; }

case "$MODE" in --check|--go|--off) ;; *) die "usage: build-realm.sh <source dir> [--check|--go|--off]";; esac
find "$SRC" -type f \( -name '*.sh' -o -name '*.py' -o -name '*.env' -o -name '*.lua' -o -name 'Dockerfile' \) \
  -exec sed -i 's/\r$//' {} + 2>/dev/null || true
for f in hosting/build-realm/buildprobe.py hosting/kit/tools/wunet/wire335.py hosting/kit/tools/Dockerfile \
         hosting/kit/tools/wuhost/agent.py hosting/kit/tools/wuhost/setup.py realm/profiles/build.env \
         realm/custom/lua_scripts/ov_buildrealm.lua realm/custom/lua_scripts/ov_den.lua; do
  [ -f "$SRC/$f" ] || die "$f is missing from the upload"
done
[ "${WU_TEST:-0}" = 1 ] || [ "$(id -u)" = 0 ] || die "run as root"
command -v docker >/dev/null || die "docker not found"

PY_IMG=""
for i in python:3.12-alpine wu-hangout-tools:1; do docker image inspect "$i" >/dev/null 2>&1 && { PY_IMG=$i; break; }; done
probe() {  # probe NETWORK ARGS...: buildprobe.py in a throwaway container (read-only, no capabilities)
  local net="$1"; shift
  [ -n "$PY_IMG" ] || { echo "no python image on this box"; return 1; }
  docker run --rm --network "$net" --memory 64m --cap-drop ALL --security-opt no-new-privileges --read-only \
    ${PROBE_ENV:+--env-file "$PROBE_ENV"} \
    -v "$SRC/hosting/kit/tools/wunet:/p/wunet:ro" -v "$SRC/hosting/build-realm/buildprobe.py:/p/buildprobe.py:ro" \
    --entrypoint python3 "$PY_IMG" -I /p/buildprobe.py "$@"
}
soap_env() {  # a 0600 env file with only the SOAP login (from signup.env), for the probe container
  local f="$DIR/soap.env"
  mkdir -p "$DIR"
  ( umask 077; grep -E '^SOAP_(USER|PASS)=' "$WRATH_DIR/signup.env" 2>/dev/null > "$f" ) || return 1
  [ -s "$f" ] || return 1
  echo "$f"
}
remove_ids() { rm -f "$PORTAL_DIR/build-realm.id" "$WRATH_DIR/build-realm.id"; }
stop_world_gently() {  # the world server stops with a countdown in game (server shutdown) when builders are on
  running "$WORLD_C" || return 0
  docker update --restart=no "$WORLD_C" >/dev/null 2>&1 || true
  local n="" waited=0
  if PROBE_ENV=$(soap_env); then
    n=$(PROBE_ENV="$PROBE_ENV" probe "$EDGE_NET" players "http://$WORLD_C:7878/" 2>/dev/null | sed -n 's/^players \([0-9]*\)$/\1/p')
  fi
  if [ -n "$n" ] && [ "$n" -gt 0 ] && [ "$WARN_SECS" -gt 0 ]; then
    note "$n builder(s) online: the world server counts down ${WARN_SECS}s in game (server shutdown), saving everyone"
    PROBE_ENV="$PROBE_ENV" probe "$EDGE_NET" soap "http://$WORLD_C:7878/" "server shutdown $WARN_SECS" >/dev/null 2>&1 || true
    while running "$WORLD_C" && [ "$waited" -le $((WARN_SECS + 120)) ]; do sleep "${WU_OFF_PAUSE:-10}"; waited=$((waited + ${WU_OFF_PAUSE:-10})); done
  else
    note "nobody online on $WORLD_C (or the count is unknown: ${n:-?}): stopping now"
  fi
  docker stop -t 120 "$WORLD_C" >/dev/null 2>&1 || true
  rm -f "$DIR/soap.env"
}
refresh_list() { [ -f "$WRATH_DIR/leyline-fetch.py" ] && { python3 -I "$WRATH_DIR/leyline-fetch.py" >/dev/null 2>&1 || true; }; }

# ---------------------------------------------------------------------------------------------------- off
if [ "$MODE" = --off ]; then
  log "Off: $NAME leaves the list, then stops (its databases and save files stay)"
  docker rm -f "$AGENT_C" >/dev/null 2>&1 && note "off the Leyline list ($AGENT_C gone)"
  remove_ids; refresh_list
  stop_world_gently
  docker rm -f "$WORLD_C" "$AUTH_C" >/dev/null 2>&1 || true
  note "$WORLD_C and $AUTH_C gone; $AUTH_DB, $CHARS_DB, $BOTS_DB and $DIR/dens kept (build-realm-on brings it back as it was)"
  echo; echo "BUILD REALM OFF"; exit 0
fi

# ---------------------------------------------------------------------------------------------------- checks
log "1/7 Checks"
for c in "$DB_C" "$MAIN_WORLD" "$MAIN_AUTH" wu-portal; do
  running "$c" || gate "$c is not running: the Builders' Realm sits beside the main realm, which must be up. Nothing was changed."
done
running wu-relay || gate "the relay (wu-relay) is not running: run the relay step first. Nothing was changed."
[ -f "$RELAY_DIR/relay.env" ] || gate "$RELAY_DIR/relay.env is missing (the relay step writes it). Nothing was changed."
BIND="$(envget "$RELAY_DIR/relay.env" RELAY_HTTP_BIND)"
RELAY_NAME="$(envget "$RELAY_DIR/relay.env" RELAY_NAME)"
RELAY_NAME="${RELAY_NAME:-relay.openvanilla.app}"
[[ "$BIND" =~ ^[0-9.]+:[0-9]+$ ]] || gate "RELAY_HTTP_BIND in $RELAY_DIR/relay.env is '$BIND', not address:port. Nothing was changed."
for n in "$BACKEND_NET" "$EDGE_NET" "$SITE_NET"; do
  docker network inspect "$n" >/dev/null 2>&1 || gate "Docker network $n not found. Nothing was changed."
done
docker volume inspect "$DATA_VOL" >/dev/null 2>&1 || gate "the client data volume $DATA_VOL is missing. Nothing was changed."
for c in "$WORLD_C" "$AUTH_C" "$AGENT_C"; do
  if exists "$c" && [ "$(docker inspect -f '{{index .Config.Labels "leyline.build"}}' "$c" 2>/dev/null)" != 1 ]; then
    gate "a container called $c is here and is not this step's. Nothing was changed."
  fi
done
WORLD_IMG=$(docker inspect -f '{{.Config.Image}}' "$MAIN_WORLD" 2>/dev/null || true)
AUTH_IMG=$(docker inspect -f '{{.Config.Image}}' "$MAIN_AUTH" 2>/dev/null || true)
note "images: world ${WORLD_IMG:-?}, login ${AUTH_IMG:-?} (the main realm's own)"
free_mb=$(awk '/MemAvailable/ {print int($2/1024)}' "${WU_MEMINFO:-/proc/meminfo}" 2>/dev/null || echo 0)
disk_mb=$(df -Pm "$WRATH_DIR" 2>/dev/null | awk 'NR==2 {print $4}')
main_mem=$(docker stats --no-stream --format '{{.Name}} {{.MemUsage}}' 2>/dev/null | awk -v w="$MAIN_WORLD" -v d="$DB_C" '$1==w || $1==d {print $1": "$2}' | tr '\n' ' ')
note "memory available: ${free_mb} MB (needs $MIN_FREE_MB: world cap $WORLD_MEM, login $AUTH_MEM, agent 64m); in use now: ${main_mem:-?}"
note "disk free in $WRATH_DIR: ${disk_mb:-?} MB (needs $MIN_DISK_MB)"
UP=0; running "$WORLD_C" && running "$AUTH_C" && UP=1
if [ "$UP" = 0 ]; then
  [ "${free_mb:-0}" -ge "$MIN_FREE_MB" ] || gate "only ${free_mb} MB of memory available; the Builders' Realm needs about $MIN_FREE_MB MB (WU_BUILD_MIN_FREE_MB). Nothing was changed."
fi
[ "${disk_mb:-0}" -ge "$MIN_DISK_MB" ] || gate "only ${disk_mb:-?} MB free in $WRATH_DIR. Nothing was changed."
[ -n "$PY_IMG" ] || gate "no python image (python:3.12-alpine or wu-hangout-tools:1) for the self-check. Nothing was changed."
# ov_den.lua is not in the scripts step's upload list (it only runs on a Den), so this step brings it (step 4);
# ov_build.lua comes with the scripts step and must already have the Den's save files.
if grep -q 'OVBuild.DenLayout' "$WRATH_DIR/custom/lua_scripts/ov_build.lua" 2>/dev/null; then
  note "the box's ov_build.lua has the Den's save files"
else
  gate "the box's ov_build.lua is older than the Den's save files: run the scripts step first (a live reload on the main realm, no restart). Nothing was changed."
fi
if grep -q 'leyline-den' "$WRATH_DIR/custom/lua_scripts/ov_den.lua" 2>/dev/null; then note "the box's ov_den.lua: there"
else note "the box's ov_den.lua: missing or old; this step installs it (it does nothing on the main realm: OV_DEN is off there)"; fi
for d in "$AUTH_DB" "$CHARS_DB" "$BOTS_DB"; do db_exists "$d" && note "database $d: there (kept)" || note "database $d: would be made"; done
note "ports: none published (players come through the relay $RELAY_NAME; the relay gives the realm its port pair)"
note "already running: $WORLD_C=$(running "$WORLD_C" && echo 1 || echo 0) $AUTH_C=$(running "$AUTH_C" && echo 1 || echo 0) $AGENT_C=$(running "$AGENT_C" && echo 1 || echo 0)"

if [ "$MODE" = --check ]; then
  # while it runs: the last lines of its servers' logs (read-only; nothing secret: those lines are dropped)
  for c in "$WORLD_C" "$AUTH_C" "$AGENT_C"; do
    if running "$c"; then
      echo; echo "== the last lines of $c"
      docker logs --tail 50 "$c" 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -viE 'password|passwd|secret|token|sessionkey' | sed 's/^/     /' || true
    fi
  done
  # the players' connections and logins (the last 50 lines are mostly the world's start-up and its creature noise)
  if running "$WORLD_C"; then
    echo; echo "== $WORLD_C: connections, logins and kicks (the last 6000 lines, filtered)"
    docker logs --tail 6000 "$WORLD_C" 2>&1 | sed 's/\x1b\[[0-9;]*m//g' \
      | grep -iE 'socket|auth|session|account|logged|login|kick|disconnect|reject|denied|realm|character|enum|addon|ban|version|build' \
      | grep -viE 'password|passwd|secret|token|sessionkey|config:|> using|loading|loaded' | tail -40 | sed 's/^/     /' || true
  fi
  # the world's own log files (SQL errors never reach the console: a failing login query just hangs the client)
  if [ -d "$DIR/logs" ]; then
    echo; echo "== $DIR/logs: files, and their last error lines"
    ls -la "$DIR/logs" 2>/dev/null | sed 's/^/     /' | head -20
    for f in "$DIR"/logs/*.log; do
      [ -f "$f" ] || continue
      n=$(grep -ciE 'error|fail|doesn.t exist|unknown column|denied' "$f" 2>/dev/null || true)
      echo "     -- $(basename "$f"): $n error lines; the last ones:"
      grep -iE 'error|fail|doesn.t exist|unknown column|denied' "$f" 2>/dev/null | grep -viE 'password|passwd|secret|token|sessionkey' \
        | tail -12 | cut -c1-300 | sed 's/^/        /' || true
    done
  fi
  if running "$AUTH_C"; then
    echo; echo "== $AUTH_C: logins (filtered)"
    docker logs --tail 3000 "$AUTH_C" 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -iE 'account|login|logon|reconnect|banned|unknown|fail|proof|realm' \
      | grep -viE 'password|passwd|secret|token|sessionkey|config:' | tail -20 | sed 's/^/     /' || true
  fi
  echo; echo "Would: back up acore_auth (and the build databases) to $DIR/backups/<time>/, make $AUTH_DB / $CHARS_DB / $BOTS_DB,"
  echo "start $AUTH_C and $WORLD_C (realm id $REALM_ID, no published port, the main realm untouched), check them and the"
  echo "main realm, then list \"$NAME\" on Leyline through the relay ($AGENT_C), and roll back on any failure."
  [ -n "$BLOCKED" ] && echo "The --go run would stop at the WOULD STOP lines above."
  echo; echo "Dry run finished: nothing changed. The build-realm-on step applies it."
  echo "BUILD REALM DRY"; exit 0
fi

# ---------------------------------------------------------------------------------------------------- backup
log "2/7 Backup"
STAMP=$(date +%Y%m%d-%H%M%S)
( umask 077; mkdir -p "$DIR/backups" "$DIR/beacon/state" ) || die "could not make $DIR"
chmod 711 "$DIR"; chmod 700 "$DIR/backups"
B="$DIR/backups/$STAMP"
( umask 077; mkdir -p "$B" )
( umask 077; dump --databases acore_auth | gzip > "$B/acore_auth.sql.gz" ) || die "the backup of acore_auth failed. Nothing was changed."
[ "$(stat -c %s "$B/acore_auth.sql.gz")" -gt "${WU_MIN_DUMP:-2000}" ] || die "the backup $B/acore_auth.sql.gz is suspiciously small. Nothing was changed."
MADE_DB=()
for d in "$AUTH_DB" "$CHARS_DB" "$BOTS_DB"; do
  if db_exists "$d"; then ( umask 077; dump --databases "$d" | gzip > "$B/$d.sql.gz" ) || die "the backup of $d failed. Nothing was changed."
  else MADE_DB+=("$d"); fi
done
for f in "$WRATH_DIR/profiles/build.env" "$WRATH_DIR/custom/lua_scripts/ov_buildrealm.lua" "$PORTAL_DIR/build-realm.id" "$WRATH_DIR/build-realm.id"; do
  [ -f "$f" ] && cp -p "$f" "$B/$(echo "${f#/}" | tr / _)"
done
tar -czf "$B/beacon-state.tgz" -C "$DIR/beacon" state 2>/dev/null || true
chmod 600 "$B"/* 2>/dev/null || true
ls -1d "$DIR"/backups/*/ 2>/dev/null | sort | head -n -5 | xargs -r rm -rf   # keep the last five
note "backup: $B"

MADE=()            # containers this run made
FILES_NEW=()       # files this run put where there was none
HAD_IMAGE=0; docker image inspect "$IMG" >/dev/null 2>&1 && { docker tag "$IMG" wu-build-tools:rollback; HAD_IMAGE=1; }
AGENT_WAS=0; running "$AGENT_C" && AGENT_WAS=1
start_agent() {
  docker rm -f "$AGENT_C" >/dev/null 2>&1 || true
  docker run -d --name "$AGENT_C" --restart unless-stopped --label leyline.build=1 --network "$SITE_NET" \
    --memory 64m --memory-swap 64m --cpus 0.5 --pids-limit 64 --read-only --tmpfs /tmp:size=8m --cap-drop ALL \
    --security-opt no-new-privileges --user "$UIDGID" -v "$DIR/beacon/state:/state" -e LOG_LEVEL=INFO \
    --log-opt max-size=10m --log-opt max-file=3 "$IMG" python -m wuhost.agent --state /state >/dev/null || return 1
  docker network connect "$EDGE_NET" "$AGENT_C" >/dev/null 2>&1 || return 1
}
rollback() {
  echo "  rolling back to $B (the main realm was not touched)" >&2
  local c d f
  docker rm -f "$AGENT_C" >/dev/null 2>&1 || true
  for c in "${MADE[@]}"; do docker rm -f "$c" >/dev/null 2>&1 || true; done
  for d in "${MADE_DB[@]}"; do echo "DROP DATABASE IF EXISTS \`$d\`;" | sql >/dev/null 2>&1 || true; done
  for f in "${FILES_NEW[@]}"; do rm -f "$f"; done
  for f in "$WRATH_DIR/profiles/build.env" "$WRATH_DIR/custom/lua_scripts/ov_buildrealm.lua" "$WRATH_DIR/custom/lua_scripts/ov_den.lua"; do
    [ -f "$B/$(echo "${f#/}" | tr / _)" ] && cp -p "$B/$(echo "${f#/}" | tr / _)" "$f"
  done
  remove_ids
  for f in "$PORTAL_DIR/build-realm.id" "$WRATH_DIR/build-realm.id"; do
    [ -f "$B/$(echo "${f#/}" | tr / _)" ] && cp -p "$B/$(echo "${f#/}" | tr / _)" "$f"
  done
  if [ -f "$B/beacon-state.tgz" ]; then
    rm -rf "$DIR/beacon/state.rollback"; mkdir -p "$DIR/beacon/state.rollback"
    tar -xzf "$B/beacon-state.tgz" -C "$DIR/beacon/state.rollback" && rm -rf "$DIR/beacon/state" && mv "$DIR/beacon/state.rollback/state" "$DIR/beacon/state"
    rm -rf "$DIR/beacon/state.rollback"
  fi
  [ "$HAD_IMAGE" = 1 ] && docker tag wu-build-tools:rollback "$IMG" >/dev/null 2>&1
  if [ "$AGENT_WAS" = 1 ] && [ "$HAD_IMAGE" = 1 ] && running "$WORLD_C"; then start_agent || true; fi
}
fail() { rollback; die "$1; rolled back. Backup: $B"; }

# ---------------------------------------------------------------------------------------------------- data
log "3/7 Databases"
DBUSER=$(docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$MAIN_WORLD" 2>/dev/null \
  | sed -n 's/^AC_LOGIN_DATABASE_INFO=[^;]*;[^;]*;\([^;]*\);.*/\1/p' | head -1)
[[ "$DBUSER" =~ ^[A-Za-z0-9_]{1,32}$ ]] || fail "the main world server's login database setting has no user name I can read"
{
  echo "CREATE DATABASE IF NOT EXISTS \`$AUTH_DB\` DEFAULT CHARACTER SET utf8mb4;"
  echo "CREATE DATABASE IF NOT EXISTS \`$CHARS_DB\` DEFAULT CHARACTER SET utf8mb4;"
  echo "CREATE DATABASE IF NOT EXISTS \`$BOTS_DB\` DEFAULT CHARACTER SET utf8mb4;"
} | sql >/dev/null || fail "could not make the build databases"
# The login database: every table of acore_auth as a view (the same accounts, bans, GM rights; the main realm's own
# ov_*/mod_* script tables are left out, so its go intents and passes stay its own), and a realm list of its own.
tables=$(echo "SELECT table_name FROM information_schema.tables WHERE table_schema = 'acore_auth' AND table_type = 'BASE TABLE';" | sql) \
  || fail "could not read acore_auth's tables"
echo "$tables" | grep -qx account || fail "acore_auth has no account table"
echo "$tables" | grep -qx realmlist || fail "acore_auth has no realmlist table"
{
  for t in $tables; do
    [[ "$t" =~ ^[A-Za-z0-9_]+$ ]] || continue
    case "$t" in realmlist|ov_*|mod_*) continue ;; esac
    echo "CREATE OR REPLACE SQL SECURITY INVOKER VIEW \`$AUTH_DB\`.\`$t\` AS SELECT * FROM \`acore_auth\`.\`$t\`;"
  done
  echo "CREATE TABLE IF NOT EXISTS \`$AUTH_DB\`.realmlist LIKE \`acore_auth\`.realmlist;"
  echo "INSERT IGNORE INTO \`$AUTH_DB\`.realmlist SELECT * FROM \`acore_auth\`.realmlist WHERE id = 1 AND NOT EXISTS (SELECT 1 FROM \`$AUTH_DB\`.realmlist WHERE id = $REALM_ID);"
  echo "UPDATE \`$AUTH_DB\`.realmlist SET id = $REALM_ID WHERE id = 1;"
  echo "DELETE FROM \`$AUTH_DB\`.realmlist WHERE id <> $REALM_ID;"
  printf "UPDATE \`%s\`.realmlist SET name = '%s', allowedSecurityLevel = 0 WHERE id = %s;\n" "$AUTH_DB" "${NAME//\'/\'\'}" "$REALM_ID"
} | sql >/dev/null || fail "could not make $AUTH_DB"
note "$AUTH_DB: $(echo "$tables" | grep -cvE '^(realmlist|ov_.*|mod_.*)$') views of acore_auth, and a realm list with \"$NAME\" (id $REALM_ID) only"
clone_schema() {  # clone_schema FROM TO: the tables (empty), and the rows of the small bookkeeping tables
  local from="$1" to="$2" n
  n=$(echo "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = '$to';" | sql) || return 1
  if [ "${n:-0}" != 0 ]; then
    note "$to: already has its tables (kept as they are)"
  else
    dump --no-data "$from" | sed -E 's/ AUTO_INCREMENT=[0-9]+//' | docker exec -i "$DB_C" sh -c 'MYSQL_PWD="${MYSQL_ROOT_PASSWORD:-}" exec mysql -uroot "$1"' sql "$to" || return 1
  fi
  # The rows the world server needs at start (2026-10-31 live: "active_arena_season can't be empty" stopped it),
  # topped up every run, so a database made by an earlier run gets them too.
  for t in updates updates_include worldstates active_arena_season; do
    if [ "$(echo "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = '$from' AND table_name = '$t';" | sql)" = 1 ]; then
      echo "INSERT IGNORE INTO \`$to\`.\`$t\` SELECT * FROM \`$from\`.\`$t\`;" | sql >/dev/null || return 1
    fi
  done
  note "$to: the tables of $from, empty (its update records and the arena season copied)"
}
clone_schema acore_characters "$CHARS_DB" || fail "could not make $CHARS_DB"
if db_exists acore_playerbots; then clone_schema acore_playerbots "$BOTS_DB" || fail "could not make $BOTS_DB"; fi
if [ "$DBUSER" != root ]; then
  hosts=$(echo "SELECT host FROM mysql.user WHERE user = '$DBUSER';" | sql)
  [ -n "$hosts" ] || fail "the database user $DBUSER is not in mysql.user"
  {
    for h in $hosts; do
      [[ "$h" =~ ^[A-Za-z0-9.%_:-]+$ ]] || continue
      for d in "$AUTH_DB" "$CHARS_DB" "$BOTS_DB"; do echo "GRANT ALL PRIVILEGES ON \`$d\`.* TO '$DBUSER'@'$h';"; done
    done
  } | sql >/dev/null || fail "could not give $DBUSER its rights on the build databases"
  note "rights on the build databases for the realm's database user"
fi

# ---------------------------------------------------------------------------------------------------- files
log "4/7 Files"
install_file() {  # install_file SRC DST: copied when different; a new file is remembered for the rollback
  [ -f "$2" ] || FILES_NEW+=("$2")
  cmp -s "$1" "$2" 2>/dev/null || { install -m 644 "$1" "$2" || return 1; note "$2 updated"; }
}
mkdir -p "$WRATH_DIR/profiles" "$WRATH_DIR/custom/lua_scripts" "$DIR/dens" "$DIR/logs"
install_file "$SRC/realm/profiles/build.env" "$WRATH_DIR/profiles/build.env" || fail "could not write profiles/build.env"
install_file "$SRC/realm/custom/lua_scripts/ov_buildrealm.lua" "$WRATH_DIR/custom/lua_scripts/ov_buildrealm.lua" || fail "could not write ov_buildrealm.lua"
[ -f "$WRATH_DIR/custom/lua_scripts/ov_den.lua" ] && [ ! -f "$B/$(echo "${WRATH_DIR#/}/custom/lua_scripts/ov_den.lua" | tr / _)" ] \
  && cp -p "$WRATH_DIR/custom/lua_scripts/ov_den.lua" "$B/$(echo "${WRATH_DIR#/}/custom/lua_scripts/ov_den.lua" | tr / _)"
install_file "$SRC/realm/custom/lua_scripts/ov_den.lua" "$WRATH_DIR/custom/lua_scripts/ov_den.lua" || fail "could not write ov_den.lua"
own=$(stat -c %u:%g "$WRATH_DIR/logs/world" 2>/dev/null || echo 1000:1000)
chown "$own" "$DIR/dens" "$DIR/logs" 2>/dev/null || true
chmod 750 "$DIR/dens" "$DIR/logs"
note "save files: $DIR/dens (the world server's own user, $own); logs: $DIR/logs"
# The world server's settings: the main one's AC_* (its database logins, data paths, modules) less its game switches,
# then build.env, then what makes it this realm. Written 0600, never printed.
( umask 077
  docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$MAIN_WORLD" > "$DIR/main.env.tmp" ) || fail "could not read $MAIN_WORLD's settings"
python3 -I - "$DIR/main.env.tmp" "$WRATH_DIR/profiles/build.env" "$DIR/world.env" "$DIR/auth.env" "$MAIN_AUTH" \
  "$REALM_ID" "$AUTH_DB" "$CHARS_DB" "$BOTS_DB" <<'PY' || { rm -f "$DIR/main.env.tmp"; fail "could not write the build realm's settings"; }
import os, re, subprocess, sys
main_env, profile, world_out, auth_out, auth_c, realm, auth_db, chars_db, bots_db = sys.argv[1:10]
KEY = re.compile(r"^([A-Z][A-Z0-9_]*)=(.*)$")
def read(lines):
    out = {}
    for ln in lines:
        m = KEY.match(ln.rstrip("\n"))
        if m:
            out[m.group(1)] = m.group(2)
    return out
def swap_db(v, db):   # "host;port;user;password;database" -> the same with another database
    parts = v.split(";")
    if len(parts) < 5:
        raise SystemExit("a database setting has an unexpected shape")
    parts[-1] = db
    return ";".join(parts)
base = read(open(main_env))
w = {k: v for k, v in base.items() if k.startswith("AC_") and not k.startswith("AC_PLAYERBOTS_PREMADE")}
w.update({k: v for k, v in base.items() if k in ("PATH", "LD_LIBRARY_PATH", "HOME", "TZ")})
for k in [k for k in w if re.match(r"^AC_(RATE_|SKILL_GAIN|START_|PLAYER_START_|ARENA_|BATTLEGROUND_|CHAT_|INDIVIDUAL_|AUTO_BALANCE|DUNGEON_)", k)]:
    del w[k]
w.update(read(l for l in open(profile) if not l.lstrip().startswith("#")))
for k, db in (("AC_LOGIN_DATABASE_INFO", auth_db), ("AC_CHARACTER_DATABASE_INFO", chars_db), ("AC_PLAYERBOTS_DATABASE_INFO", bots_db)):
    if k in w:
        w[k] = swap_db(w[k], db)
for k in ("AC_LOGIN_DATABASE_INFO", "AC_CHARACTER_DATABASE_INFO", "AC_WORLD_DATABASE_INFO"):
    if k not in w:
        raise SystemExit(f"{k} is not in the main world server's settings")
w.update({"AC_REALM_ID": realm, "AC_WORLD_SERVER_PORT": "8085", "AC_SOAP_ENABLED": "1", "AC_SOAP_IP": "0.0.0.0",
          "AC_SOAP_PORT": "7878", "AC_WARDEN_ENABLED": "0", "AC_UPDATES_ENABLE_DATABASES": "0", "AC_UPDATES_AUTO_SETUP": "0",
          "AC_MAX_CORE_STUCK_TIME": "60", "AC_VMAP_ENABLE_LOS": "0", "AC_VMAP_ENABLE_HEIGHT": "0", "AC_VMAP_PET_LOS": "0",
          "AC_MOVE_MAPS_ENABLE": "0", "AC_CLOSE_IDLE_CONNECTIONS": "0", "AC_DISABLE_INTERACTIVE": "1"})
old = os.umask(0o077)
try:
    with open(world_out, "w") as f:
        f.write("".join(f"{k}={v}\n" for k, v in w.items()))
    a = read(subprocess.run(["docker", "inspect", "-f", "{{range .Config.Env}}{{println .}}{{end}}", auth_c],
                            capture_output=True, text=True, check=True).stdout.splitlines())
    if "AC_LOGIN_DATABASE_INFO" not in a:
        raise SystemExit("the main login server has no AC_LOGIN_DATABASE_INFO")
    a = {k: v for k, v in a.items() if not k.startswith("AC_WRONG_PASS_")}
    a["AC_LOGIN_DATABASE_INFO"] = swap_db(a["AC_LOGIN_DATABASE_INFO"], auth_db)
    a.update({"AC_WRONG_PASS_MAX_COUNT": "0", "AC_WRONG_PASS_LOGGING": "1", "AC_UPDATES_ENABLE_DATABASES": "0"})
    with open(auth_out, "w") as f:
        f.write("".join(f"{k}={v}\n" for k, v in a.items()))
finally:
    os.umask(old)
print("settings written")
PY
rm -f "$DIR/main.env.tmp"
chmod 600 "$DIR/world.env" "$DIR/auth.env"
note "settings: $DIR/world.env and $DIR/auth.env (0600; the database logins are the main realm's own)"

# ---------------------------------------------------------------------------------------------------- start
log "5/7 Start $AUTH_C and $WORLD_C"
MAIN_RC=$(docker inspect -f '{{.RestartCount}}' "$MAIN_WORLD" 2>/dev/null || echo "?")
MAIN_START=$(docker inspect -f '{{.State.StartedAt}}' "$MAIN_WORLD" 2>/dev/null || echo "?")
common=(--restart unless-stopped --label leyline.build=1 --security-opt no-new-privileges --log-opt max-size=20m --log-opt max-file=3)
stop_world_gently   # a re-run (new settings): builders online get the in-game countdown first
docker rm -f "$AUTH_C" "$WORLD_C" >/dev/null 2>&1 || true
docker run -d -t --name "$AUTH_C" "${common[@]}" --network "$BACKEND_NET" --env-file "$DIR/auth.env" -w /azerothcore \
  --memory "$AUTH_MEM" --memory-swap "$AUTH_MEM" "$AUTH_IMG" >/dev/null || fail "$AUTH_C did not start"
MADE+=("$AUTH_C")
docker network connect "$EDGE_NET" "$AUTH_C" >/dev/null 2>&1 || fail "could not put $AUTH_C on $EDGE_NET"
docker run -d -t -i --name "$WORLD_C" "${common[@]}" --network "$BACKEND_NET" --env-file "$DIR/world.env" -w /azerothcore \
  --memory "$WORLD_MEM" --memory-swap "$WORLD_MEM" --ulimit nofile=65535:65535 --stop-timeout 120 \
  --mount "type=volume,src=$DATA_VOL,dst=/azerothcore/env/dist/data,readonly" \
  --mount "type=bind,src=$WRATH_DIR/custom/lua_scripts,dst=/azerothcore/lua_scripts,readonly" \
  --mount "type=bind,src=$WRATH_DIR/modules-sql/mod-playerbots,dst=/azerothcore/modules/mod-playerbots/data/sql,readonly" \
  --mount "type=bind,src=$DIR/logs,dst=/azerothcore/env/dist/logs" \
  --mount "type=bind,src=$DIR/dens,dst=/azerothcore/dens" \
  "$WORLD_IMG" >/dev/null || fail "$WORLD_C did not start"
MADE+=("$WORLD_C")
docker network connect "$EDGE_NET" "$WORLD_C" >/dev/null 2>&1 || fail "could not put $WORLD_C on $EDGE_NET"
note "started: $AUTH_C ($AUTH_MEM), $WORLD_C ($WORLD_MEM cap), on $BACKEND_NET + $EDGE_NET, nothing published"

# ---------------------------------------------------------------------------------------------------- health
log "6/7 Self-check"
ok=""
for _ in $(seq 1 "$TRIES"); do
  if out=$(probe "$EDGE_NET" logon "$AUTH_C" 3724 2>&1); then ok=1; break; fi
  running "$AUTH_C" || break
  sleep "$PAUSE"
done
[ -n "$ok" ] || { docker logs --tail 30 "$AUTH_C" >&2 2>&1 || true; fail "the login server ($AUTH_C) does not answer a logon challenge (${out:-no answer})"; }
note "login server: $out"
# by the clock: each probe itself can take up to ~20 s, so counting only the pauses made a 15-minute wait last hours
ok=""; waited=0; t0=$SECONDS
while [ "$waited" -le "$WORLD_WAIT" ]; do
  if out=$(probe "$EDGE_NET" world "$WORLD_C" 8085 2>&1); then ok=1; break; fi
  running "$WORLD_C" || break
  sleep "$PAUSE"; waited=$((SECONDS - t0))
done
[ -n "$ok" ] || { docker logs --tail 40 "$WORLD_C" >&2 2>&1 || true; fail "the world server ($WORLD_C) did not greet within ${WORLD_WAIT}s (${out:-no answer})"; }
note "world server: $out (after about ${waited}s)"
# Restarts before the first greeting (2026-10-31 live: 3, then a steady greeting) are noted with the log's last lines;
# only a restart, stop or out-of-memory after it greets fails the step.
r0=$(docker inspect -f '{{.RestartCount}}' "$WORLD_C" 2>/dev/null || echo 0)
if [ "${r0:-0}" != 0 ]; then
  note "!! the world server restarted $r0 time(s) while starting; its last lines:"
  docker logs --tail 40 "$WORLD_C" 2>&1 | grep -viE 'password|secret|token' | sed 's/^/     /' >&2 || true
fi
sleep "$SETTLE"
rc=$(docker inspect -f '{{.RestartCount}} {{.State.Running}} {{.State.OOMKilled}}' "$WORLD_C" 2>/dev/null)
if [ "$rc" != "${r0:-0} true false" ]; then
  docker logs --tail 40 "$WORLD_C" 2>&1 | grep -viE 'password|secret|token' | sed 's/^/     /' >&2 || true
  fail "the world server restarted, stopped or ran out of memory after it greeted (restarts, running, out of memory: $rc; it had $r0 at the greeting; WU_BUILD_WORLD_MEM raises the cap)"
fi
used=$(docker stats --no-stream --format '{{.Name}} {{.MemUsage}}' 2>/dev/null | awk -v w="$WORLD_C" '$1==w {print $2}')
note "memory in use: $WORLD_C ${used:-?} of $WORLD_MEM"
out=$(probe "$EDGE_NET" world "$MAIN_WORLD" 8085 2>&1) || fail "the main realm's world server does not greet any more ($out)"
[ "$(docker inspect -f '{{.RestartCount}}' "$MAIN_WORLD" 2>/dev/null)" = "$MAIN_RC" ] \
  && [ "$(docker inspect -f '{{.State.StartedAt}}' "$MAIN_WORLD" 2>/dev/null)" = "$MAIN_START" ] \
  || fail "the main realm's world server restarted during this step"
out2=$(probe "$EDGE_NET" logon "$MAIN_AUTH" 3724 2>&1) || fail "the main realm's login server does not answer any more ($out2)"
note "main realm: world $out, login $out2, not restarted"

# ---------------------------------------------------------------------------------------------------- list
log "7/7 On the Leyline list"
docker build -q -t "$IMG" "$SRC/hosting/kit/tools" >/dev/null || fail "building the agent image failed"
chown -R "$UIDGID" "$DIR/beacon/state" 2>/dev/null || true
chmod 711 "$DIR/beacon"; chmod 700 "$DIR/beacon/state"
out=$(docker run --rm --user "$UIDGID" --network none -v "$DIR/beacon:/kit" -e WU_KIT=/kit "$IMG" python -m wuhost.setup official \
  --name "$NAME" --relay "$RELAY_NAME" --relay-url "ws://$BIND/agent" --listing-url "http://$BIND/api/listing/heartbeat" \
  --auth-service "$AUTH_C:3724" --world-service "$WORLD_C:8085" --ruleset custom --tags custom --max-players 20 \
  --tagline "$TAGLINE" --summary "$SUMMARY" 2>&1) || { echo "$out" >&2; fail "writing the listing config failed"; }
RID=$(python3 -I -c 'import json,sys;print(json.loads(sys.stdin.read().strip().splitlines()[-1])["realm_id"])' <<<"$out" 2>/dev/null || true)
[[ "$RID" =~ ^[a-z2-7]{16}$ ]] || { echo "$out" >&2; fail "the listing config did not give a realm id"; }
[ "$(stat -c %a "$DIR/beacon/state/realm.key" 2>/dev/null || echo 600)" = 600 ] || fail "beacon/state/realm.key is not 0600"
rm -f "$DIR/beacon/state/relay-state.json"
start_agent || fail "the agent ($AGENT_C) did not start"
ok=""
for _ in $(seq 1 "$TRIES"); do
  [ "$(jget "$DIR/beacon/state/relay-state.json" connected)" = true ] && { ok=1; break; }
  sleep "$PAUSE"
done
[ -n "$ok" ] || { docker logs --tail 30 "$AGENT_C" >&2 2>&1 || true; fail "the agent did not connect to the relay ($(jget "$DIR/beacon/state/relay-state.json" last_error))"; }
online=$(curl -fsS -m 3 "http://$BIND/api/realms/$RID" 2>/dev/null | python3 -I -c 'import json,sys;print(json.load(sys.stdin).get("online"))' 2>/dev/null || true)
[ "$online" = True ] || fail "the relay does not see realm $RID online"
listed=""
for _ in $(seq 1 "$TRIES"); do
  curl -fsS -m 3 "http://$BIND/api/listing/realms.json" 2>/dev/null | grep -q "\"realmId\": *\"$RID\"" && { listed=1; break; }
  sleep "$PAUSE"
done
[ -n "$listed" ] || fail "the Leyline listing does not show it ($(jget "$DIR/beacon/state/relay-state.json" listing_reply))"
note "listed: realm $RID on $RELAY_NAME"
# The realm list row: the relay's address and this realm's world port there, so desktop players (the launcher,
# realmlist <relay>:<auth port>) come back through the relay; the browser game's hop bridge ignores the port.
raddr=$(jget "$DIR/beacon/state/relay-state.json" address); rport=$(jget "$DIR/beacon/state/relay-state.json" world_port)
ip=$(getent ahostsv4 "$raddr" 2>/dev/null | awk 'NR==1 {print $1}')
if [[ "$ip" =~ ^[0-9.]+$ ]] && [[ "$rport" =~ ^[0-9]+$ ]]; then
  echo "UPDATE \`$AUTH_DB\`.realmlist SET address = '$ip', localAddress = '$ip', port = $rport WHERE id = $REALM_ID;" | sql >/dev/null \
    || fail "could not set the realm list address"
  note "realm list: $raddr ($ip):$rport (through the relay)"
else
  fail "the relay gave no address or world port (${raddr:-?}:${rport:-?})"
fi
mkdir -p "$PORTAL_DIR"
for f in "$PORTAL_DIR/build-realm.id" "$WRATH_DIR/build-realm.id"; do
  [ -f "$f" ] || FILES_NEW+=("$f")
  printf '%s\n' "$RID" > "$f.tmp" && chmod 644 "$f.tmp" && mv "$f.tmp" "$f"
done
note "$PORTAL_DIR/build-realm.id: hops from the play page keep the player's own login"
refresh_list
echo
echo "$NAME: realm $RID on $RELAY_NAME (https://$RELAY_NAME/r/$RID/status.json); id $REALM_ID in its own realm list"
echo "Logs: docker logs -f $WORLD_C (and $AUTH_C, $AGENT_C)    Off: the build-realm-off step    Backup: $B"
echo "BUILD REALM ON"
