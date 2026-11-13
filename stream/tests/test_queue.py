"""The browser queue (docs/QUEUE.md): streamed seats (WU_PLAY_CAPACITY), the fair line past them, its page and its
JSON, against a fake AzerothCore SOAP server and a fake realm service.

    python3 -I stream/tests/test_queue.py stream/wrath_portal.py

Seats: one per account with a streamed session whose game asked for something lately (own game files take none). Past
capacity /wrath/play shows the queue page (no session, no realm mark); one place per account and a few per address;
first come, first served; a freed seat is held for the first in line, who then gets in through the same /wrath/play
checks; a place whose page stops asking, or a held seat not taken, is dropped. The wu_q cookie is only a turn: never
a session, never another account's place. GET /wrath/api/queue and status.json's "browser_queue" give the numbers.
"""
import re
import http.client, importlib.util, json, os, re, sys, tempfile, threading, time, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from xml.sax.saxutils import escape

SOAP_PORT, REALM_PORT, PAGE_PORT = 17989, 18290, 18292
slices = tempfile.mkdtemp()
with open(os.path.join(slices, "mini.json"), "w") as f:
    json.dump({"slice": "mini", "maxTier": 4, "title": "Mini"}, f)
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL=f"http://127.0.0.1:{SOAP_PORT}/",
                  PORTAL_URL=f"http://127.0.0.1:{REALM_PORT}", WU_SLICE_DIR=slices, WU_STREAM="1", WU_OWNFILES="1",
                  STREAM_HOST="wrath.example", WU_PLAY_CAPACITY="2", WU_QUEUE_PER_IP="2")
for k in ("WU_QUEUE_MAX", "WU_QUEUE_BEAT", "WU_QUEUE_HOLD", "WU_PLAY_IDLE", "WU_QUEUE_SEAT_MIN"):
    os.environ.pop(k, None)

spec = importlib.util.spec_from_file_location("wp", sys.argv[1])
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)

soap = {"commands": []}
PASSWORD = "Pa55word1234abcd"


class Soap(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_POST(self):
        cmd = wp._tag_text(self.rfile.read(int(self.headers["Content-Length"])), "command")
        soap["commands"].append(cmd)
        ok, text = (False, "Account with this name already exist!") if cmd.startswith("account create") else \
            (True, "ovstream ok" if cmd.startswith("ovstream") else "done")
        inner = (f"<ns1:executeCommandResponse><result>{escape(text)}</result></ns1:executeCommandResponse>" if ok else
                 f"<SOAP-ENV:Fault><faultcode>SOAP-ENV:Client</faultcode><faultstring>{escape(text)}</faultstring></SOAP-ENV:Fault>")
        body = ('<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" '
                f'xmlns:ns1="urn:AC"><SOAP-ENV:Body>{inner}</SOAP-ENV:Body></SOAP-ENV:Envelope>').encode()
        self.send_response(200 if ok else 500)
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)


class Realm(BaseHTTPRequestHandler):
    """wr_session=p<N> is the player WRQ0000<N>."""
    def log_message(self, *a): pass

    def do_GET(self):
        m = re.search(r"wr_session=p(\d+)", self.headers.get("Cookie", ""))
        if not m:
            self.send_response(401); self.send_header("Content-Length", "0"); self.end_headers(); return
        n = int(m.group(1))
        doc = {"/api/me": {"display_name": f"Tester {n}"}, "/api/play": {"user": f"WRQ{n:05d}", "pass": PASSWORD}}[self.path]
        body = json.dumps(doc).encode()
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)


for port, h in ((SOAP_PORT, Soap), (REALM_PORT, Realm), (PAGE_PORT, wp.PageHandler)):
    srv = ThreadingHTTPServer(("127.0.0.1", port), h)
    threading.Thread(target=srv.serve_forever, daemon=True).start()


def req(method, path, cookie="", headers=None, body=None, ip="10.8.0.1"):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=20)
    h = {"X-Forwarded-For": ip}
    if cookie:
        h["Cookie"] = cookie
    h.update(headers or {})
    c.request(method, path, body=body, headers=h)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra else ""))
    fails += 0 if cond else 1


