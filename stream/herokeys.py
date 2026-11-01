"""Your Hero Key: the website's Hero Key page and the account setup the launcher shares (owner 2026-10-31 19:15: "have Hero
Key setup in launcher available too and remove the screenshot area make getting your hero key simple and can change
username and pass but its initially generated and downloadable as text file and can be shown etc").

Who keeps the keys: the website's sign-in service (the Rust site on this host, PORTAL_URL), exactly as before. It makes a
key (`ovk1.<id>.<secret>`), keeps only the id and a SHA-256 of the secret, shows a new key once, and a new key revokes the
old one at once. Nothing here stores, logs or e-mails a key: this module asks that service on the player's behalf, with
the player's own website session, and hands the answer straight back. The service's own forms and limits stay in charge
(its CSRF token, its per-network and per-key limits, its 10 new keys an hour); ours sit in front of them.

The page (Leyline look, one card):
    GET  /wrath/key                 signed in: "Your Hero Key": a key made just now (a new account's, or a new one) hidden
                                    with Show, Download as a text file and Copy; otherwise when it was made and last used,
                                    and Make a new key (asks first). Under More: change your name, set a password, Hero ID.
                                    Signed out: Get your Hero Key (a new account with a made-up name) or Sign in with it.
    POST /wrath/key/start           a new account: the service's guest account, a name like "Brightfeather42" (checked
                                    against stream/namecheck.py), its Hero Key shown once
    POST /wrath/key/signin          sign in with a pasted key (the service's /auth/key; its cookie comes back to you)
The service's own GET /account/key goes to this page (fastloop.sh adds the redirect to the site's Caddyfile).

The JSON, under /wrath/api/social (stream/social.py's sign-in: the launcher's token, or the cookie with X-Leyline-CSRF):
    POST account/new                (no sign-in) a new account for the launcher: {token, key, me}
    POST herokey/signin {key}       (no sign-in) the launcher signs in with a key: {token, me}
    POST password/signin {name, password}   (no sign-in) Name#1234 and the password set below: {token, me}
    GET  herokey                    {has_key, made, last_used, saved, key (only while it is new), has_password, web_only}
    POST herokey/new                a new key (the old one stops working): {key}
    POST herokey/saved              the player saved it (downloaded, copied): the service stops showing it
    POST password {password} | {remove: true}   an optional password for signing in to the launcher
The name itself is POST profile {name} (stream/profiles.py set_name: the blocklist, at most 3 changes a day).

The launcher and the service: a launcher has no website cookie, so a launcher that signed in here (a new account, a key)
gets its own website session, and one linked on /wrath/link borrows the browser's. That session is kept encrypted with a
key made from the launcher's token (which we never keep: only its hash), so the file alone opens nothing; signing the
launcher out ends a session it owns. A launcher without one (or whose browser signed out) is told to use this page.

Passwords: optional, at least 10 characters, kept as scrypt hashes (profiles' folder, herokeys.sqlite3). They sign in
to the launcher; the website's own sign-in stays the Hero Key and Google (the service has no password form for players).

Settings: WU_HEROKEYS=0 (off). Standard library only.
"""
from __future__ import annotations

import hashlib
import hmac
import html
import json
import logging
import os
import re
import secrets
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

try:
    import sqlite3
except ImportError:   # an image without it: the page still works, the launcher's sessions and passwords don't
    sqlite3 = None

log = logging.getLogger("wrath-portal.herokeys")

PAGE = "/wrath/key"
START = PAGE + "/start"
SIGNIN = PAGE + "/signin"
FORM_MAX = 2048
KEY_RE = re.compile(r"ovk1\.[A-Za-z0-9]{10}\.[A-Za-z0-9_-]{43}")
KEEP_COOKIES = ("wr_session", "wr_guest", "wr_herokey_play")   # the service's cookies we pass on (never wr_herokey)
PASS_MIN, PASS_MAX = 10, 128
NEW_KEYS_PER_HOUR = 5
NEW_ACCOUNTS_PER_HOUR = 3        # per network
SIGNINS_PER_15MIN = 10           # per network, and per account for passwords
FILE_NAME = "Leyline Hero Key.txt"

_get = None
store = None


def bind(getter) -> None:
    global _get
    _get = getter


def P(name):
    try:
        return _get(name) if _get is not None else None
    except KeyError:
        return None


def _ratelimit():
    m = sys.modules.get("wu_ratelimit")
    if m is None:
        import importlib.util
        spec = importlib.util.spec_from_file_location("wu_ratelimit", os.path.join(os.path.dirname(os.path.abspath(__file__)), "ratelimit.py"))
        m = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(m)
        sys.modules["wu_ratelimit"] = m
    return m


_lim = None


def _hit(key: str, limit: int, seconds: float) -> bool:
    global _lim
    if _lim is None:
        _lim = _ratelimit().Limiter(3600)
    return _lim.hit(key, limit, seconds)


def host() -> str:
    return P("PUBLIC_HOST") or "play.openvanilla.app"


def _soc():
    m = P("social")
    return getattr(m, "store", None) if m is not None else None


# ------------------------------------------------------------------ made-up names
_A = ("Bright", "Swift", "Quiet", "Amber", "Silver", "Ember", "Frost", "Storm", "Sun", "Moon", "Iron", "Oak", "River", "Ash",
      "Copper", "Misty", "Golden", "Wild", "Brave", "Clever", "Lucky", "Kind", "Bold", "Sky", "Stone", "Wren", "Coral", "Maple")
_B = ("feather", "fox", "wolf", "hawk", "bloom", "brook", "spark", "stone", "wind", "leaf", "otter", "finch", "heart", "song",
      "lantern", "ridge", "cliff", "meadow", "thorn", "shield", "badger", "comet", "harbor", "willow", "pine", "raven")


def make_name(check=None) -> str:
    """A readable made-up name like "Brightfeather42", one that passes check(name) -> "" (the name rules)."""
    check = check or _name_problem
    for _ in range(40):
        n = secrets.choice(_A) + secrets.choice(_B) + str(secrets.randbelow(90) + 10)
        if not check(n):
            return n
    return "Wanderer" + str(secrets.randbelow(9000) + 1000)


def _name_problem(name: str) -> str:
    prof = P("profiles")
    if prof is not None and hasattr(prof, "name_problem"):
        return prof.name_problem(name)
    soc = P("social")
    return soc.name_error(name) if soc is not None else ""


# ------------------------------------------------------------------ the website's sign-in service
class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


_opener = urllib.request.build_opener(_NoRedirect)


def cookie_dict(header: str) -> dict:
    out = {}
    for part in (header or "").split(";"):
        k, _, v = part.strip().partition("=")
        if k:
            out[k] = v
    return out


