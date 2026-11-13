"""stream/site_pages.py on the portal: Single player (/wrath/single-player, dark until SINGLE_PLAYER_PUBLIC=1), the
Modes shelf (/wrath/modes, /wrath/modes/<way>, the front page's Ways to play), the guide series (/wrath/guides and its
12 episodes) and the press kit (/wrath/press). Both frames (the hub's front door on and off), the flag both ways, the
nav, picture slots (a missing file is a panel, never a broken image; the killswitch takes the game pictures off), no
game brand names (only the one legal line), no client version numbers in prose, "Log in" not "Sign in".

    python3 -I stream/tests/test_site_pages.py stream/wrath_portal.py
"""
import http.client, importlib.util, os, re, shutil, sys, tempfile, threading
from http.server import ThreadingHTTPServer

PORTAL = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "wrath_portal.py"))
STREAM = os.path.dirname(PORTAL)
TMP = tempfile.mkdtemp(prefix="wu-sitepages-")
MODS, ARM, ART = (os.path.join(TMP, d) for d in ("mods", "armory", "art"))
for d in (os.path.join(MODS, "files"), os.path.join(ARM, "c"), ART):
    os.makedirs(d)
for n in os.listdir(os.path.join(STREAM, "www-files", "art")):
    if os.path.isfile(os.path.join(STREAM, "www-files", "art", n)):
        shutil.copy(os.path.join(STREAM, "www-files", "art", n), ART)
os.environ["WU_DL_PROBE"] = "0"
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                  WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="play.example", WU_MODS_DIR=MODS,
                  WU_ARMORY_DIR=ARM, WU_ART_DIR=ART, WU_RELAY_LISTING="http://127.0.0.1:9/api/listing/realms.json",
                  WU_JOIN_BASE="https://relay.example", LEYLINE_BRAND="1", WU_PORTAL_DATA=os.path.join(TMP, "data"),
                  WU_GAMEDATA_FLAG=os.path.join(TMP, "gamedata-off"))
os.environ.pop("SINGLE_PLAYER_PUBLIC", None)
os.environ.pop("WU_HUB_FRONT", None)
spec = importlib.util.spec_from_file_location("wp", PORTAL)
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
sp = wp.site_pages

passed = failed = 0
def check(name, ok, extra=""):
    global passed, failed
    if ok: passed += 1; print("PASS", name)
    else: failed += 1; print("FAIL", name, str(extra)[:600])

srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()
def get(path, ip="203.0.113.7"):
    c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=20)
    c.request("GET", path, headers={"X-Forwarded-For": ip}); r = c.getresponse()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, r.read().decode("utf-8", "replace")

LEGAL = "not affiliated with or endorsed by Blizzard Entertainment"
BANNED = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
# client version numbers as words people read (a build badge in a data field is fine; none of these pages has one)
VERSION = re.compile(r"(?<![\w.])(?:1\.12(?:\.\d)?|2\.4\.3|3\.3\.5a?|4\.3\.4|5\.4\.8)(?![\w.])")
def visible(p):
    p = re.sub(r"<(style|script)\b.*?</\1>", " ", p, flags=re.S)
    p = re.sub(r"<!--.*?-->", " ", p, flags=re.S)
    attrs = " ".join(re.findall(r'(?:alt|title|aria-label|placeholder|content)="([^"]*)"', p))
    text = re.sub(r"<[^>]+>", " ", p) + " " + attrs
    return re.sub(r"https?://\S+", " ", text)

check("site_pages loaded and bound", sp is not None and callable(sp.portal("card_meta")))
check("the flag is off by default", not sp.single_public())

# ---------------------------------------------------------------- the pages, the hub's front door off (the Leyline look)
EP = [g["slug"] for g in sp.guides()]
check("12 guide episodes, numbered 1 to 12", len(EP) == 12 and [g["n"] for g in sp.guides()] == list(range(1, 13)), EP)
check("the guide slugs are unique and fit the route", len(set(EP)) == 12 and all(sp.SLUG_RE.fullmatch(s) for s in EP))
PAGES = ["/wrath/modes", "/wrath/guides", "/wrath/press"] + [f"/wrath/guides/{s}" for s in EP] + \
        [f"/wrath/modes/{m['id']}" for m in sp.modes()]

