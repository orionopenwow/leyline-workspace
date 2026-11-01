"""The hidden Leyline hub (stream/hub.py) on the portal: every route answers 200 with noindex (meta and
X-Robots-Tag), the catalog merges the Workshop, the community index and themes (a duplicate shows once), "Submit a
mod" queues without personal data, the admin console's decisions apply (approved.json, delist), the launcher feed
keeps the all.json shape, the front-door switch, and no current page links to the preview.

    python3 -I stream/tests/test_hub.py stream/wrath_portal.py
"""
import html, http.client, importlib.util, json, os, re, shutil, sys, tempfile, threading, time, urllib.parse
from http.server import ThreadingHTTPServer

PAGE_PORT = 18231
PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
HERE = os.path.dirname(PORTAL)
REPO = os.path.dirname(HERE)
DATA = tempfile.mkdtemp(prefix="wu-hub-data-")
HUBDIR = tempfile.mkdtemp(prefix="wu-hub-dir-")
shutil.copy(os.path.join(HERE, "hub", "community-index.json"), HUBDIR)
json.dump({"format": 1, "themes": [{"id": "night-sky", "name": "Night Sky", "description": "Deep blue and stars.",
                                    "author": "Leyline", "licence": "CC-BY-4.0"},
                                   {"id": "bad", "name": "WoW theme"}, {"id": "../x", "name": "Path"}]},
          open(os.path.join(HUBDIR, "themes.json"), "w"))
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example",
                  WU_RELAY_LISTING="http://127.0.0.1:9/realms.json", WU_PORTAL_DATA=DATA, WU_HUB_DIR=HUBDIR,
                  WU_MODS_DIR=tempfile.mkdtemp(), WU_WORKSHOP_ITEMS_DIR=tempfile.mkdtemp())
os.environ.pop("WU_HUB_FRONT", None)
os.environ.pop("WU_HUB", None)

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1; print("PASS", name)
    else:
        failed += 1; print("FAIL", name, extra)


BANNED = re.compile(r"(?i)world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard")


def visible(p):
    p = re.sub(r"<style.*?</style>", " ", p, flags=re.S)
    p = re.sub(r"<!--.*?-->", " ", p, flags=re.S)
    attrs = " ".join(re.findall(r'(?:alt|title|content|aria-label|placeholder|value)="([^"]*)"', p))
    return re.sub(r"<[^>]+>", " ", p) + " " + attrs


spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wp)
wp.realm_status = lambda: {"online": True, "players": 3, "bots": 10}
HB = wp.hub_mod
check("hub.py loaded beside the portal", HB is not None)
if HB is None:
    print(f"\n{passed}/{passed + failed} passed"); sys.exit(1)

srv = ThreadingHTTPServer(("127.0.0.1", PAGE_PORT), wp.PageHandler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()


def req(path, method="GET", body=None, ip="10.9.0.1"):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=30)
    hd = {"X-Forwarded-For": ip}
    if body is not None:
        hd["Content-Type"] = "application/x-www-form-urlencoded"
    c.request(method, path, body=body, headers=hd)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


INDEX = json.load(open(os.path.join(HUBDIR, "community-index.json")))
first = INDEX["entries"][0]
pages = ["/wrath/leyline-preview", "/wrath/leyline-preview/", "/wrath/leyline-preview/catalog",
         "/wrath/leyline-preview/catalog?build=1.12", "/wrath/leyline-preview/catalog?build=5.4.8&sort=name",
         "/wrath/leyline-preview/catalog?cat=themes", "/wrath/leyline-preview/catalog?source=index&page=2",
         "/wrath/leyline-preview/catalog?q=%3Cscript%3E&cat=nope&build=9&sort=x&page=-4",
         "/wrath/leyline-preview/creators", "/wrath/leyline-preview/beacon", "/wrath/leyline-preview/submit",
         "/wrath/leyline-preview/forge", "/wrath/leyline-preview/agents",
         "/wrath/leyline-preview/realms", "/wrath/leyline-preview/modes", "/wrath/leyline-preview/news",
         "/wrath/leyline-preview/gallery", "/wrath/leyline-preview/gallery?f=mods", "/wrath/leyline-preview/gallery?f=%3Cx%3E",
         "/wrath/leyline-preview/p/" + first["id"].replace(":", "-"), "/wrath/leyline-preview/p/theme-night-sky"]
for p in pages:
    st, h, b = req(p)
    s = b.decode()
    check(f"GET {p}: 200 html", st == 200 and h.get("content-type", "").startswith("text/html"), st)
    check(f"GET {p}: noindex header and meta", h.get("x-robots-tag") == "noindex, nofollow"
          and '<meta name="robots" content="noindex">' in s)
    csp = h.get("content-security-policy", "")
    if p.rstrip("/").split("?")[0] in ("/wrath/leyline-preview", "/wrath/leyline-preview/realms", "/wrath/leyline-preview/gallery"):
        # the front page, the realm list and the gallery: one small script, under this response's nonce, reading only this site
        nm = re.search(r"'nonce-([A-Za-z0-9_-]{16,})'", csp)
        gal = p.split("?")[0].endswith("/gallery")
        check(f"GET {p}: one script, under the CSP nonce, connect-src 'self'", nm and s.count("<script") == 1
              and f'<script nonce="{nm.group(1)}">' in s and ("connect-src" not in csp if gal else "connect-src 'self'" in csp)
              and "unsafe-inline" not in csp.split("script-src")[1].split(";")[0]
              and "<script src" not in s, csp)
        check(f"GET {p}: no brand words", not BANNED.findall(visible(re.sub(r"<script.*?</script>", " ", s, flags=re.S))))
    else:
        check(f"GET {p}: no script, no brand words", "<script" not in s and "script-src 'none'" in csp
              and not BANNED.findall(visible(s)), BANNED.findall(visible(s))[:5])
    front_p = p.rstrip("/") == "/wrath/leyline-preview"
    check(f"GET {p}: the hub's own header (mark, wordmark, Sign In) and, below the front page, its sub-nav",
          'class="hb-top"' in s and 'class="ly-spin slow hb-mark"' in s and "<b>Leyline</b><i>Launcher</i>" in s
          and 'class="hb-signin" href="/login?next=%2Fwrath"' in s and ('class="hb-sub"' in s) != front_p
          and 'id="ly-site-nav"' not in s and 'class="hb-foot"' in s)
    check(f"GET {p}: the flat look, none of the painted art", 'data-look="leyline"' in s and "ly-spin-turn" in s and not re.search(
          r"leyline-(?:bg|emblem|divider|corner)|wu-(?:theme|card|corner|dragon|divider)|Cinzel|Leyline Display", s))
    check(f"GET {p}: no Support link, no other social sites", not re.search(r">\s*Support\s*<|facebook|twitter|instagram|x\.com", s, re.I))
st, h, _ = req("/wrath/leyline-preview", method="HEAD")
check("HEAD works", st == 200 and h.get("x-robots-tag"))
for p in ("/wrath/leyline-preview/p/link-000000000000", "/wrath/leyline-preview/p/../../etc", "/wrath/leyline-preview/nope"):
    st, h, b = req(p)
    check(f"GET {p}: 404, still noindex", st == 404 and h.get("x-robots-tag") == "noindex, nofollow", st)

front = req("/wrath/leyline-preview")[2].decode()
check("front page: no Browse by client chips (no client numbers on the hub)", "catalog?build=" not in front)
# 4.3.4 and 5.4.8 are not offered for now (the owner, 2026-10-30): no chips, no items made only for them
check("offered builds: capped at 3.3.5a", HB.offered_builds({}) == ["1.12", "2.4.3", "3.3.5a"]
      and HB.offered_builds({"LEYLINE_ALL_CLIENTS": "1"}) == HB.BUILDS)
check("front page: no 4.3.4 / 5.4.8 chips", all(f"catalog?build={b}" not in front for b in ("4.3.4", "5.4.8")))
check("offered_only: hidden-only items go, mixed ones keep the offered builds",
      HB.offered_only({"builds": ["4.3.4", "5.4.8"]}, HB.offered_builds({})) is None
      and HB.offered_only({"builds": ["3.3.5a", "4.3.4"]}, HB.offered_builds({}))["builds"] == ["3.3.5a"]
      and HB.offered_only({"builds": ["4.3.4"]}, HB.offered_builds({"LEYLINE_ALL_CLIENTS": "1"})) is not None)
check("offered_tag: a hidden build is no filter", HB.offered_tag("5.4.8") == "" and HB.offered_tag("cata") == ""
      and HB.offered_tag("1.12.1") == "1.12" and HB.build_tag("5.4.8") == "5.4.8")
_all = req("/wrath/leyline-preview/catalog?view=all")[2].decode()
check("Mods (Browse everything): its categories as chips (those with something in them), the tabs Mods, Addons, Themes",
      'catalog?group=game' in _all and 'catalog?group=realm' in _all and 'catalog?group=tools' in _all
      and 'href="/wrath/leyline-preview/addons"' in _all and 'catalog?cat=themes' in _all and "catalog?cat=addons" not in _all
      and all(req(f"/wrath/leyline-preview/catalog?cat={c}")[0] == 200 for c in HB.CATEGORY_IDS if c != "addons"))
check("front page: creators and Host a Beacon", "/wrath/leyline-preview/creators" in front and "/wrath/leyline-preview/beacon" in front)
check("front page: a TODO(owner) for the tagline, no words in the owner's voice", "TODO(owner)" in front)
# the owner's mockup (2026-10-30): header nav, the hero, Platform Status, Community Spotlight, the footer
# the header mirrors the launcher's pages (owner 2026-10-30: "servers, mods, addons, etc ... it should all work in browser too")
NAV_ORDER = ["Play", "Realms", "Mods", "Addons", "Host", "Forum", "More"]   # owner 2026-10-30: "change community to forum"


def header_of(page):
    hd = re.search(r'<header class="hb-top">.*?</header>', page, re.S).group(0)
    nav = re.search(r'<nav class="hb-nav"[^>]*>(.*?)</nav>', hd, re.S).group(1)
    top = re.sub(r'<div class="hb-more-m">.*?</div>|<div class="hb-nav-x">.*?</div>', "", nav, flags=re.S)
    labels = re.findall(r'class="hb-ni[^"]*"[^>]*>(?:<svg.*?</svg>)<span>([^<]+)</span>', top, re.S)
    on = re.findall(r'class="hb-ni on"[^>]*>(?:<svg.*?</svg>)<span>([^<]+)</span>', top, re.S)
    return hd, nav, labels, on


hd, nav, labels, on = header_of(front)
check("side menu (owner 2026-10-31: keep the side menu): wide screens get the launcher's side menu with the fold switch, "
      "remembered in this browser; after SITE_CSS so it wins; phones keep the top bar",
      'id="hb-side"' in front and 'class="hb-side-b"' in front and "--hb-side:236px" in front
      and "leyline.side" in HB.HUB_JS and front.count('<header class="hb-top"><a class="hb-brand"') == 1
      and front.index(HB.SITE_CSS.strip()[:40]) < front.index("--hb-side:236px")
      and "@media (max-width:1080px){.hb-side-t,.hb-side-b{display:none}}" in front)
check("the launcher's palette: the generated tokens (the launcher's own blue, Light by the system) before the hub's styles",
      HB.SITE_TOKENS_CSS.strip() in front and "--lk-accent:#5b8def" in HB.SITE_TOKENS_CSS and "#3592cf" not in HB.SITE_TOKENS_CSS
      and "@media (prefers-color-scheme:light){:root{--lk-bg:#eef1f6" in HB.SITE_TOKENS_CSS
      and front.index("--lk-accent:#5b8def") < front.index(".hb-skip{") < front.index(".hb-pg{")
      and '<meta name="color-scheme" content="dark light">' in front)
check("header: Play, Realms, Mods, Addons, Host, Forum, More (Modes, Gallery and Status out of the menu)", labels == NAV_ORDER, labels)
hrefs = dict(re.findall(r'<a class="hb-ni[^"]*" href="([^"]+)"[^>]*><svg.*?</svg><span>([^<]+)</span>', nav, re.S)[i][::-1] for i in range(6))
check("header: each item goes to the launcher page's web twin", hrefs == {
      "Play": "/wrath", "Realms": "/wrath/leyline-preview/realms", "Mods": "/wrath/leyline-preview/catalog",
      "Addons": "/wrath/leyline-preview/addons", "Host": "/wrath/host",
      "Forum": "/wrath/leyline/forum"} and "addons" in HB.CATEGORY_IDS, hrefs)
more = re.search(r'<details class="hb-more-d"><summary class="hb-ni[^"]*">.*?<span>More</span></summary><div class="hb-more-m">(.*?)</div></details>', nav, re.S)
MORE_ITEMS = ["Leaderboards", "Forge", "Agents", "Docs", "News", "Join the Discord", "Creators"]   # Join the Discord under News, as in the launcher
check("header: More is a details dropdown (no script needed) with Forge, Agents, Docs, News, Join the Discord, Creators",
      more and re.findall(r"<span>([^<]+)</span>", more.group(1)) == MORE_ITEMS
      and "#status" not in more.group(1) and "/gallery" not in more.group(1))
check("header: the phone sheet has the same items as tiles, and the download", re.findall(
      r"<span>([^<]+)</span>", re.search(r'<div class="hb-nav-x">(.*?)</div>', nav, re.S).group(1)) == MORE_ITEMS
      and 'class="hb-dl-x"' in nav and 'id="hb-menu" class="hb-menu-t"' in hd and '<label for="hb-menu" class="hb-menu-b"' in hd)
check("header: a line icon by every item (inline SVG, currentColor)", nav.count('<svg viewBox="0 0 24 24" aria-hidden="true">') == 6 + 1 + 7 + 7
      and "stroke:currentColor" in front)
check("header: Download (primary) and Log In (the launcher's word; the crystal on the folded rail) on the right, no item active on the front page",
      re.search(r'<div class="hb-end"><a class="hb-dl" href="https://github\.com/orionopenwow/Leyline/releases/download/launcher-latest/Leyline\.exe"[^>]*>.*?<span>Download</span></a>'
                r'<a class="hb-signin" href="/login\?next=%2Fwrath"><img src="/wrath/art/leyline-crystal-128.png" alt="" width="26" height="26"><span>Log In</span></a>', hd, re.S)
      and on == [] and 'aria-current="page"' not in hd)
check("header: Join the Discord under News (the launcher's side menu), the portal's link, opened safely",
      re.search(r'<a class="hb-ni hb-ni-dc" href="%s" rel="noopener noreferrer" title="Join the Discord"><svg' % re.escape(html.escape(wp.DISCORD_URL)), nav)
      and nav.index('<span>News</span>') < nav.index('<span>Join the Discord</span>') < nav.index('<span>Creators</span>'))
check("the launcher's stone (launcher/ui/stone.css): the side menu on the tall tile, the bars on the strip, the current page a "
      "sunken well, Log In and Download on the red stone in Marcellus, panels in the grey metal edge; the art allowed by the portal",
      "--hb-stone-tall:url(/wrath/art/stone-side.webp) 0 0/256px 256px repeat" in front and "var(--hb-stone)" in HB.STONE_CSS
      and "border-image:url(/wrath/art/stone-btn.webp) 28 30 fill/11px stretch" in front and "url(/wrath/art/marcellus-400.woff2)" in front
      and "border-image:url(/wrath/art/window-edge.png) 19/6px round" in front and "inset 0 2px 4px rgba(0,0,0,.6)" in front
      and front.index(".hb-pg{") < front.index("--hb-stone-tall")
      and all(f'"{n}":' in open(PORTAL).read() for n in ("stone-bar.webp", "stone-side.webp", "stone-btn.webp", "window-edge.png", "marcellus-400.woff2", "Marcellus-OFL.txt"))
      and all(os.path.isfile(os.path.join(HERE, "www-files", "art", n)) for n in ("stone-bar.webp", "stone-side.webp", "stone-btn.webp", "window-edge.png", "marcellus-400.woff2", "Marcellus-OFL.txt")))
# phones (owner 2026-10-31: "sideways mode recommended on mobile, gentle encouragement ... give sidebar tool bar, match launcher view")
cat_pg = req("/wrath/leyline-preview/catalog")[2].decode()
check("phones upright: a gentle turn-your-phone tip under the top bar, Not now as a plain link (no script on the page)",
      '<aside class="hb-turn" aria-label="Tip">' in cat_pg and "Turn your phone sideways for the full view" in cat_pg
      and 'class="hb-turn-x" href="/wrath/leyline-preview/turn-later?next=/wrath/leyline-preview/catalog" rel="nofollow">Not now</a>' in cat_pg
      and "<script" not in cat_pg and cat_pg.index('class="hb-top"') < cat_pg.index('class="hb-turn"') < cat_pg.index('<main id="main"'))
check("the tip shows only on a touch phone in portrait; JS remembers Not now for 7 days (localStorage, the ly_turn cookie)",
      ".hb-turn,.hb-playx{display:none}" in HB.STONE_MOBILE_CSS and "@media (orientation:portrait) and (max-width:600px) and (pointer:coarse)" in HB.STONE_MOBILE_CSS
      and 'localStorage.setItem(TK' in HB.HUB_JS and "ly_turn=1; Max-Age=" in HB.HUB_JS and "7*864e5" in HB.HUB_JS and "STONE_MOBILE_CSS" not in cat_pg)
st_t, h_t, _ = req("/wrath/leyline-preview/turn-later?next=/wrath/leyline-preview/catalog")
st_e, h_e, _ = req("/wrath/leyline-preview/turn-later?next=//evil.example/x")
check("Not now without script: 303 back to the page with the 7-day cookie; never to another site",
      st_t == 303 and h_t.get("location") == "/wrath/leyline-preview/catalog" and "ly_turn=1; Max-Age=604800; Path=/" in h_t.get("set-cookie", "")
      and st_e == 303 and h_e.get("location") == "/wrath/leyline-preview")
_c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=30)
_c.request("GET", "/wrath/leyline-preview/catalog", headers={"X-Forwarded-For": "10.9.0.1", "Cookie": "x=1; ly_turn=1"})
_r = _c.getresponse(); _off = _r.read().decode()
check("once dismissed (the cookie): no tip on the page", _r.status == 200 and 'class="hb-turn"' not in _off and 'class="hb-top"' in _off)
check("phones sideways: the launcher's view: a stone icon rail with tiny labels, a compact stone top bar with Play on the PLAY stone",
      "@media (orientation:landscape) and (max-height:500px) and (pointer:coarse)" in HB.STONE_MOBILE_CSS
      and "var(--hb-stone-tall)" in HB.STONE_MOBILE_CSS and "body.hb .hb-nav .hb-ni{flex:none;display:flex;flex-direction:column" in HB.STONE_MOBILE_CSS
      and '<a class="hb-playx" href="/wrath">Play</a></div></header>' in cat_pg and "play-stone.webp" in HB.STONE_MOBILE_CSS
      and "STONE_CSS + STONE_MOBILE_CSS + DARK_ISLANDS" in open(os.path.join(HERE, "hub.py")).read())
check("the stone chrome stays dark in Light (its words light on the stone)", ".hb-top,.hb-foot,.hb-pg-h" in HB.DARK_ISLANDS and ".hb-top,.hb-foot,.hb-pg-h" in front)
check("header: sticky with a blur, a hairline once scrolled, none of it moving with reduced motion",
      "backdrop-filter:blur(14px)" in front and "animation-timeline:scroll(root)" in front and ".hb-top.hb-scrolled{" in front
      and "hb-scrolled" in HB.HUB_JS and re.search(r"prefers-reduced-motion:reduce\)\{[^}]*animation:none!important\}.*?\.hb-top\{border-bottom-color", front, re.S))
check("hero: the whole mark leads, crisp (the 512px layers), big (~400px, 220px on a phone), turning slowly; the heading a step down",
      'class="ly-spin slow hb-hero-mark"' in front and "leyline-swirl-512.png" in front and "leyline-crystal-512.png" in front
      and "--ly-spin-size:clamp(220px,min(34vw,42vh),400px)" in front and ".hb-hero h1{margin:0 0 8px;font-size:clamp(22px,3vw,32px)" in front
      and "hb-crystal" not in front
      and "Every client, every server" in front and "One launcher for every classic client and every server" in front
      and "hb-bullets" not in front and "TODO(owner): the hero heading" in front
      and 'class="hb-hero-a"' not in front and 'class="hb-dl" href="https://github.com/orionopenwow/Leyline/releases/download/launcher-latest/Leyline.exe"' in front)
