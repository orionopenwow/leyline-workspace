"""Leyline HQ: blocked words and names (owner 2026-10-31: "Bad actor measures, username blocks, in game name blocks").

stream/hq.py loads this beside itself. It adds:
    GET /wrath/admin/words     Content > Blocked words: the owner's own words (add, remove), protected staff names (a
                               name only its account may take, look-alikes too: "0rion", "Οrion"), the built-in list's
                               size (stream/blocklist.json), the names refused lately, and the realm's flagged
                               characters and guilds (ov_names.lua) with Force a rename / Done, and Check the realm
    Needs you                  the realm's flagged names (one item each: the name starred, the list it tripped, Force a
                               rename, Done), website names that trip the list now (Reset the name), and one item for
                               the names refused lately (a count and the last few, starred; never the word in big type)
Actions (stream/admin.py run_action: roles, typed confirms, the audit log, which notes a word only starred):
    words.add / words.remove / words.protect / words.unprotect   admin
    names.rename (the character picks a new name at its next login: ov_names.lua "ovnames rename", the same flag as
    .character rename; nothing is deleted) / names.done / names.hits.clear                         mod
    names.scan (flags the characters already made)                                                 admin
The words reach the realm on the next scripts step (realm/serverpass.sh step 1f) and the website at once.
Standard library only.
"""
from __future__ import annotations

import html
import os
import re
import sys
import time

e = html.escape
A = None    # stream/admin.py
H = None    # stream/hq.py

KIND_WORDS = {"slur": "slur or hate", "sexual": "sexual", "profanity": "rude", "staff": "passes for the team",
              "brand": "game name", "custom": "the owner's words", "gm": "a GM's rename"}
WHERE_WORDS = {"site": "website", "web": "website", "launcher": "launcher", "forum": "forum"}
FLAG_RE = re.compile(r"^(char|guild):(\d{1,10})$")


def setup(admin, hq) -> None:
    global A, H
    A, H = admin, hq
    H.INBOX_WORDS.setdefault("names", ("flagged name", "flagged names"))
    A.ACTIONS.update({
        "words.add": (a_word_add, "admin", None),
        "words.remove": (a_word_remove, "admin", None),
        "words.protect": (a_protect, "admin", None),
        "words.unprotect": (a_unprotect, "admin", None),
        "names.rename": (a_rename, "mod", None),
        "names.done": (a_done, "mod", None),
        "names.hits.clear": (a_hits_clear, "mod", None),
        "names.scan": (a_scan, "admin", None),
    })


def nc():
    """stream/namecheck.py (shared through sys.modules["wu_namecheck"], as social.py loads it)."""
    m = sys.modules.get("wu_namecheck")
    if m is None:
        soc = A.P("social")
        fn = getattr(soc, "_namecheck", None) if soc is not None else None
        m = fn() if callable(fn) else None
    if m is None:
        p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "namecheck.py")
        if os.path.isfile(p):
            import importlib.util
            spec = importlib.util.spec_from_file_location("wu_namecheck", p)
            m = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(m)
            sys.modules["wu_namecheck"] = m
    return m


def _need():
    m = nc()
    if m is None:
        raise A.Refused("The blocklist (namecheck.py) is not on this portal: run the signin step.")
    return m


def _refused(fn):
    m = _need()
    try:
        return fn(m)
    except m.Refused as ex:
        raise A.Refused(str(ex))


# ---------------------------------------------------------------------------------------------- actions
def a_word_add(user, target, form):
    return _refused(lambda m: m.add_word(form.get("word"), str(form.get("mode") or "word"), user.get("account", "")))


def a_word_remove(user, target, form):
    # the word rides in a field; the target (what the audit log keeps) is the word starred
    return _refused(lambda m: m.remove_word(form.get("word"), user.get("account", "")))


def a_protect(user, target, form):
    return _refused(lambda m: m.add_protected(form.get("name"), str(form.get("acct") or "").strip(), user.get("account", "")))


def a_unprotect(user, target, form):
    return _refused(lambda m: m.remove_protected(target, user.get("account", "")))


def _soap(cmd: str) -> list[str]:
    ok, lines, err = A.soap_lines(cmd)
    if not ok:
        raise A.Refused(err or "The realm did not answer.")
    A.forget("names:")
    return lines


def a_rename(user, target, form):
    if not re.fullmatch(r"\d{1,10}", target or "") or int(target) <= 0:
        raise A.Refused("Not a character.")
    lines = _soap(f"ovnames rename {int(target)}")
    ok = next((x for x in lines if x.startswith("ovadmin ok:")), "")
    return {"rename": False}, {"rename": True, "answer": ok[12:][:80]}


def a_done(user, target, form):
    m = FLAG_RE.fullmatch(target or "")
    if not m:
        raise A.Refused("Not a flagged name.")
    _soap(f"ovnames done {m.group(1)} {int(m.group(2))}")
    return {"status": "open"}, {"status": "done"}


