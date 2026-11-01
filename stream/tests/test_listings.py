"""List your mod (stream/listings.py with stream/projects.py, stream/workshop.py, stream/hub_rank.py, Leyline HQ's hooks):
links of every shape, pinning, format detection and checks for each format, per-file sha256, the policy (live or
waiting), rate limits and new accounts, reports into HQ and auto-hide, author edit/remove, updates when the author
pushes, all.json for the launcher, Rising. Fake forges in memory: no network.

    python3 -I stream/tests/test_listings.py
"""
import hashlib, importlib.util, io, json, os, shutil, sys, tempfile, time, zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
ROOT = os.path.dirname(STREAM)
T = tempfile.mkdtemp(prefix="wu-listings-")
os.environ["WU_PORTAL_DATA"] = os.path.join(T, "data")
os.makedirs(os.environ["WU_PORTAL_DATA"])

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, extra)


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, os.path.join(STREAM, file))
    m = importlib.util.module_from_spec(spec)
    sys.modules[name] = m
    spec.loader.exec_module(m)
    return m


L = load("wu_listings", "listings.py")
PJ = load("wu_projects_t", "projects.py")
RANK = load("wu_hub_rank_t", "hub_rank.py")
WS = load("wu_workshop_t", "workshop.py")

# ---------------------------------------------------------------- the shared rules cases
cases = json.load(open(os.path.join(ROOT, "launcher", "src-tauri", "src", "upstream-cases.json")))
bad = [c for c in cases["classify"] if not L.classify(c["path"], c["blp_ok"], bytes.fromhex(c.get("head", ""))).startswith(c["expect"])]
bad += [c for c in cases["names"] if L.name_ok(c["name"]) != c["ok"]]
bad += [c for c in cases["interface"] if L.interface_ok(c["value"]) != c["ok"]]
bad += [c for c in cases["clean_name"] if L.clean_name(c["raw"]) != c["want"]]
check("the shared upstream-cases.json hold for listings' copy of the rules", not bad, bad[:3])

# ---------------------------------------------------------------- links
P = L.parse_source
s = P("https://github.com/alice/Frames")
check("a repo link", s["forge"] == "github" and s["repo_url"] == "https://github.com/alice/Frames" and not s["ref"] and not s["latest"])
check("a .git link", P("https://github.com/alice/Frames.git/")["repo"] == "Frames")
check("a branch", P("https://github.com/alice/Frames/tree/dev")["ref"] == "dev")
C1 = "0123456789abcdef0123456789abcdef01234567"
check("a commit", P(f"https://github.com/alice/Frames/commit/{C1}")["commit"] == C1)
check("a commit archive", P(f"https://github.com/alice/Frames/archive/{C1}.zip")["commit"] == C1)
check("a release tag", P("https://github.com/alice/Frames/releases/tag/v1.2")["tag"] == "v1.2")
s = P("https://github.com/alice/Frames/releases/download/v1.2/Frames-wotlk.zip")
check("a release file", s["tag"] == "v1.2" and s["asset"] == "Frames-wotlk.zip")
check("latest release", P("https://github.com/alice/Frames/releases/latest")["latest"])
check("gitlab branch", P("https://gitlab.com/dave/raid/-/tree/main")["ref"] == "main")
check("codeberg branch", P("https://codeberg.org/erin/pack/src/branch/main")["ref"] == "main")
for raw, why in [("http://github.com/a/b", "https"), ("https://example.org/a/b", "GitHub, GitLab or Codeberg"),
                 ("https://github.com/a", "repo itself"), ("https://github.com/a/b/blob/main/x.lua", "one file"),
                 ("https://user:pw@github.com/a/b", "https"), ("https://github.com/a/b c", "spaces"), ("", "Paste")]:
    try:
        P(raw)
        check("refused: " + raw, False)
    except ValueError as e:
        check("refused: " + (raw or "(empty)"), why in str(e), str(e))


# ---------------------------------------------------------------- a fake forge
def zip_of(files: dict, top: str = "") -> bytes:
    b = io.BytesIO()
    with zipfile.ZipFile(b, "w", zipfile.ZIP_DEFLATED) as z:
        for n, data in sorted(files.items()):
            z.writestr(top + n, data)
    return b.getvalue()


class FakeNet(L.Net):
    def __init__(self):
        self.api_docs = {}
        self.blobs = {}
        self.fetched = []

    def json(self, forge, path):
        return self.api_docs.get((forge, path), "missing")

    def download(self, url, dest, cap):
        self.fetched.append(url)
        b = self.blobs[url]
        if len(b) > cap:
            raise ValueError("too big")
        open(dest, "wb").write(b)
        return len(b), hashlib.sha256(b).hexdigest()


net = FakeNet()


