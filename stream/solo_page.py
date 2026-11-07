"""Leyline Solo: the release page, for stream/wrath_portal.py (it hooks this in; bind() gives it the portal's helpers).
Plan Phase 5: the website side of the standalone single player app for the first-era game.

    GET /wrath/solo          what it is, who it is for, the three steps (install, point at your game folder, Play), your
                             companions (.bot group and the party orders, in plain words), Invite a friend, Characters,
                             the honest notes, the questions people ask, the download (GitHub releases of the
                             leyline-solo repo) and its source, and the legal line.

The download shows only once the owner turns it on: WU_SOLO_RELEASED=1 in the portal's environment (read on every
request: no restart). Until then the page says "Coming soon" where the download goes, and it names no repo address
(the repo is made at release, so a link to it would not open yet).

Pictures: the owner's own screenshots go in stream/www-files/art/slots/solo-*.webp (served at /wrath/art/slots/,
wrath_portal.py SLOT_ART_RE; off with the game data killswitch like every slot). Until he adds them each is a framed
panel with its caption (and a TODO(owner) comment naming the file). The hero uses the Leyline emblem (our own art).
Nothing here is a picture taken from the game's files.

Not in the main nav yet (TODO(owner): say when): linked from Single player (stream/site_pages.py) and from the docs
(docs/leyline/solo.md, stream/docs.py). Drawn with site_pages.py's look (lp-*) in the site's frame. Standard library
only; nothing here raises to the caller. Words follow docs/WORDS.md: "Log in", no game brand names (the one legal line
names the company), no client version numbers in sentences ("the first-era client").
TODO(owner): the lines marked below, in your own words.
"""
from __future__ import annotations

import html
import os

portal = lambda name: None   # noqa: E731  (bind() sets it: the portal's globals)


def bind(lookup) -> None:
    global portal

    def get(name):
        try:
            return lookup(name)
        except Exception:   # noqa: BLE001
            return None
    portal = get


PATH = "/wrath/solo"
REPO = "https://github.com/orionopenwow/leyline-solo"
DOWNLOAD = REPO + "/releases/latest"
RELEASED_ENV = "WU_SOLO_RELEASED"   # unset (the default): "Coming soon"; 1: the download and the source code links
GUIDE = "/wrath/docs/solo"
LEGAL = "Leyline is an independent fan project, not affiliated with or endorsed by Blizzard Entertainment."


def released() -> bool:
    return os.environ.get(RELEASED_ENV, "").strip().lower() in ("1", "true", "yes", "on")


def _e(s) -> str:
    return html.escape(str(s))


def _discord() -> str:
    d = portal("DISCORD_URL")
    return d if isinstance(d, str) and d.startswith("https://") else "https://discord.gg/RfChVr8J7T"


def _sp():
    return portal("site_pages")


# ------------------------------------------------------------------------------------------------- pictures
# (slot file, alt, caption, w, h): the owner's own screenshots, TODO(owner)
SHOTS = [
    ("slots/solo-app.webp", "The Leyline Solo window: your game folder, the Play button and Invite a friend",
     "The Leyline Solo window", 1280, 800),
    ("slots/solo-characters.webp", "Characters: a list of your heroes with their level and class, and Play",
     "Characters", 1280, 800),
    ("slots/solo-party.webp", "A player with a party of four companions in the open world", "You and your companions",
     1280, 720),
    ("slots/solo-invite.webp", "Invite a friend: the link for one friend, and the checks before you send it",
     "Invite a friend", 1280, 800),
]
SHOT = {n: (alt, cap, w, h) for n, alt, cap, w, h in SHOTS}
HERO_ART = "leyline-emblem.webp"


def _have(name: str) -> bool:
    f = portal("art_present")
    try:
        return bool(f(name)) if callable(f) else False
    except Exception:   # noqa: BLE001
        return False


def _off() -> bool:
    f = portal("gamedata_off")
    try:
        return bool(f()) if callable(f) else False
    except Exception:   # noqa: BLE001
        return False


