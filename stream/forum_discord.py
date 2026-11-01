"""The forum and the Discord, linked (stream/forum.py loads this beside itself). Owner, 2026-10-31: "merge forum with
discord somehow as the backend and talking".

OUR forum stays the source of truth: accounts, moderation, bans, blocked words and rate limits all stay ours
(stream/forum.py, stream/admin.py, Leyline HQ). Discord is a live window onto it:

  forum -> Discord   each mapped category has a Discord channel (a Forum channel, or a text channel whose threads we
                     use). A new forum thread makes a Discord thread with its first post; replies go into it as the bot:
                     the author's name, a link to the post, the text (no pings, no embeds, markdown shown as typed).
  Discord -> forum   replies in those threads come back as forum posts shown "via Discord · <name>", ONLY from Discord
                     users who linked their Leyline account (/link in the Discord, or the website's Discord sign-in).
                     They go through the same rules as a site post (store.reply: word and brand filters, rate limits,
                     locked threads) and the same sanctions (admin.blocked: a ban, mute or forum mute in HQ applies).
                     A refused or unlinked post is taken off the Discord (WU_FORUM_DISCORD_UNLINKED=keep leaves it) and
                     the person gets one private note: a DM (Discord allows "only you can see this" answers to slash
                     commands only), or, with DMs closed, a reply that pings only them and goes after 2 minutes.
                     A new thread a linked person starts in a mapped Discord Forum channel becomes a forum thread.
  moderation         hide / show a post, hide / show / lock / unlock / merge a thread in HQ -> the Discord copy is
                     edited to "removed" (our own messages) or deleted (a Discord person's), the thread deleted, locked
                     or locked with a "merged into" note. Deleted in Discord (a message, or a whole thread) -> hidden on
                     the forum (WU_FORUM_DISCORD_DELETE_HIDES=0 turns that off), written to HQ's audit log as "discord".
                     Who may delete others' messages in Discord is whoever has Manage Messages there: the Discord's mods.
  reliability        every change for Discord goes to an outbox table in forum.sqlite3 first; a worker thread sends it
                     in order per thread, with Discord's rate-limit headers obeyed, 429s waited out, and back-off
                     retries (5 s doubling to 15 min; missing permissions retry hourly). Discord down = it waits; nothing
                     is dropped except what Discord says can never work (a 400) or what is gone (a 404).

Discord is read with plain REST (no gateway): every WU_FORUM_DISCORD_POLL seconds one call lists the guild's active
threads, and only threads with new messages are read. Deletions show as a drop in a thread's message count (read at
once) and in a slow sweep (every 10 minutes, threads active in the last 7 days). /link and /unlink arrive at our
Interactions Endpoint URL (POST /wrath/forum/api/discord/interactions), checked with the app's Ed25519 public key.

Pages: GET/POST /wrath/leyline/forum/discord[?c=<code>]  link or unlink (signed in on the website).

Settings (portal.env; OFF unless WU_FORUM_DISCORD=1 and the rest is complete; the token is never printed):
  WU_FORUM_DISCORD=1            on
  DISCORD_BOT_TOKEN             the bot's token (WU_FORUM_DISCORD_TOKEN wins when both are set)
  WU_FORUM_DISCORD_CHANNELS     category=channel id[/forum tag id], comma-separated, e.g. help=123,general=456/789
  DISCORD_APP_ID, DISCORD_PUBLIC_KEY   for /link and /unlink (without them, linking is by the website sign-in only)
  WU_FORUM_DISCORD_POLL (5), WU_FORUM_DISCORD_UNLINKED (remove|keep), WU_FORUM_DISCORD_DELETE_HIDES (1),
  WU_DISCORD_API (tests). docs/FORUM.md ("The Discord link") has the owner's setup list and the bot's permissions.
Standard library only (plus ed25519_pure.py, shipped beside hostflow.py).
"""
from __future__ import annotations

import hashlib
import hmac
import html
import importlib.util
import json
import logging
import os
import re
import secrets
import socket
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

log = logging.getLogger("wrath-portal.forum-discord")

HERE = os.path.dirname(os.path.abspath(__file__))
API_DEFAULT = "https://discord.com/api/v10"
DISCORD_EPOCH_MS = 1420070400000
MSG_MAX = 2000
TITLE_MAX = 100
CODE_TTL = 900                 # a /link code: 15 minutes, used once
NOTICE_EVERY = 7 * 86400       # the "link your account" note: once a week at most per Discord person
DM_GAP = 60                    # at most one private note a minute per Discord person
SWEEP_EVERY = 600
SWEEP_DAYS = 7
SWEEP_PER_ROUND = 10
FALLBACK_TTL = 120             # the in-thread note for closed DMs is deleted after this
CHANNEL_FORUM, CHANNEL_TEXT, CHANNEL_NEWS = 15, 0, 5
REMOVED = "*This post was removed by a moderator.*"
SNOW_RE = re.compile(r"^\d{5,22}$")
EPHEMERAL = 64
SUPPRESS_EMBEDS = 4
COMMANDS = [{"name": "link", "type": 1, "description": "Link your Leyline account, so your replies here reach the forum"},
            {"name": "unlink", "type": 1, "description": "Unlink your Leyline account from this Discord account"}]


def snowflake_ms(sid) -> int:
    try:
        return (int(sid) >> 22) + DISCORD_EPOCH_MS
    except (TypeError, ValueError):
        return 0


def _load_ed25519():
    for p in (os.path.join(HERE, "ed25519_pure.py"), os.path.join(HERE, "..", "hosting", "kit", "tools", "wunet", "ed25519_pure.py")):
        if os.path.isfile(p):
            spec = importlib.util.spec_from_file_location("wu_fd_ed25519", p)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            return mod
    return None


# --------------------------------------------------------------------------------------------- settings
class Config:
    def __init__(self, env=None, cats=()):
        env = os.environ if env is None else env
        self.on = env.get("WU_FORUM_DISCORD", "0").strip() == "1"
        self.token = (env.get("WU_FORUM_DISCORD_TOKEN") or env.get("DISCORD_BOT_TOKEN") or "").strip()
        self.app_id = env.get("DISCORD_APP_ID", "").strip()
        self.public_key = env.get("DISCORD_PUBLIC_KEY", "").strip().lower()
        self.bad = []
        self.channels: dict[str, tuple[str, str]] = {}
        for part in env.get("WU_FORUM_DISCORD_CHANNELS", "").split(","):
            part = part.strip()
            if not part:
                continue
            cat, _, val = part.partition("=")
            ch, _, tag = val.strip().partition("/")
            cat, ch, tag = cat.strip(), ch.strip(), tag.strip()
            if cats and cat not in cats:
                self.bad.append(f"WU_FORUM_DISCORD_CHANNELS: {cat!r} is not a forum category ({', '.join(cats)})")
            elif not SNOW_RE.fullmatch(ch) or (tag and not SNOW_RE.fullmatch(tag)):
                self.bad.append(f"WU_FORUM_DISCORD_CHANNELS: {cat}= needs a channel id (digits), optionally /<forum tag id>")
            else:
                self.channels[cat] = (ch, tag)
        try:
            self.poll = max(2.0, float(env.get("WU_FORUM_DISCORD_POLL", "5") or 5))
        except ValueError:
            self.poll = 5.0
        self.remove = env.get("WU_FORUM_DISCORD_UNLINKED", "remove").strip().lower() != "keep"
        self.delete_hides = env.get("WU_FORUM_DISCORD_DELETE_HIDES", "1").strip() != "0"
        self.api = env.get("WU_DISCORD_API", API_DEFAULT).strip().rstrip("/")
        self.guild_id = env.get("WU_DISCORD_GUILD", "").strip()   # the server (stream/discord_growth.py); else from the channels

    def problems(self) -> list[str]:
        p = list(self.bad)
        if not self.token:
            p.append("DISCORD_BOT_TOKEN is not set")
        if self.guild_id and not SNOW_RE.fullmatch(self.guild_id):
            p.append("WU_DISCORD_GUILD is the server id (digits)")
        if not self.channels and not self.guild_id:
            p.append("WU_FORUM_DISCORD_CHANNELS maps no category to a channel (or set WU_DISCORD_GUILD for the bot alone)")
        if (self.app_id or self.public_key) and not (SNOW_RE.fullmatch(self.app_id) and re.fullmatch(r"[0-9a-f]{64}", self.public_key)):
            p.append("DISCORD_APP_ID (digits) and DISCORD_PUBLIC_KEY (64 hex characters) go together")
        return p

    def __repr__(self):   # never the token
        return (f"Config(on={self.on}, channels={self.channels}, slash={'yes' if self.app_id else 'no'}, poll={self.poll}, "
                f"remove={self.remove}, delete_hides={self.delete_hides})")


