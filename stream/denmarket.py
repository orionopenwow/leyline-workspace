"""Shared Dens: the builds marketplace for Your Den (docs/DEN-MARKETPLACE.md), for stream/wrath_portal.py.

Players share a saved Den (a whole build) or a stamp (a small ready-made place, up to 60 pieces within 40 yards) from
the launcher's Mods & Addons › Dens tab or Your Den › Saved Dens › Share. Every share waits for a staff member in
Leyline HQ (Needs you): nothing is ever approved by itself, and an update to a live build waits too while the old
version stays up. Approved builds are a catalogue in the launcher and on the website, each with its author's credit,
its remix line ("based on X by Y") and "Made with Leyline".

    GET  /wrath/dens                     the catalogue: featured builds on top, search, Dens | Stamps, tags, sort
    GET  /wrath/dens/<id>                one build's showcase: its pictures, credit, remix line, licence, Open in Leyline
    GET  /wrath/dens/<id>/card.svg       the showcase card (a picture-free card: title, author, size, Made with Leyline)
    GET  /wrath/dens/by/<pid>            one author's builds (their credit links here)
    GET  /wrath/dens/pic/<id>/<n>.jpg    a live build's picture (.png when it was a PNG)
    POST /wrath/dens/<id>                the showcase's plain forms when logged in: step=like|unlike|report (csrf)
    GET  /wrath/dens/review[/<id>]       staff only (stream/admin.py who): what waits, with its pictures (and pic/<n>)

    GET  /wrath/api/dens                 JSON for the launcher: {featured, items, tags, page, pages}
                                         ?q= &kind=den|stamp &tag= &sort=new|likes|downloads &page=
    GET  /wrath/api/dens/<id>            one build (pictures as addresses, lineage, sha256 of its file)
    GET  /wrath/api/dens/<id>/file       its .den.json with the credit put in (counted once a day per player or address)
    GET  /wrath/api/dens/mine            the caller's own builds, waiting ones too (and a csrf for the site)
    POST /wrath/api/dens/<action>        share | update | unpublish | like | report, as the launcher's Hero Key sign-in
                                         (Authorization: Bearer <token>, stream/social.py) or the site (cookie + X-Leyline-CSRF)

The switch: WU_DENS (default off, until the grand release). "1": everything; "beta": the API only (the launcher's tab,
for testing), the pages still 404. Off: every route 404s, except /wrath/dens/review for staff.

Files: $WU_PORTAL_DATA/dens: dens.db (sqlite), files/<id>/den.json (and next.json for an update that waits),
files/<id>/<n>.jpg|png (next-<n>.* likewise), secret (the forms' csrf key). A Den file is a layout only (object
entries and where each stands; nothing in it runs, and no game file is in it), checked here with the same rules as the
Den loader (stream/listings.py den_problems, launcher host/dens.rs, ov_den.lua), 512 KB at most. Pictures: JPEG or
PNG, up to 4, 900 KB each, from 320x180 to 3840x2160; read for their size only, stored as sent.

Credit: the file a player downloads carries "credit" (this build: id, title, by, line, licence, based_on, made_with,
url). Den Lite keeps it, and when a shared stamp is placed in another Den each piece keeps a "credit" number into that
Den's "credits" list, so the stamp's maker is still named when that Den is shared on (labs/den-lite den.rs).

Standard library only.
"""
from __future__ import annotations

import base64
import hashlib
import hmac
import html
import importlib.util
import json
import logging
import os
import re
import secrets
import sqlite3
import struct
import sys
import threading
import time
import urllib.parse

log = logging.getLogger("wrath-portal.denmarket")
HERE = os.path.dirname(os.path.abspath(__file__))

PAGE = "/wrath/dens"
API = "/wrath/api/dens"
REVIEW = PAGE + "/review"

ID_RE = re.compile(r"^[a-km-z2-9]{10}$")
PID_RE = re.compile(r"^[a-km-z2-9]{10}$")   # stream/social.py's public ids (the same letters)
ACCT_RE = re.compile(r"^[A-Za-z0-9]{1,32}$")
ID_ALPHABET = "abcdefghijkmnpqrstuvwxyz23456789"
CTRL_RE = re.compile(r"[\x00-\x1f\x7f-\x9f​-‏‪-‮⁦-⁩]")
# The game's brand words never show on our pages (owner rules): text a player types is cleaned with the hub's own
# cleaner (stream/hub.py neutral) when it is loaded; this is the fallback and the last check.
BRAND_RE = re.compile(r"(?i)\b(?:world\s*of\s*warcraft|warcraft|wow|lich\s*king|blizzard)\b")

DEN_MAX = 512 * 1024
SUBMIT_MAX = DEN_MAX - 4096          # room for the credit the server puts in
STAMP_MAX = 60                       # labs/den-lite den.rs STAMP_MAX, ov_den_home.lua H.STAMP_MAX
STAMP_SPAN = 40.0                    # yards (STAMP_SPAN)
CREDITS_MAX = 32                     # the stamps' makers one Den names
PIC_MAX = 900 * 1024
PICS = 4
PIC_MIN = (320, 180)
PIC_LIMIT = (3840, 2160)
BODY_MAX = 7 * 1024 * 1024           # a share: the Den's text and four pictures in base64
FORM_MAX = 8192
LINEAGE_DEPTH = 8
PER_PAGE = 24
SHARES_PER_HOUR = 6
DOWNLOAD_DAY = 86400

KINDS = {"den": "Den", "stamp": "Stamp"}
SORTS = {"new": "New", "likes": "Most liked", "downloads": "Most opened"}
# Tags from a fixed list (searchable, and nothing to moderate): the stamps' own scenes, then sizes and moods.
TAGS = {"camp": "Camp", "inn": "Inn", "house": "House", "garden": "Garden", "market": "Market", "dock": "Dock",
        "smithy": "Smithy", "shrine": "Shrine", "library": "Library", "castle": "Castle", "town": "Town",
        "nature": "Nature", "cozy": "Cozy", "spooky": "Spooky", "festive": "Festive", "arena": "Arena",
        "minigame": "Minigame", "parkour": "Parkour", "showcase": "Showcase", "small": "Small", "big": "Big"}
TAGS_MAX = 5
# What others may do with a build. Every choice keeps the credit; "look" only lets people open it, not share a remix.
LICENCES = {"cc-by": ("CC BY 4.0", "Share and remix it; credit me."),
            "cc-by-sa": ("CC BY-SA 4.0", "Share and remix it; credit me and share remixes the same way."),
            "cc-by-nc": ("CC BY-NC 4.0", "Share and remix it, not for money; credit me."),
            "cc0": ("CC0", "No rights kept: do anything with it."),
            "look": ("Look only", "Open it and build in your own copy; don't share remixes of it.")}
LICENCE_DEFAULT = "cc-by"
REPORT_REASONS = {"stolen": "Not theirs / stolen work", "rude": "Rude or hateful", "spam": "Spam",
                  "broken": "It doesn't open", "gamedata": "Holds game files", "other": "Something else"}
MADE_WITH = "Made with Leyline"
STATES = ("pending", "live", "rejected", "removed")

e = html.escape


# ------------------------------------------------------------------------------------------------ the portal glue
_get = lambda name: None   # noqa: E731


def bind(getter) -> None:
    global _get

    def get(name):
        try:
            return getter(name)
        except Exception:   # noqa: BLE001
            return None
    _get = get


def mode() -> str:
    """"on", "beta" (the API only) or "off" (the default, until the grand release)."""
    v = os.environ.get("WU_DENS", "").strip().lower()
    return "on" if v in ("1", "on", "yes", "true") else "beta" if v == "beta" else "off"


def data_dir() -> str:
    return os.environ.get("WU_DENS_DIR") or os.path.join(os.environ.get("WU_PORTAL_DATA", "/data"), "dens")


def public_host() -> str:
    h = _get("PUBLIC_HOST")
    return h if isinstance(h, str) and h else "leylinelauncher.com"


# ------------------------------------------------------------------------------------------------ text
def line(v, n: int) -> str:
    """One line of plain text: control characters out, spaces collapsed, the brand words out, cut to n."""
    if not isinstance(v, str):
        return ""
    s = re.sub(r"\s+", " ", CTRL_RE.sub(" ", v)).strip()
    hub = _get("hub_mod")
    fn = getattr(hub, "neutral", None) if hub is not None else None
    if fn is not None:
        try:
            s = fn(s)
        except Exception:   # noqa: BLE001
            pass
    s = re.sub(r"\s{2,}", " ", BRAND_RE.sub("", s)).strip(" -:,;")
    return s[:n].rstrip()


def para(v, n: int) -> str:
    """A description: up to n characters, paragraphs kept (two at most in a row)."""
    if not isinstance(v, str):
        return ""
    parts = [line(p, n) for p in re.split(r"\n\s*\n", v.replace("\r", "")[: n * 2])]
    return "\n\n".join(p for p in parts if p)[:n].rstrip()


def new_id() -> str:
    return "".join(secrets.choice(ID_ALPHABET) for _ in range(10))


