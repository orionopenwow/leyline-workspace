"""The site as one place: the one nav on every page (the right item marked), the footer and the closing step, the
styled 404, the "Right now" strip on /wrath/leyline (with and without real data, never a made-up number), share
boxes, titles and link cards, focus and reduced-motion styles, images, no game-brand names, escaping.

    python3 -I stream/tests/test_site.py stream/wrath_portal.py
"""
import http.client, importlib.util, json, os, re, shutil, sys, tempfile, threading, time
from http.server import ThreadingHTTPServer

TMP = tempfile.mkdtemp(prefix="wu-site-")
MODS, ARM, ART = (os.path.join(TMP, d) for d in ("mods", "armory", "art"))
for d in (os.path.join(MODS, "files"), os.path.join(ARM, "c"), ART):
    os.makedirs(d)
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example", WU_MODS_DIR=MODS,
                  WU_ARMORY_DIR=ARM, WU_ART_DIR=ART, WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json",
                  WU_JOIN_BASE="https://relay.example")
spec = importlib.util.spec_from_file_location("wp", sys.argv[1])
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, extra)

BANNED = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
def visible(p):
    p = re.sub(r"<style.*?</style>", " ", p, flags=re.S)
    attrs = " ".join(re.findall(r'(?:alt|title|content|aria-label|placeholder)="([^"]*)"', p))
    return re.sub(r"<[^>]+>", " ", p) + " " + attrs

def set_listing(realms, ok=True):
    with wp._listing_lock:
        wp._listing.update(at=time.monotonic(), ok=ok, realms=realms, fetched=True, when=time.time() if ok else 0.0)

def expire_strip():
    with wp._right_now_lock:
        wp._right_now["at"] = -1e9

check("modules beside the portal loaded", all(m is not None for m in (wp.modes, wp.armory, wp.workshop, wp.links)))
wp.armory.STAT_EVERY = 0
wp.armory.store.root = ARM

# ---------------------------------------------------------------- 1. no data at all
wp.realm_status = lambda: {}
set_listing([], ok=False)
items, _ = wp.right_now(fresh=True)
check("no data: no items", items == [], items)
ly = wp.leyline_page().decode()
check("no data: no Right now strip at all", "Right now" not in ly and 'class="ly-now"' not in ly)
check("no data: the page still renders", "<h1>Leyline</h1>" in ly)

# zeros are left out, never shown as live
wp.realm_status = lambda: {"online": True, "players": 0, "bots": 40, "updated_at": "2026-10-25T10:20:30Z"}
set_listing([{"id": "abcdefghijklmnop", "name": "Quiet", "live": False, "players": 0, "bots": 0}])
items, _ = wp.right_now(fresh=True)
check("zero players and no live realm: left out", items == [], items)
wp.realm_status = lambda: {"online": False, "players": 7}
items, _ = wp.right_now(fresh=True)
check("realm down: its player number is not shown", items == [], items)
wp.realm_status = lambda: {"online": True, "players": None}
check("unknown players: left out", wp.right_now(fresh=True)[0] == [])

# ---------------------------------------------------------------- 2. real data
wp.realm_status = lambda: {"online": True, "players": 12, "bots": 30, "updated_at": "2026-10-25T10:20:30Z"}
set_listing([{"id": "abcdefghijklmnop", "name": "Frost", "live": True, "players": 3, "bots": 0, "ruleset": "dalaran",
              "type": "Fun", "tagline": "", "claimed": True, "rules": []},
             {"id": "qrstuvwxyz234567", "name": "Ember", "live": True, "players": 1, "bots": 0, "ruleset": "main",
              "type": "PvE", "tagline": "", "claimed": False, "rules": []},
             {"id": "aaaaaaaaaaaaaaaa", "name": "Shut", "live": False, "players": None, "bots": None, "ruleset": "custom",
              "type": "PvE", "tagline": "", "claimed": False, "rules": []}])
cat = {"format": 1, "mods": [
    {"id": "old-mod", "name": "Old Mod", "version": "1.0.0", "kind": "ui", "zip": "old-mod-1.0.0.zip", "description": "x"},
    {"id": "evil-mod", "name": "Evil <script>alert(1)</script> & \"Co\"", "version": "1.0.0", "kind": "ui",
     "zip": "evil-mod-1.0.0.zip", "description": "y"},
    {"id": "new-mod", "name": "New Mod", "version": "2.0.0", "kind": "realm", "zip": "new-mod-2.0.0.zip", "description": "z"}]}
json.dump(cat, open(os.path.join(MODS, "catalog.json"), "w"))
now = time.time()
for i, z in enumerate(("old-mod-1.0.0.zip", "evil-mod-1.0.0.zip", "new-mod-2.0.0.zip")):
    p = os.path.join(MODS, "files", z)
    open(p, "wb").write(b"PK")
    os.utime(p, (now - 3000 + i * 1000, now - 3000 + i * 1000))
chars = [{"name": n, "race": 1, "class": 2, "level": 80, "reached80_day": d, "last_day": d}
         for n, d in (("Arthas", "2026-10-20"), ("Jaina", "2026-10-24"), ("Thrall", "2026-10-22"), ("Uther", "2026-10-21"))]
