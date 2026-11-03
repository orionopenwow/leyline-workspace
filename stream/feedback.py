#!/usr/bin/env python3
"""Feedback, sorted: every few hours, what players said (bug reports, the forum, the Discord's bug channel) grouped
into themes with a simple mood score; when to push updates (the quietest hours, from who was online); and the
launcher version policy the portal publishes. Owner, 3 Nov: "Automatic feedback and bug report collection into
sentiment every few hours, an approval window for changes and upgrades, and a suggested time window for pushing
updates constantly so the launchers everyone has stay consistent and connected."

Read-only on everything it reads. It writes only its own files:
    <out>/feedback-digest.json          the newest digest (HQ > Review > Feedback shows it)
    <out>/digests/<stamp>.json          the last DIGESTS_KEPT digests (the trend compares with the one before)
    <out>/presence.jsonl                one line per sample {at, online}: the portal's sampler (sample()) appends it

Sources (each optional; a missing or broken one is counted as 0 and named in "skipped"):
    site / launcher   $WU_PORTAL_DATA/bugs/intake.jsonl (+ .1): stream/bugreport.py
    discord           <wu>/bugs/intake/discord.jsonl: the Discord bridge's #bug-reports intake
    game              <wu>/bugs/backlog.json: the bugs manifest (gmconsole/server/bugs_manifest.py), its game reports
    forum             $WU_PORTAL_DATA/forum.sqlite3: forum posts that are not hidden (opened read-only)
Every text is redacted on the way in (emails, addresses, links with tokens, long numbers, keys) and quoted at most
QUOTE_MAX characters; player text is data, never instructions.

Themes: keyword lists (THEMES); one text can be in several; the rest is "other". Mood: a word list (LEXICON:
words that lean good or bad, with "not"/"never" flipping the next few words and "very"/"so" adding weight), a
score from -1 to 1 per text. Both are pluggable: --lexicon FILE.json ({"good": {...}, "bad": {...}, "themes":
{id: {"label", "words"}}}) adds to or replaces entries, and --scorer FILE.py uses that file's score(text) -> float
(-1..1) instead of the word list. No paid service and nothing leaves the machine.

The release window: hours of the week (UTC) by the average players online over the last PRESENCE_DAYS days, and
the quietest WINDOW_HOURS-hour slot with enough samples (MIN_SAMPLES per hour); without enough data, the default
(DEFAULT_WINDOW). Shown in UTC and Phoenix time (the owner's; UTC-7 all year).

    python3 -I feedback.py digest [--data DIR] [--wu DIR] [--out DIR] [--hours 72] [--lexicon F] [--scorer F]
    python3 -I feedback.py window [--out DIR]
Last line: "FEEDBACK OK: <n> items, <t> themes, mood <m>" or "FEEDBACK FAILED: <why>". Standard library only.
"""
from __future__ import annotations

import calendar
import importlib.util
import json
import os
import re
import sqlite3
import sys
import tempfile
import threading
import time

FORMAT = 1
HOURS = 72                  # the window a digest looks at; the trend compares with the HOURS before it
DIGESTS_KEPT = 60
QUOTE_MAX = 160
EXAMPLES = 3
TEXT_MAX = 2000
ITEMS_MAX = 20000
PRESENCE_DAYS = 28
PRESENCE_MAX_LINES = 20000
SAMPLE_EVERY = 600          # the portal's sampler: one sample every 10 minutes
WINDOW_HOURS = 2
MIN_SAMPLES = 3             # per hour of the week before that hour counts
DEFAULT_WINDOW = {"weekday": 1, "hour": 10}   # Tuesday 10:00 UTC (03:00 in Phoenix) until there is data
PHOENIX = -7
DAYS = ("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")

