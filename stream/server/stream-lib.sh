#!/usr/bin/env bash
# Shared by stream-up.sh, stream-down.sh, stream-switch.sh and slice-install.sh (sourced).
# The paths can be moved for tests (tests/test_server_scripts.sh); on the server they are these.
WU="${WU_DIR:-/srv/wrath}"
ST="${WU_STREAM_DIR:-$WU/stream}"
CF="${WU_CADDYFILE:-/opt/openvanilla-classicplus/Caddyfile}"
CADDY_DIR="${WU_CADDY_DIR:-/opt/wenilla-realm}"
STREAM_HOST="${WU_STREAM_HOST:-wrath.openvanilla.app}"
UPSTREAM="${WU_UPSTREAM:-play.openvanilla.app}"
NET=wenilla-realm_default
MARK_BEGIN="# >>> WrathUnleashed streaming (stream-up.sh)"
MARK_END="# <<< WrathUnleashed streaming"
log() { printf '\n== %s\n' "$*"; }
die() { printf '\nERROR: %s\n' "$*" >&2; exit 1; }

# key=value in a settings file, replaced or appended
kv_set() { local f=$1 k=$2 v=$3; touch "$f"; if grep -q "^$k=" "$f"; then sed -i "s#^$k=.*#$k=$v#" "$f"; else echo "$k=$v" >> "$f"; fi; }

# wu-portal reads its settings when the container is created, so a change means a new container.
# Same command as deploy.sh step 9.
recreate_portal() {
  docker image inspect python:3.12-alpine >/dev/null 2>&1 || docker pull -q python:3.12-alpine >/dev/null
  docker rm -f wu-portal >/dev/null 2>&1 || true
  mkdir -p "$WU/hosted" && chmod 700 "$WU/hosted"   # /wrath/host's pending realms (stream/hostflow.py)
  mkdir -p "$WU/portal-data"   # the Workshop's community links (stream/projects.py)
  docker run -d --name wu-portal --restart unless-stopped --memory 128m \
    --network "$NET" --network-alias mangosd \
    --env-file "$WU/portal.env" -v "$WU/portal:/app:ro" -v "$WU/hosted:/hosted" -v "$WU/portal-data:/data" \
    python:3.12-alpine python3 -I /app/wrath_portal.py >/dev/null
  docker network connect ov-wotlk_edge wu-portal
  local _
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    docker exec wu-portal python3 -c 'import urllib.request;urllib.request.urlopen("http://127.0.0.1:8091/wrath/status.json",timeout=5)' >/dev/null 2>&1 && return 0
    sleep 1
  done
  docker logs --tail 20 wu-portal >&2 || true
  return 1
}

caddy_reload() { (cd "$CADDY_DIR" && docker compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile); }

# The domain move (tools/domain/domain.sh, docs/DOMAIN-MOVE.md): after our block was rewritten (old name only), the
# new name and the 301 for old page loads go back on it. Nothing when the move is off.
DOMAIN_DIR="${WU_DOMAIN_DIR:-$WU/domain}"
domain_reapply() {
  [ -f "$DOMAIN_DIR/domain.json" ] && [ -f "$DOMAIN_DIR/domain_caddy.py" ] || return 0
  python3 -I "$DOMAIN_DIR/domain_caddy.py" reapply "$CF" "$DOMAIN_DIR/domain.json" "$DOMAIN_DIR/addresses.json"
}

# Remove our site block (between the markers) from the Caddyfile, in place (it is bind-mounted).
caddy_strip() {
  python3 -I - "$CF" "$MARK_BEGIN" "$MARK_END" <<'PY'
import sys
p, b, e = sys.argv[1:4]
s = open(p).read()
while b in s:
  i = s.index(b)
  j = s.find(e, i)
  if j < 0: sys.exit("start marker without end marker in the Caddyfile")
  j = s.find("\n", j)
  s = s[:i].rstrip("\n") + "\n" + (s[j + 1:] if j >= 0 else "")
with open(p, "r+") as f:
  f.seek(0); f.write(s); f.truncate()
PY
}

soap() { bash "$WU/soap-cmd.sh" "$@"; }

