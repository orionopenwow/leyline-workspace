"""The forum <-> Discord link (stream/forum_discord.py) through the real portal (stream/wrath_portal.py), the real forum,
social.py and admin.py, against a fake website sign-in and a FAKE DISCORD API (a local HTTP server that keeps
channels, threads and messages in memory and can fail on demand). No network, no real token.

Covers: off by default and off with incomplete settings; the token never in a log, the repr or the status; start
(channel types, /link and /unlink registered); forum -> Discord (a new thread in a Forum channel with its tag, in a
text channel as a thread, replies with name and link, markdown and pings defused, unmapped categories stay out);
linking (/link signed with Ed25519: ephemeral answer, bad / old signature refused, PING; the link page signed in,
a used code refused, signed out goes to sign-in; unlink; the website's own Discord sign-in); Discord -> forum
(a linked reply shown "via Discord · <name>", not echoed back; unlinked: one note by DM, message removed; DMs closed:
a mention-only note deleted after 2 minutes; a rate limit, a word-filter change re-posted as the forum shows it, an
HQ forum mute and an account ban refused with a note; Discord threads by linked / unlinked people; threads from
before the link ignored); moderation both ways (hide / show a post, ours edited and a Discord person's deleted and
re-posted; lock / unlock; hide / show a thread: deleted and made again; merge: note, locked, archived; deleted in
Discord: a message (count drop), the first post (sweep) and a whole thread hide it on the forum, in HQ's audit log);
reliability (Discord down: the outbox keeps everything in order and retries with back-off; a short 429 waited out, a
long one handed back; X-RateLimit-Remaining 0 obeyed without a call; a 400 dropped; a 401 pauses with nothing lost);
and the deploy step ships the module.

    python3 -I stream/tests/test_forum_discord.py stream/wrath_portal.py
"""
import http.client, importlib.util, io, itertools, json, logging, os, re, sys, tempfile, threading, time, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "stream", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
TMP = tempfile.mkdtemp(prefix="wu-fd-")
TOKEN = "fake.bot.token-" + "Q" * 30
fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra!r}"[:700] if extra and not cond else ""))
    fails += 0 if cond else 1


# ------------------------------------------------------------------------------------------- the fake website
ACCOUNTS = {"alice": ("alice", "Alice", "WR000001", ""), "bob": ("bob", "Bob", "WR000002", ""),
            "staff1": ("staff1", "Mod Sam", "WR000003", ""), "dana": ("dana", "Dana", "WR000005", "60000000000000005")}


class Site(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_GET(self):
        m = re.search(r"wr_session=(\w+)", self.headers.get("Cookie", ""))
        a = ACCOUNTS.get(m.group(1)) if m else None
        if not a:
            self.send_response(401); self.send_header("Content-Length", "0"); self.end_headers(); return
        me = {"id": a[0], "display_name": a[1]}
        if a[3]:
            me["discord_id"] = a[3]
        doc = {"/api/me": me, "/api/play": {"user": a[2], "pass": "x" * 16}}[self.path]
        b = json.dumps(doc).encode()
        self.send_response(200); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)


# ------------------------------------------------------------------------------------------- the fake Discord
EPOCH = 1420070400000
_seq = itertools.count(1)


def snow(ms=None):
    ms = int(time.time() * 1000) if ms is None else ms
    return str(((ms - EPOCH) << 22) + next(_seq))


BOT = "900000000000000001"
GUILD = "770000000000000001"
CH_HELP, CH_GENERAL, CH_NEWS, CH_DENS = "100000000000000001", "100000000000000002", "100000000000000003", "100000000000000004"
D = {"channels": {CH_HELP: {"id": CH_HELP, "type": 15, "guild_id": GUILD}, CH_GENERAL: {"id": CH_GENERAL, "type": 0, "guild_id": GUILD},
                  CH_NEWS: {"id": CH_NEWS, "type": 15, "guild_id": GUILD}, CH_DENS: {"id": CH_DENS, "type": 15, "guild_id": GUILD}},
     "threads": {}, "msgs": {}, "dms": {}, "closed_dms": set(), "commands": None, "calls": [], "auth": set(),
     "fail": [], "headers": {}}
lock = threading.Lock()


def new_msg(channel, author, content, **kw):
    mid = kw.pop("id", None) or snow()
    m = {"id": mid, "channel_id": channel, "author": author, "content": content, "type": 0, "mentions": [], "attachments": []}
    m.update(kw)
    D["msgs"].setdefault(channel, {})[mid] = m
    t = D["threads"].get(channel)
    if t:
        t["last_message_id"] = mid
        t["message_count"] = sum(1 for x in D["msgs"][channel] if x != channel)
    return m


