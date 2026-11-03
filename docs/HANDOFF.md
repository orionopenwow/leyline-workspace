# Leyline: handoff for a fresh Claude session

Written 2 Nov 2026, 21:50 (Phoenix time), updated 22:20. Read this first, then the newest `claude/morning-report-*`,
`claude/release-goals-*` and `claude/community-digest/*` docs in the Leyline Launcher project. The same text is in
the project as `claude/HANDOFF.md`.

It covers:
- who the owner is and the rules;
- where the code lives;
- how changes reach the PC and the server (Control, autopilot, ship);
- what is live;
- what is open.

---

## 1. Who and what

- **Owner:** campwilkins, who goes by **Orion**. Leyline, the Leyline Launcher, Wrath Unleashed, Your Den / Den Lite, Forge and Azeroth Survivors are his. He works in short bursts, often with screenshots, and expects you to act: build, test, deploy, then report in a few lines.
- **What it is:**
  - **Leyline Launcher:** a Tauri desktop app (Rust core in `launcher/src-tauri`, HTML/JS UI in `launcher/ui`). Players log in with a Hero Key (the sign-in screen comes first) and add their own game folders ("Wrath 3.3.5a", "Vanilla 1.12.1"…). Then they play:
    - the Official realms (Wrath Unleashed, AzerothCore, 24/7);
    - Community realms, run by players over the relay;
    - Your Den, a private building room on Den Lite (a local Rust server, `labs/den-lite`);
    - modes such as Azeroth Survivors (in the browser).
  - **Website:** leylinelauncher.com (old name play.openvanilla.app, both answer). The Python portal is `stream/wrath_portal.py` plus modules in `stream/`: herokeys, profiles, social, network, forum, hub, discord_growth, survivors and others.
  - **Relay / mesh:** `hosting/relay`. RELAY_MESH step 1 is on.
  - **Leyline Control:** `tools/control/control.py`, the owner's local control panel on his Windows PC. It runs "steps" (.bat files) from a queue.

## 2. Standing rules (never break these)

- **Brand words:**
  - Never show "World of Warcraft", "WoW", "Warcraft", "Lich King" or "Blizzard" to players.
  - The only exception is the legal line "not affiliated with or endorsed by Blizzard Entertainment".
  - No game client version numbers in prose. Data fields and build badges are fine.
- **Accounts and git:**
  - Never use the **Chumpton** GitHub account. Commits are made as **Orion** (`user.name "Orion"`, `orionopenwow@users.noreply.github.com`).
  - Our own commits from the cloud end with the Co-Authored-By / Claude-Session trailers the session gives you.
- **Posting:** Discord and Reddit links only. Never post anywhere for the owner without his explicit OK.
- **Money:** no money, tip, support or donation links. Ask before spending money.
- **Secrets:** never print secrets: tokens, keys, passwords, the Discord bot token, env files.
- **Game data:**
  - Never host or ship game data, except the approved browser slices.
  - P2P never carries game files.
  - No decompiled clients, and nothing from wowdecomp or Waystone.
- **Things you don't do yourself:**
  - Never create accounts or type passwords. The audit's own test-account API is the allowed path.
  - DNS changes are the owner's. Ask before deleting anything not approved.
  - Never run `rollback.sh` or `docker compose up` in `/opt/wenilla-realm`.
  - Don't run `ghclean` on the 1.12 images.
  - No desktop computer use. The built-in browser pane is OK; it can reach the PC's 127.0.0.1 ports.
- **Sign-in:** Discord sign-in is **not** used by us (removed 2 Nov). Sign-in is the Hero Key, with Google as an option. The wording everywhere is "Log in".
- **Copy:**
  - Copy in the owner's own voice is left as `TODO(owner)`.
  - Mods, addons, looks and HD packs the tools find stay **pending** until the owner approves them. Never approve them yourself.
- **Deploys:**
  - Every deploy needs a backup, a self-check and a rollback.
  - The auto-mode classifier sometimes blocks deploy or history-rewrite writes. **Don't work around it**: tell the owner which Control step to press.

## 3. Where the code is

