"""The light local Den and packs (docs/DEN.md, "A light local Den" and "Packs"): hosting/kit/tools/wuhost/mpq.py
(MPQ archives read and written with the standard library), denslice.py (the Den's server data: only the island's
tiles, the starting-area tiles the worldserver checks and the dbc, staged for the stock extractor) and packs.py
(recipes made from the player's own game folder into the Den), under a synthetic game folder built here. No game
files are used or needed; the formats are made by hand to the layouts the code reads.

    python3 -I hosting/tests/test_den_packs.py
"""
import bz2
import json
import os
import re
import struct
import sys
import tempfile
import unittest
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
KIT = os.path.join(ROOT, "hosting", "kit")
sys.path.insert(0, os.path.join(KIT, "tools"))
from wuhost import mpq, denslice, packs  # noqa: E402

def text(*p, mode="r"):
    with open(os.path.join(*p), mode, **({} if "b" in mode else {"encoding": "utf-8"})) as f:
        return f.read()


BRAND = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")


# ---------------------------------------------------------------------------------------------- synthetic files
def dbc(rows, fields):
    """WDBC: rows of ints and strings (strings go to the string block)."""
    strings = bytearray(b"\0")
    recs = bytearray()
    for r in rows:
        for v in r:
            if isinstance(v, str):
                recs += struct.pack("<I", len(strings))
                strings += v.encode() + b"\0"
            else:
                recs += struct.pack("<I", v)
    return b"WDBC" + struct.pack("<4I", len(rows), fields, fields * 4, len(strings)) + bytes(recs) + bytes(strings)


def chunk(tag, body):
    return tag[::-1].encode() + struct.pack("<I", len(body)) + body


def adt(textures, models, wmos, doodads):
    """doodads: [(model index, world x, world y, z, yaw degrees, scale)]."""
    mmdx = b"".join(m.encode() + b"\0" for m in models)
    offs, o = [], 0
    for m in models:
        offs.append(o)
        o += len(m) + 1
    mddf = b"".join(struct.pack("<II3f3fHH", i, n, packs.WORLD - y, z, packs.WORLD - x, 0.0, yaw, 0.0, int(s * 1024), 0)
                    for n, (i, x, y, z, yaw, s) in enumerate(doodads))
    return (chunk("MVER", struct.pack("<I", 18)) + chunk("MTEX", b"".join(t.encode() + b"\0" for t in textures))
            + chunk("MMDX", mmdx) + chunk("MMID", struct.pack("<%dI" % len(offs), *offs))
            + chunk("MWMO", b"".join(w.encode() + b"\0" for w in wmos)) + chunk("MDDF", mddf))


def m2(textures, version=264, views=1):
    names = [t.encode() + b"\0" for t in textures]
    head = bytearray(0x60)
    head[0:4] = b"MD20"
    struct.pack_into("<I", head, 4, version)
    struct.pack_into("<I", head, 0x44, views)
    tex_ofs = len(head)
    at = 0x50 if version >= 264 else 0x54
    struct.pack_into("<II", head, at, len(names), tex_ofs)
    table = bytearray()
    name_ofs = tex_ofs + 16 * len(names)
    blob = bytearray()
    for n in names:
        table += struct.pack("<IIII", 0, 0, len(n), name_ofs + len(blob))
        blob += n
    return bytes(head + table + blob)


def wmo_root(groups, textures, doodads):
    mohd = struct.pack("<II", len(textures), groups) + b"\0" * 56
    return (chunk("MVER", struct.pack("<I", 17)) + chunk("MOHD", mohd)
            + chunk("MOTX", b"".join(t.encode() + b"\0" for t in textures))
            + chunk("MODN", b"".join(d.encode() + b"\0" for d in doodads)))


MAPS = [(0, "Azeroth"), (1, "Kalimdor"), (530, "Expansion01"), (609, "DeathKnightStart")]


def critical_adts():
    out = {}
    names = dict(MAPS)
    for m, x, y in denslice.CRITICAL[:8]:
        gx, gy = denslice.grid_of(x, y)
        out["World\\Maps\\%s\\%s_%d_%d.adt" % (names[m], names[m], gy, gx)] = adt([], [], [], [])
    return out


