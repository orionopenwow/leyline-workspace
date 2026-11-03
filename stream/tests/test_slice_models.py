"""Character and creature models in the streamed slice (owner 3 Nov 2026: "Some character models missing in Wrath
Unleashed browser server, do a fix pass for everything we need for the hangout and basic zones").

- displays.py: what players turn into (shapeshift forms with every display sharing the model, transforms, the core's
  Ghost spells), the "pets" tier (every tameable beast) and a tier's "creatures" (scripted summons);
- mini.json / sampler.json: the pets tier after the starting areas and before the dungeons, the scripted creatures;
- slice-health.sh: every playable race and gender's model and skins (PROBLEM when missing), CharSections textures and
  shapeshift form models (WARN), and the players' reported misses (portal-data/stream/slice-misses.json).

    python3 -I stream/tests/test_slice_models.py
"""
import importlib.util, json, os, re, struct, subprocess, sys, tempfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
ST = os.path.join(ROOT, "stream")
spec = importlib.util.spec_from_file_location("displays", os.path.join(ST, "server", "displays.py"))
D = importlib.util.module_from_spec(spec); spec.loader.exec_module(D)
fails = n = 0


def check(name, ok, got=None):
    global fails, n
    n += 1
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else f"  {str(got)[:500]}"))
    fails += 0 if ok else 1


def wdbc(fields, recs, strings=b"\0"):
    body = b"".join(struct.pack(f"<{fields}i", *r) for r in recs)
    return b"WDBC" + struct.pack("<4I", len(recs), fields, fields * 4, len(strings)) + body + strings


def spell(sid, eff=(0, 0, 0), aura=(0, 0, 0), misc=(0, 0, 0), trig=(0, 0, 0)):
    r = [0] * 234
    r[0] = sid
    r[71:74], r[95:98], r[110:113], r[116:119] = eff, aura, misc, trig
    return r


def form(fid, displays):
    r = [0] * 35
    r[0] = fid
    r[23:23 + len(displays)] = displays
    return r


def cdi(did, model):
    r = [0] * 16
    r[0], r[1] = did, model
    return r


# --- 1. displays.py: forms, transforms, ghost seeds
SPELLS = wdbc(234, [
    spell(768, eff=(6, 0, 0), aura=(36, 0, 0), misc=(1, 0, 0)),        # Cat Form (form 1)
    spell(5487, eff=(6, 6, 0), aura=(36, 4, 0), misc=(5, 0, 0)),       # Bear Form (form 5)
    spell(9999, eff=(6, 0, 0), aura=(36, 0, 0), misc=(31, 0, 0)),      # an NPC-only form
    spell(118, eff=(6, 0, 0), aura=(56, 0, 0), misc=(16372, 0, 0)),    # Polymorph: a sheep (creature entry)
    spell(20584, eff=(6, 0, 0), aura=(56, 0, 0), misc=(16000, 0, 0)),  # the core's wisp ghost (made-up entry)
    spell(8326, eff=(6, 0, 0), aura=(0, 0, 0)),                        # Ghost
])
FORMS = wdbc(35, [form(1, [892, 8571]), form(5, [2281, 2289]), form(31, [15374]), form(2, [])])
CDI = wdbc(16, [cdi(892, 10), cdi(8571, 11), cdi(29405, 10), cdi(29409, 11), cdi(2281, 20), cdi(2289, 21),
                cdi(15374, 30), cdi(4444, 99)])
sp = D.parse_spells(SPELLS)
player = {768, 5487, 118}
check("forms: aura 36's misc values of the players' spells only", D.aura_values(sp, player, D.AURA_SHAPESHIFT) == {1, 5})
check("transforms: aura 56's creature entries", D.aura_values(sp, player, D.AURA_TRANSFORM) == {16372})
check("transforms: the core's Ghost spells add the wisp", D.aura_values(sp, player | set(D.CORE_SPELLS), D.AURA_TRANSFORM) == {16372, 16000})
forms = D.parse_forms(FORMS)
check("SpellShapeshiftForm.dbc: CreatureDisplayID columns 23-26", forms[1] == {892, 8571} and forms[2] == set(), forms)
models = D.parse_display_models(CDI)
check("CreatureDisplayInfo.dbc: ModelID column 1", models[29405] == 10 and models[2289] == 21)
got = D.form_displays(forms, {1, 5}, models)
check("forms: both factions' displays and every colour variant sharing their models, not other forms",
      got == {892, 8571, 29405, 29409, 2281, 2289}, sorted(got))
