"""The workflow source copies (tools/github-workflows/*.yml): each parses, and wu-images publishes exactly what the
host kit pulls, with only the built-in token, from the realm's own build recipe, with no game data in the context.

    python3 tools/github-workflows/test_workflows.py        (needs PyYAML)
"""
import json
import os
import re
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
try:
    import yaml
except ImportError:  # pragma: no cover
    yaml = None


def load(name):
    text = (HERE / name).read_text(encoding="utf-8")
    doc = yaml.safe_load(text)
    doc["on"] = doc.get("on", doc.pop(True, None))      # YAML 1.1 reads a bare `on` as True
    return text, doc


def steps(doc, job):
    return doc["jobs"][job]["steps"]


@unittest.skipIf(yaml is None, "PyYAML is not installed")
class Workflows(unittest.TestCase):
    def test_all_parse(self):
        names = sorted(p.name for p in HERE.glob("*.yml"))
        self.assertEqual(names, ["bots-canary.yml", "den-lite-web.yml", "den-lite.yml", "engine-windows.yml", "forge.yml", "hub-index.yml", "leyline-android.yml", "leyline-launcher.yml", "realm-scripts.yml", "scout.yml",
                                 "survivors.yml", "wu-hosting.yml", "wu-images.yml"])
        for n in names:
            text, doc = load(n)
            self.assertTrue(doc["name"] and doc["on"] and doc["jobs"], n)
            for job, j in doc["jobs"].items():
                self.assertIn("runs-on", j, f"{n}:{job}")
                for s in j["steps"]:
                    self.assertTrue("uses" in s or "run" in s, f"{n}:{job}: {s}")
                    if "uses" in s and not s["uses"].startswith("./"):
                        # pinned by commit (a moved tag can't change what runs); the line's comment names the tag
                        self.assertRegex(s["uses"], r"^[\w.-]+/[\w.-]+@[0-9a-f]{40}$", f"{n}: pin actions to a commit SHA")
                        self.assertRegex(text, re.escape(s["uses"]) + r" # (v\d+|stable)\n", f"{n}: name the tag in a comment")
            self.assertNotIn("\t", text, n)

    def test_forge(self):
        """Forge (GPL-3.0, forge/): only when forge/ changes or by hand; Windows; pinned tools; the built-in token only;
        the zip, its sha256 and the source tarball on the rolling release forge-latest; the UI word check runs first."""
        text, doc = load("forge.yml")
        self.assertEqual(doc["on"]["push"]["paths"], ["forge/**", ".github/workflows/forge.yml"])
        self.assertEqual(doc["on"]["push"]["branches"], ["main"])
        self.assertIn("workflow_dispatch", doc["on"])
        self.assertNotIn("pull_request", doc["on"])
        self.assertEqual(doc["permissions"], {"contents": "read"})
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)), {"GITHUB_TOKEN"})
        job = doc["jobs"]["windows"]
        self.assertEqual(job["runs-on"], "windows-2022")
        self.assertEqual(job["permissions"], {"contents": "write"})
        for k in ("QT_VERSION", "AQT_VERSION", "CMAKE_VERSION", "VCPKG_TAG"):
            self.assertRegex(str(doc["env"][k]), r"^\d", k)
        run = "\n".join(s.get("run", "") for s in job["steps"])
        self.assertLess(run.index("check_ui_words.py"), run.index("cmake -S forge"))
        self.assertIn("windeployqt", run)
        self.assertIn("Forge.zip.sha256", run)
        self.assertIn("gh release upload forge-latest Forge.zip Forge.zip.sha256 forge-source.tar.gz", run)
        self.assertIn("SOURCE.txt", run)
        self.assertNotRegex(run, r"(?i)\.mpq|\bData/")

    def test_den_lite(self):
        """Den Lite (labs/den-lite, Play solo's server): only when it changes or by hand; tested before it is built;
        Windows and Linux; the built-in token only; each program with its sha256 on the rolling release den-lite-latest,
        at the names the launcher downloads (host/lite.rs asset_for)."""
        text, doc = load("den-lite.yml")
        self.assertEqual(doc["on"]["push"]["paths"], ["labs/den-lite/**", ".github/workflows/den-lite.yml"])
        self.assertEqual(doc["on"]["push"]["branches"], ["main"])
        self.assertIn("workflow_dispatch", doc["on"])
        self.assertNotIn("pull_request", doc["on"])
        self.assertEqual(doc["permissions"], {"contents": "read"})
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)), {"GITHUB_TOKEN"})
        self.assertEqual(doc["jobs"]["windows"]["runs-on"], "windows-2022")
        self.assertEqual(doc["jobs"]["linux"]["needs"], "windows")
        lite = (REPO / "launcher" / "src-tauri" / "src" / "host" / "lite.rs").read_text(encoding="utf-8")
        for job, name in (("windows", "den-lite-windows-x64.exe"), ("linux", "den-lite-linux-x64")):
            j = doc["jobs"][job]
            self.assertEqual(j["permissions"], {"contents": "write"})
            run = "\n".join(s.get("run", "") for s in j["steps"])
            self.assertLess(run.index("cargo test --locked"), run.index("cargo build --locked"), job)
            self.assertIn(f"{name}.sha256", run)
            self.assertRegex(run, rf"gh release upload den-lite-latest {re.escape(name)} {re.escape(name)}\.sha256")
            self.assertIn(f'"{name}"', lite, f"the launcher downloads {name}")
            self.assertNotRegex(run, r"(?i)\.mpq|\bData/")
        self.assertIn('releases/download/den-lite-latest/"', lite)

    def test_den_lite_web(self):
        """Den Lite in the page (docs/PHONE-SOLO.md): only when Den Lite or the solo layer changes, or by hand; Den Lite's
        own tests before the wasm build; the wasm (wasm32-wasip1, feature web, bin den-lite-web) tested by the solo tests
        (node, then Chromium) before it goes with its sha256 on den-lite-latest, at the name Control's solo-wasm step
        downloads and www-patch.py copies; one release queue with den-lite.yml; the built-in token only; installed."""
        text, doc = load("den-lite-web.yml")
        paths = doc["on"]["push"]["paths"]
        for p in ("labs/den-lite/**", "stream/www-files/solo/**", ".github/workflows/den-lite-web.yml"):
            self.assertIn(p, paths)
        self.assertEqual(doc["on"]["push"]["branches"], ["main"])
        self.assertIn("workflow_dispatch", doc["on"])
        self.assertEqual(doc["permissions"], {"contents": "read"})
        self.assertEqual(doc["concurrency"]["group"], load("den-lite.yml")[1]["concurrency"]["group"])
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)), {"GITHUB_TOKEN"})
        j = doc["jobs"]["web"]
        self.assertEqual(j["permissions"], {"contents": "write"})
        run = "\n".join(s.get("run", "") for s in j["steps"])
        self.assertLess(run.index("cargo test --locked"), run.index("--target wasm32-wasip1 --features web --bin den-lite-web"))
        self.assertLess(run.index("--bin den-lite-web"), run.index("tests/solo.node.test.cjs"))
        self.assertLess(run.index("tests/solo.browser.test.cjs"), run.index("gh release upload"))
        self.assertRegex(run, r"gh release upload den-lite-latest den-lite-web\.wasm den-lite-web\.wasm\.sha256")
        self.assertNotRegex(run, r"(?i)\.mpq|\bData/")
        toml = (REPO / "labs" / "den-lite" / "Cargo.toml").read_text(encoding="utf-8")
        self.assertIn('name = "den-lite-web"', toml)
        self.assertIn('required-features = ["web"]', toml, "the PC build never builds the web bin")
        patch = (REPO / "stream" / "server" / "www-patch.py").read_text(encoding="utf-8")
        self.assertIn('"den-lite-web.wasm"', patch)
        builds = (REPO / "tools" / "github-builds" / "github_builds.py").read_text(encoding="utf-8")
        self.assertIn('"den-lite-web.yml"', builds)
        ctl = (REPO / "tools" / "control" / "control.py").read_text(encoding="utf-8")
        self.assertIn('"solo-wasm"', ctl)
        step = (REPO / "tools" / "solo" / "solo_wasm.py").read_text(encoding="utf-8")
        self.assertIn("leyline-workspace/releases/download/den-lite-latest/", step)
        self.assertIn('NAME = "den-lite-web.wasm"', step)

    def test_engine_windows(self):
        """Single player's Built-in engine (docs/BUILTIN-ENGINE.md): the realm's own sources (fetch.sh at the pinned
        commits, the same paths as wu-images) built natively for Windows; versions pinned; the built-in token only; no
        game data in the sources or the package (checked before the zip); a smoke before the release; the zip, its
        sha256 and engine.json on the rolling release engine-latest at the names the launcher downloads; the .github
        copy byte for byte the source copy."""
        text, doc = load("engine-windows.yml")
        self.assertEqual((REPO / ".github" / "workflows" / "engine-windows.yml").read_text(encoding="utf-8"), text)
        paths = doc["on"]["push"]["paths"]
        for p in ("realm/custom/sources.lock", "realm/custom/patches/**", "realm/custom/fetch.sh", "realm/custom/modules/**",
                  "realm/custom/lua_scripts/**", "realm/profiles/**", "realm/modules.env", ".github/workflows/engine-windows.yml"):
            self.assertIn(p, paths)
        self.assertEqual(doc["on"]["push"]["branches"], ["main"])
        self.assertIn("workflow_dispatch", doc["on"])
        self.assertNotIn("pull_request", doc["on"])
        self.assertEqual(doc["permissions"], {"contents": "read"})
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)), {"GITHUB_TOKEN"})
        self.assertEqual(doc["env"]["MYSQL_VERSION"], "8.4.9")   # the host kit runs mysql:8.4; the fork's own CI links 8.4.9
        self.assertRegex(doc["env"]["BOOST_VERSION"], r"^\d+\.\d+\.\d+$")
        job = doc["jobs"]["windows"]
        self.assertEqual(job["runs-on"], "windows-2022")
        self.assertEqual(job["permissions"], {"contents": "write"})
        names = [s.get("name", "") for s in job["steps"]]
        run = "\n".join(s.get("run", "") for s in job["steps"])
        self.assertIn('FETCH_DIR="$PWD/realm/custom" sh realm/custom/fetch.sh', run)
        self.assertIn('cd "${FETCH_DIR:-/w}"', (REPO / "realm" / "custom" / "fetch.sh").read_text(encoding="utf-8"),
                      "fetch.sh still runs in the Docker container at /w")
        self.assertIn("rm -rf realm/custom/acore/modules/mod-arac/patch-contents", run)
        for flag in ("-DTOOLS_BUILD=all", "-DSCRIPTS=static", "-DMODULES=static", "-DWITH_WARNINGS=OFF", "-DCMAKE_BUILD_TYPE=Release"):
            self.assertIn(flag, run)
        order = [next(i for i, n in enumerate(names) if n.startswith(k)) for k in
                 ("Sources", "No game data in the sources", "Configure", "Build", "Package", "No game data in the package", "Smoke",
                  "Zip", "Put it on engine-latest")]
        self.assertEqual(order, sorted(order))
        for prog in ("worldserver", "authserver", "dbimport", "map_extractor", "mmaps_generator", "mysqld.exe", "mysqldump.exe"):
            self.assertIn(prog, run)
        self.assertIn("-mm=Deflate", run, "the launcher's unzip has deflate only")
        self.assertIn("gh release upload engine-latest leyline-engine-win64.zip leyline-engine-win64.zip.sha256 leyline-engine-win64.json", run)
        self.assertIn("SOURCES.txt", run)
        self.assertIn("bind-address=127.0.0.1", run)
        eng = (REPO / "launcher" / "src-tauri" / "src" / "host" / "engine.rs").read_text(encoding="utf-8")
        self.assertIn('"leyline-engine-win64.zip"', eng)
        self.assertIn('releases/download/engine-latest/"', eng)
        self.assertNotRegex(run, r"(?i)\bData/|common\.mpq")

    def test_survivors(self):
        """Azeroth Survivors (docs/SURVIVORS.md): only when its lock or build script changes, or by hand; only upstream's
        wasm build steps (never the ones that download the game client); a 16 GB runner (this repo is public); the
        build checked against the lock and the committed lock before anything is published; every asset with its
        sha256 on the rolling release survivors-latest, at the names tools/survivors/survivors_fetch.py downloads."""
        text, doc = load("survivors.yml")
        self.assertEqual(doc["on"]["push"]["paths"], ["stream/survivors/upstream.lock.json", "tools/survivors/build-dist.sh",
                                                      "tools/survivors/licenses-index.html", ".github/workflows/survivors.yml"])
        self.assertEqual(doc["on"]["push"]["branches"], ["main"])
        self.assertIn("workflow_dispatch", doc["on"])
        self.assertNotIn("pull_request", doc["on"])
        self.assertEqual(doc["permissions"], {"contents": "read"})
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)), {"GITHUB_TOKEN"})
        self.assertEqual(doc["jobs"]["dist"]["runs-on"], "ubuntu-24.04")
        self.assertEqual(doc["jobs"]["windows-host"]["runs-on"], "windows-2022")
        self.assertEqual(doc["jobs"]["windows-host"]["needs"], "dist")
        run = "\n".join(s.get("run", "") for j in doc["jobs"].values() for s in j["steps"])
        for bad in ("fetch-data", "make build", "make native", "scripts/setup.sh", "setup.sh", "archive.org", "--data"):
            self.assertNotIn(bad, run, bad)
        self.assertNotRegex(run, r"(?i)\.mpq|\bData/")
        self.assertIn('build-dist.sh "$RUNNER_TEMP/sv" --write-lock', run)
        self.assertLess(run.index("--write-lock"), run.index("differs from the committed lock"))
        self.assertLess(run.index("survivors_check.py dist"), run.index("gh release upload survivors-latest"))
        self.assertIn("-p wenilla-host --bin wenilla-host -p benilla-formats --bin mpqx", run)
        fetch = (REPO / "tools" / "survivors" / "survivors_fetch.py").read_text(encoding="utf-8")
        self.assertIn("releases/download/survivors-latest/", fetch)
        for name in ("survivors-dist.tgz", "upstream.lock.json", "wenilla-host-windows-x64.exe", "mpqx-windows-x64.exe"):
            self.assertIn(f"{name}.sha256", run, name)
            self.assertRegex(run, rf"gh release upload survivors-latest [^\n]*{re.escape(name)} ")
            self.assertIn(f'"{name}"', fetch, f"the owner's fetch downloads {name}")
        self.assertIn("survivors-lock", text)   # the lock as a build artifact too

    def test_hub_index(self):
        """Daily and on demand; the built-in token only; PRs from the daily and review branches, never a push to main;
        only the daily refresh is merged by the workflow; new thumbnails travel in the review PR."""
        text, doc = load("hub-index.yml")
        self.assertIn("schedule", doc["on"])
        self.assertIn("workflow_dispatch", doc["on"])
        self.assertNotIn("push", doc["on"])
        self.assertEqual(doc["permissions"], {"contents": "write", "pull-requests": "write", "issues": "read"})
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)), {"GITHUB_TOKEN"})
        job = doc["jobs"]["refresh"]
        self.assertEqual((job["env"]["DAILY"], job["env"]["REVIEW"]), ("hub-index/daily", "hub-index/review"))
        run = "\n".join(s.get("run", "") for s in job["steps"])
        self.assertIn("hub_index.py discover", run)
        self.assertIn("hub_index.py build", run)
        self.assertIn("--thumbs 30", run)
        self.assertIn("pip install", run)
        self.assertIn("hub_index.py gate", run)
        self.assertIn("hub_index.py check", run)
        self.assertIn('git push -q -f origin "$DAILY"', run)
        self.assertIn('git push -q -f origin "$REVIEW"', run)
        self.assertIn("gh pr create --base main", run)
        self.assertNotRegex(run, r"push[^\n]*\bmain\b")
        self.assertEqual(re.findall(r"gh pr merge (\S+)", run), ['"$DAILY"'])
        self.assertIn("stream/www-files/art/hub-thumbs", run.split("Review PR")[-1] if "Review PR" in run else run[run.index('"$REVIEW"'):])
        form = yaml.safe_load((HERE / "ISSUE_TEMPLATE" / "submit-mod.yml").read_text(encoding="utf-8"))
        self.assertEqual(form["labels"], ["mod-submission"])
        labels = [b["attributes"]["label"] for b in form["body"] if b["type"] != "markdown"]
        self.assertEqual(labels[:3], ["Repo link", "Client build", "Category"])
        builds = next(b for b in form["body"] if b.get("id") == "build")["attributes"]["options"]
        self.assertEqual(builds, ["1.12.1", "2.4.3", "3.3.5a", "4.3.4", "5.4.8", "any"])

    def test_scout(self):
        """Daily and on demand; the built-in token only; one PR from the scout branch, never merged, never main."""
        text, doc = load("scout.yml")
        self.assertIn("schedule", doc["on"])
        self.assertIn("workflow_dispatch", doc["on"])
        self.assertNotIn("push", doc["on"])
        self.assertEqual(doc["permissions"], {"contents": "write", "pull-requests": "write"})
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)), {"GITHUB_TOKEN"})
        job = doc["jobs"]["scout"]
        self.assertEqual(job["env"]["BRANCH"], "scout/digest")
        run = "\n".join(s.get("run", "") for s in job["steps"])
        self.assertIn("scout.py run", run)
        self.assertIn("test_scout.py", run)
        self.assertIn('git push -q -f origin "$BRANCH"', run)
        self.assertNotRegex(run, r"push[^\n]*\bmain\b")
        self.assertNotIn("gh pr merge", run)
        self.assertEqual((REPO / ".github" / "workflows" / "scout.yml").read_text(encoding="utf-8"), text)

    def test_images(self):
        text, doc = load("wu-images.yml")
        self.assertEqual(doc["on"]["push"]["branches"], ["main"])
        self.assertIn("workflow_dispatch", doc["on"])
        paths = doc["on"]["push"]["paths"]
        for p in ("realm/custom/sources.lock", "realm/custom/patches/**", "realm/custom/fetch.sh",
                  "hosting/kit/tools/**", "hosting/relay/**"):
            self.assertIn(p, paths)
        self.assertEqual(doc["permissions"], {"contents": "read", "packages": "write"})
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)), {"GITHUB_TOKEN"})
        # what the kit pulls (Host a Wrath realm.bat, host-wrath-realm.sh, setup.py)
        kit = (REPO / "hosting" / "kit" / "host-wrath-realm.sh").read_text()
        self.assertIn('WU_IMAGE:-ghcr.io/orionopenwow/wu-ac-wotlk}', kit)
        self.assertIn('WU_TAG:-stable}', kit)
        self.assertIn("ghcr.io/orionopenwow/wu-host-tools:1", kit)
        pushed = []
        for job in ("small", "realm"):
            for s in steps(doc, job):
                if s.get("uses", "").startswith("docker/build-push-action"):
                    w = s["with"]
                    self.assertTrue(w["push"])
                    tags = [t.strip() for t in w["tags"].splitlines() if t.strip()]
                    self.assertEqual(len(tags), 2)
                    self.assertTrue(tags[1].endswith(":${{ steps.t.outputs.sha }}"), tags)
                    pushed.append(tags[0])
                    if job == "realm":
                        # the realm's own recipe: realm/docker-compose.build.yml
                        self.assertEqual(w["context"], "realm/custom/acore")
                        self.assertEqual(w["file"], "realm/custom/acore/apps/docker/Dockerfile")
                        self.assertEqual(sorted(w["build-args"].split()),
                                         ["CTOOLS_BUILD=db-only", "CTYPE=Release", "CWITH_WARNINGS=OFF"])
                        self.assertEqual(tags[0], f"ghcr.io/orionopenwow/wu-ac-wotlk-{w['target']}:stable")
        self.assertEqual(sorted(pushed), sorted([
            "ghcr.io/orionopenwow/wu-ac-wotlk-db-import:stable", "ghcr.io/orionopenwow/wu-ac-wotlk-authserver:stable",
            "ghcr.io/orionopenwow/wu-ac-wotlk-worldserver:stable", "ghcr.io/orionopenwow/wu-host-tools:1",
            "ghcr.io/orionopenwow/wu-relay:1"]))
        compose = yaml.safe_load((REPO / "realm" / "docker-compose.build.yml").read_text())
        args = compose["x-build"]["args"]
        for k, v in args.items():
            self.assertIn(f"{k}={v}", text)
        # sources: fetch.sh (pinned commits + patches) before the build; no game data in the context
        realm = steps(doc, "realm")
        names = [s.get("name", s.get("uses", "")) for s in realm]
        i_fetch = next(i for i, s in enumerate(realm) if "fetch.sh" in s.get("run", ""))
        i_clean = next(i for i, s in enumerate(realm) if "patch-contents" in s.get("run", ""))
        i_build = next(i for i, s in enumerate(realm) if s.get("uses", "").startswith("docker/build-push-action"))
        self.assertLess(i_fetch, i_clean, names)
        self.assertLess(i_clean, i_build, names)
        self.assertIn("rm -rf realm/custom/acore/modules/mod-arac/patch-contents", realm[i_clean]["run"])
        self.assertIn("exit 1", realm[i_clean]["run"])
        owners = set(re.findall(r"ghcr\.io/([a-z0-9-]+)/", text)) | set(re.findall(r"github\.com/([A-Za-z0-9-]+)/", text))
        self.assertEqual(owners, {"orionopenwow"})

    def test_bots_canary(self):
        """Bots always up to date (docs/BOTS-UPDATES.md): weekly and by hand; the compat guard before the long build;
        canary tags only (never :stable); the smoke test only on the self-hosted runner with the owner's own game
        data, when BOTS_SMOKE_RUNNER is on; a pull request (draft while the smoke test is pending), never a merge;
        the built-in token only."""
        text, doc = load("bots-canary.yml")
        self.assertEqual((REPO / ".github" / "workflows" / "bots-canary.yml").read_text(encoding="utf-8"), text)
        self.assertIn("schedule", doc["on"])
        self.assertIn("workflow_dispatch", doc["on"])
        self.assertNotIn("push", doc["on"])
        self.assertEqual(doc["permissions"], {"contents": "read"})
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)), {"GITHUB_TOKEN"})
        jobs = doc["jobs"]
        self.assertEqual(list(jobs), ["bump", "images", "smoke", "propose"])
        bump = "\n".join(s.get("run", "") for s in steps(doc, "bump"))
        self.assertLess(bump.index("compat.py"), bump.index("pins.py bump"))
        self.assertIn("pins.py same", bump)
        # canary tags only
        tags = [t.strip() for s in steps(doc, "images") if s.get("uses", "").startswith("docker/build-push-action")
                for t in s["with"]["tags"].splitlines() if t.strip()]
        self.assertEqual(len(tags), 6)
        self.assertTrue(all(":canary" in t for t in tags), tags)
        code = "\n".join(l for l in text.splitlines() if not l.lstrip().startswith("#"))
        self.assertNotIn(":stable", code)
        self.assertEqual(jobs["images"]["permissions"], {"contents": "read", "packages": "write"})
        img = "\n".join(s.get("run", "") for s in steps(doc, "images"))
        self.assertIn("rm -rf realm/custom/acore/modules/mod-arac/patch-contents", img)
        # the smoke test: the owner's own data on his own runner, only when switched on
        self.assertEqual(jobs["smoke"]["runs-on"], ["self-hosted", "leyline-bots-smoke"])
        self.assertIn("vars.BOTS_SMOKE_RUNNER == 'on'", jobs["smoke"]["if"])
        self.assertIn("realm/bots/bots-smoke.sh", "\n".join(s.get("run", "") for s in steps(doc, "smoke")))
        # the proposal: a pull request, never a merge; as Orion
        self.assertEqual(jobs["propose"]["permissions"], {"contents": "write", "pull-requests": "write"})
        run = "\n".join(s.get("run", "") for s in steps(doc, "propose"))
        self.assertIn("gh pr create --draft", run)
        self.assertNotIn("gh pr merge", text)
        self.assertNotIn("git push origin main", text)
        self.assertIn("user.name=Orion", run)
        self.assertIn("needs.smoke.result == 'success' || needs.smoke.result == 'skipped'", jobs["propose"]["if"])

    def test_hosting_tests(self):
        text, doc = load("wu-hosting.yml")
        self.assertEqual(doc["permissions"], {"contents": "read"})
        self.assertNotRegex(text, r"secrets\.\w")
        run = "\n".join(s.get("run", "") for s in steps(doc, "tests"))
        self.assertIn("discover -s hosting/tests", run)

    def test_launcher(self):
        text, doc = load("leyline-launcher.yml")
        self.assertEqual(doc["on"]["push"]["tags"], ["launcher-v*"])
        self.assertEqual(doc["permissions"], {"contents": "read"})
        jobs = doc["jobs"]
        for j in ("create-release", "release", "release-macos"):
            self.assertEqual(jobs[j]["permissions"], {"contents": "write"}, j)
        # only the built-in token and the updater's signing key ("setup\Turn on launcher updates.bat")
        self.assertEqual(set(re.findall(r"secrets\.([A-Za-z_]+)", text)),
                         {"GITHUB_TOKEN", "TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD"})
        # macOS never makes a routine run red: its jobs are allowed to fail, and it is not in the main release job
        self.assertTrue(jobs["release-macos"]["continue-on-error"])
        self.assertTrue(jobs["core-tests-macos"]["continue-on-error"])
        self.assertNotIn("macos-latest", jobs["core-tests"]["strategy"]["matrix"]["os"], "a missing macOS runner can't hold the release")
        for j in ("portable", "create-release"):
            self.assertNotIn("core-tests-macos", jobs[j]["needs"], j)
        self.assertFalse(any("macos" in m["platform"] for m in jobs["release"]["strategy"]["matrix"]["include"]))
        self.assertIn("universal-apple-darwin", text)
        bundles = " ".join(m["args"] for m in jobs["release"]["strategy"]["matrix"]["include"])
        for b in ("nsis", "msi", "appimage", "deb"):
            self.assertIn(b, bundles)
        # one tag push: the tag must match tauri.conf.json; the updater manifest only with the signing key
        self.assertIn("tauri.conf.json", jobs["create-release"]["steps"][1]["run"])
        self.assertIn("includeUpdaterJson", text)
        self.assertIn("createUpdaterArtifacts", text)
        for j in ("release", "release-macos"):
            act = [s for s in jobs[j]["steps"] if s.get("uses", "").startswith("tauri-apps/tauri-action")][0]
            self.assertEqual(act["with"]["releaseId"], "${{ needs.create-release.outputs.id }}")
        # the update check reads the same (public) repo's releases
        upd = (REPO / "launcher" / "src-tauri" / "src" / "update.rs").read_text()
        self.assertIn('"https://api.github.com/repos/orionopenwow/Leyline/releases', upd)

    def test_android(self):
        """The phone app (docs/ANDROID.md): on launcher changes and by hand; the Android project made fresh by
        `tauri android init` (gen/ never committed), the crystal icon and the full-screen activity; a debug APK for arm64
        and x86_64, the release APK only with the release key in the secrets (never in a file); an API 34 x86_64
        emulator starts it, takes the screenshots and fails on a crash; no game data in the APK."""
        text, doc = load("leyline-android.yml")
        self.assertEqual(doc["on"]["push"]["branches"], ["main"])
        self.assertIn("launcher/**", doc["on"]["push"]["paths"])
        self.assertIn(".github/workflows/leyline-android.yml", doc["on"]["push"]["paths"])
        self.assertEqual(doc["on"]["pull_request"]["paths"], ["launcher/**"])
        self.assertIn("workflow_dispatch", doc["on"])
        self.assertEqual(doc["permissions"], {"contents": "read"})
        self.assertEqual(set(re.findall(r"secrets\.(\w+)", text)),
                         {"ANDROID_KEYSTORE_B64", "ANDROID_KEYSTORE_PASSWORD", "ANDROID_KEY_ALIAS", "ANDROID_KEY_PASSWORD", "GITHUB_TOKEN"})
        jobs = doc["jobs"]
        self.assertEqual(list(jobs), ["apk", "smoke", "publish"])
        apk = jobs["apk"]
        self.assertNotIn("permissions", apk, "no write access: artifacts only")
        names = [s.get("name", s.get("uses", "")) for s in apk["steps"]]
        run = "\n".join(s.get("run", "") for s in apk["steps"])
        java = next(s for s in apk["steps"] if s.get("uses", "").startswith("actions/setup-java"))
        self.assertEqual(java["with"]["java-version"], "17")
        rust = next(s for s in apk["steps"] if s.get("uses", "").startswith("dtolnay/rust-toolchain"))
        self.assertEqual(set(rust["with"]["targets"].split(",")), {"aarch64-linux-android", "armv7-linux-androideabi", "x86_64-linux-android"})
        self.assertIn("sdkmanager", run)
        self.assertRegex(str(doc["env"]["ANDROID_NDK"]), r"^\d+\.\d+\.\d+$")
        self.assertEqual(doc["env"]["ANDROID_API"], "34")
        self.assertIn("npm ci --no-audit --no-fund", run)
        self.assertNotRegex(run, r"npm install\b")
        # the Android project is made here, never committed; the activity and the icon are ours
        self.assertIn("npx tauri android init", run)
        self.assertIn("src-tauri/gen/", (REPO / "launcher" / ".gitignore").read_text(encoding="utf-8"), "gen/ is never committed")
        self.assertIn("src-tauri/mobile/android/MainActivity.kt", run)
        self.assertTrue((REPO / "launcher" / "src-tauri" / "mobile" / "android" / "MainActivity.kt").is_file())
        self.assertIn("npx tauri icon ../stream/www-files/art/leyline-crystal-512.png", run)
        self.assertTrue((REPO / "stream" / "www-files" / "art" / "leyline-crystal-512.png").is_file())
        order = [next(i for i, n in enumerate(names) if n.startswith(k)) for k in
                 ("Android SDK", "Install the Tauri CLI", "The Android project", "The app icon", "Build the debug APK", "No game data in the APK")]
        self.assertEqual(order, sorted(order))
        self.assertIn("npx tauri android build --debug --apk --target aarch64 --target x86_64", run)
        self.assertRegex(run, r"(?i)\\\.\(mpq\|m2\|wmo\|adt\|blp\|dbc")
        # release signing: only with the key in the secrets, decoded to the runner's temp and deleted
        rel = [s for s in apk["steps"] if "apksigner" in s.get("run", "")]
        self.assertEqual(len(rel), 1)
        self.assertEqual(rel[0]["if"], "env.HAS_RELEASE_KEY == 'true'")
        self.assertEqual(apk["env"]["HAS_RELEASE_KEY"], "${{ secrets.ANDROID_KEYSTORE_B64 != '' }}")
        self.assertIn('trap \'rm -f "$ks"\' EXIT', rel[0]["run"])
        self.assertIn('"$RUNNER_TEMP/leyline-release.jks"', rel[0]["run"])
        self.assertIn("apksigner\" verify", rel[0]["run"])
        ups = {s["with"]["name"]: s for s in apk["steps"] if s.get("uses", "").startswith("actions/upload-artifact")}
        self.assertEqual(set(ups), {"leyline-android-debug", "leyline-android-release"})
        self.assertEqual(ups["leyline-android-release"]["if"], "env.HAS_RELEASE_KEY == 'true'")
        for f in (".jks", ".keystore", "keystore.properties"):
            self.assertNotIn(f, " ".join(str(s.get("with", {}).get("path", "")) for s in ups.values()))
        # the emulator: Android 14 x86_64, our smoke script, its screenshots kept even when it fails
        smoke = jobs["smoke"]
        self.assertEqual(smoke["needs"], "apk")
        emu = next(s for s in smoke["steps"] if s.get("uses", "").startswith("reactivecircus/android-emulator-runner"))
        self.assertEqual((emu["with"]["api-level"], emu["with"]["arch"]), (34, "x86_64"))
        self.assertIn("launcher/scripts/android-smoke.sh apk/Leyline-debug.apk", emu["with"]["script"])
        self.assertIn("com.leylinelauncher.app", emu["with"]["script"])
        art = [s for s in smoke["steps"] if s.get("uses", "").startswith("actions/upload-artifact")][0]
        self.assertEqual(art["if"], "always()")
        sh = (REPO / "launcher" / "scripts" / "android-smoke.sh").read_text(encoding="utf-8")
        for bit in ("adb install", "screencap -p", "FATAL EXCEPTION", "Fatal signal", "pidof", "user_rotation 1"):
            self.assertIn(bit, sh)
        # publish (docs/MOBILE-RELEASE.md): main only, after the emulator passed, onto the rolling pre-release
        # android-latest; the only job that may write, with the built-in token
        pub = jobs["publish"]
        self.assertEqual(pub["needs"], ["apk", "smoke"])
        self.assertEqual(pub["if"], "github.ref == 'refs/heads/main' && github.event_name != 'pull_request'")
        self.assertEqual(pub["permissions"], {"contents": "write"})
        prun = "\n".join(s.get("run", "") for s in pub["steps"])
        self.assertIn("gh release create android-latest", prun)
        self.assertIn("--prerelease --latest=false", prun)
        self.assertIn("sha256sum -c Leyline-debug.apk.sha256", prun)
        self.assertRegex(prun, r"(?i)\\\.\(mpq\|m2\|wmo")
        self.assertNotIn("ANDROID_KEYSTORE", prun, "the release key never reaches the publish job")
        self.assertEqual(set(re.findall(r"secrets\.(\w+)", str(pub))), {"GITHUB_TOKEN"})
        conf = json.loads((REPO / "launcher" / "src-tauri" / "tauri.android.conf.json").read_text(encoding="utf-8"))
        self.assertEqual(conf["identifier"], doc["env"]["APP_ID"])
        self.assertGreaterEqual(conf["bundle"]["android"]["minSdkVersion"], 26)
        self.assertNotRegex(run, r"(?i)\bData/")

    def test_launcher_every_ship_is_a_release_for_everyone(self):
        # owner 31 Oct: "any update to this launcher from now on is auto queued for restart update for everyone"
        text, doc = load("leyline-launcher.yml")
        jobs = doc["jobs"]
        self.assertEqual(doc["on"]["push"]["branches"], ["main"])
        self.assertNotIn("paths", doc["on"]["push"], "a paths filter can skip tag pushes")
        plan = jobs["plan"]
        self.assertIn("refs/heads/main", plan["if"])
        self.assertEqual(plan["steps"][0]["with"]["fetch-depth"], 0, "the plan reads the launcher-v tags")
        self.assertIn("release_version.py plan", plan["steps"][1]["run"])
        self.assertTrue((REPO / "launcher" / "scripts" / "release_version.py").is_file(), "exported with launcher/")
        cr = jobs["create-release"]
        self.assertEqual(cr["needs"], ["core-tests", "plan"])
        self.assertIn("needs.plan.outputs.ship == 'true'", cr["if"])
        self.assertIn("startsWith(github.ref, 'refs/tags/launcher-v')", cr["if"])
        run = cr["steps"][1]["run"]
        # a ship: never a pre-release, tagged at the pushed commit; only a hand-pushed tag with "-" is one
        ship = run.split("else", 1)[1].split("fi", 1)[0]
        self.assertNotIn("prerelease", ship)
        self.assertIn('--target "$GITHUB_SHA"', ship)
        self.assertIn('*-*) pre="--prerelease"', run)
        self.assertIn("latest.json", run, "a release already finished is not built twice")
        for j in ("release", "release-macos"):
            self.assertEqual(jobs[j]["if"], "needs.create-release.outputs.skip != 'true'", j)
            names = [s.get("name", "") for s in jobs[j]["steps"]]
            st = jobs[j]["steps"][names.index("This ship's version")]
            self.assertIn("release_version.py stamp", st["run"])
            self.assertLess(names.index("This ship's version"), names.index("Install the Tauri CLI"), "stamped before the build")
            act = [s for s in jobs[j]["steps"] if s.get("uses", "").startswith("tauri-apps/tauri-action")][0]
            self.assertEqual(act["with"]["includeUpdaterJson"], "${{ needs.create-release.outputs.signed == 'true' }}")
        self.assertIn("needs.plan.outputs.ship == 'true'", jobs["portable"]["if"])
        self.assertIn("release_version.py stamp", "\n".join(s.get("run", "") for s in jobs["portable"]["steps"]))
        # one release run at a time; pull requests apart
        self.assertFalse(doc["concurrency"]["cancel-in-progress"])
        self.assertIn("'release'", doc["concurrency"]["group"])
        self.assertNotRegex(text, r"never downloads")

    def test_launcher_pinned_builds(self):
        # security review 2 Nov (F2): the signing key runs beside the build tools, so they come from the lockfiles
        text, doc = load("leyline-launcher.yml")
        self.assertNotRegex(text, r"npm install\b")
        self.assertEqual(text.count("npm ci --no-audit --no-fund"), 3)
        self.assertIn("cargo test --locked", text)
        self.assertIn("npx tauri build --no-bundle -- --locked", text)
        for j in ("release", "release-macos"):
            act = [s for s in doc["jobs"][j]["steps"] if s.get("uses", "").startswith("tauri-apps/tauri-action")][0]
            self.assertTrue(act["with"]["args"].endswith(" -- --locked"), j)
        self.assertTrue((REPO / "launcher" / "package-lock.json").is_file(), "npm ci needs the lockfile")
        self.assertTrue((REPO / "launcher" / "src-tauri" / "Cargo.lock").is_file())
        self.assertNotIn("package-lock.json", (REPO / "launcher" / ".gitignore").read_text())

    def test_launcher_downloads_and_checksums(self):
        # owner 2 Nov: download options (the exe, a zip, ...) and a SHA256SUMS file per release (stream/downloads.py)
        text, doc = load("leyline-launcher.yml")
        jobs = doc["jobs"]
        run = "\n".join(s.get("run", "") for s in jobs["portable"]["steps"])
        self.assertIn("Leyline-portable.zip", run)
        self.assertIn("sha256sum Leyline.exe Leyline-portable.zip > SHA256SUMS.txt", run)
        self.assertIn("gh release upload launcher-latest Leyline.exe Leyline-portable.zip SHA256SUMS.txt", run)
        sums = jobs["checksums"]
        self.assertEqual(sums["needs"], ["create-release", "release", "release-macos"])
        self.assertEqual(sums["permissions"], {"contents": "write"})
        self.assertIn("needs.create-release.outputs.skip != 'true'", sums["if"])
        srun = sums["steps"][0]["run"]
        self.assertIn('gh release download "$TAG"', srun)
        self.assertIn("sha256sum -- * > ../SHA256SUMS.txt", srun)
        self.assertIn('gh release upload "$TAG" SHA256SUMS.txt', srun)
        self.assertEqual(sums["steps"][0]["env"]["TAG"], "${{ needs.create-release.outputs.tag }}")
        self.assertIn('echo "tag=$TAG"', jobs["create-release"]["steps"][1]["run"])
        # the site's names match what the workflow attaches
        dl = (REPO / "stream" / "downloads.py").read_text(encoding="utf-8")
        for name in ("Leyline.exe", "Leyline-portable.zip", "SHA256SUMS.txt"):
            self.assertIn(f'"{name}"', dl)


if __name__ == "__main__":
    unittest.main()
