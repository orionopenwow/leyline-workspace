"""Commonhold v1 (docs/COMMONHOLD-V1.md): the capability document (an `info` record), the tolerant-reader rule,
proto_min/proto_max, and the conformance vectors (launcher/src-tauri/src/p2p/commonhold-v1-vectors.json, which the
launcher's Rust tests read too)."""
import asyncio
import json
import os
import tempfile
import unittest

from helpers import Rig, free_range  # noqa: F401 (sets sys.path)
from wunet import commonhold, keys, mesh
from wurelay import meshnode
from wurelay import relay as relaymod

from test_mesh import NOW, Node, Realm, Req, cfg, realm_rec


def body(raw: bytes):
    head, _, b = raw.partition(b"\r\n\r\n")
    return int(head.split(b" ")[1]), json.loads(b or b"null")


class Vectors(unittest.TestCase):
    def test_the_file_is_what_make_writes(self):
        with open(commonhold.VECTORS, encoding="utf-8") as f:
            on_disk = json.load(f)
        self.assertEqual(commonhold.digest(on_disk), commonhold.digest(json.loads(json.dumps(commonhold.make()))),
                         "run `python3 -m wunet.commonhold make` from hosting/kit/tools after changing the vectors")

    def test_this_code_passes_every_vector(self):
        with open(commonhold.VECTORS, encoding="utf-8") as f:
            v = json.load(f)
        self.assertEqual(v["format"], "commonhold-v1-vectors")
        self.assertEqual(commonhold.run(v), [])
        names = " ".join(c["name"] for c in v["cases"])
        for want in ("tampered", "expired", "wrong relay", "equivocation", "tolerant reader", "mesh-vector.json"):
            self.assertIn(want, names)

    def test_a_broken_implementation_fails_them(self):
        with open(commonhold.VECTORS, encoding="utf-8") as f:
            v = json.load(f)
        real = mesh.check

        def strips_unknown(rec, now, staff_keys=None):
            out = real(rec, now, staff_keys)
            rec.pop("x_hint", None)   # an old node that strips what it doesn't know
            return out
        mesh.check = strips_unknown
        try:
            self.assertTrue(any("dropped x_hint" in f for f in commonhold.run(v)))
        finally:
            mesh.check = real

    def test_the_cli(self):
        self.assertEqual(commonhold.main(["check", commonhold.VECTORS]), 0)


class InfoRecords(unittest.TestCase):
    def test_signed_checked_and_merged(self):
        n = Node()
        rec = mesh.info_record(n.sec, "relay", 5, NOW, supported=["mesh", "ws-bridge", "NOT ok"], limits={"realms": 3},
                               admin="abcdefghijklmnop", languages=["en", "pt-BR", "!!"], region="eu")
        self.assertEqual(rec["info"]["supported"], ["mesh", "ws-bridge"], "bad ids are left out when making one")
        self.assertEqual(rec["info"]["languages"], ["en", "pt-BR"])
        rid, ver = mesh.check(json.loads(json.dumps(rec)), NOW + 1)
        self.assertEqual((rid, ver), ("i-" + mesh.node_id(n.pub)[2:], 5))
        st = mesh.Store()
        self.assertEqual(st.offer(rec, NOW), (True, "ok"))
        older = mesh.info_record(n.sec, "relay", 4, NOW)
        self.assertEqual(st.offer(older, NOW)[1], "older")
        self.assertEqual(len(st.infos()), 1)
        for bad in ({"role": "server"}, {"proto_min": 0}, {"proto_min": 3, "proto_max": 2}, {"admin": "Bob"},
                    {"rules_url": "http://plain.example.org"}, {"limits": {"realms": -1}}, {"region": "moon"}):
            d = dict(rec["info"], **bad)
            forged = {"kind": "info", "info": d, "sig": keys.sign_doc(n.sec, d, mesh.CTX_INFO)}
            self.assertFalse(st.offer(forged, NOW)[0], bad)
        tampered = json.loads(json.dumps(rec))
        tampered["info"]["limits"]["realms"] = 999
        self.assertEqual(st.offer(tampered, NOW)[1], "refused: signature")
        self.assertEqual(st.offer(rec, NOW + mesh.INFO_TTL + 1)[1], "refused: expired")

    def test_a_block_on_the_relay_drops_its_info_too(self):
        staff, n = keys.new_secret(), Node()
        st = mesh.Store(staff_keys=(keys.public_key(staff).hex(),))
        st.offer(n.rec(), NOW)
        st.offer(mesh.info_record(n.sec, "relay", 1, NOW), NOW)
        st.offer(mesh.block_record(staff, mesh.node_id(n.pub), "spam", NOW, NOW + 3600), NOW)
        self.assertEqual((st.relays(), st.infos()), ([], []))
        self.assertEqual(st.offer(mesh.info_record(n.sec, "relay", 2, NOW), NOW)[1], "refused: blocked")

    def test_proto_range_and_speaks(self):
        self.assertEqual(mesh.proto_range({}), (1, 1))
        self.assertEqual(mesh.proto_range({"proto_min": 1, "proto_max": 4}), (1, 4))
        self.assertEqual(mesh.proto_range({"proto_min": 5, "proto_max": 2}), (1, 1), "malformed reads as today's")
        self.assertTrue(mesh.speaks({"proto_min": 1, "proto_max": 9}))
        self.assertFalse(mesh.speaks({"proto_min": mesh.PROTO + 1, "proto_max": mesh.PROTO + 2}))


