"""Race/class combinations (docs/RACE-CLASS-COMBOS.md): every copy of the pair list agrees, the preset reaches every
part, and the portal's page env follows it.

  * the 83 modern pairs: realm/modern_combos.sql, realm/charfix_world.sql (start rows, its 21 new pairs' bars and
    gear donors), the browser client's MODERN_RACE_CLASS, the launcher's combos.rs MODERN and DONORS, the docs table;
  * every stock 3.3.5a pair is among them; the "all" set is the 100;
  * the preset: realm profiles, charfix.sh (mod-arac's pinned commit = sources.lock, the sha256 check, 83 / 100),
    the Control steps, the client patch series;
  * the portal: combos_preset() (file over env, junk is modern), /__env.js for this realm only, the listing's block;
  * no mod-arac game-format file anywhere in the repo, and no brand word in the docs page.

    python3 stream/tests/test_race_class_combos.py stream/wrath_portal.py
"""
import ast, importlib.util, json, os, re, sys, tempfile

PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
ROOT = os.path.dirname(os.path.dirname(PORTAL))
TMP = tempfile.mkdtemp(prefix="wu-combos-")
passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1; print("PASS", name)
    else:
        failed += 1; print("FAIL", name, str(extra)[:500])


def read(*p):
    with open(os.path.join(ROOT, *p), encoding="utf-8") as f:
        return f.read().replace("\r\n", "\n")


def pairs(text):
    return [tuple(map(int, m)) for m in re.findall(r"\(\s*(\d+)\s*,\s*(\d+)\s*\)", text)]


RACES = [1, 2, 3, 4, 5, 6, 7, 8, 10, 11]
CLASSES = [1, 2, 3, 4, 5, 6, 7, 8, 9, 11]
ALL = {(r, c) for r in RACES for c in CLASSES}
STOCK = {(1, 1), (1, 2), (1, 4), (1, 5), (1, 6), (1, 8), (1, 9), (2, 1), (2, 3), (2, 4), (2, 6), (2, 7), (2, 9),
         (3, 1), (3, 2), (3, 3), (3, 4), (3, 5), (3, 6), (4, 1), (4, 3), (4, 4), (4, 5), (4, 6), (4, 11),
         (5, 1), (5, 4), (5, 5), (5, 6), (5, 8), (5, 9), (6, 1), (6, 3), (6, 6), (6, 7), (6, 11),
         (7, 1), (7, 4), (7, 6), (7, 8), (7, 9), (8, 1), (8, 3), (8, 4), (8, 5), (8, 6), (8, 7), (8, 8),
         (10, 2), (10, 3), (10, 4), (10, 5), (10, 6), (10, 8), (10, 9), (11, 1), (11, 2), (11, 3), (11, 5),
         (11, 6), (11, 7), (11, 8)}

# ---------------------------------------------------------------- 1. the pair lists
modern = pairs(read("realm", "modern_combos.sql").split("NOT IN", 1)[1])
MODERN = set(modern)
check("modern_combos.sql: 83 pairs, each once", len(modern) == 83 and len(MODERN) == 83, len(modern))
check("every stock pair is a modern pair (62)", len(STOCK) == 62 and STOCK <= MODERN)
check("every modern pair is a playable race and class", MODERN <= ALL)

cf = read("realm", "charfix_world.sql")
cf_keep = set(pairs(cf.split("NOT IN", 1)[1].split(";", 1)[0]))
check("charfix_world.sql keeps the same 83", cf_keep == MODERN, sorted(cf_keep ^ MODERN))
cf_new = set(pairs(re.search(r"DELETE FROM `playercreateinfo_action` WHERE \(`race`, `class`\) IN \((.*?)\);", cf).group(1)))
check("charfix_world.sql's new pairs are exactly modern minus stock (21)", cf_new == MODERN - STOCK, sorted(cf_new ^ (MODERN - STOCK)))
cf_donors = {(int(a), int(b), int(c)) for a, b, c in re.findall(r"\((\d+),(\d+),(\d+)\)", re.search(r"INSERT INTO `ov_charfix_donor` VALUES (.*?);", cf).group(1))}

wip = read("client", "patches", "wotlk-web-port-wip.patch")
blk = re.search(r"\+const MODERN_RACE_CLASS: &\[\(u8, u8\)\] = &\[\n(.*?)\n\+\];", wip, re.S)
check("browser client: MODERN_RACE_CLASS found", blk is not None)
check("browser client: MODERN_RACE_CLASS is the 83", blk is not None and set(pairs(blk.group(1))) == MODERN)

