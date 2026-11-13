"""The site's showcase pages, for stream/wrath_portal.py (it hooks this in, bind() gives it the portal's helpers).
Grand release, 4 Nov 2026: the owner's ask for Single player as a flagship ("a section on the main site showing it
off once finished"), a shelf of every way to play, the 12-episode guide series and a press kit.

    GET /wrath/single-player        Single player: the trailer slot, what it is, the companions gallery, How to start
                                    and the questions people ask. Only with SINGLE_PLAYER_PUBLIC=1 (default off: it
                                    ships dark until the grand release; off, the address is the site's 404).
    GET /wrath/modes/<id>           one way to play from the shelf (MODES): its picture, one line, How it works and
                                    Play now. Only MODES' own ids: every other /wrath/modes/<id> stays the host
                                    templates' page (stream/modepack.py), and none of these ids is one of theirs.
    GET /wrath/guides               the guide series: 12 episodes, each with a video slot and short steps
    GET /wrath/guides/<slug>        one episode: the video slot, the steps, Stuck? and the Discord
    GET /wrath/press                the press kit: the mark, screenshots, the paragraph about Leyline, the legal line

    shelf()                         the Modes shelf, put first on /wrath/modes (stream/modes.py games=)
    front_sections()                the front page's Ways to play and (flag on) Single player sections (stream/hub.py)

Pictures are slots: our own screenshots in stream/www-files/art/slots/ (served at /wrath/art/slots/<name>, behind the
game data killswitch like every other picture of the game). A slot whose file is not there yet is a plain framed
panel with its caption: never a broken image, never game art from the web. A video slot is a <video> only when its
file is there; else its poster, else the panel. TODO(owner): drop the files named in SLOTS below into that folder.

Standard library only; nothing here raises to the caller. Player-facing words follow docs/WORDS.md: "Log in", no game
brand names (the one legal line in PRESS names the company), no client version numbers in sentences.
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


# The switch for anything new and not yet announced (owner 4 Nov: Single player is the grand release's flagship, so it
# stays dark until then). Read on every request: no restart to turn it on.
FLAG_ENV = "SINGLE_PLAYER_PUBLIC"


def single_public() -> bool:
    return os.environ.get(FLAG_ENV, "0").strip() == "1"


SINGLE = "/wrath/single-player"
MODES_PATH = "/wrath/modes"
GUIDES = "/wrath/guides"
PRESS = "/wrath/press"
SLOT_DIR = "slots/"
# The one line allowed to name the company (GR rules: only this legal line may name it).
LEGAL = "Leyline is an independent fan project, not affiliated with or endorsed by Blizzard Entertainment."


def _e(s) -> str:
    return html.escape(str(s))


def _discord() -> str:
    d = portal("DISCORD_URL")
    return d if isinstance(d, str) and d.startswith("https://") else "https://discord.gg/RfChVr8J7T"


def _download() -> str:
    f = portal("download_link")
    try:
        v = f() if callable(f) else ""
    except Exception:   # noqa: BLE001
        v = ""
    return v or "/wrath/download"


def _rel(href: str) -> str:
    return ' rel="noopener noreferrer"' if href.startswith("https://") else ""


# ------------------------------------------------------------------------------------------------- pictures
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


def _game_pic(name: str) -> bool:
    """A picture of the running game (a leyline-shot- or a slot file): off with the game data killswitch."""
    return name.startswith(("leyline-shot-", SLOT_DIR))


def _pic_ok(name: str) -> bool:
    return bool(name) and _have(name) and not (_game_pic(name) and _off())


def _first(names) -> str:
    for n in names if isinstance(names, (list, tuple)) else (names,):
        if _pic_ok(n):
            return n
    return ""


def slot(names, alt: str, caption: str, w: int = 1280, h: int = 720, cls: str = "") -> str:
    """A picture slot: the first of names that is there (our own screenshots; a later name is the stand-in), else a
    framed panel with the caption. The slot's own file is the first name: shown in a comment for the owner."""
    names = names if isinstance(names, (list, tuple)) else (names,)
    got = _first(names)
    want = names[0] if names else ""
    if got:
        return (f'<figure class="lp-pic {cls}"><img src="/wrath/art/{_e(got)}" alt="{_e(alt)}" width="{w}" height="{h}" '
                f'loading="lazy" decoding="async"><figcaption>{_e(caption)}</figcaption></figure>')
    return (f'<figure class="lp-pic lp-empty {cls}" data-slot="{_e(want)}"><!-- TODO(owner): our own screenshot here, '
            f'stream/www-files/art/{_e(want)} -->'
            f'<div class="lp-ph" style="aspect-ratio:{w}/{h}" role="img" aria-label="{_e(caption)}: picture coming soon">'
            '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="5" width="18" height="14" rx="2"/>'
            '<circle cx="9" cy="10" r="1.6"/><path d="M4 17l5-5 4 4 3-3 4 4"/></svg><span>Picture coming soon</span></div>'
            f'<figcaption>{_e(caption)}</figcaption></figure>')


def video_slot(base: str, poster_names, label: str, caption: str) -> str:
    """A video slot: a <video> with controls when slots/<base>.webm or .mp4 is there (poster from poster_names), else
    the poster as a still, else the framed panel. Nothing loads until Play is pressed (preload none)."""
    srcs = [(f"{SLOT_DIR}{base}.{x}", t) for x, t in (("webm", "video/webm"), ("mp4", "video/mp4"))]
    have = [(n, t) for n, t in srcs if _pic_ok(n)]
    poster = _first(poster_names)
    if have:
        p = f' poster="/wrath/art/{_e(poster)}"' if poster else ""
        return (f'<figure class="lp-pic lp-vid"><video controls playsinline preload="none"{p} width="1280" height="720" '
                f'aria-label="{_e(label)}">' + "".join(f'<source src="/wrath/art/{_e(n)}" type="{t}">' for n, t in have)
                + f'</video><figcaption>{_e(caption)}</figcaption></figure>')
    out = slot(list(poster_names) if isinstance(poster_names, (list, tuple)) else [poster_names], label, caption)
    if not poster:   # no poster either: the panel itself says the video is coming
        return (f'<!-- TODO(owner): the video, stream/www-files/art/{SLOT_DIR}{base}.webm (or .mp4) -->'
                + out.replace('<figure class="lp-pic', '<figure data-video="soon" class="lp-pic lp-vsoon', 1)
                .replace("Picture coming soon", "Video coming soon").replace("picture coming soon", "video coming soon"))
    # the video itself is still to come: say so under the poster, and leave the file names for the owner
    return out.replace('<figure class="lp-pic', f'<!-- TODO(owner): the video, stream/www-files/art/{SLOT_DIR}{base}.webm '
                       f'(or .mp4) --><figure data-video="soon" class="lp-pic lp-vsoon', 1).replace(
                           "</figcaption>", ' <span class="lp-soon">Video coming soon</span></figcaption>', 1)


