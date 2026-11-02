"""Leyline's Discord, grown: the bot and setup side of the growth plan (owner, 2026-10-31: "yep do it").

stream/forum_discord.py loads this beside itself. It is the SAME bot: the same token, the same REST client (its rate
limits), the same outbox in forum.sqlite3 and the same worker thread. Every part is OFF until the owner turns it on in
portal.env (docs/DISCORD.md has the checklist); nothing here ever posts on its own before that.

  setup        HQ > Content > Discord > "Set up the Discord" (owner, typed confirm "setup"): the bot makes the
               channels and roles in discord_template.json that the server doesn't have yet. Found by name (and type),
               so it is safe to run again: it never deletes, renames or changes anything that is already there.
               "Preview" first lists what it would make. Needs WU_DISCORD_GUILD (or a forum channel to learn it from).
  live realms  WU_DISCORD_FEED=1: a realm on the network that goes live (first seen, or back after 6 hours away) gets
               a card in live-realms: its name, mode, players, and its link (/r/<short> when the realm has one, else its
               join link). At most WU_DISCORD_FEED_PER_HOUR (4) an hour, busiest first; a realm still waiting after an
               hour is skipped. Realms already live when the feed is first turned on are not announced.
  picks        Den of the week and the featured mode, picked by staff in HQ: a post in den-showcase / modes-and-forks
               (with WU_DISCORD_FEED=1); Den of the week also gives its maker the Den Architect fact.
  find a group WU_DISCORD_LFG=1: the forum's Discord page (website) and POST /wrath/forum/api/discord/group (launcher,
               Bearer token) post "<name> wants a group for <realm or mode>" in looking-for-group with a Join button.
               Linked accounts only, no sanctions, the word and brand filters, one per account every WU_DISCORD_LFG_GAP
               minutes (30), WU_DISCORD_LFG_PER_HOUR (20) in all.
  roles        WU_DISCORD_ROLES=1: once a night (WU_DISCORD_ROLES_HOUR, 4 UTC) linked Discord people get the roles
               their Leyline facts earn: Realm Host (owns a realm listed for 7 days: hostflow.py's records and the
               relay's first-seen), Mode Maker, Builder, Node Keeper (facts recorded in HQ or by record_fact()), Den
               Architect (Den of the week; kept for good). A role the bot gave is taken back when the fact is gone; a
               role staff gave by hand is never touched; Guide, Moderator and Core Team stay manual. Every change is
               in HQ's audit log ("discord.role.add" / "discord.role.remove").
  automod      WU_DISCORD_AUTOMOD=1: our blocked words (stream/namecheck.py: slurs, sexual terms, profanity, the owner's
               words) as one Discord AutoMod keyword rule, "Leyline blocked words", made or updated (never deleted;
               turning it off in Discord is respected). Pushed when the list changes and once a day.

Tables (forum.sqlite3): dg_map (what setup found or made), dg_realms (the feed's memory), dg_posts (every post, its
state), dg_facts, dg_given (roles the bot gave), dg_picks. Standard library only.
"""
from __future__ import annotations

import calendar
import hashlib
import html
import json
import logging
import os
import re
import sys
import time
import urllib.parse

log = logging.getLogger("wrath-portal.discord")
HERE = os.path.dirname(os.path.abspath(__file__))
TEMPLATE_FILE = os.path.join(HERE, "discord_template.json")

# channel types and permission bits (Discord API v10)
T_TEXT, T_VOICE, T_CATEGORY, T_STAGE, T_FORUM = 0, 2, 4, 13, 15
TYPES = {"text": T_TEXT, "voice": T_VOICE, "stage": T_STAGE, "forum": T_FORUM}
P_ADD_REACTIONS, P_VIEW, P_SEND, P_READ_HISTORY = 1 << 6, 1 << 10, 1 << 11, 1 << 16
P_CONNECT, P_SPEAK, P_CREATE_THREADS, P_SEND_THREADS = 1 << 20, 1 << 21, 1 << 35, 1 << 38
P_MANAGE_CHANNELS, P_MANAGE_GUILD, P_MANAGE_ROLES = 1 << 4, 1 << 5, 1 << 28
# the bot's permissions: the forum link's (docs/FORUM.md, 326417591296) plus Manage Channels and Manage Roles (setup,
# roles), plus Manage Server for AutoMod only
FORUM_PERMS = 326417591296
BOT_PERMS = FORUM_PERMS | P_MANAGE_CHANNELS | P_MANAGE_ROLES
BOT_PERMS_AUTOMOD = BOT_PERMS | P_MANAGE_GUILD
SUPPRESS_EMBEDS = 4
BACK_AFTER = 6 * 3600          # a realm away this long is announced again when it comes back
PENDING_FOR = 3600             # a card the hourly cap held back is dropped after this
FEED_EVERY = 60
HOST_DAYS = 7
RULE_NAME = "Leyline blocked words"
ACCT_RE = re.compile(r"^[A-Z0-9]{1,16}$")
KEY_RE = re.compile(r"^[a-z][a-z0-9_]{0,31}$")
SNOW_RE = re.compile(r"^\d{5,22}$")
SHORT_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,30}[a-z0-9])?$")
HOST_RE = re.compile(r"^(?=.{1,253}$)[a-z0-9-]{1,63}(\.[a-z0-9-]{1,63})+$")
FACTS = {"realm_host": "hosts a listed realm for 7 days (automatic)", "mode_maker": "published a mode others host",
         "builder": "approved as a builder", "den_architect": "Den of the week", "node_keeper": "carries network traffic (a node)"}
MANUAL_FACTS = ("mode_maker", "builder", "node_keeper", "den_architect")

SCHEMA = """
CREATE TABLE IF NOT EXISTS dg_map (kind TEXT NOT NULL, key TEXT NOT NULL, id TEXT NOT NULL, name TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (kind, key));
CREATE TABLE IF NOT EXISTS dg_realms (id TEXT PRIMARY KEY, first_live INTEGER NOT NULL, last_live INTEGER NOT NULL,
  pending INTEGER NOT NULL DEFAULT 0, posted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS dg_posts (id INTEGER PRIMARY KEY AUTOINCREMENT, at INTEGER NOT NULL, kind TEXT NOT NULL,
  ref TEXT NOT NULL DEFAULT '', channel TEXT NOT NULL, body TEXT NOT NULL, status TEXT NOT NULL, msg TEXT NOT NULL DEFAULT '',
  error TEXT NOT NULL DEFAULT '');
CREATE INDEX IF NOT EXISTS dg_posts_kind ON dg_posts (kind, at);
CREATE TABLE IF NOT EXISTS dg_facts (acct TEXT NOT NULL, fact TEXT NOT NULL, source TEXT NOT NULL, at INTEGER NOT NULL,
  by TEXT NOT NULL DEFAULT '', PRIMARY KEY (acct, fact));
CREATE TABLE IF NOT EXISTS dg_given (discord TEXT NOT NULL, role TEXT NOT NULL, at INTEGER NOT NULL, PRIMARY KEY (discord, role));
CREATE TABLE IF NOT EXISTS dg_picks (id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, at INTEGER NOT NULL,
  by TEXT NOT NULL, acct TEXT NOT NULL DEFAULT '', title TEXT NOT NULL, link TEXT NOT NULL DEFAULT '', line TEXT NOT NULL DEFAULT '',
  post INTEGER NOT NULL DEFAULT 0);
"""


