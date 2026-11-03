"""HQ > Review: submitted links, the feedback digest and upgrades, and what each staff role may do.

Owner, 3 Nov: "Complete admin panel built out for mods of the site, and a mod-level account access panel somewhere so
I can recruit help", the review queue for submitted links (stream/linkqueue.py) and "an approval window for changes
and upgrades, and a suggested time window for pushing updates". stream/admin.py loads this beside itself (like hq.py)
and runs without it.

    GET  /wrath/admin/submissions     every submitted link (site, Discord, the hub's Submit a mod): pending first, with
                                      its checks; Approve, Reject (with a reason), a staff note; recently decided with
                                      who decided and Undo (the owner)
    GET  /wrath/admin/feedback        the newest feedback digest (stream/feedback.py): themes, mood, trend, examples,
                                      the sources it read; Run now (admins)
    GET  /wrath/admin/upgrades        proposed changes and upgrades waiting for the owner's Approve or Hold, the release
                                      window (the quietest hours), and the launcher version policy (owner)
    GET  /wrath/leyline/version.json  public (CORS *): {format, updated_at, current, minimum, notice, hold, window,
                                      releases}: what the launchers compare with (docs/LAUNCHER-VERSION-POLICY.md)
Actions (POST /wrath/admin/act, like every console action: CSRF, role, audit log):
    submit.note (helper), submit.approve / submit.reject (mod, with the limits in may_approve), submit.undo (owner),
    feedback.run (admin), upgrade.propose (admin), upgrade.approve / upgrade.hold / upgrade.reopen (owner),
    version.set (owner, typed "set")

Roles (ROLE_TABLE; docs/ADMIN-CONSOLE.md "Roles"): helper < mod < admin < owner. Helpers see Review > Submissions
and Feedback only and can leave notes. Moderators review: they approve clean mod, addon and realm links and reject
anything, handle reports and the forum. Moderators can never change roles, see secrets, keys or emails (no console
page shows any), deploy (nothing in the console deploys), or approve owner-only things: looks and HD packs, flags,
the release channel, the minimum launcher version, upgrades, and undoing a decision.

Files: admin/upgrades.json {"format": 1, "items": {id: {title, what, kind, version, by, by_name, at, status,
decided_by, decided_at, note}}}; leyline/version-policy.json {minimum, notice, by, at}; the digest in $WU_PORTAL_DATA/ops.
Standard library only.
"""
from __future__ import annotations

import html
import json
import os
import re
import secrets
import time

A = None   # stream/admin.py (attributes looked up live)
e = html.escape

REVIEW_TABS = (("submissions", "Submissions"), ("feedback", "Feedback"), ("upgrades", "Upgrades"))
HELPER_TABS = ("", "review", "submissions", "feedback")
UPGRADE_KINDS = (("launcher", "Launcher release"), ("site", "Website"), ("realm", "Realm and scripts"),
                 ("den", "Den Lite"), ("other", "Something else"))
UPGRADE_STATES = ("proposed", "approved", "held")
UP_ID_RE = re.compile(r"^u\d{12}[0-9a-f]{4}$")
LQ_ID_RE = re.compile(r"^[0-9a-f]{12}$")
VERSION_FEED = "/wrath/leyline/version.json"

# (what, helper, mod, admin, owner): the Team table on the Staff page
ROLE_TABLE = (
    ("See submitted links and the feedback digest; leave notes", True, True, True, True),
    ("Approve clean mod, addon and realm links; reject any link", False, True, True, True),
    ("Reports, forum moderation, profile resets, kicks and mutes in the game", False, True, True, True),
    ("Approve links with a warning (not a repo); catalog approvals, featured, News", False, False, True, True),
    ("Site-wide sanctions, game bans, realm and relay blocks; propose upgrades", False, False, True, True),
    ("Approve looks and HD packs; undo any decision", False, False, False, True),
    ("Approve or hold upgrades; the release channel and the minimum launcher version", False, False, False, True),
    ("Staff roles: grant and revoke; flags; notes to Claude", False, False, False, True),
    ("See secrets, keys or emails; deploy", False, False, False, False),
)


