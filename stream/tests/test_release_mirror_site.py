"""Our mirror of Leyline's releases on the site (docs/RESILIENCE.md): GET /wrath/leyline/releases/<path> streams the
files tools/release-mirror put in $WU_LEYLINE_DIR/releases, only those its index.json lists, at the listed size;
/wrath/leyline/chain.json is the release log. Nothing else is reachable from there.

    python3 -I stream/tests/test_release_mirror_site.py stream/wrath_portal.py
"""
import hashlib, http.client, importlib.util, json, os, sys, tempfile, threading
from http.server import ThreadingHTTPServer

D = tempfile.mkdtemp()
REL = os.path.join(D, "leyline", "releases")
os.makedirs(os.path.join(REL, "launcher-v0.3.4"))
os.makedirs(os.path.join(REL, "sources"))
BIG = bytes((i * 31) % 251 for i in range(3 * 1024 * 1024 + 17))       # bigger than one 1 MB piece
files = {"launcher-v0.3.4/Leyline_0.3.4_x64-setup.exe": BIG, "latest.json": b'{"version":"0.3.4","platforms":{}}\n',
         "sources/core-abc.tar.gz": b"\x1f\x8b fake", "launcher-v0.3.4/unlisted.exe": b"MZ not listed",
         "sources/half.tar.gz": b"\x1f\x8b half"}
for p, b in files.items():
    with open(os.path.join(REL, *p.split("/")), "wb") as f:
        f.write(b)
idx = {p: {"size": len(b), "sha256": hashlib.sha256(b).hexdigest()} for p, b in files.items() if "unlisted" not in p}
idx["sources/half.tar.gz"]["size"] = 999                                # still being copied: not served
with open(os.path.join(REL, "index.json"), "w") as f:
    json.dump({"format": 1, "files": idx}, f)
with open(os.path.join(REL, "chain.json"), "w") as f:
    f.write('{"format":1,"key":"","entries":[]}')
with open(os.path.join(D, "outside.txt"), "w") as f:
    f.write("secret-ish")

os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example",
                  WU_RELAY_LISTING="http://127.0.0.1:9/realms.json", WU_LEYLINE_DIR=os.path.join(D, "leyline"))
spec = importlib.util.spec_from_file_location("wp", sys.argv[1])
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
srv.daemon_threads = True
PORT = srv.server_address[1]
threading.Thread(target=srv.serve_forever, daemon=True).start()


def get(path, method="GET"):
    c = http.client.HTTPConnection("127.0.0.1", PORT, timeout=20)
    c.request(method, path, headers={"X-Forwarded-For": "10.9.0.1"})
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra else ""))
    fails += 0 if cond else 1


st, h, b = get("/wrath/leyline/releases/launcher-v0.3.4/Leyline_0.3.4_x64-setup.exe")
check("an installer: 200, every byte, streamed", st == 200 and b == BIG and h.get("content-length") == str(len(BIG)), (st, len(b)))
check("public and cross-site", h.get("access-control-allow-origin") == "*" and "public" in h.get("cache-control", ""))
check("an installer is cached a day", h.get("cache-control") == "public, max-age=86400", h.get("cache-control"))
st, h, b = get("/wrath/leyline/releases/launcher-v0.3.4/Leyline_0.3.4_x64-setup.exe", "HEAD")
check("HEAD: no body", st == 200 and b == b"" and h.get("content-length") == str(len(BIG)))
st, h, b = get("/wrath/leyline/releases/latest.json")
check("latest.json: JSON, 5 minutes", st == 200 and h.get("content-type", "").startswith("application/json") and h.get("cache-control") == "public, max-age=300", h)
st, h, b = get("/wrath/leyline/releases/index.json")
check("index.json is public", st == 200 and json.loads(b)["files"])
st, h, b = get("/wrath/leyline/chain.json")
check("chain.json at /wrath/leyline/chain.json", st == 200 and json.loads(b)["format"] == 1, st)
st, h, b = get("/wrath/leyline/releases/chain.json")
check("and beside the releases", st == 200)
for bad in ("launcher-v0.3.4/unlisted.exe", "sources/half.tar.gz", "../outside.txt", "..%2Foutside.txt", "sources/../../outside.txt",
            "nope.json", "sources/core.MPQ", "other/x.zip", "launcher-v0.3.4/", "", "sources%2Fcore-abc.tar.gz"):
    st, h, b = get("/wrath/leyline/releases/" + bad)
    check(f"refused: {bad or '(empty)'}", st == 404 and b"secret-ish" not in b, st)
st, h, b = get("/wrath/leyline/releases/sources/core-abc.tar.gz")
check("a source tarball", st == 200 and b == files["sources/core-abc.tar.gz"])
st, h, b = get("/wrath/leyline/flags.json")
check("flags.json still answers", st == 200 and "share_own_files" in json.loads(b)["flags"], st)

print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
