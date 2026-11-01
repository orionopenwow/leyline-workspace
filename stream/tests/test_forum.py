"""The Leyline forum (stream/forum.py) through the real portal (stream/wrath_portal.py) with the real social.py (the
profile names), hub.py (the look and the front page) and admin.py (the Forum queue), against a fake website sign-in.

Covers: reading signed out (pages and the JSON API), posting only when signed in (no account is ever made), the form
token, cross-site and Origin checks (Origin null only with Sec-Fetch-Site same-origin), plain text with links (no
HTML, an image address stays a link), the lengths, the brand-word and word filters, Announcements for staff only,
rate limits (stricter for accounts new to the forum), locked threads, reports into the admin console's Forum tab,
hide, pin, merge (the old address leads to the new one), title search, the hub's nav item and front page section,
and the deploy step.

    python3 -I stream/tests/test_forum.py stream/wrath_portal.py
"""
import html, http.client, importlib.util, json, os, re, sys, tempfile, threading, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "stream", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
TMP = tempfile.mkdtemp(prefix="wu-forum-")
fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra!r}"[:600] if extra and not cond else ""))
    fails += 0 if cond else 1


# site session -> (website id, display name, game account)
ACCOUNTS = {"alice": ("alice", "Alice", "WR000001"), "bob": ("bob", "Bob", "WR000002"), "staff1": ("staff1", "Mod Sam", "WR000003"),
            "carol": ("carol", "Carol", "WR000004")}
site_calls = []


class Site(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_GET(self):
        m = re.search(r"wr_session=(\w+)", self.headers.get("Cookie", ""))
        a = ACCOUNTS.get(m.group(1)) if m else None
        site_calls.append(self.path)
        if not a:
            self.send_response(401); self.send_header("Content-Length", "0"); self.end_headers(); return
        doc = {"/api/me": {"id": a[0], "display_name": a[1]}, "/api/play": {"user": a[2], "pass": "x" * 16}}[self.path]
        b = json.dumps(doc).encode()
        self.send_response(200); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)


def serve(handler):
    srv = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv.server_address[1]


SITE = serve(Site)
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL=f"http://127.0.0.1:{SITE}",
                  WU_STREAM="0", STREAM_HOST="wrath.example", WU_PORTAL_DATA=TMP, WU_HOST_DIR=os.path.join(TMP, "hosted"),
                  WU_RELAY_LISTING="http://127.0.0.1:9/none", WU_ADMIN_STAFF="staff1", WU_ADMIN_OWNERS="", WU_ADMIN_GM_LEVEL="0",
                  PUBLIC_HOST="play.example", WU_HUB_FRONT="0")
os.environ.pop("WU_FORM_SECRET", None)
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
soap_calls = []
wp.ac_soap = lambda cmd: soap_calls.append(cmd) or ""
check("the portal loads forum.py", wp.forum is not None)
wp.social.setup(start_thread=False)
F = wp.forum.setup()
FM = wp.forum
check("the store opens in WU_PORTAL_DATA (forum.sqlite3, its key 0600)", F is not None and os.path.isfile(os.path.join(TMP, "forum.sqlite3"))
      and (os.stat(os.path.join(TMP, "forum.key")).st_mode & 0o777) == 0o600)
NOW = [2_000_000_000.0]
F.clock = lambda: NOW[0]
wp.browse_limit.hit = lambda *a, **k: True
wp.admin.POSTS_PER_MIN = wp.admin.IP_PER_MIN = wp.admin.PAGES_PER_MIN = 10 ** 6
PAGE = serve(wp.PageHandler)


def req(method, path, who=None, form=None, origin="https://play.example", sfs="same-origin", raw=None):
    c = http.client.HTTPConnection("127.0.0.1", PAGE, timeout=30)
    h = {"X-Forwarded-For": "10.0.0.7"}
    if who:
        h["Cookie"] = f"wr_session={who}"
    body = None
    if form is not None or raw is not None:
        body = raw if raw is not None else urllib.parse.urlencode(form).encode()
        h["Content-Type"] = "application/x-www-form-urlencoded"
        if origin is not None:
            h["Origin"] = origin
        if sfs:
            h["Sec-Fetch-Site"] = sfs
    c.request(method, path, body=body, headers=h)
    r = c.getresponse()
    out = (r.status, {k: v for k, v in r.getheaders()}, r.read().decode("utf-8", "replace"))
    c.close()
    return out


def api(sub):
    st, hd, body = req("GET", "/wrath/forum/api/" + sub)
    try:
        return st, hd, json.loads(body)
    except ValueError:
        return st, hd, {}


def tok(who, path):
    st, _, body = req("GET", path, who)
    m = re.search(r'name="t" value="([0-9a-f]{32})"', body)
    return m.group(1) if m else ""


def new_thread(who, cat, title, body, **kw):
    t = tok(who, "/wrath/leyline/forum/new")
    return req("POST", "/wrath/leyline/forum/new", who, {"t": t, "cat": cat, "title": title, "body": body}, **kw)


def reply(who, tid, body):
    t = tok(who, f"/wrath/leyline/forum/t/{tid}/reply")
    return req("POST", f"/wrath/leyline/forum/t/{tid}/reply", who, {"t": t, "body": body})


def tid_of(r):
    m = re.search(r"/wrath/leyline/forum/t/(\d+)", r[1].get("Location", ""))
    return int(m.group(1)) if m else 0


BRAND = re.compile(r"(?i)world\s*of\s*warcraft|warcraft|lich\s*king|blizzard|\bwow\b")

