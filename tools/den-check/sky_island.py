#!/usr/bin/env python3
"""Sky Island on this PC (Control: sky-island; docs/DEN-NEXT.md, "Sky Island and light dens"): the headless smoke
first (den_smoke.py: no game window), then the island's file in Your Den's folder, an open request for it, and My Den
started the way its button does (`Leyline.exe den-play`): the game signs in by itself and you land on the island.

    py -3 -I tools/den-check/sky_island.py [--no-smoke] [--no-play]

The island is made of your own game's models (Den Lite names them; nothing of the game is in this file or shipped).
Nothing here is put on GitHub Pages or any server: it is your PC's Den.
"""
import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import den_check  # noqa: E402
import den_smoke  # noqa: E402

NAME = "sky-island"


def island_code(js=None):
    """ISLAND_CODE in the launcher's ui/denpack.js (the same island Den Lite makes: a Den Lite test checks it)."""
    text = js if js is not None else (ROOT / "launcher" / "ui" / "denpack.js").read_text(encoding="utf-8")
    m = re.search(r'const ISLAND_CODE = "([^"]+)"', text)
    if not m:
        raise ValueError("no ISLAND_CODE in ui/denpack.js")
    return m.group(1)


def make_code(text):
    """A Den code for a Den file's text (raw deflate, base64url, the crc: labs/den-lite denpack.rs)."""
    import base64
    import zlib
    data = text.encode("utf-8")
    c = zlib.compressobj(9, zlib.DEFLATED, -15)
    z = c.compress(data) + c.flush()
    return "ley1.den.z.%s.%08x" % (base64.urlsafe_b64encode(z).decode().rstrip("="), zlib.crc32(data) & 0xFFFFFFFF)


def set_island_code(code, js_path=None):
    """ISLAND_CODE in the launcher's ui/denpack.js set to `code`."""
    p = Path(js_path) if js_path else ROOT / "launcher" / "ui" / "denpack.js"
    t = p.read_text(encoding="utf-8")
    new = re.sub(r'const ISLAND_CODE = "[^"]+"', 'const ISLAND_CODE = "%s"' % code, t, count=1)
    if new == t and code not in t:
        raise ValueError("no ISLAND_CODE in %s" % p)
    p.write_text(new, encoding="utf-8")
    return p


def island_file(dens, code=None, now=time.time):
    """`<dens>/sky-island.den.json` (written when missing or different) and `<dens>/open.json` asking Den Lite to open
    it the next time you're in your Den. The file's path."""
    kind, text = den_smoke.decode_code(code or island_code())
    if kind != "den" or json.loads(text).get("room", {}).get("look") != "sky":
        raise ValueError("ISLAND_CODE isn't the island")
    dens.mkdir(parents=True, exist_ok=True)
    p = dens / (NAME + ".den.json")
    if not p.is_file() or p.read_text(encoding="utf-8") != text:
        p.write_text(text, encoding="utf-8")
    (dens / "open.json").write_text(json.dumps({"file": NAME, "id": "sky%d" % int(now())}), encoding="utf-8")
    return p


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--no-smoke", action="store_true")
    ap.add_argument("--no-play", action="store_true")
    ap.add_argument("--app-dir")
    ap.add_argument("--make-code", metavar="ISLAND_JSON", help="after a change to skyisland.rs: the island's file (the Den Lite test writes it) made the launcher's ISLAND_CODE")
    a = ap.parse_args(argv)
    if a.make_code:
        code = make_code(Path(a.make_code).read_text(encoding="utf-8"))
        print("ISLAND_CODE set in %s (%d letters)" % (set_island_code(code), len(code)))
        return 0
    rep = den_check.Report()
    title = "SKY ISLAND"
    print("%s (%s)" % (title, time.strftime("%Y-%m-%d %H:%M")))
    app = Path(a.app_dir) if a.app_dir else den_check.app_dir()
    root = app / "den-lite"
    if not a.no_smoke:
        prog = root / "bin" / den_check.exe_name("den-lite")
        if rep.ok(prog.is_file(), "the Den Lite program", str(prog), "Press den-lite in Control first."):
            den_smoke.smoke(rep, prog)
    try:
        p = island_file(root / "state" / "dens")
        rep.add("PASS", "the island's file in Your Den", str(p))
    except Exception as e:  # noqa: BLE001 - plain words
        rep.add("FAIL", "the island's file", str(e)[:200], "Run ship or ui-local so the launcher's files are current, then try again.")
        return rep.end(title)
    if not a.no_play:
        exe = den_check.launcher_exe()
        c = den_check.den_game(den_check.settings(app))
        if rep.ok(exe is not None and exe.is_file(), "the launcher program", str(exe), "Run \"Open Leyline Launcher.bat\" once.") and \
                rep.ok(c is not None, "game folder for Your Den", c.get("path") if c else "", "Add your 3.3.5a game folder in the launcher."):
            if den_check.preflight(rep, Path(c["path"]), root, exe):
                subprocess.Popen([str(exe), "den-play", "--client", c["id"]], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                rep.add("INFO", "My Den started", "the game signs in by itself; in the world Den Lite opens the island and lands you by the tree")
                print("")
                print("On the island: walk off the edge (you're put back by the tree), F7 builds, target a dummy and attack,")
                print("say \"Jaina, make a quest where you find my lost lantern\", .den pack makes it a code, .den island off goes home.")
    return rep.end(title)


if __name__ == "__main__":
    sys.exit(main())
