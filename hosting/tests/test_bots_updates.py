"""Bots always up to date (docs/BOTS-UPDATES.md): the compat guard (realm/bots/compat.py), the pins
(realm/bots/pins.py, realm/bots.json), the canary's proposal (realm/bots/candidate.py), and the smoke test's player
(realm/bots/bots_smoke.py) run whole against a fake realm whose world simulates the bots and the probe
(ov_bots_probe.lua's answers), plus a fake console (SOAP). The probe itself: test_ov_bots_probe.lua under texlua.

    python3 hosting/tests/test_bots_updates.py
"""
import asyncio
import base64
import http.server
import json
import os
import re
import shutil
import stat
import struct
import subprocess
import sys
import tempfile
import threading
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
BOTS = os.path.join(REPO, "realm", "bots")
sys.path.insert(0, os.path.join(REPO, "hosting", "kit", "tools"))
sys.path.insert(0, HERE)
sys.path.insert(0, BOTS)

import bots_smoke as bs  # noqa: E402
import candidate  # noqa: E402
import compat  # noqa: E402
import pins  # noqa: E402
from fake_realm import FakeRealm  # noqa: E402
from wunet import wire335  # noqa: E402


def put(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


def env_key_to_ini(env):
    """The inverse of AzerothCore's IniKeyToEnvVarKey for the keys the test needs (CamelCase words)."""
    words = env[len("AC_PLAYERBOTS_"):].split("_")
    return "Playerbots." + "".join(w.capitalize() for w in words if w)


def fake_playerbots(root, n, drop=()):
    """A mod-playerbots tree with exactly what `n` (compat.needs) asks for, minus `drop`."""
    keep = lambda xs: [x for x in xs if x not in drop]  # noqa: E731
    put(os.path.join(root, "src", "Ai", "Base", "ChatTriggerContext.h"),
        "\n".join(f'        creators["{c}"] = &ChatTriggerContext::x;' for c in keep(n["whispers"])))
    put(os.path.join(root, "src", "Ai", "Base", "StrategyContext.h"),
        "\n".join(f'        creators["{c}"] = &StrategyContext::x;' for c in keep(n["strategies"])))
    put(os.path.join(root, "src", "Ai", "Base", "Value", "Formations.cpp"),
        "\n".join(f'    else if (formation == "{c}")' for c in keep(n["formations"])))
    put(os.path.join(root, "src", "Bot", "PlayerbotMgr.cpp"),
        "\n".join(('    if (!strcmp(cmd, "addclass"))' if c == "addclass" else
                   '            else if (cmd == "init=auto")' if c == "init=auto" else f'    if (cmd == "{c}")')
                  for c in keep(n["bot_commands"])))
    conf = []
    for e in keep(n["env"]):
        m = re.match(r"AC_PLAYERBOTS_PREMADE_SPEC_(NAME|GLYPH|LINK)_(\d+)__(\d+)", e)
        if m:
            conf.append(f"Playerbots.PremadeSpec{m.group(1).capitalize()}.{m.group(2)}.{m.group(3)} = x")
        else:
            conf.append(f"{env_key_to_ini(e)} = 1")
    for i, s in enumerate(keep(n["specs"])):
        conf.append(f"Playerbots.PremadeSpecName.99.{i} = {s}")
    put(os.path.join(root, "conf", "playerbots.conf.dist"), "\n".join(conf) + "\n")
    return root


def fake_ale(root, names):
    put(os.path.join(root, "src", "LuaEngine", "LuaFunctions.cpp"),
        "\n".join(f'    {{ "{m}", &LuaPlayer::{m} }},' for m in names))
    return root


class Compat(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="bots-compat-")
        self.n = compat.needs(REPO)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_what_the_script_needs(self):
        n = self.n
        self.assertEqual(n["bot_commands"], ["add", "addclass", "init=auto", "remove"])
        for w in ("follow", "summon", "ll", "talents", "maintenance", "autogear", "co", "nc", "leave", "stay", "formation"):
            self.assertIn(w, n["whispers"])
        self.assertEqual(n["strategies"], ["cc", "dps assist", "mark rti", "passive"])
        self.assertIn("chaos", n["formations"])
        for m in compat.REQUIRED_METHODS:
            self.assertIn(m, n["required_methods"])
        self.assertIn("prot pve", n["specs"])
        # the loot keys that make every bot pass (realm/companions-README.md)
        for k in ("AC_PLAYERBOTS_LOOT_NEED_ROLL_LEVEL", "AC_PLAYERBOTS_LOOT_GREED_ROLL_LEVEL", "AC_PLAYERBOTS_FREE_METHOD_LOOT"):
            self.assertIn(k, n["env"])
        self.assertNotIn("AC_PLAYERBOTS_DATABASE_INFO", n["env"])
        # comments don't count: a command only named in a comment is not a need
        got = compat.script_needs('-- botCommand(owner, "ghost x")\nbotCommand(owner, "remove " .. n)\n')
        self.assertEqual(got["bot_commands"], ["remove"])

    def test_env_names_follow_the_core(self):
        self.assertEqual(compat.ini_key_to_env("Playerbots.LootNeedRollLevel"), "AC_PLAYERBOTS_LOOT_NEED_ROLL_LEVEL")
        self.assertEqual(compat.ini_key_to_env("Playerbots.PremadeSpecName.2.0"), "AC_PLAYERBOTS_PREMADE_SPEC_NAME_2__0")
        self.assertEqual(compat.ini_key_to_env("PlayerbotsDatabaseInfo"), "AC_PLAYERBOTS_DATABASE_INFO")

    def run_cli(self, pb, ale=None):
        args = [sys.executable, "-I", os.path.join(BOTS, "compat.py"), "--playerbots", pb, "--json"]
        if ale:
            args += ["--ale", ale]
        p = subprocess.run(args, capture_output=True, text=True, timeout=60)
        return p.returncode, json.loads(p.stdout) if p.stdout.strip().startswith("{") else p.stdout

    def test_a_tree_with_everything_passes(self):
        pb = fake_playerbots(os.path.join(self.tmp, "pb"), self.n)
        ale = fake_ale(os.path.join(self.tmp, "ale"), self.n["methods"])
        rc, out = self.run_cli(pb, ale)
        self.assertEqual(rc, 0, out)
        self.assertTrue(out["ok"])
        self.assertEqual(out["missing"], [])

    def test_renames_upstream_fail_the_canary(self):
        pb = fake_playerbots(os.path.join(self.tmp, "pb"), self.n,
                             drop=("summon", "passive", "init=auto", "AC_PLAYERBOTS_LOOT_NEED_ROLL_LEVEL", "prot pve"))
        rc, out = self.run_cli(pb)
        self.assertEqual(rc, 1, out)
        missing = {(m["kind"], m["name"]) for m in out["missing"] if m["fatal"]}
        self.assertEqual(missing, {("whispers", "summon"), ("strategies", "passive"), ("bot_commands", "init=auto"),
                                   ("env", "AC_PLAYERBOTS_LOOT_NEED_ROLL_LEVEL"), ("specs", "prot pve")})

    def test_methods_required_or_noted(self):
        pb = fake_playerbots(os.path.join(self.tmp, "pb"), self.n)
        ale = fake_ale(os.path.join(self.tmp, "ale"), [m for m in self.n["methods"] if m not in ("GetBotAI", "IsBattleArena")])
        rc, out = self.run_cli(pb, ale)
        self.assertEqual(rc, 1)
        got = {m["name"]: m["fatal"] for m in out["missing"]}
        self.assertEqual(got, {"GetBotAI": True, "IsBattleArena": False})   # the script has a fallback for the second

    def test_not_a_tree(self):
        rc, out = self.run_cli(self.tmp)
        self.assertEqual(rc, 2)
        self.assertIn("not a mod-playerbots tree", out)

    def test_human_report(self):
        txt = compat.report([("whispers", "summon", True), ("methods", "IsBattleArena", False)])
        self.assertIn("MISSING whispers", txt)
        self.assertIn("FAILED (1 missing)", txt)
        self.assertIn("OK (every", compat.report([]))


class Pins(unittest.TestCase):
    LOCK = ("# name repo branch commit\n"
            "core https://example.invalid/core Playerbot " + "a" * 40 + "\r\n"
            "mod-ale https://example.invalid/ale master " + "b" * 40 + "\n"
            "mod-playerbots https://example.invalid/pb master " + "c" * 40 + "\n"
            "mod-sandbox-core local - -\n")

    def test_the_repo_manifest_matches_the_lock(self):
        p = subprocess.run([sys.executable, "-I", os.path.join(BOTS, "pins.py"), "check"], capture_output=True, text=True)
        self.assertEqual(p.returncode, 0, p.stdout)
        m = json.load(open(os.path.join(REPO, "realm", "bots.json")))
        self.assertEqual(m["format"], 1)
        self.assertTrue(m["playerbots"].startswith("e88cc40"))       # companions-README.md's pin

    def test_bump_moves_only_the_two(self):
        new = pins.bump(self.LOCK, "d" * 40, "e" * 40)
        self.assertEqual(pins.pins(new), {"playerbots": "d" * 40, "core": "e" * 40})
        self.assertIn("mod-ale https://example.invalid/ale master " + "b" * 40, new)
        self.assertIn("Playerbot " + "e" * 40 + "\r\n", new)          # line endings kept
        self.assertIn("mod-sandbox-core local - -", new)
        with self.assertRaises(ValueError):
            pins.bump(self.LOCK, "short", "e" * 40)
        with self.assertRaises(ValueError):
            pins.bump("core x y " + "a" * 40 + "\n", "d" * 40, "e" * 40)

    def test_cli_bump_same_check(self):
        t = tempfile.mkdtemp()
        try:
            lock, man = os.path.join(t, "sources.lock"), os.path.join(t, "bots.json")
            with open(lock, "w", newline="") as f:
                f.write(self.LOCK)
            run = lambda *a: subprocess.run([sys.executable, "-I", os.path.join(BOTS, "pins.py"), "--lock", lock,  # noqa: E731
                                             "--manifest-file", man, *a], capture_output=True, text=True)
            self.assertEqual(run("check").returncode, 1)                       # no manifest yet
            self.assertEqual(run("manifest", "--date", "2026-11-04").returncode, 0)
            self.assertEqual(run("check").returncode, 0)
            self.assertEqual(run("same", "--playerbots", "c" * 40, "--core", "a" * 40).returncode, 0)
            self.assertEqual(run("same", "--playerbots", "d" * 40, "--core", "a" * 40).returncode, 1)
            self.assertEqual(run("bump", "--playerbots", "d" * 40, "--core", "e" * 40, "--date", "2026-11-09",
                                 "--changes", "12").returncode, 0)
            m = json.load(open(man))
            self.assertEqual(m, {"format": 1, "playerbots": "d" * 40, "core": "e" * 40, "date": "2026-11-09", "changes": 12})
            self.assertEqual(run("check").returncode, 0)
        finally:
            shutil.rmtree(t)


class Candidate(unittest.TestCase):
    PINS = {"playerbots": "d" * 40, "core": "e" * 40, "changes": 7}

    def test_passed(self):
        rep = {"ok": True, "checks": [{"name": "recruit", "ok": True, "detail": "4 bots | x"}]}
        c = candidate.candidate(self.PINS, "success", rep, {"ok": True, "missing": []}, "canary-ddddddd", "https://x/run")
        self.assertEqual(c["smoke"], "passed")
        b = candidate.body(c)
        self.assertIn("passed on the canary images (`canary-ddddddd`)", b)
        self.assertIn("| recruit | pass | 4 bots / x |", b)
        self.assertIn("Nothing here merges by itself", b)

    def test_pending_and_failed(self):
        c = candidate.candidate(self.PINS, "skipped", None, {"ok": True}, "canary-ddddddd")
        self.assertEqual(c["smoke"], "pending")
        self.assertIn("Smoke test pending", candidate.body(c))
        self.assertIn("bots-smoke", candidate.body(c))
        # a green job with a red (or missing) report is a failure
        self.assertEqual(candidate.candidate(self.PINS, "success", {"ok": False}, None)["smoke"], "failed")
        self.assertEqual(candidate.candidate(self.PINS, "success", None, None)["smoke"], "failed")
        self.assertIn("do not merge", candidate.body(candidate.candidate(self.PINS, "failure")))

    def test_players_never_see_upstream_words(self):
        c = candidate.candidate(self.PINS, "success", {"ok": True, "checks": []}, None)
        self.assertEqual(set(c), {"format", "playerbots", "core", "changes", "images", "tested", "smoke", "checks", "compat", "run"})


class HostPack(unittest.TestCase):
    """The pins travel with the Host pack: realm-files/bots.json in the pack, `bots` in hostpack.json and in the site's
    all.json project (stream/leyline_net.py), which the launcher's host/bots.rs reads."""

    def test_pack_and_listing_carry_the_pins(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location("make_hostpack", os.path.join(REPO, "hosting", "pack", "make-hostpack.py"))
        mh = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mh)
        out = tempfile.mkdtemp()
        try:
            from pathlib import Path
            made = mh.build(Path(out), "2026.1104.1200")
            m = json.load(open(os.path.join(REPO, "realm", "bots.json")))
            self.assertEqual(made["desc"]["bots"], {k: m[k] for k in ("playerbots", "core", "date", "changes") if k in m})
            import zipfile
            names = zipfile.ZipFile(made["zip"]).namelist()
            self.assertTrue(any(n.endswith("realm-files/bots.json") for n in names), names[:5])
        finally:
            shutil.rmtree(out, ignore_errors=True)
        sys.path.insert(0, os.path.join(REPO, "stream"))
        import leyline_net
        d = {"name": "Host pack", "version": "2026.1104.1200", "download": "https://x/y.zip", "sha256": "a" * 64,
             "bots": {"playerbots": "d" * 40, "core": "e" * 40, "date": "2026-11-10"}}
        self.assertEqual(leyline_net.hostpack_project(d)["bots"], d["bots"])
        d.pop("bots")
        self.assertNotIn("bots", leyline_net.hostpack_project(d))


class Packets(unittest.TestCase):
    def test_say(self):
        b = bs.say_packet(".recruit party")
        self.assertEqual(struct.unpack_from("<II", b), (bs.CHAT_MSG_SAY, bs.LANG_COMMON))
        self.assertEqual(b[8:], b".recruit party\x00")

    def test_system_chat(self):
        msg = 'OVSMOKE {"t":"report","members":[]}'
        body = bytes([0]) + struct.pack("<IQIQI", 0, 0, 0, 0, len(msg) + 1) + msg.encode() + b"\x00\x00"
        self.assertEqual(bs.parse_chat(body), (0, msg))
        self.assertIsNone(bs.parse_chat(bytes([1]) + body[1:]))        # a say line, not the system's
        self.assertIsNone(bs.parse_chat(body[:10]))
        self.assertEqual(bs.smoke_lines(msg + " junk OVSMOKE {bad"), [{"t": "report", "members": []}])

    def test_packed_guid_and_teleport_ack(self):
        for g in (0, 1, 0x1234, 0x0100000000000007, (1 << 64) - 1):
            enc = bs.packed_guid(g)
            self.assertEqual(bs.read_packed_guid(enc), (g, len(enc)))
        server = bs.packed_guid(0x55) + struct.pack("<I", 9) + b"\x00" * 30
        self.assertEqual(bs.teleport_ack(server, 123456), bs.packed_guid(0x55) + struct.pack("<II", 9, 123456))

    def test_dungeon_door_from_the_companions_table(self):
        self.assertEqual(bs.dungeon_door(REPO, "deadmines"), (36, -16.4, -383.07, 61.78, 1.86))
        with self.assertRaises(ValueError):
            bs.dungeon_door(REPO, "nowhere")

    def test_judges(self):
        me = {"name": "Me", "map": 36, "instance": 5}
        bot = lambda n, **k: dict({"name": n, "bot": True, "map": 36, "instance": 5, "alive": True, "master": "Me", "dist": 3}, **k)  # noqa: E731
        ok, d = bs.judge_recruit({"me": me, "members": [me, bot("A"), bot("B"), bot("C")]})
        self.assertTrue(ok, d)
        ok, d = bs.judge_recruit({"me": me, "members": [me, bot("A"), bot("B"), bot("C", instance=6)]})
        self.assertFalse(ok)
        self.assertIn("not with us: C", d)
        self.assertFalse(bs.judge_recruit({"me": me, "members": [me, bot("A")]})[0])
        self.assertFalse(bs.judge_recruit({"me": me, "members": [me, bot("A"), bot("B"), bot("C", alive=False)]})[0])
        self.assertTrue(bs.judge_follow({"members": [bot("A", dist=5)]}, 12)[0])
        self.assertFalse(bs.judge_follow({"members": [bot("A", dist=30)]}, 12)[0])
        self.assertFalse(bs.judge_follow({"members": []}, 12)[0])
        self.assertTrue(bs.judge_fight([{"alive": True, "hp": 80, "bots_in_combat": ["A"]}, {"alive": False, "bots_in_combat": []}])[0])
        self.assertFalse(bs.judge_fight([{"alive": False, "bots_in_combat": []}])[0])       # died, but no bot fought
        self.assertFalse(bs.judge_fight([{"alive": True, "hp": 50, "bots_in_combat": ["A"]}])[0])
        self.assertFalse(bs.judge_fight([])[0])
        self.assertTrue(bs.moved_to({"me": {"x": 1, "y": 1}}, (2, 2, 0), 5))
        self.assertFalse(bs.moved_to({"me": {"x": 1, "y": 1}}, (20, 2, 0), 5))
        self.assertFalse(bs.moved_to({"me": {}}, (2, 2, 0), 5))

    def test_admin_sql(self):
        t = tempfile.mkdtemp()
        try:
            creds = os.path.join(t, "creds")
            sql = bs.admin_sql(creds)
            self.assertEqual(stat.S_IMODE(os.stat(creds).st_mode), 0o600)
            got = dict(l.split("=", 1) for l in open(creds).read().split())
            self.assertEqual(got["SMOKE_SOAP_USER"], "SMOKEADMIN")
            self.assertGreaterEqual(len(got["SMOKE_SOAP_PASS"]), 16)
            self.assertNotIn(got["SMOKE_SOAP_PASS"], sql)                    # only the verifier goes to the database
            self.assertIn("INSERT INTO account (username, salt, verifier, expansion)", sql)
            self.assertIn("account_access (AccountID, SecurityLevel, RealmID)", sql)
            self.assertIn("account_access (id, gmlevel, RealmID)", sql)
        finally:
            shutil.rmtree(t)


# ---------------------------------------------------------------------------- the whole player against fakes
def system_line(text):
    raw = text.encode()
    return bytes([0]) + struct.pack("<IQIQI", 0, 0, 0, 0, len(raw) + 1) + raw + b"\x00\x00"


class BotsWorld:
    """What the realm does, as far as the smoke test can see: mod-playerbots' commands, ov_companions.lua's, the
    probe's answers, far and near teleports that wait for the client's ack. `broken` switches parts off."""

    def __init__(self, broken=()):
        self.broken = set(broken)
        self.me = {"name": None, "bot": False, "map": 0, "instance": 0, "alive": True, "x": -8949.95, "y": -132.49, "z": 83.53, "o": 0.0}
        self.bots = []
        self.pending = None          # ("far", map, x, y, z, o) or ("near", counter, x, y, z)
        self.counter = 0
        self.mob = None
        self.said = []

    def answer(self, send, t, **data):
        data["t"] = t
        send(bs.SMSG_MESSAGECHAT, system_line("OVSMOKE " + json.dumps(data)))

    def members(self):
        out = [dict(self.me, dist=0)]
        for b in self.bots:
            d = ((b["x"] - self.me["x"]) ** 2 + (b["y"] - self.me["y"]) ** 2) ** 0.5
            out.append(dict(b, dist=round(d, 1)))
        return out

    def bot(self, name):
        return {"name": name, "bot": True, "map": self.me["map"], "instance": self.me["instance"], "alive": True,
                "master": self.me["name"], "x": self.me["x"] + 2, "y": self.me["y"], "z": self.me["z"], "o": 0}

    def __call__(self, user, op, body, send):
        if op == bs.MSG_MOVE_WORLDPORT_ACK and self.pending and self.pending[0] == "far":
            _, mp, x, y, z, o = self.pending
            self.me.update(map=mp, instance=7, x=x, y=y, z=z, o=o)
            self.pending = None
            return
        if op == bs.MSG_MOVE_TELEPORT_ACK and self.pending and self.pending[0] == "near":
            g, p = bs.read_packed_guid(body)
            (counter,) = struct.unpack_from("<I", body, p)
            if counter == self.pending[1]:
                _, _, x, y, z = self.pending
                self.me.update(x=x, y=y, z=z)
                if "follow" not in self.broken:
                    for b in self.bots:
                        b.update(x=x + 1.5, y=y, z=z)
                self.pending = None
            return
        if op != bs.CMSG_MESSAGECHAT:
            return
        typ, lang = struct.unpack_from("<II", body)
        if typ != bs.CHAT_MSG_SAY or lang != bs.LANG_COMMON:
            raise ValueError("chat must be a /say in Common")
        text = body[8:].rstrip(b"\x00").decode()
        self.said.append(text)
        w = text.split()
        if text.startswith(".playerbots bot addclass ") and "addclass" not in self.broken:
            self.bots.append(self.bot("Rogbot"))
        elif text.startswith(".playerbots bot remove "):
            self.bots = [b for b in self.bots if b["name"] != w[3]]
        elif w[:2] == [".go", "xyz"]:
            x, y, z = map(float, w[2:5])
            if len(w) >= 6:
                self.pending = ("far", int(w[5]), x, y, z, float(w[6]) if len(w) > 6 else 0.0)
                send(bs.SMSG_NEW_WORLD, struct.pack("<Iffff", int(w[5]), x, y, z, 0.0))
            else:
                self.counter += 1
                self.pending = ("near", self.counter, x, y, z)
                send(bs.MSG_MOVE_TELEPORT_ACK, bs.packed_guid(77) + struct.pack("<I", self.counter) + bytes(30))
        elif text == ".recruit party" and "recruit" not in self.broken:
            self.bots += [self.bot(n) for n in ("Tankbot", "Healbot", "Dpsbot", "Dpsbottwo")]
        elif text == ".dismiss all":
            self.bots = []
        elif w[:2] == [".ovsmoke", "report"]:
            self.answer(send, "report", me=dict(self.me), grouped=bool(self.bots), raid=False, members=self.members())
        elif w[:2] == [".ovsmoke", "compat"]:
            names = w[2].split(",")
            gone = ["GetBotAI"] if "methods" in self.broken else []
            self.answer(send, "compat", bot=self.bots[0]["name"], ai=True, ok=[n for n in names if n not in gone], missing=gone)
        elif w[:2] == [".ovsmoke", "pull"]:
            self.mob = {"hp": 100, "looks": 0}
            self.answer(send, "pull", entry=int(w[2]), name="Defias Thug", level=10)
        elif w[:2] == [".ovsmoke", "fight"]:
            m = self.mob
            m["looks"] += 1
            fighting = "fight" not in self.broken
            if fighting:
                m["hp"] = max(0, m["hp"] - 60)
            self.answer(send, "fight", alive=m["hp"] > 0, hp=m["hp"], bots_in_combat=[b["name"] for b in self.bots] if fighting else [])


class FakeSoap(http.server.BaseHTTPRequestHandler):
    realm = None
    login = ("", "")
    commands = []

    def do_POST(self):  # noqa: N802
        body = self.rfile.read(int(self.headers.get("Content-Length", 0))).decode()
        want = "Basic " + base64.b64encode(":".join(self.login).encode()).decode()
        if self.headers.get("Authorization") != want:
            self.send_response(401)
            self.end_headers()
            return
        cmd = re.search(r"<command>(.*?)</command>", body, re.S).group(1)
        type(self).commands.append(cmd)
        w = cmd.split()
        if w[:2] == ["account", "create"]:
            self.realm.add_account(w[2], w[3])
        out = f"<?xml version='1.0'?><SOAP-ENV:Envelope><SOAP-ENV:Body><ns1:executeCommandResponse><result>ok: {w[0]}</result></ns1:executeCommandResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"
        self.send_response(200)
        self.send_header("Content-Type", "text/xml")
        self.end_headers()
        self.wfile.write(out.encode())

    def log_message(self, *a):
        pass


class SmokeRun(unittest.TestCase):
    def start(self, broken=()):
        self.loop = asyncio.new_event_loop()
        self.realm = FakeRealm()
        self.world = BotsWorld(broken)
        self.realm.on_packet = self.world
        ready = threading.Event()

        def serve():
            asyncio.set_event_loop(self.loop)
            self.loop.run_until_complete(self.realm.start())
            ready.set()
            self.loop.run_forever()
        self.t = threading.Thread(target=serve, daemon=True)
        self.t.start()
        ready.wait(10)
        FakeSoap.realm, FakeSoap.login, FakeSoap.commands = self.realm, ("SMOKEADMIN", "s3cretADMINpass"), []
        self.soap = http.server.ThreadingHTTPServer(("127.0.0.1", 0), FakeSoap)
        threading.Thread(target=self.soap.serve_forever, daemon=True).start()
        self.realm.world_address = f"127.0.0.1:{self.realm.world_port}"

    def tearDown(self):
        self.soap.shutdown()
        self.loop.call_soon_threadsafe(self.loop.stop)
        self.t.join(5)

    def run_player(self, *extra):
        out = os.path.join(tempfile.mkdtemp(), "r.json")
        env = dict(os.environ, SMOKE_SOAP_USER="SMOKEADMIN", SMOKE_SOAP_PASS="s3cretADMINpass")
        p = subprocess.run([sys.executable, "-I", os.path.join(BOTS, "bots_smoke.py"), "run",
                            "--auth", f"127.0.0.1:{self.realm.auth_port}", "--world", f"127.0.0.1:{self.realm.world_port}",
                            "--soap", f"127.0.0.1:{self.soap.server_address[1]}", "--poll", "0.2", "--wait", "10",
                            "--bot-wait", "3", "--fight-wait", "4", "--out", out, *extra],
                           env=env, capture_output=True, text=True, timeout=180)
        rep = json.load(open(out)) if os.path.exists(out) else {}
        return p.returncode, p.stdout + p.stderr, rep

    def test_everything_passes(self):
        self.start()
        rc, out, rep = self.run_player()
        self.assertEqual(rc, 0, out)
        self.assertIn("BOTS SMOKE OK", out)
        names = [c["name"] for c in rep["checks"]]
        self.assertEqual(names, ["console", "account", "character", "world", "addclass", "remove", "dungeon", "recruit",
                                 "compat_live", "follow", "pull", "fight", "dismiss"])
        self.assertTrue(all(c["ok"] for c in rep["checks"]), rep)
        # the console made the account and the GM; the password never shows
        made = [c for c in FakeSoap.commands if c.startswith("account create ")]
        self.assertEqual(len(made), 1)
        pw = made[0].split()[3]
        self.assertNotIn(pw, out)
        self.assertNotIn(pw, json.dumps(rep))
        self.assertTrue(any(c.startswith("account set gmlevel ") and c.endswith(" 3 -1") for c in FakeSoap.commands))
        self.assertNotIn("s3cretADMINpass", out)
        # what the player typed: playerbots' own commands, then the script's; the dungeon door from its table
        said = self.world.said
        self.assertIn(".playerbots bot addclass rogue", said)
        self.assertIn(".playerbots bot remove Rogbot", said)
        self.assertIn(".go xyz -16.4 -383.07 61.78 36 1.86", said)
        self.assertLess(said.index(".go xyz -16.4 -383.07 61.78 36 1.86"), said.index(".recruit party"))
        self.assertIn(".dismiss all", said)
        compat_line = next(s for s in said if s.startswith(".ovsmoke compat "))
        self.assertEqual(sorted(compat_line.split()[2].split(",")), sorted(compat.REQUIRED_METHODS))

    def test_breaking_changes_fail(self):
        self.start(broken=("recruit",))
        rc, out, rep = self.run_player()
        self.assertEqual(rc, 1)
        got = {c["name"]: c["ok"] for c in rep["checks"]}
        self.assertFalse(got["recruit"])
        self.assertNotIn("fight", got)                 # nothing to fight with

    def test_a_missing_method_and_no_fight_fail(self):
        self.start(broken=("methods", "fight", "follow"))
        rc, out, rep = self.run_player()
        self.assertEqual(rc, 1)
        got = {c["name"]: c for c in rep["checks"]}
        self.assertFalse(got["compat_live"]["ok"])
        self.assertIn("GetBotAI", got["compat_live"]["detail"])
        self.assertFalse(got["follow"]["ok"])
        self.assertFalse(got["fight"]["ok"])
        self.assertTrue(got["dismiss"]["ok"])

    def test_static_result_is_in_the_report(self):
        self.start()
        st = os.path.join(tempfile.mkdtemp(), "static.json")
        json.dump({"ok": False, "missing": [{"kind": "whispers", "name": "summon", "fatal": True}]}, open(st, "w"))
        rc, out, rep = self.run_player("--static", st)
        self.assertEqual(rc, 1)
        self.assertEqual(rep["checks"][0], {"name": "compat_static", "ok": False, "detail": "whispers summon"})


class Smoke(unittest.TestCase):
    def test_compose(self):
        try:
            import yaml
        except ImportError:
            self.skipTest("PyYAML")
        doc = yaml.safe_load(open(os.path.join(BOTS, "docker-compose.smoke.yml")))
        ws = doc["services"]["ac-worldserver"]
        self.assertIn("client-data:/azerothcore/env/dist/data:ro", ws["volumes"])
        self.assertTrue(doc["volumes"]["client-data"]["external"])
        for s in doc["services"].values():
            for p in s.get("ports", []):
                self.assertTrue(p.startswith("127.0.0.1:"), p)
        env = ws["environment"]
        self.assertEqual(env["AC_SOAP_ENABLED"], "1")
        self.assertEqual(env["AC_PLAYERBOTS_MAX_RANDOM_BOTS"], "0")
        self.assertEqual(env["OV_COMPANIONS_COOLDOWN"], "0")
        self.assertEqual([e["path"] for e in ws["env_file"]], ["../profiles/solo.env", "../modules.env"])   # the Single player ruleset
        # the database password is only ever bots-smoke.sh's random one
        self.assertTrue(doc["services"]["ac-database"]["environment"]["MYSQL_ROOT_PASSWORD"].startswith("${SMOKE_DB_PASSWORD:?"))
        self.assertNotRegex(json.dumps(doc), r"root;(?!\$\{SMOKE_DB_PASSWORD)")

    def test_script_needs_the_owners_own_data(self):
        t = tempfile.mkdtemp()
        try:
            put(os.path.join(t, "docker"), "#!/bin/sh\necho \"$*\" >> \"$LOG\"\n[ \"$1 $2\" = \"volume inspect\" ] && exit 1\nexit 0\n")
            os.chmod(os.path.join(t, "docker"), 0o755)
            log = os.path.join(t, "log")
            p = subprocess.run(["bash", os.path.join(BOTS, "bots-smoke.sh")], capture_output=True, text=True, timeout=30,
                               env=dict(os.environ, PATH=t + os.pathsep + os.environ["PATH"], LOG=log))
            self.assertEqual(p.returncode, 3, p.stdout + p.stderr)
            self.assertIn("extracted from your own game folder", p.stdout)
            calls = open(log).read().strip().splitlines()
            self.assertEqual(calls, ["volume inspect ov-client-data"])      # nothing started
        finally:
            shutil.rmtree(t)

    def test_smoke_prepares_only_what_the_bots_need(self):
        t = tempfile.mkdtemp()
        try:
            env = bs.prepare(t, REPO, "ghcr.io/orionopenwow/wu-ac-wotlk", "canary", "ov-client-data")
            self.assertEqual(sorted(os.listdir(os.path.join(t, "lua"))), sorted(list(bs.LUA_FILES) + ["ov_bots_probe.lua"]))
            self.assertEqual(stat.S_IMODE(os.stat(os.path.join(t, ".env")).st_mode), 0o600)
            self.assertEqual(len(env["SMOKE_DB_PASSWORD"]), 32)
        finally:
            shutil.rmtree(t)
        self.assertEqual(subprocess.run(["bash", "-n", os.path.join(BOTS, "bots-smoke.sh")]).returncode, 0)
        self.assertIn("bots_smoke.py\" smoke", open(os.path.join(BOTS, "bots-smoke.sh")).read())
        bat = open(os.path.join(REPO, "realm", "Bots smoke test.bat"), newline="").read()
        self.assertIn("\r\n", bat)
        self.assertIn("bots_smoke.py\" smoke --tag", bat)
        self.assertEqual(bs.main(["smoke", "--tag", "x;rm"]), 2)
        # the probe is test only: never in the realm scripts the kit ships
        self.assertFalse(os.path.exists(os.path.join(REPO, "realm", "custom", "lua_scripts", "ov_bots_probe.lua")))

    def test_probe_lua(self):
        lua = shutil.which("lua5.1") or shutil.which("texlua") or shutil.which("lua")
        if not lua:
            self.skipTest("no Lua here")
        p = subprocess.run([lua, os.path.join(BOTS, "tests", "test_ov_bots_probe.lua"), os.path.join(BOTS, "smoke", "ov_bots_probe.lua")],
                           capture_output=True, text=True, timeout=60)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertNotIn("FAIL", p.stdout)
        self.assertIn("PASS", p.stdout)


if __name__ == "__main__":
    unittest.main()
