"""Your Hero Key (stream/herokeys.py) through the real portal (stream/wrath_portal.py) with the real social.py (tokens,
names), profiles.py (the name rules) and hub.py (the look), against a fake website sign-in service that keeps keys the
way the real one does (only a SHA-256 of the secret; a new key revokes the old one; shown once).

Covers: the page signed out (Get your Hero Key, Sign in with it; the Leyline look, no old logo, crest or side art, no
brand words), a new account (a made-up readable name through the name rules, the key shown once: hidden, Show,
Download as a text file, Copy), saved (the service stops showing it), a new key (the old one stops working), the
launcher's calls (account/new, herokey/signin, password set and sign-in, herokey with its kept session, a /wrath/link
launcher borrowing the browser's, web_only without one), the name change (blocked names, the daily limit), the text
file, the limits, cross-site forms, sign out ending the launcher's own session, no key or password in the logs or the
store, and the deploy lists.

    python3 -I stream/tests/test_herokeys.py stream/wrath_portal.py
"""
import hashlib, http.client, importlib.util, io, json, logging, os, re, secrets, sys, tempfile, threading, urllib.parse
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
            return self.reply(303, loc="/", cookies=[f"wr_session={r_session(uid)}; Path=/; HttpOnly; Secure; SameSite=Lax",
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
check("page signed out: 200, one card: Get your Hero Key and Sign in with your Hero Key", st == 200 and "Get your Hero Key" in body
      and "Sign in with your Hero Key" in body and 'action="/wrath/key/start"' in body and 'action="/wrath/key/signin"' in body, body[:300])
check("page: the Leyline look (the hub's header and tokens), noindex", 'class="hb-top"' in body and "--lk-surface" in body
      and hd.get("X-Robots-Tag", "").startswith("noindex"))
check("page: no old logo, crest, side art or screenshot prompt in the card", not re.search(r"(?i)logo\.webp|crest|screenshot|<img", card), card[:400])
check("page: plain words, no game, company or old site names", not BRAND.search(visible(body)), BRAND.findall(visible(body)))
check("page: one account for Wrath Unleashed and every realm that accepts Hero Key", "every realm that accepts Hero Key" in body)
check("page: a form cookie for the signed-out forms (Path=/wrath/key, HttpOnly, Secure, Strict)", any(
    c.startswith("ly_hk=") and "Path=/wrath/key" in c and "HttpOnly" in c and "SameSite=Strict" in c for c in sc), sc)
anon = jar_add("", sc)
t_start = re.search(r'action="/wrath/key/start"[^>]*><input type="hidden" name="t" value="([0-9a-f]{32})"', body).group(1)
t_sign = re.search(r'action="/wrath/key/signin"><input type="hidden" name="t" value="([0-9a-f]{32})"', body).group(1)

st, _, _, _, _ = req("POST", "/wrath/key/start", anon, {"t": "0" * 32})
check("start: a wrong form token is refused", st == 403)
st, _, _, _, _ = req("POST", "/wrath/key/start", anon, {"t": t_start}, origin="https://evil.example")
check("start: another site's form is refused", st == 403)
st, _, _, _, _ = req("POST", "/wrath/key/start", anon, {"t": t_start}, sfs="cross-site")
check("start: cross-site is refused", st == 403)
n_users = len(R["users"])
st, hd, _, sc, _ = req("POST", "/wrath/key/start", anon, {"t": t_start})
check("start: a new account, then the page", st == 303 and hd.get("Location") == "/wrath/key?new=1" and len(R["users"]) == n_users + 1, (st, hd))
check("start: the service's own cookies come back (session, guest, the new key's), never wr_herokey",
      {c.split("=")[0] for c in sc} == {"wr_session", "wr_guest", "wr_herokey_play"}, sc)
check("start: the service saw the player's own address (its limits per network hold)", R["seen"][-1][2] == "10.0.0.7")
web = jar_add(anon, sc)
acct1 = R["users"][max(R["users"])]["acct"]
name1 = S.profile(acct1)["name"]
check("start: a made-up readable name like Brightfeather42, through the name rules", re.fullmatch(r"[A-Z][a-z]+[a-z]+\d\d", name1)
      and wp.profiles.name_problem(name1) == "", name1)

st, hd, body, sc, _ = req("GET", "/wrath/key?new=1", web)
key1 = (re.search(r'data-key="(ovk1\.[^"]+)"', body) or [None, ""])[1]
check("page: the new key, hidden by default (dots), with Show, Download as a text file and Copy", key1 and 'class="hk-dots"' in body
      and ">Show<" in body and "Download as a text file" in body and ">Copy<" in body and "Welcome! Your account is ready" in body, body[-1500:])
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
t2 = re.search(r'action="/wrath/key/signin"><input type="hidden" name="t" value="([0-9a-f]{32})"', b).group(1)
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
