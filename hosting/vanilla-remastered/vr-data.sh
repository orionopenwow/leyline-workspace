#!/usr/bin/env bash
# Vanilla Remastered: the 1.12 game data on the server again, from the owner's own client (owner 2026-10-31:
# "Vanilla Remastered ... minimal slices for 1-20 leveling ... compact for the server"). This morning's
# ov112-data-remove emptied the world server's data (volume wenilla-realm_gamedata) and the old browser stream
# (/srv/client/Data). This puts back only what is needed:
#   * the world server's data (dbc, maps, vmaps; mmaps only with VR_MMAPS=1), extracted on this box from the owner's
#     uploaded archives with the CMaNGOS extractors found in the realm's own images, into wenilla-realm_gamedata,
#     then the Classic+ server tables (/opt/openvanilla-classicplus/dbc) laid over them as before;
#   * the 1.12 browser slice for levels 1-20 (hosting/vanilla-remastered/slice-112.json, vr-slice.py): new archives
#     in /srv/vr112/slice/Data holding only the slice, served by the Vanilla Remastered page (vr-site-up.sh);
#   * compact world server settings (no random bots unless VR_BOTS, grids unload, no movement maps unless built),
#     the login server's wrong-password lock off (every relay and browser player arrives from one address), and
#     memory limits on the containers;
# and then deletes the uploaded archives. Run by "Vanilla Remastered data.bat" (vr-data.ps1 uploads the archives
# first, resumable, checksummed) and "Vanilla Remastered data - check.bat" (read-only).
#
#   bash vr-data.sh <kit dir> check [<upload MB>]   read-only: disk, memory, volumes, the extractors, what is done
#   bash vr-data.sh <kit dir> recv-status            the upload folder (name, size, part size, checked sha256)
#   bash vr-data.sh <kit dir> recv-chunk NAME OFFSET  stdin appended to NAME.part at OFFSET (truncated there first)
#   bash vr-data.sh <kit dir> recv-finish NAME SHA256 SIZE   the part checked and renamed
#   bash vr-data.sh <kit dir> run                    start the job in the background (or follow the one running)
#   bash vr-data.sh <kit dir> job                    the job itself (phases below; each one is skipped once done)
#
# Phases (markers in /srv/vr112/state; a second run goes on where the first stopped):
#   1 checks     the archives are all there and checked; the old 1.12 containers and vr-mangosd are stopped (the
#                world server must not run while its data is replaced); disk
#   2 extract    dbc + maps (ad), vmaps (vmap_extractor + vmap_assembler), mmaps only with VR_MMAPS=1; into the
#                volume; Buildings (the temporary vmap source) deleted at once
#   3 classic+   the Classic+ server tables over the stock ones (the stock files kept as .orig)
#   4 slice      the realm's spawns (classicmangos, read-only), vr-slice.py, the report
#   5 settings   compact settings (backup of every file first); memory limits
#   6 clean      the uploaded archives deleted (the slice and the extracted data stay)
# Last line: "VR DATA OK" / "VR DATA CHECK OK", or "VR DATA FAILED: ..." (exit 1). Nothing secret is printed.
set -uo pipefail
KIT="${1:?usage: vr-data.sh <kit dir> <mode>}"
MODE="${2:-check}"
VR_ROOT="${VR_ROOT:-/srv/vr112}"
UP="$VR_ROOT/upload"
STATE="$VR_ROOT/state"
SLICE_ROOT="$VR_ROOT/slice"
LOGF="$VR_ROOT/vr-data.log"
CP="${VR_CP:-/opt/openvanilla-classicplus}"
GD_VOL="${VR_GAMEDATA_VOL:-wenilla-realm_gamedata}"
DB="${VR_DB:-wenilla-realm-mariadb-1}"
OLD_LOGON="${VR_OLD_LOGON:-wenilla-realm-realmd-1}"
OLD_WORLD="${VR_OLD_WORLD:-wenilla-realm-mangosd-1}"
WORLD=vr-mangosd
LOGON=vr-realmd
SITE_C=vr-site
TOOLS_IMG="${VR_TOOLS_IMG:-vr-mpq-tools:1}"
UNIT="${VR_UNIT:-vr-data}"
VR="$KIT/hosting/vanilla-remastered"
REQUIRED="${VR_REQUIRED:-dbc.MPQ model.MPQ texture.MPQ terrain.MPQ wmo.MPQ interface.MPQ misc.MPQ fonts.MPQ sound.MPQ speech.MPQ patch.MPQ}"
BOTS="${VR_BOTS:-0}"
WORLD_MEM="${VR_WORLD_MEM:-1100m}"
LOGON_MEM="${VR_LOGON_MEM:-96m}"
log() { printf '\n== %s\n' "$*"; }
note() { printf '  %s\n' "$*"; }
die() { printf '\nVR DATA FAILED: %s\n' "$*"; exit 1; }
running() { docker ps --format '{{.Names}}' | grep -x "$1" >/dev/null; }
exists() { docker ps -a --format '{{.Names}}' | grep -x "$1" >/dev/null; }
done_() { [ -f "$STATE/$1.done" ]; }
mark() { date -u +%Y-%m-%dT%H:%M:%SZ > "$STATE/$1.done"; }
free_mb() { df -Pm "${1:-$VR_ROOT}" 2>/dev/null | awk 'NR==2 {print $4}'; }
mem_mb() { awk '/MemAvailable/ {print int($2/1024)}' /proc/meminfo 2>/dev/null || echo 0; }
gd_path() { docker volume inspect -f '{{.Mountpoint}}' "$GD_VOL" 2>/dev/null; }
safe_name() { [[ "$1" =~ ^[A-Za-z0-9_-]{1,40}\.[Mm][Pp][Qq]$ ]]; }
find_ci() { find "$1" -maxdepth 1 -iname "$2" -print -quit 2>/dev/null; }   # find_ci DIR NAME: the file, any case

