"""The host pack's and the relay's public addresses after the move to leylinelauncher.com (docs/DOMAIN-MOVE.md):
the new names first, the old ones as fallback and still accepted.

    python3 -I hosting/tests/test_domains.py
"""
import asyncio
import json
import os
import sys
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import helpers  # noqa: F401  (sys.path)
from wunet import domains
from wuhost import agent as agentmod, setup as setupmod
from wurelay import relay as relaymod

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
with open(os.path.join(ROOT, "tools", "domain", "addresses.json"), encoding="utf-8") as f:
    A = json.load(f)


class Names(unittest.TestCase):
    def test_the_one_list(self):
        self.assertEqual(domains.DOMAIN, A["site"]["new"])
        self.assertEqual(domains.SITE, "https://" + A["site"]["new"])
        self.assertEqual(domains.GAME, "https://" + A["game"]["new"])
        self.assertEqual(domains.RELAY, A["relay"]["new"])
        self.assertEqual(domains.JOIN, "https://" + A["relay"]["new"])
        self.assertEqual(domains.OLD, {"site": "https://" + A["site"]["old"], "game": "https://" + A["game"]["old"],
                                       "relay": A["relay"]["old"]})

    def test_fallback(self):
        self.assertEqual(domains.fallback("wss://relay.leylinelauncher.com/agent"), "wss://relay.openvanilla.app/agent")
        self.assertEqual(domains.fallback("https://relay.leylinelauncher.com:443/api/listing/heartbeat?x=1"),
                         "https://relay.openvanilla.app:443/api/listing/heartbeat?x=1")
        self.assertEqual(domains.fallback("https://leylinelauncher.com/wrath/leyline/herokey.json"),
                         "https://play.openvanilla.app/wrath/leyline/herokey.json")
        self.assertIsNone(domains.fallback("https://relay.openvanilla.app/agent"))
        self.assertIsNone(domains.fallback("https://v1.example.org/agent"))
        self.assertIsNone(domains.fallback("relay.leylinelauncher.com"))
        self.assertEqual(domains.current("relay.openvanilla.app"), "relay.leylinelauncher.com")

    def test_a_fork_with_its_own_domain_has_no_old_names(self):
        d = domains.load({"LEYLINE_DOMAIN": "example.org"}, path="/nonexistent")
        self.assertEqual((d["site"], d["relay"], d["old"]["relay"]), ("https://example.org", "relay.example.org", ""))
        self.assertIsNone(domains.fallback("wss://relay.example.org/agent", d))

    def test_the_relay_accepts_the_site_and_the_game_on_both_names(self):
        env = {"RELAY_SITE_ORIGINS": "https://openvanilla.app", "RELAY_PLAY_ORIGINS": "https://wrath.openvanilla.app"}
        with mock.patch.dict(os.environ, env):
            c = relaymod.Config()
        for o in ("https://leylinelauncher.com", "https://play.openvanilla.app", "https://openvanilla.app"):
            self.assertIn(o, c.site_origins)
        for o in ("https://play.leylinelauncher.com", "https://wrath.openvanilla.app", "https://leylinelauncher.com",
                  "https://play.openvanilla.app"):
            self.assertIn(o, c.play_origins)
        self.assertNotIn("https://evil.example", c.play_origins)

    def test_the_site_key_is_fetched_on_the_old_name_when_the_new_one_has_no_answer(self):
        seen = []

        def get(u):
            seen.append(u)
            if "leylinelauncher" in u:
                raise OSError("no DNS")
            return None   # 404: no Hero Key sign-in
        self.assertEqual(setupmod.pin_site_key("https://leylinelauncher.com", get=get), [])
        self.assertEqual(seen, ["https://leylinelauncher.com/wrath/leyline/herokey.json",
                                "https://play.openvanilla.app/wrath/leyline/herokey.json"])


class Agent(unittest.IsolatedAsyncioTestCase):
    async def test_the_agent_tries_the_relays_old_name(self):
        tried = []

        async def connect(url, **kw):
            tried.append(url)
            if "leylinelauncher" in url:
                raise OSError("no DNS")
            raise asyncio.CancelledError   # stop right after: only the address matters here
        a = agentmod.Agent.__new__(agentmod.Agent)
        a.cfg = {"relay_url": "wss://relay.leylinelauncher.com/agent"}
        a.ssl_context = None
        with mock.patch.object(agentmod.ws, "connect", connect):
            with self.assertRaises(asyncio.CancelledError):
                await a.session()
        self.assertEqual(tried, ["wss://relay.leylinelauncher.com/agent", "wss://relay.openvanilla.app/agent"])


if __name__ == "__main__":
    unittest.main()
