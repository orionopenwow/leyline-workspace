"""tools/addon-curate/curate.py, with fixture archives (no network): the shared rules cases the launcher also runs,
folder mapping (a repo of addon folders, a repo that is the addon), dev files skipped, game data and native code
refused, .blp recorded as the author's art and never mirrored, licences, requires and conflicts, the review gate,
pending updates and what is published.

    python3 -I tools/addon-curate/test_curate.py
"""
import importlib.util, io, json, os, tempfile, unittest, zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("curate", os.path.join(HERE, "curate.py"))
cu = importlib.util.module_from_spec(spec); spec.loader.exec_module(cu)
with open(os.path.join(HERE, "..", "..", "launcher", "src-tauri", "src", "upstream-cases.json"), encoding="utf-8") as _f:
    CASES = json.load(_f)
C1, C2 = "1" * 40, "2" * 40
MIT = "MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy"
GPL2 = ("GNU GENERAL PUBLIC LICENSE\nVersion 2, June 1991\n... If your program is a subroutine library, you may consider "
        "it more useful to permit linking proprietary applications with the library. If this is what you want to do, "
        "use the GNU Lesser General Public License instead of this License.")


def zip_bytes(files: dict) -> bytes:
    b = io.BytesIO()
    with zipfile.ZipFile(b, "w") as z:
        for n, data in files.items():
            z.writestr(n, data)
    return b.getvalue()


def fake_fetch(archives: dict):
    """resolve() stand-in: seed id -> (rev, {archive path: bytes})."""
    def fetch(seed, work):
        rev, files = archives[seed["id"]]
        p = os.path.join(work, "a.zip")
        with open(p, "wb") as f:
            f.write(zip_bytes(files))
        o, r = seed["repo"].split("/")
        return {"from": "commit-archive", "url": f"https://github.com/{o}/{r}/archive/{rev}.zip", "rev": rev, "zip": p, "notes": []}
    return fetch


TOC = "## Interface: 30300\n## Title: |cff00ff00Frames|r (WotLK)\n## Version: r736\n## Dependencies: LibCore\nFrames.lua\n"


class Rules(unittest.TestCase):
    def test_the_shared_cases(self):
        for c in CASES["classify"]:
            got = cu.classify(c["path"], c["blp_ok"], bytes.fromhex(c.get("head", ""))).split(":")[0]
            self.assertEqual(got, c["expect"], c)
        for c in CASES["names"]:
            self.assertEqual(cu.name_ok(c["name"]), c["ok"], c)
        for c in CASES["interface"]:
            self.assertEqual(cu.interface_ok(c["value"]), c["ok"], c)
        for c in CASES["clean_name"]:
            self.assertEqual(cu.clean_name(c["raw"]), c["want"], c)

    def test_licences(self):
        self.assertEqual(cu.licence_of(MIT), "MIT")
        self.assertEqual(cu.licence_of(GPL2), "GPL-2.0", "the GPL names the LGPL further down")
        self.assertEqual(cu.licence_of("GNU LESSER GENERAL PUBLIC LICENSE Version 2.1, February 1999"), "LGPL-2.1")
        self.assertEqual(cu.licence_of("Copyright (c) 2020 me. All rights reserved."), "All rights reserved")
        self.assertTrue(cu.licence_open("MIT") and not cu.licence_open("All rights reserved") and not cu.licence_open(""))

    def test_safe_names(self):
        for bad in ["a/../b", "/abs", "C:/x", "a/con.lua", "a/b ", "a/b.", "a/x:y"]:
            self.assertFalse(cu.safe_rel(bad), bad)
        self.assertTrue(cu.safe_rel("!KRT/!KRT.toc") and cu.safe_rel("xCT+/xCT+.toc"))


