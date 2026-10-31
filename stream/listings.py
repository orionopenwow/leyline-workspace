"""List your mod: any signed-in player lists a mod for one-click install in the Leyline launcher (docs/LIST-YOUR-MOD.md,
mods/SPEC.md "Listed mods").

The player pastes a link to their repo on GitHub, GitLab or Codeberg (the repo, a branch, a commit, a release or one
release file). This module pins it to one exact download on the author's own repo (a release file, or the archive of a
40-hex commit), fetches it once, finds out what it is and runs the same rules as the curated addons
(launcher/src-tauri/src/upstream-rules.json, the one rules file the launcher and tools/addon-curate/curate.py read):

    addon         3.3.5a addon folders (a .toc with Interface 30300)      -> Interface/AddOns/<Name>/
    package       a Leyline mod package (mod.json, mods/SPEC.md)           -> its ui/ addons; its realm half runs on realms
    patch         patch-*.MPQ files (or one .MPQ release file)            -> Data/patch-<X>.MPQ (a free letter)
    native        .dll / .asi / .exe at the top                            -> the client folder, after the player's OK
    realm-script  mod-ale Lua (Register*Event, no .toc)                    -> for realm owners (not into a game client)
    game-mode     one Leyline Modes Lua file (Mode.Title, OnStart...)      -> for the Modes editor (not into a game client)
    loose-data    loose Interface/ or World/ game files                    -> refused for one click (pack them as a patch)

Every file the launcher would install is recorded with its sha256 (and the digest of the whole list), so the launcher
installs exactly what was checked (launcher/src-tauri/src/listed.rs). Nothing is ever hosted here: the record keeps the
author's own address, the file list and the verdict. The download is deleted as soon as it is read.

What goes live at once ("Community · not reviewed") and what waits for a moderator in Leyline HQ (Needs you) is the
owner's policy ($WU_PORTAL_DATA/listings-policy.json, set in HQ): by default addons, packages, realm scripts and game
modes that raise no flag go live; native code, patches, textures (.blp), textures named like stock art, big packs,
new accounts and anything the hub's word rules catch wait. A refused check (game data, nested archives, unsafe paths,
forbidden realm Lua...) is not listed at all.

Listings are stored as community projects (stream/projects.py records with a "listing" object): the same ids
(link:<12 hex>), the same all.json, ratings and Hot/Rising/Top (stream/hub_rank.py), reports into HQ.

Routes (stream/wrath_portal.py routes them here):
    GET  /wrath/mods/list                    the List your mod page (plain forms, no script); ?id=<pid> manage one
    POST /wrath/mods/list                    step=check (url) | step=submit (t, ck, category, description, builds)
                                             | step=edit | step=remove | step=report (id, reason, note)
    POST /wrath/api/listings/<action>        the same as JSON for the launcher (Authorization: Bearer <friends token>)
                                             or the site (cookie + X-Leyline-CSRF): check, submit, report, edit, remove
    GET  /wrath/api/listings/mine            the caller's listings and their state

Settings (environment): WU_LISTING_FETCH_MAX (bytes fetched for a check, default 300 MB), WU_LISTING_CHECKS_HOUR (per
account, 10), WU_LISTING_CHECKS_DAY (everyone, 400), WU_LISTING_RULES (the rules file), WU_GITHUB_TOKEN (GitHub API
only), WU_GITHUB_API / WU_GITLAB_API / WU_CODEBERG_API (tests). Standard library only.
"""
from __future__ import annotations

import hashlib
import hmac
import html
import json
import logging
import os
import re
import secrets
import shutil
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile

log = logging.getLogger("wrath-portal.listings")
HERE = os.path.dirname(os.path.abspath(__file__))

PAGE = "/wrath/mods/list"
API = "/wrath/api/listings"
FORMATS = ("addon", "package", "patch", "native", "realm-script", "game-mode")
FORMAT_LABEL = {"addon": "Addon", "package": "Leyline mod package", "patch": "Client patch (.MPQ)",
                "native": "Native code (.dll / .asi / .exe)", "realm-script": "Realm script", "game-mode": "Game mode",
                "loose-data": "Loose game files", "unknown": "Not something Leyline can install"}
# What the launcher puts into a game client (the rest is listed for realm owners and the Modes editor).
CLIENT_FORMATS = ("addon", "package", "patch", "native")
REPORT_REASONS = {"broken": "It doesn't work", "malware": "Harmful or a virus", "stolen": "Not theirs / stolen work",
                  "gamedata": "Ships game files", "cheat": "A cheat or exploit", "rude": "Rude or hateful", "spam": "Spam",
                  "other": "Something else"}
DEFAULT_POLICY = {"auto_live": ["addon", "package", "realm-script", "game-mode"], "report_hide": 3, "big_mb": 64,
                  "new_account_days": 3, "new_account_per_day": 1}
BRAND_RE = re.compile(r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard|wotlk)|(?i:(?<![a-z])wow(?![a-z]))")
CTRL_RE = re.compile(r"[\x00-\x1f\x7f-\x9f​-‏‪-‮⁦-⁩]")
HEX40 = re.compile(r"[0-9a-f]{40}")
SEG = r"[A-Za-z0-9_.-]{1,100}"
FORGE_HOSTS = {"github.com": "github", "gitlab.com": "gitlab", "codeberg.org": "codeberg"}
REDIRECT_HOSTS = ("objects.githubusercontent.com", "release-assets.githubusercontent.com", "codeload.github.com",
                  "gitlab.com", "codeberg.org", "github.com")
TEXT_EXT = (".toc", ".lua", ".xml", ".json", ".txt")
TEXT_MAX = 512 * 1024
WINDOWS_DEVICES = {"con", "prn", "aux", "nul"} | {f"com{i}" for i in range(1, 10)} | {f"lpt{i}" for i in range(1, 10)}
# The launcher's own limits for what goes into a client (launcher/src-tauri/src/mods.rs).
MAX_PATCH = 1024 * 1048576
NATIVE_CODE = (".dll", ".asi", ".exe")
NATIVE_SIDE = (".ini", ".conf", ".cfg", ".json", ".toml", ".txt", ".md")
PACKAGE_TOP = ("mod.json", "ui", "realm", "native", "assets", "tests", "readme.md", "readme.txt", "license", "license.txt",
               "license.md", "changelog.md")


# ------------------------------------------------------------------------------------------------ the rules file
def _rules_path() -> str:
    for p in (os.environ.get("WU_LISTING_RULES", ""), os.path.join(HERE, "upstream-rules.json"),
              os.path.join(HERE, "..", "launcher", "src-tauri", "src", "upstream-rules.json")):
        if p and os.path.isfile(p):
            return p
    return ""


def load_rules() -> dict | None:
    p = _rules_path()
    try:
        with open(p, encoding="utf-8") as f:
            r = json.load(f)
        return r if isinstance(r, dict) and r.get("interface") and isinstance(r.get("caps"), dict) else None
    except (OSError, ValueError):
        return None


RULES = load_rules()


def ext_of(path: str) -> str:
    base = path.rsplit("/", 1)[-1]
    i = base.rfind(".")
    return base[i:].lower() if i > 0 else ""


def classify(path: str, blp_ok: bool, head: bytes = b"") -> str:
    """The curated addons' rule for one file inside an addon folder (curate.py classify, upstream.rs classify):
    "install", "skip", "blp" or "refuse: <why>". The shared cases (upstream-cases.json) hold for this copy too."""
    low = path.lower()
    parts = low.split("/")
    base, e = parts[-1], ext_of(low)
    if e in RULES["refuse_ext"]:
        return "refuse: " + e + " files are never allowed"
    h = head[:4].hex()
    for magic, what in RULES["refuse_magic"].items():
        if h.startswith(magic):
            return "refuse: " + what + " (by its first bytes)"
    if h in RULES["blp_magic"] and e != RULES["blp_ext"]:
        return "refuse: a renamed .blp texture"
    if any(p in RULES["skip_dirs"] for p in parts[:-1]):
        return "skip"
    if base in RULES["skip_names"] or e in RULES["skip_ext"]:
        return "skip"
    if e == RULES["blp_ext"]:
        return "install" if blp_ok else "blp"
    if e in RULES["install_ext"]:
        return "install"
    return "skip"


def name_ok(name: str) -> bool:
    low = name.lower()
    return (bool(re.fullmatch(RULES["name_re"], name)) and low not in RULES["reserved"]
            and not any(low.startswith(p) for p in RULES["reserved_prefix"]) and not name.endswith("."))


def interface_ok(value: str) -> bool:
    return RULES["interface"] in [v.strip() for v in (value or "").replace("\r", "").split(",")]


def stock_art(path: str) -> bool:
    low = path.lower()
    tails = {low, low.rsplit("/", 1)[-1]} | {low.split("/", i)[-1] for i in range(1, low.count("/") + 1)}
    return any(re.search(p, t) for p in RULES["stock_art"] for t in tails)


def files_digest(files: dict) -> str:
    """sha256 of the sorted "<path>\\0<sha256>\\n" lines (curate.py files_digest, upstream.rs files_digest)."""
    h = hashlib.sha256()
    for p in sorted(files):
        h.update(p.encode("utf-8") + b"\0" + files[p].encode() + b"\n")
    return h.hexdigest()


def safe_rel(name: str) -> bool:
    """A path Windows can hold, without zip-slip (the launcher's mods::safe_rel)."""
    if not name or name.startswith("/") or name[1:2] == ":" or len(name) > 400 or name.count("/") >= 16:
        return False
    for part in name.split("/"):
        if (not part or part in (".", "..") or part.endswith((".", " ")) or len(part) > 128
                or any(ord(c) < 32 or c in ':*?"<>|' for c in part) or part.split(".")[0].lower() in WINDOWS_DEVICES):
            return False
    return True


def clean_text(v, n: int = 120) -> str:
    if not isinstance(v, str):
        return ""
    return re.sub(r"\s+", " ", CTRL_RE.sub(" ", v)).strip()[:n]


NAME_BRAND_RE = re.compile(
    r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard|wotlk|wrath\s+of\s+the\s+lich\s+king)"
    r"|(?i:(?<![a-z])wow(?![a-z]))|WoW|WOW|(?<![a-z])Wow(?=[A-Z0-9])|(?<=[a-z])Wow(?![a-z])")


