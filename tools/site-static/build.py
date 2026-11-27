#!/usr/bin/env python3
"""Leyline's website as plain files, for GitHub Pages (docs/STATIC-SITE.md).

    python3 tools/site-static/build.py --out _site          build the site into _site/
    python3 tools/site-static/build.py --out _site --list   the same, then print every file and its size

The old server is gone. This runs the existing portal (stream/wrath_portal.py and the modules beside it) inside this
process, on 127.0.0.1 only, with empty data folders and no network (any connection off this machine is refused), and
saves its read-only pages at the same URL paths, so launchers already installed keep working against static files:
/wrath/mods/all.json stays /wrath/mods/all.json, /wrath/docs/mods becomes /wrath/docs/mods/index.html, and so on.

What it writes:
  * the feeds the launcher reads: flags.json (every server feature off), news.json, mods.json, catalog.json,
    all.json, modes.json, realms.json (from site/realms.json), status.json, the boards index;
  * the pages: the front page, Start here, Realms, Download, Mods, Addons, Looks, each mod's page, Creators, News,
    the docs and their search, robots.txt, sitemap.xml, 404.html, CNAME;
  * the art those pages use, and the Looks pictures the launcher shows.
The front page, Start here, Realms and the 404 are drawn here (the portal's versions are about the server that is
gone); every other page is the portal's, in the same frame, with every link to a page that no longer exists taken
out (sign-in, forum, social, the browser game, leaderboards, the Den market, admin, submit forms, the queue).

Same inputs, same bytes out (no dates from the clock: --stamp or SOURCE_DATE_EPOCH, else the newest input file).
Exit 0 when the site was built, 1 when an input was refused (the message says which), 2 for usage.
Standard library only.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import html
import http.client
import importlib.util
import json
import os
import re
import shutil
import socket
import sys
import tempfile
import threading
import time
import urllib.parse
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
STREAM = REPO / "stream"
SITE = REPO / "site"
DOMAIN = "leylinelauncher.com"
ORIGIN = "https://" + DOMAIN
OLD_HOSTS = ("leylinelauncher.com", "www.leylinelauncher.com", "play.openvanilla.app", "openvanilla.app")
LEGAL = "not affiliated with or endorsed by Blizzard Entertainment"
DISCORD = "https://discord.gg/RfChVr8J7T"
REPO_URL = "https://github.com/orionopenwow/Leyline"
RELEASES = REPO_URL + "/releases"

BANNED = re.compile(r"(?i)world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard")


class Refused(Exception):
    pass


# --------------------------------------------------------------------------------------------- what the site has
# Pages taken from the portal: these paths and everything under them (each link found is followed).
PORTAL_PAGES = ("/wrath/leyline/catalog", "/wrath/leyline/addons", "/wrath/leyline/p/", "/wrath/leyline/creators",
                "/wrath/leyline/news", "/wrath/leyline/agents", "/wrath/looks", "/wrath/download", "/wrath/docs")
# Pages drawn here.
OWN_PAGES = ("/", "/wrath/leyline", "/wrath/start", "/wrath/leyline/realms")
# Old addresses that now live elsewhere (a small page that moves on, for links people kept).
MOVED = {"/wrath/realms": "/wrath/leyline/realms", "/wrath/mods": "/wrath/leyline/catalog",
         "/wrath/workshop": "/wrath/leyline/catalog", "/wrath/leyline-preview": "/wrath/leyline",
         "/wrath/leyline-preview/catalog": "/wrath/leyline/catalog", "/wrath/leyline-preview/addons": "/wrath/leyline/addons",
         "/wrath/leyline-preview/realms": "/wrath/leyline/realms", "/wrath/leyline-preview/news": "/wrath/leyline/news",
         "/wrath/leyline-preview/creators": "/wrath/leyline/creators", "/wrath/home": "/wrath/leyline"}
# Pages that closed with the server: a small page that moves on to the front page (it says what closed), for
# bookmarks and the launcher's old buttons. Links to them in pages are still taken out.
CLOSED = {p: "/wrath/leyline" for p in ("/wrath", "/wrath/play", "/wrath/account", "/wrath/signin", "/wrath/key",
                                        "/wrath/profile", "/login", "/key", "/account/key", "/wrath/link",
                                        "/wrath/leaderboards", "/wrath/bossfights", "/wrath/arena", "/wrath/survivors",
                                        "/wrath/dens", "/wrath/mobile", "/wrath/modes", "/wrath/host")}
# Pictures the site never carries: not ours to publish (tools/leyline-repo/export.py PRIVATE_ART).
PRIVATE_ART = {"leyline-shot-backdrop.webp"}
# Old pages whose job moved off the site: links to them go there instead.
ELSEWHERE = {"/wrath/leyline/submit": REPO_URL + "/issues/new?template=submit-mod.yml",
             "/wrath/submit": REPO_URL + "/issues/new?template=submit-mod.yml",
             "/wrath/leyline/forum": DISCORD, "/wrath/forum": DISCORD, "/wrath/bugs": REPO_URL + "/issues"}
# Words on the portal's pages that described the server (phrase, what it says now).
PHRASES = (
    ("What changed in the launcher, the realms and the browser game.", "What changed in Leyline."),
    (" The browser game already uses it.", ""),
    ("Wrath Unleashed, in game", "In game"),
    ("Wrath Unleashed: Wrath on the Leyline network", "The Leyline mark"),
)
# Docs pages that still mention what closed get this note under their heading.
DOCS_NOTE = ('<p class="st-docnote" role="note"><b>Leyline runs no server of its own now.</b> Parts of this page '
             'that needed it (sign-in, the relay, the official realm, the public realm list, the forum) no longer '
             'work. <a href="/wrath/docs">What still works</a>.</p>')
DOCS_NOTE_WORDS = re.compile(r"(?i)\brelay\b|hero key|official realm|wrath unleashed|in your browser|the forum|"
                             r"leylinelauncher\.com/wrath\b|log in at|be a node|\bbeacon")
# The feeds launchers read (path -> how it is made). Every one is a plain file.
FEEDS = ("/wrath/leyline/flags.json", "/wrath/leyline/news.json", "/wrath/leyline/mods.json",
         "/wrath/leyline/catalog.json", "/wrath/mods/all.json", "/wrath/modes.json", "/wrath/realms.json",
         "/wrath/status.json", "/wrath/api/boards/index.json", "/wrath/docs/search.json")
# A catalog page's filter links (one parameter) become pages of their own: /wrath/leyline/catalog?cat=ui is saved as
# /wrath/leyline/catalog/cat-ui/ and the links point there. Links with more parameters go to the plain page.
FILTER_PAGES = ("/wrath/leyline/catalog", "/wrath/leyline/addons")
FILTER_KEYS = ("cat", "group", "tag", "build", "source", "view", "page", "sort", "works")
MAX_FILTER_PAGES = 400
# The docs pages about what the server did: shown with a short, plain page instead (site/docs/<name>.md).
RETIRED_DOCS = ("index", "getting-started", "hero-key", "always-on", "mobile", "wrath-unleashed", "play")
# Links that only meant something with the server (the whole path, or anything under it). They are taken out.
DROPPED_EXACT = frozenset({"/wrath", "/wrath/leyline/p"})
DROPPED = ("/wrath/play", "/wrath/account", "/wrath/signin", "/wrath/key", "/wrath/profile", "/wrath/link",
           "/wrath/u/", "/wrath/avatar/", "/login", "/key", "/account", "/auth/", "/logout", "/wrath/leyline/forum",
           "/wrath/forum", "/wrath/api/", "/wrath/leaderboards", "/wrath/bossfights", "/wrath/arena", "/wrath/duels",
           "/wrath/boards", "/wrath/survivors", "/wrath/dens", "/wrath/admin", "/wrath/hq", "/wrath/submit",
           "/wrath/leyline/submit", "/wrath/mods/list", "/wrath/bugs", "/wrath/host", "/wrath/leyline/beacon",
           "/wrath/mobile", "/wrath/phonelab", "/wrath/solo", "/wrath/demo", "/wrath/r/", "/wrath/h/", "/wrath/q/",
           "/wrath/m/", "/wrath/armory", "/wrath/queue", "/wrath/status", "/wrath/healthz", "/wrath/modes",
           "/wrath/leyline/modes", "/wrath/leyline/game-modes", "/wrath/leyline/forge", "/wrath/leyline/gallery",
           "/wrath/leyline/loadouts", "/wrath/leyline/turn-later", "/wrath/creations", "/wrath/single-player",
           "/wrath/guides", "/wrath/press", "/wrath/leyline/releases/", "/wrath/leyline/herokey.json",
           "/wrath/leyline/approved-links.json", "/wrath/mods/c/", "/__auth", "/__gate", "/__env.js", "/join/",
           "/r/", "/h/", "/q/", "/m/", "/u/", "/agent")
# Nav items in the frame that go when their page goes (the whole <a>, not just its link).
NAV_CLASSES = ("hb-ni", "hb-mi", "hb-signin", "hb-playx", "hb-acct", "hb-dl-x", "hb-tab", "ly-nav", "hb-health")
ASSET_EXT = re.compile(r"\.(json|xml|txt|css|js|png|jpe?g|webp|gif|svg|ico|woff2?|ttf|otf|mp4|webm|avif)$", re.I)


def norm(path: str) -> str:
    p = urllib.parse.unquote(path.split("#", 1)[0].split("?", 1)[0])
    return p.rstrip("/") or "/"


def dropped(path: str) -> bool:
    p = norm(path)
    if p in DROPPED_EXACT:
        return True
    for d in DROPPED:
        if d.endswith("/"):
            if p.startswith(d) or p == d.rstrip("/"):
                return True
        elif p == d or p.startswith(d + "/"):
            return True
    return False


def portal_page(path: str) -> bool:
    p = norm(path)
    return any(p == x.rstrip("/") or p.startswith(x.rstrip("/") + "/") for x in PORTAL_PAGES)


def out_file(path: str) -> str:
    """The file a URL path is saved as, relative to the site's root."""
    p = norm(path)
    if p == "/":
        return "index.html"
    if ASSET_EXT.search(p):
        return p.lstrip("/")
    return p.lstrip("/") + "/index.html"


