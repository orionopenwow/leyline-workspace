#!/usr/bin/env python3
"""Leyline Discord: redo the owner's Discord server to the new channel plan (docs/DISCORD-PLAN.md), in one run, from
his own PC.

The owner runs this himself (the .bat files next to it). Discord only lets a program change a server through a bot
token, so he makes a throwaway bot, invites it, pastes its token here, runs this, then kicks the bot. See README.md.

What it does, in order (the preview shows all of it first and changes nothing):
  1. backs up the server (settings, roles, channels and their permissions, the last N messages per text channel)
     to Desktop/Leyline-discord-backup-<stamp>/ before any change;
  2. the plan's eight roles (Core Team, Moderator, Guide, Realm Host, Mode Maker, Builder, Den Architect, Node Keeper);
     an old Helper role is renamed Guide, so whoever has it keeps it;
  3. the plan's categories and channels, named in the 30 Oct style "<emoji>︱<name>" ("👋︱welcome"), exactly as in
     stream/discord_template_v2.json, so the website bot's Set up the Discord finds them by exact name afterwards.
     A channel from an earlier pass ("✦︱welcome", "◆︱general", "⟡︱host-a-beacon", "#help", ...) is renamed and
     moved instead of made again, so its history stays (ALIASES below); one already named right keeps its name;
     topics, slowmodes, read-only and private permissions as in the plan; forum channels and the stage need a
     Community server; the forum channel gets one tag per forum category;
  4. system messages to #welcome, rules channel #rules (Community servers), AFK off. It never changes the server's
     name, icon, banner or invite background (owner, 1 Nov);
  5. every other channel moves into a hidden, read-only ARCHIVE category (Moderators can still read it);
  6. only with --delete-archived / --delete-old-roles / --wipe-history, and a typed confirmation: deletes things.
It posts nothing unless you add --post-first-posts AND type POST: then only the drafts in docs/discord-initial-posts.md
that have no TODO(owner) left, never in Start here. Otherwise it prints the drafts for you to paste yourself.
Running it twice changes nothing the second time.

Python 3 standard library only. The token is never printed, logged or written anywhere by this script.
"""
import argparse
import datetime as _dt
import getpass
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
API_BASE = os.environ.get("LEYLINE_DISCORD_API", "https://discord.com/api/v10").rstrip("/")
AUDIT_REASON = "Leyline rebrand"
USER_AGENT = "DiscordBot (https://github.com/orionopenwow/Leyline, 1.0) leyline-rebrand"
BACKOFF = float(os.environ.get("LEYLINE_DISCORD_BACKOFF", "1"))   # seconds; tests shrink it
POSTS_FILE = HERE.parent.parent / "docs" / "discord-initial-posts.md"

# ---------------------------------------------------------------------------------------------------------------------
# The plan. docs/DISCORD-PLAN.md; the website bot's copy is stream/discord_template_v2.json, and the tests check the
# two agree (names, types, topics, slowmodes, permissions). Change both together.
TEXT, VOICE, CATEGORY, NEWS, STAGE, FORUM = 0, 2, 4, 5, 13, 15
ARCHIVE = "ARCHIVE"


# The 30 Oct theming (owner, 30 Oct: "dress these channels with emojis not dots or stars"; 1 Nov: "keep the emoji
# thematics and bar too"): one emoji per channel, then the bar, then the name. The first eleven are the 30 Oct
# reset's own (welcome, announcements, general, help, looking-for-group, Hangout, mods-and-workshop, host-a-beacon,
# showcase, bug-reports, ideas), kept on the channels they became.
GLYPH_SEP = "︱"
EMOJI = {
    "welcome": "👋", "rules": "📜", "announcements": "📣", "changelog": "📝",
    "hangout": "💬", "levelling": "📈", "world-chat": "🌍", "looking-for-group": "🤝", "live-realms": "🟢",
    "events": "🎉",
    "den-showcase": "📸", "den-files": "📦", "den-help": "🔨",
    "host-templates": "📋", "hosting-help": "🏰", "list-your-realm": "📢", "nodes": "🔗", "host-lounge": "☕",
    "node-keepers": "🔑",
    "workshop-submissions": "📤", "mods-and-addons": "🧩", "modes-and-forks": "🎲", "scripting-help": "💻",
    "mode-jam": "🎮", "jam-judges": "🔍",
    "claude-and-agents": "🤖", "companions": "🐾",
    "bug-reports": "🐞", "feature-ideas": "💡", "launcher-help": "🛟",
    "forum": "📚",
    "roadmap": "🧭", "mod-room": "🔒",
    "Dalaran lounge": "🎧", "Group room 1": "🔊", "Group room 2": "🔊", "Group room 3": "🔊", "Showcase stage": "🎤",
}


def C(name, ctype=TEXT, topic=None, slow=0, ro=False, posters=(), private=(), optional=False, tags=()):
    """One channel of the plan. name: the base name (what ALIASES, the first posts and the ids use); full: the name
    on Discord, "<emoji>︱<name>". ro: @everyone can read, not post (posters can); private: only these roles see it;
    optional: skipped without a Community server; tags: forum tags."""
    return {"name": name, "full": f"{EMOJI[name]}{GLYPH_SEP}{name}", "type": ctype, "topic": topic, "slow": slow,
            "ro": ro, "posters": list(posters), "private": list(private), "optional": optional, "tags": list(tags)}


# The forum link's categories (stream/forum_discord.py) and the forum tag each one posts under.
FORUM_TAGS = [("realms", "Share your realm"), ("mods", "Mods & addons"), ("dens", "Dens & builds"),
              ("help", "Help"), ("general", "General")]

LAYOUT = [
    ("Start here", [
        C("welcome", topic="New here? Read this, then the rules. Play in your browser at leylinelauncher.com.",
          ro=True),
        C("rules", topic="Be kind. No game files, no selling, no ads. One ban covers your whole Leyline account.",
          ro=True),
        C("announcements", topic="News from the Leyline team. Forum announcements show up here too.", ro=True,
          posters=["Core Team", "Moderator"]),
        C("changelog", topic="What changed in the launcher, from its release notes. Posted automatically.", ro=True,
          posters=["Core Team"]),
    ]),
    ("Wrath Unleashed", [
        C("hangout", topic="Hang out: Wrath Unleashed, our own realm, and everything around it."),
        C("levelling", topic="Levelling routes, quests, the level 80 guide (.guide in the game), gear questions."),
        C("world-chat", topic="Live with the game's World channel: what you write here shows in the game as "
                              "[Discord]."),
        C("looking-for-group", topic="Find people to play with. Find a group on the website posts here too."),
        C("live-realms", topic="Realms on the Leyline network as they go live.", ro=True),
        C("events", topic="Weekly Dalaran events and community happenings.", ro=True,
          posters=["Core Team", "Moderator", "Guide"]),
    ]),
    ("Your Den", [
        C("den-showcase", topic="Show your Den: screenshots and a line about it. Den of the week is posted here."),
        C("den-files", topic="Share a Den file: put it on the Workshop and post the link here. Links to the Workshop "
                             "only.", slow=60),
        C("den-help", topic="Questions about building: edit mode (F7), props, paths, saving and loading, inviting "
                            "friends."),
    ]),
    ("Host a server", [
        C("host-templates", topic="Ready-made rulesets and presets for your own realm, from the team. Ask about them "
                                  "in hosting-help.", ro=True, posters=["Core Team", "Guide"]),
        C("hosting-help", topic="Help hosting a realm from the launcher's Host page or the Host pack."),
        C("list-your-realm", topic="Tell people about your realm. One post every 10 minutes.", slow=600),
        C("nodes", topic="Help carry the network: running a node."),
        C("host-lounge", topic="For people who host a listed realm.",
          private=["Realm Host", "Core Team", "Moderator"]),
        C("node-keepers", topic="Network changes, first.", private=["Node Keeper", "Core Team"]),
    ]),
    ("Mods & Workshop", [
        C("workshop-submissions", topic="Made a mod, addon or mode? Post its Workshop link here for the team to look "
                                        "at. One post every 10 minutes.", slow=600),
        C("mods-and-addons", topic="Mods and addons: share, ask, report problems with one."),
        C("modes-and-forks", topic="Game modes and forks. The featured mode is posted here."),
        C("scripting-help", topic="Help with realm scripts and mode files."),
        C("mode-jam", topic="The monthly mode jam. Bragging rights only, no prizes."),
        C("jam-judges", topic="Mode Makers judge the mode jam here.",
          private=["Mode Maker", "Core Team", "Moderator"]),
    ]),
    ("Claude & the companion", [
        C("claude-and-agents", topic="Your own AI assistant with Leyline: the launcher's Agents page, connecting "
                                     "Claude, what it can and can't do."),
        C("companions", topic="Bots for dungeons and raids in the game: .recruit, .buddy, the Bots window (/comp). "
                              "Always labelled as bots."),
    ]),
    ("Feedback", [
        C("bug-reports", topic="Found a bug? Say what you did, what you expected and what happened, and where. Every "
                               "message here goes on the team's bug list."),
        C("feature-ideas", FORUM, topic="Ideas for Leyline, one per thread. React to the ones you want."),
        C("launcher-help", topic="Help with the Leyline launcher: installing, signing in, updates."),
    ]),
    ("Forum", [
        C("forum", FORUM, topic="The Leyline forum, live: new threads and replies from "
                                "leylinelauncher.com/wrath/leyline/forum. Reply here once your account is linked "
                                "(/link).", tags=[t for _, t in FORUM_TAGS]),
    ]),
    ("Team", [
        C("roadmap", topic="What the core team is working on next.", private=["Core Team"]),
        C("mod-room", topic="Moderators and guides.", private=["Moderator", "Guide", "Core Team"]),
    ]),
    ("Voice", [
        C("Dalaran lounge", VOICE),
        C("Group room 1", VOICE),
        C("Group room 2", VOICE),
        C("Group room 3", VOICE),
        C("Showcase stage", STAGE, topic="Monthly showcases.", optional=True),
    ]),
]

