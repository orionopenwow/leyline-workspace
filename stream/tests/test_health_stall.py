"""No page waits on a slow worldserver (audit 12 Nov, finding 2; the 4 Nov speed audit's Q3).

A fake SOAP endpoint that takes SLOW seconds to answer "server info". Before the fix, health() held its lock across
the SOAP call: the first /wrath/leyline took ~14.5 s (lock wait + SOAP_TIMEOUT) and every request behind it queued.
Now SOAP is asked in a background thread: the front page, /wrath/healthz and /wrath/status.json answer at once (the
first call after a start waits at most HEALTH_FIRST_WAIT / STATUS_FIRST_WAIT), and the answer shows up once SOAP does.

    python3 -I stream/tests/test_health_stall.py stream/wrath_portal.py
"""
import http.client, importlib.util, json, os, sys, tempfile, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

SLOW = 7.0
calls = []


class SlowSoap(BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length") or 0))
        calls.append(time.monotonic())
        time.sleep(SLOW)
        body = (b'<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" '
                b'xmlns:ns1="urn:AC"><SOAP-ENV:Body><ns1:executeCommandResponse><result>'
                b"Connected players: 3 (peak: 5).\nCharacters in world: 9 (peak: 9, lifetime: 9).\nServer uptime: 1 hour(s)"
                b"</result></ns1:executeCommandResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>")
        try:
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        except OSError:
            pass

    def log_message(self, *a):
        pass


soap = ThreadingHTTPServer(("127.0.0.1", 0), SlowSoap)
soap.daemon_threads = True
threading.Thread(target=soap.serve_forever, daemon=True).start()

TMP = tempfile.mkdtemp(prefix="wu-stall-")
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL=f"http://127.0.0.1:{soap.server_address[1]}/",
                  PORTAL_URL="http://127.0.0.1:9", WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example",
                  WU_PORTAL_DATA=TMP, WU_DL_PROBE="0", WU_HUB_FRONT="1", SOAP_TIMEOUT="8",
                  WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json")
spec = importlib.util.spec_from_file_location("wp", sys.argv[1] if len(sys.argv) > 1 else
                                              os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "wrath_portal.py"))
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, extra)

srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()

def get(path):
    t = time.monotonic()
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=30)
    c.request("GET", path, headers={"X-Forwarded-For": "203.0.113.9"}); r = c.getresponse()
    body = r.read()
    return r.status, body, time.monotonic() - t

# 1. the very first front page after a start, with five visitors at once: nobody waits on the 7 s SOAP
res = [None] * 5
def visit(i):
    res[i] = get("/wrath/leyline")
ths = [threading.Thread(target=visit, args=(i,)) for i in range(5)]
t0 = time.monotonic()
for t in ths: t.start()
for t in ths: t.join()
worst = max(r[2] for r in res)
print(f"  first front page, 5 at once, SOAP {SLOW:.0f} s: worst {worst*1000:.0f} ms (before the fix: ~14,500 ms)")
check("first front page: every visitor answered (200)", all(r[0] == 200 for r in res), [r[0] for r in res])
check("first front page: nobody waits on SOAP (worst < 3.5 s; was ~14.5 s)", worst < 3.5, f"{worst:.1f}s")

# 2. healthz and status.json while SOAP is still busy: at once, and honest ("checking", not up)
st, body, took = get("/wrath/healthz")
check("healthz while SOAP is slow: answers at once", took < 1.0, f"{took:.2f}s")
check("healthz before the first answer: 503, not a made-up ok", st == 503, (st, body))
st, body, took = get("/wrath/status.json")
check("status.json while SOAP is slow: answers within the first wait", took < 2.5, f"{took:.2f}s")
doc = json.loads(body)
check("status.json before the first answer: says checking, not online", doc.get("checking") is True and doc.get("online") is False, doc)

# 3. one SOAP call at a time for health (no pile-up), and the answer arrives
time.sleep(SLOW + 1.5)
ok, kind = wp.health()
check("after SOAP answers: health is ok", ok and kind == "", (ok, kind))
st, body, took = get("/wrath/healthz")
check("healthz after: 200 at once", st == 200 and took < 1.0, (st, took))
doc = json.loads(get("/wrath/status.json")[1])
check("status.json after: online with the numbers", doc.get("online") is True and doc.get("players") == 3 and "checking" not in doc, doc)
n_before = len(calls)

# 4. stale again (the TTL passed): the page still answers at once, with the last answer, while one refresh runs
with wp._health_lock:
    wp._health["at"] = -1e9
with wp._status_lock:
    wp._status["at"] = -1e9
res = [None] * 4
ths = [threading.Thread(target=visit, args=(i,)) for i in range(4)]
for t in ths: t.start()
for t in ths: t.join()
worst = max(r[2] for r in res)
print(f"  stale front page, 4 at once: worst {worst*1000:.0f} ms")
check("stale: the front page never waits on SOAP (worst < 1.5 s)", worst < 1.5, f"{worst:.2f}s")
ok, kind = wp.health()
check("stale: health serves the last answer while it refreshes", ok, (ok, kind))
time.sleep(0.3)
check("stale: one refresh for health and one for status, not one per visitor", len(calls) - n_before <= 2, len(calls) - n_before)

print(f"\n{passed} passed, {failed} failed")
sys.exit(1 if failed else 0)