def pic(name: str, cls: str = "") -> str:
    """The owner's screenshot when it is there (and the killswitch is off), else a framed panel with its caption."""
    alt, cap, w, h = SHOT[name]
    if _have(name) and not _off():
        return (f'<figure class="lp-pic so-shot {cls}"><img src="/wrath/art/{_e(name)}" alt="{_e(alt)}" width="{w}" '
                f'height="{h}" loading="lazy" decoding="async"><figcaption>{_e(cap)}</figcaption></figure>')
    return (f'<figure class="lp-pic lp-empty so-shot {cls}" data-slot="{_e(name)}">'
            f'<!-- TODO(owner): your own screenshot here, stream/www-files/art/{_e(name)} ({w}x{h} WebP) -->'
            f'<div class="lp-ph" style="aspect-ratio:{w}/{h}" role="img" aria-label="{_e(cap)}: picture coming soon">'
            '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="5" width="18" height="14" rx="2"/>'
            '<circle cx="9" cy="10" r="1.6"/><path d="M4 17l5-5 4 4 3-3 4 4"/></svg><span>Picture coming soon</span>'
            f'</div><figcaption>{_e(cap)}</figcaption></figure>')


def hero_art() -> str:
    if _have(HERO_ART):
        return (f'<figure class="so-emblem"><img src="/wrath/art/{HERO_ART}" alt="The Leyline emblem" width="512" '
                'height="512" decoding="async"></figure>')
    return ""


# ------------------------------------------------------------------------------------------------- words
# TODO(owner): the heading and the line under it, in your words (drafts)
HEAD = "The first era, on your own PC"
LEDE = ("Leyline Solo is a small app that turns your own first-era game folder into a world of your own: a server on "
        "your PC, a party of companions at your side, no account to make and no internet needed once it is set up. "
        "It logs you in by itself, keeps your characters in one list, and lets a friend join when you want company.")

WHO = [
    ("You like the first era and play alone", "Level, quest and run dungeons at your own pace, with companions who "
     "tank, heal and fight beside you."),
    ("You want no server work", "Nothing to configure, no database to set up, no realm list to edit. Install, point "
     "it at your game folder, press Play."),
    ("You play with one or two friends", "Invite a friend into your world: on the same network they come straight in, "
     "anywhere else through the Leyline relay."),
    ("You want it offline", "Once set up, your world runs on your PC with no internet. It goes nowhere and nobody else "
     "can find it."),
]

COMMANDS = [
    (".bot group", "Four companions join your party at your level: a tank, a healer and two who deal damage, picked to "
     "suit you. Add size=3 for a smaller party."),
    (".bot create class=priest", "One companion of the class you name. You can add a race, a level or a name."),
    (".bot add Name", "Brings one of your own characters along as a companion, with its gear and talents."),
    (".bot remove Name", "Sends that companion away. .bot list shows who is with you."),
    (".bot gear Name", "Gives that companion the best gear for its level. .bot prepare adds food and potions."),
]
ORDERS = ("follow", "stay", "attack", "flee", "summon", "revive")

NOTES = [
    ("You need your own game folder", "Leyline Solo plays on your own first-era client, the one the game first "
     "came out with. It never downloads, ships or uploads any of the game: your world is made from your folder, on "
     "your PC."),
    ("Windows first", "The first release is for Windows PCs. Other computers may come later."),
    ("Setup takes a while, once", "The first time, Leyline Solo reads the maps from your game folder and fills the "
     "world. It can take up to an hour. After that, Play takes a minute or so."),
    ("Room on your disk", "About 8 GB free for the world's maps and database."),
    ("Your antivirus may ask", "The world runs as two small server programs. A new program can be flagged or "
     "slowed down by an antivirus the first time; the guide says what to do."),
    ("It is young", "Companions are clever but not perfect, and some quests or dungeons may not work as they should "
     "yet. Tell us on the Discord and we fix what we can."),
]

