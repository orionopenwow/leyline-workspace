#!/usr/bin/env python3
"""Leyline Discord announcements: Claude drafts, the owner approves each one, then it goes to #announcements.

The owner's rule (5 Nov): "lets keep updates on schedule and milestone announcements @everyone in the discord when we
can, just not for minor fixes but community big bug reports and long standing bugs like your mobile controls and flow
setup". And the standing rule: nothing is posted for the owner without his explicit OK.

    python announce.py list                      every draft waiting in announce/pending/ and whether it passes
    python announce.py check <id>                one draft's checks (exit 1 when it can't be posted)
    python announce.py check --all               every pending draft (read-only; exit 0 unless a file can't be read)
    python announce.py post <id>                 by hand: asks you to type YES, then posts it
    python announce.py post --yes-from-control   Leyline Control's Discord card, after the owner typed YES there
    python announce.py reject <id>               moves it to announce/rejected/ (nothing is posted)

A draft is announce/pending/<id>.md:

    ---
    title: Phones, boss fights and an Android app
    kind: milestone            (milestone | bigfix | schedule; minor fixes get no announcement)
    ping: everyone             (everyone | none)
    created: 2026-11-05
    ---
    The post, in Discord markdown.

The message posted is "@everyone" (when ping is everyone), the title in bold, then the body: at most 2000 characters
in all. check refuses: a draft with TODO(owner) in it, the game's brand words, game client version numbers, money or
donation words, links other than Discord, Reddit and our own site and GitHub, @everyone or @here typed in the body,
a kind that isn't milestone, bigfix or schedule, and an @everyone ping on anything else.

Only `post` touches the network. It reads the bot token from token.txt (Leyline Control's Discord card saves it;
never printed), finds #announcements by name (like discord_leyline.py), posts with allowed_mentions that ping
@everyone only when the draft says so, then publishes it to following servers (an announcement channel's crosspost;
a failure there is only noted). The draft moves to announce/posted/<id>.md with the message id and the time.
Standard library only.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import os
import re
import sys
import urllib.parse
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import discord_leyline as dl  # noqa: E402

ANNOUNCE = HERE / "announce"
CONTROL_STEP = "discord-announce"
LIMIT = 2000                                        # Discord's limit for one message
KINDS = ("milestone", "bigfix", "schedule")
PINGS = ("everyone", "none")
ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{0,79}$")

# The website's brand check (stream/hub.py BANNED_RE; test_announce.py keeps the two the same)
BANNED_RE = re.compile(
    r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard)"
    r"|(?i:(?<![a-z])wow(?![a-z]))"
    r"|WoW|WOW|(?<![a-z])Wow(?=[A-Z0-9])|(?<=[a-z])Wow(?![a-z])")
# Game client version numbers (1.12.1, 3.3.5a, 2.4.3): never in prose
VERSION_RE = re.compile(r"(?<![\w.])[1-4]\.\d{1,2}\.\d{1,2}[a-z]?(?![\w.])")
MONEY_RE = dl.MONEY
TODO_RE = re.compile(r"TODO\s*\(\s*owner\s*\)", re.I)
MASS_PING_RE = re.compile(r"@(everyone|here)\b", re.I)
URL_RE = re.compile(r"(?i)\b(?:https?://|www\.)[^\s<>()\[\]]+")
BARE_RE = re.compile(r"(?i)(?<![\w@./-])(?:[a-z0-9-]+\.)+(?:com|net|org|io|gg|app|co|me|ly|xyz|tv|link|it|dev|site|"
                     r"store|shop|info|biz|us|uk|ru|cn)(?:/[^\s<>()\[\]]*)?(?![\w-])")
# The links a post may carry: Discord, Reddit, our own site and our GitHub
OWN_SITES = ("leylinelauncher.com", "openvanilla.app")
LINK_HOSTS = ("discord.com", "discord.gg", "discordapp.com", "reddit.com", "redd.it") + OWN_SITES
GITHUB_OWNERS = ("orionopenwow",)

ALLOWED_KEYS = ("title", "kind", "ping", "created")
MINOR_WORDS = ("minor", "fix", "fixes", "hotfix", "patch", "tweak", "small")


# ------------------------------------------------------------------------------------------------- reading a draft
def dirs(root: Path | None = None) -> dict:
    r = Path(root) if root else ANNOUNCE
    return {"root": r, "pending": r / "pending", "posted": r / "posted", "rejected": r / "rejected"}


def valid_id(i: str) -> bool:
    return bool(ID_RE.match(i or "")) and ".." not in i


def parse(text: str) -> tuple[dict, str, list]:
    """(front matter, body, problems with the file's shape). Front matter: `key: value` lines between --- lines."""
    text = text.replace("\r\n", "\n").lstrip("﻿")
    probs = []
    if not text.startswith("---\n"):
        return {}, text.strip(), ["It has no front matter: start the file with a --- line, then title, kind, ping, "
                                  "created, then another --- line."]
    end = text.find("\n---", 4)
    if end < 0:
        return {}, text.strip(), ["Its front matter never ends: put a --- line after it."]
    head, body = text[4:end], text[end + 4:]
    body = body[1:] if body.startswith("\n") else body
    front = {}
    for ln in head.split("\n"):
        if not ln.strip() or ln.lstrip().startswith("#"):
            continue
        k, sep, v = ln.partition(":")
        k = k.strip().lower()
        if not sep:
            probs.append(f"This front matter line isn't key: value: {ln.strip()[:60]}")
            continue
        front[k] = v.strip().strip('"').strip("'")
    return front, body.strip(), probs


def message(front: dict, body: str) -> str:
    """Exactly what goes to Discord."""
    parts = []
    if front.get("ping") == "everyone":
        parts.append("@everyone")
    if front.get("title"):
        parts.append(f"**{front['title']}**")
    head = "\n".join(parts)
    return (head + "\n\n" + body).strip() if head else body.strip()


def link_ok(url: str) -> bool:
    u = url.rstrip(".,;:!?'\"*_~`")
    if re.match(r"(?i)^http://", u):
        return False                                  # https only
    if not re.match(r"(?i)^https://", u):
        u = "https://" + u                            # www.x or a bare x.com: Discord links those too
    try:
        p = urllib.parse.urlsplit(u)
        host = (p.hostname or "").lower()
        if p.username or p.password or p.port:
            return False
    except ValueError:
        return False
    if any(host == h or host.endswith("." + h) for h in LINK_HOSTS):
        return True
    if host in ("github.com", "www.github.com"):
        owner = p.path.strip("/").split("/")[0].lower()
        return owner in GITHUB_OWNERS
    return False


def links(text: str) -> list:
    found = [m.group(0) for m in URL_RE.finditer(text)]
    rest = URL_RE.sub(" ", text)
    found += [m.group(0) for m in BARE_RE.finditer(rest)]
    return found


def problems(front: dict, body: str, shape: list | None = None) -> list:
    """Why this draft can't be posted, in plain words ([] when it can)."""
    out = list(shape or [])
    kind = (front.get("kind") or "").strip().lower()
    ping = (front.get("ping") or "").strip().lower()
    for k in front:
        if k not in ALLOWED_KEYS:
            out.append(f"Unknown front matter line '{k}': only {', '.join(ALLOWED_KEYS)}.")
    if not (front.get("title") or "").strip():
        out.append("It has no title.")
    if kind not in KINDS:
        if kind in MINOR_WORDS or "minor" in kind or "fix" in kind and kind != "bigfix":
            out.append("Minor fixes don't get an announcement (the owner's rule: milestones, big community bug "
                       "fixes, long-standing bugs and schedule updates only). Put it in the changelog instead.")
        else:
            out.append(f"kind must be milestone, bigfix or schedule (it says '{kind or 'nothing'}').")
    if ping not in PINGS:
        out.append(f"ping must be everyone or none (it says '{ping or 'nothing'}').")
    if not body.strip():
        out.append("The post is empty.")
    whole = (front.get("title") or "") + "\n" + body
    if TODO_RE.search(whole):
        out.append("It still has a TODO(owner) line: the owner fills it in or removes it first.")
    if MASS_PING_RE.search(whole):
        out.append("Don't type @everyone or @here in the post: set ping: everyone instead.")
    m = BANNED_RE.search(whole)
    if m:
        out.append(f"It uses a word players must never see: '{m.group(0)}'. Say 'the game' or use our own names.")
    m = VERSION_RE.search(whole)
    if m:
        out.append(f"It has a game version number ({m.group(0)}): leave version numbers out of posts.")
    m = MONEY_RE.search(whole)
    if m:
        out.append(f"It mentions money or donations ('{m.group(0)}'): no money, tip or donation talk in posts.")
    for u in links(whole):
        if not link_ok(u):
            out.append(f"This link isn't allowed: {u[:120]} (only Discord, Reddit, leylinelauncher.com and our "
                       "GitHub, over https).")
    n = len(message(front, body))
    if n > LIMIT:
        out.append(f"It is {n} characters with the ping and title: Discord's limit is {LIMIT}. Cut {n - LIMIT}.")
    return out


def load(draft_id: str, root: Path | None = None) -> dict:
    """One pending draft: id, front matter, body, the message, its sha256 (of the file) and its problems."""
    if not valid_id(draft_id):
        raise ValueError("That isn't a draft id (letters, digits and dashes, like 2026-11-05-boss-fights).")
    f = dirs(root)["pending"] / f"{draft_id}.md"
    raw = f.read_bytes()
    front, body, shape = parse(raw.decode("utf-8", errors="replace"))
    return {"id": draft_id, "file": f, "front": front, "body": body, "message": message(front, body),
            "sha": hashlib.sha256(raw).hexdigest(), "problems": problems(front, body, shape)}


def pending(root: Path | None = None) -> list:
    d = dirs(root)["pending"]
    out = []
    for f in sorted(d.glob("*.md")) if d.is_dir() else []:
        if valid_id(f.stem):
            try:
                out.append(load(f.stem, root))
            except OSError:
                continue
    return out


# ------------------------------------------------------------------------------------------------- moving a draft
def _move(draft: dict, where: str, extra: dict, root: Path | None = None) -> Path:
    """Write the draft into posted/ or rejected/ with extra front matter lines, then remove it from pending/."""
    d = dirs(root)[where]
    d.mkdir(parents=True, exist_ok=True)
    dest = d / f"{draft['id']}.md"
    if dest.exists():
        dest = d / f"{draft['id']}-{dt.datetime.now().strftime('%Y%m%d%H%M%S')}.md"
    front = dict(draft["front"])
    front.update(extra)
    lines = ["---"] + [f"{k}: {v}" for k, v in front.items()] + ["---", draft["body"], ""]
    tmp = dest.with_suffix(".tmp")
    tmp.write_text("\n".join(lines), encoding="utf-8")
    os.replace(tmp, dest)
    draft["file"].unlink()
    return dest


def reject(draft_id: str, root: Path | None = None, why: str = "") -> Path:
    draft = load(draft_id, root)
    extra = {"rejected": dt.datetime.now().strftime("%Y-%m-%d %H:%M")}
    if why:
        extra["why"] = " ".join(why.split())[:200]
    return _move(draft, "rejected", extra, root)


# ------------------------------------------------------------------------------------------------- posting
def find_announcements(api, gid: str) -> dict | None:
    """#announcements by name (glyphs and case ignored, the same names discord_leyline.py uses), a news channel first."""
    chans = api.request("GET", f"/guilds/{gid}/channels") or []
    names = dl.ALIASES.get("announcements", ["announcements"])
    best = None
    for alias in names:
        for c in sorted(chans, key=dl.okey):
            if c.get("type") in (dl.TEXT, dl.NEWS) and dl.slug(c.get("name")) == alias:
                if c.get("type") == dl.NEWS:
                    return c
                best = best or c
        if best:
            return best
    return best


def post(draft: dict, api, gid: str, root: Path | None = None) -> dict:
    """Post one checked draft. Returns {message_id, channel, published, file}. Raises SystemExit with a plain reason."""
    if draft["problems"]:
        raise SystemExit("Not posted: " + " ".join(draft["problems"]))
    if (dirs(root)["posted"] / f"{draft['id']}.md").exists():
        raise SystemExit("Not posted: a draft with this id was posted already (announce/posted/).")
    ch = find_announcements(api, gid)
    if not ch:
        raise SystemExit("Not posted: there is no #announcements channel the bot can see. Run the Discord redo "
                         "(Control > Discord > Preview) first.")
    everyone = draft["front"].get("ping") == "everyone"
    body = {"content": draft["message"], "allowed_mentions": {"parse": ["everyone"] if everyone else []}}
    msg = api.request("POST", f"/channels/{ch['id']}/messages", body=body) or {}
    mid = str(msg.get("id") or "")
    if not mid:
        raise SystemExit("Discord didn't say it posted the message (no message id). Look in #announcements before "
                         "trying again.")
    dl.say(f"Posted to #{dl.slug(ch.get('name'))} (message {mid})" + (" with @everyone." if everyone else "."))
    published = "no"
    if ch.get("type") == dl.NEWS:
        try:
            api.request("POST", f"/channels/{ch['id']}/messages/{mid}/crosspost")
            published = "yes"
            dl.say("Published it to servers that follow #announcements.")
        except (dl.ApiError, RuntimeError) as e:
            published = "failed"
            dl.say(f"Couldn't publish it to following servers ({e}); the post itself is up. Ignored.")
    else:
        dl.say("#announcements isn't an Announcement channel, so there was nothing to publish.")
    try:
        dest = _move(draft, "posted", {"posted": dt.datetime.now().strftime("%Y-%m-%d %H:%M"), "message_id": mid,
                                       "channel_id": ch["id"], "published": published}, root)
    except OSError as e:
        dl.say(f"!! Posted, but the draft couldn't be moved to announce/posted/ ({type(e).__name__}). Move it by hand "
               "so it isn't posted twice.")
        raise SystemExit(1)
    dl.say(f"Moved the draft to announce/posted/{dest.name}.")
    return {"message_id": mid, "channel": ch["id"], "published": published, "file": dest}


# ------------------------------------------------------------------------------------------------- the command line
def say_draft(d: dict) -> None:
    f = d["front"]
    state = "ready to post" if not d["problems"] else f"{len(d['problems'])} problem(s)"
    dl.say(f"{d['id']}: {f.get('title', '(no title)')} [{f.get('kind', '?')}, ping {f.get('ping', '?')}, "
           f"{len(d['message'])}/{LIMIT} characters] {state}")
    for p in d["problems"]:
        dl.say(f"    - {p}")


def main(argv=None) -> int:
    for s in (sys.stdout, sys.stderr):
        try:
            s.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass
    ap = argparse.ArgumentParser(description="Leyline Discord announcements: drafts the owner approves.")
    ap.add_argument("command", choices=["list", "check", "post", "reject"])
    ap.add_argument("id", nargs="?", default="")
    ap.add_argument("--all", action="store_true", help="check: every pending draft")
    ap.add_argument("--yes-from-control", action="store_true",
                    help="post: Leyline Control's Discord card, after the owner typed YES there")
    ap.add_argument("--why", default="", help="reject: a short reason, kept in the file")
    ap.add_argument("--guild", help="server id (default: the one server the bot is in)")
    ap.add_argument("--token-file", default=str(HERE / "token.txt"), help=argparse.SUPPRESS)
    ap.add_argument("--dir", default=None, help=argparse.SUPPRESS)
    o = ap.parse_args(argv)
    root = Path(o.dir) if o.dir else None

    if o.command == "list" or (o.command == "check" and o.all):
        ds = pending(root)
        if not ds:
            dl.say("No announcement drafts waiting (tools/discord-leyline/announce/pending/ is empty).")
        for d in ds:
            say_draft(d)
        ready = sum(1 for d in ds if not d["problems"])
        dl.say(f"ANNOUNCE CHECK OK: {ready} of {len(ds)} draft(s) ready to post. Nothing was posted.")
        return 0

    draft_id = o.id
    if o.command == "post" and o.yes_from_control:
        if os.environ.get("LEYLINE_CONTROL_STEP") != CONTROL_STEP:
            dl.say("--yes-from-control only works from Leyline Control (Discord > Pending announcements > Post). "
                   "Nothing was posted.")
            return 1
        want = os.environ.get("LEYLINE_ANNOUNCE_ID", "")
        if draft_id and draft_id != want:
            dl.say("That isn't the draft you approved in Control. Nothing was posted.")
            return 1
        draft_id = want
    if not draft_id:
        dl.say(f"Which draft? python announce.py {o.command} <id> (python announce.py list shows them).")
        return 1
    try:
        d = load(draft_id, root)
    except ValueError as e:
        dl.say(str(e))
        return 1
    except OSError:
        dl.say(f"No draft {draft_id} in announce/pending/.")
        return 1

    if o.command == "check":
        say_draft(d)
        return 0 if not d["problems"] else 1
    if o.command == "reject":
        dest = reject(draft_id, root, o.why)
        dl.say(f"Rejected: moved to announce/rejected/{dest.name}. Nothing was posted.")
        return 0

    # post
    say_draft(d)
    if d["problems"]:
        dl.say("Not posted: fix the problems above first.")
        return 1
    if o.yes_from_control:
        if os.environ.get("LEYLINE_ANNOUNCE_SHA", "") != d["sha"]:
            dl.say("The draft changed since you read it in Control. Read it again and type YES again. Nothing was "
                   "posted.")
            return 1
    else:
        dl.say("")
        dl.say("This is the post, exactly:")
        dl.say("----")
        dl.say(d["message"])
        dl.say("----")
        try:
            ans = input("Type YES to post it to #announcements: ").strip()
        except EOFError:
            ans = ""
        if ans != "YES":
            dl.say("Not posted.")
            return 1
    dl.NONINTERACTIVE = True
    try:
        tok = dl.read_token(o.token_file)
        api = dl.Discord(tok, allow_writes=True)
        gid = dl.pick_guild(api, o.guild)
        post(d, api, gid, root)
    except dl.ApiError as e:
        dl.say(f"Discord didn't accept it: {e}. Nothing more was done; look in #announcements before trying again.")
        return 1
    except SystemExit as e:
        if e.code not in (None, 0, 1):
            dl.say(str(e.code))
        return 1 if e.code not in (None, 0) else 0
    dl.say("ANNOUNCE POST OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