chars.append({"name": "Lowbie", "race": 2, "class": 1, "level": 30})
json.dump({"updated": int(now), "chars": chars}, open(os.path.join(ARM, "index.json"), "w"))
for c in chars:
    json.dump(dict(c, items=[]), open(os.path.join(ARM, "c", c["name"] + ".json"), "w"))

items, when = wp.right_now(fresh=True)
kinds = [i["kind"] for i in items]
check("real data: four items, in order", kinds == ["players", "realms", "mods", "eighties"], kinds)
ly = wp.leyline_page().decode()
strip = ly.split('class="ly-now"', 1)[1].split("</section>", 1)[0] if 'class="ly-now"' in ly else ""
check("strip: on the page, under Right now", "Right now" in strip)
check("strip: players from realm_status, linked", '<b>12</b> players on WrathUnleashed</a>' in strip and 'href="/wrath/realms"' in strip)
check("strip: bots not counted as players", "42" not in strip and "30" not in strip.split("<p>")[0])
check("strip: live community realms counted (offline left out)", "<b>2</b> community realms live" in strip)
mods_li = re.findall(r'href="/wrath/mods/([a-z-]+)"', strip)
check("strip: latest mods, newest zip first, each linked", mods_li == ["new-mod", "evil-mod", "old-mod"], mods_li)
check("strip: mod names escaped", "Evil &lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;Co&quot;" in strip and "<script>" not in strip)
eighties = re.findall(r'href="/wrath/armory/([A-Za-z]+)"', strip)
check("strip: recent level 80s, newest first, linked to the armory", eighties == ["Jaina", "Thrall", "Uther"], eighties)
check("strip: links to the Workshop and the armory", 'href="/wrath/mods">Latest mods:' in strip and 'href="/wrath/armory">Recent level 80s:' in strip)
check("strip: says when it was read", re.search(r'Updated <time datetime="\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ">\d\d:\d\d UTC</time>', strip))

# cached for 60 s
wp.realm_status = lambda: {"online": True, "players": 99}
check("cache: the same numbers within 60 s", "<b>12</b> players" in wp.right_now_html() and "<b>99</b>" not in wp.right_now_html())
expire_strip()
check("cache: new numbers after 60 s", "<b>99</b> players" in wp.right_now_html())
check("cache: TTL is 60 s", wp.RIGHT_NOW_TTL == 60)
wp.realm_status = lambda: {"online": True, "players": 1}
expire_strip()
check("strip: one player is 'player'", "<b>1</b> player on" in wp.right_now_html())

# a source that breaks is left out, the others stay
def boom():
    raise RuntimeError("down")
wp.realm_status = boom
expire_strip()
items, _ = wp.right_now()
check("a broken source is left out, the rest stay", [i["kind"] for i in items] == ["realms", "mods", "eighties"], items)
# escaping of the realm name in the strip
saved = wp.REALM_NAME
wp.REALM_NAME = 'Wr<a>"th'
wp.realm_status = lambda: {"online": True, "players": 5}
expire_strip()
h = wp.right_now_html()
check("strip: realm name escaped", "Wr&lt;a&gt;&quot;th" in h and "Wr<a>" not in h)
wp.REALM_NAME = saved
wp.realm_status = lambda: {"online": True, "players": 12, "bots": 30, "updated_at": "2026-10-25T10:20:30Z"}
expire_strip()

# ---------------------------------------------------------------- 3. every page through the real handler
srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()
def get(path, ip="203.0.113.5"):
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=10)
    c.request("GET", path, headers={"X-Forwarded-For": ip}); r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read().decode("utf-8", "replace")

NAV_ITEMS = [("Leyline", "/wrath/leyline"), ("Play", "/wrath/account"), ("Realms", "/wrath/realms"), ("Modes", "/wrath/modes"),
             ("Mods", "/wrath/mods"), ("Armory", "/wrath/armory"), ("Docs", "/wrath/docs"), ("Server", wp.HOST_URL), ("Discord", wp.DISCORD_URL),
             ("Log In", "/login?next=%2Fwrath"), ("Play now", "/wrath")]
CURRENT_HREF = {"leyline": "/wrath/leyline", "play": "/wrath/account", "realms": "/wrath/realms", "modes": "/wrath/modes",
                "mods": "/wrath/mods", "armory": "/wrath/armory", "docs": "/wrath/docs"}

def nav_of(page):
    m = re.search(r'<nav class="ly-nav" id="ly-site-nav".*?</nav>', page, re.S)
    return m.group(0) if m else ""