# ------------------------------------------------------------------------------------------------- styles
# Works in both frames: the hub's (--lk-* tokens, the front door on) and the Leyline look (--ly-*), dark in both.
CSS = """
.lp{--lp-ink:var(--lk-text,var(--ly-ink,#e8ecf8));--lp-dim:var(--lk-muted,var(--ly-dim,#a9b0c8));--lp-line:var(--lk-border,var(--ly-line,rgba(140,150,220,.3)));
--lp-card:var(--lk-surface,rgba(18,22,54,.78));--lp-card2:var(--lk-surface-2,rgba(26,30,66,.8));--lp-acc:var(--lk-accent,var(--ly-cyan,#5b8def));
--lp-on:var(--lk-on-accent,#0b0d14);--lp-r:var(--lk-r,10px);width:min(1120px,100%);margin:0 auto;padding:clamp(10px,3vw,26px) clamp(8px,2.4vw,20px) 36px;
color:var(--lp-ink);box-sizing:border-box}
.lp *,.lp *::before,.lp *::after{box-sizing:border-box}
.lp h1{margin:0 0 10px;font-size:clamp(28px,5vw,46px);line-height:1.08;color:var(--lp-ink)}
.lp h2{margin:0 0 12px;font-size:clamp(21px,3vw,28px);line-height:1.15;color:var(--lp-ink)}
.lp h3{margin:0 0 6px;font-size:18px;line-height:1.2;color:var(--lp-ink)}
.lp p{margin:0 0 12px;line-height:1.55}
.lp a{color:var(--lp-acc)}
.lp-kick{margin:0 0 8px;font:700 12px/1 system-ui,sans-serif;letter-spacing:.16em;text-transform:uppercase;color:var(--lp-acc)}
.lp-lede{font-size:clamp(16px,2vw,19px);color:var(--lp-ink);max-width:46rem}
.lp-dim{color:var(--lp-dim)}
.lp-sec{margin:34px 0 0}
.lp-hero{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1.25fr);gap:clamp(16px,3vw,34px);align-items:center}
@media (max-width:820px){.lp-hero{grid-template-columns:1fr}}
.lp-acts{display:flex;flex-wrap:wrap;gap:10px;margin:16px 0 0}
.lp-btn{display:inline-flex;align-items:center;justify-content:center;min-height:42px;padding:9px 18px;border-radius:8px;border:1px solid var(--lp-acc);
background:var(--lp-acc);color:var(--lp-on)!important;font:650 15px/1.15 system-ui,sans-serif;text-decoration:none}
.lp-btn.ghost{background:transparent;color:var(--lp-ink)!important;border-color:var(--lp-line)}
.lp-btn:hover,.lp-btn:focus-visible{filter:brightness(1.08);box-shadow:0 0 0 3px color-mix(in srgb,var(--lp-acc) 30%,transparent)}
.lp-btn.ghost:hover,.lp-btn.ghost:focus-visible{border-color:var(--lp-acc)}
.lp-pic{margin:0;display:flex;flex-direction:column;gap:6px}
.lp-pic img,.lp-pic video{display:block;width:100%;height:auto;border-radius:var(--lp-r);border:1px solid var(--lp-line);background:#0b0e1c}
.lp-pic figcaption{font-size:13px;color:var(--lp-dim)}
.lp-ph{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;width:100%;border-radius:var(--lp-r);
border:1px dashed var(--lp-line);color:var(--lp-dim);font:600 13px/1.2 system-ui,sans-serif;letter-spacing:.04em;
background:radial-gradient(60% 80% at 30% 20%,rgba(120,110,255,.18),transparent 70%),radial-gradient(50% 70% at 80% 70%,rgba(60,190,255,.14),transparent 70%),var(--lp-card2)}
.lp-ph span{text-align:center;padding:0 8px}
.lp-ep .lp-ph{font-size:12px;gap:4px}.lp-ep .lp-ph svg{width:26px;height:26px}
.lp-ph svg{width:34px;height:34px;fill:none;stroke:currentColor;stroke-width:1.5;stroke-linecap:round;stroke-linejoin:round;opacity:.8}
.lp-soon{display:inline-block;margin-left:6px;padding:2px 8px;border:1px solid var(--lp-line);border-radius:999px;font-size:11px;letter-spacing:.06em;text-transform:uppercase}
.lp-points{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(290px,100%),1fr));gap:12px;margin:0;padding:0;list-style:none}
.lp-points li,.lp-card{background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:16px}
.lp-points b{display:block;margin:0 0 4px;font-size:16px}
.lp-points span{color:var(--lp-dim);font-size:14.5px;line-height:1.5}
.lp-gal{display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:14px}
.lp-steps{counter-reset:lp;display:grid;grid-template-columns:repeat(auto-fit,minmax(230px,1fr));gap:12px;margin:0;padding:0;list-style:none}
.lp-steps>li{counter-increment:lp;position:relative;background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:16px 16px 16px 60px}
.lp-steps>li::before{content:counter(lp);position:absolute;left:16px;top:14px;width:32px;height:32px;border-radius:50%;display:grid;place-items:center;
background:var(--lp-acc);color:var(--lp-on);font:800 16px/1 system-ui,sans-serif}
.lp-steps p{margin:0;color:var(--lp-dim);font-size:14.5px}
.lp-list{margin:0;padding-left:1.4rem}.lp-list li{margin:0 0 9px;line-height:1.5}
.lp-faq details{background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:0 16px;margin:0 0 8px}
.lp-faq summary{cursor:pointer;padding:14px 0;font-weight:650;list-style-position:outside}
.lp-faq details p{color:var(--lp-dim)}
.lp-shelf{display:grid;grid-template-columns:repeat(auto-fill,minmax(min(290px,100%),1fr));gap:14px}
.lp-mode{display:flex;flex-direction:column;gap:10px;background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:12px 12px 14px}
.lp-mode .lp-pic figcaption{display:none}
.lp-mode .lp-ph span{font-size:12px}
.lp-mode h3{margin:2px 0 0}
.lp-mode p{margin:0;color:var(--lp-dim);font-size:14.5px;line-height:1.45}
.lp-mode .lp-acts{margin-top:auto;padding-top:4px}
.lp-mode .lp-btn{min-height:36px;padding:7px 14px;font-size:14px}
.lp-tag{display:inline-block;align-self:flex-start;padding:3px 9px;border:1px solid var(--lp-line);border-radius:999px;font:650 11px/1.2 system-ui,sans-serif;
letter-spacing:.08em;text-transform:uppercase;color:var(--lp-dim)}
.lp-tag.soon{color:#f1c56a;border-color:rgba(241,197,106,.45)}
.lp-eps{display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:14px;margin:0;padding:0;list-style:none}
.lp-ep{display:flex;flex-direction:column;gap:8px;background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:12px 12px 14px;
text-decoration:none;color:var(--lp-ink)!important}
.lp-ep:hover,.lp-ep:focus-visible{border-color:var(--lp-acc)}
.lp-ep .lp-pic figcaption{display:none}
.lp-ep b{font-size:16.5px}.lp-ep span{color:var(--lp-dim);font-size:14px;line-height:1.45}
.lp-n{font:800 12px/1 system-ui,sans-serif;letter-spacing:.14em;text-transform:uppercase;color:var(--lp-acc)}
.lp-two{display:grid;grid-template-columns:minmax(0,1.5fr) minmax(0,1fr);gap:clamp(16px,3vw,30px);align-items:start}
@media (max-width:820px){.lp-two{grid-template-columns:1fr}}
.lp-guide ol.lp-do{counter-reset:lp;margin:0;padding:0;list-style:none}
.lp-guide ol.lp-do>li{counter-increment:lp;position:relative;padding:0 0 14px 46px;line-height:1.55}
.lp-guide ol.lp-do>li::before{content:counter(lp);position:absolute;left:0;top:-2px;width:30px;height:30px;border-radius:50%;display:grid;place-items:center;
background:var(--lp-acc);color:var(--lp-on);font:800 15px/1 system-ui,sans-serif}
.lp-side{background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:16px}
.lp-side h2{font-size:19px}
.lp-pn{display:flex;flex-wrap:wrap;justify-content:space-between;gap:10px;margin:28px 0 0;padding:14px 0 0;border-top:1px solid var(--lp-line)}
.lp-crumb{margin:0 0 14px;font-size:14px}.lp-crumb a{text-decoration:none}
.lp-logos{display:grid;grid-template-columns:repeat(auto-fill,minmax(170px,1fr));gap:12px}
.lp-logo{display:flex;flex-direction:column;align-items:center;gap:8px;background:var(--lp-card);border:1px solid var(--lp-line);border-radius:var(--lp-r);padding:16px;text-align:center;font-size:14px}
.lp-logo img{width:110px;height:110px;object-fit:contain}
.lp-facts{display:grid;grid-template-columns:max-content 1fr;gap:6px 16px;margin:0}
.lp-facts dt{font-weight:650}.lp-facts dd{margin:0;color:var(--lp-dim)}
.lp .lp-legal{margin:30px 0 0;padding-top:14px;border-top:1px solid var(--lp-line);font-size:13px;color:var(--lp-dim)}
.lp-front{margin:44px auto 0}
.lp-front .lp-head{display:flex;flex-wrap:wrap;align-items:end;justify-content:space-between;gap:10px;margin:0 0 14px}
.lp-front .lp-head h2{margin:0}
code.lp-k{background:var(--lp-card2);border:1px solid var(--lp-line);border-radius:5px;padding:1px 5px;font-size:.92em}
@media (max-width:560px){.lp-gal,.lp-eps{grid-template-columns:1fr 1fr;gap:10px}.lp-ep b{font-size:15px}.lp-steps>li{padding-left:56px}.lp-facts{grid-template-columns:1fr}.lp-facts dd{margin-bottom:6px}}
"""


