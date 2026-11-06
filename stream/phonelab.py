"""Phone Lab: an opt-in remote debug session for a real phone (owner 6 Nov: "connect and tap on my phone with screenshare,
invent a method"; docs/MOBILE-WEB-OVERHAUL.md).

The owner (staff, admin or above) opens /wrath/phonelab on his phone and presses Start, or starts it on the PC and opens
the join link it shows on the phone. A cookie (wu_phonelab=<code>.<ends>, on the site's parent domain so the game page on
play.<site> sees it too, 30 minutes) turns the recorder on: the site's pages get /wrath/phonelab.js put in by the portal's
send(), the game page loads it from wrath-early.js. While it is on (a banner with the code and Stop; off by itself after 30
minutes) every real touch is recorded: where it landed (the element and its words, never what was typed), the event
sequence touchstart -> pointerdown -> touchend -> pointerup -> click with timings and which ones were preventDefault'ed,
scroll and zoom, the visual viewport, orientation, full screen (and every requestFullscreen ask, with whether the browser
counted it as a user gesture), console errors, the page's load timings and the game page's stage timings, plus a small
wireframe picture after a tap (the page's boxes and labels drawn on a canvas; the game's canvas where the browser allows).
Batches are POSTed to /wrath/phonelab/rec (text/plain, so the game page can send them without a preflight): size-capped,
rate-limited, only for a live session, kept 24 hours.

Never recorded: anything typed (no key or input events are listened to; a form field is named only by its kind), no
password, no cookie, no token (long token-like strings are masked on the phone and again here), no query string or hash
(paths only), nothing on the sign-in, key, account and profile pages but the taps' timings and element names.

Reading it: GET /wrath/phonelab/<code>.json, /wrath/phonelab/latest.json, /wrath/phonelab/sessions.json: staff only
(admin.who, admin role or above). The Control step `phonelab` (tools/phonelab) prints the newest session's taps.
"""
from __future__ import annotations

import hashlib
import hmac
import html
import json
import os
import re
import secrets
import threading
import time
import urllib.parse

PATH = "/wrath/phonelab"
JS_PATH = "/wrath/phonelab.js"
REC_PATH = PATH + "/rec"
COOKIE = "wu_phonelab"
SESSION_S = 1800            # a session records for 30 minutes at most
KEEP_S = 86400              # and is kept for a day
MAX_BODY = 160_000          # one batch
MAX_SESSION_BYTES = 6_000_000
MAX_BATCHES = 900
MAX_SHOTS = 150
SHOT_MAX = 64_000           # one picture (a data: URL)
MAX_RECS = 120              # records in one batch
REC_MAX = 6_000             # one record, as JSON
IP_PER_MIN = 90             # batches from one address
SESSION_PER_MIN = 60        # batches into one session
ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"
CODE_RE = re.compile(r"[A-HJ-NP-Z2-9]{6}")
COOKIE_RE = re.compile(r"(?:^|;\s*)" + COOKIE + r"=([A-HJ-NP-Z2-9]{6})\.(\d{9,11})(?:;|$)")
TOKEN_RE = re.compile(r"[A-Za-z0-9+/_=-]{24,}")
SECRET_KEYS = {"value", "password", "pass", "token", "cookie", "cookies", "key", "secret", "authorization"}

_P = None
_lock = threading.Lock()
_ip_hits: dict = {}
_sess_hits: dict = {}


def bind(lookup) -> None:
    """The portal's globals (wrath_portal.py): admin, GAME_ORIGINS, PUBLIC_HOST."""
    global _P
    _P = lookup


def P(name):
    try:
        return _P(name) if _P else None
    except KeyError:
        return None


def data_dir() -> str:
    return os.environ.get("WU_PHONELAB_DIR") or os.path.join(os.environ.get("WU_PORTAL_DATA", "/data"), "phonelab")


def _now() -> float:
    return time.time()


def match(path: str) -> bool:
    return path == PATH or path == JS_PATH or path.startswith(PATH + "/")


# ---- sessions ---------------------------------------------------------------------------------------------------------

def _meta_path(code: str) -> str:
    return os.path.join(data_dir(), code + ".json")


def _log_path(code: str) -> str:
    return os.path.join(data_dir(), code + ".jsonl")


def _write_meta(meta: dict) -> None:
    d = data_dir()
    os.makedirs(d, exist_ok=True)
    tmp = _meta_path(meta["code"]) + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(meta, f)
    os.replace(tmp, _meta_path(meta["code"]))


def meta(code: str) -> dict | None:
    if not CODE_RE.fullmatch(code or ""):
        return None
    try:
        with open(_meta_path(code), encoding="utf-8") as f:
            m = json.load(f)
        return m if isinstance(m, dict) and m.get("code") == code else None
    except (OSError, ValueError):
        return None


def live(m: dict | None) -> bool:
    return bool(m) and not m.get("stopped") and _now() < float(m.get("ends", 0))


