"""The Leyline Admin Console (stream/admin.py hooked into wrath_portal.py), end to end on a local portal with a fake
website sign-in, a fake realm SOAP (answering ov_admin.lua's "ovadmin" words; the Lua side has its own test,
stream/tests/test_ov_admin.lua), a fake Beacons relay and a fake GitHub releases API.

Covers: who gets in (owner, staff file, WU_MODS_ADMINS, GM level, everyone else refused), noindex, CSRF on every POST,
same-origin, the typed confirm, owner-only actions, the audit log (who, what, when, before, after), rate limits, every
section's actions (catalog, Workshop, index, realms, releases and channel, news, flags, themes, reports, staff),
the public feeds, the overrides the public pages honour, no secrets on the pages, nothing deleted.

    python3 -I stream/tests/test_admin.py stream/wrath_portal.py
"""
import http.client, importlib.util, json, os, re, shutil, subprocess, sys, tempfile, threading, time, urllib.parse, zipfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from xml.sax.saxutils import escape

PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
ROOT = os.path.dirname(STREAM)

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, str(extra)[:600])


T = tempfile.mkdtemp(prefix="wu-admin-")
DATA, MODS, WSDIR = os.path.join(T, "data"), os.path.join(T, "mods"), os.path.join(T, "workshop")
for d in (DATA, MODS, WSDIR, os.path.join(DATA, "hub")):
    os.makedirs(d, exist_ok=True)

# ---------------------------------------------------------------- the catalog: the real mods, packed by wu-mod.py
r = subprocess.run([sys.executable, "-I", os.path.join(ROOT, "mods", "wu-mod.py"), "catalog", os.path.join(ROOT, "mods", "catalog"),
                    "-o", os.path.join(MODS, "catalog.json"), "--dist", os.path.join(MODS, "files")], capture_output=True, text=True)
check("setup: wu-mod.py catalog", r.returncode == 0, r.stdout + r.stderr)
cat = json.load(open(os.path.join(MODS, "catalog.json")))
# a bad mod: an MPQ inside its zip, which the checker must find
with zipfile.ZipFile(os.path.join(MODS, "files", "badmod-1.0.0.zip"), "w") as z:
    z.writestr("badmod/mod.json", json.dumps({"id": "badmod", "name": "Bad", "version": "1.0.0", "kind": "ui"}))
    z.writestr("badmod/ui/Bad/data.mpq", b"MPQ\x1a" + b"\0" * 60)
cat["mods"].append({"id": "badmod", "name": "Bad mod", "version": "1.0.0", "kind": "ui", "zip": "badmod-1.0.0.zip",
                    "description": "x", "author": "x", "licence": "MIT"})
json.dump(cat, open(os.path.join(MODS, "catalog.json"), "w"))
cdoc = json.load(open(os.path.join(ROOT, "mods", "categories.json")))
if "themes" not in [c["id"] for c in cdoc["categories"]]:   # the themes agent's 13th category (docs/THEMES.md)
    cdoc["categories"].append({"id": "themes", "name": "Themes", "line": "Launcher theme packs.", "icon": ""})
json.dump(cdoc, open(os.path.join(MODS, "categories.json"), "w"))
CATS = [c["id"] for c in cdoc["categories"]]

PID = "3f9a0c12bd47"
open(os.path.join(DATA, "projects.jsonl"), "w").write(json.dumps({
    "id": PID, "url": "https://github.com/alice/frame-kit", "host": "github.com", "forge": "github", "title": "frame-kit",
    "author": "alice", "author_url": "https://github.com/alice", "licence": "MIT", "description": "Tidy unit frames for raids.",
    "category": CATS[0], "builds": ["3.3.5a"], "stars": 4, "last_push": "", "submitted_by": "Alice", "submitter": "acc-alice",
    "submitted_at": "2026-10-25T11:58:00Z", "last_checked": "", "reports": ["acc-bob", "acc-cid"], "hidden": False}) + "\n")

json.dump({"updated": int(time.time()), "items": [
    {"id": 1, "kind": "course", "title": "Windmill Nine", "author": "Ann", "descr": "", "version": 1, "created": 100, "plays": 50, "up": 3, "down": 0},
    {"id": 2, "kind": "build", "title": "Cozy Inn", "author": "Bob", "descr": "", "version": 1, "created": 200, "plays": 1, "up": 0, "down": 0},
]}, open(os.path.join(WSDIR, "items.json"), "w"))

HUB_INDEX = os.path.join(T, "community-index.json")
H1, H2, HE = "0123456789ab", "ba9876543210", "aaaaaaaaaaaa"
json.dump({"format": 1, "entries": [
    {"id": "link:" + HE, "name": "Individual Progression", "repo": "https://github.com/x/y", "licence": "MIT",
     "category": "presets", "type": "module", "description": "d", "updated": "2026-10-01"}]}, open(HUB_INDEX, "w"))
open(os.path.join(DATA, "hub", "submissions.jsonl"), "w").write(
    json.dumps({"id": H1, "repo": "https://github.com/c/cool-addon", "build": "3.3.5a", "category": "addons",
                "note": "A cool addon", "at": "2026-10-29T10:00:00Z", "via": "portal"}) + "\n"
    + json.dumps({"id": H2, "repo": "https://github.com/m/meh", "build": "1.12", "category": "tools", "note": "", "at": "x", "via": "portal"}) + "\n")


class FakeSocial:
    """stream/social.py's Social.reports / resolve_report, as the social agent defined them."""
    def __init__(self):
        self.rows = [{"id": 5, "at": 1761739200, "reporter": "acc-bob", "reporter_name": "Bob", "target": "acc-mal",
                      "target_name": "Mallory", "reason": "abuse", "note": "keeps going", "via": "launcher", "status": "open",
                      "handled_by": None, "handled_at": None, "outcome": None,
                      "context": [{"from": "acc-bob", "text": "PRIVATE-REPORTER-MESSAGE", "at": 1, "via": "game"},
                                  {"from": "acc-mal", "text": "OLDER-MALLORY-MESSAGE", "at": 2, "via": "game"},
                                  {"from": "acc-mal", "text": "you are <b>bad</b> REPORTED-BODY", "at": 3, "via": "game"},
                                  {"from": "acc-bob", "text": "PRIVATE-LATER-REPLY", "at": 4, "via": "game"}]}]

    def reports(self, status="open", limit=100):
        return [dict(r) for r in self.rows if not status or r["status"] == status][:limit]

    ingame = {"WRMAL": {"char": "Malchar", "since": 1}}
    toks = {"WRMAL": [{"id": "abcdefabcdef", "label": "Leyline launcher, linked 2026-10-30", "created": 1, "used": 2},
                      {"id": "123456123456", "label": "Leyline launcher, linked 2026-10-31", "created": 1, "used": 2}]}

    def q(self, sql, params=()):
        if "FROM social_chars WHERE acct" in sql:
            return [("Malchar",)] if params[0] == "WRMAL" else []
        if "FROM social_chars" in sql:
            return [("WRMAL",)] if params[0] == "malchar" else []
        if "FROM social_accounts" in sql:
            return [("WRMAL",)] if params[0] == "WRMAL" or params[1] == "mallory" or params[2].startswith("mal") else []
        return []

    def profile(self, acct):
        return {"name": "Mallory", "tag": "4242", "main": "Malchar"} if acct == "WRMAL" else None

    def tokens(self, acct):
        return [dict(t) for t in self.toks.get(acct, [])]

    def revoke(self, acct, short_id=None, tok=None):
        before = len(self.toks.get(acct, []))
        self.toks[acct] = [t for t in self.toks.get(acct, []) if t["id"] != short_id]
        return len(self.toks[acct]) < before

    def resolve_report(self, rid, by, outcome, status="done"):
        for r in self.rows:
            if r["id"] == rid:
                r.update(status=status, handled_by=by, outcome=outcome)
                return True
        return False


# theme packs: Workshop entries in the "themes" category (docs/THEMES.md)
TPID = "7e7e7e7e7e7e"
with open(os.path.join(DATA, "projects.jsonl"), "a") as f:
    f.write(json.dumps({"id": TPID, "url": "https://github.com/ann/frost-glass", "host": "github.com", "forge": "github",
                        "title": "Frost Glass", "author": "ann", "licence": "MIT", "description": "A cool blue theme pack.",
                        "category": "themes", "builds": ["any"], "submitted_by": "Ann", "submitter": "acc-ann",
                        "submitted_at": "2026-10-29T11:00:00Z", "last_checked": "", "reports": [], "hidden": False}) + "\n")

SOAP_PORT, RELAY_PORT, SITE_PORT, GH_PORT, PAGE_PORT = 17931, 17932, 17933, 17934, 17935
RID_A, RID_B = "abcdefghijklmnop", "qrstuvwxyz234567"
os.environ.update(
    SOAP_USER="WUSOAP", SOAP_PASS="sekritsoappass", AC_SOAP_URL=f"http://127.0.0.1:{SOAP_PORT}/",
    PORTAL_URL=f"http://127.0.0.1:{SITE_PORT}", PUBLIC_HOST="play.example", WU_STREAM="0", STREAM_HOST="wrath.example",
    WU_RELAY_LISTING=f"http://127.0.0.1:{RELAY_PORT}/api/listing/realms.json", WU_JOIN_BASE="https://relay.example",
    WU_PORTAL_DATA=DATA, WU_MODS_DIR=MODS, WU_WORKSHOP_ITEMS_DIR=WSDIR, WU_ADMIN_OWNERS="owner1",
    WU_MODS_ADMINS="modadm", WU_ADMIN_STAFF="staff1", WU_HUB_INDEX=HUB_INDEX,
    WU_LEYLINE_RELEASES_API=f"http://127.0.0.1:{GH_PORT}/repos/orionopenwow/Leyline/releases",
    WU_LEYLINE_RUNS_API=f"http://127.0.0.1:{GH_PORT}/runs", WU_ADDONPACK_URL=f"http://127.0.0.1:{GH_PORT}/addonpack.json", WU_HANGOUT_FILE=os.path.join(T, "none"))

# ---------------------------------------------------------------- fakes
soap_log = []
ws_items = {1: {"kind": "course", "state": "approved", "title": "Windmill Nine", "descr": ""},
            2: {"kind": "build", "state": "pending", "title": "Cozy Inn", "descr": ""},
            3: {"kind": "mode", "state": "pending", "title": "Shit Arena", "descr": ""}}
bugs = {7: {"status": "new", "text": "quest giver <script>alert(1)</script> has no mark"}, 6: {"status": "seen", "text": "stuck"}}
GM = {"WRGM": 3, "WRPLAYER": 0}


GAME_ANSWERS = {"kick": "Player {0} kicked.", "mute": "Player {0} muted.", "unmute": "Player {0} unmuted.",
                "tele": "Teleporting {2} to Dalaran.", "ban": "Account {1} is banned.", "unban": "Account {1} unbanned.",
                "announce": "", "notify": ""}


def soap_answer(cmd):
    w = cmd.split(" ")
    if w[0] in GAME_ANSWERS:
        if "Nobody" in cmd:
            return "Player Nobody not found."
        return GAME_ANSWERS[w[0]].format(*(w[1:] + ["", "", ""]))
    if cmd == "ticket list":
        return "Showing list of open tickets.\nTicket: 3. Created by: Ann. Created: 2h ago. <b>help</b> stuck"
    if cmd == "account onlinelist":
        return "WRMAL  Malchar  127.0.0.1\nRNDBOT12  Botty  127.0.0.1"
    m = re.match(r"^ovadmin (\S+)\s*(.*)$", cmd)
    if not m:
        return "Connected players: 4 (peak: 9). Queue: 2.\nCharacters in world: 10\nServer uptime: 1 hour(s)"
    word, rest = m.groups()
    if word == "ping":
        return "ovadmin ok\novadmin end 0"
    if word == "gm":
        n = rest.upper()
        return f"ovadmin gm {n} {GM.get(n, 0)}\novadmin end 1"
    if word == "ws":
        a = rest.split(" ", 3)
        if a[0] == "list":
            rows = [f"ROW\t{i}\t{it['kind']}\t{it['state']}\t1\t0\t0\t0\tAnn\t{1 if 'Shit' in it['title'] else 0}\t{it['title']}\t{it['descr']}"
                    for i, it in sorted(ws_items.items(), reverse=True) if a[1] == "all" or it["state"] == a[1]]
            return "\n".join(rows + [f"ovadmin end {len(rows)}"])
        if a[0] == "set":
            it = ws_items.get(int(a[1]))
            if not it:
                return "ovadmin error: no such item"
            it["state"] = {"approve": "approved", "reject": "rejected", "pending": "pending"}[a[2]]
            return f"ovadmin ok: #{a[1]} {it['state']}\novadmin end 0"
        if a[0] == "edit":
            ws_items[int(a[1])][a[2]] = a[3]
            return f"ovadmin ok: #{a[1]} {a[3]}\novadmin end 0"
        if a[0] == "check":
            f = ["title: filtered word"] if "Shit" in ws_items[int(a[1])]["title"] else []
            return "\n".join([f"FINDING\t{x}" for x in f] + [f"ovadmin end {len(f)}"])
    if word == "bugs":
        a = rest.split()
        if a[0] == "list":
            rows = [f"ROW\t{i}\t{b['status']}\t1700000000\tAnn\t12\t0\t12\t{b['text']}" for i, b in sorted(bugs.items(), reverse=True)
                    if a[1] == "all" or (a[1] == "open" and b["status"] in ("new", "seen")) or b["status"] == a[1]]
            return "\n".join(rows + [f"ovadmin end {len(rows)}"])
        if a[0] == "set":
            bugs[int(a[1])]["status"] = a[2]
            return f"ovadmin ok: bug {a[1]} {a[2]}\novadmin end 0"
        if a[0] == "count":
            c = {}
            for b in bugs.values():
                c[b["status"]] = c.get(b["status"], 0) + 1
            return "\n".join([f"COUNT\t{k}\t{v}" for k, v in c.items()] + [f"ovadmin end {len(c)}"])
    return "ovadmin error: usage"


