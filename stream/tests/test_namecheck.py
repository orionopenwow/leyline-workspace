"""The one blocklist for names (stream/namecheck.py, stream/blocklist.json) and where it is used: tricky names (look-alike
letters, digits for letters, accents, repeats, separators, run-together words, zero-width and right-to-left characters)
and the ordinary words that must pass (Scunthorpe, Cassandra, Grape, Therapist...), text and masking, realm names, the
owner's words and protected staff names, the refusals noted for HQ, the realm's whole-name tables; social.py's
name_error / bad_word / mask, profiles.py, the forum (plain text, links only with rel=nofollow ugc noopener
noreferrer, no data: or javascript: links, no pictures, the link cap, invisible characters out), hostflow.py, the
relay's listing and the host kit; the copies (hosting/kit/tools/wunet, the table in ov_names.lua) kept equal, and the
Lua guard giving the same answers as Python.

    python3 stream/tests/test_namecheck.py
"""
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
ROOT = os.path.dirname(STREAM)
T = tempfile.mkdtemp(prefix="wu-names-")
os.environ["WU_PORTAL_DATA"] = T

passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, str(extra)[:500])


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


nc = load("wu_namecheck", os.path.join(STREAM, "namecheck.py"))
sys.modules["wu_namecheck"] = nc

# ---------------------------------------------------------------- names that must be refused
BAD = {
    "Nigger": "slur", "N1gg3r": "slur", "Niiiggerrr": "slur", "n.i.g.g.e.r": "slur", "N i g g e r": "slur", "Fagz": "slur",
    "Kike": "slur", "Tr4nny": "slur", "R3tard": "slur", "xXHitlerXx": "slur", "Nigga​": "slur",
    "TheRapist": "sexual", "The_Rapist": "sexual", "Dildomage": "sexual", "Pornstar": "sexual", "Cocks": "sexual",
    "Fuuuck": "profanity", "Fvck": None, "Sh1thead": "profanity", "MotherFrank": "profanity", "Bitchy": "profanity",
    "Admin": "staff", "Adm1n": "staff", "4dmin": "staff", "Admiiin": "staff", "A d m i n": "staff", "A.d.m.i.n": "staff",
    "Аdmin": "staff", "Ádmín": "staff", "Ａｄｍｉｎ": "staff", "Ad­min": "staff", "Ad‮min": "staff",
    "Moderat0r": "staff", "LeylineStaff": "staff", "L3yl1n3": "staff", "Leyline": "staff", "GameMaster": "staff",
    "GMBob": "staff", "BobGM": "staff", "Gm_Bob": "staff", "[GM]Bob": "staff", "Owner": "staff", "Official": "staff",
    "Support": "staff", "xXAdminXx": "staff", "AdminBob": "staff", "BobTheAdmin": "staff", "0penVanilla": "staff",
    "Staff": "staff", "Mods": "staff", "Warcraft": "brand", "WoWAlice": "brand", "Lich_King": "brand",
}
for name, kind in BAD.items():
    h = nc.name_hit(name)
    if kind is None:
        continue
    check(f"refused: {name.encode('unicode_escape').decode()} ({kind})", h is not None and h[0] == kind, h)

# ---------------------------------------------------------------- ordinary names that must pass
GOOD = ["Scunthorpe", "Cassandra", "Grape", "Therapist", "Bob", "Arthas", "Niger", "Snigger", "Badminton", "Gmork",
        "Staffan", "Dickens", "Peacock", "Hancock", "Sussex", "Essex", "Analyst", "Shiitake", "Penistone", "Toshitaka",
        "Spices", "Cocker", "Officially", "Thrall", "Jaina", "Classic", "Assassin", "Raccoon", "Japan", "Kassandra",
        "MassHitter", "Cockburn", "Document", "Cucumber", "Pakistan", "Nazira", "Sexton", "Cumming", "Analise",
        "Orion", "Supper", "Modest", "Devil", "Ownership", "Teamwork", "Mage-Lord", "Ana.Lyst", "Glassassin",
        "Hello_World", "Peter", "Grapefruit", "Rapeseed", "Shrapnel", "Titanic", "Nightwing"]
