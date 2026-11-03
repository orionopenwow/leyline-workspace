"""Den of the week (owner 2026-10-31, the growth plan's "Show it off"): staff pick one player's Den a week in HQ, and it
shows on the site's front page, on the launcher's Home and in the in-game Leyline window. Screenshots are links only:
the pick never carries a picture, and nothing here fetches one.

    the pick       $WU_DEN_WEEK_FILE, default <HQ data>/den-week.json (stream/hq.py hq_dir):
                   {"format": 1, "pick": {title, by, line, shots: [https links], visit: link or "", week}, "at", "by_staff"}
    HQ             Home: a "Den of the week" panel (staff: mod and up): the pick now, and a form to set or clear it;
                   actions den.week.set / den.week.clear (admin.ACTIONS, audited like every action)
    the site       section_html(): the front page's card (stream/hub.py): title, by, line, the screenshot links, Visit
    the launcher   merge_home(body): the site's /wrath/art/home/home.json (the one address the launcher may read)
                   gets "den_week": the pick (stream/wrath_portal.py art_bytes); launcher/ui/home.js shows it
    in game        realm/custom/leyline-fetch.py reads den_week from that home.json into leyline-realms.dat;
                   ov_leyline.lua sends it to the Leyline window (WrathUnleashedUI/Leyline.lua)

Every field is cleaned when set and again when read: plain text, short, no brand words, no client numbers; links only
https (screenshots) or https / leyline://realm/<id> (Visit). Standard library only.
"""
from __future__ import annotations

import html
import json
import os
import re
import time
import unicodedata

e = html.escape
A = None   # stream/admin.py, when HQ loads this

FORMAT = 1
TITLE_MAX, BY_MAX, LINE_MAX, SHOTS_MAX, LINK_MAX = 40, 24, 140, 4, 300
BANNED_RE = re.compile(
    r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard)"
    r"|(?i:(?<![a-z])wow(?![a-z]))"
    r"|WoW|WOW|(?<![a-z])Wow(?=[A-Z0-9])|(?<=[a-z])Wow(?![a-z])")
VERSION_RE = re.compile(r"(?<![\w.])v?(?:1\.12(?:\.\d)?|2\.4\.3|3\.3\.5a?|4\.3\.4|5\.4\.8)(?![\w]|\.\d)", re.I)
SHOT_RE = re.compile(r"^https://[A-Za-z0-9.-]{1,120}(?::\d{1,5})?/[A-Za-z0-9._~:/?#\[\]@!$&'()*+,;=%-]{0,250}$")
VISIT_RE = re.compile(r"^(?:https://[A-Za-z0-9.-]{1,120}(?::\d{1,5})?/[A-Za-z0-9._~:/?#\[\]@!$&'()*+,;=%-]{0,250}"
                      r"|leyline://realm/[a-z2-7]{16})$")
CTRL_RE = re.compile(r"[\x00-\x1f\x7f-\x9f​-‏‪-‮⁦-⁩]")


class Refused(Exception):
    pass


def _refused(msg):
    return (A.Refused if A is not None and hasattr(A, "Refused") else Refused)(msg)


def path() -> str:
    p = os.environ.get("WU_DEN_WEEK_FILE")
    if p:
        return p
    base = os.environ.get("WU_HQ_DIR") or os.path.join(os.environ.get("WU_PORTAL_DATA", "/data"), "hq")
    return os.path.join(base, "den-week.json")


def text(v, n: int) -> str:
    """Plain, short text: no control characters, one line, at most n characters; "" for a brand word or a client number."""
    v = unicodedata.normalize("NFC", str(v or ""))
    v = CTRL_RE.sub(" ", v)
    v = re.sub(r"\s+", " ", v).strip()[:n].strip()
    if BANNED_RE.search(v) or VERSION_RE.search(v):
        return ""
    return v


def links(v, rx, n: int) -> list[str]:
    raw = v if isinstance(v, list) else str(v or "").split()
    out = []
    for x in raw[: n * 3]:
        x = str(x).strip()
        if len(x) <= LINK_MAX and rx.fullmatch(x) and x not in out and not BANNED_RE.search(x):
            out.append(x)
        if len(out) >= n:
            break
    return out


def clean(d) -> dict | None:
    """A pick, cleaned: {title, by, line, shots, visit, week}; None when it has no title or no maker."""
    if not isinstance(d, dict):
        return None
    t, by = text(d.get("title"), TITLE_MAX), text(d.get("by"), BY_MAX)
    if not t or not by:
        return None
    visit = links([d.get("visit")] if d.get("visit") else [], VISIT_RE, 1)
    week = str(d.get("week") or "")
    return {"title": t, "by": by, "line": text(d.get("line"), LINE_MAX), "shots": links(d.get("shots"), SHOT_RE, SHOTS_MAX),
            "visit": visit[0] if visit else "", "week": week if re.fullmatch(r"\d{4}-\d{2}-\d{2}", week) else ""}


def current() -> dict | None:
    """The pick now, cleaned again (a file edited by hand is held to the same rules), or None."""
    try:
        with open(path(), encoding="utf-8") as f:
            doc = json.loads(f.read(64_000))
    except (OSError, ValueError):
        return None
    if not isinstance(doc, dict) or doc.get("format") != FORMAT:
        return None
    return clean(doc.get("pick"))


def save(pick: dict | None, staff: str = "") -> None:
    p = path()
    os.makedirs(os.path.dirname(p) or ".", exist_ok=True)
    doc = {"format": FORMAT, "pick": pick, "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "by_staff": str(staff)[:64]}
    tmp = p + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, indent=1)
    os.replace(tmp, p)


