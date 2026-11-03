"""Your Den on the Leyline network with no Docker, end to end on one PC: the real relay (wurelay), the launcher's
own tunnel (launcher/src-tauri/src/host/litenet.rs, run by its example `den_tunnel`) and the real Den Lite program
(labs/den-lite), with the Host kit's 3.3.5a test client (wunet/wire335) as the friend's game:

  * the tunnel signs in to the relay with its own key, private, signup off; the realm id is the Host kit's formula;
  * without the invite (no knock) the friend's game is cut off at the relay;
  * with the knock, the friend's one-time guest login (den-lite guest add) logs in through the relay, the realm list
    sends the game back through the relay (den-lite.public), and the guest enters the world on its own character;
  * the host's fixed local login (DEN / den) is refused through the relay;
  * a guest login taken back no longer works.

Needs the two programs built:
    DEN_LITE_BIN=<.../den-lite> DEN_TUNNEL_BIN=<.../examples/den_tunnel> python3 hosting/tests/test_den_lite_tunnel.py
(without them the test is skipped).
"""
import asyncio
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from helpers import free_port, free_range, http, relaymod, keys, wire335  # noqa: E402

DEN = os.environ.get("DEN_LITE_BIN", "")
TUN = os.environ.get("DEN_TUNNEL_BIN", "")


@unittest.skipUnless(DEN and TUN and os.path.exists(DEN) and os.path.exists(TUN), "DEN_LITE_BIN and DEN_TUNNEL_BIN not built")
class DenLiteTunnel(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.tmp = tempfile.mkdtemp(prefix="den-tunnel-")
        self.dens = os.path.join(self.tmp, "dens")
        lo, hi = free_range(20)
        cfg = dict(name="relay.test", http_bind="127.0.0.1:0", game_bind="127.0.0.1", shared_auth_port=free_port(),
                   port_min=lo, port_max=hi, state_file=os.path.join(self.tmp, "relay", "state.json"),
                   site_url="https://openvanilla.test", site_origins=["https://openvanilla.test"])
        self.relay = await relaymod.Relay(relaymod.Config(**cfg)).start()
        self.http = f"http://127.0.0.1:{self.relay.http_port()}"
        lp, wp = free_port(), free_port()
        self.den = subprocess.Popen([DEN, "--dens", self.dens, "--bind", "127.0.0.1", "--password", "den",
                                    "--login-port", str(lp), "--world-port", str(wp)],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(100):
            if os.path.exists(os.path.join(self.dens, "den-lite.remote")):
                break
            await asyncio.sleep(0.05)
        self.secret = "INVITESECRETINVITESECRET"
        self.key = os.path.join(self.tmp, "realm.key")
        self.tun = subprocess.Popen([TUN, "--relay", f"ws://127.0.0.1:{self.relay.http_port()}/agent", "--relay-name", "relay.test",
                                    "--dens", self.dens, "--key", self.key, "--secret", self.secret, "--name", "Wren's Den"],
                                   stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        self.state = None
        t0 = time.monotonic()
        while time.monotonic() - t0 < 15:
            line = await asyncio.to_thread(self.tun.stdout.readline)
            st = json.loads(line or "{}")
            if st.get("connected"):
                self.state = st
                break
        self.assertIsNotNone(self.state, "the tunnel never connected")

    async def asyncTearDown(self):
        for p in (self.tun, self.den):
            p.kill()
            p.wait()
        self.tun.stdout.close()
        await self.relay.stop()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def guest(self):
        out = subprocess.run([DEN, "guest", "add", "--dens", self.dens, "--name", "Rowan"], capture_output=True, text=True, check=True)
        return json.loads(out.stdout)

    async def test_a_friend_joins_through_the_relay_with_their_own_login(self):
        rid = self.state["realm_id"]
        with open(self.key) as f:
            self.assertEqual(rid, keys.realm_id(keys.public_key(bytes.fromhex(f.read().strip()))))
        s = self.relay.sessions[rid]
        self.assertEqual((s.visibility, s.signup, s.name), ("private", "off", "Wren's Den"))
        self.assertEqual(s.invite_sha, __import__("hashlib").sha256(self.secret.encode()).hexdigest())
        auth_port, world_port = self.state["auth_port"], self.state["world_port"]
        with open(os.path.join(self.dens, "den-lite.public")) as f:
            self.assertEqual(f.read().strip().rsplit(":", 1)[1], str(world_port), "friends' realm list: the relay's world port")
        g = self.guest()
        # no knock: cut off at the relay
        with self.assertRaises((wire335.WowError, asyncio.IncompleteReadError, ConnectionError, OSError)):
            await wire335.logon("127.0.0.1", auth_port, g["user"], g["pass"], timeout=3)
        st, body, _ = await http("POST", f"{self.http}/api/realms/{rid}/knock", {"invite": self.secret})
        self.assertEqual(st, 200, body)
        # the host's own login: refused through the relay
        with self.assertRaises(wire335.WowError):
            await wire335.logon("127.0.0.1", auth_port, "DEN", "den", timeout=5)
        # the guest: logs in, is sent back through the relay, enters the world on its own character
        rep = await wire335.full_run("127.0.0.1", auth_port, g["user"], g["pass"], world_override=f"127.0.0.1:{world_port}", timeout=15)
        self.assertEqual(rep["realm"]["address"].rsplit(":", 1)[1], str(world_port))
        self.assertIn("enter_world", rep["steps"])
        self.assertEqual(rep["character"]["name"], "Rowan")
        # taken back: no more
        subprocess.run([DEN, "guest", "revoke", "--dens", self.dens, g["user"]], check=True, capture_output=True)
        with self.assertRaises(wire335.WowError):
            await wire335.logon("127.0.0.1", auth_port, g["user"], g["pass"], timeout=5)


if __name__ == "__main__":
    unittest.main()
