#!/usr/bin/env python3
"""tools/boss-rush/boss_rush.py: the read-only check, on a made-up Leyline data folder.

    python3 -I tools/boss-rush/test_boss_rush.py
"""
import io
import json
import os
import re
import sys
import tempfile
import unittest
from contextlib import redirect_stdout

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import boss_rush  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(HERE))


class Check(unittest.TestCase):
    def test_nothing_set_up_is_not_a_failure(self):
        with tempfile.TemporaryDirectory() as d:
            lines = boss_rush.check(d)
            self.assertIn(("NOT YET", "engine files"), [(a, b) for a, b, _ in lines])
            self.assertIn(("NOT YET", "Boss Rush install"), [(a, b) for a, b, _ in lines])
            out = io.StringIO()
            with redirect_stdout(out):
                self.assertEqual(boss_rush.main(["check", "--app", d]), 0)
            self.assertIn("BOSS RUSH CHECK OK", out.getvalue())

    def test_a_set_up_install(self):
        with tempfile.TemporaryDirectory() as d:
            dist = os.path.join(d, "engine", "dist")
            os.makedirs(os.path.join(dist, "realm", "profiles"))
            json.dump({"name": "leyline-engine", "version": "2026.11.06-abc", "map_filter": 1}, open(os.path.join(dist, "engine.json"), "w"))
            open(os.path.join(dist, "realm", "profiles", "bossrush.env"), "w").write("x=1\n")
            st = os.path.join(d, "engine-bossrush", "state")
            os.makedirs(st)
            json.dump({"db_made": True, "filled": True, "account_made": True, "account": "RUSH", "password": "SECRET99", "db_password": "DBSECRET"}, open(os.path.join(st, "state.json"), "w"))
            maps = os.path.join(d, "engine-bossrush", "data", "maps")
            os.makedirs(maps)
            for n in ("6313531.map", "7243132.map", "0004331.map"):
                open(os.path.join(maps, n), "w").close()
            lines = boss_rush.check(d)
            got = {b: (a, c) for a, b, c in lines}
            self.assertEqual(got["engine files"][0], "OK")
            self.assertEqual(got["lighter map read"][0], "OK")
            self.assertEqual(got["Boss Rush install"], ("OK", "3 of 3 set-up steps done, login RUSH"))
            self.assertIn("2 map files of the catalog's maps, 1 others", got["raid maps"][1])
            text = "\n".join(c for _, _, c in lines)
            self.assertNotIn("SECRET99", text)
            self.assertNotIn("DBSECRET", text)

    def test_maps_are_the_launchers(self):
        src = open(os.path.join(ROOT, "launcher", "src-tauri", "src", "host", "bossrush.rs"), encoding="utf-8").read()
        self.assertIn('assert_eq!(maps(), vec![' + ", ".join(str(m) for m in boss_rush.MAPS) + "]);", src)

    def test_bat_files_are_crlf_and_named_in_control(self):
        ctl = open(os.path.join(ROOT, "tools", "control", "control.py"), encoding="utf-8").read()
        for bat, step in (("Boss Rush check.bat", "boss-rush"), ("Boss Rush tests.bat", "boss-rush-test")):
            b = open(os.path.join(HERE, bat), "rb").read()
            self.assertIn(b"\r\n", b)
            self.assertNotRegex(b.decode("utf-8"), r"[^\r]\n")
            self.assertRegex(ctl, re.escape(f'"{step}": ("tools/boss-rush/{bat}"'))


if __name__ == "__main__":
    unittest.main(verbosity=1)
