"""Leyline game modes (stream/playmodes.py + stream/playmodes.json): the catalog and its checker, GET /wrath/modes.json,
the Modes section of /wrath/modes (Duels and Boss Rush; the rest listed: false), and ?go=mode:<id> through sign-in ("ovplay go" over SOAP for
realm/custom/lua_scripts/ov_playmode.lua) against a fake SOAP. Also kept in step: the realm's table, the server-pass
.bat, the deploy scripts and the launcher's built-in copy.

    python3 -I stream/tests/test_playmodes.py stream/wrath_portal.py
"""
import copy, http.client, importlib.util, json, os, re, sys, tempfile, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from xml.sax.saxutils import escape

SOAP_PORT, REALM_PORT, PAGE_PORT = 17993, 18293, 18294
HERE = os.path.dirname(os.path.abspath(sys.argv[1]))
REPO = os.path.dirname(HERE)
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL=f"http://127.0.0.1:{SOAP_PORT}/",
                  PORTAL_URL=f"http://127.0.0.1:{REALM_PORT}", WU_STREAM="0", STREAM_HOST="wrath.example",
                  PUBLIC_HOST="play.example", WU_RELAY_LISTING="http://127.0.0.1:9/realms.json", WU_ART_DIR=tempfile.mkdtemp())
spec = importlib.util.spec_from_file_location("wp", sys.argv[1])
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
wp.realm_status = lambda: {"online": True, "players": 3, "bots": 10}

soap = {"commands": [], "ovplay": "ovplay ok: WR000123 -> minigolf (30 min)"}


class Soap(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_POST(self):
        cmd = wp._tag_text(self.rfile.read(int(self.headers["Content-Length"])), "command")
        soap["commands"].append(cmd)
        if cmd.startswith("account create"):
            ok, text = False, "Account with this name already exist!"
        elif cmd.startswith("ovplay"):
            ok, text = soap["ovplay"] is not None, soap["ovplay"] or "There is no such command."
        else:
            ok, text = True, "done"
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


for port, h in ((SOAP_PORT, Soap), (REALM_PORT, Realm), (PAGE_PORT, wp.PageHandler)):
    srv = ThreadingHTTPServer(("127.0.0.1", port), h)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()


def get(path, cookie="", ip="10.9.0.1", method="GET"):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=20)
    hd = {"X-Forwarded-For": ip}
    if cookie:
        hd["Cookie"] = cookie
    c.request(method, path, headers=hd)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, extra)


PM = wp.playmodes
check("playmodes.py loaded beside the portal", PM is not None)
if PM is None:
    print(f"\n{passed}/{passed + failed} passed"); sys.exit(1)
check("the catalog passes its check", PM.ERRORS == [] and PM.CATALOG is not None, PM.ERRORS)
ALL = ["minigolf", "bumper-balls", "coin-dash", "community", "brawl", "azeroth-survivors", "hordes-of-azeroth",
       "arena-1v1", "keystones", "transmog", "hardcore", "frozen-throne", "arena-queue"]
HIDDEN = [i for i in ALL if i not in ("frozen-throne", "arena-queue")]
check("the seed modes, all kept in the catalog", [m["id"] for m in PM.all_entries()] == ALL, [m["id"] for m in PM.all_entries()])
ids = [m["id"] for m in PM.entries()]
check("listed: Duels and Boss Rush only, Duels first (owner 12 Nov)", ids == ["arena-queue", "frozen-throne"], ids)
check("the rest are hidden, not deleted (listed: false)", PM.hidden_ids() == HIDDEN
      and all(PM.by_id(i)["listed"] is False for i in HIDDEN), PM.hidden_ids())
check("the two are called Duels and Boss Rush", PM.by_id("arena-queue")["title"] == "Duels" and PM.by_id("frozen-throne")["title"] == "Boss Rush")
check("hidden modes keep their status (live or coming) for when they come back",
      [PM.by_id(i)["status"] for i in HIDDEN] == ["live"] * 5 + ["coming"] * 2 + ["live"] * 4)


