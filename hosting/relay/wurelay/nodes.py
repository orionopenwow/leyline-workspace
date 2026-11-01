"""Community nodes ("Help carry the network", docs/leyline/be-a-node.md, docs/LEYLINE-MESH.md section 5).

A node is a community relay (RELAY_VOLUNTEER=1) on a player's own PC. It passes other players' game connections
through that PC; it never stores or serves game files or clients (the relay has no file routes for them).

Two halves, both in this module:

Seed side (our relays, RELAY_NODE_REGISTRY=1, the default when RELAY_VOLUNTEER is off):
  POST /api/nodes/register   {"kind": "relay", "node": {...}, "sig"}   a node's own signed relay record (wunet/mesh.py)
                             -> {"ok", "id", "healthy", "banned", "kill"}
  GET  /api/nodes.json       {"format": 1, "kill", "nodes": [{"id", "host", "region", "healthy", "rtt_ms", "caps",
                             "load", "last_ok"}]}   what launchers read before they pick a node per player
  Registration: the record must verify (signature over the node key, fresh), be a volunteer, and name a public DNS
  host. New registrations are capped per address per hour and in total. A health check every HEALTH_EVERY seconds
  fetches the node's own https://<host>/api/mesh/node and compares the key it signs with: a node is healthy after one
  good check and unhealthy after FAILS_UNHEALTHY bad ones in a row; a node that stops registering is dropped.
  Ban and kill switch: Leyline HQ's block feed (/wrath/leyline/relay-block.json, already polled for realms) may carry
  "nodes": [{"id": "n-...", "at"}] and "nodes_off": true. A banned node leaves nodes.json at once and its records are
  refused by this relay's mesh; "nodes_off" tells every launcher to play through our relays only, and every node
  that asks to stop taking new players.

Node side (RELAY_VOLUNTEER=1):
  Every REGISTER_EVERY seconds it registers its signed relay record with each bootstrap seed. When a seed says
  "banned" or "kill", the node pauses: it takes no new players (those already playing finish their session) and
  publishes itself as full, so nobody picks it. The node's owner pauses it too, with RELAY_NODE_DIR/pause (the
  launcher writes it when the PC is busy or on battery); RELAY_NODE_DIR/status.json tells the launcher how many
  players this PC carries now.
"""
from __future__ import annotations

import asyncio
import collections
import json
import logging
import os
import re
import time

from wunet import mesh

from . import httpd

log = logging.getLogger("wu-relay.nodes")
HEALTH_EVERY = 120
FAILS_UNHEALTHY = 3
FORGET_AFTER = 3600          # no registration for an hour: dropped
REGISTER_EVERY = 600
NEW_PER_IP_HOUR = 5
MAX_NODES = 2000
NODE_ID_RE = re.compile(r"^n-[a-z2-7]{16}$")


def _now():
    return time.time()