# TODO(owner): your words, where you want them
FAQ = [
    ("Is it free?", "Yes. No shop, nothing to buy, no account to make."),
    ("Does it change my game folder?",
     "Quick sign-in (the default) adds one small archive of ours and sets two of the game's settings files so the "
     "game logs in to your world by itself. It backs them up first. Settings › Remove sign-in puts your folder back "
     "exactly as it was. Simple sign-in changes only the two settings files."),
    ("Is this the Leyline launcher?",
     "No, it is its own app, just for the first era on your PC. The Leyline launcher is for our realms, Your Den and "
     "the rest of the network. You can have both."),
    ("Where are my characters?",
     "On your PC, in Leyline Solo's own folder. Nothing is kept on our servers. Stopping your world saves everyone."),
    ("Do companions take my loot?", "Loot follows the game's own group rules. Set loot to Free for All in the game "
     "if you want everything yourself."),
    ("Can I play on a Mac or on Linux?", "Not yet. The first release is for Windows."),
    ("Why does my antivirus warn me?",
     "The world's server programs are new to it and talk on your PC's own network. They are built in the open on "
     "GitHub from public source code, and they only listen on your own PC unless you invite a friend."),
    ("Where does the server come from?",
     "It is an open source server for the first era with its companion module, built from source by GitHub's own "
     "machines. The licences come with the app."),
]


# ------------------------------------------------------------------------------------------------- the page
CSS = """
.so .so-hero{grid-template-columns:minmax(0,1.25fr) minmax(0,.75fr)}
.so .so-emblem{margin:0;justify-self:center;max-width:300px}
.so .so-emblem img{display:block;width:100%;height:auto;filter:drop-shadow(0 12px 36px rgba(110,150,255,.28))}
@media (max-width:820px){.so .so-hero{grid-template-columns:1fr}.so .so-emblem{max-width:170px;order:-1}}
.so .so-toc{display:flex;flex-wrap:wrap;gap:8px;margin:18px 0 0;padding:0;list-style:none}
.so .so-toc a{display:inline-flex;align-items:center;min-height:40px;padding:6px 14px;border:1px solid var(--lp-line);border-radius:999px;text-decoration:none;color:var(--lp-ink)}
.so .so-toc a:hover,.so .so-toc a:focus-visible{border-color:var(--lp-acc)}
@media (max-width:560px){.so .so-toc a{min-height:36px;padding:4px 12px;font-size:14px}}
.so .lp-btn{min-height:44px}
.so .so-soon{display:inline-flex;align-items:center;gap:8px;min-height:44px;padding:10px 18px;border-radius:var(--lp-r);border:1px dashed var(--lp-line);color:var(--lp-dim);font-weight:650;cursor:default}
.so .so-band{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:clamp(14px,3vw,28px);align-items:center}
@media (max-width:820px){.so .so-band{grid-template-columns:1fr}}
.so .so-cmds{display:grid;gap:10px;margin:14px 0 0;padding:0;list-style:none}
.so .so-cmds li{display:grid;grid-template-columns:minmax(0,13em) minmax(0,1fr);gap:12px;align-items:baseline;background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:10px 14px}
@media (max-width:560px){.so .so-cmds li{grid-template-columns:1fr;gap:4px}}
.so code{font:600 14px/1.4 ui-monospace,SFMono-Regular,Consolas,monospace;background:rgba(255,255,255,.06);padding:2px 7px;border-radius:6px;overflow-wrap:anywhere}
.so .so-orders{display:flex;flex-wrap:wrap;gap:6px;margin:10px 0 0;padding:0;list-style:none}
.so .so-dl{display:flex;flex-direction:column;gap:8px;background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:18px}
.so .so-dl .lp-acts{margin-top:4px}
.so .so-gal{display:grid;grid-template-columns:repeat(auto-fill,minmax(min(260px,100%),1fr));gap:16px;align-items:start}
"""


def _btn(href: str, label: str, ghost: bool = False) -> str:
    rel = ' rel="noopener noreferrer"' if href.startswith("https://") else ""
    return f'<a class="lp-btn{" ghost" if ghost else ""}" href="{_e(href)}"{rel}>{_e(label)}</a>'


def _soon() -> str:
    return ('<span class="so-soon" role="status" aria-label="Download coming soon">Coming soon</span>')


def _single_link() -> str:
    """Single player (site_pages.py), only while that page is public (it is the 404 until SINGLE_PLAYER_PUBLIC=1)."""
    sp = _sp()
    try:
        on = bool(sp.single_public()) if sp is not None else False
    except Exception:   # noqa: BLE001
        on = False
    return ' and <a href="/wrath/single-player">Single player</a>' if on else ""


