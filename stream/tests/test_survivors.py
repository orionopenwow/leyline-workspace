"""Azeroth Survivors on the site (stream/survivors.py) and its address's gate (wrath_portal.py /__auth?site=survivors):

  * /wrath/survivors: the credit, the key art, "Coming soon" without an address, Play with one, "Resting" while the
    game data killswitch is on; no brand words, no client version numbers, no money links;
  * /wrath/survivors/play: signed out to the sign-in; signed in a one-time code to https://<address>/__gate;
  * the gate on the Survivors address: the cookie, then /__auth?site=survivors 200 for the page and /data;
    without a session 302 (page) / 403 (/data); the killswitch 410 for /data and the manifests;
  * the two sites stay apart: a Survivors session never opens the Wrath stream, a Wrath session never the Survivors
    address; ending a Wrath own-files play does not end a Survivors run;
  * the catalog entry stays "coming" with its start path.

    python3 -I stream/tests/test_survivors.py [stream/wrath_portal.py]
"""
import http.client, importlib.util, json, os, re, sys, tempfile, threading, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from xml.sax.saxutils import escape

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
SOAP_PORT, REALM_PORT, PAGE_PORT = 17989, 18196, 18197
T = tempfile.mkdtemp(prefix="wu-survivors-")
FLAG = os.path.join(T, "app", "gamedata-off")
URLF = os.path.join(T, "app", "survivors.url")
os.makedirs(os.path.dirname(FLAG))
slices = os.path.join(T, "slices"); os.makedirs(slices)
with open(os.path.join(slices, "mini.json"), "w") as f:
    json.dump({"slice": "mini", "maxTier": 4, "title": "Mini"}, f)
os.environ.pop("WU_SURVIVORS_URL", None)
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL=f"http://127.0.0.1:{SOAP_PORT}/",
                  PORTAL_URL=f"http://127.0.0.1:{REALM_PORT}", WU_SLICE_DIR=slices, WU_STREAM="1", WU_OWNFILES="1",
                  STREAM_HOST="wrath.example", WU_GAMEDATA_FLAG=FLAG, WU_PORTAL_DATA=os.path.join(T, "data"),
                  WU_SURVIVORS_URL_FILE=URLF, WU_ART_DIR=os.path.join(STREAM, "www-files", "art"))

fails = 0
count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra else ""))
    fails += 0 if cond else 1


pspec = importlib.util.spec_from_file_location("wp", sys.argv[1] if len(sys.argv) > 1 else os.path.join(STREAM, "wrath_portal.py"))
wp = importlib.util.module_from_spec(pspec); pspec.loader.exec_module(wp)
sv = wp.survivors
check("the portal loads stream/survivors.py", sv is not None)


class Soap(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_POST(self):
        cmd = wp._tag_text(self.rfile.read(int(self.headers["Content-Length"])), "command")
        text = "ovstream ok" if cmd.startswith("ovstream") else "done"
        inner = f"<ns1:executeCommandResponse><result>{escape(text)}</result></ns1:executeCommandResponse>"
        b = ('<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" '
             f'xmlns:ns1="urn:AC"><SOAP-ENV:Body>{inner}</SOAP-ENV:Body></SOAP-ENV:Envelope>').encode()
        self.send_response(200); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)


class Realm(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_GET(self):
        if "wr_session=good" not in self.headers.get("Cookie", ""):
            self.send_response(401); self.send_header("Content-Length", "0"); self.end_headers(); return
        doc = {"/api/me": {"display_name": "Tester"}, "/api/play": {"user": "WR000123", "pass": "Pa55word1234abcd"}}.get(self.path, {})
        b = json.dumps(doc).encode()
        self.send_response(200); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)


for port, h in ((SOAP_PORT, Soap), (REALM_PORT, Realm), (PAGE_PORT, wp.PageHandler)):
    srv = ThreadingHTTPServer(("127.0.0.1", port), h)
    threading.Thread(target=srv.serve_forever, daemon=True).start()

ip_n = [0]


def get(path, cookie="", headers=None):
    ip_n[0] += 1
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=20)
    hd = {"X-Forwarded-For": f"10.8.{ip_n[0] // 200}.{ip_n[0] % 200 + 1}"}
    if cookie:
        hd["Cookie"] = cookie
    hd.update(headers or {})
    c.request("GET", path, headers=hd)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