def setup(admin) -> None:
    global A
    A = admin
    A.ACTIONS.update({
        "submit.note": (a_submit_note, "helper", None),
        "submit.approve": (a_submit("approve"), "mod", None),
        "submit.reject": (a_submit("reject"), "mod", None),
        "submit.undo": (a_submit("reopen"), "owner", None),
        "feedback.run": (a_feedback_run, "admin", None),
        "upgrade.propose": (a_upgrade_propose, "admin", None),
        "upgrade.approve": (a_upgrade_decide("approved"), "owner", None),
        "upgrade.hold": (a_upgrade_decide("held"), "owner", None),
        "upgrade.reopen": (a_upgrade_decide("proposed"), "owner", None),
        "version.set": (a_version_set, "owner", "set"),
    })


def P(name):
    return A.P(name)


def lq():
    return P("linkqueue")


def fb():
    return P("feedback")


def ops_dir() -> str:
    fn = P("ops_dir")
    return fn() if callable(fn) else os.path.join(os.environ.get("WU_PORTAL_DATA", "/data"), "ops")


def _yes(v: bool) -> str:
    return A._tags(("yes", "good")) if v else '<span class="sub">no</span>'


# ------------------------------------------------------------------------------------------- who may approve what

def may_approve(user: dict, it: dict) -> str:
    """"" when this person may approve this link, else why not (the server says it too)."""
    role = user.get("role", "")
    if it.get("kind") == "look" and role != "owner":
        return "Looks and HD packs are approved by the owner."
    if [w for w in it.get("warnings") or [] if not w.startswith("a look")] and A.rank(role) < A.rank("admin"):
        return "Links with a warning need an admin or the owner."
    if A.rank(role) < A.rank("mod"):
        return "Moderators and up approve links."
    return ""


def _hub_record(it: dict, decision: str, user: dict, reason: str) -> None:
    """A forge repo approved here is approved in the community index too (hub/decisions.jsonl), so the daily index
    update lists it; an undo delists it there."""
    if not it.get("forge") and it.get("via") != "index":
        return
    if it.get("kind") not in ("mod", "addon"):
        return
    A._hub_append({"id": it["id"], "decision": decision, "repo": str(it.get("url") or "")[:300],
                   "build": str(it.get("build") or "any")[:10],
                   "category": str(it.get("category") or ("addons" if it.get("kind") == "addon" else ""))[:32],
                   "reason": reason, "by": user["account"], "at": A._iso(), "via": "review"})


def a_submit(decision):
    def fn(user, target, form):
        q = lq()
        if q is None:
            raise A.Refused("Submit a link isn't on this server.")
        it = q.item(target)
        if it is None:
            raise A.Refused("No such submission.")
        reason = A._clean_line(form.get("reason"), 200)
        if decision == "approve":
            if it["status"] == "approved":
                raise A.Refused("Already approved.")
            why = may_approve(user, it)
            if why:
                raise A.Refused(why)
        elif decision == "reject":
            if it["status"] == "rejected":
                raise A.Refused("Already turned down.")
            if not reason:
                raise A.Refused("Say why (the person may ask).")
        elif it["status"] == "pending":
            raise A.Refused("It is still waiting; nothing to undo.")
        try:
            before, after = q.decide(target, decision, user, reason)
        except ValueError as ex:
            raise A.Refused(str(ex)) from None
        if decision == "approve":
            _hub_record(it, "approve", user, reason)
        elif decision == "reject" and it.get("via") == "index":
            _hub_record(it, "reject", user, reason)
        elif decision == "reopen" and it["status"] == "approved":
            _hub_record(it, "delist", user, "undone in Review")
        return before, after
    return fn


def a_submit_note(user, target, form):
    q = lq()
    if q is None or q.item(target) is None:
        raise A.Refused("No such submission.")
    text = A._clean_line(form.get("text"), 300)
    if not text:
        raise A.Refused("Write the note first.")
    A._notes_write("sub:" + target, lambda n: n.setdefault("notes", []).append(
        {"by": user["name"], "role": user["role"], "at": A._iso(), "text": text}) or {})
    return {}, {"note": text}


# ------------------------------------------------------------------------------------------- feedback

def a_feedback_run(user, target, form):
    f = fb()
    if f is None:
        raise A.Refused("The feedback digest isn't on this server.")
    d = f.digest(os.environ.get("WU_PORTAL_DATA", "/data"), ops_dir(), os.environ.get("WU_FEEDBACK_WU", ""),
                 lex=f.load_scorer(os.environ.get("WU_SENTIMENT_LEXICON", ""), os.environ.get("WU_SENTIMENT_SCORER", "")))
    return {}, {"items": d["items"], "themes": len(d["themes"]), "mood": d["mood"]}


