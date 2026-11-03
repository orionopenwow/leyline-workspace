"""The duel leaderboards: realm/custom/duels-export.py against a fake database (sqlite, the same SQL), then
stream/duels.py and its hooks in wrath_portal.py: names only, bots and opt-outs left out, escaping, the page in the
hub's frame, the JSON, the deploy wiring (sign-in bat, fastloop.sh, the server pass).

    python3 -I stream/tests/test_duels.py stream/wrath_portal.py
"""
import calendar, http.client, importlib.util, json, os, re, shutil, sqlite3, subprocess, sys, tempfile, threading, time
from http.server import ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(sys.argv[1]))
REPO = os.path.dirname(HERE)
TMP = tempfile.mkdtemp(prefix="wu-duels-test-")
OUT = os.path.join(TMP, "duels")
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json",
                  WU_DUELS_DIR=OUT, WU_DUELS_OUT=OUT, WU_ARMORY_DIR=os.path.join(TMP, "armory"), PUBLIC_HOST="play.example",
                  WU_PORTAL_DATA=os.path.join(TMP, "data"))

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, str(extra)[:400])

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
    return m

LUA = os.path.join(REPO, "realm", "custom", "lua_scripts", "ov_duels.lua")
ex = load("duels_export", os.path.join(REPO, "realm", "custom", "duels-export.py"))
NOW = calendar.timegm((2026, 10, 31, 13, 0, 0))   # a Saturday
WK, PREV = ex.week_key(NOW), ex.week_key(NOW - 7 * 86400)

# --- the same tables in the Lua and the export -------------------------------------------------------------
lua = subprocess.run(["lua5.1", "-e", f"""
os.getenv = function() return nil end
function RegisterPlayerEvent() end
dofile({json.dumps(LUA)})
for _, s in ipairs(OV_DUELS_TEST.SCHEMA) do print(s) end
for _, t in ipairs({{ {NOW}, {NOW - 7 * 86400}, 1767225600, 1767139200, 1735603200 }}) do print("WEEK " .. OV_DUELS_TEST.weekKey(t)) end
print("PLACED " .. OV_DUELS_TEST.PLACED_AFTER)
"""], capture_output=True, text=True)
lines = lua.stdout.strip().split("\n")
check("lua: loads with the scores off", lua.returncode == 0, lua.stderr)
check("schema: ov_duels.lua and duels-export.py make the same three tables", [l for l in lines if l.startswith("CREATE")] == list(ex.SCHEMA.values()))
check("schema: --schema prints them for the server pass", ex.schema_sql() == "".join(q + ";\n" for q in ex.SCHEMA.values())
      and all(q.startswith("CREATE TABLE IF NOT EXISTS ov_duel_") for q in ex.SCHEMA.values()))
check("week keys: the Lua and the export agree (UTC, Monday weeks, year ends)",
      [l[5:] for l in lines if l.startswith("WEEK ")] == [ex.week_key(t) for t in (NOW, NOW - 7 * 86400, 1767225600, 1767139200, 1735603200)])
check("placed after: the same in both", f"PLACED {ex.PLACED_AFTER}" in lines)
check("week bounds: Monday to Monday", ex.week_bounds(NOW) == ("2026-10-26", "2026-11-02"), ex.week_bounds(NOW))

# --- fake database ------------------------------------------------------------------------------------------
db = sqlite3.connect(":memory:", check_same_thread=False)
for name in ("acore_auth", "acore_characters", "information_schema"):
    db.execute(f"ATTACH DATABASE ':memory:' AS {name}")
db.executescript(f"""
CREATE TABLE information_schema.TABLES (TABLE_SCHEMA TEXT, TABLE_NAME TEXT);
CREATE TABLE acore_auth.account (id INTEGER PRIMARY KEY, username TEXT, last_ip TEXT);
CREATE TABLE acore_characters.characters (guid INTEGER PRIMARY KEY, account INT, name TEXT, race INT, class INT, deleteDate INT);
INSERT INTO acore_auth.account VALUES (1,'SECRETACCT','203.0.113.77'),(2,'RNDBOT0042',''),(3,'OTHERACCT','');
INSERT INTO acore_characters.characters VALUES
 (101,1,'Arthas',1,2,NULL),(102,3,'Jaina',1,8,NULL),(103,2,'Botty',10,8,NULL),(104,3,'Gonechar',3,1,1761000000),
 (105,3,'Hiddenone',2,1,NULL),(106,3,'Bad Name',1,1,NULL),(107,3,'Thrall',2,7,NULL),(108,3,'Newbie',2,7,NULL),
 (109,3,'Blizzard',1,8,NULL);
""")
ran = []
def runner(sql):
    ran.append(sql)
    out = []
    for stmt in [s.strip() for s in sql.split(";\n") if s.strip()]:
        for r in db.execute(stmt.rstrip(";")):
            out.append("\t".join("NULL" if v is None else str(v) for v in r))
    return "\n".join(out) + "\n"