def _style() -> str:
    return f"<style>{CSS}</style>"


def _shell(title: str, body: str, path: str, current: str, status: int = 200) -> bytes:
    """The page in the site's frame: the hub's (the front door on) or the Leyline look's, with its link card."""
    full = f"{title} · Leyline"
    meta_f = portal("card_meta")
    try:
        meta = meta_f(path, full) if callable(meta_f) else ""
    except Exception:   # noqa: BLE001
        meta = ""
    hf = portal("hub_frame")
    if callable(hf):
        try:
            out = hf(full, body, path, current, css=CSS, meta=meta)
        except Exception:   # noqa: BLE001
            out = None
        if out is not None:
            return out
    ly = portal("ly_shell")
    if callable(ly):
        try:
            return ly(full, body, path, current, css=CSS, meta=meta)
        except Exception:   # noqa: BLE001
            pass
    return (f'<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" '
            f'content="width=device-width,initial-scale=1"><title>{_e(full)}</title>{_style()}</head>'
            f'<body>{body}</body></html>').encode("utf-8")


# ------------------------------------------------------------------------------------------------- Single player
# TODO(owner): the Single player marketing lines, in your words. Drafts (the launcher's page says the same, single.js TITLE):
#   heading: "Your world. Your pace. Your party."
#   line:    "Just you and your bot companions, on your own PC: dungeons and raids with helper bots, levelling at your pace."
SP_HEAD = "Your world. Your pace. Your party."
SP_LINE = ("Just you and your bot companions, on your own PC: dungeons and raids with helper bots, levelling at your "
           "pace.")
SP_WHAT = [
    ("Just you and your bots", "A private world on your own PC. Nobody else is on it, nobody can find it, and every "
     "bot is labelled as a bot."),
    ("Dungeons and 10-player raids", "Fill your party to five, or a raid to ten: tanks, healers and damage that fight, "
     "heal and follow you. Dungeons and raids scale to the group you bring."),
    ("The Bots window and the bot bar", "Type /comp, or click the bots button by the minimap: pick each bot's class, "
     "spec and role, give orders, and watch their health and mana on the bot bar."),
    ("Dungeon trips", "The Dungeons tab lists the dungeons for your level. Press Go: bots fill your party and you are "
     "all taken to the door."),
    ("Levelling at your pace", "From level 1 at a gentle 2x, with a levelling buddy that keeps to your level, and a "
     "few roaming bots so the world never feels empty."),
    ("Bring your own alts", "Your other characters can come along as bots, with their own gear and talents. Save them "
     "as your usual party and bring them back in one click."),
]
SP_GALLERY = [
    ("slots/sp-bots-window.webp", "The Bots window: your party's bots with their roles, classes and specs",
     "The Bots window"),
    ("slots/sp-bot-bar.webp", "The bot bar beside the player frame, with each bot's health, mana and what it is doing",
     "The bot bar"),
    ("slots/sp-dungeon.webp", "A party of four labelled bots following the player into a dungeon", "A dungeon with bots"),
    ("slots/sp-raid.webp", "A 10-player raid of bots lined up before a boss", "A 10-player raid"),
    ("slots/sp-dungeon-trip.webp", "The Dungeons tab with its list of dungeons for the player's level and Go",
     "Dungeon trips"),
    ("slots/sp-buddy.webp", "A levelling buddy questing beside the player in a starting zone", "A levelling buddy"),
]


def single_faq() -> list[tuple[str, str]]:
    return [
        ("Do I need Docker?",
         # TODO(owner): once the built-in engine ships (single.js: "Built-in, coming soon"), drop the second sentence
         "Not with the built-in engine: then the launcher runs everything itself and there is nothing else to install. "
         "Until it is out, Single player runs on Docker Desktop (free), and the launcher's Single player page checks it "
         "for you, with Start Docker or Get Docker when it is needed."),
        ("Whose game files does it use?",
         "Your own. You add your own game folder to the launcher once, and your world is made from it, on your PC. "
         "Leyline never downloads, hosts or sends game files."),
        ("Where are my characters saved?",
         "On your PC, in the launcher's own folder. Nothing is kept on our servers, and stopping your world keeps "
         "every character as it was."),
        ("Do bots take my loot?",
         "No. Bots always pass: they never roll need or greed, your alts included. Only grey and white items go round "
         "the group in turn. Want every item? Set loot to Free for All and bots never loot."),
        # TODO(owner): confirm whether play works with no internet at all once set up (logging in to the launcher, the
        # relay invite single.js uses to get you in); then say so plainly here.
        ("Does it work offline?",
         "Setting up needs the internet: the launcher and the server pack are downloaded once. After that your world "
         "runs on this PC, not on ours."),
        ("Can friends join me?",
         "Single player is private: just you. To play with friends, invite them to Your Den or host your own realm "
         "from the launcher's Host page."),
    ]


