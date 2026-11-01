"""The Leyline network for hosters (docs/leyline/join-network.md, docs/leyline/be-a-node.md):

  * the realm list's new fields for the in-game hop window: build, online, who (opt-in, filtered, never GMs or bots,
    at most 50), rules (short, word-filtered), mods {id, name, required, client}, herokey, uptime_pct; fed by the
    agent's signed heartbeat, the same in the mesh's view;
  * Hero Key for an existing server: join-network (database settings kept 0600, never printed; AzerothCore and
    TrinityCore only; the website's key pinned once), the agent takes Hero Key sign-ins on a linked server that gave it
    its login database, and the self-test (a dry run that leaves nothing behind);
  * community nodes: registration with a signed node record, health checks, a ban and the kill switch from HQ's feed,
    the node pausing itself (the owner's pause file, the team's ban), its status file, the mesh refusing a banned node.
"""
import asyncio
import contextlib
import copy
import io
import json
import os
import stat
import tempfile
import time
import types
import unittest
from contextlib import redirect_stderr, redirect_stdout

from helpers import FakeDB, FakeRealm, agentmod, make_host_state, relaymod  # noqa: F401 (sets sys.path)
from wunet import herokey, keys, mesh
from wurelay import listing, meshnode, nodes


def lcfg(**kw):
    d = tempfile.mkdtemp()
    base = {"state_file": os.path.join(d, "state.json"), "name": "relay.test", "join_base": None, "region": "",
            "advertise_direct": False}
    base.update(kw)
    return types.SimpleNamespace(**base)


class Host:
    def __init__(self):
        self.sec = keys.new_secret()
        self.pub = keys.public_key(self.sec)
        self.rid = keys.realm_id(self.pub)
        self.seq = 0

    def beat(self, now, record=None, status=None, **top):
        self.seq += 1
        st = {"online": True, "players": 3, "bots": 0, "uptime_s": 10}
        st.update(status or {})
        b = {"v": 1, "realm_id": self.rid, "pubkey": self.pub.hex(), "seq": self.seq, "ts": int(now),
             "record": record or {"name": "Frost Haven"}, "status": st}
        b.update(top)
        return {"beat": b, "sig": keys.sign_doc(self.sec, b, keys.CTX_HEARTBEAT)}


