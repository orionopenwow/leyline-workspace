"""The Leyline release log (wunet/releaselog.py): verification, forks, truncation, tampering, gossip between nodes,
and the cross-language vector the launcher checks (launcher/src-tauri/src/release-log-vector.json).

    python3 -m unittest hosting/tests/test_release_log.py       (or: python3 hosting/tests/test_release_log.py)

Regenerate the vector (only when the format changes on purpose): WU_WRITE_VECTOR=1 python3 hosting/tests/test_release_log.py
"""
import asyncio
import json
import os
import subprocess
import sys
import tempfile
import types
import unittest

from helpers import ROOT  # noqa: F401 (sets sys.path)
from wunet import keys, releaselog as rl

DEV = rl.DEV_TEST_SEED
DEV_PUB = keys.public_key(DEV).hex()
OTHER = bytes(range(32))
VECTOR = os.path.join(os.path.dirname(ROOT), "launcher", "src-tauri", "src", "release-log-vector.json")
SHA = "ab" * 32


def chain(n, secret=DEV, start=None, t0=1761900000):
    doc = start or rl.empty(keys.public_key(secret).hex())
    for i in range(len(doc["entries"]), n):
        doc = rl.append(doc, secret, kind="launcher", name="Leyline", version=f"0.3.{i}",
                        files=[{"name": f"Leyline_0.3.{i}_x64-setup.exe", "size": 100 + i, "sha256": SHA},
                               {"name": "latest.json", "size": 10, "sha256": "cd" * 32}],
                        mirrors=["https://play.openvanilla.app/wrath/leyline/releases/"], now=t0 + i * 60)
    return doc


def vector_doc():
    doc = rl.empty(DEV_PUB)
    doc = rl.append(doc, DEV, kind="launcher", name="Leyline", version="0.9.0",
                    files=[{"name": "Leyline_0.9.0_x64-setup.exe", "size": 1040, "sha256": "11" * 32},
                           {"name": "latest.json", "size": 812, "sha256": "22" * 32}],
                    mirrors=["https://play.openvanilla.app/wrath/leyline/releases/", "https://mirror.example.org/leyline/"],
                    infohash="33" * 20, magnet="magnet:?xt=urn:btih:" + "33" * 20, now=1761900000)
    doc = rl.append(doc, DEV, kind="hostpack", name="Host pack", version="0.3.1",
                    files=[{"name": "leyline-host-pack-0.3.1.zip", "size": 5000, "sha256": "44" * 32},
                           {"name": "leyline-host-pack-0.3.1.zip.torrent", "size": 300, "sha256": "55" * 32}],
                    infohash="66" * 20, now=1761900060)
    doc = rl.append(doc, DEV, kind="source", name="mangos-classic", version="5731249",
                    files=[{"name": "mangos-classic-5731249195520323cb5fb9aa29761d4f033272ce.tar.gz", "size": 9000,
                            "sha256": "77" * 32}],
                    mirrors=["https://play.openvanilla.app/wrath/leyline/releases/"],
                    source={"repo": "https://github.com/cmangos/mangos-classic",
                            "commit": "5731249195520323cb5fb9aa29761d4f033272ce"}, now=1761900120)
    return doc