def cookie_header(d: dict) -> str:
    return "; ".join(f"{k}={v}" for k, v in d.items() if v)


def merge(jar: dict, set_cookies: list[str]) -> dict:
    """The cookies after the service's Set-Cookie lines (an empty value or Max-Age=0 removes one)."""
    jar = dict(jar)
    for sc in set_cookies:
        first, *attrs = sc.split(";")
        k, _, v = first.strip().partition("=")
        gone = not v or any(a.strip().lower() in ("max-age=0",) or a.strip().lower().startswith("expires=thu, 01 jan 1970") for a in attrs)
        if gone:
            jar.pop(k, None)
        else:
            jar[k] = v
    return jar


def passed_on(set_cookies: list[str]) -> list[str]:
    """The service's cookies the browser should get (its session, guest and new-key cookies), as it sent them."""
    return [c for c in set_cookies if c.split("=", 1)[0].strip() in KEEP_COOKIES]


def realm(method: str, path: str, cookie: str, ip: str, ua: str, form: dict | None = None, script: bool = False):
    """One call to the sign-in service as the player: (status, body text, Set-Cookie lines). Status 0: not answering."""
    base = (P("PORTAL_URL") or "").rstrip("/")
    data = urllib.parse.urlencode(form).encode() if form is not None else (b"" if method == "POST" else None)
    hd = {"Host": host(), "X-Forwarded-For": ip or "", "X-Real-IP": ip or "", "User-Agent": (ua or "Leyline")[:200],
          "Origin": f"https://{host()}", "Sec-Fetch-Site": "same-origin", "Accept": "application/json" if script else "text/html"}
    if cookie:
        hd["Cookie"] = cookie
    if script:
        hd["X-Requested-With"] = "wenilla"
    if data is not None:
        hd["Content-Type"] = "application/x-www-form-urlencoded"
    req = urllib.request.Request(base + path, data=data, method=method, headers=hd)
    try:
        with _opener.open(req, timeout=15) as r:
            return r.status, r.read(262144).decode("utf-8", "replace"), r.headers.get_all("Set-Cookie") or []
    except urllib.error.HTTPError as e:
        try:
            body = e.read(65536).decode("utf-8", "replace")
        except Exception:   # noqa: BLE001
            body = ""
        return e.code, body, (e.headers.get_all("Set-Cookie") if e.headers else None) or []
    except (urllib.error.URLError, OSError, ValueError):
        return 0, "", []


class Down(Exception):
    """The sign-in service isn't answering (or answered something we can't read)."""


class Refused(Exception):
    def __init__(self, status: int, text: str):
        super().__init__(text)
        self.status, self.text = status, text


def _first(rx: str, s: str) -> str:
    m = re.search(rx, s)
    return html.unescape(m.group(1)).strip() if m else ""


def key_info(cookie: str, ip: str, ua: str) -> dict:
    """What the service's key page says about this account's key: {signed_in, key (only while new), csrf, made, last_used,
    saved, has_key, hero_id, set_cookies}. Its first visit makes an account's first key (nothing to revoke)."""
    st, body, sc = realm("GET", "/account/key", cookie, ip, ua)
    if st == 0 or st >= 500:
        raise Down(f"key page {st}")
    if 300 <= st < 400 or st in (401, 403):
        return {"signed_in": False, "set_cookies": sc}
    if st != 200:
        raise Down(f"key page {st}")
    key = _first(r'<code id="keytext">([^<]+)</code>', body)
    key = key if KEY_RE.fullmatch(key) else ""
    made = _first(r"Key made <b>([^<]+)</b>", body)
    used = _first(r"Last used <b>([^<]+)</b>", body)
    hero = _first(r'<code id="herotext">([^<]+)</code>', body)
    saved = ("Your Hero Key is saved" in body) or bool(re.search(r'id="saved"\s+checked', body))
    return {"signed_in": True, "key": key, "csrf": _first(r'name="_csrf" value="([^"]+)"', body), "made": made,
            "last_used": used, "saved": saved and not key, "has_key": bool(key or made), "set_cookies": sc,
            "hero_id": hero if re.fullmatch(r"ovh1\.[A-Za-z0-9_-]{20,64}", hero) else ""}


def mint(cookie: str, csrf: str, ip: str, ua: str) -> tuple[str, list[str]]:
    """A new key from the service (the old one stops working): (key, Set-Cookie lines). Raises Refused or Down."""
    st, body, sc = realm("POST", "/account/key/mint.json", cookie, ip, ua, {"_csrf": csrf}, script=True)
    if st == 429:
        raise Refused(429, "You've made a few keys this hour. Wait a while and try again.")
    if st in (401, 403) or 300 <= st < 400:
        raise Refused(401, "Your website sign-in ran out. Sign in again.")
    if st != 200:
        raise Down(f"mint {st}")
    try:
        key = str(json.loads(body).get("key") or "")
    except (ValueError, AttributeError):
        key = ""
    if not KEY_RE.fullmatch(key):
        raise Down("mint: no key")
    return key, sc


def mark_saved(cookie: str, csrf: str, ip: str, ua: str) -> list[str]:
    st, _, sc = realm("POST", "/account/key/saved", cookie, ip, ua, {"_csrf": csrf}, script=True)
    if st == 0 or st >= 500:
        raise Down(f"saved {st}")
    return sc


def session_from(set_cookies: list[str]) -> str:
    return merge({}, set_cookies).get("wr_session", "")


def sign_in_with_key(key: str, ip: str, ua: str) -> list[str]:
    """The service's own Hero Key sign-in: its Set-Cookie lines (a new website session). Raises Refused or Down."""
    st, body, sc = realm("POST", "/auth/key", "", ip, ua, {"key": key})
    if st == 0 or st >= 500:
        raise Down(f"auth/key {st}")
    if session_from(sc):
        return sc
    if "Too many" in body:
        raise Refused(429, "Too many tries. Wait 15 minutes and try again.")
    raise Refused(401, "That Hero Key didn't work. It may have been replaced by a newer one: use your newest key.")


def new_guest(ip: str, ua: str) -> list[str]:
    """A new account from the service (its guest account, with a Hero Key made at once): its Set-Cookie lines."""
    st, body, sc = realm("POST", "/auth/guest", "", ip, ua, {})
    if st == 0 or st >= 500:
        raise Down(f"auth/guest {st}")
    if session_from(sc):
        return sc
    if "Too many" in body:
        raise Refused(429, "Too many new accounts from here. Wait an hour and try again.")
    if "off on this realm" in body:
        raise Refused(403, "New accounts are paused right now. Sign in with Google instead.")
    raise Refused(403, "Couldn't make an account right now. Try again shortly.")


