#!/usr/bin/env bash
# Leyline: bring a server you already run onto the Leyline network, with Hero Key sign-in.
# For AzerothCore and TrinityCore servers of the Wrath client (others: not yet; use "Link my server" to list them without it).
# Guide: https://play.openvanilla.app/wrath/docs/join-network
#
#   bash join-network.sh            a few questions, then your server is listed and takes Hero Key sign-in
#   bash join-network.sh test       Test Hero Key sign-in: a full dry run, nothing kept
#   bash join-network.sh status     is the helper connected and listed?
#   bash join-network.sh stop       stop the helper (your server shows offline, then leaves the list)
#   bash join-network.sh remove     take the helper away (the realm id stays in state/ for next time)
#
# Works on its own (download just this file) or inside the Host pack folder. Needs Docker. It runs one small
# helper (container wu-link-agent) next to your server: it talks out to the Leyline relay, lists your server, passes
# the relay's players on to your login server, and makes an account for each Hero Key player (named HK + 14
# letters) in your login database. It never changes anything else: not your realm list, other accounts or files.
# The database password is asked once, kept only in state/link-db.env (readable by you alone) and never printed.
#
# Answers can come from the environment: LEYLINE_NAME, LEYLINE_CORE (azerothcore | trinitycore), LEYLINE_AUTH,
# LEYLINE_WORLD, LEYLINE_DB_HOST, LEYLINE_DB_PORT, LEYLINE_DB_USER, LEYLINE_DB_PASSWORD, LEYLINE_AUTH_DB,
# LEYLINE_CHARS_DB, LEYLINE_TAGLINE.
set -euo pipefail
cd "$(dirname "$0")"
DIR="$(pwd)"
PREBUILT="${WU_TOOLS_PREBUILT:-ghcr.io/orionopenwow/wu-host-tools:1}"
IMAGE="${WU_TOOLS_IMAGE:-wu-host-tools:1}"
RELAY="${WU_RELAY:-relay.openvanilla.app}"
LC=wu-link-agent

say() { printf '%s\n' "$*"; }
die() { say "$*" >&2; exit 1; }
command -v docker >/dev/null 2>&1 || die "Docker is not installed: install it (docs.docker.com/engine/install), then run this again."
docker version >/dev/null 2>&1 || die "Docker is not running (or this user may not use it): start it, or run with sudo."

image() {
  docker pull -q "$PREBUILT" >/dev/null 2>&1 && docker tag "$PREBUILT" "$IMAGE" || true
  if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    [ -f tools/Dockerfile ] || die "Could not fetch the helper image ($PREBUILT). Is this PC online?"
    say "  Building the small helper image (once, about a minute)..."
    docker build -q -t "$IMAGE" tools >/dev/null || die "Building the helper image failed."
  fi
}
tools() { docker run --rm --network host -v "$DIR:/kit" -e WU_KIT=/kit "$@"; }
start_agent() {
  docker rm -f "$LC" >/dev/null 2>&1 || true
  local envf=()
  [ -f state/link-db.env ] && envf=(--env-file state/link-db.env)
  docker run -d --name "$LC" --restart unless-stopped --network host --read-only --cap-drop ALL \
    --security-opt no-new-privileges --memory 96m --tmpfs /tmp --log-opt max-size=10m --log-opt max-file=3 \
    "${envf[@]}" -v "$DIR/state:/state" "$IMAGE" python -m wuhost.agent --state /state >/dev/null \
    || die "The helper did not start: docker logs $LC"
}
selftest() {
  docker ps --filter "name=^${LC}$" --format '{{.Names}}' | grep -q . || die "The helper is not running: run this file first."
  say "== Test Hero Key sign-in (a dry run: nothing is kept)"
  docker exec "$LC" python -m wuhost.hktest --state /state
}

case "${1:-}" in
  test) selftest; exit $? ;;
  status)
    [ -f state/host.json ] || die "Nothing joined yet: run this file."
    docker ps -a --filter "name=^${LC}$" --format 'Helper: {{.Status}}'
    grep -q '"connected": true' state/relay-state.json 2>/dev/null && say "Relay:  connected" || say "Relay:  not connected yet"
    tools "$IMAGE" python -m wuhost.setup herokey status
    exit 0 ;;
  stop) docker stop "$LC" >/dev/null 2>&1 || true; say "Stopped. Your server shows offline at once and leaves the list after 6 hours."; exit 0 ;;
  remove) docker rm -f "$LC" >/dev/null 2>&1 || true; say "The helper is gone. The realm id stays in state/ (realm.key): run this file again to come back as the same realm."; exit 0 ;;
  "") ;;
  *) die "Usage: bash join-network.sh [test | status | stop | remove]" ;;
