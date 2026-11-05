"""The GM console's All players tab (fix round 4 Nov, ops review 10): /api/people (accounts, their characters, online,
last seen, bans, mutes, GM notes; bots left out) and the note action. The database is a fake: no docker, no SOAP.

    python3 -I gmconsole/tests/test_people.py
"""
import importlib.util, json, os, stat, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
wu = tempfile.mkdtemp()
with open(os.path.join(wu, "gmconsole.env"), "w") as f:
    f.write("GM_TOKEN=" + "k" * 40 + "\n")
os.environ.update(GM_WU_DIR=wu, GM_DOCKER="/bin/false", GM_BUGS_EVERY="0", GM_AUDIT_FILE=os.path.join(wu, "audit.log"),
                  GM_NOTES_FILE=os.path.join(wu, "notes.json"))
spec = importlib.util.spec_from_file_location("gm_people", os.path.join(HERE, "..", "server", "gmconsole.py"))
gm = importlib.util.module_from_spec(spec); spec.loader.exec_module(gm)
fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra and not cond else ""))
    fails += 0 if cond else 1


asked = []
ROWS = [["7", "ALICE", "2026-11-01 10:00:00", "2026-11-04 20:00:00", "1", "0", "0", None, None, None,
         "Alicia:80:1:1:1:1762280000:4395,Alt:12:8:7:0:1762100000:1", "720000", "203.0.113.5"],
        ["9", "BOB", "2026-10-30 09:00:00", "2026-11-02 08:00:00", "0", str(2 ** 31), "1", "spam in trade", "1762500000",
         "1762300000", None, "0", "0.0.0.0"],
        ["11", "CARL", "2026-10-30 09:00:00", None, "0", "0", "1", "cheating", "1762300000", "1762300000", None, "0", ""],
        ["3"]]


def fake_sql(db, query):
    asked.append((db, query))
    return [list(r) for r in ROWS]


gm.sql = fake_sql
d = gm.api_people({})
a = {x["username"]: x for x in d["accounts"]}
check("three accounts and the total", len(d["accounts"]) == 3 and d["total"] == 3, d)
check("bots are left out in the query", f"NOT LIKE '{gm.BOT_PREFIX}%'" in asked[-1][1])
al = a["ALICE"]
check("characters: name, level, class, race, online, last seen, zone",
      al["characters"][0] == {"name": "Alicia", "level": 80, "class": "Warrior", "race": "Human", "online": True,
                              "last_seen": 1762280000, "zone": gm.zone_name(4395)}, al["characters"][0])
check("online, played hours, last seen", al["online"] and al["played_h"] == 200.0 and al["last_seen"] == 1762280000)
check("the address only as a short hash", len(al["ip_tag"]) == 6 and "203.0.113.5" not in json.dumps(d))
check("banned with a reason and an end; muted", a["BOB"]["banned"] and a["BOB"]["ban_reason"] == "spam in trade"
      and a["BOB"]["ban_until"] == 1762500000 and a["BOB"]["muted"] and a["BOB"]["characters"] == [] and a["BOB"]["ip_tag"] == "")
check("a ban whose end equals its start is permanent", a["CARL"]["banned"] and a["CARL"]["ban_until"] is None)
for q, part in (({"filter": "online"}, "a.online = 1"), ({"filter": "banned"}, "account_banned b WHERE b.id = a.id AND b.active = 1)"),
                ({"filter": "muted"}, "a.mutetime > UNIX_TIMESTAMP()"), ({"filter": "new"}, "INTERVAL 7 DAY"),
                ({"sort": "name"}, "ORDER BY a.username"), ({"page": "2"}, "OFFSET 100"),
                ({"q": "ali_"}, "LIKE 'ALI\\_%'")):
    gm.api_people(q)
    check(f"{q} -> {part}", part in asked[-1][1], asked[-1][1][-300:])
for bad in ({"q": "x' OR 1=1 --"}, {"q": "a" * 40}, {"filter": "everyone; DROP"}, {"sort": "a.id; --"}, {"page": "-1"}, {"page": "x"}):
    try:
        gm.api_people(bad)
        check(f"refused: {bad}", False)
    except gm.Bad:
        check(f"refused: {bad}", True)

# notes: saved through the action (audited), owner-only file, shown on the account, the "noted" filter, removed when empty
r = gm.do_action("note", {"id": 7, "text": "  talked to them about\nthe trade spam \x07 "})
check("note saved", r["ok"] and r["message"] == "note saved", r)
mode = stat.S_IMODE(os.stat(os.environ["GM_NOTES_FILE"]).st_mode)
check("the notes file is owner-only (0600)", mode == 0o600, oct(mode))
d = gm.api_people({})
al = {x["username"]: x for x in d["accounts"]}["ALICE"]
check("the note is on the account, one line, no control characters",
      al["note"] == "talked to them about the trade spam" and al["note_at"] and d["notes"] == 1, al["note"])
gm.api_people({"filter": "noted"})
check("filter noted: the noted ids", "a.id IN (7)" in asked[-1][1])
check("the note action is in the audit log", "note" in open(os.environ["GM_AUDIT_FILE"]).read())
gm.do_action("note", {"id": 7, "text": ""})
check("an empty note removes it", gm.notes_load() == {})
gm.api_people({"filter": "noted"})
check("filter noted with no notes: nothing", "a.id IN (0)" in asked[-1][1])
for bad in ({"id": "x"}, {"id": -3}, {}):
    try:
        gm.do_action("note", dict(bad, text="x"))
        check(f"note refused: {bad}", False)
    except gm.Bad:
        check(f"note refused: {bad}", True)
check("the long note is cut to 500", (gm.do_action("note", {"id": 9, "text": "y" * 900}) and len(gm.notes_load()["9"]["text"]) == 500))
check("the API route is there", gm.GETS.get("/api/people") is gm.api_people and "note" in gm.ACTIONS)
js = open(os.path.join(HERE, "..", "server", "static", "app.js"), encoding="utf-8").read()
check("the page has the All players tab", "['people', 'All players']" in js and "people: () => `/api/people?" in js)
print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
