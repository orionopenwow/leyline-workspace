#!/usr/bin/env python3
"""The commons kit: make your own Leyline, and keep the directory of what Leyline is made of (docs/BLANK-SLATE.md).

    python3 -I tools/commons/commons.py make [--answers FILE.json] [--out DIR] [--yes]
    python3 -I tools/commons/commons.py preview OUT/site [--port 8091]
    python3 -I tools/commons/commons.py check-directory
    python3 -I tools/commons/commons.py wiki [--out FILE]
    python3 -I tools/commons/commons.py paths

make     asks a few questions (name, colours, logo, domain or none, Discord link), writes one brand.json and packs:
           site/          the website and its Log in page, with the brand file, ready to upload to a server
           browser-kit/   the game page around the browser client, in the brand's look (own game files only)
           host-kit/      the host kit folder and zip (realms from the host's own client)
           launcher/      a branding.json for launcher/scripts/rebrand.py
           UPLOAD.md      the few steps to put it online
         Every packed file passes the public export's own checks (no game data, no secrets, no logs, no decompiled
         code, no game or maker names in what people read). Leyline's own files are never changed.
preview  runs a packed site on this PC only (127.0.0.1), to look at it before uploading.
check-directory   checks tools/commons/directory.json (every path exists, no brand words, no money links).
wiki     prints the directory as the forum's pinned post (plain text, cut into posts the forum accepts).
paths    prints where the one-click wrappers find the host kit (used by their test).

Standard library only. Exit codes: 0 ok, 1 refused or failed, 2 usage.
"""
from __future__ import annotations

import argparse
import importlib.util
import io
import json
import os
import re
import sys
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
DIRECTORY = HERE / "directory.json"
LEGAL = re.compile(r"not affiliated with or endorsed by Blizzard Entertainment\.?", re.I)
FORUM_BODY_MAX = 5000          # stream/forum.py BODY_MAX
LOGO_MAX = 1_000_000


class Refused(Exception):
    pass


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def brandfile():
    return sys.modules.get("commons_brandfile") or _load("commons_brandfile", REPO / "stream" / "brandfile.py")


def export():
    return sys.modules.get("export") or _load("export", REPO / "tools" / "leyline-repo" / "export.py")


def hostkit():
    export()
    return sys.modules.get("commons_hostkit") or _load("commons_hostkit", REPO / "tools" / "leyline-repo" / "hostkit.py")


def rebrand():
    return sys.modules.get("commons_rebrand") or _load("commons_rebrand", REPO / "launcher" / "scripts" / "rebrand.py")


# ------------------------------------------------------------------------------------------------ the questions
QUESTIONS = [
    ("name", "Your network's name (1-40 characters)", "Leyline"),
    ("accent", "Main colour, #rrggbb (buttons, links)", "#5b8def"),
    ("bg", "Background colour, #rrggbb", "#12151b"),
    ("text", "Text colour, #rrggbb", "#eef0f4"),
    ("logo", "Logo file, .png or .webp, square, up to 1 MB (empty: the Leyline mark)", ""),
    ("domain", "Your domain, like play.example.org (empty: none yet)", ""),
    ("discord", "Discord invite, like https://discord.gg/abc (empty: none)", ""),
]
DOMAIN = re.compile(r"^(?=.{4,253}$)([a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}$")


def ask(answers: dict | None, yes: bool, inp=input, out=print) -> dict:
    """The answers: from the file, then the prompts (--yes: defaults for anything missing)."""
    got = dict(answers or {})
    for key, label, default in QUESTIONS:
        if key in got:
            continue
        if yes:
            got[key] = default
            continue
        v = inp(f"{label} [{default}]: ").strip()
        got[key] = v or default
    return {k: str(v).strip() for k, v in got.items()}


def logo_kind(data: bytes) -> str | None:
    if data[:8] == b"\x89PNG\r\n\x1a\n":
        return "png"
    if data[:4] == b"RIFF" and data[8:12] == b"WEBP":
        return "webp"
    return None


def stronger(hex6: str) -> str:
    """The hover colour: the accent a little lighter (brandfile.mix towards white)."""
    return brandfile().mix(hex6.lower(), "#ffffff", 0.18)


