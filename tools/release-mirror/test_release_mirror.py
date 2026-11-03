#!/usr/bin/env python3
"""tools/release-mirror/release_mirror.py: what it copies, the rewritten latest.json, checksums, torrents, the release
log entries (signed with the TESTS-ONLY dev key), nothing fetched twice, source tarballs checked inside.

    python3 -I tools/release-mirror/test_release_mirror.py
"""
import hashlib
import io
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import release_mirror as rm  # noqa: E402
from wunet import keys, releaselog  # noqa: E402  (release_mirror put hosting/kit/tools on the path)

GH = "https://github.com/orionopenwow/Leyline/releases/download/"
API = "https://api.github.com/repos/orionopenwow/Leyline/releases?per_page=30"
MIRROR = "https://play.openvanilla.app/wrath/leyline/releases/"
SIG = "dW50cnVzdGVk" * 10
COMMIT = "5731249195520323cb5fb9aa29761d4f033272ce"


def targz(files):
    b = io.BytesIO()
    with tarfile.open(fileobj=b, mode="w:gz") as t:
        for name, data in files:
            i = tarfile.TarInfo(name)
            i.size = len(data)
            t.addfile(i, io.BytesIO(data))
    return b.getvalue()


def fake_web(versions=("0.3.1", "0.3.2", "0.3.3", "0.3.4"), tarball=None):
    web, rel = {}, []
    for v in versions:
        tag = f"launcher-v{v}"
        exe, app = f"Leyline_{v}_x64-setup.exe", f"Leyline_{v}_amd64.AppImage"
        web[GH + f"{tag}/{exe}"] = b"MZ exe " + v.encode()
        web[GH + f"{tag}/{app}"] = b"\x7fELF appimage " + v.encode()
        web[GH + f"{tag}/latest.json"] = json.dumps({"version": v, "platforms": {
            "windows-x86_64": {"signature": SIG, "url": GH + f"{tag}/{exe}"},
            "linux-x86_64": {"signature": SIG, "url": GH + f"{tag}/{app}"}}}).encode()
        rel.append({"tag_name": tag, "draft": False, "assets": [{"name": n} for n in (exe, app, "latest.json")]})
    rel.append({"tag_name": "launcher-v9.9.9", "draft": True, "assets": [{"name": "latest.json"}]})
    web[GH + "host-pack/leyline-host-pack-2026.1031.1200.zip"] = b"PK\x05\x06" + b"\0" * 18
    rel.append({"tag_name": "host-pack", "assets": [{"name": "leyline-host-pack-2026.1029.1700.zip"},
                                                     {"name": "leyline-host-pack-2026.1031.1200.zip"}, {"name": "hostpack.json"}]})
    web[GH + "leyline-addons/leyline-addons-1.2.zip"] = b"PK\x05\x06addons" + b"\0" * 12
    rel.append({"tag_name": "leyline-addons", "assets": [{"name": "leyline-addons-1.2.zip"}, {"name": "addonpack.json"}]})
    web[f"https://codeload.github.com/cmangos/mangos-classic/tar.gz/{COMMIT}"] = tarball or targz(
        [("mangos-classic-x/src/World.cpp", b"// code"), ("mangos-classic-x/README.md", b"# CMaNGOS")])
    web[API] = json.dumps(rel).encode()
    calls = []

    def fetch(url, timeout=60):
        calls.append(url)
        if url not in web:
            raise OSError(f"404 {url}")
        return web[url]
    return fetch, calls


def jl(p):
    with open(p, encoding="utf-8") as f:
        return json.load(f)


def rd(p):
    with open(p, encoding="utf-8") as f:
        return f.read()


PINS = [{"repo": "https://github.com/cmangos/mangos-classic", "owner": "cmangos", "name": "mangos-classic", "commit": COMMIT, "profile": "cmangos-1.12.1"}]


def dev_key(d):
    p = os.path.join(d, "release-log.key")
    with open(p, "w") as f:
        json.dump({"kind": "leyline-release-log-key", "public": keys.public_key(releaselog.DEV_TEST_SEED).hex(),
                   "secret": releaselog.DEV_TEST_SEED.hex()}, f)
    return p


