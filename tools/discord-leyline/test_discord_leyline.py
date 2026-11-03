"""Leyline Discord (discord_leyline.py) against a fake Discord REST API on localhost.

Covers: the launch layout (13 channels) agrees with stream/discord_template_launch.json (so the website bot's Set up the
Discord finds every channel afterwards); today's server (the owner's 3 Nov screenshot) needs only the few changes the
preview lists; the preview never writes; the backup lands before the first write; channels from earlier passes are
renamed and moved with their history, not made twice; old channels go to a hidden read-only ARCHIVE, or are deleted
only as the owner chose (empty ones, or all after a typed confirmation), never a channel with messages without that
choice; read-only and private permissions; a second run changes nothing; pinned posts are posted only with the owner's
OK, never with TODO(owner), and edited in place, not posted twice; 429 and 5xx retries; the @everyone role, managed
roles, roles above the bot and the owner's roles are never touched; the delete flags need their typed confirmations;
the token never reaches the output or any file; no banned words.

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
TEMPLATE = HERE.parent.parent / "stream" / "discord_template_launch.json"
POSTS = HERE.parent.parent / "docs" / "discord-launch-posts.md"
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


def today_world(community=False):
    """The server as the owner's 3 Nov screenshot shows it: START (welcome, announcements), PLAY (general, help,
    looking-for-group, the Hangout voice room), CREATE (mods-and-workshop, host-a-beacon, showcase), FEEDBACK
    (bug-reports, ideas), 🔒 STAFF (mod-log, private); the reset's roles; the website bot; and an ARCHIVE from the reset
    with one old channel that has messages and one that is empty."""
    w = reset_world(community)

    def ch(i, name, t, parent=None, pos=0, ow=None, topic="old"):
        return {"id": i, "name": name, "type": t, "parent_id": parent, "position": pos, "guild_id": GID,
                "permission_overwrites": ow or [], "topic": topic, "rate_limit_per_user": 0}
    ro = [{"id": GID, "type": 0, "allow": "0", "deny": str(dl.P_NO_TALK)}]
    staff = [{"id": GID, "type": 0, "allow": "0", "deny": str(dl.P_VIEW)},
             {"id": "60", "type": 0, "allow": str(dl.P_VIEW), "deny": "0"}]
    arch = [{"id": GID, "type": 0, "allow": "0", "deny": str(dl.P_VIEW | dl.P_NO_TALK | dl.P_CONNECT)},
            {"id": "60", "type": 0, "allow": str(dl.P_VIEW | dl.P_HISTORY), "deny": str(dl.P_NO_TALK | dl.P_CONNECT)}]
    w["channels"] = [
        ch("10", "START", 4, pos=0), ch("11", "PLAY", 4, pos=1), ch("12", "CREATE", 4, pos=2),
        ch("13", "FEEDBACK", 4, pos=3), ch("15", "🔒 STAFF", 4, pos=4), ch("14", "ARCHIVE", 4, pos=5),
        ch("20", "👋︱welcome", 0, "10", 0, ro), ch("21", "📣︱announcements", 5 if community else 0, "10", 1, ro),
        ch("22", "💬︱general", 0, "11", 0), ch("23", "🛟︱help", 0, "11", 1), ch("24", "🤝︱looking-for-group", 0, "11", 2),
        ch("25", "🎧︱Hangout", 2, "11", 3),
        ch("26", "🧩︱mods-and-workshop", 0, "12", 0), ch("27", "🏰︱host-a-beacon", 0, "12", 1),
        ch("28", "📷︱showcase", 0, "12", 2),
        ch("29", "🐞︱bug-reports", 0, "13", 0), ch("30", "💡︱ideas", 0, "13", 1),
        ch("33", "mod-log", 0, "15", 0, staff),
        ch("31", "old-stuff", 0, "14", 0, arch), ch("32", "rules", 0, "14", 1, arch),
    ]
    w["channels"][5]["permission_overwrites"] = arch
    w["messages"] = {"22": w["messages"]["22"], "27": [{"id": "270001", "content": "hosting tonight?",
                                                        "author": {"id": "7"}, "timestamp": "2026-11-02T00:00:00",
                                                        "attachments": []}],
                     "31": [{"id": "310001", "content": "old news", "author": {"id": "7"},
                             "timestamp": "2026-10-01T00:00:00", "attachments": []}]}
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
            if rest == ["pins"] and m == "GET":
                pinned = {mid for cid, mid in w.get("pins", []) if cid == c["id"]}
                return 200, [x for x in w["messages"].get(c["id"], []) if x["id"] in pinned]
            if rest[0] == "messages" and len(rest) == 2 and m == "PATCH":
                msg = next((x for x in w["messages"].get(c["id"], []) if x["id"] == rest[1]), None)
                if msg is None:
                    return 404, {"message": "Unknown Message"}
                if msg["author"]["id"] != BOT:
                    return 403, {"message": "Cannot edit a message authored by another user"}
                msg.update(content=body["content"], sent=body)
                return 200, msg
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
    world = None            # a fixture other than an untouched server (reset_world, today_world)

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="leyline-discord-test-"))
        self.backups = self.tmp / "Desktop"
        self.backups.mkdir()
        self.fake = Fake((self.world or base_world)(self.community), self.backups)
        self.srv = serve(self.fake)
        self.token_file = self.tmp / "token.txt"
        self.token_file.write_text(TOKEN + "\n")
        self.outputs = []

    def tearDown(self):
        self.srv.shutdown()
        self.srv.server_close()

    def run_script(self, *args, stdin="", token_file=None, extra_env=None):
        env = {k: v for k, v in os.environ.items() if "proxy" not in k.lower() and not k.startswith("LEYLINE_")}
        env.update(extra_env or {})
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


def changes(out):
    import re
    m = re.search(r"^(\d+) change\(s\)\.$", out, re.M)
    return int(m.group(1)) if m else (0 if "Nothing to change" in out else None)


# The pages the posts and topics may link: they exist on the site (stream/docs.py PAGES and the portal's routes).
SITE_PATHS = {"/wrath", "/wrath/download", "/wrath/start", "/wrath/docs", "/wrath/mods", "/wrath/modes", "/wrath/realms",
              "/wrath/bugs", "/wrath/key", "/wrath/submit", "/wrath/leyline/forum"}


def site_paths():
    import importlib.util
    spec = importlib.util.spec_from_file_location("docs_pages", HERE.parent.parent / "stream" / "docs.py")
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return SITE_PATHS | {f"{m.BASE}/{k}" for k, _ in m.PAGES}


class TestLayout(unittest.TestCase):
    """The script's LAYOUT is DISCORD-PLAN.md's launch layout, and the same as discord_template_launch.json."""
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

    def test_the_website_bot_accepts_the_template(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location("dg", HERE.parent.parent / "stream" / "discord_growth.py")
        dg = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(dg)
        self.assertEqual(dg.check_template(self.t), [])
        self.assertEqual(self.t["posts"]["lfg"], "looking_for_group")
        self.assertEqual(self.t["posts"]["mod"], "mods_and_workshop")

    def test_roles_match_the_template(self):
        self.assertEqual([(n, f"#{col:06x}", h) for n, col, h, *_ in dl.ROLES],
                         [(r["name"], r["color"].lower(), bool(r.get("hoist"))) for r in self.t["roles"]])
        for n, _, _, _, perms in dl.ROLES:
            self.assertFalse(perms & 8, f"{n} must not get Administrator")

    def test_minimal_launch_layout(self):
        """Owner, 3 Nov: "minimalist and only things needed ... not too many channels"."""
        cats = {c: [s["name"] for s in chans] for c, chans in dl.LAYOUT}
        self.assertEqual(cats, {
            "START": ["welcome", "announcements", "guides-and-faq"],
            "PLAY": ["general", "help", "looking-for-group", "Hangout"],
            "CREATE": ["mods-and-workshop", "host-a-realm", "showcase"],
            "FEEDBACK": ["bug-reports", "ideas"],
            "🔒 STAFF": ["mod-log"]})
        self.assertLessEqual(len(plan_names()), 13)
        ro = {s["name"] for _, c in dl.LAYOUT for s in c if s["ro"]}
        self.assertEqual(ro, {"welcome", "announcements", "guides-and-faq"})
        self.assertEqual([s["private"] for _, c in dl.LAYOUT for s in c if s["private"]], [["Moderator", "Helper"]])
        topic = {s["name"]: s["topic"] or "" for _, chans in dl.LAYOUT for s in chans}
        self.assertIn("/submit", topic["mods-and-workshop"])
        self.assertIn("leylinelauncher.com/wrath/docs/hosting", topic["host-a-realm"])
        self.assertIn("leylinelauncher.com/wrath/download", topic["welcome"])
        self.assertNotIn("beacon", " ".join(topic.values()).lower())

    def test_emoji_names_and_aliases(self):
        names = plan_names()
        self.assertEqual(dl.GLYPH_SEP, "︱")
        self.assertEqual(set(dl.EMOJI), set(names), "one emoji per channel")
        for n in names:
            self.assertEqual(dl.slug(n), n.lower().replace(" ", "-"), "base names are plain")
            self.assertEqual(dl.slug(dl.EMOJI[n] + dl.GLYPH_SEP + n), dl.slug(n))
            self.assertNotIn("️", dl.EMOJI[n], "no variation selectors: the exact name must match")
        # the 30 Oct emoji stay on the channels they are (the owner's screenshot, 3 Nov)
        for emo, name in (("👋", "welcome"), ("📣", "announcements"), ("💬", "general"), ("🛟", "help"),
                          ("🤝", "looking-for-group"), ("🎧", "Hangout"), ("🧩", "mods-and-workshop"),
                          ("🏰", "host-a-realm"), ("🐞", "bug-reports"), ("💡", "ideas")):
            self.assertEqual(dl.EMOJI[name], emo, name)
        self.assertEqual(len(set(map(dl.slug, names))), len(names))
        for target, aliases in dl.ALIASES.items():
            self.assertIn(target, map(dl.slug, names))
            self.assertEqual(aliases[0], target)
        # every earlier pass lands here: the 30 Oct reset's names and the 1-2 Nov plan's
        for old, new in (("host-a-beacon", "host-a-realm"), ("hosting-help", "host-a-realm"),
                         ("hangout", "general"), ("launcher-help", "help"), ("mods-and-addons", "mods-and-workshop"),
                         ("den-showcase", "showcase"), ("feature-ideas", "ideas"), ("mod-room", "mod-log"),
                         ("dalaran-lounge", "hangout"), ("faq", "guides-and-faq")):
            self.assertIn(old, dl.ALIASES[new], old)
        for old, new in (("start-here", "start"), ("wrath-unleashed", "play"), ("your-den", "create"),
                         ("mods-&-workshop", "create"), ("host-a-server", "create"), ("team", "staff")):
            self.assertIn(old, dl.CATEGORY_ALIASES[new], old)
        self.assertIn("guide", dl.ROLE_ALIASES["Helper"])
        for _, chans in dl.LAYOUT:
            for s in chans:
                self.assertIsNone(dl.MONEY.search(s["topic"] or ""), s["name"])

    def test_topic_links_are_real_pages(self):
        import re
        ok = site_paths()
        for _, chans in dl.LAYOUT:
            for s in chans:
                for path in re.findall(r"leylinelauncher\.com(/[\w/-]*)", s["topic"] or ""):
                    self.assertIn(path, ok, s["name"])


class TestPostsFile(unittest.TestCase):
    """docs/discord-launch-posts.md: the pinned posts."""

    def test_drafts(self):
        import re
        import urllib.parse as up
        posts = dl.read_posts(POSTS)
        chans = [n for n, _ in posts]
        for n in ("welcome", "guides-and-faq", "announcements", "help", "mods-and-workshop", "host-a-realm",
                  "bug-reports", "ideas"):
            self.assertIn(n, chans)
        self.assertEqual(chans.count("guides-and-faq"), 2, "the guides and the FAQ, one pin each")
        go, leave = dl.plan_posts(posts)
        self.assertEqual([n for n, _, _ in leave], ["announcements"], "only the launch post is the owner's")
        self.assertIn("TODO(owner)", dict(posts)["announcements"])
        heads = [(n, dl.first_line(t)) for n, t in posts]
        self.assertEqual(len(set(heads)), len(heads), "first lines tell a channel's pins apart")
        ok = site_paths()
        text = POSTS.read_text(encoding="utf-8")
        for n, t in posts:
            self.assertLess(len(t), 2000, n)
            self.assertIsNone(dl.BANNED.search(t), n)
            self.assertIsNone(dl.MONEY.search(t), n)
            for u in re.findall(r"https?://[^\s)`]+", t):
                self.assertNotRegex(u, r"[.,;:!?]$", f"{n}: punctuation right after a link")
                parts = up.urlsplit(u)
                self.assertIn(parts.hostname, {"leylinelauncher.com", "discord.gg"}, u)
                if parts.hostname == "leylinelauncher.com":
                    self.assertIn(parts.path.rstrip("/") or "/wrath", ok, u)
                else:
                    self.assertEqual(u, "https://discord.gg/RfChVr8J7T")
            for ref in re.findall(r"(?<![\w<&/])#([a-z0-9-]+)\b", t):
                self.assertIn(ref, [dl.slug(x) for x in plan_names()], (n, ref))
        welcome = dict(posts)["welcome"]
        for want in ("Get the launcher", "Log in", "Hero Key", "game folder", "The rules", "#guides-and-faq",
                     "discord.gg/RfChVr8J7T"):
            self.assertIn(want, welcome)
        guides = [t for n, t in posts if n == "guides-and-faq"]
        for page in ("getting-started", "hero-key", "/play", "/den", "den-invite", "hosting", "/wrath/mods",
                     "/wrath/modes", "common-fixes"):
            self.assertTrue(any(page in g for g in guides), page)
        self.assertNotRegex(text, r"(?i)\bsign(ed)?[ -]?in\b", "Log in, not sign in")

    def test_anchors_exist(self):
        """The #anchors the drafts link are headings on those guide pages."""
        import re
        for page, anchor in re.findall(r"wrath/docs/([a-z-]+)#([a-z-]+)", POSTS.read_text(encoding="utf-8")):
            md = (HERE.parent.parent / "docs" / "leyline" / f"{page}.md").read_text(encoding="utf-8")
            heads = [re.sub(r"[^a-z0-9 -]", "", h.lower()).strip().replace(" ", "-")
                     for h in re.findall(r"(?m)^#+ (.+)$", md)]
            self.assertIn(anchor, heads, f"{page}#{anchor}")


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
        for s in ("PREVIEW", "Choices: old channels: move them to the hidden ARCHIVE; pinned posts: printed for you",
                  "New channels:", "category START", "🤝︱looking-for-group in PLAY", "Reused channels",
                  'general -> 💬︱general (keeps its history; rename, move to PLAY, topic: "Talk about anything',
                  "General -> 🎧︱Hangout", "ov-realm-help -> ARCHIVE (hidden, read-only, history kept)",
                  "wrath-dev -> ARCHIVE (hidden, read-only, no messages)", "AFK -> ARCHIVE",
                  "no messages in: wrath-dev, rules, AFK", "Delete the empty ones",
                  "create Helper", "keep moderator (it is the plan's Moderator)", "--delete-old-roles", "Realm-Ally",
                  "Do these by hand", "Enable Community", "📣︱announcements: moderator can post",
                  "🔒︱mod-log: only Moderator, Helper", "Pinned posts:", "👋︱welcome: no pinned post yet",
                  "📣︱announcements: left for you to post and pin (has TODO(owner)", "Nothing changed",
                  "It posts nothing"):
            self.assertIn(s, out)
        self.assertIn("nothing (old channels stay in ARCHIVE", out)
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
        for args, stdin in (((), "YES\n"), (("--old-channels", "delete-empty", "--pin-posts"), "YES\nPOST\n")):
            self.setUp()
            n_prev = changes(self.run_script(*args).stdout)
            self.apply(*args, stdin=stdin)
            done = [l for l in self.outputs[-1].splitlines() if l.strip().startswith("done:")]
            self.assertEqual(len(done), n_prev, args)
            self.tearDown()
        self.setUp()


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
        gen, voice, ann = self.ch("general", (0,)), self.ch("hangout", (2,)), self.ch("announcements")
        self.assertEqual((gen["id"], ann["id"], voice["id"]), ("20", "21", "30"))      # reused, history kept
        self.assertEqual(gen["parent_id"], cats["PLAY"]["id"])
        self.assertEqual(gen["topic"], dl.LAYOUT[1][1][0]["topic"])
        self.assertEqual(voice["name"], "🎧︱Hangout")
        self.assertEqual(w["guild"]["system_channel_id"], self.ch("welcome")["id"])
        for cat, chans in dl.LAYOUT:
            for s in chans:
                c = self.ch(dl.slug(s["name"]))
                self.assertEqual(c["name"], s["full"])
                self.assertEqual(c["type"], s["type"], s["name"])
                self.assertEqual(c["parent_id"], cats[cat]["id"], s["name"])
                if s["type"] == dl.TEXT:
                    self.assertEqual(c["topic"], s["topic"], s["name"])
        # read-only: @everyone can read, not post; the named roles can post
        mod, helper = self.role("moderator"), self.role("Helper")
        for name, posters in (("welcome", []), ("announcements", [mod]), ("guides-and-faq", [mod, helper])):
            ow = dl.ow_map(self.ch(name))
            self.assertTrue(int(ow[GID]["deny"]) & dl.P_SEND, name)
            self.assertFalse(int(ow[GID]["deny"]) & dl.P_VIEW, name)
            for r in posters:
                self.assertTrue(int(ow[r["id"]]["allow"]) & dl.P_SEND, (name, r["name"]))
        self.assertNotIn(GID, dl.ow_map(gen))
        ow = dl.ow_map(self.ch("mod-log"))
        self.assertTrue(int(ow[GID]["deny"]) & dl.P_VIEW)
        self.assertTrue(int(ow[mod["id"]]["allow"]) & dl.P_VIEW)
        self.assertTrue(int(ow[helper["id"]]["allow"]) & dl.P_VIEW)
        order = [c["name"] for c in sorted((c for c in w["channels"] if c["type"] == 4), key=dl.okey)]
        self.assertEqual(order[:len(dl.LAYOUT) + 1], [c for c, _ in dl.LAYOUT] + ["ARCHIVE"])
        play = [c["name"] for c in sorted((c for c in w["channels"] if c["parent_id"] == cats["PLAY"]["id"]
                                           and c["type"] == 0), key=dl.okey)]
        self.assertEqual(play, [s["full"] for s in dl.LAYOUT[1][1] if s["type"] == dl.TEXT])
        # roles: the existing "moderator" kept as it is (name and colour); Helper made
        self.assertEqual((mod["id"], mod["name"], mod["color"]), ("54", "moderator", 0x111111))
        self.assertEqual((helper["color"], helper["hoist"]), (0x5B8DEF, True))
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
        for cid in ("22", "23", "24", "31"):
            c = self.fake.chan(cid)
            self.assertIsNotNone(c, cid)                        # nothing deleted by default
            self.assertEqual(c["parent_id"], arch["id"], cid)
            self.assertEqual(dl.ow_norm(c["permission_overwrites"]), dl.ow_norm(arch["permission_overwrites"]))
            self.assertNotIn("53", dl.ow_map(c))                # an old role's view grant is gone (synced)
        for cid in ("10", "11"):                                # empty old categories hidden, not deleted
            self.assertTrue(int(dl.ow_map(self.fake.chan(cid))[GID]["deny"]) & dl.P_VIEW)
        self.assertEqual(len(w["messages"]["20"]), 250)
        self.assertFalse([x for x in self.fake.writes() if x[0] == "DELETE" and "/permissions/" not in x[1]])

    def test_second_run_changes_nothing(self):
        self.apply()
        n = len(self.fake.writes())
        snap = copy.deepcopy(self.fake.w)
        p = self.run_script("--apply", stdin="")                 # nothing to confirm when nothing changes
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertEqual(len(self.fake.writes()), n, [x[:3] for x in self.fake.writes()[n:]])
        self.assertEqual(self.fake.w, snap)
        self.assertIn("Nothing to change", p.stdout)
        self.assertIn("Nothing to change", self.run_script().stdout)

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
        ann, bugs = self.ch("announcements")["id"], self.ch("bug-reports")["id"]
        for s in (f"DISCORD_BUG_CHANNEL_ID={bugs}", f"DISCORD_GUILD_ID={GID}", f"WU_DISCORD_GUILD={GID}",
                  "WU_DISCORD_TEMPLATE=/app/discord_template_launch.json", f"WU_FORUM_DISCORD_CHANNELS=news={ann}",
                  "WU_DISCORD_BUG_INTAKE=/srv/wrath/bugs/intake/discord.jsonl",
                  "DISCORD_CHAT_CHANNEL_ID: the launch layout has no live game-chat channel"):
            self.assertIn(s, p.stdout)


