#!/usr/bin/env python3
"""Vanilla Remastered: build the 1.12 browser slice (levels 1-20) from the owner's own client archives.

Runs on the server inside the vr-mpq-tools image (StormLib + python, hosting/vanilla-remastered/mpq-tools.Dockerfile),
called by vr-data.sh. It reads the uploaded 1.12 archives (+ our own patch-Z.MPQ, the Classic+ content), picks the
files the slice-112.json tiers need, and writes them into NEW archives with the same names (dbc.MPQ, model.MPQ, ...,
patch.MPQ, patch-2.MPQ, patch-Z.MPQ), each file in the archive it came from, so the old 1.12 site's own /data
(which reads a folder of archives, CLIENT_DATA) serves exactly the slice and nothing else.

    python3 -I vr-slice.py plan  --data DIR --slice slice-112.json [--spawns TSV] --out DIR [--dry-run]
                                 [--free-mb N]
    python3 -I vr-slice.py spawn-sql --ct COLS --gt COLS --c COLS --g COLS
                                 the SQL for the realm's spawns (COLS: the tables' column names, comma separated)

What a tier pulls in (the way the OG stream and the Wrath slice were made: whole map tiles plus what they name):
  base        the prefixes (UI, fonts, characters, items, spells, the shared sounds), then every model and texture
              those files name
  zones       WorldMapArea.dbc's rectangle of each zone -> the ADT tiles (+ marginTiles), their minimap tiles, every
              texture, model (.mdx -> .m2) and building (WMO root, its groups, textures, doodads) they name
  instances   the whole map: its WDT's building, and its tiles if it has any
  spawns      the realm's creatures and objects inside the tier's tiles: their model folders (skins), baked NPC
              textures and creature sounds
  prefixes    plain folders (music, ambience)

Standard library only (+ StormLib through ctypes for the archives). Prints a size report; never prints file contents.
Last line: "VR SLICE OK <MB>" / "VR SLICE DRY <MB>", or an error (exit 1).
"""
from __future__ import annotations

import argparse
import ctypes as C
import json
import math
import os
import re
import shutil
import struct
import sys

TILE = 533.33333
# The client's archive order, highest priority first (what overrides what). Our own patch-Z is on top.
BASE_ARCHIVES = ["wmo.MPQ", "texture.MPQ", "terrain.MPQ", "speech.MPQ", "sound.MPQ", "model.MPQ", "misc.MPQ",
                 "interface.MPQ", "fonts.MPQ", "dbc.MPQ"]


def priority(name: str) -> tuple:
    """Sort key: patch-Z ... patch-A, patch-9 ... patch-2, patch, then the base archives."""
    low = name.lower()
    m = re.fullmatch(r"patch-([0-9a-z])\.mpq", low)
    if m:
        c = m.group(1)
        return (0, -(ord(c) + (100 if c.isalpha() else 0)))
    if low == "patch.mpq":
        return (1, 0)
    order = [b.lower() for b in BASE_ARCHIVES]
    return (2, order.index(low) if low in order else len(order))


def key(name: str) -> str:
    """A game path as the archives' hash sees it: backslashes, any case. .mdx/.mdl names mean the .m2."""
    k = name.replace("/", "\\").strip().lower()
    if k.endswith(".mdx") or k.endswith(".mdl"):
        k = k[:-4] + ".m2"
    return k


# ---------------------------------------------------------------------------------------------- formats
def chunks(data: bytes):
    """Top-level IFF chunks (ADT, WDT, WMO): (id as read in the file, payload). Ids are stored reversed."""
    pos, n = 0, len(data)
    while pos + 8 <= n:
        cid = data[pos:pos + 4][::-1].decode("latin-1")
        (size,) = struct.unpack_from("<I", data, pos + 4)
        body = data[pos + 8:pos + 8 + size]
        yield cid, body
        pos += 8 + size


def strings(block: bytes) -> list[str]:
    return [s.decode("latin-1") for s in block.split(b"\x00") if s.strip()]


def adt_deps(data: bytes) -> list[str]:
    out = []
    for cid, body in chunks(data):
        if cid in ("MTEX", "MMDX", "MWMO"):
            out += strings(body)
    return out


