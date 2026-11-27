"""The static site (tools/site-static/build.py): every read-only address an installed launcher fetches is a file and
parses; the addresses that needed the server are listed in docs/STATIC-SITE.md; no game or maker names except the
legal line; no link to a page that closed; no link to the old server's API; the flags turn every server feature off;
the realm list has the launcher's shape and no official realm; bad realm or news entries stop the build.

    python3 tools/site-static/test_build.py
"""
from __future__ import annotations

import hashlib
import html
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
import urllib.parse
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
spec = importlib.util.spec_from_file_location("site_build", HERE / "build.py")
B = importlib.util.module_from_spec(spec)
spec.loader.exec_module(B)

STAMP = "2026-11-27T00:00:00Z"
LEGAL = "not affiliated with or endorsed by Blizzard Entertainment"
BANNED = re.compile(r"(?i)world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard")

# Every site address the launcher names, sorted into what the static site serves and what closed with the server.
# A new address in the launcher fails test_every_launcher_address_is_sorted until it is put in one of these.
STATIC = {
    "/": "the front page",
    "/wrath/start": "Start here",
    "/wrath/docs": "the docs",
    "/wrath/leyline/news": "News (page)",
    "/wrath/leyline/news.json": "News (feed)",
    "/wrath/leyline/flags.json": "feature flags: all off",
    "/wrath/leyline/mods.json": "the mod rankings feed",
    "/wrath/mods/all.json": "the mods catalog",
    "/wrath/mods": "Mods (moves to /wrath/leyline/catalog)",
    "/wrath/modes.json": "game modes: none now",
    "/wrath/realms.json": "the realm list (site/realms.json)",
    "/wrath/status.json": "the official realm's status: retired, offline",
    "/wrath/api/boards/index.json": "leaderboards index: empty",
    "/wrath/leyline/submit": "Submit a mod (moves to a GitHub issue)",
    "/wrath/leyline/forum": "the forum (moves to the Discord)",
    "/wrath/bugs": "Report a bug (moves to GitHub issues)",
}
SERVER_ONLY = {
    "/wrath": "Play in the browser and the account page (a page that moves to the front page)",
    "/wrath/profile": "profiles",
    "/account/key": "the Hero Key",
    "/wrath/link": "linked launchers",
    "/wrath/leyline/forum/new": "new forum topic",
    "/wrath/forum/api": "the forum's API",
    "/wrath/api/social": "friends and messages on the website",
    "/wrath/api/listings": "listing submissions",
    "/wrath/mods/list": "List your mod (form)",
    "/wrath/api/dens": "the Den market's API",
    "/wrath/dens": "the Den market",
    "/wrath/bugs/api/report": "bug reports (POST)",
    "/wrath/admin": "HQ, the admin console",
    "/wrath/host": "the Host page on the website",
    "/wrath/realms/report": "report a realm",
    "/wrath/r/": "share links",
    "/wrath/avatar/p/": "profile pictures",
    "/wrath/play": "the browser game",
    "/wrath/x": "(the address helper's own test)",
    "/wrath/api/": "every API under it: boards (local runs), hub install counts and ratings, social, listings, dens",
    "/auth/google/": "Google sign-in",
    "/wrath/art/leyline-shot-backdrop.webp": "the owner's backdrop picture (not ours to publish)",
}


def visible(page: str) -> str:
    page = page.replace(LEGAL, " ")
    page = re.sub(r"<style.*?</style>|<script.*?</script>|<!--.*?-->", " ", page, flags=re.S)
    attrs = " ".join(re.findall(r'(?:alt|title|content|aria-label|placeholder|value)="([^"]*)"', page))
    return html.unescape(re.sub(r"<[^>]+>", " ", page) + " " + attrs)


def launcher_addresses() -> set[str]:
    """Every site address the launcher's code names: site!("...") in the core, LeylineAddresses.site("...") and
    literal https://leylinelauncher.com/... in the UI, and the core's path constants used with the site."""
    found = set()
    src = REPO / "launcher" / "src-tauri" / "src"
    for p in src.rglob("*.rs"):
        t = p.read_text(encoding="utf-8", errors="replace")
        t = t.split("#[cfg(test)]", 1)[0]
        found |= set(re.findall(r'site!\("([^"]*)"\)', t))
        found |= set(re.findall(r'pub const [A-Z_]+: &str = "(/wrath/[^"]*)"', t))
    for p in (REPO / "launcher" / "ui").rglob("*.js"):
        if "test" in p.name or "node_modules" in p.parts:
            continue
        t = p.read_text(encoding="utf-8", errors="replace")
        found |= set(re.findall(r"""\bsite\(["'`]([^"'`$]*)""", t))
        found |= {urllib.parse.urlsplit(u).path or "/" for u in re.findall(r"https://leylinelauncher\.com[^\"'`\s),]*", t)}
    out = set()
    for a in found:
        a = (a or "/").split("#", 1)[0]
        a = a.split("?", 1)[0] if not a.startswith("/wrath/realms/report") else "/wrath/realms/report"
        out.add(a.rstrip("/") or "/" if not a.endswith("/") or a == "/" else a)
    return out