class Soap(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_POST(self):
        body = self.rfile.read(int(self.headers["Content-Length"])).decode()
        cmd = re.search(r"<command>(.*)</command>", body, re.S).group(1).replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
        soap_log.append(cmd)
        out = escape(soap_answer(cmd))
        b = (f'<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" xmlns:ns1="urn:AC">'
             f'<SOAP-ENV:Body><ns1:executeCommandResponse><result>{out}</result></ns1:executeCommandResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>').encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)


GAME = {"owner1": "WROWNER", "staff1": "WRSTAFF", "modadm": "WRMOD", "gmguy": "WRGM", "player": "WRPLAYER", "newbie": "WRNEW"}


class Site(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        m = re.search(r"wr_session=([a-z0-9]+)", self.headers.get("Cookie", ""))
        who = m.group(1) if m else ""
        if who not in GAME:
            self.send_response(401)
            self.end_headers()
            return
        doc = {"id": who, "display_name": who.title()} if self.path == "/api/me" else \
            {"user": GAME[who], "pass": "GAMEPASS" + who.upper()}
        b = json.dumps(doc).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)


def rec(rid, name, live=True, players=3):
    return {"schema": 1, "name": name, "official": False, "state": "live" if live else "offline", "tagline": "t", "type": "Fun",
            "profile": {"ruleset": "dalaran"}, "community": {"realmId": rid, "claimed": True, "relay": "relay.example",
                                                            "firstSeen": "2026-10-20T00:00:00Z", "lastSeen": "2026-10-29T12:00:00Z",
                                                            "lastHeartbeat": {"online": live, "players": players, "bots": 1, "seq": 42}}}


class Relay(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        b = json.dumps({"schema": 1, "realms": [rec(RID_A, "Frost Haven"), rec(RID_B, "Quiet Corner", False, 0)]}).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)


RELEASES = [{"tag_name": "launcher-v0.5.0", "name": "Leyline 0.5.0", "draft": False, "prerelease": True,
             "published_at": "2026-10-29T10:00:00Z", "html_url": "https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.5.0",
             "assets": [{"name": "latest.json"}, {"name": "Leyline.msi"}]},
            {"tag_name": "launcher-v0.4.2", "name": "Leyline 0.4.2", "draft": False, "prerelease": False,
             "published_at": "2026-10-20T10:00:00Z", "html_url": "https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.4.2",
             "assets": [{"name": "latest.json"}]},
            {"tag_name": "launcher-v0.6.0", "name": "draft", "draft": True, "prerelease": False, "published_at": None,
             "html_url": "https://github.com/orionopenwow/Leyline/releases/tag/untagged", "assets": []}]


class GitHub(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        b = json.dumps({"workflow_runs": [{"name": "Leyline launcher", "status": "completed", "conclusion": "failure",
                                           "head_branch": "main", "created_at": "2026-10-31T00:00:00Z",
                                           "html_url": "https://github.com/orionopenwow/Leyline/actions/runs/1"}]} if self.path == "/runs"
                       else {"format": 1, "version": "2026.1031.9", "addons": [1, 2, 3]} if self.path == "/addonpack.json"
                       else RELEASES).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)


for port, h in ((SOAP_PORT, Soap), (SITE_PORT, Site), (RELAY_PORT, Relay), (GH_PORT, GitHub)):
    s = ThreadingHTTPServer(("127.0.0.1", port), h)
    threading.Thread(target=s.serve_forever, daemon=True).start()

spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wp)
A = wp.admin
LIMITS = (A.POSTS_PER_MIN, A.IP_PER_MIN, A.PAGES_PER_MIN)
A.POSTS_PER_MIN = A.IP_PER_MIN = A.PAGES_PER_MIN = 10 ** 6   # the rate limits have their own test at the end
import types
FAKE_SOCIAL = FakeSocial()
wp.social = types.SimpleNamespace(store=FAKE_SOCIAL, match=lambda path: False)   # what the portal has once stream/social.py is merged
check("the portal loads admin.py", A is not None and wp.workshop is not None and wp.workshop.adjust == A.adjust_catalog
      and wp.creations.adjust == A.adjust_catalog)
srv = wp.Server(("127.0.0.1", PAGE_PORT), wp.PageHandler)
threading.Thread(target=srv.serve_forever, daemon=True).start()


def req(method, path, who=None, form=None, origin="https://play.example", ctype="application/x-www-form-urlencoded", ip=None):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=30)
    h = {"X-Forwarded-For": ip or "10.0.0.1"}
    if who:
        h["Cookie"] = f"wr_session={who}"
    body = None
    if form is not None:
        body = urllib.parse.urlencode(form).encode()
        h["Content-Type"] = ctype
        if origin:
            h["Origin"] = origin
    c.request(method, path, body=body, headers=h)
    r = c.getresponse()
    out = (r.status, {k: v for k, v in r.getheaders()}, r.read().decode("utf-8", "replace"))
    c.close()
    return out


def token(who, path="/wrath/admin/catalog"):
    st, _, body = req("GET", path, who)
    m = re.search(r'name="csrf" value="([0-9a-f]+)"', body)
    return m.group(1) if m else ""


def act(who, action, target, back="/wrath/admin", **fields):
    f = {"csrf": fields.pop("csrf", None) or token(who), "action": action, "target": target, "back": back}
    f.update(fields)
    st, hd, body = req("POST", "/wrath/admin/act", who, f)
    loc = urllib.parse.unquote(hd.get("Location", ""))
    return st, loc


def ok_(r):
    return r[0] == 303 and "ok=Done" in r[1]


def audit():
    try:
        return [json.loads(x) for x in open(os.path.join(DATA, "admin", "audit.jsonl"))]
    except OSError:
        return []


def feed(name):
    st, hd, body = req("GET", f"/wrath/leyline/{name}")
    return st, hd, json.loads(body) if st == 200 else {}


# ---------------------------------------------------------------- who gets in
st, hd, _ = req("GET", "/wrath/admin")
check("signed out: to the sign-in, and back here", st == 302 and hd.get("Location") == "/login?next=%2Fwrath%2Fadmin")
st, hd, body = req("GET", "/wrath/admin", "player")
check("a player: 403 staff only, with their own account id", st == 403 and "Staff only" in body and "<code>player</code>" in body)
check("refusals are noindex too", hd.get("X-Robots-Tag", "").startswith("noindex") and 'name="robots" content="noindex' in body)
check("a player's page has no admin links", "/wrath/admin/act" not in body and "csrf" not in body)
st, hd, body = req("GET", "/wrath/admin", "owner1")
check("the owner: HQ's home", st == 200 and "Leyline HQ" in body and "Signed in as Owner1 (owner)" in body, body[:400])
check("noindex: header and meta", hd.get("X-Robots-Tag") == "noindex, nofollow" and '<meta name="robots" content="noindex,nofollow">' in body)
check("the page's one script carries the CSP nonce", re.search(r"script-src 'nonce-([^']+)'", hd.get("Content-Security-Policy", ""))
      and re.search(r"<script nonce=\"[^\"]+\">", body) and "no-store" in hd.get("Cache-Control", ""))
st, _, body = req("GET", "/wrath/admin", "modadm")
check("WU_MODS_ADMINS (the Workshop's admins) are mods", st == 200 and "(mod)" in body)
st, _, body = req("GET", "/wrath/admin", "gmguy")
check("a GM level 3 game account is a mod (ovadmin gm over SOAP)", st == 200 and "(mod)" in body and "ovadmin gm WRGM" in soap_log)
os.environ["WU_ADMIN_GM_LEVEL"] = "0"
A._gm_cache.clear()
check("WU_ADMIN_GM_LEVEL=0 turns the GM rule off", req("GET", "/wrath/admin", "gmguy")[0] == 403)
os.environ["WU_ADMIN_GM_LEVEL"] = "4"
A._gm_cache.clear()
check("GM level below the minimum: refused", req("GET", "/wrath/admin", "gmguy")[0] == 403)
del os.environ["WU_ADMIN_GM_LEVEL"]
A._gm_cache.clear()

st, _, body = req("GET", "/wrath/admin", "staff1")
check("WU_ADMIN_STAFF are admins", st == 200 and "(admin)" in body)

# ---------------------------------------------------------------- dashboard (System > Numbers)
st, _, body = req("GET", "/wrath/admin/numbers", "owner1")
check("dashboard: realm numbers from status", "Players online" in body and "<b>4</b><span>Players online" in body
      and "<b>2</b><span>Login queue" in body, re.findall(r"<b>[^<]*</b><span>[^<]*", body))
check("dashboard: relay realms counted", "<b>2</b><span>Community realms" in body)
check("dashboard: queues (Workshop 2 waiting, 2 index submissions)", "<b>2</b><span>Workshop waiting" in body
      and "<b>2</b><span>Index submissions" in body and "<b>0</b><span>Theme packs waiting" in body)
check("dashboard: open reports (2 bugs, 1 message, 1 link)", "<b>2</b><span>Open .bug reports" in body
      and "<b>1</b><span>Open message reports" in body and "<b>1</b><span>Reported links" in body)
check("dashboard: catalog counts", f"<b>{len(cat['mods'])}</b><span>Packaged mods" in body and "<b>2</b><span>Community links" in body
      and "<b>2</b><span>Workshop items live" in body and "<b>1</b><span>Index entries" in body)
check("dashboard: launcher versions are not collected (no telemetry)", "Not collected" in body and "no telemetry" in body)
check("dashboard: the game data status line (read-only: no switch on the page)", "Game data streaming: <b>on</b>" in body
      and "gamedata" not in re.sub(r'<p class="ad-gamedata">.*?</p>', "", body).replace("ad-gamedata", ""))
os.makedirs(os.path.join(DATA, "admin"), exist_ok=True)
json.dump({"versions": {"0.4.2": 12, "0.5.0": 3, "junk<": 9}}, open(os.path.join(DATA, "admin", "launcher-versions.json"), "w"))
body = req("GET", "/wrath/admin/numbers", "owner1")[2]
check("dashboard: launcher versions from an update-check summary, when one exists", "<td>0.4.2</td><td>12</td>" in body and "junk" not in body)

TABS = ["catalog", "workshop", "hub", "realms", "releases", "flags", "themes", "reports", "health", "staff", "audit", "catalog/mod/pings"]
res = {t: req("GET", "/wrath/admin/" + t, "owner1") for t in TABS}
check("every section renders for the owner", all(r[0] == 200 for r in res.values()), {t: r[0] for t, r in res.items()})
check("every section renders for staff", all(req("GET", "/wrath/admin/" + t, "modadm")[0] == 200 for t in TABS))
check("every section refuses a player", all(req("GET", "/wrath/admin/" + t, "player")[0] == 403 for t in TABS))
st, hd, body = req("POST", "/wrath/admin/act", "owner1", {"csrf": token("owner1"), "action": "news.hide", "target": "zz", "confirm": "zz"})
loc = hd.get("Location", "")
check("after an action: the message, signed", st == 303 and "&m=" in loc and "No such post" in req("GET", loc, "owner1")[2])
check("a crafted message link shows nothing", "Call 555" not in req("GET", "/wrath/admin?err=Call+555+now&m=0000", "owner1")[2]
      and "Call 555" not in req("GET", "/wrath/admin?ok=Call+555", "owner1")[2])
