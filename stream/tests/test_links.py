"""The share links (stream/links.py, hooked into GET /r /h /q /m /u of wrath_portal.py): link cards,
the /wrath?go= deep link, strict path checks, escaping, the generic card, the rate limit, no game-brand names.

    python3 -I stream/tests/test_links.py stream/wrath_portal.py
"""
import http.client, importlib.util, os, re, sys, threading
from http.server import ThreadingHTTPServer

os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example",
                  WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json")
spec = importlib.util.spec_from_file_location("wp", sys.argv[1])
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
wp.community_realms = lambda: ([{"id": "abcdefghijklmnop", "name": "Frost <Fun> Realm", "tagline": "Bring a \"coat\""}], True)

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, extra)

L = wp.links
check("links.py loaded beside the portal", L is not None)
if L is None:
    print(f"\n{passed}/{passed + failed} passed"); sys.exit(1)

def page(path):
    return L.share_page(path, wp.card_meta, wp.render, wp.REALM_NAME, wp.community_realms).decode()

def meta(p, key):
    m = re.search(r'(?:property|name)="' + re.escape(key) + r'" content="([^"]*)"', p)
    return m.group(1) if m else None

# parse: the allow-lists
good = {"/r/wrathunleashed": ("r", "wrathunleashed"), "/r/abcdefghijklmnop": ("r", "abcdefghijklmnop"),
        "/h/krasus-landing": ("h", "krasus-landing"), "/q/258": ("q", "258"), "/q/heroic-random": ("q", "heroic-random"),
        "/m/better-bags": ("m", "better-bags"), "/u/Arthasx": ("u", "Arthasx"), "/wrath/q/258": ("q", "258")}
for p, want in good.items():
    check(f"parse {p}", L.parse(p) == want, L.parse(p))
bad = ["/r", "/r/", "/r/UPPER", "/r/-lead", "/r/trail-", "/r/" + "a" * 33, "/h/a b", "/h/a%20b", "/q/<x>",
       "/q/258;x", "/m/ab", "/m/1mod", "/m/a--b", "/m/" + "a" * 33, "/u/A", "/u/" + "A" * 13, "/u/Name1",
       "/u/N%C3%A9", "/u/../etc", "/x/thing", "/r/a/b", "/q/" + "1" * 90, "/m/a.b", "/u/Ab\nC", "/u/Abc\n", "/q/258\n", "/r\n"]
for p in bad:
    check(f"refuse {p!r}", L.parse(p) is None, L.parse(p))
check("match owns bad links too", all(L.match(p) for p in ("/r", "/q/<x>", "/u/" + "A" * 13)))
check("match leaves other paths", not any(L.match(p) for p in ("/wrath", "/wrath/realms", "/x/y", "/rr/x", "/")))

# deep links
check("deep link q", L.deep_link("q", "258") == "/wrath?go=q:258")
check("deep link u", L.deep_link("u", "Arthasx") == "/wrath?go=u:Arthasx")
check("check_go round trip", L.check_go("h:krasus-landing") == ("h", "krasus-landing"))
check("check_go refuses junk", all(L.check_go(v) is None for v in ("q:<x>", "z:1", "q", "", None, "q:" + "1" * 50, "u:a/b")))

# pages
q = page("/q/258")
check("queue page renders", q.startswith("<!doctype html>"))
check("card url", meta(q, "og:url") == "https://play.example/q/258", meta(q, "og:url"))
check("card image 1200x630", meta(q, "og:image") == "https://play.example/wrath/art/wu-card.webp"
      and meta(q, "og:image:width") == "1200" and meta(q, "og:image:height") == "630")
check("twitter large card", meta(q, "twitter:card") == "summary_large_image" and meta(q, "twitter:image"))
check("card title", "258" in (meta(q, "og:title") or "") and meta(q, "og:title") == meta(q, "twitter:title"))
check("own description", meta(q, "og:description") and meta(q, "og:description") != wp.CARD_DESCRIPTION
      and meta(q, "og:description") == meta(q, "twitter:description") == meta(q, "description"))