def play(n, ip, q=""):
    """/wrath/play for player n: (status, headers, body)."""
    return req("GET", "/wrath/play?mini=mini", f"wr_session=p{n}" + (f"; wu_q={q}" if q else ""), ip=ip)


def gate(h, ip):
    code = urllib.parse.parse_qs(urllib.parse.urlsplit(h["location"]).query)["code"][0]
    st, h2, _ = req("GET", "/__gate?code=" + urllib.parse.quote(code), ip=ip)
    return h2["set-cookie"].split(";")[0]


def qcookie(h):
    m = re.match(r"wu_q=([A-Za-z0-9_-]+);", h.get("set-cookie", ""))
    return m.group(1) if m else ""


def api(token="", me=True, ip="10.8.9.9"):
    st, h, b = req("GET", "/wrath/api/queue" + ("?me=1" if me else ""), f"wu_q={token}" if token else "", ip=ip)
    return st, h, json.loads(b)


BRANDS = re.compile(rb"world of warcraft|\bwow\b|warcraft|lich king|blizzard", re.I)

# --- settings
check("capacity from WU_PLAY_CAPACITY", wp.PLAY_CAPACITY == 2)
os.environ.pop("WU_PLAY_CAPACITY", None)
spec2 = importlib.util.spec_from_file_location("wp_default", sys.argv[1])
wp2 = importlib.util.module_from_spec(spec2); spec2.loader.exec_module(wp2)
check("default capacity: the launch number, 100 (owner 4 Nov)", wp2.PLAY_CAPACITY == 100)
check("default loading limit: 30 new seats per 120 s", (wp2.PLAY_LOADING, wp2.PLAY_LOAD_S) == (30, 120))
os.environ["WU_PLAY_CAPACITY"] = "nonsense"
check("a bad value falls back to the default", wp2._env_int("WU_PLAY_CAPACITY", 5000, 0, 5000) == 5000)
os.environ["WU_PLAY_CAPACITY"] = "-4"
check("a negative value is 0 (no line)", wp2._env_int("WU_PLAY_CAPACITY", 5000, 0, 5000) == 0)
os.environ["WU_PLAY_CAPACITY"] = "2"
check("defaults: idle 600 s, a 180 s reconnect grace for a place, hold 120 s, line 500",
      (wp.PLAY_IDLE, wp.QUEUE_BEAT, wp.QUEUE_HOLD, wp.QUEUE_MAX) == (600, 180, 120, 500))

# --- two seats fill
st, h, _ = play(1, "10.8.0.1")
check("player 1: in (a code for the game site)", st == 302 and "/__gate?code=" in h["location"])
c1 = gate(h, "10.8.0.1")
st, h, _ = play(2, "10.8.0.2")
check("player 2: in", st == 302 and "/__gate?code=" in h["location"])
c2 = gate(h, "10.8.0.2")
st, _, doc = api(me=False)
check("api: nobody waiting, and never the seat numbers", doc["waiting"] == 0 and "capacity" not in doc and "in_use" not in doc, doc)
check("inside: capacity 2, in use 2", (wp.queue_status()["capacity"], wp.queue_status()["in_use"]) == (2, 2))
st, h, _ = play(1, "10.8.0.1")
check("a seated account playing again (a reload, another tab) never queues", st == 302 and "/__gate?code=" in h["location"])
c1 = gate(h, "10.8.0.1")

# --- the third waits
n = len(soap["commands"])
st, h, b = play(3, "10.8.0.3")
q3 = qcookie(h)
check("player 3: the queue page (200, no code)", st == 200 and "location" not in h and b"__gate" not in b)
check("player 3: nothing sent to the realm (no account mark while waiting)", not [c for c in soap["commands"][n:] if c.startswith("ovstream")])
check("player 3: a place cookie, HttpOnly Secure SameSite=Lax under /wrath", bool(q3) and all(
    x in h["set-cookie"] for x in ("HttpOnly", "Secure", "SameSite=Lax", "Path=/wrath")), h.get("set-cookie"))
check("player 3: no play session cookie", "wu_play" not in h.get("set-cookie", ""))
check("queue page: You're #1 in line, about N min", b"You're #1 in line" in b and re.search(rb"About \d+ min", b) is not None)
check("queue page: Skip the line with its choices", b"Skip the line" in b and all(
    f'id="{i}"'.encode() in b for i in ("q-pc", "q-host", "q-seed")))
