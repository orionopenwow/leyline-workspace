#!/usr/bin/env bash
# The move to leylinelauncher.com (docs/DOMAIN-MOVE.md; owner 1 Nov 2026). Run as root on the server by
# "Domain move - check.bat" / "- on.bat" / "- off.bat" (one SSH login; this folder goes up with it):
#
#   bash domain.sh check   read only: this server's public IP, DNS for the four new names, ports 80/443, what the
#                          website's sign-in sends Google back to, and the exact Squarespace DNS records to add
#   bash domain.sh on      backup; the new names on the site blocks (Caddy gets their certificates by itself);
#                          self-check each new name over HTTPS; the website's sign-in, the play pages, the realm
#                          scripts and the relay say the new names; then page loads on the old names answer 301
#                          (path and query kept). Anything fails: everything is put back (automatic rollback).
#   bash domain.sh off     undo: every name back as it was (the new names stop answering)
#
# What never changes: what launchers, host packs, desktop clients, the game page and agents call on the old names
# (APIs, /data, /ws, the relay's tunnels, OAuth callbacks): they answer there exactly as today, from the same
# backends, forever. Sign-in cookies are per name: players sign in once more on the new one.
# Last line: DOMAIN OK / DOMAIN CHECK OK / DOMAIN WAITING (exit 3: the new names work, the 301 waits; see above)
# or an error (exit 1).
set -euo pipefail
MODE="${1:-check}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WU="${WU_DIR:-/srv/wrath}"
ST="${WU_STREAM_DIR:-$WU/stream}"
CF="${WU_CADDYFILE:-/opt/openvanilla-classicplus/Caddyfile}"
CADDY_DIR="${WU_CADDY_DIR:-/opt/wenilla-realm}"
REALM_ENV="${WU_REALM_ENV:-$CADDY_DIR/.env}"
REALM_CT="${WU_REALM_CONTAINER:-wenilla-realm-realm-1}"
RELAY_DIR="${WU_RELAY_DIR:-/opt/wu-relay}"
DSTATE="${WU_DOMAIN_DIR:-$WU/domain}"
BKROOT="${WU_DOMAIN_BACKUPS:-/root/domain-backups}"
TLS_WAIT="${WU_DOMAIN_TLS_WAIT:-180}"     # seconds for Caddy to get the new certificates
PAUSE="${WU_DOMAIN_PAUSE:-5}"
FLAG="$WU/custom/lua_scripts/leyline-domain.on"
PY=(python3 -I "$HERE/domain_caddy.py")
die() { printf '\nDOMAIN FAILED: %s\n' "$*" >&2; exit 1; }
log() { printf '\n== %s\n' "$*"; }
note() { printf '  %s\n' "$*"; }
case "$MODE" in check|on|off) ;; *) die "usage: domain.sh check|on|off" ;; esac
[ -f "$HERE/domain_caddy.py" ] && [ -f "$HERE/addresses.json" ] || die "domain_caddy.py and addresses.json must be next to domain.sh"

# the names (tools/domain/addresses.json: the one list)
eval "$(python3 -I - "$HERE/addresses.json" <<'PY'
import json, re, sys
a = json.load(open(sys.argv[1], encoding="utf-8"))
ok = re.compile(r"^[a-z0-9.-]{3,253}$")
for p in ("site", "game", "relay"):
    for k in ("new", "old"):
        assert ok.match(a[p][k]), a[p][k]
        print(f"{k.upper()}_{p.upper()}={a[p][k]}")
assert ok.match(a["www"]["new"])
print(f"WWW={a['www']['new']}")
sv = (a.get("survivors") or {}).get("new", "")
assert sv == "" or ok.match(sv)
print(f"SURVIVORS={sv}")
PY
)"

