#!/usr/bin/env python3
"""tools/fork-kit/fork_kit.py: the fork readiness check (docs/TOOLKIT.md). Never touches the network: --online is
tested with a fake fetch.

    python3 -I tools/fork-kit/test_fork_kit.py
"""
import base64
import contextlib
import importlib.util
import io
import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location("fork_kit", HERE / "fork_kit.py")
fk = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fk)

sys.path.insert(0, str(ROOT / "hosting" / "kit" / "tools"))
from wunet import releaselog  # noqa: E402


def minisign_pub(key_id: str) -> str:
    raw = base64.b64encode(b"Ed" + bytes.fromhex(key_id)[::-1] + b"\x07" * 32).decode()
    return base64.b64encode(f"untrusted comment: minisign public key: {key_id}\n{raw}\n".encode()).decode()


def statuses(items, area=None):
    return [i["status"] for i in items if area is None or i["area"] == area]


def texts(items, status):
    return " | ".join(i["text"] for i in items if i["status"] == status)


class Tree:
    """A small copy of the parts of the repo the check reads, for a fork to change."""

    def __init__(self):
        self.root = Path(tempfile.mkdtemp(prefix="fork-kit-"))
        for rel in ("launcher/branding.json", "launcher/scripts/rebrand.py", "launcher/src-tauri/tauri.conf.json",
                    "hosting/kit/tools/wunet/release-log.json", "hosting/kit/tools/wunet/herokey.py"):
            (self.root / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ROOT / rel, self.root / rel)
        b = self.branding()
        for k in ("logo", "emblem"):
            p = self.root / "launcher" / b[k]
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(b"picture")
        ui = self.root / "launcher" / "ui"
        (ui / "index.html").write_text("<title>Launcher</title><p>Play with your own game folder.</p>\n")
        (ui / "app.js").write_text('const SITE = "https://leylinelauncher.com";\n')
        (self.root / "launcher" / "src-tauri" / "src").mkdir(parents=True, exist_ok=True)
        (self.root / "launcher" / "src-tauri" / "src" / "net.rs").write_text('const R: &str = "orionopenwow/Leyline";\n')

    def branding(self):
        return json.loads((self.root / "launcher/branding.json").read_text())

    def set_branding(self, **changes):
        b = self.branding()
        b.update(changes)
        (self.root / "launcher/branding.json").write_text(json.dumps(b, indent=1))
        return b

    def set_updater_key(self, pub):
        p = self.root / "launcher/src-tauri/tauri.conf.json"
        t = json.loads(p.read_text())
        t["plugins"]["updater"]["pubkey"] = pub
        p.write_text(json.dumps(t))

    def fork(self):
        b = self.set_branding(name="Hearthway", identifier="org.hearthway.launcher", repo="hearthway/Hearthway")
        self.set_updater_key(minisign_pub("0123456789ABCDEF"))
        return b

    def run(self, **kw):
        return fk.run(self.root, **kw)

    def close(self):
        shutil.rmtree(self.root, ignore_errors=True)


class OnTheRealRepo(unittest.TestCase):
    def test_leyline_itself_has_no_failures(self):
        items = fk.run(ROOT)
        self.assertNotIn(fk.FAIL, statuses(items), texts(items, fk.FAIL))
        self.assertIn("this is Leyline itself", texts(items, fk.INFO))
        brand = [i for i in items if i["area"] == "brand" and i["status"] == fk.OK]
        self.assertTrue(brand, "the brand check ran")
        self.assertGreater(int(brand[0]["text"].split(" in ")[1].split()[0]), 20, "it read the exported files too")

    def test_the_real_updater_key_is_leylines(self):
        t = json.loads((ROOT / "launcher/src-tauri/tauri.conf.json").read_text())
        self.assertEqual(fk.minisign_key_id(t["plugins"]["updater"]["pubkey"]), fk.UPSTREAM["updater_key_id"])

    def test_main_exit_codes(self):
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(fk.main(["check", "--root", str(ROOT)]), 0)
        self.assertIn("Fork readiness checklist", out.getvalue())
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(fk.main([]), 2)
        with contextlib.redirect_stdout(io.StringIO()) as out:
            fk.main(["check", "--root", str(ROOT), "--json"])
        self.assertTrue(all("status" in i for i in json.loads(out.getvalue())))


