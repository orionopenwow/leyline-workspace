"""The main realm's settings stay in one story (review 4 Nov 2026):

  * every KEY=VALUE realm/serverpass.sh step 3 writes into the live profile is in realm/profiles/main.env with the
    same value (otherwise a fresh deploy from main.env and the live realm differ, and the next serverpass restarts
    the world server for a "change" nobody made);
  * main.env sets each key once (a later duplicate silently wins in docker's env_file);
  * the owner's Wrath+ calls are what main.env says: cross-faction everything, max weapon skills, instant mail,
    3x XP / 2x reputation, Death Knights from the first character, no bots, events off, duels on, a boost kit cap;
  * mod-solo-lfg and mod-autobalance (realm/modules.env) are on, so .solo tells the truth;
  * the Dungeon Finder's level-10 maps (serverpass step 2) match ov_qol.lua's .solo list;
  * every WrathUnleashedUI .lua file is in its .toc, or is the retired Nameplates.lua (empty, owner 3 Nov);
  * the Lua CI job (.github/workflows/lua-check.yml) only names test files that exist.

    python3 -I stream/tests/test_main_realm_config.py        (from the repo root, or any directory)
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
fails = count = 0


def check(name, ok, extra=""):
    global fails, count
    count += 1
    print(("PASS " if ok else "FAIL ") + name + (f"  {extra}"[:400] if extra and not ok else ""))
    fails += 0 if ok else 1


def read(*parts):
    with open(os.path.join(ROOT, *parts), encoding="utf-8") as f:
        return f.read().replace("\r\n", "\n")


def env_lines(text):
    out = []
    for line in text.splitlines():
        m = re.match(r"^([A-Z][A-Z0-9_]*)=(.*)$", line)
        if m:
            out.append((m.group(1), m.group(2)))
    return out


main_env_text = read("realm", "profiles", "main.env")
pairs = env_lines(main_env_text)
main = dict(pairs)
sp = read("realm", "serverpass.sh")

# 1. serverpass step 3's list against main.env
m = re.search(r'echo "== 3\. profile".*?\nfor kv in (.*?); do\n', sp, re.S)
check("serverpass.sh: step 3's settings list found", m is not None)
kvs = re.findall(r"([A-Z][A-Z0-9_]*)=([^\s\\]+)", m.group(1) if m else "")
check("serverpass.sh: step 3 sets at least 30 settings", len(kvs) >= 30, len(kvs))
for k, v in kvs:
    check(f"main.env has {k}={v} (as serverpass.sh sets it live)", main.get(k) == v, f"main.env: {main.get(k)!r}")

# 2. one line per key
seen, dupes = set(), []
for k, _ in pairs:
    if k in seen:
        dupes.append(k)
    seen.add(k)
check("main.env sets every key once", not dupes, dupes)

# 3. the owner's calls, as main.env's comments describe them
want = {
    "AC_RATE_XP_KILL": "3", "AC_RATE_XP_QUEST": "3", "AC_RATE_REPUTATION_GAIN": "2", "AC_START_PLAYER_LEVEL": "1",
    "AC_ALWAYS_MAX_SKILL_FOR_LEVEL": "1", "AC_MAIL_DELIVERY_DELAY": "0", "AC_MIN_PETITION_SIGNS": "0",
    "AC_CHARACTER_CREATING_MIN_LEVEL_FOR_HEROIC_CHARACTER": "0", "OV_MAIN": "1", "OV_COMPANIONS": "0",
    "AC_PLAYERBOTS_MAX_ADDED_BOTS": "0", "OV_EVENTS": "0", "OV_DUELS": "1", "OV_BRAWL": "0", "OV_FREE_REPAIR": "0",
    "AC_INDIVIDUAL_PROGRESSION_ENABLE": "0",
}
for k, v in want.items():
    check(f"main.env: {k}={v}", main.get(k) == v, main.get(k))
for side in ("GROUP", "CHAT", "CHANNEL", "GUILD", "ARENA", "AUCTION", "CALENDAR"):
    check(f"main.env: cross-faction {side.lower()}", main.get(f"AC_ALLOW_TWO_SIDE_INTERACTION_{side}") == "1")
cap = main.get("OV_BOOST_KITS_PER_ACCOUNT", "")
check("main.env: a boost kit cap per account (1-10)", cap.isdigit() and 1 <= int(cap) <= 10, cap)
check("main.env: no starting gold with the boost", main.get("OV_BOOST_GOLD") == "0")

# 4. the modules .solo relies on
mods = dict(env_lines(read("realm", "modules.env")))
check("modules.env: mod-solo-lfg on", mods.get("AC_SOLO_LFG_ENABLE") == "1")
check("modules.env: mod-autobalance on, from one player", mods.get("AC_AUTO_BALANCE_ENABLE_GLOBAL") == "1"
      and mods.get("AC_AUTO_BALANCE_MIN_PLAYERS") == "1")

# 5. the level-10 dungeons
mm = re.search(r'^MAPS="([\d,]+)"', sp, re.M)
check("serverpass.sh: step 2's level-10 maps", mm is not None and mm.group(1) == "389,43,36,33")
qol = read("realm", "custom", "lua_scripts", "ov_qol.lua")
first = re.search(r"local SOLO_DUNGEONS = \{\s*\{ (\d+), (\d+), \"([^\"]+)\"", qol)
check(".solo's first bracket starts at the finder's level 10", first is not None and first.group(1) == "10",
      first.group(1) if first else None)
for name in ("Ragefire Chasm", "Wailing Caverns", "the Deadmines", "Shadowfang Keep"):
    check(f".solo's first bracket names {name}", first is not None and name in first.group(3))

# 6. WrathUnleashedUI's .toc
ui = os.path.join(ROOT, "stream", "fui-web", "Interface", "AddOns", "WrathUnleashedUI")
toc = read("stream", "fui-web", "Interface", "AddOns", "WrathUnleashedUI", "WrathUnleashedUI.toc")
listed = [l.strip() for l in toc.splitlines() if l.strip() and not l.startswith("#")]
check("WrathUnleashedUI.toc: ## Interface: 30300", "## Interface: 30300" in toc)
for f in listed:
    check(f"WrathUnleashedUI.toc lists {f}, which exists", os.path.isfile(os.path.join(ui, f)))
for f in sorted(os.listdir(ui)):
    if f.endswith(".lua") and f not in listed:
        body = open(os.path.join(ui, f), encoding="utf-8").read()
        retired = f == "Nameplates.lua" and all(l.startswith("--") or not l.strip() for l in body.splitlines())
        check(f"{f} is in the .toc (or retired and empty)", retired)
check("nameplates untouched: no file in the .toc styles them",
      not any(re.search(r"NamePlate|nameplate", open(os.path.join(ui, f), encoding="utf-8").read()) and
              re.search(r"SetStatusBarTexture|SetBackdrop", open(os.path.join(ui, f), encoding="utf-8").read()) and
              "NamePlate" in open(os.path.join(ui, f), encoding="utf-8").read() for f in listed if f.endswith(".lua")))

# 7. the Lua CI job only names test files that exist
ci = read(".github", "workflows", "lua-check.yml")
missing = []
for group in re.findall(r"for t in ([a-z_ ]+); do\s*\n\s*f=\"(stream/tests/test_[a-z]+_)\$t\.lua\"", ci):
    names, prefix = group
    for t in names.split():
        if not os.path.isfile(os.path.join(ROOT, prefix + t + ".lua")):
            missing.append(prefix + t + ".lua")
for path in re.findall(r"(stream/tests/test_[\w]+\.(?:lua|py))", ci):
    if not os.path.isfile(os.path.join(ROOT, path)):
        missing.append(path)
check("lua-check.yml names only test files that exist (a missing one runs nothing, silently)", not missing, missing)

print(f"{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
