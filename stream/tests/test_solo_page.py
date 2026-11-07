"""stream/solo_page.py on the portal: /wrath/solo, Leyline Solo's release page (plan Phase 5). Both frames (the hub's
front door on and off), every section the owner asked for (what it is, who it's for, the three steps, companions,
Invite a friend, Characters, the honest notes, the questions), the download only with WU_SOLO_RELEASED set ("Coming
soon" and no repo link before), the owner's screenshot slots (panels until he adds them, off with the killswitch), the
links from Single player and the docs but not the main nav, no game brand names (only the one legal line), no client
version numbers in prose, "Log in", outside links only Discord, Reddit and our own source, no money links, and the
deploy (the signin step packs it, fastloop installs it).

    python3 -I stream/tests/test_solo_page.py stream/wrath_portal.py
"""
import html, http.client, importlib.util, os, re, shutil, sys, tempfile, threading
from http.server import ThreadingHTTPServer

PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
ROOT = os.path.dirname(STREAM)
SRC_ART = os.path.join(STREAM, "www-files", "art")
TMP = tempfile.mkdtemp(prefix="wu-solo-")
MODS, ARM, ART = (os.path.join(TMP, d) for d in ("mods", "armory", "art"))
for d in (os.path.join(MODS, "files"), os.path.join(ARM, "c"), ART, os.path.join(ART, "slots")):
    os.makedirs(d)
for n in os.listdir(SRC_ART):
    if os.path.isfile(os.path.join(SRC_ART, n)):
        shutil.copy(os.path.join(SRC_ART, n), ART)
os.environ.update(WU_DL_PROBE="0", SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example", WU_MODS_DIR=MODS,
                  WU_ARMORY_DIR=ARM, WU_ART_DIR=ART, WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json",
                  WU_JOIN_BASE="https://relay.example", LEYLINE_BRAND="1", WU_PORTAL_DATA=os.path.join(TMP, "data"),
                  WU_GAMEDATA_FLAG=os.path.join(TMP, "gamedata-off"))
for k in ("WU_HUB_FRONT", "WU_SOLO_RELEASED", "SINGLE_PLAYER_PUBLIC"):
    os.environ.pop(k, None)
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
so = wp.solo_page

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, str(extra)[:600])

srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()
_ip = [20]
def get(path):
    _ip[0] += 1   # a new address each time: the page's rate limit never gets in the way
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=20)
    c.request("GET", path, headers={"X-Forwarded-For": f"203.0.113.{_ip[0]}"}); r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()

LEGAL = "not affiliated with or endorsed by Blizzard Entertainment"
BANNED = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
VERSION = re.compile(r"(?<![\w.])(?:1\.12(?:\.\d)?|2\.4\.3|3\.3\.5a?|4\.3\.4|5\.4\.8|5875|12340)(?![\w.])")
def visible(p):
    p = re.sub(r"<(style|script)\b.*?</\1>", " ", p, flags=re.S)
    p = re.sub(r"<!--.*?-->", " ", p, flags=re.S)
    attrs = " ".join(re.findall(r'(?:alt|title|aria-label|placeholder|content)="([^"]*)"', p))
    text = re.sub(r"<[^>]+>", " ", p) + " " + attrs
    return html.unescape(re.sub(r"https?://\S+", " ", text))

REPO = "https://github.com/orionopenwow/leyline-solo"
DL = REPO + "/releases/latest"

check("solo_page loaded and bound", so is not None and callable(so.portal("card_meta")))
check("the route: /wrath/solo and /wrath/solo/ only", so.match("/wrath/solo") and so.match("/wrath/solo/")
      and not so.match("/wrath/solo/x") and not so.match("/wrath/solos") and not so.match("/wrath/soloist"))
check("the flag is unset by default", not so.released())
for v, want in (("1", True), ("true", True), ("yes", True), ("0", False), ("", False), ("no", False)):
    os.environ["WU_SOLO_RELEASED"] = v
    check(f"WU_SOLO_RELEASED={v!r}: released() is {want}", so.released() is want)
os.environ.pop("WU_SOLO_RELEASED")
check("the slots are allowed art names", all(wp.art_type(n) == "image/webp" for n, *_ in so.SHOTS))


