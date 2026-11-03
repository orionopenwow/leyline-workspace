"""Game mods and realm mod lists (stream/gamemods.py, docs/GAME-MODS.md): the list format is checked (ids, versions, at
most 32, each once, never a URL or a path), our realm's list comes from its own config, only catalog mods with a client
part (and hashes) are ever offered to a player, the Game mods section lists real entries only, "Play it on" names live
realms only, and the built-in modes match playmodes.json, the host kit and the relay's copy of the rules.

    python3 stream/tests/test_gamemods.py
"""
import importlib.util
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
REPO = os.path.dirname(STREAM)
spec = importlib.util.spec_from_file_location("gm", os.path.join(STREAM, "gamemods.py"))
gm = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gm)

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, extra)


bad = ["https://evil.example/x.zip", "mod:../../etc", "mod:A", "file:/x", "mod:x", "up:", "mode:", "mod:dice;rm", "MOD:dice",
       "mode:golf/../x", " mod:dice", "mod:dice\n", "up:../x"]
check("clean_mods: bad ids dropped (URLs, paths, upper case, too short, junk)",
      gm.clean_mods([{"id": b} for b in bad] + bad) == [], gm.clean_mods([{"id": b} for b in bad]))
got = gm.clean_mods([{"id": "mod:dalaran-dice", "version": "0.1.0"}, "mode:minigolf", {"id": "mod:dalaran-dice"},
                     {"id": "up:skada", "version": "../x"}, {"id": "up:pfquest-wotlk", "version": "r736"}, 7, None, ["x"]])
check("clean_mods: strings or objects, each id once, a bad version dropped (the id kept)",
      got == [{"id": "mod:dalaran-dice", "version": "0.1.0"}, {"id": "mode:minigolf"}, {"id": "up:skada"},
              {"id": "up:pfquest-wotlk", "version": "r736"}], got)
check("clean_mods: at most 32", len(gm.clean_mods([f"mod:m{i:03d}" for i in range(100)])) == 32)
check("clean_mods: not a list is an empty list", gm.clean_mods("mod:x") == [] and gm.clean_mods(None) == [] and gm.clean_mods({}) == [])
check("clean_mods: never more than id and version", all(set(m) <= {"id", "version"} for m in
      gm.clean_mods([{"id": "mod:dice", "url": "https://x", "files": ["a.dll"], "code": "os.exit()"}])))

PM = json.load(open(os.path.join(STREAM, "playmodes.json")))["modes"]
live_realm = [m["id"] for m in PM if m["kind"] == "realm" and m["status"] == "live"]
check("BUILTIN_MODES are playmodes.json's live realm modes", list(gm.BUILTIN_MODES) == live_realm, live_realm)
kit = open(os.path.join(REPO, "hosting", "kit", "tools", "wuhost", "setup.py"), encoding="utf-8").read()
m = re.search(r"^BUILTIN_MODES = \(([^)]*)\)", kit, re.M)
check("the host kit's BUILTIN_MODES match", m and [x.strip().strip('"') for x in m.group(1).split(",") if x.strip()] == list(gm.BUILTIN_MODES),
      m.group(0) if m else "missing")
relay = open(os.path.join(REPO, "hosting", "relay", "wurelay", "listing.py"), encoding="utf-8").read()
for name in ("ID_RE", "VERSION_RE"):
    r = re.search(rf'^MOD_{name} = re\.compile\(r"([^"]+)"\)', relay, re.M)
    check(f"the relay's MOD_{name} is gamemods.{name}", r and r.group(1) == getattr(gm, name).pattern, r.group(1) if r else "missing")
for name in ("ID_RE", "VERSION_RE"):
    r = re.search(rf'^MOD_{name} = re\.compile\(r"([^"]+)"\)', kit, re.M)
    check(f"the host kit's MOD_{name} is gamemods.{name}", r and r.group(1) == getattr(gm, name).pattern, r.group(1) if r else "missing")
check("the relay's MAX_MODS is gamemods.MAX_MODS", re.search(rf"^MAX_MODS = {gm.MAX_MODS}$", relay, re.M) is not None)

