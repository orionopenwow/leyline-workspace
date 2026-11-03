#!/usr/bin/env bash
# Is the streamed slice whole? Read-only: changes nothing on the server.
#
#   bash slice-health.sh                 the report: every slice folder in slices/ (and any .old/.new/.stage/.prev
#                                        copy), what wu-play serves and whether it answers; ends with
#                                        "SLICE HEALTH DONE: OK" or "SLICE HEALTH DONE: PROBLEMS"
#   bash slice-health.sh --verify <dir>  one folder, short; exit 0 = whole, 1 = not (slice-install.sh and the
#                                        overlay steps run it on a staged copy before it goes live)
#
# "Whole" means: the key files are there and readable by everyone (wu-play reads as nobody):
# DBFilesClient/Spell.dbc, DBFilesClient/Map.dbc, DBFilesClient/AreaTable.dbc, Interface/GlueXML/GlueXML.toc; and
# at least WU_SLICE_MIN_PCT (default 99) percent of the names in the slice's own tier lists (.tiers/<n>.txt,
# written by wenilla-host --export) are on disk (names compared without case, like wenilla-host).
# A slice with a Character folder must also hold every playable race and gender's model and its skin profiles (a
# PROBLEM otherwise: players show no character); the textures CharSections.dbc names and the shapeshift forms' models
# are WARN lines. The report ends with the game files players' browsers missed most (wrath-page.js reports each 404
# to wu-portal, counted in portal-data/stream/slice-misses.json; WU_SLICE_MISSES_TOP, default 25). 3 Nov 2026.
# Self-contained (python3 standard library only), so "Slice health - check.bat" can pipe it to the server.
# Incident 1 Nov 2026 (docs/INCIDENT-2026-11-01-SLICE.md): slices/mini lost DBFilesClient, Interface/GlueXML and
# 80,000 files while wu-play kept serving it; every /data answer was 404.
set -u
WU="${WU_DIR:-/srv/wrath}"
ST="${WU_STREAM_DIR:-$WU/stream}"
MIN_PCT="${WU_SLICE_MIN_PCT:-99}"