class TestFromToday(Base):
    """The owner's server today (his 3 Nov screenshot): only a few changes, every one listed in the preview."""
    world = staticmethod(today_world)
    community = True

    def test_preview_lists_exactly_the_changes(self):
        p = self.run_script()
        self.assertEqual(self.fake.writes(), [])
        out = p.stdout
        for s in ("📖︱guides-and-faq in START, read-only; topic: \"Every guide and the common questions",
                  "🏰︱host-a-beacon -> 🏰︱host-a-realm (keeps its history; rename, topic:",
                  "📷︱showcase -> 📸︱showcase (keeps its history; rename, topic:",
                  "mod-log -> 🔒︱mod-log (keeps its history; rename, topic: \"Staff only",
                  "🛟︱help (keeps its name and history; topic: \"Stuck? Ask here",
                  "keep Moderator", "keep Helper", "system messages, rules channel and AFK already right",
                  "2 old channel(s) already in ARCHIVE", "no messages in: rules"):
            self.assertIn(s, out)
        for never in ("* category START", "rename category", "create Moderator", "create Helper", "DELETED:\n  *",
                      "Hangout -> ", "a plain text channel"):
            self.assertNotIn(never, out)
        self.assertNotIn("old-stuff -> ARCHIVE", out, "already in ARCHIVE as it should be: no change")

    def test_apply_keeps_history_and_reruns_clean(self):
        self.apply()
        w = self.fake.w
        cats = {c["name"]: c["id"] for c in w["channels"] if c["type"] == 4}
        self.assertEqual((cats["START"], cats["PLAY"], cats["CREATE"], cats["FEEDBACK"], cats["🔒 STAFF"]),
                         ("10", "11", "12", "13", "15"))
        host = self.fake.chan("27")
        self.assertEqual((host["name"], host["parent_id"]), ("🏰︱host-a-realm", "12"))
        self.assertEqual(len(w["messages"]["27"]), 1)
        self.assertEqual(self.fake.chan("21")["type"], 5, "the announcement channel stays one")
        self.assertEqual(self.fake.chan("33")["name"], "🔒︱mod-log")
        self.assertEqual(self.ch("guides-and-faq")["parent_id"], "10")
        self.assertEqual(w["guild"]["rules_channel_id"], "20")
        self.assertEqual(self.role("Moderator")["color"], 0x8B5CF6)
        self.assertEqual(self.fake.chan("31")["parent_id"], "14")
        n = len(self.fake.writes())
        p = self.apply(stdin="")
        self.assertEqual(len(self.fake.writes()), n, [x[:3] for x in self.fake.writes()[n:]])
        self.assertIn("Nothing needed changing", p.stdout)

    def test_the_website_bot_finds_every_channel_afterwards(self):
        """Set up the Discord (stream/discord_growth.py) looks channels up by (exact name, type; an announcement
        channel counts as text): all found."""
        self.apply()
        t = json.loads(TEMPLATE.read_text(encoding="utf-8"))
        types = {"text": 0, "voice": 2, "stage": 13, "forum": 15}
        seen = {(c["name"].lower(), 0 if c["type"] == 5 else c["type"]) for c in self.fake.w["channels"]}
        roles = {r["name"].lower() for r in self.fake.w["roles"]}
        for cat in t["categories"]:
            self.assertIn((cat["name"].lower(), 4), seen, cat["name"])
            for c in cat["channels"]:
                self.assertIn((c["name"].lower(), types[c.get("type", "text")]), seen, c["name"])
        for r in t["roles"]:
            self.assertIn(r["name"].lower(), roles)