kv_get() { grep -m1 -E "^$2=" "$1" 2>/dev/null | cut -d= -f2- | tr -d '\r' || true; }
kv_set() { local f=$1 k=$2 v=$3; touch "$f"; if grep -q "^$k=" "$f"; then
  python3 -I - "$f" "$k" "$v" <<'PY'
import sys
p, k, v = sys.argv[1:4]
lines = open(p, encoding="utf-8").read().splitlines()
out, done = [], False
for ln in lines:
    if not done and ln.startswith(k + "="):
        out.append(f"{k}={v}"); done = True
    else:
        out.append(ln)
with open(p, "r+", encoding="utf-8") as f:   # keep the inode (env files can be bind-mounted)
    f.seek(0); f.write("\n".join(out) + "\n"); f.truncate()
PY
  else echo "$k=$v" >> "$f"; fi; }
resolve4() { getent ahostsv4 "$1" 2>/dev/null | awk 'NR==1{print $1}' || true; }
caddy_reload() { (cd "$CADDY_DIR" && docker compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile) >/dev/null 2>&1; }
# "<code> <location>" for a URL; extra args are request headers ("Sec-Fetch-Dest: document")
http() { local u=$1; shift; local h=(); for x in "$@"; do h+=(-H "$x"); done
  curl -s -m 20 -o /dev/null -w '%{http_code} %{redirect_url}' "${h[@]}" "$u" 2>/dev/null || echo "000 "; }