# --------------------------------------------------------------------------------------------- the Discord REST API
class DiscordError(Exception):
    """kind: auth (401), forbidden (403), notfound (404), bad (400 and other 4xx), ratelimited (429), down (5xx, network)."""

    def __init__(self, kind: str, status: int = 0, code: int = 0, retry_after: float = 0.0, detail: str = ""):
        super().__init__(f"{kind}: {detail or status}")
        self.kind, self.status, self.code, self.retry_after, self.detail = kind, status, code, retry_after, detail


class Api:
    """Discord REST with its rate limits obeyed: X-RateLimit-Remaining 0 blocks that route until Reset-After, a 429
    is waited out (once, when short) or handed back for the outbox to retry later, a global 429 blocks every route, and
    calls are spaced a little (MIN_GAP). The token only ever goes in the Authorization header."""
    MIN_GAP = 0.05
    MAX_WAIT = 5.0

    def __init__(self, token: str, base: str, urlopen=urllib.request.urlopen, sleep=time.sleep, clock=time.monotonic,
                 site: str = "https://play.openvanilla.app"):
        self._token, self.base, self._open, self._sleep, self._clock = token, base, urlopen, sleep, clock
        self.ua = f"DiscordBot ({site}, 1) leyline-forum"
        self.until: dict[str, float] = {}
        self.global_until = 0.0
        self.last = 0.0
        self.calls = 0
        self.lock = threading.Lock()

    def __repr__(self):
        return f"Api(base={self.base})"

    @staticmethod
    def route(method: str, path: str) -> str:
        p = path.split("?")[0]
        m = re.match(r"^/(channels|guilds|webhooks)/\d+", p)
        major = m.group(0) if m else ""
        return method + " " + major + re.sub(r"\d{5,}", "{id}", p[len(major):])

    def call(self, method: str, path: str, body=None):
        key = self.route(method, path)
        data = json.dumps(body).encode() if body is not None else None
        with self.lock:
            for attempt in range(2):
                wait = max(self.global_until, self.until.get(key, 0.0)) - self._clock()
                if wait > self.MAX_WAIT:
                    raise DiscordError("ratelimited", 429, retry_after=wait, detail=f"waiting on {key}")
                if wait > 0:
                    self._sleep(wait)
                gap = self.last + self.MIN_GAP - self._clock()
                if gap > 0:
                    self._sleep(gap)
                self.last = self._clock()
                self.calls += 1
                req = urllib.request.Request(self.base + path, data=data, method=method, headers={
                    "Authorization": "Bot " + self._token, "User-Agent": self.ua, "Content-Type": "application/json"})
                try:
                    with self._open(req, timeout=15) as r:
                        self._headers(key, r.headers)
                        raw = r.read()
                        return json.loads(raw) if raw else None
                except urllib.error.HTTPError as e:
                    try:
                        doc = json.loads(e.read() or b"{}")
                    except ValueError:
                        doc = {}
                    doc = doc if isinstance(doc, dict) else {}
                    if e.code == 429:
                        try:
                            ra = float(doc.get("retry_after") or (e.headers.get("Retry-After") if e.headers else 0) or 1)
                        except (TypeError, ValueError):
                            ra = 1.0
                        ra = min(max(ra, 0.05), 3600.0)
                        if doc.get("global"):
                            self.global_until = self._clock() + ra
                        else:
                            self.until[key] = self._clock() + ra
                        log.info("Discord rate limit on %s: %.1f s", key, ra)
                        if attempt == 0 and ra <= self.MAX_WAIT:
                            continue
                        raise DiscordError("ratelimited", 429, retry_after=ra, detail=key) from None
                    if e.headers:
                        self._headers(key, e.headers)
                    kind = {400: "bad", 401: "auth", 403: "forbidden", 404: "notfound"}.get(e.code, "down" if e.code >= 500 else "bad")
                    raise DiscordError(kind, e.code, int(doc.get("code") or 0) if str(doc.get("code") or "0").isdigit() else 0,
                                       detail=f"HTTP {e.code} on {key}") from None
                except (urllib.error.URLError, socket.timeout, OSError, ValueError) as e:
                    raise DiscordError("down", detail=type(e).__name__) from None
        raise DiscordError("ratelimited", 429, retry_after=1.0, detail=key)

    def _headers(self, key: str, h) -> None:
        try:
            rem, ra = h.get("X-RateLimit-Remaining"), h.get("X-RateLimit-Reset-After")
            if rem is not None and ra is not None and int(float(rem)) <= 0:
                self.until[key] = self._clock() + float(ra)
        except (TypeError, ValueError, AttributeError):
            pass


# --------------------------------------------------------------------------------------------- text
_MD = re.compile(r"([\\*_~`|>#\[\]()<:-])")
ZWSP = "​"


def defuse(s: str) -> str:
    s = re.sub(r"@(everyone|here)", "@" + ZWSP + r"\1", s, flags=re.I)
    return re.sub(r"<(@[!&]?|#)(\d+)>", lambda m: m.group(1).replace("@", "@" + ZWSP) + m.group(2), s)


def esc(s: str) -> str:
    return _MD.sub(r"\\\1", defuse(s or ""))


def from_discord(msg: dict) -> str:
    """A Discord message as the forum's plain text: mentions as names, no markdown marks, files named."""
    s = msg.get("content") or ""
    names = {}
    for u in msg.get("mentions") or []:
        names[str(u.get("id"))] = ((u.get("member") or {}).get("nick")) or u.get("global_name") or u.get("username") or "someone"
    s = re.sub(r"<@!?(\d+)>", lambda m: "@" + names.get(m.group(1), "someone"), s)
    s = re.sub(r"<@&\d+>", "", s)
    s = re.sub(r"<#\d+>", "#channel", s)
    s = re.sub(r"<a?:(\w{1,32}):\d+>", r":\1:", s)
    s = re.sub(r"<t:(\d+)(?::\w)?>", lambda m: time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime(int(m.group(1)))), s)
    s = re.sub(r"\[([^\]\n]{1,200})\]\(<?(https?://[^\s)>]+)>?\)", r"\1 (\2)", s)    # [text](url) -> text (url)
    s = re.sub(r"<(https?://[^\s>]+)>", r"\1", s)
    s = re.sub(r"(\*\*|__|~~|\|\||`{1,3})", "", s)
    s = re.sub(r"@(everyone|here)\b", "", s, flags=re.I)
    s = re.sub(r"\\([\\*_~`|>#\[\]()<:-])", r"\1", s)
    for a in msg.get("attachments") or []:
        fn = re.sub(r"[^\w .-]", "", str(a.get("filename") or "file"))[:80] or "file"
        s += f"\n[a file on Discord: {fn}]"
    if msg.get("sticker_items"):
        s += "\n[a sticker]"
    return s.strip()


