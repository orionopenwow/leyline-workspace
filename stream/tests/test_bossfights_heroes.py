"""Boss fight heroes on the website (stream/bossfights.py, stream/wrath_portal.py; docs/BOSS-FIGHT-HEROES.md): the hero
account rule (the same as the realm module's), the ?hero= pick, the console line, the realm's replies, Fight now's hero
path at /wrath/play against a fake realm (made, ready, not live yet, said no, down), the session seating the hero with no
character list (/__env.js "char"), Log in keeping the pick, the picker on /wrath/bossfights, the class and spec table
equal to the module's, and no brand words.

    python3 -I stream/tests/test_bossfights_heroes.py stream/wrath_portal.py
"""
import html, http.client, importlib.util, json, os, re, sys, tempfile, threading, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from xml.sax.saxutils import escape

HERE = os.path.dirname(os.path.abspath(sys.argv[1]))
REPO = os.path.dirname(HERE)
TMP = tempfile.mkdtemp(prefix="wu-bfh-test-")
OUT = os.path.join(TMP, "duels")
os.makedirs(OUT)


def free_port():
    import socket
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p


SOAP_PORT, REALM_PORT = free_port(), free_port()
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL=f"http://127.0.0.1:{SOAP_PORT}/",
                  PORTAL_URL=f"http://127.0.0.1:{REALM_PORT}", WU_STREAM="0", STREAM_HOST="wrath.example",
                  PUBLIC_HOST="play.example", WU_RELAY_LISTING="http://127.0.0.1:9/realms.json", WU_DUELS_DIR=OUT,
                  WU_ART_DIR=tempfile.mkdtemp(), WU_PORTAL_DATA=os.path.join(TMP, "data"), WU_HUB_FRONT="1")
os.environ.pop("WU_BF_HEROES", None)
spec = importlib.util.spec_from_file_location("wp", sys.argv[1])
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
wp.realm_status = lambda: {"online": True, "players": 3, "bots": 10}

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, str(extra)[:500])


BANNED = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich[\s_-]*king|blizzard|arthas")
BF = wp.bossfights
check("bossfights.py loaded", BF is not None)
if BF is None:
    print(f"\n{passed}/{passed + failed} passed"); sys.exit(1)
BF.store.root = BF.arena_store.root = OUT

# --- the hero account, the pick, the console line --------------------------------------------------------------------
check("hero account: <ACCOUNT>BF", BF.hero_account("wr000123") == "WR000123BF")
check("hero account: a long name, the module's hash (tests/test_bossfights_heroes.cpp has the same values)",
      BF.hero_account("ABCDEFGHIJKLMNO") == "BF0263AAD3AE34E1" and BF.hero_account("wr0001234567890x") == "BF96F76C9CEAE255")
check("hero account: never for a bad name", all(BF.hero_account(v) == "" for v in ("", "a b", "x;y", "A" * 17, None, 5)))
check("hero account: always a valid account name", all(BF.ACCOUNT_RE.fullmatch(BF.hero_account(v))
                                                        for v in ("A", "WR000123", "ABCDEFGHIJKLMNOP")))
good = ["mage-fire", "druid-cat", "deathknight-blood", "paladin-holy-horde", "warrior-prot-alliance"]
bad = ["mage-holy", "monk-fire", "mage", "mage-fire-elves", "Mage-fire", "mage-fire-horde-x", "x" * 40, "", None,
       "mage_fire", "../mage-fire"]
check("check_hero: real class and spec picks", all(BF.check_hero(v) == v for v in good))
check("check_hero: anything else refused", all(BF.check_hero(v) is None for v in bad), [v for v in bad if BF.check_hero(v)])
check("hero_param", BF.hero_param({"hero": ["mage-fire"]}) == "&hero=mage-fire" and BF.hero_param({"hero": ["bad"]}) == ""
      and BF.hero_param({}) == "")