def _iso(t: float) -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(t))


# ------------------------------------------------------------------------------------------------ the Den file
def _listings():
    m = _get("listings") or sys.modules.get("wu_listings")
    if m is None:
        try:
            spec = importlib.util.spec_from_file_location("wu_listings", os.path.join(HERE, "listings.py"))
            m = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(m)
            sys.modules["wu_listings"] = m
        except Exception:   # noqa: BLE001
            return None
    return m


def _credit_ok(c) -> bool:
    if not isinstance(c, dict) or len(c) > 12:
        return False
    for k, v in c.items():
        if not isinstance(k, str) or not re.fullmatch(r"[a-z_]{1,16}", k):
            return False
        if k == "based_on":
            if not (isinstance(v, list) and len(v) <= LINEAGE_DEPTH and all(_credit_ok(x) for x in v)):
                return False
        elif isinstance(v, bool) or v is None or (isinstance(v, (int, float)) and abs(v) < 1e12):
            continue
        elif not (isinstance(v, str) and len(v) <= 200 and not CTRL_RE.search(v)):
            return False
    return True


def den_check(text: str, kind: str) -> tuple[list[str], dict]:
    """(problems, facts) for a shared Den's text: the Den loader's rules (stream/listings.py den_problems) for the
    layout, the credit fields checked here; facts: props, bytes, name, stampable, room."""
    raw = text.encode("utf-8") if isinstance(text, str) else b""
    if not raw:
        return ["There's no Den file in that."], {}
    if len(raw) > SUBMIT_MAX:
        return ["That Den file is too big (508 KB at most)."], {}
    try:
        doc = json.loads(text.lstrip("﻿"))
    except ValueError:
        return ["That isn't a Den file (it can't be read)."], {}
    if not isinstance(doc, dict):
        return ["That isn't a Den file."], {}
    problems = []
    credit, credits = doc.pop("credit", None), doc.pop("credits", None)
    if credit is not None and not _credit_ok(credit):
        problems.append("its credit isn't right")
    if credits is not None and not (isinstance(credits, list) and len(credits) <= CREDITS_MAX and all(_credit_ok(c) for c in credits)):
        problems.append(f"its credits list isn't right (up to {CREDITS_MAX})")
    if credits is not None and isinstance(credits, list):
        for p in doc.get("props") or []:
            c = p.get("credit") if isinstance(p, dict) else None
            if c is not None and not (isinstance(c, int) and not isinstance(c, bool) and 0 <= c < len(credits)):
                problems.append("a piece's credit isn't in the credits list")
                break
    L = _listings()
    if L is None:
        return ["The Den checks aren't loaded here."], {}
    more, props, name = L.den_problems(json.dumps(doc))
    problems += more
    plist = doc.get("props") if isinstance(doc.get("props"), list) else []
    if not problems and not plist:
        problems.append("it has no props: build something first")

    def near(p):
        return all(isinstance(p.get(k, 0), (int, float)) and abs(p.get(k, 0)) <= STAMP_SPAN for k in ("x", "y", "z"))
    stampable = 0 < len(plist) <= STAMP_MAX and all(isinstance(p, dict) and near(p) for p in plist)
    if kind == "stamp" and not problems and not stampable:
        problems.append(f"a stamp is up to {STAMP_MAX} pieces within {int(STAMP_SPAN)} yards of its middle")
    room = doc.get("room") if isinstance(doc.get("room"), dict) else {}
    return [p[:1].upper() + p[1:] for p in problems], {"props": props, "bytes": len(raw), "name": name, "stampable": stampable,
                                                     "room": str(room.get("look") or "")[:16], "credits": len(credits or [])}


def with_credit(text: str, credit: dict) -> str:
    """The Den file a player downloads: the server's credit for this build in it (whatever the file said)."""
    doc = json.loads(text.lstrip("﻿"))
    doc["credit"] = credit
    return json.dumps(doc, ensure_ascii=False, separators=(",", ":"))


# ------------------------------------------------------------------------------------------------ pictures
def picture(b: bytes) -> tuple[str, int, int] | str:
    """("jpg"|"png", width, height) for a picture we take, else why not."""
    if not b:
        return "That picture is empty."
    if len(b) > PIC_MAX:
        return "A picture is 900 KB at most."
    w = h = 0
    ext = ""
    if b[:8] == b"\x89PNG\r\n\x1a\n" and len(b) >= 24 and b[12:16] == b"IHDR":
        ext, (w, h) = "png", struct.unpack(">II", b[16:24])
    elif b[:3] == b"\xff\xd8\xff":
        ext = "jpg"
        i = 2
        while i + 9 < len(b):
            if b[i] != 0xFF:
                i += 1
                continue
            m = b[i + 1]
            if m in (0xD8, 0x01) or 0xD0 <= m <= 0xD7:
                i += 2
                continue
            seg = struct.unpack(">H", b[i + 2:i + 4])[0]
            if m in (0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF):
                h, w = struct.unpack(">HH", b[i + 5:i + 9])
                break
            i += 2 + seg
    else:
        return "Pictures are JPEG or PNG."
    if not (PIC_MIN[0] <= w <= PIC_LIMIT[0] and PIC_MIN[1] <= h <= PIC_LIMIT[1]):
        return f"A picture is from {PIC_MIN[0]}x{PIC_MIN[1]} to {PIC_LIMIT[0]}x{PIC_LIMIT[1]}."
    return ext, w, h


def pictures_from(doc) -> list[tuple[str, bytes]] | str:
    pics = doc.get("pictures")
    if pics is None:
        return []
    if not isinstance(pics, list) or len(pics) > PICS:
        return f"Up to {PICS} pictures."
    out = []
    for p in pics:
        if not isinstance(p, str) or len(p) > PIC_MAX * 4 // 3 + 8:
            return "A picture is 900 KB at most."
        try:
            b = base64.b64decode(p, validate=True)
        except ValueError:
            return "That picture didn't arrive whole."
        r = picture(b)
        if isinstance(r, str):
            return r
        out.append((r[0], b))
    return out


# ------------------------------------------------------------------------------------------------ the store
SCHEMA = """
CREATE TABLE IF NOT EXISTS builds (
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, acct TEXT NOT NULL, pid TEXT NOT NULL DEFAULT '', author TEXT NOT NULL,
  credit TEXT NOT NULL DEFAULT '', title TEXT NOT NULL, descr TEXT NOT NULL DEFAULT '', tags TEXT NOT NULL DEFAULT '[]',
  licence TEXT NOT NULL, based_on TEXT NOT NULL DEFAULT '', based_title TEXT NOT NULL DEFAULT '',
  based_by TEXT NOT NULL DEFAULT '', props INTEGER NOT NULL, bytes INTEGER NOT NULL, sha256 TEXT NOT NULL,
  pics TEXT NOT NULL DEFAULT '[]', stampable INTEGER NOT NULL DEFAULT 0, state TEXT NOT NULL, reason TEXT NOT NULL DEFAULT '',
  featured INTEGER NOT NULL DEFAULT 0, likes INTEGER NOT NULL DEFAULT 0, downloads INTEGER NOT NULL DEFAULT 0,
  created REAL NOT NULL, updated REAL NOT NULL, rev INTEGER NOT NULL DEFAULT 1, next TEXT NOT NULL DEFAULT '',
  reviewed_by TEXT NOT NULL DEFAULT '');
CREATE INDEX IF NOT EXISTS builds_state ON builds (state, created);
CREATE TABLE IF NOT EXISTS likes (id TEXT NOT NULL, acct TEXT NOT NULL, at REAL NOT NULL, PRIMARY KEY (id, acct));
CREATE TABLE IF NOT EXISTS downloads (id TEXT NOT NULL, who TEXT NOT NULL, day INTEGER NOT NULL, PRIMARY KEY (id, who, day));
CREATE TABLE IF NOT EXISTS reports (n INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL, acct TEXT NOT NULL,
  reason TEXT NOT NULL, note TEXT NOT NULL DEFAULT '', at REAL NOT NULL, state TEXT NOT NULL DEFAULT 'open');
CREATE TABLE IF NOT EXISTS shares (acct TEXT NOT NULL, at REAL NOT NULL);
"""
COLS = ("id", "kind", "acct", "pid", "author", "credit", "title", "descr", "tags", "licence", "based_on", "based_title",
        "based_by", "props", "bytes", "sha256", "pics", "stampable", "state", "reason", "featured", "likes", "downloads",
        "created", "updated", "rev", "next", "reviewed_by")


class Refused(Exception):
    """A request we answer with its reason (400 unless status says otherwise)."""

    def __init__(self, why: str, status: int = 400):
        super().__init__(why)
        self.status = status