# --------------------------------------------------------------------------------------------- the link
SCHEMA = """
CREATE TABLE IF NOT EXISTS fd_meta (k TEXT PRIMARY KEY, v TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS fd_threads (
  dthread TEXT PRIMARY KEY, thread INTEGER NOT NULL DEFAULT 0, channel TEXT NOT NULL, state TEXT NOT NULL,
  last_seen TEXT NOT NULL DEFAULT '0', msg_count INTEGER NOT NULL DEFAULT -1, active INTEGER NOT NULL DEFAULT 0,
  swept INTEGER NOT NULL DEFAULT 0);
CREATE INDEX IF NOT EXISTS fd_threads_thread ON fd_threads (thread, state);
CREATE TABLE IF NOT EXISTS fd_posts (
  post INTEGER PRIMARY KEY, dthread TEXT NOT NULL, dmsg TEXT NOT NULL, origin TEXT NOT NULL, state TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS fd_posts_dmsg ON fd_posts (dmsg);
CREATE INDEX IF NOT EXISTS fd_posts_dthread ON fd_posts (dthread, state);
CREATE TABLE IF NOT EXISTS fd_links (discord TEXT PRIMARY KEY, acct TEXT NOT NULL UNIQUE, dname TEXT NOT NULL, at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS fd_codes (code TEXT PRIMARY KEY, discord TEXT NOT NULL, dname TEXT NOT NULL, expires INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS fd_notices (discord TEXT PRIMARY KEY, at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS fd_outbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, thread INTEGER NOT NULL DEFAULT 0, data TEXT NOT NULL,
  created INTEGER NOT NULL, next_at INTEGER NOT NULL, tries INTEGER NOT NULL DEFAULT 0, error TEXT NOT NULL DEFAULT '');
"""
# fd_threads.state: live (ours, in use), merged, deleted (we deleted it: a hidden thread), gone (deleted in Discord),
#                   ignored (a Discord thread that isn't the forum's), refused (a Discord thread we took down)
# fd_posts.origin:  site (we posted it), discord (a linked person's own message), mirror (we re-posted theirs)
# fd_posts.state:   live, masked (our message edited to REMOVED), gone (no longer on Discord)


