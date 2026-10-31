#!/usr/bin/env python3
"""Exports the duel leaderboards for wu-portal (stream/duels.py, /wrath/leaderboards). Host side, stdlib only.

    python3 -I /srv/wrath/duels-export.py                # from cron, every 5 minutes:
    */5 * * * * root python3 -I /srv/wrath/duels-export.py >/dev/null 2>&1      (/etc/cron.d/wu-duels)

The same way as the armory (armory-export.py): wu-portal has no route to the database and no MySQL client, so this
runs fixed SELECTs (nothing from a visitor ever reaches SQL) in the database container through its own root login,
inside a READ ONLY session, and writes one file atomically into OUT (default /srv/wrath/portal/duels, which wu-portal
sees as /app/duels):

    leaderboards.json   {"updated", "week", "week_start", "week_end", "rated_this_week",
                         "weekly": [...], "last_week": [...], "alltime": [...]}

The data is ov_duels.lua's own tables (ov_duel_stats, ov_duel_week, ov_duel_log in acore_characters). It asks
information_schema first and never reads a table that isn't there (a missing table gives an empty board, not an
error). Names come from the characters table (renames show at once).

Privacy (stream/duels.py checks it all again):
  - player names, class and race only: never account ids or names, character guids, IPs or anything else;
  - bots (accounts named RNDBOT*) and deleted characters are left out; the account table is only joined to drop bots;
  - a character whose player typed ".armory off" (ov_armory_privacy.hidden = 1) is left out here too;
  - names that are not plain 2-12 letters are left out.

    WU_DUELS_OUT      the folder to write     default /srv/wrath/portal/duels
    WU_DB_CONTAINER   default ov-ac-database

On any failure nothing is written and the exit code is 1 (the site keeps the last good export).
Never prints a password: mysql runs in its container with the container's own MYSQL_ROOT_PASSWORD.
"""
from __future__ import annotations

import json, os, re, subprocess, sys, tempfile, time

OUT = os.environ.get("WU_DUELS_OUT", "/srv/wrath/portal/duels")
DB_CONTAINER = os.environ.get("WU_DB_CONTAINER", "ov-ac-database")
NAME_RE = re.compile(r"^[A-Za-z]{2,12}$")
PLACED_AFTER = 3        # rated duels before the all-time board (ov_duels.lua PLACED_AFTER)
MAX_ROWS = 1000
LAST_WEEK = 3           # last season's podium
CS = "acore_characters"

# The tables ov_duels.lua makes (CREATE TABLE IF NOT EXISTS at load), the same text serverpass.sh step 1d uses so the
# export works before the restart that loads the script (--schema; stream/tests/test_duels.py keeps the two copies equal).
SCHEMA = {
    "ov_duel_stats": "CREATE TABLE IF NOT EXISTS ov_duel_stats (guid INT UNSIGNED NOT NULL PRIMARY KEY, name VARCHAR(12) NOT NULL, "
                     "class TINYINT UNSIGNED NOT NULL DEFAULT 0, rating SMALLINT UNSIGNED NOT NULL DEFAULT 1500, "
                     "best SMALLINT UNSIGNED NOT NULL DEFAULT 1500, wins INT UNSIGNED NOT NULL DEFAULT 0, "
                     "losses INT UNSIGNED NOT NULL DEFAULT 0, points INT UNSIGNED NOT NULL DEFAULT 0, "
                     "streak SMALLINT UNSIGNED NOT NULL DEFAULT 0, best_streak SMALLINT UNSIGNED NOT NULL DEFAULT 0, "
                     "last_at INT UNSIGNED NOT NULL DEFAULT 0, KEY name (name), KEY rating (rating))",
    "ov_duel_week": "CREATE TABLE IF NOT EXISTS ov_duel_week (week CHAR(7) NOT NULL, guid INT UNSIGNED NOT NULL, name VARCHAR(12) NOT NULL, "
                    "class TINYINT UNSIGNED NOT NULL DEFAULT 0, points INT UNSIGNED NOT NULL DEFAULT 0, "
                    "wins INT UNSIGNED NOT NULL DEFAULT 0, losses INT UNSIGNED NOT NULL DEFAULT 0, PRIMARY KEY (week, guid), "
                    "KEY week_points (week, points))",
    "ov_duel_log": "CREATE TABLE IF NOT EXISTS ov_duel_log (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, at INT UNSIGNED NOT NULL, "
                   "day CHAR(10) NOT NULL, winner INT UNSIGNED NOT NULL, loser INT UNSIGNED NOT NULL, pair VARCHAR(24) NOT NULL, "
                   "kind TINYINT UNSIGNED NOT NULL, secs INT UNSIGNED NOT NULL DEFAULT 0, rated TINYINT UNSIGNED NOT NULL DEFAULT 0, "
                   "delta SMALLINT NOT NULL DEFAULT 0, KEY pair_day (pair, day))",
}


def week_key(t: int) -> str:
    """The season: ov_duels.lua's os.date("!%Y-%W"): Monday-based weeks, UTC."""
    return time.strftime("%Y-%W", time.gmtime(t))


