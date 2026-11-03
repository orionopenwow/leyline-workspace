"""Loadout templates and share codes (stream/loadouts.py): every template id is a REVIEWED, published addon (never a
pending one), templates drop anything unpublished, the launcher keeps the same templates (ui/loadouts.js TEMPLATES), share
codes read back and carry catalog ids only, a realm's suggested loadout is checked like the relay's, no brand words.

    python3 -I stream/tests/test_loadouts.py
"""
import importlib.util, json, os, re, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
REPO = os.path.dirname(STREAM)
passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1; print("PASS", name)
    else:
        failed += 1; print("FAIL", name, extra)


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


LO = load("lo", os.path.join(STREAM, "loadouts.py"))
UA = load("ua", os.path.join(STREAM, "upstream_addons.py"))
sys.path.insert(0, os.path.join(REPO, "hosting", "relay"))
try:
    from wurelay import listing as RL
except Exception as ex:   # noqa: BLE001
    RL = None
    print("note: relay listing not importable:", ex)

raw = json.load(open(os.path.join(STREAM, "loadouts.json")))["templates"]
reviewed = {p["id"] for p in UA.projects()}
pending = set()
try:
    for p in json.load(open(os.path.join(REPO, "mods", "upstream.json"))).get("entries", []):
        if isinstance(p, dict) and p.get("id") and p.get("id") not in reviewed:
            pending.add(p["id"])
except (OSError, ValueError):
    pass

every = [i for t in raw for i in t["items"]]
check("every template id is a reviewed addon or the Leyline addons (never a pending one)",
      all(i == LO.LEYLINE_ADDONS or i in reviewed for i in every), [i for i in every if i != LO.LEYLINE_ADDONS and i not in reviewed])
check("... and none is on the pending list", not (set(every) & pending), set(every) & pending)
check("every template has a known preset or none, a name, a line, builds", all(
    t.get("gfx", "") in ("",) + LO.PRESETS and t["name"] and t["line"] and t["builds"] for t in raw))
check("templates as the site shows them: all of them with today's reviewed list",
      [t["id"] for t in LO.templates(reviewed)] == [t["id"] for t in raw])

d = tempfile.mkdtemp()
p = os.path.join(d, "t.json")
json.dump({"templates": [{"id": "mixed", "name": "Mixed", "line": "x", "items": ["up:skada", "up:pending-thing", "leyline:addons"], "gfx": ""},
                         {"id": "only-pending", "name": "Only pending", "line": "x", "items": ["up:pending-thing"], "gfx": ""},
                         {"id": "looks-only", "name": "Looks", "line": "x", "items": [], "gfx": "ultra"},
                         {"id": "Bad Id", "name": "x", "items": [], "gfx": "low"}]}, open(p, "w"))
got = LO.templates({"up:skada"}, p)
check("an unpublished id is dropped; a template left with nothing is left out; a preset alone is enough",
      [(t["id"], t["items"]) for t in got] == [("mixed", ["up:skada", "leyline:addons"]), ("looks-only", [])], got)

# share codes: the launcher's format (loadouts.rs share_code / parse_code)
t = {"name": "Raid; night & more", "items": ["up:skada", "leyline:addons"], "gfx": "high"}
code = LO.share_code(t)
check("share code: the launcher's format, the name escaped", code == "leyline-loadout:1;n=Raid%3B%20night%20%26%20more;g=high;i=up:skada,leyline:addons", code)
check("share code: reads back", LO.parse_code(code) == {"name": "Raid; night & more", "items": ["up:skada", "leyline:addons"], "gfx": "high"})
check("share code: only catalog ids; an unknown preset left out; not a code is None",
      LO.parse_code("leyline-loadout:1;n=X;g=insane;i=up:skada,https://evil.example/x.zip,local:Mine")
      == {"name": "X", "items": ["up:skada"], "gfx": ""} and LO.parse_code("hello") is None
      and LO.parse_code(LO.CODE_PREFIX + "n=" + "a" * 5000) is None)

# a realm's suggested loadout: the same answer here, in the relay and in realms.rs (its tests hold the same cases)
cases = [({"name": "Raid night", "items": ["up:skada", "https://x.example/a.zip", "local:Mine", "up:skada", "mod:pings"], "gfx": "high"},
          {"name": "Raid night", "items": ["up:skada", "mod:pings"], "gfx": "high"}),
         ({"items": ["up:dominos"], "gfx": "turbo"}, {"name": "Frost", "items": ["up:dominos"], "gfx": ""}),
         ({"items": ["file:x"]}, None), ("up:skada", None)]
check("realm loadout: catalog ids only, the realm's name when it has none", all(LO.clean_loadout(v, "Frost") == w for v, w in cases),
      [LO.clean_loadout(v, "Frost") for v, _ in cases])
if RL is not None:
    check("realm loadout: the relay keeps the same ids and preset", all(
        (RL.clean_loadout(v) is None) == (w is None) and (w is None or (RL.clean_loadout(v)["items"], RL.clean_loadout(v)["gfx"]) == (w["items"], w["gfx"]))
        for v, w in cases))
    check("realm loadout: the presets are the launcher's", tuple(RL.LOADOUT_PRESETS) == LO.PRESETS)

# the launcher's copy (ui/loadouts.js TEMPLATES, between the markers) is this file's templates
js = open(os.path.join(REPO, "launcher", "ui", "loadouts.js"), encoding="utf-8").read()
m = re.search(r"/\* TEMPLATES-START \*/(.*?)/\* TEMPLATES-END \*/", js, re.S)
ui = json.loads(m.group(1)) if m else None
want = [{k: t[k] for k in ("id", "name", "line", "items", "gfx", "builds")} for t in raw]
check("the launcher keeps the same templates (ui/loadouts.js TEMPLATES)", ui == want, (ui, want) if ui != want else "")
rs = open(os.path.join(REPO, "launcher", "src-tauri", "src", "gfx.rs"), encoding="utf-8").read()
check("the presets are gfx.rs's", tuple(re.findall(r'^\s*\("([a-z]+)", "[A-Z][a-z]+", &\[', rs, re.M)) == LO.PRESETS)

BANNED = re.compile(r"(?i)world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard")
words = " ".join(t["name"] + " " + t["line"] for t in raw)
check("no brand words, no client numbers in the templates' words", not BANNED.search(words) and not re.search(r"\d\.\d", words), words)

print(f"\n{passed}/{passed + failed} passed")
sys.exit(0 if failed == 0 else 1)
