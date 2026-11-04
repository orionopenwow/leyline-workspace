"""Leyline's downloads: one list of what a player can download, the same everywhere on the site (owner 2 Nov: "for the
download button on site and everywhere lets have options, like head to git to get zip, get the exe, etc").

    options(primary)       the rows, in order: Windows (Leyline.exe, recommended), the Windows installer, the portable
                           zip, macOS, Linux, GitHub (every release, the source), checksums
    panel(primary)         the small Download choice for a page: Download for Windows, Other downloads (a <details>:
                           no script, keyboard and screen reader friendly), All download options, the trust line
    page_body(primary)     /wrath/download: every option laid out, first-run help and Verify your download
    VERIFY_MD              the same Verify steps for docs/leyline/launcher.md (stream/tests/test_downloads.py keeps them alike)

What really exists comes from GitHub (tools/github-workflows/leyline-launcher.yml builds it): the rolling release
launcher-latest (Leyline.exe; Leyline-portable.zip and SHA256SUMS.txt from the next ship on) and the newest launcher-v*
release (setup .exe, .msi, .dmg, .AppImage, .deb; SHA256SUMS.txt from the next ship on). assets() reads both from
GitHub's public API in the background, at most every 10 minutes, and never blocks a page: until the first answer (or
with WU_DL_PROBE=0) the rows link Leyline.exe and the newest release's page, which really have those files, and a row
whose file GitHub doesn't list (the zip, the checksums) is left out. Sizes and SHA-256s are GitHub's own.

Standard library only; the portal loads this file from beside itself and runs without it. Nothing here raises.
"""
from __future__ import annotations

import html
import json
import os
import re
import threading
import time
import urllib.request

REPO = "https://github.com/orionopenwow/Leyline"
RELEASES = REPO + "/releases"
LATEST = RELEASES + "/latest"                      # the newest launcher-v release (the workflow marks it latest)
ROLLING = RELEASES + "/download/launcher-latest/"  # the rolling release: always the same addresses
EXE = ROLLING + "Leyline.exe"
SOURCE_ZIP = REPO + "/archive/refs/heads/main.zip"
PAGE = "/wrath/download"
VERIFY = PAGE + "#verify"
API = "https://api.github.com/repos/orionopenwow/Leyline"
TTL = 600            # seconds between two looks at GitHub
RETRY = 120          # after a failed look
TRUST = "Open source. Built on GitHub from the public code. Check the SHA-256 before you run it."

# file names on the rolling release -> kind
ROLLING_FILES = {"Leyline.exe": "exe", "Leyline-portable.zip": "zip", "SHA256SUMS.txt": "sums"}
# file names on a launcher-v release (tauri's bundle names) -> kind; .sig, latest.json and the updater's .app.tar.gz
# are for the launcher's own updater, not for people
RELEASE_FILES = ((re.compile(r"^Leyline_[\w.+-]+_x64-setup\.exe$"), "setup"), (re.compile(r"^Leyline_[\w.+-]+\.msi$"), "msi"),
                 (re.compile(r"^Leyline_[\w.+-]+\.dmg$"), "dmg"), (re.compile(r"^Leyline_[\w.+-]+\.AppImage$"), "appimage"),
                 (re.compile(r"^Leyline_[\w.+-]+\.deb$"), "deb"), (re.compile(r"^SHA256SUMS\.txt$"), "sums_rel"))
TAG_RE = re.compile(r"^launcher-v(\d+\.\d+\.\d+)$")
DIGEST_RE = re.compile(r"^sha256:([0-9a-f]{64})$")

_lock = threading.Lock()
_state = {"at": 0.0, "ok": False, "busy": False, "assets": {}, "version": "", "tag": ""}


