"""Tests for the commons kit (tools/commons): Make my own Leyline, the directory, the forum post, the one-click wrappers.

    python3 -I tools/commons/test_commons.py
"""
import hashlib, http.client, importlib.util, json, os, re, shutil, sys, tempfile, threading, zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
spec = importlib.util.spec_from_file_location("commons", HERE / "commons.py")
c = importlib.util.module_from_spec(spec); spec.loader.exec_module(c)
TMP = Path(tempfile.mkdtemp(prefix="commons-test-"))

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, str(extra)[:600])


def tree_hash(paths):
    h = hashlib.sha256()
    for base in paths:
        base = REPO / base
        files = [base] if base.is_file() else sorted(p for p in base.rglob("*") if p.is_file() and "__pycache__" not in p.parts)
        for p in files:
            h.update(p.relative_to(REPO).as_posix().encode()); h.update(p.read_bytes())
    return h.hexdigest()


OURS = ["launcher/branding.json", "launcher/ui/style.css", "stream", "hosting/kit", "docs/leyline"]
before = tree_hash(OURS)

LOGO = TMP / "logo.png"
LOGO.write_bytes(b"\x89PNG\r\n\x1a\n" + b"\0" * 64)
GOOD = {"name": "Emberline", "accent": "#e0602a", "bg": "#201010", "text": "#fff0e0", "logo": str(LOGO),
        "domain": "play.emberline.example.org", "discord": "https://discord.gg/abcDEF12"}

# ---------------------------------------------------------------- the answers
brand, logo = c.build_brand(GOOD)
check("answers: a brand file and the logo", brand["name"] == "Emberline" and brand["logo"] == "brand-logo.png" and logo == LOGO.read_bytes()
      and brand["links"]["site"] == "https://play.emberline.example.org" and re.match(r"^#[0-9a-f]{6}$", brand["colours"]["accent_strong"]))
check("answers: the hover colour is lighter than the accent", sum(int(brand["colours"]["accent_strong"][i:i + 2], 16) for i in (1, 3, 5))
      > sum(int("e0602a"[i:i + 2], 16) for i in (0, 2, 4)))
check("answers: defaults alone make Leyline's own look", c.build_brand(c.ask({}, True))[0]["colours"]["accent"] == "#5b8def")
for label, change, want in [
    ("the game's name", {"name": "Warcraft Commons"}, "maker"),
    ("a money link as Discord", {"discord": "https://ko-fi.com/x"}, "discord"),
    ("a logo that isn't a picture", {"logo": str(HERE / "commons.py")}, "logo"),
    ("a logo that isn't there", {"logo": str(TMP / "nope.png")}, "logo"),
    ("a domain with https and a path", {"domain": "https://x.org/a/b"}, "domain"),
    ("a colour that isn't #rrggbb", {"bg": "black"}, "colours.bg"),
]:
    try:
        c.build_brand(dict(GOOD, **change)); why = ""
    except c.Refused as e:
        why = str(e)
    check(f"answers refused: {label}", want in why, why)
asked = []
got = c.ask({"name": "X"}, False, inp=lambda q: asked.append(q) or "", out=lambda *a: None)
check("ask(): only what the file doesn't answer, Enter keeps the default", "X" == got["name"] and len(asked) == len(c.QUESTIONS) - 1
      and got["accent"] == "#5b8def")
check("slug", c.slug("Ember Line!") == "ember-line" and c.slug("!!!") == "my-network")