code_of() { http "$@" | cut -d' ' -f1; }
loc_of() { http "$@" | cut -d' ' -f2-; }
# what the website's Google sign-in sends people back to, asked on this name (the Location of /auth/google/start)
google_back() { loc_of "https://$1/auth/google/start" | python3 -I -c '
import sys, urllib.parse
q = urllib.parse.parse_qs(urllib.parse.urlsplit(sys.stdin.read().strip()).query)
print((q.get("redirect_uri") or ["(no Google sign-in here)"])[0])'; }
PAGE=("Sec-Fetch-Dest: document" "Sec-Fetch-Mode: navigate" "Accept: text/html")

# This server's public addresses: what the old site name points at (it points here), and the box's own.
IP4=$(resolve4 "$OLD_SITE")
BOX_IPS=$( { hostname -I 2>/dev/null || true; } | tr ' ' '\n' | grep -E '^[0-9.]+$' || true)
IP6=$( { ip -6 addr show scope global 2>/dev/null || true; } | awk '/inet6/ {sub("/.*", "", $2); print $2; exit}')
NAMES=("$NEW_SITE" "$NEW_GAME" "$NEW_RELAY" "$WWW")
dns_state() { local got; got=$(resolve4 "$1"); if [ -z "$got" ]; then echo missing; elif [ "$got" = "$IP4" ]; then echo ok; else echo "wrong:$got"; fi; }
all_dns_ok() { local n; for n in "${NAMES[@]}"; do [ "$(dns_state "$n")" = ok ] || return 1; done; }
# The website's own settings (the Rust realm service, /opt/wenilla-realm/.env) that name the old site: names only.
realm_keys() { grep -E "^[A-Z0-9_]*(URL|DOMAIN|HOST|ORIGIN|BASE|REDIRECT|CALLBACK)[A-Z0-9_]*=.*${OLD_SITE//./\\.}" "$REALM_ENV" 2>/dev/null | cut -d= -f1 || true; }
realm_ct_keys() { docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$REALM_CT" 2>/dev/null \
  | grep -E "^[A-Z0-9_]*(URL|DOMAIN|HOST|ORIGIN|BASE|REDIRECT|CALLBACK)[A-Z0-9_]*=.*${OLD_SITE//./\\.}" | cut -d= -f1 | tr '\n' ' ' || true; }
relay_on_box() { [ -f "$RELAY_DIR/relay.env" ]; }

records() {
  local ip=${IP4:-<the server IPv4 above>}
  cat <<EOF

=====================================================================
 Squarespace: Domains > leylinelauncher.com > DNS > DNS Settings.
 Delete Squarespace's own default records for @ and www (the
 "Squarespace Defaults" group) if they are there, then under
 "Custom records" add these four (TTL: the smallest offered):
   Type A     Host @       Data $ip
   Type A     Host play    Data $ip
   Type A     Host relay   Data $ip
   Type A     Host www     Data $ip
EOF
  if [ -n "$IP6" ]; then cat <<EOF
 This server also has IPv6. Optional, for the website and the game only:
   Type AAAA  Host play    Data $IP6
   Type AAAA  Host www     Data $IP6
 No AAAA for @ or relay: desktop game clients only speak IPv4 (realmlist).
EOF
  else echo " This server has no IPv6: no AAAA records."; fi
  if [ -n "$SURVIVORS" ]; then
    echo " Optional, only when Azeroth Survivors goes live (docs/SURVIVORS.md):"
    printf '   Type A     Host %-7s Data %s\n' "${SURVIVORS%.leylinelauncher.com}" "$ip"
  fi
  echo "====================================================================="
}

if [ "$MODE" = check ]; then
  echo "DOMAIN CHECK (nothing is changed)"
  log "this server"
  note "public IPv4: ${IP4:-unknown} (what $OLD_SITE points at)"
  if [ -n "$IP4" ] && [ -n "$BOX_IPS" ]; then
    echo "$BOX_IPS" | grep -qx "$IP4" && note "this box has $IP4 itself: yes" || note "NOTE: $IP4 is not on this box's own interfaces (behind NAT?): DNS still points at $IP4"
  fi
  note "IPv6: ${IP6:-none}"
  log "DNS for the new names"
  ready=1
  for n in "${NAMES[@]}"; do
    s=$(dns_state "$n")
    case "$s" in
      ok) note "$n -> $IP4  OK" ;;
      missing) note "$n -> nothing yet  (add the record below)"; ready=0 ;;
      wrong:*) note "$n -> ${s#wrong:}  WRONG: change it to $IP4"; ready=0 ;;
    esac
  done
  log "ports 80 and 443 reach Caddy"
  if command -v ss >/dev/null 2>&1; then
    for p in 80 443; do ss -ltnH "sport = :$p" 2>/dev/null | grep -q . && note "$p: listening" || note "$p: NOT listening"; done
  fi
  if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q '^Status: active'; then
    for p in 80 443; do ufw status 2>/dev/null | grep -qE "^$p(/tcp)?[[:space:]].*ALLOW" && note "ufw: $p allowed" || note "ufw: NOTE: $p not listed as allowed"; done
  fi
  c=$(code_of "https://$OLD_SITE/wrath/status.json"); note "https://$OLD_SITE/wrath/status.json -> $c (200: Caddy answers on 443)"
  c=$(code_of "http://$OLD_SITE/wrath/status.json"); note "http://$OLD_SITE/wrath/status.json -> $c (301/308: Caddy answers on 80 and sends https)"
  for n in "${NAMES[@]}"; do
    [ "$(dns_state "$n")" = ok ] || continue
    c=$(code_of "http://$n/.well-known/acme-challenge/leyline-check"); note "http://$n/ (port 80, as Let's Encrypt asks it) -> $c (any number: it reached Caddy)"
  done
  log "the Caddyfile ($CF)"
  [ -f "$CF" ] && "${PY[@]}" status "$CF" "$HERE/addresses.json" | sed 's/^/  /' || note "NOT FOUND"
  log "the website's sign-in"
  note "Google sends people back to (asked on $OLD_SITE): $(google_back "$OLD_SITE")"
  k=$(realm_keys | tr '\n' ' '); note "its settings in $REALM_ENV that name $OLD_SITE (names only): ${k:-none}"
  kc=$(realm_ct_keys); note "the same in the running realm service ($REALM_CT): ${kc:-none}"
  for x in $kc; do echo " $k " | grep -q " $x " || note "NOTE: $x is set outside $REALM_ENV (compose file?): 'on' leaves it; change it there by hand"; done
  note "'on' points the URL/host settings at $NEW_SITE, adds $NEW_SITE to the list settings, and restarts the realm service"
  note "BEFORE 'on': add these redirect URIs (keep the old ones):"
  note "  Google:  https://$NEW_SITE/auth/google/callback"
  note "  Discord: https://$NEW_SITE/auth/discord/callback"
  log "the play pages (portal.env)"
  for k in PUBLIC_HOST STREAM_HOST LEYLINE_DOMAIN_MOVE WU_JOIN_BASE WU_RELAY_WS; do note "$k=$(kv_get "$WU/portal.env" "$k")"; done
  log "the relay"
  if relay_on_box; then
    for k in RELAY_NAME RELAY_SITE_URL RELAY_SITE_ORIGINS RELAY_PLAY_ORIGINS; do note "$k=$(kv_get "$RELAY_DIR/relay.env" "$k")"; done
    note "'on' restarts the relay once (open tunnels reconnect by themselves within a minute)"
  else note "no relay on this box ($RELAY_DIR/relay.env): skipped"; fi
  if [ -n "$SURVIVORS" ]; then
    log "optional: Azeroth Survivors' address"
    s=$(dns_state "$SURVIVORS"); note "$SURVIVORS: ${s/wrong:/points at } (needed only when Survivors goes live: Put Survivors on Leyline)"
  fi
  log "state"
  note "$(cat "$DSTATE/domain.json" 2>/dev/null || echo 'never turned on')"
  records
  if [ "$ready" = 1 ]; then echo "READY: all four names point here. Next: the Google and Discord redirect URIs, then 'Domain move - on'."
  else echo "WAITING FOR DNS: add the records above, wait 10-30 minutes, run this check again."; fi
  echo "DOMAIN CHECK OK"
  exit 0
