"""The host kit's "solo" profile (realm/profiles/solo.env; owner 2026-10-31 "add solo leveling"): a private realm on the
host's own PC for levelling alone. Helper bots on (bots stay off on the official realm's main.env), dungeons scaled
for one, a modest XP boost, private by default in every flow, and switching a Den to it (and back) on the same kit.
Run: python3 -m pytest hosting/tests/test_solo_profile.py   (or python3 hosting/tests/test_solo_profile.py)"""
import io
import json
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from contextlib import redirect_stdout, redirect_stderr

import helpers  # noqa: F401
from helpers import ROOT

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
KIT = os.path.join(REPO, "hosting", "kit")
SOLO = os.path.join(REPO, "realm", "profiles", "solo.env")
BRANDS = re.compile(r"(?i)world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard")


def env_values(path):
    vals = {}
    for ln in open(path, encoding="utf-8").read().splitlines():
        if ln and not ln.startswith("#") and "=" in ln:
            k, v = ln.split("=", 1)
            vals[k] = v
    return vals


class SoloEnv(unittest.TestCase):
    def test_solo_env(self):
        v = env_values(SOLO)
        for k in v:
            self.assertTrue(re.match(r"^(AC|OV)_[A-Z0-9_]+$", k), k)
        self.assertEqual(v["OV_COMPANIONS"], "1", "helper bots on")
        self.assertEqual(v["AC_PLAYERBOTS_MAX_ADDED_BOTS"], "9", "a 10-player raid: you + 9")
        self.assertLessEqual(int(v["OV_COMPANIONS_PER_PLAYER_RAID"]), int(v["AC_PLAYERBOTS_MAX_ADDED_BOTS"]))
        self.assertEqual((v["AC_AUTO_BALANCE_ENABLE_GLOBAL"], v["AC_AUTO_BALANCE_MIN_PLAYERS"]), ("1", "1"))
        for k in ("AC_RATE_XP_KILL", "AC_RATE_XP_QUEST", "AC_RATE_XP_EXPLORE"):
            casual = env_values(os.path.join(REPO, "realm", "profiles", "casual.env"))[k]
            self.assertTrue(1 < float(v[k]) < float(casual), f"{k}: a modest boost, under casual's {casual}x")
        for k in ("OV_MAIN", "OV_BRAWL", "OV_BOOST_TO_70", "OV_DEN"):
            self.assertEqual(v[k], "0", k)
        self.assertEqual(v["AC_START_PLAYER_LEVEL"], "1")
        self.assertFalse(BRANDS.search(open(SOLO, encoding="utf-8").read()))
        # the bots' talent specs: exactly realm/playerbots-specs.env (the kit loads one profile file only)
        specs = open(os.path.join(REPO, "realm", "playerbots-specs.env"), encoding="utf-8").read()
        mine = "".join(l + "\n" for l in open(SOLO, encoding="utf-8").read().splitlines() if l.startswith("AC_PLAYERBOTS_PREMADE_SPEC_"))
        self.assertEqual(mine, specs)

    def test_the_official_realm_keeps_bots_off(self):
        v = env_values(os.path.join(REPO, "realm", "profiles", "main.env"))
        self.assertEqual((v["OV_COMPANIONS"], v["AC_PLAYERBOTS_MAX_ADDED_BOTS"]), ("0", "0"))

    def test_every_key_the_lua_reads_is_known(self):
        lua = os.path.join(REPO, "realm", "custom", "lua_scripts")
        text = "".join(open(os.path.join(lua, f), encoding="utf-8").read() for f in os.listdir(lua) if f.endswith(".lua"))
        for k in env_values(SOLO):
            if k.startswith("OV_"):
                self.assertIn(k, text, f"{k} is read by a realm script")


