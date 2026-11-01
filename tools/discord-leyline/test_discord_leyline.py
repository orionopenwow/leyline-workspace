"""Leyline Discord redo (discord_leyline.py) against a fake Discord REST API on localhost.

Covers: the layout is docs/DISCORD-PLAN.md's and agrees with stream/discord_template_v2.json (so the website bot's
Set up the Discord finds every channel afterwards); the preview never writes; the backup lands before the first write;
channels from earlier passes (glyph names, old names) are renamed and moved with their history, not made twice; old
channels move into a hidden read-only ARCHIVE; read-only and private permissions; a second run changes nothing; 429
and 5xx retries; the @everyone role, managed roles, roles above the bot and the owner's roles are never touched; the
delete flags need their typed confirmations; first posts are only posted with the flag and a typed POST, and never
with TODO(owner) or in Start here; the token never reaches the output or any file; no banned words.

    python3 tools/discord-leyline/test_discord_leyline.py
"""
import copy
import json
import os
import subprocess
import sys
import tempfile
import threading
import unittest
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "discord_leyline.py"
TEMPLATE = HERE.parent.parent / "stream" / "discord_template_v2.json"
sys.path.insert(0, str(HERE))
import discord_leyline as dl  # noqa: E402

TOKEN = "MTk4NjIyNDgzNDcxOTI1MjQ4.FAKE-tok.en_SECRET_value"
GID, BOT, OWNER = "1", "900", "100"


def base_world(community=False):
    roles = [
        {"id": GID, "name": "@everyone", "position": 0, "color": 0, "hoist": False, "managed": False,
         "permissions": "1024"},
        {"id": "50", "name": "Leyline setup", "position": 10, "color": 0, "hoist": False, "managed": True,
         "tags": {"bot_id": BOT}, "permissions": "8"},
        {"id": "51", "name": "Admin", "position": 11, "color": 0xff0000, "hoist": True, "managed": False,
         "permissions": "8"},
        {"id": "52", "name": "Founder", "position": 5, "color": 0x00ff00, "hoist": True, "managed": False,
         "permissions": "0"},
        {"id": "53", "name": "Realm-Ally", "position": 3, "color": 0x123456, "hoist": False, "managed": False,
         "permissions": "0"},
        {"id": "54", "name": "moderator", "position": 4, "color": 0x111111, "hoist": False, "managed": False,
         "permissions": "8192"},
        {"id": "55", "name": "Server Booster", "position": 2, "color": 0xf47fff, "hoist": False, "managed": True,
         "tags": {"premium_subscriber": None}, "permissions": "0"},
    ]

    def ch(i, name, t, parent=None, pos=0, ow=None, topic=None):
        return {"id": i, "name": name, "type": t, "parent_id": parent, "position": pos, "guild_id": GID,
                "permission_overwrites": ow or [], "topic": topic, "rate_limit_per_user": 0}
    channels = [
        ch("10", "OpenVanilla", 4, pos=0),
        ch("11", "Text Channels", 4, pos=1),
        ch("20", "general", 0, "11", 0, topic="old topic"),
        ch("21", "announcements", 0, "11", 1, ow=[{"id": GID, "type": 0, "allow": "0", "deny": "2048"}]),
        ch("22", "ov-realm-help", 0, "10", 0),
        ch("23", "wrath-dev", 0, "10", 1, ow=[{"id": "53", "type": 0, "allow": "1024", "deny": "0"}]),
        ch("24", "rules", 0, "11", 2),
        ch("30", "General", 2, "11", 0),
        ch("31", "AFK", 2, None, 5),
    ]
    guild = {"id": GID, "name": "OpenVanilla Wrath", "owner_id": OWNER, "icon": None, "banner": None,
             "splash": None, "system_channel_id": "24", "afk_channel_id": "31", "rules_channel_id": None,
             "public_updates_channel_id": None, "features": []}
    if community:
        guild["features"] = ["COMMUNITY", "NEWS", "BANNER"]
        guild["rules_channel_id"] = "24"
        channels.append(ch("25", "moderator-only", 0, "11", 3,
                           ow=[{"id": GID, "type": 0, "allow": "0", "deny": "1024"}]))
        guild["public_updates_channel_id"] = "25"
    msgs = {"20": [{"id": str(100000 + i), "content": f"hello {i}", "timestamp": f"2026-01-01T00:00:{i % 60:02d}",
                    "author": {"id": "7", "username": "ann", "global_name": "Ann"},
                    "attachments": [{"url": f"https://cdn.example/{i}.png"}] if i % 50 == 0 else []}
                   for i in range(250)][::-1],
            "22": [{"id": "200001", "content": "realm down?", "timestamp": "2026-02-01T00:00:00",
                    "author": {"id": "8", "username": "bo"}, "attachments": []}]}
    members = {BOT: {"user": {"id": BOT}, "roles": ["50"]}, OWNER: {"user": {"id": OWNER}, "roles": ["52"]}}
    return {"guild": guild, "roles": roles, "channels": channels, "messages": msgs, "members": members,
            "guilds": [{"id": GID, "name": guild["name"]}]}


