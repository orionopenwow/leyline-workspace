#!/usr/bin/env python3
"""Mode packages: a realm mode anyone can host and fork (docs/MODES.md, "Mode packages").

A mode package is one folder:

    mode.json        what the mode is and how a realm runs it (below)
    picture.svg      its card picture (or picture.webp / picture.png), optional
    <name>.den.json  a Den file for its map (docs/DEN.md, "The save file"), optional, only with the den ruleset
    README.md        a few lines for people who fork it, optional

mode.json (format "leyline-mode", schema 1):

    id         a-z, 0-9 and single dashes, 2-32 long, starting with a letter
    name       a short name (up to 32);  line  one plain sentence (up to 120)
    picture    the picture's file name, or null
    version    1 to 4 numbers with dots ("1", "1.2.0")
    author     who made it (up to 32)
    based_on   null, or {"id", "author"}: the mode this one was forked from
    status     "live" (default) or "coming": a coming mode is shown, never hosted or forked
    profile    the ruleset it starts from: one of PROFILES (the host kit's 3.3.5a rulesets)
    scripts    the realm scripts it uses, by file name: each one ships in the Host pack (realm/custom/lua_scripts).
               A package never carries a script of its own: a mode is settings over scripts that already exist.
    env        fixed settings the mode turns on, {"OV_DUELS": "1"}: only names env_name_ok() takes
    mods       optional client mods by id (mod:<id>, up:<slug>, mode:<id>: the realm list's mod ids)
    den        the Den file's name, or null (den ruleset only)
    tags       realm list tags (blizzlike boosted custom pvp rp hangout bots)
    jam        optional: the id of the mode jam it is entered in
    credit     optional, for something that is not ours: {"by", "source": https URL, "license"?}
    settings   the host's form: at most 12 fields, each
                 {"key", "label", "help"?, "type": "number", "min", "max", "default", "unit"?, "env": NAME or [NAMES]}
                 {"key", "label", "help"?, "type": "toggle", "default": true|false, "env": ...}       ("1" / "0")
                 {"key", "label", "help"?, "type": "choice", "options": [{"value", "label"}], "default", "env": ...}
               Each field sets its env names to the value the host picked (state/realm.env, read when the world
               server starts). A field never names an env name the fixed env already sets.

Checks (check_dir, check_manifest): sizes (mode.json 16 KB, a picture 256 KB, a Den file 512 KB, README 16 KB, the
folder 1 MB, 6 files), file names (no folders, no hidden files, no links, no "..": only the names above), an SVG
with no script, no event attribute and no outside link, the words (stream/namecheck.py's one blocklist, plus the game
and publisher names: a mode's name, line, author and labels never carry them), and every script, env name, mod id
and ruleset against the lists here. The host kit's setup.py and the launcher (host/modes.rs) check the same again.

    python3 -I stream/modepack.py --check DIR...          check packages (exit 1 with the problems listed)
    python3 -I stream/modepack.py --write                 rebuild stream/modepacks.json from hosting/kit/modes
    python3 -I stream/modepack.py --verify                exit 1 when stream/modepacks.json is out of date

stream/modepacks.json is the templates' catalog: the launcher builds it in (host/modes.rs include_str!) and the site
serves the mode pages from it. For stream/wrath_portal.py it also makes the website's pages:

    GET /wrath/modes/<id>               a mode: its picture, what it sets, live realms running it, and its forks
    GET /wrath/modes/<id>/picture.svg   (or .webp / .png) the template's picture
    GET /wrath/modes/jam                the mode jam: the theme, the entries (forks entered in it), staff picks
    GET /wrath/modes/hosted.json        every template and the realms running each (players, forks), for the launcher

Standard library only; nothing here raises to the caller of the page functions.
"""
from __future__ import annotations

import base64
import html
import importlib.util
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
CATALOG = os.path.join(HERE, "modepacks.json")
TEMPLATES = os.path.join(REPO, "hosting", "kit", "modes")
SCRIPTS_DIR = os.path.join(REPO, "realm", "custom", "lua_scripts")

FORMAT = "leyline-mode"
SCHEMA = 1
ID_RE = re.compile(r"^[a-z](?:[a-z0-9]|-(?=[a-z0-9])){1,31}$")
VERSION_RE = re.compile(r"^[0-9]{1,4}(?:\.[0-9]{1,4}){0,3}$")
SCRIPT_RE = re.compile(r"^ov_[a-z0-9_]{1,32}\.lua$")
MOD_ID_RE = re.compile(r"^(?:mod:[a-z][a-z0-9-]{2,31}|up:[a-z0-9][a-z0-9._-]{0,63}|mode:[a-z][a-z0-9-]{1,31})$")
DEN_RE = re.compile(r"^[a-z0-9_-]{1,24}\.den\.json$")
PICTURES = {"picture.svg": "image/svg+xml", "picture.webp": "image/webp", "picture.png": "image/png"}
# The host kit's 3.3.5a rulesets (hosting/kit/tools/wuhost/setup.py PROFILES, realm/profiles/<id>.env).
PROFILES = ("main", "brawl", "progression", "casual", "dalaran", "vanillaplus", "den", "solo")
TAGS = ("blizzlike", "boosted", "custom", "pvp", "rp", "hangout", "bots")
# Env a mode may set: the realm scripts' own OV_ switches and a few AzerothCore rates, never anything about the
# server itself (databases, files, folders, ports, addresses, keys, the realm's name or brand, tests).
ENV_RE = re.compile(r"^(?:OV_[A-Z0-9_]{2,40}|AC_(?:RATE_|SKILL_GAIN_|START_|AUTO_BALANCE_)[A-Z0-9_]{2,40})$")
ENV_DENY = re.compile(r"(?:DATABASE|PASS|SECRET|TOKEN|KEY|SOAP|WEBHOOK|DISCORD|URL|FILE|DIR|PATH|SAVES|TEST|HOST|"
                      r"LEYLINE|REALM_NAME|INVITE|BRAND|ADDRESS|PORT|BIND)")