class Store:
    def __init__(self, root: str):
        self.root = root
        os.makedirs(os.path.join(root, "files"), exist_ok=True)
        self.lock = threading.RLock()
        self.db = sqlite3.connect(os.path.join(root, "dens.db"), check_same_thread=False)
        self.db.executescript(SCHEMA)
        sp = os.path.join(root, "secret")
        if not os.path.exists(sp):
            with open(sp, "w") as f:
                f.write(secrets.token_hex(32))
        with open(sp) as f:
            self.secret = f.read().strip().encode()

    # -------------------------------------------------- rows
    def _row(self, r) -> dict | None:
        if r is None:
            return None
        d = dict(zip(COLS, r))
        d["tags"] = json.loads(d["tags"] or "[]")
        d["pics"] = json.loads(d["pics"] or "[]")
        d["next"] = json.loads(d["next"]) if d["next"] else None
        d["stampable"] = bool(d["stampable"])
        d["featured"] = bool(d["featured"])
        return d

    def one(self, bid: str) -> dict | None:
        if not isinstance(bid, str) or not ID_RE.fullmatch(bid):
            return None
        with self.lock:
            return self._row(self.db.execute(f"SELECT {','.join(COLS)} FROM builds WHERE id = ?", (bid,)).fetchone())

    def rows(self, where: str = "1", args: tuple = (), order: str = "created DESC", limit: int = 500) -> list[dict]:
        with self.lock:
            q = f"SELECT {','.join(COLS)} FROM builds WHERE {where} ORDER BY {order} LIMIT ?"
            return [self._row(r) for r in self.db.execute(q, args + (limit,)).fetchall()]

    def set(self, bid: str, **kv) -> None:
        for k in ("tags", "pics"):
            if k in kv:
                kv[k] = json.dumps(kv[k])
        if "next" in kv:
            kv["next"] = json.dumps(kv["next"]) if kv["next"] else ""
        with self.lock:
            self.db.execute(f"UPDATE builds SET {', '.join(k + ' = ?' for k in kv)} WHERE id = ?", tuple(kv.values()) + (bid,))
            self.db.commit()

    # -------------------------------------------------- files
    def folder(self, bid: str) -> str:
        return os.path.join(self.root, "files", bid)

    def write_text(self, bid: str, text: str, which: str = "den") -> None:
        """den.json (or next.json, an update that waits)."""
        d = self.folder(bid)
        os.makedirs(d, exist_ok=True)
        tmp = os.path.join(d, ".tmp")
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(text)
        os.replace(tmp, os.path.join(d, which + ".json"))

    def write_pics(self, bid: str, pics: list[tuple[str, bytes]], prefix: str = "") -> list[str]:
        """The pictures (n.ext, or next-n.ext for an update), the old ones of that set gone. Returns their names."""
        d = self.folder(bid)
        os.makedirs(d, exist_ok=True)
        for f in os.listdir(d):
            if re.fullmatch((re.escape(prefix) + "-" if prefix else "") + r"\d\.(jpg|png)", f):
                os.remove(os.path.join(d, f))
        names = []
        for i, (ext, b) in enumerate(pics):
            n = f"{prefix + '-' if prefix else ''}{i}.{ext}"
            with open(os.path.join(d, n), "wb") as f:
                f.write(b)
            names.append(n)
        return names

    def text(self, bid: str, which: str = "den") -> str | None:
        p = os.path.join(self.folder(bid), which + ".json")
        try:
            if os.path.getsize(p) > DEN_MAX:
                return None
            with open(p, encoding="utf-8") as f:
                return f.read()
        except OSError:
            return None

    def pic_path(self, bid: str, name: str) -> str | None:
        if not re.fullmatch(r"(?:next-)?\d\.(?:jpg|png)", name or ""):
            return None
        p = os.path.join(self.folder(bid), name)
        return p if os.path.isfile(p) else None

    def drop_files(self, bid: str, keep_pics: bool = False) -> None:
        d = self.folder(bid)
        if not os.path.isdir(d):
            return
        for f in os.listdir(d):
            if keep_pics and re.fullmatch(r"\d\.(jpg|png)", f):
                continue
            try:
                os.remove(os.path.join(d, f))
            except OSError:
                pass

    # -------------------------------------------------- what the API and pages read
    def lineage(self, b: dict) -> list[dict]:
        """The remix line, nearest first: [{id, title, by, live}] (a removed build keeps its title and maker)."""
        out, seen, cur = [], {b["id"]}, b
        while cur.get("based_on") and len(out) < LINEAGE_DEPTH:
            up = self.one(cur["based_on"])
            if up is None or up["id"] in seen:
                out.append({"id": cur["based_on"], "title": cur["based_title"], "by": cur["based_by"], "live": False})
                break
            seen.add(up["id"])
            out.append({"id": up["id"], "title": up["title"], "by": up["author"], "live": up["state"] == "live"})
            cur = up
        return out

    def remixes(self, bid: str) -> int:
        with self.lock:
            return self.db.execute("SELECT COUNT(*) FROM builds WHERE based_on = ? AND state = 'live'", (bid,)).fetchone()[0]

    def liked(self, bid: str, acct: str) -> bool:
        with self.lock:
            return self.db.execute("SELECT 1 FROM likes WHERE id = ? AND acct = ?", (bid, acct)).fetchone() is not None

    def like(self, bid: str, acct: str, on: bool) -> int:
        with self.lock:
            if on:
                self.db.execute("INSERT OR IGNORE INTO likes (id, acct, at) VALUES (?,?,?)", (bid, acct, time.time()))
            else:
                self.db.execute("DELETE FROM likes WHERE id = ? AND acct = ?", (bid, acct))
            n = self.db.execute("SELECT COUNT(*) FROM likes WHERE id = ?", (bid,)).fetchone()[0]
            self.db.execute("UPDATE builds SET likes = ? WHERE id = ?", (n, bid))
            self.db.commit()
            return n

    def count_download(self, bid: str, who: str) -> int:
        day = int(time.time() // DOWNLOAD_DAY)
        with self.lock:
            cur = self.db.execute("INSERT OR IGNORE INTO downloads (id, who, day) VALUES (?,?,?)", (bid, who, day))
            if cur.rowcount:
                self.db.execute("UPDATE builds SET downloads = downloads + 1 WHERE id = ?", (bid,))
            self.db.execute("DELETE FROM downloads WHERE day < ?", (day - 2,))
            self.db.commit()
            return self.db.execute("SELECT downloads FROM builds WHERE id = ?", (bid,)).fetchone()[0]

    def shares_lately(self, acct: str) -> int:
        with self.lock:
            self.db.execute("DELETE FROM shares WHERE at < ?", (time.time() - 3600,))
            return self.db.execute("SELECT COUNT(*) FROM shares WHERE acct = ?", (acct,)).fetchone()[0]

    def note_share(self, acct: str) -> None:
        with self.lock:
            self.db.execute("INSERT INTO shares (acct, at) VALUES (?,?)", (acct, time.time()))
            self.db.commit()

    def report(self, bid: str, acct: str, reason: str, note: str) -> bool:
        with self.lock:
            if self.db.execute("SELECT 1 FROM reports WHERE id = ? AND acct = ? AND state = 'open'", (bid, acct)).fetchone():
                return False
            self.db.execute("INSERT INTO reports (id, acct, reason, note, at) VALUES (?,?,?,?,?)", (bid, acct, reason, note, time.time()))
            self.db.commit()
            return True

    def open_reports(self) -> list[dict]:
        with self.lock:
            rs = self.db.execute("SELECT n, id, acct, reason, note, at FROM reports WHERE state = 'open' ORDER BY at DESC LIMIT 300").fetchall()
        return [dict(zip(("n", "id", "acct", "reason", "note", "at"), r)) for r in rs]

    def close_reports(self, bid: str, how: str) -> None:
        with self.lock:
            self.db.execute("UPDATE reports SET state = ? WHERE id = ? AND state = 'open'", (how, bid))
            self.db.commit()


store: Store | None = None
_store_lock = threading.Lock()


def setup() -> Store:
    global store
    with _store_lock:
        if store is None:
            store = Store(data_dir())
            adm = _get("admin")
            if adm is not None and isinstance(getattr(adm, "ACTIONS", None), dict):   # Leyline HQ's buttons
                adm.ACTIONS.update(hq_actions(adm))
    return store


# ------------------------------------------------------------------------------------------------ views
def pic_urls(b: dict, base: str = PAGE + "/pic") -> list[str]:
    return [f"{base}/{b['id']}/{n}" for n in b["pics"]]


def credit_of(b: dict) -> dict:
    """The credit the file carries (and the API shows)."""
    st = setup()
    lic = LICENCES.get(b["licence"], LICENCES[LICENCE_DEFAULT])
    return {"id": b["id"], "title": b["title"], "by": b["author"], "line": b["credit"], "licence": lic[0],
            "based_on": [{"id": x["id"], "title": x["title"], "by": x["by"]} for x in st.lineage(b)],
            "made_with": MADE_WITH, "url": f"https://{public_host()}{PAGE}/{b['id']}"}


def card_view(b: dict) -> dict:
    return {"id": b["id"], "kind": b["kind"], "title": b["title"], "author": b["author"], "pid": b["pid"],
            "credit": b["credit"], "tags": b["tags"], "props": b["props"], "bytes": b["bytes"], "likes": b["likes"],
            "downloads": b["downloads"], "featured": b["featured"], "stampable": b["stampable"],
            "picture": (pic_urls(b) or [""])[0], "pictures": len(b["pics"]), "created": _iso(b["created"]),
            "updated": _iso(b["updated"]), "based_on": bool(b["based_on"])}


def detail_view(b: dict, acct: str = "") -> dict:
    st = setup()
    lic = LICENCES.get(b["licence"], LICENCES[LICENCE_DEFAULT])
    return dict(card_view(b), description=b["descr"], pictures=pic_urls(b), licence={"id": b["licence"], "name": lic[0], "line": lic[1]},
                lineage=st.lineage(b), remixes=st.remixes(b["id"]), sha256=hashlib.sha256(served(b) or b"").hexdigest(), made_with=MADE_WITH,
                page=f"https://{public_host()}{PAGE}/{b['id']}", liked=bool(acct) and st.liked(b["id"], acct),
                mine=bool(acct) and acct == b["acct"], remix_ok=b["licence"] != "look")


def mine_view(b: dict) -> dict:
    return dict(card_view(b), state=b["state"], reason=b["reason"], update_waiting=bool(b["next"]), rev=b["rev"],
                description=b["descr"], licence=b["licence"])


def search(q: str = "", kind: str = "", tag: str = "", sort: str = "new", page: int = 1, by: str = "") -> dict:
    st = setup()
    where, args = ["state = 'live'"], []
    if kind in KINDS:
        where.append("kind = ?")
        args.append(kind)
    if by and PID_RE.fullmatch(by):
        where.append("pid = ?")
        args.append(by)
    rows = st.rows(" AND ".join(where), tuple(args), {"likes": "likes DESC, created DESC",
                                                      "downloads": "downloads DESC, created DESC"}.get(sort, "created DESC"), 5000)
    if tag in TAGS:
        rows = [b for b in rows if tag in b["tags"]]
    words = [w for w in re.split(r"\s+", line(q, 80).lower()) if w]
    if words:
        def hay(b):
            return " ".join([b["title"], b["author"], b["credit"], b["descr"], " ".join(TAGS.get(t, t) for t in b["tags"])]).lower()
        rows = [b for b in rows if all(w in hay(b) for w in words)]
    pages = max(1, -(-len(rows) // PER_PAGE))
    page = min(max(1, page), pages)
    featured = [] if (q or tag or by or page > 1) else [b for b in st.rows("state = 'live' AND featured = 1", (), "updated DESC", 6)
                                                       if kind not in KINDS or b["kind"] == kind]
    used = sorted({t for b in st.rows("state = 'live'", (), "created DESC", 5000) for t in b["tags"]}, key=list(TAGS).index)
    return {"featured": featured, "items": rows[(page - 1) * PER_PAGE: page * PER_PAGE], "tags": used, "page": page,
            "pages": pages, "total": len(rows)}


# ------------------------------------------------------------------------------------------------ the actions
def _fields(doc: dict, b: dict | None) -> dict:
    """The words of a share or an update, checked (an update keeps what it doesn't send)."""
    out = {}
    if "title" in doc or b is None:
        t = line(doc.get("title"), 48)
        if len(t) < 3:
            raise Refused("Give it a title (3 to 48 letters).")
        out["title"] = t
    if "description" in doc or b is None:
        out["descr"] = para(doc.get("description"), 600)
    if "credit" in doc or b is None:
        out["credit"] = line(doc.get("credit"), 80)
    if "tags" in doc or b is None:
        tags = doc.get("tags") or []
        if isinstance(tags, str):
            tags = [t for t in re.split(r"[,\s]+", tags) if t]
        if not isinstance(tags, list) or len(tags) > TAGS_MAX or not all(isinstance(t, str) and t in TAGS for t in tags):
            raise Refused(f"Pick up to {TAGS_MAX} tags from the list.")
        out["tags"] = list(dict.fromkeys(tags))
    if "licence" in doc or b is None:
        lic = doc.get("licence") or LICENCE_DEFAULT
        if lic not in LICENCES:
            raise Refused("Pick a licence from the list.")
        out["licence"] = lic
    return out


def _fingerprint(st: Store, bid: str) -> None:
    """The sha256 of the file as it was shared (the record HQ sees); players check served()'s own (detail_view)."""
    text = st.text(bid)
    if text is not None:
        st.set(bid, sha256=hashlib.sha256(text.encode("utf-8")).hexdigest())


def served(b: dict) -> bytes | None:
    """The file a player downloads: the Den with this build's credit in it, as it is now."""
    text = setup().text(b["id"])
    return with_credit(text, credit_of(b)).encode("utf-8") if text is not None else None


def share(acct: str, author: str, pid: str, doc: dict) -> dict:
    st = setup()
    if st.shares_lately(acct) >= SHARES_PER_HOUR:
        raise Refused("That's a lot of shares in an hour: try again later.", 429)
    kind = doc.get("kind") or "den"
    if kind not in KINDS:
        raise Refused("Share a Den or a stamp.")
    f = _fields(doc, None)
    text = doc.get("den")
    problems, facts = den_check(text if isinstance(text, str) else "", kind)
    if problems:
        raise Refused("That Den can't be shared: " + "; ".join(problems) + ".")
    pics = pictures_from(doc)
    if isinstance(pics, str):
        raise Refused(pics)
    based = {"based_on": "", "based_title": "", "based_by": ""}
    # what it is based on: what the player says, else the credit in the file (they opened a shared build and built on)
    up_id = doc.get("based_on") or (json.loads(text.lstrip("﻿")).get("credit") or {}).get("id") or ""
    if up_id:
        up = st.one(str(up_id))
        if up is None or up["state"] != "live":
            if doc.get("based_on"):
                raise Refused("The build it's based on isn't on Leyline (any more).")
        elif up["licence"] == "look" and up["acct"] != acct:
            raise Refused(f"'{up['title']}' is look only: its maker didn't allow remixes to be shared.")
        else:
            based = {"based_on": up["id"], "based_title": up["title"], "based_by": up["author"]}
    bid = new_id()
    while st.one(bid) is not None:
        bid = new_id()
    st.write_text(bid, text)
    names = st.write_pics(bid, pics)
    now = time.time()
    row = dict(id=bid, kind=kind, acct=acct, pid=pid, author=line(author, 40) or "A player", props=facts["props"],
               bytes=facts["bytes"], sha256="", pics=json.dumps(names), stampable=int(facts["stampable"]), state="pending",
               created=now, updated=now, **based, **{k: (json.dumps(v) if k == "tags" else v) for k, v in f.items()})
    with st.lock:
        st.db.execute(f"INSERT INTO builds ({','.join(row)}) VALUES ({','.join('?' * len(row))})", tuple(row.values()))
        st.db.commit()
    st.note_share(acct)
    _fingerprint(st, bid)
    return {"ok": True, "id": bid, "state": "pending", "message": "Shared. A Leyline moderator looks at it soon; it shows for "
            "everyone once they approve it."}


def update(acct: str, doc: dict) -> dict:
    st = setup()
    b = st.one(str(doc.get("id") or ""))
    if b is None or b["acct"] != acct or b["state"] == "removed":
        raise Refused("That isn't one of your shared builds.", 404)
    f = _fields(doc, b)
    text = doc.get("den")
    facts = None
    if text is not None:
        problems, facts = den_check(text if isinstance(text, str) else "", b["kind"])
        if problems:
            raise Refused("That Den can't be shared: " + "; ".join(problems) + ".")
    pics = pictures_from(doc) if "pictures" in doc else None
    if isinstance(pics, str):
        raise Refused(pics)
    if b["state"] in ("pending", "rejected"):
        # not out yet: changed in place, and it waits (again) for a moderator
        kv = dict(f, state="pending", reason="", updated=time.time())
        if text is not None:
            st.write_text(b["id"], text)
            kv.update(props=facts["props"], bytes=facts["bytes"], stampable=int(facts["stampable"]))
        if pics is not None:
            kv["pics"] = st.write_pics(b["id"], pics)
        st.set(b["id"], **kv)
        _fingerprint(st, b["id"])
        return {"ok": True, "id": b["id"], "state": "pending", "message": "Updated. It waits for a moderator."}
    # live: the new version waits; the old one stays up meanwhile
    nxt = dict(b["next"] or {}, **f)
    if text is not None:
        st.write_text(b["id"], text, "next")
        nxt.update(file=True, props=facts["props"], bytes=facts["bytes"], stampable=facts["stampable"])
    if pics is not None:
        nxt["pics"] = st.write_pics(b["id"], pics, "next")
    st.set(b["id"], next=nxt, updated=time.time())
    return {"ok": True, "id": b["id"], "state": "live", "update_waiting": True,
            "message": "Update sent. The version that's out stays up until a moderator approves this one."}


def unpublish(acct: str, doc: dict) -> dict:
    st = setup()
    b = st.one(str(doc.get("id") or ""))
    if b is None or b["acct"] != acct or b["state"] == "removed":
        raise Refused("That isn't one of your shared builds.", 404)
    st.drop_files(b["id"])
    st.set(b["id"], state="removed", featured=0, next=None, pics=[], updated=time.time())
    return {"ok": True, "id": b["id"], "state": "removed", "message": "Taken down. Copies people already opened stay theirs."}


def like(acct: str, doc: dict) -> dict:
    st = setup()
    b = st.one(str(doc.get("id") or ""))
    if b is None or b["state"] != "live":
        raise Refused("No such build.", 404)
    if b["acct"] == acct:
        raise Refused("You can't like your own build.")
    on = doc.get("on", True) not in (False, "0", "false", "off")
    return {"ok": True, "likes": st.like(b["id"], acct, on), "liked": on}


def report(acct: str, doc: dict) -> dict:
    st = setup()
    b = st.one(str(doc.get("id") or ""))
    if b is None or b["state"] != "live":
        raise Refused("No such build.", 404)
    if b["acct"] == acct:
        raise Refused("That's your own build: Unpublish takes it down.")
    reason = doc.get("reason")
    if reason not in REPORT_REASONS:
        raise Refused("Pick a reason.")
    st.report(b["id"], acct, reason, para(doc.get("note"), 300))
    return {"ok": True, "message": "Thanks: a moderator will look at it."}


ACTIONS = {"share": share, "update": update, "unpublish": unpublish, "like": like, "report": report}


def do(action: str, acct: str, name: str, pid: str, doc: dict) -> tuple[int, dict]:
    fn = ACTIONS.get(action)
    if fn is None:
        return 404, {"error": "Unknown call."}
    ban = _sanction(acct)
    if ban and action in ("share", "update"):
        return 403, {"error": ban}
    try:
        return 200, (fn(acct, name, pid, doc) if action == "share" else fn(acct, doc))
    except Refused as r:
        return r.status, {"error": str(r)}
    except OSError:
        log.exception("dens %s failed", action)
        return 503, {"error": "Couldn't save that right now. Try again in a minute."}


def _sanction(acct: str) -> str:
    adm = _get("admin")
    fn = getattr(adm, "blocked", None) if adm is not None else None
    try:
        return fn("submit", acct=acct) if fn else ""
    except Exception:   # noqa: BLE001
        return ""


# ------------------------------------------------------------------------------------------------ Leyline HQ
def hq_actions(A) -> dict:
    """Staff buttons (stream/admin.py ACTIONS, audited there): approve, reject, feature, take down, the update."""
    def known(target):
        b = setup().one(target)
        if b is None:
            raise A.Refused("No shared build by that id.")
        return b

    def approve(user, target, form):
        b = known(target)
        if b["state"] != "pending":
            raise A.Refused("It isn't waiting.")
        setup().set(b["id"], state="live", reason="", reviewed_by=str(user.get("account", ""))[:40])
        return {"state": "pending"}, {"state": "live"}

    def reject(user, target, form):
        b = known(target)
        why = " ".join(str(form.get("reason") or "").split())[:200]
        if not why:
            raise A.Refused("Say why: the maker reads it.")
        setup().set(b["id"], state="rejected", reason=why, featured=0, reviewed_by=str(user.get("account", ""))[:40])
        setup().close_reports(b["id"], "done")
        return {"state": b["state"]}, {"state": "rejected", "reason": why}

    def feature(on):
        def fn(user, target, form):
            b = known(target)
            if on and b["state"] != "live":
                raise A.Refused("Only a live build can be featured.")
            setup().set(b["id"], featured=int(on), updated=time.time())
            return {"featured": b["featured"]}, {"featured": on}
        return fn

    def up_approve(user, target, form):
        b = known(target)
        nxt = b["next"]
        if not nxt:
            raise A.Refused("No update is waiting.")
        st = setup()
        d = st.folder(b["id"])
        kv = {k: nxt[k] for k in ("title", "descr", "credit", "tags", "licence") if k in nxt}
        if nxt.get("file") and os.path.exists(os.path.join(d, "next.json")):
            os.replace(os.path.join(d, "next.json"), os.path.join(d, "den.json"))
            kv.update(props=nxt["props"], bytes=nxt["bytes"], stampable=int(nxt["stampable"]))
        if "pics" in nxt:
            for f in os.listdir(d):
                if re.fullmatch(r"\d\.(jpg|png)", f):
                    os.remove(os.path.join(d, f))
            names = []
            for n in nxt["pics"]:
                os.replace(os.path.join(d, n), os.path.join(d, n[len("next-"):]))
                names.append(n[len("next-"):])
            kv["pics"] = names
        try:
            os.remove(os.path.join(d, "next.json"))
        except OSError:
            pass
        st.set(b["id"], next=None, rev=b["rev"] + 1, updated=time.time(), reviewed_by=str(user.get("account", ""))[:40], **kv)
        _fingerprint(st, b["id"])
        return {"rev": b["rev"]}, {"rev": b["rev"] + 1}

    def up_reject(user, target, form):
        b = known(target)
        d = setup().folder(b["id"])
        for f in os.listdir(d) if os.path.isdir(d) else []:
            if f.startswith("next"):
                os.remove(os.path.join(d, f))
        setup().set(b["id"], next=None)
        return {}, {"update": "kept the old one"}

    def take_down(user, target, form):
        b = known(target)
        setup().set(b["id"], state="rejected", reason=" ".join(str(form.get("reason") or "taken down by a moderator").split())[:200],
                    featured=0)
        setup().close_reports(b["id"], "done")
        return {"state": b["state"]}, {"state": "rejected"}

    def dismiss(user, target, form):
        known(target)
        setup().close_reports(target, "dismissed")
        return {}, {"reports": "dismissed"}

    return {"den.approve": (approve, "mod", None), "den.reject": (reject, "mod", None),
            "den.feature": (feature(True), "mod", None), "den.unfeature": (feature(False), "mod", None),
            "den.update.approve": (up_approve, "mod", None), "den.update.reject": (up_reject, "mod", None),
            "den.takedown": (take_down, "mod", True), "den.reports.dismiss": (dismiss, "mod", None)}


def hq_items(A, item) -> list[dict]:
    """Needs you (stream/hq.py inbox_items): shares waiting, updates waiting, reports."""
    st = setup()
    out = []
    reason_box = {"reason": ("text", "why (required; the maker reads it)")}
    for b in st.rows("state = 'pending'", (), "created ASC", 200):
        out.append(item("den", "den:" + b["id"], f"Shared {KINDS[b['kind']].lower()}: waiting", b["title"], b["descr"][:300],
                        b["created"], f"{REVIEW}/{b['id']}",
                        [("den.approve", b["id"], "Approve", {}, None, False), ("den.reject", b["id"], "Reject", reason_box, None, True)],
                        sub=f"by {b['author']} · {b['props']} props · {len(b['pics'])} pictures"
                            + (f" · based on {b['based_title']} by {b['based_by']}" if b["based_on"] else "")))
    for b in st.rows("state = 'live' AND next != ''", (), "updated ASC", 200):
        out.append(item("den", "den:" + b["id"] + ":next", "Shared build: update waiting", b["title"],
                        str((b["next"] or {}).get("descr", ""))[:300], b["updated"], f"{REVIEW}/{b['id']}",
                        [("den.update.approve", b["id"], "Approve the update", {}, None, False),
                         ("den.update.reject", b["id"], "Keep the old one", {}, None, False)], sub=f"by {b['author']}"))
    seen = {}
    for r in st.open_reports():
        seen.setdefault(r["id"], []).append(r)
    for bid, rs in seen.items():
        b = st.one(bid)
        if b is None:
            continue
        why = ", ".join(sorted({REPORT_REASONS.get(r["reason"], r["reason"]) for r in rs}))
        out.append(item("den", "den:" + bid + ":reports", "Shared build: reported", b["title"],
                        "; ".join(r["note"] for r in rs if r["note"])[:300], rs[0]["at"], f"{REVIEW}/{bid}",
                        [("den.reports.dismiss", bid, "Dismiss", {}, None, False),
                         ("den.takedown", bid, "Take it down", {"reason": ("text", "why (required)")}, bid, True)],
                        sub=f"{len(rs)} report{'s' if len(rs) != 1 else ''} ({why}) · by {b['author']}", group=0))
    return out


# ------------------------------------------------------------------------------------------------ routes
def match(path: str) -> bool:
    return path == PAGE or path.startswith(PAGE + "/") or path == API or path.startswith(API + "/")


def _viewer(h):
    """(acct, name, how, session, pid) or None. Raises OSError when the site's sign-in isn't answering."""
    soc = _get("social")
    if soc is None or getattr(soc, "store", None) is None:
        return None
    authz = h.headers.get("Authorization", "")
    if authz.startswith("Bearer "):
        acct = soc.store.token_acct(authz[7:].strip())
        if not acct:
            return None
        p = soc.store.profile(acct) or {}
        return acct, p.get("name") or "A player", "token", authz[7:].strip(), p.get("pid") or ""
    who = soc.cookie_user(h)
    if who is None:
        return None
    sess = ""
    for part in h.headers.get("Cookie", "").split(";"):
        k, _, v = part.strip().partition("=")
        if k == "wr_session":
            sess = v
    p = soc.store.profile(who[0]) or {}
    return who[0], who[1], "cookie", sess, p.get("pid") or ""


def _csrf(sess: str) -> str:
    return hmac.new(setup().secret, ("dens|" + sess).encode(), hashlib.sha256).hexdigest()[:32]


def _json(h, status: int, doc, cache: str = "no-store") -> None:
    h.send(status, json.dumps(doc, separators=(",", ":"), ensure_ascii=False).encode("utf-8"),
           "application/json; charset=utf-8", headers={"Cache-Control": cache})


def _limited(h, n: int = 240) -> bool:
    lim = _get("browse_limit")
    rl = _get("rl_bucket")
    try:
        return lim is not None and rl is not None and not lim.hit(rl(h.ip()), n)
    except Exception:   # noqa: BLE001
        return False


def _body(h, cap: int):
    try:
        n = int(h.headers.get("Content-Length", "0"))
    except ValueError:
        return None
    if n < 0 or n > cap:
        return None
    raw = h.rfile.read(n) if n else b""
    if "json" in h.headers.get("Content-Type", ""):
        try:
            d = json.loads(raw.decode("utf-8") or "{}")
            return d if isinstance(d, dict) else None
        except (ValueError, UnicodeDecodeError):
            return None
    try:
        return {k: v[0] for k, v in urllib.parse.parse_qs(raw.decode("utf-8"), max_num_fields=20).items()}
    except (ValueError, UnicodeDecodeError):
        return None


def _arg(query: dict, name: str) -> str:
    v = query.get(name)
    v = v[0] if isinstance(v, list) and v else v
    return v if isinstance(v, str) else ""


def _int(s: str, d: int = 1) -> int:
    try:
        return int(s)
    except ValueError:
        return d


def _staff(h, path: str):
    """The staff member (stream/admin.py who), or None after it answered (log in, 403)."""
    adm = _get("admin")
    if adm is None or not hasattr(adm, "who"):
        h.send(404, b"Not found", "text/plain; charset=utf-8")
        return None
    return adm.who(h, path)


def _send_pic(h, p: str, private: bool = False) -> None:
    with open(p, "rb") as f:
        b = f.read()
    h.send(200, b, "image/png" if p.endswith(".png") else "image/jpeg",
           headers={"Cache-Control": "private, no-store" if private else "public, max-age=86400", "X-Content-Type-Options": "nosniff"})


def _ready(h, is_api: bool) -> bool:
    try:
        setup()
        return True
    except Exception:   # noqa: BLE001  (an unwritable data folder)
        log.exception("shared Dens store")
        _json(h, 503, {"error": "Shared Dens aren't available right now."}) if is_api else h.send(503, b"Try again shortly",
                                                                                                   "text/plain; charset=utf-8")
        return False


def get(h, path: str, query: dict) -> None:
    m = mode()
    if not _ready(h, path.startswith(API)):
        return
    if path.startswith(REVIEW):
        return _review(h, path)
    is_api = path == API or path.startswith(API + "/")
    if m == "off" or (m == "beta" and not is_api):
        return _json(h, 404, {"error": "not_open"}) if is_api else h.send(404, _not_found(), "text/html; charset=utf-8")
    if _limited(h):
        return _json(h, 429, {"error": "Too many requests."}) if is_api else h.send(429, b"Too many requests", "text/plain; charset=utf-8")
    st = setup()
    if is_api:
        try:
            v = _viewer(h)
        except OSError:
            v = None
        rest = path[len(API):].strip("/")
        if rest == "":
            r = search(_arg(query, "q"), _arg(query, "kind"), _arg(query, "tag"), _arg(query, "sort") or "new", _int(_arg(query, "page")),
                       _arg(query, "by"))
            return _json(h, 200, {"ok": True, "featured": [card_view(b) for b in r["featured"]], "items": [card_view(b) for b in r["items"]],
                                  "tags": [{"id": t, "name": TAGS[t]} for t in r["tags"]], "all_tags": [{"id": k, "name": n} for k, n in TAGS.items()],
                                  "licences": [{"id": k, "name": n, "line": l} for k, (n, l) in LICENCES.items()],
                                  "reasons": [{"id": k, "name": n} for k, n in REPORT_REASONS.items()],
                                  "page": r["page"], "pages": r["pages"], "total": r["total"]}, "public, max-age=30")
        if rest == "mine":
            if v is None:
                return _json(h, 401, {"error": "signed_out"})
            doc = {"ok": True, "builds": [mine_view(b) for b in st.rows("acct = ? AND state != 'removed'", (v[0],), "created DESC", 200)]}
            if v[2] == "cookie":
                doc["csrf"] = _csrf(v[3])
            return _json(h, 200, doc)
        parts = rest.split("/")
        b = st.one(parts[0])
        mine = v is not None and b is not None and b["acct"] == v[0]
        if b is None or (b["state"] != "live" and not mine):
            return _json(h, 404, {"error": "No such build."})
        if len(parts) == 1:
            return _json(h, 200, {"ok": True, "build": detail_view(b, v[0] if v else "")})
        if len(parts) == 2 and parts[1] == "file":
            body = served(b)
            if body is None:
                return _json(h, 404, {"error": "That build's file is gone."})
            if b["state"] == "live" and not mine:
                rl = _get("rl_bucket")
                who = ("a:" + v[0]) if v else ("ip:" + hashlib.sha256(str(rl(h.ip()) if rl else h.ip()).encode()).hexdigest()[:16])
                st.count_download(b["id"], who)
            return h.send(200, body, "application/json; charset=utf-8",
                          headers={"Cache-Control": "no-store", "Content-Disposition": f'attachment; filename="{b["id"]}.den.json"'})
        return _json(h, 404, {"error": "Unknown call."})
    # the pages
    rest = path[len(PAGE):].strip("/")
    v = None
    try:
        v = _viewer(h)
    except OSError:
        v = None
    if rest.startswith("pic/"):
        parts = rest.split("/")
        b = st.one(parts[1]) if len(parts) == 3 else None
        p = st.pic_path(b["id"], parts[2]) if b is not None and b["state"] == "live" and parts[2] in b["pics"] else None
        return _send_pic(h, p) if p else h.send(404, b"Not found", "text/plain; charset=utf-8")
    if rest.startswith("by/"):
        pid = rest[3:]
        if not PID_RE.fullmatch(pid):
            return h.send(404, _not_found(), "text/html; charset=utf-8")
        return _send_page(h, path, *catalogue_page(query, by=pid))
    if rest == "":
        return _send_page(h, path, *catalogue_page(query))
    parts = rest.split("/")
    b = st.one(parts[0])
    if b is None or b["state"] != "live":
        return h.send(404, _not_found(), "text/html; charset=utf-8")
    if len(parts) == 2 and parts[1] == "card.svg":
        return h.send(200, card_svg(b).encode("utf-8"), "image/svg+xml; charset=utf-8", headers={"Cache-Control": "public, max-age=3600"})
    if len(parts) != 1:
        return h.send(404, _not_found(), "text/html; charset=utf-8")
    return _send_page(h, path, *showcase_page(b, v, _arg(query, "said")))


def post(h, path: str) -> None:
    m = mode()
    if not _ready(h, path.startswith(API)):
        return
    is_api = path.startswith(API + "/")
    if m == "off" or (m == "beta" and not is_api):
        return _json(h, 404, {"error": "not_open"}) if is_api else h.send(404, _not_found(), "text/html; charset=utf-8")
    if _limited(h, 120):
        return _json(h, 429, {"error": "Too many requests."})
    action = path[len(API):].strip("/") if is_api else ""
    doc = _body(h, BODY_MAX if action in ("share", "update") else FORM_MAX)
    try:
        v = _viewer(h)
    except OSError:
        v = "down"
    if not is_api:   # the showcase's forms: like, unlike, report
        b = setup().one(path[len(PAGE):].strip("/"))
        if b is None or b["state"] != "live" or doc is None:
            return h.send(404, _not_found(), "text/html; charset=utf-8")
        if v in (None, "down") or v[2] != "cookie" or not hmac.compare_digest(str(doc.get("csrf", "")), _csrf(v[3])):
            return h.redirect(f"{PAGE}/{b['id']}?said=again") if hasattr(h, "redirect") else h.send(403, b"Forbidden")
        step = doc.get("step", "")
        st, ans = do({"like": "like", "unlike": "like", "report": "report"}.get(step, ""), v[0], v[1], v[4],
                     dict(doc, id=b["id"], on=step != "unlike"))
        said = "liked" if st == 200 and step == "like" else "reported" if st == 200 and step == "report" else "" if st == 200 else "no"
        return h.redirect(f"{PAGE}/{b['id']}" + (f"?said={said}" if said else "")) if hasattr(h, "redirect") else h.send(303, b"")
    if doc is None:
        return _json(h, 400, {"error": "Send JSON (a share is 7 MB at most)."})
    if v == "down":
        return _json(h, 503, {"error": "The website isn't answering right now."})
    if v is None:
        return _json(h, 401, {"error": "signed_out"})
    if v[2] == "cookie" and (h.headers.get("Sec-Fetch-Site", "") == "cross-site"
                             or not hmac.compare_digest(h.headers.get("X-Leyline-CSRF", ""), _csrf(v[3]))):
        return _json(h, 403, {"error": "Missing the form token."})
    st, ans = do(action, v[0], v[1], v[4], doc)
    _json(h, st, ans)


# ------------------------------------------------------------------------------------------------ the pages
DM_CSS = """
.dm-head{text-align:center;margin:8px 0 18px}.dm-head p{color:var(--ly-dim,#9aa)}
.dm-search{display:flex;gap:8px;flex-wrap:wrap;justify-content:center;margin:0 0 10px}
.dm-search input[type=search]{min-width:240px;flex:1;max-width:420px}
.dm-chips{display:flex;gap:6px;flex-wrap:wrap;justify-content:center;margin:6px 0 14px}
.dm-chips a{padding:4px 10px;border-radius:999px;border:1px solid rgba(255,255,255,.14);text-decoration:none;font-size:13px}
.dm-chips a.on{background:rgba(140,120,255,.22);border-color:rgba(140,120,255,.6)}
.dm-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(230px,1fr));gap:14px}
.dm-card{display:block;border-radius:12px;overflow:hidden;background:rgba(255,255,255,.04);border:1px solid rgba(255,255,255,.08);
  text-decoration:none;color:inherit}
.dm-card:hover{border-color:rgba(140,120,255,.6)}
.dm-pic{aspect-ratio:16/9;background:linear-gradient(135deg,#2a2440,#14202c);display:flex;align-items:center;justify-content:center;
  color:var(--ly-dim,#9aa);font-size:13px}
.dm-pic img{width:100%;height:100%;object-fit:cover;display:block}
.dm-card .t{padding:10px 12px 12px}.dm-card b{display:block}.dm-card .by,.dm-card .nums{font-size:12.5px;color:var(--ly-dim,#9aa)}
.dm-tag{display:inline-block;font-size:11.5px;padding:1px 7px;border-radius:999px;background:rgba(255,255,255,.07);margin:4px 4px 0 0}
.dm-feat{margin:0 0 22px}.dm-feat h2,.dm-all h2{font-size:16px;margin:0 0 10px}
.dm-kind{font-size:11px;text-transform:uppercase;letter-spacing:.06em;color:var(--ly-dim,#9aa)}
.dm-gal{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:10px;margin:12px 0}
.dm-gal a{display:block;border-radius:10px;overflow:hidden}.dm-gal img{width:100%;display:block}
.dm-show{max-width:980px;margin:0 auto}.dm-show dl{display:grid;grid-template-columns:auto 1fr;gap:6px 14px}
.dm-show dt{color:var(--ly-dim,#9aa)}.dm-show dd{margin:0}
.dm-made{font-size:12.5px;color:var(--ly-dim,#9aa);margin-top:16px}
.dm-acts{display:flex;gap:8px;flex-wrap:wrap;margin:12px 0}.dm-acts form{display:inline}
.dm-pages{display:flex;gap:10px;justify-content:center;margin:18px 0}
.dm-said{padding:6px 10px;border-left:3px solid #4ab07a;background:rgba(74,176,122,.08)}
"""


def _frame(title: str, body: str, path: str, desc: str = "") -> bytes:
    cm = _get("card_meta")
    css = _get("leyline_css")
    try:
        meta = cm(path, title) if cm else ""
    except Exception:   # noqa: BLE001
        meta = ""
    try:
        base = css() if css else ""
    except Exception:   # noqa: BLE001
        base = ""
    if desc:
        meta += f'<meta name="description" content="{e(desc)}">'
    return (f'<!doctype html><html lang="en"><head><meta charset="utf-8">'
            f'<meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="dark">'
            f'<title>{e(title)}</title>{meta}<style>{base}{DM_CSS}</style></head>'
            f'<body class="ly"><div class="ly-glow"></div><div class="ly-wrap">{body}</div></body></html>').encode("utf-8")


def _send_page(h, path: str, status: int, body: bytes) -> None:
    chrome = _get("site_chrome")
    try:
        body = chrome(body, path) if chrome else body
    except Exception:   # noqa: BLE001
        pass
    h.send(status, body, "text/html; charset=utf-8")


def _not_found() -> bytes:
    return _frame("Not found", '<section class="ly-panel"><h1>Not here</h1><p>That build isn\'t shared (any more).</p>'
                               f'<p><a href="{PAGE}">All shared Dens</a></p></section>', PAGE)


def _q(**kv) -> str:
    kv = {k: v for k, v in kv.items() if v and not (k == "sort" and v == "new") and not (k == "page" and v == 1)}
    return PAGE + ("?" + urllib.parse.urlencode(kv) if kv else "")


def card_html(b: dict) -> str:
    pic = (f'<img src="{e(pic_urls(b)[0])}" alt="{e(b["title"])}" loading="lazy">' if b["pics"] else e(KINDS[b["kind"]]))
    tags = "".join(f'<span class="dm-tag">{e(TAGS.get(t, t))}</span>' for t in b["tags"][:3])
    return (f'<a class="dm-card" href="{PAGE}/{b["id"]}"><div class="dm-pic">{pic}</div><div class="t">'
            f'<span class="dm-kind">{e(KINDS[b["kind"]])}{" · featured" if b["featured"] else ""}</span><b>{e(b["title"])}</b>'
            f'<span class="by">by {e(b["author"])}</span>'
            f'<span class="nums">{b["props"]} props · {_kb(b["bytes"])} · {b["likes"]} likes · {b["downloads"]} opened</span>'
            f'<div>{tags}</div></div></a>')


def _kb(n: int) -> str:
    return f"{max(1, round(n / 1024))} KB"


def catalogue_page(query: dict, by: str = "") -> tuple[int, bytes]:
    q, kind, tag, sort = _arg(query, "q"), _arg(query, "kind"), _arg(query, "tag"), _arg(query, "sort") or "new"
    kind = kind if kind in KINDS else ""
    tag = tag if tag in TAGS else ""
    sort = sort if sort in SORTS else "new"
    r = search(q, kind, tag, sort, _int(_arg(query, "page")), by)
    who = ""
    if by:
        first = next(iter(r["items"]), None)
        who = first["author"] if first else "this maker"
    title = f"Dens by {who}" if by else "Shared Dens"
    # TODO(owner): the catalogue's line. Draft: "Places players built in their Dens: open one in yours, or place it as a stamp."
    lede = (f"Everything {e(who)} shared." if by else
            "Places players built in their Dens. Open one in your own Den with the Leyline launcher, or place it as a stamp.")
    base = PAGE + (f"/by/{by}" if by else "")

    def link(**kv):
        cur = dict(q=q, kind=kind, tag=tag, sort=sort)
        cur.update(kv)
        u = _q(**cur)
        return base + u[len(PAGE):]
    form = (f'<form class="dm-search" method="get" action="{base}"><input type="search" name="q" value="{e(q)}" '
            f'placeholder="Search shared Dens" aria-label="Search shared Dens" maxlength="80">'
            + (f'<input type="hidden" name="kind" value="{kind}">' if kind else "")
            + (f'<input type="hidden" name="tag" value="{tag}">' if tag else "")
            + f'<select name="sort" aria-label="Sort">' + "".join(f'<option value="{k}"{" selected" if k == sort else ""}>{n}</option>'
                                                                  for k, n in SORTS.items())
            + '</select><button type="submit">Search</button></form>')
    kinds = ('<nav class="dm-chips" aria-label="Kind">' + f'<a href="{link(kind="", page=1)}"{" class=on" if not kind else ""}>All</a>'
             + "".join(f'<a href="{link(kind=k, page=1)}"{" class=on" if k == kind else ""}>{n}s</a>' for k, n in KINDS.items()) + '</nav>')
    tags = ('<nav class="dm-chips" aria-label="Tags">' + "".join(
        f'<a href="{link(tag="" if t == tag else t, page=1)}"{" class=on" if t == tag else ""}>{e(TAGS[t])}</a>' for t in r["tags"]) + '</nav>'
            if r["tags"] else "")
    feat = (f'<section class="dm-feat"><h2>Featured</h2><div class="dm-grid">{"".join(card_html(b) for b in r["featured"])}</div></section>'
            if r["featured"] else "")
    grid = ("".join(card_html(b) for b in r["items"]) if r["items"] else "")
    empty = ('<p class="ly-fine">Nothing shared here yet. Build something in Your Den and press Share in the launcher.</p>'
             if not r["items"] else "")
    pages = ""
    if r["pages"] > 1:
        pages = ('<nav class="dm-pages">' + (f'<a href="{link(page=r["page"] - 1)}">‹ Newer</a>' if r["page"] > 1 else "")
                 + f'<span>Page {r["page"]} of {r["pages"]}</span>'
                 + (f'<a href="{link(page=r["page"] + 1)}">More ›</a>' if r["page"] < r["pages"] else "") + '</nav>')
    found = "All" if not (q or tag) else f"{r['total']} found"
    body = (f'<header class="dm-head"><h1>{e(title)}</h1><p>{lede}</p></header>{form}{kinds}{tags}{feat}'
            f'<section class="dm-all"><h2>{found}</h2><div class="dm-grid">{grid}</div>{empty}</section>'
            f'{pages}<p class="dm-made">{MADE_WITH}. Every build is checked by a Leyline moderator before it shows here. '
            'A Den file is a layout only: where each piece stands. The pieces come from your own game.</p>')
    return 200, _frame(title, body, base, "Dens and stamps players built and shared on Leyline.")


def showcase_page(b: dict, v, said: str = "") -> tuple[int, bytes]:
    st = setup()
    lic = LICENCES.get(b["licence"], LICENCES[LICENCE_DEFAULT])
    lin = st.lineage(b)
    gal = "".join(f'<a href="{e(u)}"><img src="{e(u)}" alt="{e(b["title"])}, picture {i + 1}" loading="lazy"></a>'
                  for i, u in enumerate(pic_urls(b)))
    author = (f'<a href="{PAGE}/by/{e(b["pid"])}">{e(b["author"])}</a>' if b["pid"] else e(b["author"]))
    chain = ""
    if lin:
        parts = [(f'<a href="{PAGE}/{x["id"]}">{e(x["title"])}</a>' if x["live"] else e(x["title"])) + f' by {e(x["by"])}' for x in lin]
        chain = f'<dt>Based on</dt><dd>{" ← ".join(parts)}</dd>'
    remixes = st.remixes(b["id"])
    tags = "".join(f'<a class="dm-tag" href="{_q(tag=t)}">{e(TAGS.get(t, t))}</a>' for t in b["tags"])
    desc = "".join(f"<p>{e(p)}</p>" for p in b["descr"].split("\n\n") if p)
    said_html = {"liked": "Liked.", "reported": "Thanks: a moderator will look at it.", "again": "That form was out of date: try again.",
                 "no": "That didn't go through."}.get(said, "")
    acts = (f'<a class="wk-btn violet" href="leyline://den/{b["id"]}">Open in Leyline</a>'
            f'<a class="wk-btn" href="{API}/{b["id"]}/file">Download the Den file</a>')
    if v is not None and v[2] == "cookie" and v[0] != b["acct"]:
        tok = f'<input type="hidden" name="csrf" value="{_csrf(v[3])}">'
        liked = st.liked(b["id"], v[0])
        acts += (f'<form method="post" action="{PAGE}/{b["id"]}">{tok}<input type="hidden" name="step" value="{"unlike" if liked else "like"}">'
                 f'<button type="submit">{"Liked ✓" if liked else "Like"}</button></form>'
                 f'<details><summary>Report</summary><form method="post" action="{PAGE}/{b["id"]}">{tok}'
                 '<input type="hidden" name="step" value="report"><select name="reason" aria-label="Why">'
                 + "".join(f'<option value="{k}">{e(n)}</option>' for k, n in REPORT_REASONS.items())
                 + '</select> <input name="note" maxlength="300" placeholder="A note (optional)" aria-label="A note"> '
                 '<button type="submit">Send</button></form></details>')
    body = (f'<article class="dm-show"><p><a href="{PAGE}">‹ All shared Dens</a></p>'
            + (f'<p class="dm-said" role="status">{e(said_html)}</p>' if said_html else "")
            + f'<span class="dm-kind">{e(KINDS[b["kind"]])}{" · featured" if b["featured"] else ""}</span><h1>{e(b["title"])}</h1>'
            f'<p>by {author}{(" · " + e(b["credit"])) if b["credit"] else ""}</p>'
            f'<div class="dm-gal">{gal or "<p class=ly-fine>No pictures yet.</p>"}</div>{desc}<div>{tags}</div>'
            f'<div class="dm-acts">{acts}</div>'
            f'<dl><dt>Size</dt><dd>{b["props"]} props · {_kb(b["bytes"])}</dd>'
            f'<dt>Likes</dt><dd>{b["likes"]}</dd><dt>Opened</dt><dd>{b["downloads"]} times</dd>'
            f'<dt>Licence</dt><dd>{e(lic[0])}: {e(lic[1])}</dd>{chain}'
            + (f'<dt>Remixes</dt><dd>{remixes}</dd>' if remixes else "")
            + f'<dt>Shared</dt><dd>{_iso(b["created"])[:10]}</dd></dl>'
            f'<p class="dm-made">{MADE_WITH}. Open in Leyline opens the launcher, which checks the file and puts it in your '
            'Saved Dens. The pieces come from your own game; the file is only where each one stands.</p></article>')
    return 200, _frame(f'{b["title"]} by {b["author"]}', body, f"{PAGE}/{b['id']}", (b["descr"] or b["title"])[:160])


def card_svg(b: dict) -> str:
    """The showcase card: a fixed 1200x630 picture of words (no game picture in it), for a share."""
    def t(s, n):
        s = s if len(s) <= n else s[:n - 1] + "…"
        return e(s)
    lin = setup().lineage(b)
    based = f'Based on {lin[0]["title"]} by {lin[0]["by"]}' if lin else ""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">'
            '<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#2a2440"/>'
            '<stop offset="1" stop-color="#10202c"/></linearGradient></defs><rect width="1200" height="630" fill="url(#g)"/>'
            '<rect x="40" y="40" width="1120" height="550" rx="28" fill="none" stroke="#8c78ff" stroke-opacity=".5" stroke-width="3"/>'
            f'<text x="90" y="140" fill="#b9b0ff" font-family="sans-serif" font-size="30" letter-spacing="4">{e(KINDS[b["kind"]].upper())}</text>'
            f'<text x="90" y="240" fill="#fff" font-family="sans-serif" font-size="76" font-weight="700">{t(b["title"], 26)}</text>'
            f'<text x="90" y="310" fill="#d8d4ff" font-family="sans-serif" font-size="38">by {t(b["author"], 36)}</text>'
            f'<text x="90" y="370" fill="#9aa" font-family="sans-serif" font-size="30">{t(based, 60)}</text>'
            f'<text x="90" y="460" fill="#cfd" font-family="sans-serif" font-size="32">{b["props"]} props · {b["likes"]} likes · '
            f'{b["downloads"]} opened</text>'
            f'<text x="90" y="540" fill="#8c78ff" font-family="sans-serif" font-size="30">{MADE_WITH}</text></svg>')


def _review(h, path: str) -> None:
    """Staff: what waits, and one build as its showcase would be (pictures too), whatever the switch says."""
    user = _staff(h, path)
    if user is None:
        return
    st = setup()
    rest = path[len(REVIEW):].strip("/")
    if rest:
        parts = rest.split("/")
        b = st.one(parts[0])
        if b is None:
            return h.send(404, _not_found(), "text/html; charset=utf-8")
        if len(parts) == 3 and parts[1] == "pic":
            p = st.pic_path(b["id"], parts[2])
            return _send_pic(h, p, private=True) if p else h.send(404, b"Not found", "text/plain; charset=utf-8")
        pics = "".join(f'<a href="{REVIEW}/{b["id"]}/pic/{n}"><img src="{REVIEW}/{b["id"]}/pic/{n}" alt="picture {i + 1}"></a>'
                       for i, n in enumerate(b["pics"] + ((b["next"] or {}).get("pics") or [])))
        facts = json.dumps({k: b[k] for k in ("kind", "acct", "author", "credit", "licence", "props", "bytes", "state", "reason",
                                              "based_on", "based_title", "based_by", "tags")}, indent=1)
        nxt = f'<h2>Update waiting</h2><pre>{e(json.dumps(b["next"], indent=1))}</pre>' if b["next"] else ""
        body = (f'<article class="dm-show"><p><a href="/wrath/admin/inbox">‹ Needs you</a></p><h1>{e(b["title"])}</h1>'
                f'<div class="dm-gal">{pics or "<p>No pictures.</p>"}</div><p>{e(b["descr"])}</p><pre>{e(facts)}</pre>{nxt}'
                '<p class="ly-fine">Approve or reject it in Needs you. Check: the pictures are of this build, nothing rude, no '
                'game files or other people\'s work.</p></article>')
        return h.send(200, _frame("Review: " + b["title"], body, path), "text/html; charset=utf-8", headers={"Cache-Control": "no-store"})
    rows = st.rows("state = 'pending' OR next != ''", (), "created ASC", 200)
    body = ('<section class="ly-panel"><h1>Shared Dens waiting</h1>'
            + ("".join(f'<p><a href="{REVIEW}/{b["id"]}">{e(b["title"])}</a> by {e(b["author"])} · {e(b["state"])}'
                       f'{" · update waiting" if b["next"] else ""}</p>' for b in rows) or "<p>Nothing waits.</p>")
            + '</section>')
    h.send(200, _frame("Shared Dens waiting", body, path), "text/html; charset=utf-8", headers={"Cache-Control": "no-store"})