check_py() {   # <dir> <mode: verify|report>
  python3 -I - "$1" "$2" "$MIN_PCT" <<'PY'
import json, os, re, stat, sys
root, mode, min_pct = sys.argv[1], sys.argv[2], float(sys.argv[3])
KEYS = ["DBFilesClient\\Spell.dbc", "DBFilesClient\\Map.dbc", "DBFilesClient\\AreaTable.dbc",
        "Interface\\GlueXML\\GlueXML.toc"]
problems, notes = [], []
if not os.path.isdir(root):
    print(f"  {root}: not there")
    sys.exit(1)
files, nbytes, top, unreadable, closed_dirs = {}, 0, {}, 0, 0
for d, dirs, fs in os.walk(root):
    rel_d = os.path.relpath(d, root)
    if rel_d != "." and rel_d.split(os.sep)[0].startswith("."):
        continue    # .tiers, .overlay-*: listed below, not game files
    dirs.sort()
    for x in dirs:
        try:
            if os.stat(os.path.join(d, x)).st_mode & 0o005 != 0o005:
                closed_dirs += 1
        except OSError:
            closed_dirs += 1
    for f in fs:
        p = os.path.join(d, f)
        try:
            st = os.lstat(p)
        except OSError:
            continue
        if not stat.S_ISREG(st.st_mode):
            continue
        rel = os.path.relpath(p, root).replace(os.sep, "\\")
        files[rel.lower()] = (rel, st)
        nbytes += st.st_size
        if not st.st_mode & 0o004:
            unreadable += 1
        t = rel.split("\\", 1)[0] if "\\" in rel else "(top)"
        top[t] = top.get(t, 0) + 1
print(f"  {root}: {len(files)} files, {nbytes / 1073741824:.2f} GB")
if mode == "report":
    print("    folders: " + ", ".join(f"{k} {v}" for k, v in sorted(top.items(), key=lambda kv: kv[0].lower())))
for k in KEYS:
    hit = files.get(k.lower())
    if not hit:
        problems.append(f"missing {k}")
        continue
    rel, st = hit
    ok = bool(st.st_mode & 0o004) and st.st_size > 0
    try:
        with open(os.path.join(root, *rel.split("\\")), "rb") as fh:
            fh.read(4)
    except OSError as e:
        ok = False
        problems.append(f"{k}: cannot be read ({e.strerror})")
    if not st.st_mode & 0o004:
        problems.append(f"{k}: not readable by everyone (mode {stat.filemode(st.st_mode)})")
    elif st.st_size == 0:
        problems.append(f"{k}: empty")
    if mode == "report":
        print(f"    {'ok ' if ok else 'BAD'} {k} ({st.st_size} bytes)")
if unreadable:
    problems.append(f"{unreadable} files not readable by everyone (wu-play reads as nobody)")
if closed_dirs:
    problems.append(f"{closed_dirs} folders not searchable by everyone")
# The slice's own tier lists: what --export wrote (plus the overlays' marked lines).
tiers = os.path.join(root, ".tiers")
listed_all, missing_all = set(), set()
if os.path.isdir(tiers):
    idx = {}
    try:
        for t in json.load(open(os.path.join(tiers, "index.json"), encoding="utf-8")):
            if isinstance(t, dict) and "tier" in t:
                idx[int(t["tier"])] = int(t.get("files", 0))
    except (OSError, ValueError, TypeError):
        notes.append(".tiers/index.json missing or unreadable")
    rows = []
    for name in sorted(os.listdir(tiers), key=lambda n: (len(n), n)):
        m = re.fullmatch(r"(\d+)\.txt", name)
        if not m:
            continue
        n = int(m.group(1))
        listed = set()
        for line in open(os.path.join(tiers, name), encoding="utf-8", errors="replace"):
            line = line.rstrip("\r\n")
            if not line or line.startswith("#") or "\t" not in line:
                continue
            listed.add(line.split("\t", 1)[1].strip().replace("/", "\\").lower())
        missing = {x for x in listed if x not in files}
        listed_all |= listed
        missing_all |= missing
        rows.append((n, len(listed), len(listed) - len(missing), idx.get(n)))
    if not rows:
        notes.append("no tier lists in .tiers: only the key files were checked")
    else:
        bad_rows = [r for r in rows if r[2] < r[1]]
        if mode == "report":
            for n, l, p, ix in (rows if len(rows) <= 12 else bad_rows):
                print(f"    tier {n:>2}: {p}/{l} listed files on disk" + (f" (index.json says {ix})" if ix is not None and ix != l else ""))
        have = len(listed_all) - len(missing_all)
        pct = 100.0 * have / len(listed_all) if listed_all else 100.0
        print(f"    tier lists: {len(rows)} tiers, {have}/{len(listed_all)} listed files on disk ({pct:.2f}%)"
              + (f"; {len(bad_rows)} tiers short" if bad_rows else ""))
        if pct < min_pct:
            problems.append(f"{len(missing_all)} listed files are missing ({pct:.2f}% on disk, need {min_pct:g}%)")
        if missing_all:
            ex = sorted(missing_all)[:5]
            print("    missing, for example: " + ", ".join(ex))
else:
    notes.append("no .tiers folder: only the key files were checked")

# Player characters (3 Nov 2026: models missing in the browser). Every playable race and gender's model and its skin
# profiles (the M2's own view count) are a PROBLEM when missing; the textures CharSections.dbc names (skin, face, hair,
# facial hair, underwear) and the shapeshift forms' models (SpellShapeshiftForm -> CreatureDisplayInfo ->
# CreatureModelData) are WARN lines: the game data itself may lack a few. Only for a slice that has a Character folder.
RACES = ("Human", "Orc", "Dwarf", "NightElf", "Scourge", "Tauren", "Gnome", "Troll", "BloodElf", "Draenei")
warns = []

def read_rel(rel):
    hit = files.get(rel.lower())
    if not hit:
        return None
    try:
        with open(os.path.join(root, *hit[0].split("\\")), "rb") as fh:
            return fh.read()
    except OSError:
        return None

def dbc(rel, min_fields):
    b = read_rel(rel)
    if not b or len(b) < 20 or b[:4] != b"WDBC":
        return None
    import struct
    n, f, size, slen = struct.unpack_from("<4I", b, 4)
    if f < min_fields or size != f * 4 or 20 + n * size + slen > len(b):
        return None
    def row(r):
        return struct.unpack_from(f"<{f}i", b, 20 + r * size)
    def text(off):
        if off <= 0:
            return ""
        at = 20 + n * size + off
        end = b.find(b"\0", at)
        return b[at:end if end >= 0 else len(b)].decode("utf-8", "replace")
    return n, f, row, text

if any(k.startswith("character\\") for k in files):
    import struct
    bad_models, skins_seen = [], 0
    for race in RACES:
        for g in ("Male", "Female"):
            m2 = f"Character\\{race}\\{g}\\{race}{g}.m2"
            b = read_rel(m2)
            if b is None:
                bad_models.append(m2)
                continue
            views = struct.unpack_from("<I", b, 0x44)[0] if len(b) >= 0x48 and b[:4] == b"MD20" else 1
            for i in range(max(1, min(views, 8))):
                skin = f"{m2[:-3]}{i:02d}.skin"
                skins_seen += 1
                if skin.lower() not in files:
                    bad_models.append(skin)
    if mode == "report":
        print(f"    player models: {len(RACES) * 2} races and genders, {skins_seen} skin files checked"
              + (f", {len(bad_models)} missing" if bad_models else ", all there"))
    if bad_models:
        problems.append(f"{len(bad_models)} player model files missing (players show no character), e.g. " + ", ".join(bad_models[:4]))
    cs = dbc("DBFilesClient\\CharSections.dbc", 10)
    if cs:
        n, f, row, text = cs
        want = {text(row(r)[c]) for r in range(n) for c in (4, 5, 6)} - {""}
        gone = sorted(x for x in want if x.replace("/", "\\").lower() not in files)
        if mode == "report":
            print(f"    character textures (CharSections.dbc): {len(want) - len(gone)}/{len(want)} on disk")
        if gone:
            warns.append(f"{len(gone)} character textures CharSections.dbc names are not in the slice, e.g. " + ", ".join(gone[:4]))
    forms, cdi, cmd = (dbc("DBFilesClient\\SpellShapeshiftForm.dbc", 27), dbc("DBFilesClient\\CreatureDisplayInfo.dbc", 2),
                       dbc("DBFilesClient\\CreatureModelData.dbc", 3))
    if forms and cdi and cmd:
        model_of = {cdi[2](r)[0]: cdi[2](r)[1] for r in range(cdi[0])}
        path_of = {cmd[2](r)[0]: cmd[3](cmd[2](r)[2]) for r in range(cmd[0])}
        want = {}
        for r in range(forms[0]):
            rec = forms[2](r)
            for d in rec[23:27]:
                p = path_of.get(model_of.get(d, 0), "")
                if d > 0 and p:
                    stem = p.replace("/", "\\").rsplit(".", 1)[0]
                    want[stem + ".m2"] = d
        gone = sorted(x for x in want if x.lower() not in files)
        if mode == "report":
            print(f"    shapeshift form models: {len(want) - len(gone)}/{len(want)} on disk")
        if gone:
            warns.append(f"{len(gone)} shapeshift form models are not in the slice (a druid or shaman in that form shows nothing), e.g. " + ", ".join(gone[:4]))
for w in warns:
    print(f"    WARN: {w}")
for n in notes:
    print(f"    note: {n}")
for p in problems:
    print(f"    PROBLEM: {p}")
print(f"    {'WHOLE' if not problems else 'NOT WHOLE'}")
sys.exit(1 if problems else 0)
PY
}