# ---------------------------------------------------------------- reading, signed out
st, hd, body = req("GET", "/wrath/leyline/forum")
check("the forum: 200 signed out, the six categories in order", st == 200 and all(n in body for n in (
    "Share your realm", "Mods &amp; addons", "Dens &amp; builds", "Help", "General", "Announcements"))
      and body.index("Share your realm") < body.index("Mods &amp; addons") < body.index("Announcements"), body[:300])
check("the forum: the hub's look (its header and tokens), the Forum item lit, no script", 'class="hb-top"' in body and "--lk-bg" in body
      and re.search(r'class="hb-ni on" href="/wrath/leyline/forum" aria-current="page"', body) and "<script" not in body)
check("the forum: a big Join the Discord and Sign in to post", "Join the Discord" in body and wp.DISCORD_URL in body
      and "Sign in to post" in body and "/login?next=%2Fwrath%2Fleyline%2Fforum%2Fnew" in body)
check("the forum: Hub preview stays noindex while the front door is off", hd.get("X-Robots-Tag", "").startswith("noindex"))
st, hd, _ = req("GET", "/wrath/leyline-preview/forum/c/help?page=2")
check("the preview's address sends a 301 to the forum's own", st == 301 and hd.get("Location") == "/wrath/leyline/forum/c/help?page=2")
st, _, body = req("GET", "/wrath/leyline/forum/c/help")
check("a category: 200, empty state", st == 200 and "No threads here yet" in body)
st, _, _ = req("GET", "/wrath/leyline/forum/c/nope")
check("an unknown category: 404", st == 404)
st, _, _ = req("GET", "/wrath/leyline/forum/t/999")
check("an unknown thread: 404", st == 404)
st, hd, _ = req("GET", "/wrath/leyline/forum/new")
check("New thread signed out: to the sign-in, and back", st == 302 and hd.get("Location") == "/login?next=%2Fwrath%2Fleyline%2Fforum%2Fnew")
st, hd, doc = api("categories")
check("API categories: six, Announcements staff only, CORS *", st == 200 and [c["id"] for c in doc["categories"]] == [
    "realms", "mods", "dens", "help", "general", "news"] and [c["staff_only"] for c in doc["categories"]] == [False] * 5 + [True]
      and hd.get("Access-Control-Allow-Origin") == "*" and doc["url"] == "https://play.example/wrath/leyline/forum", doc)
st, _, doc = api("threads?cat=zzz")
check("API threads: unknown category 404", st == 404)
st, _, _ = req("POST", "/wrath/forum/api/categories", form={})
check("API: read only (POST 405)", st == 405)
site_calls.clear()
st, hd, _ = req("POST", "/wrath/leyline/forum/new", form={"cat": "help", "title": "Hello there", "body": "x"})
check("POST signed out: to the sign-in, nothing made", st == 303 and "/login" in hd.get("Location", "") and not F.latest())

# ---------------------------------------------------------------- posting
st, _, body = req("GET", "/wrath/leyline/forum/new", "alice")
check("New thread signed in: the form with a token, staff-only Announcements not offered", st == 200 and 'name="t" value="' in body
      and 'value="news"' not in body and 'value="realms"' in body and "as Alice" in body, body[-1500:])
t = tok("alice", "/wrath/leyline/forum/new")
st, _, body = req("POST", "/wrath/leyline/forum/new", "alice", {"t": "0" * 32, "cat": "help", "title": "Hello there", "body": "x"})
check("a wrong form token: 403, the form again with what was typed", st == 403 and "timed out" in body and "Hello there" in body)
st, _, _ = req("POST", "/wrath/leyline/forum/new", "alice", {"t": t, "cat": "help", "title": "Hello there", "body": "x"}, sfs="cross-site")
check("Sec-Fetch-Site cross-site: 403", st == 403)
st, _, _ = req("POST", "/wrath/leyline/forum/new", "alice", {"t": t, "cat": "help", "title": "Hello there", "body": "x"}, origin="https://evil.example")
check("another site's Origin: 403", st == 403)
st, _, _ = req("POST", "/wrath/leyline/forum/new", "alice", {"t": t, "cat": "help", "title": "Hello there", "body": "x"}, origin="null", sfs="")
check("Origin null without Sec-Fetch-Site same-origin: 403", st == 403)
check("nothing posted by the refused forms", F.latest() == [])
body_text = ("How do I share my realm?\nHere is a shot: https://i.example/shot.png and the guide (https://docs.example/a_b).\n"
             "<script>alert(1)</script> <b>bold</b> javascript:alert(2)")
r = req("POST", "/wrath/leyline/forum/new", "alice", {"t": t, "cat": "help", "title": "  Sharing   my realm ", "body": body_text},
        origin="null", sfs="same-origin")
T1 = tid_of(r)
check("Origin null with Sec-Fetch-Site same-origin: allowed; 303 to the new thread", r[0] == 303 and T1 > 0, r[:2])
check("no account made anywhere: no SOAP, only /api/me and /api/play asked", soap_calls == [] and set(site_calls) <= {"/api/me", "/api/play"})
st, _, body = req("GET", f"/wrath/leyline/forum/t/{T1}")
check("the thread: title tidied, the author's Leyline profile name and #tag", st == 200 and "<h1>Sharing my realm</h1>" in body
      and "<b>Alice</b><small>#" in body, body[-2500:])
