#!/usr/bin/env python3
"""Leyline-Mobile (github.com/orionopenwow/Leyline-Mobile): the phone release's own public repo, copied out of the
workspace (docs/MOBILE-RELEASE.md). The owner makes the empty repo on GitHub once; the Control step `leyline-mobile`
runs this and pushes.

    py -3 tools\\leyline-repo\\mobile_export.py .repos\\leyline-mobile --leyline-ref <Leyline repo commit>
    python3 tools/leyline-repo/mobile_export.py TARGET [--source REPO] [--leyline-ref SHA] [--dry-run]

What the repo is, and why the app is built from the Leyline repo:
  The Android app is the Leyline launcher crate built for Android (launcher/: Tauri 2, one Rust crate for desktop and
  phone, ~90 modules, its Cargo.lock and the stream/ JSON it builds in). Splitting the phone half into its own crate
  would mean a second copy of the launcher that drifts from the first, or a refactor of the core that can't be
  compiled here. So the app's single source stays in orionopenwow/Leyline, and this repo's CI checks out the Leyline
  repo at one pinned commit (LEYLINE_REF, written by this export from the commit the `leyline` step just pushed),
  builds the APK there, starts it on an emulator and publishes it on THIS repo's releases (android-latest).
  This repo itself carries what is the phone's own: the browser game's touch layer and phone-solo web files (web/),
  a read-only mirror of the phone-specific launcher files (app/, the same bytes as at LEYLINE_REF's source commit),
  the screenshots, the guides, the CI workflow, MIT licence, the legal line.

Safety: the same checks as export.py (its refusal(): game data, secrets, logs, decompiled code, original-binary
citations) on every file; its brand check (the one legal line allowed) on everything people read (the README, the
docs, and every file the app or the game page shows: web/, app/ui/, app/src-tauri/; the tests and tools carry the
banned words as their own checks, as in export.py), plus: no client version numbers in the
README and the docs' sentences, and only the files in LAYOUT (nothing else of the workspace). Any refusal stops the
export and nothing is written. Same source in, same bytes out. Files no longer exported are deleted, only inside the
folders this export manages (MANAGED), never in .git.

Exit codes: 0 ok, 1 refused, 2 usage. Standard library only.
"""
from __future__ import annotations

import argparse
import importlib.util
import os
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
_spec = importlib.util.spec_from_file_location("leyline_export", HERE / "export.py")
ex = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ex)
Refused = ex.Refused

SLUG = "orionopenwow/Leyline-Mobile"
LEYLINE = "orionopenwow/Leyline"
REF_RE = re.compile(r"[0-9a-f]{40}|main")