class Analyze(unittest.TestCase):
    def run_one(self, files, seed=None):
        seed = seed or {"id": "frames", "repo": "alice/Frames"}
        names = [(n, b.encode() if isinstance(b, str) else b) for n, b in files.items()]
        return cu.analyze(names, seed)

    def test_folders_and_dev_files(self):
        a = self.run_one({
            f"Frames-{C1}/Frames/Frames.toc": TOC, f"Frames-{C1}/Frames/Frames.lua": "x",
            f"Frames-{C1}/Frames/Media/bar.blp": b"BLP2\0\0", f"Frames-{C1}/Frames/.pkgmeta": "p",
            f"Frames-{C1}/Frames_Options/Frames_Options.toc": "## Interface: 30300, 11508\n", f"Frames-{C1}/README.md": "r",
            f"Frames-{C1}/.github/workflows/x.yml": "y", f"Frames-{C1}/LICENSE": MIT, f"Frames-{C1}/Old/Old.toc": "## Interface: 11200\n",
            f"Frames-{C1}/Frames/Libs/Lib/Lib.toc": "## Interface: 30300\n",
        })
        self.assertTrue(a["ok"], a.get("problem"))
        self.assertEqual(a["strip_prefix"], f"Frames-{C1}/")
        self.assertEqual(a["folders"], {"Frames": "Frames", "Frames_Options": "Frames_Options"}, "a library's .toc is not an addon")
        self.assertEqual(sorted(a["files"]), ["Frames/Frames.lua", "Frames/Frames.toc", "Frames/Libs/Lib/Lib.toc",
                                              "Frames/Media/bar.blp", "Frames_Options/Frames_Options.toc"])
        self.assertEqual(a["blp_files"], ["Frames/Media/bar.blp"])
        self.assertEqual(a["licence"], "MIT")
        self.assertEqual(a["addons"]["Frames"]["title"], "Frames")
        self.assertEqual(a["needs_folders"], ["LibCore"])

    def test_root_is_the_addon(self):
        a = self.run_one({f"Questie-{C1}/Questie.toc": "## Interface: 00000\n", f"Questie-{C1}/Questie-335.toc": "## Interface: 30300\nCore.lua\n",
                          f"Questie-{C1}/Core.lua": "x", f"Questie-{C1}/Icons/LICENSE.md": MIT, f"Questie-{C1}/tools/x.py": "p"})
        self.assertTrue(a["ok"], a.get("problem"))
        self.assertEqual(a["folders"], {"": "Questie-335"})
        self.assertEqual(sorted(a["files"]), ["Questie-335/Core.lua", "Questie-335/Questie-335.toc"])
        self.assertEqual(a["licence"], "", "an icon set's licence is not the addon's")

    def test_refusals(self):
        for bad in ["Frames/Data/patch-4.MPQ", "Frames/x.m2", "Frames/d3d9.dll", "tools/inject.exe", "Frames/x.dbc", "Release.zip"]:
            a = self.run_one({"Frames/Frames.toc": TOC, bad: b"data"})
            self.assertFalse(a["ok"], bad)
        a = self.run_one({"Frames/Frames.toc": TOC, "Frames/renamed.lua": b"MZ\x90\x00"})
        self.assertFalse(a["ok"])
        self.assertFalse(self.run_one({"Frames/Frames.toc": "## Interface: 11200\n"})["ok"])
        self.assertFalse(self.run_one({"Blizzard_X/Blizzard_X.toc": "## Interface: 30300\n"})["ok"])

    def test_packager_libraries_missing(self):
        a = self.run_one({"Frames/Frames.toc": "## Interface: 30300\nLibs/LibStub/LibStub.lua\nFrames.lua\n", "Frames/Frames.lua": "x",
                          "Frames/.pkgmeta": "externals:\n  Libs/LibStub: x"})
        self.assertFalse(a["ok"])
        self.assertIn("packaged release", a["problem"])

    def test_stock_art_is_flagged(self):
        a = self.run_one({"Frames/Frames.toc": TOC, "Frames/Frames.lua": "x", "Frames/Media/UI-Character-Tab.blp": b"BLP2"})
        self.assertTrue(any("stock interface art" in f for f in a["flags"]))


