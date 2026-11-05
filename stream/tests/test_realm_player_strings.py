"""No brand words and no game client version in the strings of the realm scripts and in-game addons (review 4 Nov 2026).

Every double-quoted Lua string in the main realm's scripts (realm/custom/lua_scripts/ov_*.lua in the list below),
the browser slice rule (stream/lua/ov_slices.lua) and the in-game addons (WrathUnleashedUI, LooseBags, LeylineChat)
is checked for "World of Warcraft", "Warcraft", "Lich King", "Blizzard", "WoW" and version numbers such as 3.3.5a or
1.12. The few strings that must hold them are named in ALLOWED (the word filters themselves, the realm browser's build
badges, the host kit's server ids and the stock combat log addon's folder name). A new player line with one fails.

    python3 -I stream/tests/test_realm_player_strings.py
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SCRIPTS = ["ov_main", "ov_kits", "ov_qol", "ov_events", "ov_discord", "ov_names", "ov_reports", "ov_social", "ov_duels",
           "ov_golf", "ov_admin", "ov_services", "ov_go", "ov_mod_pings", "ov_modes", "ov_playmode", "ov_companions",
           "ov_armory", "ov_modapi"]
ADDONS = ["WrathUnleashedUI", "LooseBags", "LeylineChat"]
# file name -> strings that may hold a flagged word (filters, data keys, badges)
ALLOWED = {
    "ov_events.lua": {"world of warcraft", "warcraft", "lich king", "blizzard", "%f[%a]wow%f[%A]"},
    "ov_names.lua": {"worldofwarcraft", "warcraft", "lichking", "blizzard", "WoW", "wow"},
    "Host.lua": {"world%s*of%s*warcraft", "warcraft", "blizzard", "%f[%a]wow%f[%A]", "cmangos-1.12.1", "cmangos-2.4.3"},
    "Leyline.lua": {"1.12.1", "2.4.3", "3.3.5a", "4.3.4", "5.4.8"},          # BUILDS: the realm list's build badges
    "WrathUnleashedUI.lua": {"Blizzard_CombatLog"},                            # the stock addon's folder name
}
BRAND = re.compile(r"(?i)world\s*of\s*warcraft|warcraft|lich\s*king|blizzard|\bwow\b")
VERSION = re.compile(r"\b\d\.\d+\.\d+a?\b|\b1\.12\b|\b3\.3\.5\b")
STRING = re.compile(r'"((?:[^"\\\n]|\\.)*)"')

files = [os.path.join(ROOT, "realm", "custom", "lua_scripts", s + ".lua") for s in SCRIPTS]
files.append(os.path.join(ROOT, "stream", "lua", "ov_slices.lua"))
for a in ADDONS:
    d = os.path.join(ROOT, "stream", "fui-web", "Interface", "AddOns", a)
    files += [os.path.join(d, f) for f in sorted(os.listdir(d)) if f.endswith(".lua")]

fails = count = 0
for path in files:
    count += 1
    if not os.path.isfile(path):
        print(f"FAIL {os.path.relpath(path, ROOT)} exists")
        fails += 1
        continue
    allowed = ALLOWED.get(os.path.basename(path), set())
    bad = []
    for n, line in enumerate(open(path, encoding="utf-8", errors="replace"), 1):
        stripped = line.lstrip()
        if stripped.startswith("--"):
            continue
        for s in STRING.findall(line):
            if (BRAND.search(s) or VERSION.search(s)) and s not in allowed:
                bad.append(f"{n}: {s[:100]}")
    ok = not bad
    fails += 0 if ok else 1
    print(("PASS " if ok else "FAIL ") + f"{os.path.relpath(path, ROOT)}: no brand word or client version in its strings"
          + ("" if ok else "  " + " | ".join(bad[:5])))

print(f"{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