def a_hits_clear(user, target, form):
    return _need().clear_hits(user.get("account", ""))


def a_scan(user, target, form):
    at, found = 0, 0
    for _ in range(40):           # 500 characters a call: 20 000 at most per tap
        lines = _soap(f"ovnames scan {at}")
        nxt = next((x for x in lines if x.startswith("ovadmin next ")), "")
        end = next((x for x in lines if x.startswith("ovadmin end ")), "ovadmin end 0")
        found += int(end.split()[-1]) if end.split()[-1].isdigit() else 0
        n = int(nxt.split()[-1]) if nxt and nxt.split()[-1].isdigit() else at
        if n <= at:
            break
        at = n
    return {}, {"flagged": found, "up_to_guid": at}


# ---------------------------------------------------------------------------------------------- reading
def realm_flags(state: str = "open") -> tuple[bool, list[dict], str]:
    def get():
        ok, lines, err = A.soap_lines(f"ovnames list {state} 100")
        rows = []
        for x in lines:
            f = x.split("\t")
            if f[0] == "ROW" and len(f) >= 7 and f[1] in ("char", "guild") and f[2].isdigit():
                rows.append({"what": f[1], "id": int(f[2]), "name": f[3][:24], "kind": f[4][:12],
                             "at": int(f[5]) if f[5].isdigit() else 0, "status": f[6][:8]})
        if not ok and "not loaded" in err:
            err = "the realm's ov_names.lua is not loaded yet (the scripts step, then its restart)"
        return ok, rows, err
    return A.cached("names:" + state, A.CACHE_S, get)


def site_flags(limit: int = 50) -> list[dict]:
    """Website display names that trip the list now (a word added later, or a name from before the list)."""
    m, store = nc(), A.social_store()
    if m is None or store is None:
        return []

    def get():
        out = []
        try:
            rows = store.q("SELECT acct, name, tag FROM social_accounts ORDER BY created DESC LIMIT 20000")
        except Exception:   # noqa: BLE001
            return []
        for acct, name, tag in rows:
            h = m.name_hit(name)
            if h and h[0] != "brand":
                out.append({"acct": acct, "name": name, "tag": tag, "kind": h[0]})
                if len(out) >= limit:
                    break
        return out
    return A.cached("names:site", 60, get)


def inbox_items(item) -> tuple[list[dict], list[str]]:
    """Needs you's items (hq._item builds them) and the sources that did not answer."""
    items, problems = [], []
    m = nc()
    if m is None:
        return items, problems
    ok, rows, err = realm_flags("open")
    if not ok and "not loaded" not in err:
        problems.append("flagged names (" + err + ")")
    for r in rows:
        auto = r["kind"] in ("slur", "sexual", "profanity", "custom", "brand")
        what = "Character" if r["what"] == "char" else "Guild"
        acts = []
        if r["what"] == "char" and r["kind"] != "gm":
            acts.append(("names.rename", str(r["id"]), "Force a rename", {}, None, False))
        acts.append(("names.done", f"{r['what']}:{r['id']}", "Done", {}, None, False))
        sub = KIND_WORDS.get(r["kind"], r["kind"]) + (" · picks a new name at the next login" if auto and r["what"] == "char"
                                                      else " · a GM decides" if r["what"] == "char" else " · a GM decides (rename in game)")
        items.append(item("names", f"names:{r['what']}:{r['id']}", f"{what} name flagged", m.shown(r["name"]), "", r["at"],
                          "/wrath/admin/words", acts, sub=sub, group=0) | {"reveal": r["name"]})
    for s in site_flags():
        items.append(item("names", "names:site:" + s["acct"], "Website name flagged", m.shown(s["name"]) + "#" + s["tag"], "", 0,
                          "/wrath/admin/words", [("profile.reset.name", s["acct"], "Reset the name to Player", {}, s["acct"], True)],
                          sub=KIND_WORDS.get(s["kind"], s["kind"]), group=0)
                     | {"reveal": s["name"], "person": s["acct"] if A.GAME_ACCT_RE.fullmatch(s["acct"]) else ""})
    week = m.hits(time.time() - 7 * 86400, 500)
    if week:
        last = ", ".join(f"{m.shown(h['name'])} ({WHERE_WORDS.get(h['where'], h['where'])}, {KIND_WORDS.get(h['kind'], h['kind'])})"
                         for h in week[:5])
        items.append(item("names", "names:hits", "Names refused", f"{len(week)} refused this week",
                          f"Last: {last}. Nobody got these names; this is for spotting someone trying again and again.",
                          week[0]["at"], "/wrath/admin/words", [("names.hits.clear", "hits", "Clear", {}, None, False)]))
    return items, problems


# ---------------------------------------------------------------------------------------------- the page
def _hidden(k: str, v: str) -> str:
    return f'<input type="hidden" name="{e(k)}" value="{e(v)}">'


