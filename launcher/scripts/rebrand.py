#!/usr/bin/env python3
"""Run the launcher under another name with your own keys and servers (docs/FORKING.md).

    python3 launcher/scripts/rebrand.py MY-BRANDING.json [--root REPO] [--dry-run]

MY-BRANDING.json has the same shape as launcher/branding.json. This copies its values into every place in the
launcher (and the host kit's copy of the release-log key) that needs them as text, then writes it as
launcher/branding.json:

  * the GitHub repo, the update API / downloads / releases page, the mirrors, the release-log addresses, the flags
    file, the site and the relays, wherever the old values appear in launcher/src-tauri/src (Rust), tauri.conf.json
    and launcher/ui;
  * the seed relays (`p2p::mesh::SEEDS`), from "relays";
  * the name in tauri.conf.json (productName, the window title) and in the UI's .html pages (whole word only), and
    the app identifier;
  * the colours: --t-bg, --t-text, --t-accent, --t-accent-strong in launcher/ui/style.css;
  * the release log's public key in hosting/kit/tools/wunet/release-log.json.

Not changed (do these yourself, docs/FORKING.md says how): the updater key in tauri.conf.json
(plugins.updater.pubkey; `npx @tauri-apps/cli signer generate`), the logo files themselves, and words in the UI's
scripts. Afterwards: `cargo test --no-default-features --lib` in launcher/src-tauri (branding::tests checks that
the code says what branding.json says). Standard library only. Exit codes: 0 ok, 1 refused, 2 usage.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_ROOT = os.path.dirname(os.path.dirname(HERE))
HEX64 = re.compile(r"^[0-9a-f]{64}$")
COLOUR = re.compile(r"^#[0-9a-fA-F]{6}$")


class Refused(Exception):
    pass


def https(u) -> bool:
    return isinstance(u, str) and re.match(r"^https://[A-Za-z0-9.-]+\.[A-Za-z]{2,}(/[^\s]*)?$", u) is not None


def check(b: dict) -> None:
    if b.get("format") != 1:
        raise Refused("format must be 1")
    if not isinstance(b.get("name"), str) or not 0 < len(b["name"].strip()) <= 40:
        raise Refused("name must be 1-40 characters")
    if not re.match(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$", b.get("repo", "")):
        raise Refused("repo must be owner/name")
    if not re.match(r"^[a-z0-9]+(\.[a-z0-9-]+)+$", b.get("identifier", "")):
        raise Refused("identifier must look like org.example.app")
    for k in ("bg", "text", "accent", "accent_strong"):
        if not COLOUR.match((b.get("colours") or {}).get(k, "")):
            raise Refused(f"colours.{k} must be #rrggbb")
    u = b.get("updates") or {}
    urls = [b.get("site"), b.get("flags_url"), u.get("github_api"), u.get("github_downloads"), u.get("releases_page")]
    urls += list(u.get("mirrors") or []) + list((b.get("release_log") or {}).get("urls") or []) + list(b.get("relays") or [])
    for x in urls:
        if not https(x):
            raise Refused(f"{x!r} is not a plain https address")
    for m in list(u.get("mirrors") or []) + [u.get("github_downloads")]:
        if not m.endswith("/"):
            raise Refused(f"{m!r} must end with /")
    k = (b.get("release_log") or {}).get("public_key", "")
    if k and not HEX64.match(k):
        raise Refused("release_log.public_key must be empty or 64 lower-case hex digits")
    if not b.get("relays"):
        raise Refused("at least one relay")


def pairs(old: dict, new: dict) -> list[tuple[str, str]]:
    """(old text, new text) to replace in code, longest first so a longer address is never half-replaced."""
    out = []

    def add(a, b):
        if isinstance(a, str) and isinstance(b, str) and a and a != b:
            out.append((a, b))
    ou, nu = old["updates"], new["updates"]
    for k in ("github_api", "github_downloads", "releases_page"):
        add(ou[k], nu[k])
    for a, b in zip(ou.get("mirrors") or [], nu.get("mirrors") or []):
        add(a, b)
    for a, b in zip(old["release_log"].get("urls") or [], new["release_log"].get("urls") or []):
        add(a, b)
    add(old["flags_url"], new["flags_url"])
    add(f"github.com/{old['repo']}", f"github.com/{new['repo']}")
    add(f"repos/{old['repo']}/", f"repos/{new['repo']}/")
    add(f"\"{old['repo']}\"", f"\"{new['repo']}\"")
    add(old["site"], new["site"])
    for a, b in zip(old.get("relays") or [], new.get("relays") or []):
        add(a, b)
    add(old["identifier"], new["identifier"])
    return sorted(set(out), key=lambda p: -len(p[0]))


def files(root: str):
    lau = os.path.join(root, "launcher")
    for base, exts in ((os.path.join(lau, "src-tauri", "src"), (".rs",)), (os.path.join(lau, "ui"), (".js", ".html", ".css"))):
        for d, dirs, fs in os.walk(base):
            dirs[:] = [x for x in dirs if x not in ("node_modules", "target")]
            for f in sorted(fs):
                if f.endswith(exts):
                    yield os.path.join(d, f)
    yield os.path.join(lau, "src-tauri", "tauri.conf.json")


def seeds_rs(relays: list[str]) -> str:
    return "pub const SEEDS: &[&str] = &[" + ", ".join(json.dumps(r) for r in relays) + "];"


def rebrand(root: str, new: dict, dry: bool = False) -> list[str]:
    check(new)
    bpath = os.path.join(root, "launcher", "branding.json")
    with open(bpath, encoding="utf-8") as f:
        old = json.load(f)
    reps = pairs(old, new)
    changed = []
    for p in files(root):
        try:
            with open(p, encoding="utf-8") as f:
                s = f.read()
        except (OSError, UnicodeDecodeError):
            continue
        t = s
        for a, b in reps:
            t = t.replace(a, b)
        # the repo's name anywhere else (comments, messages), but not a longer name that starts the same
        t = re.sub(re.escape(old["repo"]) + r"(?![\w-])", new["repo"].replace("\\", r"\\"), t)
        if p.endswith(os.path.join("p2p", "mesh.rs")):
            t = re.sub(r"pub const SEEDS: &\[&str\] = &\[[^\]]*\];", seeds_rs(new["relays"]), t, count=1)
        if p.endswith("style.css"):
            for var, key in (("--t-bg", "bg"), ("--t-text", "text"), ("--t-accent", "accent"), ("--t-accent-strong", "accent_strong")):
                t = re.sub(rf"({re.escape(var)}:\s*)#[0-9a-fA-F]{{6}}", lambda m: m.group(1) + new["colours"][key], t, count=1)
        if p.endswith(".html") and old["name"] != new["name"]:
            t = re.sub(rf"\b{re.escape(old['name'])}\b", new["name"], t)
        if p.endswith("tauri.conf.json"):
            conf = json.loads(t)
            conf["productName"] = new["name"]
            conf["identifier"] = new["identifier"]
            for w in (conf.get("app") or {}).get("windows") or []:
                if w.get("title") == old["name"]:
                    w["title"] = new["name"]
            t = json.dumps(conf, indent=2, ensure_ascii=False) + "\n" if conf != json.loads(s) else s
        if t != s:
            changed.append(os.path.relpath(p, root))
            if not dry:
                with open(p, "w", encoding="utf-8", newline="") as f:
                    f.write(t)
    kp = os.path.join(root, "hosting", "kit", "tools", "wunet", "release-log.json")
    if os.path.isfile(kp):
        with open(kp, encoding="utf-8") as f:
            kd = json.load(f)
        if kd.get("public_key") != new["release_log"].get("public_key", ""):
            kd["public_key"] = new["release_log"].get("public_key", "")
            changed.append(os.path.relpath(kp, root))
            if not dry:
                with open(kp, "w", encoding="utf-8") as f:
                    f.write(json.dumps(kd, indent=1, ensure_ascii=False) + "\n")
    if not dry:
        with open(bpath, "w", encoding="utf-8") as f:
            f.write(json.dumps(new, indent=2, ensure_ascii=False) + "\n")
    changed.append(os.path.relpath(bpath, root))
    return changed


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Rebrand the launcher from a branding.json (docs/FORKING.md).")
    ap.add_argument("branding")
    ap.add_argument("--root", default=DEFAULT_ROOT)
    ap.add_argument("--dry-run", action="store_true")
    o = ap.parse_args(argv)
    try:
        with open(o.branding, encoding="utf-8") as f:
            new = json.load(f)
        changed = rebrand(o.root, new, o.dry_run)
    except (Refused, OSError, ValueError, KeyError) as e:
        print(f"rebrand: {e}", file=sys.stderr)
        return 1
    print(("Would change" if o.dry_run else "Changed") + f" {len(changed)} files:")
    for c in changed:
        print("  " + c)
    print("Next: cargo test --no-default-features --lib (in launcher/src-tauri), then your own updater key (docs/FORKING.md).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
