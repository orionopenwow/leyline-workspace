#!/usr/bin/env python3
"""Azeroth Survivors: survivors-check. Read-only. On the owner's PC (after the exporter, before the upload) and on the
server (hosting/survivors/sv-site-up.sh runs it before it serves anything).

    py -3 -I survivors_check.py slice <slice folder> <names.sha256>
        every name listed is there with that SHA-256, nothing else is (dot files aside), and the files the slice
        must never hold (the franchise logo) are not there
    py -3 -I survivors_check.py dist <dist folder> [<upstream.lock.json>]
        the browser build is the one the lock pins (stream/survivors/upstream.lock.json: every file and its SHA-256),
        with no file the lock does not list, and the upstream licence files are next to it (licenses/)
    py -3 -I survivors_check.py all <dist folder> <slice folder> <names.sha256> [<upstream.lock.json>]

Last line "SURVIVORS CHECK OK" (exit 0), or "SURVIVORS CHECK FAILED: <n> problem(s)" (exit 1) with each problem above.
Standard library only (Python 3.9+).
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import sys

NEVER = {"interface\\glues\\common\\glues-wow-logo.blp"}
LICENSES = ("licenses/LICENSE-MIT", "licenses/LICENSE-APACHE")
HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_LOCK = os.path.join(HERE, "..", "..", "stream", "survivors", "upstream.lock.json")
SUM_RE = re.compile(r"^([0-9a-f]{64})  (.+)$")


def sha256(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def walk(root: str) -> dict[str, str]:
    """{name in lower case: path} for every file under root, names with backslashes; dot files and folders skipped."""
    out = {}
    for d, dirs, files in os.walk(root):
        dirs[:] = [x for x in dirs if not x.startswith(".")]
        for f in files:
            if f.startswith("."):
                continue
            p = os.path.join(d, f)
            out[os.path.relpath(p, root).replace(os.sep, "\\").replace("/", "\\").lower()] = p
    return out


def check_slice(root: str, sums: str) -> list[str]:
    bad = []
    if not os.path.isdir(root):
        return [f"no slice folder at {root}"]
    want = {}
    try:
        with open(sums, encoding="utf-8") as f:
            for i, line in enumerate(f, 1):
                line = line.rstrip("\r\n")
                if not line:
                    continue
                m = SUM_RE.fullmatch(line)
                if not m:
                    bad.append(f"names.sha256 line {i} is not '<sha256>  <name>'")
                    continue
                want[m.group(2).replace("/", "\\").lower()] = m.group(1)
    except OSError as e:
        return [f"cannot read {sums}: {e}"]
    if not want:
        bad.append("names.sha256 lists no files")
    have = walk(root)
    for n, h in sorted(want.items()):
        p = have.get(n)
        if p is None:
            bad.append(f"missing from the slice: {n}")
        elif sha256(p) != h:
            bad.append(f"changed since the export: {n}")
    for n in sorted(set(have) - set(want)):
        bad.append(f"in the slice but not in names.sha256: {n}")
    for n in sorted(NEVER & (set(have) | set(want))):
        bad.append(f"must never be in the slice (brand rule): {n}")
    if not bad:
        size = sum(os.path.getsize(p) for p in have.values())
        print(f"  slice: {len(have)} files, {size / 1048576:.1f} MB, all as names.sha256 says")
    return bad


def check_dist(root: str, lock_path: str) -> list[str]:
    try:
        with open(lock_path, encoding="utf-8") as f:
            lock = json.load(f)
    except (OSError, ValueError) as e:
        return [f"cannot read the lock {lock_path}: {e}"]
    files = (lock.get("dist") or {}).get("files") or {}
    if not files:
        return [f"the lock {lock_path} lists no dist files yet"]
    if not os.path.isdir(root):
        return [f"no dist folder at {root}"]
    bad = []
    have = {}
    for d, _dirs, fs in os.walk(root):
        for f in fs:
            p = os.path.join(d, f)
            have[os.path.relpath(p, root).replace(os.sep, "/")] = p
    for name, h in sorted(files.items()):
        p = have.get(name)
        if p is None:
            bad.append(f"missing from the dist: {name}")
        elif sha256(p) != h:
            bad.append(f"not the locked build: {name}")
    for name in sorted(set(have) - set(files)):
        bad.append(f"in the dist but not in the lock: {name}")
    for name in LICENSES:
        if name not in have:
            bad.append(f"the upstream licence file is not next to the build: {name}")
    if not bad:
        print(f"  dist: {len(files)} files, the build of {lock.get('repo', '?')} at {str(lock.get('sha', '?'))[:12]}")
    return bad


def main(argv: list[str]) -> int:
    if len(argv) >= 3 and argv[0] == "slice":
        bad = check_slice(argv[1], argv[2])
    elif len(argv) >= 2 and argv[0] == "dist":
        bad = check_dist(argv[1], argv[2] if len(argv) > 2 else DEFAULT_LOCK)
    elif len(argv) >= 4 and argv[0] == "all":
        bad = check_dist(argv[1], argv[4] if len(argv) > 4 else DEFAULT_LOCK) + check_slice(argv[2], argv[3])
    else:
        print(__doc__.strip().split("\n\n")[1], file=sys.stderr)
        return 64
    for b in bad[:200]:
        print("  " + b)
    if len(bad) > 200:
        print(f"  ... and {len(bad) - 200} more")
    if bad:
        print(f"SURVIVORS CHECK FAILED: {len(bad)} problem(s)")
        return 1
    print("SURVIVORS CHECK OK")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
