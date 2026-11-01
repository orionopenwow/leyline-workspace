"""Packs: parts of the game brought into a local Den from the player's OWN game folder (docs/DEN.md, "Packs").
Leyline ships recipes only (tools/wuhost/recipes/<id>.json: which tiles, which files, which doodads become Den
props); the player's own archives fill them, on their own PC. Nothing here downloads or uploads game files.

    python -m wuhost.packs list                                   the recipes in this kit
    python -m wuhost.packs show <id>                              one recipe, checked
    python -m wuhost.packs build <id> --game DIR --state DIR [--client-patch auto|always|never]
                                                                   makes the pack into the Den's state folder

A recipe (format "leyline-pack-recipe", version 1; RECIPE_KEYS below, nothing else):
  id, title, line                   [a-z0-9-] id, a short title and one plain line for the player
  tiles   [{map, box: [x min, x max, y min, y max]}]   map tiles: the server's terrain there (the Den's light data
                                    adds them: state/packs/tiles.json, then the next start re-stages) and, with deps,
                                    every texture, doodad and building model those tiles show
  files   [paths]                   single files;   prefixes [{path, ext: [".m2", ...]}]: every file under a folder
  deps    true                      walk each ADT, M2 and WMO for the files it needs (textures, skins, WMO groups)
  layout  {centre: [x, y, z], radius, max}   the tiles' doodads near centre become a Den layout (each a model prop:
                                    GameObjectDisplayInfo's row for that model), relative to centre, at most max props
  limits  {files, mb}               the most a build may take (default 4000 files, 300 MB)
What a build writes under the Den's state folder (the host kit's state/):
  packs/<id>/pack.json             the recipe id, the source client's build, files, bytes, tiles, models, when
  packs/<id>/server/World/Maps/... the ADTs of its tiles (denslice.py reads them before the game folder's own)
  packs/tiles.json, packs/restage  every pack's tiles; the flag that makes the next start re-stage the Den's data
  packs/client/patch-L.MPQ         client files from every pack (only when the source client isn't the 3.3.5a one
                                    the Den plays with, or --client-patch always): copy it to the game's Data folder
  dens/pack-<id>.den.json          the layout as a Den file (.den load pack-<id> add), "packs": [id]
  dens/packs.json                  {"packs": [{id, title, version}]}: the packs this Den has (ov_den.lua reads it,
                                    to tell a friend opening a Den file which packs to make from their own folder)
"""
import argparse
import json
import math
import os
import re
import struct
import sys
import time

try:
    from . import mpq
    from . import denslice
except ImportError:
    import mpq
    import denslice

HERE = os.path.dirname(os.path.abspath(__file__))
RECIPES = os.path.join(HERE, "recipes")
ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,23}$")
RECIPE_KEYS = {"format", "version", "id", "title", "line", "tiles", "files", "prefixes", "deps", "layout", "limits", "comment"}
EXTS = {".m2", ".skin", ".blp", ".wmo", ".adt", ".wdt", ".anim", ".dbc", ".mdx"}
TARGET_BUILD = 12340          # the 3.3.5a client a local Den plays with
MODEL_BASE = 2000000000       # ov_build.lua's X.MODEL_BASE: a model prop's entry
SOUND = re.compile(r"\.(wav|mp3|ogg)$", re.I)
WORLD = 17066.666666          # ADT placement space: world x = WORLD - pos z, world y = WORLD - pos x


class PackError(Exception):
    pass


def _path_ok(p):
    return isinstance(p, str) and 0 < len(p) <= 260 and ".." not in p and not p.startswith(("\\", "/")) and ":" not in p


