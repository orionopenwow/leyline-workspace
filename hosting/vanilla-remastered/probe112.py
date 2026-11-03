"""The 1.12 realm's self-check probes (run in a throwaway python container on the realm's network by
tools/ov112-off/start-112-realm.sh and hosting/vanilla-remastered/vr-beacon-up.sh). Standard library plus the kit's
wunet (mounted beside this file). Never logs in, never sends a password.

    python3 -I probe112.py logon HOST PORT    a 1.12.1 (build 5875) logon challenge for a made-up account;
                                              ok when the login server answers (0x04: unknown account)
    python3 -I probe112.py world HOST PORT    the world server's first packet is SMSG_AUTH_CHALLENGE
    python3 -I probe112.py http URL           the page answers (any status below 500)
    python3 -I probe112.py account SOAPHOST SOAPPORT AUTHHOST AUTHPORT WORLDHOST WORLDPORT [RELAYHOST RELAYPORT]
                                              the self-check with a fresh TEST account (vr-site-up.sh --test): made
                                              through the realm's own account command (SOAP "account create", the
                                              way its sign-up makes accounts), then a full 1.12.1 SRP6 logon and
                                              realm list on the login server (and through the relay's port for this
                                              realm when given), the world server's greeting, and the account
                                              deleted again. The SOAP login comes from VR_SOAP_USER / VR_SOAP_PASS
                                              (an env file; never printed); the test password is random and never
                                              printed either.

Prints one line (account: a few); exit 0 when it passed, 1 when not.
"""
import asyncio
import base64
import os
import re
import secrets
import struct
import sys
import urllib.error
import urllib.request
from xml.sax.saxutils import escape

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from wunet import srp6, wire335  # noqa: E402

BUILD = 5875
SMSG_AUTH_CHALLENGE = 0x1EC


async def world_hello(host, port, timeout=10):
    r, w = await asyncio.wait_for(asyncio.open_connection(host, port), timeout)
    try:
        head = await asyncio.wait_for(r.readexactly(4), timeout)
        return int.from_bytes(head[2:4], "little")
    finally:
        w.close()


async def logon112(host, port, user, password, timeout=15):
    """A full 1.12.1 logon (SRP6) and realm list request. Returns the number of realms listed."""
    r, w = await asyncio.wait_for(asyncio.open_connection(host, port), timeout)

    async def rd(n):
        return await asyncio.wait_for(r.readexactly(n), timeout)
    try:
        w.write(wire335.logon_challenge(user, BUILD))
        await w.drain()
        head = await rd(3)
        if head[0] != 0x00 or head[2] != 0x00:
            raise wire335.WowError(f"logon challenge refused (code {head[2]:#04x})")
        B = await rd(32)
        g = int.from_bytes(await rd((await rd(1))[0]), "little")
        N = int.from_bytes(await rd((await rd(1))[0]), "little")
        salt = await rd(32)
        await rd(16)
        if (await rd(1))[0]:
            raise wire335.WowError("the account wants extra security")
        c = srp6.Client(user, password)
        A, M1 = c.process(B, salt, g, N)
        w.write(bytes([0x01]) + A + M1 + bytes(20) + bytes([0, 0]))
        await w.drain()
        h = await rd(2)
        if h[0] != 0x01 or h[1] != 0x00:
            raise wire335.WowError(f"logon proof refused (code {h[1]:#04x})")
        rest = await rd(24)        # build 5875: M2 (20) + a 32-bit field
        if not c.check_server(rest[:20]):
            raise wire335.WowError("server proof did not match")
        w.write(bytes([0x10, 0, 0, 0, 0]))
        await w.drain()
        h = await rd(3)
        if h[0] != 0x10:
            raise wire335.WowError(f"unexpected reply {h[0]:#x} to the realm list request")
        body = await rd(struct.unpack_from("<H", h, 1)[0])
        return body[4] if len(body) > 4 else 0     # 1.12: uint32 unused, uint8 count
    finally:
        w.close()