def game_account(cookie: str, ip: str, ua: str) -> str:
    getter = P("portal_get")
    st, play = getter("/api/play", cookie, ip, ua) if getter else (0, {})
    user = str(play.get("user", "")) if isinstance(play, dict) else ""
    if st != 200 or not re.fullmatch(r"[A-Za-z0-9]{1,16}", user):
        raise Down(f"api/play {st}")
    return user.upper()


def realm_logout(cookie: str, ip: str, ua: str) -> None:
    realm("POST", "/logout", cookie, ip, ua, {})


# ------------------------------------------------------------------ the store: the launchers' sessions and passwords
SCHEMA = """
CREATE TABLE IF NOT EXISTS hk_links (hash TEXT PRIMARY KEY, acct TEXT NOT NULL, blob BLOB NOT NULL, own INTEGER NOT NULL,
  created INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS hk_passwords (acct TEXT PRIMARY KEY, hash TEXT NOT NULL, set_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS hk_events (id INTEGER PRIMARY KEY AUTOINCREMENT, at INTEGER NOT NULL, acct TEXT NOT NULL,
  what TEXT NOT NULL, via TEXT NOT NULL DEFAULT '');
"""


def _stream(k: bytes, nonce: bytes, n: int) -> bytes:
    out, i = b"", 0
    while len(out) < n:
        out += hmac.new(k, nonce + i.to_bytes(4, "big"), hashlib.sha256).digest()
        i += 1
    return out[:n]


def seal(token: str, text: str) -> bytes:
    """text, encrypted and authenticated with a key made from the launcher's token (HMAC-SHA256 as the stream and the tag)."""
    k = hmac.new(b"leyline-herokey-link", token.encode(), hashlib.sha256).digest()
    nonce = secrets.token_bytes(16)
    pt = text.encode("utf-8")
    ct = bytes(a ^ b for a, b in zip(pt, _stream(k, nonce, len(pt))))
    return nonce + ct + hmac.new(k, b"tag" + nonce + ct, hashlib.sha256).digest()[:16]


def unseal(token: str, blob: bytes) -> str | None:
    if not isinstance(blob, (bytes, bytearray)) or len(blob) < 32:
        return None
    k = hmac.new(b"leyline-herokey-link", token.encode(), hashlib.sha256).digest()
    nonce, ct, tag = bytes(blob[:16]), bytes(blob[16:-16]), bytes(blob[-16:])
    if not hmac.compare_digest(tag, hmac.new(k, b"tag" + nonce + ct, hashlib.sha256).digest()[:16]):
        return None
    return bytes(a ^ b for a, b in zip(ct, _stream(k, nonce, len(ct)))).decode("utf-8", "replace")


# scrypt takes 16 MB a call and wu-portal has 128 MB: at most a few at once, whatever the number of callers
_KDF = threading.BoundedSemaphore(3)


def hash_password(pw: str) -> str:
    salt = secrets.token_bytes(16)
    try:
        with _KDF:
            d = hashlib.scrypt(pw.encode(), salt=salt, n=2 ** 14, r=8, p=1, dklen=32)
        return f"scrypt$16384$8$1${salt.hex()}${d.hex()}"
    except (AttributeError, ValueError):   # an OpenSSL without scrypt
        d = hashlib.pbkdf2_hmac("sha256", pw.encode(), salt, 300000)
        return f"pbkdf2$300000${salt.hex()}${d.hex()}"


def check_password(pw: str, stored: str) -> bool:
    try:
        parts = stored.split("$")
        if parts[0] == "scrypt":
            n, r, p, salt, want = int(parts[1]), int(parts[2]), int(parts[3]), bytes.fromhex(parts[4]), bytes.fromhex(parts[5])
            if n > 2 ** 15 or r > 8 or p > 1:
                return False   # never more work than we store
            with _KDF:
                got = hashlib.scrypt(pw.encode(), salt=salt, n=n, r=r, p=p, dklen=len(want))
        elif parts[0] == "pbkdf2":
            it, salt, want = int(parts[1]), bytes.fromhex(parts[2]), bytes.fromhex(parts[3])
            got = hashlib.pbkdf2_hmac("sha256", pw.encode(), salt, it)
        else:
            return False
        return hmac.compare_digest(got, want)
    except (ValueError, IndexError, AttributeError):
        return False


class Store:
    def __init__(self, data_dir: str | None = None):
        if sqlite3 is None:
            raise RuntimeError("sqlite3 is missing")
        d = data_dir or os.environ.get("WU_PORTAL_DATA", "/data")
        self.path = os.path.join(d, "herokeys.sqlite3")
        self.lock = threading.RLock()
        self.db = sqlite3.connect(self.path, check_same_thread=False, isolation_level=None, timeout=10)
        try:
            os.chmod(self.path, 0o600)
        except OSError:
            pass
        self.db.executescript(SCHEMA)

    def q(self, sql, args=()):
        with self.lock:
            return self.db.execute(sql, args).fetchall()

    def x(self, sql, args=()):
        with self.lock:
            return self.db.execute(sql, args).rowcount

    def note(self, acct: str, what: str, via: str = "") -> None:
        """What happened (never a key or a password): "key.new", "key.saved", "name", "password.set" ..."""
        self.x("INSERT INTO hk_events (at, acct, what, via) VALUES (?,?,?,?)", (int(time.time()), acct, what, via))

    @staticmethod
    def _h(token: str) -> str:
        return hashlib.sha256(token.encode()).hexdigest()

    def link(self, token: str, acct: str, cookie: str, own: bool) -> None:
        self.x("INSERT OR REPLACE INTO hk_links (hash, acct, blob, own, created) VALUES (?,?,?,?,?)",
               (self._h(token), acct, seal(token, cookie), 1 if own else 0, int(time.time())))

    def session(self, token: str, acct: str) -> tuple[str, bool] | None:
        r = self.q("SELECT acct, blob, own FROM hk_links WHERE hash = ?", (self._h(token),))
        if not r or r[0][0] != acct:
            return None
        c = unseal(token, r[0][1])
        return (c, bool(r[0][2])) if c else None

    def forget(self, token: str) -> None:
        self.x("DELETE FROM hk_links WHERE hash = ?", (self._h(token),))

    def has_password(self, acct: str) -> bool:
        return bool(self.q("SELECT 1 FROM hk_passwords WHERE acct = ?", (acct,)))

    def set_password(self, acct: str, pw: str | None) -> None:
        if pw is None:
            self.x("DELETE FROM hk_passwords WHERE acct = ?", (acct,))
        else:
            self.x("INSERT OR REPLACE INTO hk_passwords (acct, hash, set_at) VALUES (?,?,?)", (acct, hash_password(pw), int(time.time())))

    def password_ok(self, acct: str, pw: str) -> bool:
        r = self.q("SELECT hash FROM hk_passwords WHERE acct = ?", (acct,))
        if not r:
            check_password(pw, "scrypt$16384$8$1$" + "00" * 16 + "$" + "00" * 32)   # the same work either way
            return False
        return check_password(pw, r[0][0])