check("refresh to deep link", 'http-equiv="refresh" content="1;url=/wrath?go=q:258"' in q)
check("button to deep link", 'href="/wrath?go=q:258"' in q)
check("tab title", "<title>" in q and "258" in q.split("<title>")[1].split("</title>")[0])

r = page("/r/abcdefghijklmnop")
check("community realm name, escaped", "Frost &lt;Fun&gt; Realm" in r and "<Fun>" not in r)
check("community tagline, escaped", "Bring a &quot;coat&quot;" in (meta(r, "og:description") or ""), meta(r, "og:description"))
o = page("/r/" + wp.REALM_NAME.lower())
check("official realm card", "official" in (meta(o, "og:description") or "").lower())
check("unknown realm still a card", meta(page("/r/nobody-here"), "og:title") and "go=r:nobody-here" in page("/r/nobody-here"))
check("wrath-prefixed canonical", meta(page("/wrath/u/Arthasx"), "og:url") == "https://play.example/wrath/u/Arthasx")
check("hangout page", "Krasus Landing" in page("/h/krasus-landing") and "go=h:krasus-landing" in page("/h/krasus-landing"))
check("mod page", "Better Bags" in page("/m/better-bags"))
check("player page", "Arthasx" in page("/u/Arthasx"))

for p in ("/q/<script>", "/r", "/u/" + "A" * 40, "/m/%3Cb%3E"):
    g = page(p)
    check(f"generic card for {p!r}", meta(g, "og:url") == "https://play.example/wrath" and 'url=/wrath"' in g
          and "<script>" not in g.lower().replace('<script nonce', '') and "go=" not in g, g[:120])

broken = L.share_page("/r/abcdefghijklmnop", wp.card_meta, wp.render, wp.REALM_NAME, lambda: 1 / 0).decode()
check("broken realm list still a card", meta(broken, "og:title") and "go=r:abcdefghijklmnop" in broken)

# --- realm links (owner 31 Oct): one short link per realm, the realm's card with two buttons, no refresh
import time as _time
NOW = _time.time()
def iso(t): return _time.strftime("%Y-%m-%dT%H:%M:%SZ", _time.gmtime(t))
REALMS = [
    {"id": "frostpeakaaaaaaa", "name": "Frost Peak", "tagline": "Cold and kind", "type": "PvP", "ruleset": "vanillaplus",
     "live": True, "players": 12, "up7": 99, "rules": ["Be nice", "No <script> please"], "build": 12340,
     "mods": [{"id": "mod:better-bags"}, {"id": "mode:minigolf"}], "first_seen": iso(NOW - 30 * 86400)},
    {"id": "oldlandsbbbbbbbb", "name": "Old Lands", "ruleset": "main", "live": True, "players": 12, "up7": 60, "build": 5875,
     "first_seen": iso(NOW - 2 * 86400)},
    {"id": "quietccccccccccc", "name": "Quiet One", "ruleset": "nope", "live": True, "players": 0, "up7": 100,
     "first_seen": iso(NOW - 3 * 86400)},
    {"id": "gonedddddddddddd", "name": "Gone Realm", "live": False, "players": 0, "first_seen": iso(NOW - 1 * 86400)},
    {"id": "wardeneeeeeeeeee", "name": "Warden Realm", "live": True, "players": 2, "warden": True, "first_seen": iso(NOW - 9 * 86400)},
    {"id": "homeffffffffffff", "name": "Home", "live": True, "players": 50, "hangout": True},
]
rl = lambda: (REALMS, True)
class FakeMods:
    @staticmethod
    def mod(i): return {"name": "Better Bags"} if i == "better-bags" else None
def rpage(path, realms=rl):
    return L.share_page(path, wp.card_meta, wp.render, wp.REALM_NAME, realms, mods=FakeMods.mod,
                        labels=wp.RULESET_LABELS, host="play.example", get_launcher="https://dl.example/get").decode()
