"""The join page's in-browser SRP6 verifier must match AzerothCore's (via wunet.srp6), and the relay
serves the page. Needs node for the JS part (skipped without it)."""
import json
import os
import shutil
import subprocess
import unittest

from helpers import Rig, http, ROOT
from wunet import srp6

PAGE = os.path.join(ROOT, "relay", "wurelay", "www", "join.html")


@unittest.skipUnless(shutil.which("node"), "node not installed")
class JoinPageSRP(unittest.TestCase):
    def test_js_verifier_matches(self):
        src = open(PAGE, encoding="utf-8").read()
        a = src.index("  const N = BigInt(")
        b = src.index("  window.__wuVerifier")
        js = src[a:b] + """
const cases = JSON.parse(process.argv[1]);
(async () => { const out = [];
  for (const [u, p, s] of cases) out.push(hex(await verifier(u, p, Uint8Array.from(Buffer.from(s, 'hex')))));
  console.log(JSON.stringify(out)); })();"""
        cases = [("friend1", "hunter2", os.urandom(32).hex()), ("ALICE", "password1", bytes((i * 11 + 2) & 0xFF for i in range(32)).hex()),
                 ("x1y2z3", "Pässwörd", os.urandom(32).hex())]
        out = subprocess.run(["node", "-e", js, json.dumps(cases)], capture_output=True, text=True, timeout=30)
        self.assertEqual(out.returncode, 0, out.stderr)
        got = json.loads(out.stdout)
        for (u, p, s), v in zip(cases[:2], got[:2]):
            self.assertEqual(v, srp6.make_verifier(u, p, bytes.fromhex(s))[1].hex())
        self.assertEqual(got[1], "28b837075a12b82553921d9095fa3fdcb0151c4bfc860ab97a69d0fa86a3d213")


class JoinPageOpensLeyline(unittest.TestCase):
    """Open in Leyline: the launcher's leyline://join link, made in the browser from the link the page was opened with
    (the invite after # never reaches a server), never while the browser game frames the page."""

    def test_open_in_leyline(self):
        src = open(PAGE, encoding="utf-8").read()
        self.assertIn('<button id="open-leyline" type="button">Open in Leyline</button>', src)
        self.assertIn('<div class="card" id="leyline-card" hidden>', src, "hidden until there is an invite")
        self.assertIn('"leyline://join?link=" + encodeURIComponent(location.origin + location.pathname', src)
        self.assertIn("realmId && invite && !hop", src)
        self.assertNotIn("fetch(\"leyline", src)

    @unittest.skipUnless(shutil.which("node"), "node not installed")
    def test_the_link_round_trips(self):
        # the same encoding the page does, read back as the launcher's deeplink.rs unescape does
        link = "https://relay.leylinelauncher.com/join/abcdefghijk2345a#ABCDEFGHJKLMNPQR~GUEST7~pw9"
        out = subprocess.run(["node", "-e", "console.log('leyline://join?link=' + encodeURIComponent(process.argv[1]))", link],
                             capture_output=True, text=True, timeout=30)
        self.assertEqual(out.returncode, 0, out.stderr)
        made = out.stdout.strip()
        from urllib.parse import unquote
        self.assertTrue(made.startswith("leyline://join?link=https%3A%2F%2Frelay.leylinelauncher.com%2Fjoin%2F"))
        self.assertNotIn("#", made, "the # is encoded, so the whole link reaches the launcher")
        self.assertEqual(unquote(made.split("link=", 1)[1]), link)


class JoinPageServed(unittest.IsolatedAsyncioTestCase):
    async def test_relay_serves_join_page(self):
        async with Rig() as r:
            st, body, hdr = await http("GET", f"{r.http}/join/{r.rid}")
            self.assertEqual(st, 200)
            self.assertIn("text/html", hdr["content-type"])
            self.assertIn("no-referrer", hdr["referrer-policy"])
            st, _, _ = await http("GET", f"{r.http}/join/NOTANID")
            self.assertEqual(st, 404)


if __name__ == "__main__":
    unittest.main()
