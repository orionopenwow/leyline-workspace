"""The kill switch's website half (docs/KILL-SWITCH.md): $WU_PORTAL_DATA/leyline/shutdown.json.

  * no file, or a bad one: every page as before;
  * "banner": every HTML page gets the message on top, once, escaped; JSON untouched;
  * "closed": every page answers the farewell page (503), GET and POST, while the launcher's feeds
    (/wrath/leyline/...), the status document, the art and HQ still answer;
  * the file read again when it changes; the message never carries brand words from us.

    python3 -I stream/tests/test_shutdown.py stream/wrath_portal.py
"""
import http.client, importlib.util, json, os, sys, tempfile, threading, time
from http.server import ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
PAGE_PORT = 18291
T = tempfile.mkdtemp(prefix="wu-shutdown-")
DATA = os.path.join(T, "data")
os.makedirs(os.path.join(DATA, "leyline"))
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_PORTAL_DATA=DATA, WU_ART_DIR=os.path.join(STREAM, "www-files", "art"))

fails = 0
count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra else ""))
    fails += 0 if cond else 1


pspec = importlib.util.spec_from_file_location("wp", sys.argv[1] if len(sys.argv) > 1 else os.path.join(STREAM, "wrath_portal.py"))
wp = importlib.util.module_from_spec(pspec)
pspec.loader.exec_module(wp)
srv = ThreadingHTTPServer(("127.0.0.1", PAGE_PORT), wp.PageHandler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()
FILE = os.path.join(DATA, "leyline", "shutdown.json")
ip_n = [0]


def req(method, path, body=None):
    ip_n[0] += 1
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=20)
    hd = {"X-Forwarded-For": f"10.8.{ip_n[0] // 200}.{ip_n[0] % 200 + 1}"}
    if body is not None:
        hd["Content-Type"] = "application/x-www-form-urlencoded"
    c.request(method, path, body=body, headers=hd)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


def put(doc):
    if doc is None:
        try:
            os.unlink(FILE)
        except FileNotFoundError:
            pass
        return
    with open(FILE, "w", encoding="utf-8") as f:
        f.write(doc if isinstance(doc, str) else json.dumps(doc))
    t = time.time() + ip_n[0] * 0.001   # a new mtime each time, so the cache sees the change
    os.utime(FILE, (t, t))


PAGES = ["/wrath", "/wrath/leyline", "/wrath/realms", "/wrath/key"]
# Pages that set a sign-in cookie answer through their module's own sender, not PageHandler.send: no banner there
# (the closed farewell page still covers them, before any module runs).
BANNER_PAGES = ["/wrath", "/wrath/leyline", "/wrath/realms"]
MSG = "Thank you all.\nThe servers close tonight.\n<script>alert(1)</script>"

# ---- nothing on
put(None)
before = {p: req("GET", p) for p in PAGES}
check("no file: no farewell anywhere", all(b"ly-farewell" not in v[2] for v in before.values()))
check("no file: shutdown_doc is None", wp.shutdown_doc() is None)
for bad in ("{", json.dumps({"mode": "nuke", "message": "x"}), json.dumps({"mode": "closed", "message": "  "}), "[]"):
    put(bad)
    check(f"a bad file is ignored: {bad[:30]}", wp.shutdown_doc() is None and req("GET", "/wrath")[0] == before["/wrath"][0])

# ---- banner
put({"mode": "banner", "title": "Leyline is closing", "message": MSG})
for p in BANNER_PAGES:
    st, h, b = req("GET", p)
    if not h.get("content-type", "").startswith("text/html"):
        continue
    check(f"banner: {p} answers as before ({st})", st == before[p][0])
    check(f"banner: {p} has the banner once", b.count(b'id="ly-farewell-bar"') == 1, b[:200])
    check(f"banner: {p} message escaped", b"<script>alert(1)</script>" not in b and b"&lt;script&gt;" in b)
st, h, b = req("GET", "/wrath/status.json")
check("banner: JSON untouched", st == 200 and b"ly-farewell" not in b and h["content-type"].startswith("application/json"))
check("banner: the bar goes right after <body>", b"<body" in req("GET", "/wrath")[2] and
      req("GET", "/wrath")[2].split(b'id="ly-farewell-bar"')[0].rstrip().endswith(b"<aside"))
check("banner: shutdown_body twice = once",
      wp.shutdown_body(wp.shutdown_body(b"<html><body>x</body></html>", "text/html"), "text/html").count(b"ly-farewell-bar") == 1)

# ---- closed
put({"mode": "closed", "title": "Leyline has closed", "message": MSG})
for p in PAGES + ["/", "/wrath/play", "/wrath/forum", "/__auth"]:
    st, h, b = req("GET", p)
    check(f"closed: {p} is the farewell page (503)", st == 503 and b'id="ly-farewell"' in b and b"Leyline has closed" in b, st)
st, h, b = req("POST", "/wrath/submit", "x=1")
check("closed: a POST gets the farewell page too", st == 503 and b"ly-farewell" in b)
st, h, b = req("GET", "/wrath")
check("closed: the page is escaped", b"<script>alert(1)</script>" not in b and b"<p>Thank you all.</p>" in b)
check("closed: no-store and the site's CSP", h.get("cache-control") == "no-store" and "default-src 'none'" in h.get("content-security-policy", ""))
for p in ("/wrath/status.json", "/wrath/healthz", "/wrath/leyline/news.json", "/wrath/leyline/version.json",
          "/wrath/art/wu-theme.css", "/wrath/admin"):
    st, h, b = req("GET", p)
    check(f"closed: {p} still answers itself", st != 503 or b"ly-farewell" not in b, st)
check("closed: shutdown_open", wp.shutdown_open("/wrath/leyline/news.json") and wp.shutdown_open("/wrath/admin/x")
      and not wp.shutdown_open("/wrath/leyline-preview") and not wp.shutdown_open("/wrath"))

# ---- back
put(None)
st, h, b = req("GET", "/wrath")
check("file gone: back as before", st == before["/wrath"][0] and b"ly-farewell" not in b)

src = open(sys.argv[1] if len(sys.argv) > 1 else os.path.join(STREAM, "wrath_portal.py"), encoding="utf-8").read()
seg = src[src.index("# The kill switch (docs/KILL-SWITCH.md"):src.index("def brand_body")]
for bad in ("World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard"):
    check(f"no brand word in the farewell code: {bad}", bad not in seg)

print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