# An existing category with one of these names (glyphs and case ignored) becomes the plan's category.
CATEGORY_ALIASES = {
    "start-here": ["start-here", "start"],
    "wrath-unleashed": ["wrath-unleashed", "play"],
    "your-den": ["your-den", "build", "create"],
    "mods-&-workshop": ["mods-&-workshop", "make"],
    "host-a-server": ["host-a-server", "host"],
    "feedback": ["feedback", "leyline"],
    "voice": ["voice", "voice-channels"],
}

# An existing channel with one of these names (glyphs and case ignored) is renamed and moved (history kept) instead of
# making a new one (the leading emoji or glyph is ignored when matching). First match wins; each old channel is used
# once. The 30 Oct reset's names and the growth plan's
# are all here (DISCORD-PLAN.md "Old channel to new"); anything else goes to the ARCHIVE.
ALIASES = {
    "welcome": ["welcome", "start-here"],
    "announcements": ["announcements", "announcement", "news"],
    "changelog": ["changelog", "releases", "release-notes"],
    "hangout": ["hangout", "general", "wrath-unleashed", "general-chat", "chat"],
    "levelling": ["levelling", "leveling"],
    "looking-for-group": ["looking-for-group", "lfg"],
    "den-showcase": ["den-showcase", "showcase", "builders-realm", "screenshots"],
    "den-help": ["den-help", "building-help"],
    "hosting-help": ["hosting-help", "host-a-beacon", "hosting"],
    "mods-and-addons": ["mods-and-addons", "mods-and-workshop", "mods", "addons", "workshop", "modding"],
    "modes-and-forks": ["modes-and-forks", "modes"],
    "bug-reports": ["bug-reports", "bug-report", "bugs"],
    # TODO(owner): if the old #help was mostly about hosting, move "help" to hosting-help's list instead.
    "launcher-help": ["launcher-help", "help", "support"],
    "dalaran-lounge": ["dalaran-lounge", "hangout", "lounge", "general"],
}

# Permission bits (Discord's numbers).
P_KICK = 1 << 1
P_ADD_REACTIONS = 1 << 6
P_VIEW = 1 << 10
P_SEND = 1 << 11
P_MANAGE_MESSAGES = 1 << 13
P_HISTORY = 1 << 16
P_CONNECT = 1 << 20
P_SPEAK = 1 << 21
P_MUTE = 1 << 22
P_MOVE = 1 << 24
P_MANAGE_THREADS = 1 << 34
P_PUBLIC_THREADS = 1 << 35
P_PRIVATE_THREADS = 1 << 36
P_SEND_IN_THREADS = 1 << 38
P_TIMEOUT = 1 << 40
P_NO_TALK = P_SEND | P_SEND_IN_THREADS | P_PUBLIC_THREADS | P_PRIVATE_THREADS
P_POSTER = P_SEND | P_SEND_IN_THREADS
P_MEMBER = P_VIEW | P_SEND | P_HISTORY | P_SEND_IN_THREADS | P_CONNECT | P_SPEAK      # who a private channel is for
P_BOT = P_VIEW | P_SEND | P_SEND_IN_THREADS | P_HISTORY                                # the Leyline website bot

# (name, colour, hoist (shown apart in the member list), mentionable, permissions given when the role is created).
# Colours and names as in discord_template_v2.json. Nobody gets Administrator.
ROLES = [
    ("Core Team", 0xE0C068, True, False, 0),
    ("Moderator", 0xC4523D, True, False, P_MANAGE_MESSAGES | P_MANAGE_THREADS | P_TIMEOUT | P_MUTE | P_MOVE),
    ("Guide", 0x5FB3A8, True, True, P_MANAGE_THREADS),
    ("Realm Host", 0x3F9F6F, False, False, 0),
    ("Mode Maker", 0xC58B2A, False, False, 0),
    ("Builder", 0xA0763A, False, False, 0),
    ("Den Architect", 0xB05FC4, False, False, 0),
    ("Node Keeper", 0x3D7FC4, False, False, 0),
]
# An existing role with one of these names becomes the plan's role (renamed, so whoever has it keeps it).
ROLE_ALIASES = {"Guide": ["guide", "helper"], "Moderator": ["moderator", "moderators"], "Core Team": ["core team"]}
KEEP_ROLE_NAMES = {"owner", "member"}       # never removed by --delete-old-roles either (Member: onboarding)

BANNED = re.compile(r"world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard|\d+\.\d+(\.\d+)?", re.I)
BRANDS = re.compile(r"world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard", re.I)
MONEY = re.compile(r"patreon|ko-?fi|paypal|donat|\btips?\b|buy\s*me|cash\s*app|venmo", re.I)

TEXTY = (TEXT, NEWS)
MOVABLE = (TEXT, VOICE, NEWS, STAGE, FORUM, 16)       # text, voice, announcement, stage, forum, media
MESSAGE_TYPES = (TEXT, VOICE, NEWS, STAGE)
MATCH = {TEXT: TEXTY, FORUM: (FORUM,), VOICE: (VOICE,), STAGE: (STAGE,)}
COMMUNITY_ONLY = (FORUM, STAGE)
SUPPRESS_EMBEDS = 1 << 2


# ---------------------------------------------------------------------------------------------------------------------
class Out:
    """Every line goes through here, so the token can never reach the screen even by accident."""
    secret = None

    @classmethod
    def say(cls, *parts):
        s = " ".join(str(p) for p in parts)
        if cls.secret:
            s = s.replace(cls.secret, "[hidden]")
        print(s, flush=True)


say = Out.say


class ApiError(Exception):
    def __init__(self, status, message, code=None):
        super().__init__(f"Discord said {status}: {message}")
        self.status, self.code = status, code


