"""Mode packages (stream/modepack.py, docs/MODES.md "Mode packages"): the package checker (sizes, file names, links,
SVG, words, scripts, env, settings), the templates in hosting/kit/modes and their catalog stream/modepacks.json, the
host's answers and the env they make, the realm list's mode field, and the website's pages: /wrath/modes/<id> (live
realms running it, its forks), its picture, /wrath/modes/jam, /wrath/modes/hosted.json and the "Host a mode" section of
/wrath/modes, with a fake realm list. Also the deploy scripts carry modepack.py and modepacks.json.

    python3 -I stream/tests/test_modepack.py stream/wrath_portal.py
"""
import copy
import http.client
import importlib.util
import json
import os
import shutil
import sys
import tempfile
import threading
from http.server import ThreadingHTTPServer

PORTAL = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "wrath_portal.py")
HERE = os.path.dirname(os.path.abspath(PORTAL))
REPO = os.path.dirname(HERE)
PAGE_PORT = 18311
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example",
                  WU_RELAY_LISTING="http://127.0.0.1:9/realms.json", WU_ART_DIR=tempfile.mkdtemp())

spec = importlib.util.spec_from_file_location("mp", os.path.join(HERE, "modepack.py"))
mp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mp)

fails = []


def ok(cond, what):
    print(("ok   " if cond else "FAIL ") + what)
    if not cond:
        fails.append(what)


TPL = mp.TEMPLATES
DUELS = json.load(open(os.path.join(TPL, "duel-ladder", "mode.json")))

# ------------------------------------------------------------------ the templates and the catalog
for d in sorted(os.listdir(TPL)):
    clean, probs = mp.check_dir(os.path.join(TPL, d))
    ok(not probs and clean["id"] == d, f"template {d} passes the checker ({'; '.join(probs) or 'clean'})")
ok(set(os.listdir(TPL)) == set(mp.ORDER), "every template is in ORDER, and ORDER names only templates")
doc, probs = mp.build_catalog(jam=mp.jam_ok(mp.load_catalog().get("jam")))
ok(not probs and doc == mp.load_catalog(), "stream/modepacks.json is up to date (python3 -I stream/modepack.py --write)")
cat = mp.load_catalog()
ok([m["id"] for m in cat["modes"]] == list(mp.ORDER), "the catalog in ORDER")
ok(all(m["picture_data"].startswith("data:image/svg+xml;base64,") for m in cat["modes"]), "every template has its picture inline")
have = mp.known_scripts()
for m in cat["modes"]:
    ok(all(s in have for s in m["scripts"]), f"{m['id']}: its scripts ship in realm/custom/lua_scripts")
    if m["profile"]:
        ok(os.path.isfile(os.path.join(REPO, "realm", "profiles", m["profile"] + ".env")), f"{m['id']}: its ruleset exists ({m['profile']})")
    # every env name a template sets is one its scripts (or the core) actually read
    src = "".join(open(os.path.join(mp.SCRIPTS_DIR, s), encoding="utf-8").read() for s in m["scripts"])
    names = set(m["env"]) | {k for s in m["settings"] for k in s["env"]}
    for k in names:
        ok(k.startswith("AC_") or k in src, f"{m['id']}: {k} is read by its scripts")
surv = mp.template("survivors")
ok(surv["status"] == "coming" and surv["profile"] is None and surv["credit"]["source"].startswith("https://"),
   "Survivors: shown as coming, with its credit, never hosted")
ok(not any(mp.BRAND_RE.search(json.dumps({k: m.get(k) for k in ("name", "line", "settings")})) for m in cat["modes"]),
   "no game or publisher names in the templates")

# ------------------------------------------------------------------ the checker
def pkg(**files):
    d = tempfile.mkdtemp()
    for n, data in files.items():
        with open(os.path.join(d, n.replace("__", ".")), "wb") as f:
            f.write(data if isinstance(data, bytes) else data.encode())
    return d


def man(**kw):
    m = copy.deepcopy(DUELS)
    m.update(kw)
    return json.dumps(m)


