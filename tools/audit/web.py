"""The website as a new visitor sees it, signed out, read only (no forms sent): the main pages, every public page
reached from them (200, no server error text, no banned words, no client version in a sentence, links and pictures
that resolve), the downloads (launcher, Host pack, Den Lite: each file there and matching its checksum), the forum,
Looks, Azeroth Survivors, the Host page and the relay's join page."""
from __future__ import annotations

import hashlib
import html
import json
import re
import urllib.parse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from common import BANNED, FAIL, PASS, ROOT, SERVER_ERROR, SKIP, VERSION, is_windows

F = "Site"
# (name, path, what the page must have): the pages a new player is sent to
KEY_PAGES = [
    ("Start here", "/wrath/start", r"<h1"),
    ("Leyline home", "/wrath/leyline", r"<h1"),
    ("Looks", "/wrath/looks", r"(?i)look"),
    ("Host a server", "/wrath/host", r"(?i)<form|sign in"),
    ("Realms", "/wrath/realms", r"(?i)realm"),
    ("Game modes", "/wrath/modes", r"(?i)mode"),
    ("Mods", "/wrath/mods", r"(?i)mod"),
    ("Forum", "/wrath/leyline/forum", r"(?i)forum"),
    ("Report a bug", "/wrath/bugs", r"(?i)bug"),
    ("Azeroth Survivors", "/wrath/survivors", r"(?i)survivors"),
    ("Hero Key", "/wrath/key", r"(?i)hero key|key"),
    ("Hosting guide", "/wrath/docs/hosting", r"<h1|<h2"),
    ("Play", "/wrath", r"(?i)<html"),
]
NO_FETCH = re.compile(r"^/(?:wrath/play|__gate|logout|login|auth|api/auth|wrath/admin|wrath/survivors/play)(?:[/?]|$)"
                      r"|[?&](?:go|code|t)=")
GATED = (401, 403)
SKIP_SCHEMES = ("mailto:", "javascript:", "data:", "leyline:", "tel:", "#")


def visible_lines(page: str) -> list[str]:
    """The sentences a visitor reads: no scripts, styles, code, and no version chips (short labels)."""
    t = re.sub(r"(?is)<(script|style|code|pre|kbd|textarea|svg)\b.*?</\1>", " ", page)
    t = re.sub(r'(?is)<(span|b|small|em|i|sup)\b[^>]*class="[^"]*\b(?:chip|ver|version|badge|tag|pill)\b[^"]*"[^>]*>.*?</\1>', " ", t)
    t = re.sub(r"(?i)<br\s*/?>|</(?:p|li|h[1-6]|div|td|th|dt|dd|section|article|header|footer|figcaption|label|button|a)>", "\n", t)
    t = html.unescape(re.sub(r"<[^>]+>", " ", t))
    return [re.sub(r"\s+", " ", x).strip() for x in t.split("\n") if x.strip()]


def attrs_text(page: str) -> str:
    return " ".join(re.findall(r'(?:alt|title|aria-label|placeholder)="([^"]*)"', page))


def page_problems(page: str) -> dict:
    lines = visible_lines(page)
    text = " ".join(lines) + " " + attrs_text(page)
    title = " ".join(re.findall(r"(?is)<title>(.*?)</title>", page))
    out = {"error": [], "banned": [], "version": []}
    m = SERVER_ERROR.search(page)
    if m:
        out["error"].append(m.group(0))
    for m in BANNED.finditer(text + " " + title):
        out["banned"].append(text[max(0, m.start() - 30):m.end() + 30] if m.start() < len(text) else title[:80])
    for ln in lines:
        if len(ln.split()) >= 5 and VERSION.search(ln):
            out["version"].append(ln[:120])
    return out


def links_of(page: str, base_url: str) -> tuple[set, set]:
    """(links, pictures) as absolute URLs."""
    hrefs, srcs = set(), set()
    for kind, v in re.findall(r'(?i)\b(href|src)="([^"]+)"', page):
        v = html.unescape(v).strip()
        if not v or v.startswith(SKIP_SCHEMES):
            continue
        u = urllib.parse.urljoin(base_url, v).split("#")[0]
        if not u.startswith(("http://", "https://")):
            continue
        (srcs if kind.lower() == "src" else hrefs).add(u)
    return hrefs, srcs