rs = read("launcher", "src-tauri", "src", "combos.rs")
lm = re.search(r"pub const MODERN: &\[\(u8, u8\)\] = &\[\n(.*?)\n\];", rs, re.S)
check("launcher combos.rs: MODERN is the 83", lm is not None and set(pairs(lm.group(1))) == MODERN and len(pairs(lm.group(1))) == 83)
ld = re.search(r"pub const DONORS: &\[\(u8, u8, u8\)\] = &\[\n(.*?)\n\];", rs, re.S)
l_donors = {tuple(map(int, t)) for t in re.findall(r"\((\d+), (\d+), (\d+)\)", ld.group(1))} if ld else set()
check("launcher combos.rs: DONORS are charfix's (the preview wears what the server gives)", l_donors == cf_donors and len(l_donors) == 21,
      sorted(l_donors ^ cf_donors))
check("each donor has the class as a stock pair", all((d, c) in STOCK for _, c, d in l_donors))
check("launcher combos.rs: the 10 races and 10 classes", "pub const RACES: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 10, 11];" in rs
      and "pub const CLASSES: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 11];" in rs)

new_patch = read("client", "patches", "wotlk-client-20261105a.patch")
check("browser client 20261105a: the 10 races and classes for allcombos",
      "+const RACES: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 10, 11];" in new_patch
      and "+const CLASSES: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 11];" in new_patch and 'WOW_ALLCOMBOS' in new_patch)
series = [l.strip() for l in read("client", "patches", "series.txt").splitlines() if l.strip() and not l.startswith("#")]
check("client series: 20261105a is last", series[-1] == "wotlk-client-20261105a.patch", series[-3:])

doc = read("docs", "RACE-CLASS-COMBOS.md")
names_r = {1: "Human", 2: "Orc", 3: "Dwarf", 4: "Night Elf", 5: "Undead", 6: "Tauren", 7: "Gnome", 8: "Troll", 10: "Blood Elf", 11: "Draenei"}
order_c = CLASSES
doc_ok, doc_bad = True, []
for r, n in names_r.items():
    m = re.search(rf"^\| {re.escape(n)} \({r}\) \| (.*) \|$", doc, re.M)
    if not m:
        doc_ok = False; doc_bad.append(n); continue
    cells = [x.strip() for x in m.group(1).split("|")]
    want = ["yes" if (r, c) in STOCK else "**new**" if (r, c) in MODERN else "all" for c in order_c]
    if cells != want:
        doc_ok = False; doc_bad.append((n, cells))
check("docs/RACE-CLASS-COMBOS.md: the table matches the lists", doc_ok, doc_bad)
check("docs: no brand word", not re.search(r"world of warcraft|warcraft|lich king|blizzard|\bwow\b", doc, re.I))

# ---------------------------------------------------------------- 2. the preset everywhere
for prof in ("main", "dalaran"):
    env = dict(re.findall(r"^([A-Z][A-Z0-9_]*)=(.*)$", read("realm", "profiles", prof + ".env"), re.M))
    check(f"{prof}.env: OV_RACE_CLASS_COMBOS=modern", env.get("OV_RACE_CLASS_COMBOS") == "modern", env.get("OV_RACE_CLASS_COMBOS"))
    check(f"{prof}.env: no race, class or creation switched off",
          all(env.get(k) == "0" for k in ("AC_CHARACTER_CREATING_DISABLED", "AC_CHARACTER_CREATING_DISABLED_RACE_MASK",
                                           "AC_CHARACTER_CREATING_DISABLED_CLASS_MASK")))
sh = read("realm", "charfix.sh")
lock = re.search(r"^mod-arac \S+ \S+ ([0-9a-f]{40})$", read("realm", "custom", "sources.lock"), re.M).group(1)
check("charfix.sh: mod-arac's commit is sources.lock's", f"ARAC_COMMIT={lock}" in sh)
check("charfix.sh: arac.sql's sha256 is checked before it runs",
      re.search(r"ARAC_SHA256=[0-9a-f]{64}", sh) is not None and sh.index('sha256sum "$ARAC_SQL"') < sh.index('sqlw < "$ARAC_SQL"'))
check("charfix.sh: 83 for modern, 100 for all, anything else refused",
      "modern) WANT_PAIRS=83" in sh and "all) WANT_PAIRS=100" in sh and "is not modern or all" in sh)
check("charfix.sh: the preset kept in the profile and told to the website",
      'kv OV_RACE_CLASS_COMBOS "$COMBOS"' in sh and "portal-data/leyline/race-class-combos.txt" in sh)