stats = re.search(r'<section class="hb-sbar hb-rv" id="status".*?</section>', front, re.S).group(0)
check("Platform status: one slim bar of pills, the main realm online from realm_status, sign-in, the network, the relay, the mod catalog",
      all(x in stats for x in ('<em class="ok">OK</em> · 3 playing', "<b>Sign-in</b>", "<b>Realm network</b>", "<b>Relay</b>", "<b>Mods</b>", " items</span>"))
      and '<li><a class="hb-pill ok" href="/wrath/realms"><i class="hb-pdot" aria-hidden="true"></i>' in stats
      and '<a class="hb-pill info" href="/wrath/leyline-preview/catalog">' in stats and 'class="hb-stat' not in front
      and '<h2 id="hb-st" class="hb-sbar-h">' in stats and re.search(r'<p class="hb-sbar-s">(All systems normal|Some systems are down|Systems are down)</p>', stats), stats)
check("Platform status: the pills wrap on a phone (never wider than the page), dots in the status colours",
      "@media (max-width:760px){.hb-pill{display:inline-block;white-space:normal" in front and ".hb-pills{display:flex;flex-wrap:wrap" in front
      and ".hb-pdot.ok,.hb-pill.ok .hb-pdot{background:var(--lk-ok)}" in front)
check("Platform status: a failing source is red, never a made-up OK", '<em class="bad">' in stats and '<span class="hb-pill bad"><i class="hb-pdot" aria-hidden="true"></i><b>Sign-in</b><span><em class="bad">Offline</em>' in stats)
check("Platform status: no client downloads (we never download clients)", "Client" not in stats and "download" not in stats.lower())
spot = re.search(r'<section class="hb-sec hb-cm hb-rv" id="forum".*?</section>', front, re.S).group(0)
spots = re.findall(r'<li><a class="hb-hot-r" href="/wrath/leyline-preview/p/([^"]+)"><span class="hb-hot-n" aria-hidden="true">(\d)</span>'
                   r'<span class="hb-hot-pic">.*?</span><span class="hb-hot-t"><b>([^<]+)</b><i>by ([^<]+?)(?: · \d+ installs? this month)?</i></span>'
                   r'<span class="hb-hot-k">([^<]+)</span></a></li>', spot, re.S)
names = {e["id"].replace(":", "-"): (HB.neutral(e["name"]), HB.neutral(e["author"])) for e in INDEX["entries"]}
check("Forum and Discord, Hot right now: a ranked list of five real items (rank, name, author, kind), each linking to its page",
      [x[1] for x in spots] == ["1", "2", "3", "4", "5"] and all(k in ("Mod", "Addon", "Game mode", "Theme") for *_, k in spots)
      and all(names.get(slug) == (html.escape(HB.neutral(n)), html.escape(HB.neutral(a))) or names.get(slug) == (HB.neutral(n), HB.neutral(a))
              for slug, _, n, a, _ in spots if slug in names) and any(slug in names for slug, *_ in spots), spots)
check("Forum and Discord: one section (the Discord, the latest threads, News, Hot right now, Top creators), no repeated big cards, no separate spotlight, creators or news sections",
      'class="hb-spot"' not in front and 'class="hb-cr"' not in front and "Community Spotlight" not in front and "Creator spotlight" not in front
      and front.count('id="forum"') == 1 and 'id="community"' not in front and '<h2 id="hb-cm-h">Forum and Discord</h2>' in spot and 'id="creators-top"' in spot
      and spot.index('id="hb-hot-h"') < spot.index('id="creators-top"') and front.count("<section") == front.count("</section>"))
check("Forum and Discord: two columns on wide screens, one on a phone",
      ".hb-cm-g{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr)" in front
      and "@media (max-width:900px){.hb-cm-g{grid-template-columns:minmax(0,1fr)}}" in front)
foot = re.search(r'<footer class="hb-foot">.*?</footer>', front, re.S).group(0)
check("footer: Docs, Download, Forum, Status, the wordmark, Join the Discord and GitHub only",
      all(f">{x}<" in foot for x in ("Docs", "Download", "Forum", "Status", "GitHub")) and ">Community<" not in foot and "<b>Leyline</b>" in foot
      and f'href="{wp.DISCORD_URL}" rel="noopener noreferrer"' in foot and "</svg>Join the Discord</a>" in foot)
# launch (2026-10-30): the official realm (Wrath Unleashed) is the front door, the Discord everywhere
ho = re.search(r'<section class="hb-ho[^"]*" id="hangout".*?</section>', front, re.S)
check("launch: under the hero, Wrath Unleashed is open, Play in your browser (no Discord button here: two Discord links at most)",
      ho and "<h2 id=\"hb-ho-h\">Wrath Unleashed is open</h2>" in ho.group(0)
      and '<a class="hb-cta sm" href="/wrath">Play in your browser</a>' in ho.group(0)
      and f'href="{wp.DISCORD_URL}"' not in ho.group(0) and "TODO(owner): the Hangout line" in ho.group(0)
      and front.index('class="hb-hero"') < front.index('id="hangout"') < front.index('id="realms"'))
cm = re.search(r'<section class="hb-sec hb-cm hb-rv" id="forum".*?</section>', front, re.S)
check("launch: the Forum and Discord section opens with a big Join the Discord and links the forum",
      cm and cm.group(0).index('class="hb-cm-dc big"') < cm.group(0).index('class="hb-cm-g') and 'href="/wrath/leyline/forum"' in cm.group(0) and f'href="{wp.DISCORD_URL}"' in cm.group(0))
_main = re.sub(r'<footer class="hb-foot">.*?</footer>|<header class="hb-top">.*?</header>', "", front, flags=re.S)
check("launch: one Discord call to action in the page body (the site chrome has the other link)",
      _main.count(f'href="{wp.DISCORD_URL}"') == 1, _main.count(f'href="{wp.DISCORD_URL}"'))
check("the official realm: no Temporary chip, the start line", 'hb-bdg">Temporary<' not in front
      and HB.OFFICIAL_START in re.search(r'<li class="hb-rl home.*?</li>', front, re.S).group(0))
check("how it works links the plain network explainer", 'class="hb-how-more"><a href="/wrath/docs/network">' in front)
check("launch: the Hangout row comes first, named, labelled Official realm",
      re.search(r'<li class="hb-rl home[^"]*" data-home="1"[^>]*>.*?<b>Wrath Unleashed</b><span class="hb-rl-k">Official realm</span>', front))
# the owner's 2026-10-30 additions: realms under the hero, the feature sections in order, how it works, the polish
FS_RE = (r'<section class="hb-fs hb-rv( flip)?" id="f-([a-z]+)".*?<p class="hb-eyebrow">.*?</span>([^<]+)</p><h2[^>]*>([^<]+)</h2>'
         r'<p class="hb-lede">([^<]+)</p><ul class="hb-points">(.*?)</ul><p class="hb-fs-a">(.*?)</p></div>'
         r'<div class="hb-fs-v">(.*?)</div></section>')
# owner 1 Nov ("make sure the website is clean looking too"): the front page shows four of them (FRONT_FEATURES), the
# launcher's story; the rest are drawn by features(only=None) and checked from there
allf = HB.hub.features(HB.hub.realm_rows(), only=None)
check("front page: four feature sections only (launcher, host, modes, mods), alternating, nothing repeating the realm list",
      [(f[1], bool(f[0])) for f in re.findall(FS_RE, front, re.S)] == [("launcher", False), ("host", True), ("modes", False), ("mods", True)]
      and 'id="f-realms"' not in front and 'id="f-forge"' not in front and 'id="f-agents"' not in front and 'id="f-community"' not in front
      and "Your Den" in front, [f[1] for f in re.findall(FS_RE, front, re.S)])
fs = re.findall(FS_RE, allf, re.S)
check("feature sections: one per part of Leyline, in the recommended order", [f[1] for f in fs] == [
      "launcher", "host", "realms", "modes", "mods", "forge", "agents", "community"] and [f[2] for f in fs] == [
      "Desktop launcher", "Server hosting software", "Realms &amp; hopping", "Minigolf &amp; game modes", "Mods",
      "Forge", "Agents", "Community hub"], [f[1:3] for f in fs])
check("feature sections: alternating sides (every other one flips)", [bool(f[0]) for f in fs] == [False, True] * 4)
check("feature sections: a heading, a lede, points and a first link each", all(f[3] and f[4] and f[5].count("<li>") >= 2
      and 'class="hb-cta sm"' in f[6] for f in fs))
hrefs = [re.findall(r'href="([^"]+)"', f[6]) for f in fs]
check("feature sections: links to pages that exist, nothing paid", [h[0] for h in hrefs][1:] == [
      "/wrath/host", "/wrath/leyline-preview/realms", "/wrath?go=mode:minigolf", "/wrath/leyline-preview/catalog?sort=hot",
      "/wrath/leyline-preview/forge", "/wrath/leyline-preview/agents", "/wrath/leyline/forum"] and hrefs[0][0].startswith("https://github.com/orionopenwow/Leyline/releases")
      and not re.search(r"(?i)support|donat|\bvip\b|\bshop\b", " ".join(f[6] for f in fs)))
check("feature sections: a TODO(owner) to reorder them", "TODO(owner): the order of the products" in open(HB.__file__).read())
check("feature sections: the game modes line from playmodes.json, Hero Key as coming", "Minigolf" in fs[3][4]
      and "Hero Key sign-in across community realms (coming)" in fs[2][5])
check("feature sections: every one has its icon", allf.count('<span class="hb-ico"><svg viewBox="0 0 24 24" aria-hidden="true">') == 8)
feats = allf
shots = re.findall(r'<img src="/wrath/art/(leyline-ui-[a-z]+\.webp)" alt="([^"]+)" width="1240" height="760" loading="lazy"', feats)
check("feature sections: the launcher's own pages as pictures (served, lazy, with alt text), our own panels for the rest "
      "(server software, the realm list; the Minigolf loop falls back to the Modes page without the game shots)",
      [n for n, _ in shots] == ["leyline-ui-home.webp", "leyline-ui-modes.webp", "leyline-ui-workshop.webp"]
      and 'class="hb-sw"' in fs[1][7] and 'class="hb-rlx"' in fs[2][7]
      and all(a for _, a in shots) and all(n in wp.ART_TYPES and not n.startswith("leyline-shot-") for n, _ in shots)
      and all(os.path.isfile(os.path.join(HERE, "www-files", "art", n)) for n, _ in shots)
      and 'class="hb-draw hb-people"' in fs[7][7] and 'class="hb-draw hb-code"' in fs[5][7] and 'class="hb-draw hb-code"' in fs[6][7]
      and not re.search(r"leyline-shot-|wu-load-orion|painted", feats, re.I) and not re.search(r"wu-load-orion|painted", front, re.I))
how = re.search(r'<ol class="hb-how[^"]*" aria-label="How it works">(.*?)</ol>', front, re.S)
check("how it works: three steps, launcher, game folder or browser, realm", how and re.findall(r"<b>([^<]+)</b>", how.group(1)) == [
      "Get the launcher", "Point it at your game folder", "Pick a realm"] and "play in your browser" in how.group(1)
      and "TODO(owner): the \"how it works\" wording" in front)
order = [front.index(x) for x in ('class="hb-hero"', 'id="realms"', 'class="hb-how', 'id="features"', 'id="f-launcher"', 'id="f-mods"',
                                  'id="forum"', 'id="status"')] + [front.rindex('class="hb-fine"')]
check("front page order: hero, realms, how it works, the feature sections, forum and Discord, the status bar, the fine print", order == sorted(order), order)
check("realm list without a relay: our realm still there, one click", 'data-home="1"' in front and 'class="hb-play" href="/wrath"' in front
      and f'>{wp.PUBLIC_HOST}</code>' in front and "may be a little out of date" in front)
check("polish: hover lift and the reveal stop for reduced motion", "@media (prefers-reduced-motion:reduce)" in front
      and "animation:none!important" in front and ".hb-js .hb-wait{" in front and "focus-visible{outline:2px" in front)
check("polish: the script reads only this site's public JSON", re.findall(r'json\("([^"]+)"\)', HB.HUB_JS) == ["/wrath/realms.json", "/wrath/status.json"]
      and "innerHTML" not in HB.HUB_JS and "eval" not in HB.HUB_JS)
check("sub-nav: Realms, Modes and News", all(f'href="/wrath/leyline-preview/{x}"' in req("/wrath/leyline-preview/catalog")[2].decode()
                                               for x in ("realms", "modes", "news")))
for path, want in (("/realms", ["Realms"]), ("/catalog", ["Mods"]), ("/addons", ["Addons"]), ("/catalog?cat=tools", ["Mods"]),
                   ("/modes", []), ("/beacon", ["Host"]), ("/submit", ["Mods"]), ("/p/" + first["id"].replace(":", "-"), ["Addons" if HB.shelf_of(first) == "addons" else "Mods"]),
                   ("/news", ["More"]), ("/creators", ["More"]), ("", [])):
    pg = req("/wrath/leyline-preview" + path)[2].decode()
    hd, nav, labels, on = header_of(pg)
    more_on = 'class="hb-ni on"><svg' in re.search(r"<summary.*?</summary>", nav, re.S).group(0)
    sub_on = re.findall(r'class="hb-mi on" href="[^"]+" aria-current="page" title="[^"]+"><svg.*?</svg><span>([^<]+)</span>', nav, re.S)
    check(f"header on {path or '/'}: the same items, {want or sub_on or 'nothing'} active", labels == NAV_ORDER and on == want
          and (more_on == bool(sub_on)) and (sub_on == {"/news": ["News"], "/creators": ["Creators"]}.get(path, []))
          and hd.count('aria-current="page"') == len([w for w in want if w != "More"]) + 2 * len(sub_on), (labels, on, sub_on))   # (the dropdown's and the phone sheet's copy: one shows)
modes = req("/wrath/leyline-preview/modes")[2].decode()
mode_ids = [m["id"] for m in wp.playmodes.entries()]
check("modes page: every mode in playmodes.json, live realm modes one click", all(f'id="mode-{i}"' in modes for i in mode_ids)
      and 'class="hb-play" href="/wrath?go=mode:minigolf"' in modes and "Azeroth Survivors" in modes
      and 'go=mode:azeroth-survivors' not in modes and "Coming soon" in modes and 'href="/wrath/host">Host it' in modes)
check("modes page: no script", "<script" not in modes)
news = req("/wrath/leyline-preview/news")[2].decode()
check("news page: empty says so", "No news posts yet." in news)
os.makedirs(os.path.join(DATA, "leyline"), exist_ok=True)
json.dump({"format": 1, "posts": [{"id": "a", "date": "2026-10-29", "tag": "Launcher 0.3", "title": "Every client", "items": ["One", "Two"]},
                                  {"id": "b", "title": "WoW news", "items": []}]}, open(os.path.join(DATA, "leyline", "news.json"), "w"))
os.environ["WU_LEYLINE_DIR"] = os.path.join(DATA, "leyline")
news = req("/wrath/leyline-preview/news")[2].decode()
if wp.admin is not None:
    check("news page: the admin console's posts, brand words dropped", "Every client" in news and "<li>One</li>" in news
          and "WoW news" not in news)
os.environ["WU_LEYLINE_DIR"] = os.path.join(DATA, "leyline")
os.makedirs(os.environ["WU_LEYLINE_DIR"], exist_ok=True)
rel = "https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.4.0"
json.dump({"stable": {"version": "0.4.0", "tag": "launcher-v0.4.0", "url": rel}}, open(os.path.join(os.environ["WU_LEYLINE_DIR"], "channel.json"), "w"))
if wp.admin is not None and hasattr(wp.admin, "leyline_dir"):
    check("Download Now: the stable release the admin console published", f'href="{rel}"' in req("/wrath/leyline-preview")[2].decode())
json.dump({"stable": {"version": "0.4.0", "tag": "x", "url": "https://evil.example/x"}}, open(os.path.join(os.environ["WU_LEYLINE_DIR"], "channel.json"), "w"))
check("Download Now: never a link off the releases page", "evil.example" not in req("/wrath/leyline-preview")[2].decode())
cat = req("/wrath/leyline-preview/catalog?build=2.4.3")[2].decode()
check("catalog: filtered by build, labelled in words", all(("2.4.3" in e["builds"] or "any" in e["builds"]) for e in INDEX["entries"]
                                      if f'/p/{e["id"].replace(":", "-")}"' in cat) and "for the second era" in cat
      and 'aria-current="true">Second era</a>' in cat and "2.4.3" not in visible(re.sub(r'<input type="hidden"[^>]*>', "", cat)))
cata = req("/wrath/leyline-preview/catalog?build=5.4.8")[2].decode()
check("catalog: no item made only for 4.3.4 / 5.4.8", not any(
    f'/p/{e["id"].replace(":", "-")}"' in cata for e in INDEX["entries"] if not set(e["builds"]) & {"1.12", "2.4.3", "3.3.5a", "any"}))
junk = req("/wrath/leyline-preview/catalog?q=%3Cscript%3E")[2].decode()
check("catalog: the query is escaped", "<script>" not in junk and "&lt;script&gt;" in junk)
item = req("/wrath/leyline-preview/p/" + first["id"].replace(":", "-"))[2].decode()
check("item: View source, credit, licence, no Install", "View source on GitHub" in item and first["licence"] in item
      and 'rel="noopener noreferrer nofollow ugc"' in item and ">Install<" not in item
      and "/download" not in re.sub(r'<header class="hb-top">.*?</header>|<footer class="hb-foot">.*?</footer>', "", item, flags=re.S))   # the menu's own Download is the launcher
check("item: the repo URL is a link, never shown as text", first["repo"] not in visible(item))
theme = req("/wrath/leyline-preview/p/theme-night-sky")[2].decode()
check("themes: listed, bad ones dropped", "Night Sky" in theme and "WoW theme" not in req("/wrath/leyline-preview/catalog?cat=themes")[2].decode())

# ---------------------------------------------------------------- submissions
qfile = os.path.join(DATA, "hub", "submissions.jsonl")
form = "repo=https%3A%2F%2Fgithub.com%2Fnew-author%2Fcool-mod&build=2.4.3&category=tools&note=Hi+there"
st, h, _ = req("/wrath/leyline-preview/submit", "POST", form)
check("submit: 303 to the thanks page", st == 303 and h.get("location") == "/wrath/leyline-preview/submit?done=1", (st, h))
recs = [json.loads(l) for l in open(qfile)]
check("submit: one record, no personal data", len(recs) == 1 and set(recs[0]) == {"id", "repo", "build", "category", "note", "at", "via"}
      and recs[0]["repo"] == "https://github.com/new-author/cool-mod" and recs[0]["build"] == "2.4.3"
      and "10.9.0.1" not in open(qfile).read(), recs)
check("thanks page", "Thank you" in req("/wrath/leyline-preview/submit?done=1")[2].decode())
st, _, b = req("/wrath/leyline-preview/submit", "POST", form)
check("submit: the same repo again is already queued", st == 400 and "already in the queue" in b.decode())
st, _, b = req("/wrath/leyline-preview/submit", "POST", "repo=https%3A%2F%2Fexample.com%2Fx&build=3.3.5a&category=tools")
check("submit: other sites refused, the form kept", st == 400 and "GitHub, GitLab or Codeberg" in b.decode() and "example.com" in b.decode())
st, _, b = req("/wrath/leyline-preview/submit", "POST", "repo=" + first["repo"] + "&build=3.3.5a&category=tools")
check("submit: a listed repo is already in the catalog", st == 400 and "already on the list" in b.decode())
st, _, b = req("/wrath/leyline-preview/submit", "POST", "repo=https%3A%2F%2Fgithub.com%2FChumpton%2Fx&build=3.3.5a&category=tools")
check("submit: Chumpton refused", st == 400)
st, _, b = req("/wrath/leyline-preview/submit", "POST", "repo=https%3A%2F%2Fgithub.com%2Fa%2Fb&build=9.9&category=tools")
check("submit: a build we don't have", st == 400 and "build" in b.decode())
st, _, _ = req("/wrath/leyline-preview/submit", "POST", "repo=https%3A%2F%2Fgithub.com%2Fbot%2Fspam&build=3.3.5a&category=tools&website=x")
check("submit: the hidden field filled (a bot): thanked, nothing kept", st == 303 and "bot/spam" not in open(qfile).read())
st, _, _ = req("/wrath/leyline-preview/submit", "POST", "x" * 5000)
check("submit: too large", st == 413)
codes = [req("/wrath/leyline-preview/submit", "POST", f"repo=https%3A%2F%2Fgithub.com%2Frate%2Fm{k}&build=3.3.5a&category=tools",
             ip="10.9.9.9")[0] for k in range(7)]