check("an unknown section: 404, still noindex", req("GET", "/wrath/admin/nope", "owner1")[0] == 404)
allpages = "".join(r[2] for r in res.values())
check("no secrets on any page (SOAP login, game passwords, the CSRF key)", "sekritsoappass" not in allpages and "GAMEPASS" not in allpages
      and A._secret().hex() not in allpages and "WUSOAP" not in allpages)
check("the Workshop page says 'Nothing is deleted'", "Nothing is deleted" in res["workshop"][2])

# ---------------------------------------------------------------- CSRF, origin, form shape
n0 = len(audit())
st, _, body = req("POST", "/wrath/admin/act", "owner1", {"action": "flag.set", "target": "p2p_hosting", "on": "1", "confirm": "p2p_hosting"})
check("POST without a CSRF token: 403", st == 403 and "out of date" in body)
check("... and it is in the audit log as refused", audit()[-1]["ok"] is False and "CSRF" in audit()[-1]["note"] and len(audit()) == n0 + 1)
st, _, _ = req("POST", "/wrath/admin/act", "owner1", {"csrf": "0" * 40, "action": "flag.set", "target": "p2p_hosting", "on": "1", "confirm": "p2p_hosting"})
check("a wrong token: 403", st == 403)
st, _, _ = req("POST", "/wrath/admin/act", "owner1", {"csrf": token("modadm"), "action": "flag.set", "target": "p2p_hosting", "on": "1", "confirm": "p2p_hosting"})
check("another account's token: 403", st == 403)
st, _, _ = req("POST", "/wrath/admin/act", "owner1", {"csrf": token("owner1"), "action": "news.add", "target": "new"}, origin="https://evil.example")
check("another site's form: 403", st == 403)
st, _, _ = req("POST", "/wrath/admin/act", "owner1", {"csrf": token("owner1")}, ctype="application/json")
check("not a form: 400", st == 400)
st, _, _ = req("POST", "/wrath/admin/act", "player", {"csrf": token("owner1"), "action": "flag.set", "target": "p2p_hosting"})
check("a player's POST: 403", st == 403)
check("flags untouched by all of that", feed("flags.json")[2]["flags"]["p2p_hosting"] is False)
st, _, _ = req("POST", "/wrath/admin/nope", "owner1", {"x": "1"})
check("POST elsewhere under /wrath/admin: 405", st == 405)

# ---------------------------------------------------------------- flags (owner, typed confirm)
import hashlib


def flag_sha_ok(d):
    body = {k: v for k, v in d.items() if k != "sha256"}
    return d.get("sha256") == hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


st, hd, f = feed("flags.json")
check("flags.json: the host-pack/P2P agent's format, all off, sha256", f.get("format") == 1 and f["flags"] == {
      "p2p_hosting": False, "torrent_downloads": False, "torrent_seeding": False, "webrtc_presence": False,
      "mesh_realms": False, "mesh_relays": False, "relay_volunteer": False, "herokey_sso": False, "magnet_links": False,
      "share_own_files": False}
      and flag_sha_ok(f) and set(f) == {"format", "updated_at", "flags", "limits", "stun", "sha256"}
      and hd.get("Access-Control-Allow-Origin") == "*", f)
r = act("staff1", "flag.set", "p2p_hosting", on="1", confirm="p2p_hosting")
check("staff cannot flip a flag (owner only)", "err=Only the owner" in r[1] and audit()[-1]["note"] == "owner only")
r = act("owner1", "flag.set", "p2p_hosting", on="1")
check("the owner without the typed confirm: refused", "err=Type p2p_hosting to confirm" in r[1] and not feed("flags.json")[2]["flags"]["p2p_hosting"])
r = act("owner1", "flag.set", "p2p_hosting", on="1", confirm="p2p_hostin")
check("a mistyped confirm: refused", "err=" in r[1])
r = act("owner1", "flag.set", "p2p_hosting", "/wrath/admin/flags", on="1", confirm="p2p_hosting")
f = feed("flags.json")[2]
check("typed confirm: on, a new updated_at and sha256", ok_(r) and f["flags"]["p2p_hosting"] is True and flag_sha_ok(f)
      and f["updated_at"] != "2026-10-29T00:00:00Z", r)
check("the served file never names the admin", "owner1" not in json.dumps(f))
check("redirect back to the section", r[1].startswith("/wrath/admin/flags?ok="))
a = audit()[-1]
check("audit: who, what, when, before, after", a["who"] == "owner1" and a["name"] == "Owner1" and a["role"] == "owner"
      and a["action"] == "flag.set" and a["target"] == "p2p_hosting" and a["before"] == {"p2p_hosting": False}
      and a["after"]["p2p_hosting"] is True and re.match(r"^\d{4}-\d\d-\d\dT", a["at"]) and a["ok"] is True, a)
check("unknown flags refused", "err=Unknown flag" in act("owner1", "flag.set", "voice_chat", on="1", confirm="voice_chat")[1])
check("flags page shows the flags with confirm boxes", 'data-confirm="torrent_seeding"' in req("GET", "/wrath/admin/flags", "owner1")[2])
check("staff see flags without switches", 'data-confirm="' not in req("GET", "/wrath/admin/flags", "modadm")[2])
act("owner1", "flag.set", "p2p_hosting", on="0", confirm="p2p_hosting")
check("and off again", feed("flags.json")[2]["flags"]["p2p_hosting"] is False and flag_sha_ok(feed("flags.json")[2]))
wp.leyline_net = types.SimpleNamespace(serve=lambda path: None, KNOWN=("p2p_hosting", "x_flag"), load=lambda: {"format": 1, "updated_at": "u", "flags": {"x_flag": True}},
                                       path=lambda: "/nowhere", write_flags=lambda p, ch, by="": (_ for _ in ()).throw(ValueError("x_flag is locked")))
body = req("GET", "/wrath/admin/flags", "owner1")[2]
r = act("owner1", "flag.set", "x_flag", on="0", confirm="x_flag")
check("with stream/leyline_net.py in the portal: its flags, its writer, its feed", "<code>x_flag</code>" in body and "err=x_flag is locked" in r[1]
      and not A.match("/wrath/leyline/flags.json"))
wp.leyline_net = None

# ---------------------------------------------------------------- catalog
mods_ids = lambda: [p["id"] for p in json.loads(req("GET", "/wrath/mods/all.json")[2])["projects"]]
check("catalog: pings listed before", "mod:pings" in mods_ids())
check("unpublish needs the typed confirm", "err=Type pings" in act("staff1", "mod.unpublish", "pings")[1])
r = act("staff1", "mod.unpublish", "pings", confirm="pings")
check("unpublish: gone from all.json and its page", ok_(r) and "mod:pings" not in mods_ids() and req("GET", "/wrath/mods/pings")[0] == 404)
check("... the zip and the catalog file stay", os.path.isfile(os.path.join(MODS, "files", "pings-" + next(m["version"] for m in cat["mods"] if m["id"] == "pings") + ".zip")))
check("the admin catalog still shows it, unpublished", "unpublished" in req("GET", "/wrath/admin/catalog", "staff1")[2])
r = act("staff1", "mod.approve", "pings")
check("approve: back", ok_(r) and "mod:pings" in mods_ids() and audit()[-1]["before"] == {"state": "unpublished"})
r = act("staff1", "mod.reject", "emote-wheel", confirm="emote-wheel")
check("reject: gone", ok_(r) and "mod:emote-wheel" not in mods_ids())
act("staff1", "mod.approve", "emote-wheel")
r = act("staff1", "mod.feature", "pings", on="1")
proj = {p["id"]: p for p in json.loads(req("GET", "/wrath/mods/all.json")[2])["projects"]}
check("feature: all.json says featured", ok_(r) and proj["mod:pings"]["featured"] is True and proj["mod:dalaran-dice"]["featured"] is False)
r = act("staff1", "mod.edit", "pings", name="Map Pings", description="Ping the map for your party.", category=CATS[1])
proj = {p["id"]: p for p in json.loads(req("GET", "/wrath/mods/all.json")[2])["projects"]}
check("edit: name, description, category", ok_(r) and proj["mod:pings"]["name"] == "Map Pings"
      and proj["mod:pings"]["description"] == "Ping the map for your party." and proj["mod:pings"]["category"] == CATS[1])
check("edit: shown on the mod's page too", "Map Pings" in req("GET", "/wrath/mods/pings")[2])
check("edit: a game-brand word is refused", "err=Players see this" in act("staff1", "mod.edit", "pings", name="WoW Pings")[1])
check("edit: unknown category refused", "err=Unknown category" in act("staff1", "mod.edit", "pings", category="nope")[1])
check("edit: unknown mod refused", "err=No such mod" in act("staff1", "mod.edit", "nosuch", name="x")[1])
r = act("staff1", "mod.check", "badmod")
chk = json.load(open(os.path.join(DATA, "admin", "checks", "mod-badmod.json")))
check("re-run the checker: the MPQ in a zip is found", ok_(r) and chk["errors"] >= 1 and any(i["code"] == "blocked" or "MPQ" in i["msg"] for i in chk["items"]), chk)
body = req("GET", "/wrath/admin/catalog/mod/badmod", "staff1")[2]
check("the checker's findings on the mod's page", "MPQ" in body and "errors" in body)
r = act("staff1", "mod.check", "pings")
chk = json.load(open(os.path.join(DATA, "admin", "checks", "mod-pings.json")))
check("a good mod checks clean", ok_(r) and chk["errors"] == 0, chk)
check("the catalog page tags the checker result", "checker: " in req("GET", "/wrath/admin/catalog", "staff1")[2])

# community links
r = act("staff1", "link.dismiss", PID)
recs = [json.loads(x) for x in open(os.path.join(DATA, "projects.jsonl"))]
check("dismiss reports: the link stays, its reports go", ok_(r) and recs[0]["reports"] == [] and not recs[0]["hidden"])
check("hide needs the confirm", "err=Type " + PID in act("staff1", "link.hide", PID)[1])
r = act("staff1", "link.hide", PID, confirm=PID)
recs = [json.loads(x) for x in open(os.path.join(DATA, "projects.jsonl"))]
recs = {x["id"]: x for x in recs}
check("hide: hidden in the file (nothing deleted), off all.json", ok_(r) and len(recs) == 2 and recs[PID]["hidden"] is True
      and "link:" + PID not in mods_ids())
act("staff1", "link.unhide", PID)
r = act("staff1", "link.feature", PID, on="1")
r2 = act("staff1", "link.edit", PID, name="Frame Kit", description="Unit frames for raids.", category=CATS[2])
proj = {p["id"]: p for p in json.loads(req("GET", "/wrath/mods/all.json")[2])["projects"]}
check("links: feature and edit", ok_(r) and ok_(r2) and proj["link:" + PID]["featured"] is True and proj["link:" + PID]["name"] == "Frame Kit"
      and proj["link:" + PID]["category"] == CATS[2])

# ---------------------------------------------------------------- the player Workshop (SOAP)
body = req("GET", "/wrath/admin/workshop", "staff1")[2]
check("workshop: pending items over SOAP, flagged titles tagged", "#2 Cozy Inn" in body and "#3 Shit Arena" in body and "filtered word" in body
      and "#1 Windmill" not in body)
soap_log.clear()
r = act("staff1", "ws.approve", "2", "/wrath/admin/workshop?state=pending")
check("approve: ovadmin ws set 2 approve", ok_(r) and soap_log[-1] == "ovadmin ws set 2 approve" and ws_items[2]["state"] == "approved"
      and r[1].startswith("/wrath/admin/workshop?state=pending&ok="))
check("the list is fresh after an action", "#2 Cozy Inn" in req("GET", "/wrath/admin/workshop?state=approved", "staff1")[2])
check("reject needs the confirm", "err=Type 3" in act("staff1", "ws.reject", "3")[1] and ws_items[3]["state"] == "pending")
r = act("staff1", "ws.reject", "3", confirm="3")
check("reject", ok_(r) and ws_items[3]["state"] == "rejected")
r = act("staff1", "ws.unpublish", "1", confirm="1")
check("unpublish = back to waiting (ovadmin ws set 1 pending), nothing deleted", ok_(r) and soap_log[-1] == "ovadmin ws set 1 pending"
      and ws_items[1]["state"] == "pending")