def make_thread(parent, name, owner, starter=None, **kw):
    tid = kw.pop("id", None) or snow()
    t = {"id": tid, "parent_id": parent, "guild_id": GUILD, "name": name, "owner_id": owner, "type": 11,
         "message_count": 0, "last_message_id": tid, "thread_metadata": {"locked": False, "archived": False}, **kw}
    D["threads"][tid] = t
    D["msgs"][tid] = {}
    if starter is not None:
        new_msg(tid, starter["author"], starter["content"], id=tid)
    return t


def user(uid, name, bot=False):
    return {"id": uid, "username": name.lower(), "global_name": name, **({"bot": True} if bot else {})}


BOTU = user(BOT, "Leyline", True)


class Discord(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def reply(self, status, doc=None, headers=None):
        b = json.dumps(doc).encode() if doc is not None else b""
        self.send_response(status)
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def handle_any(self, method):
        n = int(self.headers.get("Content-Length") or 0)
        body = json.loads(self.rfile.read(n)) if n else None
        u = urllib.parse.urlsplit(self.path)
        path, q = u.path.replace("/api/v10", "", 1), dict(urllib.parse.parse_qsl(u.query))
        with lock:
            D["auth"].add(self.headers.get("Authorization", ""))
            D["calls"].append((method, path, body))
            if D["fail"]:
                f = D["fail"].pop(0)
                return self.reply(f[0], f[1], f[2] if len(f) > 2 else None)
            return self.route(method, path, q, body)

    def route(self, method, path, q, body):
        hdr = D["headers"].pop(path, None)
        ok = lambda doc=None, st=200: self.reply(st, doc, hdr)   # noqa: E731
        if path == "/users/@me":
            return ok({"id": BOT, "username": "Leyline"})
        m = re.fullmatch(r"/applications/(\d+)/guilds/(\d+)/commands", path)
        if m and method == "PUT":
            D["commands"] = body
            return ok(body)
        if path == f"/guilds/{GUILD}/threads/active":
            return ok({"threads": [t for t in D["threads"].values() if not t["thread_metadata"]["archived"]], "members": []})
        if path == "/users/@me/channels" and method == "POST":
            ch = "dm" + body["recipient_id"]
            D["dms"].setdefault(ch, [])
            return ok({"id": ch, "type": 1})
        m = re.fullmatch(r"/channels/([\w]+)(/.*)?", path)
        if not m:
            return self.reply(404, {"message": "Unknown", "code": 0})
        cid, rest = m.group(1), m.group(2) or ""
        if cid.startswith("dm"):
            if cid[2:] in D["closed_dms"]:
                return self.reply(403, {"message": "Cannot send messages to this user", "code": 50007})
            D["dms"][cid].append(body["content"])
            return ok({"id": snow(), "content": body["content"]})
        if rest == "" and method == "GET":
            c = D["channels"].get(cid) or D["threads"].get(cid)
            return ok(c) if c else self.reply(404, {"message": "Unknown Channel", "code": 10003})
        if rest == "" and method == "PATCH":
            t = D["threads"].get(cid)
            if not t:
                return self.reply(404, {"message": "Unknown Channel", "code": 10003})
            for k in ("locked", "archived"):
                if k in body:
                    t["thread_metadata"][k] = body[k]
            return ok(t)
        if rest == "" and method == "DELETE":
            if D["threads"].pop(cid, None) is None:
                return self.reply(404, {"message": "Unknown Channel", "code": 10003})
            D["msgs"].pop(cid, None)
            return ok(None, 204) if False else self.reply(204)
        if rest == "/threads" and method == "POST":
            ch = D["channels"][cid]
            if ch["type"] == 15:
                t = make_thread(cid, body["name"], BOT, applied_tags=body.get("applied_tags", []))
                msg = new_msg(t["id"], BOTU, body["message"]["content"], id=t["id"], allowed_mentions=body["message"]["allowed_mentions"],
                              flags=body["message"].get("flags"))
                return ok(dict(t, message=msg), 201)
            t = make_thread(cid, body["name"], BOT)
            return ok(t, 201)
        if cid not in D["msgs"]:
            return self.reply(404, {"message": "Unknown Channel", "code": 10003})
        if rest == "/messages" and method == "POST":
            m2 = new_msg(cid, BOTU, body["content"], allowed_mentions=body.get("allowed_mentions"), flags=body.get("flags"))
            return ok(m2)
        if rest == "/messages" and method == "GET":
            ms = sorted(D["msgs"][cid].values(), key=lambda x: int(x["id"]))
            if "after" in q:
                ms = [x for x in ms if int(x["id"]) > int(q["after"])][:int(q.get("limit", 50))]
            else:
                if "before" in q:
                    ms = [x for x in ms if int(x["id"]) < int(q["before"])]
                ms = ms[-int(q.get("limit", 50)):][::-1]
            return ok(ms)
        m = re.fullmatch(r"/messages/(\d+)", rest)
        if m:
            mid = m.group(1)
            msg = D["msgs"][cid].get(mid)
            if not msg:
                return self.reply(404, {"message": "Unknown Message", "code": 10008})
            if method == "GET":
                return ok(msg)
            if method == "PATCH":
                msg["content"] = body["content"]
                return ok(msg)
            if method == "DELETE":
                del D["msgs"][cid][mid]
                t = D["threads"].get(cid)
                if t:
                    t["message_count"] = sum(1 for x in D["msgs"][cid] if x != cid)
                return self.reply(204)
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

# ------------------------------------------------------------------------------------------- the portal
logbuf = io.StringIO()
lh = logging.StreamHandler(logbuf)
lh.setLevel(logging.DEBUG)
logging.getLogger().addHandler(lh)
logging.getLogger().setLevel(logging.INFO)

ed = importlib.util.spec_from_file_location("ed", os.path.join(STREAM, "ed25519_pure.py"))
ED = importlib.util.module_from_spec(ed); ed.loader.exec_module(ED)
SK = b"\x07" * 32
PK = ED.public_key(SK).hex()
APP = "880000000000000001"
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL=f"http://127.0.0.1:{SITE}",
                  WU_STREAM="0", STREAM_HOST="wrath.example", WU_PORTAL_DATA=TMP, WU_HOST_DIR=os.path.join(TMP, "hosted"),
                  WU_RELAY_LISTING="http://127.0.0.1:9/none", WU_ADMIN_STAFF="staff1", WU_ADMIN_OWNERS="", WU_ADMIN_GM_LEVEL="0",
                  PUBLIC_HOST="play.example", WU_HUB_FRONT="0")
