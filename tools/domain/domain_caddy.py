#!/usr/bin/env python3
"""The domain move's Caddy part (docs/DOMAIN-MOVE.md): the new names on the existing site blocks, a www block, and
(phase "redirect") a 301 for web pages on the old names. Run by domain.sh; stream-up.sh and relay-up.sh run
"reapply" after they rewrite their own blocks, so a later deploy never drops the new names.

    python3 -I domain_caddy.py apply    <Caddyfile> alias|redirect [addresses.json]   idempotent
    python3 -I domain_caddy.py reapply  <Caddyfile> [state.json] [addresses.json]     the phase in state.json (none: nothing)
    python3 -I domain_caddy.py remove   <Caddyfile> [addresses.json]                  every change of ours out
    python3 -I domain_caddy.py status   <Caddyfile> [addresses.json]                  one line per part

The new names join the old site blocks' address lines (`play.openvanilla.app, leylinelauncher.com {`): one block,
one backend, so everything the old name answers the new one answers too, and a block's own scripts (stream-up.sh,
gamedata-caddy.py, caddy-apply.sh) keep finding it. The 301 is only for page loads (a browser navigating: GET/HEAD
with Sec-Fetch-Dest: document, or an old browser with no Sec-Fetch-* asking for text/html), never for what
launchers, host packs, the game page, iframes or the relay's agents call: those keep answering on the old names
exactly as today. Exit codes: 0 done, 2 a broken Caddyfile or a site block missing, 1 usage.

Standard library only. The Caddyfile is edited in place (it is bind-mounted into Caddy's container).
"""
from __future__ import annotations

import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ADDRESSES = os.path.join(HERE, "addresses.json")
PARTS = ("site", "game", "relay")
# Never redirected, even as a page load: sign-in round trips that must finish where they started (OAuth state
# cookies are per host), the game's gate and data, the relay's agents, tunnels and APIs.
EXEMPT = {
    "site": ("/auth/*", "/__*"),
    "game": ("/__gate", "/__env.js", "/__auth", "/data", "/data/*", "/ws/*"),
    "relay": ("/agent", "/r/*", "/api/*", "/ws/*", "/healthz", "/metrics"),
}
RD_BEGIN = "\t# >>> Leyline domain move (tools/domain/domain.sh)"
RD_END = "\t# <<< Leyline domain move"
WWW_BEGIN = "# >>> Leyline domain move: www (tools/domain/domain.sh)"
WWW_END = "# <<< Leyline domain move: www"


def load(path: str = ADDRESSES) -> dict:
    with open(path, encoding="utf-8") as f:
        a = json.load(f)
    for p in PARTS:
        if not (a.get(p, {}).get("new") and a[p].get("old")):
            raise ValueError(f"addresses: {p} needs new and old")
    return a


def blocks(text: str):
    """(start, open_brace_end, close_end, head) of each top-level block (comments and quotes skipped)."""
    out, depth, i, n = [], 0, 0, len(text)
    start = brace = 0
    head = ""
    while i < n:
        c = text[i]
        if c == "#" and (i == 0 or text[i - 1] in " \t\n"):
            i = text.find("\n", i)
            if i < 0:
                break
            continue
        if c in '"`':
            j = i + 1
            while j < n and text[j] != c:
                j += 2 if text[j] == "\\" and c == '"' else 1
            i = j + 1
            continue
        if c == "{":
            if depth == 0:
                start = text.rfind("\n", 0, i) + 1
                head = text[start:i].strip()
                brace = i + 1
            depth += 1
        elif c == "}":
            depth -= 1
            if depth < 0:
                raise ValueError("a '}' without its '{' in the Caddyfile")
            if depth == 0:
                out.append((start, brace, i + 1, head))
        i += 1
    if depth:
        raise ValueError("a '{' without its '}' in the Caddyfile")
    return out


def hosts(head: str) -> list[str]:
    return [h for h in re.split(r"[\s,]+", head) if h]


def bare(h: str) -> str:
    h = h.lower()
    for pre in ("https://", "http://"):
        if h.startswith(pre):
            h = h[len(pre):]
    return h.split(":")[0]


