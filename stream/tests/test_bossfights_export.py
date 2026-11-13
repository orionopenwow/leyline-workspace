"""Boss fights boards: realm/custom/bossfights-export.py's pure parts and its build() against a fake database, plus the
copies kept in step (the module's CREATE TABLE text, its SQL file, the server pass wiring).

    python3 -I stream/tests/test_bossfights_export.py
"""
import importlib.util, json, os, re, sys, tempfile, time, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
spec = importlib.util.spec_from_file_location("bossfights_export", os.path.join(REPO, "realm", "custom", "bossfights-export.py"))
ex = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ex)
MOD = os.path.join(REPO, "realm", "custom", "modules", "mod-leyline-bossfights")

NOW = 1793700000          # a Thursday
WEEK_START, _ = ex.week_bounds(NOW)


def run(id, ms, leader, roster, bots, ended=NOW - 3600, diff="normal", boss="frozen-throne", buffs=0, size=10):
    return {"id": id, "boss": boss, "difficulty": diff, "mode": "bots" if bots else "group", "size": size, "ms": ms,
            "ended_at": ended, "leader": leader, "roster": roster, "bots": bots, "bot_buffs": buffs}


def players(*names, bots=()):
    parts = [f"{n}:mage:player" for n in names] + [f"{n}:priest:bot" for n in bots]
    return ",".join(parts)


class Pure(unittest.TestCase):
    def test_time_format(self):
        self.assertEqual(ex.fmt_ms(312345), "5:12.3")
        self.assertEqual(ex.fmt_ms(0), "0:00.0")
        self.assertEqual(ex.fmt_ms(3600000), "60:00.0")

    def test_week_bounds_monday(self):
        start, end = ex.week_bounds(NOW)
        self.assertEqual(time.gmtime(start).tm_wday, 0)
        self.assertEqual(end - start, 7 * 86400)
        self.assertTrue(start <= NOW < end)

    def test_names(self):
        self.assertEqual(ex.clean_name("Orion", set()), "Orion")
        for bad in ("", "x", "Orion1", "Abcdefghijklm", "Blizzardfan", "Lichking", "MalWoW", "Wow"):
            self.assertIsNone(ex.clean_name(bad, set()), bad)
        self.assertIsNone(ex.clean_name("Orion", {"orion"}))

    def test_roster(self):
        r = ex.parse_roster("Orion:deathknight:player,Botty:paladin:bot,Bad:wizard:player,junk,Hid:mage:player", {"hid"})
        self.assertEqual([(e["name"], e["class"], e["bot"]) for e in r],
                         [("Orion", "deathknight", False), ("Botty", "paladin", True), ("Bad", "unknown", False), (None, "mage", False)])

    def test_rows_validated(self):
        rows = [["1", "frozen-throne", "normal", "bots", "10", "300000", str(NOW), "Orion", players("Orion", bots=["B"]), "9", "0"],
                ["2", "frozen-throne", "mythic", "bots", "10", "300000", str(NOW), "Orion", "", "9", "0"],
                ["3", "Frozen'Throne", "normal", "bots", "10", "300000", str(NOW), "Orion", "", "9", "0"],
                ["4", "frozen-throne", "normal", "bots", "10", "0", str(NOW), "Orion", "", "9", "0"],
                ["5", "frozen-throne", "heroic"]]
        got = ex.runs_from_rows(rows)
        self.assertEqual([r["id"] for r in got], [1])
        self.assertEqual(got[0]["ms"], 300000)