check("submit: 5 an hour from one address", codes[:5] == [303] * 5 and codes[5:] == [400, 400], codes)
st, _, _ = req("/wrath/leyline-preview/catalog", "POST", "a=b")
check("POST elsewhere: 405", st == 405)

store = HB.hub.store
new_id = recs[0]["id"]
check("pending: oldest first, one per repo", [r["id"] for r in store.pending()][0] == new_id and len(store.pending()) == 6)
delist = first["id"][5:]
with open(os.path.join(DATA, "hub", "decisions.jsonl"), "w") as f:
    f.write(json.dumps({"id": new_id, "decision": "approve", "category": "audio", "at": "2026-10-29T10:00:00Z"}) + "\n")
    f.write(json.dumps({"id": delist, "repo": first["repo"], "decision": "delist", "at": "2026-10-29T10:01:00Z"}) + "\n")
    f.write("not json\n")
    f.write(json.dumps({"id": "zz", "decision": "approve"}) + "\n")
st, h, b = req("/wrath/leyline-preview/approved.json")
doc = json.loads(b)
check("approved.json: approved with the admin's category, delisted, nothing else", st == 200 and doc == {
    "format": 1, "approved": [{"id": new_id, "repo": "https://github.com/new-author/cool-mod", "build": "2.4.3", "category": "audio"}],
    "delisted": [{"id": delist, "repo": first["repo"]}]} and h.get("x-robots-tag"), doc)
check("approved.json: no notes or times", "Hi there" not in b.decode() and "2026-10" not in b.decode())
check("a decided submission leaves the queue", new_id not in [r["id"] for r in store.pending()])
check("a delisted entry is gone at once", req("/wrath/leyline-preview/p/" + first["id"].replace(":", "-"))[0] == 404)
st, _, b = req("/wrath/leyline-preview/submit", "POST", "repo=" + first["repo"] + "&build=3.3.5a&category=tools", ip="10.1.1.1")
check("a delisted repo can't be submitted again", st == 400 and "reviewed" in b.decode())

# ---------------------------------------------------------------- the launcher feed
st, h, b = req("/wrath/leyline/catalog.json")
feed = json.loads(b)
check("feed: 200 json, CORS *", st == 200 and h.get("access-control-allow-origin") == "*" and h["content-type"].startswith("application/json"))
check("feed: the all.json shape", feed["format"] == 1 and isinstance(feed["projects"], list) and feed["builds"] == ["1.12", "2.4.3", "3.3.5a", "any"]
      and not any(set(p.get("builds") or []) & {"4.3.4", "5.4.8"} for p in feed["projects"])
      and [c["id"] for c in feed["categories"]][-1] == "themes" and len(feed["categories"]) == 13)
ids = [p["id"] for p in feed["projects"]]
check("feed: ids in the launcher's forms", all(re.fullmatch(r"link:[0-9a-f]{12}|mod:[a-z][a-z0-9-]{2,31}|theme:[a-z][a-z0-9-]{1,31}", i) for i in ids))
links = [p for p in feed["projects"] if p.get("hub_source") == "index"]
check("feed: index entries are link-out, no Install", links and all(p["link_out"] and p["download"] is None and p["sha256"] is None
      and p["source"] == "community" and p["repo"].startswith("https://") for p in links))
check("feed: every field all.json has", all(set(p) >= {"id", "source", "name", "description", "category", "builds", "author",
      "author_url", "licence", "repo", "page", "credits", "stars", "last_push", "version", "kind", "permissions", "on_wrath",
      "download", "sha256"} for p in links))
check("feed: sorted by name", [p["name"].lower() for p in feed["projects"]] == sorted(p["name"].lower() for p in feed["projects"]))
check("feed: the delisted entry is out", first["id"] not in ids)
check("feed: no brand words", not BANNED.findall(" ".join(p["name"] + " " + p["description"] + " " + p["author"] + " " + p["credits"]
                                                      for p in feed["projects"])))


class FakeWorkshop:
    def all_json(self):
        dup = INDEX["entries"][1]
        return json.dumps({"projects": [
            {"id": "mod:pings", "source": "packaged", "name": "Pings", "description": "Alt-click the map.", "category": "addons",
             "builds": ["3.3.5a"], "author": "WrathUnleashed", "licence": "MIT", "repo": "", "page": "https://play.example/wrath/mods/pings",
             "download": "https://play.example/wrath/mods/pings/download", "sha256": "a" * 64, "kind": "ui"},
            {"id": dup["id"], "source": "community", "name": "Shared link", "description": "Someone shared it.", "category": "tools",
             "builds": ["3.3.5a"], "author": "x", "licence": "MIT", "repo": dup["repo"], "page": "https://play.example/wrath/mods/p/" + dup["id"][5:]}]})

    def categories(self):
        return [{"id": c, "name": n, "line": "", "icon": ""} for c, n in HB.CATEGORIES], {}


real = HB.hub.portal
HB.hub.portal = lambda name: FakeWorkshop() if name == "workshop" else real(name)
items = HB.hub.items()
dup_id = INDEX["entries"][1]["id"]
check("merge: a Workshop link and an index entry for one repo show once, as the Workshop's",
      [i["source"] for i in items if i["id"] == dup_id] == ["workshop"])
check("merge: packaged mods first-class", any(i["id"] == "mod:pings" and i["packaged"] for i in items))
feed2 = json.loads(req("/wrath/leyline/catalog.json")[2])
pings = next(p for p in feed2["projects"] if p["id"] == "mod:pings")
check("feed: packaged mods keep their download", pings["download"].endswith("/download") and pings["link_out"] is False)
HB.hub.portal = real

# ---------------------------------------------------------------- no client numbers, no game name (owner 2026-10-30)
# Every hub page, with realms on several builds and catalog text full of build numbers and the game's name: none of it
# reaches what a visitor reads (text, title, meta, alt, labels). The launcher feed keeps the numbers (it is data).
NOPE = re.compile(r"(?i)(?<![\w.])v?(?:1\.12(?:\.1)?|2\.4\.3|3\.3\.5a?|4\.3\.4|5\.4\.8|5875|8606|12340|15595|18414)(?![\w])"
                  r"|world\s*of\s*warcraft|warcraft|(?<![a-z])wow(?![a-z])|lich\s*king|blizzard")


def shown(p):
    """What a visitor can read: the text, the title, meta content, alt, title, aria-label, placeholder (not values,
    hidden inputs, URLs, the script or the styles)."""
    p = re.sub(r"<(script|style)\b.*?</\1>", " ", p, flags=re.S)
    p = re.sub(r"<!--.*?-->|<input[^>]*type=\"hidden\"[^>]*>", " ", p, flags=re.S)
    # the realm list's version chip is the one allowed client number (owner 2026-10-30: "list the client version number in
    # the realm list too")
    p = re.sub(r'<span class="hb-ver" title="Game client version">[0-9a-z.]+</span>', " ", p)
    attrs = " ".join(re.findall(r'(?:alt|title|content|aria-label|placeholder)="([^"]*)"', p))
    import html as _h
    return _h.unescape(re.sub(r"<[^>]+>", " ", p) + " " + attrs)


def rec(rid, name, build, live=True, players=3, tags=(), core="azerothcore", port=40100):
    return wp.clean_listing_record({"schema": 1, "name": name, "state": "live" if live else "offline", "tags": list(tags),
        "profile": {"build": build, "core": core, "port": port, "address": "relay.example"},
        "community": {"realmId": rid, "relay": "relay.example", "claimed": True, "score": 5,
                      "lastHeartbeat": {"online": live, "players": players, "bots": 0}}})


listing = [rec("aaaaaaaaaaaaaaaa", "WoW 1.12.1 Classic", 5875, tags=["blizzlike"], core="vmangos"),
           rec("bbbbbbbbbbbbbbbb", "Outland 2.4.3 Blizzard Fans", 8606, core="cmangos", port=40101),
           rec("cccccccccccccccc", "Lich King 3.3.5a (12340)", 12340, port=40102),
           rec("dddddddddddddddd", "Warcraft Nights", 12340, live=False, port=40103)]
with wp._listing_lock:
    wp._listing.update(realms=[r for r in listing if r], ok=True, fetched=True, at=time.monotonic() + 1e6, when=time.time())
json.dump({"format": 1, "posts": [{"id": "v", "date": "2026-10-30", "tag": "Patch 2.4.3", "title": "Launcher 0.5 for 1.12.1 and 3.3.5a",
                                   "items": ["Finds your own 1.12.1, 2.4.3 and 3.3.5a clients", "Build 12340 realms play in the browser"]}]},
          open(os.path.join(os.environ["WU_LEYLINE_DIR"], "news.json"), "w"))
idx = json.load(open(os.path.join(HUBDIR, "community-index.json")))
kept = json.dumps(idx)
e0 = idx["entries"][2]
e0["summary"] = "An addon for 1.12.1 and 3.3.5a clients (5875 / 12340)."
e0["description"] = "Works on 2.4.3 too. Version 3.3.5a build."
e0["needs"] = "A 3.3.5a client"
json.dump(idx, open(os.path.join(HUBDIR, "community-index.json"), "w"))


class BrandWorkshop(FakeWorkshop):
    def all_json(self):
        return json.dumps({"projects": [
            {"id": "mod:bags", "source": "packaged", "name": "WoW 1.12.1 Bags", "category": "addons", "builds": ["1.12", "3.3.5a"],
             "description": "A World of Warcraft 3.3.5a addon for Wrath of the Lich King (12340), made by Blizzard fans.",
             "author": "Bag Maker", "licence": "MIT", "repo": "", "page": "https://play.example/wrath/mods/bags",
             "download": "https://play.example/wrath/mods/bags/download", "sha256": "a" * 64, "kind": "ui"}]})


HB.hub.portal = lambda name: BrandWorkshop() if name == "workshop" else real(name)
every = ["", "/realms", "/realms?client=own", "/realms?client=browser&now=1", "/realms?build=1.12.1", "/modes", "/news",
         "/catalog", "/catalog?build=1.12", "/catalog?build=3.3.5a&sort=name", "/catalog?q=bags", "/creators", "/beacon", "/forge", "/agents",
         "/submit", "/p/mod-bags", "/p/" + e0["id"].replace(":", "-"), "/p/theme-night-sky"]
for path in every:
    st, _, b = req("/wrath/leyline-preview" + path)
    found = NOPE.findall(shown(b.decode()))
    check(f"no client numbers or game name: {path or '/'} ({st})", st == 200 and not found, found[:6])
st, _, b = req("/wrath/leyline-preview/submit", "POST", "repo=https%3A%2F%2Fgithub.com%2Fa%2Fb&build=9.9&category=tools", ip="10.4.4.4")
check("no client numbers or game name: the submit form's error page", st == 400 and not NOPE.findall(shown(b.decode())))
front = req("/wrath/leyline-preview")[2].decode()
bags = req("/wrath/leyline-preview/p/mod-bags")[2].decode()
rl = req("/wrath/leyline-preview/realms")[2].decode()
check("neutralised, not dropped: the realms, the mod and the news are still there in words",
      "<b>Classic</b>" in rl and "Outland Fans" in rl or "Outland" in rl)
check("realm rows: how it plays in words, the filter by those words",
      "Plays in your browser · AzerothCore" in rl and "Bring your own game client · vMaNGOS" in rl and "Original rules" in rl
      and 'href="/wrath/leyline-preview/realms?client=browser">Plays in browser</a>' in rl
      and 'href="/wrath/leyline-preview/realms?client=own">Own client</a>' in rl
      and '<option value="browser">Plays in browser</option><option value="own">Own client</option>' in front
      and 'data-client="own"' in front)
own = req("/wrath/leyline-preview/realms?client=own")[2].decode()
check("realm filter: Own client is every realm the browser can't play", "<b>Classic</b>" in own and "Leyline Sample Hangout" not in own
      and 'data-client="browser"' not in own)
check("item page: the mod in words", "<h1>Bags</h1>" in bags and "An addon for Wrath" in bags and "Classic era, Third era" in bags
      and "<title>Bags · Leyline</title>" in bags, re.sub(r"<style.*?</style>", "", bags, flags=re.S)[:2500])
news = req("/wrath/leyline-preview/news")[2].decode()
check("news: the post kept, its numbers gone", "<h2>Launcher 0.5</h2>" in news and "Finds your own clients" in news, news[-600:])
check("the launcher feed still has the builds (data, not text)", json.loads(req("/wrath/leyline/catalog.json")[2])["builds"][:3] == ["1.12", "2.4.3", "3.3.5a"])
check("neutral(): the helper", HB.neutral("Vanilla WoW 1.12.1 addon (3.3.5a)") == "Vanilla addon"
      and HB.neutral("TrinityCore (3.3.5 branch)") == "TrinityCore" and HB.neutral("Uses Lua 5.1") == "Uses Lua 5.1"
      and HB.neutral("A WoW 1.12.1 addon for World of Warcraft") == "An addon for the game")
HB.hub.portal = real
open(os.path.join(HUBDIR, "community-index.json"), "w").write(kept)
with wp._listing_lock:
    wp._listing.update(realms=[], ok=False, fetched=False, at=-1e9)

# ---------------------------------------------------------------- the front page's new parts (owner 2026-10-30)
def realm_rec(rid, name, build=12340, players=0, tags=(), tagline="", new=False, port=40200, live=True, age=1200, claimed=True,
              name_ok=True):
    return wp.clean_listing_record({"schema": 1, "name": name, "tagline": tagline, "state": "live" if live else "offline",
        "tags": list(tags), "profile": {"build": build, "core": "azerothcore", "port": port, "address": "relay.example"},
        "community": {"realmId": rid, "relay": "relay.example", "claimed": claimed, "score": 5, "new": new, "nameOk": name_ok,
                      "firstSeen": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() - age)),
                      "lastHeartbeat": {"online": live, "players": players, "bots": 0}}})


RIDS = ["oldoldoldoldoldo", "busybusybusybusy", "plainplainplainp", "quietquietquieta"]
listing = [realm_rec(RIDS[0], "Old Days", 5875, 12, ["blizzlike", "pvp"], "The old rules, the old days.", port=40201),
           realm_rec(RIDS[1], "Busy Den", 12340, 30, ["pvp"], "Fast levels, world PvP every evening.", port=40202),
           realm_rec(RIDS[2], "Plain Field", 12340, 0, ["rp"], "Slow roleplay on quiet fields.", new=True, port=40203),
           realm_rec(RIDS[3], "Quiet Shore", 12340, 2, [], "", port=40204),
           realm_rec("offofoffofoffofo", "Sleepy Hollow", 12340, 0, live=False, port=40205),
           # not announced: up for two minutes; a name the relay changed; a brand word in the name (an older relay)
           realm_rec("freshfreshfresha", "Fresh Meadow", 12340, 0, new=True, port=40206, age=120),
           realm_rec("filtfiltfiltfilt", "Haven", 12340, 0, new=True, port=40207, name_ok=False),
           realm_rec("brandbrandbrandb", "WoW Fun Town", 12340, 0, new=True, port=40208)]
with wp._listing_lock:
    wp._listing.update(realms=[r for r in listing if r], ok=True, fetched=True, at=time.monotonic() + 1e6, when=time.time())
json.dump({"format": 1, "posts": [
    {"id": "n1", "date": "2026-10-30", "tag": "Realms", "title": "Realm hopping in one click", "items": ["Hop without signing in again"]},
    {"id": "n2", "date": "2026-10-30", "tag": "Oops", "title": "WoW night", "items": ["dropped"]},
    {"id": "n3", "date": "2026-10-29", "tag": "Launcher", "title": "Launcher 0.6 for 3.3.5a", "items": ["Finds your own 1.12.1 client"]},
    {"id": "n4", "date": "2026-10-28", "tag": "Workshop", "title": "Hot, rising and top", "items": ["Rate what you play"]},
    {"id": "n5", "date": "2026-10-20", "tag": "Old", "title": "The fourth one", "items": []}]},
    open(os.path.join(os.environ["WU_LEYLINE_DIR"], "news.json"), "w"))
pool = [i for i in HB.hub.items() if HB.player_mod(i)]   # Hot right now and Top creators: player mods only
HOT = sorted(pool, key=lambda i: i["name"].lower())[3]
check("Hot right now pool: player mods only, never a server core or server tool", pool and all(i["type"] in ("addon", "other") for i in pool)
      and not any(re.search(r"(?i)trinitycore|azerothcore|keira", i["name"]) for i in pool))          # some item that isn't the best score
AUTH = {i["author"] for i in pool}


class FakeStore:
    def ranked(self, items, sort="hot"):
        rest = [i for i in items if i["id"] != HOT["id"]]
        order = [i for i in items if i["id"] == HOT["id"]] + sorted(rest, key=lambda i: (-float(i.get("score") or 0), i["name"].lower()))
        return [(i, {"installs": {"all": 9, "d30": 500 if i["id"] == HOT["id"] else 1, "d7": 0, "prev7": 0}}) for i in order]

    def metrics(self, items):
        return {i["id"]: {"installs": {"all": 0, "d30": 0}, "rating": {"avg": None, "n": 0}} for i in items}


class FakeRank:
    store = FakeStore()
    KINDS = ()
    SORT_LABELS = []
    KIND_LABELS = []
    RANK_CSS = ""

    @staticmethod
    def installable(i):
        return i["id"] == HOT["id"]


HK = {"on": False}
WHO = {"v": None}


class FakeSocial:
    @staticmethod
    def cookie_user(h):
        return WHO["v"]


def portal_with(name):
    if name == "hub_rank":
        return FakeRank
    if name == "herokey_on":
        return lambda: HK["on"]
    if name == "social":
        return FakeSocial
    return real(name)


HB.hub.portal = portal_with
fp = req("/wrath/leyline-preview")[2].decode()
order = [fp.find(x) for x in ('class="hb-hero"', 'id="realms"', 'class="hb-how', 'id="features"',
                              'id="f-mods"', 'id="forum"', 'id="news"', 'id="creators-top"',
                              'id="status"')] + [fp.rfind('class="hb-fine"')]
check("front page order: hero, realms, how it works, features, forum and Discord (news, then hot and creators), the status "
      "bar, the fine print", -1 not in order and order == sorted(order), order)
# owner 1 Nov (fewer panels, straight to the point): the numbers, the community posters, the Hero Key line and Mods in play
# repeated what the realm list and the gallery already show; they are off the page (their methods are checked below)
check("front page: no numbers, no community posters, no Hero Key line, no Mods in play (owner 1 Nov)",
      'class="hb-nums' not in fp and 'id="hosted"' not in fp and 'id="herokey"' not in fp and 'class="hb-ms ' not in fp)
GOT, ITEMS = HB.hub.realm_rows(), HB.hub.items()
# owner 31 Oct 23:50 ("remove this area from site"): no Live now / New this week strip and no ticker on the front page
check("front page: no Live now / New this week strip and no ticker (owner 31 Oct)",
      'class="hb-tick"' not in fp and 'id="live"' not in fp and "New this week" not in fp, fp.count("hb-tick"))
check("ticker: the 30 s live update refreshes its counts (same JSON, text only)", "[data-tick=" in HB.HUB_JS and "tk(c.realmId,h.players)" in HB.HUB_JS
      and 'tk("home",s.players)' in HB.HUB_JS)
