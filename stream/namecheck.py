"""One blocklist for names everywhere (owner 2026-10-31: "Bad actor measures, username blocks, in game name blocks").

Used by the website's display names (stream/social.py name_error, stream/profiles.py), the forum (names, titles and the
words starred out of posts, through social.py), Leyline HQ (Content > Blocked words, Needs you), the relay's realm list
and the host kit (hosting/kit/tools/wunet/namecheck.py: a copy, byte for byte, with blocklist.json beside it), and the
realm: ov_names.lua carries the same table (python3 stream/namecheck.py --lua realm/custom/lua_scripts/ov_names.lua
writes it) and the scripts step fills AzerothCore's reserved_name and profanity_name from --sql.

The words: blocklist.json beside this file (small and curated: slurs and hate terms, sexual terms, the worst profanity,
staff impersonation) plus the owner's own (blocked-words.json in the portal's data folder, added on HQ). Before
matching, a name or text is undone: invisible characters (zero-width, right-to-left overrides) out, look-alike letters
from other alphabets (Cyrillic "а", Greek "ο") and accents to plain letters, digits and signs to the letters they pass
for (0 o, 1 i, 3 e, 4 a, 5 s, 7 t, 8 b, 9 g, @ a, $ s, ! i, | l), repeated letters ("shiiit"), separators ("f.u.c.k",
"A d m i n") and run-together words ("TheRapist", "LeylineStaff").

How a word matches: "word" words only as a whole word (with a plural or -ed/-er ending), or a name that is nothing
else, so Scunthorpe, Grape, Therapist and Cassandra pass; "inside" words also inside a longer word, less the "allow"
words; staff "edge" words at the start or end of a name or a word in it. Names (name_hit) get all of it; text
(text_hit, mask: forum posts, bios, chat) gets slurs, sexual terms, profanity and the owner's words, word by word.

Standard library only. python3 namecheck.py --check NAME... prints what each name trips (for a quick look).
"""
from __future__ import annotations

import itertools
import json
import os
import re
import sys
import threading
import time
import unicodedata

HERE = os.path.dirname(os.path.abspath(__file__))
BUILTIN_FILE = os.path.join(HERE, "blocklist.json")
EXTRA_NAME = "blocked-words.json"     # in the portal's data folder ($WU_PORTAL_DATA): the owner's words, from HQ
HITS_NAME = "name-hits.jsonl"         # the refusals HQ shows in Needs you (a name, never a password)
KINDS = ("slur", "sexual", "profanity", "staff", "brand", "custom")
TEXT_KINDS = ("slur", "sexual", "profanity", "custom")
WORD_RE = re.compile(r"^[a-z]{2,24}$")
MODES = ("word", "inside")
# Short staff words a realm name may carry ("Mod Paradise", "Dev realm"): realm names skip these.
REALM_STAFF_SKIP = frozenset(("mod", "mods", "dev", "devs", "owner", "support"))

# Zero-width, joiners, bidi overrides and isolates, soft hyphen, Hangul fillers, variation selectors, tag characters.
INVISIBLE_RE = re.compile("[­͏؜ᅟᅠ឴឵᠋-᠏​-‏‪-‮"
                          "⁠-⁯ㅤ︀-️﻿ﾠ\U000e0000-\U000e007f]")
