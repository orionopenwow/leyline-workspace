"""The Leyline documentation on the Wrath portal (stream/wrath_portal.py), in the Leyline look.

    GET /wrath/docs             the overview (docs/leyline/index.md)
    GET /wrath/docs/<page>      one page; <page> is one of PAGES below, anything else is a 404

The pages are Markdown files in docs/leyline/ (in the repo) or in docs/leyline/ beside this file (on the
server: $WU/portal/docs/leyline, which wu-portal sees as /app/docs/leyline; fastloop.sh installs them).
The portal sends each page through its site_chrome(), which puts in the site's one nav (with its Docs item) and
footer in place of the small nav here; with the hub's front door on it passes its hub frame instead (page(frame=)),
and the page is drawn in the hub's side menu, tokens and footer (DOCS_HUB_CSS).
WU_DOCS_DIR overrides both.

The renderer is small and safe on purpose. It knows headings, paragraphs, lists, tables, fenced code,
block quotes, inline code, bold and links. Every character from a file is HTML-escaped: raw HTML in a file
shows as text, never as markup. Links go only to https:// pages, to /wrath/ paths, to #anchors, or to
another page of the fixed list (written "hosting.md" or "hosting.md#the-relay"); any other link shows its
text without a link. The page list is fixed: a file that isn't in PAGES is never read or served.

Standard library only. The portal loads this file from beside itself and runs without it (the routes then
answer 404). Nothing here raises to the caller.
"""
from __future__ import annotations

import html
import os
import re
import threading

# The fixed, allow-listed pages: (name, title in the table of contents). The order is the contents' order.
PAGES = (
    ("index", "Overview"),
    ("network", "The network"),
    ("getting-started", "Getting started"),   # the player guide (the launcher's Help and Find your game files link it). TODO(owner): his words
    ("play", "Play"),
    ("single-player", "Single player"),   # one click, no server: the original world, Quick start, Characters, a friend (docs/SINGLE-PLAYER-ONE-CLICK.md)
    ("solo", "Leyline Solo"),   # the standalone first-era app: install, companions, a friend, fixes (stream/solo_page.py is the showcase, /wrath/solo)
    ("friends", "Friends"),   # Friends on the Leyline network: friend codes, people near you, invites, fixes (docs/LEYLINE-FRIENDS.md)
    ("wrath-unleashed", "Wrath Unleashed notes"),   # the realm's features (docs/realm-guide/wrath-unleashed.json, tools/realm-guide)
    ("commands", "Commands"),
    ("mods", "Mod format"),
    ("publishing", "Publishing"),
    ("themes", "Launcher themes"),   # theme packs (docs/THEMES.md for players; the launcher's Make a theme links it)
    ("hosting", "Hosting"),
    ("host-a-realm", "Host a realm"),   # the launcher's Host page, start to finish (launcher/ui/hostflow.js, host/flow.rs)
    ("den-invite", "Invite a friend to your Den"),   # docs/DEN-INVITE.md, the same text (launcher host/denways.rs)
    ("host-for-friends", "Host for friends"),   # Play with friends in the launcher: one invite, Ready for friends?, fixes (ui/hostfriends.js)
    ("always-on", "Keep your realm up 24/7"),
    ("join-network", "Bring your server"),   # an existing server joins with Hero Key sign-in (setup.py join-network)
    ("be-a-node", "Help carry the network"),   # community nodes (hosting/relay/wurelay/nodes.py)
    ("launcher", "Launcher"),
    ("mobile", "On your phone"),   # the phone app and the browser on a phone (stream/mobile_page.py is the showcase, /wrath/mobile)
    ("den", "Your Den"),   # Your Den: open it, build, invite (the Forge page's My Den section links it)
    ("agents", "Agents"),   # the launcher's Agents page: your own AI assistant, what it may do (launcher/src-tauri/src/agents.rs)
    ("hero-key", "Hero Key"),
    ("cross-server", "Other servers"),
    ("unite", "Working together"),
)
TITLES = dict(PAGES)
# The contents, grouped (audit 12 Nov: one flat list of 24 mixed player, host and developer pages). Every page is in
# exactly one group; within a group, this order.
GROUPS = (
    ("Start", ("index", "getting-started", "launcher", "hero-key", "mobile", "agents")),
    ("Play", ("play", "single-player", "solo", "friends", "den", "den-invite", "commands", "wrath-unleashed")),
    ("Host", ("host-a-realm", "host-for-friends", "hosting", "always-on", "join-network")),
    ("Make mods", ("mods", "publishing", "themes")),
    ("Network", ("network", "be-a-node", "cross-server", "unite")),
)
GROUP_OF = {n: g for g, names in GROUPS for n in names}
BASE = "/wrath/docs"
SEARCH = BASE + "/search"            # the docs search: a page (works without script) and its index
SEARCH_JSON = BASE + "/search.json"
MAX_BYTES = 256 * 1024      # a page file larger than this is not served
HERE = os.path.dirname(os.path.abspath(__file__))


def docs_dir() -> str:
    """Where the Markdown files are: WU_DOCS_DIR, else docs/leyline beside this file, else the repo's."""
    env = os.environ.get("WU_DOCS_DIR", "").strip()
    if env:
        return env
    for d in (os.path.join(HERE, "docs", "leyline"), os.path.join(HERE, "..", "docs", "leyline")):
        if os.path.isfile(os.path.join(d, "index.md")):
            return d
    return os.path.join(HERE, "docs", "leyline")


def page_url(name: str) -> str:
    return BASE if name == "index" else f"{BASE}/{name}"


def match(path: str) -> bool:
    return path == BASE or path.startswith(BASE + "/")


# ------------------------------------------------------------------------------------------- Markdown
def slug(text: str) -> str:
    """A heading's anchor id: "The relay" -> "the-relay"."""
    s = re.sub(r"[^a-z0-9 -]", "", text.lower().replace("`", ""))
    s = re.sub(r"[\s-]+", "-", s).strip("-")
    return s or "section"