class relist:
    """Bring hidden modes back for a moment (the catalog in memory), as deleting "listed": false would."""
    def __init__(self, *mids): self.mids, self.saved = mids, {}
    def __enter__(self):
        for m in PM.CATALOG["modes"]:
            if m["id"] in self.mids: self.saved[m["id"]] = m.pop("listed", None)
    def __exit__(self, *a):
        for m in PM.CATALOG["modes"]:
            if self.saved.get(m["id"]) is False: m["listed"] = False


INGAME = {"arena-1v1": ".hub arena", "keystones": ".hub keys", "transmog": ".hub transmog", "hardcore": ".hub hardcore"}
check("the in-game modes: live, found with .hub", all(PM.by_id(k)["kind"] == "in-game" and PM.by_id(k)["status"] == "live"
      and PM.by_id(k)["start"] == {"cmd": v} for k, v in INGAME.items()))
gol = open(os.path.join(REPO, "realm", "custom", "lua_scripts", "ov_go.lua"), encoding="utf-8").read()
hubt = gol[gol.index("local HUB = {"):gol.index("local HUB_ORDER")]
check("every .hub spot in the catalog is one ov_go.lua knows", all(re.search(rf"^\s+{v.split()[1]}\s+= \{{", hubt, re.M) for v in INGAME.values()))
check("no realm mode to send while the minigames are hidden", PM.realm_ids() == [], PM.realm_ids())
with relist(*HIDDEN):
    check("listed again: live realm modes", PM.realm_ids() == ["minigolf", "bumper-balls", "coin-dash", "community"], PM.realm_ids())
    check("listed again: boss fight and arena queue, never an ovplay id",
          sorted(PM.live_realm_ids()) == sorted(["minigolf", "bumper-balls", "coin-dash", "community", "frozen-throne", "arena-queue"])
          and not set(PM.GO_AS) & set(PM.realm_ids()), PM.live_realm_ids())
check("hidden again after", PM.hidden_ids() == HIDDEN)
check("boss fight and arena queue: live realm entries, never an ovplay id",
      PM.live_realm_ids() == ["arena-queue", "frozen-throne"] and not set(PM.GO_AS) & set(PM.realm_ids()), PM.live_realm_ids())
check("GO_AS = stream/bossfights.py MODE_GO", wp.bossfights is not None and PM.GO_AS == wp.bossfights.MODE_GO)
check("the site's Play links their own go", PM.play_href(PM.by_id("frozen-throne")) == "/wrath?go=bf:frozen-throne:normal:bots"
      and PM.play_href(PM.by_id("arena-queue")) == "/wrath?go=arena:1v1")
check("no brand words in their titles and lines", not any(PM.BANNED.search(PM.by_id(i)[f]) for i in PM.GO_AS for f in ("title", "line")))
check("every listed mode has its how-to and a board", all(PM.DETAILS.get(i, {}).get("how") and PM.DETAILS[i].get("board") for i in ids)
      and {PM.DETAILS[i]["board"] for i in ids} == {"duels", "bossrush"})
surv, hordes = PM.by_id("azeroth-survivors"), PM.by_id("hordes-of-azeroth")
check("Azeroth Survivors: offline, coming, credited (MIT or Apache-2.0)", surv["kind"] == "offline" and surv["status"] == "coming"
      and surv["credit"]["by"] == "Aethus (original game)" and surv["credit"]["note"] == "Open rebuild by OutThisLife" and "Apache-2.0" in surv["credit"]["license"] and "MIT" in surv["credit"]["license"]
      and surv["credit"]["source"] == "https://github.com/OutThisLife/azeroth-survivors-re" and surv["start"] == {"path": "/wrath/survivors"} and "in your browser now" in surv["line"])
