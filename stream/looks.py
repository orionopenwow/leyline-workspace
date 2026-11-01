"""Looks: graphics packs, UI looks and graphics presets, one of Leyline's main sections (owner 1 Nov 14:38: "make the
graphical upgrades and changes a main part of our site too, featuring different looks and 1 click install on them for
most clients").

The list is looks.json beside this file (tools/looks/looks.py edits it: check, list, approve, pin). Each entry is a
catalog project in the all.json shape (mods/all-json.md) with source "look", id look:<slug> and a "look" object:

    type      graphics-pack | ui-look | preset
    preset    for a preset: the launcher's own graphics preset id (launcher/src-tauri/src/gfx.rs PRESETS)
    uses      for a UI look: the curated addons (up:<slug>) it installs, from their authors (stream/upstream-addons.json)
    changes   what it changes, words from CHANGES
    previews  pictures we made ("basis": "ours") or the author's licence allows ("licence"); none: a styled placeholder
    featured, order, note

and, for a graphics pack the launcher can install, an install block in the listed-mod shape (kind "files": the author's
own GitHub release file, a .MPQ or a zip of them, with its sha256 and every file's sha256; launcher/src-tauri/src/looks.rs).

How it installs is worked out, never trusted from the file: one click ("launcher") for a preset, for a pack with a sound
install block, or for a UI look whose every curated addon is reviewed and published; otherwise a link to the author's
page ("link"). Only reviewed entries (review.by set) are shown. Nothing is hosted here, ever: no game data, no copies.

Served at /wrath/looks (stream/wrath_portal.py, in the hub's frame) and merged into /wrath/mods/all.json (add_looks), so
the launcher's Looks tab and the website show the same list. Standard library only.
"""
from __future__ import annotations

import hashlib
import html
import json
import os
import re
from urllib.parse import quote, urlsplit

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = os.environ.get("WU_LOOKS", os.path.join(HERE, "looks.json"))
MAX = 400
PAGE = "/wrath/looks"
DEEPLINK = "leyline://install/{id}"

TYPES = {"graphics-pack": "Graphics pack", "ui-look": "UI look", "preset": "Graphics preset"}
TYPE_FILTERS = (("", "All looks"), ("graphics-pack", "Graphics packs"), ("ui-look", "UI looks"), ("preset", "Presets"))
# What a look may say it changes (one vocabulary for the site and the launcher).
CHANGES = ("Textures", "Character models", "Creatures", "Water", "Sky", "Lighting", "Foliage", "View distance", "Shadows",
           "Ground clutter", "Weather", "Spell effects", "Anti-aliasing", "Interface", "Unit frames", "Action bars", "Chat",
           "Minimap", "Windows", "Login screen")
# launcher/src-tauri/src/gfx.rs PRESETS ids (tests check they match)
PRESETS = ("low", "classic", "balanced", "high", "ultra")
BUILDS = ("1.12", "2.4.3", "3.3.5a", "4.3.4", "5.4.8")
# hub.CLIENT_LABEL (tests check they match): the site never shows a client number in its words, chips included
CLIENT_LABEL = {"1.12": "Classic era", "2.4.3": "Second era", "3.3.5a": "Third era", "4.3.4": "Fourth era",
                "5.4.8": "Fifth era", "any": "Any client"}
OPEN_LICENCES = {"MIT", "MIT-0", "Apache-2.0", "GPL-2.0", "GPL-2.0-only", "GPL-2.0-or-later", "GPL-3.0", "GPL-3.0-only",
                 "GPL-3.0-or-later", "LGPL-2.1", "LGPL-3.0", "MPL-2.0", "BSD-2-Clause", "BSD-3-Clause", "0BSD", "ISC",
                 "Unlicense", "CC0-1.0", "Zlib", "CC-BY-4.0", "CC-BY-SA-4.0", "OFL-1.1"}

