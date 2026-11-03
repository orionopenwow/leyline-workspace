"""Leyline HQ's blocked words and names (stream/hq_words.py, through stream/hq.py and stream/admin.py, with a fake realm
SOAP that answers ov_names.lua's "ovnames" words and a small social store): Content > Blocked words (the owner's words
added and removed, protected names, the realm's flags, Check every character), Needs you (each flagged character or
guild with Force a rename and Done, website names that trip the list with Reset the name, and one item for the names
refused lately: a count and the last few, starred, never the word in big type), roles (a mod renames, only an admin
changes the list), the audit log keeping words starred, and nothing deleted on the realm.

    python3 stream/tests/test_hq_words.py [--shot DIR]     (--shot: screenshots of the page with playwright)
"""
import importlib.util
import json
import os
import re
import shutil
import sqlite3
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
T = tempfile.mkdtemp(prefix="wu-hqwords-")
os.environ.update(WU_PORTAL_DATA=T, WU_ADMIN_SECRET="test-secret")

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, str(extra)[:600])


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


nc = load("wu_namecheck", os.path.join(STREAM, "namecheck.py"))
sys.modules["wu_namecheck"] = nc
A = load("wu_admin_t", os.path.join(STREAM, "admin.py"))
H = A.hq
check("HQ loads hq_words.py", H is not None and H.W is not None and "words.add" in A.ACTIONS and "names.rename" in A.ACTIONS)
W = H.W

# ---------------------------------------------------------------- fakes: the realm, the social store
soap_log = []
flags = [{"what": "char", "id": 501, "name": "N1gg3r", "kind": "slur", "at": int(time.time()) - 60, "status": "open"},
         {"what": "char", "id": 502, "name": "LeylineGM", "kind": "staff", "at": int(time.time()) - 30, "status": "open"},
         {"what": "guild", "id": 77, "name": "Rapist Club", "kind": "sexual", "at": int(time.time()) - 10, "status": "open"}]


def soap_lines(cmd):
    soap_log.append(cmd)
    w = cmd.split()
    if w[:2] == ["ovnames", "list"]:
        rows = [f"ROW\t{f['what']}\t{f['id']}\t{f['name']}\t{f['kind']}\t{f['at']}\t{f['status']}" for f in flags
                if w[2] == "all" or f["status"] == "open"]
        return True, rows + [f"ovadmin end {len(rows)}"], ""
    if w[:2] == ["ovnames", "rename"]:
        for f in flags:
            if f["what"] == "char" and f["id"] == int(w[2]):
                f["status"] = "renamed"
        return True, ["ovadmin ok: renames at the next login", "ovadmin end 0"], ""
    if w[:2] == ["ovnames", "done"]:
        for f in flags:
            if f["what"] == w[2] and f["id"] == int(w[3]):
                f["status"] = "done"
        return True, ["ovadmin ok: done", "ovadmin end 0"], ""
    if w[:2] == ["ovnames", "scan"]:
        at = int(w[2])
        return True, ([f"ovadmin next {at + 500}", "ovadmin end 1"] if at < 1000 else [f"ovadmin next {at}", "ovadmin end 0"]), ""
    return False, [], "the realm's ov_admin.lua is not loaded (run the scripts step)"


A.soap_lines = soap_lines
A.bug_list = lambda state: (True, [], "")
A.ws_list = lambda state: (True, [], "")
A.projects_all = lambda: []
A.hub_submissions = lambda: []
A.hub_decisions = lambda: {}
A.themes_review = lambda: False
A.forum_store = lambda: None


class Store:
    def __init__(self):
        self.db = sqlite3.connect(":memory:")
        self.db.execute("CREATE TABLE social_accounts (acct TEXT, name TEXT, tag TEXT, created INTEGER)")
        self.db.executemany("INSERT INTO social_accounts VALUES (?,?,?,?)",
                            [("WR000010", "Cassandra", "1111", 1), ("WR000011", "Sh1thead", "2222", 2), ("WR000012", "Bob", "3333", 3)])

    def q(self, sql, args=()):
        return self.db.execute(sql, args).fetchall()

    def reports(self, *a):
        return []