check("forms: without CreatureDisplayInfo.dbc, the form displays alone", D.form_displays(forms, {1}, None) == {892, 8571})
for bad, what in ((wdbc(34, [[0] * 34]), "SpellShapeshiftForm.dbc"), (wdbc(15, [[0] * 15]), "CreatureDisplayInfo.dbc")):
    try:
        (D.parse_forms if "Shape" in what else D.parse_display_models)(bad); refused = False
    except ValueError:
        refused = True
    check(f"refuses a {what} with the wrong column count", refused)

# --- 2. pets and scripted creatures
psql = D.pets_sql("9")
check("pets: every tameable beast (type_flags & 1, a family), its creature_template_model displays, its tier",
      "(t.type_flags & 1) <> 0 AND t.family <> 0" in psql and "creature_template_model m ON m.CreatureID = t.entry" in psql
      and "'9'" in psql, psql)
check("pets: the old schema's modelid1-4", "t.modelid4" in D.pets_sql("9", False) and "creature_template_model" not in D.pets_sql("9", False))
doc = {"order": ["base", "a", "p"], "tiers": [{"key": "base", "creatures": [6491, True, "x", -1, 10 ** 9]},
                                             {"key": "a", "creatures": [721]}, {"key": "p", "pets": True}]}
cs = D.tier_creatures_sql(doc, {0, 1, 2})
check("creatures: one query per tier, whole entries only, no vehicle passengers",
      [t for _, t in cs] == [0, 1] and "IN (6491)" in cs[0][0] and "'0'" in cs[0][0] and "vehicle_template_accessory" not in cs[0][0], cs)
check("creatures: the other-gender model too", "DisplayID_Other_Gender" in cs[1][0])
check("pets: the tier with \"pets\": true, when chosen", D.tier_flag(doc, {0, 1, 2}, "pets") == [2] and D.tier_flag(doc, {0, 1}, "pets") == [])

# --- 3. the slice lists
L = os.path.join(ROOT, "realm", "custom", "lua_scripts")
hider = int(re.search(r'num\("OV_EVENTS_HIDER", (\d+)\)', open(os.path.join(L, "ov_events.lua"), encoding="utf-8").read()).group(1))
comp = int(re.search(r'OV_DEN_COMPANION_ENTRY"\) or ""\) or (\d+)', open(os.path.join(L, "ov_den_companion.lua"), encoding="utf-8").read()).group(1))
for name in ("mini", "sampler"):
    sl = json.load(open(os.path.join(ST, "slices", name + ".json"), encoding="utf-8"))
    by = {t["key"]: t for t in sl["tiers"]}
    order = sl["order"]
    pets = by.get("pets", {})
    first_instance = min(i for i, k in enumerate(order) if by[k].get("kind") in ("dungeon", "raid", "arena", "battleground"))
    starters = [i for i, k in enumerate(order) if by[k].get("kind") in ("starter", "home", "city")]
    check(f"{name}: a pets tier, kind base, \"pets\": true, no maps", pets.get("pets") is True and pets.get("kind") == "base" and pets.get("maps") == [])
    check(f"{name}: pets after Dalaran and the starting areas, before every dungeon, raid, arena and battleground",
          max(starters) < order.index("pets") < first_instance, order[:12])
    check(f"{name}: maxTier is the last tier", sl["maxTier"] == len(order) - 1)
    check(f"{name}: mounts and forms in the base, the Spirit Healer too", by["base"].get("mounts") is True and 6491 in by["base"].get("creatures", []))
    check(f"{name}: Dalaran's event rabbit (ov_events.lua's default hider)", hider in by["dalaran"].get("creatures", []), hider)
    check(f"{name}: the battleground Spirit Guides", {13116, 13117} <= set(by["battlegrounds"].get("creatures", [])))
    check(f"{name}: the base still takes every player model and item (Character\\, Item\\)",
          {"Character\\", "Item\\"} <= set(by["base"]["prefixes"]))
    if "den" in by:
        check(f"{name}: the Den companion (ov_den_companion.lua's default)", comp in by["den"].get("creatures", []), comp)
lua = subprocess.run([sys.executable, "-I", os.path.join(ST, "lua", "tiers_lua.py"), os.path.join(ST, "slices", "sampler.json")],
                     capture_output=True, text=True)
check("tiers_lua.py takes the new tier list", lua.returncode == 0 and 'key = "pets"' in lua.stdout, lua.stderr[-300:])

# --- 4. displays.py main(): forms and transforms in the base, the pets tier, scripted creatures
tmp = tempfile.mkdtemp()
dbcdir = os.path.join(tmp, "dbc"); os.makedirs(dbcdir)
SLA = wdbc(14, [[i, 777, s] + [0] * 11 for i, s in enumerate((768, 5487, 118), 1)])
for fn, b in (("Spell.dbc", SPELLS), ("SkillLineAbility.dbc", SLA), ("SpellShapeshiftForm.dbc", FORMS), ("CreatureDisplayInfo.dbc", CDI)):
    open(os.path.join(dbcdir, fn), "wb").write(b)