check("Hordes of Azeroth: a realm type, coming, both repos and licenses", hordes["kind"] == "realm-type" and hordes["status"] == "coming"
      and hordes["credit"]["by"] == "jonhaldors" and "GPL-3.0" in hordes["credit"]["license"] and "GPL-2.0" in hordes["credit"]["license"]
      and hordes["credit"]["source"] == "https://github.com/jonhaldors/hordes-of-azeroth"
      and hordes["credit"]["more"] == ["https://github.com/jonhaldors/hordes-of-azeroth-server"])
check("every third-party entry is credited", all(PM.by_id(i).get("credit") for i in ("azeroth-survivors", "hordes-of-azeroth")))

# --- the checker refuses ----------------------------------------------------------------------------------------
base = json.load(open(os.path.join(HERE, "playmodes.json")))


def bad(fn):
    d = copy.deepcopy(base)
    fn(d)
    return PM.check(d)


for name, fn in (
        ("not the catalog", lambda d: d.clear()),
        ("schema 2", lambda d: d.update(schema=2)),
        ("bad id", lambda d: d["modes"][0].update(id="Mini Golf")),
        ("duplicate id", lambda d: d["modes"][1].update(id="minigolf", start={"go": "mode:minigolf"})),
        ("unknown field", lambda d: d["modes"][0].update(price=5)),
        ("markup in a title", lambda d: d["modes"][0].update(title="<b>Golf</b>")),
        ("a brand name", lambda d: d["modes"][0].update(line="The best minigolf in all of Warcraft, honest.")),
        ("a short line", lambda d: d["modes"][0].update(line="Golf.")),
        ("unknown kind", lambda d: d["modes"][0].update(kind="arcade")),
        ("unknown status", lambda d: d["modes"][0].update(status="beta")),
        ("listed true (leave it out instead)", lambda d: d["modes"][0].update(listed=True)),
        ("listed not a bool", lambda d: d["modes"][0].update(listed="no")),
        ("players max below min", lambda d: d["modes"][0].update(players={"min": 4, "max": 2})),
        ("players not numbers", lambda d: d["modes"][0].update(players={"min": True, "max": 2})),
        ("realm mode with another go", lambda d: d["modes"][0].update(start={"go": "mode:coin-dash"})),
        ("realm mode with a url", lambda d: d["modes"][0].update(start={"path": "/wrath"})),
        ("two ways to start", lambda d: d["modes"][0].update(start={"go": "mode:minigolf", "path": "/wrath"})),
        ("offline path off the site", lambda d: d["modes"][5].update(start={"path": "https://evil.example/x"})),
        ("offline path with a quote", lambda d: d["modes"][5].update(start={"path": '/wrath/play?x="1'})),
        ("realm type bad preset", lambda d: d["modes"][4].update(start={"host": "../x"})),
        ("credit without a license", lambda d: d["modes"][5]["credit"].pop("license")),
        ("credit http source", lambda d: d["modes"][5]["credit"].update(source="http://github.com/x")),
        ("credit javascript source", lambda d: d["modes"][5]["credit"].update(source="javascript:alert(1)")),
        ("credit more not https", lambda d: d["modes"][6]["credit"].update(more=["ftp://x.example/y"])),
        ("credit more not a list", lambda d: d["modes"][6]["credit"].update(more="https://github.com/x")),
        ("in-game mode with another command", lambda d: d["modes"][7].update(start={"cmd": ".gm on"})),
        ("in-game mode with a go", lambda d: d["modes"][7].update(start={"go": "mode:arena-1v1"})),
        ("in-game command with more words", lambda d: d["modes"][7].update(start={"cmd": ".hub arena; .server shutdown"})),
        ("realm mode with a command", lambda d: d["modes"][0].update(start={"cmd": ".hub arena"})),
):
    check(f"checker refuses: {name}", bad(fn) != [])
check("checker on a broken file: no catalog, a reason", PM.load(os.path.join(tempfile.mkdtemp(), "none.json"))[0] is None)

