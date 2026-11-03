#!/usr/bin/env python3
"""Fork readiness check: is this copy of Leyline ready to run under its own name, keys and servers? (docs/TOOLKIT.md,
docs/SUCCESSION.md, docs/FORKING.md). Read-only: it never writes a file, and it fetches only with --online.

    python3 -I tools/fork-kit/fork_kit.py check [--root REPO] [--branding FILE] [--online] [--chain FILE] [--json]
                                                [--path EXTRA ...]

What it checks, one line each in a checklist (OK, WARN, FAIL, TODO, INFO):
  * branding.json: the same rules rebrand.py applies (launcher/scripts/rebrand.py check()), and its logo files exist;
  * identity: a fork (another name or repo than Leyline's) must have its own app identifier and its own updater key
    (tauri.conf.json plugins.updater.pubkey; Leyline's key id is known here), and no Leyline addresses left in the
    launcher's code and UI (rebrand.py not run, or run on an old copy);
  * brand words: the game's and its maker's names in player-facing files, by the public export's own checker
    (tools/leyline-repo/export.py: the files it publishes and its user_facing()/brand_hits()), plus every file in
    launcher/ui (the pages and their scripts) and any --path given. The one allowed line is the legal
    "not affiliated with or endorsed by ..." disclaimer;
  * update sources: GitHub plus at least one mirror, spare updater keys (updates.backup_pubkeys) set and well formed;
  * the release log: release_log.public_key set (64 hex), the host kit's copy (hosting/kit/tools/wunet/release-log.json)
    the same, at least two places the chain is published; with --chain FILE, that chain verified against the key;
  * relays: at least two (one is a single point of failure);
  * Hero Key: whether the host kit pins a website key (wunet/herokey.py SITE_KEYS);
  * --online: every update source, mirror manifest, release-log address, relay (/healthz) and the flags file answered.

Exit codes: 0 ready or ready with warnings, 1 something FAILed, 2 usage. Never prints a secret: it reads public keys
and addresses only. Standard library only.
"""
from __future__ import annotations

import argparse
import base64
import importlib.util
import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFAULT_ROOT = HERE.parents[1]

# Leyline's own identity, so a fork can be told apart from Leyline and told what it still shares with it.
UPSTREAM = {
    "name": "Leyline",
    "identifier": "app.openvanilla.leyline",
    "repo": "orionopenwow/Leyline",
    "updater_key_id": "F084E46520163B96",
    "hosts": ("leylinelauncher.com", "openvanilla.app", "orionopenwow"),
}
LEGAL = re.compile(r"not affiliated with or endorsed by [A-Z][a-z]+ Entertainment", re.I)
HEX64 = re.compile(r"^[0-9a-f]{64}$")
UI_EXTS = (".html", ".htm", ".js", ".json", ".md", ".txt")
SKIP = {"node_modules", "target", "tests", "__pycache__", ".git", "gen"}

OK, WARN, FAIL, TODO, INFO = "OK", "WARN", "FAIL", "TODO", "INFO"
ORDER = {FAIL: 0, TODO: 1, WARN: 2, OK: 3, INFO: 4}


