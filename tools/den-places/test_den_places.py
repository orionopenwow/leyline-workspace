"""Tests for den_places.py (the place importer) and gen_teleports.py (the teleports list).

    python3 tools/den-places/test_den_places.py

Every game file here is made up by the tests (fake_* below: a map tile, a building, a map of one building, the two
tables, an M2's header, written into MPQ archives of our own): no game data.
"""
import json
import math
import os
import struct
import subprocess
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "den-check"))
import den_places as dp  # noqa: E402
import gen_teleports  # noqa: E402
import mpqlite  # noqa: E402

M32 = 0xFFFFFFFF


# ---------------------------------------------------------------- an MPQ writer of our own (tests only)

def encrypt(words, key):
    out, s2 = [], 0xEEEEEEEE
    for w in words:
        s2 = (s2 + mpqlite.CRYPT[0x400 + (key & 0xFF)]) & M32
        out.append((w ^ ((key + s2) & M32)) & M32)
        key = ((((~key) << 0x15) + 0x11111111) | (key >> 0x0B)) & M32
        s2 = (w + s2 + (s2 << 5) + 3) & M32
    return out


def write_mpq(path, files, sectors=()):
    """An archive (format 0) with `files` {name: bytes}: stored whole, or in zlib sectors for the names in `sectors`."""
    files = dict(files)
    files["(listfile)"] = "\r\n".join(files).encode()
    names = list(files)
    hcount = 16
    while hcount < len(names) * 2:
        hcount *= 2
    shift = 0
    sector = 512 << shift
    body, blocks = bytearray(), []
    for n in names:
        data = files[n]
        at = 32 + len(body)
        if n in sectors and data:
            parts = [data[i:i + sector] for i in range(0, len(data), sector)]
            packed = []
            for p in parts:
                z = b"\x02" + zlib.compress(p)
                packed.append(z if len(z) < len(p) else p)
            offs, o = [], 4 * (len(parts) + 1)
            for p in packed:
                offs.append(o)
                o += len(p)
            offs.append(o)
            raw = struct.pack("<%dI" % len(offs), *offs) + b"".join(packed)
            flags = mpqlite.FLAG_EXISTS | mpqlite.FLAG_COMPRESS
        else:
            raw = data
            flags = mpqlite.FLAG_EXISTS | mpqlite.FLAG_SINGLE_UNIT
        body += raw
        blocks.append((at, len(raw), len(data), flags))
    table = [(M32, M32, M32, M32)] * hcount
    for i, n in enumerate(names):
        k = mpqlite.hash_name(n, 0) & (hcount - 1)
        while table[k][3] != M32:
            k = (k + 1) & (hcount - 1)
        table[k] = (mpqlite.hash_name(n, 1), mpqlite.hash_name(n, 2), 0, i)
    hpos = 32 + len(body)
    hwords = encrypt([w for e in table for w in e], mpqlite.hash_name("(hash table)", 3))
    bwords = encrypt([w for e in blocks for w in e], mpqlite.hash_name("(block table)", 3))
    bpos = hpos + hcount * 16
    size = bpos + len(blocks) * 16
    head = b"MPQ\x1a" + struct.pack("<IIHHIIII", 32, size, 0, shift, hpos, bpos, hcount, len(blocks))
    Path(path).write_bytes(head + bytes(body) + struct.pack("<%dI" % len(hwords), *hwords) + struct.pack("<%dI" % len(bwords), *bwords))


# ---------------------------------------------------------------- made-up game files

def chunk(magic, data):
    return magic[::-1].encode() + struct.pack("<I", len(data)) + data


def zblob(names):
    blob, offs = b"", []
    for n in names:
        offs.append(len(blob))
        blob += n.encode() + b"\0"
    return blob, struct.pack("<%dI" % len(offs), *offs)