def build_brand(a: dict) -> tuple[dict, bytes | None]:
    """(brand.json, logo bytes or None) from the answers; Refused with every reason when something is wrong."""
    why = []
    logo_bytes, logo = None, ""
    if a.get("logo"):
        p = Path(a["logo"]).expanduser()
        try:
            logo_bytes = p.read_bytes()
        except OSError:
            why.append(f"logo: can't read {p}")
        else:
            kind = logo_kind(logo_bytes)
            if not kind:
                why.append("logo: must be a .png or .webp picture")
            elif len(logo_bytes) > LOGO_MAX:
                why.append("logo: at most 1 MB")
            else:
                logo = f"brand-logo.{kind}"
    domain = a.get("domain", "").lower().removeprefix("https://").removeprefix("http://").strip("/")
    if domain and not DOMAIN.match(domain):
        why.append("domain: like play.example.org (no https://, no path)")
    accent = a.get("accent", "")
    brand = {
        "format": 1,
        "name": a.get("name", ""),
        "colours": {"bg": a.get("bg", ""), "text": a.get("text", ""), "accent": accent,
                    "accent_strong": stronger(accent) if re.match(r"^#[0-9a-fA-F]{6}$", accent) else ""},
        "logo": logo,
        "links": {"site": f"https://{domain}" if domain else "", "discord": a.get("discord", ""), "reddit": a.get("reddit", "")},
    }
    why += brandfile().problems(brand)
    if why:
        raise Refused("; ".join(why))
    return brand, (logo_bytes if logo else None)