def filter_path(path: str, query: dict) -> str | None:
    """The saved page for a one-parameter filter link, or None."""
    p = norm(path)
    if p not in FILTER_PAGES or len(query) != 1:
        return None
    (k, v), = query.items()
    v = v[0] if isinstance(v, list) else v
    if k not in FILTER_KEYS or not v:
        return None
    slug = re.sub(r"[^a-z0-9.]+", "-", f"{k}-{v}".lower()).strip("-")[:60]
    return f"{p}/{slug}" if slug else None


# --------------------------------------------------------------------------------------------- inputs (site/)
GAMES = {"1.12.1": 5875, "2.4.3": 8606, "3.3.5a": 12340}
CORES = ("azerothcore", "trinitycore", "cmangos", "vmangos", "mangos", "other")
TYPES = ("PvE", "PvP", "RP", "RP-PvP", "Fun")
TAGS = ("blizzlike", "boosted", "custom", "pvp", "rp", "hangout", "bots")
REGIONS = ("na-east", "na-west", "sa", "eu", "africa", "me", "asia", "oce")
HOST_RE = re.compile(r"^[a-z0-9-]{1,63}(\.[a-z0-9-]{1,63})+$")
HTTPS_RE = re.compile(r"^https://[A-Za-z0-9.-]+\.[A-Za-z]{2,}(/[A-Za-z0-9._~:/?#@!$&'()*+,;=%-]*)?$")
LANG_RE = re.compile(r"^[a-z]{2,3}(-[A-Z]{2})?$")
VERSION_RE = re.compile(r"\b\d+\.\d+(\.\d+)?[a-z]?\b")
MONEY_RE = re.compile(r"(?i)donat|patreon|ko-fi|kofi|paypal|buymeacoffee|tip jar|\bshop\b|\bstore\b|cash\s?app|venmo")


def text_ok(where: str, s, n: int, allow_versions: bool = False) -> str:
    if s is None:
        return ""
    if not isinstance(s, str):
        raise Refused(f"{where}: must be text")
    s = " ".join(s.split())
    if len(s) > n:
        raise Refused(f"{where}: longer than {n} characters")
    if BANNED.search(s):
        raise Refused(f"{where}: names the game or its maker (players read this)")
    if MONEY_RE.search(s):
        raise Refused(f"{where}: no money, donation or shop words")
    if not allow_versions and VERSION_RE.search(s):
        raise Refused(f"{where}: no version numbers in the text (the game field says which client)")
    return s


def url_ok(where: str, u, need: bool = False) -> str | None:
    if u in (None, ""):
        if need:
            raise Refused(f"{where}: needed")
        return None
    if not isinstance(u, str) or not HTTPS_RE.match(u):
        raise Refused(f"{where}: must be a plain https:// address")
    host = urllib.parse.urlsplit(u).hostname or ""
    if MONEY_RE.search(u) or BANNED.search(u):
        raise Refused(f"{where}: not a link the site can carry")
    if host in OLD_HOSTS:
        raise Refused(f"{where}: points at Leyline's old server")
    return u


def realm_id(host: str, port: int) -> str:
    """A stable 16-character id (the launcher's realm id shape: base32 a-z 2-7) for a listed server."""
    raw = hashlib.sha256(f"leyline-listed-v1|{host}|{port}".encode()).digest()
    return base64.b32encode(raw).decode().lower()[:16]


def load_realms(path: Path = SITE / "realms.json") -> list[dict]:
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        raise Refused(f"{path.name}: {e}")
    if not isinstance(doc, dict) or doc.get("format") != 1 or not isinstance(doc.get("realms"), list):
        raise Refused(f'{path.name}: needs {{"format": 1, "realms": [...]}}')
    out, seen = [], set()
    for i, r in enumerate(doc["realms"]):
        w = f"{path.name} realm {i + 1}"
        if not isinstance(r, dict):
            raise Refused(f"{w}: must be an object")
        unknown = set(r) - {"name", "realmlist", "website", "discord", "game", "type", "core", "tagline", "notes", "tags",
                            "region", "language", "featured"}
        if unknown:
            raise Refused(f"{w}: unknown fields {sorted(unknown)} (site/README.md lists them)")
        name = text_ok(f"{w} name", r.get("name"), 32)
        if not name:
            raise Refused(f"{w}: needs a name")
        w = f"{path.name} {name!r}"
        rl = str(r.get("realmlist") or "").strip().lower()
        host, _, port_s = rl.partition(":")
        port = int(port_s) if port_s.isdigit() else (3724 if not port_s else 0)
        if not HOST_RE.match(host) or not (1 <= port <= 65535) or host in OLD_HOSTS or "leylinelauncher" in host:
            raise Refused(f"{w}: realmlist must be host or host:port (a plain address, lower case)")
        if (host, port) in seen:
            raise Refused(f"{w}: listed twice")
        seen.add((host, port))
        game = r.get("game")
        if game not in GAMES:
            raise Refused(f"{w}: game must be one of {', '.join(GAMES)}")
        kind = r.get("type") or "PvE"
        core = r.get("core") or "other"
        if kind not in TYPES or core not in CORES:
            raise Refused(f"{w}: type must be one of {TYPES}, core one of {CORES}")
        tags = r.get("tags") or []
        if not isinstance(tags, list) or any(t not in TAGS for t in tags):
            raise Refused(f"{w}: tags must be some of {TAGS}")
        region, lang = r.get("region"), r.get("language")
        if region is not None and region not in REGIONS:
            raise Refused(f"{w}: region must be one of {REGIONS}")
        if lang is not None and not (isinstance(lang, str) and LANG_RE.match(lang)):
            raise Refused(f"{w}: language must be a code like en, de or pt-BR")
        out.append({"name": name, "host": host, "port": port, "rid": realm_id(host, port),
                    "website": url_ok(f"{w} website", r.get("website"), need=True),
                    "discord": url_ok(f"{w} discord", r.get("discord")), "game": game, "build": GAMES[game],
                    "type": kind, "core": core, "tagline": text_ok(f"{w} tagline", r.get("tagline"), 80),
                    "notes": text_ok(f"{w} notes", r.get("notes"), 400), "tags": [t for t in TAGS if t in tags],
                    "region": region, "language": lang, "featured": r.get("featured") is True})
    out.sort(key=lambda x: (not x["featured"], x["name"].lower()))
    return out