# --- /wrath/modes.json --------------------------------------------------------------------------------------------
st, h, b = get("/wrath/modes.json")
doc = json.loads(b)
check("GET /wrath/modes.json: 200 json", st == 200 and h.get("content-type", "").startswith("application/json"), st)
check("modes.json is the catalog's listed modes (old launchers show only these too)",
      doc == {"schema": 1, "modes": [m for m in sorted(base["modes"], key=lambda m: ids.index(m["id"]) if m["id"] in ids else 9)
                                    if m.get("listed", True)]} and [m["id"] for m in doc["modes"]] == ids)
check("modes.json carries no hidden mode", not any(m["id"] in HIDDEN for m in doc["modes"]))
check("modes.json readable from anywhere, cached", h.get("access-control-allow-origin") == "*" and "max-age" in h.get("cache-control", ""))
check("HEAD: no body", get("/wrath/modes.json", method="HEAD")[2] == b"")
check("trailing slash too", get("/wrath/modes.json/")[0] == 200)

# --- the Modes section ----------------------------------------------------------------------------------------------
st, _, b = get("/wrath/modes")
page = b.decode()
check("/wrath/modes still 200", st == 200)
check("Modes comes first, with a jump link", 'id="games"' in page and '>Modes</h2>' in page and 'href="#games"' in page
      and page.index('id="games"') < page.index('id="duels"'))
for m in PM.entries():
    check(f"card {m['id']}", f'id="mode-{m["id"]}"' in page and wp.html.escape(m["title"]) in page)
for mid in HIDDEN:
    check(f"{mid}: not on the page", f'id="mode-{mid}"' not in page)
check("no minigame names on the page", not re.search(r"Minigolf|Bumper Balls|Coin Dash|Community modes|Wrath Brawl|Azeroth Survivors|Hordes of Azeroth|Keystone|Transmog|Hardcore", page))
check("Duels: Queue now to the arena queue, its how-to, its page and the duels board",
      re.search(r'id="mode-arena-queue".*?<ol class="md-how">.*?data-board="duels".*?href="/wrath\?go=arena:1v1">Queue now<.*?href="/wrath/arena"', page, re.S))
check("Boss Rush: Fight now, its how-to, the bossrush board and the boss fights page",
      re.search(r'id="mode-frozen-throne".*?<ol class="md-how">.*?data-board="bossrush".*?href="/wrath\?go=bf:frozen-throne:normal:bots">Fight now<.*?href="/wrath/bossfights"', page, re.S))
check("Duels comes before Boss Rush", page.index('id="mode-arena-queue"') < page.index('id="mode-frozen-throne"'))
check("one line saying more is coming, under the cards", page.count('id="md-more-coming"') == 1
      and page.index('id="md-more-coming"') > page.index('id="mode-frozen-throne"'))
check("the minigames' host invite is gone", 'id="md-host-own"' not in page)
check("no Ways to play shelf and no Host a mode on this page", 'id="shelf"' not in page and 'id="host-modes"' not in page)
check("Dungeons and Hangouts are off the page (modes.SHOW)", 'id="dungeons"' not in page and 'id="hangouts"' not in page
      and 'href="#dungeons"' not in page and wp.modes.SHOW == {"dungeons": False, "hangouts": False})
check("players text", "2 players" in page and "1-10 players" in page)
BANNED = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
vis = re.sub(r"<[^>]+>", " ", re.sub(r"<style.*?</style>", " ", page, flags=re.S))
check("no game-brand names in the page", not BANNED.findall(vis.replace("not affiliated with or endorsed by Blizzard Entertainment", " ")))   # the legal line (footer) is allowed
check("no game-brand names in the catalog", not BANNED.findall(json.dumps(base)))
check("no script on the page", "<script" not in page)
first = PM.entries()[0]
saved = first["title"], first["line"]
first["title"], first["line"] = 'Go<script>x</script>"', "a & b <img src=x>"
sec = PM.section()
first["title"], first["line"] = saved
check("escaped", "Go&lt;script&gt;x&lt;/script&gt;&quot;" in sec and "a &amp; b &lt;img src=x&gt;" in sec and "<script>" not in sec)
check("two shots now (Duels, Boss Rush); the arena and boss fight buttons", page.count('class="md-shot"') == 2
      and ">Queue 1v1 arena<" in page and ">Fight now<" in page)
