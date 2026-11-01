#!/usr/bin/env bash
# Install a slice sent as a tar stream on stdin (from "Make mini slice.bat" / "Upload a mini slice.bat"):
#   tar -cf - -C work mini.json mini | ssh root@server "bash /srv/wrath/stream/slice-install.sh mini <bytes>"
# (mini.json, the slice description with the maxTier that fit, is optional in the tar)
#   bash slice-install.sh --rollback mini     put the previous slice (slices/mini.old) back
#
# Unpacks to slices/<name>.new, lays Forever UI and the mods over it (slices/<name>.stage), checks it
# (slice-health.sh --verify: the key files and the slice's own tier lists), then renames it into place. The
# previous slice becomes slices/<name>.old; wu-play is restarted and asked for a key file, and a wrong answer puts
# the previous slice back. The .old copy stays for "--rollback" when the disk has room for it (WU_KEEP_FREE_MB free
# besides, default 2048, + WU_KEEP_OLD_MARGIN_MB, default 1024), and goes first when the next upload needs the room.
# Then the realm's tier list and Dungeon Finder ids (ov_slices_tiers.lua, ov_slices_lfg.lua) are refreshed.
# With too little disk for old + new side by side, the old slice goes first (the stream is down for the upload);
# it is renamed away before it is deleted, so the served path never shows a half-deleted slice.
# One slice change at a time (slices/.lock). Incident 1 Nov 2026: docs/INCIDENT-2026-11-01-SLICE.md.
set -euo pipefail
. "$(dirname "$(readlink -f "$0")")/stream-lib.sh"
ROLLBACK=""
if [ "${1:-}" = "--rollback" ]; then ROLLBACK=1; shift; fi
NAME="${1:-}"; BYTES="${2:-0}"
[[ "$NAME" =~ ^[a-z0-9-]{1,32}$ ]] || die "usage: slice-install.sh <slice name> [bytes]   |   slice-install.sh --rollback <slice name>"
[[ "$BYTES" =~ ^[0-9]+$ ]] || BYTES=0
# The game data killswitch (gamedata-switch.sh): no game data goes onto this server while it is on.
[ -f "$WU/portal/gamedata-off" ] && die "game data is switched off on this server (gamedata-switch.sh): no slice upload. To stream again: bash $ST/gamedata-switch.sh on"
D="$ST/slices"; mkdir -p "$D"
chmod go-w "$ST" "$D" 2>/dev/null || true   # a kit unpacked from Windows left them writable by everyone (1 Nov)
slice_lock
LIVE="$D/$NAME"; OLD="$D/$NAME.old"
free_bytes() { echo $(( $(df --output=avail -k "$D" | tail -1) * 1024 )); }

play_running() { docker ps -a --format '{{.Names}}' 2>/dev/null | grep -qx wu-play; }

# The served slice changed: realm tables, wu-play. $1 = "install" | "rollback"
serve_it() {
  if [ -f "$D/$NAME.json" ] && [ -d "$WU/portal/slices" ]; then install -m 644 "$D/$NAME.json" "$WU/portal/slices/$NAME.json"; fi
  served=$(grep -E '^STREAM_SLICES=' "$WU/portal.env" 2>/dev/null | tail -1 | cut -d= -f2 | tr -d '\r"' || true)
  served=${served:-mini}
  case ",$served," in
    *",$NAME,"*) ;;
    *)
      if [ "${WU_SLICE_SERVE:-}" = 1 ] && [ "$1" = install ]; then
        exec bash "$ST/slice-serve.sh" "$NAME"
      fi
      echo "  slice $NAME is installed beside the served one ($served): the realm rule and wu-play are unchanged"
      echo "  to serve it: bash $ST/slice-serve.sh $NAME   (back: bash $ST/slice-serve.sh $served)"
      return 3 ;;
  esac
  if [ -f "$D/$NAME.json" ] && [ -f "$WU/custom/lua_scripts/ov_slices.lua" ]; then
    if realm_tables "$D/$NAME.json" "$LIVE"; then
      soap "reload ale" >/dev/null 2>&1 && echo "  realm rule reloaded" || echo "  WARNING: reload ale failed: the realm keeps the old tier list until it restarts"
    else
      echo "  WARNING: $NAME.json's tier list is broken: the realm keeps the old one"
    fi
  fi
  if play_running; then
    docker restart wu-play >/dev/null && echo "  wu-play restarted on the slice"
  elif grep -qF "$MARK_BEGIN" "$CF" 2>/dev/null; then
    # Streaming was set up before (the Caddy block is there): wu-play is gone because this script removed it to make
    # room (below), so start it again now. Live 1 Nov: only a hint was printed and the browser game answered 502.
    echo "  wu-play is not running: starting it (stream-up.sh)"
    if (cd "$ST" && WU_SLICE="$NAME" bash stream-up.sh) >/tmp/wu-stream-up.log 2>&1; then
      echo "  wu-play started (stream-up.sh OK)"
    else
      tail -20 /tmp/wu-stream-up.log | sed 's/^/    /'
      die "stream-up.sh failed: the browser game is down until it runs (bash $ST/stream-up.sh)"
    fi
  else
    echo "  next: bash $ST/stream-up.sh"
    return 3
  fi
  return 0
}