def page_checks(label):
    for path in PAGES:
        st, hd, page = get(path)
        check(f"{label} {path}: 200 html", st == 200 and hd.get("content-type", "").startswith("text/html"), st)
        v = visible(page)
        bad = [b for b in BANNED.findall(v.replace(LEGAL, ""))]
        check(f"{label} {path}: no game brand names", not bad, bad[:4])
        vers = VERSION.findall(v)
        check(f"{label} {path}: no client version numbers in prose", not vers, vers[:4])
        check(f"{label} {path}: says Log in, never Sign in", not re.search(r"\bSign in\b", visible(page.split('<main', 1)[-1])),
              re.findall(r".{30}Sign in.{30}", v)[:2])
        check(f"{label} {path}: no broken picture (every img src is an art file that is there)",
              all(wp.art_present(s[len("/wrath/art/"):]) for s in re.findall(r'<img[^>]+src="(/wrath/art/[^"]+)"', page)
                  if "lp-" in page), [s for s in re.findall(r'<img[^>]+src="(/wrath/art/[^"]+)"', page) if not wp.art_present(s[11:])][:3])
        # our own part of the page: from the first lp block to the footer (the Modes page's own sections after the
        # shelf credit their sources, stream/modes.py; the frames' share boxes name this site)
        ours = page[page.find('class="lp'):]
        ours = ours.split("</section>", 1)[0] if 'id="shelf"' in ours[:80] else re.split(r"<footer", ours, maxsplit=1)[0]
        ext = [h for h in re.findall(r'href="(https?://[^"]+)"', ours)
               if not re.match(r"https://(discord\.gg|discord\.com|(www\.)?reddit\.com|github\.com/orionopenwow/|play\.example/)", h)]
        check(f"{label} {path}: outside links only Discord, Reddit and our own source", not ext, ext[:4])
        check(f"{label} {path}: no money links", not re.search(r"(?i)donat|patreon|ko-fi|paypal|tip jar", v))

page_checks("frame off:")
st, _, modes_pg = get("/wrath/modes")
# Duels and Boss Rush only on /wrath/modes (owner 12 Nov): the shelf is on the front page (Ways to play), not here
check("/wrath/modes: no shelf (Duels and Boss Rush only)", 'id="shelf"' not in modes_pg and 'class="lp-mode"' not in modes_pg
      and 'id="duels"' in modes_pg)
shelf_pg = sp.shelf()
check("shelf: one card per way to play (flag off: four; Survivors and Minigolf hidden)", shelf_pg.count('class="lp-mode"') == 4, shelf_pg.count('class="lp-mode"'))
for want in ("Wrath Unleashed", "Community realms", "Your Den", "The seeded realm"):
    check(f"shelf: {want}", f"<h3>{want}</h3>" in shelf_pg)
for gone in ("Azeroth Survivors", "Minigolf and the Dojo"):
    check(f"shelf: {gone} hidden, kept in MODES()", f"<h3>{gone}</h3>" not in shelf_pg and any(m["title"] == gone and m.get("hidden") for m in sp.MODES()))
check("shelf: hidden_ids", sp.hidden_ids() == ["azeroth-survivors", "golf-and-dojo"], sp.hidden_ids())
for gone in ("azeroth-survivors", "golf-and-dojo"):
    st_g, h_g, _ = get(f"/wrath/modes/{gone}")
    check(f"/wrath/modes/{gone}: sends players to the Modes page", st_g == 302 and h_g.get("location") == "/wrath/modes", (st_g, h_g.get("location")))
check("front page Ways to play: no Minigolf, no Survivors", "Minigolf" not in sp.front_modes() and "Azeroth Survivors" not in sp.front_modes())
check("shelf: each card has How it works", shelf_pg.count(">How it works</a>") == 4)
check("shelf: the seeded realm is marked Coming, with no Play", re.search(r'id="way-seeded-realm".*?Coming.*?</article>', shelf_pg, re.S)
      and not re.search(r'id="way-seeded-realm"(?:(?!</article>).)*class="lp-btn" href', shelf_pg, re.S))
check("shelf: Play now on the official realm goes to /wrath", 'href="/wrath">Play now</a>' in shelf_pg)

# ---------------------------------------------------------------- the flag: dark by default
st, _, page = get("/wrath/single-player")
check("flag off: /wrath/single-player is the 404", st == 404, st)
st, _, page = get("/wrath/modes/single-player")
check("flag off: /wrath/modes/single-player is not ours (404)", st == 404 and "Single player" not in visible(page).split("No such mode")[0][-200:], st)
check("flag off: no Single player on the shelf", "Single player" not in visible(shelf_pg))
check("flag off: no Single player in the nav", "/wrath/single-player" not in wp.site_nav())
check("flag off: the front sections have Ways to play and no Single player",
      'id="ways"' in sp.front_sections() and 'id="single-player"' not in sp.front_sections())
