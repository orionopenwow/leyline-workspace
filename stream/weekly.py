"""Leyline weekly, v0 (the network plan's "weekly layer"): a "This week" row of featured picks, one per game mode, each
shown until its `featured_until`. The launcher's Home and the site's front page both show it.

    the data       $WU_WEEKLY_FILE, default stream/weekly.json beside this file:
                   {"format": 1, "items": [{"mode": "<playmodes.json id>", "title", "line", "visit": link or "",
                                            "featured_from": "<ISO time>", "featured_until": "<ISO time>"}]}
    the row        this_week(now): the items live now (featured_from <= now < featured_until), at most one per mode
                   (the one that started last), soonest-ending last, at most ROW_MAX:
                   [{"mode", "title", "line", "visit", "until": <unix seconds>}]
    the launcher   merge_home(body): the site's /wrath/art/home/home.json (the one address the launcher may read) gets
                   "this_week": {"format": 1, "at": <unix>, "items": [...]} (stream/wrath_portal.py den_week_home);
                   launcher/ui/home.js shows it as the THIS WEEK row and hides cards whose time has passed
    the site       section_html(): the front page's "This week" strip (stream/hub.py)

Every field is cleaned when read: plain short text, no brand words, no client numbers, no money links; links only
https or leyline://realm/<id>. Picking the week's items is a person's job (TODO(owner): who picks, and when the week
turns over). Standard library only.
"""
from __future__ import annotations

import calendar
import html
import json
import os
import re
import time
import unicodedata

e = html.escape
FORMAT = 1
ROW_MAX = 6
ITEMS_MAX = 64
TITLE_MAX, LINE_MAX, LINK_MAX = 40, 120, 300
MODE_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,39}$")
BANNED_RE = re.compile(r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard)|\bWoW\b|\bWOW\b")
VERSION_RE = re.compile(r"(?<![\w.])v?(?:1\.12(?:\.\d)?|2\.4\.3|3\.3\.5a?|4\.3\.4|5\.4\.8)(?![\w]|\.\d)", re.I)
MONEY_RE = re.compile(r"(?i)patreon\.|ko-?fi\.|paypal\.|buymeacoffee|liberapay|opencollective|github\.com/sponsors"
                      r"|\bdonat(?:e|ion)|\btip\s*jar")
VISIT_RE = re.compile(r"^(?:https://[A-Za-z0-9.-]{1,120}(?::\d{1,5})?/[A-Za-z0-9._~:/?#\[\]@!$&'()*+,;=%-]{0,250}"
                      r"|leyline://realm/[a-z2-7]{16})$")
CTRL_RE = re.compile(r"[\x00-\x1f\x7f-\x9f​-‏‪-‮⁦-⁩]")
HERE = os.path.dirname(os.path.abspath(__file__))


def path() -> str:
    return os.environ.get("WU_WEEKLY_FILE") or os.path.join(HERE, "weekly.json")


def text(v, n: int) -> str:
    """Plain, short, one-line text; "" for a brand word, a client number or a money link."""
    v = unicodedata.normalize("NFC", str(v or ""))
    v = re.sub(r"\s+", " ", CTRL_RE.sub(" ", v)).strip()[:n].strip()
    if BANNED_RE.search(v) or VERSION_RE.search(v) or MONEY_RE.search(v):
        return ""
    return v


def when(v) -> int | None:
    """Unix seconds from an ISO-8601 UTC time ("2026-11-16T00:00:00Z" or a date) or an integer; None otherwise."""
    if isinstance(v, int) and not isinstance(v, bool) and 0 < v < 2 ** 40:
        return v
    if not isinstance(v, str):
        return None
    for fmt in ("%Y-%m-%dT%H:%M:%SZ", "%Y-%m-%dT%H:%MZ", "%Y-%m-%d"):
        try:
            return calendar.timegm(time.strptime(v.strip(), fmt))
        except ValueError:
            continue
    return None


def clean(d) -> dict | None:
    """One item, cleaned, or None (no mode, no title, no end time, or an end before its start)."""
    if not isinstance(d, dict):
        return None
    mode = d.get("mode") if isinstance(d.get("mode"), str) and MODE_RE.match(d["mode"]) else None
    title = text(d.get("title"), TITLE_MAX)
    start, until = when(d.get("featured_from")) or 0, when(d.get("featured_until"))
    if not mode or not title or until is None or until <= start:
        return None
    visit = d.get("visit") if isinstance(d.get("visit"), str) else ""
    if not (len(visit) <= LINK_MAX and VISIT_RE.fullmatch(visit)) or BANNED_RE.search(visit) or MONEY_RE.search(visit):
        visit = ""
    return {"mode": mode, "title": title, "line": text(d.get("line"), LINE_MAX), "visit": visit,
            "from": start, "until": until}