act("staff1", "ws.approve", "1")
r = act("staff1", "ws.edit", "2", field="title", text="The 'Cozy' Inn; DROP TABLE <x>")
check("edit: text cleaned before SOAP (ov_workshop's characters only)", ok_(r) and soap_log[-1] == "ovadmin ws edit 2 title The Cozy Inn DROP TABLE x", soap_log[-1:])
check("edit: audit before/after", audit()[-1]["before"] == {"title": "Cozy Inn"} and audit()[-1]["after"] == {"title": "The Cozy Inn DROP TABLE x"})
check("edit: other fields refused", "err=Unknown field" in act("staff1", "ws.edit", "2", field="author", text="Bob")[1])
check("edit: brand words refused", "err=Players see this" in act("staff1", "ws.edit", "2", field="descr", text="best wow build")[1])
check("bad ids never reach SOAP", "err=Not a Workshop item id" in act("staff1", "ws.approve", "2;server shutdown")[1]
      and not any("shutdown" in c for c in soap_log))
r = act("staff1", "ws.check", "3")
check("re-run the checks: findings saved and shown", ok_(r) and json.load(open(os.path.join(DATA, "admin", "checks", "ws-3.json")))["items"] == ["title: filtered word"]
      and "title: filtered word" in req("GET", "/wrath/admin/workshop?state=rejected", "staff1")[2])
r = act("staff1", "ws.feature", "2", on="1")
lst = req("GET", "/wrath/workshop?sort=plays", None)[2]
check("feature: the public Workshop lists it first", ok_(r) and lst.find("Cozy Inn") < lst.find("Windmill Nine") and lst.find("Cozy Inn") > 0)
good_soap, wp.AC_SOAP_URL = wp.AC_SOAP_URL, "http://127.0.0.1:9/"
A.forget("ws:")
body = req("GET", "/wrath/admin/workshop", "staff1")[2]
r = act("staff1", "ws.approve", "3")
wp.AC_SOAP_URL = good_soap
A.forget("ws:")
check("a realm that does not answer: the page and the action say so", "the realm did not answer (down)" in body
      and "err=the realm did not answer" in r[1] and audit()[-1]["ok"] is False)

# ---------------------------------------------------------------- the community index
def decisions():
    try:
        return [json.loads(x) for x in open(os.path.join(DATA, "hub", "decisions.jsonl"))]
    except OSError:
        return []


body = req("GET", "/wrath/admin/hub", "staff1")[2]
check("hub: submissions and entries", "https://github.com/c/cool-addon" in body and "Individual Progression" in body and "A cool addon" in body)
check("reject without a reason: refused", "err=Say why" in act("staff1", "hub.reject", H2)[1] and decisions() == [])
r1 = act("staff1", "hub.approve", H1, reason="nice")
r2 = act("staff1", "hub.reject", H2, reason="no licence file")
d = decisions()
check("decisions appended to hub/decisions.jsonl in stream/hub.py's shape", ok_(r1) and ok_(r2) and len(d) == 2
      and d[0]["id"] == H1 and d[0]["decision"] == "approve" and d[0]["repo"] == "https://github.com/c/cool-addon" and d[0]["build"] == "3.3.5a"
      and d[0]["category"] == "addons" and d[1]["decision"] == "reject" and d[1]["reason"] == "no licence file" and d[1]["by"] == "staff1", d)
check("decided submissions leave the queue", "hub.approve" not in req("GET", "/wrath/admin/hub", "staff1")[2])
check("unknown submission refused", "err=No such submission" in act("staff1", "hub.approve", "ffffffffffff")[1])
check("delist needs the confirm and a reason", "err=" in act("staff1", "hub.delist", HE, reason="x")[1]
      and "err=Say why" in act("staff1", "hub.delist", HE, confirm=HE)[1])
r = act("staff1", "hub.delist", HE, reason="ships game data", confirm=HE)
r2 = act("staff1", "hub.rescan", "all")
r3 = act("staff1", "hub.rescan", HE)
d = decisions()
resc = json.load(open(os.path.join(DATA, "hub", "rescan.json")))
check("delist and re-scan", ok_(r) and ok_(r2) and ok_(r3) and d[-1]["decision"] == "delist" and d[-1]["id"] == HE
      and set(resc["requests"]) == {"all", HE})
check("the index file itself is untouched", json.load(open(HUB_INDEX))["entries"][0] == json.load(open(HUB_INDEX))["entries"][0]
      and "delist" not in open(HUB_INDEX).read())
r = act("staff1", "hub.relist", HE)
check("list again: an approve line (the latest line wins)", ok_(r) and decisions()[-1]["decision"] == "approve" and decisions()[-1]["id"] == HE)
check("bad ids refused", "err=No such index entry" in act("staff1", "hub.rescan", "../x")[1])

# ---------------------------------------------------------------- realms
body = req("GET", "/wrath/admin/realms", "staff1")[2]
check("realms: from the relay, with heartbeats", "Frost Haven" in body and RID_A in body and "seq: 42" in body and "last 2026-10-29T12:00:00Z" in body)
pub = req("GET", "/wrath/realms")[2]
check("public list before", "Frost Haven" in pub and "Quiet Corner" in pub)
check("hide needs the realm id typed", "err=" in act("staff1", "realm.hide", RID_B)[1])
r = act("staff1", "realm.hide", RID_B, confirm=RID_B)
pub = req("GET", "/wrath/realms")[2]
check("hide: off the public realm list at once", ok_(r) and "Quiet Corner" not in pub and "Frost Haven" in pub)
r = act("staff1", "realm.official", RID_A, confirm=RID_A)
pub = req("GET", "/wrath/realms")[2]
check("mark official: tagged Official on the public list", ok_(r) and re.search(r"Frost Haven</h2><div class=\"tags\"><span class=\"tag\">Official", pub), pub[:200])
st, hd, ov = feed("realm-overrides.json")
check("realm-overrides.json for the launcher: hidden and official ids only", ov.get("hidden") == [RID_B] and ov.get("official") == [RID_A]
      and set(ov) == {"format", "updated_at", "hidden", "official", "featured", "verified"} and ov["featured"] == []
      and hd.get("Access-Control-Allow-Origin") == "*")
check("flag needs a reason", "err=Say why" in act("staff1", "realm.flag", RID_A)[1])
r = act("staff1", "realm.flag", RID_A, reason="spam in the tagline")
check("flag: staff note, not on the public list or the feed", ok_(r) and "spam in the tagline" in req("GET", "/wrath/admin/realms", "staff1")[2]
      and "spam" not in req("GET", "/wrath/realms")[2] and "spam" not in json.dumps(feed("realm-overrides.json")[2]))
act("staff1", "realm.unhide", RID_B)
check("unhide: back", "Quiet Corner" in req("GET", "/wrath/realms")[2])
check("bad realm ids refused", "err=Not a realm id" in act("staff1", "realm.hide", "../../x", confirm="../../x")[1])

# ---------------------------------------------------------------- releases, channel, news
body = req("GET", "/wrath/admin/releases", "owner1")[2]
check("releases from GitHub: tags, pre-release, draft", "launcher-v0.5.0" in body and "launcher-v0.4.2" in body and "pre-release" in body and "draft" in body)
check("staff can't set the channel", "err=Only the owner" in act("staff1", "channel.set", "launcher-v0.4.2", channel="stable", confirm="0.4.2")[1])
check("stable needs the version typed", "err=Type 0.4.2" in act("owner1", "channel.set", "launcher-v0.4.2", channel="stable")[1])
r = act("owner1", "channel.set", "launcher-v0.4.2", channel="stable", confirm="0.4.2")
r2 = act("owner1", "channel.set", "launcher-v0.5.0", channel="beta")
ch = feed("channel.json")[2]
check("channel.json: stable and beta", ok_(r) and ok_(r2) and ch["stable"] == {"version": "0.4.2", "tag": "launcher-v0.4.2",
      "url": "https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.4.2"} and ch["beta"]["version"] == "0.5.0" and ch["hold"] is False, ch)
check("a draft or unknown tag is refused", "err=" in act("owner1", "channel.set", "launcher-v0.6.0", channel="beta")[1]
      and "err=" in act("owner1", "channel.set", "launcher-v9.9.9", channel="beta")[1])
check("hold needs HOLD typed", "err=Type HOLD" in act("owner1", "channel.hold", "hold", on="1")[1])
r = act("owner1", "channel.hold", "hold", on="1", note="bad build", confirm="HOLD")
check("hold: on", ok_(r) and feed("channel.json")[2]["hold"] is True)
r = act("owner1", "channel.hold", "hold", on="0", confirm="RELEASE")
check("release the hold", ok_(r) and feed("channel.json")[2]["hold"] is False)
r = act("staff1", "news.add", "new", tag="Launcher 0.5", title="Themes are here", items="Pick a theme\n\nMore to come\n")
news = feed("news.json")[2]
check("news: a post, in the launcher's shape", ok_(r) and news["posts"][0]["tag"] == "LAUNCHER 0.5" and news["posts"][0]["title"] == "THEMES ARE HERE"
      and news["posts"][0]["items"] == ["Pick a theme", "More to come"] and set(news["posts"][0]) == {"id", "date", "tag", "title", "items"})
check("news: brand words refused", "err=Players see this" in act("staff1", "news.add", "new", tag="x", title="Wrath of the Lich King", items="a")[1])
check("news: empty refused", "err=" in act("staff1", "news.add", "new", tag="x", title="", items="")[1])
pid_news = news["posts"][0]["id"]
r = act("staff1", "news.hide", pid_news, confirm=pid_news)
check("news: hide", ok_(r) and feed("news.json")[2]["posts"] == [])

# ---------------------------------------------------------------- themes (the Workshop's "themes" category)
body = req("GET", "/wrath/admin/themes", "staff1")[2]
check("themes: the Workshop's theme packs", "Frost Glass" in body and f"link:{TPID}" in body)
check("listed at once by default (like every Workshop entry)", "link:" + TPID in mods_ids())
os.environ["WU_THEMES_REVIEW"] = "1"
check("WU_THEMES_REVIEW=1: held until approved", "link:" + TPID not in mods_ids() and "link:" + PID in mods_ids(), mods_ids())
check("... and counted as waiting", "<b>1</b><span>Theme packs waiting" in req("GET", "/wrath/admin/numbers", "staff1")[2])
r = act("staff1", "link.approve", TPID, "/wrath/admin/themes")
r2 = act("staff1", "link.feature", TPID, "/wrath/admin/themes", on="1")
proj = {p["id"]: p for p in json.loads(req("GET", "/wrath/mods/all.json")[2])["projects"]}
check("approve and feature: in all.json, featured", ok_(r) and ok_(r2) and proj["link:" + TPID]["featured"] is True)
check("reject needs the confirm", "err=" in act("staff1", "link.reject", TPID)[1])
r = act("staff1", "link.reject", TPID, confirm=TPID)
check("reject: out again", ok_(r) and "link:" + TPID not in mods_ids())
del os.environ["WU_THEMES_REVIEW"]
check("a rejected pack stays out with review off too", "link:" + TPID not in mods_ids())

# ---------------------------------------------------------------- reports
body = req("GET", "/wrath/admin/reports", "staff1")[2]
check(".bug reports, escaped", "quest giver &lt;script&gt;alert(1)&lt;/script&gt;" in body and "<script>alert" not in body)
check("message reports: only the reported message", "REPORTED-BODY" in body and "you are &lt;b&gt;bad&lt;/b&gt;" in body
      and "PRIVATE-REPORTER-MESSAGE" not in body and "PRIVATE-LATER-REPLY" not in body and "OLDER-MALLORY-MESSAGE" not in body)
check("message reports: who, reason, the reporter's note", "Mallory" in body and "reported by Bob" in body and "keeps going" in body)
r = act("staff1", "bug.set", "7", "/wrath/admin/reports", status="resolved")
check("resolve a bug: ovadmin bugs set 7 resolved", ok_(r) and soap_log[-1] == "ovadmin bugs set 7 resolved" and bugs[7]["status"] == "resolved")
check("bad bug status refused", "err=" in act("staff1", "bug.set", "7", status="gone")[1])
r = act("staff1", "report.note", "bug:7", text="fixed in the next client")
check("a note on a report", ok_(r) and "fixed in the next client" in req("GET", "/wrath/admin/reports?bugs=all", "staff1")[2])
r = act("staff1", "msg.resolve", "5", "/wrath/admin/reports", outcome="warned")
check("resolve a message report (Social.resolve_report)", ok_(r) and FAKE_SOCIAL.rows[0]["status"] == "done"
      and FAKE_SOCIAL.rows[0]["outcome"] == "warned" and FAKE_SOCIAL.rows[0]["handled_by"] == "Staff1 (staff1)")