def prune() -> int:
    """Sessions older than a day go (both files). Returns how many went."""
    gone = 0
    try:
        names = os.listdir(data_dir())
    except OSError:
        return 0
    for n in names:
        p = os.path.join(data_dir(), n)
        try:
            if _now() - os.path.getmtime(p) > KEEP_S and re.fullmatch(r"[A-HJ-NP-Z2-9]{6}\.jsonl?(\.tmp)?", n):
                os.remove(p)
                gone += n.endswith(".json")
        except OSError:
            pass
    return gone


def start(by: str = "", ua: str = "") -> dict:
    prune()
    with _lock:
        for _ in range(20):
            code = "".join(secrets.choice(ALPHABET) for _ in range(6))
            if meta(code) is None:
                break
        t = _now()
        m = {"code": code, "started": int(t), "ends": int(t + SESSION_S), "stopped": None, "by": str(by)[:64],
             "ua": _clean(ua, 200), "batches": 0, "bytes": 0, "shots": 0, "recs": 0, "last": None}
        _write_meta(m)
    return m


def stop(code: str) -> bool:
    with _lock:
        m = meta(code)
        if not m or m.get("stopped"):
            return False
        m["stopped"] = int(_now())
        _write_meta(m)
    return True


def sessions() -> list:
    prune()
    out = []
    try:
        names = sorted(os.listdir(data_dir()))
    except OSError:
        return out
    for n in names:
        if n.endswith(".json") and CODE_RE.fullmatch(n[:-5]):
            m = meta(n[:-5])
            if m:
                out.append({**m, "live": live(m)})
    out.sort(key=lambda m: m.get("started", 0), reverse=True)
    return out


def read(code: str, shots: bool = True) -> dict | None:
    m = meta(code)
    if not m:
        return None
    batches = []
    try:
        with open(_log_path(code), encoding="utf-8") as f:
            for line in f:
                try:
                    b = json.loads(line)
                except ValueError:
                    continue
                if not shots:
                    b["shots"] = [{"i": s.get("i"), "bytes": len(s.get("data", ""))} for s in b.get("shots", [])]
                batches.append(b)
    except OSError:
        pass
    return {"session": {**m, "live": live(m)}, "batches": batches}


# ---- what a phone sends -----------------------------------------------------------------------------------------------

def _clean(s, n: int = 200) -> str:
    return TOKEN_RE.sub("•••", str(s or ""))[:n]


def _scrub(v, depth: int = 0):
    """A record as kept: plain JSON only, no field named like a secret, long token-like strings masked."""
    if depth > 6:
        return None
    if isinstance(v, dict):
        return {str(k)[:40]: _scrub(x, depth + 1) for k, x in list(v.items())[:60] if str(k).lower() not in SECRET_KEYS}
    if isinstance(v, list):
        return [_scrub(x, depth + 1) for x in v[:80]]
    if isinstance(v, str):
        return _clean(v, 400)
    if isinstance(v, bool) or v is None:
        return v
    if isinstance(v, (int, float)):
        return v if abs(v) < 1e12 else 0
    return None


def clean_batch(doc) -> dict | None:
    """The batch as kept, or None when it isn't one."""
    if not isinstance(doc, dict) or not CODE_RE.fullmatch(str(doc.get("code", ""))):
        return None
    recs = doc.get("recs") if isinstance(doc.get("recs"), list) else []
    kept = []
    for r in recs[:MAX_RECS]:
        r = _scrub(r)
        if isinstance(r, dict) and len(json.dumps(r)) <= REC_MAX:
            kept.append(r)
    shots = []
    for s in (doc.get("shots") if isinstance(doc.get("shots"), list) else [])[:3]:
        if (isinstance(s, dict) and isinstance(s.get("data"), str) and s["data"].startswith("data:image/jpeg;base64,")
                and len(s["data"]) <= SHOT_MAX and re.fullmatch(r"data:image/jpeg;base64,[A-Za-z0-9+/=]+", s["data"])):
            shots.append({"i": int(s.get("i", 0)) if isinstance(s.get("i"), int) else 0, "data": s["data"]})
    seq = doc.get("seq")
    return {"seq": seq if isinstance(seq, int) and 0 <= seq < 10**7 else 0, "page": _clean(doc.get("page"), 200),
            "recs": kept, "shots": shots, "stop": doc.get("stop") is True}


def _hit(table: dict, key: str, limit: int) -> bool:
    t = _now()
    with _lock:
        q = [x for x in table.get(key, []) if t - x < 60]
        if len(q) >= limit:
            table[key] = q
            return False
        q.append(t)
        table[key] = q
        if len(table) > 5000:
            for k in [k for k, v in table.items() if not v or t - v[-1] > 60]:
                table.pop(k, None)
    return True


