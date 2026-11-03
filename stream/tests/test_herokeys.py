"""Your Hero Key (stream/herokeys.py) through the real portal (stream/wrath_portal.py) with the real social.py (tokens,
names), profiles.py (the name rules) and hub.py (the look), against a fake website sign-in service that keeps keys the
way the real one does (only a SHA-256 of the secret; a new key revokes the old one; shown once).

Covers: the page signed out (Get your Hero Key, Sign in with it; the Leyline look, no old logo, crest or side art, no
brand words), a new account (a made-up readable name through the name rules, the key shown once: hidden, Show,
Download as a text file, Copy), saved (the service stops showing it), a new key (the old one stops working), the
launcher's calls (account/new, herokey/signin, password set and sign-in, herokey with its kept session, a /wrath/link
launcher borrowing the browser's, web_only without one), the name change (blocked names, the daily limit), the text
file, the limits, cross-site forms, sign out ending the launcher's own session, no key or password in the logs or the
store, and the deploy lists. What can go wrong (2 Nov): a pasted key with OVK1., spaces, line breaks and zero-width
characters; a stale or junk website cookie (the signed-out card and the cookie cleared, never a redirect loop); a
launcher's kept session running out (409 web_only, never a 401 that signs the launcher out); two new keys at once
(one made, both get it); accented names typed decomposed and with spaces; garbage bodies; the service down or
answering nonsense (503 with words); a bug of ours (503, not a 500); the release audit's test account closing itself
(and nobody else's, nor one with friends or messages).

    python3 -I stream/tests/test_herokeys.py stream/wrath_portal.py
"""
import hashlib, html, http.client, importlib.util, io, json, logging, os, re, secrets, sys, tempfile, threading, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "stream", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
TMP = tempfile.mkdtemp(prefix="wu-herokeys-")
fails = count = 0
LOGS = io.StringIO()
_lh = logging.StreamHandler(LOGS); _lh.setLevel(logging.DEBUG)
logging.getLogger().addHandler(_lh); logging.getLogger().setLevel(logging.DEBUG)


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra!r}"[:600] if extra and not cond else ""))
    fails += 0 if cond else 1


# ---------------------------------------------------------------- the fake sign-in service
ABC = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789"
R = {"users": {}, "sessions": {}, "keys": {}, "play": {}, "mints": {}, "seen": [], "guest_off": False}
LOCK = threading.Lock()


def r_mint(uid):
    kid = "".join(secrets.choice(ABC) for _ in range(10))
    sec = secrets.token_urlsafe(32)
    for k in R["keys"].values():
        if k["uid"] == uid:
            k["revoked"] = True
    R["keys"][kid] = {"uid": uid, "hash": hashlib.sha256(f"{kid}\n{sec}".encode()).hexdigest(), "revoked": False, "saved": False}
    return f"ovk1.{kid}.{sec}"


def r_user(guest=True):
    uid = len(R["users"]) + 1
    R["users"][uid] = {"username": f"guest-{uid:08d}", "display": f"Guest {uid:08d}", "acct": f"WR{uid:06d}"}
    return uid


def r_session(uid):
    t = secrets.token_urlsafe(24)
    R["sessions"][t] = {"uid": uid, "csrf": secrets.token_hex(8)}
    return t


def r_verify(key):
    m = re.fullmatch(r"ovk1\.([A-Za-z0-9]{10})\.([A-Za-z0-9_-]{43})", key or "")
    if not m:
        return None
    k = R["keys"].get(m.group(1))
    if not k or k["revoked"] or k["hash"] != hashlib.sha256(f"{m.group(1)}\n{m.group(2)}".encode()).hexdigest():
        return None
    return k["uid"]


def active(uid):
    return next((kid for kid, k in R["keys"].items() if k["uid"] == uid and not k["revoked"]), None)