_ID = re.compile(r"look:[a-z][a-z0-9]*(-[a-z0-9]+)*")
_UP = re.compile(r"up:[a-z][a-z0-9]*(-[a-z0-9]+)*")
_HEX64 = re.compile(r"[0-9a-f]{64}")
_GITHUB = re.compile(r"https://github\.com/([A-Za-z0-9_.-]{1,100})/([A-Za-z0-9_.-]{1,100})")
# hub.BANNED_RE: the brand words never shown to a visitor
_BANNED = re.compile(r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard)|(?i:(?<![a-z])wow(?![a-z]))"
                     r"|WoW|WOW|(?<![a-z])Wow(?=[A-Z0-9])|(?<=[a-z])Wow(?![a-z])")
# hub.VERSION_RE: no client numbers in prose
_VER = (r"v?(?:1\.12(?:\.[0-9x]+)?|2\.4(?:\.3)?|3\.3(?:\.5a?)?|4\.3(?:\.4)?|5\.4(?:\.8)?"
        r"|5875|6005|8606|12340|15595|18414)")
_VERSION = re.compile(rf"(?<![\w.]){_VER}(?![\w]|\.\d)", re.I)
_CTRL = re.compile(r"[\x00-\x1f\x7f-\x9f​-‏‪-‮⁦-⁩]")
_FILE_LINK = re.compile(r"(?i)\.(zip|rar|7z|mpq|exe|msi|torrent|tar(\.\w+)?|tgz)(\?|#|$)|/releases/download/")

_cache: dict = {"key": None, "doc": None}


# ---------------------------------------------------------------------------------------------------- checks

def _https(u) -> bool:
    if not isinstance(u, str) or not u.startswith("https://") or len(u) > 400 or re.search(r"[\s\"'<>\\]", u):
        return False
    try:
        s = urlsplit(u)
    except ValueError:
        return False
    return bool(s.hostname) and "." in s.hostname and not s.username and not s.password


def files_digest(files: dict) -> str:
    """sha256 of the sorted "<path>\\0<sha256>\\n" lines (launcher upstream::files_digest)."""
    h = hashlib.sha256()
    for p in sorted(files):
        h.update(p.encode("utf-8") + b"\0" + files[p].encode("ascii") + b"\n")
    return h.hexdigest()


def _safe_rel(p: str) -> bool:
    return (isinstance(p, str) and 0 < len(p) <= 200 and not p.startswith("/") and "\\" not in p
            and all(part not in ("", ".", "..") for part in p.split("/")) and not _CTRL.search(p)
            and re.fullmatch(r"[A-Za-z0-9_. +!()-]+(/[A-Za-z0-9_. +!()-]+)*", p) is not None)