fi

[ -f "$CF" ] || die "the Caddyfile ($CF) was not found. Nothing was changed."
BK="$BKROOT/$(date +%Y%m%d-%H%M%S)-$MODE"; ( umask 077; mkdir -p "$BK" ) || die "could not make $BK"
cp -p "$CF" "$BK/Caddyfile"
[ -f "$WU/portal.env" ] && cp -p "$WU/portal.env" "$BK/portal.env"
[ -f "$REALM_ENV" ] && cp -p "$REALM_ENV" "$BK/realm.env"
relay_on_box && cp -p "$RELAY_DIR/relay.env" "$BK/relay.env"
[ -f "$DSTATE/domain.json" ] && cp -p "$DSTATE/domain.json" "$BK/domain.json"
echo "DOMAIN $MODE (backup: $BK)"
mkdir -p "$DSTATE"

portal_recreate() {
  [ -f "$ST/stream-lib.sh" ] || { echo "  NOTE: no $ST/stream-lib.sh: restarting wu-portal"; docker restart wu-portal >/dev/null 2>&1; return; }
  # shellcheck disable=SC1091
  ( . "$ST/stream-lib.sh"; recreate_portal )
}
realm_restart() { (cd "$CADDY_DIR" && docker compose up -d --no-deps realm) >/dev/null 2>&1; }
relay_restart() { docker compose -p wu-relay --project-directory "$RELAY_DIR" -f "$RELAY_DIR/compose.yml" up -d >/dev/null 2>&1; }
relay_healthy() { local b; b=$(kv_get "$RELAY_DIR/relay.env" RELAY_HTTP_BIND); b=${b:-127.0.0.1:8080}
  for _ in $(seq 1 12); do curl -fsS -m 3 "http://$b/healthz" >/dev/null 2>&1 && return 0; sleep "$PAUSE"; done; return 1; }
wait_code() {   # wait_code <url> <want> [header...]: retries for TLS_WAIT seconds (a new certificate takes a minute)
  local u=$1 want=$2; shift 2; local got="" end=$(( $(date +%s) + TLS_WAIT ))
  while :; do got=$(code_of "$u" "$@"); [ "$got" = "$want" ] && { note "$u -> $got"; return 0; }
    [ "$(date +%s)" -ge "$end" ] && { note "$u -> $got (wanted $want)"; return 1; }; sleep "$PAUSE"; done; }
# the file's lines for the given keys, as they are now (KEY=value, or KEY= with a # when it was not set)
# Kept from the first "on" only: running "on" again (after DOMAIN WAITING) must not overwrite what "off" goes back to.
save_keys() { local f=$1 out=$2; shift 2; [ "$WAS_ON" = 1 ] && [ -f "$out" ] && return 0; : > "$out"; chmod 600 "$out"
  for k in "$@"; do if grep -q "^$k=" "$f" 2>/dev/null; then grep -m1 "^$k=" "$f" >> "$out"; else echo "#unset $k" >> "$out"; fi; done; }