def record(body: bytes, ip: str) -> tuple[int, str]:
    """One batch from a phone: (status, why)."""
    if len(body) > MAX_BODY:
        return 413, "too big"
    if not _hit(_ip_hits, ip, IP_PER_MIN):
        return 429, "slow down"
    try:
        doc = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        return 400, "not JSON"
    b = clean_batch(doc)
    if b is None:
        return 400, "not a batch"
    code = doc["code"]
    if not _hit(_sess_hits, code, SESSION_PER_MIN):
        return 429, "slow down"
    with _lock:
        m = meta(code)
        if not live(m):
            return 410, "no live session"
        line = json.dumps({**b, "at": int(_now())}, ensure_ascii=False)
        if (m["bytes"] + len(line) > MAX_SESSION_BYTES or m["batches"] >= MAX_BATCHES):
            return 507, "session full"
        if m["shots"] + len(b["shots"]) > MAX_SHOTS:
            b["shots"] = []
            line = json.dumps({**b, "at": int(_now())}, ensure_ascii=False)
        os.makedirs(data_dir(), exist_ok=True)
        with open(_log_path(code), "a", encoding="utf-8") as f:
            f.write(line + "\n")
        m["batches"] += 1
        m["bytes"] += len(line)
        m["shots"] += len(b["shots"])
        m["recs"] += len(b["recs"])
        m["last"] = int(_now())
        if b["stop"]:
            m["stopped"] = int(_now())
        _write_meta(m)
    return 204, "kept"


# ---- the cookie ---------------------------------------------------------------------------------------------------------

def _host(h) -> str:
    return (h.headers.get("Host", "") or "").split(":")[0].strip().lower()


def cookie_domain(host: str) -> str:
    """The site and its game page (play.<site>) share the cookie: the last two labels of the site's name. Nothing for an
    address or a one-label name (tests on 127.0.0.1)."""
    if not host or re.fullmatch(r"[\d.]+|\[?[0-9a-f:]+\]?|localhost", host) or "." not in host:
        return ""
    return ".".join(host.split(".")[-2:])


def _https(h) -> bool:
    return (h.headers.get("X-Forwarded-Proto", "") or "").lower() == "https" or not re.fullmatch(r"127\.0\.0\.1|localhost", _host(h))


def set_cookie(h, m: dict | None) -> str:
    dom = cookie_domain(_host(h))
    attrs = "; Path=/" + (f"; Domain={dom}" if dom else "") + "; SameSite=Lax" + ("; Secure" if _https(h) else "")
    if not m:
        return f"{COOKIE}=; Max-Age=0" + attrs
    return f"{COOKIE}={m['code']}.{m['ends']}; Max-Age={max(0, int(m['ends'] - _now()))}" + attrs


def cookie_code(cookie_header: str) -> str:
    m = COOKIE_RE.search(cookie_header or "")
    return m.group(1) if m and int(m.group(2)) > _now() else ""


def active_code(h) -> str:
    """The live session this browser records into, or ""."""
    code = cookie_code(h.headers.get("Cookie", ""))
    return code if code and live(meta(code)) else ""


def inject(h, body: bytes, ctype: str, nonce: str) -> tuple[bytes, str, bool]:
    """A site page with the recorder in it, while this browser has a live session: (body, nonce, connect). The nonce
    is the page's own, or a new one for a page that had no script; connect: the page may POST to /wrath/phonelab/rec."""
    try:
        if not ctype.startswith("text/html") or b"</body>" not in body or b'id="wu-phonelab-js"' in body or not active_code(h):
            return body, nonce, False
        nonce = nonce or secrets.token_urlsafe(16)
        tag = f'<script id="wu-phonelab-js" src="{JS_PATH}" nonce="{html.escape(nonce)}" defer></script>'.encode()
        i = body.rfind(b"</body>")
        return body[:i] + tag + body[i:], nonce, True
    except Exception:   # noqa: BLE001: never in the way of a page
        return body, nonce, False


# ---- pages ------------------------------------------------------------------------------------------------------------

def _staff(h, path: str, api: bool = False) -> dict | None:
    adm = P("admin")
    if adm is None:
        h.send(503, b"the admin module is not here\n", "text/plain; charset=utf-8")
        return None
    w = adm.who(h, path, api=api)
    if w is None:
        return None
    if adm.RANK.get(w.get("role"), 0) < adm.RANK["admin"]:
        msg = "Phone Lab is for the owner and admins."
        if api:
            h.send(403, (json.dumps({"ok": False, "error": msg}) + "\n").encode(), "application/json; charset=utf-8")
        else:
            h.send(403, _page("Phone Lab", f"<p>{msg}</p>"))
        return None
    return w


def _csrf(w: dict) -> str:
    adm = P("admin")
    return adm.csrf_token(w.get("session", ""), w.get("account", "")) if adm is not None else ""


CSS = """body{margin:0;background:#0f1117;color:#e8e6df;font:16px/1.5 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
main{max-width:640px;margin:0 auto;padding:20px 16px 48px}h1{font-size:24px;margin:0 0 6px}
.code{font:700 34px/1 ui-monospace,Menlo,monospace;letter-spacing:.12em;color:#ffd98a}
.card{margin:16px 0;padding:16px;border:1px solid #2a2f3d;border-radius:12px;background:#151924}
button,.btn{display:inline-flex;align-items:center;justify-content:center;min-height:48px;padding:0 20px;border-radius:10px;border:1px solid #c9a24a;
background:#2a2110;color:#ffd98a;font:600 16px system-ui,sans-serif;text-decoration:none;cursor:pointer}
.btn.ghost{border-color:#3a4050;background:none;color:#e8e6df}.row{display:flex;flex-wrap:wrap;gap:10px;margin-top:12px}
.on{color:#5ef0a0}.fine{font-size:13px;color:#a9a99f}ul{padding-left:20px}code{font-family:ui-monospace,Menlo,monospace}
table{width:100%;border-collapse:collapse;font-size:14px}td{padding:6px 4px;border-top:1px solid #2a2f3d}"""