def single_body() -> str:
    dl = _download()
    hero = (
        '<section class="lp-hero" aria-labelledby="sp-h"><div>'
        '<p class="lp-kick">Single player</p>'
        f'<!-- TODO(owner): the heading and the line under it, your words (drafts above SP_HEAD) -->'
        f'<h1 id="sp-h">{_e(SP_HEAD)}</h1><p class="lp-lede">{_e(SP_LINE)}</p>'
        f'<p class="lp-acts"><a class="lp-btn" href="{_e(dl)}"{_rel(dl)}>Get the launcher</a>'
        '<a class="lp-btn ghost" href="#start">How to start</a></p></div>'
        + video_slot("sp-trailer", ["slots/sp-poster.webp", "leyline-shot-durotar.webp"],
                     "Single player: a short film of a player and their bot party levelling, in a dungeon and in a raid",
                     "Single player, on one PC")
        + "</section>")
    what = ('<section class="lp-sec" id="what" aria-labelledby="sp-what"><h2 id="sp-what">What it is</h2>'
            '<ul class="lp-points">' + "".join(f"<li><b>{_e(t)}</b><span>{_e(s)}</span></li>" for t, s in SP_WHAT)
            + '</ul></section>')
    gal = ('<section class="lp-sec" id="companions" aria-labelledby="sp-gal"><h2 id="sp-gal">Your companions</h2>'
           '<p class="lp-dim">Pictures from our own realm.</p><div class="lp-gal">'
           + "".join(slot(n, alt, cap) for n, alt, cap in SP_GALLERY) + "</div></section>")
    start = ('<section class="lp-sec" id="start" aria-labelledby="sp-start"><h2 id="sp-start">How to start</h2>'
             '<ol class="lp-steps">'
             f'<li><h3>Get the launcher</h3><p><a href="{_e(dl)}"{_rel(dl)}>Download Leyline</a> for Windows, macOS or '
             'Linux and log in with your Hero Key.</p></li>'
             '<li><h3>Add your game folder</h3><p>Drop your own game folder on the launcher, or pick Add a game folder. '
             'It is the same folder the official realm plays with.</p></li>'
             '<li><h3>Press Start</h3><p>Open Single player in the side menu and press Start. The launcher sets up '
             'your world once, then takes you straight in.</p></li></ol>'
             f'<p class="lp-dim" style="margin-top:12px">New to the launcher? <a href="{GUIDES}/install-and-log-in">The '
             'first guide</a> walks through it.</p></section>')
    # Leyline Solo (stream/solo_page.py, /wrath/solo): the standalone app for the first era. Linked from here and the
    # docs only, not the main nav yet (TODO(owner): say when it goes in the nav)
    solo = ('<section class="lp-sec" id="solo" aria-labelledby="sp-solo"><h2 id="sp-solo">The first era: Leyline Solo</h2>'
            '<p>Want the game as it first was, on your own first-era game folder? Leyline Solo is a small app of its '
            'own: a first-era world on your PC with companions, offline, logged in by itself, your characters in one '
            'list, and a friend when you want one.</p>'
            '<p class="lp-acts"><a class="lp-btn ghost" href="/wrath/solo">See Leyline Solo</a></p></section>')
    faq = ('<section class="lp-sec lp-faq" id="faq" aria-labelledby="sp-faq"><h2 id="sp-faq">Questions</h2>'
           + "".join(f"<details><summary>{_e(q)}</summary><p>{_e(a)}</p></details>" for q, a in single_faq())
           + f'<p class="lp-dim">Something else? Ask on the <a href="{_e(_discord())}" rel="noopener noreferrer">Discord</a>.'
           '</p></section>')
    return f'<div class="lp lp-sp">{hero}{what}{gal}{start}{solo}{faq}</div>'


def single_page() -> bytes:
    return _shell("Single player", single_body(), SINGLE, "single")


# ------------------------------------------------------------------------------------------------- the Modes shelf
# Every way to play, one card each (owner 4 Nov brief: picture, one line, Play now / How it works). "flag": shown only
# with that switch on. "play": (href, label) or None (coming). Pictures: our own slot first, then a stand-in we have.
# "hidden": True keeps a card here but off the shelf, and its /wrath/modes/<id> page sends players to /wrath/modes
# (owner 12 Nov: Modes are Duels and Boss Rush only for now). Delete the line to bring it back.
def MODES() -> list[dict]:   # noqa: N802 (a function: the download link and the flag are read per request)
    dl = _download()
    return [
        {"id": "official", "title": "Wrath Unleashed", "kick": "The official realm",
         "line": "Our own realm, always on: start at level 1 or take a one-time boost to 80, in your browser or your "
                 "own game.",
         "pic": ["slots/mode-official.webp", "leyline-shot-dalaran.webp"], "alt": "Dalaran on Wrath Unleashed",
         "play": ("/wrath", "Play now"), "guide": "official-realm",
         "how": [
             "Play now opens the game in this browser: log in and you are in. No game files needed for the streamed "
             "part of the world (Dalaran, the capitals, starting areas, dungeons and raids).",
             "Or play the whole world with your own game: add your game folder in the launcher and press PLAY. The "
             "launcher points your game at the realm and keeps your own settings as a backup.",
             "A new character picks once: begin at level 1, or a one-time boost to 80 with a spec kit, professions and "
             "a trip to Dalaran.",
             "Type .wu in the game for every realm command."]},
        {"id": "community", "title": "Community realms", "kick": "Hosted by players",
         "line": "Realms players run on their own PCs, each with its own rules. Ranked by players online and uptime, "
                 "never by money.",
         "pic": ["slots/mode-community.webp", "leyline-ui-realms.webp"],
         "alt": "The launcher's Realms page: every realm with its players, uptime and badges",
         "play": ("/wrath/realms", "See the realms"), "guide": "community-realm",
         "how": [
             "Open Realms in the launcher (or on this site) and pick the Community tab.",
             "Press PLAY on a realm: the launcher picks one of your game folders of the right kind and starts it.",
             "Got an invite link from a friend? Paste it under Realms, Got an invite link?, and press Join.",
             "Each realm makes your account on its own join page, in your browser: your password never leaves it."]},
        {"id": "your-den", "title": "Your Den", "kick": "Build on your PC",
         "line": "Your own private world on your PC: build, decorate, practise in the Dojo, then invite friends.",
         "pic": ["slots/mode-den.webp", "leyline-shot-den-1.webp", "leyline-ui-launcher.webp"],
         "alt": "A room built in Your Den",
         "play": (dl, "Get the launcher"), "guide": "your-den",
         "how": [
             "In the launcher, open Your Den and press Play solo. The first time it downloads the small Den program; "
             "after that it needs no internet.",
             "Build (F7) places anything from the catalog: search, then move, turn or delete it. Undo takes the last "
             "change back.",
             "Play (F8) hides the tools so you can just walk your Den. It saves as you go.",
             "Invite makes one link for one friend: they open it in their launcher and join you."]},
        {"id": "single-player", "title": "Single player", "kick": "Just you and your bots", "flag": FLAG_ENV,
         "line": "A private world on your PC with bot companions: dungeons and raids with helper bots, at your pace.",
         "pic": ["slots/sp-poster.webp", "leyline-shot-durotar.webp"], "alt": "A player and their bot party",
         "play": (SINGLE, "See Single player"), "guide": "",
         "how": [
             "Get the launcher and add your own game folder.",
             "Open Single player in the side menu and press Start: your world is set up once, then you are in.",
             "Type /comp for the Bots window: fill your party or a 10-player raid, pick specs, give orders.",
             "The Dungeons tab takes you and your bots to the door of a dungeon for your level."]},
        {"id": "azeroth-survivors", "title": "Azeroth Survivors", "kick": "In your browser",
         "line": "A survivors-style roguelike: pick a hero and hold out against the waves. Runs in your browser.",
         "pic": ["leyline-mode-azeroth-survivors.webp"], "alt": "Azeroth Survivors key art", "w": 483, "h": 298,
         "play": ("/wrath/survivors", "Play now"), "guide": "survivors", "hidden": True,
         "how": [
             "Open Azeroth Survivors and log in. It needs a browser with WebGPU: Chrome or Edge on a computer.",
             "Pick a hero and a zone. Your spells cast themselves: you move, level up and pick upgrades.",
             "WASD moves, 1, 2 or 3 picks a card when you level up, Esc pauses.",
             "An open rebuild by its authors, credited on its page."]},
        {"id": "golf-and-dojo", "title": "Minigolf and the Dojo", "kick": "Games and practice",
         "line": "Tee off on courses players built, right on our realm, or train in the Dojo in Your Den.",
         "pic": ["slots/mode-golf.webp", "leyline-shot-minigolf.webp"],
         "alt": "Minigolf on our own realm: a fairway toward the flag in a red canyon",
         "play": ("/wrath?go=mode:minigolf", "Play Minigolf"), "guide": "", "hidden": True,
         "how": [
             "Play Minigolf logs you in and takes your character to the course once you are in the world.",
             "In the game, /golf opens it: Play here starts a game where you stand and invites your party; Practice "
             "plays alone anywhere.",
             "In Your Den, the Dojo button (or /dojo) opens the training ring: dummies to practise on, fly and speed "
             "for building.",
             "Courses are small text files anyone can write: see the Docs."]},
        {"id": "seeded-realm", "title": "The seeded realm", "kick": "Coming",
         "line": "A first-era realm the community keeps alive together: player nodes carry it, so no single place "
                 "holds it.",
         "pic": ["slots/mode-seeded.webp"], "alt": "The seeded realm",
         "play": None, "guide": "be-a-node", "coming": True,
         "how": [
             "The plan: a first-era realm from level 1, with its tuning, features and approved addons, played with "
             "your own game folder.",
             "One host runs the world at a time, while trusted keepers hold a live copy, so it can move to another "
             "PC in minutes.",
             "Players who help carry the network keep it reachable from anywhere.",
             "It is not open yet. News comes first on the Discord."]},
    ]


