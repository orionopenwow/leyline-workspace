"""Boss fights and the 1v1 arena queue on the website, for the portal (stream/wrath_portal.py), in the Leyline hub's
look (stream/hub.py). Owner 5 Nov 2026: instant boss fights, the final fight of Icecrown Citadel featured (players see it
as The Frozen Throne, never the boss's own name), queue with others or go with companions, a board of clear times; and
a duel arena queue players join from the browser.

    GET /wrath/bossfights            the featured fight: how it works, Fight now with companions / Queue with players,
                                     normal or heroic (?d=heroic), the top 10 (with players, with companions), what's next
    GET /wrath/arena                 Duels and arena: Queue 1v1 arena, the duel corner, the boards
    GET /wrath/api/bossfights        the boards as JSON for the launcher (CORS *, cached a minute)
    /wrath/leaderboards?board=bosses and ?board=arena
                                     the same boards as tabs of the duel leaderboards (stream/duels.py asks this module)

The go kinds (the play page's ?go=, carried through sign-in like every other go link, checked again at every hop):

    bf:<boss>:<difficulty>:<mode>    boss frozen-throne; difficulty normal | heroic; mode bots | group | group-only
    arena:1v1                        the unrated 1v1 arena queue

A signed-in player arriving with one of them: the portal sends one console command over SOAP,
    ovgo set <ACCOUNT> <go>
the same intent ov_go.lua keeps for the other go links (30 minutes, used once); the realm's boss-fight script then runs
".bossfight join <boss> <difficulty> <mode>" (or ".q1v1 unrated") for the character at login (docs/BOSS-FIGHTS.md).

The data: one file the realm's export writes beside the duel boards (realm/custom/bossfights-export.py, cron on the
host; the portal never touches the database):

    <WU_DUELS_DIR>/bossfights.json   {"updated": iso, "bosses": {"frozen-throne": {"normal": {"group": [rows],
                                      "bots": [rows], "weekly": [rows]}, "heroic": {...}}}}
                                      row {rank, ms, time, date, leader, roster: [{name, class, bot}], size, bots}
    <WU_DUELS_DIR>/arena.json        optional, the 1v1 ladder: {"updated": iso, "ladder": [{name, class, rating, wins,
                                      losses}]}; until an export writes it the tab says the ladder is coming

Only the fields read in clean() ever reach a page or the JSON: character names of plain 2-12 letters (never a brand
word, stream/hub.py banned()), a class, numbers and a date. Companions (bots) are counted, not named. The time shown is
worked out here from ms (the file's own "time" is not trusted). A missing, unreadable or older-shaped file is empty
boards, never an error. Standard library only; the portal runs without this file.
"""
from __future__ import annotations

import calendar
import html
import importlib.util
import json
import os
import re
import threading
import time
import urllib.parse

PAGE = "/wrath/bossfights"
ARENA_PAGE = "/wrath/arena"
API = "/wrath/api/bossfights"
BOARDS_PAGE = "/wrath/leaderboards"
DATA_DIR = os.environ.get("WU_DUELS_DIR", "/app/duels")
FILE = "bossfights.json"
ARENA_FILE = "arena.json"
DUEL_CORNER = "/wrath/h/duels"
NAME_RE = re.compile(r"^[A-Za-z]{2,12}$")
ISO_RE = re.compile(r"^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.\d{1,6})?)?)?(Z|[+-]\d{2}:?\d{2})?$")
DAY_RE = re.compile(r"^(\d{4}-\d{2}-\d{2})")
ACCOUNT_RE = re.compile(r"^[A-Za-z0-9]{1,16}$")
MAX_FILE = 4_000_000
MAX_ROWS = 500
MAX_MS = 6 * 3600 * 1000      # a clear longer than six hours is not a clear
SHOW = 10                     # rows per board on the page
API_ROWS = 50
STAT_EVERY = 5.0
STALE_S = 3 * 3600
BOARD_KEYS = (("group", "With players"), ("bots", "With companions"), ("weekly", "This week"))

CLASSES = {1: "Warrior", 2: "Paladin", 3: "Hunter", 4: "Rogue", 5: "Priest", 6: "Death Knight", 7: "Shaman",
           8: "Mage", 9: "Warlock", 11: "Druid"}
CLASS_BY_NAME = {v.lower().replace(" ", ""): k for k, v in CLASSES.items()}
CLASS_COLOR = {1: "#c79c6e", 2: "#f58cba", 3: "#abd473", 4: "#fff569", 5: "#ffffff", 6: "#ff4d6a", 7: "#2f8fff",
               8: "#69ccf0", 9: "#a29bff", 11: "#ff7d0a"}

e = html.escape
_get = None


