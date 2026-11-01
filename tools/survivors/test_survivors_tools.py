"""The owner-PC tools for Azeroth Survivors, with a stand-in game host (no game files anywhere):

  * survivors_rec.py: passes everything through, records each /data name once (any case) as served or missing,
    no-store on /data, the recorder page and its plan, loopback only;
  * survivors_export.py: exactly the recorded names into a loose-file slice (one folder per name in any case),
    names.sha256, the franchise logo and non-plain names left out, the last good slice kept on a failed run;
  * survivors_check.py: slice against names.sha256 (missing, changed, extra, never-allowed), dist against the lock;
  * the .bat: CRLF, never the data-fetching upstream scripts, nothing from labs/.

    python3 -I tools/survivors/test_survivors_tools.py
"""
import hashlib, http.client, importlib.util, json, os, shutil, subprocess, sys, tempfile, threading, time, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra else ""))
    fails += 0 if cond else 1


def load(name):
    spec = importlib.util.spec_from_file_location(name, os.path.join(HERE, name + ".py"))
    m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
    return m


rec, exp, chk = load("survivors_rec"), load("survivors_export"), load("survivors_check")
T = tempfile.mkdtemp(prefix="sv-tools-")
FILES = {"dbfilesclient\\map.dbc": b"WDBC-map", "creature\\wolf\\wolf.m2": b"MD20-wolf",
         "interface\\glues\\common\\glues-wow-logo.blp": b"BLP2-logo", "sound\\x y\\hit (1).wav": b"RIFF"}


class Fake(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a): pass

    def do_GET(self):
        p = self.path.split("?", 1)[0]
        hd = {"Cross-Origin-Opener-Policy": "same-origin", "Cross-Origin-Embedder-Policy": "require-corp"}
        if p == "/index.html" or p == "/":
            body, st = b"<!doctype html><title>Azeroth Survivors</title>", 200
        elif p.startswith("/data/"):
            n = urllib.parse.unquote(p[6:]).replace("/", "\\").lower()
            body = json.dumps(sorted(FILES)).encode() if n == "__index" else FILES.get(n, b"")
            st = 200 if body else 404
            hd["Cache-Control"] = "public, max-age=31536000, immutable"
        else:
            body, st = b"", 404
        self.send_response(st)
        for k, v in hd.items():
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    do_HEAD = do_GET


up = ThreadingHTTPServer(("127.0.0.1", 0), Fake)
threading.Thread(target=up.serve_forever, daemon=True).start()
UP = f"http://127.0.0.1:{up.server_address[1]}"

# ------------------------------------------------------------------ the recorder
out = os.path.join(T, "out")
log = rec.Log(out)
done = threading.Event()
srv = ThreadingHTTPServer(("127.0.0.1", 0), rec.make_handler(UP, log, rec.plan("full"), 80, done, out))
threading.Thread(target=srv.serve_forever, daemon=True).start()
RP = srv.server_address[1]


def get(path, method="GET", body=None):
    c = http.client.HTTPConnection("127.0.0.1", RP, timeout=10)
    c.request(method, path, body=body)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


st, h, b = get("/index.html")
check("rec: the page passes through, with its isolation headers", st == 200 and b"Azeroth Survivors" in b
      and h.get("cross-origin-embedder-policy") == "require-corp")
st, h, b = get("/data/DBFilesClient%5CMap.dbc")
check("rec: a game file passes through, bytes unchanged", st == 200 and b == b"WDBC-map")
check("rec: /data goes back no-store (the browser cache never hides a name)", h.get("cache-control") == "no-store")
get("/data/dbfilesclient%5Cmap.DBC")
get("/data/Creature/Wolf/Wolf.m2")
get("/data/Interface%5CGlues%5CCommon%5CGlues-WoW-Logo.blp")
st, _, _ = get("/data/World%5CMaps%5CNope.adt")
check("rec: a name the client lacks passes the 404 through", st == 404)
get("/data/__index")
get("/data/Creature%5CWolf%5CWolf.m2", "HEAD")
names = open(os.path.join(out, "recorded.txt"), encoding="utf-8").read().split("\n")[:-1]
check("rec: each served name once, in any case, first spelling kept, backslashes",
      names == ["DBFilesClient\\Map.dbc", "Creature\\Wolf\\Wolf.m2", "Interface\\Glues\\Common\\Glues-WoW-Logo.blp"], names)
check("rec: the missing name in missing.txt", open(os.path.join(out, "missing.txt")).read() == "World\\Maps\\Nope.adt\n")
check("rec: __index is not a game file", "__index" not in open(os.path.join(out, "recorded.txt")).read()
      and "__index" in open(os.path.join(out, "requests.log")).read())
