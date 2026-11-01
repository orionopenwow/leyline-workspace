"""Short public share links for the Wrath portal (stream/wrath_portal.py), with link-preview cards.

    GET /r/<realm slug or id>   a realm (the official one, or a community realm from the Beacons listing)
    GET /h/<hangout>            a Dalaran hangout
    GET /q/<dungeon or queue>   a Dungeon Finder queue (a dungeon id such as 258, or a queue slug)
    GET /b/<raid or wing>       a boss fight: a raid or one of its wings (stream/modes_data.py ids)
    GET /m/<mod id>             a mod (its real name and description from the Workshop catalog, stream/workshop.py)
    GET /w/<workshop id>        something players made (a course, a mode or a build: stream/creations.py), with Play
    GET /u/<character>          a player profile (with the web armory, stream/armory.py, the card shows the
                                real character: "Name, level 80 Blood Elf Paladin", and links its sheet)

The same links also answer under /wrath (/wrath/r/<id> ...), which the site's proxy already sends here.

Each one is a small public page: Open Graph and X card tags (wrath_portal.card_meta, the 1200 x 630
wu-card.webp image), a line about what was shared, and a refresh plus a button on to the play page with
a deep link:

    /wrath?go=<kind>:<id>      kind is one of r h q b m u w; id passed the same allow-list as the link

Examples: /q/258 -> /wrath?go=q:258, /u/Arthas -> /wrath?go=u:Arthas, /h/krasus-landing ->
/wrath?go=h:krasus-landing. The go value only ever holds [A-Za-z0-9._-] after the colon, so it needs no
escaping in a URL, but it is still percent-quoted and HTML-escaped on the way out. A page that reads
?go= must check it again with the same rules (stream/www-files/wrath-page.js goLink()).

A malformed link (bad characters, too long, unknown kind) or an id nobody knows still gets the generic
card and a plain link to /wrath: never an error page. Nothing here raises to the caller.

Standard library only. The portal is deployed as a single file next to this one; when this file is
missing the portal simply doesn't answer these links.
"""
from __future__ import annotations

import html
import re
import urllib.parse

