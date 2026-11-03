# First-run test: a fresh machine, from the download into the world

Owner, 2 Nov: "run through a first time install test and up to the full game and getting into the game", "all
features from exe pro tested and quality and user experience tested fully", "smoke tested fresh machine".

Two halves:

1. **In the window, every screen** (done, runs anywhere): `launcher/tests/firstrun.browser.test.cjs` walks a brand-new
   player through the whole launcher with a fake core that starts empty. See [the UI walk](#1-the-ui-walk-automated-fake-core).
2. **On the owner's PC, the real thing** (the plan below): the real `Leyline.exe` from the release download, a fresh
   data folder, a game folder added, signed in with a test Hero Key, PLAY on Wrath Unleashed, character select, then
   the world. Most of it runs by itself from Leyline Control as the step `first-run-test`; the owner watches one screen.

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
| `Leyline play-test` | `launcher/src-tauri/src/playtest.rs`: the arguments and names, parsing tested | **contract only**: not called from main.rs yet |
| Control step `first-run-test` | tools/control/control.py `STEPS` | to add |

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
