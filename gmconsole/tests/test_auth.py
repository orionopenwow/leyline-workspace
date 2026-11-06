"""The GM console's gatekeeping (review 4 Nov): Host check (no DNS rebinding), signed-out 401s, the login token and
its lockout, the session cookie, and every POST's Origin / CSRF / JSON / size checks; the inputs that go into SOAP
commands (no second command through a newline, bot accounts refused); destructive actions need confirm. No docker,
no SOAP: GM_DOCKER is /bin/false and nothing here reaches an action that would run.

    python3 -I gmconsole/tests/test_auth.py
"""
import http.client, importlib.util, json, os, sys, tempfile, threading

HERE = os.path.dirname(os.path.abspath(__file__))
SERVER = os.path.join(HERE, "..", "server")
PORT = 18098
TOK = "k" * 40
wu = tempfile.mkdtemp()
with open(os.path.join(wu, "gmconsole.env"), "w") as f:
    f.write("GM_TOKEN=" + TOK + "\n")
os.environ.update(GM_WU_DIR=wu, GM_PORT=str(PORT), GM_DOCKER="/bin/false", GM_BUGS_EVERY="0",
                  GM_AUDIT_FILE=os.path.join(wu, "audit.log"))
spec = importlib.util.spec_from_file_location("gm_auth", os.path.join(SERVER, "gmconsole.py"))
gm = importlib.util.module_from_spec(spec); spec.loader.exec_module(gm)
gm.time.sleep = lambda s: None        # the wrong-token pause, not needed here
srv = gm.ThreadingHTTPServer(("127.0.0.1", PORT), gm.Handler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()
fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra and not cond else ""))
    fails += 0 if cond else 1


def req(method, path, host=None, headers=None, body=None):
    c = http.client.HTTPConnection("127.0.0.1", PORT, timeout=20)
    h = {"Host": host or f"127.0.0.1:{PORT}"}
    h.update(headers or {})
    c.request(method, path, body=body, headers=h)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


# ---------------------------------------------------------------- Host, signed out, login
check("a rebinding Host is refused", req("GET", "/", host="evil.example:8099")[0] == 403)
check("a rebinding Host is refused on POST", req("POST", "/logout", host="evil.example")[0] == 403)
check("signed out: the page is 401", req("GET", "/")[0] == 401)
check("signed out: the API is 401", req("GET", "/api/overview")[0] == 401)
check("healthz needs nothing", req("GET", "/healthz")[0] == 200)
st, h, _ = req("GET", "/login?t=" + "x" * 40)
check("a wrong login token: 401, no cookie", st == 401 and "set-cookie" not in h)
st, h, _ = req("GET", "/login?t=" + TOK)
cookie = h.get("set-cookie", "")
check("the right token: 303 to / with a session cookie", st == 303 and cookie.startswith("gmc_s="))
check("the cookie is HttpOnly and SameSite=Strict", "HttpOnly" in cookie and "SameSite=Strict" in cookie)
sess = cookie.split(";")[0]
st, h, body = req("GET", "/", headers={"Cookie": sess})
check("signed in: the page", st == 200)
csrf = body.decode().split('name="csrf" content="', 1)[1].split('"', 1)[0]
check("the page carries a CSRF token", len(csrf) >= 20)
check("security headers", h.get("x-frame-options") == "DENY" and "content-security-policy" in h
      and h.get("cache-control") == "no-store")
st, _, _ = req("GET", "/api/export/new-reports")
check("export without a Bearer token: 401", st == 401)

# ---------------------------------------------------------------- POST checks
J = {"Cookie": sess, "Content-Type": "application/json", "X-CSRF": csrf}
post = lambda path, hdr=None, body=b"{}": req("POST", path, headers=dict(J, **(hdr or {})), body=body)  # noqa: E731
check("no session: 401", req("POST", "/api/action/kick", headers={"Content-Type": "application/json"}, body=b"{}")[0] == 401)
check("another site's Origin: 403", post("/api/action/kick", {"Origin": "https://evil.example"})[0] == 403)
check("no CSRF token: 403", post("/api/action/kick", {"X-CSRF": ""})[0] == 403)
check("a wrong CSRF token: 403", post("/api/action/kick", {"X-CSRF": "x" * 32})[0] == 403)
check("not JSON: 415", post("/api/action/kick", {"Content-Type": "text/plain"})[0] == 415)
check("too big: 413", post("/api/action/kick", {"Content-Length": "50000"}, b"{}")[0] == 413)
check("an array body: 400", post("/api/action/kick", body=b"[]")[0] == 400)
check("an unknown action: 400", post("/api/action/drop_tables")[0] == 400)
destr = [n for n, (_, d, _) in gm.ACTIONS.items() if d]
check("there are destructive actions", bool(destr), str(destr))
st, _, b = post(f"/api/action/{destr[0]}", body=json.dumps({}).encode())
check(f"{destr[0]} without confirm: 400", st == 400 and b"confirm" in b, b.decode()[:200])

# ---------------------------------------------------------------- what goes into a SOAP command
def bad(fn, v):
    try:
        fn(v)
        return False
    except gm.Bad:
        return True


check("char_name: two words refused", bad(gm.char_name, "Arthas announce"))
check("char_name: newline refused", bad(gm.char_name, "Bob\nserver shutdown 1"))
check("char_name: tidy case", gm.char_name("jAINA") == "Jaina")
check("acct_name: a bot account refused", bad(gm.acct_name, gm.BOT_PREFIX + "123"))
check("acct_name: spaces refused", bad(gm.acct_name, "a b"))
t = gm.text("hello\nserver shutdown 1\r\x00there")
check("text: control characters become spaces (no second command)", "\n" not in t and "\r" not in t and "\x00" not in t, repr(t))
check("text: empty refused", bad(gm.text, " \n\t "))
check("text: capped", len(gm.text("a" * 999, 120)) == 120)

# ---------------------------------------------------------------- the lockout (last: it locks the login)
for _ in range(25):
    req("GET", "/login?t=" + "y" * 40)
check("20 wrong tokens in 10 minutes: the next wrong one is 429", req("GET", "/login?t=" + "z" * 40)[0] == 429)
check("... but the right token still gets in (nothing local can lock the owner out)", req("GET", "/login?t=" + TOK)[0] == 303)
check("the audit log never holds the token", TOK not in open(os.environ["GM_AUDIT_FILE"]).read()
      if os.path.exists(os.environ["GM_AUDIT_FILE"]) else True)

print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