class ListingFields(unittest.TestCase):
    """The contract with the in-game hop window (WrathUnleashedUI Leyline.lua)."""

    def one(self, **kw):
        L, h, now = listing.Listing(lcfg()), Host(), time.time()
        st, body = L.heartbeat(h.beat(now, **kw), "198.51.100.1", now=now)
        self.assertEqual(st, 200, body)
        return L.realms_json(now)["realms"][0]

    def test_defaults_without_the_new_beat_fields(self):
        r = self.one()
        self.assertEqual((r["build"], r["online"], r["who"], r["herokey"], r["uptime_pct"]), (12340, 3, [], False, None))
        self.assertEqual((r["rules"], r["rulesText"], r["mods"]), ([], "", []))

    def test_who_only_when_sent_filtered_and_capped(self):
        names = ["Grumpy", "Aelyn", "Grumpy", "x", "Has Space", "Blizzard", "Nigger", "<b>Bold</b>", 7] + \
                [f"Name{chr(97 + i % 26)}{chr(97 + i // 26)}" for i in range(80)]
        names = [n.replace("0", "o") if isinstance(n, str) else n for n in names]
        r = self.one(status={"who": names})
        self.assertIn("Grumpy", r["who"])
        self.assertIn("Aelyn", r["who"])
        self.assertEqual(r["who"].count("Grumpy"), 1)
        for bad in ("x", "Has Space", "Blizzard", "Nigger", "<b>Bold</b>"):
            self.assertNotIn(bad, r["who"])
        self.assertLessEqual(len(r["who"]), 50)
        self.assertEqual(len(r["who"]), 50)

    def test_who_empty_while_offline(self):
        r = self.one(status={"online": False, "who": ["Grumpy"]})
        self.assertEqual((r["online"], r["who"]), (0, []))

    def test_rules_text_mods_herokey_build(self):
        rules = ["Be kind.", "No griefing in Dalaran.", "World of Warcraft rules apply."] + ["x" * 120] * 7
        r = self.one(record={"name": "Frost Haven", "build": 12340, "rules": rules, "mods": [
            {"id": "up:skada", "name": "Skada meter", "required": True},
            {"id": "mod:dalaran-dice", "name": "Dice", "required": False, "client": True},
            {"id": "mode:minigolf", "name": "Minigolf", "client": True}, {"id": "https://evil.example/x.zip"}]},
            herokey=True)
        self.assertEqual(r["build"], 12340)
        self.assertNotIn("World of Warcraft", r["rulesText"])
        self.assertLessEqual(len(r["rulesText"]), listing.RULES_TEXT_MAX)
        self.assertTrue(r["rulesText"].startswith("Be kind. No griefing in Dalaran."))
        self.assertEqual(r["mods"], [
            {"id": "up:skada", "name": "Skada meter", "required": True, "client": True},
            {"id": "mod:dalaran-dice", "name": "Dice", "required": False, "client": True},
            {"id": "mode:minigolf", "name": "Minigolf", "client": False}])
        self.assertTrue(r["herokey"], "no relay here: the realm's own signed beat says so")

    def test_rude_rule_starred(self):
        r = self.one(record={"name": "Frost Haven", "rules": ["no shit talk"]})
        self.assertNotIn("shit", r["rulesText"])

    def test_the_relay_s_word_wins_for_herokey(self):
        """With a relay, Hero Key comes from the agent's hello (the relay routes the sign-ins), not the beat."""
        L, h, now = listing.Listing(lcfg()), Host(), time.time()
        L.heartbeat(h.beat(now, herokey=True), "198.51.100.1", now=now)
        L.relay = types.SimpleNamespace(sessions={h.rid: types.SimpleNamespace(herokey=False, streams={},
                                                                                visibility="public")},
                                        blocked=set(), state={"realms": {}})
        self.assertFalse(L.realms_json(now)["realms"][0]["herokey"])

    def test_mesh_view_has_the_same_fields(self):
        h, now = Host(), int(time.time())
        b = h.beat(now, record={"name": "Frost Haven", "rules": ["Be kind."]}, status={"who": ["Grumpy"]}, herokey=True)
        sec = keys.new_secret()
        ep = mesh.endpoint(sec, h.rid, "relay.test", 40000, 40001, True, now)
        b["beat"]["relay"] = "relay.test"
        b["sig"] = keys.sign_doc(h.sec, b["beat"], keys.CTX_HEARTBEAT)
        v = meshnode.record_view(mesh.realm_record(b["beat"], b["sig"], [ep]), ep, now)
        for k in ("build", "online", "who", "herokey", "uptime_pct", "rulesText", "mods"):
            self.assertIn(k, v)
        self.assertEqual((v["who"], v["herokey"], v["rulesText"]), (["Grumpy"], True, "Be kind."))


class WhoDB(FakeDB):
    def __init__(self, realm):
        super().__init__(realm)
        self.who_calls = 0

    def who(self, limit=50):
        self.who_calls += 1
        return ["Grumpy", "Aelyn", "bad name!"]