class Link:
    def __init__(self, fm, store, cfg: Config, api: Api, clock=time.time):
        self.fm, self.store, self.cfg, self.api, self.clock = fm, store, cfg, api, clock
        self.bot_id = ""
        self.guild = ""
        self.ch_type: dict[str, int] = {}
        self.started = False
        self.state = "starting"
        self.last_error = ""
        self.dm_channels: dict[str, str] = {}
        self.dm_last: dict[str, float] = {}
        self.next_poll = 0.0
        self.next_sweep = 0.0
        self.local = threading.local()
        self.ed = _load_ed25519()
        self.stop = threading.Event()
        self.growth = None   # stream/discord_growth.py: setup, live realms, picks, Find a group, roles, AutoMod
        with store.lock:
            store.db.executescript(SCHEMA)
        if not self.meta("since"):
            self.meta("since", str(int(self.clock() * 1000)))   # Discord threads older than the link are never imported
        store.listeners.append(self.on_forum)

    # -- small helpers ---------------------------------------------------------------------------------------------
    def now(self) -> int:
        return int(self.clock())

    def meta(self, k: str, v: str | None = None) -> str:
        if v is None:
            r = self.store.one("SELECT v FROM fd_meta WHERE k = ?", (k,))
            return r[0] if r else ""
        self.store.x("INSERT OR REPLACE INTO fd_meta (k, v) VALUES (?, ?)", (k, v))
        return v

    def site(self) -> str:
        return "https://" + self.fm._public_host()

    def post_url(self, tid: int, pid: int | None = None) -> str:
        return f"{self.site()}{self.fm.PAGE}/t/{tid}" + (f"#p{pid}" if pid else "")

    def link_url(self, code: str = "") -> str:
        return f"{self.site()}{self.fm.PAGE}/discord" + (f"?c={code}" if code else "")

    def cat_channel(self, cat: str):
        return self.cfg.channels.get(cat)

    def thread_row(self, dthread: str):
        r = self.store.one("SELECT dthread, thread, channel, state, last_seen, msg_count, active, swept FROM fd_threads WHERE dthread = ?", (str(dthread),))
        return self._row(r)

    def live_for(self, tid: int):
        r = self.store.one("SELECT dthread, thread, channel, state, last_seen, msg_count, active, swept FROM fd_threads "
                           "WHERE thread = ? AND state = 'live' ORDER BY rowid DESC LIMIT 1", (int(tid),))
        return self._row(r)

    @staticmethod
    def _row(r):
        if not r:
            return None
        return {"dthread": r[0], "thread": r[1], "channel": r[2], "state": r[3], "last_seen": r[4], "msg_count": r[5],
                "active": r[6], "swept": r[7]}

    def set_thread_row(self, dthread: str, **kw) -> None:
        sets = ", ".join(f"{k} = ?" for k in kw)
        self.store.x(f"UPDATE fd_threads SET {sets} WHERE dthread = ?", (*kw.values(), str(dthread)))

    def post_row(self, pid: int):
        r = self.store.one("SELECT post, dthread, dmsg, origin, state FROM fd_posts WHERE post = ?", (int(pid),))
        return {"post": r[0], "dthread": r[1], "dmsg": r[2], "origin": r[3], "state": r[4]} if r else None

    def put_post(self, pid: int, dthread: str, dmsg: str, origin: str, state: str = "live") -> None:
        self.store.x("INSERT OR REPLACE INTO fd_posts (post, dthread, dmsg, origin, state) VALUES (?,?,?,?,?)",
                     (int(pid), str(dthread), str(dmsg), origin, state))

    def linked(self, discord_id: str) -> str:
        r = self.store.one("SELECT acct FROM fd_links WHERE discord = ?", (str(discord_id),))
        return r[0] if r else ""

    def link_of(self, acct: str):
        r = self.store.one("SELECT discord, dname, at FROM fd_links WHERE acct = ?", (acct,))
        return {"discord": r[0], "dname": r[1], "at": r[2]} if r else None

    # -- linking ---------------------------------------------------------------------------------------------------
    def new_code(self, discord_id: str, dname: str) -> str:
        t = self.now()
        self.store.x("DELETE FROM fd_codes WHERE expires < ? OR discord = ?", (t, str(discord_id)))
        code = secrets.token_urlsafe(18)
        self.store.x("INSERT INTO fd_codes (code, discord, dname, expires) VALUES (?,?,?,?)", (code, str(discord_id), dname[:64], t + CODE_TTL))
        return code

    def code_info(self, code: str):
        if not re.fullmatch(r"[A-Za-z0-9_-]{16,40}", code or ""):
            return None
        r = self.store.one("SELECT discord, dname, expires FROM fd_codes WHERE code = ?", (code,))
        return {"discord": r[0], "dname": r[1]} if r and r[2] >= self.now() else None

    def do_link(self, acct: str, discord_id: str, dname: str, code: str = "") -> None:
        with self.store.lock:
            self.store.x("DELETE FROM fd_links WHERE acct = ? OR discord = ?", (acct, str(discord_id)))
            self.store.x("INSERT INTO fd_links (discord, acct, dname, at) VALUES (?,?,?,?)", (str(discord_id), acct, dname[:64], self.now()))
            if code:
                self.store.x("DELETE FROM fd_codes WHERE code = ?", (code,))
            self.store.x("DELETE FROM fd_notices WHERE discord = ?", (str(discord_id),))
        log.info("forum-discord: an account linked its Discord")

    def unlink(self, acct: str = "", discord_id: str = "") -> bool:
        return bool(self.store.x("DELETE FROM fd_links WHERE acct = ? OR discord = ?", (acct or "-", str(discord_id) or "-")))

    # -- the forum's changes -> the outbox -------------------------------------------------------------------------
    def enqueue(self, kind: str, thread: int = 0, delay: int = 0, **data) -> None:
        t = self.now()
        self.store.x("INSERT INTO fd_outbox (kind, thread, data, created, next_at) VALUES (?,?,?,?,?)",
                     (kind, int(thread or 0), json.dumps(data, separators=(",", ":")), t, t + delay))

    def mapped(self, tid) -> dict | None:
        th = self.store.thread(tid)
        return th if th and th["cat"] in self.cfg.channels else None

    def on_forum(self, kind: str, d: dict) -> None:
        """Forum.listeners: called by the store after each change. Our own changes made for Discord are not echoed."""
        if getattr(self.local, "from_discord", False):
            return
        if kind == "thread.new" and d.get("via") != "discord" and self.mapped(d["thread"]):
            self.enqueue("thread.new", d["thread"])
        elif kind == "post.new" and d.get("via") != "discord" and self.mapped(d["thread"]):
            self.enqueue("post.new", d["thread"], post=d["post"])
        elif kind == "post.hidden" and self.mapped(d["thread"]):
            self.enqueue("post.hide" if d["on"] else "post.show", d["thread"], post=d["post"])
        elif kind == "thread.hidden" and self.mapped(d["thread"]):
            self.enqueue("thread.delete" if d["on"] else "thread.new", d["thread"])
        elif kind == "thread.locked" and self.mapped(d["thread"]):
            self.enqueue("thread.lock", d["thread"], on=bool(d["on"]))
        elif kind == "thread.merge" and self.mapped(d["thread"]):
            self.enqueue("thread.merge", d["thread"], into=d["into"])

    # -- the outbox -> Discord -------------------------------------------------------------------------------------
    @staticmethod
    def backoff(tries: int, kind: str, retry_after: float = 0.0) -> int:
        if kind == "forbidden":
            return 3600
        return int(max(retry_after, min(5 * 2 ** max(tries - 1, 0), 900)))

    def drain(self, limit: int = 50) -> int:
        """Send what is due, oldest first; a thread's later items wait while an earlier one is waiting (order kept)."""
        rows = self.store.q("SELECT id, kind, thread, data, tries, next_at FROM fd_outbox ORDER BY id LIMIT 500")
        held, done, t = set(), 0, self.now()
        for oid, kind, thread, data, tries, next_at in rows:
            key = thread or ("x", oid)
            if key in held:
                continue
            if next_at > t:
                held.add(key)
                continue
            if done >= limit:
                break
            try:
                self.deliver(kind, thread, json.loads(data))
            except DiscordError as e:
                if e.kind == "auth":
                    raise
                if e.kind == "bad":
                    log.warning("forum-discord: Discord refused %s for thread %s for good (%s); dropped", kind, thread, e.detail)
                    self.meta("last_drop", f"{kind} {thread}: {e.detail}")
                    self.store.x("DELETE FROM fd_outbox WHERE id = ?", (oid,))
                    continue
                held.add(key)
                self.last_error = f"{kind}: {e.kind} ({e.detail})"
                self.store.x("UPDATE fd_outbox SET tries = tries + 1, next_at = ?, error = ? WHERE id = ?",
                             (t + self.backoff(tries + 1, e.kind, e.retry_after), f"{e.kind}: {e.detail}"[:200], oid))
                continue
            self.store.x("DELETE FROM fd_outbox WHERE id = ?", (oid,))
            done += 1
        return done

    def _content(self, p: dict, th: dict, opener: bool = False) -> str:
        name = esc(p["name"]) + (f" \\#{esc(p['tag'])}" if p.get("tag") else "")
        if p.get("via") == "discord":
            name += " · via Discord"
        link = f"[open](<{self.post_url(th['id'], p['id'])}>)"
        head = f"**{name}** started this on the Leyline forum · {link}" if opener else f"**{name}** · {link}"
        if p.get("hidden"):
            return head + "\n" + REMOVED
        body = []
        for part in self.fm.parts(p["body"]):
            body.append(part["u"] if "u" in part else esc(part["t"]))
        text = "".join(body)
        room = MSG_MAX - len(head) - 1
        if len(text) > room:
            tail = "…\n*(the rest is on the forum)*"
            text = text[:room - len(tail)].rstrip("\\") + tail
        return head + "\n" + text

    def _send(self, channel: str, content: str, mention: str = "") -> dict:
        am = {"parse": [], "users": [mention]} if mention else {"parse": []}
        return self.api.call("POST", f"/channels/{channel}/messages", {"content": content[:MSG_MAX], "allowed_mentions": am,
                                                                      "flags": SUPPRESS_EMBEDS}) or {}

    def opener_of(self, tid: int) -> dict | None:
        r = self.store.one("SELECT id FROM forum_posts WHERE thread = ? ORDER BY opener DESC, created, id LIMIT 1", (int(tid),))
        return self.store.post(r[0]) if r else None

    def create_thread(self, th: dict) -> dict | None:
        """The forum thread's Discord thread with its first post; the mapping kept. None when not mapped."""
        cc = self.cat_channel(th["cat"])
        if not cc:
            return None
        ch, tag = cc
        op = self.opener_of(th["id"])
        if not op:
            return None
        content = self._content(op, th, opener=True)
        name = th["title"][:TITLE_MAX]
        if self.ch_type.get(ch, CHANNEL_FORUM) == CHANNEL_FORUM:
            body = {"name": name, "auto_archive_duration": 10080,
                    "message": {"content": content, "allowed_mentions": {"parse": []}, "flags": SUPPRESS_EMBEDS}}
            if tag:
                body["applied_tags"] = [tag]
            t = self.api.call("POST", f"/channels/{ch}/threads", body) or {}
            dthread = str(t.get("id") or "")
            starter = str((t.get("message") or {}).get("id") or dthread)
        else:
            t = self.api.call("POST", f"/channels/{ch}/threads", {"name": name, "type": 11, "auto_archive_duration": 10080}) or {}
            dthread = str(t.get("id") or "")
            starter = str(self._send(dthread, content).get("id") or "")
        if not dthread:
            raise DiscordError("down", detail="no thread id in Discord's answer")
        self.store.x("INSERT OR REPLACE INTO fd_threads (dthread, thread, channel, state, last_seen, active) VALUES (?,?,?,?,?,?)",
                     (dthread, th["id"], ch, "live", starter or dthread, self.now()))
        self.put_post(op["id"], dthread, starter, "site", "masked" if op["hidden"] else "live")
        if th["locked"]:
            self.api.call("PATCH", f"/channels/{dthread}", {"locked": True})
        return self.live_for(th["id"])

    def deliver(self, kind: str, tid: int, d: dict) -> None:
        if kind == "thread.new":
            th = self.store.thread(tid)
            if th and not th["hidden"] and not th["merged_into"] and not self.live_for(tid):
                self.create_thread(th)
        elif kind == "post.new":
            p = self.store.post(d["post"])
            if not p or p["hidden"] or self.post_row(p["id"]):
                return
            th = self.store.thread(p["thread"])
            if not th or th["hidden"]:
                return
            row = self.live_for(th["id"]) or self.create_thread(th)
            if not row or self.post_row(p["id"]):   # the opener, just made with the thread
                return
            try:
                m = self._send(row["dthread"], self._content(p, th))
            except DiscordError as e:
                if e.kind == "notfound":    # the Discord thread is gone: the sweep hides it on the forum
                    self.set_thread_row(row["dthread"], state="gone")
                    return
                raise
            self.put_post(p["id"], row["dthread"], str(m.get("id") or ""), "site")
        elif kind in ("post.hide", "post.show"):
            self._post_visibility(d["post"], kind == "post.hide")
        elif kind == "thread.delete":
            row = self.live_for(tid)
            if row:
                try:
                    self.api.call("DELETE", f"/channels/{row['dthread']}")
                except DiscordError as e:
                    if e.kind != "notfound":
                        raise
                self.set_thread_row(row["dthread"], state="deleted")
                self.store.x("UPDATE fd_posts SET state = 'gone' WHERE dthread = ?", (row["dthread"],))
        elif kind == "thread.lock":
            row = self.live_for(tid)
            if row:
                self._patch_thread(row, {"locked": bool(d.get("on")), "archived": False})
        elif kind == "thread.merge":
            row = self.live_for(tid)
            dst = self.store.resolve(d.get("into"))
            if row:
                if dst:
                    try:
                        self._send(row["dthread"], f"This thread was merged into **{esc(dst['title'])}** · [open](<{self.post_url(dst['id'])}>)")
                    except DiscordError as e:
                        if e.kind != "notfound":
                            raise
                self._patch_thread(row, {"locked": True, "archived": True})
                self.set_thread_row(row["dthread"], state="merged")
        elif kind == "dm":
            self._dm(d["to"], d["text"], d.get("where", ""))
        elif kind == "delete_msg":
            try:
                self.api.call("DELETE", f"/channels/{d['channel']}/messages/{d['msg']}")
            except DiscordError as e:
                if e.kind != "notfound":
                    raise
        elif kind == "delete_thread":
            try:
                self.api.call("DELETE", f"/channels/{d['dthread']}")
            except DiscordError as e:
                if e.kind != "notfound":
                    raise
        elif kind.startswith("dg."):   # stream/discord_growth.py's own items
            if self.growth is not None:
                self.growth.deliver(kind, d)
        elif kind == "replace":   # a Discord person's message, re-posted as the forum shows it (the word filter starred words)
            pr = self.post_row(d["post"])
            if pr and pr["origin"] == "discord" and pr["state"] == "live":
                try:
                    self.api.call("DELETE", f"/channels/{pr['dthread']}/messages/{pr['dmsg']}")
                except DiscordError as e:
                    if e.kind != "notfound":
                        raise
                self.store.x("UPDATE fd_posts SET state = 'gone' WHERE post = ?", (pr["post"],))
                self._post_visibility(pr["post"], False)
        else:
            log.warning("forum-discord: unknown outbox item %s", kind)

    def _patch_thread(self, row: dict, body: dict) -> None:
        try:
            self.api.call("PATCH", f"/channels/{row['dthread']}", body)
        except DiscordError as e:
            if e.kind != "notfound":
                raise

    def _post_visibility(self, pid: int, hide: bool) -> None:
        p = self.store.post(pid)
        if not p:
            return
        th = self.store.thread(p["thread"])
        pr = self.post_row(pid)
        if hide:
            if not pr or pr["state"] != "live" or not pr["dmsg"]:
                return
            try:
                if pr["origin"] == "discord":
                    self.api.call("DELETE", f"/channels/{pr['dthread']}/messages/{pr['dmsg']}")
                    state = "gone"
                else:
                    self.api.call("PATCH", f"/channels/{pr['dthread']}/messages/{pr['dmsg']}",
                                  {"content": self._content(dict(p, hidden=True), th, opener=p["opener"]), "allowed_mentions": {"parse": []}})
                    state = "masked"
            except DiscordError as e:
                if e.kind != "notfound":
                    raise
                state = "gone"
            self.store.x("UPDATE fd_posts SET state = ? WHERE post = ?", (state, pid))
            return
        if p["hidden"] or not th or th["hidden"]:
            return
        if pr and pr["state"] == "masked":
            try:
                self.api.call("PATCH", f"/channels/{pr['dthread']}/messages/{pr['dmsg']}",
                              {"content": self._content(p, th, opener=p["opener"]), "allowed_mentions": {"parse": []}})
                self.store.x("UPDATE fd_posts SET state = 'live' WHERE post = ?", (pid,))
                return
            except DiscordError as e:
                if e.kind != "notfound":
                    raise
        if pr and pr["state"] == "live":
            return
        row = self.live_for(th["id"])
        if not row:
            return
        m = self._send(row["dthread"], self._content(p, th))
        self.put_post(pid, row["dthread"], str(m.get("id") or ""), "mirror" if (pr and pr["origin"] != "site") or p.get("via") == "discord" else "site")

    def _dm(self, to: str, text: str, fallback_thread: str = "") -> None:
        ch = self.dm_channels.get(to)
        try:
            if not ch:
                ch = str((self.api.call("POST", "/users/@me/channels", {"recipient_id": to}) or {}).get("id") or "")
                if ch:
                    self.dm_channels[to] = ch
            if not ch:
                raise DiscordError("forbidden", 403, 50007)
            self._send(ch, text)
        except DiscordError as e:
            if e.kind not in ("forbidden", "bad"):
                raise
            if fallback_thread:   # DMs closed: a short note in the thread that pings only them, gone after 2 minutes
                m = self._send(fallback_thread, f"<@{to}> {text}", mention=to)
                if m.get("id"):
                    self.enqueue("delete_msg", 0, delay=FALLBACK_TTL, channel=fallback_thread, msg=str(m["id"]))

    def tell(self, discord_id: str, text: str, dthread: str = "") -> None:
        """One private note to a Discord person (at most one a minute each)."""
        t = self.clock()
        if t - self.dm_last.get(discord_id, 0) < DM_GAP:
            return
        self.dm_last[discord_id] = t
        if len(self.dm_last) > 5000:
            self.dm_last.clear()
        self.enqueue("dm", 0, to=str(discord_id), text=text[:1500], where=dthread)

    # -- Discord -> the forum --------------------------------------------------------------------------------------
    def start(self) -> None:
        me = self.api.call("GET", "/users/@me") or {}
        self.bot_id = str(me.get("id") or "")
        for cat, (ch, _tag) in self.cfg.channels.items():
            c = self.api.call("GET", f"/channels/{ch}") or {}
            self.ch_type[ch] = int(c.get("type") or 0)
            if self.ch_type[ch] not in (CHANNEL_FORUM, CHANNEL_TEXT, CHANNEL_NEWS):
                log.warning("forum-discord: channel for %s is neither a Forum nor a text channel", cat)
            g = str(c.get("guild_id") or "")
            if g and self.guild and g != self.guild:
                log.warning("forum-discord: the channels are in more than one server; only the first one is read")
            self.guild = self.guild or g
        if self.cfg.guild_id and self.guild and self.cfg.guild_id != self.guild:
            log.warning("forum-discord: WU_DISCORD_GUILD is not the server the forum channels are in; using the channels' server")
        self.guild = self.guild or self.cfg.guild_id
        if self.cfg.app_id and self.guild:
            try:
                self.api.call("PUT", f"/applications/{self.cfg.app_id}/guilds/{self.guild}/commands", COMMANDS)
            except DiscordError as e:
                if e.kind == "auth":
                    raise
                log.warning("forum-discord: /link could not be set up (%s): invite the bot with the applications.commands scope", e.kind)
        self.started = True
        self.state = "running"
        log.info("forum-discord: running for %d categories", len(self.cfg.channels))

    def poll(self) -> None:
        if not self.guild:
            return
        doc = self.api.call("GET", f"/guilds/{self.guild}/threads/active") or {}
        parents = {ch: cat for cat, (ch, _t) in self.cfg.channels.items()}
        for t in doc.get("threads") or []:
            parent, dthread = str(t.get("parent_id") or ""), str(t.get("id") or "")
            if parent not in parents or not dthread:
                continue
            row = self.thread_row(dthread)
            if row is None:
                self.maybe_import(t, parents[parent])
                row = self.thread_row(dthread)
                if not row or row["state"] != "live":
                    continue
            if row["state"] not in ("live", "merged"):
                continue
            lm = str(t.get("last_message_id") or "0")
            if lm.isdigit() and int(lm) > int(row["last_seen"] or 0):
                self.read_thread(row)
            mc = t.get("message_count")
            if isinstance(mc, int):
                dropped = 0 <= mc < row["msg_count"]
                self.set_thread_row(dthread, msg_count=mc, active=self.now())
                if dropped:
                    self.sweep_thread(self.thread_row(dthread))

    def read_thread(self, row: dict) -> None:
        after = row["last_seen"] or "0"
        for _ in range(5):
            msgs = self.api.call("GET", f"/channels/{row['dthread']}/messages?limit=100&after={after}") or []
            msgs = sorted(msgs, key=lambda m: int(m["id"]))
            for m in msgs:
                after = str(m["id"])
                self.set_thread_row(row["dthread"], last_seen=after)
                self.handle(row, m)
            if len(msgs) < 100:
                break

    def _person(self, m: dict) -> tuple[str, str]:
        a = m.get("author") or {}
        n = ((m.get("member") or {}).get("nick")) or a.get("global_name") or a.get("username") or ""
        n = self.fm.clean_line(n, 32)
        if not n or self.fm.BRAND_RE.search(n) or self.fm._hook("bad_word", n):
            n = ""
        return str(a.get("id") or ""), n

    def _ours(self, m: dict) -> bool:
        a = m.get("author") or {}
        return bool(a.get("bot") or a.get("system") or m.get("webhook_id") or str(a.get("id")) == self.bot_id
                    or m.get("type", 0) not in (0, 19))

    def handle(self, row: dict, m: dict) -> None:
        """One new Discord message in a mapped thread: a linked person's reply becomes a forum post, else it's refused."""
        if self._ours(m) or self.store.one("SELECT 1 FROM fd_posts WHERE dmsg = ?", (str(m["id"]),)):
            return
        th = self.store.resolve(row["thread"])
        if not th or th["hidden"]:
            return
        did, dname = self._person(m)
        acct = self.linked(did)
        if not acct:
            return self.unlinked(did, row["dthread"], m)
        body = from_discord(m)
        if not body.strip():
            return
        blk = self.blocked(acct)
        if blk:
            return self.refuse(did, row["dthread"], m, blk)
        self.seen(acct)
        self.local.from_discord = True
        try:
            pid = self.store.reply(acct, th["id"], body, False, via="discord", via_name=dname)
        except self.fm.ForumError as ex:
            return self.refuse(did, row["dthread"], m, ex.text)
        finally:
            self.local.from_discord = False
        self.put_post(pid, row["dthread"], str(m["id"]), "discord")
        if self.store.post(pid)["body"] != self.fm.clean_body(body):
            self.enqueue("replace", th["id"], post=pid)

    def blocked(self, acct: str) -> str:
        adm = self.fm.P("admin")
        try:
            return adm.blocked("post", acct=acct) if adm is not None and hasattr(adm, "blocked") else ""
        except Exception:   # noqa: BLE001
            return ""

    def seen(self, acct: str) -> None:
        name, tag = self.fm._names(acct, "")
        self.store.seen(acct, name, tag, None)

    def notice(self, did: str, dthread: str = "", extra: str = "") -> None:
        """The "link your account" note: once a week at most per Discord person."""
        r = self.store.one("SELECT at FROM fd_notices WHERE discord = ?", (did,))
        if r and self.now() - r[0] < NOTICE_EVERY:
            return
        self.store.x("INSERT OR REPLACE INTO fd_notices (discord, at) VALUES (?, ?)", (did, self.now()))
        slash = " (or type /link in the Discord)" if self.cfg.app_id else ""
        self.dm_last.pop(did, None)
        self.tell(did, f"Link your Leyline account to post here: {self.link_url()}{slash}. These Discord threads are a window "
                       "onto the Leyline forum, so only linked accounts can post in them." + extra, dthread)

    def unlinked(self, did: str, dthread: str, m: dict) -> None:
        if not did:
            return
        self.notice(did, dthread)
        if self.cfg.remove:
            self.enqueue("delete_msg", 0, channel=dthread, msg=str(m["id"]))

    def refuse(self, did: str, dthread: str, m: dict, why: str, whole_thread: bool = False) -> None:
        self.tell(did, f"Your post didn't reach the Leyline forum: {why}", dthread if not whole_thread else "")
        if self.cfg.remove:
            if whole_thread:
                self.enqueue("delete_thread", 0, dthread=dthread)
            else:
                self.enqueue("delete_msg", 0, channel=dthread, msg=str(m["id"]))

    def maybe_import(self, t: dict, cat: str) -> None:
        """A thread someone started in a mapped Discord Forum channel: a forum thread when its starter is linked."""
        dthread, parent = str(t["id"]), str(t.get("parent_id"))
        self.store.x("INSERT OR IGNORE INTO fd_threads (dthread, thread, channel, state, last_seen) VALUES (?,?,?,?,?)",
                     (dthread, 0, parent, "ignored", dthread))
        if (self.ch_type.get(parent) != CHANNEL_FORUM or str(t.get("owner_id")) == self.bot_id
                or snowflake_ms(dthread) < int(self.meta("since") or 0)):
            return
        try:
            m = self.api.call("GET", f"/channels/{dthread}/messages/{dthread}") or {}
        except DiscordError as e:
            if e.kind == "notfound":
                return
            self.store.x("DELETE FROM fd_threads WHERE dthread = ?", (dthread,))   # try again next round
            raise
        if self._ours(m):
            return
        did, dname = self._person(m)
        acct = self.linked(did)
        if not acct:
            self.set_thread_row(dthread, state="refused" if self.cfg.remove else "ignored")
            self.notice(did, "" if self.cfg.remove else dthread, " Your thread was taken down: the Discord's forum channels "
                        "are a window onto the Leyline forum." if self.cfg.remove else "")
            if self.cfg.remove:
                self.enqueue("delete_thread", 0, dthread=dthread)
            return
        blk = self.blocked(acct)
        if blk:
            self.set_thread_row(dthread, state="refused" if self.cfg.remove else "ignored")
            return self.refuse(did, dthread, m, blk, whole_thread=True)
        self.seen(acct)
        staff_only = bool(self.fm.CAT.get(cat, (0, 0, 0, False))[3])
        staff = staff_only and bool((self.store.one("SELECT staff FROM forum_users WHERE acct = ?", (acct,)) or [0])[0])
        self.local.from_discord = True
        try:
            tid = self.store.new_thread(acct, cat, t.get("name") or "", from_discord(m), staff, via="discord", via_name=dname)
        except self.fm.ForumError as ex:
            self.set_thread_row(dthread, state="refused" if self.cfg.remove else "ignored")
            return self.refuse(did, dthread, m, ex.text, whole_thread=True)
        finally:
            self.local.from_discord = False
        op = self.opener_of(tid)
        self.set_thread_row(dthread, thread=tid, state="live", active=self.now(), msg_count=int(t.get("message_count") or 0))
        self.put_post(op["id"], dthread, dthread, "discord")
        log.info("forum-discord: Discord thread became forum thread %d", tid)

    # -- deletions in Discord -> hidden on the forum ---------------------------------------------------------------
    def sweep_some(self) -> None:
        t = self.now()
        for (d,) in self.store.q("SELECT dthread FROM fd_threads WHERE state = 'live' AND active > ? ORDER BY swept LIMIT ?",
                                 (t - SWEEP_DAYS * 86400, SWEEP_PER_ROUND)):
            self.sweep_thread(self.thread_row(d))

    def sweep_thread(self, row: dict | None) -> None:
        if not row or row["state"] != "live":
            return
        ids, before, oldest, complete = set(), "", None, False
        try:
            for _ in range(5):
                msgs = self.api.call("GET", f"/channels/{row['dthread']}/messages?limit=100" + (f"&before={before}" if before else "")) or []
                ids |= {str(m["id"]) for m in msgs}
                if msgs:
                    before = str(min(int(m["id"]) for m in msgs))
                    oldest = int(before)
                if len(msgs) < 100:
                    complete = True
                    break
        except DiscordError as e:
            if e.kind != "notfound":
                raise
            return self.thread_gone(row)
        self.set_thread_row(row["dthread"], swept=self.now())
        for pid, dmsg in self.store.q("SELECT post, dmsg FROM fd_posts WHERE dthread = ? AND state IN ('live', 'masked') AND dmsg != ''",
                                      (row["dthread"],)):
            if dmsg in ids or (not complete and oldest is not None and int(dmsg) < oldest):
                continue
            self.store.x("UPDATE fd_posts SET state = 'gone' WHERE post = ?", (pid,))
            if self.cfg.delete_hides:
                p = self.store.post(pid)
                if p and not p["hidden"]:
                    self._as_discord(lambda: self.store.set_post_hidden(pid, True))
                    self.audit("forum.post.hide", pid, "deleted in Discord")

    def thread_gone(self, row: dict) -> None:
        self.set_thread_row(row["dthread"], state="gone")
        self.store.x("UPDATE fd_posts SET state = 'gone' WHERE dthread = ?", (row["dthread"],))
        th = self.store.thread(row["thread"])
        if self.cfg.delete_hides and th and not th["hidden"]:
            self._as_discord(lambda: self.store.set_thread(th["id"], "hidden", True))
            self.audit("forum.thread.hide", th["id"], "the thread was deleted in Discord")

    def _as_discord(self, fn) -> None:
        self.local.from_discord = True
        try:
            fn()
        finally:
            self.local.from_discord = False

    def audit(self, action: str, target, note: str) -> None:
        adm = self.fm.P("admin")
        try:
            if adm is not None and hasattr(adm, "audit"):
                adm.audit({"account": "discord", "name": "Discord moderator", "role": "mod", "via": "discord"}, action,
                          str(target), {}, {"hidden": True}, True, note)
        except Exception:   # noqa: BLE001
            log.exception("forum-discord: the audit log write failed")

    # -- the worker ------------------------------------------------------------------------------------------------
    def round(self) -> None:
        """One turn of the worker: start, send what is due, read Discord, sweep."""
        mono = time.monotonic()
        if not self.started:
            self.start()
        self.drain()
        if mono >= self.next_poll:
            self.next_poll = mono + self.cfg.poll
            self.poll()
            self.drain()
        if self.growth is not None:
            self.growth.tick()
        if mono >= self.next_sweep:
            self.next_sweep = mono + SWEEP_EVERY
            self.sweep_some()
            self.store.x("DELETE FROM fd_codes WHERE expires < ?", (self.now(),))

    def run(self) -> None:
        wait = 1.0
        while not self.stop.is_set():
            try:
                self.round()
                wait = 1.0
                if self.state != "running":
                    self.state = "running"
            except DiscordError as e:
                self.last_error = f"{e.kind} ({e.detail})"
                if e.kind == "auth":
                    self.state = "paused: Discord refused the bot token"
                    log.error("forum-discord: Discord refused the bot token; trying again in 10 minutes")
                    self.started = False
                    wait = 600.0
                else:
                    self.state = f"waiting for Discord ({e.kind})"
                    log.warning("forum-discord: %s", e)
                    wait = min(max(wait * 2, e.retry_after or 0), 300.0)
            except Exception:   # noqa: BLE001: the worker never dies
                log.exception("forum-discord: a round failed")
                wait = min(wait * 2, 300.0)
            self.stop.wait(wait)

    def status(self) -> dict:
        n, oldest = self.store.one("SELECT COUNT(*), MIN(created) FROM fd_outbox")
        links = self.store.one("SELECT COUNT(*) FROM fd_links")[0]
        return {"state": self.state, "outbox": n, "oldest": oldest or 0, "links": links, "last_error": self.last_error,
                "categories": sorted(self.cfg.channels)}

    # -- /link and /unlink (Discord's Interactions Endpoint URL) ---------------------------------------------------
    def verify(self, headers, body: bytes) -> bool:
        sig, ts = headers.get("X-Signature-Ed25519", ""), headers.get("X-Signature-Timestamp", "")
        if self.ed is None or not re.fullmatch(r"[0-9a-fA-F]{128}", sig) or not ts.isdigit():
            return False
        if abs(time.time() - int(ts)) > 300:
            return False
        try:
            return bool(self.ed.verify(bytes.fromhex(self.cfg.public_key), ts.encode() + body, bytes.fromhex(sig)))
        except ValueError:
            return False

    def interaction(self, headers, body: bytes) -> tuple[int, dict]:
        if not self.cfg.public_key:
            return 404, {"error": "not set up"}
        if not self.verify(headers, body):
            return 401, {"error": "invalid request signature"}
        try:
            d = json.loads(body.decode("utf-8"))
        except (UnicodeDecodeError, ValueError):
            return 400, {"error": "bad request"}
        if d.get("type") == 1:
            return 200, {"type": 1}
        if d.get("type") != 2:
            return 400, {"error": "unsupported"}
        user = ((d.get("member") or {}).get("user")) or d.get("user") or {}
        did = str(user.get("id") or "")
        dname = self.fm.clean_line(((d.get("member") or {}).get("nick")) or user.get("global_name") or user.get("username") or "", 32)
        name = str((d.get("data") or {}).get("name") or "")
        say = lambda text: (200, {"type": 4, "data": {"content": text, "flags": EPHEMERAL, "allowed_mentions": {"parse": []}}})  # noqa: E731
        if not SNOW_RE.fullmatch(did):
            return say("Discord didn't say who you are. Try again.")
        if name == "link":
            acct = self.linked(did)
            if acct:
                pr = self.store.one("SELECT name FROM forum_users WHERE acct = ?", (acct,))
                return say(f"You're linked to the Leyline account {esc(pr[0] if pr else 'Player')}. Your replies in the forum threads "
                           "here reach the forum. /unlink undoes it.")
            code = self.new_code(did, dname)
            return say(f"Open this link and sign in on the Leyline website to link your account (good for 15 minutes, once): "
                       f"{self.link_url(code)}")
        if name == "unlink":
            return say("Unlinked. Your messages here no longer reach the forum." if self.unlink(discord_id=did)
                       else "You weren't linked.")
        return say("I don't know that command.")