nums = re.findall(r'<b class="hb-num" data-count="(\d+)">(\d+)</b><span>([^<]+)</span>', HB.hub.counters(GOT, ITEMS))
check("numbers: realms, players, mods, the final values in the page (no script, reduced motion)", [n[2] for n in nums] == [
      "realms online", "players right now", "mods, addons and tools"] and all(a == b for a, b, _ in nums)
      and int(nums[2][0]) == len([i for i in HB.hub.items() if i["source"] != "theme"]) and int(nums[1][0]) >= 44, nums)
check("numbers: counted up once by the script, only below the fold, never with reduced motion",
      re.search(r"if\(!still&&\"IntersectionObserver\" in window\)\{\s*var cio=", HB.HUB_JS) and 'n.textContent="0";cio.observe(n)' in HB.HUB_JS
      and "getBoundingClientRect().top>innerHeight){n.textContent" in HB.HUB_JS)
check("ley line: a faint thread down the feature sections, drawn on scroll by script, drawn whole without it or with reduced motion,"
      " never takes a click, hidden on narrow screens",
      '<svg class="hb-thread" viewBox="0 0 40 1000" preserveAspectRatio="none" aria-hidden="true" focusable="false">' in fp
      and "pointer-events:none;z-index:2}" in fp and ".hb-js .hb-thread.draw path{stroke-dasharray:1" in fp
      and ".hb-thread path{stroke-dasharray:none!important}" in fp and "@media (max-width:1150px){.hb-thread,.hb-fs::before{display:none}}" in fp
      and 'if(th&&feats&&!still){th.classList.add("draw")' in HB.HUB_JS)
check("screenshots: tilt toward the cursor with a glow, fine pointers and motion only",
      'if(!still&&window.matchMedia&&matchMedia("(pointer:fine)").matches){d.querySelectorAll(".hb-shot")' in HB.HUB_JS
      and ".hb-shot.tilt::after{opacity:1}" in fp and ".hb-shot{transform:none!important}" in fp)
pv = re.search(rf'<li class="hb-rl pv" data-rid="{RIDS[1]}".*?</li>', fp, re.S)
check("realm rows: a preview with the tagline and tags on hover and focus, laid over (no layout shift)",
      pv and '<div class="hb-rl-pv" role="note"><p>Fast levels, world PvP every evening.</p><p class="hb-rl-pt"><span>PvP</span></p></div>' in pv.group(0)
      and ".hb-rl-pv{position:absolute;" in fp and ".hb-rl.pv:is(:hover,:focus-within) .hb-rl-pv{visibility:visible" in fp
      and re.search(rf'<li class="hb-rl" data-rid="{RIDS[3]}"', fp), pv.group(0)[-400:] if pv else "")
hot1 = re.search(r'<ol class="hb-hot"><li><a class="hb-hot-r" href="([^"]+)">.*?<b>([^<]+)</b><i>([^<]+)</i>', fp, re.S)
check("no separate Mod of the week card (the featured slider and the gallery carry it); Hot right now leads with the top Hot item, "
      "its author and its installs this month",
      'id="mod-week"' not in fp and 'class="hb-motw' not in fp and 'class="hb-pair"' not in fp
      and hot1 and hot1.group(1) == f'/wrath/leyline-preview/p/{HOT["id"].replace(":", "-")}' and hot1.group(2) == html.escape(HB.as_typed(HOT['name'], HB.neutral(HOT['name'])))
      and re.fullmatch(rf"by {re.escape(html.escape(HB.as_typed(HOT['author'], HB.neutral(HOT['author']))))} · \d+ installs? this month", hot1.group(3)), hot1.groups() if hot1 else "")
pst = HB.hub.posters(GOT)
posters = re.findall(r'<article class="hb-poster" style="--h1:(\d+);--h2:(\d+);--a:(\d+)deg;--x:(\d+)%;--y:(\d+)%;--r:(\d+)deg">(.*?)</article>', pst, re.S)
names = [re.search(r"<h3>([^<]+)</h3>", p[6]).group(1) for p in posters]
check("hosted by the community: up to three live community realms as posters (not ours, not offline)",
      names == ["Busy Den", "Old Days", "Quiet Shore"] or (len(names) == 3 and "Sleepy Hollow" not in names and "Leyline Sample Hangout" not in names), names)
again = re.findall(r'<article class="hb-poster" style="([^"]+)"', HB.hub.posters(HB.hub.realm_rows()))
check("posters: a backdrop made from each realm's id (the same every time, different per realm), the mark only",
      again == re.findall(r'<article class="hb-poster" style="([^"]+)"', pst) and len(set(again)) == len(again)
      and "url(/wrath/art/leyline-crystal-256.webp)" in HB.HUB_CSS + pst + fp and "TODO(owner): hosts uploading their own poster image" in pst)
bd = next(p[6] for p in posters if "Busy Den" in p[6])
od = next((p[6] for p in posters if "Old Days" in p[6]), "")
check("posters: name, tagline, tags, players, Play in the browser or Open in launcher",
      "<b>30</b> playing" in bd and "Fast levels, world PvP every evening." in bd and "<span>PvP</span>" in bd
      and f'href="/wrath?go=r:{RIDS[1]}"' in bd and f'href="leyline://realm/{RIDS[1]}">Open in launcher' in bd
      and (not od or ("go=r:" not in od and "Open in launcher" in od)))
newsb = re.search(r'<div class="hb-cm-p" id="news".*?</div>', fp, re.S).group(0) if 'id="news"' in fp else ""
if wp.admin is not None:
    check("news: the latest three posts, brand-word posts dropped, numbers taken out, read-only links to the news page",
          re.findall(r"<b>([^<]+)</b>", newsb) == ["Realm hopping in one click", "Launcher 0.6", "Hot, rising and top"]
          and "WoW" not in newsb and newsb.count('href="/wrath/leyline-preview/news"') == 4
          and "<form" not in newsb, newsb)
hkx = HB.hub.herokey_teaser(GOT)
hk = re.search(r'<section class="hb-hkb hb-rv" id="herokey".*?</section>', hkx, re.S).group(0)
check("Hero Key (flag off): one slim line, One key, every realm, Coming soon and one sentence; no made-up realm chips, no moving key",
      "One key, every community realm" in hk and "Coming soon" in hk and "Available now" not in hk and hk.count('<span class="hb-hkb-l">') == 1
      and 'hb-hk-c' not in fp and 'hb-hk-key' not in fp and "@keyframes hb-key" not in fp and "<a " not in hk
      and "TODO(owner): the Hero Key line" in hkx and hkx.index("TODO(owner): the Hero Key line") < hkx.index('id="herokey"'))
HK["on"] = True
hk_on = re.search(r'<section class="hb-hkb hb-rv" id="herokey".*?</section>', HB.hub.herokey_teaser(HB.hub.realm_rows()), re.S).group(0)
check("Hero Key (flag on): available now, no Coming soon", "Available now" in hk_on and "Coming soon" not in hk_on
      and 'href="/wrath/leyline-preview/realms">Pick a realm' in hk_on)
HK["on"] = False
crs = re.findall(r'<li><a class="hb-crw-a" href="/wrath/leyline-preview/catalog\?q=([^"]+)" title="([^"]+)"><img class="hb-av" '
                 r'src="/wrath/avatar/p/[a-z]+\.svg" alt="" width="44" height="44" loading="lazy"><b>([^<]+)</b><small>([^<]+)</small></a></li>',
                 re.search(r'id="creators-top".*?</div>', fp, re.S).group(0), re.S)
crs = [(q, n, t, sm) for q, t, n, sm in crs]
check("Top creators: a compact avatar row (each a portrait from the name, never a letter), up to four authors of the top mods, with counts, linking to their work in the catalog",
      1 <= len(crs) <= 4 and HB.as_typed(HOT["author"], HB.neutral(HOT["author"])) == html.unescape(crs[0][1]) and all(re.match(r"\d+ hot mods?( · |$)", html.unescape(c[2])) for c in crs)
      and re.fullmatch(r"[5-9]\d\d installs this month", crs[0][3]) and " · Top: " in crs[0][2] and len({c[1] for c in crs}) == len(crs)
      and all(urllib.parse.unquote_plus(html.unescape(c[0])) in AUTH for c in crs), crs)
foot = re.search(r'<footer class="hb-foot">.*?</footer>', fp, re.S).group(0)
st_words = re.findall(r'<em class="(ok|bad)">', re.search(r'id="status".*?</section>', fp, re.S).group(0))
check("status bar: the summary agrees with the footer's dot", ("All systems normal" if all(w == "ok" for w in st_words) else
      "Some systems are down" if "ok" in st_words else "Systems are down") in re.search(r'id="status".*?</section>', fp, re.S).group(0))
want = "ok" if all(w == "ok" for w in st_words) else "warn" if "ok" in st_words else "bad"
check(f"footer: the status dot ({want}), linking to Platform Status, pulsing only when all is OK",
      re.search(rf'<a class="hb-health {want}" href="/wrath/leyline-preview#status"><span class="hb-hdot" aria-hidden="true"></span>', foot)
      and ".hb-health.ok .hb-hdot{animation:hb-pulse" in fp and ".hb-health.warn .hb-hdot{background:#f0b54a}" in fp, (want, foot[:300]))
check("realm list: each row shows its client version as a chip (the one place the hub names a build), an icon and a fill bar",
      re.search(r'<li class="hb-rl home[^"]*" data-home="1"[^>]*><span class="hb-rl-ic home" style="--h:\d+" aria-hidden="true"><svg class="hb-crest"', fp)
      and '<span class="hb-ver" title="Game client version">3.3.5a</span>' in fp and 'class="hb-rl-bar"' in fp)
check("the front page with all of this: no client numbers or game names", not NOPE.findall(shown(fp)), NOPE.findall(shown(fp))[:6])
low = fp[fp.index('<section class="hb-sec hb-cm'):fp.index("</section>", fp.index('id="status"')) + 10]
check("the lower page (Community, the status bar): no game names or client numbers, no money or support links",
      low and not BANNED.findall(visible(low)) and not NOPE.findall(shown(low))
      and not re.search(r"(?i)donat|patreon|ko-fi|\bvip\b|\bshop\b|support us|buy ", low), (BANNED.findall(visible(low)), NOPE.findall(shown(low))))
check("the lower page: two blocks only (Community, the status bar), each revealed like the rest",
      len(re.findall(r'<section class="(hb-sec hb-cm|hb-sbar) hb-rv"', low)) == 2 and len(re.findall(r"<section", low)) == 2)
check("still one script, under the nonce, reading only this site's JSON", fp.count("<script") == 1
      and re.findall(r'json\("([^"]+)"\)', HB.HUB_JS) == ["/wrath/realms.json", "/wrath/status.json"] and "innerHTML" not in HB.HUB_JS)
# signed in: the account chip instead of Sign In (the site's session, through the portal's social module)
WHO["v"] = ("BRAN", "Bran")
c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=30)
c.request("GET", "/wrath/leyline-preview/creators", headers={"Cookie": "wr_session=abc", "X-Forwarded-For": "10.9.0.7"})
signed = c.getresponse().read().decode()
check("header signed in: the account chip, no Sign In", '<a class="hb-acct" href="/wrath/account" aria-label="Your account: Bran">' in signed
      and 'class="hb-signin"' not in signed)
check("header signed out (no cookie): Sign In, the social module never asked", 'class="hb-signin"' in req("/wrath/leyline-preview/creators")[2].decode())
WHO["v"] = None

# ---------------------------------------------------------------- the gallery (owner 2026-10-30: screenshots and featured mods)
GALS = re.compile(r'<figure class="hb-gt( [a-z ]+)?" data-k="(game|mods|launcher)" id="([^"]+)">(.*?)</figure>', re.S)


class ShotWorkshop(FakeWorkshop):
    """A Workshop whose packaged mod has a screenshot (all.json doesn't list screenshots: the hub asks mod())."""
    def mod(self, mid):
        return {"id": mid, "screenshots": ["one.png"]} if mid == "pings" else None


def portal_gal(name):
    return ShotWorkshop() if name == "workshop" else portal_with(name)


HB.hub.portal = portal_gal
check("gallery: a game shot missing from the art folder is never a broken image (the flat tile instead)",
      'id="g-leyline"' in req("/wrath/leyline-preview/gallery")[2].decode() and not os.path.isdir(wp.ART_DIR))
check("modes page: a mode's key art missing from the art folder is no picture at all (never a broken image)",
      'id="mode-azeroth-survivors"' in req("/wrath/leyline-preview/modes")[2].decode()
      and 'class="hb-mode-art"' not in req("/wrath/leyline-preview/modes")[2].decode())
check("featured modpack: missing from the art folder, no section and no tile", 'id="featured-mod"' not in req("/wrath/leyline-preview")[2].decode())
ART_WAS = wp.ART_DIR
wp.ART_DIR = os.path.join(HERE, "www-files", "art")
fp = req("/wrath/leyline-preview")[2].decode()
# the front page no longer has the gallery section (owner 2026-10-30: "remove gallery at bottom"); its place near the foot
# is the sliding strip of mod and in-game pictures ("this place should show sliding gallery of mod images")
check("front page: no gallery section, no Browse by category, no For creators / Host a Beacon boxes",
      'id="gallery"' not in fp and "Browse by category" not in fp and 'class="hb-two' not in fp)
ms = re.search(r'<section class="hb-ms hb-rv" aria-labelledby="hb-ms-h">.*?</section>', HB.hub.mod_strip(HB.hub.items()), re.S)
ms = ms.group(0) if ms else ""
ms_imgs = re.findall(r'<li><a class="hb-ms-t" href="[^"]+"(?: rel="noopener noreferrer")?><img src="([^"]+)" alt="[^"]+" width="\d+" height="\d+" loading="lazy"', ms)
check("mod strip (off the front page since 1 Nov, the gallery has the pictures): real pictures only (mods and in game, no launcher "
      "shots, no drawn tiles), twice for the loop with the copy hidden from screen readers",
      ms and 'class="hb-ms ' not in fp and len(ms_imgs) >= 6
      and not any("leyline-ui-" in u for u in ms_imgs) and '<ul class="hb-ms-l dup" aria-hidden="true" inert>' in ms
      and ms.count("<li>") == 2 * ms.split('class="hb-ms-l dup"')[0].count("<li>"), ms_imgs)
check("mod strip: stops on hover and focus, does not move with reduced motion",
      ".hb-ms:hover .hb-ms-run,.hb-ms:focus-within .hb-ms-run{animation-play-state:paused}" in fp
      and "@media (prefers-reduced-motion:reduce){.hb-ms{overflow-x:auto!important}.hb-ms-run{animation:none" in fp)
check("mod strip: clipped to the page, so the sliding copy never widens it", ".hb-ms{margin:34px 0 18px;overflow:hidden;overflow:clip;max-width:100%}" in fp)
gsec = req("/wrath/leyline-preview/gallery")[2].decode()
tiles = GALS.findall(gsec)[:12]
kinds = [t[1] for t in tiles]
check("gallery: the first tiles are in game, mods and launcher taken in turn", len(tiles) >= 8
      and kinds[:6] == ["game", "mods", "launcher"] * 2 and {"game", "mods", "launcher"} <= set(kinds), kinds)
check("gallery: the first tile is the big featured modpack shot, with its authors' credit and a link out (no Install)",
      tiles and "big" in tiles[0][0] and tiles[0][2] == "g-coa-ultrahd"
      and 'src="/wrath/art/leyline-shot-coa-ultrahd.webp"' in tiles[0][3] and "<i>Screenshot: the modpack&#x27;s authors</i>" in tiles[0][3]
      and f'<a class="hb-gt-i" href="{html.escape(HB.FEATURED_MOD["url"])}" rel="noopener nofollow ugc" target="_blank">See the modpack</a>' in tiles[0][3]
      and "Install" not in tiles[0][3] and "<small>Featured mod</small>" in tiles[0][3], tiles[0][:3] if tiles else "")
check("gallery: then the in-game shots, the hangout first", [t[2] for t in tiles if t[1] == "game"][:2] == ["g-coa-ultrahd", "g-dalaran"])
mt = [t for t in tiles if t[1] == "mods"]
check("gallery: the first mod is the Mod of the week (top Hot), then the rest of Hot",
      mt and mt[0][2] == "m-" + HOT["id"].replace(":", "-") and "Mod of the week" in mt[0][3], [t[2] for t in mt])
check("gallery: every mod tile has Install with Leyline (leyline://install/<id>), View and the author's credit", mt and all(
      re.search(r'<a class="hb-gt-i" href="leyline://install/(?:mod|link|theme):[a-z0-9-]+">(?:Install with|Open in) Leyline</a>'
                r'<a class="hb-gt-v" href="/wrath/leyline-preview/p/[a-z0-9-]+">View</a>', t[3]) and re.search(r"<i>by [^<]+</i>", t[3])
      and 'class="hb-gt-x" hidden' not in t[3] for t in mt))
check("gallery: the Mod of the week installs with Leyline", f'href="leyline://install/{HOT["id"]}">Install with Leyline</a>' in mt[0][3])
gal = req("/wrath/leyline-preview/gallery")[2].decode()
pings = next((t for t in GALS.findall(gal) if t[2] == "m-mod-pings"), None)
check("gallery page: a Workshop mod with its own screenshot shows it (from the Workshop, same origin), with alt text and size",
      pings and re.search(r'<img src="/wrath/mods/pings/shot/0" alt="A screenshot of Pings, an addon by WrathUnleashed" width="640" height="360" '
                          r'loading="lazy" decoding="async">', pings[3]), pings[3][:300] if pings else "")
gen = [t for t in GALS.findall(gal) if t[1] == "mods" and "<img" not in t[3]]
check("gallery: a mod without our screenshot gets a tile drawn from its id (gradient, initial, kind icon), labelled",
      gen and all(re.search(r'<span class="hb-gt-art gen" style="--h1:\d+;--h2:\d+;--a:\d+deg" aria-hidden="true"><b>.</b><svg', t[3])
                  and re.search(r'<a class="hb-gt-a" href="[^"]+" data-lb aria-label="[^"]+">', t[3]) for t in gen))
imgs = re.findall(r"<img\b[^>]*>", gal + gsec)
check("gallery: every picture has alt text, a width and a height, and loads lazily", imgs and all(
      re.search(r'alt="[^"]{12,}"', i) and re.search(r'width="\d+" height="\d+"', i) and 'loading="lazy"' in i
      for i in re.findall(r'<img\b[^>]*>', "".join(t[3] for t in GALS.findall(gal + gsec)))), imgs[:3])
tiles_all = GALS.findall(gal)
check("gallery page: every eligible picture (the featured modpack and 4 in game, 4 launcher, the mods) and no game shot missing",
      sum(1 for t in tiles_all if t[1] == "game") == 5 and sum(1 for t in tiles_all if t[1] == "launcher") == 4
      and sum(1 for t in tiles_all if t[1] == "mods") >= 4 and 'data-gal="all"' in gal, [t[1] for t in tiles_all])
check("gallery: without script each tile is a link: the image itself, or the mod's page", all(
      re.search(r'<a class="hb-gt-a" href="(/wrath/art/leyline-(?:shot|ui)-[a-z-]+\.webp|/wrath/leyline-preview/p/[a-z0-9-]+|/wrath)" data-lb', t[3])
      for t in tiles_all))
chips = re.findall(r'<a class="hb-chip( on)?" href="([^"]+)" data-f="([a-z]*)"(?: aria-current="true")?>([^<]+)</a>', gsec)
check("gallery filters: All, In game, Mods, Launcher, links to the gallery page (?f=) that work without script",
      [(c[1], c[3]) for c in chips] == [("/wrath/leyline-preview/gallery", "All"), ("/wrath/leyline-preview/gallery?f=game", "In game"),
                                        ("/wrath/leyline-preview/gallery?f=mods", "Mods"), ("/wrath/leyline-preview/gallery?f=launcher", "Launcher")]
      and chips[0][0] == " on", chips)
for f in ("game", "mods", "launcher"):
    g = req("/wrath/leyline-preview/gallery?f=" + f)[2].decode()
    ks = [t[1] for t in GALS.findall(g)]
    check(f"gallery ?f={f}: only those tiles, its chip on, no in-place filter", ks and set(ks) == {f} and 'data-gal="one"' in g
          and re.search(rf'<a class="hb-chip on" href="/wrath/leyline-preview/gallery\?f={f}" data-f="{f}" aria-current="true">', g), ks)
