#!/usr/bin/env python3
"""Signed leaderboards for Boss Rush and Duels (docs/LEADERBOARDS.md). Host side, standard library only (Ed25519 from
`cryptography` when the host has it, else ed25519_pure.py beside this file).

    python3 -I /srv/wrath/boards-export.py                 # from cron, every 5 minutes (/etc/cron.d/wu-boards)
    python3 -I /srv/wrath/boards-export.py --schema        # the tables ov_boards.lua makes (the server pass, step 1h)
    python3 -I /srv/wrath/boards-export.py --whoami        # this realm's board id and public key (never the secret)
    python3 -I boards-export.py --post https://leylinelauncher.com/wrath/api/boards/submit
                                                           # a community realm: export, then send the signed board

What it makes: ONE document per realm, signed with the realm's own Ed25519 key, holding one signed record per result.
Any launcher or website checks every signature itself, so a board can be mirrored or cached anywhere and still says
which realm produced it. The key file is the same shape as a hosted realm's (wunet/keys.py: 32 bytes, hex, 0600); the
official realm's is made here on the first run. The realm id is base32(sha256(public key))[:16], as on the mesh.

    a result     {"r": RESULT, "sig": hex Ed25519 over "leyline-board-result-v1\\n" + canonical(RESULT)}
    the board    {"doc": DOC, "sig": hex Ed25519 over "leyline-board-v1\\n" + canonical(DOC)}
    DOC          {"v": 1, "realm": id, "pubkey": hex, "name", "ts", "seq", "week": "YYYY-MM-DD" (Monday, UTC),
                  "featured": {"bossrush": fight key}, "results": [a result, ...]}
    RESULT, Boss Rush   {"v": 1, "realm", "mode": "bossrush", "board": fight key, "diff": "normal"|"heroic",
                         "kind": "group"|"bots", "name": the leader, "class", "ms", "size", "bots",
                         "party": [{"name"|null, "class", "bot"}], "at": the clear (unix s), "run": the realm's run id,
                         "deaths": n (only when ov_boards.lua counted them)}
    RESULT, Duels       {"v": 1, "realm", "mode": "duels", "board": "duel"|"arena-1v1", "period": "all"|"week:YYYY-MM-DD",
                         "name", "class", "rating", "wins", "losses", "points", "at": as of (unix s)}

Canonical JSON: sorted keys, no spaces, UTF-8 (the mesh's, docs/LEYLINE-MESH.md section 3).

Which results go in (the viewer ranks them; every board keeps at most MAX_PER_BOARD):
  * Boss Rush, per fight and difficulty: each leader's best clear of all time and best clear this week; only
    result = clear, gm = 0, tier = 0 (mod-leyline-bossfights' own table, leyline_bossfight_runs).
  * Duels: the duel ratings (ov_duels.lua: ov_duel_stats, at least PLACED_AFTER rated duels) and this week's points
    (ov_duel_week); the 1v1 arena's ratings (mod-1v1-arena's arena_team, captains) and this week's 1v1 arena wins
    (ov_boards.lua's ov_arena_log, at most PAIR_PER_DAY wins a day against the same opponent count).

Privacy (the same as the boards before it): character names and class words only, never accounts, guids or ids; a
character with ".armory off" is left out (in a party: name null); names that aren't 2-12 plain letters or carry a
brand word are left out; bots (accounts RNDBOT*) are never ranked.

    WU_DUELS_OUT          the folder the portal reads (default /srv/wrath/portal/duels); the board goes to boards/
    WU_BOARD_KEY          the realm's key file (default /srv/wrath/leyline/board-realm.key, made 0600 the first time)
    WU_BOARD_REALM_NAME   the name on the board (default "Wrath Unleashed")
    WU_DB_CONTAINER       default ov-ac-database

On any failure nothing is written and the exit code is 1 (the site keeps the last good board). Never prints a
password or the key: mysql runs in its container with the container's own MYSQL_ROOT_PASSWORD.
"""
from __future__ import annotations

import base64, hashlib, importlib.util, json, os, re, subprocess, sys, tempfile, time, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("WU_DUELS_OUT", "/srv/wrath/portal/duels")
KEY_FILE = os.environ.get("WU_BOARD_KEY", "/srv/wrath/leyline/board-realm.key")
REALM_NAME = os.environ.get("WU_BOARD_REALM_NAME", "Wrath Unleashed")
DB_CONTAINER = os.environ.get("WU_DB_CONTAINER", "ov-ac-database")
CS = "acore_characters"