def install_problems(inst, repo: str) -> list[str]:
    """A graphics pack's install block (launcher looks.rs install_ok): the entry's own GitHub repo's release file, a .MPQ
    or a .zip, pinned by its sha256, and every file it puts in the client (client patches only) with its sha256."""
    if not isinstance(inst, dict):
        return ["install: not an object"]
    out = []
    m = _GITHUB.fullmatch(repo or "")
    url = inst.get("url")
    if inst.get("kind") != "files":
        out.append("install.kind: files")
    if inst.get("from") != "release-asset":
        out.append("install.from: release-asset (a release file is byte-stable, so its sha256 pins it)")
    if not m:
        out.append("install: the entry's repo must be a GitHub repo")
    elif not isinstance(url, str) or re.search(r"[\s?#%\\]|/\.\./", url):
        out.append("install.url: a plain address")
    else:
        rest = url.removeprefix(f"https://github.com/{m.group(1)}/{m.group(2)}/releases/download/")
        parts = rest.split("/")
        if rest == url or len(parts) != 2 or not all(parts) or ".." in parts or not parts[1].lower().endswith((".zip", ".mpq")):
            out.append("install.url: a .zip or .MPQ release file of the entry's own repo")
    if not isinstance(inst.get("sha256"), str) or not _HEX64.fullmatch(inst["sha256"]):
        out.append("install.sha256: the release file's sha256")
    if not isinstance(inst.get("rev"), str) or not inst["rev"] or len(inst["rev"]) > 80 or re.search(r"\s", inst["rev"]):
        out.append("install.rev: the release tag")
    sp = inst.get("strip_prefix", "")
    if not isinstance(sp, str) or (sp and (not sp.endswith("/") or not _safe_rel(sp[:-1]))):
        out.append("install.strip_prefix: empty or a folder ending in /")
    files = inst.get("files")
    if not isinstance(files, dict) or not files or len(files) > 16:
        out.append("install.files: 1 to 16 files")
    else:
        for p, sha in files.items():
            if not _safe_rel(p) or not p.lower().endswith(".mpq") or not isinstance(sha, str) or not _HEX64.fullmatch(sha):
                out.append(f"install.files: {p!r} must be a client patch (.MPQ) with its sha256")
        if inst.get("files_digest") != files_digest({k: v for k, v in files.items() if isinstance(v, str)}):
            out.append("install.files_digest: the digest of the file list")
        if isinstance(url, str) and url.lower().endswith(".mpq") and (len(files) != 1 or not url.endswith("/" + next(iter(files)))):
            out.append("install: a bare release file is the one .MPQ it is named")
    for k in ("size", "file_count"):
        if not isinstance(inst.get(k), int) or inst[k] < 0:
            out.append(f"install.{k}: a whole number")
    return out


def _words_problems(name: str, text: str) -> list[str]:
    out = []
    if _BANNED.search(text or ""):
        out.append(f"{name}: the game's brand words are never shown")
    if _VERSION.search(text or ""):
        out.append(f"{name}: no client numbers in words (the version chips say it)")
    if _CTRL.search(text or ""):
        out.append(f"{name}: control characters")
    return out