def modf(entries):
    """(name index, uid, world x, y, z, world yaw degrees, doodad set)."""
    out = b""
    for ni, uid, wx, wy, wz, yaw, dset in entries:
        pos = (dp.PLACE - wy, wz, dp.PLACE - wx)
        out += struct.pack("<II3f3f3f3fHHHH", ni, uid, *pos, 0.0, (yaw - dp.YAW_OFFSET) % 360, 0.0, *pos, *pos, 0, dset, 0, 0)
    return out


def fake_adt(m2s, wmos, doodads, wmo_places, base=50.0, mcnk=None):
    """A tile: doodads (name index, uid, world x, y, z, world yaw, scale) and buildings (modf), one height chunk."""
    mmdx, mmid = zblob(m2s)
    mwmo, mwid = zblob(wmos)
    mddf = b""
    for ni, uid, wx, wy, wz, yaw, sc in doodads:
        mddf += struct.pack("<II3f3fHH", ni, uid, dp.PLACE - wy, wz, dp.PLACE - wx, 0.0, (yaw - dp.YAW_OFFSET) % 360, 0.0, int(sc * 1024), 0)
    out = chunk("MVER", struct.pack("<I", 18)) + chunk("MHDR", b"\0" * 64) + chunk("MMDX", mmdx) + chunk("MMID", mmid)
    out += chunk("MWMO", mwmo) + chunk("MWID", mwid) + chunk("MDDF", mddf) + chunk("MODF", modf(wmo_places))
    if mcnk:
        px, py = mcnk
        head = bytearray(128)
        struct.pack_into("<3f", head, 0x68, px, py, base)
        heights = [float(i) * 0.1 for i in range(145)]
        out += chunk("MCNK", bytes(head) + chunk("MCVT", struct.pack("<145f", *heights)))
    return out


def fake_wmo(sets, doodads, names):
    """sets: (name, first, count); doodads: (name index, x, y, z, yaw degrees, scale)."""
    modn, offs = zblob(names)
    offs = struct.unpack("<%dI" % len(names), offs)
    mohd = bytearray(64)
    struct.pack_into("<6f", mohd, 36, -10.0, -8.0, 0.0, 10.0, 8.0, 12.0)
    mods = b""
    for n, f, c in sets:
        mods += n.encode().ljust(20, b"\0") + struct.pack("<III", f, c, 0)
    modd = b""
    for ni, x, y, z, yaw, sc in doodads:
        h = math.radians(yaw) / 2
        modd += struct.pack("<I3f4ffI", offs[ni], x, y, z, 0.0, 0.0, math.sin(h), math.cos(h), sc, 0)
    return chunk("MVER", struct.pack("<I", 17)) + chunk("MOHD", bytes(mohd)) + chunk("MODS", mods) + chunk("MODN", modn) + chunk("MODD", modd)


def fake_wdt(tiles, wmo_name=None, place=None):
    main = bytearray(4096 * 8)
    for x, y in tiles:
        struct.pack_into("<I", main, (y * 64 + x) * 8, 1)
    out = chunk("MVER", struct.pack("<I", 18)) + chunk("MPHD", struct.pack("<I", 1 if wmo_name else 0) + b"\0" * 28) + chunk("MAIN", bytes(main))
    if wmo_name:
        out += chunk("MWMO", wmo_name.encode() + b"\0") + chunk("MODF", modf([place]))
    return out


def fake_dbc(rows, fields):
    """rows: tuples whose str items go into the string block."""
    strings = b"\0"
    recs = b""
    for r in rows:
        vals = []
        for v in r:
            if isinstance(v, str):
                vals.append(len(strings))
                strings += v.encode() + b"\0"
            else:
                vals.append(v)
        vals += [0] * (fields - len(vals))
        recs += struct.pack("<%dI" % fields, *vals)
    return b"WDBC" + struct.pack("<4I", len(rows), fields, fields * 4, len(strings)) + recs + strings


def fake_m2(box):
    b = bytearray(0xC0)
    b[:4] = b"MD20"
    struct.pack_into("<6f", b, 0xA0, *box)
    return bytes(b)