A.social_store = lambda: STORE
STORE = Store()

owner = {"account": "owner1", "name": "Owner1", "role": "owner", "session": "s1"}
admin = {"account": "adm1", "name": "Adm", "role": "admin", "session": "s2"}
mod = {"account": "mod1", "name": "Mod", "role": "mod", "session": "s3"}


def run(user, action, target, **form):
    return A.run_action(user, dict(form, action=action, target=target))


def audit():
    try:
        return [json.loads(x) for x in open(os.path.join(T, "admin", "audit.jsonl"))]
    except OSError:
        return []


# ---------------------------------------------------------------- Needs you
nc.record_hit("site", "N1gg3r", "slur", "WR000020")
nc.record_hit("launcher", "Adm1n", "staff", "")
items, problems = H.inbox_items()
names = [x for x in items if x["kind"] == "names"]
check("Needs you: every source answered", problems == [], problems)
check("Needs you: a flagged character, starred, with Force a rename and Done",
      any(x["title"] == "N****r" and [a[0] for a in x["acts"]] == ["names.rename", "names.done"] for x in names))
check("Needs you: a slur name says it picks a new name itself", any("next login" in x["sub"] for x in names if x["title"] == "N****r"))
check("Needs you: a team-looking name waits for a GM", any("a GM decides" in x["sub"] for x in names if x["title"] == "L*******M"))
check("Needs you: a guild, Done only", any(x["label"] == "Guild name flagged" and [a[0] for a in x["acts"]] == ["names.done"] for x in names))
check("Needs you: a website name that trips the list, Reset the name (typed confirm)",
      any(x["label"] == "Website name flagged" and x["acts"][0][0] == "profile.reset.name" and x["acts"][0][4] == "WR000011" for x in names)
      and not any("Cassandra" in x["title"] for x in names))
hit = [x for x in names if x["label"] == "Names refused"]
check("Needs you: the names refused lately, a count and the last few starred", hit and hit[0]["title"] == "2 refused this week"
      and "A***n" in hit[0]["text"] and "N****r" in hit[0]["text"])
user_html = H.inbox_page(owner)
check("Needs you page: no raw slur anywhere in big type", "N1gg3r" not in re.sub(r"<details>.*?</details>", "", user_html, flags=re.S)
      and "Sh1thead" not in user_html and "Rapist Club" not in user_html, user_html[:300])
check("Needs you: counts in words", "flagged name" in H.inbox_counts(items))
check("Needs you: the rename buttons are there for a mod", 'value="names.rename"' in H.inbox_page(mod))