# no tables yet: an empty board, and nothing but information_schema read
doc = ex.run(runner=runner, out=OUT, now=NOW)
check("export, no tables yet: empty boards, no error", doc["weekly"] == [] and doc["alltime"] == [] and doc["rated_this_week"] == 0)
check("export, no tables yet: only information_schema was asked", len(ran) == 1 and "information_schema.TABLES" in ran[0])

def lite(sql):   # the MySQL CREATE text, as far as sqlite needs it
    sql = re.sub(r",\s*KEY \w+ \([^)]*\)", "", sql)
    sql = sql.replace("id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY", "id INTEGER PRIMARY KEY")
    return sql.replace("AUTO_INCREMENT", "").replace("UNSIGNED", "").replace("CREATE TABLE IF NOT EXISTS ", "CREATE TABLE acore_characters.")
for t, sql in ex.SCHEMA.items():
    db.execute(lite(sql))
    db.execute("INSERT INTO information_schema.TABLES VALUES ('acore_characters', ?)", (t,))
db.executescript(f"""
INSERT INTO acore_characters.ov_duel_stats (guid, name, class, rating, best, wins, losses, points, streak, best_streak) VALUES
 (101,'OldName',2,1620,1650,9,3,30,2,6),(102,'Jaina',8,1580,1590,5,1,16,4,4),(103,'Botty',8,1900,1900,40,0,120,40,40),
 (104,'Gonechar',1,1700,1700,9,0,27,9,9),(105,'Hiddenone',1,1800,1800,9,0,27,9,9),(106,'Bad Name',1,1700,1700,9,0,27,0,1),
 (107,'Thrall',7,1490,1510,2,4,10,0,1),(108,'Newbie',7,1516,1516,1,0,3,1,1),(109,'Blizzard',8,1600,1600,5,0,15,5,5);
INSERT INTO acore_characters.ov_duel_week VALUES
 ('{WK}',101,'OldName',2,12,4,0),('{WK}',102,'Jaina',8,15,4,3),('{WK}',103,'Botty',8,99,33,0),('{WK}',105,'Hiddenone',1,50,10,0),
 ('{WK}',108,'Newbie',7,3,1,0),('{WK}',107,'Thrall',7,0,0,0),('{PREV}',107,'Thrall',7,21,7,0),('{PREV}',102,'Jaina',8,9,3,0),
 ('2026-01',101,'OldName',2,500,100,0);
INSERT INTO acore_characters.ov_duel_log (at, day, winner, loser, pair, kind, secs, rated, delta) VALUES
 ({NOW},'2026-10-30',101,102,'101:102',1,40,1,16),({NOW},'2026-10-26',102,101,'101:102',1,40,1,16),
 ({NOW},'2026-10-25',102,101,'101:102',1,40,1,16),({NOW},'2026-10-30',102,101,'101:102',1,5,0,0);
""")
# no ov_armory_privacy yet: the export must not read it (and must still run)
ran.clear()
doc = ex.run(runner=runner, out=OUT, now=NOW)
check("export without the armory's privacy table: runs, never names it", "ov_armory_privacy" not in "".join(ran[1:]) and len(doc["alltime"]) > 0)
db.executescript("CREATE TABLE acore_characters.ov_armory_privacy (guid INT PRIMARY KEY, hidden INT, changed_at INT);"
                 "INSERT INTO acore_characters.ov_armory_privacy VALUES (105,1,0),(102,0,0);"
                 "INSERT INTO information_schema.TABLES VALUES ('acore_characters','ov_armory_privacy');")