# ----------------------------------------------------------------------------------------------
# The launcher's Home (and the in-game window, through leyline-fetch.py): home.json with "den_week"
# ----------------------------------------------------------------------------------------------
def merge_home(body: bytes | None) -> bytes | None:
    """The site's home.json with the pick added as "den_week" (or the pick alone when there is no home.json). Never
    raises: a bad file stays what it was."""
    try:
        pick = current()
    except Exception:
        pick = None
    if pick is None:
        return body
    try:
        doc = json.loads(body.decode("utf-8")) if body else {}
    except (ValueError, UnicodeDecodeError):
        return body
    if not isinstance(doc, dict):
        return body
    doc["den_week"] = pick
    return json.dumps(doc, ensure_ascii=False).encode("utf-8")


# ----------------------------------------------------------------------------------------------
# The site's front page
# ----------------------------------------------------------------------------------------------
CSS = """
.dw{margin:28px auto;max-width:1100px;padding:0 16px}
.dw-card{display:grid;gap:6px;padding:18px 20px;border-radius:14px;border:1px solid var(--hb-line,rgba(255,255,255,.12));
background:var(--hb-panel,rgba(255,255,255,.04))}
.dw-k{font:700 11px/1 inherit;letter-spacing:.14em;text-transform:uppercase;opacity:.75;margin:0}
.dw-card h2{margin:0;font-size:22px}.dw-by{margin:0;opacity:.8}.dw-line{margin:2px 0 0}
.dw-acts{display:flex;flex-wrap:wrap;gap:8px 14px;margin-top:6px;align-items:center}
.dw-acts a{text-decoration:underline;text-underline-offset:3px}
.dw-acts a.dw-visit{text-decoration:none;padding:6px 14px;border-radius:999px;border:1px solid currentColor;font-weight:600}
"""


def section_html(pick: dict | None = None) -> str:
    """The front page's "Den of the week" card, or "" when there is no pick."""
    pick = pick if pick is not None else current()
    if not pick:
        return ""
    shots = "".join(f'<a href="{e(u)}" rel="noopener nofollow ugc" target="_blank">Screenshot {i}</a>'
                    for i, u in enumerate(pick["shots"], 1))
    visit = (f'<a class="dw-visit" href="{e(pick["visit"])}" rel="noopener nofollow">Visit</a>' if pick["visit"] else "")
    return (f'<section class="dw" aria-labelledby="dw-h"><style>{CSS}</style><div class="dw-card">'
            f'<p class="dw-k">Den of the week</p><h2 id="dw-h">{e(pick["title"])}</h2>'
            f'<p class="dw-by">by {e(pick["by"])}</p>'
            + (f'<p class="dw-line">{e(pick["line"])}</p>' if pick["line"] else "")
            + (f'<div class="dw-acts">{visit}{shots}</div>' if (visit or shots) else "")
            + '</div></section>')


# ----------------------------------------------------------------------------------------------
# HQ: the panel and its two actions
# ----------------------------------------------------------------------------------------------
def a_set(user, target, form):
    d = {"title": form.get("title"), "by": form.get("by"), "line": form.get("line"),
         "shots": str(form.get("shots") or "").split(), "visit": str(form.get("visit") or "").strip(),
         "week": time.strftime("%Y-%m-%d", time.gmtime())}
    if form.get("visit") and str(form.get("visit")).strip() and not links([str(form.get("visit")).strip()], VISIT_RE, 1):
        raise _refused("The Visit link must be an https link or a leyline://realm/ link.")
    pick = clean(d)
    if not pick:
        raise _refused("A Den of the week needs a name and its maker (plain words, no game names).")
    before = current()
    save(pick, (user or {}).get("account", ""))
    return before, pick


def a_clear(user, target, form):
    before = current()
    if before is None:
        raise _refused("There is no Den of the week to take down.")
    save(None, (user or {}).get("account", ""))
    return before, None


def panel_html(user: dict, back: str) -> str:
    """HQ Home's panel: the pick now and, for staff who may set it, the form."""
    if A is None:
        return ""
    pick = current()
    now = (f'<p><b>{e(pick["title"])}</b> by {e(pick["by"])}' + (f' · {e(pick["week"])}' if pick["week"] else "") + '</p>'
           + (f'<p class="fine">{e(pick["line"])}</p>' if pick["line"] else "")
           + (f'<p class="fine">{len(pick["shots"])} screenshot link{"" if len(pick["shots"]) == 1 else "s"}'
              + (", a Visit link" if pick["visit"] else "") + '</p>')
           if pick else '<p class="fine">No Den picked this week.</p>')
    fields = ('<input name="title" maxlength="40" placeholder="The Den\'s name" aria-label="The Den\'s name" required>'
              '<input name="by" maxlength="24" placeholder="Who made it" aria-label="Who made it" required>'
              '<input name="line" maxlength="140" placeholder="One line about it" aria-label="One line about it">'
              '<textarea name="shots" maxlength="1300" placeholder="Screenshot links (https), one per line, up to 4" '
              'aria-label="Screenshot links"></textarea>'
              '<input name="visit" maxlength="300" placeholder="Visit link (optional): https or leyline://realm/..." aria-label="Visit link">')
    form = A.act(user, "den.week.set", "new", "Pick it", back, fields, block=True)
    clear = A.act(user, "den.week.clear", "pick", "Take it down", back) if pick else ""
    return (f'<section class="ly-panel" id="den-week"><h2>Den of the week</h2>{now}{clear}{form}'
            '<p class="fine">Shown on the front page, the launcher\'s Home and the in-game Leyline window. Screenshots are '
            'links only: nothing is copied or shown as a picture.</p></section>')


def setup(admin) -> None:
    global A
    A = admin
    admin.ACTIONS.update({
        "den.week.set": (a_set, "mod", None),
        "den.week.clear": (a_clear, "mod", None),
    })
