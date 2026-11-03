"""A community's own brand file (stream/brandfile.py, WU_BRAND_FILE; tools/commons "Make my own Leyline").

  1. brandfile.py on its own: the checks (brand words, money links, colours, logo names), the tokens and rename().
  2. The portal WITHOUT WU_BRAND_FILE: BRAND_FILE is None and pages carry no brand-file style (the official site is
     unchanged).
  3. The portal WITH a brand file: the name in what people read, the colours after the site's own, the logo as the
     mark and the tab icon, the Discord link, addresses and scripts untouched, the logo served from the art list.

    python3 -I stream/tests/test_brandfile.py stream/wrath_portal.py
"""
import http.client, importlib.util, json, os, re, shutil, subprocess, sys, tempfile, threading
from http.server import ThreadingHTTPServer

PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
TMP = tempfile.mkdtemp(prefix="wu-brandfile-")

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, str(extra)[:600])


spec = importlib.util.spec_from_file_location("bf", os.path.join(STREAM, "brandfile.py"))
bf = importlib.util.module_from_spec(spec); spec.loader.exec_module(bf)

# ---------------------------------------------------------------- 1. brandfile.py
GOOD = {"format": 1, "name": "Emberline", "colours": {"bg": "#201010", "text": "#fff0e0", "accent": "#e0602a", "accent_strong": "#f08040"},
        "logo": "brand-logo.png", "links": {"site": "https://emberline.example.org", "discord": "https://discord.gg/abcDEF12", "reddit": ""}}
check("a good brand file has no problems", bf.problems(GOOD) == [], bf.problems(GOOD))
for label, change, want in [
    ("brand words in the name", {"name": "Warcraft Friends"}, "maker"),
    ("WoW in the name", {"name": "wow realm"}, "maker"),
    ("a money link", {"links": {"site": "https://patreon.com/x"}}, "money"),
    ("a link kind that isn't allowed", {"links": {"donate": "https://example.org"}}, "only site"),
    ("a Discord link that isn't an invite", {"links": {"discord": "https://example.org/discord"}}, "discord"),
    ("a bad colour", {"colours": {"bg": "red", "text": "#fff0e0", "accent": "#e0602a", "accent_strong": "#f08040"}}, "colours.bg"),
    ("a logo path, not the kit's logo name", {"logo": "../../etc/passwd"}, "logo"),
    ("markup in the name", {"name": "<b>x</b>"}, "name"),
    ("an empty name", {"name": " "}, "name"),
]:
    b = dict(GOOD, **change)
    p = bf.problems(b)
    check(f"refused: {label}", any(want in x for x in p), p)
check("not an object", bf.problems([]) != [])

path = os.path.join(TMP, "brand.json")
json.dump(GOOD, open(path, "w"))
b = bf.load(path)
check("load(): the file, filled in", b and b["name"] == "Emberline" and b["links"]["discord"].endswith("abcDEF12") and b["links"]["reddit"] == "")
check("load(): unset, missing and refused are None", bf.load("") is None and bf.load(os.path.join(TMP, "nope.json")) is None)
bad = os.path.join(TMP, "bad.json"); json.dump(dict(GOOD, name="Blizzard Hub"), open(bad, "w"))
check("load(): a refused file is None (the site stays as it is)", bf.load(bad) is None)
open(os.path.join(TMP, "junk.json"), "w").write("{not json")
check("load(): broken JSON is None", bf.load(os.path.join(TMP, "junk.json")) is None)

t = bf.tokens(b)
check("tokens: the four colours as given, surfaces between bg and text", t["--lk-bg"] == "#201010" and t["--lk-accent"] == "#e0602a"
      and t["--lk-text"] == "#fff0e0" and t["--lk-surface"] not in ("#201010", "#fff0e0") and all(re.match(r"^(#[0-9a-f]{6}|rgba\()", v) for v in t.values()), t)
check("mix(): ends and middle", bf.mix("#000000", "#ffffff", 0) == "#000000" and bf.mix("#000000", "#ffffff", 1) == "#ffffff"
      and bf.mix("#000000", "#ffffff", .5) == "#808080")
c = bf.css(b)
check("css(): tokens, the light scheme too, and the logo as the mark", "--lk-accent:#e0602a" in c and "prefers-color-scheme:light" in c
      and "url(/wrath/art/brand-logo.png)" in c and ".ly-spin::before{display:none" in c, c[:300])
check("css(): no logo, no mark rule", ".ly-spin" not in bf.css(dict(b, logo="")))

page = ('<html><head><title>Leyline Hub</title></head><body><h1 title="Leyline">Leyline realms</h1>'
        '<a href="https://leylinelauncher.com/wrath/leyline" class="ly-brand">Leyline</a> <script>var n="Leyline"</script>'
        '<p>Leyline-Launcher LeylineGate leyline.json Leyline.</p></body></html>')
r = bf.rename(page, "Emberline")
check("rename(): text and visible attributes", "<title>Emberline Hub</title>" in r and 'title="Emberline"' in r
      and ">Emberline realms<" in r and ">Emberline</a>" in r and "<p>" in r and "Emberline.</p>" in r, r)
check("rename(): addresses, classes, scripts and identifiers untouched", "https://leylinelauncher.com/wrath/leyline" in r
      and 'class="ly-brand"' in r and 'var n="Leyline"' in r and "Leyline-Launcher" in r and "LeylineGate" in r and "leyline.json" in r, r)