def window() -> dict | None:
    f = fb()
    if f is None:
        return None
    try:
        return f.release_window(ops_dir())
    except Exception:   # noqa: BLE001
        return None


# ------------------------------------------------------------------------------------------- upgrades and versions

def upgrades() -> dict:
    doc = A._read_json(A._apath("upgrades.json"), {})
    items = doc.get("items") if isinstance(doc.get("items"), dict) else {}
    return {k: v for k, v in items.items() if UP_ID_RE.fullmatch(k) and isinstance(v, dict)}


def _upgrades_write(fn):
    with A._lock:
        doc = A._read_json(A._apath("upgrades.json"), {})
        items = doc.setdefault("items", {})
        out = fn(items)
        doc["format"] = 1
        A._write_json(A._apath("upgrades.json"), doc, 0o640)
    return out


def a_upgrade_propose(user, target, form):
    title = A._no_brand(A._clean_line(form.get("title"), 100))
    if len(title) < 4:
        raise A.Refused("Give it a short title.")
    kind = form.get("kind") if form.get("kind") in dict(UPGRADE_KINDS) else "other"
    version = str(form.get("version") or "").strip()
    if version and not A.VERSION_RE.fullmatch(version):
        raise A.Refused("A version looks like 1.2.3.")
    rec = {"title": title, "what": A._clean_line(form.get("what"), 600), "kind": kind, "version": version,
           "by": user["account"], "by_name": user["name"], "at": A._iso(), "status": "proposed"}
    uid = "u" + time.strftime("%y%m%d%H%M%S", time.gmtime()) + secrets.token_hex(2)
    _upgrades_write(lambda items: items.__setitem__(uid, rec))
    return {}, {"id": uid, **rec}


def a_upgrade_decide(status):
    def fn(user, target, form):
        def change(items):
            it = items.get(target)
            if not isinstance(it, dict):
                raise A.Refused("No such proposal.")
            if it.get("status") == status:
                raise A.Refused(f"Already {status}.")
            before = dict(it)
            w = window() or {}
            it.update(status=status, decided_by=user["name"], decided_at=A._iso(),
                      note=A._clean_line(form.get("note"), 200),
                      ship_window=(w.get("text", "") + " (next " + w.get("next_start", "") + ")") if status == "approved" and w else "")
            return before, dict(it)
        return _upgrades_write(change)
    return fn


def policy() -> dict:
    doc = A._read_json(A._lpath("version-policy.json"), {})
    mn = doc.get("minimum") if isinstance(doc.get("minimum"), str) and A.VERSION_RE.fullmatch(doc.get("minimum")) else None
    return {"minimum": mn, "notice": str(doc.get("notice") or "")[:200], "by": str(doc.get("by") or ""),
            "at": doc.get("at") if isinstance(doc.get("at"), str) else None}


def _vkey(v: str):
    m = re.match(r"^(\d+)\.(\d+)\.(\d+)", v or "")
    return tuple(int(x) for x in m.groups()) if m else (0, 0, 0)


def a_version_set(user, target, form):
    mn = str(form.get("minimum") or "").strip()
    if mn and not A.VERSION_RE.fullmatch(mn):
        raise A.Refused("A version looks like 1.2.3 (empty: no minimum).")
    cur = (A.channel_doc().get("stable") or {}).get("version")
    if mn and cur and _vkey(mn) > _vkey(cur):
        raise A.Refused(f"The minimum can't be newer than the current stable version ({cur}).")
    notice = A._no_brand(A._clean_line(form.get("notice"), 200))
    with A._lock:
        before = A._read_json(A._lpath("version-policy.json"), {})
        after = {"minimum": mn or None, "notice": notice, "by": user["name"], "at": A._iso()}
        os.makedirs(A.leyline_dir(), exist_ok=True)
        A._write_json(A._lpath("version-policy.json"), after)
    return before, after


def version_doc() -> dict:
    ch = A.channel_doc()
    pol = policy()
    w = window() or {}
    return {"format": 1, "updated_at": pol["at"] or ch.get("updated_at"),
            "current": (ch.get("stable") or {}).get("version"), "beta": (ch.get("beta") or {}).get("version"),
            "minimum": pol["minimum"], "notice": pol["notice"], "hold": ch.get("hold", False),
            "window": {"text": w.get("text"), "next_start": w.get("next_start")} if w else None,
            "releases": A.SITE_RELEASES}


# ------------------------------------------------------------------------------------------- pages