# --------------------------------------------------------------------------------------------- the portal's side
link: Link | None = None


def attach(fm, store, env=None, start_thread: bool = True, urlopen=urllib.request.urlopen) -> Link | None:
    """forum.setup() calls this: the link when WU_FORUM_DISCORD=1 and the settings are complete, else None (logged)."""
    global link
    cfg = Config(env, tuple(fm.CAT))
    if not cfg.on:
        return None
    bad = cfg.problems()
    if bad:
        for p in bad:
            log.error("forum-discord: off: %s", p)
        return None
    api = Api(cfg.token, cfg.api, urlopen=urlopen, site="https://" + fm._public_host())
    link = Link(fm, store, cfg, api)
    link.growth = _growth(link, env)
    log.info("forum-discord: %r", cfg)
    if start_thread:
        threading.Thread(target=link.run, name="forum-discord", daemon=True).start()
    return link


def _growth(link, env):
    """stream/discord_growth.py beside this file, attached to the link (the same bot, client and outbox), or None."""
    p = os.path.join(HERE, "discord_growth.py")
    if not os.path.isfile(p):
        return None
    try:
        spec = importlib.util.spec_from_file_location("wu_discord_growth", p)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        import types
        return mod.attach(link, env, types.SimpleNamespace(esc=esc, DiscordError=DiscordError, group_http=mod.group_http))
    except Exception:   # noqa: BLE001: the forum link runs without it
        log.exception("forum-discord: discord_growth.py didn't load")
        return None


