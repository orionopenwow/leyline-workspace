"""stream/mobile_page.py on the portal: /wrath/mobile, Leyline on your phone (docs/MOBILE-RELEASE.md). Both frames (the
hub's front door on and off), every section and guide the owner asked for, the screenshots (allow-listed under
/wrath/art/mobile/, real small WebP files, up with the game data killswitch, a missing one is a panel), the Android
download on our GitHub releases (and the mobile repo once WU_MOBILE_REPO names it), the nav and the docs page, no game
brand names (only the one legal line), no client version numbers in prose, "Log in", outside links only Discord, Reddit
and our own source, no money links.

    python3 -I stream/tests/test_mobile_page.py stream/wrath_portal.py
"""
import http.client, importlib.util, os, re, shutil, sys, tempfile, threading
from http.server import ThreadingHTTPServer

PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
SRC_ART = os.path.join(STREAM, "www-files", "art")
TMP = tempfile.mkdtemp(prefix="wu-mobile-")
MODS, ARM, ART = (os.path.join(TMP, d) for d in ("mods", "armory", "art"))
for d in (os.path.join(MODS, "files"), os.path.join(ARM, "c"), ART):
    os.makedirs(d)
for n in os.listdir(SRC_ART):
    if os.path.isfile(os.path.join(SRC_ART, n)):
        shutil.copy(os.path.join(SRC_ART, n), ART)
shutil.copytree(os.path.join(SRC_ART, "mobile"), os.path.join(ART, "mobile"))
os.environ.update(WU_DL_PROBE="0", SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example", WU_MODS_DIR=MODS,
                  WU_ARMORY_DIR=ARM, WU_ART_DIR=ART, WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json",
                  WU_JOIN_BASE="https://relay.example", LEYLINE_BRAND="1", WU_PORTAL_DATA=os.path.join(TMP, "data"),
                  WU_GAMEDATA_FLAG=os.path.join(TMP, "gamedata-off"))
for k in ("WU_HUB_FRONT", "WU_MOBILE_REPO"):
    os.environ.pop(k, None)
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
mp = wp.mobile_page

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, str(extra)[:600])

srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()
def get(path, ip="203.0.113.9"):
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=20)
    c.request("GET", path, headers={"X-Forwarded-For": ip}); r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()

LEGAL = "not affiliated with or endorsed by Blizzard Entertainment"
BANNED = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
VERSION = re.compile(r"(?<![\w.])(?:1\.12(?:\.\d)?|2\.4\.3|3\.3\.5a?|4\.3\.4|5\.4\.8)(?![\w.])")
def visible(p):
    p = re.sub(r"<(style|script)\b.*?</\1>", " ", p, flags=re.S)
    p = re.sub(r"<!--.*?-->", " ", p, flags=re.S)
    attrs = " ".join(re.findall(r'(?:alt|title|aria-label|placeholder|content)="([^"]*)"', p))
    text = re.sub(r"<[^>]+>", " ", p) + " " + attrs
    return re.sub(r"https?://\S+", " ", text)

check("mobile_page loaded and bound", mp is not None and callable(mp.portal("card_meta")))
check("the route: /wrath/mobile and /wrath/mobile/ only", mp.match("/wrath/mobile") and mp.match("/wrath/mobile/")
      and not mp.match("/wrath/mobile/x") and not mp.match("/wrath/mobiles"))

SHOTS = [n for n, *_ in mp.SHOTS]
for n in SHOTS:
    f = os.path.join(SRC_ART, "mobile", n + ".webp")
    ok = os.path.isfile(f)
    head = open(f, "rb").read(12) if ok else b""
    check(f"screenshot {n}.webp is there, a real WebP, small", ok and head[:4] == b"RIFF" and head[8:12] == b"WEBP"
          and os.path.getsize(f) < 150_000, os.path.getsize(f) if ok else "missing")