check("gallery ?f=<junk>: everything", [t[1] for t in GALS.findall(req("/wrath/leyline-preview/gallery?f=nope")[2].decode())] == [t[1] for t in tiles_all])
hd, nav, labels, on = header_of(gal)
check("gallery page: the hub header (Gallery is no longer in the menu), the sub-nav",
      labels == NAV_ORDER and "/gallery" not in nav
      and '<a href="/wrath/leyline-preview/gallery" aria-current="page" class="on">Gallery</a>' in gal)
check("gallery: script filters in place and opens the lightbox (Esc, arrows, Tab kept inside, focus back to the tile), no innerHTML",
      all(x in HB.HUB_JS for x in ('d.querySelectorAll("[data-gal]")', 'ev.key==="Escape"', 'ev.key==="ArrowLeft"', 'ev.key==="ArrowRight"',
                                    'ev.key==="Tab"', 'aria-modal', "if(a)a.focus()", 'if(g.getAttribute("data-gal")!=="all")return;'))
      and "innerHTML" not in HB.HUB_JS and HB.HUB_JS.find('d.querySelectorAll("[data-gal]")') < HB.HUB_JS.find("if(!rows.length||!window.fetch)return;"))
check("gallery: hover zoom and caption reveal, none of it moving with reduced motion; uniform tiles while filtered",
      ".hb-gt:is(:hover,:focus-within) :is(.hb-gt-a img,.hb-gt-a .hb-gt-art){transform:scale(1.05)}" in fp
      and "@media (hover:none){.hb-gt-c{opacity:1;transform:none}}" in fp
      and re.search(r"prefers-reduced-motion:reduce\)\{.*?\.hb-gt :is\(img,\.hb-gt-art\),\.hb-lb-in\{transform:none!important\}", fp, re.S)
      and ".hb-gal.flt .hb-gt{grid-column:auto;grid-row:auto}" in fp)
SRC = re.compile(r'(?:src|href)="([^"]+\.(?:webp|png|jpe?g|gif|avif|svg))"|<img\b[^>]*\bsrc="([^"]+)"|url\(([^)]+)\)', re.I)
HB.hub.portal = lambda name: None if name == "hub_rank" else portal_gal(name)
cat = req("/wrath/leyline-preview/catalog?view=all")[2].decode()
catf = req("/wrath/leyline-preview/catalog?cat=addons")[2].decode()
HB.hub.portal = portal_gal
third = [u for page in (fp, gal, cat) for m in SRC.findall(page) for u in m if u and not u.startswith("#") and not (u.startswith("/") and not u.startswith("//"))]
check("no third-party pictures anywhere on the hub: every image, image link and CSS url() is our own path", not third, third[:5])
check("gallery: CSP img-src 'self' (the browser would refuse another site's picture anyway)",
      "img-src 'self'" in req("/wrath/leyline-preview/gallery")[1].get("content-security-policy", ""))
check("gallery: no client numbers or game names in captions, alt text or labels", not NOPE.findall(shown(gal)) and not NOPE.findall(shown(gsec))
      and not BANNED.findall(visible(gal)), NOPE.findall(shown(gal))[:5])
strip = re.search(r'<section class="hb-feat-s".*?</section>', cat, re.S)
check("catalog: a Featured strip of the gallery's mod tiles (plain links, no script), only unfiltered on page 1",
      strip and 1 <= len(GALS.findall(strip.group(0))) <= 4 and all(t[1] == "mods" for t in GALS.findall(strip.group(0)))
      and "data-lb" not in strip.group(0) and "<script" not in cat
      and 'class="hb-feat-s"' not in catf)
# the game data killswitch: no game picture at all, one flat Leyline tile (the mark) in their place
FLAG = os.path.join(DATA, "gamedata-off-test")
os.environ["WU_GAMEDATA_FLAG"] = FLAG
open(FLAG, "w").close()
fk = req("/wrath/leyline-preview")[2].decode()
gk = req("/wrath/leyline-preview/gallery?f=game")[2].decode()
fk_tiles = GALS.findall(req("/wrath/leyline-preview/gallery")[2].decode())
check("killswitch on: no in-game screenshot anywhere on the front page or the gallery", "leyline-shot-" not in fk
      and "leyline-shot-" not in req("/wrath/leyline-preview/gallery")[2].decode() and req("/wrath/art/leyline-shot-city.webp")[0] == 410)
check("killswitch on: one flat Leyline tile with the mark instead, still In game, Play in your browser",
      [t[2] for t in fk_tiles if t[1] == "game"] == ["g-leyline"] and [t[2] for t in GALS.findall(gk)] == ["g-leyline"]
      and re.search(r'<figure class="hb-gt[a-z ]* flat" data-k="game" id="g-leyline"><a class="hb-gt-a" href="/wrath" data-lb aria-label="[^"]+">'
                    r'<span class="hb-gt-art flat" aria-hidden="true"><span class="ly-spin slow"', gk)
      and fk_tiles[1][1] == "mods" and fk_tiles[2][1] == "launcher", [t[:3] for t in fk_tiles][:4])
check("killswitch on: no featured modpack section and no modpack tile; its picture answers 410",
      'id="featured-mod"' not in fk and "g-coa-ultrahd" not in fk and req("/wrath/art/leyline-shot-coa-ultrahd.webp")[0] == 410)
check("killswitch on: Game modes shows the launcher's Modes page instead of the Minigolf loop (and the loop answers 410)",
      "<video" not in fk and re.search(r'id="f-modes".*?<img src="/wrath/art/leyline-ui-modes\.webp"', fk, re.S)
      and req("/wrath/art/leyline-shot-minigolf.mp4")[0] == 410 and req("/wrath/art/leyline-shot-minigolf.webm")[0] == 410)
os.remove(FLAG)
os.environ.pop("WU_GAMEDATA_FLAG")
check("killswitch off again: the shots are back", "leyline-shot-dalaran.webp" in req("/wrath/leyline-preview/gallery")[2].decode()
      and 'id="featured-mod"' in req("/wrath/leyline-preview")[2].decode())

# ---------------------------------------------------------------- the featured modpack (owner 2026-10-30: "under this show a
# screenshot ... the CoA ultra hd modpack feature")
coa = re.search(r'<section class="hb-coa hb-rv" id="featured-mod".*?</section>', fp, re.S)
check("featured mod: on the front page right under how it works, before the feature sections",
      coa and fp.find('class="hb-how') < fp.find('id="featured-mod"') < fp.find('id="features"')
      and fp[fp.find('class="hb-how'):fp.find('id="featured-mod"')].count("<section") == 1)
coa = coa.group(0) if coa else ""
check("featured mod: label, title, the screenshot (served, sized, alt), a TODO for the owner's copy",
      '<p class="hb-coa-k"><span aria-hidden="true"></span>Featured mod</p>' in coa and "Conquest of Azeroth: UltraHD</h2>" in coa
      and re.search(r'<img src="/wrath/art/leyline-shot-coa-ultrahd\.webp" alt="[^"]{30,}" width="845" height="577" loading="lazy"', coa)
      and "leyline-shot-coa-ultrahd.webp" in wp.ART_TYPES and "leyline-shot-coa-ultrahd.webp" in wp.GAMEDATA_ART
      and os.path.isfile(os.path.join(HERE, "www-files", "art", "leyline-shot-coa-ultrahd.webp")) and "TODO(owner)" in coa)
check("featured mod: the external note and the credit, word for word",
      '<p class="hb-coa-n">External download, made and hosted by its authors; not checked or hosted by Leyline</p>' in coa
      and '<p class="hb-coa-cr">Screenshot: the modpack&#x27;s authors</p>' in coa)
# a slider since 2026-10-30 ("a sliding preview too of the best latest mods"): the modpack is the first slide
coa_slides = re.split(r'<div class="hb-coa-s(?: mod)?"', coa)[1:]
coa1 = coa_slides[0] if coa_slides else ""
links = re.findall(r'<a ([^>]*)>(.*?)</a>', coa1, re.S)
check("featured mod: the first slide has one link out, See the modpack (noopener nofollow ugc, a new tab), no Install, nothing hosted here",
      len(links) == 1 and f'href="{html.escape(HB.FEATURED_MOD["url"])}"' in links[0][0] and 'rel="noopener nofollow ugc"' in links[0][0]
      and 'target="_blank"' in links[0][0] and links[0][1].startswith("See the modpack")
      and not re.search(r"(?i)install|leyline://|download\"", coa1.replace("External download", "")) and "/wrath/mods/" not in coa1)
more = coa_slides[1:]
check("featured slider: then up to five of the best mods with a picture, each a link to its own page here (no Install, no link out)",
      len(coa_slides) <= 6 and all(re.search(r'<a class="hb-cta sm" href="/wrath/[a-z-]+/p/[^"]+">See the mod</a>', x) for x in more)
      and not any(re.search(r"(?i)leyline://|target=\"_blank\"", x) for x in more))
check("featured slider: dots and arrows only with the script (hidden until then); a sideways snap scroll without it",
      ('<div class="hb-coa-nav" hidden>' in coa) == (len(coa_slides) > 1) and "scroll-snap-type:x mandatory" in fp
      and "timer=setInterval" in HB.HUB_JS and "if(!still){timer=" in HB.HUB_JS)
check("featured mod: no game name in what shows (the address is only a link)", not BANNED.findall(visible(coa))
      and not BANNED.findall(re.sub(r'href="[^"]*"', "", coa)))
check("featured mod: a slow zoom and drift on scroll from the page's script, still with reduced motion",
      "var coa=d.querySelector(\".hb-coa\");" in HB.HUB_JS and "if(coa&&!still)" in HB.HUB_JS and "--coa" in fp
      and re.search(r"prefers-reduced-motion:reduce\)\{.*?\.hb-coa-pic img\{transform:scale\(1\.03\)!important\}", fp, re.S))

# ---------------------------------------------------------------- the server software (owner: "we should have server software in
# this area and themed"): the cores the host pack really runs, checked against hosting/kit/tools/wuhost/cores.py
cspec = importlib.util.spec_from_file_location("kitcores", os.path.join(REPO, "hosting", "kit", "tools", "wuhost", "cores.py"))
kc = importlib.util.module_from_spec(cspec)
cspec.loader.exec_module(kc)
kit_live = {"azerothcore"} | {p["core"] for p in kc.offered_profiles({}).values()}
kit_any = kit_live | {p["core"] for p in kc.PROFILES.values()}
sw = re.search(r'<div class="hb-sw" role="group".*?<ul class="hb-sw-cores">.*?</ul></div>', fp, re.S)
sw = sw.group(0) if sw else ""
cores = re.findall(r'<li class="hb-sw-core[^"]*" data-core="([a-z]+)" data-status="([a-z]+)" title="([^"]+)"><b>([^<]+)</b><span>([^<]+)</span></li>', sw)
check("host console (owner: \"the server GM console / health etc ... all in one host server package\"): in the hosting section",
      'id="f-host"' in fp and sw and fp.find('id="f-host"') < fp.find('class="hb-sw"') < fp.find('id="f-modes"')
      and "Your realm console" in sw and "Online · listed" in sw)
check("host console: health tiles (the four services, players, uptime)",
      re.findall(r"<li><i></i>([^<]+)<span>", sw) == ["Database", "Login server", "World server", "Leyline network", "Players", "Uptime"])
check("host console: the GM controls of the in-game Your realm window, restart with its five-minute warning",
      re.findall(r"<li><b>([^<]+)</b><span>", sw) == [a for a, _ in HB.HOST_GM]
      and "with a five-minute warning" in sw and "announce" in open(os.path.join(REPO, "realm", "custom", "lua_scripts", "ov_hostctl.lua")).read().lower())
check("host console: Check my realm's checks and Realm tools' jobs",
      all(html.escape(x) in sw for x in HB.HOST_CHECKS) and all(f"<li>{html.escape(x)}</li>" in sw for x in HB.HOST_TOOLS))
check("host console: the cores it runs, One click only for what the host pack runs today (cores.py), the rest Coming, and Your own",
      sorted(c[0] for c in cores) == ["azerothcore", "cmangos", "own", "trinitycore", "vmangos"]
      and {c[0] for c in cores if c[1] == "live"} == kit_live and all(c[4] == "One click" for c in cores if c[1] == "live")
      and all(c[4] == "Coming" for c in cores if c[1] == "coming") and [c[4] for c in cores if c[0] == "own"] == ["Small helper"],
      (kit_live, [c[:2] for c in cores]))
check("server software: our own crest (the Leyline crystal) and a rune rule drawn in SVG, no pictures, no game words, no numbers",
      '<svg class="hb-crest"' in sw and sw.count('class="hb-rune"') == 2 and "<img" not in sw and "url(" not in sw
      and not BANNED.findall(visible(sw)) and not NOPE.findall(shown(sw)), NOPE.findall(shown(sw)))
check("server software: torch gold used sparingly next to our blue and violet",
      "#d9a441" in fp and fp.count("#d9a441") <= 12 and ".hb-sw-chip.go{color:#f1cf86" in fp)

# ---------------------------------------------------------------- the realm list picture (owner: "the actual looking realm list
# thing but ... our featured server names")
check("realm types from tags: Normal, PvP, RP, RP-PvP", [HB.realm_type(t)[0] for t in ([], ["hangout"], ["pvp"], ["rp"], ["rp", "pvp"],
      ["blizzlike", "pvp", "bots"])] == ["Normal", "Normal", "PvP", "RP", "RP-PvP", "PvP"])
check("population words from players: Offline, Low, Medium, High, Full", [HB.population(*x)[0] for x in (
      (False, 50), (True, None), (True, 0), (True, 19), (True, 20), (True, 79), (True, 80), (True, 199), (True, 200), (True, 900))]
      == ["Offline", "Low", "Low", "Low", "Medium", "Medium", "High", "High", "Full", "Full"])
RROWS = [{"home": True, "name": "Wrath Unleashed", "live": True, "players": 4, "tags": ["hangout"], "build": 12340},
         {"id": "featuredfeatured", "name": "Old Days", "live": True, "players": 90, "tags": ["rp", "pvp"], "featured": True, "build": 12340},
         {"id": "busybusybusybusy", "name": "Busy Den", "live": True, "players": 230, "tags": ["pvp"], "build": 12340},
         {"id": "quietquietquietq", "name": "Quiet <Meadow>", "live": True, "players": 30, "tags": ["rp"], "build": 5875},
         {"id": "sleepysleepysle2", "name": "Sleepy", "live": False, "players": None, "tags": [], "build": 12340}]
rl = HB.hub.realm_list_art((RROWS, {}, True, {}, {}))
rrows = re.findall(r'<li><a class="hb-rlx-r([^"]*)" href="([^"]+)"[^>]*data-type="([a-z]+)" data-pop="([a-z]+)">'
                   r'<span class="hb-rlx-n">([^<]+)</span><span class="hb-rlx-t[^"]*">([^<]+)</span>.*?<span class="hb-rlx-p[^"]*">([^<]+)</span></a></li>', rl)
check("realm list: the rows from the live list in its order, then a dimmed Your realm here (Host one) row",
      [r[4] for r in rrows] == ["Wrath Unleashed", "Old Days", "Busy Den", "Quiet &lt;Meadow&gt;", "Sleepy", "Your realm here"]
      and rrows[-1][0] == " you" and rrows[-1][1] == "/wrath/host" and rrows[-1][5] == "Host one", [r[4] for r in rrows])
check("realm list: types coloured from tags, population in words", [(r[5], r[2]) for r in rrows[:5]] == [
      ("Normal", "pve"), ("RP-PvP", "rppvp"), ("PvP", "pvp"), ("RP", "rp"), ("Normal", "pve")]
      and [r[6] for r in rrows[:5]] == ["Recommended", "High", "Full", "Medium", "Offline"]
      and all(f".hb-rlx-t.{c}{{color:" in HB.HUB_CSS for c in ("pvp", "rp", "rppvp"))
      and all(f".hb-rlx-p.{c}{{color:" in HB.HUB_CSS for c in ("low", "med", "high", "full", "rec")), [(r[5], r[6]) for r in rrows])
check("realm list: ours selected and Recommended; each row one click to play, as on the realm list",
      rrows[0][0] == " sel" and 'aria-current="true"' in rl and [r[1] for r in rrows[:5]] == [
      "/wrath", "/wrath?go=r:featuredfeatured", "/wrath?go=r:busybusybusybusy", "leyline://realm/quietquietquietq", "leyline://realm/sleepysleepysle2"]
      and rrows[4][0] == " off")
check("realm list: columns Realm, Type, Characters, Population; rows are links (keyboard focusable) with a spoken summary",
      re.findall(r"<span>([A-Za-z]+)</span>", re.search(r'<p class="hb-rlx-cols".*?</p>', rl).group(0)) == ["Realm", "Type", "Characters", "Population"]
      and rl.count('aria-label="') == 5 and "Realm list</p>" in rl and ".hb-rlx-r:hover,.hb-rlx-r:focus-visible{" in HB.HUB_CSS)
full = HB.hub.realm_list_art((RROWS + [dict(RROWS[2], id="b" * 16, name="More")] * 3, {}, True, {}, {}))
check("realm list: at most six realms, no Your realm here row when the list is full", full.count('class="hb-rlx-r') == HB.REALM_LIST_MAX
      and "Your realm here" not in full)
check("realm list: in Realms & hopping (off the front page since 1 Nov), our own drawing (no picture, no game words or numbers)",
      re.search(r'id="f-realms".*?<div class="hb-rlx" role="group"', HB.hub.features(HB.hub.realm_rows(), only=None), re.S) and "<img" not in rl and "url(" not in rl
      and not BANNED.findall(visible(rl)) and not NOPE.findall(shown(rl)))
check("realm list: stacks on a phone", "@media (max-width:560px){" in HB.HUB_CSS and ".hb-rlx-cols{display:none}" in HB.HUB_CSS)

# ---------------------------------------------------------------- the Minigolf loop in Game modes (owner: "in here show the minigolf gif")
vid = re.search(r'<figure class="hb-shot hb-vid">.*?</figure>', fp, re.S)
vid = vid.group(0) if vid else ""
check("game modes: the Minigolf recording, muted, looped, inline, nothing loaded until it plays, the poster first",
      re.search(r'id="f-modes".*?<figure class="hb-shot hb-vid">', fp, re.S)
      and '<video muted loop playsinline preload="none" poster="/wrath/art/leyline-shot-minigolf.webp" width="820" height="596" data-hb-vid' in vid
      and vid.find('<source src="/wrath/art/leyline-shot-minigolf.webm" type="video/webm">')
      < vid.find('<source src="/wrath/art/leyline-shot-minigolf.mp4" type="video/mp4">') and "autoplay" not in vid)
check("game modes: caption and a Play link to the mode (works without script)", "<span>Minigolf, on our own realm</span>" in vid
      and 'href="/wrath?go=mode:minigolf">Play</a>' in vid
      and any(m["id"] == "minigolf" and m["start"] == {"go": "mode:minigolf"} for m in json.load(open(os.path.join(HERE, "playmodes.json")))["modes"]))
check("game modes: played only on screen, never with reduced motion or Save-Data",
      'd.querySelectorAll("video[data-hb-vid]")' in HB.HUB_JS and "navigator.connection.saveData" in HB.HUB_JS
      and "!still&&!save" in HB.HUB_JS and "v.pause()" in HB.HUB_JS)
check("game modes: the loop's files served and behind the killswitch",
      all(n in wp.ART_TYPES and n in wp.GAMEDATA_ART and os.path.isfile(os.path.join(HERE, "www-files", "art", n))
          for n in ("leyline-shot-minigolf.mp4", "leyline-shot-minigolf.webm", "leyline-shot-minigolf.webp"))
      and wp.ART_TYPES["leyline-shot-minigolf.mp4"] == "video/mp4" and wp.ART_TYPES["leyline-shot-minigolf.webm"] == "video/webm")
st, hd, b = req("/wrath/art/leyline-shot-minigolf.mp4")
whole = len(b)
check("game modes: the video answers a plain GET whole, with Accept-Ranges", st == 200 and hd.get("accept-ranges") == "bytes"
      and hd.get("content-type") == "video/mp4" and whole > 100000)


def ranged(path, rng):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=30)
    c.request("GET", path, headers={"Range": rng})
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