def gh_repo(o, r, files, sha, release=None, top=None, branch="main"):
    """Publish a fake GitHub repo: its meta, the branch head, the commit archive, optionally a latest release."""
    net.api_docs[("github", f"/repos/{o}/{r}")] = {"name": r, "default_branch": branch, "private": False}
    net.api_docs[("github", f"/repos/{o}/{r}/commits/{branch}")] = {"sha": sha}
    net.api_docs[("github", f"/repos/{o}/{r}/commits/{sha}")] = {"sha": sha}
    net.blobs[f"https://github.com/{o}/{r}/archive/{sha}.zip"] = zip_of(files, (top or f"{r}-{sha}") + "/")
    if release:
        tag, name, blob = release
        url = f"https://github.com/{o}/{r}/releases/download/{tag}/{name}"
        net.blobs[url] = blob
        rel = {"tag_name": tag, "draft": False, "assets": [{"name": name, "size": len(blob), "browser_download_url": url,
                                                             "digest": "sha256:" + hashlib.sha256(blob).hexdigest()}]}
        net.api_docs[("github", f"/repos/{o}/{r}/releases/latest")] = rel
        net.api_docs[("github", f"/repos/{o}/{r}/releases/tags/{tag}")] = rel


TOC = "## Interface: 30300\n## Title: |cff00ff00Frames|r (WotLK)\n## Version: 1.2\n## Author: alice\nFrames.lua\n"
SHA = {i: (format(i, "x") * 40)[:40] for i in range(1, 30)}
gh_repo("alice", "Frames", {"Frames.toc": TOC, "Frames.lua": "print('hi')\n", "README.md": "# hi", ".github/x.yml": "a: 1"}, SHA[1])
gh_repo("alice", "pack", {"Frames/Frames.toc": TOC, "Frames/Frames.lua": "x=1", "Bags/Bags.toc": TOC.replace("Frames.lua", "Bags.lua"),
                          "Bags/Bags.lua": "y=1", "Bags/Media/icon.blp": b"BLP2xxxx"}, SHA[2])
gh_repo("alice", "stockart", {"Frames/Frames.toc": TOC, "Frames/Frames.lua": "x=1",
                              "Frames/Interface/Buttons/UI-Panel-Button-Up.blp": b"BLP2xxxx"}, SHA[3])
MODJSON = json.dumps({"id": "dalaran-dice", "name": "Dalaran Dice", "version": "0.1.0", "author": "bob", "licence": "MIT",
                      "kind": "both", "wu": "1.0", "description": "Roll dice."})
gh_repo("bob", "dice", {"mod.json": MODJSON, "ui/DalaranDice/DalaranDice.toc": TOC.replace("Frames.lua", "DalaranDice.lua"),
                        "ui/DalaranDice/DalaranDice.lua": "x=1", "realm/dice.lua": "RegisterPlayerEvent(42, function() end)\n"}, SHA[4])
gh_repo("bob", "baddice", {"mod.json": MODJSON, "ui/DalaranDice/DalaranDice.toc": TOC.replace("Frames.lua", "DalaranDice.lua"),
                           "ui/DalaranDice/DalaranDice.lua": "x=1", "realm/dice.lua": "local f = io.open('x')\n"}, SHA[5])
MPQ = b"MPQ\x1a" + b"\0" * 60
gh_repo("tex", "hd-textures", {"README.md": "x"}, SHA[6], release=("v2", "patch-H.MPQ", MPQ))
gh_repo("tex", "zipped", {"patch-T.MPQ": MPQ, "readme.txt": "x"}, SHA[7])
gh_repo("tex", "fakempq", {"patch-T.MPQ": b"nope" * 10}, SHA[8])
gh_repo("nat", "fps", {"d3d9.dll": b"MZ\x90\x00" + b"\0" * 20, "d3d9.ini": "fps=144"}, SHA[9])
gh_repo("rs", "events", {"events.lua": "local E = 3\nRegisterServerEvent(E, function() end)\n", "README.md": "x"}, SHA[10])
gh_repo("rs", "evil", {"evil.lua": "RegisterPlayerEvent(42, function() os.execute('rm') end)\n"}, SHA[11])
MODE = 'Mode.Title = "Coin Dash"\nMode.Keys = {"W"}\nfunction OnStart(p) end\nfunction OnTick(dt) end\n'
gh_repo("gm", "coins", {"coins.lua": MODE}, SHA[12])
gh_repo("gm", "bigmode", {"big.lua": MODE + "-- x\n" * 4000}, SHA[13])
gh_repo("ld", "loose", {"Interface/Glues/x.blp": b"BLP2xxxx"}, SHA[14])
gh_repo("ld", "vanilla", {"V.toc": "## Interface: 11200\nV.lua\n", "V.lua": "x"}, SHA[15])
gh_repo("ld", "models", {"Frames/Frames.toc": TOC, "Frames/Frames.lua": "x", "Frames/m.m2": b"MD20xxxx"}, SHA[16])
gh_repo("ld", "nested", {"Frames/Frames.toc": TOC, "Frames/Frames.lua": "x", "extra/libs.zip": b"PK\x03\x04"}, SHA[17])
gh_repo("ld", "dllinaddon", {"Frames/Frames.toc": TOC, "Frames/Frames.lua": "x", "Frames/hook.dll": b"MZ\x90\x00"}, SHA[18])
gh_repo("big", "pack", {"Frames/Frames.toc": TOC, "Frames/Frames.lua": "x", "Frames/Media/a.tga": os.urandom(1200 * 1024)}, SHA[19])
gh_repo("leak", "client-decompiled", {"Frames.toc": TOC, "Frames.lua": "x"}, SHA[20])