_SAFE_HTTPS = re.compile(r"^https://[A-Za-z0-9.-]+(?:/[A-Za-z0-9._~:/?#\[\]@!$&*+,;=%-]*)?$")
_SAFE_LOCAL = re.compile(r"^/wrath(?:/[A-Za-z0-9._~/?=&#%-]*)?$")
_ANCHOR = re.compile(r"^#[a-z0-9-]+$")
_PAGE_LINK = re.compile(r"^([a-z0-9-]+)\.md(?:#([a-z0-9-]+))?$")


def resolve(url: str) -> str | None:
    """The href for a link written in a page, or None when it is not allowed (the text shows unlinked)."""
    if _SAFE_HTTPS.match(url) or _SAFE_LOCAL.match(url) or _ANCHOR.match(url):
        return url
    m = _PAGE_LINK.match(url)
    if m and m.group(1) in TITLES:
        return page_url(m.group(1)) + (f"#{m.group(2)}" if m.group(2) else "")
    return None


_INLINE = re.compile(r"`([^`]+)`|\*\*(.+?)\*\*|\[([^\]]+)\]\(([^)\s]+)\)|\{\{v:([0-9A-Za-z.() -]{1,24})\}\}")
# A game version is never in a sentence (owner rule: "no client version numbers in prose"): a guide that must say
# which game a thing needs writes {{v:3.3.5a}} and the page shows the small version chip, as the site's other pages.
VERSION_CHIP = '<span class="ver" title="Game version">{}</span>'


def inline(text: str, links: bool = True) -> str:
    """One line of Markdown text as safe HTML: `code`, **bold**, [text](link); everything else escaped."""
    out, pos = [], 0
    for m in _INLINE.finditer(text):
        if m.group(3) is not None and not links:
            continue
        out.append(html.escape(text[pos:m.start()]))
        if m.group(5) is not None:
            out.append(VERSION_CHIP.format(html.escape(m.group(5).strip())))
        elif m.group(1) is not None:
            out.append(f"<code>{html.escape(m.group(1))}</code>")
        elif m.group(2) is not None:
            out.append(f"<strong>{inline(m.group(2), links)}</strong>")
        else:
            href = resolve(m.group(4))
            label = inline(m.group(3), links=False)
            if href is None:
                out.append(label)
            else:
                ext = href.startswith("https://")
                out.append(f'<a href="{html.escape(href, quote=True)}"'
                           + (' rel="noopener noreferrer"' if ext else "") + f">{label}</a>")
        pos = m.end()
    out.append(html.escape(text[pos:]))
    return "".join(out)


def _cells(line: str) -> list[str]:
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|") and not s.endswith("\\|"):
        s = s[:-1]
    return [c.strip().replace("\\|", "|") for c in re.split(r"(?<!\\)\|", s)]


_HEAD = re.compile(r"^(#{1,4})\s+(.+?)\s*#*\s*$")
_ITEM = re.compile(r"^\s*([-*]|\d+\.)\s+(.*)$")
_TABLE_SEP = re.compile(r"^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$")


def render_markdown(text: str) -> tuple[str, list[tuple[int, str, str]]]:
    """(the HTML, the headings as (level, id, text)). Headings get unique ids for anchors."""
    lines = text.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    out: list[str] = []
    heads: list[tuple[int, str, str]] = []
    used: dict[str, int] = {}
    para: list[str] = []
    i = 0

    def flush():
        if para:
            out.append(f"<p>{inline(' '.join(para))}</p>")
            para.clear()

    while i < len(lines):
        line = lines[i]
        s = line.strip()
        if s.startswith("```"):
            flush()
            i += 1
            code = []
            while i < len(lines) and not lines[i].strip().startswith("```"):
                code.append(lines[i])
                i += 1
            i += 1
            out.append(f"<pre><code>{html.escape(chr(10).join(code))}</code></pre>")
            continue
        if not s:
            flush()
            i += 1
            continue
        m = _HEAD.match(s)
        if m:
            flush()
            level, title = len(m.group(1)), m.group(2)
            base = slug(title)
            n = used.get(base, 0)
            used[base] = n + 1
            hid = base if n == 0 else f"{base}-{n + 1}"
            heads.append((level, hid, title))
            out.append(f'<h{level} id="{hid}">{inline(title, links=False)}</h{level}>')
            i += 1
            continue
        if re.match(r"^(-{3,}|\*{3,})$", s):
            flush()
            out.append("<hr>")
            i += 1
            continue
        if s.startswith("|") and i + 1 < len(lines) and _TABLE_SEP.match(lines[i + 1].strip()):
            flush()
            head = _cells(s)
            i += 2
            rows = []
            while i < len(lines) and lines[i].strip().startswith("|"):
                rows.append(_cells(lines[i]))
                i += 1
            th = "".join(f"<th>{inline(c)}</th>" for c in head)
            body = "".join("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r[:len(head)])
                           + "<td></td>" * max(0, len(head) - len(r)) + "</tr>" for r in rows)
            out.append(f'<div class="ld-scroll"><table class="ld-table"><thead><tr>{th}</tr></thead>'
                       f"<tbody>{body}</tbody></table></div>")
            continue
        if s.startswith(">"):
            flush()
            quote = []
            while i < len(lines) and lines[i].strip().startswith(">"):
                quote.append(lines[i].strip()[1:].strip())
                i += 1
            out.append(f"<blockquote><p>{inline(' '.join(q for q in quote if q))}</p></blockquote>")
            continue
        m = _ITEM.match(line)
        if m:
            flush()
            ordered = m.group(1)[0].isdigit()
            items: list[str] = []
            while i < len(lines):
                m2 = _ITEM.match(lines[i])
                if m2 and m2.group(1)[0].isdigit() == ordered:
                    items.append(m2.group(2))
                elif items and lines[i].startswith((" ", "\t")) and lines[i].strip() and not _ITEM.match(lines[i]):
                    items[-1] += " " + lines[i].strip()       # a wrapped item line
                else:
                    break
                i += 1
            tag = "ol" if ordered else "ul"
            out.append(f"<{tag}>" + "".join(f"<li>{inline(t)}</li>" for t in items) + f"</{tag}>")
            continue
        para.append(s)
        i += 1
    flush()
    return "".join(out), heads