CAT = [
    {"id": "mod:dalaran-dice", "source": "packaged", "name": "Dalaran Dice", "category": "sandbox", "kind": "both", "version": "0.1.0",
     "on_wrath": True, "download": "https://play.openvanilla.app/wrath/mods/dalaran-dice/download", "sha256": "ab" * 32},
    {"id": "mod:pings", "source": "packaged", "name": "Pings", "category": "addons", "kind": "both", "version": "0.1.0", "on_wrath": True,
     "download": "https://x/d", "sha256": "cd" * 32},
    {"id": "mod:hud", "source": "packaged", "name": "Hud", "category": "tools", "kind": "realm", "version": "1.0.0", "on_wrath": False,
     "download": "https://x/d", "sha256": "ef" * 32},
    {"id": "mod:nohash", "source": "packaged", "name": "No hash", "category": "sandbox", "kind": "ui", "version": "1.0.0", "on_wrath": False,
     "download": "https://x/d", "sha256": None},
    {"id": "up:skada", "source": "upstream", "name": "Skada", "category": "addons", "kind": "ui"},
    {"id": "link:abc", "source": "community", "name": "A link", "category": "sandbox", "kind": None},
]
off = gm.official(CAT, PM)
check("official: the live Leyline Modes, then the Workshop mods the mods step installs (with versions)",
      off == [{"id": "mode:minigolf"}, {"id": "mode:bumper-balls"}, {"id": "mode:coin-dash"}, {"id": "mode:community"},
              {"id": "mod:dalaran-dice", "version": "0.1.0"}, {"id": "mod:pings", "version": "0.1.0"}], off)
check("official: without playmodes.json, the built-in modes", gm.official([], None)[:4] == gm.builtin())

need = gm.needs_client([{"id": "mode:minigolf"}, {"id": "mod:dalaran-dice"}, {"id": "mod:hud"}, {"id": "mod:nohash"},
                        {"id": "up:skada"}, {"id": "mod:not-in-catalog"}, {"id": "link:abc"}], CAT)
check("needs_client: only catalog mods with a client part (and a hash), never realm-only ones or unknown ids",
      need == ["mod:dalaran-dice", "up:skada"], need)
check("side: modes are realm-side, upstream addons client, packaged by kind",
      gm.side("mode:x") == "realm" and gm.side("up:x") == "client" and gm.side("mod:x", {"kind": "both"}) == "both"
      and gm.side("mod:x", {"kind": "realm"}) == "realm")

ents = gm.entries(PM, CAT)
ids = [g["id"] for g in ents]
check("entries: the realm modes first, then the Workshop's game categories; nothing else (no realm types, offline games, tools)",
      ids == ["mode:minigolf", "mode:bumper-balls", "mode:coin-dash", "mode:community", "mod:dalaran-dice", "mod:nohash"], ids)
dice = next(g for g in ents if g["id"] == "mod:dalaran-dice")
check("entries: a both-kind mod needs a realm and adds client files; a mode plays through the Leyline addons",
      dice["realm"] and dice["client"] and not dice["addon"] and ents[0]["realm"] and ents[0]["addon"] and not ents[0]["client"])
coming = gm.entries([{"id": "skating", "title": "Skating", "line": "Skate.", "kind": "realm", "status": "coming"}], [])
check("entries: a real entry marked coming is Coming (and only then)", coming[0]["status"] == "coming" and
      all(g["status"] == "live" for g in ents))

realms = [{"name": "Ours", "live": True, "mods": off}, {"name": "Frost", "live": True, "mods": ["mode:coin-dash"]},
          {"name": "Down", "live": False, "mods": ["mode:minigolf"]}, {"name": "Junk", "live": True, "mods": "mode:minigolf"}]
check("runs_on: live realms whose list names it, in order",
      [r["name"] for r in gm.runs_on("mode:minigolf", realms)] == ["Ours"]
      and [r["name"] for r in gm.runs_on("mode:coin-dash", realms)] == ["Ours", "Frost"] and gm.runs_on("mod:x", realms) == [])

print(f"\n{passed}/{passed + failed} passed")
sys.exit(0 if failed == 0 else 1)