fakebin = os.path.join(tmp, "bin"); os.makedirs(fakebin)
open(os.path.join(fakebin, "docker"), "w").write(r'''#!/usr/bin/env python3
import re, sys
a = sys.argv[1:]
if a[:1] == ["ps"] or "ov-ac-database" not in a:
    sys.exit(0)
q = sys.stdin.read()
if "SHOW TABLES LIKE 'creature_template_model'" in q: print("creature_template_model")
elif "SHOW TABLES" in q or "SHOW COLUMNS" in q: print("x")
elif "type_flags & 1" in q: print("c\t0\t3000\t" + re.search(r"'(\d+)' FROM creature_template t JOIN", q).group(1))
elif "x.d" in q:
    lab = re.search(r"x\.d, '(\d+)'", q).group(1)
    for e in re.search(r"CreatureID IN \(([0-9, ]+)\)", q).group(1).split(", "): print(f"c\t0\t{int(e) * 10}\t{lab}")
''')
os.chmod(os.path.join(fakebin, "docker"), 0o755)
env = dict(os.environ, PATH=fakebin + os.pathsep + os.environ["PATH"])
sampler = os.path.join(ST, "slices", "sampler.json")
sdoc = json.load(open(sampler, encoding="utf-8"))
pt = sdoc["order"].index("pets")
r = subprocess.run([sys.executable, "-I", os.path.join(ST, "server", "displays.py"), sampler, "--tiers", f"0,{pt}", "--dbc", dbcdir],
                   capture_output=True, text=True, env=env)
out = set(r.stdout.splitlines())
check("main(): the forms' displays and their variants in the base", r.returncode == 0 and
      {f"c\t0\t{d}\t0" for d in (892, 8571, 29405, 29409, 2281, 2289)} <= out and "c\t0\t15374\t0" not in out, (r.stderr[-400:], sorted(out)))
check("main(): transforms (polymorph) and the ghost wisp in the base", {"c\t0\t163720\t0", "c\t0\t160000\t0"} <= out, sorted(out))
check("main(): the Spirit Healer in the base, the pets in their tier", "c\t0\t64910\t0" in out and f"c\t0\t3000\t{pt}" in out, sorted(out))
check("main(): says what it took", "2 shapeshift forms -> 6 displays" in r.stderr and "1 transform creatures" not in r.stderr and "2 transform creatures" in r.stderr
      and "hunter pets: 1 displays" in r.stderr and "6 form and 2 transform displays" in r.stderr, r.stderr[-600:])

# --- 5. slice-health.sh: the player models, the character textures, the forms, the reported misses
RACES = ("Human", "Orc", "Dwarf", "NightElf", "Scourge", "Tauren", "Gnome", "Troll", "BloodElf", "Draenei")


def m2(views):
    b = bytearray(0x60)
    b[0:4] = b"MD20"
    struct.pack_into("<I", b, 0x44, views)
    return bytes(b)


def put(root, rel, data=b"x"):
    p = os.path.join(root, *rel.split("\\"))
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, "wb").write(data)


def make_slice(root, drop=()):
    for k in ("DBFilesClient\\Spell.dbc", "DBFilesClient\\Map.dbc", "DBFilesClient\\AreaTable.dbc", "Interface\\GlueXML\\GlueXML.toc"):
        put(root, k)
    for race in RACES:
        for g in ("Male", "Female"):
            stem = f"Character\\{race}\\{g}\\{race}{g}"
            put(root, stem + ".m2", m2(4))
            for i in range(4):
                if f"{stem}{i:02d}.skin" not in drop:
                    put(root, f"{stem}{i:02d}.skin")
    names = b"\0Character\\Human\\Male\\HumanMaleSkin00_00.blp\0Character\\Human\\Male\\HumanMaleFaceLower00_00.blp\0"
    rec = [1, 1, 0, 0, 1, names.index(b"Character\\Human\\Male\\HumanMaleFaceLower"), 0, 0, 0, 0]
    put(root, "DBFilesClient\\CharSections.dbc", wdbc(10, [rec], names))
    put(root, "Character\\Human\\Male\\HumanMaleSkin00_00.blp")
    mnames = b"\0Creature\\DruidCat\\DruidCat.mdx\0Creature\\DruidBear\\DruidBear.mdx\0"
    put(root, "DBFilesClient\\SpellShapeshiftForm.dbc", wdbc(35, [form(1, [892]), form(5, [2281])]))
    put(root, "DBFilesClient\\CreatureDisplayInfo.dbc", wdbc(16, [cdi(892, 10), cdi(2281, 20)]))
    cmd = [[10, 0, 1] + [0] * 25, [20, 0, mnames.index(b"Creature\\DruidBear")] + [0] * 25]
    put(root, "DBFilesClient\\CreatureModelData.dbc", wdbc(28, cmd, mnames))
    put(root, "Creature\\DruidCat\\DruidCat.m2", m2(1))