class Fork(unittest.TestCase):
    def setUp(self):
        self.t = Tree()

    def tearDown(self):
        self.t.close()

    def test_a_fork_keeping_leylines_identifier_and_updater_key_fails(self):
        self.t.set_branding(name="Hearthway", repo="hearthway/Hearthway")
        items = self.t.run()
        fails = texts(items, fk.FAIL)
        self.assertIn("identifier is still Leyline's", fails)
        self.assertIn("updater key is still Leyline's", fails)
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(fk.main(["check", "--root", str(self.t.root)]), 1)
        self.assertIn("NOT READY", out.getvalue())

    def test_a_proper_fork_passes_identity_and_lists_leftover_addresses(self):
        self.t.fork()
        items = self.t.run()
        ident = [i for i in items if i["area"] == "identity"]
        self.assertNotIn(fk.FAIL, statuses(ident), texts(ident, fk.FAIL))
        self.assertIn("own updater key 0123456789ABCDEF", texts(ident, fk.OK))
        warn = texts(ident, fk.WARN)
        self.assertIn("launcher/ui/app.js (1)", warn)
        self.assertIn("launcher/src-tauri/src/net.rs (1)", warn)
        # rebrand.py done: nothing left
        (self.t.root / "launcher/ui/app.js").write_text('const SITE = "https://hearthway.example.org";\n')
        (self.t.root / "launcher/src-tauri/src/net.rs").write_text('const R: &str = "hearthway/Hearthway";\n')
        t = {"productName": "Hearthway", "plugins": {"updater": {"pubkey": minisign_pub("0123456789ABCDEF")}}}
        (self.t.root / "launcher/src-tauri/tauri.conf.json").write_text(json.dumps(t))
        items = self.t.run()
        self.assertIn("no Leyline addresses left", texts(items, fk.OK))

    def test_invalid_branding_and_missing_pictures_fail(self):
        b = self.t.branding()
        b["colours"]["accent"] = "blue"
        self.t.set_branding(colours=b["colours"], logo="ui/art/nope.png")
        fails = texts(self.t.run(), fk.FAIL)
        self.assertIn("colours.accent must be #rrggbb", fails)
        self.assertIn("logo: launcher/ui/art/nope.png not found", fails)
        (self.t.root / "launcher/branding.json").write_text("{not json")
        self.assertEqual(statuses(self.t.run()), [fk.FAIL])

    def test_a_missing_or_broken_updater_key_fails(self):
        self.t.set_updater_key("")
        self.assertIn("no updater public key", texts(self.t.run(), fk.FAIL))
        self.t.set_updater_key(base64.b64encode(b"hello").decode())
        self.assertIn("not a minisign public key", texts(self.t.run(), fk.FAIL))


class BrandWords(unittest.TestCase):
    def setUp(self):
        self.t = Tree()

    def tearDown(self):
        self.t.close()

    def test_brand_words_in_the_launcher_ui_fail(self):
        ui = self.t.root / "launcher/ui"
        word = "War" + "craft"     # split so this file never trips a brand search itself
        (ui / "guide.html").write_text(f"<p>Bring your {word} folder</p>\n")
        (ui / "nav.js").write_text(f'label("{"Bliz" + "zard"} launcher");\n')
        items = self.t.run()
        fails = texts(items, fk.FAIL)
        self.assertIn("launcher/ui/guide.html:1", fails)
        self.assertIn("launcher/ui/nav.js:1", fails)

    def test_the_legal_disclaimer_is_allowed_and_wow_only_whole_word(self):
        ui = self.t.root / "launcher/ui"
        maker = "Bliz" + "zard"
        (ui / "about.html").write_text(f"<p>Leyline is not affiliated with or endorsed by {maker} Entertainment.</p>\n"
                                       "<p>Wowzers, a power-flow view.</p>\n")
        items = self.t.run()
        self.assertNotIn(fk.FAIL, statuses(items, "brand"), texts(items, fk.FAIL))
        (ui / "about.html").write_text(f"<p>{maker} Entertainment made it.</p>\n")
        self.assertIn("about.html:1", texts(self.t.run(), fk.FAIL))

    def test_extra_paths_are_checked_and_tests_are_skipped(self):
        site = self.t.root / "mysite"
        site.mkdir()
        (site / "index.html").write_text("<h1>Lich" + " King realms</h1>\n")
        tests = self.t.root / "launcher/ui/tests"
        tests.mkdir()
        (tests / "x.js").write_text("// War" + "craft in a test fixture\n")
        items = self.t.run(extra=["mysite"])
        fails = texts(items, fk.FAIL)
        self.assertIn("mysite/index.html:1", fails)
        self.assertNotIn("tests/x.js", fails)
        self.assertIn("--path nowhere not found", texts(self.t.run(extra=["nowhere"]), fk.WARN))

    def test_without_the_export_tool_it_still_checks_with_the_same_words(self):
        items = self.t.run()
        self.assertIn("export.py not found", texts(items, fk.WARN))
        self.assertTrue(any(i["area"] == "brand" and i["status"] == fk.OK for i in items))


