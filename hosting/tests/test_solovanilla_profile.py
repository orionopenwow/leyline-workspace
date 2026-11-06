"""The original world, single player (realm/profiles/solovanilla.env; docs/SINGLE-PLAYER-ONE-CLICK.md): Single player's
companions with Vanilla+'s first-era rules, on the player's own PC (the Built-in engine, launcher host/engine.rs
Kind::Vanilla).

    python3 -I hosting/tests/test_solovanilla_profile.py
"""
import os
import re
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PROFILES = os.path.join(REPO, "realm", "profiles")
ENGINE = os.path.join(REPO, "launcher", "src-tauri", "src", "host", "engine.rs")
BANNED = re.compile(r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard)|(?i:(?<![a-z])wow(?![a-z]))")


def env(path):
    out = {}
    for ln in open(path, encoding="utf-8").read().splitlines():
        ln = ln.strip()
        if ln and not ln.startswith("#") and "=" in ln:
            k, v = ln.split("=", 1)
            out[k] = v
    return out


class SoloVanillaProfile(unittest.TestCase):
    def setUp(self):
        self.path = os.path.join(PROFILES, "solovanilla.env")
        self.e = env(self.path)
        self.text = open(self.path, encoding="utf-8").read()
        self.solo = env(os.path.join(PROFILES, "solo.env"))
        self.vplus = env(os.path.join(PROFILES, "vanillaplus.env"))

    def test_the_first_era_exactly_as_vanilla_plus_holds_it(self):
        for k in ("AC_MAX_PLAYER_LEVEL", "AC_INDIVIDUAL_PROGRESSION_ENABLE", "AC_INDIVIDUAL_PROGRESSION_PROGRESSION_LIMIT",
                  "AC_INDIVIDUAL_PROGRESSION_TBC_RACES_UNLOCK_PROGRESSION", "AC_INDIVIDUAL_PROGRESSION_DEATH_KNIGHT_UNLOCK_PROGRESSION",
                  "AC_START_PLAYER_LEVEL", "OV_BOOST_TO_70"):
            self.assertEqual(self.e[k], self.vplus[k], k)
        self.assertEqual(self.e["AC_MAX_PLAYER_LEVEL"], "60")
        self.assertEqual(self.e["AC_INDIVIDUAL_PROGRESSION_PROGRESSION_LIMIT"], "7")
        # never solo.env's "Death Knights from the first character"
        self.assertNotIn("AC_CHARACTER_CREATING_MIN_LEVEL_FOR_HEROIC_CHARACTER", self.e)

    def test_single_players_companions_and_dungeon_buddies(self):
        for k in [k for k in self.solo if k.startswith("OV_COMPANIONS")] + ["AC_PLAYERBOTS_MAX_ADDED_BOTS", "AC_AUTO_BALANCE_ENABLE_GLOBAL",
                                                                            "AC_AUTO_BALANCE_MIN_PLAYERS", "OV_AUTOLEARN", "OV_QOL_BOTHELP"]:
            self.assertEqual(self.e.get(k), self.solo[k], k)
        self.assertEqual(self.e["OV_COMPANIONS"], "1")
        self.assertEqual(self.e["AC_PLAYERBOTS_ENABLED"], "1")
        self.assertEqual(self.e["AC_PLAYERBOTS_RANDOM_BOT_MAX_LEVEL"], "60")
        for k in [k for k in self.e if k.startswith("AC_PLAYERBOTS_LOOT")]:
            self.assertEqual(self.e[k], "0", k)

    def test_no_roaming_population(self):
        self.assertEqual(self.e["WU_BOTS"], "0")
        for k in ("AC_PLAYERBOTS_RANDOM_BOT_AUTOLOGIN", "AC_PLAYERBOTS_MIN_RANDOM_BOTS", "AC_PLAYERBOTS_MAX_RANDOM_BOTS",
                  "AC_PLAYERBOTS_RANDOM_BOT_JOIN_LFG", "AC_PLAYERBOTS_RANDOM_BOT_JOIN_BG", "AC_PRELOAD_ALL_NON_INSTANCED_MAP_GRIDS"):
            self.assertEqual(self.e[k], "0", k)
        for k in ("OV_MAIN", "OV_BRAWL", "OV_DEN", "OV_HOST_INVITE"):
            self.assertEqual(self.e[k], "0", k)

    def test_the_engine_knows_it(self):
        src = open(ENGINE, encoding="utf-8").read()
        self.assertIn('Kind::Vanilla => "solovanilla"', src)
        name = re.search(r'pub const VANILLA_REALM_NAME: &str = "([^"]+)";', src).group(1)
        self.assertEqual(self.e["OV_REALM_NAME"], name, "the realm list's row and the welcome say the same")
        self.assertIn('Kind::Vanilla => "engine-vanilla"', src)

    def test_bot_specs_are_exactly_the_shared_file(self):
        specs = open(os.path.join(REPO, "realm", "playerbots-specs.env"), encoding="utf-8").read()
        self.assertTrue(self.text.endswith(specs))

    def test_names_and_words(self):
        for k in self.e:
            self.assertRegex(k, r"^(AC|OV|WU)_[A-Z0-9_]+$")
        self.assertIsNone(BANNED.search(self.text))
        self.assertIn("TODO(owner)", self.text)
        self.assertNotRegex(self.text, r"\b1\.12|\b3\.3\.5", "no client version numbers in prose")


if __name__ == "__main__":
    unittest.main(verbosity=1)