# target path -> source path (files), or (target folder, source folder, name pattern) for a folder's matching files
LAYOUT_FILES = {
    # the browser game's touch layer (docs/TOUCH-CONTROLS.md)
    "web/touch.js": "stream/www-files/touch.js",
    "web/touch-driver.js": "stream/www-files/touch-driver.js",
    "web/input.js": "stream/www-files/input.js",
    "web/leyline.webmanifest": "stream/www-files/leyline.webmanifest",
    # the phone app's own parts of the launcher (mirror; the build uses the Leyline repo at LEYLINE_REF)
    "app/ui/platform.js": "launcher/ui/platform.js",
    "app/ui/phone.js": "launcher/ui/phone.js",
    "app/ui/phone.css": "launcher/ui/phone.css",
    "app/src-tauri/src/mobile.rs": "launcher/src-tauri/src/mobile.rs",
    "app/src-tauri/tauri.android.conf.json": "launcher/src-tauri/tauri.android.conf.json",
    "app/src-tauri/capabilities/mobile.json": "launcher/src-tauri/capabilities/mobile.json",
    "app/src-tauri/mobile/android/MainActivity.kt": "launcher/src-tauri/mobile/android/MainActivity.kt",
    "app/scripts/android-smoke.sh": "launcher/scripts/android-smoke.sh",
    "app/tests/phone.browser.test.cjs": "launcher/tests/phone.browser.test.cjs",
    "app/tests/phonejourney.browser.test.cjs": "launcher/tests/phonejourney.browser.test.cjs",
    # the guides
    "docs/ANDROID.md": "docs/ANDROID.md",
    "docs/TOUCH-CONTROLS.md": "docs/TOUCH-CONTROLS.md",
    "docs/MOBILE-RELEASE.md": "docs/MOBILE-RELEASE.md",
    "docs/ON-YOUR-PHONE.md": "docs/leyline/mobile.md",
    # the app icon CI makes (tauri icon)
    "art/leyline-crystal-512.png": "stream/www-files/art/leyline-crystal-512.png",
    # how the screenshots are made
    "tools/mobile-shots/mobile_shots.cjs": "tools/mobile-shots/mobile_shots.cjs",
    "tools/mobile-shots/to_webp.py": "tools/mobile-shots/to_webp.py",
}
LAYOUT_DIRS = [
    ("web/solo", "stream/www-files/solo", re.compile(r"^[a-z0-9-]+\.js$")),   # phone solo's page side (never the .wasm)
    ("screenshots", "stream/www-files/art/mobile", re.compile(r"^[a-z0-9-]+\.webp$")),
]
GENERATED = ["README.md", "LICENSE", ".gitignore", "LEYLINE_REF", ".github/workflows/android.yml"]
MANAGED = sorted({p.split("/", 1)[0] for p in list(LAYOUT_FILES) + [d for d, _, _ in LAYOUT_DIRS]} | {".github"})
LEGAL = "Leyline is an independent fan project, not affiliated with or endorsed by Blizzard Entertainment."
# a client version number as a word (a build badge in data is fine; these files are prose)
VERSION = re.compile(r"(?<![\w.\-/])(?:1\.12(?:\.\d)?|2\.4\.3|3\.3\.5a?|4\.3\.4|5\.4\.8)(?![\w.])")


def prose(text: str) -> str:
    """The sentences of a Markdown file: no fenced code, no inline code, no link targets."""
    text = re.sub(r"```.*?```", " ", text, flags=re.S)
    text = re.sub(r"`[^`\n]*`", " ", text)
    return re.sub(r"\]\([^)]*\)", "]", text)


DOC_PAGES = {"ANDROID.md", "TOUCH-CONTROLS.md", "MOBILE-RELEASE.md", "ON-YOUR-PHONE.md"}
SITE_DOCS = "https://leylinelauncher.com/wrath/docs/"
MD_LINK = re.compile(r"\[([^\]]+)\]\(([A-Za-z0-9_./-]+\.md)(#[a-z0-9-]+)?\)")


def fix_links(dst: str, data: bytes) -> bytes:
    """Links between the docs, as they are in this repo: the four guides link each other; a page of the website's docs
    (docs/leyline/<page>.md) links its page on the site; any other workspace doc is not here, so its text stays and
    the link goes."""
    if not (dst.startswith("docs/") and dst.endswith(".md")):
        return data

    def one(m):
        text, target, frag = m.group(1), m.group(2), m.group(3) or ""
        name = target.rsplit("/", 1)[-1]
        if target == "leyline/mobile.md" or (dst == "docs/ON-YOUR-PHONE.md" and name == "mobile.md"):
            return f"[{text}](ON-YOUR-PHONE.md{frag})"
        if "/" not in target and name in DOC_PAGES:
            return m.group(0)
        if target.startswith("leyline/") or dst == "docs/ON-YOUR-PHONE.md":
            return f"[{text}]({SITE_DOCS}{name[:-3]}{frag})"
        return text
    return MD_LINK.sub(one, data.decode("utf-8")).encode("utf-8")