def slug(name: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
    return s or "my-network"


# ------------------------------------------------------------------------------------------------ the packs
SITE_SKIP_TOP = {"tests", "server", "slices", "lua", "sql", "fui-web", "www-files", "__pycache__"}


def private_art() -> set:
    return set(getattr(export(), "PRIVATE_ART", set()))


def art_files() -> dict[str, bytes]:
    art = REPO / "stream" / "www-files" / "art"
    out = {}
    for p in sorted(art.rglob("*")):
        if p.is_file() and p.name not in private_art() and not export().JUNK.search(p.name):
            out[p.relative_to(art).as_posix()] = p.read_bytes()
    return out


def pack_site(brand: dict, logo: bytes | None, domain: str) -> dict[str, bytes]:
    """{path under site/: bytes}: the portal and its modules, the public guides, the art, the brand file."""
    stream = REPO / "stream"
    files = {}
    for p in sorted(stream.iterdir()):
        if p.is_file() and p.suffix in (".py", ".json"):
            files[p.name] = p.read_bytes()
    for sub in ("hub", "survivors"):
        for p in sorted((stream / sub).rglob("*")):
            if p.is_file() and "__pycache__" not in p.parts:
                files[f"{sub}/{p.relative_to(stream / sub).as_posix()}"] = p.read_bytes()
    docs = REPO / "docs" / "leyline"
    for p in sorted(docs.rglob("*")):
        if p.is_file():
            files[f"docs/leyline/{p.relative_to(docs).as_posix()}"] = p.read_bytes()
    for rel, data in art_files().items():
        files[f"art/{rel}"] = data
    if logo:
        files[f"art/{brand['logo']}"] = logo
    files["brand.json"] = (json.dumps(brand, indent=2) + "\n").encode()
    files["gamedata-off"] = b"This file keeps the game data switch on: this site never serves game files.\n"
    files["portal.env.example"] = site_env(brand, domain).encode()
    files["run-site.sh"] = RUN_SITE.encode()
    files["Caddyfile.example"] = caddyfile(domain).encode()
    files["README.txt"] = SITE_README.replace("{name}", brand["name"]).encode()
    return files


def site_env(brand: dict, domain: str) -> str:
    host = domain or "localhost"
    return f"""# {brand['name']}: the website's settings (copied to portal.env by run-site.sh the first time; edit that copy).
# Paths use $HERE, the site folder (run-site.sh sets it). No passwords or keys live in this file.
WU_BRAND_FILE=$HERE/brand.json
LEYLINE_BRAND=1
LEYLINE_DOMAIN_MOVE=1
PUBLIC_HOST={host}
PAGE_PORT=8091
BRIDGE_PORT=7878
WU_ART_DIR=$HERE/art
WU_DOCS_DIR=$HERE/docs/leyline
WU_PORTAL_DATA=$HERE/data
WU_LEYLINE_DIR=$HERE/data/leyline
WU_MODS_DIR=$HERE/data/mods
WU_ARMORY_DIR=$HERE/data/armory
WU_WORKSHOP_ITEMS_DIR=$HERE/data/workshop
WU_DUELS_DIR=$HERE/data/duels
WU_HOST_DIR=$HERE/data/hosted
WU_SLICE_DIR=$HERE/data/slices
# Never any game data: the switch is on (the file exists) and streaming is off. Players use their own game folder.
WU_GAMEDATA_FLAG=$HERE/gamedata-off
WU_STREAM=0
WU_OWNFILES=1
# Your realm's game console (only if you run a realm next to the site; otherwise leave these: the site shows it offline).
AC_SOAP_URL=http://127.0.0.1:9/
SOAP_USER=none
SOAP_PASS=none
# Log in: the sign-in service's address. It is a separate program that is not in this kit yet (docs/BLANK-SLATE.md).
PORTAL_URL=http://127.0.0.1:9
# The realm list: the network's relay listing (Leyline's, until you run your own relay).
WU_RELAY_LISTING=https://relay.leylinelauncher.com/api/listing/realms.json
WU_JOIN_BASE=https://relay.leylinelauncher.com
"""


RUN_SITE = """#!/usr/bin/env bash
# Start the website from this folder. Needs Python 3 only. Stop it with Ctrl+C.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export HERE
[ -f "$HERE/portal.env" ] || cp "$HERE/portal.env.example" "$HERE/portal.env"
mkdir -p "$HERE/data"
set -a; . "$HERE/portal.env"; set +a
exec python3 -I "$HERE/wrath_portal.py"
"""


def caddyfile(domain: str) -> str:
    host = domain or "play.example.org"
    return f"""# Caddy in front of the website: real TLS for your domain, then the site on this machine.
# Put your domain's A record at this server's address first. Then: caddy run --config Caddyfile
{host} {{
\tredir / /wrath
\treverse_proxy 127.0.0.1:8091
}}
"""


SITE_README = """{name}: the website and its Log in page
=========================================

What is here: the website program (wrath_portal.py and its modules), the public guides (docs/leyline), the art,
and brand.json (your name, colours, logo and links). It never serves game files: gamedata-off keeps that switch on.

Start it on any Linux server (or WSL):
  bash run-site.sh                 the first run copies portal.env.example to portal.env
Then put Caddy in front of it for your domain (Caddyfile.example). Full steps: UPLOAD.md, one folder up.

What works without anything else: the front page, realm list (from the network's relay), guides, mods lists,
the forum's pages, Your profile pages. What needs more:
  * Log in needs the sign-in service, a separate program not in this kit yet. Until then the Log in page says
    it isn't answering.
  * The official realm's status needs your own realm's console (AC_SOAP_URL, SOAP_USER, SOAP_PASS).
"""


BROWSER_SKIP = {"web.patch"}


def browser_css(brand: dict) -> str:
    bf = brandfile()
    t = ";".join(f"{k}:{v}" for k, v in bf.tokens(brand).items())
    css = f"/* {brand['name']}: the game page's look (brand.json). Loaded after leyline-look.css. */\n"
    css += f":root,html.ly-brand{{{t}}}\n"
    if brand.get("logo"):
        css += ("html.ly-brand #wenilla-boot .box::before,html.ly-brand #wu-loading .box::before,html.ly-brand #wu-layer-cover::before,"
                ".ly-spin::before{display:none!important;animation:none!important}\n"
                "html.ly-brand #wenilla-boot .box::after,html.ly-brand #wu-loading .box::after,html.ly-brand #wu-layer-cover::after,"
                f".ly-spin::after{{background-image:url(\"art/{brand['logo']}\")!important}}\n")
    return css


def with_brand_css(page: str, name: str) -> str:
    page = re.sub(r"<title>[^<]*</title>", f"<title>{name}</title>", page, count=1)
    link = '<link rel="stylesheet" href="brand.css">'
    if link in page:
        return page
    if "</head>" in page:
        return page.replace("</head>", link + "</head>", 1)
    return re.sub(r"(</title>)", r"\1" + link, page, count=1)


def pack_browser(brand: dict, logo: bytes | None) -> dict[str, bytes]:
    """{path under browser-kit/: bytes}: the page layer (our own code), its art, brand.css. No slices, no client."""
    www = REPO / "stream" / "www-files"
    files = {}
    for p in sorted(www.iterdir()):
        if p.is_file() and p.name not in BROWSER_SKIP:
            data = p.read_bytes()
            if p.suffix == ".html":
                data = with_brand_css(data.decode("utf-8"), brand["name"]).encode()
            files[p.name] = data
    for rel, data in art_files().items():
        files[f"art/{rel}"] = data
    if logo:
        files[f"art/{brand['logo']}"] = logo
    files["brand.css"] = browser_css(brand).encode()
    files["README.txt"] = BROWSER_README.replace("{name}", brand["name"]).encode()
    return files


BROWSER_README = """{name}: the browser kit
========================

What is here: the game page around the browser client (our own code: loading screens, the own-files reader, touch
controls, addons), its art, and brand.css with your colours and logo. index.html and tab.html load brand.css.

Own game files only. The page reads the player's own game folder in their browser tab and never uploads it. This kit
has no game files and no "slices", and it must never be given any: do not serve a /data folder from your server.

What is NOT here yet: the browser client itself (wenilla.js and its .wasm) and the small server that serves it. They
are built from a separate source that is not in this kit. Until that is published, this folder reskins the page,
but cannot run a game on its own. See docs/BLANK-SLATE.md, "browser kit".
"""


def pack_host(brand: dict) -> tuple[dict[str, bytes], list[str]]:
    hk = hostkit()
    kit = REPO / "hosting" / "kit"
    files = hk.collect(kit, hk.find_realm(kit))
    for w in WRAPPERS:                         # one thing to click, inside the kit itself
        files[w] = hk.text_fix(w, (HERE / w).read_bytes())
    files["MADE-BY.txt"] = (f"This host kit was packed for {brand['name']} by tools/commons (Make my own Leyline).\n"
                            "It is the network's own kit, unchanged: it still names Leyline in its windows and uses\n"
                            "the network's relay. Players bring their own game client; the kit never downloads one.\n").encode()
    return files, hk.problems(files)


def launcher_branding(brand: dict, domain: str) -> dict:
    b = json.loads((REPO / "launcher" / "branding.json").read_text(encoding="utf-8"))
    b["name"] = brand["name"]
    alnum = re.sub(r"[^a-z0-9]", "", brand["name"].lower()) or "network"
    b["identifier"] = f"community.{alnum}.launcher"
    b["colours"] = dict(brand["colours"])
    if domain:
        b["site"] = f"https://{domain}"
        b["flags_url"] = f"https://{domain}/wrath/leyline/flags.json"
    b["_commons"] = ("Made by tools/commons. Before you build: your own repo and update sources (repo, updates), your own "
                     "updater key, and your own relays, or your launcher updates itself back to Leyline. docs/FORKING.md.")
    try:
        rebrand().check(b)
    except Exception as e:      # rebrand.py's own Refused
        raise Refused(f"launcher branding: {e}")
    return b


LAUNCHER_README = """{name}: the launcher's branding
==============================

branding.json here is launcher/branding.json with your name, colours, app id and site. To build your launcher:

  1. In your own copy of the code (a fork): python3 launcher/scripts/rebrand.py PATH/TO/branding.json --dry-run
     then without --dry-run.
  2. Replace the logo files the launcher uses (launcher/ui/art/leyline-mark.png and leyline-emblem.webp) with yours.
  3. Make your own updater key and change repo and updates in branding.json to your own (docs/FORKING.md). Until
     you do, a built launcher updates itself from Leyline's releases and turns back into Leyline.
  4. python3 -I tools/fork-kit/fork_kit.py check   lists anything still shared with Leyline.
"""


def upload_md(brand: dict, domain: str, s: str) -> str:
    host = domain or "your domain"
    return f"""# Put {brand['name']} online

Everything here came from one brand.json. Nothing here holds game files, and nothing here may ever be given any.

## 1. The website and its Log in page (`site/`)

The website is a program, so it needs a small server (a VPS), not plain file hosting.

1. Rent a small Linux server. The step-by-step for Hetzner is in `docs/MOVEMENT.md`, "Way 3".
2. Copy the folder up: `scp -r site root@SERVER:/opt/{s}-site`
3. On the server: `apt-get install -y python3 caddy`, then `cd /opt/{s}-site && bash run-site.sh`
   (as a service: see `docs/MOVEMENT.md`).
4. Point {host} at the server (an A record at your domain's registrar), then
   `caddy run --config /opt/{s}-site/Caddyfile.example`. Caddy gets the TLS certificate by itself.
5. Open `https://{host}/wrath`.

Log in needs the sign-in service, which is not in this kit yet. The Log in page says so until it is.

## 2. The browser kit (`browser-kit/`)

Static files. They can sit on the same server under Caddy (`file_server`) or any static host. They are the page
around the browser client; the client itself is not in this kit yet (browser-kit/README.txt).

## 3. The host kit (`host-kit/`)

`{s}-host-kit.zip` is the network's host kit. Share it, or use it yourself: unzip, then
`Host a Leyline realm.bat` (Windows) or `bash leyline host` (Linux, Mac). Each host uses their own game client.

## 4. The launcher (`launcher/`)

`launcher/branding.json` plus `launcher/README.txt`: how to build a launcher with your name and colours.

## What is not reskinned yet

- Log in needs the sign-in service (separate program, not published yet).
- The host kit and the in-game windows still say Leyline.
- The launcher needs your own build (and your own updater key) to carry your name.
- The browser client build is not in this kit.
"""


def problems(packs: dict[str, dict[str, bytes]]) -> list[str]:
    """Every reason a packed file must not go out: the public export's own rules, and brand words people read."""
    ex = export()
    out = []
    for top, files in packs.items():
        for rel, data in sorted(files.items()):
            full = f"{top}/{rel}"
            if rel.endswith(".env.example") or rel == "gamedata-off":
                why = None if not ex.SECRET_TEXT.search(data) else "secret (key or token inside)"
            elif top == "host-kit":
                why = None     # hostkit.problems() checked these with the same rules
            else:
                why = ex.refusal(rel, data)
            if why:
                out.append(f"{full}: {why}")
            if ex.user_facing(rel) or rel.endswith((".txt", ".css")) and top != "host-kit":
                text = LEGAL.sub("", data.decode("utf-8", "replace"))
                out += [f"{top}/{h}" for h in ex.brand_hits(rel, text.encode())]
    return out


def write_tree(root: Path, files: dict[str, bytes]) -> None:
    for rel, data in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data)
        if rel.endswith(".sh"):
            p.chmod(0o755)