def _load_data():
    spec = importlib.util.spec_from_file_location(
        "wu_bf_modes_data", os.path.join(os.path.dirname(os.path.abspath(__file__)), "modes_data.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


D = _load_data()
BOSSES = {b["id"]: b for b in D.BOSS_FIGHTS}
LIVE = [b["id"] for b in D.BOSS_FIGHTS if b.get("live")]
FEATURED = LIVE[0] if LIVE else "frozen-throne"
DIFFS = tuple(D.BF_DIFFICULTIES)
MODES = tuple(D.BF_MODES)
ARENA_GO = D.ARENA_QUEUE["go"]
BF_RE = re.compile(r"^bf:([a-z0-9](?:[a-z0-9-]{0,22}[a-z0-9])?):(" + "|".join(DIFFS) + r"):(" + "|".join(MODES) + r")$")
ARENA_RE = re.compile(r"^arena:1v1$")


def bind(getter) -> None:
    global _get
    _get = getter


def P(name):
    try:
        return _get(name) if _get else None
    except KeyError:
        return None


# ----------------------------------------------------------------------------------------------
# the go kinds
# ----------------------------------------------------------------------------------------------

def check_go(value) -> str | None:
    """A ?go= value this module owns, as sent (bf:<live boss>:<difficulty>:<mode>, arena:1v1), else None."""
    if not isinstance(value, str) or len(value) > 40:
        return None
    if ARENA_RE.fullmatch(value):
        return value
    m = BF_RE.fullmatch(value)
    return value if m and m.group(1) in LIVE else None


def go_value(boss: str = FEATURED, difficulty: str = "normal", mode: str = "bots") -> str:
    return D.boss_go(boss, difficulty, mode)


# The mode catalog's entries for these (stream/playmodes.json: the launcher's Modes page and the Games section of
# /wrath/modes): kind "realm" with start {"go": "mode:<id>"} as the catalog's schema wants, so the launcher lists them
# unchanged. The portal turns ?go=mode:<id> into the go below (wrath_portal.remember_go); the site's own Play button
# links the go directly (play_href). The desktop path (the LeylinePlay addon) sends "wuplay go <id>": the realm's
# ov_playmode.lua maps the same ids (docs/BOSS-FIGHTS.md).
MODE_GO = {"frozen-throne": D.boss_go("frozen-throne", "normal", "bots"), "arena-queue": D.ARENA_QUEUE["go"]}


def mode_go(mid) -> str | None:
    """The go of a catalog mode id that is a boss fight or the arena queue, else None."""
    return MODE_GO.get(mid) if isinstance(mid, str) else None


def play_link(go: str) -> str:
    """/wrath?go=<go>: the play page, which signs in (or asks to) and goes on."""
    return "/wrath?go=" + urllib.parse.quote(go, safe=":")


def go_href(go: str, signed_in: bool) -> str:
    """Where a button goes: signed in, the play page with the go; signed out, Log in first, then the same play link."""
    link = play_link(go)
    if signed_in:
        return link
    sign = P("signin_url")
    try:
        return sign(link) if sign is not None else "/wrath/signin?next=" + urllib.parse.quote(link, safe="")
    except Exception:   # noqa: BLE001
        return "/wrath/signin?next=" + urllib.parse.quote(link, safe="")


def describe(go: str) -> str | None:
    """Plain words for a go value ("Boss fight: The Frozen Throne (heroic, with companions)"), else None."""
    if not check_go(go):
        return None
    if go == ARENA_GO:
        return "1v1 arena queue"
    _, boss, diff, mode = go.split(":")
    how = {"bots": "with companions", "group": "with players", "group-only": "players only"}[mode]
    return f"Boss fight: {BOSSES[boss]['name']} ({diff}, {how})"


def go_command(account: str, go: str) -> str | None:
    if not isinstance(account, str) or not ACCOUNT_RE.fullmatch(account) or not check_go(go):
        return None
    return f"ovgo set {account.upper()} {go}"


_recent: dict = {}
_recent_lock = threading.Lock()
REPEAT_S = 120


def go_intent(soap, account: str, go: str, log=None) -> bool:
    """Record the intent on the realm (ov_go.lua answers "ovgo ok"). Never raises: a failed go never stops anyone
    playing; the same intent for the same account is sent once per two minutes."""
    cmd = go_command(account, go)
    if not cmd:
        return False
    now, key = time.monotonic(), account.upper()
    with _recent_lock:
        last = _recent.get(key)
        if last and last[0] == go and now - last[1] < REPEAT_S:
            return True
    try:
        text = soap(cmd) or ""
    except Exception as ex:  # noqa: BLE001
        if log:
            log.warning("go intent %s %s failed: %s", key, go, getattr(ex, "kind", type(ex).__name__))
        return False
    ok = "ovgo ok" in text
    if ok:
        with _recent_lock:
            _recent[key] = (go, now)
            if len(_recent) > 20000:
                _recent.clear()
    elif log:
        log.warning("go intent %s %s: the realm did not confirm (does ov_go.lua know boss fights yet?)", key, go)
    return ok


# ----------------------------------------------------------------------------------------------
# boss fight heroes (docs/BOSS-FIGHT-HEROES.md)
# ----------------------------------------------------------------------------------------------
#
# A boss fight is played on a hero: a level 80 character the realm makes at once for the fight, on a second game
# account linked to the player's own (hero_account). The player's own characters are never shown, moved or bound by
# a boss fight. With heroes on (WU_BF_HEROES, default 1), /wrath/play with a bf: go:
#   1. makes sure the hero account exists with the player's own game password (ensure_account),
#   2. asks the realm over SOAP:  bossfight hero go <ACCOUNT> <boss> <difficulty> <mode> [class spec]
#      which makes the hero if there is none for that pick, remembers the pick, saves the hero on the fight's
#      platform in a fresh instance and keeps the join for its login. The reply: "bfhero ok <name> <class> <spec>
#      <spec2> <made|ready> <hero account>" or "bfhero no <why>",
#   3. signs the browser game in to the hero account with the hero named (/__env.js "char": no character list).
# A realm without the module (an older image) answers with a console fault: the page says so in plain words and
# nothing starts (owner 5 Nov: Fight now must never just land someone in the normal realm).

HERO_CLASSES = {   # class word -> (label, [(spec key, label, role)]); the realm's table is BossfightsHeroLogic.h Specs()
    "warrior": ("Warrior", [("arms", "Arms", "dps"), ("fury", "Fury", "dps"), ("prot", "Protection", "tank")]),
    "paladin": ("Paladin", [("holy", "Holy", "heal"), ("prot", "Protection", "tank"), ("ret", "Retribution", "dps")]),
    "hunter": ("Hunter", [("bm", "Beast Mastery", "dps"), ("mm", "Marksmanship", "dps"), ("sv", "Survival", "dps")]),
    "rogue": ("Rogue", [("assa", "Assassination", "dps"), ("combat", "Combat", "dps"), ("sub", "Subtlety", "dps")]),
    "priest": ("Priest", [("disc", "Discipline", "heal"), ("holy", "Holy", "heal"), ("shadow", "Shadow", "dps")]),
    "deathknight": ("Death Knight", [("blood", "Blood", "tank"), ("frost", "Frost", "dps"), ("unholy", "Unholy", "dps")]),
    "shaman": ("Shaman", [("ele", "Elemental", "dps"), ("enh", "Enhancement", "dps"), ("resto", "Restoration", "heal")]),
    "mage": ("Mage", [("arcane", "Arcane", "dps"), ("fire", "Fire", "dps"), ("frost", "Frost", "dps")]),
    "warlock": ("Warlock", [("affli", "Affliction", "dps"), ("demo", "Demonology", "dps"), ("destro", "Destruction", "dps")]),
    "druid": ("Druid", [("balance", "Balance", "dps"), ("bear", "Feral (bear)", "tank"), ("resto", "Restoration", "heal"),
                        ("cat", "Feral (cat)", "dps")]),
}
CLASS_WORD = {1: "warrior", 2: "paladin", 3: "hunter", 4: "rogue", 5: "priest", 6: "deathknight", 7: "shaman", 8: "mage",
              9: "warlock", 11: "druid"}
ROLE_WORD = {"tank": "Tank", "heal": "Healer", "dps": "Damage"}
HERO_RE = re.compile(r"^([a-z]{4,11})-([a-z]{2,8})(?:-(alliance|horde))?$")
HERO_LIMIT = 10
HERO_ACCOUNT_MAX = 16
NOT_LIVE = ("Boss fights aren't open on the realm yet: the realm update that adds them hasn't gone live. Nothing was "
            "started, and none of your characters were moved.")


def heroes_on() -> bool:
    return os.environ.get("WU_BF_HEROES", "1").strip() != "0"


def check_hero(value) -> str | None:
    """A ?hero= pick as sent (<class>-<spec>[-alliance|-horde]) when the class and spec are real, else None."""
    if not isinstance(value, str) or len(value) > 32:
        return None
    m = HERO_RE.fullmatch(value)
    if not m or m.group(1) not in HERO_CLASSES:
        return None
    return value if m.group(2) in {k for k, _, _ in HERO_CLASSES[m.group(1)][1]} else None


def hero_param(query: dict) -> str:
    """'&hero=<pick>' for a valid pick in the query (carried through Log in like the go), else ''."""
    v = check_hero((query.get("hero") or [""])[0][:32])
    return "&hero=" + v if v else ""


def hero_account(user) -> str:
    """The player's hero account (BossfightsHeroLogic.h HeroAccountName): <ACCOUNT>BF, or BF + 14 hex digits of the
    name's FNV-1a hash when that would pass 16 characters. "" for a name that isn't a plain account name."""
    if not isinstance(user, str) or not ACCOUNT_RE.fullmatch(user):
        return ""
    u = user.upper()
    if len(u) + 2 <= HERO_ACCOUNT_MAX:
        return u + "BF"
    h = 1469598103934665603
    for c in u.encode("ascii"):
        h ^= c
        h = (h * 1099511628211) & 0xFFFFFFFFFFFFFFFF
    return "BF" + "%014X" % (h & 0xFFFFFFFFFFFFFF)


def hero_command(account: str, go: str, pick: str | None = None) -> str | None:
    """The console line for a Fight now: bossfight hero go <ACCOUNT> <boss> <difficulty> <mode> [class spec [faction]]."""
    if not isinstance(account, str) or not ACCOUNT_RE.fullmatch(account) or not check_go(go) or not go.startswith("bf:"):
        return None
    _, boss, diff, mode = go.split(":")
    cmd = f"bossfight hero go {account.upper()} {boss} {diff} {mode}"
    pick = check_hero(pick) if pick else None
    if pick:
        cmd += " " + " ".join(pick.split("-"))
    return cmd


REPLY_RE = re.compile(r"^bfhero ok ([A-Z][a-z]{1,11}) ([a-z]{4,11}) ([a-z]{2,8}) ([a-z]{2,8}) (made|ready) ([A-Z0-9]{1,16})$")
LINE_RE = re.compile(r"^bfhero hero ([A-Z][a-z]{1,11}) ([a-z]{4,11}) ([a-z]{2,8}) ([a-z]{2,8}) (current|-)$")


def parse_hero_reply(text) -> dict:
    """The realm's answer to "bossfight hero go/make": {"ok": True, name, class, spec, spec2, made, account} or
    {"ok": False, "why": plain words}. A name with a brand word is refused (the module never makes one)."""
    for line in str(text or "").replace("\r", "\n").split("\n"):
        line = line.strip()
        m = REPLY_RE.fullmatch(line)
        if m and m.group(2) in HERO_CLASSES and not _banned(m.group(1)):
            return {"ok": True, "name": m.group(1), "class": m.group(2), "spec": m.group(3), "spec2": m.group(4),
                    "made": m.group(5) == "made", "account": m.group(6)}
        if line.startswith("bfhero no "):
            why = re.sub(r"[^A-Za-z0-9 .,:;'()/-]", "", line[len("bfhero no "):])[:120].strip()
            return {"ok": False, "why": why or "the realm said no"}
    return {"ok": False, "why": "the realm gave no answer"}


def parse_hero_list(text) -> list[dict]:
    out = []
    for line in str(text or "").replace("\r", "\n").split("\n"):
        m = LINE_RE.fullmatch(line.strip())
        if m and m.group(2) in HERO_CLASSES and not _banned(m.group(1)):
            out.append({"name": m.group(1), "class": m.group(2), "spec": m.group(3), "spec2": m.group(4),
                        "current": m.group(5) == "current"})
    return out[:HERO_LIMIT]


def _not_live(ex) -> bool:
    """A console fault that isn't our own answer: the command isn't known (no module, or one without heroes)."""
    return getattr(ex, "kind", "") == "fault" and "bfhero" not in str(getattr(ex, "detail", ""))


def hero_play(soap, account: str, go: str, pick: str | None = None, log=None) -> dict:
    """Fight now for a signed-in player: {"state": "ok", name, ...} (play the hero), "not-live" (the realm has no boss
    fights yet), "no" (the realm said why: "why"), "down" (the console didn't answer) or "bad" (nothing to send)."""
    cmd = hero_command(account, go, pick)
    if not cmd:
        return {"state": "bad"}
    try:
        text = soap(cmd) or ""
    except Exception as ex:  # noqa: BLE001
        if _not_live(ex):
            if log:
                log.warning("hero go %s: the realm doesn't know boss fight heroes yet (%s)", account.upper(),
                            str(getattr(ex, "detail", ""))[:80])
            return {"state": "not-live"}
        if log:
            log.warning("hero go %s failed: %s", account.upper(), getattr(ex, "kind", type(ex).__name__))
        return {"state": "down"}
    r = parse_hero_reply(text)
    if not r["ok"]:
        return {"state": "no", "why": r["why"]}
    if r["account"] != hero_account(account):
        return {"state": "no", "why": "the realm named another hero account"}
    return dict(r, state="ok")


_lists: dict = {}
_lists_lock = threading.Lock()
LIST_TTL = 30.0


def hero_list(soap, account: str) -> list[dict] | None:
    """The player's heroes (".bossfight hero list"), cached half a minute; None when the realm can't say."""
    if not isinstance(account, str) or not ACCOUNT_RE.fullmatch(account):
        return None
    key, now = account.upper(), time.monotonic()
    with _lists_lock:
        hit = _lists.get(key)
        if hit and now - hit[0] < LIST_TTL:
            return hit[1]
    try:
        heroes = parse_hero_list(soap(f"bossfight hero list {key}") or "")
    except Exception:  # noqa: BLE001
        return None
    with _lists_lock:
        if len(_lists) > 5000:
            _lists.clear()
        _lists[key] = (now, heroes)
    return heroes


def forget_list(account: str) -> None:
    with _lists_lock:
        _lists.pop(str(account).upper(), None)


def hero_label(cls: str, spec: str) -> str:
    c = HERO_CLASSES.get(cls)
    if not c:
        return ""
    s = next((lab for k, lab, _ in c[1] if k == spec), spec)
    return f"{s} {c[0]}"


def hero_href(go: str, pick: str | None, signed_in: bool) -> str:
    """Fight now with this pick (the realm makes the hero if there is none for it)."""
    link = play_link(go) + (("&hero=" + pick) if pick and check_hero(pick) else "")
    if signed_in:
        return link
    sign = P("signin_url")
    try:
        return sign(link) if sign is not None else "/wrath/signin?next=" + urllib.parse.quote(link, safe="")
    except Exception:   # noqa: BLE001
        return "/wrath/signin?next=" + urllib.parse.quote(link, safe="")


def heroes_html(go: str, signed_in: bool, heroes: list[dict] | None) -> str:
    """The hero picker: the player's heroes (signed in), then every class and spec. No script: plain links."""
    if not heroes_on():
        return ""
    mine = ""
    if heroes:
        items = "".join(
            f'<li><a class="bf-hero-me{" on" if h["current"] else ""}" href="{e(hero_href(go, h["class"] + "-" + h["spec"], signed_in))}">'
            f'<b>{e(h["name"])}</b><span>{e(hero_label(h["class"], h["spec"]))}</span></a></li>' for h in heroes)
        mine = (f'<p class="hb-fine">Your heroes ({len(heroes)} of {HERO_LIMIT}). Pick one to fight now:</p>'
                f'<ul class="bf-heroes">{items}</ul>')
    rows = []
    for cls, (label, specs) in HERO_CLASSES.items():
        links = "".join(f'<a href="{e(hero_href(go, cls + "-" + key, signed_in))}" title="{e(ROLE_WORD[role])}">'
                        f'{e(slab)}<small>{e(ROLE_WORD[role])}</small></a>' for key, slab, role in specs)
        rows.append(f'<div class="bf-cls"><span class="bf-cls-n">{e(label)}</span><div class="bf-specs">{links}</div></div>')
    return ('<section class="bf-sec" aria-labelledby="bf-hero-h"><h2 id="bf-hero-h">Your hero</h2>'
            '<p class="hb-lede">Boss fights use their own heroes, separate from your characters: level 80, geared for '
            'the fight, every spell on the bars, both talent specs ready and the rest of the class\'s gear in the bags. '
            'Pick a class and spec and the fight starts with it at once.</p>'
            + mine + f'<div class="bf-pick">{"".join(rows)}</div>'
            '<p class="hb-fine">In game: <code>/bf</code> opens the panel, <code>.bf hero</code> lists your heroes, '
            '<code>.bf hero spec &lt;spec&gt;</code> changes talents for free between pulls.</p></section>')


# ----------------------------------------------------------------------------------------------
# data: whitelisted fields only
# ----------------------------------------------------------------------------------------------

def _n(v, lo=0, hi=100000) -> int:
    return v if isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi else 0


def _banned(text: str) -> bool:
    hub = P("hub_mod")
    try:
        return bool(hub.banned(text)) if hub is not None else False
    except Exception:   # noqa: BLE001
        return True


def _name(v) -> str:
    return v if isinstance(v, str) and NAME_RE.fullmatch(v) and not _banned(v) else ""


def _class(v) -> int:
    if isinstance(v, int) and not isinstance(v, bool):
        return v if v in CLASSES else 0
    if isinstance(v, str):
        return CLASS_BY_NAME.get(v.lower().replace(" ", "").replace("_", ""), 0)
    return 0


def iso_epoch(v) -> int:
    """An ISO time ("2026-11-05T19:00:00Z", a date, or an offset) as epoch seconds; 0 for anything else. An int stays."""
    if isinstance(v, int) and not isinstance(v, bool):
        return v if v > 0 else 0
    if not isinstance(v, str) or len(v) > 40:
        return 0
    m = ISO_RE.fullmatch(v.strip())
    if not m:
        return 0
    y, mo, d, hh, mm, ss, tz = m.groups()
    try:
        t = calendar.timegm((int(y), int(mo), int(d), int(hh or 0), int(mm or 0), int(ss or 0)))
    except (ValueError, OverflowError):
        return 0
    if tz and tz != "Z":
        sign = 1 if tz[0] == "+" else -1
        z = tz[1:].replace(":", "")
        t -= sign * (int(z[:2]) * 3600 + int(z[2:]) * 60)
    return t if t > 0 else 0


def clear_time(ms: int) -> str:
    """m:ss.t (h:mm:ss.t past an hour), from milliseconds."""
    tenths = max(0, int(ms)) // 100
    s, t = divmod(tenths, 10)
    m, s = divmod(s, 60)
    if m >= 60:
        h, m = divmod(m, 60)
        return f"{h}:{m:02d}:{s:02d}.{t}"
    return f"{m}:{s:02d}.{t}"


def _row(r) -> dict | None:
    if not isinstance(r, dict):
        return None
    ms = _n(r.get("ms"), 1, MAX_MS)
    if not ms:
        return None
    roster, bots = [], 0
    for p in (r.get("roster") if isinstance(r.get("roster"), list) else [])[:40]:
        if not isinstance(p, dict):
            continue
        if p.get("bot") is True:
            bots += 1   # companions are counted, never named
            continue
        nm = _name(p.get("name"))
        if nm and nm.lower() not in {x["name"].lower() for x in roster}:
            roster.append({"name": nm, "class": _class(p.get("class"))})
    size = _n(r.get("size"), 1, 40) or (len(roster) + bots) or 10
    bots = max(bots, _n(r.get("bots"), 0, 40))
    bots = min(bots, size)
    leader = _name(r.get("leader"))
    if leader and leader.lower() not in {x["name"].lower() for x in roster}:
        leader = ""   # a leader the roster doesn't hold (or a companion) is not shown
    if not leader and roster:
        leader = roster[0]["name"]
    if not leader:
        return None   # a run with no player we can show is not a run on our boards
    day = DAY_RE.match(r.get("date")) if isinstance(r.get("date"), str) and len(r.get("date")) <= 40 else None
    date = day.group(1) if day and iso_epoch(day.group(1)) else ""
    return {"ms": ms, "time": clear_time(ms), "date": date, "leader": leader, "roster": roster, "size": size, "bots": bots}


def _board(rows) -> list[dict]:
    out = [c for c in (_row(r) for r in (rows if isinstance(rows, list) else [])[:MAX_ROWS]) if c]
    out.sort(key=lambda r: (r["ms"], r["date"], r["leader"].lower()))
    for i, r in enumerate(out, 1):
        r["rank"] = i
    return out


def clean(doc) -> dict:
    """The boards, checked: {"updated": epoch, "bosses": {boss: {difficulty: {group, bots, weekly}}}} for the known
    bosses and difficulties only. Anything else in the file is never read."""
    d = doc if isinstance(doc, dict) else {}
    src = d.get("bosses") if isinstance(d.get("bosses"), dict) else {}
    out = {}
    for bid in BOSSES:
        b = src.get(bid) if isinstance(src.get(bid), dict) else {}
        out[bid] = {}
        for diff in DIFFS:
            x = b.get(diff) if isinstance(b.get(diff), dict) else {}
            out[bid][diff] = {k: _board(x.get(k)) for k, _ in BOARD_KEYS}
    return {"updated": iso_epoch(d.get("updated")), "bosses": out}


def clean_arena(doc) -> dict:
    d = doc if isinstance(doc, dict) else {}
    rows, seen = [], set()
    for r in (d.get("ladder") if isinstance(d.get("ladder"), list) else [])[:MAX_ROWS]:
        if not isinstance(r, dict):
            continue
        nm = _name(r.get("name"))
        if not nm or nm.lower() in seen:
            continue
        seen.add(nm.lower())
        rows.append({"name": nm, "class": _class(r.get("class")), "rating": _n(r.get("rating"), 0, 10000),
                     "wins": _n(r.get("wins")), "losses": _n(r.get("losses"))})
    rows.sort(key=lambda r: (-r["rating"], -r["wins"], r["name"].lower()))
    return {"updated": iso_epoch(d.get("updated")), "ladder": rows, "present": "ladder" in d}


class Store:
    """One JSON file, re-read when it changes (checked at most every STAT_EVERY seconds); the last good copy stays."""

    def __init__(self, name: str, cleaner, root: str | None = None):
        self.name, self.cleaner = name, cleaner
        self.root = root or DATA_DIR
        self._lock = threading.Lock()
        self._doc = cleaner({})
        self._mtime = None
        self._checked = 0.0

    def doc(self) -> dict:
        now = time.monotonic()
        with self._lock:
            if now - self._checked < STAT_EVERY and self._mtime is not None:
                return self._doc
            self._checked = now
            path = os.path.join(self.root, self.name)
            try:
                st = os.stat(path)
            except OSError:
                self._doc, self._mtime = self.cleaner({}), None
                return self._doc
            if st.st_mtime == self._mtime:
                return self._doc
            try:
                if st.st_size > MAX_FILE:
                    raise ValueError("too big")
                with open(path, encoding="utf-8") as f:
                    self._doc = self.cleaner(json.load(f))
                self._mtime = st.st_mtime
            except (OSError, ValueError):
                pass
            return self._doc


store = Store(FILE, clean)
arena_store = Store(ARENA_FILE, clean_arena)


def api_doc(doc: dict | None = None, arena: dict | None = None) -> dict:
    d = doc or store.doc()
    a = arena or arena_store.doc()
    bosses = {}
    for bid, diffs in d["bosses"].items():
        b = BOSSES[bid]
        bosses[bid] = {"name": b["name"], "size": b["size"], "live": bool(b.get("live")),
                       "go": {diff: {m: go_value(bid, diff, m) for m in MODES} for diff in DIFFS},
                       "boards": {diff: {k: rows[:API_ROWS] for k, rows in boards.items()} for diff, boards in diffs.items()}}
    return {"updated": d["updated"], "bosses": bosses, "featured": FEATURED,
            "next": [{"id": n["id"], "name": n["name"]} for n in D.BOSS_FIGHTS_NEXT],
            "arena": {"updated": a["updated"], "ladder": a["ladder"][:API_ROWS], "go": ARENA_GO},
            "classes": {str(k): v for k, v in CLASSES.items()}, "page": PAGE, "arena_page": ARENA_PAGE}


# ----------------------------------------------------------------------------------------------
# the pages
# ----------------------------------------------------------------------------------------------

CSS = """
.bf-hero{position:relative;overflow:hidden;border:1px solid var(--lk-border);border-radius:var(--lk-r-sm);margin:0 0 18px;
padding:clamp(20px,5vw,44px) clamp(16px,4vw,36px);background:radial-gradient(70% 90% at 15% 10%,rgba(120,200,255,.28),transparent 70%),
radial-gradient(60% 80% at 90% 80%,rgba(150,110,255,.25),transparent 70%),linear-gradient(180deg,#0d1630,#070b1c)}
.bf-hero img{position:absolute;right:clamp(-40px,2vw,24px);top:50%;width:clamp(120px,24vw,220px);height:auto;transform:translateY(-50%);opacity:.55;pointer-events:none}
.bf-eyebrow{margin:0 0 6px;font-size:12px;font-weight:700;letter-spacing:.16em;text-transform:uppercase;color:#9fdcff}
.bf-hero h1{margin:0 0 8px;font-size:clamp(28px,6vw,46px);line-height:1.05;color:#eaf6ff;text-align:left;position:relative}
.bf-hero .bf-tag{margin:0 0 16px;max-width:560px;color:#cfe2f5;font-size:16px;position:relative}
.bf-acts{display:flex;flex-wrap:wrap;gap:10px;align-items:center;position:relative}
.bf-acts .hb-play,.bf-acts .hb-ghost{min-height:44px;display:inline-flex;align-items:center}
.bf-seg{display:inline-flex;border:1px solid var(--lk-border);border-radius:999px;overflow:hidden;margin:0 0 14px;position:relative}
.bf-seg a{padding:8px 16px;font-weight:600;font-size:14px;color:var(--lk-text-2);text-decoration:none;min-height:20px}
.bf-seg a.on{background:var(--lk-accent-soft);color:var(--lk-text)}
.bf-how{display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));gap:12px;margin:0 0 18px;padding:0;list-style:none}
.bf-how li{border:1px solid var(--lk-border);border-radius:var(--lk-r-sm);background:var(--lk-surface-2);padding:12px 14px;font-size:14px;color:var(--lk-text-2)}
.bf-how b{display:block;color:var(--lk-text);font-size:15px;margin-bottom:4px}
.bf-boards{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:14px;margin:0 0 18px}
.bf-boards h2,.bf-sec h2{margin:0 0 8px;font-size:16px;text-align:left}
.bf-wrap{overflow-x:auto;border:1px solid var(--lk-border);border-radius:var(--lk-r-sm);background:var(--lk-surface)}
.bf-t{width:100%;border-collapse:collapse;font-size:14px;font-variant-numeric:tabular-nums}
.bf-t th,.bf-t td{padding:8px 10px;text-align:left;border-bottom:1px solid var(--lk-border);white-space:nowrap}
.bf-t thead th{background:var(--lk-surface-2);color:var(--lk-muted);font-size:12px;font-weight:700;letter-spacing:.06em;text-transform:uppercase}
.bf-t tbody tr:last-child td{border-bottom:0}
.bf-t td.n,.bf-t th.n{text-align:right}.bf-t td.rk{width:2.6em;color:var(--lk-muted);font-weight:700}
.bf-t tr.p1 td.rk{color:var(--lk-gold)}.bf-t td.tm{font-weight:700;color:var(--lk-text)}
.bf-t td.who{white-space:normal;min-width:10em;color:var(--lk-text-2)}.bf-t td.who b{color:var(--lk-text)}
.bf-cl{display:inline-flex;align-items:center;gap:6px}.bf-dot{width:9px;height:9px;border-radius:50%;flex:none;box-shadow:0 0 0 1px rgba(0,0,0,.35)}
.bf-bot{color:#5ef0a0}
.bf-empty{padding:18px 14px;text-align:center;color:var(--lk-muted);margin:0}
.bf-next{display:flex;flex-wrap:wrap;gap:8px;margin:0;padding:0;list-style:none}
.bf-next li{padding:8px 12px;border:1px dashed var(--lk-border);border-radius:var(--lk-r-sm);color:var(--lk-text-2);font-size:14px}
.bf-next li span{color:var(--lk-muted);font-size:12.5px;margin-left:6px}
.bf-sec{margin:0 0 18px}
.bf-cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:12px;margin:0 0 18px}
.bf-card{display:flex;flex-direction:column;gap:8px;border:1px solid var(--lk-border);border-radius:var(--lk-r-sm);background:var(--lk-surface);padding:14px 16px}
.bf-card h2{margin:0;font-size:17px;text-align:left}.bf-card p{margin:0;color:var(--lk-text-2);font-size:14px}
.bf-card .hb-play,.bf-card .hb-ghost{align-self:flex-start;margin-top:auto;min-height:40px;display:inline-flex;align-items:center}
.bf-card code,.bf-how code{font-size:13px}
@media (max-width:640px){.bf-t th,.bf-t td{padding:7px 7px}.bf-t .hide-s{display:none}.bf-acts{flex-direction:column;align-items:stretch}
.bf-acts .hb-play,.bf-acts .hb-ghost{justify-content:center}.bf-hero img{display:none}}
.bf-pick{display:grid;gap:8px;margin:12px 0}
.bf-cls{display:grid;grid-template-columns:120px 1fr;gap:10px;align-items:center}
.bf-cls-n{font-weight:600;color:#ece6d6}
.bf-specs{display:flex;flex-wrap:wrap;gap:6px}
.bf-specs a,.bf-hero-me{display:inline-flex;flex-direction:column;justify-content:center;min-height:44px;padding:6px 12px;border:1px solid #3a3550;
border-radius:8px;background:rgba(12,10,24,.6);color:#ece6d6;text-decoration:none;line-height:1.2}
.bf-specs a:hover,.bf-hero-me:hover,.bf-hero-me.on{border-color:#d9b45a}
.bf-specs small,.bf-hero-me span{font-size:12px;color:#b4ab96}
.bf-heroes{list-style:none;padding:0;margin:8px 0 12px;display:flex;flex-wrap:wrap;gap:8px}
@media (max-width:640px){.bf-cls{grid-template-columns:1fr}}
"""


def _when(t: int) -> str:
    if not t:
        return ""
    g = time.gmtime(t)
    return (f'Updated <time datetime="{time.strftime("%Y-%m-%dT%H:%M:%SZ", g)}">{time.strftime("%d %b %H:%M", g)} UTC</time>'
            + (" (the boards may be a little behind)" if time.time() - t > STALE_S else ""))


def _cls(c: int) -> str:
    if c not in CLASSES:
        return ""
    return (f'<span class="bf-cl"><span class="bf-dot" style="background:{CLASS_COLOR[c]}" aria-hidden="true"></span>'
            f'{e(CLASSES[c])}</span>')


def _who(r: dict) -> str:
    names = [p["name"] for p in r["roster"]]
    shown = [f"<b>{e(r['leader'])}</b>"] + [e(n) for n in names if n != r["leader"]][:5]
    more = len(names) - 1 - min(5, len(names) - 1) if names else 0
    out = ", ".join(shown) + (f" +{more}" if more > 0 else "")
    if r["bots"]:
        out += f' <span class="bf-bot">+{r["bots"]} companion{"s" if r["bots"] != 1 else ""}</span>'
    return out


def board_table(rows: list[dict], label: str, empty: str, n: int = SHOW) -> str:
    rows = rows[:n]
    if not rows:
        return f'<div class="bf-wrap"><p class="bf-empty">{empty}</p></div>'
    body = "".join(
        f'<tr{_rank_cls(r["rank"])}><td class="rk">{r["rank"]}</td>'
        f'<td class="tm">{e(r["time"])}</td><td class="who">{_who(r)}</td>'
        f'<td class="n hide-s">{r["size"]}</td><td class="hide-s">{e(r["date"])}</td></tr>' for r in rows)
    return (f'<div class="bf-wrap"><table class="bf-t" aria-label="{e(label)}"><thead><tr><th scope="col">#</th>'
            '<th scope="col">Time</th><th scope="col">Raid</th><th scope="col" class="n hide-s">Size</th>'
            f'<th scope="col" class="hide-s">Date</th></tr></thead><tbody>{body}</tbody></table></div>')


def arena_table(a: dict, n: int = 100) -> str:
    rows = a["ladder"][:n]
    if not rows:
        if not a.get("present"):
            return ('<div class="bf-wrap"><p class="bf-empty">The 1v1 arena ladder is coming: ratings from the arena queue '
                    f'show here once the realm starts sending them. <a href="{ARENA_PAGE}">Queue 1v1 arena</a></p></div>')
        return (f'<div class="bf-wrap"><p class="bf-empty">Nobody is on the ladder yet. '
                f'<a href="{ARENA_PAGE}">Queue 1v1 arena</a></p></div>')
    body = "".join(
        f'<tr{_rank_cls(i)}><td class="rk">{i}</td><td class="tm">{e(r["name"])}</td><td>{_cls(r["class"])}</td>'
        f'<td class="n tm">{r["rating"]}</td><td class="n">{r["wins"]}-{r["losses"]}</td></tr>' for i, r in enumerate(rows, 1))
    return ('<div class="bf-wrap"><table class="bf-t" aria-label="1v1 arena ladder"><thead><tr><th scope="col">#</th>'
            '<th scope="col">Name</th><th scope="col">Class</th><th scope="col" class="n">Rating</th>'
            f'<th scope="col" class="n">Won-lost</th></tr></thead><tbody>{body}</tbody></table></div>')


def _diff(query: dict) -> str:
    v = (query.get("d") or ["normal"])[0]
    return v if v in DIFFS else "normal"


def boards_html(d: dict, boss: str, diff: str, keys=("group", "bots")) -> str:
    b = d["bosses"].get(boss, {}).get(diff, {})
    name = BOSSES[boss]["name"]
    parts = []
    for k, label in BOARD_KEYS:
        if k not in keys:
            continue
        empty = ("No clears yet. Be the first raid on the board." if k != "weekly" else "No clears yet this week.")
        parts.append(f'<section aria-label="{e(label)}"><h2>{e(label)}</h2>'
                     + board_table(b.get(k, []), f"{name}, {diff}: {label.lower()}", empty) + "</section>")
    return f'<div class="bf-boards">{"".join(parts)}</div>'


def _signed_in(h) -> bool:
    hub = P("hub_mod")
    try:
        who = getattr(hub._REQ, "who", None) if hub is not None else None
        return bool(who)
    except Exception:   # noqa: BLE001
        return False


def _seg_at(href, on: str) -> str:
    """The Normal / Heroic switch: plain links (no script); href(difficulty) is each one's address."""
    return ('<nav class="bf-seg" aria-label="Difficulty">' + "".join(
        f'<a href="{href(k)}"' + (' class="on" aria-current="true"' if k == on else "") + f">{e(k.capitalize())}</a>"
        for k in DIFFS) + "</nav>")


def _rank_cls(i: int) -> str:
    return f' class="p{i}"' if i <= 3 else ""


def boss_page(query: dict, signed_in: bool, d: dict | None = None, heroes: list[dict] | None = None) -> tuple[str, str]:
    """(title, body) for /wrath/bossfights. heroes: the signed-in player's boss fight heroes, when the realm said."""
    d = d or store.doc()
    boss = FEATURED
    b = BOSSES[boss]
    diff = _diff(query)
    now = go_href(go_value(boss, diff, "bots"), signed_in)
    queue = go_href(go_value(boss, diff, "group"), signed_in)
    login = "" if signed_in else '<p class="hb-fine" style="position:relative">You log in first, then the fight starts.</p>'
    art = P("art_url")
    crystal = ""
    try:
        if art is not None:
            crystal = f'<img src="{e(art("leyline-crystal-256.webp"))}" alt="" width="256" height="256" loading="lazy" decoding="async">'
    except Exception:   # noqa: BLE001
        crystal = ""
    hero = ('<header class="bf-hero">' + crystal + '<p class="bf-eyebrow">Boss fights</p>'
            f'<h1 id="hb-pg-h">{e(b["name"])}</h1>'
            # TODO(owner): the tagline under the title, your words
            '<!-- TODO(owner): the tagline, your words -->'
            f'<p class="bf-tag">The last fight at the top of {e(b["raid"])}. Straight to the platform, no wings to clear first.</p>'
            + _seg_at(lambda k: PAGE + ("" if k == "normal" else f"?d={k}"), diff)
            + '<div class="bf-acts">'
            f'<a class="hb-play" href="{e(now)}">Fight now with companions</a>'
            f'<a class="hb-ghost" href="{e(queue)}">Queue with players</a></div>' + login + '</header>')
    how = ('<ul class="bf-how" aria-label="How it works">'
           + (f'<li><b>{b["size"]} players</b>A raid of {b["size"]} at level {b["level"]}, on a hero made for the fight: '
            'none of your own characters is needed or moved.</li>' if heroes_on() else
            f'<li><b>{b["size"]} players</b>A raid of {b["size"]} at level {b["level"]}. New characters can boost to 80 first.</li>')
           + '<li><b>Go now or queue</b>Fight now and companions fill the raid around you. Queue with players and the raid '
           'forms from the line; companions fill the gaps.</li>'
           '<li><b>Normal or heroic</b>Heroic is harder and has its own board. Pick it above before you go.</li>'
           '<li><b>The clock</b>Time starts on the pull and stops when the fight is won. A wipe sends the raid back to '
           'the start of the platform to try again.</li></ul>')
    nxt = ('<section class="bf-sec" aria-labelledby="bf-next-h"><h2 id="bf-next-h">Coming next</h2><ul class="bf-next">'
           + "".join(f'<li>{e(n["name"])}<span>{e(n["raid"])}</span></li>' if n["raid"] != n["name"] else f'<li>{e(n["name"])}</li>'
                     for n in D.BOSS_FIGHTS_NEXT) + "</ul></section>")
    when = _when(d["updated"])
    boards = (f'<section class="bf-sec" aria-labelledby="bf-top-h"><h2 id="bf-top-h">Fastest clears, {e(diff)}</h2>'
              + boards_html(d, boss, diff)
              + f'<p class="hb-fine">{when + ". " if when else ""}Every board: <a href="{BOARDS_PAGE}?board=bosses'
              + ("&amp;d=heroic" if diff == "heroic" else "") + '">the leaderboards</a>. Companions are counted, never named.</p></section>')
    picker = heroes_html(go_value(boss, diff, "bots"), signed_in, heroes)
    body = ('<section class="hb-pg db bf" aria-labelledby="hb-pg-h"><div class="hb-pg-b">' + hero + picker + how + boards + nxt
            + f'<p class="hb-fine">More ways to fight: <a href="{ARENA_PAGE}">duels and the 1v1 arena</a>, '
            '<a href="/wrath/modes#bosses">every raid by its door</a>.</p></div></section>')
    return f"Boss fights · {b['name']} · Leyline", body


def arena_page(query: dict, signed_in: bool) -> tuple[str, str]:
    """(title, body) for /wrath/arena."""
    q = D.ARENA_QUEUE
    queue = go_href(ARENA_GO, signed_in)
    login = "" if signed_in else " You log in first."
    cards = ('<div class="bf-cards">'
             f'<article class="bf-card"><h2>{e(q["name"])}</h2><p>{e(q["line"])}{login}</p>'
             f'<p>Level {q["min"]}. In game, <code>.hub arena</code> leads to the battlemaster.</p>'
             f'<a class="hb-play" href="{e(queue)}">Queue 1v1 arena</a></article>'
             '<article class="bf-card"><h2>Duels in Dalaran</h2><p>The duel corner at Krasus\' Landing. Target a player and '
             'type <code>/duel</code>. Health and cooldowns come back after every duel.</p>'
             f'<a class="hb-ghost" href="{DUEL_CORNER}">Go duel</a></article>'
             '<article class="bf-card"><h2>The boards</h2><p>Duel points every week, the duel ratings that stay, and the 1v1 '
             'arena ladder.</p>'
             f'<a class="hb-ghost" href="{BOARDS_PAGE}">Duel leaderboards</a>'
             f'<a class="hb-ghost" href="{BOARDS_PAGE}?board=arena">1v1 arena ladder</a></article></div>')
    body = ('<section class="hb-pg db bf" aria-labelledby="hb-pg-h"><header class="hb-pg-h"><div class="hb-pg-t">'
            '<h1 id="hb-pg-h">Duels and arena</h1></div>'
            f'<a class="hb-play hb-pg-x" href="{e(queue)}">Queue 1v1 arena</a></header><div class="hb-pg-b">'
            '<p class="hb-lede">One on one, right in your browser: queue for the arena, or meet someone at the duel corner.</p>'
            + cards
            + f'<p class="hb-fine">Want a raid instead? <a href="{PAGE}">Boss fights</a>.</p></div></section>')
    return "Duels and arena · Leyline", body


# --- the tabs on /wrath/leaderboards (stream/duels.py calls these) -------------------------------------------------

def leaderboard_tab(board: str, query: dict) -> str:
    """The content of the Boss fights (board=bosses) or 1v1 arena (board=arena) tab."""
    if board == "arena":
        a = arena_store.doc()
        when = _when(a["updated"])
        return ('<p class="hb-lede">The 1v1 arena queue at level 80: the rating from rated matches.</p>' + arena_table(a)
                + (f'<p class="hb-fine">{when}.</p>' if when else "")
                + f'<p class="hb-fine"><a href="{ARENA_PAGE}">Queue 1v1 arena</a></p>')
    d = store.doc()
    diff = _diff(query)
    boss = FEATURED
    seg = _seg_at(lambda k: f"{BOARDS_PAGE}?board=bosses" + ("" if k == "normal" else f"&amp;d={k}"), diff)
    when = _when(d["updated"])
    return (f'<p class="hb-lede">{e(BOSSES[boss]["name"])}: the fastest clears, from the pull to the win.</p>' + seg
            + boards_html(d, boss, diff, keys=("group", "bots", "weekly"))
            + (f'<p class="hb-fine">{when}. The boards update every 5 minutes.</p>' if when else
               '<p class="hb-fine">The boards update every 5 minutes, starting with the first clear.</p>')
            + f'<p class="hb-fine"><a href="{PAGE}' + ("?d=heroic" if diff == "heroic" else "") + '">Fight it now</a></p>')


# ----------------------------------------------------------------------------------------------
# the portal's hooks (stream/wrath_portal.py: bind, match, get)
# ----------------------------------------------------------------------------------------------

def match(path: str) -> bool:
    return path in (PAGE, ARENA_PAGE, API)


def _shell(title: str, body: str, path: str, active: str) -> tuple[bytes, dict]:
    hub = P("hub_mod")
    if hub is not None and getattr(hub, "hub", None) is not None:
        try:
            out = hub.hub._shell(title, body, path, active=active, css=CSS)
            return out, ({} if hub.front_on() else {"X-Robots-Tag": "noindex, nofollow"})
        except Exception:   # noqa: BLE001
            pass
    shell = P("ly_shell")
    if shell is not None:
        try:
            return shell(title, body, path, "", "play", CSS), {}
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


def _my_heroes(h) -> list[dict] | None:
    """The signed-in viewer's heroes: their game account from the realm service, then the realm's list (cached)."""
    portal_get, soap, bucket = P("portal_get"), P("ac_soap"), P("rl_bucket")
    if portal_get is None or soap is None:
        return None
    try:
        ip = h.ip()
        status, play = portal_get("/api/play", h.headers.get("Cookie", ""), ip, h.headers.get("User-Agent", ""))
        user = str((play or {}).get("user", "")) if status == 200 else ""
        return hero_list(soap, user) if user else None
    except Exception:   # noqa: BLE001
        return None


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
    if hub is not None:
        try:
            hub._REQ.who = hub._viewer(h)
        except Exception:   # noqa: BLE001
            pass
    signed = _signed_in(h)
    if path == ARENA_PAGE:
        title, body = arena_page(query, signed)
        out, headers = _shell(title, body, path, "arena")
    else:
        title, body = boss_page(query, signed, heroes=_my_heroes(h) if signed and heroes_on() else None)
        out, headers = _shell(title, body, path, "bossfights")
    h.send(200, out, headers=headers)