def check_chrome(label, page, current, public=True, next_step=True, signed_in=False):
    nav = nav_of(page)
    check(f"{label}: the site nav, once", page.count('id="ly-site-nav"') == 1 and nav)
    check(f"{label}: no nav of its own left", page.count('<nav class="ly-nav"') == 1 and 'class="grp"' not in page)
    want = [x for x in NAV_ITEMS if not (signed_in and x[0] == "Log In")]
    missing = [t for t, href in want if f'href="{wp.html.escape(href)}"' not in nav or f">{t}</a>" not in nav]
    check(f"{label}: every nav item", not missing, missing)
    marked = re.findall(r'<a href="([^"]+)"[^>]*aria-current="page"', nav)
    check(f"{label}: marks {current or 'nothing'} as current", marked == ([CURRENT_HREF[current]] if current else []), marked)
    if current:
        check(f"{label}: the current item is styled", re.search(r'<a href="%s" class="[^"]*\bon\b' % re.escape(CURRENT_HREF[current]), nav))
    check(f"{label}: phone menu works without script", re.search(r'<input type="checkbox" id="ly-menu" class="ly-menu-t"><label for="ly-menu"[^>]*>Menu</label><div class="ly-links">', nav)
          and ".ly-menu-t:checked~.ly-links" in page)
    check(f"{label}: skip link to the content", 'href="#main">Skip to content' in page and 'id="main"' in page)
    foot = page.split('<footer class="ly-foot">', 1)[-1].split("</footer>", 1)[0] if '<footer class="ly-foot">' in page else ""
    check(f"{label}: footer (GitHub, Discord, Report a bug, no shop)", f'href="{wp.LEYLINE_REPO}"' in foot and f'href="{wp.DISCORD_URL}"' in foot
          and ">Report a bug<" in foot and f'href="{wp.BUG_URL}"' in foot and "No shop. Bots are labelled." in foot)
    title = re.search(r"<title>([^<]+)</title>", page)
    check(f"{label}: a title", title and title.group(1).strip())
    if public:
        check(f"{label}: a link card", 'property="og:title"' in page and 'name="twitter:card"' in page)
    if next_step:
        nxt = re.findall(r'<aside class="ly-next".*?</aside>', page, re.S)
        check(f"{label}: ends with one clear next step", len(nxt) == 1 and ("Play now</a>" in nxt[0] or "Host a realm</a>" in nxt[0])
              and page.index(nxt[0]) < page.index('<footer class="ly-foot">'))
    check(f"{label}: focus and reduced-motion styles", ":focus-visible" in page and "prefers-reduced-motion:reduce" in page)
    imgs = re.findall(r"<img\b[^>]*>", page)
    bad = [i for i in imgs if not (re.search(r'\bwidth="\d+"', i) and re.search(r'\bheight="\d+"', i))
           or not ('loading="lazy"' in i or 'fetchpriority="high"' in i)]
    check(f"{label}: images sized and lazy (the hero emblem loads first)", not bad, bad[:2])
    bad = BANNED.findall(visible(page))
    check(f"{label}: no game-brand names", not bad, bad)

PAGES = [("/wrath/leyline", "leyline", 200), ("/wrath/realms", "realms", 200), ("/wrath/modes", "modes", 200),
         ("/wrath/mods", "mods", 200), ("/wrath/mods/new-mod", "mods", 200), ("/wrath/armory", "armory", 200),
         ("/wrath/armory/Arthas", "armory", 200), ("/wrath/armory/Nobody", "armory", 404), ("/wrath/q/258", "modes", 200),
         ("/wrath/u/Arthas", "armory", 200), ("/wrath/r/wrath", "realms", 200), ("/wrath/m/new-mod", "mods", 200),
         ("/wrath/demo", "", 200), ("/wrath/docs", "docs", 200), ("/wrath/docs/mods", "docs", 200)]
for path, cur, code in PAGES:
    st, hd, page = get(path)
    check(f"{path}: {code}", st == code, st)
    check(f"{path}: still no script (CSP script-src 'none')", "<script" not in page and "script-src 'none'" in hd.get("content-security-policy", ""))
    share = path.startswith(("/wrath/q", "/wrath/u", "/wrath/r/", "/wrath/m/"))
    check_chrome(path, page, cur, public=path != "/wrath/demo", next_step=not share)

st, hd, page = get("/wrath")   # signed out: the saved-key sign-in page (it has its own nonce script)
check_chrome("/wrath (signed out)", page, "play", next_step=False)
check_chrome("sign-in page", wp.signed_out_page().decode(), "play", next_step=False)
check_chrome("error page", wp.error_page("x").decode(), "", public=False, next_step=False)
check_chrome("busy page", wp.busy_page(back="/wrath/realms").decode(), "realms", public=False, next_step=False)
check_chrome("streaming off page", wp.stream_off_page().decode(), "play", public=False, next_step=False)
acct = wp.account_page("Tester", "WR1", "pw", "n").decode()
check_chrome("account page (signed in)", acct, "play", public=False, next_step=False, signed_in=True)
check("account page: still the realm list and the Hero Key link", "Choose a realm" in acct and 'href="/wrath/key"' in acct
      and 'class="wu-panel wide"' in acct)
check("account page: no Log In once signed in", ">Log In</a>" not in nav_of(acct))
check("the gold pages keep their themed body", '<body class="wu">' in wp.signed_out_page().decode())
check("titles say which page", "<title>Realms · WrathUnleashed</title>" in get("/wrath/realms")[2]
      and "<title>Leyline</title>" in get("/wrath/leyline")[2] and "<title>Sign in · WrathUnleashed</title>" in wp.signed_out_page().decode())