def group_http(fm, h) -> None:
    """POST /wrath/forum/api/discord/group: Find a group from the launcher (stream/discord_growth.py)."""
    g = getattr(link, "growth", None) if link is not None else None
    if g is None:
        fm._read_json(h)
        return fm._jpost(h, 404, {"error": "Find a group isn't on yet."})
    return g.fd.group_http(fm, h, link)


def _discord_of_site(fm, h) -> str:
    """The Discord id the website's own Discord sign-in recorded, when its /api/me says (best effort: "discord_id", or
    {"discord": {"id"}}); "" otherwise."""
    try:
        status, me = fm.P("portal_get")("/api/me", h.headers.get("Cookie", ""), h.ip(), h.headers.get("User-Agent", ""))
    except Exception:   # noqa: BLE001
        return ""
    if status != 200 or not isinstance(me, dict):
        return ""
    v = me.get("discord_id") or ((me.get("discord") or {}).get("id") if isinstance(me.get("discord"), dict) else "")
    return str(v) if SNOW_RE.fullmatch(str(v or "")) else ""


def page(fm, h, who, query: dict, note: str = "", ok: str = "", status: int = 200) -> None:
    """GET /wrath/leyline/forum/discord[?c=<code>]: link this Discord account, or see and undo the link."""
    e = html.escape
    path = fm.PAGE + "/discord"
    code = (query.get("c") or [""])[0]
    if not who:
        return h.redirect(fm._signin_url(path + (f"?c={urllib.parse.quote(code)}" if code else "")))
    msg = (f'<p class="fm-msg err" role="alert">{e(note)}</p>' if note else "") + (f'<p class="fm-msg" role="status">{e(ok)}</p>' if ok else "")
    cur = link.link_of(who["acct"])
    parts = []
    info = link.code_info(code) if code else None
    if code and not info:
        parts.append('<p class="fm-msg err" role="alert">That link is old or was used already. Type /link in the Discord again.</p>')
    if info:
        parts.append(f'<form class="fm-form" method="post" action="{path}"><input type="hidden" name="t" value="{fm.form_token(who, "dlink")}">'
                     f'<input type="hidden" name="c" value="{e(code)}"><p>Link the Discord account <b>{e(info["dname"] or "you")}</b> '
                     f'to your Leyline account <b>{e(who["name"])}</b>? Your replies in the Discord\'s forum threads will then show on '
                     'the forum as you, "via Discord", under the forum\'s rules.</p><p class="fm-fine">Only link a Discord account that is '
                     'yours: whoever uses it can post on the forum as you. Didn\'t type /link yourself? Close this page.</p>'
                     '<div class="fm-acts"><button class="fm-btn" type="submit" name="do" value="link">Link</button>'
                     f'<a class="fm-btn ghost" href="{fm.PAGE}">Not now</a></div></form>')
    elif not cur:
        site_id = _discord_of_site(fm, h)
        if site_id:
            parts.append(f'<form class="fm-form" method="post" action="{path}"><input type="hidden" name="t" value="{fm.form_token(who, "dlink")}">'
                         '<p>You signed in to the website with Discord. Link that Discord account, so your replies there reach the forum?</p>'
                         '<div class="fm-acts"><button class="fm-btn" type="submit" name="do" value="site">Link my Discord</button></div></form>')
        parts.append('<p>In our Discord, type <b>/link</b> in any channel and open the link it gives you (only you see it).'
                     + ('' if site_id else ' Or link Discord as a way to sign in on <a href="/wrath/profile">your profile</a>, then come back here.')
                     + '</p>')
    if cur and link.growth is not None:   # Find a group (WU_DISCORD_LFG=1)
        parts.append(link.growth.group_form(who, query, fm.form_token(who, "dlink"), path))
    if cur:
        parts.append(f'<form class="fm-form" method="post" action="{path}"><input type="hidden" name="t" value="{fm.form_token(who, "dlink")}">'
                     f'<p>Linked to the Discord account <b>{e(cur["dname"] or "(no name)")}</b>. Your replies in the Discord\'s forum '
                     'threads show on the forum as you.</p><div class="fm-acts"><button class="fm-btn ghost" type="submit" name="do" '
                     'value="unlink">Unlink</button></div></form>')
    body = (f'<section class="fm-head"><div><p class="fm-crumb"><a href="{fm.PAGE}">Forum</a></p><h1>The forum on Discord</h1>'
            '<p>The forum\'s threads also show in our Discord. Link your account to reply from there.</p></div></section>'
            f'{msg}<section class="fm-box">{"".join(parts)}</section>')
    fm._shell(h, "Discord · Forum · Leyline", body, path, who, status)


