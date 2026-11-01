"""The light Den: the server's game data for a Den only, from the player's OWN 3.3.5a folder (docs/DEN.md, "A light
local Den"). Nothing is downloaded and nothing is hosted: this reads the player's archives and writes a small game
folder the stock extractor (AzerothCore's map_extractor, the kit's acore/ac-wotlk-tools image) reads instead of
the real one, so it converts a few dozen map tiles instead of the whole world.

    python -m wuhost.denslice tiles [--pack-tiles FILE]          the tiles the Den keeps (map, grid x, grid y, ADT)
    python -m wuhost.denslice stage --game DIR --out DIR [--pack-tiles FILE]
                                                                  writes DIR/Data/common.MPQ and
                                                                  DIR/Data/<locale>/locale-<locale>.MPQ, and
                                                                  DIR/den-slice.json (what went in, sizes, seconds)

What the staged folder holds (each file read from the player's archives, the highest patch winning):
  * every DBFilesClient\\*.dbc and component.wow-<locale>.txt (the server loads all the dbc; the extractor reads
    the build from that file), in the locale archive;
  * the WDT of each map with a kept tile, the kept tiles' ADTs and Cameras\\* (the intro cameras), in common.MPQ.
Kept tiles: the 3 x 3 tiles around the Den (map 1, the island at 16226, 16257: grid 1,1 and its ring, as far as
the map has them), the tile of each starting-area point the worldserver checks before it starts (World.cpp: it
exits with "Failed to find map files for starting areas" unless all eight .map files are there), the Death
Knight start (map 609, where new Death Knights stand before ov_den.lua moves them), and a pack's tiles (packs.py
writes them to state/packs/tiles.json, and the tiles' files under state/packs/<id>/server/, read before the game
folder's own: a pack made from another client's folder brings its own land). Vmaps and mmaps are not made at all: the Den runs with pathing off
(OV_PATHING=0: no vmap or mmap is read, and ExistVMap passes when vmap loading is off).
Grid math (AzerothCore's ComputeGridCoord, and map_extractor's names): gx = 31 - floor(x / 533.333), gy = 31 -
floor(y / 533.333); the server's file maps/<map:03><gx:02><gy:02>.map comes from World\\Maps\\<Name>\\<Name>_<gy>_<gx>.adt.
"""
import argparse
import json
import os
import struct
import sys
import time

try:
    from . import mpq
except ImportError:  # python denslice.py
    import mpq

GRID = 533.3333333
DEN = (1, 16226.2, 16257.0)
# World.cpp (SetInitialWorldSettings): the starting areas checked at startup, then the Death Knight start.
CRITICAL = [
    (0, -6240.32, 331.033), (0, -8949.95, -132.493), (1, -618.518, -4251.67), (0, 1676.35, 1677.45),
    (1, 10311.3, 832.463), (1, -2917.58, -257.98), (530, 10349.6, -6357.29), (530, -3961.64, -13931.2),
    (609, 2355.84, -5662.21),
]