def check_recipe(doc):
    """The recipe, checked (paths as backslashes), or PackError naming what is wrong."""
    if not isinstance(doc, dict) or doc.get("format") != "leyline-pack-recipe":
        raise PackError("not a pack recipe")
    if doc.get("version") != 1:
        raise PackError("recipe version must be 1")
    extra = set(doc) - RECIPE_KEYS
    if extra:
        raise PackError("unknown recipe keys: %s" % ", ".join(sorted(extra)))
    if not ID_RE.match(str(doc.get("id", ""))):
        raise PackError("bad recipe id")
    for k in ("title", "line"):
        if not isinstance(doc.get(k), str) or not doc[k] or len(doc[k]) > 120:
            raise PackError("recipe needs a %s" % k)
    out = {"id": doc["id"], "title": doc["title"], "line": doc["line"], "tiles": [], "files": [], "prefixes": [],
           "deps": doc.get("deps", True) is True, "layout": None, "limits": {"files": 4000, "mb": 300}}
    for t in doc.get("tiles", []):
        b = t.get("box") if isinstance(t, dict) else None
        if not (isinstance(t, dict) and isinstance(t.get("map"), int) and isinstance(b, list) and len(b) == 4
                and all(isinstance(v, (int, float)) for v in b) and b[0] < b[1] and b[2] < b[3]):
            raise PackError("a tile entry needs map and box [x min, x max, y min, y max]")
        if (b[1] - b[0]) > 4000 or (b[3] - b[2]) > 4000:
            raise PackError("a tile box is at most 4000 yards a side")
        out["tiles"].append({"map": t["map"], "box": [float(v) for v in b]})
    for p in doc.get("files", []):
        if not _path_ok(p) or os.path.splitext(p)[1].lower() not in EXTS:
            raise PackError("bad file path: %r" % (p,))
        out["files"].append(p.replace("/", "\\"))
    for p in doc.get("prefixes", []):
        if not (isinstance(p, dict) and _path_ok(p.get("path")) and isinstance(p.get("ext", []), list)
                and all(e in EXTS for e in p.get("ext", []))):
            raise PackError("a prefix needs path and ext (some of %s)" % ", ".join(sorted(EXTS)))
        out["prefixes"].append({"path": p["path"].replace("/", "\\"), "ext": list(p.get("ext") or [])})
    lay = doc.get("layout")
    if lay is not None:
        c = lay.get("centre") if isinstance(lay, dict) else None
        if not (isinstance(c, list) and len(c) == 3 and all(isinstance(v, (int, float)) for v in c)):
            raise PackError("layout needs centre [x, y, z]")
        r, mx = lay.get("radius", 28), lay.get("max", 120)
        if not (isinstance(r, (int, float)) and 1 <= r <= 40 and isinstance(mx, int) and 1 <= mx <= 400):
            raise PackError("layout radius 1-40 yards, max 1-400 props")
        if not out["tiles"]:
            raise PackError("a layout comes from tiles")
        out["layout"] = {"centre": [float(v) for v in c], "radius": float(r), "max": mx}
    lim = doc.get("limits", {})
    if not isinstance(lim, dict):
        raise PackError("bad limits")
    for k, hi in (("files", 20000), ("mb", 2000)):
        if k in lim:
            if not (isinstance(lim[k], int) and 1 <= lim[k] <= hi):
                raise PackError("limits.%s is 1-%d" % (k, hi))
            out["limits"][k] = lim[k]
    if not (out["tiles"] or out["files"] or out["prefixes"]):
        raise PackError("a recipe needs tiles, files or prefixes")
    return out


def load_recipe(rid, folder=RECIPES):
    if not ID_RE.match(rid or ""):
        raise PackError("bad recipe id")
    p = os.path.join(folder, rid + ".json")
    if not os.path.isfile(p):
        raise PackError("no recipe called %s" % rid)
    with open(p, encoding="utf-8") as f:
        return check_recipe(json.load(f))


def list_recipes(folder=RECIPES):
    out = []
    for n in sorted(os.listdir(folder)) if os.path.isdir(folder) else []:
        if n.endswith(".json"):
            try:
                out.append(load_recipe(n[:-5], folder))
            except (PackError, ValueError, OSError):
                continue
    return out


# ---------------------------------------------------------------------------------------------- the game's formats
def chunks(data):
    """An ADT or WMO's chunks: [(four letters, bytes)], the letters as read (the files store them reversed)."""
    out, p = [], 0
    while p + 8 <= len(data):
        tag, size = data[p:p + 4][::-1].decode("latin-1"), struct.unpack_from("<I", data, p + 4)[0]
        out.append((tag, data[p + 8:p + 8 + size]))
        p += 8 + size
    return out


def zstrings(b):
    return [s.decode("utf-8", "replace") for s in b.split(b"\0") if s]


def model_path(p):
    """A doodad's name as the archives hold it: .mdx / .mdl are .m2 since 2.0."""
    p = p.replace("/", "\\")
    root, ext = os.path.splitext(p)
    return root + ".m2" if ext.lower() in (".mdx", ".mdl") else p