def problems(e) -> list[str]:
    """What is wrong with one entry ([] when it is sound). Pending entries are checked too."""
    if not isinstance(e, dict):
        return ["not an object"]
    out = []
    if not isinstance(e.get("id"), str) or not _ID.fullmatch(e["id"]) or len(e["id"]) > 37:
        out.append("id: look:<slug>")
    if e.get("source") != "look":
        out.append('source: "look"')
    for k, n in (("name", 48), ("description", 300), ("author", 80)):
        v = e.get(k)
        if not isinstance(v, str) or not v.strip() or len(v) > n:
            out.append(f"{k}: text, up to {n}")
        else:
            out += _words_problems(k, v)
    if e.get("category") not in ("graphics", "addons"):
        out.append("category: graphics or addons")
    b = e.get("builds")
    if not isinstance(b, list) or not b or any(x not in BUILDS for x in b) or len(set(b)) != len(b):
        out.append(f"builds: some of {', '.join(BUILDS)}")
    if not isinstance(e.get("licence"), str) or len(e["licence"]) > 60:
        out.append('licence: an SPDX id, or "" when none is stated')
    if not isinstance(e.get("licence_source", ""), str):
        out.append("licence_source: text")
    for k in ("repo", "page", "author_url"):
        v = e.get(k, "")
        if v and not _https(v):
            out.append(f"{k}: an https address")
    if e.get("page") and _FILE_LINK.search(e["page"]):
        out.append("page: a page, never a file")
    if not (e.get("page") or e.get("repo")):
        out.append("page or repo: where the author's page is")
    if e.get("size") is not None and (not isinstance(e["size"], int) or e["size"] < 0):
        out.append("size: bytes, or null when unknown")
    lk = e.get("look")
    if not isinstance(lk, dict):
        return out + ["look: an object"]
    t = lk.get("type")
    if t not in TYPES:
        out.append("look.type: graphics-pack, ui-look or preset")
    ch = lk.get("changes")
    if not isinstance(ch, list) or not ch or any(c not in CHANGES for c in ch):
        out.append("look.changes: words from CHANGES")
    uses = lk.get("uses", [])
    if not isinstance(uses, list) or any(not isinstance(u, str) or not _UP.fullmatch(u) for u in uses) or len(uses) > 8:
        out.append("look.uses: up:<slug> ids")
    pre = lk.get("preset", "")
    if t == "preset":
        if pre not in PRESETS:
            out.append(f"look.preset: one of {', '.join(PRESETS)}")
        if e.get("install") or uses:
            out.append("a preset installs nothing")
    elif pre:
        out.append("look.preset: only for a preset")
    if t == "ui-look" and e.get("install"):
        out.append("a UI look installs through look.uses (curated addons), not an install block")
    if t == "graphics-pack" and uses:
        out.append("a graphics pack has no look.uses")
    if e.get("install") is not None:
        out += install_problems(e["install"], e.get("repo", ""))
    pv = lk.get("previews", [])
    if not isinstance(pv, list) or len(pv) > 4:
        out.append("look.previews: up to 4")
    else:
        for p in pv:
            if not isinstance(p, dict) or not isinstance(p.get("src"), str) \
                    or not re.fullmatch(r"/wrath/art/looks/[a-z0-9][a-z0-9-]{0,60}\.webp", p["src"]):
                out.append("look.previews: src is our own /wrath/art/looks/<name>.webp")
                continue
            if p.get("basis") not in ("ours", "licence"):
                out.append('look.previews: basis "ours" (we made it) or "licence" (the author\'s licence allows it)')
            elif p["basis"] == "licence" and e.get("licence") not in OPEN_LICENCES:
                out.append("look.previews: the author's pictures only under an open licence")
            if not isinstance(p.get("alt"), str) or not p["alt"].strip() or not isinstance(p.get("credit"), str) or not p["credit"].strip():
                out.append("look.previews: alt and credit")
            else:
                out += _words_problems("look.previews.alt", p["alt"])
            if not all(isinstance(p.get(k), int) and 16 <= p[k] <= 4000 for k in ("w", "h")):
                out.append("look.previews: w and h")
    note = lk.get("note", "")
    if not isinstance(note, str) or len(note) > 240:
        out.append("look.note: text, up to 240")
    else:
        out += _words_problems("look.note", note)
    if not isinstance(lk.get("order", 0), int) or not isinstance(lk.get("featured", False), bool):
        out.append("look.order and look.featured")
    rv = e.get("review")
    if not isinstance(rv, dict) or not isinstance(rv.get("by"), str):
        out.append("review: {by, at, notes}")
    return out


def reviewed(e: dict) -> bool:
    rv = e.get("review")
    return isinstance(rv, dict) and isinstance(rv.get("by"), str) and rv["by"].strip() != ""


# ---------------------------------------------------------------------------------------------------- the list

def load(path: str | None = None) -> dict:
    """{"looks": every entry that checks out, "bad": [(id, problems)]}, re-read when the file changes."""
    path = path or PATH
    try:
        key = (path, os.path.getmtime(path))
    except OSError:
        return {"looks": [], "bad": []}
    if _cache["key"] == key:
        return _cache["doc"]
    try:
        with open(path, encoding="utf-8") as f:
            raw = json.load(f)
        items = raw.get("looks") if isinstance(raw, dict) else None
    except (OSError, ValueError):
        items = None
    good, bad, seen = [], [], set()
    for e in (items or [])[:MAX] if isinstance(items, list) else []:
        p = problems(e)
        if not p and e["id"] in seen:
            p = ["the same id twice"]
        if p:
            bad.append((e.get("id") if isinstance(e, dict) else None, p))
            continue
        seen.add(e["id"])
        good.append(e)
    good.sort(key=lambda e: (e["look"].get("order", 0), e["name"].lower()))
    doc = {"looks": good, "bad": bad}
    _cache.update(key=key, doc=doc)
    return doc