# ---------------------------------------------------------------- 4. the 404
st, hd, nf = get("/wrath/no-such-page")
check("404: status and html", st == 404 and hd.get("content-type", "").startswith("text/html"), (st, hd.get("content-type")))
check("404: a styled page, not 'not found'", nf != "not found\n" and "<h1>Not found</h1>" in nf and 'class="ly"' in nf)
check_chrome("404 page", nf, "")
for href in ("/wrath", "/wrath/leyline", "/wrath/realms", "/wrath/modes", "/wrath/mods", "/wrath/armory", wp.HOST_URL):
    check(f"404: links on to {href}", f'<a class="ly-feat" href="{wp.html.escape(href)}"' in nf)
check("404: kept out of search results", '<meta name="robots" content="noindex">' in nf)
check("404: other unknown paths too", get("/nope")[0] == 404 and "Not found" in get("/x/y/z")[2])
check("404: art files stay a plain 404", get("/wrath/art/nothing.webp")[0] == 404 and "<nav" not in get("/wrath/art/nothing.webp")[2])
check("404: a path with markup is not echoed", "<b>" not in get("/wrath/%3Cb%3Ehi")[2])

# ---------------------------------------------------------------- 4b. Start here (/wrath/start): the one path for new players
def start_cards(page):
    return re.findall(r'<a class="st-card" id="st-([a-z]+)" href="([^"]+)"(?: rel="noopener noreferrer")?><span class="st-n" aria-hidden="true">\d</span>'
                      r'<h2>([^<]+)</h2><p>([^<]+)</p><span class="st-go">([^<]+) ›</span></a>', page)

st, hd, sp = get("/wrath/start")
check("/wrath/start: 200, html, no script (CSP script-src 'none')", st == 200 and hd.get("content-type", "").startswith("text/html")
      and "<script" not in sp and "script-src 'none'" in hd.get("content-security-policy", ""), st)
check_chrome("/wrath/start", sp, "", next_step=False)
cards = start_cards(sp)
links = {k: h for k, h, *_ in cards}
check("/wrath/start: four cards in order (Play in your browser, Get the launcher, Make your Den, Host a server)",
      [c[2] for c in cards] == ["Play in your browser", "Get the launcher", "Make your Den", "Host a server"], cards)
check("/wrath/start: each card is one link to the real place", links.get("play") == "/wrath" and links.get("den") == "/wrath/docs/getting-started#your-den"
      and links.get("host") == wp.html.escape(wp.HOST_URL) and links.get("launcher", "").startswith("https://github.com/orionopenwow/Leyline/releases")
      and sp.count('class="st-card"') == 4, links)
check("/wrath/start: short words (a line each), no client numbers", all(len(c[3]) <= 120 for c in cards)
      and not re.search(r"\b(1\.12|2\.4\.3|3\.3\.5)", visible(sp)))
check("/wrath/start: the longer guide and the Discord, once each", sp.count('href="/wrath/docs/getting-started"') == 1
      and f'href="{wp.DISCORD_URL}"' in sp.split('class="ly-foot"')[0])
check("/wrath/start: its own link card and title", "<title>Start here · " in sp and 'content="https://play.example/wrath/start"' in sp)
check("/wrath/start: no nav item of its own (the hub's hero and the launcher's Help link it)", wp.nav_key("/wrath/start") == ""
      and 'href="/wrath/start"' not in nav_of(sp))
check("/wrath/start/x is a 404", get("/wrath/start/x")[0] == 404)

# ---------------------------------------------------------------- 4c. Looks (/wrath/looks): graphics packs, UI looks, presets
LK = wp.looks_mod
def look_cards(page):
    return re.findall(r'<li class="lk-card" id="look-([a-z0-9-]+)" data-type="([a-z-]+)" data-how="(launcher|link)">(.*?)</div></li>', page, re.S)

check("Looks: the module beside the portal is loaded", LK is not None)
st, hd, lp = get("/wrath/looks")
check("/wrath/looks: 200, html, no script (CSP script-src 'none')", st == 200 and hd.get("content-type", "").startswith("text/html")
      and "<script" not in lp and "script-src 'none'" in hd.get("content-security-policy", ""), st)
check_chrome("/wrath/looks", lp, "", next_step=False)
cards = look_cards(lp)
ids = [c[0] for c in cards]
pub = [e["id"].split(":", 1)[1] for e in LK.published()]
check("/wrath/looks: every published look, in its order, and nothing that waits for a review", ids == pub and len(ids) >= 6
      and "elvui" not in ids and "clean-frames" not in ids, ids)
by = {c[0]: c for c in cards}
check("/wrath/looks: the presets install with one click, through the launcher's link",
      all(by[k][2] == "launcher" and f'href="leyline://install/look:{k}">Install with Leyline</a>' in by[k][3] for k in ("max", "balanced", "classic")))