def _page(title: str, inner: str) -> bytes:
    return (f'<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">'
            f'<meta name="robots" content="noindex"><title>{html.escape(title)}</title><style>{CSS}</style></head>'
            f'<body><main>{inner}</main></body></html>').encode()


def page_body(m: dict | None, token: str, recent: list, staff: bool, join_url: str = "") -> str:
    e = html.escape
    hidden = f'<input type="hidden" name="csrf" value="{e(token)}">' if token else ""
    if live(m):
        left = max(0, int((m["ends"] - _now()) // 60))
        on = (f'<div class="card"><p class="on">Phone Lab is on on this phone.</p><div class="code">{e(m["code"])}</div>'
              f'<p>{left} min left. Use the site and the game as you normally would: every tap is recorded for Claude to read. '
              'A small banner shows while it is on.</p>'
              '<div class="row"><a class="btn" href="/wrath">Open the game</a><a class="btn ghost" href="/wrath/leyline">Open the site</a></div>'
              f'<form method="post" action="{PATH}/stop" class="row">{hidden}<button type="submit">Stop</button></form>'
              + (f'<p class="fine">Or on another phone: open <code>{e(join_url)}</code></p>' if join_url else "") + '</div>')
    else:
        on = ('<div class="card"><p>Start a session on this phone, then use the site and the game normally. It stops by itself after '
              '30 minutes.</p>'
              + (f'<form method="post" action="{PATH}/start" class="row">{hidden}<button type="submit">Start Phone Lab</button></form>'
                 if staff else '<p class="fine">Log in as the owner or an admin to start one, or open a join link.</p>') + '</div>')
    what = ('<div class="card"><b>What it records</b><ul><li>each tap: where, on what, the order of the touch events and how long the '
            'page took to answer</li><li>scrolling, zoom, turning the phone, full screen</li><li>errors on the page, load times</li>'
            '<li>a small sketch of the screen after a tap</li></ul><b>Never</b><ul><li>anything you type, passwords, keys, cookies</li>'
            '<li>anything on the log in, key and account pages but tap timings</li></ul><p class="fine">Kept for 24 hours.</p></div>')
    rows = "".join(f'<tr><td><code>{e(s["code"])}</code></td><td>{time.strftime("%d %b %H:%M", time.gmtime(s["started"]))} UTC</td>'
                   f'<td>{s["recs"]} records, {s["shots"]} sketches</td><td>{"on" if s["live"] else "ended"}</td>'
                   f'<td><a href="{PATH}/{e(s["code"])}.json">JSON</a></td></tr>' for s in recent[:10])
    recent_html = f'<div class="card"><b>Recent sessions</b><table>{rows}</table></div>' if staff and rows else ""
    return f'<h1>Phone Lab</h1><p class="fine">A remote look at taps on a real phone, for fixing the phone version.</p>{on}{what}{recent_html}'


def get(h, path: str, query: dict) -> None:
    if path == JS_PATH:
        body = RECORDER_JS.encode()
        h.send_response(200)
        h.send_header("Content-Type", "text/javascript; charset=utf-8")
        h.send_header("Content-Length", str(len(body)))
        h.send_header("Cache-Control", "no-cache")
        h.send_header("Access-Control-Allow-Origin", "*")
        h.send_header("Cross-Origin-Resource-Policy", "cross-origin")
        h.send_header("X-Content-Type-Options", "nosniff")
        h.end_headers()
        if h.command != "HEAD":
            h.wfile.write(body)
        return
    m = re.fullmatch(PATH + r"/join/([A-HJ-NP-Z2-9]{6})", path)
    if m:   # a join link: this browser records into a live session someone on the staff started
        s = meta(m.group(1))
        if not live(s):
            h.send(410, _page("Phone Lab", '<h1>Phone Lab</h1><p>That session has ended. Start a new one.</p>'))
            return
        h.redirect(PATH, set_cookie(h, s))
        return
    if path == PATH:
        code = active_code(h)   # recording already (Start, or a join link): Stop needs no staff login
        w = _staff(h, path) if not code else None
        if w is None and not code:
            return   # answered: sign in, or staff only
        token = _csrf(w) if w else ""
        s = meta(code) if code else None
        join = f"https://{_host(h)}{PATH}/join/{s['code']}" if s else ""
        h.send(200, _page("Phone Lab", page_body(s, token, sessions() if w else [], bool(w), join)), headers={"X-Robots-Tag": "noindex"})
        return
    m = re.fullmatch(PATH + r"/(latest|sessions|[A-HJ-NP-Z2-9]{6})\.json", path)
    if m:
        if _staff(h, path, api=True) is None:
            return
        shots = (query.get("shots") or ["1"])[0] != "0"
        if m.group(1) == "sessions":
            doc = {"sessions": sessions()}
        else:
            code = m.group(1)
            if code == "latest":
                ss = sessions()
                code = ss[0]["code"] if ss else ""
            doc = read(code, shots) if code else None
            if doc is None:
                h.send(404, b'{"ok":false,"error":"no such session"}\n', "application/json; charset=utf-8")
                return
        h.send(200, (json.dumps(doc, ensure_ascii=False) + "\n").encode(), "application/json; charset=utf-8")
        return
    h.send(404, _page("Phone Lab", "<p>Not here.</p>"))


def _form(h) -> dict:
    try:
        n = int(h.headers.get("Content-Length", "0"))
    except ValueError:
        n = -1
    if n < 0 or n > 4096:
        return {}
    return {k: v[0] for k, v in urllib.parse.parse_qs(h.rfile.read(n).decode("utf-8", "replace")).items()}


def _cors(h) -> dict:
    origin = h.headers.get("Origin", "")
    ok = set(P("GAME_ORIGINS") or ()) | {f"https://{P('PUBLIC_HOST') or ''}"}
    return {"Access-Control-Allow-Origin": origin, "Vary": "Origin"} if origin in ok else {}


def post(h, path: str) -> None:
    if path == REC_PATH:
        try:
            n = int(h.headers.get("Content-Length", "0"))
        except ValueError:
            n = -1
        if n < 0 or n > MAX_BODY:
            h.send(413, b"too big\n", "text/plain; charset=utf-8", headers=_cors(h))
            return
        status, why = record(h.rfile.read(n), h.ip())
        h.send(status, b"" if status == 204 else (why + "\n").encode(), "text/plain; charset=utf-8", headers=_cors(h))
        return
    if path == PATH + "/stop":   # ends this browser's own session (the cookie names it): no staff needed
        _form(h)
        code = cookie_code(h.headers.get("Cookie", ""))
        if code:
            stop(code)
        h.redirect(PATH, set_cookie(h, None))
        return
    if path == PATH + "/start":
        form = _form(h)
        w = _staff(h, path)
        if w is None:
            return
        if not hmac.compare_digest(form.get("csrf", ""), _csrf(w)):
            h.send(403, _page("Phone Lab", "<p>That form is out of date. Go back, reload the page and try again.</p>"))
            return
        m = start(by=w.get("account", ""), ua=h.headers.get("User-Agent", ""))
        h.redirect(PATH, set_cookie(h, m))
        return
    h.send(404, b"not here\n", "text/plain; charset=utf-8")


# ---- the recorder (the phone's side) ------------------------------------------------------------------------------------
# Served at /wrath/phonelab.js; on the site's pages the portal puts it in (inject), on the game page wrath-early.js
# loads it from the site. It reads its session from the cookie and does nothing without a live one.
RECORDER_JS = r"""// Leyline Phone Lab recorder (stream/phonelab.py). Records taps, never typing. Stop: the banner's Stop button.
(() => {
  'use strict';
  if (window.__phonelab) return;
  const m = document.cookie.match(/(?:^|;\s*)wu_phonelab=([A-HJ-NP-Z2-9]{6})\.(\d{9,11})/);
  if (!m) return;
  const code = m[1], ends = +m[2] * 1000;
  if (ends <= Date.now()) return;
  const me = document.currentScript;
  const SITE = me && me.src ? new URL(me.src).origin : location.origin;
  const onGame = SITE !== location.origin;
  const now = () => Math.round(performance.now());
  const TOKEN = /[A-Za-z0-9+\/_=-]{20,}/g;
  const clean = (s, n = 40) => String(s || '').replace(/\s+/g, ' ').trim().slice(0, n).replace(TOKEN, '•••');
  const privatePage = /^\/(wrath\/)?(key|signin|login|account|profile)(\/|$)|^\/__gate/.test(location.pathname);
  const FIELD = 'input,textarea,select,[contenteditable]:not([contenteditable="false"])';
  const field = (el) => (el && el.closest ? el.closest(FIELD) : null);
  const sel = (el) => {
    if (!el || el.nodeType !== 1) return el === window ? 'window' : el === document ? 'document' : '?';
    let s = el.tagName.toLowerCase();
    if (el.id) return s + '#' + clean(el.id, 30);
    const c = typeof el.className === 'string' ? el.className.trim().split(/\s+/).filter(Boolean).slice(0, 2) : [];
    if (c.length) s += '.' + c.join('.');
    const p = el.parentElement;
    if (p && p !== document.body && p !== document.documentElement) {
      const ps = p.id ? '#' + clean(p.id, 30) : (typeof p.className === 'string' && p.className.trim() ? '.' + p.className.trim().split(/\s+/)[0] : p.tagName.toLowerCase());
      s = ps + ' > ' + s;
    }
    return clean(s, 120);
  };
  // the words on what was tapped: a form field only by its kind, never its value; nothing on the private pages
  const label = (el) => {
    if (!el || el.nodeType !== 1) return '';
    const f = field(el);
    if (f) return '[' + f.tagName.toLowerCase() + (f.type ? ' ' + f.type : '') + ']';
    if (privatePage) return '';
    const a = el.closest('a,button,summary,label,[role=button],[role=tab]') || el;
    return clean(a.getAttribute('aria-label') || a.innerText || a.textContent);
  };
  const page = () => location.pathname.slice(0, 200);
  const view = () => {
    const vv = window.visualViewport;
    return { w: innerWidth, h: innerHeight, vv: vv ? [Math.round(vv.width), Math.round(vv.height), +vv.scale.toFixed(2), Math.round(vv.offsetTop)] : null,
      o: (screen.orientation && screen.orientation.type) || (innerWidth > innerHeight ? 'landscape' : 'portrait'),
      fs: !!(document.fullscreenElement || document.webkitFullscreenElement), sy: Math.round(scrollY) };
  };
  const queue = [], shots = [];
  let seq = 0, sent = 0, failed = 0, bytes = 0, shotN = 0, lastShot = 0, stopped = false, errors = 0;
  const push = (r) => { if (stopped) return; r.t = now(); r.p = page(); queue.push(r); if (queue.length >= 40) flush(); };

  // ---- the banner -----------------------------------------------------------------------------------------------------
  const bar = document.createElement('div');
  bar.id = 'wu-phonelab';
  bar.setAttribute('data-nofs', '');
  bar.setAttribute('role', 'status');
  bar.style.cssText = 'position:fixed;z-index:2147483646;left:6px;' + (onGame ? 'top:max(4px,env(safe-area-inset-top));' : 'bottom:calc(72px + env(safe-area-inset-bottom));')
    + 'display:flex;align-items:center;gap:8px;padding:4px 4px 4px 10px;border-radius:999px;background:rgba(60,8,8,.88);border:1px solid #ff6b5e;'
    + 'color:#ffe1dc;font:600 12px/1 system-ui,sans-serif;box-shadow:0 2px 10px rgba(0,0,0,.5);pointer-events:auto';
  const left = () => Math.max(0, Math.round((ends - Date.now()) / 60000));
  bar.innerHTML = '<span aria-hidden="true" style="width:8px;height:8px;border-radius:50%;background:#ff3b30"></span><span class="t"></span>'
    + '<button type="button" style="min-height:32px;padding:0 12px;border-radius:999px;border:1px solid #ff9c92;background:#2a0505;color:#fff;font:600 12px system-ui,sans-serif">Stop</button>';
  const tick = () => { bar.querySelector('.t').textContent = 'Phone Lab ' + code + ' · ' + left() + ' min'; };
  tick();
  const mount = () => { if (!bar.isConnected && document.body) document.body.appendChild(bar); };
  if (document.body) mount(); else addEventListener('DOMContentLoaded', mount);

  // ---- a tap, from the first touch to 700 ms after the lift -------------------------------------------------------------
  let tap = null, endTimer = 0, muts = 0;
  const mo = new MutationObserver((l) => { muts += l.length; if (tap && !tap.resp) tap.resp = Math.round(performance.now() - tap.t0); });
  const watch = () => mo.observe(document.documentElement, { subtree: true, childList: true, attributes: true, attributeFilter: ['class', 'style', 'hidden', 'open'] });
  if (document.documentElement) watch();
  const TYPES = ['touchstart', 'touchend', 'touchcancel', 'pointerdown', 'pointerup', 'pointercancel', 'click', 'contextmenu', 'dblclick'];
  function endTap(cut) {
    clearTimeout(endTimer);
    if (!tap) return;
    const t = tap;
    tap = null;
    t.after = view();
    if (cut) t.cut = cut;
    delete t.t0;
    t.fsAsk = t.fsAsk || null;
    const pic = cut ? null : sketch(t);
    if (pic) { t.shot = shotN; shots.push({ i: shotN, data: pic }); shotN++; }
    push(t);
  }
  const first = (e) => {
    if (stopped || !e.isTrusted || bar.contains(e.target)) return;
    const pt = e.changedTouches ? e.changedTouches[0] : e;
    const starts = e.type === 'touchstart' || (e.type === 'pointerdown' && e.pointerType !== 'mouse');
    if (starts && (!tap || tap.lifted)) {
      if (tap) endTap();
      muts = 0;
      tap = { kind: 'tap', t0: performance.now(), x: Math.round(pt.clientX), y: Math.round(pt.clientY), on: sel(e.target), text: label(e.target),
        seq: [], moved: 0, fingers: e.touches ? e.touches.length : 1, before: view() };
    }
    if (!tap) {
      if (e.type === 'click') push({ kind: 'click', on: sel(e.target), text: label(e.target), x: Math.round(e.clientX), y: Math.round(e.clientY), pt: e.pointerType || '' });
      return;
    }
    tap.seq.push([e.type, Math.round(performance.now() - tap.t0), 0]);
    if (e.type === 'click') tap.click = sel(e.target);
    if (/^(touchend|pointerup|touchcancel|pointercancel)$/.test(e.type)) {
      tap.lifted = true;
      clearTimeout(endTimer);
      endTimer = setTimeout(endTap, 700);
    }
  };
  const last = (e) => {
    if (!tap || !e.defaultPrevented) return;
    for (let i = tap.seq.length - 1; i >= 0; i--) if (tap.seq[i][0] === e.type) { tap.seq[i][2] = 1; break; }
  };
  for (const t of TYPES) { addEventListener(t, first, { capture: true, passive: true }); addEventListener(t, last, { passive: true }); }
  addEventListener('touchmove', (e) => {
    if (!tap || !e.touches || !e.touches[0]) return;
    const d = Math.hypot(e.touches[0].clientX - tap.x, e.touches[0].clientY - tap.y);
    if (d > tap.moved) tap.moved = Math.round(d);
  }, { capture: true, passive: true });

  // full screen asks: from which event, and did the browser count it as a gesture
  for (const name of ['requestFullscreen', 'webkitRequestFullscreen']) {
    const orig = Element.prototype[name];
    if (typeof orig !== 'function') continue;
    Element.prototype[name] = function () {
      const ev = window.event;
      const ask = { kind: 'fs-ask', from: ev ? ev.type : '', trusted: ev ? ev.isTrusted : null, gesture: navigator.userActivation ? navigator.userActivation.isActive : null };
      if (tap) tap.fsAsk = ask.from + (ask.gesture ? '+gesture' : '-no-gesture');
      let r;
      try { r = orig.apply(this, arguments); } catch (err) { ask.error = clean(err && err.message, 120); push(ask); throw err; }
      Promise.resolve(r).then(() => { ask.ok = true; push(ask); }, (err) => { ask.error = clean(err && err.message, 120); push(ask); });
      return r;
    };
  }

  // ---- the rest of what a phone does --------------------------------------------------------------------------------------
  let scrollAt = 0;
  addEventListener('scroll', () => { const t = Date.now(); if (t - scrollAt > 1000) { scrollAt = t; push({ kind: 'scroll', y: Math.round(scrollY) }); } }, { passive: true, capture: true });
  const vchange = (why) => push({ kind: 'view', why, ...view() });
  addEventListener('resize', () => vchange('resize'));
  if (window.visualViewport) visualViewport.addEventListener('resize', () => vchange('zoom'));
  addEventListener('orientationchange', () => vchange('turn'));
  for (const ev of ['fullscreenchange', 'webkitfullscreenchange']) document.addEventListener(ev, () => push({ kind: 'fs', on: view().fs }));
  document.addEventListener('visibilitychange', () => push({ kind: 'visible', on: !document.hidden }));
  addEventListener('error', (e) => { if (++errors <= 50) push({ kind: 'error', msg: clean(e.message, 200), at: clean(String(e.filename || '').split('/').pop(), 60) + ':' + (e.lineno || 0) }); });
  addEventListener('unhandledrejection', (e) => { if (++errors <= 50) push({ kind: 'error', msg: clean(e.reason && (e.reason.message || e.reason), 200), at: 'promise' }); });
  const cerr = console.error;
  console.error = function () {
    try { if (++errors <= 50) push({ kind: 'console', msg: clean(Array.from(arguments).map((a) => (a && a.message) || String(a)).join(' '), 200) }); } catch (_) { /* never in the way */ }
    return cerr.apply(console, arguments);
  };
  const timings = () => {
    const n = performance.getEntriesByType('navigation')[0] || {};
    const paint = {};
    for (const p of performance.getEntriesByType('paint')) paint[p.name] = Math.round(p.startTime);
    return { fcp: paint['first-contentful-paint'] || null, dcl: Math.round(n.domContentLoadedEventEnd || 0), load: Math.round(n.loadEventEnd || 0) };
  };
  push({ kind: 'page', ua: clean(navigator.userAgent, 160).replace(/•••/g, ''), dpr: devicePixelRatio, screen: [screen.width, screen.height],
    touch: navigator.maxTouchPoints || 0, standalone: matchMedia('(display-mode: standalone), (display-mode: fullscreen)').matches, game: onGame, ...view() });
  addEventListener('load', () => setTimeout(() => push({ kind: 'timing', ...timings() }), 50));
  // the game page: its stage timings (stages.js) and its sign-in cover's state (wrath-page.js), now and then
  const game = () => {
    const st = window.owStages, wp = window.wuPage && window.wuPage.status;
    if (!st && !wp && !window.__wuFs) return;
    const marks = st && st.marks ? (Array.isArray(st.marks) ? st.marks.slice(0, 40) : Object.entries(st.marks).slice(0, 40)) : null;
    push({ kind: 'game', marks: marks ? JSON.parse(JSON.stringify(marks)) : null, signin: wp ? clean(wp.signin, 60) : null,
      fsFirst: window.__wuFs ? { asked: window.__wuFs.asked, refused: window.__wuFs.refused, last: clean(window.__wuFs.last, 40) } : null });
  };
  const gameTimer = setInterval(game, 15000);

  // ---- a sketch of the screen: the page's boxes and labels on a small canvas (no pictures of what was typed) ----------------
  function sketch(t) {
    if (privatePage || shotN >= 150 || Date.now() - lastShot < 1500 || !document.body) return null;
    lastShot = Date.now();
    try {
      const W = 200, sc = W / innerWidth, H = Math.round(innerHeight * sc);
      const c = document.createElement('canvas');
      c.width = W; c.height = H;
      const g = c.getContext('2d');
      const bodyBg = getComputedStyle(document.body).backgroundColor;
      g.fillStyle = /rgba\(.*,\s*0\)|transparent/.test(bodyBg) ? '#111' : bodyBg;
      g.fillRect(0, 0, W, H);
      let n = 0;
      for (const el of document.body.getElementsByTagName('*')) {
        if (++n > 2500) break;
        if (el === bar || bar.contains(el)) continue;
        const r = el.getBoundingClientRect();
        if (r.width < 2 || r.height < 2 || r.bottom < 0 || r.top > innerHeight || r.right < 0 || r.left > innerWidth) continue;
        const cs = getComputedStyle(el);
        if (cs.visibility === 'hidden' || cs.display === 'none' || +cs.opacity === 0) continue;
        const x = r.left * sc, y = r.top * sc, w = r.width * sc, h = r.height * sc;
        if (el.tagName === 'CANVAS') { try { g.drawImage(el, x, y, w, h); } catch (_) { g.fillStyle = '#333'; g.fillRect(x, y, w, h); } continue; }
        if (/^(IMG|VIDEO|svg|PICTURE)$/.test(el.tagName)) { g.fillStyle = 'rgba(120,120,150,.45)'; g.fillRect(x, y, w, h); continue; }
        const bg = cs.backgroundColor;
        if (bg && !/rgba\(.*,\s*0\)|transparent/.test(bg)) { g.fillStyle = bg; g.fillRect(x, y, w, h); }
        if (field(el)) { g.strokeStyle = '#888'; g.strokeRect(x, y, w, h); continue; }
        if (parseFloat(cs.borderTopWidth) > 0) { g.strokeStyle = cs.borderTopColor; g.strokeRect(x, y, w, h); }
        if (el.childElementCount === 0 && /^(A|BUTTON|H1|H2|H3|H4|LABEL|SUMMARY|SPAN|P|LI|B|STRONG|DIV)$/.test(el.tagName)) {
          const s = clean(el.textContent, 28);
          if (s) { g.fillStyle = cs.color; g.font = Math.max(5, Math.min(10, parseFloat(cs.fontSize) * sc)) + 'px sans-serif'; g.fillText(s, x + 1, y + Math.min(h, 8)); }
        }
      }
      g.strokeStyle = '#ff3b30'; g.lineWidth = 2; g.beginPath(); g.arc(t.x * sc, t.y * sc, 6, 0, 7); g.stroke();
      let d = c.toDataURL('image/jpeg', 0.6);
      if (d.length > 60000) d = c.toDataURL('image/jpeg', 0.35);
      return d.length <= 60000 && d.startsWith('data:image/jpeg') ? d : null;
    } catch (_) { return null; }
  }

  // ---- sending ------------------------------------------------------------------------------------------------------------
  const URL_REC = SITE + '/wrath/phonelab/rec';
  function flush(final) {
    if (!queue.length && !shots.length && !final) return;
    const recs = queue.splice(0, 100);
    let pics = shots.splice(0, 3);
    let body = JSON.stringify({ code, seq: ++seq, page: page(), recs, shots: pics, stop: !!(final && stopped) });
    if (body.length > 150000) { pics = []; body = JSON.stringify({ code, seq, page: page(), recs, shots: [], stop: !!(final && stopped) }); }
    if (body.length > 150000) return;
    bytes += body.length;
    if (final && navigator.sendBeacon) { try { if (navigator.sendBeacon(URL_REC, new Blob([body], { type: 'text/plain' }))) { sent++; return; } } catch (_) { /* fetch then */ } }
    fetch(URL_REC, { method: 'POST', body, mode: onGame ? 'no-cors' : 'same-origin', credentials: 'omit', keepalive: body.length < 60000,
      headers: { 'Content-Type': 'text/plain' } }).then((r) => { if (r.type === 'opaque' || r.ok) sent++; else failed++; if (r.status === 410) off(false); }, () => { failed++; });
  }
  const flushTimer = setInterval(() => flush(false), 3000);
  addEventListener('pagehide', () => { endTap('left the page'); game(); flush(true); });
  const domain = () => { const h = location.hostname; return /^[\d.]+$|^\[|^localhost$/.test(h) || h.indexOf('.') < 0 ? '' : h.split('.').slice(-2).join('.'); };
  let tickTimer = 0;
  function off(send) {
    if (stopped) return;
    if (tap) endTap('stopped');
    game();
    if (send) queue.push({ kind: 'stop', t: now(), p: page() });
    stopped = true;
    clearInterval(flushTimer); clearInterval(gameTimer); clearInterval(tickTimer); mo.disconnect();
    if (send) flush(true);
    const d = domain();
    document.cookie = 'wu_phonelab=; Max-Age=0; Path=/' + (d ? '; Domain=' + d : '');
    document.cookie = 'wu_phonelab=; Max-Age=0; Path=/';
    bar.remove();
  }
  bar.querySelector('button').addEventListener('click', (e) => { e.preventDefault(); e.stopPropagation(); off(true); });
  tickTimer = setInterval(() => { tick(); if (Date.now() >= ends) off(true); }, 20000);
  setTimeout(() => off(true), Math.max(0, ends - Date.now()));
  window.__phonelab = { code, ends, label, sel, flush: () => flush(false), stop: () => off(true), sketch,
    state: () => ({ seq, sent, failed, bytes, shots: shotN, queued: queue.length, stopped, privatePage }) };
})();
"""