def week_bounds(t: int) -> tuple[str, str]:
    g = time.gmtime(t)
    start = int(t) - g.tm_wday * 86400 - g.tm_hour * 3600 - g.tm_min * 60 - g.tm_sec
    return time.strftime("%Y-%m-%d", time.gmtime(start)), time.strftime("%Y-%m-%d", time.gmtime(start + 7 * 86400))


def tables_sql() -> str:
    names = ", ".join(f"'{n}'" for n in (*SCHEMA, "ov_armory_privacy"))
    return f"SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = '{CS}' AND TABLE_NAME IN ({names});\n"


def board_queries(have: set, now: int) -> list[tuple[str, str]]:
    """Fixed SELECTs on the tables that are there (the week keys are made here, never from input)."""
    hidden = "IFNULL(p.hidden, 0) = 0" if "ov_armory_privacy" in have else "1 = 1"
    priv = f"LEFT JOIN {CS}.ov_armory_privacy p ON p.guid = c.guid " if "ov_armory_privacy" in have else ""
    visible = (f"JOIN {CS}.characters c ON c.guid = x.guid JOIN acore_auth.account a ON a.id = c.account {priv}"
               f"WHERE c.deleteDate IS NULL AND a.username NOT LIKE 'RNDBOT%' AND {hidden}")
    out = []
    if "ov_duel_stats" in have:
        out.append(("alltime", "SELECT c.name, c.class, c.race, x.rating, x.best, x.wins, x.losses, x.points, x.best_streak, x.streak "
                               f"FROM {CS}.ov_duel_stats x {visible} AND x.wins + x.losses >= {PLACED_AFTER} "
                               f"ORDER BY x.rating DESC, x.wins DESC, c.name LIMIT {MAX_ROWS}"))
    if "ov_duel_week" in have:
        for key, wk, n in (("weekly", week_key(now), MAX_ROWS), ("last_week", week_key(now - 7 * 86400), LAST_WEEK)):
            out.append((key, "SELECT c.name, c.class, c.race, x.points, x.wins, x.losses "
                             f"FROM {CS}.ov_duel_week x {visible} AND x.week = '{wk}' AND x.points > 0 "
                             f"ORDER BY x.points DESC, x.wins DESC, c.name LIMIT {n}"))
    if "ov_duel_log" in have:
        out.append(("count", f"SELECT COUNT(*) FROM {CS}.ov_duel_log WHERE rated = 1 AND day >= '{week_bounds(now)[0]}'"))
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


def _int(v, lo=0, hi=2**31 - 1) -> int:
    try:
        n = int(v)
    except (TypeError, ValueError):
        return lo
    return min(max(n, lo), hi)


def _rows(rows, fields) -> list[dict]:
    out, seen = [], set()
    for r in rows:
        if len(r) < len(fields) or not NAME_RE.fullmatch(r[0]) or r[0].lower() in seen:
            continue
        seen.add(r[0].lower())
        rec = {"name": r[0]}
        for i, f in enumerate(fields[1:], 1):
            rec[f] = _int(r[i], 0, 100000)
        out.append(rec)
    return out


ALLTIME = ("name", "class", "race", "rating", "best", "wins", "losses", "points", "best_streak", "streak")
WEEKLY = ("name", "class", "race", "points", "wins", "losses")


def build(runner, now: int) -> dict:
    have = {r.strip() for r in runner(tables_sql()).split("\n") if r.strip()}
    qs = board_queries(have, now)
    res = parse_batch(runner("".join(f"SELECT '#Q {k}';\n{q};\n" for k, q in qs)), [k for k, _ in qs]) if qs else {}
    start, end = week_bounds(now)
    count = res.get("count") or [["0"]]
    return {"updated": now, "week": week_key(now), "week_start": start, "week_end": end,
            "rated_this_week": _int(count[0][0] if count and count[0] else 0),
            "placed_after": PLACED_AFTER,
            "weekly": _rows(res.get("weekly", []), WEEKLY), "last_week": _rows(res.get("last_week", []), WEEKLY),
            "alltime": _rows(res.get("alltime", []), ALLTIME)}


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


def run(runner=docker_runner, out: str = OUT, now: int | None = None) -> dict:
    now = int(time.time()) if now is None else now
    doc = build(runner, now)
    os.makedirs(out, exist_ok=True)
    _write_json(os.path.join(out, "leaderboards.json"), doc)
    return doc


def schema_sql() -> str:
    return "".join(q + ";\n" for q in SCHEMA.values())


def main() -> int:
    if sys.argv[1:] == ["--schema"]:   # serverpass.sh step 1d pipes this into mysql (the tables before the first export)
        sys.stdout.write(schema_sql())
        return 0
    try:
        doc = run()
    except Exception as e:  # noqa: BLE001: cron: one line, the old export stays
        print("duels export failed: " + str(e)[:300], file=sys.stderr)
        return 1
    print(f"duels export: {len(doc['alltime'])} rated, {len(doc['weekly'])} this week")
    return 0


if __name__ == "__main__":
    sys.exit(main())