for k in ("WU_FORM_SECRET", "WU_FORUM_DISCORD", "DISCORD_BOT_TOKEN"):
    os.environ.pop(k, None)
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
wp.ac_soap = lambda cmd: ""
wp.social.setup(start_thread=False)
F = wp.forum.setup()
FM = wp.forum
check("off by default: no WU_FORUM_DISCORD, no link", F is not None and F.discord is None)
NOW = [2_000_000_000.0]
F.clock = lambda: NOW[0]
wp.browse_limit.hit = lambda *a, **k: True
wp.admin.POSTS_PER_MIN = wp.admin.IP_PER_MIN = wp.admin.PAGES_PER_MIN = 10 ** 6
PAGE = serve(wp.PageHandler)

FD = FM._discord_mod()
check("forum_discord.py loads beside forum.py", FD is not None)
check("off with WU_FORUM_DISCORD unset, even with every setting", FD.attach(FM, F, {"DISCORD_BOT_TOKEN": TOKEN, "WU_FORUM_DISCORD_CHANNELS": "help=1" * 1}, False) is None)
cfg = FD.Config({"WU_FORUM_DISCORD": "1", "WU_FORUM_DISCORD_CHANNELS": "help=12345,nope=67890,general=abc"}, tuple(FM.CAT))
pr = cfg.problems()
check("incomplete settings: the reasons (no token, unknown category, a bad id)", any("DISCORD_BOT_TOKEN" in p for p in pr)
      and any("'nope'" in p for p in pr) and any("general=" in p for p in pr), pr)
check("on with incomplete settings: stays off", FD.attach(FM, F, {"WU_FORUM_DISCORD": "1", "WU_FORUM_DISCORD_CHANNELS": "help=12345"}, False) is None)
check("DISCORD_APP_ID and DISCORD_PUBLIC_KEY go together", any("go together" in p for p in FD.Config(
    {"DISCORD_BOT_TOKEN": "x", "WU_FORUM_DISCORD_CHANNELS": "help=12345", "DISCORD_APP_ID": "123456"}, tuple(FM.CAT)).problems()))

ENV = {"WU_FORUM_DISCORD": "1", "DISCORD_BOT_TOKEN": TOKEN, "DISCORD_APP_ID": APP, "DISCORD_PUBLIC_KEY": PK,
       "WU_FORUM_DISCORD_CHANNELS": f"help={CH_HELP},general={CH_GENERAL},news={CH_NEWS}/555000000000000001,dens={CH_DENS}",
       "WU_DISCORD_API": f"http://127.0.0.1:{DISC}/api/v10"}