check("hero_command: the go and the pick", BF.hero_command("wr000123", "bf:frozen-throne:heroic:group", "druid-bear-horde")
      == "bossfight hero go WR000123 frozen-throne heroic group druid bear horde")
check("hero_command: no pick keeps the realm's last one", BF.hero_command("WR1", "bf:frozen-throne:normal:bots")
      == "bossfight hero go WR1 frozen-throne normal bots")
check("hero_command: refuses a bad account, a bad go, the arena, a bad pick dropped",
      BF.hero_command("WR 1", "bf:frozen-throne:normal:bots") is None and BF.hero_command("WR1", "bf:onyxia:normal:bots") is None
      and BF.hero_command("WR1", "arena:1v1") is None
      and BF.hero_command("WR1", "bf:frozen-throne:normal:bots", "mage-holy") == "bossfight hero go WR1 frozen-throne normal bots")

# --- the realm's replies ------------------------------------------------------------------------------------------------
r = BF.parse_hero_reply("Some line\r\nbfhero ok Kalinra mage fire frost made WR1BF\r\n")
check("reply ok", r == {"ok": True, "name": "Kalinra", "class": "mage", "spec": "fire", "spec2": "frost", "made": True, "account": "WR1BF"}, r)
check("reply no: plain words kept, nothing else", BF.parse_hero_reply("bfhero no you have 10 heroes: delete one <b>") ==
      {"ok": False, "why": "you have 10 heroes: delete one b"})
check("reply: nothing usable", not BF.parse_hero_reply("done")["ok"] and not BF.parse_hero_reply(None)["ok"]
      and not BF.parse_hero_reply("bfhero ok kalinra mage fire frost made WR1BF")["ok"])
check("reply: a brand-word name is never shown", not BF.parse_hero_reply("bfhero ok Blizzard mage fire frost made WR1BF")["ok"])
lst = BF.parse_hero_list("bfhero list 2 WR1BF\nbfhero hero Kalinra mage fire frost current\nbfhero hero Torvin warrior prot fury -\nbfhero hero Bad<x> mage fire frost -")
check("hero list", [h["name"] for h in lst] == ["Kalinra", "Torvin"] and lst[0]["current"] and not lst[1]["current"], lst)


class Fault(Exception):
    def __init__(self, kind, detail=""):
        self.kind, self.detail = kind, detail


calls = []
def fake(text=None, exc=None):
    def soap(cmd):
        calls.append(cmd)
        if exc:
            raise exc
        return text
    return soap


check("hero_play: ok", BF.hero_play(fake("bfhero ok Kalinra mage fire frost ready WR1BF"), "WR1", "bf:frozen-throne:normal:bots")["state"] == "ok"
      and calls[-1] == "bossfight hero go WR1 frozen-throne normal bots")
check("hero_play: no such command = boss fights not live yet",
      BF.hero_play(fake(exc=Fault("fault", "There is no such command.")), "WR1", "bf:frozen-throne:normal:bots")["state"] == "not-live")
check("hero_play: a module without heroes (no such subcommand) = not live",
      BF.hero_play(fake(exc=Fault("fault", "There is no such subcommand")), "WR1", "bf:frozen-throne:normal:bots")["state"] == "not-live")
check("hero_play: the realm down", BF.hero_play(fake(exc=Fault("down", "x")), "WR1", "bf:frozen-throne:normal:bots")["state"] == "down")
check("hero_play: the realm said no", BF.hero_play(fake("bfhero no boss fights are off on this realm"), "WR1",
                                                   "bf:frozen-throne:normal:bots") == {"state": "no", "why": "boss fights are off on this realm"})
check("hero_play: another account's hero is refused",
      BF.hero_play(fake("bfhero ok Kalinra mage fire frost ready WR2BF"), "WR1", "bf:frozen-throne:normal:bots")["state"] == "no")
n = len(calls)
check("hero_play: nothing sent for the arena or a bad go", BF.hero_play(fake("x"), "WR1", "arena:1v1")["state"] == "bad"
      and len(calls) == n)

