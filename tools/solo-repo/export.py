#!/usr/bin/env python3
"""Leyline Solo's own public repo (github.com/orionopenwow/leyline-solo), copied out of the workspace
(docs/LEYLINE-SOLO.md). The owner makes the empty repo on GitHub once; the Control step `leyline-solo` runs this
into the clean clone .repos\\leyline-solo-clean and pushes as Orion.

    py -3 tools\\solo-repo\\export.py .repos\\leyline-solo-clean
    python3 tools/solo-repo/export.py TARGET [--source REPO] [--dry-run] [--list-exec]

What the repo holds:
  * the launcher crate (launcher/src-tauri, Rust + Tauri 2) with the `solo` feature on by default, its lockfiles, and
    the files its build reads in (include_str!: three stream/*.json catalogs, one ui/ script, the branding art);
  * the Solo window launcher/ui-solo/ and its tests; launcher/scripts/release_version.py (the version of each ship);
  * tools/solo/: the engine's pins and Solo profile, the sign-in glue, mpq112, solo_proof (a developer tool);
  * .github/workflows/: leyline-solo.yml (the Windows installer, signed for the updater) and solo-engine.yml (the
    world server, built from source at pinned commits);
  * README.md, LICENSE (the Leyline repo's MIT), NOTICE, .gitignore.
Never: the launcher's own UI, Wrath Unleashed, community realms, Your Den's files, Boss Rush, the website, the phone
app, any .env, token, key, log or game data. (The crate's Rust is shared with the launcher and carries those modes'
code; the Solo window never reaches them.)

Safety, before anything is written (export.py's own checks): every file goes through its refusal() (game data,
secrets, logs, decompiled code, original-binary citations); every include_str!/include_bytes! must land on an
exported file; the brand check (the one legal line allowed) on everything people read: the README, NOTICE and every
file of the Solo window; no client version numbers in the README's and NOTICE's sentences. Only files git tracks are
taken (a game folder's path in tools/solo/folder.txt, caches and build output never leave the PC). Any refusal stops
the export and nothing is written. Same source in, same bytes out. Files no longer exported are deleted, only inside
the folders this export manages, never in .git.

Exit codes: 0 ok, 1 refused, 2 usage. Standard library only.
"""
from __future__ import annotations

import argparse
import importlib.util
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
_spec = importlib.util.spec_from_file_location("leyline_export", REPO / "tools" / "leyline-repo" / "export.py")
ex = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ex)
Refused = ex.Refused

SLUG = "orionopenwow/leyline-solo"
LEGAL = "Leyline is an independent fan project, not affiliated with or endorsed by Blizzard Entertainment."
# The first version Solo ships as (tauri.conf.json's version in the solo repo: release_version.py's floor)
VERSION_FLOOR = "0.1.0"

# Folders copied whole (tracked files only, minus refusals and SKIP), and single files. Paths as in the workspace.
DIRS = [
    "launcher/src-tauri",
    "launcher/ui-solo",
    "tools/solo",
]
FILES = [
    "launcher/package.json",
    "launcher/package-lock.json",
    "launcher/branding.json",                       # branding.rs include_str!
    "launcher/scripts/release_version.py",
    "launcher/tests/soloapp.browser.test.cjs",
    "launcher/ui/hostflow-catalog.js",              # host/flow.rs include_str!
    "launcher/ui/art/leyline-mark.png",             # branding.json's logo
    "launcher/ui/art/leyline-emblem.webp",          # branding.json's emblem
    "stream/playmodes.json",                        # modes.rs include_str!
    "stream/modepacks.json",                        # host/modes.rs include_str!
    "stream/loadouts.json",                         # host/flow.rs include_str!
    "hosting/kit/profiles/cmangos-1.12.1/profile.lock",   # the engine's core and database pins (solo-engine.yml)
]
# Workflows: target -> source
WORKFLOWS = {
    ".github/workflows/leyline-solo.yml": "tools/solo-repo/workflows/leyline-solo.yml",
    ".github/workflows/solo-engine.yml": "tools/github-workflows/solo-engine.yml",
}
# Left out of the copied folders: the phone app, the launcher-only examples and the phone's Den Lite page tools
SKIP = re.compile(r"^launcher/src-tauri/(mobile/|examples/|gen/|target/|tauri\.android\.conf\.json$)"
                  r"|^tools/solo/(folder\.txt$|solo_wasm\.py$|test_solo_wasm\.py$|Get solo wasm\.bat$)")