def wdt_deps(data: bytes) -> list[str]:
    out = []
    for cid, body in chunks(data):
        if cid == "MWMO":
            out += strings(body)
    return out


GROUP_RE = re.compile(r"_\d{3}\.wmo$", re.I)


def wmo_deps(name: str, data: bytes) -> list[str]:
    """A WMO root: its textures, doodads, skybox, and its group files."""
    out, groups = [], 0
    for cid, body in chunks(data):
        if cid == "MOHD" and len(body) >= 8:
            groups = struct.unpack_from("<I", body, 4)[0]
        elif cid in ("MOTX", "MODN", "MOSB"):
            out += strings(body)
    stem = name[:-4]
    out += [f"{stem}_{i:03d}.wmo" for i in range(min(groups, 512))]
    return out


def m2_deps(data: bytes) -> list[str]:
    """A 1.12 (version 256) model's hard-coded textures (type 0)."""
    if len(data) < 0x64 or data[:4] != b"MD20":
        return []
    n, ofs = struct.unpack_from("<II", data, 0x5C)
    out = []
    if n > 512 or ofs + n * 16 > len(data):
        return out
    for i in range(n):
        ttype, _flags, ln, lo = struct.unpack_from("<IIII", data, ofs + i * 16)
        if ttype == 0 and 1 < ln < 260 and lo + ln <= len(data):
            s = data[lo:lo + ln].split(b"\x00")[0].decode("latin-1")
            if s:
                out.append(s)
    return out


class Dbc:
    """A WDBC table: rows of 32-bit fields and a string block."""

    def __init__(self, data: bytes):
        if data[:4] != b"WDBC":
            raise ValueError("not a WDBC table")
        self.n, self.fields, self.size, self.slen = struct.unpack_from("<IIII", data, 4)
        self.rows = data[20:20 + self.n * self.size]
        self.sblock = data[20 + self.n * self.size:20 + self.n * self.size + self.slen]

    def u32(self, r: int, f: int) -> int:
        return struct.unpack_from("<I", self.rows, r * self.size + f * 4)[0]

    def f32(self, r: int, f: int) -> float:
        return struct.unpack_from("<f", self.rows, r * self.size + f * 4)[0]

    def s(self, r: int, f: int) -> str:
        o = self.u32(r, f)
        if o >= len(self.sblock):
            return ""
        return self.sblock[o:self.sblock.index(b"\x00", o) if b"\x00" in self.sblock[o:] else len(self.sblock)].decode("latin-1")

    def strings_ending(self, r: int, ext: str) -> list[str]:
        """Every field of the row that points at a string ending in ext (robust to field layout)."""
        out = []
        for f in range(self.fields):
            o = self.u32(r, f)
            if 0 < o < len(self.sblock) and self.sblock[o - 1] == 0:
                v = self.s(r, f)
                if v.lower().endswith(ext):
                    out.append(v)
        return out

    def by_id(self) -> dict[int, int]:
        return {self.u32(r, 0): r for r in range(self.n)}


def tile_of(x: float, y: float) -> tuple[int, int]:
    """World position -> (a, b) of the ADT file Map_a_b.adt."""
    return int(math.floor(32 - y / TILE)), int(math.floor(32 - x / TILE))


def zone_tiles(left: float, right: float, top: float, bottom: float, margin: int = 0) -> set[tuple[int, int]]:
    """WorldMapArea's rectangle (left/right are y, top/bottom are x) -> ADT tiles."""
    a0, a1 = sorted((int(math.floor(32 - left / TILE)), int(math.floor(32 - right / TILE))))
    b0, b1 = sorted((int(math.floor(32 - top / TILE)), int(math.floor(32 - bottom / TILE))))
    return {(a, b) for a in range(max(0, a0 - margin), min(63, a1 + margin) + 1)
            for b in range(max(0, b0 - margin), min(63, b1 + margin) + 1)}


def parse_trs(text: str) -> dict[str, str]:
    """md5translate.trs -> {"azeroth\\map32_48.blp": "textures\\minimap\\<hash>.blp"}."""
    out = {}
    for line in text.splitlines():
        if "\t" not in line or line.startswith("dir:"):
            continue
        a, b = line.split("\t", 1)
        out[a.strip().replace("/", "\\").lower()] = "textures\\minimap\\" + b.strip().lower()
    return out


