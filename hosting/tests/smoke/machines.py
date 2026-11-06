"""Hosting smoke test as separate machines: the relay, the host's PC and the players are separate OS processes,
each with its own address (127.0.0.1 relay, 127.0.0.2 the host's PC, 127.0.0.3/.4/.9 players), so the relay's
per-address rules (knocks, private realms, limits) are tested between different "computers", which the in-process
rigs can't do (every player there is 127.0.0.1). No Docker, no game data: the realm is tests/fake_realm.py (the
3.3.5a login and world handshake), the relay, agent and client are the real code.

  python3 hosting/tests/smoke/machines.py          # runs everything, prints PASS/FAIL lines and a JSON summary

The host's PC opens no port players can use: its fake realm listens on 127.0.0.2 and only the agent (outbound to
the relay, like behind a home router or CGNAT) talks to it. Players only ever dial the relay.

Phases:
  1. private realm: a player who opened the invite (127.0.0.3) makes an account and plays (per-realm port, shared
     port, browser WebSocket); a stranger at another address (127.0.0.9) is refused on every path, also when the
     first player is still knocked; the realm's own address never reaches a player.
  2. public: the host flips to public; a fresh address (127.0.0.4) plays with no invite; it is in the listing.
"""
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
TESTS = os.path.dirname(HERE)
ROOT = os.path.dirname(TESTS)
SRC = {"relay": "127.0.0.1", "host": "127.0.0.2"}


def bind_source(ip):
    """Every outgoing IPv4 socket of this process leaves from `ip` (a second computer on the loopback /8)."""
    orig = socket.socket.connect
    orig_ex = socket.socket.connect_ex

    def pre(sock):
        if sock.family == socket.AF_INET:
            try:
                if sock.getsockname()[1] == 0:
                    sock.bind((ip, 0))
            except OSError:
                pass

    def connect(self, addr):
        pre(self)
        return orig(self, addr)

    def connect_ex(self, addr):
        pre(self)
        return orig_ex(self, addr)

    socket.socket.connect = connect
    socket.socket.connect_ex = connect_ex


def free_port(ip="127.0.0.1"):
    s = socket.socket()
    s.bind((ip, 0))
    p = s.getsockname()[1]
    s.close()
    return p


# ------------------------------------------------------------------------------------------------- the host's PC
def host_main(state_dir, relay_http):
    bind_source(SRC["host"])
    sys.path[:0] = [os.path.join(ROOT, "kit", "tools"), os.path.join(ROOT, "relay"), TESTS]
    import asyncio
    from helpers import FakeDB
    from fake_realm import FakeRealm
    from wuhost import agent as agentmod

    async def run():
        realm = await FakeRealm().start(host=SRC["host"])
        db = FakeDB(realm)
        a = agentmod.Agent(state_dir, db=db, services={3724: (SRC["host"], realm.auth_port),
                                                       8085: (SRC["host"], realm.world_port)})
        a.heartbeat_every = 1

        async def ready():   # the realm list carries the relay's address before any player is let at it
            while not a.address_done:
                await asyncio.sleep(0.05)
            with open(os.path.join(state_dir, "realm-ports.txt"), "w") as f:
                f.write(f"{realm.auth_port} {realm.world_port}\n")
        asyncio.ensure_future(ready())
        await a.run()

    asyncio.run(run())


# ------------------------------------------------------------------------------------------------- orchestration
def http(url, body=None, src=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"},
                                 method="POST" if data else "GET")
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            return r.status, json.loads(r.read() or b"null")
    except urllib.error.HTTPError as e:
        return e.code, None


def player(ip, *args, timeout=240):
    """The docker e2e's player script (tests/e2e/e2e_client.py), run as a computer at `ip`."""
    code = (f"import sys,runpy; sys.path.insert(0,{HERE!r}); import machines; machines.bind_source({ip!r}); "
            f"sys.argv=['e2e_client.py']+{list(args)!r}; runpy.run_path({os.path.join(TESTS, 'e2e', 'e2e_client.py')!r}, run_name='__main__')")
    p = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, timeout=timeout)
    return p.returncode, p.stdout + p.stderr


STRANGER = r'''
import asyncio, json, sys
sys.path[:0] = [{tools!r}, {tests!r}]
from helpers import ws_opener
from wunet import ws, wire335
rid, host, ap, shared, http_port, wp = {rid!r}, "127.0.0.1", {ap}, {shared}, {http_port}, {wp}
async def tcp_refused(port):
    try:
        await wire335.full_run(host, port, {user!r}, {pw!r}, timeout=8)
        return False
    except Exception:
        return True
async def main():
    out = {{}}
    out["per-realm port"] = await tcp_refused(ap)
    out["shared port"] = await tcp_refused(shared)
    try:
        s = await ws.connect(f"ws://{{host}}:{{http_port}}/r/{{rid}}/ws/3724"); s.abort(); out["websocket"] = False
    except ws.HandshakeError as e:
        out["websocket"] = e.status == 403
    print(json.dumps(out))
asyncio.run(main())
'''