class Discord:
    def __init__(self, token, allow_writes=False, sleep=time.sleep):
        self._token = token
        self.allow_writes = allow_writes
        self.sleep = sleep

    def request(self, method, path, body=None, query=None):
        if method != "GET" and not self.allow_writes:
            raise RuntimeError(f"blocked a {method} in preview mode")       # belt and braces: preview never writes
        url = API_BASE + path + ("?" + urllib.parse.urlencode(query) if query else "")
        data = json.dumps(body).encode() if body is not None else None
        headers = {"Authorization": "Bot " + self._token, "User-Agent": USER_AGENT}
        if data is not None:
            headers["Content-Type"] = "application/json"
        if method != "GET":
            headers["X-Audit-Log-Reason"] = urllib.parse.quote(AUDIT_REASON)
        tries_5xx = 0
        tries_429 = 0
        while True:
            req = urllib.request.Request(url, data=data, method=method, headers=headers)
            try:
                with urllib.request.urlopen(req, timeout=60) as r:
                    raw = r.read()
                    if r.headers.get("X-RateLimit-Remaining") == "0":
                        self.sleep(min(float(r.headers.get("X-RateLimit-Reset-After") or 0), 60))
                    return json.loads(raw) if raw else None
            except urllib.error.HTTPError as e:
                raw = e.read()
                try:
                    info = json.loads(raw) if raw else {}
                except ValueError:
                    info = {}
                if e.code == 429 and tries_429 < 10:
                    tries_429 += 1
                    wait = info.get("retry_after") or e.headers.get("Retry-After") or 1
                    self.sleep(float(wait) + 0.05)
                    continue
                if e.code >= 500 and tries_5xx < 4:
                    self.sleep(BACKOFF * (2 ** tries_5xx))
                    tries_5xx += 1
                    continue
                msg = info.get("message") or (e.reason if isinstance(e.reason, str) else "error")
                raise ApiError(e.code, msg, info.get("code")) from None
            except (urllib.error.URLError, TimeoutError, ConnectionError) as e:
                if tries_5xx < 4:
                    self.sleep(BACKOFF * (2 ** tries_5xx))
                    tries_5xx += 1
                    continue
                raise ApiError("no answer", str(getattr(e, "reason", e))) from None


# ---------------------------------------------------------------------------------------------------------------------
def slug(name):
    """'✦︱Welcome' -> 'welcome', 'Dalaran lounge' -> 'dalaran-lounge'. Drops leading glyphs, lowercases, spaces to
    dashes."""
    s = (name or "").strip().lower()
    m = re.search(r"[0-9a-z]", s)
    s = s[m.start():] if m else ""
    return re.sub(r"[\s_]+", "-", s).strip("-")


def okey(o):
    try:
        n = int(o["id"])
    except (ValueError, TypeError):
        n = 10 ** 30
    return (o.get("position") or 0, n)


def ow_map(ch):
    return {o["id"]: o for o in ch.get("permission_overwrites") or []}


def ow_norm(lst):
    return sorted((str(o["id"]), int(o.get("type", 0)), int(o.get("allow") or 0), int(o.get("deny") or 0))
                  for o in lst or [])


def type_word(t):
    return {FORUM: "forum channel", VOICE: "voice channel", STAGE: "stage"}.get(t, "channel")