def make(answers: dict, out_root: Path, log=print) -> Path:
    brand, logo = build_brand(answers)
    domain = brand["links"]["site"].removeprefix("https://")
    s = slug(brand["name"])
    out = out_root / s
    host, host_problems = pack_host(brand)
    lb = launcher_branding(brand, domain)
    packs = {
        "site": pack_site(brand, logo, domain),
        "browser-kit": pack_browser(brand, logo),
        "host-kit": {},
        "launcher": {"branding.json": (json.dumps(lb, indent=2) + "\n").encode(),
                     "README.txt": LAUNCHER_README.replace("{name}", brand["name"]).encode()},
    }
    if logo:
        packs["launcher"][f"logo.{brand['logo'].rsplit('.', 1)[1]}"] = logo
    bad = problems(packs) + [f"host-kit/{p}" for p in host_problems]
    if bad:
        raise Refused("these files must not go out:\n  " + "\n  ".join(bad[:40]))
    out.mkdir(parents=True, exist_ok=True)
    (out / "brand.json").write_text(json.dumps(brand, indent=2) + "\n", encoding="utf-8")
    for top, files in packs.items():
        write_tree(out / top, files)
    folder = f"{brand['name']} host"
    write_tree(out / "host-kit" / folder, host)
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for rel in sorted(host):
            z.writestr(f"{folder}/{rel}", host[rel])
    (out / "host-kit" / f"{s}-host-kit.zip").write_bytes(buf.getvalue())
    (out / "UPLOAD.md").write_text(upload_md(brand, domain, s), encoding="utf-8")
    log(f"Made {brand['name']} in {out}")
    for top in ("site", "browser-kit", "host-kit", "launcher"):
        n = len(host) if top == "host-kit" else len(packs[top])
        log(f"  {top + '/':14} {n} files")
    log(f"  UPLOAD.md      the steps to put it online")
    return out


