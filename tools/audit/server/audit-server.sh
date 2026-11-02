#!/usr/bin/env bash
# The release audit's server side (tools/audit/server.py sends this over the one SSH login, docs/AUDIT.md).
#
#   bash audit-server.sh checks     read only: the world server (up, restarts, out-of-memory kills), the Lua scripts
#                                   and their errors, the slice health (stream/server/slice-health.sh, passed in as
#                                   SLICE_HEALTH_B64), the relay, the GM console and the bug manifest. One
#                                   "AUDIT <key> <status> <detail>" line per check (status ok|bad|skip).
#   bash audit-server.sh account    the audit's own test account: AUDIT made, or its password changed, to a fresh
#                                   random one (over SOAP, as soap-cmd.sh does). The password is printed once on
#                                   the line "AUDIT_SECRET <password>" for the PC side, which keeps it in memory and
#                                   never shows or logs it; on the server it is kept only in /root/.leyline-audit
#                                   (chmod 600). Nothing else is touched: never another account.
# Last line: "AUDIT DONE".
set -uo pipefail
MODE="${1:-checks}"
WU="${WU_DIR:-/srv/wrath}"
WORLD="${WU_WORLD_CT:-ov-ac-worldserver}"
ACCOUNT="AUDIT"
say() { printf 'AUDIT %s %s %s\n' "$1" "$2" "$(printf '%s' "${3:-}" | tr '\n\r' '  ' | cut -c1-400)"; }

soap() {   # one console command over SOAP; the login from signup.env goes to curl on a file descriptor, never argv
  local cmd="$1" body resp code
  [ -f "$WU/signup.env" ] || { echo "no $WU/signup.env"; return 2; }
  set -a; . "$WU/signup.env"; set +a
  body="<?xml version=\"1.0\" encoding=\"utf-8\"?><SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\" xmlns:ns1=\"urn:AC\"><SOAP-ENV:Body><ns1:executeCommand><command>${cmd}</command></ns1:executeCommand></SOAP-ENV:Body></SOAP-ENV:Envelope>"
  resp=$(curl -sS --max-time 20 -K /dev/fd/3 -H 'Content-Type: text/xml; charset=utf-8' --data-binary @- -w '\n%{http_code}' \
    "${SOAP_URL:-http://127.0.0.1:7878/}" <<<"$body" 3<<<"user = \"${SOAP_USER}:${SOAP_PASS}\"") || return 2
  code="${resp##*$'\n'}"
  printf '%s' "${resp%$'\n'*}" | python3 -I -c '
import sys, xml.etree.ElementTree as ET
try:
    root = ET.fromstring(sys.stdin.read())
except ET.ParseError:
    sys.exit(0)
for el in root.iter():
    if el.tag.rsplit("}", 1)[-1] in ("result", "faultstring") and el.text:
        print(el.text.strip()); break'
  [ "$code" = 200 ]
}

if [ "$MODE" = "account" ]; then
  umask 077
  PW=$(python3 -I -c 'import secrets, string; a = string.ascii_uppercase + string.digits; print("".join(secrets.choice(a) for _ in range(16)))')
  if out=$(soap "account create $ACCOUNT $PW"); then
    how="made"
  elif out=$(soap "account set password $ACCOUNT $PW $PW"); then
    how="password changed"
  else
    say account bad "SOAP refused both account create and set password: $(printf '%s' "$out" | head -c 160)"
    echo "AUDIT DONE"; exit 1
  fi
  # never more than a player: no GM level for this account
  soap "account set gmlevel $ACCOUNT 0 -1" >/dev/null 2>&1 || true
  mkdir -p /root/.leyline-audit && chmod 700 /root/.leyline-audit
  printf '%s\n' "$PW" > /root/.leyline-audit/password && chmod 600 /root/.leyline-audit/password
  say account ok "$ACCOUNT $how (a fresh password, kept in /root/.leyline-audit)"
  echo "AUDIT_SECRET $PW"
  unset PW
  echo "AUDIT DONE"; exit 0
fi

cd "$WU" 2>/dev/null || { say world bad "no $WU on this server"; echo "AUDIT DONE"; exit 1; }

# ---- the world server
st=$(docker inspect -f '{{.State.Status}} {{.RestartCount}} {{.State.OOMKilled}} {{.State.StartedAt}}' "$WORLD" 2>/dev/null)
if [ -z "$st" ]; then
  say world bad "no container $WORLD"
else
  read -r status restarts oom started <<<"$st"
  age=$(( $(date +%s) - $(date -d "$started" +%s 2>/dev/null || date +%s) ))
  if [ "$status" != running ]; then say world bad "the world server is $status (restarts $restarts)"
  elif [ "$oom" = true ]; then say world bad "the world server was killed for memory (OOM) and restarted"
  else say world ok "running for $((age / 3600)) h $(( (age % 3600) / 60 )) min; restarts since it was made: $restarts"
  fi
  if [ "$status" = running ] && [ "$restarts" -gt 0 ] && [ "$age" -lt 3600 ]; then
    say world_restart bad "restarted $((age / 60)) min ago (restarts: $restarts): the worldlog step says why"
  else
    say world_restart ok "no restart in the last hour"
  fi