check("/wrath/looks: link-only looks open their author's page (never a file), with no Install button",
      all(c[2] == "link" and "Install with Leyline" not in c[3] and "Author's page</a>" in c[3]
          and 'rel="noopener nofollow ugc"' in c[3] for c in cards if c[0] in ("forever-ui", "ultrahd", "epoch-water", "pfui")))
check("/wrath/looks: each card credits its author, licence (or that none is stated) and source",
      "Forever UI by Adrestie · GPL-2.0 · github.com" in lp and "UltraHD Graphics Modpack by the Conquest of Azeroth modpack authors · licence not stated · reddit.com" in lp
      and all('class="lk-credit"' in c[3] for c in cards))
check("/wrath/looks: what it changes, its size or that it needs no download, which clients (chips, in words)",
      all('class="lk-ch"><b>Changes:</b>' in c[3] and 'class="lk-meta"' in c[3] and 'class="lk-chips"' in c[3] for c in cards)
      and "Nothing to download" in by["max"][3] and "330 MB" in by["forever-ui"][3] and "<li>Third era</li>" in by["forever-ui"][3])
check("/wrath/looks: no client numbers anywhere a visitor reads, chips included", not re.search(r"\b(1\.12|2\.4\.3|3\.3\.5)", visible(lp)))
PICS = {"max", "balanced", "classic", "forever-ui"}   # our own pictures (stream/looks.json look.previews)
check("/wrath/looks: our own pictures on the looks that have one, sized never wider than the picture, with their credit",
      all(re.search(r'<figure class="lk-pv"><img src="/wrath/art/looks/look-%s(?:-hud)?\.webp" alt="[^"]+" width="(\d+)" height="\d+" style="max-width:\1px"' % k, by[k][3])
          and "<figcaption>" in by[k][3] for k in PICS)
      and "<figcaption>Screenshot by the Leyline team</figcaption>" in by["max"][3]
      and "<figcaption>Wrath Unleashed, in game</figcaption>" in by["forever-ui"][3], [by[k][3][:200] for k in PICS])
check("/wrath/looks: every other look its own drawing (no picture of anyone else's)",
      all('<img' not in c[3] and 'class="lk-art m-' in c[3] for c in cards if c[0] not in PICS)
      and 'class="lk-art m-pack"' in by["ultrahd"][3] and 'class="lk-art m-ui"' in by["dragonflight-ui"][3] and ".m-classic .m-far" in lp)
LOOKS_SRC = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "www-files", "art", "looks")
os.makedirs(os.path.join(ART, "looks"), exist_ok=True)
PIC_FILES = sorted({p["src"].rsplit("/", 1)[1] for e in LK.published() for p in e["look"]["previews"]})
for f in PIC_FILES:
    shutil.copy(os.path.join(LOOKS_SRC, f), os.path.join(ART, "looks"))
st, hd, pic = get("/wrath/art/looks/look-max.webp")
check("/wrath/art/looks/<name>.webp: served from the allow-list, a real webp", st == 200 and hd.get("content-type") == "image/webp"
      and get("/wrath/art/looks/../wu-theme.css")[0] == 404 and get("/wrath/art/looks/other.png")[0] == 404, st)
check("the Looks pictures in the repo: each one a webp named in a published look's previews", sorted(os.listdir(LOOKS_SRC)) ==
      PIC_FILES and len(PIC_FILES) == 7 and all(open(os.path.join(LOOKS_SRC, f), "rb").read(12)[8:] == b"WEBP" for f in os.listdir(LOOKS_SRC)))
fu = by["forever-ui"][3]
thumbs = re.findall(r'<li><a href="#(lk-pic-forever-ui-\d)" aria-label="([^"]+) \(larger\)"><img src="/wrath/art/looks/(look-forever-ui-[a-z]+\.webp)"', fu)
boxes = re.findall(r'<div class="lk-lb" id="(lk-pic-forever-ui-\d)" role="dialog"[^>]*><a class="lk-lb-bg" href="#look-forever-ui" aria-label="Close"></a>'
                   r'<figure><img src="/wrath/art/looks/([a-z-]+\.webp)" alt="([^"]+)"', lp)
check("/wrath/looks: Forever UI shows its four in-game pictures as a strip, each opening a larger view (no script: :target)",
      [t[2] for t in thumbs] == ["look-forever-ui-hud.webp", "look-forever-ui-charselect.webp", "look-forever-ui-character.webp",
                                 "look-forever-ui-professions.webp"] and [b[0] for b in boxes] == [t[0] for t in thumbs]
      and boxes[1][2] == "Character select with the Forever UI look" and ".lk-lb:target{" in lp
      and 'href="#lk-pic-forever-ui-2">Next ›</a>' in lp and 'href="#lk-pic-forever-ui-4">‹ Previous</a>' in lp
      and lp.index('class="lk-lb"') > lp.index('<p class="fine">')   # the full views outside the card and the box
      and "<script" not in lp and 'class="lk-thumbs"' not in by["max"][3], (thumbs, boxes))
