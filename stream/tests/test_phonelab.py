"""stream/phonelab.py on the portal: Phone Lab (docs/MOBILE-WEB-OVERHAUL.md). Starting a session is for staff (admin or
above); a join link puts another phone in it; the cookie is on the site's parent domain (the game page on play.<site>
sees it), 30 minutes, readable by the page; the recorder is put into this browser's site pages only while its session is
live (with the page's CSP nonce and connect-src 'self'); the phones' batches: only into a live session, size-capped,
rate-limited, secrets dropped and tokens masked, pictures only small JPEGs; the readings (JSON) are staff only; a day
later the files go. The recorder's own behaviour in a phone: tests/phonelab.browser.test.cjs.

    python3 -I stream/tests/test_phonelab.py stream/wrath_portal.py
"""
import http.client, importlib.util, json, os, re, sys, tempfile, threading, time
from http.server import ThreadingHTTPServer

PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
TMP = tempfile.mkdtemp(prefix="wu-phonelab-")
for d in ("mods/files", "armory/c", "data"):
    os.makedirs(os.path.join(TMP, d))
os.environ.update(WU_DL_PROBE="0", SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="play.leylinelauncher.com", PUBLIC_HOST="leylinelauncher.com", WU_MODS_DIR=os.path.join(TMP, "mods"),
                  WU_ARMORY_DIR=os.path.join(TMP, "armory"), WU_PORTAL_DATA=os.path.join(TMP, "data"),
                  WU_RELAY_LISTING="http://127.0.0.1:9/realms.json", WU_PHONELAB_DIR=os.path.join(TMP, "data", "phonelab"))
for k in ("WU_HUB_FRONT", "WU_BRAND_FILE"):
    os.environ.pop(k, None)
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
pl = wp.phonelab

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, str(extra)[:600])

check("phonelab loaded and bound", pl is not None and pl.P("admin") is wp.admin and pl.P("PUBLIC_HOST") == "leylinelauncher.com")

# staff as the portal sees them: the test says who is signed in (X-Test-Role), the real refusal otherwise
def fake_who(h, path, api=False):
    role = h.headers.get("X-Test-Role", "")
    if not role:
        if api:
            h.send(401, b'{"ok":false,"error":"Log in first."}\n', "application/json; charset=utf-8")
        else:
            h.redirect("/wrath/signin?next=" + path)
        return None
    return {"account": "acct-" + role, "name": role, "role": role, "session": "sess-" + role, "game": "", "via": "api" if api else "web"}
wp.admin.who = fake_who

srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()
def req(method, path, body=None, headers=None, ip="203.0.113.7"):
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=20)
    hd = {"X-Forwarded-For": ip, **(headers or {})}
    c.request(method, path, body=body, headers=hd); r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()

# ---- pure parts -------------------------------------------------------------------------------------------------------
check("the cookie's domain: the site and its game page share it; nothing on an address",
      pl.cookie_domain("leylinelauncher.com") == "leylinelauncher.com" and pl.cookie_domain("play.leylinelauncher.com") == "leylinelauncher.com"
      and pl.cookie_domain("play.openvanilla.app") == "openvanilla.app" and pl.cookie_domain("127.0.0.1") == "" and pl.cookie_domain("localhost") == "")
check("routes: the page, its parts and the script only", pl.match("/wrath/phonelab") and pl.match("/wrath/phonelab/rec") and pl.match("/wrath/phonelab.js")
      and not pl.match("/wrath/phonelabs") and not pl.match("/wrath/phone"))
b = pl.clean_batch({"code": "ABCDEF", "seq": 3, "page": "/wrath/leyline", "recs": [
    {"kind": "tap", "on": "button#go", "text": "Play", "value": "typed words", "password": "hunter2", "token": "x",
     "note": "key " + "Ab1" * 12, "seq": [["touchstart", 0, 0], ["click", 90, 0]], "nested": {"Cookie": "wr_session=1", "ok": 1}}],
    "shots": [{"i": 0, "data": "data:image/jpeg;base64,/9j/AAAA"}, {"i": 1, "data": "data:image/png;base64,iVBOR"},
              {"i": 2, "data": "data:image/jpeg;base64," + "A" * 70000}, {"i": 3, "data": "data:image/jpeg;base64,<script>"}]})