def published(path: str | None = None) -> list[dict]:
    """The reviewed entries, in their order."""
    return [e for e in load(path)["looks"] if reviewed(e)]


def upstream_ids() -> set:
    """The curated addons that are reviewed and published (stream/upstream_addons.py), for look.uses."""
    try:
        import importlib.util
        import sys
        m = sys.modules.get("wu_upstream_addons")
        if m is None:
            spec = importlib.util.spec_from_file_location("wu_upstream_addons", os.path.join(HERE, "upstream_addons.py"))
            m = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(m)
            sys.modules["wu_upstream_addons"] = m
        return {p["id"] for p in m.projects()}
    except Exception:   # noqa: BLE001: no curated addons, no one-click UI looks
        return set()


def how(e: dict, ups: set | None = None) -> str:
    """"launcher" (one click) or "link" (the author's page)."""
    lk = e["look"]
    if lk["type"] == "preset":
        return "launcher"
    if e.get("install"):
        return "launcher"
    uses = lk.get("uses") or []
    if uses:
        ups = upstream_ids() if ups is None else ups
        return "launcher" if all(u in ups for u in uses) else "link"
    return "link"


def author_page(e: dict) -> str:
    return e.get("page") or e.get("repo") or ""


def host_of(u: str) -> str:
    try:
        h = urlsplit(u).hostname or ""
    except ValueError:
        return ""
    return h[4:] if h.startswith("www.") else h


def credits(e: dict) -> str:
    """The credit line under each card: "<name> by <author> · <licence or "licence not stated"> · <source host>"."""
    lic = e.get("licence") or "licence not stated"
    where = host_of(e.get("repo") or e.get("page") or "")
    return " · ".join(x for x in (f"{e['name']} by {e['author']}", lic, where) if x)


def project(e: dict, ups: set | None = None) -> dict:
    """The entry in the all.json shape (mods/all-json.md "Looks"), with how it installs worked out."""
    p = {k: v for k, v in e.items() if k != "review"}
    p["look"] = dict(e["look"], how=how(e, ups))
    p["credits"] = credits(e)
    p["page"] = e.get("page") or e.get("repo") or ""
    p.setdefault("repo", "")
    p["review"] = {"by": e["review"]["by"], "at": e["review"].get("at", ""), "notes": ""}
    return p


def add_looks(all_json: bytes, items: list[dict] | None = None) -> bytes:
    """all.json with the published looks added (each once, sorted in by name). Unchanged when there are none or the
    body is odd."""
    items = published() if items is None else [e for e in items if not problems(e) and reviewed(e)]
    if not items:
        return all_json
    try:
        doc = json.loads(all_json)
        have = doc["projects"]
    except (ValueError, KeyError, TypeError):
        return all_json
    if not isinstance(have, list):
        return all_json
    ups = upstream_ids()
    ids = {p.get("id") for p in have if isinstance(p, dict)}
    have.extend(project(e, ups) for e in items if e["id"] not in ids)
    have.sort(key=lambda p: (str(p.get("name", "")).lower(), str(p.get("source", ""))) if isinstance(p, dict) else ("", ""))
    return (json.dumps(doc, indent=1, ensure_ascii=False) + "\n").encode("utf-8")


# ---------------------------------------------------------------------------------------------------- the page

def size_words(n) -> str:
    if n is None:
        return "Size on the author's page"
    if n == 0:
        return "Nothing to download"
    if n < 1024 * 1024:
        return f"{max(1, round(n / 1024))} KB"
    if n < 1024 ** 3:
        return f"{round(n / 1024 / 1024)} MB"
    return f"{n / 1024 ** 3:.1f} GB"


