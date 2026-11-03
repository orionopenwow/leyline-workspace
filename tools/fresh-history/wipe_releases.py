#!/usr/bin/env python3
"""Old launcher releases off a repo: every `launcher-v*` release and its tag except the newest one.

Owner 2 Nov 22:36: "wipe releases up until our last one now". Kept: the newest `launcher-v*` release (the one the
launcher's updater and the download page read), and every release whose tag is not `launcher-v*`: launcher-latest
(the one-file exe the site links), host-pack, host-kit, leyline-addons, den-lite-latest, forge-latest. The launcher
and the site download from those, so they are never touched.

    py -3 tools\\fresh-history\\wipe_releases.py --repo orionopenwow/Leyline          dry run: lists what would go
    py -3 tools\\fresh-history\\wipe_releases.py --repo orionopenwow/Leyline --go     deletes them (gh, as orionopenwow)

Uses the GitHub CLI signed in on this PC (never the helper account). Standard library only.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys

KEEP_PREFIX = re.compile(r"^launcher-v\d")


def gh(*args: str) -> str:
    r = subprocess.run(["gh", *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"gh {' '.join(args[:2])} failed: {r.stderr.strip()[:300]}")
    return r.stdout


def plan(releases: list[dict]) -> tuple[dict | None, list[dict]]:
    """(the newest launcher-v release kept, the launcher-v releases to delete). Other tags are never in either."""
    ver = [r for r in releases if KEEP_PREFIX.match(r.get("tag_name", "")) and not r.get("draft")]
    if not ver:
        return None, []
    newest = max(ver, key=lambda r: (r.get("published_at") or r.get("created_at") or ""))
    return newest, [r for r in ver if r is not newest]


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    ap.add_argument("--go", action="store_true")
    a = ap.parse_args(argv)
    who = gh("api", "user", "--jq", ".login").strip()
    print(f"[1/3] GitHub sign-in: {who}")
    if who.lower() == "chumpton":
        print("  The GitHub CLI is signed in as the helper account. Run: gh auth switch (to orionopenwow)")
        return 1
    releases = json.loads(gh("api", f"repos/{a.repo}/releases?per_page=100"))
    keep, gone = plan(releases)
    print(f"[2/3] {a.repo}: {len(releases)} releases")
    if keep is None:
        print("  no launcher-v releases: nothing to do")
        return 0
    print(f"  kept: {keep['tag_name']} (the newest), and every release that isn't launcher-v*: "
          + ", ".join(sorted(r["tag_name"] for r in releases if not KEEP_PREFIX.match(r.get("tag_name", "")))))
    print(f"  to delete ({len(gone)}): " + ", ".join(r["tag_name"] for r in gone))
    if not a.go:
        print("[3/3] dry run: nothing deleted")
        return 0
    n = 0
    for r in gone:
        subprocess.run(["gh", "release", "delete", r["tag_name"], "-R", a.repo, "--yes", "--cleanup-tag"], check=False)
        n += 1
    left = json.loads(gh("api", f"repos/{a.repo}/releases?per_page=100"))
    still = [r["tag_name"] for r in left if r["tag_name"] in {g["tag_name"] for g in gone}]
    print(f"[3/3] deleted {n - len(still)} of {len(gone)}; {len(left)} releases left")
    if still:
        print("  still there: " + ", ".join(still))
        return 1
    print("WIPE OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
