"""The signed boards' export (realm/custom/boards-export.py) against a fake database (sqlite, the same SQL): the record
format, the signatures, the privacy rules, deaths, the arena's week, the featured fight, the launcher's test vector.

    python3 -I stream/tests/test_boards_export.py [stream/wrath_portal.py]
    BOARDS_VECTOR_WRITE=1 ... writes launcher/src-tauri/src/boards-vector.json again (it must not change otherwise)
"""
import calendar, importlib.util, json, os, re, sqlite3, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
TMP = tempfile.mkdtemp(prefix="wu-boards-export-")

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
bf = load("bossfights_export", os.path.join(REPO, "realm", "custom", "bossfights-export.py"))
du = load("duels_export", os.path.join(REPO, "realm", "custom", "duels-export.py"))
LUA = os.path.join(REPO, "realm", "custom", "lua_scripts", "ov_boards.lua")
NOW = calendar.timegm((2026, 11, 12, 19, 0, 0))     # a Thursday; the week began Monday 9 Nov
WK = calendar.timegm((2026, 11, 9, 0, 0, 0))

# --- the same tables, the same catalog, the same week everywhere -------------------------------------------------
lua = None
LUA_SRC = os.path.join(TMP, "probe.lua")
for exe in ("lua5.1", "texlua"):
    try:
        with open(LUA_SRC, "w") as f:
            f.write(f"""
os.getenv = function(k) if k == "OV_BOARDS" then return "1" end return nil end
function RegisterPlayerEvent() end
function RegisterBGEvent() end
function CharDBExecute(s) print(s) end
dofile({json.dumps(LUA)})
for _, f in ipairs(OV_BOARDS_TEST.CATALOG) do print("FIGHT " .. f[1]) end
for _, t in ipairs({{ {NOW}, {WK}, {WK - 1}, 1767225600, 1735603200 }}) do print("WEEK " .. OV_BOARDS_TEST.weekKey(t)) end
""")
        lua = subprocess.run([exe, LUA_SRC], capture_output=True, text=True)
        break
    except FileNotFoundError:
        continue
if lua is None:
    print("SKIP the Lua cross-checks: no lua5.1 or texlua here")
else:
    lines = lua.stdout.strip().split("\n")
    check("lua: loads", lua.returncode == 0, lua.stderr)
    check("schema: ov_boards.lua and boards-export.py make the same tables", [l for l in lines if l.startswith("CREATE")] == list(ex.SCHEMA.values()))
    check("catalog: the Lua's is the export's", [l[6:] for l in lines if l.startswith("FIGHT ")] == list(ex.CATALOG))
    check("week: the Lua and the export agree (Monday, UTC, year ends)",
          [l[5:] for l in lines if l.startswith("WEEK ")] == [ex.day(ex.week_start(t)) for t in (NOW, WK, WK - 1, 1767225600, 1735603200)])

rs = open(os.path.join(REPO, "launcher", "src-tauri", "src", "host", "bossrush.rs"), encoding="utf-8").read()
fights = re.findall(r'Fight \{ key: "([a-z0-9-]+)"', rs.split("pub const FIGHTS")[1].split("];")[0])
# the boards' fights first, in the same order (bossrush.rs BOARD_FIGHTS); after them only start-anywhere bosses (icc-*)
check("catalog: the launcher's FIGHTS in the same order", tuple(fights[:len(ex.CATALOG)]) == ex.CATALOG
      and all(f.startswith("icc-") for f in fights[len(ex.CATALOG):]), fights)
check("catalog: bossrush.rs BOARD_FIGHTS is the catalog's length", f"pub const BOARD_FIGHTS: usize = {len(ex.CATALOG)};" in rs)
check("week: Monday to Monday", ex.day(ex.week_start(NOW)) == "2026-11-09" and ex.week_start(WK) == WK and ex.week_start(WK - 1) == WK - 7 * 86400)
check("week: the same Monday as the boss boards' week", ex.week_start(NOW) == bf.week_bounds(NOW)[0])
check("featured: one fight a week, in turn", ex.featured(NOW) != ex.featured(NOW + 7 * 86400)
      and ex.featured(NOW) == ex.featured(NOW + 12 * 7 * 86400) and ex.featured(NOW, ("frozen-throne",)) == "frozen-throne")