# ---------------------------------------------------------------------------------------------- archives
class Storm:
    """StormLib through ctypes (libstorm.so in the tools image)."""
    MPQ_FILE_COMPRESS = 0x00000200
    MPQ_FILE_REPLACEEXISTING = 0x80000000
    MPQ_COMPRESSION_ZLIB = 0x02
    MPQ_CREATE_LISTFILE = 0x00100000
    MPQ_CREATE_ATTRIBUTES = 0x00200000
    STREAM_FLAG_READ_ONLY = 0x00000100

    def __init__(self, path=None):
        lib = C.CDLL(path or os.environ.get("STORMLIB", "libstorm.so"))
        H = C.c_void_p
        lib.SFileOpenArchive.argtypes = [C.c_char_p, C.c_uint, C.c_uint, C.POINTER(H)]
        lib.SFileCloseArchive.argtypes = [H]
        lib.SFileOpenFileEx.argtypes = [H, C.c_char_p, C.c_uint, C.POINTER(H)]
        lib.SFileReadFile.argtypes = [H, C.c_void_p, C.c_uint, C.POINTER(C.c_uint), C.c_void_p]
        lib.SFileGetFileSize.argtypes = [H, C.POINTER(C.c_uint)]
        lib.SFileGetFileSize.restype = C.c_uint
        lib.SFileCloseFile.argtypes = [H]
        lib.SFileCreateArchive.argtypes = [C.c_char_p, C.c_uint, C.c_uint, C.POINTER(H)]
        lib.SFileCreateFile.argtypes = [H, C.c_char_p, C.c_uint64, C.c_uint, C.c_uint, C.c_uint, C.POINTER(H)]
        lib.SFileWriteFile.argtypes = [H, C.c_void_p, C.c_uint, C.c_uint]
        lib.SFileFinishFile.argtypes = [H]
        self.lib, self.H = lib, H


class Archive:
    def __init__(self, storm: Storm, path: str):
        self.s, self.path, self.name = storm, path, os.path.basename(path)
        self.h = storm.H()
        if not storm.lib.SFileOpenArchive(path.encode(), 0, Storm.STREAM_FLAG_READ_ONLY, C.byref(self.h)):
            raise OSError(f"cannot open {path}")

    def read(self, name: str) -> bytes | None:
        f = self.s.H()
        if not self.s.lib.SFileOpenFileEx(self.h, name.encode("latin-1"), 0, C.byref(f)):
            return None
        try:
            size = self.s.lib.SFileGetFileSize(f, None)
            if size == 0xFFFFFFFF:
                return None
            buf = C.create_string_buffer(max(size, 1))
            got = C.c_uint()
            self.s.lib.SFileReadFile(f, buf, size, C.byref(got), None)
            return buf.raw[:got.value]
        finally:
            self.s.lib.SFileCloseFile(f)

    def size(self, name: str) -> int:
        f = self.s.H()
        if not self.s.lib.SFileOpenFileEx(self.h, name.encode("latin-1"), 0, C.byref(f)):
            return 0
        try:
            v = self.s.lib.SFileGetFileSize(f, None)
            return 0 if v == 0xFFFFFFFF else v
        finally:
            self.s.lib.SFileCloseFile(f)

    def names(self) -> list[str]:
        lst = self.read("(listfile)") or b""
        return [n.strip() for n in lst.decode("latin-1").splitlines() if n.strip()]


class Chain:
    """The archives in priority order; a name resolves to the highest archive that holds it."""

    def __init__(self, archives):
        self.archives = sorted(archives, key=lambda a: priority(a.name))
        self.where: dict[str, tuple] = {}
        for a in reversed(self.archives):          # lowest first, so higher ones overwrite
            for n in a.names():
                if n.startswith("("):
                    continue
                self.where[key(n)] = (a, n)

    def has(self, k: str) -> bool:
        return k in self.where

    def read(self, k: str) -> bytes | None:
        hit = self.where.get(k)
        return hit[0].read(hit[1]) if hit else None

    def size(self, k: str) -> int:
        hit = self.where.get(k)
        return hit[0].size(hit[1]) if hit else 0

    def keys(self):
        return self.where.keys()