INN = "World\\Made\\Up\\GoldshireInn.wmo"
SHED = "World\\Made\\Up\\Shed.wmo"
ARENA = "World\\Made\\Up\\TestArena.wmo"
INN_AT = (-9460.0, 20.0, 56.0, 270.0)   # world x, y, z, facing


def game_files():
    t = dp.tile_of(INN_AT[0], INN_AT[1])
    adt = fake_adt(
        ["World\\Made\\Up\\Barrel.mdx", "World\\Made\\Up\\Rock.mdx", "World\\Made\\Up\\Fence.mdx"],
        [SHED, INN],
        # a barrel 5 yards north of the inn (world +x), twice the size; a rock 30 yards away; a fence 70 yards away
        [(0, 11, INN_AT[0] + 5, INN_AT[1], INN_AT[2] + 1, 0.0, 2.0), (1, 12, INN_AT[0], INN_AT[1] + 30, INN_AT[2], 90.0, 1.0),
         (2, 13, INN_AT[0] + 70, INN_AT[1], INN_AT[2], 0.0, 1.0)],
        [(0, 21, INN_AT[0] - 40, INN_AT[1] - 40, INN_AT[2], 0.0, 0), (1, 22, INN_AT[0], INN_AT[1], INN_AT[2], INN_AT[3], 1)],
        mcnk=(INN_AT[0] + 10, INN_AT[1] + 10))
    # the next tile west has the same inn placement again (a building on two tiles is in both) and nothing else
    adt2 = fake_adt([], [INN], [], [(0, 22, INN_AT[0], INN_AT[1], INN_AT[2], INN_AT[3], 1)])
    inn = fake_wmo([("Set_$DefaultGlobal", 0, 1), ("Set_Tavern", 1, 2)],
                   [(0, 2.0, 0.0, 0.5, 0.0, 1.0), (1, 2.0, 1.5, 0.5, 90.0, 1.5), (2, -1.0, -1.0, 2.0, 0.0, 1.0)],
                   ["World\\Made\\Up\\Table.mdx", "World\\Made\\Up\\Chair.mdx", "World\\Made\\Up\\Lamp.mdx"])
    arena = fake_wmo([("Set_$DefaultGlobal", 0, 1)], [(0, 3.0, 0.0, 1.0, 0.0, 1.0)], ["World\\Made\\Up\\Banner.mdx"])
    displays = fake_dbc([(5, "World\\Made\\Up\\Table.mdx"), (6, "world/made/up/chair.m2"), (7, "World\\Made\\Up\\Barrel.mdx"),
                         (8, INN), (9, ARENA), (10, "World\\Made\\Up\\Banner.mdx"), (4, "World\\Made\\Up\\Table.m2"),
                         (11, "World\\Made\\Up\\Fence.mdx")], 19)
    maps = fake_dbc([(0, "Testland"), (617, "TestArena")], 4)
    return {
        "World\\Maps\\Testland\\Testland_%d_%d.adt" % t: adt,
        "World\\Maps\\Testland\\Testland_%d_%d.adt" % (t[0] + 1, t[1]): adt2,
        "World\\Maps\\Testland\\Testland.wdt": fake_wdt([t, (t[0] + 1, t[1])]),
        INN: inn,
        SHED: fake_wmo([], [], []),
        ARENA: arena,
        "World\\Maps\\TestArena\\TestArena.wdt": fake_wdt([], ARENA, (0, 31, 1289.0, 791.0, 10.0, 90.0, 0)),
        "World\\Made\\Up\\Lamp.m2": fake_m2((-0.5, -0.5, 0.0, 0.5, 0.5, 2.0)),
        dp.DISPLAY_TABLE: displays,
        dp.MAP_TABLE: maps,
    }


