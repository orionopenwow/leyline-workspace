#!/usr/bin/env python3
"""The bots' compatibility guard (owner 4 Nov, Single player Phase 4: "bots always up to date").

ov_companions.lua (realm/custom/lua_scripts) has no bot AI of its own: it drives mod-playerbots through the player's
own chat commands (`.playerbots bot addclass|add|init=auto|remove`), whispers to the bot (`follow`, `summon`,
`talents spec ...`, `co +passive`...), mod-ale's playerbot methods (Player:GetBotAI, PlayerbotAI:HandleCommand...)
and the AC_PLAYERBOTS_* settings in realm/modules.env and realm/profiles/*.env. Upstream renames any of these from
time to time, and nothing fails loudly: a renamed config key is just ignored (the loot keys that make bots always
pass would stop working), a renamed whisper is answered by nothing. This reads what the script and the env files
use, reads what a mod-playerbots (and mod-ale) source tree has, and lists what is missing. The weekly canary
(.github/workflows/bots-canary.yml) runs it on the newest upstream before the long build; the smoke test
(realm/bots/bots_smoke.py) asks the running realm the same about the methods.

    python3 realm/bots/compat.py --playerbots DIR [--ale DIR] [--repo ROOT] [--json]
    python3 realm/bots/compat.py --needs [--repo ROOT]        what the script needs (JSON), nothing checked

Exit 0: nothing missing. Exit 1: something missing (listed). Exit 2: a tree doesn't look like the module.
Standard library only; reads files, runs nothing from the trees it reads.
"""
import argparse
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))

# mod-ale methods the script cannot do without (botsBuilt, the bot's AI, talking to it): missing = the canary fails.
# Every other method the script calls through try()/hasMethod() has a fallback in the script, so a missing one is
# only reported.
REQUIRED_METHODS = ("IsPlayerbot", "RunCommand", "GetBotAI", "HandleCommand", "ChangeStrategy", "GetMaster",
                    "IsAddclassBot", "IsTank", "IsHealer", "Whisper")
# env keys that are AzerothCore's own (the core reads them), not mod-playerbots'
NOT_PLAYERBOTS = ("AC_PLAYERBOTS_DATABASE_INFO",)


def ini_key_to_env(key):
    """AzerothCore's IniKeyToEnvVarKey (src/common/Configuration/Config.cpp), as tools/playerbots-specs does."""
    out = []
    n = len(key)
    for i, c in enumerate(key):
        if c in " .-":
            out.append("_")
            continue
        if i < n - 1:
            nxt = key[i + 1]
            if (not c.isupper() and nxt.isupper()) or (not c.isdigit() and nxt.isdigit()) or (c.isdigit() and not nxt.isdigit()):
                out.append(c.upper() + "_")
                continue
        out.append(c.upper())
    return "AC_" + "".join(out)