def plan(src: Path) -> dict:
    files = {}
    for dst, rel in LAYOUT_FILES.items():
        p = src / rel
        if not p.is_file():
            raise Refused(f"{rel} (for {dst}) is missing from {src}")
        files[dst] = (rel, p.read_bytes())
    for dst_dir, rel_dir, pat in LAYOUT_DIRS:
        d = src / rel_dir
        names = sorted(n for n in (os.listdir(d) if d.is_dir() else []) if pat.match(n) and (d / n).is_file())
        if not names:
            raise Refused(f"{rel_dir} has no files for {dst_dir}")
        for n in names:
            files[f"{dst_dir}/{n}"] = (f"{rel_dir}/{n}", (d / n).read_bytes())
    for dst, (rel, data) in files.items():
        if (src / rel).is_symlink():
            raise Refused(f"{rel} is a symlink: not exported")
        why = ex.refusal(rel, data)
        if why:
            raise Refused(f"{rel} looks like {why}: not exported")
    return {dst: fix_links(dst, data) for dst, (rel, data) in files.items()}


# --------------------------------------------------------------------------------------------- generated files
README = """# Leyline on your phone

Play Leyline on a phone: in your phone's browser, or in the **Leyline app for Android**. This repo is the phone
release: the app's download, the browser game's touch controls, phone solo, the guides and the screenshots.

<!-- TODO(owner): a line or two in your own words -->

![The Leyline app's Play tab](screenshots/phone-play.webp)

## What you can do on a phone

- **Play in the browser.** Open the Leyline website on your phone and press Play: nothing to install.
- **Add it to your home screen.** It opens full screen and sideways, like an app.
- **Touch controls.** A stick to walk, an attack key with four ability keys round it, a tray for bags, quests and
  chat. Drag to look, pinch to zoom, two fingers to right-click. ([the full list](docs/TOUCH-CONTROLS.md))
- **Play solo on this phone.** Your own world, run by the phone itself: a small server of our own runs inside the
  game page. No realm, your companion at your side, saved on the phone.
- **Join a friend.** A friend hosts on their PC and sends you an invite link: copy it, tap **Paste invite link and
  join**, and their realm opens on your phone.
- **Addons.** Interface addons run in the game page too (Your addons, from the addon's .zip).

| | |
|---|---|
| ![Touch controls](screenshots/phone-touch.webp) | ![Join a friend](screenshots/phone-join.webp) |
| ![Realms](screenshots/phone-realms.webp) | ![Mods and addons](screenshots/phone-mods.webp) |

**What a phone can't do:** host a realm for others (a phone sleeps and has no address friends can reach: host on a
PC with the Leyline launcher), a friend's Den, client mods, looks and HD packs (they change a game folder), and
carrying the network. [Phone or computer?](docs/ON-YOUR-PHONE.md)

## Get the app (Android)

1. On your phone, open this repo's **[Releases](../../releases)**, then **Leyline for Android (latest)**, and tap
   **Leyline-debug.apk**. The **.sha256** file beside it is its checksum.
2. Android asks once whether your browser (or Files) may install apps: **Settings**, **Allow from this source**.
   Play Protect may say it doesn't know us: **Install anyway**.
3. Open Leyline. Log in under Account (your Hero Key, Google, or your name and password), then press **Play**.

The app asks for no permissions beyond the internet and contains no game files.

## Guides

- [On your phone](docs/ON-YOUR-PHONE.md): what works where, joining a friend, troubleshooting
- [The Android app](docs/ANDROID.md): what it does, how CI builds it, the phone test checklist
- [Touch controls](docs/TOUCH-CONTROLS.md): every control, and a five-minute phone test
- [The mobile release](docs/MOBILE-RELEASE.md): what's done, what isn't, and how this repo is made

## How it is built

The app is the Leyline launcher (Tauri 2: a web UI over one Rust core) built for Android. Its source is one crate
shared with the desktop launcher, so it lives in one place: **[{leyline}](https://github.com/{leyline})**. This
repo's workflow (`.github/workflows/android.yml`) checks that repo out at the commit in `LEYLINE_REF`, builds the
APK on GitHub's own machines, starts it on an Android emulator, and only then puts it on this repo's releases.

| Folder | What |
|---|---|
| `web/` | The browser game's touch layer (`touch.js`, `touch-driver.js`, `input.js`) and phone solo's page side (`web/solo/`: the worker, the fake sockets, the saves) |
| `app/` | The phone app's own files, mirrored from the Leyline repo for reading: the phone layout (`ui/phone.js`, `ui/phone.css`, `ui/platform.js`), the Android entry point (`src-tauri/src/mobile.rs`), the Android config and activity, the emulator smoke test and the phone UI tests |
| `screenshots/` | Made from the real pages at phone sizes (`tools/mobile-shots/`) |
| `docs/` | The guides |

Changes to the app go to the Leyline repo; this repo follows it.

## Licence

MIT (see `LICENSE`). No game data is in this repo or in the app: the browser game brings what it needs from the
Leyline website while you play.

{legal} Game names belong to their owners.
"""