class TolerantReader(unittest.TestCase):
    def test_unknown_fields_survive_a_hop_and_unknown_kinds_are_ignored(self):
        r, n = Realm(), Node()
        b, _ = r.beat(relay=n.host)
        b["x_next"] = {"anything": [1, 2]}
        rec = mesh.realm_record(b, keys.sign_doc(r.sec, b, keys.CTX_HEARTBEAT), [n.ep(r.rid)])
        rec["x_beside"] = "kept"
        st = mesh.Store()
        self.assertTrue(st.offer(rec, NOW)[0])
        kept = st.recs["r-" + r.rid]
        self.assertEqual((kept["beat"]["x_next"], kept["x_beside"]), ({"anything": [1, 2]}, "kept"))
        self.assertTrue(mesh.Store().offer(json.loads(json.dumps(kept)), NOW)[0], "the next node takes it as sent")
        ok, why = st.offer({"kind": "probe", "probe": {"v": 1}}, NOW)
        self.assertEqual((ok, why.split(":")[0]), (False, "ignored"))
        newer = json.loads(json.dumps(rec))
        newer["beat"]["v"] = 2
        self.assertTrue(st.offer(newer, NOW)[1].startswith("ignored"))
        self.assertEqual((st.stats["ignored"], st.stats["refused"]), (2, 0))

    def test_the_mesh_list_hides_realms_that_need_a_newer_protocol_but_keeps_them(self):
        node = meshnode.MeshNode(cfg(tempfile.mkdtemp(), mesh=True), clock=lambda: NOW)
        r1, r2, n = Realm(), Realm(), Node()
        b, _ = r2.beat(relay=n.host)
        b["record"]["proto_min"] = mesh.PROTO + 1
        b["record"]["proto_max"] = mesh.PROTO + 1
        node.accept([realm_rec(r1, n), mesh.realm_record(b, keys.sign_doc(r2.sec, b, keys.CTX_HEARTBEAT), [])], NOW)
        self.assertEqual(len(node.store.realms()), 2, "kept and passed on")
        self.assertEqual([x["community"]["realmId"] for x in node.realms_json(NOW)["realms"]], [r1.rid])


