"""Project Reforged's graphics in YOUR Den only (owner 4 Nov 2026: "apply the 3.3.5a reforged graphics to MY local Den
only as well please").

Project Reforged (https://projectreforged.github.io/, credited with their permission) is a set of patch files for the
3.3.5a game. Leyline never hosts them: `get` downloads them from Project Reforged's own links into a folder of this PC
that the game never reads (Desktop\\Leyline\\looks\\reforged-335). `play` puts them into your game folder's Data only
while your Den runs: it links them in (a hard link: no copy, no extra disk), starts My Den the way the My Den button
does, waits until the game closes, and takes exactly those files out again. So the Official and Community realms keep
the game's normal look. `clean` takes them out if a Den session ended without that (a closed window, a crash), and
`status` says what is where.

The game's own patch letters are kept (their order matters: A before C, B + D + E together). The Den's own ground is
patch-T, which Project Reforged doesn't use. A patch file of the same name that isn't ours is never touched: `play`
stops and says so.

Not done here (each changes the game folder for every realm): Project Reforged's exe patcher (it lets the game use
more memory, which HD textures may need), DXVK, ReShade presets. See their downloads page.
"""
import argparse
import json
import os
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE.parent / "den-check"))
import den_check  # noqa: E402

BASE = "https://pub-0f05631d243e4046993fc02ca7be9542.r2.dev/Wrath%20of%20the%20Lich%20King/patches/"
# Project Reforged's 3.3.5a modules (their downloads page, 4 Nov 2026): letter, file, what, part of the default set
MODULES = [
    ("A", "patch-A.mpq", "Player characters and NPCs (HD)", True),
    ("B", "patch-B.mpq", "Buildings (with D and E)", True),
    ("C", "patch-C.mpq", "Creatures (needs A)", True),
    ("D", "patch-D.mpq", "Doodads (with B and E)", True),
    ("E", "patch-E.mpq", "Environment (with B and D)", True),
    ("G", "patch-G.mpq", "Gear and weapons", True),
    ("I", "patch-I.mpq", "Interface", True),
    ("P", "patch-P.mpq", "Spell effects", True),
    ("M", "patch-M.mpq", "Maps and loading screens", False),
    ("N", "patch-N.mpq", "Darker nights", False),
    ("U", "patch-U.mpq", "Upright orcs (needs A and C)", False),
]
STORE = ROOT / "looks" / "reforged-335"
MARK = "leyline-den-looks.json"   # in the game's Data folder while a Den session has the patches in


def game_data():
    c = den_check.den_game(den_check.settings(den_check.app_dir()))
    if not c:
        return None, None
    return c, Path(c["path"]) / "Data"


def chosen(extra):
    want = {m[0] for m in MODULES if m[3]} | {x.upper() for x in extra}
    if "B" in want or "D" in want or "E" in want:
        want |= {"B", "D", "E"}   # one set: all three or none (their page)
    if "C" in want or "U" in want:
        want.add("A")
    return [m for m in MODULES if m[0] in want]


def human(n):
    return "%.1f GB" % (n / 1e9) if n >= 1e9 else "%.0f MB" % (n / 1e6)


def get(extra):
    STORE.mkdir(parents=True, exist_ok=True)
    mods = chosen(extra)
    print("Downloading Project Reforged's patches from their own links into %s" % STORE)
    print("Made by Project Reforged: https://projectreforged.github.io/ (not hosted or checked by Leyline)")
    for letter, name, what, _ in mods:
        dest = STORE / name
        url = BASE + name
        req = urllib.request.Request(url, method="HEAD", headers={"User-Agent": "Leyline-den-looks"})
        try:
            size = int(urllib.request.urlopen(req, timeout=30).headers.get("Content-Length") or 0)
        except OSError as e:
            print("  %s: can't reach it (%s)" % (name, e))
            return 1
        if dest.is_file() and size and dest.stat().st_size == size:
            print("  %s (%s): already here, %s" % (name, what, human(size)))
            continue
        print("  %s (%s): %s ..." % (name, what, human(size) if size else "size unknown"), flush=True)
        part = dest.with_suffix(".part")
        have = part.stat().st_size if part.is_file() else 0
        headers = {"User-Agent": "Leyline-den-looks"}
        if have and size and have < size:
            headers["Range"] = "bytes=%d-" % have
        else:
            have = 0
        with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=60) as r, open(part, "ab" if have else "wb") as f:
            got, last = have, time.time()
            while True:
                b = r.read(1 << 20)
                if not b:
                    break
                f.write(b)
                got += len(b)
                if time.time() - last > 2:
                    last = time.time()
                    print("    %s of %s" % (human(got), human(size) if size else "?"), flush=True)
        if size and part.stat().st_size != size:
            print("  %s: the download stopped early: run this again, it goes on where it stopped" % name)
            return 1
        os.replace(part, dest)
        print("  %s: done" % name)
    print("Ready. Play your Den with them: \"Play my Den with Project Reforged.bat\".")
    return 0