class UpdatesAndReleaseLog(unittest.TestCase):
    def setUp(self):
        self.t = Tree()
        self.tmp = Path(tempfile.mkdtemp(prefix="fork-kit-key-"))

    def tearDown(self):
        self.t.close()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_spare_keys_and_mirrors(self):
        b = self.t.branding()
        self.assertIn("no spare updater key", texts(self.t.run(), fk.TODO))
        b["updates"]["backup_pubkeys"] = [minisign_pub("FEDCBA9876543210")]
        b["updates"]["mirrors"] = []
        self.t.set_branding(updates=b["updates"])
        items = self.t.run()
        self.assertIn("1 spare updater key(s): FEDCBA9876543210", texts(items, fk.OK))
        self.assertIn("no mirror", texts(items, fk.WARN))
        b["updates"]["backup_pubkeys"] = ["not-a-key"]
        self.t.set_branding(updates=b["updates"])
        self.assertIn("not minisign public keys", texts(self.t.run(), fk.FAIL))

    def test_release_log_key_must_match_the_kit_and_a_chain_verifies(self):
        self.assertIn("no release-log public key", texts(self.t.run(), fk.TODO))
        keyfile = self.tmp / "release-log.key"
        pub = releaselog.write_key(str(keyfile))
        b = self.t.branding()
        b["release_log"]["public_key"] = pub
        self.t.set_branding(release_log=b["release_log"])
        self.assertIn("host kit's release-log key differs", texts(self.t.run(), fk.FAIL))
        kit = self.t.root / "hosting/kit/tools/wunet/release-log.json"
        kit.write_text(json.dumps({"public_key": pub}))
        items = self.t.run()
        self.assertNotIn(fk.FAIL, statuses(items, "release log"))
        self.assertIn(f"release-log key {pub[:16]}", texts(items, fk.OK))

        secret = releaselog.read_key(str(keyfile))
        art = self.tmp / "Hearthway_1.0.0_x64-setup.exe"
        art.write_bytes(b"installer")
        doc = releaselog.append(releaselog.empty(pub), secret, kind="launcher", name="Hearthway", version="1.0.0",
                                files=[releaselog.file_ref(str(art))], mirrors=["https://mirror.example.org/r/"])
        chain = self.tmp / "chain.json"
        chain.write_text(json.dumps(doc))
        with contextlib.redirect_stdout(io.StringIO()) as out:
            code = fk.main(["check", "--root", str(self.t.root), "--chain", str(chain)])
        self.assertIn("chain.json: 1 entries verify", out.getvalue())
        self.assertEqual(code, 0)
        self.assertNotIn(secret.hex(), out.getvalue(), "never prints the secret")

        doc["entries"][0]["entry"] = doc["entries"][0]["entry"].replace("1.0.0", "9.9.9")
        chain.write_text(json.dumps(doc))
        self.assertIn("does not verify", texts(self.t.run(chain=str(chain)), fk.FAIL))

    def test_a_malformed_release_log_key_fails(self):
        b = self.t.branding()
        b["release_log"]["public_key"] = "ABC"
        self.t.set_branding(release_log=b["release_log"])
        self.assertTrue(any("release_log.public_key" in i["text"] for i in self.t.run() if i["status"] == fk.FAIL))

    def test_relays(self):
        self.t.set_branding(relays=["https://relay.one.example.org"])
        self.assertIn("one seed relay", texts(self.t.run(), fk.WARN))
        self.t.set_branding(relays=["https://a.example.org", "https://b.example.org"])
        self.assertIn("all under example.org", texts(self.t.run(), fk.WARN))
        self.t.set_branding(relays=["https://a.example.org", "https://b.example.net"])
        self.assertIn("2 seed relays on 2 domains", texts(self.t.run(), fk.OK))

    def test_hero_key_pins(self):
        self.assertIn("pins no website key", texts(self.t.run(), fk.INFO))
        p = self.t.root / "hosting/kit/tools/wunet/herokey.py"
        p.write_text(p.read_text().replace("SITE_KEYS: tuple = ()", 'SITE_KEYS: tuple = ("' + "ab" * 32 + '",)'))
        self.assertIn("pins 1 website key", texts(self.t.run(), fk.OK))
        self.t.fork()
        self.assertIn("make sure they are your site's", texts(self.t.run(), fk.WARN))


class Online(unittest.TestCase):
    def setUp(self):
        self.t = Tree()

    def tearDown(self):
        self.t.close()

    def test_every_address_is_asked_through_the_given_fetch(self):
        asked = []

        def fetch(url):
            asked.append(url)
            if "relay.openvanilla" in url:
                raise OSError("down")
            if "chain.json" in url:
                return 404
            if "flags" in url:
                return 500
            return 200
        items = self.t.run(online=True, fetch=fetch)
        b = self.t.branding()
        self.assertIn(b["updates"]["github_api"], asked)
        self.assertIn(b["updates"]["mirrors"][0] + "latest.json", asked)
        self.assertIn(b["relays"][0] + "/healthz", asked)
        self.assertIn(b["flags_url"], asked)
        warn = texts(items, fk.WARN)
        self.assertIn("relay.openvanilla.app/healthz did not answer", warn)
        self.assertIn("flags.json answered 500", warn)
        self.assertIn("expected while there is no release key", texts(items, fk.INFO))

    def test_offline_by_default(self):
        items = self.t.run(fetch=lambda u: self.fail("fetched without --online"))
        self.assertIn("not fetched", texts(items, fk.INFO))


class KeyIds(unittest.TestCase):
    def test_minisign_key_id(self):
        self.assertEqual(fk.minisign_key_id(minisign_pub("0123456789ABCDEF")), "0123456789ABCDEF")
        self.assertIsNone(fk.minisign_key_id("!!"))
        self.assertIsNone(fk.minisign_key_id(base64.b64encode(b"untrusted comment: x\nabc").decode()))


if __name__ == "__main__":
    unittest.main(verbosity=1)