def realms_doc(realms: list[dict], stamp: str) -> dict:
    """/wrath/realms.json: the network listing's schema-1 shape (what the launcher's realms.rs parse_community reads),
    with no official realm and no live numbers. Extra fields (realmlist, website) are for launchers that read them."""
    recs = []
    for r in realms:
        recs.append({
            "schema": 1, "slug": "c-" + r["rid"], "name": r["name"], "official": False, "state": "live", "listed": True,
            "tagline": r["tagline"], "summary": r["notes"], "type": r["type"],
            "realmlist": f'{r["host"]}:{r["port"]}' if r["port"] != 3724 else r["host"],
            "website": r["website"], "discord": r["discord"],
            "profile": {"game": r["game"], "version": r["game"], "build": r["build"], "core": r["core"],
                        "address": r["host"], "port": r["port"], "ruleset": "custom"},
            "connect": {"kind": "website", "url": r["website"], "label": "Make an account on their site"},
            "rules": [], "mods": [], "tags": r["tags"], "region": r["region"], "language": r["language"],
            "community": {"realmId": r["rid"], "relay": r["host"], "claimed": True, "listed": True,
                          "lastHeartbeat": {"online": None, "players": None, "bots": None},
                          "uptime": {"h24": None, "d7": None}, "new": False, "score": 0.0,
                          "featured": r["featured"], "verified": False, "official": False, "sample": False,
                          "group": "featured" if r["featured"] else "community"},
        })
    return {"schema": 1, "kind": "listing", "ok": True, "updated": stamp, "source": "curated",
            "note": "The servers Leyline lists, chosen by hand. Leyline runs no server of its own.", "realms": recs}


def load_news(path: Path = SITE / "news.json") -> dict:
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        raise Refused(f"{path.name}: {e}")
    if not isinstance(doc, dict) or doc.get("format") != 1 or not isinstance(doc.get("posts"), list):
        raise Refused(f'{path.name}: needs {{"format": 1, "posts": [...]}}')
    posts = []
    for i, p in enumerate(doc["posts"][:20]):
        w = f"{path.name} post {i + 1}"
        if not isinstance(p, dict):
            raise Refused(f"{w}: must be an object")
        date = str(p.get("date") or "")
        if not re.match(r"^\d{4}-\d{2}-\d{2}$", date):
            raise Refused(f"{w}: date must be YYYY-MM-DD")
        title = text_ok(f"{w} title", p.get("title"), 120, allow_versions=True)
        if not title:
            raise Refused(f"{w}: needs a title")
        items = p.get("items") or []
        if not isinstance(items, list):
            raise Refused(f"{w}: items must be a list")
        posts.append({"id": re.sub(r"[^a-z0-9-]", "", str(p.get("id") or f"{date}-{i}").lower())[:60] or f"{date}-{i}",
                      "date": date, "tag": text_ok(f"{w} tag", p.get("tag"), 60, allow_versions=True), "title": title,
                      "items": [text_ok(f"{w} item", x, 200, allow_versions=True) for x in items[:12]]})
    return {"format": 1, "updated_at": (posts[0]["date"] + "T00:00:00Z") if posts else None, "posts": posts}


def newest_input() -> float:
    newest = 0.0
    for d in (SITE, STREAM / "hub", REPO / "docs" / "leyline"):
        for p in d.rglob("*"):
            if p.is_file():
                newest = max(newest, p.stat().st_mtime)
    for f in ("upstream-addons.json", "looks.json", "hostpack.json"):
        p = STREAM / f
        if p.is_file():
            newest = max(newest, p.stat().st_mtime)
    return newest or time.time()


# --------------------------------------------------------------------------------------------- the portal, in here
def no_network() -> None:
    """Every connection off this machine fails at once: the build never reaches a server, ours or anyone's."""
    real = socket.socket.connect

    def local(addr) -> bool:
        host = addr[0] if isinstance(addr, tuple) else addr
        return isinstance(host, str) and (host in ("127.0.0.1", "::1", "localhost"))

    def connect(self, addr):
        if not local(addr):
            raise OSError("the static build makes no network connections")
        return real(self, addr)

    def create_connection(addr, *a, **k):
        if not local(addr):
            raise OSError("the static build makes no network connections")
        return _orig_cc(addr, *a, **k)

    _orig_cc = socket.create_connection
    socket.socket.connect = connect
    socket.create_connection = create_connection
    real_gai = socket.getaddrinfo

    def getaddrinfo(host, *a, **k):
        if host not in ("127.0.0.1", "::1", "localhost", None):
            raise socket.gaierror("the static build makes no network connections")
        return real_gai(host, *a, **k)
    socket.getaddrinfo = getaddrinfo