class Realm(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def cookies(self):
        out = {}
        for part in self.headers.get("Cookie", "").split(";"):
            k, _, v = part.strip().partition("=")
            out[k] = v
        return out

    def sess(self):
        return R["sessions"].get(self.cookies().get("wr_session", ""))

    def reply(self, st, body=b"", ctype="text/html", cookies=(), loc=None):
        body = body.encode() if isinstance(body, str) else body
        self.send_response(st)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        for c in cookies:
            self.send_header("Set-Cookie", c)
        if loc:
            self.send_header("Location", loc)
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        s = self.sess()
        R["seen"].append(("GET", self.path, self.headers.get("X-Forwarded-For", "")))
        if not s:
            return self.reply(303, loc="/login") if self.path.startswith("/account") else self.reply(401)
        u = R["users"][s["uid"]]
        if self.path == "/api/me":
            return self.reply(200, json.dumps({"username": u["username"], "display_name": u["display"]}), "application/json")
        if self.path == "/api/play":
            return self.reply(200, json.dumps({"user": u["acct"], "pass": "p" * 16}), "application/json")
        if self.path.startswith("/account/key"):
            key = R["play"].get(self.cookies().get("wr_herokey_play", ""))
            if key and r_verify(key) != s["uid"]:
                key = None
            set_c = []
            if not key and not active(s["uid"]):
                key = r_mint(s["uid"])
                set_c.append("wr_herokey=enc; Path=/account/key; HttpOnly; Secure; SameSite=Lax")
            kid = active(s["uid"])
            saved = R["keys"][kid]["saved"]
            page = ('<img class="logo" src="/brand/logo.webp"><img class="crest" src="/brand/crest.webp">'
                    + (f'<code id="keytext">{key}</code>' if key else "")
                    + ('<div class="ok">Your Hero Key is saved. Sign in with it on any device.</div>' if saved and not key else "")
                    + '<span>Key made <b>31 Oct 2026</b></span><span>Last used <b>31 Oct 2026, Firefox</b></span>'
                    + '<code id="herotext">ovh1.' + "A" * 43 + '</code>'
                    + f'<form><input type="hidden" name="_csrf" value="{s["csrf"]}"></form>')
            return self.reply(200, page, cookies=set_c)
        self.reply(404)

    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0") or 0)
        form = dict(urllib.parse.parse_qsl(self.rfile.read(n).decode()))
        R["seen"].append(("POST", self.path, self.headers.get("X-Forwarded-For", "")))
        if self.headers.get("Sec-Fetch-Site") not in ("same-origin", "none"):
            return self.reply(403, "cross-site")
        if self.path == "/auth/key":
            uid = r_verify(form.get("key", ""))
            if not uid:
                return self.reply(200, "<p class=err>That Hero Key didn't work.</p>")
            return self.reply(303, loc="/account/key?back=1", cookies=[f"wr_session={r_session(uid)}; Path=/; HttpOnly; Secure; SameSite=Lax"])
        if self.path == "/auth/guest":
            if R["guest_off"]:
                return self.reply(200, "Guest play is off on this realm.")
            uid = r_user()
            key = r_mint(uid)
            pc = secrets.token_hex(8); R["play"][pc] = key
            return self.reply(303, loc="/", cookies=[f"wr_session={r_session(uid)}; Domain=play.openvanilla.app; Path=/; HttpOnly; Secure; SameSite=Lax",
                                                     f"wr_guest=g{uid}; Path=/; Max-Age=34560000; HttpOnly; Secure; SameSite=Lax",
                                                     f"wr_herokey_play={pc}; Path=/; Max-Age=2592000; HttpOnly; Secure; SameSite=Lax"])
        s = self.sess()
        if not s:
            return self.reply(303, loc="/login")
        if self.path == "/logout":
            R["sessions"] = {k: v for k, v in R["sessions"].items() if v is not s}
            return self.reply(303, loc="/", cookies=["wr_session=; Path=/; Max-Age=0"])
        if form.get("_csrf") != s["csrf"]:
            return self.reply(403, "form token mismatch")
        if self.path == "/account/key/mint.json":
            R["mints"].setdefault(s["uid"], []).append(1)
            if len(R["mints"][s["uid"]]) > 10:
                return self.reply(429)
            key = r_mint(s["uid"])
            pc = secrets.token_hex(8); R["play"][pc] = key
            return self.reply(200, json.dumps({"key": key}), "application/json",
                              cookies=[f"wr_herokey_play={pc}; Path=/; Max-Age=2592000; HttpOnly; Secure; SameSite=Lax"])
        if self.path == "/account/key/saved":
            kid = active(s["uid"])
            if kid:
                R["keys"][kid]["saved"] = True
            return self.reply(200, json.dumps({"saved": True}), "application/json",
                              cookies=["wr_herokey_play=; Path=/; Max-Age=0"])
        self.reply(404)


def serve(handler):
    srv = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv.server_address[1]


SITE = serve(Realm)
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL=f"http://127.0.0.1:{SITE}",
                  WU_STREAM="0", STREAM_HOST="wrath.example", WU_PORTAL_DATA=TMP, WU_HOST_DIR=os.path.join(TMP, "hosted"),
                  WU_RELAY_LISTING="http://127.0.0.1:9/none", WU_ADMIN_STAFF="", WU_ADMIN_OWNERS="", WU_ADMIN_GM_LEVEL="0",
                  PUBLIC_HOST="play.example", WU_HUB_FRONT="0")
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
wp.ac_soap = lambda cmd: ""
HK = wp.herokeys
check("the portal loads herokeys.py", HK is not None)
S = wp.social.setup(start_thread=False)
wp.profiles.setup()
HS = HK.setup()
check("the store opens in WU_PORTAL_DATA (herokeys.sqlite3, 0600)", HS is not None
      and (os.stat(os.path.join(TMP, "herokeys.sqlite3")).st_mode & 0o777) == 0o600)
wp.browse_limit.hit = lambda *a, **k: True
PAGE = serve(wp.PageHandler)
BRAND = re.compile(r"(?i)world\s*of\s*warcraft|warcraft|lich\s*king|blizzard|\bwow\b|open\s*vanilla")
IP = ["10.0.0.7"]


def req(method, path, cookies="", form=None, js=None, origin="https://play.example", sfs="same-origin", headers=None):
    c = http.client.HTTPConnection("127.0.0.1", PAGE, timeout=30)
    h = {"X-Forwarded-For": IP[0]}
    if cookies:
        h["Cookie"] = cookies
    body = None
    if form is not None:
        body = urllib.parse.urlencode(form).encode(); h["Content-Type"] = "application/x-www-form-urlencoded"
    if js is not None:
        body = json.dumps(js).encode(); h["Content-Type"] = "application/json"
    if body is not None:
        if origin:
            h["Origin"] = origin
        if sfs:
            h["Sec-Fetch-Site"] = sfs
    h.update(headers or {})
    c.request(method, path, body=body, headers=h)
    r = c.getresponse()
    hd = {}
    for k, v in r.getheaders():
        hd.setdefault(k, []).append(v)
    raw = r.read().decode("utf-8", "replace")
    c.close()
    try:
        doc = json.loads(raw)
    except ValueError:
        doc = None
    return r.status, {k: v[0] for k, v in hd.items()}, raw, hd.get("Set-Cookie", []), doc


def jar_add(jar, set_cookies):
    d = dict(p.split("=", 1) for p in jar.split("; ") if "=" in p) if jar else {}
    for c in set_cookies:
        k, _, v = c.split(";")[0].partition("=")
        if not v or "Max-Age=0" in c:
            d.pop(k, None)
        else:
            d[k] = v
    return "; ".join(f"{k}={v}" for k, v in d.items())


def visible(body):
    body = re.sub(r"(?s)<script.*?</script>|<style.*?</style>", " ", body)
    return re.sub(r"<[^>]+>", " ", body)


# ---------------------------------------------------------------- signed out
st, hd, body, sc, _ = req("GET", "/wrath/key")
card = body[body.find('<div class="hk">'):body.find("<script nonce")]
check("page signed out: 200, one card: Log in (key file, paste, the key kept here), Google, then Create a guest account", st == 200
      and ">Log in</h1>" in body and "Drop your key file here" in body and 'id="hk-usesaved"' in body and "Create a guest account" in body
      and 'action="/wrath/key/start"' in body and 'action="/wrath/key/signin"' in body
      and body.index("/wrath/key/signin") < body.index("go/google") < body.index("/wrath/key/start"), body[:300])
check("page signed out: What is a Hero Key? (your login, save it, a fingerprint only, lost it, one account)",
      "What is a Hero Key?" in body and "only a fingerprint" in body and "A new key stops the old one" in body)
check("page signed out: no Discord sign-in, no link to the old sign-in pages", "Continue with Discord" not in body and 'href="/login' not in body
      and 'href="/key"' not in body)