# ---------------------------------------------------------------------------------------------- what GitHub has
def parse(rolling: dict | None, releases: list | None) -> dict:
    """{"assets": {kind: {name, url, size, sha256}}, "version", "tag"} from GitHub's answers for launcher-latest and
    the release list. Only https://github.com/orionopenwow/Leyline/releases/download/ links are kept."""
    out: dict = {}

    def one(a, tag):
        url = a.get("browser_download_url") if isinstance(a, dict) else None
        name = a.get("name") if isinstance(a, dict) else None
        if not (isinstance(url, str) and isinstance(name, str) and url == f"{RELEASES}/download/{tag}/{name}"):
            return None
        size = a.get("size")
        m = DIGEST_RE.match(str(a.get("digest") or ""))
        return {"name": name, "url": url, "size": size if isinstance(size, int) and 0 < size < 4_000_000_000 else 0,
                "sha256": m.group(1) if m else ""}

    for a in (rolling or {}).get("assets") or [] if isinstance(rolling, dict) else []:
        kind = ROLLING_FILES.get(a.get("name") if isinstance(a, dict) else None)
        got = one(a, "launcher-latest") if kind else None
        if got:
            out[kind] = got
    # the newest finished stable launcher-v release: the one with the Windows installer (macOS may come later)
    version = tag = ""
    for r in releases if isinstance(releases, list) else []:
        if not isinstance(r, dict) or r.get("draft") or r.get("prerelease"):
            continue
        m = TAG_RE.match(str(r.get("tag_name") or ""))
        files = {}
        for a in r.get("assets") or [] if m else []:
            name = a.get("name") if isinstance(a, dict) else None
            kind = next((k for rx, k in RELEASE_FILES if isinstance(name, str) and rx.match(name)), None)
            got = one(a, r["tag_name"]) if kind else None
            if got and kind not in files:
                files[kind] = got
        if "setup" in files:
            out.update(files)
            version, tag = m.group(1), r["tag_name"]
            break
    return {"assets": out, "version": version, "tag": tag}


def _get(url: str):
    req = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json", "User-Agent": "leyline-site-downloads"})
    with urllib.request.urlopen(req, timeout=8) as r:
        return json.loads(r.read(4_000_000).decode("utf-8"))


def _refresh() -> None:
    try:
        rolling = _get(API + "/releases/tags/launcher-latest")
        releases = _get(API + "/releases?per_page=10")
        doc, ok = parse(rolling, releases), True
    except Exception:   # noqa: BLE001: keep what we had, look again soon
        doc, ok = None, False
    with _lock:
        if doc is not None:
            _state.update(doc)
        _state.update(at=time.monotonic(), ok=ok or _state["ok"], busy=False, failed=not ok)


def set_assets(doc: dict) -> None:
    """Use GitHub's answers given here ({"rolling": <launcher-latest>, "releases": [<release>, ...]}), for tests and
    tools; the background look then waits a full TTL."""
    got = parse(doc.get("rolling"), doc.get("releases"))
    with _lock:
        _state.update(got, at=time.monotonic(), ok=True, busy=False, failed=False)


def assets() -> dict:
    """The current {"assets", "version", "tag"}; starts a background look at GitHub when due. Never blocks."""
    with _lock:
        age = time.monotonic() - _state["at"]
        due = (not _state["at"]) or age > (RETRY if _state.get("failed") else TTL)
        start = due and not _state["busy"] and os.environ.get("WU_DL_PROBE", "1") != "0"
        if start:
            _state["busy"] = True
        snap = {"assets": dict(_state["assets"]), "version": _state["version"], "tag": _state["tag"]}
    if start:
        threading.Thread(target=_refresh, name="downloads", daemon=True).start()
    return snap


# ---------------------------------------------------------------------------------------------- the rows
def size_text(n: int) -> str:
    if not n:
        return ""
    mb = n / 1048576
    return f"{mb:.1f} MB" if mb < 10 else f"{mb:.0f} MB"