SVG = open(os.path.join(TPL, "duel-ladder", "picture.svg"), "rb").read()
good = pkg(mode__json=man(id="my-duels", author="Ann", based_on={"id": "duel-ladder", "author": "Leyline"}), picture__svg=SVG)
clean, probs = mp.check_dir(good)
ok(not probs and clean["based_on"] == {"id": "duel-ladder", "author": "Leyline"}, "a fork passes, with its credit")


def bad(what, d, needle):
    _, probs = mp.check_dir(d)
    ok(any(needle in p for p in probs), f"refused: {what} ({'; '.join(probs)[:140]})")


bad("no mode.json", pkg(picture__svg=SVG), "mode.json is missing")
bad("a script of its own", pkg(mode__json=man(), picture__svg=SVG, evil__lua="os.execute('x')"), "evil.lua")
sub = pkg(mode__json=man(), picture__svg=SVG)
os.mkdir(os.path.join(sub, "scripts"))
bad("a folder inside", sub, "no folders")
lnk = pkg(mode__json=man(picture=None))
os.symlink("/etc/passwd", os.path.join(lnk, "README.md"))
bad("a link", lnk, "no links")
bad("a hidden file", pkg(mode__json=man(), picture__svg=SVG, **{"__env": "x"}), "hidden")
bad("an SVG with a script", pkg(mode__json=man(), picture__svg=b'<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>'), "no scripts")
bad("an SVG with an onload", pkg(mode__json=man(), picture__svg=b'<svg onload="x()"></svg>'), "no scripts")
bad("an SVG with an outside link", pkg(mode__json=man(), picture__svg=b'<svg><image href="https://evil.example/x.png"/></svg>'), "outside links")
bad("a big picture", pkg(mode__json=man(), picture__svg=b"<svg>" + b" " * (300 * 1024) + b"</svg>"), "256 KB")
bad("a PNG that isn't", pkg(mode__json=man(picture="picture.png"), picture__png=b"GIF89a"), "not a PNG")
bad("a picture it doesn't name", pkg(mode__json=man(picture=None), picture__svg=SVG), "names another picture")
bad("an unknown script", pkg(mode__json=man(scripts=["ov_nothere.lua"]), picture__svg=SVG), "not in the Host pack")
bad("a path as a script", pkg(mode__json=man(scripts=["../ov_duels.lua"]), picture__svg=SVG), "not a realm script")
for k in ("AC_LOGIN_DATABASE_INFO", "OV_DISCORD_WEBHOOK", "AC_SOAP_ENABLED", "OV_DEN_SAVES", "OV_DUELS_TEST", "WU_BOTS", "AC_PLAYER_LIMIT"):
    bad(f"env {k}", pkg(mode__json=man(env={k: "1"}), picture__svg=SVG), "not a setting a mode may change")
bad("an env value with a shell in it", pkg(mode__json=man(env={"OV_DUELS": "$(reboot)"}), picture__svg=SVG), "value")
bad("a setting over the fixed env", pkg(mode__json=man(settings=[{"key": "x", "label": "X", "type": "toggle", "default": True, "env": "OV_DUELS"}]),
                                         picture__svg=SVG), "already set")
bad("a number out of its range", pkg(mode__json=man(settings=[{"key": "x", "label": "X", "type": "number", "min": 5, "max": 2, "default": 3,
                                                                "env": "OV_DUELS_K"}]), picture__svg=SVG), "min <= default <= max")
bad("a brand word", pkg(mode__json=man(name="Warcraft duels"), picture__svg=SVG), "no game or publisher names")
bad("a blocked word", pkg(mode__json=man(name="Fuck duels"), picture__svg=SVG), "isn't allowed")
bad("a staff name", pkg(mode__json=man(id="my-duels", author="Leyline Staff"), picture__svg=SVG), "isn't allowed")
bad("the team's name on a fork", pkg(mode__json=man(id="my-duels", author="Leyline", based_on={"id": "duel-ladder", "author": "Leyline"}),
                                     picture__svg=SVG), "isn't allowed")
bad("a fork with its base's id", pkg(mode__json=man(based_on={"id": "duel-ladder", "author": "x"}), picture__svg=SVG), "id of its own")
bad("a Den file with another ruleset", pkg(mode__json=man(den="room.den.json"), picture__svg=SVG,
                                           room__den__json='{"format":"leyline-den","version":1,"props":[]}'), "den ruleset only")