THEMES = {
    "crash": ("Crashes and errors", ("crash", "crashes", "crashed", "error", "exception", "closes", "closed itself",
                                     "freeze", "freezes", "frozen", "hang", "hangs", "stuck", "broken", "won't start",
                                     "doesn't start", "black screen", "white screen")),
    "login": ("Log in and Hero Key", ("log in", "login", "logged out", "sign in", "signin", "hero key", "herokey",
                                      "password", "account", "google")),
    "connect": ("Connecting and realms", ("disconnect", "disconnected", "can't connect", "cannot connect", "connection",
                                          "realm down", "server down", "offline", "lag", "latency", "ping", "relay",
                                          "realm list", "timeout", "timed out")),
    "launcher": ("Launcher and updates", ("launcher", "update", "updates", "updating", "install", "installer",
                                          "download", "game folder", "game folders", "patch")),
    "performance": ("Speed and graphics", ("fps", "slow", "stutter", "laggy", "performance", "graphics", "shader",
                                           "resolution", "hd", "looks", "texture", "textures")),
    "den": ("Your Den", ("den", "dens", "build mode", "forge", "shape ground", "stamp", "props")),
    "mods": ("Mods and addons", ("mod", "mods", "addon", "addons", "workshop", "modpack")),
    "gameplay": ("Quests and gameplay", ("quest", "quests", "npc", "boss", "dungeon", "raid", "loot", "talent", "spell",
                                         "level", "leveling", "levelling", "bots", "bot", "companion", "pvp", "duel")),
    "browser": ("Browser and phones", ("browser", "phone", "mobile", "touch", "tablet", "chrome", "safari", "firefox")),
    "modes": ("Game modes", ("survivors", "minigolf", "game mode", "game modes", "golf")),
    "community": ("Forum, Discord and friends", ("forum", "discord", "friend", "friends", "chat", "message", "guild")),
}

LEXICON_GOOD = {
    "love": 2, "loved": 2, "awesome": 2, "amazing": 2, "great": 1.5, "good": 1, "nice": 1, "fun": 1.5, "cool": 1,
    "thanks": 1, "thank": 1, "works": 1, "working": 0.5, "fixed": 1, "smooth": 1.5, "fast": 1, "easy": 1,
    "beautiful": 1.5, "best": 1.5, "enjoy": 1.5, "enjoying": 1.5, "perfect": 2, "excellent": 2, "happy": 1.5,
    "like": 0.5, "helpful": 1, "clean": 0.5, "stable": 1, "glad": 1,
}
LEXICON_BAD = {
    "bug": 0.5, "bugs": 0.5, "broken": 1.5, "crash": 1.5, "crashes": 1.5, "crashed": 1.5, "error": 1, "errors": 1,
    "fail": 1, "fails": 1, "failed": 1, "stuck": 1, "slow": 1, "lag": 1, "laggy": 1, "freeze": 1.5, "freezes": 1.5,
    "bad": 1, "worse": 1.5, "worst": 2, "hate": 2, "annoying": 1.5, "frustrating": 1.5, "confusing": 1, "ugly": 1,
    "can't": 0.5, "cannot": 0.5, "unplayable": 2.5, "terrible": 2, "awful": 2, "useless": 2, "missing": 0.5,
    "wrong": 1, "disconnect": 1, "disconnected": 1, "problem": 1, "problems": 1, "issue": 0.5, "issues": 0.5,
    "doesn't": 0.5, "won't": 0.5, "angry": 1.5, "sad": 1, "lost": 0.5, "kicked": 1,
}
NEGATORS = {"not", "no", "never", "isn't", "wasn't", "don't", "didn't", "hardly", "without"}
BOOSTERS = {"very": 1.5, "so": 1.3, "really": 1.3, "super": 1.5, "extremely": 1.8, "totally": 1.3}