def clean_name(raw: str) -> str:
    """A shown name: TOC colour codes and the game's brand words out (curate.py clean_name, the shared cases)."""
    s = re.sub(r"\|c[0-9a-fA-F]{8}|\|r|\|T[^|]*\|t", "", raw or "")
    s = NAME_BRAND_RE.sub(" ", s)
    s = re.sub(r"\(\s*\)|\[\s*\]", " ", s)
    s = re.sub(r"(?<=[\s_.-])[\s_.-]+|^[\s_.*-]+|[\s_.-]+$", "", s)
    s = re.sub(r"\s{2,}", " ", s).strip(" -_.*")
    return "".join(c for c in s if c.isprintable())[:64]


def parse_toc(text: str) -> tuple[dict, list]:
    meta, files = {}, []
    for line in (text or "").splitlines():
        s = line.strip().lstrip("﻿")
        if s.startswith("##"):
            k, sep, v = s[2:].partition(":")
            if sep:
                meta.setdefault(k.strip(), v.strip())
        elif s and not s.startswith("#"):
            files.append(s.replace("\\", "/"))
    return meta, files


# ------------------------------------------------------------------------------------------------ the link
def parse_source(raw) -> dict:
    """What a pasted link points at, or ValueError with words for the player. Only GitHub, GitLab and Codeberg:
    {forge, host, owner, repo, repo_url, ref, commit, tag, asset, latest}."""
    if not isinstance(raw, str) or not raw.strip():
        raise ValueError("Paste the link to your mod's repo.")
    raw = raw.strip()
    if len(raw) > 400 or CTRL_RE.search(raw) or re.search(r"\s", raw):
        raise ValueError("That link has spaces or hidden characters in it.")
    try:
        u = urllib.parse.urlsplit(raw)
        port = u.port
    except ValueError:
        raise ValueError("That doesn't look like a link.") from None
    if u.scheme.lower() != "https" or u.username or u.password or port not in (None, 443):
        raise ValueError("Links must start with https:// (no login, no port).")
    host = (u.hostname or "").lower()
    if host.startswith("www."):
        host = host[4:]
    forge = FORGE_HOSTS.get(host)
    if not forge:
        raise ValueError("Paste a link to a repo on GitHub, GitLab or Codeberg. Leyline installs from the author's own "
                         "repo, so other sites can't be one-click yet.")
    path = urllib.parse.unquote(u.path or "")
    parts = [p for p in path.split("/") if p]
    if len(parts) < 2 or not all(re.fullmatch(SEG, p) for p in parts[:2]) or parts[0] in ("orgs", "users", "explore", "settings"):
        raise ValueError(f"Link the repo itself: https://{host}/<you>/<your-mod>.")
    owner, repo = parts[0], parts[1]
    if repo.lower().endswith(".git"):
        repo = repo[:-4]
    rest = parts[2:]
    out = {"forge": forge, "host": host, "owner": owner, "repo": repo, "repo_url": f"https://{host}/{owner}/{repo}",
           "ref": "", "commit": "", "tag": "", "asset": "", "latest": False}
    if forge == "gitlab" and rest[:1] == ["-"]:
        rest = rest[1:]
    if not rest:
        return out
    if any(not safe_rel(p) for p in rest):
        raise ValueError("That link's path isn't valid.")
    head = rest[0]
    if head in ("tree", "src") and len(rest) >= 2:
        ref = "/".join(rest[2:] if forge == "codeberg" and rest[1] in ("branch", "tag", "commit") else rest[1:])
        key = "commit" if HEX40.fullmatch(ref) else "ref"
        out[key] = ref
    elif head in ("commit", "commits") and len(rest) >= 2 and HEX40.fullmatch(rest[1]):
        out["commit"] = rest[1]
    elif head == "archive" and len(rest) >= 2:
        name = rest[-1]
        stem = re.sub(r"(?i)\.(zip|tar\.gz)$", "", name)
        if HEX40.fullmatch(stem):
            out["commit"] = stem
        elif forge == "gitlab" and HEX40.fullmatch(rest[1]):
            out["commit"] = rest[1]
        else:
            out["ref"] = stem.removeprefix("refs/heads/") if forge == "github" else rest[1]
    elif head == "releases":
        if len(rest) >= 2 and rest[1] == "latest":
            out["latest"] = True
        elif len(rest) >= 3 and rest[1] == "tag":
            out["tag"] = "/".join(rest[2:])
        elif len(rest) >= 4 and rest[1] == "download" and forge == "github":
            out["tag"], out["asset"] = rest[2], rest[3]
        elif len(rest) >= 2 and forge != "github":
            out["tag"] = rest[-1]
        else:
            out["latest"] = True
    elif head in ("blob", "raw"):
        raise ValueError("That's one file. Link the repo, a release, or a release file (.zip or .MPQ).")
    return out


def redirect_ok(to: str) -> bool:
    try:
        u = urllib.parse.urlsplit(to)
    except ValueError:
        return False
    return u.scheme == "https" and (u.hostname or "").lower() in REDIRECT_HOSTS and not u.username and u.port in (None, 443)


class _NoBadRedirect(urllib.request.HTTPRedirectHandler):
    max_redirections = 5

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        test = os.environ.get("WU_LISTING_TEST_HOST", "")
        if not redirect_ok(newurl) and not (test and newurl.startswith(test)):
            raise urllib.error.HTTPError(newurl, code, "a redirect Leyline does not follow", headers, fp)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


class Net:
    """The forges' public APIs and their downloads (the tests swap it for a fake)."""

    def __init__(self, timeout: float = 20.0):
        self.timeout = timeout

    def api(self, forge: str) -> str:
        return {"github": os.environ.get("WU_GITHUB_API", "https://api.github.com"),
                "gitlab": os.environ.get("WU_GITLAB_API", "https://gitlab.com/api/v4"),
                "codeberg": os.environ.get("WU_CODEBERG_API", "https://codeberg.org/api/v1")}[forge].rstrip("/")

    def json(self, forge: str, path: str):
        """The decoded answer, "missing" for a 404, None when the API could not be reached."""
        headers = {"Accept": "application/json", "User-Agent": "leyline-listings"}
        if forge == "github":
            headers["Accept"] = "application/vnd.github+json"
            tok = os.environ.get("WU_GITHUB_TOKEN", "").strip()
            if tok:
                headers["Authorization"] = "Bearer " + tok   # only ever to the GitHub API, never logged
        try:
            with urllib.request.urlopen(urllib.request.Request(self.api(forge) + path, headers=headers), timeout=8) as r:
                return json.loads(r.read(2_000_000).decode("utf-8"))
        except urllib.error.HTTPError as e:
            return "missing" if e.code in (404, 410, 451) else None
        except (urllib.error.URLError, OSError, ValueError, UnicodeDecodeError):
            return None

    def download(self, url: str, dest: str, cap: int) -> tuple[int, str]:
        """Fetch url into dest (at most cap bytes), redirects only to the forges' download hosts. (size, sha256)."""
        opener = urllib.request.build_opener(_NoBadRedirect())
        req = urllib.request.Request(url, headers={"User-Agent": "leyline-listings"})
        h = hashlib.sha256()
        got = 0
        with opener.open(req, timeout=self.timeout) as r, open(dest, "wb") as f:
            n = r.headers.get("Content-Length")
            if n and n.isdigit() and int(n) > cap:
                raise ValueError("too big")
            while True:
                b = r.read(1 << 16)
                if not b:
                    break
                got += len(b)
                if got > cap:
                    raise ValueError("too big")
                h.update(b)
                f.write(b)
        return got, h.hexdigest()


# ------------------------------------------------------------------------------------------------ pinning
GOOD_ASSET = re.compile(r"(?i)wotlk|wrath|3\.3\.5|335|30300")
BAD_ASSET = re.compile(r"(?i)classic|retail|vanilla|tbc|bcc|cata|mists|mainline|1\.12|2\.4|source")


def pick_asset(assets: list) -> dict | None:
    """The one release file to install: a .zip or .MPQ, the one for this client when there are several."""
    cand = [a for a in assets if isinstance(a, dict) and str(a.get("name", "")).lower().endswith((".zip", ".mpq"))]
    pref = [a for a in cand if GOOD_ASSET.search(a["name"])] or [a for a in cand if not BAD_ASSET.search(a["name"])]
    if len(pref) == 1 or (pref and GOOD_ASSET.search(pref[0]["name"])):
        return pref[0]
    return None


def commit_url(src: dict, sha: str) -> str:
    o, r = src["owner"], src["repo"]
    return {"github": f"https://github.com/{o}/{r}/archive/{sha}.zip",
            "gitlab": f"https://gitlab.com/{o}/{r}/-/archive/{sha}/{r}-{sha}.zip",
            "codeberg": f"https://codeberg.org/{o}/{r}/archive/{sha}.zip"}[src["forge"]]