class Verify(unittest.TestCase):
    def test_a_good_chain_verifies(self):
        doc = chain(4)
        bodies = rl.verify(doc, DEV_PUB)
        self.assertEqual([b["seq"] for b in bodies], [0, 1, 2, 3])
        self.assertEqual(bodies[0]["prev"], rl.ZERO)
        self.assertEqual(bodies[2]["prev"], rl.entry_hash(doc["entries"][1]["entry"]))
        self.assertEqual(rl.head(doc), rl.entry_hash(doc["entries"][3]["entry"]))
        self.assertEqual(rl.verify(rl.empty(DEV_PUB), DEV_PUB), [])

    def test_tampering_is_caught(self):
        doc = chain(3)
        # a changed byte in an entry's text
        t = json.loads(json.dumps(doc))
        t["entries"][1]["entry"] = t["entries"][1]["entry"].replace("0.3.1", "0.3.9")
        with self.assertRaisesRegex(rl.Bad, "signature"):
            rl.verify(t, DEV_PUB)
        # a changed signature
        t = json.loads(json.dumps(doc))
        s = t["entries"][2]["sig"]
        t["entries"][2]["sig"] = ("0" if s[0] != "0" else "1") + s[1:]
        with self.assertRaisesRegex(rl.Bad, "signature"):
            rl.verify(t, DEV_PUB)
        # a whole chain signed with another key
        with self.assertRaisesRegex(rl.Bad, "another key|signature"):
            rl.verify(chain(2, OTHER), DEV_PUB)
        o = chain(2, OTHER)
        o["key"] = DEV_PUB
        with self.assertRaisesRegex(rl.Bad, "signature"):
            rl.verify(o, DEV_PUB)
        with self.assertRaisesRegex(rl.Bad, "no release key"):
            rl.verify(doc, "")

    def test_reorder_and_truncation_in_the_middle_are_caught(self):
        doc = chain(4)
        t = dict(doc, entries=[doc["entries"][0], doc["entries"][2], doc["entries"][3]])
        with self.assertRaisesRegex(rl.Bad, "out of order"):
            rl.verify(t, DEV_PUB)
        t = dict(doc, entries=[doc["entries"][1], doc["entries"][0]])
        with self.assertRaisesRegex(rl.Bad, "out of order"):
            rl.verify(t, DEV_PUB)
        # dropping the end is a good (shorter) chain: choose() keeps the longer copy
        short = dict(doc, entries=doc["entries"][:2])
        self.assertEqual(len(rl.verify(short, DEV_PUB)), 2)
        kept, what = rl.choose(doc, short, DEV_PUB)
        self.assertEqual((kept, what), (doc, "kept"))

    def test_game_data_is_never_named(self):
        doc = rl.empty(DEV_PUB)
        for name in ("patch-4.MPQ", "Wow.exe", "Spell.dbc", "../x.zip", "a/b.zip"):
            with self.assertRaises(rl.Bad, msg=name):
                rl.append(doc, DEV, kind="addons", name="x", version="1", files=[{"name": name, "size": 1, "sha256": SHA}])

    def test_source_entries_name_their_commit(self):
        doc = rl.empty(DEV_PUB)
        with self.assertRaisesRegex(rl.Bad, "source"):
            rl.append(doc, DEV, kind="source", name="core", version="1", files=[])
        with self.assertRaisesRegex(rl.Bad, "source"):
            rl.append(doc, DEV, kind="launcher", name="x", version="1", files=[],
                      source={"repo": "https://github.com/a/b", "commit": "a" * 40})
        with self.assertRaisesRegex(rl.Bad, "mirrors"):
            rl.append(doc, DEV, kind="addons", name="x", version="1", files=[], mirrors=["http://plain.example.com/"])


class Forks(unittest.TestCase):
    def test_longer_extension_is_adopted_and_shorter_kept_out(self):
        a = chain(2)
        b = chain(4, start=a)
        new, what = rl.choose(a, b, DEV_PUB)
        self.assertEqual(what, "adopted")
        self.assertEqual(len(new["entries"]), 4)
        self.assertEqual(rl.choose(b, a, DEV_PUB)[1], "kept")
        self.assertEqual(rl.choose(b, b, DEV_PUB)[1], "kept")
        self.assertEqual(rl.choose(None, a, DEV_PUB)[1], "adopted")

    def test_forks_signed_by_the_key_newest_head_wins(self):
        base = chain(2)
        f1 = rl.append(base, DEV, kind="note", name="fork one", version="1", files=[], now=1761990000)
        f2 = rl.append(rl.append(base, DEV, kind="note", name="fork two", version="1", files=[], now=1761990000),
                       DEV, kind="note", name="fork two b", version="1", files=[], now=1761990060)
        self.assertEqual(rl.choose(f1, f2, DEV_PUB)[1], "replaced")
        self.assertEqual(rl.choose(f2, f1, DEV_PUB)[1], "kept")

    def test_forks_not_signed_by_the_key_are_refused(self):
        base = chain(2)
        # someone without the key re-signs the whole chain with their own key and adds to it
        evil = chain(5, OTHER)
        evil["key"] = DEV_PUB
        got, what = rl.choose(base, evil, DEV_PUB)
        self.assertIs(got, base)
        self.assertTrue(what.startswith("refused"), what)
        # or splices an entry signed by another key onto a good chain
        other_tail = chain(3, OTHER)["entries"][2]
        got, what = rl.extend(base, [other_tail], DEV_PUB)
        self.assertTrue(what.startswith("refused"), what)


