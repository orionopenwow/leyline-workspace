#!/usr/bin/env python3
"""Leyline Discord, read-only: the newest messages in every channel of the owner's server, into a file on his PC
(owner, 3 Nov launch night: "hook into my discord to gather messages and updates and suggestions").

    python discord_read.py --from-control [--max 200] [--since-hours 48]

What it does: with the bot token Control saved (token.txt, never printed), lists the server's text channels the bot
can see and fetches each one's messages since the last run (live/read-state.json keeps the newest id per channel; the
first run looks back --since-hours). It writes live/inbox-<stamp>.md (newest last per channel: time, name, the text,
attachment and link counts) and live/inbox-latest.md, and prints a short count per channel. Nothing is posted,
edited, reacted to or deleted: the client refuses every method but GET (discord_leyline.Discord, allow_writes off).

Message text needs the bot's Message Content intent (Developer Portal > the app > Bot > Privileged Gateway Intents).
Without it Discord sends empty text for messages that don't mention the bot; the file says so instead of guessing.
Players' messages stay on the owner's PC (live/ is git-ignored); only the counts are printed.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import discord_leyline as dl  # noqa: E402

LIVE = HERE / "live"
STATE = LIVE / "read-state.json"
TEXT_TYPES = {0, 5, 15}          # text, announcement, forum (a forum's posts are threads: their messages come below)
DISCORD_EPOCH_MS = 1420070400000


def snowflake_at(when: dt.datetime) -> str:
    return str((int(when.timestamp() * 1000) - DISCORD_EPOCH_MS) << 22)


def load_state() -> dict:
    try:
        return json.loads(STATE.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def one_line(text: str, n: int = 600) -> str:
    t = " ".join((text or "").split())
    return t if len(t) <= n else t[: n - 1] + "…"


def fetch_since(api, cid: str, after: str, cap: int) -> list:
    """Messages after `after`, oldest first, at most `cap`."""
    out = []
    while len(out) < cap:
        page = api.request("GET", f"/channels/{cid}/messages", query={"after": after, "limit": 100}) or []
        if not page:
            break
        page.sort(key=lambda m: int(m["id"]))
        out.extend(page)
        after = page[-1]["id"]
        if len(page) < 100:
            break
    return out[:cap]


def channel_list(api, gid: str) -> list:
    chans = api.request("GET", f"/guilds/{gid}/channels") or []
    cats = {c["id"]: c.get("name", "") for c in chans if c.get("type") == 4}
    out = [c for c in chans if c.get("type") in (0, 5)]
    # active threads (forum posts and threads under text channels)
    try:
        th = (api.request("GET", f"/guilds/{gid}/threads/active") or {}).get("threads") or []
    except dl.ApiError:
        th = []
    out.extend(th)
    for c in out:
        c["_cat"] = cats.get(c.get("parent_id") or "", "")
    return out


def render(gname: str, rows: list, empty_text: int, total: int) -> str:
    now = dt.datetime.now().strftime("%Y-%m-%d %H:%M")
    lines = [f"# Discord inbox: {gname}", f"Read {now}. {total} new messages.", ""]
    if empty_text and empty_text == total:
        lines += ["**Every message came without its text.** Turn on the bot's Message Content intent: Developer Portal "
                  "> your app > Bot > Privileged Gateway Intents > Message Content Intent, Save, then read again.", ""]
    for name, msgs in rows:
        if not msgs:
            continue
        lines.append(f"## #{name} ({len(msgs)})")
        for m in msgs:
            a = m.get("author") or {}
            who = (m.get("member") or {}).get("nick") or a.get("global_name") or a.get("username") or "?"
            if a.get("bot"):
                who += " (bot)"
            at = (m.get("timestamp") or "")[:16].replace("T", " ")
            extra = []
            if m.get("attachments"):
                extra.append(f"{len(m['attachments'])} attachment(s)")
            if m.get("embeds"):
                extra.append(f"{len(m['embeds'])} embed(s)")
            text = one_line(m.get("content") or "") or "(no text)"
            lines.append(f"- {at} **{who}**: {text}" + (f" [{', '.join(extra)}]" if extra else ""))
        lines.append("")
    if total == 0:
        lines.append("Nothing new since the last read.")
    return "\n".join(lines) + "\n"


def main(argv=None) -> int:
    for s in (sys.stdout, sys.stderr):
        try:
            s.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass
    ap = argparse.ArgumentParser(description="Read the newest Discord messages (read-only).")
    ap.add_argument("--from-control", action="store_true", help="run from Leyline Control: never asks anything")
    ap.add_argument("--guild", help="server id (default: the one server the bot is in)")
    ap.add_argument("--max", type=int, default=200, help="at most this many new messages per channel")
    ap.add_argument("--since-hours", type=float, default=48, help="the first read looks back this far")
    ap.add_argument("--token-file", default=str(HERE / "token.txt"), help=argparse.SUPPRESS)
    o = ap.parse_args(argv)
    dl.NONINTERACTIVE = dl.NONINTERACTIVE or o.from_control
    tok = dl.read_token(o.token_file)
    api = dl.Discord(tok, allow_writes=False)
    try:
        gid = dl.pick_guild(api, o.guild)
        guild = api.request("GET", f"/guilds/{gid}")
        chans = channel_list(api, gid)
    except dl.ApiError as e:
        dl.say(f"Discord refused: {e}. Is the bot still in the server? (Re-invite it: README step 2.)")
        return 1
    state = load_state()
    first = snowflake_at(dt.datetime.now(dt.timezone.utc) - dt.timedelta(hours=o.since_hours))
    rows, total, empty, hidden = [], 0, 0, 0
    for c in sorted(chans, key=lambda c: (c.get("_cat", ""), c.get("position", 0), c.get("name", ""))):
        cid = c["id"]
        try:
            msgs = fetch_since(api, cid, state.get(cid) or first, max(1, o.max))
        except dl.ApiError as e:
            if e.status in (403, 50001) or e.code in (50001, 50013):
                hidden += 1
                continue
            raise
        if msgs:
            state[cid] = msgs[-1]["id"]
        total += len(msgs)
        empty += sum(1 for m in msgs if not (m.get("content") or "").strip() and not m.get("attachments") and not m.get("embeds"))
        rows.append((c.get("name", cid), msgs))
        if msgs:
            dl.say(f"  #{c.get('name', cid)}: {len(msgs)} new")
    LIVE.mkdir(exist_ok=True)
    text = render(guild.get("name", "server"), rows, empty, total)
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M")
    (LIVE / f"inbox-{stamp}.md").write_text(text, encoding="utf-8")
    (LIVE / "inbox-latest.md").write_text(text, encoding="utf-8")
    STATE.write_text(json.dumps(state, indent=1), encoding="utf-8")
    dl.say(f"{total} new messages in {sum(1 for _, m in rows if m)} channels"
           + (f"; {hidden} channels the bot can't read" if hidden else "")
           + (f"; {empty} without text (the Message Content intent is off?)" if empty else ""))
    dl.say(f"Saved tools\\discord-leyline\\live\\inbox-{stamp}.md")
    dl.say("DISCORD READ OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