check("featured: the whole week is one fight", ex.featured(WK) == ex.featured(WK + 7 * 86400 - 1) != ex.featured(WK - 1))
check("schema: --schema prints the tables", ex.schema_sql() == "".join(q + ";\n" for q in ex.SCHEMA.values()))

# --- keys --------------------------------------------------------------------------------------------------------
kf = os.path.join(TMP, "k", "board-realm.key")
sec = ex.load_or_create_key(kf)
check("key: made 0600, 32 bytes as hex", oct(os.stat(kf).st_mode & 0o777) == "0o600" and len(open(kf).read().strip()) == 64)
check("key: the same key the next time", ex.load_or_create_key(kf) == sec)
pub = ex.public_key(sec)
check("realm id: base32(sha256(key))[:16], as wunet/keys.py", re.fullmatch(r"[a-z2-7]{16}", ex.realm_id(pub)) is not None)
wk = load("wunet_keys_shim", os.path.join(REPO, "hosting", "kit", "tools", "wunet", "ed25519_pure.py"))
check("pure and fast Ed25519 agree", wk.public_key(sec) == pub)

# --- the fake database -------------------------------------------------------------------------------------------
db = sqlite3.connect(":memory:", check_same_thread=False)
for name in ("acore_auth", "acore_characters", "information_schema"):
    db.execute(f"ATTACH DATABASE ':memory:' AS {name}")
db.executescript("""
CREATE TABLE information_schema.TABLES (TABLE_SCHEMA TEXT, TABLE_NAME TEXT);
CREATE TABLE acore_auth.account (id INTEGER PRIMARY KEY, username TEXT);
CREATE TABLE acore_characters.characters (guid INTEGER PRIMARY KEY, account INTEGER, name TEXT, class INTEGER, deleteDate INTEGER);
CREATE TABLE acore_characters.ov_armory_privacy (guid INTEGER PRIMARY KEY, hidden INTEGER);
CREATE TABLE acore_characters.arena_team (arenaTeamId INTEGER PRIMARY KEY, type INTEGER, rating INTEGER, seasonWins INTEGER,
  seasonGames INTEGER, captainGuid INTEGER);
""")
ran = []
def runner(sql):
    ran.append(sql)
    out = []
    for stmt in [s.strip() for s in sql.split(";\n") if s.strip()]:
        for r in db.execute(stmt.rstrip(";")):
            out.append("\t".join("NULL" if v is None else str(v) for v in r))
    return "\n".join(out) + "\n"

OUT = os.path.join(TMP, "duels")
board = ex.run(runner=runner, out=OUT, now=NOW, key_file=kf, name="Wrath Unleashed")
check("no tables yet: a signed, empty board", board["doc"]["results"] == [] and ex.check_board(board, NOW) == (True, "ok"))
check("no tables yet: only information_schema asked", len(ran) == 1 and "information_schema.TABLES" in ran[0])

def lite(sql):
    sql = re.sub(r",\s*KEY \w+ \([^)]*\)", "", sql)
    sql = sql.replace("id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY", "id INTEGER PRIMARY KEY")
    return sql.replace("AUTO_INCREMENT", "").replace("UNSIGNED", "").replace("CREATE TABLE IF NOT EXISTS ", "CREATE TABLE acore_characters.")
for schema in (ex.SCHEMA, bf.SCHEMA, du.SCHEMA):
    for t, sql in schema.items():
        db.execute(lite(sql))
for t in ex.WANT:
    db.execute("INSERT INTO information_schema.TABLES VALUES ('acore_characters', ?)", (t,))