def reset_world(community=False):
    """The server as the 30 Oct reset left it: glyph and emoji names, START/PLAY/CREATE/FEEDBACK, ARCHIVE, the
    reset's four roles, and a Leyline website bot already in the server."""
    w = base_world(community)
    w["roles"] = [r for r in w["roles"] if r["id"] in (GID, "50", "51", "52", "55")] + [
        {"id": "60", "name": "Moderator", "position": 9, "color": 0x8B5CF6, "hoist": True, "managed": False,
         "permissions": "0"},
        {"id": "61", "name": "Helper", "position": 8, "color": 0x5B8DEF, "hoist": True, "managed": False,
         "permissions": "0"},
        {"id": "62", "name": "Creator", "position": 7, "color": 0xA78BFA, "hoist": True, "managed": False,
         "permissions": "0"},
        {"id": "63", "name": "Member", "position": 6, "color": 0x93B4F5, "hoist": False, "managed": False,
         "permissions": "0"},
        {"id": "64", "name": "Leyline", "position": 4, "color": 0, "hoist": False, "managed": True,
         "tags": {"bot_id": "901"}, "permissions": "0"},
    ]
    w["roles"][1]["position"] = 12          # the setup bot's role on top

    def ch(i, name, t, parent=None, pos=0, ow=None):
        return {"id": i, "name": name, "type": t, "parent_id": parent, "position": pos, "guild_id": GID,
                "permission_overwrites": ow or [], "topic": "old", "rate_limit_per_user": 0}
    ro = [{"id": GID, "type": 0, "allow": "0", "deny": str(dl.P_NO_TALK)}]
    news = 5 if community else 0
    w["channels"] = [
        ch("10", "START", 4, pos=0), ch("11", "PLAY", 4, pos=1), ch("12", "CREATE", 4, pos=2),
        ch("13", "FEEDBACK", 4, pos=3), ch("14", "ARCHIVE", 4, pos=4),
        ch("20", "✦︱welcome", 0, "10", 0, ro), ch("21", "📣︱announcements", news, "10", 1, ro),
        ch("22", "◆︱general", 0, "11", 0), ch("23", "🛟︱help", 0, "11", 1), ch("24", "🤝︱looking-for-group", 0, "11", 2),
        ch("25", "🎧︱Hangout", 2, "11", 3),
        ch("26", "🧩︱mods-and-workshop", 0, "12", 0), ch("27", "⟡︱host-a-beacon", 0, "12", 1),
        ch("28", "📸︱showcase", 0, "12", 2),
        ch("29", "·︱bug-reports", 0, "13", 0), ch("30", "💡︱ideas", 0, "13", 1),
        ch("31", "old-stuff", 0, "14", 0), ch("32", "rules", 0, "14", 1),
    ]
    w["guild"].update(name="Leyline", system_channel_id="20", afk_channel_id=None,
                      rules_channel_id="20" if community else None)
    if community:
        w["guild"]["public_updates_channel_id"] = None
    w["messages"] = {"22": w["messages"]["20"], "30": [{"id": "300001", "content": "an idea", "author": {"id": "7"},
                                                        "timestamp": "2026-10-30T00:00:00", "attachments": []}]}
    return w


class Fake:
    def __init__(self, world, backup_root):
        self.w = world
        self.backup_root = Path(backup_root)
        self.log = []                # every request: (method, path, body, headers, backup_ready)
        self.fail_once = {}          # (method, path) -> status to answer once
        self.served_429 = 0
        self.served_5xx = 0
        self.next_id = 5000
        self.lock = threading.Lock()

    def writes(self):
        return [x for x in self.log if x[0] != "GET"]

    def backup_ready(self):
        return any((d / "channels.json").exists() for d in self.backup_root.glob("Leyline-discord-backup-*"))

    def nid(self):
        self.next_id += 1
        return str(self.next_id)

    def chan(self, cid):
        return next((c for c in self.w["channels"] if c["id"] == cid), None)

    def handle(self, method, path, query, body, headers):
        with self.lock:
            self.log.append((method, path, body, dict(headers), self.backup_ready() if method != "GET" else None))
            if headers.get("Authorization") != "Bot " + TOKEN:
                return 401, {"message": "401: Unauthorized", "code": 0}
            key = (method, path)
            if key in self.fail_once:
                st = self.fail_once.pop(key)
                if st == 429:
                    self.served_429 += 1
                    return 429, {"message": "You are being rate limited.", "retry_after": 0.01, "global": False}
                self.served_5xx += 1
                return st, {"message": "oops"}
            return self.route(method, path, query, body)

    def route(self, m, path, q, body):
        w = self.w
        p = path.split("/")[1:]
        if path == "/users/@me":
            return 200, {"id": BOT, "username": "Leyline setup", "bot": True}
        if path == "/users/@me/guilds":
            return 200, w["guilds"]
        if p[0] == "guilds" and p[1] == GID:
            rest = p[2:]
            if not rest:
                if m == "GET":
                    return 200, w["guild"]
                if m == "PATCH":
                    for k, v in body.items():
                        if k in ("icon", "banner", "splash"):
                            w["guild"][k] = "h" + str(abs(hash(v)) % 10 ** 8) if v else None
                        else:
                            w["guild"][k] = v
                    return 200, w["guild"]
            if rest == ["roles"]:
                if m == "GET":
                    return 200, w["roles"]
                if m == "POST":
                    for r in w["roles"]:
                        if r["id"] != GID and r["position"] >= 1:
                            r["position"] += 1
                    r = {"id": self.nid(), "name": body["name"], "color": body.get("color", 0),
                         "hoist": body.get("hoist", False), "position": 1, "managed": False,
                         "mentionable": body.get("mentionable", False), "permissions": body.get("permissions", "0")}
                    w["roles"].append(r)
                    return 200, r
                if m == "PATCH":
                    for b in body:
                        next(r for r in w["roles"] if r["id"] == b["id"])["position"] = b["position"]
                    return 200, w["roles"]
            if rest[0] == "roles" and len(rest) == 2:
                r = next((r for r in w["roles"] if r["id"] == rest[1]), None)
                if r is None:
                    return 404, {"message": "Unknown Role"}
                if m == "PATCH":
                    r.update(body)
                    return 200, r
                if m == "DELETE":
                    w["roles"].remove(r)
                    return 204, None
            if rest == ["channels"]:
                if m == "GET":
                    return 200, w["channels"]
                if m == "POST":
                    if body.get("type") in (13, 15) and "COMMUNITY" not in w["guild"]["features"]:
                        return 400, {"message": "Invalid Form Body (needs Community)"}
                    c = {"id": self.nid(), "guild_id": GID, "name": body["name"], "type": body.get("type", 0),
                         "parent_id": body.get("parent_id"), "topic": body.get("topic"),
                         "rate_limit_per_user": body.get("rate_limit_per_user", 0),
                         "permission_overwrites": body.get("permission_overwrites") or [],
                         "position": max([c["position"] for c in w["channels"]] or [0]) + 1}
                    w["channels"].append(c)
                    return 201, c
                if m == "PATCH":
                    for b in body:
                        self.chan(b["id"])["position"] = b["position"]
                    return 204, None
            if rest[0] == "members":
                mem = w["members"].get(rest[1])
                return (200, mem) if mem else (404, {"message": "Unknown Member"})
        if p[0] == "channels":
            c = self.chan(p[1])
            if c is None:
                return 404, {"message": "Unknown Channel"}
            rest = p[2:]
            if not rest:
                if m == "PATCH":
                    if "type" in body and {c["type"], body["type"]} != {0, 5}:
                        return 400, {"message": "Invalid channel type change"}
                    for t in body.get("available_tags") or []:
                        t.setdefault("id", self.nid())
                    c.update(body)
                    return 200, c
                if m == "DELETE":
                    w["channels"].remove(c)
                    return 200, c
            if rest == ["messages"] and m == "POST":
                msg = {"id": self.nid(), "content": body["content"], "timestamp": "2026-11-01T00:00:00",
                       "author": {"id": BOT, "username": "Leyline setup"}, "attachments": [], "sent": body}
                w["messages"].setdefault(c["id"], []).insert(0, msg)
                return 200, msg
            if rest[0] == "pins" and m == "PUT":
                w.setdefault("pins", []).append((c["id"], rest[1]))
                return 204, None
            if rest == ["messages"] and m == "GET":
                msgs = w["messages"].get(c["id"], [])
                if "before" in q:
                    msgs = [x for x in msgs if int(x["id"]) < int(q["before"])]
                return 200, msgs[:int(q.get("limit", 50))]
            if rest[0] == "permissions":
                ow = [o for o in c["permission_overwrites"] if o["id"] != rest[1]]
                if m == "PUT":
                    ow.append({"id": rest[1], "type": body["type"], "allow": body["allow"], "deny": body["deny"]})
                c["permission_overwrites"] = ow
                return 204, None
        return 404, {"message": f"fake has no {m} {path}"}