def resolve(src: dict, net: Net) -> dict:
    """The exact download for a parsed link: {from, url, rev, follow, tag, asset, size?, digest?}. ValueError with
    words for the player. `follow` is what an update check asks again ("latest" release, "branch:<name>"), or ""
    when the player pinned one commit or tag (then the listing never moves on its own)."""
    f, o, r = src["forge"], src["owner"], src["repo"]
    q = urllib.parse.quote
    if f == "github":
        if src["asset"] or src["tag"] or src["latest"] or not (src["ref"] or src["commit"]):
            path = f"/repos/{q(o)}/{q(r)}/releases/" + (f"tags/{q(src['tag'], safe='')}" if src["tag"] else "latest")
            rel = net.json(f, path)
            if isinstance(rel, dict) and not rel.get("draft"):
                assets = rel.get("assets") if isinstance(rel.get("assets"), list) else []
                a = (next((x for x in assets if isinstance(x, dict) and x.get("name") == src["asset"]), None)
                     if src["asset"] else pick_asset(assets))
                if src["asset"] and a is None:
                    raise ValueError(f"That release has no file named {src['asset']}.")
                u = str((a or {}).get("browser_download_url") or "")
                if a and u.startswith(f"https://github.com/{o}/{r}/releases/download/") and not re.search(r"[\s?#%\\]", u):
                    dig = str(a.get("digest") or "")
                    return {"from": "release-asset", "url": u, "rev": str(rel.get("tag_name") or "")[:80],
                            "follow": "latest" if not src["tag"] else "", "tag": str(rel.get("tag_name") or ""),
                            "asset": str(a.get("name")), "size": a.get("size") if isinstance(a.get("size"), int) else None,
                            "digest": dig[7:] if re.fullmatch(r"sha256:[0-9a-f]{64}", dig) else ""}
                if src["tag"] or src["asset"]:
                    raise ValueError("That release has no .zip or .MPQ file Leyline can pick. Link the file itself.")
            elif src["tag"] or src["latest"]:
                if rel is None:
                    raise ValueError("GitHub isn't answering right now. Please try again in a minute.")
                raise ValueError("We couldn't find that release. Is the repo public?")
        ref = src["commit"] or src["ref"]
        follow = ""
        if not ref:
            meta = net.json(f, f"/repos/{q(o)}/{q(r)}")
            if meta == "missing" or (isinstance(meta, dict) and meta.get("private")):
                raise ValueError("We couldn't find a public repo at that link. Is it public, and spelled right?")
            if not isinstance(meta, dict):
                raise ValueError("GitHub isn't answering right now. Please try again in a minute.")
            ref = str(meta.get("default_branch") or "main")
        if not src["commit"]:
            follow = "branch:" + ref
        c = net.json(f, f"/repos/{q(o)}/{q(r)}/commits/{q(ref, safe='')}")
        sha = c.get("sha") if isinstance(c, dict) else None
    elif f == "gitlab":
        pid = q(o + "/" + r, safe="")
        ref = src["commit"] or src["ref"] or src["tag"]
        follow = "" if src["commit"] or src["tag"] else "branch:"
        if not ref:
            meta = net.json(f, f"/projects/{pid}")
            if meta == "missing" or (isinstance(meta, dict) and meta.get("visibility") not in (None, "public")):
                raise ValueError("We couldn't find a public repo at that link. Is it public, and spelled right?")
            if not isinstance(meta, dict):
                raise ValueError("GitLab isn't answering right now. Please try again in a minute.")
            ref = str(meta.get("default_branch") or "main")
        follow = follow + ref if follow else ""
        c = net.json(f, f"/projects/{pid}/repository/commits/{q(ref, safe='')}")
        sha = c.get("id") if isinstance(c, dict) else None
    else:
        ref = src["commit"] or src["ref"] or src["tag"]
        follow = "" if src["commit"] or src["tag"] else "branch:"
        if not ref:
            meta = net.json(f, f"/repos/{q(o)}/{q(r)}")
            if meta == "missing" or (isinstance(meta, dict) and meta.get("private")):
                raise ValueError("We couldn't find a public repo at that link. Is it public, and spelled right?")
            if not isinstance(meta, dict):
                raise ValueError("Codeberg isn't answering right now. Please try again in a minute.")
            ref = str(meta.get("default_branch") or "main")
        follow = follow + ref if follow else ""
        c = net.json(f, f"/repos/{q(o)}/{q(r)}/commits/{q(ref, safe='')}")
        sha = (c.get("sha") or c.get("id")) if isinstance(c, dict) else None
    if not isinstance(sha, str) or not HEX40.fullmatch(sha):
        raise ValueError("We couldn't find that branch or commit. Is the repo public?" if c == "missing"
                         else "The repo's site isn't answering right now. Please try again in a minute.")
    return {"from": "commit-archive", "url": commit_url(src, sha), "rev": sha, "follow": follow, "tag": "", "asset": ""}


# ------------------------------------------------------------------------------------------------ reading a download
class Entry(dict):
    """One file: path (after the common top folder), size, sha256, head (4 bytes), text (small text files)."""


def scan(path: str, asset_name: str = "") -> list[Entry]:
    """Every file of a download: a zip's files, or the file itself (a bare .MPQ release file). ValueError for what the
    launcher's reader refuses (zip-slip, links, passwords, case twins, too many or too big)."""
    caps = RULES["caps"]
    if not zipfile.is_zipfile(path):
        name = asset_name.rsplit("/", 1)[-1]
        if not name.lower().endswith(".mpq") or not safe_rel(name):
            raise ValueError("the download is not a zip file")
        h = hashlib.sha256()
        head = b""
        with open(path, "rb") as f:
            while True:
                b = f.read(1 << 16)
                if not b:
                    break
                head = head or b[:4]
                h.update(b)
        return [Entry(path=name, size=os.path.getsize(path), sha256=h.hexdigest(), head=head, text="")]
    out, seen, total = [], set(), 0
    with zipfile.ZipFile(path) as zf:
        infos = [i for i in zf.infolist() if not i.is_dir()]
        if len(infos) > caps["files"] * 2:
            raise ValueError("more than %d files in it" % (caps["files"] * 2))
        for i in infos:
            name = i.filename.replace("\\", "/")
            if not safe_rel(name):
                raise ValueError("a file name Leyline cannot install safely: " + name[:120])
            if (i.external_attr >> 16) & 0o170000 == 0o120000:
                raise ValueError("a symbolic link: " + name[:120])
            if i.flag_bits & 0x1:
                raise ValueError("a password-protected file: " + name[:120])
            if i.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                raise ValueError("a file packed in a way Leyline cannot read: " + name[:120])
            if name.lower() in seen:
                raise ValueError("two files whose names differ only in case: " + name[:120])
            seen.add(name.lower())
            total += i.file_size
            if total > MAX_PATCH + caps["total"]:
                raise ValueError("it unpacks too big")
            h = hashlib.sha256()
            head, keep, n = b"", ext_of(name) in TEXT_EXT and i.file_size <= TEXT_MAX, 0
            buf = []
            with zf.open(i) as f:
                while True:
                    b = f.read(1 << 16)
                    if not b:
                        break
                    if not head:
                        head = b[:4]
                    n += len(b)
                    h.update(b)
                    if keep:
                        buf.append(b)
            text = b"".join(buf).decode("utf-8", "replace") if keep else ""
            out.append(Entry(path=name, size=n, sha256=h.hexdigest(), head=head, text=text))
    tops = {e["path"].split("/", 1)[0] for e in out}
    prefix = tops.pop() + "/" if len(tops) == 1 and all("/" in e["path"] for e in out) else ""
    for e in out:
        e["path"] = e["path"][len(prefix):]
    if out:
        out[0]["_prefix"] = prefix
    return out


# ------------------------------------------------------------------------------------------------ what is it
_LUA_STRIP = re.compile(r"--\[(=*)\[.*?\]\1\]|--[^\n]*|\[(=*)\[.*?\]\2\]|\"(?:\\.|[^\"\\\n])*\"|'(?:\\.|[^'\\\n])*'", re.S)
FORBIDDEN_LUA = [
    (r"\bio\s*\.", "io.*"), (r"\bos\s*\.\s*(?!time\b|clock\b|date\b|difftime\b)\w+", "os.* (other than time/clock/date)"),
    (r"\bloadstring\b", "loadstring"), (r"(?<![\w.:])load\s*\(", "load"), (r"\bloadfile\b", "loadfile"),
    (r"\bdofile\b", "dofile"), (r"\brequire\b", "require"), (r"(?<![\w.:])module\s*\(", "module"),
    (r"\bdebug\s*\.", "debug.*"), (r"\bpackage\s*\.", "package.*"), (r"\b[sg]etfenv\b", "setfenv/getfenv"),
    (r"\b_G\b", "_G"), (r"\bstring\s*\.\s*dump\b", "string.dump"), (r"\bcollectgarbage\b", "collectgarbage"),
    (r"\bHttpRequest\b", "HttpRequest"), (r"\bRunCommand\b", "RunCommand"), (r"\bAuthDB\w*", "AuthDB*"),
    (r"\bRegisterPacketEvent\b", "RegisterPacketEvent"), (r"\bReload(?:ALE|Eluna)\b", "ReloadALE"),
    (r"(?<![\w.]):\s*Ban\s*\(|(?<![\w.:])Ban\s*\(", "Ban"), (r":\s*Kick\s*\(", "Kick"),
]
_FORBIDDEN = [(re.compile(p), w) for p, w in FORBIDDEN_LUA]
ALE_RE = re.compile(r"\bRegister(?:Player|Server|Creature|GameObject|Item|Group|Guild|BG|Map|Instance|Spell|Ticket|"
                    r"PlayerGossip|CreatureGossip|GameObjectGossip|ItemGossip)Event\s*\(")
MODE_RE = re.compile(r"\bMode\s*\.\s*Title\s*=")
MODE_HOOK_RE = re.compile(r"\bfunction\s+On(?:Start|Tick|Key|Join|Leave)\s*\(")
MODE_MAX = 14 * 1024


def lua_problems(text: str) -> list[str]:
    """The realm Lua rules (mods/SPEC.md, Realm rules 3): what a script uses that it may not."""
    code = _LUA_STRIP.sub(" ", text or "")
    return sorted({w for rx, w in _FORBIDDEN if rx.search(code)})


def find_addons(rel: dict) -> tuple[dict, dict]:
    """Addon folders ({archive dir: name}, {name: toc text}): curate.py find_addons."""
    tocs = {p: (parse_toc(e["text"])[0], e["text"]) for p, e in rel.items() if p.lower().endswith(".toc")}
    folders, texts = {}, {}
    root = sorted(p for p in tocs if "/" not in p and interface_ok(tocs[p][0].get("Interface", "")))
    if root:
        name = sorted(root, key=lambda p: (len(p), p))[0][:-4]
        folders[""] = name
        texts[name] = tocs[name + ".toc"][1]
        return folders, texts
    cands = []
    for p in tocs:
        parts = p.split("/")
        if len(parts) in (2, 3) and parts[-1][:-4] == parts[-2] and interface_ok(tocs[p][0].get("Interface", "")):
            cands.append("/".join(parts[:-1]))
    for d in sorted(cands, key=lambda d: (d.count("/"), d)):
        if any(d.startswith(o + "/") for o in folders):
            continue
        folders[d] = d.rsplit("/", 1)[-1]
        texts[folders[d]] = tocs[d + "/" + folders[d] + ".toc"][1]
    return folders, texts


def _mb(n: int) -> str:
    return f"{n / 1048576:.1f} MB" if n >= 1048576 else f"{max(1, n // 1024)} KB"


def _patch_name(path: str) -> str:
    return path.rsplit("/", 1)[-1]


def detect(entries: list[Entry]) -> str:
    """The format of a download (see the module's table)."""
    rel = [e["path"] for e in entries]
    low = [p.lower() for p in rel]
    if any(p == "mod.json" for p in low):
        return "package"
    mpq = [p for p in low if p.endswith(".mpq")]
    if mpq:
        return "patch"
    by = {e["path"]: e for e in entries}
    folders, _ = find_addons(by)
    tops = [p for p in low if "/" not in p]
    native = [p for p in tops if ext_of(p) in NATIVE_CODE]
    if native and not any(ext_of(p) == ".toc" for p in tops):
        return "native"
    if folders:
        return "addon"
    if any(p.startswith(("interface/", "world/", "textures/", "character/", "creature/", "item/", "sound/", "dbfilesclient/"))
           and ext_of(p) in (".blp", ".m2", ".wmo", ".adt", ".dbc", ".skin", ".wav", ".mp3", ".ogg") for p in low):
        return "loose-data"
    luas = [e for e in entries if ext_of(e["path"]) == ".lua"]
    if luas and len(luas) == 1 and MODE_RE.search(luas[0]["text"]) and MODE_HOOK_RE.search(luas[0]["text"]):
        return "game-mode"
    if luas and any(ALE_RE.search(e["text"]) for e in luas):
        return "realm-script"
    return "unknown"


