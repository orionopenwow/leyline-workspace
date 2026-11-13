"""Commonhold v1 conformance kit (docs/COMMONHOLD-V1.md): test vectors, a runner, and a probe for a live node.

    python3 -m wunet.commonhold make  [OUT]     write the vectors (deterministic: fixed keys, fixed times)
    python3 -m wunet.commonhold check [FILE]    run the vectors against this code (exit 1 on a failure)
    python3 -m wunet.commonhold probe URL       fetch a node's /.well-known/leyline, /api/mesh/node and
                                                /api/mesh/snapshot and check every record it serves

The vectors file is `launcher/src-tauri/src/p2p/commonhold-v1-vectors.json`; the launcher's Rust tests read the same
file (p2p/mesh.rs `commonhold_vectors`). Another implementation passes when, for every case, it gives the same
result: "ok" (with the same id, version and number of good endpoints, and the listed fields still there), "refused"
or "ignored". Merge cases also name the record each id must end with. Standard library only.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import urllib.request

from . import keys, mesh

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
VECTORS = os.path.join(REPO, "launcher", "src-tauri", "src", "p2p", "commonhold-v1-vectors.json")
ORIGINAL = os.path.join(REPO, "launcher", "src-tauri", "src", "p2p", "mesh-vector.json")
T = 1761700000          # the vectors' clock (the original mesh-vector.json's beat time)
FORMAT = "commonhold-v1-vectors"


def _key(n: int) -> bytes:
    return bytes([n]) * 32   # fixed test seeds: never use these anywhere real


def _beat(sec, seq, ts, name="Vector Realm", relay="relay.example.org", relays=None, extra=None, record_extra=None):
    pub = keys.public_key(sec)
    b = {"v": 1, "realm_id": keys.realm_id(pub), "pubkey": pub.hex(), "seq": seq, "ts": ts, "relay": relay,
         "record": dict({"name": name, "tags": ["pve"]}, **(record_extra or {})),
         "status": {"online": True, "players": 3, "bots": 10, "uptime_s": 60}}
    if relays:
        b["relays"] = relays
    b.update(extra or {})
    return b, keys.sign_doc(sec, b, keys.CTX_HEARTBEAT)


def _realm(sec, seq, ts, eps=(), **kw):
    b, s = _beat(sec, seq, ts, **kw)
    return mesh.realm_record(b, s, list(eps))


def _rid(sec):
    return keys.realm_id(keys.public_key(sec))


def _id(prefix, sec):
    return prefix + _rid(sec)


def ok(rec_id, version, endpoints=None, keeps=()):
    e = {"result": "ok", "id": rec_id, "version": version}
    if endpoints is not None:
        e["endpoints"] = endpoints
    if keeps:
        e["keeps"] = list(keeps)
    return e


def make() -> dict:
    realm, relay, other, staff, host = _key(11), _key(12), _key(13), _key(14), _key(15)
    rid = _rid(realm)
    ep = mesh.endpoint(relay, rid, "relay.example.org", 40000, 40001, True, T)
    ep_wrong = mesh.endpoint(other, rid, "elsewhere.example.org", 40100, 40101, False, T)
    good = _realm(realm, 100, T, [ep])
    node = mesh.relay_record(relay, "relay.example.org", T * 1000, T, region="eu", volunteer=True,
                             caps={"realms": 25, "streams": 512, "kbps": 256}, load={"realms": 1, "streams": 4})
    info_relay = mesh.info_record(relay, "relay", T * 1000, T, supported=["mesh", "ws-bridge", "listing"],
                                  limits={"realms": 25, "streams": 512, "kbps": 256, "record_bytes": 6000},
                                  admin=_rid(staff), software="wurelay", version="2026.11", region="eu",
                                  languages=["en", "de"], rules_url="https://relay.example.org/rules")
    info_host = mesh.info_record(host, "host", T * 1000, T, supported=["listing", "herokey"], limits={"players": 100},
                                 software="wuhost", version="wu-agent/1", restricted=["herokey"])
    info_future = mesh.info_record(relay, "relay", T * 1000 + 1, T, supported=["mesh", "quantum-relay"],
                                   proto_min=1, proto_max=3, extra={"motd": "fields from a newer spec are kept"})
    tampered = json.loads(json.dumps(good))
    tampered["beat"]["status"]["players"] = 4
    tampered_ep = json.loads(json.dumps(good))
    tampered_ep["endpoints"][0]["att"]["auth_port"] = 1
    b2, _ = _beat(other, 100, T)
    b2["realm_id"] = rid                         # another key claiming this realm's id
    stolen_id = mesh.realm_record(b2, keys.sign_doc(other, b2, keys.CTX_HEARTBEAT), [])
    wrong_relay = _realm(realm, 101, T, [ep_wrong])
    tolerant = _realm(realm, 102, T, [ep], extra={"x_future": {"level": 2}}, record_extra={"proto_min": 1, "proto_max": 2})
    tolerant["x_hint"] = "unsigned fields beside the record are kept too"
    newer_v = json.loads(json.dumps(good))
    newer_v["beat"]["v"] = 2
    unknown = {"kind": "probe", "probe": {"v": 1, "target": "r-" + rid, "ok": True, "rtt_ms": 31}, "sig": "00" * 64}
    bad_info = mesh.info_record(relay, "relay", T * 1000, T, proto_min=3, proto_max=2)
    blk = mesh.block_record(staff, "r-" + rid, "vector", T, T + 86400)
    eq_a = _realm(realm, 200, T, name="Equivocation A")
    eq_b = _realm(realm, 200, T, name="Equivocation B")
    eq_win = min((eq_a, eq_b), key=mesh.sig_rank)
    ep2 = mesh.endpoint(other, rid, "two.example.org", 41000, 41001, False, T)
    m1 = _realm(realm, 300, T, [ep], relays=["two.example.org"])
    m2 = mesh.realm_record(m1["beat"], m1["sig"], [ep2])
    newer = _realm(realm, 301, T + 60, [ep])
    with open(ORIGINAL, encoding="utf-8") as f:
        orig = json.load(f)["records"]
    o_rid = orig[0]["beat"]["realm_id"]
    o_node = mesh.node_id(orig[1]["node"]["pubkey"])

    cases = [
        {"name": "mesh-vector.json (2026-10-29): a realm with its relay's endpoint, and that relay's record",
         "now": 1761700010, "check": [{"record": orig[0], "expect": ok("r-" + o_rid, orig[0]["beat"]["seq"], 1)},
                                       {"record": orig[1], "expect": ok(o_node, orig[1]["node"]["seq"])}]},
        {"name": "valid realm, relay and capability documents", "now": T + 10,
         "check": [{"record": good, "expect": ok("r-" + rid, 100, 1)},
                   {"record": node, "expect": ok(_id("n-", relay), T * 1000)},
                   {"record": info_relay, "expect": ok(_id("i-", relay), T * 1000)},
                   {"record": info_host, "expect": ok(_id("i-", host), T * 1000)}]},
        {"name": "tampered: a changed byte breaks the signature", "now": T + 10,
         "check": [{"record": tampered, "expect": {"result": "refused"}}]},
        {"name": "tampered endpoint: dropped, the realm stays", "now": T + 10,
         "check": [{"record": tampered_ep, "expect": ok("r-" + rid, 100, 0)}]},
        {"name": "a key claiming another realm's id", "now": T + 10,
         "check": [{"record": stolen_id, "expect": {"result": "refused"}}]},
        {"name": "expired: 15 minutes for realms, 30 for relays, a day for info", "now": T + mesh.INFO_TTL + 1,
         "check": [{"record": good, "expect": {"result": "refused"}},
                   {"record": node, "expect": {"result": "refused"}},
                   {"record": info_relay, "expect": {"result": "refused"}}]},
        {"name": "from the future (beyond the 300 s skew)", "now": T - mesh.SKEW - 1,
         "check": [{"record": good, "expect": {"result": "refused"}}]},
        {"name": "wrong relay: an endpoint on a relay the realm never chose is dropped", "now": T + 10,
         "check": [{"record": wrong_relay, "expect": ok("r-" + rid, 101, 0)}]},
        {"name": "tolerant reader: unknown fields are kept, signed and unsigned", "now": T + 10,
         "check": [{"record": tolerant, "expect": ok("r-" + rid, 102, 1,
                                                     ["beat.x_future", "beat.record.proto_max", "x_hint"])},
                   {"record": info_future, "expect": ok(_id("i-", relay), T * 1000 + 1, keeps=["info.motd"])}]},
        {"name": "tolerant reader: unknown kinds and newer document versions are ignored, not refused", "now": T + 10,
         "check": [{"record": unknown, "expect": {"result": "ignored"}},
                   {"record": newer_v, "expect": {"result": "ignored"}}]},
        {"name": "a capability document with proto_min above proto_max", "now": T + 10,
         "check": [{"record": bad_info, "expect": {"result": "refused"}}]},
        {"name": "a block counts only from a pinned staff key", "now": T + 10,
         "check": [{"record": blk, "expect": {"result": "refused"}},
                   {"record": blk, "staff": [keys.public_key(staff).hex()], "expect": ok("b-r-" + rid, T)}]},
        {"name": "merge: equivocation keeps the smaller sha256(signature), in either order", "now": T + 10,
         "merge": [eq_a, eq_b], "final": {"r-" + rid: eq_win["sig"]}},
        {"name": "merge: equivocation, other order", "now": T + 10,
         "merge": [eq_b, eq_a], "final": {"r-" + rid: eq_win["sig"]}},
        {"name": "merge: the same beat from two relays merges their endpoints", "now": T + 70,
         "merge": [m1, m2], "final": {"r-" + rid: m1["sig"]}, "final_endpoints": {"r-" + rid: 2}},
        {"name": "merge: newer seq wins, an older one after it changes nothing", "now": T + 70,
         "merge": [newer, m1], "final": {"r-" + rid: newer["sig"]}},
        {"name": "merge: a staff block drops the realm and refuses it after", "now": T + 10,
         "staff": [keys.public_key(staff).hex()], "merge": [good, blk, good],
         "final": {"r-" + rid: None, "b-r-" + rid: blk["sig"]}},
    ]
    return {"format": FORMAT, "spec": "docs/COMMONHOLD-V1.md", "proto": [mesh.PROTO_MIN, mesh.PROTO],
            "note": "Test keys are seeds 0x0b..0x0f repeated: never use them anywhere real.", "cases": cases}


def _has(doc, path):
    for p in path.split("."):
        if not isinstance(doc, dict) or p not in doc:
            return False
        doc = doc[p]
    return True


def run(v: dict) -> list[str]:
    """Every failure, as text; empty = conformant."""
    fails = []
    for c in v["cases"]:
        now = c["now"]
        for i, t in enumerate(c.get("check", [])):
            rec = json.loads(json.dumps(t["record"]))
            exp = t["expect"]
            staff = tuple(t.get("staff", c.get("staff", ())))
            try:
                rid, ver = mesh.check(rec, now, staff)
                got = {"result": "ok", "id": rid, "version": ver}
            except mesh.Skip as e:
                got = {"result": "ignored", "why": str(e)}
            except (mesh.Bad, ValueError, TypeError, KeyError, AttributeError) as e:
                got = {"result": "refused", "why": str(e)}
            where = f"{c['name']} [{i}]"
            if got["result"] != exp["result"]:
                fails.append(f"{where}: expected {exp['result']}, got {got}")
                continue
            if exp["result"] == "ok":
                if (got["id"], got["version"]) != (exp["id"], exp["version"]):
                    fails.append(f"{where}: id/version {got['id']} {got['version']}")
                if "endpoints" in exp and len(rec.get("endpoints", [])) != exp["endpoints"]:
                    fails.append(f"{where}: {len(rec.get('endpoints', []))} endpoints, expected {exp['endpoints']}")
                for k in exp.get("keeps", []):
                    if not _has(rec, k):
                        fails.append(f"{where}: dropped {k}")
        if "merge" in c:
            st = mesh.Store(staff_keys=tuple(c.get("staff", ())))
            for r in c["merge"]:
                st.offer(r, now)
            for rid, sig in c["final"].items():
                have = st.recs.get(rid)
                if (have or {}).get("sig") != sig:
                    fails.append(f"{c['name']}: {rid} ended with {(have or {}).get('sig', 'nothing')[:16]}")
            for rid, n in c.get("final_endpoints", {}).items():
                if len((st.recs.get(rid) or {}).get("endpoints", [])) != n:
                    fails.append(f"{c['name']}: {rid} endpoints")
    return fails


def _get(url, timeout=8.0):
    req = urllib.request.Request(url, headers={"User-Agent": "leyline-conform/1"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read(8 << 20).decode("utf-8"))


def probe(base: str, get=_get, now=None) -> list[str]:
    """Check a live node: its capability document, its relay record (same key), and every record in its snapshot."""
    import time
    now = time.time() if now is None else now
    base = base.rstrip("/")
    out = []
    try:
        info = get(base + "/.well-known/leyline")["info"]
        iid, _ = mesh.check(json.loads(json.dumps(info)), now)
        out.append(f"ok   info {iid}: proto {info['info']['proto_min']}..{info['info']['proto_max']}, "
                   f"supports {', '.join(info['info'].get('supported', []))}")
        if not mesh.speaks(info["info"]):
            out.append("FAIL the node speaks no protocol level this kit speaks")
    except Exception as e:   # noqa: BLE001 (report and go on)
        info = None
        out.append(f"FAIL /.well-known/leyline: {e}")
    try:
        node = get(base + "/api/mesh/node")["node"]
        nid, _ = mesh.check(json.loads(json.dumps(node)), now)
        same = info is not None and info["info"]["pubkey"] == node["node"]["pubkey"]
        out.append(f"{'ok  ' if same else 'FAIL'} node {nid}" + ("" if same else ": not the info document's key"))
    except Exception as e:   # noqa: BLE001
        out.append(f"FAIL /api/mesh/node: {e}")
    try:
        snap = get(base + "/api/mesh/snapshot")
        bad = 0
        for r in snap.get("records", []):
            try:
                mesh.check(json.loads(json.dumps(r)), now)
            except mesh.Skip:
                pass
            except Exception:   # noqa: BLE001
                bad += 1
        out.append(f"{'ok  ' if not bad else 'FAIL'} snapshot: {len(snap.get('records', []))} records, {bad} bad")
    except Exception as e:   # noqa: BLE001
        out.append(f"FAIL /api/mesh/snapshot: {e}")
    return out


def digest(v: dict) -> str:
    return hashlib.sha256(keys.canonical(v)).hexdigest()


def main(argv=None) -> int:
    a = list(sys.argv[1:] if argv is None else argv)
    cmd = a[0] if a else "check"
    if cmd == "make":
        out = a[1] if len(a) > 1 else VECTORS
        with open(out, "w", encoding="utf-8", newline="\n") as f:
            json.dump(make(), f, indent=1, sort_keys=True, ensure_ascii=False)
            f.write("\n")
        print(f"wrote {out}")
        return 0
    if cmd == "check":
        with open(a[1] if len(a) > 1 else VECTORS, encoding="utf-8") as f:
            v = json.load(f)
        fails = run(v)
        for x in fails:
            print("FAIL", x)
        n = sum(len(c.get("check", [])) + (1 if "merge" in c else 0) for c in v["cases"])
        print(f"{n - len(fails)} of {n} checks passed" if fails else f"all {n} checks passed")
        return 1 if fails else 0
    if cmd == "probe" and len(a) > 1:
        lines = probe(a[1])
        print("\n".join(lines))
        return 1 if any(x.startswith("FAIL") for x in lines) else 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
