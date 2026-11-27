"""The signed leaderboards on the website (stream/boards.py) through the real portal handler: the boards checked before
they show, a tampered or foreign board refused, the page (Boss Rush, Duels, Local runs, filters, the verified mark),
the JSON (byte for byte, its own type), a community realm's submit, a local run from the launcher, the deploy wiring.

    python3 -I stream/tests/test_boards.py stream/wrath_portal.py
"""
import copy, http.client, importlib.util, json, os, re, shutil, sys, tempfile, threading, time
from http.server import ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(sys.argv[1]))
REPO = os.path.dirname(HERE)
TMP = tempfile.mkdtemp(prefix="wu-boards-test-")
DUELS = os.path.join(TMP, "duels")
DATA = os.path.join(TMP, "data")
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json",
                  WU_DUELS_DIR=DUELS, WU_DUELS_OUT=DUELS, WU_ARMORY_DIR=os.path.join(TMP, "armory"), PUBLIC_HOST="play.example",
                  WU_PORTAL_DATA=DATA)

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, str(extra)[:400])

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
    return m

ex = load("boards_export", os.path.join(REPO, "realm", "custom", "boards-export.py"))
NOW = int(time.time())
WK = ex.week_start(NOW)
T = WK + 3600   # this week
OLD = WK - 3 * 86400   # last week

def res(**k):
    base = {"v": 1, "realm": "", "mode": "bossrush", "board": "frozen-throne", "diff": "normal", "kind": "bots", "class": "mage",
            "size": 10, "bots": 9, "party": [], "at": T}
    return {**base, **k}

sec_off = bytes([1] * 32)
sec_com = bytes([2] * 32)
sec_bad = bytes([3] * 32)
wk = "week:" + ex.day(WK)
official = ex.make_board(sec_off, [
    res(name="Orion", ms=400000, run=1, deaths=2),
    res(name="Orion", ms=350000, at=OLD, run=2),
    res(name="Jaina", ms=380000, run=3, kind="group", bots=0, party=[{"name": "Jaina", "class": "mage", "bot": False}]),
    res(name="Thrall", ms=300000, diff="heroic", run=4),
    res(name="Thrall", ms=200000, board="bone-lord", run=5),
    res(name="Blizzard", ms=100000, run=6),
    res(name="Evil<b>", ms=100000, run=7),
    {"v": 1, "realm": "", "mode": "duels", "board": "duel", "period": "all", "name": "Orion", "class": "deathknight",
     "rating": 1640, "wins": 8, "losses": 2, "points": 26, "at": NOW},
    {"v": 1, "realm": "", "mode": "duels", "board": "duel", "period": "all", "name": "Thrall", "class": "shaman",
     "rating": 1700, "wins": 3, "losses": 3, "points": 12, "at": NOW},
    {"v": 1, "realm": "", "mode": "duels", "board": "duel", "period": wk, "name": "Jaina", "class": "mage",
     "rating": 0, "wins": 5, "losses": 0, "points": 15, "at": NOW},
    {"v": 1, "realm": "", "mode": "duels", "board": "arena-1v1", "period": wk, "name": "Sylvana", "class": "hunter",
     "rating": 0, "wins": 3, "losses": 1, "points": 0, "at": NOW},
], NOW, "Wrath Unleashed")
OFF_ID = official["doc"]["realm"]
os.makedirs(os.path.join(DUELS, "boards"))
with open(os.path.join(DUELS, "boards", OFF_ID + ".json"), "w") as f:
    json.dump(official, f, ensure_ascii=True, separators=(",", ":"))
# a planted, tampered copy under another id: never shown
bad = ex.make_board(sec_bad, [res(name="Cheater", ms=1000, run=9)], NOW, "Fake")
bad["doc"]["results"][0]["r"]["ms"] = 1
with open(os.path.join(DUELS, "boards", bad["doc"]["realm"] + ".json"), "w") as f:
    json.dump(bad, f)

wp = load("wp", sys.argv[1])
wp.realm_status = lambda: {"online": True, "players": 4, "bots": 0}
B = wp.boards
check("boards.py loaded beside the portal", B is not None and B.match("/wrath/boards") and B.match("/wrath/api/boards/index.json")
      and not B.match("/wrath/boardsx"))
COM_ID = ex.realm_id(ex.public_key(sec_com))
LISTED = {COM_ID}
wp.community_realms = lambda: ([{"id": i, "name": "Listed"} for i in LISTED], True)