r = b["recs"][0] if b and b["recs"] else {}
check("a batch as kept: secret-named fields dropped (value, password, token, cookie), token-like strings masked",
      r.get("on") == "button#go" and r.get("text") == "Play" and "value" not in r and "password" not in r and "token" not in r
      and "Cookie" not in r.get("nested", {}) and r["nested"].get("ok") == 1 and "•••" in r.get("note", "") and "Ab1Ab1Ab1" not in json.dumps(b), r)
check("pictures: small JPEG data URLs only (no PNG, nothing too big, nothing that isn't base64)",
      [s["i"] for s in b["shots"]] == [0], [s["i"] for s in b["shots"]])
check("not a batch: no code, a wrong code, not a dict", pl.clean_batch({"recs": []}) is None and pl.clean_batch({"code": "abc"}) is None
      and pl.clean_batch([1]) is None and pl.clean_batch({"code": "ABCDE0"}) is None)
check("records capped per batch", len(pl.clean_batch({"code": "ABCDEF", "recs": [{"kind": "x"}] * 500})["recs"]) == pl.MAX_RECS)

# ---- the page and starting a session -----------------------------------------------------------------------------------
st, hd, _ = req("GET", "/wrath/phonelab")
check("signed out: Phone Lab sends you to log in", st in (302, 303) and "/wrath/signin" in hd.get("location", ""), (st, hd.get("location")))
st, _, body = req("GET", "/wrath/phonelab", headers={"X-Test-Role": "mod"})
check("a mod: staff only for the owner and admins (403)", st == 403 and b"owner and admins" in body, st)
st, hd, body = req("GET", "/wrath/phonelab", headers={"X-Test-Role": "admin"})
page = body.decode()
token = (re.search(r'name="csrf" value="([^"]+)"', page) or [None, ""])[1]
check("an admin: the page with Start, what it records and what it never does", st == 200 and "Start Phone Lab" in page and token
      and "anything you type" in page and "Kept for 24 hours" in page and hd.get("content-security-policy", "").find("script-src 'none'") >= 0, st)
st, hd, _ = req("POST", "/wrath/phonelab/start", body="csrf=wrong", headers={"X-Test-Role": "admin", "Content-Type": "application/x-www-form-urlencoded"})
check("Start with a stale form: refused", st == 403, st)
st, hd, _ = req("POST", "/wrath/phonelab/start", body="csrf=" + token, headers={"X-Test-Role": "admin", "Content-Type": "application/x-www-form-urlencoded"})
ck = hd.get("set-cookie", "")
m = re.match(r"wu_phonelab=([A-HJ-NP-Z2-9]{6})\.(\d+); Max-Age=(\d+); Path=/", ck)
check("Start: a session and its cookie (30 minutes, the whole site, readable by the game page, not HttpOnly)",
      st in (302, 303) and m and 1700 <= int(m.group(3)) <= 1800 and "HttpOnly" not in ck and "SameSite=Lax" in ck, (st, ck))
CODE, COOKIE = (m.group(1), f"wu_phonelab={m.group(1)}.{m.group(2)}") if m else ("", "")
class H:   # a request as phonelab sees one
    def __init__(self, host, proto=""):
        self.headers = {"Host": host, "X-Forwarded-Proto": proto}
check("on the live site the cookie goes to the parent domain, Secure",
      "Domain=leylinelauncher.com" in pl.set_cookie(H("leylinelauncher.com", "https"), {"code": "ABCDEF", "ends": time.time() + 60})
      and "; Secure" in pl.set_cookie(H("leylinelauncher.com", "https"), {"code": "ABCDEF", "ends": time.time() + 60}))