check("page: the Leyline look (the hub's header and tokens), noindex", 'class="hb-top"' in body and "--lk-surface" in body
      and hd.get("X-Robots-Tag", "").startswith("noindex"))
check("page: no old logo, crest, side art or screenshot prompt in the card", not re.search(r"(?i)logo\.webp|crest|screenshot|<img", card), card[:400])
check("page: plain words, no game, company or old site names", not BRAND.search(visible(body)), BRAND.findall(visible(body)))
check("page: one account for Wrath Unleashed and every realm that accepts Hero Key", "every realm that accepts Hero Key" in body)
check("page: a form cookie for the signed-out forms (Path=/wrath/key, HttpOnly, Secure, Strict)", any(
    c.startswith("ly_hk=") and "Path=/wrath/key" in c and "HttpOnly" in c and "SameSite=Strict" in c for c in sc), sc)
anon = jar_add("", sc)
t_start = re.search(r'action="/wrath/key/start"[^>]*><input type="hidden" name="t" value="([0-9a-f]{32})"', body).group(1)
t_sign = re.search(r'action="/wrath/key/signin" id="hk-login"><input type="hidden" name="t" value="([0-9a-f]{32})"', body).group(1)
# the fresh computer test logs in here as a browser does (tools/first-run/first_run.py web_lines): its reading of the page
_frs = importlib.util.spec_from_file_location("first_run_contract", os.path.join(ROOT, "tools", "first-run", "first_run.py"))
FR = importlib.util.module_from_spec(_frs)
_frs.loader.exec_module(FR)
check("fresh computer test: it finds the Log in form's token and the signed-out card on this page",
      FR.FORM_T.search(body) is not None and FR.FORM_T.search(body).group(1) == t_sign and FR.SIGNED_OUT_MARK in body
      and FR.KEY_PAGE == "/wrath/key")

st, _, gbody, _, _ = req("GET", "/wrath/key?guest=1", anon)
check("?guest=1 (the sign-in page's Create a guest account): the form submits itself", 'id="hk-start" data-go="1"' in gbody and "st.submit()" in gbody)
st, _, _, _, _ = req("POST", "/wrath/key/start", anon, {"t": "0" * 32})
check("start: a wrong form token is refused", st == 403)
st, _, _, _, _ = req("POST", "/wrath/key/start", anon, {"t": t_start}, origin="https://evil.example")
check("start: another site's form is refused", st == 403)
st, _, _, _, _ = req("POST", "/wrath/key/start", anon, {"t": t_start}, sfs="cross-site")
check("start: cross-site is refused", st == 403)
n_users = len(R["users"])
st, hd, _, sc, _ = req("POST", "/wrath/key/start", anon, {"t": t_start})
check("start: a guest account, signed in, straight to the profile step (owner 3 Nov 01:33)", st == 303 and hd.get("Location") == "/wrath/signin/welcome"
      and len(R["users"]) == n_users + 1, (st, hd))
check("start: after the profile step, back to the login to save (ly_next)", any(c.startswith("ly_next=/wrath/key%3Fready%3D1;") or c.startswith("ly_next=/wrath/key?ready=1;") for c in sc), sc)
sc = [c for c in sc if not c.startswith("ly_next=")]
check("start: the cookies carry no Domain (the site answers on two names)", not any("domain=" in c.lower() for c in sc), sc)
check("start: the service's own cookies come back (session, guest, the new key's), never wr_herokey",
      {c.split("=")[0] for c in sc} == {"wr_session", "wr_guest", "wr_herokey_play"}, sc)
check("start: the service saw the player's own address (its limits per network hold)", R["seen"][-1][2] == "10.0.0.7")
web = jar_add(anon, sc)
acct1 = R["users"][max(R["users"])]["acct"]
name1 = S.profile(acct1)["name"]
check("start: a made-up readable name like Brightfeather42, through the name rules", re.fullmatch(r"[A-Z][a-z]+[a-z]+\d\d", name1)
      and wp.profiles.name_problem(name1) == "", name1)
st, _, mbody, _, _ = req("GET", "/wrath/key?ready=1", jar_add(anon, sc))
check("ready: the browser is asked to keep the login at once, then Play in your browser or Get the Leyline launcher",
      'data-autosave="1"' in mbody and 'id="hk-play" href="/wrath"' in mbody and 'id="hk-get" href="/wrath/download"' in mbody
      and "Ready to play" in mbody)
mpw = (re.search(r'id="hk-mpw" name="password" type="password" readonly autocomplete="new-password" value="([^"]+)"', mbody) or [None, ""])[1]
check("start: a password made with it (4 groups of 4, easy to read), hidden until Show", re.fullmatch(r"[a-z2-9]{4}(-[a-z2-9]{4}){3}", mpw)
      and ">Show<" in mbody and mpw not in visible(mbody), mpw)
check("start: the name#tag for the browser's password manager (username / new-password), Save in this browser, Download",
      f'autocomplete="username" value="{name1}#' in mbody and 'action="/wrath/key/saved"' in mbody and "Save in this browser" in mbody
      and "Download as a text file" in mbody and "PasswordCredential" in mbody)
check("start: the text file has the name, the password and the key, and says the launcher finds it",
      "Name: " + name1 + "#" in html.unescape(mbody) and "Password: " + mpw in html.unescape(mbody) and re.search(r"Key: ovk1\.", mbody)
      and "finds it in Downloads" in mbody)
check("start: plain words, no brand names", not BRAND.search(visible(mbody)))
st, _, _, _, d = req("POST", "/wrath/api/social/password/signin", js={"name": name1 + "#" + str(S.profile(acct1)["tag"]), "password": mpw})
check("start: the made password signs the launcher in", st == 200 and (d or {}).get("token"), (st, d))
st, hd, _, _, _ = req("POST", "/wrath/key/saved", anon, {"username": "x", "password": "y"})
check("saved: Save in this browser's form goes back to the page (fields never kept)", st == 303 and hd.get("Location") == "/wrath/key?saved=1", (st, hd))
st, _, _, _, _ = req("POST", "/wrath/key/saved", anon, {"username": "x"}, origin="https://evil.example")
check("saved: another site's form is refused", st == 403)

