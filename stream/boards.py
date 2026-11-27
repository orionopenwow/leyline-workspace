"""Signed leaderboards for Boss Rush and Duels on the website (docs/LEADERBOARDS.md), for the portal
(stream/wrath_portal.py), in the hub's look (the duel boards' CSS, stream/duels.py).

Every result on these boards is signed by the realm that produced it (realm/custom/boards-export.py), and this module
checks every signature before showing a row, as the launcher does. "Authentic" means a server produced it: the
official realm, or a listed community realm that signs its board. A run on a player's own PC is never mixed in: it
goes to the separate "Local runs" board, only when the player sends it, and says so.

    GET  /wrath/boards                       the page: Boss Rush | Duels | Local runs, this week / all time, the fight,
                                             the difficulty, the board (duels / 1v1 arena), the realm, a name search;
                                             every row with the realm that signed it
    GET  /wrath/api/boards/index.json        the realms with a board: {"v", "week", "official": [...], "community":
                                             [...], "local": url}; each {"realm", "name", "pubkey", "url", "ts"}
    GET  /wrath/api/boards/<realm id>.json   that realm's signed board, byte for byte (CORS *): a stable URL per realm
    POST /wrath/api/boards/submit            a community realm sends its signed board: the signature, the id from the
                                             key, and the realm listed on the network (community_realms); one every 4
                                             minutes per realm, 2 MB at most
    GET  /wrath/api/boards/local.json        the local-run board
    POST /wrath/api/boards/local             a run from a player's own PC, sent from the launcher (signed in: Bearer),
                                             signed with the launcher's own key (leyline-board-local-v1); kept apart

Where the boards are: the official realm's in <WU_DUELS_DIR>/boards/ (boards-export.py, cron on the host; the portal
never holds a realm key and never touches the database); community and local boards in <WU_PORTAL_DATA>/boards/.
A missing, unreadable or unsigned file is no board, never an error. Standard library only (Ed25519 from
`cryptography` when the image has it, else ed25519_pure.py beside this file).
"""
from __future__ import annotations

import base64
import hashlib
import html
import importlib.util
import json
import os
import re
import tempfile
import threading
import time
import urllib.parse

PAGE = "/wrath/boards"
API = "/wrath/api/boards/"
INDEX = API + "index.json"
SUBMIT = API + "submit"
LOCAL = API + "local"
LOCAL_JSON = API + "local.json"
HERE = os.path.dirname(os.path.abspath(__file__))
OFFICIAL_DIR = os.path.join(os.environ.get("WU_DUELS_DIR", "/app/duels"), "boards")
DATA_DIR = os.path.join(os.environ.get("WU_PORTAL_DATA", "/data"), "boards")

CTX_RESULT = "leyline-board-result-v1"
CTX_BOARD = "leyline-board-v1"
CTX_LOCAL = "leyline-board-local-v1"
REALM_ID_RE = re.compile(r"^[a-z2-7]{16}$")
NAME_RE = re.compile(r"^[A-Za-z]{2,12}$")
KEY_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,31}$")
HEX64 = re.compile(r"^[0-9a-f]{64}$")
BOARD_TYPE = "application/vnd.leyline.board+json; charset=utf-8"
MAX_BOARD = 2_000_000
MAX_LOCAL = 4_000
MAX_RESULTS = 4000
MAX_AGE = 400 * 86400
SUBMIT_EVERY = 240
LOCAL_EVERY = 30
LOCAL_KEEP = 200            # local runs kept per fight, difficulty and engine
SHOW = 50
WEEK0 = 4 * 86400
DIFFS = ("normal", "heroic")
CLASSES = ("warrior", "paladin", "hunter", "rogue", "priest", "deathknight", "shaman", "mage", "warlock", "druid")
CLASS_LABEL = {"warrior": "Warrior", "paladin": "Paladin", "hunter": "Hunter", "rogue": "Rogue", "priest": "Priest",
               "deathknight": "Death Knight", "shaman": "Shaman", "mage": "Mage", "warlock": "Warlock", "druid": "Druid"}
CLASS_TINT = {"warrior": "#c79c6e", "paladin": "#f58cba", "hunter": "#abd473", "rogue": "#fff569", "priest": "#e8e8e8",
              "deathknight": "#ff4d6a", "shaman": "#2f8fff", "mage": "#69ccf0", "warlock": "#a29bff", "druid": "#ff7d0a"}