def same_site(u: str, base: str) -> bool:
    return urllib.parse.urlsplit(u).netloc == urllib.parse.urlsplit(base).netloc


def landed_on_sign_in(r) -> bool:
    return bool(r.hops) and re.search(r"(?i)/(login|sign-?in|account|auth|key)(?:[/?]|$)", r.url or "") is not None


# ---------------------------------------------------------------------- the pages
def check_pages(ctx, rec):
    base = ctx.base("site")
    new, old = ctx.names("site")
    if "site" not in ctx.fake:
        host = ctx.host("site")
        if host == new:
            rec.add(F, "The new address answers", PASS, new)
        else:
            rec.add(F, "The new address answers", SKIP, f"{new} doesn't answer yet; checked {old} (the domain move isn't on)")
    seen: dict[str, object] = {}
    problems = {"error": [], "banned": [], "version": []}

    def fetch(u):
        return u, ctx.get(u, timeout=25, max_bytes=3 << 20)

    # the key pages, one line each
    key = [(n, base + path, rx) for n, path, rx in KEY_PAGES]
    with ThreadPoolExecutor(8) as pool:
        for (name, url, rx), (_, r) in zip(key, pool.map(lambda k: fetch(k[1]), key)):
            seen[url] = r
            if r.status == 200 and re.search(rx, r.text):
                pr = page_problems(r.text)
                bad = [f"{k}: {v[0]}" for k, v in pr.items() if v]
                rec.add(F, f"{name} page", PASS if not bad else FAIL, f"{urllib.parse.urlsplit(url).path}" +
                        (f"; {bad[0][:120]}" if bad else ""), "Tell Claude this line (the signin step ships the fix).")
            elif r.status == 200 or landed_on_sign_in(r) or r.status in GATED:
                rec.add(F, f"{name} page", PASS if r.status != 200 else FAIL,
                        f"{urllib.parse.urlsplit(url).path}: " + ("asks to sign in" if r.status != 200 else "the page is missing its content"),
                        "Tell Claude this line.")
            else:
                rec.add(F, f"{name} page", FAIL, f"{urllib.parse.urlsplit(url).path}: HTTP {r.status or r.error}",
                        "Run the signin step (the site's pages), then tell Claude if it stays.")

    # every public page reached from them (one hop, then one more), each checked once
    pages, pics, outside = {}, set(), set()
    frontier = [u for u, r in seen.items() if r.status == 200 and "html" in r.headers.get("content-type", "")]
    for u in frontier:
        pages[u] = seen[u]
    limit = int(ctx.cfg.get("max_pages", 220))
    for _depth in range(2):
        nxt = set()
        for u in frontier:
            hrefs, srcs = links_of(seen[u].text, u)
            pics |= {s for s in srcs if same_site(s, base)}
            for h in hrefs:
                if same_site(h, base):
                    path = urllib.parse.urlsplit(h).path + ("?" + urllib.parse.urlsplit(h).query if urllib.parse.urlsplit(h).query else "")
                    if h not in seen and not NO_FETCH.search(path):
                        nxt.add(h)
                elif "/releases/download/" in h:
                    outside.add(h)
        nxt = sorted(nxt)[:max(0, limit - len(seen))]
        with ThreadPoolExecutor(8) as pool:
            for u, r in pool.map(fetch, nxt):
                seen[u] = r
        frontier = [u for u in nxt if seen[u].status == 200 and "html" in seen[u].headers.get("content-type", "")]
        for u in frontier:
            pages[u] = seen[u]

    broken = [(u, r.status or r.error) for u, r in seen.items()
              if not (r.status == 200 or r.status in GATED or landed_on_sign_in(r))]
    rec.add(F, "Every public page answers", PASS if not broken else FAIL,
            f"{len(seen)} addresses" + (": " + ", ".join(f"{urllib.parse.urlsplit(u).path} {c}" for u, c in broken[:4]) if broken else ""),
            "Tell Claude these addresses (a broken link or page).")
    for u, r in pages.items():
        for k, v in page_problems(r.text).items():
            problems[k] += [(urllib.parse.urlsplit(u).path, x) for x in v]
    rows = (("error", "No server error text on any page", lambda p: True),
            ("banned", "No banned words on any page", lambda p: True),
            ("version", "No client version numbers in sentences (main pages)", lambda p: not p.startswith("/wrath/docs")),
            ("version", "No client version numbers in sentences (the guides, /wrath/docs)", lambda p: p.startswith("/wrath/docs")))
    for k, name, keep in rows:
        if not pages:
            rec.add(F, name, SKIP, "no page opened")
            continue
        v = [(p, x) for p, x in problems[k] if keep(p)]
        n = len({p for p in pages if keep(urllib.parse.urlsplit(p).path)}) if k == "version" else len(pages)
        rec.add(F, name, PASS if not v else FAIL, f"{n} pages" + (f", {len(v)} lines: " + "; ".join(f"{p} \"{x[:70]}\"" for p, x in v[:3]) if v else ""),
                "Tell Claude these lines (the words are fixed in the site's code; the full list is in logs/audit-latest.txt).")
    ctx.cache["appendix"] = [f"{p}: {x}" for k in ("error", "banned", "version") for p, x in problems[k]]

    with ThreadPoolExecutor(8) as pool:
        res = list(pool.map(lambda u: (u, ctx.get(u, timeout=20, max_bytes=1 << 16)), sorted(pics)[:150]))
    bad = [(u, r.status or r.error) for u, r in res if r.status != 200]
    rec.add(F, "Pictures load", PASS if not bad else FAIL, f"{len(res)} pictures" +
            (": " + ", ".join(f"{urllib.parse.urlsplit(u).path} {c}" for u, c in bad[:4]) if bad else ""),
            "Tell Claude these pictures (missing art: the signin or homeart step).")
    with ThreadPoolExecutor(4) as pool:
        res = list(pool.map(lambda u: (u, ctx.get(u, timeout=30, max_bytes=1 << 14)), sorted(outside)[:20]))
    bad = [(u, r.status or r.error) for u, r in res if r.status != 200]
    if res:
        rec.add("Downloads", "Download buttons on the site lead to real files", PASS if not bad else FAIL,
                f"{len(res)} release files" + (": " + ", ".join(f"{u.rsplit('/', 2)[-2]}/{u.rsplit('/', 1)[-1]} {c}" for u, c in bad[:3]) if bad else ""),
                "Tell Claude which download is missing (the hostkit, addons or launcher release).")
    ctx.cache["pages"] = pages