class AgentFields(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="wu-jn-")

    def agent(self, extra=None, db=True):
        make_host_state(self.tmp, extra=extra)
        a = agentmod.Agent(self.tmp, db=WhoDB(FakeRealm()) if db else None)
        a.world_up = self._up
        return a

    async def _up(self):
        return True

    async def test_who_only_when_the_host_opted_in(self):
        a = self.agent()
        st = await a.status()
        self.assertNotIn("who", st)
        self.assertNotIn("who", a.beat(st)["beat"]["status"])
        a = self.agent({"share_names": True})
        st = await a.status()
        self.assertEqual(st["who"], ["Grumpy", "Aelyn"])
        b = a.beat(st)
        self.assertEqual(b["beat"]["status"]["who"], ["Grumpy", "Aelyn"])
        self.assertTrue(keys.verify_doc(a.pub, b["beat"], keys.CTX_HEARTBEAT, b["sig"]), "who is inside the signature")

    async def test_herokey_on_a_linked_server_with_its_database(self):
        pub = keys.public_key(keys.new_secret())
        a = self.agent({"linked": True, "herokey": True})
        self.assertFalse(a.herokey_on(), "no key pinned yet")
        herokey.pin_state_keys(self.tmp, [pub.hex()], source="https://site.test/wrath/leyline/herokey.json")
        self.assertTrue(a.herokey_on())
        self.assertTrue(a.beat(await a.status())["beat"].get("herokey"))
        self.assertFalse(self.agent({"linked": True, "herokey": True}, db=False).herokey_on(), "no database: no accounts")
        self.assertFalse(self.agent({"official": True, "herokey": True}).herokey_on(), "the operator's own realm")

    def test_arrivals_are_counts_only(self):
        a = self.agent()
        now = time.time()
        for t in (now - 90000, now - 7000, now - 10, now - 5):
            a.note_arrival(t)
        self.assertEqual(a.arrival_counts(now), {"hour": 2, "day": 3})


class PinnedKeys(unittest.TestCase):
    def test_state_file_pins_and_is_checked(self):
        d = tempfile.mkdtemp()
        self.assertEqual(herokey.pinned_keys(env={}, state_dir=d), {})
        good = keys.public_key(keys.new_secret()).hex()
        with self.assertRaises(ValueError):
            herokey.pin_state_keys(d, ["zz", "00" * 31])
        kids = herokey.pin_state_keys(d, [good, good, "junk"])
        self.assertEqual(kids, [herokey.key_id(bytes.fromhex(good))])
        self.assertEqual(set(herokey.pinned_keys(env={}, state_dir=d)), set(kids))


class DryDB:
    """A login database for the self-test: create_account/get_login, and dry_run() that undoes everything."""

    def __init__(self, build=12340, can_write=True):
        self.accounts, self.build, self.can_write = {}, build, can_write

    def probe(self):
        return {"build": self.build, "realm_name": "Frost Haven", "accounts": len(self.accounts),
                "can_write": self.can_write}

    def create_account(self, username, salt, verifier, gmlevel=0):
        if username in self.accounts:
            return "taken"
        self.accounts[username] = (salt, verifier)
        return "ok"

    def get_login(self, username):
        return self.accounts.get(username)

    @contextlib.contextmanager
    def dry_run(self):
        keep = copy.deepcopy(self.accounts)
        try:
            yield self
        finally:
            self.accounts = keep


class SelfTest(unittest.TestCase):
    def setUp(self):
        from wuhost import hktest
        self.hk = hktest
        self.tmp = tempfile.mkdtemp(prefix="wu-hk-")

    def run_it(self, db, extra=None, relay_says=True):
        cfg, _ = make_host_state(self.tmp, extra=dict({"linked": True, "herokey": True, "relay": "relay.test"},
                                                      **(extra or {})))
        lines = []
        rows = self.hk.run(self.tmp, db, get_json=lambda u: {"herokey": relay_says}, out=lines.append)
        return {r[0]: r[1] for r in rows}, "\n".join(lines)

    def test_all_steps_pass_and_nothing_stays(self):
        herokey.pin_state_keys(self.tmp, [keys.public_key(keys.new_secret()).hex()])
        db = DryDB()
        ok, text = self.run_it(db)
        self.assertEqual(ok, {"on": True, "key": True, "db": True, "assertion": True, "account": True, "relay": True}, text)
        self.assertEqual(db.accounts, {}, "the dry run left no account behind")
        self.assertNotIn("PROBLEM", text)

    def test_each_problem_has_a_plain_line(self):
        ok, text = self.run_it(DryDB(build=5875, can_write=False), extra={"herokey": False}, relay_says=False)
        self.assertFalse(ok["on"] or ok["key"] or ok["db"] or ok["relay"])
        self.assertTrue(ok["assertion"], "the check itself still behaves")
        self.assertIn("another client", text)
        self.assertIn("INSERT and UPDATE", text)
        ok, text = self.run_it(None)
        self.assertFalse(ok["db"])
        self.assertIn("no database settings", text)

    def test_srp_check_is_real(self):
        from wunet import srp6
        salt, ver = srp6.make_verifier("HKABCDEFGHIJKLMN", "secret1")
        self.assertTrue(self.hk.srp_login_ok("HKABCDEFGHIJKLMN", "secret1", salt.hex(), ver.hex()))
        self.assertFalse(self.hk.srp_login_ok("HKABCDEFGHIJKLMN", "wrong", salt.hex(), ver.hex()))