st, hd, b = ranged("/wrath/art/leyline-shot-minigolf.webm", "bytes=0-99")
check("game modes: a byte range answers 206 with Content-Range (seeking works)", st == 206 and len(b) == 100
      and re.fullmatch(r"bytes 0-99/\d+", hd.get("content-range", "")))
st, hd, b = ranged("/wrath/art/leyline-shot-minigolf.mp4", "bytes=-10")
check("game modes: a suffix range is the last bytes", st == 206 and len(b) == 10 and hd["content-range"] == f"bytes {whole - 10}-{whole - 1}/{whole}")
check("game modes: a range past the end is 416; a picture ignores ranges", ranged("/wrath/art/leyline-shot-minigolf.mp4", f"bytes={whole + 5}-")[0] == 416
      and ranged("/wrath/art/leyline-shot-dalaran.webp", "bytes=0-9")[0] == 200)
check("game modes: the page's CSP lets our own media play", "media-src 'self'" in req("/wrath/leyline-preview")[1].get("content-security-policy", ""))

# ---------------------------------------------------------------- the modes' key art (Azeroth Survivors)
mp = req("/wrath/leyline-preview/modes")[2].decode()
sv = re.search(r'<article class="hb-mode art" id="mode-azeroth-survivors">.*?</article>', mp, re.S)
check("modes page: Azeroth Survivors shows its key art (served, sized, alt), the others none",
      sv and re.search(r'<img class="hb-mode-art" src="/wrath/art/leyline-mode-azeroth-survivors\.webp" alt="[^"]{20,}" width="483" height="298"', sv.group(0))
      and mp.count('class="hb-mode-art"') == 1 and "leyline-mode-azeroth-survivors.webp" in wp.ART_TYPES
      and os.path.isfile(os.path.join(HERE, "www-files", "art", "leyline-mode-azeroth-survivors.webp")))
wp.ART_DIR = ART_WAS
HB.hub.portal = portal_with
HB.hub.portal = lambda name: None if name == "hub_rank" else portal_with(name)
best = sorted(pool, key=lambda i: (-float(i.get("score") or 0), i["name"].lower(), i["id"]))[0]
check("Hot right now without rankings: the best score first, no installs line, Every mod links to the plain catalog",
      re.search(rf'<ol class="hb-hot"><li><a class="hb-hot-r" href="/wrath/leyline-preview/p/{re.escape(best["id"].replace(":", "-"))}">',
                fpn := req("/wrath/leyline-preview")[2].decode()) and "installs this month" not in fpn.split('class="hb-hot"')[1].split("</ol>")[0]
      and '<a href="/wrath/leyline-preview/catalog">Every mod, by what\'s hot</a>' in fpn, re.findall(r'class="hb-hot-r" href="([^"]+)"', fpn)[:2] + [best["id"]])
HB.hub.portal = real
with wp._listing_lock:
    wp._listing.update(realms=[], ok=False, fetched=False, at=-1e9)

# ---------------------------------------------------------------- hidden: nothing links here; the switches
for p in ("/wrath/leyline", "/wrath/realms", "/wrath/modes", "/wrath/mods", "/wrath/docs", "/wrath/armory", "/wrath/workshop",
          "/wrath/nope"):
    st, _, b = req(p)
    check(f"{p} ({st}) doesn't link to the preview", b"leyline-preview" not in b)
leaks = []
for d in (os.path.join(HERE, "www-files"), os.path.join(REPO, "docs", "leyline"), os.path.join(REPO, "launcher", "ui")):
    for root, _, files in os.walk(d):
        for n in files:
            if n.endswith((".html", ".js", ".md", ".css")):
                if "leyline-preview" in open(os.path.join(root, n), encoding="utf-8", errors="replace").read():
                    leaks.append(n)
check("no page file, doc or launcher page links to the preview", not leaks, leaks)
check("the site nav and footer don't", "leyline-preview" not in wp.site_nav() + wp.site_footer())
old = req("/wrath/leyline")[2].decode()
check("switch off (default): /wrath/leyline is the current page", 'class="hb-top"' not in old)
check("switch off (default): /wrath/leyline/gallery is not served", 'class="hb-top"' not in req("/wrath/leyline/gallery")[2].decode())
os.environ["WU_HUB_FRONT"] = "1"
st, h, b = req("/wrath/leyline")
s = b.decode()
check("switch on: /wrath/leyline is the hub's front page, indexable", st == 200 and 'class="hb-top"' in s
      and "x-robots-tag" not in h and 'content="noindex"' not in s and "Preview:" not in s)
st, h, b = req("/wrath/leyline/gallery")
check("switch on: /wrath/leyline/gallery is the gallery, indexable (no longer in the menu)",
      st == 200 and "<h1>Gallery</h1>" in b.decode() and "x-robots-tag" not in h and 'content="noindex"' not in b.decode()
      and "/wrath/leyline/gallery" not in s)
# the front door is the main site (owner 2026-10-30: "make this our main one"): every hub page at /wrath/leyline/<page>
check("switch on: no link on the front page says leyline-preview", "leyline-preview" not in s)
for pg in ("/realms", "/addons", "/addons?view=all", "/modes", "/news", "/creators", "/beacon", "/submit", "/forge", "/agents",
           "/p/theme-night-sky"):
    st, h, b = req("/wrath/leyline" + pg)
    t = b.decode()
    check(f"switch on: /wrath/leyline{pg} is served, indexable, no preview flag or links",
          st == 200 and 'class="hb-top"' in t and "leyline-preview" not in t and "Preview:" not in t
          and 'class="hb-sub"' not in t and "x-robots-tag" not in h,
          (st, "leyline-preview" in t, "Preview:" in t, 'class="hb-sub"' in t, h.get("x-robots-tag"), re.findall(r".{60}leyline-preview.{40}", t)[:2]))
st, h, _ = req("/wrath/leyline-preview/catalog?cat=addons&q=bags")
check("switch on: an old preview address sends a 301 to the front door, with its query",
      st == 301 and h.get("location") == "/wrath/leyline/catalog?cat=addons&q=bags", (st, h.get("location")))
check("switch on: the preview's own front page too", req("/wrath/leyline-preview")[1].get("location") == "/wrath/leyline")
check("switch on: the catalog feed stays where launchers read it", req("/wrath/leyline/catalog.json")[0] == 200)
for old_p, to in (("/wrath/realms?client=own", "/wrath/leyline/realms?client=own"), ("/wrath/mods", "/wrath/leyline/catalog"),
                  ("/wrath/workshop?q=bags&x=1", "/wrath/leyline/catalog?q=bags"),
                  ("/wrath/mods/c/addons", "/wrath/leyline/addons?view=all"), ("/wrath/mods?cat=addons", "/wrath/leyline/addons?view=all"),
                  ("/wrath/mods/c/tools", "/wrath/leyline/catalog?cat=tools")):
    st, h, _ = req(old_p)
    check(f"switch on: the older page {old_p} sends a 301 to its twin", st == 301 and h.get("location") == to, (st, h.get("location")))
check("switch on: a Workshop item page stays", req("/wrath/mods/pings")[0] != 301)
st, h, _ = req("/wrath/leyline/submit", "POST", "repo=https%3A%2F%2Fgithub.com%2Fa%2Fb&build=3.3.5a&category=tools", ip="10.4.4.9")
check("switch on: Submit a mod posts at the front door and lands there", st in (303, 400)
      and (st == 400 or h.get("location") == "/wrath/leyline/submit?done=1"), (st, h.get("location")))
check("Referrer-Policy same-origin: Chrome sends a real Origin on the site's own forms (no-referrer made it null, and "
      "every Origin check refused them), and other sites still get nothing", req("/wrath/leyline")[1].get("referrer-policy") == "same-origin")
os.environ.pop("WU_HUB_FRONT")
check("switch off: the older pages stay themselves", req("/wrath/realms")[0] == 200 and req("/wrath/mods")[0] == 200)
os.environ["WU_HUB"] = "0"
check("WU_HUB=0: the preview is off", req("/wrath/leyline-preview")[0] == 404 and req("/wrath/leyline/catalog.json")[0] == 404)
os.environ.pop("WU_HUB")

# ---------------------------------------------------------------- preview thumbnails and external packs
TCRED = "Preview image from the project's GitHub page"
ENOTE = "External download, made and hosted by its authors; not checked or hosted by Leyline"
gh = [e for e in INDEX["entries"] if e["forge"] == "github"]
base_e = dict(gh[0])
base_e.pop("thumb", None); base_e.pop("thumb_credit", None)
tn = HB.thumb_name(base_e["repo"])
good = dict(base_e, thumb="/wrath/art/hub-thumbs/" + tn, thumb_credit=TCRED)
check("thumb: the entry's own /wrath/art/hub-thumbs/<owner>__<repo>.webp with the exact credit passes",
      tn.endswith(".webp") and "__" in tn and not HB.thumb_problem(good))
for why, bad in {"another repo's file": dict(good, thumb="/wrath/art/hub-thumbs/someone__else.webp"),
                 "an outside link": dict(good, thumb="https://opengraph.githubassets.com/x/" + tn),
                 "a png": dict(good, thumb="/wrath/art/hub-thumbs/" + tn[:-5] + ".png"),
                 "another credit": dict(good, thumb_credit="From GitHub"),
                 "no credit": {k: v for k, v in good.items() if k != "thumb_credit"}}.items():
    doc = dict(INDEX, entries=[bad if e["id"] == bad["id"] else e for e in INDEX["entries"]])
    check(f"thumb: {why} is refused (validate_index)", any("thumb" in p for p in HB.validate_index(doc)))
gl = [e for e in INDEX["entries"] if e["forge"] != "github"]
if gl:
    check("thumb: only for a GitHub repo", HB.thumb_problem(dict(gl[0], thumb="/wrath/art/hub-thumbs/a__b.webp", thumb_credit=TCRED)))


def ext(url, **kw):
    x = {"id": HB.external_id(url), "name": "Crisp Ground Pack", "author": "Pack Folk", "url": url, "category": "graphics",
         "type": "external-pack", "builds": ["3.3.5a"], "summary": "Sharper ground textures, from its authors' page.",
         "external": True, "note": ENOTE}
    x.update(kw)
    return x


EXT_REPO = ext("https://github.com/packfolk/crisp-ground", repo="https://github.com/packfolk/crisp-ground",
               thumb="/wrath/art/hub-thumbs/packfolk__crisp-ground.webp", thumb_credit=TCRED)
EXT_PAGE = ext("https://forum.example.org/t/sky-pack", name="Sky Pack", author="")
check("external: a well-formed pack passes", HB.external_problems(EXT_REPO) == [] and HB.external_problems(EXT_PAGE) == [])
for why, bad in {"a made-up id": dict(EXT_PAGE, id="ext:000000000000"), "an id that isn't ext:<12hex>": dict(EXT_PAGE, id="link:abc"),
                 "http": ext("http://forum.example.org/t/x"), "a direct file": ext("https://cdn.example.org/pack.zip"),
                 "a file host": ext("https://mega.nz/file/abc"), "an install link": ext("leyline://install/x"),
                 "another type": dict(EXT_PAGE, type="addon"), "no flag": dict(EXT_PAGE, external=False),
                 "another note": dict(EXT_PAGE, note="Fine to install"), "an install field": dict(EXT_PAGE, download="https://x.example/a"),
                 "a brand word": dict(EXT_PAGE, name="WoW HD"), "markup": dict(EXT_PAGE, summary="<b>hi</b>"),
                 "a thumb without a repo": dict(EXT_PAGE, thumb="/wrath/art/hub-thumbs/a__b.webp", thumb_credit=TCRED),
                 "a repack": dict(EXT_PAGE, summary="A full repack")}.items():
    check(f"external: {why} is refused", HB.external_problems(bad), why)
EXT_BAD = ext("https://files.example.org/pack.7z", name="Bad Pack")
fx = json.loads(json.dumps(INDEX))
skip_thumb = gh[1]["id"]
for e in fx["entries"]:
    if e["forge"] == "github" and e["id"] != skip_thumb:
        e.update(thumb="/wrath/art/hub-thumbs/" + HB.thumb_name(e["repo"]), thumb_credit=TCRED)
fx["external"] = [EXT_REPO, EXT_PAGE, EXT_BAD]
check("fixture index (thumbs on GitHub entries) passes the hub's rules", HB.validate_index(fx) == [], HB.validate_index(fx)[:3])
json.dump(fx, open(os.path.join(HUBDIR, "community-index.json"), "w"))
ART_TMP = tempfile.mkdtemp(prefix="wu-hub-art-")
os.makedirs(os.path.join(ART_TMP, "hub-thumbs"))
WEBP = b"RIFF" + (30).to_bytes(4, "little") + b"WEBPVP8 " + b"\0" * 22
for e in fx["entries"] + [EXT_REPO]:
    if "thumb" in e:
        open(os.path.join(ART_TMP, e["thumb"][len("/wrath/art/"):]), "wb").write(WEBP)
open(os.path.join(ART_TMP, "hub-thumbs", "fake__html.webp"), "wb").write(b"<html>not an image</html>")
open(os.path.join(ART_TMP, "hub-thumbs", "x__y.png"), "wb").write(b"\x89PNG\r\n\x1a\n")
open(os.path.join(ART_TMP, "wu-theme.css"), "w").write("body{}")
ART_WAS2, wp.ART_DIR = wp.ART_DIR, ART_TMP
thumb1 = fx["entries"][[e["id"] for e in fx["entries"]].index(gh[0]["id"])]["thumb"]
st, h, b = req(thumb1)
check("art route: a hub thumbnail is served (image/webp, nosniff)", st == 200 and h.get("content-type") == "image/webp" and b == WEBP
      and h.get("x-content-type-options") == "nosniff", (st, h.get("content-type")))
for p in ("/wrath/art/hub-thumbs/fake__html.webp", "/wrath/art/hub-thumbs/x__y.png", "/wrath/art/hub-thumbs/..%2Fwu-theme.css",
          "/wrath/art/hub-thumbs/../wu-theme.css", "/wrath/art/hub-thumbs/Upper__Case.webp", "/wrath/art/hub-thumbs/nounderscore.webp",
          "/wrath/art/hub-thumbs/", "/wrath/art/hub-thumbs/a__b.webp/x"):
    check(f"art route: {p} is refused", req(p)[0] == 404)
check("art_present: thumbnails by pattern only", wp.art_present(thumb1[len("/wrath/art/"):]) and not wp.art_present("hub-thumbs/x__y.png")
      and not wp.art_present("../art/wu-theme.css"))
catx = req("/wrath/leyline-preview/catalog?sort=name")[2].decode() + req("/wrath/leyline-preview/addons?sort=name")[2].decode()
cards = re.findall(r'<article class="hb-card(?: hb-ext)?">.*?</article>', catx, re.S)
withth = [c for c in cards if '/wrath/art/hub-thumbs/' in c]
check("catalog: GitHub entries show their thumbnail on the card (credited in its title)", withth and all(
      re.search(r'<a class="hb-card-pic" href="/wrath/leyline-preview/p/[a-z0-9-]+" tabindex="-1" aria-hidden="true"><img src="/wrath/art/hub-thumbs/'
                r'[a-z0-9._-]+__[a-z0-9._-]+\.webp" alt="" loading="lazy" width="640" height="320" title="Preview image from the project&#x27;s GitHub page">', c)
      for c in withth), withth[:1])
noth = next((c for c in cards if f'p/{skip_thumb.replace(":", "-")}"' in c), "")
check("catalog: an entry without a thumbnail gets the tile drawn from its id", noth and 'class="hb-gt-art gen hb-card-art"' in noth
      and "hub-thumbs" not in noth, noth[:300])
grp = re.search(r'<section class="hb-ext-s" id="external".*?</section>', catx, re.S)
grp = grp.group(0) if grp else ""
check("catalog: a 'Graphics & external packs' group with the good packs only", "<h2 id=\"hb-ext-h\">Graphics &amp; external packs</h2>" in grp
      and "Crisp Ground Pack" in grp and "Sky Pack" in grp and "Bad Pack" not in catx)
ecards = re.findall(r'<article class="hb-card hb-ext">.*?</article>', grp, re.S)
check("catalog: each external pack carries the badge and Visit the authors' page (new tab, noopener nofollow ugc)", len(ecards) == 2 and all(
      f'<p class="hb-ext-b" role="note">{html.escape(ENOTE)}</p>' in c
      and re.search(r'<a class="hb-ghost" href="https://[^"]+" rel="noopener nofollow ugc" target="_blank">Visit the authors. page</a>', c)
      for c in ecards), ecards[:1])
check("catalog: never an Install for an external pack", "leyline://install/ext:" not in catx and not any("Install" in c for c in ecards))
check("catalog: the source chip 'Graphics & external packs' (?source=external) shows only them",
      'href="/wrath/leyline-preview/catalog?source=external"' in req("/wrath/leyline-preview/catalog?view=all")[2].decode())
only = req("/wrath/leyline-preview/catalog?source=external")[2].decode()
check("catalog ?source=external: the two packs, nothing else", len(re.findall(r'<article class="hb-card hb-ext">', only)) == 2
      and '<article class="hb-card">' not in only)
check("catalog ?cat=addons: no external packs (they are graphics)", 'class="hb-ext-s"' not in req("/wrath/leyline-preview/catalog?cat=addons")[2].decode())
check("catalog: search finds an external pack", "Sky Pack" in req("/wrath/leyline-preview/catalog?q=sky")[2].decode())
st, h, b = req("/wrath/leyline-preview/p/" + EXT_REPO["id"].replace(":", "-"))
xp = b.decode()
check("external item page: 200, noindex, the badge, Visit the authors' page (new tab, nofollow) and the source repo",
      st == 200 and h.get("x-robots-tag") and f"{html.escape(ENOTE)}" in xp and '<b>External</b>' in xp
      and "<a class=\"ly-btn\" href=\"https://github.com/packfolk/crisp-ground\" rel=\"noopener nofollow ugc\" target=\"_blank\">Visit the authors' page</a>" in xp)
check("external item page: never an Install, an install link or a download of ours", "leyline://" not in xp
      and not re.search(r"(?i)\binstall with\b|>Install<|download=", xp))
check("external item page: its thumbnail with the credit under it", '<img src="/wrath/art/hub-thumbs/packfolk__crisp-ground.webp"' in xp
      and f"<figcaption>{html.escape(TCRED)}</figcaption>" in xp)
xp2 = req("/wrath/leyline-preview/p/" + EXT_PAGE["id"].replace(":", "-"))[2].decode()
check("external item page without a repo: the generated tile, 'its authors', no source button", 'class="hb-gt-art gen"' in xp2
      and "its authors" in xp2 and "View source" not in xp2 and "hub-thumbs" not in xp2)
check("external item page: a bad pack is not there", req("/wrath/leyline-preview/p/" + EXT_BAD["id"].replace(":", "-"))[0] == 404)
check("external pages: no brand words or client numbers", not BANNED.findall(visible(xp + xp2 + grp)) and not NOPE.findall(shown(xp + xp2 + grp)))
ip = req(re.search(r'href="(/wrath/leyline-preview/p/link-[a-z0-9]+)"', withth[0]).group(1) if withth else "/x")[2].decode()
check("item page: the thumbnail with its credit", re.search(r'<figure class="hb-item-pic"><img src="/wrath/art/hub-thumbs/[^"]+\.webp" alt="The preview '
      r'image of [^"]+" width="640" height="320"><figcaption>Preview image from the project&#x27;s GitHub page</figcaption></figure>', ip))
galx = req("/wrath/leyline-preview/gallery")[2].decode()
gt = {t[2]: t for t in GALS.findall(galx)}
xt = gt.get("x-" + EXT_REPO["id"].replace(":", "-"))
check("gallery: an external pack with its GitHub thumbnail is a tile, credited, flagged external, no Install",
      xt and 'src="/wrath/art/hub-thumbs/packfolk__crisp-ground.webp"' in xt[3] and TCRED.replace("'", "&#x27;") in xt[3]
      and "not checked or hosted by Leyline" in xt[3] and "leyline://" not in xt[3]
      and 'rel="noopener nofollow ugc" target="_blank">Visit the authors&#x27; page</a>' in xt[3], xt[3][:400] if xt else list(gt)[:20])