def adt_deps(data):
    """(textures, models, wmos, doodads) of an ADT. doodads: [(model, world x, y, z, yaw degrees, scale)]."""
    tex, mdx, wmo, mmid, mddf = [], [], [], [], b""
    raw_mmdx = b""
    for tag, body in chunks(data):
        if tag == "MTEX":
            tex = zstrings(body)
        elif tag == "MMDX":
            raw_mmdx, mdx = body, zstrings(body)
        elif tag == "MWMO":
            wmo = zstrings(body)
        elif tag == "MMID":
            mmid = list(struct.unpack("<%dI" % (len(body) // 4), body[:len(body) // 4 * 4]))
        elif tag == "MDDF":
            mddf = body
    doodads = []
    for i in range(len(mddf) // 36):
        nid, _uid, px, py, pz, _rx, ry, _rz, scale, _flags = struct.unpack_from("<II3f3fHH", mddf, i * 36)
        if nid < len(mmid):
            off = mmid[nid]
            end = raw_mmdx.find(b"\0", off)
            name = raw_mmdx[off:end if end >= 0 else len(raw_mmdx)].decode("utf-8", "replace")
            # ADT placement space to the world (to verify in game: the facing; positions are the usual conversion)
            doodads.append((model_path(name), WORLD - pz, WORLD - px, py, ry, scale / 1024.0))
    return tex, [model_path(m) for m in mdx], wmo, doodads


def m2_deps(path, data):
    """The textures and skins an M2 needs (2.4.3 / 1.12 models: v256-263, textures at 0x54; 3.3.5a: v264, at 0x50,
    skins <name>00.skin...). To verify per client: older models keep their skins inside the file."""
    if len(data) < 0x5c or data[:4] != b"MD20":
        return []
    ver = struct.unpack_from("<I", data, 4)[0]
    out = []
    at = 0x50 if ver >= 264 else 0x54
    n, ofs = struct.unpack_from("<II", data, at)
    for i in range(min(n, 64)):
        e = ofs + i * 16
        if e + 16 > len(data):
            break
        kind, _flags, ln, no = struct.unpack_from("<IIII", data, e)
        if kind == 0 and ln > 1 and no + ln <= len(data):
            out.append(data[no:no + ln].split(b"\0")[0].decode("utf-8", "replace"))
    if ver >= 264:
        views = struct.unpack_from("<I", data, 0x44)[0]
        base = os.path.splitext(path)[0]
        out += ["%s%02d.skin" % (base, k) for k in range(min(views, 4))]
    return out


def wmo_deps(path, data):
    """A WMO root's textures, doodad models and group files."""
    out, groups = [], 0
    for tag, body in chunks(data):
        if tag == "MOHD" and len(body) >= 8:
            groups = struct.unpack_from("<I", body, 4)[0]
        elif tag == "MOTX":
            out += zstrings(body)
        elif tag == "MODN":
            out += [model_path(m) for m in zstrings(body)]
    base = os.path.splitext(path)[0]
    out += ["%s_%03d.wmo" % (base, g) for g in range(min(groups, 512))]
    return out


def tiles_of(box):
    """The (gx, gy) grid tiles a world box covers (denslice's grid math)."""
    x1, x2, y1, y2 = box
    gxs = sorted({denslice.grid_of(x, 0)[0] for x in (x1, x2)})
    gys = sorted({denslice.grid_of(0, y)[1] for y in (y1, y2)})
    return [(gx, gy) for gx in range(gxs[0], gxs[-1] + 1) for gy in range(gys[0], gys[-1] + 1)]


def display_ids(game):
    """{model path lowercased without extension: GameObjectDisplayInfo id} (field 0 the id, field 1 the model)."""
    data = game.read("DBFilesClient\\GameObjectDisplayInfo.dbc")
    out = {}
    if not data:
        return out
    for row, text in denslice.dbc_rows(data):
        did, off = struct.unpack_from("<II", row, 0)
        key = os.path.splitext(text(off).replace("/", "\\").lower())[0]
        if key and key not in out:
            out[key] = did
    return out


# ---------------------------------------------------------------------------------------------- building a pack
def collect(recipe, game):
    """{path: bytes} the pack takes, the tiles [(map, gx, gy, adt path)], the doodads near the layout's centre."""
    want, got, skipped = [], {}, []
    names = denslice.map_names(game) if recipe["tiles"] else {}
    tiles, doodads = [], []
    lim_files, lim_bytes = recipe["limits"]["files"], recipe["limits"]["mb"] * 1024 * 1024
    for t in recipe["tiles"]:
        name = names.get(t["map"])
        if not name:
            raise PackError("this game folder has no map %d" % t["map"])
        wdt = "World\\Maps\\%s\\%s.wdt" % (name, name)
        want.append(wdt)
        for gx, gy in tiles_of(t["box"]):
            adt = "World\\Maps\\%s\\%s_%d_%d.adt" % (name, name, gy, gx)
            if game.has(adt):
                tiles.append((t["map"], gx, gy, adt))
                want.append(adt)
    want += recipe["files"]
    for p in recipe["prefixes"]:
        for n in game.names(p["path"]):
            if not p["ext"] or os.path.splitext(n)[1].lower() in p["ext"]:
                want.append(n)
    total = 0
    seen = set()
    while want:
        n = want.pop(0)
        k = n.lower()
        if k in seen or SOUND.search(n):
            continue
        seen.add(k)
        d = game.read(n)
        if d is None:
            skipped.append(n)
            continue
        got[n] = d
        total += len(d)
        if len(got) > lim_files or total > lim_bytes:
            raise PackError("the pack grew past its limits (%d files, %d MB)" % (recipe["limits"]["files"], recipe["limits"]["mb"]))
        if not recipe["deps"]:
            continue
        ext = os.path.splitext(n)[1].lower()
        if ext == ".adt":
            tex, mdx, wmo, dd = adt_deps(d)
            want += tex + mdx + wmo
            doodads += dd
        elif ext == ".m2":
            want += m2_deps(n, d)
        elif ext == ".wmo" and not re.search(r"_\d{3}\.wmo$", n, re.I):
            want += wmo_deps(n, d)
    return got, tiles, doodads, skipped


def layout_of(recipe, doodads, displays):
    """The layout's props (a Den file's props): doodads within radius of centre that have a display row."""
    lay = recipe["layout"]
    if not lay:
        return [], 0
    cx, cy, cz = lay["centre"]
    props, unknown = [], 0
    near = sorted(doodads, key=lambda d: (d[1] - cx) ** 2 + (d[2] - cy) ** 2)
    for model, x, y, z, yaw, scale in near:
        if math.hypot(x - cx, y - cy) > lay["radius"] or abs(z - cz) > 30:
            continue
        did = displays.get(os.path.splitext(model.lower())[0])
        if not did:
            unknown += 1
            continue
        props.append({"entry": MODEL_BASE + did, "model": did, "x": round(x - cx, 3), "y": round(y - cy, 3),
                      "z": round(z - cz, 3), "yaw": round((yaw + 90.0) % 360.0, 2), "scale": round(min(5.0, max(0.1, scale)), 3)})
        if len(props) >= lay["max"]:
            break
    return props, unknown


def _write_json(path, doc):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=1, sort_keys=True)
        f.write("\n")
    os.replace(tmp, path)


def _read_json(path, default):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def build(rid, game_dir, state, client_patch="auto", folder=RECIPES):
    """Makes pack rid from the game folder into state (the kit's state/). Returns pack.json's table."""
    recipe = load_recipe(rid, folder)
    t0 = time.time()
    with mpq.GameData(game_dir) as game:
        source = game.build()
        got, tiles, doodads, skipped = collect(recipe, game)
        displays = display_ids(game) if recipe["layout"] else {}
    if not got:
        raise PackError("nothing of %s is in that game folder" % recipe["title"])
    props, unknown = layout_of(recipe, doodads, displays)
    packs = os.path.join(state, "packs")
    home = os.path.join(packs, rid)
    for m, gx, gy, adt in tiles:   # the server's terrain: read by denslice.py before the game folder's own
        p = os.path.join(home, "server", *adt.split("\\"))
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "wb") as f:
            f.write(got[adt])
    wdts = {n for n in got if n.lower().endswith(".wdt")}
    for n in wdts:
        p = os.path.join(home, "server", *n.split("\\"))
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "wb") as f:
            f.write(got[n])
    patch = client_patch == "always" or (client_patch == "auto" and source != TARGET_BUILD)
    client = sorted(n for n in got if not n.lower().endswith((".adt", ".wdt"))) if patch else []
    man = {"format": "leyline-pack", "version": 1, "id": rid, "title": recipe["title"], "line": recipe["line"],
           "source_build": source, "files": len(got), "bytes": sum(len(v) for v in got.values()),
           "tiles": [[m, gx, gy] for m, gx, gy, _ in tiles], "client_files": client, "missing": skipped[:200],
           "props": len(props), "props_unknown": unknown, "made": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
           "seconds": round(time.time() - t0, 1)}
    _write_json(os.path.join(home, "pack.json"), man)
    # every pack's tiles, and the flag for the next start (host-wrath-realm.sh client_data re-stages the Den)
    all_tiles = set()
    for d in sorted(os.listdir(packs)):
        pj = _read_json(os.path.join(packs, d, "pack.json"), None)
        if isinstance(pj, dict) and pj.get("format") == "leyline-pack":
            all_tiles.update(tuple(t) for t in pj.get("tiles", []))
    _write_json(os.path.join(packs, "tiles.json"), {"tiles": sorted(list(t) for t in all_tiles)})
    if tiles:
        with open(os.path.join(packs, "restage"), "w") as f:
            f.write(rid + "\n")
    if patch:
        merged = {}
        for d in sorted(os.listdir(packs)):
            pj = _read_json(os.path.join(packs, d, "pack.json"), None)
            if isinstance(pj, dict) and pj.get("client_files") and d != rid:
                merged.update(_read_client(os.path.join(packs, "client", "patch-L.MPQ"), pj["client_files"]))
        merged.update({n: got[n] for n in client})
        os.makedirs(os.path.join(packs, "client"), exist_ok=True)
        mpq.write_archive(os.path.join(packs, "client", "patch-L.MPQ"), merged)
    dens = os.path.join(state, "dens")
    if props:
        _write_json(os.path.join(dens, "pack-%s.den.json" % rid), {
            "format": "leyline-den", "version": 1, "name": "pack-" + rid, "title": recipe["title"], "packs": [rid],
            "room": {"look": "white"}, "props": props, "paths": []})
    have = _read_json(os.path.join(dens, "packs.json"), {"packs": []})
    lst = [p for p in have.get("packs", []) if isinstance(p, dict) and p.get("id") != rid]
    lst.append({"id": rid, "title": recipe["title"], "version": 1})
    _write_json(os.path.join(dens, "packs.json"), {"packs": sorted(lst, key=lambda p: p["id"])})
    return man


def _read_client(path, names):
    out = {}
    if not os.path.isfile(path):
        return out
    try:
        with mpq.Archive(path) as a:
            for n in names:
                d = a.read(n)
                if d is not None:
                    out[n] = d
    except (OSError, mpq.MPQError, struct.error):
        pass
    return out


def main(argv=None):
    p = argparse.ArgumentParser(prog="wuhost.packs", description="Packs from your own game folder (see the module doc).")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("list")
    s = sub.add_parser("show")
    s.add_argument("id")
    b = sub.add_parser("build")
    b.add_argument("id")
    b.add_argument("--game", required=True)
    b.add_argument("--state", required=True)
    b.add_argument("--client-patch", choices=["auto", "always", "never"], default="auto")
    a = p.parse_args(argv)
    try:
        if a.cmd == "list":
            for r in list_recipes():
                print("%-16s %s: %s" % (r["id"], r["title"], r["line"]))
            return 0
        if a.cmd == "show":
            print(json.dumps(load_recipe(a.id), indent=1))
            return 0
        m = build(a.id, a.game, a.state, a.client_patch)
    except (PackError, mpq.MPQError, OSError) as e:
        sys.stderr.write("The pack couldn't be made: %s\n" % e)
        return 1
    print("%s: %d files (%.1f MB), %d map tiles, %d Den props, in %.1f s." % (m["title"], m["files"], m["bytes"] / 1e6,
                                                                              len(m["tiles"]), m["props"], m["seconds"]))
    if m["tiles"]:
        print("Restart your Den to bring its land in.")
    if m["props"]:
        print("In your Den: .den load pack-%s add" % m["id"])
    if m["client_files"]:
        print("Copy state/packs/client/patch-L.MPQ into your game's Data folder to see it in your own game.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