os.environ.update(ENV)
L = FM.discord_setup(start_thread=False)
FD = FM._fd
check("on: the link attaches to the store", L is not None and F.discord is L and FD.link is L)
check("the repr never shows the token", TOKEN not in repr(L.cfg) and TOKEN not in repr(L.api))
LT = [time.time()]
L.clock = lambda: LT[0]
L.api.MIN_GAP = 0
sleeps = []
L.api._sleep = lambda s: sleeps.append(s)
L.start()
check("start: the bot, the channel types, the server", L.bot_id == BOT and L.guild == GUILD and L.ch_type[CH_HELP] == 15 and L.ch_type[CH_GENERAL] == 0)
check("start: /link and /unlink registered for the server", [c["name"] for c in D["commands"] or []] == ["link", "unlink"])
check("every call carries the bot token in the Authorization header only", D["auth"] == {"Bot " + TOKEN})


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


def tok(who, path):
    _, _, body = req("GET", path, who)
    m = re.search(r'name="t" value="([0-9a-f]{32})"', body)
    return m.group(1) if m else ""


def site_thread(who, cat, title, body):
    st, hd, _ = req("POST", "/wrath/leyline/forum/new", who, {"t": tok(who, "/wrath/leyline/forum/new"), "cat": cat, "title": title, "body": body})
    NOW[0] += 100
    m = re.search(r"/t/(\d+)", hd.get("Location", ""))
    return int(m.group(1)) if m else 0


def site_reply(who, tid, body):
    st, hd, _ = req("POST", f"/wrath/leyline/forum/t/{tid}/reply", who, {"t": tok(who, f"/wrath/leyline/forum/t/{tid}/reply"), "body": body})
    NOW[0] += 100
    m = re.search(r"#p(\d+)", hd.get("Location", ""))
    return int(m.group(1)) if m else 0


def outbox():
    return F.q("SELECT kind, thread, tries, next_at FROM fd_outbox ORDER BY id")


def dthread_of(tid):
    r = L.live_for(tid)
    return r["dthread"] if r else ""


def bot_msgs(dt):
    return [m for m in sorted(D["msgs"].get(dt, {}).values(), key=lambda x: int(x["id"])) if m["author"]["id"] == BOT]


# ------------------------------------------------------------------------------------------- forum -> Discord
T1 = site_thread("alice", "help", "Launcher will not start", "It says **error** and @everyone should see <@123>.\nhttps://example.com/a_b_c")
check("a site thread goes to the outbox first", [k for k, *_ in outbox()] == ["thread.new"])
L.drain()
d1 = dthread_of(T1)
th = D["threads"].get(d1, {})
st1 = D["msgs"].get(d1, {}).get(d1, {})
check("Forum channel: a Discord thread named after the forum thread, with the first post as its starter",
      th.get("parent_id") == CH_HELP and th.get("name") == "Launcher will not start" and "Alice" in st1.get("content", ""), (th, st1))
check("the starter: the author's name and a link to the post, no pings, no embeds",
      "https://play.example/wrath/leyline/forum/t/%d#p" % T1 in st1["content"] and st1["allowed_mentions"] == {"parse": []} and st1["flags"] == 4)
check("markdown shown as typed, @everyone and <@id> defused, the link left whole",
      "\\*\\*error\\*\\*" in st1["content"] and "@everyone" not in st1["content"].replace("@​everyone", "")
      and "<@123>" not in st1["content"] and "https://example.com/a_b_c" in st1["content"], st1["content"])
check("the outbox is empty after delivery", outbox() == [])
P2 = site_reply("bob", T1, "Try the repair button.")
L.drain()
bm = bot_msgs(d1)
check("a site reply goes into the thread as the bot: name, link, text", len(bm) == 2 and bm[-1]["content"].startswith("**Bob")
      and f"#p{P2}" in bm[-1]["content"] and "Try the repair button." in bm[-1]["content"], bm[-1:])
T2 = site_thread("bob", "general", "Hello there all", "Hi")
T3 = site_thread("alice", "mods", "My cool addon", "Look")
L.drain()
d2 = dthread_of(T2)
check("text channel: a thread in it, the first post inside", D["threads"].get(d2, {}).get("parent_id") == CH_GENERAL
      and len(bot_msgs(d2)) == 1 and "started this on the Leyline forum" in bot_msgs(d2)[0]["content"])
check("an unmapped category (mods) stays off the Discord", not dthread_of(T3))
TN = 0
st_, hd_, _ = req("POST", "/wrath/leyline/forum/new", "staff1", {"t": tok("staff1", "/wrath/leyline/forum/new"), "cat": "news", "title": "Patch day news", "body": "New things"})
NOW[0] += 100
TN = int(re.search(r"/t/(\d+)", hd_.get("Location", "")).group(1))
L.drain()
check("a forum tag on the Discord thread when the setting names one", D["threads"][dthread_of(TN)]["applied_tags"] == ["555000000000000001"])