# ---------------------------------------------------------------- actions and roles
ok, msg = run(mod, "words.add", "new", word="grumbo", mode="word")
check("a mod can't change the list", not ok and "admins" in msg)
ok, msg = run(admin, "words.add", "new", word="Grumb0", mode="word")
check("an admin adds a word; the website refuses it at once", ok and nc.name_hit("Grumbo") == ("custom", "grumbo"), msg)
ok, msg = run(admin, "words.add", "new", word="<b>x", mode="word")
check("a bad word is refused, with a reason", not ok and "letters" in msg)
a = audit()
check("the audit log keeps the word starred", a and a[-2]["after"]["word"] == "g****o" and "grumbo" not in json.dumps(a), a[-2:])
ok, msg = run(admin, "words.remove", nc.shown("grumbo"), word="grumbo")
check("an admin removes it (target starred in the log)", ok and nc.name_hit("Grumbo") is None and "grumbo" not in json.dumps(audit()), msg)
ok, msg = run(admin, "words.protect", "new", name="Orion", acct="WR000001")
check("protect a staff name", ok and nc.protected_clash("0rion", "WR000002"), msg)
ok, msg = run(admin, "words.protect", "new", name="0rion", acct="WR000002")
check("a look-alike of a protected name can't be protected again", not ok)
ok, msg = run(admin, "words.unprotect", "Orion")
check("unprotect", ok and not nc.protected_clash("0rion", "WR000002"), msg)
soap_log.clear()
ok, msg = run(mod, "names.rename", "501")
check("a mod forces a rename: ovnames rename 501", ok and soap_log[-1] == "ovnames rename 501", soap_log)
ok, msg = run(mod, "names.rename", "501; shutdown")
check("junk never reaches SOAP", not ok and all("shutdown" not in c for c in soap_log))
ok, msg = run(mod, "names.done", "guild:77")
check("Done: ovnames done guild 77", ok and soap_log[-1] == "ovnames done guild 77")
ok, msg = run(mod, "names.done", "table:1")
check("Done: only char or guild", not ok)
ok, msg = run(mod, "names.scan", "realm")
check("a mod can't scan every character", not ok)
soap_log.clear()
ok, msg = run(admin, "names.scan", "realm")
check("an admin scans, 500 at a time until the end", ok and soap_log[:3] == ["ovnames scan 0", "ovnames scan 500", "ovnames scan 1000"])
ok, msg = run(mod, "names.hits.clear", "hits")
check("a mod clears the refused list", ok and nc.hits() == [])
check("nothing deleted on the realm", not any(re.search(r"(?i)delete|ban|account", c) for c in soap_log))

# ---------------------------------------------------------------- the page
nc.add_word("zorbix", "inside", "adm1")
nc.record_hit("forum", "Fuuuck", "profanity", "")
body = W.page(owner)
check("the page: your words (starred, the word itself behind a fold), protected names, flags, refused lately",
      "Your words" in body and "z****x" in body and "<b>z****x</b>" in body and "<details><summary>Show it</summary><code>zorbix</code></details>" in body
      and "Protected names" in body and "Flagged in the game" in body and "Refused lately" in body and "F****k" in body)
check("the page: the add form for an admin", 'value="words.add"' in body and 'value="names.scan"' in body)
check("the page: a mod sees it but not the list's buttons", 'value="words.add"' not in W.page(mod) and 'value="names.rename"' in W.page(mod))
st, title, pbody = H.page(owner, "words", {})
check("HQ page: Content > Blocked words", st == 200 and title == "Blocked words" and ("words", "Blocked words") in A.CONTENT_TABS)
check("the page: plain words, no brand", not re.search(r"(?i)warcraft|blizzard|lich king|\bwow\b", body))
A.soap_lines = lambda cmd: (False, [], "the realm's ov_admin.lua is not loaded (run the scripts step)")
A.forget("names:")
items, problems = H.inbox_items()
check("a realm without ov_names.lua: Needs you still works, no alarm", problems == [] and not [x for x in items if x["label"] == "Character name flagged"])
check("a realm without ov_names.lua: the page says so", "not loaded yet" in W.page(owner))
A.soap_lines = soap_lines
A.forget("names:")

if "--shot" in sys.argv:
    out = sys.argv[sys.argv.index("--shot") + 1]
    os.makedirs(out, exist_ok=True)
    from playwright.sync_api import sync_playwright
    page = A.shell(owner, "words", "Blocked words", W.page(owner), "n0nce").decode()
    inbox = A.shell(owner, "inbox", "Needs you", H.inbox_page(owner), "n0nce").decode()
    with sync_playwright() as p:
        b = p.chromium.launch()
        for name, html, w in (("words-desktop", page, 1200), ("words-phone", page, 390), ("inbox-phone", inbox, 390)):
            pg = b.new_page(viewport={"width": w, "height": 900})
            pg.set_content(html)
            pg.screenshot(path=os.path.join(out, name + ".png"), full_page=True)
            sw = pg.evaluate("document.documentElement.scrollWidth")
            check(f"screenshot {name}: no sideways scroll", sw <= w, sw)
        b.close()

shutil.rmtree(T, ignore_errors=True)
print(f"{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