class TestFromTheReset(Base):
    """The server as the 30 Oct reset left it (glyph names), on a Community server."""
    world = staticmethod(reset_world)
    community = True

    def test_glyph_channels_are_renamed_not_duplicated(self):
        self.apply()
        w = self.fake.w
        for cid, name in (("20", "👋︱welcome"), ("22", "💬︱general"), ("27", "🏰︱host-a-realm"),
                          ("29", "🐞︱bug-reports"), ("30", "💡︱ideas"), ("25", "🎧︱Hangout")):
            c = self.fake.chan(cid)
            self.assertEqual(c["name"], name, cid)
            self.assertEqual(sum(dl.slug(x["name"]) == dl.slug(name) for x in w["channels"]), 1, name)
        self.assertFalse([x for x in self.fake.writes() if x[1] == "/channels/24" and "name" in (x[2] or {})])
        self.assertEqual(len(w["messages"]["30"]), 1)
        self.assertEqual(self.ch("mod-log")["parent_id"], self.ch("staff", (4,))["id"])
        for name in ("announcements", "guides-and-faq", "mod-log"):
            self.assertTrue(int(dl.ow_map(self.ch(name))["64"]["allow"]) & dl.P_SEND, name)   # the website bot
        self.assertNotIn("64", dl.ow_map(self.ch("general")))
        self.assertEqual(self.role("Helper")["id"], "61")
        n = len(self.fake.writes())
        self.apply(stdin="")
        self.assertEqual(len(self.fake.writes()), n, [x[:3] for x in self.fake.writes()[n:]])

    def test_a_guide_role_counts_as_helper(self):
        next(r for r in self.fake.w["roles"] if r["id"] == "61")["name"] = "Guide"
        p = self.apply()
        self.assertIn("keep Guide (it is the plan's Helper)", p.stdout)
        self.assertFalse([r for r in self.fake.w["roles"] if r["name"] == "Helper"])