def links_in(text: str) -> list[str]:
    """Every link target written in a page's Markdown (outside code blocks and code spans)."""
    found, fenced = [], False
    for line in text.split("\n"):
        if line.strip().startswith("```"):
            fenced = not fenced
            continue
        if fenced:
            continue
        for m in _INLINE.finditer(line):
            if m.group(4) is not None:
                found.append(m.group(4))
    return found


# ------------------------------------------------------------------------------------------- the brand switch
# docs/leyline says Leyline (renamed 2026-10-30, for the public repo). Until the owner flips the Leyline brand
# (LEYLINE_BRAND=1 in portal.env, realm/brand.sh on), the site shows these pages with the words they had before:
# (page, the new words, the old words). Delete this table once the brand is on for good.
UNTIL_FLIP = (
    ("hero-key", "is run by the Leyline Launcher realm service on leylinelauncher.com.",
     "is run by the Open Vanilla realm service on leylinelauncher.com."),
    ("hero-key", "These are in the Leyline Launcher repo, not here,",
     "These are in the Open Vanilla repo, not here,"),
    ("hero-key", "(`launcher/herokey_v2.py` in Leyline Launcher)",
     "(`launcher/herokey_v2.py` in Open Vanilla)"),
    ("hero-key", "What `launcher/passport.py` in Leyline Launcher does,",
     "What `launcher/passport.py` in Open Vanilla does,"),
    ("hosting", "1. Download the Host pack: [the Host pack zip](",
     "1. Download the Host pack: [WrathUnleashed-host-kit.zip]("),
    ("hosting", "You get the Host pack's folder.",
     "You get a folder called `WrathUnleashed host`."),
    ("mods", "the lowest Leyline version the mod works with",
     "the lowest WU version the mod works with"),
    ("mods", "\"preview.png - Leyline - MIT - drawn for this mod\"",
     "\"preview.png - WrathUnleashed - MIT - drawn for this mod\""),
    ("mods", "  \"author\": \"Leyline\",",
     "  \"author\": \"WrathUnleashed\","),
    ("play", "The player package has a `Play` .bat file that asks",
     "The player package has a `Play WrathUnleashed.bat` that asks"),
)


# Blocks a page shows only while a site switch is on (or off), so a page never offers what the site doesn't:
#   <!-- if single_player -->  ...  <!-- end if -->     SINGLE_PLAYER_PUBLIC=1 (stream/site_pages.py: the grand release)
#   <!-- if be_a_node --> / <!-- if not be_a_node -->   the be_a_node flag in flags.json (stream/leyline_net.py)
# HTML comments, so GitHub shows the markdown without them; the site drops the block (lines and markers) when its
# switch says so. Unknown switch names count as off. Nested blocks are not supported.
_GATE = re.compile(r"^[ \t]*<!--\s*if\s+(not\s+)?([a-z_]+)\s*-->[ \t]*\n(.*?)^[ \t]*<!--\s*end if\s*-->[ \t]*\n?", re.M | re.S)


def _net_flag(name: str) -> bool:
    try:
        import importlib.util
        import sys
        m = sys.modules.get("wu_leyline_net")
        if m is None:
            spec = importlib.util.spec_from_file_location("wu_leyline_net", os.path.join(HERE, "leyline_net.py"))
            m = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(m)
            sys.modules["wu_leyline_net"] = m
        return bool((m.load().get("flags") or {}).get(name))
    except Exception:   # noqa: BLE001: a switch we can't read is off
        return False


def switch_on(name: str) -> bool:
    """The site switches the docs follow (read per request: no restart)."""
    if name == "single_player":
        return os.environ.get("SINGLE_PLAYER_PUBLIC", "0").strip() == "1"   # the same switch as stream/site_pages.py
    if name == "be_a_node":
        return _net_flag("be_a_node")
    return False


def gated(text: str) -> str:
    """The markdown with each <!-- if ... --> block kept or dropped by its switch (markers always dropped)."""
    def one(m):
        on = switch_on(m.group(2))
        keep = (not on) if m.group(1) else on
        return m.group(3) if keep else ""
    return _GATE.sub(one, text)


def site_text(name: str, text: str) -> str:
    """A page's markdown as the site shows it: as written with the brand on, else with UNTIL_FLIP's old words."""
    if os.environ.get("LEYLINE_BRAND", "0").strip() == "1":
        return text
    for page_name, new, old in UNTIL_FLIP:
        if page_name == name:
            text = text.replace(new, old)
    return text


# ------------------------------------------------------------------------------------------- pages
_cache: dict[str, tuple[tuple, str, list]] = {}
_lock = threading.Lock()


# The pages' Host pack link (the old host-kit release, kept in the markdown) becomes the pack the launcher installs:
# leyline_net.hostpack(), the same descriptor as /wrath/mods/all.json and /wrath/host (hostflow.kit_url()).
OLD_KIT_URL = "https://github.com/orionopenwow/Leyline/releases/download/host-kit/WrathUnleashed-host-kit.zip"


def kit_url() -> str:
    try:
        import importlib.util
        import sys
        m = sys.modules.get("wu_leyline_net")
        if m is None:
            spec = importlib.util.spec_from_file_location("wu_leyline_net", os.path.join(os.path.dirname(os.path.abspath(__file__)), "leyline_net.py"))
            m = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(m)
            sys.modules["wu_leyline_net"] = m
        d = m.hostpack()
        return d["download"] if d and d.get("download") else OLD_KIT_URL
    except Exception:   # noqa: BLE001: the link as written
        return OLD_KIT_URL