check("handled reports leave the open list", "REPORTED-BODY" not in req("GET", "/wrath/admin/reports", "staff1")[2]
      and "REPORTED-BODY" in req("GET", "/wrath/admin/reports?msgs=done", "staff1")[2])
r = act("staff1", "msg.reopen", "5")
check("reopen", ok_(r) and FAKE_SOCIAL.rows[0]["status"] == "open")
check("unknown report refused", "err=No such report" in act("staff1", "msg.dismiss", "99")[1])
check("bad report keys refused", "err=Not a report" in act("staff1", "report.note", "x:../1", text="y")[1])

# ---------------------------------------------------------------- health
body = req("GET", "/wrath/admin/health", "staff1")[2]
check("health: SOAP, ov_admin.lua, relay, exports, folders, checker, GitHub", all(x in body for x in (
    "Realm SOAP", "ov_admin.lua on the realm", "Beacons relay listing", "Workshop export", "Admin folder writable",
    "Checker (wu-mod.py)", "GitHub releases API")))
check("health: Autopilot skipped, and why", "Skipped: Autopilot runs on the owner" in body)
ap = os.path.join(T, "last-run.json")
json.dump({"at": "2026-10-29 14:00:00", "ids": ["1063a"], "ok": False, "failed_step": "client"}, open(ap, "w"))
os.environ["WU_AUTOPILOT_STATUS"] = ap
body = req("GET", "/wrath/admin/health", "staff1")[2]
check("health: Autopilot's last run when a copy is reachable", "failed step: client" in body and "1063a" in body)

# ---------------------------------------------------------------- staff
check("staff can't grant", "err=Only the owner" in act("staff1", "staff.grant", "new", account="newbie")[1])
check("newbie is refused before", req("GET", "/wrath/admin", "newbie")[0] == 403)
r = act("owner1", "staff.grant", "new", account="newbie", name="New Helper")
check("the owner grants staff: newbie gets in", ok_(r) and req("GET", "/wrath/admin", "newbie")[0] == 200)
st = os.stat(os.path.join(DATA, "admin", "staff.json")).st_mode & 0o777
check("staff.json is not world-readable", st == 0o640, oct(st))
check("revoke needs the account id typed", "err=Type newbie" in act("owner1", "staff.revoke", "newbie")[1])
r = act("owner1", "staff.revoke", "newbie", confirm="newbie")
check("revoke: refused again", ok_(r) and req("GET", "/wrath/admin", "newbie")[0] == 403)
check("bad account ids refused", "err=" in act("owner1", "staff.grant", "new", account="a b<c>")[1])

# ---------------------------------------------------------------- audit
body = req("GET", "/wrath/admin/audit?action=realm.", "staff1")[2]
check("audit page, filtered", "realm.hide" in body and "flag.set" not in body)
log_ = audit()
check("every action is in the audit log", {"flag.set", "mod.unpublish", "mod.edit", "mod.check", "link.hide", "link.dismiss",
      "ws.approve", "ws.edit", "ws.check", "hub.reject", "hub.rescan", "realm.hide", "realm.official", "realm.flag",
      "channel.set", "channel.hold", "news.add", "news.hide", "link.approve", "link.reject", "bug.set", "report.note",
      "msg.resolve", "hub.delist", "hub.relist", "staff.grant", "staff.revoke"} - {a["action"] for a in log_ if a["ok"]} == set(),
      {"flag.set"} - {a["action"] for a in log_ if a["ok"]})
check("refusals are logged too", any(not a["ok"] and "confirm" in a["note"] for a in log_))
check("nothing was ever deleted on the realm", not any(re.search(r"(?i)\bdelete\b|unpublish|account (delete|set)|\bban\b", c) for c in soap_log))

# ---------------------------------------------------------------- public feeds: no names a player shouldn't see
allfeeds = "".join(req("GET", f"/wrath/leyline/{n}")[2] for n in ("flags.json", "channel.json", "news.json", "realm-overrides.json"))
check("feeds: no game-brand names, no staff ids", not re.search(r"(?i)warcraft|\bwow\b|blizzard|lich king", allfeeds) and "staff1" not in allfeeds
      and "owner1" not in allfeeds)
check("the /wrath/leyline page is unchanged", req("GET", "/wrath/leyline")[0] == 200)

# ---------------------------------------------------------------- Leyline HQ (stream/hq.py)
H = A.hq
check("hq.py is loaded beside admin.py", H is not None)
HQD = os.path.join(DATA, "hq")
os.makedirs(HQD, exist_ok=True)
st, hd, body = req("GET", "/wrath/admin", "owner1")
check("HQ home: the status strip (website, the realm, relay, browser play, server check)", st == 200 and all(x in body for x in (
    "Website", H.realm_name(), "Relay", "Browser play", "Server check", "Not sent yet")) and "4 playing" in body,
      [x for x in ("Website", "Wrath Unleashed", "Relay", "Browser play", "Server check", "Not sent yet", "4 playing") if x not in body])
check("HQ home: Needs you lists open items with one-click actions", "Needs you" in body and "Bug report" in body
      and 'value="bug.set"' in body)
check("HQ home: Releases card (launcher, channel, builds, addon pack, host pack, website)", all(x in body for x in (
    "Launcher", "Channel", "Addon pack", "Host pack")) and "0.5.0" in body
      and "Build: Leyline launcher" in body and "failed" in body and "2026.1031.9" in body)
check("HQ home: From Claude and Note to Claude (owner)", "From Claude" in body and "Note to Claude" in body
      and "Claude reads these next time it checks in; for a live chat use the Claude app." in body)
check("HQ home: the eight-word menu", all(f">{w}</a>" in body for w in ("Home", "Needs you", "Players", "Realms", "Releases", "Content", "Game", "System")))
check("HQ: no game-brand words on Home", not re.search(r"(?i)warcraft|\bwow\b|blizzard|lich king", re.sub(r"<style>.*?</style>", "", body, flags=re.S)))
body = req("GET", "/wrath/admin", "modadm")[2]
check("a mod sees no Note to Claude (owner only)", "Note to Claude" not in body and "hq.note.add" not in body)
check("a mod sees approve buttons nowhere (admins approve)", 'value="ws.approve"' not in body and 'value="bug.set"' in body)
r = act("modadm", "ws.approve", "2")
check("a mod can't approve (server side too)", "err=Only admins" in r[1] and audit()[-1]["note"] == "admins only")
r = act("modadm", "bug.set", "6", status="seen")
check("a mod can handle a report", ok_(r))

body = req("GET", "/wrath/admin/inbox", "staff1")[2]
check("Needs you: reports and queues together", all(x in body for x in ("Bug report", "Message report")), body[-2000:])
check("Needs you: a message report shows only the reported message", "REPORTED-BODY" in body and "PRIVATE-REPORTER-MESSAGE" not in body)

# the PC's feeds
json.dump({"format": 1, "made_at": "2026-10-31T01:00:00Z", "steps": [
    {"id": "1090a", "step": "signin", "title": "Play page and sign-in", "note": "HQ <b>home</b>", "ok": True, "at": "2026-10-31T00:50:00Z"},
    {"id": "1090b", "step": "relay", "title": "Beacons relay", "note": "", "ok": False, "at": "2026-10-31T00:55:00Z"},
    {"id": "../x", "step": "evil"}], "last_check": {"at": "2026-10-31T00:30:00Z", "ok": True, "id": "check-auto-1"}},
    open(os.path.join(HQD, "changes.json"), "w"))
json.dump({"format": 1, "notes": [{"id": "n1", "date": "2026-10-31", "title": "HQ is live", "items": ["Home shows everything", "<script>x</script>"]}],
           "ack": []}, open(os.path.join(HQD, "claude-notes.json"), "w"))
body = req("GET", "/wrath/admin", "owner1")[2]
check("Recent changes from the PC: done, failed, escaped, bad ids dropped", "Play page and sign-in" in body and "HQ &lt;b&gt;home&lt;/b&gt;" in body
      and ">failed<" in body and "evil" not in body)
check("Server check from the PC", "Passed" in body)
check("From Claude: the latest note, escaped", "HQ is live" in body and "Home shows everything" in body and "<script>x</script>" not in body)

# note to Claude
r = act("modadm", "hq.note.add", "new", text="hi")
check("only the owner leaves notes to Claude", "err=Only the owner" in r[1])
r = act("owner1", "hq.note.add", "new", text="Please look at the relay\nand the bots")
notes = [json.loads(x) for x in open(os.path.join(HQD, "owner-notes.jsonl"))]
check("a note to Claude: appended with time and author", ok_(r) and notes[-1]["text"] == "Please look at the relay\nand the bots"
      and notes[-1]["by"] == "owner1" and re.match(r"^o\d{14}[0-9a-f]{4}$", notes[-1]["id"]))
nid = notes[-1]["id"]
check("... shown as waiting", ">waiting<" in req("GET", "/wrath/admin", "owner1")[2])
open(os.path.join(HQD, "picked-up-at"), "w").write(time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() + 5)) + "\n")
check("... on Claude's PC once the PC picked it up", "on Claude&#x27;s PC" in req("GET", "/wrath/admin", "owner1")[2])
json.dump({"format": 1, "notes": [], "ack": [nid]}, open(os.path.join(HQD, "claude-notes.json"), "w"))
check("... seen once Claude acknowledged it", ">seen<" in req("GET", "/wrath/admin/notes", "owner1")[2])
r = act("owner1", "hq.note.close", nid, "/wrath/admin/notes")
check("close a note (append-only: a status line)", ok_(r) and ">closed<" in req("GET", "/wrath/admin/notes", "owner1")[2]
      and len(open(os.path.join(HQD, "owner-notes.jsonl")).read().splitlines()) == len(notes) + 1)
check("empty notes refused", "err=Write the note" in act("owner1", "hq.note.add", "new", text="  ")[1])

# players
body = req("GET", "/wrath/admin/players?q=mallory", "modadm")[2]
check("Players: find by name", "Mallory" in body and "/wrath/admin/person?acct=WRMAL" in body)
check("Players: find by character", "WRMAL" in req("GET", "/wrath/admin/players?q=Malchar", "modadm")[2])
check("Players: find by Name#tag", "WRMAL" in req("GET", "/wrath/admin/players?q=Mallory%234242", "modadm")[2]
      and "WRMAL" not in req("GET", "/wrath/admin/players?q=Mallory%231111", "modadm")[2])
st, _, body = req("GET", "/wrath/admin/person?acct=WRMAL", "staff1")
check("a person's page: profile, online, characters, devices, game tools", st == 200 and "Mallory#4242" in body
      and "In the game now as Malchar" in body and "Leyline launcher, linked" in body and 'value="game.kick"' in body
      and 'value="game.ban"' in body and 'value="sanction.set"' in body)
check("a person's page for a mod: kick and mute, no ban or sanction", 'value="game.kick"' in req("GET", "/wrath/admin/person?acct=WRMAL", "modadm")[2]
      and 'value="game.ban"' not in req("GET", "/wrath/admin/person?acct=WRMAL", "modadm")[2])
check("a bad account on the person page: 404", req("GET", "/wrath/admin/person?acct=../x", "staff1")[0] == 404)

# the game, over SOAP: built from checked parts
soap_log.clear()
r = act("modadm", "game.kick", "malchar")
check("kick: 'kick Malchar' (the name cased like the game)", ok_(r) and soap_log[-1] == "kick Malchar", soap_log[-1:])
r = act("modadm", "game.mute", "Malchar", minutes="30", reason="spam in /say; DROP")
check("mute: minutes and a cleaned reason", ok_(r) and soap_log[-1] == "mute Malchar 30 spam in /say DROP", soap_log[-1:])
check("mute: minutes out of range refused", "err=Minutes" in act("modadm", "game.mute", "Malchar", minutes="99999", reason="x y z")[1])
r = act("modadm", "game.rescue", "Malchar")
check("rescue: tele name Malchar Dalaran", ok_(r) and soap_log[-1] == "tele name Malchar Dalaran")
n = len(soap_log)
for bad in ("Mal char", "Mal;server shutdown", "Malchar\nserver shutdown", "x", "Mal1", "a" * 13, "Mal\u0000"):
    act("modadm", "game.kick", bad)