class TestOldChannels(Base):
    """Old channels: archived by default; the owner's card can delete the empty ones, or all of them."""
    world = staticmethod(today_world)

    def test_delete_empty_keeps_the_ones_with_messages(self):
        prev = self.run_script("--old-channels", "delete-empty")
        self.assertIn("old channel rules (no messages)", prev.stdout)
        self.assertNotIn("old channel old-stuff", prev.stdout)
        self.assertIn("Choices: old channels: delete the empty ones, archive the rest", prev.stdout)
        self.apply("--old-channels", "delete-empty")
        self.assertIsNone(self.fake.chan("32"))
        self.assertEqual(self.fake.chan("31")["parent_id"], "14", "a channel with messages stays in ARCHIVE")
        dels = [x for x in self.fake.writes() if x[0] == "DELETE" and "/permissions/" not in x[1]]
        self.assertEqual([x[1] for x in dels], ["/channels/32"])
        self.assertTrue(all(x[4] for x in dels), "deleted only after the backup")
        n = len(self.fake.writes())
        self.apply("--old-channels", "delete-empty", stdin="")
        self.assertEqual(len(self.fake.writes()), n)

    def test_delete_all_needs_the_server_name(self):
        p = self.run_script("--apply", "--old-channels", "delete-all", stdin="YES\nnope\n")
        self.assertEqual(p.returncode, 1)
        self.assertEqual(self.fake.writes(), [])
        p = self.apply("--old-channels", "delete-all", stdin="YES\nLeyline\n")
        self.assertIn("old channel old-stuff (has messages: the backup keeps its last 1000)", p.stdout)
        for cid in ("31", "32", "14"):
            self.assertIsNone(self.fake.chan(cid), cid)
        b = self.backup_dirs()[-1]
        self.assertTrue((b / "messages" / "old-stuff-31.json").exists())
        self.assertEqual(self.fake.chan("22")["name"], "💬︱general", "plan channels are never deleted")

    def test_old_categories_deleted_when_deleting(self):
        self.fake.w["channels"].append({"id": "16", "name": "Text Channels", "type": 4, "parent_id": None,
                                        "position": 9, "guild_id": GID, "permission_overwrites": [], "topic": None})
        p = self.apply("--old-channels", "delete-empty")
        self.assertIn("empty old category 'Text Channels'", p.stdout)
        self.assertIsNone(self.fake.chan("16"))

    def test_a_channel_the_bot_cannot_read_counts_as_used(self):
        self.fake.fail_once[("GET", "/channels/32/messages")] = 403
        p = self.run_script("--old-channels", "delete-empty")
        self.assertNotIn("old channel rules (no messages)", p.stdout)


