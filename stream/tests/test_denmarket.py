"""Shared Dens (stream/denmarket.py, docs/DEN-MARKETPLACE.md): the switch (off, beta, on), sharing with the Hero Key
sign-in, the Den file and picture checks, the moderation queue (nothing goes live by itself; updates wait while the old
version stays), likes and downloads, reports into Leyline HQ, the remix line and the credit in the downloaded file,
the catalogue and showcase pages (no brand words, the author's link), and the site's forms. A fake sign-in and HQ.

    python3 -I stream/tests/test_denmarket.py [stream/wrath_portal.py]
"""
import base64, hashlib, importlib.util, io, json, os, re, struct, sys, tempfile, zlib

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
T = tempfile.mkdtemp(prefix="wu-dens-")
os.environ["WU_PORTAL_DATA"] = os.path.join(T, "data")
os.environ.pop("WU_DENS", None)

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, str(extra)[-1500:])


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, os.path.join(STREAM, file))
    m = importlib.util.module_from_spec(spec)
    sys.modules[name] = m
    spec.loader.exec_module(m)
    return m


L = load("wu_listings", "listings.py")
HUB = load("wu_hub_t", "hub.py")
D = load("wu_denmarket", "denmarket.py")


# ---------------------------------------------------------------- fakes: the sign-in, HQ, the handler
class Store:
    TOKENS = {"tok-bea": "BEA", "tok-cal": "CAL", "tok-dax": "DAX"}
    PROFILES = {"BEA": {"name": "Bea", "pid": "beabeabea2"}, "CAL": {"name": "Cal", "pid": "cacacacaa3"}, "DAX": {"name": "Dax", "pid": "daxdaxdax4"}}

    def token_acct(self, tok):
        return self.TOKENS.get(tok)

    def profile(self, acct):
        return self.PROFILES.get(acct)


class Social:
    store = Store()

    @staticmethod
    def cookie_user(h):
        return ("CAL", "Cal") if "wr_session=s-cal" in h.headers.get("Cookie", "") else None


class FakeA:
    class Refused(Exception):
        pass
    ACTIONS = {}
    STAFF = None

    @classmethod
    def who(cls, h, path):
        if cls.STAFF is None:
            h.send(403, b"Staff only")
            return None
        return cls.STAFF


class H:
    def __init__(self, body=b"", headers=None):
        self.headers = dict(headers or {})
        self.headers.setdefault("Content-Length", str(len(body)))
        self.rfile = io.BytesIO(body)
        self.sent = None
        self.where = None

    def ip(self):
        return "127.0.0.1"

    def send(self, status, body, ctype="text/html; charset=utf-8", headers=None, **kw):
        self.sent = (status, body.decode("utf-8") if isinstance(body, bytes) and (not ctype.startswith("image/") or "svg" in ctype) else body, ctype, headers or {})

    def redirect(self, url, cookie=""):
        self.where = url
        self.sent = (303, "", "", {})


D._get = lambda name: {"social": Social, "admin": FakeA, "listings": L, "hub_mod": HUB, "PUBLIC_HOST": "leylinelauncher.com"}.get(name)
D.setup()
check("HQ's buttons are registered at setup", {"den.approve", "den.reject", "den.feature", "den.update.approve", "den.takedown"} <= set(FakeA.ACTIONS))
STAFF = {"account": "staff1", "role": "mod"}


def act(name, target, form=None):
    fn = FakeA.ACTIONS[name][0]
    return fn(STAFF, target, form or {})


def api_get(path, headers=None, query=None):
    h = H(b"", headers)
    D.get(h, path, query or {})
    body = h.sent[1]
    if path.endswith("/file") and h.sent[0] == 200:
        return h.sent[0], body, h
    try:
        return h.sent[0], json.loads(body), h
    except (ValueError, TypeError):
        return h.sent[0], body, h


def api_post(action, doc, headers):
    h = H(json.dumps(doc).encode(), dict({"Content-Type": "application/json"}, **headers))
    D.post(h, D.API + "/" + action)
    return h.sent[0], json.loads(h.sent[1])


