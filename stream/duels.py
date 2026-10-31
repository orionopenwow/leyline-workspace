"""The duel leaderboards for the portal (stream/wrath_portal.py), in the Leyline hub's look (stream/hub.py).

Owner 2026-10-31: "Working in game duels points, leaderboards on site".

    GET /wrath/leaderboards                    this week's duel points (?board=week, the default), the all-time
                                               ratings (?board=all) and the best of each class (?board=class, one class:
                                               &class=<id>); plain links, no script
    GET /wrath/api/leaderboards                the same as JSON for the launcher (CORS *, cached a minute)

The data is one file, written every 5 minutes by realm/custom/duels-export.py (cron on the host, from ov_duels.lua's
tables; the portal never touches the database):

    <WU_DUELS_DIR>/leaderboards.json           (default /app/duels = /srv/wrath/portal/duels)

Privacy. The export already leaves out bots, deleted characters, characters whose player typed ".armory off", and every
account detail. This module checks again: only the fields in clean() are ever read (a name of plain 2-12 letters, class,
race and the numbers), so no account id, guid or anything else reaches a page or the JSON even if the file held it.
Names with a brand word in them are left out (stream/hub.py banned()). When the file is missing the page says the
boards start with the first rated duel. Standard library only; the portal runs without this file.
"""
from __future__ import annotations

import html
import json
import os
import re
import threading
import time
import urllib.parse

PAGE = "/wrath/leaderboards"
API = "/wrath/api/leaderboards"
DATA_DIR = os.environ.get("WU_DUELS_DIR", "/app/duels")
FILE = "leaderboards.json"
NAME_RE = re.compile(r"^[A-Za-z]{2,12}$")
WEEK_RE = re.compile(r"^\d{4}-\d{2}$")
DAY_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
MAX_FILE = 4_000_000
MAX_ROWS = 1000
SHOW = 100            # rows on a board
PER_CLASS = 5         # rows per class on the By class board
STAT_EVERY = 5.0      # seconds between checks of the file's mtime
STALE_S = 3 * 3600    # older than this: the page says the boards may be behind

CLASSES = {1: "Warrior", 2: "Paladin", 3: "Hunter", 4: "Rogue", 5: "Priest", 6: "Death Knight", 7: "Shaman",
           8: "Mage", 9: "Warlock", 11: "Druid"}
CLASS_COLOR = {1: "#c79c6e", 2: "#f58cba", 3: "#abd473", 4: "#fff569", 5: "#ffffff", 6: "#ff4d6a", 7: "#2f8fff",
               8: "#69ccf0", 9: "#a29bff", 11: "#ff7d0a"}
BOARDS = (("week", "This week"), ("all", "All time"), ("class", "By class"))
DUEL_CORNER = "/wrath/h/duels"     # the go link to the duel corner (stream/modes.py link("h", "duels"))

e = html.escape
_get = None


def bind(getter) -> None:
    global _get
    _get = getter


def P(name):
    try:
        return _get(name) if _get else None
    except KeyError:
        return None


# ----------------------------------------------------------------------------------------------
# data: whitelisted fields only
# ----------------------------------------------------------------------------------------------

def _n(v, lo=0, hi=100000) -> int:
    return v if isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi else 0


def _banned(text: str) -> bool:
    """stream/hub.py's brand rule (the one copy); a name it can't check is left out rather than shown."""
    hub = P("hub_mod")
    try:
        return bool(hub.banned(text)) if hub is not None else False
    except Exception:   # noqa: BLE001
        return True


def _row(r, fields) -> dict | None:
    if not isinstance(r, dict):
        return None
    name = r.get("name")
    if not isinstance(name, str) or not NAME_RE.fullmatch(name) or _banned(name):
        return None
    cls = r.get("class")
    out = {"name": name, "class": cls if cls in CLASSES else 0}
    for f in fields:
        out[f] = _n(r.get(f))
    return out


ALL_FIELDS = ("rating", "best", "wins", "losses", "points", "best_streak", "streak")
WEEK_FIELDS = ("points", "wins", "losses")


