#!/usr/bin/env python3
"""Link the game's World chat to Discord in one press (owner 4 Nov: "fix the process to link it to Discord").

    python bridge_link.py --from-control

1. With the bot token Control saved (token.txt; never printed), finds the server's #world-chat and #bug-reports
   (discord_leyline's names and aliases), read-only.
2. Packs the bridge (hosting/discord/wudiscord, hosting/discord/deploy) with a settings.env holding the token and the
   channel ids, and a turn-on mark, and sends it over ONE SSH login on stdin (never on a command line).
3. On the server, discord-up.sh installs it, writes only those keys into /opt/wu-discord/discord.env (chmod 600),
   checks the settings and turns wu-discord on. The last line is DISCORD BRIDGE OK.

The bot needs View Channel, Read Message History and Send Messages in both channels and the Message Content intent
(Developer Portal > the app > Bot). It must stay in the server: don't kick it after the layout (README step 9 is for a
separate setup bot). No #world-chat yet: press Preview, then Do it, in Control's Discord card (the layout makes it).
"""
from __future__ import annotations

import argparse
import io
import os
import subprocess
import sys
import tarfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import discord_leyline as dl  # noqa: E402

HOST = "root@play.openvanilla.app"
KEY = Path(os.environ.get("USERPROFILE", str(Path.home()))) / ".ssh" / "openwow"
CHAT_NAMES = ("world-chat", "world", "game-chat")
BUG_NAMES = ("bug-reports", "bug-report", "bugs")
INTAKE = "/srv/wrath/bugs/intake/discord.jsonl"


def base_name(name: str) -> str:
    """'🌍︱world-chat' -> 'world-chat'."""
    return (name or "").split(dl.GLYPH_SEP)[-1].strip().lower()


def find_channels(chans: list) -> tuple[str | None, str | None]:
    by = {}
    for c in chans:
        if c.get("type") in (0, 5):
            by.setdefault(base_name(c.get("name", "")), c["id"])
    chat = next((by[n] for n in CHAT_NAMES if n in by), None)
    bug = next((by[n] for n in BUG_NAMES if n in by), None)
    return chat, bug


def bundle(settings: dict) -> bytes:
    """The upload: discord/wudiscord, discord/deploy, settings.env and turn-on, as a gzip tar in memory."""
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        for sub in ("wudiscord", "deploy"):
            src = ROOT / "hosting" / "discord" / sub
            for p in sorted(src.rglob("*")):
                if p.is_file() and "__pycache__" not in p.parts and p.suffix != ".pyc":
                    tar.add(str(p), arcname=f"discord/{sub}/{p.relative_to(src).as_posix()}")
        for name, text in (("settings.env", "".join(f"{k}={v}\n" for k, v in settings.items())), ("turn-on", "")):
            data = text.encode("utf-8")
            info = tarfile.TarInfo(name)
            info.size, info.mode = len(data), 0o600
            tar.addfile(info, io.BytesIO(data))
    return buf.getvalue()


REMOTE = ("T=$(mktemp -d) && chmod 700 $T && tar -xzf - -C $T && sed 's/\\r$//' $T/discord/deploy/discord-up.sh > $T/up.sh "
          "&& bash $T/up.sh $T 2>&1 | tee /tmp/wu-discord-up.log; rm -rf $T; "
          "grep -q '^DISCORD BRIDGE OK' /tmp/wu-discord-up.log")


def main(argv=None) -> int:
    for s in (sys.stdout, sys.stderr):
        try:
            s.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass
    ap = argparse.ArgumentParser(description="Link World chat to Discord (installs and turns on the bridge).")
    ap.add_argument("--from-control", action="store_true")
    ap.add_argument("--guild")
    ap.add_argument("--token-file", default=str(HERE / "token.txt"), help=argparse.SUPPRESS)
    ap.add_argument("--dry-run", action="store_true", help="find the channels and pack, send nothing")
    o = ap.parse_args(argv)
    dl.NONINTERACTIVE = dl.NONINTERACTIVE or o.from_control
    tok = dl.read_token(o.token_file)
    api = dl.Discord(tok, allow_writes=False)
    try:
        gid = dl.pick_guild(api, o.guild)
        chans = api.request("GET", f"/guilds/{gid}/channels") or []
    except dl.ApiError as e:
        dl.say(f"Discord refused: {e}. Is the bot still in the server? (Re-invite it: README step 2.)")
        return 1
    chat, bug = find_channels(chans)
    if not chat:
        dl.say("No #world-chat channel yet. In Control's Discord card press Preview, then Do it: the layout makes it. "
               "Then run this again.")
        return 1
    dl.say(f"World chat <-> #world-chat (id {chat})" + (f"; bug feed -> #bug-reports (id {bug})" if bug else "; no bug channel"))
    settings = {"DISCORD_BOT_TOKEN": tok, "DISCORD_CHAT_CHANNEL_ID": chat, "DISCORD_GUILD_ID": gid}
    if bug:
        settings.update(DISCORD_BUG_CHANNEL_ID=bug, WU_DISCORD_BUG_INTAKE=INTAKE)
    data = bundle(settings)
    if o.dry_run:
        dl.say(f"Dry run: {len(data)} bytes packed, nothing sent.")
        return 0
    dl.say("Installing the bridge on the server and turning it on (one SSH login)...")
    cmd = ["ssh", "-i", str(KEY), "-o", "StrictHostKeyChecking=accept-new", "-o", "ServerAliveInterval=30", HOST, REMOTE]
    p = subprocess.run(cmd, input=data, capture_output=True)
    out = (p.stdout + p.stderr).decode("utf-8", "replace").replace(tok, "<token>")
    print(out.rstrip())
    if p.returncode == 0:
        dl.say("DISCORD LINK OK: World chat and #world-chat now talk both ways. In game, .discord off hides the lines "
               "for one player.")
        return 0
    dl.say("The bridge did not come up (see above). Nothing on the game side changed.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