def main():
    tmp = tempfile.mkdtemp(prefix="leyline-machines-")
    results, failed = [], []

    def check(name, ok, detail=""):
        (results if ok else failed).append(name)
        print(("PASS " if ok else "FAIL ") + name + ("" if ok else f"  ({str(detail)[:400]})"), flush=True)

    sys.path[:0] = [os.path.join(ROOT, "kit", "tools"), TESTS]
    from helpers import make_host_state
    http_port, shared = free_port(), free_port()
    lo = 47000 + (os.getpid() % 500) * 4
    env = dict(os.environ, RELAY_NAME="127.0.0.1", RELAY_HTTP_BIND=f"127.0.0.1:{http_port}", RELAY_GAME_BIND="127.0.0.1",
               RELAY_SHARED_AUTH_PORT=str(shared), RELAY_PORT_MIN=str(lo), RELAY_PORT_MAX=str(lo + 3),
               RELAY_STATE=os.path.join(tmp, "relay", "state.json"), RELAY_LISTING="1",
               RELAY_LISTING_SITE_SECRET="smoke-site", RELAY_MAX_STREAMS_PER_IP="64",
               RELAY_MAX_STREAMS_PER_IP_TOTAL="64", RELAY_CONN_PER_MIN_PER_IP="1000",
               RELAY_ACCOUNTS_PER_IP_HOUR="20", LOG_LEVEL="WARNING", PYTHONPATH=os.pathsep.join(
                   [os.path.join(ROOT, "relay"), os.path.join(ROOT, "kit", "tools")]))
    procs = []
    try:
        relay_log = open(os.path.join(tmp, "relay.log"), "w")
        procs.append(subprocess.Popen([sys.executable, "-m", "wurelay.relay"], env=env, stdout=relay_log,
                                      stderr=subprocess.STDOUT))
        base = f"http://127.0.0.1:{http_port}"
        for _ in range(100):
            try:
                if http(base + "/healthz")[0] == 200:
                    break
            except Exception:  # noqa: BLE001
                time.sleep(0.1)
        host_dir = os.path.join(tmp, "host")
        os.makedirs(host_dir)
        cfg, _ = make_host_state(host_dir, name="Smoke Realm", visibility="private", relay_url=f"ws://127.0.0.1:{http_port}/agent",
                                 listing_url=base + "/api/listing/heartbeat")
        rid = cfg["realm_id"]
        host_log = open(os.path.join(tmp, "host.log"), "w")
        procs.append(subprocess.Popen([sys.executable, os.path.abspath(__file__), "host", host_dir, base],
                                      stdout=host_log, stderr=subprocess.STDOUT))
        info = None
        for _ in range(150):
            st, info = http(f"{base}/api/realms/{rid}")
            if st == 200 and info.get("online") and os.path.exists(os.path.join(host_dir, "realm-ports.txt")):
                break
            time.sleep(0.2)
        check("host's PC (127.0.0.2) is online at the relay through its outbound tunnel only", bool(info and info.get("online")), info)
        ports = open(os.path.join(host_dir, "realm-ports.txt")).read().split()
        acc = os.path.join(tmp, "account.txt")
        common = ["--state", host_dir, "--relay-host", "127.0.0.1", "--relay-http", base, "--shared-port", str(shared)]
        code, out = player("127.0.0.3", "private", *common, "--save-account", acc)
        print("\n".join("  [127.0.0.3] " + ln for ln in out.strip().splitlines()), flush=True)
        check("private phase from the invited computer (127.0.0.3)", code == 0, out[-400:])
        user, pw = (open(acc).read().split(":", 1) + [""])[:2] if os.path.exists(acc) else ("X", "Y")
        st, info = http(f"{base}/api/realms/{rid}")
        code = (STRANGER.format(tools=os.path.join(ROOT, "kit", "tools"), tests=TESTS, rid=rid, ap=info["auth_port"],
                                wp=info["world_port"], shared=shared, http_port=http_port, user=user, pw=pw))
        p = subprocess.run([sys.executable, "-c", f"import sys; sys.path.insert(0,{HERE!r}); import machines; "
                            f"machines.bind_source('127.0.0.9'); exec({code!r})"], capture_output=True, text=True, timeout=120)
        try:
            got = json.loads(p.stdout.strip().splitlines()[-1])
        except (ValueError, IndexError):
            got = {"error": p.stdout + p.stderr}
        for k in ("per-realm port", "shared port", "websocket"):
            check(f"a stranger (127.0.0.9) with a real account is refused on the {k} while 127.0.0.3 is knocked",
                  got.get(k) is True, got)
        st, doc = http(f"{base}/api/listing/realms.json")
        txt = json.dumps(doc)
        check("the private realm is not in the public listing", rid not in txt, txt[:300])
        st, info = http(f"{base}/api/realms/{rid}")
        leaked = [p_ for p_ in ports if f":{p_}" in json.dumps(info)] + (["127.0.0.2"] if "127.0.0.2" in json.dumps(info) else [])
        check("the relay never hands out the host's own address or ports", not leaked, info)
        # phase 2: public
        hj = os.path.join(host_dir, "host.json")
        c = json.load(open(hj))
        c["visibility"] = "public"
        json.dump(c, open(hj, "w"))
        os.utime(hj, (time.time() + 2, time.time() + 2))
        code, out = player("127.0.0.4", "public", *common, "--account", f"{user}:{pw}")
        print("\n".join("  [127.0.0.4] " + ln for ln in out.strip().splitlines()), flush=True)
        check("public phase from a fresh computer (127.0.0.4)", code == 0, out[-400:])
    finally:
        for p_ in procs:
            p_.terminate()
        for p_ in procs:
            try:
                p_.wait(5)
            except subprocess.TimeoutExpired:
                p_.kill()
    rl = open(os.path.join(tmp, "relay.log")).read()
    check("relay log has no invite secret and no player address in clear",
          cfg["invite_secret"] not in rl and "127.0.0.3" not in rl and "127.0.0.9" not in rl, rl[-300:])
    print(json.dumps({"ok": not failed, "passed": len(results), "failed": failed, "logs": tmp}))
    return 0 if not failed else 1


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "host":
        host_main(sys.argv[2], sys.argv[3])
    else:
        sys.exit(main())