def serve(fake):
    class H(BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def go(self):
            u = urllib.parse.urlparse(self.path)
            path = u.path[len("/api"):]
            n = int(self.headers.get("Content-Length") or 0)
            body = json.loads(self.rfile.read(n)) if n else None
            q = dict(urllib.parse.parse_qsl(u.query))
            st, out = fake.handle(self.command, path, q, body, self.headers)
            raw = json.dumps(out).encode() if out is not None else b""
            self.send_response(st)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)
        do_GET = do_POST = do_PATCH = do_PUT = do_DELETE = go

    srv = ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


class Base(unittest.TestCase):
    community = False
    reset = False           # start from the 30 Oct reset's server instead of an untouched one

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="leyline-discord-test-"))
        self.backups = self.tmp / "Desktop"
        self.backups.mkdir()
        self.fake = Fake((reset_world if self.reset else base_world)(self.community), self.backups)
        self.srv = serve(self.fake)
        self.token_file = self.tmp / "token.txt"
        self.token_file.write_text(TOKEN + "\n")
        self.outputs = []

    def tearDown(self):
        self.srv.shutdown()
        self.srv.server_close()

    def run_script(self, *args, stdin="", token_file=None):
        env = {k: v for k, v in os.environ.items() if "proxy" not in k.lower()}
        env.update(LEYLINE_DISCORD_API=f"http://127.0.0.1:{self.srv.server_address[1]}/api",
                   LEYLINE_BACKUP_ROOT=str(self.backups),
                   LEYLINE_DISCORD_BACKOFF="0.01", NO_PROXY="*", PYTHONIOENCODING="utf-8")
        p = subprocess.run([sys.executable, str(SCRIPT), "--token-file", str(token_file or self.token_file), *args],
                           input=stdin, capture_output=True, text=True, env=env, timeout=120, encoding="utf-8")
        self.outputs.append(p.stdout + p.stderr)
        return p

    def apply(self, *args, stdin="YES\n"):
        p = self.run_script("--apply", *args, stdin=stdin)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        return p

    def ch(self, name_slug, types=(0, 2, 4, 5, 13, 15)):
        hits = [c for c in self.fake.w["channels"] if dl.slug(c["name"]) == name_slug and c["type"] in types]
        self.assertEqual(len(hits), 1, f"{name_slug}: {hits}")
        return hits[0]

    def role(self, name):
        return next(r for r in self.fake.w["roles"] if r["name"].lower() == name.lower())

    def backup_dirs(self):
        return sorted(self.backups.glob("Leyline-discord-backup-*"))


def plan_names():
    return [s["name"] for _, chans in dl.LAYOUT for s in chans]


class TestLayout(unittest.TestCase):
    """The script's LAYOUT is DISCORD-PLAN.md's, and the same as the website bot's discord_template_v2.json."""
    TYPES = {"text": dl.TEXT, "voice": dl.VOICE, "stage": dl.STAGE, "forum": dl.FORUM}

    def setUp(self):
        self.t = json.loads(TEMPLATE.read_text(encoding="utf-8"))
        self.rname = {r["key"]: r["name"] for r in self.t["roles"]}

    def test_categories_and_channels_match_the_template(self):
        self.assertEqual([c for c, _ in dl.LAYOUT], [c["name"] for c in self.t["categories"]])
        for (cat, chans), tcat in zip(dl.LAYOUT, self.t["categories"]):
            self.assertEqual([s["full"] for s in chans], [c["name"] for c in tcat["channels"]], cat)
            for s, c in zip(chans, tcat["channels"]):
                n = c["name"]
                self.assertEqual(s["full"], f"{dl.EMOJI[s['name']]}︱{s['name']}", n)
                self.assertEqual(s["type"], self.TYPES[c.get("type", "text")], n)
                self.assertEqual(s["topic"], c.get("topic"), n)
                self.assertEqual(s["slow"], c.get("slowmode", 0), n)
                self.assertEqual(s["ro"], bool(c.get("read_only")), n)
                self.assertEqual(s["posters"], [self.rname[k] for k in c.get("posters") or []], n)
                self.assertEqual(s["private"], [self.rname[k] for k in c.get("private") or []], n)
                self.assertEqual(s["optional"], bool(c.get("optional")), n)

    def test_roles_match_the_template(self):
        self.assertEqual([(n, f"#{col:06x}", h) for n, col, h, *_ in dl.ROLES],
                         [(r["name"], r["color"].lower(), bool(r.get("hoist"))) for r in self.t["roles"]])
        for n, _, _, _, perms in dl.ROLES:
            self.assertFalse(perms & 8, f"{n} must not get Administrator")

    def test_emoji_names_and_aliases(self):
        names = plan_names()
        self.assertEqual(dl.GLYPH_SEP, "︱")
        self.assertEqual(set(dl.EMOJI), set(names), "one emoji per channel")
        for n in names:
            self.assertEqual(dl.slug(n), n.lower().replace(" ", "-"), "base names are plain")
            self.assertEqual(dl.slug(dl.EMOJI[n] + dl.GLYPH_SEP + n), dl.slug(n))
            self.assertNotIn("\ufe0f", dl.EMOJI[n], "no variation selectors: the exact name must match")
        # the 30 Oct reset's emoji stay on the channels they became
        for old, new in (("👋", "welcome"), ("📣", "announcements"), ("💬", "hangout"), ("🛟", "launcher-help"),
                         ("🤝", "looking-for-group"), ("🎧", "Dalaran lounge"), ("🧩", "mods-and-addons"),
                         ("🏰", "hosting-help"), ("📸", "den-showcase"), ("🐞", "bug-reports"), ("💡", "feature-ideas")):
            self.assertEqual(dl.EMOJI[new], old, new)
        self.assertEqual(len(set(map(dl.slug, names))), len(names))
        for target, aliases in dl.ALIASES.items():
            self.assertIn(target, map(dl.slug, names))
            self.assertEqual(aliases[0], target)
        # the DISCORD-PLAN.md old -> new table
        for old, new in (("general", "hangout"), ("wrath-unleashed", "hangout"), ("help", "launcher-help"),
                         ("mods-and-workshop", "mods-and-addons"), ("mods", "mods-and-addons"),
                         ("host-a-beacon", "hosting-help"), ("showcase", "den-showcase"),
                         ("builders-realm", "den-showcase"), ("building-help", "den-help"),
                         ("hangout", "dalaran-lounge")):
            self.assertIn(old, dl.ALIASES[new], old)
        for old, new in (("start", "start-here"), ("play", "wrath-unleashed"), ("create", "your-den"),
                         ("build", "your-den"), ("make", "mods-&-workshop"), ("host", "host-a-server"),
                         ("feedback", "feedback"), ("leyline", "feedback")):
            self.assertIn(old, dl.CATEGORY_ALIASES[new], old)
        self.assertIn("helper", dl.ROLE_ALIASES["Guide"])