def _int(v, d: int, lo: int, hi: int) -> int:
    try:
        return max(lo, min(hi, int(str(v).strip())))
    except (TypeError, ValueError):
        return d


class Config:
    def __init__(self, env=None):
        env = os.environ if env is None else env
        on = lambda k: env.get(k, "0").strip() == "1"   # noqa: E731
        self.guild = env.get("WU_DISCORD_GUILD", "").strip()
        self.feed = on("WU_DISCORD_FEED")
        self.feed_per_hour = _int(env.get("WU_DISCORD_FEED_PER_HOUR"), 4, 1, 30)
        self.lfg = on("WU_DISCORD_LFG")
        self.lfg_gap = _int(env.get("WU_DISCORD_LFG_GAP"), 30, 1, 1440) * 60
        self.lfg_per_hour = _int(env.get("WU_DISCORD_LFG_PER_HOUR"), 20, 1, 200)
        self.roles = on("WU_DISCORD_ROLES")
        self.roles_hour = _int(env.get("WU_DISCORD_ROLES_HOUR"), 4, 0, 23)
        self.automod = on("WU_DISCORD_AUTOMOD")
        self.template = env.get("WU_DISCORD_TEMPLATE", "").strip() or TEMPLATE_FILE


# --------------------------------------------------------------------------------------------- the template
def check_template(doc) -> list[str]:
    """What is wrong with a template (an empty list: good)."""
    bad = []
    if not isinstance(doc, dict) or doc.get("format") != 1:
        return ["not a format 1 template"]
    roles = doc.get("roles") if isinstance(doc.get("roles"), list) else []
    rkeys = set()
    for r in roles:
        if not isinstance(r, dict) or not KEY_RE.fullmatch(str(r.get("key") or "")) or not str(r.get("name") or "").strip():
            bad.append(f"a role needs a key and a name: {r!r}"[:120])
            continue
        if r.get("auto") and r["auto"] not in FACTS:
            bad.append(f"role {r['key']}: unknown auto fact {r['auto']!r}")
        if r.get("color") and not re.fullmatch(r"#[0-9a-fA-F]{6}", str(r["color"])):
            bad.append(f"role {r['key']}: color is #rrggbb")
        rkeys.add(r["key"])
    ckeys = set()
    for c in doc.get("categories") if isinstance(doc.get("categories"), list) else []:
        if not isinstance(c, dict) or not KEY_RE.fullmatch(str(c.get("key") or "")) or not str(c.get("name") or "").strip():
            bad.append("a category needs a key and a name")
            continue
        for ch in c.get("channels") if isinstance(c.get("channels"), list) else []:
            k = str((ch or {}).get("key") or "")
            if not KEY_RE.fullmatch(k) or not str(ch.get("name") or "").strip() or k in ckeys:
                bad.append(f"channel {k or '?'}: needs a new key and a name")
                continue
            ckeys.add(k)
            if ch.get("type", "text") not in TYPES:
                bad.append(f"channel {k}: type is text, voice, stage or forum")
            for rk in list(ch.get("private") or []) + list(ch.get("posters") or []):
                if rk not in rkeys:
                    bad.append(f"channel {k}: unknown role {rk!r}")
    for what, k in (doc.get("posts") or {}).items():
        if k not in ckeys:
            bad.append(f"posts.{what}: unknown channel {k!r}")
    return bad


def load_template(path: str = TEMPLATE_FILE) -> dict:
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    bad = check_template(doc)
    if bad:
        raise ValueError("; ".join(bad[:5]))
    return doc


def overwrites(ch: dict, guild: str, bot: str, role_ids: dict) -> list[dict] | None:
    """A channel's permission overwrites; None when a role it is private to doesn't exist (never made public by mistake)."""
    out, everyone = [], {"id": guild, "type": 0, "allow": 0, "deny": 0}
    if ch.get("private"):
        everyone["deny"] |= P_VIEW
        for rk in ch["private"]:
            if not role_ids.get(rk):
                return None
            out.append({"id": role_ids[rk], "type": 0, "allow": P_VIEW | P_SEND | P_READ_HISTORY | P_SEND_THREADS | P_CONNECT | P_SPEAK,
                        "deny": 0})
    if ch.get("read_only"):
        everyone["deny"] |= P_SEND | P_SEND_THREADS | P_CREATE_THREADS
        for rk in ch.get("posters") or []:
            if role_ids.get(rk):
                out.append({"id": role_ids[rk], "type": 0, "allow": P_SEND | P_SEND_THREADS, "deny": 0})
    if (ch.get("read_only") or ch.get("private")) and bot:
        out.append({"id": bot, "type": 1, "allow": P_VIEW | P_SEND | P_SEND_THREADS | P_READ_HISTORY, "deny": 0})
    if everyone["deny"]:
        out.insert(0, everyone)
    return [dict(o, allow=str(o["allow"]), deny=str(o["deny"])) for o in out]


