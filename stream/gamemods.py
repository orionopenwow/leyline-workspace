"""Game mods and the mods a realm uses (owner 31 Oct 15:13: "a client mod section like future skating and golf ... we
need to ensure these mods work on servers that allow it too like minecraft style adaptations and downloads and like
gmod too"). docs/GAME-MODS.md has the whole model; standard library only, loaded by stream/wrath_portal.py beside
hub.py (Hub reaches it through portal("gamemods")) and network.py.

Game mods are the things that change how you play in game:
  - Leyline Modes (stream/playmodes.json kind "realm": minigolf, bumper balls, coin dash, community minigames). They
    run on any realm with the Leyline realm scripts (ov_modes.lua, ov_golf.lua, ov_playmode.lua: every AzerothCore realm
    the host kit makes, and ours) and show in game through the Leyline addons. Id "mode:<id>".
  - Workshop mods in a game category (GAME_CATS: sandbox, maps, encounters, conversions). Id "mod:<id>" (packaged) or
    "up:<slug>". A realm-side one ("realm" or "both") only plays on a realm that installed it.
A future one (skating and the like) is listed as "Coming" only once it is a real entry in one of those two lists.

A realm's mod list (the realm listing's "mods", hosting/relay/wurelay/listing.py clean_mods keeps the same rules):
    [{"id": "mode:minigolf"}, {"id": "mod:dalaran-dice", "version": "0.1.0"}, {"id": "up:skada"}]
at most MAX_MODS, ids in ID_RE, versions in VERSION_RE, each id once. It is only a list of names: it never carries a
URL, a file or code, and a player's launcher only ever offers mods it finds in its own Workshop catalog (with the
catalog's own hashes), never anything else a realm names.
"""
from __future__ import annotations

import re

ID_RE = re.compile(r"^(?:mod:[a-z][a-z0-9-]{2,31}|up:[a-z0-9][a-z0-9._-]{0,63}|mode:[a-z][a-z0-9-]{1,31})$")
VERSION_RE = re.compile(r"^[0-9A-Za-z][0-9A-Za-z.+-]{0,31}$")
MAX_MODS = 32
# The Leyline Modes every realm with the Leyline realm scripts runs (playmodes.json kind "realm", live). The host kit
# keeps the same list (hosting/kit/tools/wuhost/setup.py BUILTIN_MODES); stream/tests/test_gamemods.py checks all three.
BUILTIN_MODES = ("minigolf", "bumper-balls", "coin-dash", "community")
GAME_CATS = ("sandbox", "maps", "encounters", "conversions")


def clean_mods(v) -> list[dict]:
    """A realm's mod list, checked: [{"id", "version"?}], bad entries dropped, each id once, at most MAX_MODS."""
    out, seen = [], set()
    for m in (v if isinstance(v, list) else [])[:MAX_MODS * 2]:
        if isinstance(m, str):
            m = {"id": m}
        if not isinstance(m, dict):
            continue
        mid = m.get("id")
        if not isinstance(mid, str) or not ID_RE.fullmatch(mid) or mid in seen:
            continue
        seen.add(mid)
        one = {"id": mid}
        ver = m.get("version")
        if isinstance(ver, str) and VERSION_RE.fullmatch(ver):
            one["version"] = ver
        out.append(one)
        if len(out) >= MAX_MODS:
            break
    return out


def builtin() -> list[dict]:
    return [{"id": "mode:" + m} for m in BUILTIN_MODES]


def official(projects: list | None, modes: list | None = None) -> list[dict]:
    """Our own realm's mod list, from its own config: the live Leyline Modes it runs (playmodes.json, else the built-in
    list) and every packaged Workshop mod the `mods` step installs on it (all.json on_wrath, with its version)."""
    live = [m["id"] for m in (modes or []) if isinstance(m, dict) and m.get("kind") == "realm" and m.get("status") == "live"
            and isinstance(m.get("id"), str)]
    out = [{"id": "mode:" + i} for i in (live or BUILTIN_MODES)]
    for p in projects or []:
        if isinstance(p, dict) and p.get("source") == "packaged" and p.get("on_wrath") is True:
            out.append({"id": p.get("id"), "version": p.get("version")})
    return clean_mods(out)


def side(mid: str, project: dict | None = None) -> str:
    """Where a listed mod runs: "realm" (nothing to add on the client beyond Leyline itself), "client" or "both".
    A Leyline Mode is "realm": it plays through the Leyline addons, which the launcher already offers on its own."""
    if mid.startswith("mode:"):
        return "realm"
    if mid.startswith("up:"):
        return "client"
    kind = (project or {}).get("kind")
    return {"ui": "client", "both": "both", "realm": "realm"}.get(kind, "client")


def needs_client(mids: list[dict], projects: list | None) -> list[str]:
    """The ids of a realm's list a player's client needs (ui/both packaged mods and upstream addons), only those in
    the Workshop catalog: what the launcher's "Get them and play" may install. A realm-only mod needs nothing."""
    by_id = {p.get("id"): p for p in projects or [] if isinstance(p, dict)}
    out = []
    for m in clean_mods(mids):
        p = by_id.get(m["id"])
        if p is None or side(m["id"], p) == "realm":
            continue
        if p.get("source") == "packaged" and not (p.get("download") and p.get("sha256")):
            continue   # nothing the launcher could check: never offered
        out.append(m["id"])
    return out


def entries(modes: list | None, projects: list | None) -> list[dict]:
    """The Game mods section: the Leyline Modes, then the Workshop's game-category mods. Each:
    {"id", "name", "line", "status" live|coming, "realm" (needs a realm that allows it), "kind" mode|mod,
     "addon" (plays through the Leyline addons), "client" (the launcher installs something), "project"?}"""
    out = []
    for m in modes or []:
        if not isinstance(m, dict) or m.get("kind") != "realm" or not isinstance(m.get("id"), str):
            continue
        out.append({"id": "mode:" + m["id"], "mode": m["id"], "name": m.get("title") or m["id"], "line": m.get("line") or "",
                    "status": "coming" if m.get("status") == "coming" else "live", "realm": True, "kind": "mode",
                    "addon": True, "client": False, "players": m.get("players")})
    for p in projects or []:
        if not isinstance(p, dict) or p.get("category") not in GAME_CATS or not isinstance(p.get("id"), str):
            continue
        if p.get("source") not in ("packaged", "upstream", "workshop"):
            continue
        s = side(p["id"], p)
        out.append({"id": p["id"], "name": p.get("name") or p["id"], "line": p.get("description") or p.get("summary") or "",
                    "status": "coming" if p.get("status") == "coming" else "live", "realm": s in ("realm", "both"),
                    "kind": "mod", "addon": False, "client": s in ("client", "both"), "project": p})
    return out


def runs_on(mid: str, realms: list | None) -> list[dict]:
    """The live realms whose list has this id, in the order given (the network's own)."""
    return [r for r in realms or [] if isinstance(r, dict) and r.get("live")
            and any(m.get("id") == mid for m in clean_mods(r.get("mods")))]