# The styled placeholder: a small drawing per type, never a game picture.
_PH = {
    "preset": '<i class="lk-bar" style="--w:92%"></i><i class="lk-bar" style="--w:64%"></i><i class="lk-bar" style="--w:38%"></i>',
    "graphics-pack": '<i class="lk-sun"></i><i class="lk-hill a"></i><i class="lk-hill b"></i>',
    "ui-look": '<i class="lk-fr a"></i><i class="lk-fr b"></i><i class="lk-row"></i>',
}

CSS = """
.lk h1{text-align:center}.lk .sub{text-align:center;max-width:44rem;margin-left:auto;margin-right:auto}
.wu-pg.lk{width:min(72rem,100%)}
.lk-filters{display:flex;flex-direction:column;gap:8px;margin:16px 0 6px}
.lk-frow{display:flex;flex-wrap:wrap;gap:6px;align-items:center;margin:0;padding:0;list-style:none}
.lk-frow b{font-size:13px;letter-spacing:.08em;text-transform:uppercase;color:var(--lk-muted,#9aa3b5);margin-right:4px;min-width:4.5rem}
.lk-frow a{display:inline-block;padding:4px 12px;border-radius:999px;border:1px solid var(--lk-border-strong,rgba(255,255,255,.2));
color:var(--lk-text-2,inherit);text-decoration:none;font-size:14px}
.lk-frow a:hover,.lk-frow a:focus-visible{border-color:var(--lk-accent,#7aa3f5)}
.lk-frow a[aria-current]{background:var(--lk-accent-soft,rgba(91,141,239,.18));border-color:var(--lk-accent,#7aa3f5);color:var(--lk-text,inherit);font-weight:650}
.lk-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(min(100%,19rem),1fr));gap:14px;margin:16px 0 8px;padding:0;list-style:none}
.lk-card{display:flex;flex-direction:column;min-width:0;border-radius:var(--lk-r-sm,10px);overflow:hidden;
border:1px solid var(--lk-border,rgba(255,255,255,.12));background:var(--lk-surface-2,rgba(255,255,255,.04))}
.lk-card:target{border-color:var(--lk-accent,#7aa3f5);box-shadow:0 0 0 2px var(--lk-accent-soft,rgba(91,141,239,.3))}
.lk-pv{position:relative;aspect-ratio:16/6;overflow:hidden;margin:0;background:linear-gradient(160deg,#1d2a44,#2c2347 55%,#3a2a3a)}
.lk-pv img{display:block;width:100%;height:100%;object-fit:cover}
.lk-pv figcaption,.lk-ph-cap{position:absolute;right:8px;bottom:6px;font-size:12px;color:#e8ecf5;background:rgba(10,12,18,.55);
padding:2px 8px;border-radius:999px}
.lk-ph-cap a{color:inherit}
.lk-ph.preset{background:linear-gradient(160deg,#16314a,#1e4a5c 60%,#2a5e5a)}
.lk-ph.ui-look{background:linear-gradient(160deg,#2a2240,#3b2d5c 60%,#4d3a6b)}
.lk-ph i{position:absolute;display:block}
.lk-bar{left:12%;height:9%;width:var(--w);max-width:76%;border-radius:999px;background:rgba(255,255,255,.75)}
.lk-bar:nth-child(1){top:24%}.lk-bar:nth-child(2){top:45%;opacity:.7}.lk-bar:nth-child(3){top:66%;opacity:.5}
.lk-sun{right:16%;top:16%;width:16%;aspect-ratio:1;border-radius:50%;background:radial-gradient(circle,#ffe6a8,#f1a95b)}
.lk-hill{bottom:-30%;border-radius:50% 50% 0 0;background:#1a3b33}.lk-hill.a{left:-10%;width:70%;height:75%;background:#245246}
.lk-hill.b{right:-12%;width:80%;height:62%;background:#1b4038}
.lk-fr{border:2px solid rgba(255,255,255,.7);border-radius:6px;background:rgba(255,255,255,.08)}
.lk-fr.a{left:8%;top:14%;width:30%;height:22%}.lk-fr.b{right:8%;top:14%;width:30%;height:22%}
.lk-row{left:8%;right:60%;bottom:12%;height:14%;border-radius:4px;background:repeating-linear-gradient(90deg,rgba(255,255,255,.7) 0 9%,transparent 9% 11%)}
.lk-b{display:flex;flex-direction:column;gap:6px;padding:12px 14px 14px;flex:1}
.wu-box .lk-b p,.wu-box .lk-b h2{margin:0}
.lk-type{margin:0;font-size:12px;letter-spacing:.1em;text-transform:uppercase;color:var(--lk-muted,#9aa3b5);font-weight:650}
.lk-card h2{margin:0;font-size:18px;text-align:left}
.lk-chips{display:flex;flex-wrap:wrap;gap:5px;margin:0;padding:0;list-style:none}
.lk-chips li{padding:1px 9px;border-radius:999px;border:1px solid var(--lk-border-strong,rgba(255,255,255,.2));font-size:12.5px;color:var(--lk-text-2,inherit)}
.lk-desc{margin:0;font-size:14.5px;line-height:1.45;color:var(--lk-text-2,inherit)}
.lk-ch,.lk-meta,.lk-credit,.lk-note{margin:0;font-size:13px;color:var(--lk-muted,#9aa3b5)}
.lk-ch b,.lk-meta b{color:var(--lk-text-2,inherit);font-weight:650}
.lk-acts{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin:auto 0 0;padding-top:6px}
.wu-box .lk-acts a.btn{min-height:36px;padding:6px 14px;font-size:14.5px}
.lk-acts a.ext{font-size:14px}
.lk .fine{text-align:center;margin:14px 0 0}
@media (max-width:560px){.lk-frow b{min-width:0;width:100%}}
"""