class TestPreview(Base):
    def test_preview_makes_no_writes_and_shows_the_plan(self):
        before = copy.deepcopy(self.fake.w)
        p = self.run_script()
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertEqual(self.fake.writes(), [])
        self.assertEqual(self.fake.w, before)
        self.assertEqual(self.backup_dirs(), [])
        out = p.stdout
        for never in ("'Leyline'", "icon", "banner", "invite background"):
            self.assertNotIn(never, out)
        for s in ("PREVIEW", "New channels:", "category Start here", "🤝︱looking-for-group in Wrath Unleashed",
                  "Reused channels", "general -> 💬︱hangout", "General -> 🎧︱Dalaran lounge",
                  "rules -> 📜︱rules (keeps its history",
                  "ov-realm-help -> ARCHIVE", "wrath-dev -> ARCHIVE", "AFK -> ARCHIVE",
                  "create Guide", "rename moderator -> Moderator", "--delete-old-roles", "Realm-Ally",
                  "feature-ideas (forum channel): needs a Community server", "Do these by hand", "Enable Community",
                  "📣︱announcements: Moderator can post", "☕︱host-lounge: only Realm Host, Core Team, Moderator",
                  "Nothing changed", "It posts nothing"):
            self.assertIn(s, out)
        self.assertIn("nothing (archived channels stay; --delete-archived would delete them)", out)
        self.assertIn("for the server 'OpenVanilla Wrath'", out)

    def test_preview_with_delete_flags_lists_deletions_but_still_writes_nothing(self):
        p = self.run_script("--delete-archived", "--delete-old-roles")
        self.assertEqual(self.fake.writes(), [])
        self.assertIn("DELETED:", p.stdout)
        self.assertIn("role Realm-Ally", p.stdout)
        self.assertIn("channel wrath-dev (from ARCHIVE)", p.stdout)
        for never in ("role Admin", "role Founder", "role Server Booster", "role Leyline setup", "role @everyone"):
            self.assertNotIn(never, p.stdout)

    def test_preview_lists_the_same_changes_the_apply_makes(self):
        prev = self.run_script()
        n_prev = int(prev.stdout.split(" change(s).")[0].rsplit("\n", 1)[-1])
        self.apply()
        done = [l for l in self.outputs[-1].splitlines() if l.strip().startswith("done:")]
        self.assertEqual(len(done), n_prev)