# ---------------------------------------------------------------------------------------------------------------------
class Rebrand:
    """Works out every change from the server as it is now. In preview it only records them; in apply it makes them.
    The same code runs both ways, so the preview is exactly what will happen."""

    def __init__(self, api, guild_id, opts, apply):
        self.api, self.gid, self.opts, self.apply = api, guild_id, opts, apply
        self.lines = []          # (section, text, is_change)
        self.changes = 0
        self.fake = 0
        self.archived = []       # channel ids that are (or will be) in ARCHIVE
        self.hidden_cats = []
        self.skipped = []        # plan channels not made (need Community, a role missing)

    # -- recording / doing --------------------------------------------------------------------------------------------
    def note(self, section, text):
        self.lines.append((section, text, False))

    def act(self, section, text, method, path, body=None):
        self.changes += 1
        self.lines.append((section, text, True))
        if not self.apply:
            return None
        res = self.api.request(method, path, body)
        say("  done:", text)
        return res

    def new_id(self):
        self.fake += 1
        return f"new-{self.fake}"

    # -- reading ------------------------------------------------------------------------------------------------------
    def load(self):
        g = self.api.request
        self.guild = g("GET", f"/guilds/{self.gid}")
        self.original_name = self.guild.get("name") or ""     # only shown and typed back; never changed
        self.roles = g("GET", f"/guilds/{self.gid}/roles")
        self.channels = g("GET", f"/guilds/{self.gid}/channels")
        self.me = g("GET", "/users/@me")
        bot = g("GET", f"/guilds/{self.gid}/members/{self.me['id']}")
        try:
            owner = g("GET", f"/guilds/{self.gid}/members/{self.guild['owner_id']}")
            self.owner_roles = set(owner.get("roles") or [])
        except ApiError:
            self.owner_roles = set()
        pos = {r["id"]: r.get("position", 0) for r in self.roles}
        self.my_roles = set(bot.get("roles") or [])
        self.bot_top = max([pos.get(r, 0) for r in self.my_roles] or [0])
        self.features = set(self.guild.get("features") or [])
        self.community = "COMMUNITY" in self.features
        self.portal_bot = self.find_portal_bot()

    def find_portal_bot(self):
        """The Leyline website bot's own role (Discord makes one per bot, named after the app), if it is in the server:
        it gets Send Messages in the read-only and private channels, as Set up the Discord would give it."""
        want = (getattr(self.opts, "leyline_bot", None) or "").strip().lower()
        for r in sorted(self.roles, key=lambda r: -r.get("position", 0)):
            tags = r.get("tags") or {}
            if not tags.get("bot_id") or tags.get("bot_id") == self.me.get("id") or r["id"] in self.my_roles:
                continue
            name = r["name"].strip().lower()
            if (want and name == want) or (not want and "leyline" in name):
                return r
        return None

    def role_block(self, r):
        """Why the script must leave this role alone, or None if it may change it."""
        if r["id"] == self.gid:
            return "@everyone"
        if r.get("managed") or (r.get("tags") or {}).get("integration_id") or (r.get("tags") or {}).get("bot_id"):
            return "belongs to a bot or integration"
        if r.get("position", 0) >= self.bot_top:
            return "above the bot's role"
        if r["id"] in self.owner_roles:
            return "you have this role"
        return None

    # -- the steps ----------------------------------------------------------------------------------------------------
    def run(self):
        self.load()
        self.do_roles()
        self.do_categories()
        self.do_channels()
        self.do_guild()
        self.do_archive()
        if self.opts.wipe_history:
            self.do_wipe_history()
        self.do_order()
        if self.opts.delete_old_roles:
            self.do_delete_roles()
        if self.opts.delete_archived:
            self.do_delete_archived()
        self.manual_steps()

    def find_role(self, name):
        names = ROLE_ALIASES.get(name, [name.lower()])
        for alias in names:
            found = sorted([r for r in self.roles if r["name"].strip().lower() == alias
                            and r["id"] not in self.role_ids.values() and r["id"] != self.gid],
                           key=lambda r: -r.get("position", 0))
            if found:
                return found[0]
        return None

    def do_roles(self):
        self.role_ids = {}
        for name, colour, hoist, mention, perms in ROLES:
            r = self.find_role(name)
            if r:
                self.role_ids[name] = r["id"]
                why = self.role_block(r)
                diff = {}
                if r["name"] != name:
                    diff["name"] = name
                if r.get("color", 0) != colour:
                    diff["color"] = colour
                if bool(r.get("hoist")) != hoist:
                    diff["hoist"] = hoist
                if not diff:
                    self.note("roles", f"keep {name} (already right)")
                elif why:
                    self.note("roles", f"keep {r['name']} as it is ({why}); can't restyle it")
                else:
                    what = (f"rename {r['name']} -> {name} (whoever has it keeps it)" if "name" in diff
                            else f"restyle {name}") + f" (colour #{colour:06x})"
                    self.act("roles", what, "PATCH", f"/guilds/{self.gid}/roles/{r['id']}", diff)
                    r.update(diff)
            else:
                body = {"name": name, "color": colour, "hoist": hoist, "mentionable": mention,
                        "permissions": str(perms)}
                res = self.act("roles", f"create {name} (colour #{colour:06x}{', shown apart' if hoist else ''})",
                               "POST", f"/guilds/{self.gid}/roles", body)
                if res:     # Discord puts a new role at the bottom and shifts the rest: read them again
                    self.roles = self.api.request("GET", f"/guilds/{self.gid}/roles")
                    self.bot_top += 1
                    r = next(x for x in self.roles if x["id"] == res["id"])
                else:
                    for o in self.roles:
                        if o["id"] != self.gid and o.get("position", 0) >= 1:
                            o["position"] = o.get("position", 0) + 1
                    self.bot_top += 1
                    r = dict(body, id=self.new_id(), position=1)
                    self.roles.append(r)
                self.role_ids[name] = r["id"]
        # Order the plan's roles (Core Team first) using only the slots these roles already hold.
        mine = [r for r in self.roles if r["id"] in self.role_ids.values() and not self.role_block(r)]
        want = [n for n, *_ in ROLES if any(r["id"] == self.role_ids[n] for r in mine)]
        if len(mine) > 1:
            slots = sorted((r.get("position", 0) for r in mine), reverse=True)
            byname = {n: next(r for r in mine if r["id"] == self.role_ids[n]) for n in want}
            cur = [n for n in sorted(want, key=lambda n: -byname[n].get("position", 0))]
            if cur != want or len(set(slots)) != len(slots):
                if len(set(slots)) != len(slots):
                    slots = list(range(max(slots), max(slots) - len(slots), -1))
                body = [{"id": byname[n]["id"], "position": p} for n, p in zip(want, slots) if p >= 1]
                self.act("roles", "order roles: " + " > ".join(want), "PATCH", f"/guilds/{self.gid}/roles", body)
                for b in body:
                    next(r for r in self.roles if r["id"] == b["id"])["position"] = b["position"]

    def find_cat(self, name):
        for alias in CATEGORY_ALIASES.get(slug(name), [slug(name)]):
            for c in sorted(self.channels, key=okey):
                if c["type"] == CATEGORY and slug(c["name"]) == alias and c["id"] not in self.used:
                    return c
        return None

    def label(self, target_id):
        if target_id == self.gid:
            return "@everyone"
        return next((r["name"] for r in self.roles if r["id"] == target_id), target_id)

    def ensure_overwrite(self, ch, target_id, otype, allow, deny, section):
        """Make one permission overwrite on a channel exactly (allow, deny); (0, 0) means 'none at all'."""
        cur = ow_map(ch).get(target_id)
        label = self.label(target_id)
        if allow == 0 and deny == 0:
            if cur is not None:
                self.act(section, f"{ch['name']}: clear the {label} override", "DELETE",
                         f"/channels/{ch['id']}/permissions/{target_id}")
                ch["permission_overwrites"] = [o for o in ch["permission_overwrites"] if o["id"] != target_id]
            return
        if cur is not None and int(cur.get("allow") or 0) == allow and int(cur.get("deny") or 0) == deny:
            return
        text = f"{ch['name']}: {label} " + ("can't see it" if deny & P_VIEW else "read-only" if deny & P_SEND else
                                           "can post" if allow & P_SEND else "can view")
        self.act(section, text, "PUT", f"/channels/{ch['id']}/permissions/{target_id}",
                 {"allow": str(allow), "deny": str(deny), "type": otype})
        ch["permission_overwrites"] = [o for o in ch.get("permission_overwrites") or [] if o["id"] != target_id]
        ch["permission_overwrites"].append({"id": target_id, "type": otype, "allow": str(allow), "deny": str(deny)})

    def do_categories(self):
        self.used = set()
        self.cat_ids = {}
        for cat, _ in LAYOUT:
            c = self.find_cat(cat)
            if c:
                self.used.add(c["id"])
                if c["name"] != cat:
                    self.act("channels", f"rename category {c['name']!r} -> {cat}", "PATCH",
                             f"/channels/{c['id']}", {"name": cat})
                    c["name"] = cat
                else:
                    self.note("reused", f"category {cat}")
            else:
                body = {"name": cat, "type": CATEGORY}
                res = self.act("created", f"category {cat}", "POST", f"/guilds/{self.gid}/channels", body)
                c = res or dict(body, id=self.new_id(), position=10 ** 6 + self.fake, permission_overwrites=[])
                self.channels.append(c)
                self.used.add(c["id"])
            self.cat_ids[cat] = c["id"]
            # a Leyline category is visible to everyone (private channels hide themselves): drop an @everyone override
            self.ensure_overwrite(c, self.gid, 0, 0, 0, "channels")

    def find_channel(self, spec):
        ok = MATCH[spec["type"]]
        s = slug(spec["name"])
        for alias in ALIASES.get(s, [s]):
            for c in sorted(self.channels, key=lambda c: int(c["id"]) if c["id"].isdigit() else 10 ** 30):
                if c["type"] in ok and c["id"] not in self.used and slug(c["name"]) == alias:
                    return c
        return None

    def wanted_overwrites(self, spec):
        """{target id: (type, allow, deny)} for @everyone, the plan's roles and the Leyline bot, as the plan says
        (the same overwrites Set up the Discord gives a channel it makes)."""
        out = {self.gid: (0, 0, 0)}
        if spec["private"]:
            out[self.gid] = (0, 0, P_VIEW)
            for rn in spec["private"]:
                out[self.role_ids[rn]] = (0, P_MEMBER, 0)
        if spec["ro"]:
            t, a, d = out[self.gid]
            out[self.gid] = (0, a, d | P_NO_TALK)
            for rn in spec["posters"]:
                out[self.role_ids[rn]] = (0, P_POSTER, 0)
        if (spec["ro"] or spec["private"]) and self.portal_bot:
            out[self.portal_bot["id"]] = (0, P_BOT, 0)
        return out

    def do_channels(self):
        self.chan_ids = {}
        for cat, chans in LAYOUT:
            for spec in chans:
                name, full = spec["name"], spec["full"]
                ctype, topic, slow = spec["type"], spec["topic"], spec["slow"]
                c = self.find_channel(spec)
                if c is None and ctype in COMMUNITY_ONLY and not self.community:
                    self.skipped.append(name)
                    self.note("can't", f"{name} ({type_word(ctype)}): needs a Community server. Server Settings -> "
                                       "Enable Community, then run this again"
                              + (" (or skip it)" if spec["optional"] else ""))
                    continue
                if c is None and any(rn not in self.role_ids for rn in spec["private"]):
                    self.skipped.append(name)
                    self.note("can't", f"{name}: a role it is private to is missing, so it wasn't made")
                    continue
                if c:
                    self.used.add(c["id"])
                    diff = {}
                    if c["name"] != full:
                        diff["name"] = full
                    if c["type"] == NEWS and ctype == TEXT:
                        diff["type"] = TEXT         # Set up the Discord looks for a text channel by name and type
                    if c.get("parent_id") != self.cat_ids[cat]:
                        diff["parent_id"] = self.cat_ids[cat]
                    if ctype in (TEXT, FORUM) and topic is not None and (c.get("topic") or None) != topic:
                        diff["topic"] = topic
                    if ctype in (TEXT, FORUM) and (c.get("rate_limit_per_user") or 0) != slow:
                        diff["rate_limit_per_user"] = slow
                    if diff:
                        what = ", ".join({"name": "rename", "type": "a plain text channel",
                                          "parent_id": f"move to {cat}", "topic": "topic",
                                          "rate_limit_per_user": f"slowmode {slow}s"}[k] for k in diff)
                        text = (f"{c['name']} -> {full} (keeps its history; {what})" if "name" in diff
                                else f"{full} (keeps its name and history; {what})")
                        self.act("reused", text, "PATCH",
                                 f"/channels/{c['id']}", diff)
                        c.update(diff)
                    else:
                        self.note("reused", f"{full} (already right)")
                else:
                    body = {"name": full, "type": ctype, "parent_id": self.cat_ids[cat]}
                    if ctype in (TEXT, FORUM) and topic:
                        body["topic"] = topic
                    if ctype in (TEXT, FORUM) and slow:
                        body["rate_limit_per_user"] = slow
                    extra = (f", slowmode {slow}s" if slow else "") + (", read-only" if spec["ro"] else "") + \
                            (", only " + ", ".join(spec["private"]) if spec["private"] else "")
                    word = "" if ctype == TEXT else f" ({type_word(ctype)})"
                    res = self.act("created", f"{full}{word} in {cat}{extra}", "POST",
                                   f"/guilds/{self.gid}/channels", body)
                    c = res or dict(body, id=self.new_id(), position=10 ** 6 + self.fake, permission_overwrites=[])
                    c.setdefault("permission_overwrites", [])
                    self.channels.append(c)
                    self.used.add(c["id"])
                self.chan_ids[name] = c["id"]
                want = self.wanted_overwrites(spec)
                if spec["private"]:
                    # a private channel gets exactly these overwrites, so no old grant leaves it visible
                    lst = [{"id": k, "type": t, "allow": str(a), "deny": str(d)} for k, (t, a, d) in want.items()]
                    if ow_norm(c.get("permission_overwrites")) != ow_norm(lst):
                        self.act("channels", f"{full}: only {', '.join(spec['private'])} can see it", "PATCH",
                                 f"/channels/{c['id']}", {"permission_overwrites": lst})
                        c["permission_overwrites"] = lst
                else:
                    for k, (t, a, d) in want.items():
                        self.ensure_overwrite(c, k, t, a, d, "channels")
                if spec["tags"]:
                    self.do_tags(c, spec["tags"])

    def do_tags(self, c, names):
        have = list(c.get("available_tags") or [])
        missing = [n for n in names if n.lower() not in {str(t.get("name", "")).lower() for t in have}]
        if not missing:
            return
        body = {"available_tags": [{k: t[k] for k in ("id", "name", "moderated", "emoji_id", "emoji_name") if k in t}
                                   for t in have] + [{"name": n} for n in missing]}
        res = self.act("channels", f"{c['name']}: forum tags " + ", ".join(missing), "PATCH", f"/channels/{c['id']}",
                       body)
        c["available_tags"] = (res or {}).get("available_tags") or body["available_tags"]

    def do_guild(self):
        """Where Discord's own messages go, and AFK off. Never the name, icon, banner or invite background."""
        diff, said = {}, []
        welcome, rules = self.chan_ids["welcome"], self.chan_ids["rules"]
        if self.guild.get("system_channel_id") != welcome:
            diff["system_channel_id"] = welcome
            said.append("system messages -> #welcome")
        if self.community and self.guild.get("rules_channel_id") != rules:
            diff["rules_channel_id"] = rules
            said.append("rules channel -> #rules")
        if self.guild.get("afk_channel_id"):
            diff["afk_channel_id"] = None
            said.append("AFK channel off")
        if not diff:
            self.note("server", "system messages, rules channel and AFK already right (name and icon never touched)")
            return
        res = self.act("server", "; ".join(said), "PATCH", f"/guilds/{self.gid}", diff)
        self.guild.update(res or diff)

    def archive_overwrites(self):
        return [{"id": self.gid, "type": 0, "allow": "0", "deny": str(P_VIEW | P_NO_TALK | P_CONNECT)},
                {"id": self.role_ids["Moderator"], "type": 0, "allow": str(P_VIEW | P_HISTORY),
                 "deny": str(P_NO_TALK | P_CONNECT)}]

    def do_archive(self):
        keep = {self.guild.get("public_updates_channel_id"), self.guild.get("safety_alerts_channel_id")} - {None}
        todo = [c for c in sorted(self.channels, key=okey)
                if c["type"] in MOVABLE and c["id"] not in self.used and c["id"] not in keep]
        for c in self.channels:
            if c["id"] in keep and c["id"] not in self.used:
                self.note("kept", f"{c['name']} stays where it is (Discord needs it for Community updates)")
        arch = self.find_cat(ARCHIVE)
        if not todo and not arch:
            self.note("archive", "nothing to archive")
            self.arch_id = None
            self.hide_old_categories()
            return
        want = self.archive_overwrites()
        if arch:
            self.used.add(arch["id"])
            if arch["name"] != ARCHIVE:
                self.act("archive", f"rename category {arch['name']!r} -> {ARCHIVE}", "PATCH",
                         f"/channels/{arch['id']}", {"name": ARCHIVE})
                arch["name"] = ARCHIVE
            if ow_norm(arch.get("permission_overwrites")) != ow_norm(want):
                self.act("archive", "ARCHIVE: hidden from everyone, Moderators can read", "PATCH",
                         f"/channels/{arch['id']}", {"permission_overwrites": want})
                arch["permission_overwrites"] = want
        else:
            body = {"name": ARCHIVE, "type": CATEGORY, "permission_overwrites": want}
            res = self.act("archive", "create hidden ARCHIVE category (read-only, Moderators can read)", "POST",
                           f"/guilds/{self.gid}/channels", body)
            arch = res or dict(body, id=self.new_id(), position=10 ** 6 + self.fake)
            self.channels.append(arch)
            self.used.add(arch["id"])
        self.arch_id = arch["id"]
        for c in todo:
            self.used.add(c["id"])
            self.archived.append(c["id"])
            body = {}
            if c.get("parent_id") != arch["id"]:
                body["parent_id"] = arch["id"]
            if ow_norm(c.get("permission_overwrites")) != ow_norm(want):
                body["permission_overwrites"] = want
            if body:
                self.act("archive", f"{c['name']} -> ARCHIVE (hidden, read-only, history kept)", "PATCH",
                         f"/channels/{c['id']}", body)
                c.update(body)
        if not todo:
            self.note("archive", "every old channel is already in ARCHIVE")
        self.hide_old_categories()

    def hide_old_categories(self):
        for cat in sorted(self.channels, key=okey):
            if cat["type"] != CATEGORY or cat["id"] in self.used:
                continue
            self.hidden_cats.append(cat["id"])
            if any(c.get("parent_id") == cat["id"] for c in self.channels if c["type"] != CATEGORY):
                continue        # still holds something we left alone (e.g. Community updates)
            cur = ow_map(cat).get(self.gid)
            if cur and int(cur.get("deny") or 0) & P_VIEW:
                continue
            deny = (int(cur.get("deny") or 0) if cur else 0) | P_VIEW
            allow = (int(cur.get("allow") or 0) if cur else 0) & ~P_VIEW
            self.act("archive", f"hide the empty old category {cat['name']!r}", "PUT",
                     f"/channels/{cat['id']}/permissions/{self.gid}",
                     {"allow": str(allow), "deny": str(deny), "type": 0})
            cat["permission_overwrites"] = [o for o in cat.get("permission_overwrites") or []
                                            if o["id"] != self.gid] + [
                {"id": self.gid, "type": 0, "allow": str(allow), "deny": str(deny)}]

    def do_order(self):
        cats = sorted([c for c in self.channels if c["type"] == CATEGORY], key=okey)
        want = [self.cat_ids[c] for c, _ in LAYOUT] + ([self.arch_id] if self.arch_id else [])
        want += [c["id"] for c in cats if c["id"] not in want]
        body = []
        if [c["id"] for c in cats] != want:
            body += [{"id": i, "position": n} for n, i in enumerate(want)]
        for cat, chans in LAYOUT:
            for group in ((TEXT, NEWS, FORUM), (VOICE, STAGE)):
                ids = [self.chan_ids[s["name"]] for s in chans if s["type"] in group and s["name"] in self.chan_ids]
                cur = [c["id"] for c in sorted(self.channels, key=okey)
                       if c.get("parent_id") == self.cat_ids[cat] and c["type"] in group]
                if cur != ids:
                    body += [{"id": i, "position": n} for n, i in enumerate(ids)]
        if body:
            self.act("order", "put categories and channels in the plan's order (Start here first, ARCHIVE last)",
                     "PATCH", f"/guilds/{self.gid}/channels", body)
            for b in body:
                next(c for c in self.channels if c["id"] == b["id"])["position"] = b["position"]

    def old_roles(self):
        keep_names = {n.lower() for n, *_ in ROLES} | KEEP_ROLE_NAMES
        out, blocked = [], []
        for r in sorted(self.roles, key=lambda r: -r.get("position", 0)):
            if r["id"] in self.role_ids.values() or r["name"].strip().lower() in keep_names:
                continue
            why = self.role_block(r)
            if why == "@everyone":
                continue
            (blocked if why else out).append((r, why))
        return out, blocked

    def do_delete_roles(self):
        out, blocked = self.old_roles()
        for r, why in blocked:
            self.note("roles", f"keep {r['name']} ({why})")
        for r, _ in out:
            self.act("deleted", f"role {r['name']}", "DELETE", f"/guilds/{self.gid}/roles/{r['id']}")

    def do_wipe_history(self):
        """--wipe-history (owner 2026-10-30: "wipe all chat histories"): every Leyline text channel is replaced by a
        fresh copy (same name, topic, category, permissions, slowmode) and the old one deleted, so its messages go
        with it. Discord has no "clear channel"; deleting messages one by one takes hours and can't touch ones older
        than 14 days in bulk. The backup (made first) keeps the last --backup-messages messages of each. Forum and
        voice channels are left as they are."""
        keep = ("name", "type", "topic", "parent_id", "position", "rate_limit_per_user", "nsfw",
                "permission_overwrites")
        for cat, chans in LAYOUT:
            for spec in chans:
                name = spec["name"]
                cid = self.chan_ids.get(name)
                c = next((x for x in self.channels if x["id"] == cid), None)
                if c is None or c["type"] not in TEXTY:
                    continue
                body = {k: c[k] for k in keep if c.get(k) is not None}
                body["parent_id"] = self.cat_ids.get(cat, body.get("parent_id"))
                new = self.act("wiped", f"{body['name']}: a fresh copy (the old one and its messages deleted)",
                               "POST", f"/guilds/{self.gid}/channels", body)
                nid = (new or {}).get("id") or self.new_id()
                for key in ("system_channel_id", "rules_channel_id", "public_updates_channel_id"):
                    if self.guild.get(key) == cid:
                        self.act("wiped", f"server setting {key} -> the fresh {body['name']}", "PATCH",
                                 f"/guilds/{self.gid}", {key: nid})
                        self.guild[key] = nid
                self.act("wiped", f"delete the old {c['name']}", "DELETE", f"/channels/{cid}")
                self.channels = [x for x in self.channels if x["id"] != cid] + [dict(body, id=nid)]
                self.chan_ids[name] = nid
                self.used.add(nid)

    def do_delete_archived(self):
        if not self.arch_id:
            self.note("deleted", "nothing archived to delete")
        else:
            for c in sorted(self.channels, key=okey):
                if c.get("parent_id") == self.arch_id and c["type"] != CATEGORY:
                    self.act("deleted", f"channel {c['name']} (from ARCHIVE)", "DELETE", f"/channels/{c['id']}")
            self.act("deleted", "the ARCHIVE category", "DELETE", f"/channels/{self.arch_id}")
        for cid in self.hidden_cats:
            c = next(x for x in self.channels if x["id"] == cid)
            if not any(x.get("parent_id") == cid for x in self.channels if x["type"] != CATEGORY):
                self.act("deleted", f"empty old category {c['name']!r}", "DELETE", f"/channels/{cid}")

    def manual_steps(self):
        m = []
        if self.community:
            m.append("Onboarding: Server Settings -> Onboarding. Rules: #rules. Default channels: #welcome, "
                     "#announcements, #hangout, #bug-reports. A question \"What do you do?\" with Play / Build a Den / "
                     "Host / Make mods, each pointing at its category.")
            m.append("Description: Server Settings -> Community -> Overview -> Server Description (TODO(owner)).")
        else:
            m.append("Community: Server Settings -> Enable Community (rules channel: #rules), then run this again: "
                     "it makes the forum channels and the stage. Then Server Settings -> Onboarding.")
        m.append("Changelog: #changelog -> Edit Channel -> Integrations -> Webhooks -> New Webhook \"Leyline releases\" "
                 "-> Copy Webhook URL. On GitHub: the Leyline repo -> Settings -> Webhooks -> Add webhook, the URL + "
                 "/github, application/json, events: Releases only. The URL is a secret: paste it nowhere else.")
        m.append("The bots' settings (README step 7): the lines below go in portal.env and discord.env on the server.")
        if not self.portal_bot:
            m.append("The Leyline website bot isn't in the server (no bot role with 'leyline' in its name). After you "
                     "invite it, run this again (or --leyline-bot \"<its role>\") so it may post in the read-only and "
                     "private channels.")
        m.append("Roles: Moderator and Guide are given by hand (right-click a member -> Roles). Creator and Member from "
                 "before are left alone (TODO(owner): keep or delete them).")
        m.append("First posts: paste them from docs/discord-initial-posts.md (printed below), fill in the TODO(owner) "
                 "lines, then hover -> ... -> Pin Message.")
        m.append("Last: kick the helper bot (Server Settings -> Integrations -> the bot -> Kick), then Developer Portal "
                 "-> your app -> Bot -> Reset Token (or delete the app). Delete token.txt if you made one.")
        for x in m:
            self.note("manual", x)

    def ids_for_the_bots(self):
        """The settings lines for the website bot and the game bridge (channel and tag ids; not secrets)."""
        ids = self.chan_ids
        real = lambda i: i and not str(i).startswith("new-")    # noqa: E731
        show = lambda i: i if real(i) else "<made when you run Do it>"    # noqa: E731
        forum = next((c for c in self.channels if c["id"] == ids.get("forum")), None)
        tags = {str(t.get("name", "")).lower(): t.get("id") for t in (forum or {}).get("available_tags") or []}
        parts = []
        for key, tag in FORUM_TAGS:
            tid = tags.get(tag.lower())
            if forum and real(forum["id"]):
                parts.append(f"{key}={forum['id']}" + (f"/{tid}" if tid else ""))
        if real(ids.get("announcements")):
            parts.append(f"news={ids['announcements']}")
        return [
            "In /srv/wrath/portal.env (the website bot), then restart the portal:",
            f"  WU_DISCORD_GUILD={self.gid}",
            "  WU_DISCORD_TEMPLATE=/app/discord_template_v2.json",
            "  WU_FORUM_DISCORD_CHANNELS=" + (",".join(parts) if parts else "<needs the forum channel: Community>"),
            "In /opt/wu-discord/discord.env (the game bridge), then systemctl restart wu-discord:",
            f"  DISCORD_GUILD_ID={self.gid}",
            f"  DISCORD_CHAT_CHANNEL_ID={show(ids.get('world-chat'))}",
            f"  DISCORD_BUG_CHANNEL_ID={show(ids.get('bug-reports'))}",
            "  WU_DISCORD_BUG_INTAKE=/srv/wrath/bugs/intake/discord.jsonl",
        ]

    # -- printing -----------------------------------------------------------------------------------------------------
    SECTIONS = [("server", "Server"), ("roles", "Roles"), ("created", "New channels"),
                ("reused", "Reused channels (history kept)"), ("channels", "Channel settings"),
                ("archive", "Archive"), ("kept", "Left alone"), ("order", "Order"), ("wiped", "WIPED"),
                ("deleted", "DELETED"), ("can't", "Can't do"), ("manual", "Do these by hand afterwards")]

    def report(self):
        say("")
        for key, title in self.SECTIONS:
            items = [(t, ch) for s, t, ch in self.lines if s == key]
            if key == "roles" and not self.opts.delete_old_roles:
                out, _ = self.old_roles()
                if out:
                    items.append(("old roles left alone (add --delete-old-roles to remove them): "
                                  + ", ".join(r["name"] for r, _ in out), False))
            if key == "deleted" and not self.opts.delete_archived and self.archived:
                items.append(("nothing (archived channels stay; --delete-archived would delete them)", False))
            if not items:
                continue
            say(f"{title}:")
            for t, ch in items:
                say(("  * " if ch else "    ") + t)
        say("")
        say(f"{self.changes} change(s)." if self.changes else "Nothing to change: the server is already Leyline.")