st, hd, body, sc, _ = req("GET", "/wrath/key?new=1", web)
key1 = (re.search(r'data-key="(ovk1\.[^"]+)"', body) or [None, ""])[1]
check("page: the new key, hidden by default (dots), with Show, Download as a text file and Copy", key1 and 'class="hk-dots"' in body
      and ">Show<" in body and "Download as a text file" in body and ">Copy<" in body, body[-1500:])
check("page: the key isn't in the visible text (only Show reveals it)", key1 not in visible(body).replace(key1[:5], ""))
check("page: the name with its #tag, and the text file's words in the script (name, key, date, what it is)",
      f'data-name="{name1}#' in body and "Leyline Hero Key.txt" in body and "Made: " in body and "every realm that accepts Hero Key" in body)
check("page: plain words, no brand names, no version numbers in prose", not BRAND.search(visible(body))
      and not re.search(r"\b\d\.\d+\.\d", visible(body)))
check("page: the Hero ID is under More", body.find("<summary>More</summary>") < body.find('id="hk-hero"') and "Show my Hero ID" not in body)
check("page: a script under a nonce, fetch to our own site only", re.search(r"<script nonce=\"[^\"]+\">", body) is not None
      and "connect-src 'self'" in hd.get("Content-Security-Policy", ""))
check("the service keeps only a hash of the key (as before)", all("hash" in k and key1.split(".")[2] not in json.dumps(k) for k in R["keys"].values()))

# the cookie API: csrf from the page
csrf = re.search(r'data-csrf="([0-9a-f]{32})"', body).group(1)
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/saved", web, js={})
check("saved: without the page's token: refused", st == 403)
st, _, _, sc, d = req("POST", "/wrath/api/social/herokey/saved", web, js={}, headers={"X-Leyline-CSRF": csrf})
check("saved: ok, and the service's 'stop showing it' cookie comes back", st == 200 and d == {"ok": True}
      and any(c.startswith("wr_herokey_play=;") for c in sc), (st, d, sc))
web = jar_add(web, sc)
st, _, body, _, _ = req("GET", "/wrath/key", web)
check("fresh computer test: signed in, the page has no signed-out card", FR.SIGNED_OUT_MARK not in body)
check("page after saving: no key shown, 'Your key is saved', when made and last used, Make a new key", "data-key=" not in body
      and "Your key is saved" in body and "Made <b>31 Oct 2026</b>" in body and 'id="hk-new"' in body, visible(body)[-800:])

st, _, _, sc, d = req("POST", "/wrath/api/social/herokey/new", web, js={}, headers={"X-Leyline-CSRF": csrf})
key2 = (d or {}).get("key", "")
check("new key: a new key comes back", st == 200 and re.fullmatch(r"ovk1\.[A-Za-z0-9]{10}\.[A-Za-z0-9_-]{43}", key2) and key2 != key1, d)
web = jar_add(web, sc)
check("new key: the old one stops working on the service, the new one works", r_verify(key1) is None and r_verify(key2) == int(acct1[2:]))
st, _, body, _, _ = req("GET", "/wrath/key?made=1", web)
check("new key: the page shows it again after the reload, until it is saved", f'data-key="{key2}"' in body)

# sign in with a key on the site
anon2 = jar_add("", req("GET", "/wrath/key")[3])
b = req("GET", "/wrath/key", anon2)[2]
t2 = re.search(r'action="/wrath/key/signin" id="hk-login"><input type="hidden" name="t" value="([0-9a-f]{32})"', b).group(1)
st, hd, body, _, _ = req("POST", "/wrath/key/signin", anon2, {"t": t2, "key": key1})
check("site sign-in: the old key is refused, plainly", st == 401 and "replaced by a newer one" in body)
st, hd, _, sc, _ = req("POST", "/wrath/key/signin", anon2, {"t": t2, "key": "  " + key2 + "\n"})
check("site sign-in: the newest key signs in (the service's session cookie comes back)", st == 303 and hd["Location"] == "/wrath/key?back=1"
      and any(c.startswith("wr_session=") for c in sc), (st, sc))
st, _, body, _, _ = req("POST", "/wrath/key/signin", anon2, {"t": t2, "key": "hello"})
check("site sign-in: not a key: plain words", st == 400 and "starts with ovk1." in body)

# name change (the same profile call the launcher uses)
st, _, _, _, d = req("POST", "/wrath/api/social/profile", web, js={"name": "WoWAlice"}, headers={"X-Leyline-CSRF": csrf})
check("name: a blocked name is refused", st == 400 and d.get("error"), d)
st, _, _, _, d = req("POST", "/wrath/api/social/profile", web, js={"name": "Tamsin_7"}, headers={"X-Leyline-CSRF": csrf})
check("name: changed", st == 200 and d["me"]["name"] == "Tamsin_7", d)
for nm in ("Tamsin_8", "Tamsin_9"):
    req("POST", "/wrath/api/social/profile", web, js={"name": nm}, headers={"X-Leyline-CSRF": csrf})
st, _, _, _, d = req("POST", "/wrath/api/social/profile", web, js={"name": "Tamsin_10"}, headers={"X-Leyline-CSRF": csrf})
check("name: at most a few changes a day (the made-up first name doesn't count)", st == 429, (st, d))

# ---------------------------------------------------------------- the launcher
IP[0] = "10.0.0.8"
st, _, _, _, d = req("POST", "/wrath/api/social/account/new", js={})
tok = (d or {}).get("token", "")
lkey = (d or {}).get("key", "")
check("launcher account/new: a token, the key and the made-up name", st == 200 and len(tok) == 43 and lkey.startswith("ovk1.")
      and re.fullmatch(r"[A-Z][a-z]+\d\d", d["me"]["name"]) and d["me"]["tag"], d)