# --- the portal: hero_go, Log in, the session ------------------------------------------------------------------------------
check("hero_go: a boss fight", wp.hero_go({"go": ["bf:frozen-throne:heroic:bots"]}) == "bf:frozen-throne:heroic:bots")
check("hero_go: the launcher's mode id", wp.hero_go({"go": ["mode:frozen-throne"]}) == "bf:frozen-throne:normal:bots")
check("hero_go: not the arena, not a place", wp.hero_go({"go": ["arena:1v1"]}) is None and wp.hero_go({"go": ["q:deadmines"]}) is None)
os.environ["WU_BF_HEROES"] = "0"
check("hero_go: heroes off", wp.hero_go({"go": ["bf:frozen-throne:heroic:bots"]}) is None and BF.heroes_html("bf:frozen-throne:normal:bots", True, []) == "")
del os.environ["WU_BF_HEROES"]
nxt = urllib.parse.parse_qs(urllib.parse.urlsplit(wp.go_signin({"go": ["bf:frozen-throne:normal:bots"], "hero": ["druid-cat"]})).query)["next"]
check("go_signin: keeps the hero pick through Log in", nxt == ["/wrath?go=bf:frozen-throne:normal:bots&hero=druid-cat"], nxt)
check("go_signin: a bad pick is dropped, a go without a boss fight keeps none",
      "hero" not in wp.go_signin({"go": ["bf:frozen-throne:normal:bots"], "hero": ["mage-holy"]})
      and "hero" not in wp.go_signin({"go": ["arena:1v1"], "hero": ["mage-fire"]}))
code = wp.sessions.create("WR000123BF", "pw", "mini", 3, char="Kalinra")
sid = wp.sessions.redeem(code)
env = wp.env_js(wp.sessions.get(sid)).decode()
check("env.js: the hero is seated (char), marked as a boss fight", '"char":"Kalinra"' in env and '"bf":"1"' in env)
code = wp.sessions.create("WR000123", "pw", "mini", 3, char="<script>")
env = wp.env_js(wp.sessions.get(wp.sessions.redeem(code))).decode()
check("env.js: no char for an odd name, none for a normal session", '"char"' not in env)
page = html.unescape(wp.bf_not_live_page().decode()).replace("not affiliated with or endorsed by Blizzard Entertainment", " ")   # the legal line in the footer: the one allowed mention
check("not-live page: plain words, nothing about brands or versions", "aren't open on the realm yet" in page
      and not BANNED.search(re.sub(r"<[^>]+>", " ", page)) and "3.3" not in re.sub(r"<[^>]+>", " ", page))

# --- hero_login against a fake realm ------------------------------------------------------------------------------------------
realm = {"mode": "ok", "commands": []}


class Soap(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_POST(self):
        cmd = wp._tag_text(self.rfile.read(int(self.headers["Content-Length"])), "command")
        realm["commands"].append(cmd)
        ok, text = True, "done"
        if cmd.startswith("account create"):
            ok, text = False, "Account with this name already exist!"
        elif cmd.startswith("bossfight hero go"):
            if realm["mode"] == "not-live":
                ok, text = False, "There is no such command."
            elif realm["mode"] == "no":
                text = "bfhero no you have 10 heroes: delete one first (.bf hero delete <name>)"
            else:
                text = "bfhero ok Kalinra mage fire frost made " + cmd.split()[3] + "BF"
        elif cmd.startswith("bossfight hero list"):
            text = "bfhero list 1 WR000123BF\r\nbfhero hero Kalinra mage fire frost current"
        inner = (f"<ns1:executeCommandResponse><result>{escape(text)}</result></ns1:executeCommandResponse>" if ok else
                 f"<SOAP-ENV:Fault><faultcode>SOAP-ENV:Client</faultcode><faultstring>{escape(text)}</faultstring></SOAP-ENV:Fault>")
        body = ('<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" '
                f'xmlns:ns1="urn:AC"><SOAP-ENV:Body>{inner}</SOAP-ENV:Body></SOAP-ENV:Envelope>').encode()
        self.send_response(200 if ok else 500)
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)