[ "${VR_TEST:-0}" = 1 ] || [ "$(id -u)" = 0 ] || die "run as root"
command -v docker >/dev/null || die "docker not found"
( umask 077; mkdir -p "$UP" "$STATE" ) || die "could not make $VR_ROOT"
mkdir -p "$SLICE_ROOT"

# ---------------------------------------------------------------------------------------------- the upload helpers
case "$MODE" in
  recv-status)
    for f in "$UP"/*; do
      [ -e "$f" ] || continue
      b=$(basename "$f")
      case "$b" in
        *.part) printf 'part\t%s\t%s\n' "${b%.part}" "$(stat -c %s "$f")" ;;
        *.sha256) ;;
        *) printf 'file\t%s\t%s\t%s\n' "$b" "$(stat -c %s "$f")" "$(cat "$f.sha256" 2>/dev/null | cut -c1-64)" ;;
      esac
    done
    printf 'free\t%s\n' "$(free_mb "$UP")"
    done_ clean && printf 'cleaned\t%s\n' "$(cat "$STATE/clean.done")"   # extracted already: no upload needed
    exit 0 ;;
  recv-chunk)
    n="${3:-}"; off="${4:-}"
    safe_name "$n" || die "bad name"
    [[ "$off" =~ ^[0-9]+$ ]] || die "bad offset"
    p="$UP/$n.part"; touch "$p"; chmod 600 "$p"
    have=$(stat -c %s "$p")
    [ "$have" -ge "$off" ] || die "the part is $have bytes, shorter than offset $off"
    truncate -s "$off" "$p"
    cat >> "$p" || die "the chunk did not arrive"
    echo "PART $(stat -c %s "$p")"
    exit 0 ;;
  recv-finish)
    n="${3:-}"; want="${4:-}"; size="${5:-}"
    safe_name "$n" || die "bad name"
    [[ "$want" =~ ^[0-9a-fA-F]{64}$ ]] || die "bad checksum"
    p="$UP/$n.part"
    [ -f "$p" ] || die "no part for $n"
    [ "$(stat -c %s "$p")" = "$size" ] || die "$n: $(stat -c %s "$p") bytes, want $size"
    got=$(sha256sum "$p" | cut -c1-64)
    if [ "${got,,}" != "${want,,}" ]; then rm -f "$p"; die "$n: the checksum does not match (the part was deleted; run the step again)"; fi
    mv -f "$p" "$UP/$n"; echo "$got" > "$UP/$n.sha256"; chmod 600 "$UP/$n" "$UP/$n.sha256"
    echo "$n" >> "$STATE/uploaded.txt"; sort -u -o "$STATE/uploaded.txt" "$STATE/uploaded.txt"
    echo "CHECKED $n"
    exit 0 ;;
esac

# ---------------------------------------------------------------------------------------------- the extractors
# CMaNGOS Classic's own tools, from the images the realm already runs (never pulled): ad (dbc + maps),
# vmap_extractor, vmap_assembler, MoveMapGen. Prints "IMAGE<TAB>tool=path ..." for the first image that has them.
find_tools() {
  local imgs=() i out
  for c in "$WORLD" "$OLD_WORLD"; do
    exists "$c" && imgs+=("$(docker inspect -f '{{.Image}}' "$c" 2>/dev/null)")
  done
  while read -r i; do [ -n "$i" ] && imgs+=("$i"); done < <(docker images --format '{{.Repository}}:{{.Tag}}' 2>/dev/null | grep -Ei 'mangos|wenilla|extract' | grep -v '<none>')
  for i in "${imgs[@]}"; do
    [ -n "$i" ] || continue
    out=$(docker run --rm --network none --entrypoint sh "$i" -c '
      for n in ad mapextractor map-extractor vmap_extractor vmapextractor vmap_assembler vmapassembler MoveMapGen movemapgen; do
        p=$(command -v "$n" 2>/dev/null || find / -xdev -type f -name "$n" -perm -u+x 2>/dev/null | head -1)
        [ -n "$p" ] && printf "%s=%s " "$n" "$p"
      done' 2>/dev/null)
    case "$out" in *vmap_assembler=*|*vmapassembler=*) printf '%s\t%s\n' "$i" "$out"; return 0 ;; esac
  done
  return 1
}
tool() { tr ' ' '\n' <<<"$2" | sed -n "s/^$1=//p" | head -1; }

# ---------------------------------------------------------------------------------------------- check (read-only)
report() {
  log "Vanilla Remastered data: what is here"
  local gd; gd=$(gd_path)
  note "disk free: $(free_mb "$VR_ROOT") MB on $VR_ROOT; memory available: $(mem_mb) MB"
  note "upload folder $UP: $(du -sh "$UP" 2>/dev/null | cut -f1) ($(ls "$UP"/*.[Mm][Pp][Qq] 2>/dev/null | wc -l) checked archives)"
  if [ -n "$gd" ]; then
    note "world server data ($GD_VOL): $(du -sh "$gd" 2>/dev/null | cut -f1)"
    for d in dbc maps vmaps mmaps; do note "  $d: $(ls "$gd/$d" 2>/dev/null | wc -l) files"; done
  else
    note "volume $GD_VOL: NOT FOUND"
  fi
  note "browser slice $SLICE_ROOT/Data: $(du -sh "$SLICE_ROOT/Data" 2>/dev/null | cut -f1 || echo none)"
  [ -f "$CP/patch-Z.MPQ" ] && note "our Classic+ archive: $CP/patch-Z.MPQ ($(du -h "$CP/patch-Z.MPQ" | cut -f1))" || note "no $CP/patch-Z.MPQ (the slice goes without the Classic+ client content)"
  for t in SkillRaceClassInfo CreatureModelData CreatureDisplayInfo; do
    [ -f "$CP/dbc/$t.dbc" ] && note "Classic+ server table ready: $t.dbc"
  done
  for c in "$OLD_LOGON" "$OLD_WORLD" "$LOGON" "$WORLD" "$SITE_C"; do
    if exists "$c"; then note "$c: $(running "$c" && echo RUNNING || echo stopped)"; else note "$c: not here"; fi
  done
  local t
  if t=$(find_tools); then note "extractors: image ${t%%$'\t'*}: ${t#*$'\t'}"
  else note "extractors: NOT FOUND in the realm's images (the job would stop at phase 2)"; fi
  if [ -f "$OLD_DIR_REALMCTL" ]; then note "realmctl's extract (for reference):"; grep -n -i 'extract' "$OLD_DIR_REALMCTL" | head -8 | sed 's/^/    /'; fi
  for p in "${PHASES[@]}"; do note "phase $p: $(done_ "$p" && echo "done $(cat "$STATE/$p.done")" || echo 'to do')"; done
  if systemctl is-active --quiet "$UNIT" 2>/dev/null; then note "the job is RUNNING (systemd unit $UNIT; log $LOGF)"; fi
}
PHASES=(extract classicplus slice settings mmaps clean)
OLD_DIR_REALMCTL="${VR_REALMCTL:-/opt/wenilla-realm/realmctl}"

if [ "$MODE" = check ]; then
  report
  need="${3:-0}"; [[ "$need" =~ ^[0-9]+$ ]] || need=0
  done_ clean && need=0   # the archives were used and deleted: nothing is uploaded again
  have=$(free_mb "$VR_ROOT")
  # the peak: the archives + the extracted data (~1.3 GB) + the larger of the temporary vmap source (~1.5 GB, deleted
  # before the slice is made) and the slice (budgetMB), + a margin
  sl="${VR_SLICE_MB:-${VR_BUDGET_MB:-2800}}"; mg="${VR_DISK_MARGIN_MB:-1500}"
  want=$(( need + 1300 + (sl > 1500 ? sl : 1500) + mg ))
  if done_ extract; then want=$(( need + sl + mg )); fi
  log "Disk"
  note "needed for an upload of ${need} MB and the rest of the job: about $want MB; free: $have MB"
  if [ "${have:-0}" -lt "$want" ]; then
    echo "VR DATA FAILED: not enough disk (free $have MB, needs about $want MB). Nothing was changed."; exit 1
  fi
  echo; echo "VR DATA CHECK OK"; exit 0
fi

# ---------------------------------------------------------------------------------------------- run (background)
if [ "$MODE" = run ]; then
  if ! systemctl is-active --quiet "$UNIT" 2>/dev/null; then
    [ -f "$LOGF" ] && mv -f "$LOGF" "$LOGF.prev"
    : > "$LOGF"; chmod 600 "$LOGF"
    # a copy of the kit the job keeps, so a dropped SSH session doesn't take its files away
    rm -rf "$VR_ROOT/kit"; cp -a "$KIT" "$VR_ROOT/kit"
    env_args=()
    for v in VR_MMAPS VR_BOTS VR_WORLD_MEM VR_LOGON_MEM VR_BUDGET_MB VR_KEEP_UPLOAD; do
      [ -n "${!v:-}" ] && env_args+=(--setenv="$v=${!v}")
    done
    systemd-run --unit="$UNIT" --collect --quiet "${env_args[@]}" -p StandardOutput="append:$LOGF" -p StandardError="append:$LOGF" \
      bash "$VR_ROOT/kit/hosting/vanilla-remastered/vr-data.sh" "$VR_ROOT/kit" job \
      || die "could not start the job (systemd-run)"
    note "the job runs on the server (unit $UNIT); this window follows its log. If the connection drops, run the step again: it follows the same job."
  else
    note "the job is already running: following it"
  fi
  tail -n +1 -F "$LOGF" 2>/dev/null &
  T=$!
  while systemctl is-active --quiet "$UNIT" 2>/dev/null; do sleep 5; done
  sleep 2; kill "$T" 2>/dev/null; wait "$T" 2>/dev/null
  tail -1 "$LOGF" | grep -q '^VR DATA OK' && exit 0
  exit 1
fi

[ "$MODE" = job ] || die "usage: vr-data.sh <kit dir> check|recv-status|recv-chunk|recv-finish|run|job"

# ---------------------------------------------------------------------------------------------- the job
echo "Vanilla Remastered data job, $(date -u +%Y-%m-%dT%H:%M:%SZ)"
GD=$(gd_path); [ -n "$GD" ] && [ -d "$GD" ] || die "the volume $GD_VOL is not on this box"

log "1/6 Checks"
for n in $REQUIRED; do
  f=$(find_ci "$UP" "$n")
  [ -n "$f" ] && [ -f "$f.sha256" ] || done_ clean || die "the archive $n is not uploaded and checked yet (run the step from the PC again)"
done
for c in "$OLD_LOGON" "$OLD_WORLD"; do running "$c" && die "$c is running: the old 1.12 containers must stay stopped"; done
if ! done_ extract || ! done_ classicplus; then
  running "$WORLD" && die "$WORLD is running: its data can't be replaced under it. Stop it first (the ov112-realm-off step), then run this again."
fi
note "archives: $(ls "$UP"/*.[Mm][Pp][Qq] 2>/dev/null | wc -l) checked; disk free $(free_mb "$VR_ROOT") MB; memory available $(mem_mb) MB"
if [ -f "$CP/patch-Z.MPQ" ] && [ ! -f "$UP/patch-Z.MPQ" ] && ! done_ clean; then
  cp -p "$CP/patch-Z.MPQ" "$UP/patch-Z.MPQ" && note "our Classic+ archive joined the upload folder (for the server tables and the slice)"
fi

if ! done_ extract; then
  log "2/6 Extract the world server's data (CMaNGOS Classic's own extractors)"
  TL=$(find_tools) || die "no CMaNGOS extractors (ad, vmap_extractor, vmap_assembler) in the realm's images. Nothing was changed."
  IMG=${TL%%$'\t'*}; T=${TL#*$'\t'}
  AD=$(tool ad "$T"); [ -n "$AD" ] || AD=$(tool mapextractor "$T"); [ -n "$AD" ] || AD=$(tool map-extractor "$T")
  VE=$(tool vmap_extractor "$T"); [ -n "$VE" ] || VE=$(tool vmapextractor "$T")
  VA=$(tool vmap_assembler "$T"); [ -n "$VA" ] || VA=$(tool vmapassembler "$T")
  MM=$(tool MoveMapGen "$T"); [ -n "$MM" ] || MM=$(tool movemapgen "$T")
  [ -n "$AD" ] && [ -n "$VE" ] && [ -n "$VA" ] || die "the image $IMG lacks one of ad / vmap_extractor / vmap_assembler"
  note "image $IMG: ad=$AD vmap_extractor=$VE vmap_assembler=$VA ${MM:+MoveMapGen=$MM}"
  if [ -n "$(ls -A "$GD" 2>/dev/null | grep -v '^\.')" ]; then
    B="$VR_ROOT/gamedata-before-$(date +%Y%m%d-%H%M%S)"; mkdir -p "$B"
    note "the volume is not empty: what was there moves to $B first (kept)"
    find "$GD" -mindepth 1 -maxdepth 1 -exec mv -t "$B" -- {} + || die "could not move the old data aside"
  fi
  W="$VR_ROOT/work"; rm -rf "$W"; mkdir -p "$W"
  rt() {  # rt BIN ARGS: one extractor run, no network, the archives read-only
    docker run --rm --network none --memory "${VR_EXTRACT_MEM:-1536m}" --cpus "${VR_EXTRACT_CPUS:-1.5}" --user 0 \
      -v "$UP:/work/client/Data:ro" -v "$GD:/out" -v "$W:/work/tmp" -w /work/tmp --entrypoint "$1" "$IMG" "${@:2}"
  }
  t0=$(date +%s)
  quiet() { grep -vE '^\s*(Processing|Extract|Reading|Loading|[0-9]+%|.*\.(adt|wmo|m2|mdx|dbc)\b)' | tail -12; }
  rt "$AD" -i /work/client -o /out 2>&1 | quiet || true
  if [ -z "$(ls "$GD/dbc" 2>/dev/null)" ]; then   # some builds of ad take the Data folder itself
    note "(ad found no archives under /work/client/Data; trying the Data folder itself)"
    rt "$AD" -i /work/client/Data -o /out 2>&1 | quiet || true
  fi
  [ -n "$(ls "$GD/maps" 2>/dev/null)" ] && [ -n "$(ls "$GD/dbc" 2>/dev/null)" ] || die "ad wrote no maps or dbc (see above)"
  note "dbc: $(ls "$GD/dbc" | wc -l) tables; maps: $(ls "$GD/maps" | wc -l) files ($(( $(date +%s) - t0 )) s)"
  t0=$(date +%s)
  rt "$VE" -d /work/client 2>&1 | quiet || true
  if [ -z "$(ls "$W/Buildings" 2>/dev/null)" ]; then
    note "(vmap_extractor found no archives under /work/client/Data; trying the Data folder itself)"
    rm -rf "$W/Buildings"; rt "$VE" -d /work/client/Data 2>&1 | quiet || true
  fi
  [ -n "$(ls "$W/Buildings" 2>/dev/null)" ] || die "vmap_extractor made no Buildings folder (see above)"
  mkdir -p "$GD/vmaps"
  rt "$VA" Buildings /out/vmaps 2>&1 | tail -5 || true
  rm -rf "$W/Buildings"
  [ -n "$(ls "$GD/vmaps" 2>/dev/null)" ] || die "vmap_assembler wrote no vmaps (see above)"
  note "vmaps: $(ls "$GD/vmaps" | wc -l) files ($(( $(date +%s) - t0 )) s)"
  if false; then   # movement maps: their own phase below
    t0=$(date +%s); mkdir -p "$GD/mmaps"
    docker run --rm --network none --memory "${VR_EXTRACT_MEM:-1536m}" --cpus "${VR_EXTRACT_CPUS:-1.5}" --user 0 \
      -v "$GD:/work/tmp" -w /work/tmp --entrypoint "$MM" "$IMG" 0 --threads 2 2>&1 | tail -3 || true
    docker run --rm --network none --memory "${VR_EXTRACT_MEM:-1536m}" --cpus "${VR_EXTRACT_CPUS:-1.5}" --user 0 \
      -v "$GD:/work/tmp" -w /work/tmp --entrypoint "$MM" "$IMG" 1 --threads 2 2>&1 | tail -3 || true
    note "mmaps (Eastern Kingdoms, Kalimdor): $(ls "$GD/mmaps" 2>/dev/null | wc -l) files ($(( $(date +%s) - t0 )) s)"
  else
    note "mmaps: their own phase after the settings (VR_MMAPS=0 skips them; the world server will not start without them)"
  fi
  rm -rf "$W"
  # the world server's container user must read it all
  chmod -R a+rX "$GD"
  note "world server data: $(du -sh "$GD" | cut -f1)"
  mark extract
fi

if ! done_ classicplus; then
  log "3/6 Classic+ server tables"
  n=0
  for t in SkillRaceClassInfo CreatureModelData CreatureDisplayInfo; do
    src="$CP/dbc/$t.dbc"; [ -f "$src" ] || continue
    dst=$(find_ci "$GD/dbc" "$t.dbc"); dst=${dst:-$GD/dbc/$t.dbc}
    [ -f "$dst.orig" ] || cp -p "$dst" "$dst.orig" 2>/dev/null || true
    cp "$src" "$dst" && chmod a+r "$dst" && n=$((n+1)) && note "dbc/$(basename "$dst"): the Classic+ table (stock kept as .orig)"
  done
  [ "$n" = 0 ] && note "no Classic+ tables in $CP/dbc: the world server runs on the stock tables"
  mark classicplus
fi

if ! done_ slice; then
  log "4/6 The 1.12 browser slice (levels 1-20)"
  if ! docker image inspect "$TOOLS_IMG" >/dev/null 2>&1; then
    note "building $TOOLS_IMG (StormLib from its authors' source, pinned tag; a few minutes)"
    docker build -q -t "$TOOLS_IMG" -f "$VR/mpq-tools.Dockerfile" "$VR" >/dev/null || die "could not build $TOOLS_IMG"
  fi
  sqlq() { docker exec -i "$DB" sh -c 'p="${MARIADB_ROOT_PASSWORD:-${MYSQL_ROOT_PASSWORD:-}}"; c=$(command -v mariadb || command -v mysql); MYSQL_PWD="$p" exec "$c" -uroot -N -B classicmangos'; }
  cols() { echo "SELECT GROUP_CONCAT(COLUMN_NAME) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA='classicmangos' AND TABLE_NAME='$1';" | sqlq 2>/dev/null | tail -1; }
  Q=$(docker run --rm --network none -v "$VR:/k:ro" "$TOOLS_IMG" python3 -I /k/vr-slice.py spawn-sql \
        --ct "$(cols creature_template)" --gt "$(cols gameobject_template)" --c "$(cols creature)" --g "$(cols gameobject)")
  if [ -n "$Q" ]; then
    printf '%s' "$Q" | sqlq > "$STATE/spawns.tsv" 2>/dev/null || note "!! the spawn query failed: the slice takes only the base creature list"
  else
    : > "$STATE/spawns.tsv"; note "!! the realm's tables have unexpected columns: the slice takes only the base creature list"
  fi
  note "spawns: $(wc -l < "$STATE/spawns.tsv") rows (read-only query on classicmangos)"
  fm=$(free_mb "$VR_ROOT")
  SPEC="$VR/slice-112.json"
  if [ -n "${VR_BUDGET_MB:-}" ]; then
    python3 -I -c 'import json,sys;d=json.load(open(sys.argv[1]));d["budgetMB"]=int(sys.argv[2]);json.dump(d,open(sys.argv[3],"w"))' "$SPEC" "$VR_BUDGET_MB" "$STATE/slice.json"
    SPEC="$STATE/slice.json"
  else
    cp "$SPEC" "$STATE/slice.json"
  fi
  docker run --rm --network none --memory 1g --user 0 -v "$UP:/data:ro" -v "$SLICE_ROOT:/outp" -v "$VR:/k:ro" -v "$STATE:/state" \
    "$TOOLS_IMG" python3 -I /k/vr-slice.py plan --data /data --slice /state/slice.json --spawns /state/spawns.tsv \
    --out /outp/Data --free-mb "$fm" --report /state/slice-report.json 2>&1 | tee "$STATE/slice.log" | sed 's/^/  /'
  tail -1 "$STATE/slice.log" | grep -q '^VR SLICE OK' || die "the slice was not made (see above); the old one (if any) is unchanged"
  chmod -R a+rX "$SLICE_ROOT/Data"
  note "slice: $(du -sh "$SLICE_ROOT/Data" | cut -f1) in $SLICE_ROOT/Data"
  if running "$SITE_C"; then docker restart "$SITE_C" >/dev/null && note "$SITE_C restarted on the new slice"; fi
  mark slice
fi

conf_set() {  # conf_set FILE KEY VALUE: "Key = value" (the line replaced, or added at the end)
  python3 -I - "$1" "$2" "$3" <<'PY'
import re, sys
p, k, v = sys.argv[1:4]
s = open(p, encoding="utf-8", errors="replace").read()
rx = re.compile(r"(?m)^[ \t]*" + re.escape(k) + r"[ \t]*=.*$")
line = f"{k} = {v}"
s2, n = rx.subn(line, s)
if not n:
    s2 = s.rstrip("\n") + f"\n# Vanilla Remastered (vr-data.sh)\n{line}\n"
if s2 != s:
    open(p, "w", encoding="utf-8").write(s2)
    print(f"    {k} = {v}")
PY
}
conf_get() { sed -n "s/^[[:space:]]*$2[[:space:]]*=[[:space:]]*//p" "$1" 2>/dev/null | tail -1 | tr -d '"\r'; }
conf_files() {  # conf_files CONTAINER NAME: the config files of that name under the container's mounts (host paths)
  local c="$1" src
  exists "$c" || return 0
  docker inspect -f '{{range .Mounts}}{{.Source}}{{println}}{{end}}' "$c" 2>/dev/null | while read -r src; do
    [ -n "$src" ] && [ -d "$src" ] && [ "$src" != "$GD" ] && find "$src" -maxdepth 3 -type f -name "$2" 2>/dev/null
  done | sort -u
}