check("rename(): JSON values, not addresses", json.loads(bf.rename('{"name":"Leyline","url":"https://x.org/Leyline"}', "Emberline", "application/json"))
      == {"name": "Emberline", "url": "https://x.org/Leyline"})
check("rename(): the name Leyline changes nothing", bf.rename(page, "Leyline") == page)
out = bf.apply(b, page.encode(), "text/html; charset=utf-8").decode()
check("apply(): the style once, before </head>, and the logo as the tab icon", out.count('id="brand-file"') == 1
      and out.index('id="brand-file"') < out.index("</head>") and 'rel="icon" type="image/png" href="/wrath/art/brand-logo.png"' in out, out[:400])
check("apply(): twice is the same as once", bf.apply(b, out.encode(), "text/html").decode().count('id="brand-file"') == 1)
check("apply(): other types and no brand untouched", bf.apply(b, b"Leyline", "application/javascript") == b"Leyline"
      and bf.apply(None, page.encode(), "text/html") == page.encode())


# ---------------------------------------------------------------- 2 and 3. the portal, without and with the file
CHILD = r'''
import http.client, importlib.util, json, os, sys, threading
from http.server import ThreadingHTTPServer
spec = importlib.util.spec_from_file_location("wp", sys.argv[1]); wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler); srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()
def get(p):
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=20)
    c.request("GET", p, headers={"X-Forwarded-For": "203.0.113.9"}); r = c.getresponse()
    return [r.status, r.getheader("Content-Type") or "", r.read().decode("utf-8", "replace")]
out = {"brand_file": wp.BRAND_FILE, "discord": wp.DISCORD_URL, "pages": {p: get(p) for p in sys.argv[2:]}}
print(json.dumps(out))
'''
PAGES = ["/wrath", "/wrath/leyline", "/wrath/key", "/wrath/realms", "/wrath/download", "/wrath/art/brand-logo.png"]
ART = os.path.join(TMP, "art"); os.makedirs(ART)
for n in os.listdir(os.path.join(STREAM, "www-files", "art")):
    if os.path.isfile(os.path.join(STREAM, "www-files", "art", n)):
        shutil.copy(os.path.join(STREAM, "www-files", "art", n), ART)
PNG = b"\x89PNG\r\n\x1a\n" + b"\0" * 24
open(os.path.join(ART, "brand-logo.png"), "wb").write(PNG)


def portal(extra):
    env = {k: v for k, v in os.environ.items() if k not in ("WU_BRAND_FILE", "LEYLINE_BRAND")}
    env.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
               WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example", WU_DL_PROBE="0", WU_ART_DIR=ART,
               WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json", WU_JOIN_BASE="https://relay.example",
               WU_PORTAL_DATA=os.path.join(TMP, "data-" + str(len(extra))), WU_MODS_DIR=os.path.join(TMP, "mods"),
               WU_ARMORY_DIR=os.path.join(TMP, "armory"), **extra)
    r = subprocess.run([sys.executable, "-I", "-c", CHILD, PORTAL, *PAGES], env=env, capture_output=True, text=True, timeout=180)
    try:
        return json.loads(r.stdout.strip().splitlines()[-1])
    except (ValueError, IndexError):
        return {"error": r.stderr[-1500:]}


off = portal({})
check("no brand file: the portal ran", "pages" in off, off.get("error"))
if "pages" in off:
    check("no brand file: BRAND_FILE is None and the Discord link is Leyline's", off["brand_file"] is None and off["discord"].startswith("https://discord.gg/"))
    for p, (st, ct, body) in off["pages"].items():
        if ct.startswith("text/html"):
            check(f"no brand file: {p} has no brand-file style ({st})", st < 500 and 'id="brand-file"' not in body, st)

on = portal({"WU_BRAND_FILE": path, "LEYLINE_BRAND": "1"})
check("with the brand file: the portal ran", "pages" in on, on.get("error"))
if "pages" in on:
    check("with the brand file: read, and its Discord link", on["brand_file"]["name"] == "Emberline" and on["discord"] == "https://discord.gg/abcDEF12")
    for p, (st, ct, body) in on["pages"].items():
        if not ct.startswith("text/html"):
            continue
        head, _, rest = body.partition("</head>")
        visible = re.sub(r"<(script|style)\b.*?</\1>", " ", rest, flags=re.S)
        visible = re.sub(r"<[^>]+>", " ", visible)
        check(f"with the brand file: {p} ({st}) has the style after the site's own, and says Emberline",
              st < 500 and 'id="brand-file"' in head and head.rfind('id="brand-file"') > head.rfind("<style") - 1
              and ("Emberline" in visible or "Emberline" in head), (st, re.findall(r"\bLeyline\b[^<]{0,40}", visible)[:4]))
        check(f"with the brand file: {p} names no Leyline in its text people read",
              not re.search(r"(?<![\w/.-])Leyline(?![\w-]|\.\w)", visible), re.findall(r".{0,30}\bLeyline\b.{0,30}", visible)[:4])
    st, ct, body = on["pages"]["/wrath/art/brand-logo.png"]
    check("with the brand file: the logo is served from the art list", st == 200 and ct.startswith("image/png"), (st, ct))

shutil.rmtree(TMP, ignore_errors=True)
print(f"\n{passed} passed, {failed} failed")
sys.exit(1 if failed else 0)