class Realm(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_GET(self):
        if "wr_session=good" not in self.headers.get("Cookie", ""):
            self.send_response(401); self.send_header("Content-Length", "0"); self.end_headers(); return
        doc = {"/api/me": {"display_name": "Tester"}, "/api/play": {"user": "WR000123", "pass": "Pa55word1234abcd"}}[self.path]
        body = json.dumps(doc).encode()
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)


page_srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
for srv in (ThreadingHTTPServer(("127.0.0.1", SOAP_PORT), Soap), ThreadingHTTPServer(("127.0.0.1", REALM_PORT), Realm), page_srv):
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()


class FakeHandler:
    def __init__(self):
        self.sent = []

    def send(self, status, body, *a, **k):
        self.sent.append((status, html.unescape(body.decode("utf-8", "replace")) if isinstance(body, bytes) else str(body)))


def login(query, mode="ok"):
    realm["mode"] = mode
    h = FakeHandler()
    out = wp.PageHandler.hero_login(h, "WR000123", "Pa55word1234abcd", query)
    return out, h.sent


out, sent = login({"go": ["bf:frozen-throne:heroic:bots"], "hero": ["mage-fire"]})
check("Fight now: the hero account, then the hero, then play it", out == ("WR000123BF", "Kalinra") and not sent, (out, sent))
check("Fight now: the hero account gets the player's own game password",
      "account set password WR000123BF Pa55word1234abcd Pa55word1234abcd" in realm["commands"], realm["commands"][-4:])
check("Fight now: the realm is asked for the hero with the pick",
      realm["commands"][-1] == "bossfight hero go WR000123 frozen-throne heroic bots mage fire", realm["commands"][-1])
out, sent = login({"go": ["bf:frozen-throne:normal:bots"]}, "not-live")
check("Fight now on a realm without boss fights: the plain-words page, no session",
      out is None and sent and sent[0][0] == 200 and "aren't open on the realm yet" in sent[0][1], sent[:1])
out, sent = login({"go": ["bf:frozen-throne:normal:bots"]}, "no")
check("Fight now, the realm says no: its words on the page", out is None and sent and "delete one first" in sent[0][1], sent[:1])
n = len(realm["commands"])
out, sent = login({"go": ["arena:1v1"]})
check("not a boss fight: the player's own account, nothing asked", out == ("WR000123", "") and len(realm["commands"]) == n)
out, sent = login({"go": ["q:deadmines"]})
check("a share link: the player's own account", out == ("WR000123", ""))


def get(path, cookie="", ip="10.9.0.9"):
    c = http.client.HTTPConnection("127.0.0.1", page_srv.server_address[1], timeout=20)
    hd = {"X-Forwarded-For": ip}
    if cookie:
        hd["Cookie"] = cookie
    c.request("GET", path, headers=hd)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read().decode("utf-8", "replace")


# --- the picker on /wrath/bossfights ----------------------------------------------------------------------------------
title, body = BF.boss_page({}, False)
links = re.findall(r'href="([^"]+)"', body)
# the widget (owner 5 Nov: "a character creation widget ... instead of ugly switches"): one GET form to /wrath
check("picker: one form to /wrath carrying the go", re.search(r'<form class="bfw" method="get" action="/wrath"', body)
      and 'name="go" value="bf:frozen-throne:normal:bots"' in body)
check("picker: a tile for every class", all(f'name="cls" value="{c}"' in body for c in BF.HERO_CLASSES))
check("picker: every class and spec is a choice", all(f'name="spec-{c}" value="{k}"' in body
                                                       for c, (_, specs) in BF.HERO_CLASSES.items() for k, _, _ in specs))
check("picker: each class starts on one spec of its own (switching class never sends another class's spec)",
      all(len(re.findall(rf'name="spec-{c}" value="[a-z]+" id="[^"]+" class="bfw-r" checked', body)) == 1 for c in BF.HERO_CLASSES))