check("the same rules as the export: a good board", B.check_board(official, NOW) == (True, "ok"))
check("the same rules: a tampered one", B.check_board(bad, NOW)[0] is False)
check("store: the tampered board is left out", [b["doc"]["realm"] for _, b in B.store.all(NOW)] == [OFF_ID])
rows = B.rows(B.store.all(NOW))
check("rows: brand words and odd names never", not any(r["name"] in ("Blizzard", "Evil<b>") for r in rows))
check("rows: each with the realm that signed it", all(r["realm"] == {"id": OFF_ID, "name": "Wrath Unleashed", "kind": "official"} for r in rows))
r = B.rank(rows, "bossrush", "week", NOW, "frozen-throne", "normal")
check("rank: this week by time, each name once", [(x["name"], x["ms"]) for x in r] == [("Jaina", 380000), ("Orion", 400000)], r)
r = B.rank(rows, "bossrush", "all", NOW, "frozen-throne", "normal")
check("rank: all time takes last week's faster run", [(x["name"], x["ms"]) for x in r] == [("Orion", 350000), ("Jaina", 380000)], r)
check("rank: per fight and difficulty", [x["name"] for x in B.rank(rows, "bossrush", "all", NOW, "frozen-throne", "heroic")] == ["Thrall"]
      and [x["name"] for x in B.rank(rows, "bossrush", "all", NOW, "bone-lord", "normal")] == ["Thrall"])
check("rank: duels all time by rating", [x["name"] for x in B.rank(rows, "duels", "all", NOW, "duel")] == ["Thrall", "Orion"])
check("rank: duels this week by points", [x["name"] for x in B.rank(rows, "duels", "week", NOW, "duel")] == ["Jaina"])
check("rank: the arena's week by wins", [x["name"] for x in B.rank(rows, "duels", "week", NOW, "arena-1v1")] == ["Sylvana"])
check("rank: a name search", [x["name"] for x in B.rank(rows, "bossrush", "all", NOW, "frozen-throne", "normal", q="ori")] == ["Orion"])
check("clock: m:ss.t", B.clock(312345) == "5:12.3" and B.clock(59999) == "0:59.9")
check("catalog: the same as the export's", tuple(k for k, _ in B.FIGHTS) == ex.CATALOG)

# --- through the real handler -------------------------------------------------------------------------------
srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
threading.Thread(target=srv.serve_forever, daemon=True).start()
def req(method, path, body=None, headers=None, ip="203.0.113.5"):
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=20)
    h = {"X-Forwarded-For": ip, **(headers or {})}
    data = None
    if body is not None:
        data = body if isinstance(body, bytes) else json.dumps(body).encode()
        h["Content-Type"] = "application/json"
    c.request(method, path, body=data, headers=h); res_ = c.getresponse()
    return res_.status, dict(res_.getheaders()), res_.read()

def visible(b):
    b = (b.replace('not affiliated with or endorsed by Blizzard Entertainment', ' ') if isinstance(b, str) else b.replace(b'not affiliated with or endorsed by Blizzard Entertainment', b' '))   # the legal line is the one allowed mention (audit 12 Nov: it is in every footer now)
    return re.sub(r"<[^>]+>", " ", re.sub(r"<style.*?</style>|<script.*?</script>", " ", b, flags=re.S))
BRANDS = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
VERS = re.compile(r"3\.3\.5|12340|1\.12|2\.4\.3")

s, hd, b = req("GET", "/wrath/boards")
b = b.decode()
check("GET page: 200, no script", s == 200 and "<script" not in b and "script-src 'none'" in hd.get("Content-Security-Policy", ""), s)
check("page: Boss Rush tab on, the featured fight, the board", 'aria-current="page">Boss Rush<' in b and "This week's featured fight" in b
      and "The Frozen Throne" in b and "6:20.0" in b and "6:40.0" in b, visible(b)[-1500:])
check("page: the verified mark names the realm", "Signed by Wrath Unleashed (" + OFF_ID + ")" in b and "Official realm" in b)
check("page: deaths shown where counted", re.search(r">6:40\.0</td><td class=\"n\">2<", b) is not None)
check("page: never a tampered or bad row", "Cheater" not in b and "Blizzard" not in visible(b) and "Evil" not in b)
check("page: no game-brand names", not BRANDS.search(visible(b)), BRANDS.findall(visible(b)))
check("page: no client version numbers", not VERS.search(visible(b)), VERS.findall(visible(b)))
check("page: no money or support links", not re.search(r"(?i)donat|patreon|ko-fi|paypal|tip jar", b))
s, _, b = req("GET", "/wrath/boards?tab=bossrush&p=all&f=frozen-throne&d=normal")
check("page all time: last week's faster run counts", "5:50.0" in b.decode())
s, _, b = req("GET", "/wrath/boards?tab=duels&p=all&b=duel")
b = b.decode()
check("page duels: ratings in order", s == 200 and b.index("Thrall") < b.index("Orion") and ">1700<" in b)
s, _, b = req("GET", "/wrath/boards?tab=duels&p=week&b=arena-1v1")
check("page arena week: wins", "Sylvana" in b.decode())
s, _, b = req("GET", "/wrath/boards?tab=bossrush&q=jai%3Cscript%3E")
check("page: the search keeps letters only", s == 200 and "<script>" not in b.decode() and 'value="jaiscript"' in b.decode())
s, _, b = req("GET", "/wrath/boards?tab=nope&f=../../etc&d=x&realm=zz")
check("page: odd parameters fall back", s == 200 and 'aria-current="page">Boss Rush<' in b.decode())