class SoloSetup(unittest.TestCase):
    def setUp(self):
        self.kit = tempfile.mkdtemp(prefix="wu-solo-")
        os.makedirs(os.path.join(self.kit, "realm-files", "lua_scripts"))
        os.environ["WU_KIT"] = self.kit
        import importlib
        from wuhost import setup
        self.setup = importlib.reload(setup)

    def tearDown(self):
        shutil.rmtree(self.kit, ignore_errors=True)

    def run_cmd(self, *argv):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            self.setup.main(list(argv))
        return out.getvalue(), err.getvalue()

    def cfg(self):
        return json.load(open(os.path.join(self.kit, "state", "host.json")))

    def test_tables(self):
        s = self.setup
        self.assertIn("solo", s.PROFILES)
        self.assertNotIn("solo", s.NO_BOTS)
        self.assertEqual(s.PROFILE_BOTS["solo"], 20)
        self.assertEqual(s.check_options({"bots": 10}, "solo"), {"bots": 10})
        self.assertTrue(all(t in s.TAGS for t in s.PROFILE_TAGS["solo"]))
        self.assertTrue(s.DEFAULT_RULES["solo"] and s.DEFAULT_TAGLINE["solo"] and s.PROFILE_MAX["solo"] == 5)
        self.assertFalse(BRANDS.search(s.PROFILES["solo"] + s.DEFAULT_TAGLINE["solo"]))

    def test_init_private_with_bots(self):
        self.run_cmd("init", "--name", "Solo leveling", "--profile", "solo")
        c = self.cfg()
        self.assertEqual((c["profile"], c["visibility"], c["signup"], c["bots"]), ("solo", "private", "invite", 20))
        env = open(os.path.join(self.kit, ".env")).read()
        self.assertIn("WU_PROFILE=solo", env)
        self.assertIn("WU_BOTS=20", env)
        self.assertIn("WU_PLAYER_LIMIT=5", env)

    def test_a_den_switches_to_solo_and_back(self):
        """One kit hosts one realm: the launcher switches a Den to Solo leveling (and back) on the same realm, its key,
        invite and accounts kept; each profile's own bots and player cap."""
        self.run_cmd("init", "--name", "My Den", "--profile", "den")
        den = self.cfg()
        self.run_cmd("init", "--name", "Solo leveling", "--profile", "solo", "--visibility", "private")
        solo = self.cfg()
        self.assertEqual((solo["realm_id"], solo["invite_secret"]), (den["realm_id"], den["invite_secret"]))
        self.assertEqual((solo["name"], solo["bots"], solo["visibility"]), ("Solo leveling", 20, "private"))
        env = open(os.path.join(self.kit, ".env")).read()
        self.assertIn("WU_BOTS=20", env)
        self.assertNotIn("WU_DB_BUFFER=256M", env, "the Den's small database goes with the Den")
        self.run_cmd("init", "--name", "My Den", "--profile", "den")
        back = self.cfg()
        self.assertEqual((back["profile"], back["bots"], back["name"]), ("den", 0, "My Den"))
        self.assertIn("WU_BOTS=0", open(os.path.join(self.kit, ".env")).read())


class SoloKitFlows(unittest.TestCase):
    def test_ruleset_lists_name_solo(self):
        ley = open(os.path.join(KIT, "leyline")).read()
        sh = open(os.path.join(KIT, "host-wrath-realm.sh")).read()
        raw = open(os.path.join(KIT, "Host a Wrath realm.bat"), "rb").read()
        self.assertIn("vanillaplus den solo $others", ley)
        self.assertIn('case "$LEYLINE_RULESET" in den|solo) vis_def=private', ley)
        self.assertIn("dalaran vanillaplus den solo $others", sh)
        self.assertIn('solo)  files="main_unbrawl.sql modern_combos.sql"', sh)
        bat = raw.decode()
        self.assertIn('if /i not "%WU_PROFILE%"=="solo"', bat)
        self.assertIn("echo   solo ", bat)
        self.assertIn('if /i "%WU_PROFILE%"=="solo" set "FILES=main_unbrawl.sql modern_combos.sql"', bat)
        self.assertTrue((b"\r\n" in raw and b"\n" not in raw.replace(b"\r\n", b"")) or b"\r" not in raw,
                        "one line-ending style (LF in git, CRLF in the kit zip: tools/leyline-repo/hostkit.py)")
        for f in ("make-kit.sh", "Make host kit.bat"):
            self.assertRegex(open(os.path.join(KIT, f), encoding="utf-8").read(), r"den,? ?solo\b", f)
        self.assertIn('"solo"', open(os.path.join(REPO, "tools", "leyline-repo", "hostkit.py"), encoding="utf-8").read())

    def test_leyline_host_solo_is_private(self):
        tmp = tempfile.mkdtemp(prefix="leyline-solo-")
        try:
            kit = os.path.join(tmp, "kit")
            os.makedirs(kit)
            for f in ("leyline", "host-wrath-realm.sh"):
                shutil.copy(os.path.join(ROOT, "kit", f), kit)
            shutil.copytree(os.path.join(ROOT, "kit", "tools"), os.path.join(kit, "tools"), ignore=shutil.ignore_patterns("__pycache__"))
            env = dict(os.environ, LEYLINE_LOCAL_TOOLS="1", LEYLINE_NONINTERACTIVE="1", LEYLINE_NAME="Solo leveling",
                       LEYLINE_GM="soloone", LEYLINE_RULESET="solo", WU_RELAY="relay.test")
            env.pop("LEYLINE_VISIBILITY", None)
            p = subprocess.run(["bash", os.path.join(kit, "leyline"), "host", "--config-only"], env=env,
                               stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=120)
            self.assertEqual(p.returncode, 0, p.stdout[-800:] + p.stderr[-800:])
            cfg = json.load(open(os.path.join(kit, "state", "host.json")))
            self.assertEqual((cfg["profile"], cfg["visibility"], cfg["bots"]), ("solo", "private", 20))
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