check("gallery: an external pack without a GitHub thumbnail is not", "x-" + EXT_PAGE["id"].replace(":", "-") not in gt)
tm = [t for t in GALS.findall(galx) if t[1] == "mods" and "/hub-thumbs/" in t[3] and not t[2].startswith("x-")]
check("gallery: mod tiles use the thumbnail, and the credit is in the caption the lightbox shows (.hb-gt-c i)", tm and all(
      re.search(r'<img src="/wrath/art/hub-thumbs/[^"]+" alt="The preview image of [^"]+" width="640" height="320"', t[3])
      and re.search(r"<i>by [^<]+ · Preview image from the project&#x27;s GitHub page</i>", t[3]) for t in tm)
      and 'parts.c.textContent=txt(t,".hb-gt-c i")' in HB.HUB_JS, [t[2] for t in tm][:5])
check("gallery page: the fine print says where the pictures come from", "preview image from its GitHub page" in galx)
frontx = req("/wrath/leyline-preview")[2].decode()
top = HB.hub._ranked([i for i in HB.hub.items() if HB.player_mod(i)])[0][0]
spot = re.search(r'id="forum".*?</section>', frontx, re.S).group(0)
hot1 = re.search(r'<span class="hb-hot-pic">(.*?)</span><span class="hb-hot-t">', spot, re.S).group(1)
if top.get("thumb"):
    check("Hot right now: the top item's thumbnail, credited", f'src="{top["thumb"]}"' in hot1 and f'title="{html.escape(TCRED)}"' in hot1, hot1)
else:
    check("Hot right now: the generated tile when there's no thumbnail", 'class="hb-gt-art gen"' in hot1, hot1)
check("Hot right now: thumbnails where there's no Workshop screenshot, small and lazy", "/wrath/art/hub-thumbs/" in spot
      and 'loading="lazy" decoding="async" width="64" height="40"' in spot)
check("front page: never an install link for an external pack", "leyline://install/ext:" not in frontx)
feedx = json.loads(req("/wrath/leyline/catalog.json")[2])
check("launcher feed: no external packs (never installable)", not any(str(p.get("id", "")).startswith("ext:") for p in feedx["projects"]))
wp.ART_DIR = ART_WAS2
check("thumbnail file gone: the generated tile instead of a broken image",
      "/wrath/art/hub-thumbs/" not in req("/wrath/leyline-preview/catalog")[2].decode())
n_before = len(HB.hub.index())
json.dump(dict(fx, external=[dict(EXT_PAGE, download="https://x.example/a")]), open(os.path.join(HUBDIR, "community-index.json"), "w"))
check("a bad external list never takes the index down", 'class="hb-ext-s"' not in req("/wrath/leyline-preview/catalog")[2].decode()
      and n_before > 0 and len(HB.hub.index()) == n_before)
shutil.copy(os.path.join(HERE, "hub", "community-index.json"), HUBDIR)
shutil.rmtree(ART_TMP, ignore_errors=True)

# ---------------------------------------------------------------- the launch lineup: the Hangout, then the official ones
with wp._listing_lock:
    keep_listing = dict(wp._listing)
    wp._listing.update(realms=[realm_rec("busybusybusybusy", "Busy Den", 12340, 30, port=40202),
                               realm_rec("vanillavanillava", "Vanilla Remastered", 5875, 0, port=40206),
                               realm_rec("staffstaffstaffs", "Staff Pick", 12340, 1, port=40207)],
                       ok=True, fetched=True, at=time.monotonic() + 1e6, when=time.time())
lr = HB.hub.realm_rows()
names = [r["name"] for r in lr[0]] if lr else []
check("launch lineup: the Wrath Unleashed pinned first, then Vanilla Remastered (official by name until its record says so), "
      "then the network's own order", names[:2] == ["Wrath Unleashed", "Vanilla Remastered"] and names.index("Busy Den") < names.index("Staff Pick"), names)
check("launch lineup: a record marked official or pinned joins the lineup", HB.lineup_rank({"official": True}) == 0
      and HB.lineup_rank({"pinned": True}) == 0 and HB.lineup_rank({"name": "Busy Den"}) == 1)
lfp = req("/wrath/leyline-preview")[2].decode()
check("launch lineup: Vanilla Remastered shows Official, its client version only in the hb-ver chip",
      re.search(r'<b>Vanilla Remastered</b><span class="hb-ver"[^>]*>[^<]+</span><span class="hb-bdg o">Official</span>', lfp))
# Warden (a host's Browser players off): no Play in browser; the header counts players and bots apart, from the rows
ward = wp.clean_listing_record({"schema": 1, "name": "Warded Keep", "state": "live", "tags": [],
    "profile": {"build": 12340, "core": "azerothcore", "port": 40208, "address": "relay.example", "warden": True},
    "community": {"realmId": "wardwardwardward", "relay": "relay.example", "claimed": True, "score": 5,
                  "lastHeartbeat": {"online": True, "players": 0, "bots": 1}}})
with wp._listing_lock:
    wp._listing["realms"] = wp._listing["realms"] + [ward]
lfp = req("/wrath/leyline-preview")[2].decode()
wrow = re.search(r'<li class="hb-rl[^"]*" data-rid="wardwardwardward"[^>]*>.*?</li>', lfp, re.S)
check("Warden on (Browser players off): Bring your own client, no Play in browser, still Open in launcher",
      ward and ward.get("warden") is True and wrow and 'data-client="own"' in wrow.group(0) and "Play in browser" not in wrow.group(0)
      and "Open in launcher" in wrow.group(0) and HB.client_of({"build": 12340, "warden": True}) == "own"
      and HB.client_of({"build": 12340}) == "browser" and HB.client_of({"build": 12340, "warden": False}) == "browser"
      and HB.client_of({"home": True, "build": 12340, "warden": True}) == "browser")
# ours from realm_status (3 players, 10 bots) + Busy Den 30 + Vanilla Remastered 0 + Staff Pick 1 + Warded Keep 0 and 1 bot
check("the realm list's header: players and bots apart, from the same numbers as the rows",
      '<p class="hb-netline" data-net>5 realms · 34 players online · 11 bots</p>' in lfp
      and HB.hub_totals([{"live": True, "players": 0, "bots": 1}, {"live": False, "players": 5}]) == {"realms": 1, "players": 0, "bots": 1},
      re.findall(r'data-net>[^<]*', lfp))
check("the header's live update: our sample record left out (its count can include bots), ours from status.json",
      "if(c.sample||h.online!==true)return;" in HB.HUB_JS and "homeS=s;netLine();" in HB.HUB_JS and "j.network" not in HB.HUB_JS)
# ---------------------------------------------------------------- the Builders' Realm is off (owner 1 Nov): on no list, by its
# id or by its name (any marks or case, under any id), on the front page, the realm list, realms.json and the portal's own lists
BUILD_ID = "jgt5rpofn7yaisep"
with wp._listing_lock:
    wp._listing["realms"] = wp._listing["realms"] + [r for r in (
        realm_rec(BUILD_ID, "Builders' Realm", 12340, 6, tagline="Build in the world.", port=40209),
        realm_rec("buildbuildbuildb", "BUILDERS\u2019 REALM", 12340, 2, port=40210),
        realm_rec("renamedrenamedre", "Stone Garden", 12340, 1, port=40211)) if r]
bpages = {p: req(p)[2].decode() for p in ("/wrath/leyline-preview", "/wrath/leyline-preview/realms",
                                          "/wrath/leyline-preview/realms?client=browser", "/wrath/realms")}
bjson = json.loads(req("/wrath/realms.json")[2])
bshown = {p: [n for n in ("Builders", "BUILDERS", BUILD_ID, "buildbuildbuildb") if n in v] for p, v in bpages.items()}
check("Builders' Realm: on no site list (front page, realm list, its filters, the portal's realm page), by id or by name",
      not any(bshown.values()) and all("Stone Garden" in v for k, v in bpages.items() if "leyline-preview" in k), bshown)
check("Builders' Realm: not in realms.json either (the site's live counts and the launcher read it), its players not counted",
      not [r for r in bjson.get("realms", []) if "uild" in str(r.get("name")) or BUILD_ID in json.dumps(r)]
      and any(r.get("name") == "Stone Garden" for r in bjson.get("realms", [])), [r.get("name") for r in bjson.get("realms", [])])
check("Builders' Realm: hidden in hub.py and network.py alike (each without the other), never our own realm",
      HB.retired_realm({"id": BUILD_ID, "name": "x"}) and HB.retired_realm({"id": "a" * 16, "name": "Builders Realm"})
      and not HB.retired_realm({"id": "a" * 16, "name": "Stone Garden"}) and not HB.retired_realm({"hangout": True, "name": "Builders' Realm"})
      and wp.network.retired({"id": BUILD_ID}) and wp.network.retired({"name": "the builders' realm"})
      and [r["id"] for r in wp.network.listed([{"id": BUILD_ID, "name": "B"}, {"id": "c" * 16, "name": "C"}])] == ["c" * 16])
check("Builders' Realm: a hop to it still resolves while it is up (lists only, never the relay's hop)",
      any(r["id"] == BUILD_ID for r in wp.community_realms()[0]) and not any(r["id"] == BUILD_ID for r in wp.listed_realms()[0]))
with wp._listing_lock:
    wp._listing.clear(); wp._listing.update(keep_listing)