DEN = json.load(open(os.path.join(TPL, "den", "mode.json")))
dd = pkg(mode__json=json.dumps(dict(DEN, id="my-den", author="Ann", based_on={"id": "den", "author": "Leyline"}, den="room.den.json")),
         picture__svg=SVG, room__den__json='{"format":"leyline-den","version":1,"props":[{"entry":1,"x":0,"y":0,"z":0}]}')
clean, probs = mp.check_dir(dd)
ok(not probs and clean["den"] == "room.den.json", f"a Den mode with its Den file passes ({probs})")
bad("a Den file that isn't", pkg(mode__json=json.dumps(dict(DEN, den="room.den.json")), picture__svg=SVG, room__den__json="return {}"), "Den file")
bad("too many files", pkg(mode__json=man(), picture__svg=SVG, README__md="hi", **{f"r{i}__den__json": "{}" for i in range(5)}), "6 files")
bad("a bad id", pkg(mode__json=man(id="Duel_Ladder"), picture__svg=SVG), "id:")
bad("schema from the future", pkg(mode__json=man(schema=2), picture__svg=SVG), "schema 1")

# ------------------------------------------------------------------ the host's answers and the env
m = mp.template("duel-ladder")
vals, probs = mp.values_for(m, {"min_s": "30", "streaks": False})
ok(not probs and vals == {"min_s": 30, "per_day": 3, "win_points": 3, "streaks": False}, "answers checked, defaults for the rest")
env = mp.env_for(m, vals)
ok(env == {"OV_DUELS": "1", "OV_DUEL_RESET": "1", "OV_DUELS_MIN_S": "30", "OV_DUELS_PAIR_PER_DAY": "3",
           "OV_DUELS_WIN_POINTS": "3", "OV_DUELS_STREAKS": "0"}, f"the env: {env}")
_, probs = mp.values_for(m, {"min_s": 1000})
ok(probs == ["Shortest duel that counts: 5 to 120."], f"out of range: {probs}")
lv = mp.template("levelling")
ok(mp.env_for(lv, {"xp": 5})["AC_RATE_XP_QUEST"] == "5", "one field sets several names (the levelling speed)")
_, probs = mp.values_for(mp.template("den"), {"look": "gold"})
ok(probs == ["The room's look: pick one."], "a choice takes its options only")

# ------------------------------------------------------------------ the realm list's field
lm = mp.listing_mode(m)
ok(lm["id"] == "duel-ladder" and lm["author"] == "Leyline" and lm["profile"] == "brawl" and "picture_data" not in lm,
   "the list carries the recipe, never the picture")
ok(mp.listing_mode({"id": "x-y", "name": "Warcraft", "author": "Ann"}) == {"id": "x-y", "author": "Ann"}, "credit only when it's not a whole mode")
ok(mp.listing_mode({"id": "../x"}) is None and mp.listing_mode("x") is None, "not a mode: None")

# ------------------------------------------------------------------ the website
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wp)
ok(wp.modepacks is not None, "the portal loads modepack.py from beside it")