_REDACT = [
    (re.compile(r"[\w.+-]+@[\w-]+\.[\w.-]+"), "[email]"),
    (re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b"), "[address]"),
    (re.compile(r"(?i)\bovk1\.[\w.-]+"), "[key]"),
    (re.compile(r"(?i)\b(?:token|key|secret|password|pass)\s*[:=]\s*\S+"), "[secret]"),
    (re.compile(r"https?://\S+"), "[link]"),
    (re.compile(r"\b\d{7,}\b"), "[number]"),
    (re.compile(r"(?i)[a-z]:\\users\\[^\\\s]+"), r"C:\\Users\\[user]"),
    (re.compile(r"/home/[^/\s]+"), "/home/[user]"),
]
WORD_RE = re.compile(r"[a-z][a-z'-]*")


def redact(text) -> str:
    s = " ".join(str(text or "").split())[:TEXT_MAX]
    for rx, sub in _REDACT:
        s = rx.sub(sub, s)
    return s


def quote(text) -> str:
    s = redact(text)
    return s if len(s) <= QUOTE_MAX else s[:QUOTE_MAX - 1].rstrip() + "…"


def epoch(v) -> int:
    if isinstance(v, (int, float)) and not isinstance(v, bool):
        return int(v / 1000) if v > 10 ** 11 else int(v)
    m = re.match(r"^(\d{4})-(\d\d)-(\d\d)[T ](\d\d):(\d\d)(?::(\d\d))?", str(v or ""))
    if not m:
        return 0
    y, mo, d, h, mi, s = (int(x or 0) for x in m.groups())
    try:
        return calendar.timegm((y, mo, d, h, mi, s, 0, 0, 0))
    except (ValueError, OverflowError):
        return 0


# ------------------------------------------------------------------------------------------- scoring (pluggable)

class Lexicon:
    """The word-list scorer: score(text) in -1..1, themes(text) -> [theme ids]."""

    def __init__(self, extra: dict | None = None):
        self.good, self.bad = dict(LEXICON_GOOD), dict(LEXICON_BAD)
        self.themes = {k: (label, tuple(words)) for k, (label, words) in THEMES.items()}
        extra = extra if isinstance(extra, dict) else {}
        for name, target in (("good", self.good), ("bad", self.bad)):
            v = extra.get(name)
            if isinstance(v, dict):
                for w, n in v.items():
                    if isinstance(w, str) and isinstance(n, (int, float)) and not isinstance(n, bool):
                        target[w.lower()] = float(n)
        th = extra.get("themes")
        if isinstance(th, dict):
            for k, v in th.items():
                if re.fullmatch(r"[a-z][a-z0-9_-]{0,23}", str(k)) and isinstance(v, dict) and isinstance(v.get("words"), list):
                    self.themes[k] = (str(v.get("label") or k)[:40], tuple(str(w).lower() for w in v["words"] if str(w).strip()))

    def score(self, text: str) -> float:
        words = WORD_RE.findall(text.lower())
        total, flip, boost = 0.0, 0, 1.0
        for w in words:
            if w in NEGATORS:
                flip = 3
                continue
            if w in BOOSTERS:
                boost = BOOSTERS[w]
                continue
            v = self.good.get(w, 0.0) - self.bad.get(w, 0.0)
            if v:
                total += (-v if flip else v) * boost
            boost = 1.0
            flip = max(0, flip - 1)
        if not total:
            return 0.0
        return max(-1.0, min(1.0, total / (abs(total) + 2.0)))   # squashed: one strong word is about +-0.5

    def theme_ids(self, text: str) -> list[str]:
        low = " " + " ".join(WORD_RE.findall(text.lower())) + " "
        out = [k for k, (_, words) in self.themes.items() if any(f" {w} " in low for w in words)]
        return out or ["other"]

    def label(self, tid: str) -> str:
        return self.themes.get(tid, ("Everything else", ()))[0]


def load_scorer(lexicon_path: str = "", scorer_path: str = ""):
    """The Lexicon (with the extra word list when given), its score() replaced by a scorer file's when given."""
    extra = {}
    if lexicon_path:
        with open(lexicon_path, encoding="utf-8") as f:
            extra = json.load(f)
    lex = Lexicon(extra)
    if scorer_path:
        spec = importlib.util.spec_from_file_location("wu_feedback_scorer", scorer_path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        fn = getattr(mod, "score")

        def score(text, _fn=fn):
            try:
                return max(-1.0, min(1.0, float(_fn(text))))
            except Exception:   # noqa: BLE001: a bad scorer scores 0, never stops the digest
                return 0.0
        lex.score = score
    return lex


# ------------------------------------------------------------------------------------------- sources (read-only)

def _jsonl(path: str, limit_bytes: int = 8_000_000) -> list[dict]:
    out = []
    try:
        with open(path, "rb") as f:
            f.seek(0, 2)
            size = f.tell()
            f.seek(max(0, size - limit_bytes))
            data = f.read().decode("utf-8", "replace")
    except OSError:
        return out
    for ln in data.splitlines():
        ln = ln.strip()
        if ln.startswith("{"):
            try:
                d = json.loads(ln)
            except ValueError:
                continue
            if isinstance(d, dict):
                out.append(d)
    return out


def from_intake(path: str, default_source: str) -> list[dict]:
    out = []
    for p in (path + ".1", path):
        for d in _jsonl(p):
            src = d.get("source") if d.get("source") in ("site", "launcher", "discord", "game") else default_source
            text = d.get("text")
            if isinstance(text, str) and text.strip():
                out.append({"key": f"{src}:{d.get('id') or len(out)}", "source": src, "at": epoch(d.get("at")),
                            "text": text, "where": (d.get("where") or {}).get("kind", "") if isinstance(d.get("where"), dict) else ""})
    return out


def from_backlog(path: str) -> list[dict]:
    try:
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
    except (OSError, ValueError):
        return []
    out = []
    for it in doc.get("items", []) if isinstance(doc, dict) else []:
        for r in it.get("reports", []) if isinstance(it, dict) else []:
            if isinstance(r, dict) and isinstance(r.get("text"), str):
                out.append({"key": str(r.get("key") or ""), "source": str(r.get("source") or "game"), "at": epoch(r.get("at")),
                            "text": r["text"], "where": ""})
    return out


def from_forum(path: str, since: int) -> list[dict]:
    if not os.path.isfile(path):
        return []
    try:
        con = sqlite3.connect(f"file:{path}?mode=ro", uri=True, timeout=5)
        try:
            rows = con.execute("SELECT p.id, p.body, p.created, t.cat FROM forum_posts p JOIN forum_threads t ON t.id = p.thread "
                               "WHERE p.hidden = 0 AND t.hidden = 0 AND p.created >= ? ORDER BY p.id DESC LIMIT ?",
                               (since, ITEMS_MAX)).fetchall()
        finally:
            con.close()
    except sqlite3.Error:
        return []
    return [{"key": f"forum:{i}", "source": "forum", "at": int(c or 0), "text": str(b or ""), "where": str(cat or "")}
            for i, b, c, cat in rows]


def sources(data: str, wu: str = "") -> dict[str, str]:
    """name -> path of every source this machine might have. wu: the host's /srv/wrath (the Control step), else only
    what the portal's own data folder holds."""
    out = {"intake": os.path.join(data, "bugs", "intake.jsonl"), "forum": os.path.join(data, "forum.sqlite3")}
    if wu:
        out["discord"] = os.path.join(wu, "bugs", "intake", "discord.jsonl")
        out["backlog"] = os.path.join(wu, "bugs", "backlog.json")
    for extra in (os.environ.get("WU_FEEDBACK_EXTRA") or "").split(","):
        name, _, p = extra.strip().partition("=")
        if re.fullmatch(r"[a-z][a-z0-9_]{0,23}", name) and p:
            out[name] = p
    return out


def collect(paths: dict[str, str], since: int) -> tuple[list[dict], dict, list[str]]:
    items, counts, skipped = [], {}, []
    for name, p in paths.items():
        if not os.path.exists(p):
            skipped.append(name)
            counts[name] = 0
            continue
        if name == "forum":
            got = from_forum(p, since)
        elif p.endswith(".json"):
            got = from_backlog(p)
        else:
            got = from_intake(p, "discord" if name == "discord" else "site")
        counts[name] = len(got)
        items += got
    seen, out = set(), []
    for it in sorted(items, key=lambda x: -x["at"]):
        k = it["key"] or (it["source"] + ":" + str(hash(it["text"])))
        if k in seen or it["at"] < since:
            continue
        seen.add(k)
        out.append(it)
    return out[:ITEMS_MAX], counts, skipped


# ------------------------------------------------------------------------------------------- the digest

def _mood_word(v: float) -> str:
    return "good" if v >= 0.15 else "bad" if v <= -0.15 else "mixed"


def summarize(items: list[dict], lex) -> dict:
    themes: dict[str, dict] = {}
    pos = neg = 0
    total = 0.0
    for it in items:
        text = redact(it["text"])
        s = lex.score(text)
        total += s
        pos += s > 0.1
        neg += s < -0.1
        for tid in lex.theme_ids(text):
            t = themes.setdefault(tid, {"id": tid, "label": lex.label(tid), "count": 0, "sum": 0.0, "neg": 0,
                                        "sources": {}, "examples": []})
            t["count"] += 1
            t["sum"] += s
            t["neg"] += s < -0.1
            t["sources"][it["source"]] = t["sources"].get(it["source"], 0) + 1
            if len(t["examples"]) < EXAMPLES:
                t["examples"].append({"source": it["source"], "at": it["at"], "text": quote(text), "mood": round(s, 2)})
    out = []
    for t in themes.values():
        avg = t.pop("sum") / t["count"]
        t["mood"] = round(avg, 3)
        t["mood_word"] = _mood_word(avg)
        t["neg_share"] = round(t.pop("neg") / t["count"], 3)
        out.append(t)
    # most-mentioned first; among equals, the unhappier first
    out.sort(key=lambda t: (-t["count"], t["mood"]))
    n = len(items)
    return {"items": n, "mood": round(total / n, 3) if n else 0.0, "mood_word": _mood_word(total / n) if n else "quiet",
            "positive": pos, "negative": neg, "neutral": n - pos - neg, "themes": out}


def digest(data: str, out: str, wu: str = "", hours: int = HOURS, lex=None, now: int | None = None,
           paths: dict | None = None) -> dict:
    now = int(time.time() if now is None else now)
    lex = lex or Lexicon()
    since = now - hours * 3600
    paths = paths if paths is not None else sources(data, wu)
    items, counts, skipped = collect(paths, now - 2 * hours * 3600)
    cur = [i for i in items if i["at"] >= since]
    prev = [i for i in items if i["at"] < since]
    s_now, s_prev = summarize(cur, lex), summarize(prev, lex)
    before = {t["id"]: t for t in s_prev["themes"]}
    for t in s_now["themes"]:
        b = before.get(t["id"])
        t["trend"] = t["count"] - (b["count"] if b else 0)
        t["mood_trend"] = round(t["mood"] - b["mood"], 3) if b else None
    doc = {"format": FORMAT, "made_at": _iso(now), "made": now, "hours": hours, "since": _iso(since),
           "sources": counts, "skipped": skipped, **s_now,
           "before": {"items": s_prev["items"], "mood": s_prev["mood"]},
           "window": release_window(out, now)}
    write(out, doc, now)
    return doc


def _iso(t: int) -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(t))


def _atomic(path: str, text: str) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path), prefix=".tmp-")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(text)
        os.chmod(tmp, 0o640)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def write(out: str, doc: dict, now: int) -> None:
    text = json.dumps(doc, indent=1, ensure_ascii=False) + "\n"
    _atomic(os.path.join(out, "feedback-digest.json"), text)
    d = os.path.join(out, "digests")
    _atomic(os.path.join(d, time.strftime("%Y%m%d-%H%M%S", time.gmtime(now)) + ".json"), text)
    old = sorted(f for f in os.listdir(d) if re.fullmatch(r"\d{8}-\d{6}\.json", f))
    for f in old[:-DIGESTS_KEPT]:
        try:
            os.unlink(os.path.join(d, f))
        except OSError:
            pass