def iso_epoch(s) -> int:
    if not isinstance(s, str) or not re.match(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d", s):
        return 0
    try:
        return calendar.timegm(time.strptime(s[:19], "%Y-%m-%dT%H:%M:%S"))
    except ValueError:
        return 0


# --------------------------------------------------------------------------------------------- the growth side
class Growth:
    def __init__(self, link, cfg: Config, fd):
        self.link, self.cfg, self.fd = link, cfg, fd
        self.fm, self.store = link.fm, link.store
        self.template, self.template_error = None, ""
        try:
            self.template = load_template(cfg.template)
        except (OSError, ValueError) as ex:
            self.template_error = str(ex)[:200]
            log.error("discord: the template didn't load: %s", self.template_error)
        with self.store.lock:
            self.store.db.executescript(SCHEMA)
        self.next_feed = 0.0
        self.next_check = 0.0

    # -- small helpers ---------------------------------------------------------------------------------------------
    def now(self) -> int:
        return self.link.now()

    def meta(self, k, v=None):
        return self.link.meta("dg." + k, v)

    def jmeta(self, k, v=None):
        if v is not None:
            self.meta(k, json.dumps(v, separators=(",", ":"), default=str))
            return v
        try:
            return json.loads(self.meta(k) or "null")
        except ValueError:
            return None

    def mapped(self, kind: str, key: str) -> str:
        r = self.store.one("SELECT id FROM dg_map WHERE kind = ? AND key = ?", (kind, key))
        return r[0] if r else ""

    def set_map(self, kind: str, key: str, ident: str, name: str) -> None:
        self.store.x("INSERT OR REPLACE INTO dg_map (kind, key, id, name) VALUES (?,?,?,?)", (kind, key, str(ident), name[:100]))

    def post_channel(self, what: str) -> str:
        key = ((self.template or {}).get("posts") or {}).get(what, "")
        return self.mapped("channel", key) if key else ""

    def pending(self, kind: str) -> bool:
        return bool(self.store.one("SELECT 1 FROM fd_outbox WHERE kind = ? LIMIT 1", (kind,)))

    def audit(self, action: str, target: str, before, after, note: str = "", user=None) -> None:
        adm = self.fm.P("admin")
        try:
            if adm is not None and hasattr(adm, "audit"):
                adm.audit(user or {"account": "discord-bot", "name": "Discord bot", "role": "admin", "via": "discord"},
                          action, str(target), before, after, True, note)
        except Exception:   # noqa: BLE001
            log.exception("discord: the audit log write failed")

    def name_of(self, acct: str) -> str:
        """The Leyline profile name (read only: never makes a profile), else the forum's, else "Player"."""
        try:
            st = getattr(self.fm.P("social"), "store", None)
            p = st.profile(acct) if st is not None and acct else None
            if p and p.get("name"):
                return str(p["name"])
        except Exception:   # noqa: BLE001
            pass
        r = self.store.one("SELECT name FROM forum_users WHERE acct = ?", (acct,))
        return (r[0] if r and r[0] else "") or "Player"

    def text_problem(self, s: str) -> str:
        if self.fm.BRAND_RE.search(s or ""):
            return "Leave the game's name out (Leyline's rule)."
        if s and self.fm._hook("bad_word", s):
            return "Please keep it friendly: a word in that isn't allowed."
        return ""

    def site_link(self, v: str) -> str:
        """A link the bot may post: a path on our own site, or https on our own host. "" for anything else."""
        v = (v or "").strip()
        if not v:
            return ""
        if v.startswith("/") and not v.startswith("//"):
            v = self.link.site() + v
        u = urllib.parse.urlsplit(v)
        if u.scheme != "https" or u.hostname != self.fm._public_host() or len(v) > 300 or any(c in v for c in " <>\"'"):
            return ""
        return v

    # -- the worker ------------------------------------------------------------------------------------------------
    def tick(self) -> None:
        """Called by forum_discord.Link.round(): the feed every minute, the nightly and AutoMod checks."""
        mono = time.monotonic()
        if self.cfg.feed and mono >= self.next_feed:
            self.next_feed = mono + FEED_EVERY
            self.feed_tick()
        if mono >= self.next_check:
            self.next_check = mono + 300
            t = self.now()
            if (self.cfg.roles and time.gmtime(t).tm_hour == self.cfg.roles_hour and t - int(self.meta("roles_at") or 0) > 20 * 3600
                    and not self.pending("dg.roles")):
                self.link.enqueue("dg.roles")
            if self.cfg.automod and not self.pending("dg.automod"):
                last = self.jmeta("automod") or {}
                if last.get("hash") != self.words_hash() or t - int(last.get("at") or 0) > 86400:
                    self.link.enqueue("dg.automod")

    def deliver(self, kind: str, d: dict) -> None:
        """An outbox item of ours (forum_discord.Link.deliver hands every "dg." kind here)."""
        if kind == "dg.post":
            self.send_post(int(d.get("id") or 0))
        elif kind == "dg.setup":
            self.setup(dry=False, by=d.get("by") or {})
        elif kind == "dg.roles":
            self.sync_roles()
        elif kind == "dg.automod":
            self.push_automod()
        else:
            log.warning("discord: unknown outbox item %s", kind)

    # -- setup -----------------------------------------------------------------------------------------------------
    def setup(self, dry: bool = True, by: dict | None = None) -> dict:
        """Make what the template has and the server doesn't. Never deletes or changes what is there. A report:
        {at, dry, made: [...], found: [...], failed: [[what, why]], guild}."""
        E = self.fd.DiscordError
        if self.template is None:
            raise ValueError("the template didn't load: " + self.template_error)
        g = self.link.guild
        if not g:
            raise ValueError("no server: set WU_DISCORD_GUILD (the server id) in portal.env")
        rep = {"at": self.now(), "dry": dry, "made": [], "found": [], "failed": [], "guild": g}

        def soft(what, ex):
            if ex.kind in ("auth", "down", "ratelimited"):
                raise ex
            why = {"forbidden": "the bot isn't allowed (Manage Channels / Manage Roles, and its role high enough)",
                   "bad": "Discord refused it"}.get(ex.kind, ex.kind)
            if ex.kind == "bad" and what.startswith("stage"):
                why = "Discord refused it (stages need a Community server: Server Settings > Enable Community)"
            rep["failed"].append([what, why])

        roles = self.link.api.call("GET", f"/guilds/{g}/roles") or []
        by_name = {str(r.get("name") or "").lower(): str(r.get("id")) for r in roles}
        role_ids = {}
        for r in self.template["roles"]:
            rid = by_name.get(r["name"].lower())
            if rid:
                role_ids[r["key"]] = rid
                rep["found"].append("role " + r["name"])
                if not dry:
                    self.set_map("role", r["key"], rid, r["name"])
                continue
            if dry:
                role_ids[r["key"]] = "new"
                rep["made"].append("role " + r["name"])
                continue
            try:
                made = self.link.api.call("POST", f"/guilds/{g}/roles", {
                    "name": r["name"], "permissions": "0", "color": int(str(r.get("color") or "#000000")[1:], 16),
                    "hoist": bool(r.get("hoist")), "mentionable": False}) or {}
                role_ids[r["key"]] = str(made.get("id") or "")
                self.set_map("role", r["key"], role_ids[r["key"]], r["name"])
                rep["made"].append("role " + r["name"])
            except E as ex:
                soft("role " + r["name"], ex)
        chans = self.link.api.call("GET", f"/guilds/{g}/channels") or []
        seen = {(str(c.get("name") or "").lower(), int(c.get("type") or 0)): c for c in chans}
        for cat in self.template["categories"]:
            have = seen.get((cat["name"].lower(), T_CATEGORY))
            cat_id = str(have["id"]) if have else ""
            if have:
                rep["found"].append("category " + cat["name"])
                if not dry:
                    self.set_map("category", cat["key"], cat_id, cat["name"])
            elif dry:
                rep["made"].append("category " + cat["name"])
                cat_id = "new"
            else:
                try:
                    cat_id = str((self.link.api.call("POST", f"/guilds/{g}/channels", {"name": cat["name"], "type": T_CATEGORY}) or {}).get("id") or "")
                    self.set_map("category", cat["key"], cat_id, cat["name"])
                    rep["made"].append("category " + cat["name"])
                except E as ex:
                    soft("category " + cat["name"], ex)
            for ch in cat.get("channels") or []:
                typ = TYPES[ch.get("type", "text")]
                label = ("stage " if typ == T_STAGE else "channel ") + ch["name"]
                c = seen.get((ch["name"].lower(), typ))
                if c:
                    rep["found"].append(label)
                    if not dry:
                        self.set_map("channel", ch["key"], str(c["id"]), ch["name"])
                    continue
                ow = overwrites(ch, g, self.link.bot_id, role_ids)
                if ow is None:
                    rep["failed"].append([label, "a role it is private to is missing, so it wasn't made (it would be public)"])
                    continue
                if dry:
                    rep["made"].append(label)
                    continue
                body = {"name": ch["name"], "type": typ, "permission_overwrites": ow}
                if cat_id and cat_id != "new":
                    body["parent_id"] = cat_id
                if ch.get("topic") and typ in (T_TEXT, T_FORUM):
                    body["topic"] = str(ch["topic"])[:1024]
                if ch.get("slowmode") and typ in (T_TEXT, T_FORUM):
                    body["rate_limit_per_user"] = _int(ch["slowmode"], 0, 0, 21600)
                try:
                    made = self.link.api.call("POST", f"/guilds/{g}/channels", body) or {}
                    self.set_map("channel", ch["key"], str(made.get("id") or ""), ch["name"])
                    rep["made"].append(label)
                except E as ex:
                    soft(label, ex)
        if dry:
            self.jmeta("preview", rep)
        else:
            self.jmeta("setup", rep)
            self.audit("discord.setup", g, None, {"made": len(rep["made"]), "found": len(rep["found"]), "failed": len(rep["failed"])},
                       "; ".join(f"{a}: {b}" for a, b in rep["failed"])[:300], user=by or None)
            log.info("discord: setup made %d, found %d, failed %d", len(rep["made"]), len(rep["found"]), len(rep["failed"]))
        return rep

    # -- posts -----------------------------------------------------------------------------------------------------
    def queue_post(self, kind: str, ref: str, what: str, content: str, button: tuple[str, str] | None = None,
                   send: bool = True) -> int:
        """A post recorded (dg_posts) and, when send, handed to the outbox. Its id."""
        body = {"content": content[:2000], "allowed_mentions": {"parse": []}, "flags": SUPPRESS_EMBEDS}
        if button and button[1]:
            body["components"] = [{"type": 1, "components": [{"type": 2, "style": 5, "label": button[0][:80], "url": button[1]}]}]
        pid = int(self.store.x("INSERT INTO dg_posts (at, kind, ref, channel, body, status) VALUES (?,?,?,?,?,?)",
                               (self.now(), kind, ref[:64], what, json.dumps(body, separators=(",", ":")),
                                "waiting" if send else "not posted (WU_DISCORD_FEED is off)")))
        if send:
            self.link.enqueue("dg.post", 0, id=pid)
        return pid

    def send_post(self, pid: int) -> None:
        r = self.store.one("SELECT channel, body, status FROM dg_posts WHERE id = ?", (pid,))
        if not r or r[2] not in ("waiting", "retrying"):
            return
        ch = self.post_channel(r[0])
        if not ch:
            self.store.x("UPDATE dg_posts SET status = 'not posted', error = ? WHERE id = ?",
                         (f"no {r[0]} channel: run Set up the Discord first", pid))
            return
        try:
            m = self.link.api.call("POST", f"/channels/{ch}/messages", json.loads(r[1])) or {}
        except self.fd.DiscordError as ex:
            st = "failed" if ex.kind in ("bad", "notfound") else "retrying"
            self.store.x("UPDATE dg_posts SET status = ?, error = ? WHERE id = ?", (st, f"{ex.kind}: {ex.detail}"[:200], pid))
            if ex.kind == "notfound":
                return
            raise
        self.store.x("UPDATE dg_posts SET status = 'sent', msg = ?, error = '' WHERE id = ?", (str(m.get("id") or ""), pid))

    def posts(self, n: int = 20, kind: str = "") -> list[dict]:
        rows = self.store.q("SELECT id, at, kind, ref, channel, body, status, error FROM dg_posts"
                            + (" WHERE kind = ?" if kind else "") + " ORDER BY id DESC LIMIT ?", ((kind, n) if kind else (n,)))
        out = []
        for i, at, k, ref, ch, body, st, err in rows:
            try:
                text = json.loads(body).get("content", "")
            except ValueError:
                text = ""
            out.append({"id": i, "at": at, "kind": k, "ref": ref, "channel": ch, "text": text, "status": st, "error": err})
        return out

    def sent_since(self, kind: str, since: int, ref: str = "") -> int:
        q = "SELECT COUNT(*) FROM dg_posts WHERE kind = ? AND at > ? AND status NOT LIKE 'not posted%'"
        args = [kind, since]
        if ref:
            q += " AND ref = ?"
            args.append(ref)
        return int(self.store.one(q, tuple(args))[0] or 0)

    # -- live realms -----------------------------------------------------------------------------------------------
    def realms(self) -> tuple[list[dict], bool]:
        cr = self.fm.P("community_realms")
        if not callable(cr):
            return [], False
        try:
            realms, ok = cr()
        except Exception:   # noqa: BLE001
            return [], False
        net = self.fm.P("network")
        out = []
        for r in realms or []:
            if not isinstance(r, dict) or not r.get("live") or r.get("hangout") or r.get("sample") or r.get("name_ok") is False:
                continue
            try:
                if net is not None and hasattr(net, "offered") and not net.offered(r.get("build")):
                    continue
            except Exception:   # noqa: BLE001
                pass
            name = str(r.get("name") or "")
            if not name or self.text_problem(name):
                continue
            out.append(r)
        return out, bool(ok)

    def mode_of(self, r: dict) -> str:
        m = r.get("mode_name") or r.get("mode")
        if isinstance(m, str) and m.strip() and not self.text_problem(m):
            return self.fm.clean_line(m, 40)
        labels = self.fm.P("RULESET_LABELS") or {}
        return labels.get(r.get("ruleset"), "Custom rules") if isinstance(labels, dict) else "Custom rules"

    def realm_url(self, r: dict) -> str:
        short = r.get("short") or r.get("share")
        if isinstance(short, str) and SHORT_RE.fullmatch(short):
            return f"{self.link.site()}/r/{short}"
        relay = r.get("relay") if isinstance(r.get("relay"), str) and HOST_RE.fullmatch(r["relay"]) else "relay.leylinelauncher.com"
        return f"https://{relay}/join/{r['id']}"

    def feed_tick(self) -> None:
        realms, ok = self.realms()
        if not ok:
            return
        t = self.now()
        last_tick = int(self.meta("feed_tick") or 0)
        watching = bool(last_tick) and t - last_tick <= 600   # away longer (off, or the portal down): no "back" cards
        self.meta("feed_tick", str(t))
        live = {}
        for r in realms:
            rid = str(r.get("id") or "")
            if not re.fullmatch(r"[a-z2-7]{16}", rid):
                continue
            live[rid] = r
            row = self.store.one("SELECT last_live, pending FROM dg_realms WHERE id = ?", (rid,))
            if row is None:
                self.store.x("INSERT INTO dg_realms (id, first_live, last_live, pending) VALUES (?,?,?,?)", (rid, t, t, t if watching else 0))
            else:
                pend = t if (watching and t - row[0] >= BACK_AFTER) else row[1]
                self.store.x("UPDATE dg_realms SET last_live = ?, pending = ? WHERE id = ?", (t, pend, rid))
        self.store.x("UPDATE dg_realms SET pending = 0 WHERE pending > 0 AND pending < ?", (t - PENDING_FOR,))
        budget = self.cfg.feed_per_hour - self.sent_since("live", t - 3600)
        if budget <= 0:
            return
        cands = [live[i] for (i,) in self.store.q("SELECT id FROM dg_realms WHERE pending > 0") if i in live]
        cands.sort(key=lambda r: (-(r.get("players") or 0), str(r.get("name", "")).lower()))
        for r in cands[:budget]:
            self.post_live(r)
            self.store.x("UPDATE dg_realms SET pending = 0, posted = ? WHERE id = ?", (t, r["id"]))

    def live_text(self, r: dict) -> str:
        esc = self.fd.esc
        n = r.get("players")
        players = (f" · {n} playing" if isinstance(n, int) and not isinstance(n, bool) and n > 0 else "")
        tag = r.get("tagline") if isinstance(r.get("tagline"), str) and not self.text_problem(r["tagline"]) else ""
        url = self.realm_url(r)
        return (f"**{esc(r['name'])}** is live · {esc(self.mode_of(r))}{players}\n" + (f"{esc(tag)}\n" if tag else "")
                + f"[Play](<{url}>)")

    def post_live(self, r: dict) -> int:
        return self.queue_post("live", r["id"], "live", self.live_text(r), ("Play", self.realm_url(r)))

    # -- staff picks -----------------------------------------------------------------------------------------------
    def pick(self, kind: str, by: dict, acct: str, title: str, link: str, line: str) -> tuple[dict, dict]:
        """Den of the week (kind "den") or the featured mode ("mode"). (before, after) for the audit log; raises ValueError."""
        acct = (acct or "").strip().upper()
        title = self.fm.clean_line(title, 60)
        line = self.fm.clean_line(line, 200)
        if acct and not ACCT_RE.fullmatch(acct):
            raise ValueError("The account is a game account id, like WR000123.")
        if kind == "den" and not acct:
            raise ValueError("Whose Den? Give their game account (WR...).")
        if not title:
            raise ValueError("Give it a name.")
        for s in (title, line):
            p = self.text_problem(s)
            if p:
                raise ValueError(p)
        url = self.site_link(link)
        if link and not url:
            raise ValueError(f"The link must be on our own site ({self.fm._public_host()}).")
        esc = self.fd.esc
        who = f" by **{esc(self.name_of(acct))}**" if acct else ""
        if kind == "den":
            text, btn = f"**Den of the week:** {esc(title)}{who}", ("Visit", url)
        else:
            text, btn = f"**Featured mode this week:** {esc(title)}{who}", ("Play", url)
        if line:
            text += "\n" + esc(line)
        if url:
            text += f"\n[{btn[0]}](<{url}>)"
        pid = self.queue_post(kind, acct or title, kind, text, btn, send=self.cfg.feed)
        self.store.x("INSERT INTO dg_picks (kind, at, by, acct, title, link, line, post) VALUES (?,?,?,?,?,?,?,?)",
                     (kind, self.now(), str(by.get("account") or ""), acct, title, url, line, pid))
        if kind == "den":
            self.record_fact(acct, "den_architect", title, str(by.get("account") or ""))
        return {}, {"kind": kind, "acct": acct, "title": title, "posted": self.cfg.feed}

    def picks(self, kind: str, n: int = 5) -> list[dict]:
        return [{"at": a, "by": b, "acct": c, "title": d, "link": e} for a, b, c, d, e in
                self.store.q("SELECT at, by, acct, title, link FROM dg_picks WHERE kind = ? ORDER BY id DESC LIMIT ?", (kind, n))]

    # -- find a group ----------------------------------------------------------------------------------------------
    def group(self, acct: str, realm: str = "", what: str = "", note: str = "") -> str:
        """Post "<name> wants a group for <realm or mode>" in looking-for-group. The answer text; raises fm.ForumError."""
        FE = self.fm.ForumError
        if not self.cfg.lfg:
            raise FE(404, "Find a group isn't on yet.")
        ln = self.link.link_of(acct)
        if not ln:
            raise FE(403, f"Link your Discord first: {self.link.link_url()}")
        blk = self.link.blocked(acct)
        if blk:
            raise FE(403, blk)
        t = self.now()
        if self.sent_since("lfg", t - self.cfg.lfg_gap, acct):
            raise FE(429, f"You asked for a group a moment ago. Try again in {self.cfg.lfg_gap // 60} minutes.")
        if self.sent_since("lfg", t - 3600) >= self.cfg.lfg_per_hour:
            raise FE(429, "Lots of people are looking right now. Try again in a few minutes.")
        realm = (realm or "").strip()
        note = self.fm.clean_line(note, 120)
        r = None
        if realm:
            r = next((x for x in self.realms()[0] if x.get("id") == realm), None)
            if r is None:
                raise FE(400, "That realm isn't live on the network right now. Pick another.")
            target, url = f"{r['name']} ({self.mode_of(r)})", self.realm_url(r)
        else:
            target, url = self.fm.clean_line(what, 40), ""
            if not target:
                raise FE(400, "Pick a realm or say what you want to play.")
        for s in (target, note):
            p = self.text_problem(s)
            if p:
                raise FE(400, p)
        esc = self.fd.esc
        text = f"**{esc(self.name_of(acct))}** wants a group for **{esc(target)}** · <@{ln['discord']}>"
        if note:
            text += "\n> " + esc(note)
        if url:
            text += f"\n[Join](<{url}>)"
        self.queue_post("lfg", acct, "lfg", text, ("Join", url))
        return "Posted in looking-for-group on the Discord. People can reply there or message you."

    def group_form(self, who: dict, query: dict, token: str, path: str) -> str:
        """The website's Find a group box (on /wrath/leyline/forum/discord), or "" when it's off or not linked."""
        if not self.cfg.lfg or not who or not self.link.link_of(who["acct"]):
            return ""
        e = html.escape
        pre = (query.get("group") or [""])[0]
        opts = "".join(f'<option value="{e(r["id"])}"{" selected" if r["id"] == pre else ""}>{e(r["name"])} · {e(self.mode_of(r))}</option>'
                       for r in sorted(self.realms()[0], key=lambda r: -(r.get("players") or 0))[:40])
        return (f'<form class="fm-form" method="post" action="{path}"><input type="hidden" name="t" value="{e(token)}">'
                '<h2>Find a group</h2><p>Posts in looking-for-group on the Discord, with a Join button.</p>'
                f'<p><label>Realm <select name="realm"><option value="">(none: say what below)</option>{opts}</select></label></p>'
                '<p><label>Or what you want to play <input type="text" name="what" maxlength="40" placeholder="a dungeon, a mode, PvP"></label></p>'
                '<p><label>A note (optional) <input type="text" name="note" maxlength="120"></label></p>'
                '<div class="fm-acts"><button class="fm-btn" type="submit" name="do" value="group">Post it</button></div></form>')

    # -- facts and roles -------------------------------------------------------------------------------------------
    def record_fact(self, acct: str, fact: str, source: str, by: str = "") -> bool:
        """A Leyline fact that earns a role (other modules may call this too). True when it is new."""
        acct = (acct or "").strip().upper()
        if not ACCT_RE.fullmatch(acct) or fact not in FACTS:
            return False
        new = not self.store.one("SELECT 1 FROM dg_facts WHERE acct = ? AND fact = ?", (acct, fact))
        self.store.x("INSERT OR REPLACE INTO dg_facts (acct, fact, source, at, by) VALUES (?,?,?,?,?)",
                     (acct, fact, self.fm.clean_line(source, 120), self.now(), by[:32]))
        return new

    def drop_fact(self, acct: str, fact: str) -> bool:
        return bool(self.store.x("DELETE FROM dg_facts WHERE acct = ? AND fact = ?", ((acct or "").strip().upper(), fact)))

    def facts(self, n: int = 50) -> list[dict]:
        return [{"acct": a, "fact": f, "source": s, "at": t, "by": b} for a, f, s, t, b in
                self.store.q("SELECT acct, fact, source, at, by FROM dg_facts ORDER BY at DESC LIMIT ?", (n,))]

    def hosts(self) -> set[str]:
        """Accounts that own a realm listed on the network for HOST_DAYS days (hostflow.py's records, the relay's first
        seen, or the feed's own memory)."""
        hf = self.fm.P("hostflow")
        if hf is None or not hasattr(hf, "all_ids"):
            return set()
        cr = self.fm.P("community_realms")
        try:
            listed = {r["id"]: r for r in (cr()[0] if callable(cr) else []) if isinstance(r, dict) and r.get("id")}
        except Exception:   # noqa: BLE001
            listed = {}
        t, out = self.now(), set()
        try:
            recs = [hf.load(i) for i in hf.all_ids()]
        except Exception:   # noqa: BLE001
            return set()
        for rec in recs:
            if not rec or not rec.get("realm_id") or not ACCT_RE.fullmatch(str(rec.get("owner") or "").upper()):
                continue
            r = listed.get(rec["realm_id"])
            if r is None:
                continue
            since = iso_epoch(r.get("first_seen"))
            mine = self.store.one("SELECT first_live FROM dg_realms WHERE id = ?", (rec["realm_id"],))
            if mine:
                since = min(since, mine[0]) if since else mine[0]
            if since and t - since >= HOST_DAYS * 86400:
                out.add(str(rec["owner"]).upper())
        return out

    def earned(self) -> dict[str, set]:
        out: dict[str, set] = {}
        for acct, fact in self.store.q("SELECT acct, fact FROM dg_facts"):
            out.setdefault(acct, set()).add(fact)
        for acct in self.hosts():
            out.setdefault(acct, set()).add("realm_host")
        return out

    def sync_roles(self) -> dict:
        """Linked people get the roles their facts earn; roles the bot gave and no longer earned are taken back.
        Resumable (a cursor), idempotent, every change audited."""
        E = self.fd.DiscordError
        g = self.link.guild
        auto = [(r["key"], r["auto"], bool(r.get("keep"))) for r in (self.template or {}).get("roles", []) if r.get("auto")]
        ids = {k: self.mapped("role", k) for k, _f, _keep in auto}
        rep = self.jmeta("roles_run") or {}
        cursor = self.meta("roles_cursor")
        if not cursor:
            rep = {"started": self.now(), "added": 0, "removed": 0, "people": 0, "absent": 0, "errors": []}
        if not g or not any(ids.values()):
            rep.update(at=self.now(), errors=["no server or no roles yet: run Set up the Discord first"])
            self.jmeta("roles_last", rep)
            self.meta("roles_at", str(self.now()))
            return rep
        earned = self.earned()
        links = dict(self.store.q("SELECT discord, acct FROM fd_links"))
        given = {}
        for d, role in self.store.q("SELECT discord, role FROM dg_given"):
            given.setdefault(d, set()).add(role)
        people = sorted(set(links) | set(given), key=lambda x: int(x) if x.isdigit() else 0)
        for did in people:
            if cursor and (int(did) if did.isdigit() else 0) <= int(cursor):
                continue
            acct = links.get(did, "")
            facts = earned.get(acct, set()) if acct else set()
            try:
                mem = self.link.api.call("GET", f"/guilds/{g}/members/{did}") or {}
            except E as ex:
                if ex.kind != "notfound":
                    self.jmeta("roles_run", rep)
                    raise
                mem = None
            if mem is None:
                rep["absent"] += 1
                self.meta("roles_cursor", did)
                continue
            rep["people"] += 1
            have = {str(x) for x in mem.get("roles") or []}
            mine = given.get(did, set())
            for key, fact, keep in auto:
                rid = ids.get(key)
                if not rid:
                    continue
                try:
                    if fact in facts and rid not in have:
                        self.link.api.call("PUT", f"/guilds/{g}/members/{did}/roles/{rid}")
                        self.store.x("INSERT OR REPLACE INTO dg_given (discord, role, at) VALUES (?,?,?)", (did, key, self.now()))
                        rep["added"] += 1
                        self.audit("discord.role.add", acct or did, None, {"role": key}, f"earned: {FACTS[fact]}")
                    elif fact not in facts and rid in have and key in mine and not (keep and acct):
                        self.link.api.call("DELETE", f"/guilds/{g}/members/{did}/roles/{rid}")
                        self.store.x("DELETE FROM dg_given WHERE discord = ? AND role = ?", (did, key))
                        rep["removed"] += 1
                        self.audit("discord.role.remove", acct or did, {"role": key}, None,
                                   "no longer earned" if acct else "the Discord account was unlinked")
                    elif fact not in facts and rid not in have and key in mine:
                        self.store.x("DELETE FROM dg_given WHERE discord = ? AND role = ?", (did, key))
                except E as ex:
                    if ex.kind in ("auth", "down", "ratelimited"):
                        self.jmeta("roles_run", rep)
                        raise
                    if len(rep["errors"]) < 10:
                        rep["errors"].append(f"{key}: {ex.kind} (is the bot's role above the {key.replace('_', ' ')} role?)")
            self.meta("roles_cursor", did)
        rep["at"] = self.now()
        self.meta("roles_cursor", "")
        self.meta("roles_run", "null")
        self.jmeta("roles_last", rep)
        self.meta("roles_at", str(self.now()))
        log.info("discord: roles synced: %d added, %d removed, %d people", rep["added"], rep["removed"], rep["people"])
        return rep

    # -- AutoMod ---------------------------------------------------------------------------------------------------
    def _namecheck(self):
        m = sys.modules.get("wu_namecheck")
        if m is None:
            soc = self.fm.P("social")
            fn = getattr(soc, "_namecheck", None) if soc is not None else None
            try:
                m = fn() if callable(fn) else None
            except Exception:   # noqa: BLE001
                m = None
        return m

    def words(self) -> tuple[list[str], list[str]]:
        """(keyword_filter, allow_list) for Discord: "word" words whole, "inside" words as *w*; slurs, sexual terms,
        profanity and the owner's own words (not the staff and brand rules: those are for names)."""
        nc = self._namecheck()
        if nc is None:
            return [], []
        L = nc.lists()
        kw, seen = [], set()
        for kind, w, mode, *_r in L.entries:
            if kind not in ("slur", "sexual", "profanity", "custom"):
                continue
            k = f"*{w}*" if mode == "inside" else w
            if k not in seen and len(k) <= 60:
                seen.add(k)
                kw.append(k)
        return kw[:1000], sorted(L.allow)[:100]

    def words_hash(self) -> str:
        kw, allow = self.words()
        return hashlib.sha256(json.dumps([kw, allow]).encode()).hexdigest()[:16]

    def push_automod(self) -> dict:
        g = self.link.guild
        kw, allow = self.words()
        rep = {"at": self.now(), "words": len(kw), "hash": self.words_hash(), "did": "", "error": ""}
        if not g or not kw:
            rep["error"] = "no server" if not g else "no blocked words here (namecheck.py missing?)"
            return self.jmeta("automod", rep)
        meta = {"keyword_filter": kw, "allow_list": allow}
        actions = [{"type": 1, "metadata": {"custom_message": "That word isn't allowed here (Leyline's blocked words)."}}]
        try:
            rules = self.link.api.call("GET", f"/guilds/{g}/auto-moderation/rules") or []
            ours = next((r for r in rules if r.get("name") == RULE_NAME and int(r.get("trigger_type") or 0) == 1), None)
            if ours:
                self.link.api.call("PATCH", f"/guilds/{g}/auto-moderation/rules/{ours['id']}",
                                   {"trigger_metadata": meta, "actions": actions})
                rep["did"] = "updated"
            else:
                self.link.api.call("POST", f"/guilds/{g}/auto-moderation/rules", {
                    "name": RULE_NAME, "event_type": 1, "trigger_type": 1, "trigger_metadata": meta, "actions": actions,
                    "enabled": True})
                rep["did"] = "made"
        except self.fd.DiscordError as ex:
            if ex.kind in ("auth", "down", "ratelimited"):
                raise
            rep["error"] = {"forbidden": "the bot needs Manage Server for AutoMod"}.get(ex.kind, f"Discord refused it ({ex.kind})")
            rep["hash"] = ""
        self.jmeta("automod", rep)
        if rep["did"]:
            self.audit("discord.automod", g, None, {"words": len(kw)}, f"AutoMod rule {rep['did']}")
        return rep

    # -- HQ --------------------------------------------------------------------------------------------------------
    def status(self) -> dict:
        n = self.store.one("SELECT COUNT(*) FROM fd_outbox WHERE kind LIKE 'dg.%'")[0]
        errs = [r[0] for r in self.store.q("SELECT error FROM fd_outbox WHERE kind LIKE 'dg.%' AND error != '' ORDER BY id LIMIT 5")]
        return {"guild": self.link.guild, "feed": self.cfg.feed, "lfg": self.cfg.lfg, "roles": self.cfg.roles,
                "automod": self.cfg.automod, "template": self.template_error or "ok", "waiting": n, "errors": errs,
                "setup": self.jmeta("setup"), "roles_last": self.jmeta("roles_last"), "automod_last": self.jmeta("automod"),
                "channels": int(self.store.one("SELECT COUNT(*) FROM dg_map WHERE kind = 'channel'")[0]),
                "roles_mapped": int(self.store.one("SELECT COUNT(*) FROM dg_map WHERE kind = 'role'")[0])}

    def act(self, name: str, user: dict, target: str, form: dict, Refused):
        """HQ's actions (stream/hq.py registers them; admin.run_action checks roles, confirms and audits)."""
        who = {k: user.get(k) for k in ("account", "name", "role")}
        try:
            if name == "discord.preview":
                rep = self.setup(dry=True)
                return None, {"would_make": len(rep["made"]), "found": len(rep["found"])}
            if name == "discord.setup":
                if self.template is None:
                    raise ValueError("the template didn't load: " + self.template_error)
                if not self.link.guild:
                    raise ValueError("no server: set WU_DISCORD_GUILD in portal.env first")
                self.link.enqueue("dg.setup", 0, by=dict(who, via="discord"))
                return None, {"queued": True}
            if name in ("discord.den", "discord.mode"):
                return self.pick(name[8:], user, form.get("acct", ""), form.get("title", ""), form.get("link", ""), form.get("line", ""))
            if name == "discord.fact.add":
                fact = str(form.get("fact") or "")
                if fact not in MANUAL_FACTS:
                    raise ValueError("Pick a fact.")
                acct = str(target or form.get("acct") or "").strip().upper()
                if not ACCT_RE.fullmatch(acct):
                    raise ValueError("The account is a game account id, like WR000123.")
                note = self.fm.clean_line(form.get("note", ""), 120) or "recorded in HQ"
                self.record_fact(acct, fact, note, str(user.get("account") or ""))
                return None, {"acct": acct, "fact": fact}
            if name == "discord.fact.remove":
                acct, _, fact = str(target).partition(":")
                if not self.drop_fact(acct, fact):
                    raise ValueError("That fact isn't recorded.")
                return {"acct": acct, "fact": fact}, None
            if name == "discord.roles.run":
                if not self.pending("dg.roles"):
                    self.link.enqueue("dg.roles")
                return None, {"queued": True}
            if name == "discord.automod.push":
                if not self.cfg.automod:
                    raise ValueError("AutoMod is off: WU_DISCORD_AUTOMOD=1 in portal.env turns it on.")
                if not self.pending("dg.automod"):
                    self.link.enqueue("dg.automod")
                return None, {"queued": True}
        except ValueError as ex:
            raise Refused(str(ex)) from None
        except self.fd.DiscordError as ex:
            raise Refused(f"Discord didn't answer that ({ex.kind}). Try again in a minute.") from None
        raise Refused("Unknown Discord action.")

    def hq_page(self, user: dict, A) -> str:
        e = html.escape
        back = "/wrath/admin/discord"
        st = self.status()
        chip = lambda on: '<span class="tag good">on</span>' if on else '<span class="tag">off</span>'   # noqa: E731
        when = lambda t: time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime(int(t))) if t else "never"   # noqa: E731
        link_st = self.link.status()
        feats = (f'<ul class="hq-list"><li>{chip(True)} The forum link: {e(link_st["state"])}; {link_st["links"]} linked '
                 f'account{"" if link_st["links"] == 1 else "s"}.</li>'
                 f'<li>{chip(bool(st["guild"]))} Server: <code>{e(st["guild"] or "not known: set WU_DISCORD_GUILD")}</code></li>'
                 f'<li>{chip(st["feed"])} Live realms, Den of the week, featured mode <code>WU_DISCORD_FEED</code> '
                 f'(at most {self.cfg.feed_per_hour} realm cards an hour)</li>'
                 f'<li>{chip(st["lfg"])} Find a group <code>WU_DISCORD_LFG</code></li>'
                 f'<li>{chip(st["roles"])} Roles from accounts, nightly at {self.cfg.roles_hour:02d}:00 UTC <code>WU_DISCORD_ROLES</code></li>'
                 f'<li>{chip(st["automod"])} AutoMod with our blocked words <code>WU_DISCORD_AUTOMOD</code></li></ul>')
        if st["template"] != "ok":
            feats += f'<p class="hq-warn">The template didn\'t load: {e(st["template"])}</p>'
        errs = ([self.link.last_error] if self.link.last_error else []) + st["errors"]
        err = (f'<p class="hq-warn" style="overflow-wrap:anywhere">Last problem: {e(errs[0])}</p>' if errs else "")
        head = (f'<section class="ly-panel"><h2>Discord</h2><p class="fine">The Leyline bot on the Discord. Each part is off '
                f'until portal.env turns it on (docs/DISCORD.md, then restart the portal). {st["waiting"]} waiting to go to Discord.</p>'
                f'{feats}{err}</section>')

        def rep_html(rep, title):
            if not rep:
                return f'<p class="fine">{e(title)}: never run.</p>'
            made = ", ".join(rep.get("made") or []) or "nothing"
            failed = "".join(f'<li>{e(a)}: {e(b)}</li>' for a, b in rep.get("failed") or [])
            return (f'<p class="fine">{e(title)} ({when(rep.get("at"))}): {"would make" if rep.get("dry") else "made"} '
                    f'{e(made)}; already there: {len(rep.get("found") or [])}.</p>' + (f'<ul class="hq-list">{failed}</ul>' if failed else ""))
        setup = (f'<section class="ly-panel"><h2>Set up the Discord</h2><p class="fine">Makes the channels and roles of the '
                 f'plan that the server doesn\'t have yet ({st["channels"]} channels and {st["roles_mapped"]} roles known). '
                 f'It never deletes, renames or changes anything already there, so it is safe to run again. Preview first.</p>'
                 + A.act(user, "discord.preview", "plan", "Preview", back)
                 + A.act(user, "discord.setup", "plan", "Set up the Discord", back, confirm="setup")
                 + rep_html(self.jmeta("preview"), "Preview") + rep_html(st["setup"], "Last setup") + '</section>')
        pick_fields = lambda den: (   # noqa: E731
            (f'<input type="text" name="acct" maxlength="16" placeholder="their game account (WR...)" aria-label="Game account" size="16"{" required" if den else ""}>')
            + f'<input type="text" name="title" maxlength="60" placeholder="{"the Den" if den else "the mode"}\'s name" aria-label="Name" required>'
            + '<input type="text" name="link" maxlength="300" placeholder="a link on our site (optional)" aria-label="Link">'
            + '<input type="text" name="line" maxlength="200" placeholder="one line about it (optional)" aria-label="One line">')
        last = lambda k: "".join(f'<li>{when(p["at"])}: {e(p["title"])}' + (f' ({e(p["acct"])})' if p["acct"] else "") + '</li>'   # noqa: E731
                                 for p in self.picks(k))
        picks = (f'<section class="ly-panel"><h2>Staff picks</h2><p class="fine">Posted in den-showcase and modes-and-forks when the '
                 f'feed is on. Den of the week also gives its maker the Den Architect role.</p><h3>Den of the week</h3>'
                 + A.act(user, "discord.den", "pick", "Post Den of the week", back, fields=pick_fields(True))
                 + (f'<ul class="hq-list">{last("den")}</ul>' if last("den") else "")
                 + '<h3>Featured mode</h3>' + A.act(user, "discord.mode", "pick", "Post the featured mode", back, fields=pick_fields(False))
                 + (f'<ul class="hq-list">{last("mode")}</ul>' if last("mode") else "") + '</section>')
        fact_opts = "".join(f'<option value="{k}">{e(FACTS[k])}</option>' for k in MANUAL_FACTS)
        frows = "".join(f'<li><code>{e(f["acct"])}</code> {e(FACTS.get(f["fact"], f["fact"]))}: {e(f["source"])} ({when(f["at"])})'
                        + A.act(user, "discord.fact.remove", f'{f["acct"]}:{f["fact"]}', "Remove", back) + '</li>' for f in self.facts(30))
        rl = st["roles_last"] or {}
        rl_line = (f'Last run {when(rl.get("at"))}: {rl.get("added", 0)} added, {rl.get("removed", 0)} taken back, '
                   f'{rl.get("people", 0)} people checked, {rl.get("absent", 0)} not in the server.'
                   + (" Problems: " + "; ".join(rl.get("errors") or []) if rl.get("errors") else "")) if rl else "Never run."
        roles = (f'<section class="ly-panel"><h2>Roles from accounts</h2><p class="fine">Linked Discord people get Realm Host '
                 f'(a listed realm for 7 days, automatic), Mode Maker, Builder, Node Keeper and Den Architect from these facts. '
                 f'Guide, Moderator and Core Team stay manual in Discord. {e(rl_line)}</p>'
                 + A.act(user, "discord.roles.run", "now", "Run the role sync now", back)
                 + '<h3>Record a fact</h3>'
                 + A.act(user, "discord.fact.add", "", "Record", back, fields=(
                     '<input type="text" name="acct" maxlength="16" placeholder="game account (WR...)" aria-label="Game account" size="16" required>'
                     f'<select name="fact" aria-label="Fact">{fact_opts}</select>'
                     '<input type="text" name="note" maxlength="120" placeholder="why (optional)" aria-label="Why">'))
                 + (f'<ul class="hq-list">{frows}</ul>' if frows else "") + '</section>')
        am = st["automod_last"] or {}
        am_line = (f'Last push {when(am.get("at"))}: {am.get("words", 0)} words, ' + (e(am.get("did") or "") or "nothing changed")
                   + (f'. Problem: {e(am["error"])}' if am.get("error") else "")) if am else "Never pushed."
        automod = (f'<section class="ly-panel"><h2>AutoMod</h2><p class="fine">Our blocked words (Content > Blocked words) as one '
                   f'Discord AutoMod rule, "{RULE_NAME}". Turning the rule off in Discord is respected. {am_line}</p>'
                   + (A.act(user, "discord.automod.push", "now", "Push the words now", back) if self.cfg.automod else "") + '</section>')
        plain = lambda t: re.sub(r"\\(.)", r"\1", t)[:200]   # noqa: E731: the text without Discord's escapes
        prow = "".join(f'<tr><td class="name">{when(p["at"])} <span class="tag">{e(p["kind"])}</span></td><td style="overflow-wrap:anywhere">{e(plain(p["text"]))}</td>'
                       f'<td style="overflow-wrap:anywhere">{e(p["status"])}' + (f' · {e(p["error"][:80])}' if p["error"] else "") + '</td></tr>'
                       for p in self.posts(20))
        posts = (f'<section class="ly-panel"><h2>Last posts</h2>' + (f'<table class="ad-t"><tbody>{prow}</tbody></table>' if prow
                 else '<p class="fine">Nothing posted yet.</p>') + '</section>')
        return head + setup + picks + roles + automod + posts


