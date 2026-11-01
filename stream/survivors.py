"""Azeroth Survivors on Leyline: its page on this site and the sign-in gate for its own address. For
stream/wrath_portal.py (it hooks this in); standard library only. Owner 2026-11-01: "build Azeroth Survivors".

The game is the upstream open rebuild (https://github.com/OutThisLife/azeroth-survivors-re, MIT or Apache-2.0,
the original game by Aethus), built unmodified at the sha in stream/survivors/upstream.lock.json. It runs in the
browser with no server of its own and reads its game files at /data on its OWN origin (the base is the page's
origin, hard-coded upstream), so it gets its own address (docs/SURVIVORS.md): a Caddy site in front of the sv-play
container (hosting/survivors/), behind the same sign-in gate as the Wrath stream.

    GET /wrath/survivors        the page: what it is, the credit, and Play (only when the address is set;
                                "Coming soon" until then, "Resting" while the game data killswitch is on)
    GET /wrath/survivors/play   signed in: a play session (mode "survivors", no game account, no realm) and a
                                one-time code, then https://<address>/__gate?code=... (the same gate as the
                                stream: it swaps the code for the wu_play cookie on that address and lands on /)
    on the Survivors address, Caddy's forward_auth asks GET /__auth?site=survivors (site_auth below):
                                200 for a live survivors session; /data without one 403, the page a 302 to Play;
                                with the killswitch on, /data and the manifests 410 Gone for everyone.
A Wrath stream session never opens the Survivors address and a Survivors session never opens the Wrath stream
(wrath_portal.stream_session ignores mode "survivors").

The address, read on every request (no restart): WU_SURVIVORS_URL, else the file WU_SURVIVORS_URL_FILE (default
/app/survivors.url = /srv/wrath/portal/survivors.url, written by hosting/survivors/sv-site-up.sh --go and removed
by --off). One line, https://<host>/. Unset or not a plain https host: "Coming soon".
"""
from __future__ import annotations

import html
import os
import re
import urllib.parse

