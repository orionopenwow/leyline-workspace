"""The launcher's version for each ship (owner 31 Oct: "make sure any update to this launcher from now on is auto
queued for restart update for everyone").

Every push to main of the public Leyline repo that changes the launcher becomes a normal (stable) release, so every
installed launcher (the default channel) downloads it and offers Restart. CI (tools/github-workflows/leyline-launcher.yml
in the workspace; .github/workflows/leyline-launcher.yml in the Leyline repo) runs:

    python3 launcher/scripts/release_version.py plan     prints ship=true|false, version=X.Y.Z, last=<tag or "">
    python3 launcher/scripts/release_version.py stamp X.Y.Z   writes X.Y.Z into tauri.conf.json, Cargo.toml, Cargo.lock

The version: tauri.conf.json's version is the floor (raise it there for a bigger step); otherwise one patch above the
newest stable `launcher-v*` tag. A version with "-" in tauri.conf.json counts by its plain part: ships are always
stable. Pre-releases (the opt-in early channel) stay what they were: a hand-pushed tag with "-" in it.
Nothing changed under launcher/ (or stream/playmodes.json, which it builds in) since that tag: no release.
Standard library only.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONF = ROOT / "launcher" / "src-tauri" / "tauri.conf.json"
CARGO = ROOT / "launcher" / "src-tauri" / "Cargo.toml"
LOCK = ROOT / "launcher" / "src-tauri" / "Cargo.lock"
TAG = "launcher-v"
WATCH = ("launcher", "stream/playmodes.json")
PLAIN = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


def plain(v: str):
    """(major, minor, patch) of a plain version, or None (pre-releases and odd text are None)."""
    m = PLAIN.match(v.strip().lstrip("v"))
    return tuple(int(x) for x in m.groups()) if m else None


def floor_of(conf_version: str):
    """tauri.conf.json's version as a plain triple: its part before any "-"."""
    t = plain(conf_version.split("-", 1)[0].split("+", 1)[0])
    if t is None:
        raise SystemExit(f"tauri.conf.json has no usable version ({conf_version!r})")
    return t


def newest_stable(tags):
    """The newest plain launcher-v tag as (triple, tag), or None."""
    best = None
    for t in tags:
        t = t.strip()
        if not t.startswith(TAG):
            continue
        v = plain(t[len(TAG):])
        if v is not None and (best is None or v > best[0]):
            best = (v, t)
    return best


def next_version(conf_version: str, tags) -> str:
    """The version this ship gets: the floor, or one patch above the newest stable tag, whichever is higher."""
    floor = floor_of(conf_version)
    last = newest_stable(tags)
    if last is None or floor > last[0]:
        return "%d.%d.%d" % floor
    a, b, c = last[0]
    return f"{a}.{b}.{c + 1}"


def git(*args, cwd=ROOT) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)


def plan(root: Path = ROOT) -> dict:
    conf = json.loads((root / "launcher" / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8-sig"))["version"]
    tags = git("tag", "-l", f"{TAG}*", cwd=root).stdout.split()
    last = newest_stable(tags)
    changed = True
    if last is not None:
        d = git("diff", "--quiet", last[1], "HEAD", "--", *WATCH, cwd=root)
        changed = d.returncode != 0      # 1: changes; 128 (the tag's commit is unknown here): treat as changed
    return {"ship": "true" if changed else "false",
            "version": next_version(conf, tags) if changed else "%d.%d.%d" % max(floor_of(conf), last[0]),
            "last": last[1] if last else ""}


def stamp(version: str, root: Path = ROOT) -> None:
    """Write `version` into the three files the build reads it from (only in CI's checkout; never committed)."""
    if plain(version) is None:
        raise SystemExit(f"not a plain version: {version!r}")
    conf_p = root / "launcher" / "src-tauri" / "tauri.conf.json"
    text = conf_p.read_text(encoding="utf-8")
    new, n = re.subn(r'("version"\s*:\s*")[^"]*(")', lambda m: m.group(1) + version + m.group(2), text, count=1)
    if n != 1:
        raise SystemExit("tauri.conf.json: no version line")
    conf_p.write_text(new, encoding="utf-8")
    cargo_p = root / "launcher" / "src-tauri" / "Cargo.toml"
    text = cargo_p.read_text(encoding="utf-8")
    new, n = re.subn(r'(?m)^(version\s*=\s*")[^"]*(")', lambda m: m.group(1) + version + m.group(2), text, count=1)
    if n != 1:
        raise SystemExit("Cargo.toml: no version line")
    cargo_p.write_text(new, encoding="utf-8")
    lock_p = root / "launcher" / "src-tauri" / "Cargo.lock"
    if lock_p.is_file():
        text = lock_p.read_text(encoding="utf-8")
        new = re.sub(r'(\[\[package\]\]\nname = "leyline"\nversion = ")[^"]*(")', lambda m: m.group(1) + version + m.group(2), text, count=1)
        lock_p.write_text(new, encoding="utf-8")
    pkg_p = root / "launcher" / "package.json"
    if pkg_p.is_file():
        text = pkg_p.read_text(encoding="utf-8")
        pkg_p.write_text(re.sub(r'("version"\s*:\s*")[^"]*(")', lambda m: m.group(1) + version + m.group(2), text, count=1), encoding="utf-8")


def main(argv) -> int:
    if len(argv) >= 1 and argv[0] == "plan":
        for k, v in plan().items():
            print(f"{k}={v}")
        return 0
    if len(argv) == 2 and argv[0] == "stamp":
        stamp(argv[1])
        print(f"version {argv[1]}")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