ENV_VALUE_RE = re.compile(r"^[A-Za-z0-9._-]{1,24}$")
KEY_RE = re.compile(r"^[a-z][a-z0-9_]{0,23}$")
BRAND_RE = re.compile(r"(?i)world\s*of\s*warcraft|warcraft|lich\s*king|blizzard|\bwow\b")

MANIFEST_MAX = 16 * 1024
PICTURE_MAX = 256 * 1024
DEN_MAX = 512 * 1024
README_MAX = 16 * 1024
TOTAL_MAX = 1024 * 1024
FILES_MAX = 6
SETTINGS_MAX = 12
SCRIPTS_MAX = 12
ENV_MAX = 24
MODS_MAX = 8
OPTIONS_MAX = 8
# The templates (hosting/kit/modes), in the order they show in (the launcher's grid and the site), and their author.
ORDER = ("levelling", "solo-leveling", "duel-ladder", "hide-and-seek", "minigolf-night", "den", "survivors")
TEAM = "Leyline"
# What a realm's heartbeat may carry of its mode (hosting/relay/wurelay/listing.py clean_mode keeps the same bound):
# the whole recipe while it fits, else only the credit.
LISTING_MAX = 4096


def _load_namecheck():
    for p in (os.path.join(HERE, "namecheck.py"), os.path.join(REPO, "hosting", "kit", "tools", "wunet", "namecheck.py")):
        if os.path.isfile(p):
            try:
                spec = importlib.util.spec_from_file_location("wu_modepack_namecheck", p)
                mod = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(mod)
                return mod
            except Exception:   # noqa: BLE001 - no blocklist: the brand words below still apply
                return None
    return None


namecheck = _load_namecheck()


def _flat(v, n):
    """One line of plain text, at most n characters, or None when it is not text."""
    if not isinstance(v, str):
        return None
    s = " ".join(v.split())
    return s if 0 < len(s) <= n and s.isprintable() else None


def words_problem(text: str, what: str, name_like: bool = False) -> str | None:
    """A plain line when text carries a blocked word or a game or publisher name, else None."""
    if BRAND_RE.search(text):
        return f"{what}: no game or publisher names."
    if namecheck is not None:
        try:
            hit = namecheck.realm_name_hit(text) if name_like else namecheck.text_hit(text)
        except Exception:   # noqa: BLE001
            hit = None
        if hit:
            return f"{what}: \"{text}\" isn't allowed ({namecheck.message(hit[0]) if hasattr(namecheck, 'message') else 'a blocked word'})."
    return None


def env_name_ok(k) -> bool:
    return isinstance(k, str) and bool(ENV_RE.fullmatch(k)) and not ENV_DENY.search(k)


def known_scripts(d: str = SCRIPTS_DIR) -> set[str]:
    try:
        return {n for n in os.listdir(d) if SCRIPT_RE.fullmatch(n)}
    except OSError:
        return set()


def _envs(v):
    if isinstance(v, str):
        return [v]
    if isinstance(v, list) and 1 <= len(v) <= 6 and all(isinstance(x, str) for x in v):
        return list(v)
    return None


def _num(v):
    return isinstance(v, int) and not isinstance(v, bool)


def check_setting(s, fixed: dict, problems: list, n: int) -> dict | None:
    at = f"Setting {n}"
    if not isinstance(s, dict):
        problems.append(f"{at}: not a field.")
        return None
    key = s.get("key")
    if not isinstance(key, str) or not KEY_RE.fullmatch(key):
        problems.append(f"{at}: key must be a-z, 0-9 and _ (up to 24).")
        return None
    at = f"Setting {key}"
    label = _flat(s.get("label"), 40)
    if not label:
        problems.append(f"{at}: a label of up to 40 characters.")
        return None
    p = words_problem(label, f"{at} label")
    if p:
        problems.append(p)
    out = {"key": key, "label": label}
    if s.get("help") is not None:
        h = _flat(s.get("help"), 120)
        if not h:
            problems.append(f"{at}: help is one line of up to 120 characters.")
        else:
            p = words_problem(h, f"{at} help")
            if p:
                problems.append(p)
            out["help"] = h
    envs = _envs(s.get("env"))
    if not envs:
        problems.append(f"{at}: env is one setting name or a list of up to 6.")
        return None
    for k in envs:
        if not env_name_ok(k):
            problems.append(f"{at}: {k!r} is not a setting a mode may change.")
        elif k in fixed:
            problems.append(f"{at}: {k} is already set by the mode's env.")
    out["env"] = envs
    t = s.get("type")
    out["type"] = t
    if t == "number":
        lo, hi, d = s.get("min"), s.get("max"), s.get("default")
        if not (_num(lo) and _num(hi) and _num(d) and 0 <= lo <= d <= hi <= 100000):
            problems.append(f"{at}: a number needs whole min <= default <= max, from 0 to 100000.")
            return None
        out.update(min=lo, max=hi, default=d)
        if s.get("unit") is not None:
            u = _flat(s.get("unit"), 16)
            if not u:
                problems.append(f"{at}: unit is a short word.")
            else:
                out["unit"] = u
    elif t == "toggle":
        if not isinstance(s.get("default"), bool):
            problems.append(f"{at}: a toggle's default is true or false.")
            return None
        out["default"] = s["default"]
    elif t == "choice":
        opts = s.get("options")
        if not isinstance(opts, list) or not 2 <= len(opts) <= OPTIONS_MAX:
            problems.append(f"{at}: a choice has 2 to {OPTIONS_MAX} options.")
            return None
        clean = []
        for o in opts:
            v = o.get("value") if isinstance(o, dict) else None
            lab = _flat(o.get("label"), 32) if isinstance(o, dict) else None
            if not isinstance(v, str) or not ENV_VALUE_RE.fullmatch(v) or not lab or any(c["value"] == v for c in clean):
                problems.append(f"{at}: each option is {{\"value\": letters and digits, \"label\": a short name}}, values unique.")
                return None
            p = words_problem(lab, f"{at} option")
            if p:
                problems.append(p)
            clean.append({"value": v, "label": lab})
        if s.get("default") not in [c["value"] for c in clean]:
            problems.append(f"{at}: the default is one of the options' values.")
            return None
        out.update(options=clean, default=s["default"])
    else:
        problems.append(f"{at}: type is number, toggle or choice.")
        return None
    return out


