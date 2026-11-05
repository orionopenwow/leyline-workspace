"""Put this repo's in-game UI (stream/fui-web/Interface/AddOns: WrathUnleashedUI, LooseBags) into the Den's own game
folder on this PC, for testing before a release (owner 5 Nov: "auto smoke tests with live gameplay ... run until it
works"). The public addons step (addons/pack) stays the way players get it; this only updates this PC's game.

A copy of each addon folder as it was goes to <game>\\Interface\\AddOns\\.leyline-backup\\<time>\\ first; --undo puts
the newest back. Only our own addons are touched; the game must be closed (files in use are refused)."""
import argparse
import shutil
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import den_check  # noqa: E402

SRC = ROOT / "stream" / "fui-web" / "Interface" / "AddOns"
OURS = ["WrathUnleashedUI", "LooseBags"]
RETIRED = {"Nameplates.lua"}


def game_addons():
    c = den_check.den_game(den_check.settings(den_check.app_dir()))
    if not c:
        return None
    return Path(c["path"]) / "Interface" / "AddOns"


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--undo", action="store_true")
    a = ap.parse_args(argv)
    dst = game_addons()
    if not dst:
        print("No 3.3.5a game folder in the launcher.")
        return 1
    bak_root = dst / ".leyline-backup"
    if a.undo:
        last = sorted(p for p in bak_root.glob("*") if p.is_dir())[-1:] if bak_root.is_dir() else []
        if not last:
            print("No backup to put back.")
            return 1
        for name in OURS:
            if (last[0] / name).is_dir():
                shutil.rmtree(dst / name, ignore_errors=True)
                shutil.copytree(last[0] / name, dst / name)
        print("Put back the UI from %s" % last[0].name)
        return 0
    bak = bak_root / time.strftime("%Y%m%d-%H%M%S")
    n = 0
    for name in OURS:
        src = SRC / name
        if not src.is_dir():
            continue
        if (dst / name).is_dir():
            shutil.copytree(dst / name, bak / name)
        for f in src.rglob("*"):
            rel = f.relative_to(src)
            if f.is_dir() or rel.parts[0] in ("tests",) or f.name in RETIRED or f.name.lower().startswith("readme"):
                continue
            out = dst / name / rel
            out.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(f, out)
            n += 1
    print("UI from this repo put in %s: %d files (backup: %s)" % (dst, n, bak))
    print("UI-LOCAL OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