check("the nav has Guides", 'href="/wrath/guides"' in wp.site_nav())

# the host templates' pages are untouched: their ids are not ours
for mid in ("den", "survivors", "levelling", "jam"):
    check(f"/wrath/modes/{mid}: still stream/modepack.py's", not sp.match(f"/wrath/modes/{mid}"))
check("no id of ours is a host template id", not {m["id"] for m in sp.MODES()} & {m.get("id") for m in (wp.modepacks.load_catalog().get("modes") or [])}
      if wp.modepacks is not None and hasattr(wp.modepacks, "load_catalog") else True)
st, _, _ = get("/wrath/guides/no-such-guide")
check("an unknown guide: 404", st == 404, st)
st, _, _ = get("/wrath/guides/../press")
check("a guide path with dots: not a page of ours", st in (400, 404), st)

# ---------------------------------------------------------------- the flag on
os.environ["SINGLE_PLAYER_PUBLIC"] = "1"
st, _, page = get("/wrath/single-player")
check("flag on: /wrath/single-player answers", st == 200, st)
v = visible(page)
for part in ("What it is", "Your companions", "How to start", "Questions", "Get the launcher", "Add your game folder", "Press Start"):
    check(f"single player: {part}", part in v)
for q in ("Do I need Docker?", "Whose game files does it use?", "Where are my characters saved?", "Do bots take my loot?", "Does it work offline?"):
    check(f"single player FAQ: {q}", f"<summary>{q}</summary>" in page)
check("single player: six companion slots", page.count('data-slot="slots/sp-') == 6, page.count('data-slot="slots/sp-'))
check("single player: the slots say a picture is coming (no broken image)", page.count("Picture coming soon") >= 6)
check("single player: no <video> without its file (poster only, marked as coming)", "<video" not in page and 'data-video="soon"' in page)
check("single player: the marketing lines are left for the owner", "TODO(owner)" in open(os.path.join(STREAM, "site_pages.py")).read())
check("single player: no brand words", not BANNED.findall(v.replace(LEGAL, " ")))   # the legal line is in the footer now
check("single player: no version numbers", not VERSION.findall(v), VERSION.findall(v))
check("flag on: the shelf has five cards", sp.shelf().count('class="lp-mode"') == 5)
check("flag on: /wrath/modes/single-player answers", get("/wrath/modes/single-player")[0] == 200)
check("flag on: Single player in the nav", 'href="/wrath/single-player"' in wp.site_nav())
check("flag on: the front page section", 'id="single-player"' in sp.front_sections())

# a video slot with its file: a <video> with controls and no autoplay
os.makedirs(os.path.join(ART, "slots"), exist_ok=True)
with open(os.path.join(ART, "slots", "sp-trailer.webm"), "wb") as f:
    f.write(b"\x1a\x45\xdf\xa3" + b"\0" * 64)
st, _, page = get("/wrath/single-player")
check("with the trailer: a <video> with controls, nothing preloaded, no autoplay",
      '<video controls playsinline preload="none"' in page and "autoplay" not in page and "/wrath/art/slots/sp-trailer.webm" in page)
st, hd, body = get("/wrath/art/slots/sp-trailer.webm")
check("the slot file is served as video/webm", st == 200 and hd.get("content-type") == "video/webm", (st, hd.get("content-type")))
with open(os.path.join(ART, "slots", "sp-bot-bar.webp"), "wb") as f:
    f.write(b"not a webp at all")
check("a slot file that isn't what its name says: not served", get("/wrath/art/slots/sp-bot-bar.webp")[0] == 404)
check("a name outside the slot pattern: not served", get("/wrath/art/slots/../leyline-mark-512.png")[0] in (400, 404)
      and wp.art_type("slots/x.exe") == "" and wp.art_type("slots/sub/x.webp") == "")
# the game data killswitch: the game pictures and slots go, the page shows its panels
open(os.environ["WU_GAMEDATA_FLAG"], "w").close()
if wp.gamedata_off():
    st, _, page = get("/wrath/single-player")
    check("killswitch: no game pictures on the page", "leyline-shot-" not in page and "/wrath/art/slots/" not in page.split("<!--")[0] + "".join(
        x.split("-->", 1)[-1] for x in page.split("<!--")[1:]))
    check("killswitch: the slot file answers 410", get("/wrath/art/slots/sp-trailer.webm")[0] == 410)
    os.remove(os.environ["WU_GAMEDATA_FLAG"])
