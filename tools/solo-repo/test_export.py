#!/usr/bin/env python3
"""tools/solo-repo/export.py: the leyline-solo repo's export (docs/LEYLINE-SOLO.md).

    python3 -I tools/solo-repo/test_export.py
"""
import importlib.util
import io
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
_spec = importlib.util.spec_from_file_location("solo_export", HERE / "export.py")
se = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(se)


def quiet(*_a, **_k):
    pass


def git(cwd, *args):
    return subprocess.run(["git", "-C", str(cwd), *args], capture_output=True, text=True, check=True)


class FakeSource:
    """A small git work tree holding exactly what the export reads from the real workspace (plus the MIT LICENSE)."""

    def __init__(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name) / "ws"
        files = se.plan(ROOT)
        rels = set()
        for d in se.DIRS:
            rels |= {r for r in git(ROOT, "ls-files", d).stdout.split("\n") if r and not se.SKIP.search(r)}
        rels |= set(se.FILES) | set(se.WORKFLOWS.values())
        lic = ROOT / "LICENSE" if (ROOT / "LICENSE").is_file() else ROOT / "mods" / "template" / "LICENSE"
        rels.add(lic.relative_to(ROOT).as_posix())
        for r in sorted(rels):
            (self.root / r).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / r, self.root / r)
        git(self.root.parent, "init", "-q", str(self.root))
        git(self.root, "add", "-A")
        assert files

    def write(self, rel, data, track=True):
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data if isinstance(data, bytes) else data.encode())
        if track:
            git(self.root, "add", rel)

    def close(self):
        self.tmp.cleanup()