_GLYPHS = {
    # Cyrillic
    "А": "A", "а": "a", "В": "B", "в": "b", "Е": "E", "е": "e", "Ё": "E", "ё": "e", "К": "K", "к": "k", "М": "M",
    "м": "m", "Н": "H", "н": "h", "О": "O", "о": "o", "Р": "P", "р": "p", "С": "C", "с": "c", "Т": "T", "т": "t",
    "У": "Y", "у": "y", "Х": "X", "х": "x", "І": "I", "і": "i", "Ї": "I", "ї": "i", "Ј": "J", "ј": "j", "Ѕ": "S",
    "ѕ": "s", "Ԁ": "D", "ԁ": "d", "Ԛ": "Q", "ԛ": "q", "Ԝ": "W", "ԝ": "w", "Ү": "Y", "ү": "y", "Ь": "b", "ь": "b",
    "Ӏ": "l", "ӏ": "l", "Ɡ": "G", "п": "n", "г": "r", "и": "u", "З": "3", "з": "3",
    # Greek
    "Α": "A", "α": "a", "Β": "B", "β": "b", "Ε": "E", "ε": "e", "Ζ": "Z", "Η": "H", "η": "n", "Ι": "I", "ι": "i",
    "Κ": "K", "κ": "k", "Μ": "M", "Ν": "N", "ν": "v", "Ο": "O", "ο": "o", "Ρ": "P", "ρ": "p", "Τ": "T", "τ": "t",
    "Υ": "Y", "υ": "u", "Χ": "X", "χ": "x", "ω": "w", "ς": "s", "σ": "o",
    # Latin look-alikes NFKD leaves alone
    "ɑ": "a", "ɡ": "g", "ǀ": "l", "ı": "i", "ȷ": "j", "ł": "l", "Ł": "L", "ø": "o", "Ø": "O", "đ": "d", "Đ": "D",
    "ħ": "h", "Ħ": "H", "ŧ": "t", "Ŧ": "T", "ß": "ss", "æ": "ae", "Æ": "AE", "œ": "oe", "Œ": "OE", "þ": "th",
    "Þ": "Th", "ð": "d", "Ð": "D", "ƒ": "f", "ʀ": "r", "ꞵ": "b",
}
_GLYPH_TABLE = str.maketrans(_GLYPHS)
_LEET = {"0": "o", "1": "i", "3": "e", "4": "a", "5": "s", "7": "t", "8": "b", "9": "g", "@": "a", "$": "s", "!": "i",
         "|": "l"}
LEET = str.maketrans(_LEET)
LEET_L = str.maketrans(dict(_LEET, **{"1": "l"}))     # "L3yl1n3": a 1 can stand for an l too
BRAND_RE = re.compile(r"(?i)world\s*of\s*warcraft|warcraft|lich\s*king|blizzard|\bwow\b")
SQUASHED_BRANDS = ("worldofwarcraft", "warcraft", "lichking", "blizzard")
SUFFIX = r"(?:s|es|z|ed|er|ers)?"
ALLOW_SUFFIX = r"(?:s|es|d|ed|r|er|ers|y|ing|ings)?"     # "spices", "cockers": an allowed word keeps its endings


# ------------------------------------------------------------------------------------------- normalising
def strip_invisible(s) -> str:
    """The text without zero-width characters, joiners and right-to-left overrides (they hide or reorder letters)."""
    return INVISIBLE_RE.sub("", s) if isinstance(s, str) else ""


def fold(s, keep_case: bool = False) -> str:
    """Invisible characters out, full-width and ligatures to plain (NFKC), look-alike letters from other alphabets to
    Latin, accents off; lower case unless keep_case. Digits and signs stay (leet() turns them into letters)."""
    s = unicodedata.normalize("NFKC", strip_invisible(s)).translate(_GLYPH_TABLE)
    s = "".join(c for c in unicodedata.normalize("NFKD", s) if not unicodedata.combining(c))
    s = s.translate(_GLYPH_TABLE)
    return s if keep_case else s.lower()


def leet(s: str) -> str:
    return s.translate(LEET)


def skeleton(s) -> str:
    """What a name looks like, for telling look-alikes apart: folded, digits as letters, letters only, no doubles
    ("0rion", "Οrion", "Orrion" and "O.rion" all give "orion")."""
    t = re.sub(r"[^a-z]", "", leet(fold(s)))
    return re.sub(r"(.)\1+", r"\1", t)