def setup() -> Store | None:
    global store
    if store is not None:
        return store
    if os.environ.get("WU_HEROKEYS", "1") == "0" or sqlite3 is None:
        return None
    try:
        store = Store()
    except Exception:   # noqa: BLE001: the page works without it
        log.exception("herokeys: the store didn't open")
        store = None
    return store


def on() -> bool:
    return os.environ.get("WU_HEROKEYS", "1") != "0"


# ------------------------------------------------------------------ the text file and the words
def text_file(name: str, key: str, day: str | None = None) -> str:
    """The .txt a player keeps: the name, the key, the date and one line on what it is (the launcher writes the same)."""
    day = day or time.strftime("%d %B %Y", time.gmtime()).lstrip("0")
    return (f"Leyline Hero Key\r\n\r\nName: {name}\r\nKey: {key}\r\nMade: {day}\r\n\r\n"
            "This key signs you in to your Leyline account on Wrath Unleashed and on every realm that accepts Hero Key. "
            "Keep it private, like a password.\r\n")


def password_problem(pw, name: str = "") -> str:
    if not isinstance(pw, str) or not (PASS_MIN <= len(pw) <= PASS_MAX):
        return f"Passwords are {PASS_MIN} to {PASS_MAX} characters."
    if name and pw.lower() == name.lower():
        return "Pick a password that isn't your name."
    if len(set(pw)) < 4:
        return "Pick a password that's harder to guess."
    return ""


# ------------------------------------------------------------------ the JSON (from stream/social.py)
NO_AUTH = ("account/new", "herokey/signin", "password/signin")
WITH_AUTH_GET = ("herokey",)
WITH_AUTH_POST = ("herokey/new", "herokey/saved", "password")


def handles(sub: str, method: str, authed: bool) -> bool:
    if not on():
        return False
    if not authed:
        return method == "POST" and sub in NO_AUTH
    return sub in (WITH_AUTH_GET if method == "GET" else WITH_AUTH_POST)


def _me(acct: str) -> dict:
    soc = _soc()
    p = soc.ensure(acct) if soc is not None else {"name": "", "tag": "", "pid": ""}
    return {"pid": p.get("pid", ""), "name": p.get("name", ""), "tag": p.get("tag", "")}


def _issue(acct: str, how: str) -> str:
    soc = _soc()
    return soc.issue_token(acct, f"Leyline launcher, {how} " + time.strftime("%Y-%m-%d", time.gmtime()))


def _set_made_name(acct: str) -> None:
    """A new account's first name: made up, readable, and through the name rules like any other."""
    prof = P("profiles")
    soc = _soc()
    if soc is None:
        return
    soc.ensure(acct)
    name = make_name()
    try:
        if prof is not None and getattr(prof, "store", None) is not None:
            prof.set_name(acct, name, by="made")
        else:
            soc.set_profile(acct, {"name": name})
    except Exception:   # noqa: BLE001: the account works with the service's own name
        log.warning("herokeys: couldn't give a new account its name")


def new_account(ip: str, ua: str) -> dict:
    """{cookies (Set-Cookie for the browser), cookie (the jar as a header), acct, key}. Raises Refused or Down."""
    b = _ratelimit().bucket(ip)
    if not _hit("new|" + b, NEW_ACCOUNTS_PER_HOUR, 3600) or not _hit("new-all", 2000, 86400):
        raise Refused(429, "Too many new accounts from here. Wait an hour and try again.")
    sc = new_guest(ip, ua)
    jar = merge({}, sc)
    acct = game_account(cookie_header(jar), ip, ua)
    info = key_info(cookie_header(jar), ip, ua)
    jar = merge(jar, info.get("set_cookies") or [])
    key = info.get("key", "")
    if not key and info.get("csrf"):
        key, sc2 = mint(cookie_header(jar), info["csrf"], ip, ua)
        sc = sc + sc2
        jar = merge(jar, sc2)
    _set_made_name(acct)
    if store is not None:
        store.note(acct, "account.new")
    return {"cookies": passed_on(sc + (info.get("set_cookies") or [])), "cookie": cookie_header(jar), "acct": acct, "key": key}


def key_signin(key, ip: str, ua: str) -> dict:
    """{cookies, cookie, acct} for a pasted key. Raises Refused or Down. The key itself goes no further than the service."""
    m = KEY_RE.search(re.sub(r"\s+", "", str(key or ""))[:400])
    if not m:
        raise Refused(400, "That doesn't look like a Hero Key. It starts with ovk1. Paste the whole line.")
    if not _hit("ksi|" + _ratelimit().bucket(ip), SIGNINS_PER_15MIN, 900):
        raise Refused(429, "Too many tries. Wait 15 minutes and try again.")
    sc = sign_in_with_key(m.group(0), ip, ua)
    jar = merge({}, sc)
    acct = game_account(cookie_header(jar), ip, ua)
    return {"cookies": passed_on(sc), "cookie": cookie_header(jar), "acct": acct}


def _find_by_name(text) -> str | None:
    soc = _soc()
    m = re.fullmatch(r"\s*([^#\s]{2,24})#(\d{4})\s*", str(text or ""))
    if soc is None or not m:
        return None
    r = soc.one("SELECT acct FROM social_accounts WHERE name_lc = ? AND tag = ?", (m.group(1).lower(), m.group(2)))
    return r[0] if r else None


def _jar_for(how: str, extra: str, acct: str, h) -> tuple[str, bool]:
    """(the website cookie to act with, whether it is a launcher's kept session). Raises Refused when there is none."""
    if how == "cookie":
        return h.headers.get("Cookie", ""), False
    got = store.session(extra, acct) if store is not None else None
    if not got:
        raise Refused(409, "web_only")
    return got[0], True


def _keep(how: str, extra: str, acct: str, jar: str, sc: list[str], own: bool = True) -> None:
    if how == "token" and store is not None and sc:
        store.link(extra, acct, cookie_header(merge(cookie_dict(jar), sc)), own)


