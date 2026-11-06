#!/usr/bin/env python3
"""Leyline Control: the owner's one window for launching what Claude has ready.

    py -3 tools\\control\\control.py          (or double-click "Leyline Control.bat")

Serves a page on http://127.0.0.1:8097 (this PC only) and opens it. The page shows:
  * Live: the realm, Play page and game files answering (checked from this PC every 30 s).
  * Claude: what Claude is working on, what is waiting for you, what finished (control/claude-status.json,
    which Claude keeps up to date in this folder).
  * Waiting on you: each thing Claude needs, with its own buttons (Do it: runs a step; Open the page; Open the
    folder; Add files: saved in control/inbox for Claude; an answer box; Done), answers in control/owner-answers.json.
  * Ready to launch: the steps in UPDATES-PENDING.txt not yet in UPDATES-DONE.txt; "Launch all" runs them in
    order (the same steps UPDATE.bat runs) and streams each step's output live; it stops at a failure.
  * History: finished steps with their logs (logs/<id>-<step>.log).
  * Discord: the Discord redo's bot token (pasted once, saved on this PC only in tools/discord-leyline/token.txt,
    git-ignored, never shown back, never in a log), Preview (read-only), and Do it: only after a fresh preview, and
    only when the owner types YES under it; never from Autopilot, Launch all or a Waiting on you button.
    Its Pending announcements list (tools/discord-leyline/announce/pending, README "Announcements"): each draft Claude
    wrote, exactly as it would post, with Check, Reject and "Post to #announcements" (only after a typed YES, and only
    the draft as the owner read it: its sha256 goes with the YES). discord-announce is never a queued step.
  * Kill switch (docs/KILL-SWITCH.md, tools/killswitch): the farewell message and the parts to take down; Pause, Undo
    and Delete, each only after its own fresh preview and a typed phrase (SHUT DOWN LEYLINE, BRING LEYLINE BACK,
    DELETE LEYLINE FOREVER), from this card only: it is not a step, so never the queue, Autopilot or a button elsewhere.
  * Discord & GM: read-only, the server's checks, the Discord bridge (wu-discord, from the last server check) and bot,
    the open bug reports (the bugs step's backlog), the addons and looks waiting for review, the last quick edits; its
    buttons run existing steps (check, bugs, discord-check, discord-preview, worldlog, mods-daily) on a click.
  * Quick edits: the apply-check / apply steps (tools/control/quick.py): a manifest of files and their sha256 and one
    deploy step, checked, instead of a new .bat for each small change.
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
INBOX = ROOT / "control" / "inbox"                  # files the owner drops for Claude (Waiting on you: Add files)
ANSWERS = ROOT / "control" / "owner-answers.json"     # the owner's Done / answers to Waiting on you, for Claude
DROP_MAX = 200 * 1024 * 1024
AUDIT_HTML = ROOT / "logs" / "audit-latest.html"     # the release audit's report (tools/audit, docs/AUDIT.md)
AUDIT_TXT = ROOT / "logs" / "audit-latest.txt"
# The Discord redo (tools/discord-leyline): the bot token box and the preview / do it / bot settings steps
DISCORD_DIR = ROOT / "tools" / "discord-leyline"
DISCORD_TOKEN = DISCORD_DIR / "token.txt"            # git-ignored; the script reads it; Control never shows it
DISCORD_DOC = ROOT / "control" / "discord.json"      # the last preview's summary, the last do it and bot settings
DISCORD_FRESH = 3600                                  # Do it needs a preview from the last hour
ANNOUNCE_DIR = DISCORD_DIR / "announce"              # announcement drafts: pending/, posted/, rejected/ (announce.py)
# The kill switch (docs/KILL-SWITCH.md): its card's message, choices, last preview and runs (no secrets in it)
KS_DIR = ROOT / "tools" / "killswitch"
KS_DOC = ROOT / "control" / "killswitch.json"
HQ_URL = "https://leylinelauncher.com/wrath/admin"   # Leyline HQ: the one place online (stream/hq.py)
# The Discord & GM card (gm_overview): what it reads, all files this PC already has; it changes none of them
BUGS_BACKLOG = ROOT / "tools" / "bugs" / "live" / "backlog.json"     # the bugs step's copy (docs/BUGS-MANIFEST.md)
SERVER_CHECK = ROOT / "logs" / "server-check-latest.txt"             # the check step's report (realm/diag.sh)
UPSTREAM = ROOT / "mods" / "upstream.json"                           # curated addons; pending = no reviewer yet
LOOKS = ROOT / "stream" / "looks.json"                               # looks; pending = no reviewer yet
QUICK_LOG = ROOT / "control" / "quick" / "applied.jsonl"             # quick edits applied (tools/control/quick.py)
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
    # only Den Lite's editor tests (gizmo.rs, pick.rs): the quick loop for the mouse and the arrows (owner 4 Nov: "one test at a time")
    "den-lite-test": ("tools/den-lite/Den Lite editor tests.bat", "Den Lite editor tests only: click to carry, the arrows, the ground, the debug line (a few minutes)", False),
    "den-lite-all": ("tools/den-lite/Den Lite all tests.bat", "Every Den Lite test: units and the test client (several minutes)", False),
    "den-lite": ("tools/den-lite/Build Den Lite.bat", "Build Den Lite on this PC (the instant Den server, no Docker) for the launcher's Your Den", False),
    # My Den, one click into the world on this PC's own game (tools/den-check/den_check.py): den-check looks (read-only:
    # the game folder, Leyline's login archive, Config.wtf, Den Lite and a sign-in through the exact steps on a Den Lite
    # of its own); den-test runs the real thing unattended (Leyline.exe den-play --test: Den Lite, the game, signed in,
    # the white island, then the Den's smoke test through Den Lite's command files, a screenshot, closed again) and
    # leaves tools\den-check\logs\den-test-latest.txt and .png.
    "den-check": ("tools/den-check/Den check.bat", "My Den check (read-only): game folder, login archive, Config.wtf, Den Lite and a test sign-in", False),
    "client-smoke": ("tools/client-smoke/Client smoke test.bat", "Client smoke test: your game started as PLAY starts it on Wrath Unleashed (the launcher's sign-in skips the login screen), a screenshot every few seconds, then closed and the sign-in taken out (logs\\client-smoke)", False),
    "client-smoke-world": ("tools/client-smoke/Client smoke test - world.bat", "Client smoke test, on into the world with your last character (logs\\client-smoke)", False),
    "den-test": ("tools/den-check/Den test.bat", "My Den test: starts your game by itself, signs in, then builds, saves and loads, the white room, Forge's files, a shareable Den file, an invite a test friend signs in with, the autosave; a screenshot, closes it (logs in tools\\den-check\\logs)", False),
    # the first-run test (tools/first-run/first_run.py, docs/FIRST-RUN-TEST.md): a brand-new launcher through to in the game
    "first-run-test": ("tools/first-run/First run test.bat", "First-run test: today's download in an empty data folder, your game folder added, a test Hero Key logged in, PLAY on Wrath Unleashed, the game logs in by itself, into the world, then the Den test; screenshots, then the test account and temp folders removed (logs in tools\\first-run\\logs)", False),
    # the fresh computer test (docs/FRESH-COMPUTER-TEST.md): every feature on a brand-new launcher; changes nothing of
    # yours but a temp profile (your game folder's files are put back), so not READ_ONLY: it starts your game and
    # makes (and closes) a test account on the website
    "fresh-test": ("tools/first-run/Fresh computer test.bat", "Fresh computer test: today's download in an empty profile; WebView2, runtimes, addons and Den Lite downloads; your game folders added; drag and drop (folder, zip, link; magnet and .torrent refused); a test Hero Key in by file, out, in by paste; the website's sign-in cookie; PLAY into the world; a community realm and back; Your Den; loadouts Clean and back; all cleaned up (logs in tools\\first-run\\logs)", False),
    "fresh-sandbox": ("tools/first-run/Fresh computer test - sandbox.bat", "Fresh computer test in Windows Sandbox: a brand-new Windows with your game folder copied in (yours mapped read-only); the report lands in tools\\first-run\\logs\\sandbox\\out (needs Windows Sandbox on)", False),
    "den-test-variants": ("tools/den-check/Den test - variants.bat", "The Den test once (since 3 Nov the Den is the game's own grass and sky: nothing left to vary)", False),
    "den-test-invite": ("tools/den-check/Den test - invite.bat", "My Den test (the same as den-test: it always invites a test friend now)", False),
    "ui-local": ("tools/den-check/UI into my game.bat", "The newest in-game UI into this PC's game folder for testing (backup first; players still get it from addons)", False),
    # the gliding copy's creature rows from this PC's own game tables (labs/den-lite gliderows.rs), before a launcher
    # with them is out; den-move makes them itself when they're missing
    "den-glide": ("tools/den-check/Den glide rows.bat", "The gliding copy in Your Den: Den Lite makes each prop model's creature rows from your own game's tables into Data\\patch-S.MPQ (backup first; den-glide-undo takes them away). Run den-lite first; close the game", False),
    "den-glide-undo": ("tools/den-check/Den glide rows - undo.bat", "Takes the gliding copy's rows away (our Data\\patch-S.MPQ and the marker): the copy is a gameobject made anew again", False),
    "den-move": ("tools/den-check/Den move.bat", "The one Den test for now: select and move (build mode, a prop placed and selected, hover glow, click selects, press and drag moves it, snap out and back, zoom far and back; pictures in tools\\den-check\\logs\\den-move)", False),
    "den-features": ("tools/den-check/Den features.bat", "The Den editor's feature test: den-test, then the gallery (do the tiles show models?), five props placed, select, move, turn, copy, paste, delete, undo, redo, save and open, every panel and play mode, through the editor's own buttons; a screenshot after each (tools\\den-check\\logs\\den-features)", False),
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
    # Azeroth Survivors at its own address (docs/SURVIVORS.md, steps 7 and "Off again"). The recording before it
    # (tools\survivors\Record Survivors files.bat) asks for your Data folder and plays in a browser window for about
    # 45 minutes, so it stays a double-click and is not a step here.
    # owner 2 Nov "do the record": the same double-click as a step, now that tools\survivors\out\data-folder.txt names
    # the 1.12 Data folder (no question). An Edge window plays for about 45 minutes, then asks for one pass by hand.
    "sv-record": ("tools/survivors/Record Survivors files.bat", "Azeroth Survivors: record the game files it uses from your 1.12 folder (an Edge window, about 45 min, then one short pass by hand)", False),
    "sv-export": ("tools/survivors/Export Survivors files.bat", "Azeroth Survivors: copy the recorded files out of your 1.12 folder (no new recording; a few minutes)", False),
    "sv-preview": ("tools/survivors/Survivors preview.bat", "Azeroth Survivors on this PC only for 20 minutes (127.0.0.1:18093), for filming the card's GIF", False),
    "sv-site-check": ("hosting/survivors/Put Survivors on Leyline - check.bat", "Azeroth Survivors: its address and DNS, and whether the build and your recorded files are ready (read-only)", False),
    "sv-site": ("hosting/survivors/Put Survivors on Leyline.bat", "Azeroth Survivors on: upload the build and your recorded files, its own site behind the sign-in, self-check, then Play (rollback on failure)", False),
    "sv-site-off": ("hosting/survivors/Put Survivors on Leyline - off.bat", "Undo: Azeroth Survivors off (Play off, its site out; the files stay on the server)", False),
    "ovsite": ("tools/launcher-site/Land Leyline-Launcher site.bat", "openvanilla.app: land the site and repo cleanup on Leyline-Launcher main (Pages publishes it)", False),
    "ghclean-check": ("tools/github-cleanup/GitHub cleanup - dry run.bat", "What the GitHub cleanup would delete (read-only)", False),
    "ghclean": ("tools/github-cleanup/GitHub cleanup - do it.bat", "GitHub cleanup: old releases and packages (owner's choices, 2026-10-30)", False),
    "scripts-reload": ("realm/Reload realm scripts.bat", "Realm scripts reloaded live even when unchanged (for a script that is there but silent)", False),
    "scripts": ("realm/Server pass - finder 10 and bug reports.bat", "Realm scripts (live reload)", False),
    "realm":   ("realm/Server pass - finder 10 and bug reports.bat", "Realm settings (5-min restart)", False),
    # A new realm image (C++ modules, e.g. mod-leyline-bossfights; docs/BOSS-FIGHTS.md): the build GitHub made after a
    # ship (wu-images), pulled on the server; backup, :rollback kept, 5-min warning, self-check, rollback on failure.
    "realm-image": ("realm/Update realm image.bat", "New realm image: the C++ modules GitHub built after the last ship, live (backup, 5-min restart, self-check, rollback)", False),
    # Race/class pairs at character creation (docs/RACE-CLASS-COMBOS.md): the realm's preset, its world tables, the
    # website's copy (the browser page and the launcher follow it). "combos" keeps the realm's preset (modern the
    # first time); the other two switch it. Backup first; "realm/Rollback character fix.bat" undoes it.
    "combos": ("realm/Fix character create.bat", "Race/class combos: the realm's preset (modern: today's 83 retail pairs), start data, website told (world server restart)", False),
    "combos-all": ("realm/Race class combos - all.bat", "Race/class combos: every race with every class (100 pairs; world server restart)", False),
    "combos-modern": ("realm/Race class combos - modern.bat", "Race/class combos: back to today's 83 retail pairs (world server restart)", False),
    "combos-check": ("realm/Diagnose character create.bat", "Race/class combos: what the realm has now (read-only)", False),
    "foreverui": ("stream/Add Forever UI to the stream.bat", "Forever UI look (menus and character select)", False),
    "slice":   ("stream/Make mini slice.bat", "Streamed game files (30-60 min)", False),
    # docs/INCIDENT-2026-11-01-SLICE.md: the streamed slice's files on the server, checked (read-only)
    "slice-health": ("stream/Slice health - check.bat", "Read-only: is the streamed slice whole (key files, its tier lists, what wu-play answers)", False),
    "ship":    ("Ship to GitHub.bat", "Save everything to GitHub", True),
    "ship-again": ("tools/ship/Save again as one fresh commit.bat", "Save to GitHub again as one fresh commit (after GitHub refused a save)", False),
    # the bot smoke test (docs/BOTS-UPDATES.md): a throwaway realm from the canary images (bots-canary.yml) on this PC's
    # own extracted game data, a headless player, the bots checked, all removed after. Touches no realm of yours.
    "bots-smoke": ("realm/Bots smoke test.bat", "Bot smoke test: a throwaway realm from the newest canary bots, a test player, bots join, go into a dungeon, follow, fight one pull, leave; then removed (report in logs\\bots-smoke-latest.json)", False),
    "check":   ("realm/Server check.bat", "Server check (read-only)", False),
    # The release audit (docs/AUDIT.md): every feature as a new player meets it, the report in logs/audit-latest.html.
    # Read only but for the audit's own AUDIT account and its test characters, and a Den Lite temp folder on this PC.
    "audit":   ("tools/audit/Release audit.bat", "Release audit (read-only, ~10 min)", False),
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
    "leyline-launcher": ("tools/leyline-repo/Update Leyline-Launcher repo.bat", "Leyline-Launcher repo (the old Open Vanilla one): the same Leyline version, copied and pushed (its site and releases kept)", False),
    "leyline": ("tools/leyline-repo/Update Leyline repo.bat", "Leyline repo: copy and push (github.com/orionopenwow/Leyline)", False),
    "mods":  ("mods/Put mods on Wrath.bat", "Mods catalog: realm (live reload), streamed addons and /wrath/mods", False),
    # Resilience (docs/RESILIENCE.md): after a ship, our own copy of each Leyline release, the Host pack, the addons and
    # the upstream server source at our pinned commits, on our server with checksums and torrents (and, with the release
    # key on this PC, the signed release log), so launchers keep updating and people keep hosting if GitHub is down.
    "release-mirror": ("tools/release-mirror/Mirror releases.bat", "Release mirror: copy the new Leyline releases, Host pack, addons and server source to our server (after a ship)", False),
    "release-mirror-check": ("tools/release-mirror/Mirror releases - check.bat", "Release mirror: what it would copy (read-only)", False),
    # The fork kit (docs/TOOLKIT.md, docs/SUCCESSION.md): is this copy ready to run under another name, keys and
    # servers: branding.json, brand words, update sources and spare keys, the release-log key, relays (fetched).
    "fork-check": ("tools/fork-kit/Fork check.bat", "Fork readiness: branding, brand words, update sources, spare keys, release-log key, relays (read-only)", False),
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
    "fresh-leyline-check": ("tools/fresh-history/Fresh history - Leyline repo (dry run).bat", "The public Leyline repo's history as Orion: what it would do (read-only)", False),
    "wipe-releases": ("tools/fresh-history/Wipe old releases - Leyline repo.bat", "Old launcher releases off the public Leyline repo (the newest, launcher-latest, host-pack, host-kit and leyline-addons kept)", False),
    "retag-leyline-check": ("tools/fresh-history/Retag - Leyline repo (dry run).bat", "The public Leyline repo's release tags still on old commits by someone else: which would move (read-only)", False),
    "retag-leyline": ("tools/fresh-history/Retag - Leyline repo.bat", "Those release tags moved onto Orion's main (releases and files kept; old tags backed up on the Desktop); needs tools\\fresh-history\\CONFIRM-retag", False),
    "fresh-leyline": ("tools/fresh-history/Fresh history - Leyline repo.bat", "The public Leyline repo's main as one commit by Orion (releases kept; backup on the Desktop)", False),
    "fresh-history": ("tools/fresh-history/Fresh history - do it.bat", "The workspace repo's main as one commit by Orion (releases kept; backup on the Desktop)", False),
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
    # The daily mods and addons run (docs/MODS-DAILY.md): curate, new thumbnails, checks, then what waits for review.
    # It never approves anything; scouting and the index gate run in the cloud.
    "mods-daily": ("tools/hub-index/Mods daily.bat", "Mods & addons daily: curate addons, new thumbnails, check the lists (everything new waits for your review; then ship + signin)", False),
    "tidy-check": ("tools/server-tidy/Server tidy - dry run.bat", "Read-only: what making room on the server would remove, and how much", False),
    "server-fix": ("tools/server-fix/Server fix.bat", "Server fix: the streamed-file folders root-only; a read-only look at why .bug didn't answer (no restart)", False),
    "tidy": ("tools/server-tidy/Server tidy.bat", "Make room on the server: build cache, old logs, old site backups, the parked VR browser slice (owner OK 1 Nov)", False),
    "ov112-data-check": ("tools/ov112-data/Old 1.12 data - check.bat", "Read-only: where the old 1.12 game files are on the server, their size and who serves them", False),
    "ov112-data-remove-check": ("tools/ov112-data/Old 1.12 data - remove (dry run).bat", "Dry run: what removing the old 1.12 game files would do", False),
    "ov112-data-remove": ("tools/ov112-data/Old 1.12 data - remove.bat", "Remove the old 1.12 game files (the old site answers 410 for /data first; Wrath untouched)", False),
    "worldlog": ("tools/diag/World server log.bat", "Read-only: why the world server restarted (state, recent output, crash dumps)", False),
    "caddy-check": ("tools/security/Caddy check.bat", "Caddy and compose check: live config to the PC, secrets removed (read-only)", False),
    # HTTP/3 off: browsers use HTTP/2 over TCP (4 Nov: downloads crawled at ~20 KB/s over HTTP/3). Backup, self-check, rollback.
    # Spike test (owner 4 Nov): a minute of moderate load on the live services from inside the server; what each takes.
    "spike-test": ("tools/spike/Spike test.bat", "Spike test: a minute of load on the live server, what each part can take (writes nothing)", False),
    "h3-off":  ("tools/security/Caddy - HTTP3 off.bat", "Website speed: HTTP/3 off, browsers use HTTP/2 (backup, self-check, rollback)", False),
    "h3-on":   ("tools/security/Caddy - HTTP3 on again.bat", "Undo h3-off: HTTP/3 on again", False),
    # The bugs manifest (docs/BUGS-MANIFEST.md): every bug report (game, site, launcher, Discord) in one de-duplicated
    # backlog. Uploads tools/bugs/status.json, has the GM console rebuild, copies BUGS-MANIFEST.md and backlog.json to
    # tools/bugs/live/ (git-ignored). Touches only the bug backlog, so Autopilot may run it (READ_ONLY below).
    "bugs": ("tools/bugs/Pull bug manifest.bat", "Bug manifest: every bug report in one list, copied to tools\\bugs\\live (statuses from tools\\bugs\\status.json go up first)", False),
    # The feedback digest (stream/feedback.py, docs/ADMIN-CONSOLE.md "Review"): bug reports, the forum and the Discord's
    # bug channel in themes with a mood score, and the quietest window for pushing updates. Writes only the digest on the
    # server (HQ > Review > Feedback) and copies it, with the upgrades waiting for the owner, to tools/feedback/live/
    # (git-ignored). The portal makes one by itself every few hours too; this one reads every source on the host.
    "feedback": ("tools/feedback/Feedback digest.bat", "Feedback digest (read-only): themes and mood from bug reports, the forum and Discord, and the quietest release window, copied to tools\\feedback\\live", False),
    # The Discord redo (tools/discord-leyline/README.md), from the Discord panel (the bot token pasted there, saved in
    # tools\discord-leyline\token.txt). discord-apply runs only from that panel: after a preview from the last hour, when
    # the owner types YES under its summary (OWNER_ONLY below); it backs up the server to the Desktop first.
    "discord-preview": ("tools/discord-leyline/Control - Discord preview.bat", "Discord redo preview: what would change on your server (read-only)", False),
    "discord-apply": ("tools/discord-leyline/Control - Discord do it.bat", "Discord redo, done: the server backed up to your Desktop, then the changes the preview showed", False),
    "discord-read": ("tools/discord-leyline/Control - Discord read.bat", "Discord read (read-only): the newest messages in every channel since the last read, into tools\\discord-leyline\\live\\inbox-latest.md on this PC", False),
    "discord-bridge": ("tools/discord-leyline/Control - Discord bridge.bat", "Link World chat to Discord: finds #world-chat, installs the bridge on the server with the saved bot token and turns it on (token never shown)", False),
    "discord-check": ("tools/discord-leyline/Control - Discord bot settings.bat", "Discord bot settings: the channel ids for the website bot and the game bridge (read-only)", False),
    # Milestone announcements (tools/discord-leyline/README.md "Announcements"): Claude drafts into announce/pending/,
    # the owner reads each one on the Discord card and posts it there after typing YES (discord-announce, OWNER_ONLY:
    # never from the queue). announce-check only checks the drafts (read-only, needs no token), so it may be queued.
    "discord-announce": ("tools/discord-leyline/Control - Discord announce.bat", "Post one announcement you approved to #announcements (only from the Discord card, after you type YES)", False),
    "announce-check": ("tools/discord-leyline/Control - Announce check.bat", "Announcement drafts check (read-only): length, words, links and ping rules for each draft waiting; posts nothing", False),
    # Quick Claude edits (tools/control/quick.py, README.md "Quick edits"): control/quick/<id>.json names the files, their
    # sha256 and one deploy step (signin, addons, scripts or none); apply-check only checks it, apply checks it again and
    # runs that step's own .bat. Anything else in a manifest is refused. No new .bat for each small change.
    "apply-check": ("tools/control/Quick apply - check.bat", "Quick edit check (read-only): the manifest's files are inside Leyline, no secrets, each its sha256", False),
    "apply": ("tools/control/Quick apply.bat", "Quick edit: the manifest's files checked, then its one deploy step (signin, addons, scripts or none)", False),
    "hq-sync": ("tools/hq/HQ sync.bat", "Leyline HQ: send the recent changes and Claude's notes, fetch the owner's notes", False),
    "hqnote": ("tools/hq/HQ sync.bat", "Leyline HQ: publish Claude's notes (and sync)", False),
    "hq-inbox": ("tools/hq/HQ sync.bat", "Leyline HQ: fetch the owner's notes to Claude (and sync)", False),
    # The commons kit (tools/commons, docs/BLANK-SLATE.md): the directory and "Make my own Leyline" check themselves.
    "commons-check": ("tools/commons/Commons check.bat", "Commons kit check: the directory and Make my own Leyline's tests (read-only, a temp folder)", False),
    # The reel kit (tools/reels, docs/growth/README.md; owner 4 Nov: reels showing features in Leyline's style, a launch
    # video kept up to date, flow videos). reel-record plays the takes on this PC through the launcher's own tests (your
    # game, Your Den: saved first and put back) and records them with ffmpeg; reel-record-hand shows a box per take you
    # play yourself (OK starts it, it stops by itself); reel-make cuts everything recorded into the finished videos.
    # Nothing is ever posted: the videos wait in tools\reels\out for you.
    "reel-record": ("tools/reels/Reel record.bat", "Reels: record the takes (the launcher's own tests start your game: PLAY into the world, Your Den, carry a piece, free flight; the launcher's pages if node is here) into tools\\reels\\takes", False),
    "reel-record-hand": ("tools/reels/Reel record - hand takes.bat", "Reels: the takes you play yourself (a community realm, Single player with bots, HD before and after): a box says what to do, OK records a set time", False),
    "reel-make": ("tools/reels/Reel make.bat", "Reels: cut the recorded takes into the reels, flow videos and the launch video (9:16 and 16:9 MP4s, a GIF for Reddit) in tools\\reels\\out; nothing is posted", False),
}
# Steps that change nothing in production. With "Ask before deploys" on (control/autopilot.json "confirm"),
# How long a step may run before Control stops it (review 4 Nov: a step waiting on a question, such as an ssh asking
# about a host key, blocked every later step and Autopilot until the window was closed). Minutes; the long builds,
# exports and uploads get more, and LEYLINE_STEP_TIMEOUT_MIN changes the default.
STEP_TIMEOUT_MIN = {"client-build": 360, "slice": 360, "slice-sampler": 360, "sampler": 240, "sv-record": 180,
                    "sv-export": 240, "client": 120, "ship": 120, "ship-again": 120, "leyline": 120,
                    "leyline-launcher": 120, "release-mirror": 180, "audit": 60, "den-test-variants": 120,
                    "fresh-test": 180, "fresh-sandbox": 180, "first-run-test": 180, "mods-daily": 180, "thumbs": 120,
                    "addons-curate": 120, "ov112-data-remove": 120, "foreverui": 120}


def step_timeout_s(step: str) -> int:
    try:
        default = int(os.environ.get("LEYLINE_STEP_TIMEOUT_MIN") or 90)
    except ValueError:
        default = 90
    return max(1, STEP_TIMEOUT_MIN.get(step, default)) * 60


def kill_tree(p) -> None:
    """Stop a step's process and everything it started (cmd -> bat -> ssh, python, ...)."""
    pid = getattr(p, "pid", None)
    if os.name == "nt" and pid:
        subprocess.run(["taskkill", "/T", "/F", "/PID", str(pid)], capture_output=True,
                       creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    try:
        p.kill()
    except (OSError, AttributeError):
        pass


def watch_step(p, step: str, why: dict) -> threading.Event:
    """A watchdog for one running step: it stops the step at its time limit, or when Stop is pressed
    (state["stop"]); `why` says which. Set the returned event when the step ends by itself."""
    done = threading.Event()
    limit = time.monotonic() + step_timeout_s(step)

    def watch():
        while not done.wait(1.0):
            if state.get("stop"):
                why["reason"] = "stopped"
            elif time.monotonic() > limit:
                why["reason"] = "timeout"
            else:
                continue
            kill_tree(p)
            return
    threading.Thread(target=watch, daemon=True).start()
    return done


# Autopilot runs only these by itself; anything else waits for "Launch all".
READ_ONLY = {"check", "combos-check", "ov112-check", "ov112-realm-check", "build-realm-check", "ghclean-check", "sampler-check", "brand-check", "builds-why",
             "fresh-check", "fresh-leyline-check", "retag-leyline-check", "hq-sync", "hqnote", "hq-inbox", "firewall-check", "caddy-check", "ov112-data-check", "ov112-data-remove-check", "worldlog", "tidy-check",
             "vr-data-check", "vr-site-check", "sv-site-check", "release-mirror-check", "fork-check", "bugs", "audit", "den-check", "domain-check",
             "discord-preview", "discord-check", "discord-read", "apply-check", "feedback", "reel-make", "announce-check"}
# Steps only the owner starts, by hand, from their own panel (never Autopilot, Launch all or a Waiting on you button)
OWNER_ONLY = {"discord-apply", "discord-announce"}
HQ_STEPS = {"hq-sync", "hqnote", "hq-inbox"}

# Never in a log or on the page: passwords, tokens, keys, connection strings (scrub() on every line a step prints).
SECRET_RES = [
    # review 4 Nov: also *_KEY names (RELAY_BLOCK_FEED_KEY, HMAC_KEY...) and JSON's "token": "..." (the quote before the colon)
    (re.compile(r"(?i)\b([A-Z0-9_]*(?:PASSWORD|PASSWD|PASS|TOKEN|SECRET|API_?KEY|PRIVATE_?KEY|_KEY)(?:_[A-Z0-9_]*)?[\"']?\s*[=:]\s*)(\"[^\"]*\"|'[^']*'|\S+)"), r"\1[hidden]"),
    (re.compile(r"\bGOCSPX-[A-Za-z0-9_-]{10,}"), "[hidden]"),                 # a Google OAuth client secret
    (re.compile(r"\bAKIA[0-9A-Z]{16}\b"), "[hidden]"),                      # an AWS access key id
    (re.compile(r"\bsk-(?:ant-)?[A-Za-z0-9_-]{20,}"), "[hidden]"),           # API keys of the sk- shape
    (re.compile(r"(?i)(discord(?:app)?\.com/api/webhooks/\d+/)[A-Za-z0-9_-]+"), r"\1[hidden]"),   # a webhook's secret half
    (re.compile(r"\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}"), "[hidden]"),
    (re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}"), "[hidden]"),
    (re.compile(r"(?i)\b(Bearer|Basic)\s+[A-Za-z0-9._~+/=-]{12,}"), r"\1 [hidden]"),
    (re.compile(r"(?i)\b((?:mysql|postgres(?:ql)?|mongodb|redis|amqp|https?)://[^:/\s@]+:)[^@\s]+@"), r"\1[hidden]@"),
    (re.compile(r"(?i)(DatabaseInfo\s*=\s*\"?[^;\"]*;[^;\"]*;[^;\"]*;)[^;\"]*"), r"\1[hidden]"),
    (re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----.*"), "[hidden private key]"),
    # a Discord bot token's shape (the saved one is also hidden by its exact text, below)
    (re.compile(r"\b[MNO][A-Za-z0-9_-]{22,30}\.[A-Za-z0-9_-]{5,8}\.[A-Za-z0-9_-]{25,}"), "[hidden]"),
]


ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")   # terminal colours (the server's logs): shown as junk on the page


_dsecret = {"at": None, "tok": ""}


def discord_secret() -> str:
    """The saved Discord bot token, read again only when token.txt changes (scrub() hides it in every line)."""
    try:
        st = DISCORD_TOKEN.stat()
        at = (st.st_mtime_ns, st.st_size)
    except OSError:
        return ""
    if _dsecret["at"] != at:
        try:
            _dsecret["tok"] = DISCORD_TOKEN.read_text(encoding="utf-8-sig").strip()
        except OSError:
            _dsecret["tok"] = ""
        _dsecret["at"] = at
    return _dsecret["tok"]


def scrub(line: str) -> str:
    line = ANSI.sub("", line)
    tok = discord_secret()
    if len(tok) >= 8 and tok in line:
        line = line.replace(tok, "[hidden]")
    for rx, rep in SECRET_RES:
        line = rx.sub(rep, line)
    return line
CHECKS = [
    ("Play page", "https://leylinelauncher.com/wrath"),
    ("Game files", "https://play.leylinelauncher.com/data/__tier/index.json"),
    ("Client", "https://play.leylinelauncher.com/boot-manifest.json"),
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
        if sid not in notes:
            # the lines run_all adds by itself after a ship (review 4 Nov: they showed as "?" with no log)
            if sid.endswith("-leyline-launcher"):
                step, note = "leyline-launcher", "follows the Leyline repo"
            elif sid.endswith("-leyline"):
                step, note = "leyline", "follows Save to GitHub"
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
    # checked and claimed under the lock that start_run takes, so a step starting in the same second is never killed by
    # the restart (review 4 Nov): once claimed, "running" says restarting and nothing else can start
    with lock:
        try:
            if SELF.stat().st_mtime == SELF_MTIME or state["running"]:
                return
        except OSError:
            return
        state["running"] = {"id": "", "step": "restarting", "title": "Restarting Leyline Control"}
    emit("line", "== The control window was updated: restarting it (a few seconds).")
    flags = getattr(subprocess, "CREATE_NEW_CONSOLE", 0)
    subprocess.Popen([sys.executable, str(SELF), "--quiet", "--wait-port"], cwd=str(ROOT), creationflags=flags)
    time.sleep(0.5)
    os._exit(0)


def autopilot_safe(items: list) -> list:
    """With "Ask before deploys" on: the read-only steps at the front of the queue, up to the first one that isn't."""
    safe = []
    for it in items:
        if it["step"] not in READ_ONLY:
            break
        safe.append(it)
    return safe


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
                safe = autopilot_safe(items)
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
            if item["step"] in OWNER_ONLY and not item.get("owner_confirmed"):
                what = "the draft (Pending announcements)" if item["step"] == "discord-announce" else "the preview"
                emit("line", f"!! '{item['step']}' runs only from Leyline Control's Discord panel, after you read "
                             f"{what} and type YES there. It was not run.")
                emit("result", {"ok": False, "step": item["step"]})
                if record:
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
            if item["step"] == "audit" and not item.get("open"):
                env["LEYLINE_AUDIT_NO_OPEN"] = "1"      # only the Release audit button opens the report by itself
            if interactive:
                env["WU_SHIP_AUTO"] = env["LEYLINE_SHIP_AUTO"] = "1"
                note = (ship_message or "").replace("!", "").replace("%", "").replace('"', "'").strip()
                for k in ("WU_SHIP_MSG", "LEYLINE_SHIP_MSG"):
                    env.pop(k, None)
                if note:
                    env["WU_SHIP_MSG"] = env["LEYLINE_SHIP_MSG"] = note
            for k in ("LEYLINE_CONTROL_STEP", "LEYLINE_DISCORD_EXPECT_CHANGES"):
                env.pop(k, None)
            # which line this is (quick.py finds its manifest by it: control/quick/<id>.json, or "quick:<name>" in the note)
            env["LEYLINE_CONTROL_ID"] = str(item.get("id") or "")
            env["LEYLINE_CONTROL_NOTE"] = str(item.get("note") or "")[:200]
            for k in ("LEYLINE_DISCORD_OLD", "LEYLINE_DISCORD_PINS", "LEYLINE_ANNOUNCE_ID", "LEYLINE_ANNOUNCE_SHA"):
                env.pop(k, None)
            if item["step"].startswith("discord-"):
                env["LEYLINE_CONTROL_STEP"] = item["step"]
                env["PYTHONIOENCODING"] = "utf-8"
                # the card's choices: a preview uses the ones set now, Do it the ones its preview used
                if item["step"] == "discord-preview":
                    item["options"] = discord_options()
                opts = clean_discord_options(item.get("options"))
                env["LEYLINE_DISCORD_OLD"] = opts["old"]
                env["LEYLINE_DISCORD_PINS"] = "1" if opts["pins"] else "0"
                if item["step"] == "discord-apply":
                    env["LEYLINE_DISCORD_EXPECT_CHANGES"] = str(int(item["expect"]))
                if item["step"] == "discord-announce":     # the one draft the owner read and typed YES under
                    env["LEYLINE_ANNOUNCE_ID"] = str(item.get("announce_id") or "")
                    env["LEYLINE_ANNOUNCE_SHA"] = str(item.get("announce_sha") or "")
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
                why = {}
                state["stop"] = False
                ended = watch_step(p, item["step"], why)
                try:
                    for raw in iter(p.stdout.readline, b""):
                        line = scrub(raw.decode("utf-8", errors="replace").rstrip("\r\n"))
                        if line.strip().lower().startswith("press any key"):
                            continue
                        lf.write(line + "\n")
                        emit("line", line)
                    rc = p.wait()
                finally:
                    ended.set()
                if why.get("reason"):
                    rc = rc or 1
                    msg = ("!! Stopped by you." if why["reason"] == "stopped" else
                           f"!! Stopped: it ran past its {step_timeout_s(item['step']) // 60} minutes (it may be waiting "
                           "on a question, such as ssh asking about a host key). Run it by hand to see, or ask Claude.")
                    lf.write(msg + "\n")
                    emit("line", msg)
            if item["step"] == "check":
                write_last_check(item["id"], rc == 0)
            if item["step"].startswith("discord-"):
                discord_record(item, rc, log)
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
            # ... and the Leyline-Launcher repo gets the same version right after it (owner 2 Nov)
            if item["step"] == "leyline" and not any(q["step"] == "leyline-launcher" for q in queue[i:]):
                queue.insert(i, {"id": item["id"] + "-launcher", "step": "leyline-launcher",
                                 "title": STEPS["leyline-launcher"][1], "note": "follows the Leyline repo",
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


def clean_discord_token(raw) -> str:
    """The pasted bot token, tidied (quotes, a "Bot " in front), or "" when it can't be one."""
    t = str(raw or "").strip().strip('"').strip("'").strip()
    if t.lower().startswith("bot "):
        t = t[4:].strip()
    return t if 30 <= len(t) <= 200 and re.fullmatch(r"[A-Za-z0-9._-]+", t) else ""


def discord_token_status() -> dict:
    """Only whether a token is saved and its last 4 characters: the page never gets the token itself."""
    t = discord_secret()
    return {"saved": bool(t), "last4": t[-4:] if len(t) >= 30 else ""}


def save_discord_token(tok: str) -> None:
    DISCORD_TOKEN.parent.mkdir(parents=True, exist_ok=True)
    tmp = DISCORD_TOKEN.with_name("token.txt.tmp")
    tmp.write_text(tok + "\n", encoding="utf-8")
    try:
        os.chmod(tmp, 0o600)
    except OSError:
        pass
    os.replace(tmp, DISCORD_TOKEN)


def remove_discord_token() -> None:
    for f in (DISCORD_TOKEN, DISCORD_TOKEN.with_name("token.txt.tmp")):
        try:
            f.unlink()
        except FileNotFoundError:
            pass


DISCORD_OLD = ("archive", "empty", "all")     # old channels: archive them, delete the empty ones, delete them all


def clean_discord_options(o) -> dict:
    """The Discord card's choices, tidied: old channels (archive by default) and Post and pin (off by default)."""
    o = o if isinstance(o, dict) else {}
    old = o.get("old") if o.get("old") in DISCORD_OLD else "archive"
    return {"old": old, "pins": o.get("pins") is True}


def discord_options() -> dict:
    return clean_discord_options(discord_doc().get("options"))


def discord_doc() -> dict:
    try:
        d = json.loads(DISCORD_DOC.read_text(encoding="utf-8"))
        return d if isinstance(d, dict) else {}
    except (OSError, ValueError):
        return {}


def discord_summary(log: Path) -> dict:
    """What the preview's log says: the server, the number of changes, and each change by section (the lines
    discord_leyline.py's report prints: a "Section:" title, then "  * change" or "    note")."""
    out = {"server": "", "changes": None, "items": [], "error": ""}
    try:
        lines = log.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return out
    section, in_report = "", False
    for ln in lines:
        m = re.match(r"^PREVIEW \(nothing will change\) for the server (['\"])(.*)\1:$", ln)
        if m:
            out["server"], in_report = m.group(2), True
            continue
        m = re.match(r"^(\d+) change\(s\)\.$", ln)
        if m and in_report:
            out["changes"] = int(m.group(1))
            in_report = False
            continue
        if ln.startswith("Nothing to change") and in_report:
            out["changes"], in_report = 0, False
            continue
        if in_report:
            if ln and not ln.startswith(" ") and ln.endswith(":"):
                section = ln[:-1]
            elif ln.startswith("  * "):
                out["items"].append({"section": section, "text": ln[4:], "change": True})
            elif ln.startswith("    ") and ln.strip():
                out["items"].append({"section": section, "text": ln.strip(), "change": False})
        elif ln.startswith("Stopped") or "Discord didn't accept" in ln or ln.startswith("No bot token"):
            out["error"] = ln.strip()
    out["items"] = out["items"][:400]
    return out


def discord_record(item: dict, rc: int, log: Path) -> None:
    """After a Discord step: the preview's summary (for the panel and for Do it), or how the last do it / bot
    settings went. A successful do it uses the preview up, so the next one needs a new preview."""
    key = {"discord-preview": "preview", "discord-apply": "apply", "discord-check": "check",
           "discord-announce": "announce"}.get(item["step"])
    if not key:
        return
    d = discord_doc()
    rec = {"id": item["id"], "at": time.time(), "ok": rc == 0, "log": log.name}
    if key == "announce":
        rec["draft"] = str(item.get("announce_id") or "")
    if key == "preview":
        rec.update(discord_summary(log))
        rec["ok"] = rc == 0 and rec["changes"] is not None
        rec["options"] = clean_discord_options(item.get("options"))
    if key == "apply":
        rec["changes"] = item.get("expect")
        if rc == 0 and isinstance(d.get("preview"), dict):
            d["preview"]["used"] = True
    if key == "check":
        try:
            rec["lines"] = [l for l in log.read_text(encoding="utf-8", errors="replace").splitlines()
                            if l.startswith("  ")][:20]
        except OSError:
            rec["lines"] = []
    d[key] = rec
    try:
        DISCORD_DOC.parent.mkdir(parents=True, exist_ok=True)
        DISCORD_DOC.write_text(json.dumps(d, indent=1), encoding="utf-8")
    except OSError:
        pass


def discord_ready(d: dict | None = None) -> tuple[dict | None, str]:
    """The preview Do it would carry out, or why there is none."""
    p = (d if d is not None else discord_doc()).get("preview")
    if not discord_token_status()["saved"]:
        return None, "Save the bot token first."
    if not isinstance(p, dict) or not p.get("ok"):
        return None, "Run the preview first."
    if p.get("used"):
        return None, "That preview was done. Run the preview again."
    if time.time() - float(p.get("at") or 0) > DISCORD_FRESH:
        return None, "The preview is over an hour old. Run it again."
    if clean_discord_options(p.get("options")) != clean_discord_options((d if d is not None else discord_doc())
                                                                        .get("options")):
        return None, "You changed the choices since the preview. Run the preview again."
    if not p.get("changes"):
        return None, "Nothing to change: the server is already done."
    return p, ""


def discord_state() -> dict:
    d = discord_doc()
    p, why = discord_ready(d)
    out = {"token": discord_token_status(), "ready": bool(p), "why": why,
           "options": clean_discord_options(d.get("options"))}
    for k in ("preview", "apply", "check"):
        if isinstance(d.get(k), dict):
            r = dict(d[k])
            r["when"] = time.strftime("%d %b %H:%M", time.localtime(float(r.get("at") or 0)))
            out[k] = r
    out["announce"] = announce_state(d)
    return out


# ------------------------------------------------------------------ announcements (README "Announcements")
# Claude writes drafts into tools/discord-leyline/announce/pending/ on this PC; the Discord card lists each one exactly
# as it would post, checked by announce.py's own rules. Check and Reject happen here (no network); Post runs the
# discord-announce step with the draft's id and sha256, only after the owner typed YES under that draft.
_ANN = {}


def announce_mod():
    """tools/discord-leyline/announce.py, loaded once (it is the one place the rules live)."""
    if "mod" not in _ANN:
        import importlib.util
        try:
            spec = importlib.util.spec_from_file_location("leyline_announce", DISCORD_DIR / "announce.py")
            m = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(m)
            _ANN["mod"], _ANN["error"] = m, ""
        except Exception as e:                  # the card says so instead of breaking the page
            _ANN["mod"], _ANN["error"] = None, f"{type(e).__name__}: {e}"[:200]
    return _ANN["mod"]


def announce_view(dr: dict) -> dict:
    f = dr["front"]
    return {"id": dr["id"], "title": f.get("title", ""), "kind": f.get("kind", ""), "ping": f.get("ping", ""),
            "created": f.get("created", ""), "message": dr["message"], "length": len(dr["message"]),
            "limit": 2000, "problems": dr["problems"], "sha": dr["sha"]}


def announce_state(d: dict | None = None) -> dict:
    """The pending drafts for the card (at most 20), and how the last post went."""
    m = announce_mod()
    out = {"drafts": [], "error": _ANN.get("error", "")}
    if m is not None:
        try:
            out["drafts"] = [announce_view(x) for x in m.pending(ANNOUNCE_DIR)[:20]]
        except Exception as e:
            out["error"] = f"{type(e).__name__}: {e}"[:200]
    last = (d if d is not None else discord_doc()).get("announce")
    if isinstance(last, dict):
        r = dict(last)
        r["when"] = time.strftime("%d %b %H:%M", time.localtime(float(r.get("at") or 0)))
        out["last"] = r
    return out


def announce_load(draft_id: str):
    """(the draft, "") or (None, why)."""
    m = announce_mod()
    if m is None:
        return None, "announce.py didn't load: " + _ANN.get("error", "")
    if not m.valid_id(draft_id):
        return None, "That isn't a draft id."
    try:
        return m.load(draft_id, ANNOUNCE_DIR), ""
    except OSError:
        return None, "That draft isn't waiting any more (posted, rejected or removed). Refresh."


# ------------------------------------------------------------------ the kill switch (docs/KILL-SWITCH.md)
# Only from its card: never a step (not in STEPS), so never UPDATES-PENDING.txt, Autopilot, Launch all or /api/do.
_KS = {}
KS_FACTORY = None          # tests: a callable giving {"github", "discord", "server", "run_step"} (fakes)
KS_BACKUP_ROOT = None      # tests: where the backups go (else the Desktop)


def ks_mod():
    """tools/killswitch/killswitch.py, loaded once."""
    if "m" not in _KS:
        if str(KS_DIR) not in sys.path:
            sys.path.insert(0, str(KS_DIR))
        import killswitch   # noqa: PLC0415
        _KS["m"] = killswitch
    return _KS["m"]


def ks_doc() -> dict:
    try:
        d = json.loads(KS_DOC.read_text(encoding="utf-8"))
        return d if isinstance(d, dict) else {}
    except (OSError, ValueError):
        return {}


def ks_write(d: dict) -> None:
    KS_DOC.parent.mkdir(parents=True, exist_ok=True)
    tmp = KS_DOC.with_name("killswitch.json.tmp")
    tmp.write_text(json.dumps(d, indent=1, ensure_ascii=False), encoding="utf-8")
    os.replace(tmp, KS_DOC)


def ks_config(d: dict | None = None) -> dict:
    """The card's message and choices (the TODO(owner) text and every part ticked until the owner saves his)."""
    ks = ks_mod()
    c = (d if d is not None else ks_doc()).get("config")
    c = c if isinstance(c, dict) else {}
    return {"title": str(c.get("title") or ks.DEFAULT_TITLE)[:ks.TITLE_MAX],
            "message": str(c.get("message") if isinstance(c.get("message"), str) else ks.DEFAULT_MESSAGE)[:ks.MESSAGE_MAX],
            "parts": ks.clean_parts(c.get("parts") if isinstance(c.get("parts"), list) else ks.PARTS),
            "options": ks.clean_options(c.get("options"))}


def ks_paused(d: dict | None = None) -> dict | None:
    """The Pause that is on now (not undone, not deleted), or None."""
    p = (d if d is not None else ks_doc()).get("paused")
    return p if isinstance(p, dict) and not p.get("resumed") else None


def ks_adapters() -> dict:
    """The real services: gh on this PC, Discord with the Discord card's saved token, the server over SSH, and this
    window's own step runner."""
    ks = ks_mod()
    tok = discord_secret()
    return {"github": ks.GitHub() if shutil_which("gh") else None, "discord": ks.DiscordAPI(tok) if tok else None,
            "server": ks.Server(), "run_step": ks_run_step}


def shutil_which(name: str):
    import shutil   # noqa: PLC0415
    return shutil.which(name)


def ks_run_step(step: str) -> int:
    """One existing step for the kill switch (Pause's realm steps): its own log, its lines on the page."""
    if step not in STEPS or step in OWNER_ONLY:
        emit("line", f"!! {step}: not a step it may run")
        return 1
    bat = ROOT / STEPS[step][0]
    log = LOGS / f"killswitch-{time.strftime('%Y%m%d-%H%M%S')}-{step}.log"
    env = dict(os.environ, LEYLINE_CONTROL_ID=log.stem, LEYLINE_CONTROL_NOTE="kill switch")
    with open(log, "w", encoding="utf-8") as lf:
        p = subprocess.Popen(["cmd", "/c", "call", str(bat)], cwd=str(ROOT), env=env, stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                             creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        try:
            p.stdin.write(b"\n" * 8)
            p.stdin.close()
        except OSError:
            pass
        for raw in iter(p.stdout.readline, b""):
            line = scrub(raw.decode("utf-8", errors="replace").rstrip("\r\n"))
            if line.strip().lower().startswith("press any key"):
                continue
            lf.write(line + "\n")
            emit("line", "    " + line)
        return p.wait()


def ks_ctx(cfg: dict, log):
    ks = ks_mod()
    a = (KS_FACTORY or ks_adapters)()
    if a.get("discord") is not None:
        log.hide(discord_secret())
    return ks.Ctx(title=cfg["title"], message=cfg["message"], parts=cfg["parts"], options=cfg["options"], log=log,
                  github=a.get("github"), discord=a.get("discord"), server=a.get("server"), run_step=a.get("run_step"),
                  backup_root=KS_BACKUP_ROOT)


def ks_log(kind: str):
    ks = ks_mod()
    LOGS.mkdir(parents=True, exist_ok=True)
    log = ks.Log(LOGS / f"killswitch-{time.strftime('%Y%m%d-%H%M%S')}-{kind}.log", emit=lambda l: emit("line", l),
                 scrub=scrub)
    log.hide(discord_secret())
    return log


def ks_ready(stage: str, d: dict | None = None) -> tuple[dict | None, str]:
    """The preview a stage would carry out, or why there is none."""
    ks = ks_mod()
    d = d if d is not None else ks_doc()
    p = d.get("preview")
    paused = ks_paused(d)
    if stage == "resume" and (not paused or paused.get("deleted")):
        return None, "Nothing is paused." if not paused else "A Delete ran: there is nothing to bring back."
    if stage == "delete" and not paused:
        return None, "Delete comes only after a Pause."
    if not isinstance(p, dict) or p.get("stage") != stage:
        return None, "Run its preview first."
    if p.get("used"):
        return None, "That preview was used. Run it again."
    if time.time() - float(p.get("at") or 0) > ks.PREVIEW_FRESH:
        return None, "The preview is over an hour old. Run it again."
    c = ks_config(d)
    if p.get("key") != ks.preview_key(stage, c["title"], c["message"], c["parts"], c["options"]):
        return None, "You changed the message or the choices since the preview. Run it again."
    if p.get("problems"):
        return None, "The preview found problems: fix them, then run it again."
    return p, ""


def ks_state() -> dict:
    ks = ks_mod()
    d = ks_doc()
    paused = ks_paused(d)
    out = {"config": ks_config(d), "parts": [{"key": k, "title": ks.PART_TITLES[k], "deletable": k in ks.DELETABLE}
                                              for k in ks.PARTS],
           "phrases": ks.PHRASES, "ready": {}, "why": {}, "delete_wait_h": ks.DELETE_WAIT // 3600}
    for st in ("pause", "resume", "delete"):
        p, why = ks_ready(st, d)
        out["ready"][st], out["why"][st] = bool(p), why
    p = d.get("preview")
    if isinstance(p, dict):
        out["preview"] = {k: p.get(k) for k in ("id", "stage", "items", "problems", "log", "used", "options", "parts")}
        out["preview"]["when"] = _when(p.get("at"))
    if paused:
        out["paused"] = {"id": paused.get("id"), "when": _when(paused.get("at")), "parts": paused.get("parts"),
                         "ok": paused.get("ok"), "problems": paused.get("problems") or [],
                         "deleted": bool(paused.get("deleted")),
                         "delete_opens": _when(float(paused.get("at") or 0) + ks.DELETE_WAIT)}
    if isinstance(d.get("last"), dict):
        out["last"] = dict(d["last"], when=_when(d["last"].get("at")))
    return out


def ks_state_safe() -> dict:
    try:
        return ks_state()
    except Exception as e:   # noqa: BLE001 (the rest of the page still works)
        return {"error": f"{type(e).__name__}"}


def ks_task(title: str, fn) -> bool:
    with lock:
        if state["running"]:
            return False
        state["running"] = {"id": "killswitch", "step": "killswitch", "title": title}
        state["lines"] = []

    def go():
        try:
            fn()
        except Exception as e:   # noqa: BLE001 (never a dead window; the reason on the page)
            emit("line", f"!! Kill switch stopped: {type(e).__name__}: {scrub(str(e))[:300]}")
        finally:
            with lock:
                state["running"] = None
            emit("state", {})
    threading.Thread(target=go, daemon=True).start()
    return True


def ks_preview_run(stage: str) -> None:
    ks = ks_mod()
    d = ks_doc()
    cfg = ks_config(d)
    log = ks_log(f"preview-{stage}")
    log(f"== Kill switch: preview of {stage} (it only reads)")
    ctx = ks_ctx(cfg, log)
    p = ks.preview(ctx, stage, ks_paused(d))
    p["log"] = log.path.name
    for i in p["items"]:                 # what the page and killswitch.json keep: the same hiding as the log
        i["text"] = log.clean(i["text"])
    p["problems"] = [log.clean(x) for x in p["problems"]]
    d = ks_doc()
    d["preview"] = p
    ks_write(d)
    log(f"== Preview done: {len(p['problems'])} problem(s)")


def ks_go_run(stage: str, pid: str) -> None:
    ks = ks_mod()
    d = ks_doc()
    p, why = ks_ready(stage, d)
    if not p or p["id"] != pid:
        emit("line", f"!! Kill switch: {why or 'that preview is not the latest'}")
        return
    d["preview"]["used"] = True          # one press, one run
    ks_write(d)
    cfg = ks_config(d)
    log = ks_log(stage)
    ctx = ks_ctx(cfg, log)
    last = {"stage": stage, "at": time.time(), "log": log.path.name, "dry": cfg["options"]["dry"], "ok": False}
    try:
        if stage == "pause":
            rec = ks.run_pause(ctx, ks.new_id())
            rec["problems"] = [log.clean(x) for x in rec["problems"]]
            d = ks_doc()
            if not rec["dry"]:
                d["paused"] = ks.merge_pause(ks_paused(d), rec)
            last.update(ok=rec["ok"], problems=rec["problems"])
        elif stage == "resume":
            paused = ks_paused(d)
            out = ks.run_resume(ctx, paused)
            d = ks_doc()
            if out["ok"]:
                paused["resumed"] = out
                d.setdefault("history", []).append(paused)
                d["paused"] = None
            last.update(ok=out["ok"], problems=out["problems"], turn_back_on=out["turn_back_on"])
        else:
            paused = ks_paused(d)
            out = ks.run_delete(ctx, ks.new_id(), paused)
            d = ks_doc()
            if out["ok"]:
                d["paused"]["deleted"] = out
                d["deleted"] = out
            last.update(ok=out["ok"], manual=out["manual"], backup=out["backup"])
    except (ks.Refused, RuntimeError) as e:
        log(f"!! Stopped: {e}")
        d = ks_doc()
        last.update(ok=False, problems=[str(e)])
    last["problems"] = [log.clean(str(x)) for x in last.get("problems") or []]
    d["last"] = last
    ks_write(d)


def audit_state() -> dict | None:
    """The last release audit: when, and its result line (logs/audit-latest.txt's second line), or None."""
    try:
        at = AUDIT_HTML.stat().st_mtime
        lines = AUDIT_TXT.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return None
    result = lines[1][len("Result: "):] if len(lines) > 1 and lines[1].startswith("Result: ") else ""
    return {"at": time.strftime("%d %b %H:%M", time.localtime(at)), "result": result, "green": result.startswith("ALL GREEN")}


# ------------------------------------------------------------------ the Discord & GM card (read-only)
# Quick actions: existing steps only, each on a click (never Autopilot; discord-apply stays on the Discord panel).
GM_ACTIONS = [("check", "Check server"), ("bugs", "Pull bug reports"), ("discord-check", "Discord bot settings"),
              ("discord-preview", "Discord preview"), ("worldlog", "World server log"), ("mods-daily", "Find new mods (approves nothing)")]
OPEN_BUGS = ("new", "triaged", "planned")


def _read_json(p: Path):
    try:
        return json.loads(p.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return None


def _when(t) -> str:
    try:
        return time.strftime("%d %b %H:%M", time.localtime(float(t)))
    except (TypeError, ValueError, OverflowError, OSError):
        return ""


def bugs_overview(n: int = 8) -> dict:
    """The open bug reports from the bugs step's backlog, newest first: id, title, status, how many, where from."""
    b = _read_json(BUGS_BACKLOG)
    items = b.get("items") if isinstance(b, dict) and isinstance(b.get("items"), list) else None
    if items is None:
        return {"have": False, "open": 0, "items": [], "at": ""}
    live = [it for it in items if isinstance(it, dict) and it.get("status") in OPEN_BUGS and not it.get("merged_into")]
    live.sort(key=lambda it: -(it.get("last") or 0))
    out = [{"id": str(it.get("id", ""))[:20], "title": scrub(str(it.get("title", "")))[:140], "status": it.get("status"),
            "count": int(it.get("count") or 0), "sources": [str(x)[:10] for x in (it.get("sources") or [])][:4],
            "last": _when(it.get("last"))} for it in live[:n]]
    return {"have": True, "open": len(live), "items": out, "at": _when(BUGS_BACKLOG.stat().st_mtime)}


def _pending_review(entries) -> list:
    return [e for e in entries if isinstance(e, dict) and isinstance(e.get("review"), dict) and not e["review"].get("by")]


def mods_overview(n: int = 6) -> dict:
    """What waits for the owner's review: curated addons and looks with no reviewer yet. Never approves anything."""
    up = _read_json(UPSTREAM)
    looks = _read_json(LOOKS)
    addons = _pending_review(up.get("entries", [])) if isinstance(up, dict) else []
    lk = _pending_review(looks.get("looks", [])) if isinstance(looks, dict) else []
    name = lambda e: scrub(str(e.get("display_name") or e.get("name") or e.get("id") or ""))[:60]
    return {"addons": len(addons), "addon_names": [name(e) for e in addons[:n]], "looks": len(lk), "look_names": [name(e) for e in lk[:n]]}


def bridge_overview() -> dict:
    """The Discord bridge on the server (/opt/wu-discord, the wu-discord service), as the last server check saw it."""
    try:
        text = SERVER_CHECK.read_text(encoding="utf-8", errors="replace")
        at = _when(SERVER_CHECK.stat().st_mtime)
    except OSError:
        return {"seen": False, "state": "", "enabled": "", "lines": [], "at": ""}
    lines, on = [], False
    for ln in text.splitlines():
        if ln.startswith("== "):
            on = ln.startswith("== Discord bridge")
            continue
        if ln.startswith("DIAG OK"):
            on = False
        if on and ln.strip():
            lines.append(scrub(ANSI.sub("", ln.rstrip()))[:200])
    m = re.search(r"state: (\S+); enabled: (\S+)", "\n".join(lines))
    return {"seen": bool(lines), "state": m.group(1) if m else "", "enabled": m.group(2) if m else "",
            "lines": [l for l in lines if not l.lstrip().startswith("state:")][:8], "at": at}


def quick_overview(n: int = 5) -> list:
    """The last quick edits applied (tools/control/quick.py): when, which manifest, its deploy, ok."""
    out = []
    for ln in read_lines(QUICK_LOG)[-n:]:
        try:
            d = json.loads(ln)
        except ValueError:
            continue
        if isinstance(d, dict):
            out.append({"at": str(d.get("at", ""))[:20], "id": str(d.get("id", ""))[:64], "deploy": str(d.get("deploy", ""))[:10],
                        "files": len(d.get("files") or []), "ok": bool(d.get("ok")), "why": scrub(str(d.get("why", "")))[:200]})
    return list(reversed(out))


def gm_overview() -> dict:
    """The Discord & GM card: the server, the Discord bridge and bot, open bug reports, what waits for review, the last
    quick edits, and the quick actions (existing steps, each on a click). Read-only: it only reads files."""
    with lock:
        health, at = dict(state["health"] or {}), state["health_at"]
    d = discord_state()
    return {"server": {"health": health, "at": _when(at) if at else "", "last_check": _read_json(LAST_CHECK)},
            "bridge": bridge_overview(),
            "discord": {"token": d["token"]["saved"], "preview": (d.get("preview") or {}).get("when", ""),
                        "check": (d.get("check") or {}).get("when", ""), "check_ok": (d.get("check") or {}).get("ok")},
            "bugs": bugs_overview(), "mods": mods_overview(), "quick": quick_overview(),
            "actions": [{"step": k, "label": v} for k, v in GM_ACTIONS if k in STEPS and k not in OWNER_ONLY]}


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


def _safe_name(n: str) -> str:
    n = re.sub(r"[^A-Za-z0-9._ -]+", "_", os.path.basename(str(n or "")))[:120].strip(" .")
    return n or "file"


def _safe_dir(d: str) -> Path:
    parts = [re.sub(r"[^A-Za-z0-9_-]+", "_", x)[:40] for x in str(d or "").replace("\\", "/").split("/") if x not in ("", ".", "..")]
    return INBOX.joinpath(*parts[:3]) if parts else INBOX


def save_answer(item_id: str, done: bool, text: str = "", files=None) -> None:
    """The owner's Done / answer / files for one Waiting on you item, for Claude (control/owner-answers.json)."""
    try:
        doc = json.loads(ANSWERS.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        doc = {}
    a = doc.setdefault(str(item_id)[:80], {})
    a["at"] = time.strftime("%Y-%m-%d %H:%M:%S")
    if done:
        a["done"] = True
    if text:
        a["text"] = text[:4000]
    if files:
        a["files"] = sorted(set(a.get("files", []) + list(files)))
    ANSWERS.parent.mkdir(parents=True, exist_ok=True)
    ANSWERS.write_text(json.dumps(doc, indent=1), encoding="utf-8")


def answers() -> dict:
    try:
        return json.loads(ANSWERS.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


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
                                               "autopilot": autopilot_on(), "confirm": confirm_on(),
                                               "audit": audit_state(), "answers": answers(),
                                               "discord": discord_state(), "killswitch": ks_state_safe()}))
        if path == "/api/gm":
            return self._send(200, json.dumps(gm_overview()))
        if path == "/audit":
            try:
                return self._send(200, AUDIT_HTML.read_bytes(), "text/html; charset=utf-8")
            except OSError:
                return self._send(404, "No release audit yet: press Release audit.", "text/plain")
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
        if self.path.startswith("/api/discord/"):
            return self._discord(n)
        if self.path.startswith("/api/killswitch/"):
            return self._killswitch(n)
        if self.path.startswith("/api/drop"):
            # a file for Claude: /api/drop?id=<item>&dir=<folder>&name=<file name>, the raw bytes as the body
            qs = urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query)
            if n <= 0 or n > DROP_MAX:
                return self._send(413, '{"error":"empty or over 200 MB"}')
            d = _safe_dir((qs.get("dir") or [""])[0])
            d.mkdir(parents=True, exist_ok=True)
            name = _safe_name((qs.get("name") or [""])[0])
            dest = d / name
            left = n
            with open(dest, "wb") as f:
                while left > 0:
                    chunk = self.rfile.read(min(left, 1 << 20))
                    if not chunk:
                        break
                    f.write(chunk)
                    left -= len(chunk)
            rel = dest.relative_to(ROOT).as_posix()
            save_answer((qs.get("id") or ["files"])[0], False, files=[rel])
            emit("state", {})
            return self._send(200, json.dumps({"ok": True, "saved": rel}))
        body = json.loads(self.rfile.read(n) or b"{}") if n else {}
        if self.path == "/api/launch":
            msg = str(body.get("message") or "")[:200].replace("\n", " ")
            state["auto_failed"] = None
            if not start_run(msg):
                return self._send(409, '{"error":"already running"}')
            return self._send(200, '{"ok":true}')
        if self.path == "/api/stop":   # the Stop button: the running step is stopped (its whole process tree)
            if not state.get("running"):
                return self._send(409, '{"error":"nothing is running"}')
            state["stop"] = True
            emit("line", "== Stopping the running step...")
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
        if self.path == "/api/audit":
            item = {"id": "audit-" + time.strftime("%Y%m%d-%H%M%S"), "step": "audit",
                    "title": STEPS["audit"][1], "note": "", "known": True, "open": True}
            if not start_run("", [item], False, "Release audit"):
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
        if self.path == "/api/answer":
            save_answer(str(body.get("id") or "")[:80] or "note", bool(body.get("done")), str(body.get("text") or ""))
            emit("state", {})
            return self._send(200, '{"ok":true}')
        if self.path == "/api/do":
            # a Waiting on you button: run its step now, open its page, or open its folder
            step = str(body.get("run") or "")
            if step:
                if step not in STEPS:
                    return self._send(400, '{"error":"unknown step"}')
                if step in OWNER_ONLY:
                    return self._send(400, '{"error":"that one runs only from the Discord panel, after you type YES"}')
                item = {"id": step + "-" + time.strftime("%Y%m%d-%H%M%S"), "step": step, "title": STEPS[step][1],
                        "note": "", "known": True, "open": True}
                if not start_run("", [item], False, STEPS[step][1]):
                    return self._send(409, '{"error":"already running"}')
                return self._send(200, '{"ok":true}')
            url = str(body.get("open") or "")
            if url:
                if not url.startswith("https://"):
                    return self._send(400, '{"error":"https links only"}')
                webbrowser.open(url)
                return self._send(200, '{"ok":true}')
            folder = str(body.get("folder") or "")
            if folder:
                f = (ROOT / folder).resolve() if folder != "inbox" else INBOX
                if ROOT not in f.parents and f != ROOT and f != INBOX:
                    return self._send(400, '{"error":"only folders in Leyline"}')
                f.mkdir(parents=True, exist_ok=True)
                if os.name == "nt":
                    os.startfile(str(f))   # noqa: S606 (the owner's own Explorer)
                return self._send(200, '{"ok":true}')
            return self._send(400, '{"error":"nothing to do"}')
        if self.path == "/api/open-game":
            webbrowser.open("https://play.openvanilla.app/wrath")
            return self._send(200, '{"ok":true}')
        self._send(404, '{"error":"not found"}')


    def _discord(self, n: int):
        """The Discord panel: save / remove the bot token, and Do it. Stricter than the other POSTs: the session token
        in the X-Token header only (never the cookie or the link), a JSON body, same-origin, small."""
        if not token_ok(self.headers.get("X-Token", "")):
            return self._send(403, '{"error":"forbidden"}')
        if self.headers.get("Sec-Fetch-Site", "same-origin") not in ("same-origin", "none"):
            return self._send(403, '{"error":"forbidden"}')
        if not (self.headers.get("Content-Type") or "").startswith("application/json"):
            return self._send(415, '{"error":"json only"}')
        if n > 4096:
            return self._send(413, '{"error":"too big"}')
        try:
            body = json.loads(self.rfile.read(n) or b"{}") if n > 0 else {}
        except ValueError:
            return self._send(400, '{"error":"bad json"}')
        if not isinstance(body, dict):
            return self._send(400, '{"error":"bad json"}')
        if self.path == "/api/discord/token":
            tok = clean_discord_token(body.get("token"))
            body.clear()
            if not tok:
                return self._send(400, json.dumps({"error": "That doesn't look like a bot token. In the Developer "
                                                   "Portal: your app > Bot > Reset Token > Copy, then paste it here."}))
            try:
                save_discord_token(tok)
            except OSError as e:
                return self._send(500, json.dumps({"error": f"Couldn't save it ({type(e).__name__})."}))
            emit("state", {})
            return self._send(200, json.dumps({"ok": True, "token": discord_token_status()}))
        if self.path == "/api/discord/token-remove":
            remove_discord_token()
            emit("state", {})
            return self._send(200, json.dumps({"ok": True, "token": discord_token_status()}))
        if self.path == "/api/discord/options":
            if body.get("old") not in DISCORD_OLD or not isinstance(body.get("pins"), bool):
                return self._send(400, json.dumps({"error": "Choose what happens to old channels, and Post and pin "
                                                            "on or off."}))
            d = discord_doc()
            d["options"] = clean_discord_options(body)
            try:
                DISCORD_DOC.parent.mkdir(parents=True, exist_ok=True)
                DISCORD_DOC.write_text(json.dumps(d, indent=1), encoding="utf-8")
            except OSError as e:
                return self._send(500, json.dumps({"error": f"Couldn't save it ({type(e).__name__})."}))
            emit("state", {})
            return self._send(200, json.dumps({"ok": True, "options": d["options"]}))
        if self.path == "/api/discord/apply":
            p, why = discord_ready()
            if not p:
                return self._send(409, json.dumps({"error": why}))
            if str(body.get("confirm") or "").strip() != "YES":
                return self._send(400, json.dumps({"error": "Type YES to go ahead."}))
            if str(body.get("preview") or "") != p["id"]:
                return self._send(409, json.dumps({"error": "There is a newer preview. Read it, then type YES again."}))
            item = {"id": "discord-apply-" + time.strftime("%Y%m%d-%H%M%S"), "step": "discord-apply",
                    "title": STEPS["discord-apply"][1], "note": f"after preview {p['id']}", "known": True,
                    "owner_confirmed": True, "expect": int(p["changes"]),
                    "options": clean_discord_options(p.get("options"))}
            if not start_run("", [item], False, "Discord redo"):
                return self._send(409, '{"error":"already running"}')
            return self._send(200, '{"ok":true}')
        if self.path in ("/api/discord/announce-check", "/api/discord/announce-reject", "/api/discord/announce-post"):
            dr, why = announce_load(str(body.get("id") or ""))
            if not dr:
                return self._send(409, json.dumps({"error": why}))
            if self.path == "/api/discord/announce-check":        # no network: the same rules announce.py posts by
                return self._send(200, json.dumps({"ok": not dr["problems"], "draft": announce_view(dr)}))
            if self.path == "/api/discord/announce-reject":
                try:
                    dest = announce_mod().reject(dr["id"], ANNOUNCE_DIR, str(body.get("why") or "")[:200])
                except OSError as e:
                    return self._send(500, json.dumps({"error": f"Couldn't move it ({type(e).__name__})."}))
                emit("state", {})
                return self._send(200, json.dumps({"ok": True, "moved": dest.name}))
            # Post: a token, a draft with no problems, this very draft as the owner read it, and a typed YES
            if not discord_token_status()["saved"]:
                return self._send(409, json.dumps({"error": "Save the bot token first."}))
            if dr["problems"]:
                return self._send(409, json.dumps({"error": "This draft can't be posted yet: " + " ".join(dr["problems"])}))
            if str(body.get("confirm") or "").strip() != "YES":
                return self._send(400, json.dumps({"error": "Type YES under the draft to post it."}))
            if str(body.get("sha") or "") != dr["sha"]:
                return self._send(409, json.dumps({"error": "The draft changed since you read it. Read it again, then "
                                                            "type YES again."}))
            item = {"id": "discord-announce-" + time.strftime("%Y%m%d-%H%M%S"), "step": "discord-announce",
                    "title": STEPS["discord-announce"][1], "note": f"draft {dr['id']}", "known": True,
                    "owner_confirmed": True, "announce_id": dr["id"], "announce_sha": dr["sha"]}
            if not start_run("", [item], False, "Discord announcement"):
                return self._send(409, '{"error":"already running"}')
            return self._send(200, '{"ok":true}')
        return self._send(404, '{"error":"not found"}')


    def _strict_body(self, n: int, limit: int):
        """The owner's own panels (Discord, kill switch): the session token in the X-Token header only (never the
        cookie or the link), same-origin, a JSON object, small. Returns the body, or None after answering."""
        if not token_ok(self.headers.get("X-Token", "")):
            self._send(403, '{"error":"forbidden"}')
            return None
        if self.headers.get("Sec-Fetch-Site", "same-origin") not in ("same-origin", "none"):
            self._send(403, '{"error":"forbidden"}')
            return None
        if not (self.headers.get("Content-Type") or "").startswith("application/json"):
            self._send(415, '{"error":"json only"}')
            return None
        if n > limit:
            self._send(413, '{"error":"too big"}')
            return None
        try:
            body = json.loads(self.rfile.read(n) or b"{}") if n > 0 else {}
        except ValueError:
            body = None
        if not isinstance(body, dict):
            self._send(400, '{"error":"bad json"}')
            return None
        return body

    def _killswitch(self, n: int):
        """The kill switch card (docs/KILL-SWITCH.md): save the message and choices, preview a stage, and go (after
        that preview, with its typed phrase)."""
        body = self._strict_body(n, 16384)
        if body is None:
            return
        ks = ks_mod()
        if self.path == "/api/killswitch/save":
            t, m, parts, o = body.get("title"), body.get("message"), body.get("parts"), body.get("options")
            if not (isinstance(t, str) and len(t) <= ks.TITLE_MAX and isinstance(m, str) and len(m) <= ks.MESSAGE_MAX
                    and isinstance(parts, list) and all(x in ks.PARTS for x in parts) and isinstance(o, dict)
                    and all(isinstance(o.get(k, False), bool) for k in ("private", "dry"))):
                return self._send(400, json.dumps({"error": f"A title up to {ks.TITLE_MAX} characters, a message up "
                                                            f"to {ks.MESSAGE_MAX}, and the parts ticked."}))
            d = ks_doc()
            d["config"] = {"title": " ".join(t.split()), "message": m.replace("\r\n", "\n"), "parts": ks.clean_parts(parts),
                           "options": ks.clean_options(o)}
            ks_write(d)
            emit("state", {})
            return self._send(200, json.dumps({"ok": True, "config": ks_config(d)}))
        stage = body.get("stage")
        if stage not in ("pause", "resume", "delete"):
            return self._send(400, '{"error":"pause, resume or delete"}')
        if self.path == "/api/killswitch/preview":
            if stage == "resume" and not ks_paused():
                return self._send(409, json.dumps({"error": "Nothing is paused."}))
            if not ks_task(f"Kill switch: preview {stage}", lambda: ks_preview_run(stage)):
                return self._send(409, '{"error":"already running"}')
            return self._send(200, '{"ok":true}')
        if self.path == "/api/killswitch/go":
            p, why = ks_ready(stage)
            if not p:
                return self._send(409, json.dumps({"error": why}))
            if str(body.get("phrase") or "").strip() != ks.PHRASES[stage]:
                return self._send(400, json.dumps({"error": f"Type {ks.PHRASES[stage]} exactly."}))
            if str(body.get("preview") or "") != p["id"]:
                return self._send(409, json.dumps({"error": "There is a newer preview. Read it, then type again."}))
            if not ks_task(f"Kill switch: {stage}", lambda: ks_go_run(stage, p["id"])):
                return self._send(409, '{"error":"already running"}')
            return self._send(200, '{"ok":true}')
        return self._send(404, '{"error":"not found"}')


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
.dos{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-top:8px}
.dos button.sm{font-size:15px}
.drop{display:inline-flex;align-items:center;min-height:36px;padding:6px 14px;border:1px dashed var(--gold2);border-radius:8px;cursor:pointer;
color:var(--gold);background:rgba(0,0,0,.35);font-weight:600}.drop.over{background:rgba(241,207,134,.15)}
.dos textarea{font:inherit;flex:1 1 100%;min-height:52px;background:rgba(0,0,0,.5);border:1px solid #0a0908;border-radius:7px;color:var(--text);padding:8px 11px}
.youbox h2+p{margin:0 0 6px;color:var(--dim);font-size:12.5px}
#dtok.ok{color:var(--ok)}#dtok.no{color:var(--warn)}
.dsum{margin-top:10px}.dsum ul{margin:4px 0 0;padding-left:18px;max-height:260px;overflow:auto}.dsum li.n{color:var(--dim);list-style:none}
.dsum .sec{font-weight:600;margin-top:6px}.ann{margin-top:12px}.ann>.sec{font-weight:600}
.anitem{margin-top:10px;padding:10px;border:1px solid var(--edge);border-radius:8px}
.anmsg{height:auto;max-height:320px;white-space:pre-wrap;overflow-wrap:anywhere;margin:8px 0}.anbad{margin:4px 0;padding-left:18px;color:var(--bad)}
.dgo{margin-top:10px;padding:10px;border:1px solid #8a6a2c;border-radius:8px;background:rgba(242,201,76,.06)}
.dgo input{max-width:140px;flex:0 1 140px}
.dopt{margin-top:10px;display:flex;flex-direction:column;gap:6px}.dopt select{font:inherit;background:rgba(0,0,0,.5);color:var(--text);border:1px solid #0a0908;border-radius:7px;padding:4px 8px}
.dopt .chk{display:flex;gap:8px;align-items:center}
.gml{margin:4px 0 0;padding-left:18px;max-height:200px;overflow:auto}.gml li{margin:2px 0}
#killswitch{border-image:none;border:2px solid #6b2620;border-radius:10px}#killswitch summary{color:var(--bad)}
.ks textarea{font:inherit;width:100%;min-height:120px;background:rgba(0,0,0,.5);border:1px solid #0a0908;border-radius:7px;color:var(--text);padding:8px 11px}
.ks .chk{display:flex;gap:8px;align-items:center;margin:3px 0}.ks .chk input{flex:none;min-width:0;width:auto;box-shadow:none}.ks .parts{margin:8px 0}.ks li.problem{color:var(--bad)}
.ks li.manual{color:var(--warn)}.ks li.keep{color:var(--dim)}.ks .dgo{border-color:#8a2c22;background:rgba(255,131,120,.06)}
.ks .dgo input{max-width:260px;flex:0 1 260px}
</style></head><body class="ly">
<header><div class="brand"><span class="mark" aria-hidden="true"></span><h1>Leyline Control</h1></div>
<div class="health" id="health"></div>
<div class="actions"><button id="auto" title="While this window is open, Claude's updates run on their own">Autopilot: …</button>
<button id="audit" title="Checks every feature as a new player would (about 10 minutes, read-only), then opens the report">Release audit</button>
<a id="areport" class="sub" href="#" target="_blank" hidden></a>
<details class="more"><summary>More</summary><div class="menu">
<button id="hq" title="Leyline HQ: status, what needs you, releases, notes (play.openvanilla.app/wrath/admin)">Open HQ</button>
<button id="gmc" title="Opens the GM console (bug reports, players, health, logs) in your browser">GM console</button>
<button id="game">Open the game</button><button id="check">Check server</button>
<button id="ask" title="On: deploys wait for Launch all; read-only checks still run by themselves">Ask before deploys: …</button>
</div></details></div></header>
<section class="youbox"><h2>Waiting on you</h2><p>Everything Claude needs from you shows up here, with its button. Nothing else to look for.</p><div id="you"></div></section>
<main>
<div class="stack">
  <section class="card"><h2>Up next</h2><div id="pending"></div>
    <div class="launch"><input id="msg" placeholder="GitHub save note (only for 'Save to GitHub')">
    <button class="primary" id="go">Launch all</button></div></section>
  <section class="card" id="discord"><h2>Discord</h2>
    <div class="sub">The Discord launch layout: 13 channels with emoji, topics, permissions and pinned guides
      (tools\discord-leyline\README.md). Safe to run again: it only changes what differs. Bot token: <b id="dtok">…</b></div>
    <div class="launch"><input id="dtokin" type="password" autocomplete="off" spellcheck="false" aria-label="Discord bot token"
      placeholder="Discord bot token: paste it here"><button id="dsave">Save</button><button id="drm">Remove</button></div>
    <div class="sub">Kept on this PC only (tools\discord-leyline\token.txt, never on GitHub), never shown again, never in a log.</div>
    <div class="dos"><button class="primary sm" id="dprev" title="Shows what would change on your server. Changes nothing.">Preview</button>
      <button id="dcheck" title="The channel ids for the website bot and the game bridge. Changes nothing.">Bot settings</button></div>
    <div class="dopt"><label>Old channels (not in the layout): <select id="dold">
      <option value="archive">Move them to a hidden ARCHIVE</option>
      <option value="empty">Delete the empty ones, archive the rest</option>
      <option value="all">Delete them all, even with messages (backed up first)</option></select></label>
      <label class="chk"><input type="checkbox" id="dpins"> Post and pin the guide posts (only finished ones: none with
        TODO(owner))</label>
      <div class="sub">Set these, then press Preview: Do it does exactly what that preview lists.</div></div>
    <div class="dsum" id="dsum"></div>
    <div class="dgo" id="dgo" hidden><div id="dask"></div>
      <div class="launch"><input id="dyes" autocomplete="off" placeholder="Type YES"><button class="primary" id="dapply">Do it</button></div>
      <div class="sub" id="dgonote">It backs up the server to your Desktop first (Leyline-discord-backup-...).</div></div>
    <div class="sub" id="dwhy"></div><div class="dsum" id="dlast"></div>
    <div class="ann" id="ann"><div class="sec">Pending announcements</div>
      <div class="sub">Claude's drafts for #announcements (tools\discord-leyline\announce\pending). Nothing is posted
        until you type YES under one and press Post. Milestones, big community bug fixes, long-standing bugs and schedule
        updates; minor fixes don't get one. A draft with TODO(owner) in it can't be posted.</div>
      <div id="anlist"></div><div class="sub" id="anlast"></div></div></section>
  <section class="card" id="gmcard"><h2>Discord &amp; GM</h2>
    <div class="sub">Read-only: the server, the Discord bridge and bot, open bug reports and what waits for your review, from
      this PC's latest checks. Each button runs an existing step, only when you press it.</div>
    <div id="gmo"><div class="empty">Loading…</div></div>
    <div class="dos" id="gmacts"></div></section>
  <section class="card"><details class="fold"><summary>Claude's notes</summary>
    <h3>Working on</h3><div id="now"></div><div class="updated" id="cupd"></div>
    <h3>Finished</h3><div id="fin"></div></details></section>
  <section class="card hist"><details class="fold"><summary>History</summary><div id="hist"></div></details></section>
  <section class="card ks" id="killswitch"><details class="fold" id="ksfold"><summary>Kill switch</summary>
    <div class="sub">Takes Leyline down with your farewell (docs\KILL-SWITCH.md). <b>Pause</b> posts the message
      everywhere and stops or hides each part you tick; Undo brings it back. <b>Delete</b> comes only after a Pause and
      a day's wait, backs everything up to your Desktop first, and can't be undone. Each one needs its own preview and
      a typed phrase. Only this card runs it: never the queue or Autopilot.</div>
    <div class="launch"><input id="kstitle" maxlength="80" aria-label="Title" placeholder="Title"></div>
    <textarea id="ksmsg" maxlength="1800" aria-label="Farewell message" placeholder="Your farewell to the community"></textarea>
    <div class="parts" id="ksparts"></div>
    <label class="chk"><input type="checkbox" id="ksprivate"> Make the GitHub repos private on Pause (else public, read-only)</label>
    <label class="chk"><input type="checkbox" id="ksdry"> Dry run: check and log everything, change nothing</label>
    <div class="dos"><button id="kssave">Save</button>
      <button class="primary sm" id="ksprev-pause" title="Reads only. Lists exactly what Pause would do.">Preview Pause</button>
      <button id="ksprev-resume" title="Reads only.">Preview Undo</button>
      <button id="ksprev-delete" title="Reads only.">Preview Delete</button></div>
    <div class="sub" id="kspaused"></div>
    <div class="dsum" id="kssum"></div>
    <div class="dgo" id="ksgo" hidden><div id="ksask"></div>
      <div class="launch"><input id="ksphrase" autocomplete="off" spellcheck="false" placeholder="Type the phrase">
      <button class="primary" id="ksdo">Go</button></div></div>
    <div class="sub" id="kswhy"></div><div class="dsum" id="kslast"></div></details></section>
</div>
<div class="stack">
  <section class="card"><h2>Live output</h2><div class="status" id="run"></div><button id="stop" hidden title="Stops the running step and everything it started">Stop</button><div class="console" id="con"></div></section>
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
function youList(items,ans){const el=$("you");items=(items||[]).filter(i=>{const o=typeof i==="string"?{text:i}:i;return !(o.id&&ans[o.id]&&ans[o.id].done)});
if(!items.length){el.innerHTML='<div class="empty">Nothing. You\'re clear.</div>';return}
if(el.contains(document.activeElement)&&document.activeElement.tagName==="TEXTAREA")return;
el.innerHTML=items.map((i,n)=>{const o=typeof i==="string"?{text:i}:i,a=o.id?ans[o.id]||{}:{},id=esc(o.id||"");
const b=[];if(o.run)b.push(`<button class="primary sm" data-do="run" data-v="${esc(o.run)}">${esc(o.button||"Do it")}</button>`);
if(o.open)b.push(`<button data-do="open" data-v="${esc(o.open)}">${esc(o.open_label||"Open the page")}</button>`);
if(o.folder)b.push(`<button data-do="folder" data-v="${esc(o.folder)}">${esc(o.folder_label||"Open the folder")}</button>`);
if(o.drop)b.push(`<label class="drop" data-dir="${esc(o.drop)}" data-id="${id}">${esc(o.drop_label||"Add files (or drop them here)")}<input type="file" multiple hidden></label>`);
if(o.id&&o.ask)b.push(`<textarea data-id="${id}" rows="2" placeholder="${esc(o.ask)}">${esc(a.text||"")}</textarea><button data-do="send" data-id="${id}">Send</button>`);
if(o.id)b.push(`<button data-do="done" data-id="${id}">Done</button>`);
const got=(a.files||[]).length?`<div class="sub">Got: ${a.files.map(esc).join(", ")}</div>`:"";
return `<div class="item you"><span class="tag">${esc(o.tag||n+1)}</span><div><b>${esc(o.text)}</b>${o.detail?`<div class="sub">${esc(o.detail)}</div>`:""}${got}${b.length?`<div class="dos">${b.join("")}</div>`:""}</div></div>`}).join("")}
async function post(u,b){const r=await fetch(u,{method:"POST",headers:{...H,"Content-Type":"application/json"},body:JSON.stringify(b)});if(!r.ok)alert("Couldn't: "+(await r.text()));refresh()}
async function sendFiles(lab,files){for(const f of files){lab.firstChild.textContent=`Sending ${f.name}…`;
const r=await fetch(`/api/drop?id=${encodeURIComponent(lab.dataset.id||"files")}&dir=${encodeURIComponent(lab.dataset.dir||"")}&name=${encodeURIComponent(f.name)}`,{method:"POST",headers:H,body:f});
if(!r.ok)alert(`Couldn't send ${f.name}: `+(await r.text()))}refresh()}
$("you").addEventListener("click",e=>{const t=e.target.closest("button[data-do]");if(!t)return;const d=t.dataset.do;
if(d==="run"){$("con").innerHTML="";post("/api/do",{run:t.dataset.v})}else if(d==="open")post("/api/do",{open:t.dataset.v});
else if(d==="folder")post("/api/do",{folder:t.dataset.v});else if(d==="done")post("/api/answer",{id:t.dataset.id,done:true});
else if(d==="send"){const ta=$("you").querySelector(`textarea[data-id="${CSS.escape(t.dataset.id)}"]`);post("/api/answer",{id:t.dataset.id,text:ta?ta.value:""})}});
$("you").addEventListener("change",e=>{const i=e.target;if(i.type==="file"&&i.files.length)sendFiles(i.closest("label"),[...i.files])});
$("you").addEventListener("dragover",e=>{if(e.target.closest(".drop")){e.preventDefault();e.target.closest(".drop").classList.add("over")}});
$("you").addEventListener("dragleave",e=>{const l=e.target.closest(".drop");if(l)l.classList.remove("over")});
$("you").addEventListener("drop",e=>{const l=e.target.closest(".drop");if(!l)return;e.preventDefault();l.classList.remove("over");sendFiles(l,[...e.dataTransfer.files])});
async function refresh(){const r=await fetch("/api/state",{headers:H});if(!r.ok)return;const s=await r.json();last=s;
const h=$("health");h.innerHTML=Object.entries(s.health||{}).map(([k,v])=>`<span class="pill"><span class="dot ${v.ok?"ok":"bad"}"></span>${esc(k)}${v.ms!=null?` · ${v.ms} ms`:` · ${esc(v.code)}`}</span>`).join("")||'<span class="pill">checking…</span>';
const c=s.claude||{};youList(c.waiting_on_you,s.answers||{});
list($("now"),c.now,"now",c.note||"Idle.");list($("fin"),c.done,"fin","Nothing yet.");
$("cupd").textContent=c.updated?`Updated ${c.updated}`:"";
list($("pending"),(s.pending||[]).map(p=>({tag:p.step,text:p.title,detail:p.note})),"","Nothing to launch. You're up to date.");
const a=$("auto");a.textContent=s.autopilot?"Autopilot: ON":"Autopilot: OFF";a.className=s.autopilot?"primary":"";a.dataset.on=s.autopilot?"1":"";
const q=$("ask");q.textContent=s.confirm?"Ask before deploys: ON":"Ask before deploys: OFF";q.dataset.on=s.confirm?"1":"";
$("go").disabled=!!s.running||!(s.pending||[]).length;$("stop").hidden=!s.running||s.running.step==="restarting";$("audit").disabled=!!s.running;
const ar=$("areport");if(s.audit){ar.hidden=false;ar.href=`/audit?t=${T}`;ar.textContent=`Last audit ${s.audit.at}: ${s.audit.green?"all green":s.audit.result}`;ar.style.color=s.audit.green?"var(--ok)":"var(--bad)"}else ar.hidden=true;
$("run").innerHTML=s.running?`<span class="spinner"></span>${esc(s.running.title||"Running")}…`:'<span class="sub">Idle</span>';
$("hist").innerHTML=(s.history||[]).map(x=>`<div class="item fin"><span class="tag">${esc(x.step)}</span><div>${esc(x.note||x.id)} ${x.log?`<a href="/api/log?t=${T}&name=${encodeURIComponent(x.log)}" target="_blank">log</a>`:""}</div></div>`).join("")||'<div class="empty">No runs yet.</div>';
discordPanel(s);annPanel(s);ksPanel(s);gmPanel(gmBusy&&!s.running);gmBusy=!!s.running;
document.querySelectorAll("#gmacts [data-gmrun]").forEach(b=>{b.disabled=!!s.running});
if(!$("con").childElementCount&&s.lines)s.lines.forEach(line)}
let gmAt=0,gmBusy=false;
async function gmPanel(force){if(!force&&Date.now()-gmAt<20000)return;gmAt=Date.now();
const r=await fetch("/api/gm",{headers:H});if(!r.ok)return;const g=await r.json();
const hs=Object.entries(g.server.health||{}),up=hs.filter(([,v])=>v.ok).length,lc=g.server.last_check;
let h=`<div class="item"><span class="tag">server</span><div>${hs.length?`${up} of ${hs.length} answering`:"checking…"}${lc?` · last check ${esc(lc.at)}: ${lc.ok?"ok":"failed"}`:""}</div></div>`;
const b=g.bridge;h+=`<div class="item"><span class="tag">bridge</span><div>${b.seen?`wu-discord: ${esc(b.state||"?")}, ${esc(b.enabled||"?")} · seen ${esc(b.at)}`:"Not in the last server check yet: press Check server."}${b.lines.length?`<div class="sub">${b.lines.map(esc).join("<br>")}</div>`:""}</div></div>`;
const d=g.discord;h+=`<div class="item"><span class="tag">bot</span><div>Token ${d.token?"saved":"not saved"}${d.check?` · bot settings ${esc(d.check)}${d.check_ok===false?" (stopped)":""}`:""}${d.preview?` · last preview ${esc(d.preview)}`:""}</div></div>`;
const u=g.bugs;h+=`<div class="item"><span class="tag">bugs</span><div>${u.have?`${u.open} open · pulled ${esc(u.at)}`:"No bug list on this PC yet: press Pull bug reports."}${u.items.length?`<ul class="gml">${u.items.map(i=>`<li><b>${esc(i.status)}</b> ${esc(i.title)} <span class="sub">x${i.count} · ${esc(i.sources.join(", "))} · ${esc(i.last)}</span></li>`).join("")}</ul>`:""}</div></div>`;
const m=g.mods,names=m.addon_names.concat(m.look_names);h+=`<div class="item"><span class="tag">mods</span><div>${m.addons} addon(s) and ${m.looks} look(s) wait for your review (HQ, Mods)${names.length?`<div class="sub">${esc(names.join(", "))}</div>`:""}</div></div>`;
if(g.quick.length)h+=`<div class="item"><span class="tag">quick</span><div>${g.quick.map(q=>`${esc(q.at)} ${esc(q.id)}, ${esc(q.deploy)}: ${q.ok?"done":"not done"}${q.why?` (${esc(q.why)})`:""}`).join("<br>")}</div></div>`;
$("gmo").innerHTML=h;
$("gmacts").innerHTML=g.actions.map(a=>`<button data-gmrun="${esc(a.step)}"${last&&last.running?" disabled":""}>${esc(a.label)}</button>`).join("")
+'<button data-gm="console" title="The server GM console (bug reports, players, logs)">GM console</button><button data-gm="hq">Open HQ</button>'}
$("gmacts").addEventListener("click",e=>{const t=e.target.closest("button");if(!t||t.disabled)return;
if(t.dataset.gmrun){$("con").innerHTML="";post("/api/do",{run:t.dataset.gmrun})}
else if(t.dataset.gm==="console")$("gmc").click();else if(t.dataset.gm==="hq")$("hq").click()});
const logLink=n=>n?` <a href="/api/log?t=${T}&name=${encodeURIComponent(n)}" target="_blank">log</a>`:"";
let dprevKey="";
function discordPanel(s){const d=s.discord||{},t=d.token||{},busy=!!s.running,el=$("dtok");
el.textContent=t.saved?`saved${t.last4?` (ends ${t.last4})`:""}`:"not saved";el.className=t.saved?"ok":"no";
$("drm").disabled=!t.saved;$("dprev").disabled=busy||!t.saved;$("dcheck").disabled=busy||!t.saved;
const p=d.preview,key=p?p.id+(p.used?"u":""):"";
if(key!==dprevKey){dprevKey=key;let h="";
if(p){const head=p.ok?`Last preview ${esc(p.when)}: <b>${p.changes} change(s)</b> to ${esc(p.server)}${p.used?" (done since)":""}`:`Last preview ${esc(p.when)} didn't finish${p.error?`: ${esc(p.error)}`:""}`;
h=`<div>${head}${logLink(p.log)}</div>`;const its=p.items||[];
if(its.length){let sec=null,body="";for(const i of its){if(i.section!==sec){if(sec!==null)body+="</ul>";sec=i.section;body+=`<div class="sec">${esc(sec)}</div><ul>`}
body+=`<li${i.change?"":' class="n"'}>${esc(i.text)}</li>`}body+="</ul>";
h+=`<details class="fold"${p.ok&&!p.used?" open":""}><summary>What would change</summary>${body}</details>`}}
$("dsum").innerHTML=h||'<div class="empty">No preview yet. Save the token, then press Preview.</div>'}
const o=d.options||{old:"archive",pins:false};
if(document.activeElement!==$("dold"))$("dold").value=o.old;$("dpins").checked=!!o.pins;$("dold").disabled=$("dpins").disabled=busy;
const po=(p&&p.options)||o;$("dgonote").textContent="It backs up the server to your Desktop first (Leyline-discord-backup-...). "+
(po.pins?"It posts and pins only the posts the preview lists. ":"It posts nothing. ")+
(po.old==="all"?"It DELETES every old channel the preview lists, even ones with messages.":po.old==="empty"?"It deletes the old channels with no messages.":"It deletes nothing.");
$("dgo").hidden=!d.ready;$("dapply").disabled=busy;
if(d.ready)$("dask").innerHTML=`Read the preview above. To make these <b>${p.changes} change(s)</b> to ${esc(p.server)}, type YES and press Do it.`;
$("dwhy").textContent=d.ready||!t.saved&&!p?"":d.why||"";
const a=d.apply,c=d.check;let l="";
if(a)l+=`<div>Last do it ${esc(a.when)}: ${a.ok?'<span style="color:var(--ok)">done</span>':'<span style="color:var(--bad)">stopped</span>'}${logLink(a.log)}${a.ok?" Then press Bot settings, and kick the bot when you're finished.":""}</div>`;
if(c)l+=`<div>Bot settings ${esc(c.when)}:${logLink(c.log)}${(c.lines||[]).length?`<pre class="console" style="height:auto;max-height:220px">${esc(c.lines.join("\n"))}</pre>`:""}</div>`;
$("dlast").innerHTML=l}
async function dpost(u,b){const r=await fetch(u,{method:"POST",headers:{...H,"Content-Type":"application/json"},body:JSON.stringify(b||{})});
if(!r.ok){let m=await r.text();try{m=JSON.parse(m).error||m}catch(e){}alert(m)}refresh();return r.ok}
$("dsave").onclick=async()=>{const i=$("dtokin"),v=i.value;if(!v.trim()){alert("Paste the bot token first.");return}i.value="";await dpost("/api/discord/token",{token:v})};
$("dtokin").addEventListener("keydown",e=>{if(e.key==="Enter")$("dsave").click()});
$("drm").onclick=()=>{if(confirm("Remove the saved Discord bot token from this PC?"))dpost("/api/discord/token-remove")};
$("dprev").onclick=()=>{$("con").innerHTML="";post("/api/do",{run:"discord-preview"})};
$("dcheck").onclick=()=>{$("con").innerHTML="";post("/api/do",{run:"discord-check"})};
const dopt=()=>dpost("/api/discord/options",{old:$("dold").value,pins:$("dpins").checked});
$("dold").onchange=dopt;$("dpins").onchange=dopt;
$("dapply").onclick=async()=>{const p=(last&&last.discord||{}).preview;if(!p)return;$("con").innerHTML="";
if(await dpost("/api/discord/apply",{preview:p.id,confirm:$("dyes").value.trim()}))$("dyes").value=""};
let annKey="";
function annPanel(s,force){const d=s.discord||{},a=d.announce||{},busy=!!s.running,tok=!!(d.token||{}).saved,ds=a.drafts||[];
const key=JSON.stringify([ds.map(x=>[x.id,x.sha,x.problems.length]),busy,tok,a.error||""]);
if(key!==annKey||force){annKey=key;
$("anlist").innerHTML=a.error?`<div class="empty">The drafts didn't load: ${esc(a.error)}</div>`:ds.length?ds.map(x=>{const ok=!x.problems.length;
return `<div class="anitem" data-id="${esc(x.id)}" data-sha="${esc(x.sha)}"><div><b>${esc(x.title||x.id)}</b> <span class="tag">${esc(x.kind||"?")}</span>
 ${x.ping==="everyone"?"pings @everyone":"no ping"} · ${x.length}/${x.limit} characters${x.created?` · ${esc(x.created)}`:""} <span class="sub">${esc(x.id)}</span></div>
<pre class="console anmsg">${esc(x.message)}</pre>
${ok?'<div style="color:var(--ok)">Passes the checks.</div>':`<ul class="anbad">${x.problems.map(p=>`<li>${esc(p)}</li>`).join("")}</ul>`}
<div class="launch"><button data-an="check">Check</button><input data-an="yes" autocomplete="off" placeholder="Type YES"${ok&&tok?"":" disabled"}>
<button class="primary" data-an="post"${ok&&tok&&!busy?"":" disabled"} title="${ok?(tok?"Posts exactly this to #announcements":"Save the bot token first"):"Fix the problems first"}">Post to #announcements</button>
<button data-an="reject"${busy?" disabled":""}>Reject</button></div></div>`}).join(""):'<div class="empty">No drafts waiting.</div>'}
const l=a.last;$("anlast").innerHTML=l?`Last announcement ${esc(l.when)} (${esc(l.draft||"")}): ${l.ok?'<span style="color:var(--ok)">posted</span>':'<span style="color:var(--bad)">not posted</span>'}${logLink(l.log)}`:""}
async function anpost(u,b){const r=await fetch(u,{method:"POST",headers:{...H,"Content-Type":"application/json"},body:JSON.stringify(b||{})});
let j={};try{j=await r.json()}catch(e){}if(!r.ok)alert(j.error||"That didn't work.");return r.ok?j:null}
$("anlist").addEventListener("click",async e=>{const t=e.target.closest("button[data-an]");if(!t||t.disabled)return;
const it=t.closest(".anitem"),id=it.dataset.id,sha=it.dataset.sha,act=t.dataset.an;
if(act==="check"){const j=await anpost("/api/discord/announce-check",{id});if(j)alert(j.ok?"Passes every check. Nothing was posted.":"Not ready:\n- "+j.draft.problems.join("\n- "));annKey="";refresh()}
else if(act==="reject"){if(!confirm("Reject this draft? It moves to announce\\rejected. Nothing is posted."))return;await anpost("/api/discord/announce-reject",{id});annKey="";refresh()}
else if(act==="post"){const y=it.querySelector('input[data-an="yes"]').value.trim();if(y!=="YES"){alert("Type YES under the draft to post it.");return}
$("con").innerHTML="";if(await anpost("/api/discord/announce-post",{id,sha,confirm:y}))annKey="";refresh()}});
const KS_WORD={pause:"Pause",resume:"Undo pause",delete:"Delete"};let ksKey="",ksCfg="";
function ksPanel(s){const k=s.killswitch||{};if(k.error){$("kswhy").textContent="The kill switch didn't load: "+k.error;return}
const c=k.config||{},busy=!!s.running,ck=JSON.stringify(c);
if(ck!==ksCfg&&![$("kstitle"),$("ksmsg")].includes(document.activeElement)){ksCfg=ck;$("kstitle").value=c.title||"";$("ksmsg").value=c.message||"";
$("ksparts").innerHTML=(k.parts||[]).map(p=>`<label class="chk"><input type="checkbox" data-part="${esc(p.key)}"${(c.parts||[]).includes(p.key)?" checked":""}> ${esc(p.title)}${p.deletable?"":' <span class="sub">(Pause only)</span>'}</label>`).join("");
$("ksprivate").checked=!!(c.options||{}).private;$("ksdry").checked=!!(c.options||{}).dry}
["kssave","ksprev-pause","ksprev-resume","ksprev-delete"].forEach(id=>$(id).disabled=busy);
$("ksprev-resume").hidden=!k.paused;$("ksprev-delete").hidden=!k.paused;
const pa=k.paused;$("kspaused").innerHTML=pa?`<b style="color:var(--bad)">Paused</b> since ${esc(pa.when)} (${esc((pa.parts||[]).join(", "))})${pa.ok?"":" with problems"}. A real Delete opens ${esc(pa.delete_opens)}.`:"";
const p=k.preview,key=p?p.id+(p.used?"u":""):"";
if(key!==ksKey){ksKey=key;let h="";
if(p){h=`<div>Preview of <b>${esc(KS_WORD[p.stage])}</b>, ${esc(p.when)}${p.used?" (used)":""}${logLink(p.log)}</div>`;
const PT=Object.fromEntries((k.parts||[]).map(x=>[x.key,x.title]));PT.backup="Backup";PT.all="All";
let sec=null,body="";for(const i of p.items||[]){if(i.part!==sec){if(sec!==null)body+="</ul>";sec=i.part;body+=`<div class="sec">${esc(PT[sec]||sec)}</div><ul>`}
body+=`<li class="${esc(i.kind)}">${i.kind==="manual"?"You: ":i.kind==="problem"?"Problem: ":""}${esc(i.text)}</li>`}if(sec!==null)body+="</ul>";
if((p.problems||[]).length)body+=`<div class="sec" style="color:var(--bad)">Fix these first</div><ul>${p.problems.map(x=>`<li class="problem">${esc(x)}</li>`).join("")}</ul>`;
h+=body}$("kssum").innerHTML=h}
const st=p&&k.ready[p.stage]?p.stage:"";$("ksgo").hidden=!st;$("ksdo").disabled=busy;
if(st){$("ksask").innerHTML=`Read the preview above. To ${esc(KS_WORD[st].toLowerCase())}${(c.options||{}).dry?" (dry run: nothing changes)":""}, type <b>${esc(k.phrases[st])}</b> and press ${esc(KS_WORD[st])}.`;$("ksdo").textContent=KS_WORD[st]}
$("kswhy").textContent=p&&!st?(k.why[p.stage]||""):"";
const l=k.last;$("kslast").innerHTML=l?`<div>Last ${esc(KS_WORD[l.stage])} ${esc(l.when)}: ${l.dry?"dry run, checked":l.ok?'<span style="color:var(--ok)">done</span>':'<span style="color:var(--bad)">stopped or with problems</span>'}${logLink(l.log)}</div>`
+((l.problems||[]).length?`<ul>${l.problems.map(x=>`<li class="problem">${esc(x)}</li>`).join("")}</ul>`:"")
+((l.manual||[]).length?`<div class="sec">Your turn</div><ul>${l.manual.map(x=>`<li class="manual">${esc(x)}</li>`).join("")}</ul>`:"")
+((l.turn_back_on||[]).length?`<div>Turn back on what you want: ${esc(l.turn_back_on.join(", "))}</div>`:""):""}
function ksConfig(){return{title:$("kstitle").value,message:$("ksmsg").value,parts:[...$("ksparts").querySelectorAll("input[data-part]:checked")].map(i=>i.dataset.part),options:{private:$("ksprivate").checked,dry:$("ksdry").checked}}}
async function ksSave(){ksCfg="";return dpost("/api/killswitch/save",ksConfig())}
$("kssave").onclick=ksSave;
["pause","resume","delete"].forEach(st=>{$("ksprev-"+st).onclick=async()=>{if(!await ksSave())return;$("con").innerHTML="";dpost("/api/killswitch/preview",{stage:st})}});
$("ksdo").onclick=async()=>{const k=(last&&last.killswitch)||{},p=k.preview;if(!p)return;$("con").innerHTML="";
if(await dpost("/api/killswitch/go",{stage:p.stage,preview:p.id,phrase:$("ksphrase").value.trim()}))$("ksphrase").value=""};
$("go").onclick=async()=>{$("con").innerHTML="";const r=await fetch("/api/launch",{method:"POST",headers:{...H,"Content-Type":"application/json"},body:JSON.stringify({message:$("msg").value})});if(!r.ok)alert("Couldn't start: "+(await r.text()));refresh()};
$("audit").onclick=async()=>{$("con").innerHTML="";const r=await fetch("/api/audit",{method:"POST",headers:H});if(!r.ok)alert("Busy: "+(await r.text()));refresh()};
$("stop").onclick=async()=>{if(!confirm("Stop the running step? It is marked failed and Autopilot pauses."))return;const r=await fetch("/api/stop",{method:"POST",headers:H});if(!r.ok)alert("Couldn't: "+(await r.text()));refresh()};
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
