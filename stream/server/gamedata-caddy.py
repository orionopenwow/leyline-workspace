#!/usr/bin/env python3
"""The game data killswitch's Caddy part: a small block inside the streaming site (stream-up.sh's block for
wrath.openvanilla.app) that answers every game-data route with 410 Gone, before the sign-in gate and before
wu-play are ever asked.

    python3 -I gamedata-caddy.py add    <Caddyfile> [<stream host>]   put the block in (idempotent)
    python3 -I gamedata-caddy.py remove <Caddyfile>                   take it out (idempotent)
    python3 -I gamedata-caddy.py status <Caddyfile>                   "on" (block in), "off", or "no-site"
    python3 -I gamedata-caddy.py add-vr    <Caddyfile> <host>         the same for the Vanilla Remastered page's site
    python3 -I gamedata-caddy.py remove-vr <Caddyfile>                (hosting/vanilla-remastered/vr-site-up.sh's block:
    python3 -I gamedata-caddy.py status-vr <Caddyfile>                 /data, /data/* answer 410 there too)
    python3 -I gamedata-caddy.py add-sv    <Caddyfile> <host>         the same for Azeroth Survivors' site
    python3 -I gamedata-caddy.py remove-sv <Caddyfile>                (hosting/survivors/sv-site-up.sh's block: /data,
    python3 -I gamedata-caddy.py status-sv <Caddyfile>                 the manifests 410, every page a "resting" page)

Exit codes: 0 done, 3 no streaming site block in the Caddyfile (then nothing on that site is served at all, so
there is nothing to switch), 2 a broken Caddyfile (a start marker without its end marker).

The routes it closes (docs/GAME-DATA-ON-OUR-SERVER.md):
    /data and /data/*        the slice, the Forever UI overlay laid over it, /data/__index, /data/__tier/*
    /boot-manifest.json      the boot read-set: game file names (paths inside the game's archives)
    /world-manifest.json     the world read-set: the same, for the world
The page, the client's own code (/wenilla.js, the wasm, /boot.js, ...), /__gate, /__env.js and /ws stay as they
are: a player with their own game folder still plays (ownfiles.js answers /data in the page itself).

Standard library only. The Caddyfile is edited in place (it is bind-mounted into Caddy's container).
"""
from __future__ import annotations

import sys

STREAM_BEGIN = "# >>> WrathUnleashed streaming (stream-up.sh)"
STREAM_END = "# <<< WrathUnleashed streaming"
BEGIN = "\t# >>> game data killswitch (gamedata-switch.sh)"
END = "\t# <<< game data killswitch"
ROUTES = ("/data", "/data/*", "/boot-manifest.json", "/world-manifest.json")
PLAY_OWN = "https://play.openvanilla.app/wrath/play?mode=own"
# The answer: small JSON the client and the launcher can read, and plain words for anyone who opens it.
BODY = ('{"error":"gamedata_off","gamedata_streaming":false,'
        '"message":"This realm needs your own game folder. Drop your 3.3.5a game folder on the game page.",'
        '"own_files":"' + PLAY_OWN + '"}')


def block() -> str:
    """The lines that go right after the site's opening line (tab-indented like stream-up.sh's block)."""
    return (f"{BEGIN}: no game data from this server.\n"
            "\t# Every game-data route answers 410 Gone before the sign-in gate or wu-play is asked.\n"
            "\t# Undo: bash /srv/wrath/stream/gamedata-switch.sh on\n"
            f"\t@gamedata_off path {' '.join(ROUTES)}\n"
            "\thandle @gamedata_off {\n"
            '\t\theader Content-Type "application/json; charset=utf-8"\n'
            '\t\theader Cache-Control "no-store"\n'
            "\t\theader Cross-Origin-Resource-Policy same-origin\n"
            f"\t\trespond `{BODY}` 410\n"
            "\t}\n"
            f"{END}\n")


def _site(s: str) -> tuple[int, int] | None:
    """(start, end) of stream-up.sh's block, or None when the Caddyfile has none."""
    if STREAM_BEGIN not in s:
        return None
    i = s.index(STREAM_BEGIN)
    j = s.find(STREAM_END, i)
    if j < 0:
        raise ValueError("the streaming block's start marker has no end marker")
    return i, j


def strip(s: str) -> str:
    """The Caddyfile without our block (every copy of it)."""
    while BEGIN in s:
        i = s.index(BEGIN)
        j = s.find(END, i)
        if j < 0:
            raise ValueError("the killswitch start marker has no end marker")
        j = s.find("\n", j)
        s = s[:i] + (s[j + 1:] if j >= 0 else "")
    return s


