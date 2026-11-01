"""The Discord bot's growth side (stream/discord_growth.py, loaded by stream/forum_discord.py) through the real portal
(stream/wrath_portal.py), the real forum, social.py, admin.py and HQ, against a fake website sign-in and a FAKE DISCORD
API (a local HTTP server keeping roles, channels, members, messages and AutoMod rules in memory, failing on demand).
No network, no real token.

Covers: everything off by default (no calls, Find a group 404, the HQ tab says why); the bot alone with WU_DISCORD_GUILD
(no forum channels); the template (the shipped one is valid, a bad one is refused, private channels never made public);
setup (owner only with the typed confirm, Preview makes nothing, missing roles / categories / channels made with the
right overwrites, slowmode and topics, what exists left alone, a stage refused without stopping the rest, a second run
makes nothing, never a DELETE or PATCH); the live realms feed (realms already live when it starts are not announced, a
new realm gets a card with name, mode, players and its link, /r/<short> when it has one, the hourly cap busiest first,
held cards dropped after an hour, back after 6 hours announced again, a short gap not, the portal away no flood, brand
names and hidden realms left out); staff picks (Den of the week and the featured mode posted, the Den Architect fact,
outside links and the game's name refused, feed off: recorded, not posted); Find a group (off, unlinked, linked from the
website and from the launcher with a Bearer token, the rate limit, the word filter, a realm not live); roles from
accounts (Realm Host from a hosted realm listed 7 days, recorded facts, a person not in the server, idempotent, a role
staff gave by hand never taken, taken back when the fact goes, unlinked people lose the bot's roles, Den Architect kept,
resumable after Discord fails mid-way, the audit log); AutoMod (off, made, updated, a 403 explained, pushed again only
when the words change); HQ's Discord tab; the token never in a log; and the deploy step ships both files.

    python3 -I stream/tests/test_discord_growth.py stream/wrath_portal.py
"""
import http.client, importlib.util, io, itertools, json, logging, os, re, sys, tempfile, threading, time, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "stream", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
TMP = tempfile.mkdtemp(prefix="wu-dg-")
TOKEN = "fake.bot.token-" + "Z" * 30
fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra!r}"[:900] if extra and not cond else ""))
    fails += 0 if cond else 1


# ------------------------------------------------------------------------------------------- the fake website
ACCOUNTS = {"alice": ("alice", "Alice", "WR000001"), "bob": ("bob", "Bob", "WR000002"), "owner": ("owner", "Boss", "WR000009"),
            "carl": ("carl", "Carl", "WR000004")}


class Site(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_GET(self):
        m = re.search(r"wr_session=(\w+)", self.headers.get("Cookie", ""))
        a = ACCOUNTS.get(m.group(1)) if m else None
        if not a:
            self.send_response(401); self.send_header("Content-Length", "0"); self.end_headers(); return
        doc = {"/api/me": {"id": a[0], "display_name": a[1]}, "/api/play": {"user": a[2], "pass": "x" * 16}}[self.path]
        b = json.dumps(doc).encode()
        self.send_response(200); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)


# ------------------------------------------------------------------------------------------- the fake Discord
_seq = itertools.count(1000)
snow = lambda: str(800000000000000000 + next(_seq))   # noqa: E731
BOT = "900000000000000001"
GUILD = "770000000000000001"
D = {"roles": {GUILD: {"id": GUILD, "name": "@everyone"}}, "channels": {}, "members": {}, "msgs": {}, "rules": {},
     "calls": [], "auth": set(), "fail": [], "community": False}
lock = threading.Lock()


def add_role(name):
    rid = snow()
    D["roles"][rid] = {"id": rid, "name": name}
    return rid


def add_channel(name, typ, parent=None):
    cid = snow()
    D["channels"][cid] = {"id": cid, "name": name, "type": typ, "parent_id": parent, "guild_id": GUILD}
    return cid