def api(h, sub: str, method: str, acct: str | None, how: str, extra: str, doc: dict | None, send) -> None:
    """One call; send(status, doc, cookies) answers. how: "token", "cookie" or "" (signed out)."""
    ip, ua = h.ip(), h.headers.get("User-Agent", "")
    try:
        if sub == "account/new":
            out = new_account(ip, ua)
            tok = _issue(out["acct"], "new account")
            if store is not None:
                store.link(tok, out["acct"], out["cookie"], True)
            return send(200, {"ok": True, "token": tok, "key": out["key"], "me": _me(out["acct"])}, [])
        if sub == "herokey/signin":
            out = key_signin((doc or {}).get("key"), ip, ua)
            tok = _issue(out["acct"], "Hero Key")
            if store is not None:
                store.link(tok, out["acct"], out["cookie"], True)
                store.note(out["acct"], "signin.key", "launcher")
            return send(200, {"ok": True, "token": tok, "me": _me(out["acct"])}, [])
        if sub == "password/signin":
            b = _ratelimit().bucket(ip)
            if not _hit("psi|" + b, SIGNINS_PER_15MIN, 900):
                return send(429, {"error": "Too many tries. Wait 15 minutes and try again."}, [])
            who = _find_by_name((doc or {}).get("name"))
            pw = (doc or {}).get("password")
            if who and not _hit("psa|" + who, SIGNINS_PER_15MIN, 900):
                return send(429, {"error": "Too many tries for that name. Wait 15 minutes and try again."}, [])
            if store is None or not isinstance(pw, str) or not store.password_ok(who or "", pw[:PASS_MAX]) or not who:
                return send(401 if who else 400, {"error": "That name and password don't match. Use your name with its #tag, like Alice#1234."}, [])
            store.note(who, "signin.password", "launcher")
            return send(200, {"ok": True, "token": _issue(who, "password"), "me": _me(who)}, [])
        # signed in from here on
        if sub == "herokey" and method == "GET":
            base = {"has_password": bool(store and store.has_password(acct)), "file_name": FILE_NAME}
            try:
                jar, kept = _jar_for(how, extra, acct, h)
            except Refused:
                return send(200, dict(base, web_only=True, has_key=None, key="", saved=False, made="", last_used=""), [])
            info = key_info(jar, ip, ua)
            if not info["signed_in"]:
                if kept and store is not None:
                    store.forget(extra)
                    return send(200, dict(base, web_only=True, has_key=None, key="", saved=False, made="", last_used=""), [])
                return send(401, {"error": "Sign in first."}, [])
            _keep(how, extra, acct, jar, info["set_cookies"])
            return send(200, dict(base, web_only=False, has_key=info["has_key"], key=info["key"], saved=info["saved"],
                                  made=info["made"], last_used=info["last_used"], hero_id=info["hero_id"]),
                        passed_on(info["set_cookies"]) if how == "cookie" else [])
        if sub == "herokey/new":
            if not _hit("hknew|" + acct, NEW_KEYS_PER_HOUR, 3600):
                return send(429, {"error": "You've made a few keys this hour. Wait a while and try again."}, [])
            jar, kept = _jar_for(how, extra, acct, h)
            info = key_info(jar, ip, ua)
            if not info["signed_in"] or not info.get("csrf"):
                raise Refused(409 if kept else 401, "web_only" if kept else "Sign in first.")
            key, sc = mint(cookie_header(merge(cookie_dict(jar), info["set_cookies"])), info["csrf"], ip, ua)
            _keep(how, extra, acct, jar, info["set_cookies"] + sc, own=kept)
            if store is not None:
                store.note(acct, "key.new", "launcher" if how == "token" else "web")
            return send(200, {"ok": True, "key": key}, passed_on(info["set_cookies"] + sc) if how == "cookie" else [])
        if sub == "herokey/saved":
            jar, kept = _jar_for(how, extra, acct, h)
            info = key_info(jar, ip, ua)
            if not info["signed_in"]:
                raise Refused(401, "Sign in first.")
            sc = mark_saved(cookie_header(merge(cookie_dict(jar), info["set_cookies"])), info.get("csrf", ""), ip, ua)
            _keep(how, extra, acct, jar, info["set_cookies"] + sc, own=kept)
            if store is not None:
                store.note(acct, "key.saved", "launcher" if how == "token" else "web")
            return send(200, {"ok": True}, passed_on(sc) if how == "cookie" else [])
        if sub == "password":
            if store is None:
                return send(404, {"error": "Passwords are off here."}, [])
            if not _hit("pw|" + acct, 10, 3600):
                return send(429, {"error": "Too many changes. Wait a while and try again."}, [])
            if (doc or {}).get("remove") is True:
                store.set_password(acct, None)
                store.note(acct, "password.removed", how)
                return send(200, {"ok": True, "has_password": False}, [])
            pw = (doc or {}).get("password")
            err = password_problem(pw, _me(acct)["name"])
            if err:
                return send(400, {"error": err}, [])
            store.set_password(acct, pw)
            store.note(acct, "password.set", how)
            return send(200, {"ok": True, "has_password": True}, [])
    except Refused as e:
        if e.text == "web_only":
            return send(409, {"error": "Make a new key on the website: open Your Hero Key there.", "web_only": True}, [])
        return send(e.status, {"error": e.text}, [])
    except Down:
        log.warning("herokeys: the sign-in service didn't answer (%s)", sub)
        return send(503, {"error": "The website's sign-in isn't answering right now. Try again shortly."}, [])
    send(404, {"error": "Unknown call."}, [])


def launcher_signed_out(token: str, acct: str, ip: str, ua: str) -> None:
    """The launcher signed out: end the website session it owns (one it borrowed from a browser stays), forget it."""
    if store is None:
        return
    got = store.session(token, acct)
    store.forget(token)
    if got and got[1]:
        try:
            realm_logout(got[0], ip, ua)
        except Exception:   # noqa: BLE001
            pass


def launcher_linked(token: str, acct: str, cookie_header_value: str) -> None:
    """/wrath/link approved a launcher in this browser: lend it the browser's website session, for its Hero Key."""
    if store is None or not token:
        return
    jar = {k: v for k, v in cookie_dict(cookie_header_value).items() if k in KEEP_COOKIES}
    if jar.get("wr_session"):
        store.link(token, acct, cookie_header(jar), False)