check("links made clickable, nofollow ugc, punctuation left outside",
      '<a href="https://i.example/shot.png" rel="nofollow ugc noopener noreferrer">https://i.example/shot.png</a>' in body
      and '<a href="https://docs.example/a_b" rel="nofollow ugc noopener noreferrer">https://docs.example/a_b</a>).' in body)
check("an image address is only a link (no <img> from a post), no HTML from the post, javascript: never a link",
      "i.example/shot.png\"" not in body.replace('href="https://i.example/shot.png"', "") and "<img src=\"https://" not in body
      and "&lt;script&gt;alert(1)&lt;/script&gt; &lt;b&gt;bold&lt;/b&gt;" in body and 'href="javascript' not in body)
check("line breaks kept as text (pre-wrap)", "white-space:pre-wrap" in body and "How do I share my realm?\nHere is a shot" in body)
check("signed out: Sign in to reply, no reply form, no Report link", "Sign in to reply" in req("GET", f"/wrath/leyline/forum/t/{T1}")[2]
      and 'action="/wrath/leyline/forum/t/' not in req("GET", f"/wrath/leyline/forum/t/{T1}")[2])

# the rules for what is written (bob: also new, so space the posts out)
NOW[0] += 4000


def refused(who, cat, title, body_, want):
    NOW[0] += 4000
    st, _, b = new_thread(who, cat, title, body_)
    return st == 400 and want in html.unescape(b), (st, re.findall(r'role="alert">([^<]*)', b))


ok, info = refused("bob", "general", "WoW addons for my realm", "hi", "Leave game and company names out of the title")
check("a brand word in a title: refused", ok, info)
ok, info = refused("bob", "general", "World of Warcraft talk", "hi", "Leave game and company names")
check("World of Warcraft in a title: refused", ok, info)
ok, info = refused("bob", "general", "Blizzard stuff here", "hi", "Leave game and company names")
check("Blizzard in a title: refused", ok, info)
ok, info = refused("bob", "general", "you are a fucking idiot", "hi", "Pick another title")
check("the word filter on titles: refused", ok, info)
ok, info = refused("bob", "general", "Hey", "hi", "at least 4")
check("a title under 4 characters: refused", ok, info)
ok, info = refused("bob", "general", "x" * 101, "hi", "at most 100")
check("a title over 100: refused", ok, info)
ok, info = refused("bob", "general", "Long post", "y" * 5001, "at most 5000")
check("a post over 5000 characters: refused", ok, info)
ok, info = refused("bob", "general", "Empty post", "   ", "Write something first")
check("an empty post: refused", ok, info)
ok, info = refused("bob", "general", "Look at https://x.example", "hi", "not the title")
check("a link in a title: refused", ok, info)
ok, info = refused("bob", "nope", "Good title", "hi", "Pick a category")
check("an unknown category: refused", ok, info)
ok, info = refused("bob", "news", "Big news today", "hi", "Only the Leyline team")
check("Announcements: a player can't start a thread (403)", not ok and new_thread("bob", "news", "Big news today", "hi")[0] == 403)
NOW[0] += 4000
r = new_thread("bob", "mods", "Addon pack for my Den", "A mod for World of Warcraft and the Lich King, see https://x.example/wow-ui ok? Fuck yes.")
T2 = tid_of(r)
b2 = req("GET", f"/wrath/leyline/forum/t/{T2}")[2]
check("brand words in a post replaced (links untouched), the word filter starred out",
      r[0] == 303 and "A mod for the game and the Wrath" in b2 and "https://x.example/wow-ui" in b2 and "**** yes" in b2
      and not BRAND.search(re.sub(r"https://x\.example/wow-ui", "", re.sub(r"<style>.*?</style>", "", b2, flags=re.S))), b2[-1500:])

# rate limits: carol is new to the forum
NOW[0] += 4000
r = new_thread("carol", "general", "First thread", "hello")
check("a new account's first thread", r[0] == 303)
NOW[0] += 120
st, _, b = new_thread("carol", "general", "Second thread", "hello again")
check("a new account: one new thread an hour", st == 429 and "lower limit" in b)
st, _, b = reply("carol", T1, "quick reply")
check("a new account can still reply in the meantime (past the 60 s gap)", st == 303, (st, b[-300:]))
NOW[0] += 61
st, _, b = reply("carol", T1, "links https://a.example/1 https://a.example/2 https://a.example/3")
check("a new account: at most 2 links in a post", st == 400 and "At most 2 links" in b)
NOW[0] += 61
r = reply("carol", T1, "Thanks! https://a.example/1")
check("a reply: 303 to the post's anchor", r[0] == 303 and re.search(rf"/t/{T1}#p\d+$", r[1].get("Location", "")), r[:2])
st, _, b = reply("carol", T1, "and again")
check("the gap between posts (new: 60 s)", st == 429 and "Wait" in b)
for i in range(6):
    NOW[0] += 61
    reply("carol", T1, f"reply {i}")
NOW[0] += 61
st, _, b = reply("carol", T1, "one too many")
check("a new account: 8 posts an hour", st == 429 and "a lot of posts" in b, (st, b[-300:]))
NOW[0] += 73 * 3600
r1 = new_thread("carol", "general", "Third thread", "after three days")
NOW[0] += 16
r2 = new_thread("carol", "general", "Fourth thread", "the normal limits now")
NOW[0] += 5
r3 = reply("carol", tid_of(r2), "too fast")
check("after 72 hours: the normal limits (a second thread soon after, a 15 s gap)", r1[0] == 303 and r2[0] == 303 and r3[0] == 429)
check("limits come from the database (a restart doesn't reset them)", F.check_rate.__doc__ is None and
      F.one("SELECT COUNT(*) FROM forum_posts WHERE author = 'WR000004'")[0] >= 9)

