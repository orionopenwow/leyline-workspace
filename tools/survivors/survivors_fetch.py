#!/usr/bin/env python3
"""Azeroth Survivors: fetch the CI build onto the owner's PC (run by "Record Survivors files.bat"; docs/SURVIVORS.md).

The workflow tools/github-workflows/survivors.yml builds the upstream browser build and the upstream game host for
Windows at the locked commit and puts them on the release "survivors-latest" of the workspace repo, each file with a
.sha256. This downloads them, checks every hash, and lays them out where the recorder looks:

    tools/survivors/dist/                      the build (survivors-dist.tgz unpacked; checked file by file)
    tools/survivors/bin/wenilla-host.exe       the upstream host (and mpqx.exe)

    py -3 -I survivors_fetch.py [--base <release download URL>] [--force]

The lock: the downloaded upstream.lock.json must name the same upstream repo and commit as
stream/survivors/upstream.lock.json. If the repo's lock already lists the build's files they must be the same; if it
lists none yet, the downloaded lock is written into it (then ship it, so the server checks against it too).
Already there and the same: nothing is downloaded again. Standard library only (Python 3.9+).
Last line "FETCH OK" or an error (exit 1). Never downloads game data: the release holds none.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import shutil
import sys
import tarfile
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
LOCK = os.path.join(ROOT, "stream", "survivors", "upstream.lock.json")
BASE = "https://github.com/orionopenwow/leyline-workspace/releases/download/survivors-latest/"
ASSETS = ("survivors-dist.tgz", "upstream.lock.json", "wenilla-host-windows-x64.exe", "mpqx-windows-x64.exe")
MAX = 1 << 30


def fail(msg: str) -> int:
    print(f"ERROR: {msg}", file=sys.stderr)
    return 1


def get(url: str, limit: int = MAX) -> bytes:
    if not (url.startswith("https://") or re.match(r"^http://127\.0\.0\.1:\d+/", url)):
        raise ValueError("downloads are https only")
    req = urllib.request.Request(url, headers={"User-Agent": "leyline-survivors-fetch"})
    with urllib.request.urlopen(req, timeout=120) as r:
        data = r.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f"{url}: bigger than expected")
    return data


def checked(base: str, name: str) -> bytes:
    """The asset, after its .sha256 says it is whole."""
    want = get(base + name + ".sha256", 4096).decode("ascii", "replace").split()
    if not want or not re.fullmatch(r"[0-9a-f]{64}", want[0]):
        raise ValueError(f"{name}.sha256 is not a checksum")
    data = get(base + name)
    if hashlib.sha256(data).hexdigest() != want[0]:
        raise ValueError(f"{name} does not match its .sha256 (a broken download?)")
    return data


def unpack(tgz: bytes, dest: str) -> None:
    """The build archive into dest: plain files and folders only, nothing outside dest."""
    tmp = dest + ".new"
    shutil.rmtree(tmp, ignore_errors=True)
    os.makedirs(tmp)
    with tarfile.open(fileobj=io.BytesIO(tgz), mode="r:gz") as t:
        for m in t.getmembers():
            name = m.name[2:] if m.name.startswith("./") else m.name
            if name in ("", "."):
                continue
            parts = name.split("/")
            if name.startswith("/") or ".." in parts or ":" in name or "\\" in name:
                raise ValueError(f"the build archive has an unsafe path: {m.name}")
            out = os.path.join(tmp, *parts)
            if m.isdir():
                os.makedirs(out, exist_ok=True)
            elif m.isfile():
                os.makedirs(os.path.dirname(out), exist_ok=True)
                with t.extractfile(m) as src, open(out, "wb") as f:
                    shutil.copyfileobj(src, f)
            else:
                raise ValueError(f"the build archive has a link or device: {m.name}")
    shutil.rmtree(dest, ignore_errors=True)
    os.replace(tmp, dest)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--base", default=BASE)
    ap.add_argument("--force", action="store_true", help="download again even if what is here matches")
    ap.add_argument("--lock", default=LOCK, help=argparse.SUPPRESS)    # tests
    ap.add_argument("--into", default=HERE, help=argparse.SUPPRESS)    # tests
    a = ap.parse_args(argv)
    lock_path, into = a.lock, a.into
    base = a.base if a.base.endswith("/") else a.base + "/"
    sys.path.insert(0, HERE)
    import survivors_check as chk   # noqa: E402  (beside this file)
    try:
        with open(lock_path, encoding="utf-8") as f:
            mine = json.load(f)
        print("== the lock from the build")
        theirs = json.loads(checked(base, "upstream.lock.json"))
    except (OSError, ValueError) as e:
        return fail(str(e))
    if theirs.get("repo") != mine.get("repo") or theirs.get("sha") != mine.get("sha"):
        return fail(f"the release was built from {theirs.get('repo')} at {theirs.get('sha')}, not the locked "
                    f"{mine.get('repo')} at {mine.get('sha')}: wait for the survivors build to finish, then run this again")
    have = (mine.get("dist") or {}).get("files") or {}
    got = (theirs.get("dist") or {}).get("files") or {}
    if not got:
        return fail("the release's lock lists no build files")
    if have and have != got:
        return fail("the release's build is not the one stream/survivors/upstream.lock.json pins: get the latest repo "
                    "(UPDATE.bat), then run this again")
    if not have:
        with open(lock_path, "w", encoding="utf-8", newline="\n") as f:
            f.write(json.dumps(theirs, indent=1) + "\n")
        print("  stream/survivors/upstream.lock.json now lists the build (ship it, so the server checks the same)")
    dist = os.path.join(into, "dist")
    if a.force or chk.check_dist(dist, lock_path):
        print("== the browser build (survivors-dist.tgz)")
        try:
            unpack(checked(base, "survivors-dist.tgz"), dist)
        except (OSError, ValueError, tarfile.TarError) as e:
            return fail(str(e))
        bad = chk.check_dist(dist, lock_path)
        if bad:
            return fail("the unpacked build does not match the lock: " + "; ".join(bad[:5]))
    else:
        print("== the browser build: already here, as the lock says")
    os.makedirs(os.path.join(into, "bin"), exist_ok=True)
    for asset, local in (("wenilla-host-windows-x64.exe", "wenilla-host.exe"), ("mpqx-windows-x64.exe", "mpqx.exe")):
        p = os.path.join(into, "bin", local)
        try:
            want = get(base + asset + ".sha256", 4096).decode("ascii", "replace").split()[0]
            if not a.force and os.path.isfile(p) and chk.sha256(p) == want:
                print(f"== {local}: already here")
                continue
            print(f"== {local}")
            data = checked(base, asset)
        except (OSError, ValueError, IndexError) as e:
            return fail(f"{asset}: {e}")
        with open(p + ".new", "wb") as f:
            f.write(data)
        os.replace(p + ".new", p)
    print("FETCH OK: tools\\survivors\\dist and tools\\survivors\\bin are the locked build")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
