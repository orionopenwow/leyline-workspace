"""The Builders' Realm's self-check probes (run in a throwaway python container on the realm's network by
hosting/build-realm/build-realm.sh). Standard library plus the kit's wunet (mounted beside this file). Never logs in
to the game, never sends a game password.

    python3 -I buildprobe.py logon HOST PORT     a 3.3.5a logon challenge for a made-up account; ok when the login
                                                 server answers (0x04: unknown account)
    python3 -I buildprobe.py world HOST PORT     the world server's first packet is SMSG_AUTH_CHALLENGE
    python3 -I buildprobe.py players URL         SOAP "server info" (SOAP_USER / SOAP_PASS from the environment):
                                                 prints "players N"
    python3 -I buildprobe.py soap URL COMMAND    one console command over SOAP (the same login); prints ok or failed

Prints one line; exit 0 when it passed, 1 when not. The SOAP login comes from the environment only and is never
printed.
"""
import asyncio
import base64
import os
import re
import sys
import urllib.error
import urllib.request
from xml.sax.saxutils import escape

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from wunet import wire335  # noqa: E402

SMSG_AUTH_CHALLENGE = 0x1EC


async def world_hello(host, port, timeout=10):
    r, w = await asyncio.wait_for(asyncio.open_connection(host, port), timeout)
    try:
        head = await asyncio.wait_for(r.readexactly(4), timeout)
        return int.from_bytes(head[2:4], "little")
    finally:
        w.close()


def soap(url, command, timeout=20):
    """(ok, result text) for one console command. The login is SOAP_USER / SOAP_PASS from the environment."""
    user, pw = os.environ.get("SOAP_USER", ""), os.environ.get("SOAP_PASS", "")
    if not user or not pw:
        raise ValueError("no SOAP login in the environment")
    body = ('<?xml version="1.0" encoding="utf-8"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" '
            'xmlns:ns1="urn:AC"><SOAP-ENV:Body><ns1:executeCommand><command>' + escape(command)
            + '</command></ns1:executeCommand></SOAP-ENV:Body></SOAP-ENV:Envelope>').encode()
    req = urllib.request.Request(url, data=body, method="POST", headers={
        "Content-Type": "text/xml", "Authorization": "Basic " + base64.b64encode(f"{user}:{pw}".encode()).decode()})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            text = r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        text = e.read().decode("utf-8", "replace")
    m = re.search(r"<result>(.*?)</result>", text, re.S)
    return (m is not None and "faultcode" not in text), (m.group(1) if m else "")


def main(argv):
    if len(argv) < 2:
        print(__doc__.strip().splitlines()[0])
        return 2
    what = argv[0]
    try:
        if what == "logon":
            code = asyncio.run(wire335.logon_probe(argv[1], int(argv[2]), timeout=8))
            ok = code in (0x00, 0x03, 0x04, 0x06)
            print(f"logon answer {code:#04x} ({'ok' if ok else 'unexpected'})")
            return 0 if ok else 1
        if what == "world":
            op = asyncio.run(world_hello(argv[1], int(argv[2])))
            ok = op == SMSG_AUTH_CHALLENGE
            print(f"world greeting {op:#06x} ({'ok' if ok else 'unexpected'})")
            return 0 if ok else 1
        if what == "players":
            ok, text = soap(argv[1], "server info")
            m = re.search(r"Connected players:\s*(\d+)", text)
            if not ok or not m:
                print("players unknown")
                return 1
            print(f"players {m.group(1)}")
            return 0
        if what == "soap" and len(argv) >= 3:
            ok, _ = soap(argv[1], " ".join(argv[2:]))
            print("ok" if ok else "failed")
            return 0 if ok else 1
    except (OSError, asyncio.TimeoutError, asyncio.IncompleteReadError, wire335.WowError, ValueError) as e:
        print(f"no answer: {type(e).__name__}")
        return 1
    print("unknown probe")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