def find(text: str, host: str):
    """The top-level block whose address line names `host` (with or without scheme/port), or None."""
    for b in blocks(text):
        if b[3] and host in [bare(h) for h in hosts(b[3])]:
            return b
    return None


def _set_head(text: str, b, names: list[str]) -> str:
    return text[:b[0]] + ", ".join(names) + " " + text[b[1] - 1:]   # b[1] - 1: the block's "{"


def _strip_marked(text: str, begin: str, end: str) -> str:
    while begin in text:
        i = text.index(begin)
        j = text.find(end, i)
        if j < 0:
            raise ValueError(f"'{begin.strip()}' without its end marker in the Caddyfile")
        j = text.find("\n", j)
        if not begin.startswith("\t") and text[max(0, i - 2):i] == "\n\n":
            i -= 1   # the blank line apply() put before a top-level block of ours
        text = text[:i] + (text[j + 1:] if j >= 0 else "")
    return text


def unalias(text: str, a: dict) -> str:
    news = {a[p]["new"] for p in PARTS}
    for p in PARTS:
        b = find(text, a[p]["new"])
        if b is None:
            continue
        keep = [h for h in hosts(b[3]) if bare(h) not in news]
        if not keep:   # a block that only ever had a new name is not ours: leave it
            continue
        text = _set_head(text, b, keep)
    return text


def remove(text: str, a: dict) -> str:
    text = _strip_marked(text, RD_BEGIN, RD_END)
    text = _strip_marked(text, WWW_BEGIN, WWW_END)
    return unalias(text, a)


def snippet(part: str, a: dict) -> str:
    old, new = a[part]["old"], a[part]["new"]
    ex = " ".join(EXEMPT[part])
    return (f"{RD_BEGIN}: page loads on {old} go to {new}; everything else answers here as before\n"
            "\t@ly_moved {\n"
            f"\t\thost {old}\n\t\tmethod GET HEAD\n\t\theader Sec-Fetch-Dest document\n\t\tnot path {ex}\n\t}}\n"
            "\t@ly_moved_nofetch {\n"
            f"\t\thost {old}\n\t\tmethod GET HEAD\n\t\tnot header Sec-Fetch-Dest *\n\t\theader Accept *text/html*\n"
            f"\t\tnot path {ex}\n\t}}\n"
            f"\tredir @ly_moved https://{new}{{uri}} 301\n"
            f"\tredir @ly_moved_nofetch https://{new}{{uri}} 301\n"
            f"{RD_END}\n")


def www_block(a: dict) -> str:
    w = a["www"]
    return (f"\n{WWW_BEGIN}: the www name goes to the site\n"
            f"{w['new']} {{\n\tredir https://{w['to']}{{uri}} 301\n}}\n{WWW_END}\n")


def apply(text: str, phase: str, a: dict, strict: bool = True) -> tuple[str, list[str]]:
    """The Caddyfile with the new names (phase alias), and the 301 for old page loads (phase redirect). Not strict
    (reapply, after another script rewrote the file): a site block that is not there now is skipped, not an error."""
    if phase not in ("alias", "redirect"):
        raise ValueError("phase: alias or redirect")
    notes = []
    text = _strip_marked(text, RD_BEGIN, RD_END)
    for p in PARTS:
        old, new = a[p]["old"], a[p]["new"]
        b = find(text, old)
        if b is None:
            if p == "relay" or not strict:
                notes.append(f"{p}: no {old} block in this Caddyfile: skipped")
                continue
            raise LookupError(f"no site block for {old} in the Caddyfile")
        other = find(text, new)
        if other is not None and other[0] != b[0]:
            raise LookupError(f"{new} already has its own site block (not ours): nothing changed")
        names = hosts(b[3])
        if new not in [bare(h) for h in names]:
            text = _set_head(text, b, names + [new])
            notes.append(f"{p}: {new} added to the {old} block")
        else:
            notes.append(f"{p}: {new} already on the {old} block")
    if WWW_BEGIN not in text:
        if find(text, a["www"]["new"]) is not None:
            raise LookupError(f"{a['www']['new']} already has its own site block (not ours): nothing changed")
        text = text.rstrip("\n") + "\n" + www_block(a)
        notes.append(f"www: {a['www']['new']} -> https://{a['www']['to']} (301)")
    if phase == "redirect":
        for p in PARTS:
            b = find(text, a[p]["old"])
            if b is None:
                continue
            k = text.find("\n", b[1] - 1) + 1
            text = text[:k] + snippet(p, a) + text[k:]
            notes.append(f"{p}: page loads on {a[p]['old']} answer 301 to {a[p]['new']}")
    return text, notes


