"""Friends' mailbox on the relay (wurelay/mailbox.py, RELAY_FRIENDS_MAILBOX; docs/LEYLINE-FRIENDS.md): off by default,
boxes kept blind until the key's owner fetches them with a signed proof, tags replace, caps and rate limits, expiry.

    cd hosting/tests && python3 -m unittest test_relay_mailbox
"""
import os
import unittest

from helpers import Rig, http
from wunet import keys
from wurelay import mailbox


class Cfg:
    name = "relay.test"


def peer(seed):
    sk = bytes([seed]) * 32
    pk = keys.public_key(sk)
    return sk, pk, keys.realm_id(pk)


def proof(sk, pk, pid, ts, relay="relay.test"):
    doc = {"v": 1, "peer_id": pid, "relay": relay, "ts": ts}
    return {"pub": pk.hex(), "ts": str(ts), "relay": relay, "sig": keys.sign_doc(sk, doc, mailbox.CTX_FETCH)}


class Clock:
    def __init__(self):
        self.t = 1_800_000_000.0

    def __call__(self):
        return self.t


class MailboxUnit(unittest.TestCase):
    def setUp(self):
        self.clock = Clock()
        self.m = mailbox.Mailbox(Cfg(), clock=self.clock)
        self.sk, self.pk, self.pid = peer(7)

    def env(self, box="abc", tag=None, to=None):
        e = {"v": 1, "to": to or self.pid, "box": box}
        if tag:
            e["tag"] = tag
        return e

    def test_post_and_fetch_once(self):
        self.assertEqual(self.m.post(self.pid, self.env("aaaa"), "1.1.1.1")[0], 200)
        self.assertEqual(self.m.post(self.pid, self.env("bbbb"), "1.1.1.1")[0], 200)
        st, body = self.m.fetch(self.pid, proof(self.sk, self.pk, self.pid, int(self.clock.t)), "2.2.2.2")
        self.assertEqual(st, 200)
        self.assertEqual([b["box"] for b in body["boxes"]], ["aaaa", "bbbb"])
        st, body = self.m.fetch(self.pid, proof(self.sk, self.pk, self.pid, int(self.clock.t)), "2.2.2.2")
        self.assertEqual(body["boxes"], [], "handed over once")
        self.assertEqual(self.m.count, 0)

    def test_only_the_owner_fetches(self):
        self.m.post(self.pid, self.env("aaaa"), "1.1.1.1")
        osk, opk, opid = peer(9)
        # another key, its own proof, this mailbox: refused
        self.assertEqual(self.m.fetch(self.pid, proof(osk, opk, self.pid, int(self.clock.t)), "3.3.3.3")[0], 403)
        # the right key, a signature for another mailbox's id
        p = proof(self.sk, self.pk, self.pid, int(self.clock.t))
        p["sig"] = keys.sign_doc(self.sk, {"v": 1, "peer_id": opid, "relay": "relay.test", "ts": int(self.clock.t)}, mailbox.CTX_FETCH)
        self.assertEqual(self.m.fetch(self.pid, p, "3.3.3.3")[0], 403)
        # an old proof, a proof for another relay
        self.assertEqual(self.m.fetch(self.pid, proof(self.sk, self.pk, self.pid, int(self.clock.t) - 600), "3.3.3.3")[0], 403)
        self.assertEqual(self.m.fetch(self.pid, proof(self.sk, self.pk, self.pid, int(self.clock.t), "relay.evil"), "3.3.3.3")[0], 403)
        self.assertEqual(self.m.count, 1, "still there for its owner")

    def test_bad_envelopes(self):
        for e in (self.env("NOT-B32"), self.env("a" * (mailbox.BOX_MAX + 1)), self.env(tag="zz"), self.env(to="bbbbbbbbbbbbbbbb"),
                  dict(self.env(), sender="me"), {"v": 2, "to": self.pid, "box": "aa"}):
            self.assertEqual(self.m.post(self.pid, e, "1.1.1.1")[0], 400, e)

    def test_tags_replace_and_caps(self):
        tag = "0" * 32
        self.m.post(self.pid, self.env("aaaa", tag), "1.1.1.1")
        self.m.post(self.pid, self.env("bbbb", tag), "1.1.1.2")
        self.assertEqual(self.m.count, 1)
        _, body = self.m.fetch(self.pid, proof(self.sk, self.pk, self.pid, int(self.clock.t)), "2.2.2.2")
        self.assertEqual([b["box"] for b in body["boxes"]], ["bbbb"])
        for n in range(mailbox.PER_PEER):
            self.assertEqual(self.m.post(self.pid, self.env("aa"), f"10.0.{n}.1")[0], 200)
        self.assertEqual(self.m.post(self.pid, self.env("aa"), "10.1.0.1")[0], 429, "a full mailbox")

    def test_rate_limit_and_expiry(self):
        _, _, other = peer(8)
        codes = [self.m.post(other, self.env("aa", to=other), "9.9.9.9")[0] for _ in range(mailbox.POSTS_PER_MIN + 1)]
        self.assertEqual(codes[-1], 429)
        self.assertNotIn(429, codes[:-1][:mailbox.PER_PEER])
        self.m.post(self.pid, self.env("aaaa"), "1.1.1.1")
        self.clock.t += mailbox.TTL + 61
        _, body = self.m.fetch(self.pid, proof(self.sk, self.pk, self.pid, int(self.clock.t)), "2.2.2.2")
        self.assertEqual(body["boxes"], [], "expired")


class MailboxOnRelay(unittest.IsolatedAsyncioTestCase):
    async def test_off_by_default_on_with_the_flag(self):
        sk, pk, pid = peer(7)
        async with Rig() as r:
            st, _, _ = await http("POST", f"{r.http}/api/mailbox/{pid}", {"v": 1, "to": pid, "box": "aa"})
            self.assertEqual(st, 404, "off unless RELAY_FRIENDS_MAILBOX=1")
        async with Rig(friends_mailbox=True) as r:
            st, _, _ = await http("POST", f"{r.http}/api/mailbox/{pid}", {"v": 1, "to": pid, "box": "aa"})
            self.assertEqual(st, 200)
            import time
            p = proof(sk, pk, pid, int(time.time()))
            q = "&".join(f"{k}={v}" for k, v in p.items())
            st, body, _ = await http("GET", f"{r.http}/api/mailbox/{pid}?{q}")
            self.assertEqual((st, [b["box"] for b in body["boxes"]]), (200, ["aa"]))


if __name__ == "__main__":
    unittest.main()