def pin_and_check(url):
    src = L.parse_source(url)
    pin = L.resolve(src, net)
    return pin, svc.fetch_and_analyse(src, pin)


DATA = os.environ["WU_PORTAL_DATA"]
store = PJ.Projects(data_dir=DATA, categories=["addons", "graphics", "tools", "realm-scripts", "sandbox"],
                    meta=type("M", (), {"fetch": staticmethod(lambda f, o, r: {"title": r, "licence": "MIT" if o != "nolic" else "",
                                                                                  "author": o, "description": ""})})())
NOW = [1_800_000_000.0]
CREATED = {"OLDIE": 0, "NEWBIE": NOW[0] - 3600}
svc = L.Listings(store, net=net, now=lambda: NOW[0], accounts=lambda a: CREATED.get(a, 0))
store.now = lambda: NOW[0]
store.per_day = 20   # the Workshop's links-a-day limit applies to listings too; checked last below

# ---------------------------------------------------------------- pinning
pin, r = pin_and_check("https://github.com/alice/Frames")
check("a repo without a release pins the default branch's commit archive", pin["from"] == "commit-archive" and pin["rev"] == SHA[1]
      and pin["url"] == f"https://github.com/alice/Frames/archive/{SHA[1]}.zip" and pin["follow"] == "branch:main", pin)
check("addon detected; the repo is the addon", r["ok"] and r["format"] == "addon" and r["install"]["folders"] == {"": "Frames"}, r)
inst = r["install"]
want = {"Frames/Frames.toc": hashlib.sha256(TOC.encode()).hexdigest(), "Frames/Frames.lua": hashlib.sha256(b"print('hi')\n").hexdigest()}
check("every installed file pinned by sha256; dev files and notes left out", inst["files"] == want, inst["files"])
check("the digest of the whole list (curate.py files_digest)", inst["files_digest"] == L.files_digest(want))
check("the archive's top folder is stripped", inst["strip_prefix"] == f"Frames-{SHA[1]}/")
check("a commit archive has no zip sha256 (GitHub's archives aren't byte-stable)", inst["sha256"] is None)
check("brand words and colour codes out of the shown name", r["name"] == "Frames", r["name"])
check("the plan says what one click does", any("Interface/AddOns" in x for x in r["plan"]) and any("fingerprint" in x for x in r["plan"]), r["plan"])
check("safe addon: no flags", r["flags"] == [], r["flags"])
pin, r = pin_and_check(f"https://github.com/alice/Frames/commit/{SHA[1]}")
check("a pinned commit never follows the branch", pin["follow"] == "" and pin["rev"] == SHA[1])

pin, r = pin_and_check("https://github.com/tex/hd-textures")
check("a latest release with one .MPQ file: the release file, by its own sha256", pin["from"] == "release-asset"
      and pin["rev"] == "v2" and r["install"]["sha256"] == hashlib.sha256(MPQ).hexdigest() and pin["follow"] == "latest", pin)
check("a bare .MPQ release file is a client patch", r["ok"] and r["format"] == "patch" and r["install"]["files"] == {"patch-H.MPQ": hashlib.sha256(MPQ).hexdigest()}, r)
check("a patch is flagged for a moderator; plan names the next free letter", r["flags"] and "next free patch letter" in " ".join(r["plan"]))
pin, r = pin_and_check("https://github.com/tex/hd-textures/releases/download/v2/patch-H.MPQ")
check("a release file link pins that file", pin["asset"] == "patch-H.MPQ" and r["format"] == "patch")
try:
    L.resolve(L.parse_source("https://github.com/tex/hd-textures/releases/download/v2/other.zip"), net)
    check("a missing release file is refused", False)
except ValueError as e:
    check("a missing release file is refused", "no file named" in str(e))
net.api_docs[("gitlab", "/projects/dave%2Fraid")] = {"default_branch": "main", "visibility": "public"}
net.api_docs[("gitlab", "/projects/dave%2Fraid/repository/commits/main")] = {"id": SHA[21]}
check("gitlab pins the commit archive", L.resolve(L.parse_source("https://gitlab.com/dave/raid"), net)["url"]
      == f"https://gitlab.com/dave/raid/-/archive/{SHA[21]}/raid-{SHA[21]}.zip")
net.api_docs[("codeberg", "/repos/erin/pack/commits/dev")] = {"sha": SHA[22]}
check("codeberg pins the commit archive", L.resolve(L.parse_source("https://codeberg.org/erin/pack/src/branch/dev"), net)["url"]
      == f"https://codeberg.org/erin/pack/archive/{SHA[22]}.zip")
try:
    L.resolve(L.parse_source("https://github.com/nobody/nothing"), net)
    check("a repo that isn't there", False)
except ValueError as e:
    check("a repo that isn't there", "public repo" in str(e), str(e))

# ---------------------------------------------------------------- formats and checks
def fmt(url):
    return pin_and_check(url)[1]


r = fmt("https://github.com/alice/pack")
check("two addon folders, one with its own .blp texture: flagged (curated), installed as author art",
      r["ok"] and set(r["install"]["folders"].values()) == {"Frames", "Bags"} and r["install"]["blp"] == "author-art"
      and any(".blp" in f for f in r["flags"]), r)