if [ "${1:-}" = "--verify" ]; then
  [ -n "${2:-}" ] || { echo "usage: slice-health.sh --verify <slice dir>" >&2; exit 2; }
  check_py "${2%/}" verify
  exit $?
fi
[ -z "${1:-}" ] || { echo "usage: slice-health.sh [--verify <slice dir>]" >&2; exit 2; }

D="$ST/slices"
bad=0
echo "== slices in $D"
if [ ! -d "$D" ]; then echo "  no $D"; bad=1; fi
ls -la --time-style=+%Y-%m-%dT%H:%M:%S%z "$D" 2>/dev/null | sed 's/^/  /'
served=$(grep -E '^STREAM_SLICES=' "$WU/portal.env" 2>/dev/null | tail -1 | cut -d= -f2 | cut -d, -f1 | tr -d '\r"' || true)
served=${served:-mini}
echo "  served (portal.env STREAM_SLICES): $served"
for p in "$ST" "$D"; do
  [ -d "$p" ] || continue
  m=$(stat -c %a "$p"); case "$m" in *[2367]) echo "  PROBLEM: $p is writable by everyone (mode $m): chmod go-w $p"; bad=1 ;; esac
done
echo
for d in "$D"/*/; do
  [ -d "$d" ] || continue
  d=${d%/}; b=${d##*/}
  case "$b" in
    *.*) echo "== $b (a copy, not served: ${b##*.})"; check_py "$d" report || true ;;
    *)   echo "== $b$([ "$b" = "$served" ] && echo ' (SERVED)')"
         if [ -f "$d/.gamedata-parked" ]; then echo "  parked by the game data killswitch (empty placeholder)"; continue; fi
         check_py "$d" report || { [ "$b" = "$served" ] && bad=1; } ;;
  esac
  for s in "$d"/.overlay-*; do [ -f "$s/added.txt" ] && echo "    overlay ${s##*/.overlay-}: $(grep -c . "$s/added.txt") files laid over"; done
  echo