GITIGNORE = """# Leyline-Mobile: our own code, docs and pictures only. Never game data, secrets, keys or build output.
*.mpq
*.MPQ
*.wasm
*.apk
*.aab
*.jks
*.keystore
*.env
.env*
node_modules/
target/
gen/
leyline/
"""

# The source workflow (tools/github-workflows/leyline-android.yml) made to build the Leyline repo at LEYLINE_REF from
# this repo: every change is an exact replacement that must apply, so a change to the source workflow that breaks one
# stops the export instead of shipping a workflow that builds the wrong thing.
CHECKOUT = "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4\n"
CHECKOUT_LEYLINE = (CHECKOUT + "        with:\n          repository: " + LEYLINE + "\n          ref: ${{ env.LEYLINE_REF }}\n")
ON_SOURCE = re.compile(r"^on:\n(?:  .*\n|    .*\n|      .*\n)+", re.M)


def workflow(src: Path, ref: str) -> bytes:
    text = (src / "tools" / "github-workflows" / "leyline-android.yml").read_text(encoding="utf-8")
    head = ("# Leyline-Mobile: the Android app, built from " + LEYLINE + " at LEYLINE_REF (written by the workspace's\n"
            "# tools/leyline-repo/mobile_export.py; a new ref is a new build), started on an emulator, then published on\n"
            "# THIS repo's releases (android-latest). Made from the Leyline repo's leyline-android.yml: change that one.\n")
    if text.count(CHECKOUT) != 2:
        raise Refused("leyline-android.yml: expected the checkout step in apk and smoke (the mobile workflow needs them)")
    text = text.replace(CHECKOUT, CHECKOUT_LEYLINE)
    if text.count("name: leyline android\n") != 1:
        raise Refused("leyline-android.yml: its name line changed")
    text = text.replace("name: leyline android\n", "name: leyline mobile android\n")
    on = ("on:\n  push:\n    branches: [main]\n    paths:\n      - \"LEYLINE_REF\"\n      - \".github/workflows/android.yml\"\n"
          "  workflow_dispatch:\n")
    text, n = ON_SOURCE.subn(on, text, count=1)
    if n != 1:
        raise Refused("leyline-android.yml: its on: block changed")
    if text.count("env:\n  ANDROID_API:") != 1:
        raise Refused("leyline-android.yml: its env block changed")
    text = text.replace("env:\n  ANDROID_API:", f"env:\n  LEYLINE_REF: \"{ref}\"\n  ANDROID_API:")
    if "github.ref == 'refs/heads/main'" not in text or "gh release upload android-latest" not in text:
        raise Refused("leyline-android.yml: the publish job changed")
    return (head + text).encode("utf-8")


def generated(src: Path, ref: str) -> dict:
    return {
        "README.md": README.replace("{leyline}", LEYLINE).replace("{legal}", LEGAL).encode("utf-8"),
        "LICENSE": ex.license_text(src),
        ".gitignore": GITIGNORE.encode("utf-8"),
        "LEYLINE_REF": (ref + "\n").encode("ascii"),
        ".github/workflows/android.yml": workflow(src, ref),
    }