# ------------------------------------------------------------------------------------------- linking
def interact(doc, ts=None, sk=SK, sig=None):
    body = json.dumps(doc).encode()
    ts = str(int(time.time())) if ts is None else ts
    s = sig or ED.sign(sk, ts.encode() + body).hex()
    st, _, b = req("POST", "/wrath/forum/api/discord/interactions", raw=body,
                   headers={"X-Signature-Ed25519": s, "X-Signature-Timestamp": ts, "Content-Type": "application/json"})
    try:
        return st, json.loads(b)
    except ValueError:
        return st, {}


st, doc = interact({"type": 1})
check("interactions: Discord's PING answered", st == 200 and doc == {"type": 1}, (st, doc))
st, doc = interact({"type": 1}, sk=b"\x08" * 32)
check("interactions: a bad signature refused (401)", st == 401)
st, doc = interact({"type": 1}, ts=str(int(time.time()) - 3600))
check("interactions: an old timestamp refused (401)", st == 401)
U_AL, U_ST, U_UN, U_CL, U_DA = "50000000000000001", "50000000000000003", "50000000000000009", "50000000000000008", "60000000000000005"
st, doc = interact({"type": 2, "data": {"name": "link"}, "member": {"user": user(U_AL, "AliceD"), "nick": None}})
m = re.search(r"/wrath/leyline/forum/discord\?c=([\w-]+)", doc.get("data", {}).get("content", ""))
check("/link: an only-you-can-see answer with a one-time link", st == 200 and doc["type"] == 4 and doc["data"]["flags"] == 64 and m, doc)
code = m.group(1) if m else ""
st, hd, body = req("GET", "/wrath/leyline/forum/discord?c=" + code)
check("the link page signed out: to the sign-in, the code kept", st in (302, 303) and re.search(r"signin|login", hd.get("Location", "")) and code in urllib.parse.unquote(hd.get("Location", "")), hd)
st, _, body = req("GET", "/wrath/leyline/forum/discord?c=" + code, "alice")
check("the link page signed in: which Discord account, which Leyline account", st == 200 and "AliceD" in body and "Alice" in body and 'value="link"' in body)
t = re.search(r'name="t" value="([0-9a-f]{32})"', body).group(1)
st, _, body = req("POST", "/wrath/leyline/forum/discord", "alice", {"t": "0" * 32, "c": code, "do": "link"})
check("linking needs the form token", st == 403 and not L.linked(U_AL))
st, _, body = req("POST", "/wrath/leyline/forum/discord", "alice", {"t": t, "c": code, "do": "link"})
check("linked", st == 200 and "Linked." in body and L.linked(U_AL) == "WR000001", body[-400:])
st, _, body = req("POST", "/wrath/leyline/forum/discord", "bob", {"t": FM.form_token({"session": "bob", "acct": "WR000002"}, "dlink"), "c": code, "do": "link"})
check("a used code is refused", st == 400 and "old or was used" in body and L.linked(U_AL) == "WR000001")
st, doc = interact({"type": 2, "data": {"name": "link"}, "member": {"user": user(U_AL, "AliceD")}})
check("/link when linked: says to whom", "Alice" in doc["data"]["content"] and doc["data"]["flags"] == 64)
for uid, sess in ((U_ST, "staff1"), ("50000000000000002", "bob")):
    _, doc = interact({"type": 2, "data": {"name": "link"}, "user": user(uid, sess.title() + "D")})
    c2 = re.search(r"\?c=([\w-]+)", doc["data"]["content"]).group(1)
    req("POST", "/wrath/leyline/forum/discord", sess, {"t": tok(sess, "/wrath/leyline/forum/discord?c=" + c2), "c": c2, "do": "link"})
check("two more linked (a staff member, Bob)", L.linked(U_ST) == "WR000003" and L.linked("50000000000000002") == "WR000002")
st, _, body = req("GET", "/wrath/leyline/forum/discord", "dana")
check("the website's own Discord sign-in: offered on the page", 'value="site"' in body)
req("POST", "/wrath/leyline/forum/discord", "dana", {"t": tok("dana", "/wrath/leyline/forum/discord"), "do": "site"})
check("... and linked from it", L.linked(U_DA) == "WR000005")
_, doc = interact({"type": 2, "data": {"name": "unlink"}, "user": user(U_DA, "Dana")})
check("/unlink", "Unlinked" in doc["data"]["content"] and not L.linked(U_DA))

# ------------------------------------------------------------------------------------------- Discord -> forum
def discord_says(dt, uid, name, text, **kw):
    with lock:
        return new_msg(dt, user(uid, name), text, **kw)