def analyse(entries: list[Entry], pin: dict, policy: dict | None = None) -> dict:
    """The check: {ok, format, label, client, problems (refused), flags (a moderator looks first), notes, plan (what
    one click does, in plain words), install (the block the launcher checks), name, version, author, size}."""
    pol = dict(DEFAULT_POLICY, **(policy or {}))
    out = {"ok": False, "format": "unknown", "label": FORMAT_LABEL["unknown"], "client": False, "problems": [],
           "flags": [], "notes": [], "plan": [], "install": None, "name": "", "version": "", "author": "", "size": 0}
    if not entries:
        out["problems"].append("there are no files in it")
        return out
    prefix = entries[0].get("_prefix", "")
    caps = RULES["caps"]
    fmt = detect(entries)
    out["format"], out["label"], out["client"] = fmt, FORMAT_LABEL.get(fmt, fmt), fmt in CLIENT_FORMATS
    P, F, N = out["problems"], out["flags"], out["notes"]

    def dev(p: str) -> bool:
        parts = p.lower().split("/")
        return any(x in RULES["skip_dirs"] for x in parts[:-1])
    live = [e for e in entries if not dev(e["path"])]
    # Refused anywhere in the download: game data, nested archives, programs other than the client's native mods.
    for e in live:
        p, x, h = e["path"], ext_of(e["path"]), e["head"][:4].hex()
        if x == ".mpq" and fmt == "patch":
            if h not in ("4d50511a", "4d50511b"):
                P.append(f"{p} is named .MPQ but is not an MPQ archive")
            continue
        if x in NATIVE_CODE and fmt in ("native", "addon", "package"):
            continue
        if x == RULES["blp_ext"]:
            continue
        v = classify(p.rsplit("/", 1)[-1], True, e["head"])
        if v.startswith("refuse"):
            P.append(f"{p}: {v[8:]}")
    if fmt == "loose-data":
        P.append("loose game files (Interface/, World/...) are not loaded by the client: pack them into a "
                 "patch-<letter>.MPQ and release that, then Leyline can install it")
    if fmt == "unknown":
        tocs = sorted({parse_toc(e["text"])[0].get("Interface", "?").strip() for e in live if e["path"].lower().endswith(".toc")})
        P.append("no addon for this client (" + ("Interface " + ", ".join(tocs) if tocs else "no .toc, mod.json, .MPQ or script found") + ")")
    total = sum(e["size"] for e in live)
    out["size"] = total
    if total > caps["download"] + MAX_PATCH or len(live) > caps["files"]:
        P.append(f"too big: {len(live)} files, {_mb(total)}")
    if P:
        return out
    files: dict[str, str] = {}
    sizes: dict[str, int] = {}
    folders: dict[str, str] = {}
    by = {e["path"]: e for e in live}
    kind = "files"
    if fmt == "addon":
        kind = "addon"
        folders, texts = find_addons(by)
        for name in folders.values():
            if not name_ok(name):
                P.append(f"{name}: not a folder name Leyline installs")
        if P:
            return out
        blp, art, missing = [], [], []
        for p, e in sorted(by.items()):
            home = next((d for d in sorted(folders, key=len, reverse=True) if d == "" or p.startswith(d + "/")), None)
            if home is None:
                continue
            inner = p[len(home) + 1:] if home else p
            if home == "" and "/" not in inner and inner.lower().endswith(".toc") and inner[:-4] != folders[""]:
                continue
            x = ext_of(inner)
            if x in NATIVE_CODE:
                P.append(f"{p}: programs inside an addon folder are never allowed")
                continue
            v = classify(inner, True, e["head"])
            if v != "install":
                continue
            dest = folders[home] + "/" + inner
            if x == RULES["blp_ext"]:
                blp.append(dest)
                if stock_art(inner):
                    art.append(dest)
            if e["size"] > caps["file"]:
                P.append(f"{dest} is larger than {caps['file'] // 1048576} MB")
            files[dest] = e["sha256"]
            sizes[dest] = e["size"]
        tops = [e for e in live if "/" not in e["path"] and ext_of(e["path"]) in NATIVE_CODE]
        if tops:
            P.append("an addon with programs beside it: list the program separately")
        for name, text in sorted(texts.items()):
            meta, listed = parse_toc(text)
            out["name"] = out["name"] or clean_name(meta.get("Title", "")) or name
            out["version"] = out["version"] or clean_name(meta.get("Version", ""))[:40]
            out["author"] = out["author"] or clean_name(meta.get("Author", ""))[:60]
            missing += [name + "/" + i for i in listed if name + "/" + i not in files]
        if missing:
            F.append(f"its .toc lists {len(missing)} file(s) that are not there ({', '.join(missing[:3])})")
        if art:
            F.append(f"{len(art)} texture(s) named like the game's own interface art: a moderator checks them ({', '.join(art[:3])})")
        elif blp:
            F.append(f"{len(blp)} .blp texture(s): textures are allowed, and a moderator looks first")
        names = sorted(set(folders.values()))
        out["plan"].append(f"Adds {len(names)} addon folder{'s' if len(names) != 1 else ''} to Interface/AddOns: "
                           + ", ".join(names[:6]) + ("..." if len(names) > 6 else "") + ".")
    elif fmt == "package":
        try:
            m = json.loads(by.get("mod.json", Entry(text="{}"))["text"] or "{}")
        except ValueError:
            m = None
        if not isinstance(m, dict) or not re.fullmatch(r"[a-z][a-z0-9]*(-[a-z0-9]+)*", str(m.get("id", ""))) \
                or not 3 <= len(str(m.get("id", ""))) <= 32:
            P.append("mod.json does not read as a Leyline mod package (mods/SPEC.md: id, name, version, kind...)")
            return out
        out["name"], out["version"] = clean_name(str(m.get("name") or m["id"])), clean_text(str(m.get("version") or ""), 40)
        out["author"] = clean_name(str(m.get("author") or ""))[:60]
        ui_addons = set()
        for p, e in sorted(by.items()):
            low = p.lower()
            top = low.split("/", 1)[0]
            if top not in PACKAGE_TOP:
                P.append(f"{p}: a mod package holds only mod.json, ui/, realm/, native/, assets/ and notes")
                continue
            x = ext_of(p)
            if x == RULES["blp_ext"]:
                P.append(f"{p}: .blp textures are not allowed in a mod package (use .tga)")
                continue
            if top == "realm" and x == ".lua":
                bad = lua_problems(e["text"])
                if bad:
                    P.append(f"{p} uses what realm scripts may not: {', '.join(bad)}")
            if x in NATIVE_CODE and top != "native":
                P.append(f"{p}: programs in a mod package go in its native/ folder")
                continue
            if low == "mod.json" or (top == "ui" and x in RULES["install_ext"]) or (top == "native" and x in NATIVE_CODE + NATIVE_SIDE):
                files[p] = e["sha256"]
                sizes[p] = e["size"]
                if top == "ui" and p.count("/") >= 2:
                    ui_addons.add(p.split("/")[1])
        for a in sorted(ui_addons):
            if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_.-]{0,63}", a) or not interface_ok(parse_toc(by.get(f"ui/{a}/{a}.toc", Entry(text=""))["text"])[0].get("Interface", "")):
                P.append(f"ui/{a}: an addon folder needs ui/{a}/{a}.toc with Interface {RULES['interface']}")
        has_realm = any(p.lower().startswith("realm/") for p in by)
        if any(p.lower().startswith("native/") and ext_of(p) in NATIVE_CODE for p in files):
            F.append("native code: it runs with full access to the player's PC, so a moderator looks first")
        if ui_addons:
            out["plan"].append("Adds its addon" + ("s" if len(ui_addons) > 1 else "") + " to Interface/AddOns: " + ", ".join(sorted(ui_addons)) + ".")
        if has_realm:
            out["plan"].append("Its realm half (realm/) runs only on realms that add it, never in your game.")
        if not ui_addons and not any(p.lower().startswith("native/") for p in files):
            out["client"] = False
            files = {}
    elif fmt == "patch":
        for e in live:
            p = e["path"]
            x = ext_of(p)
            depth_ok = "/" not in p or (p.lower().startswith("data/") and p.count("/") == 1)
            if x == ".mpq":
                if not depth_ok:
                    P.append(f"{p}: a patch goes at the top of the zip (or in its Data folder)")
                elif e["size"] > MAX_PATCH:
                    P.append(f"{p} is larger than 1 GB")
                else:
                    files[p] = e["sha256"]
                    sizes[p] = e["size"]
            elif "/" not in p and x in (".txt", ".md") or "/" not in p and p.lower() in ("license", "licence", "readme"):
                continue
            elif x not in RULES["skip_ext"]:
                P.append(f"{p}: a patch download holds only .MPQ files (and notes)")
        names = sorted(_patch_name(p) for p in files)
        F.append("a client patch changes the game files your client loads; a moderator checks it first")
        out["name"] = out["name"] or (names[0][:-4] if names else "")
        out["plan"].append(f"Adds {', '.join(names[:4])} to your client's Data folder (as the next free patch letter when "
                           "that one is taken), so the game's own patches are never replaced.")
    elif fmt == "native":
        for e in live:
            p = e["path"]
            x = ext_of(p)
            if "/" not in p and x in NATIVE_CODE + NATIVE_SIDE and p.lower().split(".")[0] not in ("readme", "license", "licence", "changelog"):
                if e["size"] > 10 * 1048576:
                    P.append(f"{p} is larger than 10 MB")
                files[p] = e["sha256"]
                sizes[p] = e["size"]
            elif "/" in p and x in NATIVE_CODE:
                P.append(f"{p}: programs go at the top of the download")
        code = sorted(p for p in files if ext_of(p) in NATIVE_CODE)
        F.append("native code: it runs with full access to the player's PC, so a moderator looks first")
        out["name"] = out["name"] or (code[0].rsplit(".", 1)[0] if code else "")
        out["plan"].append(f"Puts {', '.join(code[:4])} in your client folder. Leyline asks you first: native code "
                           "runs with full access to your PC.")
    elif fmt in ("realm-script", "game-mode"):
        luas = [e for e in live if ext_of(e["path"]) == ".lua"]
        for e in luas:
            bad = lua_problems(e["text"])
            if bad:
                P.append(f"{e['path']} uses what {'game modes' if fmt == 'game-mode' else 'realm scripts'} may not: {', '.join(bad)}")
        if fmt == "game-mode":
            if luas[0]["size"] > MODE_MAX:
                P.append(f"{luas[0]['path']} is larger than 14 KB (the Modes limit)")
            m = re.search(r"\bMode\s*\.\s*Title\s*=\s*([\"'])(.{1,48}?)\1", luas[0]["text"])
            out["name"] = clean_name(m.group(2)) if m else ""
            out["plan"].append("A game mode: open it in game with /modes edit (the Modes editor) and paste it, or host it on "
                               "your realm. It is not installed into the game client.")
        else:
            out["plan"].append(f"{len(luas)} realm script file(s) for realm owners: they go into the realm's lua_scripts. "
                               "Nothing is installed into the game client.")
        out["client"] = False
    if P:
        return out
    if total > int(pol["big_mb"]) * 1048576:
        F.append(f"a big pack ({_mb(total)}): a moderator looks first")
    if BRAND_RE.search(out["name"]):
        out["name"] = clean_name(out["name"])
    out["name"] = out["name"] or clean_name(pin.get("repo_name", ""))
    out["notes"] += [f"{len(live) - len(files)} file(s) that the game does not load are left out (notes, screenshots, dev files)"] \
        if out["client"] and len(live) > len(files) else []
    if out["client"]:
        if not files:
            P.append("nothing in it would go into a game client")
            return out
        inst = {"kind": kind, "from": pin["from"], "url": pin["url"], "rev": pin["rev"], "sha256": pin.get("sha256"),
                "size": sum(sizes.values()),
                "file_count": len(files), "strip_prefix": prefix, "files": files, "files_digest": files_digest(files)}
        if kind == "addon":
            inst["folders"] = folders
            inst["blp"] = "author-art" if any(ext_of(p) == RULES["blp_ext"] for p in files) else "none"
        inst["native"] = any(ext_of(p) in NATIVE_CODE for p in files)
        out["install"] = inst
        rev = pin["rev"] if not HEX40.fullmatch(pin["rev"] or "") else pin["rev"][:7]
        what = (pin["url"].rsplit("/", 1)[-1] + " (release " + rev + ")") if pin["from"] == "release-asset" else f"commit {rev}"
        out["plan"].insert(0, f"Downloads {what} from the author's own repo ({_mb(inst['size'])}), checks every file's "
                              "fingerprint and installs only " + ("that file." if len(files) == 1 else f"those {len(files)} files."))
        out["plan"].append("My Mods can turn it off or remove it again; files you changed yourself are kept.")
    out["ok"] = True
    return out




