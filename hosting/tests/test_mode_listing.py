"""Modes anyone can host and fork (docs/MODES.md, "Mode packages") on the network: the relay listing's "mode" field
(hosting/relay/wurelay/listing.py clean_mode: checked, bounded, the credit kept when the recipe is too big), and the
host kit's state/mode.json (hosting/kit/tools/wuhost/setup.py read_mode: its env into state/realm.env before the host's
own options, its credit and recipe on the record, its client mods on the mod list). The patterns match
stream/modepack.py's.

    python3 hosting/tests/test_mode_listing.py
"""
import importlib
import importlib.util
import json
import os
import tempfile
import time
import unittest

from helpers import ROOT  # noqa: F401 (sets sys.path)
from wurelay import listing
from test_network_listing import Host, cfg

REPO = os.path.dirname(ROOT)
spec = importlib.util.spec_from_file_location("mp", os.path.join(REPO, "stream", "modepack.py"))
mp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mp)
DUELS = json.load(open(os.path.join(ROOT, "kit", "modes", "duel-ladder", "mode.json")))


class ModeField(unittest.TestCase):
    def test_a_template_goes_through_whole(self):
        m = listing.clean_mode(DUELS)
        self.assertEqual(m["id"], "duel-ladder")
        self.assertEqual((m["profile"], m["scripts"], m["env"]), ("brawl", ["ov_duels.lua", "ov_qol.lua"],
                                                                   {"OV_DUELS": "1", "OV_DUEL_RESET": "1"}))
        self.assertEqual([s["key"] for s in m["settings"]], ["min_s", "per_day", "win_points", "streaks"])
        self.assertEqual(m, {k: v for k, v in mp.listing_mode(DUELS).items() if v is not None}, "the relay and the site keep the same")

    def test_fork_credit(self):
        f = dict(DUELS, id="my-duels", name="Ann's duels", author="Ann", based_on={"id": "duel-ladder", "author": "Leyline"},
                 jam="first-jam")
        m = listing.clean_mode(f)
        self.assertEqual((m["based_on"], m["jam"], m["author"]), ({"id": "duel-ladder", "author": "Leyline"}, "first-jam", "Ann"))

    def test_bad_parts_dropped(self):
        f = dict(DUELS, scripts=["ov_duels.lua", "../../etc/passwd", "evil.sh"],
                 env={"OV_DUELS": "1", "AC_LOGIN_DATABASE_INFO": "x", "OV_DISCORD_WEBHOOK": "x", "OV_DUELS_K": "$(rm)",
                      "AC_SOAP_ENABLED": "1", "OV_X_TEST": "1"},
                 mods=["mod:ok-mod", "https://evil.example/x.zip"],
                 settings=DUELS["settings"] + [{"key": "bad", "label": "x", "type": "number", "min": 0, "max": 5, "default": 1,
                                                "env": "AC_WORLD_DATABASE_INFO"},
                                               {"key": "dup", "label": "Again", "type": "toggle", "default": True, "env": "OV_DUELS"}])
        m = listing.clean_mode(f)
        self.assertEqual(m["scripts"], ["ov_duels.lua"])
        self.assertEqual(m["env"], {"OV_DUELS": "1"})
        self.assertEqual(m["mods"], ["mod:ok-mod"])
        self.assertEqual([s["key"] for s in m["settings"]], ["min_s", "per_day", "win_points", "streaks"])
        self.assertIsNone(listing.clean_mode({"id": "../x"}))
        self.assertIsNone(listing.clean_mode("duel-ladder"))
        self.assertIsNone(listing.clean_mode({"id": "a"}), "2 to 32 long")

    def test_words(self):
        m = listing.clean_mode(dict(DUELS, name="Warcraft duels", line="The best WoW duels", author="nigger"))
        self.assertEqual(m["name"], "duels")
        self.assertNotIn("author", m)
        self.assertNotRegex(m["line"], "(?i)wow")

    def test_bounded(self):
        big = dict(DUELS, settings=[{"key": f"k{i}", "label": "L" * 40, "help": "H" * 120, "type": "choice",
                                     "options": [{"value": f"v{j}", "label": "O" * 32} for j in range(8)], "default": "v0",
                                     "env": f"OV_K{i}"} for i in range(12)])
        m = listing.clean_mode(big)
        self.assertEqual(set(m), {"id", "name", "author", "version"}, "too big: only the credit")
        self.assertLessEqual(len(json.dumps(listing.clean_mode(DUELS), separators=(",", ":"))), listing.MODE_MAX)
        self.assertEqual(listing.MODE_MAX, mp.LISTING_MAX)

    def test_patterns_match_the_package_checker(self):
        for a, b in ((listing.MODE_ID_RE, mp.ID_RE), (listing.MODE_ENV_RE, mp.ENV_RE), (listing.MODE_ENV_DENY, mp.ENV_DENY),
                     (listing.MODE_VALUE_RE, mp.ENV_VALUE_RE), (listing.MODE_SCRIPT_RE, mp.SCRIPT_RE),
                     (listing.MODE_VERSION_RE, mp.VERSION_RE), (listing.MODE_KEY_RE, mp.KEY_RE)):
            self.assertEqual(a.pattern, b.pattern)
        self.assertEqual(listing.MODE_PROFILES, mp.PROFILES)
        self.assertEqual(listing.MODE_TEMPLATES, mp.ORDER)
        self.assertNotIn("author", listing.clean_mode(dict(DUELS, id="fake-ladder")), "the team's name only on a template")

    def test_heartbeat_to_the_list(self):
        L = listing.Listing(cfg())
        now = time.time()
        h = Host()
        f = dict(DUELS, id="ann-duels", name="Ann duels", author="Ann", based_on={"id": "duel-ladder", "author": "Leyline"})
        st, _ = L.heartbeat(h.beat(now, record={"name": "Frost Haven", "mode": f}), "198.51.100.20", now=now)
        self.assertEqual(st, 200)
        r = L.realms_json(now)["realms"][0]
        self.assertEqual((r["mode"]["id"], r["mode"]["based_on"]["id"], r["mode"]["profile"]), ("ann-duels", "duel-ladder", "brawl"))
        h2 = Host()
        L.heartbeat(h2.beat(now, record={"name": "Plain"}), "198.51.100.21", now=now)
        r2 = next(r for r in L.realms_json(now)["realms"] if r["community"]["realmId"] == h2.rid)
        self.assertIsNone(r2["mode"], "no mode: null")