r = fmt("https://github.com/alice/stockart")
check("a texture named like stock interface art: flagged", r["ok"] and any("stock" in f or "interface art" in f for f in r["flags"]), r["flags"])
r = fmt("https://github.com/bob/dice")
check("a mod package: ui/ addon in the client, realm half explained", r["ok"] and r["format"] == "package" and r["client"]
      and "ui/DalaranDice/DalaranDice.toc" in r["install"]["files"] and "mod.json" in r["install"]["files"]
      and not any(p.startswith("realm/") for p in r["install"]["files"]) and any("realm half" in x for x in r["plan"]), r)
r = fmt("https://github.com/bob/baddice")
check("a package whose realm Lua uses io.*: refused", not r["ok"] and any("io.*" in p for p in r["problems"]), r["problems"])
r = fmt("https://github.com/tex/zipped")
check("a zip with a patch-*.MPQ at its top", r["ok"] and r["format"] == "patch" and list(r["install"]["files"]) == ["patch-T.MPQ"], r)
r = fmt("https://github.com/tex/fakempq")
check("a .MPQ that isn't one: refused", not r["ok"] and any("not an MPQ" in p for p in r["problems"]), r["problems"])
r = fmt("https://github.com/nat/fps")
check("native code: listed with its settings file, flagged, needs the player's OK", r["ok"] and r["format"] == "native"
      and set(r["install"]["files"]) == {"d3d9.dll", "d3d9.ini"} and r["install"]["native"] and any("native code" in f for f in r["flags"]), r)
r = fmt("https://github.com/rs/events")
check("a realm script: allowed APIs only, not for a game client", r["ok"] and r["format"] == "realm-script" and not r["client"]
      and r["install"] is None and r["flags"] == [], r)
r = fmt("https://github.com/rs/evil")
check("a realm script using os.execute: refused", not r["ok"] and any("os.*" in p for p in r["problems"]), r["problems"])
r = fmt("https://github.com/gm/coins")
check("a game mode", r["ok"] and r["format"] == "game-mode" and r["name"] == "Coin Dash" and not r["client"], r)
r = fmt("https://github.com/gm/bigmode")
check("a game mode over 14 KB: refused", not r["ok"] and any("14 KB" in p for p in r["problems"]), r["problems"])
r = fmt("https://github.com/ld/loose")
check("loose game files: refused with how to pack them", not r["ok"] and r["format"] == "loose-data" and "patch" in " ".join(r["problems"]))
r = fmt("https://github.com/ld/vanilla")
check("an addon for another client: refused with its Interface", not r["ok"] and "11200" in " ".join(r["problems"]))
r = fmt("https://github.com/ld/models")
check("a game model (.m2) anywhere: refused", not r["ok"] and any(".m2" in p for p in r["problems"]), r["problems"])
r = fmt("https://github.com/ld/nested")
check("a zip inside the download: refused", not r["ok"] and any(".zip" in p for p in r["problems"]), r["problems"])
r = fmt("https://github.com/ld/dllinaddon")
check("a program inside an addon folder: refused", not r["ok"] and any("programs" in p for p in r["problems"]), r["problems"])
os.environ["WU_LISTING_FETCH_MAX"] = "100"
svc2 = L.Listings(store, net=net, now=lambda: NOW[0])
res = svc2.fetch_and_analyse(L.parse_source("https://github.com/alice/Frames"), L.resolve(L.parse_source("https://github.com/alice/Frames"), net))
check("a download over the fetch cap is refused, not read", isinstance(res, str) and "bigger than" in res, res)
del os.environ["WU_LISTING_FETCH_MAX"]

# ---------------------------------------------------------------- submit, policy, limits
def list_mod(acct, url, desc="A tidy set of unit frames for raids.", cat="addons"):
    ok, r = svc.check(acct, url)
    if not ok:
        return False, r
    return svc.submit(acct, acct.title(), {"t": r["token"], "category": cat, "description": desc})


ok, rec = list_mod("OLDIE", "https://github.com/alice/Frames")
check("a safe addon goes live at once, community (not reviewed)", ok and not PJ.held(rec) and rec["listing"]["state"] == "community"
      and rec["submitter"] == "OLDIE", rec if not ok else rec.get("review"))