T = NOW - 3600
db.executescript(f"""
INSERT INTO acore_auth.account VALUES (1,'ORION'),(2,'JAINA'),(3,'RNDBOT7'),(4,'HIDE'),(5,'ALT');
INSERT INTO acore_characters.characters VALUES (11,1,'Orion',6,NULL),(12,2,'Jaina',8,NULL),(13,3,'Botty',5,NULL),
  (14,4,'Secret',2,NULL),(15,5,'Gone',1,{NOW}),(16,2,'Blizzard',8,NULL),(17,1,'Thrall',7,NULL);
INSERT INTO acore_characters.ov_armory_privacy VALUES (14,1);
INSERT INTO acore_characters.leyline_bossfight_runs (id, boss, difficulty, mode, size, ms, started_at, ended_at, leader_guid, instance_id,
  roster, bots, bot_buffs, gm, tier, wipes_before, result) VALUES
 (1,'frozen-throne','normal','bots',10,400000,{T - 400},{T},11,501,'Orion:deathknight:player,Botty:priest:bot',9,0,0,0,0,'clear'),
 (2,'frozen-throne','normal','bots',10,350000,{T - 9 * 86400 - 350},{T - 9 * 86400},11,502,'Orion:deathknight:player',9,0,0,0,0,'clear'),
 (3,'frozen-throne','normal','group',10,500000,{T - 500},{T},12,503,'Jaina:mage:player,Secret:paladin:player,Thrall:shaman:player',0,0,0,0,0,'clear'),
 (4,'frozen-throne','normal','bots',10,100000,{T - 100},{T},12,504,'Jaina:mage:player',9,0,1,0,0,'clear'),
 (5,'frozen-throne','normal','bots',10,90000,{T - 90},{T},12,505,'Jaina:mage:player',9,0,0,0,0,'wipe'),
 (6,'frozen-throne','heroic','bots',10,600000,{T - 600},{T},14,506,'Secret:paladin:player',9,0,0,0,0,'clear'),
 (7,'frozen-throne','heroic','bots',10,610000,{T - 610},{T},15,507,'Gone:warrior:player',9,0,0,0,0,'clear'),
 (8,'bone-lord','normal','bots',10,200000,{T - 200},{T},17,508,'Thrall:shaman:player',9,0,0,0,0,'clear'),
 (9,'frozen-throne','normal','bots',10,450000,{T - 450},{T},16,509,'Blizzard:mage:player',9,0,0,0,0,'clear'),
 (10,'frozen-throne','normal','bots',10,120000,{T - 120},{T},12,510,'Jaina:mage:player',9,0,0,1,0,'clear');
INSERT INTO acore_characters.ov_board_deaths (at, instance_id, map, guid, bot) VALUES
 ({T - 300},501,631,11,0),({T - 200},501,631,13,1),({T - 900},501,631,11,0),({T - 100},503,631,12,0),({T + 100},503,631,12,0);
INSERT INTO acore_characters.ov_duel_stats (guid, name, class, rating, best, wins, losses, points, streak, best_streak) VALUES
 (11,'Orion',6,1640,1650,8,2,26,2,4),(12,'Jaina',8,1580,1590,2,0,6,2,2),(13,'Botty',5,1900,1900,40,0,120,40,40),
 (14,'Secret',2,1700,1700,9,0,27,9,9),(17,'Thrall',7,1520,1530,3,3,12,0,1);
INSERT INTO acore_characters.ov_duel_week VALUES ('{ex.time.strftime("%Y-%W", ex.time.gmtime(NOW))}',11,'Orion',6,9,3,0),
 ('{ex.time.strftime("%Y-%W", ex.time.gmtime(NOW))}',13,'Botty',5,30,10,0),('2026-01',17,'Thrall',7,50,10,0);
INSERT INTO acore_characters.arena_team VALUES (1,1,1810,6,9,12),(2,1,2100,30,30,13),(3,2,2200,10,10,11),(4,1,1700,1,1,11);
INSERT INTO acore_characters.ov_arena_log (at, week, day, bracket, map, secs, winner, winner_name, winner_class, loser,
  loser_name, loser_class) VALUES
 ({T},'2026-11-09','2026-11-12',1,617,60,11,'Orion',6,12,'Jaina',8),({T},'2026-11-09','2026-11-12',1,617,60,11,'Orion',6,12,'Jaina',8),
 ({T},'2026-11-09','2026-11-12',1,617,60,11,'Orion',6,12,'Jaina',8),({T},'2026-11-09','2026-11-12',1,617,60,11,'Orion',6,12,'Jaina',8),
 ({T},'2026-11-09','2026-11-11',1,617,60,12,'Jaina',8,11,'Orion',6),({T},'2026-11-02','2026-11-03',1,617,60,17,'Thrall',7,11,'Orion',6),
 ({T},'2026-11-09','2026-11-12',1,617,60,14,'Secret',2,11,'Orion',6);
""")

