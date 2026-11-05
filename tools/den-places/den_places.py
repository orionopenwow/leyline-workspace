"""Den places: real places from the player's own game, rebuilt 1:1 as Den builds (owner 4 Nov: "a 1:1 recreation of
real places as Den builds, e.g. the Lion's Pride Inn (Goldshire) and arenas").

Runs on the player's PC and reads only their own game folder (its MPQ archives, through tools/den-check/mpqlite.py):

- the map's folder name from their own Map.dbc (or the place's `dir`);
- the map tiles (ADT) the place's box touches: the loose models (MDDF) and the buildings (MODF) standing in it; a map
  made of one building (an arena: WDT MODF) gives that building;
- each building's own root file (WMO): its doodad sets (MODS) and doodads (MODN, MODD), the default set and the
  placement's own, put where the building stands;
- the model table (GameObjectDisplayInfo.dbc): which display shows each model. Den Lite shows any display through its
  model carrier (a prop's entry is MODEL_BASE + the display: labs/den-lite den.rs). A model with no display is left
  out and counted, unless `--add-models`: then it gets a row of its own in `<dens>/place-models.json`, which the
  launcher adds to the Den's own patch table (host/denmodels.rs) pointing at the file the player's game already has.

It writes, into the player's own Dens folder (never shipped, never committed, never uploaded):

- `<dens>/<name>.den.json`: the whole place as a Den (`.den load <name>`: up to 800 pieces, a city-size Den's reach);
- `<dens>/stamps-local/<name>.den.json`: the same as a stamp when it fits one (400 pieces within 40 yards:
  `.den copy <name>`, Jaina builds it piece by piece);
- `<dens>/place-models.json` with `--add-models`.

What ships is this code and places.json (names, maps, boxes: no positions of anything). The frames are the ones
leyline-terrain place.rs uses: a placement's world x = PLACE - its z, world y = PLACE - its x, height = its y; its facing
is its y rotation + 180 degrees; a building's doodads are in its own frame (x its front, y its left, z up).
docs/DEN-PLACES.md has the rules and the limits.

    python3 tools/den-places/den_places.py --game "C:/Games/Wrath" --dens "%APPDATA%/Leyline/dens" --place lionspride
    python3 tools/den-places/den_places.py --game ... --dens ... --all [--add-models]
    python3 tools/den-places/den_places.py --list
"""
import argparse
import json
import math
import os
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "den-check"))
import mpqlite  # noqa: E402

PLACES_FILE = HERE / "places.json"
TILE = 1600.0 / 3.0            # a map tile, yards
PLACE = 32 * TILE              # placement space's zero (leyline-terrain PLACE)
CHUNK = TILE / 16              # an MCNK
UNIT = CHUNK / 8               # between two outer height points
YAW_OFFSET = 180.0             # leyline-terrain place.rs YAW_OFFSET
MODEL_BASE = 2_000_000_000     # labs/den-lite den.rs MODEL_BASE
STAMP_MAX, STAMP_SPAN = 400, 40.0   # den.rs LOCAL_MAX, STAMP_SPAN (a copied place as a stamp)
DEN_MAX, DEN_SPAN = 800, 256.0      # den.rs MAX_PROPS; a city-size Den's half width (ring_half(6))
OPEN_GROUND = 64.0             # den.rs ring_half(ROOM_RINGS): from this size a Den is open ground, no walls
ROW_BASE, ROW_LAST = 62001, 69999   # place-models.json display ids (my-models are 60001...)
ROWS_FILE = "place-models.json"
ROWS_FORMAT = "leyline-den-place-models"
SOURCE = "den-places"
MAX_BYTES = 512 * 1024         # den.rs MAX_BYTES
DISPLAY_TABLE = "DBFilesClient\\GameObjectDisplayInfo.dbc"
MAP_TABLE = "DBFilesClient\\Map.dbc"


class PlaceError(Exception):
    pass


# ---------------------------------------------------------------- the player's archives, in the client's order

BASE = ["common.mpq", "common-2.mpq", "expansion.mpq", "lichking.mpq"]
LOCALE_BASE = ["locale-%s.mpq", "speech-%s.mpq", "expansion-locale-%s.mpq", "lichking-locale-%s.mpq",
               "expansion-speech-%s.mpq", "lichking-speech-%s.mpq"]