def load(name: str) -> tuple[str, list] | None:
    """(HTML, headings) for an allow-listed page, with the current Host pack link; None when it is missing."""
    got = _load(name)
    if got is None:
        return None
    return got[0].replace(OLD_KIT_URL, kit_url()), got[1]


def _load(name: str) -> tuple[str, list] | None:
    """(HTML, headings) for an allow-listed page, cached until its file changes; None when it is missing."""
    if name not in TITLES:
        return None
    path = os.path.join(docs_dir(), name + ".md")
    try:
        st = os.stat(path)
        if st.st_size > MAX_BYTES:
            return None
        key = (st.st_mtime, switch_on("single_player"), switch_on("be_a_node"), os.environ.get("LEYLINE_BRAND", ""))
        with _lock:
            got = _cache.get(path)
            if got and got[0] == key:
                return got[1], got[2]
        with open(path, encoding="utf-8", errors="replace") as f:
            text = f.read(MAX_BYTES)
        body, heads = render_markdown(site_text(name, gated(text)))
        with _lock:
            _cache[path] = (key, body, heads)
        return body, heads
    except OSError:
        return None


DOCS_CSS = """
.ld-doc .ver{display:inline-block;padding:0 7px;border-radius:6px;border:1px solid currentColor;font-size:.8em;line-height:1.5;
vertical-align:.08em;opacity:.85;white-space:nowrap}
.ld-wrap{display:grid;grid-template-columns:230px minmax(0,1fr);gap:22px;align-items:start}
@media (max-width:860px){.ld-wrap{grid-template-columns:minmax(0,1fr)}}
.ld-toc{position:sticky;top:64px;background:rgba(10,12,34,.82);border:1px solid var(--ly-line);border-radius:8px;padding:14px 12px;margin:18px 0}
@media (max-width:860px){.ld-toc{position:static;margin:12px 0 0}}
.ld-toc p{margin:0 0 8px;font:600 11px/1 system-ui,sans-serif;letter-spacing:.14em;text-transform:uppercase;color:var(--ly-dim)}
.ld-toc ol{list-style:none;margin:0;padding:0}
.ld-toc li{margin:2px 0}
.ld-toc a{display:block;padding:5px 8px;border-radius:4px;color:var(--ly-ink);text-decoration:none;font-size:15px}
.ld-toc a:hover,.ld-toc a:focus-visible{background:rgba(40,52,110,.6);color:#fff}
.ld-toc a[aria-current]{background:rgba(40,52,110,.9);color:var(--ly-cyan);box-shadow:inset 2px 0 0 var(--ly-cyan)}
.ld-toc ol.ld-sub{margin:2px 0 6px 10px;border-left:1px solid var(--ly-line)}
.ld-toc ol.ld-sub a{font-size:13px;color:var(--ly-dim);padding:3px 8px}
.ld-doc{min-width:0;padding:26px 26px 30px}
@media (max-width:560px){.ld-doc{padding:20px 16px}}
.ld-doc h1{font-size:clamp(34px,6vw,52px);text-align:left;margin:0 0 16px}
.ld-doc h2{text-align:left;font-size:24px;margin:30px 0 10px}
.ld-doc h2::after{margin:6px 0 0;width:min(240px,60%)}
.ld-doc h3{font:600 19px/1.2 'Leyline Display','OW Cinzel',Impact,sans-serif;letter-spacing:.08em;text-transform:uppercase;color:var(--ly-cyan);margin:22px 0 8px}
.ld-doc h4{font-size:16px;color:var(--ly-violet);margin:18px 0 6px}
.ld-doc p,.ld-doc li{max-width:72ch}
.ld-doc ul,.ld-doc ol{padding-left:1.4rem}
.ld-doc li{margin:5px 0}
.ld-doc code{background:rgba(5,6,18,.85);border:1px solid var(--ly-line);border-radius:4px;padding:1px 5px;font:14px ui-monospace,SFMono-Regular,Menlo,monospace;overflow-wrap:anywhere}
.ld-doc pre{background:rgba(5,6,18,.9);border:1px solid var(--ly-line);border-radius:6px;padding:12px 14px;overflow-x:auto;max-width:100%}
.ld-doc pre code{background:none;border:0;padding:0;overflow-wrap:normal;white-space:pre}
.ld-doc blockquote{margin:14px 0;padding:10px 14px;border-left:3px solid var(--ly-violet);background:rgba(60,30,110,.25);border-radius:0 6px 6px 0}
.ld-doc blockquote p{margin:0}
.ld-doc strong{color:#fff}
.ld-doc hr{border:0;border-top:1px solid var(--ly-line);margin:24px 0}
.ld-scroll{overflow-x:auto;margin:12px 0;max-width:100%}
.ld-table{width:100%;border-collapse:collapse;font-size:15px}
.ld-table th{font:600 11px/1.2 system-ui,sans-serif;letter-spacing:.12em;text-transform:uppercase;color:var(--ly-dim);text-align:left;padding:9px 10px;border-bottom:1px solid var(--ly-line);white-space:nowrap}
.ld-table td{padding:9px 10px;border-bottom:1px solid rgba(120,150,255,.12);vertical-align:top}
.ld-foot{color:var(--ly-dim);font-size:14px;margin-top:26px}
.ld-gh{display:block;margin:12px 0 4px 8px;font:600 11px/1 system-ui,sans-serif;letter-spacing:.14em;text-transform:uppercase;color:var(--ly-dim)}
.ld-toc>.ld-toc-in>nav>ol>li:first-child .ld-gh{margin-top:4px}
.ld-find{display:flex;gap:6px;margin:0 0 10px}
.ld-find input{flex:1;min-width:0;padding:7px 9px;border-radius:5px;border:1px solid var(--ly-line);background:rgba(5,6,18,.85);color:var(--ly-ink);font:15px system-ui,sans-serif}
.ld-find button{padding:7px 10px;border-radius:5px;border:1px solid var(--ly-line);background:rgba(40,52,110,.6);color:var(--ly-ink);font:600 13px system-ui,sans-serif;cursor:pointer}
.ld-hits{list-style:none;padding:0;margin:14px 0}.ld-hits li{margin:0 0 16px}.ld-hits a{font-size:17px}.ld-hits small{display:block;color:var(--ly-dim);font-size:12px;text-transform:uppercase;letter-spacing:.1em}
.ld-hits p{margin:4px 0 0}.ld-hits mark{background:rgba(95,227,255,.25);color:inherit;padding:0 1px}
.ld-say{color:var(--ly-dim)}
""" + """
.ld-vh{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
.ld-toc-t{position:absolute;opacity:0;width:1px;height:1px;pointer-events:none}
.ld-toc-b{display:none}
@media (max-width:860px){
.ld-toc-b{display:flex;align-items:center;gap:8px;min-height:44px;padding:0 12px;border-radius:6px;cursor:pointer;font-weight:600;font-size:15px}
.ld-toc-b::before{content:"";width:9px;height:9px;border:solid currentColor;border-width:0 2px 2px 0;transform:rotate(45deg) translate(-2px,-2px);margin-right:4px}
.ld-toc-b b{font-weight:400;opacity:.75}
.ld-toc-t:checked+.ld-toc-b::before{transform:rotate(-135deg) translate(-2px,-2px)}
.ld-toc-t:focus-visible+.ld-toc-b{outline:2px solid currentColor;outline-offset:2px}
.ld-toc-in{display:none;margin-top:8px}.ld-toc-t:checked~.ld-toc-in{display:block}
.ld-toc{padding:6px!important}
}
"""