# ---------------------------------------------------------------- shipping
bat = open(os.path.join(HERE, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
fl = open(os.path.join(HERE, "fastloop.sh"), encoding="utf-8").read()
check("sign-in bat: packs hub.py and hub/", 'if exist hub.py if exist hub\\community-index.json set "LINKS=%LINKS% hub.py hub"' in bat
      and " hub.py " in re.search(r"\$\(ls ([^)]*)\)", bat).group(0))
check("fastloop.sh: installs hub.py and its data, backs them up, puts them back", 'install -m 644 hub.py "$WU/portal/hub.py"' in fl
      and '"$BK/hub.py"' in fl and 'install -m 644 hub/*.json "$WU/portal/hub/"' in fl)
check("fastloop.sh: installs the art's files and the hub-thumbs folder (WebP only), never trips on the subfolder",
      "install -m 644 www-files/art/*" not in fl and 'find www-files/art -maxdepth 1 -type f -exec install -m 644 {} "$WU/portal/art/"' in fl
      and "find www-files/art/hub-thumbs -maxdepth 1 -type f -name '*.webp' -exec install -m 644 {} \"$WU/portal/art/hub-thumbs/\"" in fl)
check("sign-in bat: packs the whole www-files/art folder (tar is recursive: hub-thumbs goes along)", " www-files/art " in bat)
_bc = req("/wrath/leyline-preview/beacon")[2].decode()
check("Beacon page: no promise of adding realm scripts (per-realm mods don't exist yet), the needs and time up front",
      "add realm scripts" not in _bc and "You need:" in _bc and "about 10 GB free" in _bc and "5 to 20 minutes" in _bc
      and 'href="/wrath/docs/network"' in _bc and "host kit" not in _bc and "a profile" not in _bc)
_h = type(HB.hub)()
_t = time.time()
_per_ip = [_h._rate_ok("10.1.1.1", _t) for _ in range(6)]
_flood = sum(_h._rate_ok(f"10.2.{i // 250}.{i % 250}", _t) for i in range(6000))
check("submit limits: 5 an hour per address, and a flood from many addresses never resets the daily cap of 200",
      _per_ip == [True] * 5 + [False] and _flood == 195 and not _h._rate_ok("10.9.9.9", _t)
      and not _h._rate_ok("2001:db8:1:2::5", _t), (_per_ip, _flood))
# ---------------------------------------------------------------- the launcher's look (owner 2026-10-31)
# The addon marketplace on the web: upstream addons (stream/upstream_addons.py) as the launcher's cards, the kinds and
# "Works on <the official realm>" chips, the licence chip and credit line, Install handed to the launcher, their page.
def _up(slug, name, **kw):
    d = {"id": "up:" + slug, "source": "upstream", "name": name, "description": f"{name} does useful things.", "category": "addons",
         "tags": ["meters"], "builds": ["3.3.5a"], "author": "alice", "author_url": "https://github.com/alice", "licence": "MIT",
         "repo": f"https://github.com/alice/{slug}", "addons": [name], "requires": [], "conflicts": [], "replaces_ui": False,
         "native": False, "blp": "none", "review": {"by": "owner", "at": "2026-10-31T09:00:00Z", "notes": ""},
         "compat": {"realm": "Wrath Unleashed", "status": "untested", "checked_at": ""},
         "install": {"from": "commit-archive", "url": "x", "size": 2400000, "file_count": 120, "folders": {"": name}}}
    d.update(kw)
    return d


class FakeUp:
    @staticmethod
    def projects():
        return [_up("elvui", "ElvUI", tags=["full-ui"], replaces_ui=True, licence="", requires=["up:skada"],
                    compat={"realm": "Wrath Unleashed", "status": "works", "checked_at": "2026-10-31T10:00:00Z"}),
                _up("skada", "Skada"), _up("bags", "Bagger", tags=["bags"], builds=["5.4.8"]),
                _up("evil", "<img src=x onerror=alert(1)>", tags=["chat"])]


HB.hub.portal = lambda name: FakeUp if name == "upstream_addons" else real(name)
ups = HB.hub.upstream()
check("upstream: the reviewed addons in the catalog's shape, hidden builds left out, never in items() or the launcher feed",
      [u["id"] for u in ups] == ["up:evil", "up:elvui", "up:skada"] and all(u["source"] == "upstream" for u in ups)
      and not any(i["id"].startswith("up:") for i in HB.hub.items())
      and not any(p["id"].startswith("up:") for p in json.loads(req("/wrath/leyline/catalog.json")[2])["projects"]))
ad = req("/wrath/leyline-preview/addons?view=all")[2].decode()
cards = dict(re.findall(r'<article class="hb-card hb-mk" data-tags="[^"]*">.*?<h3><a href="/wrath/leyline-preview/p/up-([a-z-]+)">.*?</article>', ad, re.S) and
             [(m.group(1), m.group(0)) for m in re.finditer(r'<article class="hb-card hb-mk" data-tags="[^"]*">.*?/p/up-([a-z-]+)".*?</article>', ad, re.S)])
elv, ska = cards.get("elvui", ""), cards.get("skada", "")
check("addons tab: the upstream cards, each with Install handed to the launcher and Source, never a download of ours",
      elv and ska and 'href="leyline://install/up:skada"' in ska and '>Install</a>' in ska
      and 'href="https://github.com/alice/skada" rel="noopener noreferrer nofollow ugc"' in ska and "/download" not in ska, list(cards))
check("upstream card: the \"Works on\" badge on the picture only when tried on the official realm",
      f'<span class="hb-mk-badge on-pic" title="Tried on {HB.HANGOUT_NAME} by our team">Works on {HB.HANGOUT_NAME}</span>' in elv
      and "hb-mk-badge" not in ska)
check("upstream card: the licence chip (No licence when none), the credit line (author · where it comes from), the kind over the picture",
      '<span class="hb-mk-lic" title="Licence: MIT">MIT</span>' in ska and '<span class="hb-mk-lic none" title="The author states no licence">No licence</span>' in elv
      and '<p class="hb-mk-credit" title="Alice · MIT · Installs from the author&#x27;s GitHub"><span>Alice</span>' in ska
      and '<span class="hb-dc-tag ui">Whole interface</span>' in elv and '<span class="hb-dc-tag">Meters</span>' in ska, (elv[-700:], ska[-700:]))
check("upstream card: names are text, never markup", "<img src=x" not in ad and "&lt;img src=x onerror=alert(1)&gt;" in ad)
check("tabs: Mods, Addons, Themes as links (the launcher's Discover has the same three), the current one marked",
      '<nav class="hb-seg" aria-label="Mods, addons and themes"><a href="/wrath/leyline-preview/catalog">Mods</a>'
      '<a href="/wrath/leyline-preview/addons" class="on" aria-current="page">Addons</a>'
      '<a href="/wrath/leyline-preview/catalog?cat=themes">Themes</a></nav>' in ad and '<h1 id="hb-pg-h">Addons</h1>' in ad)
bar = re.search(r'<nav class="hb-mk-bar" aria-label="Addon types">.*?</nav>', ad, re.S)
bar = bar.group(0) if bar else ""
check("the marketplace's chips: All addons, the kinds that have an addon (in the launcher's order), Works on the official realm",
      re.findall(r'>([^<>]+)</a>', bar) == ["All addons", "Full interfaces", "Meters", "Chat", f"Works on {HB.HANGOUT_NAME}"]
      and 'href="/wrath/leyline-preview/addons?tag=meters"' in bar
      and 'href="/wrath/leyline-preview/addons?works=1"' in bar, bar[:600])
w1 = req("/wrath/leyline-preview/addons?works=1")[2].decode()
m1 = req("/wrath/leyline-preview/addons?tag=meters")[2].decode()
check("the chips filter: Works on shows only tried addons, a kind only its addons (both marked on)",
      "/p/up-elvui" in w1 and "/p/up-skada" not in w1 and '<article class="hb-card">' not in w1
      and "/p/up-skada" in m1 and "/p/up-elvui" not in m1 and 'class="hb-chip on" href="/wrath/leyline-preview/addons?view=all"' in m1)
check("search finds upstream addons by kind and name (on Addons; never on Mods)", "/p/up-skada" in req("/wrath/leyline-preview/addons?q=meters")[2].decode()
      and "/p/up-elvui" in req("/wrath/leyline-preview/addons?q=elvui")[2].decode()
      and "/p/up-" not in req("/wrath/leyline-preview/catalog?q=elvui")[2].decode())
check("other categories and external packs: no upstream addons", "/p/up-" not in req("/wrath/leyline-preview/catalog?cat=tools")[2].decode()
      and "/p/up-" not in req("/wrath/leyline-preview/catalog?source=external")[2].decode())
st, _, up_b = req("/wrath/leyline-preview/p/up-elvui")
up_p = up_b.decode()
check("upstream page: the launcher's detail view (badge, the whole-interface warning, the facts, Install with Leyline, Source)",
      st == 200 and "<h1>ElvUI</h1>" in up_p and f">Works on {HB.HANGOUT_NAME}</span>" in up_p
      and "Replaces your whole interface" in up_p and "<dt>Licence</dt><dd>Licence not stated</dd>" in up_p
      and "<dt>Needs</dt><dd>Skada</dd>" in up_p and "<dt>Size</dt><dd>2.3 MB, 120 files</dd>" in up_p
      and f"<dt>On {HB.HANGOUT_NAME}</dt><dd>Works (checked 2026-10-31)</dd>" in up_p
      and 'href="leyline://install/up:elvui">Install with Leyline</a>' in up_p and "Get the launcher" in up_p
      and not NOPE.findall(shown(up_p)), NOPE.findall(shown(up_p)))
check("upstream page: an unknown or hidden-build addon is not found", req("/wrath/leyline-preview/p/up-bags")[0] == 404
      and req("/wrath/leyline-preview/p/up-nope")[0] == 404)
check("Install is the launcher's blue action, Play the launcher's own PLAY picture (owner 2026-10-31: the launcher's button style)",
      '<a class="hb-inst sm" href="leyline://install/up:skada"' in ska and "hb-play sm" not in ad
      and ".hb-play,.hb-ho-a .hb-cta{position:relative;border:12px solid transparent" in HB.SITE_CSS
      and "border-image:url(/wrath/art/play-stone.webp)" in HB.SITE_CSS and '"play-stone.webp": "image/webp"' in open(PORTAL).read())
HB.hub.portal = real
cat0 = req("/wrath/leyline-preview/catalog?view=all")[2].decode()
check("catalog panel: the page's panel with its header strip, More filters as a details (no script), Get the launcher",
      '<section class="hb-pg hb-cat" data-shelf="mods" aria-labelledby="hb-pg-h"><header class="hb-pg-h">' in cat0
      and '<details class="hb-fmore"><summary>More filters</summary>' in cat0 and 'class="hb-get"' in cat0
      and "<script" not in cat0)
rlp = req("/wrath/leyline-preview/realms")[2].decode()
check("realms panel: the realm list in the page's panel, Host your own in its header", '<section class="hb-pg hb-realms-pg"' in rlp
      and '<a class="hb-ghost hb-pg-x" href="/wrath/host">Host your own</a>' in rlp)
check("phones: 16px gutters, chip rows scroll inside the panel, one card per row",
      ".hb-main:has(>.hb-pg),.hb-main:has(>.fm),.hb-main:has(>.hf){padding:16px 16px 32px}" in HB.SITE_CSS
      and ".hb-pg .hb-grid{grid-template-columns:minmax(0,1fr)}" in HB.SITE_CSS)
check("the index in the repo passes the hub's rules", HB.validate_index(json.load(open(os.path.join(HERE, "hub", "community-index.json")))) == [])

# ---- Mods, the Simple view (owner 31 Oct: "'Mods' should also have a simple section, trim search")
sim = req("/wrath/leyline-preview/catalog")[2].decode()
check("Mods: /catalog is the Simple view: one search box, rows of big cards, Browse everything, no filter rows",
      'class="hb-pg hb-cat hb-simple"' in sim and sim.count('role="search"') == 1 and 'class="hb-fmore"' not in sim
      and 'class="hb-filters"' not in sim and '<h2 id="hb-sr-popular">Popular</h2>' in sim
      and '<a class="hb-cta sm" href="/wrath/leyline-preview/catalog?view=all">Browse everything</a>' in sim
      and 'class="hb-grid hb-big"' in sim and "<script" not in sim)
check("Mods Simple: Mods is the tab that is on, the one-line explanation at the top, Search mods",
      '<a href="/wrath/leyline-preview/catalog" class="on" aria-current="page">Mods</a>' in sim
      and f'<p class="hb-lede hb-shelf-line">{html.escape(HB.SHELF_LINE["mods"])}</p>' in sim and 'placeholder="Search mods"' in sim
      and ">List your mod</a>" in sim)
full = req("/wrath/leyline-preview/catalog?view=all")[2].decode()
smart = re.search(r'<div class="hb-chips hb-smart"[^>]*>(.*?)</div>', full, re.S)
_chips = [html.unescape(c) for c in re.findall(r'class="hb-chip[^"]*"[^>]*>([^<]+)</a>', smart.group(1))] if smart else []
check("Mods Browse everything: its categories as chips (All mods, then the ones with something in them) before More filters, Sort inside it",
      _chips[:1] == ["All mods"] and len(_chips) >= 3 and all(c in HB.MOD_GROUP_NAME.values() for c in _chips[1:])
      and _chips[1:] == [n for _k, n, _c, _l in HB.MOD_GROUPS if n in _chips[1:]]
      and full.index('hb-smart') < full.index('<summary>More filters</summary>') < full.index('>Sort</span>'), _chips)
check("Mods: a search (or any filter) is the full catalog; a chip that clears every filter stays there (view=all)",
      'class="hb-simple"' not in req("/wrath/leyline-preview/catalog?q=bags")[2].decode()
      and 'hb-simple' not in req("/wrath/leyline-preview/addons?q=bags")[2].decode()
      and 'href="/wrath/leyline-preview/catalog?view=all" aria-current="true">All mods</a>' in full)
mk = lambda i, **k: dict({"id": i, "source": "workshop", "name": i, "category": "addons", "score": 1.0, "tags": []}, **k)   # noqa: E731
_pool = [mk("mod:a", score=9), mk("mod:b", category="sandbox"), mk("theme:c", category="themes", source="theme"),
         mk("mod:d"), mk("ext:x", source="external", category="graphics"), mk("link:t", category="tools", type="tool"),
         mk("link:g", category="tools", type="addon"), mk("link:r", category="realm-scripts", type="module", score=5)]
_ups = [mk("up:e", source="upstream", tags=["unit-frames"]), mk("up:f", source="upstream", tags=["bags"])]
rows = HB.hub.simple_rows(_pool, _ups)
got = {k: [i["id"] for i in its] for k, _t, _l, _h, its in rows}
check("simple_rows (Mods): Popular first, then one row per category with something in it; no addon, theme or external pack",
      list(got) == ["popular", "game", "realm", "tools"] and got["game"] == ["mod:b"] and got["realm"] == ["link:r"]
      and got["tools"] == ["link:t"] and not {"mod:a", "mod:d", "up:e", "up:f", "link:g", "theme:c", "ext:x"} & set(sum(got.values(), []))
      and all(h == f"/wrath/leyline-preview/catalog?group={k}" for k, _t, _l, h, _i in rows if k != "popular"), got)
rows = HB.hub.simple_rows(_pool, _ups, shelf="addons")
got = {k: [i["id"] for i in its] for k, _t, _l, _h, its in rows}
check("simple_rows (Addons): Popular first, then Make it look nicer, Quality of life; never a mod, theme or external pack",
      list(got) == ["popular", "looks", "qol"] and got["popular"][0] == "mod:a" and got["looks"] == ["up:e"] and len(got["qol"]) == 3
      and not {"mod:b", "link:t", "link:r", "theme:c", "ext:x"} & set(sum(got.values(), []))
      and len(sum((v for k, v in got.items() if k != "popular"), [])) == len(set(sum((v for k, v in got.items() if k != "popular"), []))), got)

# ---- Mods and Addons are two shelves (owner 31 Oct 19:22; docs/WORDS.md): nothing addon-kind on Mods, no mod on Addons
_by_slug = {i["id"].replace(":", "-", 1): i for i in HB.hub.items() + HB.hub.upstream() + HB.hub.external()}


def _shelf_ids(path):
    """Every item linked from a list page and its next pages (the cards' /p/ links)."""
    out, url = set(), path
    for _ in range(40):
        body = req(url)[2].decode()
        main = re.search(r'<main.*?</main>', body, re.S).group(0)
        main = re.sub(r'<section class="hb-feat-s".*?</section>', "", main, flags=re.S)   # the gallery's featured strip
        out |= set(re.findall(r'href="/wrath/leyline-preview/p/([a-z]+-[a-z0-9-]+)"', main))
        nxt = re.search(r'<a href="([^"]+)">Next</a>', main)
        if not nxt:
            break
        url = html.unescape(nxt.group(1))
    return out


_mods_pages = ["/wrath/leyline-preview/catalog", "/wrath/leyline-preview/catalog?view=all"] + [
    f"/wrath/leyline-preview/catalog?group={k}" for k, *_x in HB.MOD_GROUPS] + ["/wrath/leyline-preview/catalog?q=a"]
_addon_pages = ["/wrath/leyline-preview/addons", "/wrath/leyline-preview/addons?view=all", "/wrath/leyline-preview/addons?q=a"]
_on_mods = set().union(*(_shelf_ids(u) for u in _mods_pages))
_on_addons = set().union(*(_shelf_ids(u) for u in _addon_pages))
check("Mods shows no addon: every card on Mods (Simple, Browse everything, each category, a search) is a mod",
      _on_mods and all(HB.shelf_of(_by_slug[x]) == "mods" for x in _on_mods if x in _by_slug)
      and any(_by_slug.get(x, {}).get("category") == "realm-scripts" for x in _on_mods),
      sorted(x for x in _on_mods if x in _by_slug and HB.shelf_of(_by_slug[x]) != "mods")[:5])
check("Addons shows no mod: every card on Addons is an addon (an in-game interface helper)",
      _on_addons and all(HB.shelf_of(_by_slug[x]) == "addons" for x in _on_addons if x in _by_slug),
      sorted(x for x in _on_addons if x in _by_slug and HB.shelf_of(_by_slug[x]) != "addons")[:5])
_every = {x for x, i in _by_slug.items() if i["source"] != "theme"}
check("Mods and Addons together list everything (nothing falls between them) and never share a card",
      not (_on_mods & _on_addons) and {x for x in _every if _by_slug[x]["source"] != "external"} <= (_on_mods | _on_addons),
      sorted(_every - _on_mods - _on_addons)[:5])
check("shelf_of: one rule: the addons category and the marketplace are Addons; a game mode or map stays a mod; an addon-type entry "
      "elsewhere (a GM panel, spoken alerts) is an addon; realm scripts, tools, graphics, sounds are mods; themes are themes",
      [HB.shelf_of(x) for x in ({"category": "addons"}, {"source": "upstream", "category": "addons"},
                                 {"category": "sandbox", "kind": "ui"}, {"category": "maps", "type": "addon"},
                                 {"category": "tools", "type": "addon"}, {"category": "audio", "type": "addon"},
                                 {"category": "realm-scripts", "type": "module"}, {"category": "tools", "type": "tool"},
                                 {"category": "graphics", "type": "external-pack"}, {"category": "audio"},
                                 {"category": "themes", "source": "theme"}, {"category": "encounters", "ws": {"kind": "ui"}})]
      == ["addons", "addons", "mods", "mods", "addons", "addons", "mods", "mods", "mods", "mods", "themes", "addons"])
check("MOD_GROUPS: every mods-list category but addons and themes is in exactly one Mods category",
      sorted(HB.MOD_GROUP_OF) == sorted(c for c in HB.CATEGORY_IDS if c != "addons")
      and sum(len(cs) for _k, _n, cs, _l in HB.MOD_GROUPS) == len(HB.MOD_GROUP_OF))
_js = open(os.path.join(HERE, "..", "launcher", "ui", "shelf.js"), encoding="utf-8").read()
_jsg = re.findall(r'\["([a-z]+)", "([^"]+)", \[([^\]]*)\], "([^"]+)"\]', _js)
check("the launcher keeps the same rule and categories (launcher/ui/shelf.js GROUPS = MOD_GROUPS, the same game categories)",
      [(k, n, tuple(re.findall(r'"([^"]+)"', c)), line) for k, n, c, line in _jsg] == [tuple(g) for g in HB.MOD_GROUPS]
      and re.search(r'const GAME_CATS = \[([^\]]*)\]', _js).group(1).replace('"', "").split(", ") == list(HB.SHELF_GAME_CATS)
      and 'p.category === "addons" || p.source === "upstream"' in _js and 'p.type === "addon" || p.kind === "ui"' in _js, _jsg[:2])
for _u, _want in (("/wrath/leyline-preview/catalog?cat=addons", "/wrath/leyline-preview/addons?view=all"),
                  ("/wrath/leyline-preview/catalog?cat=addons&tag=meters&q=x", "/wrath/leyline-preview/addons?tag=meters&q=x"),
                  ("/wrath/leyline-preview/catalog?kind=addon&sort=name", "/wrath/leyline-preview/addons?sort=name"),
                  ("/wrath/leyline-preview/catalog?source=upstream", "/wrath/leyline-preview/addons?source=upstream"),
                  ("/wrath/leyline-preview/catalog?works=1", "/wrath/leyline-preview/addons?works=1"),
                  ("/wrath/leyline-preview/catalog?kind=mode", "/wrath/leyline-preview/catalog?group=game"),
                  ("/wrath/leyline-preview/catalog?kind=theme", "/wrath/leyline-preview/catalog?cat=themes"),
                  ("/wrath/leyline-preview/catalog?kind=mod", "/wrath/leyline-preview/catalog?view=all"),
                  ("/wrath/leyline-preview/addons?cat=addons", "/wrath/leyline-preview/addons?view=all")):
    st, h, _ = req(_u)
    check(f"old address {_u[22:]} sends a 301 to {_want[22:]}", st == 301 and h.get("location") == _want, (st, h.get("location")))
_ads = req("/wrath/leyline-preview/addons")[2].decode()
check("Addons: /addons is its own Simple view: the one-line explanation, Search addons, List your addon, Addons on in the menu",
      'class="hb-pg hb-cat hb-simple" data-shelf="addons"' in _ads and '<h1 id="hb-pg-h">Addons</h1>' in _ads
      and f'<p class="hb-lede hb-shelf-line">{html.escape(HB.SHELF_LINE["addons"])}</p>' in _ads
      and 'placeholder="Search addons"' in _ads and 'href="/wrath/leyline-preview/submit?kind=addon" data-slot="list-your-mod">List your addon</a>' in _ads
      and '<a href="/wrath/leyline-preview/addons?view=all" class="hb-cta sm">' not in _ads
      and '<a class="hb-cta sm" href="/wrath/leyline-preview/addons?view=all">Browse everything</a>' in _ads
      and header_of(_ads)[3] == ["Addons"] and "hb-sr-game" not in _ads)
check("the one-line explanations say what each is, in plain words (no brand words, no version numbers)",
      all(not BANNED.findall(v) and not re.search(r"\d\.\d", v) for v in HB.SHELF_LINE.values())
      and "AddOns folder" in HB.SHELF_LINE["addons"] and "game modes" in HB.SHELF_LINE["mods"])
_sub = req("/wrath/leyline-preview/submit")[2].decode()
_suba = req("/wrath/leyline-preview/submit?kind=addon")[2].decode()
check("Submit a mod or an addon: the category says which it is (Addon first, then Mod with the Mods categories); ?kind=addon picks Addon",
      '<optgroup label="Addon: an in-game interface helper"><option value="addons">' in _sub
      and '<optgroup label="Mod: changes the game itself"><option value="sandbox">' in _sub
      and '<option value="addons" selected>' in _suba and '<option value="addons" selected>' not in _sub
      and "<h1>Submit a mod or an addon</h1>" in _sub)

# ---- Game mods (stream/gamemods.py; owner 31 Oct: "a client mod section like future skating and golf")
st, h, _ = req("/wrath/leyline-preview/catalog?view=game")
check("the old Game mods tab (catalog?view=game) sends a 301 to Mods › Game modes", st == 301
      and h.get("location") == "/wrath/leyline-preview/catalog?group=game", (st, h.get("location")))
gmp = req("/wrath/leyline-preview/catalog?group=game")[2].decode()
gcards = re.findall(r'<article class="hb-card hb-gm".*?</article>', gmp, re.S)
gids = re.findall(r'data-id="([^"]+)"', " ".join(gcards))
check("Mods › Game modes: the Leyline Modes, each saying what it adds, that it needs a realm that allows it, and where to play it",
      gids[:4] == ["mode:minigolf", "mode:bumper-balls", "mode:coin-dash", "mode:community"]
      and all("Needs a realm that allows it" in c and f"<b>Play it on:</b> {HB.HANGOUT_NAME}" in c for c in gcards[:4])
      and '<a class="hb-play sm" href="/wrath?go=mode:minigolf">Play</a>' in gmp
      and 'href="/wrath/leyline-preview/catalog?group=game" aria-current="true">Game modes</a>' in gmp
      and '<h1 id="hb-pg-h">Mods</h1>' in gmp, gids)
check("Mods › Game modes: no made-up entries (only playmodes.json realm modes and Workshop game categories), nothing Coming that isn't real",
      all(i.startswith("mode:") or i.startswith("mod:") or i.startswith("up:") for i in gids) and "skating" not in gmp.lower()
      and ">Coming<" not in gmp)
check("Mods › Game modes: no brand words shown", not BANNED.findall(visible(gmp)))
sim2 = req("/wrath/leyline-preview/catalog")[2].decode()
check("Simple view: the Game modes row is the Leyline Modes and game mods, See all opens Mods › Game modes",
      '<h2 id="hb-sr-game">Game modes</h2>' in sim2 and 'href="/wrath/leyline-preview/catalog?group=game">See all</a>' in sim2
      and 'data-id="mode:minigolf"' in sim2)
real_rows = HB.hub.realm_rows
HB.hub.realm_rows = lambda query=None: ([{"home": True, "name": HB.HANGOUT_NAME, "live": True, "mods": [{"id": "mode:minigolf"}]},
                                         {"name": "Frost Haven", "live": True, "mods": [{"id": "mode:minigolf"}, {"id": "mode:coin-dash"}]},
                                         {"name": "Sleepy", "live": False, "mods": [{"id": "mode:coin-dash"}]}], {}, True, {}, {})
g2 = req("/wrath/leyline-preview/catalog?group=game")[2].decode()
golf = re.search(r'<article class="hb-card hb-gm"[^>]*data-id="mode:minigolf".*?</article>', g2, re.S).group(0)
dash = re.search(r'<article class="hb-card hb-gm"[^>]*data-id="mode:coin-dash".*?</article>', g2, re.S).group(0)
bump = re.search(r'<article class="hb-card hb-gm"[^>]*data-id="mode:bumper-balls".*?</article>', g2, re.S).group(0)
check("Mods › Game modes: Play it on lists the live realms whose listing names the mod (offline ones left out)",
      f"<b>Play it on:</b> {HB.HANGOUT_NAME}, Frost Haven</p>" in golf and "<b>Play it on:</b> Frost Haven</p>" in dash
      and "Sleepy" not in g2 and "No live realm runs it right now" in bump and 'href="/wrath?go=mode:bumper-balls"' not in bump, golf[-400:])
HB.hub.realm_rows = real_rows

# ---- realms declare their mods (docs/GAME-MODS.md): "Uses 3 mods" on realm rows, and the launcher line
rmh = HB.hub.realm_mods_html({"mods": [{"id": "mode:minigolf"}, {"id": "up:skada", "version": "r1"}, {"id": "https://evil.example/x"},
                                       {"id": "mod:unknown-one"}]})
check("realm row: Uses N mods, named (ids the Workshop doesn't know by their id), the client ones marked, Get the launcher",
      "<summary>Uses 3 mods</summary>" in rmh and "<li>Minigolf</li>" in rmh and "Skada <small>r1</small> <span class=\"hb-rl-mg\">in your game</span>" in rmh
      and "<li>unknown-one</li>" in rmh and "evil" not in rmh and "to add them automatically when you join" in rmh, rmh)
check("realm row: no list, no line", HB.hub.realm_mods_html({}) == "" and HB.hub.realm_mods_html({"mods": "mod:x"}) == "")
rl = req("/wrath/leyline-preview/realms")[2].decode()
home = re.search(r'<li class="hb-rl home.*?</li>\s*(?=<li|</ul>)', rl, re.S)
check("realm list: our realm says which mods it uses (from its own config)", home and "<summary>Uses " in home.group(0)
      and "<li>Minigolf</li>" in home.group(0), home.group(0)[:300] if home else rl[:200])

# ---- realm rows at a desktop window narrower than a full row (owner, 31 Oct, 800px: the name squeezed to a letter a
# line, the meta one word a line, the actions keeping the whole row): the name stays on one line, no sideways scroll
try:
    from playwright.sync_api import sync_playwright
except ImportError:
    sync_playwright = None
if sync_playwright is None:
    print("SKIP realm row layout: playwright is not installed")
else:
    def wide_rows(query=None):
        got = real_rows(query)
        rows = [dict(r, address="play.openvanilla.app", kind="Official realm") if r.get("home") else r for r in got[0]]
        rows.append({"id": "abcdefghijklmnop", "name": "Frosthaven Duelists Guild", "live": True, "players": 7,
                     "address": "relay.openvanilla.app:8101", "tags": ["pvp"]})
        return (rows,) + tuple(got[1:])
    HB.hub.realm_rows = wide_rows
    with sync_playwright() as p:
        b = p.chromium.launch()
        for path in ("/wrath/leyline-preview", "/wrath/leyline-preview/realms"):
            for w in (390, 800, 1024, 1150, 1440):
                pg = b.new_page(viewport={"width": w, "height": 900})
                pg.goto(f"http://127.0.0.1:{PAGE_PORT}{path}")
                got = pg.evaluate("""() => [...document.querySelectorAll('#hb-rlist .hb-rl')].map(li => {
                    const b = li.querySelector('.hb-rl-n b'), m = li.querySelector('.hb-rl-m');
                    const lh = parseFloat(getComputedStyle(b).lineHeight) || 22;
                    return {name: b.textContent, lines: Math.round(b.getBoundingClientRect().height / lh),
                            meta: Math.round(m.getBoundingClientRect().width)};
                })""")
                sw = pg.evaluate("document.documentElement.scrollWidth")
                check(f"realm rows {path} at {w}px: names on one line, the meta line wide, no sideways scroll",
                      len(got) >= 2 and all(r["lines"] == 1 and r["meta"] >= 200 for r in got) and sw <= w, (got, sw))
                pg.close()
        b.close()
    HB.hub.realm_rows = real_rows

srv.shutdown()
shutil.rmtree(DATA, ignore_errors=True)
shutil.rmtree(HUBDIR, ignore_errors=True)
print(f"\n{passed}/{passed + failed} passed")
sys.exit(0 if failed == 0 else 1)