def ours(data):
    try:
        return json.loads((data / MARK).read_text(encoding="utf-8")).get("files", [])
    except (OSError, ValueError):
        return []


def clean(data, quiet=False):
    files = ours(data)
    for f in files:
        try:
            (data / f).unlink()
        except FileNotFoundError:
            pass
        except OSError as e:
            print("Couldn't take out %s (%s): is the game still open?" % (f, e))
            return 1
    try:
        (data / MARK).unlink()
    except FileNotFoundError:
        pass
    if files and not quiet:
        print("Project Reforged's patches are out of your game folder again: %s" % ", ".join(files))
    return 0


def put_in(data, mods):
    have = [m for m in mods if (STORE / m[1]).is_file()]
    if not have:
        print("No Project Reforged patches yet: run \"Den looks - get Project Reforged.bat\" first.")
        return None
    mine = {x.lower() for x in ours(data)}
    want = {m[1].lower() for m in have}
    taken = [e for e in os.listdir(data) if e.lower() in want and e.lower() not in mine]
    if taken:
        print("Your game folder already has %s (not put there by this tool), so nothing was changed." % ", ".join(sorted(set(taken))))
        print("Those slots are used by something else (another mod?). Move them out first, or tell Claude.")
        return None
    done = []
    for _, name, _, _ in have:
        src, dst = STORE / name, data / name
        try:
            os.link(src, dst)             # a hard link: instant, no extra disk (same drive)
        except OSError:
            import shutil
            shutil.copyfile(src, dst)     # another drive: a copy
        done.append(name)
        (data / MARK).write_text(json.dumps({"files": done, "at": int(time.time())}), encoding="utf-8")
    return done


def game_running(folder=None):
    """Whether the game is running (one game at a time on this PC; Windows only)."""
    if os.name != "nt":
        return False
    try:
        out = subprocess.run(["tasklist", "/fi", "imagename eq Wow.exe", "/fo", "csv", "/nh"], capture_output=True, text=True, timeout=15).stdout
    except (OSError, subprocess.SubprocessError):
        return False
    return "wow.exe" in out.lower()


def play(extra):
    c, data = game_data()
    if not c:
        print("No 3.3.5a game folder in the launcher yet.")
        return 1
    folder = Path(c["path"])
    if game_running(folder):
        print("Your game is open: close it first (the patches can only go in while it's closed).")
        return 1
    clean(data, quiet=True)               # leftovers from a session that didn't end here
    done = put_in(data, chosen(extra))
    if done is None:
        return 1
    print("In for your Den only: %s" % ", ".join(done))
    exe = den_check.launcher_exe()
    if not exe:
        print("The launcher isn't in .launcher\\Leyline.exe: run \"Open Leyline Launcher.bat\" once.")
        clean(data)
        return 1
    print("Starting your Den (the My Den button's way). Keep this window open: it takes the patches out when the game closes.")
    subprocess.Popen([str(exe), "den-play", "--client", c["id"]])
    t0 = time.time()
    while time.time() - t0 < 120 and not game_running(folder):
        time.sleep(1)
    while game_running(folder):
        time.sleep(3)
    time.sleep(2)
    return clean(data)


def status():
    c, data = game_data()
    print("Downloaded (%s):" % STORE)
    for _, name, what, dflt in MODULES:
        p = STORE / name
        print("  %-12s %-36s %s" % (name, what, human(p.stat().st_size) if p.is_file() else "-"))
    if data:
        f = ours(data)
        print("In your game folder now: %s" % (", ".join(f) if f else "none (the normal look)"))
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("what", choices=["get", "play", "clean", "status"])
    ap.add_argument("--with", dest="extra", default="", help="optional modules too, e.g. MN (maps, darker nights)")
    a = ap.parse_args(argv)
    extra = [x for x in a.extra if x.strip()]
    if a.what == "get":
        return get(extra)
    if a.what == "play":
        return play(extra)
    if a.what == "clean":
        _, data = game_data()
        return clean(data) if data else 0
    return status()


if __name__ == "__main__":
    sys.exit(main())