| What | Where |
|---|---|
| Main working repo (private) | GitHub `orionopenwow/wrathunleashed` (redirects to `orionopenwow/leyline-workspace`) |
| Cloud working copy | `/home/claude/orionopenwow/wrathunleashed`. The day's work is in the git worktree **`/home/claude/wt-theme`**, branch **`feat-dark-light`** |
| Push from the cloud | `git push origin HEAD:feat-dark-light`, then `git update-ref refs/remotes/origin/feat-dark-light HEAD` |
| Owner's PC copy (the one that ships) | `C:\Users\campw\Desktop\Leyline`, through the device tools; there is **no device_bash**, only the stage, commit and list tools. Its git ships to `leyline-workspace` main |
| Public repos (exported by Control) | `orionopenwow/Leyline` (the launcher and the network) and `orionopenwow/Leyline-Launcher` (the old Open Vanilla repo; its `www/` is the openvanilla.app Pages site and is kept) |
| Server | One host behind Caddy. Its portal is `/srv/wrath`, `/opt/wu-discord` holds the Discord bridge, and the realm site is the Rust service `ovlive-realm:8090` |

**Disk in the cloud container is tight** (about 600 MB free; `launcher/src-tauri/target` alone is 5.9 GB):
- Do not start fresh cargo builds in new worktrees.
- Run `cargo test --lib -q` only in `wt-theme/launcher/src-tauri`. It takes about 10 minutes and last showed 387 passed.
- Remove finished worktrees with `git worktree remove --force`, but only once their work is committed and merged. **Never remove one an agent is still using** (it happened once).

## 4. Tests (run before every deploy)

- **Website and portal:**
  - Run `python3 stream/tests/test_<name>.py stream/wrath_portal.py` from the repo root. Most need that argument.
  - Key suites: site (531), hub (551), profiles (120), herokeys (122), admin (275), forum (154), network (88), social, discord_growth (141), looks, brand, realms.
  - `test_listings.py` and `test_site.py` had known unrelated failures on some commits; check against the base before blaming yourself.
- **Launcher UI:**
  - `cd launcher && node --test tests/<file>.browser.test.cjs`, using Playwright with a fake core. Chromium is preinstalled. There are 36 files and 282 tests.
  - Long runs: put them in the background.
  - Key files: gate (the sign-in screen), firstrun (a whole first-time walkthrough; screenshots with `FIRSTRUN_SHOTS=<dir>`), realms, playbar, den, home, nav, account, social, guide.
  - Fakes for pages not about sign-in keep a sign-in (`social_status` true). Sign-in tests call `LeylineGate.skip()`.
- **Tools:**
  - `python3 tools/discord-leyline/test_discord_leyline.py`
  - `tools/control/test_*.py`
  - `tools/hub-index/test_*.py`
  - `tools/addon-curate/test_curate.py`
  - `tools/fresh-history/test_*.py`
  - `hosting/tests/*.py`

## 5. Getting a change onto the PC (the deploy pattern)

The PC copy must match git before you overwrite it. Every time:

1. **Commit** in `wt-theme` and push to `feat-dark-light`.
2. **List the changed files** since the last commit that is already on the PC (`git diff --name-status <base> HEAD`). The PC is at **the commit after cf92c12** (export.py comment fix) as of 22:45 on 2 Nov.
3. **Stage the PC copies** with `device_stage_files` (paths like `C:\Users\campw\Desktop\Leyline\...`). They land in `/mnt/user-data/uploads/Leyline/...`.
4. **Check the PC copy equals `git show <base>:<file>`**, with CR characters stripped. If it differs, the owner changed it: stop and look.
5. **Export the new versions** with `git show HEAD:<file>` into `/mnt/user-data/outputs/rNN/...`. Keep CRLF if the PC file had it; new `.bat` files always get CRLF. The last folder used was `r65`.
6. **Write the files** with `device_commit_files`, with `expectedMtimeMs` taken from the stage. New files need no mtime.
7. **Queue the Control steps last.** Stage `UPDATES-PENDING.txt`, append lines `"<id> <step>  <what it brings>"` (CRLF), write it back with its mtime guard. Ids are date-ish and increasing; the last used was **1310c**. Use **1311a** next.

## 6. Leyline Control: steps, autopilot, logs