FR = rec["id"]
ok, rec = list_mod("OLDIE", "https://github.com/nat/fps", "Unlocks the frame rate for smooth play.", "tools")
check("native code waits for a moderator", ok and PJ.held(rec) and any("native" in w for w in rec["review"]["reasons"]))
NAT = rec["id"]
ok, rec = list_mod("NEWBIE", "https://github.com/alice/pack", "Frames and bags in one tidy pack.")
check("a new account's listing waits", ok and PJ.held(rec) and any("new account" in w for w in rec["review"]["reasons"]))
ok, msg = list_mod("NEWBIE", "https://github.com/rs/events", "Server events for a small realm.", "realm-scripts")
check("new accounts: one listing a day", not ok and "New accounts" in msg, msg)
ok, rec = list_mod("OLDIE", "https://github.com/leak/client-decompiled", "Unit frames from a big project.")
check("the hub's word rules still hold a listing", ok and PJ.held(rec) and any("decompiled" in w for w in rec["review"]["reasons"]))
ok, msg = svc.check("OLDIE", "https://github.com/bob/baddice")
check("a failed check gives the problems, and can't be submitted", ok and not msg["ok"])
ok2, msg2 = svc.submit("OLDIE", "Oldie", {"t": msg["token"], "category": "addons", "description": "Dice rolls for groups."})
check("... submitting it is refused", not ok2 and "didn't pass" in msg2, msg2)
ok, msg = svc.submit("OLDIE", "Oldie", {"t": "0" * 32, "category": "addons", "description": "Dice rolls for groups."})
check("an unknown or other player's check token is refused", not ok and "out of date" in msg)
ok, r = svc.check("RATER", "https://github.com/rs/events")
ok3, msg3 = svc.submit("OTHER", "Other", {"t": r["token"], "category": "realm-scripts", "description": "Server events for a small realm."})
check("a check token is bound to the account that made it", not ok3)
L.set_policy({"auto_live": ["package"]})
ok, rec = list_mod("OLDIE", "https://github.com/rs/events", "Server events for a small realm.", "realm-scripts")
check("the owner's policy: realm scripts no longer go live at once", ok and PJ.held(rec), rec)
L.set_policy({})
check("the policy file round-trips and is cleaned", L.policy() == L.DEFAULT_POLICY and L.clean_policy({"report_hide": 999, "auto_live": ["x", "addon"]})
      == dict(L.DEFAULT_POLICY, auto_live=["addon"]))
svc.check_log.clear()
for i in range(svc.per_hour):
    svc.check("SPAMMER", "https://github.com/alice/Frames")
ok, msg = svc.check("SPAMMER", "https://github.com/alice/Frames")
check("checks are rate limited per account (they fetch a download)", not ok and "an hour" in msg, msg)
NOW[0] += 3601
check("... and open again an hour later", svc.check("SPAMMER", "https://github.com/alice/Frames")[0])
ok, msg = list_mod("SOMEONE", "https://github.com/alice/Frames")
check("someone else can't list a repo that's already listed", not ok and "already listed" in msg, msg)
ok, rec = list_mod("OLDIE", "https://github.com/alice/Frames", "Unit frames, now with raid frames.")
check("its author can re-list it (a new check replaces the listing, same id)", ok and rec["id"] == FR and rec["description"].startswith("Unit frames, now"))

# ---------------------------------------------------------------- reports, HQ, author edit/remove
ok, msg = store.report(FR, {"id": "OLDIE"}, "spam", hide_after=3)
check("authors can't report their own listing", not ok and "your own" in msg)
for i, who in enumerate(("R1", "R2", "R2")):
    store.report(FR, {"id": who}, "malware" if i else "broken", "it breaks my bags", hide_after=3)
rec = next(r for r in store.load() if r["id"] == FR)
check("one report per account, reasons kept without account ids", len(rec["reports"]) == 2 and [x["reason"] for x in rec["report_log"]] == ["broken", "malware"]
      and all(set(x) == {"reason", "note", "at"} for x in rec["report_log"]))
check("... still listed under the threshold", not PJ.held(rec))
store.report(FR, {"id": "R3"}, "bogus-reason", hide_after=3)
rec = next(r for r in store.load() if r["id"] == FR)
check("the Nth independent report hides it until a moderator looks", PJ.held(rec) and rec["review"].get("by_reports")
      and rec["report_log"][-1]["reason"] == "other")


class FakeA:
    class Refused(Exception):
        pass
    ACTIONS = {}

    @staticmethod
    def can(user, need):
        return user.get("role") == "owner" or need != "owner"

    @staticmethod
    def act(user, action, target, label, back, fields="", confirm=None, danger=False, block=False):
        return f"<form data-action='{action}'>{fields}</form>"


L._get = lambda name: {"workshop": type("W", (), {"projects": store})()}.get(name)
items = L.hq_items(FakeA, lambda *a, **k: {"kind": a[0], "id": a[1], "label": a[2], "title": a[3], "sub": k.get("sub", ""),
                                            "acts": a[7], "group": k.get("group", 1)})
labels = {x["id"]: x for x in items}
check("Needs you: hidden-by-reports first, with the reasons", labels.get("listing:" + FR, {}).get("label") == "Listing: hidden by reports"
      and "harmful" in labels["listing:" + FR]["sub"].lower() and labels["listing:" + FR]["group"] == 0, labels.get("listing:" + FR))
check("Needs you: native code and new accounts waiting", "listing:" + NAT in labels and labels["listing:" + NAT]["label"] == "Listing: waiting")
acts = L.hq_actions(FakeA)
check("HQ actions: approve, take down (typed confirm), updates, policy (owner)", set(acts) == {"listing.approve", "listing.reject",
      "listing.update.approve", "listing.update.reject", "listing.policy"} and acts["listing.reject"][2] is True and acts["listing.policy"][1] == "owner")
acts["listing.approve"][0]({"account": "MOD1"}, FR, {})
rec = next(r for r in store.load() if r["id"] == FR)
check("approve: live again, reviewed, reports cleared", not PJ.held(rec) and rec["listing"]["state"] == "reviewed" and rec["reports"] == []
      and rec.get("hide_votes") == [])
for who in ("N1", "N2", "N3", "N4"):
    store.report(FR, {"id": who}, "spam", hide_after=3, trusted=False)