# ---------------------------------------------------------------- make
out = c.make(GOOD, TMP / "out", log=lambda *a: None)
check("make: the folder is named after the network", out == TMP / "out" / "emberline")
for rel in ("brand.json", "UPLOAD.md", "site/wrath_portal.py", "site/brandfile.py", "site/herokeys.py", "site/forum.py", "site/brand.json",
            "site/art/brand-logo.png", "site/docs/leyline/index.md", "site/portal.env.example", "site/run-site.sh", "site/gamedata-off",
            "site/Caddyfile.example", "browser-kit/index.html", "browser-kit/ownfiles.js", "browser-kit/brand.css",
            "browser-kit/art/brand-logo.png", "launcher/branding.json", "launcher/README.txt", "launcher/logo.png",
            "host-kit/emberline-host-kit.zip", "host-kit/Emberline host/Host a Leyline realm.bat"):
    check(f"make: {rel}", (out / rel).is_file())
every = [p for p in out.rglob("*") if p.is_file()]
ex = c.export()
check("make: no game files anywhere (by name)", not [p for p in every if ex.GAME_DATA.search(p.name)], [p for p in every if ex.GAME_DATA.search(p.name)][:5])
check("make: no slices, stream server scripts or tests in the site", not [p for p in (out / "site").rglob("*")
      if set(p.relative_to(out / "site").parts) & {"slices", "server", "tests", "lua", "sql", "fui-web"}])
check("make: the owner's backdrop art is not packed", not list(out.rglob("leyline-shot-backdrop.webp")))
check("make: no secrets in any file", not [p for p in every if ex.SECRET_TEXT.search(p.read_bytes())])
env = (out / "site" / "portal.env.example").read_text()
check("site env: the brand file, game data switch on, streaming off, own files on", "WU_BRAND_FILE=$HERE/brand.json" in env
      and "WU_GAMEDATA_FLAG=$HERE/gamedata-off" in env and "WU_STREAM=0" in env and "WU_OWNFILES=1" in env and "PUBLIC_HOST=play.emberline.example.org" in env)
check("site env: no real passwords", re.search(r"^SOAP_PASS=none$", env, re.M) and not re.search(r"(?i)token|secret=", env))
check("site: run-site.sh is executable and starts the portal", os.access(out / "site" / "run-site.sh", os.X_OK)
      and "wrath_portal.py" in (out / "site" / "run-site.sh").read_text())
check("site: Caddy for the domain", "play.emberline.example.org {" in (out / "site" / "Caddyfile.example").read_text())
idx = (out / "browser-kit" / "index.html").read_text()
check("browser kit: index.html loads brand.css and has the name as its title", '<link rel="stylesheet" href="brand.css">' in idx and "<title>Emberline</title>" in idx)
css = (out / "browser-kit" / "brand.css").read_text()
check("browser kit: brand.css has the colours and the logo", "--lk-accent:#e0602a" in css and 'url("art/brand-logo.png")' in css)
check("browser kit: no client patch, no slice manifests", not (out / "browser-kit" / "web.patch").exists() and not list((out / "browser-kit").rglob("*manifest*.json")))
lb = json.loads((out / "launcher" / "branding.json").read_text())
check("launcher: name, colours, app id and site; rebrand.py accepts it", lb["name"] == "Emberline" and lb["identifier"] == "community.emberline.launcher"
      and lb["colours"]["accent"] == "#e0602a" and lb["site"] == "https://play.emberline.example.org")
try:
    c.rebrand().check(lb); ok = True
except Exception as e:
    ok = e
check("launcher: branding passes rebrand.py's own check", ok is True, ok)
with zipfile.ZipFile(out / "host-kit" / "emberline-host-kit.zip") as z:
    names = z.namelist()
check("host kit zip: one top folder, the kit, the relay code and the one-click wrappers",
      all(n.startswith("Emberline host/") for n in names) and "Emberline host/Host a Leyline realm.bat" in names
      and "Emberline host/Start my own server.bat" in names and "Emberline host/Be a node.bat" in names
      and any(n.startswith("Emberline host/tools/wurelay/") for n in names) and "Emberline host/tools/Dockerfile.relay" in names)