class TestApply(Base):
    def test_backup_is_written_before_any_write(self):
        self.apply()
        writes = self.fake.writes()
        self.assertTrue(writes)
        self.assertTrue(all(w[4] for w in writes), "a write happened before the backup existed")
        [b] = self.backup_dirs()
        self.assertIn(str(b), self.outputs[-1])
        g = json.loads((b / "guild.json").read_text(encoding="utf-8"))
        self.assertEqual(g["name"], "OpenVanilla Wrath")       # the state before the change
        chans = json.loads((b / "channels.json").read_text(encoding="utf-8"))
        self.assertTrue(any(c["id"] == "23" and c["permission_overwrites"] for c in chans))
        roles = json.loads((b / "roles.json").read_text(encoding="utf-8"))
        self.assertIn("Realm-Ally", [r["name"] for r in roles])
        gen = json.loads((b / "messages" / "general-20.json").read_text(encoding="utf-8"))
        self.assertEqual(len(gen["messages"]), 250)
        m0 = gen["messages"][0]
        self.assertEqual(set(m0), {"id", "author", "author_id", "time", "edited", "content", "pinned", "attachments"})
        self.assertEqual(m0["author"], "Ann")
        self.assertTrue(any(m["attachments"] for m in gen["messages"]))
        for f in ("what-changed.txt", "first-posts.txt", "settings-for-the-bots.txt"):
            self.assertTrue((b / f).exists(), f)

    def test_backup_message_limit(self):
        self.apply("--backup-messages", "120")
        b = self.backup_dirs()[-1]
        gen = json.loads((b / "messages" / "general-20.json").read_text(encoding="utf-8"))
        self.assertEqual(len(gen["messages"]), 120)
        self.apply("--backup-messages", "0")
        self.assertEqual(list((self.backup_dirs()[-1] / "messages").iterdir()), [])

    def test_layout_reuse_and_theme(self):
        self.apply()
        w = self.fake.w
        self.assertEqual(w["guild"]["name"], "OpenVanilla Wrath")       # never renamed (owner, 1 Nov)
        self.assertIsNone(w["guild"]["icon"])                           # nor its icon or banner
        self.assertIsNone(w["guild"]["banner"])
        self.assertIsNone(w["guild"]["afk_channel_id"])
        cats = {c["name"]: c for c in w["channels"] if c["type"] == 4}
        for cat, _ in dl.LAYOUT:
            self.assertIn(cat, cats)
        self.assertIn("ARCHIVE", cats)
        # reused, history kept: same ids
        hang, rules, ann, voice = self.ch("hangout", (0,)), self.ch("rules"), self.ch("announcements"), \
            self.ch("dalaran-lounge", (2,))
        self.assertEqual((hang["id"], rules["id"], ann["id"], voice["id"]), ("20", "24", "21", "30"))
        self.assertEqual(hang["parent_id"], cats["Wrath Unleashed"]["id"])
        self.assertEqual(hang["topic"], dl.LAYOUT[1][1][0]["topic"])
        self.assertEqual(voice["name"], "🎧︱Dalaran lounge")
        self.assertEqual(w["guild"]["system_channel_id"], self.ch("welcome")["id"])
        # every plan channel there once, plain name, right category, topic, slowmode; no forum/stage (no Community)
        for cat, chans in dl.LAYOUT:
            for s in chans:
                if s["type"] in (dl.FORUM, dl.STAGE):
                    self.assertFalse([c for c in w["channels"] if dl.slug(c["name"]) == dl.slug(s["name"]) and c["type"] != 4])
                    continue
                c = self.ch(dl.slug(s["name"]))
                self.assertEqual(c["name"], s["full"])
                self.assertEqual(c["type"], s["type"], s["name"])
                self.assertEqual(c["parent_id"], cats[cat]["id"], s["name"])
                if s["type"] == dl.TEXT:
                    self.assertEqual(c["topic"], s["topic"], s["name"])
                    self.assertEqual(c["rate_limit_per_user"] or 0, s["slow"], s["name"])
        self.assertEqual(self.ch("den-files")["rate_limit_per_user"], 60)
        self.assertEqual(self.ch("list-your-realm")["rate_limit_per_user"], 600)
        # read-only: @everyone can read, not post; the named roles can post
        mod, core, guide = self.role("Moderator"), self.role("Core Team"), self.role("Guide")
        for name, posters in (("welcome", []), ("announcements", [core, mod]), ("events", [core, mod, guide]),
                              ("live-realms", [])):
            ow = dl.ow_map(self.ch(name))
            self.assertTrue(int(ow[GID]["deny"]) & dl.P_SEND, name)
            self.assertFalse(int(ow[GID]["deny"]) & dl.P_VIEW, name)
            for r in posters:
                self.assertTrue(int(ow[r["id"]]["allow"]) & dl.P_SEND, (name, r["name"]))
        self.assertNotIn(GID, dl.ow_map(hang))
        # private: hidden from @everyone, open to its roles only
        host = self.role("Realm Host")
        ow = dl.ow_map(self.ch("host-lounge"))
        self.assertTrue(int(ow[GID]["deny"]) & dl.P_VIEW)
        self.assertTrue(int(ow[host["id"]]["allow"]) & dl.P_VIEW)
        self.assertNotIn(guide["id"], ow)
        # order: the plan's categories first, then ARCHIVE
        order = [c["name"] for c in sorted((c for c in w["channels"] if c["type"] == 4), key=dl.okey)]
        self.assertEqual(order[:len(dl.LAYOUT) + 1], [c for c, _ in dl.LAYOUT] + ["ARCHIVE"])
        wu = [c["name"] for c in sorted((c for c in w["channels"] if c["parent_id"] == cats["Wrath Unleashed"]["id"]),
                                        key=dl.okey)]
        self.assertEqual(wu, [s["full"] for s in dl.LAYOUT[1][1]])
        # roles: all eight, in the plan's order, the existing "moderator" reused
        for name, colour, hoist, *_ in dl.ROLES:
            r = self.role(name)
            self.assertEqual((r["color"], r["hoist"]), (colour, hoist), name)
        pos = [self.role(n)["position"] for n, *_ in dl.ROLES]
        self.assertEqual(pos, sorted(pos, reverse=True))
        self.assertEqual(self.role("Moderator")["id"], "54")
        self.assertEqual(sum(r["name"].lower() == "moderator" for r in w["roles"]), 1)

    def test_archive_moves_and_hides(self):
        self.apply()
        w = self.fake.w
        arch = self.ch("archive", (4,))
        mod = self.role("Moderator")
        aow = dl.ow_map(arch)
        self.assertTrue(int(aow[GID]["deny"]) & dl.P_VIEW)
        self.assertTrue(int(aow[mod["id"]]["allow"]) & dl.P_VIEW)
        self.assertTrue(int(aow[mod["id"]]["deny"]) & dl.P_SEND)
        for cid in ("22", "23", "31"):
            c = self.fake.chan(cid)
            self.assertIsNotNone(c, cid)                        # nothing deleted
            self.assertEqual(c["parent_id"], arch["id"], cid)
            self.assertEqual(dl.ow_norm(c["permission_overwrites"]), dl.ow_norm(arch["permission_overwrites"]))
            self.assertNotIn("53", dl.ow_map(c))                # an old role's view grant is gone (synced)
        for cid in ("10", "11"):                                # empty old categories hidden, not deleted
            self.assertTrue(int(dl.ow_map(self.fake.chan(cid))[GID]["deny"]) & dl.P_VIEW)
        self.assertEqual(len(w["messages"]["20"]), 250)

    def test_second_run_changes_nothing(self):
        self.apply()
        n = len(self.fake.writes())
        snap = copy.deepcopy(self.fake.w)
        p = self.run_script("--apply", stdin="")                 # nothing to confirm when nothing changes
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertEqual(len(self.fake.writes()), n, [x[:3] for x in self.fake.writes()[n:]])
        self.assertEqual(self.fake.w, snap)
        self.assertIn("Nothing to change", p.stdout)
        prev = self.run_script()
        self.assertIn("Nothing to change", prev.stdout)

    def test_no_yes_no_change(self):
        p = self.run_script("--apply", stdin="yes please\n")
        self.assertEqual(p.returncode, 1)
        self.assertEqual(self.fake.writes(), [])
        self.assertEqual(self.backup_dirs(), [])

    def test_audit_reason_and_user_agent(self):
        self.apply()
        for m, path, body, headers, _ in self.fake.writes():
            self.assertEqual(urllib.parse.unquote(headers.get("X-Audit-Log-Reason", "")), "Leyline rebrand", path)
            self.assertTrue(headers.get("User-Agent", "").startswith("DiscordBot ("))

    def test_settings_for_the_bots(self):
        p = self.apply()
        wc, bugs = self.ch("world-chat")["id"], self.ch("bug-reports")["id"]
        for s in (f"DISCORD_CHAT_CHANNEL_ID={wc}", f"DISCORD_BUG_CHANNEL_ID={bugs}", f"DISCORD_GUILD_ID={GID}",
                  "WU_DISCORD_TEMPLATE=/app/discord_template_v2.json", "WU_FORUM_DISCORD_CHANNELS=news=",
                  "WU_DISCORD_BUG_INTAKE=/srv/wrath/bugs/intake/discord.jsonl"):
            self.assertIn(s, p.stdout)