B = {"Authorization": "Bearer " + tok}
lacct = S.token_acct(tok)
st, _, _, _, d = req("GET", "/wrath/api/social/herokey", headers=B)
check("launcher herokey: its kept session: the key while it's new, when made", st == 200 and d["key"] == lkey and d["has_key"]
      and d["web_only"] is False and d["made"] and d["file_name"] == "Leyline Hero Key.txt", d)
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/saved", js={}, headers=B)
st, _, _, _, d = req("GET", "/wrath/api/social/herokey", headers=B)
check("launcher saved: the key isn't shown any more", d["key"] == "" and d["saved"] is True, d)
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/new", js={}, headers=B)
lkey2 = (d or {}).get("key", "")
check("launcher new key: made; the old one stops working", st == 200 and lkey2 and lkey2 != lkey and r_verify(lkey) is None
      and r_verify(lkey2) is not None, d)
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/signin", js={"key": lkey})
check("launcher sign-in: the old key is refused", st == 401 and "newer one" in d["error"], d)
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/signin", js={"key": lkey2})
tok2 = (d or {}).get("token", "")
check("launcher sign-in: the new key gives a token for the same account", st == 200 and S.token_acct(tok2) == lacct, d)
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/signin", js={"key": "nope"})
check("launcher sign-in: not a key: plain words", st == 400 and "ovk1." in d["error"])
for _ in range(12):
    st, _, _, _, d = req("POST", "/wrath/api/social/herokey/signin", js={"key": lkey})
check("launcher sign-in: limited per network (10 in 15 minutes)", st == 429, (st, d))

st, _, _, _, d = req("POST", "/wrath/api/social/password", js={"password": "short"}, headers=B)
check("password: too short is refused", st == 400 and "10 to 128" in d["error"])
st, _, _, _, d = req("POST", "/wrath/api/social/password", js={"password": "correct horse battery"}, headers=B)
check("password: set", st == 200 and d["has_password"] is True)
me = S.profile(lacct)
st, _, _, _, d = req("POST", "/wrath/api/social/password/signin", js={"name": f"{me['name']}#{me['tag']}", "password": "correct horse battery"})
check("password sign-in: Name#tag and the password give a token", st == 200 and S.token_acct(d["token"]) == lacct, d)
st, _, _, _, d = req("POST", "/wrath/api/social/password/signin", js={"name": f"{me['name']}#{me['tag']}", "password": "wrong password!!"})
check("password sign-in: a wrong password is refused", st == 401)
st, _, _, _, d = req("POST", "/wrath/api/social/password/signin", js={"name": "Nobody#0000", "password": "wrong password!!"})
check("password sign-in: an unknown name gets the same words", st == 400 and "don't match" in d["error"])
IP[0] = "10.0.0.9"
for _ in range(10):
    st, _, _, _, d = req("POST", "/wrath/api/social/password/signin", js={"name": f"{me['name']}#{me['tag']}", "password": "wrong password!!"})
IP[0] = "10.0.0.10"
st, _, _, _, d = req("POST", "/wrath/api/social/password/signin", js={"name": f"{me['name']}#{me['tag']}", "password": "correct horse battery"})
check("password sign-in: limited per account too (from any network)", st == 429, (st, d))
st, _, _, _, d = req("GET", "/wrath/api/social/herokey", headers=B)
check("herokey: says a password is set", d["has_password"] is True)
st, _, _, _, d = req("POST", "/wrath/api/social/password", js={"remove": True}, headers=B)
check("password: removed", st == 200 and HS.has_password(lacct) is False)

# the new-key limit
IP[0] = "10.0.0.11"
outs = [req("POST", "/wrath/api/social/herokey/new", js={}, headers=B)[0] for _ in range(6)]
check("new key: at most 5 an hour per account", outs[:4] == [200] * 4 and outs[-1] == 429, outs)

# a launcher linked on /wrath/link borrows the browser's session; without one: the website
tok3 = S.issue_token(acct1, "test")
st, _, _, _, d = req("GET", "/wrath/api/social/herokey", headers={"Authorization": "Bearer " + tok3})
check("a launcher with no website session: web_only (the launcher opens the page)", st == 200 and d["web_only"] is True)
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/new", js={}, headers={"Authorization": "Bearer " + tok3})
check("a launcher with no website session: a new key says use the website", st == 409 and d.get("web_only") is True, d)
HK.launcher_linked(tok3, acct1, web)
st, _, _, _, d = req("GET", "/wrath/api/social/herokey", headers={"Authorization": "Bearer " + tok3})
check("/wrath/link lends the browser's session: the launcher sees the key", st == 200 and d["web_only"] is False and d["has_key"], d)
check("another account's token can't use that session", HS.session(tok3, lacct) is None)

# PLAY on Wrath Unleashed from the launcher (owner 2 Nov): the player's own game login, for one start
ensured = []
wp_ensure = wp.ensure_account
wp.ensure_account = lambda u, p: ensured.append(u)
st, _, _, _, d = req("POST", "/wrath/api/social/game/login", js={}, headers=B)
check("game/login: the launcher gets its own account's game login", st == 200 and d.get("user") == lacct and d.get("pass") == "p" * 16
      and ensured == [lacct], (st, d))
st, _, _, _, d = req("POST", "/wrath/api/social/game/login", web, js={}, headers={"X-Leyline-CSRF": csrf})
check("game/login: never for a website cookie (the launcher's token only)", st == 404, (st, d))
st, _, _, _, d = req("POST", "/wrath/api/social/game/login", js={}, headers={"Authorization": "Bearer " + S.issue_token(acct1, "t2")})
check("game/login: a launcher with no website session is told to sign in on the web", st == 409 and d.get("web_only") is True, (st, d))
wp.ensure_account = lambda u, p: (_ for _ in ()).throw(wp.SoapError("down", "x"))
st, _, _, _, d = req("POST", "/wrath/api/social/game/login", js={}, headers=B)
check("game/login: the realm down: plain words, no login", st == 503 and "pass" not in d, (st, d))
wp.ensure_account = wp_ensure
check("game/login: the password is never logged", "p" * 16 not in LOGS.getvalue())

# sign out ends the launcher's own session, not a borrowed one
n_sess = len(R["sessions"])
st, _, _, _, d = req("POST", "/wrath/api/social/token/revoke", js={}, headers=B)
check("sign out: the launcher's own website session ends too", st == 200 and len(R["sessions"]) == n_sess - 1 and HS.session(tok, lacct) is None)
n_sess = len(R["sessions"])
req("POST", "/wrath/api/social/token/revoke", js={}, headers={"Authorization": "Bearer " + tok3})
check("sign out: a borrowed (browser's) session stays", len(R["sessions"]) == n_sess)

# a new account limit per network, and the service saying no
IP[0] = "10.0.0.12"
outs = [req("POST", "/wrath/api/social/account/new", js={})[0] for _ in range(4)]
check("account/new: at most 3 an hour per network", outs == [200, 200, 200, 429], outs)
IP[0] = "10.0.0.13"; R["guest_off"] = True
st, _, _, _, d = req("POST", "/wrath/api/social/account/new", js={})
check("account/new: the service has new accounts off: plain words", st == 403 and "paused" in d["error"], d)
R["guest_off"] = False