# ------------------------------------------------------------------------------------------------ preview
def preview(site: Path, port: int = 8091, serve: bool = True):
    """The packed site on 127.0.0.1 only (never the network). Returns the server when serve is False (tests)."""
    from http.server import ThreadingHTTPServer
    env_file = site / "portal.env.example"
    if not (site / "wrath_portal.py").is_file() or not env_file.is_file():
        raise Refused(f"{site} is not a packed site folder (make one first)")
    for line in env_file.read_text(encoding="utf-8").splitlines():
        if line.strip() and not line.lstrip().startswith("#") and "=" in line:
            k, v = line.split("=", 1)
            os.environ[k.strip()] = v.strip().replace("$HERE", str(site))
    os.environ["PUBLIC_HOST"] = "localhost"
    (site / "data").mkdir(exist_ok=True)
    wp = _load("commons_site", site / "wrath_portal.py")
    srv = ThreadingHTTPServer(("127.0.0.1", port), wp.PageHandler)
    srv.daemon_threads = True
    if not serve:
        return srv
    print(f"Your site is at http://127.0.0.1:{srv.server_address[1]}/wrath  (only this PC can see it; Ctrl+C stops it)")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
    return None


# ------------------------------------------------------------------------------------------------ the directory
AREAS = ("player", "host", "network", "site", "tool")
ALONE = ("today", "later", "plan")