def _chip_link(label: str, v: str, t: str, cur: bool) -> str:
    q = "&".join(x for x in ((f"v={quote(v)}" if v else ""), (f"t={quote(t)}" if t else "")) if x)
    href = PAGE + ("?" + q if q else "")
    return f'<li><a href="{html.escape(href)}"' + (' aria-current="true"' if cur else "") + f">{html.escape(label)}</a></li>"


def card(e: dict, ups: set, labels: dict, offered) -> str:
    x = html.escape
    lk = e["look"]
    slug = e["id"].split(":", 1)[1]
    way = how(e, ups)
    page = author_page(e)
    pv = lk.get("previews") or []
    if pv:
        p = pv[0]
        pic = (f'<figure class="lk-pv"><img src="{x(p["src"])}" alt="{x(p["alt"])}" width="{int(p["w"])}" height="{int(p["h"])}" '
               f'loading="lazy" decoding="async"><figcaption>{x(p["credit"])}</figcaption></figure>')
    else:
        cap = (f'<span class="lk-ph-cap"><a href="{x(page)}" rel="noopener nofollow ugc">Pictures on the author\'s page</a></span>'
               if page and e["author"] != "Leyline" else "")
        pic = (f'<div class="lk-pv lk-ph {x(lk["type"])}" aria-hidden="{"false" if cap else "true"}">{_PH[lk["type"]]}'
               f'{cap}</div>')
    chips = "".join(f"<li>{x(labels.get(b, b))}</li>" for b in e["builds"] if b in offered)
    acts = []
    if way == "launcher":
        acts.append(f'<a class="btn primary" href="{x(DEEPLINK.format(id=e["id"]))}">Install with Leyline</a>')
    if page and e["author"] != "Leyline":
        acts.append(f'<a class="{"ext" if way == "launcher" else "btn"}" href="{x(page)}" rel="noopener nofollow ugc">Author\'s page</a>')
    elif page:
        acts.append(f'<a class="ext" href="{x(page)}" rel="noopener noreferrer">Source</a>')
    install_words = {"launcher": "One click in the launcher", "link": "From the author's page"}[way]
    if lk["type"] == "preset":
        install_words = "One click in the launcher: your game's own settings"
    note = f'<p class="lk-note">{x(lk["note"])}</p>' if lk.get("note") else ""
    return (f'<li class="lk-card" id="look-{x(slug)}" data-type="{x(lk["type"])}" data-how="{way}">{pic}<div class="lk-b">'
            f'<p class="lk-type">{x(TYPES[lk["type"]])}</p><h2>{x(e["name"])}</h2>'
            f'<ul class="lk-chips" aria-label="Works with">{chips}</ul>'
            f'<p class="lk-desc">{x(e["description"])}</p>'
            f'<p class="lk-ch"><b>Changes:</b> {x(", ".join(lk["changes"]))}</p>'
            f'<p class="lk-meta"><b>{x(size_words(e.get("size")))}</b> · {x(install_words)}</p>'
            f'<p class="lk-credit">{x(credits(e))}</p>{note}'
            f'<p class="lk-acts">{"".join(acts)}</p></div></li>')