def _read(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        return f.read()


def _words(text):
    return text.strip().split()


# ---------------------------------------------------------------------------- what the script uses
def script_needs(lua):
    """What ov_companions.lua sends to mod-playerbots and asks of mod-ale."""
    code = "\n".join(l.split("--", 1)[0] if not l.lstrip().startswith("--") else "" for l in lua.splitlines())
    sub = {w for t in re.findall(r'botCommand\(\s*[\w.]+\s*,\s*"([^"]+)"', code) for w in _words(t)[:1]}
    chat = {_words(t)[0] for t in re.findall(r'\btell\(\s*[\w.]+\s*,\s*[\w.]+\s*,\s*"([^"]+)"', code) if _words(t)}
    chat |= {_words(t)[0] for t in re.findall(r'\bwhisper\(\s*[\w.]+\s*,\s*[\w.]+\s*,\s*"([^"]+)"', code) if _words(t)}
    strat = {s.strip() for s in re.findall(r'\bstrategy\(\s*[\w.]+\s*,\s*[\w.]+\s*,\s*"[+-]([^"]+)"', code)}
    forms = set()
    m = re.search(r"local FORMATIONS\s*=\s*\{(.*?)\}", code, re.S)
    if m:
        forms = set(re.findall(r"(\w+)\s*=\s*true", m.group(1)))
    methods = set(re.findall(r'\b(?:try|hasMethod)\(\s*[\w.]+\s*,\s*"(\w+)"', code))
    specs = set()
    m = re.search(r"local FALLBACK_SPECS\s*=\s*\{(.*?)\n\}", code, re.S)
    if m:
        specs = set(re.findall(r'"([a-z ]+ pve)"', m.group(1)))
    m = re.search(r"local ROLES\s*=\s*\{(.*?)\n\}", code, re.S)
    if m:
        specs |= set(re.findall(r'"([a-z ]+ pve)"', m.group(1)))
    return {"bot_commands": sorted(sub), "whispers": sorted(chat), "strategies": sorted(strat),
            "formations": sorted(forms), "methods": sorted(methods), "required_methods": sorted(REQUIRED_METHODS),
            "specs": sorted(specs)}


def env_keys(repo):
    """The AC_PLAYERBOTS_* keys the realm sets: modules.env, profiles/*.env, playerbots-specs.env (file -> keys)."""
    realm = os.path.join(repo, "realm")
    files = [os.path.join(realm, "modules.env")]
    prof = os.path.join(realm, "profiles")
    if os.path.isdir(prof):
        files += [os.path.join(prof, f) for f in sorted(os.listdir(prof)) if f.endswith(".env")]
    files.append(os.path.join(realm, "playerbots-specs.env"))
    out = {}
    for f in files:
        if not os.path.isfile(f):
            continue
        for line in _read(f).splitlines():
            m = re.match(r"\s*(AC_PLAYERBOTS_[A-Z0-9_]+)\s*=", line)
            if m and m.group(1) not in NOT_PLAYERBOTS:
                out.setdefault(m.group(1), os.path.relpath(f, repo).replace(os.sep, "/"))
    return out


def needs(repo):
    lua = _read(os.path.join(repo, "realm", "custom", "lua_scripts", "ov_companions.lua"))
    n = script_needs(lua)
    n["env"] = sorted(env_keys(repo))
    return n


# ---------------------------------------------------------------------------- what a mod-playerbots tree has
def _walk(root, exts=(".h", ".cpp")):
    for d, dirs, files in os.walk(root):
        dirs[:] = [x for x in dirs if x != ".git"]
        for f in files:
            if f.endswith(exts):
                yield os.path.join(d, f)


def playerbots_has(root):
    src = os.path.join(root, "src")
    conf = os.path.join(root, "conf", "playerbots.conf.dist")
    if not os.path.isdir(src) or not os.path.isfile(conf):
        raise ValueError(f"{root}: no src/ or conf/playerbots.conf.dist (not a mod-playerbots tree?)")
    chat, strat, forms, sub, keys = set(), set(), set(), set(), set()
    found_mgr = False
    for p in _walk(src):
        name = os.path.basename(p)
        text = _read(p)
        if name.startswith("Chat") and "Context" in name:
            chat |= set(re.findall(r'creators\["([^"]+)"\]', text))
        if "StrategyContext" in name or name.endswith("AiObjectContext.cpp") or name.endswith("AiObjectContext.h"):
            strat |= set(re.findall(r'creators\["([^"]+)"\]', text))
        if name.startswith("Formations"):
            forms |= set(re.findall(r'formation\s*==\s*"(\w+)"', text))
            forms |= set(re.findall(r'Formation\(\s*botAI\s*,\s*"(\w+)"', text))
        if name == "PlayerbotMgr.cpp":
            found_mgr = True
            sub |= set(re.findall(r'\bcmd\s*==\s*"([\w=]+)"', text))
            sub |= set(re.findall(r'strn?cmp\(\s*cmd\s*,\s*"([\w=]+)"', text))
            sub |= set(re.findall(r'"(init=\w+)"', text))
        keys |= set(re.findall(r'GetOption<[^>]+>\(\s*"([\w.]+)"', text))
    if not found_mgr:
        raise ValueError(f"{root}: no PlayerbotMgr.cpp (not a mod-playerbots tree?)")
    conf_text = _read(conf)
    keys |= set(re.findall(r"^\s*#?\s*((?:Playerbots|AiPlayerbot)\.[\w.]+)\s*=", conf_text, re.M))
    keys |= set(re.findall(r"^\s*(PlayerbotsDatabaseInfo)\s*=", conf_text, re.M))
    specs = set(v.strip().strip('"') for v in re.findall(r"^\s*Playerbots\.PremadeSpecName\.\d+\.\d+\s*=\s*(.+?)\s*$", conf_text, re.M))
    # GetOption("Playerbots.PremadeSpecName." + ...) keys are built at run time: the conf's own lines carry them
    return {"whispers": chat, "strategies": strat, "formations": forms, "bot_commands": sub,
            "env": {ini_key_to_env(k) for k in keys}, "specs": specs,
            "env_prefixes": {ini_key_to_env(k) for k in keys if k.endswith(".")}}


def ale_has(root):
    found = set()
    seen = False
    for p in _walk(os.path.join(root, "src")):
        if os.path.basename(p).startswith("LuaFunctions"):
            seen = True
            found |= set(re.findall(r'\{\s*"(\w+)"\s*,\s*&', _read(p)))
    if not seen:
        raise ValueError(f"{root}: no src/**/LuaFunctions*.cpp (not a mod-ale tree?)")
    return found


# ---------------------------------------------------------------------------- the check
def check(n, pb, ale=None):
    """Missing things, as a list of (kind, name, fatal)."""
    miss = []
    for k in ("bot_commands", "whispers", "strategies", "formations"):
        miss += [(k, x, True) for x in n[k] if x not in pb[k]]
    miss += [("specs", x, True) for x in n["specs"] if x not in pb["specs"]]
    for e in n["env"]:
        if e in pb["env"]:
            continue
        # premade spec lines (AC_PLAYERBOTS_PREMADE_SPEC_NAME_1__0): numbered keys; their names are the conf's own
        m = re.match(r"(AC_PLAYERBOTS_PREMADE_SPEC_(?:NAME|GLYPH|LINK))_\d", e)
        if m and any(k.startswith(m.group(1) + "_") for k in pb["env"]):
            continue
        miss.append(("env", e, True))
    if ale is not None:
        req = set(n["required_methods"])
        miss += [("methods", x, x in req) for x in n["methods"] if x not in ale]
        miss += [("methods", x, True) for x in sorted(req - set(n["methods"])) if x not in ale]
    return miss


def report(miss):
    fatal = [m for m in miss if m[2]]
    lines = []
    if not miss:
        lines.append("bots compat: OK (every command, whisper, strategy, formation, spec, setting and method is there)")
    for kind, name, bad in miss:
        lines.append(f"  {'MISSING' if bad else 'note   '} {kind:13} {name}")
    if fatal:
        lines.append(f"bots compat: FAILED ({len(fatal)} missing). ov_companions.lua or the env files need a change "
                     "before these pins can be promoted (realm/companions-README.md).")
    elif miss:
        lines.append("bots compat: OK (notes only: the script has a fallback for those)")
    return "\n".join(lines)


def main(argv=None):
    ap = argparse.ArgumentParser(description="Does a mod-playerbots (and mod-ale) tree still have what ov_companions.lua uses?")
    ap.add_argument("--playerbots", help="a mod-playerbots source tree")
    ap.add_argument("--ale", help="a mod-ale source tree (optional)")
    ap.add_argument("--repo", default=REPO, help="this repository (default: the one this file is in)")
    ap.add_argument("--needs", action="store_true", help="print what the script needs, as JSON, and stop")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args(argv)
    n = needs(a.repo)
    if a.needs:
        print(json.dumps(n, indent=1))
        return 0
    if not a.playerbots:
        ap.error("--playerbots DIR is needed (or --needs)")
    try:
        pb = playerbots_has(a.playerbots)
        ale = ale_has(a.ale) if a.ale else None
    except ValueError as e:
        print(f"bots compat: {e}")
        return 2
    miss = check(n, pb, ale)
    if a.json:
        print(json.dumps({"ok": not any(m[2] for m in miss), "missing": [{"kind": k, "name": x, "fatal": f} for k, x, f in miss]}))
    else:
        print(report(miss))
    return 1 if any(m[2] for m in miss) else 0


if __name__ == "__main__":
    sys.exit(main())