check("bad character names never reach SOAP (no injection)", len(soap_log) == n and not any("shutdown" in c for c in soap_log), soap_log[n:])
check("a player the realm doesn't know: refused with its words", "err=The realm said: Player Nobody not found" in act("modadm", "game.kick", "Nobody")[1])
check("a mod can't ban", "err=Only admins" in act("modadm", "game.ban", "WRMAL", hours="24", reason="cheating", confirm="WRMAL")[1])
check("ban needs the account typed", "err=Type WRMAL" in act("staff1", "game.ban", "WRMAL", hours="24", reason="cheating")[1])
check("ban needs a reason", "err=Say why" in act("staff1", "game.ban", "WRMAL", hours="24", reason="", confirm="WRMAL")[1])
r = act("staff1", "game.ban", "WRMAL", hours="24", reason="cheating, 3 reports", confirm="WRMAL")
check("ban: 'ban account WRMAL 24h <reason>'", ok_(r) and soap_log[-1] == "ban account WRMAL 24h cheating, 3 reports", soap_log[-1:])
r = act("staff1", "game.ban", "WRMAL", hours="0", reason="for good", confirm="WRMAL")
check("ban 0 hours: permanent (-1)", ok_(r) and soap_log[-1] == "ban account WRMAL -1 for good")
check("bot accounts refused", "err=That is a bot" in act("staff1", "game.ban", "RNDBOT7", hours="1", reason="xyz", confirm="RNDBOT7")[1])
check("account names checked", "err=Not a game account" in act("staff1", "game.unban", "WR MAL", confirm="WR MAL")[1])
r = act("staff1", "game.unban", "WRMAL", confirm="WRMAL")
check("unban", ok_(r) and soap_log[-1] == "unban account WRMAL")
r = act("staff1", "game.announce", "all", text="Restart at 20:00 |cffff0000red|r")
check("announce: one line, no chat codes", ok_(r) and soap_log[-1] == "announce Restart at 20:00 /cffff0000red/r", soap_log[-1:])
check("announce: game-brand words refused", "err=Players see this" in act("staff1", "game.announce", "all", text="WoW night")[1])
check("a mod can't announce", "err=Only admins" in act("modadm", "game.announce", "all", text="hello there")[1])
for kind, form, tgt in (("kick", {}, "Ann"), ("mute", {"minutes": "5", "reason": "abc"}, "Ann"), ("rescue", {}, "Ann"),
                        ("ban", {"hours": "1", "reason": "abc"}, "WRX"), ("unban", {}, "WRX"), ("announce", {"text": "hi all"}, ""),
                        ("unmute", {}, "Ann"), ("notify", {"text": "hi all"}, "")):
    c = H.build(kind, form, tgt)
    check(f"build({kind}): one allow-listed line", "\n" not in c and c.lower().startswith(H.GAME_ALLOW), c)
body = req("GET", "/wrath/admin/game", "modadm")[2]
check("Game page: tools, GM tickets (escaped), online list without bots", "Say to everyone" not in body and "A character" in body
      and "Ticket: 3. Created by: Ann" in body and "&lt;b&gt;help&lt;/b&gt;" in body and "WRMAL" in body and "RNDBOT12" not in body)
check("Game page: admins see the announce and ban tools", all(x in req("GET", "/wrath/admin/game", "staff1")[2] for x in ("Say to everyone", 'value="game.ban"')))
_gb = req("GET", "/wrath/admin/game", "staff1")[2]
_pb = req("GET", "/wrath/admin/person?acct=WRMAL", "staff1")[2]
check("HQ says Ban account / Unban account (the player realms' words), with how long and a reason, on both pages",
      all(">Ban account</button>" in b and ">Unban account</button>" in b and 'aria-label="Hours (0 = permanent)"' in b
          and 'placeholder="Reason (the player sees it)"' in b for b in (_gb, _pb)) and "Ban the game account" not in _pb)
# one meaning of a ban everywhere (owner, 31 Oct): the player realms ban the character's ACCOUNT, with HQ's limits and words
_lua = open(os.path.join(ROOT, "realm", "custom", "lua_scripts", "ov_hostctl.lua"), encoding="utf-8").read()
_rs = open(os.path.join(ROOT, "launcher", "src-tauri", "src", "host", "console.rs"), encoding="utf-8").read()
_js = open(os.path.join(ROOT, "launcher", "ui", "hostconsole.js"), encoding="utf-8").read()
_gui = open(os.path.join(STREAM, "fui-web", "Interface", "AddOns", "WrathUnleashedUI", "Host.lua"), encoding="utf-8").read()
_lim = "Hours: 1 to 8760, or 0 for a permanent ban."
check("player realms ban by account, never by character, with HQ's limits", 'ban account %s' in _lua and "ban character" not in _lua
      and "unban character" not in _lua and _lim in _lua and _lim in _rs and "BAN_HOURS: u32 = 8760" in _rs
      and _lim in open(os.path.join(STREAM, "hq.py"), encoding="utf-8").read())
check("the same button words everywhere: Ban account / Unban account", all('"Ban account"' in t for t in (_js, _gui))
      and all('"Unban account"' in t for t in (_js, _gui)) and H.BAN_LABEL == "Ban account" and H.UNBAN_LABEL == "Unban account")
check("game actions are audited", {"game.kick", "game.mute", "game.rescue", "game.ban", "game.unban", "game.announce"}
      <= {a["action"] for a in audit() if a["ok"]})

# site-wide sanctions
check("a mod can't sanction", "err=Only admins" in act("modadm", "sanction.set", "WRMAL", type="mute", hours="2", reason="spam", confirm="WRMAL")[1])
check("a sanction needs the account typed", "err=Type WRMAL" in act("staff1", "sanction.set", "WRMAL", type="mute", hours="2", reason="spam")[1])
check("a sanction needs a reason", "err=Say why" in act("staff1", "sanction.set", "WRMAL", type="mute", hours="2", reason="", confirm="WRMAL")[1])
check("unknown sanction refused", "err=Pick" in act("staff1", "sanction.set", "WRMAL", type="ban", hours="2", reason="spam", confirm="WRMAL")[1])
check("nobody sanctioned yet", A.blocked("post", acct="WRMAL") == "" and A.blocked("submit", acct="WRMAL") == "")
r = act("staff1", "sanction.set", "WRMAL", "/wrath/admin/person?acct=WRMAL", type="mute", hours="2", reason="spam in the forum", confirm="WRMAL")
check("mute: stops posts and messages, not submissions", ok_(r) and "spam in the forum" in A.blocked("post", acct="WRMAL")
      and A.blocked("message", acct="wrmal") and A.blocked("submit", acct="WRMAL") == "" and A.blocked("realm", acct="WRMAL") == "")
check("the refusal says until when and why", re.search(r"until \d{4}-\d\d-\d\d \d\d:\d\d UTC: spam in the forum", A.blocked("post", acct="WRMAL")))
check("sanctions.json is not world-readable", os.stat(os.path.join(DATA, "admin", "sanctions.json")).st_mode & 0o777 == 0o640)
r = act("staff1", "sanction.set", "site:acc-mal", type="nosubmit", hours="0", reason="spam links", confirm="site:acc-mal")
check("nosubmit by website id: until lifted", ok_(r) and A.blocked("submit", site="acc-mal") and A.blocked("post", site="acc-mal") == "")
check("the owner can't be sanctioned", "err=The owner" in act("staff1", "sanction.set", "site:owner1", type="suspend", hours="1", reason="xyz", confirm="site:owner1")[1])
# the hooks
check("hook: the Workshop's link form (by website id)", "spam links" in wp.workshop._sanctioned({"id": "acc-mal"}) and wp.workshop._sanctioned({"id": "acc-ok"}) == "")
check("hook: + Your server (by game account): nosubmit is per website id, so WRMAL may still host", wp.hostflow._sanctioned("WRMAL") == "")
check("hook: the forum (posts, not reports)", "spam" in wp.forum._sanctioned({"acct": "WRMAL"}, "new") and wp.forum._sanctioned({"acct": "WRMAL"}, "report") == ""
      and wp.forum._sanctioned({"acct": "WRBOB"}, "new") == "")
_sspec = importlib.util.spec_from_file_location("soc_for_test", os.path.join(STREAM, "social.py"))
SOC = importlib.util.module_from_spec(_sspec)
_sspec.loader.exec_module(SOC)
SOC.bind(lambda n: {"admin": A}[n])
check("hook: friends and messages (send yes, read and block never)", "spam" in SOC._sanctioned("WRMAL", "send")
      and SOC._sanctioned("WRMAL", "read") == "" and SOC._sanctioned("WRMAL", "block") == "" and SOC._sanctioned("WRMAL", "report") == "")
SOC.bind(lambda n: {}[n])
check("hook: a portal without the console never refuses", SOC._sanctioned("WRMAL", "send") == "")
r = act("staff1", "sanction.set", "WRMAL", type="suspend", hours="1", reason="cool off", confirm="WRMAL")
SOC.bind(lambda n: {"admin": A}[n])
check("suspend: everything, incl. Hero Key sign-in, + Your server and reports", ok_(r) and bool(A.blocked("realm", acct="WRMAL"))
      and bool(wp.hostflow._sanctioned("WRMAL")) and bool(SOC._sanctioned("WRMAL", "report")))
check("Players lists sanctions in force", ">suspend<" in req("GET", "/wrath/admin/players", "staff1")[2])
check("hook: Hero Key sign-in refused while suspended", wp.herokey_issue("WRMAL", RID_A, 60) is None)
sid = next(k for k, v in A.sanctions().items() if v["type"] == "suspend")
r = act("staff1", "sanction.lift", sid, "/wrath/admin/person?acct=WRMAL")
check("lift: off at once, still in the file (lifted)", ok_(r) and A.blocked("realm", acct="WRMAL") == "" and A.sanctions()[sid]["lifted"]["by"] == "staff1")
check("lift twice: refused", "err=No such sanction" in act("staff1", "sanction.lift", sid)[1])
A._san_cache["key"] = None
doc = json.load(open(os.path.join(DATA, "admin", "sanctions.json")))
for v in doc["sanctions"].values():
    if v["type"] == "mute":
        v["until"] = int(time.time()) - 10
json.dump(doc, open(os.path.join(DATA, "admin", "sanctions.json"), "w"))
check("an expired sanction stops nothing", A.blocked("post", acct="WRMAL") == "")
# a suspended mod loses the console
r = act("staff1", "sanction.set", "site:modadm", type="suspend", hours="1", reason="on a break", confirm="site:modadm")
st, _, body = req("GET", "/wrath/admin", "modadm")
check("a suspended staff member gets no console", ok_(r) and st == 403 and "suspended" in body)
act("staff1", "sanction.lift", next(k for k, v in A.sanctions().items() if v.get("site") == "modadm"))
check("... and back after the lift", req("GET", "/wrath/admin", "modadm")[0] == 200)

# launcher sign-ins
r = act("staff1", "device.revoke", "WRMAL", id="abcdefabcdef")
check("sign out one launcher", ok_(r) and [t["id"] for t in FAKE_SOCIAL.toks["WRMAL"]] == ["123456123456"])
check("unknown sign-in refused", "err=No such sign-in" in act("staff1", "device.revoke", "WRMAL", id="ffffffffffff")[1])
check("a mod can't sign launchers out", "err=Only admins" in act("modadm", "device.revoke", "WRMAL", id="all")[1])

# realms: the relay block feed
check("block needs the realm id typed and a reason", "err=Type" in act("staff1", "realm.block", RID_A, reason="x")[1]
      and "err=Say why" in act("staff1", "realm.block", RID_A, confirm=RID_A)[1])
