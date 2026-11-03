"""Loadouts on the website (owner 3 Nov 2026: "Encourage downloading mods onto one's own client: style / loadout for
clients. Make that part intuitive so people can load various versions of their games, templates and other people's
realms without losing game file access and ease"). The launcher side is launcher/src-tauri/src/loadouts.rs and ui/loadouts.js; standard library only.

A loadout is a named set of mods, addons and a graphics preset for one game folder in the launcher (launcher/src-tauri/
src/loadouts.rs). Switching turns Leyline's own installs on and off; the game's files are never touched, and Clean (every
Leyline mod off) is one click away. This module has:
  - the templates (loadouts.json beside this file): starter loadouts of REVIEWED catalog entries only. A template id
    that is not published (a pending addon, a typo) is dropped here, so the site never shows it, and test_loadouts.py
    fails on it so it gets fixed;
  - share codes in the launcher's format (share_code, parse_code: the same rules as loadouts.rs), catalog ids only;
  - clean_loadout: a realm listing's suggested loadout ("loadout": {"name", "items", "gfx"}), the same rules as
    hosting/relay/wurelay/listing.py clean_loadout and realms.rs parse_loadout.
"""
from __future__ import annotations

import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = os.environ.get("WU_LOADOUTS", os.path.join(HERE, "loadouts.json"))

# loadouts.rs: CODE_PREFIX, MAX_ITEMS, MAX_NAME, MAX_CODE; gfx.rs PRESETS ids; catalog.rs id_ok and addons.rs KEY
CODE_PREFIX = "leyline-loadout:1;"
MAX_ITEMS = 64
MAX_NAME = 40
MAX_CODE = 4000
PRESETS = ("low", "classic", "balanced", "high", "ultra")
PRESET_NAME = {"low": "Low", "classic": "Classic", "balanced": "Balanced", "high": "High", "ultra": "Ultra"}
LEYLINE_ADDONS = "leyline:addons"
_SLUG = re.compile(r"^[a-z][a-z0-9-]{2,31}$")
_LINK = re.compile(r"^[0-9a-f]{12}$")


def id_ok(i) -> bool:
    """A catalog id (catalog.rs id_ok) or the Leyline addons."""
    if not isinstance(i, str):
        return False
    if i == LEYLINE_ADDONS:
        return True
    kind, _, rest = i.partition(":")
    if kind in ("mod", "up", "look"):
        return bool(_SLUG.fullmatch(rest)) and "--" not in rest and not rest.endswith("-")
    return kind == "link" and bool(_LINK.fullmatch(rest))


def clean_name(s) -> str:
    if not isinstance(s, str):
        return ""
    t = "".join(c for c in s if c.isprintable()).strip()[:MAX_NAME]
    return t.strip()


def clean_shared(v) -> dict | None:
    """{"name", "items", "gfx"} made safe (loadouts.rs clean_shared): a name, catalog ids each once, a known preset."""
    if not isinstance(v, dict):
        return None
    name = clean_name(v.get("name"))
    if not name:
        return None
    items: list[str] = []
    for i in (v.get("items") if isinstance(v.get("items"), list) else [])[:MAX_ITEMS * 2]:
        if id_ok(i) and i not in items:
            items.append(i)
        if len(items) >= MAX_ITEMS:
            break
    gfx = v.get("gfx") if v.get("gfx") in PRESETS else ""
    return {"name": name, "items": items, "gfx": gfx}


def clean_loadout(v, realm_name: str = "") -> dict | None:
    """A realm listing's "loadout" (realms.rs parse_loadout): None unless it names something installable."""
    if not isinstance(v, dict):
        return None
    name = v.get("name") if isinstance(v.get("name"), str) and v["name"].strip() else realm_name
    s = clean_shared(dict(v, name=name))
    if s is None or not (s["items"] or s["gfx"]):
        return None
    return s


def _pct(s: str) -> str:
    return "".join(chr(b) if (chr(b).isascii() and (chr(b).isalnum() or chr(b) in "-_.")) else f"%{b:02X}" for b in s.encode())


def share_code(t: dict) -> str:
    """The launcher's share code (loadouts.rs share_code): the name, the preset and the catalog ids."""
    ids = [i for i in t.get("items", []) if id_ok(i)]
    return f"{CODE_PREFIX}n={_pct(t.get('name', ''))};g={_pct(t.get('gfx', ''))};i={','.join(ids)}"


def parse_code(code: str) -> dict | None:
    """A share code read back (loadouts.rs parse_code), or None."""
    import urllib.parse
    if not isinstance(code, str) or len(code.strip()) > MAX_CODE or not code.strip().startswith(CODE_PREFIX):
        return None
    out = {"name": "", "items": [], "gfx": ""}
    for part in code.strip()[len(CODE_PREFIX):].split(";"):
        k, _, v = part.partition("=")
        if k == "n":
            out["name"] = urllib.parse.unquote(v)
        elif k == "g":
            out["gfx"] = urllib.parse.unquote(v)
        elif k == "i":
            out["items"] = [x for x in v.split(",") if x]
    out["name"] = out["name"].strip() or "Shared loadout"
    return clean_shared(out)


# ---------------------------------------------------------------------------------------------------- templates

_cache: dict = {"key": None, "doc": []}


def raw_templates(path: str | None = None) -> list[dict]:
    """Every template in the file (checked for shape only), in order."""
    path = path or PATH
    try:
        key = (path, os.path.getmtime(path))
    except OSError:
        return []
    if _cache["key"] == key:
        return _cache["doc"]
    try:
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
    except (OSError, ValueError):
        doc = {}
    out = []
    for t in (doc.get("templates") if isinstance(doc, dict) and isinstance(doc.get("templates"), list) else [])[:40]:
        if not isinstance(t, dict) or not isinstance(t.get("id"), str) or not _SLUG.fullmatch(t["id"]):
            continue
        s = clean_shared(t)
        if s is None:
            continue
        line = t.get("line") if isinstance(t.get("line"), str) else ""
        builds = [b for b in (t.get("builds") or []) if isinstance(b, str)][:6]
        out.append(dict(s, id=t["id"], line=line[:200], builds=builds, listed=list(t.get("items") or [])))
    _cache.update(key=key, doc=out)
    return out


def templates(published: set | None, path: str | None = None) -> list[dict]:
    """The templates the site shows: only published ids (and the Leyline addons) stay in; a template left with nothing
    to install and no preset is dropped. `published` None: the reviewed addons and the Workshop as this server has them."""
    pub = set(published) if published is not None else published_ids()
    out = []
    for t in raw_templates(path):
        items = [i for i in t["items"] if i == LEYLINE_ADDONS or i in pub]
        if not items and not t["gfx"]:
            continue
        out.append(dict(t, items=items))
    return out


def published_ids() -> set:
    """The reviewed upstream addons (stream/upstream_addons.py) this server publishes."""
    try:
        import importlib.util
        import sys
        m = sys.modules.get("wu_upstream_addons")
        if m is None:
            spec = importlib.util.spec_from_file_location("wu_upstream_addons", os.path.join(HERE, "upstream_addons.py"))
            m = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(m)
            sys.modules["wu_upstream_addons"] = m
        return {p["id"] for p in m.projects()}
    except Exception:   # noqa: BLE001: no reviewed addons, only the preset templates
        return set()