# kind -> (allow-list for the id, what the card calls it)
KINDS = {
    "r": (re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,30}[a-z0-9])?$"), "Realm"),
    "h": (re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,30}[a-z0-9])?$"), "Dalaran hangout"),
    "q": (re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,30}[a-z0-9])?$"), "Dungeon Finder"),
    "b": (re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,30}[a-z0-9])?$"), "Boss fight"),
    "m": (re.compile(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$"), "Mod"),   # mods/wu-mod.py ID_RE, 3-32 long
    "u": (re.compile(r"^[A-Za-z]{2,12}$"), "Player"),               # a 3.3.5a character name
    "w": (re.compile(r"^[1-9][0-9]{0,8}$"), "Workshop"),            # a Workshop item id (ov_workshop.lua)
}
ID_LEN = {"m": (3, 32)}
MAX_PATH = 80
PREFIXES = ("/wrath/", "/")
_PATH_RE = re.compile(r"^/(?:wrath/)?([a-z])(?:/(.*))?$")
_DESC = {
    "r": "Join this realm on the Leyline network, right in your browser.",
    "h": "Meet up in Dalaran on the Leyline network, right in your browser.",
    "q": "Queue up together on Wrath, the Leyline network's browser realm.",
    "b": "A boss fight on Wrath, the Leyline network's browser realm. Gather a raid and go.",
    "m": "A community mod for Wrath on the Leyline network.",
    "u": "A player on Wrath, the Leyline network's browser realm.",
    "w": "Made by a player on Wrath. Press Play and it starts in your browser.",
}


# Known names for ids, filled by the portal from stream/modes_data.py: {"q:deadmines": "The Deadmines", ...}.
# An id missing here still gets a card, from the id itself.
NAMES: dict = {}


def match(path: str) -> bool:
    """True for any path these links own (/r, /r/..., /wrath/q/...), valid or not, so the portal sends
    them all to share_page() and a bad one gets the generic card instead of a 404."""
    m = _PATH_RE.fullmatch(path or "")
    return bool(m and m.group(1) in KINDS)


def parse(path: str) -> tuple[str, str] | None:
    """(kind, id) for a well-formed share link, else None. The id is matched as sent: no unquoting,
    no case folding, no trimming beyond the portal's trailing-slash strip."""
    if not isinstance(path, str) or len(path) > MAX_PATH:
        return None
    m = _PATH_RE.fullmatch(path)
    if not m or m.group(1) not in KINDS or m.group(2) is None:
        return None
    kind, ident = m.group(1), m.group(2)
    rx = KINDS[kind][0]
    lo, hi = ID_LEN.get(kind, (1, 32))
    if not (lo <= len(ident) <= hi) or not rx.fullmatch(ident):
        return None
    return kind, ident


def go_value(kind: str, ident: str) -> str:
    return f"{kind}:{ident}"


def deep_link(kind: str, ident: str) -> str:
    """/wrath?go=<kind>:<id>, for an already-validated pair."""
    return "/wrath?go=" + urllib.parse.quote(go_value(kind, ident), safe=":")


def check_go(value) -> tuple[str, str] | None:
    """The reverse of go_value, with the same rules: (kind, id) or None. For pages that read ?go=."""
    if not isinstance(value, str) or ":" not in value or len(value) > 40:
        return None
    kind, ident = value.split(":", 1)
    return parse(f"/{kind}/{ident}")


def _pretty(slug: str) -> str:
    return " ".join(w.capitalize() for w in slug.split("-") if w)


def describe(kind: str, ident: str, realm_name: str, realms=None, who=None, mods=None, made=None) -> tuple[str, str, str]:
    """(card title, card description, the line on the page) for a valid link. realms is a callable
    giving the community realm list ([{"id", "name", "tagline"}...]); who(name) gives a character in the
    armory ({"name", "line"}) or None; mods(id) gives one Workshop entry ({"name", "description"}) or
    None (stream/workshop.py Workshop.mod); made(id) gives one Workshop item ({"title", "kind", "author"},
    stream/creations.py card_item) or None; any failure falls back to generic."""
    label = KINDS[kind][1]
    desc = _DESC[kind]
    if kind == "r":
        if ident in (realm_name.lower(), "official", "wrath", "wrathunleashed"):   # the old name's links keep working
            return f"{realm_name} · Leyline", "The official Wrath realm on the Leyline network. Play in your browser.", \
                f"{realm_name}, the official realm"
        try:
            for r in (realms() if realms else ([], False))[0]:
                if r.get("id") == ident:
                    name = str(r.get("name") or "A community realm")[:32]
                    tag = str(r.get("tagline") or "")[:80]
                    return f"{name} · Leyline realm", (tag + " " if tag else "") + desc, f"{name}, a community realm"
        except Exception:   # a broken listing never breaks a link
            pass
        return f"Realm {ident} · Leyline", desc, f"realm {ident}"
    known = NAMES.get(f"{kind}:{ident}")
    if known and kind in ("q", "b", "h"):
        name = str(known)[:64]
        if kind == "q" and ident.isdigit():   # a Dungeon Finder id keeps its number in the title
            return f"{label}: {ident} · {realm_name}", desc, name
        return f"{name} · {realm_name}", desc, name
    if kind == "b":
        return f"{label}: {_pretty(ident)} · {realm_name}", desc, f"the {_pretty(ident)} boss fight"
    if kind == "q":
        what = f"Dungeon Finder queue {ident}" if ident.isdigit() else f"the {_pretty(ident)} queue"
        return f"{label}: {_pretty(ident) if not ident.isdigit() else ident} · {realm_name}", desc, what
    if kind == "u":
        got = _who(who, ident)
        if got:
            name, line = got
            return (f"{name} · {realm_name}", f"{name}, {line}. {desc}", f"{name}, {line}")
        return f"{ident} · {realm_name}", desc, f"{ident}'s profile"
    if kind == "m":
        try:
            m = mods(ident) if mods else None
            if m:
                name = str(m.get("name") or _pretty(ident))[:48]
                about = str(m.get("description") or "").strip()[:200]
                return f"{label}: {name} · Leyline", (about + " " if about else "") + desc, f"the {name} mod"
        except Exception:   # a broken catalog never breaks a link
            pass
        return f"{label}: {_pretty(ident)} · Leyline", desc, f"the {_pretty(ident)} mod"
    if kind == "w":
        try:
            it = made(ident) if made else None
            if it:
                title = str(it.get("title") or "")[:48]
                what = str(it.get("kind") or "item").lower()[:24]
                author = str(it.get("author") or "")[:12]
                if title:
                    return (f"{title} · Workshop · {realm_name}", f"A {what} by {author}. " + desc,
                            f"{title}, a {what} by {author}")
        except Exception:   # a broken listing never breaks a link
            pass
        return f"Workshop item {ident} · {realm_name}", desc, f"Workshop item {ident}"
    return f"{_pretty(ident)} · {label}", desc, f"the {_pretty(ident)} hangout in Dalaran"


def _who(who, ident: str) -> tuple[str, str] | None:
    """(canonical name, "level 80 Blood Elf Paladin") from the armory, or None (not there, opted out, a bot,
    no armory, or anything odd: the generic card then)."""
    if who is None:
        return None
    try:
        got = who(ident)
    except Exception:
        return None
    if not isinstance(got, dict):
        return None
    name, line = got.get("name"), got.get("line")
    if not (isinstance(name, str) and KINDS["u"][0].fullmatch(name) and name.lower() == ident.lower()):
        return None
    if not (isinstance(line, str) and re.fullmatch(r"[A-Za-z0-9 ]{1,60}", line)):
        return None
    return name, line


# ------------------------------------------------------------------------------------------- realm links
# One short link per realm (owner 31 Oct, "share links"): https://<site>/wrath/r/<short>, short = the first 8 letters
# of the relay's 16-letter realm id (stream/network.py REALM_ID_RE). The site's proxy sends /wrath* here today; /r/<short>
# answers too once the proxy forwards /r/* (one Caddy line). The page is the realm's card (name, mode badge, players,
# mods, rules) with two buttons: Play in your browser (/wrath?go=r:<id>) and Open in Leyline (leyline://realm/<id>,
# with Get the launcher under it). No refresh: the visitor picks.
REALM_ID16 = re.compile(r"^[a-z2-7]{16}$")
SHORT_LEN = 8
SHARE_PREFIX = "/wrath/r/"
DEEP_REALM = "leyline://realm/{id}"
GET_LAUNCHER = "https://github.com/orionopenwow/Leyline/releases/latest"
OFFICIAL_IDS = ("official", "wrath", "wrathunleashed")


def short_of(rid: str) -> str:
    """The short slug of a realm id (its first 8 letters); anything else as it is."""
    return rid[:SHORT_LEN] if isinstance(rid, str) and REALM_ID16.fullmatch(rid) else str(rid or "")


def share_path(rid: str) -> str:
    return SHARE_PREFIX + short_of(rid)


def find_realm(realms_list, ident: str):
    """The listed realm a link names: its full id, or the one realm whose id starts with the short slug (6 letters at
    least). Two realms sharing a prefix (never seen: 40 bits) name nobody, so the generic card shows."""
    if not isinstance(ident, str) or not ident:
        return None
    rows = [r for r in (realms_list or []) if isinstance(r, dict) and isinstance(r.get("id"), str)]
    for r in rows:
        if r["id"] == ident:
            return r
    if len(ident) < 6 or not re.fullmatch(r"[a-z2-7]+", ident):
        return None
    hits = [r for r in rows if r["id"].startswith(ident)]
    return hits[0] if len(hits) == 1 else None


def _realm_list(realms):
    try:
        got = realms() if realms else ([], False)
        return got[0] if isinstance(got, tuple) else (got or [])
    except Exception:   # a broken listing never breaks a link
        return []


def mode_label(r: dict, labels=None) -> str:
    """The realm's mode badge: its ruleset in words (the portal's RULESET_LABELS), else Custom rules."""
    rs = r.get("ruleset") if isinstance(r, dict) else None
    labels = labels or {}
    return str(labels.get(rs) or labels.get("custom") or "Custom rules")[:40]


def _uptime(r: dict) -> int:
    for k in ("up7", "up24"):
        v = r.get(k)
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            return int(v)
    return -1


def _players(r: dict) -> int:
    v = r.get("players")
    return v if isinstance(v, int) and not isinstance(v, bool) and v >= 0 else 0


def plays_in_browser(r: dict) -> bool:
    """The browser game (a 3.3.5a client without Warden) can join it; unknown (an older record): yes."""
    return r.get("build", 12340) == 12340 and r.get("warden") is not True


def _mod_names(r: dict, mods=None) -> list[str]:
    out = []
    for m in (r.get("mods") or [])[:12]:
        mid = m.get("id") if isinstance(m, dict) else None
        if not isinstance(mid, str) or ":" not in mid:
            continue
        kind, ident = mid.split(":", 1)
        name = ""
        if kind == "mod" and mods:
            try:
                got = mods(ident)
                name = str((got or {}).get("name") or "")[:40]
            except Exception:
                name = ""
        out.append(name or _pretty(ident.replace(".", "-").replace("_", "-")))
    return out


REALM_CSS = """
.rc{border:1px solid var(--wu-rim,#4a3b1c);border-radius:6px;padding:16px 18px;margin:14px 0;background:var(--wu-panel,rgba(5,4,12,.55))}
.rc-tags{display:flex;flex-wrap:wrap;gap:6px;align-items:center;margin:0 0 10px;font-size:.86rem;color:var(--wu-dim,#b4ab96)}
.rc-tag{display:inline-block;border:1px solid var(--wu-rim,#4a3b1c);border-radius:999px;padding:1px 9px}
.rc-mode{border-color:var(--wu-gold,#d8b25a);color:var(--wu-text,#ece6d6);font-weight:600}
.rc-on{color:var(--wu-ok,#6fd88a)}.rc p{margin:8px 0}.rc ol{margin:6px 0 0;padding-left:20px}
.rc-acts{display:flex;flex-wrap:wrap;gap:10px;justify-content:center;margin:16px 0 6px}
.rc-copy{font-family:var(--wu-mono,monospace);font-size:.86rem;word-break:break-all}
@media (max-width:480px){.rc{padding:14px}.rc-acts .btn{display:flex;width:100%;box-sizing:border-box;justify-content:center}}
"""


def realm_body(r: dict | None, ident: str, host: str = "", labels=None, mods=None, official_name: str = "",
               get_launcher: str = "") -> tuple[str, str, str, str]:
    """(card title, card description, page body, canonical path) for a realm link. r: the listed realm, or None for the
    official realm (official_name set) or an unknown one (the generic realm card with both buttons)."""
    e = html.escape
    get_launcher = get_launcher or GET_LAUNCHER
    if r is None and official_name:
        name, rid, browser = official_name, "official", True
        title = f"{name} · Leyline"
        desc = "The official realm on the Leyline network. Play in your browser, or open it in Leyline."
        tags = '<span class="rc-tag rc-mode">Official</span>'
        about = '<p>Everyone meets in Dalaran. Start at level 1 or boost to 80.</p>'
        go = "/wrath"
        canon = SHARE_PREFIX + ident
    elif r is None:
        name, rid, browser = f"Realm {ident}", ident, True
        title, desc = f"Realm {ident} · Leyline", _DESC["r"]
        tags, about = "", "<p>This realm isn't on the list right now: it may be offline. Try again in a few minutes.</p>"
        go, canon = deep_link("r", ident), SHARE_PREFIX + ident
    else:
        rid = r["id"]
        name = str(r.get("name") or "A community realm")[:32]
        tag = str(r.get("tagline") or "")[:80]
        browser = plays_in_browser(r)
        live, n = bool(r.get("live")), _players(r)
        mode = mode_label(r, labels)
        state = ((f'<span class="rc-on">● {n} playing</span>' if n else '<span class="rc-on">● Online</span>') if live
                 else '<span>Offline</span>')
        title = f"{name} · Leyline realm"
        who = f"{n} playing now" if live and n else "Online now" if live else "Offline right now"
        desc = f"{mode} · {who}. " + (tag + " " if tag else "") + "Play in your browser or open it in Leyline."
        tags = (f'<span class="rc-tag rc-mode">{e(mode)}</span>'
                + (f'<span class="rc-tag">{e(str(r.get("type"))[:8])}</span>' if r.get("type") else "") + state)
        names = _mod_names(r, mods)
        rules = [str(x)[:120] for x in (r.get("rules") or [])[:10] if isinstance(x, str) and x.strip()]
        about = ((f"<p>{e(tag)}</p>" if tag else "")
                 + (f'<p><b>Mods:</b> {e(", ".join(names))}</p>' if names else "<p><b>Mods:</b> none</p>")
                 + ('<details open><summary>House rules</summary><ol>' + "".join(f"<li>{e(x)}</li>" for x in rules)
                    + "</ol></details>" if rules else ""))
        go, canon = deep_link("r", rid), share_path(rid)
    play = (f'<a class="btn primary" href="{e(go)}">Play in your browser</a>' if browser else "")
    app = f'<a class="btn" href="{e(DEEP_REALM.format(id=rid))}">Open in Leyline</a>'
    note = ("" if browser else '<p class="fine center">This realm needs your own game on your PC: open it in Leyline.</p>')
    link = f"https://{host}{canon}" if host else canon
    body = (f'<style>{REALM_CSS}</style><div class="crest wu-brand"><h1>{e(name)}</h1></div>'
            f'<p class="sub">Someone shared a realm with you.</p><hr class="rule wu-divider">'
            f'<div class="rc">{"<div class=rc-tags>" + tags + "</div>" if tags else ""}{about}</div>'
            f'<div class="rc-acts">{play}{app}</div>{note}'
            f'<p class="fine center">No launcher yet? <a href="{e(get_launcher)}" rel="noopener noreferrer">Get the launcher</a>. '
            'Free, for Windows, macOS and Linux.</p>'
            f'<p class="fine center">Share it: <span class="rc-copy">{e(link)}</span></p>')
    return title, desc, body, canon


def realm_page(path: str, card_meta, render, realm_name: str, realms=None, labels=None, mods=None, host: str = "",
               get_launcher: str = "") -> bytes | None:
    """The realm link page for /r/<id or short> (or /wrath/r/...), or None when path is not a well-formed realm link."""
    got = parse(path)
    if not got or got[0] != "r":
        return None
    ident = got[1]
    off = ident in OFFICIAL_IDS or ident == realm_name.lower()
    r = None if off else find_realm(_realm_list(realms), ident)
    try:
        title, desc, body, canon = realm_body(r, ident, host, labels, mods, realm_name if off else "", get_launcher)
    except Exception:   # never an error page
        title, desc, body, canon = realm_body(None, ident, host)
    meta = _swap_description(card_meta(canon, title), desc)
    page = render(body, meta=meta).decode("utf-8")
    page = re.sub(r"<title>[^<]*</title>", lambda _m: f"<title>{html.escape(title)}</title>", page, count=1)
    return page.encode("utf-8")


# ------------------------------------------------------------------------------------------- live now
# "Live now" on the site's front page and the launcher's Home (owner 31 Oct): the busiest player realms right now
# (players online, then uptime), and "New this week": realms first seen in the last 7 days with a player online in
# the last day. The portal notes when it last saw players on each realm (note_players, from every good listing
# fetch), kept in $WU_PORTAL_DATA/realm-last-player.json so a restart doesn't forget.
LAST_PLAYER: dict = {}
LAST_FILE = ""
_last_saved = [0.0]
WEEK, DAY = 7 * 86400, 86400


def _iso(v):
    if not isinstance(v, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", v):
        return None
    import calendar
    import time as _t
    try:
        return float(calendar.timegm(_t.strptime(v, "%Y-%m-%dT%H:%M:%SZ")))
    except (ValueError, OverflowError):
        return None


def load_last(path: str) -> None:
    """Read the saved last-player times (missing or broken: start empty)."""
    import json
    global LAST_FILE
    LAST_FILE = path
    try:
        with open(path, encoding="utf-8") as f:
            v = json.load(f)
        if isinstance(v, dict):
            LAST_PLAYER.update({k: float(t) for k, t in v.items() if isinstance(k, str) and REALM_ID16.fullmatch(k)
                                and isinstance(t, (int, float)) and not isinstance(t, bool)})
    except (OSError, ValueError):
        pass


def note_players(realms_list, now: float | None = None) -> None:
    """Remember now for every live realm with players; forget realms not seen with players for 8 days. Saves at most
    once a minute. Never raises."""
    import json
    import os
    import time as _t
    now = _t.time() if now is None else now
    try:
        for r in realms_list or []:
            if isinstance(r, dict) and r.get("live") and _players(r) > 0 and isinstance(r.get("id"), str):
                LAST_PLAYER[r["id"]] = now
        for k in [k for k, t in LAST_PLAYER.items() if now - t > WEEK + DAY]:
            del LAST_PLAYER[k]
        if LAST_FILE and now - _last_saved[0] >= 60:
            _last_saved[0] = now
            tmp = LAST_FILE + ".tmp"
            with open(tmp, "w", encoding="utf-8") as f:
                json.dump(LAST_PLAYER, f)
            os.replace(tmp, LAST_FILE)
    except Exception:
        pass


def _community(r) -> bool:
    return (isinstance(r, dict) and isinstance(r.get("id"), str) and not r.get("official") and not r.get("home")
            and not r.get("sample") and not r.get("hangout") and not r.get("vr"))


def live_now(realms_list, n: int = 6) -> list[dict]:
    """The busiest player realms right now: live community realms by players online, then uptime, then name."""
    rows = [r for r in realms_list or [] if _community(r) and r.get("live")]
    rows.sort(key=lambda r: (-_players(r), -_uptime(r), str(r.get("name") or "").lower()))
    return rows[:n]


def new_this_week(realms_list, now: float | None = None, n: int = 6, last=None) -> list[dict]:
    """Realms first seen in the last 7 days with a player online in the last day (now, or noted by note_players),
    newest first."""
    import time as _t
    now = _t.time() if now is None else now
    last = LAST_PLAYER if last is None else last
    out = []
    for r in realms_list or []:
        if not _community(r):
            continue
        t = _iso(r.get("first_seen"))
        if t is None or not (0 <= now - t <= WEEK):
            continue
        seen = now if (r.get("live") and _players(r) > 0) else last.get(r["id"])
        if seen is not None and now - seen <= DAY:
            out.append((t, r))
    out.sort(key=lambda x: -x[0])
    return [r for _, r in out[:n]]


def live_item(r: dict, labels=None) -> dict:
    """One strip entry in plain fields (for HTML and JSON alike): id, short link, name, mode, players, live, play."""
    rid = r["id"]
    browser = plays_in_browser(r)
    return {"id": rid, "short": short_of(rid), "link": share_path(rid), "name": str(r.get("name") or "A realm")[:32],
            "mode": mode_label(r, labels), "players": _players(r), "live": bool(r.get("live")),
            "play": deep_link("r", rid) if browser else share_path(rid), "browser": browser}


def share_page(path: str, card_meta, render, realm_name: str = "WrathUnleashed", realms=None, who=None,
               mods=None, made=None, labels=None, host: str = "", get_launcher: str = "") -> bytes:
    """The share page for path. card_meta(path, title) and render(body, meta=...) are the portal's own;
    who(name) gives an armory character for /u/<name>, mods(id) a Workshop entry for /m/<id>.
    Always returns a page: an unknown or malformed link gets the generic card and a link to /wrath.
    A realm link (/r/<id or short>) is the realm's own card: realm_page()."""
    e = html.escape
    try:
        rp = realm_page(path, card_meta, render, realm_name, realms, labels, mods, host, get_launcher)
    except Exception:
        rp = None
    if rp is not None:
        return rp
    got = None
    try:
        got = parse(path)
    except Exception:
        got = None
    if got:
        kind, ident = got
        try:
            title, desc, line = describe(kind, ident, realm_name, realms, who, mods, made)
        except Exception:
            title, desc, line = f"{realm_name} · Leyline", _DESC[kind], "a link"
        target = deep_link(kind, ident)
        canon = f"/{kind}/{ident}" if not path.startswith("/wrath/") else f"/wrath/{kind}/{ident}"
    else:
        title, desc, line, target, canon = "", "", "", "/wrath", "/wrath"
    meta = card_meta(canon, title)
    if desc:
        # card_meta carries the shared CARD_DESCRIPTION; a share link says what was shared instead.
        meta = _swap_description(meta, desc)
    meta += f'<meta http-equiv="refresh" content="1;url={e(target)}">'
    shared = (f'<p class="sub">Someone shared {e(line)} with you.</p>' if line
              else '<p class="sub">Play Wrath in your browser, on the Leyline network.</p>')
    sheet = ""
    if got and got[0] == "u":
        found = _who(who, got[1])
        if found:
            sheet = f'<p class="center"><a href="/wrath/armory/{e(found[0])}">See {e(found[0])} in the armory</a></p>'
    if got and got[0] == "m":
        try:
            if mods and mods(got[1]):
                sheet = f'<p class="center"><a href="/wrath/mods/{e(got[1])}">See it in the Workshop</a></p>'
        except Exception:
            sheet = ""
    if got and got[0] == "w":
        try:
            if made and made(got[1]):
                sheet = f'<p class="center"><a href="/wrath/workshop/{e(got[1])}">See it in the Workshop</a></p>'
        except Exception:
            sheet = ""

    body = (f'<div class="crest wu-brand"><h1>{e(title or realm_name)}</h1></div>{shared}'
            '<hr class="rule wu-divider">'
            f'<p class="center"><a class="btn primary" href="{e(target)}">Open in Wrath</a></p>{sheet}'
            '<p class="fine center">Taking you there now. Nothing to install: it runs in your browser '
            '(or with your own game files).</p>')
    page = render(body, meta=meta).decode("utf-8")
    if title:   # the tab title says what was shared too
        page = re.sub(r"<title>[^<]*</title>", lambda _m: f"<title>{e(title)}</title>", page, count=1)
    return page.encode("utf-8")


def _swap_description(meta: str, desc: str) -> str:
    """Put desc in the description tags of card_meta's output (all of them, already escaped)."""
    d = html.escape(desc)
    return re.sub(r'((?:name="description"|property="og:description"|name="twitter:description") content=")[^"]*(")',
                  lambda m: m.group(1) + d + m.group(2), meta)