def round_():
    L.poll(); L.drain()


NOW[0] += 100
m1 = discord_says(d1, U_AL, "AliceD", "Thanks **Bob**, <@50000000000000002> that worked", mentions=[user("50000000000000002", "BobD")])
round_()
ps, _, _ = F.posts(T1)
last = ps[-1]
check("a linked person's Discord reply becomes a forum post as them", last["author"] == "WR000001" and last["via"] == "discord"
      and last["via_name"] == "AliceD" and last["body"] == "Thanks Bob, @BobD that worked", last)
check("... and is not echoed back to Discord", len(bot_msgs(d1)) == 2 and m1["id"] in D["msgs"][d1])
st, _, body = req("GET", f"/wrath/leyline/forum/t/{T1}")
check("the thread page shows it via Discord with the Discord name", "via Discord · AliceD" in body)
st, _, body = req("GET", f"/wrath/forum/api/thread?id={T1}")
check("the JSON says via discord", json.loads(body)["posts"][-1]["via"] == "discord")

mu = discord_says(d1, U_UN, "Stranger", "hello?")
round_()
check("unlinked: nothing reaches the forum", len(F.posts(T1)[0]) == len(ps))
check("unlinked: one note by DM with the link page", any("Link your Leyline account" in x and "/wrath/leyline/forum/discord" in x for x in D["dms"].get("dm" + U_UN, [])))
check("unlinked: the message is taken off the Discord", mu["id"] not in D["msgs"][d1])
discord_says(d1, U_UN, "Stranger", "again?")
round_()
check("unlinked: the note only once", len(D["dms"]["dm" + U_UN]) == 1)
D["closed_dms"].add(U_CL)
mc = discord_says(d1, U_CL, "Quiet", "hi")
round_()
fb = [x for x in bot_msgs(d1) if x["content"].startswith(f"<@{U_CL}>")]
check("DMs closed: a short note in the thread that pings only them", len(fb) == 1 and fb[0]["allowed_mentions"] == {"parse": [], "users": [U_CL]}, fb)
LT[0] += 200
L.drain()
check("... deleted after 2 minutes", fb and fb[0]["id"] not in D["msgs"][d1])

discord_says(d1, U_AL, "AliceD", "one more right away")
round_()
check("a rate limit (an account new to the forum): refused, a note, taken off", not F.posts(T1)[0][-1]["body"].startswith("one more")
      and any("Wait" in x for x in D["dms"].get("dm" + U_AL, [])))
NOW[0] += 100
LT[0] += 100
mb = discord_says(d1, U_AL, "AliceD", "Blizzard made this")
round_()
p_b = F.posts(T1)[0][-1]
check("the word / brand filter applies (as on the site)", p_b["body"] == "the publisher made this" and p_b["via"] == "discord", p_b)
check("... and the Discord copy is replaced by the forum's text", mb["id"] not in D["msgs"][d1]
      and any("the publisher made this" in x["content"] and "via Discord" in x["content"] for x in bot_msgs(d1)))
NOW[0] += 100
LT[0] += 100
wp.admin.set_sanction({"account": "staff1"}, "noforum", "WR000001", "", 1, "cool down")
mm = discord_says(d1, U_AL, "AliceD", "muted words")
round_()
check("an HQ forum mute applies: refused, the reason by DM, taken off", F.posts(T1)[0][-1]["body"] != "muted words"
      and any("can't post on the forum" in x for x in D["dms"]["dm" + U_AL]) and mm["id"] not in D["msgs"][d1])
for v in wp.admin.sanctions_for("WR000001"):
    wp.admin.lift_sanction({"account": "staff1"}, v["id"])
wp.admin.set_sanction({"account": "staff1"}, "suspend", "WR000002", "", 0, "banned for spam")
NOW[0] += 100
LT[0] += 100
discord_says(d1, "50000000000000002", "BobD", "banned words")
round_()
check("an account ban applies too", F.posts(T1)[0][-1]["body"] != "banned words" and any("suspended" in x for x in D["dms"].get("dm50000000000000002", [])))
for v in wp.admin.sanctions_for("WR000002"):
    wp.admin.lift_sanction({"account": "staff1"}, v["id"])

# Discord-made threads (the forum's limits on new threads apply: an hour on, so these are within them)
NOW[0] += 7200
LT[0] += 100
with lock:
    old = make_thread(CH_HELP, "From last year", U_AL, {"author": user(U_AL, "AliceD"), "content": "old"}, id=snow(int(time.time() * 1000) - 10 ** 9))
    dt_new = make_thread(CH_HELP, "Started on Discord", U_AL, {"author": user(U_AL, "AliceD"), "content": "Hi from Discord"})
    dt_un = make_thread(CH_HELP, "Spam thread here", U_UN, {"author": user(U_UN, "Stranger"), "content": "buy stuff"})