st, h, b = get("/__rec/")
check("rec: the recorder page, isolated, with the plan", st == 200 and b"survivors_auto=Mage:late&survivors_map=" in b
      and h.get("cross-origin-opener-policy") == "same-origin" and b"__PLAN__" not in b)
st, _, b = get("/__rec/stats")
check("rec: stats", json.loads(b)["names"] == 3 and json.loads(b)["missing"] == 1)
full = rec.plan("full")
qs = [p["q"] for p in full if p["q"]]
check("rec: the full plan covers all 16 zones and all 9 classes, the results screen and a hand pass",
      all(any(f"survivors_map={m}" in q and ":late" in q for q in qs) for m in range(16))
      and all(any(q.startswith(f"survivors_auto={c}:all") for q in qs) for c in rec.CLASSES)
      and any(":idle" in q for q in qs) and full[-1].get("hand"))
check("rec: the quick plan is short", len(rec.plan("quick")) == 6)
get("/__rec/note", "POST", json.dumps({"done": True, "evil": "x" * 10}).encode())
check("rec: the page's done ends the session; notes keep only known fields", done.is_set()
      and "evil" not in open(os.path.join(out, "passes.json")).read())
log2 = rec.Log(out)
check("rec: a second session starts from what is there (names add up)", len(log2.seen["recorded.txt"]) == 3)
r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "survivors_rec.py"), "--listen", "0.0.0.0:18999", "--out", out],
                   capture_output=True, text=True, timeout=20)
check("rec: refuses to listen beyond this PC", r.returncode == 2)
check("rec: --wait-only sees the host", rec.wait_for(UP, 5) and not rec.wait_for("http://127.0.0.1:1", 1))

# ------------------------------------------------------------------ the exporter
lst = os.path.join(T, "extra.txt")
with open(lst, "w") as f:
    f.write("# a comment\nSound\\X Y\\Hit (1).wav\n..\\..\\secret\nC:\\x\\y\n/abs\\path\ncreature\\WOLF\\wolf.m2\n.hidden\\x\n")
r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "survivors_export.py"), os.path.join(out, "recorded.txt"), lst,
                    "--out", out, "--host", UP], capture_output=True, text=True, timeout=60)
check("exp: runs", r.returncode == 0 and "EXPORT OK: 3 files" in r.stdout, r.stdout + r.stderr)
sl = os.path.join(out, "slice")
have = sorted(os.path.relpath(os.path.join(d, f), sl).replace(os.sep, "/") for d, _, fs in os.walk(sl) for f in fs)
check("exp: exactly the recorded files, folders per name, first spelling", have == ["Creature/Wolf/Wolf.m2", "DBFilesClient/Map.dbc",
                                                                                   "Sound/X Y/Hit (1).wav"], have)
check("exp: the franchise logo is left out", not any("logo" in x.lower() for x in have))
sums = open(os.path.join(out, "names.sha256"), encoding="utf-8").read()
check("exp: names.sha256, sorted, sha256 + two spaces + name", sums.splitlines() == [
    f"{hashlib.sha256(b'MD20-wolf').hexdigest()}  Creature\\Wolf\\Wolf.m2",
    f"{hashlib.sha256(b'WDBC-map').hexdigest()}  DBFilesClient\\Map.dbc",
    f"{hashlib.sha256(b'RIFF').hexdigest()}  Sound\\X Y\\Hit (1).wav"], sums)
rep = json.load(open(os.path.join(out, "export.json")))
check("exp: the report: left out, not refused silently", rep["left_out"] == ["Interface\\Glues\\Common\\Glues-WoW-Logo.blp"]
      and len(rep["refused_names"]) == 4 and rep["files"] == 3)
for bad in ("..\\x", "a\\..\\b", "C:\\x", "\\x", "a\\\\b", ".git\\config", "a\\b.", "a\\<b>", ""):
    check(f"exp: {bad!r} is not a plain game path", exp.clean(bad) is None)
check("exp: slashes become backslashes", exp.clean("World/Maps/x.adt") == "World\\Maps\\x.adt")
r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "survivors_export.py"), os.path.join(out, "recorded.txt"),
                    "--out", out, "--host", "http://127.0.0.1:1"], capture_output=True, text=True, timeout=60)
check("exp: a host that is gone fails, and the last good slice stays", r.returncode == 1 and os.path.isfile(os.path.join(sl, "DBFilesClient", "Map.dbc")))
r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "survivors_export.py"), lst, "--out", out, "--host", "http://example.com"],
                   capture_output=True, text=True, timeout=20)