class SoloExport(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.src = FakeSource()

    @classmethod
    def tearDownClass(cls):
        cls.src.close()

    def setUp(self):
        self.out = tempfile.TemporaryDirectory()
        self.target = Path(self.out.name) / "leyline-solo-clean"

    def tearDown(self):
        self.out.cleanup()

    def run_export(self, src=None, dry=False):
        return se.export(src or self.src.root, self.target, dry=dry, out=quiet)

    def test_the_real_workspace_exports(self):
        r = se.export(ROOT, self.target, out=quiet)
        files = set(r["files"])
        for must in ("README.md", "LICENSE", "NOTICE", ".gitignore", "launcher/src-tauri/Cargo.toml", "launcher/src-tauri/Cargo.lock",
                     "launcher/src-tauri/tauri.solo.conf.json", "launcher/ui-solo/index.html", "launcher/ui-solo/solo.js",
                     "launcher/package-lock.json", "launcher/tests/soloapp.browser.test.cjs", "tools/solo/solo_proof.py",
                     "tools/solo/mpq112.py", "tools/solo/glue/LeylineSolo.lua", "tools/solo/solo.lock",
                     ".github/workflows/leyline-solo.yml", ".github/workflows/solo-engine.yml",
                     "hosting/kit/profiles/cmangos-1.12.1/profile.lock"):
            self.assertIn(must, files)
        # never the launcher's own window, the phone app, the website, Your Den's page tools, the local folder file
        self.assertEqual(sorted(f for f in files if f.startswith("launcher/ui/")),
                         ["launcher/ui/art/leyline-emblem.webp", "launcher/ui/art/leyline-mark.png", "launcher/ui/hostflow-catalog.js"])
        self.assertEqual(sorted(f for f in files if f.startswith("stream/")),
                         ["stream/loadouts.json", "stream/modepacks.json", "stream/playmodes.json"])
        for f in files:
            self.assertFalse(f.startswith(("launcher/src-tauri/mobile/", "launcher/src-tauri/examples/", "labs/", "docs/", "realm/")), f)
            self.assertNotIn("solo_wasm", f)
            self.assertNotEqual(f, "tools/solo/folder.txt")
        self.assertEqual({f.split("/", 1)[0] for f in files} - set(se.GENERATED), {".github", "hosting", "launcher", "stream", "tools"})
        cargo = (self.target / "launcher/src-tauri/Cargo.toml").read_text()
        self.assertIn('default = ["app", "solo"]', cargo)
        self.assertIn('"version": "0.1.0"', (self.target / "launcher/src-tauri/tauri.conf.json").read_text())
        readme = (self.target / "README.md").read_text()
        self.assertIn("TODO(owner)", readme)
        self.assertIn(se.LEGAL, readme)
        self.assertIn(se.LEGAL, (self.target / "NOTICE").read_text())
        self.assertTrue((self.target / "LICENSE").read_bytes().startswith(b"MIT License"))
        # same source in, same bytes out
        again = se.export(ROOT, self.target, out=quiet)
        self.assertEqual((again["written"], again["deleted"]), ([], []))

    def test_only_tracked_files_leave_the_pc(self):
        self.src.write("tools/solo/folder.txt", "D:\\Games\\Mine\n", track=False)
        self.src.write("tools/solo/notes-untracked.txt", "local\n", track=False)
        files = set(self.run_export()["files"])
        self.assertNotIn("tools/solo/folder.txt", files)
        self.assertNotIn("tools/solo/notes-untracked.txt", files)

    def _refused(self, rel, data, words):
        p = self.src.root / rel
        old = p.read_bytes() if p.exists() else None
        self.src.write(rel, data)
        try:
            with self.assertRaises(se.Refused) as cm:
                self.run_export()
            self.assertRegex(str(cm.exception), words)
            self.assertFalse(self.target.exists(), "nothing written")
        finally:
            if old is None:
                p.unlink()
                git(self.src.root, "rm", "-q", "--cached", rel)
            else:
                p.write_bytes(old)
                git(self.src.root, "add", rel)

    def test_brand_words_in_the_window_stop_it(self):
        self._refused("launcher/ui-solo/extra.js", "// Play World of Warcraft here\n", r"brand|World of Warcraft|ui-solo/extra\.js")

    def test_the_legal_line_alone_is_fine(self):
        self.src.write("launcher/ui-solo/legal.js", "const L = 'Leyline is not affiliated with or endorsed by Blizzard Entertainment.';\n")
        try:
            self.assertIn("launcher/ui-solo/legal.js", self.run_export()["files"])
        finally:
            (self.src.root / "launcher/ui-solo/legal.js").unlink()
            git(self.src.root, "rm", "-q", "--cached", "launcher/ui-solo/legal.js")

    def test_game_data_secrets_and_logs_stop_it(self):
        self._refused("tools/solo/conf/terrain.MPQ", b"MPQ\x1a", "game data")
        self._refused("tools/solo/conf/deploy.env", "TOKEN=abc\n", "secret")
        self._refused("tools/solo/conf/key.txt", "-----BEGIN RSA PRIVATE KEY-----\nabc\n", "secret")
        self._refused("tools/solo/run.log", "x\n", "log")

    def test_a_missing_include_stops_it(self):
        self._refused("launcher/src-tauri/src/solo_extra.rs", 'const X: &str = include_str!("../../../stream/other.json");\n', "builds in a file")

    def test_version_numbers_in_the_readme_stop_it(self):
        old = se.README
        try:
            se.README = old + "\nWorks with the 1.12.1 game.\n"
            with self.assertRaises(se.Refused) as cm:
                self.run_export()
            self.assertIn("version number", str(cm.exception))
        finally:
            se.README = old

    def test_stale_files_go_only_inside_its_folders(self):
        self.run_export()
        (self.target / "tools/solo/old.py").write_text("x\n")
        (self.target / "keep-me.txt").write_text("the owner's\n")
        (self.target / ".git").mkdir()
        (self.target / ".git/config").write_text("[core]\n")
        r = self.run_export()
        self.assertEqual(r["deleted"], ["tools/solo/old.py"])
        self.assertTrue((self.target / "keep-me.txt").is_file())
        self.assertTrue((self.target / ".git/config").is_file())

    def test_the_target_is_never_the_workspace(self):
        with self.assertRaises(se.Refused):
            se.export(self.src.root, self.src.root / "launcher", out=quiet)
        r = se.export(self.src.root, self.src.root / ".repos" / "leyline-solo-clean", dry=True, out=quiet)
        self.assertTrue(r["written"])

    def test_the_cargo_and_conf_rewrites(self):
        self.assertIn(b'default = ["app", "solo"]', se.solo_cargo(b'[features]\ndefault = ["app"]\nsolo = ["app"]\n'))
        with self.assertRaises(se.Refused):
            se.solo_cargo(b'[features]\ndefault = ["app"]\n')
        self.assertEqual(se.solo_conf(b'{"version": "0.3.1"}'), b'{"version": "0.1.0"}')

    def test_the_main_prints(self):
        buf = io.StringIO()
        from contextlib import redirect_stderr
        with redirect_stderr(buf):
            self.assertEqual(se.main([]), 2)


class Workflow(unittest.TestCase):
    TEXT = (HERE / "workflows" / "leyline-solo.yml").read_text()

    def test_the_installer_build(self):
        t = self.TEXT
        self.assertIn("--features solo --config src-tauri/tauri.solo.conf.json --bundles nsis", t)
        self.assertIn("RELEASE_TAG: solo-v", t)
        self.assertIn('tags: ["solo-v*"]', t)
        self.assertIn("solo-latest", t)
        self.assertIn("-- --locked", t)
        self.assertIn("npm ci", t)
        # the launcher's signing secret names, never echoed
        for name in ("TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD"):
            self.assertIn("${{ secrets.%s }}" % name, t)
        for line in t.splitlines():
            if "secrets." in line:
                self.assertNotRegex(line, r"echo|cat |printf")

    def test_the_updater_reads_this_repo(self):
        upd = (ROOT / "launcher/src-tauri/src/update.rs").read_text()
        self.assertIn('"https://github.com/orionopenwow/leyline-solo/releases/download/"', upd)
        self.assertIn('pub const TAG_PREFIX: &str = "solo-v";', upd)
        self.assertIn("solo-v", self.TEXT)


if __name__ == "__main__":
    unittest.main()