def options(primary: str = "", doc: dict | None = None) -> list[dict]:
    """The download rows, in order: {key, title, line, links: [(label, href, size, file)], rec}. primary: the Windows
    download the site leads with (the hub's _download(): Leyline.exe, or the release the admin console published)."""
    doc = doc if doc is not None else assets()
    a = doc.get("assets") or {}
    primary = primary if isinstance(primary, str) and (primary == EXE or primary.startswith(RELEASES + "/")) else EXE
    exe = a.get("exe") or {}
    rows = [{"key": "windows", "title": "Windows", "rec": True,
             "line": "One file, nothing to install: download it and double-click. Windows 10 and 11.",
             "links": [("Leyline.exe", primary, size_text(exe.get("size", 0)) if primary == EXE else "", True)]}]

    def link(kind, label):
        f = a.get(kind)
        return (label, f["url"], size_text(f["size"]), True) if f else None

    inst = [x for x in (link("setup", "Setup .exe"), link("msi", ".msi")) if x]
    rows.append({"key": "installer", "title": "Windows installer", "rec": False,
                 "line": "Installs Leyline with a Start menu entry; remove it in Settings like any app.",
                 "links": inst or [("Setup .exe and .msi", LATEST, "", False)]})
    z = link("zip", "Leyline-portable.zip")
    if z:
        rows.append({"key": "zip", "title": "Portable zip", "rec": False,
                     "line": "Leyline.exe in a zip, for a USB stick or a folder of your choice.", "links": [z]})
    mac = link("dmg", ".dmg")
    rows.append({"key": "macos", "title": "macOS", "rec": False, "line": "One app for Apple Silicon and Intel.",
                 "links": [mac] if mac else [(".dmg", LATEST, "", False)]})
    lin = [x for x in (link("appimage", ".AppImage"), link("deb", ".deb")) if x]
    rows.append({"key": "linux", "title": "Linux", "rec": False, "line": ".AppImage runs on most systems; .deb for Debian and Ubuntu.",
                 "links": lin or [(".AppImage and .deb", LATEST, "", False)]})
    rows.append({"key": "github", "title": "GitHub", "rec": False, "line": "Every version with its notes, and the code Leyline is built from.",
                 "links": [("All releases", RELEASES, "", False), ("Source code", REPO, "", False),
                           ("Source as a ZIP", SOURCE_ZIP, "", True)]})
    sums = [x for x in (link("sums", "SHA256SUMS.txt for Leyline.exe"),
                        link("sums_rel", "SHA256SUMS.txt for the installers")) if x]
    rows.append({"key": "checksums", "title": "Checksums", "rec": False, "line": "The SHA-256 of every file, to check your download.",
                 "links": sums + [("How to verify", VERIFY, "", False)]})
    return rows


def _rel(href: str) -> str:
    return ' rel="noopener noreferrer"' if href.startswith("https://") else ""


DL_ICON = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v11"/><path d="m7 10 5 5 5-5"/><path d="M5 20h14"/></svg>'
CHEVRON = '<svg class="dlp-chev" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>'


def _chips(links, cls="dl-chip") -> str:
    e = html.escape
    out = []
    for label, href, size, _file in links:
        sz = f' <span class="dl-sz">{e(size)}</span>' if size else ""
        out.append(f'<a class="{cls}" href="{e(href)}"{_rel(href)}>{e(label)}{sz}</a>')
    return "".join(out)


def zip_button(doc: dict | None = None) -> str:
    """The portable zip as a second main button beside the .exe (owner 3 Nov: "have the zip as the main thing next to
    the exe in case people are deterred from the more info sequence"), or "" when the release has no zip."""
    doc = doc if doc is not None else assets()
    f = (doc.get("assets") or {}).get("zip")
    if not f:
        return ""
    e = html.escape
    sz = f' <span class="dl-sz">{e(size_text(f["size"]))}</span>' if f.get("size") else ""
    return f'<a class="ly-btn dlp-zip" href="{e(f["url"])}"{_rel(f["url"])}>{DL_ICON}<span>Download .zip</span>{sz}</a>'


def panel(primary: str = "", doc: dict | None = None, page_link: bool = True) -> str:
    """The small Download choice: Download for Windows (the recommended file) and Other downloads, a <details> list of
    every row (opens with Enter or Space, no script), All download options, the trust line."""
    e = html.escape
    rows = options(primary, doc)
    win = rows[0]["links"][0]
    sz = f" · {win[2]}" if win[2] else ""
    items = "".join(f'<li><span class="dlp-t">{e(r["title"])}</span><span class="dlp-l">{_chips(r["links"])}</span></li>'
                    for r in rows[1:])
    more = (f'<details class="dlp-more"><summary class="dlp-sum">Other downloads{CHEVRON}</summary>'
            f'<div class="dlp-menu"><ul class="dlp-list" aria-label="Other downloads">{items}</ul>'
            + (f'<p class="dlp-all"><a href="{PAGE}">All download options ›</a></p>' if page_link else "")
            + "</div></details>")
    zp = zip_button(doc)
    return (f'<div class="dlp"><div class="dlp-row"><a class="ly-btn dlp-main" href="{e(win[1])}"{_rel(win[1])}>{DL_ICON}'
            f'<span>Download for Windows</span></a>{zp}{more}</div>'
            f'<p class="dlp-meta">Leyline.exe{e(sz)}' + (" or the same in a .zip" if zp else "") + ' · also for macOS and Linux</p>'
            f'<p class="dlp-trust">{e(TRUST)}</p></div>')


