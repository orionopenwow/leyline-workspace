"""The website as a visitor meets it (site review, 4 Nov): the portal with the hub's front door on (as live), fake
services on loopback, nothing live. One crawl from the main pages (two hops), signed out and signed in, then:

  * every page: no server error, the site's CSP (frame-ancestors 'none') and nosniff, no game-brand word outside the
    legal line, no money link, outside links only to the hosts the site means to link, no link to the hub's old
    preview address (a 301 for every click: the Hero Key, Sign in and leaderboards pages had 14 each);
  * hostile query values on every page and a hostile display name are never echoed raw;
  * the kill switch ("closed") closes the front door's own pages and forms under /wrath/leyline/ too, while the
    launcher's feeds there (*.json, the release mirror) keep answering (it left the Mods pages, the forum and its
    forms open);
  * Submit a mod answers a bad Content-Length with 400, not 413;
  * KNOWN (owner decisions, not failures yet): page titles say "Leyline" (some say the realm's name), the sign-in
    words are "Log in" (owner 2 Nov), and the docs keep client version numbers out of sentences.

    python3 -I stream/tests/test_site_review.py stream/wrath_portal.py
"""
import collections, html as H, http.client, importlib.util, json, os, re, shutil, sys, tempfile, threading, time, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

REALM_PORT, PAGE_PORT = 18461, 18462
PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
HERE = os.path.dirname(PORTAL)
TMP = tempfile.mkdtemp(prefix="wu-review-")
for d in ("data", "mods/files", "armory/c", "hub", "items"):
    os.makedirs(os.path.join(TMP, d), exist_ok=True)
shutil.copy(os.path.join(HERE, "hub", "community-index.json"), os.path.join(TMP, "hub"))
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL=f"http://127.0.0.1:{REALM_PORT}",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="leylinelauncher.com", WU_DL_PROBE="0",
                  WU_RELAY_LISTING="http://127.0.0.1:9/realms.json", WU_PORTAL_DATA=os.path.join(TMP, "data"),
                  WU_HUB_DIR=os.path.join(TMP, "hub"), WU_MODS_DIR=os.path.join(TMP, "mods"),
                  WU_WORKSHOP_ITEMS_DIR=os.path.join(TMP, "items"), WU_ARMORY_DIR=os.path.join(TMP, "armory"),
                  WU_ART_DIR=os.path.join(HERE, "www-files", "art"), WU_HUB_FRONT="1")
for k in ("WU_HUB", "WU_BRAND_FILE", "WU_GUEST_DEMO"):
    os.environ.pop(k, None)

passed = failed = 0
known = []


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1; print("PASS", name)
    else:
        failed += 1; print("FAIL", name, str(extra)[:600])


def known_fail(name, ok, extra=""):
    """A real finding the owner decides on (copy, wording): reported, not counted. Promote to check() once fixed."""
    if ok:
        print("PASS (was KNOWN)", name)
    else:
        known.append(name); print("KNOWN", name, str(extra)[:600])


# The website's sign-in service: wr_session=good is "Tester", wr_session=evil has a hostile display name.
WHO = {"good": ("Tester", "WR000123"), "evil": ('<zq9 onerror=x>"\'', "WR000666")}


class Realm(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_GET(self):
        m = re.search(r"wr_session=([A-Za-z0-9]+)", self.headers.get("Cookie", ""))
        who = WHO.get(m.group(1)) if m else None
        path = self.path.split("?")[0]
        if path in ("/api/me", "/api/play") and who:
            doc = {"display_name": who[0], "username": "tester"} if path == "/api/me" else {"user": who[1], "pass": "Pa55word1234abcd"}
            body, st = json.dumps(doc).encode(), 200
        else:
            body, st = b"{}", 401 if path.startswith("/api/") else 404
        self.send_response(st); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)

    do_POST = do_GET


spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
wp.realm_status = lambda: {"online": True, "players": 3, "bots": 10}
for mod in ("social", "profiles", "herokeys", "forum"):
    m = getattr(wp, mod, None)
    check(f"{mod}.py loaded beside the portal", m is not None)
    if m is not None:
        m.setup(start_thread=False) if mod == "social" else m.setup()
check("hub's front door on", wp.hub_framed())
for port, h in ((REALM_PORT, Realm), (PAGE_PORT, wp.PageHandler)):
    s = ThreadingHTTPServer(("127.0.0.1", port), h); s.daemon_threads = True
    threading.Thread(target=s.serve_forever, daemon=True).start()

_ip = [0]


def req(path, method="GET", body=None, cookie="", headers=None):
    _ip[0] += 1   # a fresh address each time: the browse limits are not what this test is about
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=30)
    hd = {"X-Forwarded-For": f"10.{40 + _ip[0] // 62500 % 200}.{_ip[0] // 250 % 250}.{_ip[0] % 250 + 1}", "Host": "leylinelauncher.com"}
    if cookie:
        hd["Cookie"] = cookie
    if body is not None:
        hd["Content-Type"] = "application/x-www-form-urlencoded"
        body = body.encode() if isinstance(body, str) else body
    hd.update(headers or {})
    c.request(method, path, body=body, headers=hd)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


def visible(p):
    p = (p.replace('not affiliated with or endorsed by Blizzard Entertainment', ' ') if isinstance(p, str) else p.replace(b'not affiliated with or endorsed by Blizzard Entertainment', b' '))   # the legal line is the one allowed mention (audit 12 Nov: it is in every footer now)
    p = re.sub(r"<(style|script|svg)\b.*?</\1>", " ", p, flags=re.S | re.I)
    p = re.sub(r"<!--.*?-->", " ", p, flags=re.S)
    attrs = " ".join(re.findall(r'(?:alt|title|content|aria-label|placeholder|value)="([^"]*)"', p))
    return H.unescape(re.sub(r"<[^>]+>", " ", p) + " " + attrs)


BANNED = re.compile(r"(?i)world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard")
MONEY = re.compile(r"(?i)patreon\.com|ko-fi\.com|paypal\.|buymeacoffee|opencollective\.com|github\.com/sponsors|liberapay|cash\.app|venmo")
# Outside hosts a page may link: the code (GitHub), the Discord, Reddit, the HD pack's own site (credited with its
# team's OK), Docker (the hosting guide), and the realm's own address (a realmlist, shown to copy).
HOSTS = {"github.com", "discord.gg", "discord.com", "www.reddit.com", "reddit.com", "projectreforged.github.io",
         "www.docker.com", "docs.docker.com", "play.openvanilla.app", "leylinelauncher.com"}
VERSIONS = re.compile(r"\b(?:3\.3\.5a?|1\.12(?:\.1)?|2\.4\.3|4\.3\.4|5\.4\.8)\b")
START = ["/wrath/leyline", "/wrath/start", "/wrath/download", "/wrath/looks", "/wrath/docs", "/wrath/key", "/wrath/signin",
         "/wrath/bugs", "/wrath/submit", "/wrath/host", "/wrath/survivors", "/wrath/modes", "/wrath/demo", "/wrath/leaderboards",
         "/wrath/leyline/forum", "/wrath/leyline/catalog", "/wrath/leyline/realms", "/wrath/leyline/gallery",
         "/wrath/leyline/news", "/wrath/leyline/creators", "/wrath/leyline/submit", "/wrath/armory", "/wrath/nope"]


