"""Boss fights and the 1v1 arena queue on the website (stream/bossfights.py) and their hooks in wrath_portal.py: the go
kinds (bf:<boss>:<difficulty>:<mode>, arena:1v1) checked, sent as "ovgo set" for the signed-in game account against a
fake SOAP, carried to the stream site and kept through Log in; bossfights.json read with names only (companions counted,
never named), the time worked out from ms, a missing or odd file as empty boards; /wrath/bossfights, /wrath/arena, the
JSON, the Leaderboards tabs, the nav, /wrath/modes; no brand words; the deploy wiring; the slice covers the fight.

    python3 -I stream/tests/test_bossfights.py stream/wrath_portal.py
"""
import http.client, importlib.util, json, math, os, re, sys, tempfile, threading, time, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from xml.sax.saxutils import escape

HERE = os.path.dirname(os.path.abspath(sys.argv[1]))
REPO = os.path.dirname(HERE)
TMP = tempfile.mkdtemp(prefix="wu-bf-test-")
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
check("bossfights.py loaded beside the portal", BF is not None)
if BF is None:
    print(f"\n{passed}/{passed + failed} passed"); sys.exit(1)
BF.STAT_EVERY = 0
BF.store.root = BF.arena_store.root = OUT

# --- the go kinds -------------------------------------------------------------------------------------------------
good = ["bf:frozen-throne:normal:bots", "bf:frozen-throne:normal:group", "bf:frozen-throne:heroic:group-only", "arena:1v1"]
bad = ["bf:frozen-throne", "bf:frozen-throne:mythic:bots", "bf:frozen-throne:normal:solo", "bf:onyxia:normal:bots",
       "bf:Frozen-Throne:normal:bots", "bf:frozen-throne:normal:bots:x", "arena:2v2", "arena:1v1x", "bf:../x:normal:bots",
       "bf:frozen-throne:normal:bots ", "x" * 41, "", None, 5, "bf:frozen-throne:normal:%3Cb%3E"]
check("check_go: the live boss, both difficulties, the three ways; arena:1v1", all(BF.check_go(v) == v for v in good))
check("check_go: anything else refused", all(BF.check_go(v) is None for v in bad), [v for v in bad if BF.check_go(v)])
check("go_value: frozen-throne normal bots by default", BF.go_value() == "bf:frozen-throne:normal:bots")
check("go values fit the 40-character go limit", all(len(BF.go_value("frozen-throne", d, m)) <= 40 for d in BF.DIFFS for m in BF.MODES))
check("go_command: ovgo set for the account, upper case", BF.go_command("wr000123", "bf:frozen-throne:heroic:group")
      == "ovgo set WR000123 bf:frozen-throne:heroic:group" and BF.go_command("WR1", "arena:1v1") == "ovgo set WR1 arena:1v1")
check("go_command: bad account or go refused", BF.go_command("WR 1", "arena:1v1") is None and BF.go_command("WR1", "bf:x") is None
      and BF.go_command("WR1;x", "arena:1v1") is None)
calls = []
check("go_intent: confirmed", BF.go_intent(lambda c: calls.append(c) or "ovgo ok", "UNIT1", "arena:1v1") and calls == ["ovgo set UNIT1 arena:1v1"])
check("go_intent: the same again within 2 min is not resent", BF.go_intent(lambda c: calls.append(c) or "ovgo ok", "UNIT1", "arena:1v1") and len(calls) == 1)
def boom(c):
    raise wp.SoapError("down", "x")
check("go_intent: SOAP down is False, never raises", BF.go_intent(boom, "UNIT2", "arena:1v1") is False)
check("go_intent: no confirmation is False", BF.go_intent(lambda c: "There is no such command.", "UNIT3", "arena:1v1") is False)
check("describe: plain words", BF.describe("bf:frozen-throne:heroic:bots") == "Boss fight: The Frozen Throne (heroic, with companions)"
      and BF.describe("arena:1v1") == "1v1 arena queue" and BF.describe("bf:nope:normal:bots") is None)
check("mode_go: the catalog's ids", BF.mode_go("frozen-throne") == "bf:frozen-throne:normal:bots" and BF.mode_go("arena-queue") == "arena:1v1"
      and BF.mode_go("minigolf") is None and BF.mode_go(None) is None)