# The live folder goes to $2 and $1 (a whole copy) takes its place: two renames, no copying.
swap_in() {   # <new dir> <where the live one goes>
  if [ -d "$LIVE" ]; then mv -T -- "$LIVE" "$2"; fi
  mv -T -- "$1" "$LIVE" || { [ -d "$2" ] && mv -T -- "$2" "$LIVE"; die "could not rename $1 into place: the previous slice is back"; }
}

if [ -n "$ROLLBACK" ]; then
  [ -d "$OLD" ] || die "no previous slice ($OLD) to go back to"
  echo "  checking the previous slice $OLD"
  slice_verify "$OLD" || die "the previous slice is not whole either: nothing changed"
  BAD="$D/$NAME.prev"; slice_retire "$BAD"
  swap_in "$OLD" "$BAD"
  if [ -f "$D/$NAME.json.old" ]; then
    [ -f "$D/$NAME.json" ] && mv -f "$D/$NAME.json" "$D/$NAME.json.prev"
    mv -f "$D/$NAME.json.old" "$D/$NAME.json"
    [ -f "$D/$NAME.json.prev" ] && mv -f "$D/$NAME.json.prev" "$D/$NAME.json.old"
  fi
  # the one rolled back from stays as .old (whole or not), so this can be undone the same way
  [ -d "$BAD" ] && mv -T -- "$BAD" "$OLD"
  echo "  slice $NAME: the previous one is back ($(find "$LIVE" -type f | wc -l) files); the one before is $OLD"
  serve_it rollback || true
  echo "SLICE ROLLBACK OK"
  exit 0
fi

# Leftovers of an earlier run that stopped halfway (any slice name; wu-play is moved off one it still serves) and the
# client kept for rollback. The .old copies (the previous slice of each name, kept for --rollback) stay for now.
for x in "$D"/*.new "$D"/*.stage "$D"/*.prev "$D"/*.going; do slice_retire "$x"; done
rm -rf "$ST/www.old"
# Room for the new copy plus 2 GB kept free for the database, logs and Docker.
KEEP_FREE=$(( ${WU_KEEP_FREE_MB:-2048} * 1024 * 1024 ))
need=$(( BYTES + BYTES / 20 + KEEP_FREE ))
avail=$(free_bytes)
if [ "$BYTES" -gt 0 ] && [ "$avail" -lt "$need" ]; then
  # Would removing the previous copies and the live slice even be enough? If not, nothing is touched.
  kb() { du -sk "$@" 2>/dev/null | awk '{s += $1} END {print s * 1024 + 0}'; }
  olds=(); [ -d "$OLD" ] && olds+=("$OLD")
  for x in "$D"/*.old; do [ -d "$x" ] && [ "$x" != "$OLD" ] && olds+=("$x"); done
  room=$(( avail + $( [ ${#olds[@]} -gt 0 ] && kb "${olds[@]}" || echo 0) + $( [ -d "$LIVE" ] && kb "$LIVE" || echo 0) ))
  [ "$room" -ge "$need" ] \
    || die "not enough disk even without the old slice: $((room/1048576)) MB at most, $((need/1048576)) MB needed. Nothing changed: the old slice keeps serving"
  # First the rollback copies (this name's, then any other's), which are never served.
  for x in "${olds[@]}"; do
    [ "$avail" -lt "$need" ] || break
    [ -d "$x" ] || continue
    echo "  making room: removing the previous copy ${x##*/}"
    slice_retire "$x"; avail=$(free_bytes)
  done
fi
if [ "$BYTES" -gt 0 ] && [ "$avail" -lt "$need" ] && [ -d "$LIVE" ]; then
  echo "  not enough disk for both copies ($((avail/1048576)) MB free, $((need/1048576)) MB needed): removing the old slice first"
  docker rm -f wu-play >/dev/null 2>&1 || true
  # renamed first: an interrupted delete leaves a .going folder to clean up, never a half slice at the served path
  mv -T -- "$LIVE" "$OLD.going"
  rm -rf --one-file-system -- "$OLD.going"
  avail=$(free_bytes)