class JoinNetwork(unittest.TestCase):
    def setUp(self):
        self.kit = tempfile.mkdtemp(prefix="wu-kit-")
        os.environ["WU_KIT"] = self.kit
        import importlib
        from wuhost import setup
        self.setup = importlib.reload(setup)

    def run_cmd(self, *argv, env=None):
        out, err = io.StringIO(), io.StringIO()
        old = dict(os.environ)
        os.environ.update(env or {})
        try:
            with redirect_stdout(out), redirect_stderr(err):
                self.setup.main(list(argv))
        finally:
            os.environ.clear()
            os.environ.update(old)
        return out.getvalue(), err.getvalue()

    def join(self, *extra, pin=None):
        self.setup.pin_site_key = lambda site, get=None: pin or []
        return self.run_cmd("join-network", "--name", "Frost Haven", "--core", "azerothcore", "--auth-service",
                            "127.0.0.1:3724", "--world-service", "127.0.0.1:8085", "--db-host", "127.0.0.1",
                            "--db-user", "leyline", "--skip-check", *extra,
                            env={"WU_JOIN_DB_PASSWORD": "pw-not-to-be-printed"})

    def test_join_writes_the_database_settings_privately(self):
        out, err = self.join(pin=["abcdef0123456789"])
        st = os.path.join(self.kit, "state")
        cfg = json.load(open(os.path.join(st, "host.json")))
        self.assertTrue(cfg["linked"] and cfg["herokey"])
        self.assertEqual((cfg["record"]["build"], cfg["record"]["core"]), (12340, "azerothcore"))
        envp = os.path.join(st, "link-db.env")
        self.assertEqual(stat.S_IMODE(os.stat(envp).st_mode), 0o600)
        env = open(envp).read()
        self.assertIn("DB_PASSWORD=pw-not-to-be-printed\n", env)
        self.assertIn("WU_DB_AUTH=acore_auth\n", env)
        self.assertIn("WU_EXPANSION=2\n", env)
        self.assertNotIn("pw-not-to-be-printed", out + err)
        self.assertNotIn("pw-not-to-be-printed", json.dumps(cfg))
        self.assertIn("pinned (abcdef0123456789)", err)

    def test_trinitycore_names_and_not_yet_cores(self):
        self.setup.pin_site_key = lambda site, get=None: []
        self.run_cmd("join-network", "--name", "Frost Haven", "--core", "trinitycore", "--auth-service", "127.0.0.1:3724",
                     "--world-service", "127.0.0.1:8085", "--db-host", "db.example", "--skip-check")
        env = open(os.path.join(self.kit, "state", "link-db.env")).read()
        self.assertIn("WU_CORE=trinitycore\nWU_DB_AUTH=auth\nWU_DB_CHARS=characters\n", env)
        for core in ("cmangos", "vmangos"):
            with self.assertRaises(SystemExit) as c:
                self.run_cmd("join-network", "--core", core, "--db-host", "x")
            self.assertIn("not yet", str(c.exception))

    def test_pin_from_the_site_and_status(self):
        pub = keys.public_key(keys.new_secret()).hex()
        self.join()
        os.makedirs(os.path.join(self.kit, "state"), exist_ok=True)
        import importlib
        from wuhost import setup
        self.setup = importlib.reload(setup)
        kids = self.setup.pin_site_key("https://site.test", get=lambda u: {"keys": [{"pub": pub, "use": "current"}]})
        self.assertEqual(kids, [herokey.key_id(bytes.fromhex(pub))])
        self.assertEqual(self.setup.pin_site_key("https://site.test", get=lambda u: None), [], "flag off: 404")
        out, _ = self.run_cmd("herokey", "status")
        self.assertEqual(json.loads(out), {"on": True, "pinned": kids, "database": True})

    def test_share_names_and_mods_set(self):
        self.join()
        self.run_cmd("share-names", "on")
        self.assertTrue(json.load(open(os.path.join(self.kit, "state", "host.json")))["share_names"])
        self.run_cmd("mods", "set", "--json", json.dumps([{"id": "up:skada", "name": "Skada", "required": True,
                                                          "client": True}, {"id": "mod:pings"}]))
        mods = json.load(open(os.path.join(self.kit, "state", "host.json")))["record"]["mods"]
        self.assertEqual(mods, [{"id": "up:skada", "name": "Skada", "required": True, "client": True}, {"id": "mod:pings"}])
        with self.assertRaises(SystemExit):
            self.run_cmd("mods", "set", "--json", json.dumps([{"id": "https://evil.example/x.zip"}]))