def check_manifest(doc, files: dict | None = None, scripts: set | None = None) -> tuple[dict | None, list[str]]:
    """(the clean manifest, problems). files: {name: bytes} of the package (to check picture and den against);
    scripts: the script file names the Host pack has (None: realm/custom/lua_scripts)."""
    problems: list[str] = []
    if not isinstance(doc, dict):
        return None, ["mode.json is not a JSON object."]
    if doc.get("format") != FORMAT:
        problems.append(f"mode.json: format must be \"{FORMAT}\".")
    if doc.get("schema") != SCHEMA:
        problems.append("mode.json: schema 1 is the one this checker reads." if _num(doc.get("schema")) and doc["schema"] > SCHEMA
                        else "mode.json: schema must be 1.")
    mid = doc.get("id")
    if not isinstance(mid, str) or not ID_RE.fullmatch(mid):
        problems.append("id: a-z, 0-9 and single dashes, 2 to 32 long, starting with a letter.")
        mid = None
    out = {"format": FORMAT, "schema": SCHEMA, "id": mid}
    for k, n, name_like in (("name", 32, True), ("line", 120, False), ("author", 32, True)):
        v = _flat(doc.get(k), n)
        if not v:
            problems.append(f"{k}: one line of up to {n} characters.")
            continue
        # the templates are the team's own: their author is the one name a player can't take
        if not (k == "author" and v == TEAM and mid in ORDER and doc.get("based_on") is None):
            p = words_problem(v, k, name_like)
            if p:
                problems.append(p)
        out[k] = v
    ver = doc.get("version")
    if not isinstance(ver, str) or not VERSION_RE.fullmatch(ver):
        problems.append("version: numbers with dots, like 1 or 1.2.0.")
    else:
        out["version"] = ver
    bo = doc.get("based_on")
    if bo is None:
        out["based_on"] = None
    elif isinstance(bo, dict) and isinstance(bo.get("id"), str) and ID_RE.fullmatch(bo["id"]) and _flat(bo.get("author"), 32):
        out["based_on"] = {"id": bo["id"], "author": _flat(bo["author"], 32)}
        if mid and bo["id"] == mid:
            problems.append("based_on: a fork has an id of its own.")
    else:
        problems.append("based_on: null, or {\"id\": the mode's id, \"author\": its author}.")
    status = doc.get("status", "live")
    if status not in ("live", "coming"):
        problems.append("status: live or coming.")
    out["status"] = status
    prof = doc.get("profile")
    if prof is None and status == "coming":
        out["profile"] = None
    elif prof not in PROFILES:
        problems.append("profile: one of " + ", ".join(PROFILES) + ".")
    else:
        out["profile"] = prof
    have = known_scripts() if scripts is None else scripts
    sc = doc.get("scripts", [])
    if not isinstance(sc, list) or len(sc) > SCRIPTS_MAX:
        problems.append(f"scripts: a list of up to {SCRIPTS_MAX} script file names.")
        sc = []
    out["scripts"] = []
    for s in sc:
        if not isinstance(s, str) or not SCRIPT_RE.fullmatch(s):
            problems.append(f"scripts: {s!r} is not a realm script's file name (ov_<name>.lua).")
        elif s not in have:
            problems.append(f"scripts: {s} is not in the Host pack's realm scripts.")
        elif s not in out["scripts"]:
            out["scripts"].append(s)
    env = doc.get("env", {})
    out["env"] = {}
    if not isinstance(env, dict) or len(env) > ENV_MAX:
        problems.append(f"env: an object of up to {ENV_MAX} settings.")
        env = {}
    for k, v in env.items():
        if not env_name_ok(k):
            problems.append(f"env: {k!r} is not a setting a mode may change.")
        elif not isinstance(v, str) or not ENV_VALUE_RE.fullmatch(v):
            problems.append(f"env: {k}'s value is letters, digits, . _ or - (up to 24).")
        else:
            out["env"][k] = v
    mods = doc.get("mods", [])
    out["mods"] = []
    if not isinstance(mods, list) or len(mods) > MODS_MAX:
        problems.append(f"mods: a list of up to {MODS_MAX} mod ids.")
        mods = []
    for m in mods:
        if not isinstance(m, str) or not MOD_ID_RE.fullmatch(m):
            problems.append(f"mods: {m!r} is not a mod id (mod:<id>, up:<slug> or mode:<id>).")
        elif m not in out["mods"]:
            out["mods"].append(m)
    tags = doc.get("tags", [])
    if not isinstance(tags, list) or any(t not in TAGS for t in tags):
        problems.append("tags: from " + ", ".join(TAGS) + ".")
        tags = []
    out["tags"] = [t for t in TAGS if t in tags]
    jam = doc.get("jam")
    if jam is not None and (not isinstance(jam, str) or not ID_RE.fullmatch(jam)):
        problems.append("jam: the jam's id, or leave it out.")
    elif jam:
        out["jam"] = jam
    cr = doc.get("credit")
    if cr is not None:
        by, src = _flat((cr or {}).get("by") if isinstance(cr, dict) else None, 80), (cr or {}).get("source") if isinstance(cr, dict) else None
        if not by or not isinstance(src, str) or not re.fullmatch(r"https://[A-Za-z0-9.-]+(?:/[A-Za-z0-9._~%/+-]*)?", src) or len(src) > 200:
            problems.append("credit: {\"by\": who, \"source\": an https link, \"license\"?: its licence}.")
        else:
            out["credit"] = {"by": by, "source": src}
            lic = _flat(cr.get("license"), 60)
            if lic:
                out["credit"]["license"] = lic
    pic = doc.get("picture")
    if pic is not None and pic not in PICTURES:
        problems.append("picture: picture.svg, picture.webp or picture.png, or null.")
        pic = None
    elif pic is not None and files is not None and pic not in files:
        problems.append(f"picture: {pic} is not in the folder.")
    out["picture"] = pic
    den = doc.get("den")
    if den is not None:
        if not isinstance(den, str) or not DEN_RE.fullmatch(den):
            problems.append("den: a Den file's name, like room.den.json, or null.")
            den = None
        elif out.get("profile") != "den":
            problems.append("den: a Den file goes with the den ruleset only.")
        elif files is not None and den not in files:
            problems.append(f"den: {den} is not in the folder.")
    out["den"] = den
    sets = doc.get("settings", [])
    out["settings"] = []
    if not isinstance(sets, list) or len(sets) > SETTINGS_MAX:
        problems.append(f"settings: a list of up to {SETTINGS_MAX} fields.")
        sets = []
    seen_keys, seen_env = set(), set()
    for i, s in enumerate(sets, 1):
        c = check_setting(s, out["env"], problems, i)
        if not c:
            continue
        if c["key"] in seen_keys:
            problems.append(f"Setting {c['key']}: two fields have this key.")
            continue
        dup = [k for k in c["env"] if k in seen_env]
        if dup:
            problems.append(f"Setting {c['key']}: {dup[0]} is set by another field too.")
            continue
        seen_keys.add(c["key"])
        seen_env.update(c["env"])
        out["settings"].append(c)
    if len(json.dumps(doc)) > MANIFEST_MAX:
        problems.append("mode.json: 16 KB at most.")
    return (None if problems else out), problems


