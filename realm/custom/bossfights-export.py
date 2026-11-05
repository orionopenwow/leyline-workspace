#!/usr/bin/env python3
"""Exports the Boss fights boards (and the 1v1 arena ladder) for wu-portal. Host side, stdlib only.

    python3 -I /srv/wrath/bossfights-export.py                # from cron, every 5 minutes:
    */5 * * * * root python3 -I /srv/wrath/bossfights-export.py >/dev/null 2>&1      (/etc/cron.d/wu-bossfights)

The same way as duels-export.py: wu-portal has no route to the database, so this runs fixed SELECTs (nothing from a
visitor ever reaches SQL) in the database container through its own root login, inside a READ ONLY session, and
writes its files atomically into the SAME folder as the duel boards (default /srv/wrath/portal/duels, which wu-portal
sees as /app/duels):

    bossfights.json  {"updated", "week", "week_start", "week_end",
                      "bosses": {"frozen-throne": {"normal": {"group": [...], "bots": [...], "weekly": [...]},
                                                   "heroic": {...}}}}
        a row:       {"rank", "ms", "time", "date", "leader", "roster": [{"name", "class", "bot"}], "size", "bots",
                      "bot_buffs"}   (+ "kind": "group"|"bots" on the weekly board)
        group:       clears with no bots; the best time of each set of players (the same ten names once)
        bots:        clears with bots in the raid; the best time of each leader
        weekly:      this week's clears (Monday 00:00 UTC on), either kind; the best of each leader
    arena.json       {"updated", "ladder": [{"name", "class", "rating", "wins", "losses"}]}   (mod-1v1-arena teams,
                     arena_team.type = 1, captains only, at least one game this season)

The data is mod-leyline-bossfights' own table (characters.leyline_bossfight_runs; docs/BOSS-FIGHTS.md). Only clears
count, and never a run with a GM in it (gm = 1), nor a run above tier 0 (tiers come later). The time is the
module's: from the boss engaging to the start of its 10% cast.

Privacy (the site checks again): character names and class words only, never account names, guids or ids; a
character whose player typed ".armory off" shows as name null; names that are not plain 2-12 letters, or that carry
a brand word, show as null too (a leader like that drops the row); arena bots (accounts RNDBOT*) are left out.

    WU_DUELS_OUT      the folder to write     default /srv/wrath/portal/duels
    WU_DB_CONTAINER   default ov-ac-database

On any failure nothing is written and the exit code is 1 (the site keeps the last good export).
Never prints a password: mysql runs in its container with the container's own MYSQL_ROOT_PASSWORD.
"""
from __future__ import annotations

import json, os, re, subprocess, sys, tempfile, time

OUT = os.environ.get("WU_DUELS_OUT", "/srv/wrath/portal/duels")
DB_CONTAINER = os.environ.get("WU_DB_CONTAINER", "ov-ac-database")
CS = "acore_characters"
NAME_RE = re.compile(r"^[A-Za-z]{2,12}$")
KEY_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,31}$")
# stream/hub.py BANNED_RE (the brand words; "WoW" inside a name counts too)
BANNED_RE = re.compile(
    r"(?i:world\s*of\s*warcraft|warcraft|lich\s*king|blizzard)"
    r"|(?i:(?<![a-z])wow(?![a-z]))"
    r"|WoW|WOW|(?<![a-z])Wow(?=[A-Z0-9])|(?<=[a-z])Wow(?![a-z])")
BOSSES = ("frozen-throne",)          # mod-leyline-bossfights BossfightsLogic.h Bosses()
DIFFS = ("normal", "heroic")
CLASSES = {"warrior", "paladin", "hunter", "rogue", "priest", "deathknight", "shaman", "mage", "warlock", "druid"}
CLASS_BY_ID = {1: "warrior", 2: "paladin", 3: "hunter", 4: "rogue", 5: "priest", 6: "deathknight", 7: "shaman",
               8: "mage", 9: "warlock", 11: "druid"}
MAX_ROWS = 100
MAX_RUNS = 50000

# The module's tables (CREATE TABLE IF NOT EXISTS at its start), the same text as its data/sql file, so the export
# works before the new realm image runs (--schema; stream/tests/test_bossfights_export.py keeps the copies equal).
SCHEMA = {
    "leyline_bossfight_runs": "CREATE TABLE IF NOT EXISTS leyline_bossfight_runs (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, "
        "boss VARCHAR(32) NOT NULL, difficulty VARCHAR(8) NOT NULL, mode VARCHAR(12) NOT NULL DEFAULT 'bots', "
        "size TINYINT UNSIGNED NOT NULL DEFAULT 0, ms INT UNSIGNED NOT NULL DEFAULT 0, started_at INT UNSIGNED NOT NULL DEFAULT 0, "
        "ended_at INT UNSIGNED NOT NULL DEFAULT 0, leader_guid INT UNSIGNED NOT NULL DEFAULT 0, instance_id INT UNSIGNED NOT NULL DEFAULT 0, "
        "roster TEXT NOT NULL, bots TINYINT UNSIGNED NOT NULL DEFAULT 0, bot_buffs TINYINT UNSIGNED NOT NULL DEFAULT 0, "
        "gm TINYINT UNSIGNED NOT NULL DEFAULT 0, tier TINYINT UNSIGNED NOT NULL DEFAULT 0, wipes_before SMALLINT UNSIGNED NOT NULL DEFAULT 0, "
        "result VARCHAR(8) NOT NULL, KEY board (boss, difficulty, result, ms), KEY ended (ended_at))",
    "leyline_bossfight_instances": "CREATE TABLE IF NOT EXISTS leyline_bossfight_instances (instance_id INT UNSIGNED NOT NULL PRIMARY KEY, "
        "boss VARCHAR(32) NOT NULL, created_at INT UNSIGNED NOT NULL DEFAULT 0)",
}