def load_directory(path: Path = DIRECTORY) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def check_directory(d: dict, root: Path = REPO) -> list[str]:
    ex = export()
    bf = brandfile()
    out = []
    if d.get("format") != 1:
        out.append("format must be 1")
    seen = set()
    for c in d.get("capabilities") or []:
        cid = c.get("id", "?")
        for k in ("id", "name", "area", "what", "code", "public", "licence", "needs", "run_alone", "alone", "phase"):
            if k not in c:
                out.append(f"{cid}: missing {k}")
        if cid in seen:
            out.append(f"{cid}: id used twice")
        seen.add(cid)
        if c.get("area") not in AREAS:
            out.append(f"{cid}: area must be one of {', '.join(AREAS)}")
        if c.get("alone") not in ALONE:
            out.append(f"{cid}: alone must be one of {', '.join(ALONE)}")
        if c.get("phase") not in (1, 2, 3, 4):
            out.append(f"{cid}: phase must be 1-4")
        for p in c.get("code") or []:
            if not (root / p).exists():
                out.append(f"{cid}: {p} does not exist")
        text = " ".join([c.get("name", ""), c.get("what", ""), c.get("run_alone", ""), " ".join(c.get("needs") or [])])
        if ex.brand_hits(cid, text.encode()):
            out.append(f"{cid}: names the game or its maker")
        if bf.MONEY.search(text):
            out.append(f"{cid}: money words or links")
    if not d.get("capabilities"):
        out.append("no capabilities")
    for m in d.get("mod_directories") or []:
        if not (root / m.get("file", "")).is_file():
            out.append(f"mod directory {m.get('id')}: {m.get('file')} does not exist")
    return out