def crawl(cookie, limit=260):
    seen, queue, pages = set(), collections.deque((p, 0) for p in START), {}
    while queue and len(pages) < limit:
        p, d = queue.popleft()
        if p in seen:
            continue
        seen.add(p)
        st, h, b = req(p, cookie=cookie)
        pages[p] = (st, h, b)
        if not h.get("content-type", "").startswith("text/html") or d >= 2:
            continue
        for href in re.findall(r'href="([^"]+)"', b.decode("utf-8", "replace")):
            u = urllib.parse.urljoin(p, H.unescape(href)).split("#")[0]
            if (u.startswith("/wrath") and not u.startswith(("/wrath/art/", "/wrath/play", "/wrath/avatar/"))
                    and "/download" not in u and "signout" not in u and "/go/" not in u):
                queue.append((u, d + 1))
    return pages


for label, cookie in (("signed out", ""), ("signed in", "wr_session=good")):
    pages = crawl(cookie)
    html_pages = {p: v for p, v in pages.items() if v[1].get("content-type", "").startswith("text/html")}
    check(f"{label}: the crawl found the site ({len(pages)} addresses, {len(html_pages)} pages)", len(html_pages) > 80, len(html_pages))
    # /wrath and /wrath/account make the game login over SOAP, which is down here: 503 is their honest answer
    errs = [f"{v[0]} {p}" for p, v in pages.items() if v[0] >= 500 and not (v[0] == 503 and p.split("?")[0] in ("/wrath", "/wrath/account", "/wrath/key"))]
    check(f"{label}: no server errors", not errs, errs)
    no_csp = [p for p, v in html_pages.items() if "frame-ancestors 'none'" not in v[1].get("content-security-policy", "")
              or v[1].get("x-content-type-options") != "nosniff"]
    check(f"{label}: every page has the site's CSP and nosniff", not no_csp, no_csp[:10])
    brand, money, hosts, preview, titles, signin, vers = [], [], collections.Counter(), [], [], collections.Counter(), collections.Counter()
    mpq = set()
    for p, (st, h, b) in html_pages.items():
        s = b.decode("utf-8", "replace")
        v = visible(s)
        for m in BANNED.finditer(v):
            ctx = v[max(0, m.start() - 80):m.end() + 80]
            if re.match(r"(?i)lichking\.mpq", v[m.start():m.end() + 4]):
                mpq.add(p)   # the game's own file name, which players search for: the owner's call (below)
            elif "not affiliated" not in ctx.lower():
                brand.append(f"{p}: ...{' '.join(ctx.split())}...")
        money += [f"{p}: {m.group(0)}" for m in MONEY.finditer(s)]
        for href in re.findall(r'(?:href|src|action)="(https?://[^"]+|//[^"]+)"', s):
            hosts[urllib.parse.urlsplit(H.unescape(href)).netloc.lower()] += 1
        if re.search(r'href="/wrath/leyline-preview', s):
            preview.append(p)
        t = re.search(r"<title>([^<]*)</title>", s)
        if t and st == 200 and not H.unescape(t.group(1)).strip().endswith(("Leyline", "Leyline docs", "Leyline Admin")):
            titles.append(f"{p}: {H.unescape(t.group(1))}")
        for m in re.finditer(r"\bSign in\b", v):
            signin[p.split("?")[0]] += 1
        if p.startswith("/wrath/docs"):
            # a version chip (docs.py VERSION_CHIP, {{v:...}}) and code spans are the allowed badges and data fields;
            # only a number left in the sentence itself counts (fix round 4 Nov: the chips were counted too)
            prose = visible(re.sub(r'<span class="ver"[^>]*>[^<]*</span>|<code>[^<]*</code>|<pre\b.*?</pre>', " ", s, flags=re.S))
            for m in VERSIONS.finditer(prose):
                vers[p] += 1
    check(f"{label}: no game-brand word outside the legal line", not brand, brand[:5])
    check(f"{label}: no money, tip or donation links", not money, money[:5])
    odd = sorted(set(hosts) - HOSTS)
    check(f"{label}: outside links only to the hosts the site means to link", not odd, odd)
    check(f"{label}: no link to the hub's old preview address (each one a 301)", not preview, preview)
    known_fail(f"{label}: every page title ends with Leyline (some say the realm's name)", not titles, sorted(set(t for t in titles if "/wrath/b/" not in t))[:12] + sorted(set(titles))[:2])
    check(f"{label}: the sign-in words are \"Log in\" (owner 2 Nov), not \"Sign in\"", not signin,
               f"{sum(signin.values())} on {len(signin)} pages: " + ", ".join(sorted(signin)[:12]))
    if label == "signed out":
        known_fail("docs: the game's archive name (lichking.MPQ) is shown to players: allowed as a file name?", not mpq, sorted(mpq))
        check("docs: no client version numbers in sentences (rule: data fields and badges only)", not vers,
                   ", ".join(f"{p} ({n})" for p, n in sorted(vers.items())))
        crawled = [p for p, v in html_pages.items() if v[0] == 200]