def soap(host, port, user, password, command, timeout=15):
    """One console command through the world server's SOAP port (urn:MaNGOS). Returns (ok, result text)."""
    body = ('<?xml version="1.0" encoding="utf-8"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" '
            'xmlns:ns1="urn:MaNGOS"><SOAP-ENV:Body><ns1:executeCommand><command>' + escape(command)
            + '</command></ns1:executeCommand></SOAP-ENV:Body></SOAP-ENV:Envelope>').encode()
    auth = base64.b64encode(f"{user}:{password}".encode()).decode()
    req = urllib.request.Request(f"http://{host}:{port}/", data=body, method="POST",
                                 headers={"Content-Type": "text/xml", "Authorization": "Basic " + auth})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            text = resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        text = e.read().decode("utf-8", "replace")
    m = re.search(r"<result>(.*?)</result>", text, re.S) or re.search(r"<faultstring>(.*?)</faultstring>", text, re.S)
    res = (m.group(1) if m else text[:200]).strip()
    return ("<result>" in text and "<faultstring>" not in text), res


def account_check(a):
    su, sp = os.environ.get("VR_SOAP_USER", ""), os.environ.get("VR_SOAP_PASS", "")
    if not su or not sp:
        print("account: SKIPPED (no SOAP login found for the world server)")
        return 3
    soap_h, soap_p, auth_h, auth_p, world_h, world_p = a[0], int(a[1]), a[2], int(a[3]), a[4], int(a[5])
    user = "VRTEST" + "".join(secrets.choice("0123456789") for _ in range(6))
    pw = "".join(secrets.choice("ABCDEFGHJKLMNPQRSTUVWXYZ23456789") for _ in range(12))
    ok, res = soap(soap_h, soap_p, su, sp, f"account create {user} {pw}")
    res = res.replace(pw, "***")
    if not ok:
        print(f"account: could not make the test account ({res[:120]})")
        return 1
    print(f"account: test account {user} made through the realm's own account command")
    rc = 0
    try:
        n = asyncio.run(logon112(auth_h, auth_p, user, pw))
        print(f"account: logon on {auth_h}:{auth_p} ok, realm list has {n} realm(s)")
        if n < 1:
            rc = 1
        if len(a) >= 8:
            n = asyncio.run(logon112(a[6], int(a[7]), user, pw, timeout=25))
            print(f"account: logon through the relay ({a[6]}:{a[7]}) ok, realm list has {n} realm(s)")
        op = asyncio.run(world_hello(world_h, world_p))
        print(f"account: world server greeting {op:#06x} ({'ok' if op == SMSG_AUTH_CHALLENGE else 'unexpected'})")
        if op != SMSG_AUTH_CHALLENGE:
            rc = 1
    except (OSError, asyncio.TimeoutError, asyncio.IncompleteReadError, wire335.WowError, ValueError) as e:
        print(f"account: login FAILED: {str(e).replace(pw, '***')[:160] or type(e).__name__}")
        rc = 1
    finally:
        ok, res = soap(soap_h, soap_p, su, sp, f"account delete {user}")
        print(f"account: test account deleted ({'ok' if ok else res[:80]})")
    return rc


def main(argv):
    if len(argv) < 2:
        print(__doc__.strip().splitlines()[0])
        return 2
    what = argv[0]
    try:
        if what == "logon":
            code = asyncio.run(wire335.logon_probe(argv[1], int(argv[2]), timeout=8, build=BUILD))
            # a real answer for a 1.12.1 client: go on / banned / unknown account / in use. 0x09 (wrong version)
            # means the login server does not take 1.12.1 clients: not ok.
            ok = code in (0x00, 0x03, 0x04, 0x06)
            print(f"logon answer {code:#04x} ({'ok' if ok else 'unexpected'})")
            return 0 if ok else 1
        if what == "world":
            op = asyncio.run(world_hello(argv[1], int(argv[2])))
            ok = op == SMSG_AUTH_CHALLENGE
            print(f"world greeting {op:#06x} ({'ok' if ok else 'unexpected'})")
            return 0 if ok else 1
        if what == "http":
            try:
                with urllib.request.urlopen(urllib.request.Request(argv[1], method="GET"), timeout=10) as r:
                    code = r.status
            except urllib.error.HTTPError as e:
                code = e.code
            print(f"HTTP {code}")
            return 0 if code < 500 else 1
        if what == "account":
            return account_check(argv[1:])
    except (OSError, asyncio.TimeoutError, asyncio.IncompleteReadError, wire335.WowError, ValueError) as e:
        print(f"no answer: {type(e).__name__}")
        return 1
    print("unknown probe")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