check("short slug: first 8 letters", L.short_of("frostpeakaaaaaaa") == "frostpea" and L.share_path("frostpeakaaaaaaa") == "/wrath/r/frostpea")
check("find by short, by full id", L.find_realm(REALMS, "frostpea")["name"] == "Frost Peak"
      and L.find_realm(REALMS, "frostpeakaaaaaaa")["name"] == "Frost Peak")
check("find refuses too short, ambiguous, unknown", L.find_realm(REALMS, "fro") is None and L.find_realm(REALMS, "zzzzzzzz") is None
      and L.find_realm(REALMS + [{"id": "frostpeazzzzzzzz"}], "frostpea") is None)
f = rpage("/r/frostpea")
check("realm card: name and tab title", "<h1>Frost Peak</h1>" in f and "<title>Frost Peak · Leyline realm</title>" in f)
check("realm card: OG title, url, image", meta(f, "og:title") == "Frost Peak · Leyline realm"
      and meta(f, "og:url") == "https://play.example/wrath/r/frostpea" and meta(f, "og:image") and meta(f, "twitter:card"))
check("realm card: OG description says mode and players", meta(f, "og:description").startswith("Vanilla+ · 12 playing now.")
      and meta(f, "og:description") == meta(f, "twitter:description"), meta(f, "og:description"))
check("realm card: mode badge, players", 'rc-mode">Vanilla+<' in f and "12 playing" in f)
check("realm card: mods by name", "Better Bags, Minigolf" in f)
check("realm card: rules, escaped", "<li>Be nice</li>" in f and "&lt;script&gt;" in f and "No <script>" not in f)
check("realm card: Play in your browser hop", 'href="/wrath?go=r:frostpeakaaaaaaa">Play in your browser' in f)
check("realm card: Open in Leyline + Get the launcher", 'href="leyline://realm/frostpeakaaaaaaa">Open in Leyline' in f
      and 'href="https://dl.example/get"' in f and "Get the launcher" in f)
check("realm card: no refresh (the visitor picks)", 'http-equiv="refresh"' not in f)
check("realm card: the short link to share", "rc-copy\">https://play.example/wrath/r/frostpea<" in f)
check("same page under /wrath/r/ and the full id", meta(rpage("/wrath/r/frostpea"), "og:title") == meta(rpage("/r/frostpeakaaaaaaa"), "og:title"))
w = rpage("/r/wardenee")
check("warden realm: no browser button, says why", "go=r:wardeneeeeeeeeee" not in w and "Open in Leyline" in w and "your own game" in w)
check("other client version: no browser button", "go=r:oldlandsbbbbbbbb" not in rpage("/r/oldlands"))
off = rpage("/r/official")
check("official realm link: both buttons", 'href="/wrath">Play in your browser' in off and "leyline://realm/official" in off)
unk = rpage("/r/zzzzzzzz")
check("unknown realm: still a card with both buttons", meta(unk, "og:title") and "go=r:zzzzzzzz" in unk and "Open in Leyline" in unk
      and "isn't on the list" in unk)