# ---------------------------------------------------------------------------------------------- Verify your download
VERIFY_STEPS = (
    ("Windows", "Open Command Prompt in the folder with the file and run:", "certutil -hashfile Leyline.exe SHA256"),
    ("macOS and Linux", "Open a terminal in the folder with the file and run:", "shasum -a 256 Leyline*"),
)
VERIFY_INTRO = ("Every file is built on GitHub from the public code. Each release lists the SHA-256 of its files in "
                "SHA256SUMS.txt, and GitHub shows the same value next to each file on the release page. Hash your "
                "download and compare: if the two don't match, don't run it and download it again.")
VERIFY_MD = (VERIFY_INTRO + "\n\n" + "\n".join(f"- {who}: {how[0].lower() + how[1:-1]}: `{cmd}`" for who, how, cmd in VERIFY_STEPS)
             + "\n\nUse your file's name in the command (`sha256sum` works on Linux too).")


def verify_html(doc: dict | None = None) -> str:
    e = html.escape
    doc = doc if doc is not None else assets()
    a = doc.get("assets") or {}
    steps = "".join(f'<li><b>{e(who)}</b>: {e(how[0].lower() + how[1:])}<pre class="dl-cmd"><code>{e(cmd)}</code></pre></li>' for who, how, cmd in VERIFY_STEPS)
    sums = [(lbl, a[k]["url"]) for k, lbl in (("sums", "SHA256SUMS.txt for Leyline.exe and the zip"),
                                              ("sums_rel", "SHA256SUMS.txt for the installers")) if a.get(k)]
    sl = (" ".join(f'<a href="{e(u)}" rel="noopener noreferrer">{e(lbl)}</a>' for lbl, u in sums) + ". " if sums else "")
    table = ""
    hashed = [f for k in ("setup", "msi", "dmg", "appimage", "deb") if (f := a.get(k)) and f.get("sha256")]
    if hashed:
        ver = f" (Leyline {e(doc.get('version'))})" if doc.get("version") else ""
        table = (f'<details class="dl-sha"><summary>SHA-256 of each installer{ver}</summary><dl>'
                 + "".join(f'<dt>{e(f["name"])}</dt><dd><code>{e(f["sha256"])}</code></dd>' for f in hashed) + "</dl></details>")
    return (f'<section class="dl-verify" id="verify" aria-labelledby="dl-verify-h"><h2 id="dl-verify-h">Verify your download</h2>'
            f'<p>{e(VERIFY_INTRO)}</p><ol class="dl-steps">{steps}</ol>'
            f'<p class="fine">Use your file\'s name in the command (<code>sha256sum</code> works on Linux too). {sl}'
            f'Every release page shows each file\'s SHA-256 too: <a href="{LATEST}" rel="noopener noreferrer">the newest release</a>.</p>'
            f'{table}</section>')