def status(text: str, a: dict) -> dict:
    out = {}
    for p in PARTS:
        b = find(text, a[p]["old"])
        if b is None:
            out[p] = "no-block"
        elif a[p]["new"] in [bare(h) for h in hosts(b[3])]:
            out[p] = "redirect" if RD_BEGIN in text[b[1]:b[2]] else "alias"
        else:
            out[p] = "old-only"
    out["www"] = "on" if WWW_BEGIN in text else "off"
    return out


# ---- a model of the redirect rules above (the tests use it to follow sign-in flows through Caddy)
def _path_match(pat: str, path: str) -> bool:
    if pat.endswith("*"):
        return path.startswith(pat[:-1])
    return path == pat


def would_redirect(a: dict, phase: str, host: str, method: str, uri: str, headers: dict) -> str | None:
    """Where Caddy sends this request (the Location), or None when the backend answers it."""
    host = bare(host)
    hdr = {k.lower(): v for k, v in headers.items()}
    if host == a["www"]["new"]:
        return f"https://{a['www']['to']}{uri}"
    if phase != "redirect":
        return None
    for p in PARTS:
        if host != a[p]["old"]:
            continue
        path = uri.split("?", 1)[0]
        if method not in ("GET", "HEAD") or any(_path_match(x, path) for x in EXEMPT[p]):
            return None
        dest = hdr.get("sec-fetch-dest", "")
        page = dest == "document" or (not dest and "text/html" in hdr.get("accept", ""))
        return f"https://{a[p]['new']}{uri}" if page else None
    return None


def _write(path: str, text: str) -> None:
    with open(path, "r+", encoding="utf-8") as f:   # same file: it is bind-mounted into Caddy
        f.seek(0)
        f.write(text)
        f.truncate()


def main(argv: list[str]) -> int:
    if len(argv) < 3 or argv[1] not in ("apply", "reapply", "remove", "status"):
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 1
    cmd, cf = argv[1], argv[2]
    try:
        with open(cf, encoding="utf-8") as f:
            text = f.read()
        if cmd == "apply":
            if len(argv) < 4:
                print("apply needs alias or redirect", file=sys.stderr)
                return 1
            a = load(argv[4] if len(argv) > 4 else ADDRESSES)
            new, notes = apply(text, argv[3], a)
        elif cmd == "reapply":
            a = load(argv[4] if len(argv) > 4 else ADDRESSES)
            try:
                with open(argv[3] if len(argv) > 3 else "/srv/wrath/domain/domain.json", encoding="utf-8") as f:
                    phase = (json.load(f) or {}).get("phase")
            except (OSError, ValueError):
                phase = None
            if phase not in ("alias", "redirect"):
                print("domain move: off (nothing to reapply)")
                return 0
            new, notes = apply(text, phase, a, strict=False)
        elif cmd == "remove":
            a = load(argv[3] if len(argv) > 3 else ADDRESSES)
            new, notes = remove(text, a), ["the new names, the www block and the redirects are out"]
        else:
            a = load(argv[3] if len(argv) > 3 else ADDRESSES)
            for k, v in status(text, a).items():
                print(f"{k}: {v}")
            return 0
        blocks(new)   # still balanced
    except (OSError, ValueError, LookupError) as e:
        print(f"domain_caddy: {e}", file=sys.stderr)
        return 2
    if new != text:
        _write(cf, new)
    for n in notes:
        print("  " + n)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