def clean(doc) -> dict:
    """The fields a board may show, checked. Anything else in the file is never read."""
    d = doc if isinstance(doc, dict) else {}

    def rows(key, fields):
        out, seen = [], set()
        for r in (d.get(key) if isinstance(d.get(key), list) else [])[:MAX_ROWS]:
            c = _row(r, fields)
            if c and c["name"].lower() not in seen:
                seen.add(c["name"].lower())
                out.append(c)
        return out
    week = d.get("week") if isinstance(d.get("week"), str) and WEEK_RE.fullmatch(d.get("week")) else ""
    day = lambda k: d.get(k) if isinstance(d.get(k), str) and DAY_RE.fullmatch(d.get(k)) else ""   # noqa: E731
    upd = d.get("updated")
    return {"updated": upd if isinstance(upd, int) and not isinstance(upd, bool) and upd > 0 else 0,
            "week": week, "week_start": day("week_start"), "week_end": day("week_end"),
            "rated_this_week": _n(d.get("rated_this_week"), 0, 10 ** 9), "placed_after": _n(d.get("placed_after"), 0, 100) or 3,
            "weekly": sorted(rows("weekly", WEEK_FIELDS), key=lambda r: (-r["points"], -r["wins"], r["name"].lower())),
            "last_week": sorted(rows("last_week", WEEK_FIELDS), key=lambda r: (-r["points"], -r["wins"], r["name"].lower()))[:3],
            "alltime": sorted(rows("alltime", ALL_FIELDS), key=lambda r: (-r["rating"], -r["wins"], r["name"].lower()))}


class Store:
    """leaderboards.json, re-read when it changes (checked at most every STAT_EVERY seconds)."""

    def __init__(self, root: str | None = None):
        self.root = root or DATA_DIR
        self._lock = threading.Lock()
        self._doc = clean({})
        self._mtime = None
        self._checked = 0.0

    def doc(self) -> dict:
        now = time.monotonic()
        with self._lock:
            if now - self._checked < STAT_EVERY and self._mtime is not None:
                return self._doc
            self._checked = now
            path = os.path.join(self.root, FILE)
            try:
                st = os.stat(path)
            except OSError:
                self._doc, self._mtime = clean({}), None
                return self._doc
            if st.st_mtime == self._mtime:
                return self._doc
            try:
                if st.st_size > MAX_FILE:
                    raise ValueError("too big")
                with open(path, encoding="utf-8") as f:
                    self._doc = clean(json.load(f))
                self._mtime = st.st_mtime
            except (OSError, ValueError):
                pass   # keep the last good one
            return self._doc


store = Store()


def api_doc(doc: dict | None = None) -> dict:
    """The JSON for the launcher: the boards, names only, and the by-class tops."""
    d = doc or store.doc()
    return {"updated": d["updated"], "week": d["week"], "week_start": d["week_start"], "week_end": d["week_end"],
            "rated_this_week": d["rated_this_week"], "placed_after": d["placed_after"],
            "weekly": d["weekly"][:SHOW], "last_week": d["last_week"], "alltime": d["alltime"][:SHOW],
            "by_class": {str(c): by_class(d, c)[:PER_CLASS] for c in CLASSES if by_class(d, c)},
            "classes": {str(k): v for k, v in CLASSES.items()}, "page": PAGE}


def by_class(d: dict, cls: int) -> list[dict]:
    return [r for r in d["alltime"] if r["class"] == cls]


# ----------------------------------------------------------------------------------------------
# the page
# ----------------------------------------------------------------------------------------------