def make_game(root, build=12340, locale="enUS", extra_common=None, drop=None, patch=None):
    data = os.path.join(root, "Data")
    os.makedirs(os.path.join(data, locale), exist_ok=True)
    common = {"World\\Maps\\%s\\%s.wdt" % (n, n): b"MVER" + b"\0" * 8 for _, n in MAPS}
    common.update(critical_adts())
    common["World\\Maps\\Kalimdor\\Kalimdor_1_1.adt"] = adt([], [], [], [])
    common["World\\Maps\\Kalimdor\\Kalimdor_2_1.adt"] = adt([], [], [], [])
    common["Cameras\\FlyBy.m2"] = b"MD20cam"
    common["World\\Generic\\Far\\Away.m2"] = b"MD20far"     # nowhere near the Den: never staged
    common.update(extra_common or {})
    for k in drop or ():
        common.pop(k, None)
    loc = {"component.wow-%s.txt" % locale: ('<component name="wow-%s" version="%d"/>' % (locale, build)).encode(),
           "DBFilesClient\\Map.dbc": dbc([list(m) for m in MAPS], 2),
           "DBFilesClient\\Spell.dbc": b"WDBC" + b"\0" * 16,
           "DBFilesClient\\GameObjectDisplayInfo.dbc": dbc([[101, "World\\Generic\\Human\\Lamp.mdx"],
                                                            [102, "World\\Generic\\Human\\Barrel.mdx"]], 2)}
    mpq.write_archive(os.path.join(data, "common.MPQ"), common)
    mpq.write_archive(os.path.join(data, locale, "locale-%s.MPQ" % locale), loc)
    if patch:
        mpq.write_archive(os.path.join(data, "patch-2.MPQ"), patch)
    return root


# ---------------------------------------------------------------------------------------------- MPQ
class MpqTests(unittest.TestCase):
    def test_hash_keys(self):
        self.assertEqual(mpq.hash_string("(hash table)", 3), 0xC3AF3770)
        self.assertEqual(mpq.hash_string("(block table)", 3), 0xEC83B3A3)
        self.assertEqual(mpq.hash_string("a/b", 1), mpq.hash_string("A\\B", 1))

    def test_explode_reference(self):
        # zlib's contrib/blast test vector (PKWARE DCL)
        self.assertEqual(mpq.explode(bytes([0, 4, 0x82, 0x24, 0x25, 0x8F, 0x80, 0x7F]), 13), b"AIAIAIAIAIAIA")

    def test_round_trip(self):
        with tempfile.TemporaryDirectory() as d:
            big = os.urandom(3000) + b"x" * 20000
            files = {"A\\one.txt": b"hello", "B\\big.bin": big, "empty.dat": b""}
            mpq.write_archive(os.path.join(d, "t.MPQ"), files)
            with mpq.Archive(os.path.join(d, "t.MPQ")) as a:
                for k, v in files.items():
                    self.assertEqual(a.read(k), v, k)
                self.assertEqual(a.read("a/ONE.TXT"), b"hello")
                self.assertIsNone(a.read("nope"))
                self.assertEqual(sorted(a.names()), sorted(files))

    def test_encrypted_and_bzip2(self):
        """Hand-made: one encrypted sectored zlib file, one single-unit bzip2 file, as the client's archives have."""
        with tempfile.TemporaryDirectory() as d:
            body = b"secret " * 900
            name = "Dir\\locked.txt"
            key = mpq.hash_string("locked.txt", 3)
            sec = 4096
            secs = [b"\x02" + zlib.compress(body[i:i + sec]) for i in range(0, len(body), sec)]
            offs = [(len(secs) + 1) * 4]
            for s in secs:
                offs.append(offs[-1] + len(s))
            table = mpq.encrypt(struct.pack("<%dI" % len(offs), *offs), (key - 1) & mpq.M32)
            blob1 = table + b"".join(mpq.encrypt(s, (key + i) & mpq.M32) for i, s in enumerate(secs))
            blob2 = b"\x10" + bz2.compress(b"bz" * 500)
            hcount = 16
            p1 = 32
            p2 = p1 + len(blob1)
            hpos = p2 + len(blob2)
            entries = [(mpq.HASH_EMPTY, mpq.HASH_EMPTY, 0xFFFF, 0xFFFF, mpq.HASH_EMPTY)] * hcount
            for bi, n in enumerate([name, "z.bin"]):
                i = mpq.hash_string(n, 0) % hcount
                while entries[i][4] != mpq.HASH_EMPTY:
                    i = (i + 1) % hcount
                entries[i] = (mpq.hash_string(n, 1), mpq.hash_string(n, 2), 0, 0, bi)
            ht = b"".join(struct.pack("<IIHHI", *e) for e in entries)
            bt = (struct.pack("<IIII", p1, len(blob1), len(body), mpq.FLAG_EXISTS | mpq.FLAG_COMPRESS | mpq.FLAG_ENCRYPTED)
                  + struct.pack("<IIII", p2, len(blob2), 1000, mpq.FLAG_EXISTS | mpq.FLAG_COMPRESS | mpq.FLAG_SINGLE))
            bpos = hpos + len(ht)
            hdr = struct.pack("<4sIIHHIIII", b"MPQ\x1a", 32, bpos + len(bt), 0, 3, hpos, bpos, hcount, 2)
            with open(os.path.join(d, "h.MPQ"), "wb") as f:
                f.write(hdr + blob1 + blob2 + mpq.encrypt(ht, mpq.hash_string("(hash table)", 3))
                        + mpq.encrypt(bt, mpq.hash_string("(block table)", 3)))
            with mpq.Archive(os.path.join(d, "h.MPQ")) as a:
                self.assertEqual(a.read(name), body)
                self.assertEqual(a.read("z.bin"), b"bz" * 500)

    def test_game_data_order(self):
        with tempfile.TemporaryDirectory() as d:
            make_game(d, patch={"Cameras\\FlyBy.m2": b"MD20new"})
            with mpq.GameData(d) as g:
                self.assertEqual(g.locale, "enUS")
                self.assertEqual(g.build(), 12340)
                self.assertEqual(g.read("Cameras\\FlyBy.m2"), b"MD20new", "a patch wins over the base archive")
                self.assertIn("DBFilesClient\\Map.dbc", g.names("DBFilesClient\\"))