check("broken realm list: a card", "Open in Leyline" in rpage("/r/frostpea", realms=lambda: 1 / 0))
check("realm pages: no brand words", not re.findall(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard", f + w + off + unk))

# live now and new this week
ln = [r["name"] for r in L.live_now(REALMS)]
check("live now: community, live, by players then uptime", ln == ["Frost Peak", "Old Lands", "Warden Realm", "Quiet One"], ln)
L.LAST_PLAYER.clear()
nw = [r["name"] for r in L.new_this_week(REALMS, now=NOW)]
check("new this week: first seen < 7 d and a player in the last day", nw == ["Old Lands"], nw)
L.note_players([{"id": "quietccccccccccc", "live": True, "players": 1}], now=NOW - 3600)
nw = [r["name"] for r in L.new_this_week(REALMS, now=NOW)]
check("new this week: a player an hour ago counts", nw == ["Old Lands", "Quiet One"], nw)
check("new this week: a player two days ago doesn't", [r["name"] for r in L.new_this_week(
    [dict(r, live=False) for r in REALMS], now=NOW + 2 * 86400)] == [])
import tempfile
tmpd = tempfile.mkdtemp()
L.load_last(os.path.join(tmpd, "last.json")); L._last_saved[0] = 0
L.note_players([{"id": "gonedddddddddddd", "live": True, "players": 3}], now=NOW)
L.LAST_PLAYER.clear(); L.load_last(os.path.join(tmpd, "last.json"))
check("last-player times survive a restart", L.LAST_PLAYER.get("gonedddddddddddd") == NOW, L.LAST_PLAYER)
L.LAST_FILE = ""; L.LAST_PLAYER.clear()
it = L.live_item(REALMS[1], wp.RULESET_LABELS)
check("live item: own-client realm plays via its link page", it["play"] == "/wrath/r/oldlands" and it["mode"] == "Level 1 or boost to 80"
      and it["players"] == 12 and it["short"] == "oldlands")
check("live item: browser realm plays via the hop", L.live_item(REALMS[0])["play"] == "/wrath?go=r:frostpeakaaaaaaa")

def visible(p):
    return re.sub(r"<[^>]+>", " ", re.sub(r"<style.*?</style>", " ", p, flags=re.S))
allp = "".join(page(p) for p in list(good) + bad[:5])
brand = re.findall(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard", visible(allp) + " ".join(re.findall(r'content="([^"]*)"', allp)))
check("no game-brand names", not brand, brand)
check("no old brand names", not re.findall(r"(?i)open ?vanilla|openwow", visible(allp)))
check("says Leyline", "Leyline" in allp)

# through the real handler: routing, headers, rate limit
srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
threading.Thread(target=srv.serve_forever, daemon=True).start()
def get(path, ip="203.0.113.5"):
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=5)
    c.request("GET", path, headers={"X-Forwarded-For": ip}); res = c.getresponse()
    return res.status, dict(res.getheaders()), res.read().decode()
st, hd, body = get("/q/258")
check("GET /q/258 is 200", st == 200, st)
check("html, no-store, CSP", hd.get("Content-Type", "").startswith("text/html") and "default-src 'none'" in hd.get("Content-Security-Policy", ""))
check("GET bad link is 200 card", get("/q/%3Cscript%3E")[0] == 200 and "Traceback" not in get("/u/!!!")[2])
check("GET /wrath/m/better-bags", get("/wrath/m/better-bags")[0] == 200)
wp.community_realms = rl
st, hd, body = get("/wrath/r/frostpea")
check("GET /wrath/r/<short> is the realm card", st == 200 and "<h1>Frost Peak</h1>" in body and "leyline://realm/frostpeakaaaaaaa" in body)
check("GET /r/<short> too", "<h1>Frost Peak</h1>" in get("/r/frostpea")[2])
check("other paths unchanged", get("/nope")[0] == 404)
codes = [get("/h/x", ip="198.51.100.9")[0] for _ in range(605)]
check("rate limit like /wrath/leyline (600/min)", codes.count(200) == 600 and codes[-1] == 429, (codes.count(200), codes[-1]))
srv.shutdown()

# --- the deep link survives sign-in: /wrath -> /wrath/play -> /__gate -> the game page
check("go_param: a good link is carried", wp.go_param({"go": ["q:258"]}) == "&go=q:258", wp.go_param({"go": ["q:258"]}))
check("go_param: a bad link is dropped", wp.go_param({"go": ["q:<x>"]}) == "" and wp.go_param({"go": ["zz:1"]}) == "")
check("go_param: nothing given, nothing carried", wp.go_param({}) == "")
check("go_param: over-long dropped", wp.go_param({"go": ["u:" + "a" * 60]}) == "")

print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
