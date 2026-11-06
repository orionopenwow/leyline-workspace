"""The solo-wasm step (docs/PHONE-SOLO.md): Den Lite's page build (den-lite-web.wasm, made by CI's den-lite-web.yml)
from the rolling release den-lite-latest into stream/www-files/solo/, checked against its .sha256 and its wasm magic.
The page step then puts it on the game page (stream/server/www-patch.py copies www-files/solo/). Downloads only our
own build; nothing else is touched. Ends "SOLO WASM OK".

    python3 tools/solo/solo_wasm.py [--dest <folder>]
"""
import argparse
import hashlib
import os
import sys
import tempfile
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
BASE = "https://github.com/orionopenwow/leyline-workspace/releases/download/den-lite-latest/"
NAME = "den-lite-web.wasm"
DEST = REPO / "stream" / "www-files" / "solo"
MAX_BYTES = 64 * 1024 * 1024


def fetch(url: str, limit: int) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": "leyline-control-solo-wasm"})
    with urllib.request.urlopen(req, timeout=120) as r:
        data = r.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f"{url}: bigger than {limit} bytes")
    return data


def check(wasm: bytes, sums: str) -> str:
    """The sha256 the release names for the wasm, checked; the wasm's own magic and version. Raises on any mismatch."""
    want = None
    for line in sums.splitlines():
        parts = line.split()
        if len(parts) == 2 and parts[1].lstrip("*") == NAME:
            want = parts[0].lower()
    if not want or len(want) != 64:
        raise ValueError("the .sha256 file doesn't name den-lite-web.wasm")
    got = hashlib.sha256(wasm).hexdigest()
    if got != want:
        raise ValueError(f"checksum mismatch: {got} != {want}")
    if wasm[:8] != b"\0asm\x01\0\0\0":
        raise ValueError("not a wasm module")
    return got


def install(wasm: bytes, dest: Path) -> Path:
    dest.mkdir(parents=True, exist_ok=True)
    out = dest / NAME
    fd, tmp = tempfile.mkstemp(dir=dest, prefix=".den-lite-web.", suffix=".tmp")
    with os.fdopen(fd, "wb") as f:
        f.write(wasm)
    os.replace(tmp, out)
    return out


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dest", default=str(DEST))
    a = ap.parse_args(argv)
    try:
        sums = fetch(BASE + NAME + ".sha256", 4096).decode("utf-8", "replace")
        wasm = fetch(BASE + NAME, MAX_BYTES)
        digest = check(wasm, sums)
        out = install(wasm, Path(a.dest))
    except Exception as e:  # one line for the log, then a failed step
        print(f"solo-wasm: {e}")
        print("Is den-lite-web.yml on GitHub and green? (setup\\Turn on GitHub builds.bat, then Actions > den-lite-web)")
        return 1
    print(f"{out} ({len(wasm) / 1048576:.1f} MB, sha256 {digest[:16]}...)")
    print("Next: the page step puts it on the game page.")
    print("SOLO WASM OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
