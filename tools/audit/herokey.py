"""Your Hero Key, the whole journey a new player takes, live on the website (stream/herokeys.py, docs/AUDIT.md):

    1. get a Hero Key (a new account: a token, the key, a made-up name that passes the name rules)
    2. its status (the key shown while new; saved; then no longer shown)
    3. sign in with the key (a new token, the same account; a pasted key with spaces and OVK1. too)
    4. sign in on the website's own page (its form token; the session cookie; the signed-in card), sign out there,
       and the stale cookie after it gives the signed-out card (never a redirect loop)
    5. a new key (reset): the old key is refused, the new one works
    6. a new name, read back; a refused name gets plain words (one change a run: the limit is 3 a day)
    7. a password: set, sign in with Name#tag, removed, refused after
    8. sign out: a launcher token revoked stops working
    9. limits stay friendly: a wrong key is a plain refusal, a garbage body a 4xx (never a 5xx)
   10. cleanup: the test account closes itself (audit/close) and its token stops working

One new account a run (account/new {audit: true}), two keys (the one it is born with and one reset): inside the
site's 3 new accounts an hour a network and the service's 10 new keys an hour. Keys, tokens and passwords never go
in a line or the report: the recorder hides them and the lines say "ovk1.ab…" at most. LEYLINE_AUDIT_HEROKEY=0 skips
the section. Standard library only.
"""
from __future__ import annotations

import http.client
import json
import os
import re
import secrets
import ssl
import time
import urllib.error
import urllib.parse
import urllib.request

from common import BANNED, FAIL, PASS, SKIP, UA, NoRedirect, SERVER_ERROR

F = "Hero Key"
API = "/wrath/api/social/"
KEY_RE = re.compile(r"ovk1\.[A-Za-z0-9]{10}\.[A-Za-z0-9_-]{43}")
NAME_RE = re.compile(r"^[A-Za-z0-9_.\-À-ɏ]{2,24}$")     # stream/social.py NAME_RE
DEPLOY = "Run the Control step \"signin\" (Update sign-in and warm-up) so the site has the newest herokeys.py, then run the audit again."
TELL = "Tell Claude this line and logs\\audit-latest.txt."

_OPENER = urllib.request.build_opener(NoRedirect, urllib.request.HTTPSHandler(context=ssl.create_default_context()))


def on() -> bool:
    return os.environ.get("LEYLINE_AUDIT_HEROKEY", "1") != "0"


def mask(secret: str) -> str:
    """What a line may show of a key or token: its first few characters ("ovk1.ab…")."""
    s = str(secret or "")
    return (s[:7] + "…") if s else "(none)"


class Answer:
    def __init__(self, status: int, body: bytes = b"", headers=None, cookies=(), error: str = ""):
        self.status, self.body, self.headers, self.cookies, self.error = status, body, headers or {}, list(cookies), error

    @property
    def text(self) -> str:
        return self.body.decode("utf-8", "replace")

    def json(self) -> dict:
        try:
            d = json.loads(self.body or b"null")
        except ValueError:
            return {}
        return d if isinstance(d, dict) else {}

    def err(self) -> str:
        e = self.json().get("error")
        return e if isinstance(e, str) else ""

    def said(self) -> str:
        """HTTP <status> and the server's own words, short (never a secret: those are hidden by the recorder too)."""
        return f"HTTP {self.status or self.error}" + (f": {self.err()[:120]}" if self.err() else "")