# ---------------------------------------------------------------------------------------------- /wrath/download
def page_body(primary: str = "", doc: dict | None = None) -> str:
    """The inside of /wrath/download (the portal puts it in the hub's frame or the Leyline look)."""
    e = html.escape
    doc = doc if doc is not None else assets()
    rows = options(primary, doc)
    win = rows[0]["links"][0]
    ver = f'<p class="dl-ver">Leyline {e(doc["version"])}</p>' if doc.get("version") else ""
    sz = f" · {win[2]}" if win[2] else ""
    hero = (f'<h1>Download Leyline</h1><p class="sub">Free and open source. Pick the one for your computer.</p>{ver}'
            f'<div class="dl-rec"><div class="dl-rec-row"><a class="ly-btn dl-big" href="{e(win[1])}"{_rel(win[1])}>{DL_ICON}<span>Download for Windows</span></a>'
            f'{zip_button(doc)}</div>'
            f'<p class="dl-rec-m">Leyline.exe{e(sz)} · one file, nothing to install · Windows 10 and 11</p></div>'
            f'<p class="dl-trust">{e(TRUST)}</p>')
    lis = []
    for r in rows:
        badge = '<span class="dl-badge">Recommended</span>' if r["rec"] else ""
        lis.append(f'<li class="dl-opt" id="dl-{r["key"]}"><div class="dl-opt-h"><h3>{e(r["title"])}</h3>{badge}</div>'
                   f'<p>{e(r["line"])}</p><div class="dl-chips">{_chips(r["links"])}</div></li>')
    every = f'<section aria-labelledby="dl-all-h"><h2 id="dl-all-h">Every download</h2><ul class="dl-opts">{"".join(lis)}</ul></section>'
    first = ('<section class="dl-first" aria-labelledby="dl-first-h"><h2 id="dl-first-h">The first time you open it</h2><ul>'
             '<li><b>Windows</b> may say it protected your PC: choose More info, then Run anyway. Builds are not code-signed yet. '
             'The .zip holds the same Leyline.exe: unzip it anywhere and run it.</li>'
             '<li><b>macOS</b>: right-click Leyline and choose Open.</li>'
             '<li><b>Linux</b>: make the .AppImage executable, then run it.</li></ul>'
             '<p>After that Leyline keeps itself up to date. It uses the game folder you already have; it never downloads, '
             'offers or shares a game client.</p></section>')
    return f'<div class="dlpg">{hero}{every}{first}{verify_html(doc)}</div>'