SVG_BAD = re.compile(rb"(?is)<\s*script|<\s*foreignobject|\bon[a-z]+\s*=|javascript:|(?:xlink:)?href\s*=\s*[\"']?(?!#)|@import|url\(\s*[\"']?(?!#)")


def picture_problem(name: str, data: bytes) -> str | None:
    if len(data) > PICTURE_MAX:
        return f"{name}: 256 KB at most."
    if name.endswith(".svg"):
        head = data[:400].lstrip()
        if not (head.startswith(b"<svg") or head.startswith(b"<?xml")):
            return f"{name}: not an SVG."
        if SVG_BAD.search(data):
            return f"{name}: an SVG with no scripts, event attributes or outside links."
    elif name.endswith(".png") and not data.startswith(b"\x89PNG\r\n\x1a\n"):
        return f"{name}: not a PNG."
    elif name.endswith(".webp") and not (data[:4] == b"RIFF" and data[8:12] == b"WEBP"):
        return f"{name}: not a WebP."
    return None


def den_problem(name: str, data: bytes) -> str | None:
    """The Den file's shape (host/dens.rs check, ov_den.lua checks every prop again when it opens it)."""
    if len(data) > DEN_MAX:
        return f"{name}: 512 KB at most."
    try:
        d = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        return f"{name}: can't be read as a Den file."
    if not isinstance(d, dict) or d.get("format") != "leyline-den":
        return f"{name}: isn't a Den file."
    if d.get("version") != 1:
        return f"{name}: a Den file of version 1."
    props = d.get("props")
    if not isinstance(props, list) or len(props) > 800 or not all(isinstance(p, dict) and _num(p.get("entry")) for p in props):
        return f"{name}: up to 800 props, each with its entry."
    return None


def read_dir(d: str) -> tuple[dict, list[str]]:
    """{file name: bytes} of a package folder, and the problems with its files (names, sizes, links)."""
    files, problems = {}, []
    try:
        names = sorted(os.listdir(d))
    except OSError:
        return {}, [f"{d}: not a folder."]
    total = 0
    for n in names:
        p = os.path.join(d, n)
        if n.startswith(".") or n == "__pycache__":
            if n not in (".DS_Store",):
                problems.append(f"{n}: no hidden files.")
            continue
        if os.path.islink(p):
            problems.append(f"{n}: no links.")
            continue
        if os.path.isdir(p):
            problems.append(f"{n}: no folders inside a mode package.")
            continue
        if n != "mode.json" and n not in PICTURES and n != "README.md" and not DEN_RE.fullmatch(n):
            problems.append(f"{n}: only mode.json, picture.svg|webp|png, <name>.den.json and README.md.")
            continue
        try:
            size = os.path.getsize(p)
            if size > DEN_MAX:
                problems.append(f"{n}: too big.")
                continue
            with open(p, "rb") as f:
                data = f.read()
        except OSError:
            problems.append(f"{n}: can't be read.")
            continue
        total += len(data)
        files[n] = data
    if len(files) > FILES_MAX:
        problems.append(f"A mode package has {FILES_MAX} files at most.")
    if total > TOTAL_MAX:
        problems.append("A mode package is 1 MB at most.")
    for n, data in files.items():
        if n == "mode.json" and len(data) > MANIFEST_MAX:
            problems.append("mode.json: 16 KB at most.")
        elif n == "README.md":
            if len(data) > README_MAX:
                problems.append("README.md: 16 KB at most.")
            try:
                text = data.decode("utf-8")
                p = words_problem(text, "README.md")
                if p and BRAND_RE.search(text):
                    problems.append(p)
            except UnicodeDecodeError:
                problems.append("README.md: plain UTF-8 text.")
        elif n in PICTURES:
            p = picture_problem(n, data)
            if p:
                problems.append(p)
        elif DEN_RE.fullmatch(n):
            p = den_problem(n, data)
            if p:
                problems.append(p)
    return files, problems