round_()
tn = L.thread_row(dt_new["id"])
check("a linked person's new Discord thread becomes a forum thread", tn and tn["state"] == "live" and F.thread(tn["thread"])["title"] == "Started on Discord"
      and F.posts(tn["thread"])[0][0]["via"] == "discord", tn)
check("an unlinked person's new Discord thread is taken down", dt_un["id"] not in D["threads"] and L.thread_row(dt_un["id"])["state"] == "refused")
check("threads from before the link are left alone", old["id"] in D["threads"] and L.thread_row(old["id"])["state"] == "ignored")
st_th = len(F.posts(tn["thread"])[0])
NOW[0] += 100
discord_says(dt_new["id"], U_ST, "SamD", "Welcome!")
round_()
check("replies in a Discord-made thread reach the forum", len(F.posts(tn["thread"])[0]) == st_th + 1)

# ------------------------------------------------------------------------------------------- moderation, both ways
def act(action, target, **f):
    with_user = {"account": "staff1", "name": "Mod Sam", "role": "admin"}
    return wp.admin.run_action(with_user, dict(f, action=action, target=str(target), confirm=str(target)))


r = act("forum.post.hide", P2)
L.drain()
pr2 = L.post_row(P2)
msg2 = D["msgs"][d1][pr2["dmsg"]]
check("HQ hides a site post: our Discord copy says removed", F.post(P2)["hidden"] and FD.REMOVED in msg2["content"] and "Try the repair" not in msg2["content"], (r, msg2))
act("forum.post.unhide", P2)
L.drain()
check("HQ shows it again: the Discord copy is back", "Try the repair button." in D["msgs"][d1][pr2["dmsg"]]["content"])
pa = [p for p in F.posts(T1)[0] if p["via"] == "discord" and p["body"].startswith("Thanks")][0]
dm_a = L.post_row(pa["id"])["dmsg"]
act("forum.post.hide", pa["id"])
L.drain()
check("HQ hides a Discord person's post: deleted on Discord", dm_a not in D["msgs"][d1])
act("forum.post.unhide", pa["id"])
L.drain()
check("shown again: re-posted by the bot as the forum shows it", any("Thanks Bob" in x["content"] and "via Discord" in x["content"] for x in bot_msgs(d1)))
act("forum.thread.lock", T1)
L.drain()
check("HQ locks: the Discord thread is locked", D["threads"][d1]["thread_metadata"]["locked"] is True)
act("forum.thread.unlock", T1)
L.drain()
check("HQ unlocks: unlocked", D["threads"][d1]["thread_metadata"]["locked"] is False)
act("forum.thread.hide", T2)
L.drain()
check("HQ hides a thread: the Discord thread is deleted", d2 not in D["threads"] and not dthread_of(T2))
act("forum.thread.unhide", T2)
L.drain()
check("shown again: a new Discord thread with its first post", dthread_of(T2) and dthread_of(T2) != d2 and dthread_of(T2) in D["threads"])
NOW[0] += 7200
T4 = site_thread("bob", "help", "Launcher won't start too", "Same here")
L.drain()
d4 = dthread_of(T4)
act("forum.thread.merge", T4, into=str(T1))
L.drain()
check("HQ merges: a note in the old Discord thread, locked and archived", D["threads"][d4]["thread_metadata"] == {"locked": True, "archived": True}
      and "merged into" in bot_msgs(d4)[-1]["content"] and f"/t/{T1}" in bot_msgs(d4)[-1]["content"])

# deleted in Discord -> hidden on the forum
P5 = site_reply("bob", T1, "A reply a Discord mod will delete")
L.drain()
round_()
dm5 = L.post_row(P5)["dmsg"]
with lock:
    del D["msgs"][d1][dm5]
    D["threads"][d1]["message_count"] -= 1
    D["threads"][d1]["last_message_id"] = max(D["msgs"][d1], key=int)
round_()
check("a Discord mod deletes our copy: the count drops, the post is hidden on the forum", F.post(P5)["hidden"])
aud = [json.loads(x) for x in open(os.path.join(TMP, "admin", "audit.jsonl"))]
check("... written in HQ's audit log as Discord", any(a["action"] == "forum.post.hide" and a["target"] == str(P5) and a["via"] == "discord" for a in aud))
check("... and not sent back to Discord as a change", not any(k == "post.hide" for k, *_ in outbox()))
with lock:
    del D["msgs"][dthread_of(TN)][dthread_of(TN)]
