"""The browser game's review checks (4 Nov 2026): the browser queue's seats between /wrath/play and the game's first
request (PlayQueue.pending, WU_PLAY_PENDING), and the lists that must agree across the page, the service worker and
Caddy: what counts as world data (kept a week / kept by wrath-sw.js / kept across slices by sections.js) and the
client files the realm list's warm-up fetches past the sign-in gate (stream-up.sh @client).

    python3 -I stream/tests/test_stream_review.py stream/wrath_portal.py
"""
import importlib.util, json, os, re, sys, tempfile

PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
REPO = os.path.dirname(STREAM)
slices = tempfile.mkdtemp()
with open(os.path.join(slices, "mini.json"), "w") as f:
    json.dump({"slice": "mini", "maxTier": 4, "title": "Mini"}, f)
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_SLICE_DIR=slices, WU_STREAM="1", STREAM_HOST="wrath.example", WU_PLAY_CAPACITY="2",
                  WU_PLAY_LOADING="30", WU_PLAY_LOAD_S="120", WU_DL_PROBE="0", WU_PORTAL_DATA=os.path.join(slices, "data"))
for k in ("WU_QUEUE_MAX", "WU_QUEUE_BEAT", "WU_QUEUE_HOLD", "WU_PLAY_IDLE", "WU_QUEUE_SEAT_MIN", "WU_PLAY_PENDING"):
    os.environ.pop(k, None)
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)

fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra else ""))
    fails += 0 if cond else 1


# --- the browser queue: a seat just handed out is taken until its game shows up -------------------------------------
check("default: 90 s for a new seat's game to show up", wp.PLAY_PENDING == 90)
t0 = 1_000_000.0
q = wp.PlayQueue()
none = set()   # nobody's game has reached the stream site yet (they are all between /wrath/play and /__gate)
got = [q.admit(u, f"ip{i}", "", "mini", "", none, t0 + i * 0.2)["state"] for i, u in enumerate(["A", "B", "C", "D"])]
check("a full server in a burst: 2 seats, 2 in, the others in line (not 4 in before any game asked)",
      got == ["go", "go", "waiting", "waiting"], got)
check("the status counts the seats just handed out", q.status(none, t0 + 1)["in_use"] == 2, q.status(none, t0 + 1))
check("a player let in reloading /wrath/play before the game asked: in again, no line",
      q.admit("A", "ip0", "", "mini", "", none, t0 + 5)["state"] == "go")
# A's game arrives (seated); B never does (closed the tab right after Play)
seated = {"A"}
check("its game seen: still 2 in use, not 3 (seated is not pending twice)", q.status(seated, t0 + 10)["in_use"] == 2,
      q.status(seated, t0 + 10))
c = [t for t in q.line if t["user"] == "C"][0]
check("C still waiting while B's seat may still be loading", q.poll(c["token"], seated, t0 + 60)["state"] == "waiting")
v = q.poll(c["token"], seated, t0 + wp.PLAY_PENDING + 2)
check("B never showed up: after WU_PLAY_PENDING its seat goes to the first in line", v and v["state"] == "ready", v)
check("C at /wrath/play: in", q.admit("C", "ip2", "", "mini", "", seated, t0 + wp.PLAY_PENDING + 3)["state"] == "go")
q.drop_user("C")
check("drop_user (own game files, End my pass): the pending seat goes too", "C" not in q.pending)
# the old entry points (a number of seated accounts) still work
check("status with a number still works", q.status(1, t0 + 500)["capacity"] == 2)

# --- what is world data: one list in three places ---------------------------------------------------------------------
sw = open(os.path.join(STREAM, "www-files", "wrath-sw.js"), encoding="utf-8").read()
keep = re.search(r"const KEEP = \[([^\]]*)\]", sw).group(1)
sw_set = {x.strip(" '\"/") for x in keep.split(",") if x.strip()}
up = open(os.path.join(STREAM, "server", "stream-up.sh"), encoding="utf-8").read()
caddy = re.search(r"@dataworld path_regexp dataworld \(\?i\)\^/data/\(([^)]*)\)", up).group(1)
caddy_set = set(caddy.split("|"))
sec = open(os.path.join(STREAM, "www-files", "sections.js"), encoding="utf-8").read()
m = re.search(r"const WORLD_DATA = /\^\(([^)]*)\)", sec)
sec_set = set(m.group(1).split("|")) if m else set()
check("world data: wrath-sw.js KEEP == stream-up.sh @dataworld", sw_set == caddy_set, sw_set ^ caddy_set)
check("world data: sections.js WORLD_DATA == stream-up.sh @dataworld", sec_set == caddy_set, sec_set ^ caddy_set)
other = re.search(r"@dataother \{(.*?)\n\t\t\}", up, re.S).group(1)
check("@dataother leaves out exactly the @dataworld prefixes", caddy in other)

# --- the realm list's warm-up: every client file warm.html fetches skips the gate -------------------------------------
client = re.search(r"@client path ([^\n]+)", up).group(1).split()
patch = open(os.path.join(REPO, "client", "patches", "wotlk-web-port-wip.patch"), encoding="utf-8", errors="replace").read()
w = re.search(r"\+\+\+ b/web/warm\.html\n@@[^\n]*\n(.*?)(?:\ndiff --git|\Z)", patch, re.S)
warm = w.group(1) if w else ""
wanted = set(re.findall(r"'\./([\w.-]+\.(?:js|json))'", warm)) | {"warm.html", "wenilla_bg.wasm"}
check("warm.html found in the client patch", bool(warm) and "boot.js" in wanted, sorted(wanted))
missing = sorted(f for f in wanted if "/" + f not in client)
check("every file the warm-up fetches is in @client (else it gets the sign-in redirect, and nothing is warmed)",
      not missing, missing)
check("@client holds no game data path", not [p for p in client if p.startswith("/data")], client)

print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
