"""Ops review (4 Nov): the relay, the mesh and the community node registry, the edges the other suites don't reach.

  * a relay whose state file is unreadable keeps a copy of it before the next save writes over it;
  * mesh records and node registrations never name an IP literal (the seed would fetch it; launchers would dial it);
  * the node registry takes only https nodes;
  * /metrics answers 403 to the internet in every Caddy block the relay ships with;
  * wrong-password lockouts and the knock table forget old entries (memory stays bounded);
  * RELAY_VOLUNTEER=1 gives the small caps, and an operator's own env value still wins.

    python3 hosting/tests/test_ops_review.py
"""
import glob
import json
import os
import tempfile
import time
import unittest
from unittest import mock

import helpers  # noqa: F401  (puts kit/tools and relay on sys.path)
from wunet import keys, mesh
from wurelay import nodes
from wurelay import relay as relaymod

HERE = os.path.dirname(os.path.abspath(__file__))
HOSTING = os.path.dirname(HERE)


def cfg(tmp, **kw):
    base = dict(name="relay.test", http_bind="127.0.0.1:0", game_bind="127.0.0.1", shared_auth_port=0,
                port_min=42000, port_max=42011, state_file=os.path.join(tmp, "state.json"), listing=False)
    base.update(kw)
    return relaymod.Config(**base)


class State(unittest.TestCase):
    def test_unreadable_state_is_kept_aside(self):
        tmp = tempfile.mkdtemp()
        p = os.path.join(tmp, "state.json")
        with open(p, "w") as f:
            f.write('{"realms": {"abc": ')   # cut off mid-write (disk full, power loss)
        with self.assertLogs("wu-relay", "ERROR") as lg:
            r = relaymod.Relay(cfg(tmp))
        self.assertEqual(r.state["realms"], {})
        kept = glob.glob(p + ".unreadable-*")
        self.assertEqual(len(kept), 1, "the bad file is copied aside")
        self.assertEqual(open(kept[0]).read(), '{"realms": {"abc": ')
        self.assertIn("kept as", "".join(lg.output))
        r.dirty = True
        r.save()
        self.assertEqual(json.load(open(p))["realms"], {})
        self.assertEqual(open(kept[0]).read(), '{"realms": {"abc": ', "the copy survives the next save")

    def test_save_is_atomic_and_round_trips(self):
        tmp = tempfile.mkdtemp()
        r = relaymod.Relay(cfg(tmp))
        a = r.allocate("aaaaaaaaaaaaaaaa", "One")
        b = r.allocate("bbbbbbbbbbbbbbbb", "Two")
        self.assertEqual((a["auth"] % 2, b["auth"], b["world"]), (0, a["auth"] + 2, a["auth"] + 3))
        r.save()
        self.assertFalse(os.path.exists(r.cfg.state_file + ".tmp"))
        r2 = relaymod.Relay(cfg(tmp))
        self.assertEqual(r2.port_owner[a["auth"]], ("aaaaaaaaaaaaaaaa", 3724))
        self.assertEqual(r2.port_owner[b["world"]], ("bbbbbbbbbbbbbbbb", 8085))

    def test_full_relay_reclaims_only_stale_offline_realms(self):
        tmp = tempfile.mkdtemp()
        r = relaymod.Relay(cfg(tmp, port_min=42000, port_max=42003))   # two realms fit
        r.allocate("aaaaaaaaaaaaaaaa", "One")
        r.allocate("bbbbbbbbbbbbbbbb", "Two")
        self.assertIsNone(r.allocate("cccccccccccccccc", "Three"), "full, nobody stale")
        r.state["realms"]["aaaaaaaaaaaaaaaa"]["last"] = int(time.time()) - 40 * 86400
        r.sessions["aaaaaaaaaaaaaaaa"] = object()     # stale but online: kept
        self.assertIsNone(r.allocate("cccccccccccccccc", "Three"))
        del r.sessions["aaaaaaaaaaaaaaaa"]
        got = r.allocate("cccccccccccccccc", "Three")
        self.assertIsNotNone(got)
        self.assertNotIn("aaaaaaaaaaaaaaaa", r.state["realms"])


def relay_rec(host, http=None, volunteer=True):
    sec = keys.new_secret()
    ts = int(time.time())
    return mesh.relay_record(sec, host, ts * 1000, ts, http=http, volunteer=volunteer,
                             caps={"realms": 25, "streams": 64}, load={"realms": 0, "streams": 0})