# ------------------------------------------------------------------ the page
CSS = """
.hk{max-width:560px;margin:28px auto 48px;padding:0 16px;display:grid;gap:16px}
.hk-card{background:var(--lk-surface,#1a1d25);border:1px solid var(--lk-border,#262a34);border-radius:var(--lk-r,14px);padding:22px}
.hk h1{margin:0 0 4px;font-size:24px;font-weight:750;letter-spacing:-.01em}
.hk h2{margin:0 0 6px;font-size:16px;font-weight:700}
.hk p{margin:6px 0;color:var(--lk-text-2,#c8cdd7)}
.hk-fine{font-size:13.5px;color:var(--lk-muted,#9aa1ae)!important}
.hk-key{display:flex;align-items:center;gap:10px;margin:14px 0 6px;padding:12px 14px;border-radius:var(--lk-r-sm,10px);
background:var(--lk-surface-2,#20242e);border:1px solid var(--lk-border-strong,#343a47)}
.hk-key code{flex:1;min-width:0;font:600 14px/1.4 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;overflow-wrap:anywhere;color:var(--lk-text,#eef0f4)}
.hk-key code.hk-dots{letter-spacing:.18em;color:var(--lk-muted,#9aa1ae)}
.hk-acts{display:flex;flex-wrap:wrap;gap:10px;margin-top:12px}
.hk-btn{display:inline-flex;align-items:center;justify-content:center;gap:8px;min-height:40px;padding:8px 16px;border-radius:var(--lk-r-sm,10px);
border:1px solid var(--lk-border-strong,#343a47);background:var(--lk-raised,#262b37);color:var(--lk-text,#eef0f4);font:inherit;font-weight:650;cursor:pointer;text-decoration:none}
.hk-btn.primary{background:var(--lk-accent,#5b8def);border-color:var(--lk-accent,#5b8def);color:var(--lk-on-accent,#fff)}
.hk-btn.small{min-height:32px;padding:4px 12px;font-size:14px}
.hk-btn:hover{filter:brightness(1.08)}
.hk-msg{margin:0;padding:10px 14px;border-radius:var(--lk-r-sm,10px);background:var(--lk-accent-soft,rgba(91,141,239,.16));border:1px solid var(--lk-accent,#5b8def);color:var(--lk-text,#eef0f4)!important}
.hk-msg.err{background:rgba(255,127,118,.12);border-color:var(--lk-bad,#ff7f76)}
.hk-meta{display:flex;flex-wrap:wrap;gap:4px 16px;margin-top:10px;font-size:14px;color:var(--lk-muted,#9aa1ae)}
.hk label{display:block;margin:12px 0 4px;font-weight:650;font-size:14px}
.hk input[type=text],.hk input[type=password]{width:100%;min-height:40px;padding:8px 12px;border-radius:var(--lk-r-sm,10px);border:1px solid var(--lk-border-strong,#343a47);
background:var(--lk-surface-2,#20242e);color:var(--lk-text,#eef0f4);font:inherit}
.hk details{border-top:1px solid var(--lk-border,#262a34);margin-top:16px;padding-top:10px}
.hk summary{cursor:pointer;font-weight:650;color:var(--lk-text-2,#c8cdd7)}
.hk-or{display:flex;align-items:center;gap:10px;margin:16px 0 4px;color:var(--lk-muted,#9aa1ae);font-size:13px}
.hk-or::before,.hk-or::after{content:"";flex:1;height:1px;background:var(--lk-border,#262a34)}
.hk-status{min-height:1.4em;font-size:14px}.hk-status.ok{color:var(--lk-ok,#4fd18b)}.hk-status.bad{color:var(--lk-bad,#ff7f76)}
.hk-sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)}
@media (max-width:520px){.hk-card{padding:16px}.hk-acts .hk-btn{flex:1 1 auto}}
"""

JS = r"""
(function(){
  var root=document.getElementById('hk'); if(!root) return;
  var csrf=root.getAttribute('data-csrf')||'', who=root.getAttribute('data-name')||'';
  var st=document.getElementById('hk-status');
  function say(t,k){ if(st){ st.textContent=t||''; st.className='hk-status'+(k?' '+k:''); } }
  function call(sub,body){ return fetch('/wrath/api/social/'+sub,{method:'POST',credentials:'same-origin',
    headers:{'Content-Type':'application/json','X-Leyline-CSRF':csrf},body:JSON.stringify(body||{})})
    .then(function(r){ return r.json().catch(function(){return {};}).then(function(d){ if(!r.ok) throw (d&&d.error)||'Something went wrong.'; return d; }); }); }
  var code=document.getElementById('hk-keytext'), key=code?code.getAttribute('data-key'):'';
  var show=document.getElementById('hk-show'), told=false;
  function dots(){ return key ? key.slice(0,5)+'•'.repeat(24) : ''; }
  function draw(on){ if(!code) return; code.textContent=on?key:dots(); code.classList.toggle('hk-dots',!on);
    if(show){ show.textContent=on?'Hide':'Show'; show.setAttribute('aria-pressed',on?'true':'false'); } }
  function saved(){ if(told) return; told=true; call('herokey/saved').catch(function(){ told=false; }); }
  if(code){ draw(false);
    show.addEventListener('click',function(){ draw(code.classList.contains('hk-dots')); });
    document.getElementById('hk-copy').addEventListener('click',function(){
      var ok=function(){ say('Copied. Keep it somewhere safe.','ok'); saved(); };
      if(navigator.clipboard&&navigator.clipboard.writeText) navigator.clipboard.writeText(key).then(ok,function(){ draw(true); say('Select the key and copy it.','bad'); });
      else { draw(true); say('Select the key and copy it.','bad'); } });
    document.getElementById('hk-dl').addEventListener('click',function(){
      var day=new Date().toLocaleDateString('en-GB',{day:'numeric',month:'long',year:'numeric'});
      var text='Leyline Hero Key\r\n\r\nName: '+who+'\r\nKey: '+key+'\r\nMade: '+day+'\r\n\r\nThis key signs you in to your Leyline account on Wrath Unleashed and on every realm that accepts Hero Key. Keep it private, like a password.\r\n';
      var a=document.createElement('a'); a.href=URL.createObjectURL(new Blob([text],{type:'text/plain'}));
      a.download='Leyline Hero Key.txt'; document.body.appendChild(a); a.click(); a.remove();
      setTimeout(function(){ URL.revokeObjectURL(a.href); },5000);
      say('Saved Leyline Hero Key.txt.','ok'); saved(); });
  }
  var nk=document.getElementById('hk-new');
  if(nk) nk.addEventListener('click',function(){
    if(!confirm('Make a new Hero Key? Your old key stops working at once.')) return;
    nk.disabled=true; say('Making your new key…');
    call('herokey/new').then(function(){ location.href='/wrath/key?made=1'; },function(e){ nk.disabled=false; say(String(e),'bad'); }); });
  function form(id,fn){ var f=document.getElementById(id); if(f) f.addEventListener('submit',function(e){ e.preventDefault(); fn(f); }); }
  form('hk-name',function(f){ var n=f.querySelector('input').value.trim(); var s=f.querySelector('.hk-status');
    s.textContent='Saving…'; s.className='hk-status';
    call('profile',{name:n}).then(function(d){ s.textContent='Saved. You are '+d.me.name+'#'+d.me.tag+'.'; s.className='hk-status ok'; who=d.me.name+'#'+d.me.tag; },
      function(e){ s.textContent=String(e); s.className='hk-status bad'; }); });
  form('hk-pass',function(f){ var i=f.querySelectorAll('input'), s=f.querySelector('.hk-status');
    if(i[0].value!==i[1].value){ s.textContent='The two passwords are different.'; s.className='hk-status bad'; return; }
    s.textContent='Saving…'; s.className='hk-status';
    call('password',{password:i[0].value}).then(function(){ i[0].value=i[1].value=''; s.textContent='Password set. Use it with your name and #tag in the launcher.'; s.className='hk-status ok'; },
      function(e){ s.textContent=String(e); s.className='hk-status bad'; }); });
  var hc=document.getElementById('hk-herocopy');
  if(hc) hc.addEventListener('click',function(){ var t=document.getElementById('hk-hero').textContent;
    if(navigator.clipboard) navigator.clipboard.writeText(t).then(function(){ say('Hero ID copied.','ok'); }); });
})();
"""

