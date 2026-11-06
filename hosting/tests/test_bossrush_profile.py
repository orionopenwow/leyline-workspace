"""Boss Rush's profile (realm/profiles/bossrush.env; docs/BOSS-RUSH.md): the exact engine's realm on the player's own PC.

    python3 -I hosting/tests/test_bossrush_profile.py
"""
import os
import re
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PROFILE = os.path.join(REPO, "realm", "profiles", "bossrush.env")
LOGIC = os.path.join(REPO, "realm", "custom", "modules", "mod-leyline-bossfights", "src", "BossfightsLogic.h")
BANNED = re.compile(r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard)|(?i:(?<![a-z])wow(?![a-z]))")


def env(path):
    out = {}
    for ln in open(path, encoding="utf-8").read().splitlines():
        ln = ln.strip()
        if ln and not ln.startswith("#") and "=" in ln:
            k, v = ln.split("=", 1)
            out[k] = v
    return out


class BossRushProfile(unittest.TestCase):
    def setUp(self):
        self.e = env(PROFILE)
        self.text = open(PROFILE, encoding="utf-8").read()

    def test_the_whole_catalog_with_bots(self):
        self.assertEqual(self.e["AC_LEYLINE_BOSSFIGHT_ALL_FIGHTS"], "1")
        self.assertEqual(self.e["AC_LEYLINE_BOSSFIGHT_ENABLE"], "1")
        self.assertEqual(self.e["AC_LEYLINE_BOSSFIGHT_BOTS"], "1")
        self.assertEqual(self.e["AC_PLAYERBOTS_ENABLED"], "1")
        self.assertEqual(self.e["AC_PLAYERBOTS_MAX_ADDED_BOTS"], "9")

    def test_nothing_but_the_fights(self):
        self.assertEqual(self.e["WU_BOTS"], "0")
        for k in ("AC_PLAYERBOTS_RANDOM_BOT_AUTOLOGIN", "AC_PLAYERBOTS_MIN_RANDOM_BOTS", "AC_PLAYERBOTS_MAX_RANDOM_BOTS",
                  "AC_PRELOAD_ALL_NON_INSTANCED_MAP_GRIDS", "AC_AUTO_BALANCE_ENABLE_GLOBAL", "AC_WINTERGRASP_KICK_VO_APLAYERS"):
            self.assertEqual(self.e[k], "0", k)
        for k in ("OV_MAIN", "OV_BRAWL", "OV_DEN", "OV_COMPANIONS"):
            self.assertEqual(self.e[k], "0", k)

    def test_no_scaling_on_any_catalog_map(self):
        maps = set(re.findall(r'\{ "[a-z0-9-]+", "[^"]*", "[^"]*", (\d+), ', open(LOGIC, encoding="utf-8").read()))
        self.assertGreaterEqual(len(maps), 6)
        self.assertEqual(set(self.e["AC_AUTO_BALANCE_DISABLE_PER_INSTANCE"].split(",")), maps)

    def test_bot_specs_are_exactly_the_shared_file(self):
        specs = open(os.path.join(REPO, "realm", "playerbots-specs.env"), encoding="utf-8").read()
        self.assertTrue(self.text.endswith(specs))

    def test_names_and_words(self):
        for k in self.e:
            self.assertRegex(k, r"^(AC|OV|WU)_[A-Z0-9_]+$")
        self.assertIsNone(BANNED.search(self.text))
        self.assertIn("TODO(owner)", self.text)


if __name__ == "__main__":
    unittest.main(verbosity=1)