def post(fm, h, who, form: dict) -> None:
    """POST /wrath/leyline/forum/discord {t, do: link|site|unlink, c?}."""
    if not hmac.compare_digest(form.get("t", ""), fm.form_token(who, "dlink")):
        return page(fm, h, who, {"c": [form.get("c", "")]}, "That form timed out. Try again.", status=403)
    do = form.get("do", "")
    if do == "unlink":
        link.unlink(acct=who["acct"])
        return page(fm, h, who, {}, ok="Unlinked. Your Discord messages no longer reach the forum.")
    if do == "link":
        info = link.code_info(form.get("c", ""))
        if not info:
            return page(fm, h, who, {}, "That link is old or was used already. Type /link in the Discord again.", status=400)
        link.do_link(who["acct"], info["discord"], info["dname"], form.get("c", ""))
        return page(fm, h, who, {}, ok="Linked. Your replies in the Discord's forum threads now reach the forum as you.")
    if do == "group" and link.growth is not None:
        try:
            text = link.growth.group(who["acct"], form.get("realm", ""), form.get("what", ""), form.get("note", ""))
        except fm.ForumError as ex:
            return page(fm, h, who, {}, ex.text, status=ex.status)
        return page(fm, h, who, {}, ok=text)
    if do == "site":
        did = _discord_of_site(fm, h)
        if not did:
            return page(fm, h, who, {}, "The website doesn't know a Discord account for you. Type /link in the Discord instead.", status=400)
        link.do_link(who["acct"], did, "")
        return page(fm, h, who, {}, ok="Linked. Your replies in the Discord's forum threads now reach the forum as you.")
    return page(fm, h, who, {}, "Unknown request.", status=400)


def interactions_http(fm, h) -> None:
    """POST /wrath/forum/api/discord/interactions: Discord's Interactions Endpoint URL (/link, /unlink)."""
    try:
        n = int(h.headers.get("Content-Length", "0"))
    except ValueError:
        n = -1
    body = h.rfile.read(n) if 0 <= n <= 65536 else b""
    if link is None:
        st, doc = 404, {"error": "not set up"}
    elif not (0 <= n <= 65536):
        st, doc = 413, {"error": "too big"}
    else:
        st, doc = link.interaction(h.headers, body)
    h.send(st, json.dumps(doc, separators=(",", ":")).encode(), "application/json", headers={"Cache-Control": "no-store"})