# ---------------------------------------------------------------- what can go wrong (the hero key audit, 2 Nov)
import socket, unicodedata
IP[0] = "10.0.1.1"
st, _, _, _, d = req("POST", "/wrath/api/social/account/new", js={})
tokR, keyR = d["token"], d["key"]
BR = {"Authorization": "Bearer " + tokR}
acctR = S.token_acct(tokR)
req("POST", "/wrath/api/social/herokey/saved", js={}, headers=BR)

# a pasted key: any case of the prefix, spaces, line breaks and invisible characters, inside the text file's line
k_id, k_sec = keyR[5:15], keyR[16:]
check("find_key: OVK1. and Ovk1. are read as ovk1. (the id and secret keep their case)",
      HK.find_key("OVK1." + keyR[5:]) == keyR and HK.find_key("Ovk1." + keyR[5:]) == keyR and HK.find_key(keyR.upper()) != keyR)
check("find_key: spaces, line breaks, zero-width spaces and a BOM are dropped",
      HK.find_key(f"﻿  ovk1.{k_id[:4]}​{k_id[4:]}.\r\n{k_sec[:20]} {k_sec[20:]}\n") == keyR)
check("find_key: the key inside the text file's line, and nothing for not-a-key", HK.find_key("Key: " + keyR + "\r\nMade: 1 Nov") == keyR
      and HK.find_key("hello") == "" and HK.find_key(None) == "" and HK.find_key(["ovk1."]) == "")
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/signin", js={"key": "  OVK1." + keyR[5:20] + "\n" + keyR[20:] + "​ "})
check("launcher sign-in: an uppercase prefix with a line break and a zero-width space inside still signs in",
      st == 200 and S.token_acct(d.get("token", "")) == acctR, (st, d))
anonR = jar_add("", req("GET", "/wrath/key")[3])
bR = req("GET", "/wrath/key", anonR)[2]
tR = re.search(r'action="/wrath/key/signin" id="hk-login"><input type="hidden" name="t" value="([0-9a-f]{32})"', bR).group(1)
st, hd, _, sc, _ = req("POST", "/wrath/key/signin", anonR, {"t": tR, "key": "Ovk1." + keyR[5:] + "\r\n"})
check("site sign-in: a mixed-case prefix and a trailing line break sign in", st == 303 and any(c.startswith("wr_session=") for c in sc), (st, sc))
webR = jar_add(anonR, sc)

# a stale website session: signed out plainly, never a redirect loop
st, _, body, _, _ = req("GET", "/wrath/key", webR)
check("stale: signed in first (the name is cached for a minute)", st == 200 and 'id="hk"' in body)
R["sessions"].pop(dict(p.split("=", 1) for p in webR.split("; "))["wr_session"], None)
st, hd, body, sc, _ = req("GET", "/wrath/key", webR)
check("stale: the session ran out inside the cache minute: the signed-out card, not a 303 back to itself", st == 200
      and "Location" not in hd and "Log in with your Hero Key" in body and "ran out" in body, (st, hd.get("Location")))
check("stale: the dead website cookie is cleared", any(c.startswith("wr_session=;") and "Max-Age=0" in c and "Path=/" in c for c in sc), sc)
st, hd, body, sc, _ = req("GET", "/wrath/key", "wr_session=junk")
check("stale: a junk session cookie: the signed-out card and the cookie cleared", st == 200 and "Create a guest account" in body
      and any(c.startswith("wr_session=;") for c in sc), (st, sc))
st, _, _, _, d = req("GET", "/wrath/api/social/herokey", "wr_session=junk")
check("stale: the cookie API says sign in (401), plainly", st == 401 and d.get("error"), (st, d))

# a launcher whose kept website session ran out: told to use the website (409), never a 401 that signs it out
cookieR = HS.session(tokR, acctR)[0]
R["sessions"].pop(HK.cookie_dict(cookieR)["wr_session"], None)
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/saved", js={}, headers=BR)
check("kept session ran out: saved says use the website (409 web_only), not 401", st == 409 and d.get("web_only") is True, (st, d))
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/new", js={}, headers=BR)
check("kept session ran out: a new key says use the website (409 web_only), not 401", st == 409 and d.get("web_only") is True, (st, d))
st, _, _, _, d = req("GET", "/wrath/api/social/herokey", headers=BR)
check("kept session ran out: status is web_only, and the launcher is still signed in", st == 200 and d["web_only"] is True
      and S.token_acct(tokR) == acctR, (st, d))

# two new keys at once (a double click, two windows): one key made, both get it, and it works
IP[0] = "10.0.1.2"
st, _, _, _, d = req("POST", "/wrath/api/social/account/new", js={})
tokQ, BQ = d["token"], {"Authorization": "Bearer " + d["token"]}
acctQ = S.token_acct(tokQ)
req("POST", "/wrath/api/social/herokey/saved", js={}, headers=BQ)
got = []
ths = [threading.Thread(target=lambda: got.append(req("POST", "/wrath/api/social/herokey/new", js={}, headers=BQ))) for _ in range(2)]
n_mint = len(R["mints"].get(int(acctQ[2:]), []))
for t_ in ths: t_.start()
for t_ in ths: t_.join()
keysQ = {g[4].get("key") for g in got}
check("new key race: both answers carry the same key, it works, and only one was made", [g[0] for g in got] == [200, 200]
      and len(keysQ) == 1 and r_verify(keysQ.pop()) == int(acctQ[2:]) and len(R["mints"][int(acctQ[2:])]) == n_mint + 1,
      [(g[0], g[4]) for g in got])

# names: spaces, accents as a Mac types them (e + combining accent), refused ones in plain words
nfd = unicodedata.normalize("NFD", "Zoë_Q")
st, _, _, _, d = req("POST", "/wrath/api/social/profile", js={"name": "  " + nfd + " "}, headers=BQ)
check("name: outer spaces trimmed and accents composed: saved as Zoë_Q", st == 200 and d["me"]["name"] == "Zoë_Q", (st, d))
st, _, _, _, d = req("GET", "/wrath/api/social/me", headers=BQ)
check("name: read back from me (the launcher sees the change at once)", st == 200 and d["me"]["name"] == "Zoë_Q", d)
for bad in ("x", "Blizzard_Fan", "a" * 30, "Admin", 5):
    st, _, _, _, d = req("POST", "/wrath/api/social/profile", js={"name": bad}, headers=BQ)
    check(f"name: {bad!r} refused with plain words", st == 400 and isinstance(d.get("error"), str) and 5 < len(d["error"]) < 140
          and not BRAND.search(d["error"]), (st, d))