for name in GOOD:
    check(f"passes: {name}", nc.name_hit(name) is None, nc.name_hit(name))
check("a game name in a realm list name is left to the brand rule there (realm_name_hit)", nc.realm_name_hit("Wrath Realm") is None)
for r in ("Mod Paradise", "Dev realm", "Support Group PvP", "Owner's Den", "My Wrath Realm", "Wrath Unleashed",
          "Vanilla Remastered", "Scunthorpe Heroes"):
    check(f"realm name passes: {r}", nc.realm_name_hit(r) is None, nc.realm_name_hit(r))
for r, k in (("Leyline Official", "staff"), ("GM Island", "staff"), ("0fficial Realm", "staff"), ("Admin Realm", "staff"),
             ("Porn Palace", "sexual"), ("N1gg3r Town", "slur"), ("Shit Realm", "profanity")):
    h = nc.realm_name_hit(r)
    check(f"realm name refused: {r}", h is not None and h[0] == k, h)

# ---------------------------------------------------------------- text and masking
check("text: a staff word is fine in a sentence", nc.text_hit("ask an admin or a GM") is None)
check("text: Scunthorpe, Grape, Therapist, Cassandra pass", nc.text_hit("Scunthorpe grape therapist Cassandra shiitake") is None)
check("text: a word with an ending", nc.text_hit("those retards") == ("slur", "retard"))
for src, want in (("what the fuck is this", "what the **** is this"),
                  ("I love grape juice in Scunthorpe", "I love grape juice in Scunthorpe"),
                  ("f u c k you", "* * * * you"),
                  ("you n1gg3r!", "you *******"),
                  ("sh.it happens", "***** happens"),
                  ("a therapist said hi", "a therapist said hi"),
                  ("Fuuuuck", "*******")):
    got = nc.mask(src)
    check(f"mask: {src!r}", got == want and len(got) == len(src), got)
check("mask keeps the length (the forum checks it)", all(len(nc.mask(s)) == len(s) for s in ("a​b fuck", "Ｆｕｃｋ x", "x" * 300)))
check("invisible characters: zero-width, bidi overrides, soft hyphen, BOM out",
      nc.strip_invisible("a​b‮c⁦d­e﻿f⁠g") == "abcdefg")
check("skeleton: look-alikes of a name agree", len({nc.skeleton(x) for x in ("Orion", "0rion", "Οrion", "Orrion", "O.rion", "ORION")}) == 1)
check("the refusal never names the word", nc.message("slur") == "Pick another name." and "team" in nc.message("staff"))
check("shown: first and last letter, the rest starred", nc.shown("Nigger") == "N****r" and nc.shown("ab") == "**")

# ---------------------------------------------------------------- the owner's words, protected names, hits
check("owner's word: unknown before", nc.name_hit("Grumbo") is None)
nc.add_word("Grumb0", "word", "owner1")
check("owner's word: added (digits undone), refused at once", nc.name_hit("Grumbo") == ("custom", "grumbo") and nc.text_hit("a grumbo here"))
check("owner's word: whole word only by default", nc.name_hit("Grumbology") is None)
nc.add_word("zorb", "inside", "owner1")
check("owner's word: inside words too when asked", nc.name_hit("Xzorbx") == ("custom", "zorb"))
for bad in ("a", "x", "<script>", "word'; DROP", ""):
    try:
        nc.add_word(bad, "word", "owner1")
        check(f"owner's word refused: {bad!r}", False)
    except nc.Refused:
        check(f"owner's word refused: {bad!r}", True)
try:
    nc.add_word("grumbo", "word", "x")
    check("owner's word: twice refused", False)
except nc.Refused:
    check("owner's word: twice refused", True)