FLAG = os.path.join(TMP, "gamedata-off")
os.environ["WU_GAMEDATA_FLAG"] = FLAG
open(FLAG, "w").write("off\n")
st410 = get("/wrath/art/looks/look-max.webp")[0]
lk_off = get("/wrath/looks")[2]
os.remove(FLAG)
check("game data killswitch: the Looks pictures answer 410 and the cards show the drawings instead", st410 == 410
      and "/wrath/art/looks/" not in lk_off and 'class="lk-art m-ultra"' in lk_off)
check("/wrath/looks: placeholders of others' looks link to the author's page for pictures",
      "Pictures on the author's page" in by["ultrahd"][3] and "Pictures on the author" not in by["max"][3])
st, _, lv = get("/wrath/looks?v=1.12")
vids = [c[0] for c in look_cards(lv)]
check("/wrath/looks?v=1.12: only looks for that client, and its chip marked", st == 200 and vids and all(
      "1.12" in next(e for e in LK.published() if e["id"] == "look:" + i)["builds"] for i in vids) and "forever-ui" not in vids
      and '<a href="/wrath/looks?v=1.12" aria-current="true">Classic era</a>' in lv, vids)
st, _, lt = get("/wrath/looks?t=preset&v=3.3.5a")
check("/wrath/looks?t=preset: the three presets; the type chip keeps the client", [c[0] for c in look_cards(lt)] == ["max", "balanced", "classic"]
      and '<a href="/wrath/looks?v=3.3.5a&amp;t=preset" aria-current="true">Presets</a>' in lt)
st, _, lx = get("/wrath/looks?t=%3Cb%3E&v=9.9")
check("/wrath/looks: unknown filters are ignored, never echoed", st == 200 and len(look_cards(lx)) == len(cards) and "<b>" not in lx.split("<main")[-1].split('class="lk-grid"')[0].replace("<b>Client</b>", "").replace("<b>Type</b>", ""))
check("/wrath/looks/x is a 404", get("/wrath/looks/x")[0] == 404)
st, _, aj = get("/wrath/mods/all.json")
try:
    projs = {p["id"]: p for p in json.loads(aj)["projects"]}
except (ValueError, KeyError):
    projs = {}
check("all.json: the published looks, source look, how each installs worked out, the credit line",
      projs.get("look:max", {}).get("source") == "look" and projs["look:max"]["look"]["how"] == "launcher"
      and projs.get("look:ultrahd", {}).get("look", {}).get("how") == "link" and "look:elvui" not in projs
      and projs["look:forever-ui"]["credits"] == "Forever UI by Adrestie · GPL-2.0 · github.com"
      and "review" in projs["look:max"] and not projs["look:max"]["review"]["notes"], sorted(projs)[:5])

# ---------------------------------------------------------------- 5. share boxes and update times
st, _, realms = get("/wrath/realms")
boxes = re.findall(r'<details class="ly-share"><summary>Share</summary><code tabindex="0">([^<]+)</code>', realms)
check("realms: a share box per realm card", boxes == ["https://play.example/wrath/r/wrath", "https://play.example/wrath/r/abcdefgh",
      "https://play.example/wrath/r/qrstuvwx", "https://play.example/wrath/r/aaaaaaaa"], boxes)   # short links (8 letters), 31 Oct
check("realms: every share link is a real share link", all(wp.links.parse(b.split("play.example", 1)[1]) for b in boxes))
check("realms: status time shown", 'Realm status <time datetime="2026-10-25T10:20:30Z">10:20 UTC</time>' in realms)
check("realms: community list time shown", re.search(r'Community list <time datetime="[^"]+">\d\d:\d\d UTC</time>', realms))
check("realms: ends with Host a realm", "Host a realm</a>" in re.findall(r'<aside class="ly-next".*?</aside>', realms, re.S)[0])
check("realms: the old Back link is gone", 'class="keylink" href="/wrath">Back' not in realms)
st, _, md = get("/wrath/modes")
go = re.findall(r'class="md-go" href="(/wrath/[hqb]/[^"]+)"', md)
sh = re.findall(r'<code tabindex="0">https://play\.example(/wrath/[hqb]/[^<]+)</code>', md)
check("modes: a share box on every card, with that card's link", len(go) > 50 and go == sh, (len(go), len(sh)))
st, _, sheet = get("/wrath/armory/Arthas")
check("armory: the sheet's share link", 'href="/wrath/u/Arthas"' in sheet and "https://play.example/wrath/u/Arthas</code>" in sheet)
check("armory: last updated shown", "Updated every 5 minutes (last: " in sheet)
check("leyline: realm list update time", re.search(r'Realm status <time datetime="2026-10-25T10:20:30Z">', get("/wrath/leyline")[2]))
check("updated_text: junk is nothing", wp.updated_text("<b>") == "" and wp.updated_text(None) == "" and wp.updated_text(0) == "")

# ---------------------------------------------------------------- 6. the pieces
check("nav_key", [wp.nav_key(p) for p in ("/wrath/leyline", "/wrath", "/wrath/account", "/wrath/realms", "/wrath/r/x",
      "/wrath/modes", "/wrath/q/258", "/wrath/mods/x", "/m/x", "/wrath/armory/A", "/u/A", "/wrath/x", "/wrath/docs", "/wrath/docs/mods")]
      == ["leyline", "play", "play", "realms", "realms", "modes", "modes", "mods", "mods", "armory", "armory", "", "docs", "docs"])