def _nav() -> str:
    """A stand-in nav for this page on its own; the portal swaps it for the site's nav and footer (site_chrome)."""
    return ('<nav class="ly-nav" aria-label="Leyline"><div class="grp">'
            '<a href="/wrath/leyline">Hub</a><a href="/wrath/modes">Modes</a><a href="/wrath/realms">Realms</a>'
            '<a href="/wrath/mods">Mods</a><a href="/wrath/docs">Docs</a></div>'
            '<div class="grp"><a href="/wrath/signin?next=%2Fwrath">Log in</a><a class="hot" href="/wrath">Play now</a></div></nav>')


def _search_form(q: str = "") -> str:
    """The docs search box: a plain GET form (no script needed), answered by /wrath/docs/search."""
    return (f'<form class="ld-find" role="search" action="{SEARCH}" method="get">'
            '<label class="ld-vh" for="ld-q">Search the docs</label>'
            f'<input id="ld-q" name="q" type="search" placeholder="Search the docs" value="{html.escape(q, quote=True)}" '
            'maxlength="80" autocomplete="off" enterkeyhint="search">'
            '<button type="submit">Search</button></form>')


def _toc(current: str, heads: list, q: str = "") -> str:
    """The contents: the search box, then the pages by group, the current page's sections under it. On a phone it
    folds behind an "All docs" button (a checkbox and its label: works with no script), so the page comes first."""
    groups = []
    for group, names in GROUPS:
        items = []
        for name in names:
            cur = ' aria-current="page"' if name == current else ""
            sub = ""
            if name == current:
                subs = [f'<li><a href="#{hid}">{inline(t, links=False)}</a></li>' for lvl, hid, t in heads if lvl == 2]
                sub = f'<ol class="ld-sub">{"".join(subs)}</ol>' if subs else ""
            items.append(f'<li><a href="{page_url(name)}"{cur}>{html.escape(TITLES[name])}</a>{sub}</li>')
        groups.append(f'<li class="ld-grp"><span class="ld-gh">{html.escape(group)}</span><ol>{"".join(items)}</ol></li>')
    here = TITLES.get(current, "Search" if current == "search" else "")
    return ('<aside class="ld-toc"><input type="checkbox" id="ld-toc-t" class="ld-toc-t">'
            f'<label for="ld-toc-t" class="ld-toc-b"><span>All docs</span>{(" <b>" + html.escape(here) + "</b>") if here else ""}</label>'
            f'<div class="ld-toc-in">{_search_form(q)}<nav aria-label="Documentation"><p>Leyline docs</p>'
            f'<ol>{"".join(groups)}</ol></nav></div></aside>')


# ------------------------------------------------------------------------------------------- search
# A small index of every page's sections (title, heading, a little of the text), built from the same markdown the
# pages show (switches applied), cached until a file or a switch changes. /wrath/docs/search answers a query on the
# server (no script needed); with script, the search page loads /wrath/docs/search.json once and answers as you type.
SNIPPET = 220
_index_cache: dict = {"key": None, "doc": None}


def _plain(md: str) -> str:
    t = re.sub(r"```.*?```", " ", md, flags=re.S)
    t = re.sub(r"\{\{v:[^}]*\}\}", " ", t)
    t = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", t)
    t = re.sub(r"^\s*[-*]\s+|^\s*\d+\.\s+", " ", t, flags=re.M)
    t = re.sub(r"[`*]+", "", t)
    t = re.sub(r"[>|#]+|-{3,}", " ", t)
    return re.sub(r"\s+", " ", t).strip()