# ---------------------------------------------------------------------- downloads
def sha_of(ctx, url, dest: Path | None = None, timeout=300):
    h, f = hashlib.sha256(), open(dest, "wb") if dest else None
    size = [0]

    def sink(chunk):
        h.update(chunk)
        size[0] += len(chunk)
        if f:
            f.write(chunk)
    try:
        r = ctx.get(url, timeout=timeout, sink=sink)
    finally:
        if f:
            f.close()
    return r, h.hexdigest(), size[0]


def check_downloads(ctx, rec):
    D = "Downloads"
    temp = Path(ctx.temp)
    # the launcher: the rolling release, its checksum from GitHub
    rel = ctx.cfg["releases"]
    api = ctx.get(rel["launcher_api"], timeout=20, headers={"Accept": "application/vnd.github+json"})
    info = api.json() if api.status == 200 else None
    asset = next((a for a in (info or {}).get("assets", []) if a.get("name") == "Leyline.exe"), None)
    m = re.search(r"\(latest, ([^)]+)\)", (info or {}).get("name", ""))
    ctx.cache["launcher_version"] = m.group(1) if m else ""
    r, got, size = sha_of(ctx, rel["launcher_exe"], temp / "Leyline-latest.exe")
    want = (asset or {}).get("digest", "").replace("sha256:", "")
    if r.status != 200:
        rec.add(D, "Launcher (Leyline.exe)", FAIL, f"HTTP {r.status or r.error}", "Run the Ship the Leyline launcher step.")
    elif want and got != want:
        rec.add(D, "Launcher (Leyline.exe)", FAIL, f"sha256 {got[:12]} is not GitHub's {want[:12]}", "Run the Ship the Leyline launcher step.")
    else:
        ctx.cache["launcher_sha"] = got
        rec.add(D, "Launcher (Leyline.exe)", PASS, f"{size // (1 << 20)} MB, version {ctx.cache['launcher_version'] or '?'}"
                + (", sha256 matches GitHub's" if want else ", GitHub gives no checksum"))

    # the Host pack: the site's descriptor, the file, its checksum
    src = "the site's"
    if site_down(ctx):   # the copy in this workspace (stream/hostpack.json, what the signin step deploys)
        src = "the workspace's (the site didn't answer)"
        try:
            hp = json.loads((ROOT / "stream" / "hostpack.json").read_text(encoding="utf-8"))
        except (OSError, ValueError):
            hp = None
    else:
        hp = ctx.get(ctx.url("site", "/wrath/leyline/hostpack.json"), timeout=20).json()
    if not isinstance(hp, dict) or not hp.get("download"):
        rec.add(D, "Host pack", FAIL, "the site's /wrath/leyline/hostpack.json is missing", "Run the signin step.")
    else:
        dest = temp / "host-pack.zip"
        r, got, size = sha_of(ctx, hp["download"], dest)
        if r.status != 200:
            rec.add(D, "Host pack", FAIL, f"{hp['download'].rsplit('/', 1)[-1]}: HTTP {r.status or r.error}", "Run the hostkit step.")
        elif got != hp.get("sha256"):
            rec.add(D, "Host pack", FAIL, f"sha256 {got[:12]} is not the {str(hp.get('sha256'))[:12]} of {src} descriptor",
                    "Run the hostkit step, then the signin step.")
        else:
            side = ctx.get(hp["download"] + ".sha256", timeout=20)
            extra = ""
            if side.status == 200:
                extra = ", its .sha256 file matches" if side.text.split()[:1] == [got] else ", its .sha256 file DIFFERS"
            ok = "DIFFERS" not in extra
            ctx.cache["hostpack_zip"] = str(dest)
            rec.add(D, "Host pack", PASS if ok else FAIL, f"version {hp.get('version')}, {size // 1024} KB, matches {src} sha256{extra}",
                    "Run the hostkit step.")

    # Den Lite: both release files and their .sha256
    for name in (rel["den_lite_windows"], rel["den_lite_linux"]):
        sha = ctx.get(rel["den_lite"] + name + ".sha256", timeout=20)
        if sha.status != 200:
            rec.add(D, f"Den Lite ({name})", FAIL, f".sha256: HTTP {sha.status or sha.error}",
                    "Ask Claude to run the Den Lite release build (den-lite.yml).")
            continue
        want = (sha.text.split() or [""])[0].lower()
        mine = name == (rel["den_lite_windows"] if is_windows() else rel["den_lite_linux"])
        dest = temp / name if mine else None
        r, got, size = sha_of(ctx, rel["den_lite"] + name, dest)
        if r.status == 200 and got == want:
            if mine:
                import os
                if not is_windows():
                    os.chmod(dest, 0o755)
                ctx.cache["den_lite_download"] = str(dest)
            rec.add(D, f"Den Lite ({name})", PASS, f"{size // 1024} KB, sha256 matches its .sha256")
        else:
            rec.add(D, f"Den Lite ({name})", FAIL, f"HTTP {r.status or r.error}" if r.status != 200 else
                    f"sha256 {got[:12]} is not {want[:12]}", "Ask Claude to run the Den Lite release build again.")


