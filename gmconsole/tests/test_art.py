"""The GM console's look (the Leyline Launcher's): GET /static/art/<name> (the allow-list, content types, no login
needed), the shell and the signed-out page linking leyline-theme.css, the copy of the art matching launcher/ui/art,
the brand words kept off the page, and the fallback when the art is missing.

    python3 -I gmconsole/tests/test_art.py
"""
import http.client, importlib.util, os, sys, tempfile, threading

HERE = os.path.dirname(os.path.abspath(__file__))
SERVER = os.path.join(HERE, "..", "server")
LAUNCHER_ART = os.path.join(HERE, "..", "..", "launcher", "ui", "art")
# the console's copy of each launcher file: name here -> path under launcher/ui/art
FROM_LAUNCHER = {
    "leyline-bg.webp": "leyline-bg.webp", "stone-bar.webp": "stone-bar.webp", "stone-side.webp": "stone-side.webp",
    "stone-btn.webp": "stone-btn.webp", "leyline-crystal-128.png": "brand/leyline-crystal-128.png",
    "leyline-swirl-128.png": "brand/leyline-swirl-128.png",
    "marcellus-latin-400-normal.woff2": "fonts/marcellus-latin-400-normal.woff2", "Marcellus-OFL.txt": "fonts/Marcellus-OFL.txt",
    "inter-latin-400-normal.woff2": "fonts/inter-latin-400-normal.woff2",
    "inter-latin-600-normal.woff2": "fonts/inter-latin-600-normal.woff2", "Inter-OFL.txt": "fonts/Inter-OFL.txt",
}
PORT = 18099
wu = tempfile.mkdtemp()
with open(os.path.join(wu, "gmconsole.env"), "w") as f:
    f.write("GM_TOKEN=" + "t" * 40 + "\n")
os.environ.update(GM_WU_DIR=wu, GM_PORT=str(PORT), GM_DOCKER="/bin/false")
os.environ.pop("GM_ART_DIR", None)
spec = importlib.util.spec_from_file_location("gm", os.path.join(SERVER, "gmconsole.py"))
gm = importlib.util.module_from_spec(spec); spec.loader.exec_module(gm)
srv = gm.ThreadingHTTPServer(("127.0.0.1", PORT), gm.Handler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()


def get(path, cookie=""):
    c = http.client.HTTPConnection("127.0.0.1", PORT, timeout=20)
    h = {"Host": f"127.0.0.1:{PORT}"}
    if cookie:
        h["Cookie"] = cookie
    c.request("GET", path, headers=h)
    r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read()


fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra else ""))
    fails += 0 if cond else 1


# the copy in static/art is the launcher's art, file for file, plus the theme; nothing else in the kit
art = os.path.join(SERVER, "static", "art")
diff = [n for n, src in FROM_LAUNCHER.items() if not os.path.exists(os.path.join(art, n))
        or open(os.path.join(art, n), "rb").read() != open(os.path.join(LAUNCHER_ART, src), "rb").read()]
check("static/art matches launcher/ui/art (copy it again after a launcher art change)", not diff, diff)
check("the kit is exactly the allow-list", sorted(os.listdir(art)) == sorted(gm.ART_TYPES), sorted(set(os.listdir(art)) ^ set(gm.ART_TYPES)))
check("the allow-list is the launcher copies plus leyline-theme.css", sorted(gm.ART_TYPES) == sorted(list(FROM_LAUNCHER) + ["leyline-theme.css"]))
check("no old WrathUnleashed art left", not any(n.startswith("wu-") or n.startswith("Cinzel") for n in gm.ART_TYPES))
fonts = [n for n in gm.ART_TYPES if n.endswith(".woff2")]
check("every font ships with its OFL text", all(any(n.lower().startswith(o.split("-")[0].lower()) for o in gm.ART_TYPES if o.endswith("-OFL.txt")) for n in fonts), fonts)
theme = open(os.path.join(art, "leyline-theme.css"), encoding="utf-8").read()
import re
urls = set(re.findall(r'url\("([^"]+)"\)', theme))
check("leyline-theme.css only points at allow-listed art (relative)", urls and all(u in gm.ART_TYPES for u in urls), urls - set(gm.ART_TYPES))
check("leyline-theme.css: Marcellus, Inter, the red stone, the backdrop and the mark",
      all(k in urls for k in ("marcellus-latin-400-normal.woff2", "inter-latin-400-normal.woff2", "stone-btn.webp",
                              "leyline-bg.webp", "leyline-crystal-128.png")))