class Discord(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def reply(self, status, doc=None):
        b = json.dumps(doc).encode() if doc is not None else b""
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def handle_any(self, method):
        n = int(self.headers.get("Content-Length") or 0)
        body = json.loads(self.rfile.read(n)) if n else None
        path = urllib.parse.urlsplit(self.path).path.replace("/api/v10", "", 1)
        with lock:
            D["auth"].add(self.headers.get("Authorization", ""))
            D["calls"].append((method, path, body))
            for i, f in enumerate(D["fail"]):
                if re.search(f[0], method + " " + path):
                    D["fail"].pop(i)
                    return self.reply(f[1], f[2])
            return self.route(method, path, body)

    def route(self, method, path, body):
        G = f"/guilds/{GUILD}"
        if path == "/users/@me":
            return self.reply(200, {"id": BOT, "username": "Leyline"})
        if path == G + "/threads/active":
            return self.reply(200, {"threads": [], "members": []})
        if path == G + "/roles":
            if method == "GET":
                return self.reply(200, list(D["roles"].values()))
            rid = add_role(body["name"])
            D["roles"][rid].update(body)
            return self.reply(200, D["roles"][rid])
        if path == G + "/channels":
            if method == "GET":
                return self.reply(200, list(D["channels"].values()))
            if body["type"] == 13 and not D["community"]:
                return self.reply(400, {"message": "Invalid Form Body", "code": 50035})
            cid = add_channel(body["name"], body["type"], body.get("parent_id"))
            D["channels"][cid].update(body)
            return self.reply(201, D["channels"][cid])
        m = re.fullmatch(G + r"/members/(\d+)(?:/roles/(\d+))?", path)
        if m:
            mem = D["members"].get(m.group(1))
            if mem is None:
                return self.reply(404, {"message": "Unknown Member", "code": 10007})
            if not m.group(2):
                return self.reply(200, mem)
            if method == "PUT":
                if m.group(2) not in mem["roles"]:
                    mem["roles"].append(m.group(2))
            elif method == "DELETE":
                mem["roles"] = [r for r in mem["roles"] if r != m.group(2)]
            return self.reply(204)
        m = re.fullmatch(G + r"/auto-moderation/rules(?:/(\d+))?", path)
        if m:
            if method == "GET":
                return self.reply(200, list(D["rules"].values()))
            if method == "POST":
                rid = snow()
                D["rules"][rid] = dict(body, id=rid, creator_id=BOT)
                return self.reply(200, D["rules"][rid])
            if method == "PATCH":
                D["rules"][m.group(1)].update(body)
                return self.reply(200, D["rules"][m.group(1)])
        m = re.fullmatch(r"/channels/(\d+)/messages", path)
        if m and method == "POST":
            if m.group(1) not in D["channels"]:
                return self.reply(404, {"message": "Unknown Channel", "code": 10003})
            mid = snow()
            D["msgs"].setdefault(m.group(1), []).append(dict(body, id=mid))
            return self.reply(200, {"id": mid})
        return self.reply(404, {"message": "Unknown", "code": 0})

    def do_GET(self): self.handle_any("GET")
    def do_POST(self): self.handle_any("POST")
    def do_PATCH(self): self.handle_any("PATCH")
    def do_PUT(self): self.handle_any("PUT")
    def do_DELETE(self): self.handle_any("DELETE")


def serve(handler):
    srv = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv.server_address[1]


SITE, DISC = serve(Site), serve(Discord)

# the server as the owner left it: a few things already made by hand
MOD_ROLE = add_role("Moderator")
START = add_channel("Start here", 4)
RULES = add_channel("rules", 0, START)

# ------------------------------------------------------------------------------------------- the portal
logbuf = io.StringIO()
lh = logging.StreamHandler(logbuf)
lh.setLevel(logging.DEBUG)
logging.getLogger().addHandler(lh)
logging.getLogger().setLevel(logging.INFO)
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL=f"http://127.0.0.1:{SITE}",
                  WU_STREAM="0", STREAM_HOST="wrath.example", WU_PORTAL_DATA=TMP, WU_HOST_DIR=os.path.join(TMP, "hosted"),
                  WU_RELAY_LISTING="http://127.0.0.1:9/none", WU_ADMIN_STAFF="", WU_ADMIN_OWNERS="owner", WU_ADMIN_GM_LEVEL="0",
                  PUBLIC_HOST="play.example", WU_HUB_FRONT="0")
for k in list(os.environ):
    if k.startswith("WU_DISCORD") or k in ("WU_FORM_SECRET", "WU_FORUM_DISCORD", "DISCORD_BOT_TOKEN", "WU_FORUM_DISCORD_CHANNELS"):
        os.environ.pop(k, None)
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
wp.ac_soap = lambda cmd: ""
wp.social.setup(start_thread=False)
F = wp.forum.setup()
FM = wp.forum
wp.browse_limit.hit = lambda *a, **k: True
wp.admin.POSTS_PER_MIN = wp.admin.IP_PER_MIN = wp.admin.PAGES_PER_MIN = 10 ** 6
PAGE = serve(wp.PageHandler)
HQ = wp.admin.hq
OWNER = {"account": "owner", "name": "Boss", "role": "owner", "session": "s"}
ADMIN = {"account": "adm", "name": "Ann", "role": "admin", "session": "s"}
MOD = {"account": "mod", "name": "Mo", "role": "mod", "session": "s"}

_off = HQ.page(OWNER, "discord", {})
check("off by default: no Discord bot, HQ's Discord tab says why", F.discord is None and "WU_FORUM_DISCORD=1 isn" in _off[2], _off)
check("HQ's Content row has Discord", ("discord", "Discord") in wp.admin.CONTENT_TABS)
ok, msg = wp.admin.run_action(OWNER, {"action": "discord.setup", "target": "plan", "confirm": "setup"})
check("off: Set up the Discord refused, plainly", not ok and "off" in msg, msg)

ENV = {"WU_FORUM_DISCORD": "1", "DISCORD_BOT_TOKEN": TOKEN, "WU_DISCORD_GUILD": GUILD,
       "WU_DISCORD_API": f"http://127.0.0.1:{DISC}/api/v10"}
