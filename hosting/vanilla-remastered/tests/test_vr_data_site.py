"""Vanilla Remastered levels 1-20 in the browser: the slice builder (vr-slice.py) against fake archives, the data step's
upload helpers and check (vr-data.sh) and the page step (vr-site-up.sh) against stub docker/curl/getent, the test
account probe against a fake 1.12 login server and SOAP port, the 1.12 part of the killswitch, and the step wiring.
No Docker daemon, no network, nothing deployed.

    python3 -I -m unittest discover -s hosting/vanilla-remastered/tests
"""
import asyncio
import hashlib
import http.server
import importlib.util
import json
import os
import re
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
VR = os.path.dirname(HERE)
ROOT = os.path.dirname(os.path.dirname(VR))
sys.path.insert(0, os.path.join(ROOT, "hosting", "kit", "tools"))
BRAND = re.compile(r"(?i)world of warcraft|warcraft|lich king|blizzard|(^|[^a-z])wow([^a-z]|$)")


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


vs = load("vr_slice", os.path.join(VR, "vr-slice.py"))
probe = load("probe112", os.path.join(VR, "probe112.py"))
gc = load("gamedata_caddy", os.path.join(ROOT, "stream", "server", "gamedata-caddy.py"))


# ---------------------------------------------------------------------------------------------- fake game files
def chunk(cid, body):
    return cid[::-1].encode() + struct.pack("<I", len(body)) + body


def names_block(*n):
    return b"".join(x.encode() + b"\x00" for x in n)


def dbc(rows, strings_list):
    """rows: lists of int|float|str (str -> offset into the string block)."""
    sb, offs = b"\x00", {}
    for s in strings_list:
        offs[s] = len(sb)
        sb += s.encode() + b"\x00"
    nf = len(rows[0])
    out = b""
    for r in rows:
        for v in r:
            out += struct.pack("<f", v) if isinstance(v, float) else struct.pack("<I", offs[v] if isinstance(v, str) else v)
    return b"WDBC" + struct.pack("<IIII", len(rows), nf, nf * 4, len(sb)) + out + sb


def m2(textures):
    head = bytearray(0x64)
    head[0:4] = b"MD20"
    struct.pack_into("<I", head, 4, 256)
    defs, strs = b"", b""
    base = 0x64 + 16 * len(textures)
    for t in textures:
        s = t.encode() + b"\x00"
        defs += struct.pack("<IIII", 0, 0, len(s), base + len(strs))
        strs += s
    struct.pack_into("<II", head, 0x5C, len(textures), 0x64)
    return bytes(head) + defs + strs


class FakeArchive:
    def __init__(self, name, files):
        self.name, self.path, self.files = name, "/fake/" + name, files

    def names(self):
        return list(self.files)

    def read(self, n):
        return self.files.get(n)

    def size(self, n):
        return len(self.files.get(n, b""))