class Registry:
    """The seed's list of community nodes and their health."""

    def __init__(self, cfg, relay=None, fetch=None, clock=_now):
        self.cfg, self.relay, self.clock = cfg, relay, clock
        if fetch is None:
            from .meshnode import _http
            fetch = _http
        self.fetch = fetch
        self.nodes: dict[str, dict] = {}
        self.new_by_ip = collections.defaultdict(collections.deque)
        self.banned: set[str] = set()
        self.kill = False

    # ---------------------------------------------------------------- the HQ feed
    def take_feed(self, doc):
        """The node half of the block feed: banned node ids and the kill switch. Unknown or missing keys keep the
        old values only when the document is not a feed at all (the caller checked the signature)."""
        rows = doc.get("nodes") if isinstance(doc.get("nodes"), list) else []
        self.banned = {r["id"] for r in rows if isinstance(r, dict) and isinstance(r.get("id"), str)
                       and NODE_ID_RE.match(r["id"])}
        self.kill = doc.get("nodes_off") is True
        for nid in self.banned:
            self.nodes.pop(nid, None)

    # ---------------------------------------------------------------- registration
    def register(self, rec, ip, now=None):
        """(status, reply) for one node's signed relay record."""
        now = self.clock() if now is None else now
        try:
            nid, _seq = mesh.check(rec, now)
        except mesh.Bad as e:
            return 400, {"ok": False, "error": f"not a valid node record ({e})"}
        if rec.get("kind") != "relay":
            return 400, {"ok": False, "error": "a relay record is expected"}
        n = rec["node"]
        if n.get("volunteer") is not True:
            return 400, {"ok": False, "error": "only community relays register here"}
        if nid in self.banned:
            return 403, {"ok": False, "id": nid, "banned": True, "kill": self.kill,
                         "error": "this node is stopped by the Leyline team"}
        e = self.nodes.get(nid)
        if e is None:
            q = self.new_by_ip[ip]
            while q and now - q[0] > 3600:
                q.popleft()
            if len(q) >= NEW_PER_IP_HOUR:
                return 429, {"ok": False, "error": "too many new nodes from this address"}
            self.forget(now)
            if len(self.nodes) >= MAX_NODES:
                return 503, {"ok": False, "error": "the node list is full"}
            q.append(now)
            e = {"id": nid, "first": now, "healthy": False, "fails": 0, "last_ok": None, "last_check": 0.0,
                 "rtt_ms": None}
            self.nodes[nid] = e
        elif e.get("pubkey") and e["pubkey"] != n["pubkey"]:
            return 409, {"ok": False, "error": "key mismatch"}
        e.update(pubkey=n["pubkey"], host=n["host"], http=n["http"], region=n.get("region") or "",
                 caps=dict(n.get("caps") or {}), load=dict(n.get("load") or {}), last_reg=now)
        return 200, {"ok": True, "id": nid, "healthy": e["healthy"], "banned": False, "kill": self.kill}

    def forget(self, now=None):
        now = self.clock() if now is None else now
        for nid in [k for k, e in self.nodes.items() if now - e.get("last_reg", e["first"]) > FORGET_AFTER]:
            del self.nodes[nid]

    # ---------------------------------------------------------------- health
    def check_one(self, e, now=None):
        """One health check (blocking: run in a thread). The node must answer on its own public https name with the
        same key it registered with."""
        now = self.clock() if now is None else now
        t0 = time.monotonic()
        st, doc = self.fetch("GET", e["http"].rstrip("/") + "/api/mesh/node")
        rtt = (time.monotonic() - t0) * 1000
        node = ((doc or {}).get("node") or {}).get("node") if isinstance(doc, dict) else None
        ok = st == 200 and isinstance(node, dict) and node.get("pubkey") == e["pubkey"]
        e["last_check"] = now
        if ok:
            e.update(healthy=True, fails=0, last_ok=now, rtt_ms=round(rtt, 1))
        else:
            e["fails"] = e.get("fails", 0) + 1
            if e["fails"] >= FAILS_UNHEALTHY:
                e["healthy"] = False
        return ok

    async def health_round(self, now=None):
        now = self.clock() if now is None else now
        self.forget(now)
        due = [e for e in self.nodes.values() if now - e.get("last_check", 0) >= HEALTH_EVERY]
        for e in due[:50]:
            try:
                await asyncio.to_thread(self.check_one, e, now)
            except Exception as ex:   # noqa: BLE001: one bad node never stops the round
                log.info("node check %s: %s", e.get("host"), type(ex).__name__)

    def healthy_hosts(self) -> set:
        return {e["host"] for e in self.nodes.values() if e.get("healthy") and e["id"] not in self.banned}

    def doc(self, now=None):
        now = self.clock() if now is None else now
        rows = [{"id": e["id"], "host": e["host"], "region": e.get("region") or "", "healthy": bool(e.get("healthy")),
                 "rtt_ms": e.get("rtt_ms"), "caps": e.get("caps") or {}, "load": e.get("load") or {},
                 "last_ok": int(e["last_ok"]) if e.get("last_ok") else None}
                for e in sorted(self.nodes.values(), key=lambda x: x["host"]) if e["id"] not in self.banned]
        return {"format": 1, "kill": self.kill, "updated": int(now), "nodes": [] if self.kill else rows,
                "counts": {"nodes": len(rows), "healthy": sum(1 for r in rows if r["healthy"]),
                           "banned": len(self.banned)}}

    async def route(self, req, rest):
        pub = {"Access-Control-Allow-Origin": "*"}
        if rest == ["register"] and req.method == "POST":
            st, body = self.register(req.json(), req.ip)
            return httpd.response(st, body)
        if rest == ["nodes.json"] and req.method == "GET":
            return httpd.response(200, self.doc(), headers=dict(pub, **{"Cache-Control": "public, max-age=30"}))
        return httpd.response(404, {"error": "not found"})