os.environ.update(ENV)
L = FM.discord_setup(start_thread=False)
FD = FM._fd
check("the bot alone: WU_DISCORD_GUILD without forum channels is enough", L is not None and L.growth is not None, FD and FD.Config(ENV).problems())
check("a bad WU_DISCORD_GUILD is refused", any("WU_DISCORD_GUILD" in p for p in FD.Config(dict(ENV, WU_DISCORD_GUILD="abc")).problems()))
G = L.growth
DGM = importlib.util.module_from_spec(importlib.util.spec_from_file_location("dgm", os.path.join(STREAM, "discord_growth.py")))
importlib.util.spec_from_file_location("dgm", os.path.join(STREAM, "discord_growth.py")).loader.exec_module(DGM)
NOW = [2_000_000_000]
L.clock = lambda: NOW[0]
L.api.MIN_GAP = 0
L.api._sleep = lambda s: None
L.round()
check("start: the bot and the server from WU_DISCORD_GUILD", L.bot_id == BOT and L.guild == GUILD)
check("every part off by default", not (G.cfg.feed or G.cfg.lfg or G.cfg.roles or G.cfg.automod))
calls0 = len(D["calls"])
G.next_check = G.next_feed = 0
L.round()
check("off: a round of the worker makes no Discord call of ours", all("/threads/active" in c[1] for c in D["calls"][calls0:]), D["calls"][calls0:])

# ------------------------------------------------------------------------------------------- the template
T = json.load(open(os.path.join(STREAM, "discord_template.json"), encoding="utf-8"))
check("the shipped template is valid", DGM.check_template(T) == [], DGM.check_template(T))
names = {c["name"] for cat in T["categories"] for c in cat["channels"]}
check("the template has the plan's channels", {"welcome", "rules", "roles", "announcements", "looking-for-group", "live-realms",
                                                "den-showcase", "modes-and-forks", "list-your-realm", "bug-reports", "Dalaran lounge"} <= names)
check("... and the plan's roles", [r["name"] for r in T["roles"]] == ["Core Team", "Moderator", "Guide", "Realm Host", "Mode Maker",
                                                                       "Builder", "Den Architect", "Node Keeper"])
check("staff roles are manual, the rest automatic", all(bool(r.get("staff")) != bool(r.get("auto")) for r in T["roles"]))
bad = DGM.check_template({"format": 1, "roles": [{"key": "x", "name": "X", "auto": "nope"}],
                          "categories": [{"key": "c", "name": "C", "channels": [{"key": "a", "name": "a", "private": ["ghost"]}]}],
                          "posts": {"live": "zzz"}})
check("a bad template: its problems", len(bad) == 3, bad)
check("a private channel whose role is missing is never made", DGM.overwrites({"private": ["core_team"]}, GUILD, BOT, {}) is None)
ow = DGM.overwrites({"read_only": True, "posters": ["moderator"]}, GUILD, BOT, {"moderator": "55555"})
check("a read-only channel: everyone can't send, the posters and the bot can",
      ow[0]["id"] == GUILD and int(ow[0]["deny"]) & DGM.P_SEND and any(o["id"] == "55555" and int(o["allow"]) & DGM.P_SEND for o in ow)
      and any(o["id"] == BOT and o["type"] == 1 and int(o["allow"]) & DGM.P_SEND for o in ow), ow)
check("the bot's permission integers", DGM.BOT_PERMS == 326686026768 and DGM.BOT_PERMS_AUTOMOD == 326686026800, (DGM.BOT_PERMS, DGM.BOT_PERMS_AUTOMOD))

# ------------------------------------------------------------------------------------------- setup
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.setup", "target": "plan", "confirm": "setup"})
check("setup: owner only", not ok and "owner" in msg.lower(), msg)
ok, msg = wp.admin.run_action(OWNER, {"action": "discord.setup", "target": "plan", "confirm": "yes"})
check("setup: the typed confirm", not ok and "setup" in msg, msg)
n0 = len(D["calls"])
ok, msg = wp.admin.run_action(OWNER, {"action": "discord.preview", "target": "plan"})
pv = G.jmeta("preview")
check("Preview: what it would make, nothing made", ok and pv and pv["dry"] and "role Realm Host" in pv["made"] and "role Moderator" in pv["found"]
      and "category Start here" in pv["found"] and "channel rules" in pv["found"] and "channel live-realms" in pv["made"]
      and all(c[0] == "GET" for c in D["calls"][n0:]), (msg, pv))
ok, msg = wp.admin.run_action(OWNER, {"action": "discord.setup", "target": "plan", "confirm": "setup"})
check("setup: queued for the worker", ok and any(r[0] == "dg.setup" for r in F.q("SELECT kind FROM fd_outbox")), msg)
n0 = len(D["calls"])
L.drain()
rep = G.jmeta("setup")
made = D["calls"][n0:]
check("setup ran: roles, categories and channels made", rep and not rep["dry"] and "role Core Team" in rep["made"] and "category Play" in rep["made"]
      and "channel live-realms" in rep["made"] and "channel Dalaran lounge" in rep["made"], rep)