# --------------------------------------------------------------------------------------------- the portal's side
def attach(link, env=None, fd=None) -> Growth:
    return Growth(link, Config(env), fd)


def group_http(fm, h, link) -> None:
    """POST /wrath/forum/api/discord/group {realm?, what?, note?}: Find a group from the launcher (Bearer token only, as
    the forum's other launcher calls: no cookie, no CORS)."""
    doc = fm._read_json(h)
    g = getattr(link, "growth", None) if link is not None else None
    if g is None or not g.cfg.lfg:
        return fm._jpost(h, 404, {"error": "Find a group isn't on yet."})
    if not h.headers.get("Authorization", "").startswith("Bearer "):
        return fm._jpost(h, 401, {"error": "Sign in from the launcher first."})
    if not isinstance(doc, dict):
        return fm._jpost(h, 400, {"error": "Bad request."})
    lim = fm.P("browse_limit")
    if lim is not None and not lim.hit("discord-group|" + h.ip(), 30):
        return fm._jpost(h, 429, {"error": "Slow down a little."})
    try:
        who = fm.token_viewer(h)
        s = lambda k: doc.get(k) if isinstance(doc.get(k), str) else ""   # noqa: E731
        text = g.group(who["acct"], s("realm"), s("what"), s("note"))
    except fm.ForumError as ex:
        return fm._jpost(h, ex.status, {"error": ex.text})
    return fm._jpost(h, 200, {"ok": True, "text": text})
