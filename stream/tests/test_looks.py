"""stream/looks.py and stream/looks.json: every entry checks out, the rules (no brand words, no client numbers in
words, previews only ours or under an open licence, install blocks pinned to the author's own release), how a look
installs is worked out (never trusted), all.json gets the published looks, and the words and ids agree with the hub
and the launcher (hub.CLIENT_LABEL and BANNED_RE, gfx.rs PRESETS).

    python3 -I stream/tests/test_looks.py
"""
import copy, hashlib, importlib.util, json, os, re, sys, tempfile

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ROOT = os.path.dirname(HERE)


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    sys.modules[name] = m
    spec.loader.exec_module(m)
    return m


LK = load("wu_looks", os.path.join(HERE, "looks.py"))
HB = load("wu_hub_for_looks", os.path.join(HERE, "hub.py"))

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, extra)


raw = json.load(open(os.path.join(HERE, "looks.json"), encoding="utf-8"))
items = raw["looks"]
doc = LK.load()
check("looks.json: every entry checks out (pending ones too)", doc["bad"] == [] and len(doc["looks"]) == len(items), doc["bad"])
check("looks.json: unique ids, look:<slug>", len({e["id"] for e in items}) == len(items) and all(e["id"].startswith("look:") for e in items))
pub = LK.published()
check("published: only reviewed entries", [e["id"] for e in pub] == [e["id"] for e in doc["looks"] if e["review"]["by"]]
      and all(e["review"]["by"] for e in pub) and any(not e["review"]["by"] for e in items))
check("published: the three presets, Forever UI and the UltraHD link among them",
      {"look:max", "look:balanced", "look:classic", "look:forever-ui", "look:ultrahd"} <= {e["id"] for e in pub})
check("every entry: a licence and where it came from, or \"\" (link only then)",
      all(isinstance(e["licence"], str) for e in items)
      and all(LK.how(e, set()) == "link" for e in items if not e["licence"] and e["look"]["type"] != "preset" and not e["look"]["uses"]))

# the words agree with the hub's and the launcher's
check("words: CLIENT_LABEL is the hub's", LK.CLIENT_LABEL == HB.CLIENT_LABEL)
check("words: the brand-word rule is the hub's", all(bool(LK._BANNED.search(t)) == HB.banned(t) for t in (
      "World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard", "MalfurionWoW", "WotLK", "Wrath", "wowhead", "Wowser", "pfUI")))
gfx = open(os.path.join(ROOT, "launcher", "src-tauri", "src", "gfx.rs"), encoding="utf-8").read()
check("presets: the ids are gfx.rs PRESETS", sorted(re.findall(r'^    \("([a-z]+)", "[A-Z][a-z]+", &\[', gfx, re.M)) == sorted(LK.PRESETS),
      re.findall(r'^    \("([a-z]+)", "[A-Z][a-z]+", &\[', gfx, re.M))
check("presets: Max, Balanced and Classic map to the launcher's ultra, balanced and classic",
      {e["name"]: e["look"]["preset"] for e in pub if e["look"]["type"] == "preset"} == {"Max": "ultra", "Balanced": "balanced", "Classic": "classic"})

# the rules
base = copy.deepcopy(next(e for e in items if e["id"] == "look:epoch-water"))


def bad(change, want):
    e = copy.deepcopy(base)
    change(e)
    p = LK.problems(e)
    return any(want in x for x in p), p


for label, change, want in (
    ("brand words in a name", lambda e: e.update(name="WoW water"), "brand words"),
    ("brand words in the words", lambda e: e.update(description="Water for World of Warcraft."), "brand words"),
    ("a client number in the words", lambda e: e.update(description="Water for 3.3.5a."), "client numbers"),
    ("a page that is a file", lambda e: e.update(page="https://example.com/pack.zip"), "never a file"),
    ("a plain http page", lambda e: e.update(page="http://example.com/x"), "https"),
    ("an unknown build", lambda e: e.update(builds=["9.9"]), "builds"),
    ("an unknown change word", lambda e: e["look"].update(changes=["Everything"]), "look.changes"),
    ("a preset id the launcher doesn't have", lambda e: e["look"].update(type="preset", preset="insane"), "look.preset"),
    ("a picture from somewhere else", lambda e: e["look"].update(previews=[{"src": "https://i.imgur.com/x.webp", "basis": "ours",
                                                                            "alt": "x", "credit": "x", "w": 100, "h": 50}]), "our own"),
    ("the author's picture without an open licence", lambda e: e["look"].update(previews=[{"src": "/wrath/art/looks/water.webp", "basis": "licence",
                                                                                         "alt": "Water", "credit": "Its author", "w": 100, "h": 50}]), "open licence"),
    ("a UI look with an install block", lambda e: e.update(install={"kind": "files"}) or e["look"].update(type="ui-look"), "look.uses"),
    ("uses that are not curated addon ids", lambda e: e["look"].update(type="ui-look", uses=["mod:x"]), "look.uses"),
):
    ok, p = bad(change, want)
    check(f"refused: {label}", ok, p)
ours = copy.deepcopy(base)
ours["look"]["previews"] = [{"src": "/wrath/art/looks/water.webp", "basis": "ours", "alt": "Calm water at dusk", "credit": "Leyline",
                             "w": 640, "h": 320}]
check("a picture we made, served from our own art folder, is fine", LK.problems(ours) == [], LK.problems(ours))

# install blocks: the author's own release file, pinned
mpq = b"MPQ\x1a" + b"\0" * 28
sha = hashlib.sha256(mpq).hexdigest()