# ---------------------------------------------------------------------------------------------- the light Den
class DenSliceTests(unittest.TestCase):
    def test_grid(self):
        self.assertEqual(denslice.grid_of(16226.2, 16257.0), (1, 1))      # the island
        self.assertEqual(denslice.grid_of(-8949.95, -132.493), (48, 32))   # Northshire
        tiles = denslice.kept_tiles()
        self.assertIn((1, 1, 1), tiles)
        self.assertEqual(sum(1 for t in tiles if t[0] == 1 and t[1] <= 2 and t[2] <= 2), 9)
        for m, x, y in denslice.CRITICAL:
            self.assertIn((m,) + denslice.grid_of(x, y), tiles)
        self.assertLess(len(tiles), 25)
        self.assertIn((0, 49, 31), denslice.kept_tiles([[0, 49, 31]]))

    def test_stage(self):
        with tempfile.TemporaryDirectory() as d:
            game, out = os.path.join(d, "game"), os.path.join(d, "out")
            make_game(game)
            r = denslice.stage(game, out)
            self.assertEqual(r["build"], 12340)
            self.assertEqual(r["critical_missing"], [])
            self.assertIn([1, 1, 1], r["tiles"])
            self.assertTrue(os.path.isfile(os.path.join(out, "den-slice.json")))
            # exactly the layout map_extractor reads: Data/<locale>/locale-<locale>.MPQ (dbc, build) and Data/common.MPQ
            with mpq.Archive(os.path.join(out, "Data", "enUS", "locale-enUS.MPQ")) as a:
                self.assertIn("component.wow-enUS.txt", a.names())
                self.assertIn("DBFilesClient\\Spell.dbc", a.names(), "every dbc, listed (the extractor lists them)")
            with mpq.Archive(os.path.join(out, "Data", "common.MPQ")) as a:
                n = set(a.names())
                self.assertIn("World\\Maps\\Kalimdor\\Kalimdor.wdt", n)
                self.assertIn("World\\Maps\\Kalimdor\\Kalimdor_1_1.adt", n)
                self.assertIn("World\\Maps\\Azeroth\\Azeroth_32_48.adt", n, "Northshire: maps/0004832.map")
                self.assertIn("Cameras\\FlyBy.m2", n)
                self.assertNotIn("World\\Generic\\Far\\Away.m2", n)
            self.assertLess(r["bytes"]["archives"], 200000)

    def test_stage_needs_starting_areas(self):
        with tempfile.TemporaryDirectory() as d:
            game = os.path.join(d, "game")
            make_game(game, drop=["World\\Maps\\Azeroth\\Azeroth_32_48.adt"])
            with self.assertRaises(mpq.MPQError):
                denslice.stage(game, os.path.join(d, "out"))
            self.assertEqual(denslice.main(["stage", "--game", game, "--out", os.path.join(d, "o2")]), 1)

    def test_pack_tiles_override(self):
        with tempfile.TemporaryDirectory() as d:
            game, state = os.path.join(d, "game"), os.path.join(d, "state")
            make_game(game)
            p = os.path.join(state, "packs", "x", "server", "World", "Maps", "Kalimdor")
            os.makedirs(p)
            with open(os.path.join(p, "Kalimdor_1_1.adt"), "wb") as f:
                f.write(b"PACK")
            os.makedirs(os.path.join(state, "packs"), exist_ok=True)
            with open(os.path.join(state, "packs", "tiles.json"), "w") as f:
                json.dump({"tiles": [[1, 1, 1]]}, f)
            out = os.path.join(d, "out")
            self.assertEqual(denslice.main(["stage", "--game", game, "--out", out,
                                            "--pack-tiles", os.path.join(state, "packs", "tiles.json")]), 0)
            with mpq.Archive(os.path.join(out, "Data", "common.MPQ")) as a:
                self.assertEqual(a.read("World\\Maps\\Kalimdor\\Kalimdor_1_1.adt"), b"PACK")

    def test_kit_scripts(self):
        sh = text(KIT, "host-wrath-realm.sh")
        raw = text(KIT, "Host a Wrath realm.bat", mode="rb")
        bat = raw.decode("ascii")
        for body in (sh, bat):
            self.assertIn("wuhost.denslice stage", body)
            self.assertIn("den-light.json", body)
            self.assertIn("map_extractor -i /wow -o /out", body)
            self.assertIn("packs\\restage" if body is bat else "packs/restage", body)
            self.assertIn("WU_DEN_FULL_DATA", body)
        # the fallback: the whole world when the Den-only data fails
        self.assertIn("taking the whole world instead", sh)
        self.assertIn("taking the whole world instead", bat)
        self.assertTrue(raw.count(b"\r\n") > 100 and b"\n" not in raw.replace(b"\r\n", b""), "the .bat keeps CRLF")