check("only the listed screenshots in art/mobile", sorted(os.listdir(os.path.join(SRC_ART, "mobile"))) == sorted(n + ".webp" for n in SHOTS))
check("art: mobile/<name>.webp is allowed, nothing else in that folder",
      wp.art_type("mobile/phone-play.webp") == "image/webp" and not wp.art_type("mobile/phone-play.png")
      and not wp.art_type("mobile/../wu-theme.css") and not wp.art_type("mobile/x/y.webp"))

def page_checks(label):
    st, hd, raw = get("/wrath/mobile")
    page = raw.decode("utf-8", "replace")
    check(f"{label}: 200 html", st == 200 and hd.get("content-type", "").startswith("text/html"), st)
    check(f"{label}: one h1", page.count("<h1") == 1, page.count("<h1"))
    v = visible(page)
    check(f"{label}: no game brand names (only the legal line)", not BANNED.findall(v.replace(LEGAL, "")), BANNED.findall(v.replace(LEGAL, ""))[:4])
    check(f"{label}: the legal line", LEGAL in page)
    check(f"{label}: no client version numbers in prose", not VERSION.findall(v), VERSION.findall(v)[:4])
    check(f"{label}: says Log in, never Sign in", not re.search(r"\bSign in\b|\bsign in\b", visible(page.split("<main", 1)[-1])))
    check(f"{label}: no money links", not re.search(r"(?i)donat|patreon|ko-fi|paypal|tip jar|buy me", v))
    ours = page[page.find('class="lp mb"'):]
    ours = re.split(r"<footer", ours, maxsplit=1)[0]
    ext = [h for h in re.findall(r'href="(https?://[^"]+)"', ours)
           if not re.match(r"https://(discord\.gg|discord\.com|(www\.)?reddit\.com|github\.com/orionopenwow/)", h)]
    check(f"{label}: outside links only Discord, Reddit and our own source", not ext, ext[:4])
    for sec in ("what", "shots", "android", "guides", "where", "trouble", "faq", "install", "home-screen", "first-run",
                "join-guide", "addons", "phone-test"):
        check(f"{label}: section #{sec}", f'id="{sec}"' in page)
    for words in ("Play in the browser", "Add it to your home screen", "Touch controls", "Play solo on this phone",
                  "Join a friend", "Paste invite link and join", "Leyline for Android (latest)", "Leyline-debug.apk",
                  "Install unknown apps", "Tap test", "Copy debug info", "A phone can't keep a realm up for friends",
                  "Open in Chrome", "Your addons"):
        check(f"{label}: says {words!r}", words in html_unescape(v), "")
    check(f"{label}: the Android download is our GitHub releases",
          'href="https://github.com/orionopenwow/Leyline/releases"' in page)
    check(f"{label}: no game data offered (no .mpq, no client download)", not re.search(r"(?i)\.mpq|client download|game files download", v))
    imgs = re.findall(r'<img[^>]+src="(/wrath/art/mobile/[^"]+)"', page)
    check(f"{label}: every screenshot shown, each one there", sorted(set(imgs)) == sorted(f"/wrath/art/mobile/{n}.webp" for n in SHOTS) and len(imgs) == len(set(imgs)) and all(wp.art_present(s[len("/wrath/art/"):]) for s in imgs), imgs)
    check(f"{label}: every screenshot has alt text and a size", all(re.search(r'alt="[^"]{12,}"[^>]*width="\d+" height="\d+"', t)
                                                                  for t in re.findall(r'<img[^>]+/wrath/art/mobile/[^>]+>', page)))
    check(f"{label}: Phone or computer? says what doesn't work too", page.count('class="mb-no"') >= 4 and page.count('class="mb-yes"') >= 4)
    check(f"{label}: owner-voice lines marked", "TODO(owner)" in page)   # in an HTML comment
    return page

import html as _html
html_unescape = _html.unescape

page = page_checks("Leyline look")
check("Leyline look: the site nav has the phone page", 'href="/wrath/mobile"' in page.split('class="lp mb"')[0])