fi
[ "$BYTES" = 0 ] || [ "$avail" -ge "$need" ] || die "not enough disk: $((avail/1048576)) MB free, $((need/1048576)) MB needed"
NEW="$D/$NAME.new"; STAGE="$D/$NAME.stage"
mkdir "$NEW"
trap 'rm -rf -- "$NEW" "$STAGE"' EXIT
tar -x -C "$NEW" -f - --no-same-owner --no-same-permissions
# the tar holds one top folder (the slice); take its contents
inner="$NEW/$NAME"
[ -d "$inner" ] || inner=$(find "$NEW" -mindepth 1 -maxdepth 1 -type d | head -1)
[ -n "$inner" ] && [ -d "$inner" ] || die "the upload held no slice folder"
find "$inner" -maxdepth 2 -iname DBFilesClient | grep -c . >/dev/null || die "the upload is not an exported slice (no DBFilesClient)"
# The tar may carry the slice description next to the folder (<name>.json): one SSH connection. It goes in only
# together with the slice (below).
NEWJSON=""
if [ -f "$NEW/$NAME.json" ]; then
  python3 -I "$ST/lua/tiers_lua.py" "$NEW/$NAME.json" > /dev/null || die "the uploaded $NAME.json's tier list is broken: nothing changed"
  NEWJSON="$D/$NAME.json.new"; mv -f "$NEW/$NAME.json" "$NEWJSON"
fi
mv -T "$inner" "$STAGE"; rm -rf "$NEW"
chmod -R a+rX "$STAGE"
# Forever UI, when the server has it (fui-install.sh), goes back over the new slice before the swap.
if [ -f "$ST/fui-install.sh" ]; then
  bash "$ST/fui-install.sh" reapply-slice "$NAME" "$STAGE" || echo "  WARNING: Forever UI could not be laid over the new slice: it streams with the stock interface (bash $ST/fui-install.sh status)"
fi
# The catalog mods' addons (mods-install.sh), when the server has them, go back over it too.
if [ -f "$ST/mods-install.sh" ]; then
  bash "$ST/mods-install.sh" reapply-slice "$NAME" "$STAGE" || echo "  WARNING: the mods' addons could not be laid over the new slice (bash $ST/mods-install.sh status)"
fi
# Whole? (the key files and the slice's own tier lists). Not whole: nothing changes, the old slice keeps serving.
echo "  checking the new slice"
if ! slice_verify "$STAGE"; then
  rm -f "$NEWJSON"
  die "the uploaded slice is not whole (above): nothing changed$([ -d "$LIVE" ] && echo ', the old slice keeps serving' || echo '; there is no slice on the server now, upload it again')"
fi
slice_retire "$OLD"
swap_in "$STAGE" "$OLD"
trap - EXIT
if [ -n "$NEWJSON" ]; then
  [ -f "$D/$NAME.json" ] && cp -p "$D/$NAME.json" "$D/$NAME.json.old"
  mv -f "$NEWJSON" "$D/$NAME.json"
fi
echo "  slice $NAME: $(du -sh "$LIVE" | cut -f1), $(find "$LIVE" -type f | wc -l) files"
rc=0; serve_it install || rc=$?
if [ "$rc" = 0 ] && play_running; then
  # wu-play is on the new slice: does it answer for a key file? A wrong answer puts the previous slice back.
  pr=0; play_probe || pr=$?
  if [ "$pr" = 0 ]; then echo "  wu-play answers 200 for DBFilesClient\\Spell.dbc"
  elif [ "$pr" = 1 ] && [ -d "$OLD" ]; then
    echo "  wu-play answers $PLAY_PROBE for DBFilesClient\\Spell.dbc on the new slice: putting the previous one back"
    exec bash "$0" --rollback "$NAME"
  else echo "  WARNING: could not confirm wu-play's answer (${PLAY_PROBE:-?}): bash $ST/slice-health.sh"; fi
fi
# Keep the previous slice for --rollback only when the disk has room to spare.
if [ -d "$OLD" ]; then
  if [ "$(free_bytes)" -ge $(( KEEP_FREE + ${WU_KEEP_OLD_MARGIN_MB:-1024} * 1024 * 1024 )) ]; then
    echo "  previous slice kept as $OLD (back to it: bash $ST/slice-install.sh --rollback $NAME)"
  else
    slice_retire "$OLD"; rm -f "$D/$NAME.json.old"; echo "  previous slice removed (no room to keep it)"
  fi
fi
echo "  disk free now: $(df -h --output=avail "$D" | tail -1 | tr -d ' ')"