def modes() -> list[dict]:
    """MODES with the unannounced ones left out while their switch is off, and the hidden ones left out."""
    return [m for m in MODES() if (not m.get("flag") or single_public()) and not m.get("hidden")]


def hidden_ids() -> list[str]:
    """The shelf's hidden cards (their pages send players to /wrath/modes)."""
    return [m["id"] for m in MODES() if m.get("hidden")]


def mode_by_id(ident: str) -> dict | None:
    return next((m for m in modes() if m["id"] == ident), None)


def _mode_card(m: dict) -> str:
    play = m.get("play")
    acts = (f'<a class="lp-btn" href="{_e(play[0])}"{_rel(play[0])}>{_e(play[1])}</a>' if play else "")
    acts += f'<a class="lp-btn ghost" href="{MODES_PATH}/{_e(m["id"])}">How it works</a>'
    tag = '<span class="lp-tag soon">Coming</span>' if m.get("coming") else f'<span class="lp-tag">{_e(m["kick"])}</span>'
    # way-<id>: /wrath/modes also has the Games cards (playmodes.py, id="mode-<id>"); one id per element on the page
    return (f'<article class="lp-mode" id="way-{_e(m["id"])}">'
            + slot(m["pic"], m["alt"], m["title"], m.get("w", 1280), m.get("h", 720))
            + f'{tag}<h3>{_e(m["title"])}</h3><p>{_e(m["line"])}</p><p class="lp-acts">{acts}</p></article>')


def shelf(heading: str = "Ways to play", hid: str = "lp-shelf-h") -> str:
    """The Modes shelf: one card per way to play."""
    try:
        cards = "".join(_mode_card(m) for m in modes())
    except Exception:   # noqa: BLE001
        return ""
    return (f'{_style()}<section class="lp" id="shelf" aria-labelledby="{hid}"><h2 id="{hid}">{_e(heading)}</h2>'
            f'<div class="lp-shelf">{cards}</div></section>')


def mode_page(ident: str) -> bytes | None:
    m = mode_by_id(ident)
    if m is None:
        return None
    play = m.get("play")
    acts = (f'<a class="lp-btn" href="{_e(play[0])}"{_rel(play[0])}>{_e(play[1])}</a>' if play
            else '<span class="lp-tag soon">Coming</span>')
    g = guide_by_slug(m.get("guide") or "")
    if g:
        acts += f'<a class="lp-btn ghost" href="{GUIDES}/{_e(g["slug"])}">Guide: {_e(g["title"])}</a>'
    others = [x for x in modes() if x["id"] != m["id"]]
    body = (f'<div class="lp lp-modepg"><p class="lp-crumb"><a href="{MODES_PATH}">All modes</a> ›</p>'
            '<section class="lp-hero"><div>'
            f'<p class="lp-kick">{_e(m["kick"])}</p><h1>{_e(m["title"])}</h1><p class="lp-lede">{_e(m["line"])}</p>'
            f'<p class="lp-acts">{acts}</p></div>'
            + slot(m["pic"], m["alt"], m["title"], m.get("w", 1280), m.get("h", 720)) + '</section>'
            '<section class="lp-sec" aria-labelledby="md-how"><h2 id="md-how">How it works</h2>'
            '<ol class="lp-steps">' + "".join(f"<li><p>{_e(s)}</p></li>" for s in m["how"]) + '</ol></section>'
            '<section class="lp-sec" aria-labelledby="md-more"><h2 id="md-more">More ways to play</h2><div class="lp-shelf">'
            + "".join(_mode_card(x) for x in others[:3]) + '</div>'
            f'<p class="lp-acts"><a class="lp-btn ghost" href="{MODES_PATH}">Every mode</a></p></section></div>')
    return _shell(m["title"], body, f"{MODES_PATH}/{m['id']}", "modes")