# ---------------------------------------------------------------- hostile values are never echoed raw
EVIL = '<zq1 x="1">\'"'
q = urllib.parse.quote(EVIL)
params = ("q", "next", "f", "cat", "sort", "build", "tag", "region", "lang", "kind", "where", "page", "url", "note", "go",
          "id", "view", "t", "done", "err", "msg", "back", "source", "client", "name", "c", "p", "realm", "guest")
raw = []
for p in crawled:
    url = p + ("&" if "?" in p else "?") + "&".join(f"{k}={q}" for k in params)
    for cookie in ("", "wr_session=evil"):
        st, h, b = req(url, cookie=cookie)
        if b"<zq1" in b or b"<zq9" in b:
            i = max(b.find(b"<zq1"), b.find(b"<zq9"))
            raw.append(f"{st} {url[:80]} {b[max(0, i - 80):i + 30]!r}")
        if st >= 500 and st != 503:
            raw.append(f"{st} {url[:80]}")
check(f"hostile query values and display name: never echoed raw, never a crash ({len(crawled) * 2} pages)", not raw, raw[:5])

# ---------------------------------------------------------------- what the owner approves (standing rule: found mods,
# addons, looks and HD packs stay pending until the owner approves them; a tool or Claude never approves them)
lk = json.load(open(os.path.join(HERE, "looks.json"), encoding="utf-8"))["looks"]
up = json.load(open(os.path.join(HERE, "upstream-addons.json"), encoding="utf-8"))["projects"]
by_claude = [x["id"] for x in lk if (x.get("review") or {}).get("by") == "Claude" and x.get("author") != "Leyline"]
by_claude += [x["id"] for x in up if (x.get("review") or {}).get("by") == "Claude"]
known_fail("third-party looks and addons shown on the site carry an owner's approval (review.by is not Claude)", not by_claude,
           f"{len(by_claude)}: " + ", ".join(by_claude[:8]))

# ---------------------------------------------------------------- page weight (docs/SITE-SPEED-AUDIT.md A)
heavy = []
for p in ("/wrath/leyline", "/wrath/leyline/catalog", "/wrath/leyline/forum", "/wrath/leyline/realms", "/wrath/signin"):
    st, h, b = req(p)
    if len(b) > 120_000:
        heavy.append(f"{p} {len(b) // 1024} KB")
known_fail("main pages under 120 KB of HTML before compression (the styles are inline on every page)", not heavy, heavy)

# ---------------------------------------------------------------- Submit a mod: a bad Content-Length
for bad in ("-5", "abc"):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=10)
    c.putrequest("POST", "/wrath/leyline/submit"); c.putheader("X-Forwarded-For", "10.7.7.7"); c.putheader("Content-Length", bad)
    c.endheaders(); r = c.getresponse(); r.read()
    check(f"Submit a mod: Content-Length {bad} is a bad request (400), not 'too large'", r.status == 400, r.status)