check("setup: what was there is left alone (found, not made again)", "role Moderator" in rep["found"] and "channel rules" in rep["found"]
      and sum(1 for r in D["roles"].values() if r["name"] == "Moderator") == 1 and sum(1 for c in D["channels"].values() if c["name"] == "rules") == 1)
check("setup never deletes or changes anything", not any(c[0] in ("DELETE", "PATCH", "PUT") for c in made))
check("setup: a stage refused (no Community) is noted, the rest goes on",
      any(a == "stage Showcase stage" and "Community" in b for a, b in rep["failed"]) and G.mapped("channel", "mode_jam"))
byname = {c["name"]: c for c in D["channels"].values()}
lr, cat_play = byname["live-realms"], byname["Play"]
check("live-realms: in Play, read-only for everyone, the bot may post", lr["parent_id"] == cat_play["id"]
      and any(o["id"] == GUILD and int(o["deny"]) & DGM.P_SEND for o in lr["permission_overwrites"])
      and any(o["id"] == BOT and o["type"] == 1 for o in lr["permission_overwrites"]))
rd = byname["roadmap"]
core = next(r["id"] for r in D["roles"].values() if r["name"] == "Core Team")
check("roadmap: hidden from everyone, seen by Core Team", any(o["id"] == GUILD and int(o["deny"]) & DGM.P_VIEW for o in rd["permission_overwrites"])
      and any(o["id"] == core and int(o["allow"]) & DGM.P_VIEW for o in rd["permission_overwrites"]))
check("list-your-realm: the 10-minute slowmode; topics set", byname["list-your-realm"].get("rate_limit_per_user") == 600
      and "Find a group" in (byname["looking-for-group"].get("topic") or ""))
check("the made roles have no powers and nobody can ping them", all(r.get("permissions") == "0" and r.get("mentionable") is False
                                                                     for r in D["roles"].values() if r["name"] not in ("@everyone", "Moderator")))
check("announcements: Core Team and Moderator may post", any(o["id"] == MOD_ROLE and int(o["allow"]) & DGM.P_SEND
                                                             for o in byname["announcements"]["permission_overwrites"]))
check("setup is in HQ's audit log", any(r.get("action") == "discord.setup" for r in wp.admin.audit_tail(50)))
n_roles, n_chans = len(D["roles"]), len(D["channels"])
wp.admin.run_action(OWNER, {"action": "discord.setup", "target": "plan", "confirm": "setup"})
L.drain()
check("setup again: nothing new (idempotent)", len(D["roles"]) == n_roles and len(D["channels"]) == n_chans
      and G.jmeta("setup")["made"] == [] and len(G.jmeta("setup")["failed"]) == 1)
with lock:
    D["fail"].append((r"POST /guilds/\d+/channels", 503, {"message": "down"}))
D["community"] = True
wp.admin.run_action(OWNER, {"action": "discord.setup", "target": "plan", "confirm": "setup"})
L.drain()
check("Discord down during setup: kept in the outbox to try again", any(r[0] == "dg.setup" for r in F.q("SELECT kind FROM fd_outbox")))
NOW[0] += 3600
L.drain()
check("... and done later (the stage too, once the server is a Community)", not F.q("SELECT 1 FROM fd_outbox WHERE kind = 'dg.setup'")
      and "stage Showcase stage" in G.jmeta("setup")["made"])

# ------------------------------------------------------------------------------------------- live realms
REALMS = []
wp.community_realms = lambda: ([dict(r) for r in REALMS], True)


def realm(rid, name, players=0, **kw):
    return dict({"id": rid, "name": name, "live": True, "players": players, "ruleset": "brawl", "build": 12340, "name_ok": True,
                 "relay": "relay.example", "first_seen": "2026-10-01T00:00:00Z"}, **kw)


def feed():
    G.next_feed = 0
    G.feed_tick()
    L.drain()


def lr_msgs():
    return D["msgs"].get(G.mapped("channel", "live_realms"), [])


REALMS[:] = [realm("aaaaaaaaaaaaaaaa", "Early Bird", 3)]
G.next_feed = 0
G.tick()
L.drain()
check("feed off: nothing tracked or posted", not F.q("SELECT 1 FROM dg_realms") and not lr_msgs())
G.cfg = DGM.Config(dict(ENV, WU_DISCORD_FEED="1"))
feed()
check("feed on: realms already live are not announced", not lr_msgs() and F.q("SELECT 1 FROM dg_realms"))
NOW[0] += 60
REALMS.append(realm("bbbbbbbbbbbbbbbb", "Goblin Brawl", 7, tagline="Fight *now*", short="goblins"))
feed()
m = lr_msgs()
check("a new realm: a card in live-realms (name, mode, players, its /r/ link)", len(m) == 1 and "**Goblin Brawl** is live" in m[0]["content"]
      and "Brawl\\: everyone 80" in m[0]["content"] and "7 playing" in m[0]["content"] and "https://play.example/r/goblins" in m[0]["content"]
      and m[0]["components"][0]["components"][0]["url"] == "https://play.example/r/goblins", m)