CSS = """
.db-top{display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:10px 16px;margin:0 0 14px}
.db-facts{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 16px;padding:0;list-style:none}
.db-facts li{padding:8px 12px;border-radius:var(--lk-r-sm);border:1px solid var(--lk-border);background:var(--lk-surface-2);font-size:14px;color:var(--lk-text-2)}
.db-facts b{color:var(--lk-text);font-size:16px;margin-right:4px}
.db-wrap{overflow-x:auto;border:1px solid var(--lk-border);border-radius:var(--lk-r-sm);background:var(--lk-surface)}
.db-t{width:100%;border-collapse:collapse;font-size:14.5px;font-variant-numeric:tabular-nums}
.db-t th,.db-t td{padding:9px 12px;text-align:left;border-bottom:1px solid var(--lk-border);white-space:nowrap}
.db-t thead th{position:sticky;top:0;background:var(--lk-surface-2);color:var(--lk-muted);font-size:12px;font-weight:700;letter-spacing:.06em;text-transform:uppercase}
.db-t tbody tr:last-child td{border-bottom:0}
.db-t tbody tr:hover td{background:var(--lk-accent-soft)}
.db-t td.n,.db-t th.n{text-align:right}
.db-t td.rk{width:3.2em;color:var(--lk-muted);font-weight:700}
.db-t tr.p1 td.rk{color:var(--lk-gold)}.db-t tr.p2 td.rk,.db-t tr.p3 td.rk{color:var(--lk-text-2)}
.db-t td.nm a{color:var(--lk-text)!important;font-weight:600;text-decoration:none}.db-t td.nm a:hover{text-decoration:underline}
.db-t td.big{font-weight:700;color:var(--lk-text)}
.db-cl{display:inline-flex;align-items:center;gap:7px;color:var(--lk-text-2)}
.db-dot{width:10px;height:10px;border-radius:50%;flex:none;box-shadow:0 0 0 1px rgba(0,0,0,.35)}
.db-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:12px}
.db-card{border:1px solid var(--lk-border);border-radius:var(--lk-r-sm);background:var(--lk-surface);padding:12px 14px}
.db-card h3{display:flex;align-items:center;justify-content:space-between;gap:8px;margin:0 0 8px;font-size:15px}
.db-card h3 a{font-size:12.5px;font-weight:600}
.db-card ol{margin:0;padding:0 0 0 1.4em;font-size:14px;font-variant-numeric:tabular-nums}
.db-card li{margin:3px 0;color:var(--lk-text-2)}.db-card li b{color:var(--lk-text)}
.db-card li span{float:right;color:var(--lk-muted)}
.db-card p{margin:0;color:var(--lk-muted);font-size:13.5px}
.db-last{margin:0 0 16px;font-size:14px;color:var(--lk-text-2)}
.db-how{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:12px;margin:22px 0 0}
.db-how section{border:1px solid var(--lk-border);border-radius:var(--lk-r-sm);padding:12px 16px;background:var(--lk-surface-2)}
.db-how h2{margin:0 0 6px;font-size:15px;text-align:left}
.db-how ul{margin:0;padding-left:1.1em;font-size:14px;color:var(--lk-text-2)}.db-how li{margin:3px 0}
.db-how code{font-size:13px}
.db-empty{padding:22px 16px;text-align:center;color:var(--lk-muted)}
.db-chips{margin:0 0 14px}
@media (max-width:640px){.db-t th,.db-t td{padding:8px 8px}.db-t .hide-s{display:none}}
"""


def _when(t: int) -> str:
    if not t:
        return ""
    g = time.gmtime(t)
    return (f'Updated <time datetime="{time.strftime("%Y-%m-%dT%H:%M:%SZ", g)}">{time.strftime("%H:%M", g)} UTC</time>'
            + (" (the boards may be a little behind)" if time.time() - t > STALE_S else ""))


def _cls(c: int) -> str:
    if c not in CLASSES:
        return '<span class="db-cl">-</span>'
    return f'<span class="db-cl"><span class="db-dot" style="background:{CLASS_COLOR[c]}" aria-hidden="true"></span>{e(CLASSES[c])}</span>'


def _name(n: str) -> str:
    if P("armory") is not None:
        return f'<a href="/wrath/armory/{e(urllib.parse.quote(n))}">{e(n)}</a>'
    return e(n)


def _table(head, rows_html, label) -> str:
    th = "".join(f'<th scope="col"{f" class={c}" if c else ""}>{e(t)}</th>' for t, c in head)
    return (f'<div class="db-wrap"><table class="db-t" aria-label="{e(label)}"><thead><tr>{th}</tr></thead>'
            f'<tbody>{rows_html}</tbody></table></div>')


def _rank_cls(i: int) -> str:
    return f' class="p{i}"' if i <= 3 else ""


def week_board(d: dict) -> str:
    rows = d["weekly"][:SHOW]
    if not rows:
        return ('<div class="db-wrap"><p class="db-empty">No duel points yet this week. Win a duel at level 80 and you are '
                f'on the board. <a href="{DUEL_CORNER}">Go to the duel corner</a></p></div>')
    body = "".join(f'<tr{_rank_cls(i)}><td class="rk">{i}</td><td class="nm">{_name(r["name"])}</td><td>{_cls(r["class"])}</td>'
                   f'<td class="n big">{r["points"]}</td><td class="n">{r["wins"]}</td><td class="n hide-s">{r["losses"]}</td></tr>'
                   for i, r in enumerate(rows, 1))
    return _table((("#", ""), ("Name", ""), ("Class", ""), ("Points", "n"), ("Wins", "n"), ("Losses", "n hide-s")), body,
                  "This week's duel points")