# ------------------------------------------------------------------------------------------------ policy
def data_dir() -> str:
    return os.environ.get("WU_PORTAL_DATA", "/data")


def policy_path() -> str:
    return os.path.join(data_dir(), "listings-policy.json")


def policy() -> dict:
    """The owner's policy: what goes live without a moderator, after how many reports a listing hides itself."""
    try:
        with open(policy_path(), encoding="utf-8") as f:
            doc = json.load(f)
    except (OSError, ValueError):
        doc = {}
    return clean_policy(doc if isinstance(doc, dict) else {})


def clean_policy(doc: dict) -> dict:
    out = dict(DEFAULT_POLICY)
    if isinstance(doc.get("auto_live"), list):
        out["auto_live"] = [f for f in FORMATS if f in doc["auto_live"]]
    for k, lo, hi in (("report_hide", 1, 50), ("big_mb", 1, 2048), ("new_account_days", 0, 90), ("new_account_per_day", 0, 10)):
        v = doc.get(k)
        if isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi:
            out[k] = v
    return out


def set_policy(doc: dict) -> dict:
    pol = clean_policy(doc)
    os.makedirs(data_dir(), exist_ok=True)
    tmp = policy_path() + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(pol, f, indent=1)
    os.replace(tmp, policy_path())
    return pol


def verdict(result: dict, pol: dict, new_account: bool, word_rules: list[str]) -> tuple[str, list[str]]:
    """("live" | "waiting", why it waits)."""
    why = list(result["flags"]) + list(word_rules)
    if result["format"] not in pol["auto_live"]:
        why.append(f"{FORMAT_LABEL.get(result['format'], result['format']).lower()}: the owner has these checked first")
    if new_account:
        why.append("a new account: a moderator looks at its first listings")
    return ("waiting" if why else "live"), why


# ------------------------------------------------------------------------------------------------ the service
class Listings:
    """Checks (fetch, analyse, cache), the policy, and the record changes on top of a stream/projects.py store."""

    def __init__(self, projects=None, net: Net | None = None, now=time.time, accounts=None):
        self.projects = projects          # stream/projects.py Projects (or a callable returning it)
        self.net = net or Net()
        self.now = now
        self.accounts = accounts          # callable(acct) -> account created (unix) or None; social store
        self.lock = threading.Lock()
        self.checked: dict[str, tuple[float, dict]] = {}   # check token -> (expires, result), 30 minutes
        self.check_log: dict[str, list[float]] = {}
        self.day_log: list[float] = []
        self.secret = (os.environ.get("WU_LISTING_SECRET", "") or secrets.token_hex(16)).encode()
        self.per_hour = int(os.environ.get("WU_LISTING_CHECKS_HOUR", "10") or 10)
        self.per_day = int(os.environ.get("WU_LISTING_CHECKS_DAY", "400") or 400)
        self.fetch_max = int(os.environ.get("WU_LISTING_FETCH_MAX", str(300 * 1048576)) or 300 * 1048576)

    def store(self):
        p = self.projects
        return p() if callable(p) else p

    def is_new(self, acct: str) -> bool:
        days = policy()["new_account_days"]
        if not days or self.accounts is None:
            return False
        try:
            created = self.accounts(acct)
        except Exception:   # noqa: BLE001
            created = None
        return created is None or self.now() - float(created) < days * 86400

    # -------------------------------------------------- check
    def _allow_check(self, acct: str) -> str:
        now = self.now()
        with self.lock:
            mine = [t for t in self.check_log.get(acct, []) if t > now - 3600]
            self.day_log = [t for t in self.day_log if t > now - 86400]
            if len(mine) >= self.per_hour:
                return f"You can check {self.per_hour} links an hour. Please try again a bit later."
            if len(self.day_log) >= self.per_day:
                return "Lots of mods were checked today. Please try again tomorrow."
            self.check_log[acct] = mine + [now]
            self.day_log.append(now)
        return ""

    def check(self, acct: str, url: str) -> tuple[bool, dict | str]:
        """Pin, fetch and analyse a link. (True, result with "token") or (False, words for the player)."""
        if RULES is None:
            return False, "Listing mods is closed right now (the rules file is missing here)."
        if not acct:
            return False, "Sign in to list a mod."
        try:
            src = parse_source(url)
        except ValueError as e:
            return False, str(e)
        why = self._allow_check(acct)
        if why:
            return False, why
        try:
            pin = resolve(src, self.net)
        except ValueError as e:
            return False, str(e)
        result = self.fetch_and_analyse(src, pin)
        if isinstance(result, str):
            return False, result
        tok = hmac.new(self.secret, f"{acct}|{src['repo_url']}|{pin['rev']}|{secrets.token_hex(8)}".encode(),
                       hashlib.sha256).hexdigest()[:32]
        result["token"] = tok
        with self.lock:
            now = self.now()
            self.checked = {k: v for k, v in self.checked.items() if v[0] > now}
            if len(self.checked) > 500:
                self.checked.clear()
            self.checked[tok] = (now + 1800, dict(result, acct=acct))
        return True, result

    def fetch_and_analyse(self, src: dict, pin: dict) -> dict | str:
        cap = min(self.fetch_max, RULES["caps"]["download"] + MAX_PATCH)
        if isinstance(pin.get("size"), int) and pin["size"] > cap:
            return f"That download is {_mb(pin['size'])}: bigger than Leyline checks ({_mb(cap)})."
        work = tempfile.mkdtemp(prefix="wu-listing-")
        try:
            dest = os.path.join(work, "download")
            try:
                size, sha = self.net.download(pin["url"], dest, cap)
            except ValueError:
                return f"That download is bigger than Leyline checks ({_mb(cap)})."
            except (urllib.error.URLError, OSError) as e:
                log.info("listing download failed: %s", type(e).__name__)
                return "We couldn't download it from the author's repo just now. Please try again in a minute."
            if pin.get("digest") and pin["digest"] != sha:
                return "The release file changed while we read it. Please try again."
            # Release files are byte-stable: the launcher checks the whole file too. GitHub's commit archives are not.
            pin = dict(pin, sha256=sha if pin["from"] == "release-asset" else None)
            try:
                entries = scan(dest, pin.get("asset", ""))
            except (ValueError, zipfile.BadZipFile, OSError) as e:
                return "We couldn't read the download: " + str(e)[:200] + "."
        finally:
            shutil.rmtree(work, ignore_errors=True)
        meta = None
        st = self.store()
        if st is not None and getattr(st, "meta", None) is not None:
            try:
                meta = st.meta.fetch(src["forge"], src["owner"], src["repo"])
            except Exception:   # noqa: BLE001
                meta = None
        meta = meta if isinstance(meta, dict) else {}
        pin = dict(pin, repo_name=meta.get("title") or src["repo"])
        res = analyse(entries, pin, policy())
        res.update(source=src, pin={k: pin.get(k) for k in ("from", "url", "rev", "follow", "tag", "asset", "sha256")},
                   repo=src["repo_url"], licence=meta.get("licence", ""), repo_description=meta.get("description", ""),
                   author=res["author"] or meta.get("author") or src["owner"], checked_at=_iso(self.now()))
        if not res["licence"]:
            res["notes"].append("no licence in the repo: it installs from the author only and is never copied by us")
        return res

    def checked_result(self, acct: str, tok: str) -> dict | None:
        with self.lock:
            hit = self.checked.get(tok) if isinstance(tok, str) else None
        if not hit or hit[0] < self.now() or hit[1].get("acct") != acct:
            return None
        return hit[1]

    # -------------------------------------------------- submit
    def submit(self, acct: str, name: str, form: dict) -> tuple[bool, dict | str]:
        """List a checked mod. form: t (the check token), category, description, builds."""
        st = self.store()
        if st is None:
            return False, "Listing mods is closed right now."
        res = self.checked_result(acct, form.get("t", ""))
        if res is None:
            return False, "That check is out of date. Please check the link again."
        if not res["ok"]:
            return False, "That mod didn't pass the checks: " + "; ".join(res["problems"][:3])
        pol = policy()
        new = self.is_new(acct)
        if new:
            day = self.now() - 86400
            mine = [r for r in st.load() if r.get("submitter") == acct and _epoch(r.get("submitted_at")) > day]
            if len(mine) >= pol["new_account_per_day"]:
                return False, f"New accounts can list {pol['new_account_per_day']} mod a day. Please come back tomorrow."
        builds = form.get("builds") or ("3.3.5a" if res["format"] in ("addon", "package", "native", "patch") else "any")
        listing = listing_record(res)
        word = []
        try:
            word = [w for w in st_hub_problems(res) if w != "no open licence"]
        except Exception:   # noqa: BLE001
            word = []
        state, why = verdict(res, pol, new, word)
        listing["state"] = "community"
        me = {"id": acct, "display_name": name or "a player"}
        f = {"url": res["repo"], "category": form.get("category", ""), "builds": builds,
             "description": form.get("description", "")}
        ok, rec = st.submit(me, f, listing=listing, hold=why if state == "waiting" else [])
        if ok:
            log.info("listing %s: %s (%s) %s", rec["id"], res["format"], state, "; ".join(why)[:200])
        return ok, rec

    # -------------------------------------------------- updates
    def refresh_one(self, rec: dict) -> str:
        """Ask the author's repo again for a listing that follows a branch or the latest release. "same", "updated"
        (applied: it passed and the policy lets it go live), "pending" (waits in HQ), "failed" or "fixed" (pinned)."""
        lst = rec.get("listing")
        if not isinstance(lst, dict) or not lst.get("follow"):
            return "fixed"
        try:
            src = parse_source(lst.get("source_url") or rec.get("url", ""))
            if lst["follow"].startswith("branch:"):
                src.update(ref=lst["follow"][7:], tag="", asset="", latest=False, commit="")
            pin = resolve(src, self.net)
        except ValueError:
            return "failed"
        if pin["rev"] == (lst.get("install") or {}).get("rev") or pin["rev"] == lst.get("rev"):
            return "same"
        if (lst.get("pending_update") or {}).get("rev") == pin["rev"]:
            return "pending"
        res = self.fetch_and_analyse(src, pin)
        if isinstance(res, str):
            return "failed"
        st = self.store()
        pol = policy()
        new = listing_record(res)
        state, why = verdict(res, pol, False, [])
        if not res["ok"]:
            why = ["the new version does not pass: " + "; ".join(res["problems"][:3])]
            state = "waiting"
        if new.get("format") != lst.get("format"):
            why.append(f"its format changed ({lst.get('format')} -> {new.get('format')})")
            state = "waiting"
        old_files = (lst.get("install") or {}).get("files") or {}
        new_files = (new.get("install") or {}).get("files") or {}
        diff = {"added": len(set(new_files) - set(old_files)), "removed": len(set(old_files) - set(new_files)),
                "changed": sum(1 for p in set(old_files) & set(new_files) if old_files[p] != new_files[p])}
        if state == "live" and res["ok"]:
            st.update_listing(rec["id"], lambda r: _apply_update(r, new, diff))
            return "updated"
        new["why"], new["diff"], new["rev"] = why, diff, pin["rev"]
        st.update_listing(rec["id"], lambda r: r["listing"].__setitem__("pending_update", new))
        return "pending"

    def refresh(self, limit: int = 10) -> dict:
        st = self.store()
        counts: dict[str, int] = {}
        if st is None:
            return counts
        recs = sorted((r for r in st.load() if isinstance(r.get("listing"), dict) and r["listing"].get("follow")
                       and not r.get("hidden")), key=lambda r: r["listing"].get("checked_at", ""))[:limit]
        for r in recs:
            try:
                k = self.refresh_one(r)
            except Exception:   # noqa: BLE001: one odd repo never stops the rest
                log.exception("listing refresh %s", r.get("id"))
                k = "failed"
            counts[k] = counts.get(k, 0) + 1
            st.update_listing(r["id"], lambda x: x["listing"].__setitem__("checked_at", _iso(self.now())))
        return counts


