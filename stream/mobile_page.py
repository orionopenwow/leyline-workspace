"""Leyline on your phone: the mobile release page, for stream/wrath_portal.py (it hooks this in; bind() gives it the
portal's helpers). Owner, 5 Nov 2026: "design its webpage release with screenshots and depictions and guides".

    GET /wrath/mobile        what you can do on a phone (play in the browser, add to home screen, touch controls, solo
                             on this phone, join a friend), the Android app (from the Leyline repo's GitHub releases:
                             the leyline android workflow's android-latest pre-release), the screenshots, the guides
                             (install, first run, join a friend, phone test, troubleshooting), what needs a computer,
                             the questions people ask, and the legal line.

The screenshots are our own, made from the real pages at phone sizes by tools/mobile-shots/mobile_shots.cjs (the phone
app with a fake core, the touch controls over a plain stand-in for the game): stream/www-files/art/mobile/*.webp,
served at /wrath/art/mobile/<name>.webp (wrath_portal.py MOBILE_ART_RE). None of them shows game data, so they stay up
with the game data killswitch. A missing file is a framed panel with its caption, never a broken picture.

Drawn with site_pages.py's look (its CSS and its frame: the hub's side menu, or the Leyline look) when that module is
there; a plain page otherwise. Standard library only; nothing here raises to the caller. Words follow docs/WORDS.md:
"Log in", no game brand names (the one legal line names the company), no client version numbers in sentences.
TODO(owner): the lines marked below, in your own words.
"""
from __future__ import annotations

import html
import os
import re

portal = lambda name: None   # noqa: E731  (bind() sets it: the portal's globals)


def bind(lookup) -> None:
    global portal

    def get(name):
        try:
            return lookup(name)
        except Exception:   # noqa: BLE001
            return None
    portal = get


PATH = "/wrath/mobile"
ART = "mobile/"
DEFAULT_REPO = "https://github.com/orionopenwow/Leyline"
# The mobile repo, once the owner has made it on GitHub (docs/MOBILE-RELEASE.md): WU_MOBILE_REPO=https://github.com/...
# Until then the page names the Leyline repo, where the app is built.
MOBILE_REPO_ENV = "WU_MOBILE_REPO"
ANDROID_TAG = "android-latest"   # tools/github-workflows/leyline-android.yml, job publish
LEGAL = "Leyline is an independent fan project, not affiliated with or endorsed by Blizzard Entertainment."


def _e(s) -> str:
    return html.escape(str(s))


def _repo() -> str:
    r = portal("LEYLINE_REPO")
    return r if isinstance(r, str) and re.fullmatch(r"https://github\.com/[\w.-]+/[\w.-]+", r) else DEFAULT_REPO


def mobile_repo() -> str:
    """The mobile repo's address when the owner has set it (a github.com repo), else ""."""
    r = os.environ.get(MOBILE_REPO_ENV, "").strip().rstrip("/")
    return r if re.fullmatch(r"https://github\.com/[\w.-]+/[\w.-]+", r) else ""


def releases_url() -> str:
    """Where the Android app is downloaded: the repo's GitHub releases (the android-latest pre-release is there once
    CI has published it; the releases page itself always opens)."""
    return (mobile_repo() or _repo()) + "/releases"


def apk_url() -> str:
    return (mobile_repo() or _repo()) + f"/releases/download/{ANDROID_TAG}/Leyline-debug.apk"


def _discord() -> str:
    d = portal("DISCORD_URL")
    return d if isinstance(d, str) and d.startswith("https://") else "https://discord.gg/RfChVr8J7T"


def _sp():
    return portal("site_pages")


# ------------------------------------------------------------------------------------------------- pictures
SHOTS = [
    ("phone-play", "The Leyline app on a phone: the Play tab with Play, Play solo on this phone and Join a friend",
     "The app's Play tab", 720, 1558),
    ("phone-touch", "The browser game's touch controls: the stick, the attack key, four ability keys and the tray",
     "The touch controls (shown over a plain field)", 1100, 508),
    ("phone-solo", "The Play solo on this phone button and its line", "Play solo on this phone", 720, 167),
    ("phone-join", "Join a friend: Paste invite link and join, with a friend's invite link typed in", "Join a friend", 720, 370),
    ("phone-realms", "The app's Realms tab: the official realm and its Play", "Realms", 720, 1558),
    ("phone-mods", "The app's Mods tab: browse mods and addons, each card says whether it works on a phone", "Mods and addons", 720, 1558),
    ("phone-account", "The app's Account tab: log in with your Hero Key or Google", "Log in", 720, 1558),
    ("phone-play-landscape", "The app's Play tab with the phone held sideways", "Sideways", 1100, 508),
]
SHOT = {n: (alt, cap, w, h) for n, alt, cap, w, h in SHOTS}