- Opened by `Leyline Control.bat` on the PC. It has a web UI on localhost.
- **Queue:** `UPDATES-PENDING.txt`. Finished ids go to `UPDATES-DONE.txt`, and logs to `Desktop\Leyline\logs\<id>-<step>.log`.
- **Autopilot** (header switch, `control/autopilot.json`) runs new pending steps by itself. "Ask before deploys" makes it wait for the owner on anything outside READ_ONLY.
- **A failed step stops autopilot.** Read its log, fix the cause, remove or replace that line in PENDING and rewrite PENDING to resume.
- **Reading results:** stage the log and UPDATES-DONE, then read them. To find a log, list `logs\` and sort by mtime.
- **Chains:**
  - `ship` saves the PC repo to GitHub as Orion, then auto-queues `leyline` (export to the public Leyline repo), which auto-queues `leyline-launcher` (the same export into a clean clone at `.repos\leyline-launcher-clean`; it keeps `www/`, the Pages and uptime workflows, and the signing files).
  - Never point it back at `.repos\leyline-launcher`: that is an old working folder holding `.env`, game data and `_local`.
  - The launcher build that players get comes from GitHub CI on the Leyline repo after `leyline` pushes.
- **The steps you'll use most:**

| Step | Does |
|---|---|
| `signin` | Deploys the website (portal modules, art, JSON) and runs `stream/fastloop.sh`: Caddy redirects and the self-check, ending "FASTLOOP OK" |
| `ship` | Saves to GitHub as Orion, then `leyline`, then `leyline-launcher` |
| `check` / `audit` | Read-only server check / release audit (the last audit was 88/93 before today's fixes) |
| `den-test` | The real launcher exe plays Your Den headless into the world (round 7 passed) |
| `scripts-reload` | Live `reload ale` of realm scripts |
| `server-fix` | Container memory and limit fixes (wu-play has 512 MB) |
| `sv-site` (also `sv-record`, `sv-export`, `sv-preview`) | Azeroth Survivors site and recording |
| `addons-curate`, `mods-daily`, `thumbs` | Mods discovery (approves nothing; plan in docs/MODS-DAILY.md) |
| `discord-preview` / `discord-apply` / `discord-check` | Discord redo. The owner pastes the bot token in Control's Discord card; it is kept in `tools/discord-leyline/token.txt`, git-ignored, never logged. `discord-apply` only runs from the card after a fresh preview and a typed YES, never from the queue |
| `fresh-leyline-check` / `fresh-leyline`, `fresh-check` / `fresh-history` | Rewrite repo history to one Orion commit (to wipe Chumpton). Needs CONFIRM files the owner makes; see §8 |
| `worldlog`, `bugs`, `gm` | Diagnostics, the bug manifest, and the GM console |

The full list (about 100 steps) is the STEPS dict in `tools/control/control.py`.

## 7. What's live now (as of 22:20, 2 Nov)

- **Website:**
  - Official and Community realm sections.
  - Pruned sign-in: no Discord. `/wrath/key` is Log in (key file, paste, or the key kept in this browser), then Google, then Create a Hero Key, then "What is a Hero Key?".
  - The old `/login` and `/key` redirect to `/wrath/signin` and `/wrath/key` (Caddy).
  - The tab favicon; Survivors live at survivors.leylinelauncher.com.
  - New mods, all pending: 44 upstream addons, 48 index seeds, 33 candidates, 2 held HD packs, Performance and High looks.
  - Epoch water moved back to pending: its download points at a blocklisted lure account.
- **Control:** the Discord card (1308b).
- **Launcher, queued as 1309a (ship) at 22:20:**
  - **The sign-in screen first** (`ui/gate.js` / `gate.css`):
    - Log in with your browser, Google, the Hero Key file (drop or choose), a pasted key, or name and password;
    - Sign up (Create your Hero Key);
    - offline with a saved sign-in still gets in;
    - if the website is down, "Continue without logging in".
  - **Official PLAY never starts signed out** (it opens sign-in, then goes on).
  - **Game folders** named "Wrath 3.3.5a" and so on, in era order. Old auto names are tidied on load (clients.rs `tidy_name`).
  - **Seamless official/community switching:** Den login lines are removed from Config.wtf before other realms (herokey.rs `clear_login`); play bar lines clear on change.
  - **Wording:** "Log in" everywhere; "Game folders"; realm rows show "PvE · Bots", not server software.
  - **Earlier fixes:** Your Den's Play goes straight in, Official and Community tabs, Your Den only in the realm picker, the slate icon, the slimmer Log In button.
- **Den Lite:** answers the game's account data (key bindings), so walking works.

## 8. Open work (pick up here)

1. **Check 1309a ran.** Read `logs\1309a-ship.log`, `1309a-leyline-leyline.log` and `1309a-leyline-launcher-leyline-launcher.log`. Then check the Leyline repo's "leyline launcher" CI build passed (`gh run list -R orionopenwow/leyline`). Ask the owner to try the new launcher:
   - the sign-in screen;
   - Play on Wrath Unleashed reaching character select by itself;
   - switching to a community realm and back.
2. **Build the real fresh-machine test** (`docs/FIRST-RUN-TEST.md`; `playtest.rs` holds the CLI contract, not yet called from main.rs):
   - wire `playtest::parse_cli` in main.rs/app.rs beside `den-play`, with `LEYLINE_APP_DIR` for a fresh profile;
   - add `GET audit/presence` in stream/herokeys.py;
   - add a test character via `play335`;
   - add `tools/first-run/first_run.py`, its .bat, and a `first-run-test` Control step.

   The owner wants "fresh install, through to in the game" proven.
3. **Wipe Chumpton from history:**
   - After the latest ship, read the current main SHAs of `orionopenwow/leyline` and `orionopenwow/leyline-workspace` (`gh api repos/<r>/commits/main --jq .sha`).
   - Give them to the owner. He makes `tools\fresh-history\CONFIRM-leyline` (the Leyline SHA) and `tools\fresh-history\CONFIRM` (the workspace SHA), then runs `fresh-leyline` and `fresh-history`. The classifier blocks you from writing those files.
   - Any ship in between changes the SHAs.
   - Leyline-Launcher has no Chumpton commits.
4. **Leyline-Launcher's old "devkit" CI job** fails on the removed Open Vanilla files. Ask the owner whether to remove it or make it skip those checks.
5. **The Survivors signed-out redirect** still points at `play.openvanilla.app`; change it to the new name. Also give the `signin` step's closing message the new name.
6. **Survivors as a downloadable mod** (local launcher mode): asked for, not built. The Survivors permission statement from Aethus is still needed from the owner.
7. **Owner decisions waiting:**
   - **Realm Seeds plan** (Claude Doc, artifact `B9Q9XVDWkLsoydT8UuUfTs`).
   - **Touch controls plan** (Claude Doc, artifact `RotJG1ctmhZTouXrukoHGm`; updated for an instant press on touch, see thread `82b5ddf0-8813`).
   - **The Discord TODO(owner) lines** (docs/discord-initial-posts.md).
   - **Mods questions:** Masque/AtlasLoot folder names with a brand word, textures named like the game's interface art, ReShade being an "injector", the Modern Renderer credit, the 10-a-day target and the daily cloud task.
   - **Names:** whether to keep the "Azeroth Survivors" and "Wrath" names.
8. **Re-run `audit` and `den-test`** after the deploys. Verify `.bug`/`.wu` and the Hero Key checks. Update the morning report artifact (`LL4iJU7qdNLCRknAwyEJ2U`, file `scratchpad/report/index.html`).
9. **UX findings not yet fixed:**
   - Home's first banner slide is empty until its picture loads;
   - Settings opens on "Advanced: peer-to-peer";
   - Installed opens on the Mods tab right after an addon install.

## 9. Working style that works with this owner

- **Start replies with one sentence on what you're doing, then do it.** End with what changed and what he needs to press.
- **Big or parallel jobs:**
  - use background agents, each in its own `git worktree add -b agent/<name> /home/claude/wt-<name> HEAD`;
  - give each a tight brief (rules, files, tests, "commit often, don't push, no cargo build");
  - then cherry-pick and test yourself.
- **Read Control's logs to confirm a step**, rather than assuming it worked. The website self-check lines are in the `signin` log.
- **Docs he comments on** (Claude Docs): answer in the doc's thread with Claude Docs `create` (object `utterance`, parent = the thread root), and edit with `update` and `answering`.
- **Report artifacts:** the morning report artifact is updated in place.