def get_block() -> str:
    """The download: the repo's latest release and its source once released; "Coming soon" before that."""
    if released():
        return ('<div class="so-dl" id="download-box"><b>Leyline Solo for Windows</b>'
                '<span class="lp-dim">On the release page, download the installer and run it. The <b>.sha256</b> file '
                'beside it is its checksum. Built in the open by GitHub\'s own machines from our source.</span>'
                '<p class="lp-acts">' + _btn(DOWNLOAD, "Download for Windows") + _btn(REPO, "Source code", True)
                + '</p></div>')
    return ('<div class="so-dl" id="download-box"><b>Leyline Solo for Windows</b>'
            '<span class="lp-dim">Not out yet. When it is, the installer and the source code are published together '
            'on GitHub, and the download appears right here.</span>'
            '<p class="lp-acts">' + _soon() + _btn(GUIDE, "Read the guide", True) + '</p></div>')


def body() -> str:
    first = (_btn(DOWNLOAD, "Download for Windows") if released() else _soon())
    hero = ('<section class="lp-hero so-hero" aria-labelledby="so-h"><div>'
            '<p class="lp-kick">Leyline Solo</p>'
            '<!-- TODO(owner): the heading and the line under it, your words (drafts in HEAD and LEDE) -->'
            f'<h1 id="so-h">{_e(HEAD)}</h1><p class="lp-lede">{_e(LEDE)}</p>'
            '<p class="lp-acts">' + first + _btn("#start", "How it works", True) + _btn(GUIDE, "Guide", True) + '</p>'
            '<ul class="so-toc" aria-label="On this page">'
            + "".join(f'<li><a href="#{a}">{_e(t)}</a></li>' for a, t in (
                ("who", "Who it's for"), ("start", "Three steps"), ("companions", "Companions"),
                ("invite", "Invite a friend"), ("characters", "Characters"), ("notes", "Good to know"),
                ("faq", "Questions"), ("download", "Download")))
            + '</ul></div>' + hero_art() + '</section>')
    who = ('<section class="lp-sec" id="who" aria-labelledby="so-who"><h2 id="so-who">Who it\'s for</h2>'
           '<ul class="lp-points">' + "".join(f"<li><b>{_e(t)}</b><span>{_e(s)}</span></li>" for t, s in WHO)
           + '</ul></section>')
    start = ('<section class="lp-sec so-band" id="start" aria-labelledby="so-start"><div>'
             '<h2 id="so-start">Three steps</h2><ol class="lp-steps">'
             '<li><h3>Install</h3><p>Download Leyline Solo for Windows and run the installer. It asks for nothing '
             'else: no account, no extra programs.</p></li>'
             '<li><h3>Point it at your game folder</h3><p>Choose your own first-era game folder, the one with the '
             'game\'s program and its Data folder. Leyline Solo checks it is the right one, then reads its maps once '
             '(up to an hour, the first time only).</p></li>'
             '<li><h3>Press Play</h3><p>Your world starts on your PC, the game opens and logs you in by itself, and '
             'you land on your characters. New here? <b>Quick start</b> makes a character and takes you straight in.'
             '</p></li></ol></div>' + pic("slots/solo-app.webp") + '</section>')
    companions = ('<section class="lp-sec" id="companions" aria-labelledby="so-comp">'
                  '<h2 id="so-comp">Your companions</h2>'
                  '<p>Your world starts quiet: no strangers, just you. Companions come when you call them, by typing in '
                  'the game\'s chat:</p><ul class="so-cmds">'
                  + "".join(f'<li><code>{_e(c)}</code><span>{_e(s)}</span></li>' for c, s in COMMANDS)
                  + '</ul><p style="margin-top:16px">Then give the whole party orders by saying a word in party chat:'
                  '</p><ul class="so-orders" aria-label="Party orders">'
                  + "".join(f"<li><code>{_e(o)}</code></li>" for o in ORDERS)
                  + '</ul><p class="lp-dim" style="margin-top:12px">Companions are real characters on your own '
                  'account, so they show on character select too. The full list is in '
                  f'<a href="{GUIDE}#playing-with-companions">the guide</a>.</p>'
                  + '<div style="margin-top:16px">' + pic("slots/solo-party.webp") + '</div></section>')
    invite = ('<section class="lp-sec so-band" id="invite" aria-labelledby="so-inv"><div>'
              '<h2 id="so-inv">Invite a friend</h2>'
              '<p>Want company? While your world runs, press <b>Invite a friend</b>. Leyline Solo makes one link for one '
              'friend, with a login made just for them, and checks the way in before you send it.</p>'
              '<ul class="lp-list"><li><b>On the same network</b> (your home, a LAN party): your friend comes '
              'straight in.</li><li><b>Anywhere else</b>: through the Leyline relay. Nothing to open on your router, '
              'and your address is never shown.</li></ul>'
              '<p class="lp-dim">Your friend needs Leyline Solo and their own first-era game folder. Game files never '
              'travel between you.</p></div>' + pic("slots/solo-invite.webp") + '</section>')
    chars = ('<section class="lp-sec so-band" id="characters" aria-labelledby="so-chars"><div>'
             '<h2 id="so-chars">Characters</h2>'
             '<p>Every hero you make is in one list: name, level and class, the way old single player games kept '
             'them. Pick one and press <b>Play</b>: the world starts if it isn\'t running and that character goes '
             'straight in.</p>'
             '<p class="lp-dim">Your characters live on your PC and nowhere else. The online side of Leyline, our '
             'realms, is the Leyline launcher\'s.</p></div>' + pic("slots/solo-characters.webp") + '</section>')
    notes = ('<section class="lp-sec" id="notes" aria-labelledby="so-notes"><h2 id="so-notes">Good to know</h2>'
             '<ul class="lp-points">' + "".join(f"<li><b>{_e(t)}</b><span>{_e(s)}</span></li>" for t, s in NOTES)
             + '</ul></section>')
    faq = ('<section class="lp-sec lp-faq" id="faq" aria-labelledby="so-faq"><h2 id="so-faq">Questions</h2>'
           + "".join(f"<details><summary>{_e(q)}</summary><p>{_e(a)}</p></details>" for q, a in FAQ)
           + f'<p class="lp-dim">Something else? Ask on the <a href="{_e(_discord())}" rel="noopener noreferrer">'
           f'Discord</a>, or read <a href="{GUIDE}">the guide</a>{_single_link()}.</p></section>')
    dl = ('<section class="lp-sec" id="download" aria-labelledby="so-dl"><h2 id="so-dl">Download</h2>'
          + get_block() + '</section>')
    return (f'<div class="lp so">{hero}{who}{start}{companions}{invite}{chars}{notes}{faq}{dl}'
            f'<p class="lp-legal">{_e(LEGAL)} Game names belong to their owners.</p></div>')


