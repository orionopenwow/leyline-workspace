"""tools/looks/looks.py: check, list, approve, unapprove and pin on a copy of stream/looks.json.

    python3 -I tools/looks/test_looks_tool.py
"""
import contextlib, hashlib, importlib.util, io, json, os, shutil, sys, tempfile, zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
spec = importlib.util.spec_from_file_location("looks_tool", os.path.join(HERE, "looks.py"))
T = importlib.util.module_from_spec(spec)
spec.loader.exec_module(T)

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, extra)


def run(*argv):
    out = io.StringIO()
    code = 0
    with contextlib.redirect_stdout(out):
        try:
            code = T.main(["--file", LIST, *argv])
        except SystemExit as e:
            code = e.code if isinstance(e.code, int) else 1
            out.write(str(e.code))
    return code, out.getvalue()


d = tempfile.mkdtemp(prefix="looks-tool-")
LIST = os.path.join(d, "looks.json")
shutil.copy(os.path.join(ROOT, "stream", "looks.json"), LIST)
code, out = run("check")
check("check: the real list is sound", code == 0 and out.strip().endswith("ok"), out)
code, out = run("list")
check("list: every entry with its state and how it installs", code == 0 and "look:max" in out and "published" in out
      and "waiting" in out and "launcher" in out and "link" in out, out)

code, out = run("approve", "look:aurora", "--by", "Owner", "--notes", "looked at the art")
doc = json.load(open(LIST))
au = next(e for e in doc["looks"] if e["id"] == "look:aurora")
check("approve: by, at and notes", code == 0 and au["review"]["by"] == "Owner" and au["review"]["at"].endswith("Z")
      and au["review"]["notes"] == "looked at the art")
code, _ = run("unapprove", "look:aurora")
au = next(e for e in json.load(open(LIST))["looks"] if e["id"] == "look:aurora")
check("unapprove: waits again", code == 0 and au["review"]["by"] == "")
code, out = run("approve", "look:nope", "--by", "x")
check("approve: an unknown id is refused", code != 0 and "not in the list" in out)

# pin a zip of two patches and a readme (a local copy of the author's release file: no network in the test)
mpq_a, mpq_b = b"MPQ\x1a" + b"\0" * 60, b"MPQ\x1a" + b"\1" * 60
zp = os.path.join(d, "water-v2.zip")
with zipfile.ZipFile(zp, "w") as z:
    z.writestr("water-v2/patch-W.MPQ", mpq_a)
    z.writestr("water-v2/Data/patch-X.MPQ", mpq_b)
    z.writestr("water-v2/README.txt", b"hi")
url = "https://github.com/ZythDr/EpochWater/releases/download/v2/water-v2.zip"
code, out = run("pin", "look:epoch-water", "--url", url, "--file", zp)
ew = next(e for e in json.load(open(LIST))["looks"] if e["id"] == "look:epoch-water")
inst = ew.get("install") or {}
check("pin: the release file's sha256 and size, only the .MPQ files with theirs, the top folder taken off",
      code == 0 and inst.get("sha256") == hashlib.sha256(open(zp, "rb").read()).hexdigest() and inst.get("size") == os.path.getsize(zp)
      and inst.get("strip_prefix") == "water-v2/" and inst.get("files") == {"patch-W.MPQ": hashlib.sha256(mpq_a).hexdigest(),
                                                                              "Data/patch-X.MPQ": hashlib.sha256(mpq_b).hexdigest()}
      and inst.get("rev") == "v2" and inst.get("file_count") == 3, (out, inst))
check("pin: it waits for a review again", ew["review"]["by"] == "" and "needs a review" in ew["review"]["notes"])
code, out = run("check")
check("pin: the pinned list still checks out", code == 0, out)
code, out = run("pin", "look:epoch-water", "--url", "https://github.com/mallory/x/releases/download/v1/water.zip", "--file", zp)
check("pin: another repo's file is refused", code != 0 and "own repo" in out, out)
code, out = run("pin", "look:max", "--url", url, "--file", zp)
check("pin: only graphics packs", code != 0 and "only a graphics pack" in out, out)
bare = os.path.join(d, "patch-W.MPQ")
open(bare, "wb").write(mpq_a)
code, out = run("pin", "look:epoch-water", "--url", "https://github.com/ZythDr/EpochWater/releases/download/v3/patch-W.MPQ", "--file", bare)
inst = next(e for e in json.load(open(LIST))["looks"] if e["id"] == "look:epoch-water")["install"]
check("pin: a bare .MPQ release file is the one file", code == 0 and inst["files"] == {"patch-W.MPQ": hashlib.sha256(mpq_a).hexdigest()}
      and inst["strip_prefix"] == "", out)
junk = os.path.join(d, "x.zip")
with zipfile.ZipFile(junk, "w") as z:
    z.writestr("readme.txt", b"no patches")
code, out = run("pin", "look:epoch-water", "--url", "https://github.com/ZythDr/EpochWater/releases/download/v4/x.zip", "--file", junk)
check("pin: a zip without a client patch is refused", code != 0 and "no .MPQ" in out, out)
shutil.rmtree(d)
print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