check("the card: no pings, no previews, markdown typed by the host shown as typed", m[0]["allowed_mentions"] == {"parse": []}
      and m[0]["flags"] == 4 and "Fight \\*now\\*" in m[0]["content"], m[0])
NOW[0] += 60
REALMS.append(realm("cccccccccccccccc", "No Link Realm", 0))
feed()
check("no short link: the realm's join link", "https://relay.example/join/cccccccccccccccc" in lr_msgs()[-1]["content"]
      and "playing" not in lr_msgs()[-1]["content"])
NOW[0] += 60
REALMS.append(realm("dddddddddddddddd", "Warcraft Fans", 9))
REALMS.append(realm("eeeeeeeeeeeeeeee", "Name Flagged", 9, name_ok=False))
feed()
check("a realm named with the game's name, or flagged by the relay: never posted", len(lr_msgs()) == 2)
NOW[0] += 60
for i, (rid, pl) in enumerate([("ffffffffffffffff", 1), ("gggggggggggggggg", 50), ("hhhhhhhhhhhhhhhh", 5)]):
    REALMS.append(realm(rid, f"Realm {i}", pl))
feed()
names = [x["content"].split("**")[1] for x in lr_msgs()]
check("at most WU_DISCORD_FEED_PER_HOUR an hour (4): the busiest first", len(lr_msgs()) == 4 and names[-2:] == ["Realm 1", "Realm 2"], names)
NOW[0] += 3700
feed()
check("a card held back more than an hour is dropped, not posted late", len(lr_msgs()) == 4)
REALMS[:] = [r for r in REALMS if r["id"] != "bbbbbbbbbbbbbbbb"]
feed()
NOW[0] += 3 * 3600
for _ in range(1):
    feed()
REALMS.append(realm("bbbbbbbbbbbbbbbb", "Goblin Brawl", 2, short="goblins"))
feed()
check("back after 3 hours: not announced again", len(lr_msgs()) == 4)
REALMS[:] = [r for r in REALMS if r["id"] != "bbbbbbbbbbbbbbbb"]
for _ in range(7):
    NOW[0] += 3600
    feed()
REALMS.append(realm("bbbbbbbbbbbbbbbb", "Goblin Brawl", 2, short="goblins"))
feed()
check("back after 6 hours away: announced again", len(lr_msgs()) == 5 and "Goblin Brawl" in lr_msgs()[-1]["content"])
REALMS[:] = [r for r in REALMS if r["id"] != "cccccccccccccccc"]
feed()
NOW[0] += 8 * 3600   # the portal was down (no ticks)
REALMS.append(realm("cccccccccccccccc", "No Link Realm", 0))
REALMS.append(realm("iiiiiiiiiiiiiiii", "Brand New", 4))
feed()
check("the portal away for hours: no flood of cards when it comes back", len(lr_msgs()) == 5)
wp.admin.set_realm_override("jjjjjjjjjjjjjjjj", {"hidden": True})
NOW[0] += 60
REALMS.append(realm("jjjjjjjjjjjjjjjj", "Hidden One", 40))
wp.community_realms = lambda: (wp.admin.apply_realm_overrides([dict(r) for r in REALMS]), True)
feed()
check("a realm staff hid: never posted", len(lr_msgs()) == 5)

# ------------------------------------------------------------------------------------------- staff picks
ok, msg = wp.admin.run_action(MOD, {"action": "discord.den", "target": "pick", "acct": "WR000001", "title": "Glade"})
check("staff picks: admins and the owner", not ok)
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.den", "target": "pick", "acct": "WR000001", "title": "Mossy Glade",
                                      "link": "/wrath/leyline/dens/7", "line": "A stream, a cottage, warm light."})
L.drain()
den = D["msgs"].get(G.mapped("channel", "den_showcase"), [])
check("Den of the week: posted in den-showcase with its link", ok and len(den) == 1 and "**Den of the week:** Mossy Glade by **" in den[0]["content"]
      and den[0]["components"][0]["components"][0]["url"] == "https://play.example/wrath/leyline/dens/7", (msg, den))
check("... and its maker has the Den Architect fact", F.one("SELECT source FROM dg_facts WHERE acct = 'WR000001' AND fact = 'den_architect'"))
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.mode", "target": "pick", "title": "Goblin Golf", "link": "https://evil.example/x"})
check("a link off our site: refused", not ok and "our own site" in msg, msg)
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.mode", "target": "pick", "title": "Warcraft Golf"})
check("the game's name: refused", not ok and "game's name" in msg, msg)
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.mode", "target": "pick", "title": "Goblin Golf", "acct": "WR000002",
                                      "link": "https://play.example/wrath?go=w:12"})