def submissions_page(user: dict, query: dict) -> str:
    q = lq()
    if q is None:
        return '<section class="ly-panel"><h2>Submissions</h2><p class="fine">Submit a link (linkqueue.py) isn\'t on this server.</p></section>'
    back = "/wrath/admin/submissions"
    notes = A.notes()
    c = q.counts()
    rows = []
    for it in q.items("pending")[:200]:
        why = may_approve(user, it)
        acts = (A.act(user, "submit.approve", it["id"], "Approve", back) if not why else f'<span class="sub">{e(why)}</span>')
        acts += A.act(user, "submit.reject", it["id"], "Reject", back,
                      '<input type="text" name="reason" maxlength="200" placeholder="why" aria-label="Why" required>')
        acts += A.act(user, "submit.note", it["id"], "Add note", back,
                      '<input type="text" name="text" maxlength="300" placeholder="a note for the team" aria-label="Note" required>')
        nl = "".join(f'<span class="sub">{e(n.get("by", ""))} ({e(n.get("role", ""))}): {e(n.get("text", ""))}</span>'
                     for n in (notes.get("sub:" + it["id"]) or {}).get("notes", [])[-5:])
        warn = A._tags(*[(w, "bad") for w in it.get("warnings") or []])
        who = e(it.get("by_name", "")) + (f' <code>{e(it["acct"])}</code>' if it.get("acct") else "")
        rows.append(f'<tr><td class="name">{A._tags((it.get("kind", ""), "good"))} <a href="{e(it.get("url", ""))}" '
                    f'rel="noopener noreferrer nofollow" target="_blank">{e(it.get("url", ""))}</a>'
                    f'<span class="sub">{e(it.get("at", ""))} · via {e(it.get("via", ""))} · {who}</span>'
                    f'{("<span class=sub>" + e(it["note"]) + "</span>") if it.get("note") else ""}{nl}</td>'
                    f'<td>{warn}</td><td>{acts}</td></tr>')
    done = []
    for it in [r for r in q.items() if r["status"] != "pending" and r.get("decision")][:50]:
        d = it["decision"]
        undo = A.act(user, "submit.undo", it["id"], "Undo", back)
        done.append(f'<tr><td class="name">{A._tags((it.get("kind", ""), ""))} {e(it.get("url", ""))}'
                    f'<span class="sub">{e(d.get("at", ""))}</span></td>'
                    f'<td>{A._tags((it["status"], "good" if it["status"] == "approved" else "bad"))} by {e(d.get("by_name", ""))} '
                    f'({e(d.get("role", ""))}){(" · " + e(d["reason"])) if d.get("reason") else ""}</td><td>{undo}</td></tr>')
    return (f'<section class="ly-panel"><h2>Submissions</h2><p class="fine">Links people sent from the website '
            f'(<a href="/wrath/submit">/wrath/submit</a>), the Discord (/submit) and the hub\'s Submit a mod. Nothing shows '
            f'anywhere until it is approved here. {c["pending"]} waiting, {c["approved"]} approved, {c["rejected"]} turned down. '
            f'Approving a repo also lists it in the community index. Looks are the owner\'s to approve; links with a '
            f'warning need an admin.</p>'
            + (f'<table class="ly-table"><thead><tr><th>Link</th><th>Checks</th><th></th></tr></thead><tbody>{"".join(rows)}</tbody></table>'
               if rows else '<p class="fine">Nothing waiting.</p>') + '</section>'
            + '<section class="ly-panel"><h2>Recently decided</h2><p class="fine">Every decision is in the audit log; the owner can undo any '
              'of them (it goes back to waiting, and a repo leaves the index).</p>'
            + (f'<table class="ly-table"><tbody>{"".join(done)}</tbody></table>' if done else '<p class="fine">None yet.</p>')
            + '</section>')