class Build(unittest.TestCase):
    def run_build(self, d, fetch, have=None, key=None, chain_path=None, logs=None):
        return rm.build(d, api=API, gh_download=GH, mirror=MIRROR, have=have or {}, pins=PINS, key_path=key,
                        chain_path=chain_path, fetch=fetch, now=1761930000, log=(logs.append if logs is not None else print))

    def test_copies_the_newest_launchers_hostpack_addons_and_source(self):
        with tempfile.TemporaryDirectory() as d:
            fetch, _ = fake_web()
            rep = self.run_build(d, fetch)
            kinds = sorted(rep["items"])
            self.assertIn(("hostpack", "Host pack", "2026.1031.1200"), kinds)
            self.assertIn(("addons", "Leyline addons", "1.2"), kinds)
            self.assertIn(("source", "mangos-classic", COMMIT[:12]), kinds)
            self.assertEqual(sorted(v for k, _, v in kinds if k == "launcher"), ["0.3.2", "0.3.3", "0.3.4"], "the newest three, no drafts")
            rel = os.path.join(d, "releases")
            top = jl(os.path.join(rel, "latest.json"))
            self.assertEqual(top["version"], "0.3.4")
            self.assertEqual(top["platforms"]["windows-x86_64"]["url"], MIRROR + "launcher-v0.3.4/Leyline_0.3.4_x64-setup.exe")
            self.assertEqual(top["platforms"]["windows-x86_64"]["signature"], SIG, "the signature is kept as it is")
            per = jl(os.path.join(rel, "launcher-v0.3.4", "latest.json"))
            self.assertEqual(per, top)
            idx = jl(os.path.join(rel, "index.json"))["files"]
            for path, meta in idx.items():
                with open(os.path.join(rel, *path.split("/")), "rb") as f:
                    self.assertEqual(hashlib.sha256(f.read()).hexdigest(), meta["sha256"], path)
            self.assertIn(f"sources/mangos-classic-{COMMIT}.tar.gz", idx)
            self.assertIn(f"sources/mangos-classic-{COMMIT}.tar.gz.torrent", idx)
            sums = rd(os.path.join(rel, "SHA256SUMS"))
            self.assertIn("  host-pack/leyline-host-pack-2026.1031.1200.zip\n", sums)
            self.assertFalse(os.path.exists(os.path.join(rel, "chain.json")))
            self.assertIn("waits", rep["chain"])

    def test_the_release_log_is_signed_and_nothing_is_fetched_twice(self):
        with tempfile.TemporaryDirectory() as d, tempfile.TemporaryDirectory() as home:
            key, chain_path = dev_key(home), os.path.join(home, "release-log-chain.json")
            secret_hex = releaselog.DEV_TEST_SEED.hex()
            fetch, calls = fake_web()
            logs = []
            rep = self.run_build(d, fetch, key=key, chain_path=chain_path, logs=logs)
            self.assertNotIn(secret_hex, json.dumps(rep, default=str) + "".join(logs))
            pub = keys.public_key(releaselog.DEV_TEST_SEED).hex()
            chain = jl(os.path.join(d, "releases", "chain.json"))
            bodies = releaselog.verify(chain, pub)
            self.assertEqual(len(bodies), 6, [(b["kind"], b["version"]) for b in bodies])
            hp = next(b for b in bodies if b["kind"] == "hostpack")
            idx = jl(os.path.join(d, "releases", "index.json"))["files"]
            self.assertEqual(hp["infohash"], idx["host-pack/leyline-host-pack-2026.1031.1200.zip"]["infohash"])
            self.assertEqual(hp["mirrors"], [MIRROR])
            src = next(b for b in bodies if b["kind"] == "source")
            self.assertEqual(src["source"], {"repo": "https://github.com/cmangos/mangos-classic", "commit": COMMIT})
            la = next(b for b in bodies if b["kind"] == "launcher" and b["version"] == "0.3.4")
            lj = next(f for f in la["files"] if f["name"] == "latest.json")
            with open(os.path.join(d, "releases", "launcher-v0.3.4", "latest.json"), "rb") as f:
                self.assertEqual(lj["sha256"], hashlib.sha256(f.read()).hexdigest(), "the log names the mirror's manifest")
            # a second run against the mirror as it now is: nothing new fetched, no new entries
            with tempfile.TemporaryDirectory() as d2:
                calls.clear()
                rep2 = self.run_build(d2, fetch, have=idx, key=key, chain_path=chain_path)
                self.assertEqual(rep2["new"], [])
                self.assertEqual([c for c in calls if "codeload" in c or c.endswith(".exe")], [])
                self.assertIn("0 new entries, 6 in all", rep2["chain"])
            # a newer release later: one more entry, the chain extends
            fetch3, _ = fake_web(versions=("0.3.2", "0.3.3", "0.3.4", "0.3.5"))
            with tempfile.TemporaryDirectory() as d3:
                rep3 = self.run_build(d3, fetch3, have=idx, key=key, chain_path=chain_path)
                self.assertIn("1 new entries, 7 in all", rep3["chain"])
                idx3 = jl(os.path.join(d3, "releases", "index.json"))["files"]
                self.assertFalse(any(p.startswith("launcher-v0.3.2/") for p in idx3), "older launchers leave the index")
                self.assertIn("latest.json", idx3)

    def test_a_key_other_than_the_trusted_one_is_refused(self):
        with tempfile.TemporaryDirectory() as d, tempfile.TemporaryDirectory() as home:
            key = dev_key(home)
            old = rm.branding
            rm.branding = lambda: {"release_log": {"public_key": "ab" * 32}}
            try:
                fetch, _ = fake_web()
                with self.assertRaisesRegex(rm.Refused, "not the one"):
                    self.run_build(d, fetch, key=key, chain_path=os.path.join(home, "c.json"))
            finally:
                rm.branding = old

    def test_source_tarballs_with_game_data_are_refused(self):
        for bad in ([("x/Data/patch.MPQ", b"..")], [("x/readme.txt", b"MPQ\x1a hidden")], [("x/Wow.exe", b"MZ")]):
            with tempfile.TemporaryDirectory() as d:
                fetch, _ = fake_web(tarball=targz(bad))
                with self.assertRaises(rm.Refused, msg=bad[0][0]):
                    self.run_build(d, fetch)
        with self.assertRaises(rm.Refused):
            rm.check_tarball(b"not a tarball", "x.tar.gz")

    def test_manifest_rewrite_refuses_foreign_downloads(self):
        doc = json.dumps({"version": "1.0.0", "platforms": {"windows-x86_64": {"signature": SIG, "url": "https://evil.example.com/x.exe"}}}).encode()
        with self.assertRaises(rm.Refused):
            rm.rewrite_manifest(doc, GH, MIRROR)

    def test_the_same_torrents_as_the_launcher(self):
        # launcher/src-tauri/src/p2p/share.rs torrents_hash_like_the_server_tools checks the same info-hash
        data = bytes((i * 7) % 256 for i in range(600000))
        self.assertEqual(rm.make_torrent("x.tar.gz", data, [])[1], "a74c848c91be56c4acf99f4d8431bf0a009ee700")


