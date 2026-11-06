#!/usr/bin/env python3
"""Tests for mobile_export.py (the Leyline-Mobile repo, docs/MOBILE-RELEASE.md) and the Control step that runs it.
Temp folders only.

    python3 -I tools/leyline-repo/test_mobile_export.py
"""
import importlib.util
import io
import os
import re
import shutil
import sys
import tempfile
from contextlib import redirect_stdout
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
spec = importlib.util.spec_from_file_location("mobile_export", HERE / "mobile_export.py")
me = importlib.util.module_from_spec(spec)
spec.loader.exec_module(me)
try:
    import yaml
except ImportError:   # pragma: no cover
    yaml = None

fails = count = 0
REF = "0123456789abcdef0123456789abcdef01234567"


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra and not cond else ""))
    fails += 0 if cond else 1


def run(src, dst, **kw):
    buf = io.StringIO()
    with redirect_stdout(buf):
        res = me.export(Path(src), Path(dst), out=print, **kw)
    return res, buf.getvalue()


def refused(src, dst, **kw):
    try:
        run(src, dst, **kw)
    except me.Refused as e:
        return str(e)
    return ""


tmp = Path(tempfile.mkdtemp(prefix="ly-mobile-"))
try:
    out = tmp / "Leyline-Mobile"
    res, log = run(REPO, out, ref=REF)
    files = set(res["files"])
    # ------------------------------------------------------------------ the tree
    for want in ("README.md", "LICENSE", ".gitignore", "LEYLINE_REF", ".github/workflows/android.yml",
                 "web/touch.js", "web/touch-driver.js", "web/input.js", "web/leyline.webmanifest", "web/solo/solo.js",
                 "web/solo/den-lite-worker.js", "app/ui/phone.js", "app/ui/phone.css", "app/ui/platform.js",
                 "app/src-tauri/src/mobile.rs", "app/src-tauri/mobile/android/MainActivity.kt",
                 "app/src-tauri/tauri.android.conf.json", "docs/ANDROID.md", "docs/TOUCH-CONTROLS.md",
                 "docs/MOBILE-RELEASE.md", "docs/ON-YOUR-PHONE.md", "screenshots/phone-play.webp", "art/leyline-crystal-512.png"):
        check(f"exports {want}", want in files)
    check("every screenshot of the page", {f"screenshots/{n}.webp" for n in ["phone-play", "phone-play-landscape",
          "phone-realms", "phone-mods", "phone-account", "phone-join", "phone-solo", "phone-touch"]} <= files)
    check("nothing outside the layout's folders", all(f.split("/")[0] in set(me.MANAGED) | {"README.md", "LICENSE", ".gitignore", "LEYLINE_REF"} for f in files),
          sorted(f for f in files if f.split("/")[0] not in me.MANAGED))
    check("no wasm, no APK, no keys, no game data", not [f for f in files if re.search(r"\.(wasm|apk|aab|jks|keystore|mpq|m2|blp|dbc|env)$", f, re.I)])
    check("byte-for-byte copies of the sources", (out / "web/touch.js").read_bytes() == (REPO / "stream/www-files/touch.js").read_bytes()
          and (out / "app/ui/phone.js").read_bytes() == (REPO / "launcher/ui/phone.js").read_bytes())
    check("LICENSE is MIT", (out / "LICENSE").read_bytes().lstrip().startswith(b"MIT License"))
    check("LEYLINE_REF is the commit given", (out / "LEYLINE_REF").read_text() == REF + "\n")
    readme = (out / "README.md").read_text(encoding="utf-8")
    check("README: the legal line", me.LEGAL in readme)
    check("README: shows screenshots that are in the repo", all((out / p).is_file() for p in re.findall(r"\]\((screenshots/[^)]+)\)", readme))
          and len(re.findall(r"\]\((screenshots/[^)]+)\)", readme)) >= 5)
    check("README: links only docs that are in the repo", all((out / p).is_file() for p in re.findall(r"\]\((docs/[^)#]+)", readme)))
    check("README: no brand words but the legal line", not me.ex.brand_hits("README.md", readme.encode()))
    check("README: says what a phone can't do, and why the app builds from the Leyline repo",
          "What a phone can't do" in readme and "orionopenwow/Leyline" in readme and "LEYLINE_REF" in readme)
    check("README: owner-voice line marked", "TODO(owner)" in readme)
    check("README: no money links", not re.search(r"(?i)donat|patreon|ko-fi|paypal|sponsor", readme))
    for d in ("docs/MOBILE-RELEASE.md", "docs/ON-YOUR-PHONE.md", "docs/ANDROID.md"):
        t = (out / d).read_text(encoding="utf-8")
        links = re.findall(r"\]\(([A-Za-z0-9_./-]+\.md)", t)
        check(f"{d}: its links point at docs in this repo", all((out / "docs" / l).is_file() for l in links), links)
    # ------------------------------------------------------------------ the workflow
    wf = (out / ".github/workflows/android.yml").read_text(encoding="utf-8")
    if yaml:
        doc = yaml.safe_load(wf)
        on = doc.get("on", doc.get(True))
        check("workflow: parses; apk, smoke, publish", list(doc["jobs"]) == ["apk", "smoke", "publish"])
        check("workflow: runs on a new LEYLINE_REF and by hand", on["push"]["paths"] == ["LEYLINE_REF", ".github/workflows/android.yml"]
              and "workflow_dispatch" in on and "pull_request" not in on)
        check("workflow: pinned to the Leyline commit", doc["env"]["LEYLINE_REF"] == REF)
        cos = [s for j in ("apk", "smoke") for s in doc["jobs"][j]["steps"] if s.get("uses", "").startswith("actions/checkout")]
        check("workflow: apk and smoke check out the Leyline repo at LEYLINE_REF",
              len(cos) == 2 and all(c["with"] == {"repository": "orionopenwow/Leyline", "ref": "${{ env.LEYLINE_REF }}"} for c in cos))
        check("workflow: publishes on this repo's android-latest, main only, after smoke",
              doc["jobs"]["publish"]["needs"] == ["apk", "smoke"] and "gh release upload android-latest" in wf
              and '-R "$GITHUB_REPOSITORY"' in wf and doc["jobs"]["publish"]["permissions"] == {"contents": "write"})
        check("workflow: no game data check kept", "game data in the APK" in wf)
    else:
        check("PyYAML missing: workflow not parsed (skipped)", True)
    check("workflow: a ref that isn't a commit is refused", "full commit id" in refused(REPO, tmp / "x", ref="v1; rm -rf /"))
    check("workflow: main is allowed", run(REPO, tmp / "main", ref="main")[0]["files"] and
          'LEYLINE_REF: "main"' in (tmp / "main/.github/workflows/android.yml").read_text())
    # ------------------------------------------------------------------ the same again: nothing changes; stale files go
    res2, _ = run(REPO, out, ref=REF)
    check("same source in, same bytes out: nothing written the second time", res2["written"] == [] and res2["deleted"] == [])
    (out / "web" / "old.js").write_text("x")
    (out / "notes.txt").write_text("mine")
    (out / ".git").mkdir()
    (out / ".git" / "HEAD").write_text("ref")
    res3, _ = run(REPO, out, ref=REF)
    check("a file no longer exported is deleted, inside the managed folders only", res3["deleted"] == ["web/old.js"]
          and (out / "notes.txt").is_file() and (out / ".git/HEAD").is_file())
    # ------------------------------------------------------------------ refusals on a fake source
    fake = tmp / "src"
    for rel in list(me.LAYOUT_FILES.values()) + ["tools/github-workflows/leyline-android.yml", "LICENSE"]:
        (fake / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(REPO / rel if (REPO / rel).is_file() else REPO / "mods/template/LICENSE", fake / rel)
    for _d, rel_dir, _p in me.LAYOUT_DIRS:
        shutil.copytree(REPO / rel_dir, fake / rel_dir, ignore=shutil.ignore_patterns("*.wasm"))
    check("the fake source exports", run(fake, tmp / "f0", ref=REF)[0]["files"])
    (fake / "stream/www-files/solo/den-lite-web.wasm").write_bytes(b"\0asm")
    check("the solo wasm is never copied", "web/solo/den-lite-web.wasm" not in run(fake, tmp / "f1", ref=REF)[0]["files"])
    t = (fake / "launcher/ui/phone.js").read_text()
    (fake / "launcher/ui/phone.js").write_text(t + '\nconst X = "Play World of Warcraft here";\n')
    check("a brand word in what the app shows stops the export", "World of Warcraft" in refused(fake, tmp / "f2"))
    check("... and nothing was written", not (tmp / "f2").exists())
    (fake / "launcher/ui/phone.js").write_text(t + '\n// not affiliated with or endorsed by Blizzard Entertainment\n')
    check("the legal line is allowed", not refused(fake, tmp / "f3"))
    (fake / "launcher/ui/phone.js").write_text(t)
    d = (fake / "docs/ANDROID.md").read_text()
    (fake / "docs/ANDROID.md").write_text(d + "\nIt needs the 3.3.5a client.\n")
    check("a client version number in a doc's sentence stops the export", "version number" in refused(fake, tmp / "f4"))
    (fake / "docs/ANDROID.md").write_text(d + "\nThe badge: `3.3.5a`.\n")
    check("... but not in code", not refused(fake, tmp / "f5"))
    (fake / "docs/ANDROID.md").write_text(d)
    (fake / "stream/www-files/art/mobile/x.webp").write_bytes(b"-----BEGIN RSA PRIVATE KEY-----")
    check("a secret inside a picture's name pattern stops the export", "secret" in refused(fake, tmp / "f6"))
    (fake / "stream/www-files/art/mobile/x.webp").unlink()
    (fake / "web").mkdir(exist_ok=True)
    (fake / "stream/www-files/touch.js").unlink()
    check("a missing source file stops the export", "missing" in refused(fake, tmp / "f7"))
    shutil.copy(REPO / "stream/www-files/touch.js", fake / "stream/www-files/touch.js")
    wf_src = (fake / "tools/github-workflows/leyline-android.yml").read_text()
    (fake / "tools/github-workflows/leyline-android.yml").write_text(wf_src.replace("name: leyline android\n", "name: other\n"))
    check("a changed source workflow stops the export (no half-made workflow)", "name line" in refused(fake, tmp / "f8"))
    (fake / "tools/github-workflows/leyline-android.yml").write_text(wf_src)
    check("never into the workspace itself", "outside the workspace" in refused(REPO, REPO / "docs"))
    # ------------------------------------------------------------------ the Control step
    bat = (HERE / "Update Leyline-Mobile repo.bat").read_bytes()
    check("bat: CRLF", b"\r\n" in bat and b"\n" not in bat.replace(b"\r\n", b""))
    bt = bat.decode("ascii")
    check("bat: never creates the repo, never force-pushes", "gh repo create" not in bt and "--force" not in bt and "push -f" not in bt)
    check("bat: refuses Chumpton, commits as Orion", "Chumpton" in bt and 'git config user.name "Orion"' in bt
          and 'git config user.email "orionopenwow@users.noreply.github.com"' in bt)
    check("bat: pins the Leyline commit from .repos\\leyline-public", "rev-parse HEAD" in bt and "--leyline-ref !REF!" in bt)
    ctl = (REPO / "tools/control/control.py").read_text(encoding="utf-8")
    check("Control step leyline-mobile runs this bat", '"leyline-mobile": ("tools/leyline-repo/Update Leyline-Mobile repo.bat"' in ctl)
finally:
    shutil.rmtree(tmp, ignore_errors=True)

print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