class Site:
    """The website as one browser or one launcher: JSON calls, form posts and a cookie jar. Never raises."""

    def __init__(self, base: str, timeout: float = 30):
        self.base, self.timeout = base.rstrip("/"), timeout

    def call(self, method: str, path: str, js=None, form=None, raw: bytes | None = None, token: str = "",
             jar: dict | None = None, headers=None) -> Answer:
        h = {"User-Agent": UA, "Accept": "application/json, text/html"}
        data = None
        if js is not None:
            data, h["Content-Type"] = json.dumps(js).encode(), "application/json"
        elif form is not None:
            data, h["Content-Type"] = urllib.parse.urlencode(form).encode(), "application/x-www-form-urlencoded"
        elif raw is not None:
            data, h["Content-Type"] = raw, "application/json"
        if data is not None:
            h["Sec-Fetch-Site"] = "same-origin"
        if token:
            h["Authorization"] = "Bearer " + token
        if jar:
            h["Cookie"] = "; ".join(f"{k}={v}" for k, v in jar.items() if v)
        h.update(headers or {})
        req = urllib.request.Request(self.base + path, data=data, method=method, headers=h)
        try:
            r = _OPENER.open(req, timeout=self.timeout)
        except urllib.error.HTTPError as e:
            r = e
        except (urllib.error.URLError, OSError, ValueError, http.client.HTTPException) as e:
            return Answer(0, error=type(e).__name__)
        try:
            body = r.read(1 << 20)
        except (OSError, ValueError, http.client.HTTPException):
            body = b""
        hd = r.headers
        out = Answer(r.status if hasattr(r, "status") else r.code, body,
                     {k.lower(): v for k, v in (hd.items() if hd else [])}, (hd.get_all("Set-Cookie") or []) if hd else [])
        try:
            r.close()
        except Exception:  # noqa: BLE001
            pass
        return out

    def api(self, method: str, sub: str, js=None, token: str = "", **kw) -> Answer:
        if method == "POST" and js is None and "raw" not in kw:
            js = {}
        return self.call(method, API + sub, js=js, token=token, **kw)


def take_cookies(jar: dict, set_cookies) -> dict:
    jar = dict(jar)
    for c in set_cookies:
        first, *attrs = c.split(";")
        k, _, v = first.strip().partition("=")
        if not v or any(a.strip().lower() == "max-age=0" for a in attrs):
            jar.pop(k, None)
        else:
            jar[k] = v
    return jar


def friendly(a: Answer) -> bool:
    """A refusal a player can read: a 4xx, words, no server error text."""
    return 400 <= a.status < 500 and not SERVER_ERROR.search(a.text)


def run(ctx, rec) -> None:
    if not on():
        rec.add(F, "Hero Key checks", SKIP, "LEYLINE_AUDIT_HEROKEY=0")
        return
    j = Journey(ctx, rec)
    try:
        j.go()
    finally:
        j.cleanup()