def page(user: dict) -> str:
    m = nc()
    if m is None:
        return ('<section class="ly-panel"><h2>Blocked words</h2><p>The blocklist (namecheck.py) is not on this portal yet: '
                'run the signin step.</p></section>')
    back = "/wrath/admin/words"
    act = A.act
    d = m.extra_doc()
    L = m.lists()
    counts = {}
    for kind, *_ in L.entries:
        counts[kind] = counts.get(kind, 0) + 1
    built = " · ".join(f"{n} {KIND_WORDS.get(k, k)}" for k, n in counts.items() if k != "custom")
    built += f" · {len(L.staff_edge) + len(L.staff_word)} that pass for the team · {len(L.allow)} ordinary words kept"
    def card(what, title, reveal, sub, acts):
        return (f'<article class="hq-item"><p class="what">{e(what)}</p><p class="title"><b>{e(title)}</b></p>'
                + (f'<details><summary>Show it</summary><code>{e(reveal)}</code></details>' if reveal else "")
                + (f'<p class="sub">{e(sub)}</p>' if sub else "")
                + (f'<div class="acts">{acts}</div>' if acts.strip() else "") + '</article>')
    words = "".join(card("Inside other words too" if x.get("mode") == "inside" else "Whole word only", m.shown(x["w"]), x["w"],
                         f'added by {x.get("by") or "?"} · {H.ago(x.get("at"))}',
                         act(user, "words.remove", m.shown(x["w"]), "Remove", back, _hidden("word", x["w"])))
                    for x in d["words"]) or '<p class="fine">None yet: the built-in list is working on its own.</p>'
    add = act(user, "words.add", "new", "Add the word", back,
              '<input type="text" name="word" maxlength="40" placeholder="a word" aria-label="Word" required autocomplete="off">'
              '<select name="mode" aria-label="How it matches"><option value="word">Whole word only</option>'
              '<option value="inside">Inside other words too</option></select>')
    prot = "".join(card("Protected name", p["name"], "", f'account {p["acct"]} · added by {p.get("by") or "?"}',
                        f'<a href="/wrath/admin/person?acct={e(p["acct"])}">their page</a>'
                        + act(user, "words.unprotect", p["name"], "Remove", back))
                   for p in d["protected"]) or '<p class="fine">None yet.</p>'
    padd = act(user, "words.protect", "new", "Protect the name", back,
               '<input type="text" name="name" maxlength="24" placeholder="name" aria-label="Name" required>'
               '<input type="text" name="acct" maxlength="32" placeholder="its account" aria-label="Account" required>')
    ok, flags, err = realm_flags("all")
    fl = "".join(card(("Character" if r["what"] == "char" else "Guild") + " · " + r["status"] + " · " + H.ago(r["at"]),
                      m.shown(r["name"]), r["name"], KIND_WORDS.get(r["kind"], r["kind"]),
                      (act(user, "names.rename", str(r["id"]), "Force a rename", back)
                       if r["what"] == "char" and r["status"] != "renamed" else "")
                      + (act(user, "names.done", f'{r["what"]}:{r["id"]}', "Done", back) if r["status"] == "open" else ""))
                 for r in flags) or f'<p class="fine">{e("Could not read the realm: " + err) if not ok else "Nothing flagged."}</p>'
    scan = act(user, "names.scan", "realm", "Check every character", back)
    hits = m.hits(0, 30)
    hrows = "".join(f'<li>{e(m.shown(h["name"]))} · {e(WHERE_WORDS.get(h["where"], h["where"]))} · '
                    f'{e(KIND_WORDS.get(h["kind"], h["kind"]))} · {e(H.ago(h["at"]))}</li>' for h in hits)
    return (f'<section class="ly-panel"><h2>Blocked words</h2><p class="fine">One list for names everywhere: website and '
            f'launcher names, the forum, realm names on the list, and characters and guilds in the game. Letters that look '
            f'alike, digits for letters, accents, repeats and separators are undone first. Built in: {e(built)}.</p>'
            f'<h3>Your words</h3>{words}{add}<p class="fine">The website uses them at once; the realm on the next scripts '
            f'step.</p></section>'
            f'<section class="ly-panel"><h2>Protected names</h2><p class="fine">A team member\'s name nobody else may take, '
            f'look-alikes included.</p>{prot}{padd}</section>'
            f'<section class="ly-panel"><h2>Flagged in the game</h2><p class="fine">Characters with a slur or a rude name pick '
            f'a new one at their next login by themselves; names that pass for the team wait for you. Nothing is deleted.'
            f'</p>{fl}{scan}</section>'
            f'<section class="ly-panel"><h2>Refused lately</h2>'
            + (f'<ul class="fine">{hrows}</ul>' if hrows else '<p class="fine">Nothing refused yet.</p>')
            + '</section>')