st, _, body = req("GET", "/wrath/phonelab", headers={"Cookie": COOKIE})
check("the phone with the cookie sees its code, the time left and Stop (no staff login needed to stop)", st == 200 and CODE.encode() in body and b">Stop<" in body, st)

# ---- the recorder in the site's pages ----------------------------------------------------------------------------------
st, hd, body = req("GET", "/wrath/leyline", headers={"Cookie": COOKIE})
csp = hd.get("content-security-policy", "")
n = re.search(r'id="wu-phonelab-js" src="/wrath/phonelab.js" nonce="([^"]+)"', body.decode())
check("a site page while the session is live: the recorder in it, allowed by the page's CSP (its nonce, connect-src 'self')",
      st == 200 and n and f"'nonce-{n.group(1)}'" in csp and "connect-src 'self'" in csp, (st, csp))
st, hd, body = req("GET", "/wrath/leyline")
check("without the cookie: no recorder, no connect-src added", b"wu-phonelab-js" not in body and "connect-src" not in hd.get("content-security-policy", ""))
st, hd, body = req("GET", "/wrath/leyline", headers={"Cookie": "wu_phonelab=ABCDEF.9999999999"})
check("a made-up code: no recorder", b"wu-phonelab-js" not in body)
st, hd, js = req("GET", "/wrath/phonelab.js")
check("the script: JavaScript, for the game page too (CORS, cross-origin resource policy), never cached stale",
      st == 200 and hd.get("content-type", "").startswith("text/javascript") and hd.get("access-control-allow-origin") == "*"
      and hd.get("cross-origin-resource-policy") == "cross-origin" and hd.get("cache-control") == "no-cache" and b"__phonelab" in js)
check("the recorder never listens to typing", not re.search(rb"'(keydown|keyup|keypress|input|change|beforeinput)'", js)
      and b".value" not in js)
early = open(os.path.join(os.path.dirname(PORTAL), "www-files", "wrath-early.js"), encoding="utf-8").read()
check("the game page loads it only with a live cookie, from the site (wrath-early.js)",
      "// >>> phonelab" in early and "wu_phonelab=([A-HJ-NP-Z2-9]{6})" in early and "'/wrath/phonelab.js'" in early)

# ---- the phones' batches -----------------------------------------------------------------------------------------------
def rec(doc, ip="198.51.100.1", origin=""):
    raw = doc if isinstance(doc, bytes) else json.dumps(doc).encode()
    return req("POST", "/wrath/phonelab/rec", body=raw, headers={"Content-Type": "text/plain", **({"Origin": origin} if origin else {})}, ip=ip)
st, hd, _ = rec({"code": CODE, "seq": 1, "page": "/wrath/leyline", "recs": [{"kind": "tap", "on": "a#x", "seq": [["touchstart", 0, 0]]}]},
                origin="https://play.leylinelauncher.com")
check("a batch into the live session: kept (204), the game page may read the answer", st == 204 and hd.get("access-control-allow-origin") == "https://play.leylinelauncher.com", (st, hd))
st, hd, _ = rec({"code": CODE, "seq": 2, "recs": []}, origin="https://evil.example")
check("another site's page: no CORS answer for it", "access-control-allow-origin" not in hd)
st, _, _ = rec({"code": "ZZZZZZ", "recs": []})
check("no such session: 410", st == 410, st)
st, _, _ = rec(b"{not json")
check("not JSON: 400", st == 400, st)
st, _, _ = rec(b"x" * (pl.MAX_BODY + 10))
check("too big: 413", st == 413, st)
codes = [rec({"code": CODE, "recs": []}, ip="198.51.100.77")[0] for _ in range(pl.IP_PER_MIN + 3)]
check("one address sending too fast: 429", codes[-1] == 429 and codes.count(204) <= pl.SESSION_PER_MIN, codes[-5:])
pl._sess_hits.clear(); pl._ip_hits.clear()
st, hd, body = req("GET", f"/wrath/phonelab/{CODE}.json")
check("the readings, signed out: 401", st == 401, st)
st, hd, body = req("GET", f"/wrath/phonelab/{CODE}.json", headers={"X-Test-Role": "mod"})
check("the readings, a mod: 403", st == 403, st)
st, hd, body = req("GET", f"/wrath/phonelab/{CODE}.json", headers={"X-Test-Role": "owner"})
doc = json.loads(body) if st == 200 else {}
check("the readings, the owner: the session and its batches", st == 200 and doc["session"]["code"] == CODE and doc["session"]["live"]
      and doc["batches"][0]["recs"][0]["on"] == "a#x", (st, body[:200]))