ORDER = "0123456789abcdefghijklmnopqrstuvwxyz"


def patch_rank(n, pre):
    """patch < patch-2 < ... < patch-9 < patch-a < ... (later wins); None when it isn't one. patch-T is the Den's own
    (the launcher's: its ground tile and table) and is never read."""
    if n == pre + ".mpq":
        return 0
    if n.startswith(pre + "-") and n.endswith(".mpq"):
        c = n[len(pre) + 1:-4]
        if len(c) == 1 and c in ORDER and c != "t":
            return ORDER.index(c) + 1
    return None


def archive_order(game):
    """Every archive of a game folder, lowest first (the last one that has a file wins)."""
    data = Path(game) / "Data"
    if not data.is_dir():
        data = Path(game)
    files = {e.lower(): data / e for e in os.listdir(data) if e.lower().endswith(".mpq")} if data.is_dir() else {}
    locs = sorted(e for e in (os.listdir(data) if data.is_dir() else [])
                  if len(e) == 4 and e[:2].islower() and e[2:].isupper() and (data / e).is_dir())
    out = [files[b] for b in BASE if b in files]
    lfiles = {}
    for loc in locs:
        for e in os.listdir(data / loc):
            if e.lower().endswith(".mpq"):
                lfiles[e.lower()] = data / loc / e
        out += [lfiles[b % loc.lower()] for b in LOCALE_BASE if (b % loc.lower()) in lfiles]
    out += [p for _, p in sorted((patch_rank(n, "patch"), p) for n, p in files.items() if patch_rank(n, "patch") is not None)]
    for loc in locs:
        pre = "patch-%s" % loc.lower()
        out += [p for _, p in sorted((patch_rank(n, pre), p) for n, p in lfiles.items() if patch_rank(n, pre) is not None)]
    return out


class Game:
    """The player's game: a file by its archive name, from the highest archive that has it."""

    def __init__(self, archives):
        self.archives = []
        self.unreadable = []
        for a in archives:
            if isinstance(a, mpqlite.Archive):
                self.archives.append(a)
                continue
            try:
                self.archives.append(mpqlite.Archive(str(a)))
            except (OSError, mpqlite.MpqError) as e:
                self.unreadable.append("%s (%s)" % (Path(a).name, e))

    @classmethod
    def open(cls, folder):
        return cls(archive_order(folder))

    def read(self, name):
        for a in reversed(self.archives):
            try:
                if a.has(name):
                    return a.read(name)
            except mpqlite.MpqError as e:
                self.unreadable.append("%s in %s (%s)" % (name, Path(a.path).name, e))
                return None
        return None


# ---------------------------------------------------------------- the formats (the wowdev wiki's ADT/v18, WDT, WMO)

def chunks(b):
    """Top-level chunks: (magic as written in the docs, start of the data, its size). The files keep magics reversed."""
    out, i = [], 0
    while i + 8 <= len(b):
        magic = b[i:i + 4][::-1].decode("latin-1")
        size = struct.unpack_from("<I", b, i + 4)[0]
        if i + 8 + size > len(b):
            break
        out.append((magic, i + 8, size))
        i += 8 + size
    return out


def first(b, ch, magic):
    for m, o, n in ch:
        if m == magic:
            return b[o:o + n]
    return b""


def zstrings(blob, offsets):
    out = []
    for off in offsets:
        end = blob.find(b"\0", off)
        out.append(blob[off:end if end >= 0 else len(blob)].decode("latin-1"))
    return out