def search_index() -> list[dict]:
    """[{p: page, t: page title, g: group, h: section heading, a: anchor, x: text}], one per section."""
    paths = [os.path.join(docs_dir(), n + ".md") for n, _ in PAGES]
    try:
        stamp = tuple(os.stat(p).st_mtime for p in paths if os.path.exists(p))
    except OSError:
        stamp = ()
    key = (stamp, switch_on("single_player"), switch_on("be_a_node"), os.environ.get("LEYLINE_BRAND", ""), docs_dir())
    with _lock:
        if _index_cache["key"] == key:
            return _index_cache["doc"]
    out = []
    for name, title in PAGES:
        try:
            with open(os.path.join(docs_dir(), name + ".md"), encoding="utf-8", errors="replace") as f:
                text = site_text(name, gated(f.read(MAX_BYTES)))
        except OSError:
            continue
        used: dict[str, int] = {}
        head, anchor, buf = (title if name != "index" else "Overview"), "", []
        def flush():
            body = _plain("\n".join(buf))
            if body or anchor:
                out.append({"p": name, "t": title, "g": GROUP_OF.get(name, ""), "h": _plain(head), "a": anchor, "x": body})
        fenced = False
        for line in text.split("\n"):
            if line.strip().startswith("```"):
                fenced = not fenced
            m = None if fenced else _HEAD.match(line.strip())
            if m:
                base = slug(m.group(2))
                n = used.get(base, 0)
                used[base] = n + 1
                if len(m.group(1)) in (2, 3):
                    flush()
                    head, anchor, buf = m.group(2), (base if n == 0 else f"{base}-{n + 1}"), []
                continue
            buf.append(line)
        flush()
    with _lock:
        _index_cache.update(key=key, doc=out)
    return out


def _words(q: str) -> list[str]:
    return [w for w in re.findall(r"[a-z0-9][a-z0-9'.-]*", q.lower())[:8] if len(w) >= 2 or w.isdigit()]


def search(q: str, limit: int = 30) -> list[dict]:
    """The sections that hold every word of q, best first: a word in the page title counts most, then the heading,
    then the text. Each hit carries its url and a snippet around the first word found in the text."""
    words = _words(q)
    if not words:
        return []
    hits = []
    for e in search_index():
        t, h, x = e["t"].lower(), e["h"].lower(), e["x"].lower()
        if not all(w in t or w in h or w in x for w in words):
            continue
        score = sum(6 * (w in t) + 4 * (w in h) + min(x.count(w), 5) for w in words)
        pos = min((x.find(w) for w in words if w in x), default=0)
        start = max(0, pos - 60)
        snip = e["x"][start:start + SNIPPET]
        hits.append((score, {"url": page_url(e["p"]) + (f"#{e['a']}" if e["a"] else ""), "title": e["t"], "group": e["g"],
                             "heading": e["h"] if e["h"] != e["t"] else "",
                             "snippet": ("…" if start else "") + snip + ("…" if start + SNIPPET < len(e["x"]) else "")}))
    hits.sort(key=lambda s: -s[0])
    return [h for _, h in hits[:limit]]


def _mark(text: str, words: list[str]) -> str:
    """Escaped text with the query's words in <mark>."""
    out = html.escape(text)
    for w in sorted(set(words), key=len, reverse=True):
        out = re.sub("(" + re.escape(html.escape(w)) + ")", r"<mark>\1</mark>", out, flags=re.I)
    return out


# The search page's script (it runs only on /wrath/docs/search, under the page's nonce): the index once, then the
# results as you type, with the same rules as search() above. textContent only; never innerHTML from the index.
SEARCH_JS = r"""
(function(){
var f=document.getElementById("ld-q"),list=document.getElementById("ld-hits"),say=document.getElementById("ld-say");
if(!f||!list||!window.fetch)return;var idx=null,t=0;
function words(q){return (q.toLowerCase().match(/[a-z0-9][a-z0-9'.-]*/g)||[]).slice(0,8).filter(function(w){return w.length>=2||/^\d+$/.test(w)});}
function count(s,w){var n=0,i=s.indexOf(w);while(i>=0&&n<5){n++;i=s.indexOf(w,i+w.length);}return n;}
function run(){var ws=words(f.value);list.textContent="";if(!ws.length){say.textContent="";return;}
var hits=[];idx.forEach(function(e){var t=e.t.toLowerCase(),h=e.h.toLowerCase(),x=e.x.toLowerCase();
if(!ws.every(function(w){return t.indexOf(w)>=0||h.indexOf(w)>=0||x.indexOf(w)>=0}))return;
var sc=0,pos=1e9;ws.forEach(function(w){sc+=6*(t.indexOf(w)>=0)+4*(h.indexOf(w)>=0)+count(x,w);var p=x.indexOf(w);if(p>=0&&p<pos)pos=p;});
if(pos===1e9)pos=0;var st=Math.max(0,pos-60);hits.push([sc,e,st]);});
hits.sort(function(a,b){return b[0]-a[0]});hits=hits.slice(0,30);
say.textContent=hits.length?(hits.length+(hits.length===30?"+":"")+" found"):"Nothing found. Try fewer or other words.";
hits.forEach(function(r){var e=r[1],li=document.createElement("li"),a=document.createElement("a");
a.href="/wrath/docs"+(e.p==="index"?"":"/"+e.p)+(e.a?"#"+e.a:"");var b=document.createElement("b");b.textContent=e.t;a.appendChild(b);
if(e.h&&e.h!==e.t){var s=document.createElement("span");s.textContent=" › "+e.h;a.appendChild(s);}
var g=document.createElement("small");g.textContent=e.g;var p=document.createElement("p");
p.textContent=(r[2]?"…":"")+e.x.substr(r[2],220)+(r[2]+220<e.x.length?"…":"");li.appendChild(a);li.appendChild(g);li.appendChild(p);list.appendChild(li);});}
function go(){clearTimeout(t);t=setTimeout(function(){if(idx)run();},80);}
fetch("/wrath/docs/search.json",{credentials:"omit"}).then(function(r){return r.json()}).then(function(d){idx=d.sections||[];f.addEventListener("input",go);if(f.value)run();}).catch(function(){});
})();
"""