st, _, body = req("GET", "/wrath/phonelab/latest.json", headers={"X-Test-Role": "admin"})
check("latest.json: the newest session", st == 200 and json.loads(body)["session"]["code"] == CODE)
st, _, body = req("GET", "/wrath/phonelab/sessions.json", headers={"X-Test-Role": "admin"})
check("sessions.json: the list", st == 200 and [s["code"] for s in json.loads(body)["sessions"]] == [CODE])

# caps: a full session refuses more; pictures stop at the cap
m0 = pl.meta(CODE)
m0["bytes"] = pl.MAX_SESSION_BYTES
pl._write_meta(m0)
st, _, _ = rec({"code": CODE, "recs": [{"kind": "x"}]})
check("a full session: 507", st == 507, st)
m0["bytes"] = 0; m0["shots"] = pl.MAX_SHOTS
pl._write_meta(m0)
st, _, _ = rec({"code": CODE, "recs": [{"kind": "x"}], "shots": [{"i": 9, "data": "data:image/jpeg;base64,/9j/AAAA"}]})
check("past the picture cap: the batch is kept, its picture is not", st == 204 and pl.read(CODE)["batches"][-1]["shots"] == [], st)

# Stop: from the batch (the banner's Stop) or the page's form; then nothing more goes in
st, _, _ = rec({"code": CODE, "recs": [{"kind": "stop"}], "stop": True})
check("the banner's Stop: the session ends", st == 204 and pl.meta(CODE)["stopped"])
st, _, _ = rec({"code": CODE, "recs": [{"kind": "x"}]})
check("after Stop: refused (410)", st == 410, st)
s2 = pl.start(by="t")
st, hd, _ = req("POST", "/wrath/phonelab/stop", body="", headers={"Cookie": f"wu_phonelab={s2['code']}.{s2['ends']}"})
check("the page's Stop: the session ends and the cookie goes", st in (302, 303) and pl.meta(s2["code"])["stopped"] and "Max-Age=0" in hd.get("set-cookie", ""))
s3 = pl.start(by="t")
st, hd, _ = req("GET", f"/wrath/phonelab/join/{s3['code']}")
check("a join link: this phone records into the live session", st in (302, 303) and f"wu_phonelab={s3['code']}." in hd.get("set-cookie", ""))
pl.stop(s3["code"])
st, _, _ = req("GET", f"/wrath/phonelab/join/{s3['code']}")
check("a join link for an ended session: 410", st == 410, st)
s4 = pl.start(by="t")
m4 = pl.meta(s4["code"]); m4["ends"] = int(time.time()) - 1; pl._write_meta(m4)
check("30 minutes on: the session is over by itself", not pl.live(pl.meta(s4["code"])) and rec({"code": s4["code"], "recs": []})[0] == 410)
old = time.time() - pl.KEEP_S - 60
for f in (pl._meta_path(s4["code"]), ):
    os.utime(f, (old, old))
check("a day later the files go", pl.prune() >= 1 and pl.meta(s4["code"]) is None)

here = os.path.dirname(PORTAL)
fl = open(os.path.join(here, "fastloop.sh"), encoding="utf-8").read()
bat = open(os.path.join(here, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
check("deployed with the signin step: packed, backed up, put back on a failed check, installed, and self-checked",
      'if exist phonelab.py set "LINKS=%LINKS% phonelab.py"' in bat and " phonelab.py profiles.py" in bat
      and fl.count("site_pages.py mobile_page.py phonelab.py;") == 3 and "/wrath/phonelab.js" in fl)
print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