# ---------------------------------------------------------------------- forum, Looks, Survivors, hosting
def check_forum(ctx, rec):
    G = "Forum"
    cats = ctx.get(ctx.url("site", "/wrath/forum/api/categories"), timeout=20).json()
    if isinstance(cats, dict) and isinstance(cats.get("categories"), list) and cats["categories"]:
        rec.add(G, "Categories load", PASS, ", ".join(c.get("name", "?") for c in cats["categories"][:6]))
    else:
        rec.add(G, "Categories load", FAIL, "no categories from /wrath/forum/api/categories", "Run the signin step.")
    lat = ctx.get(ctx.url("site", "/wrath/forum/api/latest?n=5"), timeout=20).json()
    if isinstance(lat, dict) and isinstance(lat.get("threads"), list):
        th = lat["threads"]
        rec.add(G, "Latest threads load", PASS, f"{len(th)} threads" + (f", newest: {th[0].get('title', '')[:50]}" if th else " (none yet)"))
        if th and th[0].get("id"):
            r = ctx.get(ctx.url("site", f"/wrath/leyline/forum/t/{th[0]['id']}"), timeout=20)
            rec.add(G, "A thread opens", PASS if r.status == 200 else FAIL, f"HTTP {r.status or r.error}", "Tell Claude this line.")
    else:
        rec.add(G, "Latest threads load", FAIL, "no thread list from /wrath/forum/api/latest", "Run the signin step.")