ran.clear()
doc = ex.run(runner=runner, out=OUT, now=NOW)
saved = json.load(open(os.path.join(OUT, "leaderboards.json")))
names = [r["name"] for r in saved["alltime"]]
check("export: the file is what run() returned", saved == doc)
check("export: all time by rating, placed after 3 rated duels", names[0] == "Arthas" and names.index("Jaina") > 0 and "Thrall" in names and "Newbie" not in names, names)
check("export: names from the characters table (renames show)", "OldName" not in json.dumps(saved))
check("export: bots, deleted, opted-out and odd names left out", not ({"Botty", "Gonechar", "Hiddenone", "Bad Name"} & set(names)))
check("export: this week by points, zero points left out", [r["name"] for r in saved["weekly"]] == ["Jaina", "Arthas", "Newbie"], saved["weekly"])
check("export: last week's podium", [r["name"] for r in saved["last_week"]] == ["Thrall", "Jaina"])
check("export: rated duels this week (from Monday, rated only)", saved["rated_this_week"] == 2, saved["rated_this_week"])
check("export: week and its dates", saved["week"] == WK and saved["week_start"] == "2026-10-26" and saved["week_end"] == "2026-11-02")
blob = json.dumps(saved)
for bait in ("SECRETACCT", "203.0.113.77", "101", "guid", "account"):
    check(f"export: never carries {bait!r}", f'"{bait}"' not in blob and f":{bait}," not in blob and bait not in [k for r in saved["alltime"] for k in r])
check("export: only fixed SELECTs ran", all(not re.search(r"(?i)\b(insert|update|delete|replace|drop|create|alter)\b", s) for s in ran))
check("export: the real runner uses a read-only session", "SET SESSION TRANSACTION READ ONLY" in open(ex.__file__).read())
try:
    ex.parse_batch("#Q alltime\n", ["alltime", "weekly"]); cut = False
except RuntimeError:
    cut = True
check("export: a cut-off answer is an error (nothing written)", cut)

# --- the portal module -----------------------------------------------------------------------------------------
wp = load("wp", sys.argv[1])
wp.realm_status = lambda: {"online": True, "players": 4, "bots": 0}
D = wp.duels
check("duels.py loaded beside the portal", D is not None and D.match("/wrath/leaderboards") and D.match("/wrath/api/leaderboards")
      and not D.match("/wrath/leaderboards/x"))
D.STAT_EVERY = 0
D.store.root = OUT
# planted by hand: bait fields, an escaped name, a brand word, a duplicate
plant = dict(saved)
plant["alltime"] = saved["alltime"] + [dict(saved["alltime"][0], name="<script>"), dict(saved["alltime"][0], name="Blizzard"),
                                       dict(saved["alltime"][0], name="arthas", rating=9999)]
plant["alltime"][0] = dict(plant["alltime"][0], guid=101, account="SECRETACCT", ip="203.0.113.77")
plant["weekly"] = saved["weekly"] + [{"name": "Weird", "class": 99, "points": "lots", "wins": -4, "losses": True}]
plant["secret"] = "SECRETACCT"
json.dump(plant, open(os.path.join(OUT, "leaderboards.json"), "w"))
os.utime(os.path.join(OUT, "leaderboards.json"), (time.time() + 5, time.time() + 5))
d = D.store.doc()
check("clean: only the listed fields", set(d["alltime"][0]) == {"name", "class", "rating", "best", "wins", "losses", "points", "best_streak", "streak"})
check("clean: bad names, brand words and duplicates dropped", [r["name"] for r in d["alltime"]].count("Arthas") == 1
      and "<script>" not in json.dumps(d) and "Blizzard" not in json.dumps(d) and "arthas" not in [r["name"] for r in d["alltime"]])
w = [r for r in d["weekly"] if r["name"] == "Weird"][0]
check("clean: odd numbers become 0, an unknown class 0", w["points"] == 0 and w["wins"] == 0 and w["losses"] == 0 and w["class"] == 0)
api = D.api_doc()
ab = json.dumps(api)
check("api: boards, by class, the page", api["weekly"][0]["name"] == "Jaina" and api["alltime"][0]["name"] == "Arthas"
      and api["by_class"]["8"][0]["name"] == "Jaina" and api["page"] == "/wrath/leaderboards" and api["classes"]["6"] == "Death Knight")
for bait in ("SECRETACCT", "203.0.113.77", '"guid"', '"account"', '"secret"'):
    check(f"api: never carries {bait}", bait not in ab)
check("store: a missing file is empty boards", D.Store(os.path.join(TMP, "none")).doc()["alltime"] == [])
with open(os.path.join(TMP, "broken.json"), "w") as f:
    f.write("{not json")