def _have(name: str) -> bool:
    f = portal("art_present")
    try:
        return bool(f(ART + name + ".webp")) if callable(f) else False
    except Exception:   # noqa: BLE001
        return False


def pic(name: str, cls: str = "") -> str:
    alt, cap, w, h = SHOT[name]
    if _have(name):
        return (f'<figure class="lp-pic mb-shot {cls}"><img src="/wrath/art/{ART}{_e(name)}.webp" alt="{_e(alt)}" '
                f'width="{w}" height="{h}" loading="lazy" decoding="async"><figcaption>{_e(cap)}</figcaption></figure>')
    return (f'<figure class="lp-pic lp-empty mb-shot {cls}" data-slot="{ART}{_e(name)}.webp">'
            f'<!-- tools/mobile-shots/mobile_shots.cjs makes stream/www-files/art/{ART}{_e(name)}.webp -->'
            f'<div class="lp-ph" style="aspect-ratio:{w}/{h}" role="img" aria-label="{_e(cap)}: picture coming soon">'
            '<span>Picture coming soon</span></div><figcaption>{0}</figcaption></figure>').replace("{0}", _e(cap))


# ------------------------------------------------------------------------------------------------- words
# TODO(owner): the heading and the line under it, in your words (drafts)
HEAD = "Leyline in your pocket"
LEDE = ("Play in your phone's browser or in the Leyline app for Android: the official realm with touch controls, a "
        "world of your own that runs on the phone itself, and a friend's realm from one invite link.")

WHAT = [
    ("Play in the browser", "Open our website on your phone and press Play. Nothing to install: the game "
     "comes to you as you play."),
    ("Add it to your home screen", "From the game page, add Leyline to your home screen: it opens full screen, "
     "sideways, like an app."),
    ("Touch controls", "A stick to walk, an attack key with four ability keys round it, a tray for bags, quests and "
     "chat. Drag to look, pinch to zoom, two fingers to right-click."),
    ("Play solo on this phone", "Your own world, run by the phone itself: no realm, your companion at your side, "
     "kept on this phone."),
    ("Join a friend", "A friend hosts on their PC and sends you an invite link. Copy it, tap Paste invite link and "
     "join: their realm opens on your phone."),
    ("Addons in the game", "Interface addons work on your phone too: add an addon's .zip in the game's Your addons, "
     "and it loads the next time the game opens."),
]

# What works where: (thing, on a phone, why / where instead)
WHERE = [
    ("The official realm", "Yes", "In the browser or the app, with touch controls."),
    ("A friend's realm on their PC", "Yes", "From their invite link: the app's Join a friend, or Play in your browser on the link's page."),
    ("Community realms", "Most", "Those for the same game as the browser game play in the app; others need the game on a computer."),
    ("Your own world", "Yes", "Play solo on this phone: the phone runs it, just for you."),
    ("Hosting a realm for others", "No", "A phone can't keep a realm up for friends. Host on a PC with the Leyline launcher."),
    ("A friend's Den", "No", "Dens play in the game on a computer."),
    ("Interface addons", "Yes", "Your addons, in the game page."),
    ("Realm mods and game modes", "Yes", "On a realm that runs them: nothing to install."),
    ("Client mods, looks, HD packs", "No", "They change a game folder: Leyline on a computer installs them."),
    ("Be a node", "No", "Carrying the network needs a computer that stays on."),
]