KEY_ICON = '<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path fill="currentColor" d="M7 14a3 3 0 1 1 0-6 3 3 0 0 1 0 6Zm5.65-4A6 6 0 1 0 12.65 14H15v3h3v-3h2v-4h-7.35Z"/></svg>'


def _send(h, status: int, body: bytes, cookies: list[str], nonce: str = "") -> None:
    if not cookies:
        return h.send(status, body, nonce=nonce, connect=True, headers={"X-Robots-Tag": "noindex, nofollow"})
    h.send_response(status)
    h.send_header("Content-Type", "text/html; charset=utf-8")
    h.send_header("Content-Length", str(len(body)))
    for c in cookies:
        h.send_header("Set-Cookie", c)
    h.send_header("Cache-Control", "no-store")
    h.send_header("X-Content-Type-Options", "nosniff")
    h.send_header("Referrer-Policy", "same-origin")
    h.send_header("X-Robots-Tag", "noindex, nofollow")
    script = f"'nonce-{nonce}'" if nonce else "'none'"
    h.send_header("Content-Security-Policy", f"default-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; "
                                             f"script-src {script}; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'")
    h.end_headers()
    if h.command != "HEAD":
        h.wfile.write(body)


def _go(h, url: str, cookies: list[str] | None = None) -> None:
    h.send_response(303)
    h.send_header("Location", url)
    for c in cookies or []:
        h.send_header("Set-Cookie", c)
    h.send_header("Cache-Control", "no-store")
    h.send_header("Referrer-Policy", "same-origin")
    h.send_header("Content-Length", "0")
    h.end_headers()


def _shell(h, inner: str, status: int = 200, cookies: list[str] | None = None, who=None) -> None:
    nonce = secrets.token_urlsafe(16)
    page = f'<div class="hk">{inner}</div><script nonce="{nonce}">{JS}</script>'
    body = None
    hub = P("hub_mod")
    if hub is not None and getattr(hub, "hub", None) is not None:
        try:
            hub._REQ.who = who
            body = hub.hub._shell("Your Hero Key · Leyline", page, PAGE, nonce=nonce, active="", css=CSS)
        except Exception:   # noqa: BLE001
            log.exception("herokeys: the hub's page failed")
            body = None
    if body is None:
        shell = P("ly_shell")
        try:
            body = shell("Your Hero Key · Leyline", page, PAGE, "", "", CSS, noindex=True) if shell else None
        except Exception:   # noqa: BLE001
            body = None
    if body is None:
        body = (f"<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>"
                f"<title>Your Hero Key · Leyline</title><style>{CSS}</style><main>{page}</main>").encode("utf-8")
    _send(h, status, body, cookies or [], nonce)


def _form_token(sess: str, what: str) -> str:
    soc = _soc()
    k = soc.key if soc is not None else b"herokeys"
    return hmac.new(k, f"herokey|{sess}|{what}".encode(), hashlib.sha256).hexdigest()[:32]


def _anon_token(h) -> str:
    """A form token for the signed-out forms, tied to a cookie we set (ly_hk), so another site can't post them."""
    return cookie_dict(h.headers.get("Cookie", "")).get("ly_hk", "")