class Nodes(unittest.TestCase):
    def test_a_log_file_survives_restarts_and_ignores_junk(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "chain.json")
            log = rl.Log(p, DEV_PUB)
            self.assertEqual(len(log), 0)
            self.assertEqual(log.offer(chain(3)), "adopted")
            self.assertEqual(rl.Log(p, DEV_PUB).head()["seq"], 2)
            self.assertEqual(log.offer_more(chain(5)["entries"][3:]), "adopted")
            self.assertEqual(len(rl.Log(p, DEV_PUB)), 5)
            self.assertTrue(log.offer({"format": 1, "entries": "junk"}).startswith("refused"))
            with open(p, "w") as f:
                f.write("{broken")
            self.assertEqual(len(rl.Log(p, DEV_PUB)), 0)

    def test_two_mesh_nodes_swap_the_chain(self):
        from wurelay import meshnode
        with tempfile.TemporaryDirectory() as d:
            nodes, web = [], {}
            for i in range(2):
                cfg = types.SimpleNamespace(state_file=os.path.join(d, f"n{i}", "state.json"), name=f"n{i}.example.org",
                                            mesh_node_key="", mesh_public_http=f"https://n{i}.example.org",
                                            mesh_bootstrap="", mesh_fanout=1, release_log_key=DEV_PUB)
                os.makedirs(os.path.dirname(cfg.state_file))
                nodes.append(meshnode.MeshNode(cfg, fetch=lambda m, u, b=None: web_fetch(m, u, b)))
            nodes[0].peers = {"https://n1.example.org": {"fails": 0, "cursor": 0, "epoch": None}}

            def web_fetch(method, url, body=None):
                base = url.split("/api/mesh/")[0]
                node = nodes[0] if base.endswith("n0.example.org") else nodes[1]
                rest, _, q = url.split("/api/mesh/")[1].partition("?")
                req = types.SimpleNamespace(method=method, query=dict(x.split("=", 1) for x in q.split("&") if "=" in x),
                                            ip="127.0.0.1", json=lambda: body)
                resp = asyncio.run(node.route(req, rest.split("/")))
                return resp_status(resp), resp_json(resp)

            nodes[1].chain.offer(chain(3))
            self.assertEqual(len(nodes[0].chain), 0)
            asyncio.run(nodes[0].sync_chain("https://n1.example.org"))
            self.assertEqual(len(nodes[0].chain), 3, "pulled the whole chain")
            nodes[1].chain.offer(chain(5))
            asyncio.run(nodes[0].sync_chain("https://n1.example.org"))
            self.assertEqual(len(nodes[0].chain), 5, "pulled only what was new")
            self.assertEqual(nodes[0].chain.head(), nodes[1].chain.head())


def resp_status(resp):
    return int(resp.split(b" ", 2)[1]) if isinstance(resp, (bytes, bytearray)) else resp[0]


def resp_json(resp):
    body = resp.split(b"\r\n\r\n", 1)[1] if isinstance(resp, (bytes, bytearray)) else resp[1]
    return json.loads(body) if body else None


class CrossLanguage(unittest.TestCase):
    def test_the_launcher_vector(self):
        doc = vector_doc()
        if os.environ.get("WU_WRITE_VECTOR") == "1":
            with open(VECTOR, "w", encoding="utf-8") as f:
                json.dump({"about": "Signed with releaselog.DEV_TEST_SEED (TESTS ONLY). Written by hosting/tests/test_release_log.py; "
                                    "checked by launcher/src-tauri/src/releaselog.rs.", "pub": DEV_PUB, "chain": doc},
                          f, indent=1, ensure_ascii=False)
                f.write("\n")
        with open(VECTOR, encoding="utf-8") as f:
            v = json.load(f)
        self.assertEqual(v["pub"], DEV_PUB)
        self.assertEqual(v["chain"], doc, "Ed25519 is deterministic: the same entries sign to the same bytes")
        self.assertEqual(len(rl.verify(v["chain"], DEV_PUB)), 3)


class Keys(unittest.TestCase):
    def test_the_shipped_key_is_the_launchers(self):
        # launcher/branding.json and wunet/release-log.json name the same release key (launcher/scripts/rebrand.py
        # writes both); empty until the owner makes it, and never the dev key
        with open(os.path.join(os.path.dirname(ROOT), "launcher", "branding.json"), encoding="utf-8") as f:
            want = json.load(f)["release_log"]["public_key"]
        self.assertEqual(rl.shipped_key() or "", want)
        self.assertNotEqual(want, DEV_PUB)


class Cli(unittest.TestCase):
    def test_keygen_append_verify_never_prints_the_secret(self):
        with tempfile.TemporaryDirectory() as d:
            env = dict(os.environ, PYTHONPATH=os.path.join(ROOT, "kit", "tools"))
            run = lambda *a: subprocess.run([sys.executable, "-m", "wunet.releaselog", *a], capture_output=True, text=True, env=env)
            key = os.path.join(d, "release.key")
            r = run("keygen", "--out", key)
            self.assertEqual(r.returncode, 0, r.stderr)
            pub = r.stdout.strip()
            with open(key) as kf:
                secret = json.load(kf)["secret"]
            self.assertNotIn(secret, r.stdout + r.stderr)
            self.assertNotEqual(run("keygen", "--out", key).returncode, 0, "never overwrites a key")
            if os.name == "posix":
                self.assertEqual(os.stat(key).st_mode & 0o777, 0o600)
            f = os.path.join(d, "leyline-host-pack-0.3.1.zip")
            with open(f, "wb") as zf:
                zf.write(b"PK\x05\x06" + b"\0" * 18)
            log = os.path.join(d, "chain.json")
            r = run("append", "--log", log, "--key", key, "--kind", "hostpack", "--name", "Host pack", "--version", "0.3.1",
                    "--file", f, "--mirror", "https://play.openvanilla.app/wrath/leyline/releases/")
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertNotIn(secret, r.stdout + r.stderr)
            r = run("verify", "--log", log, "--pub", pub)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertIn("good: 1 entries", r.stdout)
            self.assertNotEqual(run("verify", "--log", log, "--pub", DEV_PUB).returncode, 0)


if __name__ == "__main__":
    unittest.main()