L.drain()
md = D["msgs"].get(G.mapped("channel", "modes_and_forks"), [])
check("the featured mode: posted in modes-and-forks", ok and len(md) == 1 and "**Featured mode this week:** Goblin Golf by **" in md[0]["content"], md)
G.cfg = DGM.Config(ENV)
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.den", "target": "pick", "acct": "WR000002", "title": "Dark Keep"})
L.drain()
check("feed off: the pick is recorded, not posted", ok and len(D["msgs"].get(G.mapped("channel", "den_showcase"), [])) == 1
      and G.posts(1)[0]["status"].startswith("not posted"))


# ------------------------------------------------------------------------------------------- Find a group
def req(method, path, who=None, form=None, headers=None, raw=None):
    c = http.client.HTTPConnection("127.0.0.1", PAGE, timeout=30)
    h = {"X-Forwarded-For": "10.0.0.7", **(headers or {})}
    if who:
        h["Cookie"] = f"wr_session={who}"
    body = raw
    if form is not None:
        body = urllib.parse.urlencode(form).encode()
        h.update({"Content-Type": "application/x-www-form-urlencoded", "Origin": "https://play.example", "Sec-Fetch-Site": "same-origin"})
    c.request(method, path, body=body, headers=h)
    r = c.getresponse()
    out = (r.status, dict(r.getheaders()), r.read().decode("utf-8", "replace"))
    c.close()
    return out


DPAGE = "/wrath/leyline/forum/discord"


def tok(who):
    _, _, body = req("GET", DPAGE, who)
    m = re.search(r'name="t" value="([0-9a-f]{32})"', body)
    return m.group(1) if m else ""


def lfg_msgs():
    return D["msgs"].get(G.mapped("channel", "looking_for_group"), [])


L.do_link("WR000001", "600000000000000001", "alice#1")
L.do_link("WR000002", "600000000000000002", "bob#2")
st, _, body = req("GET", DPAGE, "alice")
check("Find a group off: no box on the Discord page", st == 200 and "Find a group" not in body)
try:
    G.group("WR000001", what="a dungeon")
    raised = ""
except FM.ForumError as ex:
    raised = (ex.status, ex.text)
check("Find a group off: refused", raised and raised[0] == 404, raised)
G.cfg = DGM.Config(dict(ENV, WU_DISCORD_LFG="1"))
st, _, body = req("GET", DPAGE + "?group=bbbbbbbbbbbbbbbb", "alice")
check("on: the box, with the live realms (the shared one picked)", "Find a group" in body
      and 'value="bbbbbbbbbbbbbbbb" selected' in body and "Hidden One" not in body, body[-3000:])
st, _, body = req("GET", DPAGE, "carl")
check("an unlinked account sees no box (it links first)", "Find a group" not in body)
st, _, body = req("POST", DPAGE, "alice", {"t": tok("alice"), "do": "group", "realm": "bbbbbbbbbbbbbbbb", "note": "need a healer"})
m = lfg_msgs()
check("the website: '<name> wants a group for <realm>' with a Join button, in looking-for-group",
      st == 200 and "Posted in looking-for-group" in body and len(m) == 0)
L.drain()
m = lfg_msgs()
check("... delivered: the name, the realm and its mode, their Discord mention (no ping), the note, Join",
      len(m) == 1 and "**Alice** wants a group for **Goblin Brawl \\(Brawl\\: everyone 80\\)**" in m[0]["content"] and "<@600000000000000001>" in m[0]["content"]
      and "need a healer" in m[0]["content"] and m[0]["allowed_mentions"] == {"parse": []}
      and m[0]["components"][0]["components"][0]["url"] == "https://play.example/r/goblins", m)
st, _, body = req("POST", DPAGE, "alice", {"t": tok("alice"), "do": "group", "what": "a dungeon"})
check("again within 30 minutes: refused, plainly", st == 429 and "a moment ago" in body)
try:
    G.group("WR000004", what="a dungeon")
    raised = ""
except FM.ForumError as ex:
    raised = (ex.status, ex.text)
check("unlinked: told to link first", raised and raised[0] == 403 and "Link your Discord" in raised[1], raised)
for note, why in (("you shitface", "friendly"), ("best wow realm", "game's name")):
    try:
        G.group("WR000002", what="a raid", note=note)
        raised = ""
    except FM.ForumError as ex:
        raised = ex.text
    check(f"the filter refuses a note ({why})", why in raised, raised)
try:
    G.group("WR000002", realm="zzzzzzzzzzzzzzzz")
    raised = ""
except FM.ForumError as ex:
    raised = ex.text
check("a realm that isn't live: refused", "isn't live" in raised, raised)
S = wp.social.store
d = S.device_start("10.9.9.9")
S.device_answer(S.device_lookup(d["user_code"]), "WR000002", True)
btok = S.device_poll(d["device_code"])["token"]
st, _, body = req("POST", "/wrath/forum/api/discord/group", raw=json.dumps({"what": "arena 2v2", "note": "anyone?"}).encode(),
                  headers={"Authorization": "Bearer " + btok, "Content-Type": "application/json"})