check("picker: Frost Mage chosen first", 'name="cls" value="mage" id="bfw-c-mage" class="bfw-r" checked' in body
      and 'name="spec-mage" value="frost" id="bfw-s-mage-frost" class="bfw-r" checked' in body)
check("picker: no script (the page has no nonce)", "<script" not in body)
check("hero_pick: the form's class + that class's spec", BF.hero_pick({"cls": ["warrior"], "spec-warrior": ["prot"], "spec-mage": ["fire"]}) == "warrior-prot")
check("hero_pick: ?hero= still wins; junk refused", BF.hero_pick({"hero": ["druid-cat"], "cls": ["mage"]}) == "druid-cat"
      and BF.hero_pick({"cls": ["mage"], "spec-mage": ["holy"]}) is None and BF.hero_pick({"cls": ["x"], "spec-x": ["y"]}) is None)
check("hero_param: the form's pick becomes &hero=", BF.hero_param({"cls": ["paladin"], "spec-paladin": ["holy"]}) == "&hero=paladin-holy")
title, body = BF.boss_page({"d": ["heroic"]}, True, heroes=[{"name": "Kalinra", "class": "mage", "spec": "fire", "spec2": "frost", "current": True}])
check("picker signed in: the form on the chosen difficulty", 'name="go" value="bf:frozen-throne:heroic:bots"' in body)
check("picker: the current hero's class and spec chosen first", 'value="mage" id="bfw-c-mage" class="bfw-r" checked' in body
      and 'name="spec-mage" value="fire" id="bfw-s-mage-fire" class="bfw-r" checked' in body)
check("picker: the player's own heroes are one-tap links", "/wrath?go=bf:frozen-throne:heroic:bots&amp;hero=mage-fire" in body)
check("picker: the player's heroes, the current one marked", "Kalinra" in body and "Fire Mage" in body and "bf-hero-me on" in body)
text = re.sub(r"<[^>]+>", " ", body)
check("picker: no brand words, no client versions", not BANNED.search(text) and not re.search(r"\b3\.3\.5|\b1\.12\b", text))
check("picker: tap targets are at least 44 px", "min-height:44px" in BF.CSS)
st, hd, b = get("/wrath/bossfights", cookie="wr_session=good")
check("/wrath/bossfights signed in: the realm's list of heroes shows (via the game account)", st == 200 and "Kalinra" in b
      and "bossfight hero list WR000123" in realm["commands"], (st, realm["commands"][-2:]))

# --- the table equals the realm module's ------------------------------------------------------------------------------
logic = open(os.path.join(REPO, "realm", "custom", "modules", "mod-leyline-bossfights", "src", "BossfightsHeroLogic.h"), encoding="utf-8").read()
mod = {}
for cls, key in re.findall(r'\{ (\d+), "([a-z]+)", "[^"]+", "[a-z]+", \d, ROLE_', logic):
    mod.setdefault(BF.CLASS_WORD[int(cls)], []).append(key)
site = {c: [k for k, _, _ in specs] for c, (_, specs) in BF.HERO_CLASSES.items()}
check("the class and spec table equals the module's (BossfightsHeroLogic.h Specs)", mod == site, (mod, site))

js = open(os.path.join(HERE, "www-files", "wrath-page.js"), encoding="utf-8").read()
check("wrath-page.js: a hero pass keeps the sign-in cover up past the character list",
      "export function heroPass" in js and "env.bf === '1'" in js and "guestPass() || hero" in js
      and "if (guest && name === 'charselect') return;" in js)

import subprocess
gen = os.path.join(REPO, "realm", "custom", "modules", "mod-leyline-bossfights", "tools", "gen_hero_data.py")
res = subprocess.run([sys.executable, "-I", gen, "--check"], capture_output=True, text=True, timeout=60)
check("BossfightsHeroData.h matches ov_kits.lua and ov_main.lua (gen_hero_data.py --check)", res.returncode == 0, res.stdout + res.stderr)

print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