class Boards(unittest.TestCase):
    def setUp(self):
        ten = ["Aa", "Bb", "Cc", "Dd", "Ee", "Ff", "Gg", "Hh", "Ii", "Jj"]
        self.runs = [
            run(1, 400000, "Aa", players(*ten), 0),
            run(2, 380000, "Bb", players(*reversed(ten)), 0),                # the same ten players: one row, the best
            run(3, 390000, "Kk", players("Kk", *ten[1:]), 0),                # another set of ten
            run(4, 300000, "Orion", players("Orion", bots=["Ba", "Bc"]), 9),
            run(5, 350000, "Orion", players("Orion", bots=["Ba"]), 9),       # slower: not shown
            run(6, 320000, "Lyra", players("Lyra", bots=["Ba"]), 9, ended=WEEK_START - 10),   # last week
            run(7, 200000, "Orion", players("Orion", bots=["Ba"]), 9, diff="heroic", buffs=1),
            run(8, 100000, "Blizzardx", players("Blizzardx"), 9),            # a brand word as the leader: dropped
            run(9, 150000, "Hidden", players("Hidden"), 9),                  # .armory off: dropped as leader
        ]
        self.b = ex.build_boards(self.runs, {"hidden"}, NOW)

    def test_shape(self):
        self.assertEqual(set(self.b), {"frozen-throne"})
        self.assertEqual(set(self.b["frozen-throne"]), {"normal", "heroic"})
        for d in ("normal", "heroic"):
            self.assertEqual(set(self.b["frozen-throne"][d]), {"group", "bots", "weekly"})
        row = self.b["frozen-throne"]["normal"]["bots"][0]
        self.assertEqual(set(row), {"rank", "ms", "time", "date", "leader", "roster", "size", "bots", "bot_buffs"})
        self.assertEqual(set(row["roster"][0]), {"name", "class", "bot"})

    def test_group_board_best_per_set_of_players(self):
        g = self.b["frozen-throne"]["normal"]["group"]
        self.assertEqual([(r["rank"], r["leader"], r["ms"]) for r in g], [(1, "Bb", 380000), (2, "Kk", 390000)])
        self.assertEqual(g[0]["time"], "6:20.0")

    def test_bots_board_best_per_leader(self):
        b = self.b["frozen-throne"]["normal"]["bots"]
        self.assertEqual([(r["leader"], r["ms"]) for r in b], [("Orion", 300000), ("Lyra", 320000)])
        self.assertEqual(b[0]["bots"], 9)
        self.assertTrue(all(r["leader"] not in ("Blizzardx", "Hidden") for r in b))

    def test_weekly(self):
        w = self.b["frozen-throne"]["normal"]["weekly"]
        self.assertEqual([(r["leader"], r["kind"]) for r in w], [("Orion", "bots"), ("Bb", "group"), ("Kk", "group"), ("Aa", "group")])
        self.assertNotIn("Lyra", [r["leader"] for r in w])

    def test_heroic_separate(self):
        h = self.b["frozen-throne"]["heroic"]["bots"]
        self.assertEqual([(r["leader"], r["ms"], r["bot_buffs"]) for r in h], [("Orion", 200000, True)])
        self.assertEqual(self.b["frozen-throne"]["heroic"]["group"], [])

    def test_empty(self):
        e = ex.build_boards([], set(), NOW)
        self.assertEqual(e["frozen-throne"]["normal"], {"group": [], "bots": [], "weekly": []})

    def test_no_account_fields(self):
        text = json.dumps(self.b)
        for k in ("guid", "account", "instance", "\"id\""):
            self.assertNotIn(k, text)


class Arena(unittest.TestCase):
    def test_ladder(self):
        rows = [["Orion", "6", "1650", "30", "40"], ["Lyra", "8", "1700", "10", "12"], ["Orion", "6", "1500", "1", "1"],
                ["Hid", "1", "2000", "5", "5"], ["Bad1", "1", "1900", "1", "1"], ["Short"]]
        got = ex.arena_ladder(rows, {"hid"})
        self.assertEqual(got, [{"name": "Lyra", "class": "mage", "rating": 1700, "wins": 10, "losses": 2},
                               {"name": "Orion", "class": "deathknight", "rating": 1650, "wins": 30, "losses": 10}])


class FakeDb(unittest.TestCase):
    def test_build_and_write(self):
        def runner(sql):
            if "information_schema" in sql:
                return "leyline_bossfight_runs\nov_armory_privacy\narena_team\n"
            self.assertIn("SET SESSION TRANSACTION READ ONLY", "SET SESSION TRANSACTION READ ONLY;\n" + sql)
            self.assertIn("r.gm = 0", sql)
            self.assertIn("r.result = 'clear'", sql)
            self.assertIn("t.type = 1", sql)
            return ("#Q hidden\nSecret\n#Q runs\n"
                    "4\tfrozen-throne\tnormal\tbots\t10\t300000\t%d\tOrion\tOrion:mage:player,Secret:priest:player,Bo:priest:bot\t1\t0\n"
                    "#Q arena\nOrion\t8\t1600\t3\t4\n" % NOW)
        out = tempfile.mkdtemp()
        boards, arena = ex.run(runner, out, NOW)
        self.assertTrue(os.path.exists(os.path.join(out, "bossfights.json")))
        self.assertTrue(os.path.exists(os.path.join(out, "arena.json")))
        saved = json.load(open(os.path.join(out, "bossfights.json")))
        self.assertEqual(saved["updated"], NOW)
        row = saved["bosses"]["frozen-throne"]["normal"]["bots"][0]
        self.assertEqual(row["leader"], "Orion")
        self.assertEqual([e["name"] for e in row["roster"]], ["Orion", None, "Bo"])
        self.assertEqual(json.load(open(os.path.join(out, "arena.json")))["ladder"][0]["losses"], 1)

    def test_missing_tables(self):
        def runner(sql):
            return "" if "information_schema" in sql else ""
        boards, arena = ex.build(runner, NOW)
        self.assertEqual(boards["bosses"]["frozen-throne"]["normal"]["bots"], [])
        self.assertEqual(arena["ladder"], [])