# ---------------------------------------------------------------------------------------------------------------------
# First posts: docs/discord-initial-posts.md. Printed for the owner to paste; posted only with --post-first-posts and a
# typed POST, and then only the drafts with no TODO(owner) left, never in Start here (those come from the owner).
def read_posts(path):
    """[(channel name, text)] in the file's order."""
    try:
        doc = Path(path).read_text(encoding="utf-8")
    except OSError:
        return []
    return [(m.group(1), m.group(2).strip("\n")) for m in
            re.finditer(r"(?m)^### #([a-z0-9-]+)[^\n]*\n+```\n(.*?)```", doc, re.S)]


def plan_posts(posts):
    """[(channel, text, None)] to post, and [(channel, text, why not)] to leave for the owner."""
    start = {s["name"] for s in LAYOUT[0][1]}
    types = {s["name"]: s["type"] for _, chans in LAYOUT for s in chans}
    go, leave = [], []
    for name, text in posts:
        why = None
        if name not in types:
            why = "not a channel in the plan"
        elif "TODO(owner)" in text:
            why = "has TODO(owner) lines for you to fill in"
        elif name in start:
            why = "Start here: these come from you, not a bot"
        elif types[name] == FORUM:
            why = "a forum channel: make it a post yourself"
        elif BRANDS.search(text) or MONEY.search(text) or len(text) >= 2000:
            why = "breaks the posting rules (a game name, a money link, or too long)"
        (leave if why else go).append((name, text, why))
    return go, leave