esac

if [ -f state/host.json ] && ! grep -q '"linked": true' state/host.json; then
  die "This folder already hosts a Leyline realm. Use a fresh folder to join a server you already run."
fi
mkdir -p state
say ""
say "== Helper image"
image

ask() {   # ask VAR "question" default
  local cur="${!1:-}"
  if [ -z "$cur" ]; then
    read -r -p "$2 [$3]: " cur || true
    cur="${cur:-$3}"
  fi
  printf -v "$1" '%s' "$cur"
}
say ""
say "== Your server"
ask LEYLINE_NAME "1. Its name on the realm list" "My Realm"
ask LEYLINE_CORE "2. Its core: azerothcore or trinitycore" "azerothcore"
case "$LEYLINE_CORE" in
  azerothcore) DA=acore_auth; DC=acore_characters ;;
  trinitycore) DA=auth; DC=characters ;;
  *) die "$LEYLINE_CORE: not yet. Hero Key sign-in is for AzerothCore and TrinityCore servers of the Wrath client for now." ;;
esac
ask LEYLINE_AUTH "3. Its login server, as this PC reaches it" "127.0.0.1:3724"
ask LEYLINE_WORLD "4. Its world server, as this PC reaches it" "127.0.0.1:8085"
say ""
say "== Its login database (for Hero Key accounts)"
ask LEYLINE_DB_HOST "5. Database address" "127.0.0.1"
ask LEYLINE_DB_PORT "6. Database port" "3306"
ask LEYLINE_DB_USER "7. Database user (one that may add accounts)" "root"
if [ -z "${LEYLINE_DB_PASSWORD:-}" ]; then
  read -r -s -p "8. Its password (not shown): " LEYLINE_DB_PASSWORD || true
  say ""
fi
ask LEYLINE_AUTH_DB "9. The login database's name" "$DA"
ask LEYLINE_CHARS_DB "10. The characters database's name" "$DC"
EXTRA=()
[ -n "${LEYLINE_TAGLINE:-}" ] && EXTRA+=(--tagline "$LEYLINE_TAGLINE")

say ""
say "== Checking and joining"
WU_JOIN_DB_PASSWORD="$LEYLINE_DB_PASSWORD" tools -e WU_JOIN_DB_PASSWORD "$IMAGE" python -m wuhost.setup join-network \
  --name "$LEYLINE_NAME" --core "$LEYLINE_CORE" --auth-service "$LEYLINE_AUTH" --world-service "$LEYLINE_WORLD" \
  --db-host "$LEYLINE_DB_HOST" --db-port "$LEYLINE_DB_PORT" --db-user "$LEYLINE_DB_USER" \
  --auth-db "$LEYLINE_AUTH_DB" --chars-db "$LEYLINE_CHARS_DB" --relay "$RELAY" "${EXTRA[@]}" >/dev/null \
  || die "Joining was refused (see above). Nothing was changed on your server."
chmod 600 state/link-db.env 2>/dev/null || true

say ""
say "== Starting the helper"
start_agent
for _ in $(seq 1 30); do
  grep -q '"connected": true' state/relay-state.json 2>/dev/null && { say "  Connected to the Leyline relay."; break; }
  sleep 2
done
sleep 3
selftest || say "  Some steps of the test did not pass yet: each line above says what to do. Run: bash join-network.sh test"

RID="$(sed -n 's/.*"realm_id": *"\([a-z2-7]*\)".*/\1/p' state/host.json | head -1)"
say ""
say "== Done"
say "  Your server is on the Leyline realm list within a minute: https://play.openvanilla.app/wrath/r/$RID"
say "  - Players arrive through the relay at your login server; the realm address your server hands out must already"
say "    work for players. Leyline leaves your realm list alone."
say "  - Relay players all come from the helper's address: set WrongPass.MaxCount = 0 in authserver.conf (the relay"
say "    limits wrong passwords per player instead)."
say "  - Hero Key players get an account named HK + 14 letters the first time they sign in. You can ban one like any"
say "    other account."
say "  The helper restarts by itself with Docker."
