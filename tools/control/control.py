#!/usr/bin/env python3
"""Leyline Control: the owner's one window for launching what Claude has ready.

    py -3 tools\\control\\control.py          (or double-click "Leyline Control.bat")

Serves a page on http://127.0.0.1:8097 (this PC only) and opens it. The page shows:
  * Live: the realm, Play page and game files answering (checked from this PC every 30 s).
  * Claude: what Claude is working on, what is waiting for you, what finished (control/claude-status.json,
    which Claude keeps up to date in this folder).
  * Ready to launch: the steps in UPDATES-PENDING.txt not yet in UPDATES-DONE.txt; "Launch all" runs them in
    order (the same steps UPDATE.bat runs) and streams each step's output live; it stops at a failure.
  * History: finished steps with their logs (logs/<id>-<step>.log).
  * Autopilot (switch in the header, remembered in control/autopilot.json): while this window is open, new
    steps Claude adds to UPDATES-PENDING.txt run on their own within a minute, and a read-only server check
    runs every 3 hours (logs/check-auto-*.log), so Claude can read the results without asking you. A failed
    step is not retried until Claude changes UPDATES-PENDING.txt. The owner's standing rules still apply.
Only this PC can open it: it listens on 127.0.0.1, and every action needs the session token in the page.
Standard library only.
"""
import hashlib
import hmac
import html
import http.server
import json
import os
import queue
import re
import secrets
import subprocess
import sys
import threading
import time
import urllib.parse
import urllib.request
import webbrowser
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]          # the workspace folder (Desktop\\Leyline)
PEND = ROOT / "UPDATES-PENDING.txt"
DONE = ROOT / "UPDATES-DONE.txt"
LOGS = ROOT / "logs"
STATUS = ROOT / "control" / "claude-status.json"
PORT = int(os.environ.get("WU_CONTROL_PORT", "8097"))
AUTO = ROOT / "control" / "autopilot.json"
LAST = ROOT / "control" / "last-run.json"
LAST_CHECK = ROOT / "control" / "last-check.json"    # the last server check: {at, id, ok} (Leyline HQ shows it)
HQ_URL = "https://play.openvanilla.app/wrath/admin"   # Leyline HQ: the one place online (stream/hq.py)
QUIET = "--quiet" in sys.argv                         # started at Windows sign-in: no browser tab
TOKEN_FILE = Path(os.environ.get("LOCALAPPDATA") or Path.home()) / "WrathUnleashed" / "control-token"
# The shared look (stream/www-files/art: wu-theme.css, the starfield, corners, dragons, divider, Cinzel), the same
# files the play pages and the GM console use. Served at /art/<name>, from this list only.
ART_DIR = Path(os.environ.get("WU_ART_DIR") or ROOT / "stream" / "www-files" / "art")
ART_TYPES = {
    "wu-theme.css": "text/css; charset=utf-8",
    "wu-load-orion.webp": "image/webp", "wu-load-bg.webp": "image/webp", "wu-card.webp": "image/webp",
    "wu-corner-tl.webp": "image/webp", "wu-corner-tr.webp": "image/webp",
    "wu-corner-bl.webp": "image/webp", "wu-corner-br.webp": "image/webp",
    "wu-divider.webp": "image/webp", "wu-dragon-l.webp": "image/webp", "wu-dragon-r.webp": "image/webp",
    "Cinzel-SemiBold.woff2": "font/woff2", "Cinzel-OFL.txt": "text/plain; charset=utf-8",
    # the Leyline look (owner 1 Nov): stone, the window edge, the stone button, Marcellus, the mark
    "stone-side.webp": "image/webp", "stone-bar.webp": "image/webp", "stone-btn.webp": "image/webp",
    "window-edge.png": "image/png", "marcellus-400.woff2": "font/woff2", "Marcellus-OFL.txt": "text/plain; charset=utf-8",
    "leyline-swirl-256.webp": "image/webp", "leyline-crystal-256.webp": "image/webp",
}


def art_bytes(name: str):
    """One file from the allow-list, or None. The name is only ever a key of ART_TYPES, never a path."""
    if name not in ART_TYPES:
        return None
    try:
        return (ART_DIR / name).read_bytes()
    except OSError:
        return None


def theme_version() -> str:
    b = art_bytes("wu-theme.css")
    return hashlib.sha256(b).hexdigest()[:10] if b else "none"


def _token() -> str:
    """One key per PC, kept outside the repo, so a reload, a bookmark or a second start keeps working."""
    try:
        t = TOKEN_FILE.read_text().strip()
        if len(t) >= 20:
            return t
    except OSError:
        pass
    t = secrets.token_urlsafe(18)
    try:
        TOKEN_FILE.parent.mkdir(parents=True, exist_ok=True)
        TOKEN_FILE.write_text(t)
    except OSError:
        pass
    return t


TOKEN = _token()