FAQ = [
    ("Is there a server on my phone?",
     "For Play solo, yes: a small server of our own runs inside the game page, so your world needs no realm and goes "
     "nowhere. It is just for you. A phone can't host a realm for friends: for that, a friend (or you) hosts on a PC, "
     "and the phone joins with the invite link."),
    ("Do I need my own game files on a phone?",
     "No. The browser game brings what it needs while you play. Leyline never puts game files in the app."),
    ("Can I play on an iPhone or an iPad?",
     "In the browser, yes, when your browser can draw the game (it needs WebGPU). The app is for Android phones."),
    ("Why isn't the app in an app store?",
     # TODO(owner): your words, and the plan for a store listing if there is one
     "Not yet. For now it comes from our GitHub releases, built in the open by GitHub's own machines from our source."),
    ("Is installing an app from outside the store safe?",
     "Android asks you to allow it once, for the browser or Files app you install from. Our app asks for no "
     "permissions beyond the internet, and its source and every build are public. You can check the download against "
     "its .sha256 file on the same release."),
    ("Does it use a lot of data?",
     "The first time in a place, the game downloads it; after that your phone keeps it. Use Wi-Fi for your first "
     "sessions if your data plan is small."),
    ("Am I logged in on the game page and in the app at once?",
     "Log in once in the app (Account) and once on the game page the first time you press Play: the app's web view "
     "keeps both after that."),
    ("Is it free?", "Yes. No shop and nothing to buy."),
]

TROUBLE = [
    ("Play shows \"This phone's web view can't draw the game yet\"",
     "The app's built-in web view has no WebGPU on this phone. Tap Open in Chrome. Updating Chrome and Android System "
     "WebView from the Play Store often fixes it."),
    ("A black screen in the browser", "Use Chrome on Android, up to date. Close other tabs and apps, then open the game again."),
    ("The touch controls don't show", "Open the game with ?touch=1 at the end of its address once: it is remembered. "
     "Menu › Controls sets their size and how see-through they are."),
    ("Play solo says it can't start here", "This browser doesn't allow what solo needs (shared memory on the game page): "
     "the game plays on the realm instead. Chrome on Android has it."),
    ("Join a friend says it's a Den", "Dens play in the game on a computer. Ask your friend for a realm invite instead, or join from a PC."),
    ("\"This invite link no longer works\"", "Links work for 24 hours and stop when the host stops sharing. Ask your friend for a new one."),
    ("The friend's realm isn't answering", "Your friend opens Leyline on their PC and presses Invite a friend: it checks "
     "everything and says what to fix. Their PC must stay awake."),
    ("The app won't install", "Allow Install unknown apps for the app you downloaded it with (Settings › Apps › Special "
     "app access). Switching between the test build and a signed build? Uninstall the old one first."),
    ("The keyboard covers the chat", "Tray › Chat moves the chat box above the keyboard. If it doesn't, copy the debug "
     "line (Menu › Controls › Copy debug info) and send it to us."),
]


# ------------------------------------------------------------------------------------------------- the page
CSS = """
.mb .mb-hero{grid-template-columns:minmax(0,1.2fr) minmax(0,.8fr)}
.mb .mb-hero .mb-shot{max-width:320px;justify-self:center}
@media (max-width:820px){.mb .mb-hero{grid-template-columns:1fr}.mb .mb-hero .mb-shot{max-width:260px}}
@media (max-width:560px){.mb .mb-toc a{min-height:36px;padding:4px 12px;font-size:14px}}
.mb .mb-shot img{border-radius:22px;border:6px solid #0b0d14;box-shadow:0 0 0 1px var(--lp-line),0 14px 40px rgba(0,0,0,.45)}
.mb .mb-wide img{border-radius:14px;border-width:5px}
.mb .mb-gal{display:grid;grid-template-columns:repeat(auto-fill,minmax(min(210px,100%),1fr));gap:16px;align-items:start}
.mb .mb-gal .mb-wide{grid-column:span 2}
@media (max-width:560px){.mb .mb-gal{grid-template-columns:1fr 1fr}.mb .mb-gal .mb-wide{grid-column:1/-1}}
.mb .mb-band{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:clamp(14px,3vw,28px);align-items:center}
@media (max-width:820px){.mb .mb-band{grid-template-columns:1fr}}
.mb table{width:100%;border-collapse:collapse;font-size:14.5px}
.mb th,.mb td{text-align:left;vertical-align:top;padding:9px 10px;border-bottom:1px solid var(--lp-line)}
.mb th{font-weight:650;color:var(--lp-dim);font-size:12.5px;letter-spacing:.06em;text-transform:uppercase}
.mb td:first-child{font-weight:600}
.mb .mb-tbl{overflow-x:auto;background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r)}
.mb .mb-yes,.mb .mb-no,.mb .mb-most{display:inline-block;min-width:3.2em;padding:2px 9px;border-radius:999px;font:700 12px/1.4 system-ui,sans-serif;text-align:center}
.mb .mb-yes{background:rgba(95,191,127,.18);color:#8fe0a8}
.mb .mb-most{background:rgba(224,160,48,.18);color:#f1c56a}
.mb .mb-no{background:rgba(160,160,180,.14);color:var(--lp-dim)}
.mb .mb-dl{display:flex;flex-direction:column;gap:6px;background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:16px}
.mb .mb-dl .lp-acts{margin-top:4px}
.mb .mb-toc{display:flex;flex-wrap:wrap;gap:8px;margin:18px 0 0;padding:0;list-style:none}
.mb .mb-toc a{display:inline-flex;align-items:center;min-height:40px;padding:6px 14px;border:1px solid var(--lp-line);border-radius:999px;text-decoration:none;color:var(--lp-ink)}
.mb .mb-toc a:hover,.mb .mb-toc a:focus-visible{border-color:var(--lp-acc)}
.mb .lp-btn{min-height:44px}
"""