BEA = {"Authorization": "Bearer tok-bea"}
CAL = {"Authorization": "Bearer tok-cal"}
DAX = {"Authorization": "Bearer tok-dax"}


def png(w=640, h=360):
    raw = b"".join(b"\x00" + b"\x40\x30\x80" * w for _ in range(h))
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def jpg(w=800, h=450):
    return b"\xff\xd8\xff\xe0" + struct.pack(">H", 16) + b"JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00" + \
        b"\xff\xc0" + struct.pack(">HBHH", 17, 8, h, w) + b"\x03\x01\x22\x00\x02\x11\x01\x03\x11\x01" + b"\xff\xd9"


def den(n=12, span=10, name="garden", extra=None):
    d = {"format": "leyline-den", "version": 1, "name": name, "saved": "2026-11-04T10:00:00Z", "room": {"look": "white", "size": 16},
         "props": [{"entry": 19006 + i % 3, "x": (i % 5) * span / 5, "y": (i // 5) * span / 5, "z": 0, "yaw": 90} for i in range(n)]}
    d.update(extra or {})
    return json.dumps(d)


P1, P2 = base64.b64encode(png()).decode(), base64.b64encode(jpg()).decode()

# ---------------------------------------------------------------- the switch
st, d, _ = api_get(D.API)
check("off by default: the API is not open", st == 404 and d["error"] == "not_open", d)
st, body, _ = api_get(D.PAGE)
check("off by default: the catalogue page is a 404", st == 404)
st, d = api_post("share", {"title": "x"}, BEA)
check("off by default: sharing is not open", st == 404)
os.environ["WU_DENS"] = "beta"
st, d, _ = api_get(D.API)
check("beta: the API answers (the launcher's tab)", st == 200 and d["ok"] and d["items"] == [], d)
st, body, _ = api_get(D.PAGE)
check("beta: the pages still 404", st == 404)
os.environ["WU_DENS"] = "1"
st, body, _ = api_get(D.PAGE)
check("on: the catalogue page answers", st == 200 and "Shared Dens" in body)

# ---------------------------------------------------------------- sharing
st, d = api_post("share", {"title": "Garden"}, {})
check("signed out can't share", st == 401 and d["error"] == "signed_out")
good = {"kind": "den", "title": "Bea's Garden  ", "description": "Rows of flowers.\n\nA bench to sit on.", "tags": ["garden", "cozy"],
        "credit": "Bea and the Sunday crew", "licence": "cc-by", "den": den(), "pictures": [P1, P2]}
st, d = api_post("share", good, BEA)
check("a Hero Key sign-in shares a Den: it waits for a moderator", st == 200 and d["state"] == "pending" and D.ID_RE.fullmatch(d["id"]), d)
G = d["id"]
st, d, _ = api_get(D.API)
check("... and isn't in the catalogue (never live by itself)", st == 200 and d["items"] == [])
st, d, _ = api_get(D.API + "/" + G, CAL)
check("... nor open to others", st == 404)
st, d, _ = api_get(D.API + "/" + G, BEA)
check("... but its maker sees it", st == 200 and d["build"]["title"] == "Bea's Garden" and d["build"]["mine"], d)
st, d, _ = api_get(D.API + "/mine", BEA)
check("mine lists it with its state", st == 200 and d["builds"][0]["id"] == G and d["builds"][0]["state"] == "pending", d)
b = D.store.one(G)
check("the pictures are stored as sent (png, jpg) with the Den's facts", b["pics"] == ["0.png", "1.jpg"] and b["props"] == 12 and b["stampable"]
      and b["tags"] == ["garden", "cozy"] and b["author"] == "Bea" and b["pid"] == "beabeabea2", b)

bad_cases = [
    ({"den": "not json"}, "can't be read"),
    ({"den": json.dumps({"format": "other", "version": 1, "props": []})}, "isn't a Den file"),
    ({"den": den(0)}, "no props"),
    ({"den": den(900)}, "800 props"),
    ({"den": den(extra={"props": [{"entry": 1, "x": 0, "y": 0, "code": {"a": {"b": {"c": 1}}}}]})}, "holds something"),
    ({"den": "x" * (520 * 1024)}, "too big"),
    ({"kind": "stamp", "den": den(80)}, "a stamp is up to 60"),
    ({"kind": "stamp", "den": den(10, span=200)}, "a stamp is up to 60"),
    ({"title": "ab"}, "title"),
    ({"tags": ["garden", "nope"]}, "tags"),
    ({"tags": list(D.TAGS)[:6]}, "tags"),
    ({"licence": "mine-all-mine"}, "licence"),
    ({"pictures": ["!!notbase64"]}, "didn't arrive whole"),
    ({"pictures": [base64.b64encode(b"GIF89a....").decode()]}, "JPEG or PNG"),
    ({"pictures": [base64.b64encode(png(100, 100)).decode()]}, "from 320x180"),
    ({"pictures": [P1] * 5}, "Up to 4"),
    ({"den": den(extra={"credits": [{"by": "Cal"}], "props": [{"entry": 1, "x": 0, "y": 0, "credit": 3}]})}, "credit isn't in the credits"),
    ({"den": den(extra={"credit": {"by": ["not", "a", "word"]}})}, "credit isn't right"),
]
for change, want in bad_cases:
    st, d = api_post("share", dict(good, **change), DAX)
    check(f"refused: {want}", st == 400 and want.lower() in d["error"].lower(), (st, d))
D.store.db.execute("DELETE FROM shares")
st, d = api_post("share", dict(good, title="World of Warcraft Garden by Blizzard", description="A WoW garden"), DAX)
dax_brand = d.get("id")
bb = D.store.one(dax_brand) if dax_brand else None
check("the brand words are taken out of what a player types", st == 200 and bb and not D.BRAND_RE.search(bb["title"] + bb["descr"]), (d, bb))
ok_credits = den(extra={"credits": [{"id": "abcdefghij", "title": "Camp", "by": "Cal"}],
                        "props": [{"entry": 1, "x": 0, "y": 0, "credit": 0}, {"entry": 2, "x": 1, "y": 0}]})
st, d = api_post("share", dict(good, title="With a stamp in it", den=ok_credits, pictures=[]), DAX)
check("a Den that names its stamps' makers (credits, a piece's credit number) is fine", st == 200, d)
for _ in range(10):
    st, d = api_post("share", dict(good, pictures=[]), DAX)
check("shares are limited per hour", st == 429, d)

# ---------------------------------------------------------------- moderation (Leyline HQ)
items = D.hq_items(FakeA, lambda kind, ident, label, title, text, at, where, acts, sub="", group=1: dict(kind=kind, id=ident, label=label,
                                                                                                          title=title, acts=acts, where=where, sub=sub))
mine_item = next((x for x in items if x["id"] == "den:" + G), None)
check("Needs you lists the share with Approve and Reject, and a review link", mine_item and [a[0] for a in mine_item["acts"]] == ["den.approve", "den.reject"]
      and mine_item["where"].startswith(D.REVIEW) and "12 props" in mine_item["sub"], mine_item)
FakeA.STAFF = None
st, body, h = api_get(D.REVIEW + "/" + G)
check("the review page is staff only", st == 403)
FakeA.STAFF = STAFF
st, body, h = api_get(D.REVIEW + "/" + G)
check("staff see a waiting build with its pictures, whatever the switch", st == 200 and f"{D.REVIEW}/{G}/pic/0.png" in body)
st, body, h = api_get(D.REVIEW + "/" + G + "/pic/0.png")
check("... and its pictures (private)", st == 200 and h.sent[2] == "image/png" and "no-store" in h.sent[3]["Cache-Control"])
try:
    act("den.reject", G, {})
    check("reject needs a reason", False)
except FakeA.Refused:
    check("reject needs a reason", True)
act("den.approve", G)
check("approve: it is live", D.store.one(G)["state"] == "live" and D.store.one(G)["reviewed_by"] == "staff1")
try:
    act("den.approve", G)
    check("approving twice is refused", False)
except FakeA.Refused:
    check("approving twice is refused", True)

# ---------------------------------------------------------------- browsing, the file, likes
st, d, _ = api_get(D.API)
check("live: in the catalogue as a card (picture, author, size, props, tags, likes, downloads)", st == 200 and len(d["items"]) == 1
      and all(k in d["items"][0] for k in ("picture", "author", "bytes", "props", "tags", "likes", "downloads"))
      and d["items"][0]["picture"] == f"{D.PAGE}/pic/{G}/0.png", d)
check("... with the tags in use, every tag, the licences and report reasons for the launcher's forms",
      [t["id"] for t in d["tags"]] == ["garden", "cozy"] and len(d["all_tags"]) == len(D.TAGS) and len(d["licences"]) == len(D.LICENCES)
      and d["reasons"])
for q, want in ((dict(q="flowers"), 1), (dict(q="sunday crew"), 1), (dict(q="castle"), 0), (dict(kind="stamp"), 0), (dict(kind="den"), 1),
                (dict(tag="cozy"), 1), (dict(tag="spooky"), 0)):
    st, d, _ = api_get(D.API, query={k: [v] for k, v in q.items()})
    check(f"search and filters: {q} -> {want}", len(d["items"]) == want, d)
st, d, _ = api_get(D.API + "/" + G, CAL)
det = d["build"]
check("the detail: pictures, licence, lineage, sha256, made with Leyline", det["pictures"] == [f"{D.PAGE}/pic/{G}/0.png", f"{D.PAGE}/pic/{G}/1.jpg"]
      and det["licence"]["name"] == "CC BY 4.0" and det["lineage"] == [] and len(det["sha256"]) == 64 and det["made_with"] == "Made with Leyline"
      and det["remix_ok"] and not det["mine"], det)
st, text, h = api_get(D.API + "/" + G + "/file", CAL)
fdoc = json.loads(text)
check("the file a player gets carries the credit (by, line, licence, made with, the page)", st == 200 and fdoc["credit"]["by"] == "Bea"
      and fdoc["credit"]["line"] == "Bea and the Sunday crew" and fdoc["credit"]["licence"] == "CC BY 4.0"
      and fdoc["credit"]["made_with"] == "Made with Leyline" and fdoc["credit"]["url"].endswith("/wrath/dens/" + G), fdoc.get("credit"))
check("... it matches the detail's sha256 (the launcher checks it)", hashlib.sha256(text.encode("utf-8")).hexdigest() == det["sha256"])
probs, facts = D.den_check(text, "den")
check("... and is still a Den file by the loader's rules", not probs and facts["props"] == 12, probs)
api_get(D.API + "/" + G + "/file", CAL)
api_get(D.API + "/" + G + "/file", DAX)
api_get(D.API + "/" + G + "/file", BEA)
check("downloads count once a day per player (not the maker's own)", D.store.one(G)["downloads"] == 2, D.store.one(G)["downloads"])
st, d = api_post("like", {"id": G}, CAL)
st2, d2 = api_post("like", {"id": G}, CAL)
check("a like counts once", st == 200 and d["likes"] == 1 and d2["likes"] == 1)
st, d = api_post("like", {"id": G}, BEA)
check("you can't like your own", st == 400)
st, d = api_post("like", {"id": G, "on": False}, CAL)
check("unlike", d["likes"] == 0 and not d["liked"])
api_post("like", {"id": G}, CAL)
api_post("like", {"id": G}, DAX)

# ---------------------------------------------------------------- remixes and credit
st, d = api_post("share", dict(good, title="Garden with a pond", den=text, pictures=[P1]), CAL)
R = d["id"]
check("a Den opened from Leyline and shared again is based on it (the credit in the file)", st == 200 and D.store.one(R)["based_on"] == G, d)
act("den.approve", R)
st, d, _ = api_get(D.API + "/" + R, DAX)
check("its lineage names the original and its maker", d["build"]["lineage"] == [{"id": G, "title": "Bea's Garden", "by": "Bea", "live": True}], d)
st, d, _ = api_get(D.API + "/" + G, DAX)
check("the original counts its remixes", d["build"]["remixes"] == 1)
st, t2, _ = api_get(D.API + "/" + R + "/file", DAX)
check("the remix's file names the chain", json.loads(t2)["credit"]["based_on"] == [{"id": G, "title": "Bea's Garden", "by": "Bea"}])
st, d = api_post("share", dict(good, title="Look only den", licence="look", pictures=[]), BEA)
LK = d["id"]
act("den.approve", LK)
st, d = api_post("share", dict(good, title="My copy", based_on=LK, pictures=[]), CAL)
check("a look-only build can't be shared as someone else's remix", st == 400 and "look only" in d["error"], d)
st, d = api_post("share", dict(good, title="Based on nothing", based_on="zzzzzzzzzz", pictures=[]), CAL)
check("based on a build that isn't there is refused", st == 400)

# ---------------------------------------------------------------- updates, reports, featured, unpublish
st, d = api_post("update", {"id": G, "title": "Bea's Big Garden", "den": den(20)}, BEA)
check("an update to a live build waits; the old one stays up", st == 200 and d["update_waiting"] and D.store.one(G)["title"] == "Bea's Garden", d)
st, t3, _ = api_get(D.API + "/" + G + "/file", DAX)
check("... players still get the old file", len(json.loads(t3)["props"]) == 12)
st, d = api_post("update", {"id": G, "title": "Hijack"}, CAL)
check("only the maker updates", st == 404)
items = D.hq_items(FakeA, lambda kind, ident, label, title, text, at, where, acts, sub="", group=1: dict(id=ident, acts=acts))
check("Needs you shows the update", any(x["id"] == f"den:{G}:next" for x in items))
act("den.update.approve", G)
b = D.store.one(G)
check("approve the update: the new title and file go out, rev 2", b["title"] == "Bea's Big Garden" and b["props"] == 20 and b["rev"] == 2 and not b["next"], b)
st, d = api_post("update", {"id": G, "pictures": [P2]}, BEA)
act("den.update.approve", G)
check("a picture update replaces the pictures", D.store.one(G)["pics"] == ["0.jpg"], D.store.one(G)["pics"])
st, d = api_post("report", {"id": G, "reason": "stolen", "note": "That's my garden"}, DAX)
st2, d2 = api_post("report", {"id": G, "reason": "because"}, DAX)
st3, d3 = api_post("report", {"id": G, "reason": "spam"}, BEA)
check("report: a reason from the list; not your own", st == 200 and st2 == 400 and st3 == 400)
items = D.hq_items(FakeA, lambda kind, ident, label, title, text, at, where, acts, sub="", group=1: dict(id=ident, acts=acts, text=text, group=group))
rep = next((x for x in items if x["id"] == f"den:{G}:reports"), None)
check("Needs you shows the report first (group 0), with Dismiss and Take it down", rep and rep["group"] == 0 and "That's my garden" in rep["text"]
      and [a[0] for a in rep["acts"]] == ["den.reports.dismiss", "den.takedown"], rep)
act("den.reports.dismiss", G)
check("dismissed", not D.store.open_reports())
act("den.feature", G)
st, d, _ = api_get(D.API)
check("featured builds come first", [x["id"] for x in d["featured"]] == [G])
try:
    act("den.feature", dax_brand)
    check("only live builds can be featured", False)
except FakeA.Refused:
    check("only live builds can be featured", True)

# ---------------------------------------------------------------- the pages
st, body, _ = api_get(D.PAGE)
check("the catalogue: featured on top, then all, with cards", st == 200 and "Featured" in body and "<h2>All</h2>" in body
      and body.index("Featured") < body.index("<h2>All</h2>") and f'href="{D.PAGE}/{G}"' in body, body[-3000:])
st, body, _ = api_get(D.PAGE + "/" + G, {"Cookie": "wr_session=s-cal"})
check("the showcase: the pictures, the author's link, made with Leyline, licence, Open in Leyline",
      st == 200 and f"{D.PAGE}/pic/{G}/0.jpg" in body and f'href="{D.PAGE}/by/beabeabea2"' in body and "Made with Leyline" in body
      and "CC BY 4.0" in body and f"leyline://den/{G}" in body, body[-2500:])
check("... logged in: Like and Report forms with a token", 'name="step" value="unlike"' in body and 'name="csrf"' in body and "Report" in body)
st, body, _ = api_get(D.PAGE + "/" + R)
check("a remix's showcase says what it is based on, linked", "Based on" in body and f'href="{D.PAGE}/{G}"' in body and "by Bea" in body)
st, body, _ = api_get(D.PAGE + "/by/cacacacaa3")
check("an author's page lists their builds", st == 200 and "Dens by Cal" in body and f"{D.PAGE}/{R}" in body, body[-2000:])
st, svg, h = api_get(D.PAGE + "/" + G + "/card.svg")
check("the showcase card", st == 200 and svg.startswith("<svg") and "Made with Leyline" in svg and "by Bea" in svg and h.sent[2].startswith("image/svg"))
st, img, h = api_get(D.PAGE + "/pic/" + G + "/0.jpg")
check("a live build's picture", st == 200 and h.sent[2] == "image/jpeg")
st, img, h = api_get(D.PAGE + "/pic/" + dax_brand + "/0.png")
check("a waiting build's picture is not public", st == 404)
st, body, _ = api_get(D.PAGE + "/" + dax_brand)
check("a waiting build's page is not public", st == 404)
BRAND = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
pages = [api_get(p)[1] for p in (D.PAGE, D.PAGE + "/" + G, D.PAGE + "/" + R, D.PAGE + "/by/beabeabea2")]
check("no brand words and no client numbers on the pages", not any(BRAND.search(re.sub(r"<[^>]+>", " ", p)) or "3.3.5" in p for p in pages))
csrf = D._csrf("s-cal")
h = H(f"step=report&reason=spam&csrf={csrf}".encode(), {"Cookie": "wr_session=s-cal", "Content-Type": "application/x-www-form-urlencoded"})
D.post(h, D.PAGE + "/" + G)
check("the showcase's report form", h.where == f"{D.PAGE}/{G}?said=reported" and D.store.open_reports(), h.where)
h = H(b"step=like&csrf=bad", {"Cookie": "wr_session=s-cal", "Content-Type": "application/x-www-form-urlencoded"})
D.post(h, D.PAGE + "/" + G)
check("... a stale token is refused", h.where.endswith("said=again"))
h = H(json.dumps({"id": G}).encode(), {"Cookie": "wr_session=s-cal", "Content-Type": "application/json"})
D.post(h, D.API + "/like")
check("the API with the site's cookie needs the CSRF header", h.sent[0] == 403)

st, d = api_post("unpublish", {"id": G}, BEA)
check("unpublish takes it down", st == 200 and api_get(D.API + "/" + G, CAL)[0] == 404 and api_get(D.PAGE + "/" + G)[0] == 404)
st, d, _ = api_get(D.API + "/" + R, CAL)
check("... and its remix still credits it by name", d["build"]["lineage"][0]["title"] == "Bea's Big Garden" and not d["build"]["lineage"][0]["live"], d)
st, d = api_post("unpublish", {"id": R}, BEA)
check("only the maker unpublishes", st == 404)
os.environ["WU_DENS"] = "0"
st, body, _ = api_get(D.PAGE + "/" + R)
check("off again: pages 404", st == 404)

print(f"\n{passed} passed, {failed} failed")
sys.exit(1 if failed else 0)