MODE = "survivors"
PATHS = ("/wrath/survivors", "/wrath/survivors/play")
TITLE = "Azeroth Survivors"
ART = "leyline-mode-azeroth-survivors.webp"   # the mode's key art (stream/www-files/art), 483x298
SOURCE = "https://github.com/OutThisLife/azeroth-survivors-re"
CREDIT = "Azeroth Survivors by Aethus. Open rebuild by OutThisLife, MIT or Apache-2.0."
# TODO(owner): the permission statement (what Aethus said about hosting it), in your words. Shown under the credit
# once it is set here; empty = not shown.
PERMISSION = ""
HOST_RE = re.compile(r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$")
DATA_ROUTES = ("/boot-manifest.json", "/world-manifest.json")

portal = lambda name: None   # noqa: E731  (bind() sets it: the portal's globals)


def bind(lookup) -> None:
    global portal

    def get(name):
        try:
            return lookup(name)
        except Exception:   # noqa: BLE001
            return None
    portal = get


def match(path: str) -> bool:
    return path in PATHS


def site_url() -> str:
    """The Survivors address ("https://<host>/"), or "" when it is not set (or not a plain https host)."""
    v = os.environ.get("WU_SURVIVORS_URL", "")
    if not v:
        try:
            with open(os.environ.get("WU_SURVIVORS_URL_FILE", "/app/survivors.url"), encoding="utf-8") as f:
                v = f.read(300)
        except OSError:
            return ""
    v = v.strip()
    m = re.fullmatch(r"https://([^/?#:]+)/?", v)
    if not m or not HOST_RE.fullmatch(m.group(1)) or len(m.group(1)) > 100:
        return ""
    return f"https://{m.group(1)}/"


def site_host() -> str:
    u = site_url()
    return u[len("https://"):-1] if u else ""


def _off() -> bool:
    f = portal("gamedata_off")
    try:
        return bool(f()) if f else False
    except Exception:   # noqa: BLE001
        return False


CSS = """
.sv-top{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1.1fr);gap:18px;align-items:start}
@media (max-width:720px){.sv-top{grid-template-columns:1fr}}
.sv-art{display:block;width:100%;height:auto;border-radius:6px;border:1px solid var(--lk-border,rgba(120,140,255,.3))}
.sv-h{margin:0 0 8px;font-size:clamp(26px,4.5vw,38px);line-height:1.1}
.sv-line{margin:0 0 12px}
.sv-facts{margin:0 0 14px;padding:0;list-style:none;display:grid;gap:6px;font-size:14px}
.sv-facts b{margin-right:6px}
.sv-play{display:flex;flex-wrap:wrap;gap:10px;align-items:center;margin:4px 0 0}
.sv-soon{display:inline-block;font-weight:600;font-size:12px;letter-spacing:.08em;text-transform:uppercase;border:1px solid currentColor;border-radius:999px;padding:6px 12px;opacity:.85}
.sv-credit{margin:18px 0 0;font-size:13px;opacity:.9}
.sv-keys{margin:6px 0 0;padding-left:1.2rem;font-size:14px}
"""


def page_body(url: str, off: bool) -> str:
    e = html.escape
    art = portal("art_url")
    src = art(ART) if callable(art) else "/wrath/art/" + ART
    if off:
        play = ('<p class="sv-play"><span class="sv-soon">Resting</span> The game is taking a short rest. '
                'Please check back soon.</p>')
    elif url:
        play = '<p class="sv-play"><a class="btn primary" href="/wrath/survivors/play">Play</a></p>'
    else:
        play = '<p class="sv-play"><span class="sv-soon">Coming soon</span></p>'
    lic = f' · <a href="{e(url)}licenses/">Licences</a>' if url and not off else ""
    perm = f'<p class="sv-credit">{e(PERMISSION)}</p>' if PERMISSION else ""
    return (f'<style>{CSS}</style>'
            '<div class="sv-top">'
            f'<img class="sv-art" src="{e(src)}" alt="Azeroth Survivors key art: the title over a band of heroes and a '
            'moonlit castle." width="483" height="298">'
            '<div>'
            f'<h1 class="sv-h">{e(TITLE)}</h1>'
            '<p class="sv-line">A survivors-style roguelike. Pick a hero and a zone, then hold out against the waves. '
            'Your spells cast themselves: you move, level up and pick upgrades, and a boss turns up every few minutes.</p>'
            '<ul class="sv-facts"><li><b>Players</b>One, on its own</li>'
            '<li><b>Runs</b>In your browser, with your Leyline sign-in</li>'
            '<li><b>Needs</b>A browser with WebGPU (Chrome or Edge on a computer)</li></ul>'
            f'{play}'
            '<ul class="sv-keys"><li>WASD to move, F for your class\'s movement spell</li>'
            '<li>1, 2 or 3 to pick a card when you level up</li><li>Esc to pause</li></ul>'
            '</div></div>'
            f'<p class="sv-credit">{e(CREDIT)} <a href="{e(SOURCE)}" rel="noopener">Source</a>{lic}</p>'
            f'{perm}')


def page() -> bytes:
    render = portal("render")
    meta_f = portal("card_meta")
    meta = meta_f("/wrath/survivors", f"{TITLE} · Leyline") if callable(meta_f) else ""
    body = page_body(site_url(), _off())
    if callable(render):
        return render(body, wide=True, meta=meta, title=TITLE, current="modes", path="/wrath/survivors")
    return (f'<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" '
            f'content="width=device-width,initial-scale=1"><title>{TITLE}</title></head><body>{body}</body></html>').encode()


def _who(h):
    """The signed-in account's game login name, or None after answering (the sign-in, or an error page)."""
    cookie = h.headers.get("Cookie", "")
    ip, ua = h.ip(), h.headers.get("User-Agent", "")
    signin = portal("signin_url")
    to = signin("/wrath/survivors/play") if callable(signin) else "/login?next=%2Fwrath%2Fsurvivors%2Fplay"
    if "wr_session=" not in cookie:
        h.redirect(to)
        return None
    get = portal("portal_get")
    status, me = get("/api/me", cookie, ip, ua)
    if status in (401, 403) or 300 <= status < 400:
        h.redirect(to)
        return None
    err = portal("error_page")
    if status != 200 or not me:
        h.send(503, err("The website isn't answering right now. Please try again shortly."))
        return None
    status, play = get("/api/play", cookie, ip, ua)
    user = str(play.get("user", "")) if isinstance(play, dict) else ""
    if status != 200 or not re.fullmatch(r"[A-Za-z0-9]{1,16}", user):
        h.send(503, err("Your account isn't ready yet. Please try again in a minute."))
        return None
    return user


def get(h, path: str, query: dict) -> None:
    lim = portal("browse_limit") if path == "/wrath/survivors" else portal("gate_limit")
    bucket = portal("rl_bucket")
    key = bucket(h.ip()) if callable(bucket) else h.ip()
    if lim is not None and not lim.hit(("" if path == "/wrath/survivors" else "sv:") + key,
                                       600 if path == "/wrath/survivors" else 30):
        busy = portal("busy_page")
        h.send(429, busy(back="/wrath/survivors") if callable(busy) else b"busy\n")
        return
    if path == "/wrath/survivors":
        h.send(200, page())
        return
    url = site_url()
    if not url or _off():
        h.redirect("/wrath/survivors")
        return
    user = _who(h)
    if user is None:
        return
    # Its own session name ("SV-" + the login): ending a Wrath stream or own-files play (sessions.end_user) never
    # ends a Survivors run, and the other way round. No password: the game needs no account.
    code = portal("sessions").create("SV-" + user, "", MODE, 0, mode=MODE)
    h.redirect(f"{url}__gate?code={urllib.parse.quote(code)}")


def site_auth(h) -> None:
    """GET /__auth?site=survivors: Caddy's forward_auth on the Survivors address. The original request is in
    X-Forwarded-Uri."""
    uri = h.headers.get("X-Forwarded-Uri", "/")
    bare = uri.split("?", 1)[0]
    data = bare == "/data" or bare.startswith("/data/") or bare in DATA_ROUTES
    if _off() and data:
        h.send(410, portal("GAMEDATA_GONE") or b'{"error":"gamedata_off"}\n', "application/json; charset=utf-8",
               same_origin=True)
        return
    ck = portal("cookie_value")
    sid = ck(h.headers.get("Cookie", ""), "wu_play") if callable(ck) else ""
    sess = portal("sessions").get(sid) if sid and len(sid) < 128 else None
    if sess and sess.get("mode") == MODE and not _off():
        h.send(200, b"", "text/plain")
        return
    if data:
        h.send(403, b"sign in on the Leyline site first\n", "text/plain")
        return
    pub = portal("PUBLIC_HOST") or "play.openvanilla.app"
    h.redirect(f"https://{pub}/wrath/survivors" + ("" if _off() else "/play"))