L.sweep_some()
check("the first post deleted in Discord (the sweep): hidden on the forum", F.posts(TN)[0][0]["hidden"])
with lock:
    D["threads"].pop(dthread_of(T2)); D["msgs"].pop(dthread_of(T2))
L.sweep_some()
check("a whole thread deleted in Discord: the forum thread is hidden", F.thread(T2)["hidden"])

# ------------------------------------------------------------------------------------------- reliability
n0 = len(D["calls"])
with lock:
    D["fail"] += [(503, {"message": "down"})] * 3
PA = site_reply("alice", T1, "first while down")
PB = site_reply("bob", T1, "second while down")
L.drain()
ob = outbox()
check("Discord down: both kept in the outbox, the first retried later, the second waits behind it",
      [k for k, *_ in ob] == ["post.new", "post.new"] and ob[0][2] == 1 and ob[0][3] > LT[0] and ob[1][2] == 0, ob)
L.drain()
check("not due yet: no call made", len(D["calls"]) == n0 + 1)
LT[0] += 6
L.drain()
ob = outbox()
check("back-off doubles", ob and ob[0][2] == 2 and ob[0][3] - LT[0] >= 9, ob)
LT[0] += 11
L.drain()
LT[0] += 21
L.drain()
bodies = [x["content"] for x in bot_msgs(d1)]
check("Discord back: delivered, in order", outbox() == [] and bodies[-2].endswith("first while down") and bodies[-1].endswith("second while down"), bodies[-2:])
with lock:
    D["fail"].append((429, {"message": "slow", "retry_after": 0.2, "global": False}))
sleeps.clear()
PC = site_reply("alice", T1, "after a short 429")
L.drain()
check("a short 429: waited out and sent in the same round", outbox() == [] and bot_msgs(d1)[-1]["content"].endswith("after a short 429") and any(0.15 < x <= 0.2 for x in sleeps), sleeps)
with lock:
    D["fail"].append((429, {"message": "slow", "retry_after": 40, "global": True}))
PD = site_reply("bob", T1, "after a long 429")
L.drain()
ob = outbox()
check("a long (global) 429: handed back, retried after it", ob and ob[0][3] - LT[0] >= 39 and L.api.global_until > 0, ob)
L.api.global_until = 0
LT[0] += 41
L.drain()
check("... then sent", outbox() == [])
with lock:
    D["headers"][f"/channels/{d1}/messages"] = {"X-RateLimit-Remaining": "0", "X-RateLimit-Reset-After": "30"}
site_reply("alice", T1, "uses up the bucket")
L.drain()
n1 = len(D["calls"])
site_reply("bob", T1, "must wait for the bucket")
L.drain()
check("X-RateLimit-Remaining 0: the route waits without calling Discord", len(D["calls"]) == n1 and len(outbox()) == 1)
L.api.until.clear()
LT[0] += 31
L.drain()
check("... and goes when it resets", outbox() == [])
with lock:
    D["fail"].append((400, {"message": "Invalid Form Body", "code": 50035}))
site_reply("alice", T1, "Discord says no to this")
L.drain()
check("a 400: dropped (it can never work), noted", outbox() == [] and "post.new" in L.meta("last_drop"))
with lock:
    D["fail"].append((401, {"message": "401: Unauthorized", "code": 0}))
site_reply("bob", T1, "token went bad")
try:
    L.drain()
    raised = False
except FD.DiscordError as e:
    raised = e.kind == "auth"
check("a 401: the worker stops and nothing is lost", raised and len(outbox()) == 1)
L.drain()
check("... delivered once the token works again", outbox() == [])
st = L.status()
check("status for HQ: state, outbox, links; no token", st["outbox"] == 0 and st["links"] >= 3 and TOKEN not in json.dumps(st), st)
code_hq, page_hq = wp.admin.hq.forum_page({"account": "staff1", "role": "admin", "name": "Mod Sam"}, {}) if getattr(wp.admin, "hq", None) else (200, "Discord link")
check("HQ's Forum page says how the Discord link is doing", "Discord link" in page_hq, page_hq[:300])
check("the token never reached a log line", TOKEN not in logbuf.getvalue())

# ------------------------------------------------------------------------------------------- the deploy step
bat = open(os.path.join(ROOT, "stream", "Update sign-in and warm-up.bat"), encoding="utf-8").read()
sh = open(os.path.join(ROOT, "stream", "fastloop.sh"), encoding="utf-8").read()
check("the signin step ships forum_discord.py (with forum.py) and fastloop.sh installs it, with a backup",
      "forum_discord.py" in bat and re.search(r"install -m 644 forum_discord\.py", sh) and "BK/forum_discord.py" in sh)

print(f"{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
