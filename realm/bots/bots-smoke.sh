#!/usr/bin/env bash
# The bot smoke test (docs/BOTS-UPDATES.md): a throwaway realm from the images under test, a headless player
# (bots_smoke.py, no game client), the bot checks, then everything removed again.
#
#   realm/bots/bots-smoke.sh [--image ghcr.io/orionopenwow/wu-ac-wotlk] [--tag canary] [--playerbots-src DIR]
#                            [--ale-src DIR] [--out report.json] [--keep] [--data-volume ov-client-data]
#
# Needs Docker and the dbc + maps volume extracted from YOUR OWN game folder (realm\1-extract.bat makes
# ov-client-data). It is only read; nothing downloads or ships game data. Runs where that volume is: the owner's PC
# (Control step bots-smoke) or a self-hosted runner that has it (bots-canary.yml, BOTS_SMOKE_RUNNER).
# With --playerbots-src the static guard (compat.py) runs first and is part of the report.
# No password is printed: the database's and the console's are random, kept in a 600 temp folder, removed at the end.
# Exit 0 = every check passed.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
IMAGE="${SMOKE_IMAGE:-ghcr.io/orionopenwow/wu-ac-wotlk}"
TAG="${SMOKE_TAG:-canary}"
DATA="${SMOKE_DATA_VOLUME:-ov-client-data}"
OUT=""; PB_SRC=""; ALE_SRC=""; KEEP=0
while [ $# -gt 0 ]; do
  case "$1" in
    --image) IMAGE="$2"; shift 2 ;;
    --tag) TAG="$2"; shift 2 ;;
    --data-volume) DATA="$2"; shift 2 ;;
    --playerbots-src) PB_SRC="$2"; shift 2 ;;
    --ale-src) ALE_SRC="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --keep) KEEP=1; shift ;;
    -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
PY="${PYTHON:-python3}"
P=wu-bots-smoke
COMPOSE=(docker compose -p "$P" -f "$HERE/docker-compose.smoke.yml")

if ! docker volume inspect "$DATA" >/dev/null 2>&1; then
  echo "bots smoke: no '$DATA' volume here. It holds the dbc + maps extracted from your own game folder"
  echo "(realm\\1-extract.bat). The smoke test runs only where that volume is; nothing is downloaded."
  exit 3
fi

WORK="$(mktemp -d -t wu-bots-smoke-XXXX)"
chmod 700 "$WORK"
cleanup() {
  local rc=$?
  if [ "$rc" != 0 ] || [ "${SMOKE_LOGS:-0}" = 1 ]; then
    echo "== worldserver, last lines"
    "${COMPOSE[@]}" --env-file "$WORK/.env" logs --no-color --tail 80 ac-worldserver 2>/dev/null | grep -v -i 'password' || true
  fi
  if [ "$KEEP" = 1 ]; then echo "kept: $WORK (docker compose -p $P)"; return; fi
  "${COMPOSE[@]}" --env-file "$WORK/.env" down -v --remove-orphans >/dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT

# the realm scripts the bots need, and the probe (test only)
LUA="$WORK/lua"
mkdir -p "$LUA"
for f in ov_companions.lua ov_companions_dungeons.lua ov_modapi.lua; do
  cp "$HERE/../custom/lua_scripts/$f" "$LUA/"
done
cp "$HERE/smoke/ov_bots_probe.lua" "$LUA/"
chmod -R a+rX "$LUA"

umask 077
{
  echo "SMOKE_DB_PASSWORD=$("$PY" -c 'import secrets;print(secrets.token_hex(16))')"
  echo "SMOKE_IMAGE=$IMAGE"
  echo "SMOKE_TAG=$TAG"
  echo "SMOKE_DATA_VOLUME=$DATA"
  echo "SMOKE_LUA_DIR=$LUA"
} > "$WORK/.env"
umask 022

STATIC_ARGS=()
if [ -n "$PB_SRC" ]; then
  echo "== static guard (compat.py)"
  set +e
  "$PY" "$HERE/compat.py" --playerbots "$PB_SRC" ${ALE_SRC:+--ale "$ALE_SRC"}
  "$PY" "$HERE/compat.py" --playerbots "$PB_SRC" ${ALE_SRC:+--ale "$ALE_SRC"} --json > "$WORK/static.json"
  set -e
  STATIC_ARGS=(--static "$WORK/static.json")
fi

echo "== realm from $IMAGE-*:$TAG"
"${COMPOSE[@]}" --env-file "$WORK/.env" pull -q ac-db-import ac-authserver ac-worldserver 2>/dev/null || true
"${COMPOSE[@]}" --env-file "$WORK/.env" up -d --wait ac-database
"${COMPOSE[@]}" --env-file "$WORK/.env" up -d ac-db-import ac-authserver ac-worldserver

echo "== the console's own login (random, never printed)"
for i in $(seq 1 120); do
  "${COMPOSE[@]}" --env-file "$WORK/.env" exec -T ac-database sh -c 'mysql -uroot -p"$MYSQL_ROOT_PASSWORD" -N -e "SELECT 1 FROM acore_auth.realmlist LIMIT 1" 2>/dev/null' | grep -q 1 && break
  sleep 10
done
"$PY" "$HERE/bots_smoke.py" admin-sql --creds "$WORK/creds" | \
  "${COMPOSE[@]}" --env-file "$WORK/.env" exec -T ac-database sh -c 'mysql --force -uroot -p"$MYSQL_ROOT_PASSWORD" acore_auth 2>/dev/null' || true

echo "== the player"
set -a; . "$WORK/creds"; set +a
set +e
"$PY" "$HERE/bots_smoke.py" run --auth "127.0.0.1:${SMOKE_AUTH_PORT:-13724}" --world "127.0.0.1:${SMOKE_WORLD_PORT:-13785}" \
  --soap "127.0.0.1:${SMOKE_SOAP_PORT:-17878}" --out "$WORK/report.json" "${STATIC_ARGS[@]}"
RC=$?
set -e
unset SMOKE_SOAP_USER SMOKE_SOAP_PASS
if [ -n "$OUT" ] && [ -f "$WORK/report.json" ]; then cp "$WORK/report.json" "$OUT"; fi
exit $RC
