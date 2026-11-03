"""release_version.py: every ship a stable version one above the last, the floor in tauri.conf.json, no release when
the launcher did not change, and the stamp in the three files the build reads.

    python3 launcher/scripts/test_release_version.py
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import release_version as rv  # noqa: E402

HERE = Path(__file__).resolve().parent
SRC = HERE.parent / "src-tauri"


class Versions(unittest.TestCase):
    def test_next_version(self):
        self.assertEqual(rv.next_version("0.3.1", []), "0.3.1", "the first ship: the file's version")
        self.assertEqual(rv.next_version("0.3.1", ["launcher-v0.3.0"]), "0.3.1")
        self.assertEqual(rv.next_version("0.3.1", ["launcher-v0.3.0", "launcher-v0.3.1"]), "0.3.2", "one above the last")
        self.assertEqual(rv.next_version("0.3.1", ["launcher-v0.3.9", "launcher-v0.3.10"]), "0.3.11", "numbers, not text")
        self.assertEqual(rv.next_version("0.4.0", ["launcher-v0.3.11"]), "0.4.0", "a raised floor wins")
        self.assertEqual(rv.next_version("0.4.0-beta.1", ["launcher-v0.3.11"]), "0.4.0", "ships are always stable")
        # pre-release tags (the early channel), other tags and odd ones do not count
        tags = ["launcher-v0.3.2", "launcher-v0.5.0-beta.1", "launcher-latest", "host-kit", "launcher-vX", "v9.9.9"]
        self.assertEqual(rv.next_version("0.3.1", tags), "0.3.3")
        with self.assertRaises(SystemExit):
            rv.next_version("soon", [])

    def test_plain(self):
        self.assertEqual(rv.plain("1.2.3"), (1, 2, 3))
        self.assertIsNone(rv.plain("1.2.3-beta.1"))
        self.assertIsNone(rv.plain("1.2"))


def git(cwd, *a):
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@example.org", *a], cwd=cwd, check=True, capture_output=True)


@unittest.skipIf(shutil.which("git") is None, "git is not installed")
class Plan(unittest.TestCase):
    def setUp(self):
        self.d = Path(tempfile.mkdtemp())
        (self.d / "launcher" / "src-tauri").mkdir(parents=True)
        (self.d / "stream").mkdir()
        (self.d / "launcher" / "src-tauri" / "tauri.conf.json").write_text(json.dumps({"version": "0.3.1"}))
        (self.d / "stream" / "playmodes.json").write_text("{}")
        (self.d / "README.md").write_text("x")
        git(self.d, "init", "-q", "-b", "main")
        git(self.d, "add", "-A")
        git(self.d, "commit", "-q", "-m", "one")

    def tearDown(self):
        shutil.rmtree(self.d, ignore_errors=True)

    def test_ships_only_when_the_launcher_changed(self):
        self.assertEqual(rv.plan(self.d), {"ship": "true", "version": "0.3.1", "last": ""})
        git(self.d, "tag", "launcher-v0.3.1")
        (self.d / "README.md").write_text("y")
        git(self.d, "commit", "-qam", "docs only")
        self.assertEqual(rv.plan(self.d), {"ship": "false", "version": "0.3.1", "last": "launcher-v0.3.1"})
        (self.d / "launcher" / "a.txt").write_text("new")
        git(self.d, "add", "-A")
        git(self.d, "commit", "-qm", "launcher")
        self.assertEqual(rv.plan(self.d), {"ship": "true", "version": "0.3.2", "last": "launcher-v0.3.1"})
        git(self.d, "tag", "launcher-v0.3.2")
        (self.d / "stream" / "playmodes.json").write_text('{"a":1}')
        git(self.d, "commit", "-qam", "modes it builds in")
        self.assertEqual(rv.plan(self.d)["version"], "0.3.3")
        git(self.d, "tag", "launcher-v0.4.0-beta.1")
        self.assertEqual(rv.plan(self.d)["version"], "0.3.3", "an early-channel tag is not the last ship")

    def test_stamp(self):
        root = self.d
        shutil.copy(SRC / "tauri.conf.json", root / "launcher" / "src-tauri" / "tauri.conf.json")
        shutil.copy(SRC / "Cargo.toml", root / "launcher" / "src-tauri" / "Cargo.toml")
        shutil.copy(SRC / "Cargo.lock", root / "launcher" / "src-tauri" / "Cargo.lock")
        shutil.copy(HERE.parent / "package.json", root / "launcher" / "package.json")
        rv.stamp("0.3.7", root)
        conf = json.loads((root / "launcher" / "src-tauri" / "tauri.conf.json").read_text())
        self.assertEqual(conf["version"], "0.3.7")
        self.assertEqual(conf["plugins"]["updater"], json.loads((SRC / "tauri.conf.json").read_text())["plugins"]["updater"], "nothing else moves")
        cargo = (root / "launcher" / "src-tauri" / "Cargo.toml").read_text()
        self.assertIn('\nversion = "0.3.7"\n', cargo)
        self.assertIn('rust-version = "1.77"', cargo, "only the package's own version line")
        self.assertIn('name = "leyline"\nversion = "0.3.7"', (root / "launcher" / "src-tauri" / "Cargo.lock").read_text())
        self.assertEqual(json.loads((root / "launcher" / "package.json").read_text())["version"], "0.3.7")
        with self.assertRaises(SystemExit):
            rv.stamp("0.4.0-beta.1", root)


if __name__ == "__main__":
    unittest.main()