# The realm rule's tables, from the slice: ov_slices_tiers.lua (the tiers of slices/<slice>.json, by
# lua/tiers_lua.py) and ov_slices_lfg.lua (the Dungeon Finder ids and levels, from LFGDungeons.dbc).
# Returns 1 when the tiers could not be made (the slice description is broken).
realm_tables() {
  local slice_json="$1" slice_dir="$2" scripts="$WU/custom/lua_scripts"
  python3 -I "$ST/lua/tiers_lua.py" "$slice_json" > "$ST/ov_slices_tiers.lua.new" || { rm -f "$ST/ov_slices_tiers.lua.new"; return 1; }
  install -m 644 "$ST/ov_slices_tiers.lua.new" "$scripts/ov_slices_tiers.lua"; rm -f "$ST/ov_slices_tiers.lua.new"
  echo "  tiers: $(grep -c '^    \[' "$scripts/ov_slices_tiers.lua") (from $slice_json)"
  local lfg
  lfg=$(find "$WU/client-data/dbc" "$slice_dir" -maxdepth 2 -iname 'LFGDungeons.dbc' 2>/dev/null | head -1 || true)
  if [ -n "$lfg" ] && python3 -I "$ST/lua/lfg_ids.py" "$lfg" > "$ST/ov_slices_lfg.lua.new"; then
    install -m 644 "$ST/ov_slices_lfg.lua.new" "$scripts/ov_slices_lfg.lua"; rm -f "$ST/ov_slices_lfg.lua.new"
    echo "  Dungeon Finder ids: $(grep -c '\] = [0-9]*,$' "$scripts/ov_slices_lfg.lua") dungeons (from $lfg)"
  else
    rm -f "$ST/ov_slices_lfg.lua.new"
    echo "  WARNING: no LFGDungeons.dbc found: stream players cannot use the Dungeon Finder (everything else works)"
  fi
}

# --- Live slices (slices/<name>, served by wu-play). Incident 1 Nov 2026 (docs/INCIDENT-2026-11-01-SLICE.md).
# Rules for every script that changes a slice:
#   * one change at a time: slice_lock (flock on slices/.lock; nested scripts see WU_SLICE_LOCKED and go on);
#   * never in place: the change goes into a copy (slice_restage: hard links, so no second copy of the data on
#     disk), the copy is checked (slice-health.sh --verify) and then renamed over the live folder;
#   * a copy made by hard links shares its files with the live slice, so nothing may write a file in place:
#     write a new file and rename it (fui-overlay.py / mods-overlay.py write_atomic), or unlink and add;
#   * wu-play keeps serving the folder it was started on, even after a rename: a replaced folder is removed only
#     after wu-play has been restarted onto the new one (slice_retire checks the inode it serves).
slice_lock() {
  [ -n "${WU_SLICE_LOCKED:-}" ] && return 0
  mkdir -p "$ST/slices"
  exec 9>>"$ST/slices/.lock"
  if command -v flock >/dev/null 2>&1; then
    flock -w "${WU_SLICE_LOCK_WAIT:-900}" 9 \
      || die "another slice change is still running (lock $ST/slices/.lock): wait for it to finish, then run this again"
  fi
  export WU_SLICE_LOCKED=1
}

# 0 = the folder is whole (key files, tier lists); prints the short report. No checker on the server: 0 with a note.
slice_verify() {
  if [ -f "$ST/slice-health.sh" ]; then bash "$ST/slice-health.sh" --verify "$1"
  else echo "  (no $ST/slice-health.sh: the slice was not checked)"; return 0; fi
}

# 0 = wu-play runs and serves exactly this folder (same inode; it may have been renamed since).
play_serves() {
  local want got
  want=$(stat -c %i "$1" 2>/dev/null) || return 1
  got=$(docker exec wu-play stat -c %i /slice 2>/dev/null) || return 1
  [ -n "$got" ] && [ "$got" = "$want" ]
}

# What wu-play answers for a key file, asked from wu-portal inside the Docker network:
# 0 = 200, 1 = another answer (printed), 2 = no answer (wu-play still starting, or no wu-portal to ask from).
play_probe() {
  local c="" _
  for _ in $(seq 1 "${WU_PLAY_WAIT:-20}"); do
    c=$(docker exec wu-portal python3 -c 'import urllib.request,urllib.error
try: print(urllib.request.urlopen("http://wu-play:8090/data/DBFilesClient%5CSpell.dbc",timeout=15).status)
except urllib.error.HTTPError as e: print(e.code)
except Exception: print("none")' 2>/dev/null | tail -1)
    [ "$c" = 200 ] && return 0
    [ -n "$c" ] || { PLAY_PROBE="no answer"; return 2; }   # docker exec failed: nothing to ask from
    sleep 1   # "none" (still starting) or 404 while its index builds: ask again
  done
  PLAY_PROBE=$c
  case "$c" in [0-9][0-9][0-9]) return 1 ;; *) return 2 ;; esac
}

# Remove a slice folder that is not live any more (.old/.prev/.new/.stage). When wu-play still serves it (it was
# renamed away under the running container), wu-play is restarted onto the live path first.
slice_retire() {
  local d=${1%/}
  [ -e "$d" ] || return 0
  if play_serves "$d"; then
    docker restart wu-play >/dev/null 2>&1 && echo "  wu-play restarted onto the live slice (it still served ${d##*/})"
  fi
  rm -rf --one-file-system -- "$d"
}