def sorted_as(addr: str) -> str | None:
    """static, server, or None (not sorted yet). Exact entries first, then prefixes ending in "/"."""
    for table, name in ((STATIC, "static"), (SERVER_ONLY, "server")):
        if addr in table:
            return name
    for table, name in ((SERVER_ONLY, "server"), (STATIC, "static")):
        for k in table:
            if k.endswith("/") and k != "/" and addr.startswith(k):
                return name
    if addr.startswith("/wrath/docs/") or addr.startswith("/wrath/art/"):
        return "static"
    return None


class Site(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix="leyline-site-test-")
        cls.out = Path(cls.tmp.name) / "site"
        r = subprocess.run([sys.executable, "-I", str(HERE / "build.py"), "--out", str(cls.out), "--stamp", STAMP, "--quiet"],
                           capture_output=True, text=True, timeout=900)
        if r.returncode != 0:
            raise AssertionError("the build failed:\n" + r.stdout + r.stderr)
        cls.files = {p.relative_to(cls.out).as_posix(): p for p in cls.out.rglob("*") if p.is_file()}
        cls.pages = {rel: p.read_text(encoding="utf-8") for rel, p in cls.files.items() if rel.endswith(".html")}

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def has(self, path: str) -> bool:
        return B.out_file(path) in self.files

    # ---------------------------------------------------------------- what launchers read
    def test_every_launcher_address_is_sorted(self):
        addrs = launcher_addresses()
        self.assertGreater(len(addrs), 30, addrs)
        loose = sorted(a for a in addrs if sorted_as(a) is None)
        self.assertEqual(loose, [], "launcher addresses not in STATIC or SERVER_ONLY (sort them, and docs/STATIC-SITE.md)")

    def test_every_read_only_launcher_address_is_a_file(self):
        for a in sorted(launcher_addresses()):
            if sorted_as(a) != "static" or a.startswith("/wrath/art/looks/look-<"):
                continue
            if "<" in a or "${" in a:
                continue
            with self.subTest(a=a):
                self.assertTrue(self.has(a), f"{a} -> {B.out_file(a)} is not in the site")
                if a.endswith(".json"):
                    json.loads(self.files[B.out_file(a)].read_bytes())

    def test_server_only_addresses_are_in_the_docs(self):
        doc = (REPO / "docs" / "STATIC-SITE.md").read_text(encoding="utf-8")
        for a in SERVER_ONLY:
            if a == "/wrath/x":
                continue
            self.assertIn(f"`{a}", doc, f"docs/STATIC-SITE.md should list {a} under what can't be static")

    def test_feeds(self):
        for f in B.FEEDS:
            rel = B.out_file(f)
            with self.subTest(f=f):
                self.assertIn(rel, self.files)
                json.loads(self.files[rel].read_bytes())
        self.assertTrue(self.has("/wrath/leyline/mods.json?sort=hot&limit=1000"))

    def test_flags_are_all_off_and_hashed(self):
        doc = json.loads(self.files["wrath/leyline/flags.json"].read_bytes())
        self.assertEqual(doc["format"], 1)
        self.assertTrue(doc["flags"])
        self.assertFalse(any(doc["flags"].values()), doc["flags"])
        self.assertEqual(doc["stun"], [])
        self.assertFalse(any(doc["limits"].values()))
        body = {k: v for k, v in doc.items() if k != "sha256"}
        canon = json.dumps(body, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        self.assertEqual(doc["sha256"], hashlib.sha256(canon.encode()).hexdigest())
        rust = (REPO / "launcher" / "src-tauri" / "src" / "p2p" / "flags.rs").read_text(encoding="utf-8")
        for name in re.findall(r'\("([a-z_]+)", "', rust.split("pub const KNOWN", 1)[1].split("];", 1)[0]):
            self.assertIn(name, doc["flags"], f"the launcher knows {name}: the file should say it is off")

    def test_realms_feed(self):
        doc = json.loads(self.files["wrath/realms.json"].read_bytes())
        self.assertIsInstance(doc["realms"], list)
        self.assertNotIn("sample", doc, "no official realm")
        self.assertFalse(any(r.get("official") for r in doc["realms"]))
        src = json.loads((REPO / "site" / "realms.json").read_text(encoding="utf-8"))
        self.assertEqual(len(doc["realms"]), len(src["realms"]))

    def test_realm_records_have_the_launcher_shape(self):
        sample = {"format": 1, "realms": [
            {"name": "Example", "realmlist": "logon.example.org", "website": "https://example.org", "game": "3.3.5a",
             "discord": "https://discord.gg/example", "type": "PvP", "core": "trinitycore", "tags": ["pvp"],
             "region": "eu", "language": "de", "notes": "Fresh start, steady rates."},
            {"name": "Other", "realmlist": "play.other.net:4724", "website": "https://other.net", "game": "1.12.1"}]}
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "realms.json"
            p.write_text(json.dumps(sample))
            doc = B.realms_doc(B.load_realms(p), STAMP)
        rid_ok = re.compile(r"^[a-z2-7]{16}$")
        host_ok = re.compile(r"^[a-z0-9-]+(\.[a-z0-9-]+)+$")
        by = {r["name"]: r for r in doc["realms"]}
        for r in doc["realms"]:   # what realms.rs parse_community_in needs to keep a record
            c = r["community"]
            self.assertRegex(c["realmId"], rid_ok)
            self.assertRegex(c["relay"], host_ok)
            self.assertTrue(1 <= r["profile"]["port"] <= 65535)
            self.assertIn(r["profile"]["build"], (5875, 8606, 12340))
            self.assertFalse(c["sample"])
            self.assertTrue(r["website"].startswith("https://"))
        self.assertEqual(by["Other"]["profile"]["port"], 4724)
        self.assertEqual(by["Other"]["realmlist"], "play.other.net:4724")
        self.assertEqual(by["Example"]["realmlist"], "logon.example.org")
        self.assertEqual(by["Example"]["profile"]["port"], 3724)
        self.assertNotEqual(by["Example"]["community"]["realmId"], by["Other"]["community"]["realmId"])

    def test_bad_inputs_stop_the_build(self):
        good = {"name": "Example", "realmlist": "logon.example.org", "website": "https://example.org", "game": "3.3.5a"}
        bad = [dict(good, name="WoW Example"), dict(good, notes="The best Warcraft realm"), dict(good, notes="Now on 3.3.5"),
               dict(good, website="http://example.org"), dict(good, website="https://patreon.com/x"),
               dict(good, realmlist="logon.example.org:99999"), dict(good, game="4.3.4"), dict(good, colour="red"),
               dict(good, realmlist="relay.leylinelauncher.com"), dict(good, tagline="Donate for perks"),
               dict(good, website="https://leylinelauncher.com/x")]
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "realms.json"
            for b in bad:
                with self.subTest(b=b):
                    p.write_text(json.dumps({"format": 1, "realms": [b]}))
                    with self.assertRaises(B.Refused):
                        B.load_realms(p)
            p.write_text(json.dumps({"format": 1, "realms": [good, good]}))
            with self.assertRaises(B.Refused):
                B.load_realms(p)
            n = Path(d) / "news.json"
            n.write_text(json.dumps({"format": 1, "posts": [{"date": "27 Nov", "title": "x"}]}))
            with self.assertRaises(B.Refused):
                B.load_news(n)
            n.write_text(json.dumps({"format": 1, "posts": [{"date": "2026-11-27", "title": "Back on WoW"}]}))
            with self.assertRaises(B.Refused):
                B.load_news(n)
            n.write_text(json.dumps({"format": 1, "posts": [{"date": "2026-11-27", "title": "Hello", "items": ["One"]}]}))
            self.assertEqual(B.load_news(n)["posts"][0]["items"], ["One"])

    def test_mods_install_from_their_authors_only(self):
        for rel in ("wrath/mods/all.json", "wrath/leyline/catalog.json"):
            doc = json.loads(self.files[rel].read_bytes())
            for p in doc.get("projects") or []:
                for u in (p.get("download"), p.get("torrent"), (p.get("install") or {}).get("url")):
                    if u:
                        host = urllib.parse.urlsplit(u).hostname or ""
                        self.assertNotIn(host, B.OLD_HOSTS, f"{rel} {p.get('id')}: {u}")
        names = {p["id"] for p in json.loads(self.files["wrath/mods/all.json"].read_bytes())["projects"]}
        self.assertTrue(any(n.startswith("up:") for n in names), "the approved upstream addons are listed")
        self.assertTrue(any(n.startswith("look:") for n in names), "the approved looks are listed")

    def test_no_pending_looks(self):
        """Only looks a person approved (stream/looks.py published())."""
        looks = json.loads((REPO / "stream" / "looks.json").read_text(encoding="utf-8"))["looks"]
        listed = {p["id"] for p in json.loads(self.files["wrath/mods/all.json"].read_bytes())["projects"] if p["id"].startswith("look:")}
        for lk in looks:
            if lk["id"] in listed:
                self.assertTrue((lk.get("review") or {}).get("by"), f"{lk['id']} is listed without a review")

    # ---------------------------------------------------------------- what players read
    def test_no_brand_words_but_the_legal_line(self):
        for rel, page in self.pages.items():
            m = BANNED.search(visible(page))
            with self.subTest(rel=rel):
                self.assertIsNone(m, f"{rel}: {m and visible(page)[max(0, m.start() - 60):m.end() + 60]!r}")
        for rel in ("wrath/realms.json", "wrath/leyline/news.json", "wrath/modes.json", "wrath/status.json"):
            self.assertIsNone(BANNED.search(self.files[rel].read_text(encoding="utf-8")), rel)

    def test_feed_texts_have_no_brand_words(self):
        doc = json.loads(self.files["wrath/mods/all.json"].read_bytes())
        for p in doc["projects"]:
            for k in ("name", "display_name", "description", "credits"):
                v = p.get(k)
                if isinstance(v, str):
                    self.assertIsNone(BANNED.search(v), f"{p['id']} {k}: {v}")

    def test_every_page_has_the_legal_line(self):
        for rel, page in self.pages.items():
            if "Moved</title>" in page:
                continue
            with self.subTest(rel=rel):
                self.assertIn(LEGAL, page)

    def test_no_links_to_closed_pages(self):
        closed = re.compile(r"/wrath/(leyline/forum|forum|leaderboards|bossfights|arena|survivors|dens|admin|profile|"
                            r"account|signin|key|play|api/|bugs|host|mobile|modes)\b|/login\b|/auth/|/account/key")
        for rel, page in self.pages.items():
            if "Moved</title>" in page:
                continue
            for attr, url in re.findall(r'\b(href|src|action)="([^"]*)"', page):
                lp = B.local_path(url)
                if lp is None:
                    if url.startswith("leyline://"):
                        self.assertNotIn("realm/official", url, rel)
                    continue
                p = B.norm(lp)
                with self.subTest(rel=rel, url=url):
                    self.assertFalse(B.dropped(p) and p not in ("/wrath/docs",), f"{rel}: {attr}={url}")
                    self.assertIsNone(closed.search(p) if not p.startswith("/wrath/docs") else None, f"{rel}: {url}")
                    if url.startswith("/"):
                        self.assertTrue(self.has(p) or p.startswith("/wrath/art/site.css"), f"{rel}: {url} is not in the site")

    def test_no_links_to_the_private_repo(self):
        for rel, f in self.files.items():
            if rel.endswith((".html", ".json")):
                self.assertNotRegex(f.read_text(encoding="utf-8"), r"github\.com/orionopenwow/(leyline-workspace|WrathUnleashed)\b", rel)

    def test_no_forms_that_post(self):
        for rel, page in self.pages.items():
            self.assertNotRegex(page, r'(?i)<form[^>]*method="post"', rel)

    def test_no_absolute_links_to_the_old_server_api(self):
        """No link (href, src, action, a feed's URL fields) to the relay, the old name or the server's API. Docs prose
        may still name an address as text."""
        dead = re.compile(r"^https?://(relay\.leylinelauncher\.com|play\.openvanilla\.app|(www\.)?openvanilla\.app|"
                          r"leylinelauncher\.com/(wrath/api|api|wrath/play|wrath/forum|wrath/leyline/forum|wrath/bugs|__))")
        for rel, page in self.pages.items():
            for attr, url in re.findall(r'\b(href|src|action|content)="([^"]*)"', page):
                with self.subTest(rel=rel, url=url):
                    self.assertIsNone(dead.search(html.unescape(url)), f"{rel}: {attr}={url}")

        def urls(v, key=""):
            if isinstance(v, dict):
                for k, x in v.items():
                    yield from urls(x, k)
            elif isinstance(v, list):
                for x in v:
                    yield from urls(x, key)
            elif isinstance(v, str) and key in ("url", "download", "page", "torrent", "repo", "author_url", "website",
                                                 "discord", "browserUrl", "formulas"):
                yield key, v
        for rel, f in self.files.items():
            if rel.endswith(".json"):
                for k, u in urls(json.loads(f.read_bytes())):
                    with self.subTest(rel=rel, u=u):
                        self.assertIsNone(dead.search(u), f"{rel}: {k}={u}")
                        if u.startswith("https://leylinelauncher.com/"):
                            self.assertTrue(self.has(urllib.parse.urlsplit(u).path), f"{rel}: {k}={u} is not in the site")

    def test_scripts_call_nothing_live(self):
        for rel, page in self.pages.items():
            for s in re.findall(r"<script\b[^>]*>(.*?)</script>", page, flags=re.S):
                for u in re.findall(r"""fetch\(\s*["'`]([^"'`]+)""", s):
                    self.assertTrue(self.has(u), f"{rel}: its script fetches {u}, which is not in the site")
                self.assertNotRegex(s, r"WebSocket|EventSource|/wrath/api/|/__", rel)

    def test_own_pages_read_honestly(self):
        front = visible(self.pages["index.html"])
        self.assertIn("Every client, every server", front)   # the portal's own front page, hero and all
        for w in ("The official realm", "Platform status", "Systems are down", "Open the forum", "signs you in"):
            self.assertNotIn(w, front)
        for w in ("Log In", "Play in your browser", "Hero Key"):
            self.assertNotIn(w, front)
        realms = visible(self.pages["wrath/leyline/realms/index.html"])
        self.assertNotIn("Wrath Unleashed", realms)
        self.assertNotIn("players online", realms)
        self.assertIn("Page not found", visible(self.pages["404.html"]))
        for rel, page in self.pages.items():
            if "Moved</title>" in page:
                continue
            v = visible(page)
            with self.subTest(rel=rel):
                self.assertNotIn("Log In", v.replace("Log in to", ""))
                self.assertNotRegex(v, r"Systems (online|offline)")

    def test_front_page_way_forward(self):
        page = self.pages["index.html"]
        front = visible(page)
        heads = ["Built so nobody can shut it down again", "Log in with no server", "Friends connect straight to each other",
                 "Characters that outlive any server", "Nobody can switch it off, including us"]
        at = [front.find(h) for h in heads]
        for h, i in zip(heads, at):
            self.assertGreaterEqual(i, 0, f"the front page lost the section {h!r}")
        self.assertEqual(at, sorted(at), "the way forward's sections are out of order")
        hero, ways, launcher = (front.find(x) for x in ("Every client, every server", "Ways to play", "Play, mod and meet up in one app"))
        self.assertTrue(0 <= hero < ways < at[0], "the hero and Ways to play come first, as before")
        self.assertLess(at[-1], launcher, "the way forward sits before the Desktop launcher")
        self.assertIn("Three clicks: Host, pick a template, Start", front)
        self.assertIn("Coming", front)   # what is still being built says so
        # every picture is in the site, a small plain SVG, with its size, alt text and lazy loading
        imgs = re.findall(r'<img\b[^>]*\bsrc="(/wrath/art/forward/[^"]+)"[^>]*>', page)
        self.assertEqual(len(imgs), 4, imgs)
        for tag in re.findall(r'<img\b[^>]*\bsrc="/wrath/art/forward/[^"]+"[^>]*>', page):
            with self.subTest(img=tag[:80]):
                for a in ('width="1280"', 'height="720"', 'loading="lazy"'):
                    self.assertIn(a, tag)
                alt = re.search(r'\balt="([^"]*)"', tag)
                self.assertTrue(alt and len(alt.group(1)) > 30, "a picture needs a real alt text")
        for src in imgs:
            with self.subTest(src=src):
                f = self.files.get(src.lstrip("/"))
                self.assertIsNotNone(f, f"{src} is not in the built site")
                b = f.read_bytes()
                self.assertTrue(b.lstrip().startswith(b"<svg"), src)
                self.assertLess(len(b), 20_000, src)
                self.assertNotRegex(b.lower(), rb"<script|href=|<text\b|foreignobject", src)
        self.assertIsNone(BANNED.search(front), "no game or maker names on the front page")
        self.assertNotRegex(front, r"\b\d+\.\d+\.\d+[a-z]?\b", "no client version numbers in the front page's words")
        news = json.loads(self.files["wrath/leyline/news.json"].read_text(encoding="utf-8"))
        self.assertIn("The way forward", [p["title"] for p in news["posts"]])

    def test_docs_pages_the_launcher_opens(self):
        docs = (REPO / "stream" / "docs.py").read_text(encoding="utf-8")
        for n in re.findall(r'^\s*\("([a-z0-9-]+)", "', docs.split("PAGES = (", 1)[1].split("\n)", 1)[0], flags=re.M):
            with self.subTest(n=n):
                self.assertTrue(self.has("/wrath/docs" if n == "index" else f"/wrath/docs/{n}"), n)
        for n in B.RETIRED_DOCS:
            self.assertTrue((REPO / "site" / "docs" / f"{n}.md").is_file(), f"site/docs/{n}.md replaces the server's page")
        hk = visible(self.pages["wrath/docs/hero-key/index.html"])
        self.assertIn("no longer work", hk)

    def test_site_files(self):
        self.assertEqual(self.files["CNAME"].read_text(), "leylinelauncher.com\n")
        self.assertIn(".nojekyll", self.files)
        self.assertIn("404.html", self.files)
        robots = self.files["robots.txt"].read_text()
        self.assertIn("Sitemap: https://leylinelauncher.com/sitemap.xml", robots)
        sm = self.files["sitemap.xml"].read_text()
        self.assertIn("<loc>https://leylinelauncher.com/</loc>", sm)
        self.assertIn("/wrath/docs/mods/", sm)
        self.assertNotIn("/wrath/leyline/forum", sm)
        self.assertNotIn("/catalog/sort-", sm, "filter pages stay out of the sitemap")
        for p in ("/", "/wrath/leyline", "/wrath/start", "/wrath/download", "/wrath/leyline/realms", "/wrath/leyline/catalog",
                  "/wrath/leyline/addons", "/wrath/looks", "/wrath/docs", "/wrath/docs/search", "/wrath/leyline/news",
                  "/wrath/realms", "/wrath/mods"):
            self.assertTrue(self.has(p), p)
        self.assertFalse(any(k.endswith("leyline-shot-backdrop.webp") for k in self.files), "not ours to publish")
        self.assertTrue(any(k.startswith("wrath/art/looks/") for k in self.files), "the Looks pictures the launcher shows")

    def test_no_game_data_and_no_secrets(self):
        game = re.compile(r"(?i)\.(mpq|m2|wmo|adt|blp|dbc|db2|skin|anim|wdt|wdl)$")
        for rel, f in self.files.items():
            self.assertIsNone(game.search(rel), rel)
            if f.stat().st_size < 2_000_000:
                b = f.read_bytes()
                self.assertNotRegex(b, rb"-----BEGIN [A-Z ]*PRIVATE KEY-----|ghp_[A-Za-z0-9]{30}|github_pat_", rel)

    def test_the_build_makes_no_network_connections(self):
        code = ("import importlib.util,sys,os,urllib.request\n"
                f"spec=importlib.util.spec_from_file_location('b', {str(HERE / 'build.py')!r})\n"
                "b=importlib.util.module_from_spec(spec); spec.loader.exec_module(b); b.no_network()\n"
                "for k in ('http_proxy','https_proxy','HTTP_PROXY','HTTPS_PROXY'): os.environ.pop(k, None)\n"
                "try:\n    urllib.request.urlopen('https://example.com/', timeout=5)\nexcept OSError as e:\n"
                "    print('refused', e)\nelse:\n    print('CONNECTED')\n")
        r = subprocess.run([sys.executable, "-I", "-c", code], capture_output=True, text=True, timeout=60)
        self.assertIn("refused", r.stdout, r.stdout + r.stderr)

    def test_same_inputs_same_bytes(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "again"
            r = subprocess.run([sys.executable, "-I", str(HERE / "build.py"), "--out", str(out), "--stamp", STAMP, "--quiet"],
                               capture_output=True, text=True, timeout=900)
            self.assertEqual(r.returncode, 0, r.stderr)
            again = {p.relative_to(out).as_posix(): p.read_bytes() for p in out.rglob("*") if p.is_file()}
        self.assertEqual(sorted(again), sorted(self.files))
        diff = [k for k in again if again[k] != self.files[k].read_bytes()]
        self.assertEqual(diff, [], "the build is not reproducible")


if __name__ == "__main__":
    unittest.main(verbosity=1)