s, hd, b = req("GET", "/wrath/api/boards/index.json")
idx = json.loads(b)
check("index: the official realm, its url and key, the local board", s == 200 and [x["realm"] for x in idx["official"]] == [OFF_ID]
      and idx["official"][0]["url"] == "/wrath/api/boards/" + OFF_ID + ".json" and idx["official"][0]["pubkey"] == official["doc"]["pubkey"]
      and idx["community"] == [] and idx["local"] == "/wrath/api/boards/local.json" and hd.get("Access-Control-Allow-Origin") == "*")
s, hd, b = req("GET", "/wrath/api/boards/" + OFF_ID + ".json")
check("board: byte for byte, its own type, CORS", s == 200 and b == open(os.path.join(DUELS, "boards", OFF_ID + ".json"), "rb").read()
      and hd.get("Content-Type", "").startswith("application/vnd.leyline.board+json") and hd.get("Access-Control-Allow-Origin") == "*")
check("board: checks after the trip", ex.check_board(json.loads(b), NOW) == (True, "ok"))
check("board: unknown or odd ids are 404", req("GET", "/wrath/api/boards/" + bad["doc"]["realm"] + ".json")[0] == 404
      and req("GET", "/wrath/api/boards/..%2f..%2fetc.json")[0] == 404)

# --- a community realm sends its board ------------------------------------------------------------------------
com = ex.make_board(sec_com, [res(name="Valeera", ms=330000, run=1)], NOW, "Fröst Håven")
s, _, b = req("POST", "/wrath/api/boards/submit", com, ip="198.51.100.10")
check("submit: a listed realm's signed board is taken", s == 200 and json.loads(b)["url"] == "/wrath/api/boards/" + COM_ID + ".json", b)
check("submit: kept in the portal's data, not with the official boards", os.path.exists(os.path.join(DATA, "boards", "realms", COM_ID + ".json"))
      and not os.path.exists(os.path.join(DUELS, "boards", COM_ID + ".json")))
s, _, b = req("POST", "/wrath/api/boards/submit", com, ip="198.51.100.10")
check("submit: one every four minutes", s == 429, b)
t = copy.deepcopy(com); t["doc"]["results"][0]["r"]["ms"] = 1000
check("submit: a tampered board is refused", req("POST", "/wrath/api/boards/submit", t, ip="198.51.100.11")[0] == 400)
other = ex.make_board(bytes([4] * 32), [res(name="Nobody", ms=1, run=1)], NOW, "Unlisted")
s, _, b = req("POST", "/wrath/api/boards/submit", other, ip="198.51.100.12")
check("submit: a realm not listed on the network is refused", s == 403, b)
check("submit: not JSON or too big is refused", req("POST", "/wrath/api/boards/submit", b"{nope", ip="198.51.100.13")[0] == 400)
s, _, b = req("GET", "/wrath/boards?tab=bossrush&p=all")
b = b.decode()
check("page: the community realm's row, marked as such", "Valeera" in b and "Signed by Fröst Håven (" + COM_ID + ")" in b and "Community realm" in b)
check("page: the realm filter appears with two realms", 'name="realm"' in b)
s, _, b = req("GET", "/wrath/boards?tab=bossrush&p=all&realm=" + COM_ID)
check("page: the realm filter", "Valeera" in b.decode() and "Jaina" not in b.decode())
idx = json.loads(req("GET", "/wrath/api/boards/index.json")[2])
check("index: the community realm", [x["realm"] for x in idx["community"]] == [COM_ID] and idx["community"][0]["name"] == "Fröst Håven")

# --- a local run from the launcher ----------------------------------------------------------------------------
class FakeStore:
    def token_acct(self, tok): return "ORION" if tok == "good" else None
if wp.social is not None:
    wp.social.store = FakeStore()
else:
    wp.social = type("S", (), {"store": FakeStore(), "match": staticmethod(lambda p: False)})()
peer_sec = bytes([9] * 32)
peer_pub = ex.public_key(peer_sec)
def local_run(**k):
    r = {"v": 1, "peer": ex.realm_id(peer_pub), "mode": "bossrush", "board": "bone-lord", "diff": "normal", "engine": "light",
         "name": "Orion", "class": "deathknight", "spec": "blood", "ms": 245000, "at": NOW - 60, **k}
    return {"r": r, "pubkey": peer_pub.hex(), "sig": ex.sign_doc(peer_sec, r, "leyline-board-local-v1")}