def st_hub_problems(res: dict) -> list[str]:
    """The hub's word rules (stream/projects.py hub_problems: decompiled, leaked or repack material, cheats)."""
    import sys
    pm = sys.modules.get("wu_projects_listing")
    if pm is None:
        import importlib.util
        spec = importlib.util.spec_from_file_location("wu_projects_listing", os.path.join(HERE, "projects.py"))
        pm = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(pm)
        sys.modules["wu_projects_listing"] = pm
    return pm.hub_problems(res["repo"], [res["name"], res.get("author", ""), res.get("repo_description", "")],
                           res.get("licence", ""), False)


def listing_record(res: dict) -> dict:
    """What a projects.py record keeps of a check (all of it public except nothing: there is nothing private in it)."""
    return {"format": res["format"], "label": res["label"], "client": res["client"], "install": res["install"],
            "plan": res["plan"], "flags": res["flags"], "notes": res["notes"], "name": res["name"],
            "version": res["version"], "size": res["size"], "follow": res["pin"].get("follow", ""),
            "rev": res["pin"].get("rev", ""), "source_url": _source_url(res), "checked_at": res["checked_at"],
            "native": bool((res["install"] or {}).get("native"))}


def _source_url(res: dict) -> str:
    s, p = res["source"], res["pin"]
    if p.get("follow", "").startswith("branch:"):
        return f"{s['repo_url']}/tree/{p['follow'][7:]}" if s["forge"] == "github" else s["repo_url"]
    if p.get("tag"):
        return f"{s['repo_url']}/releases/tag/{p['tag']}"
    return s["repo_url"]


def _apply_update(r: dict, new: dict, diff: dict) -> None:
    keep = {k: r["listing"].get(k) for k in ("state", "source_url", "follow")}
    r["listing"] = dict(new, **{k: v for k, v in keep.items() if v}, updated_at=new["checked_at"], last_diff=diff)
    r["listing"].pop("pending_update", None)


def _iso(t: float) -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(t))


def _epoch(s) -> float:
    try:
        import calendar
        return float(calendar.timegm(time.strptime(str(s)[:19], "%Y-%m-%dT%H:%M:%S")))
    except (ValueError, TypeError, OverflowError):
        return 0.0


def all_json_fields(listing) -> dict:
    """The fields /wrath/mods/all.json carries for a listed mod (mods/all-json.md, "Listed mods"): source "listed"
    when the launcher can install it, the install block (without nothing: the file list is what it checks), format,
    state, flags and the plan in plain words."""
    if not isinstance(listing, dict):
        return {}
    inst = listing.get("install") if isinstance(listing.get("install"), dict) else None
    out = {"format": listing.get("format", ""), "format_label": listing.get("label", ""),
           "state": "reviewed" if listing.get("state") == "reviewed" else "community",
           "flags": list(listing.get("flags") or [])[:8], "plan": list(listing.get("plan") or [])[:8],
           "native": bool(listing.get("native")), "version": listing.get("version") or None,
           "size": (inst or {}).get("size")}
    if inst and listing.get("client"):
        out["source"] = "listed"
        out["install"] = inst
        out["file_count"] = inst.get("file_count")
        out["blp"] = inst.get("blp", "none")
    return out


# ------------------------------------------------------------------------------------------------ the portal glue
_get = lambda name: None   # noqa: E731
service: Listings | None = None


def bind(getter) -> None:
    global _get

    def get(name):
        try:
            return getter(name)
        except Exception:   # noqa: BLE001
            return None
    _get = get


def _projects_store():
    w = _get("workshop")
    return getattr(w, "projects", None) if w is not None else None


def _account_created(acct: str):
    soc = _get("social")
    st = getattr(soc, "store", None) if soc is not None else None
    if st is None:
        return None
    r = st.one("SELECT created FROM social_accounts WHERE acct = ?", (acct.upper(),))
    return r[0] if r else None


def setup() -> Listings:
    global service
    if service is None:
        service = Listings(_projects_store, accounts=_account_created)
        threading.Thread(target=_refresher, daemon=True).start()
        adm = _get("admin")
        if adm is not None and isinstance(getattr(adm, "ACTIONS", None), dict):   # Leyline HQ's buttons for listings
            adm.ACTIONS.update(hq_actions(adm))
    return service


def _refresher():
    while True:
        time.sleep(3600)
        try:
            if service is not None:
                service.refresh()
        except Exception:   # noqa: BLE001
            log.exception("listing refresh failed")


def match(path: str) -> bool:
    return path == PAGE or path == API or path.startswith(API + "/")


def _viewer(h):
    """(acct, name, how, session) or None. Raises OSError when the site's sign-in isn't answering."""
    soc = _get("social")
    if soc is None or getattr(soc, "store", None) is None:
        return None
    authz = h.headers.get("Authorization", "")
    if authz.startswith("Bearer "):
        tok = authz[7:].strip()
        acct = soc.store.token_acct(tok)
        if not acct:
            return None
        p = soc.store.profile(acct)
        return acct, (p or {}).get("name") or "a player", "token", tok
    who = soc.cookie_user(h)
    if who is None:
        return None
    sess = ""
    for part in h.headers.get("Cookie", "").split(";"):
        k, _, v = part.strip().partition("=")
        if k == "wr_session":
            sess = v
    return who[0], who[1], "cookie", sess


def _csrf(sess: str) -> str:
    return hmac.new(setup().secret, ("listing|" + sess).encode(), hashlib.sha256).hexdigest()[:32]


def _json(h, status: int, doc) -> None:
    h.send(status, json.dumps(doc, separators=(",", ":"), ensure_ascii=False).encode("utf-8"),
           "application/json; charset=utf-8", headers={"Cache-Control": "no-store"})


def _sanction(acct: str) -> str:
    adm = _get("admin")
    fn = getattr(adm, "blocked", None) if adm is not None else None
    try:
        return fn("submit", acct=acct) if fn else ""
    except Exception:   # noqa: BLE001
        return ""


def public_view(res: dict) -> dict:
    keys = ("ok", "format", "label", "client", "problems", "flags", "notes", "plan", "name", "version", "author", "size",
            "licence", "repo", "token")
    out = {k: res.get(k) for k in keys}
    out["pin"] = res.get("pin")
    out["files"] = (res.get("install") or {}).get("file_count", 0)
    return out