PLACES = [
    {"name": "testinn", "title": "The Test Inn", "map": 0, "box": [INN_AT[0] - 60, INN_AT[1] - 60, INN_AT[0] + 60, INN_AT[1] + 60],
     "focus": ["goldshireinn", "inn"], "radius": 24},
    {"name": "testvillage", "title": "Test Village", "map": 0, "box": [INN_AT[0] - 100, INN_AT[1] - 100, INN_AT[0] + 100, INN_AT[1] + 100],
     "focus": ["goldshireinn"]},
    {"name": "testarena", "title": "Test Arena", "map": 617, "box": [1199, 701, 1379, 881], "focus": ["testarena"]},
    {"name": "nowhere", "title": "Nowhere", "map": 99, "box": [0, 0, 10, 10]},
]


def near(a, b, eps=0.02):
    return abs(a - b) <= eps


class Formats(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.files = game_files()
        data = self.dir / "game" / "Data"
        data.mkdir(parents=True)
        tables = {k: v for k, v in self.files.items() if k.startswith("DBFiles")}
        rest = {k: v for k, v in self.files.items() if not k.startswith("DBFiles")}
        write_mpq(data / "common.MPQ", rest, sectors={INN})
        (data / "enUS").mkdir()
        write_mpq(data / "enUS" / "locale-enUS.MPQ", tables, sectors={dp.DISPLAY_TABLE})
        # the Den's own patch is never read (it would hide the table)
        write_mpq(data / "patch-T.MPQ", {dp.DISPLAY_TABLE: fake_dbc([(1, "x.mdx")], 19)})
        self.game = dp.Game.open(self.dir / "game")
        self.dens = self.dir / "dens"

    def tearDown(self):
        self.tmp.cleanup()

    def test_our_archives_open_and_the_den_patch_is_left_out(self):
        order = [p.name for p in dp.archive_order(self.dir / "game")]
        self.assertEqual(order, ["common.MPQ", "locale-enUS.MPQ"])
        self.assertEqual(self.game.read(INN), self.files[INN], "zlib sectors")
        self.assertEqual(self.game.read(dp.DISPLAY_TABLE), self.files[dp.DISPLAY_TABLE])
        self.assertIsNone(self.game.read("World\\Nothing.wmo"))

    def test_the_clients_order(self):
        d = self.dir / "order" / "Data"
        (d / "deDE").mkdir(parents=True)
        for n in ["patch-2.MPQ", "patch.MPQ", "common.MPQ", "lichking.MPQ", "patch-A.MPQ", "patch-T.MPQ", "expansion.MPQ", "Patch-9.mpq"]:
            (d / n).write_bytes(b"")
        for n in ["patch-deDE.MPQ", "locale-deDE.MPQ", "patch-deDE-2.MPQ", "patch-deDE-T.MPQ"]:
            (d / "deDE" / n).write_bytes(b"")
        self.assertEqual([p.name for p in dp.archive_order(self.dir / "order")],
                         ["common.MPQ", "expansion.MPQ", "lichking.MPQ", "locale-deDE.MPQ", "patch.MPQ", "patch-2.MPQ", "Patch-9.mpq",
                          "patch-A.MPQ", "patch-deDE.MPQ", "patch-deDE-2.MPQ"])

    def test_the_readers(self):
        adt = dp.read_adt(self.files["World\\Maps\\Testland\\Testland_%d_%d.adt" % dp.tile_of(INN_AT[0], INN_AT[1])])
        self.assertEqual([d["model"] for d in adt["doodads"]], ["World\\Made\\Up\\Barrel.mdx", "World\\Made\\Up\\Rock.mdx", "World\\Made\\Up\\Fence.mdx"])
        self.assertEqual(adt["wmos"][1]["model"], INN)
        self.assertEqual(adt["wmos"][1]["set"], 1)
        x, y, z = dp.world(adt["wmos"][1]["pos"])
        self.assertTrue(near(x, INN_AT[0]) and near(y, INN_AT[1]) and near(z, INN_AT[2]))
        self.assertEqual(adt["doodads"][0]["scale"], 2.0)
        w = dp.read_wmo(self.files[INN])
        self.assertEqual(w["sets"], [("Set_$DefaultGlobal", 0, 1), ("Set_Tavern", 1, 2)])
        self.assertEqual(w["doodads"][1]["model"], "World\\Made\\Up\\Chair.mdx")
        self.assertTrue(near(math.degrees(dp.quat_yaw(w["doodads"][1]["rot"])), 90.0))
        self.assertEqual(w["box"], (-10.0, -8.0, 0.0, 10.0, 8.0, 12.0))
        wdt = dp.read_wdt(self.files["World\\Maps\\TestArena\\TestArena.wdt"])
        self.assertEqual((wdt["tiles"], wdt["wmos"][0]["model"]), (set(), ARENA))
        ix, top = dp.display_index(self.files[dp.DISPLAY_TABLE])
        self.assertEqual((ix["world\\made\\up\\table"], ix["world\\made\\up\\chair"], top), (4, 6, 11), "the lowest id; .m2 and .mdx are one model")
        self.assertEqual(dp.map_dirs(self.files[dp.MAP_TABLE]), {0: "Testland", 617: "TestArena"})
        self.assertEqual(dp.m2_box(self.files["World\\Made\\Up\\Lamp.m2"]), (-0.5, -0.5, 0.0, 0.5, 0.5, 2.0))
        for bad, fn in [(b"nope", dp.read_adt), (b"", dp.read_wmo), (b"WDBC", dp.read_dbc), (b"x" * 10, dp.read_wdt)]:
            with self.assertRaises(dp.PlaceError):
                fn(bad)

    def test_tiles_and_ground(self):
        self.assertEqual(dp.tile_of(-9464.0, 62.0), (31, 49), "Goldshire's tile")
        self.assertEqual(dp.tiles_for([-10, -10, 10, 10]), [(31, 31), (31, 32), (32, 31), (32, 32)])
        adt = dp.read_adt(self.files["World\\Maps\\Testland\\Testland_%d_%d.adt" % dp.tile_of(INN_AT[0], INN_AT[1])])
        px, py = INN_AT[0] + 10, INN_AT[1] + 10
        self.assertTrue(near(dp.ground_at(adt["heights"], px, py), 50.0), "the chunk's corner: its base")
        self.assertTrue(near(dp.ground_at(adt["heights"], px - dp.UNIT, py - 2 * dp.UNIT), 50.0 + (17 + 2) * 0.1))
        self.assertIsNone(dp.ground_at(adt["heights"], 0.0, 0.0))

    def test_the_inn_one_to_one(self):
        rep = dp.run(self.game, self.dens, ["testinn"], places=PLACES)[0]
        self.assertTrue(rep["ok"], rep)
        doc = json.loads((self.dens / "testinn.den.json").read_text())
        props = doc["props"]
        by = {}
        for p in props:
            by.setdefault(p["entry"] - dp.MODEL_BASE, []).append(p)
        # the inn (its own building, the middle), the table (set 0), the chair (set 1), the barrel; the lamp has no
        # display; the rock is 30 yards out (past the radius), the shed and the fence too
        self.assertEqual(sorted(by), [4, 6, 7, 8], "the table: the lowest display of its model")
        inn = by[8][0]
        self.assertEqual((inn["x"], inn["y"], inn["z"], inn["yaw"], inn.get("role")), (0, 0, 0, 270.0, "house"))
        # the building faces 270 degrees: its front (its x) is world -y, its left (its y) world +x
        t = by[4][0]
        self.assertTrue(near(t["x"], 0.0) and near(t["y"], -2.0) and near(t["z"], 0.5) and near(t["yaw"], 270.0), t)
        c = by[6][0]
        self.assertTrue(near(c["x"], 1.5) and near(c["y"], -2.0) and near(c["yaw"] % 360, 0.0) and c["scale"] == 1.5, c)
        b = by[7][0]
        self.assertTrue(near(b["x"], 5.0) and near(b["y"], 0.0) and near(b["z"], 1.0) and b["scale"] == 2.0 and near(b["yaw"], 0.0), b)
        self.assertEqual(len(props), 4, "the inn on two tiles is there once")
        self.assertEqual(doc["skipped"], [{"file": "Lamp.mdx", "count": 1}])
        self.assertIn("1 models with no display left out", doc["line"])
        self.assertEqual((doc["format"], doc["source"], doc["local"], doc["room"]["size"]), ("leyline-den", "den-places", True, 64.0))
        # it fits a stamp: Jaina builds it (.den copy testinn)
        self.assertTrue(rep["stamp"])
        st = json.loads((self.dens / "stamps-local" / "testinn.den.json").read_text())
        self.assertEqual((st["kind"], len(st["props"]), "room" in st), ("stamp", 4, False))

    def test_a_village_is_a_den_not_a_stamp(self):
        rep = dp.run(self.game, self.dens, ["testvillage"], places=PLACES)[0]
        self.assertTrue(rep["ok"], rep)
        self.assertFalse(rep["stamp"], "the fence 70 yards out: more than a stamp's 40")
        self.assertFalse((self.dens / "stamps-local" / "testvillage.den.json").exists())
        doc = json.loads((self.dens / "testvillage.den.json").read_text())
        self.assertGreaterEqual(doc["room"]["size"], 70.0, "the Den opens big enough to hold it")
        self.assertIn("Rock.mdx", [s["file"] for s in doc["skipped"]])
        self.assertIn("Shed.wmo", [s["file"] for s in doc["skipped"]], "a building with no display: left out, counted")

    def test_rows_of_our_own_for_models_without_a_display(self):
        rep = dp.run(self.game, self.dens, ["testvillage"], add_models=True, places=PLACES)[0]
        self.assertTrue(rep["ok"], rep)
        rows = json.loads((self.dens / dp.ROWS_FILE).read_text())
        self.assertEqual(rows["format"], dp.ROWS_FORMAT)
        got = {r["model"]: r for r in rows["models"]}
        for m in ["World\\Made\\Up\\Rock.mdx", "World\\Made\\Up\\Lamp.mdx", SHED]:
            self.assertIn(m, got)
            self.assertTrue(dp.ROW_BASE <= got[m]["display"] <= dp.ROW_LAST)
        self.assertEqual(got["World\\Made\\Up\\Lamp.mdx"]["box"], [-0.5, -0.5, 0.0, 0.5, 0.5, 2.0], "the M2's own box (.m2 on disk)")
        self.assertEqual(got[SHED]["box"], [-10.0, -8.0, 0.0, 10.0, 8.0, 12.0], "the building's own box")
        doc = json.loads((self.dens / "testvillage.den.json").read_text())
        self.assertEqual(doc["skipped"], [])
        entries = {p["entry"] - dp.MODEL_BASE for p in doc["props"]}
        self.assertTrue({got[m]["display"] for m in got} <= entries)
        # again: the same numbers
        before = (self.dens / dp.ROWS_FILE).read_text()
        dp.run(self.game, self.dens, ["testvillage", "testinn"], add_models=True, places=PLACES)
        self.assertEqual((self.dens / dp.ROWS_FILE).read_text(), before)

    def test_an_arena_that_is_one_building(self):
        rep = dp.run(self.game, self.dens, ["testarena"], places=PLACES)[0]
        self.assertTrue(rep["ok"], rep)
        doc = json.loads((self.dens / "testarena.den.json").read_text())
        e = [(p["entry"] - dp.MODEL_BASE, p["x"], p["y"]) for p in doc["props"]]
        self.assertEqual(e[0], (9, 0, 0), "the arena's building, the middle")
        self.assertEqual(e[1][0], 10)
        self.assertTrue(near(e[1][1], 0.0) and near(e[1][2], 3.0), "its banner, 3 yards before its front (facing 90: world +y)")

    def test_what_cant_be_made_says_why_and_a_players_own_file_is_never_written_over(self):
        (self.dens).mkdir(parents=True)
        mine = self.dens / "testinn.den.json"
        mine.write_text('{"format": "leyline-den", "version": 1, "props": []}')
        reps = {r["name"]: r for r in dp.run(self.game, self.dens, ["testinn", "nowhere", "atlantis"], places=PLACES)}
        self.assertEqual(mine.read_text(), '{"format": "leyline-den", "version": 1, "props": []}', "the player's own Den kept")
        self.assertTrue(reps["testinn"]["ok"] and all("testinn.den.json" not in f or "stamps-local" in f for f in reps["testinn"]["files"]))
        self.assertIn("Your game has no map 99", reps["nowhere"]["error"])
        self.assertIn("No place called 'atlantis'", reps["atlantis"]["error"])

    def test_the_command_line(self):
        out = subprocess.run([sys.executable, "-I", str(HERE / "den_places.py"), "--list"], capture_output=True, text=True)
        self.assertEqual(out.returncode, 0)
        self.assertIn("lionspride", out.stdout)
        self.assertIn("nagrandarena", out.stdout)
        out = subprocess.run([sys.executable, "-I", str(HERE / "den_places.py")], capture_output=True, text=True)
        self.assertEqual(out.returncode, 2)

    def test_nothing_players_never_see(self):
        dp.run(self.game, self.dens, ["testinn", "testvillage", "testarena"], add_models=True, places=PLACES)
        for f in self.dens.rglob("*.json"):
            t = f.read_text().lower()
            for w in ["warcraft", "blizzard", "lich king", "world of"]:
                self.assertNotIn(w, t, f)


class Lists(unittest.TestCase):
    def test_the_place_list(self):
        places = dp.load_places()
        names = [p["name"] for p in places]
        self.assertEqual(len(names), len(set(names)))
        for want in ["lionspride", "goldshire", "nagrandarena", "bladesedgearena", "ruinsoflordaeron", "dalaransewers", "ringofvalor"]:
            self.assertIn(want, names)
        tele = {p["name"]: p for p in gen_teleports.load()}
        for p in places:
            self.assertIsNone(dp.check_place(p), p["name"])
            b = p["box"]
            self.assertTrue(all(abs(v) < 17066 for v in b))
            # each place's box holds where .go takes you there (the same list's numbers), when .go knows it
            t = tele.get(p["name"])
            if t and p["name"] not in ("goldshire", "lionspride"):
                self.assertEqual(t["map"], p["map"], p["name"])
                self.assertTrue(dp.inside(b, t["x"], t["y"]), p["name"])
            for w in ["warcraft", "blizzard", "lich king"]:
                self.assertNotIn(w, p["title"].lower())
        g = tele["goldshire"]
        self.assertTrue(dp.inside(dp.find_place(places, "goldshire")["box"], g["x"], g["y"]))
        self.assertTrue(dp.inside(dp.find_place(places, "lionspride")["box"], g["x"], g["y"]))
        self.assertEqual(dp.find_place(places, "inn")["name"], "lionspride")
        self.assertIn("bigger than a Den", dp.check_place({"name": "x", "title": "X", "map": 0, "box": [0, 0, 600, 10]}))
        self.assertIsNotNone(dp.check_place({"name": "X Y", "title": "X", "map": 0, "box": [0, 0, 1, 1]}))

    def test_the_teleports_list_is_current_and_sane(self):
        places = gen_teleports.load()
        self.assertEqual(gen_teleports.check(places), [])
        self.assertGreater(len(places), 200)
        out = subprocess.run([sys.executable, "-I", str(HERE / "gen_teleports.py"), "--check"], capture_output=True, text=True)
        self.assertEqual(out.returncode, 0, out.stdout)
        bad = [dict(places[0], name="Bad Name"), dict(places[1], title="The Lich King's"), dict(places[2], x=99999), dict(places[3], cat="moon")]
        self.assertEqual(len(gen_teleports.check(bad)), 4)
        self.assertEqual(gen_teleports.check([places[0], places[0]]), ["den: twice"])


if __name__ == "__main__":
    unittest.main(verbosity=1)
