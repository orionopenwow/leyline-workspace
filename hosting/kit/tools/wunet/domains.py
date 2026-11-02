"""Leyline's public addresses, from leyline.env next to this file (LEYLINE_DOMAIN, and the names before the move).

The environment may override each value: LEYLINE_DOMAIN, LEYLINE_RELAY, LEYLINE_SITE, LEYLINE_JOIN, LEYLINE_GAME.
With the domain leylinelauncher.com: the website https://leylinelauncher.com, the browser game
https://play.leylinelauncher.com, the relay relay.leylinelauncher.com. The old names (play.openvanilla.app,
wrath.openvanilla.app, relay.openvanilla.app) keep answering forever: `fallback` gives the old twin of a new address,
and SITE_ORIGINS / PLAY_ORIGINS accept both (docs/DOMAIN-MOVE.md).
"""
import os

FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "leyline.env")


def _read(path=FILE):
    out = {}
    try:
        with open(path, encoding="utf-8") as f:
            for ln in f:
                ln = ln.strip()
                if ln and not ln.startswith("#") and "=" in ln:
                    k, v = ln.split("=", 1)
                    out[k.strip()] = v.split("#", 1)[0].strip()
    except OSError:
        pass
    return out


def load(env=None, path=FILE):
    env = os.environ if env is None else env
    f = _read(path)
    g = lambda k, d="": env.get(k) or f.get(k) or d   # noqa: E731
    domain = g("LEYLINE_DOMAIN", "leylinelauncher.com")
    relay = g("LEYLINE_RELAY", f"relay.{domain}")
    site = g("LEYLINE_SITE", f"https://{domain}")
    game = g("LEYLINE_GAME", f"https://play.{domain}")
    join = g("LEYLINE_JOIN", f"https://{relay}")
    old = {"site": g("LEYLINE_OLD_SITE").rstrip("/"), "game": g("LEYLINE_OLD_GAME").rstrip("/"),
           "relay": g("LEYLINE_OLD_RELAY")}
    return {"domain": domain, "relay": relay, "site": site.rstrip("/"), "game": game.rstrip("/"),
            "join": join.rstrip("/"), "old": old}


_d = load()
DOMAIN, RELAY, SITE, GAME, JOIN, OLD = _d["domain"], _d["relay"], _d["site"], _d["game"], _d["join"], _d["old"]
# origins the relay and hosts accept from the website and the browser game: both names of each
SITE_ORIGINS = [o for o in (SITE, OLD["site"]) if o]
PLAY_ORIGINS = [o for o in (GAME, SITE, OLD["game"], OLD["site"]) if o]


def _pairs(d=None):
    d = d or _d
    o = d["old"]
    out = []
    for new, old in ((d["relay"], o["relay"]), (d["site"].split("://", 1)[-1], o["site"].split("://", 1)[-1]),
                     (d["game"].split("://", 1)[-1], o["game"].split("://", 1)[-1])):
        if new and old and new != old:
            out.append((new.lower(), old.lower()))
    return out


def fallback(url: str, d=None):
    """The same address on the old name when `url` is on one of the new ones (path and query kept), else None:
    fallback("wss://relay.leylinelauncher.com/agent") == "wss://relay.openvanilla.app/agent"."""
    if "://" not in url:
        return None
    scheme, rest = url.split("://", 1)
    cut = min([i for i in (rest.find("/"), rest.find("?"), rest.find("#"), rest.find(":")) if i >= 0] or [len(rest)])
    host = rest[:cut].lower()
    for new, old in _pairs(d):
        if host == new:
            return f"{scheme}://{old}{rest[cut:]}"
    return None


def current(host: str, d=None) -> str:
    """The name players read for an old one (relay.openvanilla.app -> relay.leylinelauncher.com); others unchanged."""
    for new, old in _pairs(d):
        if host.lower() == old:
            return new
    return host