def check_dir(d: str, scripts: set | None = None) -> tuple[dict | None, list[str]]:
    files, problems = read_dir(d)
    if "mode.json" not in files:
        return None, problems + ["mode.json is missing."]
    try:
        doc = json.loads(files["mode.json"].decode("utf-8-sig"))
    except (UnicodeDecodeError, ValueError):
        return None, problems + ["mode.json can't be read as JSON."]
    clean, more = check_manifest(doc, files, scripts)
    problems += more
    for n in files:
        if n in PICTURES and n != (clean or doc).get("picture"):
            problems.append(f"{n}: mode.json names another picture (or none).")
        if DEN_RE.fullmatch(n) and n != (clean or doc).get("den"):
            problems.append(f"{n}: mode.json doesn't name this Den file.")
    return (None if problems else clean), problems


# ------------------------------------------------------------------------------------------------ hosting it
def values_for(m: dict, given: dict | None) -> tuple[dict, list[str]]:
    """The host's answers to the mode's form, checked ({key: value}); a missing answer takes the default."""
    given = given if isinstance(given, dict) else {}
    out, problems = {}, []
    for s in m.get("settings") or []:
        v = given.get(s["key"], s["default"])
        if s["type"] == "number":
            if isinstance(v, str) and re.fullmatch(r"\d{1,6}", v.strip()):
                v = int(v.strip())
            if not _num(v) or not s["min"] <= v <= s["max"]:
                problems.append(f"{s['label']}: {s['min']} to {s['max']}.")
                continue
        elif s["type"] == "toggle":
            if not isinstance(v, bool):
                problems.append(f"{s['label']}: on or off.")
                continue
        elif v not in [o["value"] for o in s["options"]]:
            problems.append(f"{s['label']}: pick one.")
            continue
        out[s["key"]] = v
    return out, problems


def env_for(m: dict, values: dict) -> dict:
    """The env the realm gets: the mode's fixed env, then each field's names set to its value."""
    env = dict(m.get("env") or {})
    for s in m.get("settings") or []:
        v = values.get(s["key"], s["default"])
        text = ("1" if v else "0") if s["type"] == "toggle" else str(v)
        for k in ([s["env"]] if isinstance(s["env"], str) else s["env"]):
            env[k] = text
    return env


def credit_of(m: dict) -> dict:
    return {k: m[k] for k in ("id", "name", "author", "version", "based_on", "jam") if m.get(k) is not None}


def listing_mode(v) -> dict | None:
    """A realm's mode as the realm list carries it (the relay's listing.py clean_mode does the same): the recipe while
    it fits in LISTING_MAX, else only the credit; None when it isn't one."""
    if not isinstance(v, dict):
        return None
    doc = dict(v, format=FORMAT, schema=SCHEMA)
    doc.setdefault("picture", None)
    doc.setdefault("den", None)
    full, _ = check_manifest(doc, None, scripts=set(v.get("scripts") or []) if isinstance(v.get("scripts"), list) else set())
    if full:
        full = {k: full[k] for k in ("id", "name", "line", "author", "version", "based_on", "profile", "scripts", "env",
                                     "mods", "tags", "settings", "jam") if k in full}
        if len(json.dumps(full, separators=(",", ":"))) <= LISTING_MAX:
            return full
    mid = v.get("id")
    if not isinstance(mid, str) or not ID_RE.fullmatch(mid):
        return None
    out = {"id": mid}
    for k, n in (("name", 32), ("author", 32), ("line", 120)):
        t = _flat(v.get(k), n)
        if t and not words_problem(t, k, k != "line"):
            out[k] = t
    if isinstance(v.get("version"), str) and VERSION_RE.fullmatch(v["version"]):
        out["version"] = v["version"]
    bo = v.get("based_on")
    if isinstance(bo, dict) and isinstance(bo.get("id"), str) and ID_RE.fullmatch(bo["id"]) and _flat(bo.get("author"), 32):
        out["based_on"] = {"id": bo["id"], "author": _flat(bo["author"], 32)}
    if isinstance(v.get("jam"), str) and ID_RE.fullmatch(v["jam"]):
        out["jam"] = v["jam"]
    return out


# ------------------------------------------------------------------------------------------------ the catalog
def build_catalog(root: str = TEMPLATES, scripts: set | None = None, jam: dict | None = None) -> tuple[dict, list[str]]:
    """{"schema", "about", "modes": [manifest + "picture_data" (a data: URL)], "jam"} from the template folders."""
    modes, problems = [], []
    try:
        names = sorted(n for n in os.listdir(root) if os.path.isdir(os.path.join(root, n)))
    except OSError:
        names = []
    for n in names:
        clean, probs = check_dir(os.path.join(root, n), scripts)
        if probs:
            problems += [f"{n}/{p}" for p in probs]
            continue
        if clean["id"] != n:
            problems.append(f"{n}: the folder's name is the mode's id ({clean['id']}).")
            continue
        entry = dict(clean)
        if clean["picture"]:
            with open(os.path.join(root, n, clean["picture"]), "rb") as f:
                entry["picture_data"] = f"data:{PICTURES[clean['picture']]};base64," + base64.b64encode(f.read()).decode()
        if clean["den"]:
            with open(os.path.join(root, n, clean["den"]), encoding="utf-8") as f:
                entry["den_text"] = f.read()
        modes.append(entry)
    order = {m: i for i, m in enumerate(ORDER)}
    modes.sort(key=lambda m: (order.get(m["id"], 99), m["id"]))
    doc = {"schema": 1,
           "about": "Mode templates anyone can host and fork (hosting/kit/modes, docs/MODES.md). Made by "
                    "python3 -I stream/modepack.py --write; the launcher builds it in, the site serves /wrath/modes/<id>.",
           "modes": modes,
           "jam": jam if jam is not None else DEFAULT_JAM}
    return doc, problems