os.environ["WU_RELAY_FEED_KEY"] = "feedkey"
r = act("staff1", "realm.block", RID_A, "/wrath/admin/realms", reason="ships game data", confirm=RID_A)
st, hd, fd = feed("relay-block.json")
body_sig = {k: v for k, v in fd.items() if k != "sig"}
import hmac as _hmac
check("relay-block.json: ids and times, signed, no reasons or staff", ok_(r) and [x["id"] for x in fd["blocked"]] == [RID_A]
      and fd["sig"] == _hmac.new(b"feedkey", json.dumps(body_sig, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode(), hashlib.sha256).hexdigest()
      and "ships" not in json.dumps(fd) and "staff1" not in json.dumps(fd) and hd.get("Access-Control-Allow-Origin") == "*")
body = req("GET", "/wrath/admin/realms", "staff1")[2]
check("Realms: the official realm, community realms, relay blocks", "Official realm" in body and "Frost Haven" in body
      and "Blocked at the relay" in body and "ships game data" in body and ", signed" in body)
r = act("staff1", "realm.unblock", RID_A, "/wrath/admin/realms")
check("unblock: off the feed", ok_(r) and feed("relay-block.json")[2]["blocked"] == [])
del os.environ["WU_RELAY_FEED_KEY"]
check("unsigned feed without a key", "sig" not in feed("relay-block.json")[2])
check("each realm row: Block at the relay for admins, not for mods", 'value="realm.block"' in req("GET", "/wrath/admin/realms", "staff1")[2]
      and 'value="realm.block"' not in req("GET", "/wrath/admin/realms", "modadm")[2])

# the JSON API
st, hd, body = req("GET", "/wrath/admin/api/me", "staff1")
me = json.loads(body)
check("api/me: role, CSRF token, what this role can do", st == 200 and me["role"] == "admin" and len(me["csrf"]) == 40
      and "game.ban" in me["can"] and "flag.set" not in me["can"] and hd.get("X-Robots-Tag", "").startswith("noindex"))
check("api: signed out is 401 JSON (no redirect)", req("GET", "/wrath/admin/api/me")[0] == 401 and json.loads(req("GET", "/wrath/admin/api/me")[2])["ok"] is False)
check("api: a player is 403 JSON", req("GET", "/wrath/admin/api/me", "player")[0] == 403)
st, _, body = req("GET", "/wrath/admin/api/inbox", "modadm")
inb = json.loads(body)
check("api/inbox: items with the actions this role may run", st == 200 and inb["items"] and all(
      a["action"] not in ("ws.approve", "hub.approve") for x in inb["items"] for a in x["actions"]))
check("api/search and api/person", json.loads(req("GET", "/wrath/admin/api/search?q=mallory", "modadm")[2])["people"][0]["acct"] == "WRMAL"
      and json.loads(req("GET", "/wrath/admin/api/person?acct=WRMAL", "modadm")[2])["person"]["chars"] == ["Malchar"])
check("api/realm, api/audit, api/stats", json.loads(req("GET", f"/wrath/admin/api/realm?id={RID_A}", "modadm")[2])["realm"]["name"] == "Frost Haven"
      and json.loads(req("GET", "/wrath/admin/api/audit?n=5", "modadm")[2])["audit"]
      and "needs_you" in json.loads(req("GET", "/wrath/admin/api/stats", "modadm")[2]))
check("api/notes: the owner's notes for the owner only", "to_claude" in json.loads(req("GET", "/wrath/admin/api/notes", "owner1")[2])
      and "to_claude" not in json.loads(req("GET", "/wrath/admin/api/notes", "staff1")[2]))
check("api: unknown call 404", req("GET", "/wrath/admin/api/nope", "staff1")[0] == 404)


def api_act(who, doc, csrf=True, headers=None):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=30)
    h = {"X-Forwarded-For": "10.0.0.1", "Cookie": f"wr_session={who}", "Content-Type": "application/json", "Origin": "https://play.example"}
    if csrf:
        h["X-Leyline-CSRF"] = json.loads(req("GET", "/wrath/admin/api/me", who)[2])["csrf"]
    h.update(headers or {})
    c.request("POST", "/wrath/admin/api/act", body=json.dumps(doc).encode(), headers=h)
    r = c.getresponse()
    out = r.status, json.loads(r.read() or b"{}")
    c.close()
    return out


st, doc = api_act("modadm", {"action": "game.kick", "target": "Malchar"})
check("api/act: the same actions, JSON in and out, audited via api", st == 200 and doc["ok"] and audit()[-1]["via"] == "api"
      and audit()[-1]["action"] == "game.kick")
check("api/act: no CSRF token, refused", api_act("modadm", {"action": "game.kick", "target": "Malchar"}, csrf=False)[0] == 403)
check("api/act: the role still counts", api_act("modadm", {"action": "game.ban", "target": "WRMAL", "hours": 1, "reason": "abc", "confirm": "WRMAL"})[1]["ok"] is False)
check("api/act: another site refused", api_act("modadm", {"action": "game.kick", "target": "Malchar"}, headers={"Origin": "https://evil.example"})[0] == 403)

# CSRF: Sec-Fetch-Site and Origin/Referer
def raw_post(headers):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=30)
    body = urllib.parse.urlencode({"csrf": token("staff1"), "action": "report.note", "target": "bug:6", "text": "x", "back": "/wrath/admin"}).encode()
    h = {"X-Forwarded-For": "10.0.0.1", "Cookie": "wr_session=staff1", "Content-Type": "application/x-www-form-urlencoded"}
    h.update(headers)
    c.request("POST", "/wrath/admin/act", body=body, headers=h)
    st = c.getresponse().status
    c.close()
    return st


check("POST: Sec-Fetch-Site cross-site refused", raw_post({"Origin": "https://play.example", "Sec-Fetch-Site": "cross-site"}) == 403)
check("POST: Sec-Fetch-Site same-site (a sibling subdomain) refused", raw_post({"Origin": "https://play.example", "Sec-Fetch-Site": "same-site"}) == 403)
check("POST: no Origin and no Referer refused", raw_post({}) == 403)
check("POST: a Referer of ours is enough", raw_post({"Referer": "https://play.example/wrath/admin"}) == 303)
check("POST: another site's Referer refused", raw_post({"Referer": "https://play.example.evil.com/x"}) == 403)
check("POST: same-origin with our Origin passes", raw_post({"Origin": "https://play.example", "Sec-Fetch-Site": "same-origin"}) == 303)
check("POST: Origin null only with same-origin", raw_post({"Origin": "null"}) == 403 and raw_post({"Origin": "null", "Sec-Fetch-Site": "same-origin"}) == 303)

# roles: the owner grants a mod or an admin
r = act("owner1", "staff.grant", "new", account="newbie", name="New", role="mod")
check("grant a mod", ok_(r) and "(mod)" in req("GET", "/wrath/admin", "newbie")[2])
r = act("owner1", "staff.grant", "new", account="newbie", name="New", role="admin")
check("... then an admin", ok_(r) and "(admin)" in req("GET", "/wrath/admin", "newbie")[2])
check("unknown roles refused", "err=Pick helper, mod or admin" in urllib.parse.unquote(act("owner1", "staff.grant", "new", account="newbie", role="owner")[1]))
act("owner1", "staff.revoke", "newbie", confirm="newbie")
pages = "".join(req("GET", "/wrath/admin/" + t, "owner1")[2] for t in ("", "inbox", "players", "person?acct=WRMAL", "game", "notes", "realms"))
check("HQ pages: no secrets", "sekritsoappass" not in pages and "GAMEPASS" not in pages and A._secret().hex() not in pages and "feedkey" not in pages)
check("HQ pages: every one renders on the menu", all(req("GET", "/wrath/admin/" + t, "modadm")[0] == 200 for t in ("", "inbox", "players", "game", "notes", "realms", "numbers", "content", "system")))
check("signin step ships hq.py beside admin.py", 'install -m 644 hq.py "$WU/portal/hq.py"' in open(os.path.join(STREAM, "fastloop.sh")).read()
      and 'set "LINKS=%LINKS% hq.py"' in open(os.path.join(STREAM, "Update sign-in and warm-up.bat")).read())

# ---------------------------------------------------------------- HQ > Review (stream/staffdesk.py, linkqueue.py, feedback.py)
LQ, FB, SD = wp.linkqueue, wp.feedback, A.staffdesk
check("review: the portal loads linkqueue.py, feedback.py and staffdesk.py", LQ is not None and FB is not None and SD is not None)
WHO_A = {"acct": "WRPLAYER", "name": "Player"}
ok1, _, ID_ADDON = LQ.submit("https://github.com/Ann/Tidy-Bars", "addon", "nice bars", "site", WHO_A)
ok2, _, ID_LOOK = LQ.submit("https://github.com/ann/crisp-look", "look", "", "site", {"acct": "WRGM", "name": "Gm"})
ok3, _, ID_WARN = LQ.submit("https://ann.itch.io/frost-mod", "mod", "", "discord", {"discord": "70000000000000001", "name": "Ann D"})
ok4, _, ID_REALM = LQ.submit("https://play.example/r/frost", "realm", "", "site", {"acct": "WRNEW", "name": "New"})
check("links: clean addon, a look, a non-repo page and our own realm page are queued", ok1 and ok2 and ok3 and ok4, (ok1, ok2, ok3, ok4))
refused = {u: LQ.submit(u, k, "", "site", {"acct": "WROTHER", "name": "O"})[1] for u, k in (
    ("http://github.com/a/b", "mod"), ("https://evil.example/a/b", "mod"), ("https://github.com/a/b/releases/download/v1/x.zip", "mod"),
    ("https://github.com/a/b/blob/main/patch-Z.mpq", "mod"), ("https://github.com/MidnightZora737r/pack", "mod"),
    ("https://github.com/wowdecomp/client", "mod"), ("https://user:pw@github.com/a/b", "mod"), ("https://github.com/Ann/Tidy-Bars.git", "addon"),
    ("https://play.example/r/frost", "mod"), ("https://github.com/x/y", "nonsense"))}
check("links: http, unknown sites, downloads, game files, blocklisted and decompiled accounts, logins, duplicates, "
      "our site for non-realms, unknown kinds are all refused",
      all(refused.values()) and len(LQ.items("pending")) == 4 and "already waiting" in refused["https://github.com/Ann/Tidy-Bars.git"]
      and "blocklist" in refused["https://github.com/MidnightZora737r/pack"] and "not a file" in refused["https://github.com/a/b/releases/download/v1/x.zip"], refused)
A.set_sanction({"account": "owner1", "name": "Owner1", "role": "owner"}, "nosubmit", "WRBAD", "", 1, "spam links") if hasattr(A, "set_sanction") else None
okb, msgb, _ = LQ.submit("https://github.com/bad/spam", "mod", "", "site", {"acct": "WRBAD", "name": "Bad"})
check("links: a site sanction (no submissions) is obeyed", not okb and msgb, msgb)
LQ._times.clear()
q_raw = open(os.path.join(LQ.qdir(), "queue.jsonl")).read()
check("links: no Discord id on disk", "70000000000000001" not in q_raw)

sub = req("GET", "/wrath/admin/submissions", "owner1")[2]
check("submissions page: every waiting link, its checks, the menu says how many wait",
      all(x in sub for x in ("github.com/Ann/Tidy-Bars", "ann.itch.io/frost-mod", "crisp-look", "play.example/r/frost"))
      and "not a repo: check the page by hand" in sub and "Review (4)" in sub and 'rel="noopener noreferrer nofollow"' in sub, sub[:500])
msub = req("GET", "/wrath/admin/submissions", "modadm")[2]
check("submissions page (mod): no Approve on the look or the warned link, says why; no Undo",
      "Looks and HD packs are approved by the owner." in msub and "Links with a warning need an admin or the owner." in msub
      and 'value="submit.undo"' not in msub)
r = act("modadm", "submit.approve", ID_ADDON, "/wrath/admin/submissions")
hubdec = [json.loads(l) for l in open(os.path.join(DATA, "hub", "decisions.jsonl"))] if os.path.isfile(os.path.join(DATA, "hub", "decisions.jsonl")) else []
check("a mod approves a clean addon: approved, listed in the index too, in the public feed",
      ok_(r) and LQ.item(ID_ADDON)["status"] == "approved"
      and any(d["id"] == ID_ADDON and d["decision"] == "approve" and d["category"] == "addons" for d in hubdec)
      and ID_ADDON in req("GET", "/wrath/leyline/approved-links.json")[2], (r, hubdec[-1:]))
feed = json.loads(req("GET", "/wrath/leyline/approved-links.json")[2])
check("approved-links.json: public, ids, kinds and links only (no names, notes or accounts)",
      feed["links"] and set(feed["links"][0]) == {"id", "kind", "url", "approved_at"} and "WRPLAYER" not in json.dumps(feed)
      and req("GET", "/wrath/leyline/approved-links.json")[1].get("Access-Control-Allow-Origin") == "*")
check("a mod can't approve a look (owner) or a warned link (admin)",
      "Looks and HD packs" in act("modadm", "submit.approve", ID_LOOK)[1] and "need an admin" in act("modadm", "submit.approve", ID_WARN)[1]
      and LQ.item(ID_LOOK)["status"] == "pending")
check("an admin approves the warned link; only the owner the look", ok_(act("staff1", "submit.approve", ID_WARN))
      and "Looks and HD packs" in act("staff1", "submit.approve", ID_LOOK)[1] and ok_(act("owner1", "submit.approve", ID_LOOK)))
check("reject needs a reason", "Say why" in act("modadm", "submit.reject", ID_REALM)[1])
check("a mod rejects with a reason", ok_(act("modadm", "submit.reject", ID_REALM, reason="not a real realm"))
      and LQ.item(ID_REALM)["status"] == "rejected")