bad = D.Store(TMP); bad_doc = bad.doc()
check("store: a broken file is empty boards, not an error", bad_doc["weekly"] == [])

def visible(b):
    return re.sub(r"<[^>]+>", " ", re.sub(r"<style.*?</style>|<script.*?</script>", " ", b, flags=re.S))
BRANDS = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
VERS = re.compile(r"3\.3\.5|12340|1\.12|2\.4\.3")

# --- through the real handler -------------------------------------------------------------------------------
srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
threading.Thread(target=srv.serve_forever, daemon=True).start()
def get(path, ip="203.0.113.5"):
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=10)
    c.request("GET", path, headers={"X-Forwarded-For": ip}); res = c.getresponse()
    return res.status, dict(res.getheaders()), res.read().decode()
s, hd, b = get("/wrath/leaderboards")
check("GET page: 200, no script", s == 200 and "script-src 'none'" in hd.get("Content-Security-Policy", "") and "<script" not in b, s)
check("page: the hub's frame (header, tokens, footer) with Leaderboards marked", '<header class="hb-top">' in b and "--lk-accent" in b
      and 'class="hb-foot"' in b and re.search(r'class="hb-mi on" href="/wrath/leaderboards" aria-current="page"', b), re.findall(r'class="hb-m?i on"[^>]*', b))
check("page: title and tabs (This week on)", "<title>Leaderboards · This week · Leyline</title>" in b
      and '<a href="/wrath/leaderboards" class="on" aria-current="page">This week</a>' in b
      and 'href="/wrath/leaderboards?board=all"' in b and 'href="/wrath/leaderboards?board=class"' in b)
week = b.split('aria-label="This week&#x27;s duel points"')[1].split("</table>")[0]
check("page: this week, by points", week.index("Jaina") < week.index("Arthas") < week.index("Newbie") and ">15<" in week)
check("page: the week's facts and last week's podium", "2026-10-26" in b and "rated duels this week" in b
      and "Last week: 1st <b>Thrall</b> (21), 2nd <b>Jaina</b> (9)" in b)
check("page: how it counts, commands, the duel corner", ".duel stats" in b and ".hub duels" in b and 'href="/wrath/h/duels"' in b
      and "Two characters on one account" in b)
check("page: updated time", re.search(r'Updated <time datetime="[^"]+">\d\d:\d\d UTC</time>', b))
for bait in ("SECRETACCT", "203.0.113.77", "&lt;script", "Botty", "Hiddenone"):
    check(f"page: never shows {bait!r}", bait not in b)
check("page: no game-brand names", not BRANDS.search(visible(b)), BRANDS.findall(visible(b)))
check("page: no client version numbers", not VERS.search(visible(b)), VERS.findall(visible(b)))
s, _, ba = get("/wrath/leaderboards?board=all")
allt = ba.split('aria-label="All-time duel ratings"')[1].split("</table>")[0]
check("page all: ratings, best, record and streak", s == 200 and allt.index("Arthas") < allt.index("Jaina") and ">1620<" in allt and ">9-3<" in allt
      and 'class="on" aria-current="page">All time<' in ba)
s, _, bc = get("/wrath/leaderboards?board=class")
check("page by class: a card per class", s == 200 and bc.count('class="db-card"') == 10 and "Death Knight" in bc and "Nobody yet." in bc)
s, _, bm = get("/wrath/leaderboards?board=class&class=8")
check("page one class: only that class", s == 200 and 'aria-label="Mage duel ratings"' in bm and "Jaina" in bm
      and "Arthas" not in bm.split('aria-label="Mage duel ratings"')[1].split("</table>")[0])
s, _, bx = get("/wrath/leaderboards?board=%3Cscript%3E&class=%22%3E")
check("page: odd queries are the default board", s == 200 and "<script>" not in bx and 'class="on" aria-current="page">This week<' in bx)
s, hd, bj = get("/wrath/api/leaderboards")
j = json.loads(bj)
check("GET api: 200, JSON, CORS * for the launcher", s == 200 and hd.get("Content-Type", "").startswith("application/json")
      and hd.get("Access-Control-Allow-Origin") == "*" and j["alltime"][0]["name"] == "Arthas")