check("host kit zip: no state, .env or owner-only tools", not [n for n in names if "/state/" in n or n.endswith("/.env") or "Publish host kit" in n])
up = (out / "UPLOAD.md").read_text()
check("UPLOAD.md: the steps and what isn't reskinned yet", "## 1. The website" in up and "What is not reskinned yet" in up and "docs/MOVEMENT.md" in up)
words = [str(p) for p in every if p.suffix in (".md", ".txt", ".html", ".css") and ex.brand_hits(p.name, c.LEGAL.sub("", p.read_text("utf-8", "replace")).encode())]
check("make: no game or maker names in anything people read", not words, words[:5])
check("make: Leyline's own files are unchanged", tree_hash(OURS) == before)
c.make(GOOD, TMP / "out", log=lambda *a: None)
check("make: running it again works (same folder)", (out / "brand.json").is_file())

# no logo, no domain
out2 = c.make(dict(GOOD, name="Plain Commons", logo="", domain="", discord=""), TMP / "out", log=lambda *a: None)
check("make without a logo or domain: no logo file, localhost, Leyline's site kept for the launcher",
      not list(out2.rglob("brand-logo.*")) and "PUBLIC_HOST=localhost" in (out2 / "site" / "portal.env.example").read_text()
      and json.loads((out2 / "launcher" / "branding.json").read_text())["site"] == "https://leylinelauncher.com")

# the safety net itself
bad = c.problems({"site": {"a.mpq": b"x", "README.txt": b"Play World of Warcraft here", "ok.txt": b"Not affiliated with or endorsed by Blizzard Entertainment."},
                  "browser-kit": {"k.pem": b"x", "x.js": b"-----BEGIN RSA PRIVATE KEY-----"}})
check("problems(): game data, brand words, secrets; the legal line is allowed",
      any("a.mpq: game data" in b for b in bad) and any("README.txt" in b and "World of Warcraft" in b for b in bad)
      and any("k.pem" in b for b in bad) and any("x.js" in b for b in bad) and not any("ok.txt" in b for b in bad), bad)

# ---------------------------------------------------------------- preview: the packed site really runs
srv = c.preview(out / "site", 0, serve=False)
threading.Thread(target=srv.serve_forever, daemon=True).start()
check("preview: on this PC only", srv.server_address[0] == "127.0.0.1")
def get(p):
    h = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=30); h.request("GET", p); r = h.getresponse()
    return r.status, r.getheader("Content-Type") or "", r.read().decode("utf-8", "replace")
for p in ("/wrath", "/wrath/key", "/wrath/docs"):
    st, ct, body = get(p)
    check(f"preview {p}: answers in the brand ({st})", st == 200 and 'id="brand-file"' in body and "Emberline" in body
          and "--lk-accent:#e0602a" in body, st)
st, ct, body = get("/wrath/art/brand-logo.png")
check("preview: the logo is served", st == 200 and ct.startswith("image/png"))
srv.shutdown()
try:
    c.preview(TMP, 0, serve=False); ok = False
except c.Refused:
    ok = True
check("preview: refuses a folder that isn't a packed site", ok)

# ---------------------------------------------------------------- the directory and its forum post
d = c.load_directory()
check("directory: checks clean", c.check_directory(d) == [], c.check_directory(d))
ids = {x["id"] for x in d["capabilities"]}
for want in ("launcher", "login", "website", "forum", "mod-index", "browser-kit", "host-kit", "relay", "den-lite", "forge",
             "control", "survivors", "seed-server", "kill-switch", "commons-kit"):
    check(f"directory: lists {want}", want in ids)
broken = json.loads(json.dumps(d))
broken["capabilities"][0]["code"] = ["no/such/path"]
broken["capabilities"][1]["what"] = "Play WoW with us"
broken["capabilities"][2]["needs"] = ["a patreon page"]
broken["capabilities"].append(dict(broken["capabilities"][3]))
broken["capabilities"][4]["alone"] = "soon"
bad = c.check_directory(broken)
check("directory: catches a missing path, brand words, money, a repeated id, a bad 'alone'",
      any("no/such/path" in b for b in bad) and any("names the game" in b for b in bad) and any("money" in b for b in bad)
      and any("twice" in b for b in bad) and any("alone must" in b for b in bad), bad)