check("name: the refused ones changed nothing", S.profile(acctQ)["name"] == "Zoë_Q")
tagQ = S.profile(acctQ)["tag"]
st, _, _, _, d = req("POST", "/wrath/api/social/password", js={"password": "zoe quiet lantern"}, headers=BQ)
st, _, _, _, d = req("POST", "/wrath/api/social/password/signin", js={"name": nfd + " #" + tagQ, "password": "zoe quiet lantern"})
check("password sign-in: the accented name typed decomposed, with a space before the #tag", st == 200 and S.token_acct(d["token"]) == acctQ, (st, d))
st, _, _, _, d = req("POST", "/wrath/api/social/password/signin", js={"name": "zoë_q", "password": "zoe quiet lantern"})
check("password sign-in: a name only one account has works without the #tag", st == 200 and S.token_acct(d["token"]) == acctQ, (st, d))

# garbage in: a 4xx with words, never a 500
for path, raw in (("herokey/signin", b"not json"), ("password/signin", b"[1,2]"), ("account/new", b"\xff\xfe")):
    c = http.client.HTTPConnection("127.0.0.1", PAGE, timeout=30)
    c.request("POST", "/wrath/api/social/" + path, body=raw, headers={"Content-Type": "application/json", "X-Forwarded-For": "10.0.1.3"})
    r = c.getresponse(); r.read(); c.close()
    check(f"garbage body to {path}: 400", r.status == 400, r.status)
st, _, _, _, d = req("POST", "/wrath/api/social/password/signin", js={"name": ["x"], "password": 5})
check("password sign-in: wrong types: a 4xx with words", 400 <= st < 500 and d.get("error"), (st, d))
st, _, _, _, d = req("POST", "/wrath/api/social/herokey/signin", js={"key": {"a": 1}})
check("key sign-in: a key that isn't text: 400 with words", st == 400 and "ovk1." in d["error"], (st, d))

# the service slow, down or answering nonsense: plain words (503), never a 500
good_url = wp.PORTAL_URL
junk = socket.socket(); junk.bind(("127.0.0.1", 0)); junk.listen(8)
def _junk():
    while True:
        try:
            cn, _ = junk.accept()
        except OSError:
            return
        try:
            cn.recv(4096); cn.sendall(b"garbage that is not http\r\n\r\n")
        finally:
            cn.close()
threading.Thread(target=_junk, daemon=True).start()
IP[0] = "10.0.1.4"
for label, url in (("down", "http://127.0.0.1:9"), ("answering nonsense", f"http://127.0.0.1:{junk.getsockname()[1]}")):
    wp.PORTAL_URL = url
    st, _, _, _, d = req("POST", "/wrath/api/social/herokey/signin", js={"key": keyR})
    check(f"service {label}: key sign-in says the website's sign-in isn't answering (503)", st == 503 and "isn't answering" in d["error"], (st, d))
    st, _, _, _, d = req("GET", "/wrath/api/social/herokey", headers=BQ)
    check(f"service {label}: status: 503 with words", st == 503 and d.get("error"), (st, d))
    st, _, body, _, _ = req("POST", "/wrath/key/signin", anonR, {"t": tR, "key": keyR})
    check(f"service {label}: the page's sign-in: 503 with words", st == 503 and "answering right now" in body, st)
wp.PORTAL_URL = good_url
junk.close()
real_info = HK.key_info
HK.key_info = lambda *a, **k: 1 / 0
st, _, _, _, d = req("GET", "/wrath/api/social/herokey", headers=BQ)
HK.key_info = real_info
check("a bug on our side: 503 with words, not a 500 page", st == 503 and "our side" in d["error"], (st, d))

# the release audit's own test account closes itself; nobody else's can
IP[0] = "10.0.1.5"
st, _, _, _, d = req("POST", "/wrath/api/social/account/new", js={"audit": True})
tokA, BA = d["token"], {"Authorization": "Bearer " + d["token"]}
acctA = S.token_acct(tokA)
check("audit account: marked at birth", HS.audit_born(acctA) is not None)
st, _, _, _, d = req("POST", "/wrath/api/social/audit/close", js={}, headers=BQ)
check("audit close: an ordinary account can't close this way (403), and nothing changed", st == 403 and S.profile(acctQ) is not None
      and S.token_acct(tokQ) == acctQ, (st, d))
st, _, _, _, d = req("POST", "/wrath/api/social/audit/close", web, js={}, headers={"X-Leyline-CSRF": csrf})
check("audit close: never through a website cookie", st in (401, 403, 404), (st, d))
os.environ["WU_AUDIT_CLOSE"] = "0"
st, _, _, _, d = req("POST", "/wrath/api/social/audit/close", js={}, headers=BA)
os.environ.pop("WU_AUDIT_CLOSE")
check("audit close: WU_AUDIT_CLOSE=0 turns it off", st == 403 and S.profile(acctA) is not None, (st, d))
IP[0] = "10.0.1.6"
st, _, _, _, d = req("POST", "/wrath/api/social/account/new", js={"audit": True})
tokF, BF = d["token"], {"Authorization": "Bearer " + d["token"]}
acctF = S.token_acct(tokF)
pq = S.profile(acct1)
S.request(acctF, f"{pq['name']}#{pq['tag']}")
st, _, _, _, d = req("POST", "/wrath/api/social/audit/close", js={}, headers=BF)
check("audit close: an account with a friend request stays (409): closing never takes from another player", st == 409
      and S.profile(acctF) is not None, (st, d))
# GET audit/presence: where the audit's test account's game got to (the first-run test); nobody else's
HK._presence_cache.update(at=0.0, text=None)
soap_was = wp.ac_soap
soap_calls = []
def fake_soap(cmd):
    soap_calls.append(cmd)
    return ("-[         Account][   Character][             IP][Map][Zone][Exp][GMLev]-\n"
            f"-[{acctA:<16}][Zarathea    ][10.0.1.5       ][571][4395][  2][    0]-\n"
            "-[BOTSOMEONE      ][Botty       ][127.0.0.1      ][  0][  12][  2][    0]-")