class Portal:
    """The portal, loaded from stream/ and served on 127.0.0.1 for this build only."""

    def __init__(self, tmp: Path, news: dict, stamp: str):
        self.tmp, self.news = tmp, news
        data, ley = tmp / "data", tmp / "data" / "leyline"
        for d in (data, ley, tmp / "mods", tmp / "hub-data", tmp / "docs", tmp / "empty"):
            d.mkdir(parents=True, exist_ok=True)
        (ley / "news.json").write_text(json.dumps(news), encoding="utf-8")
        # the docs: the repo's pages, with the retired ones swapped for their short static page
        src = REPO / "docs" / "leyline"
        for p in sorted(src.glob("*.md")):
            mine = SITE / "docs" / p.name
            shutil.copyfile(mine if (p.stem in RETIRED_DOCS and mine.is_file()) else p, tmp / "docs" / p.name)
        env = {
            "SOAP_USER": "none", "SOAP_PASS": "none", "AC_SOAP_URL": "http://127.0.0.1:9/", "PORTAL_URL": "http://127.0.0.1:9",
            "WU_STREAM": "0", "WU_OWNFILES": "0", "PUBLIC_HOST": DOMAIN, "STREAM_HOST": DOMAIN, "LEYLINE_BRAND": "1",
            "WU_HUB_FRONT": "1", "WU_PORTAL_DATA": str(data), "WU_LEYLINE_DIR": str(ley), "WU_HEROKEY_DIR": str(ley),
            "WU_RELAY_LISTING": "http://127.0.0.1:9/none.json", "WU_RELAY_LISTING_FILE": str(tmp / "empty" / "none"),
            "WU_HANGOUT_FILE": str(tmp / "empty" / "none"), "WU_BUILD_REALM_FILE": str(tmp / "empty" / "none"),
            "WU_DL_PROBE": "0", "WU_MODS_DIR": str(tmp / "mods"), "WU_WORKSHOP_ITEMS_DIR": str(tmp / "empty"),
            "WU_ART_DIR": str(STREAM / "www-files" / "art"), "WU_HUB_DIR": str(STREAM / "hub"),
            "WU_DOCS_DIR": str(tmp / "docs"), "WU_ARMORY_DIR": str(tmp / "empty"), "WU_DUELS_DIR": str(tmp / "empty"),
            "WU_SLICE_DIR": str(tmp / "empty"), "WU_GAMEDATA_FLAG": str(tmp / "empty" / "gamedata-off"),
            "SINGLE_PLAYER_PUBLIC": "0", "WU_SOLO_RELEASED": "0", "WU_DENS": "0", "WU_HEALTH_FIRST_WAIT": "0",
            "WU_STATUS_FIRST_WAIT": "0", "TZ": "UTC",
        }
        os.environ.update(env)
        for k in list(os.environ):   # no proxy either: a proxy on this machine would carry requests out
            if k.lower() in ("http_proxy", "https_proxy", "all_proxy", "ftp_proxy"):
                del os.environ[k]
        os.environ["no_proxy"] = os.environ["NO_PROXY"] = "*"
        no_network()
        sys.dont_write_bytecode = True
        spec = importlib.util.spec_from_file_location("wrath_portal_static", STREAM / "wrath_portal.py")
        wp = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(wp)
        self.wp = wp
        self._patch()
        from http.server import ThreadingHTTPServer
        self.srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
        self.port = self.srv.server_address[1]
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()

    def _patch(self) -> None:
        wp = self.wp
        hub_mod = wp.hub_mod
        if hub_mod is None or getattr(hub_mod, "hub", None) is None:
            raise Refused("stream/hub.py did not load: the site's frame comes from it")
        Hub = type(hub_mod.hub)
        F = hub_mod.FRONT
        B = hub_mod.BASE

        def nav_items(self):
            main = [("realms", "Realms", B + "/realms", "realms"), ("mods", "Mods", B + "/catalog", "mods"),
                    ("addons", "Addons", B + "/addons", "addons"), ("looks", "Looks", "/wrath/looks", "looks"),
                    ("docs", "Docs", "/wrath/docs", "docs"), ("news", "News", B + "/news", "news")]
            more = [("guides", "Start here", "/wrath/start", "guides"), ("agents", "Agents", B + "/agents", "agents"),
                    ("creators", "Creators", B + "/creators", "creators")]
            return main, more
        Hub.nav_items = nav_items
        Hub.health_state = lambda self: ""
        Hub._backdrop = lambda self: ""   # the owner's backdrop picture is not ours to publish (PRIVATE_ART)
        # nobody is ever signed in here, and nothing is live
        if hasattr(hub_mod, "_viewer"):
            hub_mod._viewer = lambda h: None
        news = self.news

        class Admin:   # the old admin console: only the News it published, from site/news.json now
            match = staticmethod(lambda path: False)
            news_doc = staticmethod(lambda: news)
            channel_doc = staticmethod(lambda: {})
        wp.admin = Admin()
        if getattr(wp, "playmodes", None) is not None:   # every mode ran on the official realm
            wp.playmodes.CATALOG = {"schema": 1, "modes": []}
        wp.realm_status = lambda: {}
        wp.community_realms = lambda: ([], False)
        if hasattr(wp, "listed_realms"):
            wp.listed_realms = lambda: ([], False)

    def get(self, path: str):
        c = http.client.HTTPConnection("127.0.0.1", self.port, timeout=60)
        try:
            c.request("GET", path, headers={"Host": DOMAIN, "User-Agent": "leyline-static-build"})
            r = c.getresponse()
            return r.status, (r.getheader("Content-Type") or ""), r.read(), r.getheader("Location")
        finally:
            c.close()

    def frame(self, title: str, body: str, path: str, current: str, css: str = "", desc: str = "") -> bytes:
        meta = ""
        if desc:
            meta = self.wp.page_card(path, title, desc, with_description=True) if hasattr(self.wp, "page_card") else ""
        out = self.wp.hub_frame(title, body, path, current, css=css, meta=meta)
        if not out:
            raise Refused("the site's frame (stream/hub.py) failed")
        return out

    def close(self) -> None:
        self.srv.shutdown()
        self.srv.server_close()


# --------------------------------------------------------------------------------------------- tidying a page
A_RE = re.compile(r"<a\b([^>]*)>(.*?)</a>", re.S | re.I)
HREF_RE = re.compile(r'\bhref="([^"]*)"', re.I)
CLASS_RE = re.compile(r'\bclass="([^"]*)"', re.I)
FORM_RE = re.compile(r"<form\b([^>]*)>.*?</form>", re.S | re.I)


def local_path(href: str) -> str | None:
    """The site path of a link to our own site (relative, or absolute to one of our names), else None."""
    h = html.unescape(href).strip()
    if h.startswith("//"):
        h = "https:" + h
    if h.startswith(("http://", "https://")):
        u = urllib.parse.urlsplit(h)
        if (u.hostname or "").lower() not in OLD_HOSTS:
            return None
        return (u.path or "/") + ("?" + u.query if u.query else "") + ("#" + u.fragment if u.fragment else "")
    if h.startswith("/"):
        return h
    return None


class Tidy:
    """Rewrites links in a page: to pages that exist, to their saved filter page, or out."""

    def __init__(self, have: set[str], filters: dict[str, str]):
        self.have, self.filters = have, filters
        self.prose = False   # docs pages: a link that goes keeps its words

    def target(self, href: str) -> tuple[str, str | None]:
        """("keep", new href) | ("drop", None) | ("leave", None) for an outside link."""
        h = html.unescape(href).strip()
        if h.startswith("leyline://"):
            return ("drop", None) if "realm/official" in h or "/play" in h else ("leave", None)
        if h.startswith(("#", "mailto:")) or not h:
            return "leave", None
        lp = local_path(h)
        if lp is None:
            return "leave", None
        u = urllib.parse.urlsplit(lp)
        p = norm(u.path)
        frag = "#" + u.fragment if u.fragment else ""
        q = urllib.parse.parse_qs(u.query)
        if p in MOVED:
            p = MOVED[p]
        if p in ELSEWHERE:
            return "keep", ELSEWHERE[p]
        if u.query:
            fp = filter_path(p, q)
            if fp and fp in self.filters:
                return "keep", self.filters[fp] + frag
        if dropped(p) and p not in self.have:
            return "drop", None
        if p in self.have or p.startswith("/wrath/art/"):
            return "keep", (p if p == "/" else p) + frag
        return "drop", None

    def page(self, body: str) -> str:
        def one(m):
            attrs, inner = m.group(1), m.group(2)
            hm = HREF_RE.search(attrs)
            if not hm:
                return m.group(0)
            what, new = self.target(hm.group(1))
            if what == "leave":
                return m.group(0)
            if what == "keep":
                return "<a" + attrs[:hm.start()] + f'href="{html.escape(new, quote=True)}"' + attrs[hm.end():] + ">" + inner + "</a>"
            cls = (CLASS_RE.search(attrs) or [None, ""])[1] if CLASS_RE.search(attrs) else ""
            if any(c in cls.split() for c in NAV_CLASSES) or re.search(r"\b(btn|button|hb-btn|ly-btn|hb-cta|cta|hb-go|stone)\b", cls):
                return ""
            words = re.sub(r"<[^>]+>", "", inner).strip()
            if not self.prose and len(words) <= 40:
                return ""
            return f"<span>{inner}</span>"
        body = A_RE.sub(one, body)

        def form(m):
            act = re.search(r'\baction="([^"]*)"', m.group(1))
            if not act:
                return m.group(0) if re.search(r'method="get"', m.group(1), re.I) is None else ""
            what, new = self.target(act.group(1))
            if what == "keep" and norm(new).startswith("/wrath/docs"):
                return m.group(0)
            return ""
        body = FORM_RE.sub(form, body)
        return body