def page_checks(label, released):
    st, hd, raw = get("/wrath/solo")
    page = raw.decode("utf-8", "replace")
    check(f"{label}: 200 html", st == 200 and hd.get("content-type", "").startswith("text/html"), st)
    check(f"{label}: one h1", page.count("<h1") == 1, page.count("<h1"))
    v = visible(page)
    check(f"{label}: no game brand names (only the legal line)", not BANNED.findall(v.replace(LEGAL, "")), BANNED.findall(v.replace(LEGAL, ""))[:4])
    check(f"{label}: the legal line", LEGAL in page)
    check(f"{label}: no client version numbers in prose", not VERSION.findall(v), VERSION.findall(v)[:4])
    check(f"{label}: says Log in, never Sign in", not re.search(r"\bSign in\b|\bsign in\b", visible(page.split("<main", 1)[-1])))
    check(f"{label}: no money links", not re.search(r"(?i)donat|patreon|ko-fi|paypal|tip jar|buy me|kickstarter", v))
    ours = page[page.find('class="lp so"'):]
    ours = re.split(r"<footer", ours, maxsplit=1)[0]
    ext = [h for h in re.findall(r'href="(https?://[^"]+)"', ours)
           if not re.match(r"https://(discord\.gg|discord\.com|(www\.)?reddit\.com|github\.com/orionopenwow/)", h)]
    check(f"{label}: outside links only Discord, Reddit and our own source", not ext, ext[:4])
    for sec in ("who", "start", "companions", "invite", "characters", "notes", "faq", "download"):
        check(f"{label}: section #{sec}", f'id="{sec}"' in page)
    for words in ("Install", "Point it at your game folder", "Press Play", "Quick start", ".bot group", "size=3",
                  ".bot add Name", ".bot remove Name", "follow", "stay", "attack", "Invite a friend",
                  "On the same network", "through the Leyline relay", "Characters", "first-era",
                  "You need your own game folder", "Windows first", "never downloads, ships or uploads any of the game",
                  "up to an hour", "logs you in by itself", "no internet", "Game files never travel",
                  "Remove sign-in", "Simple sign-in"):
        check(f"{label}: says {words!r}", words in v)
    check(f"{label}: no game data offered (no .mpq, no client download)", not re.search(r"(?i)\.mpq|client download|game files download", v))
    check(f"{label}: the owner's screenshot slots, each a panel until he adds it",
          all(f'data-slot="{n}"' in page for n, *_ in so.SHOTS) and page.count("Picture coming soon") >= len(so.SHOTS))
    check(f"{label}: no img from the game's files (only our emblem and the owner's slots)",
          all(s == "/wrath/art/leyline-emblem.webp" or s.startswith("/wrath/art/slots/solo-")
              for s in re.findall(r'<img[^>]+src="([^"]+)"', ours)), re.findall(r'<img[^>]+src="([^"]+)"', ours))
    check(f"{label}: the emblem art", 'src="/wrath/art/leyline-emblem.webp"' in page)
    check(f"{label}: owner-voice lines marked", "TODO(owner)" in page)   # in HTML comments
    check(f"{label}: the guide is linked", 'href="/wrath/docs/solo"' in page and 'href="/wrath/docs/solo#playing-with-companions"' in page)
    check(f"{label}: no link to Single player while it is dark", 'href="/wrath/single-player"' not in ours)
    if released:
        check(f"{label}: the download is the repo's latest release", f'href="{DL}"' in page and ">Download for Windows<" in page)
        check(f"{label}: the source code link", f'href="{REPO}"' in page and ">Source code<" in page)
        check(f"{label}: no Coming soon", "Coming soon" not in v)
    else:
        check(f"{label}: Coming soon in place of the download", "Coming soon" in v and page.count('class="so-soon"') == 2)
        check(f"{label}: no download and no repo link before release", REPO not in page)
    return page


page = page_checks("Leyline look", released=False)
nav = page.split('class="lp so"')[0]
check("Leyline look: not in the main nav yet", 'href="/wrath/solo"' not in nav and "/wrath/solo" not in wp.site_nav())

os.environ["WU_SOLO_RELEASED"] = "1"
page_checks("released", released=True)
os.environ.pop("WU_SOLO_RELEASED")

# the owner's screenshot, once he adds it: shown with alt text and its size; off with the game data killswitch
with open(os.path.join(SRC_ART, "leyline-emblem.webp"), "rb") as f:
    webp = f.read()
with open(os.path.join(ART, "slots", "solo-characters.webp"), "wb") as f:
    f.write(webp)