ran.clear()
board = ex.run(runner=runner, out=OUT, now=NOW, key_file=kf, name="Wrath Unleashed")
doc = board["doc"]
rid = ex.realm_id(pub)
res = [x["r"] for x in doc["results"]]
check("read only: every batch starts READ ONLY in the real runner", "SET SESSION TRANSACTION READ ONLY" in open(ex.__file__).read())
check("only SELECTs", all(s.strip().upper().startswith("SELECT") for q in ran for s in q.split(";\n") if s.strip()))
check("the board checks", ex.check_board(board, NOW) == (True, "ok"))
check("written to boards/<realm id>.json", json.load(open(os.path.join(OUT, "boards", rid + ".json"))) == board)
check("the document: realm, key, name, week, featured", doc["realm"] == rid and doc["pubkey"] == pub.hex() and doc["name"] == "Wrath Unleashed"
      and doc["week"] == "2026-11-09" and doc["featured"] == {"bossrush": "frozen-throne"} and doc["ts"] == NOW)
check("every result names this realm", all(r["realm"] == rid for r in res))

br = [r for r in res if r["mode"] == "bossrush"]
orion = [r for r in br if r["name"] == "Orion"]
check("boss rush: a leader's all-time best and this week's best (two runs)", sorted(r["run"] for r in orion) == [1, 2], orion)
r1 = next(r for r in br if r["run"] == 1)
check("boss rush: the record's fields", set(r1) == {"v", "realm", "mode", "board", "diff", "kind", "name", "class", "ms", "size",
      "bots", "party", "at", "run", "deaths"} and r1["board"] == "frozen-throne" and r1["diff"] == "normal" and r1["kind"] == "bots"
      and r1["class"] == "deathknight" and r1["ms"] == 400000 and r1["at"] == T, r1)
check("boss rush: deaths in the run's instance between the pull and the end", r1["deaths"] == 2 and next(r for r in br if r["run"] == 3)["deaths"] == 1)
check("boss rush: a party member with .armory off is a null name", [p["name"] for p in next(r for r in br if r["run"] == 3)["party"]] == ["Jaina", None, "Thrall"])
check("boss rush: a group clear is kind group", next(r for r in br if r["run"] == 3)["kind"] == "group")
check("boss rush: GM runs, wipes and tiers never", not {4, 5, 10} & {r["run"] for r in br})
check("boss rush: a hidden, deleted or brand-word leader never", not {6, 7, 9} & {r["run"] for r in br})
check("boss rush: other fights get their own board", any(r["board"] == "bone-lord" and r["run"] == 8 for r in br))
check("boss rush: no account names, guids or instance ids", not any(k in json.dumps(board) for k in ('"guid"', "instance", "ORION", "RNDBOT", "leader_guid")))

dl = [r for r in res if r["mode"] == "duels" and r["board"] == "duel"]
check("duels: all time ratings, placed, no bots or hidden", [(r["name"], r["rating"]) for r in dl if r["period"] == "all"] == [("Orion", 1640), ("Thrall", 1520)], dl)
check("duels: this week's points, no bots, not other weeks", [(r["name"], r["points"], r["period"]) for r in dl if r["period"] != "all"] == [("Orion", 9, "week:2026-11-09")])
check("duels: the record's fields", set(dl[0]) == {"v", "realm", "mode", "board", "period", "name", "class", "rating", "wins", "losses", "points", "at"})
ar = [r for r in res if r["board"] == "arena-1v1"]
check("arena: the 1v1 ratings, captains of 1v1 teams, no bots", [(r["name"], r["rating"], r["wins"], r["losses"]) for r in ar if r["period"] == "all"]
      == [("Jaina", 1810, 6, 3), ("Orion", 1700, 1, 0)], ar)
check("arena: this week's wins, 3 a day against the same one, other weeks and hidden left out",
      [(r["name"], r["wins"], r["losses"]) for r in ar if r["period"] == "week:2026-11-09"] == [("Orion", 3, 2), ("Jaina", 1, 3)], ar)