check("go_href: signed in, the play page", BF.go_href("arena:1v1", True) == "/wrath?go=arena:1v1")
sh = BF.go_href("bf:frozen-throne:normal:bots", False)
nxt = urllib.parse.parse_qs(urllib.parse.urlsplit(sh).query).get("next", [""])[0]
check("go_href: signed out, Log in first with the same play link next", nxt == "/wrath?go=bf:frozen-throne:normal:bots", sh)

# --- the portal: go_param, remember_go, the sign-in -------------------------------------------------------------------
check("go_param carries a boss fight", wp.go_param({"go": ["bf:frozen-throne:heroic:group"]}) == "&go=bf:frozen-throne:heroic:group")
check("go_param carries the arena queue", wp.go_param({"go": ["arena:1v1"]}) == "&go=arena:1v1")
check("go_param carries solo on this device (docs/PHONE-SOLO.md)", wp.go_param({"go": ["solo"]}) == "&go=solo:phone" == wp.go_param({"go": ["solo:phone"]})
      and wp.go_param({"go": ["solo:x"]}) == "")
check("go_param: the launcher's mode id becomes its go", wp.go_param({"go": ["mode:frozen-throne"]}) == "&go=bf:frozen-throne:normal:bots"
      and wp.go_param({"go": ["mode:arena-queue"]}) == "&go=arena:1v1")
check("go_param: other modes still carry nothing", wp.go_param({"go": ["mode:minigolf"]}) == "")
check("go_param: a bad boss fight is dropped", all(wp.go_param({"go": [v]}) == "" for v in bad if isinstance(v, str)))
check("go_param: share links unchanged", wp.go_param({"go": ["q:deadmines"]}) == "&go=q:deadmines" and wp.go_param({"go": ["b:icc-frozen-throne"]}) == "&go=b:icc-frozen-throne")
check("go_signin: keeps the go through Log in", urllib.parse.parse_qs(urllib.parse.urlsplit(wp.go_signin({"go": ["arena:1v1"]})).query)["next"] == ["/wrath?go=arena:1v1"])
check("go_signin: nothing without a go", wp.go_signin({}) == "" and wp.go_signin({"go": ["bf:bad"]}) == "")
check("auto sign-in page unchanged without a go", wp.auto_signin_page("n") == wp.render(
    f'{wp.crest(wp.REALM_NAME)}<p class="sub">Signing you in…</p>{wp.RULE}'
    f'<p class="fine center"><a href="{wp.SIGN_IN_URL}">Log in another way</a></p>', "n", wp.AUTO_SIGNIN_JS,
    meta=wp.card_meta("/wrath"), title="Log in", current="play"))
check("auto sign-in page: a weird address falls back", wp.SIGN_IN_URL in wp.auto_signin_page("n", "javascript:x").decode())

soap = {"commands": []}