wp.ac_soap = fake_soap
st, _, _, _, d = req("GET", "/wrath/api/social/audit/presence", headers=BQ)
check("presence: an ordinary account gets 404 (only the audit's own test account)", st == 404, (st, d))
st, _, _, _, d = req("GET", "/wrath/api/social/audit/presence", web, headers={"X-Leyline-CSRF": csrf})
check("presence: never through a website cookie", st in (401, 403, 404), (st, d))
st, _, _, _, d = req("GET", "/wrath/api/social/audit/presence", headers=BA)
check("presence: in the world, its character and map, no login given yet", st == 200 and d == {"account_online": True, "in_world": True,
      "character": "Zarathea", "map": 571, "login_given": False}, (st, d))
check("presence: the console is asked a fixed command", soap_calls == ["account onlinelist"], soap_calls)
HS.note(acctA, "game.login", "launcher")
HK._presence_cache.update(at=0.0, text=None)
wp.ac_soap = lambda cmd: "There are no online accounts."
st, _, _, _, d = req("GET", "/wrath/api/social/audit/presence", headers=BA)
check("presence: not in the world yet, but the launcher was given its game login", st == 200 and d["in_world"] is False
      and d["character"] is None and d["login_given"] is True, (st, d))
HK._presence_cache.update(at=0.0, text=None)
wp.ac_soap = lambda cmd: (_ for _ in ()).throw(wp.SoapError("down", "x"))
st, _, _, _, d = req("GET", "/wrath/api/social/audit/presence", headers=BA)
check("presence: the realm down is 503 with words, not a 500", st == 503 and d.get("error"), (st, d))
check("presence: the console's rows parse in both shapes", HK.parse_online("-|ABC |Zara |1.2.3.4 |0 |12 |2 |0 |", "abc") == {"character": "Zara", "map": 0}
      and HK.parse_online("-[Account][Character]-", "ACCOUNT") is None and HK.parse_online("", "A") is None)
wp.ac_soap = soap_was
HK._presence_cache.update(at=0.0, text=None)
n_sess = len(R["sessions"])
HS.set_password(acctA, "audit password 123")
st, _, _, _, d = req("POST", "/wrath/api/social/audit/close", js={}, headers=BA)
check("audit close: closed", st == 200 and d == {"ok": True}, (st, d))
check("audit close: the profile, launcher sign-ins, password and marker are gone, its website session signed out",
      S.profile(acctA) is None and S.token_acct(tokA) is None and not HS.has_password(acctA) and HS.audit_born(acctA) is None
      and HS.session(tokA, acctA) is None and len(R["sessions"]) == n_sess - 1
      and wp.profiles.store.q("SELECT COUNT(*) FROM profile_history WHERE acct = ?", (acctA,))[0][0] == 0)
st, _, _, _, d = req("GET", "/wrath/api/social/me", headers=BA)
check("audit close: the old token is signed out (401)", st == 401, (st, d))

# ---------------------------------------------------------------- the text file, the made-up names
t = HK.text_file("Brightfeather42#1234", "ovk1.AAAAAAAAAA." + "b" * 43, "31 October 2026")
check("text file: the name, the key, the date and one line on what it is", t.startswith("Leyline Hero Key\r\n") and "Name: Brightfeather42#1234" in t
      and "Key: ovk1.AAAAAAAAAA." in t and "Made: 31 October 2026" in t and "every realm that accepts Hero Key" in t and not BRAND.search(t))
names = {HK.make_name() for _ in range(200)}
check("made-up names: readable, varied, all pass the name rules", len(names) > 100 and all(wp.profiles.name_problem(n) == "" for n in names))
check("made-up names: a name the rules refuse is never given", HK.make_name(lambda n: "no" if n[0] != "Z" else "") .startswith("Wanderer"))
check("sealed sessions: open only with the launcher's token", HK.unseal("a" * 43, HK.seal("a" * 43, "wr_session=x")) == "wr_session=x"
      and HK.unseal("b" * 43, HK.seal("a" * 43, "wr_session=x")) is None)
check("passwords: scrypt hashes, checked", HK.hash_password("x" * 12).startswith(("scrypt$", "pbkdf2$"))
      and HK.check_password("x" * 12, HK.hash_password("x" * 12)) and not HK.check_password("y" * 12, HK.hash_password("x" * 12)))

# ---------------------------------------------------------------- nothing kept or logged
all_keys = [key1, key2, lkey, lkey2]
logged = LOGS.getvalue()
check("no key in the logs", not any(k and k in logged for k in all_keys) and "ovk1." not in logged, logged[-500:])
check("no password in the logs", "correct horse" not in logged)
raw_db = open(os.path.join(TMP, "herokeys.sqlite3"), "rb").read() + b"".join(
    open(os.path.join(TMP, f), "rb").read() for f in os.listdir(TMP) if f.startswith("herokeys.sqlite3-"))
check("the store holds no key, no password and no plain website session", b"ovk1." not in raw_db and b"correct horse" not in raw_db
      and not any(t.encode() in raw_db for t in R["sessions"]), len(raw_db))

# ---------------------------------------------------------------- deploy
bat = open(os.path.join(STREAM, "Update sign-in and warm-up.bat")).read()
fl = open(os.path.join(STREAM, "fastloop.sh")).read()
check("deploy: the .bat sends herokeys.py (and strips its line ends)", 'if exist herokeys.py set "LINKS=%LINKS% herokeys.py"' in bat
      and " herokeys.py " in bat.split("sed -i", 1)[1])
check("deploy: fastloop.sh backs up, puts back and installs herokeys.py", '"$BK/herokeys.py"' in fl and 'install -m 644 herokeys.py' in fl)
check("deploy: the website's /account/key opens /wrath/key (GET only, a backup and the old file back if Caddy refuses)",
      "path /account/key" in fl and "method GET" in fl and "redir @hkpage /wrath/key" in fl and "Caddyfile.hk" in fl)
check("links: the realm list and the profile page go to /wrath/key", 'href="/wrath/key"' in open(PORTAL).read()
      and 'href="/wrath/key"' in open(os.path.join(STREAM, "profiles.py")).read())

print(f"{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