# ------------------------------------------------------------------------------------------------- guides
# The 12-episode guide series. Each episode: short steps from the launcher and the docs as they are today (docs/leyline/,
# launcher/ui: the words on its buttons), a Stuck? list, and a video slot (slots/guide-<slug>.webm, poster .webp).
def GUIDE_LIST() -> list[dict]:   # noqa: N802 (a function: the download link is read per request)
    dl = _download()
    return [
        {"slug": "install-and-log-in", "title": "Install Leyline and log in with a Hero Key",
         "line": "Get the launcher, make your free Hero Key and log in.",
         "steps": [
             f"Open the download page (<a href=\"{_e(dl)}\"{_rel(dl)}>Download</a>) and get Leyline for your PC. On "
             "Windows it is one file, Leyline.exe, with nothing to install.",
             "Windows may warn that the app is new: choose More info, then Run anyway. On a Mac, right-click the app "
             "and choose Open.",
             "The first screen is Log in to Leyline. New here? Create a guest account there, or make a Hero Key on "
             "this site: <a href=\"/wrath/key\">Get your Hero Key</a>.",
             "Your Hero Key is shown once: press Download to keep it as a file (Leyline Hero Key.txt), or Copy.",
             "Back in the launcher, log in with your Hero Key file (drop it on the window or choose it), paste the "
             "key, or log in with your browser or Google.",
             "Logged in: the launcher remembers you, even offline next time."],
         "stuck": [
             "The website isn't answering? Press Continue without logging in and try again later.",
             "Lost your Hero Key? Log in on the site another way and make a new key at /wrath/key: the old one stops "
             "working at once.",
             "Treat your key like a password: anyone who has it can log in as you."],
         "docs": [("Hero Key", "/wrath/docs/hero-key"), ("The launcher", "/wrath/docs/launcher")]},
        {"slug": "add-your-game-folder", "title": "Add your game folder",
         "line": "Show the launcher the game you already have.",
         "steps": [
             "Find your game folder: the one that holds the game's program and a Data folder. Look in Program Files, "
             "C:\\Games, your Desktop or Downloads, or an old drive or backup.",
             "In the launcher, press PLAY, or open the play bar's client list and pick Add a game folder.",
             "Choose the folder, or drop it on the client list. A zip of your own folder works too: a bar shows the "
             "unpacking.",
             "The launcher reads which version it is and names it for you under Game folders.",
             "Got more than one? Add each: the launcher picks the right one for every realm."],
         "stuck": [
             "\"Not a game folder\": choose the folder that holds the program and Data, not a folder inside it.",
             "No files at all? You can still play in your browser: see the guide on browser and phone.",
             "Leyline never downloads the game: ask on the Discord and people help you find or restore your folder."],
         "docs": [("Getting started", "/wrath/docs/getting-started")]},
        {"slug": "official-realm", "title": "Play the Official realm",
         "line": "Into Wrath Unleashed, our own realm, in one press.",
         "steps": [
             "In the launcher, Wrath Unleashed is picked in the play bar for you.",
             "Press PLAY. If you are not logged in yet, the launcher asks first, then goes on by itself.",
             "The launcher points your game at the realm (your own settings are kept as a backup) and starts it, "
             "through to character select.",
             "Make a character and pick once: begin at level 1, or a one-time boost to 80.",
             "In the game, type .wu for every realm command."],
         "stuck": [
             "The game can't connect? Check the status dot in the launcher's title bar, then delete the Cache folder "
             "next to the game and start again.",
             "Need a different game? The play bar says which one the realm needs.",
             "Changed the realm by hand before? Realms, then …, then Use my own realmlist puts back what you had."],
         "docs": [("Play", "/wrath/docs/play"), ("Wrath Unleashed", "/wrath/docs/wrath-unleashed")]},
        {"slug": "community-realm", "title": "Join a Community realm",
         "line": "Find a realm players host and join it.",
         "steps": [
             "Open Realms in the launcher and pick the Community tab, or see every realm on this site.",
             "Each realm shows its rules, players online and uptime. Bots are always counted apart.",
             "Press PLAY on a realm: the launcher picks one of your game folders of the right kind.",
             "The first time, make your account on the realm's join page in your browser. Your password never "
             "leaves it.",
             "Got an invite link for a private realm? Paste it under Realms, Got an invite link?, and press Join.",
             "Switching back is one press: pick Wrath Unleashed again and press PLAY."],
         "stuck": [
             "An invite lasts 24 hours: ask your friend for a new one.",
             "Open a private realm's link on the same PC and connection you play from.",
             "The launcher says a game folder is missing? Add one of the kind the realm says."],
         "docs": [("Play: community realms", "/wrath/docs/play#community-realms"), ("The network", "/wrath/docs/network")]},
        {"slug": "your-den", "title": "Open Your Den and build",
         "line": "Your own white world on your PC, to build in.",
         "steps": [
             "In the launcher, open Your Den and press Play solo. The first time, it downloads the small Den program.",
             "You arrive in your Den. The bar at the top of the screen has everything.",
             "Build (F7): search the catalog, place a piece, then move, turn or delete it. Undo takes the last change "
             "back.",
             "In chat, <code class=\"lp-k\">.den place chair</code> puts a chair in front of you, and "
             "<code class=\"lp-k\">.den stamp inn now</code> a whole inn.",
             "Play (F8) hides the tools so you just walk around. F8 brings them back.",
             "Your Den saves itself as you go. Save keeps a copy under a name; Saved Dens in the launcher opens it."],
         "stuck": [
             "Need more room? <code class=\"lp-k\">.den grow</code>.",
             "Type <code class=\"lp-k\">.den</code> in your Den for every command.",
             "Nothing happens on Play solo? Press it once more after the Den program finished downloading."],
         "docs": [("Your Den", "/wrath/docs/den")]},
        {"slug": "invite-friends", "title": "Invite friends",
         "line": "One link for one friend, into your Den.",
         "steps": [
             "Your friend needs Leyline, their own game folder and a Hero Key. That's all.",
             "Open Your Den in the launcher and press Invite friends. The link is copied.",
             "Send it to one friend: a message, the Discord, an email. Each friend gets their own link.",
             "They open it and press Open in Leyline, or paste it under Realms, Got an invite link?, and Join.",
             "Their Leyline tries the ways in by itself: the same network, your own address, then the Leyline relay.",
             "In your Den only you build, unless you type <code class=\"lp-k\">.den friends build on</code>."],
         "stuck": [
             "A link stops working after 24 hours or when you press Stop sharing: send a new one.",
             "Same network says off? Allow Den Lite in your firewall for private networks, then invite again.",
             "Game files never travel: each of you plays with your own folder."],
         "docs": [("Invite a friend", "/wrath/docs/den-invite")]},
        {"slug": "host-your-realm", "title": "Host your own realm",
         "line": "Your realm on your PC, on the realm list in minutes.",
         "steps": [
             "Open Host in the launcher. The line at the top says if this PC is ready.",
             "Get ready: Get the Host pack, and Docker Desktop (free), one click each.",
             "Pick a template (Classic levelling, Fun realm, Hardcore and more) and press Host, or Fork to make it "
             "yours.",
             "Choose Private (friends with your link) or Public (on the Community list), a name, and your account "
             "name, which gets GM.",
             "Press Start your realm. No ports to open: your realm connects out to the Leyline relay.",
             "Send your invite link. The Host page is now your realm's console."],
         "stuck": [
             "You need about 10 GB free and your own game folder: the server reads what it needs from it, on your PC.",
             "Docker not running? The Host page has Start Docker.",
             "The realm runs only while your PC does."],
         "docs": [("Host a realm", "/wrath/docs/host-a-realm"), ("Hosting", "/wrath/docs/hosting")]},
        {"slug": "be-a-node", "title": "Be a node",
         "line": "Help carry the network from your own PC.",
         "steps": [
             "Open Host, then the Network tab, then Help carry the network.",
             "Type your PC's public name (a DNS name that points at your PC).",
             "Set your caps: upload in KB/s, and the most players at once.",
             "Leave Only while this PC is idle and plugged in on if you use this PC.",
             "Press Start helping. The line above the caps says how many players your PC carries now.",
             "Turn off stops it in one click."],
         "stuck": [
             "It says Not open yet until it opens for everyone at once.",
             "Carrying for others needs open ports: TCP 80, 443 and 41000 to 41051. Hosting a realm needs none.",
             "Your PC passes connections along only: it never stores or shares game files."],
         "docs": [("Be a node", "/wrath/docs/be-a-node")]},
        {"slug": "mods-and-looks", "title": "Mods and looks",
         "line": "Addons, graphics packs and presets, in a click.",
         "steps": [
             "Open Mods & Addons, then Discover. Search, or browse by category.",
             "Pick the game folder to install into, then press Install. Each mod goes into that one folder only.",
             "Looks has graphics packs, new interfaces and one-click presets, credited to their makers.",
             "Installed shows what each folder has: update, turn off and on, or remove in one click.",
             "Save the set you like as a loadout, and go back to Clean (the game as installed) any time."],
         "stuck": [
             "A mod made for another kind of game folder is flagged and can't be installed.",
             "Removing a mod removes exactly the files Leyline put in, nothing else.",
             "Only install mods you trust: nobody else decides for you."],
         "docs": [("Mods", "/wrath/docs/mods"), ("Looks", "/wrath/looks")]},
        {"slug": "survivors", "title": "Azeroth Survivors in the browser",
         "line": "A roguelike that runs in a browser tab.",
         "steps": [
             "Open <a href=\"/wrath/survivors\">Azeroth Survivors</a> on this site.",
             "Press Play and log in. It needs a browser with WebGPU: Chrome or Edge on a computer.",
             "Pick a hero and a zone. Your spells cast themselves: you move, level up and pick upgrades.",
             "WASD moves, F is your class's movement spell, 1, 2 or 3 picks a card, Esc pauses.",
             "A boss turns up every few minutes. Hold out as long as you can."],
         "stuck": [
             "A blank page? Your browser may not have WebGPU: try a recent Chrome or Edge on a computer.",
             "It says Resting: the game is taking a short break. Check back soon."],
         "docs": [("Azeroth Survivors", "/wrath/survivors")]},
        {"slug": "browser-and-phone", "title": "Play in the browser and on a phone",
         "line": "No download, no game files: straight into the game.",
         "steps": [
             "Open this site and press <a href=\"/wrath\">Play now</a>, then log in.",
             "The game opens in the tab. The browser streams Dalaran, the capitals, the starting areas, dungeons and "
             "raids.",
             "On a phone or tablet, touch controls switch on by themselves: the stick bottom-left moves you, the "
             "buttons on the right attack and cast.",
             "Turn your phone sideways for the full view.",
             "Have your own game folder? Drag it onto the game page on a computer to go anywhere in the world."],
         "stuck": [
             "It needs a browser with WebGPU, such as a recent Chrome or Edge.",
             "Two game tabs open? Only one plays at a time: the other offers Play here.",
             "Before playing the same account with your own game, press End my browser pass on the account page."],
         "docs": [("Play", "/wrath/docs/play")]},
        # TODO(owner): Leyline Control itself is your own control panel (tools/control: SSH and GitHub, yours alone
        # today, docs/TOOLKIT.md 13). This episode shows hosts their own controls: the Host page's console and the Host
        # pack's tools. Rename it, or say when a host's Control ships.
        {"slug": "control-for-hosts", "title": "Leyline Control for hosts",
         "line": "Run your realm day to day: start, stop, check and fix.",
         "steps": [
             "Once your realm runs, the launcher's Host page is its console: its state, Start and Stop, and its "
             "status from the relay (online, on the list, players and bots).",
             "The GM controls, a health check, and restarts with a warning to players are built in.",
             "In the Host pack folder, Check my realm tests it the way a player would and says what it found in plain "
             "words.",
             "Realm tools has the small jobs: your invite link, a new invite, an account for a friend, public or "
             "private, who may make accounts, the logs and the most players at once.",
             "Stop my realm stops it. Characters, accounts and your invite link are kept."],
         "stuck": [
             "Check my realm first: it names the part that failed.",
             "Docker Desktop must be running for every tool.",
             "Post what Check my realm said on the Discord: other hosts help."],
         "docs": [("Hosting", "/wrath/docs/hosting"), ("Always on", "/wrath/docs/always-on")]},
    ]