def _pat(word: str) -> str:
    """A word as a pattern where each letter may repeat ("shit" -> s+h+i+t+; "nigger" -> n+i+g{2,}e+r+)."""
    out = []
    for c, g in itertools.groupby(word):
        n = len(list(g))
        out.append(re.escape(c) + ("+" if n == 1 else "{%d,}" % n))
    return "".join(out)


def _camel(s: str) -> str:
    s = re.sub(r"([a-z])([A-Z])", r"\1 \2", s)
    return re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1 \2", s)


def _join_singles(tokens: list[str]) -> list[str]:
    """Runs of one-letter words joined ("f u c k" -> "fuck")."""
    out, run = [], []
    for t in tokens + [""]:
        if len(t) == 1:
            run.append(t)
            continue
        if len(run) >= 2:
            out.append("".join(run))
        run = []
    return out


# ------------------------------------------------------------------------------------------- the lists
class Lists:
    """The compiled lists: {kind: [(word, mode, regex)]}, the allow words, and the staff edge/word words."""

    def __init__(self, doc: dict, extra: list[dict] | None = None):
        self.entries: list[tuple[str, str, str, re.Pattern, re.Pattern | None]] = []   # kind, word, mode, full, inside
        for kind in ("slur", "sexual", "profanity"):
            sec = doc.get(kind) if isinstance(doc.get(kind), dict) else {}
            for mode in MODES:
                for w in sec.get(mode) or []:
                    self._add(kind, w, mode)
        st = doc.get("staff") if isinstance(doc.get("staff"), dict) else {}
        self.staff_edge = [(w, re.compile(_pat(w))) for w in st.get("edge") or [] if WORD_RE.fullmatch(str(w))]
        self.staff_word = [(w, re.compile(_pat(w) + r"s?")) for w in st.get("word") or [] if WORD_RE.fullmatch(str(w))]
        for x in extra or []:
            self._add("custom", x.get("w"), x.get("mode") if x.get("mode") in MODES else "word")
        allow = [a for a in doc.get("allow") or [] if WORD_RE.fullmatch(str(a))]
        self.allow = frozenset(allow)
        self.allow_full = (re.compile("(?:" + "|".join(_pat(a) for a in allow) + ")" + ALLOW_SUFFIX) if allow else None)
        self.allow_re = re.compile("|".join(_pat(a) for a in sorted(allow, key=len, reverse=True))) if allow else None

    def _add(self, kind, w, mode):
        if not isinstance(w, str) or not WORD_RE.fullmatch(w):
            return
        p = _pat(w)
        self.entries.append((kind, w, mode, re.compile(p + SUFFIX), re.compile(p) if mode == "inside" else None))

    def unallowed(self, t: str) -> str:
        return self.allow_re.sub("-", t) if self.allow_re is not None else t


_lock = threading.RLock()
_cache: dict[str, tuple] = {}


def data_dir() -> str:
    return os.environ.get("WU_PORTAL_DATA", "/data")


def _read_json(path: str, default):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def _mtime(path: str) -> float:
    try:
        return os.stat(path).st_mtime_ns
    except OSError:
        return 0


def extra_path() -> str:
    return os.path.join(data_dir(), EXTRA_NAME)


def extra_doc() -> dict:
    d = _read_json(extra_path(), {})
    d = d if isinstance(d, dict) else {}
    words = [x for x in d.get("words") or [] if isinstance(x, dict) and WORD_RE.fullmatch(str(x.get("w") or ""))]
    prot = [x for x in d.get("protected") or [] if isinstance(x, dict) and skeleton(x.get("name") or "")]
    return {"format": 1, "updated_at": d.get("updated_at") or "", "words": words, "protected": prot}


def lists() -> Lists:
    """The built-in list plus the owner's words, compiled once per change of either file."""
    key = (BUILTIN_FILE, _mtime(BUILTIN_FILE), extra_path(), _mtime(extra_path()))
    with _lock:
        hit = _cache.get("lists")
        if hit and hit[0] == key:
            return hit[1]
        doc = _read_json(BUILTIN_FILE, {})
        extra = extra_doc()["words"] if key[3] else []
        L = Lists(doc if isinstance(doc, dict) else {}, extra)
        _cache["lists"] = (key, L)
        return L