check("exp: only the host on this PC", r.returncode != 0)

# ------------------------------------------------------------------ the check
r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "survivors_check.py"), "slice", sl, os.path.join(out, "names.sha256")],
                   capture_output=True, text=True)
check("chk: the fresh slice passes", r.returncode == 0 and r.stdout.strip().endswith("SURVIVORS CHECK OK"), r.stdout)
open(os.path.join(sl, ".export.json"), "w").write("{}")
check("chk: dot files are not the slice's", chk.check_slice(sl, os.path.join(out, "names.sha256")) == [])
with open(os.path.join(sl, "DBFilesClient", "Map.dbc"), "ab") as f:
    f.write(b"!")
os.makedirs(os.path.join(sl, "Interface", "Glues", "Common"))
open(os.path.join(sl, "Interface", "Glues", "Common", "Glues-WoW-Logo.blp"), "wb").write(b"x")
os.remove(os.path.join(sl, "Sound", "X Y", "Hit (1).wav"))
bad = chk.check_slice(sl, os.path.join(out, "names.sha256"))
check("chk: changed, extra, missing and never-allowed files all found",
      any("changed" in b and "map.dbc" in b for b in bad) and any("missing" in b and "hit (1).wav" in b for b in bad)
      and any("not in names.sha256" in b and "logo" in b for b in bad) and any("brand rule" in b for b in bad), bad)
dist = os.path.join(T, "dist")
os.makedirs(os.path.join(dist, "licenses"))
for n, b in (("index.html", b"<html>"), ("wenilla_bg.wasm", b"\0asm"), ("licenses/LICENSE-MIT", b"MIT"),
             ("licenses/LICENSE-APACHE", b"Apache")):
    open(os.path.join(dist, n), "wb").write(b)
lock = os.path.join(T, "lock.json")
json.dump({"repo": "https://github.com/OutThisLife/azeroth-survivors-re", "sha": "a" * 40, "dist": {"files": {
    n: chk.sha256(os.path.join(dist, n)) for n in ("index.html", "wenilla_bg.wasm", "licenses/LICENSE-MIT", "licenses/LICENSE-APACHE")}}},
          open(lock, "w"))
check("chk: the locked dist passes", chk.check_dist(dist, lock) == [])
open(os.path.join(dist, "evil.js"), "w").write("x")
open(os.path.join(dist, "index.html"), "a").write("x")
bad = chk.check_dist(dist, lock)
check("chk: a changed or extra dist file fails", any("not the locked build: index.html" in b for b in bad)
      and any("not in the lock: evil.js" in b for b in bad))
real = os.path.join(HERE, "..", "..", "stream", "survivors", "upstream.lock.json")
lk = json.load(open(real))
check("chk: the repo's lock names the upstream repo and a full sha", lk["repo"] == "https://github.com/OutThisLife/azeroth-survivors-re"
      and len(lk["sha"]) == 40)

# ------------------------------------------------------------------ the fetch (the CI release, every hash checked)
import io, tarfile
rel = os.path.join(T, "release"); os.makedirs(rel)
bd = os.path.join(T, "built"); os.makedirs(os.path.join(bd, "licenses"))
for n, b in (("index.html", b"<html>"), ("wenilla_bg.wasm", b"\0asm1"), ("licenses/LICENSE-MIT", b"MIT"), ("licenses/LICENSE-APACHE", b"Apache")):
    open(os.path.join(bd, n), "wb").write(b)
rlock = {"repo": "https://github.com/OutThisLife/azeroth-survivors-re", "sha": "c" * 40,
         "dist": {"files": {n: chk.sha256(os.path.join(bd, n)) for n in ("index.html", "wenilla_bg.wasm", "licenses/LICENSE-MIT", "licenses/LICENSE-APACHE")}}}
buf = io.BytesIO()
with tarfile.open(fileobj=buf, mode="w:gz") as t:
    t.add(bd, arcname=".")
assets = {"survivors-dist.tgz": buf.getvalue(), "upstream.lock.json": json.dumps(rlock).encode(),
          "wenilla-host-windows-x64.exe": b"MZ-host", "mpqx-windows-x64.exe": b"MZ-mpqx"}


def publish(a):
    for n, b in a.items():
        open(os.path.join(rel, n), "wb").write(b)
        open(os.path.join(rel, n + ".sha256"), "w").write(hashlib.sha256(b).hexdigest() + "  " + n + "\n")