class TestPins(Base):
    world = staticmethod(today_world)

    def posted(self):
        return [(x[1].split("/")[2], x[2]) for x in self.fake.writes()
                if x[0] == "POST" and x[1].endswith("/messages")]

    def test_no_tick_posts_nothing_and_prints_them(self):
        p = self.apply()
        self.assertEqual(self.posted(), [])
        self.assertIn("----- #welcome", p.stdout)
        self.assertIn("----- #announcements", p.stdout)
        self.assertIn("How to report a bug", p.stdout)

    def test_pin_posts_without_post_posts_nothing(self):
        p = self.apply("--pin-posts", stdin="YES\nyes\n")
        self.assertEqual(self.posted(), [])
        self.assertIn("won't post", p.stdout)

    def test_pin_posts_posts_finished_drafts_once_and_edits_in_place(self):
        prev = self.run_script("--pin-posts")
        self.assertIn('👋︱welcome: post and pin "**Welcome to Leyline** 👋"', prev.stdout)
        self.assertIn("📣︱announcements: left for you to post and pin (has TODO(owner)", prev.stdout)
        self.assertEqual(self.fake.writes(), [])
        self.apply("--pin-posts", stdin="YES\nPOST\n")
        got = self.posted()
        ids = {n: self.ch(dl.slug(n))["id"] for n in plan_names()}
        where = [c for c, _ in got]
        self.assertEqual(where.count(ids["guides-and-faq"]), 2)
        for n in ("welcome", "help", "mods-and-workshop", "host-a-realm", "bug-reports", "ideas"):
            self.assertIn(ids[n], where, n)
        self.assertNotIn(ids["announcements"], where)
        for _, body in got:
            self.assertNotIn("TODO(owner)", body["content"])
            self.assertEqual(body["allowed_mentions"], {"parse": []})
        self.assertEqual(len(self.fake.w["pins"]), len(got))
        welcome = next(b for c, b in got if c == ids["welcome"])["content"]
        self.assertIn(f"<#{ids['guides-and-faq']}>", welcome)                   # #names become links
        self.assertIn("----- #announcements", self.outputs[-1])                   # the owner's, printed
        # again: every pin up to date, nothing posted
        n = len(self.fake.writes())
        p = self.apply("--pin-posts", stdin="")
        self.assertEqual(len(self.fake.writes()), n, [x[:3] for x in self.fake.writes()[n:]])
        self.assertIn("👋︱welcome: pinned post up to date", p.stdout)
        # the owner edits a draft: the bot's own pin is edited, not posted again
        posts = self.tmp / "posts.md"
        posts.write_text(POSTS.read_text(encoding="utf-8").replace("Ideas for Leyline", "Your ideas for Leyline")
                         .replace("One idea per message", "One idea per message, please"), encoding="utf-8")
        prev = self.run_script("--posts-file", str(posts))
        self.assertIn("💡︱ideas: its pinned post differs from the draft", prev.stdout)
        self.apply("--pin-posts", "--posts-file", str(posts), stdin="YES\nPOST\n")
        edits = [x for x in self.fake.writes()[n:] if x[0] == "PATCH" and "/messages/" in x[1]]
        self.assertEqual(len(edits), 1)
        self.assertEqual(len(self.posted()), len(got))
        self.assertIn("One idea per message, please", edits[0][2]["content"])

    def test_someone_elses_pin_is_left_alone(self):
        self.fake.w["messages"]["30"] = [{"id": "300009", "content": "**Ideas** 💡\n\nold text", "author": {"id": "7"},
                                          "timestamp": "2026-11-01T00:00:00", "attachments": []}]
        self.fake.w["pins"] = [("30", "300009")]
        p = self.apply("--pin-posts", stdin="YES\nPOST\n")
        self.assertIn("💡︱ideas: pinned by someone else", p.stdout)
        self.assertNotIn("30", [c for c, _ in self.posted()])


