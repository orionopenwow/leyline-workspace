#!/usr/bin/env python3
"""The bots' pins: which mod-playerbots and which core (its AzerothCore fork) the realm images are built from
(realm/custom/sources.lock), and realm/bots.json, the small manifest the Host pack and the launcher read
("Bots updated", and the Single player page's "Update the bots"; docs/BOTS-UPDATES.md).

    python3 realm/bots/pins.py show                          the pinned commits (name commit), one a line
    python3 realm/bots/pins.py manifest [--date D] [--changes N]   writes realm/bots.json from sources.lock
    python3 realm/bots/pins.py check                         exit 1 when realm/bots.json and sources.lock disagree
    python3 realm/bots/pins.py bump --playerbots SHA --core SHA [--date D] [--changes N]
                                                             a canary: both pins moved, bots.json rewritten
    python3 realm/bots/pins.py same --playerbots SHA --core SHA   exit 0 when those are the pins already

Only these two lines of sources.lock ever change here; the other modules keep their pins. Promotion to the stable
images is always the owner merging the canary's pull request (wu-images.yml builds :stable from main).
"""
import argparse
import datetime
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
LOCK = os.path.join(REPO, "realm", "custom", "sources.lock")
MANIFEST = os.path.join(REPO, "realm", "bots.json")
NAMES = {"playerbots": "mod-playerbots", "core": "core"}
SHA = re.compile(r"^[0-9a-f]{40}$")


def read_lock(text):
    """name -> (repo, branch, commit) for every line of a sources.lock."""
    out = {}
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        parts = line.split()
        if len(parts) == 4:
            out[parts[0]] = tuple(parts[1:])
    return out


def pins(text):
    lock = read_lock(text)
    missing = [n for n in NAMES.values() if n not in lock]
    if missing:
        raise ValueError("sources.lock has no line for " + ", ".join(missing))
    return {k: lock[n][2] for k, n in NAMES.items()}


def bump(text, playerbots, core):
    """sources.lock with the mod-playerbots and core commits replaced (nothing else touched)."""
    for v in (playerbots, core):
        if not SHA.match(v or ""):
            raise ValueError(f"not a full commit: {v!r}")
    want = {"mod-playerbots": playerbots, "core": core}
    out = []
    for line in text.splitlines(keepends=True):
        parts = line.split()
        if len(parts) == 4 and parts[0] in want and not line.lstrip().startswith("#"):
            nl = "\r\n" if line.endswith("\r\n") else "\n" if line.endswith("\n") else ""
            line = " ".join(parts[:3] + [want.pop(parts[0])]) + nl
        out.append(line)
    if want:
        raise ValueError("sources.lock has no line for " + ", ".join(want))
    return "".join(out)


def manifest(text, date=None, changes=None):
    p = pins(text)
    m = {"format": 1, "playerbots": p["playerbots"], "core": p["core"],
         "date": date or datetime.date.today().isoformat()}
    if changes is not None:
        m["changes"] = int(changes)
    return m


def write_manifest(m, path=MANIFEST):
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        json.dump(m, f, indent=1)
        f.write("\n")


def main(argv=None):
    ap = argparse.ArgumentParser(description="The bots' pins in sources.lock and realm/bots.json.")
    ap.add_argument("--lock", default=LOCK)
    ap.add_argument("--manifest-file", default=MANIFEST)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("show")
    sub.add_parser("check")
    for name in ("manifest", "bump", "same"):
        s = sub.add_parser(name)
        if name != "manifest":
            s.add_argument("--playerbots", required=True)
            s.add_argument("--core", required=True)
        if name != "same":
            s.add_argument("--date")
            s.add_argument("--changes", type=int)
    a = ap.parse_args(argv)
    with open(a.lock, encoding="utf-8", newline="") as f:
        text = f.read()
    if a.cmd == "show":
        for k, v in pins(text).items():
            print(k, v)
        return 0
    if a.cmd == "same":
        return 0 if pins(text) == {"playerbots": a.playerbots, "core": a.core} else 1
    if a.cmd == "check":
        try:
            have = json.load(open(a.manifest_file, encoding="utf-8"))
        except (OSError, ValueError) as e:
            print(f"bots.json: {e}")
            return 1
        p = pins(text)
        bad = [k for k in p if have.get(k) != p[k]]
        if bad or have.get("format") != 1:
            print("realm/bots.json does not match sources.lock (" + ", ".join(bad or ["format"]) + "): run pins.py manifest")
            return 1
        print("bots.json matches sources.lock")
        return 0
    if a.cmd == "bump":
        text = bump(text, a.playerbots, a.core)
        with open(a.lock, "w", encoding="utf-8", newline="") as f:
            f.write(text)
    write_manifest(manifest(text, a.date, a.changes), a.manifest_file)
    return 0


if __name__ == "__main__":
    sys.exit(main())