class KitMode(unittest.TestCase):
    def setUp(self):
        self.kit = tempfile.mkdtemp()
        os.makedirs(os.path.join(self.kit, "state"))
        os.environ["WU_KIT"] = self.kit
        from wuhost import setup as setupmod
        self.s = importlib.reload(setupmod)

    def tearDown(self):
        os.environ.pop("WU_KIT", None)

    def write_mode(self, **kw):
        m = dict(DUELS, resolved=mp.env_for(DUELS, {"min_s": 30}))
        m.update(kw)
        with open(os.path.join(self.kit, "state", "mode.json"), "w") as f:
            json.dump(m, f)

    def test_mode_env_record_and_mods(self):
        self.write_mode(mods=["mode:minigolf", "mod:dalaran-dice"])
        self.s.main(["init", "--name", "Frost Haven", "--profile", "brawl", "--visibility", "public", "--gm-name", "HOSTER"])
        env = open(os.path.join(self.kit, "state", "realm.env")).read()
        self.assertIn("# The mode: Duel ladder", env)
        self.assertIn("OV_DUELS=1\n", env)
        self.assertIn("OV_DUELS_MIN_S=30\n", env)
        self.assertIn("OV_DUELS_STREAKS=1\n", env)
        rec = json.load(open(os.path.join(self.kit, "state", "host.json")))["record"]
        self.assertEqual((rec["mode"]["id"], rec["mode"]["profile"]), ("duel-ladder", "brawl"))
        self.assertNotIn("resolved", rec["mode"], "the host's env stays on the PC; the recipe goes on the list")
        ids = [m["id"] for m in rec["mods"]]
        self.assertIn("mod:dalaran-dice", ids)
        self.assertEqual(ids.count("mode:minigolf"), 1)
        # the host's own options come after the mode's lines (Realm tools wins)
        self.s.main(["options", "--xp", "5"])
        self.s.main(["init"])
        env = open(os.path.join(self.kit, "state", "realm.env")).read()
        self.assertLess(env.index("OV_DUELS=1"), env.index("AC_RATE_XP_KILL=5"))

    def test_another_ruleset_or_bad_env_is_ignored(self):
        self.write_mode(resolved={"OV_DUELS": "1", "AC_LOGIN_DATABASE_INFO": "x;y", "OV_DUELS_K": "1 2", "OV_DEN_SAVES": "/etc"})
        self.s.main(["init", "--name", "Frost Haven", "--profile", "brawl", "--visibility", "public", "--gm-name", "HOSTER"])
        env = open(os.path.join(self.kit, "state", "realm.env")).read()
        self.assertIn("OV_DUELS=1", env)
        for bad in ("DATABASE", "OV_DUELS_K", "DEN_SAVES"):
            self.assertNotIn(bad, env)
        self.s.main(["init", "--profile", "dalaran"])
        env = open(os.path.join(self.kit, "state", "realm.env")).read()
        self.assertNotIn("OV_DUELS", env, "a mode for brawl does nothing on another ruleset")
        self.assertIsNone(json.load(open(os.path.join(self.kit, "state", "host.json")))["record"]["mode"])

    def test_patterns_match(self):
        for a, b in ((self.s.MODE_ID_RE, mp.ID_RE), (self.s.MODE_ENV_RE, mp.ENV_RE), (self.s.MODE_ENV_DENY, mp.ENV_DENY),
                     (self.s.MODE_VALUE_RE, mp.ENV_VALUE_RE), (self.s.MOD_ID_RE, mp.MOD_ID_RE)):
            self.assertEqual(a.pattern, b.pattern)


if __name__ == "__main__":
    unittest.main()