restore_keys() { local f=$1 saved=$2 ln k
  [ -f "$saved" ] || return 0
  while IFS= read -r ln; do
    case "$ln" in
      "#unset "*) k=${ln#\#unset }; python3 -I - "$f" "$k" <<'PY'
import sys
p, k = sys.argv[1:3]
s = [l for l in open(p, encoding="utf-8").read().splitlines() if not l.startswith(k + "=")]
with open(p, "r+", encoding="utf-8") as f:
    f.seek(0); f.write("\n".join(s) + ("\n" if s else "")); f.truncate()
PY
      ;;
      *=*) kv_set "$f" "${ln%%=*}" "${ln#*=}" ;;
    esac
  done < "$saved"; }
# list-like settings get the new value added (both names allowed); the others point at the new name
realm_env_move() {
  python3 -I - "$REALM_ENV" "$OLD_SITE" "$NEW_SITE" "$@" <<'PY'
import re, sys
p, old, new, keys = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
lines = open(p, encoding="utf-8").read().splitlines()
out = []
for ln in lines:
    k, sep, v = ln.partition("=")
    if sep and k in keys and old in v:
        quote = v[:1] if v[:1] in "\"'" and v.endswith(v[:1]) and len(v) > 1 else ""
        body = v[1:-1] if quote else v
        if re.search(r"(ORIGINS|ALLOWED|HOSTS|DOMAINS|REDIRECTS|URLS)$", k) or "," in body:
            items = [x for x in body.split(",") if x]
            for x in list(items):
                y = x.replace(old, new)
                if y not in items:
                    items.append(y)
            body = ",".join(items)
        else:
            body = body.replace(old, new)
        ln = f"{k}={quote}{body}{quote}"
        print(f"  {k}: now names {new}")
    out.append(ln)
with open(p, "r+", encoding="utf-8") as f:
    f.seek(0); f.write("\n".join(out) + "\n"); f.truncate()
PY
}
list_add() { local cur=$1; shift; local x; for x in "$@"; do case ",$cur," in *",$x,"*) ;; *) cur="${cur:+$cur,}$x" ;; esac; done; echo "$cur"; }
set_state() { printf '{"phase": "%s", "at": "%s", "backup": "%s"}\n' "$1" "$(date -u +%FT%TZ)" "$BK" > "$DSTATE/domain.json.tmp" && mv -f "$DSTATE/domain.json.tmp" "$DSTATE/domain.json"; }

WAS_ON=0; [ -f "$DSTATE/domain.json" ] && WAS_ON=1
PORTAL_CHANGED=0; REALM_CHANGED=0; RELAY_CHANGED=0; FLAG_WAS=0; [ -f "$FLAG" ] && FLAG_WAS=1
rollback() {
  echo "  rolling back (backup $BK)" >&2
  cat "$BK/Caddyfile" > "$CF"; caddy_reload || echo "  NOTE: Caddy did not reload the old Caddyfile: docker compose exec caddy caddy reload (in $CADDY_DIR)" >&2
  if [ "$PORTAL_CHANGED" = 1 ] && [ -f "$BK/portal.env" ]; then cat "$BK/portal.env" > "$WU/portal.env"; portal_recreate >/dev/null 2>&1 || echo "  NOTE: wu-portal did not come back: docker logs wu-portal" >&2; fi
  if [ "$REALM_CHANGED" = 1 ] && [ -f "$BK/realm.env" ]; then cat "$BK/realm.env" > "$REALM_ENV"; realm_restart || echo "  NOTE: the realm service did not restart: cd $CADDY_DIR && docker compose up -d --no-deps realm" >&2; fi
  if [ "$RELAY_CHANGED" = 1 ] && [ -f "$BK/relay.env" ]; then cat "$BK/relay.env" > "$RELAY_DIR/relay.env"; relay_restart || echo "  NOTE: the relay did not restart: docker logs wu-relay" >&2; fi
  if [ "$FLAG_WAS" = 1 ]; then [ -f "$FLAG" ] || printf 'Leyline domain move (restored)\n' > "$FLAG"; else rm -f "$FLAG"; fi
  if [ -f "$BK/domain.json" ]; then cp -p "$BK/domain.json" "$DSTATE/domain.json"; else rm -f "$DSTATE/domain.json"; fi
}
fail() { rollback; die "$1; everything is back as it was (backup $BK)"; }