health = os.path.join(ST, "server", "slice-health.sh")
good = os.path.join(tmp, "good"); make_slice(good)
r = subprocess.run(["bash", health, "--verify", good], capture_output=True, text=True)
check("slice-health --verify: every race and gender's model and skins there: whole", r.returncode == 0 and "WHOLE" in r.stdout, r.stdout[-600:])
check("slice-health: a character texture CharSections.dbc names is missing: a WARN, not a PROBLEM",
      "WARN: 1 character textures" in r.stdout and "HumanMaleFaceLower00_00.blp" in r.stdout, r.stdout[-600:])
check("slice-health: a shapeshift form's model is missing: a WARN naming it", "WARN: 1 shapeshift form models" in r.stdout
      and "Creature\\DruidBear\\DruidBear.m2" in r.stdout, r.stdout[-600:])
bad = os.path.join(tmp, "bad"); make_slice(bad, drop=("Character\\Tauren\\Female\\TaurenFemale02.skin",))
os.remove(os.path.join(bad, "Character", "Gnome", "Male", "GnomeMale.m2"))
r = subprocess.run(["bash", health, "--verify", bad], capture_output=True, text=True)
check("slice-health --verify: a missing player model or skin is a PROBLEM (the slice is not whole)",
      r.returncode == 1 and "PROBLEM: 2 player model files missing" in r.stdout and "GnomeMale.m2" in r.stdout
      and "TaurenFemale02.skin" in r.stdout, r.stdout[-600:])
plain = os.path.join(tmp, "plain")
for k in ("DBFilesClient\\Spell.dbc", "DBFilesClient\\Map.dbc", "DBFilesClient\\AreaTable.dbc", "Interface\\GlueXML\\GlueXML.toc"):
    put(plain, k)
r = subprocess.run(["bash", health, "--verify", plain], capture_output=True, text=True)
check("slice-health: a slice without a Character folder is checked as before", r.returncode == 0 and "player model" not in r.stdout, r.stdout)

# the report: the served slice, and the players' misses
wu = os.path.join(tmp, "srv", "wrath")
make_slice(os.path.join(wu, "stream", "slices", "mini"))
os.makedirs(os.path.join(wu, "portal-data", "stream"))
open(os.path.join(wu, "portal.env"), "w").write("STREAM_SLICES=mini\n")
miss = {"updated": 1793700000, "names": {
    "creature\\druidcat\\druidcat00.skin": {"name": "Creature\\DruidCat\\DruidCat00.skin", "n": 7},
    "character\\gnome\\male\\gnomemale.m2": {"name": "Character\\Gnome\\Male\\GnomeMale.m2", "n": 3},
    "world\\maps\\azeroth\\azeroth_1_1.adt": {"name": "World\\Maps\\Azeroth\\Azeroth_1_1.adt", "n": 1}}}
json.dump(miss, open(os.path.join(wu, "portal-data", "stream", "slice-misses.json"), "w"))
r = subprocess.run(["bash", health], capture_output=True, text=True, env=dict(env, WU_DIR=wu))
o = r.stdout
check("slice-health report: the player models line for the served slice", "player models: 20 races and genders, 80 skin files checked, all there" in o, o[-1500:])
check("slice-health report: the most missed names first, with counts", "3 names reported" in o
      and o.index("Creature\\DruidCat\\DruidCat00.skin") < o.index("Character\\Gnome\\Male\\GnomeMale.m2"), o[-1500:])
check("slice-health report: says which misses the slice now has and which a model folder still lacks",
      "GnomeMale.m2  (now in the slice)" in o and "DruidCat00.skin  (NOT in the slice, under a model or UI folder)" in o
      and "Azeroth_1_1.adt  (not in the slice)" in o, o[-1500:])
check("slice-health report: by folder", "by folder: Creature 1, Character 1, World 1" in o, o[-800:])
check("slice-health report: still ends with its verdict line", "SLICE HEALTH DONE:" in o.splitlines()[-1])
os.remove(os.path.join(wu, "portal-data", "stream", "slice-misses.json"))
o = subprocess.run(["bash", health], capture_output=True, text=True, env=dict(env, WU_DIR=wu)).stdout
check("slice-health report: no reports yet is said plainly", "none reported yet" in o, o[-800:])

print(f"\n{n - fails}/{n} passed")
sys.exit(1 if fails else 0)