# ---- pure parts (stream/tests/test_bossfights_export.py) ---------------------------------------------------------

def week_bounds(t: int) -> tuple[int, int]:
    """Monday 00:00 UTC of t's week, and a week later (duels-export.py's seasons)."""
    g = time.gmtime(t)
    start = int(t) - g.tm_wday * 86400 - g.tm_hour * 3600 - g.tm_min * 60 - g.tm_sec
    return start, start + 7 * 86400


def fmt_ms(ms: int) -> str:
    """m:ss.t, as the game's chat line says it (BossfightsLogic.h FormatMs)."""
    tenths = ms // 100
    return f"{tenths // 600}:{(tenths // 10) % 60:02d}.{tenths % 10}"


def clean_name(name: str, hidden: set[str]) -> str | None:
    if not name or not NAME_RE.fullmatch(name) or BANNED_RE.search(name) or name.lower() in hidden:
        return None
    return name


def parse_roster(text: str, hidden: set[str]) -> list[dict]:
    """'Name:class:bot|player,...' -> [{"name", "class", "bot"}]; unreadable parts are skipped."""
    out = []
    for part in (text or "").split(","):
        bits = part.strip().split(":")
        if len(bits) != 3 or bits[2] not in ("bot", "player"):
            continue
        cls = bits[1] if bits[1] in CLASSES else "unknown"
        out.append({"name": clean_name(bits[0], hidden), "class": cls, "bot": bits[2] == "bot", "_key": bits[0].lower()})
    return out


def _int(v, lo=0, hi=2**31 - 1) -> int:
    try:
        n = int(v)
    except (TypeError, ValueError):
        return lo
    return min(max(n, lo), hi)


RUN_FIELDS = ("id", "boss", "difficulty", "mode", "size", "ms", "ended_at", "leader", "roster", "bots", "bot_buffs")


def runs_from_rows(rows) -> list[dict]:
    out = []
    for r in rows:
        if len(r) < len(RUN_FIELDS):
            continue
        d = dict(zip(RUN_FIELDS, r))
        if not KEY_RE.fullmatch(d["boss"] or "") or d["difficulty"] not in DIFFS:
            continue
        for k in ("id", "size", "ms", "ended_at", "bots", "bot_buffs"):
            d[k] = _int(d[k])
        if d["ms"] <= 0:
            continue
        out.append(d)
    return out


def _row(run: dict, roster: list[dict], leader: str) -> dict:
    return {"ms": run["ms"], "time": fmt_ms(run["ms"]),
            "date": time.strftime("%Y-%m-%d", time.gmtime(run["ended_at"])) if run["ended_at"] else None,
            "leader": leader, "roster": [{k: v for k, v in e.items() if not k.startswith("_")} for e in roster],
            "size": run["size"] or len(roster), "bots": run["bots"], "bot_buffs": bool(run["bot_buffs"])}


def _ranked(best: dict) -> list[dict]:
    rows = sorted(best.values(), key=lambda x: (x[0], x[1], x[2]))[:MAX_ROWS]
    out = []
    for i, (_ms, _at, _id, row) in enumerate(rows, 1):
        out.append({"rank": i, **row})
    return out


def build_boards(runs: list[dict], hidden: set[str], now: int, bosses=BOSSES) -> dict:
    week_start, _ = week_bounds(now)
    best: dict = {}
    for b in set(bosses) | {r["boss"] for r in runs}:
        for d in DIFFS:
            best[(b, d)] = {"group": {}, "bots": {}, "weekly": {}}
    for run in runs:
        leader = clean_name(run["leader"], hidden)
        if not leader:
            continue
        roster = parse_roster(run["roster"], hidden)
        boards = best[(run["boss"], run["difficulty"])]
        kind = "bots" if run["bots"] > 0 else "group"
        row = _row(run, roster, leader)
        order = (run["ms"], run["ended_at"], run["id"])
        if kind == "group":
            key = frozenset(e["_key"] for e in roster if not e["bot"]) or frozenset([leader.lower()])
        else:
            key = leader.lower()
        cur = boards[kind].get(key)
        if cur is None or order < cur[:3]:
            boards[kind][key] = (*order, row)
        if run["ended_at"] >= week_start:
            wk = boards["weekly"].get(leader.lower())
            if wk is None or order < wk[:3]:
                boards["weekly"][leader.lower()] = (*order, {**row, "kind": kind})
    out: dict = {}
    for (b, d), boards in sorted(best.items()):
        out.setdefault(b, {})[d] = {k: _ranked(v) for k, v in boards.items()}
    return out