# staff
NOW[0] += 100
st, _, body = req("GET", "/wrath/leyline/forum/new", "staff1")
check("staff: Announcements offered", 'value="news"' in body)
r = new_thread("staff1", "news", "Leyline is live", "Welcome, everyone.")
TN = tid_of(r)
check("staff: an Announcements thread", r[0] == 303 and TN)
b = req("GET", f"/wrath/leyline/forum/t/{TN}")[2]
check("staff posts carry the Team badge", '<span class="fm-team">Team</span>' in b)
NOW[0] += 100
check("anyone may reply under an announcement", reply("bob", TN, "Great news")[0] == 303)

# the JSON API
st, _, doc = api(f"thread?id={T1}")
p0 = doc["posts"][0] if st == 200 else {}
check("API thread: posts with parts (text and links), the reply address", st == 200 and doc["thread"]["title"] == "Sharing my realm"
      and {"u": "https://i.example/shot.png"} in p0.get("parts", []) and p0.get("author") == "Alice"
      and doc["reply_url"] == f"https://play.example/wrath/leyline/forum/t/{T1}/reply"
      and set(doc["thread"]) == {"id", "cat", "title", "author", "tag", "created", "bumped", "replies", "pinned", "locked", "url"}, doc)
check("API thread: no account names (WR...) or website ids in it", "WR0000" not in json.dumps(doc) and '"alice"' not in json.dumps(doc))
st, _, doc = api("latest?n=3")
check("API latest: the most recently active first, at most n", st == 200 and len(doc["threads"]) == 3 and doc["threads"][0]["id"] == TN)
st, _, doc = api("search?q=sharing")
check("API search: titles", st == 200 and [x["id"] for x in doc["threads"]] == [T1], doc)
st, _, body = req("GET", "/wrath/leyline/forum/search?q=%25")
check("search: a % matches nothing (escaped), 200", st == 200 and "No thread titles match" in body)
st, _, body = req("GET", "/wrath/leyline/forum/search?q=Addon")
check("search page: titles found", st == 200 and "Addon pack for my Den" in body)

# reports -> the admin console's Forum tab
pid = F.posts(T2)[0][0]["id"]
st, _, body = req("GET", f"/wrath/leyline/forum/report?p={pid}", "alice")
t = re.search(r'name="t" value="([0-9a-f]{32})"', body).group(1)
r = req("POST", "/wrath/leyline/forum/report", "alice", {"t": t, "p": pid, "reason": "spam", "note": "ads"})
check("report: 303 back to the post with a thank-you", r[0] == 303 and "reported=1" in r[1].get("Location", ""))
r = req("POST", "/wrath/leyline/forum/report", "alice", {"t": t, "p": pid, "reason": "spam"})
check("reporting twice: kept once", F.one("SELECT COUNT(*) FROM forum_reports")[0] == 1)
st, _, body = req("GET", f"/wrath/leyline/forum/report?p={pid}", "bob")
check("no Report on your own post", f"/report?p={pid}" not in req("GET", f"/wrath/leyline/forum/t/{T2}", "bob")[2])
st, _, body = req("GET", "/wrath/admin/forum", "alice")
check("the Forum tab: staff only", st == 403)
st, _, body = req("GET", "/wrath/admin/forum", "staff1")
check("the Forum tab: the reported post, its reason and the thread", st == 200 and "Reported posts" in body and "spam" in body
      and "Addon pack for my Den" in body and "forum.post.hide" in body, body[:500])
check("the dashboard counts open forum reports", re.search(r"<b>1</b><span>Open forum reports", req("GET", "/wrath/admin/numbers", "staff1")[2]))
csrf = re.search(r'name="csrf" value="([0-9a-f]+)"', body).group(1)


def act(action, target, **f):
    d = {"csrf": csrf, "action": action, "target": str(target), "back": "/wrath/admin/forum"}
    d.update(f)
    st, hd, _ = req("POST", "/wrath/admin/act", "staff1", d)
    return st == 303 and "ok=Done" in hd.get("Location", ""), hd.get("Location", "")


check("hide the post", act("forum.post.hide", pid)[0])
b = req("GET", f"/wrath/leyline/forum/t/{T2}")[2]
check("a hidden post: removed by a moderator, its text gone", "removed by a moderator" in b and "Addon" in b and "**** yes" not in b)
st, _, doc = api(f"thread?id={T2}")
check("API: a hidden post has no text", doc["posts"][0]["hidden"] and doc["posts"][0]["text"] == "")
rid = F.reports()[0]["id"]
check("the report: done", act("forum.report.done", rid, outcome="hidden")[0] and F.reports("open") == [])
check("show the post again", act("forum.post.unhide", pid)[0] and not F.post(pid)["hidden"])
check("lock", act("forum.thread.lock", T1)[0] and F.thread(T1)["locked"])
NOW[0] += 100
st, _, b = reply("bob", T1, "after the lock")
check("a locked thread: no reply form, a reply refused", "This thread is locked" in req("GET", f"/wrath/leyline/forum/t/{T1}", "bob")[2]
      and st == 403)