def guides() -> list[dict]:
    out = GUIDE_LIST()
    for n, g in enumerate(out, 1):
        g["n"] = n
    return out


def guide_by_slug(slug: str) -> dict | None:
    return next((g for g in guides() if g["slug"] == slug), None)


def _poster(g: dict) -> list[str]:
    return [f"slots/guide-{g['slug']}.webp"]


def guides_body() -> str:
    cards = "".join(
        f'<li><a class="lp-ep" href="{GUIDES}/{_e(g["slug"])}">'
        + slot(_poster(g), f"Episode {g['n']}: {g['title']}", g["title"]).replace("Picture coming soon", "Video coming soon")
        + f'<span class="lp-n">Episode {g["n"]}</span><b>{_e(g["title"])}</b><span>{_e(g["line"])}</span></a></li>'
        for g in guides())
    # TODO(owner): the guide series' opening line, your words (draft below)
    return ('<div class="lp lp-guides"><p class="lp-kick">Guides</p><h1>Leyline, step by step</h1>'
            '<p class="lp-lede">Twelve short guides, from your first log in to running your own realm. Each has a video '
            'and the same steps written down.</p>'
            f'<ol class="lp-eps" style="margin-top:18px">{cards}</ol>'
            f'<p class="lp-dim" style="margin-top:18px">Stuck anywhere? Ask on the <a href="{_e(_discord())}" '
            'rel="noopener noreferrer">Discord</a>, or read the <a href="/wrath/docs">Docs</a>.</p></div>')


def guide_body(g: dict) -> str:
    all_g = guides()
    i = g["n"] - 1
    prev = all_g[i - 1] if i > 0 else None
    nxt = all_g[i + 1] if i + 1 < len(all_g) else None
    # steps carry a little markup of ours (links, code): every step is our own text, never a visitor's
    steps = "".join(f"<li>{s}</li>" for s in g["steps"])
    stuck = "".join(f"<li>{s}</li>" for s in g["stuck"])
    docs = "".join(f'<li><a href="{_e(h)}">{_e(t)}</a></li>' for t, h in g.get("docs", []))
    pn = (f'<nav class="lp-pn" aria-label="Episodes">'
          + (f'<a href="{GUIDES}/{_e(prev["slug"])}">‹ Episode {prev["n"]}: {_e(prev["title"])}</a>' if prev else "<span></span>")
          + (f'<a href="{GUIDES}/{_e(nxt["slug"])}">Episode {nxt["n"]}: {_e(nxt["title"])} ›</a>' if nxt else "")
          + "</nav>")
    return (f'<div class="lp lp-guide"><p class="lp-crumb"><a href="{GUIDES}">Guides</a> › Episode {g["n"]}</p>'
            f'<p class="lp-kick">Episode {g["n"]} of {len(all_g)}</p><h1>{_e(g["title"])}</h1>'
            f'<p class="lp-lede">{_e(g["line"])}</p>'
            '<div class="lp-two" style="margin-top:16px"><div>'
            + video_slot(f"guide-{g['slug']}", _poster(g), f"Video guide: {g['title']}", g["title"])
            + f'<section class="lp-sec" aria-labelledby="gd-steps"><h2 id="gd-steps">Steps</h2><ol class="lp-do">{steps}</ol></section>'
            '</div><aside class="lp-side" aria-labelledby="gd-stuck"><h2 id="gd-stuck">Stuck?</h2>'
            f'<ul class="lp-list">{stuck}</ul>'
            f'<p>Still stuck? Ask on the <a href="{_e(_discord())}" rel="noopener noreferrer">Discord</a>: people there '
            'help.</p>'
            + (f'<h2 style="margin-top:16px">Read more</h2><ul class="lp-list">{docs}</ul>' if docs else "")
            + f'</aside></div>{pn}</div>')


def guide_page(slug: str) -> bytes | None:
    g = guide_by_slug(slug)
    if g is None:
        return None
    return _shell(g["title"], guide_body(g), f"{GUIDES}/{g['slug']}", "guides")