nc.remove_word("grumbo", "owner1")
check("owner's word: removed", nc.name_hit("Grumbo") is None)
before, after = nc.add_word("faggy", "word", "owner1")
check("what the audit log gets is starred", after["word"] == "f***y" and "faggy" not in json.dumps([before, after]))
nc.add_protected("Orion", "WR000001", "owner1")
check("protected: a look-alike is refused for others", nc.protected_clash("0rion", "WR000002") and nc.protected_clash("Οrion", "")
      and nc.protected_clash("O_r_i_o_n", "WR000009"))
check("protected: its own account may use it", not nc.protected_clash("Orion", "WR000001"))
check("protected: other names fine", not nc.protected_clash("Orionis", "WR000002"))
nc.record_hit("site", "N1gg3r", "slur", "WR000005")
nc.record_hit("launcher", "Adm1n", "staff", "")
h = nc.hits()
check("hits: newest first, what and where", [x["where"] for x in h] == ["launcher", "site"] and h[1]["acct"] == "WR000005")
nc.clear_hits("owner1")
check("hits: cleared", nc.hits() == [])

# ---------------------------------------------------------------- the realm's tables
r = nc.reserved_names([{"w": "grumbo"}])
check("reserved_name: the team words, whole names, 2-12 letters", {"admin", "gm", "gamemaster", "leyline", "moderator", "owner"} <= set(r["reserved_name"])
      and all(re.fullmatch(r"[a-z]{2,12}", n) for n in r["reserved_name"] + r["profanity_name"]))
check("profanity_name: the rest and the owner's words, no overlap", {"nigger", "fuck", "grumbo", "rapist"} <= set(r["profanity_name"])
      and not set(r["reserved_name"]) & set(r["profanity_name"]))
sp = open(os.path.join(ROOT, "realm", "serverpass.sh"), encoding="utf-8").read()
check("scripts step: fills both tables from the portal's namecheck.py, takes out only its own rows, restarts on a change",
      "nc.reserved_names(words)" in sp and "ov_names_reserved" in sp and "need_names=1" in sp
      and '[ "${need_names:-0}" = 0 ] || need=1' in sp and subprocess.run(["bash", "-n", os.path.join(ROOT, "realm", "serverpass.sh")]).returncode == 0)
gen = re.search(r'NSQL=\$\(python3 -I - "\$NC" [^\n]*<<\'PY\'\n(.*?)\nPY\n', sp, re.S)
if gen:
    p = os.path.join(T, "gen.py")
    open(p, "w").write(gen.group(1))
    bw = os.path.join(T, "bw.json")
    json.dump({"words": [{"w": "grumbo", "mode": "word"}, {"w": "x'); DROP TABLE characters; --", "mode": "word"}]}, open(bw, "w"))
    out = subprocess.run([sys.executable, "-I", p, os.path.join(STREAM, "namecheck.py"), bw], capture_output=True, text=True)
    sql = out.stdout
    check("scripts step SQL: the owner's words, the reserved names, nothing else gets in",
          out.returncode == 0 and "('grumbo', 'word')" in sql and "INSERT IGNORE INTO reserved_name (name) VALUES ('admin');" in sql
          and "DROP" not in sql and "INSERT IGNORE INTO profanity_name (name) VALUES ('nigger');" in sql
          and not re.search(r"(?im)^DELETE FROM (reserved|profanity)_name", sql), out.stderr)
else:
    check("scripts step SQL generator found", False)
spb = open(os.path.join(ROOT, "realm", "Server pass - finder 10 and bug reports.bat"), encoding="utf-8").read()
check("scripts step ships ov_names.lua", 'copy /y "custom\\lua_scripts\\ov_names.lua"' in spb)

# ---------------------------------------------------------------- the copies
check("the relay and the host kit carry the same namecheck.py and blocklist.json",
      open(os.path.join(STREAM, "namecheck.py"), "rb").read() == open(os.path.join(ROOT, "hosting/kit/tools/wunet/namecheck.py"), "rb").read()
      and open(os.path.join(STREAM, "blocklist.json"), "rb").read() == open(os.path.join(ROOT, "hosting/kit/tools/wunet/blocklist.json"), "rb").read())