def block(url, files=None, **kw):
    files = files or {"patch-W.MPQ": sha}
    b = {"kind": "files", "from": "release-asset", "url": url, "rev": "v1", "sha256": sha, "size": 32, "file_count": len(files),
         "strip_prefix": "", "files": files, "files_digest": LK.files_digest(files)}
    b.update(kw)
    return b


pack = copy.deepcopy(base)
pack["install"] = block("https://github.com/ZythDr/EpochWater/releases/download/v1/patch-W.MPQ")
check("install: a .MPQ release file of the entry's own repo, with its sha256: sound, and one click", LK.problems(pack) == []
      and LK.how(pack, set()) == "launcher", LK.problems(pack))
check("install: the digest is the launcher's (sorted <path>\\0<sha>\\n lines)",
      LK.files_digest({"b": "2" * 64, "a": "1" * 64}) == hashlib.sha256(("a\0" + "1" * 64 + "\nb\0" + "2" * 64 + "\n").encode()).hexdigest())
for label, inst, want in (
    ("another repo's file", block("https://github.com/mallory/EpochWater/releases/download/v1/patch-W.MPQ"), "own repo"),
    ("a commit archive (not byte-stable)", block("https://github.com/ZythDr/EpochWater/archive/" + "a" * 40 + ".zip", **{"from": "commit-archive"}), "release-asset"),
    ("no sha256", block("https://github.com/ZythDr/EpochWater/releases/download/v1/patch-W.MPQ", sha256=None), "sha256"),
    ("a program inside", block("https://github.com/ZythDr/EpochWater/releases/download/v1/pack.zip", files={"d3d9.dll": sha}), "client patch"),
    ("an unsafe path", block("https://github.com/ZythDr/EpochWater/releases/download/v1/pack.zip", files={"../patch-W.MPQ": sha}), "client patch"),
    ("a wrong digest", block("https://github.com/ZythDr/EpochWater/releases/download/v1/patch-W.MPQ", files_digest="0" * 64), "files_digest"),
    ("a bare file named otherwise", block("https://github.com/ZythDr/EpochWater/releases/download/v1/other.MPQ"), "the one .MPQ"),
):
    e = copy.deepcopy(base)
    e["install"] = inst
    p = LK.problems(e)
    check(f"install refused: {label}", any(want in x for x in p), p)

# how a UI look installs: only when every curated addon it uses is reviewed and published
df = next(e for e in items if e["id"] == "look:dragonflight-ui")
check("how: a UI look is one click only when all its addons are published", LK.how(df, set()) == "link" and LK.how(df, {"up:dragonflightui"}) == "launcher")
cf = next(e for e in items if e["id"] == "look:clean-frames")
check("how: one missing addon makes it a link", LK.how(cf, {"up:ruf", "up:dominos"}) == "link"
      and LK.how(cf, {"up:ruf", "up:dominos", "up:pretty-minimap"}) == "launcher")
check("how: upstream_ids are the published curated addons", "up:ruf" in LK.upstream_ids() and "up:dragonflightui" not in LK.upstream_ids())

# all.json
src = json.dumps({"format": 1, "projects": [{"id": "mod:pings", "name": "Pings", "source": "packaged"}]}).encode()
out = json.loads(LK.add_looks(src))
ids = [p["id"] for p in out["projects"]]
check("add_looks: the published looks, once each, sorted by name with the rest", "look:max" in ids and "mod:pings" in ids
      and len(ids) == len(set(ids)) == 1 + len(pub) and [p["name"].lower() for p in out["projects"]] == sorted(p["name"].lower() for p in out["projects"]))
mx = next(p for p in out["projects"] if p["id"] == "look:max")
check("add_looks: each with look.how, the credit line and a page", mx["look"]["how"] == "launcher" and mx["credits"] == "Max by Leyline · MIT · github.com"
      and mx["page"] == "https://github.com/orionopenwow/Leyline")
check("add_looks: an odd body is left alone", LK.add_looks(b"not json") == b"not json" and LK.add_looks(src, items=[]) == src)

# the page
page = LK.body({"v": "<script>", "t": "x"})
check("page: no script, nothing echoed", "<script" not in page)
check("page: one card per published look for the offered clients", page.count('<li class="lk-card"') == len([e for e in pub if any(b in LK.BUILDS[:3] for b in e["builds"])]))
hidden = copy.deepcopy(base)
hidden.update(id="look:later", builds=["5.4.8"])
hidden["review"] = {"by": "x", "at": "", "notes": ""}
check("page: looks only for clients not offered yet stay off the page", 'id="look-later"' not in LK.body({}, items=[hidden])
      and 'id="look-later"' in LK.body({}, items=[hidden], offered=list(LK.BUILDS)))
check("page: the featured looks for the front page", [e["id"] for e in LK.featured(3)] == ["look:max", "look:balanced", "look:classic"])
check("size words", [LK.size_words(x) for x in (None, 0, 2048, 7387881, 2 * 1024 ** 3)] ==
      ["Size on the author's page", "Nothing to download", "2 KB", "7 MB", "2.0 GB"])

# a bad file is never half-served
with tempfile.TemporaryDirectory() as d:
    p = os.path.join(d, "looks.json")
    open(p, "w").write(json.dumps({"format": 1, "looks": [dict(base, review={"by": "x"}), dict(base, name="WoW", id="look:bad", review={"by": "x"})]}))
    got = LK.load(p)
    check("load: a bad entry is left out with its reasons, the rest still show", [e["id"] for e in got["looks"]] == ["look:epoch-water"]
          and got["bad"] and got["bad"][0][0] == "look:bad")
    open(p, "w").write("{")
    check("load: an unreadable file is an empty list", LK.load(p) == {"looks": [], "bad": []})

print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