else:
    check("killswitch flag file is read (WU_GAMEDATA_FLAG)", False, wp.gamedata_flag())
shutil.rmtree(os.path.join(ART, "slots"))

# ---------------------------------------------------------------- guides
st, _, page = get("/wrath/guides")
check("guides: every episode linked", all(f'href="/wrath/guides/{s}"' in page for s in EP))
for g in sp.guides():
    st, _, page = get(f"/wrath/guides/{g['slug']}")
    v = visible(page)
    check(f"guide {g['n']}: steps, Stuck?, the Discord", "<h2 id=\"gd-steps\">Steps</h2>" in page and "Stuck?" in v
          and re.search(r'href="https://discord\.gg/[^"]+"[^>]*>Discord</a>', page), g["slug"])
    check(f"guide {g['n']}: at least 4 steps and 2 stuck lines", len(g["steps"]) >= 4 and len(g["stuck"]) >= 2)
    check(f"guide {g['n']}: a video slot", 'data-video="soon"' in page or "<video" in page)
    check(f"guide {g['n']}: links to the next and previous", (g["n"] == 12 or f'href="/wrath/guides/{EP[g["n"]]}"' in page)
          and (g["n"] == 1 or f'href="/wrath/guides/{EP[g["n"] - 2]}"' in page))
    for t, h in g.get("docs", []):
        if h.startswith("/wrath/docs/"):
            name = h[len("/wrath/docs/"):].split("#")[0]
            check(f"guide {g['n']}: its doc {name} exists", os.path.exists(os.path.join(os.path.dirname(STREAM), "docs", "leyline", name + ".md")), h)
titles = [g["title"] for g in sp.guides()]
for want in ("Install", "game folder", "Official realm", "Community realm", "Your Den", "Invite friends", "Host your own",
             "node", "Mods and looks", "Survivors", "phone", "Control"):
    check(f"the series covers: {want}", any(want.lower() in t.lower() for t in titles))

# ---------------------------------------------------------------- the press kit
st, _, page = get("/wrath/press")
v = visible(page)
check("press: the legal line, once", v.count(LEGAL) == 1)
check("press: Blizzard only in the legal line", len(re.findall(r"(?i)blizzard", v)) == 1)
check("press: the mark, downloadable", 'href="/wrath/art/leyline-mark-512.png" download' in page)
check("press: screenshots of the launcher", page.count('class="lp-pic') >= 6)
check("press: the paragraph is left for the owner", "TODO(owner): the press paragraph" in page)
check("press: contact is the Discord", "Contact" in v and "discord.gg" in page)

# ---------------------------------------------------------------- the hub's front door on: the same pages in the hub's frame
os.environ["WU_HUB_FRONT"] = "1"
if wp.hub_mod is not None:
    page_checks("frame on:")
    st, _, page = get("/wrath/guides")
    check("frame on: drawn in the hub's frame", 'data-look="leyline"' in page and 'class="hb-top"' in page)
    check("frame on: Guides marked in the menu", re.search(r'class="hb-(?:ni|mi) on" href="/wrath/guides"', page))
    st, _, page = get("/wrath/single-player")
    check("frame on, flag on: Single player in the side menu, marked", re.search(r'class="hb-ni on" href="/wrath/single-player"', page))
    front = wp.hub_mod.hub.front_page(wp.hub_mod.FRONT).decode()
    check("front page: Ways to play and Single player", 'id="ways"' in front and 'id="single-player"' in front)
    check("front page: Ways to play links Every mode", 'href="/wrath/modes">Every mode</a>' in front)
    check("front page: no brand words in the new sections", not BANNED.findall(visible(sp.front_sections())))
    check("footer: Guides, Modes and Press", all(f'href="{h}">{t}</a>' in front for h, t in
                                               (("/wrath/guides", "Guides"), ("/wrath/modes", "Modes"), ("/wrath/press", "Press"))))
    os.environ["SINGLE_PLAYER_PUBLIC"] = "0"
    front = wp.hub_mod.hub.front_page(wp.hub_mod.FRONT).decode()
    check("front page, flag off: no Single player section or menu item", 'id="single-player"' not in front
          and 'href="/wrath/single-player"' not in front)
    check("front page, flag off: Ways to play stays", 'id="ways"' in front)
os.environ.pop("WU_HUB_FRONT", None)
os.environ.pop("SINGLE_PLAYER_PUBLIC", None)

srv.shutdown()
shutil.rmtree(TMP, ignore_errors=True)
print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
