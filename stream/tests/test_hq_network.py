"""Leyline HQ's Network page (stream/hq.py network_page, stream/admin.py node blocks): realms, Hero Key realms, the
community nodes and their health, the network switches; Stop this node (admins, a reason, a typed confirm) and Stop every
node (owner only); both reach the relay's block feed (/wrath/leyline/relay-block.json) as ids and times only, and
stopping a node never drops a realm block.

    python3 stream/tests/test_hq_network.py
"""
import importlib.util
import json
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
T = tempfile.mkdtemp(prefix="wu-hqnet-")
os.environ.update(WU_PORTAL_DATA=T, WU_ADMIN_SECRET="test-secret")

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, str(extra)[:600])


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


nc = load("wu_namecheck", os.path.join(STREAM, "namecheck.py"))
sys.modules["wu_namecheck"] = nc
A = load("wu_admin_net", os.path.join(STREAM, "admin.py"))
H = A.hq
owner = {"account": "owner1", "name": "Owner1", "role": "owner", "session": "s1"}
admin = {"account": "adm1", "name": "Adm", "role": "admin", "session": "s2"}
mod = {"account": "mod1", "name": "Mod", "role": "mod", "session": "s3"}
NID = "n-abcdefghijk2345a"
RID = "qrstuvwxyz234567"


def run(user, action, target, **form):
    return A.run_action(user, dict(form, action=action, target=target))


def fake_fetch(url):
    if url.endswith("/api/listing/realms.json"):
        return {"realms": [{"name": "Frost Haven", "state": "live", "online": 4, "herokey": True},
                           {"name": "Quiet Glade", "state": "live", "online": 1, "herokey": False},
                           {"name": "Gone", "state": "offline", "online": 0, "herokey": True}]}
    if url.endswith("/api/nodes.json"):
        return {"kill": False, "counts": {"nodes": 1, "healthy": 1}, "nodes": [
            {"id": NID, "host": "node.example.org", "region": "eu", "healthy": True, "rtt_ms": 31.5,
             "caps": {"streams": 40}, "load": {"streams": 6}}]}
    raise OSError("no")


check("the Network tab is in HQ's menu", ("network", "Network") in A.HQ_TABS)
for a in ("node.block", "node.unblock", "nodes.off", "nodes.on"):
    check(f"action {a} registered", a in A.ACTIONS)
o = H.network_overview(fake_fetch)
check("overview counts", (o["realms"], o["live"], o["players"], o["herokey"]) == (3, 2, 5, ["Frost Haven"]), o)
check("overview nodes", o["nodes"]["counts"] == {"nodes": 1, "healthy": 1} and o["nodes"]["rows"][0]["host"] == "node.example.org")
page = H.network_page(owner, fake_fetch)
for want in ("Hero Key realms", "Frost Haven", "node.example.org", "3 / 20 players", "Stop this node", "Stop every node now",
             "herokey_sso", "relay_volunteer", "mesh_relays"):
    check(f"page shows {want}", want in page, page[:300])
check("a mod sees no stop buttons", "Stop this node" not in H.network_page(mod, fake_fetch))
check("an admin can't stop every node", "Stop every node now" not in H.network_page(admin, fake_fetch))
broken = H.network_overview(lambda u: (_ for _ in ()).throw(OSError("down")))
check("a relay that doesn't answer is said plainly", len(broken["errors"]) == 2 and broken["nodes"] is None)

# a realm block first: stopping a node must keep it
A.set_relay_block(admin, RID, True, "spam")
ok, msg = run(admin, "node.block", NID, reason="", confirm=NID)
check("a reason is needed", not ok, msg)
ok, msg = run(admin, "node.block", NID, reason="relays junk", confirm="nope")
check("the typed confirm is needed", not ok, msg)
ok, msg = run(admin, "node.block", NID, reason="relays junk", confirm=NID)
check("an admin stops a node", ok, msg)
ok, msg = run(mod, "node.block", NID, reason="x", confirm=NID)
check("a mod can't", not ok)
ok, msg = run(admin, "node.block", "n-../../etc", reason="x", confirm="n-../../etc")
check("a node id is checked", not ok)
feed = A.relay_block_feed()
check("the feed carries the node, without the reason", [n["id"] for n in feed["nodes"]] == [NID]
      and "relays junk" not in json.dumps(feed) and feed["nodes_off"] is False, feed)
check("the realm block stays", [b["id"] for b in feed["blocked"]] == [RID], feed)
ok, msg = run(admin, "nodes.off", "all", confirm="all")
check("only the owner stops every node", not ok)
ok, msg = run(owner, "nodes.off", "all", confirm="all")
check("the owner stops every node", ok and A.relay_block_feed()["nodes_off"] is True, msg)
ok, msg = run(owner, "nodes.on", "all")
check("and lets them carry again", ok and A.relay_block_feed()["nodes_off"] is False, msg)
ok, msg = run(admin, "node.unblock", NID)
check("a stopped node is let back", ok and A.relay_block_feed().get("nodes", []) == [], msg)
A.set_relay_block(admin, RID, False)
check("a realm-only feed keeps its old shape", "nodes" not in A.relay_block_feed() or A.relay_block_feed()["nodes"] == [])

# the relay takes the feed (hosting/relay/wurelay): the same document stops the node there
sys.path[:0] = [os.path.join(STREAM, "..", "hosting", "kit", "tools"), os.path.join(STREAM, "..", "hosting", "relay")]
from wurelay import nodes  # noqa: E402
import types  # noqa: E402
reg = nodes.Registry(types.SimpleNamespace(), fetch=lambda *a: (0, None))
run(admin, "node.block", NID, reason="relays junk", confirm=NID)
reg.take_feed(A.relay_block_feed())
check("the relay's registry reads HQ's feed", reg.banned == {NID} and reg.kill is False)

print(f"\n{passed} passed, {failed} failed")
sys.exit(1 if failed else 0)