def names_at(blob, ids_blob):
    offsets = list(struct.unpack("<%dI" % (len(ids_blob) // 4), ids_blob[:len(ids_blob) // 4 * 4]))
    return zstrings(blob, offsets)


def read_modf(blob, names):
    """MODF: 64 bytes each (name index, unique id, position, rotation, extents, flags, doodad set, name set, pad)."""
    out = []
    for i in range(len(blob) // 64):
        v = struct.unpack_from("<II3f3f3f3fHHHH", blob, i * 64)
        name = names[v[0]] if v[0] < len(names) else ""
        out.append({"model": name, "uid": v[1], "pos": v[2:5], "rot": v[5:8], "set": v[15], "scale": 1.0, "wmo": True})
    return out


def read_adt(b):
    """A map tile: its models (MDDF) and buildings (MODF) with their file names, and its height chunks (MCNK)."""
    ch = chunks(b)
    if not any(m == "MVER" for m, _, _ in ch):
        raise PlaceError("That isn't a map tile.")
    m2 = names_at(first(b, ch, "MMDX"), first(b, ch, "MMID"))
    wmo = names_at(first(b, ch, "MWMO"), first(b, ch, "MWID"))
    doodads = []
    md = first(b, ch, "MDDF")
    for i in range(len(md) // 36):
        v = struct.unpack_from("<II3f3fHH", md, i * 36)
        doodads.append({"model": m2[v[0]] if v[0] < len(m2) else "", "uid": v[1], "pos": v[2:5], "rot": v[5:8],
                        "scale": v[8] / 1024.0, "wmo": False})
    heights = []
    for m, o, n in ch:
        if m != "MCNK" or n < 128:
            continue
        px, py, pz = struct.unpack_from("<3f", b, o + 0x68)
        sub = chunks(b[o + 128:o + n])
        mcvt = first(b[o + 128:o + n], sub, "MCVT")
        if len(mcvt) >= 145 * 4:
            heights.append((px, py, pz, struct.unpack_from("<145f", mcvt)))
    return {"doodads": doodads, "wmos": read_modf(first(b, ch, "MODF"), wmo), "heights": heights}


def read_wdt(b):
    """A map's WDT: which tiles there are (MAIN) and, for a map that is one building, that building (MODF)."""
    ch = chunks(b)
    if not any(m == "MPHD" for m, _, _ in ch):
        raise PlaceError("That isn't a map's WDT.")
    main = first(b, ch, "MAIN")
    tiles = set()
    for i in range(min(4096, len(main) // 8)):
        if struct.unpack_from("<I", main, i * 8)[0] & 1:
            tiles.add((i % 64, i // 64))
    names = zstrings(first(b, ch, "MWMO"), [0]) if first(b, ch, "MWMO") else []
    return {"tiles": tiles, "wmos": read_modf(first(b, ch, "MODF"), names)}


def read_wmo(b):
    """A building's root file: its doodad sets (name, first, count) and doodads (model, position, rotation x y z w,
    size), and its box."""
    ch = chunks(b)
    mohd = first(b, ch, "MOHD")
    if not mohd:
        raise PlaceError("That isn't a building's root file.")
    box = struct.unpack_from("<6f", mohd, 36) if len(mohd) >= 60 else (0.0,) * 6
    sets = []
    mods = first(b, ch, "MODS")
    for i in range(len(mods) // 32):
        name = mods[i * 32:i * 32 + 20].split(b"\0")[0].decode("latin-1")
        f, n = struct.unpack_from("<II", mods, i * 32 + 20)
        sets.append((name, f, n))
    modn, modd = first(b, ch, "MODN"), first(b, ch, "MODD")
    doodads = []
    for i in range(len(modd) // 40):
        v = struct.unpack_from("<I3f4ff", modd, i * 40)
        doodads.append({"model": zstrings(modn, [v[0] & 0xFFFFFF])[0], "pos": v[1:4], "rot": v[4:8], "scale": v[8]})
    return {"sets": sets, "doodads": doodads, "box": box}


def m2_box(b):
    """An M2's bounding box (3.3.5a header: at 0xA0, min then max)."""
    if b and len(b) >= 0xB8 and b[:4] == b"MD20":
        return struct.unpack_from("<6f", b, 0xA0)
    return None


def read_dbc(b):
    if not b or len(b) < 20 or b[:4] != b"WDBC":
        raise PlaceError("That isn't a table.")
    n, fields, size, sn = struct.unpack_from("<4I", b, 4)
    if 20 + n * size + sn > len(b) or size < fields * 4:
        raise PlaceError("That table is cut short.")
    rows = [struct.unpack_from("<%dI" % fields, b, 20 + i * size) for i in range(n)]
    strings = b[20 + n * size:20 + n * size + sn]
    return rows, strings


def norm(path):
    """A model file name as the tables and tiles compare (leyline-terrain place.rs norm): lower case, backslashes,
    no .m2 / .mdx / .mdl."""
    p = path.strip().replace("/", "\\").lower()
    for ext in (".m2", ".mdx", ".mdl"):
        if p.endswith(ext):
            return p[:-len(ext)]
    return p


def display_index(b):
    """model (normed) -> display id (the lowest when several share a model), and the highest id there is."""
    rows, strings = read_dbc(b)
    out, top = {}, 0
    for r in rows:
        top = max(top, r[0])
        path = zstrings(strings, [r[1]])[0] if r[1] < len(strings) else ""
        if path:
            k = norm(path)
            out[k] = min(out.get(k, r[0]), r[0])
    return out, top


def map_dirs(b):
    """Map.dbc: map id -> its folder (field 1)."""
    rows, strings = read_dbc(b)
    return {r[0]: zstrings(strings, [r[1]])[0] for r in rows if r[1] < len(strings)}


# ---------------------------------------------------------------- frames

def world(pos):
    """A placement's position -> the world (x, y, height)."""
    return (PLACE - pos[2], PLACE - pos[0], pos[1])


def quat_yaw(q):
    x, y, z, w = q
    return math.atan2(2.0 * (w * z + x * y), 1.0 - 2.0 * (y * y + z * z))


def tile_of(x, y):
    """The tile (its file's x, y) a world spot is in."""
    return (int(math.floor(32 - y / TILE)), int(math.floor(32 - x / TILE)))


def tiles_for(box):
    x0, y0, x1, y1 = box
    out = set()
    for x in (x0, x1, (x0 + x1) / 2):
        for y in (y0, y1, (y0 + y1) / 2):
            out.add(tile_of(x, y))
    tx = sorted({t[0] for t in out})
    ty = sorted({t[1] for t in out})
    return sorted((a, b) for a in range(tx[0], tx[-1] + 1) for b in range(ty[0], ty[-1] + 1))


def ground_at(heights, x, y):
    """The ground's height at a world spot from the tiles' MCNKs (the nearest outer point), or None."""
    for px, py, pz, h in heights:
        if px - CHUNK < x <= px + 1e-3 and py - CHUNK < y <= py + 1e-3:
            i = min(8, max(0, int(round((px - x) / UNIT))))
            j = min(8, max(0, int(round((py - y) / UNIT))))
            return pz + h[i * 17 + j]
    return None


# ---------------------------------------------------------------- the places

def load_places(path=PLACES_FILE):
    doc = json.loads(Path(path).read_text(encoding="utf-8"))
    if doc.get("format") != "leyline-den-place-list":
        raise PlaceError("That isn't the place list.")
    out = []
    for p in doc["places"]:
        bad = check_place(p)
        if bad:
            raise PlaceError("%s: %s" % (p.get("name"), bad))
        out.append(p)
    return out


def check_place(p):
    n = p.get("name", "")
    if not (1 <= len(n) <= 24 and all(c.isalnum() or c in "-_" for c in n) and n == n.lower()):
        return "a name is 1 to 24 lower-case letters, digits, - and _"
    if not isinstance(p.get("map"), int):
        return "no map"
    box = p.get("box")
    if not (isinstance(box, list) and len(box) == 4 and box[0] < box[2] and box[1] < box[3]):
        return "a box is [x0, y0, x1, y1]"
    if max(box[2] - box[0], box[3] - box[1]) > 2 * DEN_SPAN:
        return "bigger than a Den can hold (%d yards across)" % (2 * DEN_SPAN)
    if not p.get("title") or len(p["title"]) > 40:
        return "its title"
    return None


def find_place(places, name):
    for p in places:
        if p["name"] == name or name in p.get("words", []):
            return p
    return None


def inside(box, x, y, pad=0.0):
    return box[0] - pad <= x <= box[2] + pad and box[1] - pad <= y <= box[3] + pad


def collect(game, p, dirs):
    """Every model standing in a place: (model, world x, y, z, world yaw degrees, scale, is a building, role)."""
    # the player's own Map.dbc names the folder; the list's `dir` only when it can't be read
    d = dirs.get(p["map"]) or p.get("dir")
    if not d:
        raise PlaceError("Your game has no map %d." % p["map"])
    box, pieces, heights, seen = p["box"], [], [], set()
    found_tile = False
    placements = []
    wdt_b = game.read("World\\Maps\\%s\\%s.wdt" % (d, d))
    wdt = read_wdt(wdt_b) if wdt_b else None
    if wdt:
        placements += wdt["wmos"]
    for t in tiles_for(box):
        if wdt and wdt["tiles"] and t not in wdt["tiles"]:
            continue
        b = game.read("World\\Maps\\%s\\%s_%d_%d.adt" % (d, d, t[0], t[1]))
        if not b:
            continue
        found_tile = True
        adt = read_adt(b)
        heights += adt["heights"]
        placements += adt["wmos"] + adt["doodads"]
    if not placements and not found_tile:
        raise PlaceError("Your game has no map files for %s." % p["title"])
    words = [w.lower() for w in p.get("focus", [])]
    focus, focus_rank = None, len(words)
    for m in placements:
        # a placement on two tiles is in both: once
        key = (m["wmo"], m["uid"])
        if key in seen:
            continue
        seen.add(key)
        wx, wy, wz = world(m["pos"])
        if not inside(box, wx, wy):
            continue
        if not (p.get("z", [-1e9, 1e9])[0] <= wz <= p.get("z", [-1e9, 1e9])[1]):
            continue
        yaw = (m["rot"][1] + YAW_OFFSET) % 360.0
        short = m["model"].replace("/", "\\").split("\\")[-1].lower()
        pc = {"model": m["model"], "x": wx, "y": wy, "z": wz, "yaw": yaw, "scale": m["scale"], "wmo": m["wmo"],
              "role": None, "set": m.get("set", 0)}
        # the place's own building: the one whose file has the earliest of the focus words
        rank = next((i for i, w in enumerate(words) if w in short), len(words)) if m["wmo"] else len(words)
        if rank < focus_rank:
            focus, focus_rank = pc, rank
        pieces.append(pc)
    if focus:
        focus["role"] = "house"
        focus = (focus["x"], focus["y"], focus["z"], focus["yaw"])
        # only what stands round it, when the place says how far
        if p.get("radius"):
            r = float(p["radius"])
            pieces = [pc for pc in pieces if pc["role"] == "house" or math.hypot(pc["x"] - focus[0], pc["y"] - focus[1]) <= r]
    return pieces, heights, focus


def with_doodads(game, pieces):
    """Each building's own doodads (its default set and the placement's own), where the building stands."""
    out = []
    for pc in pieces:
        out.append(pc)
        if not pc["wmo"]:
            continue
        b = game.read(pc["model"])
        if not b:
            continue
        w = read_wmo(b)
        pc["box"] = w["box"]
        sets = [0] + ([pc["set"]] if pc["set"] else [])
        a = math.radians(pc["yaw"])
        c, s = math.cos(a), math.sin(a)
        for k in sets:
            if k >= len(w["sets"]):
                continue
            _, f, n = w["sets"][k]
            for dd in w["doodads"][f:f + n]:
                lx, ly, lz = dd["pos"]
                out.append({"model": dd["model"], "x": pc["x"] + lx * c - ly * s, "y": pc["y"] + lx * s + ly * c, "z": pc["z"] + lz,
                            "yaw": (pc["yaw"] + math.degrees(quat_yaw(dd["rot"]))) % 360.0, "scale": dd["scale"], "wmo": False,
                            "role": None, "set": 0})
    return out


class Rows:
    """`<dens>/place-models.json`: a display row of our own for a model the player's table has none for (it names the
    file the player's game already has; the launcher adds the rows to the Den's own patch table)."""

    def __init__(self, dens, top):
        self.path = Path(dens) / ROWS_FILE
        self.rows = {}
        self.top = top
        try:
            doc = json.loads(self.path.read_text(encoding="utf-8"))
            if doc.get("format") == ROWS_FORMAT:
                for r in doc.get("models", []):
                    if ROW_BASE <= r["display"] <= ROW_LAST and r["display"] > top:
                        self.rows[norm(r["model"])] = r
        except (OSError, ValueError, KeyError, TypeError):
            pass
        self.changed = False

    def get(self, model, box):
        k = norm(model)
        if k in self.rows:
            return self.rows[k]["display"]
        used = {r["display"] for r in self.rows.values()}
        n = max(ROW_BASE, self.top + 1)
        while n in used:
            n += 1
        if n > ROW_LAST:
            return None
        self.rows[k] = {"display": n, "model": model.replace("/", "\\"), "box": [round(v, 3) for v in (box or (0.0,) * 6)]}
        self.changed = True
        return n

    def save(self):
        if not self.changed:
            return
        doc = {"format": ROWS_FORMAT, "version": 1, "note": "Made on this PC from your own game files by tools/den-places: "
               "rows the launcher adds to the Den's own model table. Never shared.",
               "models": sorted(self.rows.values(), key=lambda r: r["display"])}
        self.path.write_text(json.dumps(doc, indent=1), encoding="utf-8")


def build(game, p, dirs, displays, rows=None):
    """One place as a Den document (and whether it fits a stamp). Raises PlaceError."""
    pieces, heights, focus = collect(game, p, dirs)
    if p.get("doodads", True):
        pieces = with_doodads(game, pieces)
    if not pieces:
        raise PlaceError("Nothing stands in %s's box in your game." % p["title"])
    box = p["box"]
    if focus:
        ox, oy, oz = focus[0], focus[1], focus[2]
    else:
        ox, oy = p.get("middle", [(box[0] + box[2]) / 2, (box[1] + box[3]) / 2])
        g = ground_at(heights, ox, oy)
        oz = g if g is not None else min(pc["z"] for pc in pieces)
    props, skipped, far = [], {}, 0
    # the building first, then the biggest pieces (when a place has more than a Den holds, the small ones go)
    pieces.sort(key=lambda pc: (pc["role"] != "house", not pc["wmo"], -(pc["scale"] or 1.0)))
    for pc in pieces:
        dx, dy, dz = pc["x"] - ox, pc["y"] - oy, pc["z"] - oz
        if math.hypot(dx, dy) > DEN_SPAN or abs(dz) > DEN_SPAN:
            far += 1
            continue
        k = norm(pc["model"])
        disp = displays.get(k)
        if disp is None and rows is not None and k:
            if pc["wmo"]:
                bx = pc.get("box")
            else:
                mp = pc["model"]
                mb = game.read(mp) or (game.read(mp[:-4] + ".m2") if mp.lower().endswith((".mdx", ".mdl")) else None)
                bx = m2_box(mb)
            disp = rows.get(pc["model"], bx)
        if disp is None:
            short = pc["model"].replace("/", "\\").split("\\")[-1]
            skipped[short] = skipped.get(short, 0) + 1
            continue
        if len(props) >= DEN_MAX:
            far += 1
            continue
        e = {"entry": MODEL_BASE + disp, "x": round(dx, 2), "y": round(dy, 2), "z": round(dz, 2), "yaw": round(pc["yaw"], 2)}
        sc = pc["scale"] or 1.0
        if abs(sc - 1.0) > 0.01:
            e["scale"] = round(min(5.0, max(0.1, sc)), 2)
        if pc["role"]:
            e["role"] = pc["role"]
        props.append(e)
    reach = max([math.hypot(e["x"], e["y"]) for e in props] + [0.0])
    stamp = len(props) <= STAMP_MAX and all(abs(e[a]) <= STAMP_SPAN for e in props for a in ("x", "y", "z"))
    n_skip = sum(skipped.values())
    line = "Made from your own game files: %d pieces%s%s." % (
        len(props), ", %d models with no display left out" % n_skip if n_skip else "",
        ", %d pieces past what a Den holds left out" % far if far else "")
    doc = {"format": "leyline-den", "version": 1, "kind": "den", "local": True, "source": SOURCE, "name": p["name"],
           "title": p["title"], "line": line, "focus": [0.0, 0.0], "from": {"map": p["map"]},
           "room": {"look": "white", "size": round(max(OPEN_GROUND, reach), 1)},
           "skipped": [{"file": f, "count": c} for f, c in sorted(skipped.items(), key=lambda kv: (-kv[1], kv[0]))][:40],
           "props": props}
    return doc, stamp


def ours(path):
    """A file this tool may write over: none there, or one it made."""
    try:
        return json.loads(Path(path).read_text(encoding="utf-8")).get("source") == SOURCE
    except FileNotFoundError:
        return True
    except (OSError, ValueError, AttributeError):
        return False


def write_place(dens, doc, stamp):
    """The Den file and, when it fits one, the stamp. What was written."""
    dens = Path(dens)
    made = []
    text = json.dumps(doc, indent=1)
    if len(text.encode("utf-8")) > MAX_BYTES:
        text = json.dumps(doc, separators=(",", ":"))
    if len(text.encode("utf-8")) > MAX_BYTES:
        raise PlaceError("%s is too big for a Den file." % doc["title"])
    den = dens / ("%s.den.json" % doc["name"])
    if ours(den):
        den.write_text(text, encoding="utf-8")
        made.append(str(den))
    if stamp:
        st = dict(doc, kind="stamp")
        st.pop("room", None)
        (dens / "stamps-local").mkdir(parents=True, exist_ok=True)
        sp = dens / "stamps-local" / ("%s.den.json" % doc["name"])
        if ours(sp):
            sp.write_text(json.dumps(st, indent=1), encoding="utf-8")
            made.append(str(sp))
    return made


def run(game, dens, names, add_models=False, places=None):
    """Import places into a Dens folder: a report per place {name, ok, pieces, stamp, skipped, line | error}."""
    places = places or load_places()
    table = game.read(DISPLAY_TABLE)
    if not table:
        raise PlaceError("Your game's model table can't be read.")
    displays, top = display_index(table)
    mb = game.read(MAP_TABLE)
    dirs = map_dirs(mb) if mb else {}
    rows = Rows(dens, top) if add_models else None
    Path(dens).mkdir(parents=True, exist_ok=True)
    out = []
    for n in names:
        p = find_place(places, n)
        if not p:
            out.append({"name": n, "ok": False, "error": "No place called '%s' (--list)." % n})
            continue
        try:
            doc, stamp = build(game, p, dirs, displays, rows)
            files = write_place(dens, doc, stamp)
            out.append({"name": p["name"], "ok": True, "pieces": len(doc["props"]), "stamp": stamp, "files": files,
                        "skipped": sum(s["count"] for s in doc["skipped"]), "line": doc["line"]})
        except (PlaceError, struct.error) as e:
            out.append({"name": p["name"], "ok": False, "error": str(e)})
    if rows:
        rows.save()
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description="Rebuild real places from your own game files as Den builds.")
    ap.add_argument("--game", help="your game folder (the one with Data)")
    ap.add_argument("--dens", help="your Dens folder")
    ap.add_argument("--place", action="append", default=[], help="a place's name (again for more)")
    ap.add_argument("--all", action="store_true", help="every place in places.json")
    ap.add_argument("--add-models", action="store_true", help="models with no display get a row of our own (place-models.json)")
    ap.add_argument("--list", action="store_true", help="the places")
    a = ap.parse_args(argv)
    places = load_places()
    if a.list:
        for p in places:
            print("%-18s %-28s map %-4d %s" % (p["name"], p["title"], p["map"], p.get("note", "")))
        return 0
    if not a.game or not a.dens or not (a.place or a.all):
        ap.print_usage()
        return 2
    game = Game.open(a.game)
    names = [p["name"] for p in places] if a.all else a.place
    try:
        rep = run(game, a.dens, names, a.add_models, places)
    except PlaceError as e:
        print("den-places: %s" % e)
        return 1
    for r in rep:
        print("%s: %s" % (r["name"], r["line"] + (" (a stamp too: .den copy %s)" % r["name"] if r.get("stamp") else "") if r["ok"] else r["error"]))
    if game.unreadable:
        print("couldn't read: %s" % "; ".join(game.unreadable[:6]))
    return 0 if all(r["ok"] for r in rep) else 1


if __name__ == "__main__":
    sys.exit(main())
