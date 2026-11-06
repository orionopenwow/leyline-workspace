#!/usr/bin/env python3
"""Boss Rush on this PC (docs/BOSS-RUSH.md): what is set up, read-only.

    python -I tools/boss-rush/boss_rush.py check [--app <Leyline data folder>]

The exact engine: Boss Rush's install (<app>/engine-bossrush: its state, its maps marker and how many map files,
its database folder), the shared engine files (<app>/engine/dist: the bossrush profile, map_extractor's -m), the
engine running or not (the ports). The light version: Den Lite's program and its Dens folder, the last fights'
results. The board (<app>/bossrush/board.tsv). Nothing is changed, started or sent; no password is printed.
Standard library only. Exit 0 when every line is OK or a plain "not yet" (Boss Rush not set up is not a failure).
"""
from __future__ import annotations

import json
import os
import socket
import sys

MAPS = (249, 615, 616, 624, 631, 724)          # launcher host/bossrush.rs maps()
PORTS = (("database", 13306), ("login", 3724), ("world", 8085))


def app_dir(argv) -> str:
    if "--app" in argv:
        i = argv.index("--app")
        if i + 1 < len(argv):
            return argv[i + 1]
    base = os.environ.get("APPDATA") or os.path.expanduser("~/.local/share")
    return os.path.join(base, "Leyline")


def port(n: int) -> bool:
    try:
        with socket.create_connection(("127.0.0.1", n), timeout=0.3):
            return True
    except OSError:
        return False


def map_files(maps_dir: str) -> tuple[int, int]:
    """(catalog map files, other map files)."""
    ours = other = 0
    try:
        names = os.listdir(maps_dir)
    except OSError:
        return 0, 0
    for n in names:
        if n.endswith(".map") and len(n) == 11 and n[:7].isdigit():
            if int(n[:3]) in MAPS:
                ours += 1
            else:
                other += 1
    return ours, other


def check(app: str) -> list[tuple[str, str, str]]:
    """(OK | NOT YET | WARN, what, detail) lines."""
    out = []
    dist = os.path.join(app, "engine", "dist")
    root = os.path.join(app, "engine-bossrush")
    eng = {}
    try:
        with open(os.path.join(dist, "engine.json"), encoding="utf-8") as f:
            eng = json.load(f)
    except (OSError, ValueError):
        pass
    if not eng:
        out.append(("NOT YET", "engine files", "not on this PC yet (Boss Rush's Set up downloads them once, shared with Single player)"))
    else:
        out.append(("OK", "engine files", f"version {eng.get('version', '?')}"))
        prof = os.path.isfile(os.path.join(dist, "realm", "profiles", "bossrush.env"))
        out.append(("OK" if prof else "WARN", "bossrush profile", "in the engine" if prof else "this engine is older than Boss Rush: press Update the engine"))
        out.append(("OK" if eng.get("map_filter") else "WARN", "lighter map read",
                    "map_extractor reads only the raid maps" if eng.get("map_filter") else "an older engine: every map is read, then trimmed"))
    state = {}
    try:
        with open(os.path.join(root, "state", "state.json"), encoding="utf-8") as f:
            state = json.load(f)
    except (OSError, ValueError):
        pass
    if not state:
        out.append(("NOT YET", "Boss Rush install", f"not set up ({root})"))
    else:
        done = [k for k in ("db_made", "filled", "account_made") if state.get(k)]
        out.append(("OK" if len(done) == 3 else "NOT YET", "Boss Rush install", f"{len(done)} of 3 set-up steps done, login {state.get('account', '-')}"))
    ours, other = map_files(os.path.join(root, "data", "maps"))
    if ours or other:
        out.append(("OK" if other <= 16 else "WARN", "raid maps", f"{ours} map files of the catalog's maps, {other} others (the start tiles are 8)"))
    for name, n in PORTS:
        out.append(("OK", f"{name} port {n}", "answering" if port(n) else "not running"))
    dens = os.path.join(app, "den-lite", "state", "dens")
    lite = any(os.path.isfile(os.path.join(app, "den-lite", "bin", n)) for n in ("den-lite.exe", "den-lite"))
    out.append(("OK" if lite else "NOT YET", "light version", "Den Lite is on this PC" if lite else "Den Lite downloads on first use"))
    try:
        with open(os.path.join(dens, "bossrush-results.tsv"), encoding="utf-8") as f:
            runs = [l.split("\t") for l in f.read().splitlines() if l.count("\t") == 7]
        clears = sum(1 for r in runs if r[6] == "clear")
        out.append(("OK", "light results", f"{len(runs)} runs, {clears} clears"))
    except OSError:
        out.append(("OK", "light results", "none yet"))
    try:
        with open(os.path.join(app, "bossrush", "board.tsv"), encoding="utf-8") as f:
            n = sum(1 for l in f if l.strip())
        out.append(("OK", "board", f"{n} runs kept"))
    except OSError:
        out.append(("OK", "board", "empty"))
    return out


def main(argv) -> int:
    if not argv or argv[0] != "check":
        print(__doc__)
        return 2
    lines = check(app_dir(argv))
    for state, what, detail in lines:
        print(f"{state:8} {what}: {detail}")
    bad = [l for l in lines if l[0] == "WARN"]
    print("BOSS RUSH CHECK " + ("OK" if not bad else f"OK with {len(bad)} warning(s)"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