done
echo "== wu-play"
if docker ps --format '{{.Names}}' 2>/dev/null | grep -qx wu-play; then
  docker inspect -f '  running since {{.State.StartedAt}}; mounts: {{range .Mounts}}{{.Source}}->{{.Destination}} {{end}}' wu-play 2>&1
  # wu-play holds the folder it was started on: renamed or deleted afterwards, it keeps serving that one.
  inside=$(docker exec wu-play stat -c %i /slice 2>/dev/null || true)
  live=$(stat -c %i "$D/$served" 2>/dev/null || true)
  if [ -n "$inside" ] && [ -n "$live" ]; then
    if [ "$inside" = "$live" ]; then echo "  wu-play serves slices/$served itself (inode $live)"
    else echo "  PROBLEM: wu-play serves another folder than slices/$served (inode $inside, the folder is $live): it was replaced after wu-play started. Fix: docker restart wu-play"; bad=1; fi
  fi
  for u in '/data/DBFilesClient%5CSpell.dbc' '/data/Interface%5CGlueXML%5CGlueXML.toc' '/data/__tier/index.json'; do
    c=$(docker exec wu-portal python3 -c 'import sys,urllib.request,urllib.error
try: print(urllib.request.urlopen("http://wu-play:8090"+sys.argv[1],timeout=15).status)
except urllib.error.HTTPError as e: print(e.code)
except Exception: print("no answer")' "$u" 2>/dev/null || true)
    echo "  inside the Docker network: $u -> ${c:-?}"
    case "$c" in 200|"") ;; *) bad=1 ;; esac
  done
else
  echo "  PROBLEM: wu-play is not running (the browser game answers 502): bash $ST/stream-up.sh"; bad=1
fi
echo
# What players' browsers missed (wrath-page.js reports each 404 to wu-portal: POST /__gate?missing=1, counted in
# portal-data/stream/slice-misses.json). Information only: a name the game data does not have is a 404 too.
MISSES="${WU_SLICE_MISSES_FILE:-$WU/portal-data/stream/slice-misses.json}"
echo "== most missed game files (players' reports: $MISSES)"
python3 -I - "$MISSES" "$D/$served" "${WU_SLICE_MISSES_TOP:-25}" <<'PY'
import json, os, sys, time
path, root, top = sys.argv[1], sys.argv[2], int(sys.argv[3])
try:
    doc = json.load(open(path, encoding="utf-8"))
    names = doc.get("names") or {}
except FileNotFoundError:
    print("  none reported yet (the page reports misses from the next slice upload on)")
    sys.exit(0)
except (OSError, ValueError, AttributeError) as e:
    print(f"  cannot read it: {e}")
    sys.exit(0)
rows = sorted((v for v in names.values() if isinstance(v, dict) and isinstance(v.get("name"), str)),
              key=lambda v: (-int(v.get("n") or 0), v["name"].lower()))
when = time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime(int(doc.get("updated") or 0)))
print(f"  {len(rows)} names reported, last write {when}; the top {min(top, len(rows))}:")
BASE = ("character\\", "item\\", "creature\\", "spells\\", "interface\\", "dbfilesclient\\")
def on_disk(name):
    at = root
    for comp in name.replace("/", "\\").split("\\"):
        try:
            hit = next((e for e in os.listdir(at) if e.lower() == comp.lower()), None)
        except OSError:
            return False
        if hit is None:
            return False
        at = os.path.join(at, hit)
    return os.path.isfile(at)
by_top = {}
for v in rows:
    t = v["name"].replace("/", "\\").split("\\", 1)[0]
    by_top[t] = by_top.get(t, 0) + 1
for v in rows[:top]:
    name = v["name"]
    tag = "now in the slice" if on_disk(name) else ("NOT in the slice, under a model or UI folder" if name.lower().startswith(BASE) else "not in the slice")
    print(f"  {int(v.get('n') or 0):>6}  {name}  ({tag})")
if by_top:
    print("  by folder: " + ", ".join(f"{k} {n}" for k, n in sorted(by_top.items(), key=lambda kv: -kv[1])[:12]))
PY
echo
echo "== disk"; df -h "$ST" 2>/dev/null | tail -1 | sed 's/^/  /'
echo
if [ "$bad" = 0 ]; then echo "SLICE HEALTH DONE: OK"; else echo "SLICE HEALTH DONE: PROBLEMS (see PROBLEM lines above)"; fi
exit 0