# ------------------------------------------------------------------------------------------------ community nodes
def node_rec(sec=None, host="node.example.org", volunteer=True, ts=None):
    sec = sec or keys.new_secret()
    ts = int(time.time() if ts is None else ts)
    return sec, mesh.relay_record(sec, host, ts * 1000, ts, volunteer=volunteer, caps={"realms": 25, "streams": 64},
                                  load={"realms": 1, "streams": 4})


class Registry(unittest.TestCase):
    def setUp(self):
        self.answer = {}
        self.reg = nodes.Registry(lcfg(), fetch=self.fetch)

    def fetch(self, method, url, body=None):
        return self.answer.get(url, (0, None))

    def test_register_health_ban_kill(self):
        sec, rec = node_rec()
        st, body = self.reg.register(rec, "203.0.113.5")
        self.assertEqual((st, body["ok"], body["healthy"]), (200, True, False))
        nid = body["id"]
        self.assertEqual(self.reg.doc()["nodes"][0]["healthy"], False)
        # the node answers with the key it registered with: healthy
        self.answer["https://node.example.org/api/mesh/node"] = (200, {"node": rec})
        asyncio.run(self.reg.health_round())
        self.assertTrue(self.reg.doc()["nodes"][0]["healthy"])
        self.assertEqual(self.reg.healthy_hosts(), {"node.example.org"})
        # another key behind that name: unhealthy after three checks in a row
        _, other = node_rec(host="node.example.org")
        self.answer["https://node.example.org/api/mesh/node"] = (200, {"node": other})
        for _ in range(nodes.FAILS_UNHEALTHY):
            self.reg.check_one(self.reg.nodes[nid])
        self.assertFalse(self.reg.nodes[nid]["healthy"])
        # HQ's feed: the node is banned, then every node stopped
        self.reg.take_feed({"format": 1, "blocked": [], "nodes": [{"id": nid, "at": "x"}]})
        self.assertEqual(self.reg.doc()["nodes"], [])
        st, body = self.reg.register(rec, "203.0.113.5")
        self.assertEqual((st, body["banned"]), (403, True))
        self.reg.take_feed({"format": 1, "blocked": [], "nodes_off": True})
        st, body = self.reg.register(rec, "203.0.113.5")
        self.assertEqual((st, body["kill"]), (200, True))
        self.assertEqual((self.reg.doc()["kill"], self.reg.doc()["nodes"]), (True, []))

    def test_refusals(self):
        _, rec = node_rec(volunteer=False)
        self.assertEqual(self.reg.register(rec, "203.0.113.5")[0], 400, "our own relays don't register")
        _, rec = node_rec()
        bad = copy.deepcopy(rec)
        bad["node"]["caps"]["streams"] = 99999
        self.assertEqual(self.reg.register(bad, "203.0.113.5")[0], 400, "a changed byte")
        _, old = node_rec(ts=time.time() - 3 * 3600)
        self.assertEqual(self.reg.register(old, "203.0.113.5")[0], 400, "a stale record")
        for i in range(nodes.NEW_PER_IP_HOUR):
            self.assertEqual(self.reg.register(node_rec(host=f"n{i}.example.org")[1], "203.0.113.9")[0], 200)
        self.assertEqual(self.reg.register(node_rec(host="nx.example.org")[1], "203.0.113.9")[0], 429)


