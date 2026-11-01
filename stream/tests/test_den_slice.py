"""The Den tier of the streamed slice (owner 1 Nov 00:16: a light, fully working Den in the browser, the slice way):
the island every Den room stands on, and the models of every object a Den can place.

    python3 -I stream/tests/test_den_slice.py
"""
import glob, importlib.util, json, os, re, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
L = os.path.join(ROOT, "realm", "custom", "lua_scripts")
spec = importlib.util.spec_from_file_location("displays", os.path.join(ROOT, "stream", "server", "displays.py"))
D = importlib.util.module_from_spec(spec); spec.loader.exec_module(D)
fails = n = 0
def check(name, ok, got=None):
    global fails, n
    n += 1
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else f" {got!r}")); fails += 0 if ok else 1

doc = json.load(open(os.path.join(ROOT, "stream", "slices", "mini.json"), encoding="utf-8"))
den = next((t for t in doc["tiers"] if t["key"] == "den"), None)
check("mini.json has the Den tier, right after Dalaran (the budget always keeps it)",
      den is not None and doc["order"][:3] == ["base", "dalaran", "den"])
check("kind home: the stream rule lets every player stay in a Den; no travel entries", den["kind"] == "home" and not den.get("play"))
check("maxTier covers every tier", doc["maxTier"] == len(doc["order"]) - 1, doc["maxTier"])

# where the room stands (ov_build.lua DEN defaults) and how wide it gets (OV_DEN_SIZE at most 32, the yard half + 14)
b = open(os.path.join(L, "ov_build.lua"), encoding="utf-8").read()
m = re.search(r'DEN = \{ name = "Your Den", map = (\d+), x = at\("OV_DEN_X", ([-\d.]+)\), y = at\("OV_DEN_Y", ([-\d.]+)\)', b)
mid, x, y = int(m.group(1)), float(m.group(2)), float(m.group(3))
box = den["maps"][0]["box"]
reach = 32 + 14 + 40
check("the tier's map is the Den's map, its box around the room and yard with room to look around",
      den["maps"][0]["id"] == mid and box[0] <= x - reach and box[1] >= x + reach and box[2] <= y - reach and box[3] >= y + reach,
      (mid, x, y, box))

# every object a Den can place
cat = b[b.index("local CATALOG = {"):b.index("local CATEGORIES")]
want = {int(v) for v in re.findall(r"\bid\s*=\s*(\d+)", cat)}
bl = b[b.index("for _, b in ipairs({"):b.index("CATALOG[#CATALOG + 1] = { id = b[1]")]
want |= {int(v) for v in re.findall(r"\{\s*(\d{4,6}),", bl)}
d = open(os.path.join(L, "ov_den.lua"), encoding="utf-8").read()
want |= {int(v) for v in re.findall(r"(?:floor|wall|light) = (\d+)", d)}
files = glob.glob(os.path.join(L, "den-stamps", "*.den.json")) + glob.glob(os.path.join(L, "den-starters", "*.den.json"))
for f in files:
    want |= {int(p["entry"]) for p in json.load(open(f, encoding="utf-8"))["props"]}
have = set(den["objects"])
check(f"the tier carries every Den object ({len(want)}: catalog, buildings, room blocks, {len(files)} stamp and starter files)",
      want <= have, sorted(want - have)[:20])
check("no stray entries (only what a Den can place)", have <= want, sorted(have - want)[:20])
check("the Inn stamp's building and lanterns are in", {19007, 179977, 19626} <= have)

# displays.py turns the list into one query for that tier, and nothing for tiers without one
tiers = set(range(0, 1000))
q = D.tier_objects_sql(doc, tiers)
idx = doc["order"].index("den")
check("displays.py: one objects query, for the Den tier only", [t for _, t in q] == [idx], [t for _, t in q])
sql = q[0][0]
check("the query reads the templates' displays, labelled with the Den tier",
      "FROM gameobject_template t WHERE t.entry IN (" in sql and f"'{idx}'" in sql and "19007" in sql)
check("a tier not chosen gives no query", D.tier_objects_sql(doc, {0, 1}) == [])
bad = {"order": ["x"], "tiers": [{"key": "x", "objects": ["1; DROP TABLE x", True, -5, 2.5, 10**9]}]}
check("only plain entry numbers reach the SQL", D.tier_objects_sql(bad, tiers) == [])
check("--sql (the spawn query) is unchanged by objects", "gameobject_template t WHERE t.entry IN" not in D.build_sql(doc, tiers))
print(f"{n - fails}/{n} passed" if not fails else f"{fails} failed"); sys.exit(1 if fails else 0)