def checks(files: dict) -> list:
    """Brand words (export.py's, the legal line allowed) in every text file people read here, and client version
    numbers in the README's and the docs' sentences."""
    hits = []
    for rel in sorted(files):
        # what people read: the docs and the README, and every file the app or the game page shows (web/, app/ui/,
        # the manifest, the Android config); the tests and tools hold the banned words as their own checks
        if not (rel.endswith((".md", ".html", ".webmanifest")) or rel.startswith(("web/", "app/ui/", "app/src-tauri/"))) \
                or not rel.endswith((".md", ".html", ".webmanifest", ".js", ".css", ".json", ".rs", ".kt")):
            continue
        hits += ex.brand_hits(rel, files[rel])
        if rel.endswith(".md"):
            for n, line in enumerate(prose(files[rel].decode("utf-8", "replace")).splitlines(), 1):
                m = VERSION.search(line)
                if m:
                    hits.append(f"{rel}:{n}: a client version number in a sentence (\"{m.group(0)}\")")
    if LEGAL.encode() not in files["README.md"]:
        hits.append("README.md: the legal line is missing")
    return hits


def stale(target: Path, keep: set) -> list:
    out = []
    for top in MANAGED:
        base = target / top
        if not base.is_dir() or base.is_symlink():
            continue
        for d, dirs, names in os.walk(base):
            dirs[:] = sorted(x for x in dirs if x != ".git" and not (Path(d) / x).is_symlink())
            for n in sorted(names):
                rel = (Path(d) / n).relative_to(target).as_posix()
                if rel not in keep:
                    out.append(rel)
    return out


def export(src: Path, target: Path, ref: str = "main", dry: bool = False, out=print) -> dict:
    src, target = src.resolve(), target.resolve()
    if not REF_RE.fullmatch(ref or ""):
        raise Refused(f"--leyline-ref must be a full commit id (40 hex) or main, not {ref!r}")
    in_repos = (src / ".repos") in target.parents
    if target == src or target in src.parents or (src in target.parents and not in_repos):
        raise Refused(f"the target {target} must be outside the workspace folder (or under its .repos/), and not contain it")
    files = plan(src)
    files.update(generated(src, ref))
    hits = checks(files)
    if hits:
        raise Refused("brand words or version numbers (nothing was written):\n  " + "\n  ".join(hits))
    written, deleted = [], []
    for rel in sorted(files):
        exe = files[rel].startswith(b"#!")
        if dry:
            p = target / rel
            if not p.is_file() or p.read_bytes() != files[rel]:
                written.append(rel)
        elif ex.write_if_changed(target / rel, files[rel], exe):
            written.append(rel)
    for rel in stale(target, set(files)):
        deleted.append(rel)
        if not dry:
            (target / rel).unlink()
    for rel in written:
        out(f"wrote   {rel}")
    for rel in deleted:
        out(f"deleted {rel}")
    out(f"Leyline-Mobile export: {len(files)} files, {len(written)} written, {len(deleted)} deleted"
        f"{' (dry run)' if dry else ''}, Leyline at {ref} -> {target}")
    return {"files": sorted(files), "written": written, "deleted": deleted,
            "exec": sorted(r for r, d in files.items() if d.startswith(b"#!"))}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Copy the phone release into the Leyline-Mobile repo folder.")
    ap.add_argument("target", nargs="?")
    ap.add_argument("--source", default=str(REPO))
    ap.add_argument("--leyline-ref", default="main", help="the Leyline repo commit the app is built from")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--list-exec", action="store_true")
    a = ap.parse_args(argv)
    try:
        if a.list_exec:
            for rel, data in sorted(plan(Path(a.source).resolve()).items()):
                if data.startswith(b"#!"):
                    print(rel)
            return 0
        if not a.target:
            ap.print_usage(sys.stderr)
            return 2
        export(Path(a.source), Path(a.target), a.leyline_ref.strip().lower() if a.leyline_ref != "main" else "main", a.dry_run)
        return 0
    except Refused as e:
        print(f"EXPORT STOPPED: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