def page() -> bytes:
    """The page in the site's frame (the hub's side menu with the front door on, else the Leyline look), with
    site_pages.py's look (lp-*) and ours on top; a plain page when neither frame is there."""
    sp = _sp()
    css = (getattr(sp, "CSS", "") if sp is not None else "") + CSS
    full = "Leyline Solo · Leyline"
    meta = ""
    meta_f = portal("card_meta")
    try:
        meta = meta_f(PATH, full) if callable(meta_f) else ""
    except Exception:   # noqa: BLE001
        meta = ""
    for frame in ("hub_frame", "ly_shell"):
        f = portal(frame)
        if callable(f):
            try:
                out = f(full, body(), PATH, "single", css=css, meta=meta)
            except Exception:   # noqa: BLE001
                out = None
            if out is not None:
                return out
    return (f'<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" '
            f'content="width=device-width,initial-scale=1"><title>{_e(full)}</title>'
            f'<style>{css}</style></head><body>{body()}</body></html>').encode("utf-8")


# ------------------------------------------------------------------------------------------------- route
def match(path: str) -> bool:
    return (path or "").rstrip("/") == PATH


def get(h, path: str, query: dict) -> None:
    lim, bucket = portal("browse_limit"), portal("rl_bucket")
    try:
        key = bucket(h.ip()) if callable(bucket) else h.ip()
        if lim is not None and not lim.hit(key, 600):
            busy = portal("busy_page")
            h.send(429, busy(back=path) if callable(busy) else b"busy\n")
            return
    except Exception:   # noqa: BLE001: no limiter to hand, serve it anyway
        pass
    try:
        out = page()
    except Exception:   # noqa: BLE001
        nf = portal("not_found_page")
        h.send(404, nf() if callable(nf) else b"not found\n")
        return
    h.send(200, out)
