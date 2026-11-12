"""Friends' mailbox (docs/LEYLINE-FRIENDS.md, "Transport"): OFF unless RELAY_FRIENDS_MAILBOX=1.

A blind drop box for sealed friend messages between launchers that aren't on the same network. The relay stores
envelopes it cannot read and hands them to the one launcher that proves it owns the recipient's peer key.

* ``POST /api/mailbox/<peer id>`` ``{"v": 1, "to": <peer id>, "box": <base32 sealed bytes>, "tag"?: <32 hex>}``:
  kept until fetched or for 24 hours. With a tag, a newer box replaces the older one with the same tag (a friend's
  presence, sent every few minutes, is kept once). Caps: 16 KB a box, 64 boxes a recipient, 20 000 in all, 60 posts a
  minute per address.
* ``GET /api/mailbox/<peer id>?pub=<hex>&ts=<unix>&sig=<hex>``: the boxes, removed as they are handed over. The
  signature is the recipient's Ed25519 key over ``leyline-mailbox-fetch-v1`` + "\\n" + canonical
  ``{"v": 1, "peer_id", "relay", "ts"}``; base32(sha256(pub))[:16] must be the peer id, ``ts`` within 5 minutes,
  ``relay`` one of this relay's names. 20 fetches a minute per address.

What the relay sees: the recipient's peer id, the sender's address (as for any request), sizes and times. Never who
sent a box (that is inside it), never what it says. Nothing is written to disk: a restart empties the mailbox.
"""
from __future__ import annotations

import collections
import re
import time

from wunet import domains, keys

from . import httpd

CTX_FETCH = "leyline-mailbox-fetch-v1"
PEER_RE = re.compile(r"^[a-z2-7]{16}$")
B32_RE = re.compile(r"^[a-z2-7]+$")
TAG_RE = re.compile(r"^[0-9a-f]{32}$")
HEX64_RE = re.compile(r"^[0-9a-f]{64}$")
BOX_MAX = 16 * 1024
PER_PEER = 64
TOTAL = 20_000
TTL = 24 * 3600
SKEW = 300
POSTS_PER_MIN = 60
FETCHES_PER_MIN = 20


class Mailbox:
    def __init__(self, cfg, clock=time.time):
        self.cfg, self.clock = cfg, clock
        self.boxes: dict[str, collections.OrderedDict] = {}   # peer id -> {key: (stored at, envelope)}
        self.count = 0
        self.hits: dict[str, list] = {}                        # (kind, ip) -> [minute, n]
        self.serial = 0
        names = {getattr(cfg, "name", "") or "", domains.RELAY, (domains.OLD or {}).get("relay") or ""}
        self.names = {n for n in names if n}

    def _limit(self, kind, ip, per_min):
        now = int(self.clock()) // 60
        k = f"{kind}:{ip}"
        h = self.hits.get(k)
        if h is None or h[0] != now:
            if len(self.hits) > 50_000:
                self.hits = {kk: v for kk, v in self.hits.items() if v[0] == now}
            h = self.hits[k] = [now, 0]
        h[1] += 1
        return h[1] <= per_min

    def _expire(self):
        cut = self.clock() - TTL
        for pid in list(self.boxes):
            q = self.boxes[pid]
            for key in [k for k, (at, _) in q.items() if at < cut]:
                del q[key]
                self.count -= 1
            if not q:
                del self.boxes[pid]

    def post(self, pid, env, ip):
        if not self._limit("post", ip, POSTS_PER_MIN):
            return 429, {"error": "slow down"}
        if not isinstance(env, dict) or env.get("v") != 1 or env.get("to") != pid:
            return 400, {"error": "not an envelope for this mailbox"}
        box, tag = env.get("box"), env.get("tag")
        if not isinstance(box, str) or not B32_RE.match(box) or len(box) > BOX_MAX:
            return 400, {"error": "box"}
        if tag is not None and not (isinstance(tag, str) and TAG_RE.match(tag)):
            return 400, {"error": "tag"}
        if set(env) - {"v", "to", "box", "tag"}:
            return 400, {"error": "unknown fields"}
        self._expire()
        q = self.boxes.setdefault(pid, collections.OrderedDict())
        if tag is not None and ("t:" + tag) in q:
            del q["t:" + tag]
            self.count -= 1
        if len(q) >= PER_PEER:
            return 429, {"error": "this mailbox is full"}
        if self.count >= TOTAL:
            return 503, {"error": "the mailbox is full, try later"}
        self.serial += 1
        q["t:" + tag if tag is not None else f"n:{self.serial}"] = (self.clock(), {"v": 1, "to": pid, "box": box, **({"tag": tag} if tag else {})})
        self.count += 1
        return 200, {"ok": True}

    def fetch(self, pid, q, ip):
        if not self._limit("fetch", ip, FETCHES_PER_MIN):
            return 429, {"error": "slow down"}
        pub, sig, ts = q.get("pub", ""), q.get("sig", ""), q.get("ts", "")
        if not HEX64_RE.match(pub) or not ts.isdigit() or len(ts) > 12:
            return 400, {"error": "proof"}
        pk = bytes.fromhex(pub)
        if keys.realm_id(pk) != pid:
            return 403, {"error": "not this mailbox's key"}
        t = int(ts)
        if abs(self.clock() - t) > SKEW:
            return 403, {"error": "proof expired: check this PC's clock"}
        relay = q.get("relay", "")
        if relay not in self.names:
            return 403, {"error": "proof for another relay"}
        doc = {"v": 1, "peer_id": pid, "relay": relay, "ts": t}
        if not keys.verify_doc(pk, doc, CTX_FETCH, sig):
            return 403, {"error": "proof"}
        self._expire()
        box = self.boxes.pop(pid, None) or {}
        self.count -= len(box)
        return 200, {"boxes": [env for _, env in box.values()]}

    async def route(self, req, rest):
        if len(rest) != 1 or not PEER_RE.match(rest[0]):
            return httpd.response(404, {"error": "not found"})
        pid = rest[0]
        if req.method == "POST":
            st, body = self.post(pid, req.json(), req.ip)
        elif req.method == "GET":
            st, body = self.fetch(pid, req.query, req.ip)
        else:
            st, body = 405, {"error": "method"}
        return httpd.response(st, body, headers={"Cache-Control": "no-store"})