if ! done_ settings; then
  log "5/6 Compact settings"
  BK="/root/vr112-conf-$(date +%Y%m%d-%H%M%S)"; ( umask 077; mkdir -p "$BK" )
  mm=0; [ -n "$(ls "$GD/mmaps" 2>/dev/null)" ] && mm=1
  found=0
  for f in $(conf_files "$OLD_WORLD" mangosd.conf; conf_files "$WORLD" mangosd.conf); do
    [ -f "$BK/$(echo "$f" | tr / _)" ] && continue
    cp -p "$f" "$BK/$(echo "$f" | tr / _)"; found=1
    note "$f (backup in $BK):"
    conf_set "$f" GridUnload 1
    conf_set "$f" mmap.enabled "$mm"
    note "    SOAP.Enabled is $(conf_get "$f" SOAP.Enabled || echo '?') (the page's sign-up and the self-check use it)"
  done
  [ "$found" = 1 ] || note "!! no mangosd.conf under the world server's mounts: settings left as they are (the image's own)"
  for f in $(conf_files "$OLD_WORLD" aiplayerbot.conf; conf_files "$WORLD" aiplayerbot.conf); do
    [ -f "$BK/$(echo "$f" | tr / _)" ] && continue
    cp -p "$f" "$BK/$(echo "$f" | tr / _)"
    note "$f:"
    conf_set "$f" AiPlayerbot.MinRandomBots "$BOTS"
    conf_set "$f" AiPlayerbot.MaxRandomBots "$BOTS"
    [ "$BOTS" = 0 ] && conf_set "$f" AiPlayerbot.RandomBotAutologin 0
  done
  for f in $(conf_files "$OLD_LOGON" realmd.conf; conf_files "$LOGON" realmd.conf); do
    [ -f "$BK/$(echo "$f" | tr / _)" ] && continue
    cp -p "$f" "$BK/$(echo "$f" | tr / _)"
    note "$f:"
    conf_set "$f" WrongPass.MaxCount 0
  done
  # the start script may write the config from the environment at every start: say so, so nobody is surprised
  img=$(docker inspect -f '{{.Image}}' "$OLD_WORLD" 2>/dev/null)
  if [ -n "$img" ] && docker run --rm --network none --entrypoint sh "$img" -c 'grep -lE "sed .*(GridUnload|mmap\.enabled|MaxRandomBots)" /*.sh /usr/local/bin/* /opt/mangos/*.sh 2>/dev/null' 2>/dev/null | grep -q .; then
    note "!! the world server's start script rewrites some of these settings from its environment: check docker logs $WORLD after ov112-on"
  fi
  for c in "$WORLD:$WORLD_MEM" "$LOGON:$LOGON_MEM"; do
    n=${c%%:*}; m=${c#*:}
    if exists "$n"; then
      docker update --memory "$m" --memory-swap "$(( ${m%m} + 512 ))m" "$n" >/dev/null 2>&1 && note "$n: memory limit $m (+512 MB swap)" \
        || note "!! could not set $n's memory limit"
    else
      note "$n: not made yet (ov112-on makes it; run this step again after it for the memory limit, or it runs unlimited)"
    fi
  done
  if running "$WORLD"; then note "!! $WORLD runs: the new settings apply at its next restart (5-minute warning first)"; fi
  mark settings
fi

# Movement maps: this core's grid loader asserts without them whatever mmap.enabled says (mangosd-start.sh holds
# the world server until they exist), so they are built here from the maps and vmaps already in the volume. Long
# (the two continents take the most); a second run goes on where the first stopped (MoveMapGen skips tiles it made).
if ! done_ mmaps && [ "${VR_MMAPS:-1}" != 0 ]; then
  log "Movement maps (the world server needs them)"
  [ -n "$(ls "$GD/maps" 2>/dev/null)" ] && [ -n "$(ls "$GD/vmaps" 2>/dev/null)" ] || die "no maps/vmaps in $GD_VOL to build movement maps from"
  TL=$(find_tools) || die "no CMaNGOS tools in the realm's images"
  IMG=${TL%%$'\t'*}; T=${TL#*$'\t'}
  MM=$(tool MoveMapGen "$T"); [ -n "$MM" ] || MM=$(tool movemapgen "$T")
  [ -n "$MM" ] || die "the image $IMG has no MoveMapGen"
  cpus=$(nproc 2>/dev/null || echo 2); [ "$cpus" -ge 1 ] || cpus=1; [ "$cpus" -gt "${VR_MMAP_THREADS:-2}" ] && cpus=${VR_MMAP_THREADS:-2}
  mem=$(( $(mem_mb) - 900 )); [ "$mem" -gt 3000 ] && mem=3000; [ "$mem" -lt 1000 ] && mem=1000
  note "MoveMapGen ($IMG), $cpus threads, memory ${mem}m; free disk $(free_mb "$VR_ROOT") MB"
  mkdir -p "$GD/mmaps"; t0=$(date +%s)
  docker run --rm --network none --memory "${mem}m" --cpus "$cpus" --user 0 \
    -v "$GD:/work/tmp" -w /work/tmp --entrypoint "$MM" "$IMG" --threads "$cpus" --silent 2>&1 | tail -5 || true
  n=$(ls "$GD/mmaps" 2>/dev/null | wc -l)
  note "movement maps: $n files, $(du -sh "$GD/mmaps" | cut -f1) ($(( $(date +%s) - t0 )) s)"
  ls "$GD/mmaps"/000*.mmap "$GD/mmaps"/001*.mmap >/dev/null 2>&1 || die "MoveMapGen made no maps for the two continents (see above); run the step again to go on"
  chmod -R a+rX "$GD/mmaps"
  for f in $(conf_files "$OLD_WORLD" mangosd.conf; conf_files "$WORLD" mangosd.conf); do
    cp -p "$f" "$f.pre-mmaps" 2>/dev/null; conf_set "$f" mmap.enabled 1
  done
  mark mmaps
fi

if ! done_ clean; then
  log "6/6 Delete the uploaded archives"
  if [ "${VR_KEEP_UPLOAD:-0}" = 1 ]; then
    note "kept (VR_KEEP_UPLOAD=1): $(du -sh "$UP" | cut -f1) in $UP"
  else
    before=$(du -sm "$UP" | cut -f1)
    find "$UP" -mindepth 1 -maxdepth 1 -type f \( -iname '*.mpq' -o -iname '*.mpq.part' -o -iname '*.mpq.sha256' \) -delete
    note "deleted: ${before} MB of uploaded archives (the extracted data and the slice stay)"
    mark clean
  fi
fi

log "Sizes"
note "world server data: $(du -sh "$GD" | cut -f1)  ($(for d in dbc maps vmaps mmaps; do printf '%s %s  ' "$d" "$(du -sh "$GD/$d" 2>/dev/null | cut -f1 || echo -)"; done))"
note "browser slice: $(du -sh "$SLICE_ROOT/Data" 2>/dev/null | cut -f1)"
note "upload folder: $(du -sh "$UP" | cut -f1)"
note "disk free: $(free_mb "$VR_ROOT") MB; memory available: $(mem_mb) MB"
echo
echo "Next: ov112-on (the realm), vr-beacon (Leyline), vr-site (the browser page)."
echo "VR DATA OK"