class TestRetries(Base):
    def test_429_and_5xx_are_retried(self):
        self.fake.fail_once[("PATCH", f"/guilds/{GID}")] = 429
        self.fake.fail_once[("POST", f"/guilds/{GID}/roles")] = 429
        self.fake.fail_once[("GET", f"/guilds/{GID}/channels")] = 502
        self.apply()
        self.assertEqual(self.fake.served_429, 2)
        self.assertEqual(self.fake.served_5xx, 1)
        self.assertIsNone(self.fake.w["guild"]["afk_channel_id"])
        self.assertEqual(sum(r["name"] == "Helper" for r in self.fake.w["roles"]), 1)

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
        self.apply()
        self.assertEqual(self.role("moderator")["color"], 0x111111)
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
        self.assertEqual(len([c for c in self.fake.w["channels"] if c["name"] == "💬︱general"]), 1)
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
        self.assertEqual(self.fake.chan("20")["name"], "💬︱general")     # reused channels never deleted
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
        self.assertEqual(g["rules_channel_id"], self.ch("welcome")["id"])
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
        self.assertIn("The Leyline website bot isn't in the server", self.outputs[-1])
        self.assertIn("Onboarding", self.outputs[-1])
        n = len(self.fake.writes())
        self.apply()
        self.assertEqual(len(self.fake.writes()), n)


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
        self.apply("--pin-posts", stdin="YES\nPOST\n")
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
        self.apply("--pin-posts", stdin="YES\nPOST\n")
        for m, path, body, *_ in self.fake.writes():
            for b in (body if isinstance(body, list) else [body or {}]):
                for k in ("name", "topic", "content"):
                    if isinstance(b.get(k), str):
                        self.assertIsNone(dl.BANNED.search(b[k]), b[k])
        for bad in ("World of Warcraft", "WoW", "warcraft", "Lich King", "Blizzard", "3.3.5a", "1.12"):
            self.assertIsNotNone(dl.BANNED.search(bad), bad)
        for bad in ("patreon.com/x", "ko-fi", "Tip me", "donate"):
            self.assertIsNotNone(dl.MONEY.search(bad), bad)

    def test_posting_refuses_rule_breakers(self):
        go, leave = dl.plan_posts([("general", "come play WoW"), ("general", "tips: ko-fi.com/x"),
                                   ("nowhere", "hi"), ("Hangout", "voice"), ("bug-reports", "fine")])
        self.assertEqual([n for n, *_ in go], ["bug-reports"])
        self.assertEqual(len(leave), 4)