once = wp.site_chrome(get("/wrath/modes")[2].encode(), "/wrath/modes")
check("site_chrome: never twice", once.decode().count('id="ly-site-nav"') == 1)
check("site_chrome: leaves non-pages alone", wp.site_chrome(b"plain", "/x") == b"plain" and wp.site_chrome(b"\xff\xfe", "/x") == b"\xff\xfe")
mod_page = wp.site_chrome(b'<!doctype html><html><head><title>T &amp; U</title></head><body class="ly"><div class="ly-glow"></div>'
                          b'<nav class="ly-nav" aria-label="Leyline"><div class="grp"><a href="/x">Old</a></div></nav>'
                          b'<div class="ly-wrap"><img src="/a.webp" width="2" height="2"></div></body></html>', "/wrath/mods").decode()
check("site_chrome: a module's own nav is replaced", ">Old<" not in mod_page and 'aria-current="page">Mods<' in mod_page)
check("site_chrome: a link card added when missing", 'property="og:title" content="T &amp; U"' in mod_page)
check("site_chrome: images made lazy", '<img loading="lazy" decoding="async" src="/a.webp"' in mod_page)
check("share_box escapes", "&lt;" in wp.share_box("/wrath/<x>") and "<x>" not in wp.share_box("/wrath/<x>"))

# ---------------------------------------------------------------- 7. no game-brand names in the new code's strings
for f in ("wrath_portal.py", "modes.py", "armory.py"):
    src = open(os.path.join(os.path.dirname(os.path.abspath(sys.argv[1])), f), encoding="utf-8").read()
    check(f"no game-brand names in {f}", not BANNED.findall(src), BANNED.findall(src)[:3])

# ---------------------------------------------------------------- 8. the hub's frame (front door on): one product
# With WU_HUB_FRONT=1 (on with the brand) the play and sign-in pages, the docs and the 404 are drawn in the hub's frame
# (hub.Hub._shell: the stone side menu, tokens, Marcellus, footer); the module pages still in the old bar list the
# hub's menu. Same forms, scripts and CSP as before.
os.environ["WU_HUB_FRONT"] = "1"
HUB_MENU = [("Play", "/wrath"), ("Realms", "/wrath/leyline/realms"), ("Mods", "/wrath/leyline/catalog"),
            ("Addons", "/wrath/leyline/addons"), ("Looks", "/wrath/looks"), ("Host", "/wrath/host"), ("Forum", "/wrath/leyline/forum"),
            ("Leaderboards", "/wrath/leaderboards"), ("Docs", "/wrath/docs"), ("News", "/wrath/leyline/news")]
OLD_ITEMS = re.compile(r">(?:Workshop|Armory|Server|Play now)</a>|class=\"ly-hot\"")

def hub_menu(page):
    m = re.search(r'<nav class="hb-nav"[^>]*>(.*?)</nav>', page, re.S)
    return m.group(1) if m else ""

def check_frame(label, page, active, scripts=0):
    nav = hub_menu(page)
    check(f"{label}: the hub's frame (side menu, footer, tokens), not the old bar",
          page.count('<header class="hb-top">') == 1 and 'class="hb-foot"' in page and 'id="ly-site-nav"' not in page
          and '<nav class="ly-nav"' not in page and 'class="ly-foot"' not in page and "--lk-surface" in page and 'data-look="leyline"' in page)
    missing = [t for t, h in HUB_MENU if not re.search(r'href="%s"[^>]*><svg[^>]*>.*?</svg><span>%s</span>' % (re.escape(h), t), nav, re.S)]
    check(f"{label}: the hub's menu items", not missing, missing)
    check(f"{label}: none of the old menu (Workshop, Armory, Server, Play now)", not OLD_ITEMS.search(page))
    on = re.findall(r'<a class="hb-ni on" href="([^"]+)" aria-current="page"', nav)
    check(f"{label}: marks {active or 'nothing'}", on == ([active] if active else []), on)
    check(f"{label}: Marcellus headings and the stone button", '"Marcellus"' in page and ".wu-box h1" in page and "stone-btn.webp" in page)
    check(f"{label}: no preview flag, no hidden-page robots unless asked", "Preview:" not in page and "leyline-preview" not in page)
    check(f"{label}: {scripts} script(s)", page.count("<script") == scripts, page.count("<script"))
    bad = BANNED.findall(visible(page))
    check(f"{label}: no game-brand names", not bad, bad)

so = wp.signed_out_page().decode()
check_frame("framed sign-in page", so, "/wrath")
check("framed sign-in page: the same Sign in link, its link card and title", f'href="{wp.html.escape(wp.signin_url("/wrath"))}">Sign in</a>' in so
      and 'property="og:title"' in so and "<title>Sign in · WrathUnleashed</title>" in so and so.count('name="description"') == 1)