def _btn(href: str, label: str, ghost: bool = False) -> str:
    rel = ' rel="noopener noreferrer"' if href.startswith("https://") else ""
    return f'<a class="lp-btn{" ghost" if ghost else ""}" href="{_e(href)}"{rel}>{_e(label)}</a>'


def body() -> str:
    rel = releases_url()
    src = mobile_repo() or _repo()
    hero = ('<section class="lp-hero mb-hero" aria-labelledby="mb-h"><div>'
            '<p class="lp-kick">On your phone</p>'
            '<!-- TODO(owner): the heading and the line under it, your words (drafts in HEAD and LEDE) -->'
            f'<h1 id="mb-h">{_e(HEAD)}</h1><p class="lp-lede">{_e(LEDE)}</p>'
            '<p class="lp-acts">' + _btn("/wrath", "Play in your browser") + _btn("#android", "Get the Android app", True)
            + _btn("#guides", "Guides", True) + '</p>'
            '<ul class="mb-toc" aria-label="On this page">'
            + "".join(f'<li><a href="#{a}">{_e(t)}</a></li>' for a, t in (
                ("what", "What you can do"), ("shots", "Screenshots"), ("android", "The Android app"), ("guides", "Guides"),
                ("where", "Phone or computer?"), ("trouble", "Troubleshooting"), ("faq", "Questions")))
            + '</ul></div>' + pic("phone-play") + '</section>')
    what = ('<section class="lp-sec" id="what" aria-labelledby="mb-what"><h2 id="mb-what">What you can do on a phone</h2>'
            '<ul class="lp-points">' + "".join(f"<li><b>{_e(t)}</b><span>{_e(s)}</span></li>" for t, s in WHAT)
            + '</ul></section>')
    touch = ('<section class="lp-sec mb-band" aria-labelledby="mb-touch"><div>'
             '<h2 id="mb-touch">Made for your thumbs</h2>'
             '<p>Hold the phone sideways. The stick walks (double-tap it to run on its own), the big key attacks or talks '
             'to whatever you have targeted, and four ability keys sit round it (swipe them for more). Tap the reticle '
             'for the nearest enemy. Drag the world to look round, pinch to zoom, and tap with two fingers to '
             'right-click. The tray at the top right opens bags, quests, chat and the menu.</p>'
             '<p class="lp-dim">Menu › Controls: bigger or smaller buttons, how see-through they are, left-handed, and '
             'touch and hold to right-click.</p></div>' + pic("phone-touch", "mb-wide") + '</section>')
    solo = ('<section class="lp-sec mb-band" aria-labelledby="mb-solo"><div>'
            '<h2 id="mb-solo">A world of your own, on the phone</h2>'
            '<p>Play solo on this phone opens the game with its own small server running inside the page. No realm, no '
            'queue and nobody else: you, your companion, the training dummies and the building tools. Your world is '
            'saved on the phone and is there next time, as you left it.</p>'
            '<p class="lp-dim">It is a sandbox, not the full realm: no quests or loot there yet. For those, press Play.</p>'
            '</div>' + pic("phone-solo") + '</section>')
    join = ('<section class="lp-sec mb-band" aria-labelledby="mb-join"><div>'
            '<h2 id="mb-join">Join a friend in one tap</h2>'
            '<p>A phone can\'t host a realm, but it can join one. Your friend presses <b>Invite a friend</b> in Leyline on '
            'their PC and sends you the link. On your phone, copy it, open the Leyline app and tap <b>Paste invite link '
            'and join</b>. Their realm opens in the app, and its own page lets you make your account there.</p>'
            '<p class="lp-dim">No app? Open the link in your phone\'s browser and press <b>Play in your browser</b>. The '
            'secret part of the link stays on your phone.</p></div>' + pic("phone-join") + '</section>')
    gal = ('<section class="lp-sec" id="shots" aria-labelledby="mb-shots"><h2 id="mb-shots">Screenshots</h2>'
           '<p class="lp-dim">From the app and the game page, at phone size.</p><div class="mb-gal">'
           + "".join(pic(n, "mb-wide" if w > h else "") for n, _a, _c, w, h in SHOTS if n not in ("phone-play", "phone-touch", "phone-solo", "phone-join"))
           + '</div></section>')
    android = ('<section class="lp-sec" id="android" aria-labelledby="mb-android"><h2 id="mb-android">The Android app</h2>'
               '<div class="mb-band"><div>'
               '<p>The Leyline app is the same launcher as on a computer, laid out for a phone: Play, Realms, Mods and '
               'Account, a tab bar at the bottom, and the game full screen in the app. It is built in the open by '
               'GitHub\'s own machines from our source, tested on an Android emulator, and published on our GitHub '
               'releases as <b>Leyline for Android (latest)</b>.</p>'
               '<ul class="lp-list"><li>Android phones (Android 8 or newer).</li>'
               '<li>No game files inside, and none downloaded by the app.</li>'
               '<li>Asks for no permissions beyond the internet.</li></ul></div>'
               '<div class="mb-dl"><b>Download</b><span class="lp-dim">On the releases page, open <b>Leyline for '
               'Android (latest)</b> and tap <b>Leyline-debug.apk</b>. The <b>.sha256</b> file beside it is its '
               'checksum.</span><p class="lp-acts">' + _btn(rel, "GitHub releases") + _btn(src, "Source code", True)
               + '</p></div></div></section>')
    guides = ('<section class="lp-sec" id="guides" aria-labelledby="mb-guides"><h2 id="mb-guides">Guides</h2>'
              '<h3 id="install">Install the app (Android)</h3><ol class="lp-steps">'
              f'<li><h3>Download</h3><p>On your phone, open <a href="{_e(rel)}" rel="noopener noreferrer">our GitHub '
              'releases</a>, then <b>Leyline for Android (latest)</b>, and tap <b>Leyline-debug.apk</b>.</p></li>'
              '<li><h3>Allow it once</h3><p>Android asks whether your browser (or Files) may install apps: <b>Settings</b>, '
              'then <b>Allow from this source</b>. Play Protect may say it doesn\'t know us: <b>Install anyway</b>.</p></li>'
              '<li><h3>Open Leyline</h3><p>Tap <b>Open</b>, or find Leyline among your apps. It opens on Play.</p></li></ol>'
              '<h3 id="home-screen" style="margin-top:22px">No app: the home screen</h3><ol class="lp-steps">'
              '<li><h3>Open the game</h3><p>In Chrome on Android (or Safari on iPhone), open our website and press '
              '<b>Play</b>.</p></li>'
              '<li><h3>Add it</h3><p>Chrome: the ⋮ menu, then <b>Add to Home screen</b>. Safari: the Share button, then '
              '<b>Add to Home Screen</b>.</p></li>'
              '<li><h3>Play from the icon</h3><p>It opens full screen and sideways, without the browser\'s bars.</p></li></ol>'
              '<h3 id="first-run" style="margin-top:22px">First run</h3><ol class="lp-steps">'
              '<li><h3>Log in</h3><p>Account: your Hero Key, Google, or your name and password. New? <b>Get your Hero Key</b> '
              'makes a free account at once.</p></li>'
              '<li><h3>Press Play</h3><p>The game opens full screen. Log in on the game page once the first time; turn the '
              'phone sideways.</p></li>'
              '<li><h3>Learn the controls</h3><p>Walk with the stick, tap the big key to attack, drag to look. The Back '
              'button brings you to the app.</p></li></ol>'
              '<h3 id="join-guide" style="margin-top:22px">Join a friend</h3><ol class="lp-steps">'
              '<li><h3>Your friend invites</h3><p>On their PC, in Leyline: <b>Invite a friend</b>. When every line is green, '
              'the link is copied: they send it to you.</p></li>'
              '<li><h3>Copy, then one tap</h3><p>Copy the link from the message, open the app, tap <b>Paste invite link and '
              'join</b>.</p></li>'
              '<li><h3>Make your account there</h3><p>The realm\'s own page asks for an account name and password, once. '
              'Your password never leaves your phone.</p></li></ol>'
              '<h3 id="addons" style="margin-top:22px">Addons on your phone</h3>'
              '<p>Interface addons run in the game itself. Download an addon\'s .zip on your phone, then in the game page '
              'open <b>Your addons</b>, choose the .zip, and open the game again. Each mod in the app\'s Mods tab says '
              'whether it works on a phone.</p>'
              '<h3 id="phone-test" style="margin-top:22px">Phone test (five minutes)</h3><ol class="lp-list">'
              '<li>On the character list, tap <b>Controls</b> (top right) › <b>Tap test</b>: tap the five gold rings. '
              'Expect "5 of 5 hit".</li>'
              '<li>In the world: walk with the stick, double-tap it to run on its own, hold Jump, drag to look, pinch to zoom.</li>'
              '<li>Tap the reticle (the nearest enemy), then the big key to attack.</li>'
              '<li>Tray › Bags: one finger on an item, tap a second finger near it: it is used (a right-click).</li>'
              '<li>Tray › Chat: the box sits above the keyboard. Send hi.</li>'
              '<li>Menu › Controls › <b>Copy debug info</b>, and paste it to us on the Discord with your phone\'s name.</li>'
              '</ol></section>')
    where = ('<section class="lp-sec" id="where" aria-labelledby="mb-where"><h2 id="mb-where">Phone or computer?</h2>'
             '<p class="lp-dim">What plays on a phone, and what needs Leyline on a computer.</p>'
             '<div class="mb-tbl"><table><thead><tr><th scope="col">What</th><th scope="col">Phone</th>'
             '<th scope="col">How</th></tr></thead><tbody>'
             + "".join(f'<tr><td>{_e(t)}</td><td><span class="mb-{ {"Yes": "yes", "No": "no"}.get(y, "most") }">{_e(y)}</span>'
                       f'</td><td>{_e(w)}</td></tr>' for t, y, w in WHERE)
             + '</tbody></table></div></section>')
    trouble = ('<section class="lp-sec" id="trouble" aria-labelledby="mb-trouble"><h2 id="mb-trouble">Troubleshooting</h2>'
               '<div class="mb-tbl"><table><thead><tr><th scope="col">What you see</th><th scope="col">What to do</th></tr>'
               '</thead><tbody>' + "".join(f'<tr><td>{_e(a)}</td><td>{_e(b)}</td></tr>' for a, b in TROUBLE)
               + f'</tbody></table></div><p class="lp-dim" style="margin-top:12px">Still stuck? Ask on the '
               f'<a href="{_e(_discord())}" rel="noopener noreferrer">Discord</a>, with Menu › Controls › Copy debug info.</p>'
               '</section>')
    faq = ('<section class="lp-sec lp-faq" id="faq" aria-labelledby="mb-faq"><h2 id="mb-faq">Questions</h2>'
           + "".join(f"<details><summary>{_e(q)}</summary><p>{_e(a)}</p></details>" for q, a in FAQ)
           + '<p class="lp-dim">More about the network and hosting: <a href="/wrath/docs/mobile">the phone docs</a> and '
           '<a href="/wrath/docs/host-for-friends">Host for friends</a>.</p></section>')
    return (f'<div class="lp mb">{hero}{what}{touch}{solo}{join}{gal}{android}{guides}{where}{trouble}{faq}'
            f'<p class="lp-legal">{_e(LEGAL)} Game names belong to their owners.</p></div>')


def page() -> bytes:
    """The page in the site's frame (the hub's side menu with the front door on, else the Leyline look), with
    site_pages.py's look (lp-*) and ours on top; a plain page when neither frame is there."""
    sp = _sp()
    css = (getattr(sp, "CSS", "") if sp is not None else "") + CSS
    full = "On your phone · Leyline"
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
                out = f(full, body(), PATH, "mobile", css=css, meta=meta)
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