class TestFromTheResetPreview(Base):
    reset = True
    community = True

    def test_preview(self):
        p = self.run_script("--post-first-posts")
        self.assertIn("◆︱general -> 💬︱hangout (keeps its history", p.stdout)
        self.assertIn("✦︱welcome -> 👋︱welcome", p.stdout)
        self.assertIn("🤝︱looking-for-group (keeps its name and history; topic)", p.stdout)
        self.assertEqual(self.fake.writes(), [])


class TestFromTheReset(Base):
    """The server as the 30 Oct reset left it (glyph names, START/PLAY/CREATE/FEEDBACK), on a Community server."""
    reset = True
    community = True

    def test_glyph_channels_are_renamed_and_moved_not_duplicated(self):
        self.apply()
        w = self.fake.w
        cats = {c["name"]: c["id"] for c in w["channels"] if c["type"] == 4}
        for cid, name, cat in (("20", "👋︱welcome", "Start here"), ("21", "📣︱announcements", "Start here"),
                               ("32", "📜︱rules", "Start here"), ("22", "💬︱hangout", "Wrath Unleashed"),
                               ("24", "🤝︱looking-for-group", "Wrath Unleashed"), ("23", "🛟︱launcher-help", "Feedback"),
                               ("25", "🎧︱Dalaran lounge", "Voice"), ("26", "🧩︱mods-and-addons", "Mods & Workshop"),
                               ("27", "🏰︱hosting-help", "Host a server"), ("28", "📸︱den-showcase", "Your Den"),
                               ("29", "🐞︱bug-reports", "Feedback")):
            c = self.fake.chan(cid)
            self.assertEqual((c["name"], c["parent_id"]), (name, cats[cat]), cid)
            self.assertEqual(sum(dl.slug(x["name"]) == dl.slug(name) for x in w["channels"]), 1, name)
        # a channel already named right keeps its name (no rename sent)
        self.assertFalse([x for x in self.fake.writes() if x[1] == "/channels/24" and "name" in (x[2] or {})])
        self.assertEqual(self.fake.chan("21")["type"], 0, "announcements is a plain text channel, as Set up wants")
        # the old categories became the plan's (same ids)
        self.assertEqual((cats["Start here"], cats["Wrath Unleashed"], cats["Your Den"], cats["Feedback"],
                          cats["ARCHIVE"]), ("10", "11", "12", "13", "14"))
        # ideas can't become a forum channel: it goes to the archive with its history; feature-ideas is new
        self.assertEqual(self.fake.chan("30")["parent_id"], "14")
        self.assertEqual(len(w["messages"]["30"]), 1)
        self.assertEqual(self.ch("feature-ideas")["type"], dl.FORUM)
        self.assertEqual(self.ch("showcase-stage")["type"], dl.STAGE)
        # Helper became Guide (same role, so whoever had it keeps it); Creator and Member left alone
        self.assertEqual(self.role("Guide")["id"], "61")
        self.assertFalse([r for r in w["roles"] if r["name"] == "Helper"])
        self.assertIn("62", {r["id"] for r in w["roles"]})
        # the Community rules channel is #rules
        self.assertEqual(w["guild"]["rules_channel_id"], "32")
        # forum tags, and the website bot may post in the read-only and private channels
        forum = self.ch("forum", (15,))
        self.assertEqual([t["name"] for t in forum["available_tags"]], [t for _, t in dl.FORUM_TAGS])
        for name in ("live-realms", "announcements", "roadmap"):
            self.assertTrue(int(dl.ow_map(self.ch(name))["64"]["allow"]) & dl.P_SEND, name)
        self.assertNotIn("64", dl.ow_map(self.ch("hangout")))
        tag = forum["available_tags"][0]["id"]
        self.assertIn(f"WU_FORUM_DISCORD_CHANNELS=realms={forum['id']}/{tag},", self.outputs[-1])
        # again: nothing to do
        n = len(self.fake.writes())
        self.apply(stdin="")
        self.assertEqual(len(self.fake.writes()), n, [x[:3] for x in self.fake.writes()[n:]])

    def test_the_website_bot_finds_every_channel_afterwards(self):
        """Set up the Discord (stream/discord_growth.py) looks channels up by (exact name, type): all found."""
        self.apply()
        t = json.loads(TEMPLATE.read_text(encoding="utf-8"))
        types = {"text": 0, "voice": 2, "stage": 13, "forum": 15}
        seen = {(c["name"].lower(), c["type"]) for c in self.fake.w["channels"]}
        roles = {r["name"].lower() for r in self.fake.w["roles"]}
        for cat in t["categories"]:
            self.assertIn((cat["name"].lower(), 4), seen, cat["name"])
            for c in cat["channels"]:
                self.assertIn((c["name"].lower(), types[c.get("type", "text")]), seen, c["name"])
        for r in t["roles"]:
            self.assertIn(r["name"].lower(), roles)