class TestFromControl(Base):
    """Leyline Control's Discord steps: --from-control never asks, --yes-from-control is Control's do it (the owner
    typed YES in Control), the card's choices come in LEYLINE_DISCORD_OLD / LEYLINE_DISCORD_PINS, --bot-settings only
    looks; and the token is in none of their output."""
    CTL = {"LEYLINE_CONTROL_STEP": "discord-apply"}
    world = staticmethod(today_world)

    def test_preview_never_asks_or_writes(self):
        p = self.run_script("--from-control")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn("PREVIEW (nothing will change)", p.stdout)
        self.assertIn("Leyline Control > Discord > Do it", p.stdout)
        self.assertGreater(changes(p.stdout), 0)
        self.assertEqual(self.fake.writes(), [])

    def test_no_token_file_stops_instead_of_asking(self):
        p = self.run_script("--from-control", token_file=self.tmp / "missing.txt")
        self.assertNotEqual(p.returncode, 0)
        self.assertIn("Leyline Control > Discord", p.stdout + p.stderr)
        self.assertNotIn("Bot token:", p.stdout + p.stderr)
        self.assertEqual(self.fake.log, [])

    def test_yes_from_control_backs_up_then_applies(self):
        n = changes(self.run_script("--from-control").stdout)
        p = self.run_script("--yes-from-control", extra_env={**self.CTL, "LEYLINE_DISCORD_EXPECT_CHANGES": str(n)})
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn("You typed YES in Leyline Control", p.stdout)
        self.assertNotIn("Type YES", p.stdout)
        writes = self.fake.writes()
        self.assertTrue(writes)
        self.assertTrue(all(w[4] for w in writes), "a write before the backup")
        self.assertEqual(len(self.backup_dirs()), 1)
        self.assertIn("Nothing to change", self.run_script("--from-control").stdout)

    def test_the_cards_choices(self):
        card = {"LEYLINE_DISCORD_OLD": "all", "LEYLINE_DISCORD_PINS": "1"}
        prev = self.run_script("--from-control", extra_env=card)
        self.assertIn("Choices: old channels: delete them all (after the backup); pinned posts: the bot posts", prev.stdout)
        self.assertIn("old channel old-stuff (has messages", prev.stdout)
        n = changes(prev.stdout)
        self.assertGreater(n, changes(self.run_script("--from-control").stdout))
        p = self.run_script("--yes-from-control", extra_env={**self.CTL, **card,
                                                             "LEYLINE_DISCORD_EXPECT_CHANGES": str(n)})
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertNotIn("Type the server's name", p.stdout)
        self.assertNotIn("Type POST", p.stdout)
        self.assertIsNone(self.fake.chan("31"))
        self.assertTrue(self.fake.w.get("pins"))
        self.assertIn("Nothing to change", self.run_script("--from-control", extra_env=card).stdout)

    def test_choices_from_flags_are_refused_from_control(self):
        for flag in (("--old-channels", "delete-all"), ("--pin-posts",)):
            p = self.run_script("--from-control", *flag)
            self.assertNotEqual(p.returncode, 0, flag)
        self.assertEqual(self.fake.log, [])

    def test_yes_from_control_only_from_the_step(self):
        p = self.run_script("--yes-from-control", extra_env={"LEYLINE_DISCORD_EXPECT_CHANGES": "5"})
        self.assertNotEqual(p.returncode, 0)
        self.assertIn("only works from Leyline Control", p.stdout)
        p = self.run_script("--yes-from-control", extra_env=self.CTL)            # no count from the preview
        self.assertNotEqual(p.returncode, 0)
        for flag in ("--delete-archived", "--wipe-history", "--delete-old-roles", "--post-first-posts"):
            p = self.run_script("--yes-from-control", flag, extra_env={**self.CTL, "LEYLINE_DISCORD_EXPECT_CHANGES": "1"})
            self.assertNotEqual(p.returncode, 0, flag)
        self.assertEqual(self.fake.writes(), [])
        self.assertEqual(self.backup_dirs(), [])

    def test_stops_when_the_server_changed_since_the_preview(self):
        n = changes(self.run_script("--from-control").stdout)
        p = self.run_script("--yes-from-control", extra_env={**self.CTL, "LEYLINE_DISCORD_EXPECT_CHANGES": str(n + 1)})
        self.assertEqual(p.returncode, 3, p.stdout)
        self.assertIn("Run the preview again", p.stdout)
        self.assertEqual(self.fake.writes(), [])
        self.assertEqual(self.backup_dirs(), [])

    def test_bot_settings_only_look(self):
        self.apply()
        before = len(self.fake.writes())
        p = self.run_script("--from-control", "--bot-settings")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn("WU_DISCORD_GUILD=1", p.stdout)
        self.assertIn("DISCORD_BUG_CHANNEL_ID=29", p.stdout)
        self.assertNotIn("<made when you run Do it>", p.stdout)
        self.assertEqual(len(self.fake.writes()), before)
        self.assertNotEqual(self.run_script("--bot-settings", "--apply").returncode, 0)

    def test_token_never_in_control_output(self):
        n = changes(self.run_script("--from-control").stdout)
        self.run_script("--yes-from-control", extra_env={**self.CTL, "LEYLINE_DISCORD_EXPECT_CHANGES": str(n)})
        self.run_script("--from-control", "--bot-settings")
        self.assertEqual(len(self.outputs), 3)
        for out in self.outputs:
            self.assertIn("token", out.lower())                    # it does talk about the token...
            self.assertNotIn(TOKEN, out)                           # ...never shows it
            for part in TOKEN.split("."):
                self.assertNotIn(part, out)
        for f in self.tmp.rglob("*"):
            if f.is_file() and f != self.token_file:
                self.assertNotIn(TOKEN, f.read_text(encoding="utf-8", errors="replace"), str(f))


class TestOldInviteGone(unittest.TestCase):
    """The old Discord invites are gone from everything players see (3 Nov: discord.gg/RfChVr8J7T)."""

    def test_no_old_invite(self):
        root = HERE.parent.parent
        out = subprocess.run(["git", "-C", str(root), "grep", "-n", "-I", "-e", "hVeUxYwbkr", "--", ".",
                              ":!tools/discord-leyline/test_discord_leyline.py"], capture_output=True, text=True)
        self.assertEqual(out.stdout, "", out.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