def tidy_frame(page: str) -> str:
    """The frame's pieces that belonged to the server: the account chip, Play, the status dot, the footer's links."""
    page = re.sub(r'<a class="hb-signin"[^>]*>.*?</a>', "", page, flags=re.S)
    page = re.sub(r'<a class="hb-acct"[^>]*>.*?</a>', "", page, flags=re.S)
    page = re.sub(r'<a class="hb-playx"[^>]*>.*?</a>', "", page, flags=re.S)
    page = re.sub(r'<div class="hb-turn\b.*?</div>\s*</div>', "", page, flags=re.S) if 'class="hb-turn' in page else page
    foot = ('<nav class="hb-foot-links" aria-label="More">'
            '<a href="/wrath/start">Start here</a><a href="/wrath/download">Download</a>'
            '<a href="/wrath/leyline/realms">Realms</a><a href="/wrath/leyline/catalog">Mods</a>'
            '<a href="/wrath/leyline/addons">Addons</a><a href="/wrath/looks">Looks</a><a href="/wrath/docs">Docs</a>'
            '<a href="/wrath/leyline/news">News</a><a href="/wrath/leyline/creators">Creators</a></nav>')
    page = re.sub(r'<nav class="hb-foot-links"[^>]*>.*?</nav>', foot.replace("\\", "\\\\"), page, count=1, flags=re.S)
    page = page.replace("No shop, nothing to buy. Bots are labelled.", "No shop, nothing to buy.")
    page = re.sub(r'<aside class="hb-turn".*?</aside>', "", page, flags=re.S)        # the turn-your-phone hint
    page = re.sub(r'<section class="ly-panel hb-rank">.*?</section>', "", page, flags=re.S)   # ratings: signed in only
    page = re.sub(r'<p class="hb-rk">.*?</p>', "", page, flags=re.S)                 # "No ratings yet · 0 installs"
    page = re.sub(r"<dt>On Wrath Unleashed</dt><dd>.*?</dd>", "", page, flags=re.S)   # tried on the official realm
    for old, new in PHRASES:
        page = page.replace(html.escape(old, quote=True), html.escape(new, quote=True)).replace(old, new)
    page = PRIVATE_REPO.sub(REPO_URL, page)   # the workspace repo is private: its public twin instead
    return page


def strip_noindex(page: str) -> str:
    page = re.sub(r' nonce="[^"]*"', "", page)   # no CSP on a static host: the nonces only made each build differ
    return page.replace('<meta name="robots" content="noindex">', "").replace('<meta name="robots" content="noindex,nofollow">', "")


# --------------------------------------------------------------------------------------------- our own pages
OWN_CSS = """
.st-wrap{max-width:1040px;margin:0 auto;padding:8px 18px 40px}
.st-hero{text-align:center;padding:28px 0 8px}.st-hero h1{margin:0 0 8px}
.st-lead{font-size:1.12rem;max-width:720px;margin:0 auto 14px;color:var(--lk-text-2,#cfc8b6)}
.st-cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(230px,1fr));gap:14px;margin:18px 0}
.st-card{border:1px solid var(--lk-line,#4a3b1c);border-radius:10px;padding:16px 18px;background:rgba(12,10,24,.55)}
.st-card h2,.st-card h3{margin:0 0 6px;font-size:1.1rem}.st-card p{margin:0 0 8px}
.st-note{border:1px solid #8a6a2c;border-radius:10px;padding:14px 18px;margin:18px 0;background:rgba(58,42,18,.55)}
.st-note h2{margin:0 0 6px;font-size:1.05rem}
.st-realm{border:1px solid var(--lk-line,#4a3b1c);border-radius:10px;padding:14px 18px;margin:12px 0;background:rgba(12,10,24,.55)}
.st-realm h3{margin:0 0 4px}.st-realm .st-meta{color:var(--lk-text-2,#b4ab96);font-size:.92rem;margin:0 0 6px}
.st-realm code{user-select:all}
.st-shot{margin:18px auto;max-width:1040px}.st-shot img{width:100%;height:auto;border-radius:10px}
.st-list{margin:8px 0 0 1.2em;padding:0}.st-list li{margin:4px 0}
"""
DOCNOTE_CSS = ("<style>.st-docnote{border:1px solid #8a6a2c;border-radius:8px;padding:10px 14px;margin:10px 0 16px;"
               "background:rgba(58,42,18,.55)}</style>")


def esc(s) -> str:
    return html.escape(str(s or ""), quote=True)


def front_body() -> str:
    return (
        '<div class="st-wrap"><section class="st-hero" aria-labelledby="st-h">'
        '<!-- TODO(owner): the hero heading and the line under it; his final wording -->'
        '<h1 id="st-h">Every client, every server</h1>'
        '<p class="st-lead">Leyline is a desktop launcher for the game clients you already have. It finds your game '
        'folders, lists community servers, runs single player and Your Den on your own PC, and installs mods from the '
        'people who made them.</p>'
        '<p><a href="/wrath/download">Download the launcher</a> · <a href="/wrath/start">New here? Start here ›</a></p>'
        '</section>'
        '<figure class="st-shot"><img src="/wrath/art/leyline-ui-clients.webp" width="1600" height="900" '
        'alt="The Leyline launcher with its game client list open, each client with its version." decoding="async"></figure>'
        '<section class="st-note" aria-labelledby="st-now"><h2 id="st-now">Leyline runs no server of its own now</h2>'
        '<p>The official realm, the relay, sign-in, the forum, the leaderboards and the browser game have closed. '
        'The launcher keeps working on your PC: single player, Your Den, mods and the servers on the realm list do not '
        'need us. <!-- TODO(owner): a line in your words about why and what comes next --></p>'
        f'<p>Questions and news: <a href="{DISCORD}" rel="noopener noreferrer">the Leyline Discord</a>.</p></section>'
        '<div class="st-cards">'
        '<div class="st-card"><h2>Your own game folders</h2><p>Point Leyline at the clients you already have. It never '
        'downloads, shares or ships game files.</p><p><a href="/wrath/docs/getting-started">Getting started ›</a></p></div>'
        '<div class="st-card"><h2>Servers, in one list</h2><p>Community servers run by other people, with their address '
        'and their own website. Pick one and Leyline sets your realm list for you.</p>'
        '<p><a href="/wrath/leyline/realms">See the realms ›</a></p></div>'
        '<div class="st-card"><h2>Single player and Your Den</h2><p>A world of your own on your own PC: no account and no '
        'internet once it is set up. Your Den is a private place to build in.</p>'
        '<p><a href="/wrath/docs/single-player">Single player ›</a> <a href="/wrath/docs/den">Your Den ›</a></p></div>'
        '<div class="st-card"><h2>Mods from their authors</h2><p>Addons, interfaces and graphics looks, each installed '
        'from its author\'s own page, with credit. One click takes you back to the original.</p>'
        '<p><a href="/wrath/leyline/catalog">Mods ›</a> <a href="/wrath/leyline/addons">Addons ›</a> '
        '<a href="/wrath/looks">Looks ›</a></p></div>'
        '</div>'
        '<p class="hb-fine">Community projects are links: nothing of theirs is hosted here. Each belongs to its authors, '
        'under the licence they chose. Leyline is free and open source; there is nothing to buy.</p></div>')


