"""Live now and New this week on the site's front page (stream/hub.py live_strip, the data from stream/links.py
live_now / new_this_week): the busiest player realms by players then uptime, each with its mode badge and Play; new
realms with a player in the last day; plain-word empty states; nothing without a realm list. And the realm share
link page under the real handler (GET /wrath/r/<short>).

    python3 -I stream/tests/test_livenow.py stream/wrath_portal.py
    SHOTS=<dir> ...   also writes the pages (front strip, realm link page) as HTML there for screenshots
"""
import http.client, importlib.util, os, re, sys, tempfile, threading, time
from http.server import ThreadingHTTPServer

os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example",
                  WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json", WU_PORTAL_DATA=tempfile.mkdtemp(),
                  WU_MODS_DIR=tempfile.mkdtemp(), WU_WORKSHOP_ITEMS_DIR=tempfile.mkdtemp())
os.environ.pop("WU_HUB_FRONT", None)
spec = importlib.util.spec_from_file_location("wp", sys.argv[1])
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, extra)

HB, L = wp.hub_mod, wp.links
check("hub and links loaded", HB is not None and L is not None)
if HB is None or L is None:
    print(f"\n{passed}/{passed + failed} passed"); sys.exit(1)

NOW = time.time()
iso = lambda t: time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(t))
ROWS = [
    {"home": True, "official": True, "name": HB.HANGOUT_NAME, "live": True, "players": 40, "id": "homehomehomehome"},
    {"id": "frostpeakaaaaaaa", "name": "Frost Peak", "ruleset": "vanillaplus", "live": True, "players": 12, "up7": 99,
     "build": 12340, "first_seen": iso(NOW - 30 * 86400)},
    {"id": "oldlandsbbbbbbbb", "name": "Old Lands", "ruleset": "main", "live": True, "players": 12, "up7": 60, "build": 5875,
     "first_seen": iso(NOW - 2 * 86400)},
    {"id": "quietccccccccccc", "name": "Quiet <One>", "ruleset": "brawl", "live": True, "players": 0, "up7": 100,
     "first_seen": iso(NOW - 40 * 86400)},
    {"id": "gonedddddddddddd", "name": "Gone Realm", "live": False, "players": 0, "first_seen": iso(NOW - 86400)},
]
got = (ROWS, HB.hub_totals(ROWS), True, {}, {})
s = HB.hub.live_strip(got)
check("strip: both halves", "<h2>Live now</h2>" in s and "<h2>New this week</h2>" in s)
live = s.split("<h2>New this week</h2>")[0]
names = re.findall(r'class="hb-lv-n" href="[^"]*">([^<]*)<', live)
check("live now: player realms only, by players then uptime", names == ["Frost Peak", "Old Lands", "Quiet &lt;One&gt;"], names)
check("live now: mode badges", '<span class="hb-bdg">Vanilla+</span>' in live and "Brawl: everyone 80" in live)
check("live now: players, Online when none", "12 playing" in live and ">Online<" in live)
check("live now: Play goes in (browser hop, or the realm's link page for its own client)",
      'href="/wrath?go=r:frostpeakaaaaaaa">Play<' in live and 'href="/wrath/r/oldlands">Play<' in live)
check("live now: the name opens the realm's short link", 'href="/wrath/r/frostpea">Frost Peak<' in live)
new = s.split("<h2>New this week</h2>")[1]
check("new this week: first seen this week with a player now", re.findall(r'class="hb-lv-n" href="[^"]*">([^<]*)<', new) == ["Old Lands"], new)
empty = HB.hub.live_strip(([ROWS[0], ROWS[4]], {}, True, {}, {}))
check("empty states in plain words", "No player realms are up right now." in empty and "No new realms with players this week yet." in empty)
check("no realm list: no strip", HB.hub.live_strip(None) == "" and HB.hub.live_strip(False) == "")
check("no brand words, no version numbers in the strip",
      not re.search(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard|\d\.\d", re.sub(r"<style>.*?</style>", "", s + empty)))

real = HB.hub.realm_rows
HB.hub.realm_rows = lambda query=None: got
front = HB.hub.front_page().decode()
check("front page carries the strip after the official realm", 'id="live"' in front
      and front.index('id="hangout"') < front.index('id="live"'))
HB.hub.realm_rows = real

# the realm link page under the real handler
wp.community_realms = lambda: ([r for r in ROWS if not r.get("home")], True)
srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
threading.Thread(target=srv.serve_forever, daemon=True).start()
def get(path):
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=5)
    c.request("GET", path, headers={"X-Forwarded-For": "203.0.113.7"}); r = c.getresponse()
    return r.status, r.read().decode()
st, page = get("/wrath/r/frostpea")
check("GET /wrath/r/<short>: the card", st == 200 and "<h1>Frost Peak</h1>" in page and 'property="og:title" content="Frost Peak · Leyline realm"' in page)
check("GET: OG url is the short link", 'property="og:url" content="https://play.example/wrath/r/frostpea"' in page)
check("GET: both buttons", "Play in your browser</a>" in page and 'href="leyline://realm/frostpeakaaaaaaa"' in page and "Get the launcher" in page)
srv.shutdown()

shots = os.environ.get("SHOTS")
if shots:
    os.makedirs(shots, exist_ok=True)
    HB.hub.realm_rows = lambda query=None: got
    open(os.path.join(shots, "site-front.html"), "w").write(HB.hub.front_page().decode())
    HB.hub.realm_rows = lambda query=None: ([ROWS[0], ROWS[4]], HB.hub_totals([ROWS[0], ROWS[4]]), True, {}, {})
    open(os.path.join(shots, "site-front-empty.html"), "w").write(HB.hub.front_page().decode())
    HB.hub.realm_rows = real
    open(os.path.join(shots, "site-realm.html"), "w").write(page)

print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
