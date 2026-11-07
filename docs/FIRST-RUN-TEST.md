# First-run test: a fresh machine, from the download into the world

Owner, 2 Nov: "run through a first time install test and up to the full game and getting into the game", "all
features from exe pro tested and quality and user experience tested fully", "smoke tested fresh machine".

Two halves:

1. **In the window, every screen** (done, runs anywhere): `launcher/tests/firstrun.browser.test.cjs` walks a brand-new
   player through the whole launcher with a fake core that starts empty. See [the UI walk](#1-the-ui-walk-automated-fake-core).
2. **On the owner's PC, the real thing** (the plan below): the real `Leyline.exe` from the release download, a fresh
   data folder, a game folder added, signed in with a test Hero Key, PLAY on Wrath Unleashed, character select, then
   the world. Most of it runs by itself from Leyline Control as the step `first-run-test`; the owner watches one screen.

**3 Nov:** built. `Leyline play-test --fresh --game <folder>` (main.rs, app.rs), `GET audit/presence` (stream/herokeys.py),
`tools/first-run/first_run.py` with `First run test.bat` and the Control step `first-run-test` (PASS / FAIL per phase,
unit-tested with fakes in `tools/first-run/test_first_run.py`). The owner's manual checklist is docs/OWNER-TEST-PLAN.md.

**3 Nov, later:** grown into the **fresh computer test**: every feature, the computer's own needs, cookies, drag and
drop, a Windows Sandbox run. Plan and numbers: docs/FRESH-COMPUTER-TEST.md (Control `fresh-test`, `fresh-sandbox`;
`first-run-test` runs the same tool). Its report numbers are that plan's part B, not the table in 3.5 below.

Rules this test keeps: no game or publisher names on any screen; nothing typed by a person into a sign-in; no account
made by a person's route (the audit's own test-account API, `account/new {audit: true}` and `audit/close`, is the one
allowed way, as `tools/audit/herokey.py` does it); keys and passwords never on a command line, in a log or a report.

## 1. The UI walk (automated, fake core)

```
cd launcher && node --test tests/firstrun.browser.test.cjs
FIRSTRUN_SHOTS=<folder> node --test tests/firstrun.browser.test.cjs     # also NN-name.png per step + findings.txt
```

In order: first launch (no settings, no game folders), the tour (4 steps), the client picker, Find your game files,
a Wrath folder (the addons offer, Not now) and a Vanilla folder, Home, Realms (Official, Community), the realm picker
and the official realm, PLAY (the addons offer again, Install and play), Your Den from the picker, Mods & Addons
(Discover › Mods, › Addons, install an addon, Installed › Mods, › Addons), Host, Forum, News, Settings, Agents, Help,
the Log In button, its chooser, signed in, the account panel, Manage account, PLAY signed in. Screens at 1100x760, a few
at 1400x860. Every step fails on a page error or a game/publisher name, and notes (findings.txt) the old site name, a
version number in a sentence, cut-off text and a sideways scroll.

The sign-in is one helper, `signIn(page)`: when the sign-in gate lands (people log in first), change that helper and
move its call to just after step 1.

## 2. The real PC: what is there today