# The portal's own front page (the hero, Ways to play, the launcher, Looks, Host, modes, the Discord), with what
# needed the server taken out. Owner 27 Nov: "the way it was minus the live server connotations".
FRONT_DROP = (
    r'<article class="lp-mode" id="way-official">.*?</article>',      # the official realm's card (and its picture)
    r'<section class="hb-sbar\b.*?</section>',                        # the live platform status bar
    r'<li>One sign-in everywhere</li>',
    r'<img\b[^>]*\bsrc="/wrath/avatar/[^"]*"[^>]*>',                    # profile pictures were drawn by the server
    r'<li>Duels: the 1v1 queue[^<]*</li>',
    r'<a href="[^"]*">Open the forum</a>',              # the forum closed with the server                            # Duels ran on the official realm
)
FRONT_SWAP = (
    ("Or play in your browser, nothing to install", "Leyline finds it for you"),
    ("finds the game folders you already have, signs you in once, installs", "finds the game folders you already have, installs"),
    (" PLAY signs you in and starts once your character is in the world.", ""),
    ("Ranked by players online and uptime, never by money.", "Never ranked by money."),
    ("Fight the next player in the 1v1 queue, or go straight onto", "Go straight onto"),
    ("Duels and Boss Rush, one click each", "Boss Rush, one click"),
    ('<h2 id="hb-cm-h">Forum and Discord</h2>', '<h2 id="hb-cm-h">Discord</h2>'),
)


def front_from_portal(page: str) -> str:
    for rx in FRONT_DROP:
        page = re.sub(rx, "", page, flags=re.S)
    for old, new in FRONT_SWAP:
        page = page.replace(html.escape(old, quote=True), html.escape(new, quote=True)).replace(old, new)
    return page


def start_body() -> str:
    return (
        '<div class="st-wrap"><section class="st-hero"><h1>Start here</h1>'
        '<p class="st-lead">Three steps from nothing to playing. You need a copy of the game you already own; Leyline '
        'never provides one.</p></section>'
        '<div class="st-cards">'
        '<div class="st-card"><h2>1. Get the launcher</h2><p>Free, for Windows, macOS and Linux. It updates itself.</p>'
        '<p><a href="/wrath/download">Every download ›</a></p></div>'
        '<div class="st-card"><h2>2. Add your game folder</h2><p>Open <b>Game folders</b> and choose the folder you '
        'play from. Leyline names it and remembers it.</p><p><a href="/wrath/docs/getting-started">The full guide ›</a></p></div>'
        '<div class="st-card"><h2>3. Pick a way to play</h2><ul class="st-list">'
        '<li><a href="/wrath/docs/single-player">Single player</a>: your own world on your PC.</li>'
        '<li><a href="/wrath/docs/den">Your Den</a>: a private place to build.</li>'
        '<li><a href="/wrath/leyline/realms">A community server</a> from the list.</li></ul></div>'
        '</div>'
        '<div class="st-cards">'
        '<div class="st-card"><h3>Make it yours</h3><p>Addons, new interfaces and graphics presets, from their authors.</p>'
        '<p><a href="/wrath/leyline/addons">Addons ›</a> <a href="/wrath/looks">Looks ›</a></p></div>'
        '<div class="st-card"><h3>Stuck?</h3><p>The docs cover every page of the launcher.</p>'
        f'<p><a href="/wrath/docs">Docs ›</a> <a href="{DISCORD}" rel="noopener noreferrer">Ask on Discord ›</a></p></div>'
        '</div></div>')


def realms_body(realms: list[dict]) -> str:
    rows = []
    for r in realms:
        addr = f'{r["host"]}:{r["port"]}' if r["port"] != 3724 else r["host"]
        meta = " · ".join(x for x in (r["type"], {"azerothcore": "AzerothCore", "trinitycore": "TrinityCore",
                                                  "cmangos": "CMaNGOS", "vmangos": "VMaNGOS", "mangos": "MaNGOS"}.get(r["core"], ""),
                                      ", ".join(r["tags"]), r["region"] or "", r["language"] or "") if x)
        links = [f'<a href="{esc(r["website"])}" rel="noopener noreferrer">Website</a>']
        if r["discord"]:
            links.append(f'<a href="{esc(r["discord"])}" rel="noopener noreferrer">Discord</a>')
        rows.append(
            f'<article class="st-realm" id="{esc(r["rid"])}"><h3>{esc(r["name"])}'
            f' <span class="ver" title="Game version">{esc(r["game"])}</span></h3>'
            + (f'<p class="st-meta">{esc(meta)}</p>' if meta else "")
            + (f'<p>{esc(r["tagline"])}</p>' if r["tagline"] else "")
            + (f'<p>{esc(r["notes"])}</p>' if r["notes"] else "")
            + f'<p>Realm list address: <code>{esc(addr)}</code> · ' + " · ".join(links) + '</p></article>')
    if not rows:
        rows.append('<div class="st-note"><h2>No servers listed yet</h2><p>The list is being put together. '
                    f'Run a server and want it here? Ask on <a href="{DISCORD}" rel="noopener noreferrer">the Leyline '
                    'Discord</a>.</p></div>')
    return (
        '<div class="st-wrap"><section class="st-hero"><h1>Realms</h1>'
        '<p class="st-lead">Community servers run by other people. Each runs its own accounts and rules: make your '
        'account on its website, then pick it in the launcher and press Play.</p></section>'
        + "".join(rows) +
        '<div class="st-card"><h2>How this list works</h2><ul class="st-list">'
        '<li>Servers are added by hand, with their owner\'s OK. Leyline does not run, check or vouch for them.</li>'
        '<li>No live player numbers: the launcher checks each server itself when it shows the list.</li>'
        '<li>Your game files stay yours: Leyline only changes the realm list address in your own client.</li>'
        f'<li>Run a server? Ask on <a href="{DISCORD}" rel="noopener noreferrer">Discord</a> to be listed.</li>'
        '</ul></div></div>')


def not_found_body() -> str:
    return (
        '<div class="st-wrap"><section class="st-hero"><h1>Page not found</h1>'
        '<p class="st-lead">This page is not here. Some pages closed when Leyline stopped running its own server: '
        'sign-in, the forum, the leaderboards, the browser game and the official realm.</p></section>'
        '<div class="st-cards">'
        '<div class="st-card"><h2>Get the launcher</h2><p><a href="/wrath/download">Download ›</a></p></div>'
        '<div class="st-card"><h2>Find a server</h2><p><a href="/wrath/leyline/realms">Realms ›</a></p></div>'
        '<div class="st-card"><h2>Mods and addons</h2><p><a href="/wrath/leyline/catalog">Mods ›</a> '
        '<a href="/wrath/leyline/addons">Addons ›</a></p></div>'
        '<div class="st-card"><h2>Help</h2><p><a href="/wrath/docs">Docs ›</a> '
        f'<a href="{DISCORD}" rel="noopener noreferrer">Discord ›</a></p></div>'
        '</div></div>')


def moved_page(to: str) -> bytes:
    t = esc(to)
    canon = to if to.startswith("https://") else ORIGIN + to
    return (f'<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Moved</title>'
            f'<meta http-equiv="refresh" content="0; url={t}"><link rel="canonical" href="{esc(canon)}">'
            f'<meta name="robots" content="noindex"></head><body><p>This page moved: <a href="{t}">{t}</a>.</p></body>'
            f'</html>\n').encode("utf-8")