class InStep(unittest.TestCase):
    def test_module_creates_the_same_tables(self):
        src = open(os.path.join(MOD, "src", "Bossfights.cpp"), encoding="utf-8").read()
        stmts = []
        for m in re.finditer(r'DirectExecute\(((?:\s*"(?:[^"\\]|\\.)*")+)\)', src):
            stmts.append("".join(re.findall(r'"((?:[^"\\]|\\.)*)"', m.group(1))))
        creates = [s for s in stmts if s.startswith("CREATE TABLE")]
        self.assertEqual(creates, list(ex.SCHEMA.values()))

    def test_sql_file_has_both_tables(self):
        sql = open(os.path.join(MOD, "data", "sql", "db-characters", "base", "leyline_bossfights.sql"), encoding="utf-8").read()
        for t in ex.SCHEMA:
            self.assertIn(f"CREATE TABLE IF NOT EXISTS `{t}`", sql)
        cols = re.findall(r"^\s+`(\w+)` ", sql.split("leyline_bossfight_instances")[0], re.M)
        want = re.findall(r"(?:\(|, )(\w+) (?:INT|VARCHAR|TINYINT|SMALLINT|TEXT)", ex.SCHEMA["leyline_bossfight_runs"])
        self.assertEqual(cols, want)

    def test_bosses_match_the_module(self):
        h = open(os.path.join(MOD, "src", "BossfightsLogic.h"), encoding="utf-8").read()
        # The official realm offers the module's featured fight only (the first; the rest are Boss Rush's, offered
        # with LeylineBossfight.AllFights on the player's own PC: docs/BOSS-RUSH.md). The boards and ov_go.lua list
        # what the official realm offers.
        keys = tuple(re.findall(r'\{ "([a-z0-9-]+)", "', h))
        self.assertEqual(keys[:len(ex.BOSSES)], ex.BOSSES)
        self.assertEqual(ex.BOSSES, ("frozen-throne",))
        self.assertEqual(len(set(keys)), len(keys))
        go = open(os.path.join(REPO, "realm", "custom", "lua_scripts", "ov_go.lua"), encoding="utf-8").read()
        self.assertEqual(set(re.findall(r'\["([a-z0-9-]+)"\] = "', go.split("local BOSSFIGHTS = ")[1].split("\n")[0])), set(ex.BOSSES))

    def test_no_brand_words_in_player_text(self):
        for path in [os.path.join(MOD, "src", f) for f in ("Bossfights.cpp", "BossfightsLogic.h", "BossfightsStarts.h", "BossfightHeroes.cpp",
                                                           "BossfightHeroes.h", "BossfightsHeroLogic.h", "BossfightsHeroData.h")]:
            src = open(path, encoding="utf-8").read()
            strings = re.findall(r'"((?:[^"\\]|\\.)*)"', src)
            for s in strings:
                self.assertIsNone(ex.BANNED_RE.search(s), (path, s))

    def test_server_pass_wiring(self):
        sp = open(os.path.join(REPO, "realm", "serverpass.sh"), encoding="utf-8").read()
        self.assertIn('python3 -I "$T/bossfights-export.py" --schema |', sp)
        self.assertIn('install -m 755 "$T/bossfights-export.py" /srv/wrath/bossfights-export.py', sp)
        self.assertIn("/etc/cron.d/wu-bossfights", sp)
        bat = open(os.path.join(REPO, "realm", "Server pass - finder 10 and bug reports.bat"), "rb").read().decode("utf-8")
        self.assertIn('custom\\bossfights-export.py', bat)
        self.assertIn("duels-export.py bossfights-export.py 2>/dev/null", bat)


if __name__ == "__main__":
    unittest.main(verbosity=1)