# slice_restage <slice dir> <command> [args...]: runs `<command> [args...] <dir>.stage` on a hard-link copy of the
# slice, checks the copy, and renames it over the live folder. A failed command or check leaves the live slice
# exactly as it was (the copy is dropped). A folder that is itself a copy (.stage/.new) is changed directly.
# The replaced folder is kept as <dir>.prev until slice_swapped_done (after the caller restarted wu-play).
SLICE_SWAPPED=""
slice_restage() {
  local s=${1%/}; shift
  case "$s" in *.stage|*.new) "$@" "$s"; return ;; esac
  [ -d "$s" ] || { echo "  no slice folder $s" >&2; return 1; }
  slice_lock
  local stg="$s.stage" prev="$s.prev"
  slice_retire "$stg"
  cp -al -- "$s" "$stg" || { rm -rf -- "$stg"; echo "  could not copy $s (hard links): nothing changed" >&2; return 1; }
  if ! "$@" "$stg"; then
    rm -rf -- "$stg"; echo "  the change failed on a copy: ${s##*/} is as it was" >&2; return 1
  fi
  if ! slice_verify "$stg" >"$ST/.slice-verify.log" 2>&1; then
    sed 's/^/  /' "$ST/.slice-verify.log" >&2
    rm -rf -- "$stg"; echo "  the changed copy is not a whole slice: ${s##*/} is as it was" >&2; return 1
  fi
  slice_retire "$prev"
  mv -T -- "$s" "$prev" || { rm -rf -- "$stg"; return 1; }
  if ! mv -T -- "$stg" "$s"; then mv -T -- "$prev" "$s"; rm -rf -- "$stg"; return 1; fi
  SLICE_SWAPPED="$SLICE_SWAPPED $prev"
}
# Undo every slice_restage of this run: each .prev (untouched) goes back into place, the changed folder is dropped.
slice_swapped_undo() {
  local p live
  for p in $SLICE_SWAPPED; do
    live=${p%.prev}
    [ -d "$p" ] || continue
    if [ -d "$live" ]; then mv -T -- "$live" "$live.stage"; fi
    mv -T -- "$p" "$live" && echo "  ${live##*/}: back as it was"
    SLICE_SWAPPED="$SLICE_SWAPPED $live.stage"
    SLICE_SWAPPED=${SLICE_SWAPPED/ $p/}
  done
}
# After the caller restarted wu-play (or chose not to): the replaced folders go (slice_retire restarts wu-play
# first if it still serves one of them).
slice_swapped_done() {
  local p
  for p in $SLICE_SWAPPED; do slice_retire "$p"; done
  SLICE_SWAPPED=""
}

# The streamed client page takes the login from /__env.js (the gate), never from the URL.
patch_index() {
  local IDX="$ST/www/index.html"
  # The dev page lets ?user=&pass= links log in; the streamed page never takes credentials from the URL.
  sed -i "s/window.__wenilla_env = { dev_query_creds: '1' };/window.__wenilla_env = {};/" "$IDX"
  if ! grep -q 'src="/__env.js"' "$IDX"; then
    python3 -I - "$IDX" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
anchor = '<script type="module">'
if anchor not in s: sys.exit("no module script in index.html")
tag = '<script src="/__env.js"></script>\n'
s = s.replace(anchor, tag + anchor, 1)
open(p, "w").write(s)
PY
  fi
  # A precompressed copy of the old page would be served instead of the edited one.
  rm -f "$IDX.br" "$IDX.gz"
  # The client (~90 MB wasm) goes out precompressed: wu-play serves .br/.gz siblings when the
  # browser accepts them (about 4x smaller), and nothing is compressed per request.
  local f
  for f in "$ST"/www/*.wasm "$ST"/www/*.js; do
    [ -f "$f" ] || continue
    if [ ! -f "$f.gz" ] || [ "$f" -nt "$f.gz" ]; then gzip -9 -k -f "$f"; fi
    if command -v brotli >/dev/null && { [ ! -f "$f.br" ] || [ "$f" -nt "$f.br" ]; }; then brotli -q 9 -k -f "$f"; fi
  done
  # "Play with my own game files" + "Download this section" (www-files, from the kit).
  if [ -d "$ST/www-files" ] && [ -f "$ST/www-patch.py" ]; then
    python3 -I "$ST/www-patch.py" "$ST/www" "$ST/www-files" \
      || echo "  WARNING: www-patch.py could not edit this client build: no own-files mode or section downloads (see above)"
  fi
}