# The mode jam (GET /wrath/modes/jam). Staff set it here (--write keeps what stream/modepacks.json has): its id is what
# a fork puts in mode.json "jam" to enter, picks are mode ids staff liked. Bragging rights only: no prizes.
DEFAULT_JAM = {"id": "first-jam", "theme": "Small realms, big games",   # TODO(owner): the jam's theme and dates
               "line": "Fork any mode, make it your own, host it, and enter it here.", "until": None, "picks": []}


def jam_ok(j) -> dict | None:
    if not isinstance(j, dict) or not isinstance(j.get("id"), str) or not ID_RE.fullmatch(j["id"]):
        return None
    out = {"id": j["id"], "theme": _flat(j.get("theme"), 60) or "Mode jam", "line": _flat(j.get("line"), 160) or "",
           "until": j.get("until") if isinstance(j.get("until"), str) and re.fullmatch(r"\d{4}-\d{2}-\d{2}", j["until"]) else None,
           "picks": [p for p in (j.get("picks") or [])[:12] if isinstance(p, str) and ID_RE.fullmatch(p)]}
    return out


def write_catalog(path: str = CATALOG) -> list[str]:
    jam = None
    try:
        with open(path, encoding="utf-8") as f:
            jam = jam_ok(json.load(f).get("jam"))
    except (OSError, ValueError, AttributeError):
        pass
    doc, problems = build_catalog(jam=jam)
    if problems:
        return problems
    text = json.dumps(doc, indent=1, ensure_ascii=False) + "\n"
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    return []


def load_catalog(path: str = CATALOG) -> dict:
    try:
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
        if isinstance(doc, dict) and isinstance(doc.get("modes"), list):
            return doc
    except (OSError, ValueError):
        pass
    return {"schema": 1, "modes": [], "jam": DEFAULT_JAM}


# ------------------------------------------------------------------------------------------------ the website
_cat = {"doc": None, "mtime": None}


def catalog() -> dict:
    try:
        mt = os.path.getmtime(CATALOG)
    except OSError:
        mt = None
    if _cat["doc"] is None or mt != _cat["mtime"]:
        _cat["doc"], _cat["mtime"] = load_catalog(), mt
    return _cat["doc"]


def template(mid: str) -> dict | None:
    return next((m for m in catalog().get("modes", []) if m.get("id") == mid), None)


portal = lambda name: None   # noqa: E731  (bind() sets it: the portal's globals)


def bind(lookup) -> None:
    global portal

    def get(name):
        try:
            return lookup(name)
        except Exception:   # noqa: BLE001
            return None
    portal = get


PATH_RE = re.compile(r"^/wrath/modes/(?:jam|hosted\.json|[a-z][a-z0-9-]{1,31}(?:/picture\.(?:svg|webp|png))?)/?$")


def match(path: str) -> bool:
    return bool(PATH_RE.fullmatch(path or ""))


def _realms() -> list[dict]:
    f = portal("community_realms")
    try:
        realms, _ok = f() if f else ([], False)
    except Exception:   # noqa: BLE001
        realms = []
    return [r for r in realms if isinstance(r, dict) and isinstance(r.get("mode"), dict)]


def running(mid: str, realms: list[dict]) -> list[dict]:
    return [r for r in realms if r["mode"].get("id") == mid]


def forks_of(mid: str, realms: list[dict]) -> list[dict]:
    return [r for r in realms if (r["mode"].get("based_on") or {}).get("id") == mid and r["mode"].get("id") != mid]


def hosted_doc(realms: list[dict]) -> dict:
    """GET /wrath/modes/hosted.json: per template, the realms running it (live ones and their players) and its forks."""
    out = []
    for m in catalog().get("modes", []):
        run = running(m["id"], realms)
        live = [r for r in run if r.get("live")]
        out.append({"id": m["id"], "realms": len(run), "live": len(live),
                    "players": sum(r.get("players") or 0 for r in live), "forks": len(forks_of(m["id"], realms))})
    return {"schema": 1, "modes": out}


CSS = """
.mp-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(230px,1fr));gap:12px}
.mp-card{display:flex;flex-direction:column;background:rgba(18,22,54,.7);border:1px solid var(--ly-line);border-radius:6px;overflow:hidden;text-decoration:none;color:inherit}
.mp-card:hover,.mp-card:focus-visible{border-color:var(--ly-cyan);box-shadow:var(--ly-glow)}
.mp-card img,.mp-pic{display:block;width:100%;height:auto;aspect-ratio:16/9;object-fit:cover;background:#0b0e26 center/cover no-repeat}
.mp-card .b{padding:12px 14px;display:flex;flex-direction:column;gap:6px}
.mp-card h3,.mp-h{margin:0;font:600 18px/1.15 'Leyline Display','OW Cinzel',Impact,sans-serif;letter-spacing:.06em;text-transform:uppercase;color:var(--ly-cyan)}
.mp-card p{margin:0;font-size:14px;color:var(--ly-ink)}
.mp-by{font-size:12px;color:var(--ly-dim)}
.mp-live{font:600 12px/1 system-ui,sans-serif;color:#5ef0a0}
.ly h1.mp-h{font-size:clamp(26px,4.5vw,40px);line-height:1.1;margin:0 0 8px;text-align:left}
.mp-top{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1.2fr);gap:16px;align-items:start}
@media (max-width:720px){.mp-top{grid-template-columns:1fr}}
.mp-facts{margin:0;padding:0;list-style:none;display:grid;gap:6px;font-size:14px}
.mp-facts b{color:var(--ly-ink);margin-right:6px}
.mp-rows{margin:0;padding:0;list-style:none;display:grid;gap:8px}
.mp-rows li{display:flex;flex-wrap:wrap;gap:6px 12px;align-items:baseline;justify-content:space-between;background:rgba(10,12,34,.6);border:1px solid var(--ly-line);border-radius:6px;padding:10px 12px}
.mp-rows a{color:var(--ly-cyan)}
.mp-pick{font:600 11px/1 system-ui,sans-serif;letter-spacing:.08em;text-transform:uppercase;color:#ffd77a;border:1px solid rgba(255,215,122,.5);border-radius:999px;padding:4px 8px}
.mp-empty{color:var(--ly-dim);font-size:14px}
.md-lede{max-width:720px;margin:0 auto 14px;text-align:center;color:var(--ly-ink)}
.md-tag{font:600 11px/1 system-ui,sans-serif;letter-spacing:.08em;text-transform:uppercase;color:var(--ly-dim);border:1px solid var(--ly-line);border-radius:999px;padding:4px 8px}
.md-hint{font-size:13px;color:var(--ly-dim);margin:8px 0 0}
.md-steps{margin:8px 0 0;padding-left:1.2rem}.md-steps li{margin:4px 0}
.ly code{background:rgba(5,6,20,.8);border:1px solid var(--ly-line);border-radius:4px;padding:1px 5px}
"""