# Boss Rush's catalog, our own titles (launcher/src-tauri/src/host/bossrush.rs FIGHTS; the test keeps them equal)
FIGHTS = (("frozen-throne", "The Frozen Throne"), ("bone-lord", "The Bone Lord"), ("cult-mistress", "The Cult Mistress"),
          ("twilight-destroyer", "The Twilight Destroyer"), ("obsidian-sanctum", "The Obsidian Sanctum"),
          ("obsidian-three", "The Obsidian Sanctum, three drakes"), ("brood-mother", "The Brood Mother's Lair"),
          ("stone-watcher", "The Stone Watcher"), ("storm-watcher", "The Storm Watcher"), ("flame-watcher", "The Flame Watcher"),
          ("frost-watcher", "The Frost Watcher"), ("spell-weaver", "The Spellweaver's Eye"),
          ("icc-gunship", "The Gunship Battle"), ("icc-deathbringer", "The Deathbringer"), ("icc-bloat", "The Bloated Horror"),
          ("icc-ooze", "The Ooze Horror"), ("icc-professor", "The Mad Professor"), ("icc-council", "The Blood Council"),
          ("icc-blood-queen", "The Blood Queen"), ("icc-dreamwalker", "The Dreamwalker"), ("icc-frost-queen", "The Frost Queen"))
FIGHT_TITLE = dict(FIGHTS)
DUEL_BOARDS = (("duel", "Duels"), ("arena-1v1", "1v1 arena"))

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


# ---- Ed25519 and the record checks (the same rules as boards-export.py check_board and the launcher's boards.rs) --

def _load_pure():
    for p in (os.path.join(HERE, "ed25519_pure.py"),
              os.path.join(HERE, "..", "hosting", "kit", "tools", "wunet", "ed25519_pure.py")):
        if os.path.isfile(p):
            spec = importlib.util.spec_from_file_location("wu_boards_ed25519", p)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            return mod
    raise ImportError("ed25519_pure.py not found")


try:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
    from cryptography.exceptions import InvalidSignature
    _pure = None
except ImportError:  # pragma: no cover - the portal image has only the standard library
    Ed25519PublicKey = None
    _pure = _load_pure()


def _verify(public: bytes, msg: bytes, sig: bytes) -> bool:
    if len(public) != 32 or len(sig) != 64:
        return False
    if Ed25519PublicKey is not None:
        try:
            Ed25519PublicKey.from_public_bytes(public).verify(sig, msg)
            return True
        except (InvalidSignature, ValueError):
            return False
    return _pure.verify(public, msg, sig)