# ------------------------------------------------------------------------------------------- matching
def _token_hit(L: Lists, t: str, kinds) -> tuple[str, str] | None:
    if not t or t in L.allow or (L.allow_full is not None and L.allow_full.fullmatch(t)):
        return None
    inner = None
    for kind, w, mode, full, inside in L.entries:
        if kind not in kinds:
            continue
        if full.fullmatch(t):
            return kind, w
        if inside is not None:
            if inner is None:
                inner = L.unallowed(t)
            if inside.search(inner):
                return kind, w
    return None


def _tokens(text: str) -> tuple[list[str], str]:
    """(the words, every letter joined) of a name or a line: folded, digits as letters, split on anything else and at
    run-together capitals; runs of single letters joined."""
    cased = fold(text, keep_case=True)
    plain = leet(cased.lower())
    words = re.findall(r"[a-z]+", plain)
    camel = re.findall(r"[a-z]+", leet(_camel(cased)).lower())
    chunks = [re.sub(r"[^a-z]", "", c) for c in plain.split()]    # "f.u.c.k", "sh-it": a word with separators in it
    toks = words + camel + _join_singles(words) + [c for c in chunks if c]
    if "1" in cased:
        alt = cased.lower().translate(LEET_L)
        toks += re.findall(r"[a-z]+", alt) + [re.sub(r"[^a-z]", "", c) for c in alt.split()]
    return list(dict.fromkeys(t for t in toks if t)), "".join(words)


def _staff_hit(L: Lists, original: str, toks: list[str], joined: str, skip=frozenset()) -> tuple[str, str] | None:
    cased = re.sub(r"[^A-Za-z]", "", fold(original, keep_case=True))
    if re.match(r"GM(?![a-z])", cased) or cased.endswith("GM") or re.match(r"Gm(?=[A-Z])", cased):
        if "gm" not in skip:
            return "staff", "gm"
    for w, rx in L.staff_edge:
        if w in skip:
            continue
        if rx.match(joined) or re.search(rx.pattern + r"$", joined):
            return "staff", w
        for t in toks:
            if rx.match(t) or re.search(rx.pattern + r"$", t):
                return "staff", w
    for w, rx in L.staff_word:
        if w in skip:
            continue
        if rx.fullmatch(joined) or any(rx.fullmatch(t) for t in toks):
            return "staff", w
    return None


def name_hit(name, kinds=KINDS, staff_skip=frozenset()) -> tuple[str, str] | None:
    """(kind, word) when a name trips the list, else None. kinds: which lists (KINDS: all of them)."""
    if not isinstance(name, str) or not name.strip():
        return None
    L = lists()
    toks, joined = _tokens(name)
    if "brand" in kinds:
        if BRAND_RE.search(fold(name)) or "WoW" in name or "wow" in toks or any(b in joined for b in SQUASHED_BRANDS):
            return "brand", "brand"
    for t in toks:
        h = _token_hit(L, t, kinds)
        if h:
            return h
    if joined and joined not in toks:
        h = _token_hit(L, joined, kinds)
        if h:
            return h
    if "staff" in kinds:
        return _staff_hit(L, name, toks, joined, staff_skip)
    return None


def realm_name_hit(name) -> tuple[str, str] | None:
    """A realm list name: everything but the brand (the relay and the kit take brand words out themselves) and the short
    staff words a realm may fairly use (REALM_STAFF_SKIP)."""
    return name_hit(name, ("slur", "sexual", "profanity", "staff", "custom"), REALM_STAFF_SKIP)


def text_hit(text, kinds=TEXT_KINDS) -> tuple[str, str] | None:
    """(kind, word) for the first bad word in a line of text (a title, a post, a chat line), word by word."""
    if not isinstance(text, str) or not text:
        return None
    L = lists()
    toks, _ = _tokens(text)
    for t in toks:
        h = _token_hit(L, t, kinds)
        if h:
            return h
    return None