| Piece | Where | Status |
|---|---|---|
| The release download | `https://github.com/orionopenwow/Leyline/releases/download/launcher-latest/Leyline.exe` (one portable file, built by tools/github-workflows/leyline-launcher.yml on every ship) | there |
| A fresh data folder | `LEYLINE_APP_DIR=<absolute folder>`: the launcher's settings, sign-in token, Den Lite, addons, themes, caches all go there (`settings::app_dir`, every caller goes through it); tools/den-check already reads the same name | **added now** (settings.rs `APP_DIR_ENV`, tested) |
| A fresh window profile | The window's own storage (remembered tab, filters) is WebView2's, in `%LOCALAPPDATA%\app.openvanilla.leyline\EBWebView`, not in the data folder. `WEBVIEW2_USER_DATA_FOLDER=<folder>` should move it (WebView2 lets that variable override the app's choice): check on the first run that the tour shows and Home opens on Home | to check once |
| A test Hero Key | `tools/audit/herokey.py`: `POST /wrath/api/social/account/new {"audit": true}` → token, key, made-up name; `POST audit/close` with the token closes it | there |
| Headless game client | `tools/audit/play335.py`: the 3.3.5a protocol (char enum, char create, enter world, chat) | there |
| The real launcher, no window | `Leyline den-play --test` (denclick.rs): Your Den, signed in, into the world, report in `den-click.json` | there (Your Den only) |
| `Leyline play-test` | `playtest.rs` (arguments), main.rs (`--fresh`: an empty data folder), app.rs `play_test_cli` (`--game` added when new, the test key from `LEYLINE_TEST_HEROKEY`, PLAY, report) | there (3 Nov) |
| Presence | `GET /wrath/api/social/audit/presence` (stream/herokeys.py; the audit's test account only) | there (3 Nov; deploy with `signin`) |
| Control step `first-run-test` | tools/control/control.py `STEPS`, tools/first-run/first_run.py | there (3 Nov) |

## 3. The plan: `first-run-test`

### 3.1 The launcher: `Leyline play-test --official --fresh --game <folder> --world [--hold <s>]`

Arguments and names are fixed in `playtest.rs` (`parse_cli`, `CLI_MARK = "leyline-play-test-v1"`, `KEY_ENV =
"LEYLINE_TEST_HEROKEY"`, `REPORT = "play-test.json"`). To build (main session; app.rs and main.rs are shared with
the agents changing PLAY and realm switching, so this waits for their work to land):

1. **main.rs**, beside `den-play`: `if let Some(cli) = leyline_lib::playtest::parse_cli(&args)` → on `Err(e)` print it
   and exit 2; on `Ok(cli)`: when `cli.fresh`, before anything else reads the settings,
   `std::env::set_var(settings::APP_DIR_ENV, playtest::fresh_dir(&std::env::temp_dir(), now_ms))` and create it; then
   `std::process::exit(leyline_lib::play_test_cli(cli))`.
2. **app.rs `play_test_cli(cli)`** (about 120 lines, the same shape as `den_play_cli`):
   - `black_box(playtest::CLI_MARK)` so the tool can find it in the program;
   - a `Core` on `settings::app_dir()` (fresh: empty);
   - `--game`: `clients::detect(&path)`, `clients::new_profile`, saved, selected (what `add_client` does); report
     `added: {build, complete, missing}`;
   - the key: `std::env::var(playtest::KEY_ENV)`, then `social::sign_in_key(&dir, &key)`; drop the variable and
     the string at once; report `signed_in: true` and the account's name only;
   - PLAY: the realm list (`realms::fetch` / `cached`), the official one, then `play_official(&core, &client, &addr)`
     with `Session.enter = cli.world` (today `play_official` hard-codes `enter: false`: give it a parameter, false
     from the PLAY button); report `launch` (the message), `hybrid` (auto sign-in or why not);
   - wait for the milestones (below), `--hold` seconds, a screenshot (`denclick::shot_marker`-style, the same
     PrintWindow code den-test uses), close the game, scrub the one-time login (`hybrid::scrub`), write the report,
     exit 0 only when the asked milestone was reached.
3. **Stock game folder** (no Forever UI login screens, so the glue can't sign in): the den-test's key driver
   (`denclick::Driver`, posted window messages, Windows only) types the one-time login from the website's
   `game/login` at the login screen, Enter at the realm list, Enter at character select. That password is the
   site's one-start login for the test account, never a person's. Report which way it went (`glue` or `keys`).

### 3.2 Knowing where the game got to (official realm)

Den Lite writes milestones; the official realm doesn't, and the game can't write a file while it runs. Two ways,
both automatic:

- **The site says (preferred, one small endpoint)**: `GET /wrath/api/social/audit/presence` with the test account's
  token → `{"account_online": bool, "character": name|null, "in_world": bool, "map": id}` from the auth and character
  databases (`account.online` is set once the game reached character select; `characters.online` once in the world).
  Only for accounts made with `{audit: true}`, 404 for any other. `play-test` polls it every 2 s (token from its own
  sign-in): character select = `account_online`, world = `in_world`. The stream side (stream/herokeys.py, beside
  `audit/close`) has to add it; then run the Control step `signin` to deploy.
- **A watcher (no site change)**: the first-run tool's own headless player (`play335.py`, a second test account) logs
  in and sends `/who <name>` every 5 s; the test character showing up means in the world. Character select can't be
  seen this way, so the report says "world: seen by /who" or "not seen".

### 3.3 The test character

A fresh account has no character, and `--world` enters with the last one. Before starting the exe, the tool logs in
headlessly once with the account's game login (the site's `game/login`, as `play-test` will) and makes one with
`play335.char_create` (name from the made-up account name, Human Warrior), then logs out. If the site's
`game/login` isn't available to a token from the tool, `play-test` does it instead with `--make-character`
(a small extra: the glue already makes a Den character when there is none; the same Lua call on the official realm).

### 3.4 The tool and the Control step

New `tools/first-run/first_run.py` (standard library, `-I`), and `tools/first-run/First run test.bat` in the same
shape as `tools/den-check/Den test.bat`:

1. **Download**: fetch `launcher-latest/Leyline.exe` into `.launcher\first-run\Leyline.exe` (a new copy, not the
   owner's); check it is over 1 MB, a Windows program (`MZ`), and that it contains `leyline-play-test-v1`
   (else: "Update the launcher (the release with play-test), then run first-run-test again.").
2. **Fresh folders**: `%TEMP%\leyline-first-run-<time>\data` and `...\webview`; nothing of `%APPDATA%\Leyline` is read
   or written (checked: its settings.json is the same before and after).
3. **Game folder**: the owner's Wrath folder from his own settings (`%APPDATA%\Leyline\settings.json`, the first 3.3.5a
   client; read only), passed as `--game`. Its `Config.wtf` and `Data\patch-Z.MPQ` slot are stashed first and put back
   after (`den_check.stash` / `unstash`), so his game is left as it was.
4. **Test account**: `account/new {audit: true}` (one a run, inside the site's 3 an hour); the key goes only into the
   child's environment as `LEYLINE_TEST_HEROKEY`; the recorder hides it.
5. **Run**: `Leyline.exe play-test --official --fresh --game <folder> --world --hold 20` with
   `LEYLINE_APP_DIR=<data>`, `WEBVIEW2_USER_DATA_FOLDER=<webview>`; up to 6 minutes.
6. **Read** `<data>\play-test.json`, the presence answers, the screenshot.
7. **Clean up, always** (also on a failure or Ctrl+C): game closed, `audit/close` (the account, its sign-ins and its
   character gone; its token answers 401 after), the temp folders removed, the owner's files put back.
8. **Report**: `tools\first-run\logs\first-run-latest.txt` and `.png`, PASS / FAIL per line with the fix.

Control (`tools/control/control.py` `STEPS`, after `den-test`):

```python
"first-run-test": ("tools/first-run/First run test.bat", "First-run test: today's download in an empty data folder, your game folder added, a test Hero Key signed in, PLAY on Wrath Unleashed, character select, into the world; a screenshot, then everything removed (logs in tools\\first-run\\logs)", False),
```

### 3.5 Pass / fail lines (automated)

| # | Line | Pass when |
|---|---|---|
| 1 | The download | Leyline.exe fetched, over 1 MB, `MZ`, has `leyline-play-test-v1` |
| 2 | Fresh start | the data folder was empty; after the run it holds settings.json with exactly one client and `seen: []` |
| 3 | Your own launcher untouched | `%APPDATA%\Leyline\settings.json` and the token file unchanged (hash before = after) |
| 4 | Game folder added | `added.build == "3.3.5a"`, `complete`, nothing missing |
| 5 | Test account | `account/new` 200 with a key; never in a line or the report |
| 6 | Signed in | `signed_in: true`, the made-up name shown |
| 7 | PLAY started | `launch` starts with "Started", the game process up within 30 s |
| 8 | Signed in by itself | `hybrid` ok (`glue`), or `keys` on a stock folder; FAIL if the login screen waited for a person |
| 9 | Character select | presence `account_online` within 90 s |
| 10 | In the world | presence `in_world` (or `/who`) within 60 s more |
| 11 | Screenshot | the .png is there, not black |
| 12 | One-time login gone | `hybrid::has_session(game)` false after |
| 13 | Clean up | `audit/close` 200, then the token 401; temp folders gone; the owner's Config.wtf and patch slot as before |
| 14 | No secrets | the key, token and game password appear in no log, report or the exe's own logs |

### 3.6 What the owner watches (about 10 minutes, once per release)

The automated run has no launcher window. These need eyes; open the fresh launcher by hand with the same folders:

```
set LEYLINE_APP_DIR=%TEMP%\leyline-fresh\data
set WEBVIEW2_USER_DATA_FOLDER=%TEMP%\leyline-fresh\webview
.launcher\first-run\Leyline.exe
```

(Close your normal Leyline first.) Then:

1. **The tour shows** on first open, 4 steps, Done closes it. *Fail*: no tour, or Home opens on another page (the
   window profile wasn't fresh: see the WebView2 line in section 2).
2. **Windows' first-run prompt**: SmartScreen asks once (More info, Run anyway; not code-signed yet). *Fail*: any other
   prompt, a firewall prompt before PLAY, or an error dialog.
3. **Add your Wrath folder** from Find your game files. *Pass*: "Added …" and READY TO PLAY in the bar.
4. **Log In** with Get your Hero Key (this is the player's own route: a new account is fine here, it's yours).
5. **PLAY**: the game opens and goes to character select by itself. *Fail*: the login screen waits for typing.
6. **Enter the world** with a new character. *Pass*: in the world within a minute; your name in the bar.
7. Close the game; delete `%TEMP%\leyline-fresh`.

## 4. Findings from the UI walk (2 Nov)

Fixed in this change: the Your Den badge said "Open on this PC (Den Lite)" (an internal name) → "Open on this PC";
the first-install toast said "Your first mod is in" after an addon → "Your first mod or addon is in".

Open (in files other agents are changing, or the owner's call), most important first:

1. **PLAY's result is cut off** in the play bar at the default window size: "Started My Wrath on Wrath Unleashed.
   Sign in on Ho…". The one next step (sign in) is the part cut. (app.js play bar; app.rs `official_note`.)
2. **"Log In" vs "Sign in"**: the button says Log In; its card, PLAY's message ("Sign in on Home (top right)"), the
   Forum ("sign in on Home or Manage account") and Help say Sign in, and "Home (top right)" points at the title bar,
   which isn't Home. One word, and "the Log In button, top right". (social.js, app.rs, forum.js)
3. **The play bar keeps an old line**: after signing in it still says "Sign in on Ho…" until the next PLAY; after
   picking Your Den from the realm list the bar still shows Wrath Unleashed · READY TO PLAY with PLAY's old message.
   (app.js play/realm code)
4. **Server software names on realm rows**: "AzerothCore", "vMaNGOS" under each realm name (app.js `CORE_LABELS`).
   Jargon for players, and the first carries a franchise place name.
5. **"Azeroth Survivors"**: the featured mode's name and key art on Home use a franchise place name. Owner's call
   (`TODO(owner)` already in modes.js).
6. **Home's first banner slide is a dark empty box** until the website's picture loads (offline it stays empty),
   and the carousel then sits on slide 2 for the rest of the session.
7. **Settings opens on "Advanced: peer-to-peer"** with two empty number boxes, under a lone "Game clients" button
   with no heading. A first-timer meets the advanced part first.
8. **Installed opens on Mods** ("No mods in this client yet") right after the player installed an addon; it could open
   on the shelf that has something.
9. **"Game Clients"** is the page's title (title case, and the only place "clients" is said), while the bar, Help and
   Find your game files say "game folder"; the added line reads "Added My Vanilla: 1.12.1 (5875), detected from the
   executable." (a version and a build number in a sentence).
10. **Manage account** says the name "was made for you (a guest's or the default)": awkward for a new Hero Key.
    (account.js)
11. The first-install toast sits over the card it is about (its Installed button) for 9 s at 1100x760.

## 5. Leyline Solo: a fresh profile, through to character select

Plan Phase 5. Leyline Solo is the standalone first-era app (its own repo `orionopenwow/leyline-solo`, the page
`/wrath/solo`, the player guide `docs/leyline/solo.md`). Its fresh-machine test is
`tools/first-run/solo_first_run.py`, run by `tools/first-run/Solo first run test.bat` and the Control step
**`solo-first-run`** (not READ_ONLY: it starts the game and signs it in; up to 4 hours). Unit-tested with fakes in
`tools/first-run/test_solo_first_run.py`.

### 5.1 The fresh profile, and what it refuses

The app keeps its data in `%APPDATA%\Leyline Solo` (the app sets `LEYLINE_APP_DIR` to it unless one is given); the
Phase 0 engine tool keeps its world in `%LOCALAPPDATA%\Leyline\solo-proof` (`LEYLINE_SOLO_DIR` moves it). The test
makes `%TEMP%\leyline-solo-first-run-<time>\app` and `...\solo`, both empty, and gives every step those two
variables. It **refuses to run** (and touches nothing) when:

- the test folder is, holds or sits inside one of the owner's own: `%APPDATA%\Leyline Solo`, `%APPDATA%\Leyline`,
  `%LOCALAPPDATA%\Leyline` (and its `solo-proof`), `%LOCALAPPDATA%\Leyline Solo`; or it is not new and empty
  (`--work`);
- a Solo port is in use (3724, 8085, 13336, 7878): the owner's own world, or another server, is running;
- the game folder already has `Data\patch-Z.MPQ` (the owner's own Solo sign-in: `solo-signin-off` first);
- this is not Windows.

Before and after, it fingerprints the owner's folders (their `state.json`, `settings.json`, `SOLO password.txt` and
top-level names): any change is a FAIL. The game folder's files sign-in may change (`Config.wtf`, every
`realmlist.wtf`, `patch-Z.MPQ`) are kept in memory and compared byte for byte at the end; put back by the test if
sign-in off didn't.

### 5.2 What runs by itself

| # | Check | How |
|---|---|---|
| S1 | a fresh profile | above |
| S2 | install (only with `--installer <setup.exe>`) | the NSIS setup program run silently into the test folder (`/S /D=<work>\install`), `Leyline Solo.exe` there; `uninstall.exe /S` at the end. Skipped when Leyline Solo is installed for real (it would take over its Start menu entry): test the installer in Windows Sandbox instead |
| S3 | the folder is the first-era client | `solo_proof.py check <folder>` |
| S4 | setup | `solo_proof.py setup <folder> [--zip]`: the engine (checked), the maps, the database, the account; `SETUP OK` |
| S5 | start | `solo_proof.py start`: `START OK` once the world says World initialized |
| S6 | sign-in (Quick) | `solo_proof.py signin <folder>`: `SIGNIN ON` |
| S7 | the game, to character select | the game's program started from the folder; alive; character select = **either** the world server's log line for the account SOLO logging in (`Server.log`, `mangosd.out`) **or** the login database marking SOLO on the realm (`account.active_realm_id`); both reported |
| S8 | clean up, always | the game the test started closed, the world stopped, `signin --off` (`SIGNIN OFF`), the game folder byte for byte, the owner's folders unchanged, no password in the Solo tool's lines or the world's logs, the test folder removed (`--keep` keeps it) |

The steps S3 to S6 run the Phase 0 engine tool (`tools/solo/solo_proof.py`), which is the same engine, profile and
sign-in the app is built on. When the app gets a headless test mode (a `solo-test` command, like the launcher's
`play-test`), S3 to S7 should call it instead, so the app's own code is what is tested. TODO (solo-app): that mode.

### 5.3 What it cannot do, and the owner's checklist

The test never drives the game's own screens: it cannot press Quick start, type in chat or fight. After S7 it prints
**OWNER now** and watches for `--hold` seconds (default 600), reading the test world's database and `Char.log`:

| The owner does | The test sees | Line |
|---|---|---|
| Quick start (or makes a character on the creation screen) | a character on the SOLO account | `OWNER a character made` |
| enters the world | a character online, or `Login Character:[Name]` in `Char.log` | `OWNER a character in the world` |
| types `.bot group` | more characters of SOLO online (companions are characters on the same account) | `OWNER companions in the world (.bot group)` |

Each is PASS when seen and INFO when not (never FAIL: it is the owner's part). The rest is by eye, with a screenshot
each: the yellow "signing you in" line, character select by itself, the party following and fighting, Invite a friend
(a second PC joins: same network, then through the relay), Characters (Play on a character goes straight in),
Settings, Simple sign-in (the account name filled in, the password typed once), Remove sign-in (the folder as it was).

### 5.4 To confirm on the first real run

- **The log line for character select.** The test looks for the world socket accepting `SOLO`'s session
  (`CHARSEL_RE`). Whether CMaNGOS writes it at the Solo profile's log level is not known yet. The login database is the
  second signal, so a missing line alone doesn't fail S7; if only the database says yes, tighten the pattern to the
  line the log does show (or raise the profile's log level in `tools/solo/conf/solo.conf`).
- **`account.active_realm_id`** is the CMaNGOS login database's column for "on a realm" at this pin; if the column has
  another name, S7 shows "the login database: not read".
- **The installer's silent switches** (`/S`, `/D=`) are NSIS's standard ones; check once with the first release.
