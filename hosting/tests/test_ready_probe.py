"""Ready for friends? (launcher host/ready.rs, docs/leyline/host-for-friends.md): the host's self-test sends the
game's first login message for LEYLINECHECK through the relay, the same knock and port a friend uses. The relay
lets it through, the realm answers "unknown account", and that answer is not counted as a wrong password (a host
pressing Check must never lock out their own home address, which their friends at home share). A probe stream
carries nothing after its one question, so the name can't be used to guess passwords uncounted."""
import asyncio
import struct
import unittest

from helpers import Rig, http
from wurelay import relay as relaymod


def hello(account):
    """The same bytes as launcher/src-tauri/src/host/ready.rs logon_hello."""
    name = account.upper().encode()
    b = bytes([0x00, 0x08]) + struct.pack("<H", 30 + len(name)) + b"\x57\x6f\x57\x00" + bytes([3, 3, 5]) + struct.pack("<H", 12340)
    b += b"68x\0" + b"niW\0" + b"SUne" + struct.pack("<I", 0) + bytes([127, 0, 0, 1]) + bytes([len(name)]) + name
    return b


async def ask(port, data, more=None):
    rd, wr = await asyncio.open_connection("127.0.0.1", port)
    wr.write(data)
    await wr.drain()
    got = await asyncio.wait_for(rd.readexactly(3), 5)
    closed = None
    if more is not None:
        wr.write(more)
        await wr.drain()
        try:
            closed = (await asyncio.wait_for(rd.read(10), 5)) == b""
        except (ConnectionError, asyncio.IncompleteReadError):
            closed = True
    wr.close()
    return got, closed


class Probe(unittest.TestCase):
    def test_the_hello_is_recognised(self):
        self.assertEqual(len(hello("LEYLINECHECK")), 46)
        self.assertTrue(relaymod.is_probe(hello("leylinecheck")))
        self.assertFalse(relaymod.is_probe(hello("ALICE")))
        self.assertFalse(relaymod.is_probe(hello("LEYLINECHECKX")))
        self.assertFalse(relaymod.is_probe(b"\x01" + hello("LEYLINECHECK")[1:]))


class ProbeThroughTheRelay(unittest.IsolatedAsyncioTestCase):
    async def test_the_self_test_reaches_the_realm_and_is_never_counted(self):
        async with Rig(visibility="private") as r:
            st, body, _ = await http("POST", f"{r.http}/api/realms/{r.rid}/knock", {"invite": r.cfg["invite_secret"]})
            self.assertEqual(st, 200, body)
            port = int(body["realmlist_port"].rsplit(":", 1)[1])
            for _ in range(r.relay.cfg.auth_fails_per_ip + 2):
                got, _ = await ask(port, hello("LEYLINECHECK"))
                self.assertEqual(got, bytes([0, 0, 4]), "the realm itself answered: unknown account")
            await asyncio.sleep(0.1)
            self.assertFalse(r.relay.auth_locked("127.0.0.1", r.rid), "probes are not wrong passwords")
            self.assertGreaterEqual(r.relay.metrics["probes"], r.relay.cfg.auth_fails_per_ip)
            self.assertIn(("unknown_account", "LEYLINECHECK"), r.realm.events)

    async def test_a_probe_stream_takes_no_second_message(self):
        async with Rig() as r:
            got, closed = await ask(r.ports[0], hello("LEYLINECHECK"), more=b"\x01" + bytes(74))
            self.assertEqual(got, bytes([0, 0, 4]))
            self.assertTrue(closed, "no proof goes through a probe")

    async def test_an_ordinary_unknown_account_still_counts(self):
        async with Rig() as r:
            await ask(r.ports[0], hello("NOBODYHERE"))
            await asyncio.sleep(0.1)
            self.assertEqual(r.relay.metrics["auth_failed"], 1)


if __name__ == "__main__":
    unittest.main()