def _pic_url(m: dict) -> str | None:
    return f"/wrath/modes/{m['id']}/{m['picture']}" if m.get("picture") and m.get("picture_data") else None


def _by(m: dict) -> str:
    e = html.escape
    bo = m.get("based_on")
    s = f"by {e(m.get('author') or 'someone')}"
    if bo:
        base = template(bo["id"])
        bname = base["name"] if base else bo["id"]
        s = f'based on <a href="/wrath/modes/{e(bo["id"])}">{e(bname)}</a> by {e(bo.get("author") or "someone")} · ' + s
    return s


def card(m: dict, stats: dict | None = None) -> str:
    e = html.escape
    pic = _pic_url(m)
    # the picture is decoration (the name says what it is): a background, so the card stays one link of text
    img = (f'<span class="mp-pic" aria-hidden="true" style="background-image:url({e(pic)})"></span>' if pic
           else '<span class="mp-pic" aria-hidden="true"></span>')
    live = ""
    if stats and stats.get("live"):
        live = f'<span class="mp-live">● {stats["live"]} live · {stats["players"]} playing</span>'
    soon = ' <span class="md-tag">Coming soon</span>' if m.get("status") == "coming" else ""
    return (f'<a class="mp-card" href="/wrath/modes/{e(m["id"])}">{img}<span class="b"><h3>{e(m["name"])}{soon}</h3>'
            f'<p>{e(m.get("line") or "")}</p><span class="mp-by">{_by(m)}</span>{live}</span></a>')


def section() -> str:
    """The "Host a mode" section for /wrath/modes: every template's card (players on the realms running it)."""
    try:
        realms = _realms()
        stats = {x["id"]: x for x in hosted_doc(realms)["modes"]}
        cards = "".join(card(m, stats.get(m["id"])) for m in catalog().get("modes", []))
        if not cards:
            return ""
        return (f'<style>{CSS}</style><section class="ly-panel" id="host-modes" aria-labelledby="mp-h">'
                '<h2 id="mp-h">Host a mode</h2><p class="md-lede">Pick a mode, give your realm a name and host it from '
                'Leyline on your own PC. Fork any mode to make it your own. '
                '<a href="/wrath/modes/jam">The mode jam</a></p>'
                f'<div class="mp-grid">{cards}</div></section>')
    except Exception:   # noqa: BLE001
        return ""


def _realm_row(r: dict, pick: bool = False) -> str:
    e = html.escape
    m = r.get("mode") or {}
    who = f'{r.get("players") or 0} playing' if r.get("live") else "offline"
    credit = ""
    if m.get("based_on"):
        b = m["based_on"]
        base = template(b["id"])
        credit = f' · based on {e(base["name"] if base else b["id"])} by {e(b.get("author") or "someone")}'
    rid = r.get("id") or ""
    return (f'<li><span><a href="/wrath/r/{e(rid)}"><b>{e(r.get("name") or "A realm")}</b></a> '
            f'<span class="mp-by">{e(m.get("name") or "")}{" by " + e(m["author"]) if m.get("author") else ""}{credit}</span></span>'
            f'{"<span class=mp-pick>Staff pick</span>" if pick else ""}<span class="mp-by">{e(who)}</span></li>')


def _rows(rs: list[dict], empty: str, picks=()) -> str:
    if not rs:
        return f'<p class="mp-empty">{html.escape(empty)}</p>'
    rs = sorted(rs, key=lambda r: ((r.get("mode") or {}).get("id") not in picks, not r.get("live"), -(r.get("players") or 0),
                                   (r.get("name") or "").lower()))
    return '<ul class="mp-rows">' + "".join(_realm_row(r, (r.get("mode") or {}).get("id") in picks) for r in rs[:100]) + "</ul>"