if [ "$MODE" = on ]; then
  log "1/7 checks"
  [ -n "$IP4" ] || die "could not work out this server's IPv4 (what $OLD_SITE points at). Nothing was changed."
  for n in "${NAMES[@]}"; do
    s=$(dns_state "$n"); [ "$s" = ok ] || die "DNS: $n is ${s/wrong:/pointing at } (it must point at $IP4). Add the records the check step prints, then run this again. Nothing was changed."
    note "$n -> $IP4"
  done
  [ "$(code_of "https://$OLD_SITE/wrath/status.json")" = 200 ] || die "https://$OLD_SITE/wrath/status.json does not answer 200 now: fix that first. Nothing was changed."

  log "2/7 Caddy: the new names on the site blocks"
  install -m 644 "$HERE/domain_caddy.py" "$DSTATE/domain_caddy.py"; install -m 644 "$HERE/addresses.json" "$DSTATE/addresses.json"
  "${PY[@]}" apply "$CF" alias "$HERE/addresses.json" || fail "could not add the new names to the Caddyfile"
  caddy_reload || fail "Caddy refused the new names"
  set_state alias
  log "3/7 self-check: each new name over HTTPS (Caddy gets the certificates now; up to ${TLS_WAIT}s)"
  wait_code "https://$NEW_SITE/wrath/status.json" 200 || fail "$NEW_SITE did not answer over HTTPS"
  wait_code "https://$NEW_GAME/warm.html" 200 || fail "$NEW_GAME did not answer over HTTPS"
  if [ "$("${PY[@]}" status "$CF" "$HERE/addresses.json" | sed -n 's/^relay: //p')" != no-block ]; then
    wait_code "https://$NEW_RELAY/healthz" 200 || fail "$NEW_RELAY did not answer over HTTPS"
  else note "no relay block in this Caddyfile: $NEW_RELAY skipped"; fi
  wait_code "https://$WWW/" 301 || fail "$WWW did not answer 301"
  l=$(loc_of "https://$WWW/x?y=1"); [ "$l" = "https://$NEW_SITE/x?y=1" ] || fail "$WWW sends to '$l', not https://$NEW_SITE/x?y=1"
  wait_code "https://$OLD_SITE/wrath/status.json" 200 || fail "the old name stopped answering"

  log "4/7 the website's sign-in"
  keys=$(realm_keys | tr '\n' ' ')
  if [ -n "$keys" ]; then
    # shellcheck disable=SC2086
    save_keys "$REALM_ENV" "$DSTATE/realm-env-before.txt" $keys
    REALM_CHANGED=1
    sum=$(sha256sum < "$REALM_ENV")
    # shellcheck disable=SC2086
    realm_env_move $keys || fail "could not edit $REALM_ENV"
    if [ "$(sha256sum < "$REALM_ENV")" != "$sum" ]; then
      realm_restart || fail "the realm service did not restart"
      wait_code "https://$NEW_SITE/login" 200 || fail "the website did not come back on $NEW_SITE"
    else note "its settings already name $NEW_SITE"; fi
  else
    [ "$WAS_ON" = 1 ] || : > "$DSTATE/realm-env-before.txt"
    note "no setting in $REALM_ENV names $OLD_SITE: the website is left as it is"
  fi
  for x in $(realm_ct_keys); do echo " $keys " | grep -q " $x " || note "NOTE: $x (set outside $REALM_ENV) still names $OLD_SITE: change it by hand"; done
  back=$(google_back "$NEW_SITE"); note "Google sign-in on $NEW_SITE comes back to: $back"
  SIGNIN_OK=1
  case "$back" in *"$OLD_SITE"*) SIGNIN_OK=0 ;; esac

  log "5/7 the play pages say the new names (portal.env)"
  save_keys "$WU/portal.env" "$DSTATE/portal-env-before.txt" PUBLIC_HOST STREAM_HOST LEYLINE_DOMAIN_MOVE WU_JOIN_BASE WU_RELAY_WS
  PORTAL_CHANGED=1
  kv_set "$WU/portal.env" LEYLINE_DOMAIN_MOVE 1
  kv_set "$WU/portal.env" PUBLIC_HOST "$NEW_SITE"
  kv_set "$WU/portal.env" STREAM_HOST "$NEW_GAME"
  kv_set "$WU/portal.env" WU_JOIN_BASE "https://$NEW_RELAY"
  kv_set "$WU/portal.env" WU_RELAY_WS "wss://$NEW_RELAY"
  portal_recreate >/dev/null || fail "wu-portal did not come back (docker logs wu-portal)"
  wait_code "https://$NEW_SITE/wrath/status.json" 200 || fail "the play pages did not come back"
  l=$(loc_of "https://$NEW_SITE/wrath/play?mini=mini"); note "signed out, https://$NEW_SITE/wrath/play -> ${l:-(the page itself)}"
  case "$l" in *"$OLD_SITE"*|*"$OLD_GAME"*) fail "the play page still sends people to the old name ($l)" ;; esac
  printf 'Leyline domain move on (tools/domain/domain.sh, %s): the realm scripts say %s. Run domain.sh off to undo.\n' "$(date -u +%FT%TZ)" "$NEW_SITE" > "$FLAG"
  chmod 644 "$FLAG"; note "realm scripts: $FLAG (they see it within 30 s; no reload, nobody kicked)"

  log "6/7 the relay"
  if relay_on_box; then
    save_keys "$RELAY_DIR/relay.env" "$DSTATE/relay-env-before.txt" RELAY_NAME RELAY_SITE_URL RELAY_SITE_ORIGINS RELAY_PLAY_ORIGINS
    RELAY_CHANGED=1
    trim() { local v=${1%%#*}; v=${v//[[:space:]]/}; echo "$v"; }
    so=$(trim "$(kv_get "$RELAY_DIR/relay.env" RELAY_SITE_ORIGINS)"); po=$(trim "$(kv_get "$RELAY_DIR/relay.env" RELAY_PLAY_ORIGINS)")
    sum=$(sha256sum < "$RELAY_DIR/relay.env")
    kv_set "$RELAY_DIR/relay.env" RELAY_NAME "$NEW_RELAY"
    kv_set "$RELAY_DIR/relay.env" RELAY_SITE_URL "https://$NEW_SITE"
    kv_set "$RELAY_DIR/relay.env" RELAY_SITE_ORIGINS "$(list_add "$so" "https://$OLD_SITE" "https://$NEW_SITE")"
    kv_set "$RELAY_DIR/relay.env" RELAY_PLAY_ORIGINS "$(list_add "$po" "https://$OLD_GAME" "https://$OLD_SITE" "https://$NEW_GAME" "https://$NEW_SITE")"
    if [ "$(sha256sum < "$RELAY_DIR/relay.env")" != "$sum" ]; then
      relay_restart || fail "the relay did not restart"
      relay_healthy || fail "the relay did not answer /healthz after the restart"
      wait_code "https://$NEW_RELAY/healthz" 200 || fail "$NEW_RELAY did not answer after the restart"
    fi
    note "relay: says $NEW_RELAY; allows the game and the site on both names"
  else
    [ "$WAS_ON" = 1 ] || : > "$DSTATE/relay-env-before.txt"
    note "no relay on this box: skipped"
  fi

  if [ "$SIGNIN_OK" != 1 ]; then
    cat <<EOF

=====================================================================
 The new names work. The 301 on the old names WAITS: the website's
 Google sign-in still comes back to $OLD_SITE ($back),
 so a 301 now would leave people signed out on the new name.
 Fix: the website's own address setting (see "the website's sign-in"
 in the check step), then run "Domain move - on" again.
=====================================================================
DOMAIN WAITING
EOF
    exit 3
  fi

  log "7/7 Caddy: page loads on the old names answer 301"
  cp -p "$CF" "$BK/Caddyfile.alias"
  "${PY[@]}" apply "$CF" redirect "$HERE/addresses.json" || fail "could not add the redirects"
  caddy_reload || fail "Caddy refused the redirects"
  set_state redirect
  bad=""
  l=$(loc_of "https://$OLD_SITE/wrath/leyline?from=check" "${PAGE[@]}"); note "a page on $OLD_SITE -> $l"
  [ "$l" = "https://$NEW_SITE/wrath/leyline?from=check" ] || bad="$bad site-page"
  c=$(code_of "https://$OLD_SITE/wrath/status.json"); note "an API on $OLD_SITE -> $c (200: answered there)"; [ "$c" = 200 ] || bad="$bad site-api"
  l=$(loc_of "https://$OLD_SITE/auth/google/start" "${PAGE[@]}"); case "$l" in "https://$NEW_SITE"*) bad="$bad oauth" ;; esac
  l=$(loc_of "https://$OLD_GAME/?from=check" "${PAGE[@]}"); note "the game page on $OLD_GAME -> $l"
  [ "$l" = "https://$NEW_GAME/?from=check" ] || bad="$bad game-page"
  l=$(loc_of "https://$OLD_GAME/data/__index" "${PAGE[@]}"); case "$l" in "https://$NEW_GAME"*) bad="$bad game-data" ;; esac
  if relay_on_box || [ "$("${PY[@]}" status "$CF" "$HERE/addresses.json" | sed -n 's/^relay: //p')" = redirect ]; then
    c=$(code_of "https://$OLD_RELAY/healthz"); note "the relay on $OLD_RELAY -> $c"; [ "$c" = 200 ] || bad="$bad relay"
  fi
  [ -z "$bad" ] || fail "the redirects did not answer as they should ($bad)"
  echo
  echo "New names: https://$NEW_SITE  https://$NEW_GAME  https://$NEW_RELAY  (www -> $NEW_SITE)"
  echo "Desktop realmlist: $NEW_SITE (and $OLD_SITE keeps working). Undo: Domain move - off"
  echo "DOMAIN OK"
  exit 0