def add(s: str, host: str = "wrath.openvanilla.app") -> str:
    """The Caddyfile with our block first inside the streaming site. Raises LookupError when there is no site."""
    s = strip(s)
    span = _site(s)
    if span is None:
        raise LookupError("no streaming site block")
    i, j = span
    opening = f"\n{host} {{\n"
    k = s.find(opening, i, j)
    if k < 0:
        raise LookupError(f"the streaming block has no '{host} {{' line")
    k += len(opening)
    return s[:k] + block() + s[k:]


def status(s: str) -> str:
    if _site(s) is None:
        return "no-site"
    return "on" if BEGIN in s else "off"


# The Vanilla Remastered page (the 1.12 browser slice, levels 1-20): its own site block, made by vr-site-up.sh. One
# switch for both: gamedata-switch.sh off puts this block in too. Its own markers, so each status stays exact.
VR_SITE_BEGIN = "# >>> Vanilla Remastered site (vr-site-up.sh)"
VR_SITE_END = "# <<< Vanilla Remastered site"
VR_BEGIN = "\t# >>> 1.12 game data killswitch (gamedata-switch.sh)"
VR_END = "\t# <<< 1.12 game data killswitch"
VR_ROUTES = ("/data", "/data/*")
VR_BODY = ('{"error":"gamedata_off","gamedata_streaming":false,'
           '"message":"This realm needs your own game client folder for now."}')


def vr_block() -> str:
    return (f"{VR_BEGIN}: no game data from this server.\n"
            "\t# Undo: bash /srv/wrath/stream/gamedata-switch.sh on\n"
            f"\t@gamedata_off_vr path {' '.join(VR_ROUTES)}\n"
            "\thandle @gamedata_off_vr {\n"
            '\t\theader Content-Type "application/json; charset=utf-8"\n'
            '\t\theader Cache-Control "no-store"\n'
            f"\t\trespond `{VR_BODY}` 410\n"
            "\t}\n"
            f"{VR_END}\n")


def vr_strip(s: str) -> str:
    while VR_BEGIN in s:
        i = s.index(VR_BEGIN)
        j = s.find(VR_END, i)
        if j < 0:
            raise ValueError("the 1.12 killswitch start marker has no end marker")
        j = s.find("\n", j)
        s = s[:i] + (s[j + 1:] if j >= 0 else "")
    return s


def vr_add(s: str, host: str) -> str:
    s = vr_strip(s)
    if VR_SITE_BEGIN not in s:
        raise LookupError("no Vanilla Remastered site block")
    i = s.index(VR_SITE_BEGIN)
    j = s.find(VR_SITE_END, i)
    if j < 0:
        raise ValueError("the Vanilla Remastered site block has no end marker")
    opening = f"\n{host} {{\n"
    k = s.find(opening, i, j)
    if k < 0:
        raise LookupError(f"the Vanilla Remastered block has no '{host} {{' line")
    k += len(opening)
    return s[:k] + vr_block() + s[k:]


def vr_status(s: str) -> str:
    if VR_SITE_BEGIN not in s:
        return "no-site"
    return "on" if VR_BEGIN in s else "off"


# Azeroth Survivors (docs/SURVIVORS.md): its own site block, made by hosting/survivors/sv-site-up.sh. The same switch:
# off puts this block first inside it, so the game files and the manifests answer 410 and every page but the licences
# answers a short "resting" page (the game is nothing without its files). Its own markers, so each status stays exact.
SV_SITE_BEGIN = "# >>> Azeroth Survivors site (sv-site-up.sh)"
SV_SITE_END = "# <<< Azeroth Survivors site"
SV_BEGIN = "\t# >>> Survivors game data killswitch (gamedata-switch.sh)"
SV_END = "\t# <<< Survivors game data killswitch"
SV_ROUTES = ("/data", "/data/*", "/boot-manifest.json", "/world-manifest.json")
SV_BODY = ('{"error":"gamedata_off","gamedata_streaming":false,'
           '"message":"Azeroth Survivors is resting. Please check back soon."}')