check("no third-party fonts or images in the CSS", "http:" not in theme and "https:" not in theme and "//" not in theme.replace("://", ""))

for name, ctype in (("leyline-theme.css", "text/css"), ("leyline-bg.webp", "image/webp"), ("stone-btn.webp", "image/webp"),
                    ("leyline-crystal-128.png", "image/png"), ("marcellus-latin-400-normal.woff2", "font/woff2"),
                    ("inter-latin-600-normal.woff2", "font/woff2"), ("Inter-OFL.txt", "text/plain")):
    st, h, b = get("/static/art/" + name)
    check(f"/static/art/{name}: 200 {ctype} without a login", st == 200 and h.get("content-type", "").startswith(ctype)
          and b == open(os.path.join(art, name), "rb").read(), (st, h.get("content-type")))
st, h, b = get("/static/art/leyline-bg.webp")
check("art: cached, one Cache-Control header", h.get("cache-control") == "private, max-age=604800", h.get("cache-control"))
for bad in ("wu-theme.css", "Cinzel-SemiBold.woff2", "../gmconsole.py", "..%2fgmconsole.py", "%2e%2e%2f%2e%2e%2fgmconsole.py", "..%2f..%2fgmconsole.env", "app.js", "x.css", ""):
    st, h, b = get("/static/art/" + bad)
    check(f"art: {bad!r} refused", st == 404 and b"import " not in b and b"GM_TOKEN" not in b, st)
st, h, b = get("/static/app.js")
check("app.js still needs the login", st == 401)

st, h, b = get("/")
check("signed out: themed page", st == 401 and b"/static/art/leyline-theme.css" in b and b"Open GM Console.bat" in b
      and b'class="ly"' in b and b"Leyline GM console" in b and b'class="ly-mark"' in b)
check("signed out: no inline styles (the CSP blocks them)", b"style=" not in b)
check("CSP allows our own fonts", "font-src 'self'" in h.get("content-security-policy", ""))
st, h, b = get("/login?t=" + "t" * 40)
cookie = (h.get("set-cookie") or "").split(";")[0]
st, h, b = get("/", cookie)
check("signed in: the shell links the theme before app.css", st == 200 and 0 < b.find(b"leyline-theme.css") < b.find(b"app.css")
      and b'<body class="ly">' in b, st)
check("signed in: titled Leyline GM console, the crystal as the tab icon", b"<title>Leyline GM console</title>" in b
      and b'href="/static/art/leyline-crystal-128.png"' in b)
js = open(os.path.join(SERVER, "static", "app.js"), encoding="utf-8").read()
check("app.js: the brand is the mark, Leyline and GM console", "class: 'ly-mark'" in js and "text: 'Leyline'" in js and "WrathUnleashed'" not in js)
BRAND = re.compile(r"world of warcraft|\bwow\b|warcraft|lich king|blizzard", re.I)
for n in ("app.js", "app.css", os.path.join("art", "leyline-theme.css")):
    t = open(os.path.join(SERVER, "static", n), encoding="utf-8").read()
    check(f"brand words: none in {n}", not BRAND.search(t), BRAND.findall(t))
check("brand words: none in the shell or the signed-out page", not BRAND.search(gm.SHELL) and not BRAND.search(gm.NOAUTH.decode()))

gm.ART_DIR = os.path.join(tempfile.mkdtemp(), "missing")
st, h, b = get("/static/art/leyline-theme.css")
check("missing art: 404", st == 404)
st, h, b = get("/", cookie)
check("missing art: the console still loads with app.css", st == 200 and b"/static/app.css" in b)
st, h, b = get("/static/app.css")
check("app.css is served (no login needed: styles only)", st == 200 and h.get("content-type", "").startswith("text/css"))

print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