def link_channels(text, chan_ids):
    """'#hangout' -> '<#id>' for the plan's channels, so a posted draft links them."""
    def sub(m):
        cid = chan_ids.get(m.group(1))
        return f"<#{cid}>" if cid and not str(cid).startswith("new-") else m.group(0)
    return re.sub(r"(?<![\w<&/])#([a-z0-9-]+)\b", sub, text)


def post_first_posts(api, work, go):
    done = 0
    for name, text, _ in go:
        cid = work.chan_ids.get(name)
        if not cid or name in work.skipped:
            say(f"  skipped #{name}: the channel isn't there")
            continue
        content = link_channels(text, work.chan_ids)
        recent = api.request("GET", f"/channels/{cid}/messages", query={"limit": 50}) or []
        if any((m.get("content") or "").strip() == content.strip() for m in recent):
            say(f"  #{name}: already posted")
            continue
        msg = api.request("POST", f"/channels/{cid}/messages",
                          {"content": content, "allowed_mentions": {"parse": []}, "flags": SUPPRESS_EMBEDS})
        try:
            api.request("PUT", f"/channels/{cid}/pins/{msg['id']}")
        except (ApiError, KeyError, TypeError):
            pass
        say(f"  posted and pinned in #{name}")
        done += 1
    return done


def print_posts(posts_list, title):
    if not posts_list:
        return
    say("")
    say(title)
    for name, text, why in posts_list:
        say("")
        say(f"----- #{name}" + (f"  ({why})" if why else "") + " -----")
        say(text)
    say("-----")