rec = next(r for r in store.load() if r["id"] == FR)
check("reports from new accounts are kept but never hide a listing by themselves", not PJ.held(rec) and len(rec["reports"]) == 4)
CREATED.update(Q1=NOW[0] - 3600, Q2=NOW[0] - 3600, Q3=NOW[0] - 3600)
L._get = lambda name: {"workshop": type("W", (), {"projects": store})()}.get(name)
_setup_was = L.setup
L.setup = lambda: svc
try:
    for who in ("Q1", "Q2", "Q3"):
        L.do("report", who, who, {"id": "link:" + FR, "reason": "spam"})
finally:
    L.setup = _setup_was
rec = next(r for r in store.load() if r["id"] == FR)
check("... also through the API: three fresh accounts can't take a mod down", not PJ.held(rec) and len(rec["reports"]) == 7, rec.get("review"))
ok, msg = store.edit(FR, {"id": "OLDIE"}, {"description": "Unit frames with a free aimbot built in."})
check("an edit can't slip the hub's held words past a moderator", not ok and "moderator" in msg, msg)
ok, msg = store.edit(FR, {"id": "OLDIE"}, {"description": "Unit frames, raid frames and a cast bar."})
check("... a plain edit saves", ok, msg)
try:
    acts["listing.reject"][0]({"account": "MOD1"}, NAT, {})
    check("take down needs a reason", False)
except FakeA.Refused:
    check("take down needs a reason", True)
acts["listing.reject"][0]({"account": "MOD1"}, NAT, {"reason": "unsigned injector"})
rec = next(r for r in store.load() if r["id"] == NAT)
check("take down hides it with the reason", rec["hidden"] and rec["review"]["reasons"] == ["unsigned injector"])
acts["listing.policy"][0]({"account": "OWNER", "role": "owner"}, "policy", {"auto_addon": "1", "auto_game-mode": "on", "report_hide": "5"})
check("HQ's policy form saves the owner's choice", L.policy()["auto_live"] == ["addon", "game-mode"] and L.policy()["report_hide"] == 5)
check("the policy panel is for the owner only", L.hq_policy_html({"role": "mod"}, FakeA) == "" and "listing.policy" in L.hq_policy_html({"role": "owner"}, FakeA))
L.set_policy({})
ok, msg = store.edit(FR, {"id": "R1"}, {"description": "Mine now, all mine, really."})
check("only the author edits", not ok)
ok, msg = store.edit(FR, {"id": "OLDIE"}, {"description": "Unit frames with clean raid frames."})
check("the author edits the description", ok and next(r for r in store.load() if r["id"] == FR)["description"] == "Unit frames with clean raid frames.")
ok, msg = store.edit(FR, {"id": "OLDIE"}, {"description": "Best World of Warcraft frames ever."})
check("... with the same rules as at submit", not ok)

# ---------------------------------------------------------------- all.json, hub rankings, Rising
rec = next(r for r in store.load() if r["id"] == FR)
f = L.all_json_fields(rec["listing"])
check("all.json: a client format is source listed with its install block", f["source"] == "listed" and f["install"]["files"]
      and f["install"]["kind"] == "addon" and f["state"] == "reviewed" and f["format"] == "addon")
rs = next(r for r in store.load() if r.get("listing", {}).get("format") == "realm-script")
check("all.json: a realm script stays a link (nothing for a client)", "source" not in L.all_json_fields(rs["listing"]))
PUB = os.path.join(T, "mods")
os.makedirs(PUB)
shutil.copy(os.path.join(ROOT, "mods", "catalog.json"), PUB)
shutil.copy(os.path.join(ROOT, "mods", "categories.json"), PUB)
w = WS.Workshop(PUB, projects=store)
doc = json.loads(w.all_json())
mine = next((p for p in doc["projects"] if p["id"] == "link:" + FR), None)
check("the Workshop's all.json carries the listed mod for the launcher", mine is not None and mine["source"] == "listed"
      and mine["install"]["files_digest"] == rec["listing"]["install"]["files_digest"], mine)
check("held listings are not in all.json", not any(p["id"] == "link:" + NAT for p in doc["projects"]))
item = {"id": "link:" + FR, "source": "workshop", "ws": mine, "packaged": False}
check("hub rankings: a listed mod is installable (leyline://install link)", RANK.installable(item)
      and not RANK.installable({"source": "workshop", "ws": {"source": "community"}}))