class NoIpLiterals(unittest.TestCase):
    def test_host_ok(self):
        for h in ("127.0.0.1", "10.0.0.5", "169.254.169.254", "192.168.1.1", "0.0.0.0"):
            self.assertFalse(mesh.host_ok(h), h)
        for h in ("relay.leylinelauncher.com", "node1.example.org", "a1.b2.example"):
            self.assertTrue(mesh.host_ok(h), h)

    def test_mesh_refuses_a_relay_record_naming_an_ip(self):
        rec = relay_rec("node.example.org")
        mesh.check(rec, time.time())     # a good one passes
        with self.assertRaises(mesh.Bad):
            mesh.check(relay_rec("10.0.0.5", http="http://10.0.0.5:8080"), time.time())


class Registry(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.mkdtemp()
        self.reg = nodes.Registry(cfg(tmp), fetch=lambda m, u, b=None: (0, None))

    def test_https_only(self):
        st, body = self.reg.register(relay_rec("node.example.org", http="http://node.example.org"), "203.0.113.5")
        self.assertEqual(st, 400)
        self.assertIn("https", body["error"])
        st, _ = self.reg.register(relay_rec("node.example.org"), "203.0.113.5")
        self.assertEqual(st, 200)

    def test_ip_host_refused(self):
        st, _ = self.reg.register(relay_rec("127.0.0.1", http="https://127.0.0.1"), "203.0.113.5")
        self.assertEqual(st, 400)
        self.assertEqual(self.reg.doc()["nodes"], [])


class MetricsNotPublic(unittest.TestCase):
    def test_every_relay_caddy_block_hides_metrics(self):
        for path in ("relay/deploy/Caddyfile", "kit/tools/Caddyfile.volunteer", "relay/deploy/relay-up.sh"):
            src = open(os.path.join(HOSTING, path), encoding="utf-8").read()
            self.assertIn("@metrics path /metrics", src, path)
            self.assertRegex(src, r"respond @(notlocal|metrics) 403", path)


class Bounded(unittest.TestCase):
    def test_auth_lock_expires(self):
        tmp = tempfile.mkdtemp()
        r = relaymod.Relay(cfg(tmp, auth_fails_per_ip=3, auth_fail_window=600))
        now = [1000.0]
        with mock.patch.object(relaymod.time, "monotonic", lambda: now[0]):
            r.auth_fails[("198.51.100.7", "aaaaaaaaaaaaaaaa")] = __import__("collections").deque([now[0]] * 3)
            self.assertTrue(r.auth_locked("198.51.100.7", "aaaaaaaaaaaaaaaa"))
            self.assertFalse(r.auth_locked("198.51.100.8", "aaaaaaaaaaaaaaaa"), "another address is not locked")
            now[0] += 601
            self.assertFalse(r.auth_locked("198.51.100.7", "aaaaaaaaaaaaaaaa"))
            self.assertNotIn(("198.51.100.7", "aaaaaaaaaaaaaaaa"), r.auth_fails, "the entry is forgotten")

    def test_knock_table_caps_realms_per_ip_and_expires(self):
        tmp = tempfile.mkdtemp()
        r = relaymod.Relay(cfg(tmp, knock_ttl=60))
        for i in range(20):
            r.knock("198.51.100.7", f"realm{i:02d}aaaaaaaaa")
        self.assertEqual(len(r.knocks["198.51.100.7"]), 16)
        self.assertEqual(r.last_knock("198.51.100.7"), "realm19aaaaaaaaa")
        later = time.monotonic() + 61
        with mock.patch.object(relaymod.time, "monotonic", lambda: later):
            self.assertIsNone(r.last_knock("198.51.100.7"))
            self.assertFalse(r.knocked("198.51.100.7", "realm19aaaaaaaaa"))


class Volunteer(unittest.TestCase):
    def test_small_caps_and_operator_wins(self):
        with mock.patch.dict(os.environ, {"RELAY_VOLUNTEER": "1", "RELAY_KBPS": "1000"}, clear=False):
            for k in ("RELAY_MAX_REALMS", "RELAY_MAX_STREAMS_TOTAL", "RELAY_DAILY_GB"):
                os.environ.pop(k, None)
            c = relaymod.Config(state_file=os.path.join(tempfile.mkdtemp(), "s.json"))
        self.assertEqual((c.max_realms, c.max_streams_total, c.daily_gb, c.mesh), (25, 512, 5, True))
        self.assertEqual(c.kbps, 1000, "the operator's own RELAY_KBPS is kept")
        self.assertEqual(c.shared_auth_port, 0, "a community relay doesn't take port 3724")


if __name__ == "__main__":
    unittest.main()