L.drain()
check("the launcher: POST /wrath/forum/api/discord/group with its Bearer token", st == 200 and json.loads(body)["ok"]
      and "wants a group for **arena 2v2**" in lfg_msgs()[-1]["content"] and "[Join]" not in lfg_msgs()[-1]["content"], (st, body))
st, _, body = req("POST", "/wrath/forum/api/discord/group", "alice", raw=json.dumps({"what": "x"}).encode(),
                  headers={"Content-Type": "application/json"})
check("the launcher call takes a Bearer token only (no cookie)", st == 401, (st, body))
G.cfg = DGM.Config(dict(ENV, WU_DISCORD_LFG="1", WU_DISCORD_LFG_PER_HOUR="2"))
NOW[0] += 3600 + 60
L.do_link("WR000004", "600000000000000004", "carl")
G.group("WR000001", what="one")
G.group("WR000002", what="two")
try:
    G.group("WR000004", what="three")
    raised = ""
except FM.ForumError as ex:
    raised = ex.text
check("WU_DISCORD_LFG_PER_HOUR for everyone", "Lots of people" in raised, raised)
L.drain()

# ------------------------------------------------------------------------------------------- roles from accounts
G.cfg = DGM.Config(dict(ENV, WU_DISCORD_ROLES="1"))
rid_of = lambda key: G.mapped("role", key)   # noqa: E731
D["members"]["600000000000000001"] = {"user": {"id": "600000000000000001"}, "roles": []}
D["members"]["600000000000000002"] = {"user": {"id": "600000000000000002"}, "roles": [rid_of("builder")]}   # staff gave it by hand
# carl (WR000004) is linked but not in the server
hf = wp.hostflow
hf.save({"pending_id": "kkkkkkkkkkkkkkkk", "realm_id": "kkkkkkkkkkkkkkkk", "owner": "WR000002", "created": 1})
REALMS.append(realm("kkkkkkkkkkkkkkkk", "Bob's Realm", 1, first_seen="2026-01-01T00:00:00Z"))
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.fact.add", "target": "", "acct": "wr000001", "fact": "builder", "note": "approved"})
check("a fact recorded in HQ", ok and F.one("SELECT 1 FROM dg_facts WHERE acct = 'WR000001' AND fact = 'builder'"), msg)
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.fact.add", "target": "", "acct": "WR000001", "fact": "realm_host"})
check("Realm Host can't be recorded by hand (it is counted)", not ok, msg)
check("Realm Host: the owner of a realm listed 7 days", G.hosts() == {"WR000002"}, G.hosts())
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.roles.run", "target": "now"})
n0 = len(D["calls"])
L.drain()
r1 = G.jmeta("roles_last")
check("the sync: Alice gets Builder and Den Architect, Bob Realm Host", ok and set(D["members"]["600000000000000001"]["roles"]) == {rid_of("builder"), rid_of("den_architect")}
      and rid_of("realm_host") in D["members"]["600000000000000002"]["roles"], (r1, D["members"]))
check("... a linked person not in the server is skipped", r1["absent"] == 1 and r1["people"] == 2, r1)
check("... Den Architect for Bob too (the Dark Keep pick); all in HQ's audit log", rid_of("den_architect") in D["members"]["600000000000000002"]["roles"]
      and sum(1 for r in wp.admin.audit_tail(100) if r.get("action") == "discord.role.add") == 4)
n0 = len(D["calls"])
G.sync_roles()
check("again: nothing to change (idempotent)", not any(c[0] in ("PUT", "DELETE") for c in D["calls"][n0:]))
G.drop_fact("WR000001", "builder")
REALMS[:] = [r for r in REALMS if r["id"] != "kkkkkkkkkkkkkkkk"]
G.sync_roles()
check("a fact gone: the role the bot gave is taken back; Den Architect kept",
      set(D["members"]["600000000000000001"]["roles"]) == {rid_of("den_architect")}, D["members"]["600000000000000001"])
check("Bob stops hosting: Realm Host taken back; the Builder role staff gave by hand stays",
      set(D["members"]["600000000000000002"]["roles"]) == {rid_of("builder"), rid_of("den_architect")}, D["members"]["600000000000000002"])
L.unlink(acct="WR000001")
G.sync_roles()
check("unlinked: the bot's roles taken back (even Den Architect)", D["members"]["600000000000000001"]["roles"] == [])
G.record_fact("WR000002", "node_keeper", "runs a node")
G.record_fact("WR000004", "mode_maker", "Goblin Golf")
D["members"]["600000000000000004"] = {"user": {"id": "600000000000000004"}, "roles": []}
with lock:
    D["fail"].append((r"GET /guilds/\d+/members/600000000000000004", 502, {"message": "bad gateway"}))
try:
    G.sync_roles()
    raised = False
except FD.DiscordError:
    raised = True
check("Discord fails mid-way: the sync stops, its place kept", raised and G.meta("roles_cursor") == "600000000000000002"
      and rid_of("node_keeper") in D["members"]["600000000000000002"]["roles"])