CSS = """
.dlp{margin:14px 0 4px;max-width:36rem}
.dlp-row{display:flex;flex-wrap:wrap;align-items:flex-start;gap:10px}
.dlp-main{display:inline-flex;align-items:center;gap:8px;min-height:44px}
.dlp-zip{display:inline-flex;align-items:center;gap:8px;min-height:44px;background:var(--lk-surface-2,rgba(255,255,255,.08));
color:var(--lk-text,inherit);border:1px solid var(--lk-border-strong,rgba(255,255,255,.2))}
.dlp-zip svg{width:20px;height:20px;flex:none;fill:none;stroke:currentColor;stroke-width:2;stroke-linecap:round;stroke-linejoin:round}
.dlp-zip .dl-sz{opacity:.7;font-weight:400}
.dl-rec-row{display:flex;flex-wrap:wrap;gap:10px;align-items:center}
.dlp-main svg,.dl-big svg{width:20px;height:20px;flex:none;fill:none;stroke:currentColor;stroke-width:2;stroke-linecap:round;stroke-linejoin:round}
.dlp-more{position:relative;flex:1 1 15rem;min-width:0}
.dlp-sum{display:inline-flex;align-items:center;gap:6px;min-height:44px;padding:0 16px;box-sizing:border-box;cursor:pointer;list-style:none;
border:1px solid var(--lk-border-strong,rgba(255,255,255,.2));border-radius:var(--lk-r-sm,10px);background:var(--lk-surface-2,rgba(255,255,255,.05));
color:var(--lk-text,inherit);font-weight:600}
.dlp-sum::-webkit-details-marker{display:none}.dlp-sum::marker{content:""}
.dlp-sum:hover{border-color:var(--lk-accent,#7aa3f5)}
.dlp-sum:focus-visible,.dl-chip:focus-visible,.dlp-all a:focus-visible{outline:2px solid #f2c94c;outline-offset:3px}
.dlp-chev{width:16px;height:16px;fill:none;stroke:currentColor;stroke-width:2.2;stroke-linecap:round;stroke-linejoin:round;transition:transform .15s}
.dlp-more[open] .dlp-chev{transform:rotate(180deg)}
.dlp-menu{margin-top:8px;padding:10px 12px;border:1px solid var(--lk-border,rgba(255,255,255,.12));border-radius:var(--lk-r-sm,10px);
background:var(--lk-surface,#1e222b);box-shadow:0 12px 28px rgba(0,0,0,.35)}
.dlp-list{list-style:none;margin:0;padding:0}
.dlp-list li{display:grid;grid-template-columns:8.5rem minmax(0,1fr);gap:4px 10px;align-items:baseline;padding:7px 0;border-top:1px solid var(--lk-border,rgba(255,255,255,.1))}
.dlp-list li:first-child{border-top:0}
.dlp-t{font-weight:700;color:var(--lk-text,inherit);font-size:14px}
.dlp-l{display:flex;flex-wrap:wrap;gap:6px}
.dlp-all{margin:8px 0 0;font-size:14px}.dlp-all a{font-weight:700;color:var(--lk-accent,#7aa3f5)}
.dlp-meta{margin:8px 0 0;font-size:13.5px;color:var(--lk-muted,inherit)}
.dlp-trust,.dl-trust{margin:4px 0 0;font-size:13px;color:var(--lk-muted,inherit)}
.dl-chip{display:inline-flex;align-items:baseline;gap:6px;padding:5px 10px;border-radius:999px;font-size:13.5px;font-weight:600;text-decoration:none;
border:1px solid var(--lk-border-strong,rgba(255,255,255,.2));color:var(--lk-text,inherit);background:var(--lk-surface-2,rgba(255,255,255,.04));overflow-wrap:anywhere}
.dl-chip:hover{border-color:var(--lk-accent,#7aa3f5);background:var(--lk-accent-soft,rgba(91,141,239,.14))}
.dl-sz{font-weight:400;font-size:12.5px;color:var(--lk-muted,inherit);white-space:nowrap}
@media (min-width:700px){.dlp-menu{position:absolute;left:0;top:100%;z-index:30;width:min(30rem,calc(100vw - 48px));box-sizing:border-box}}
@media (max-width:560px){.dlp-list li{grid-template-columns:minmax(0,1fr)}.dlp-main,.dlp-sum{width:100%;justify-content:center}.dlp-more{flex-basis:100%}}
.dlpg h1{text-align:center}.dlpg .sub{text-align:center}
.dl-ver{margin:-8px 0 12px;text-align:center;font-size:13px;color:var(--lk-muted,inherit)}
.dl-rec{display:flex;flex-direction:column;align-items:center;gap:8px;margin:6px 0 4px}
.dl-big{display:inline-flex;align-items:center;justify-content:center;gap:10px;min-height:52px;padding:0 28px;font-size:18px}
.dl-rec-m{margin:0;text-align:center;font-size:14px;color:var(--lk-text-2,inherit)}
.dlpg .dl-trust{text-align:center;margin:8px 0 6px}
.dlpg h2{margin:26px 0 10px;font-size:19px}
.dl-opts{list-style:none;margin:0;padding:0;display:grid;gap:10px}
.dl-opt{padding:12px 14px;border:1px solid var(--lk-border,rgba(255,255,255,.12));border-radius:var(--lk-r-sm,10px);background:var(--lk-surface-2,rgba(255,255,255,.04));min-width:0}
.dl-opt:target{border-color:var(--lk-accent,#7aa3f5)}
.dl-opt-h{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
.dl-opt h3{margin:0;font-size:16px}
.dl-opt p{margin:4px 0 8px;font-size:14px;color:var(--lk-text-2,inherit)}
.dl-badge{font-size:11.5px;font-weight:700;letter-spacing:.04em;text-transform:uppercase;padding:2px 8px;border-radius:999px;
color:var(--lk-ok,#4fd18b);border:1px solid currentColor}
.dl-chips{display:flex;flex-wrap:wrap;gap:6px}
.dl-first ul,.dl-steps{margin:0;padding-left:20px}.dl-first li,.dl-steps li{margin:6px 0;font-size:14.5px}
.dl-cmd{margin:6px 0 2px;padding:8px 10px;border-radius:8px;background:var(--lk-bg,#15181f);border:1px solid var(--lk-border,rgba(255,255,255,.12));
overflow-x:auto;font-size:13.5px}
.dl-verify code,.dl-sha code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
.dl-sha{margin-top:10px}.dl-sha summary{cursor:pointer;font-weight:600}
.dl-sha dl{margin:8px 0 0}.dl-sha dt{font-size:13.5px;font-weight:700;margin-top:6px}.dl-sha dd{margin:0;font-size:12.5px;overflow-wrap:anywhere}
@media (prefers-reduced-motion:reduce){.dlp-chev{transition:none}}
"""