today = int(NOW[0] // 86400)
m = RANK.compute([item], [("link:" + FR, today - 10, 2), ("link:" + FR, today - 2, 9), ("link:" + FR, today - 1, 8)],
                 [("link:" + FR, 5, NOW[0] - 3600)], {}, today)
check("Rising: installs this week far above last week", m["link:" + FR]["rising"] and m["link:" + FR]["rising"] > 1
      and m["link:" + FR]["rating"]["n"] == 1, m)

# ---------------------------------------------------------------- updates when the author pushes
gh_repo("alice", "Frames", {"Frames.toc": TOC, "Frames.lua": "print('v2')\n"}, SHA[23])
check("a branch-following listing picks up a safe push and applies it", svc.refresh_one(next(r for r in store.load() if r["id"] == FR)) == "updated")
rec = next(r for r in store.load() if r["id"] == FR)
check("... the new commit, new hashes, kept state", rec["listing"]["install"]["rev"] == SHA[23] and rec["listing"]["state"] == "reviewed"
      and rec["listing"]["install"]["files"]["Frames/Frames.lua"] == hashlib.sha256(b"print('v2')\n").hexdigest()
      and rec["listing"]["last_diff"]["changed"] == 1)
check("... and nothing new means same", svc.refresh_one(rec) == "same")
gh_repo("alice", "Frames", {"Frames.toc": TOC, "Frames.lua": "x", "Media/x.blp": b"BLP2xxxx"}, SHA[24])
check("a push that raises a flag waits as a pending update", svc.refresh_one(rec) == "pending")
rec = next(r for r in store.load() if r["id"] == FR)
upd = rec["listing"]["pending_update"]
check("... the old install stays live; the update has its diff and why", rec["listing"]["install"]["rev"] == SHA[23] and upd["rev"] == SHA[24]
      and upd["diff"]["added"] == 1 and upd["why"])
items = L.hq_items(FakeA, lambda *a, **k: {"id": a[1], "label": a[2]})
check("Needs you: the update waits", any(x["id"] == "listing-up:" + FR for x in items))
acts["listing.update.approve"][0]({"account": "MOD1"}, FR, {})
rec = next(r for r in store.load() if r["id"] == FR)
check("approving the update makes it the install", rec["listing"]["install"]["rev"] == SHA[24] and "pending_update" not in rec["listing"])
gh_repo("alice", "Frames", {"Frames.toc": TOC, "Frames.lua": "x", "m.m2": b"MD20xxxx"}, SHA[25])
check("a push that fails the checks waits too (the old one stays)", svc.refresh_one(rec) == "pending"
      and next(r for r in store.load() if r["id"] == FR)["listing"]["install"]["rev"] == SHA[24])
counts = svc.refresh()
check("refresh() walks the following listings", sum(counts.values()) >= 1, counts)
check("pinned listings never move on their own", svc.refresh_one({"listing": {"follow": ""}}) == "fixed")
store.per_day = 1
ok, msg = list_mod("R1", "https://github.com/gm/coins", "Collect coins against friends.", "sandbox")
ok2, msg2 = list_mod("R1", "https://github.com/tex/zipped", "Sharper ground textures.", "graphics")
check("listings count toward the Workshop's links a day", ok and not ok2 and "a day" in msg2, msg2)
ok, msg = store.remove(FR, {"id": "R1"})
check("only the author removes", not ok)
ok, msg = store.remove(FR, {"id": "OLDIE"})
check("the author removes their listing for good", ok and not any(r["id"] == FR for r in store.load()))
check("nothing was ever written but the store and the policy", sorted(os.listdir(DATA)) == sorted(
      [x for x in os.listdir(DATA) if x in ("projects.jsonl", ".projects.lock", "listings-policy.json", "hub")]))
check("downloads were only ever the author's own addresses", all(u.startswith(("https://github.com/", "https://gitlab.com/", "https://codeberg.org/"))
      for u in net.fetched))

# ---------------------------------------------------------------- the routes (a fake handler and sign-in)
class FakeSocialStore:
    def token_acct(self, tok):
        return {"tok-oldie": "OLDIE"}.get(tok)

    def profile(self, acct):
        return {"name": acct.title()}


class FakeSocial:
    store = FakeSocialStore()

    @staticmethod
    def cookie_user(h):
        return ("OLDIE", "Oldie") if "wr_session=s1" in h.headers.get("Cookie", "") else None


class H:
    def __init__(self, method, path, body=b"", headers=None):
        self.headers = dict(headers or {})
        self.headers.setdefault("Content-Length", str(len(body)))
        self.rfile = io.BytesIO(body)
        self.sent = None

    def ip(self):
        return "127.0.0.1"

    def send(self, status, body, ctype="text/html; charset=utf-8", headers=None, **kw):
        self.sent = (status, body.decode("utf-8") if isinstance(body, bytes) else body, ctype)


L.service = svc
L._get = lambda name: {"workshop": w, "social": FakeSocial}.get(name)
svc.check_log.clear()
store.per_day = 20


def api(action, doc, headers):
    h = H("POST", L.API + "/" + action, json.dumps(doc).encode(), dict({"Content-Type": "application/json"}, **headers))
    L.post(h, L.API + "/" + action)
    return h.sent[0], json.loads(h.sent[1])


st, d = api("check", {"url": "https://github.com/gm/coins"}, {})
check("API: signed out is refused", st == 401 and d["error"] == "signed_out")
st, d = api("check", {"url": "https://github.com/rs/events"}, {"Authorization": "Bearer tok-oldie"})
check("API: the launcher's token checks a link and sees what one click does", st == 200 and d["check"]["ok"]
      and d["check"]["format"] == "realm-script" and d["check"]["plan"] and "install" not in d["check"], d)
st, d2 = api("submit", {"t": d["check"]["token"], "category": "realm-scripts", "description": "Server events for small realms."},
             {"Authorization": "Bearer tok-oldie"})
check("API: ... and lists it", st == 200 and d2["state"] == "live" and d2["id"].startswith("link:"), d2)
st, d3 = api("check", {"url": "https://github.com/rs/events"}, {"Cookie": "wr_session=s1"})
check("API: the site's cookie needs the CSRF header", st == 403)
h = H("GET", L.API + "/mine", b"", {"Cookie": "wr_session=s1"})
L.get(h, L.API + "/mine", {})
mine_doc = json.loads(h.sent[1])
check("API: mine lists the caller's listings with their state, and a CSRF token", h.sent[0] == 200 and mine_doc["csrf"]
      and any(m["pid"] == d2["pid"] and m["state"].startswith("live") for m in mine_doc["listings"]))
st, d4 = api("report", {"id": d2["id"], "reason": "spam"}, {"Cookie": "wr_session=s1", "X-Leyline-CSRF": mine_doc["csrf"]})
check("API: report (own listing refused)", st == 400 and "your own" in d4["error"])
h = H("GET", L.PAGE)
L.get(h, L.PAGE, {})
check("page: signed out asks to sign in", h.sent[0] == 200 and "Sign in" in h.sent[1] and "List your mod" in h.sent[1])
h = H("GET", L.PAGE, b"", {"Cookie": "wr_session=s1"})
L.get(h, L.PAGE, {})
check("page: signed in shows the link form and your listings", 'name="url"' in h.sent[1] and "Your listings" in h.sent[1])
csrf = L._csrf("s1")
body = "step=check&csrf=" + csrf + "&url=" + "https%3A%2F%2Fgithub.com%2Fgm%2Fcoins"
h = H("POST", L.PAGE, body.encode(), {"Cookie": "wr_session=s1", "Content-Type": "application/x-www-form-urlencoded"})
L.post(h, L.PAGE)
check("page: check shows the plan and the List it form", h.sent[0] == 200 and "What one click does" in h.sent[1] and "List it" in h.sent[1]
      and "Coin Dash" in h.sent[1], h.sent[1][-1500:])
h = H("POST", L.PAGE, body.replace(csrf, "bad").encode(), {"Cookie": "wr_session=s1", "Content-Type": "application/x-www-form-urlencoded"})
L.post(h, L.PAGE)
check("page: a form without its token is refused", h.sent[0] == 403)
h = H("POST", L.PAGE, ("step=check&csrf=" + csrf + "&url=https%3A%2F%2Fgithub.com%2Fld%2Fmodels").encode(),
      {"Cookie": "wr_session=s1", "Content-Type": "application/x-www-form-urlencoded"})
L.post(h, L.PAGE)
check("page: a refused check says why and offers Check again", "lm-bad" in h.sent[1] and ".m2" in h.sent[1] and "Check again" in h.sent[1])

# ---- Addon or mod (owner 31 Oct 19:22; docs/WORDS.md): the form says which it is, Addon first, then Mod's categories
_HUB = load("wu_hub_for_listings", "hub.py")
_cats = json.load(open(os.path.join(STREAM, "..", "mods", "categories.json")))["categories"]


class _FakeWs:
    @staticmethod
    def categories():
        return (_cats, {})


_old_get = L._get
_ok_check = {"ok": True, "name": "x", "plan": ["Adds an addon folder"], "token": "t" * 32}
for _hubv in (_HUB, None):
    L._get = lambda n, _h=_hubv: _FakeWs if n == "workshop" else _h if n == "hub_mod" else None
    _ph = L._preview_html(dict(_ok_check, format="addon"), "", {})
    _pm = L._preview_html(dict(_ok_check, format="realm-script"), "", {})
    _pk = L._preview_html(dict(_ok_check, format="patch"), "", {"kind": "addon"})
    check("List your mod: Addon or mod as two groups (Addon first), themes left out, the default from what it is"
          + (" (the hub's options)" if _hubv else " (without the hub)"),
          '<optgroup label="Addon: an in-game interface helper"><option value="addons" selected>' in _ph
          and '<optgroup label="Mod: changes the game itself">' in _ph and _ph.index("Addon: an") < _ph.index("Mod: changes")
          and '<option value="realm-scripts" selected>' in _pm and '<option value="addons">' in _pm
          and 'value="themes"' not in _ph and '<option value="addons" selected>' in _pk and ">Addon or mod <" in _ph, _ph[-900:])
L._get = _old_get

# ---------------------------------------------------------------- checks at once, and the disk they download to
svc.check_log.clear(); svc.day_log.clear()
held = [L.CHECKS_AT_ONCE.acquire(blocking=False) for _ in range(2)]
ok, msg = svc.check("BUSY1", "https://github.com/alice/Frames")
for _ in held:
    L.CHECKS_AT_ONCE.release()
check("two checks download at once; a third is asked to wait a minute", all(held) and not ok and "a minute" in msg, msg)
_du = L.shutil.disk_usage
L.shutil.disk_usage = lambda p: type("U", (), {"free": 100 * 1048576})()
try:
    ok, msg = svc.check("BUSY2", "https://github.com/alice/Frames")
finally:
    L.shutil.disk_usage = _du
check("a check never downloads into the disk's last free space", not ok and "paused" in msg, msg)
check("... and checks work again after both", svc.check("BUSY3", "https://github.com/alice/Frames")[0])

shutil.rmtree(T, ignore_errors=True)
print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