# ---------------------------------------------------------------------------------------------- packs
class PackTests(unittest.TestCase):
    def test_shipped_recipes(self):
        rs = {r["id"]: r for r in packs.list_recipes()}
        self.assertEqual(sorted(rs), ["goldshire", "human-buildings", "mounts"])
        self.assertEqual(packs.tiles_of(rs["goldshire"]["tiles"][0]["box"]), [(49, 31), (49, 32), (50, 31), (50, 32)])
        for name in os.listdir(packs.RECIPES):
            with open(os.path.join(packs.RECIPES, name), encoding="utf-8") as f:
                doc = json.load(f)
            for k in ("title", "line", "comment"):
                self.assertNotRegex(doc.get(k, ""), BRAND, name)

    def test_recipe_checks(self):
        ok = {"format": "leyline-pack-recipe", "version": 1, "id": "x", "title": "X", "line": "x", "files": ["A\\b.m2"]}
        packs.check_recipe(ok)
        for bad in ({"files": ["..\\up.m2"]}, {"files": ["C:\\x.m2"]}, {"files": ["a.exe"]}, {"id": "Bad Id"},
                    {"version": 2}, {"run": "rm -rf /"}, {"tiles": [{"map": 0, "box": [0, 9000, 0, 1]}]},
                    {"layout": {"centre": [0, 0, 0]}}, {"limits": {"mb": 99999}}):
            doc = dict(ok, **bad)
            with self.assertRaises(packs.PackError, msg=str(bad)):
                packs.check_recipe(doc)

    def _goldshire_game(self, d, build):
        cx, cy, cz = -9448.55, 68.24, 56.32
        names = dict(MAPS)
        gx, gy = denslice.grid_of(cx, cy)
        tile = "World\\Maps\\Azeroth\\Azeroth_%d_%d.adt" % (gy, gx)
        extra = {
            tile: adt(["Tileset\\Elwynn\\Grass.blp"], ["World\\Generic\\Human\\Lamp.mdx", "World\\Generic\\Human\\Barrel.mdx",
                                                       "World\\Generic\\Odd\\Unknown.m2"],
                      ["World\\wmo\\Azeroth\\Buildings\\Inn\\Inn.wmo"],
                      [(0, cx + 5, cy, cz, 10.0, 1.0), (1, cx - 3, cy + 4, cz + 0.5, 0.0, 2.0),
                       (2, cx + 1, cy + 1, cz, 0.0, 1.0), (0, cx + 200, cy, cz, 0.0, 1.0)]),
            "Tileset\\Elwynn\\Grass.blp": b"BLP2grass",
            "World\\Generic\\Human\\Lamp.m2": m2(["World\\Generic\\Human\\Lamp.blp"]),
            "World\\Generic\\Human\\Lamp00.skin": b"SKIN",
            "World\\Generic\\Human\\Lamp.blp": b"BLP2lamp",
            "World\\Generic\\Human\\Barrel.m2": m2(["World\\Generic\\Human\\Barrel.blp"]),
            "World\\Generic\\Human\\Barrel00.skin": b"SKIN",
            "World\\Generic\\Human\\Barrel.blp": b"BLP2barrel",
            "World\\Generic\\Odd\\Unknown.m2": m2([]),
            "World\\wmo\\Azeroth\\Buildings\\Inn\\Inn.wmo": wmo_root(2, ["World\\wmo\\Inn\\Wall.blp"], []),
            "World\\wmo\\Azeroth\\Buildings\\Inn\\Inn_000.wmo": b"GROUP0",
            "World\\wmo\\Azeroth\\Buildings\\Inn\\Inn_001.wmo": b"GROUP1",
            "World\\wmo\\Inn\\Wall.blp": b"BLP2wall",
            "Sound\\Ambience\\Inn.wav": b"RIFF",
        }
        self.assertEqual(names[0], "Azeroth")
        return make_game(d, build=build, extra_common=extra), tile

    def test_build_goldshire(self):
        with tempfile.TemporaryDirectory() as d:
            game, tile = self._goldshire_game(os.path.join(d, "game"), 5875)   # an older client's folder
            state = os.path.join(d, "state")
            m = packs.build("goldshire", game, state)
            self.assertEqual(m["source_build"], 5875)
            self.assertEqual(m["tiles"], [[0, 49, 31]])
            got = set(m["client_files"])
            for want in ("World\\Generic\\Human\\Lamp.m2", "World\\Generic\\Human\\Lamp00.skin", "World\\Generic\\Human\\Lamp.blp",
                         "Tileset\\Elwynn\\Grass.blp", "World\\wmo\\Azeroth\\Buildings\\Inn\\Inn_001.wmo", "World\\wmo\\Inn\\Wall.blp"):
                self.assertIn(want, got, "dependencies are walked: " + want)
            self.assertFalse(any(n.lower().endswith(".wav") for n in got), "no sound")
            self.assertFalse(any(n.lower().endswith(".adt") for n in got), "land goes to the server, not the patch")
            # the server's land, read by the light Den's next start
            self.assertTrue(os.path.isfile(os.path.join(state, "packs", "goldshire", "server", *tile.split("\\"))))
            with open(os.path.join(state, "packs", "tiles.json")) as f:
                self.assertEqual(json.load(f)["tiles"], [[0, 49, 31]])
            self.assertTrue(os.path.isfile(os.path.join(state, "packs", "restage")))
            # the client patch, readable
            with mpq.Archive(os.path.join(state, "packs", "client", "patch-L.MPQ")) as a:
                self.assertEqual(a.read("World\\Generic\\Human\\Lamp.blp"), b"BLP2lamp")
            # the layout: the two known doodads near the inn, as model props relative to it
            with open(os.path.join(state, "dens", "pack-goldshire.den.json")) as f:
                lay = json.load(f)
            self.assertEqual(lay["format"], "leyline-den")
            self.assertEqual(lay["version"], 1)
            self.assertEqual(lay["packs"], ["goldshire"])
            self.assertEqual(sorted(p["model"] for p in lay["props"]), [101, 102])
            lamp = [p for p in lay["props"] if p["model"] == 101][0]
            self.assertEqual(lamp["entry"], 2000000101)
            self.assertAlmostEqual(lamp["x"], 5.0, places=2)
            self.assertAlmostEqual(lamp["y"], 0.0, places=2)
            barrel = [p for p in lay["props"] if p["model"] == 102][0]
            self.assertAlmostEqual(barrel["scale"], 2.0, places=2)
            self.assertEqual(m["props_unknown"], 1, "a doodad with no display row is left out")
            with open(os.path.join(state, "dens", "packs.json")) as f:
                self.assertEqual(json.load(f)["packs"], [{"id": "goldshire", "title": "Goldshire", "version": 1}])

    def test_same_client_no_patch(self):
        with tempfile.TemporaryDirectory() as d:
            game, _ = self._goldshire_game(os.path.join(d, "game"), 12340)
            state = os.path.join(d, "state")
            m = packs.build("goldshire", game, state)
            self.assertEqual(m["client_files"], [], "the 3.3.5a folder already has these files")
            self.assertFalse(os.path.exists(os.path.join(state, "packs", "client", "patch-L.MPQ")))
            m = packs.build("goldshire", game, state, client_patch="always")
            self.assertTrue(m["client_files"])

    def test_prefix_pack_and_limits(self):
        with tempfile.TemporaryDirectory() as d:
            extra = {"Creature\\Horse\\Horse.m2": m2(["Creature\\Horse\\Horse.blp"]), "Creature\\Horse\\Horse00.skin": b"S",
                     "Creature\\Horse\\Horse.blp": b"B", "Creature\\Ram\\Ram.m2": m2([], version=256)}
            game = make_game(os.path.join(d, "game"), build=8606, extra_common=extra)
            state = os.path.join(d, "state")
            m = packs.build("mounts", game, state)
            self.assertEqual(m["tiles"], [])
            self.assertIn("Creature\\Horse\\Horse.blp", m["client_files"])
            self.assertIn("Creature\\Ram\\Ram.m2", m["client_files"])
            self.assertFalse(os.path.exists(os.path.join(state, "packs", "restage")), "no land: nothing to re-stage")
            self.assertFalse(os.path.exists(os.path.join(state, "dens", "pack-mounts.den.json")))
            # two packs: one client patch holds both
            game2, _ = self._goldshire_game(os.path.join(d, "g2"), 8606)
            packs.build("goldshire", game2, state)
            with mpq.Archive(os.path.join(state, "packs", "client", "patch-L.MPQ")) as a:
                self.assertIsNotNone(a.read("Creature\\Horse\\Horse.blp"))
                self.assertIsNotNone(a.read("World\\Generic\\Human\\Lamp.blp"))
            with open(os.path.join(state, "dens", "packs.json")) as f:
                self.assertEqual([p["id"] for p in json.load(f)["packs"]], ["goldshire", "mounts"])
            # a recipe's limits stop a runaway pack
            rd = os.path.join(d, "recipes")
            os.makedirs(rd)
            with open(os.path.join(rd, "tiny.json"), "w") as f:
                json.dump({"format": "leyline-pack-recipe", "version": 1, "id": "tiny", "title": "T", "line": "t",
                           "prefixes": [{"path": "Creature\\", "ext": []}], "limits": {"files": 2}}, f)
            with self.assertRaises(packs.PackError):
                packs.build("tiny", game, state, folder=rd)

    def test_den_lua_reads_packs(self):
        lua = text(ROOT, "realm", "custom", "lua_scripts", "ov_den.lua")
        self.assertIn('SAVES .. "/packs.json"', lua)
        self.assertIn("packs = #packs > 0 and packs or nil", lua)
        self.assertIn("leyline pack build", lua)
        self.assertIn("pack|packs) shift; cmd_pack", text(KIT, "leyline"))


if __name__ == "__main__":
    unittest.main()
