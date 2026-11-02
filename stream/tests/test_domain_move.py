"""The move to leylinelauncher.com in the portal (docs/DOMAIN-MOVE.md): before it (LEYLINE_DOMAIN_MOVE unset) every
page says the old names; with it on, the new ones, both names are accepted (origins, the game's frame), launcher
documents on the old site name keep their links there, and the sign-in flow from an old link through Caddy's 301
(tools/domain/domain_caddy.py's rules) signs in once on the new names with no loop.

    python3 -I stream/tests/test_domain_move.py stream/wrath_portal.py
"""
import http.client, importlib.util, json, os, re, subprocess, sys, tempfile, threading, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from xml.sax.saxutils import escape

PORTAL = os.path.abspath(sys.argv[1])
MODE = sys.argv[2] if len(sys.argv) > 2 else ""
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
NEW = {"site": "leylinelauncher.com", "game": "play.leylinelauncher.com", "relay": "relay.leylinelauncher.com"}
OLD = {"site": "play.openvanilla.app", "game": "wrath.openvanilla.app", "relay": "relay.openvanilla.app"}

if not MODE:   # each mode in its own process: the portal reads its settings once
    bad = 0
    for m in ("old", "new"):
        r = subprocess.run([sys.executable, "-I", os.path.abspath(__file__), PORTAL, m], capture_output=True, text=True, timeout=300)
        sys.stdout.write(r.stdout + r.stderr)
        bad += r.returncode != 0
    sys.exit(1 if bad else 0)

SOAP_PORT, REALM_PORT, PAGE_PORT = (17981, 18191, 18192) if MODE == "old" else (17982, 18193, 18194)
slices = tempfile.mkdtemp()
with open(os.path.join(slices, "mini.json"), "w") as f:
    json.dump({"slice": "mini", "maxTier": 4, "title": "Mini"}, f)
for k in ("PUBLIC_HOST", "STREAM_HOST", "WU_JOIN_BASE", "WU_RELAY_WS", "WU_RELAY_LISTING"):
    os.environ.pop(k, None)
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL=f"http://127.0.0.1:{SOAP_PORT}/",
                  PORTAL_URL=f"http://127.0.0.1:{REALM_PORT}", WU_SLICE_DIR=slices, WU_STREAM="1", WU_OWNFILES="1",
                  WU_LEYLINE_DIR=tempfile.mkdtemp(), WU_RELAY_LISTING_FILE="/nonexistent",
                  LEYLINE_DOMAIN_MOVE="1" if MODE == "new" else "0")
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
sys.path.insert(0, os.path.join(ROOT, "tools", "domain"))
import domain_caddy as dc  # noqa: E402
A = dc.load()


class Soap(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        body = ('<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" '
                f'xmlns:ns1="urn:AC"><SOAP-ENV:Body><ns1:executeCommandResponse><result>{escape("ovstream ok")}</result>'
                '</ns1:executeCommandResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>').encode()
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)


class Realm(BaseHTTPRequestHandler):   # the website (Rust): signed in with wr_session=good
    def log_message(self, *a): pass

    def do_GET(self):
        if "wr_session=good" not in self.headers.get("Cookie", ""):
            self.send_response(401); self.send_header("Content-Length", "0"); self.end_headers(); return
        doc = {"/api/me": {"display_name": "Tester"}, "/api/play": {"user": "WR000123", "pass": "Pa55word1234abcd"}}.get(self.path.split("?")[0], {})
        body = json.dumps(doc).encode()
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)


for port, h in ((SOAP_PORT, Soap), (REALM_PORT, Realm), (PAGE_PORT, wp.PageHandler)):
    srv = ThreadingHTTPServer(("127.0.0.1", port), h)
    threading.Thread(target=srv.serve_forever, daemon=True).start()


def req(method, path, host, cookie="", headers=None, body=None):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=20)
    hd = {"Host": host, "X-Forwarded-For": "10.0.0.9", **(headers or {})}
    if cookie:
        hd["Cookie"] = cookie
    c.request(method, path, body=body, headers=hd)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(f"PASS [{MODE}] {name}" if cond else f"FAIL [{MODE}] {name}  {extra}")
    fails += 0 if cond else 1


now = NEW if MODE == "new" else OLD
other = OLD if MODE == "new" else NEW

# --- the names
check("ADDRESSES matches tools/domain/addresses.json",
      wp.ADDRESSES == {p: (A[p]["new"], A[p]["old"]) for p in ("site", "game", "relay")})
check("the site, the game and the relay say the names of now",
      (wp.PUBLIC_HOST, wp.STREAM_HOST, wp.JOIN_BASE, wp.RELAY_WS) == (now["site"], now["game"], "https://" + now["relay"], "wss://" + now["relay"]),
      (wp.PUBLIC_HOST, wp.STREAM_HOST, wp.JOIN_BASE, wp.RELAY_WS))
check("both names accepted", wp.SITE_ORIGINS >= {"https://" + NEW["site"], "https://" + OLD["site"]}
      and wp.GAME_ORIGINS >= {"https://" + NEW["game"], "https://" + OLD["game"]})