def all_board(d: dict, rows=None, label="All-time duel ratings") -> str:
    rows = (d["alltime"] if rows is None else rows)[:SHOW]
    if not rows:
        return (f'<div class="db-wrap"><p class="db-empty">Nobody is on this board yet: it starts after {d["placed_after"]} rated '
                f'duels. <a href="{DUEL_CORNER}">Go to the duel corner</a></p></div>')
    body = "".join(f'<tr{_rank_cls(i)}><td class="rk">{i}</td><td class="nm">{_name(r["name"])}</td><td>{_cls(r["class"])}</td>'
                   f'<td class="n big">{r["rating"]}</td><td class="n hide-s">{r["best"]}</td><td class="n">{r["wins"]}-{r["losses"]}</td>'
                   f'<td class="n hide-s">{r["best_streak"]}</td></tr>' for i, r in enumerate(rows, 1))
    return _table((("#", ""), ("Name", ""), ("Class", ""), ("Rating", "n"), ("Best", "n hide-s"), ("Won-lost", "n"),
                   ("Best streak", "n hide-s")), body, label)


def class_board(d: dict, cls: int | None) -> str:
    chips = ('<p class="hb-chips db-chips">' + f'<a class="hb-chip{" on" if cls is None else ""}" href="{PAGE}?board=class">All classes</a>'
             + "".join(f'<a class="hb-chip{" on" if cls == c else ""}" href="{PAGE}?board=class&amp;class={c}">{e(n)}</a>'
                       for c, n in sorted(CLASSES.items(), key=lambda x: x[1])) + "</p>")
    if cls is not None:
        return chips + all_board(d, by_class(d, cls), f"{CLASSES[cls]} duel ratings")
    cards = []
    for c, n in sorted(CLASSES.items(), key=lambda x: x[1]):
        top = by_class(d, c)[:PER_CLASS]
        inner = (("<ol>" + "".join(f'<li><b>{e(r["name"])}</b><span>{r["rating"]}</span></li>' for r in top) + "</ol>") if top
                 else "<p>Nobody yet.</p>")
        cards.append(f'<article class="db-card"><h3>{_cls(c)}<a href="{PAGE}?board=class&amp;class={c}">See all</a></h3>{inner}</article>')
    return chips + f'<div class="db-grid">{"".join(cards)}</div>'


def how_it_works() -> str:
    return ('<div class="db-how">'
            '<section aria-labelledby="db-h1"><h2 id="db-h1">How it counts</h2><ul>'
            '<li>Duels between level 80 players count.</li>'
            '<li>A win is 3 points, a loss 1. Points start again every Monday (00:00 UTC).</li>'
            '<li>Everyone starts at a rating of 1500. Beat a higher rating and you gain more.</li>'
            '<li>The all-time board starts after 3 rated duels.</li></ul></section>'
            '<section aria-labelledby="db-h2"><h2 id="db-h2">What does not count</h2><ul>'
            '<li>Duels under 15 seconds, and duels someone runs out of early.</li>'
            '<li>Duels that get interrupted.</li>'
            '<li>Two characters on one account.</li>'
            '<li>The same two players more than 3 times a day.</li></ul></section>'
            '<section aria-labelledby="db-h3"><h2 id="db-h3">In game</h2><ul>'
            '<li><code>.hub duels</code> takes you to the duel corner in Dalaran.</li>'
            '<li><code>.duel stats</code> shows your rating, <code>.duel stats Name</code> someone else\'s.</li>'
            '<li><code>.duel top</code> this week, <code>.duel top all</code> all time.</li>'
            '<li>Health and cooldowns come back after every duel.</li></ul></section></div>')


