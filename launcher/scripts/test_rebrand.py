"""rebrand.py: a fork's branding.json reaches every place that needs it (docs/FORKING.md), and nothing else moves.

    python3 launcher/scripts/test_rebrand.py
"""
import json
import os
import re
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rebrand  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]


def fork_branding():
    with open(ROOT / "launcher" / "branding.json", encoding="utf-8") as f:
        b = json.load(f)
    b.update(name="Starpath", identifier="org.starpath.launcher", repo="starpath-folk/Starpath",
             site="https://play.starpath.example", flags_url="https://play.starpath.example/wrath/leyline/flags.json",
             relays=["https://relay.starpath.example", "https://relay2.starpath.example"])
    b["colours"] = {"bg": "#101010", "text": "#fafafa", "accent": "#ff8800", "accent_strong": "#ffaa33"}
    b["updates"].update(github_api="https://api.github.com/repos/starpath-folk/Starpath/releases?per_page=30",
                        github_downloads="https://github.com/starpath-folk/Starpath/releases/download/",
                        releases_page="https://github.com/starpath-folk/Starpath/releases",
                        mirrors=["https://play.starpath.example/wrath/leyline/releases/"])
    b["release_log"] = {"public_key": "ab" * 32, "urls": ["https://play.starpath.example/wrath/leyline/chain.json"]}
    return b


class Rebrand(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        lau = Path(self.d) / "launcher"
        shutil.copytree(ROOT / "launcher" / "src-tauri" / "src", lau / "src-tauri" / "src")
        shutil.copy(ROOT / "launcher" / "src-tauri" / "tauri.conf.json", lau / "src-tauri" / "tauri.conf.json")
        shutil.copy(ROOT / "launcher" / "branding.json", lau / "branding.json")
        shutil.copytree(ROOT / "launcher" / "ui", lau / "ui", ignore=lambda d, names: [n for n in names
                        if not (os.path.isdir(os.path.join(d, n)) or n.endswith((".js", ".html", ".css")))])
        k = Path(self.d) / "hosting" / "kit" / "tools" / "wunet"
        k.mkdir(parents=True)
        shutil.copy(ROOT / "hosting" / "kit" / "tools" / "wunet" / "release-log.json", k / "release-log.json")

    def tearDown(self):
        shutil.rmtree(self.d, ignore_errors=True)

    def read(self, rel):
        with open(os.path.join(self.d, rel), encoding="utf-8") as f:
            return f.read()

    def test_a_fork_reaches_every_place(self):
        new = fork_branding()
        changed = rebrand.rebrand(self.d, new)
        self.assertIn(os.path.join("launcher", "src-tauri", "src", "update.rs"), changed)
        upd = self.read("launcher/src-tauri/src/update.rs")
        self.assertIn('pub const REPO: &str = "starpath-folk/Starpath";', upd)
        self.assertIn('pub const URL: &str = "https://api.github.com/repos/starpath-folk/Starpath/releases?per_page=30";', upd)
        self.assertIn('pub const DOWNLOAD_PREFIX: &str = "https://github.com/starpath-folk/Starpath/releases/download/";', upd)
        self.assertNotIn('pub const REPO: &str = "orionopenwow/Leyline"', upd)
        mesh = self.read("launcher/src-tauri/src/p2p/mesh.rs")
        self.assertIn('pub const SEEDS: &[&str] = &["https://relay.starpath.example", "https://relay2.starpath.example"];', mesh)
        flags = self.read("launcher/src-tauri/src/p2p/flags.rs")
        self.assertIn('pub const URL: &str = "https://play.starpath.example/wrath/leyline/flags.json";', flags)
        conf = json.loads(self.read("launcher/src-tauri/tauri.conf.json"))
        self.assertEqual((conf["productName"], conf["identifier"]), ("Starpath", "org.starpath.launcher"))
        self.assertEqual(conf["plugins"]["updater"]["pubkey"],
                         json.loads((ROOT / "launcher/src-tauri/tauri.conf.json").read_text())["plugins"]["updater"]["pubkey"],
                         "the updater key is the fork's own job")
        css = self.read("launcher/ui/style.css")
        self.assertRegex(css, r"--t-accent:\s*#ff8800")
        self.assertRegex(css, r"--t-bg:\s*#101010")
        html = self.read("launcher/ui/index.html")
        self.assertNotRegex(html, r"\bLeyline\b")
        self.assertEqual(json.loads(self.read("hosting/kit/tools/wunet/release-log.json"))["public_key"], "ab" * 32)
        self.assertEqual(json.loads(self.read("launcher/branding.json"))["name"], "Starpath")
        # no old address left in the Rust code
        for p in Path(self.d, "launcher/src-tauri/src").rglob("*.rs"):
            t = p.read_text(encoding="utf-8")
            self.assertNotIn("orionopenwow/Leyline", t, p)
        # the names in Rust identifiers are untouched (only addresses move there)
        self.assertIn("leyline_lib", self.read("launcher/src-tauri/src/main.rs") if os.path.exists(os.path.join(self.d, "launcher/src-tauri/src/main.rs")) else "leyline_lib")

    def test_dry_run_changes_nothing_and_bad_branding_is_refused(self):
        before = self.read("launcher/src-tauri/src/update.rs")
        changed = rebrand.rebrand(self.d, fork_branding(), dry=True)
        self.assertGreater(len(changed), 3)
        self.assertEqual(before, self.read("launcher/src-tauri/src/update.rs"))
        for bad in ({"repo": "noslash"}, {"site": "http://plain.example.com"}, {"relays": []}, {"colours": {"bg": "red"}}):
            b = fork_branding()
            b.update(bad)
            with self.assertRaises(rebrand.Refused, msg=bad):
                rebrand.rebrand(self.d, b, dry=True)

    def test_the_same_branding_changes_no_code(self):
        with open(ROOT / "launcher" / "branding.json", encoding="utf-8") as f:
            same = json.load(f)
        changed = rebrand.rebrand(self.d, same, dry=True)
        self.assertEqual(changed, [os.path.join("launcher", "branding.json")])


if __name__ == "__main__":
    unittest.main()