class Pipeline(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.TemporaryDirectory()
        self.out = os.path.join(self.d.name, "upstream.json")
        self.pub = os.path.join(self.d.name, "pub.json")

    def tearDown(self):
        self.d.cleanup()

    SEEDS = [
        {"repo": "alice/Frames", "id": "frames", "description": "Unit frames.", "tags": ["unit-frames"]},
        {"repo": "bob/LibCore", "id": "libcore", "description": "A library."},
        {"repo": "carol/Frames", "id": "frames-fork", "description": "A fork."},
        {"repo": "dan/Art", "id": "art", "description": "Art."},
        {"repo": "eve/Broken", "id": "broken", "description": "x"},
        {"repo": "mallory/Injector", "exclude": "native code"},
    ]

    def archives(self, rev=C1, lua="x"):
        return {
            "frames": (rev, {f"Frames-{rev}/Frames/Frames.toc": TOC, f"Frames-{rev}/Frames/Frames.lua": lua, f"Frames-{rev}/LICENSE": MIT}),
            "libcore": (C1, {f"LibCore-{C1}/LibCore/LibCore.toc": "## Interface: 30300\n"}),
            "frames-fork": (C1, {f"Frames-{C1}/Frames/Frames.toc": TOC, f"Frames-{C1}/Frames/Frames.lua": "y"}),
            "art": (C1, {f"Art-{C1}/Art.toc": "## Interface: 30300\n", f"Art-{C1}/a.blp": b"BLP2", f"Art-{C1}/LICENSE": MIT}),
            "broken": (C1, {f"Broken-{C1}/Broken/Broken.toc": "## Interface: 30300\n", f"Broken-{C1}/Broken/x.m2": b"MD20"}),
        }

    def test_curate_review_publish_and_update(self):
        doc = cu.load_doc(self.out)
        c = cu.curate(self.SEEDS, doc, self.d.name, fetch=fake_fetch(self.archives()), log=lambda *_: None)
        self.assertEqual(c, {"resolved": 4, "failed": 1, "excluded": 1})
        by = {e["id"]: e for e in doc["entries"]}
        self.assertEqual(set(by), {"up:frames", "up:libcore", "up:frames-fork", "up:art"})
        self.assertIn(".m2", doc["failed"][0]["reason"])
        f = by["up:frames"]
        self.assertEqual(f["requires"], ["up:libcore"], "from the TOC's Dependencies")
        self.assertEqual(f["conflicts"], ["up:frames-fork"], "same folder")
        self.assertTrue(f["mirror_allowed"], "MIT, no .blp")
        self.assertFalse(by["up:art"]["mirror_allowed"], ".blp art is never mirrored")
        self.assertEqual(by["up:art"]["blp"], "author-art")
        self.assertFalse(by["up:frames-fork"]["mirror_allowed"], "no licence: install from the author only")
        self.assertEqual(f["review"]["by"], "", "pending")
        cu.save_doc(doc, self.out)
        self.assertEqual(cu.publish(doc, self.pub), 0, "nothing reviewed, nothing published")
        self.assertEqual(len(cu.pending_entries(self.out)), 4)

        with self.assertRaises(ValueError):
            cu.approve(doc, "up:frames", "  ")
        cu.approve_entry(self.out, "up:frames", "owner", "looked at it", works=True, publish_path=self.pub)
        with open(self.pub) as fh:
            pub = json.load(fh)
        self.assertEqual([p["id"] for p in pub["projects"]], ["up:frames"])
        p = pub["projects"][0]
        self.assertEqual(p["source"], "upstream")
        self.assertNotIn("files", p["install"])
        self.assertEqual(p["install"]["files_digest"], cu.files_digest(f["install"]["files"]))
        self.assertIn("Installs from the author's GitHub", p["credits"])
        self.assertEqual(p["compat"]["status"], "works")

        # Upstream moves on: the approved install stays published; the update waits for review.
        doc = cu.load_doc(self.out)
        cu.curate(self.SEEDS[:1], doc, self.d.name, fetch=fake_fetch(self.archives(rev=C2, lua="new")), log=lambda *_: None)
        e = next(x for x in doc["entries"] if x["id"] == "up:frames")
        self.assertEqual(e["install"]["rev"], C1)
        self.assertEqual(e["pending_update"]["install"]["rev"], C2)
        self.assertEqual(e["pending_update"]["diff"]["changed"], 1)
        cu.approve(doc, "up:frames", "owner")
        self.assertEqual(e["install"]["rev"], C2)
        self.assertNotIn("pending_update", e)
        cu.unapprove(doc, "up:frames")
        self.assertEqual(cu.publish(doc, self.pub), 0)

    def test_brand_words_flagged_and_cleaned(self):
        seeds = [{"repo": "alice/Frames", "id": "frames", "display_name": "WoW Frames", "description": "For WoW."}]
        doc = cu.load_doc(self.out)
        cu.curate(seeds, doc, self.d.name, fetch=fake_fetch(self.archives()), log=lambda *_: None)
        e = doc["entries"][0]
        self.assertEqual(e["display_name"], "Frames")
        self.assertIn("brand word in our text", e["flags"])

    def test_seeds_file_is_sound(self):
        with open(cu.SEEDS_PATH, encoding="utf-8") as fh:
            seeds = json.load(fh)["seeds"]
        ids = [s["id"] for s in seeds if not s.get("exclude")]
        self.assertEqual(len(ids), len(set(ids)))
        for s in seeds:
            if s.get("exclude"):
                continue
            self.assertTrue(cu.name_ok(s["id"].replace("-", "_")) and s["description"], s)
            self.assertFalse(cu.banned(s["description"]) or cu.banned(s.get("display_name", "")), s)
        out = {s["repo"] for s in seeds if s.get("exclude")}
        for must in ["ErebusAres/ZygorGuidesRemaster-3.3.5a_WOTLK", "FrostAtom/awesome_wotlk", "suprepupre/LuaBoost",
                     "Kirazul/Transmorpher", "NeticSoul/DragonUI", "a3st/RetailUI", "NoM0Re/WoW-3.3.5a-Addons"]:
            self.assertIn(must, out)


def repo_doc(full, stars=10, desc="", topics=(), fork=False):
    return {"full_name": full, "stargazers_count": stars, "description": desc, "topics": list(topics), "fork": fork,
            "license": {"spdx_id": "MIT"}}


class Discover(unittest.TestCase):
    FOUND = {
        "alice/NicePlates": {"stars": 40, "description": "Clean nameplates for WotLK 3.3.5a", "topics": []},
        "bob/QuestLog": {"stars": 12, "description": "A bigger quest log with level colours", "topics": []},
        "carl/WoW-3.3.5a-Addons": {"stars": 90, "description": "All my addons", "topics": []},
        "dora/OneButton": {"stars": 30, "description": "One-button rotation helper", "topics": []},
        "chumpton/Thing": {"stars": 30, "description": "x", "topics": []},
        "mallory/Thing": {"stars": 30, "description": "x", "topics": []},
        "erin/Fork": {"stars": 30, "description": "x", "topics": [], "fork": True},
        "fred/Broken": {"stars": 5, "description": "Broken addon", "topics": []},
        "gail/Many": {"stars": 5, "description": "Raid addon", "topics": []},
        "hal/Zero": {"stars": 0, "description": "Bags", "topics": []},
        "ElvUI-WotLK/ElvUI": {"stars": 500, "description": "known already", "topics": []},
    }

    def check(self, seed):
        n = seed["repo"].split("/")[1]
        if n == "Broken":
            return {"ok": False, "problem": "x.m2: .m2 files are never allowed"}
        if n == "Many":
            return {"ok": True, "addons": {f"A{i}": {"title": f"A{i}"} for i in range(12)}, "licence": "MIT", "blp_files": []}
        title = {"NicePlates": "|cff00ff00NicePlates|r (WotLK)", "QuestLog": "QuestLog"}[n]
        return {"ok": True, "addons": {n: {"title": title}}, "licence": "MIT" if n == "NicePlates" else "", "blp_files": []}

    def test_discover_adds_pending_seeds_in_our_words(self):
        doc = {"seeds": [{"repo": "ElvUI-WotLK/ElvUI", "id": "elvui", "description": "x"}]}
        block = {"owners": [{"owner": "Mallory", "reason": "a lure account"}]}
        out = cu.discover(doc, None, self.check, "2026-11-03", found=dict(self.FOUND), block=block, log=lambda *_: None)
        added = {s["repo"]: s for s in out["added"]}
        self.assertEqual(sorted(added), ["alice/NicePlates", "bob/QuestLog"])
        np_ = added["alice/NicePlates"]
        self.assertEqual(np_["display_name"], "NicePlates")
        self.assertEqual(np_["id"], "niceplates")
        self.assertEqual(cu.discover_slug("ElvUI Extras", set()), "elvui-extras")
        self.assertEqual(cu.discover_slug("Skada", {"skada"}), "skada-2")
        self.assertEqual(cu.discover_name("LootCollector beta 0.8.8r"), "LootCollector")
        self.assertEqual(cu.discover_name("|cff00ff00Recount|r (3.3.5a) v1.2"), "Recount")
        self.assertIn("nameplates", np_["tags"])
        self.assertFalse(cu.banned(np_["description"]) or cu.VERSION_IN_PROSE.search(np_["description"]))
        self.assertEqual(np_["description"], "Nameplates: NicePlates.")           # their words had a brand word
        ql = added["bob/QuestLog"]
        self.assertEqual(ql["description"], "A bigger quest log with level colours.")
        self.assertIn("no open licence", ql["notes"])
        for s in out["added"]:
            self.assertNotIn("review", s)                                       # seeds only: nothing approved
            self.assertTrue(cu.name_ok(s["id"].replace("-", "_")))
            self.assertEqual(s["found"], "discover 2026-11-03")
        why = dict(out["rejected"])
        self.assertIn("collection", why["carl/WoW-3.3.5a-Addons"])
        self.assertIn("by choice", why["dora/OneButton"])
        self.assertIn("blocklist", why["mallory/Thing"])
        self.assertIn("Chumpton", why["chumpton/Thing"])
        self.assertEqual(why["erin/Fork"], "a fork")
        self.assertIn("12 addon folders", why["gail/Many"])
        self.assertIn("below the bar", why["hal/Zero"])
        self.assertIn(".m2", dict(out["failed"])["fred/Broken"])
        self.assertNotIn("ElvUI-WotLK/ElvUI", why)                              # known seeds are not looked at again
        self.assertEqual(len(doc["seeds"]), 3)

    def test_discover_limit(self):
        doc = {"seeds": []}
        out = cu.discover(doc, None, self.check, "2026-11-03", limit=1, found=dict(self.FOUND), log=lambda *_: None)
        self.assertEqual(len(out["added"]), 1)

    def test_discover_find_uses_search_only(self):
        urls = []

        def get(url):
            urls.append(url)
            if "/search/code?" in url:
                return {"items": [{"repository": {"full_name": "ivy/CodeHit"}}]} if "page=1" in url else {"items": []}
            if "/search/repositories?" in url:
                q = cu.urllib.parse.parse_qs(url.split("?", 1)[1])["q"][0]
                if q.startswith("repo:ivy/CodeHit"):
                    return {"items": [repo_doc("ivy/CodeHit", 7)]}
                if q.startswith("user:bkader"):
                    return {"items": [repo_doc("bkader/Thing", 3)]}
                if q.startswith("topic:wotlk-addon "):
                    return {"items": [repo_doc("alice/NicePlates", 40, "Nameplates"), repo_doc("x/Forked", 9, fork=True)]}
                return {"items": []}
            return None
        found = cu.discover_find(get, 1, sleep=lambda s: None)
        self.assertEqual(found["ivy/CodeHit"]["stars"], 7)
        self.assertIn("bkader/Thing", found)
        self.assertEqual(found["alice/NicePlates"]["description"], "Nameplates")
        self.assertTrue(found["x/Forked"]["fork"])
        self.assertTrue(all(u.startswith("https://api.github.com/search/") for u in urls))

    def test_discover_text(self):
        self.assertEqual(cu.discover_text("A tidy bag addon"), "A tidy bag addon.")
        for bad in ("Backport of X for WotLK", "Addon for 3.3.5a", "See https://x.example", "short", "Made for Warmane server"):
            self.assertEqual(cu.discover_text(bad), "", bad)


if __name__ == "__main__":
    unittest.main()