def mine(acct: str) -> list[dict]:
    st = _projects_store()
    if st is None:
        return []
    out = []
    for r in st.load():
        if r.get("submitter") != acct or not isinstance(r.get("listing"), dict):
            continue
        lst = r["listing"]
        state = ("taken down" if r.get("hidden") else "waiting for a moderator" if (r.get("review") or {}).get("state") == "pending"
                 else "reviewed" if lst.get("state") == "reviewed" else "live · community, not reviewed")
        out.append({"id": "link:" + r["id"], "pid": r["id"], "name": r.get("title", ""), "format": lst.get("format"),
                    "state": state, "why": list((r.get("review") or {}).get("reasons") or [])[:6],
                    "update": bool(lst.get("pending_update")), "repo": r.get("url", "")})
    return out


def do(action: str, acct: str, name: str, doc: dict) -> tuple[int, dict]:
    """One action for the JSON API and the page. (status, answer)."""
    svc = setup()
    st = _projects_store()
    if action in ("check", "submit") and _sanction(acct):
        return 403, {"error": _sanction(acct)}
    if action == "check":
        ok, res = svc.check(acct, str(doc.get("url", ""))[:400])
        return (200, {"ok": True, "check": public_view(res)}) if ok else (400, {"error": res})
    if action == "submit":
        form = {k: str(doc.get(k, ""))[:400] for k in ("t", "category", "description", "builds")}
        ok, res = svc.submit(acct, name, form)
        if not ok:
            return 400, {"error": str(res).partition("|")[0]}
        held = (res.get("review") or {}).get("state") == "pending"
        return 200, {"ok": True, "id": "link:" + res["id"], "pid": res["id"], "state": "waiting" if held else "live",
                     "why": (res.get("review") or {}).get("reasons", [])}
    if st is None:
        return 503, {"error": "Listing mods is closed right now."}
    pid = str(doc.get("id", "")).removeprefix("link:")
    if action == "report":
        ok, msg = st.report(pid, {"id": acct}, str(doc.get("reason", "")), str(doc.get("note", ""))[:300],
                            hide_after=policy()["report_hide"])
        return (200 if ok else 400), ({"ok": True, "message": msg} if ok else {"error": msg})
    if action == "edit":
        ok, msg = st.edit(pid, {"id": acct}, {k: str(doc.get(k, ""))[:400] for k in ("description", "category") if k in doc})
        return (200 if ok else 400), ({"ok": True, "message": msg} if ok else {"error": msg})
    if action == "remove":
        ok, msg = st.remove(pid, {"id": acct})
        return (200 if ok else 400), ({"ok": True, "message": msg} if ok else {"error": msg})
    return 404, {"error": "Unknown action."}


def _body(h) -> dict | None:
    try:
        n = int(h.headers.get("Content-Length", "0"))
    except ValueError:
        return None
    if n < 0 or n > 8192:
        return None
    raw = h.rfile.read(n) if n else b""
    if "json" in h.headers.get("Content-Type", ""):
        try:
            d = json.loads(raw.decode("utf-8") or "{}")
            return d if isinstance(d, dict) else None
        except (ValueError, UnicodeDecodeError):
            return None
    try:
        return {k: v[0] for k, v in urllib.parse.parse_qs(raw.decode("utf-8"), max_num_fields=20).items()}
    except (ValueError, UnicodeDecodeError):
        return None


def _limited(h) -> bool:
    lim = _get("page_limit")
    rl = _get("ratelimit")
    try:
        return lim is not None and rl is not None and not lim.hit(rl.bucket(h.ip()), 120)
    except Exception:   # noqa: BLE001
        return False


def get(h, path: str, query: dict) -> None:
    if _limited(h):
        return _json(h, 429, {"error": "Too many requests."})
    try:
        v = _viewer(h)
    except OSError:
        v = "down"
    if path == API + "/mine":
        if v in (None, "down"):
            return _json(h, 401 if v is None else 503, {"error": "Sign in first."})
        doc = {"ok": True, "listings": mine(v[0])}
        if v[2] == "cookie":
            doc["csrf"] = _csrf(v[3])
        return _json(h, 200, doc)
    if path == PAGE:
        return _page(h, v, query=query)
    _json(h, 404, {"error": "Unknown call."})


def post(h, path: str) -> None:
    if _limited(h):
        return _json(h, 429, {"error": "Too many requests."})
    doc = _body(h)
    try:
        v = _viewer(h)
    except OSError:
        v = "down"
    if path == PAGE:
        if doc is None:
            return _page(h, v, error="That form didn't arrive whole. Please try again.", status=400)
        if v in (None, "down") or v[2] != "cookie" or not hmac.compare_digest(str(doc.get("csrf", "")), _csrf(v[3])):
            return _page(h, v, error="That form was out of date. Please try again." if v not in (None, "down") else "", status=403)
        step = doc.get("step", "")
        st, ans = do({"check": "check", "submit": "submit", "report": "report", "edit": "edit", "remove": "remove"}.get(step, ""),
                     v[0], v[1], doc)
        return _page(h, v, form=doc, answer=ans, step=step, status=st)
    action = path[len(API):].strip("/")
    if doc is None:
        return _json(h, 400, {"error": "Send JSON."})
    if v == "down":
        return _json(h, 503, {"error": "The website isn't answering right now."})
    if v is None:
        return _json(h, 401, {"error": "signed_out"})
    if v[2] == "cookie":
        if h.headers.get("Sec-Fetch-Site", "") == "cross-site" or not hmac.compare_digest(h.headers.get("X-Leyline-CSRF", ""), _csrf(v[3])):
            return _json(h, 403, {"error": "Missing the form token."})
    st, ans = do(action, v[0], v[1], doc)
    _json(h, st, ans)


# ------------------------------------------------------------------------------------------------ the page
PAGE_CSS = """
.lm-steps{display:flex;gap:8px;margin:0 0 14px;font-size:13px;color:var(--ly-muted,#9aa)}
.lm-steps b{color:var(--ly-text,#fff)}
.lm-plan{margin:8px 0 0 18px;padding:0}.lm-plan li{margin:4px 0}
.lm-flag{border-left:3px solid #e0a43a;padding:6px 10px;margin:6px 0;background:rgba(224,164,58,.08)}
.lm-bad{border-left:3px solid #e05a4a;padding:6px 10px;margin:6px 0;background:rgba(224,90,74,.08)}
.lm-ok{border-left:3px solid #4ab07a;padding:6px 10px;margin:6px 0;background:rgba(74,176,122,.08)}
.lm-mine{list-style:none;padding:0}.lm-mine li{padding:8px 0;border-top:1px solid rgba(255,255,255,.08)}
.lm-mine form{display:inline}
"""


HOW_HTML = (
    '<details class="ly-fine" style="text-align:left"><summary>How listing works</summary>'
    '<p>Any signed-in player can list a mod: an addon, a Leyline mod package, a texture or client patch (.MPQ), a native '
    'mod (.dll, .asi), a realm script or a game mode. We read your repo once, pin the exact version, and record a '
    'fingerprint of every file, so players get exactly what was checked. Nothing is copied to our site: players install '
    'from your repo.</p><p>Addons, packages, realm scripts and game modes that raise no flag go live at once as '
    '"Community · not reviewed". Native code, client patches, textures, big packs and new accounts wait for a moderator. '
    'Game files (models, maps, databases), programs inside addons and zips inside zips are never allowed.</p>'
    '<p>When you push to the branch (or make a new release), we check it again: a clean update goes out by itself, a '
    'flagged one waits for a moderator while the old version stays. Players rate and report listings; enough reports hide '
    'one until a moderator looks. You can change or remove your listing below at any time.</p></details>')


def _page(h, v, form=None, answer=None, step="", error="", status=200, query=None) -> None:
    e = html.escape
    form, answer = form or {}, answer or {}
    if v == "down":
        inner = '<p class="wk-msg" role="alert">The website isn\'t answering right now. Please try again shortly.</p>'
        status = 503
    elif v is None:
        inner = ('<p>Sign in to list your mod. Any player can list one: paste your repo link, we check it and show '
                 'exactly what one click will install.</p><p class="wk-actions"><a class="wk-btn violet" '
                 'href="/login?next=%2Fwrath%2Fmods%2Flist">Sign in</a></p>')
    else:
        csrf = _csrf(v[3])
        hid = f'<input type="hidden" name="csrf" value="{e(csrf)}">'
        msg = ""
        if error:
            msg = f'<p class="wk-msg" role="alert">{e(error)}</p>'
        elif answer.get("error"):
            msg = f'<p class="wk-msg" role="alert">{e(answer["error"])}</p>'
        elif answer.get("message"):
            msg = f'<p class="wk-msg" role="status">{e(answer["message"])}</p>'
        body = ""
        chk = answer.get("check") if step == "check" else None
        if step == "submit" and answer.get("ok"):
            slug = e(answer["pid"])
            if answer["state"] == "live":
                body = (f'<p class="lm-ok">Listed. It shows as <b>Community · not reviewed</b>, and players can install it '
                        f'in the Leyline launcher in one click. <a href="/wrath/mods/p/{slug}">Open its page</a></p>')
            else:
                body = ('<p class="lm-flag">Thanks: a moderator looks at it first (' + e("; ".join(answer.get("why") or [])[:300])
                        + '). It goes live as soon as they say yes.</p>')
        elif chk:
            body = _preview_html(chk, hid, form)
        if not body:
            body = (f'<form class="wk-form" method="post" action="{PAGE}">{hid}<input type="hidden" name="step" value="check">'
                    '<label>Your mod\'s link <span class="hint">Your repo on GitHub, GitLab or Codeberg, a branch, a release, or '
                    'one release file (.zip or .MPQ).</span><input type="url" name="url" required maxlength="400" '
                    f'placeholder="https://github.com/you/your-mod" value="{e(str(form.get("url", "")))}"></label>'
                    '<p class="ly-fine" style="text-align:left;margin:0">We download it once to check it, then delete it. '
                    'Nothing is hosted here: players install it from your repo.</p>'
                    '<p class="wk-actions" style="margin:0"><button class="wk-btn violet" type="submit">Check it</button></p></form>')
        rows = []
        for m in mine(v[0]):
            pid = e(m["pid"])
            rows.append(f'<li><b><a href="/wrath/mods/p/{pid}">{e(m["name"])}</a></b> · {e(FORMAT_LABEL.get(m["format"], ""))} · '
                        f'{e(m["state"])}' + (" · an update waits for a moderator" if m["update"] else "")
                        + (f'<br><span class="ly-fine">{e("; ".join(m["why"]))}</span>' if m["why"] else "")
                        + f'<br><form method="post" action="{PAGE}">{hid}<input type="hidden" name="step" value="edit">'
                        f'<input type="hidden" name="id" value="{pid}"><input type="text" name="description" maxlength="140" '
                        f'minlength="10" placeholder="New one-line description" aria-label="New description" required>'
                        '<button type="submit" class="wk-btn quiet">Save</button></form> '
                        f'<form method="post" action="{PAGE}">{hid}<input type="hidden" name="step" value="remove">'
                        f'<input type="hidden" name="id" value="{pid}"><button type="submit" class="wk-btn quiet">Remove</button></form></li>')
        mine_html = (f'<section class="ly-panel"><h2>Your listings</h2><ul class="lm-mine">{"".join(rows)}</ul></section>'
                     if rows else "")
        inner = msg + body + mine_html
    page_body = ('<p><a href="/wrath/mods">&larr; All mods</a></p><style>' + PAGE_CSS + '</style>'
                 '<section class="ly-panel" aria-labelledby="lm-h"><h2 id="lm-h">List your mod</h2>'
                 '<p class="lm-steps"><span><b>1</b> Paste the link</span><span><b>2</b> See what one click installs</span>'
                 '<span><b>3</b> List it</span></p>' + inner + HOW_HTML + '</section>')
    w = _get("workshop")
    if w is not None and hasattr(w, "_page"):
        body = w._page("List your mod · Workshop", PAGE, "List your mod · Leyline Workshop", page_body,
                       "List your mod for one-click install in the Leyline launcher.")
    else:
        body = ("<!doctype html><meta charset=utf-8><title>List your mod</title>" + page_body).encode("utf-8")
    h.send(status, body)