def item(status: str, area: str, text: str, fix: str = "") -> dict:
    return {"status": status, "area": area, "text": text, "fix": fix}


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def read_json(path: Path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def minisign_key_id(pub_b64: str) -> str | None:
    """The key id of a Tauri updater public key (base64 of minisign's two-line .pub file), or None if malformed."""
    try:
        text = base64.b64decode(pub_b64, validate=True).decode("utf-8")
    except (ValueError, UnicodeDecodeError):
        return None
    m = re.match(r"untrusted comment: minisign public key:? ([0-9A-F]{16})\s*\n(\S+)", text)
    if not m:
        return None
    try:
        raw = base64.b64decode(m.group(2), validate=True)
    except ValueError:
        return None
    return m.group(1) if len(raw) == 42 else None


def is_fork(b: dict) -> bool:
    return b.get("name") != UPSTREAM["name"] or b.get("repo") != UPSTREAM["repo"]


# ------------------------------------------------------------------------------------------------- the checks
def check_branding(root: Path, b: dict) -> list[dict]:
    out = []
    rebrand = root / "launcher" / "scripts" / "rebrand.py"
    if rebrand.is_file():
        rb = load("fk_rebrand", rebrand)
        try:
            rb.check(b)
            out.append(item(OK, "branding", "branding.json is valid (rebrand.py's rules)"))
        except rb.Refused as e:
            out.append(item(FAIL, "branding", f"branding.json: {e}", "fix it, then run launcher/scripts/rebrand.py"))
    else:
        out.append(item(FAIL, "branding", "launcher/scripts/rebrand.py is missing", "restore it from Leyline's source"))
    for k in ("logo", "emblem"):
        rel = b.get(k)
        if not isinstance(rel, str) or not (root / "launcher" / rel).is_file():
            out.append(item(FAIL, "branding", f"{k}: launcher/{rel} not found", "put your picture there"))
    return out


def check_identity(root: Path, b: dict, tauri: dict) -> list[dict]:
    fork = is_fork(b)
    pub = ((tauri.get("plugins") or {}).get("updater") or {}).get("pubkey", "")
    kid = minisign_key_id(pub) if pub else None
    out = []
    if not pub:
        out.append(item(FAIL, "identity", "tauri.conf.json has no updater public key (plugins.updater.pubkey)",
                        "npx @tauri-apps/cli signer generate -w my-updater.key; put the public half there"))
    elif not kid:
        out.append(item(FAIL, "identity", "tauri.conf.json's updater public key is not a minisign public key"))
    if not fork:
        out.append(item(INFO, "identity", f"this is {UPSTREAM['name']} itself ({UPSTREAM['repo']}): the fork checks "
                                          "below apply once name or repo change"))
        if kid:
            out.append(item(OK, "identity", f"updater key {kid}"))
        return out
    out.append(item(INFO, "identity", f"a fork: {b.get('name')} ({b.get('repo')})"))
    if b.get("identifier") == UPSTREAM["identifier"]:
        out.append(item(FAIL, "identity", "the app identifier is still Leyline's: the fork would install over Leyline",
                        "set branding.json identifier (org.example.app) and run rebrand.py"))
    else:
        out.append(item(OK, "identity", f"own app identifier {b.get('identifier')}"))
    if kid == UPSTREAM["updater_key_id"]:
        out.append(item(FAIL, "identity", "the updater key is still Leyline's: the fork can't sign releases its "
                                          "launchers would install", "make your own (docs/FORKING.md, 'Your own keys')"))
    elif kid:
        out.append(item(OK, "identity", f"own updater key {kid}"))
    left = leftovers(root)
    if left:
        shown = ", ".join(f"{p} ({n})" for p, n in left[:5])
        out.append(item(WARN, "identity", f"Leyline's addresses still in {len(left)} launcher file(s): {shown}",
                        "run launcher/scripts/rebrand.py with your branding.json; search launcher/ui for the rest"))
    else:
        out.append(item(OK, "identity", "no Leyline addresses left in the launcher's code and UI"))
    return out


def leftovers(root: Path) -> list[tuple[str, int]]:
    """Launcher files (Rust, UI, tauri.conf.json) that still name Leyline's hosts or GitHub owner: (path, count)."""
    rx = re.compile("|".join(re.escape(h) for h in UPSTREAM["hosts"]))
    lau = root / "launcher"
    paths = [lau / "src-tauri" / "tauri.conf.json"]
    for base, exts in ((lau / "src-tauri" / "src", (".rs",)), (lau / "ui", (".js", ".html", ".css"))):
        for d, dirs, names in os.walk(base):
            dirs[:] = sorted(x for x in dirs if x not in SKIP)
            paths += [Path(d) / n for n in sorted(names) if n.endswith(exts)]
    out = []
    for p in paths:
        try:
            n = len(rx.findall(p.read_text(encoding="utf-8", errors="replace")))
        except OSError:
            continue
        if n:
            out.append((p.relative_to(root).as_posix(), n))
    return out


def brand_files(root: Path, extra: list[str]):
    """{rel: bytes} of the player-facing files: what the public export publishes and calls user-facing, every file in
    launcher/ui, and the extra paths. Plus notes (the export's allow-list not matching this tree)."""
    notes, files = [], {}
    export_py = root / "tools" / "leyline-repo" / "export.py"
    ex = None
    if export_py.is_file():
        ex = load("fk_export", export_py)
        try:
            planned, _refused, _skipped = ex.plan(root)
            files.update({rel: data for rel, (data, _x) in planned.items() if ex.user_facing(rel)})
        except ex.Refused as e:
            notes.append(f"the public export's allow-list doesn't match this tree ({e})")
    else:
        notes.append("tools/leyline-repo/export.py not found: its brand checker is not used")

    def add_tree(base: Path, exts):
        if base.is_file():
            files[base.relative_to(root).as_posix()] = base.read_bytes()
            return
        for d, dirs, names in os.walk(base):
            dirs[:] = sorted(x for x in dirs if x not in SKIP)
            for n in sorted(names):
                if exts is None or n.lower().endswith(exts):
                    p = Path(d) / n
                    files[p.relative_to(root).as_posix()] = p.read_bytes()
    add_tree(root / "launcher" / "ui", UI_EXTS)
    for e in extra:
        p = (root / e) if not os.path.isabs(e) else Path(e)
        if p.exists():
            add_tree(p, UI_EXTS)
        else:
            notes.append(f"--path {e} not found")
    return files, notes, ex


def brand_hits(files: dict, ex) -> list[str]:
    banned = ex.BANNED if ex is not None else [re.compile(r"world\s+of\s+warcraft", re.I), re.compile(r"warcraft", re.I),
                                             re.compile(r"lich\s+king", re.I), re.compile(r"blizzard", re.I),
                                             re.compile(r"\bWoW\b")]
    hits = []
    for rel in sorted(files):
        for n, line in enumerate(files[rel].decode("utf-8", "replace").splitlines(), 1):
            line = LEGAL.sub(" ", line)
            for rx in banned:
                m = rx.search(line)
                if m:
                    hits.append(f"{rel}:{n}: \"{m.group(0)}\"")
                    break
    return hits


def check_brand_words(root: Path, extra: list[str]) -> list[dict]:
    files, notes, ex = brand_files(root, extra)
    out = [item(WARN, "brand", n) for n in notes]
    hits = brand_hits(files, ex)
    if hits:
        shown = "; ".join(hits[:8]) + (f"; and {len(hits) - 8} more" if len(hits) > 8 else "")
        out.append(item(FAIL, "brand", f"brand words in {len(hits)} player-facing line(s): {shown}",
                        "players never see the game's or its maker's names (only the legal disclaimer)"))
    else:
        out.append(item(OK, "brand", f"no brand words in {len(files)} player-facing files"))
    return out


def check_updates(b: dict) -> list[dict]:
    u = b.get("updates") or {}
    out = []
    mirrors = [m for m in (u.get("mirrors") or []) if isinstance(m, str)]
    if u.get("github_api"):
        out.append(item(OK, "updates", "GitHub releases are an update source"))
    if mirrors:
        out.append(item(OK, "updates", f"{len(mirrors)} mirror(s) besides GitHub: {', '.join(mirrors)}"))
    else:
        out.append(item(WARN, "updates", "no mirror: launchers stop updating when GitHub does",
                        "add updates.mirrors (any static web server; tools/release-mirror fills it)"))
    spares = u.get("backup_pubkeys") or []
    bad = [k for k in spares if not isinstance(k, str) or not minisign_key_id(k)]
    if bad:
        out.append(item(FAIL, "updates", f"{len(bad)} spare updater key(s) are not minisign public keys"))
    elif spares:
        out.append(item(OK, "updates", f"{len(spares)} spare updater key(s): "
                                       + ", ".join(minisign_key_id(k) for k in spares)))
    else:
        out.append(item(TODO, "updates", "no spare updater key: if the one signing key is lost, no installed launcher "
                                         "can be updated again", "docs/RESILIENCE.md, 'What you must do' step 2"))
    return out


def check_release_log(root: Path, b: dict, chain: str | None) -> list[dict]:
    rl = b.get("release_log") or {}
    key = rl.get("public_key", "")
    urls = rl.get("urls") or []
    out = []
    if not key:
        out.append(item(TODO, "release log", "no release-log public key: the signed release log is dormant and "
                                             "nobody can check a mirror's files against it",
                        "docs/RESILIENCE.md, 'What you must do' step 1 (keygen prints the public half only)"))
    elif not HEX64.match(key):
        out.append(item(FAIL, "release log", "release_log.public_key is not 64 lower-case hex digits"))
    else:
        out.append(item(OK, "release log", f"release-log key {key[:16]}..."))
    kit = root / "hosting" / "kit" / "tools" / "wunet" / "release-log.json"
    if kit.is_file():
        try:
            kit_key = read_json(kit).get("public_key", "")
        except (ValueError, OSError):
            kit_key = None
        if kit_key != key:
            out.append(item(FAIL, "release log", "the host kit's release-log key differs from branding.json's",
                            "run launcher/scripts/rebrand.py (it writes both)"))
    if len(urls) >= 2:
        out.append(item(OK, "release log", f"the chain is published in {len(urls)} places"))
    else:
        out.append(item(WARN, "release log", f"the chain is published in {len(urls)} place(s): use two or more"))
    if chain:
        out += check_chain(root, chain, key)
    return out


def check_chain(root: Path, chain: str, key: str) -> list[dict]:
    tools = root / "hosting" / "kit" / "tools"
    if str(tools) not in sys.path:
        sys.path.insert(0, str(tools))
    try:
        from wunet import releaselog
    except ImportError as e:
        return [item(FAIL, "release log", f"can't load wunet.releaselog to check {chain}: {e}")]
    try:
        doc = read_json(Path(chain))
        entries = releaselog.verify(doc, key or doc.get("key", ""))
    except Exception as e:     # verify raises ValueError on any broken entry; a bad file is the same answer
        return [item(FAIL, "release log", f"{chain} does not verify: {e}")]
    note = "" if key else " (against its own key: branding.json has none to pin)"
    return [item(OK if key else WARN, "release log", f"{chain}: {len(entries)} entries verify{note}")]


def check_relays(b: dict) -> list[dict]:
    relays = b.get("relays") or []
    hosts = {re.sub(r"^https://([^/]+).*$", r"\1", r) for r in relays}
    roots = {".".join(h.split(".")[-2:]) for h in hosts}
    if len(relays) >= 2 and len(roots) >= 2:
        return [item(OK, "relays", f"{len(relays)} seed relays on {len(roots)} domains (make sure they are separate "
                                   "servers, not two names for one)")]
    if len(relays) >= 2:
        return [item(WARN, "relays", f"{len(relays)} seed relays, all under {', '.join(sorted(roots))}: one domain or "
                                     "one server going away takes them all", "add a community relay on another domain")]
    return [item(WARN, "relays", "one seed relay: a single point of failure", "add a second relay")]


def check_herokey(root: Path, b: dict) -> list[dict]:
    p = root / "hosting" / "kit" / "tools" / "wunet" / "herokey.py"
    if not p.is_file():
        return [item(WARN, "hero key", "hosting/kit/tools/wunet/herokey.py not found")]
    m = re.search(r"^SITE_KEYS\s*:\s*\w+\s*=\s*(\(.*?\)|\[.*?\])", p.read_text(encoding="utf-8"), re.M | re.S)
    keys = re.findall(r"[0-9a-f]{64}", m.group(1)) if m else []
    if not keys:
        return [item(INFO, "hero key", "the host kit pins no website key: Hero Key sign-in on community realms is off "
                                       "(docs/HERO-KEY-SSO.md); hosts can pin one with setup.py herokey pin")]
    if is_fork(b):
        return [item(WARN, "hero key", f"the host kit pins {len(keys)} website key(s): make sure they are your site's, "
                                       "not Leyline's", "wunet/herokey.py SITE_KEYS")]
    return [item(OK, "hero key", f"the host kit pins {len(keys)} website key(s)")]


def default_fetch(url: str, timeout: float = 10.0) -> int:
    req = urllib.request.Request(url, headers={"User-Agent": "leyline-fork-kit/1"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            r.read(1)
            return r.status
    except urllib.error.HTTPError as e:
        return e.code


def online_targets(b: dict) -> list[tuple[str, str]]:
    u = b.get("updates") or {}
    t = []
    if u.get("github_api"):
        t.append(("updates", u["github_api"]))
    t += [("mirror", m + "latest.json") for m in (u.get("mirrors") or [])]
    t += [("release log", x) for x in ((b.get("release_log") or {}).get("urls") or [])]
    t += [("relay", r.rstrip("/") + "/healthz") for r in (b.get("relays") or [])]
    if b.get("flags_url"):
        t.append(("flags", b["flags_url"]))
    return t


def check_online(b: dict, fetch) -> list[dict]:
    out = []
    has_key = bool((b.get("release_log") or {}).get("public_key"))
    for area, url in online_targets(b):
        try:
            code = fetch(url)
        except Exception as e:    # DNS, TLS, timeout: all "did not answer"
            why = getattr(e, "reason", None) or type(e).__name__
            out.append(item(WARN, area, f"{url} did not answer ({str(why)[:80]})"))
            continue
        if 200 <= code < 400:
            out.append(item(OK, area, f"{url} answered {code}"))
        elif area == "release log" and code == 404 and not has_key:
            out.append(item(INFO, area, f"{url} is 404 (expected while there is no release key)"))
        else:
            out.append(item(WARN, area, f"{url} answered {code}"))
    return out


# ------------------------------------------------------------------------------------------------------ driver
def run(root: Path, branding: Path | None = None, online: bool = False, fetch=None, chain: str | None = None,
        extra: list[str] | None = None) -> list[dict]:
    root = Path(root)
    bpath = Path(branding) if branding else root / "launcher" / "branding.json"
    try:
        b = read_json(bpath)
    except (OSError, ValueError) as e:
        return [item(FAIL, "branding", f"can't read {bpath}: {e}")]
    if not isinstance(b, dict):
        return [item(FAIL, "branding", f"{bpath} is not a JSON object")]
    try:
        tauri = read_json(root / "launcher" / "src-tauri" / "tauri.conf.json")
    except (OSError, ValueError):
        tauri = {}
    items = check_branding(root, b)
    items += check_identity(root, b, tauri)
    items += check_brand_words(root, extra or [])
    items += check_updates(b)
    items += check_release_log(root, b, chain)
    items += check_relays(b)
    items += check_herokey(root, b)
    if online:
        items += check_online(b, fetch or default_fetch)
    else:
        items.append(item(INFO, "online", "addresses not fetched (add --online)"))
    return items


def render(items: list[dict]) -> str:
    lines = ["Fork readiness checklist (tools/fork-kit, docs/TOOLKIT.md)", ""]
    for it in items:
        lines.append(f"[{it['status']:<4}] {it['area']}: {it['text']}")
        if it["fix"] and it["status"] in (FAIL, TODO, WARN):
            lines.append(f"         -> {it['fix']}")
    n = {s: sum(1 for i in items if i["status"] == s) for s in ORDER}
    lines += ["", f"{n[OK]} ok, {n[WARN]} warnings, {n[TODO]} to do, {n[FAIL]} failed",
              "READY" if not n[FAIL] and not n[TODO] else
              "READY WITH TODOS" if not n[FAIL] else "NOT READY: fix the FAIL lines first"]
    return "\n".join(lines)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Check a fork's readiness (read-only).")
    sub = ap.add_subparsers(dest="cmd")
    c = sub.add_parser("check")
    c.add_argument("--root", default=str(DEFAULT_ROOT))
    c.add_argument("--branding", help="a branding.json to check instead of launcher/branding.json")
    c.add_argument("--online", action="store_true", help="fetch every update source, mirror, relay and the flags file")
    c.add_argument("--chain", help="a chain.json to verify against the release-log key")
    c.add_argument("--path", action="append", default=[], help="more player-facing files or folders to check")
    c.add_argument("--json", action="store_true")
    a = ap.parse_args(argv)
    if a.cmd != "check":
        ap.print_help()
        return 2
    items = run(Path(a.root), a.branding, a.online, None, a.chain, a.path)
    print(json.dumps(items, indent=1) if a.json else render(items))
    return 1 if any(i["status"] == FAIL for i in items) else 0


if __name__ == "__main__":
    sys.exit(main())