_RUN_RE = re.compile(r"[^\s]+")


def mask(text, kinds=TEXT_KINDS) -> str:
    """The text with each word that trips the list starred out, the same length (the rest untouched; links too: a
    run with a bad word in it is starred whole)."""
    if not isinstance(text, str) or not text:
        return text if isinstance(text, str) else ""
    L = lists()

    def rep(m):
        run = m.group(0)
        toks, joined = _tokens(run)
        if any(_token_hit(L, t, kinds) for t in toks):
            return "*" * len(run)
        return run
    out = _RUN_RE.sub(rep, text)
    # spaced out over several runs: "f u c k", "n i g g e r"
    for m in list(re.finditer(r"(?:(?<![^\s])[^\s](?:\s+|$)){3,}", out)):
        seg = m.group(0)
        letters = re.sub(r"[^a-z]", "", leet(fold(seg)))
        if letters and _token_hit(L, letters, kinds):
            out = out[:m.start()] + re.sub(r"[^\s]", "*", seg) + out[m.end():]
    return out


def message(kind: str | None) -> str:
    """The refusal a person reads (never the word itself)."""
    if not kind:
        return ""
    if kind == "staff":
        return "That name is kept for the Leyline team. Pick another."
    return "Pick another name."


def shown(text: str) -> str:
    """A refused name for HQ's lists: first and last letter, the rest starred ("N****r")."""
    t = strip_invisible(str(text or ""))[:32]
    if len(t) <= 2:
        return "*" * len(t)
    return t[0] + "*" * (len(t) - 2) + t[-1]


# ------------------------------------------------------------------------------------------- protected names
def protected() -> list[dict]:
    return extra_doc()["protected"]


def protected_clash(name, acct: str = "") -> bool:
    """True when the name looks like a protected staff name (HQ > Blocked words) and acct is not its owner."""
    sk = skeleton(name)
    if len(sk) < 3:
        return False
    for p in protected():
        if skeleton(p.get("name")) == sk and str(p.get("acct") or "").upper() != str(acct or "").upper():
            return True
    return False


# ------------------------------------------------------------------------------------------- the owner's words (HQ)
def _write_extra(doc: dict) -> None:
    p = extra_path()
    os.makedirs(os.path.dirname(p), exist_ok=True)
    tmp = p + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=1, ensure_ascii=False)
    os.chmod(tmp, 0o640)
    os.replace(tmp, p)


def _iso() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


class Refused(ValueError):
    pass


def clean_word(w) -> str:
    """A word the owner adds: letters only after undoing accents and digits ("N1gg3r" -> "nigger"), 2-24 letters."""
    t = re.sub(r"[\s.\-_]", "", leet(fold(w or "")))
    if not WORD_RE.fullmatch(t):
        raise Refused("A word is 2 to 24 letters (digits for letters are undone, other signs are not allowed).")
    return t


def add_word(word, mode: str, by: str) -> tuple[dict, dict]:
    w = clean_word(word)
    if mode not in MODES:
        raise Refused("Pick whole word or inside words.")
    with _lock:
        d = extra_doc()
        if any(x["w"] == w for x in d["words"]):
            raise Refused("That word is on the list already.")
        if len(d["words"]) >= 500:
            raise Refused("The list is full (500 words).")
        d["words"].append({"w": w, "mode": mode, "by": str(by)[:40], "at": _iso()})
        d["updated_at"] = _iso()
        _write_extra(d)
    return {"word": None}, {"word": shown(w), "mode": mode}


def remove_word(word, by: str) -> tuple[dict, dict]:
    w = clean_word(word)
    with _lock:
        d = extra_doc()
        before = [x for x in d["words"] if x["w"] == w]
        if not before:
            raise Refused("That word is not on the owner's list (the built-in words are in stream/blocklist.json).")
        d["words"] = [x for x in d["words"] if x["w"] != w]
        d["updated_at"] = _iso()
        _write_extra(d)
    return {"word": shown(w), "mode": before[0].get("mode")}, {"word": None}