STEPS = {
    # Builds the browser client on this PC (client/build-browser-client.sh under Git Bash): tools once, benilla-wotlk
    # with client/patches/series.txt, the fat-LTO release build, packed into play\ like every build. Then "client" ships it.
    "client-build": ("client/Build browser client.bat", "Build the browser client on this PC (about an hour; then the client step)", False),
    "den-lite": ("tools/den-lite/Build Den Lite.bat", "Build Den Lite on this PC (the instant Den server, no Docker) for the launcher's Your Den", False),
    "client":  ("stream/Update browser client.bat", "New browser game build", False),
    "client-back": ("stream/Roll back browser client.bat", "Undo: the previous browser game build back (the server keeps one; running it again swaps back)", False),
    "signin":  ("stream/Update sign-in and warm-up.bat", "Play page and sign-in", False),
    "page":    ("stream/Update game page.bat", "Game page only (keeps the live game build)", False),
    "frontpage": ("tools/front-page/Front page as home - on.bat", "The site's home (/) opens the Leyline front page", False),
    "frontpage-off": ("tools/front-page/Front page as home - off.bat", "Undo: the site's home opens the game's sign-in page again", False),
    "ov112-check": ("tools/ov112-off/Turn off 1.12 jobs - dry run.bat", "1.12 Open Vanilla jobs: what would be turned off (read-only)", False),
    "ov112-off": ("tools/ov112-off/Turn off 1.12 jobs - do it.bat", "Turn off the 1.12 Open Vanilla jobs (crons, timers, containers; backup in /root)", False),
    # Vanilla Remastered (owner 2026-10-30 20:38: the 1.12 realm back as an optional official server, hosted by us
    # until a community host takes it over through the Leyline network; docs/VANILLA-REMASTERED.md). ov112-on starts
    # only the realm's own login and world servers (as vr-realmd / vr-mangosd, no published port); vr-beacon lists it.
    "ov112-realm-check": ("tools/ov112-off/Turn on the 1.12 realm - check.bat", "1.12 realm: what turning it on would do, and how the 1.12 site is reached (read-only)", False),
    "ov112-on": ("tools/ov112-off/Turn on the 1.12 realm.bat", "1.12 realm on: its login and world servers only (backup, self-check, rollback)", False),
    "ov112-realm-off": ("tools/ov112-off/Turn off the 1.12 realm.bat", "Undo: the 1.12 realm's login and world servers off again (kept)", False),
    "ov112-undo": ("tools/ov112-off/Turn 1.12 jobs back on.bat", "Undo ov112-off in full: every 1.12 job back as it was (Duels, Herald, timers too)", False),
    "vr-beacon": ("hosting/vanilla-remastered/Put Vanilla Remastered on Leyline.bat", "Vanilla Remastered on Leyline (the 1.12 realm as an official Beacon)", False),
    "vr-beacon-off": ("hosting/vanilla-remastered/Take Vanilla Remastered off Leyline.bat", "Take Vanilla Remastered off Leyline (the 1.12 realm keeps running)", False),
    # Vanilla Remastered levels 1-20 in the browser (owner 2026-10-31): the 1.12 game data back from the owner's own
    # client (upload, extract, the 1-20 slice, compact settings, archives deleted), and the OG 1.12 site at its own address.
    "vr-data-check": ("hosting/vanilla-remastered/Vanilla Remastered data - check.bat", "Vanilla Remastered data: this PC's 1.12 client, the server's disk and what is done (read-only)", False),
    "vr-data": ("hosting/vanilla-remastered/Vanilla Remastered data.bat", "Vanilla Remastered data: upload the 1.12 archives, extract, 1-20 browser slice, compact settings (long; resumable)", False),
    "vr-site-check": ("hosting/vanilla-remastered/Vanilla Remastered browser page - check.bat", "Vanilla Remastered in the browser: what the page step would do, its address and DNS (read-only)", False),
    "vr-site": ("hosting/vanilla-remastered/Vanilla Remastered browser page.bat", "Vanilla Remastered in the browser: the OG 1.12 site at its own address (backup, self-check, rollback)", False),
    "vr-site-off": ("hosting/vanilla-remastered/Vanilla Remastered browser page - off.bat", "Undo: the Vanilla Remastered browser page off (the realm keeps running)", False),
    "vr-test": ("hosting/vanilla-remastered/Vanilla Remastered self-test.bat", "Vanilla Remastered self-test: a fresh TEST account made, logged in, deleted", False),
    "ovsite": ("tools/launcher-site/Land Leyline-Launcher site.bat", "openvanilla.app: land the site and repo cleanup on Leyline-Launcher main (Pages publishes it)", False),
    "ghclean-check": ("tools/github-cleanup/GitHub cleanup - dry run.bat", "What the GitHub cleanup would delete (read-only)", False),
    "ghclean": ("tools/github-cleanup/GitHub cleanup - do it.bat", "GitHub cleanup: old releases and packages (owner's choices, 2026-10-30)", False),
    "scripts": ("realm/Server pass - finder 10 and bug reports.bat", "Realm scripts (live reload)", False),
    "realm":   ("realm/Server pass - finder 10 and bug reports.bat", "Realm settings (5-min restart)", False),
    "foreverui": ("stream/Add Forever UI to the stream.bat", "Forever UI look (menus and character select)", False),
    "slice":   ("stream/Make mini slice.bat", "Streamed game files (30-60 min)", False),
    # docs/INCIDENT-2026-11-01-SLICE.md: the streamed slice's files on the server, checked (read-only)
    "slice-health": ("stream/Slice health - check.bat", "Read-only: is the streamed slice whole (key files, its tier lists, what wu-play answers)", False),
    "ship":    ("Ship to GitHub.bat", "Save everything to GitHub", True),
    "ship-again": ("tools/ship/Save again as one fresh commit.bat", "Save to GitHub again as one fresh commit (after GitHub refused a save)", False),
    "check":   ("realm/Server check.bat", "Server check (read-only)", False),
    "gm":      ("gmconsole/Install GM Console.bat", "GM console update (game keeps running)", False),
    "bots-off": ("realm/Bots - none.bat", "No roaming bots on Wrath Unleashed (5-min warning, then a restart)", False),
    "bots-on": ("realm/Bots - back on.bat", "Undo: the roaming bots back on Wrath Unleashed (5-min warning, then a restart)", False),
    "lean":    ("realm/Lean server.bat", "Lean server: fewer bots, idle areas unload (5-min restart)", False),
    "relay": ("hosting/relay/Deploy relay.bat", "Beacons relay (the relay's site, its port book kept)", False),
    # The move to leylinelauncher.com (owner 1 Nov 2026; docs/DOMAIN-MOVE.md): the site, play. for the game, relay.
    # for the relay. Old names keep working forever (page loads 301; APIs, /data, /ws and tunnels answer as before).
    # Order: Squarespace DNS -> domain-check -> Google/Discord redirect URIs -> domain-on -> test -> launcher release.
    "domain-check": ("tools/domain/Domain move - check.bat", "Domain move: the server IP, DNS for the four new names, ports 80/443 and the Squarespace records to add (read-only)", False),
    "domain-on": ("tools/domain/Domain move - on.bat", "Domain move ON: leylinelauncher.com live, self-checked, then old page loads 301 (rollback on failure)", False),
    "domain-off": ("tools/domain/Domain move - off.bat", "Undo the domain move: every name back as it was", False),
    "hangout": ("hosting/hangout/Deploy Dalaran hangout.bat", "Wrath Unleashed on Leyline (the realm as a Beacon)", False),
    "hangout-off": ("hosting/hangout/Take Dalaran hangout off Leyline.bat", "Take Wrath Unleashed off Leyline", False),
    # The Builders' Realm (owner 2026-10-31: an official realm just for building, open to approved builders;
    # docs/BUILD-REALM.md): a second small world server beside the main realm, which is never restarted.
    "build-realm-check": ("hosting/build-realm/Builders realm - check.bat", "Builders' Realm: memory, disk and what turning it on would do (read-only)", False),
    "build-realm-on": ("hosting/build-realm/Builders realm - on.bat", "Builders' Realm on: backup, its own databases and servers, self-check, on Leyline (rollback on failure)", False),
    "build-realm-off": ("hosting/build-realm/Builders realm - off.bat", "Builders' Realm off: off Leyline, a 5-minute countdown for builders online, stopped (data kept)", False),
    "build-realm-saves": ("hosting/build-realm/Builders realm - saves.bat", "Builders' Realm saves: copy them to this PC, send the files in its send folder", False),
    "leyline": ("tools/leyline-repo/Update Leyline repo.bat", "Leyline repo: copy and push (github.com/orionopenwow/Leyline)", False),
    "mods":  ("mods/Put mods on Wrath.bat", "Mods catalog: realm (live reload), streamed addons and /wrath/mods", False),
    # Resilience (docs/RESILIENCE.md): after a ship, our own copy of each Leyline release, the Host pack, the addons and
    # the upstream server source at our pinned commits, on our server with checksums and torrents (and, with the release
    # key on this PC, the signed release log), so launchers keep updating and people keep hosting if GitHub is down.
    "release-mirror": ("tools/release-mirror/Mirror releases.bat", "Release mirror: copy the new Leyline releases, Host pack, addons and server source to our server (after a ship)", False),
    "release-mirror-check": ("tools/release-mirror/Mirror releases - check.bat", "Release mirror: what it would copy (read-only)", False),
    "hostkit": ("hosting/kit/Publish host kit.bat", "Host kit download (checked zip on the public Leyline release)", False),
    # The Leyline addons (addons/pack/make-addonpack.py): our in-game addons as a checked zip that the launcher installs
    # into players' own 3.3.5a folders. Run it after changing WrathUnleashedUI, OVArmory, OVQoL or the mods catalog.
    "addons": ("addons/pack/Publish Leyline addons.bat", "Leyline addons download (our in-game addons for desktop players, on the public Leyline release)", False),
    "sampler-check": ("realm/Sample hangout - check.bat", "Leyline Sample Hangout: what would change (read-only)", False),
    "sampler": ("realm/Sample hangout - on.bat", "Leyline Sample Hangout on: lighter realm, browser fence (5-min restart)", False),
    "sampler-off": ("realm/Sample hangout - off.bat", "Leyline Sample Hangout off: back to the full realm (5-min restart)", False),
    "slice-sampler": ("stream/Make sampler slice.bat", "Sample Hangout streamed game files (30-60 min)", False),
    # The Leyline brand switch (realm/brand.sh; owner 2026-10-30: "wait for the site to change on my command"). Only
    # when the owner says so. No restart, nobody kicked; brand-off is the rollback.
    "brand-check": ("realm/Leyline brand - check.bat", "Leyline brand: what would change (read-only)", False),
    "brand-leyline": ("realm/Leyline brand - on.bat", "Leyline brand ON: site, game and realm list say Leyline / Wrath Unleashed", False),
    "brand-off": ("realm/Leyline brand - off.bat", "Leyline brand off: every name back as it was", False),
    # The hub's preview thumbnails (tools/hub-index, docs/HUB-INDEX.md): top 30 and new entries, then ship + signin.
    "thumbs": ("tools/hub-index/Refresh hub thumbnails.bat", "Hub thumbnails: preview images for the top 30 and new catalog entries (then ship)", False),
    # The launcher Home gallery's in-game screenshots (owner 2026-10-30): from site-art\home\ on this PC to the website's
    # art dir (/wrath/art/home/, with home.json), backup first; never into the repo. tools/site-art/publish_home_art.py.
    "homeart": ("tools/site-art/Publish home screenshots.bat", "Home gallery screenshots: site-art\\home to the website (the launcher reads them)", False),
    "builds-why": ("tools/github-builds/Why did the build fail.bat", "Why the last GitHub build failed (read-only)", False),
    "remove-files": ("tools/cleanup/Remove listed files.bat", "Remove the files a change took out (tools/cleanup/REMOVE.txt)", False),
    "fresh-check": ("tools/fresh-history/Fresh history - dry run.bat", "Fresh Leyline history: what it would do (read-only)", False),
    "fresh-history": ("tools/fresh-history/Fresh history - do it.bat", "Fresh Leyline history: one commit, old history to a Desktop backup", False),
    # ovrename / ovland (the OpenVanilla repo's rename to Leyline-Launcher, 2026-10-29) are done for good: the tool is in
    # labs/done/ov-rename/ (labs/README.md says how to run it again).
    # The game data killswitch (docs/GAME-DATA-ON-OUR-SERVER.md): only when the owner asks for it.
    "gamedata-off": ("stream/Game data killswitch ON.bat", "Game data OFF: our server serves no game data (players use their own game folder)", False),
    "gamedata-on": ("stream/Game data killswitch OFF.bat", "Game data back ON: stream the slim sampler again", False),
    # Leyline HQ (docs/ADMIN-CONSOLE.md, "What the PC sends"): one bat, three names. It uploads the last deploys
    # (step names and outcomes only) and Claude's notes (control/hq-notes.json), and downloads the owner's notes to
    # Claude into control/notes-from-owner.json. This window also runs it by itself after every run and with the
    # 3-hourly server check.
    "firewall-check": ("tools/security/Firewall limits - dry run.bat", "Firewall limits: what would be added (read-only)", False),
    "firewall": ("tools/security/Firewall limits - apply.bat", "Firewall limits on game, relay and web ports + fail2ban for wrong passwords", False),
    "addons-curate": ("tools/addon-curate/Curate addons.bat", "Addon marketplace: curate upstream addons from their authors (new ones wait for review; then ship + signin)", False),
    "tidy-check": ("tools/server-tidy/Server tidy - dry run.bat", "Read-only: what making room on the server would remove, and how much", False),
    "tidy": ("tools/server-tidy/Server tidy.bat", "Make room on the server: build cache, old logs, old site backups, the parked VR browser slice (owner OK 1 Nov)", False),
    "ov112-data-check": ("tools/ov112-data/Old 1.12 data - check.bat", "Read-only: where the old 1.12 game files are on the server, their size and who serves them", False),
    "ov112-data-remove-check": ("tools/ov112-data/Old 1.12 data - remove (dry run).bat", "Dry run: what removing the old 1.12 game files would do", False),
    "ov112-data-remove": ("tools/ov112-data/Old 1.12 data - remove.bat", "Remove the old 1.12 game files (the old site answers 410 for /data first; Wrath untouched)", False),
    "worldlog": ("tools/diag/World server log.bat", "Read-only: why the world server restarted (state, recent output, crash dumps)", False),
    "caddy-check": ("tools/security/Caddy check.bat", "Caddy and compose check: live config to the PC, secrets removed (read-only)", False),
    # The bugs manifest (docs/BUGS-MANIFEST.md): every bug report (game, site, launcher, Discord) in one de-duplicated
    # backlog. Uploads tools/bugs/status.json, has the GM console rebuild, copies BUGS-MANIFEST.md and backlog.json to
    # tools/bugs/live/ (git-ignored). Touches only the bug backlog, so Autopilot may run it (READ_ONLY below).
    "bugs": ("tools/bugs/Pull bug manifest.bat", "Bug manifest: every bug report in one list, copied to tools\\bugs\\live (statuses from tools\\bugs\\status.json go up first)", False),
    "hq-sync": ("tools/hq/HQ sync.bat", "Leyline HQ: send the recent changes and Claude's notes, fetch the owner's notes", False),
    "hqnote": ("tools/hq/HQ sync.bat", "Leyline HQ: publish Claude's notes (and sync)", False),
    "hq-inbox": ("tools/hq/HQ sync.bat", "Leyline HQ: fetch the owner's notes to Claude (and sync)", False),
}
# Steps that change nothing in production. With "Ask before deploys" on (control/autopilot.json "confirm"),
# Autopilot runs only these by itself; anything else waits for "Launch all".
READ_ONLY = {"check", "ov112-check", "ov112-realm-check", "build-realm-check", "ghclean-check", "sampler-check", "brand-check", "builds-why",
             "fresh-check", "hq-sync", "hqnote", "hq-inbox", "firewall-check", "caddy-check", "ov112-data-check", "ov112-data-remove-check", "worldlog", "tidy-check",
             "vr-data-check", "vr-site-check", "release-mirror-check", "bugs", "domain-check"}