class Pins(unittest.TestCase):
    def test_every_profile_pin_is_found(self):
        pins = rm.profile_pins()
        repos = {p["name"] for p in pins}
        self.assertTrue({"mangos-classic", "classic-db", "mangos-tbc", "tbc-db", "TrinityCore"} <= repos, repos)
        self.assertTrue(all(len(p["commit"]) == 40 for p in pins))


class Files(unittest.TestCase):
    def test_server_script_and_bats(self):
        sh = os.path.join(HERE, "install-release-mirror.sh")
        self.assertEqual(subprocess.run(["sh", "-n", sh]).returncode, 0)
        text = rd(sh)
        self.assertIn("/srv/wrath/portal-data/leyline/releases", text)
        self.assertIn("RELEASE MIRROR OK", text)
        bat = rd(os.path.join(HERE, "Mirror releases.bat"))
        self.assertIn("release_mirror.py\" upload", bat)
        self.assertNotIn("set /p", bat.lower())
        # stream/leyline_net.py serves the same layout
        import importlib.util
        spec = importlib.util.spec_from_file_location("ln", os.path.join(rm.ROOT, "stream", "leyline_net.py"))
        ln = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(ln)
        with tempfile.TemporaryDirectory() as d:
            fetch, _ = fake_web()
            rm.build(d, api=API, gh_download=GH, mirror=MIRROR, have={}, pins=PINS, key_path=None, chain_path=None, fetch=fetch, now=1)
            root = os.path.join(d, "releases")
            idx = jl(os.path.join(root, "index.json"))["files"]
            for path in idx:
                self.assertIsNotNone(ln.release_file(ln.RELEASES_PREFIX + path, root), path)


if __name__ == "__main__":
    unittest.main()