check("unlock", act("forum.thread.unlock", T1)[0] and not F.thread(T1)["locked"])
NOW[0] += 100
r = new_thread("bob", "help", "Sharing my realm (again)", "Same question.")
TD = tid_of(r)
NOW[0] += 100
check("pin", act("forum.thread.pin", T1)[0])
st, _, doc = api("threads?cat=help")
check("pinned threads first in their category", st == 200 and doc["threads"][0]["id"] == T1 and doc["threads"][0]["pinned"])
ok, loc = act("forum.thread.merge", TD, into=str(T1))
check("merge without the typed confirm: refused", not ok and "confirm" in urllib.parse.unquote(loc))
n_before = len(F.posts(T1)[0])
ok, _ = act("forum.thread.merge", TD, into=str(T1), confirm=str(TD))
check("merge a duplicate into another: its posts move over", ok and len(F.posts(T1)[0]) == n_before + 1 and F.thread(TD)["merged_into"] == T1)
st, hd, _ = req("GET", f"/wrath/leyline/forum/t/{TD}")
check("the merged thread's address: 301 to the one it went into", st == 301 and hd.get("Location") == f"/wrath/leyline/forum/t/{T1}")
st, _, doc = api(f"thread?id={TD}")
check("API thread on a merged id: the merged-into thread", st == 200 and doc["thread"]["id"] == T1)
check("merged threads leave the lists and the search", TD not in [x["id"] for x in api("threads?cat=help")[2]["threads"]]
      and api("search?q=again")[2]["threads"] == [])
check("merging into itself: refused", not act("forum.thread.merge", T1, into=str(T1), confirm=str(T1))[0])
check("hide a thread (typed confirm)", act("forum.thread.hide", T2, confirm=str(T2))[0] and req("GET", f"/wrath/leyline/forum/t/{T2}")[0] == 404)
check("show it again", act("forum.thread.unhide", T2)[0] and req("GET", f"/wrath/leyline/forum/t/{T2}")[0] == 200)
check("nothing deleted: every post is still in the database", F.one("SELECT COUNT(*) FROM forum_posts")[0] >= 15)
aud = [json.loads(x) for x in open(os.path.join(TMP, "admin", "audit.jsonl"))]
check("forum actions are in the audit log", any(a["action"] == "forum.thread.merge" and a["ok"] for a in aud))

# ---------------------------------------------------------------- the hub: nav, footer, front page
st, _, front = req("GET", "/wrath/leyline-preview")
nav = re.search(r'<nav class="hb-nav".*?</nav>', front, re.S).group(0)
check("hub nav: Forum (with its icon) where Community was, pointing at the forum",
      '<a class="hb-ni" href="/wrath/leyline/forum" title="Forum"><svg' in nav and ">Community<" not in nav, nav[:800])
foot = re.search(r'<footer.*?</footer>', front, re.S).group(0)
check("footer: Forum, not Community", ">Forum</a>" in foot and ">Community</a>" not in foot)
sec = re.search(r'<section class="hb-sec hb-cm hb-rv" id="forum".*?</section>', front, re.S)
sec = sec.group(0) if sec else ""
check("front page: Forum and Discord, a big Join the Discord first, then the latest threads",
      '<h2 id="hb-cm-h">Forum and Discord</h2>' in sec and 'class="hb-cm-dc big"' in sec and "Join the Discord" in sec
      and "Latest on the forum" in sec and "Leyline is live" in sec and sec.index("hb-cm-dc big") < sec.index("Latest on the forum"), sec[:1500])
check("front page: no friends features", not re.search(r"(?i)\bfriends? list|add friend|friend request|appear (online|offline)", front))
for path in ("/wrath/leyline/forum", f"/wrath/leyline/forum/t/{T1}", "/wrath/leyline/forum/c/help", "/wrath/leyline/forum/new",
             f"/wrath/leyline/forum/t/{T1}/reply"):
    st, _, b = req("GET", path, "alice")
    vis = re.sub(r"<style>.*?</style>|https?://\S+", "", b, flags=re.S)
    check(f"no game or company names on {path}", st == 200 and not BRAND.search(vis), BRAND.findall(vis))