HQ_STEPS = {"hq-sync", "hqnote", "hq-inbox"}

# Never in a log or on the page: passwords, tokens, keys, connection strings (scrub() on every line a step prints).
SECRET_RES = [
    (re.compile(r"(?i)\b([A-Z0-9_]*(?:PASSWORD|PASSWD|PASS|TOKEN|SECRET|API_?KEY|PRIVATE_?KEY)(?:_[A-Z0-9_]*)?\s*[=:]\s*)(\"[^\"]*\"|'[^']*'|\S+)"), r"\1[hidden]"),
    (re.compile(r"\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}"), "[hidden]"),
    (re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}"), "[hidden]"),
    (re.compile(r"(?i)\b(Bearer|Basic)\s+[A-Za-z0-9._~+/=-]{12,}"), r"\1 [hidden]"),
    (re.compile(r"(?i)\b((?:mysql|postgres(?:ql)?|mongodb|redis|amqp|https?)://[^:/\s@]+:)[^@\s]+@"), r"\1[hidden]@"),
    (re.compile(r"(?i)(DatabaseInfo\s*=\s*\"?[^;\"]*;[^;\"]*;[^;\"]*;)[^;\"]*"), r"\1[hidden]"),
    (re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----.*"), "[hidden private key]"),
]


def scrub(line: str) -> str:
    for rx, rep in SECRET_RES:
        line = rx.sub(rep, line)
    return line
CHECKS = [
    ("Play page", "https://play.openvanilla.app/wrath"),
    ("Game files", "https://wrath.openvanilla.app/data/__tier/index.json"),
    ("Client", "https://wrath.openvanilla.app/boot-manifest.json"),
]

state = {"running": None, "lines": [], "health": {}, "health_at": 0, "auto_failed": None}
lock = threading.Lock()
listeners: list[queue.Queue] = []


def read_lines(p: Path) -> list[str]:
    try:
        return [l.strip() for l in p.read_text(encoding="utf-8-sig", errors="replace").splitlines()]
    except FileNotFoundError:
        return []


def pending() -> list[dict]:
    done = set(read_lines(DONE))
    out = []
    for line in read_lines(PEND):
        if not line or line.startswith("#"):
            continue
        parts = line.split(None, 2)
        if len(parts) < 2 or parts[0] in done:
            continue
        sid, step = parts[0], parts[1].lower()
        note = parts[2] if len(parts) > 2 else ""
        title = STEPS.get(step, (None, "Unknown step", False))[1]
        out.append({"id": sid, "step": step, "title": title, "note": note, "known": step in STEPS})
    return out


def history() -> list[dict]:
    done = read_lines(DONE)
    notes = {}
    for line in read_lines(PEND):
        parts = line.split(None, 2)
        if len(parts) >= 2:
            notes[parts[0]] = (parts[1], parts[2] if len(parts) > 2 else "")
    out = []
    for sid in reversed([d for d in done if d]):
        step, note = notes.get(sid, ("?", ""))
        log = LOGS / f"{sid}-{step}.log"
        out.append({"id": sid, "step": step, "note": note, "log": log.name if log.exists() else ""})
    return out[:30]


def claude_status() -> dict:
    try:
        return json.loads(STATUS.read_text(encoding="utf-8-sig"))
    except Exception:
        return {"updated": "", "now": [], "waiting_on_you": [], "done": [], "note": "No status from Claude yet."}


def _auto_doc() -> dict:
    try:
        d = json.loads(AUTO.read_text(encoding="utf-8-sig"))
        return d if isinstance(d, dict) else {}
    except Exception:
        return {}


def autopilot_on() -> bool:
    return bool(_auto_doc().get("on"))


def confirm_on() -> bool:
    """"Ask before deploys" (default off): Autopilot runs read-only steps by itself, the rest wait for Launch all."""
    return bool(_auto_doc().get("confirm"))


def set_autopilot(on: bool | None = None, confirm: bool | None = None) -> None:
    d = _auto_doc()
    if on is not None:
        d["on"] = bool(on)
    if confirm is not None:
        d["confirm"] = bool(confirm)
    d["changed"] = time.strftime("%Y-%m-%d %H:%M:%S")
    AUTO.parent.mkdir(exist_ok=True)
    AUTO.write_text(json.dumps({"on": bool(d.get("on")), "confirm": bool(d.get("confirm")), "changed": d["changed"]}),
                    encoding="utf-8")


def pending_stamp() -> float:
    try:
        return PEND.stat().st_mtime
    except OSError:
        return 0.0


def write_last(ids, ok, step=None) -> None:
    try:
        LAST.parent.mkdir(exist_ok=True)
        LAST.write_text(json.dumps({"at": time.strftime("%Y-%m-%d %H:%M:%S"), "ids": ids, "ok": ok,
                                    "failed_step": step, "autopilot": autopilot_on()}, indent=1), encoding="utf-8")
    except OSError:
        pass


def start_run(msg: str, items=None, record=True, title="Starting") -> bool:
    with lock:
        if state["running"]:
            return False
        state["running"] = {"id": "", "step": "starting", "title": title}
        state["lines"] = []
    threading.Thread(target=run_all, args=(msg, items, record), daemon=True).start()
    return True


SELF = Path(__file__).resolve()
try:
    SELF_MTIME = SELF.stat().st_mtime
except OSError:
    SELF_MTIME = 0.0


def restart_if_updated() -> None:
    """When Claude ships a new control.py, the window restarts itself (minimised, Autopilot as it was) once idle."""
    try:
        if SELF.stat().st_mtime == SELF_MTIME or state["running"]:
            return
    except OSError:
        return
    emit("line", "== The control window was updated: restarting it (a few seconds).")
    flags = getattr(subprocess, "CREATE_NEW_CONSOLE", 0)
    subprocess.Popen([sys.executable, str(SELF), "--quiet", "--wait-port"], cwd=str(ROOT), creationflags=flags)
    time.sleep(0.5)
    os._exit(0)


def autopilot_loop() -> None:
    """Runs Claude's new steps on their own while the switch is on; a server check every 3 hours."""
    next_check = time.time() + 300
    while True:
        time.sleep(30)
        restart_if_updated()
        if not autopilot_on():
            continue
        try:
            items = pending()
            failed = state.get("auto_failed")
            if confirm_on():   # "Ask before deploys": only the read-only steps at the front run by themselves
                safe = []
                for it in items:
                    if it["step"] not in READ_ONLY:
                        break
                    safe.append(it)
                if items and not safe and state.get("asked") != pending_stamp():
                    state["asked"] = pending_stamp()
                    emit("line", "== Ask before deploys is on: the next step waits for you (Launch all).")
                items = safe
            if items and not (failed and failed == pending_stamp()):
                start_run("", items, title="Autopilot")
            elif time.time() >= next_check:
                next_check = time.time() + 3 * 3600
                item = {"id": "check-auto-" + time.strftime("%Y%m%d-%H%M%S"), "step": "check",
                        "title": STEPS["check"][1], "note": "", "known": True}
                start_run("", [item], False, "Autopilot server check")
        except Exception as e:                  # never let the loop die
            emit("line", f"!! autopilot: {e}")


def emit(kind: str, data) -> None:
    msg = json.dumps({"kind": kind, "data": data})
    with lock:
        if kind == "line":
            state["lines"].append(data)
            state["lines"] = state["lines"][-2000:]
        for q in list(listeners):
            q.put(msg)


def run_all(ship_message: str, items=None, record=True) -> None:
    ran, started = [], []
    try:
        LOGS.mkdir(exist_ok=True)
        queue = list(items if items is not None else pending())
        i = 0
        while i < len(queue):
            item = queue[i]
            i += 1
            if not item["known"]:
                emit("line", f"!! Unknown step '{item['step']}' ({item['id']}): ask Claude.")
                emit("result", {"ok": False, "step": item["step"]})
                state["auto_failed"] = pending_stamp()
                write_last([item["id"]], False, item["step"])
                return
            bat, title, interactive = STEPS[item["step"]]
            with lock:
                state["running"] = item
            started.append(item["step"])
            emit("step", item)
            emit("line", f"===== {title} ({item['id']}) =====")
            # The step's own "press any key" lines get an empty answer. Nothing is answered through stdin (git can
            # swallow it); Ship to GitHub asks nothing and gets the hands-free switch (LEYLINE_SHIP_AUTO, and the older
            # WU_SHIP_AUTO) plus the owner's note, if he typed one (LEYLINE_SHIP_MSG / WU_SHIP_MSG); without a note
            # it commits as "Leyline update <date> <time>". Empty lines only clear "press any key".
            feed = "\n" * 8
            env = dict(os.environ)
            if interactive:
                env["WU_SHIP_AUTO"] = env["LEYLINE_SHIP_AUTO"] = "1"
                note = (ship_message or "").replace("!", "").replace("%", "").replace('"', "'").strip()
                for k in ("WU_SHIP_MSG", "LEYLINE_SHIP_MSG"):
                    env.pop(k, None)
                if note:
                    env["WU_SHIP_MSG"] = env["LEYLINE_SHIP_MSG"] = note
            log = LOGS / f"{item['id']}-{item['step']}.log"
            with open(log, "w", encoding="utf-8") as lf:
                p = subprocess.Popen(["cmd", "/c", "call", str(ROOT / bat)], cwd=str(ROOT), env=env,
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                     creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
                try:
                    p.stdin.write(feed.encode())
                    p.stdin.close()
                except OSError:
                    pass
                for raw in iter(p.stdout.readline, b""):
                    line = scrub(raw.decode("utf-8", errors="replace").rstrip("\r\n"))
                    if line.strip().lower().startswith("press any key"):
                        continue
                    lf.write(line + "\n")
                    emit("line", line)
                rc = p.wait()
            if item["step"] == "check":
                write_last_check(item["id"], rc == 0)
            if rc != 0:
                emit("line", f"!! {title} failed (exit {rc}). Send Claude a screenshot, or the log: logs/{log.name}")
                emit("result", {"ok": False, "step": item["step"]})
                if record:
                    state["auto_failed"] = pending_stamp()
                write_last([item["id"]], False, item["step"])
                return
            if record:
                with open(DONE, "a", encoding="utf-8") as d:
                    d.write(item["id"] + "\n")
            emit("line", f"== {title}: done")
            ran.append(item["id"])
            # Every save to GitHub (the private workspace repo) also refreshes the public Leyline copy (owner
            # 2026-10-25: "make sure it works with ... all changes we make"), unless a Leyline step
            # is already queued after it. A Leyline failure stops the run like any other step.
            if item["step"] == "ship" and not any(q["step"] == "leyline" for q in queue[i:]):
                queue.insert(i, {"id": item["id"] + "-leyline", "step": "leyline",
                                 "title": STEPS["leyline"][1], "note": "follows Save to GitHub",
                                 "known": True})
        emit("result", {"ok": True})
        if ran:
            write_last(ran, True)
    finally:
        if started and not set(started) <= HQ_STEPS:
            hq_sync_quiet()
        with lock:
            state["running"] = None
        emit("state", {})


def write_last_check(sid: str, ok: bool) -> None:
    try:
        LAST_CHECK.parent.mkdir(exist_ok=True)
        LAST_CHECK.write_text(json.dumps({"at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "id": sid, "ok": bool(ok)}),
                              encoding="utf-8")
    except OSError:
        pass


def hq_sync_quiet() -> None:
    """After a run: tell Leyline HQ what changed (tools/hq/HQ sync.bat). Never fails the run; its output goes to
    logs/hq-sync-auto.log only."""
    bat = ROOT / STEPS["hq-sync"][0]
    if not bat.exists() or os.name != "nt":
        return
    try:
        LOGS.mkdir(exist_ok=True)
        with open(LOGS / "hq-sync-auto.log", "w", encoding="utf-8") as lf:
            p = subprocess.run(["cmd", "/c", "call", str(bat)], cwd=str(ROOT), stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=180,
                               creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
            for line in p.stdout.decode("utf-8", errors="replace").splitlines():
                lf.write(scrub(line) + "\n")
        emit("line", "== Leyline HQ updated" if p.returncode == 0 else "== Leyline HQ sync did not finish (logs/hq-sync-auto.log)")
    except Exception as e:   # noqa: BLE001
        emit("line", f"== Leyline HQ sync skipped: {type(e).__name__}")


def check_health() -> None:
    while True:
        res = {}
        for name, url in CHECKS:
            t0 = time.time()
            try:
                req = urllib.request.Request(url, headers={"User-Agent": "WrathUnleashed-Control"})
                with urllib.request.urlopen(req, timeout=10) as r:
                    res[name] = {"ok": r.status < 400, "ms": int((time.time() - t0) * 1000), "code": r.status}
            except urllib.error.HTTPError as e:
                ok = e.code in (401, 403, 410)     # the gate answering = up; 410 = game data switched off on purpose
                res[name] = {"ok": ok, "ms": int((time.time() - t0) * 1000), "code": e.code}
            except Exception as e:
                res[name] = {"ok": False, "ms": None, "code": type(e).__name__}
        with lock:
            state["health"], state["health_at"] = res, time.time()
        emit("state", {})
        time.sleep(30)


def token_ok(t: str) -> bool:
    return bool(t) and hmac.compare_digest(t.encode(), TOKEN.encode())


def cookie_token(cookie: str) -> str:
    for part in (cookie or "").split(";"):
        k, _, v = part.strip().partition("=")
        if k == "lc_token":
            return v
    return ""


def url_token(path: str) -> str:
    q = urllib.parse.parse_qs(path.split("?", 1)[1]) if "?" in path else {}
    return (q.get("t") or [""])[0]


class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, body, ctype="application/json", cookie=""):
        b = body if isinstance(body, bytes) else body.encode("utf-8")
        self.send_response(code)
        if cookie:
            self.send_header("Set-Cookie", cookie)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(b)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Content-Security-Policy", "default-src 'self'; style-src 'self' 'unsafe-inline'; "
                         "script-src 'self' 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'")
        self.end_headers()
        self.wfile.write(b)

    def _authed(self) -> bool:
        host = self.headers.get("Host", "")
        origin = self.headers.get("Origin")
        if host not in (f"127.0.0.1:{PORT}", f"localhost:{PORT}"):
            return False
        if origin and origin not in (f"http://127.0.0.1:{PORT}", f"http://localhost:{PORT}"):
            return False
        return token_ok(self.headers.get("X-Token", "")) or token_ok(cookie_token(self.headers.get("Cookie", ""))) \
            or token_ok(url_token(self.path))

    def _art(self, name: str):
        if self.headers.get("Host", "") not in (f"127.0.0.1:{PORT}", f"localhost:{PORT}"):
            return self._send(403, '{"error":"forbidden"}')
        name = urllib.parse.unquote(name)
        b = art_bytes(name)
        if b is None:
            return self._send(404, "not found", "text/plain")
        self.send_response(200)
        self.send_header("Content-Type", ART_TYPES[name])
        self.send_header("Content-Length", str(len(b)))
        self.send_header("Cache-Control", "public, max-age=604800")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        path = self.path.split("?")[0]
        if path.startswith("/art/"):
            return self._art(path[len("/art/"):])
        if path == "/":
            if not (token_ok(url_token(self.path)) or token_ok(cookie_token(self.headers.get("Cookie", "")))):
                return self._send(403, "This link is old. Close this tab and double-click 'Leyline Control.bat'.",
                                  "text/plain")
            return self._send(200, PAGE.replace("__TOKEN__", TOKEN).replace("__THEMEV__", theme_version()),
                              "text/html; charset=utf-8", cookie=f"lc_token={TOKEN}; HttpOnly; SameSite=Strict; Path=/")
        if not self._authed():
            return self._send(403, '{"error":"forbidden"}')
        if path == "/api/state":
            with lock:
                running = state["running"]
                health, at = state["health"], state["health_at"]
                lines = state["lines"][-400:]
            return self._send(200, json.dumps({"pending": pending(), "history": history(),
                                               "claude": claude_status(), "running": running,
                                               "health": health, "health_at": at, "lines": lines,
                                               "autopilot": autopilot_on(), "confirm": confirm_on()}))
        if path == "/api/log":
            name = self.path.split("name=", 1)[-1]
            f = (LOGS / os.path.basename(urllib.request.unquote(name)))
            if not f.exists():
                return self._send(404, "no such log", "text/plain")
            return self._send(200, f.read_text(encoding="utf-8", errors="replace"), "text/plain; charset=utf-8")
        if path == "/api/events":
            q: queue.Queue = queue.Queue()
            with lock:
                listeners.append(q)
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            try:
                while True:
                    try:
                        msg = q.get(timeout=15)
                        self.wfile.write(f"data: {msg}\n\n".encode())
                    except queue.Empty:
                        self.wfile.write(b": ping\n\n")
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
                pass
            finally:
                with lock:
                    if q in listeners:
                        listeners.remove(q)
            return
        self._send(404, '{"error":"not found"}')

    def do_POST(self):
        if not self._authed():
            return self._send(403, '{"error":"forbidden"}')
        n = int(self.headers.get("Content-Length") or 0)
        body = json.loads(self.rfile.read(n) or b"{}") if n else {}
        if self.path == "/api/launch":
            msg = str(body.get("message") or "")[:200].replace("\n", " ")
            state["auto_failed"] = None
            if not start_run(msg):
                return self._send(409, '{"error":"already running"}')
            return self._send(200, '{"ok":true}')
        if self.path == "/api/confirm":
            set_autopilot(confirm=bool(body.get("on")))
            emit("state", {})
            return self._send(200, json.dumps({"ok": True, "confirm": confirm_on()}))
        if self.path == "/api/open-hq":
            webbrowser.open(HQ_URL)
            return self._send(200, '{"ok":true}')
        if self.path == "/api/autopilot":
            set_autopilot(bool(body.get("on")))
            state["auto_failed"] = None
            emit("state", {})
            return self._send(200, json.dumps({"ok": True, "on": autopilot_on()}))
        if self.path == "/api/check":
            item = {"id": "check-" + time.strftime("%Y%m%d-%H%M%S"), "step": "check",
                    "title": STEPS["check"][1], "note": "", "known": True}
            if not start_run("", [item], False, "Server check"):
                return self._send(409, '{"error":"already running"}')
            return self._send(200, '{"ok":true}')
        if self.path == "/api/gm-console":
            bat = ROOT / "gmconsole" / "Open GM Console.bat"
            if not bat.exists():
                return self._send(404, '{"error":"gmconsole\\Open GM Console.bat is missing"}')
            # Its own window (it keeps the SSH tunnel open; closing that window closes the console).
            subprocess.Popen(["cmd", "/c", "start", "Leyline GM Console", "cmd", "/k", "call", str(bat)],
                             cwd=str(bat.parent), creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
            return self._send(200, '{"ok":true}')
        if self.path == "/api/open-game":
            webbrowser.open("https://play.openvanilla.app/wrath")
            return self._send(200, '{"ok":true}')
        self._send(404, '{"error":"not found"}')


PAGE = r"""<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Leyline Control</title>
<style>
/* Leyline Control in the launcher's and the website's look (owner 1 Nov 01:09: "greatly simplify this control
   center, more like our overall theme"): dark slate, the stone side texture, metal-edged panels, the spinning Leyline
   mark, Marcellus titles and the red stone button. One screen: what needs you, what's next, the live output. The art
   comes from /art (stream/www-files/art, the site's own files); without it the page still reads with its own colours. */
@font-face{font-family:"Marcellus";src:url("/art/marcellus-400.woff2") format("woff2");font-display:swap}
:root{--bg:#121417;--panel:rgba(14,16,19,.82);--edge:#333b38;--text:#e9ebef;--dim:#9aa1ab;--gold:#f1cf86;--gold2:#d9a73a;
--ok:#5fd18a;--bad:#ff8378;--warn:#f2c94c;--blue:#7aa3f5;--mono:ui-monospace,SFMono-Regular,Consolas,monospace;
--serif:"Marcellus",Georgia,serif}
*{box-sizing:border-box}html,body{margin:0;color:var(--text);font:14px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif}
html{background:var(--bg)}
body{min-height:100vh;overflow-x:hidden;background:linear-gradient(rgba(10,11,13,.72),rgba(10,11,13,.82)),url("/art/stone-side.webp") 0 0/256px,var(--bg)}
header{display:flex;align-items:center;gap:10px 16px;padding:12px 22px;flex-wrap:wrap;
background:linear-gradient(rgba(0,0,0,.35),rgba(0,0,0,.55)),url("/art/stone-bar.webp") center/cover,#1a1c1f;
border-bottom:1px solid #000;box-shadow:0 1px 0 rgba(255,255,255,.05),0 6px 18px rgba(0,0,0,.45)}
.brand{display:flex;align-items:center;gap:11px}
.mark{position:relative;width:40px;height:40px;flex:none}
.mark::before,.mark::after{content:"";position:absolute;inset:0;background:center/contain no-repeat}
.mark::before{background-image:url("/art/leyline-swirl-256.webp");animation:turn 16s linear infinite}
.mark::after{background-image:url("/art/leyline-crystal-256.webp")}
@keyframes turn{to{transform:rotate(360deg)}}
h1{margin:0;font:400 23px/1 var(--serif);letter-spacing:.02em;color:var(--gold);white-space:nowrap;
text-shadow:0 -1px 0 rgba(255,255,255,.15),0 1.5px 0 #000,0 3px 6px rgba(0,0,0,.7)}
.health{display:flex;gap:6px;margin-left:auto;flex-wrap:wrap}
.pill{display:flex;align-items:center;gap:6px;padding:3px 10px;border:1px solid var(--edge);border-radius:99px;font-size:12px;
color:var(--dim);background:rgba(0,0,0,.45)}
.dot{width:7px;height:7px;border-radius:50%;background:var(--dim)}
.dot.ok{background:var(--ok);box-shadow:0 0 6px var(--ok)}.dot.bad{background:var(--bad);box-shadow:0 0 6px var(--bad)}
.actions{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
button{font:600 13px system-ui,sans-serif;cursor:pointer;border-radius:7px;border:1px solid #3d4743;background:rgba(0,0,0,.45);
color:var(--text);padding:7px 12px}
button:hover:not(:disabled){border-color:#6d7a74}
button.primary{font:400 17px/1 var(--serif);color:var(--gold);background:none;border:11px solid transparent;
border-image:url("/art/stone-btn.webp") 28 30 fill/11px stretch;padding:2px 14px;border-radius:0;text-shadow:0 2px 2px rgba(0,0,0,.85)}
button.primary:hover:not(:disabled){filter:brightness(1.15)}
button:disabled{opacity:.45;cursor:not-allowed}
#auto.primary{font-size:15px}
.more{position:relative}
.more summary{list-style:none;cursor:pointer;font:600 13px system-ui,sans-serif;border:1px solid #3d4743;border-radius:7px;
padding:7px 12px;background:rgba(0,0,0,.45)}
.more summary::-webkit-details-marker{display:none}
.more[open] .menu{position:absolute;right:0;top:calc(100% + 6px);z-index:5;display:grid;gap:6px;padding:10px;min-width:220px;
background:#16191c;border:1px solid var(--edge);border-radius:10px;box-shadow:0 12px 30px rgba(0,0,0,.6)}
.menu button{text-align:left;width:100%}
main{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1.6fr);gap:16px;padding:18px 22px;max-width:1500px;margin:0 auto}
@media (max-width:980px){main{grid-template-columns:minmax(0,1fr)}}
.card{background:var(--panel);border:7px solid transparent;border-image:url("/art/window-edge.png") 19/7px round;
background-clip:padding-box;padding:14px 16px;min-width:0}
.card h2{margin:0 0 10px;font:400 17px/1.2 var(--serif);letter-spacing:.02em;color:var(--gold);text-shadow:0 1px 0 #000}
.stack{display:flex;flex-direction:column;gap:16px;min-width:0}
.youbox{margin:14px 22px 0;padding:10px 14px;border:1px solid #8a6a2c;border-radius:10px;background:rgba(242,201,76,.08)}
.youbox h2{margin:0 0 4px;font:400 15px var(--serif);color:var(--warn)}
.youbox:has(> #you > .empty){display:none}
.item{display:flex;gap:10px;padding:7px 0;border-top:1px solid rgba(255,255,255,.05)}
.item:first-of-type{border-top:0}
.item>div{min-width:0;overflow-wrap:anywhere}
.tag{flex:none;font:11px var(--mono);padding:2px 7px;border-radius:5px;height:fit-content;border:1px solid var(--edge);
color:var(--gold);background:rgba(0,0,0,.45)}
.sub{color:var(--dim);font-size:12.5px}
.you .tag{color:var(--warn)}.now .tag{color:var(--blue)}.fin .tag{color:var(--ok)}
.launch{display:flex;gap:10px;align-items:center;margin-top:12px;flex-wrap:wrap}
input{font:inherit;flex:1;min-width:160px;background:rgba(0,0,0,.5);border:1px solid #0a0908;border-radius:7px;color:var(--text);
padding:8px 11px;box-shadow:inset 0 2px 4px rgba(0,0,0,.6)}
.console{font:12.5px/1.45 var(--mono);background:rgba(3,4,5,.9);border:1px solid #0a0908;border-radius:8px;height:min(62vh,640px);
overflow:auto;padding:10px 12px;white-space:pre-wrap;word-break:break-word;color:#d3d6dc;box-shadow:inset 0 2px 8px rgba(0,0,0,.7)}
.console .err{color:var(--bad)}.console .hdr{color:var(--gold)}.console .ok{color:var(--ok)}
.status{display:flex;align-items:center;gap:9px;margin-bottom:8px;font-weight:600}
.spinner{position:relative;width:16px;height:16px;flex:none}
.spinner::before,.spinner::after{content:"";position:absolute;inset:0;background:center/contain no-repeat}
.spinner::before{background-image:url("/art/leyline-swirl-256.webp");animation:turn 2.4s linear infinite}
.spinner::after{background-image:url("/art/leyline-crystal-256.webp")}
@media (prefers-reduced-motion:reduce){.mark::before,.spinner::before{animation:none}}
details.fold>summary{cursor:pointer;font:400 15px var(--serif);color:var(--gold);list-style:none}
details.fold>summary::-webkit-details-marker{display:none}
details.fold>summary::before{content:"\25B8  ";color:var(--dim)}details.fold[open]>summary::before{content:"\25BE  "}
details.fold[open]>summary{margin-bottom:8px}
.fold h3{margin:10px 0 4px;font:600 12px system-ui,sans-serif;letter-spacing:.06em;text-transform:uppercase;color:var(--dim)}
.empty{color:var(--dim);font-style:italic}
a{color:var(--gold)}.hist a{font:12px var(--mono)}
.updated{font-size:12px;color:var(--dim);margin-top:6px}
@media (max-width:640px){header{padding:10px 14px}.health{margin-left:0;width:100%}main{padding:12px;gap:12px}
.card{padding:10px 12px}.console{height:340px}.youbox{margin:12px 12px 0}}
</style></head><body class="ly">
<header><div class="brand"><span class="mark" aria-hidden="true"></span><h1>Leyline Control</h1></div>
<div class="health" id="health"></div>
<div class="actions"><button id="auto" title="While this window is open, Claude's updates run on their own">Autopilot: …</button>
<details class="more"><summary>More</summary><div class="menu">
<button id="hq" title="Leyline HQ: status, what needs you, releases, notes (play.openvanilla.app/wrath/admin)">Open HQ</button>
<button id="gmc" title="Opens the GM console (bug reports, players, health, logs) in your browser">GM console</button>
<button id="game">Open the game</button><button id="check">Check server</button>
<button id="ask" title="On: deploys wait for Launch all; read-only checks still run by themselves">Ask before deploys: …</button>
</div></details></div></header>
<section class="youbox"><h2>Waiting on you</h2><div id="you"></div></section>
<main>
<div class="stack">
  <section class="card"><h2>Up next</h2><div id="pending"></div>
    <div class="launch"><input id="msg" placeholder="GitHub save note (only for 'Save to GitHub')">
    <button class="primary" id="go">Launch all</button></div></section>
  <section class="card"><details class="fold"><summary>Claude's notes</summary>
    <h3>Working on</h3><div id="now"></div><div class="updated" id="cupd"></div>
    <h3>Finished</h3><div id="fin"></div></details></section>
  <section class="card hist"><details class="fold"><summary>History</summary><div id="hist"></div></details></section>
</div>
<div class="stack">
  <section class="card"><h2>Live output</h2><div class="status" id="run"></div><div class="console" id="con"></div></section>
</div></main>
<script>
const T="__TOKEN__",H={"X-Token":T};
const $=id=>document.getElementById(id);
const esc=s=>String(s??"").replace(/[&<>"]/g,c=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;"}[c]));
function list(el,items,cls,empty){el.innerHTML=items&&items.length?items.map(i=>{const o=typeof i==="string"?{text:i}:i;
return `<div class="item ${cls}"><span class="tag">${esc(o.tag||"•")}</span><div>${esc(o.text)}${o.detail?`<div class="sub">${esc(o.detail)}</div>`:""}</div></div>`}).join(""):`<div class="empty">${empty}</div>`}
function line(t){const c=$("con"),d=document.createElement("div");d.textContent=t;
if(/^=====/.test(t))d.className="hdr";else if(/^!!|FAILED|error/i.test(t))d.className="err";else if(/^== .*done|OK$/.test(t))d.className="ok";
const stick=c.scrollTop+c.clientHeight>=c.scrollHeight-30;c.appendChild(d);if(stick)c.scrollTop=c.scrollHeight}
let last=null;
async function refresh(){const r=await fetch("/api/state",{headers:H});if(!r.ok)return;const s=await r.json();last=s;
const h=$("health");h.innerHTML=Object.entries(s.health||{}).map(([k,v])=>`<span class="pill"><span class="dot ${v.ok?"ok":"bad"}"></span>${esc(k)}${v.ms!=null?` · ${v.ms} ms`:` · ${esc(v.code)}`}</span>`).join("")||'<span class="pill">checking…</span>';
const c=s.claude||{};list($("you"),c.waiting_on_you,"you","Nothing. You're clear.");
list($("now"),c.now,"now",c.note||"Idle.");list($("fin"),c.done,"fin","Nothing yet.");
$("cupd").textContent=c.updated?`Updated ${c.updated}`:"";
list($("pending"),(s.pending||[]).map(p=>({tag:p.step,text:p.title,detail:p.note})),"","Nothing to launch. You're up to date.");
const a=$("auto");a.textContent=s.autopilot?"Autopilot: ON":"Autopilot: OFF";a.className=s.autopilot?"primary":"";a.dataset.on=s.autopilot?"1":"";
const q=$("ask");q.textContent=s.confirm?"Ask before deploys: ON":"Ask before deploys: OFF";q.dataset.on=s.confirm?"1":"";
$("go").disabled=!!s.running||!(s.pending||[]).length;
$("run").innerHTML=s.running?`<span class="spinner"></span>${esc(s.running.title||"Running")}…`:'<span class="sub">Idle</span>';
$("hist").innerHTML=(s.history||[]).map(x=>`<div class="item fin"><span class="tag">${esc(x.step)}</span><div>${esc(x.note||x.id)} ${x.log?`<a href="/api/log?t=${T}&name=${encodeURIComponent(x.log)}" target="_blank">log</a>`:""}</div></div>`).join("")||'<div class="empty">No runs yet.</div>';
if(!$("con").childElementCount&&s.lines)s.lines.forEach(line)}
$("go").onclick=async()=>{$("con").innerHTML="";const r=await fetch("/api/launch",{method:"POST",headers:{...H,"Content-Type":"application/json"},body:JSON.stringify({message:$("msg").value})});if(!r.ok)alert("Couldn't start: "+(await r.text()));refresh()};
$("check").onclick=async()=>{$("con").innerHTML="";const r=await fetch("/api/check",{method:"POST",headers:H});if(!r.ok)alert("Busy: "+(await r.text()));refresh()};
$("auto").onclick=async()=>{await fetch("/api/autopilot",{method:"POST",headers:{...H,"Content-Type":"application/json"},body:JSON.stringify({on:!$("auto").dataset.on})});refresh()};
$("gmc").onclick=async()=>{const r=await fetch("/api/gm-console",{method:"POST",headers:H});if(!r.ok)alert("Couldn't open it: "+(await r.text()))};
$("game").onclick=()=>fetch("/api/open-game",{method:"POST",headers:H});
$("hq").onclick=()=>fetch("/api/open-hq",{method:"POST",headers:H});
$("ask").onclick=async()=>{await fetch("/api/confirm",{method:"POST",headers:{...H,"Content-Type":"application/json"},body:JSON.stringify({on:!$("ask").dataset.on})});refresh()};
const es=new EventSource(`/api/events?t=${T}`);
es.onmessage=e=>{const m=JSON.parse(e.data);if(m.kind==="line")line(m.data);else refresh()};
refresh();setInterval(refresh,10000);
</script></body></html>"""


def main() -> int:
    url = f"http://127.0.0.1:{PORT}/?t={TOKEN}"
    class Server(http.server.ThreadingHTTPServer):
        # Windows' SO_REUSEADDR lets a second process share the port while the old one keeps answering;
        # claim it exclusively instead, so a stale copy can never serve the page.
        allow_reuse_address = False

        def server_bind(self):
            import socket
            if hasattr(socket, "SO_EXCLUSIVEADDRUSE"):
                self.socket.setsockopt(socket.SOL_SOCKET, socket.SO_EXCLUSIVEADDRUSE, 1)
            super().server_bind()

    srv = None
    # --wait-port: started by restart_if_updated(); the old copy is still letting go of the port.
    for _ in range(40 if "--wait-port" in sys.argv else 1):
        try:
            srv = Server(("127.0.0.1", PORT), H)
            break
        except OSError:
            if "--wait-port" in sys.argv:
                time.sleep(0.5)
    if srv is None:
        # Already running (another window): just open it.
        print("Leyline Control is already running; opening it.")
        if not QUIET:
            webbrowser.open(url)
        return 0
    threading.Thread(target=check_health, daemon=True).start()
    threading.Thread(target=autopilot_loop, daemon=True).start()
    print(f"Leyline Control: {url}\n(close this window to stop it)")
    print("Autopilot is " + ("ON: Claude's updates run on their own." if autopilot_on() else "OFF (switch it on in the window)."))
    if not QUIET:
        webbrowser.open(url)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