acct = wp.account_page("Tester", "WR1", "pw", "n0nce").decode()
check_frame("framed account page", acct, "/wrath", scripts=2)
check("framed account page: the realm list, the Hero Key link, the copy fields and the + Your server link",
      "Choose a realm" in acct and 'href="/wrath/key"' in acct and 'data-copy="acct"' in acct and 'id="pw" data-v="pw"' in acct
      and 'class="ly-plus" href="/wrath/host"' in acct and 'class="wu-pg wide"' in acct)
check("framed account page: both scripts carry the page's nonce (COPY_JS, the hub's)",
      acct.count('<script nonce="n0nce">') == 2 and wp.COPY_JS in acct)
check("framed account page: the gold realm styles are not sent", wp.REALM_CSS not in acct)
check("framed account page: no client version in a sentence (the chip stays)", ">3.3.5a</span>" in acct
      and not re.search(r"[A-Za-z,] 3\.3\.5a|3\.3\.5a [a-z]", re.sub(r"<[^>]+>", " ", acct)))
check_frame("framed error page", wp.error_page("x").decode(), "")
busy = wp.busy_page(back="/wrath/realms").decode()
check_frame("framed busy page", busy, "/wrath/leyline/realms")
check("framed busy page: still tries again by itself", '<meta http-equiv="refresh" content="15;url=/wrath/realms">' in busy)
st, hd, page = get("/wrath")
check_frame("framed /wrath (signed out)", page, "/wrath", scripts=2)
n = re.search(r"'nonce-([^']+)'", hd.get("content-security-policy", ""))
check("framed /wrath: the saved-key script and the hub's under the response's nonce, nothing else",
      n and page.count(f'<script nonce="{n.group(1)}">') == 2 and "/auth/key" in page and "connect-src 'self'" in hd["content-security-policy"])
st, hd, page = get("/wrath/docs")
check_frame("framed /wrath/docs", page, "/wrath/docs")
check("framed /wrath/docs: 200, the table of contents and the docs' tokens, no script allowed",
      st == 200 and 'aria-label="Documentation"' in page and 'class="ld-doc"' in page and "--lk-accent-soft" in page
      and "leyline-bg.webp" not in page and "script-src 'none'" in hd.get("content-security-policy", ""))
check("framed /wrath/docs: its link card", 'property="og:title" content="Leyline · Leyline docs"' in page)
st, hd, page = get("/wrath/start")
check_frame("framed /wrath/start", page, "")
fc = {k: h for k, h, *_ in start_cards(page)}
check("framed /wrath/start: 200, the four cards, Host to our own host page, the hub's card styles, no script allowed",
      st == 200 and list(fc) == ["play", "launcher", "den", "host"] and fc["host"] == ("/wrath/host" if wp.hostflow is not None else wp.html.escape(wp.HOST_URL))
      and 'class="wu-pg wide st"' in page and "a.st-card" in page and "script-src 'none'" in hd.get("content-security-policy", ""), fc)
check("framed /wrath/start: its link card", 'property="og:title" content="Start here · Leyline"' in page)
st, hd, page = get("/wrath/looks")
check_frame("framed /wrath/looks", page, "/wrath/looks")
check("framed /wrath/looks: 200, the cards, the box styles, Get the launcher linked, no script allowed",
      st == 200 and len(look_cards(page)) == len(LK.published()) and 'class="wu-pg wide lk"' in page and ".lk-card{" in page
      and "Get the launcher</a>" in page and "script-src 'none'" in hd.get("content-security-policy", ""))
check("framed /wrath/looks: its link card", 'property="og:title" content="Looks · Leyline"' in page)
st, _, page = get("/wrath/docs/nope")
check("framed docs 404", st == 404 and "There is no docs page with that name" in page and 'class="hb-top"' in page)
if wp.bugreport is not None and wp.bugreport.ON:
    st, _, bug = get("/wrath/bugs")
    check_frame("framed /wrath/bugs", bug, "")
    check("framed /wrath/bugs: the same page inside (sign in first, the other ways)", st == 200 and "<h1>Report a bug</h1>" in bug
          and "/login?next=%2Fwrath%2Fbugs" in bug)
st, hd, nf = get("/wrath/no-such-page")
check_frame("framed 404", nf, "")
check("framed 404: status, kept out of search, links on (Host to our own page)", st == 404 and '<meta name="robots" content="noindex">' in nf
      and '<a class="ly-feat" href="/wrath/host">' in nf and "<h1>Not found</h1>" in nf)
st, _, md = get("/wrath/modes")
nav = nav_of(md)
check("module pages in the old bar list the hub's menu", 200 == st and all(f'href="{h}"' in nav and f">{t}</a>" in nav for t, h in HUB_MENU)
      and not OLD_ITEMS.search(nav), nav[:400])
os.environ["WU_HUB_FRONT"] = "0"
check("front door off: the gold frame as before", 'id="ly-site-nav"' in wp.signed_out_page().decode()
      and 'class="hb-top"' not in wp.signed_out_page().decode())

srv.shutdown()
shutil.rmtree(TMP, ignore_errors=True)
print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