def page_mode(mid: str, realms: list[dict]) -> tuple[str, str] | None:
    """(title, body) for /wrath/modes/<id>: a template, or a fork some listed realm runs. None: nobody knows it."""
    e = html.escape
    m = template(mid)
    run = running(mid, realms)
    if m is None:
        named = next((r["mode"] for r in run if r["mode"].get("name")), None)
        if not named:
            return None
        m = dict(named, status="live", picture=None)
    forks = forks_of(mid, realms)
    pic = _pic_url(m)
    img = (f'<img class="mp-pic" src="{e(pic)}" alt="" width="640" height="360" decoding="async">' if pic
           else '<div class="mp-pic" aria-hidden="true"></div>')
    facts = []
    if m.get("profile"):
        facts.append(("Starts from", RULESET_WORDS.get(m["profile"], m["profile"])))
    for s in m.get("settings") or []:
        d = s["default"]
        shown = ("on" if d else "off") if s["type"] == "toggle" else next((o["label"] for o in s.get("options", []) if o["value"] == d), str(d))
        facts.append((s["label"], f'{shown}{" " + s["unit"] if s.get("unit") and s["type"] == "number" else ""}'))
    if m.get("credit"):
        facts.append(("Credit", f'{m["credit"]["by"]}' + (f' ({m["credit"]["license"]})' if m["credit"].get("license") else "")))
    facts_html = "".join(f"<li><b>{e(k)}</b>{e(v)}</li>" for k, v in facts)
    how = ('<p class="md-hint">Coming soon: it can\'t be hosted yet.</p>' if m.get("status") == "coming" else
           '<p class="md-hint">Host it: open Leyline, go to Host and pick this mode. Fork it there to make it your own: '
           'your copy shows "based on" this one.</p>')
    body = (f'<section class="ly-panel"><div class="mp-top">{img}<div>'
            f'<h1 class="mp-h">{e(m["name"])}</h1><p>{e(m.get("line") or "")}</p><p class="mp-by">{_by(m)}</p>'
            f'<ul class="mp-facts">{facts_html}</ul>{how}</div></div></section>'
            f'<section class="ly-panel"><h2>Realms running it</h2>{_rows(run, "No realm on the list runs it right now.")}</section>'
            f'<section class="ly-panel"><h2>Forks</h2>{_rows(forks, "No forks on the list yet. Be the first.")}</section>'
            '<p class="fine"><a href="/wrath/modes">All modes</a> · <a href="/wrath/modes/jam">The mode jam</a></p>')
    return m["name"], body


RULESET_WORDS = {"main": "Classic levelling", "brawl": "Brawl (everyone at the top level)", "progression": "Progression",
                 "casual": "Casual", "dalaran": "Dalaran hangout", "vanillaplus": "Vanilla+", "den": "My Den",
                 "solo": "Solo leveling"}


def page_jam(realms: list[dict]) -> tuple[str, str]:
    e = html.escape
    j = jam_ok(catalog().get("jam")) or jam_ok(DEFAULT_JAM)
    entries = [r for r in realms if (r.get("mode") or {}).get("jam") == j["id"]]
    picks = tuple(j["picks"])
    until = f'<p class="md-hint">Entries until {e(j["until"])}.</p>' if j["until"] else ""
    body = (f'<section class="ly-panel"><h1 class="mp-h">Mode jam: {e(j["theme"])}</h1>'
            f'<p>{e(j["line"])}</p>{until}'
            '<ol class="md-steps"><li>In Leyline, go to Host and press Fork on any mode.</li>'
            f'<li>Make it yours: its name, its settings, and the jam: <code>{e(j["id"])}</code>.</li>'
            '<li>Host it as a public realm. It shows here while it is on the realm list.</li></ol>'
            '<p class="md-hint">Bragging rights only: no prizes. Staff picks are marked.</p></section>'
            f'<section class="ly-panel"><h2>Entries</h2>{_rows(entries, "No entries yet.", picks)}</section>'
            '<p class="fine"><a href="/wrath/modes">All modes</a></p>')
    return f"Mode jam: {j['theme']}", body


def get(h, path: str, query: dict) -> None:
    """Serve one of PATH_RE's pages on the portal's handler h (its send, ip and the portal's chrome)."""
    try:
        bucket = portal("rl_bucket")(h.ip())
        if not portal("browse_limit").hit(bucket, 600):
            h.send(429, portal("busy_page")(back=path))
            return
    except Exception:   # noqa: BLE001 - no rate limit to hand: serve it anyway
        pass
    p = path.rstrip("/")
    mm = re.fullmatch(r"/wrath/modes/([a-z][a-z0-9-]{1,31})/(picture\.(?:svg|webp|png))", p)
    if mm:
        m = template(mm.group(1))
        if not m or m.get("picture") != mm.group(2) or not m.get("picture_data"):
            h.send(404, b"not found", "text/plain; charset=utf-8")
            return
        h.send(200, base64.b64decode(m["picture_data"].split(",", 1)[1]), PICTURES[m["picture"]], public=True)
        return
    realms = _realms()
    if p == "/wrath/modes/hosted.json":
        h.send(200, json.dumps(hosted_doc(realms)).encode(), "application/json; charset=utf-8", public=True)
        return
    got = page_jam(realms) if p == "/wrath/modes/jam" else page_mode(p.rsplit("/", 1)[1], realms)
    shell = portal("ly_shell")
    if got is None:
        got, status = ("No such mode", '<section class="ly-panel"><h1>No such mode</h1><p>Nobody on the realm list runs a mode '
                       'with that name. <a href="/wrath/modes">All modes</a></p></section>'), 404
    else:
        status = 200
    title, body = got
    if shell is None:
        h.send(status, body.encode("utf-8"))
        return
    h.send(status, shell(title, body, p, "modes", next_step="host", css=CSS))


def _main(argv: list[str]) -> int:
    if argv[:1] == ["--check"] and argv[1:]:
        bad = 0
        for d in argv[1:]:
            _, probs = check_dir(d)
            for p in probs:
                print(f"{d}: {p}")
            bad += bool(probs)
            if not probs:
                print(f"{d}: ok")
        return 1 if bad else 0
    if argv == ["--write"]:
        probs = write_catalog()
        for p in probs:
            print(p)
        if not probs:
            print(f"wrote {os.path.relpath(CATALOG, REPO)}")
        return 1 if probs else 0
    if argv == ["--verify"]:
        doc, probs = build_catalog(jam=jam_ok(load_catalog().get("jam")))
        if probs or doc != load_catalog():
            print("stream/modepacks.json is out of date: python3 -I stream/modepack.py --write", *probs, sep="\n")
            return 1
        print("stream/modepacks.json is up to date")
        return 0
    print(__doc__.split("\n\n")[0])
    print("usage: modepack.py --check DIR... | --write | --verify")
    return 2


if __name__ == "__main__":
    sys.exit(_main(sys.argv[1:]))