def feedback_page(user: dict) -> str:
    f = fb()
    if f is None:
        return '<section class="ly-panel"><h2>Feedback</h2><p class="fine">The feedback digest (feedback.py) isn\'t on this server.</p></section>'
    d = f.latest(ops_dir())
    run = A.act(user, "feedback.run", "now", "Run now", "/wrath/admin/feedback")
    if d is None:
        return (f'<section class="ly-panel"><h2>Feedback</h2><p class="fine">No digest yet. The portal makes one every '
                f'{e(os.environ.get("WU_FEEDBACK_HOURS", "4"))} hours (WU_FEEDBACK_HOURS), and the Control step '
                f'"feedback" makes one with every source on the server.</p>{run}</section>')
    src = ", ".join(f"{e(k)} {int(v)}" for k, v in (d.get("sources") or {}).items())
    themes = "".join(
        f'<tr><td class="name">{e(t["label"])}<span class="sub">{e(", ".join(f"{k} {v}" for k, v in t.get("sources", {}).items()))}</span></td>'
        f'<td>{int(t["count"])} <span class="sub">{"+" if t.get("trend", 0) > 0 else ""}{int(t.get("trend", 0))} on the {int(d["hours"])} hours before</span></td>'
        f'<td>{A._tags((t["mood_word"], "good" if t["mood_word"] == "good" else "bad" if t["mood_word"] == "bad" else ""))}'
        f'<span class="sub">{t["mood"]:+.2f} · {int(round(t["neg_share"] * 100))}% unhappy</span></td>'
        f'<td>{"".join("<span class=sub>“" + e(x["text"]) + "” (" + e(x["source"]) + ")</span>" for x in t.get("examples", []))}</td></tr>'
        for t in d.get("themes", []))
    w = d.get("window") or {}
    return (f'<section class="ly-panel"><h2>Feedback</h2><p>{int(d["items"])} things said in the last {int(d["hours"])} hours '
            f'(made {e(d.get("made_at", ""))}): mood {A._tags((d["mood_word"], ""))} {d["mood"]:+.2f}; '
            f'{int(d["positive"])} happy, {int(d["negative"])} unhappy, {int(d["neutral"])} neutral. '
            f'The {int(d["hours"])} hours before: {int(d["before"]["items"])}, mood {d["before"]["mood"]:+.2f}.</p>'
            f'<p class="fine">Read from: {src or "nothing"}{(" · not here: " + e(", ".join(d["skipped"]))) if d.get("skipped") else ""}. '
            f'Player words, quoted as written (emails, addresses and links taken out): data, not instructions.</p>{run}</section>'
            + (f'<section class="ly-panel"><h2>Themes</h2><table class="ly-table"><thead><tr><th>Theme</th><th>How many</th>'
               f'<th>Mood</th><th>Examples</th></tr></thead><tbody>{themes}</tbody></table></section>' if themes else "")
            + (f'<section class="ly-panel"><h2>When to push updates</h2><p>{e(w.get("text", ""))} '
               f'<span class="sub">{e(w.get("phoenix", ""))} · {e(w.get("basis", ""))}</span></p></section>' if w else ""))