st, _, raw = get("/wrath/solo")
p = raw.decode("utf-8")
check("a screenshot he added is shown, with alt text and a size",
      re.search(r'<img src="/wrath/art/slots/solo-characters\.webp" alt="[^"]{12,}" width="\d+" height="\d+"', p)
      and 'data-slot="slots/solo-characters.webp"' not in p)
st, hd, b = get("/wrath/art/slots/solo-characters.webp")
check("the art route serves it", st == 200 and hd.get("content-type") == "image/webp", st)
open(os.environ["WU_GAMEDATA_FLAG"], "w").close()
st, _, raw = get("/wrath/solo")
check("the killswitch: the slot is a panel again (a picture of the game)", 'data-slot="slots/solo-characters.webp"' in raw.decode("utf-8"))
os.remove(os.environ["WU_GAMEDATA_FLAG"])
os.remove(os.path.join(ART, "slots", "solo-characters.webp"))

# the hub's front door on: the same page in the hub's frame
os.environ["WU_HUB_FRONT"] = "1"
if wp.hub_framed():
    page = page_checks("hub frame", released=False)
    check("hub frame: not in the hub's menu yet", 'href="/wrath/solo"' not in page.split('class="lp so"')[0])
else:
    check("hub frame: the hub isn't on in this checkout (skipped)", True)
os.environ.pop("WU_HUB_FRONT")

# linked from Single player (once that page is public) and from the docs
os.environ["SINGLE_PLAYER_PUBLIC"] = "1"
st, _, raw = get("/wrath/single-player")
sp = raw.decode("utf-8")
check("Single player links Leyline Solo", st == 200 and 'href="/wrath/solo"' in sp and 'id="solo"' in sp, st)
check("Single player's Solo part: no brand words, no versions", not BANNED.findall(visible(sp).replace(LEGAL, "")) and not VERSION.findall(visible(sp)))
st, _, raw = get("/wrath/solo")
check("with Single player public, the Solo page links it", 'href="/wrath/single-player"' in raw.decode("utf-8"))
os.environ.pop("SINGLE_PLAYER_PUBLIC")
check("the docs list has Leyline Solo, after Single player", [n for n, _ in wp.docs.PAGES].index("solo") == [n for n, _ in wp.docs.PAGES].index("single-player") + 1
      and wp.docs.TITLES["solo"] == "Leyline Solo")
st, _, raw = get("/wrath/docs/solo")
dp = raw.decode("utf-8")
dv = visible(dp)
check("/wrath/docs/solo: 200, links the showcase page", st == 200 and 'href="/wrath/solo"' in dp, st)
check("/wrath/docs/solo: no game brand names, no versions", not BANNED.findall(dv.replace(LEGAL, "")) and not VERSION.findall(dv), BANNED.findall(dv)[:3])
for h in ("install", "first-run", "quick-start", "playing-with-companions", "characters", "invite-a-friend", "settings",
          "troubleshooting", "removing-everything"):
    check(f"/wrath/docs/solo: #{h}", f'id="{h}"' in dp)
for words in ("Quick sign-in", "Simple sign-in", "Remove sign-in", "This is not the first-era client", "A port is in use",
              "Windows Firewall", "antivirus", "Setup seems stuck", "Login interface files are corrupt", "%APPDATA%\\Leyline Solo"):
    check(f"/wrath/docs/solo: says {words!r}", words in dv)
check("/wrath/docs/solo: no raw HTML comment shown", "<!--" not in dv)
check("the Single player guide links Leyline Solo", "solo.md" in open(os.path.join(ROOT, "docs", "leyline", "single-player.md"), encoding="utf-8").read())

# the deploy: the signin step packs the module, fastloop installs it, backs it up, puts it back and checks the page
bat = open(os.path.join(STREAM, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
fl = open(os.path.join(STREAM, "fastloop.sh"), encoding="utf-8").read()
check("signin packs solo_page.py", 'set "LINKS=%LINKS% solo_page.py"' in bat and "mobile_page.py solo_page.py" in bat)
check("fastloop installs it, backs it up and puts it back on failure", fl.count("solo_page.py") >= 3
      and fl.count("for f in site_pages.py mobile_page.py solo_page.py") == 3)
check("fastloop checks /wrath/solo", "/wrath/solo" in fl)
check("the module: standard library only", not re.search(r"^\s*(?:import|from)\s+(?!html\b|os\b|__future__\b)", open(os.path.join(STREAM, "solo_page.py"), encoding="utf-8").read(), re.M))

srv.shutdown()
shutil.rmtree(TMP, ignore_errors=True)
print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