def latest(out: str) -> dict | None:
    try:
        with open(os.path.join(out, "feedback-digest.json"), encoding="utf-8") as f:
            d = json.load(f)
        return d if isinstance(d, dict) and d.get("format") == FORMAT else None
    except (OSError, ValueError):
        return None


# ------------------------------------------------------------------------------------------- presence and the window

_sample_lock = threading.Lock()


def sample(out: str, online: int, now: int | None = None) -> None:
    """One presence sample (players online now). Trimmed to the last PRESENCE_MAX_LINES lines now and then."""
    now = int(time.time() if now is None else now)
    if not isinstance(online, int) or isinstance(online, bool) or online < 0:
        return
    path = os.path.join(out, "presence.jsonl")
    with _sample_lock:
        os.makedirs(out, exist_ok=True)
        with open(path, "a", encoding="utf-8") as f:
            f.write(json.dumps({"at": now, "online": min(online, 100000)}) + "\n")
        try:
            if os.path.getsize(path) > PRESENCE_MAX_LINES * 40:
                lines = open(path, encoding="utf-8").read().splitlines()[-PRESENCE_MAX_LINES:]
                _atomic(path, "\n".join(lines) + "\n")
        except OSError:
            pass


def _slot(weekday: int, hour: int) -> dict:
    utc = {"weekday": weekday, "hour": hour}
    ph = (weekday * 24 + hour + PHOENIX) % (7 * 24)
    end = (hour + WINDOW_HOURS) % 24
    return {**utc, "hours": WINDOW_HOURS, "text": f"{DAYS[weekday]} {hour:02d}:00-{end:02d}:00 UTC",
            "phoenix": f"{DAYS[ph // 24]} {ph % 24:02d}:00-{(ph % 24 + WINDOW_HOURS) % 24:02d}:00 Phoenix time"}