# ---------------------------------------------------------------------------------------------- the plan
def prefix_match(k: str, prefixes: list[str]) -> bool:
    for p in prefixes:
        pl = p.lower()
        if pl.endswith("\\*"):
            d = pl[:-1]
            if k.startswith(d) and "\\" not in k[len(d):]:
                return True
        elif k.startswith(pl) if pl.endswith("\\") else k == pl:
            return True
    return False


class Planner:
    def __init__(self, chain, spec: dict, spawns: list[tuple] | None = None, log=print):
        self.chain, self.spec, self.spawns, self.log = chain, spec, spawns or [], log
        self.taken: set[str] = set()
        self.missing: set[str] = set()
        self._dbc: dict[str, Dbc | None] = {}
        self._trs = None

    def dbc(self, name: str) -> Dbc | None:
        if name not in self._dbc:
            data = self.chain.read(key("DBFilesClient\\" + name))
            try:
                self._dbc[name] = Dbc(data) if data else None
            except ValueError:
                self._dbc[name] = None
        return self._dbc[name]

    def trs(self) -> dict:
        if self._trs is None:
            data = self.chain.read(key("Textures\\Minimap\\md5translate.trs"))
            self._trs = parse_trs(data.decode("latin-1")) if data else {}
        return self._trs

    def map_dir(self, map_id: int) -> str | None:
        t = self.dbc("Map.dbc")
        if not t:
            return None
        r = t.by_id().get(map_id)
        return t.s(r, 1) if r is not None else None

    def zone_rect(self, area_name: str):
        t = self.dbc("WorldMapArea.dbc")
        if not t:
            return None
        for r in range(t.n):
            for f in range(1, min(t.fields, 5)):
                if t.s(r, f).lower() == area_name.lower():
                    return t.u32(r, 1), [t.f32(r, i) for i in range(f + 1, f + 5)]
        return None

    def walk(self, start) -> set[str]:
        """Add names (and everything they name) not taken yet; returns what this call added."""
        added, work = set(), [key(n) for n in start]
        while work:
            k = work.pop()
            if k in self.taken or k in added:
                continue
            if not self.chain.has(k):
                self.missing.add(k)
                continue
            added.add(k)
            deps = []
            if k.endswith(".m2"):
                deps = m2_deps(self.chain.read(k) or b"")
            elif k.endswith(".wmo") and not GROUP_RE.search(k):
                deps = wmo_deps(k, self.chain.read(k) or b"")
            elif k.endswith(".adt"):
                deps = adt_deps(self.chain.read(k) or b"")
            elif k.endswith(".wdt"):
                deps = wdt_deps(self.chain.read(k) or b"")
            work += [key(d) for d in deps]
        return added

    def folder(self, d: str) -> list[str]:
        d = key(d)
        if not d.endswith("\\"):
            d += "\\"
        return [k for k in self.chain.keys() if k.startswith(d) and "\\" not in k[len(d):]]

    def creature_names(self, display_ids) -> list[str]:
        cdi, cmd, extra = self.dbc("CreatureDisplayInfo.dbc"), self.dbc("CreatureModelData.dbc"), self.dbc("CreatureDisplayInfoExtra.dbc")
        if not cdi or not cmd:
            return []
        cdi_ids, cmd_ids = cdi.by_id(), cmd.by_id()
        ex_ids = extra.by_id() if extra else {}
        out = []
        for did in display_ids:
            r = cdi_ids.get(did)
            if r is None:
                continue
            mr = cmd_ids.get(cdi.u32(r, 1))
            if mr is not None:
                path = cmd.s(mr, 2)
                if path:
                    d = key(path).rsplit("\\", 1)[0]
                    out += self.folder(d)
                    out.append(path)
                    base = d.rsplit("\\", 1)[-1]
                    out += [k for k in self.chain.keys() if k.startswith("sound\\creature\\" + base + "\\")]
            er = ex_ids.get(cdi.u32(r, 3)) if extra and cdi.fields > 3 else None
            if er is not None:
                for s in extra.strings_ending(er, ".blp"):
                    out.append(s if "\\" in s else "Textures\\BakedNpcTextures\\" + s)
        return out

    def object_names(self, display_ids) -> list[str]:
        t = self.dbc("GameObjectDisplayInfo.dbc")
        if not t:
            return []
        ids = t.by_id()
        return [t.s(ids[d], 1) for d in display_ids if d in ids and t.s(ids[d], 1)]

    def tiles_for(self, tier) -> tuple[int | None, str | None, set]:
        m = tier.get("map")
        if m is None:
            return None, None, set()
        mdir = self.map_dir(m)
        tiles = set()
        for z in tier.get("zones", []):
            got = self.zone_rect(z)
            if not got:
                self.log(f"  !! zone {z}: not in WorldMapArea.dbc (left out)")
                continue
            zm, (left, right, top, bottom) = got
            if zm != m:
                self.log(f"  !! zone {z} is on map {zm}, not {m} (left out)")
                continue
            t = zone_tiles(left, right, top, bottom, int(self.spec.get("marginTiles", 1)))
            self.log(f"  zone {z}: {len(t)} tiles")
            tiles |= t
        return m, mdir, tiles

    def tier_names(self, tier) -> list[str]:
        names = []
        names += [k for k in self.chain.keys() if prefix_match(k, tier.get("prefixes", []))]
        for d in tier.get("creatureFolders", []):
            names += self.folder(d)
        m, mdir, tiles = self.tiles_for(tier)
        if mdir:
            names += [f"World\\Maps\\{mdir}\\{mdir}.wdt", f"World\\Maps\\{mdir}\\{mdir}.wdl"]
            trs = self.trs()
            for a, b in sorted(tiles):
                names.append(f"World\\Maps\\{mdir}\\{mdir}_{a}_{b}.adt")
                h = trs.get(f"{mdir.lower()}\\map{a:02d}_{b:02d}.blp")
                if h:
                    names.append(h)
            cre = {d for (kind, sm, x, y, d) in self.spawns if kind == "c" and sm == m and tile_of(x, y) in tiles}
            obj = {d for (kind, sm, x, y, d) in self.spawns if kind == "g" and sm == m and tile_of(x, y) in tiles}
            names += self.creature_names(sorted(cre)) + self.object_names(sorted(obj))
        for inst in tier.get("instances", []):
            idir = self.map_dir(inst)
            if not idir:
                self.log(f"  !! map {inst}: not in Map.dbc (left out)")
                continue
            names += [f"World\\Maps\\{idir}\\{idir}.wdt", f"World\\Maps\\{idir}\\{idir}.wdl"]
            names += [k for k in self.chain.keys() if k.startswith(key(f"World\\Maps\\{idir}\\") ) and k.endswith(".adt")]
            cre = {d for (kind, sm, x, y, d) in self.spawns if kind == "c" and sm == inst}
            obj = {d for (kind, sm, x, y, d) in self.spawns if kind == "g" and sm == inst}
            names += self.creature_names(sorted(cre)) + self.object_names(sorted(obj))
        return names

    def plan(self, free_mb: int | None = None):
        budget = int(self.spec.get("budgetMB", 2800)) * 1048576
        if free_mb is not None:
            budget = min(budget, max(0, (free_mb - int(self.spec.get("minFreeMB", 3000)))) * 1048576)
        tiers = [dict(self.spec.get("base", {}), key="base", name="base")] + list(self.spec.get("tiers", []))
        total, kept, report, stopped = 0, [], [], None
        # the transports and the always-there objects (ships, zeppelins: object type 15) belong to the base
        transports = {d for (kind, _m, _x, _y, d) in self.spawns if kind == "t"}
        for i, tier in enumerate(tiers):
            names = self.tier_names(tier)
            if i == 0:
                names += self.object_names(sorted(transports))
            before = set(self.taken)
            added = self.walk(names)
            size = sum(self.chain.size(k) for k in added)
            if stopped is None and total + size > budget and i > 0:
                stopped = tier["key"]
            if stopped is not None:
                report.append((tier["key"], tier.get("name", ""), len(added), size, False))
                self.taken = before
                continue
            self.taken |= added
            total += size
            kept.append(tier["key"])
            report.append((tier["key"], tier.get("name", ""), len(added), size, True))
        return total, kept, report, stopped

    def groups(self) -> dict[str, list[tuple[str, str]]]:
        """{archive name: [(key, name as listed)]} for what was taken."""
        out: dict[str, list] = {a.name: [] for a in self.chain.archives}
        for k in sorted(self.taken):
            a, n = self.chain.where[k]
            out[a.name].append((k, n))
        return out