def search_page(q: str, css: str = "", frame=None, nonce: str = "", meta: str = "") -> bytes:
    """/wrath/docs/search?q=: the results drawn here (no script needed); with script (nonce) they follow your typing."""
    q = (q or "")[:80]
    words = _words(q)
    hits = search(q) if words else []
    items = "".join(
        f'<li><a href="{html.escape(h["url"], quote=True)}"><b>{_mark(h["title"], words)}</b>'
        + (f'<span> › {_mark(h["heading"], words)}</span>' if h["heading"] else "")
        + f'</a><small>{html.escape(h["group"])}</small><p>{_mark(h["snippet"], words)}</p></li>' for h in hits)
    say = ("" if not words else f"{len(hits)}{'+' if len(hits) == 30 else ''} found" if hits
           else "Nothing found. Try fewer or other words.")
    body = ('<h1>Search the docs</h1>'
            '<p>Every page of the Leyline docs, searched on this site. Type a few words: "friend code", "lost key", '
            '"port", "invite".</p>'
            f'<p class="ld-say" id="ld-say" role="status">{html.escape(say)}</p><ol class="ld-hits" id="ld-hits">{items}</ol>'
            + (f'<script nonce="{html.escape(nonce, quote=True)}">{SEARCH_JS}</script>' if nonce else ""))
    return _shell("Search", "search", [], body, css, meta, frame, q=q)