fi
oomk=$( (journalctl -k --since "-24h" 2>/dev/null || dmesg 2>/dev/null) | grep -ciE 'out of memory|oom-kill|killed process' || true)
if [ "${oomk:-0}" -gt 0 ]; then say oom bad "$oomk out-of-memory lines in the kernel log in the last 24 h"
else say oom ok "no out-of-memory kills in the last 24 h"; fi
soapinfo=$(soap "server info" 2>&1 | head -3 | tr '\n' ' ')
if printf '%s' "$soapinfo" | grep -qiE 'uptime|players|connected'; then say soap ok "$(printf '%s' "$soapinfo" | cut -c1-200)"
else say soap bad "server info over SOAP: ${soapinfo:-no answer}"; fi

# ---- Lua scripts
n=$(ls -1 custom/lua_scripts/*.lua 2>/dev/null | wc -l)
errs=$(docker logs --since 24h "$WORLD" 2>&1 | grep -iE 'lua.*(error|failed)|\[Eluna\].*error|ALE.*error' | sed -E 's/[0-9]{4}-[0-9]{2}-[0-9]{2}[ T][0-9:.]+//' | sort | uniq -c | sort -rn | head -3 | tr '\n' ';')
if [ "$n" -eq 0 ]; then say lua bad "no Lua scripts in $WU/custom/lua_scripts"
elif [ -n "$errs" ]; then say lua bad "$n scripts; errors in the last 24 h: $errs"
else say lua ok "$n scripts, no Lua errors in the last 24 h"; fi
if docker logs --since 72h "$WORLD" 2>&1 | grep -qiE '(ALE|Eluna).*(loaded|Loaded)'; then say lua_loaded ok "the Lua engine reported its scripts loaded"
else say lua_loaded skip "no 'loaded' line in the last 72 h of the log (normal when it has run longer)"; fi

# ---- the streamed slice
if [ -n "${SLICE_HEALTH_B64:-}" ]; then
  out=$(printf '%s' "$SLICE_HEALTH_B64" | base64 -d | sed 's/\r$//' | bash 2>&1)
  last=$(printf '%s\n' "$out" | grep '^SLICE HEALTH DONE' | tail -1)
  probs=$(printf '%s\n' "$out" | grep 'PROBLEM' | head -3 | tr '\n' ';')
  case "$last" in
    "SLICE HEALTH DONE: OK"*) say slice ok "the served slice is whole" ;;
    "SLICE HEALTH DONE"*) say slice bad "${probs:-problems}" ;;
    *) say slice bad "the slice check did not finish" ;;
  esac
else
  say slice skip "slice-health.sh was not sent"
fi

# ---- the relay
if docker ps --format '{{.Names}}' | grep -qx wu-relay; then
  bind=$(sed -n 's/^RELAY_HTTP_BIND=//p' /opt/wu-relay/relay.env 2>/dev/null | tail -1)
  if [ -n "$bind" ] && curl -fsS -m 5 "http://$bind/healthz" >/dev/null 2>&1; then
    realms=$(curl -fsS -m 5 "http://$bind/api/listing/realms.json" 2>/dev/null | python3 -I -c 'import json,sys; print(len(json.load(sys.stdin)["realms"]))' 2>/dev/null)
    say relay ok "running, healthz ok, ${realms:-?} realms listed"
  else
    say relay bad "wu-relay runs but its healthz does not answer (${bind:-no RELAY_HTTP_BIND})"
  fi
else
  say relay bad "wu-relay is not running"
fi

# ---- the GM console and the bug manifest
if curl -fsS -m 5 -H 'Host: 127.0.0.1:8099' http://127.0.0.1:8099/healthz >/dev/null 2>&1; then
  say gmconsole ok "answers on 127.0.0.1:8099"
  TOK=$(sed -n 's/^GM_TOKEN=//p' "$WU/gmconsole.env" 2>/dev/null | tail -1)
  if [ -n "$TOK" ]; then
    counts=$(curl -fsS -m 30 -H 'Host: 127.0.0.1:8099' -H @<(printf 'Authorization: Bearer %s\n' "$TOK") \
      http://127.0.0.1:8099/api/export/bugs-manifest 2>/dev/null | python3 -I -c '
import json, sys
c = json.load(sys.stdin)["counts"]
print("%d open, %d in all" % (c["open"], c["total"]))' 2>/dev/null)
    unset TOK
    if [ -n "$counts" ]; then say bugs ok "the bug manifest answers: $counts"
    else say bugs bad "the bug manifest did not answer"; fi
  else
    say bugs bad "no GM console token in $WU/gmconsole.env"
  fi
else
  say gmconsole bad "the GM console does not answer (the gm step installs it)"
  say bugs bad "the bug manifest lives in the GM console"
fi

# ---- disk and memory (the realm's room to breathe)
free_mb=$(free -m | awk '/^Mem:/{print $7}')
disk=$(df -P / | awk 'NR==2{print int($4/1024/1024)}')
if [ "${free_mb:-0}" -lt 300 ]; then say memory bad "only ${free_mb} MB memory available"; else say memory ok "${free_mb} MB memory available"; fi
if [ "${disk:-0}" -lt 3 ]; then say disk bad "only ${disk} GB disk free (the tidy step makes room)"; else say disk ok "${disk} GB disk free"; fi
echo "AUDIT DONE"