publish(assets)


class Rel(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_GET(self):
        p = os.path.join(rel, os.path.basename(self.path))
        b = open(p, "rb").read() if os.path.isfile(p) else None
        self.send_response(200 if b is not None else 404)
        self.send_header("Content-Length", str(len(b or b"")))
        self.end_headers()
        self.wfile.write(b or b"")


rs = ThreadingHTTPServer(("127.0.0.1", 0), Rel)
threading.Thread(target=rs.serve_forever, daemon=True).start()
BASE = f"http://127.0.0.1:{rs.server_address[1]}/"
into = os.path.join(T, "pc"); os.makedirs(into)
mylock = os.path.join(T, "mylock.json")


def fetch(*extra):
    r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "survivors_fetch.py"), "--base", BASE, "--lock", mylock,
                        "--into", into, *extra], capture_output=True, text=True, timeout=60)
    return r.returncode, r.stdout + r.stderr


json.dump({"repo": rlock["repo"], "sha": "d" * 40, "dist": {}}, open(mylock, "w"))
rc, out = fetch()
check("fetch: refuses a release built from another commit", rc == 1 and "not the locked" in out, out[-300:])
json.dump({"repo": rlock["repo"], "sha": "c" * 40, "deviations": [], "dist": {}}, open(mylock, "w"))
rc, out = fetch()
check("fetch: downloads, checks, unpacks", rc == 0 and out.rstrip().endswith("FETCH OK: tools\\survivors\\dist and tools\\survivors\\bin are the locked build"), out[-400:])
check("fetch: an empty repo lock takes the release's lock (same repo and commit)", json.load(open(mylock))["dist"] == rlock["dist"])
check("fetch: the build and the hosts in place", chk.check_dist(os.path.join(into, "dist"), mylock) == []
      and open(os.path.join(into, "bin", "wenilla-host.exe"), "rb").read() == b"MZ-host"
      and open(os.path.join(into, "bin", "mpqx.exe"), "rb").read() == b"MZ-mpqx")
rc, out = fetch()
check("fetch: again: nothing downloaded twice", rc == 0 and "already here" in out)
open(os.path.join(rel, "wenilla-host-windows-x64.exe"), "wb").write(b"MZ-evil")
rc, out = fetch("--force")
check("fetch: a file that does not match its .sha256 is refused", rc == 1 and "does not match its .sha256" in out)
check("fetch: ... and the host already here stays", open(os.path.join(into, "bin", "wenilla-host.exe"), "rb").read() == b"MZ-host")
other = dict(rlock, dist={"files": dict(rlock["dist"]["files"], **{"index.html": "0" * 64})})
publish(dict(assets, **{"upstream.lock.json": json.dumps(other).encode()}))
rc, out = fetch()
check("fetch: a release build that differs from the repo's lock is refused", rc == 1 and "not the one" in out)
buf2 = io.BytesIO()
with tarfile.open(fileobj=buf2, mode="w:gz") as t:
    ti = tarfile.TarInfo("../evil.txt"); ti.size = 1; t.addfile(ti, io.BytesIO(b"x"))
publish(dict(assets, **{"survivors-dist.tgz": buf2.getvalue()}))
rc, out = fetch("--force")
check("fetch: an archive path outside the folder is refused", rc == 1 and "unsafe path" in out
      and not os.path.exists(os.path.join(into, "evil.txt")))
r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "survivors_fetch.py"), "--base", "http://example.com/", "--lock", mylock,
                    "--into", into], capture_output=True, text=True, timeout=30)
check("fetch: https only (loopback aside)", r.returncode == 1 and "https only" in r.stdout + r.stderr)

# ------------------------------------------------------------------ the .bat
for bat in [f for f in os.listdir(HERE) if f.endswith(".bat")]:
    b = open(os.path.join(HERE, bat), "rb").read()
    check(f"{bat}: CRLF only", b.count(b"\n") == b.count(b"\r\n") > 10)
    low = b.decode("ascii").lower()
    check(f"{bat}: never the upstream scripts that download the game client", not any(
        x in low for x in ("fetch-data", "make build", "make native", "scripts\\setup.sh", "scripts/setup.sh", "archive.org")))
    check(f"{bat}: nothing from labs", "labs\\" not in low and "labs/" not in low)
    check(f"{bat}: loopback only", "0.0.0.0" not in low and "127.0.0.1:18090" in low)
    check(f"{bat}: downloads the checked build, never builds with Rust", "survivors_fetch.py" in low and "cargo" not in low)

shutil.rmtree(T, ignore_errors=True)
print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