class TestRetries(Base):
    def test_429_and_5xx_are_retried(self):
        self.fake.fail_once[("PATCH", f"/guilds/{GID}")] = 429
        self.fake.fail_once[("POST", f"/guilds/{GID}/roles")] = 429
        self.fake.fail_once[("GET", f"/guilds/{GID}/channels")] = 502
        self.apply()
        self.assertEqual(self.fake.served_429, 2)
        self.assertEqual(self.fake.served_5xx, 1)
        self.assertIsNone(self.fake.w["guild"]["afk_channel_id"])
        self.assertEqual(sum(r["name"] == "Guide" for r in self.fake.w["roles"]), 1)

    def test_retry_after_is_honoured(self):
        slept = []
        api = dl.Discord(TOKEN, allow_writes=True, sleep=slept.append)
        self.fake.fail_once[("GET", "/users/@me")] = 429
        old = dl.API_BASE
        dl.API_BASE = f"http://127.0.0.1:{self.srv.server_address[1]}/api"
        try:
            env_proxy = {k: os.environ.pop(k) for k in list(os.environ) if "proxy" in k.lower()}
            try:
                self.assertEqual(api.request("GET", "/users/@me")["id"], BOT)
            finally:
                os.environ.update(env_proxy)
        finally:
            dl.API_BASE = old
        self.assertEqual(len(slept), 1)
        self.assertAlmostEqual(slept[0], 0.06, places=3)

    def test_preview_client_refuses_writes(self):
        with self.assertRaises(RuntimeError):
            dl.Discord(TOKEN).request("PATCH", "/guilds/1", {"name": "x"})


class TestNeverTouch(Base):
    PROTECTED = {GID, "50", "51", "52", "55"}    # @everyone, bot's managed role, above bot, owner's role, booster

    def check_untouched(self):
        for m, path, body, *_ in self.fake.writes():
            parts = path.split("/")
            if len(parts) >= 5 and parts[3] == "roles":
                self.assertNotIn(parts[4], self.PROTECTED, f"{m} {path}")
            if path == f"/guilds/{GID}/roles" and m == "PATCH":
                self.assertFalse({b["id"] for b in body} & self.PROTECTED, body)
        ids = {r["id"] for r in self.fake.w["roles"]}
        self.assertTrue(self.PROTECTED <= ids)
        everyone = self.role("@everyone")
        self.assertEqual((everyone["permissions"], everyone["position"]), ("1024", 0))
        self.assertEqual(self.role("Admin")["color"], 0xff0000)
        self.assertEqual(self.role("Founder")["color"], 0x00ff00)

    def test_default_run(self):
        self.apply()
        self.check_untouched()
        self.assertIn("53", {r["id"] for r in self.fake.w["roles"]})      # old roles stay without the flag

    def test_delete_old_roles(self):
        p = self.apply("--delete-old-roles", stdin="YES\nDELETE ROLES\n")
        self.check_untouched()
        self.assertNotIn("53", {r["id"] for r in self.fake.w["roles"]})
        self.assertIn("keep Admin (above the bot's role)", p.stdout)
        self.assertIn("keep Founder (you have this role)", p.stdout)
        deleted = [x[1] for x in self.fake.writes() if x[0] == "DELETE" and "/roles/" in x[1]]
        self.assertEqual(deleted, [f"/guilds/{GID}/roles/53"])

    def test_owner_holds_a_target_role(self):
        self.fake.w["members"][OWNER]["roles"].append("54")              # owner is a "moderator" too
        p = self.apply()
        self.assertEqual(self.role("moderator")["color"], 0x111111)
        self.assertIn("keep moderator as it is (you have this role)", p.stdout)
        self.check_untouched()


class TestDeleteFlags(Base):
    def test_delete_archived_needs_the_server_name(self):
        p = self.run_script("--apply", "--delete-archived", stdin="YES\nLeyline\n")
        self.assertEqual(p.returncode, 1)
        self.assertEqual(self.fake.writes(), [])
        p = self.run_script("--apply", "--delete-archived", stdin="YES\n\n")
        self.assertEqual(p.returncode, 1)
        self.assertEqual(self.fake.writes(), [])

    def test_delete_old_roles_needs_its_words(self):
        p = self.run_script("--apply", "--delete-old-roles", stdin="YES\ndelete\n")
        self.assertEqual(p.returncode, 1)
        self.assertEqual(self.fake.writes(), [])

    def test_wipe_history_fresh_channels(self):
        self.apply("--wipe-history", stdin="YES\nWIPE\n")
        self.assertIsNone(self.fake.chan("20"), "the old general (and its messages) is gone")
        self.assertEqual(len([c for c in self.fake.w["channels"] if c["name"] == "💬︱hangout"]), 1)
        welcome = [c for c in self.fake.w["channels"] if c["name"] == "👋︱welcome"][0]
        self.assertEqual(self.fake.w["guild"].get("system_channel_id"), welcome["id"])
        self.assertIsNotNone(self.fake.chan("30"), "voice channels are left as they are")
        self.assertTrue(self.backup_dirs())

    def test_wipe_history_needs_wipe(self):
        n = len(self.fake.writes())
        p = self.run_script("--apply", "--wipe-history", stdin="YES\nno\n")
        self.assertEqual(p.returncode, 1)
        self.assertEqual(len(self.fake.writes()), n)

    def test_delete_archived_after_backup(self):
        self.apply("--delete-archived", stdin="YES\nOpenVanilla Wrath\n")
        b = self.backup_dirs()[-1]
        self.assertTrue((b / "messages" / "ov-realm-help-22.json").exists())
        for cid in ("22", "23", "31", "10", "11"):
            self.assertIsNone(self.fake.chan(cid), cid)
        self.assertFalse([c for c in self.fake.w["channels"] if c["name"] == "ARCHIVE"])
        self.assertEqual(self.fake.chan("20")["name"], "💬︱hangout")     # reused channels never deleted
        dels = [x for x in self.fake.writes() if x[0] == "DELETE"]
        self.assertTrue(dels and all(x[4] for x in dels))
        n = len(self.fake.writes())
        self.apply("--delete-archived", stdin="OpenVanilla Wrath\n")      # the name is never changed
        self.assertEqual(len(self.fake.writes()), n)


class TestCommunity(Base):
    community = True

    def test_community_server(self):
        self.apply()
        g = self.fake.w["guild"]
        self.assertEqual(g["rules_channel_id"], self.ch("rules")["id"])
        self.assertIsNone(g["banner"])                                       # BANNER feature, but never touched
        self.assertIsNone(g["icon"])
        self.assertIsNone(g["splash"])
        self.assertEqual(g["name"], "OpenVanilla Wrath")
        for m, path, body, *_ in self.fake.writes():
            if path == f"/guilds/{GID}":
                self.assertFalse({"name", "icon", "banner", "splash"} & set(body), body)
        mo = self.fake.chan("25")
        self.assertEqual(mo["parent_id"], "11")                             # Community updates channel left alone
        self.assertFalse(int(dl.ow_map(self.fake.chan("11")).get(GID, {}).get("deny", 0)) & dl.P_VIEW)
        self.assertEqual(self.ch("forum", (15,))["type"], dl.FORUM)
        self.assertEqual(self.ch("showcase-stage")["type"], dl.STAGE)
        self.assertIn("The Leyline website bot isn't in the server", self.outputs[-1])
        n = len(self.fake.writes())
        self.apply()
        self.assertEqual(len(self.fake.writes()), n)