check("GET api: no account details", "SECRETACCT" not in bj and '"guid"' not in bj)
codes = [get("/wrath/api/leaderboards", ip="198.51.100.31")[0] for _ in range(602)]
check("rate limit (600/min)", codes.count(200) == 600 and codes[-1] == 429, (codes.count(200), codes[-1]))
os.remove(os.path.join(OUT, "leaderboards.json"))
D.store._checked = 0
s, _, be = get("/wrath/leaderboards", ip="198.51.100.40")
check("page with no export yet: the empty board, not an error", s == 200 and "No duel points yet this week" in be)
s, _, be = get("/wrath/leaderboards?board=all", ip="198.51.100.40")
check("all time with no export yet: starts after 3 rated duels", s == 200 and "starts after 3 rated" in be)

# the hub links it: the More menu, Realms and Modes
_, _, hm = get("/wrath/leyline-preview/modes", ip="198.51.100.41")
check("hub modes page links the leaderboards", 'href="/wrath/leaderboards">the leaderboards</a>' in hm)
check("hub header: Leaderboards in More", re.search(r'class="hb-mi" href="/wrath/leaderboards"[^>]*><svg', hm))
_, _, hr = get("/wrath/leyline-preview/realms", ip="198.51.100.41")
check("hub realms page links the leaderboards", hr.count('href="/wrath/leaderboards">the leaderboards</a>') == 1, hr[-3000:])
srv.shutdown()

# --- without the hub: the portal's own Leyline page ---------------------------------------------------------
hubmod = D.P("hub_mod")
D.bind(lambda n: None if n == "hub_mod" else getattr(wp, n))
title, body = D.page_body({})
out, _ = D._shell(title, body)
check("without the hub: the portal's page, still a whole page", out.startswith(b"<!doctype html>") and b"Leaderboards" in out)
D.bind(lambda n: getattr(wp, n))

# --- deploy wiring -----------------------------------------------------------------------------------------------
bat = open(os.path.join(HERE, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
fl = open(os.path.join(HERE, "fastloop.sh"), encoding="utf-8").read()
check("sign-in bat packs duels.py and strips its CRs", 'if exist duels.py set "LINKS=%LINKS% duels.py"' in bat and " duels.py " in bat.split("sed -i")[1])
check("fastloop.sh installs duels.py, backs it up and puts it back",
      'install -m 644 duels.py "$WU/portal/duels.py"' in fl and 'cp -p "$WU/portal/duels.py" "$BK/duels.py"' in fl
      and 'cp -p "$BK/duels.py" "$WU/portal/duels.py"; else rm -f "$WU/portal/duels.py"' in fl)
sp = open(os.path.join(REPO, "realm", "serverpass.sh"), encoding="utf-8").read()
spbat = open(os.path.join(REPO, "realm", "Server pass - finder 10 and bug reports.bat"), encoding="utf-8").read()
check("server pass bat: ov_duels.lua and duels-export.py ride along", 'custom\\lua_scripts\\ov_duels.lua' in spbat
      and 'custom\\duels-export.py' in spbat and "workshop-export.py duels-export.py 2>/dev/null" in spbat)
check("serverpass.sh 1d: tables from --schema, install, a 5-minute cron, one run", "duels-export.py\" --schema |" in sp
      and "install -m 755 \"$T/duels-export.py\" /srv/wrath/duels-export.py" in sp and "/etc/cron.d/wu-duels" in sp
      and "install -d -m 755 /srv/wrath/portal/duels" in sp)
check("serverpass.sh: OV_DUELS=1 in the profile step", re.search(r"for kv in [^;]*\bOV_DUELS=1\b", sp))
env = open(os.path.join(REPO, "realm", "profiles", "main.env"), encoding="utf-8").read()
check("main.env: OV_DUELS=1; other profiles leave it off", re.search(r"^OV_DUELS=1$", env, re.M) and all(
      "OV_DUELS=1" not in open(os.path.join(REPO, "realm", "profiles", f), encoding="utf-8").read()
      for f in os.listdir(os.path.join(REPO, "realm", "profiles")) if f.endswith(".env") and f != "main.env"))

for p in (os.path.join(HERE, "duels.py"), ex.__file__, LUA):
    txt = open(p, encoding="utf-8").read()
    check(f"no game-brand names in {os.path.basename(p)}", not BRANDS.search(txt), BRANDS.findall(txt))

shutil.rmtree(TMP, ignore_errors=True)
print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