def load() -> list[dict]:
    """Every clean item in the file ([] when it is missing or broken)."""
    try:
        with open(path(), encoding="utf-8") as f:
            doc = json.loads(f.read(256_000))
    except (OSError, ValueError):
        return []
    if not isinstance(doc, dict) or doc.get("format") != FORMAT or not isinstance(doc.get("items"), list):
        return []
    return [x for x in (clean(d) for d in doc["items"][:ITEMS_MAX]) if x]


def this_week(now: float | None = None, items: list | None = None) -> list[dict]:
    """The "This week" row: the items live now, one per mode (the one that started last), at most ROW_MAX."""
    now = time.time() if now is None else now
    best: dict[str, dict] = {}
    for it in load() if items is None else items:
        if it["from"] <= now < it["until"] and (it["mode"] not in best or it["from"] > best[it["mode"]]["from"]):
            best[it["mode"]] = it
    row = sorted(best.values(), key=lambda x: (-x["until"], x["mode"]))[:ROW_MAX]
    return [{"mode": x["mode"], "title": x["title"], "line": x["line"], "visit": x["visit"], "until": x["until"]}
            for x in row]


def merge_home(body: bytes | None, now: float | None = None) -> bytes | None:
    """The site's home.json with "this_week" added (or that alone when there is no home.json); unchanged when the row
    is empty. Never raises."""
    try:
        row = this_week(now)
    except Exception:   # noqa: BLE001
        row = []
    if not row:
        return body
    try:
        doc = json.loads(body.decode("utf-8")) if body else {}
    except (ValueError, UnicodeDecodeError):
        return body
    if not isinstance(doc, dict):
        return body
    doc["this_week"] = {"format": FORMAT, "at": int(time.time() if now is None else now), "items": row}
    return json.dumps(doc, ensure_ascii=False).encode("utf-8")


# ----------------------------------------------------------------------------------------------
# The site's front page
# ----------------------------------------------------------------------------------------------
CSS = """
.tw{margin:28px auto;max-width:1100px;padding:0 16px}
.tw-h{font:700 11px/1 inherit;letter-spacing:.14em;text-transform:uppercase;opacity:.75;margin:0 0 10px}
.tw-row{display:grid;gap:12px;grid-template-columns:repeat(auto-fill,minmax(220px,1fr))}
.tw-card{display:grid;gap:4px;padding:14px 16px;border-radius:12px;border:1px solid var(--hb-line,rgba(255,255,255,.12));
background:var(--hb-panel,rgba(255,255,255,.04))}
.tw-k{margin:0;font-size:11px;letter-spacing:.08em;text-transform:uppercase;opacity:.7}
.tw-card h3{margin:0;font-size:17px}.tw-card p{margin:0}.tw-until{font-size:12px;opacity:.7}
.tw-card a{text-decoration:underline;text-underline-offset:3px}
"""


def mode_names() -> dict:
    """playmodes.json's id -> title, for the cards' kicker (read only)."""
    try:
        with open(os.path.join(HERE, "playmodes.json"), encoding="utf-8") as f:
            return {m["id"]: text(m.get("title"), 40) for m in json.load(f).get("modes", [])
                    if isinstance(m, dict) and isinstance(m.get("id"), str)}
    except (OSError, ValueError, AttributeError):
        return {}


def section_html(row: list | None = None, now: float | None = None) -> str:
    """The front page's "This week" strip, or "" when nothing is featured."""
    row = this_week(now) if row is None else row
    if not row:
        return ""
    names = mode_names()
    cards = []
    for it in row:
        until = time.strftime("%a %d %b", time.gmtime(it["until"]))
        visit = it["visit"] if it["visit"].startswith("https://") else ""
        cards.append('<article class="tw-card">'
                     f'<p class="tw-k">{e(names.get(it["mode"]) or it["mode"].replace("-", " ").title())}</p>'
                     f'<h3>{e(it["title"])}</h3>'
                     + (f'<p>{e(it["line"])}</p>' if it["line"] else "")
                     + f'<p class="tw-until">Until {e(until)}</p>'
                     + (f'<p><a href="{e(visit)}" rel="noopener nofollow">Have a look</a></p>' if visit else "")
                     + '</article>')
    return (f'<section class="tw" aria-labelledby="tw-h"><style>{CSS}</style><h2 class="tw-h" id="tw-h">This week</h2>'
            f'<div class="tw-row">{"".join(cards)}</div></section>')