def write_archives(storm: Storm, chain: Chain, groups, out_dir: str, log=print, whole=()) -> int:
    """Write each group into a new archive of the same name (empty ones too: some readers expect every name).
    whole: archives copied as they are (our own patch-Z: Classic+ content, never cut)."""
    tmp = out_dir + ".new"
    shutil.rmtree(tmp, ignore_errors=True)
    os.makedirs(tmp)
    total = 0
    wl = {w.lower() for w in whole}
    for a in chain.archives:
        if a.name.lower() in wl:
            shutil.copyfile(a.path, os.path.join(tmp, a.name))
            sz = os.path.getsize(os.path.join(tmp, a.name))
            total += sz
            log(f"  {a.name:<14} (ours, whole) {sz / 1048576:8.1f} MB")
    for arc, items in groups.items():
        if arc.lower() in wl:
            continue
        path = os.path.join(tmp, arc)
        h = storm.H()
        cap = max(64, 1 << math.ceil(math.log2(len(items) * 2 + 16)))
        if not storm.lib.SFileCreateArchive(path.encode(), Storm.MPQ_CREATE_LISTFILE | Storm.MPQ_CREATE_ATTRIBUTES, cap, C.byref(h)):
            raise OSError(f"cannot create {path}")
        src = next(a for a in chain.archives if a.name == arc)
        try:
            for _k, n in items:
                data = src.read(n)
                if data is None:
                    raise OSError(f"cannot read {n} from {arc}")
                f = storm.H()
                if not storm.lib.SFileCreateFile(h, n.encode("latin-1"), 0, len(data), 0,
                                                 Storm.MPQ_FILE_COMPRESS | Storm.MPQ_FILE_REPLACEEXISTING, C.byref(f)):
                    raise OSError(f"cannot add {n} to {arc}")
                if data and not storm.lib.SFileWriteFile(f, data, len(data), Storm.MPQ_COMPRESSION_ZLIB):
                    raise OSError(f"cannot write {n} to {arc}")
                storm.lib.SFileFinishFile(f)
        finally:
            storm.lib.SFileCloseArchive(h)
        sz = os.path.getsize(path)
        total += sz
        log(f"  {arc:<14} {len(items):>6} files  {sz / 1048576:8.1f} MB")
    old = out_dir + ".old"
    shutil.rmtree(old, ignore_errors=True)
    if os.path.isdir(out_dir):
        os.rename(out_dir, old)
    os.rename(tmp, out_dir)
    shutil.rmtree(old, ignore_errors=True)
    return total