def world():
    """A tiny 1.12-shaped world: Map.dbc, WorldMapArea.dbc (one zone of 1x1 tiles at a=32,b=48), one ADT naming a model
    and a building, the building's group and textures, creature tables, the minimap index."""
    left, right, top, bottom = (32 - 32) * 533.33333 - 1, (32 - 32) * 533.33333 - 500, (32 - 48) * 533.33333 - 1, (32 - 48) * 533.33333 - 500
    mapdbc = dbc([[0, "Azeroth"], [36, "DeadminesInstance"]], ["Azeroth", "DeadminesInstance"])
    wma = dbc([[1, 0, 12, "Elwynn", left, right, top, bottom]], ["Elwynn"])
    cdi = dbc([[100, 7, 0, 0], [101, 8, 0, 0]], [])
    cmd = dbc([[7, 0, "Creature\\Wolf\\Wolf.mdx"], [8, 0, "Creature\\Bear\\Bear.mdx"]], ["Creature\\Wolf\\Wolf.mdx", "Creature\\Bear\\Bear.mdx"])
    adt = (chunk("MVER", struct.pack("<I", 18)) + chunk("MTEX", names_block("Tileset\\Elwynn\\Grass.blp"))
           + chunk("MMDX", names_block("World\\Generic\\Tree.mdx")) + chunk("MWMO", names_block("World\\wmo\\Abbey.wmo")))
    wmo = (chunk("MVER", struct.pack("<I", 17)) + chunk("MOHD", struct.pack("<II", 1, 2))
           + chunk("MOTX", names_block("World\\wmo\\Stone.blp")) + chunk("MODN", names_block("World\\Generic\\Barrel.mdx")))
    trs = "dir: Azeroth\nAzeroth\\map32_48.blp\tabc123.blp\nAzeroth\\map10_10.blp\tfff.blp\n"
    big = b"x" * (3 * 1048576)
    base = {
        "DBFilesClient\\Map.dbc": mapdbc, "DBFilesClient\\WorldMapArea.dbc": wma,
        "DBFilesClient\\CreatureDisplayInfo.dbc": cdi, "DBFilesClient\\CreatureModelData.dbc": cmd,
        "Interface\\FrameXML\\UIParent.lua": b"-- ui", "Textures\\Minimap\\md5translate.trs": trs.encode(),
        "Textures\\ShadowBlob.blp": b"t", "Textures\\Minimap\\abc123.blp": b"mm", "Textures\\Minimap\\fff.blp": b"other",
        "World\\Maps\\Azeroth\\Azeroth.wdt": chunk("MVER", b"\x12\x00\x00\x00"), "World\\Maps\\Azeroth\\Azeroth_32_48.adt": adt,
        "World\\Maps\\Azeroth\\Azeroth_10_10.adt": b"far away", "Tileset\\Elwynn\\Grass.blp": b"g",
        "World\\Generic\\Tree.m2": m2(["World\\Generic\\Bark.blp"]), "World\\Generic\\Bark.blp": b"b",
        "World\\wmo\\Abbey.wmo": wmo, "World\\wmo\\Abbey_000.wmo": b"g0", "World\\wmo\\Abbey_001.wmo": b"g1",
        "World\\wmo\\Stone.blp": b"s", "World\\Generic\\Barrel.m2": m2([]),
        "Creature\\Wolf\\Wolf.m2": m2([]), "Creature\\Wolf\\WolfSkin.blp": b"w", "Sound\\Creature\\Wolf\\Howl.wav": b"h",
        "Creature\\Bear\\Bear.m2": m2([]), "Sound\\Music\\ZoneMusic\\Forest.mp3": big,
    }
    patch = {"Interface\\FrameXML\\UIParent.lua": b"-- patched ui"}
    return vs.Chain([FakeArchive("model.MPQ", base), FakeArchive("patch.MPQ", patch)])


SPEC = {"budgetMB": 100, "minFreeMB": 0, "marginTiles": 0,
        "base": {"prefixes": ["Interface\\", "DBFilesClient\\", "Textures\\*", "Textures\\Minimap\\md5translate.trs"]},
        "tiers": [{"key": "elwynn", "map": 0, "zones": ["Elwynn"]},
                  {"key": "music", "prefixes": ["Sound\\Music\\ZoneMusic\\"]}]}


