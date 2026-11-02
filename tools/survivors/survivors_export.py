#!/usr/bin/env python3
"""Azeroth Survivors: the exporter, run on the owner's PC after the recorder (docs/SURVIVORS.md).

Copies the recorded game file names out of the owner's OWN client into a loose-file slice, the shape the Survivors
address serves (our wenilla-host --www <dist> --patch <slice> --only-listed: one file per name, each path component
a folder, matched in any case), and writes names.sha256 beside it.

    py -3 -I survivors_export.py <recorded.txt>... --out <folder>
        [--host http://127.0.0.1:8090]                 read each file from the upstream wenilla-host the recorder ran
        [--mpqx <mpqx.exe> --data <client Data folder>] or with upstream's mpqx, one file at a time

Writes <folder>/slice/ (the files), <folder>/names.sha256 ("<sha256>  <name>", one line per file, sorted) and
<folder>/export.json (counts, bytes, what was left out). A fresh slice each time: <folder>/slice.new, swapped in at
the end, so a failed run leaves the last good slice alone.

Left out on purpose (never in the slice): Interface\\Glues\\Common\\Glues-WoW-Logo.blp (the brand rule: the game shows
no franchise logo; the verify pass shows whether the game misses it), and any name that is not a plain relative
game path.
Standard library only (Python 3.9+). Last line: "EXPORT OK: ..." or an error (exit 1).
"""
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.parse

LEAVE_OUT = {"interface\\glues\\common\\glues-wow-logo.blp"}
NAME_RE = re.compile(r"^[A-Za-z0-9 _\-.()'&+!,\[\]]+(\\[A-Za-z0-9 _\-.()'&+!,\[\]]+)*$")


def clean(name: str) -> str | None:
    """The name as a game path with backslashes, or None when it is not a plain relative path."""
    n = name.strip().replace("/", "\\")
    if not n or len(n) > 260 or not NAME_RE.fullmatch(n):
        return None
    parts = n.split("\\")
    if any(p in ("", ".", "..") or p.startswith(".") or p.endswith((" ", ".")) for p in parts):
        return None
    return n


def read_lists(paths: list[str]) -> tuple[list[str], list[str]]:
    """(names to export, in first-seen order, one per name in any case; the names refused as not plain paths)."""
    seen, out, bad = set(), [], []
    for p in paths:
        with open(p, encoding="utf-8-sig") as f:
            for line in f:
                raw = line.strip()
                if not raw or raw.startswith("#"):
                    continue
                n = clean(raw)
                if n is None:
                    bad.append(raw)
                    continue
                if n.lower() not in seen:
                    seen.add(n.lower())
                    out.append(n)
    return out, bad


class Writer:
    """Files into the slice folder, each component matched case-insensitively to what is there already (one folder
    per name in any case, as the host's --patch walk expects)."""

    def __init__(self, root: str):
        self.root = root
        self.dirs: dict[str, str] = {"": root}

    def path_for(self, name: str) -> str:
        parts = name.split("\\")
        key, cur = "", self.root
        for comp in parts[:-1]:
            key = key + "\\" + comp.lower() if key else comp.lower()
            if key not in self.dirs:
                d = os.path.join(cur, comp)
                os.makedirs(d, exist_ok=True)
                self.dirs[key] = d
            cur = self.dirs[key]
        return os.path.join(cur, parts[-1])


def fetch_http(host: str):
    u = urllib.parse.urlsplit(host)
    if u.scheme != "http" or u.hostname not in ("127.0.0.1", "localhost", "::1"):
        raise SystemExit("--host must be the wenilla-host on this PC (http://127.0.0.1:<port>)")
    conn = [None]

    def get(name: str) -> bytes | None:
        for _ in range(2):
            if conn[0] is None:
                conn[0] = http.client.HTTPConnection(u.hostname, u.port or 80, timeout=120)
            try:
                conn[0].request("GET", "/data/" + urllib.parse.quote(name, safe=""), headers={"Accept-Encoding": "identity"})
                r = conn[0].getresponse()
                data = r.read()
            except (OSError, http.client.HTTPException):
                conn[0].close()
                conn[0] = None
                continue
            if r.status == 200:
                return data
            if r.status == 404:
                return None
            raise RuntimeError(f"{name}: the host answered {r.status}")
        raise RuntimeError(f"{name}: the host on {host} is not answering")
    return get


def fetch_mpqx(exe: str, data: str):
    tmp = tempfile.mkdtemp(prefix="sv-mpqx-")

    def get(name: str) -> bytes | None:
        out = os.path.join(tmp, "f.bin")
        r = subprocess.run([exe, data, name, out], capture_output=True, text=True)
        if r.returncode != 0:
            return None   # not in the client (or delete-marked: the game does not load it either)
        with open(out, "rb") as f:
            b = f.read()
        os.remove(out)
        return b
    return get


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("lists", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--host", default="http://127.0.0.1:8090")
    ap.add_argument("--mpqx")
    ap.add_argument("--data")
    a = ap.parse_args(argv)
    names, bad = read_lists(a.lists)
    if not names:
        print("ERROR: no names in " + ", ".join(a.lists), file=sys.stderr)
        return 1
    if a.mpqx:
        if not a.data or not os.path.isdir(a.data):
            print("ERROR: --mpqx needs --data <the client's Data folder>", file=sys.stderr)
            return 1
        get = fetch_mpqx(a.mpqx, a.data)
    else:
        get = fetch_http(a.host)
    os.makedirs(a.out, exist_ok=True)
    new = os.path.join(a.out, "slice.new")
    shutil.rmtree(new, ignore_errors=True)
    os.makedirs(new)
    w = Writer(new)
    rows, left_out, missing, total = [], [], [], 0
    for i, n in enumerate(names, 1):
        if n.lower() in LEAVE_OUT:
            left_out.append(n)
            continue
        try:
            data = get(n)
        except RuntimeError as e:
            print(f"ERROR: {e}", file=sys.stderr)
            return 1
        if data is None:
            missing.append(n)
            continue
        with open(w.path_for(n), "wb") as f:
            f.write(data)
        rows.append((n, hashlib.sha256(data).hexdigest(), len(data)))
        total += len(data)
        if i % 250 == 0:
            print(f"  {i}/{len(names)} ({total / 1048576:.0f} MB)", flush=True)
    rows.sort(key=lambda r: r[0].lower())
    sums = "".join(f"{h}  {n}\n" for n, h, _ in rows)
    old = os.path.join(a.out, "slice")
    if os.path.isdir(old):
        shutil.rmtree(old + ".prev", ignore_errors=True)
        os.replace(old, old + ".prev")
    os.replace(new, old)
    with open(os.path.join(a.out, "names.sha256"), "w", encoding="utf-8", newline="\n") as f:
        f.write(sums)
    report = {"files": len(rows), "bytes": total, "names_sha256": hashlib.sha256(sums.encode()).hexdigest(),
              "left_out": left_out, "not_in_client": missing, "refused_names": bad[:50],
              "from": "mpqx" if a.mpqx else a.host}
    with open(os.path.join(a.out, "export.json"), "w", encoding="utf-8") as f:
        json.dump(report, f, indent=1)
    if missing:
        print(f"  {len(missing)} recorded name(s) are not in this client now (export.json lists them)")
    if bad:
        print(f"  {len(bad)} line(s) were not plain game paths and were skipped")
    print(f"EXPORT OK: {len(rows)} files, {total / 1048576:.1f} MB in {os.path.abspath(old)}"
          + (f"; left out: {', '.join(left_out)}" if left_out else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
