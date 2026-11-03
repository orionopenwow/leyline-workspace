#!/usr/bin/env python3
"""The one-click client build ("client/Build browser client.bat" -> client/build-browser-client.sh), checked without
building: the dry run's memory, page-file and disk checks stop with plain words; a passing dry run lists every step and
writes nothing; the patch list is the README's order and every file in it exists; the .bat asks nothing; the client step
joins every part the build can make; the Control step points at the .bat.

    python3 -I client/tests/test_build_browser_client.py
"""
import ast
import os
import re
import shutil
import subprocess
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
CLIENT = os.path.dirname(HERE)
ROOT = os.path.dirname(CLIENT)
SCRIPT = os.path.join(CLIENT, "build-browser-client.sh")
BAT = os.path.join(CLIENT, "Build browser client.bat")


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def dry(ram=16, commit=40, auto=1, free=40, extra=()):
    work = tempfile.mkdtemp(prefix="cb-")
    env = dict(os.environ, LEYLINE_TEST_RAM_GB=str(ram), LEYLINE_TEST_COMMIT_GB=str(commit),
               LEYLINE_TEST_PAGEFILE_AUTO=str(auto), LEYLINE_TEST_FREE_GB=str(free),
               LEYLINE_BUILD_ROOT=os.path.join(work, "root"), LEYLINE_PLAY_DIR=os.path.join(work, "play"))
    p = subprocess.run(["bash", SCRIPT, "--dry-run", *extra], env=env, capture_output=True, text=True, timeout=60)
    made = os.listdir(work)
    shutil.rmtree(work)
    return p.returncode, p.stdout + p.stderr, made


class DryRun(unittest.TestCase):
    def test_too_little_memory_stops_first(self):
        rc, out, _ = dry(ram=8)
        self.assertEqual(rc, 1)
        self.assertIn("8 GB of memory; the client build needs 16 GB", out)
        self.assertNotIn("== 2.", out)

    def test_a_fixed_small_page_file_stops_with_the_setting_to_change(self):
        rc, out, _ = dry(commit=20, auto=0)
        self.assertEqual(rc, 1)
        self.assertIn("Automatically manage paging file size", out)

    def test_a_page_file_windows_grows_is_enough(self):
        rc, _, _ = dry(commit=20, auto=1)
        self.assertEqual(rc, 0)

    def test_too_little_disk_stops(self):
        rc, out, _ = dry(free=10)
        self.assertEqual(rc, 1)
        self.assertIn("only 10 GB free", out)

    def test_a_passing_dry_run_lists_every_step_and_writes_nothing(self):
        rc, out, made = dry()
        self.assertEqual(rc, 0, out)
        self.assertEqual(made, [], "a dry run wrote files")
        for want in ("Checks", "Source: benilla-wotlk at b9a49428", "Rust toolchain", "wasm-bindgen", "wasi-sdk 27",
                     "binaryen", "brotli", "Build (release: fat LTO", "Pack", "Into play",
                     "-Z build-std=std,panic_abort", "--release", "--features webgpu",
                     "split -b 9969155 -d -a 2", "wasm-opt -O3", "Dry run done"):
            self.assertIn(want, out)
        self.assertNotIn("Done in", out)

    def test_unknown_options_are_refused(self):
        p = subprocess.run(["bash", SCRIPT, "--frobnicate"], capture_output=True, text=True)
        self.assertEqual(p.returncode, 64)


class Files(unittest.TestCase):
    def test_series_is_the_readme_order_and_every_patch_exists(self):
        lines = [l.strip() for l in read(os.path.join(CLIENT, "patches", "series.txt")).splitlines()
                 if l.strip() and not l.startswith("#")]
        self.assertEqual(lines[0], "wotlk-web-port-wip.patch")
        for l in lines:
            self.assertTrue(os.path.isfile(os.path.join(CLIENT, "patches", l)), l)
        newest = sorted(f for f in os.listdir(os.path.join(CLIENT, "patches")) if f.startswith("wotlk-client-"))
        self.assertEqual(len(newest), 1, "one newest incremental patch at the top; the rest in history/")
        self.assertEqual(lines[-1], newest[0])
        incr = [l for l in lines if "wotlk-client-" in l]
        self.assertEqual(incr, sorted(incr, key=os.path.basename), "incremental patches in name order")
        self.assertIn("series.txt", read(os.path.join(CLIENT, "patches", "README.md")))

    def test_the_bat_asks_nothing_and_runs_the_script(self):
        bat = read(BAT)
        self.assertNotIn("set /p", bat.lower())
        self.assertIn("build-browser-client.sh", bat)
        self.assertIn("/dryrun", bat)
        self.assertIn("pause", bat[bat.index(":fail"):])
        for word in ("World of Warcraft", "Warcraft", "Blizzard", "Lich King"):
            self.assertNotIn(word, bat)
            self.assertNotIn(word, read(SCRIPT))

    def test_the_client_step_joins_every_part_the_build_allows(self):
        bat = read(os.path.join(ROOT, "stream", "Update browser client.bat"))
        for i in range(1, 10):
            self.assertIn(f"dist-test.tgz.part{i:02d}", bat)
        self.assertIn("joins at most 10", read(SCRIPT))

    def test_control_step(self):
        tree = ast.parse(read(os.path.join(ROOT, "tools", "control", "control.py")))
        steps = next(ast.literal_eval(n.value) for n in tree.body if isinstance(n, ast.Assign)
                     and any(getattr(t, "id", "") == "STEPS" for t in n.targets))
        self.assertEqual(steps["client-build"][0], "client/Build browser client.bat")
        self.assertIn('"client-build" set "BAT=client\\Build browser client.bat"', read(os.path.join(ROOT, "UPDATE.bat")))

    def test_shellcheck_clean(self):
        if not shutil.which("shellcheck"):
            self.skipTest("no shellcheck")
        p = subprocess.run(["shellcheck", "-s", "bash", SCRIPT], capture_output=True, text=True)
        self.assertEqual(p.returncode, 0, p.stdout)


if __name__ == "__main__":
    unittest.main()