check("undo is the owner's", "Only the owner" in act("modadm", "submit.undo", ID_ADDON)[1] and "Only the owner" in act("staff1", "submit.undo", ID_ADDON)[1])
r = act("owner1", "submit.undo", ID_ADDON)
hubdec = [json.loads(l) for l in open(os.path.join(DATA, "hub", "decisions.jsonl"))]
check("the owner undoes a mod's approval: waiting again, and off the index", ok_(r) and LQ.item(ID_ADDON)["status"] == "pending"
      and hubdec[-1]["id"] == ID_ADDON and hubdec[-1]["decision"] == "delist" and ID_ADDON not in req("GET", "/wrath/leyline/approved-links.json")[2])
au = [x for x in audit() if x.get("action", "").startswith("submit.")]
check("every review decision and refusal is in the audit log with who and their role",
      any(x["action"] == "submit.approve" and x["role"] == "mod" and x["ok"] for x in au)
      and any(x["action"] == "submit.undo" and x["role"] == "owner" for x in au) and any(not x["ok"] for x in au))
done = req("GET", "/wrath/admin/submissions", "owner1")[2]
check("recently decided: who decided, their role, the reason, Undo for the owner",
      "by Modadm (mod)" in done and "not a real realm" in done and 'value="submit.undo"' in done)

# the hub's Submit a mod lands in the same queue; its Index-page decision counts
HUBQ = wp.hub_mod.hub if wp.hub_mod is not None else None
if HUBQ is not None:
    HUBQ._rl = None
    okh, _ = HUBQ.submit({"repo": "https://github.com/zed/zap-addon", "build": "3.3.5a", "category": "addons", "note": "zap"}, "10.7.0.1")
    hid = LQ.validate("https://github.com/zed/zap-addon", "addon")["id"]
    check("the hub's Submit a mod form puts a copy in the one queue", okh and LQ.item(hid) and LQ.item(hid)["via"] == "index"
          and LQ.item(hid)["status"] == "pending")

# helpers: a mod-level account panel for recruits
check("the owner grants a helper", ok_(act("owner1", "staff.grant", "new", account="newbie", name="Newbie", role="helper")))
st, _, hb = req("GET", "/wrath/admin", "newbie")
check("a helper's home is Submissions; the menu has only Review", st == 200 and "<h2>Submissions</h2>" in hb and "(helper)" in hb
      and 'href="/wrath/admin/players"' not in hb and 'value="submit.approve"' not in hb and 'value="submit.note"' in hb)
check("a helper sees Feedback, not Players, Forum, Staff, Upgrades, Audit or the API",
      req("GET", "/wrath/admin/feedback", "newbie")[0] == 200
      and all(req("GET", "/wrath/admin/" + t, "newbie")[0] == 403 for t in ("players", "forum", "staff", "upgrades", "audit", "inbox", "system"))
      and req("GET", "/wrath/admin/api/inbox", "newbie")[0] == 403)
HTOK = token("newbie", "/wrath/admin/submissions")
check("a helper leaves a note; can't approve or reject", ok_(act("newbie", "submit.note", ID_ADDON, "/wrath/admin/submissions", text="looks fine to me", csrf=HTOK))
      and "Only moderators" in act("newbie", "submit.approve", ID_ADDON, csrf=HTOK)[1] and "Only moderators" in act("newbie", "submit.reject", ID_ADDON, reason="x", csrf=HTOK)[1]
      and "looks fine to me" in req("GET", "/wrath/admin/submissions", "owner1")[2])
stp = req("GET", "/wrath/admin/staff", "owner1")[2]
check("staff page: helper in the roles, the What each role can do table, how to recruit",
      '<option value="helper">' in stp and "What each role can do" in stp and "See secrets, keys or emails; deploy" in stp
      and "To recruit someone" in stp)
act("owner1", "staff.revoke", "newbie", confirm="newbie")

# feedback digest
os.makedirs(os.path.join(DATA, "bugs"), exist_ok=True)
now_s = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
with open(os.path.join(DATA, "bugs", "intake.jsonl"), "w") as f:
    for i, t in enumerate(["The launcher crashes when I press Play, terrible", "game crashed again <script>x</script> mail me at a@b.example",
                           "I love the new Den build mode, so smooth", "can't log in with my hero key, password fails"]):
        f.write(json.dumps({"id": f"s{i}", "source": "site", "at": now_s, "reporter": "P", "text": t}) + "\n")
check("feedback: Run now is an admin's", "Only admins" in act("modadm", "feedback.run", "now")[1] and ok_(act("staff1", "feedback.run", "now")))
fp = req("GET", "/wrath/admin/feedback", "modadm")[2]
d = FB.latest(os.path.join(DATA, "ops"))
check("feedback: themes with counts and mood; quotes escaped and redacted",
      d and d["items"] == 4 and any(t["id"] == "crash" and t["count"] == 2 and t["mood"] < 0 for t in d["themes"])
      and any(t["id"] == "den" and t["mood"] > 0 for t in d["themes"]) and "Crashes and errors" in fp
      and "<script>x" not in fp and "a@b.example" not in fp and "[email]" in fp, (d or {}).get("themes"))

# upgrades and the launcher version policy
check("upgrades: only admins propose", "Only admins" in act("modadm", "upgrade.propose", "new", title="Launcher 0.5", kind="launcher")[1])
r = act("staff1", "upgrade.propose", "new", "/wrath/admin/upgrades", title="Launcher 0.5.0 to stable", kind="launcher", version="0.5.0", what="new sign-in screen")
ups = SD.upgrades()
uid = next(iter(ups), "")
check("upgrades: proposed, waiting for the owner", ok_(r) and ups[uid]["status"] == "proposed")
check("upgrades: only the owner approves or holds", "Only the owner" in act("staff1", "upgrade.approve", uid)[1])
r = act("owner1", "upgrade.approve", uid, note="go")
check("upgrades: approved with the release window to ship in", ok_(r) and SD.upgrades()[uid]["status"] == "approved"
      and "UTC" in SD.upgrades()[uid]["ship_window"])
check("upgrades: hold", ok_(act("owner1", "upgrade.hold", uid, note="wait for the fix")) and SD.upgrades()[uid]["status"] == "held")
upg = req("GET", "/wrath/admin/upgrades", "owner1")[2]
check("upgrades page: the window, proposals, the version policy form (owner)",
      "When to push updates" in upg and "Launcher 0.5.0 to stable" in upg and 'value="version.set"' in upg)
check("version policy: the owner types set; a minimum newer than stable is refused",
      "Type set" in act("owner1", "version.set", "policy", minimum="0.4.0")[1]
      and "newer than the current stable" in act("owner1", "version.set", "policy", minimum="9.0.0", confirm="set")[1]
      and "Only the owner" in act("staff1", "version.set", "policy", minimum="0.4.0", confirm="set")[1])
r = act("owner1", "version.set", "policy", minimum="0.4.0", notice="Please update to keep playing together", confirm="set")
st, hd, vb = req("GET", "/wrath/leyline/version.json")
v = json.loads(vb) if st == 200 else {}
check("version.json: public, current stable, minimum, notice and the release window",
      ok_(r) and v.get("current") == "0.4.2" and v.get("minimum") == "0.4.0" and v.get("notice", "").startswith("Please")
      and v.get("window", {}).get("text") and hd.get("Access-Control-Allow-Origin") == "*", v)
rev = "".join(req("GET", "/wrath/admin/" + t, "owner1")[2] for t in ("submissions", "feedback", "upgrades", "staff"))
check("Review pages: no secrets", "sekritsoappass" not in rev and "GAMEPASS" not in rev and A._secret().hex() not in rev)

# the release window from presence samples (feedback.py, unit)
W = tempfile.mkdtemp(prefix="wu-win-")
base = 1730073600   # a Monday 00:00 UTC
for day in range(14):
    for hr in range(24):
        for k in range(4):
            FB.sample(W, 2 if hr in (9, 10) and day % 7 == 2 else 20 + hr, base + day * 86400 + hr * 3600 + k * 600)
w = FB.release_window(W, base + 15 * 86400)
check("release window: the quietest two hours of the week from the samples, in UTC and Phoenix time",
      w["weekday"] == 2 and w["hour"] == 9 and w["text"].startswith("Wednesday 09:00-11:00 UTC") and "Wednesday 02:00" in w["phoenix"], w)
_cli = subprocess.run([sys.executable, "-I", os.path.join(STREAM, "feedback.py"), "digest", "--data", DATA, "--wu", os.path.join(T, "nowu"),
                       "--out", os.path.join(T, "ops-cli")], capture_output=True, text=True)
check("feedback.py from the command line (the Control step): last line FEEDBACK OK, the missing host sources named",
      _cli.returncode == 0 and _cli.stdout.strip().splitlines()[-1].startswith("FEEDBACK OK: 4 items")
      and re.search(r"not here: [a-z, ]*discord, backlog", _cli.stdout), _cli.stdout + _cli.stderr)
check("release window: the default until there is data", FB.release_window(tempfile.mkdtemp(), base)["basis"].startswith("default"))
shutil.rmtree(W, ignore_errors=True)

# the website's Submit a link form
st, _, sp = req("GET", "/wrath/submit")
check("/wrath/submit signed out: Log in, and /submit in the Discord", st == 200 and "Log in" in sp and "/submit" in sp)
st, _, sp = req("GET", "/wrath/submit?kind=look", "player")
tk = re.search(r'name="t" value="([0-9a-f]{32})"', sp)
check("/wrath/submit signed in: the form with the kinds, the look picked", st == 200 and tk and 'value="look" checked' in sp)
st, _, sp = req("POST", "/wrath/submit", "player", {"t": tk.group(1) if tk else "", "kind": "realm", "url": "https://play.example/r/cozy"})
check("/wrath/submit: queued, thanks page", st == 200 and "in the queue" in sp and any("r/cozy" in r["url"] for r in LQ.items("pending")), sp[-400:])
st, _, sp = req("POST", "/wrath/submit", "player", {"t": "0" * 32, "kind": "mod", "url": "https://github.com/q/r"})
check("/wrath/submit: the form token is needed", st == 400 and "timed out" in sp)
check("signin step ships linkqueue.py (with the blocklist), feedback.py and staffdesk.py",
      all(x in open(os.path.join(STREAM, "fastloop.sh")).read() for x in ('install -m 644 linkqueue.py', 'install -m 644 feedback.py', 'install -m 644 staffdesk.py'))
      and all(x in open(os.path.join(STREAM, "Update sign-in and warm-up.bat")).read() for x in ("linkqueue.py link-blocklist.json", "feedback.py", "staffdesk.py")))

# ---------------------------------------------------------------- rate limits
A.POSTS_PER_MIN, A.IP_PER_MIN, A.PAGES_PER_MIN = LIMITS
A.limits = A._Window(60)
tok = token("staff1")
codes = [req("POST", "/wrath/admin/act", "staff1", {"csrf": tok, "action": "report.note", "target": "bug:6", "text": f"n{i}",
                                                     "back": "/wrath/admin"}, ip=f"10.1.0.{i}")[0] for i in range(A.POSTS_PER_MIN + 1)]
check("at most 30 changes a minute per account", codes[:-1] == [303] * A.POSTS_PER_MIN and codes[-1] == 429, codes)
A.limits = A._Window(60)
codes = [req("GET", "/wrath/admin", None, ip="10.2.0.1")[0] for _ in range(A.IP_PER_MIN + 1)]
check("at most 120 requests a minute per address before sign-in", codes[-1] == 429 and codes[0] == 302)

# ---------------------------------------------------------------- shipping: the signin step and the scripts step
fl = open(os.path.join(STREAM, "fastloop.sh"), encoding="utf-8").read()
bat = open(os.path.join(STREAM, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
sp = open(os.path.join(ROOT, "realm", "Server pass - finder 10 and bug reports.bat"), encoding="utf-8").read()
check("signin step ships admin.py and wu-mod.py, backs them up, puts them back", 'install -m 644 admin.py "$WU/portal/admin.py"' in fl
      and 'cp -p "$BK/admin.py" "$WU/portal/admin.py"' in fl and 'install -m 644 wu-mod.py "$WU/portal/wu-mod.py"' in fl
      and 'set "LINKS=%LINKS% admin.py"' in bat and all(" %s " % n in re.search(r"\$\(ls ([^)]*)\)", bat).group(1) + " " for n in ("admin.py", "wu-mod.py"))
      and subprocess.run(["bash", "-n", os.path.join(STREAM, "fastloop.sh")]).returncode == 0)
check("scripts step ships ov_admin.lua", 'copy /y "custom\\lua_scripts\\ov_admin.lua"' in sp)

shutil.rmtree(T, ignore_errors=True)
print(f"{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