check("queue page: Become a node only while the be_a_node flag is on (off here: audit 12 Nov)",
      (b'id="q-node"' in b) == wp.be_a_node_on() and (b'href="/wrath/docs/be-a-node"' in b) == wp.be_a_node_on())
check("queue page: seed a server -> the roadmap's seeded servers", b"#seeded-servers" in b)
check("queue page: own game folder in the browser needs no seat", b'href="/wrath/play?mode=own"' in b)
check("queue page: leave the line", b'id="q-leave"' in b)
check("queue page: the Leyline look (ly_shell)", b'<body class="ly">' in b)
check("queue page: not indexed", b'name="robots" content="noindex"' in b)
nonce = re.search(rb'<script nonce="([^"]+)">', b)
csp = h.get("content-security-policy", "")
check("queue page: its script runs under the page's nonce, may only fetch this site", nonce and f"'nonce-{nonce.group(1).decode()}'" in csp
      and "connect-src 'self'" in csp, csp)
check("queue page: a reload without script", b'<noscript><meta http-equiv="refresh" content="30;url=/wrath/play?mini=mini">' in b)
check("queue page: no password, no game login", PASSWORD.encode() not in b and b"WRQ00003" not in b)
check("queue page: no game-brand names", not BRANDS.search(b.replace(b"not affiliated with or endorsed by Blizzard Entertainment", b" ")), BRANDS.findall(b))
check("queue page: no money links", not re.search(rb"donat|patreon|ko-fi|paypal|tip jar", b, re.I))

st, h, b = play(3, "10.8.0.3", q3)
check("player 3 again: the same place, still #1 (one place per account)", st == 200 and b"You're #1 in line" in b
      and qcookie(h) == q3 and len(wp.play_queue.line) == 1)
st, h, b = play(3, "10.8.0.33")
check("player 3 from another address: still the one place", len(wp.play_queue.line) == 1 and b"You're #1" in b)
st, h, b = play(4, "10.8.0.4")
q4 = qcookie(h)
check("player 4: #2", b"You're #2 in line" in b and q4 and q4 != q3)
st, h, b = play(5, "10.8.0.4")
check("player 5, the same address: #3 (two per address here)", b"You're #3 in line" in b)
q5 = qcookie(h)
st, h, b = play(6, "10.8.0.4")
check("player 6, a third from that address: refused a place, no cookie, told why", st == 200 and not qcookie(h)
      and b"already in line" in b and len(wp.play_queue.line) == 3)
check("refused page: tries again by itself", b'<meta http-equiv="refresh" content="60;url=/wrath/play?mini=mini">' in b)

# --- the JSON
st, h, doc = api(me=False)
check("api: waiting, a wait, a line, no seat numbers", (doc["waiting"], doc["line"]) == (3, True) and doc["wait_min"] >= 1 and "capacity" not in doc and "in_use" not in doc, doc)
check("api: public numbers, readable from anywhere", h.get("access-control-allow-origin") == "*" and "you" not in doc)
st, h, doc = api(q4)
check("api ?me=1: this browser's place", doc["you"]["state"] == "waiting" and doc["you"]["position"] == 2
      and doc["you"]["wait_min"] >= 1 and doc["you"]["go"] == "/wrath/play?mini=mini", doc)
check("api ?me=1: no-store, same-origin", h.get("cache-control") == "no-store" and h.get("cross-origin-resource-policy") == "same-origin")
check("api: no account names, no tokens of others", b"WRQ" not in json.dumps(doc).encode() and q3 not in json.dumps(doc))
st, h, doc = api("A" * 32)
check("api: an unknown place is none", doc["you"] is None)
st, h, b = req("GET", "/wrath/api/queue?me=1", "wu_q=../../etc", ip="10.8.9.9")
check("api: a malformed cookie is ignored", st == 200 and json.loads(b)["you"] is None)
st, h, b = req("GET", "/wrath/status.json")
bq = json.loads(b).get("browser_queue")
check("status.json: browser_queue for the launcher, no seat numbers", bq and bq["waiting"] == 3 and "capacity" not in bq and "in_use" not in bq, bq)