class SliceFormats(unittest.TestCase):
    def test_tiles_and_names(self):
        self.assertEqual(vs.tile_of(0.0, 0.0), (32, 32))
        self.assertEqual(vs.tile_of(-100.0, -100.0), (32, 32))
        self.assertEqual(vs.tile_of(533.4, 533.4), (30, 30))
        self.assertEqual(vs.key("World/Generic/Tree.MDX"), "world\\generic\\tree.m2")
        t = vs.zone_tiles(1000.0, -1000.0, 1000.0, -1000.0)
        self.assertEqual(t, {(a, b) for a in (30, 31, 32, 33) for b in (30, 31, 32, 33)})
        self.assertIn((29, 29), vs.zone_tiles(1000.0, -1000.0, 1000.0, -1000.0, 1))

    def test_parsers(self):
        self.assertEqual(vs.m2_deps(m2(["A.blp", "B.blp"])), ["A.blp", "B.blp"])
        self.assertEqual(vs.m2_deps(b"MD20short"), [])
        wmo = chunk("MOHD", struct.pack("<II", 0, 3)) + chunk("MOTX", names_block("x.blp"))
        self.assertEqual(vs.wmo_deps("w\\a.wmo", wmo), ["x.blp", "w\\a_000.wmo", "w\\a_001.wmo", "w\\a_002.wmo"])
        self.assertEqual(vs.parse_trs("dir: Azeroth\nAzeroth\\map01_02.blp\th.blp\n"),
                         {"azeroth\\map01_02.blp": "textures\\minimap\\h.blp"})
        self.assertEqual(vs.priority("patch-Z.MPQ") < vs.priority("patch-2.MPQ") < vs.priority("patch.MPQ")
                         < vs.priority("wmo.MPQ") < vs.priority("dbc.MPQ"), True)

    def test_chain_priority(self):
        c = world()
        self.assertEqual(c.read(vs.key("Interface\\FrameXML\\UIParent.lua")), b"-- patched ui")

    def test_plan_walks_tiles_models_buildings_and_spawns(self):
        c = world()
        spawns = [("c", 0, -100.0, (32 - 48) * 533.33333 + 0 - 100.0 + 0.0, 100) if False else
                  ("c", 0, (32 - 48) * 533.33333 - 100.0, -100.0, 100),
                  ("c", 0, 5000.0, 5000.0, 101)]   # the bear stands outside the tier
        pl = vs.Planner(c, SPEC, spawns, log=lambda *_: None)
        total, kept, report, stopped = pl.plan()
        t = pl.taken
        for want in ("world\\maps\\azeroth\\azeroth_32_48.adt", "tileset\\elwynn\\grass.blp", "world\\generic\\tree.m2",
                     "world\\generic\\bark.blp", "world\\wmo\\abbey.wmo", "world\\wmo\\abbey_001.wmo", "world\\wmo\\stone.blp",
                     "world\\generic\\barrel.m2", "textures\\minimap\\abc123.blp", "creature\\wolf\\wolfskin.blp",
                     "sound\\creature\\wolf\\howl.wav", "textures\\shadowblob.blp", "interface\\framexml\\uiparent.lua",
                     "sound\\music\\zonemusic\\forest.mp3"):
            self.assertIn(want, t)
        for no in ("world\\maps\\azeroth\\azeroth_10_10.adt", "textures\\minimap\\fff.blp", "creature\\bear\\bear.m2"):
            self.assertNotIn(no, t)
        self.assertEqual(kept, ["base", "elwynn", "music"])
        self.assertIsNone(stopped)
        g = pl.groups()
        self.assertIn(("interface\\framexml\\uiparent.lua", "Interface\\FrameXML\\UIParent.lua"), g["patch.MPQ"])
        self.assertNotIn("interface\\framexml\\uiparent.lua", [k for k, _ in g["model.MPQ"]])

    def test_budget_stops_at_a_tier_and_after(self):
        spec = dict(SPEC, budgetMB=2)
        pl = vs.Planner(world(), spec, [], log=lambda *_: None)
        total, kept, report, stopped = pl.plan()
        self.assertEqual(stopped, "music")
        self.assertNotIn("sound\\music\\zonemusic\\forest.mp3", pl.taken)
        self.assertLess(total, 2 * 1048576)
        pl = vs.Planner(world(), SPEC, [], log=lambda *_: None)
        self.assertEqual(pl.plan(free_mb=2)[3], "music")    # little disk free: the same

    def test_spawn_sql(self):
        q = vs.spawn_sql(["Entry", "DisplayId1", "DisplayId2", "Name"], ["entry", "displayId", "type"],
                         ["guid", "id", "map", "position_x", "position_y"], ["guid", "id", "map", "position_x", "position_y"])
        self.assertEqual(q.count("SELECT 'c'"), 2)
        self.assertIn("SELECT 'g'", q)
        self.assertIn("= 15", q)
        self.assertEqual(vs.spawn_sql(["Name"], [], [], []), "")

    def test_shipped_spec(self):
        spec = json.load(open(os.path.join(VR, "slice-112.json"), encoding="utf-8"))
        keys = [t["key"] for t in spec["tiers"]]
        self.assertEqual(len(keys), len(set(keys)))
        self.assertEqual(spec["copyWhole"], ["patch-Z.MPQ"])
        zones = {z for t in spec["tiers"] for z in t.get("zones", [])}
        for z in ("Elwynn", "DunMorogh", "Teldrassil", "Durotar", "Mulgore", "Tirisfal", "Westfall", "Barrens", "Silverpine",
                  "Darkshore", "LochModan"):
            self.assertIn(z, zones)
        self.assertLessEqual(spec["budgetMB"], 3000)