# --- redirects the portal writes
st, h, _ = req("GET", "/__gate?code=nope", now["game"])
check("a bad gate code goes to the site's sign-in on the name of now", h.get("location") == f"https://{now['site']}/wrath", h.get("location"))
st, h, _ = req("GET", "/__auth", now["game"], headers={"X-Forwarded-Uri": "/"})
check("/__auth without a session: the site of now", h.get("location") == f"https://{now['site']}/wrath", h.get("location"))
st, h, b = req("GET", "/__env.js", now["game"])
check("/__env.js signed out: back to the site of now", st == 401 and f"https://{now['site']}/wrath".encode() in b and other["site"].encode() not in b, b[:200])
st, h, _ = req("GET", "/wrath/play?mini=mini", now["site"], cookie="wr_session=good")
check("Play: to the game's gate on the name of now", st == 302 and h.get("location", "").startswith(f"https://{now['game']}/__gate?code="), h.get("location"))

# --- the page's frame and the game's POSTs accept both names
st, h, _ = req("GET", "/wrath/play?mode=own", now["site"])
csp = h.get("content-security-policy", "")
check("the game may be framed on both names", f"https://{NEW['game']}" in csp and f"https://{OLD['game']}" in csp, csp)
for origin, ok in ((f"https://{NEW['game']}", True), (f"https://{OLD['game']}", True), ("https://evil.example", False)):
    st, h, b = req("POST", "/__gate?hop=wrath", now["game"], headers={"Origin": origin, "Content-Type": "application/json"}, body=b"{}")
    refused = st == 403 and b"same-origin only" in b
    check(f"the game's hop call from {origin}: {'taken' if ok else 'refused'}", refused != ok, (st, b[:80]))

# --- what pages and documents say
if MODE == "old":
    html = b'<a href="https://leylinelauncher.com/wrath/docs">x</a> play.leylinelauncher.com relay.leylinelauncher.com'
    out = wp.legacy_body(html, "text/html; charset=utf-8", OLD["site"])
    check("before the move: new names in a page become the old ones",
          out == b'<a href="https://play.openvanilla.app/wrath/docs">x</a> wrath.openvanilla.app relay.openvanilla.app', out)
    check("before the move: pictures are left alone", wp.legacy_body(b"\x89PNG leylinelauncher.com", "image/png", "x") == b"\x89PNG leylinelauncher.com")
else:
    doc = b'{"download":"https://leylinelauncher.com/wrath/mods/x/download","page":"https://leylinelauncher.com/wrath/mods/x"}'
    old = wp.legacy_body(doc, "application/json; charset=utf-8", OLD["site"])
    check("a launcher document on the old name keeps its links there (installed launchers download from that name only)",
          old == doc.replace(b"https://leylinelauncher.com/", b"https://play.openvanilla.app/"), old)
    check("the same document on the new name: unchanged", wp.legacy_body(doc, "application/json", NEW["site"]) == doc)
    check("a page is never rewritten after the move", wp.legacy_body(b"https://leylinelauncher.com/", "text/html", OLD["site"]) == b"https://leylinelauncher.com/")
    st, h, b = req("GET", "/wrath/status.json", OLD["site"])
    check("an API on the old name answers there (200), as before", st == 200, st)

# --- the whole sign-in flow through Caddy's rules, from a link on the old name (a browser: page loads)
if MODE == "new":
    PAGE = {"Sec-Fetch-Dest": "document", "Sec-Fetch-Mode": "navigate", "Accept": "text/html"}
    jar = {}   # host -> cookie header (cookies are per host)
    seen = []
    url = f"https://{OLD['site']}/wrath/play?mini=mini"
    signed_in_once = False
    for _ in range(20):
        u = urllib.parse.urlsplit(url)
        host, path = u.hostname, (u.path or "/") + ("?" + u.query if u.query else "")
        seen.append(url)
        if seen.count(url) > 1:
            break
        loc = dc.would_redirect(A, "redirect", host, "GET", path, PAGE)
        if loc:
            url = loc
            continue
        if host == NEW["game"] and not path.startswith("/__"):
            # Caddy asks /__auth first (forward_auth), then wu-play serves the game page
            st, h, _ = req("GET", "/__auth", host, cookie=jar.get(host, ""), headers={"X-Forwarded-Uri": path})
            if st == 200:
                break
            url = h["location"]
            continue
        st, h, _ = req("GET", path, host, cookie=jar.get(host, ""), headers=PAGE)
        if "set-cookie" in h:
            jar[host] = h["set-cookie"].split(";")[0]
        if st in (301, 302, 303):
            loc = h["location"]
            url = loc if loc.startswith("https://") else f"https://{host}{loc}"
            continue
        if st == 200 and path.startswith("/wrath") and not signed_in_once:
            jar[host] = "wr_session=good"   # the player signs in here (the website sets its cookie on this name)
            signed_in_once = True
            url = f"https://{NEW['site']}/wrath/play?mini=mini&signedin=1"   # and presses Play again
            continue
        break
    check("an old link ends in the game on the new name", seen[-1].startswith(f"https://{NEW['game']}/") and "/__gate" not in seen[-1], seen)
    check("no loop: no address twice", len(seen) == len(set(seen)), seen)
    check("the old names only at the start", not any(OLD["site"] in s or OLD["game"] in s for s in seen[1:]), seen)
    check("signed in once, on the new name", signed_in_once and set(jar) <= {NEW["site"], NEW["game"]}, jar)

print(f"[{MODE}] {count - fails}/{count} passed" if not fails else f"[{MODE}] {fails} failed")
sys.exit(1 if fails else 0)