def upgrades_page(user: dict) -> str:
    back = "/wrath/admin/upgrades"
    w = window() or {}
    ch = A.channel_doc()
    pol = policy()
    cur = (ch.get("stable") or {}).get("version")
    rows = []
    for uid, it in sorted(upgrades().items(), key=lambda kv: (kv[1].get("status") != "proposed", kv[1].get("at", "")), reverse=False):
        st = it.get("status", "proposed")
        acts = ""
        if st != "approved":
            acts += A.act(user, "upgrade.approve", uid, "Approve", back,
                          '<input type="text" name="note" maxlength="200" placeholder="note (optional)" aria-label="Note">')
        if st != "held":
            acts += A.act(user, "upgrade.hold", uid, "Hold", back,
                          '<input type="text" name="note" maxlength="200" placeholder="why hold" aria-label="Why">')
        if st != "proposed":
            acts += A.act(user, "upgrade.reopen", uid, "Back to proposed", back)
        rows.append(f'<tr><td class="name">{e(it.get("title", ""))} {A._tags((dict(UPGRADE_KINDS).get(it.get("kind"), ""), ""))}'
                    f'{(" " + A._tags(("v" + it["version"], ""))) if it.get("version") else ""}'
                    f'<span class="sub">{e(it.get("what", ""))}</span><span class="sub">proposed by {e(it.get("by_name", ""))} '
                    f'{e(it.get("at", ""))}</span></td>'
                    f'<td>{A._tags((st, "good" if st == "approved" else "bad" if st == "held" else ""))}'
                    f'{("<span class=sub>" + e(it.get("decided_by", "")) + " " + e(it.get("decided_at", "")) + "</span>") if it.get("decided_by") else ""}'
                    f'{("<span class=sub>" + e(it["note"]) + "</span>") if it.get("note") else ""}'
                    f'{("<span class=sub>ship in: " + e(it["ship_window"]) + "</span>") if it.get("ship_window") else ""}</td>'
                    f'<td>{acts}</td></tr>')
    ok, rels, _ = A.releases()
    pre = next((r for r in rels if r.get("prerelease") and not r.get("draft")), None) if ok else None
    hint = (f'<p class="fine">On GitHub: {e(pre.get("version", ""))} is a pre-release newer than stable; '
            f'the <a href="/wrath/admin/releases">Releases</a> page moves it to stable.</p>'
            if pre and cur and _vkey(pre.get("version", "")) > _vkey(cur) else "")
    propose = A.act(user, "upgrade.propose", "new", "Propose", back,
                    '<input type="text" name="title" maxlength="100" placeholder="what changes, in a few words" aria-label="Title" required>'
                    '<select name="kind" aria-label="Kind">' + "".join(f'<option value="{k}">{e(v)}</option>' for k, v in UPGRADE_KINDS) + '</select>'
                    '<input type="text" name="version" maxlength="40" placeholder="version (optional)" aria-label="Version" size="10">'
                    '<input type="text" name="what" maxlength="600" placeholder="what it brings, what could break" aria-label="What">', block=True)
    vform = A.act(user, "version.set", "policy", "Save", back,
                  f'<input type="text" name="minimum" maxlength="40" value="{e(pol["minimum"] or "")}" placeholder="minimum, e.g. 0.4.0" aria-label="Minimum version" size="12">'
                  f'<input type="text" name="notice" maxlength="200" value="{e(pol["notice"])}" placeholder="a line launchers show (optional)" aria-label="Notice">',
                  confirm="set", block=True)
    return (f'<section class="ly-panel"><h2>When to push updates</h2>'
            + (f'<p><b>{e(w.get("text", ""))}</b> <span class="sub">{e(w.get("phoenix", ""))}</span>'
               f'<span class="sub">{e(w.get("basis", ""))}; next {e(w.get("next_start", ""))}</span></p>' if w else
               '<p class="fine">No player data on this server yet (feedback.py isn\'t here).</p>')
            + '<p class="fine">Ship launcher releases, site deploys and realm changes in this window, every week, so the launchers '
              'everyone has move together while the fewest people are playing.</p></section>'
            + f'<section class="ly-panel"><h2>Waiting for the owner</h2>{hint}'
            + (f'<table class="ly-table"><tbody>{"".join(rows)}</tbody></table>' if rows else '<p class="fine">No proposals.</p>')
            + propose + '</section>'
            + f'<section class="ly-panel"><h2>Launcher versions</h2><p>Stable: <b>{e(cur or "none yet")}</b>'
              f'{" · on hold" if ch.get("hold") else ""} · minimum: <b>{e(pol["minimum"] or "none")}</b>'
              f'{(" · set by " + e(pol["by"]) + " " + e(pol["at"] or "")) if pol["by"] else ""}</p>'
              f'<p class="fine">Published at <a href="{VERSION_FEED}">{VERSION_FEED}</a>: a launcher older than the minimum '
              f'asks its player to update before playing; older than stable, it offers the update '
              f'(docs/LAUNCHER-VERSION-POLICY.md). Lowering or clearing the minimum is always safe.</p>{vform}</section>')


def role_table() -> str:
    head = "".join(f"<th>{r}</th>" for r in ("Helper", "Mod", "Admin", "Owner"))
    rows = "".join(f'<tr><td class="name">{e(w)}</td>' + "".join(f"<td>{_yes(x)}</td>" for x in xs) + "</tr>"
                   for w, *xs in ROLE_TABLE)
    return (f'<h3>What each role can do</h3><table class="ly-table"><thead><tr><th></th>{head}</tr></thead><tbody>{rows}'
            f'</tbody></table><p class="fine">To recruit someone: they log in on the website with their Hero Key and open '
            f'/wrath/admin. The staff-only page shows their account id; they send it to you, and you grant a role here. '
            f'Every staff action is in the audit log with who did it, and the owner can undo approvals.</p>')


def page(user: dict, tab: str, query: dict):
    """(status, title, body) for the Review pages, or None. A helper's home is Submissions."""
    if tab in ("review", "submissions") or (tab == "" and user.get("role") == "helper"):
        return 200, "Submissions", submissions_page(user, query)
    if tab == "feedback":
        return 200, "Feedback", feedback_page(user)
    if tab == "upgrades":
        return 200, "Upgrades", upgrades_page(user)
    return None