def launcher_presets() -> set:
    try:
        src = (ROOT / "launcher" / "src-tauri" / "src" / "gfx.rs").read_text(encoding="utf-8")
    except OSError:
        return set()
    block = src.split("pub const PRESETS", 1)[-1].split("];", 1)[0]
    return set(re.findall(r'\(\s*"([a-z0-9-]+)",\s*"[^"]+",\s*&\[', block))


def check_catalog(ctx, rec):
    G = "Looks"
    r = ctx.get(ctx.url("site", "/wrath/mods/all.json"), timeout=30, max_bytes=8 << 20)
    data = r.json()
    items = (data or {}).get("projects") or (data or {}).get("mods") or (data if isinstance(data, list) else [])
    if r.status != 200 or not isinstance(items, list) or not items:
        rec.add("Catalog", "The one catalog (all.json) loads", FAIL, f"HTTP {r.status or r.error}", "Run the signin step.")
        return
    kinds = {}
    for x in items:
        k = x.get("source") or x.get("kind") or "?"
        kinds[k] = kinds.get(k, 0) + 1
    hostpack = any(x.get("kind") == "hostpack" for x in items)
    rec.add("Catalog", "The one catalog (all.json) loads", PASS if hostpack else FAIL,
            f"{len(items)} entries: " + ", ".join(f"{k} {n}" for k, n in sorted(kinds.items())) + ("" if hostpack else "; no Host pack in it"),
            "Run the signin step.")
    looks = [x for x in items if x.get("source") == "look"]
    presets = {x["look"].get("preset") for x in looks if isinstance(x.get("look"), dict) and x["look"].get("type") == "preset"}
    rec.add(G, "Looks in the catalog", PASS if looks else FAIL, f"{len(looks)} looks, {len(presets)} graphics presets",
            "Run the signin step (looks.json goes with it).")
    known = launcher_presets()
    if not known:
        rec.add(G, "The launcher knows every preset in the catalog", SKIP, "no launcher source in this folder")
    else:
        missing = sorted(p for p in presets if p and p not in known)
        rec.add(G, "The launcher knows every preset in the catalog", PASS if not missing else FAIL,
                f"catalog {sorted(presets)}; launcher {sorted(known)}" + (f"; unknown: {missing}" if missing else ""),
                "Tell Claude: a Looks preset the launcher doesn't have.")


def check_survivors(ctx, rec):
    G = "Azeroth Survivors"
    r = ctx.get(ctx.url("site", "/wrath/survivors"), timeout=20)
    if r.status != 200:
        rec.add(G, "The page", FAIL, f"HTTP {r.status or r.error}", "Run the signin step.")
        return
    t = r.text
    if 'href="/wrath/survivors/play"' in t:
        state = "live"
    elif "Resting" in t:
        state = "resting"
    elif "Coming soon" in t:
        state = "coming"
    else:
        state = "unknown"
    rec.add(G, "The page", PASS if state != "unknown" else FAIL, f"state: {state}", "Tell Claude this line.")
    if state != "live":
        rec.add(G, "The game's own address", SKIP, f"the game is {state}: nothing to reach yet")
        return
    m = re.search(r'href="(https://[^"/]+/)licenses/"', t)
    addr = m.group(1) if m else None
    if not addr:
        rec.add(G, "The game's own address", FAIL, "the live page names no address (no Licences link)", "Tell Claude this line.")
        return
    home = ctx.get(addr, timeout=20, follow=0)
    rec.add(G, "The game is behind sign-in", PASS if home.status in (302, 303, 401, 403) else FAIL,
            f"{addr} answers {home.status or home.error} signed out", "Tell Claude: Survivors answers without sign-in.")
    lic = ctx.get(addr + "licenses/", timeout=20)
    rec.add(G, "Its licences page is public", PASS if lic.status == 200 else FAIL, f"HTTP {lic.status or lic.error}",
            "Run Put Survivors on Leyline.bat noupload, then tell Claude.")
    data = ctx.get(addr + "data/__index", timeout=20, follow=0)
    rec.add(G, "Its game files need sign-in", PASS if data.status in (302, 303, 401, 403) else FAIL,
            f"/data/__index answers {data.status or data.error} signed out", "Tell Claude: Survivors' files are open to anyone.")