lua_file = os.path.join(ROOT, "realm/custom/lua_scripts/ov_names.lua")
lua = open(lua_file, encoding="utf-8").read()
check("ov_names.lua carries the table namecheck.py writes (python3 stream/namecheck.py --lua ...)", nc.lua_table() in lua)
bl = json.load(open(os.path.join(STREAM, "blocklist.json")))
words = [w for k in ("slur", "sexual", "profanity") for m in ("inside", "word") for w in bl[k][m]]
check("blocklist.json: small (under 300 words), letters only, no word twice", len(words) < 300
      and all(re.fullmatch(r"[a-z]{2,24}", w) for w in words + bl["allow"] + bl["staff"]["edge"] + bl["staff"]["word"])
      and len(set(words)) == len(words))
check("blocklist.json: no allow word is itself refused", all(nc.name_hit(a, ("slur", "sexual", "profanity")) is None for a in bl["allow"]))

lua51 = shutil.which("lua5.1") or shutil.which("lua")
if lua51:
    names = list(BAD) + GOOD
    script = ("package.path = ''\nCharDBQuery = nil\nfunction RegisterPlayerEvent() end\nfunction RegisterGuildEvent() end\n"
              f"local N = dofile({json.dumps(lua_file)})\n"
              "for line in io.lines() do local k = N.NameHit(line); io.write((k or 'ok'), '\\n') end\n")
    sp_ = os.path.join(T, "parity.lua")
    open(sp_, "w").write(script)
    nc_tmp = os.environ.pop("WU_PORTAL_DATA")
    os.environ["WU_PORTAL_DATA"] = os.path.join(T, "empty")   # the built-in list only, as the Lua table
    out = subprocess.run([lua51, sp_], input="\n".join(names) + "\n", capture_output=True, text=True)
    lk = out.stdout.split("\n")
    diff = []
    for i, n in enumerate(names):
        py = nc.name_hit(n)
        py = py[0] if py else "ok"
        if (lk[i] if i < len(lk) else "?") != py:
            diff.append((n, py, lk[i] if i < len(lk) else "?"))
    os.environ["WU_PORTAL_DATA"] = nc_tmp
    check(f"the Lua guard and Python agree on {len(names)} names", out.returncode == 0 and not diff, diff or out.stderr)
else:
    print("SKIP lua5.1 not installed: Lua/Python parity")

# ---------------------------------------------------------------- social.py, profiles.py
soc = load("wu_social", os.path.join(STREAM, "social.py"))
check("social: uses namecheck", soc._namecheck() is nc)
check("social name_error: a slur", soc.name_error("N1gg3r") == "Pick another name.")
check("social name_error: a team name", "team" in soc.name_error("Adm1n"))
check("social name_error: Scunthorpe, Cassandra, Grape, Therapist pass", all(soc.name_error(n) == "" for n in ("Scunthorpe", "Cassandra", "Grape", "Therapist")))
check("social name_error: a protected name (look-alike), its owner may", soc.name_error("0rion", "WR000002") != "" and soc.name_error("Orion", "WR000001") == "")
check("social name_error: an invisible character is not a name character", soc.name_error("Bo​b") != "")
soc.name_error("Fuuuck", "WR000007", "launcher")
h = nc.hits()
check("social name_error: a refusal with where is noted for HQ", h and h[0]["where"] == "launcher" and h[0]["acct"] == "WR000007")
check("social bad_word / mask", soc.bad_word("you n1gg3r") == "nigger" and soc.mask("what the fuck") == "what the ****"
      and soc.bad_word("grape therapist") is None)
check("social clean_name: refused names come back empty", soc.clean_name("Adm1n") == "" and soc.clean_name("Bob") == "Bob"
      and soc.clean_name("Bo​b") == "Bob")
prof_src = open(os.path.join(STREAM, "profiles.py"), encoding="utf-8").read()
check("profiles: name_problem passes the account and where", "mod.name_error(name, acct, where)" in prof_src)
soc_src = open(os.path.join(STREAM, "social.py"), encoding="utf-8").read()
check("social API: the launcher's and the website's renames go through the check with the account",
      "check_name(doc.get(\"name\"), acct, via)" in soc_src)