# --------------------------------------------------------------------------------------------- feeds
def flags_doc(leyline_net, stamp: str) -> dict:
    """Every server feature off, hashed as the launcher checks it (p2p/flags.rs)."""
    if leyline_net is None:
        raise Refused("stream/leyline_net.py did not load: the flags file needs its hash")
    return leyline_net.make_doc(flags={}, limits={}, stun=[], updated_at=stamp)


def status_doc(stamp: str) -> dict:
    return {"realm": None, "online": False, "players": None, "bots": None, "queue": None, "uptime_s": None,
            "retired": True, "note": "Leyline runs no realm of its own now.", "gamedata_streaming": False,
            "updated_at": stamp, "browser_queue": {"waiting": 0, "wait_min": 0, "line": False}}


def our_server(u) -> bool:
    if not isinstance(u, str) or not u.startswith(("http://", "https://")):
        return False
    return (urllib.parse.urlsplit(u).hostname or "").lower() in OLD_HOSTS


def all_json(body: bytes) -> bytes:
    """all.json with every item whose download is on our old server left out (only installs from their authors, or
    from Leyline's own GitHub releases, stay), and its pages pointed at the static ones."""
    doc = json.loads(body)
    keep = []
    for p in doc.get("projects") or []:
        if not isinstance(p, dict):
            continue
        urls = [p.get("download"), p.get("torrent"), (p.get("install") or {}).get("url") if isinstance(p.get("install"), dict) else None]
        if any(our_server(u) for u in urls):
            continue
        if our_server(p.get("page")):
            lp = norm(local_path(p["page"]) or "/")
            p["page"] = ORIGIN + lp if not dropped(lp) else None
        keep.append(p)
    doc["projects"] = keep
    return (json.dumps(doc, indent=1, ensure_ascii=False) + "\n").encode("utf-8")


def restamp(body: bytes, stamp: str) -> bytes:
    """A feed's own clock times ("generated_at") replaced by the build's stamp, so the same inputs give the same bytes."""
    return re.sub(rb'"generated_at":\s*"[^"]*"', b'"generated_at": "' + stamp.encode() + b'"', body)


PRIVATE_REPO = re.compile(r"https://github\.com/orionopenwow/(?:leyline-workspace|WrathUnleashed)\b")


def fix_pages_json(body: bytes) -> bytes:
    """Feeds that name item pages at the old preview address: the front-door address instead. The rankings' formulas
    link went to the private workspace repo: left out."""
    body = body.replace(b"/wrath/leyline-preview/", b"/wrath/leyline/").replace(b"play.openvanilla.app", DOMAIN.encode())
    doc = json.loads(body)
    if isinstance(doc, dict) and PRIVATE_REPO.match(str(doc.get("formulas") or "")):
        doc["formulas"] = None
        body = json.dumps(doc, ensure_ascii=False, separators=(",", ":")).encode("utf-8") + b"\n"
    return body