fi

# off
log "1/5 Caddy: the old names only"
"${PY[@]}" remove "$CF" "$HERE/addresses.json" || fail "could not edit the Caddyfile"
caddy_reload || fail "Caddy refused the Caddyfile without the new names"
log "2/5 the play pages"
if [ -f "$WU/portal.env" ]; then
  PORTAL_CHANGED=1
  if [ -s "$DSTATE/portal-env-before.txt" ]; then restore_keys "$WU/portal.env" "$DSTATE/portal-env-before.txt"
  else kv_set "$WU/portal.env" PUBLIC_HOST "$OLD_SITE"; kv_set "$WU/portal.env" STREAM_HOST "$OLD_GAME"; fi
  kv_set "$WU/portal.env" LEYLINE_DOMAIN_MOVE 0
  portal_recreate >/dev/null || fail "wu-portal did not come back (docker logs wu-portal)"
fi
rm -f "$FLAG"; note "realm scripts: say $OLD_SITE again within 30 s"
log "3/5 the website's sign-in"
if [ -s "$DSTATE/realm-env-before.txt" ] && [ -f "$REALM_ENV" ]; then
  REALM_CHANGED=1; restore_keys "$REALM_ENV" "$DSTATE/realm-env-before.txt"
  realm_restart || fail "the realm service did not restart"
  note "its settings are back as they were"
else note "nothing was changed there"; fi
log "4/5 the relay"
if [ -s "$DSTATE/relay-env-before.txt" ] && relay_on_box; then
  RELAY_CHANGED=1; restore_keys "$RELAY_DIR/relay.env" "$DSTATE/relay-env-before.txt"
  relay_restart || fail "the relay did not restart"
  relay_healthy || fail "the relay did not answer /healthz after the restart"
  note "relay: back as it was"
else note "nothing was changed there"; fi
log "5/5 self-check"
wait_code "https://$OLD_SITE/wrath/status.json" 200 || fail "$OLD_SITE does not answer"
rm -f "$DSTATE/domain.json"
echo "The new names no longer answer (DNS can stay). On again: Domain move - on"
echo "DOMAIN OK"