# ---------------------------------------------------------------- the forum
fm = load("wu_forum_t", os.path.join(STREAM, "forum.py"))
html_ = fm.render("look https://x.example/cat.png and data:text/html,<script>alert(1)</script> javascript:alert(1) "
                  "<img src=x onerror=alert(1)> <iframe src=https://evil.example>")
check("forum: an image address is a link, never a picture or an embed", "<img" not in html_ and "<iframe" not in html_
      and '<a href="https://x.example/cat.png" rel="nofollow ugc noopener noreferrer">' in html_)
check("forum: data: and javascript: are never links (only http and https are)", 'href="data:' not in html_
      and 'href="javascript:' not in html_ and all(h.startswith("https://") for h in re.findall(r'href="([^"]*)"', html_)))
check("forum: every link nofollow ugc noopener noreferrer", all('rel="nofollow ugc noopener noreferrer"' in a for a in re.findall(r"<a [^>]+>", html_)))
check("forum: the link cap per post (2 for new accounts)", fm.LIMITS["new"]["links"] == 2 and fm.LIMITS["normal"]["links"] >= 2)
check("forum: invisible characters out of titles and posts", fm.clean_body("a​b‮c⁦d﻿e­f⁠g") == "abcdefg"
      and "‮" not in fm.clean_line("x‮y", 10))
fsrc = open(os.path.join(STREAM, "forum.py"), encoding="utf-8").read()
check("forum: titles through the word filter, posts starred (social.py, so namecheck)", '_hook("bad_word", t)' in fsrc and '_hook("mask", b)' in fsrc)

# ---------------------------------------------------------------- hostflow, the relay, the kit
hf = load("wu_hostflow_t", os.path.join(STREAM, "hostflow.py"))
check("hostflow name_ok: refuses a team name and a sexual one, keeps an ordinary one",
      not hf.name_ok("Leyline Official") and not hf.name_ok("Porn Palace") and hf.name_ok("Scunthorpe Heroes") and hf.name_ok("My Wrath Realm"))
sys.path.insert(0, os.path.join(ROOT, "hosting", "kit", "tools"))
sys.path.insert(0, os.path.join(ROOT, "hosting", "relay"))
from wurelay import listing  # noqa: E402
rec = listing.clean_record({"name": "L3yline Staff"})
check("relay: a realm named like the team is refused", rec["name"] == "" and rec["name_refused"])
rec = listing.clean_record({"name": "Scunthorpe Heroes"})
check("relay: an ordinary name is listed", rec["name"] == "Scunthorpe Heroes" and not rec["name_refused"])
from wuhost import setup as kit  # noqa: E402
check("host kit: refuses what the relay refuses", kit.name_refused("Official GM Realm") and not kit.name_refused("My Wrath Realm"))

# ---------------------------------------------------------------- shipping
fl = open(os.path.join(STREAM, "fastloop.sh"), encoding="utf-8").read()
bat = open(os.path.join(STREAM, "Update sign-in and warm-up.bat"), encoding="utf-8").read()
check("signin step ships namecheck.py with blocklist.json, and hq_words.py; backs them up and puts them back",
      'install -m 644 namecheck.py blocklist.json "$WU/portal/"' in fl and 'install -m 644 hq_words.py' in fl
      and 'had_names' in fl and 'set "LINKS=%LINKS% namecheck.py blocklist.json"' in bat and 'set "LINKS=%LINKS% hq_words.py"' in bat
      and subprocess.run(["bash", "-n", os.path.join(STREAM, "fastloop.sh")]).returncode == 0)
for f in ("namecheck.py", "blocklist.json", "hq_words.py"):
    txt = open(os.path.join(STREAM, f), encoding="utf-8").read()
    check(f"{f}: no brand word in what players read", "World of Warcraft" not in txt and "Blizzard Entertainment" not in txt)

shutil.rmtree(T, ignore_errors=True)
print(f"{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