class NodeSide(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.tmp = tempfile.mkdtemp(prefix="wu-node-")
        self.ndir = os.path.join(self.tmp, "node")
        self.relay = relaymod.Relay(relaymod.Config(
            name="node.example.org", http_bind="127.0.0.1:0", game_bind="127.0.0.1", shared_auth_port=0,
            port_min=41000, port_max=41011, state_file=os.path.join(self.tmp, "relay", "state.json"), volunteer=True,
            mesh=True, mesh_bootstrap="https://seed.test", node_dir=self.ndir, total_kbps=128))
        self.answer = (200, {"ok": True, "healthy": True, "banned": False, "kill": False})
        self.relay.node_self.fetch = lambda m, u, b=None: self.answer

    async def test_pause_ban_kill_and_status(self):
        ns = self.relay.node_self
        self.assertIsNotNone(self.relay.total_bucket)
        self.assertIsNone(self.relay.nodes, "a node is not a registry")
        self.assertIsNone(ns.register_once())
        self.assertTrue(ns.registered and ns.healthy)
        fake = types.SimpleNamespace(visibility="public", streams={})
        self.relay.sessions["abcdefghijk2345a"] = fake
        self.assertIsNotNone(self.relay.admit("abcdefghijk2345a", "198.51.100.7")[0])
        # the owner's launcher pauses it (busy PC, on battery)
        os.makedirs(self.ndir, exist_ok=True)
        open(os.path.join(self.ndir, "pause"), "w").close()
        s, err = self.relay.admit("abcdefghijk2345a", "198.51.100.7")
        self.assertEqual((s, err[0]), (None, 503))
        self.assertEqual(self.relay.mesh.load()["streams"], self.relay.cfg.max_streams_total, "shows as full")
        doc = ns.write_status()
        self.assertEqual(doc["paused"], "owner")
        self.assertEqual(json.load(open(os.path.join(self.ndir, "status.json")))["paused"], "owner")
        os.remove(os.path.join(self.ndir, "pause"))
        # the team bans it, then lifts it
        self.answer = (403, {"ok": False, "banned": True, "kill": False})
        self.assertEqual(ns.register_once(), "banned")
        self.assertEqual(self.relay.admit("abcdefghijk2345a", "198.51.100.7")[1][0], 503)
        self.answer = (200, {"ok": True, "healthy": True, "banned": False, "kill": True})
        self.assertEqual(ns.register_once(), "kill")
        self.answer = (200, {"ok": True, "healthy": True, "banned": False, "kill": False})
        self.assertIsNone(ns.register_once())
        self.assertIsNotNone(self.relay.admit("abcdefghijk2345a", "198.51.100.7")[0])
        st = ns.status()
        self.assertEqual((st["players"], st["caps"]["kbps"]), (0, 128))

    async def test_seed_mesh_refuses_a_banned_node(self):
        seed = relaymod.Relay(relaymod.Config(
            name="seed.test", http_bind="127.0.0.1:0", game_bind="127.0.0.1", shared_auth_port=0, port_min=42000,
            port_max=42011, state_file=os.path.join(self.tmp, "seed", "state.json"), mesh=True, node_registry=True))
        sec, rec = node_rec()
        ok, _ = seed.mesh.accept([rec])
        self.assertTrue(ok)
        nid = mesh.node_id(rec["node"]["pubkey"])
        seed.take_feed({"format": 1, "blocked": [], "nodes": [{"id": nid, "at": "2026-10-31T00:00:00Z"}]})
        _, rec2 = node_rec(sec)
        rec2["node"]["seq"] += 5
        rec2["sig"] = keys.sign_doc(sec, rec2["node"], mesh.CTX_NODE)
        ok, why = seed.mesh.accept([rec2])
        self.assertFalse(ok)
        self.assertIn("Leyline team", why)


if __name__ == "__main__":
    unittest.main()