# ---------------------------------------------------------------------------------------------- stubs
DOCKER = r"""#!/usr/bin/env bash
S="$STUB_STATE"; echo "docker $*" >> "$S/docker.log"
has() { [ -f "$S/$1" ]; }
case "$1" in
  ps) all=0; [ "$2" = -a ] && all=1
      for f in "$S"/exists_*; do [ -e "$f" ] || continue; n=${f##*/exists_}
        if [ $all = 1 ] || has "running_$n"; then echo "$n"; fi; done; exit 0 ;;
  image) has noimage && exit 1; exit 0 ;;
  images) exit 0 ;;
  volume) v="${@: -1}"; case "$2" in inspect) [ "$v" = wenilla-realm_gamedata ] && { echo "$S/gd"; exit 0; }; has "vol_$v" && exit 0; exit 1 ;;
            create) touch "$S/vol_${@: -1}"; exit 0 ;; rm) exit 0 ;; esac ;;
  inspect)
    t="$3"; n="$4"
    case "$t" in
      '{{.Image}}') echo "sha256:site$n" ;;
      *Config.Image*) echo "ghcr.io/orionopenwow/openvanilla-realm:live" ;;
      *Config.Env*) printf 'PATH=/usr/bin\nCLIENT_DATA=/client/Data\nREALMD_HOST=realmd\nMANGOSD_HOST=mangosd\nPUBLIC_URL=https://play.openvanilla.app\nSOAP_USER=SOAPUSER-sentinel\nSOAP_PASSWORD=SOAPPASS-sentinel\nDB_PASSWORD=DBSECRET-sentinel\n' ;;
      *Mounts*) case "$n" in wenilla-realm-realm-1) printf 'bind||/srv/client/Data|/client/Data|false\nvolume|wenilla-realm_realmdata||/data|true\n' ;; esac ;;
      *WorkingDir*) echo "/app" ;;
      *Config.User*) echo "" ;;
      *'json .Config.Entrypoint'*) echo '["/app/wenilla-realm"]' ;;
      *'json .Config.Cmd'*) echo null ;;
    esac; exit 0 ;;
  exec) echo "$*" >> "$S/exec.log"; exit 0 ;;
  network) echo "$*" >> "$S/net.log"; exit 0 ;;
  run) shift
    if [ "$1" = --rm ]; then
      case "$*" in *probe112.py*account*) echo "account: test account VRTEST000001 made"; has fail_account && exit 1; exit 0 ;;
                   *probe112.py*) echo "HTTP 200"; exit 0 ;; esac
      exit 0
    fi
    name=""; prev=""; for a; do [ "$prev" = --name ] && name=$a; prev=$a; done
    echo "$*" > "$S/run_$name"; touch "$S/exists_$name" "$S/running_$name"; exit 0 ;;
  rm) for a; do rm -f "$S/running_$a" "$S/exists_$a"; done; exit 0 ;;
  restart|update|logs|start|stop) exit 0 ;;
esac
exit 0
"""
GETENT = r"""#!/usr/bin/env bash
case "$2" in play.openvanilla.app) echo "10.0.0.5 STREAM play.openvanilla.app" ;;
  *) [ -f "$STUB_STATE/dns_ok" ] && echo "10.0.0.5 STREAM $2" ;; esac
exit 0
"""
CURL = r"""#!/usr/bin/env bash
for a; do u=$a; done
case "$*" in *%\{http_code\}*) case "$u" in *__index*) echo 403 ;; *) echo 200 ;; esac ;; *) echo "<html>Open Vanilla</html>" ;; esac
exit 0
"""