def add_protected(name, acct: str, by: str) -> tuple[dict, dict]:
    n = strip_invisible(str(name or "")).strip()[:24]
    if len(skeleton(n)) < 3:
        raise Refused("A protected name has at least 3 letters.")
    if not re.fullmatch(r"[A-Za-z0-9]{1,32}", str(acct or "")):
        raise Refused("Give the account that owns the name (its id, from their page in Players).")
    with _lock:
        d = extra_doc()
        if any(skeleton(p["name"]) == skeleton(n) for p in d["protected"]):
            raise Refused("That name (or one that looks like it) is protected already.")
        if len(d["protected"]) >= 100:
            raise Refused("The list is full (100 names).")
        d["protected"].append({"name": n, "acct": acct.upper(), "by": str(by)[:40], "at": _iso()})
        d["updated_at"] = _iso()
        _write_extra(d)
    return {"protected": None}, {"protected": n, "acct": acct.upper()}


def remove_protected(name, by: str) -> tuple[dict, dict]:
    sk = skeleton(name)
    with _lock:
        d = extra_doc()
        before = [p for p in d["protected"] if skeleton(p["name"]) == sk]
        if not before:
            raise Refused("That name is not protected.")
        d["protected"] = [p for p in d["protected"] if skeleton(p["name"]) != sk]
        d["updated_at"] = _iso()
        _write_extra(d)
    return {"protected": before[0]["name"]}, {"protected": None}


# ------------------------------------------------------------------------------------------- hits (HQ Needs you)
def record_hit(where: str, text: str, kind: str, acct: str = "") -> None:
    """One refused name, for HQ: where, the name (HQ shows it starred), the list, the account when known. Best effort."""
    rec = {"at": int(time.time()), "where": str(where)[:24], "name": strip_invisible(str(text or ""))[:40],
           "kind": kind if kind in KINDS else "custom", "acct": str(acct or "")[:32]}
    p = os.path.join(data_dir(), HITS_NAME)
    try:
        with _lock:
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "a", encoding="utf-8") as f:
                f.write(json.dumps(rec, ensure_ascii=False) + "\n")
            if os.path.getsize(p) > 400_000:      # keep the newest ~1000
                with open(p, encoding="utf-8") as f:
                    keep = f.readlines()[-1000:]
                with open(p + ".tmp", "w", encoding="utf-8") as f:
                    f.writelines(keep)
                os.replace(p + ".tmp", p)
    except OSError:
        pass


def hits(since: float = 0, limit: int = 200) -> list[dict]:
    """The newest refusals first."""
    p = os.path.join(data_dir(), HITS_NAME)
    out = []
    try:
        with open(p, encoding="utf-8") as f:
            lines = f.readlines()[-2000:]
    except OSError:
        return []
    for ln in reversed(lines):
        try:
            r = json.loads(ln)
        except ValueError:
            continue
        if isinstance(r, dict) and isinstance(r.get("at"), int) and r["at"] >= since:
            out.append(r)
            if len(out) >= limit:
                break
    return out


def clear_hits(by: str) -> tuple[dict, dict]:
    p = os.path.join(data_dir(), HITS_NAME)
    n = len(hits(0, 100000))
    with _lock:
        try:
            os.replace(p, p + ".seen")
        except OSError:
            pass
    return {"hits": n}, {"hits": 0}


# ------------------------------------------------------------------------------------------- the realm's copies
LUA_BEGIN = "-- >>> blocklist (generated: python3 stream/namecheck.py --lua <this file>; do not edit by hand)"
LUA_END = "-- <<< blocklist"