def search_json() -> bytes:
    import json
    return json.dumps({"format": 1, "sections": search_index()}, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


# In the hub's frame (the portal's hub_frame: the site's side menu, tokens and footer): the same contents and table of
# contents on the hub's tokens, light and dark. The h1 is in Marcellus like every hub page (hub.STONE_CSS: .ld-doc h1).
DOCS_HUB_CSS = """
.ld-doc .ver{display:inline-block;padding:0 7px;border-radius:6px;border:1px solid var(--lk-border);color:var(--lk-muted);
font-size:.8em;line-height:1.5;vertical-align:.08em;white-space:nowrap}
.ld-wrap{display:grid;grid-template-columns:230px minmax(0,1fr);gap:22px;align-items:start;max-width:1180px;margin:8px auto 40px}
@media (max-width:860px){.ld-wrap{grid-template-columns:minmax(0,1fr);gap:14px}}
.ld-toc{position:sticky;top:16px;background:color-mix(in srgb,var(--lk-surface) 94%,transparent);border:1px solid var(--lk-border);
border-radius:var(--lk-r);padding:14px 10px}
@media (max-width:860px){.ld-toc{position:static}.ld-toc ol.ld-sub{display:none}}
.ld-toc p{margin:0 0 8px 8px;font-size:11.5px;font-weight:700;letter-spacing:.14em;text-transform:uppercase;color:var(--lk-muted)}
.ld-toc ol{list-style:none;margin:0;padding:0}
.ld-toc li{margin:1px 0}
.ld-toc a{display:block;padding:5px 10px;border-radius:var(--lk-r-xs);color:var(--lk-text-2)!important;text-decoration:none;font-size:14.5px}
.ld-toc a:hover,.ld-toc a:focus-visible{background:var(--lk-accent-soft);color:var(--lk-text)!important}
.ld-toc a[aria-current]{background:var(--lk-accent-soft);color:var(--lk-text)!important;font-weight:650;box-shadow:inset 2px 0 0 var(--lk-accent)}
.ld-toc ol.ld-sub{margin:2px 0 6px 12px;border-left:1px solid var(--lk-border)}
.ld-toc ol.ld-sub a{font-size:13px;color:var(--lk-muted)!important;padding:3px 10px}
.ld-doc{min-width:0;padding:26px 30px 30px;background:color-mix(in srgb,var(--lk-surface) 94%,transparent);border:1px solid var(--lk-border);
border-radius:var(--lk-r);box-shadow:0 14px 36px rgba(0,0,0,.25);color:var(--lk-text)}
@media (max-width:560px){.ld-doc{padding:20px 16px}}
.ld-doc h1{font-size:clamp(30px,5vw,42px);line-height:1.15;margin:0 0 16px;color:var(--lk-text)}
.ld-doc h2{font-size:23px;font-weight:700;margin:32px 0 10px;padding-bottom:6px;border-bottom:1px solid var(--lk-border);color:var(--lk-text)}
.ld-doc h3{font-size:18px;font-weight:700;margin:22px 0 8px;color:var(--lk-text)}
.ld-doc h4{font-size:16px;margin:18px 0 6px;color:var(--lk-text-2)}
.ld-doc :is(p,li){max-width:72ch;color:var(--lk-text-2)}
.ld-doc ul,.ld-doc ol{padding-left:1.4rem}
.ld-doc li{margin:5px 0}
.ld-doc code{background:var(--lk-surface-2);border:1px solid var(--lk-border);border-radius:5px;padding:1px 5px;font:13.5px ui-monospace,SFMono-Regular,Menlo,monospace;overflow-wrap:anywhere}
.ld-doc pre{background:var(--lk-surface-2);border:1px solid var(--lk-border);border-radius:var(--lk-r-sm);padding:12px 14px;overflow-x:auto;max-width:100%}
.ld-doc pre code{background:none;border:0;padding:0;overflow-wrap:normal;white-space:pre}
.ld-doc blockquote{margin:14px 0;padding:10px 14px;border-left:3px solid var(--lk-accent-2);background:var(--lk-accent-soft);border-radius:0 var(--lk-r-sm) var(--lk-r-sm) 0}
.ld-doc blockquote p{margin:0}
.ld-doc strong{color:var(--lk-text)}
.ld-doc hr{border:0;border-top:1px solid var(--lk-border);margin:24px 0}
.ld-scroll{overflow-x:auto;margin:12px 0;max-width:100%}
.ld-table{width:100%;border-collapse:collapse;font-size:14.5px}
.ld-table th{font-size:11.5px;font-weight:700;letter-spacing:.12em;text-transform:uppercase;color:var(--lk-muted);text-align:left;padding:9px 10px;
border-bottom:1px solid var(--lk-border-strong);white-space:nowrap}
.ld-table td{padding:9px 10px;border-bottom:1px solid var(--lk-border);vertical-align:top;color:var(--lk-text-2)}
.ld-foot{color:var(--lk-muted)!important;font-size:14px;margin-top:26px}
.ld-gh{display:block;margin:12px 0 4px 10px;font-size:11px;font-weight:700;letter-spacing:.14em;text-transform:uppercase;color:var(--lk-muted)}
.ld-toc>.ld-toc-in>nav>ol>li:first-child .ld-gh{margin-top:4px}
@media (max-width:860px){.ld-toc .ld-grp>ol{display:flex;flex-wrap:wrap;gap:2px 4px}}
.ld-find{display:flex;gap:6px;margin:0 0 10px}
.ld-find input{flex:1;min-width:0;padding:8px 10px;border-radius:var(--lk-r-xs);border:1px solid var(--lk-border);background:var(--lk-surface-2);color:var(--lk-text);font-size:15px}
.ld-find button{padding:8px 11px;border-radius:var(--lk-r-xs);border:1px solid var(--lk-border);background:var(--lk-accent-soft);color:var(--lk-text);font-weight:650;font-size:13px;cursor:pointer}
.ld-hits{list-style:none;padding:0;margin:14px 0}.ld-hits li{margin:0 0 18px}.ld-hits a{font-size:17px}
.ld-hits small{display:block;color:var(--lk-muted);font-size:11.5px;text-transform:uppercase;letter-spacing:.1em;margin-top:2px}
.ld-hits p{margin:4px 0 0}.ld-hits mark{background:var(--lk-accent-soft);color:var(--lk-text);padding:0 1px;border-radius:2px}
.ld-say{color:var(--lk-muted)!important}
.ld-vh{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
.ld-toc-t{position:absolute;opacity:0;width:1px;height:1px;pointer-events:none}
.ld-toc-b{display:none}
@media (max-width:860px){
.ld-toc-b{display:flex;align-items:center;gap:8px;min-height:44px;padding:0 12px;border-radius:var(--lk-r-xs);cursor:pointer;font-weight:650;font-size:15px;color:var(--lk-text)}
.ld-toc-b::before{content:"";width:8px;height:8px;border:solid currentColor;border-width:0 2px 2px 0;transform:rotate(45deg) translate(-2px,-2px);margin-right:4px}
.ld-toc-b b{font-weight:400;color:var(--lk-muted)}
.ld-toc-t:checked+.ld-toc-b::before{transform:rotate(-135deg) translate(-2px,-2px)}
.ld-toc-t:focus-visible+.ld-toc-b{outline:2px solid var(--lk-accent);outline-offset:2px}
.ld-toc-in{display:none;margin-top:8px}.ld-toc-t:checked~.ld-toc-in{display:block}
.ld-toc{padding:6px!important}
}
"""


def _shell(title: str, current: str, heads: list, body: str, css: str, meta: str, frame=None, q: str = "") -> bytes:
    """The page. frame: the portal's hub_frame-like callable (title, inner, css, meta) -> bytes or None; when it gives
    a page, that is the page (the hub's frame), else this module's own."""
    if frame is not None:
        try:
            out = frame(f"{title} · Leyline docs", f'<div class="ld-wrap">{_toc(current, heads, q)}'
                        f'<article class="ld-doc">{body}</article></div>', DOCS_HUB_CSS, meta)
            if out:
                return out
        except Exception:  # noqa: BLE001: this module's own page then
            pass
    return (f'<!doctype html><html lang="en"><head><meta charset="utf-8">'
            f'<meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="dark">'
            f'<title>{html.escape(title)} · Leyline docs</title>{meta}<style>{css}{DOCS_CSS}</style></head>'
            f'<body class="ly"><div class="ly-glow"></div>{_nav()}'
            f'<div class="ly-wrap"><div class="ld-wrap">{_toc(current, heads, q)}'
            f'<article class="ly-panel ld-doc">{body}</article></div></div></body></html>').encode("utf-8")


def not_found(css: str = "", frame=None) -> bytes:
    links = "".join(f'<li><a href="{page_url(n)}">{html.escape(TITLES[n])}</a></li>' for _g, names in GROUPS for n in names)
    body = ("<h1>Not found</h1><p>There is no docs page with that name. These are the pages:</p>"
            f"<ul>{links}</ul>")
    return _shell("Not found", "", [], body, css, "", frame)


def page(path: str, css: str = "", card_meta=None, realm_name: str = "WrathUnleashed", frame=None,
         query: dict | None = None, nonce: str = "") -> tuple[int, bytes]:
    """(status, HTML) for GET /wrath/docs and /wrath/docs/<page>. frame: the portal's hub frame (see _shell).
    /wrath/docs/search (query q; nonce: the page's script, if the portal allows one) and /wrath/docs/search.json are
    the search (see the portal: search.json is JSON, sent by the caller with its own type). Never raises."""
    try:
        if path.rstrip("/") == SEARCH:
            q = ((query or {}).get("q") or [""])[0]
            meta = ""
            if card_meta is not None:
                try:
                    meta = card_meta(SEARCH, "Search · Leyline docs")
                except Exception:  # noqa: BLE001
                    meta = ""
            return 200, search_page(q, css, frame, nonce, meta)
        if path == SEARCH_JSON:
            return 200, search_json()
        name = "index" if path == BASE else path[len(BASE) + 1:]
        if name not in TITLES:
            return 404, not_found(css, frame)
        got = load(name)
        if got is None:
            return 404, not_found(css, frame)
        body, heads = got
        title = TITLES[name] if name != "index" else "Leyline"
        meta = ""
        if card_meta is not None:
            try:
                meta = card_meta(page_url(name), f"{title} · Leyline docs")
            except Exception:  # noqa: BLE001: the page works without a preview card
                meta = ""
        foot = ('<p class="ld-foot">These pages are in the repo at docs/leyline. '
                '<a href="https://github.com/orionopenwow/Leyline/tree/main/docs/leyline" '
                'rel="noopener noreferrer">Source</a></p>')
        return 200, _shell(title, name, heads, body + foot, css, meta, frame)
    except Exception:  # noqa: BLE001: never take the portal down
        return 404, b"not found\n"