def page(h, query: dict, note: str = "", ok: str = "", status: int = 200, cookies: list[str] | None = None) -> None:
    e = html.escape
    mod = P("social")
    cookie = h.headers.get("Cookie", "")
    try:
        who = mod.cookie_user(h) if (mod is not None and "wr_session=" in cookie) else None
    except OSError:
        return _shell(h, '<section class="hk-card"><h1>Your Hero Key</h1><p>The website\'s sign-in isn\'t answering right now. '
                         'Try again shortly.</p></section>', 503)
    msg = (f'<p class="hk-msg err" role="alert">{e(note)}</p>' if note else "") + (f'<p class="hk-msg" role="status">{e(ok)}</p>' if ok else "")
    if not who:
        anon = _anon_token(h)
        set_c = list(cookies or [])
        if not re.fullmatch(r"[A-Za-z0-9_-]{22,43}", anon):
            anon = secrets.token_urlsafe(16)
            set_c.append(f"ly_hk={anon}; Path=/wrath/key; Max-Age=3600; HttpOnly; Secure; SameSite=Strict")
        t1, t2 = _form_token(anon, "start"), _form_token(anon, "signin")
        inner = (f'{msg}<section class="hk-card" aria-labelledby="hk-t"><h1 id="hk-t">Your Hero Key</h1>'
                 '<p>A Hero Key is your sign-in: one line of text you keep. One account works on Wrath Unleashed and on every '
                 'realm that accepts Hero Key.</p>'
                 f'<form method="post" action="{START}" class="hk-acts"><input type="hidden" name="t" value="{t1}">'
                 f'<button class="hk-btn primary" type="submit">{KEY_ICON}Get your Hero Key</button></form>'
                 '<p class="hk-fine">New here? This makes your free account with a name made for you. You can change the name later.</p>'
                 '<p class="hk-or"><span>or</span></p>'
                 f'<form method="post" action="{SIGNIN}"><input type="hidden" name="t" value="{t2}">'
                 '<label for="hk-paste">Sign in with your Hero Key</label>'
                 '<input id="hk-paste" name="key" type="password" autocomplete="current-password" spellcheck="false" placeholder="ovk1." required>'
                 '<div class="hk-acts"><button class="hk-btn" type="submit">Sign in</button>'
                 f'<a class="hk-btn" href="{e(P("signin_url")("/wrath/key") if P("signin_url") else "/wrath/signin")}">Other ways to sign in</a></div></form>'
                 '</section>')
        return _shell(h, inner, status, set_c)
    acct, shown = who
    soc = _soc()
    p = soc.ensure(acct, shown) if soc is not None else {"name": shown, "tag": ""}
    full = f'{p["name"]}#{p["tag"]}' if p.get("tag") else p["name"]
    try:
        info = key_info(cookie, h.ip(), h.headers.get("User-Agent", ""))
    except Down:
        return _shell(h, '<section class="hk-card"><h1>Your Hero Key</h1><p>The website\'s sign-in isn\'t answering right now. '
                         'Try again shortly.</p></section>', 503)
    if not info["signed_in"]:
        return _go(h, PAGE)
    relay = passed_on(info["set_cookies"]) + list(cookies or [])
    sess = cookie_dict(cookie).get("wr_session", "")
    csrf = mod.csrf_for(sess) if mod is not None and getattr(mod, "store", None) is not None else ""
    if info["key"]:
        body = ('<p>Save it now: download it or copy it. For your safety it is shown only this once.</p>'
                f'<div class="hk-key"><code id="hk-keytext" class="hk-dots" data-key="{e(info["key"])}" aria-label="Your Hero Key">'
                f'{e(info["key"][:5])}••••••••••••••••••••••••</code>'
                '<button class="hk-btn small" id="hk-show" type="button" aria-pressed="false" aria-controls="hk-keytext">Show</button></div>'
                '<div class="hk-acts"><button class="hk-btn primary" id="hk-dl" type="button">Download as a text file</button>'
                '<button class="hk-btn" id="hk-copy" type="button">Copy</button></div>')
    else:
        state = ("Your key is saved. Use it to sign in on any device and in the launcher." if info["saved"] else
                 "Your key was made but not saved yet, so it can't be shown again. Make a new one and save it.") if info["has_key"] else \
                "You don't have a key yet."
        meta = "".join(f"<span>{e(a)} <b>{e(b)}</b></span>" for a, b in (("Made", info["made"]), ("Last used", info["last_used"])) if b)
        body = (f'<p>{e(state)}</p>' + (f'<div class="hk-meta">{meta}</div>' if meta else "")
                + '<div class="hk-acts"><button class="hk-btn' + ("" if info["saved"] else " primary")
                + '" id="hk-new" type="button">Make a new key</button></div>'
                '<p class="hk-fine">Lost it, or someone else has it? A new key stops the old one working at once.</p>')
    pw = bool(store and store.has_password(acct))
    more = ('<details><summary>More</summary>'
            '<form id="hk-name"><label for="hk-n">Your name</label>'
            f'<input id="hk-n" type="text" maxlength="24" value="{e(p["name"])}" autocomplete="off" spellcheck="false">'
            '<p class="hk-fine">2 to 24 letters, digits, dots, dashes or underscores. A few changes a day.</p>'
            '<div class="hk-acts"><button class="hk-btn small" type="submit">Save name</button></div><p class="hk-status" role="status"></p></form>'
            + (f'<form id="hk-pass"><label for="hk-p1">{"Change your password" if pw else "Set a password (optional)"}</label>'
               '<input id="hk-p1" type="password" minlength="10" maxlength="128" autocomplete="new-password">'
               '<label for="hk-p2">Type it again</label><input id="hk-p2" type="password" minlength="10" maxlength="128" autocomplete="new-password">'
               '<p class="hk-fine">Signs you in to the launcher with your name and #tag. At least 10 characters. Your Hero Key keeps working.</p>'
               '<div class="hk-acts"><button class="hk-btn small" type="submit">Save password</button></div><p class="hk-status" role="status"></p></form>'
               if store is not None else "")
            + (f'<p style="margin-top:14px"><b>Hero ID</b></p><div class="hk-key"><code id="hk-hero">{e(info["hero_id"])}</code>'
               '<button class="hk-btn small" id="hk-herocopy" type="button">Copy</button></div>'
               '<p class="hk-fine">Safe to share: it proves something is yours without giving away your key.</p>' if info["hero_id"] else "")
            + '</details>')
    inner = (f'{msg}<section class="hk-card" id="hk" data-csrf="{e(csrf)}" data-name="{e(full)}" aria-labelledby="hk-t">'
             f'<h1 id="hk-t">Your Hero Key</h1><p class="hk-fine">{e(full)}</p>{body}'
             '<p class="hk-status" id="hk-status" role="status"></p>'
             '<p class="hk-fine">One account works on Wrath Unleashed and on every realm that accepts Hero Key. Keep the key '
             'private, like a password. We never ask for it.</p>'
             f'{more}</section>')
    _shell(h, inner, status, relay, (acct, p["name"]))


def _read_form(h) -> dict | None:
    try:
        n = int(h.headers.get("Content-Length", "0"))
    except ValueError:
        return None
    if n < 0 or n > FORM_MAX:
        return None
    try:
        q = urllib.parse.parse_qs(h.rfile.read(n).decode("utf-8"), keep_blank_values=True, max_num_fields=6)
    except (UnicodeDecodeError, ValueError):
        return None
    return {k: v[0] for k, v in q.items()}


def _same_site(h) -> bool:
    sfs = h.headers.get("Sec-Fetch-Site", "")
    if sfs == "cross-site":
        return False
    origin = h.headers.get("Origin", "")
    return not origin or origin == f"https://{host()}" or (origin == "null" and sfs == "same-origin")


def match(path: str) -> bool:
    return on() and path in (PAGE, START, SIGNIN)


def get(h, path: str, query: dict) -> None:
    if path != PAGE:
        return h.send(404, b"not found\n", "text/plain")
    lim = P("browse_limit")
    if lim is not None and not lim.hit(_ratelimit().bucket(h.ip()), 600):
        return h.send(429, b"too many requests\n", "text/plain")
    one = lambda k: (query.get(k) or [""])[0]   # noqa: E731
    ok = {"1": "Your new key is below. Save it now."}.get(one("made"), "") or ("Signed in with your Hero Key." if one("back") else "") \
        or ("Welcome! Your account is ready. Save your Hero Key now." if one("new") else "")
    page(h, query, ok=ok)


def post(h, path: str) -> None:
    form = _read_form(h)
    if path not in (START, SIGNIN):
        return h.send(405, b"method not allowed\n", "text/plain")
    if not _same_site(h):
        return h.send(403, b"That form came from another site.\n", "text/plain")
    if form is None:
        return h.send(400, b"That form didn't arrive in one piece.\n", "text/plain")
    anon = _anon_token(h)
    what = "start" if path == START else "signin"
    if not anon or not hmac.compare_digest(form.get("t", ""), _form_token(anon, what)):
        return page(h, {}, "That form timed out. Try again.", status=403)
    ip, ua = h.ip(), h.headers.get("User-Agent", "")
    try:
        if path == START:
            out = new_account(ip, ua)
            return _go(h, PAGE + "?new=1", out["cookies"])
        out = key_signin(form.get("key"), ip, ua)
        if store is not None:
            store.note(out["acct"], "signin.key", "web")
        return _go(h, PAGE + "?back=1", out["cookies"])
    except Refused as e:
        return page(h, {}, e.text, status=e.status if e.status in (400, 401, 403, 429) else 400)
    except Down:
        return page(h, {}, "The website's sign-in isn't answering right now. Try again shortly.", status=503)