posts = c.wiki_posts(d)
text = "\n".join(posts)
check("wiki: every post fits the forum (5000 characters)", all(len(p) <= c.FORUM_BODY_MAX for p in posts), [len(p) for p in posts])
check("wiki: every piece and every mods list is in it", all(x["name"] in text for x in d["capabilities"])
      and all(m["file"] in text for m in d["mod_directories"]))
check("wiki: numbered when it takes more than one post", len(posts) == 1 or posts[0].startswith(f"(1/{len(posts)})"))
check("wiki: plain text (no HTML)", "<" not in text.replace("<=", ""))

# ---------------------------------------------------------------- the one-click wrappers
for name in ("Make my own Leyline.bat", *c.WRAPPERS):
    data = (HERE / name).read_bytes()
    check(f"{name}: CRLF line ends", b"\n" in data and data.count(b"\n") == data.count(b"\r\n"))
    check(f"{name}: no game or maker names", not ex.brand_hits(name, data))
mk = (HERE / "Make my own Leyline.bat").read_text()
check("Make my own Leyline.bat runs commons.py make", '"%~dp0commons.py" make' in mk)
paths = c.kit_paths()
check("wrappers in the repo find hosting/kit", paths["kit"] == REPO / "hosting" / "kit" and paths["host"].is_file() and paths["node_compose"].is_file())
srv_bat = (HERE / "Start my own server.bat").read_text()
node_bat = (HERE / "Be a node.bat").read_text()
order = ['"%~dp0Host a Leyline realm.bat"', '"%~dp0host-kit\\Host a Leyline realm.bat"', '"%~dp0..\\..\\hosting\\kit\\Host a Leyline realm.bat"']
check("Start my own server.bat looks in the same places, in the same order, as kit_paths()",
      all(o in srv_bat for o in order) and [srv_bat.index(o) for o in order] == sorted(srv_bat.index(o) for o in order))
check("Start my own server.bat starts private and calls the kit's own host file",
      'set "WU_VISIBILITY=private"' in srv_bat and 'call "%KIT%\\Host a Leyline realm.bat"' in srv_bat)
check("Be a node.bat: the volunteer relay through the kit's own setup and compose file",
      "volunteer-relay --host %NODE%" in node_bat and "volunteer-relay --off" in node_bat
      and 'docker-compose.volunteer-relay.yml" --project-directory "%KIT%" up -d' in node_bat)
setup = (REPO / "hosting" / "kit" / "tools" / "wuhost" / "setup.py").read_text()
check("Be a node.bat's arguments exist in setup.py", all(f in setup for f in ('"volunteer-relay"', '"--host"', '"--region"', '"--off"'))
      and all(r in setup for r in ("na-east", "na-west", "oce")))
check("Be a node.bat needs the relay code and the repo's kit lacks it (so it says so)", 'not exist "%KIT%\\tools\\wurelay"' in node_bat
      and paths["relay_code"] is False)
packed = c.kit_paths(out / "host-kit" / "Emberline host")
check("wrappers inside a packed host kit find it, with the relay code", packed["kit"] == out / "host-kit" / "Emberline host"
      and packed["relay_code"] is True and (packed["tools"] / "Dockerfile.relay").is_file() and (packed["tools"] / "Dockerfile").is_file())
check("the helper image lines match the host kit's", "ghcr.io/orionopenwow/wu-host-tools:1" in node_bat
      and "ghcr.io/orionopenwow/wu-host-tools:1" in (REPO / "hosting" / "kit" / "Host a Wrath realm.bat").read_text())

shutil.rmtree(TMP, ignore_errors=True)
print(f"\n{passed} passed, {failed} failed")
sys.exit(1 if failed else 0)
