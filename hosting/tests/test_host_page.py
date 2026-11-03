"""The launcher's Host page (launcher/src-tauri/src/host/flow.rs, ui/hostflow.js) and the host kit: the page writes
state/mode.json (the world server's settings as "resolved"), state/options.json (realm type, roaming bots, player limit,
who can make an account) and state/rules.txt; setup.py init reads options.json once with the seed's checks
(check_options), then removes it, so Realm tools own those settings afterwards. The relay keeps the mode's credit.

    python3 hosting/tests/test_host_page.py
"""
import importlib
import json
import os
import tempfile
import unittest

from helpers import ROOT  # noqa: F401 (sets sys.path)
from wurelay import listing

# what host/flow.rs apply() writes for the Fun realm with XP moved to 7 (its test checks the same shape)
MODE = {"id": "realm-custom", "name": "Custom (from Fun realm)", "line": "Fast and loud.", "author": "Host", "version": "1",
        "profile": "main", "scripts": [], "env": {}, "mods": ["up:questie"], "tags": [], "settings": [],
        "resolved": {"AC_RATE_XP_KILL": "7", "AC_RATE_XP_QUEST": "7", "OV_MAIN": "1", "OV_START_MOUNT": "1",
                     "AC_START_PLAYER_MONEY": "1000000", "AC_RATE_HONOR": "5", "AC_AUTO_BALANCE_ENABLE_GLOBAL": "1"},
        "host_page": {"template": "fun", "custom": True}}


class HostPage(unittest.TestCase):
    def setUp(self):
        self.kit = tempfile.mkdtemp()
        os.makedirs(os.path.join(self.kit, "state"))
        os.environ["WU_KIT"] = self.kit
        from wuhost import setup as setupmod
        self.s = importlib.reload(setupmod)

    def tearDown(self):
        os.environ.pop("WU_KIT", None)

    def put(self, name, doc):
        with open(os.path.join(self.kit, "state", name), "w") as f:
            f.write(doc if isinstance(doc, str) else json.dumps(doc))

    def read(self, name):
        return open(os.path.join(self.kit, "state", name)).read()

    def init(self, *more):
        self.s.main(["init", "--name", "Fun Town", "--profile", "main", "--visibility", "public", "--gm-name", "HOSTER", *more])
        return json.load(open(os.path.join(self.kit, "state", "host.json")))

    def test_options_are_read_once_then_realm_tools_own_them(self):
        self.put("mode.json", MODE)
        self.put("options.json", {"type": "rp", "bots": 60, "max": 25, "signup": "invite"})
        self.put("rules.txt", "# House rules\nRole-play realm: names fit the world.\nBe kind.\n")
        cfg = self.init()
        self.assertEqual((cfg["bots"], cfg["signup"], cfg["player_limit"], cfg["record"]["type"]), (60, "invite", 25, "RP"))
        env = self.read("realm.env")
        for line in ("AC_RATE_XP_KILL=7", "OV_START_MOUNT=1", "AC_START_PLAYER_MONEY=1000000", "AC_GAME_TYPE=6"):
            self.assertIn(line + "\n", env)
        self.assertLess(env.index("AC_RATE_XP_KILL=7"), env.index("AC_GAME_TYPE=6"), "the mode first, then the options")
        self.assertIn("WU_PLAYER_LIMIT=25", open(os.path.join(self.kit, ".env")).read())
        self.assertIn("WU_BOTS=60", open(os.path.join(self.kit, ".env")).read())
        self.assertEqual(cfg["record"]["rules"], ["Role-play realm: names fit the world.", "Be kind."])
        self.assertFalse(os.path.exists(os.path.join(self.kit, "state", "options.json")), "used once")
        # a later start without the file keeps them; a Realm tools change is not undone
        self.s.main(["signup", "open"])
        cfg = self.init()
        self.assertEqual((cfg["bots"], cfg["signup"], cfg["record"]["type"]), (60, "open", "RP"))
        # an explicit flag still wins over the page's file
        self.put("options.json", {"bots": 10})
        self.assertEqual(self.init("--bots", "5")["bots"], 5)

    def test_a_bad_or_unsuitable_file_is_ignored(self):
        self.put("options.json", "{not json")
        self.assertEqual(self.init()["signup"], "open", "a public realm's own default")
        self.put("options.json", {"bots": 999})
        self.assertEqual(self.init()["bots"], 40, "out of range: the whole file is left out")
        self.s.main(["init", "--profile", "dalaran"])
        self.put("options.json", {"bots": 30, "type": "pvp"})
        cfg = json.load(open(os.path.join(self.kit, "state", "host.json")))
        self.s.main(["init"])
        cfg = json.load(open(os.path.join(self.kit, "state", "host.json")))
        self.assertEqual(cfg["bots"], 0, "the hangout runs no bots (check_options refuses the file)")

    def test_the_relay_keeps_the_credit(self):
        m = listing.clean_mode(MODE)
        self.assertEqual((m["id"], m["name"], m["author"], m["profile"]), ("realm-custom", "Custom (from Fun realm)", "Host", "main"))
        self.assertNotIn("resolved", m)
        self.assertNotIn("host_page", m)


if __name__ == "__main__":
    unittest.main()