s, _, b = req("POST", "/wrath/api/boards/local", local_run(), headers={"Authorization": "Bearer nope"}, ip="198.51.100.20")
check("local: signed out is refused", s == 401, b)
s, _, b = req("POST", "/wrath/api/boards/local", local_run(), headers={"Authorization": "Bearer good"}, ip="198.51.100.20")
check("local: a signed run is taken", s == 200 and json.loads(b)["ok"] is True, b)
s, _, b = req("POST", "/wrath/api/boards/local", local_run(ms=240000), headers={"Authorization": "Bearer good"}, ip="198.51.100.20")
check("local: one every half a minute", s == 429, b)
B.local.last.clear()
t = local_run(); t["r"]["ms"] = 1000
check("local: a changed run is refused", req("POST", "/wrath/api/boards/local", t, headers={"Authorization": "Bearer good"}, ip="198.51.100.21")[0] == 400)
t = local_run(); t["r"]["peer"] = "a" * 16
check("local: a peer id not from the key is refused", req("POST", "/wrath/api/boards/local", t, headers={"Authorization": "Bearer good"}, ip="198.51.100.22")[0] == 400)
check("local: a name with a brand word is refused", req("POST", "/wrath/api/boards/local", local_run(name="Warcraft"), headers={"Authorization": "Bearer good"}, ip="198.51.100.23")[0] == 400)
check("local: a fight not in the catalog is refused", req("POST", "/wrath/api/boards/local", local_run(board="nope"), headers={"Authorization": "Bearer good"}, ip="198.51.100.24")[0] == 400)
check("local: an impossible time is refused", req("POST", "/wrath/api/boards/local", local_run(ms=500), headers={"Authorization": "Bearer good"}, ip="198.51.100.25")[0] == 400)
s, _, b = req("POST", "/wrath/api/boards/local", local_run(ms=260000), headers={"Authorization": "Bearer good"}, ip="198.51.100.26")
check("local: a slower run keeps the best", s == 200 and "faster" in json.loads(b)["message"])
lj = json.loads(req("GET", "/wrath/api/boards/local.json")[2])
check("local.json: labelled, the best run, no account", lj["local"] is True and "Not checked by a server" in lj["note"]
      and [x["ms"] for x in lj["runs"]] == [245000] and "ORION" not in json.dumps(lj) and "who" not in json.dumps(lj))
s, _, b = req("GET", "/wrath/boards?tab=local&f=bone-lord&d=normal")
b = b.decode()
check("page local: the run, marked a local run, not checked", "4:05.0" in b and "Local run" in b and "not checked" in b and "Light version" in b)
s, _, b = req("GET", "/wrath/boards?tab=bossrush&p=all&f=bone-lord")
check("page: a local run never shows on the signed board", "4:05.0" not in b.decode())

# --- deploy wiring --------------------------------------------------------------------------------------------
fl = open(os.path.join(REPO, "stream", "fastloop.sh")).read()
check("fastloop.sh: installs boards.py, backs it up and puts it back",
      'install -m 644 boards.py "$WU/portal/boards.py"' in fl and 'cp -p "$WU/portal/boards.py" "$BK/boards.py"' in fl
      and 'cp -p "$BK/boards.py" "$WU/portal/boards.py"; else rm -f "$WU/portal/boards.py"' in fl and "/wrath/api/boards/index.json" in fl)
bat = open(os.path.join(REPO, "stream", "Update sign-in and warm-up.bat")).read()
check("signin .bat: sends boards.py", "boards.py" in bat.split("sed -i")[-1] and 'set "LINKS=%LINKS% boards.py"' in bat)
sp = open(os.path.join(REPO, "realm", "serverpass.sh")).read()
check("serverpass.sh: step 1h installs the export, its key folder and cron, and runs it",
      "boards-export.py --schema" in sp and "/etc/cron.d/wu-boards" in sp and "install -m 755 \"$T/boards-export.py\" /srv/wrath/boards-export.py" in sp
      and "install -d -m 700 /srv/wrath/leyline" in sp and "ed25519_pure.py" in sp)
rb = open(os.path.join(REPO, "realm", "Server pass - finder 10 and bug reports.bat")).read()
check("realm .bat: sends ov_boards.lua, boards-export.py and ed25519_pure.py", "ov_boards.lua" in rb and "boards-export.py" in rb and "ed25519_pure.py" in rb)
env = open(os.path.join(REPO, "realm", "profiles", "main.env")).read()
check("main.env: OV_BOARDS on, the official fight", re.search(r"^OV_BOARDS=1$", env, re.M) and re.search(r"^OV_BOARDS_FIGHTS=frozen-throne$", env, re.M))

shutil.rmtree(TMP, ignore_errors=True)
print(f"{passed} passed, {failed} failed")
sys.exit(1 if failed else 0)