class Journey:
    def __init__(self, ctx, rec):
        self.ctx, self.rec = ctx, rec
        self.base = ctx.base("site")
        self.site = Site(self.base)
        self.signout_base = (ctx.fake.get("signin") or self.base).rstrip("/")   # the sign-in service's own /logout
        self.token = ""          # the new account's token (its kept website session; the one that closes it)
        self.extra = []          # every other token this run got: revoked before the end
        self.me = {}
        self.key = ""

    def hide(self, *words):
        self.rec.hide(*[w for w in words if w])

    def add(self, check, ok, detail="", action="", status=None, t0=None):
        st = status or (PASS if ok else FAIL)
        self.rec.add(F, check, st, detail, "" if st != FAIL else (action or TELL), (time.time() - t0) if t0 else 0)
        return st == PASS

    def need(self, check, why="needs a Hero Key from the first check"):
        self.rec.add(F, check, SKIP, why)

    # ------------------------------------------------------------------ the journey
    def go(self):
        if not self.new_account():
            for c in ("Status: the key while new, then saved", "Sign in with the key", "Sign in on the website's page",
                      "A new key: the old one stops working", "Change your name", "Password: set, sign in, remove",
                      "Sign out: the token stops working", "A wrong key and a garbage body get plain refusals",
                      "The test account is cleaned up"):
                self.need(c)
            return
        self.status()
        self.key_signin()
        self.page_signin()
        self.reset()
        self.rename()
        self.password()
        self.signout()
        self.limits()

    def new_account(self) -> bool:
        t0 = time.time()
        a = self.site.api("POST", "account/new", {"audit": True})
        d = a.json()
        tok, key, me = str(d.get("token") or ""), str(d.get("key") or ""), d.get("me") if isinstance(d.get("me"), dict) else {}
        self.hide(tok, key)
        self.token = tok    # whatever else came back: a token means an account, and the cleanup closes it
        if a.status == 429:
            self.add("Get a Hero Key", False, "this network made 3 new accounts in the last hour (the site's limit)",
                     status=SKIP)
            return False
        if a.status == 404:
            return self.add("Get a Hero Key", False, "the site has no Hero Key calls (404)", DEPLOY, t0=t0)
        if a.status != 200 or not tok or not KEY_RE.fullmatch(key):
            return self.add("Get a Hero Key", False, a.said(),
                            "The website's sign-in may be down: open the site's Hero Key page. " + TELL, t0=t0)
        self.token, self.key, self.me = tok, key, me
        name = str(me.get("name") or "")
        ok_name = bool(NAME_RE.fullmatch(name)) and not BANNED.search(name) and bool(me.get("tag")) and bool(me.get("pid"))
        self.add("Get a Hero Key", ok_name, f"a key {mask(key)}, a token and the made-up name {name}#{me.get('tag', '')}"
                 if ok_name else f"the name {name!r} or its #tag doesn't pass the name rules",
                 "The made-up names (stream/herokeys.py make_name) broke a name rule. " + TELL, t0=t0)
        return True   # the account is there either way: the rest of the journey (and the cleanup) runs

    def status(self):
        t0 = time.time()
        a = self.site.api("GET", "herokey", token=self.token)
        d = a.json()
        if a.status != 200 or d.get("web_only") or d.get("has_key") is not True or d.get("key") != self.key:
            self.add("Status: the key while new, then saved", False,
                     a.said() if a.status != 200 else f"has_key {d.get('has_key')}, web_only {d.get('web_only')}, "
                     f"the new key {'shown' if d.get('key') == self.key else 'not shown'}", t0=t0)
            return
        s = self.site.api("POST", "herokey/saved", token=self.token)
        b = self.site.api("GET", "herokey", token=self.token).json()
        ok = s.status == 200 and b.get("key") == "" and b.get("saved") is True and b.get("has_key") is True
        self.add("Status: the key while new, then saved", ok, "shown while new, saved, then no longer shown" if ok else
                 f"saved: {s.said()}; after it: key {'still shown' if b.get('key') else 'hidden'}, saved {b.get('saved')}", t0=t0)

    def sign_in(self, key) -> Answer:
        a = self.site.api("POST", "herokey/signin", {"key": key})
        tok = str(a.json().get("token") or "")
        if tok:
            self.hide(tok)
            self.extra.append(tok)
        return a

    def same_me(self, a: Answer) -> bool:
        me = a.json().get("me")
        return isinstance(me, dict) and me.get("pid") == self.me.get("pid") and bool(a.json().get("token"))

    def key_signin(self):
        t0 = time.time()
        a = self.sign_in(self.key)
        if a.status == 429:
            return self.add("Sign in with the key", False, "the sign-in limit for this network is used up (10 in 15 minutes)", status=SKIP)
        self.add("Sign in with the key", a.status == 200 and self.same_me(a),
                 "a new token for the same account" if a.status == 200 else a.said(), t0=t0)

    def page_signin(self):
        t0 = time.time()
        c = "Sign in on the website's page"
        g = self.site.call("GET", "/wrath/key")
        jar = take_cookies({}, g.cookies)
        m = re.search(r'action="/wrath/key/signin"[^>]*><input type="hidden" name="t" value="([0-9a-f]{32})"', g.text)
        if g.status != 200 or not m or "ly_hk" not in jar:
            return self.add(c, False, f"/wrath/key: HTTP {g.status or g.error}, form token {'found' if m else 'missing'}", t0=t0)
        p = self.site.call("POST", "/wrath/key/signin", form={"t": m.group(1), "key": self.key}, jar=jar)
        jar = take_cookies(jar, p.cookies)
        if p.status == 429 or "Too many" in p.text:
            return self.add(c, False, "the sign-in limit for this network is used up (10 in 15 minutes)", status=SKIP)
        if p.status != 303 or not jar.get("wr_session"):
            return self.add(c, False, f"the form: HTTP {p.status or p.error}, {'a' if jar.get('wr_session') else 'no'} session cookie", t0=t0)
        self.hide(jar["wr_session"])
        card = self.site.call("GET", "/wrath/key", jar=jar)
        signed = card.status == 200 and 'id="hk"' in card.text and "data-csrf=" in card.text
        if not self.add(c, signed, "the form signs in and the page shows Your Hero Key, signed in" if signed
                        else f"after signing in: HTTP {card.status}, no signed-in card", t0=t0):
            return
        # sign out on the website, then the old cookie: the signed-out card (never a redirect back to itself)
        t1 = time.time()
        old = dict(jar)
        service = Site(self.signout_base)
        service.call("POST", "/logout", form={}, jar=jar)
        if service.call("GET", "/api/me", jar=old).status not in (401, 403):
            return self.add("Signed out on the website: the old cookie gets the signed-out card", False,
                            "the website's sign-out didn't end this session from here, so there is no stale cookie to try",
                            status=SKIP)
        after = self.site.call("GET", "/wrath/key", jar=old)
        ok = (after.status == 200 and ('id="hk-out"' in after.text or re.search(r"(?:Log|Sign) in with your Hero Key", after.text))
              and not SERVER_ERROR.search(after.text))   # the signed-out card ("Log in" since 2 Nov)
        self.add("Signed out on the website: the old cookie gets the signed-out card", ok,
                 "the signed-out card, no redirect loop" if ok else
                 f"HTTP {after.status}{' to ' + after.headers.get('location', '') if 300 <= after.status < 400 else ''}",
                 DEPLOY if 300 <= after.status < 400 else TELL, t0=t1)

    def reset(self):
        t0 = time.time()
        c = "A new key: the old one stops working"
        a = self.site.api("POST", "herokey/new", token=self.token)
        new = str(a.json().get("key") or "")
        self.hide(new)
        if a.status != 200 or not KEY_RE.fullmatch(new) or new == self.key:
            return self.add(c, False, a.said() if a.status != 200 else "no new key came back", t0=t0)
        old, self.key = self.key, new
        o = self.sign_in(old)
        n = self.sign_in(new)
        if 429 in (o.status, n.status):
            return self.add(c, False, f"a new key {mask(new)}; the sign-in limit for this network is used up", status=SKIP)
        ok = friendly(o) and o.status in (400, 401) and bool(o.err()) and n.status == 200 and self.same_me(n)
        self.add(c, ok, f"a new key {mask(new)}: the old key is refused ({o.status}), the new one signs in" if ok else
                 f"old key: {o.said()}; new key: {n.said()}", t0=t0)
        t1 = time.time()
        p = self.sign_in("  OVK1." + new[5:20] + "\r\n" + new[20:] + "\n")
        if p.status != 429:
            self.add("A pasted key with spaces, a line break and OVK1. still signs in", p.status == 200 and self.same_me(p),
                     "trimmed and read" if p.status == 200 else p.said(), DEPLOY, t0=t1)

    def rename(self):
        t0 = time.time()
        c = "Change your name"
        name = "Auditor" + "".join(secrets.choice("abcdefghjkmnpqrstuvwxyz") for _ in range(5))
        a = self.site.api("POST", "profile", {"name": name}, token=self.token)
        me = a.json().get("me") if isinstance(a.json().get("me"), dict) else {}
        back = self.site.api("GET", "me", token=self.token).json().get("me") or {}
        if a.status == 429:
            return self.add(c, False, "this test account changed its name 3 times today", status=SKIP)
        ok = a.status == 200 and me.get("name") == name and back.get("name") == name
        if ok:
            self.me.update(name=name, tag=back.get("tag") or me.get("tag", ""))
        self.add(c, ok, f"now {name}#{back.get('tag', '')}, read back from me" if ok else
                 f"{a.said()}; me says {back.get('name')!r}", t0=t0)
        t1 = time.time()
        bad = []
        for what, n in (("too short", "x"), ("a company name", "Bliz" + "zard_Fan"), ("a team name", "Admin")):
            r = self.site.api("POST", "profile", {"name": n}, token=self.token)
            words = r.err()
            if not (r.status == 400 and friendly(r) and 5 < len(words) < 200 and not BANNED.search(words)):
                bad.append(f"{what}: HTTP {r.status or r.error}")
        self.add("A refused name gets plain words", not bad, "too short, a game or company name, a team name: each refused, "
                 "plainly" if not bad else "; ".join(bad), t0=t1)

    def password(self):
        t0 = time.time()
        c = "Password: set, sign in, remove"
        pw = secrets.token_urlsafe(12)       # 16 characters
        self.hide(pw)
        full = f"{self.me.get('name', '')}#{self.me.get('tag', '')}"
        s = self.site.api("POST", "password", {"password": pw}, token=self.token)
        if s.status != 200 or s.json().get("has_password") is not True:
            return self.add(c, False, "setting it: " + s.said(), t0=t0)
        i = self.site.api("POST", "password/signin", {"name": full, "password": pw})
        tok = str(i.json().get("token") or "")
        self.hide(tok)
        if tok:
            self.extra.append(tok)
        r = self.site.api("POST", "password", {"remove": True}, token=self.token)
        after = self.site.api("POST", "password/signin", {"name": full, "password": pw})
        if i.status == 429 or after.status == 429:
            return self.add(c, False, "the password sign-in limit is used up (10 in 15 minutes)", status=SKIP)
        ok = i.status == 200 and self.same_me(i) and r.status == 200 and friendly(after) and after.status in (400, 401)
        self.add(c, ok, f"set, {full} and the password sign in, removed, then refused ({after.status})" if ok else
                 f"sign in: {i.said()}; remove: {r.said()}; after removing: {after.said()}", t0=t0)

    def signout(self):
        t0 = time.time()
        c = "Sign out: the token stops working"
        if not self.extra:
            return self.need(c, "no second sign-in to sign out")
        tok = self.extra.pop(0)
        a = self.site.api("POST", "token/revoke", token=tok)
        b = self.site.api("GET", "me", token=tok)
        self.add(c, a.status == 200 and b.status == 401, "revoked, then signed out (401)" if a.status == 200 else a.said(),
                 t0=t0)

    def limits(self):
        t0 = time.time()
        wrong = self.site.api("POST", "herokey/signin", {"key": "ovk1." + "A" * 10 + "." + "B" * 43})
        garbage = self.site.api("POST", "herokey/signin", raw=b"{not json")
        ok = friendly(wrong) and wrong.status in (401, 429) and bool(wrong.err()) and friendly(garbage)
        self.add("A wrong key and a garbage body get plain refusals", ok,
                 f"wrong key {wrong.status} with words, garbage {garbage.status}" if ok else f"wrong key: {wrong.said()}; garbage: {garbage.said()}",
                 t0=t0)

    # ------------------------------------------------------------------ the end: nothing left behind
    def cleanup(self):
        if not self.token:
            return
        t0 = time.time()
        for tok in self.extra:   # every other sign-in this run made (and the website sessions they own)
            self.site.api("POST", "token/revoke", token=tok)
        self.extra = []
        a = self.site.api("POST", "audit/close", token=self.token)
        gone = self.site.api("GET", "me", token=self.token)
        if a.status == 200 and gone.status == 401:
            self.add("The test account is cleaned up", True, "closed (profile, sign-ins, password gone), its token signed out", t0=t0)
        else:
            self.site.api("POST", "token/revoke", token=self.token)   # at least nothing stays signed in
            self.add("The test account is cleaned up", False,
                     f"audit/close: {a.said()}" + ("; the site doesn't have it yet, so this run's test account stays" if a.status == 404 else ""),
                     DEPLOY if a.status == 404 else TELL, t0=t0)
        self.token = ""