def canonical(doc) -> bytes:
    return json.dumps(doc, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def verify_doc(public: bytes, doc, context: str, sig_hex) -> bool:
    if not isinstance(sig_hex, str) or len(sig_hex) != 128:
        return False
    try:
        sig = bytes.fromhex(sig_hex)
    except ValueError:
        return False
    return _verify(public, context.encode() + b"\n" + canonical(doc), sig)


def realm_id(public: bytes) -> str:
    return base64.b32encode(hashlib.sha256(public).digest()).decode().lower()[:16]


def week_start(t: int) -> int:
    return WEEK0 + (int(t) - WEEK0) // (7 * 86400) * 7 * 86400


def _banned(text: str) -> bool:
    hub = P("hub_mod")
    fn = getattr(hub, "banned", None) if hub is not None else None
    if fn is not None:
        try:
            return bool(fn(text))
        except Exception:   # noqa: BLE001
            pass
    return bool(re.search(r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard)|(?i:(?<![a-z])wow(?![a-z]))", text or ""))


def _int(v, lo=0, hi=2**31 - 1) -> int | None:
    return v if isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi else None


def _name(v) -> str | None:
    return v if isinstance(v, str) and NAME_RE.fullmatch(v) and not _banned(v) else None


def clean_result(r: dict) -> dict | None:
    """Only the fields a page shows, each checked; anything else in a signed record is ignored (never shown)."""
    if not isinstance(r, dict) or r.get("v") != 1:
        return None
    name, cls = _name(r.get("name")), r.get("class") if r.get("class") in CLASSES else "unknown"
    at = _int(r.get("at"))
    if not name or at is None:
        return None
    if r.get("mode") == "bossrush":
        board, diff, ms = r.get("board"), r.get("diff"), _int(r.get("ms"), 1, 6 * 3600 * 1000)
        if not (isinstance(board, str) and KEY_RE.fullmatch(board)) or diff not in DIFFS or ms is None:
            return None
        party = []
        for p in r.get("party") if isinstance(r.get("party"), list) else []:
            if isinstance(p, dict):
                party.append({"name": _name(p.get("name")), "class": p.get("class") if p.get("class") in CLASSES else "unknown",
                              "bot": p.get("bot") is True})
        out = {"mode": "bossrush", "board": board, "diff": diff, "kind": "group" if r.get("kind") == "group" else "bots",
               "name": name, "class": cls, "ms": ms, "size": _int(r.get("size"), 0, 40) or len(party), "bots": _int(r.get("bots"), 0, 40) or 0,
               "party": party[:40], "at": at, "run": _int(r.get("run")) or 0}
        if _int(r.get("deaths"), 0, 10000) is not None:
            out["deaths"] = r["deaths"]
        return out
    if r.get("mode") == "duels":
        board, period = r.get("board"), r.get("period")
        if board not in dict(DUEL_BOARDS) or not (period == "all" or (isinstance(period, str) and re.fullmatch(r"week:\d{4}-\d{2}-\d{2}", period))):
            return None
        return {"mode": "duels", "board": board, "period": period, "name": name, "class": cls,
                "rating": _int(r.get("rating"), 0, 100000) or 0, "wins": _int(r.get("wins")) or 0,
                "losses": _int(r.get("losses")) or 0, "points": _int(r.get("points")) or 0, "at": at}
    return None


def check_board(board, now: int | None = None) -> tuple[bool, str]:
    if not isinstance(board, dict) or not isinstance(board.get("doc"), dict):
        return False, "shape"
    doc = board["doc"]
    pk = doc.get("pubkey")
    if not isinstance(pk, str) or not HEX64.fullmatch(pk):
        return False, "pubkey"
    pub = bytes.fromhex(pk)
    if realm_id(pub) != doc.get("realm"):
        return False, "realm id"
    if not verify_doc(pub, doc, CTX_BOARD, board.get("sig")):
        return False, "signature"
    ts = doc.get("ts")
    if now is not None and not (_int(ts) is not None and ts <= now + 300 and now - ts <= MAX_AGE):
        return False, "expired"
    res = doc.get("results")
    if not isinstance(res, list) or len(res) > MAX_RESULTS:
        return False, "results"
    for x in res:
        if not isinstance(x, dict) or not isinstance(x.get("r"), dict) or x["r"].get("realm") != doc["realm"]:
            return False, "result realm"
        if not verify_doc(pub, x["r"], CTX_RESULT, x.get("sig")):
            return False, "result signature"
    return True, "ok"


def realm_name(v) -> str:
    s = re.sub(r"[\x00-\x1f<>]", "", v if isinstance(v, str) else "").strip()[:40]
    return s if s and not _banned(s) else "A realm"


# ---- the boards on disk -------------------------------------------------------------------------------------------

class Boards:
    """The checked boards, kept while their files are unchanged (a check is one signature per result)."""

    def __init__(self):
        self.lock = threading.Lock()
        self.seen: dict[str, tuple] = {}      # path -> (mtime, size, board or None, rows)

    def _read(self, path: str, now: int):
        try:
            st = os.stat(path)
        except OSError:
            return None
        key = (st.st_mtime_ns, st.st_size)
        with self.lock:
            hit = self.seen.get(path)
        if hit and hit[:2] == key:
            return hit[2]
        board = None
        if st.st_size <= MAX_BOARD:
            try:
                with open(path, "rb") as f:
                    raw = json.loads(f.read().decode("utf-8"))
                if check_board(raw, now)[0]:
                    board = raw
            except (OSError, ValueError, UnicodeDecodeError):
                board = None
        with self.lock:
            self.seen[path] = (*key, board)
        return board

    def all(self, now: int | None = None, paths: bool = False) -> list:
        """[(kind, board)]: kind "official" (our own export's folder) or "community" (sent and checked here); with
        paths, [(kind, board, path)]."""
        now = int(time.time()) if now is None else now
        out = []
        for kind, folder in (("official", OFFICIAL_DIR), ("community", os.path.join(DATA_DIR, "realms"))):
            try:
                names = sorted(os.listdir(folder))
            except OSError:
                continue
            for f in names:
                if f.endswith(".json") and REALM_ID_RE.fullmatch(f[:-5]):
                    b = self._read(os.path.join(folder, f), now)
                    if b is not None and b["doc"]["realm"] == f[:-5]:
                        out.append((kind, b, os.path.join(folder, f)) if paths else (kind, b))
        return out

    def one(self, rid: str):
        """(kind, board, path) of that realm's board, or None."""
        return next((x for x in self.all(paths=True) if x[1]["doc"]["realm"] == rid), None)


store = Boards()


def rows(boards: list[tuple[str, dict]]) -> list[dict]:
    """Every result, checked and cleaned, with the realm that signed it."""
    out = []
    for kind, b in boards:
        doc = b["doc"]
        for x in doc["results"]:
            r = clean_result(x["r"])
            if r is not None:
                r["realm"] = {"id": doc["realm"], "name": realm_name(doc.get("name")), "kind": kind}
                out.append(r)
    return out


def rank(all_rows: list[dict], tab: str, period: str, now: int, board: str = "", diff: str = "normal",
         realm: str = "", q: str = "") -> list[dict]:
    """One board, ranked: Boss Rush by time (each character's best), Duels by rating (all time) or by points / wins
    (this week). The launcher's boards.js ranks the same way."""
    wk = week_start(now)
    wkey = "week:" + time.strftime("%Y-%m-%d", time.gmtime(wk))
    q = (q or "").strip().lower()
    pick = []
    for r in all_rows:
        if realm and r["realm"]["id"] != realm:
            continue
        if q and q not in r["name"].lower():
            continue
        if tab == "bossrush":
            if r["mode"] != "bossrush" or r["board"] != board or r["diff"] != diff:
                continue
            if period == "week" and r["at"] < wk:
                continue
        else:
            if r["mode"] != "duels" or r["board"] != board or r["period"] != ("all" if period == "all" else wkey):
                continue
        pick.append(r)
    if tab == "bossrush":
        best: dict = {}
        for r in pick:
            k = (r["realm"]["id"], r["name"].lower())
            if k not in best or (r["ms"], r["at"]) < (best[k]["ms"], best[k]["at"]):
                best[k] = r
        out = sorted(best.values(), key=lambda r: (r["ms"], r["at"], r["name"].lower()))
    elif period == "all":
        out = sorted(pick, key=lambda r: (-r["rating"], -r["wins"], r["name"].lower()))
    elif board == "duel":
        out = sorted(pick, key=lambda r: (-r["points"], -r["wins"], r["name"].lower()))
    else:
        out = sorted(pick, key=lambda r: (-r["wins"], r["losses"], r["name"].lower()))
    return [{**r, "rank": i} for i, r in enumerate(out, 1)]


def clock(ms: int) -> str:
    """m:ss.t (BossfightsLogic.h FormatMs)."""
    t = ms // 100
    return f"{t // 600}:{(t // 10) % 60:02d}.{t % 10}"


# ---- local runs -------------------------------------------------------------------------------------------------

def check_local(body) -> tuple[dict | None, str]:
    """A local run from the launcher: signed by the launcher's own key, its peer id from that key."""
    if not isinstance(body, dict):
        return None, "That isn't a run."
    r, pk = body.get("r"), body.get("pubkey")
    if not isinstance(r, dict) or not isinstance(pk, str) or not HEX64.fullmatch(pk):
        return None, "That isn't a run."
    pub = bytes.fromhex(pk)
    if r.get("peer") != realm_id(pub) or not verify_doc(pub, r, CTX_LOCAL, body.get("sig")):
        return None, "The run's signature doesn't check."
    if r.get("v") != 1 or r.get("mode") != "bossrush" or r.get("board") not in FIGHT_TITLE or r.get("diff") not in DIFFS \
            or r.get("engine") not in ("exact", "light"):
        return None, "That fight isn't on the boards."
    name = _name(r.get("name"))
    if not name:
        return None, "Pick a name of 2 to 12 letters for the board."
    ms, at = _int(r.get("ms"), 10_000, 6 * 3600 * 1000), _int(r.get("at"))
    if ms is None or at is None or abs(at - time.time()) > 2 * 86400:
        return None, "That run's time doesn't look right."
    return {"peer": r["peer"], "board": r["board"], "diff": r["diff"], "engine": r["engine"], "name": name,
            "class": r.get("class") if r.get("class") in CLASSES else "unknown", "ms": ms, "at": at}, ""


class Local:
    """The local-run board: each player's best per fight, difficulty and engine. Kept apart from the signed boards."""

    def __init__(self, path: str):
        self.path = path
        self.lock = threading.Lock()
        self.last: dict[str, float] = {}

    def load(self) -> dict:
        try:
            with open(self.path, "r", encoding="utf-8") as f:
                d = json.load(f)
            return d if isinstance(d, dict) and isinstance(d.get("runs"), list) else {"runs": []}
        except (OSError, ValueError):
            return {"runs": []}

    def add(self, who: str, run: dict, now: float | None = None) -> tuple[bool, str]:
        now = time.time() if now is None else now
        who_h = hashlib.sha256(("leyline-board-local\n" + who).encode()).hexdigest()[:24]   # never the account itself
        with self.lock:
            if now - self.last.get(who_h, 0) < LOCAL_EVERY:
                return False, "One run every half a minute."
            self.last[who_h] = now
            d = self.load()
            k = (run["board"], run["diff"], run["engine"])
            mine = next((x for x in d["runs"] if x.get("who") == who_h and (x["board"], x["diff"], x["engine"]) == k), None)
            if mine is not None and mine["ms"] <= run["ms"]:
                return True, "Your best on that board is faster; it stays."
            if mine is not None:
                d["runs"].remove(mine)
            d["runs"].append({**run, "who": who_h})
            same = sorted((x for x in d["runs"] if (x["board"], x["diff"], x["engine"]) == k), key=lambda x: (x["ms"], x["at"]))
            for x in same[LOCAL_KEEP:]:
                d["runs"].remove(x)
            os.makedirs(os.path.dirname(self.path), exist_ok=True)
            fd, tmp = tempfile.mkstemp(dir=os.path.dirname(self.path), prefix=".tmp-")
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                json.dump(d, f, separators=(",", ":"))
            os.replace(tmp, self.path)
        return True, "On the local-run board."

    def public(self) -> dict:
        runs = [{k: x[k] for k in ("board", "diff", "engine", "name", "class", "ms", "at")} for x in self.load()["runs"]]
        return {"v": 1, "local": True, "note": "Runs on players' own PCs, sent by them. Not checked by a server.", "runs": runs}


local = Local(os.path.join(DATA_DIR, "local.json"))
_submit_last: dict[str, float] = {}


def save_community(board: dict) -> None:
    folder = os.path.join(DATA_DIR, "realms")
    os.makedirs(folder, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=folder, prefix=".tmp-")
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        json.dump(board, f, ensure_ascii=True, separators=(",", ":"))
    os.chmod(tmp, 0o644)
    os.replace(tmp, os.path.join(folder, board["doc"]["realm"] + ".json"))


def listed(rid: str) -> bool:
    cr = P("community_realms")
    try:
        realms = cr()[0] if cr else []
    except Exception:   # noqa: BLE001
        return False
    return any(isinstance(r, dict) and r.get("id") == rid for r in realms)


def index(now: int | None = None) -> dict:
    now = int(time.time()) if now is None else now
    out = {"v": 1, "week": time.strftime("%Y-%m-%d", time.gmtime(week_start(now))), "official": [], "community": [],
           "local": LOCAL_JSON}
    for kind, b in store.all(now):
        d = b["doc"]
        out[kind].append({"realm": d["realm"], "name": realm_name(d.get("name")), "pubkey": d["pubkey"], "ts": d["ts"],
                          "url": API + d["realm"] + ".json",
                          "featured": (d.get("featured") or {}).get("bossrush") if isinstance(d.get("featured"), dict) else None})
    return out


# ---- the page -----------------------------------------------------------------------------------------------------

CSS = """
.bd-mark{display:inline-flex;align-items:center;gap:5px;font-size:12.5px;color:var(--lk-text-2);white-space:nowrap}
.bd-mark .bd-ok{display:inline-grid;place-items:center;width:15px;height:15px;border-radius:50%;background:#2e8b57;color:#fff;font-size:10px;font-weight:800}
.bd-mark.local .bd-ok{background:var(--lk-muted)}
.bd-mark small{color:var(--lk-muted)}
.bd-feat{margin:0 0 14px;padding:10px 14px;border:1px solid var(--lk-border);border-radius:var(--lk-r-sm);background:var(--lk-surface-2);font-size:14px}
.bd-filters{display:flex;flex-wrap:wrap;gap:8px 12px;align-items:center;margin:0 0 14px}
.bd-filters label{font-size:13px;color:var(--lk-text-2);display:inline-flex;gap:6px;align-items:center}
.bd-filters select,.bd-filters input{font:inherit;padding:5px 8px;border-radius:var(--lk-r-sm);border:1px solid var(--lk-border);background:var(--lk-surface);color:var(--lk-text)}
.bd-party{color:var(--lk-muted);font-size:12.5px}
.bd-note{color:var(--lk-muted);font-size:13.5px;margin:12px 0 0}
"""


def _cls(c: str) -> str:
    return (f'<span class="db-cl"><span class="db-dot" style="background:{CLASS_TINT.get(c, "#888")}"></span>'
            f'{e(CLASS_LABEL.get(c, "Unknown"))}</span>')


def _mark(realm: dict) -> str:
    what = "Official realm" if realm["kind"] == "official" else "Community realm"
    return (f'<span class="bd-mark" title="Signed by {e(realm["name"])} ({e(realm["id"])}), checked here">'
            f'<span class="bd-ok" aria-hidden="true">&#10003;</span>{e(realm["name"])} <small>{what}</small></span>')


def _qs(base: dict, **over) -> str:
    q = {**base, **over}
    return PAGE + "?" + urllib.parse.urlencode({k: v for k, v in q.items() if v})


def _one(query: dict, k: str, allowed, default: str) -> str:
    v = (query.get(k) or [""])[0][:40]
    return v if v in allowed else default


def page_body(query: dict, now: int | None = None) -> tuple[str, str]:
    now = int(time.time()) if now is None else now
    boards = store.all(now)
    all_rows = rows(boards)
    tab = _one(query, "tab", ("bossrush", "duels", "local"), "bossrush")
    period = _one(query, "p", ("week", "all"), "week")
    realms = {b["doc"]["realm"]: realm_name(b["doc"].get("name")) for _, b in boards}
    realm = _one(query, "realm", realms, "")
    q = re.sub(r"[^A-Za-z]", "", (query.get("q") or [""])[0])[:12]
    featured = next((b["doc"].get("featured", {}).get("bossrush") for k, b in boards if k == "official"
                     and isinstance(b["doc"].get("featured"), dict)), None)
    fights_here = sorted({r["board"] for r in all_rows if r["mode"] == "bossrush"} | ({featured} if featured else set()),
                         key=lambda f: [k for k, _ in FIGHTS].index(f) if f in FIGHT_TITLE else 99)
    default_fight = featured if featured in FIGHT_TITLE else (fights_here[0] if fights_here else FIGHTS[0][0])
    fight = _one(query, "f", [k for k, _ in FIGHTS], default_fight)
    diff = _one(query, "d", DIFFS, "normal")
    dboard = _one(query, "b", dict(DUEL_BOARDS), "duel")
    base = {"tab": tab, "p": period, "f": fight if tab != "duels" else "", "d": diff if tab != "duels" else "",
            "b": dboard if tab == "duels" else "", "realm": realm, "q": q}
    tabs = ('<nav class="hb-seg" aria-label="Boards">' + "".join(
        f'<a href="{e(_qs({}, tab=k))}"' + (' class="on" aria-current="page"' if k == tab else "") + f">{e(t)}</a>"
        for k, t in (("bossrush", "Boss Rush"), ("duels", "Duels"), ("local", "Local runs"))) + "</nav>")
    parts = []
    if featured in FIGHT_TITLE:
        parts.append(f'<p class="bd-feat">This week\'s featured fight: <b>{e(FIGHT_TITLE[featured])}</b>. '
                     f'<a href="{e(_qs({}, tab="bossrush", f=featured))}">See its board</a></p>')
    # the filters (a plain GET form: works without scripts)
    sel = lambda name, opts, cur: (f'<select name="{name}">' + "".join(
        f'<option value="{e(v)}"' + (" selected" if v == cur else "") + f">{e(t)}</option>" for v, t in opts) + "</select>")
    f = [f'<input type="hidden" name="tab" value="{e(tab)}">']
    if tab != "local":
        f.append(f'<label>When {sel("p", (("week", "This week"), ("all", "All time")), period)}</label>')
    if tab in ("bossrush", "local"):
        f.append(f'<label>Fight {sel("f", FIGHTS, fight)}</label>')
        f.append(f'<label>Difficulty {sel("d", (("normal", "Normal"), ("heroic", "Heroic")), diff)}</label>')
    if tab == "duels":
        f.append(f'<label>Board {sel("b", DUEL_BOARDS, dboard)}</label>')
    if tab != "local" and len(realms) > 1:
        f.append(f'<label>Realm {sel("realm", (("", "Every realm"),) + tuple(sorted(realms.items(), key=lambda x: x[1])), realm)}</label>')
    f.append(f'<label>Name <input name="q" value="{e(q)}" maxlength="12" placeholder="Find a name" pattern="[A-Za-z]*"></label>')
    f.append('<button class="hb-btn" type="submit">Show</button>')
    parts.append('<form class="bd-filters" method="get" action="' + PAGE + '">' + "".join(f) + "</form>")

    if tab == "local":
        runs = [x for x in local.public()["runs"] if x["board"] == fight and x["diff"] == diff and (not q or q.lower() in x["name"].lower())]
        runs.sort(key=lambda x: (x["ms"], x["at"]))
        body_rows = "".join(
            f'<tr><td class="rk">{i}</td><td class="nm">{e(x["name"])}</td><td>{_cls(x["class"])}</td>'
            f'<td class="n big">{e(clock(x["ms"]))}</td><td>{"Exact" if x["engine"] == "exact" else "Light version"}</td>'
            f'<td><span class="bd-mark local"><span class="bd-ok" aria-hidden="true">?</span>Local run <small>not checked</small></span></td>'
            f'<td>{e(time.strftime("%Y-%m-%d", time.gmtime(x["at"])))}</td></tr>'
            for i, x in enumerate(runs[:SHOW], 1))
        table = (('<div class="db-wrap"><table class="db-t"><thead><tr><th>#</th><th>Name</th><th>Class</th><th class="n">Time</th>'
                  '<th>Engine</th><th>Source</th><th>Date</th></tr></thead><tbody>' + body_rows + "</tbody></table></div>")
                 if runs else '<p class="hb-lede">No local runs on this board yet.</p>')
        parts.append(table)
        parts.append('<p class="bd-note">Local runs were played on the player\'s own PC (Boss Rush, single player) and sent '
                     'by the player from the launcher. No server saw them, so they are kept apart from the signed boards.</p>')
    else:
        ranked = rank(all_rows, tab, period, now, fight if tab == "bossrush" else dboard, diff, realm, q)
        if tab == "bossrush":
            head = '<th>#</th><th>Leader</th><th>Class</th><th class="n">Time</th><th class="n">Deaths</th><th>Party</th><th>Signed by</th><th>Date</th>'
            def tr(r):
                bots = r["bots"]
                party = f'{r["size"]} · {bots} companions' if bots else f'{r["size"]} players'
                deaths = str(r["deaths"]) if "deaths" in r else "–"
                return (f'<tr class="p{r["rank"]}"><td class="rk">{r["rank"]}</td><td class="nm">{e(r["name"])}</td><td>{_cls(r["class"])}</td>'
                        f'<td class="n big">{e(clock(r["ms"]))}</td><td class="n">{e(deaths)}</td><td class="bd-party">{e(party)}</td>'
                        f'<td>{_mark(r["realm"])}</td><td>{e(time.strftime("%Y-%m-%d", time.gmtime(r["at"])))}</td></tr>')
        else:
            if period == "all":
                head = '<th>#</th><th>Name</th><th>Class</th><th class="n">Rating</th><th class="n">Wins</th><th class="n">Losses</th><th>Signed by</th>'
                tr = lambda r: (f'<tr class="p{r["rank"]}"><td class="rk">{r["rank"]}</td><td class="nm">{e(r["name"])}</td><td>{_cls(r["class"])}</td>'
                                f'<td class="n big">{r["rating"]}</td><td class="n">{r["wins"]}</td><td class="n">{r["losses"]}</td><td>{_mark(r["realm"])}</td></tr>')
            else:
                big = "Points" if dboard == "duel" else "Wins"
                head = f'<th>#</th><th>Name</th><th>Class</th><th class="n">{big}</th><th class="n">Wins</th><th class="n">Losses</th><th>Signed by</th>'
                tr = lambda r: (f'<tr class="p{r["rank"]}"><td class="rk">{r["rank"]}</td><td class="nm">{e(r["name"])}</td><td>{_cls(r["class"])}</td>'
                                f'<td class="n big">{r["points"] if dboard == "duel" else r["wins"]}</td><td class="n">{r["wins"]}</td>'
                                f'<td class="n">{r["losses"]}</td><td>{_mark(r["realm"])}</td></tr>')
        if ranked:
            parts.append('<div class="db-wrap"><table class="db-t"><thead><tr>' + head + "</tr></thead><tbody>"
                         + "".join(tr(r) for r in ranked[:SHOW]) + "</tbody></table></div>")
        else:
            parts.append('<p class="hb-lede">' + ("No one matches that name on this board." if q else "No results on this board yet.") + "</p>")
        parts.append('<p class="bd-note">Every row is signed by the realm that recorded it, and checked here before it shows; '
                     'the launcher checks again. Weeks start Monday 00:00 UTC. Bragging rights only. '
                     f'Raw boards: <a href="{INDEX}">{INDEX}</a></p>')
    title = {"bossrush": "Boss Rush", "duels": "Duels", "local": "Local runs"}[tab]
    body = ('<section class="hb-pg db" aria-labelledby="hb-pg-h"><header class="hb-pg-h"><div class="hb-pg-t">'
            f'<h1 id="hb-pg-h">Leaderboards</h1>{tabs}</div></header><div class="hb-pg-b">' + "".join(parts) + "</div></section>")
    return f"Leaderboards · {title} · Leyline", body


def _shell(title: str, body: str) -> tuple[bytes, dict]:
    duels = P("duels")
    css = str(getattr(duels, "CSS", "") or "") + CSS
    hub = P("hub_mod")
    if hub is not None and getattr(hub, "hub", None) is not None:
        try:
            out = hub.hub._shell(title, body, PAGE, active="leaderboards", css=css)
            return out, ({} if hub.front_on() else {"X-Robots-Tag": "noindex, nofollow"})
        except Exception:   # noqa: BLE001
            pass
    shell = P("ly_shell")
    if shell is not None:
        try:
            return shell(title, body, PAGE, "", "play", css), {}
        except Exception:   # noqa: BLE001
            pass
    return (f"<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>"
            f"<title>{e(title)}</title><style>{css}</style><main>{body}</main></html>").encode("utf-8"), {}


# ---- routing ------------------------------------------------------------------------------------------------------

def match(path: str) -> bool:
    return path == PAGE or path.startswith(API)


def _limited(h, cost: int = 600) -> bool:
    limit, bucket = P("browse_limit"), P("rl_bucket")
    try:
        ip = h.ip()
        return limit is not None and not limit.hit(bucket(ip) if bucket else ip, cost)
    except Exception:   # noqa: BLE001
        return False


def _json(h, status: int, obj, public: bool = False) -> None:
    body = json.dumps(obj, separators=(",", ":"), sort_keys=True).encode("utf-8")
    h.send(status, body, "application/json; charset=utf-8", public=public)


def get(h, path: str, query: dict) -> None:
    if _limited(h):
        if path == PAGE:
            h.send(429, b"too many requests\n", "text/plain")
        else:
            _json(h, 429, {"error": "busy"})
        return
    if path == INDEX:
        return _json(h, 200, index(), public=True)
    if path == LOCAL_JSON:
        return _json(h, 200, local.public(), public=True)
    if path.startswith(API) and path.endswith(".json"):
        rid = path[len(API):-5]
        hit = store.one(rid) if REALM_ID_RE.fullmatch(rid) else None
        if hit is None:
            return _json(h, 404, {"error": "No board for that realm."})
        try:
            with open(hit[2], "rb") as f:
                raw = f.read(MAX_BOARD + 1)
        except OSError:
            return _json(h, 404, {"error": "No board for that realm."})
        # byte for byte, and under its own type: the site's text filters (the brand switch, the old names) never touch
        # application/json's siblings, so nothing here can change a signed byte
        return h.send(200, raw, BOARD_TYPE, public=True)
    if path != PAGE:
        return _json(h, 404, {"error": "not found"})
    hub = P("hub_mod")
    if hub is not None:
        try:
            hub._REQ.who = hub._viewer(h)
        except Exception:   # noqa: BLE001
            pass
    title, body = page_body(query)
    out, headers = _shell(title, body)
    h.send(200, out, headers=headers)


def _read_body(h, cap: int):
    try:
        n = int(h.headers.get("Content-Length", "0"))
    except ValueError:
        return None
    if n <= 0 or n > cap:
        return None
    try:
        doc = json.loads(h.rfile.read(n).decode("utf-8"))
    except (ValueError, UnicodeDecodeError):
        return None
    return doc if isinstance(doc, dict) else None


def post(h, path: str) -> None:
    if _limited(h, 60):
        return _json(h, 429, {"error": "busy"})
    if path == SUBMIT:
        board = _read_body(h, MAX_BOARD)
        if board is None:
            return _json(h, 400, {"error": "Send the signed board as JSON (2 MB at most)."})
        ok, why = check_board(board, int(time.time()))
        if not ok:
            return _json(h, 400, {"error": "The board doesn't check: " + why})
        rid = board["doc"]["realm"]
        if not listed(rid):
            return _json(h, 403, {"error": "Only realms listed on the network can send a board."})
        now = time.time()
        if now - _submit_last.get(rid, 0) < SUBMIT_EVERY:
            return _json(h, 429, {"error": "One board every four minutes."})
        _submit_last[rid] = now
        save_community(board)
        return _json(h, 200, {"ok": True, "url": API + rid + ".json", "results": len(board["doc"]["results"])})
    if path == LOCAL:
        social = P("social")
        st = getattr(social, "store", None) if social is not None else None
        authz = h.headers.get("Authorization", "")
        acct = st.token_acct(authz[7:].strip()) if st is not None and authz.startswith("Bearer ") else None
        if not acct:
            return _json(h, 401, {"error": "Log in from the launcher first."})
        body = _read_body(h, MAX_LOCAL)
        run, why = check_local(body)
        if run is None:
            return _json(h, 400, {"error": why})
        ok, msg = local.add(acct, run)
        return _json(h, 200 if ok else 429, {"ok": ok, "message": msg} if ok else {"error": msg})
    return _json(h, 404, {"error": "not found"})