def _preview_html(c: dict, hid: str, form: dict) -> str:
    e = html.escape
    out = [f'<p><b>{e(c.get("name") or c.get("repo") or "")}</b> · {e(c.get("label") or "")}'
           + (f' · v{e(c["version"])}' if c.get("version") else "") + (f' · {e(c["licence"])}' if c.get("licence") else "")
           + "</p>"]
    for p in c.get("problems") or []:
        out.append(f'<p class="lm-bad">{e(p)}</p>')
    if not c.get("ok"):
        out.append('<p>Fix this in your repo and check again (see "How listing works" below).</p>')
        return "".join(out) + (f'<form method="post" action="{PAGE}">{hid}<input type="hidden" name="step" value="check">'
                               f'<input type="hidden" name="url" value="{e(str(form.get("url", "")))}">'
                               '<button class="wk-btn" type="submit">Check again</button></form>')
    out.append('<p><b>What one click does</b></p><ul class="lm-plan">' + "".join(f"<li>{e(x)}</li>" for x in c.get("plan") or []) + "</ul>")
    for f in c.get("flags") or []:
        out.append(f'<p class="lm-flag">{e(f)}</p>')
    for n in c.get("notes") or []:
        out.append(f'<p class="ly-fine" style="text-align:left">{e(n)}</p>')
    w = _get("workshop")
    cats = []
    try:
        cats = w.categories()[0] if w is not None else []
    except Exception:   # noqa: BLE001
        cats = []
    default = {"addon": "addons", "package": "addons", "patch": "graphics", "native": "tools", "realm-script": "realm-scripts",
               "game-mode": "sandbox"}.get(c.get("format"), "addons")
    opts = "".join(f'<option value="{e(k["id"])}"{" selected" if k["id"] == default else ""}>{e(k["name"])}</option>' for k in cats) \
        or f'<option value="{default}">{default}</option>'
    out.append(f'<form class="wk-form" method="post" action="{PAGE}">{hid}<input type="hidden" name="step" value="submit">'
               f'<input type="hidden" name="t" value="{e(c.get("token") or "")}"><input type="hidden" name="url" value="{e(str(form.get("url", "")))}">'
               f'<label>Category<select name="category" required>{opts}</select></label>'
               '<label>One-line description <span class="hint">What it does, in up to 140 characters.</span>'
               f'<input type="text" name="description" required minlength="10" maxlength="140" value="{e(str(form.get("description", "")))}"></label>'
               '<p class="wk-actions" style="margin:0"><button class="wk-btn violet" type="submit">List it</button></p></form>')
    return "".join(out)


# ------------------------------------------------------------------------------------------------ Leyline HQ
def hq_items(A, item) -> list[dict]:
    """Needs you: listings waiting for a moderator (flagged, new accounts, hidden by reports) and updates waiting."""
    st = _projects_store()
    if st is None:
        return []
    out = []
    for r in st.load():
        lst = r.get("listing")
        if not isinstance(lst, dict) or r.get("hidden"):
            continue
        pid = r["id"]
        rv = r.get("review") if isinstance(r.get("review"), dict) else {}
        reps = r.get("report_log") if isinstance(r.get("report_log"), list) else []
        why_reps = ", ".join(sorted({REPORT_REASONS.get(x.get("reason"), "other") for x in reps if isinstance(x, dict)}))
        plan = " ".join(lst.get("plan") or [])[:500]
        if rv.get("state") == "pending":
            label = "Listing: hidden by reports" if rv.get("by_reports") else "Listing: waiting"
            out.append(item("listing", "listing:" + pid, label, str(r.get("title") or pid)[:80],
                            plan, r.get("submitted_at"), "/wrath/mods/p/" + pid,
                            [("listing.approve", pid, "Approve (goes live)", {}, None, False),
                             ("listing.reject", pid, "Take down", {"reason": ("text", "why (required; the author reads it)")}, pid, True)],
                            sub=f"{FORMAT_LABEL.get(lst.get('format'), '')} · {'; '.join(rv.get('reasons') or [])[:200]}"
                                + (f" · reports: {why_reps}" if why_reps else "") + f" · {r.get('url', '')}",
                            group=0 if rv.get("by_reports") else 1))
        upd = lst.get("pending_update")
        if isinstance(upd, dict):
            d = upd.get("diff") or {}
            out.append(item("listing", "listing-up:" + pid, "Listing: update waiting", str(r.get("title") or pid)[:80],
                            " ".join(upd.get("plan") or [])[:500], upd.get("checked_at"), "/wrath/mods/p/" + pid,
                            [("listing.update.approve", pid, "Approve the update", {}, None, False),
                             ("listing.update.reject", pid, "Keep the old one", {}, None, False)],
                            sub=f"+{d.get('added', 0)} -{d.get('removed', 0)} ~{d.get('changed', 0)} files · "
                                + "; ".join(upd.get("why") or [])[:200]))
    return out


def hq_actions(A) -> dict:
    def known(target):
        st = _projects_store()
        if st is None:
            raise A.Refused("Community links are not set up here.")
        r = next((x for x in st.load() if x.get("id") == target), None)
        if r is None or not isinstance(r.get("listing"), dict):
            raise A.Refused("No listing by that id.")
        return st, r

    def approve(user, target, form):
        st, r = known(target)
        st.update_listing(target, lambda x: (x.pop("review", None), x.__setitem__("reports", []),
                                             x.__setitem__("report_log", []),
                                             x["listing"].__setitem__("state", "reviewed"),
                                             x["listing"].__setitem__("reviewed_by", str(user.get("account", ""))[:40])))
        return {"state": "waiting"}, {"state": "reviewed"}

    def reject(user, target, form):
        st, r = known(target)
        why = " ".join(str(form.get("reason") or "").split())[:200]
        if not why:
            raise A.Refused("Say why: the author reads it.")
        st.update_listing(target, lambda x: (x.__setitem__("hidden", True), x.__setitem__("hidden_by", str(user.get("account", ""))[:40]),
                                             x.__setitem__("hidden_at", _iso(time.time())),
                                             x.__setitem__("review", {"state": "rejected", "reasons": [why]})))
        return {"hidden": False}, {"hidden": True, "reason": why}

    def up_approve(user, target, form):
        st, r = known(target)
        upd = r["listing"].get("pending_update")
        if not isinstance(upd, dict):
            raise A.Refused("No update is waiting.")
        st.update_listing(target, lambda x: _apply_update(x, {k: v for k, v in upd.items() if k not in ("why", "diff")}, upd.get("diff") or {}))
        return {"rev": r["listing"].get("rev")}, {"rev": upd.get("rev")}

    def up_reject(user, target, form):
        st, r = known(target)
        st.update_listing(target, lambda x: x["listing"].pop("pending_update", None))
        return {}, {"update": "kept the old one"}

    def set_pol(user, target, form):
        auto = [f for f in FORMATS if str(form.get("auto_" + f, "")) in ("1", "on", "yes")]
        doc = {"auto_live": auto}
        for k in ("report_hide", "big_mb", "new_account_days", "new_account_per_day"):
            try:
                doc[k] = int(str(form.get(k, "")).strip())
            except ValueError:
                pass
        before = policy()
        return before, set_policy(doc)

    return {"listing.approve": (approve, "mod", None), "listing.reject": (reject, "mod", True),
            "listing.update.approve": (up_approve, "mod", None), "listing.update.reject": (up_reject, "mod", None),
            "listing.policy": (set_pol, "owner", None)}


def hq_policy_html(user, A, back: str = "/wrath/admin/inbox") -> str:
    """The owner's switch for what goes live without a moderator (HQ, under Needs you)."""
    if not A.can(user, "owner"):
        return ""
    e = html.escape
    pol = policy()
    boxes = "".join(f'<label style="margin-right:12px"><input type="checkbox" name="auto_{f}" value="1"'
                    f'{" checked" if f in pol["auto_live"] else ""}> {e(FORMAT_LABEL[f])}</label>' for f in FORMATS)
    nums = "".join(f'<label style="margin-right:12px">{e(lbl)} <input type="number" name="{k}" value="{pol[k]}" min="0" max="2048" '
                   f'style="width:5em"></label>' for k, lbl in (("report_hide", "Hide after this many reports"),
                                                                ("big_mb", "Big pack (MB)"),
                                                                ("new_account_days", "New account (days)"),
                                                                ("new_account_per_day", "New account listings a day")))
    form = A.act(user, "listing.policy", "policy", "Save", back, boxes + "<br>" + nums, block=True)
    return ('<section class="ly-panel"><h2>Listed mods: what goes live at once</h2><p class="fine">Ticked kinds go live as '
            '"Community · not reviewed" when they raise no flag. Flags (native code, patches, textures, big packs), new '
            'accounts and the word rules always wait here.</p>' + form + '</section>')