# ---------------------------------------------------------------- the deploy step (signin)
fl = open(os.path.join(STREAM, "fastloop.sh"), encoding="utf-8").read()
bat = open(os.path.join(STREAM, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
check("fastloop.sh: installs forum.py, backs it up, puts it back", 'install -m 644 forum.py "$WU/portal/forum.py"' in fl
      and 'cp -p "$WU/portal/forum.py" "$BK/forum.py"' in fl and 'cp -p "$BK/forum.py" "$WU/portal/forum.py"' in fl)
check("the sign-in bat packs forum.py", 'if exist forum.py set "LINKS=%LINKS% forum.py"' in bat and " forum.py " in bat)

# ---------------------------------------------------------------- the launcher posts with its own sign-in (Bearer token)
S = wp.social.store


def launcher_token(acct):
    d = S.device_start("10.9.9.%d" % (len(acct) % 200))
    S.device_answer(S.device_lookup(d["user_code"]), acct, True)
    return S.device_poll(d["device_code"])["token"]


def tpost(sub, doc, token=None, cookie=None, origin=None):
    c = http.client.HTTPConnection("127.0.0.1", PAGE, timeout=30)
    h = {"X-Forwarded-For": "10.0.0.8", "Content-Type": "application/json"}
    if token:
        h["Authorization"] = "Bearer " + token
    if cookie:
        h["Cookie"] = "wr_session=" + cookie
    if origin:
        h["Origin"] = origin
        h["Sec-Fetch-Site"] = "cross-site"
    c.request("POST", "/wrath/forum/api/" + sub, body=json.dumps(doc).encode(), headers=h)
    r = c.getresponse()
    hd, b = dict(r.getheaders()), r.read().decode()
    c.close()
    try:
        return r.status, hd, json.loads(b)
    except ValueError:
        return r.status, hd, {}


ACCOUNTS["dave"] = ("dave", "Dave", "WR000006")
TOK = launcher_token("WR000004")   # carol, through the launcher only
NOW[0] += 4000
st, hd, doc = tpost("new", {"cat": "help", "title": "Posted from the launcher", "body": "Hello from the app https://example.org/x"}, TOK)
TL = (doc.get("thread") or {}).get("id")
check("launcher: a new thread with the Bearer token (no cookie, no form token)", st == 200 and doc.get("ok") and TL
      and F.thread(TL)["author_acct"] == "WR000004" and "Access-Control-Allow-Origin" not in hd, doc)
check("launcher: the poster's Leyline name on the thread", F.thread(TL)["author"] == wp.social.store.profile("WR000004")["name"])
NOW[0] += 100
st, _, doc = tpost("reply", {"thread": TL, "body": "and a reply"}, TOK)
check("launcher: a reply", st == 200 and doc.get("ok") and doc.get("thread") == TL and F.post(doc["post"])["author"] == "WR000004", doc)
st, _, doc = tpost("reply", {"thread": TL, "body": "too fast"}, TOK)
check("launcher: the same rate limits as the forms (a new account waits a minute)", st == 429 and "Wait" in doc.get("error", ""), doc)
NOW[0] += 100
st, _, doc = tpost("new", {"cat": "news", "title": "Not the team", "body": "x"}, TOK)
check("launcher: Announcements stays the team's", st == 403, doc)
st, _, doc = tpost("new", {"cat": "help", "title": "Warcraft tips", "body": "x"}, TOK)
check("launcher: the title rules (game names refused)", st == 400 and "game and company names" in doc.get("error", ""), doc)
st, _, doc = tpost("new", {"cat": ["help"], "title": {"a": 1}, "body": 5}, TOK)
check("launcher: odd JSON values refused plainly", st == 400, doc)
st, _, doc = tpost("reply", {"thread": TL, "body": "x" * 5001}, TOK)
check("launcher: the length limit", st == 400 and "5000" in doc.get("error", ""), doc)
bob_post = F.posts(T1)[0][0]["id"]
st, _, doc = tpost("report", {"post": bob_post, "reason": "spam", "note": "from the app"}, TOK)
check("launcher: a report", st == 200 and "moderators" in doc.get("text", ""), doc)
st, _, doc = tpost("report", {"post": doc and F.posts(TL)[0][0]["id"], "reason": "spam"}, TOK)
check("launcher: no reporting your own post", st == 400, doc)
st, _, doc = tpost("reply", {"thread": TL, "body": "cookie only"}, cookie="alice")
check("CSRF: the website cookie alone can't post to the JSON (Bearer only)", st == 401 and F.one("SELECT COUNT(*) FROM forum_posts WHERE body = 'cookie only'")[0] == 0)
st, _, doc = tpost("reply", {"thread": TL, "body": "cross"}, cookie="alice", origin="https://evil.example")
check("CSRF: a cross-site page with the visitor's cookie: refused", st == 401)
st, _, doc = tpost("reply", {"thread": TL, "body": "x"}, "A" * 43)
check("an unknown token: 401 (the launcher signs out)", st == 401 and "Sign in again" in doc.get("error", ""))
st, _, doc = tpost("mine", {}, TOK)
check("launcher: mine lists the player's own threads only", st == 200 and [t["id"] for t in doc.get("threads", [])][:1] == [TL]
      and all(F.thread(t["id"])["author_acct"] == "WR000004" for t in doc["threads"]), doc)
st, _, doc = tpost("mine", {})
check("launcher: mine needs the Bearer token", st == 401)
st, _, doc = tpost("categories", {}, TOK)
check("the reading calls stay GET only", st == 405)
# the staff badge from the website survives a launcher post; a token never carries staff powers
TS = launcher_token("WR000003")
NOW[0] += 100
st, _, doc = tpost("new", {"cat": "news", "title": "Team news", "body": "x"}, TS)
check("launcher: a staff token posts as a player (Announcements from the website only)", st == 403)
st, _, doc = tpost("reply", {"thread": TL, "body": "Team here"}, TS)
check("launcher: the Team badge (set by the website sign-in) stays on a launcher post",
      st == 200 and F.post(doc["post"])["staff"], doc)
# a site-wide sanction stops launcher posts too; "noforum" stops forum posts only
A = wp.admin
owner = {"account": "staff1", "name": "Mod Sam", "role": "admin", "session": "s"}
A.set_sanction(owner, "noforum", "WR000004", "", 2, "spam in the forum")
NOW[0] += 100
st, _, doc = tpost("reply", {"thread": TL, "body": "muted?"}, TOK)
check("muted from the forum: a launcher reply refused, with the reason", st == 403 and "spam in the forum" in doc.get("error", ""), doc)
check("muted from the forum: messages are not stopped", A.blocked("message", acct="WR000004") == "")
check("muted from the forum: the website's form refused too", "spam in the forum" in wp.forum._sanctioned({"acct": "WR000004"}, "new"))
st, _, doc = tpost("report", {"post": bob_post, "reason": "other"}, TOK)
check("muted from the forum: reporting still works", st == 200, doc)

# ---------------------------------------------------------------- the header and the forum show the Leyline name at once
wp.social._who_cache.clear()
FM._who_cache.clear()
st, _, b = req("GET", "/wrath/leyline/forum", "dave")
pre = wp.social.store.profile("WR000006")["name"]
wp.social.store.set_profile("WR000006", {"name": "DaveTheBold"})
st, _, b = req("GET", "/wrath/leyline/forum/new", "dave")
check("a rename shows at once in the forum (no minute's wait) and in the site header", "Post, as DaveTheBold" in b
      and 'class="hb-acct-n">DaveTheBold<' in b, re.findall(r'hb-acct-n">[^<]*', b))

# ---------------------------------------------------------------- Leyline HQ's Forum section
ACCOUNTS["mod1"] = ("mod1", "Mo Mod", "WR000005")
os.environ["WU_MODS_ADMINS"] = "mod1"
st, _, body = req("GET", "/wrath/admin/forum", "alice")
check("HQ Forum: staff only", st == 403)
st, _, body = req("GET", "/wrath/admin/forum", "mod1")
check("HQ Forum: a mod gets in; Forum in HQ's main menu, lit", st == 200 and re.search(r'<a href="/wrath/admin/forum" class=on aria-current=page>Forum</a>', body)
      and "Reported posts" in body and "Threads" in body and "Hidden posts" in body, body[:400])
check("HQ Forum: the report from the launcher, with hide, done, dismiss and the poster's mute", "from the app" in body and "forum.post.hide" in body
      and "forum.report.done" in body and "forum.report.dismiss" in body and 'value="forum.mute"' in body)
check("HQ Forum: a mod sees no account ban (admins only)", 'name="type" value="suspend"' not in body)
mcsrf = re.search(r'name="csrf" value="([0-9a-f]+)"', body).group(1)


def hq_act(who, csrf_, action, target, back="/wrath/admin/forum", **f):
    d = {"csrf": csrf_, "action": action, "target": str(target), "back": back}
    d.update(f)
    st, hd, _ = req("POST", "/wrath/admin/act", who, d)
    loc = urllib.parse.unquote(hd.get("Location", ""))
    return "ok=Done" in loc, loc


ok, loc = hq_act("mod1", mcsrf, "forum.mute", "WR000002", hours="72", reason="flooding")
check("mute from the forum: needs the typed confirm", not ok and "Type WR000002" in loc, loc)
ok, loc = hq_act("mod1", mcsrf, "forum.mute", "WR000002", hours="500", reason="flooding", confirm="WR000002")
check("mute from the forum: a mod, at most a week", not ok and "168" in loc, loc)
ok, loc = hq_act("mod1", mcsrf, "forum.mute", "WR000002", hours="24", reason="flooding", confirm="WR000002")
check("mute from the forum: a mod mutes bob for a day", ok and "flooding" in A.blocked("post", acct="WR000002")
      and A.blocked("message", acct="WR000002") == "", loc)
NOW[0] += 100
st, _, b = reply("bob", T1, "am I muted")
check("muted: the website's reply form refuses", st == 403 and "flooding" in b)
ok, loc = hq_act("mod1", mcsrf, "sanction.set", "WR000002", type="suspend", hours="0", reason="ban", confirm="WR000002")
check("ban the account: admins only (a mod is refused, and it's audited)", not ok and "admins" in loc.lower())
ok, loc = hq_act("mod1", mcsrf, "forum.unmute", "WR000002")
check("lift the forum mute", ok and A.blocked("post", acct="WR000002") == "", loc)
st, _, body = req("GET", "/wrath/admin/forum?acct=WR000004", "mod1")
check("a poster's forum history: their posts, the reports about them, their forum mute",
      st == 200 and "Posted from the launcher" in body and "and a reply" in body and "spam in the forum" in body
      and "forum.unmute" in body and "their page" in body, body[-1500:])
st, _, body = req("GET", "/wrath/admin/forum?view=threads", "mod1")
check("HQ Forum threads: pin, lock, hide; merge is the admins'", st == 200 and "forum.thread.pin" in body and "forum.thread.lock" in body
      and "forum.thread.hide" in body and "forum.thread.merge" not in body)
st, _, body = req("GET", "/wrath/admin/forum?view=threads", "staff1")
check("HQ Forum threads: an admin can merge", "forum.thread.merge" in body)
st, _, body = req("GET", "/wrath/admin/forum?view=hidden", "mod1")
check("HQ Forum hidden posts", st == 200 and "Hidden posts" in body)
st, _, body = req("GET", "/wrath/admin/inbox", "mod1")
check("Needs you: the forum report with Hide and Mute from the forum", "Forum report" in body and "forum.mute" in body)
st, _, body = req("GET", "/wrath/admin/person?acct=WR000004", "mod1")
check("a person's page: On the forum, with a link to their history", "On the forum" in body and "/wrath/admin/forum?acct=WR000004" in body)


def hq_api(who, path):
    st, _, b = req("GET", "/wrath/admin/api/" + path, who)
    try:
        return st, json.loads(b)
    except ValueError:
        return st, {}


st, doc = hq_api("mod1", "forum")
check("HQ API forum: the open reports and the actions a mod may call", st == 200 and doc["ok"] and any(r["post_text"] for r in doc["reports"])
      and "mute" in doc["actions"] and "merge" not in doc["actions"] and "ban" not in doc["actions"], doc)
st, doc = hq_api("staff1", "forum?view=poster&acct=WR000004")
check("HQ API forum poster", st == 200 and doc["poster"]["posts"] >= 2 and doc["muted"] and "ban" in doc["actions"], doc)
st, doc = hq_api("staff1", "forum?view=threads")
check("HQ API forum threads", st == 200 and any(t["id"] == TL for t in doc["threads"]))
st, doc = hq_api("alice", "forum")
check("HQ API forum: staff only", st == 403)
st, _, meb = req("GET", "/wrath/admin/api/me", "mod1")
mtok = json.loads(meb)["csrf"]
c = http.client.HTTPConnection("127.0.0.1", PAGE, timeout=30)
c.request("POST", "/wrath/admin/api/act", body=json.dumps({"action": "forum.thread.lock", "target": str(TL), "csrf": mtok}).encode(),
          headers={"Cookie": "wr_session=mod1", "Content-Type": "application/json", "Origin": "https://play.example", "X-Forwarded-For": "10.0.0.9"})
r = c.getresponse(); jb = json.loads(r.read()); c.close()
check("HQ API act: a mod locks a thread through the JSON", r.status == 200 and jb["ok"] and F.thread(TL)["locked"], jb)
aud = [json.loads(x) for x in open(os.path.join(TMP, "admin", "audit.jsonl"))]
check("the audit log: who muted, the refused ban, the API lock", any(a["action"] == "forum.mute" and a["ok"] and a["who"] == "mod1" for a in aud)
      and any(a["action"] == "sanction.set" and not a["ok"] and a["who"] == "mod1" for a in aud)
      and any(a["action"] == "forum.thread.lock" and a["ok"] and a["target"] == str(TL) for a in aud))

# ---------------------------------------------------------------- the same text again, and one network's new accounts
F.seen("WR000099", "Dupe")
NOW[0] += 100
TA = F.new_thread("WR000099", "general", "Where do I find the dens", "Looking for a den to join, anyone around tonight?")
NOW[0] += 100
F.reply("WR000099", TA, "Thanks!")
NOW[0] += 100
try:
    F.reply("WR000099", TA, "Thanks!"); dup_same = False
except FM.ForumError as ex:
    dup_same = ex.status == 400
check("the same reply twice in one thread is refused", dup_same)
TB = TL if not F.thread(TL)["locked"] else T1
F.set_thread(TB, "locked", False)
NOW[0] += 100
F.reply("WR000099", TB, "Thanks!")
check("a short reply in another thread is fine", True)
NOW[0] += 100
try:
    F.reply("WR000099", TB, "Looking for a den to join, anyone around tonight?"); dup_long = False
except FM.ForumError as ex:
    dup_long = ex.status == 400 and "already" in ex.text
check("a long post repeated in another thread is refused", dup_long)
NOW[0] += 4000
try:
    F.new_thread("WR000099", "help", "Where do I find the dens", "Another try at the same title, different words."); dup_title = False
except FM.ForumError as ex:
    dup_title = ex.status == 400
check("a second thread with the same title the same day is refused", dup_title)
NOW[0] += 86400 * 2
F.reply("WR000099", TA, "Thanks!")
check("the same text a day later is fine", True)

del wp.browse_limit.hit   # the real limiter again (memory, per network)
wp.browse_limit.clear()
NOW[0] += 86400
for i in range(6):
    ACCOUNTS[f"net{i}"] = (f"net{i}", f"Net{i}", f"WR00010{i}")
codes = [new_thread(f"net{i}", "general", f"Fresh account number {i}", f"Hello from fresh account {i}.")[0] for i in range(5)]
check(f"one network's new accounts: {FM.NET_NEW_THREADS_H} new threads an hour, then 429", codes == [303] * FM.NET_NEW_THREADS_H + [429], codes)
got = []
for n in range(FM.NET_NEW_POSTS_H):
    NOW[0] += 61
    got.append(reply(f"net{n % 6}", TA, f"Reply number {n} from a fresh account.")[0])
check(f"... and {FM.NET_NEW_POSTS_H} posts an hour together (threads count), then 429",
      got[:FM.NET_NEW_POSTS_H - FM.NET_NEW_THREADS_H] == [303] * (FM.NET_NEW_POSTS_H - FM.NET_NEW_THREADS_H)
      and got[FM.NET_NEW_POSTS_H - FM.NET_NEW_THREADS_H] == 429, got)
NOW[0] += 1
st_old = reply("alice", TA, "An established account still posts from the same network.")[0]
check("an account that isn't new still posts from that network", st_old == 303, st_old)
check("the per-network keys are the network's /64 for IPv6", FM._net(type("H", (), {"ip": lambda s: "2001:db8:1:2::5"})()) == "2001:db8:1:2::/64")
wp.browse_limit.hit = lambda *a, **k: True

# ---------------------------------------------------------------- a thread over more than one page
while F.posts(TA)[2] < 2:
    F.reply("WR000003", TA, f"Team note {F.thread(TA)['replies']}", staff=True)
_, _, p1 = req("GET", f"/wrath/leyline/forum/t/{TA}", "bob")
_, _, p2 = req("GET", f"/wrath/leyline/forum/t/{TA}?page=2", "bob")
_, _, pz = req("GET", f"/wrath/leyline/forum/t/{TA}?page=abc")
check("thread pages: page 1 has Later and no reply form", "Page 1 of 2" in p1 and f"?page=2" in p1 and 'name="body"' not in p1)
check("... the last page has Earlier and the reply form", "Page 2 of 2" in p2 and "Earlier" in p2 and 'name="body"' in p2)
check("... a bad page number reads as page 1, signed out", "Page 1 of 2" in pz and "Sign in to reply" not in pz)
last = F.posts(TA, 2)[0][-1]["id"]
check("page_of finds a post's page (the reply's redirect)", F.page_of(last) == 2)

print(f"{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