# ---------------------------------------------------------------- one same-site check for every form (fix round 4 Nov)
for hd, want in (({"Sec-Fetch-Site": "cross-site"}, 403), ({"Origin": "https://evil.example"}, 403),
                 ({"Origin": "https://leylinelauncher.com", "Sec-Fetch-Site": "same-origin"}, None),
                 ({"Origin": "https://play.openvanilla.app"}, None)):
    st, h, b = req("/wrath/leyline/submit", "POST", "repo=https://github.com/a/b", headers=hd)
    check(f"Submit a mod: {hd} -> " + ("403 (another site)" if want else "not refused as another site"),
          st == 403 if want else st != 403, st)
for path in ("/wrath/bugs", "/wrath/submit"):
    st, h, b = req(path, "POST", "text=x", cookie="wr_session=good", headers={"Origin": "https://evil.example"})
    check(f"POST {path} from another site: 403 (the portal's same_site, not only the forum's)", st == 403, st)


class _H:
    def __init__(self, **hd):
        self.headers = hd


check("same_site: a missing Origin is ours, both names are ours, null only with same-origin",
      wp.same_site(_H()) and wp.same_site(_H(Origin="https://leylinelauncher.com"))
      and wp.same_site(_H(Origin="https://play.openvanilla.app")) and not wp.same_site(_H(Origin="null"))
      and wp.same_site(_H(Origin="null", **{"Sec-Fetch-Site": "same-origin"})) and not wp.same_site(_H(Origin="https://x.leylinelauncher.com.evil")))
check("guest passes off: /wrath?demo=1 is the same 200 as /wrath/demo",
      req("/wrath?demo=1")[0] == 200 == req("/wrath/demo")[0])
check("Admin Console for a player who isn't staff: a page with an h1", b"<h1>Staff only</h1>" in wp.admin.denied_page("WR1")
      if getattr(wp, "admin", None) else True)

# ---------------------------------------------------------------- the kill switch closes the front door too
FILE = os.path.join(TMP, "data", "leyline", "shutdown.json")
os.makedirs(os.path.dirname(FILE), exist_ok=True)
with open(FILE, "w") as f:
    json.dump({"mode": "closed", "title": "Leyline has closed", "message": "Thank you all."}, f)
for p in ("/wrath/leyline", "/wrath/leyline/catalog", "/wrath/leyline/forum", "/wrath/leyline/realms", "/wrath/leyline/submit",
          "/wrath/leyline/gallery", "/wrath/leyline/forum/new"):
    st, h, b = req(p)
    check(f"closed: {p} is the farewell page", st == 503 and b"ly-farewell" in b, st)
for p, body in (("/wrath/leyline/forum/new", "c=general&title=x&body=y"), ("/wrath/leyline/submit", "repo=https://github.com/a/b")):
    st, h, b = req(p, "POST", body, cookie="wr_session=good")
    check(f"closed: POST {p} is refused with the farewell page", st == 503 and b"ly-farewell" in b, st)
for p in ("/wrath/leyline/flags.json", "/wrath/leyline/catalog.json", "/wrath/leyline/news.json", "/wrath/leyline/releases/latest.json",
          "/wrath/status.json", "/wrath/admin"):
    st, h, b = req(p)
    check(f"closed: the feed {p} still answers itself", b"ly-farewell" not in b, st)
check("closed: shutdown_open keeps feeds, not pages",
      wp.shutdown_open("/wrath/leyline/mods.json") and wp.shutdown_open("/wrath/leyline/releases/Leyline.exe")
      and not wp.shutdown_open("/wrath/leyline/forum") and not wp.shutdown_open("/wrath/leyline/forum/t/1.json")
      and not wp.shutdown_open("/wrath/leyline/catalog"))
os.remove(FILE)
time.sleep(0.01)
st, h, b = req("/wrath/leyline/catalog")
check("file gone: the front door is back", st == 200 and b"ly-farewell" not in b, st)

print(f"\n{passed}/{passed + failed} passed" + (f"; {len(known)} known (owner decisions): " + "; ".join(known) if known else ""))
sys.exit(1 if failed else 0)