class TestFirstPosts(Base):
    community = True

    def posted(self):
        return [(x[1].split("/")[2], x[2]) for x in self.fake.writes()
                if x[0] == "POST" and x[1].endswith("/messages")]

    def test_no_flag_posts_nothing_and_prints_them(self):
        p = self.apply()
        self.assertEqual(self.posted(), [])
        self.assertIn("----- #welcome", p.stdout)
        self.assertIn("----- #bug-reports", p.stdout)
        self.assertIn("How to report a bug", p.stdout)

    def test_flag_without_post_posts_nothing(self):
        p = self.apply("--post-first-posts", stdin="YES\nyes\n")
        self.assertEqual(self.posted(), [])
        self.assertIn("won't post", p.stdout)

    def test_flag_and_post_posts_only_finished_drafts(self):
        prev = self.run_script("--post-first-posts")
        self.assertIn("#welcome: left for you (has TODO(owner)", prev.stdout)
        self.assertIn("#changelog: left for you (Start here", prev.stdout)
        self.assertEqual(self.fake.writes(), [])
        p = self.apply("--post-first-posts", stdin="YES\nPOST\n")
        got = self.posted()
        self.assertTrue(got, p.stdout)
        start = {self.ch(s["name"])["id"] for s in dl.LAYOUT[0][1]}
        forums = {self.ch(n, (15,))["id"] for n in ("forum", "feature-ideas")}
        names = {c["id"]: c["name"] for c in self.fake.w["channels"]}
        drafts = dict(dl.read_posts(dl.POSTS_FILE))
        for cid, body in got:
            self.assertNotIn(cid, start | forums)
            self.assertNotIn("TODO(owner)", body["content"])
            self.assertEqual(body["allowed_mentions"], {"parse": []})
            self.assertTrue("TODO(owner)" not in drafts[names[cid].split("︱")[-1]])
        self.assertIn(self.ch("bug-reports")["id"], {c for c, _ in got})
        self.assertNotIn(self.ch("hangout")["id"], {c for c, _ in got})          # it has a TODO(owner)
        self.assertEqual(len(self.fake.w["pins"]), len(got))
        hosting = next(b for c, b in got if c == self.ch("hosting-help")["id"])
        self.assertIn(f"<#{self.ch('host-templates')['id']}>", hosting["content"])     # #names become links
        self.assertIn("----- #welcome", p.stdout)                                 # the rest printed for the owner
        # again: already posted, nothing new
        n = len(got)
        self.apply("--post-first-posts", stdin="POST\n")
        self.assertEqual(len(self.posted()), n)


class TestGuildChoice(Base):
    def test_one_guild_is_named(self):
        p = self.run_script()
        self.assertIn("The bot is in one server: OpenVanilla Wrath (id 1)", p.stdout)

    def test_many_guilds_asks(self):
        self.fake.w["guilds"].append({"id": "2", "name": "Other"})
        p = self.run_script(stdin="1\n")
        self.assertIn("more than one server", p.stdout)
        self.assertEqual(p.returncode, 0, p.stdout)
        p = self.run_script(stdin="3\n")
        self.assertNotEqual(p.returncode, 0)
        p = self.run_script("--guild", "1")
        self.assertEqual(p.returncode, 0)


class TestSecretsAndWords(Base):
    def test_token_never_in_output_or_files(self):
        self.apply("--delete-old-roles", stdin="YES\nDELETE ROLES\n")
        self.apply()
        self.run_script()
        bad = self.tmp / "bad.txt"
        bad.write_text("Bot not-the-right-one\n")
        p = self.run_script(token_file=bad)
        self.assertNotEqual(p.returncode, 0)
        self.assertIn("didn't accept that token", p.stdout + p.stderr)
        self.assertNotIn("not-the-right-one", p.stdout + p.stderr)
        for out in self.outputs:
            self.assertNotIn(TOKEN, out)
            self.assertNotIn(TOKEN[:20], out)
        for f in self.tmp.rglob("*"):
            if f.is_file() and f != self.token_file:
                self.assertNotIn(TOKEN, f.read_text(encoding="utf-8", errors="replace"), str(f))
        for m, path, body, headers, _ in self.fake.log:
            self.assertNotIn(TOKEN, path + json.dumps(body))

    def test_no_banned_words(self):
        texts = [dl.ARCHIVE]
        for cat, chans in dl.LAYOUT:
            texts.append(cat)
            for s in chans:
                texts += [s["full"], s["topic"] or ""] + s["tags"]
        texts += [n for n, *_ in dl.ROLES]
        for t in texts:
            self.assertIsNone(dl.BANNED.search(t), t)
            self.assertIsNone(dl.MONEY.search(t), t)
        self.apply()
        for m, path, body, *_ in self.fake.writes():
            for b in (body if isinstance(body, list) else [body or {}]):
                for k in ("name", "topic"):
                    if isinstance(b.get(k), str):
                        self.assertIsNone(dl.BANNED.search(b[k]), b[k])
        for bad in ("World of Warcraft", "WoW", "warcraft", "Lich King", "Blizzard", "3.3.5a", "1.12"):
            self.assertIsNotNone(dl.BANNED.search(bad), bad)
        for bad in ("patreon.com/x", "ko-fi", "Tip me", "donate"):
            self.assertIsNotNone(dl.MONEY.search(bad), bad)

    def test_posting_refuses_rule_breakers(self):
        go, leave = dl.plan_posts([("hangout", "come play WoW"), ("hangout", "tips: ko-fi.com/x"),
                                   ("nowhere", "hi"), ("bug-reports", "fine")])
        self.assertEqual([n for n, *_ in go], ["bug-reports"])
        self.assertEqual(len(leave), 3)


if __name__ == "__main__":
    unittest.main(verbosity=2)