class RelayInfo(unittest.TestCase):
    def test_a_relay_publishes_and_serves_its_capability_document(self):
        tmp = tempfile.mkdtemp()
        c = cfg(tmp, mesh=True, region="eu", admin_peer="abcdefghijklmnop", rules_url="https://relay.test/rules",
                languages="en,de", friends_mailbox=True)
        rl = relaymod.Relay(c)
        st, doc = body(asyncio.run(rl.route(Req("GET", "/.well-known/leyline"), None, None)))
        self.assertEqual(st, 200)
        info = doc["info"]
        mesh.check(json.loads(json.dumps(info)), rl.mesh.clock())
        d = info["info"]
        self.assertEqual((d["role"], d["pubkey"], d["admin"], d["region"]), ("relay", rl.mesh.pub, "abcdefghijklmnop", "eu"))
        self.assertEqual(d["languages"], ["en", "de"])
        for cap in ("mesh", "ws-bridge", "listing", "mailbox"):
            self.assertIn(cap, d["supported"])
        self.assertEqual(d["limits"]["record_bytes"], mesh.MAX_RECORD_BYTES)
        self.assertEqual((d["proto_min"], d["proto_max"]), (mesh.PROTO_MIN, mesh.PROTO))
        self.assertEqual(doc["proto"], [mesh.PROTO_MIN, mesh.PROTO])
        st2, doc2 = body(asyncio.run(rl.route(Req("GET", "/api/mesh/info"), None, None)))
        self.assertEqual((st2, doc2["info"]["sig"]), (200, info["sig"]))

    def test_a_bad_admin_or_rules_setting_is_left_out_not_signed(self):
        n = meshnode.MeshNode(cfg(tempfile.mkdtemp(), mesh=True, admin_peer="me@example.org",
                                  rules_url="http://x.example.org"), clock=lambda: NOW)
        d = n.info_doc(NOW)["info"]
        self.assertEqual((d["admin"], d["rules_url"]), ("", ""))

    def test_off_without_the_mesh(self):
        tmp = tempfile.mkdtemp()
        old = os.environ.pop("RELAY_MESH", None)
        try:
            rl = relaymod.Relay(relaymod.Config(state_file=os.path.join(tmp, "s.json")))
            st, _ = body(asyncio.run(rl.route(Req("GET", "/.well-known/leyline"), None, None)))
            self.assertEqual(st, 404)
        finally:
            if old is not None:
                os.environ["RELAY_MESH"] = old

    def test_a_hosts_info_is_passed_on_only_with_its_own_key(self):
        node = meshnode.MeshNode(cfg(tempfile.mkdtemp(), mesh=True), clock=lambda: NOW)
        r, other = Realm(), Realm()
        b, _ = r.beat()
        mine = mesh.info_record(r.sec, "host", 1, NOW, supported=["listing"], limits={"players": 40})
        self.assertFalse(node.host_info(b, mesh.info_record(other.sec, "host", 1, NOW), NOW))
        self.assertFalse(node.host_info(b, mesh.info_record(r.sec, "relay", 1, NOW), NOW), "a host is a host")
        self.assertFalse(node.host_info(b, "junk", NOW))
        self.assertTrue(node.host_info(b, mine, NOW))
        self.assertIn("i-" + r.rid, node.store.recs)

    def test_the_agent_signs_a_host_document(self):
        from wuhost import agent
        tmp = tempfile.mkdtemp()
        sec = keys.load_or_create(os.path.join(tmp, "realm.key"))
        rid = keys.realm_id(keys.public_key(sec))
        with open(os.path.join(tmp, "host.json"), "w") as f:
            json.dump({"realm_id": rid, "name": "Test", "admin_peer": "abcdefghijklmnop", "signups": "invite",
                       "record": {"name": "Test", "max_players": 40, "region": "eu", "languages": ["en"]}}, f)
        a = agent.Agent(tmp)
        bodies = [a.beat_body({"online": True, "players": 1}, i) for i in (0, 1, 60)]
        self.assertEqual(["info" in x for x in bodies], [True, False, True], "the first beat, then hourly")
        rec = bodies[0]["info"]
        import time
        self.assertEqual(mesh.check(rec, time.time())[0], "i-" + rid)
        d = rec["info"]
        self.assertEqual((d["role"], d["admin"], d["limits"], d["restricted"]),
                         ("host", "abcdefghijklmnop", {"players": 40}, ["invite"]))


if __name__ == "__main__":
    unittest.main()