def text(b):
    """What a player reads: the page without tags, scripts and styles."""
    s = b.decode("utf-8", "replace")
    s = re.sub(r"<(script|style)\b.*?</\1>", " ", s, flags=re.S)
    return re.sub(r"<[^>]+>", " ", s)


def auth(uri, cookie=""):
    st, h, b = get("/__auth?site=survivors", cookie=cookie, headers={"X-Forwarded-Uri": uri})
    return st, h, b


def gate(loc):
    code = urllib.parse.parse_qs(urllib.parse.urlsplit(loc).query)["code"][0]
    st, h, _ = get("/__gate?code=" + urllib.parse.quote(code))
    return st, h.get("location"), h.get("set-cookie", "")


BRAND = re.compile(r"world\s*of\s*warcraft|(^|[^a-z])wow([^a-z]|$)|warcraft|lich\s*king|blizzard", re.I)

# ------------------------------------------------------------------ no address yet: Coming soon
st, _, b = get("/wrath/survivors")
t = text(b)
check("page: 200 with the title", st == 200 and "Azeroth Survivors" in t)
check("page: the credit, word for word", "Azeroth Survivors by Aethus. Open rebuild by OutThisLife, MIT or Apache-2.0." in t)
check("page: the source link", 'href="https://github.com/OutThisLife/azeroth-survivors-re"' in b.decode())
check("page: the key art, sized, with alt text", re.search(r'<img class="sv-art" src="[^"]*leyline-mode-azeroth-survivors\.webp" '
                                                         r'alt="[^"]{20,}" width="483" height="298"', b.decode()) is not None)
check("page: no address -> Coming soon, no Play", "Coming soon" in t and 'href="/wrath/survivors/play"' not in b.decode())
check("page: no brand words a player reads", not BRAND.search(t), BRAND.search(t) and BRAND.search(t).group(0))
check("page: no client version numbers", not re.search(r"\b(1\.12|3\.3\.5|5875|12340)\b", t))
check("page: no money or tip links", not re.search(r"patreon|ko-fi|paypal|donat|\btip\b", b.decode(), re.I))
check("page: Aethus's site address is not printed", "aethus.pl" not in b.decode())
check("page: no permission line until the owner writes it", sv.PERMISSION == "" and "TODO" not in t)
st, h, _ = get("/wrath/survivors/play", cookie="wr_session=good")
check("play without an address: back to the page", st == 302 and h.get("location") == "/wrath/survivors")

for bad in ("http://survivors.leylinelauncher.com/", "https://survivors.leylinelauncher.com/x", "https://exa mple/", "https://survivors.leylinelauncher.com:8443/", "nope"):
    with open(URLF, "w") as f:
        f.write(bad + "\n")
    check(f"a bad address is not used ({bad})", sv.site_url() == "")

# ------------------------------------------------------------------ the address set: Play
with open(URLF, "w") as f:
    f.write("https://survivors.leylinelauncher.com/\n")
check("the address from the file", sv.site_url() == "https://survivors.leylinelauncher.com/" and sv.site_host() == "survivors.leylinelauncher.com")
st, _, b = get("/wrath/survivors")
check("page: with an address -> Play, no Coming soon", 'class="btn primary" href="/wrath/survivors/play">Play</a>' in b.decode()
      and "Coming soon" not in text(b))
check("page: the licences link on the game's address", 'href="https://survivors.leylinelauncher.com/licenses/"' in b.decode())
st, h, _ = get("/wrath/survivors/play")
check("play signed out: to the sign-in, and back to Play", st == 302 and "survivors" in urllib.parse.unquote(h.get("location", "")), h.get("location"))
st, h, _ = get("/wrath/survivors/play", cookie="wr_session=bad")
check("play with a stale sign-in: to the sign-in", st == 302 and "survivors" in urllib.parse.unquote(h.get("location", "")))
st, h, _ = get("/wrath/survivors/play", cookie="wr_session=good")
loc = h.get("location", "")
check("play signed in: a one-time code to the Survivors address's gate", st == 302 and loc.startswith("https://survivors.leylinelauncher.com/__gate?code="), loc)
gst, land, ck = gate(loc)
check("the gate: lands on / with the wu_play cookie (HttpOnly, Secure)", gst == 302 and land == "/" and ck.startswith("wu_play=")
      and "HttpOnly" in ck and "Secure" in ck)