# No braces (Caddy placeholders) and no backticks (the respond quoting) in here: style attributes only.
SV_RESTING = ('<!doctype html><html lang="en"><head><meta charset="utf-8">'
              '<meta name="viewport" content="width=device-width,initial-scale=1"><title>Azeroth Survivors</title></head>'
              '<body style="margin:0;min-height:100vh;display:grid;place-items:center;background:#0b0e1a;color:#e8ecff;'
              'font:16px/1.5 system-ui,sans-serif;text-align:center"><main style="padding:24px;max-width:520px">'
              '<h1 style="font-size:28px;margin:0 0 8px">Azeroth Survivors is resting</h1>'
              '<p style="margin:0 0 16px">The game is taking a short rest. Please check back soon.</p>'
              '<p style="margin:0"><a style="color:#8fd3ff" href="https://play.openvanilla.app/wrath/survivors">Back to Leyline</a>'
              '</p></main></body></html>')


def sv_block() -> str:
    return (f"{SV_BEGIN}: no game data from this server, the game rests.\n"
            "\t# Undo: bash /srv/wrath/stream/gamedata-switch.sh on\n"
            f"\t@gamedata_off_sv path {' '.join(SV_ROUTES)}\n"
            "\thandle @gamedata_off_sv {\n"
            '\t\theader Content-Type "application/json; charset=utf-8"\n'
            '\t\theader Cache-Control "no-store"\n'
            f"\t\trespond `{SV_BODY}` 410\n"
            "\t}\n"
            "\t@resting_sv not path /licenses/*\n"
            "\thandle @resting_sv {\n"
            '\t\theader Content-Type "text/html; charset=utf-8"\n'
            '\t\theader Cache-Control "no-store"\n'
            f"\t\trespond `{SV_RESTING}` 503\n"
            "\t}\n"
            f"{SV_END}\n")


def sv_strip(s: str) -> str:
    while SV_BEGIN in s:
        i = s.index(SV_BEGIN)
        j = s.find(SV_END, i)
        if j < 0:
            raise ValueError("the Survivors killswitch start marker has no end marker")
        j = s.find("\n", j)
        s = s[:i] + (s[j + 1:] if j >= 0 else "")
    return s


def sv_add(s: str, host: str) -> str:
    s = sv_strip(s)
    if SV_SITE_BEGIN not in s:
        raise LookupError("no Azeroth Survivors site block")
    i = s.index(SV_SITE_BEGIN)
    j = s.find(SV_SITE_END, i)
    if j < 0:
        raise ValueError("the Azeroth Survivors site block has no end marker")
    opening = f"\n{host} {{\n"
    k = s.find(opening, i, j)
    if k < 0:
        raise LookupError(f"the Azeroth Survivors block has no '{host} {{' line")
    k += len(opening)
    return s[:k] + sv_block() + s[k:]


def sv_status(s: str) -> str:
    if SV_SITE_BEGIN not in s:
        return "no-site"
    return "on" if SV_BEGIN in s else "off"


def main(argv: list[str]) -> int:
    if len(argv) >= 3 and argv[1] in ("add-vr", "remove-vr", "status-vr", "add-sv", "remove-sv", "status-sv"):
        with open(argv[2], encoding="utf-8") as f:
            s = f.read()
        sv = argv[1].endswith("-sv")
        try:
            if argv[1] in ("status-vr", "status-sv"):
                print(sv_status(s) if sv else vr_status(s))
                return 0
            if argv[1] in ("add-vr", "add-sv") and len(argv) < 4:
                print(f"{argv[1]} needs the host", file=sys.stderr)
                return 64
            if sv:
                new = sv_add(s, argv[3]) if argv[1] == "add-sv" else sv_strip(s)
            else:
                new = vr_add(s, argv[3]) if argv[1] == "add-vr" else vr_strip(s)
        except LookupError as e:
            print(f"  {e}: nothing to change on that site", file=sys.stderr)
            return 3
        except ValueError as e:
            print(f"ERROR: {e}", file=sys.stderr)
            return 2
        if new != s:
            with open(argv[2], "r+", encoding="utf-8") as f:
                f.seek(0)
                f.write(new)
                f.truncate()
        return 0
    if len(argv) < 3 or argv[1] not in ("add", "remove", "status"):
        print(__doc__.strip().split("\n\n")[1], file=sys.stderr)
        return 64
    cmd, path = argv[1], argv[2]
    host = argv[3] if len(argv) > 3 else "wrath.openvanilla.app"
    with open(path, encoding="utf-8") as f:
        s = f.read()
    try:
        if cmd == "status":
            print(status(s))
            return 0
        new = add(s, host) if cmd == "add" else strip(s)
    except LookupError as e:
        print(f"  {e}: nothing to change on that site", file=sys.stderr)
        return 3
    except ValueError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2
    if new != s:
        with open(path, "r+", encoding="utf-8") as f:
            f.seek(0)
            f.write(new)
            f.truncate()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