class Soap(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_POST(self):
        cmd = wp._tag_text(self.rfile.read(int(self.headers["Content-Length"])), "command")
        soap["commands"].append(cmd)
        ok, text = (False, "Account with this name already exist!") if cmd.startswith("account create") else (
            True, "ovgo ok: noted" if cmd.startswith("ovgo") else "ovplay ok" if cmd.startswith("ovplay") else "done")
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


def get(path, cookie="", ip="10.9.0.1"):
    c = http.client.HTTPConnection("127.0.0.1", page_srv.server_address[1], timeout=20)
    hd = {"X-Forwarded-For": ip}
    if cookie:
        hd["Cookie"] = cookie
    c.request("GET", path, headers=hd)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read().decode("utf-8", "replace")


def sent(prefix):
    return [c for c in soap["commands"] if c.startswith(prefix)]


st, hd, b = get("/wrath?go=bf:frozen-throne:normal:bots")
check("signed out: nothing sent", sent("ovgo") == [])
js = re.search(r"location\.replace\('([^']*)'\)", b)
check("signed out /wrath with a boss fight: no saved key goes to Log in, then back with the same go",
      js and urllib.parse.parse_qs(urllib.parse.urlsplit(js.group(1)).query).get("next") == ["/wrath?go=bf:frozen-throne:normal:bots"],
      js.group(1) if js else b[:300])
check("signed out: Log in another way keeps the go too", "next=%2Fwrath%3Fgo%3Dbf%3Afrozen-throne%3Anormal%3Abots" in b)
st, hd, b = get("/wrath?go=arena:1v1", cookie="wr_session=stale")
check("a stale session: to Log in with the go", st in (302, 303) and "next=%2Fwrath%3Fgo%3Darena%3A1v1" in hd.get("location", ""), (st, hd.get("location")))
st, _, b = get("/wrath/account?go=bf:frozen-throne:heroic:group", cookie="wr_session=good")
check("signed in: the page works", st == 200 and "WR000123" in b, st)
check("signed in, heroes on: a boss fight is never an ovgo for the player's own account (it is played on a hero)",
      sent("ovgo") == [], sent("ovgo"))
get("/wrath/account?go=arena:1v1", cookie="wr_session=good")
check("signed in: the arena queue", sent("ovgo")[-1] == "ovgo set WR000123 arena:1v1", sent("ovgo"))
n_play, n_go = len(sent("ovplay")), len(sent("ovgo"))
get("/wrath/account?go=mode:frozen-throne", cookie="wr_session=good")
check("the launcher's ?go=mode:frozen-throne, heroes on: no ovgo, never ovplay",
      len(sent("ovgo")) == n_go and len(sent("ovplay")) == n_play, soap["commands"][-3:])
os.environ["WU_BF_HEROES"] = "0"   # heroes off: the boss fight go is the player's own account's intent, as before
BF._recent.clear()
get("/wrath/account?go=bf:frozen-throne:heroic:group", cookie="wr_session=good")
check("heroes off: ovgo set for the game account", sent("ovgo")[-1] == "ovgo set WR000123 bf:frozen-throne:heroic:group", sent("ovgo"))
get("/wrath/account?go=mode:frozen-throne", cookie="wr_session=good")
check("heroes off: the launcher's ?go=mode:frozen-throne is the boss fight go, never ovplay",
      sent("ovgo")[-1] == "ovgo set WR000123 bf:frozen-throne:normal:bots" and len(sent("ovplay")) == n_play, soap["commands"][-3:])
del os.environ["WU_BF_HEROES"]
n = len(sent("ovgo"))
for v in bad:
    if isinstance(v, str) and v:
        get("/wrath/account?go=" + urllib.parse.quote(v, safe=":"), cookie="wr_session=good")
check("bad boss fights are never sent", len(sent("ovgo")) == n, sent("ovgo")[n:])
_golf = next(m for m in wp.playmodes.CATALOG["modes"] if m["id"] == "minigolf")
_golf.pop("listed", None)   # listed again for a moment (hidden since 12 Nov: Duels and Boss Rush only)
get("/wrath/account?go=mode:minigolf", cookie="wr_session=good")
_golf["listed"] = False
check("other modes still go as ovplay (when listed)", sent("ovplay")[-1:] == ["ovplay go WR000123 minigolf"], sent("ovplay"))
n = len(sent("ovplay"))
get("/wrath/account?go=mode:minigolf", cookie="wr_session=good")
check("a hidden mode is never sent", len(sent("ovplay")) == n)

# --- the data -----------------------------------------------------------------------------------------------------
def row(ms, leader, roster, size=10, bots=0, date="2026-11-05", **kw):
    r = {"rank": 99, "ms": ms, "time": "9:99.9", "date": date, "leader": leader, "roster": roster, "size": size, "bots": bots}
    r.update(kw)
    return r


P = lambda n, c=2, bot=False: {"name": n, "class": c, "bot": bot}
doc = {"updated": "2026-11-05T19:00:00Z", "bosses": {
    "frozen-throne": {
        "normal": {
            "group": [row(400123, "Slowpoke", [P("Slowpoke", 1), P("Mira", "Mage")]),
                      row(312345, "Orion", [P("Orion", 2), P("Kel", "death knight"), P("Botone", 1, True), P("Bottwo", 5, True)],
                          bots=8, account=7, guid=1234, ip="1.2.3.4"),
                      row(312345, "Ana", [P("Ana", 8)], date="2026-11-04"),
                      row(0, "Zero", [P("Zero")]), row(-5, "Neg", [P("Neg")]), row(10 ** 12, "Huge", [P("Huge")]),
                      row(300000, "Lichking", [P("Lichking")]), row(300001, "Blizzardfan", [P("Blizzardfan"), P("Ok", 4)]),
                      row(300002, "Ghost", [P("Botonly", 1, True)]), "junk", None,
                      row(300003, "<script>", [P("<script>"), P("Fine", 3)])],
            "bots": [row(1234567, "Solo", [P("Solo", 7)] + [P(f"Comp{chr(97 + i)}", 1, True) for i in range(9)])],
            "weekly": [],
        },
        "heroic": {"group": [row(3_725_400, "Hero", [P("Hero", 6)])]},
        "mythic": {"group": [row(1, "Cheat", [P("Cheat")])]},
    },
    "onyxia": {"normal": {"group": [row(1, "Nope", [P("Nope")])]}}}}
json.dump(doc, open(os.path.join(OUT, "bossfights.json"), "w"))
d = BF.store.doc()
g = d["bosses"]["frozen-throne"]["normal"]["group"]
check("only the known boss and difficulties", set(d["bosses"]) == {"frozen-throne"} and set(d["bosses"]["frozen-throne"]) == {"normal", "heroic"})
check("updated: the ISO time", d["updated"] == 1793905200, d["updated"])
check("rows: fastest first, ranked here (the file's rank ignored), ties by date",
      [(r["rank"], r["leader"]) for r in g][:4] == [(1, "Ok"), (2, "Fine"), (3, "Ana"), (4, "Orion")], [(r["rank"], r["leader"], r["ms"]) for r in g])
orion = next(r for r in g if r["leader"] == "Orion")
check("time from ms, not the file's", orion["time"] == "5:12.3" and BF.clear_time(3_725_400) == "1:02:05.4" and BF.clear_time(59_999) == "0:59.9")
check("companions counted, never named", orion["bots"] == 8 and [p["name"] for p in orion["roster"]] == ["Orion", "Kel"]
      and "Botone" not in json.dumps(d))
check("classes: ids and names", [p["class"] for p in orion["roster"]] == [2, 6] and BF._class("Mage") == 8 and BF._class(99) == 0)
check("only whitelisted fields", set(orion) == {"rank", "ms", "time", "date", "leader", "roster", "size", "bots"} and "1.2.3.4" not in json.dumps(d))
check("bad times dropped (0, negative, over six hours)", not {"Zero", "Neg", "Huge"} & {r["leader"] for r in g})
check("brand-word and odd names dropped; the leader falls back to a shown player",
      not any(BANNED.search(r["leader"]) or "<" in r["leader"] for r in g) and {"Ok", "Fine"} <= {r["leader"] for r in g}
      and "Blizzardfan" not in json.dumps(d) and "script" not in json.dumps(d))
check("a run with no player to show is left out", "Ghost" not in json.dumps(d))
check("the companions board", d["bosses"]["frozen-throne"]["normal"]["bots"][0]["bots"] == 9
      and d["bosses"]["frozen-throne"]["normal"]["bots"][0]["roster"] == [{"name": "Solo", "class": 7}])
check("heroic: its own board", d["bosses"]["frozen-throne"]["heroic"]["group"][0]["leader"] == "Hero"
      and d["bosses"]["frozen-throne"]["heroic"]["bots"] == [])
for label, content in (("missing", None), ("broken JSON", "{not json"), ("a list (an older shape)", "[1,2,3]"),
                       ("an older shape (boards at the top)", json.dumps({"updated": 5, "group": [row(1, "A", [P("A")])]})),
                       ("bosses not an object", json.dumps({"bosses": [1]}))):
    path = os.path.join(OUT, "bossfights.json")
    if os.path.exists(path):
        os.remove(path)
    if content is not None:
        open(path, "w").write(content)
    BF.store._mtime = None
    BF.store._doc = BF.clean({})
    e = BF.store.doc()
    empty = all(not rows for diffs in e["bosses"].values() for boards in diffs.values() for rows in boards.values())
    check(f"{label}: empty boards, no error", empty and set(e["bosses"]) == {"frozen-throne"})
json.dump(doc, open(os.path.join(OUT, "bossfights.json"), "w"))
BF.store._mtime = None
check("iso_epoch: offsets and dates", BF.iso_epoch("2026-11-05T21:00:00+02:00") == 1793905200 and BF.iso_epoch("2026-11-05") == 1793836800
      and BF.iso_epoch("yesterday") == 0 and BF.iso_epoch(True) == 0)

# arena: no file yet = the ladder is coming
check("arena: no file, not present", BF.arena_store.doc()["present"] is False and BF.arena_store.doc()["ladder"] == [])

# --- the pages ----------------------------------------------------------------------------------------------------
def visible(page: str) -> str:
    page = (page.replace('not affiliated with or endorsed by Blizzard Entertainment', ' ') if isinstance(page, str) else page.replace(b'not affiliated with or endorsed by Blizzard Entertainment', b' '))   # the legal line is the one allowed mention (audit 12 Nov: it is in every footer now)
    page = re.sub(r"<style.*?</style>|<script.*?</script>|<!--.*?-->", " ", page, flags=re.S)
    attrs = " ".join(re.findall(r'(?:alt|title|content|aria-label|placeholder)="([^"]*)"', page))
    return re.sub(r"<[^>]+>", " ", page) + " " + attrs


st, hd, b = get("/wrath/bossfights")
check("/wrath/bossfights: 200 in the hub's frame", st == 200 and '<header class="hb-top">' in b and "bf-hero" in b, st)
check("the hero: The Frozen Throne, the tagline left to the owner", "<h1 id=\"hb-pg-h\">The Frozen Throne</h1>" in b and "TODO(owner)" in b)
check("signed out: both buttons Log in first, then the go",
      'href="/login?next=%2Fwrath%3Fgo%3Dbf%3Afrozen-throne%3Anormal%3Abots">Fight now with companions<' in b
      and 'href="/login?next=%2Fwrath%3Fgo%3Dbf%3Afrozen-throne%3Anormal%3Agroup">Queue with players<' in b)
check("how it works: 10 players, companions, normal or heroic", all(w in b for w in ("10 players", "companions", "Normal or heroic")))
check("the board: top of both, companions counted", "5:12.3" in b and "+8 companions" in b and "Botone" not in b and "With companions" in b)
check("coming next: Vault, Onyxia, Sartharion, Halion", all(n in b for n in ("Vault of Archavon", "Onyxia", "Sartharion", "Halion")))
check("no brand words people read", not BANNED.findall(visible(b)), BANNED.findall(visible(b))[:3])
check("no seat counts", not re.search(r"(?i)\bseats? (left|free|open)\b|\d+ in (line|queue)", visible(b)))
check("no hotlinked art: only our own /wrath/art", all(u.startswith("/wrath/art/") for u in re.findall(r'<img[^>]+src="([^"]+)"', b.split("bf-hero", 1)[1].split("</header>", 1)[0])))
check("no script on the page body", "<script" not in b.split('<main', 1)[-1].split("</main>", 1)[0])
st, _, h = get("/wrath/bossfights?d=heroic")
check("?d=heroic: heroic links and the heroic board", "bf%3Afrozen-throne%3Aheroic%3Abots" in h and "bf%3Afrozen-throne%3Aheroic%3Agroup" in h
      and "1:02:05.4" in h and 'aria-current="true">Heroic<' in h)
check("?d=nonsense: normal", "bf%3Afrozen-throne%3Anormal%3Abots" in get("/wrath/bossfights?d=%3Cx%3E")[2])
t, body = BF.boss_page({}, True)
check("signed in: the buttons go straight to the play page", 'href="/wrath?go=bf:frozen-throne:normal:bots"' in body
      and 'href="/wrath?go=bf:frozen-throne:normal:group"' in body and "You log in first" not in body)
check("the title", t == "Boss fights · The Frozen Throne · Leyline")
st, hd, b = get("/wrath/bossfights", ip="10.9.0.77")
check("mobile: the viewport tag and the phone layout rules", 'name="viewport"' in b and "@media (max-width:640px)" in b)

st, _, a = get("/wrath/arena")
check("/wrath/arena: 200, Queue 1v1 arena Logs in first then arena:1v1", st == 200
      and 'href="/login?next=%2Fwrath%3Fgo%3Darena%3A1v1">Queue 1v1 arena<' in a)
check("/wrath/arena: the duel corner and the boards", 'href="/wrath/h/duels"' in a and 'href="/wrath/leaderboards"' in a
      and 'href="/wrath/leaderboards?board=arena"' in a)
check("/wrath/arena: no brand words", not BANNED.findall(visible(a)))
check("arena signed in: straight to the play page", 'href="/wrath?go=arena:1v1"' in BF.arena_page({}, True)[1])

st, hd, j = get("/wrath/api/bossfights")
api = json.loads(j)
check("the JSON: 200, public", st == 200 and hd.get("access-control-allow-origin") == "*", hd)
fb = api["bosses"]["frozen-throne"]
check("the JSON: boards, go values, what's next", fb["boards"]["normal"]["group"][3]["leader"] == "Orion"
      and fb["go"]["heroic"]["group-only"] == "bf:frozen-throne:heroic:group-only" and api["arena"]["go"] == "arena:1v1"
      and [n["id"] for n in api["next"]] == ["vault-of-archavon", "onyxia", "sartharion", "halion"])
check("the JSON: no companions' names, no brand words", "Botone" not in j and not BANNED.findall(j))

st, _, lb = get("/wrath/leaderboards?board=bosses")
check("Leaderboards: a Boss fights tab", st == 200 and 'href="/wrath/leaderboards?board=bosses" class="on" aria-current="page">Boss fights<' in lb
      and "5:12.3" in lb and "This week" in lb and ".bf-t{" in lb)
check("Leaderboards: the duel tabs still there", 'href="/wrath/leaderboards">This week<' in lb and 'href="/wrath/leaderboards?board=all">All time<' in lb)
st, _, lh = get("/wrath/leaderboards?board=bosses&d=heroic")
check("Leaderboards: heroic", "1:02:05.4" in lh)
st, _, la = get("/wrath/leaderboards?board=arena")
check("Leaderboards: 1v1 arena tab, coming (no export yet)", st == 200 and "ladder is coming" in la and "1v1 arena</a>" in la)
st, _, lw = get("/wrath/leaderboards")
check("Leaderboards: the duel board by default, no boss board", st == 200 and "Duels at level 80" in lw and "Fastest clears" not in lw)
json.dump({"updated": "2026-11-05T19:00:00Z", "ladder": [{"name": "Duelist", "class": 4, "rating": 1830, "wins": 20, "losses": 4},
                                                          {"name": "Blizzardfan", "class": 4, "rating": 2000, "wins": 1, "losses": 0},
                                                          {"name": "Second", "class": "Rogue", "rating": 1700, "wins": 9, "losses": 9, "ip": "x"}]},
          open(os.path.join(OUT, "arena.json"), "w"))
BF.arena_store._mtime = None
st, _, la = get("/wrath/leaderboards?board=arena")
check("Leaderboards: the 1v1 ladder from arena.json, names checked", "Duelist" in la and "1830" in la and "Blizzardfan" not in la
      and la.index("Duelist") < la.index("Second"))

st, _, hub_page = get("/wrath/leaderboards")
nav = re.search(r'<nav class="hb-nav"[^>]*>(.*?)</nav>', hub_page, re.S).group(1)
check("nav: Boss fights and Duels and arena under More", '>Boss fights</span>' in nav and '>Duels and arena</span>' in nav
      and 'href="/wrath/bossfights"' in nav and 'href="/wrath/arena"' in nav)
st, _, bp = get("/wrath/bossfights")
navb = re.search(r'<nav class="hb-nav"[^>]*>(.*?)</nav>', bp, re.S).group(1)
check("nav: Boss fights marked on its page", re.search(r'class="hb-mi on" href="/wrath/bossfights" aria-current="page"', navb), navb[:200])

st, _, mp = get("/wrath/modes")
check("/wrath/modes: The Frozen Throne first under Boss fights, Fight now and Queue with players",
      'id="md-bf-frozen-throne"' in mp and 'href="/wrath?go=bf:frozen-throne:normal:bots">Fight now<' in mp
      and 'href="/wrath?go=bf:frozen-throne:normal:group">Queue with players<' in mp
      and mp.index('id="md-bosses"') < mp.index('id="md-bf-frozen-throne"') < mp.index('class="md-raid"'))
check("/wrath/modes: Queue 1v1 arena under Duels", 'href="/wrath?go=arena:1v1">Queue 1v1 arena<' in mp
      and mp.index('id="md-duels"') < mp.index('id="md-arena-queue"') < mp.index('id="md-bosses"'))
check("/wrath/modes: the Modes cards link their go", 'class="md-go" href="/wrath?go=bf:frozen-throne:normal:bots">Fight now<' in mp
      and 'class="md-go" href="/wrath?go=arena:1v1">Queue now<' in mp)
check("/wrath/modes: no brand words", not BANNED.findall(visible(mp)), BANNED.findall(visible(mp))[:3])
st, _, mj = get("/wrath/modes.json")
ids = [m["id"] for m in json.loads(mj)["modes"]]
check("/wrath/modes.json lists both for the launcher", "frozen-throne" in ids and "arena-queue" in ids)

# --- the data file, the sources, the slice --------------------------------------------------------------------------
D = BF.D
check("modes_data: the boss fights are not ov_go.lua places (the generated table is unchanged)",
      not any(k.startswith(("bf:", "arena:")) for k in D.places()) and D.check() == [])
for f in ("bossfights.py", "modes_data.py", "playmodes.json", "duels.py", "modes.py", os.path.join("www-files", "stages.js"),
          os.path.join("www-files", "wu-loading.js")):   # (wrath-page.js holds the brand filter's own word list)
    src = open(os.path.join(HERE, f), encoding="utf-8").read()
    check(f"no brand words in {f}", not BANNED.findall(src), BANNED.findall(src)[:3])
mini = json.load(open(os.path.join(HERE, "slices", "mini.json")))
icc = next(t for t in mini["tiers"] if t["key"] == "icc-toc")
whole = [m for m in icc["maps"] if m["id"] == 631]
check("mini.json: icc-toc holds the whole of map 631 (the platform included)", whole and not any(k in whole[0] for k in ("zone", "box")))
b0 = D.BOSS_FIGHTS[0]
check("modes_data: the fight's tier is icc-toc, in the slice order", b0["tier"] == "icc-toc" and "icc-toc" in mini["order"])
tile = (math.floor(32 - b0["pos"][2] / (1600 / 3)), math.floor(32 - b0["pos"][1] / (1600 / 3)))
st_js = open(os.path.join(HERE, "www-files", "stages.js"), encoding="utf-8").read()
check("stages.js BOSS_TILES: the platform's tile", f"'frozen-throne': ['IcecrownCitadel', {tile[0]}, {tile[1]}]" in st_js, tile)
lo = open(os.path.join(HERE, "www-files", "wu-loading.js"), encoding="utf-8").read()
wpj = open(os.path.join(HERE, "www-files", "wrath-page.js"), encoding="utf-8").read()
check("wrath-page.js goLink knows bf: and arena:", "bf: /^frozen-throne:(?:normal|heroic):(?:bots|group|group-only)$/" in wpj
      and "arena: /^1v1$/" in wpj)
check("wu-loading.js: the boss fight note, no seat counts", "getting your raid ready" in lo and "'frozen-throne': 'The Frozen Throne'" in lo
      and not re.search(r"(?i)seat", lo.split("BOSS_NAMES", 1)[1].split("/** Put a rotating", 1)[0]))

# --- deploy wiring --------------------------------------------------------------------------------------------------
bat = open(os.path.join(HERE, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
check("sign-in bat packs bossfights.py and strips its CRs", 'if exist bossfights.py set "LINKS=%LINKS% bossfights.py"' in bat
      and " bossfights.py " in bat.split("sed -i")[1])
fl = open(os.path.join(HERE, "fastloop.sh"), encoding="utf-8").read()
check("fastloop.sh installs bossfights.py, backs it up and puts it back",
      'install -m 644 bossfights.py "$WU/portal/bossfights.py"' in fl and 'cp -p "$WU/portal/bossfights.py" "$BK/bossfights.py"' in fl
      and 'cp -p "$BK/bossfights.py" "$WU/portal/bossfights.py"; else rm -f "$WU/portal/bossfights.py"' in fl
      and "/wrath/api/bossfights" in fl)
check("bossfights.py and modes_data.py ship together (modes.py's pair)", 'if exist modes.py if exist modes_data.py' in bat)
docs = open(os.path.join(REPO, "docs", "BOSS-FIGHTS.md"), encoding="utf-8").read()
check("docs/BOSS-FIGHTS.md: the Website section", "## Website" in docs and "bf:frozen-throne" in docs and "bossfights.json" in docs)
check("docs: no brand words", not BANNED.findall(docs), BANNED.findall(docs)[:3])

print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