check("the code works once", gate(loc)[1] != "/")
sv_ck = ck.split(";")[0]
for uri in ("/", "/wenilla_bg.wasm", "/data/__index", "/data/DBFilesClient%5CMap.dbc", "/boot-manifest.json"):
    check(f"auth with the session: {uri} -> 200", auth(uri, sv_ck)[0] == 200)
st, h, _ = auth("/")
check("auth without a session: the page -> 302 to Play", st == 302 and h.get("location") == "https://play.openvanilla.app/wrath/survivors/play")
check("auth without a session: /data -> 403", auth("/data/__index")[0] == 403)
check("auth with a junk cookie: /data -> 403", auth("/data/__index", "wu_play=nope")[0] == 403)
sess = wp.sessions.get(sv_ck.split("=", 1)[1])
check("the session: mode survivors, its own name, no password", sess and sess["mode"] == "survivors" and sess["user"] == "SV-WR000123"
      and sess["pass"] == "")

# ------------------------------------------------------------------ the two sites stay apart
st, _, _ = get("/__auth", cookie=sv_ck, headers={"X-Forwarded-Uri": "/data/__tier/index.json"})
check("a Survivors session does not open the Wrath stream's /data", st == 302)
st, _, b = get("/__env.js", cookie=sv_ck)
check("a Survivors session gets no Wrath login from /__env.js", b"WR000123" not in b)
st, h, _ = get("/wrath/play?mini=mini", cookie="wr_session=good")
_, _, wck = gate(h.get("location", ""))
wck = wck.split(";")[0]
check("a Wrath stream session does not open the Survivors address", auth("/data/__index", wck)[0] == 403 and auth("/", wck)[0] == 302)
st, h, _ = get("/wrath/play?mode=own", cookie="wr_session=good")   # own files ends the player's Wrath sessions
check("own-files play (ends the Wrath sessions) leaves the Survivors run alone", auth("/data/__index", sv_ck)[0] == 200)

# ------------------------------------------------------------------ the killswitch
with open(FLAG, "w") as f:
    f.write('{"gamedata_streaming": false, "since": "2026-11-01T18:00:00Z", "by": "owner"}\n')
for uri in ("/data/__index", "/data", "/data/DBFilesClient%5CMap.dbc", "/boot-manifest.json", "/world-manifest.json?v=1"):
    for who, c in (("session", sv_ck), ("no session", "")):
        st, h, b = auth(uri, c)
        check(f"off: {uri} ({who}) -> 410 JSON", st == 410 and json.loads(b)["error"] == "gamedata_off")
st, h, _ = auth("/", sv_ck)
check("off: the page sends even a session to the resting page", st == 302 and h.get("location") == "https://play.openvanilla.app/wrath/survivors")
st, _, b = get("/wrath/survivors")
check("off: the page says Resting, no Play", "Resting" in text(b) and 'href="/wrath/survivors/play"' not in b.decode()
      and "licenses/" not in b.decode())
st, h, _ = get("/wrath/survivors/play", cookie="wr_session=good")
check("off: Play goes back to the page", st == 302 and h.get("location") == "/wrath/survivors")
os.remove(FLAG)
check("on again: the session reads /data", auth("/data/__index", sv_ck)[0] == 200)

# ------------------------------------------------------------------ the catalog
pm = wp.playmodes
e = pm.by_id("azeroth-survivors") if pm is not None else None
check("catalog: still coming, start /wrath/survivors", e and e["status"] == "coming" and e["start"] == {"path": "/wrath/survivors"})
with open(os.path.join(STREAM, "survivors", "upstream.lock.json"), encoding="utf-8") as f:
    lock = json.load(f)
check("lock: the upstream repo and a full sha", lock["repo"] == sv.SOURCE and re.fullmatch(r"[0-9a-f]{40}", lock["sha"]))

print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