check("charfix.sh: arac.sql after charfix_world.sql", sh.index('sqlw < "$KIT/charfix_world.sql"') < sh.index('sqlw < "$ARAC_SQL"'))
check("rollback: the website's file follows the restored profile", "race-class-combos.txt" in read("realm", "charfix_rollback.sh"))
bat = read("realm", "Fix character create.bat")
check("Fix character create.bat: passes the preset (modern / all / none)", "CHARFIX_COMBOS=%COMBOS%" in bat and '"%~1"=="all"' in bat)
tree = ast.parse(read("tools", "control", "control.py"))
steps = next(ast.literal_eval(n.value) for n in tree.body if isinstance(n, ast.Assign) and any(getattr(t, "id", "") == "STEPS" for t in n.targets))
check("Control: combos, combos-all, combos-modern, combos-check",
      steps.get("combos", ("",))[0] == "realm/Fix character create.bat"
      and steps.get("combos-all", ("",))[0] == "realm/Race class combos - all.bat"
      and steps.get("combos-modern", ("",))[0] == "realm/Race class combos - modern.bat"
      and steps.get("combos-check", ("",))[0] == "realm/Diagnose character create.bat")
check("Control: combos-check is read-only", '"combos-check"' in read("tools", "control", "control.py").split("READ_ONLY = {", 1)[1].split("}", 1)[0])
for w, arg in (("Race class combos - all.bat", "all"), ("Race class combos - modern.bat", "modern")):
    check(f"{w}: calls the fix with {arg}", f'call "%~dp0Fix character create.bat" {arg}' in read("realm", w))
check("launcher: mods never get slot Y", 'const PATCH_SLOTS: &str = "456789ABCDEFGHIJKLMNOPQRSUVWX";' in read("launcher", "src-tauri", "src", "mods.rs"))
check("launcher: PLAY on the official realm prepares the tables, elsewhere takes them away",
      "crate::combos::prepare_client(&profile.path, preset)" in read("launcher", "src-tauri", "src", "app.rs"))

# ---------------------------------------------------------------- 3. no game data from mod-arac
bad = []
for d, _, files in os.walk(ROOT):
    if any(x in d for x in (os.sep + ".git", "node_modules", os.sep + "target")):
        continue
    for f in files:
        low = f.lower()
        if low in ("patch-a.mpq", "charbaseinfo.dbc", "charstartoutfit.dbc", "skillraceclassinfo.dbc"):
            bad.append(os.path.relpath(os.path.join(d, f), ROOT))
check("no mod-arac Patch-A.MPQ or its tables in the repo", not bad, bad)

# ---------------------------------------------------------------- 4. the portal
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example", WU_DL_PROBE="0",
                  WU_PORTAL_DATA=os.path.join(TMP, "data"), WU_HANGOUT_REALM="hangouthangout22",
                  WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json", WU_ART_DIR=os.path.join(TMP, "art"))
os.environ.pop("WU_RACE_CLASS_COMBOS", None)
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)


def env_of(sess):
    txt = wp.env_js(sess).decode()
    return json.loads(txt.split("window.__wenilla_env = ", 1)[1].split(";\n", 1)[0]), txt


def set_file(word):
    p = wp.combos_file()
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w") as f:
        f.write(word)
    st = os.stat(p)
    os.utime(p, ns=(st.st_atime_ns, st.st_mtime_ns + 1_000_000_000))   # a new mtime even within one tick


S = {"user": "WR1", "pass": "pw", "slice": "mini", "tier": 2, "mode": "stream"}
check("preset: modern with no file and no env", wp.combos_preset() == "modern")
os.environ["WU_RACE_CLASS_COMBOS"] = "ALL"
check("preset: the env when there is no file", wp.combos_preset() == "all")
os.environ["WU_RACE_CLASS_COMBOS"] = "junk"
check("preset: a junk env is modern", wp.combos_preset() == "modern")
os.environ["WU_RACE_CLASS_COMBOS"] = "all"
set_file("modern\n")
check("preset: the realm's file wins over the env", wp.combos_preset() == "modern")
e, txt = env_of(S)
check("env (modern): moderncombos only", e.get("moderncombos") == "1" and "allcombos" not in e, e)
set_file("all\n")
e, _ = env_of(S)
check("env (all): moderncombos and allcombos (an older client build still gets the 83)", e.get("moderncombos") == "1" and e.get("allcombos") == "1", e)
e, _ = env_of(dict(S, hop={"id": "hangouthangout22", "name": "W", "home": True}))
check("env: the realm's Beacon gets them too", e.get("moderncombos") == "1" and e.get("allcombos") == "1", e)
e, _ = env_of(dict(S, hop={"id": "abcdefghijklmnop", "name": "Frost", "home": False, "pre": True}))
check("env: a community realm gets neither", "moderncombos" not in e and "allcombos" not in e, e)
set_file("off\n")
e, _ = env_of(S)
check("env (off): neither", "moderncombos" not in e and "allcombos" not in e, e)
set_file("../../etc\n")
check("preset: a junk file is modern", wp.combos_preset() == "modern")
check("listing: our realm's block carries the preset", wp.sample_info().get("combos") == "modern")
check("env: the page's ?moderncombos= / ?allcombos= win, =0 turns both off",
      "['moderncombos','allcombos'].forEach" in txt and "if(e.moderncombos==='0')e.allcombos='0'" in txt)

print(f"{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