# ---------------------------------------------------------------------------------------------------------------------
def desktop():
    root = os.environ.get("LEYLINE_BACKUP_ROOT")
    if root:
        return Path(root)
    home = Path(os.environ.get("USERPROFILE") or Path.home())
    for d in (Path(os.environ["OneDrive"]) / "Desktop" if os.environ.get("OneDrive") else None, home / "Desktop"):
        if d and d.is_dir():
            return d
    return home


def safe_name(s):
    return re.sub(r"[^0-9A-Za-z_-]+", "", slug(s)) or "channel"


def backup(api, gid, n_messages):
    stamp = _dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = desktop() / f"Leyline-discord-backup-{stamp}"
    k = 1
    while out.exists():
        k += 1
        out = desktop() / f"Leyline-discord-backup-{stamp}-{k}"
    (out / "messages").mkdir(parents=True)

    def dump(name, obj):
        (out / name).write_text(json.dumps(obj, indent=1, ensure_ascii=False), encoding="utf-8")

    guild = api.request("GET", f"/guilds/{gid}")
    roles = api.request("GET", f"/guilds/{gid}/roles")
    channels = api.request("GET", f"/guilds/{gid}/channels")
    dump("guild.json", guild)
    dump("roles.json", roles)
    dump("channels.json", channels)          # includes every channel's permission_overwrites
    skipped = []
    if n_messages > 0:
        texty = [c for c in sorted(channels, key=okey) if c["type"] in MESSAGE_TYPES]
        for i, c in enumerate(texty, 1):
            say(f"  backing up messages {i}/{len(texty)}: {c['name']}")
            msgs, before = [], None
            try:
                while len(msgs) < n_messages:
                    q = {"limit": min(100, n_messages - len(msgs))}
                    if before:
                        q["before"] = before
                    page = api.request("GET", f"/channels/{c['id']}/messages", query=q)
                    if not page:
                        break
                    msgs += page
                    before = page[-1]["id"]
                    if len(page) < q["limit"]:
                        break
            except ApiError as e:
                skipped.append(f"{c['name']}: {e}")
            slim = [{"id": m["id"],
                     "author": (m.get("author") or {}).get("global_name") or (m.get("author") or {}).get("username"),
                     "author_id": (m.get("author") or {}).get("id"),
                     "time": m.get("timestamp"), "edited": m.get("edited_timestamp"),
                     "content": m.get("content"), "pinned": m.get("pinned", False),
                     "attachments": [a.get("url") for a in m.get("attachments") or []]} for m in msgs]
            (out / "messages" / f"{safe_name(c['name'])}-{c['id']}.json").write_text(
                json.dumps({"channel": c["name"], "id": c["id"], "messages": slim}, indent=1, ensure_ascii=False),
                encoding="utf-8")
    if skipped:
        dump("messages-skipped.json", skipped)
    return out


def read_token(path):
    tok = ""
    if path and Path(path).is_file():
        tok = Path(path).read_text(encoding="utf-8-sig").strip()
        if tok:
            say(f"Using the bot token from {Path(path).name} (not shown).")
    if not tok:
        say("Paste the bot token (Developer Portal -> your app -> Bot -> Reset Token -> Copy).")
        say("It won't show while you paste. Press Enter after.")
        tok = getpass.getpass("Bot token: ").strip()
    tok = tok.strip().strip('"').strip("'")
    if tok.lower().startswith("bot "):
        tok = tok[4:].strip()
    if not tok or re.search(r"\s", tok):
        raise SystemExit("That doesn't look like a bot token. Copy it again from the Developer Portal.")
    Out.secret = tok
    return tok