# --- a place is only a turn
st, h, b = req("GET", "/wrath/play?mini=mini", f"wu_q={q3}", ip="10.8.0.3")
check("a place cookie without a sign-in: the sign-in page, no session", st == 200 and b"__gate" not in b and "wu_play" not in h.get("set-cookie", ""))
st, h, b = req("GET", "/__auth", f"wu_q={q3}", ip="10.8.0.3")
check("a place cookie is no game session (/__auth)", st == 302)
st, h, b = req("GET", "/__env.js", f"wu_q={q3}", ip="10.8.0.3")
check("a place cookie gets no game login (/__env.js)", st == 401 and PASSWORD.encode() not in b)

# --- a seat frees: the first in line, and only them
st, h, b = req("GET", "/wrath/play/end", "wr_session=p1", ip="10.8.0.1")
check("player 1 ends the browser pass", st == 200)
check("the ended page: play on your own PC", b'id="end-pc"' in b and b"Play on your own PC" in b)
st, h, doc = api(q3)
check("player 3: ready, the seat held", doc["you"]["state"] == "ready" and doc["you"]["hold_s"] > 60 and doc["you"]["go"].startswith("/wrath/play?mini="), doc)
st, h, doc = api(q4)
check("player 4: now #1, still waiting", doc["you"]["state"] == "waiting" and doc["you"]["position"] == 1, doc)
st, h, b = play(4, "10.8.0.4", q4)
check("player 4 at /wrath/play: still in line (the seat is held for player 3)", st == 200 and b"You're #1" in b)
st, h, b = play(4, "10.8.0.4", q3)
check("player 4 with player 3's place cookie: still player 4's own place", st == 200 and b"You're #1" in b)
st, h, b = play(3, "10.8.0.3", q3)
check("player 3: in, through the usual checks", st == 302 and "/__gate?code=" in h["location"])
check("player 3: the place cookie cleared", h.get("set-cookie", "").startswith("wu_q=;") and "Max-Age=0" in h["set-cookie"])
c3 = gate(h, "10.8.0.3")
check("player 3's place is gone", all(t["user"] != "WRQ00003" for t in wp.play_queue.line))
st, h, doc = api(me=False)
check("api: 2 waiting (2 in use inside)", doc["waiting"] == 2 and wp.queue_status()["in_use"] == 2, doc)

# --- a seat whose game went quiet frees itself
sid2 = c2.split("=", 1)[1]
with wp.sessions.lock:
    wp.sessions.seen[sid2] = time.time() - wp.PLAY_IDLE - 5
st, h, doc = api(q4)
check("player 2 quiet past WU_PLAY_IDLE: player 4 ready", doc["you"]["state"] == "ready" and wp.queue_status()["in_use"] == 1, doc)
st, h, b = req("GET", "/__env.js", c2, ip="10.8.0.2")
check("game traffic marks the seat taken again (/__env.js)", wp.sessions.seen[sid2] > time.time() - 5)
st, h, b = req("GET", "/__auth", c2, ip="10.8.0.2", headers={"X-Forwarded-Uri": "/data/x"})
check("and /__auth (the game's data and its realm connection)", st == 200 and wp.sessions.seen[sid2] > time.time() - 5)
st, h, b = play(5, "10.8.0.4", q5)
check("player 5 meanwhile: still waiting (#1 behind the held seat)", b"You're #1 in line" in b)

# --- leaving
st, h, b = req("POST", "/wrath/api/queue/leave", f"wu_q={q5}", {"Content-Type": "application/json"}, b"{}", ip="10.8.0.4")
check("leave without an Origin: 403", st == 403 and any(t["token"] == q5 for t in wp.play_queue.line))
st, h, b = req("POST", "/wrath/api/queue/leave", f"wu_q={q5}", {"Origin": "https://evil.example", "Content-Type": "application/json"}, b"{}", ip="10.8.0.4")
check("leave from another site: 403", st == 403)
st, h, b = req("POST", "/wrath/api/queue/leave", f"wu_q={q5}", {"Origin": "https://play.openvanilla.app", "Content-Type": "application/json"}, b"{}", ip="10.8.0.4")
check("leave: the place is gone, the cookie cleared", st == 200 and all(t["token"] != q5 for t in wp.play_queue.line)
      and h.get("set-cookie", "").startswith("wu_q=;"))