# --- tampering: every reader's check ------------------------------------------------------------------------------
import copy
t = copy.deepcopy(board); t["doc"]["results"][0]["r"]["ms"] -= 1
check("tamper: a changed time breaks the board", ex.check_board(t, NOW)[0] is False)
t = copy.deepcopy(board); t["doc"]["results"].pop()
check("tamper: a dropped result breaks the board", ex.check_board(t, NOW) == (False, "signature"))
other = ex.load_or_create_key(os.path.join(TMP, "k2"))
fake = ex.make_board(other, [{**res[0]}], NOW, "Wrath Unleashed")
check("a different key is a different realm id", fake["doc"]["realm"] != rid and ex.check_board(fake, NOW) == (True, "ok"))
t = copy.deepcopy(fake); t["doc"]["realm"] = rid
check("tamper: claiming another realm's id fails", ex.check_board(t, NOW) == (False, "realm id"))
t = copy.deepcopy(fake); t["doc"]["pubkey"] = pub.hex(); t["doc"]["realm"] = rid
check("tamper: claiming another realm's key fails", ex.check_board(t, NOW) == (False, "signature"))
check("old boards expire for readers", ex.check_board(board, NOW + 401 * 86400) == (False, "expired"))
mix = copy.deepcopy(board); mix["doc"]["results"].append(fake["doc"]["results"][0])
check("tamper: a result from another realm in this board fails", ex.check_board(mix, NOW)[0] is False)

# a new key: the old board goes
kf2 = os.path.join(TMP, "k3")
b2 = ex.run(runner=runner, out=OUT, now=NOW, key_file=kf2)
check("a new key replaces the folder's board", sorted(os.listdir(os.path.join(OUT, "boards"))) == [b2["doc"]["realm"] + ".json"])

# --- main(): one line, never the secret ---------------------------------------------------------------------------
os.environ["WU_BOARD_KEY"] = kf
ex2 = load("boards_export2", os.path.join(REPO, "realm", "custom", "boards-export.py"))
import io, contextlib
buf = io.StringIO()
with contextlib.redirect_stdout(buf):
    ex2.main(["--whoami"])
check("--whoami: the id and the public key, never the secret", rid in buf.getvalue() and pub.hex() in buf.getvalue() and sec.hex() not in buf.getvalue())
try:
    ex.post(board, "http://example.invalid/")
    check("post: refuses http", False)
except ValueError:
    check("post: refuses http", True)

# --- the launcher's test vector (launcher/src-tauri/src/boards-vector.json) ---------------------------------------
VSEC = bytes([7] * 32)
VNOW = 1794510000
vres = [
    {"v": 1, "realm": "", "mode": "bossrush", "board": "frozen-throne", "diff": "normal", "kind": "bots", "name": "Orion",
     "class": "deathknight", "ms": 312345, "size": 10, "bots": 9, "party": [{"name": "Orion", "class": "deathknight", "bot": False},
     {"name": None, "class": "priest", "bot": True}], "at": VNOW - 600, "run": 42, "deaths": 1},
    {"v": 1, "realm": "", "mode": "bossrush", "board": "frozen-throne", "diff": "normal", "kind": "group", "name": "Jaina",
     "class": "mage", "ms": 298000, "size": 10, "bots": 0, "party": [], "at": VNOW - 9 * 86400, "run": 7},
    {"v": 1, "realm": "", "mode": "duels", "board": "duel", "period": "all", "name": "Thrall", "class": "shaman", "rating": 1712,
     "wins": 20, "losses": 5, "points": 65, "at": VNOW},
    {"v": 1, "realm": "", "mode": "duels", "board": "arena-1v1", "period": "week:2026-11-09", "name": "Sylvana", "class": "hunter",
     "rating": 0, "wins": 3, "losses": 1, "points": 0, "at": VNOW},
]
vec = ex.make_board(VSEC, vres, VNOW, "Fröst Håven \"Q\"", ex.CATALOG)
vpath = os.path.join(REPO, "launcher", "src-tauri", "src", "boards-vector.json")
if os.environ.get("BOARDS_VECTOR_WRITE") == "1":
    with open(vpath, "w", encoding="utf-8") as f:
        json.dump(vec, f, ensure_ascii=False, indent=None, separators=(", ", ": "))
        f.write("\n")
check("vector: the launcher's copy is what this signs (deterministic Ed25519)", os.path.exists(vpath) and json.load(open(vpath, encoding="utf-8")) == vec)
check("vector: it checks", ex.check_board(vec, VNOW) == (True, "ok"))

print(f"{passed} passed, {failed} failed")
sys.exit(1 if failed else 0)