def read_spawns(path: str | None) -> list[tuple]:
    """TSV lines: kind(c|g|t) map x y displayId (from vr-data.sh's spawn queries)."""
    out = []
    if not path or not os.path.exists(path):
        return out
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            p = line.rstrip("\n").split("\t")
            if len(p) < 5:
                continue
            try:
                out.append((p[0], int(p[1]), float(p[2]), float(p[3]), int(p[4])))
            except ValueError:
                continue
    return out


def spawn_sql(ct: list[str], gt: list[str], c: list[str], g: list[str]) -> str:
    """The spawn queries for this database's column names (CMaNGOS renamed a few over the years)."""
    def pick(cols, *cands):
        low = {x.lower(): x for x in cols}
        for cand in cands:
            if cand.lower() in low:
                return low[cand.lower()]
        return None
    ent = pick(ct, "Entry", "entry")
    disp = [x for x in ct if re.fullmatch(r"(?i)(DisplayId|ModelId)[1-4]", x)]
    cid, cmap, cx, cy = pick(c, "id"), pick(c, "map"), pick(c, "position_x"), pick(c, "position_y")
    gent, gdisp, gtype = pick(gt, "entry"), pick(gt, "displayId", "displayid"), pick(gt, "type")
    gid, gmap, gx, gy = pick(g, "id"), pick(g, "map"), pick(g, "position_x"), pick(g, "position_y")
    q = []
    if ent and disp and cid and cmap and cx and cy:
        for d in disp:
            q.append(f"SELECT 'c', c.`{cmap}`, c.`{cx}`, c.`{cy}`, t.`{d}` FROM creature c JOIN creature_template t "
                     f"ON t.`{ent}` = c.`{cid}` WHERE t.`{d}` > 0")
    if gent and gdisp and gid and gmap and gx and gy:
        q.append(f"SELECT 'g', o.`{gmap}`, o.`{gx}`, o.`{gy}`, t.`{gdisp}` FROM gameobject o JOIN gameobject_template t "
                 f"ON t.`{gent}` = o.`{gid}` WHERE t.`{gdisp}` > 0")
        if gtype:
            q.append(f"SELECT 't', 0, 0, 0, t.`{gdisp}` FROM gameobject_template t WHERE t.`{gtype}` = 15 AND t.`{gdisp}` > 0")
    return ";\n".join(q) + (";\n" if q else "")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("plan")
    p.add_argument("--data", required=True)
    p.add_argument("--slice", required=True)
    p.add_argument("--spawns")
    p.add_argument("--out", required=True)
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--free-mb", type=int)
    p.add_argument("--report")
    s = sub.add_parser("spawn-sql")
    for a in ("--ct", "--gt", "--c", "--g"):
        s.add_argument(a, default="")
    a = ap.parse_args(argv)
    if a.cmd == "spawn-sql":
        sp = lambda v: [x.strip() for x in v.split(",") if x.strip()]  # noqa: E731
        sys.stdout.write(spawn_sql(sp(a.ct), sp(a.gt), sp(a.c), sp(a.g)))
        return 0

    spec = json.load(open(a.slice, encoding="utf-8"))
    storm = Storm()
    files = sorted(f for f in os.listdir(a.data) if f.lower().endswith(".mpq"))
    if not any(f.lower() == "dbc.mpq" for f in files):
        print(f"VR SLICE FAILED: no dbc.MPQ in {a.data} (not a 1.12 Data folder)")
        return 1
    chain = Chain([Archive(storm, os.path.join(a.data, f)) for f in files])
    print(f"== archives (highest first): {', '.join(x.name for x in chain.archives)}; {len(chain.where)} names")
    spawns = read_spawns(a.spawns)
    print(f"== spawns from the realm: {len(spawns)} rows" + ("" if spawns else " (none: creature folders come only from the base list)"))
    pl = Planner(chain, spec, spawns)
    total, kept, report, stopped = pl.plan(a.free_mb)
    print("== tiers")
    for k, name, n, size, ok in report:
        print(f"  {'in ' if ok else 'OUT'} {k:<26} {n:>6} files {size / 1048576:9.1f} MB  {name}")
    if stopped:
        print(f"  the budget stopped at '{stopped}': it and the tiers after it are not in the slice")
    if pl.missing:
        print(f"  {len(pl.missing)} named files are in no archive (normal for a few; first: {sorted(pl.missing)[:3]})")
    print(f"== slice: {len(pl.taken)} files, {total / 1048576:.0f} MB uncompressed")
    if a.report:
        with open(a.report, "w", encoding="utf-8") as f:
            json.dump({"files": len(pl.taken), "mb": round(total / 1048576), "kept": kept, "stopped": stopped,
                       "tiers": [{"key": k, "files": n, "mb": round(sz / 1048576, 1), "in": ok} for k, _n, n, sz, ok in report]},
                      f, indent=1)
    if a.dry_run:
        print(f"VR SLICE DRY {total // 1048576}")
        return 0
    print("== writing the slice archives")
    size = write_archives(storm, chain, pl.groups(), a.out, whole=spec.get("copyWhole", ["patch-Z.MPQ"]))
    print(f"VR SLICE OK {size // 1048576}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