n0 = len(D["calls"])
G.sync_roles()
check("... and goes on from there", D["members"]["600000000000000004"]["roles"] == [rid_of("mode_maker")] and G.meta("roles_cursor") == ""
      and not any("600000000000000002" in c[1] for c in D["calls"][n0:]))
NOW[0] = (NOW[0] // 86400) * 86400 + 4 * 3600 + 120
G.meta("roles_at", str(NOW[0] - 86400))
G.next_check = 0
G.tick()
check("nightly: queued at WU_DISCORD_ROLES_HOUR", F.q("SELECT 1 FROM fd_outbox WHERE kind = 'dg.roles'"))
L.drain()
G.next_check = 0
G.tick()
check("... once a night", not F.q("SELECT 1 FROM fd_outbox WHERE kind = 'dg.roles'"))

# ------------------------------------------------------------------------------------------- AutoMod
G.next_check = 0
G.tick()
check("AutoMod off: no rule", not D["rules"] and not F.q("SELECT 1 FROM fd_outbox WHERE kind = 'dg.automod'"))
ok, msg = wp.admin.run_action(ADMIN, {"action": "discord.automod.push", "target": "now"})
check("AutoMod off: Push now refused", not ok and "WU_DISCORD_AUTOMOD" in msg)
G.cfg = DGM.Config(dict(ENV, WU_DISCORD_AUTOMOD="1"))
with lock:
    D["fail"].append((r"POST /guilds/\d+/auto-moderation", 403, {"message": "Missing Permissions", "code": 50013}))
G.next_check = 0
G.tick()
L.drain()
check("a 403: explained (Manage Server)", "Manage Server" in (G.jmeta("automod") or {}).get("error", ""), G.jmeta("automod"))
G.next_check = 0
G.tick()
L.drain()
rule = next(iter(D["rules"].values()), {})
kw = rule.get("trigger_metadata", {}).get("keyword_filter", [])
check("on: one keyword rule with our blocked words", rule.get("name") == "Leyline blocked words" and rule.get("trigger_type") == 1
      and "shit" in kw and "*fuck*" in kw and rule["actions"][0]["type"] == 1, rule)
check("... names-only rules (staff words, the game's name) are not in it", "admin" not in kw and "warcraft" not in kw)
check("... the allow words carried", len(rule["trigger_metadata"]["allow_list"]) > 0)
n0 = len(D["calls"])
G.next_check = 0
G.tick()
L.drain()
check("the same words: not pushed again", not any("auto-moderation" in c[1] for c in D["calls"][n0:]))
rule["enabled"] = False
wp.admin.run_action(ADMIN, {"action": "words.add", "target": "", "word": "grobble", "mode": "word"})
G.next_check = 0
G.tick()
L.drain()
rule = next(iter(D["rules"].values()))
check("a new blocked word: the rule updated (not a second one), Discord's off switch kept", len(D["rules"]) == 1
      and "grobble" in rule["trigger_metadata"]["keyword_filter"] and rule["enabled"] is False, rule)

# ------------------------------------------------------------------------------------------- HQ
st, title, body = HQ.page(ADMIN, "discord", {})
check("HQ's Discord tab: on and off, setup, picks, roles, AutoMod, the last posts", st == 200 and "Set up the Discord" in body
      and "WU_DISCORD_FEED" in body and "Last posts" in body and "wants a group" in body and "Record a fact" in body
      and "Push the words now" in body, body[:500])
check("... Set up the Discord shown to the owner only", 'value="discord.setup"' not in body and 'value="discord.setup"' in HQ.page(OWNER, "discord", {})[2])
with lock:
    D["fail"].append((r"POST /channels/\d+/messages", 403, {"message": "Missing Access", "code": 50001}))
G.cfg = DGM.Config(dict(ENV, WU_DISCORD_FEED="1"))
feed()
NOW[0] += 60
REALMS.append(realm("llllllllllllllll", "Fresh Realm", 2))
feed()
st, title, body = HQ.page(ADMIN, "discord", {})
check("a post Discord refused: its state and error in HQ", "retrying" in body and "forbidden" in body, body[-2000:])
check("the token never reached a log line, the status or HQ", TOKEN not in logbuf.getvalue() and TOKEN not in json.dumps(G.status(), default=str)
      and TOKEN not in body)
check("every call carries the bot token in the Authorization header only", D["auth"] == {"Bot " + TOKEN})

# ------------------------------------------------------------------------------------------- the deploy step
bat = open(os.path.join(ROOT, "stream", "Update sign-in and warm-up.bat"), encoding="utf-8").read()
sh = open(os.path.join(ROOT, "stream", "fastloop.sh"), encoding="utf-8").read()
check("the signin step ships discord_growth.py and discord_template.json; fastloop.sh installs them, with a backup",
      "LINKS=%LINKS% discord_growth.py discord_template.json" in bat and "install -m 644 discord_growth.py discord_template.json" in sh
      and 'cp -p "$WU/portal/$f" "$BK/$f"; done' in sh)

print(f"{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