def check_hosting(ctx, rec, wu_listing):
    G = "Hosting"
    if wu_listing and wu_listing.get("id"):
        r = ctx.get(ctx.url("relay", f"/join/{wu_listing['id']}"), timeout=20)
        rec.add(G, "The relay's join page answers", PASS if r.status == 200 and "<html" in r.text.lower() else FAIL,
                f"/join/{wu_listing['id'][:8]}...: HTTP {r.status or r.error}", "Run the relay step.")
    else:
        rec.add(G, "The relay's join page answers", SKIP, "no realm on the network list to open it for")
    if site_down(ctx):
        rec.add(G, "The Host page loads", SKIP, "the website didn't answer")
    else:
        r = ctx.get(ctx.url("site", "/wrath/host"), timeout=20)
        gated = landed_on_sign_in(r) or r.status in GATED
        if r.status == 200 and "<form" in r.text:
            rec.add(G, "The Host page loads", PASS, "its form is there")
        elif gated or r.status == 200:
            rec.add(G, "The Host page loads", PASS, "signed out: it asks to sign in first (the form itself: manual item 8)")
        else:
            rec.add(G, "The Host page loads", FAIL, f"HTTP {r.status or r.error}", "Run the signin step.")
    zp = ctx.cache.get("hostpack_zip")
    if not zp:
        rec.add(G, "Host pack: its self-check (dry run)", SKIP, "the Host pack didn't download")
        return
    import zipfile
    try:
        with zipfile.ZipFile(zp) as z:
            names = z.namelist()
            bad = z.testzip()
            py = [n for n in names if n.endswith(".py")]
            broken = []
            for n in py:
                try:
                    compile(z.read(n), n, "exec")
                except SyntaxError as e:
                    broken.append(f"{n}: {e.msg}")
    except (OSError, zipfile.BadZipFile) as e:
        rec.add(G, "Host pack: its self-check (dry run)", FAIL, f"the zip doesn't open: {e}", "Run the hostkit step.")
        return
    selfcheck = [n for n in names if n.endswith("wuhost/selfcheck.py")]
    starters = [n for n in names if n.lower().endswith(".bat") and "host" in n.lower()]
    ok = not bad and not broken and selfcheck and starters
    rec.add(G, "Host pack: its self-check (dry run)", PASS if ok else FAIL,
            f"{len(names)} files, {len(py)} Python files compile, self-check {'there' if selfcheck else 'MISSING'}, "
            f"{len(starters)} start files; a real run needs a running realm (manual list)"
            + (f"; {broken[0]}" if broken else "") + (f"; damaged: {bad}" if bad else ""), "Run the hostkit step.")


def site_down(ctx) -> str:
    """"" when the website answers from this PC, else why not (asked once; the other checks then skip)."""
    if "site_down" not in ctx.cache:
        r = ctx.get(ctx.url("site", "/wrath/status.json"), timeout=20, max_bytes=1 << 16)
        ctx.cache["site_down"] = "" if r.status else (r.error or "no answer")
    return ctx.cache["site_down"]


def run(ctx, rec):
    why = site_down(ctx)
    if why:
        rec.add(F, "The website answers", FAIL, f"{ctx.base('site')}: {why}",
                "Check this PC's internet; if other sites open, run the check step (the website may be down).")
        for feat, name in (("Site", "The pages"), ("Catalog", "The one catalog"), ("Forum", "The forum"),
                           ("Looks", "Looks"), ("Azeroth Survivors", "The page")):
            rec.add(feat, name, SKIP, "the website didn't answer")
        return
    rec.add(F, "The website answers", PASS, ctx.base("site"))
    check_pages(ctx, rec)
    check_catalog(ctx, rec)
    check_forum(ctx, rec)
    check_survivors(ctx, rec)


def run_downloads(ctx, rec):
    check_downloads(ctx, rec)
