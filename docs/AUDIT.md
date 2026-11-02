# The release audit

Owner, 1 Nov 16:28: "create an automated test list for me to audit everything as a new user and make sure all our
features work like Den, hosting, Azeroth Survivors, Wrath Unleashed, forum etc, everything, with as little as I need
to do as possible."

## What you do

1. Open **Leyline Control** and press **Release audit** (top right). It runs about 10 minutes, read only, and opens
   the report when it is done (`logs\audit-latest.html`; the same in `logs\audit-latest.txt`). The link
   "Last audit ..." beside the button opens the last one again.
2. If the report is red: do the one **Next** line at its top (or send Claude `logs\audit-latest.txt`).
3. When it is green: do the manual list below (it is also at the top of the report).

The same run is the Control step `audit` ("Release audit (read-only, ~10 min)"), so Claude can queue it in
UPDATES-PENDING.txt and Autopilot runs it by itself (then the report doesn't pop up; the link shows it).

## Only you can check these

What the automated run can't judge, or what needs a second PC. About 30 minutes; 4 and 7 need a friend.

1. **Play solo with your real game**  
   Where: Leyline > Home > Your Den > Play. At the login screen: account DEN, password den, Login.  
   Good: No firewall or Defender prompt. Character select shows the Forever UI look; Enter puts you in the white room, dressed, with the chair the quick start talks about. Close Leyline, open it again, Play: straight in again.

2. **Jaina looks and walks right**  
   Where: In Your Den, run a few steps, then type: Jaina, come  
   Good: Jaina is her own model (not a cube or a missing model), walks and runs after you with her feet moving, and her lines show as "Jaina says:" in chat and in a bubble over her head.

3. **Forge opens on the Den tile**  
   Where: Leyline > Your Den > Shape the ground > Open in Forge. Raise a hill, Ctrl+S, close Forge.  
   Good: Forge opens on the Den's own tile (not an empty map). After closing: "Terrain updated". The next Play shows the hill.

4. **A friend on another network joins your Den**  
   Where: Leyline > Your Den > Invite friends > copy the link. Your friend (another home, not your Wi-Fi): Leyline > Realms > Got an invite link? > paste > Play.  
   Good: Your friend lands at character select on their own character and comes into your Den; you see each other move and talk. Stop sharing: their next login is refused.

5. **Wrath Unleashed on your desktop game**  
   Where: Leyline > Play (Wrath Unleashed chosen) > log in > make or pick a character > Enter world. Then Esc > Report a bug.  
   Good: Nameplates are easy to read and sit right over heads; the bars and frames look finished. Report a bug opens a box (press Cancel, so nothing is filed).

6. **The browser game feels right**  
   Where: On the website: Play in the browser > sign in > Play.  
   Good: Character select within about 2 minutes on a first visit, much faster on the second. In the world: smooth turning, about 30 frames a second or better, no long freezes when you run into a new area.

7. **Host a server, start to finish**  
   Where: Leyline > Host > Host a mode > Classic levelling > Host. Wait for "You're live".  
   Good: The first start finishes (5 to 20 minutes). "You're live" shows a link; the realm is on Realms (public) and a friend outside your network can join it with the link.

8. **The website when you are signed in**  
   Where: On the website: Sign in (Hero Key or Google) > Forum > New thread (don't post) > Report a bug (don't send) > Host > Start server.  
   Good: Sign-in brings you back where you were. The forum and bug forms show (not "sign in"). Start server gives a file that runs on your PC and the page turns live.

9. **Looks: one click changes the game**  
   Where: Leyline > Looks > Classic > Apply, then Play. Afterwards Looks > Balanced > Apply.  
   Good: The game opens looking different (shorter view, plainer shadows); Balanced puts it back.

10. **Azeroth Survivors (only when its page shows Play)**  
   Where: Website > Azeroth Survivors > Play.  
   Good: The game opens and a run starts. The pick screen and the results screen show no franchise name or logo.
## What the automated run checks

One PASS / FAIL / SKIP line per check, grouped by feature. A check this PC can't run is a SKIP with the reason,
never a silent FAIL.

| Feature | Checked |
|---|---|
| Site | The main pages (Start here, Leyline home, Looks, Host, Realms, Modes, Mods, Forum, Report a bug, Survivors, Hero Key, the hosting guide, the play page) answer, then every public page linked from them (two hops, up to 220 addresses): 200 or a sign-in, no server error text, no banned words, no client version in a sentence (main pages and the guides on separate lines), every picture loads, every release download button leads to a real file. The new address (leylinelauncher.com) answering, or SKIP while the domain move isn't on |
| Downloads | The launcher (Leyline.exe on launcher-latest, sha256 against GitHub's), the Host pack (the site's hostpack.json: the file, its sha256, its .sha256), Den Lite (both release files against their .sha256) |
| Catalog, Looks | /wrath/mods/all.json loads with the Host pack in it; the Looks entries and their graphics presets, each one a preset the launcher has (launcher/src-tauri/src/gfx.rs) |
| Forum | Categories, the latest threads, one thread opens (read only) |
| Azeroth Survivors | The page's state (coming, resting, live); when live: its own address asks for sign-in, /licenses/ is public, /data needs sign-in |
| Server | Over the PC's SSH key, read only (tools/audit/server/audit-server.sh): the world server up, no restart in the last hour, no out-of-memory kills, SOAP answers, the Lua scripts and no Lua errors, the slice health (stream/server/slice-health.sh), the relay, the GM console, the bug manifest, memory and disk |
| Hosting | The network list (the relay's listing) with Wrath Unleashed on it, the relay's join page for it, the Host page, the Host pack opened and its self-check there and compiling (a real self-check needs a running realm: manual item 7) |
| Wrath Unleashed | The headless 3.3.5a client (hosting/kit/tools/wunet/wire335.py plus tools/audit/play335.py) on the live realm with the audit's own account: log in, list characters, make a level-1 night elf hunter, enter the world, the first-login choice, spawn, the starting weapon and bow, the realm's level-1 mount, a basic spell, running a few yards, targeting a creature and starting melee (SMSG_ATTACKSTART), `.bug` answering (empty, so nothing is filed), log out; a second character takes the boost to 80; both are deleted again |
| Your Den | Den Lite on this PC (the launcher's copy, or den-lite-latest checked against its .sha256) on loopback, its own free ports and a temporary Dens folder: DEN / den, the quick start, the character and the room, `.den place chair`, `.den save`, Jaina there and answering "Jaina, tips", the stop file, the Den back after a restart (autosave), a guest login (Invite friends), the friend's client in through the friends' listener, both see each other, the login taken back. Through the relay: SKIP (the launcher's tunnel: manual item 4) |
| Launcher | The Leyline.exe on this PC is the latest release; %APPDATA%\Leyline\settings.json reads and its game folders exist |

### The audit account

The realm makes its own test account for each run: `audit-server.sh account` runs `account create AUDIT <pw>` (or
`account set password`) over SOAP with a fresh random 16-character password, sets its GM level to 0, keeps the
password only in `/root/.leyline-audit/password` (chmod 600) and prints it once to the SSH session. The PC side
keeps it in memory, passes it to the test client and never prints or logs it (the recorder hides it from every
line). Never the owner's account, never another player's. The only things changed on the realm: that account,
its test characters (made and deleted in the run; leftovers from a broken run are deleted at the start of the
next) and what the boost writes for its own character (`ov_boosts`, `ov_main_choice`: a boosted test
character can't be boosted again, and after 10 boosts the account's kits run out: the boost still levels, which
is what is checked). `--no-boost` leaves the boost out.

## Running it elsewhere

```
py -3 tools\audit\audit.py                         the live run (what the button does)
python3 tools/audit/audit.py --fake                  anywhere: the site's own code and fake servers on 127.0.0.1
python3 tools/audit/audit.py --only site,downloads   some sections (site, downloads, server, hosting, wrath, den, launcher)
DEN_LITE_BIN=<den-lite> python3 tools/audit/audit.py --fake --only den    the Den section against a built Den Lite
```

`--fake` (tools/audit/fakes.py): the website is stream/wrath_portal.py run on 127.0.0.1 with fake data (signed out),
the relay a listing with Wrath Unleashed, GitHub a file server whose files match their checksums, the realm the Host
kit's fake realm (hosting/tests/fake_realm.py) with a world that answers the audit's player, and SSH the server
script's all-green output. Exit code 0 when nothing failed, 1 when something did, 2 when the audit itself broke.

Tests: `python3 -I tools/audit/tests/test_audit.py` (the parsers, the report, the manual list here against the
report's, secrets, and the whole `--fake` run; the Den section too when `DEN_LITE_BIN` is set),
`python3 -I tools/control/test_control_audit.py` (the step and the button).

## Addresses

Every address comes from `tools/audit/audit.json` (`addresses`: site, game, relay, realmlist; new and old), or
from `tools/domain/addresses.json` when that file is in the workspace (the domain move's one list wins). Each is
tried by its new name first (leylinelauncher.com, play.leylinelauncher.com, relay.leylinelauncher.com) and the
old one (play.openvanilla.app, wrath.openvanilla.app, relay.openvanilla.app) when the new one doesn't answer; the
report's header says which answered. SSH uses the realmlist name the same way.

## Files

| Path | What |
|---|---|
| `tools/audit/Release audit.bat` | the Control step `audit` |
| `tools/audit/audit.py` | the runner, `--fake`, `--only` |
| `tools/audit/common.py`, `audit.json` | results, settings, addresses, HTTP |
| `tools/audit/web.py` | site, downloads, catalog, Looks, forum, Survivors, hosting pages |
| `tools/audit/server.py`, `server/audit-server.sh` | the server over SSH, the audit account, the network list |
| `tools/audit/play335.py`, `wrath.py` | the headless player and the Wrath Unleashed run |
| `tools/audit/den.py` | Your Den with Den Lite |
| `tools/audit/pc.py` | the launcher on this PC |
| `tools/audit/report.py` | the report and the manual list (MANUAL: the same as above) |
| `tools/audit/fakes.py`, `tests/test_audit.py` | the fake world and the tests |