class NodeSelf:
    """The node side: register with the seeds, follow their ban / kill answer, honour the owner's pause file, and tell
    the owner's launcher how many players this PC carries (RELAY_NODE_DIR/status.json)."""

    def __init__(self, cfg, relay, fetch=None, clock=_now):
        self.cfg, self.relay, self.clock = cfg, relay, clock
        if fetch is None:
            from .meshnode import _http
            fetch = _http
        self.fetch = fetch
        self.dir = getattr(cfg, "node_dir", "") or ""
        self.seeds = [u.strip().rstrip("/") for u in (getattr(cfg, "mesh_bootstrap", "") or "").split(",") if u.strip()]
        self.hq_stop = None          # None, "banned" or "kill": the Leyline team stopped this node
        self.registered = False
        self.healthy = False
        self.last_bytes = 0
        self.last_at = clock()
        self.kbps_now = 0.0

    def owner_paused(self) -> bool:
        return bool(self.dir) and os.path.exists(os.path.join(self.dir, "pause"))

    def paused(self):
        """Why this node takes no new players now, or None."""
        if self.hq_stop:
            return self.hq_stop
        if self.owner_paused():
            return "owner"
        return None

    def register_once(self):
        """Blocking: one registration with every seed. A seed's ban or kill answer wins over the others."""
        mesh_node = getattr(self.relay, "mesh", None)
        if mesh_node is None or not self.seeds:
            return None
        rec = mesh_node.publish_self()
        stop, ok_any, healthy = None, False, False
        for base in self.seeds:
            st, doc = self.fetch("POST", base + "/api/nodes/register", rec)
            doc = doc if isinstance(doc, dict) else {}
            if doc.get("banned"):
                stop = "banned"
            elif doc.get("kill") and stop is None:
                stop = "kill"
            if st == 200 and doc.get("ok"):
                ok_any = True
                healthy = healthy or bool(doc.get("healthy"))
        if stop != self.hq_stop:
            log.warning("node: the Leyline team %s", {"banned": "stopped this node", "kill": "paused every node",
                                                       None: "lets this node carry players again"}[stop])
        self.hq_stop, self.registered, self.healthy = stop, ok_any, healthy
        return stop

    def players(self) -> int:
        """Players this PC carries now: distinct addresses with a stream open (a player uses two streams)."""
        ips = set()
        for s in getattr(self.relay, "sessions", {}).values():
            for st in s.streams.values():
                ips.add(st.ip)
        return len(ips)

    def status(self, now=None):
        now = self.clock() if now is None else now
        total = int(getattr(self.relay, "metrics", {}).get("bytes", 0))
        dt = max(0.001, now - self.last_at)
        self.kbps_now = round(max(0, total - self.last_bytes) / 1024 / dt, 1)
        self.last_bytes, self.last_at = total, now
        return {"format": 1, "ts": int(now), "players": self.players(),
                "streams": self.relay.streams_total() if self.relay else 0,
                "realms": len(getattr(self.relay, "sessions", {})), "kbps": self.kbps_now,
                "paused": self.paused(), "registered": self.registered, "healthy": self.healthy,
                "caps": {"players": int(getattr(self.cfg, "max_streams_total", 0) or 0) // 2,
                         "kbps": int(getattr(self.cfg, "total_kbps", 0) or 0)}}

    def write_status(self, now=None):
        if not self.dir:
            return None
        doc = self.status(now)
        try:
            os.makedirs(self.dir, exist_ok=True)
            tmp = os.path.join(self.dir, "status.json.tmp")
            with open(tmp, "w", encoding="utf-8") as f:
                json.dump(doc, f)
            os.replace(tmp, os.path.join(self.dir, "status.json"))
        except OSError:
            pass
        return doc