with relist(*HIDDEN):
    sec = PM.section()
    check("listed again, the old cards come back as they were",
          all(f'id="mode-{i}"' in sec for i in HIDDEN) and re.search(r'id="mode-brawl".*?href="/wrath/host">Host it<', sec, re.S)
          and all(f"<code>{c}</code>" in sec for c in INGAME.values()) and "Coming soon" in sec)

# --- the play intent: unit --------------------------------------------------------------------------------------------
check("go_mode: a hidden mode is never a play intent", PM.go_mode("mode:minigolf") is None and PM.go_mode("mode:coin-dash") is None)
check("go_mode: the boss fight and the queue still resolve", PM.go_mode("mode:frozen-throne") == "frozen-throne" and PM.go_mode("mode:arena-queue") == "arena-queue")
check("go_command: hidden is refused", PM.go_command("wr000123", "minigolf") is None)
with relist(*HIDDEN):
    check("go_mode (listed again)", PM.go_mode("mode:minigolf") == "minigolf" and PM.go_mode("mode:coin-dash") == "coin-dash")
    for v in ("mode:brawl", "mode:azeroth-survivors", "mode:MINIGOLF", "mode:minigolf x", "mode:", "minigolf", "q:minigolf",
              "mode:" + "a" * 40, None, 5):
        check(f"go_mode refuses {v!r}", PM.go_mode(v) is None)
    check("go_command", PM.go_command("wr000123", "minigolf") == "ovplay go WR000123 minigolf")
    for acct, mid in (("WR 1", "minigolf"), ("A" * 17, "minigolf"), ("WR1;x", "minigolf"), ("WR1", "brawl"), ("WR1", "x;y"), (None, "minigolf")):
        check(f"go_command refuses {acct!r} {mid!r}", PM.go_command(acct, mid) is None)
    calls = []
    check("go_intent: confirmed", PM.go_intent(lambda c: calls.append(c) or "ovplay ok", "UNIT1", "minigolf") and calls == ["ovplay go UNIT1 minigolf"])
    check("go_intent: the same again within 2 min is not resent", PM.go_intent(lambda c: calls.append(c) or "ovplay ok", "UNIT1", "minigolf") and len(calls) == 1)

    def boom(c):
        raise wp.SoapError("down", "x")

    check("go_intent: SOAP down is False, never raises", PM.go_intent(boom, "UNIT2", "minigolf") is False)
    check("go_intent: no confirmation is False", PM.go_intent(lambda c: "There is no such command.", "UNIT3", "minigolf") is False)
check("go_intent: hidden, nothing sent", PM.go_intent(lambda c: calls.append(c) or "ovplay ok", "UNIT4", "coin-dash") is False and len(calls) == 1)


# --- through the portal -------------------------------------------------------------------------------------------------
def sent(prefix):
    return [c for c in soap["commands"] if c.startswith(prefix)]


get("/wrath?go=mode:minigolf")
check("signed out: nothing sent", sent("ovplay") == [])
st, _, b = get("/wrath/account?go=mode:bumper-balls", cookie="wr_session=good")
check("signed in, a hidden mode: the page still works, nothing sent", st == 200 and b"WR000123" in b and sent("ovplay") == [], sent("ovplay"))
with relist(*HIDDEN):
    st, _, b = get("/wrath/account?go=mode:bumper-balls", cookie="wr_session=good")
    check("listed again, signed in: ovplay go for the game account", sent("ovplay") == ["ovplay go WR000123 bumper-balls"], sent("ovplay"))
    check("never as an ovgo place", sent("ovgo") == [])
    get("/wrath?go=mode:coin-dash", cookie="wr_session=good")
    check("/wrath with a mode", sent("ovplay")[-1] == "ovplay go WR000123 coin-dash", sent("ovplay"))
    n = len(sent("ovplay"))
    for v in ("mode:brawl", "mode:azeroth-survivors", "mode:hordes-of-azeroth", "mode:nope", "mode:MINIGOLF", "mode:%3Cx%3E", "mode:minigolf%20x"):
        get("/wrath/account?go=" + v, cookie="wr_session=good")
    check("only live realm modes are sent", len(sent("ovplay")) == n, sent("ovplay")[n:])
    soap["ovplay"] = None
    st, _, b = get("/wrath/account?go=mode:community", cookie="wr_session=good")
    check("realm without ov_playmode.lua: the page still works", st == 200 and b"WR000123" in b and sent("ovplay")[-1] == "ovplay go WR000123 community")
    soap["ovplay"] = "ovplay ok"