def page_body(query: dict, d: dict | None = None) -> tuple[str, str]:
    """(title, body) for /wrath/leaderboards."""
    d = d or store.doc()
    board = (query.get("board") or ["week"])[0]
    board = board if board in dict(BOARDS) else "week"
    cls = None
    try:
        c = int((query.get("class") or [""])[0])
        cls = c if c in CLASSES else None
    except ValueError:
        pass
    realm = P("REALM_NAME") or "Wrath Unleashed"
    tabs = ('<nav class="hb-seg" aria-label="Boards">' + "".join(
        f'<a href="{PAGE}' + ("" if k == "week" else f"?board={k}") + '"' + (' class="on" aria-current="page"' if k == board else "")
        + f">{e(t)}</a>" for k, t in BOARDS) + "</nav>")
    facts = []
    if board == "week":
        if d["week_start"]:
            facts.append(f'<li><b>{e(d["week_start"])}</b>to {e(d["week_end"])}</li>')
        facts.append(f'<li><b>{d["rated_this_week"]}</b>rated duels this week</li>')
        facts.append(f'<li><b>{len(d["weekly"])}</b>players on the board</li>')
    elif board == "all":
        facts.append(f'<li><b>{len(d["alltime"])}</b>rated players</li>')
        if d["alltime"]:
            facts.append(f'<li><b>{max(r["best"] for r in d["alltime"])}</b>best rating ever</li>')
            facts.append(f'<li><b>{max(r["best_streak"] for r in d["alltime"])}</b>longest win streak</li>')
    last = ""
    if board == "week" and d["last_week"]:
        last = ('<p class="db-last">Last week: ' + ", ".join(
            f'{("1st", "2nd", "3rd")[i]} <b>{e(r["name"])}</b> ({r["points"]})' for i, r in enumerate(d["last_week"])) + "</p>")
    content = week_board(d) if board == "week" else all_board(d) if board == "all" else class_board(d, cls)
    when = _when(d["updated"])
    title = {"week": "This week", "all": "All time", "class": "By class"}[board]
    body = ('<section class="hb-pg db" aria-labelledby="hb-pg-h"><header class="hb-pg-h"><div class="hb-pg-t">'
            f'<h1 id="hb-pg-h">Leaderboards</h1>{tabs}</div>'
            f'<a class="hb-play hb-pg-x" href="{DUEL_CORNER}">Go duel</a></header><div class="hb-pg-b">'
            f'<p class="hb-lede">Duels at level 80 on {e(realm)}: points every week, and a rating that stays.</p>'
            + (f'<ul class="db-facts">{"".join(facts)}</ul>' if facts else "") + last + content
            + (f'<p class="hb-fine">{when}. The boards update every 5 minutes.</p>' if when else
               '<p class="hb-fine">The boards update every 5 minutes, starting with the first rated duel.</p>')
            + how_it_works() + "</div></section>")
    return f"Leaderboards · {title} · Leyline", body


# ----------------------------------------------------------------------------------------------
# the portal's hooks (stream/wrath_portal.py: bind, match, get)
# ----------------------------------------------------------------------------------------------

def match(path: str) -> bool:
    return path in (PAGE, API)


def _shell(title: str, body: str) -> tuple[bytes, dict]:
    """The page in the hub's frame (header, tokens, footer), else the portal's Leyline page, else a plain one."""
    hub = P("hub_mod")
    if hub is not None and getattr(hub, "hub", None) is not None:
        try:
            out = hub.hub._shell(title, body, PAGE, active="leaderboards", css=CSS)
            return out, ({} if hub.front_on() else {"X-Robots-Tag": "noindex, nofollow"})
        except Exception:   # noqa: BLE001: the plain page then
            pass
    shell = P("ly_shell")
    if shell is not None:
        try:
            return shell(title, body, PAGE, "", "play", CSS), {}
        except Exception:   # noqa: BLE001
            pass
    return (f"<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>"
            f"<title>{e(title)}</title><style>{CSS}</style><main>{body}</main></html>").encode("utf-8"), {}


def _limited(h) -> bool:
    limit, bucket = P("browse_limit"), P("rl_bucket")
    try:
        ip = h.ip()
        return limit is not None and not limit.hit(bucket(ip) if bucket else ip, 600)
    except Exception:   # noqa: BLE001
        return False


def get(h, path: str, query: dict) -> None:
    if _limited(h):
        h.send(429, b'{"error":"busy"}' if path == API else b"too many requests\n",
               "application/json; charset=utf-8" if path == API else "text/plain")
        return
    if path == API:
        body = json.dumps(api_doc(), separators=(",", ":"), sort_keys=True).encode("utf-8")
        h.send(200, body, "application/json; charset=utf-8", public=True)
        return
    hub = P("hub_mod")
    if hub is not None:   # the header's account chip
        try:
            hub._REQ.who = hub._viewer(h)
        except Exception:   # noqa: BLE001
            pass
    title, body = page_body(query)
    out, headers = _shell(title, body)
    h.send(200, out, headers=headers)