def body(query: dict | None = None, items: list[dict] | None = None, labels: dict | None = None, offered=None,
         ups: set | None = None, download: str = "") -> str:
    """The Looks page's inner HTML (the portal puts it in the hub's frame). query: {"v": build, "t": type} from the
    address (anything else is ignored). No script: the filters are links."""
    x = html.escape
    q = query or {}
    labels = labels or CLIENT_LABEL
    offered = [b for b in (offered or BUILDS[:3]) if b in BUILDS]
    items = published() if items is None else items
    ups = upstream_ids() if ups is None else ups
    items = [e for e in items if any(b in offered for b in e["builds"])]
    v = q.get("v") if q.get("v") in offered else ""
    t = q.get("t") if q.get("t") in TYPES else ""
    shown = [e for e in items if (not v or v in e["builds"]) and (not t or e["look"]["type"] == t)]
    vrow = "".join([_chip_link("Every client", "", t, not v)] + [_chip_link(labels.get(b, b), b, t, v == b) for b in offered])
    trow = "".join(_chip_link(label, v, k, t == k) for k, label in TYPE_FILTERS)
    filters = (f'<nav class="lk-filters" aria-label="Filter looks"><ul class="lk-frow"><b>Client</b>{vrow}</ul>'
               f'<ul class="lk-frow"><b>Type</b>{trow}</ul></nav>')
    grid = (f'<ul class="lk-grid" aria-label="Looks">{"".join(card(e, ups, labels, offered) for e in shown)}</ul>' if shown else
            '<p class="sub">No looks for that yet. <a href="/wrath/looks">See every look</a>.</p>')
    # TODO(owner): the heading and the line under it, his voice
    head = ('<h1>Looks</h1><p class="sub">Make the game look new: graphics packs, new interfaces and graphics presets. '
            'Most install in one click with the Leyline launcher, and one click takes you back to the original look.</p>')
    dl = f'<a href="{x(download)}" rel="noopener noreferrer">Get the launcher</a>' if download else "Get the launcher"
    fine = ('<p class="fine">Install with Leyline opens the launcher: you pick the game folder and press Install. '
            f'{dl} first if you don\'t have it. Packs from other sites are links: made and hosted by their authors, '
            'not checked or hosted by Leyline. Changed your mind? The launcher\'s Looks tab has Back to the original look.</p>')
    return f'<div class="wu-pg wide lk"><div class="wu-box">{head}{filters}{grid}{fine}</div></div>'


def featured(n: int = 4, items: list[dict] | None = None, offered=None) -> list[dict]:
    """The front page's few looks: featured ones first, in their order, made for an offered client."""
    offered = offered or BUILDS[:3]
    items = published() if items is None else items
    pool = [e for e in items if any(b in offered for b in e["builds"])]
    return ([e for e in pool if e["look"].get("featured")] + [e for e in pool if not e["look"].get("featured")])[:n]