def next_start(slot: dict, now: int) -> int:
    """The next time the slot starts, after now (UTC)."""
    t = time.gmtime(now)
    week_start = now - (t.tm_wday * 86400 + t.tm_hour * 3600 + t.tm_min * 60 + t.tm_sec)
    start = week_start + slot["weekday"] * 86400 + slot["hour"] * 3600
    return start if start > now else start + 7 * 86400


def release_window(out: str, now: int | None = None) -> dict:
    """The suggested slot for pushing updates: the quietest WINDOW_HOURS hours of the week, with the data behind it."""
    now = int(time.time() if now is None else now)
    since = now - PRESENCE_DAYS * 86400
    sums, ns = [0.0] * 168, [0] * 168
    for d in _jsonl(os.path.join(out, "presence.jsonl"), 2_000_000):
        at, n = d.get("at"), d.get("online")
        if isinstance(at, int) and isinstance(n, int) and at >= since:
            t = time.gmtime(at)
            i = t.tm_wday * 24 + t.tm_hour
            sums[i] += n
            ns[i] += 1
    avg = [sums[i] / ns[i] if ns[i] >= MIN_SAMPLES else None for i in range(168)]
    best, best_v = None, None
    for i in range(168):
        span = [avg[(i + k) % 168] for k in range(WINDOW_HOURS)]
        if any(v is None for v in span):
            continue
        v = sum(span) / WINDOW_HOURS
        if best_v is None or v < best_v - 1e-9:
            best, best_v = i, v
    known = sum(1 for v in avg if v is not None)
    if best is None:
        slot = _slot(DEFAULT_WINDOW["weekday"], DEFAULT_WINDOW["hour"])
        slot.update(basis="default (not enough player data yet)", avg_online=None)
    else:
        slot = _slot(best // 24, best % 24)
        busiest = max(v for v in avg if v is not None)
        slot.update(basis=f"quietest {WINDOW_HOURS} hours over the last {PRESENCE_DAYS} days ({known} of 168 hours known)",
                    avg_online=round(best_v, 1), busiest_avg=round(busiest, 1))
    slot["next_start"] = _iso(next_start(slot, now))
    slot["samples"] = sum(ns)
    return slot


# ------------------------------------------------------------------------------------------- the command line

def main(argv: list[str]) -> int:
    import argparse
    ap = argparse.ArgumentParser(prog="feedback.py")
    ap.add_argument("cmd", choices=("digest", "window"))
    ap.add_argument("--data", default=os.environ.get("WU_PORTAL_DATA", "/data"))
    ap.add_argument("--wu", default="")
    ap.add_argument("--out", default="")
    ap.add_argument("--hours", type=int, default=HOURS)
    ap.add_argument("--lexicon", default=os.environ.get("WU_SENTIMENT_LEXICON", ""))
    ap.add_argument("--scorer", default=os.environ.get("WU_SENTIMENT_SCORER", ""))
    o = ap.parse_args(argv)
    out = o.out or os.path.join(o.data, "ops")
    try:
        if o.cmd == "window":
            w = release_window(out)
            print(f"window: {w['text']} ({w['phoenix']}); {w['basis']}; next {w['next_start']}")
            print("FEEDBACK OK: window")
            return 0
        d = digest(o.data, out, o.wu, max(1, min(o.hours, 24 * 30)), load_scorer(o.lexicon, o.scorer))
    except Exception as ex:   # noqa: BLE001
        print(f"FEEDBACK FAILED: {type(ex).__name__}: {str(ex)[:200]}")
        return 1
    print("sources: " + ", ".join(f"{k} {v}" for k, v in d["sources"].items()) + (f" (not here: {', '.join(d['skipped'])})" if d["skipped"] else ""))
    for t in d["themes"][:8]:
        print(f"  {t['label']}: {t['count']} ({t['mood_word']}, trend {t['trend']:+d})")
    print(f"release window: {d['window']['text']} ({d['window']['phoenix']})")
    print(f"FEEDBACK OK: {d['items']} items, {len(d['themes'])} themes, mood {d['mood_word']} ({d['mood']:+.2f})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