CTX_RESULT = "leyline-board-result-v1"
CTX_BOARD = "leyline-board-v1"
VERSION = 1
MAX_PER_BOARD = 100
MAX_RESULTS = 4000
MAX_RUNS = 50000
PLACED_AFTER = 3            # ov_duels.lua PLACED_AFTER
PAIR_PER_DAY = 3            # ov_duels.lua OV_DUELS_PAIR_PER_DAY: the same two can't farm the arena board either
WEEK0 = 4 * 86400           # Monday 5 Jan 1970 00:00 UTC: week numbers count from here

NAME_RE = re.compile(r"^[A-Za-z]{2,12}$")
KEY_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,31}$")
REALM_ID_RE = re.compile(r"^[a-z2-7]{16}$")
# stream/hub.py BANNED_RE (the brand words; "WoW" inside a name counts too)
BANNED_RE = re.compile(
    r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard)"
    r"|(?i:(?<![a-z])wow(?![a-z]))"
    r"|WoW|WOW|(?<![a-z])Wow(?=[A-Z0-9])|(?<=[a-z])Wow(?![a-z])")
DIFFS = ("normal", "heroic")
CLASSES = ("warrior", "paladin", "hunter", "rogue", "priest", "deathknight", "shaman", "mage", "warlock", "druid")
CLASS_BY_ID = {1: "warrior", 2: "paladin", 3: "hunter", 4: "rogue", 5: "priest", 6: "deathknight", 7: "shaman",
               8: "mage", 9: "warlock", 11: "druid"}
# Boss Rush's catalog in the launcher's order (launcher/src-tauri/src/host/bossrush.rs FIGHTS; a test keeps them equal).
# The official realm offers only the first; a Boss Rush install offers all of them.
CATALOG = ("frozen-throne", "bone-lord", "cult-mistress", "twilight-destroyer", "obsidian-sanctum", "obsidian-three",
           "brood-mother", "stone-watcher", "storm-watcher", "flame-watcher", "frost-watcher", "spell-weaver")
OFFICIAL_FIGHTS = ("frozen-throne",)

# ov_boards.lua's two tables (the same text; stream/tests/test_boards_export.py keeps the copies equal)
SCHEMA = {
    "ov_board_deaths": "CREATE TABLE IF NOT EXISTS ov_board_deaths (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, "
        "at INT UNSIGNED NOT NULL, instance_id INT UNSIGNED NOT NULL, map SMALLINT UNSIGNED NOT NULL, "
        "guid INT UNSIGNED NOT NULL, bot TINYINT UNSIGNED NOT NULL DEFAULT 0, KEY inst (instance_id, at))",
    "ov_arena_log": "CREATE TABLE IF NOT EXISTS ov_arena_log (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, "
        "at INT UNSIGNED NOT NULL, week CHAR(10) NOT NULL, day CHAR(10) NOT NULL, bracket TINYINT UNSIGNED NOT NULL, "
        "map SMALLINT UNSIGNED NOT NULL, secs INT UNSIGNED NOT NULL DEFAULT 0, "
        "winner INT UNSIGNED NOT NULL, winner_name VARCHAR(12) NOT NULL, winner_class TINYINT UNSIGNED NOT NULL, "
        "loser INT UNSIGNED NOT NULL, loser_name VARCHAR(12) NOT NULL, loser_class TINYINT UNSIGNED NOT NULL, "
        "KEY week (week), KEY pair_day (day, winner, loser))",
}


# ---- keys and signatures ---------------------------------------------------------------------------------------

_PURE = []


def _pure():
    if _PURE:
        return _PURE[0]
    for p in (os.path.join(HERE, "ed25519_pure.py"),
              os.path.join(HERE, "..", "..", "hosting", "kit", "tools", "wunet", "ed25519_pure.py"),
              os.path.join(HERE, "..", "..", "stream", "ed25519_pure.py")):
        if os.path.isfile(p):
            spec = importlib.util.spec_from_file_location("wu_board_ed25519", p)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            _PURE.append(mod)
            return mod
    raise ImportError("ed25519_pure.py not found beside boards-export.py")


try:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey
    from cryptography.hazmat.primitives import serialization
    from cryptography.exceptions import InvalidSignature
except ImportError:  # the host's own python may not have it
    Ed25519PrivateKey = None


def public_key(secret: bytes) -> bytes:
    if Ed25519PrivateKey is not None and os.environ.get("WU_PURE_ED25519") != "1":
        return Ed25519PrivateKey.from_private_bytes(secret).public_key().public_bytes(
            serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    return _pure().public_key(secret)


def _sign(secret: bytes, msg: bytes) -> bytes:
    if Ed25519PrivateKey is not None and os.environ.get("WU_PURE_ED25519") != "1":
        return Ed25519PrivateKey.from_private_bytes(secret).sign(msg)
    return _pure().sign(secret, msg)


def verify(public: bytes, msg: bytes, sig: bytes) -> bool:
    if len(public) != 32 or len(sig) != 64:
        return False
    if Ed25519PrivateKey is not None and os.environ.get("WU_PURE_ED25519") != "1":
        try:
            Ed25519PublicKey.from_public_bytes(public).verify(sig, msg)
            return True
        except (InvalidSignature, ValueError):
            return False
    return _pure().verify(public, msg, sig)


def canonical(doc) -> bytes:
    """The bytes a signature covers: sorted keys, no spaces, UTF-8 (wunet/keys.py canonical)."""
    return json.dumps(doc, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def sign_doc(secret: bytes, doc: dict, context: str) -> str:
    return _sign(secret, context.encode() + b"\n" + canonical(doc)).hex()


def verify_doc(public: bytes, doc, context: str, sig_hex) -> bool:
    try:
        sig = bytes.fromhex(sig_hex)
    except (ValueError, TypeError):
        return False
    return verify(public, context.encode() + b"\n" + canonical(doc), sig)


def realm_id(public: bytes) -> str:
    return base64.b32encode(hashlib.sha256(public).digest()).decode().lower()[:16]


def load_or_create_key(path: str) -> bytes:
    """The realm's secret (32 bytes, hex in the file; wunet/keys.py load_or_create). Made 0600 the first time."""
    if os.path.exists(path):
        with open(path, "r", encoding="ascii") as f:
            s = bytes.fromhex(f.read().strip())
        if len(s) != 32:
            raise ValueError("the board key file is not a realm key")
        return s
    s = os.urandom(32)
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w", encoding="ascii") as f:
        f.write(s.hex() + "\n")
    return s


# ---- pure parts (stream/tests/test_boards_export.py) -----------------------------------------------------------

def week_start(t: int) -> int:
    """Monday 00:00 UTC of t's week (the boss boards' and the duels' week)."""
    return WEEK0 + (int(t) - WEEK0) // (7 * 86400) * 7 * 86400


def day(t: int) -> str:
    return time.strftime("%Y-%m-%d", time.gmtime(t))


def featured(t: int, fights=CATALOG) -> str:
    """This week's featured fight: the catalog in turn, one a week (bossrush.rs featured_fight: the same rule)."""
    n = (int(t) - WEEK0) // (7 * 86400)
    return fights[n % len(fights)]


def clean_name(name, hidden: set) -> str | None:
    if not isinstance(name, str) or not NAME_RE.fullmatch(name) or BANNED_RE.search(name) or name.lower() in hidden:
        return None
    return name


def _int(v, lo=0, hi=2**31 - 1) -> int:
    try:
        n = int(v)
    except (TypeError, ValueError):
        return lo
    return min(max(n, lo), hi)


def parse_roster(text: str, hidden: set) -> list[dict]:
    """'Name:class:bot|player,...' (the module's roster) -> [{"name", "class", "bot"}]; unreadable parts skipped."""
    out = []
    for part in (text or "").split(","):
        bits = part.strip().split(":")
        if len(bits) != 3 or bits[2] not in ("bot", "player"):
            continue
        out.append({"name": clean_name(bits[0], hidden), "class": bits[1] if bits[1] in CLASSES else "unknown",
                    "bot": bits[2] == "bot"})
    return out[:40]


RUN_FIELDS = ("id", "boss", "difficulty", "size", "ms", "started_at", "ended_at", "leader", "roster", "bots", "instance_id")


def runs_from_rows(rows) -> list[dict]:
    out = []
    for r in rows:
        if len(r) < len(RUN_FIELDS):
            continue
        d = dict(zip(RUN_FIELDS, r))
        if not KEY_RE.fullmatch(d["boss"] or "") or d["difficulty"] not in DIFFS:
            continue
        for k in ("id", "size", "ms", "started_at", "ended_at", "bots", "instance_id"):
            d[k] = _int(d[k])
        if d["ms"] <= 0 or d["ms"] > 6 * 3600 * 1000:
            continue
        out.append(d)
    return out


def count_deaths(runs: list[dict], deaths) -> None:
    """Each run's deaths: ov_boards.lua's rows in the run's own instance between its pull and its end."""
    by_inst: dict[int, list[int]] = {}
    for row in deaths:
        if len(row) >= 2:
            by_inst.setdefault(_int(row[0]), []).append(_int(row[1]))
    for run in runs:
        ats = by_inst.get(run["instance_id"], [])
        lo = run["started_at"] or (run["ended_at"] - run["ms"] // 1000 - 1)
        run["deaths"] = sum(1 for a in ats if lo <= a <= run["ended_at"])


def bossrush_results(runs: list[dict], hidden: set, rid: str, now: int, with_deaths: bool) -> list[dict]:
    """Each leader's best of all time and best this week, per fight and difficulty (the viewer ranks)."""
    wk = week_start(now)
    best: dict = {}
    for run in runs:
        roster = parse_roster(run["roster"], hidden)
        leader = clean_name(run["leader"], hidden)
        if not leader:
            continue
        me = next((p for p in roster if p["name"] == leader), None)
        r = {"v": VERSION, "realm": rid, "mode": "bossrush", "board": run["boss"], "diff": run["difficulty"],
             "kind": "bots" if run["bots"] > 0 else "group", "name": leader, "class": me["class"] if me else "unknown",
             "ms": run["ms"], "size": run["size"] or len(roster), "bots": run["bots"], "party": roster,
             "at": run["ended_at"], "run": run["id"]}
        if with_deaths:
            r["deaths"] = run.get("deaths", 0)
        order = (run["ms"], run["ended_at"], run["id"])
        for period in ("all", "week") if run["ended_at"] >= wk else ("all",):
            k = (run["boss"], run["difficulty"], period, leader.lower())
            if k not in best or order < best[k][0]:
                best[k] = (order, r)
    per: dict = {}
    for (boss, diff, period, _), (order, r) in best.items():
        per.setdefault((boss, diff, period), []).append((order, r))
    out, seen = [], set()
    for key in sorted(per):
        for _, r in sorted(per[key], key=lambda x: x[0])[:MAX_PER_BOARD]:
            if r["run"] not in seen:
                seen.add(r["run"])
                out.append(r)
    return out


def duel_results(alltime, weekly, rid: str, now: int, hidden: set) -> list[dict]:
    out = []
    wk = "week:" + day(week_start(now))
    for rows, period in ((alltime, "all"), (weekly, wk)):
        seen = set()
        for r in rows:
            if len(r) < 6:
                continue
            name = clean_name(r[0], hidden)
            if not name or name.lower() in seen:
                continue
            seen.add(name.lower())
            out.append({"v": VERSION, "realm": rid, "mode": "duels", "board": "duel", "period": period, "name": name,
                        "class": CLASS_BY_ID.get(_int(r[1]), "unknown"), "rating": _int(r[2], 0, 100000) if period == "all" else 0,
                        "wins": _int(r[3]), "losses": _int(r[4]), "points": _int(r[5]), "at": now})
            if len(seen) >= MAX_PER_BOARD:
                break
    return out


def arena_results(ladder, log, rid: str, now: int, hidden: set) -> list[dict]:
    """The 1v1 arena: ratings (mod-1v1-arena's teams) and this week's wins (ov_arena_log, PAIR_PER_DAY a pair a day)."""
    out, seen = [], set()
    for r in ladder:
        if len(r) < 5:
            continue
        name = clean_name(r[0], hidden)
        if not name or name.lower() in seen:
            continue
        seen.add(name.lower())
        games, wins = _int(r[4]), _int(r[3])
        out.append({"v": VERSION, "realm": rid, "mode": "duels", "board": "arena-1v1", "period": "all", "name": name,
                    "class": CLASS_BY_ID.get(_int(r[1]), "unknown"), "rating": _int(r[2], 0, 100000), "wins": wins,
                    "losses": max(0, games - wins), "points": 0, "at": now})
        if len(seen) >= MAX_PER_BOARD:
            break
    pairs: dict = {}
    tally: dict = {}
    for r in log:   # winner, winner_name, winner_class, loser, loser_name, loser_class, day
        if len(r) < 7:
            continue
        pk = (r[6], r[0], r[3])
        pairs[pk] = pairs.get(pk, 0) + 1
        if pairs[pk] > PAIR_PER_DAY:
            continue
        for guid, nm, cls, won in ((r[0], r[1], r[2], True), (r[3], r[4], r[5], False)):
            t = tally.setdefault(guid, {"name": nm, "class": cls, "wins": 0, "losses": 0})
            t["wins" if won else "losses"] += 1
    wk = "week:" + day(week_start(now))
    rows = sorted(tally.values(), key=lambda t: (-t["wins"], t["losses"], t["name"].lower()))
    n, seen = 0, set()
    for t in rows:
        name = clean_name(t["name"], hidden)
        if not name or t["wins"] == 0 or name.lower() in seen:
            continue
        seen.add(name.lower())
        out.append({"v": VERSION, "realm": rid, "mode": "duels", "board": "arena-1v1", "period": wk, "name": name,
                    "class": CLASS_BY_ID.get(_int(t["class"]), "unknown"), "rating": 0, "wins": t["wins"],
                    "losses": t["losses"], "points": 0, "at": now})
        n += 1
        if n >= MAX_PER_BOARD:
            break
    return out


def make_board(secret: bytes, results: list[dict], now: int, name: str = REALM_NAME, fights=OFFICIAL_FIGHTS) -> dict:
    """The signed board: each result signed, then the whole document."""
    pub = public_key(secret)
    rid = realm_id(pub)
    signed = []
    for r in results[:MAX_RESULTS]:
        r = {**r, "realm": rid}
        signed.append({"r": r, "sig": sign_doc(secret, r, CTX_RESULT)})
    doc = {"v": VERSION, "realm": rid, "pubkey": pub.hex(), "name": (name or "")[:40], "ts": now, "seq": now * 1000,
           "week": day(week_start(now)), "featured": {"bossrush": featured(now, fights)}, "results": signed}
    return {"doc": doc, "sig": sign_doc(secret, doc, CTX_BOARD)}


def check_board(board, now: int | None = None, max_age: int = 400 * 86400) -> tuple[bool, str]:
    """What every reader does (stream/boards.py, launcher boards.rs): the key makes the id, the document and every
    result are signed by it, every result names this realm."""
    if not isinstance(board, dict) or not isinstance(board.get("doc"), dict):
        return False, "shape"
    doc = board["doc"]
    try:
        pub = bytes.fromhex(doc.get("pubkey", ""))
    except (ValueError, TypeError):
        return False, "pubkey"
    if len(pub) != 32 or realm_id(pub) != doc.get("realm"):
        return False, "realm id"
    if not verify_doc(pub, doc, CTX_BOARD, board.get("sig")):
        return False, "signature"
    if now is not None and not (isinstance(doc.get("ts"), int) and doc["ts"] <= now + 300 and now - doc["ts"] <= max_age):
        return False, "expired"
    res = doc.get("results")
    if not isinstance(res, list):
        return False, "results"
    for x in res:
        if not isinstance(x, dict) or not isinstance(x.get("r"), dict) or x["r"].get("realm") != doc["realm"]:
            return False, "result realm"
        if not verify_doc(pub, x["r"], CTX_RESULT, x.get("sig")):
            return False, "result signature"
    return True, "ok"


# ---- the database ----------------------------------------------------------------------------------------------

WANT = ("leyline_bossfight_runs", "ov_board_deaths", "ov_armory_privacy", "ov_duel_stats", "ov_duel_week",
        "arena_team", "ov_arena_log")


def tables_sql() -> str:
    names = ", ".join(f"'{n}'" for n in WANT)
    return f"SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = '{CS}' AND TABLE_NAME IN ({names});\n"


def queries(have: set, now: int) -> list[tuple[str, str]]:
    """Fixed SELECTs on the tables that are there; the week keys are made here, never from input."""
    out = []
    bots = "JOIN acore_auth.account a ON a.id = c.account"
    if "ov_armory_privacy" in have:
        out.append(("hidden", f"SELECT c.name FROM {CS}.ov_armory_privacy p JOIN {CS}.characters c ON c.guid = p.guid WHERE p.hidden = 1"))
    if "leyline_bossfight_runs" in have:
        out.append(("runs", "SELECT r.id, r.boss, r.difficulty, r.size, r.ms, r.started_at, r.ended_at, IFNULL(c.name, ''), "
                            "REPLACE(REPLACE(r.roster, '\\t', ''), '\\n', ''), r.bots, r.instance_id "
                            f"FROM {CS}.leyline_bossfight_runs r LEFT JOIN {CS}.characters c ON c.guid = r.leader_guid "
                            "WHERE r.result = 'clear' AND r.gm = 0 AND r.tier = 0 AND r.ms > 0 AND c.deleteDate IS NULL "
                            f"ORDER BY r.ms LIMIT {MAX_RUNS}"))
        if "ov_board_deaths" in have:
            out.append(("deaths", f"SELECT d.instance_id, d.at FROM {CS}.ov_board_deaths d JOIN {CS}.leyline_bossfight_runs r "
                                  "ON r.instance_id = d.instance_id AND d.at BETWEEN r.started_at AND r.ended_at "
                                  "WHERE r.result = 'clear' LIMIT 500000"))
    if "ov_duel_stats" in have:
        out.append(("duel_all", "SELECT c.name, c.class, x.rating, x.wins, x.losses, x.points "
                                f"FROM {CS}.ov_duel_stats x JOIN {CS}.characters c ON c.guid = x.guid {bots} "
                                f"WHERE c.deleteDate IS NULL AND a.username NOT LIKE 'RNDBOT%' AND x.wins + x.losses >= {PLACED_AFTER} "
                                f"ORDER BY x.rating DESC, x.wins DESC, c.name LIMIT {MAX_PER_BOARD * 2}"))
    if "ov_duel_week" in have:
        wk = time.strftime("%Y-%W", time.gmtime(now))   # ov_duels.lua's season key
        out.append(("duel_week", "SELECT c.name, c.class, 0, x.wins, x.losses, x.points "
                                 f"FROM {CS}.ov_duel_week x JOIN {CS}.characters c ON c.guid = x.guid {bots} "
                                 f"WHERE c.deleteDate IS NULL AND a.username NOT LIKE 'RNDBOT%' AND x.week = '{wk}' AND x.points > 0 "
                                 f"ORDER BY x.points DESC, x.wins DESC, c.name LIMIT {MAX_PER_BOARD * 2}"))
    if "arena_team" in have:
        out.append(("arena", "SELECT c.name, c.class, t.rating, t.seasonWins, t.seasonGames "
                             f"FROM {CS}.arena_team t JOIN {CS}.characters c ON c.guid = t.captainGuid {bots} "
                             "WHERE t.type = 1 AND t.seasonGames > 0 AND c.deleteDate IS NULL AND a.username NOT LIKE 'RNDBOT%' "
                             f"ORDER BY t.rating DESC LIMIT {MAX_PER_BOARD * 2}"))
    if "ov_arena_log" in have:
        out.append(("arena_log", "SELECT winner, winner_name, winner_class, loser, loser_name, loser_class, day "
                                 f"FROM {CS}.ov_arena_log WHERE week = '{day(week_start(now))}' AND bracket = 1 "
                                 "ORDER BY id LIMIT 200000"))
    return out


def docker_runner(sql: str) -> str:
    """The real database: mysql inside its container, read-only session, batch output."""
    cmd = ["docker", "exec", "-i", DB_CONTAINER, "sh", "-c",
           'MYSQL_PWD="$MYSQL_ROOT_PASSWORD" exec mysql -uroot -N -B --default-character-set=utf8mb4']
    r = subprocess.run(cmd, input=("SET SESSION TRANSACTION READ ONLY;\n" + sql).encode("utf-8"),
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
    if r.returncode != 0:
        raise RuntimeError("mysql failed: " + r.stderr.decode("utf-8", "replace").strip()[:300])
    return r.stdout.decode("utf-8", "replace")


def parse_batch(text: str, keys) -> dict[str, list[list[str]]]:
    out: dict[str, list[list[str]]] = {}
    cur = None
    for line in text.split("\n"):
        if line.startswith("#Q "):
            cur = line[3:].strip()
            out[cur] = []
        elif line and cur is not None:
            out[cur].append(line.split("\t"))
    missing = [k for k in keys if k not in out]
    if missing:
        raise RuntimeError("no result for " + ", ".join(missing))
    return out


def build_results(runner, rid: str, now: int) -> list[dict]:
    have = {r.strip() for r in runner(tables_sql()).split("\n") if r.strip()}
    qs = queries(have, now)
    res = parse_batch(runner("".join(f"SELECT '#Q {k}';\n{q};\n" for k, q in qs)), [k for k, _ in qs]) if qs else {}
    hidden = {r[0].lower() for r in res.get("hidden", []) if r}
    runs = runs_from_rows(res.get("runs", []))
    with_deaths = "deaths" in res
    if with_deaths:
        count_deaths(runs, res["deaths"])
    return (bossrush_results(runs, hidden, rid, now, with_deaths)
            + duel_results(res.get("duel_all", []), res.get("duel_week", []), rid, now, hidden)
            + arena_results(res.get("arena", []), res.get("arena_log", []), rid, now, hidden))


def _write_json(path: str, obj) -> None:
    d = os.path.dirname(path)
    fd, tmp = tempfile.mkstemp(dir=d, prefix=".tmp-")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(obj, f, ensure_ascii=True, separators=(",", ":"))
        os.chmod(tmp, 0o644)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def run(runner=docker_runner, out: str = OUT, now: int | None = None, key_file: str = KEY_FILE,
        name: str = REALM_NAME) -> dict:
    now = int(time.time()) if now is None else now
    secret = load_or_create_key(key_file)
    rid = realm_id(public_key(secret))
    board = make_board(secret, build_results(runner, rid, now), now, name)
    ok, why = check_board(board, now)
    if not ok:
        raise RuntimeError("the board does not check: " + why)
    folder = os.path.join(out, "boards")
    os.makedirs(folder, exist_ok=True)
    _write_json(os.path.join(folder, rid + ".json"), board)
    for f in os.listdir(folder):   # this folder holds only this realm's board: an older key's board goes
        if f.endswith(".json") and REALM_ID_RE.fullmatch(f[:-5] or "") and f[:-5] != rid:
            os.unlink(os.path.join(folder, f))
    return board


def post(board: dict, url: str) -> str:
    """A community realm: send the signed board to the site (it checks the signature and that the realm is listed)."""
    if not url.startswith("https://"):
        raise ValueError("the board goes to an https address only")
    req = urllib.request.Request(url, data=json.dumps(board, separators=(",", ":")).encode("utf-8"),
                                 headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=30) as r:   # noqa: S310: https only, checked above
        return r.read(2000).decode("utf-8", "replace")


def schema_sql() -> str:
    return "".join(q + ";\n" for q in SCHEMA.values())


def main(argv=None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if argv == ["--schema"]:
        sys.stdout.write(schema_sql())
        return 0
    if argv == ["--whoami"]:
        pub = public_key(load_or_create_key(KEY_FILE))
        print(f"board realm id {realm_id(pub)} public key {pub.hex()}")
        return 0
    try:
        board = run()
        if len(argv) == 2 and argv[0] == "--post":
            print("posted: " + post(board, argv[1])[:200])
    except Exception as e:  # noqa: BLE001: cron: one line, the old board stays
        print("boards export failed: " + str(e)[:300], file=sys.stderr)
        return 1
    doc = board["doc"]
    modes = {}
    for x in doc["results"]:
        modes[x["r"]["mode"]] = modes.get(x["r"]["mode"], 0) + 1
    print(f"boards export: realm {doc['realm']}, {modes.get('bossrush', 0)} Boss Rush and {modes.get('duels', 0)} Duels "
          f"results signed, featured {doc['featured']['bossrush']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