def wiki_posts(d: dict) -> list[str]:
    """The directory as plain-text forum posts (the forum shows plain text and links; each post <= BODY_MAX)."""
    names = {"today": "runs on its own today", "later": "runs on its own after some work", "plan": "designed, not built"}
    blocks = [f"THE DIRECTORY (updated {d.get('updated', '?')})\n"
              "Everything this network is made of, one piece at a time, and where the mods lists live. "
              "It is generated from tools/commons/directory.json: change that file, not this post.\n"]
    for area in AREAS:
        rows = [c for c in d["capabilities"] if c["area"] == area]
        if not rows:
            continue
        blocks.append(f"== {area.upper()} ==")
        for c in rows:
            blocks.append(f"* {c['name']} ({names[c['alone']]}; {'in the public repo' if c['public'] else 'not in the public repo yet'}; "
                          f"licence: {c['licence']})\n  {c['what']}\n  Needs: {'; '.join(c['needs'])}\n"
                          f"  Run it alone: {c['run_alone']}\n  Code: {', '.join(c['code'])}")
    blocks.append("== MODS LISTS ==")
    for m in d.get("mod_directories") or []:
        blocks.append(f"* {m['name']}: {m['what']} ({'approved only' if m['approved_only'] else 'found, not approved'})\n"
                      f"  File: {m['file']}")
    posts, cur = [], ""
    for b in blocks:
        b = b.strip() + "\n\n"
        if cur and len(cur) + len(b) > FORUM_BODY_MAX - 40:
            posts.append(cur.rstrip())
            cur = ""
        cur += b
    if cur:
        posts.append(cur.rstrip())
    n = len(posts)
    return [f"({i}/{n})\n{p}" if n > 1 else p for i, p in enumerate(posts, 1)]


# ------------------------------------------------------------------------------------------------ the wrappers
WRAPPERS = ("Start my own server.bat", "Be a node.bat")


def kit_paths(here: Path = HERE) -> dict:
    """Where the one-click wrappers find the host kit, in their own order: the folder they are in (inside a host kit),
    host-kit next to them, or the repo's hosting/kit (two folders up from tools/commons)."""
    for cand in (here, here / "host-kit", here.parents[1] / "hosting" / "kit"):
        if (cand / "Host a Leyline realm.bat").is_file():
            return {"kit": cand, "host": cand / "Host a Leyline realm.bat",
                    "node_compose": cand / "docker-compose.volunteer-relay.yml", "tools": cand / "tools",
                    "relay_code": (cand / "tools" / "wurelay").is_dir()}
    raise Refused("no host kit found next to this folder")


# ------------------------------------------------------------------------------------------------ main
def main(argv=None) -> int:
    ap = argparse.ArgumentParser(prog="commons.py", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("make", help="make your own Leyline")
    m.add_argument("--answers", help="a JSON file with the answers (name, accent, bg, text, logo, domain, discord)")
    m.add_argument("--out", default=str(HERE / "out"), help="where to put it (default tools/commons/out)")
    m.add_argument("--yes", action="store_true", help="no questions: defaults for anything not in --answers")
    p = sub.add_parser("preview", help="run a packed site on this PC only")
    p.add_argument("site")
    p.add_argument("--port", type=int, default=8091)
    sub.add_parser("check-directory", help="check tools/commons/directory.json")
    w = sub.add_parser("wiki", help="the directory as the forum's pinned post")
    w.add_argument("--out", help="write here instead of printing")
    sub.add_parser("paths", help="where the wrappers find the host kit")
    a = ap.parse_args(argv)
    try:
        if a.cmd == "make":
            answers = json.loads(Path(a.answers).read_text(encoding="utf-8")) if a.answers else None
            if not a.yes and not a.answers:
                print("Make your own Leyline: a few questions. Press Enter to keep the value in [brackets].\n")
            out = make(ask(answers, a.yes), Path(a.out))
            if not a.yes and not a.answers and input("\nLook at your site on this PC now? (y/N): ").strip().lower() == "y":
                preview(out / "site", 8091)
        elif a.cmd == "preview":
            preview(Path(a.site), a.port)
        elif a.cmd == "check-directory":
            bad = check_directory(load_directory())
            for b in bad:
                print("FAIL", b)
            print("directory OK" if not bad else f"{len(bad)} problems")
            return 1 if bad else 0
        elif a.cmd == "wiki":
            text = "\n\n----- next post -----\n\n".join(wiki_posts(load_directory()))
            if a.out:
                Path(a.out).write_text(text + "\n", encoding="utf-8")
            else:
                print(text)
        elif a.cmd == "paths":
            for k, v in kit_paths().items():
                print(f"{k}={v}")
    except Refused as e:
        print(f"Refused: {e}", file=sys.stderr)
        return 1
    except (OSError, ValueError) as e:
        print(f"Failed: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