# ------------------------------------------------------------------------------------------------- press kit
# TODO(owner): the paragraph about Leyline for the press, your words. Draft:
PRESS_ABOUT = ("Leyline is a free, open-source launcher and network for classic-era online roleplaying games. Players "
               "bring the game folder they already own; Leyline finds it, logs them in once with a Hero Key, and takes "
               "them to the official realm, to realms other players host on their own PCs, or to a private world of "
               "their own to build in. There is no shop and nothing to buy. Leyline never hosts or ships game files.")
PRESS_LOGOS = [("leyline-mark-512.png", "The Leyline mark", "The mark, 512 px"),
               ("leyline-swirl-512.png", "The Leyline swirl", "The swirl, 512 px"),
               ("leyline-crystal-512.png", "The Leyline crystal", "The crystal, 512 px")]
PRESS_SHOTS = [("leyline-ui-launcher.webp", "The Leyline launcher's Home with its realm list open", "The launcher"),
               ("leyline-ui-realms.webp", "The launcher's Realms page", "Realms"),
               ("leyline-ui-hosting.webp", "The launcher's Host page with its templates", "Host your own"),
               ("leyline-ui-workshop.webp", "The launcher's Discover page for mods", "Mods"),
               ("leyline-ui-modes.webp", "The launcher's Modes page", "Modes"),
               ("slots/press-game-1.webp", "A screenshot from our own realm", "In the game")]


def press_body() -> str:
    logos = "".join(
        (f'<div class="lp-logo"><img src="/wrath/art/{_e(n)}" alt="{_e(alt)}" width="110" height="110" loading="lazy">'
         f'<a href="/wrath/art/{_e(n)}" download>{_e(cap)}</a></div>') if _pic_ok(n) else ""
        for n, alt, cap in PRESS_LOGOS)
    shots = "".join(
        slot(n, alt, cap, 1240, 760).replace("</figcaption>", f' · <a href="/wrath/art/{_e(n)}" download>Full size</a></figcaption>', 1)
        if _pic_ok(n) else slot(n, alt, cap, 1240, 760)
        for n, alt, cap in PRESS_SHOTS)
    repo = portal("LEYLINE_REPO")
    repo = repo if isinstance(repo, str) and repo.startswith("https://github.com/") else "https://github.com/orionopenwow/Leyline"
    return ('<div class="lp lp-press"><p class="lp-kick">Press</p><h1>Press kit</h1>'
            '<p class="lp-lede">Everything you need to write about Leyline: the mark, screenshots and the facts.</p>'
            '<section class="lp-sec" aria-labelledby="pr-about"><h2 id="pr-about">About Leyline</h2>'
            '<!-- TODO(owner): the press paragraph, your words (draft in PRESS_ABOUT) -->'
            f'<p class="lp-lede">{_e(PRESS_ABOUT)}</p>'
            '<dl class="lp-facts"><dt>Name</dt><dd>Leyline (the app: Leyline Launcher)</dd>'
            '<dt>Price</dt><dd>Free. No shop, nothing to buy.</dd>'
            '<dt>Runs on</dt><dd>Windows, macOS and Linux, and in the browser</dd>'
            f'<dt>Source</dt><dd>Open source, <a href="{_e(repo)}" rel="noopener noreferrer">on GitHub</a></dd>'
            f'<dt>Contact</dt><dd>The <a href="{_e(_discord())}" rel="noopener noreferrer">Leyline Discord</a></dd></dl></section>'
            + (f'<section class="lp-sec" aria-labelledby="pr-logo"><h2 id="pr-logo">The mark</h2><div class="lp-logos">{logos}</div>'
               '<p class="lp-dim" style="margin-top:10px">Please keep the mark as it is: no new colours, no stretching.</p></section>'
               if logos else "")
            + f'<section class="lp-sec" aria-labelledby="pr-shots"><h2 id="pr-shots">Screenshots</h2><div class="lp-gal">{shots}</div></section>'
            f'<p class="lp-legal">{_e(LEGAL)} Game names belong to their owners.</p></div>')


def press_page() -> bytes:
    return _shell("Press kit", press_body(), PRESS, "press")


# ------------------------------------------------------------------------------------------------- the front page
def front_single() -> str:
    """The front page's Single player section (flag on only)."""
    if not single_public():
        return ""
    pts = "".join(f"<li><b>{_e(t)}</b><span>{_e(s)}</span></li>" for t, s in SP_WHAT[:3])
    return (f'{_style()}<section class="lp lp-front" id="single-player" aria-labelledby="lp-sp-h">'
            '<div class="lp-hero"><div><p class="lp-kick">Single player</p>'
            '<!-- TODO(owner): the front page\'s Single player heading, your words -->'
            f'<h2 id="lp-sp-h">{_e(SP_HEAD)}</h2><p class="lp-lede">{_e(SP_LINE)}</p>'
            f'<p class="lp-acts"><a class="lp-btn" href="{SINGLE}">See Single player</a>'
            f'<a class="lp-btn ghost" href="{SINGLE}#start">How to start</a></p></div>'
            + slot(["slots/sp-poster.webp", "leyline-shot-durotar.webp"], "A player and their bot party", "Single player")
            + f'</div><ul class="lp-points" style="margin-top:16px">{pts}</ul></section>')


def front_modes() -> str:
    """The front page's Ways to play: the shelf, with a link to the whole Modes page."""
    s = shelf("Ways to play", "lp-front-modes-h")
    if not s:
        return ""
    return s.replace('<section class="lp" id="shelf"', '<section class="lp lp-front" id="ways"', 1).replace(
        '<h2 id="lp-front-modes-h">Ways to play</h2>',
        '<div class="lp-head"><h2 id="lp-front-modes-h">Ways to play</h2>'
        f'<a class="lp-btn ghost" href="{MODES_PATH}">Every mode</a></div>', 1)


def front_sections() -> str:
    try:
        return front_modes() + front_single()
    except Exception:   # noqa: BLE001: the front page goes out without them
        return ""


# ------------------------------------------------------------------------------------------------- routes
SLUG_RE = re.compile(r"[a-z0-9](?:[a-z0-9-]{0,40}[a-z0-9])?")


def match(path: str) -> bool:
    p = (path or "").rstrip("/")
    if p in (GUIDES, PRESS) or (p == SINGLE and single_public()):
        return True
    if p.startswith(GUIDES + "/"):
        return bool(SLUG_RE.fullmatch(p[len(GUIDES) + 1:]))
    if p.startswith(MODES_PATH + "/"):
        ident = p[len(MODES_PATH) + 1:]
        return mode_by_id(ident) is not None or ident in hidden_ids()
    return False


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
    p = path.rstrip("/")
    body = None
    if p.startswith(MODES_PATH + "/") and p[len(MODES_PATH) + 1:] in hidden_ids():
        h.redirect(MODES_PATH)   # a hidden way to play: the Modes page (Duels and Boss Rush)
        return
    if p == SINGLE:
        body = single_page()
    elif p == GUIDES:
        body = _shell("Guides", guides_body(), GUIDES, "guides")
    elif p == PRESS:
        body = press_page()
    elif p.startswith(GUIDES + "/"):
        body = guide_page(p[len(GUIDES) + 1:])
    elif p.startswith(MODES_PATH + "/"):
        body = mode_page(p[len(MODES_PATH) + 1:])
    if body is None:
        nf = portal("not_found_page")
        h.send(404, nf() if callable(nf) else b"not found\n")
        return
    h.send(200, body)