n = len(sent("ovplay"))
get("/wrath/account?go=mode:community", cookie="wr_session=good")
check("hidden again: nothing sent", len(sent("ovplay")) == n)
check("the gate's go carries no mode (the realm acts on the account; nothing in the page)", wp.go_param({"go": ["mode:minigolf"]}) == "")
check("share-link go values still work", wp.go_param({"go": ["q:deadmines"]}) == "&go=q:deadmines")

# --- kept in step ---------------------------------------------------------------------------------------------------------
lua = open(os.path.join(REPO, "realm", "custom", "lua_scripts", "ov_playmode.lua"), encoding="utf-8").read()
table = lua[lua.index("local MODES = {"):lua.index("\n}\n", lua.index("local MODES = {"))]
lua_ids = re.findall(r'\["([a-z0-9-]+)"\] = \{', table)
with relist(*HIDDEN):
    realm_now, live_now = PM.realm_ids(), PM.live_realm_ids()
# the boss fight and arena queue ids may be in the realm's table too (the desktop launcher's LeylinePlay addon sends
# "wuplay go <id>"; docs/BOSS-FIGHTS.md), or not yet; the hidden minigames stay in it (and still start with .playmode)
check("ov_playmode.lua's table = the catalog's realm modes, hidden ones included", set(realm_now) <= set(lua_ids)
      and set(lua_ids) <= set(live_now), (lua_ids, realm_now))
for m in PM.all_entries():
    if m["kind"] == "realm" and m["id"] not in PM.GO_AS:
        check(f"ov_playmode.lua title of {m['id']}", f'["{m["id"]}"] = {{ title = "{m["title"]}"' in table)
        check(f"ov_playmode.lua keeps {m['id']} off .playmode's list while it is hidden",
              re.search(rf'\["{m["id"]}"\] = \{{[^\n]*listed = false', table) is not None)
for i in PM.GO_AS:
    check(f"ov_playmode.lua lists {i}", re.search(rf'\["{i}"\] = \{{[^\n]*listed = false', table) is None)
bat = open(os.path.join(REPO, "realm", "Server pass - finder 10 and bug reports.bat"), "rb").read()
check("server pass .bat copies ov_playmode.lua", b'copy /y "custom\\lua_scripts\\ov_playmode.lua" "%T%\\" >nul' in bat)
check("server pass .bat line endings unchanged (one style)", (b"\r\n" in bat) == (bat.count(b"\n") == bat.count(b"\r\n")))
for f in ("Update sign-in and warm-up.bat", "fastloop.sh", os.path.join("server", "stream-up.sh"), "Update browser client.bat"):
    t = open(os.path.join(HERE, f), encoding="utf-8", errors="replace").read()
    check(f"deploy: {f} carries playmodes.py and playmodes.json", "playmodes.py" in t and "playmodes.json" in t)
rs = open(os.path.join(REPO, "launcher", "src-tauri", "src", "modes.rs"), encoding="utf-8").read()
check("the launcher's built-in copy is this file", 'include_str!("../../../stream/playmodes.json")' in rs)
check("the launcher reads /wrath/modes.json", 'crate::site!("/wrath/modes.json")' in rs)

print(f"\n{passed}/{passed + failed} passed")
sys.exit(0 if failed == 0 else 1)