# --- own game files skip the line
st, h, b = play(7, "10.8.0.7")
check("player 7 waits", st == 200 and qcookie(h))
st, h, b = req("GET", "/wrath/play?mode=own", "wr_session=p7", ip="10.8.0.7")
check("player 7 picks own game files: in at once, and out of the line", st == 302 and "/__gate?code=" in h["location"]
      and all(t["user"] != "WRQ00007" for t in wp.play_queue.line))
c7 = gate(h, "10.8.0.7")
req("GET", "/__env.js", c7, ip="10.8.0.7")
check("an own-files session takes no streamed seat", "WRQ00007" not in wp.seated_now())

# --- the line itself, by the clock (PlayQueue)
Q = wp.PlayQueue()
t0 = 1_000_000.0
full = {"A", "B"}
a = Q.admit("u1", "ip1", "", "mini", "", full, t0)
b_ = Q.admit("u2", "ip2", "", "mini", "", full, t0 + 1)
c_ = Q.admit("u3", "ip3", "", "mini", "", full, t0 + 2)
check("FIFO: 1, 2, 3", [a["position"], b_["position"], c_["position"]] == [1, 2, 3])
Q.poll(b_["token"], 2, t0 + 120); Q.poll(c_["token"], 2, t0 + 120)
check("a page gone quiet for most of the grace (a reconnect) keeps its place", Q.poll(a["token"], 2, t0 + wp.QUEUE_BEAT - 5)["position"] == 1)
Q.poll(b_["token"], 2, t0 + 240); Q.poll(c_["token"], 2, t0 + 240)
check("a place nobody asks about is dropped after the grace (WU_QUEUE_BEAT)", Q.poll(a["token"], 2, t0 + 2 * wp.QUEUE_BEAT + 1) is None)
check("the others moved up (they asked)", Q.poll(b_["token"], 2, t0 + 362)["position"] == 1 and Q.poll(c_["token"], 2, t0 + 362)["position"] == 2)
t0 += 330
v = Q.poll(b_["token"], 1, t0 + 40)
check("a seat frees: the first ready", v["state"] == "ready")
check("the next is #1, not let in", Q.poll(c_["token"], 1, t0 + 41)["state"] == "waiting" and Q.poll(c_["token"], 1, t0 + 41)["position"] == 1)
check("a newcomer while the seat is held: in line, not in", Q.admit("u9", "ip9", "", "mini", "", {"A"}, t0 + 42)["state"] == "waiting")
Q.poll(c_["token"], 1, t0 + 100)   # its page keeps asking
Q.poll(c_["token"], 1, t0 + 40 + wp.QUEUE_HOLD + 1)
check("a held seat not taken in WU_QUEUE_HOLD: that place is dropped, the next is ready",
      all(t["user"] != "U2" for t in Q.line) and Q.poll(c_["token"], 1, t0 + 40 + wp.QUEUE_HOLD + 2)["state"] == "ready")
g = Q.admit("u3", "ip3", "", "mini", "", {"A"}, t0 + 40 + wp.QUEUE_HOLD + 3)
check("the ready one at /wrath/play: go", g["state"] == "go" and g["used"] == c_["token"])
check("a seated account: go, and its place is dropped", Q.admit("A", "ip", "", "mini", "", {"A", "B"}, t0 + 300)["state"] == "go")
E = wp.PlayQueue()
check("an empty line and a free seat: straight in", E.admit("x", "i", "", "mini", "", {"A"}, t0)["state"] == "go" and not E.line)
cap = wp.PLAY_CAPACITY
wp.PLAY_CAPACITY = 0
check("WU_PLAY_CAPACITY=0: no line at all", E.admit("y", "i", "", "mini", "", set(range(9999)), t0)["state"] == "go")
wp.PLAY_CAPACITY = cap
F = wp.PlayQueue()
for i in range(12):
    F.admit(f"w{i}", f"i{i}", "", "mini", "", {"A", "B"}, t0)