# --------------------------------------------------------------------------------------------- the build
class Build:
    def __init__(self, out: Path, stamp: str, quiet: bool = False):
        self.out, self.stamp, self.quiet = out, stamp, quiet
        self.files: dict[str, bytes] = {}
        self.pages: dict[str, str] = {}      # site path -> HTML (not yet tidied)
        self.filters: dict[str, str] = {}    # filter path -> its own path
        self.art: set[str] = set()

    def say(self, msg: str) -> None:
        if not self.quiet:
            print(msg)

    def put_file(self, rel: str, data: bytes | str) -> None:
        self.files[rel] = data.encode("utf-8") if isinstance(data, str) else data

    def run(self) -> dict:
        realms = load_realms()
        news = load_news()
        with tempfile.TemporaryDirectory(prefix="leyline-site-") as tmp:
            portal = Portal(Path(tmp), news, self.stamp)
            try:
                self._feeds(portal, realms, news)
                self._own(portal, realms)
                self._crawl(portal)
                self._finish(portal)
            finally:
                portal.close()
        self._write()
        return {"files": {k: len(v) for k, v in sorted(self.files.items())}}

    # the feeds
    def _feeds(self, portal: Portal, realms: list[dict], news: dict) -> None:
        wp = portal.wp
        dump = lambda d: (json.dumps(d, indent=1, ensure_ascii=False) + "\n").encode("utf-8")
        self.put_file("wrath/leyline/flags.json", dump(flags_doc(wp.leyline_net, self.stamp)))
        self.put_file("wrath/leyline/news.json", dump(news))
        self.put_file("wrath/realms.json", dump(realms_doc(realms, self.stamp)))
        self.put_file("wrath/status.json", dump(status_doc(self.stamp)))
        # the modes the launcher offered all ran on the official realm: none now (the launcher's own copy is built in)
        self.put_file("wrath/modes.json", dump({"schema": 1, "modes": []}))
        self.put_file("wrath/api/boards/index.json", dump({"v": 1, "week": None, "official": [], "community": []}))
        for path, fix in (("/wrath/mods/all.json", all_json), ("/wrath/leyline/catalog.json", fix_pages_json),
                          ("/wrath/leyline/mods.json?sort=hot&limit=1000", fix_pages_json),
                          ("/wrath/docs/search.json", None), ("/wrath/leyline/hostpack.json", None)):
            st, ct, body, _ = portal.get(path)
            if st != 200 or "json" not in ct:
                if path.endswith("hostpack.json"):
                    continue
                raise Refused(f"the portal answered {st} for {path}")
            json.loads(body)
            if fix is not None:
                body = fix(body)
            if path.startswith("/wrath/mods/all.json"):
                body = all_json(body)
            self.put_file(out_file(path), restamp(body, self.stamp))
        # the catalog feed: the same rule as all.json (installs from their authors only)
        cat = json.loads(self.files["wrath/leyline/catalog.json"])
        if isinstance(cat.get("projects"), list):
            self.files["wrath/leyline/catalog.json"] = restamp(all_json(json.dumps(cat).encode()), self.stamp)
        self.say(f"feeds: {len([f for f in self.files if f.endswith('.json')])}")

    # the pages drawn here
    def _own(self, portal: Portal, realms: list[dict]) -> None:
        st, ct, body, _ = portal.get("/wrath/leyline")
        if st != 200 or not ct.startswith("text/html"):
            raise Refused(f"the portal's front page answered {st}")
        front = front_from_portal(body.decode("utf-8"))
        self.pages["/wrath/leyline"] = front
        self.pages["/"] = front
        self.pages["/wrath/start"] = portal.frame("Start here · Leyline", start_body(), "/wrath/start", "start",
                                                  css=OWN_CSS, desc="Start here: get the launcher, add your game folder, pick a way to play.").decode("utf-8")
        self.pages["/wrath/leyline/realms"] = portal.frame("Realms · Leyline", realms_body(realms), "/wrath/leyline/realms",
                                                           "realms", css=OWN_CSS,
                                                           desc="Community servers listed by Leyline.").decode("utf-8")
        self.pages["/404"] = portal.frame("Page not found · Leyline", not_found_body(), "/wrath/leyline/404", "",
                                          css=OWN_CSS).decode("utf-8")

    # the portal's pages
    def _crawl(self, portal: Portal) -> None:
        todo = [p for p in PORTAL_PAGES if not p.endswith("/")] + ["/wrath/docs/search"]
        todo += [f"/wrath/docs/{n}" for n, _ in getattr(portal.wp.docs, "PAGES", ()) if n != "index"]
        seen: set[str] = set(self.pages)
        filt_todo: list[tuple[str, str]] = []
        while todo or filt_todo:
            if todo:
                path = todo.pop(0)
                fetch, save = path, path
            else:
                fetch, save = filt_todo.pop(0)
            if save in seen:
                continue
            seen.add(save)
            st, ct, body, loc = portal.get(fetch)
            if st in (301, 302, 307, 308) and loc:
                to = norm(local_path(loc) or "/")
                if portal_page(to) and to not in seen:
                    todo.append(to)
                continue
            if st != 200 or not ct.startswith("text/html"):
                continue
            text = body.decode("utf-8", "replace")
            self.pages[save] = text
            for href in HREF_RE.findall(text) + re.findall(r'\bsrc="([^"]*)"', text) + re.findall(r'\bsrcset="([^"]*)"', text):
                for h in (x.strip().split(" ")[0] for x in href.split(",")):
                    lp = local_path(h)
                    if not lp:
                        continue
                    u = urllib.parse.urlsplit(lp)
                    p = norm(u.path)
                    if p.startswith("/wrath/art/"):
                        self.art.add(p)
                        continue
                    p = MOVED.get(p, p)
                    if u.query:
                        fp = filter_path(p, urllib.parse.parse_qs(u.query))
                        if fp and fp not in seen and len(self.filters) < MAX_FILTER_PAGES:
                            self.filters[fp] = fp
                            filt_todo.append((p + "?" + u.query, fp))
                    if portal_page(p) and not dropped(p) and p not in seen and not ASSET_EXT.search(p):
                        todo.append(p)
        self.say(f"pages: {len(self.pages)} ({len(self.filters)} filter pages)")

    def _finish(self, portal: Portal) -> None:
        have = set(self.pages) | set(MOVED) | {"/" + f[:-len("/index.html")] for f in self.files if f.endswith("/index.html")}
        have |= {"/" + f for f in self.files}
        have.discard("/404")
        tidy = Tidy(have, self.filters)
        for path, text in sorted(self.pages.items()):
            page = strip_noindex(tidy_frame(text))
            tidy.prose = path.startswith("/wrath/docs")
            if tidy.prose and DOCS_NOTE_WORDS.search(re.sub(r"<[^>]+>", " ", text.split('id="main"', 1)[-1])):
                page = re.sub(r"(<h1\b[^>]*>.*?</h1>)", lambda m: m.group(1) + DOCS_NOTE + DOCNOTE_CSS, page, count=1, flags=re.S)
            page = tidy.page(page)
            for src in re.findall(r'\b(?:src|href)="(/wrath/art/[^"?#]*)', page) + \
                    [x.strip().split(" ")[0] for s in re.findall(r'\bsrcset="([^"]*)"', page) for x in s.split(",")]:
                if src.startswith("/wrath/art/"):
                    self.art.add(norm(src))
            for u in re.findall(r"url\((?:'|\")?(/wrath/art/[^)'\"?#]+)", page):
                self.art.add(norm(u))
            if path == "/404":
                self.put_file("404.html", page)
            else:
                self.put_file(out_file(path), page)
        for old, new in list(MOVED.items()) + list(CLOSED.items()) + list(ELSEWHERE.items()):
            if out_file(old) not in self.files:
                self.put_file(out_file(old), moved_page(new))
        # the art the pages use, and the Looks pictures the launcher shows (ui: /wrath/art/looks/look-<name>.webp)
        looks = STREAM / "www-files" / "art" / "looks"
        if looks.is_dir():
            self.art |= {f"/wrath/art/looks/{p.name}" for p in looks.iterdir() if p.is_file()}
        css_seen: set[str] = set()
        self.art = {a for a in self.art if a.rsplit("/", 1)[-1] not in PRIVATE_ART}
        for a in sorted(self.art):
            st, ct, body, _ = portal.get(urllib.parse.quote(a))
            if st != 200:
                continue
            self.put_file(out_file(a), body)
            if a.endswith(".css") and a not in css_seen:
                css_seen.add(a)
                for u in re.findall(rb"url\((?:'|\")?(/wrath/art/[^)'\"?#]+)", body):
                    u = norm(u.decode())
                    if u not in self.art and u.rsplit("/", 1)[-1] not in PRIVATE_ART:
                        st2, _, b2, _ = portal.get(urllib.parse.quote(u))
                        if st2 == 200:
                            self.put_file(out_file(u), b2)
        # the shared stylesheet (its link carries ?v=<hash>; Pages ignores the query)
        if "wrath/art/site.css" not in self.files:
            st, ct, body, _ = portal.get("/wrath/art/site.css")
            if st == 200:
                self.put_file("wrath/art/site.css", body)
                for u in re.findall(rb"url\((?:'|\")?(/wrath/art/[^)'\"?#]+)", body):
                    u = norm(u.decode())
                    if u.rsplit("/", 1)[-1] in PRIVATE_ART:
                        continue
                    st2, _, b2, _ = portal.get(urllib.parse.quote(u))
                    if st2 == 200:
                        self.put_file(out_file(u), b2)
        # the favicon at the root, robots, sitemap, the domain
        ico = self.files.get("wrath/art/leyline-favicon-32.png")
        if ico:
            self.put_file("favicon.png", ico)
        pages = sorted({"/" + f[:-len("index.html")].rstrip("/") if f != "index.html" else "/"
                        for f in self.files if f.endswith("index.html")} - {"/" + o.strip("/") for o in list(MOVED) + list(CLOSED) + list(ELSEWHERE)})
        pages = [p for p in pages if not any(p.startswith(fp + "/") or p == fp for fp in self.filters)]
        self.put_file("robots.txt", f"User-agent: *\nAllow: /\nSitemap: {ORIGIN}/sitemap.xml\n")
        self.put_file("sitemap.xml", '<?xml version="1.0" encoding="UTF-8"?>\n'
                      '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
                      + "".join(f"  <url><loc>{esc(ORIGIN + (p if p == '/' else p + '/'))}</loc></url>\n" for p in pages)
                      + "</urlset>\n")
        self.put_file("CNAME", DOMAIN + "\n")
        self.put_file(".nojekyll", "")

    def _write(self) -> None:
        out = self.out
        if out.exists():
            if not (out / "CNAME").is_file() and any(out.iterdir()):
                raise Refused(f"{out} is not empty and is not a site this build made (no CNAME): choose another --out")
            shutil.rmtree(out)
        out.mkdir(parents=True)
        for rel, data in sorted(self.files.items()):
            p = out / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(data)
        self.say(f"wrote {len(self.files)} files, {sum(len(v) for v in self.files.values()) / 1e6:.1f} MB -> {out}")


def stamp_of(arg: str | None) -> str:
    if arg:
        return arg
    sde = os.environ.get("SOURCE_DATE_EPOCH", "").strip()
    t = int(sde) if sde.isdigit() else int(newest_input())
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(t))


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Build Leyline's website as plain files for GitHub Pages.")
    ap.add_argument("--out", required=True, help="the folder to write (made fresh each time)")
    ap.add_argument("--stamp", help="the time the feeds carry (default: SOURCE_DATE_EPOCH, else the newest input)")
    ap.add_argument("--list", action="store_true", help="print every file and its size")
    ap.add_argument("--quiet", action="store_true")
    a = ap.parse_args(argv)
    try:
        b = Build(Path(a.out).resolve(), stamp_of(a.stamp), quiet=a.quiet)
        got = b.run()
    except Refused as e:
        print(f"SITE BUILD STOPPED: {e}", file=sys.stderr)
        return 1
    if a.list:
        for rel, n in got["files"].items():
            print(f"{n:>9}  {rel}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