# the screenshots themselves, through the art route; up with the game data killswitch (our own UI, no game data)
st, hd, body = get("/wrath/art/mobile/phone-play.webp")
check("art route serves a screenshot as WebP", st == 200 and hd.get("content-type") == "image/webp" and body[:4] == b"RIFF", (st, hd.get("content-type")))
st, _, _ = get("/wrath/art/mobile/nope.webp")
check("a missing screenshot is a 404", st == 404, st)
open(os.environ["WU_GAMEDATA_FLAG"], "w").close()
st, _, _ = get("/wrath/art/mobile/phone-touch.webp")
check("the killswitch leaves the phone screenshots up (no game data in them)", st == 200, st)
os.remove(os.environ["WU_GAMEDATA_FLAG"])

# a missing screenshot is a framed panel, never a broken picture
os.rename(os.path.join(ART, "mobile", "phone-touch.webp"), os.path.join(TMP, "touch.webp"))
st, _, raw = get("/wrath/mobile", ip="203.0.113.10")
p2 = raw.decode("utf-8")
check("a missing screenshot: its panel and caption, no img", 'data-slot="mobile/phone-touch.webp"' in p2
      and "/wrath/art/mobile/phone-touch.webp" not in p2 and "Picture coming soon" in p2)
os.rename(os.path.join(TMP, "touch.webp"), os.path.join(ART, "mobile", "phone-touch.webp"))

# the mobile repo, once the owner made it: the download and the source go there; anything not GitHub is ignored
os.environ["WU_MOBILE_REPO"] = "https://github.com/orionopenwow/Leyline-Mobile"
st, _, raw = get("/wrath/mobile", ip="203.0.113.11")
check("WU_MOBILE_REPO: releases and source on the mobile repo", b'href="https://github.com/orionopenwow/Leyline-Mobile/releases"' in raw
      and b'href="https://github.com/orionopenwow/Leyline-Mobile"' in raw)
os.environ["WU_MOBILE_REPO"] = "https://evil.example.com/x/y"
check("WU_MOBILE_REPO that isn't a GitHub repo is ignored", mp.mobile_repo() == "" and mp.releases_url().startswith("https://github.com/orionopenwow/Leyline/"))
os.environ.pop("WU_MOBILE_REPO")
check("the APK address is the android-latest release's file", mp.apk_url() == "https://github.com/orionopenwow/Leyline/releases/download/android-latest/Leyline-debug.apk")

# the hub's front door on: the same page in the hub's frame, with its menu item
os.environ["WU_HUB_FRONT"] = "1"
if wp.hub_framed():
    page = page_checks("hub frame")
    check("hub frame: the menu has On your phone", 'href="/wrath/mobile"' in page and "On your phone" in page)
else:
    check("hub frame: the hub isn't on in this checkout (skipped)", True)
os.environ.pop("WU_HUB_FRONT")

# the docs page beside it
st, _, raw = get("/wrath/docs/mobile", ip="203.0.113.12")
check("/wrath/docs/mobile: 200, links the showcase page", st == 200 and b'href="/wrath/mobile"' in raw, st)
check("/wrath/docs/mobile: no game brand names", not BANNED.findall(visible(raw.decode("utf-8")).replace(LEGAL, "")))

# the deploy: the signin step packs both modules and fastloop installs them and the screenshots
bat = open(os.path.join(STREAM, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
fl = open(os.path.join(STREAM, "fastloop.sh"), encoding="utf-8").read()
check("signin packs mobile_page.py and site_pages.py", 'set "LINKS=%LINKS% mobile_page.py"' in bat and 'set "LINKS=%LINKS% site_pages.py"' in bat
      and "site_pages.py mobile_page.py" in bat)
check("fastloop installs both, backs them up and puts them back on failure",
      fl.count("for f in site_pages.py mobile_page.py") == 3 and 'install -m 644 "$f" "$WU/portal/$f"' in fl)
check("fastloop installs the screenshots and checks the page", "www-files/art/mobile" in fl and "/wrath/mobile" in fl)

srv.shutdown()
shutil.rmtree(TMP, ignore_errors=True)
print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