LIST = [
    {"id": "aaaaaaaaaaaaaaaa", "name": "Duel Town", "live": True, "players": 7, "bots": 0, "mode": mp.listing_mode(m)},
    {"id": "bbbbbbbbbbbbbbbb", "name": "Ann Arena", "live": True, "players": 3, "bots": 0,
     "mode": mp.listing_mode(dict(m, id="ann-duels", name="Ann duels", author="Ann", based_on={"id": "duel-ladder", "author": "Leyline"},
                                  jam="first-jam"))},
    {"id": "cccccccccccccccc", "name": "Quiet Den", "live": False, "players": 0, "bots": 0, "mode": mp.listing_mode(mp.template("den"))},
    {"id": "dddddddddddddddd", "name": "No Mode", "live": True, "players": 50, "bots": 0},
]
wp.community_realms = lambda: (copy.deepcopy(LIST), True)
srv = ThreadingHTTPServer(("127.0.0.1", PAGE_PORT), wp.PageHandler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()


def get(path):
    c = http.client.HTTPConnection("127.0.0.1", PAGE_PORT, timeout=10)
    c.request("GET", path)
    r = c.getresponse()
    return r.status, r.getheader("Content-Type") or "", r.read()


st, ct, body = get("/wrath/modes/duel-ladder")
page = body.decode()
ok(st == 200 and "text/html" in ct, f"/wrath/modes/duel-ladder answers ({st})")
ok("Duel ladder" in page and "Duel Town" in page and "7 playing" in page, "the mode's page lists the live realms running it")
ok("Ann Arena" in page and "based on Duel ladder by Leyline" in page, "and its forks, with the credit")
ok("No Mode" not in page and "Quiet Den" not in page, "and nothing else")
ok('href="/wrath/r/aaaaaaaaaaaaaaaa"' in page, "each realm row links its realm page (stream/links.py)")
ok('src="/wrath/modes/duel-ladder/picture.svg"' in page and "Shortest duel that counts" in page, "its picture and what it sets")
ok('id="ly-site-nav"' in page, "in the site's chrome")
st, ct, body = get("/wrath/modes/duel-ladder/picture.svg")
ok(st == 200 and ct.startswith("image/svg+xml") and body == SVG, "the picture, as an image")
st, _, _ = get("/wrath/modes/den/picture.png")
ok(st == 404, "a picture it doesn't have: 404")
st, _, body = get("/wrath/modes/ann-duels")
ok(st == 200 and "Ann duels" in body.decode() and "Ann Arena" in body.decode(), "a fork a listed realm runs has a page too")
st, _, body = get("/wrath/modes/nobody-runs-this")
ok(st == 404 and "No such mode" in body.decode(), "an unknown mode: a plain 404 page")
st, _, body = get("/wrath/modes/jam")
page = body.decode()
ok(st == 200 and "Mode jam" in page and "Ann Arena" in page and "Duel Town" not in page, "the jam lists the forks entered in it")
ok("Bragging rights only" in page and "first-jam" in page, "bragging rights only, and how to enter")
st, ct, body = get("/wrath/modes/hosted.json")
h = {x["id"]: x for x in json.loads(body)["modes"]}
ok(st == 200 and h["duel-ladder"] == {"id": "duel-ladder", "realms": 1, "live": 1, "players": 7, "forks": 1},
   f"hosted.json: players on realms running each mode, and its forks ({h.get('duel-ladder')})")
ok(h["den"]["live"] == 0 and h["den"]["realms"] == 1, "an offline realm counts, not its players")
st, _, body = get("/wrath/modes")
page = body.decode()
ok(st == 200 and 'id="host-modes"' in page and "Host a mode" in page and 'href="/wrath/modes/minigolf-night"' in page,
   "/wrath/modes has the Host a mode section")
ok("1 live · 7 playing" in page, "with the players on realms running each mode")
rec = wp.clean_listing_record({"name": "X", "state": "live", "community": {"realmId": "eeeeeeeeeeeeeeee"}, "mode": dict(m, extra="x")})
ok(rec["mode"]["id"] == "duel-ladder" and "extra" not in rec["mode"], "the portal keeps a realm's mode from the listing, checked")
for p in ("/wrath/modes/jam/", "/wrath/modes/a", "/wrath/modes/../x", "/wrath/modes/den/picture.gif"):
    st, _, _ = get(p)
    ok(st in (200, 404), f"{p}: answers ({st})")

# ------------------------------------------------------------------ deploy
bat = open(os.path.join(HERE, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
ok("modepack.py modepacks.json" in bat, "the sign-in step uploads modepack.py with its catalog")
fl = open(os.path.join(HERE, "fastloop.sh"), encoding="utf-8").read()
ok("modepack.py" in fl and "modepacks.json" in fl and "--verify" not in fl.split("modepack.py")[0][-5:], "fastloop.sh installs them")
kit = open(os.path.join(REPO, "tools", "leyline-repo", "hostkit.py"), encoding="utf-8").read()
ok("SKIP" in kit, "the Host pack carries hosting/kit/modes (the kit folder, nothing skips it)")

srv.shutdown()
print(f"\n{len(fails)} failed" if fails else "\nall passed")
sys.exit(1 if fails else 0)