def lua_table(doc: dict | None = None) -> str:
    doc = doc if doc is not None else _read_json(BUILTIN_FILE, {})
    q = lambda ws: ", ".join(f'"{w}"' for w in ws if WORD_RE.fullmatch(str(w)))   # noqa: E731
    lines = [LUA_BEGIN, "local LISTS = {"]
    for kind in ("slur", "sexual", "profanity"):
        sec = doc.get(kind) or {}
        lines.append(f"    {kind} = {{ inside = {{ {q(sec.get('inside') or [])} }},")
        lines.append(f"        word = {{ {q(sec.get('word') or [])} }} }},")
    st = doc.get("staff") or {}
    lines.append(f"    staff = {{ edge = {{ {q(st.get('edge') or [])} }},")
    lines.append(f"        word = {{ {q(st.get('word') or [])} }} }},")
    lines.append(f"    allow = {{ {q(doc.get('allow') or [])} }},")
    lines.append("}")
    lines.append(LUA_END)
    return "\n".join(lines)


def write_lua(path: str) -> bool:
    """Put the table into ov_names.lua between the markers. True when the file changed."""
    with open(path, encoding="utf-8") as f:
        s = f.read()
    i, j = s.find(LUA_BEGIN), s.find(LUA_END)
    if i < 0 or j < 0:
        raise SystemExit(f"{path}: the blocklist markers are missing")
    new = s[:i] + lua_table() + s[j + len(LUA_END):]
    if new != s:
        with open(path, "w", encoding="utf-8") as f:
            f.write(new)
    return new != s


def reserved_names(extra: list[dict] | None = None) -> dict[str, list[str]]:
    """Whole character names for AzerothCore's tables (letters only, 2-12): {"reserved_name": staff words,
    "profanity_name": the rest}. The server compares whole names, lower case; ov_names.lua catches the rest after."""
    doc = _read_json(BUILTIN_FILE, {})
    res, prof = set(), set()
    st = doc.get("staff") or {}
    for w in (st.get("edge") or []) + (st.get("word") or []):
        res.update({w, w + "s"})
    for w in ("gamemaster", "leylineteam", "leylinestaff", "leylinegm", "leylineadmin", "officialgm", "theadmin",
              "theowner", "realmadmin", "servergm", "headgm", "seniorgm", "admingm"):
        res.add(w)
    for kind in ("slur", "sexual", "profanity"):
        for mode in MODES:
            for w in (doc.get(kind) or {}).get(mode) or []:
                prof.update({w, w + "s"})
    for x in extra or []:
        if WORD_RE.fullmatch(str(x.get("w") or "")):
            prof.update({x["w"], x["w"] + "s"})
    ok = lambda s: {w for w in s if re.fullmatch(r"[a-z]{2,12}", w)}   # noqa: E731
    return {"reserved_name": sorted(ok(res)), "profanity_name": sorted(ok(prof) - ok(res))}


def _main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[0] == "--lua":
        changed = write_lua(argv[1])
        print(("updated " if changed else "unchanged ") + argv[1])
        return 0
    if argv and argv[0] == "--words":
        # the realm's copy: "<mode>\t<word>" lines of the owner's words (serverpass.sh fills ov_blocked_words)
        d = _read_json(argv[1], {}) if len(argv) > 1 else {}
        for x in (d.get("words") if isinstance(d, dict) else None) or []:
            if isinstance(x, dict) and WORD_RE.fullmatch(str(x.get("w") or "")):
                print(f"{x.get('mode') if x.get('mode') in MODES else 'word'}\t{x['w']}")
        return 0
    if argv and argv[0] == "--reserved":
        d = _read_json(argv[1], {}) if len(argv) > 1 else {}
        r = reserved_names((d.get("words") if isinstance(d, dict) else None) or [])
        for table, names in r.items():
            for n in names:
                print(f"{table}\t{n}")
        return 0
    if argv and argv[0] == "--check":
        for n in argv[1:]:
            print(f"{n!r}: name {name_hit(n)}, realm {realm_name_hit(n)}, text {text_hit(n)}")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(_main(sys.argv[1:]))