guess = F.status(2, t0)["wait_min"]
check("the first wait guess: WU_QUEUE_SEAT_MIN per seat (12 waiting, 2 seats, 30 min: about 3 h)", 170 <= guess <= 200, guess)
F.freed.extend([t0 - 600, t0 - 400, t0 - 200, t0 - 100, t0 - 10])
check("then from how fast the line really moved (5 in 10 min: 12 waiting is about 26 min)", 20 <= F.status(2, t0)["wait_min"] <= 30, F.status(2, t0))
mx = wp.QUEUE_MAX
wp.QUEUE_MAX = 12
check("the line full: told so, no place", F.admit("late", "il", "", "mini", "", {"A", "B"}, t0)["state"] == "full")
wp.QUEUE_MAX = mx
G = wp.PlayQueue()
gv = G.admit("u", "i", "", "mini", "&go=r:abc", {"A", "B"}, t0)
check("a deep link (go) is kept for the way back in", G.poll(gv["token"], 2, t0)["go"] == "/wrath/play?mini=mini&go=r:abc")

# --- the gate and the rest are unchanged
st, h, _ = req("GET", "/__gate?code=bogus")
check("GET /__gate unchanged", st == 302 and h["location"] == "https://play.openvanilla.app/wrath")

# --- the loading limit (owner 4 Nov, ready for a spike): at most PLAY_LOADING new seats per PLAY_LOAD_S seconds
saved = (wp.PLAY_CAPACITY, wp.PLAY_LOADING, wp.PLAY_LOAD_S)
wp.PLAY_CAPACITY, wp.PLAY_LOADING, wp.PLAY_LOAD_S = 100, 3, 120
q = wp.PlayQueue()
t0 = 1_000_000.0
seated = set()
got = []
for i in range(5):
    r = q.admit(f"WRL{i:05d}", f"10.9.0.{i}", "", "mini", "", seated, now=t0 + i)
    got.append(r["state"])
    if r["state"] == "go":
        seated.add(f"WRL{i:05d}")
check("loading limit: 3 start at once, the next wait though seats are free", got == ["go", "go", "go", "waiting", "waiting"], got)
r = q.admit("WRL00003", "10.9.0.3", "", "mini", "", seated, now=t0 + 60)
check("loading limit: still in line a minute later", r["state"] == "waiting", r)
r = q.admit("WRL00003", "10.9.0.3", "", "mini", "", seated, now=t0 + 125)
check("loading limit: once the first ones have loaded, the next go in", r["state"] == "go", r)
seated.add("WRL00003")
r = q.admit("WRL00000", "10.9.0.0", "", "mini", "", seated, now=t0 + 126)
check("loading limit: a seated player reloading never waits", r["state"] == "go", r)
wp.PLAY_LOADING = 0
q2 = wp.PlayQueue()
st2 = [q2.admit(f"WRM{i:05d}", f"10.9.1.{i}", "", "mini", "", set(), now=t0)["state"] for i in range(10)]
check("WU_PLAY_LOADING=0: no loading limit", all(x == "go" for x in st2), st2)
wp.PLAY_CAPACITY, wp.PLAY_LOADING, wp.PLAY_LOAD_S = saved

# ---- the waiting page itself (fix round 4 Nov): the place in the tab, back at once from the background, Leave asks
qp = wp.queue_page({"state": "line", "position": 3, "wait_min": 7, "token": "t"}, "n0nce", "/wrath/play").decode("utf-8")
check("queue page: the tab title carries the place", "<title>(#3) In line · Leyline</title>" in qp, qp[qp.find("<title>"):qp.find("</title>") + 8])
js = qp.split('<script nonce="n0nce">')[-1].split("</script>")[0]
check("queue page: asks at once when the page comes back (visibilitychange), says Your turn in the tab, Leave asks first",
      "visibilitychange" in js and "Your turn!" in js and "confirm(" in js and "In line" in js)
check("queue page: no seat numbers (owner 4 Nov)", "seats" not in qp.lower() and f"of {wp.PLAY_CAPACITY}" not in qp)
import shutil as _sh, subprocess as _sp
if _sh.which("node"):
    r = _sp.run(["node", "-e", "new Function(require('fs').readFileSync(0,'utf8'))"], input=js, capture_output=True, text=True)
    check("queue page: its script parses", r.returncode == 0, r.stderr[:300])
print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