class Stubbed(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.mkdtemp(prefix="vr-data-site-")
        self.bin, self.st = os.path.join(self.t, "bin"), os.path.join(self.t, "stub")
        os.makedirs(self.bin)
        os.makedirs(os.path.join(self.st, "gd"))
        for n, body in (("docker", DOCKER), ("getent", GETENT), ("curl", CURL)):
            p = os.path.join(self.bin, n)
            with open(p, "w") as f:
                f.write(body)
            os.chmod(p, 0o755)
        p = os.path.join(self.bin, "hostname")
        with open(p, "w") as f:
            f.write("#!/bin/sh\necho 10.0.0.5\n")
        os.chmod(p, 0o755)
        self.root = os.path.join(self.t, "vr112")
        self.env = dict(os.environ, VR_SLICE_MB="10", VR_DISK_MARGIN_MB="10", PATH=self.bin + os.pathsep + os.environ["PATH"], STUB_STATE=self.st, VR_TEST="1",
                        VR_ROOT=self.root, VR_ENV_DIR=os.path.join(self.t, "env"), WU_PORTAL_DIR=os.path.join(self.t, "portal"),
                        VR_CADDYFILE=os.path.join(self.t, "Caddyfile"), VR_CP=os.path.join(self.t, "cp"),
                        VR_BEACON_DIR=os.path.join(self.t, "beacon"), VR_REALMCTL=os.path.join(self.t, "none"))
        os.makedirs(os.path.join(self.t, "portal"))
        with open(os.path.join(self.t, "Caddyfile"), "w") as f:
            f.write("play.openvanilla.app {\n\treverse_proxy ovlive-realm:8090\n}\n")
        self.src = os.path.join(self.t, "src")
        os.makedirs(os.path.join(self.src, "hosting/vanilla-remastered"))
        os.makedirs(os.path.join(self.src, "stream/server"))
        for f in ("vr-data.sh", "vr-site-up.sh", "probe112.py", "vr-slice.py", "slice-112.json"):
            shutil.copy(os.path.join(VR, f), os.path.join(self.src, "hosting/vanilla-remastered", f))
        shutil.copy(os.path.join(ROOT, "stream/server/gamedata-caddy.py"), os.path.join(self.src, "stream/server"))
        shutil.copytree(os.path.join(ROOT, "hosting/kit/tools/wunet"), os.path.join(self.src, "hosting/kit/tools/wunet"),
                        ignore=shutil.ignore_patterns("__pycache__"))

    def tearDown(self):
        shutil.rmtree(self.t, ignore_errors=True)

    def touch(self, n):
        open(os.path.join(self.st, n), "w").close()

    def data(self, *a, stdin=None):
        return subprocess.run(["bash", os.path.join(self.src, "hosting/vanilla-remastered/vr-data.sh"), self.src] + list(a),
                              env=self.env, input=stdin, capture_output=True, timeout=60)

    def site(self, *a):
        return subprocess.run(["bash", os.path.join(self.src, "hosting/vanilla-remastered/vr-site-up.sh"), self.src] + list(a),
                              env=self.env, capture_output=True, text=True, timeout=60)


class DataStep(Stubbed):
    def test_resumable_checked_upload(self):
        blob = os.urandom(300000)
        sha = hashlib.sha256(blob).hexdigest()
        r = self.data("recv-chunk", "dbc.MPQ", "0", stdin=blob[:100000])
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn(b"PART 100000", r.stdout)
        r = self.data("recv-chunk", "dbc.MPQ", "50000", stdin=blob[50000:200000])     # a resend from an earlier offset
        self.assertIn(b"PART 200000", r.stdout)
        r = self.data("recv-status")
        self.assertIn(b"part\tdbc.MPQ\t200000", r.stdout)
        self.data("recv-chunk", "dbc.MPQ", "200000", stdin=blob[200000:])
        r = self.data("recv-finish", "dbc.MPQ", "0" * 64, str(len(blob)))
        self.assertNotEqual(r.returncode, 0)            # a wrong checksum: refused, the part deleted
        self.assertFalse(os.path.exists(os.path.join(self.root, "upload", "dbc.MPQ.part")))
        self.data("recv-chunk", "dbc.MPQ", "0", stdin=blob)
        r = self.data("recv-finish", "dbc.MPQ", sha, str(len(blob)))
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn(b"CHECKED dbc.MPQ", r.stdout)
        r = self.data("recv-status")
        self.assertIn(f"file\tdbc.MPQ\t{len(blob)}\t{sha}".encode(), r.stdout)
        self.assertEqual(oct(os.stat(os.path.join(self.root, "upload", "dbc.MPQ")).st_mode & 0o777), "0o600")

    def test_names_are_checked(self):
        for bad in ("../x.MPQ", "a b.MPQ", "x.exe"):
            r = self.data("recv-chunk", bad, "0", stdin=b"x")
            self.assertNotEqual(r.returncode, 0)
            self.assertIn(b"bad name", r.stdout)

    def test_check_is_read_only_and_reports_disk(self):
        r = self.data("check", "999999999")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn(b"not enough disk", r.stdout)
        r = self.data("check", "0")
        out = r.stdout.decode()
        self.assertIn("world server data", out)
        self.assertIn("extractors: NOT FOUND", out)
        self.assertNotIn("sentinel", out)
        log = open(os.path.join(self.st, "docker.log")).read()
        for verb in ("docker rm", "docker update", "docker restart", "docker build"):
            self.assertNotIn(verb, log)

    def test_job_refuses_while_world_runs_or_archives_missing(self):
        r = self.data("job")
        self.assertIn(b"is not uploaded and checked", r.stdout)
        self.touch("exists_vr-mangosd")
        self.touch("running_vr-mangosd")
        os.makedirs(os.path.join(self.root, "upload"), exist_ok=True)
        for n in ("dbc", "model", "texture", "terrain", "wmo", "interface", "misc", "fonts", "sound", "speech", "patch"):
            for ext in (".MPQ", ".MPQ.sha256"):
                open(os.path.join(self.root, "upload", n + ext), "w").close()
        r = self.data("job")
        self.assertIn(b"vr-mangosd is running", r.stdout)
        self.assertNotEqual(r.returncode, 0)


    def test_after_clean_no_upload_and_movement_maps_are_built(self):
        st = os.path.join(self.root, "state")
        os.makedirs(st, exist_ok=True)
        for p in ("extract", "classicplus", "slice", "settings", "clean"):
            with open(os.path.join(st, p + ".done"), "w") as f:
                f.write("2026-10-31T00:00:00Z\n")
        r = self.data("recv-status")
        self.assertIn(b"cleaned\t", r.stdout)              # the PC side skips the 5 GB upload
        r = self.data("check", "999999999")
        self.assertIn(b"VR DATA CHECK OK", r.stdout)       # the archive size no longer counts
        for d in ("maps", "vmaps"):
            os.makedirs(os.path.join(self.st, "gd", d), exist_ok=True)
            open(os.path.join(self.st, "gd", d, "0004331.map"), "w").close()
        r = self.data("job")
        out = r.stdout.decode()
        self.assertIn("Movement maps (the world server needs them)", out)
        self.assertIn("no CMaNGOS tools", out)             # the stub has no MoveMapGen: stops, nothing marked
        self.assertFalse(os.path.exists(os.path.join(st, "mmaps.done")))
        self.assertNotEqual(r.returncode, 0)


class SiteStep(Stubbed):
    def setUp(self):
        super().setUp()
        for c in ("wenilla-realm-realm-1", "vr-realmd", "vr-mangosd", "wenilla-realm-caddy-1"):
            self.touch("exists_" + c)
            self.touch("running_" + c)
        os.makedirs(os.path.join(self.root, "slice", "Data"))
        open(os.path.join(self.root, "slice", "Data", "dbc.MPQ"), "w").close()

    def test_check_without_dns_says_what_the_owner_adds(self):
        r = self.site("--check")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("WOULD STOP: the address vanilla.openvanilla.app has no DNS record", r.stdout)
        self.assertIn("A record: vanilla.openvanilla.app -> 10.0.0.5", r.stdout)
        self.assertIn("VR SITE DRY", r.stdout)
        self.assertIn("PUBLIC_URL", r.stdout)            # the names of address settings, never values
        self.assertNotIn("sentinel", r.stdout + r.stderr)

    def test_go_refuses_without_dns_and_changes_nothing(self):
        before = open(os.path.join(self.t, "Caddyfile")).read()
        r = self.site("--go")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("no DNS record", r.stderr)
        self.assertEqual(open(os.path.join(self.t, "Caddyfile")).read(), before)
        self.assertFalse(os.path.exists(os.path.join(self.st, "run_vr-site")))

    def test_go(self):
        self.touch("dns_ok")
        r = self.site("--go")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("VR SITE ON", r.stdout)
        run = open(os.path.join(self.st, "run_vr-site")).read()
        self.assertIn("--network vr-112", run)
        self.assertIn(f"type=bind,src={self.root}/slice/Data,dst=/client/Data,readonly", run)
        self.assertNotIn("/srv/client/Data", run)
        self.assertIn("type=volume,src=vr-site-wenilla-realm_realmdata,dst=/data", run)    # a copy, not the old volume
        self.assertIn("sha256:sitewenilla-realm-realm-1", run)                                 # the same image id
        env = open(os.path.join(self.t, "env", "vr-site.env")).read()
        self.assertIn("REALMD_HOST=vr-realmd", env)
        self.assertIn("MANGOSD_HOST=vr-mangosd", env)
        self.assertIn("PUBLIC_URL=https://vanilla.openvanilla.app", env)
        self.assertEqual(oct(os.stat(os.path.join(self.t, "env", "vr-site.env")).st_mode & 0o777), "0o600")
        cf = open(os.path.join(self.t, "Caddyfile")).read()
        self.assertIn("vanilla.openvanilla.app {\n\tencode zstd gzip\n\treverse_proxy vr-site:8090\n}", cf)
        self.assertIn("play.openvanilla.app {\n\treverse_proxy ovlive-realm:8090\n}", cf)
        self.assertEqual(open(os.path.join(self.t, "portal", "vr-site.url")).read(), "https://vanilla.openvanilla.app/\n")
        self.assertEqual(open(os.path.join(self.root, "site-host")).read().strip(), "vanilla.openvanilla.app")
        self.assertNotIn("sentinel", r.stdout + r.stderr)
        self.assertFalse(os.path.exists(os.path.join(self.t, "env", "vr-soap-test.env")))   # the SOAP login file is gone
        # run again: safe
        r = self.site("--go")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertEqual(open(os.path.join(self.t, "Caddyfile")).read().count("vanilla.openvanilla.app {"), 1)
        # off
        r = self.site("--off")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        cf = open(os.path.join(self.t, "Caddyfile")).read()
        self.assertNotIn("vanilla.openvanilla.app", cf)
        self.assertFalse(os.path.exists(os.path.join(self.t, "portal", "vr-site.url")))

    def test_killswitch_on_puts_the_410_block_in(self):
        self.touch("dns_ok")
        open(os.path.join(self.t, "portal", "gamedata-off"), "w").close()
        r = self.site("--go")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        cf = open(os.path.join(self.t, "Caddyfile")).read()
        self.assertEqual(gc.vr_status(cf), "on")

    def test_failed_account_check_rolls_back(self):
        self.touch("dns_ok")
        self.touch("fail_account")
        r = self.site("--go")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("rolled back", r.stderr)
        self.assertNotIn("vanilla.openvanilla.app", open(os.path.join(self.t, "Caddyfile")).read())
        self.assertFalse(os.path.exists(os.path.join(self.st, "exists_vr-site")))


class KillswitchVr(unittest.TestCase):
    def test_block(self):
        s = ("play.openvanilla.app {\n\thandle {\n\t}\n}\n\n" + gc.VR_SITE_BEGIN + ": x\nvanilla.example.app {\n"
             "\treverse_proxy vr-site:8090\n}\n" + gc.VR_SITE_END + "\n")
        self.assertEqual(gc.vr_status(s), "off")
        on = gc.vr_add(s, "vanilla.example.app")
        self.assertEqual(gc.vr_status(on), "on")
        self.assertEqual(gc.status(on), "no-site")          # the Wrath block's status is its own
        self.assertEqual(gc.vr_add(on, "vanilla.example.app"), on)
        self.assertIn("@gamedata_off_vr path /data /data/*", on)
        self.assertEqual(gc.vr_strip(on), s)
        self.assertEqual(gc.strip(on), on)                  # the Wrath strip leaves the 1.12 block alone
        with self.assertRaises(LookupError):
            gc.vr_add("x {\n}\n", "vanilla.example.app")
        self.assertIsNone(BRAND.search(gc.VR_BODY))
        self.assertNotRegex(gc.VR_BODY, r"\d+\.\d+")

    def test_switch_script_wires_it(self):
        s = open(os.path.join(ROOT, "stream/server/gamedata-switch.sh"), encoding="utf-8").read()
        self.assertIn('caddy_apply add-vr "$(vr_host)"', s)
        self.assertIn("caddy_apply remove-vr", s)
        self.assertIn('"$VR_SLICE" "$dest/vr112/Data"', s)
        self.assertEqual(subprocess.run(["bash", "-n", os.path.join(ROOT, "stream/server/gamedata-switch.sh")]).returncode, 0)


# ---------------------------------------------------------------------------------------------- the account probe
class Fake112:
    """A 1.12.1 login server (SRP6, build 5875 framing) and a SOAP port that makes and deletes accounts."""

    def __init__(self):
        from wunet import srp6
        self.srp6, self.accounts, self.cmds = srp6, {}, []
        self.loop = asyncio.new_event_loop()
        self.sock = socket.socket(); self.sock.bind(("127.0.0.1", 0)); self.sock.listen()
        self.port = self.sock.getsockname()[1]
        threading.Thread(target=self._run, daemon=True).start()
        fake = self

        class H(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                body = self.rfile.read(int(self.headers["Content-Length"])).decode()
                cmd = re.search(r"<command>(.*)</command>", body).group(1)
                fake.cmds.append(cmd)
                p = cmd.split()
                if p[:2] == ["account", "create"]:
                    fake.accounts[p[2].upper()] = srp6.make_verifier(p[2], p[3])
                    res = "Account created"
                elif p[:2] == ["account", "delete"]:
                    fake.accounts.pop(p[2].upper(), None)
                    res = "Account deleted"
                else:
                    res = "?"
                out = f"<SOAP-ENV:Envelope><SOAP-ENV:Body><ns1:executeCommandResponse><result>{res}</result></ns1:executeCommandResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>".encode()
                self.send_response(200); self.send_header("Content-Length", str(len(out))); self.end_headers(); self.wfile.write(out)

            def log_message(self, *a):
                pass
        self.http = http.server.HTTPServer(("127.0.0.1", 0), H)
        self.soap_port = self.http.server_address[1]
        threading.Thread(target=self.http.serve_forever, daemon=True).start()

    def _run(self):
        asyncio.set_event_loop(self.loop)
        self.loop.run_until_complete(self._serve())

    async def _serve(self):
        srv = await asyncio.start_server(self._client, sock=self.sock)
        async with srv:
            await srv.serve_forever()

    async def _client(self, r, w):
        srp6 = self.srp6
        try:
            head = await r.readexactly(4)
            body = await r.readexactly(struct.unpack_from("<H", head, 2)[0])
            user = body[30:30 + body[29]].decode()
            if body[29] and body[0:4] == b"WoW\x00" and body[4:7] == bytes([1, 12, 1]) and user.upper() in self.accounts:
                salt, ver = self.accounts[user.upper()]
                s = srp6.Server(user, salt, ver)
                w.write(bytes([0, 0, 0]) + s.B + bytes([1, srp6.G]) + bytes([32]) + srp6.N.to_bytes(32, "little")
                        + salt + bytes(16) + bytes([0]))
                await w.drain()
                proof = await r.readexactly(75)
                got = s.verify(proof[1:33], proof[33:53])
                if not got:
                    w.write(bytes([1, 4])); await w.drain(); return
                w.write(bytes([1, 0]) + got[1] + bytes(4))
                await r.readexactly(5)
                realms = struct.pack("<IB", 0, 1) + struct.pack("<IB", 1, 0) + b"Vanilla Remastered\x00vr:8085\x00" + struct.pack("<fBBB", 0, 0, 1, 0) + b"\x02\x00"
                w.write(bytes([0x10]) + struct.pack("<H", len(realms)) + realms)
            else:
                w.write(bytes([0, 0, 4]))
            await w.drain()
        except (asyncio.IncompleteReadError, ConnectionError):
            pass
        finally:
            w.close()


def probe_dir():
    """probe112.py beside the kit's wunet, as the probe container mounts them."""
    d = tempfile.mkdtemp(prefix="vr-probe-")
    shutil.copy(os.path.join(VR, "probe112.py"), d)
    shutil.copytree(os.path.join(ROOT, "hosting/kit/tools/wunet"), os.path.join(d, "wunet"),
                    ignore=shutil.ignore_patterns("__pycache__"))
    return os.path.join(d, "probe112.py")


class AccountProbe(unittest.TestCase):
    def test_full_account_check(self):
        f = Fake112()
        world = socket.socket(); world.bind(("127.0.0.1", 0)); world.listen()

        def greet():
            c, _ = world.accept()
            c.sendall(struct.pack(">H", 6) + struct.pack("<H", 0x1EC) + b"\x00" * 4)
            c.close()
        threading.Thread(target=greet, daemon=True).start()
        env = dict(os.environ, VR_SOAP_USER="gm", VR_SOAP_PASS="gmpass-sentinel")
        r = subprocess.run([sys.executable, "-I", probe_dir(), "account", "127.0.0.1", str(f.soap_port),
                            "127.0.0.1", str(f.port), "127.0.0.1", str(world.getsockname()[1])],
                           env=env, capture_output=True, text=True, timeout=60)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("realm list has 1 realm(s)", r.stdout)
        self.assertIn("world server greeting 0x01ec (ok)", r.stdout)
        self.assertIn("test account deleted (ok)", r.stdout)
        self.assertEqual(f.accounts, {})                           # deleted again
        self.assertTrue(f.cmds[0].startswith("account create VRTEST"))
        pw = f.cmds[0].split()[3]
        self.assertNotIn(pw, r.stdout + r.stderr)                  # the test password is never printed
        self.assertNotIn("sentinel", r.stdout + r.stderr)

    def test_no_soap_login_is_skipped_not_failed(self):
        env = {k: v for k, v in os.environ.items() if not k.startswith("VR_SOAP")}
        r = subprocess.run([sys.executable, "-I", probe_dir(), "account", "h", "1", "h", "1", "h", "1"],
                           env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(r.returncode, 3)
        self.assertIn("SKIPPED", r.stdout)


# ---------------------------------------------------------------------------------------------- wiring
class Wiring(unittest.TestCase):
    def test_steps_and_bats(self):
        ctl = open(os.path.join(ROOT, "tools/control/control.py"), encoding="utf-8").read()
        for step, bat in (("vr-data-check", "Vanilla Remastered data - check.bat"), ("vr-data", "Vanilla Remastered data.bat"),
                          ("vr-site-check", "Vanilla Remastered browser page - check.bat"),
                          ("vr-site", "Vanilla Remastered browser page.bat"),
                          ("vr-site-off", "Vanilla Remastered browser page - off.bat"),
                          ("vr-test", "Vanilla Remastered self-test.bat")):
            self.assertIn(f'"{step}": ("hosting/vanilla-remastered/{bat}"', ctl)
            b = open(os.path.join(VR, bat), "rb").read()
            self.assertIn(b"\r\n", b)
        ro = re.search(r"READ_ONLY = \{([^}]*)\}", ctl, re.S).group(1)
        self.assertIn('"vr-data-check"', ro)
        self.assertIn('"vr-site-check"', ro)
        self.assertNotIn('"vr-data"', ro.replace('"vr-data-check"', ""))
        self.assertNotIn('"vr-test"', ro)

    def test_no_brand_words_or_versions_for_players(self):
        for f in ("vr-site-up.sh", "vr-data.sh", "vr-data.ps1", "slice-112.json"):
            for line in open(os.path.join(VR, f), encoding="utf-8"):
                if re.search(r"^\s*(echo|note|Say|Fail)\b", line) or '"name"' in line or '"title"' in line:
                    self.assertIsNone(BRAND.search(line), f + ": " + line)
        net = open(os.path.join(ROOT, "stream/network.py"), encoding="utf-8").read()
        self.assertIn("Play in your browser", net)

    def test_syntax(self):
        for f in ("vr-data.sh", "vr-site-up.sh"):
            self.assertEqual(subprocess.run(["bash", "-n", os.path.join(VR, f)]).returncode, 0, f)


if __name__ == "__main__":
    unittest.main()