def ask(prompt):
    try:
        return input(prompt)
    except EOFError:
        return ""


def pick_guild(api, given):
    if given:
        return str(given)
    guilds = api.request("GET", "/users/@me/guilds")
    if not guilds:
        raise SystemExit("The bot isn't in any server yet. Invite it first (README step 2), then run this again.")
    if len(guilds) == 1:
        say(f"The bot is in one server: {guilds[0]['name']} (id {guilds[0]['id']}). Using that.")
        return guilds[0]["id"]
    say("The bot is in more than one server:")
    for g in guilds:
        say(f"  {g['id']}  {g['name']}")
    gid = ask("Type the id of the server to change: ").strip()
    if gid not in {g["id"] for g in guilds}:
        raise SystemExit("That id isn't one of them. Nothing changed.")
    return gid


def main(argv=None):
    for s in (sys.stdout, sys.stderr):
        try:
            s.reconfigure(errors="replace")
        except (AttributeError, ValueError):
            pass
    ap = argparse.ArgumentParser(description="Redo a Discord server to the Leyline channel plan (owner-run).")
    ap.add_argument("--apply", action="store_true", help="make the changes (asks you to type YES). Without it: preview")
    ap.add_argument("--guild", help="server id (default: the one server the bot is in, or it asks)")
    ap.add_argument("--backup-messages", type=int, default=1000, metavar="N",
                    help="messages to back up per text channel (default 1000; 0 skips messages)")
    ap.add_argument("--post-first-posts", action="store_true",
                    help="let the bot post the first-post drafts that have no TODO(owner) left (asks you to type POST)")
    ap.add_argument("--posts-file", default=str(POSTS_FILE), help=argparse.SUPPRESS)
    ap.add_argument("--leyline-bot", metavar="ROLE",
                    help="the website bot's role name, if it has no 'leyline' in it")
    ap.add_argument("--delete-archived", action="store_true",
                    help="after the backup, delete the archived channels (asks you to type the server name)")
    ap.add_argument("--wipe-history", action="store_true",
                    help="replace every Leyline text channel with a fresh copy, deleting its messages (type WIPE)")
    ap.add_argument("--delete-old-roles", action="store_true",
                    help="delete roles that aren't in the new set (asks you to type DELETE ROLES)")
    ap.add_argument("--plain", action="store_true", help=argparse.SUPPRESS)     # names are always plain now
    ap.add_argument("--token-file", default=str(HERE / "token.txt"), help=argparse.SUPPRESS)
    o = ap.parse_args(argv)

    try:
        tok = read_token(o.token_file)
        api = Discord(tok)
        try:
            me = api.request("GET", "/users/@me")
        except ApiError as e:
            if e.status == 401:
                raise SystemExit("Discord didn't accept that token. In the Developer Portal: Bot -> Reset Token, "
                                 "copy the new one, and try again.") from None
            raise
        say(f"Signed in as the bot {me.get('username')}.")
        gid = pick_guild(api, o.guild)

        posts = read_posts(o.posts_file)
        go, leave = plan_posts(posts)

        plan = Rebrand(api, gid, o, apply=False)
        plan.run()
        old_name = plan.original_name
        say("")
        say(("PREVIEW (nothing will change)" if not o.apply else "PLAN") + f" for the server {old_name!r}:")
        plan.report()
        if o.post_first_posts:
            say("")
            say("First posts the bot would post and pin (only after you type POST):" if go else
                "First posts: none can be posted by the bot (each one has TODO(owner) lines or is yours to post).")
            for name, _, _ in go:
                say(f"  * #{name}")
            for name, _, why in leave:
                say(f"    #{name}: left for you ({why})")
        elif not posts:
            say(f"\n(The first-post drafts weren't found at {o.posts_file}.)")

        if not o.apply:
            say("")
            say("This was a preview. Nothing changed. To do it: run 'Leyline Discord - do it.bat'.")
            say(f"Before any change it backs up the server to your Desktop (last {o.backup_messages} messages "
                "per channel). It posts nothing; it prints the first posts for you to paste yourself.")
            return 0

        if plan.changes:
            if ask(f"\nType YES to make these {plan.changes} change(s) to {old_name!r}: ").strip() != "YES":
                say("Stopped. Nothing changed.")
                return 1
        if o.delete_old_roles:
            if ask("You asked to DELETE the old roles listed above. Type DELETE ROLES to confirm: ").strip() \
                    != "DELETE ROLES":
                say("Stopped. Nothing changed.")
                return 1
        if o.wipe_history:
            say("You asked to WIPE every channel's messages. The backup keeps only the last "
                f"{o.backup_messages} messages of each channel.")
            if ask("Type WIPE to confirm: ").strip() != "WIPE":
                say("Stopped. Nothing changed.")
                return 1
        if o.delete_archived:
            say("You asked to DELETE the archived channels. The backup keeps only their last "
                f"{o.backup_messages} messages each{' (NONE: --backup-messages 0)' if o.backup_messages <= 0 else ''}.")
            if ask(f"Type the server's name ({old_name}) to confirm: ").strip() != old_name.strip() or not old_name:
                say("Stopped. Nothing changed.")
                return 1
        do_post = False
        if o.post_first_posts and go:
            say(f"\nThe bot would post and pin {len(go)} first post(s) in its own name: "
                + ", ".join("#" + n for n, _, _ in go) + ".")
            do_post = ask("Type POST to let it, or press Enter to skip and paste them yourself: ").strip() == "POST"
            if not do_post:
                say("OK: the bot won't post. They're printed at the end for you.")

        say("\nBacking up first...")
        folder = backup(api, gid, max(0, o.backup_messages))
        say(f"Backup: {folder}")

        api.allow_writes = True
        say("\nChanging the server...")
        work = Rebrand(api, gid, o, apply=True)
        work.run()
        (folder / "what-changed.txt").write_text(
            "\n".join(f"[{s}] {'* ' if c else ''}{t}" for s, t, c in work.lines) + "\n", encoding="utf-8")
        say(f"Done: {work.changes} change(s)." if work.changes else "Nothing needed changing.")

        posted = 0
        if do_post:
            say("\nPosting the first posts you allowed...")
            posted = post_first_posts(api, work, go)
        to_print = leave if do_post else [(n, t, w) for n, t, w in go + leave]
        to_print.sort(key=lambda p: [n for n, _ in posts].index(p[0]))
        if to_print:
            (folder / "first-posts.txt").write_text(
                "\n\n".join(f"----- #{n} -----\n{t}" for n, t, _ in to_print) + "\n", encoding="utf-8")
            print_posts(to_print, "First posts for YOU to paste (fill in the TODO(owner) lines, then pin each one). "
                                  f"Also saved in {folder / 'first-posts.txt'}:")

        ids = work.ids_for_the_bots()
        (folder / "settings-for-the-bots.txt").write_text("\n".join(ids) + "\n", encoding="utf-8")
        say("")
        say("Settings for the bots (channel ids, not secrets; also in settings-for-the-bots.txt):")
        for line in ids:
            say("  " + line)
        say("")
        say("Do these by hand:")
        for s, t, _ in work.lines:
            if s in ("manual", "can't"):
                say("  - " + t)
        if posted:
            say(f"  - (the bot posted {posted} first post(s); check them, and edit or delete any you don't want)")
        return 0
    except ApiError as e:
        say(f"\nStopped: {e}")
        if e.status == 403:
            say("The bot needs the Administrator permission, and its role must be at the top of the role list "
                "(Server Settings -> Roles: drag the bot's role to the top). Then run this again; it carries on.")
        return 2
    except KeyboardInterrupt:
        say("\nStopped.")
        return 130


if __name__ == "__main__":
    sys.exit(main())