GENERATED = ["README.md", "LICENSE", "NOTICE", ".gitignore"]
MANAGED = sorted({p.split("/", 1)[0] for p in DIRS + FILES + list(WORKFLOWS)})
# Client version numbers (the README and NOTICE's sentences never name one)
VERSION = re.compile(r"(?<![\w.\-/])(?:1\.12(?:\.\d)?|2\.4\.3|3\.3\.5a?|4\.3\.4|5\.4\.8)(?![\w.])")


def tracked(src: Path) -> set[str] | None:
    """The files git tracks in src (posix paths), or None when src isn't a git work tree."""
    try:
        r = subprocess.run(["git", "-C", str(src), "ls-files", "-z"], capture_output=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0:
        return None
    return {p for p in r.stdout.decode("utf-8", "replace").split("\0") if p}


def solo_cargo(data: bytes) -> bytes:
    """Cargo.toml with the solo feature on by default (the solo repo builds Leyline Solo with a plain build)."""
    text = data.decode("utf-8")
    new, n = re.subn(r'(?m)^default = \["app"\]$', 'default = ["app", "solo"]', text, count=1)
    if n != 1:
        raise Refused('launcher/src-tauri/Cargo.toml: no `default = ["app"]` line to turn solo on')
    if not re.search(r'(?m)^solo = \[', new):
        raise Refused("launcher/src-tauri/Cargo.toml has no `solo` feature")
    return new.encode("utf-8")


def solo_conf(data: bytes) -> bytes:
    """tauri.conf.json with the solo repo's own version floor (each ship stamps its version over it in CI)."""
    text = data.decode("utf-8")
    new, n = re.subn(r'("version"\s*:\s*")[^"]*(")', lambda m: m.group(1) + VERSION_FLOOR + m.group(2), text, count=1)
    if n != 1:
        raise Refused("launcher/src-tauri/tauri.conf.json: no version line")
    return new.encode("utf-8")


TRANSFORM = {"launcher/src-tauri/Cargo.toml": solo_cargo, "launcher/src-tauri/tauri.conf.json": solo_conf}


def plan(src: Path) -> dict:
    """{target rel: (bytes, executable)} for every copied file (not the generated ones)."""
    git = tracked(src)
    out = {}

    def take(dst: str, rel: str, named: bool):
        p = src / rel
        if p.is_symlink():
            raise Refused(f"{rel} is a symlink")
        data = p.read_bytes()
        why = ex.refusal(rel, data)
        if why:
            raise Refused(f"{rel} looks like {why}: not exported" if named else f"{rel} ({why}) is inside {dst.split('/')[0]}: not exported")
        if rel in TRANSFORM:
            data = TRANSFORM[rel](data)
        out[dst] = (data, data.startswith(b"#!"))

    for d in DIRS:
        if not (src / d).is_dir():
            raise Refused(f"{d} is missing from {src}")
        files, _skipped = ex.walk(src, d)
        for rel in files:
            if SKIP.search(rel) or (git is not None and rel not in git):
                continue
            take(rel, rel, False)
    for rel in FILES:
        if not (src / rel).is_file():
            raise Refused(f"{rel} is missing from {src}")
        take(rel, rel, True)
    for dst, rel in WORKFLOWS.items():
        if not (src / rel).is_file():
            raise Refused(f"{rel} is missing from {src}")
        take(dst, rel, True)
    return out


# --------------------------------------------------------------------------------------------- generated files
# TODO(owner): the intro in your own voice (the first paragraph)
README = f"""# Leyline Solo

<!-- TODO(owner): a few lines in your own voice: why Leyline Solo, who it is for -->

**Leyline Solo** is a single player for the first-era game, on your own game folder. It runs a small world of your
own on your PC: you, your companions, and friends you invite. No server to rent, no account to make, and it works
offline. It signs you in by itself.

- **Your own game folder.** Leyline Solo never downloads, offers or shares the game. It reads the maps from your
  folder once, on your PC, and they never leave it.
- **Companions.** In the game, type `.bot group` in chat: four companions of the right classes at your level join your
  party. In party chat: `follow`, `stay`, `attack`.
- **Co-op by invite.** One link: on the same network your friend joins you directly, anywhere else through the
  Leyline relay. No router settings.

## Install

Download the newest installer from the [releases](https://github.com/{SLUG}/releases) (`Leyline Solo_..._x64-setup.exe`,
Windows 10 and 11). It is not code-signed yet: Windows may ask once ("More info", then "Run anyway"). Leyline Solo
updates itself: when a new version is ready it says so, and "Restart to update" finishes it.

Each release lists the SHA-256 of its files in `SHA256SUMS.txt`: check yours with `certutil -hashfile <file> SHA256`.

## First run

1. **Point it at your game folder**: the folder of your own first-era game (the original, before any expansion), the
   one that holds the game's program. Leyline Solo tells you plainly if a folder is another version.
2. **Set up**, once: the world's own files download (about 90 MB), the maps are read from your game folder and the
   world is filled. About 20 to 50 minutes the first time; after that it starts in a minute or two.
3. **Play**: your game opens and signs you in. **Quick start** makes a new character (race, class, name) and takes you
   straight in; **Characters** lists yours, and Play enters that one.

Sign-in has two ways (Settings): **Simple**, the default, opens the login screen with your account name filled in
(the password is in Settings, "Show password"); **Quick sign-in** uses a small add-on to go straight to character
select. Settings also removes the sign-in from your game folder, putting it back as it was.

## Companions

| In chat | What it does |
|---|---|
| `.bot group` | four companions (a tank, a healer, damage dealers) at your level join your party |
| `.bot create class=priest level=10` | one companion of that class and level |
| `.bot add <Name>` / `.bot remove <Name>` | bring one of your own characters in as a companion / send it away |
| `.bot list` | your companions in the world |
| party chat: `follow`, `stay`, `attack`, `flee`, `summon`, `revive` | orders to the whole party |

## Invite a friend

Start your world, open **Invite a friend**, press **Make the invite link** and send it. Your friend needs their own
first-era game folder and Leyline Solo (or the Leyline Launcher); they open the link and press Join.

## Build it from source

Windows, with Rust (stable), Node 20 and the WebView2 runtime:

```sh
cd launcher
npm ci
npx tauri build --features solo --config src-tauri/tauri.solo.conf.json --bundles nsis
```

The window is plain HTML, CSS and JS in `launcher/ui-solo/` (no bundler). Its tests run in Chromium with a fake core:
`cd launcher && node --test tests/soloapp.browser.test.cjs` (Playwright).

The world server is built by `.github/workflows/solo-engine.yml` from source at pinned commits
(`hosting/kit/profiles/cmangos-1.12.1/profile.lock`, `tools/solo/solo.lock`) and published on this repo's release
`solo-engine-latest`. `tools/solo/` holds its settings (`conf/`), the sign-in add-on (`glue/`) and `solo_proof.py`,
the developer tool that sets a world up by hand.

## Licences

- Leyline Solo's own code: MIT (`LICENSE`).
- The world server: CMaNGOS Classic (GPL-2.0), its playerbots module (companions, GPL-2.0) and the classic-db world
  database (GPL-2.0), built from their sources at pinned commits. No source of theirs is in this repo; the server's
  package carries their licences and a list of every source at its exact commit. See `NOTICE`.
- **No game data, ever**: not in this repo, not in a release. Your maps are made on your PC from your own game folder.

{LEGAL}
"""

NOTICE = f"""Leyline Solo
Copyright Leyline contributors. MIT licensed (LICENSE).

This repo builds, but does not contain, these programs:

- CMaNGOS Classic (GPL-2.0), https://github.com/cmangos/mangos-classic, at the commit in
  hosting/kit/profiles/cmangos-1.12.1/profile.lock.
- playerbots for CMaNGOS (GPL-2.0), https://github.com/cmangos/playerbots, at the commit in tools/solo/solo.lock.
- classic-db, the CMaNGOS world database (GPL-2.0), https://github.com/cmangos/classic-db, at the commit in
  hosting/kit/profiles/cmangos-1.12.1/profile.lock.
- MySQL Community Server (GPL-2.0 with the FOSS exception), the database the world runs on, as in tools/solo/solo.lock.

The world server's package (the release solo-engine-latest) carries their licences (LICENSES/), NOTICE.txt and
SOURCES.txt (every source at its exact commit).

The launcher crate is built with Tauri (MIT or Apache-2.0) and the Rust crates in launcher/src-tauri/Cargo.lock,
each under its own licence. The window's font is Inter, SIL Open Font License 1.1
(launcher/ui-solo/art/fonts/Inter-OFL.txt).

No game data is in this repo or its releases: players make their maps on their own PC from their own game folder.

{LEGAL}
"""

GITIGNORE = """# Leyline Solo: our own code and docs only. Never game data, secrets or machine-local files.
**/state/
**/cache/
**/logs/
.env
*.env
*.pem
*.key
*.log
__pycache__/
*.pyc
node_modules/
launcher/src-tauri/target/
launcher/src-tauri/gen/
launcher/updater.conf.json
tools/solo/folder.txt
*.mpq
*.MPQ
*.m2
*.wmo
*.adt
*.blp
*.dbc
*.map
*.vmtree
*.vmtile
*.mmap
*.mmtile
.DS_Store
Thumbs.db
"""


def generated(src: Path) -> dict:
    return {"README.md": (README.encode(), False), "LICENSE": (ex.license_text(src), False),
            "NOTICE": (NOTICE.encode(), False), ".gitignore": (GITIGNORE.encode(), False)}


def prose(text: str) -> str:
    """A text's sentences: code spans, code blocks and links' addresses taken out."""
    text = re.sub(r"```.*?```", "", text, flags=re.S)
    text = re.sub(r"`[^`]*`", "", text)
    text = re.sub(r"\]\([^)]*\)", "]", text)
    text = re.sub(r"https?://\S+", "", text)
    return re.sub(r"\S+/\S+", "", text)          # paths: the profile folder's name carries its build


def people_read(rel: str) -> bool:
    return rel in ("README.md", "NOTICE") or rel.startswith("launcher/ui-solo/") and rel.endswith((".html", ".js", ".css"))


def checks(files: dict) -> list:
    """Every reason the export must stop (empty: none)."""
    out = [f"the launcher builds in a file the export leaves out: {m}" for m in ex.missing_includes(files)]
    for rel in sorted(files):
        if people_read(rel):
            out += ex.brand_hits(rel, files[rel][0])
        if rel in ("README.md", "NOTICE"):
            m = VERSION.search(prose(files[rel][0].decode("utf-8", "replace")))
            if m:
                out.append(f"{rel}: a client version number in a sentence ({m.group(0)})")
        top = rel.split("/", 1)[0]
        if rel not in GENERATED and top not in MANAGED:
            out.append(f"{rel}: outside the solo repo's folders")
        if rel.startswith("launcher/ui/") and rel not in FILES:
            out.append(f"{rel}: the launcher's own window is not part of Leyline Solo")
    for lock in ("launcher/package-lock.json", "launcher/src-tauri/Cargo.lock"):
        if lock not in files:
            out.append(f"{lock}: the installer workflow builds from it")
    cargo = files.get("launcher/src-tauri/Cargo.toml", (b"", False))[0]
    if b'default = ["app", "solo"]' not in cargo:
        out.append("launcher/src-tauri/Cargo.toml: the solo feature is not on by default")
    if "launcher/src-tauri/tauri.solo.conf.json" not in files or "launcher/ui-solo/index.html" not in files:
        out.append("the Solo window (tauri.solo.conf.json, ui-solo/index.html) is missing")
    return out


def stale(target: Path, keep: set) -> list:
    res = []
    for top in MANAGED:
        base = target / top
        if not base.is_dir() or base.is_symlink():
            continue
        for d, dirs, names in os.walk(base):
            dirs[:] = sorted(x for x in dirs if x != ".git" and not (Path(d) / x).is_symlink())
            for n in sorted(names):
                rel = (Path(d) / n).relative_to(target).as_posix()
                if rel not in keep:
                    res.append(rel)
    return res


def export(src: Path, target: Path, dry: bool = False, out=print) -> dict:
    src, target = src.resolve(), target.resolve()
    in_repos = (src / ".repos") in target.parents
    if target == src or target in src.parents or (src in target.parents and not in_repos):
        raise Refused(f"the target {target} must be outside the workspace folder (or under its .repos/), and not contain it")
    files = plan(src)
    files.update(generated(src))
    problems = checks(files)
    if problems:
        raise Refused("nothing was written:\n  " + "\n  ".join(problems))
    written, deleted = [], []
    if not dry:
        target.mkdir(parents=True, exist_ok=True)
    for rel in sorted(files):
        data, exe = files[rel]
        if dry:
            p = target / rel
            if not p.is_file() or p.read_bytes() != data:
                written.append(rel)
        elif ex.write_if_changed(target / rel, data, exe):
            written.append(rel)
    for rel in stale(target, set(files)):
        deleted.append(rel)
        if not dry:
            (target / rel).unlink()
    if not dry:
        for top in MANAGED:
            base = target / top
            if base.is_dir():
                for d, _dirs, _names in sorted(os.walk(base, topdown=False), key=lambda t: -len(t[0])):
                    if ".git" not in Path(d).relative_to(target).parts:
                        try:
                            if not os.listdir(d):
                                os.rmdir(d)
                        except OSError:
                            pass
    for rel in written:
        out(f"wrote   {rel}")
    for rel in deleted:
        out(f"deleted {rel}")
    out(f"Leyline Solo export: {len(files)} files, {len(written)} written, {len(deleted)} deleted"
        f"{' (dry run)' if dry else ''} -> {target}")
    return {"files": sorted(files), "written": written, "deleted": deleted, "exec": sorted(r for r, (_, e) in files.items() if e)}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Copy Leyline Solo's parts of the workspace into the leyline-solo repo folder.")
    ap.add_argument("target", nargs="?")
    ap.add_argument("--source", default=str(REPO), help="the workspace folder (default: this repo)")
    ap.add_argument("--dry-run", action="store_true", help="say what would change, write nothing")
    ap.add_argument("--list-exec", action="store_true", help="print the exported files that are executable")
    a = ap.parse_args(argv)
    try:
        if a.list_exec:
            for rel in sorted(r for r, (_, e) in plan(Path(a.source).resolve()).items() if e):
                print(rel)
            return 0
        if not a.target:
            ap.print_usage(sys.stderr)
            return 2
        export(Path(a.source), Path(a.target), a.dry_run)
        return 0
    except Refused as e:
        print(f"EXPORT STOPPED: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