LADDER_FIELDS = ("name", "class", "rating", "wins", "games")


def arena_ladder(rows, hidden: set[str]) -> list[dict]:
    out, seen = [], set()
    for r in rows:
        if len(r) < len(LADDER_FIELDS):
            continue
        name = clean_name(r[0], hidden)
        if not name or name.lower() in seen:
            continue
        seen.add(name.lower())
        games, wins = _int(r[4]), _int(r[3])
        out.append({"name": name, "class": CLASS_BY_ID.get(_int(r[1]), "unknown"), "rating": _int(r[2], 0, 100000),
                    "wins": wins, "losses": max(0, games - wins)})
    out.sort(key=lambda x: (-x["rating"], -x["wins"], x["name"]))
    return out[:1000]


# ---- the database --------------------------------------------------------------------------------------------

def tables_sql() -> str:
    names = ", ".join(f"'{n}'" for n in ("leyline_bossfight_runs", "ov_armory_privacy", "arena_team"))
    return f"SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = '{CS}' AND TABLE_NAME IN ({names});\n"


def queries(have: set) -> list[tuple[str, str]]:
    out = []
    if "ov_armory_privacy" in have:
        out.append(("hidden", f"SELECT c.name FROM {CS}.ov_armory_privacy p JOIN {CS}.characters c ON c.guid = p.guid WHERE p.hidden = 1"))
    if "leyline_bossfight_runs" in have:
        out.append(("runs", "SELECT r.id, r.boss, r.difficulty, r.mode, r.size, r.ms, r.ended_at, IFNULL(c.name, ''), "
                            "REPLACE(REPLACE(r.roster, '\\t', ''), '\\n', ''), r.bots, r.bot_buffs "
                            f"FROM {CS}.leyline_bossfight_runs r LEFT JOIN {CS}.characters c ON c.guid = r.leader_guid "
                            "WHERE r.result = 'clear' AND r.gm = 0 AND r.tier = 0 AND r.ms > 0 AND c.deleteDate IS NULL "
                            f"ORDER BY r.ms LIMIT {MAX_RUNS}"))
    if "arena_team" in have:
        out.append(("arena", "SELECT c.name, c.class, t.rating, t.seasonWins, t.seasonGames "
                             f"FROM {CS}.arena_team t JOIN {CS}.characters c ON c.guid = t.captainGuid "
                             "JOIN acore_auth.account a ON a.id = c.account "
                             "WHERE t.type = 1 AND t.seasonGames > 0 AND c.deleteDate IS NULL AND a.username NOT LIKE 'RNDBOT%' "
                             "ORDER BY t.rating DESC LIMIT 1000"))
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


def build(runner, now: int) -> tuple[dict, dict]:
    have = {r.strip() for r in runner(tables_sql()).split("\n") if r.strip()}
    qs = queries(have)
    res = parse_batch(runner("".join(f"SELECT '#Q {k}';\n{q};\n" for k, q in qs)), [k for k, _ in qs]) if qs else {}
    hidden = {r[0].lower() for r in res.get("hidden", []) if r}
    start, end = week_bounds(now)
    boards = {"updated": now, "week_start": time.strftime("%Y-%m-%d", time.gmtime(start)),
              "week_end": time.strftime("%Y-%m-%d", time.gmtime(end)),
              "bosses": build_boards(runs_from_rows(res.get("runs", [])), hidden, now)}
    arena = {"updated": now, "ladder": arena_ladder(res.get("arena", []), hidden)}
    return boards, arena


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


def run(runner=docker_runner, out: str = OUT, now: int | None = None) -> tuple[dict, dict]:
    now = int(time.time()) if now is None else now
    boards, arena = build(runner, now)
    os.makedirs(out, exist_ok=True)
    _write_json(os.path.join(out, "bossfights.json"), boards)
    _write_json(os.path.join(out, "arena.json"), arena)
    return boards, arena


def schema_sql() -> str:
    return "".join(q + ";\n" for q in SCHEMA.values())


def main() -> int:
    if sys.argv[1:] == ["--schema"]:   # serverpass.sh step 1g pipes this into mysql (the tables before the first export)
        sys.stdout.write(schema_sql())
        return 0
    try:
        boards, arena = run()
    except Exception as e:  # noqa: BLE001: cron: one line, the old export stays
        print("bossfights export failed: " + str(e)[:300], file=sys.stderr)
        return 1
    n = sum(len(v["group"]) + len(v["bots"]) for b in boards["bosses"].values() for v in b.values())
    print(f"bossfights export: {n} board rows, {len(arena['ladder'])} on the arena ladder")
    return 0


if __name__ == "__main__":
    sys.exit(main())