def grid_of(x, y):
    return 31 - int(x // GRID), 31 - int(y // GRID)


def kept_tiles(extra=()):
    """[(map, gx, gy)] sorted, no repeats: the Den's 3 x 3, the critical points, and extra (map, gx, gy)."""
    out = set()
    m, x, y = DEN
    gx, gy = grid_of(x, y)
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            if 0 <= gx + dx < 64 and 0 <= gy + dy < 64:
                out.add((m, gx + dx, gy + dy))
    for m, x, y in CRITICAL:
        out.add((m,) + grid_of(x, y))
    for t in extra:
        m, a, b = (int(v) for v in t)
        if 0 <= a < 64 and 0 <= b < 64:
            out.add((m, a, b))
    return sorted(out)


def read_pack_tiles(path):
    """state/packs/tiles.json: {"tiles": [[map, gx, gy], ...]} (packs.py); missing or bad: none."""
    if not path or not os.path.isfile(path):
        return []
    try:
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
        return [t for t in doc.get("tiles", []) if isinstance(t, list) and len(t) == 3 and all(isinstance(v, int) for v in t)]
    except (OSError, ValueError, AttributeError):
        return []


def dbc_strings(data):
    """A WDBC file: (records, fields, record size, the string block)."""
    if not data or data[:4] != b"WDBC":
        raise mpq.MPQError("not a dbc file")
    n, fields, size, ssize = struct.unpack_from("<4I", data, 4)
    return n, fields, size, data[20 + n * size:20 + n * size + ssize]


def dbc_rows(data):
    n, _fields, size, strings = dbc_strings(data)

    def text(off):
        end = strings.find(b"\0", off)
        return strings[off:end if end >= 0 else len(strings)].decode("utf-8", "replace")
    for i in range(n):
        yield data[20 + i * size:20 + (i + 1) * size], text


def map_names(game):
    """{map id: internal name} from Map.dbc (field 0 the id, field 1 the folder name in every client version)."""
    data = game.read("DBFilesClient\\Map.dbc")
    out = {}
    for row, text in dbc_rows(data):
        mid, off = struct.unpack_from("<II", row, 0)
        out[mid] = text(off)
    return out


def pack_file(packs_dir, name):
    """A map file a pack made (packs.py: state/packs/<id>/server/World/Maps/...), or None."""
    if not packs_dir or not os.path.isdir(packs_dir):
        return None
    rel = os.path.join(*name.split("\\"))
    for d in sorted(os.listdir(packs_dir)):
        p = os.path.join(packs_dir, d, "server", rel)
        if os.path.isfile(p):
            with open(p, "rb") as f:
                return f.read()
    return None


def stage(game_dir, out_dir, extra=(), packs_dir=None):
    t0 = time.time()
    with mpq.GameData(game_dir) as game:
        loc = game.locale
        if not loc:
            raise mpq.MPQError("no locale folder (Data/enUS, Data/enGB, ...) in that game folder")
        comp = game.read("component.wow-%s.txt" % loc)
        if not comp:
            raise mpq.MPQError("no component.wow-%s.txt: is this a 3.3.5a folder?" % loc)
        names = map_names(game)
        locale_files = {"component.wow-%s.txt" % loc: comp}
        for n in game.names("DBFilesClient\\"):
            if n.lower().endswith(".dbc"):
                d = game.read(n)
                if d is not None:
                    locale_files[n] = d
        common, tiles, missing = {}, [], []
        for m, gx, gy in kept_tiles(extra):
            name = names.get(m)
            if not name:
                missing.append([m, gx, gy])
                continue
            wdt = "World\\Maps\\%s\\%s.wdt" % (name, name)
            if wdt not in common:
                d = pack_file(packs_dir, wdt) or game.read(wdt)
                if d is None:
                    missing.append([m, gx, gy])
                    continue
                common[wdt] = d
            adt = "World\\Maps\\%s\\%s_%d_%d.adt" % (name, name, gy, gx)
            d = pack_file(packs_dir, adt) or game.read(adt)
            if d is None:
                missing.append([m, gx, gy])   # sea: the map has no tile there (the island's ring is mostly sea)
                continue
            common[adt] = d
            tiles.append([m, gx, gy])
        for n in game.names("Cameras\\"):
            d = game.read(n)
            if d is not None:
                common[n] = d
        build = game.build()
    data = os.path.join(out_dir, "Data")
    os.makedirs(os.path.join(data, loc), exist_ok=True)
    s1 = mpq.write_archive(os.path.join(data, loc, "locale-%s.MPQ" % loc), locale_files)
    s2 = mpq.write_archive(os.path.join(data, "common.MPQ"), common)
    crit = {(m,) + grid_of(x, y) for m, x, y in CRITICAL[:8]}
    lost = sorted(c for c in crit if list(c) not in tiles)
    report = {"format": "leyline-den-slice", "version": 1, "locale": loc, "build": build, "tiles": tiles,
              "missing": missing, "critical_missing": [list(c) for c in lost],
              "dbc": sum(1 for n in locale_files if n.lower().endswith(".dbc")),
              "bytes": {"dbc": sum(len(v) for v in locale_files.values()), "maps_in": sum(len(v) for v in common.values()),
                        "archives": s1 + s2},
              "seconds": round(time.time() - t0, 1)}
    with open(os.path.join(out_dir, "den-slice.json"), "w", encoding="utf-8") as f:
        json.dump(report, f, indent=1)
    if lost:
        raise mpq.MPQError("the game folder lacks starting-area tiles the server needs: %s" % lost)
    return report


def main(argv=None):
    p = argparse.ArgumentParser(prog="wuhost.denslice", description="The light Den's server data (see the module doc).")
    sub = p.add_subparsers(dest="cmd", required=True)
    t = sub.add_parser("tiles")
    t.add_argument("--pack-tiles")
    s = sub.add_parser("stage")
    s.add_argument("--game", required=True)
    s.add_argument("--out", required=True)
    s.add_argument("--pack-tiles")
    a = p.parse_args(argv)
    extra = read_pack_tiles(a.pack_tiles)
    if a.cmd == "tiles":
        for m, gx, gy in kept_tiles(extra):
            print("map %d grid %02d,%02d  maps/%03d%02d%02d.map" % (m, gx, gy, m, gx, gy))
        return 0
    try:
        r = stage(a.game, a.out, extra, os.path.dirname(a.pack_tiles) if a.pack_tiles else None)
    except (mpq.MPQError, OSError) as e:
        sys.stderr.write("The light Den data couldn't be made: %s\n" % e)
        return 1
    print("Den data staged: %d tiles, %d dbc, %.1f MB in %.1f s" % (len(r["tiles"]), r["dbc"], r["bytes"]["archives"] / 1e6, r["seconds"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
