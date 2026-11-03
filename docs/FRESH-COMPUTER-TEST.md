# Fresh computer test: every feature, from a brand-new PC

Owner, 3 Nov: "a clean test from a fresh computer to test every feature with a bat: like login, select client, torrent
drag and drop, dens instant sign in and more, all needed assets on fresh computer and cookies — plan."

**The short version.** Double-click `tools\first-run\Fresh computer test.bat` (Control step **`fresh-test`**). It
downloads today's launcher into an empty profile and runs 30 numbered checks (about 60 report lines) by itself, then cleans up. For a
truly new Windows, run **`fresh-sandbox`**: the same test inside Windows Sandbox. What only you can check is in
[part D](#d-what-only-you-can-check-about-15-minutes): about 15 minutes, once per release.

The report is `tools\first-run\logs\first-run-latest.txt`: one PASS / FAIL / INFO line per check, each FAIL with what
to do. The numbers in the report are the numbers in [part B](#b-every-feature-numbered).

This grew from the first-run test (docs/FIRST-RUN-TEST.md, step `first-run-test`). That step now runs the same tool.

## Rules the test keeps

- **No person's account, no typed password.** The test account comes from the website's own test-account API
  (`account/new {audit: true}`), and it is closed after with `audit/close`. Its key goes only into the launcher's
  environment (`LEYLINE_TEST_HEROKEY`), never onto a command line, into a log or into the report.
- **No game data moves.** The test uses your own game folder where it is (or a copy inside the sandbox). Nothing of the
  game is downloaded, uploaded or shared. The drop test uses tiny made-up files (a zip header, an empty `.torrent`).
- **Your own launcher is left alone.** Everything happens in a temp profile. Your `%APPDATA%\Leyline` settings and
  sign-in are hashed before and after (check 29). The files PLAY changes in your game folder are saved first and put
  back after (check 27). Loadouts put your mods back exactly (check 25).
- **No secrets printed.** Every line goes through a filter that hides the key, the token, the game login and the
  website cookie. Check 30 looks for them once more.

## A. What a fresh Windows PC needs, and what it gets

| What | Where it comes from | On a fresh PC | Checked by |
|---|---|---|---|
| **The launcher** | `Leyline.exe`, one portable file from the `launcher-latest` release (GitHub CI builds it on every ship). No installer. | Downloaded from the website's download page. Windows SmartScreen asks once (not code-signed yet). | 1 (size, a Windows program, its checksum from `SHA256SUMS.txt`) |
| **WebView2** (the launcher's window) | Part of Windows 11, and of Windows 10 through Edge updates. The launcher does not bring it. | Usually there. If missing, the window can't open. | 2 (registry version) |
| **Visual C++ runtime** | Rust programs built with MSVC use `vcruntime140.dll`. The release build does not link it statically today. | Often there (many apps install it), but **not promised** on a clean Windows. | 3 (the DLLs; if missing, did the launcher still run?) |
| **Your game folder** | Yours. **Never shipped**, never downloaded by Leyline. | You point the launcher at it (Find your game files, or drop it). | 6, 7, 8 |
| **Leyline addons pack** | `leyline-addons` release, `addonpack.json` (addons.rs). Downloaded into your game's AddOns on first PLAY. | Downloaded. Must be public. | 4 |
| **Den Lite** (Your Den's server) | `den-lite-latest` release of the workspace repo, checked against its `.sha256` (host/lite.rs). | Downloaded on first Your Den. Must be public (it is: checked 3 Nov, HTTP 200 without sign-in). | 4, 23 |
| **The launcher's saved sign-in** | `social-session.json` in the data folder: a launcher token (the website keeps only its hash, and a website session for it, encrypted). | None: log in. | 11, 12, 13 |
| **The website's cookies** | `wr_session` (the sign-in, set by the sign-in service), `ly_hk` (the Log in form's token, `/wrath/key` only, Strict), `ly_next` (where to go after the profile step). | None: log in on the website. | 14 (HttpOnly, Secure, SameSite; log out ends it) |
| **The window's own storage** | WebView2 profile in `%LOCALAPPDATA%\app.openvanilla.leyline\EBWebView` (remembered tab, filters, the tour seen). | Empty: the tour shows. | 5 (`WEBVIEW2_USER_DATA_FOLDER` points it at the temp folder); by hand: M1 |
| **Firewall prompts** | None for PLAY or the official realm (outgoing only). Den Lite asks Windows once **only** when you first Invite over your own network (its direct listener on ports 3724 and 8085). | One prompt, at that moment. | By hand: M4 |

**Where a fresh PC keeps Leyline's things** (this is also the whole "uninstall"; the launcher has no uninstall button):

- `Leyline.exe`, wherever it was downloaded.
- `%APPDATA%\Leyline`: settings, sign-in, Den Lite, Dens, caches.
- `%LOCALAPPDATA%\app.openvanilla.leyline`: the window's storage.
- In each game folder, `Leyline\` (the mod list, disabled mods, loadouts and their WTF copies) and what Leyline put in
  `Interface\AddOns`. Mods & Addons › Uninstall takes out exactly what Leyline put in.

## B. Every feature, numbered

**How:** *Auto* means `fresh-test` checks it every run, with the check named. *CI* means GitHub's tests check it on
every ship (`cargo test`, the launcher's UI tests in `launcher/tests/*.browser.test.cjs`). *Step* means another
Control step. *You* means only you, by hand ([part D](#d-what-only-you-can-check-about-15-minutes)).

### This computer

| # | Check | How it is proven |
|---|---|---|
| 1 | Today's launcher downloads: over 1 MB, a Windows program, the checksum matches `SHA256SUMS.txt`, it has `play-test` and the feature checks | Auto |
| 2 | WebView2 is there | Auto (registry); You: the window opens (M1) |
| 3 | The Visual C++ runtime is there, or the launcher runs without it | Auto; Sandbox shows the real answer |
| 4 | The addons pack and Den Lite can be downloaded with no sign-in | Auto (anonymous download of each small file) |
| 5 | The launcher starts on an empty data folder and an empty window profile | Auto (`fresh: {asked, empty}`) |

### Game folders and drag and drop

| # | Check | How it is proven |
|---|---|---|
| 6 | Your Wrath folder is found (from your own launcher's settings, read only, or `--game`) | Auto |
| 7 | It is added in the fresh launcher as Find your game files does: build, complete, nothing missing | Auto (`play-test --game`) |
| 8 | Your other versions are added too, in the same profile | Auto (`--game` more than once); the era order and names: CI (clients.rs `tidy_name`) |
| 9 | Each kind of drop on the game folder box does the right thing (below) | Auto (`--checks`: the launcher's own `clientimport::classify_with`); the real mouse drop: You (M3) |

**What the launcher does with each drop (from the code, 3 Nov):**

| Dropped | What happens | Check 9 expects |
|---|---|---|
| Your game folder (or a `file://` link to it) | Added where it is (the folder or one up to 3 levels inside). | added |
| A zip of your game folder | Unpacked into a folder you pick (default `<data>\Game clients\<name>`), checked for zip tricks (no `..`, no links, size caps), then added. | zip |
| An `https` link to your own backup zip | Downloaded with a progress bar, then unpacked as above. Leyline never suggests a link. | link |
| A magnet link, a `.torrent` file, an `https` link to a `.torrent`, or a browser shortcut (`.url`) to a magnet | **Refused** with one sentence: "Leyline can't download game clients from torrents. Drop your own client folder or zip instead." | refused |
| A 7z, rar, iso … | "Leyline can open zip files only. Unpack it yourself, then drop the folder." | not a zip |
| Anything on the sign-in screen | Read as a Hero Key file only (gate.js `dropPath`). | 11 (the same reading) |
| Files anywhere else in the window | Mod files for Mods › Installed. | CI (addons, mods tests) |
| Model files on the Your Den card | My models: checked, kept on this PC, put in the Den's own patch. Never uploaded; a friend sees a stand-in. | `den-features` |

**The torrent switch, honestly.** The refusal above is the default. There is an owner-only testing switch,
Settings › Testing › "Allow torrent downloads of game clients (testing only)", saved on this PC only
(`leyline-testing.json`), **off** in a fresh profile. Turned on, a dropped magnet or `.torrent` **is** downloaded with a
real BitTorrent engine into the folder you pick, and with "Share while downloading" or the Nodes chip on, **this PC
uploads pieces of it, and keeps sharing after the download until Stop**. Leyline's servers and its own P2P network
never carry any of it. But a player's PC sharing a client torrent is P2P carrying game files. Check 9 FAILs if a
fresh profile ever starts with the switch on. See owner decision 1.

Separate from that, Leyline's own `magnet:` links (p2p/magnet.rs, behind the `magnet_links` flag, off) open only
Leyline's own checked packages: mods, themes, the Host pack. Never game data. They don't go through the game folder box.

### Log in, cookies, your account

| # | Check | How it is proven |
|---|---|---|
| 10 | A test account from the website's test-account API | Auto |
| 11 | Launcher: log in with the Hero Key file, found the way the sign-in screen finds one in Downloads (the same reading a dropped file gets) | Auto (`social::find_key_file`, `key_file_text`) |
| 12 | Launcher: log out (the token revoked on the website and forgotten here) | Auto |
| 13 | Launcher: log in with the key pasted, with the spaces and "Key:" a paste brings | Auto |
| 14 | Website: the Log in page; log in with the key as a browser does (the form and its token); the sign-in cookie is HttpOnly, Secure and SameSite; signed in on the next page; Log out ends the session (the old cookie no longer signs in) | Auto (`web_lines`); the page's shape is also checked by the portal's herokeys tests |
| – | Log in with your browser (the launcher opens the website, you approve) | CI (gate); You (M2) |
| – | Log in with Google | You (M2): a real Google account can't be automated, and we never type passwords |
| – | Log in with name and password | CI (account); not automated on the real site (it would mean typing a password) |
| – | "Share a character" = play on another PC: your characters live on the realm and come with your account. Move the Hero Key file. | 11 proves the file logs in on a fresh profile; You: the same characters (owner test plan 23–26) |
| – | Sign-up (Create your Hero Key) in the launcher | CI (gate); the website's own: `audit` |

### Play

| # | Check | How it is proven |
|---|---|---|
| 15 | A test character on Wrath Unleashed (the headless player) | Auto |
| 16 | PLAY on Wrath Unleashed: the website gives the game login, the game starts | Auto |
| 17 | The game logs in by itself (the login screen hand-over, or the launcher's keys on a stock folder) | Auto |
| 18 | Character select (the website's `audit/presence` says the login was given) | Auto |
| 19 | In the world (`audit/presence` sees the character) | Auto |
| 20 | Screenshots of the game window | Auto (`tools\first-run\logs\NN-*.png`) |
| 21 | The one-time login taken back out of the game folder | Auto |
| 22 | A community realm and back: the game folder pointed at a community realm from the live list, then at the official realm again | Auto (`--checks`, without starting the game a second time); playing on a community realm: You (M5) |
| – | Logged out, PLAY opens Log in, then goes on by itself | CI (gate) |
| – | Play in your browser | `check`, `audit` |

### Your Den

| # | Check | How it is proven |
|---|---|---|
| 23 | Your Den: instant sign-in (no login screen), walk, build, save and load, the white room, a shareable Den file, an invite a test friend signs in with | Auto (`den-test` on the same fresh profile) |
| 24 | The Den editor: gallery, place, select, move, turn, copy, paste, delete, undo, redo, save, open, every panel | Auto with `--den-features` (`den-features`) |
| – | Places, My models (drop a model on the Your Den card) | `den-features`; You (M6) |
| – | Invite over your own network, over the relay, or at your own address | `den-test` tests the invite through the relay; the same network and your own address: You (M4) |
| – | Dojo / combat | Its own tests (sandbox.rs, `Dojo.lua`); You (M6) |

### Loadouts, mods, modes, website

| # | Check | How it is proven |
|---|---|---|
| 25 | Loadouts: a loadout of what is on now, Clean (every Leyline mod off), back again; then your mods and the loadouts file exactly as before, and the WTF copies it made removed | Auto (`--checks`, `playtest::loadout_round_trip`, after the game closed) |
| – | Mods & Addons: install, turn off and on, uninstall | CI (addons, firstrun); You (M7) |
| – | Game modes; Azeroth Survivors in the browser | `sv-site-check`; You (M8) |
| – | Website: forum, submit a link, profile | Portal tests (forum, listings, profiles) on every change; You (M9) |

### Clean up (always, also after a FAIL or Ctrl+C)

| # | Check | How it is proven |
|---|---|---|
| 26 | The test account closed (`audit/close`); its token no longer works (401) | Auto |
| 27 | Your game folder's Config.wtf, realmlist and login archive put back | Auto |
| 28 | The temp folders removed (`--keep` keeps them, to open that launcher by hand) | Auto |
| 29 | Your own launcher untouched (settings and sign-in hashed before and after) | Auto |
| 30 | No secrets in the log, the console or the saved report | Auto |

## C. A clean machine: where to run it

| Way | What it proves | Trade-offs |
|---|---|---|
| **On your PC, temp profile** (`fresh-test`) | Every launcher and website feature from an empty profile, with your real game. | Not a new Windows: WebView2, the runtime, fonts and the firewall are your PC's. Your game folder is used in place (its changed files are put back). |
| **Windows Sandbox** (`fresh-sandbox`) | A brand-new Windows every time: the real answer for WebView2, the Visual C++ runtime, SmartScreen, the firewall. | Needs Windows Pro, Enterprise or Education, with Windows Sandbox turned on (Windows Features, one restart). Your game folder is mapped **read-only** and copied inside (a few minutes, and as much of the sandbox's own disk as the game takes, thrown away after). The game runs on the sandbox's virtual GPU, which may be slow. Python is this PC's, mapped read-only (not the Microsoft Store's: it can't be mapped). |
| **A second Windows user** | A fresh user profile (`%APPDATA%`, the window storage, cookies, the browser) on the real GPU. | Same Windows, so not a test of WebView2 or the runtime. Make the user yourself (we never create accounts); log in to it and run `Fresh computer test.bat`. |
| **GitHub's Windows runner** (CI) | The parts with no game: the download and its checksum, the runtimes on a clean image, the drops, log in by file and paste, the website's cookie, clean-up: `first_run.py run --no-game`. | Makes one test account per run from GitHub's network, against the real website (3 an hour per network). **Not added yet**: owner decision 3. |

**The sandbox, in detail.** `fresh-sandbox` writes `tools\first-run\logs\sandbox\Leyline fresh computer.wsb` and
`out\run.cmd`, then opens Windows Sandbox on it. The `.wsb` maps:

- the workspace to `C:\leyline`, read-only;
- this PC's Python to `C:\py`, read-only;
- your Wrath folder to `C:\game-ro`, read-only;
- `logs\sandbox\out` to `C:\out`, the only writable folder.

It turns networking on and the clipboard and printers off. At logon, `run.cmd`:

1. copies only the test's own folders to the sandbox's disk. That is `tools\first-run`, `tools\den-check`,
   `tools\audit` and `hosting\kit\tools`, never all of `tools\`, which holds saved tokens;
2. copies the game to `C:\fresh\Wrath`;
3. runs `first_run.py run --game C:\fresh\Wrath`;
4. copies the logs to `C:\out`.

The report is `logs\sandbox\out\logs\first-run-latest.txt`, and the console is `out\console.txt`. Close the sandbox
window when it says done. Everything inside is gone. `--no-game` makes a quick sandbox run without the game.

**Running it by hand:**

```
tools\first-run\Fresh computer test.bat                    everything (about 10 minutes)
tools\first-run\Fresh computer test.bat --den-features     plus the Den editor's feature test
tools\first-run\Fresh computer test.bat --no-game          no game folder: the computer, log in, the website, drops
tools\first-run\Fresh computer test.bat --keep             keep the fresh profile, to open it by hand (part D)
tools\first-run\Fresh computer test - sandbox.bat          in Windows Sandbox
```

## D. What only you can check (about 15 minutes)

Run `Fresh computer test.bat --keep` (or use Windows Sandbox and do this inside it). Close your normal Leyline, then open
the fresh copy:

```
set LEYLINE_APP_DIR=<the folder the log names>\data
set WEBVIEW2_USER_DATA_FOLDER=<the folder the log names>\webview
.launcher\first-run\Leyline.exe
```

| # | Do | You should see |
|---|---|---|
| M1 | Open it | SmartScreen once (More info, Run anyway), then the sign-in screen first. No other prompt, no error. After Log in: Home and the 4-step tour. |
| M2 | Log in with your browser; log out; Continue with Google | Each logs you in. A browser already logged in only asks you to approve. Always "Log in" / "Log out", never "Sign in". |
| M3 | Drag your game folder from Explorer onto the play bar's game folder box; then a `.torrent` file; then a magnet link from a web page | "Added …"; then the one refusal sentence twice. Nothing downloads. |
| M4 | Your Den › Invite (first time) | Windows' firewall asks once for Den Lite (private networks is enough). A friend on your network joins; one elsewhere joins through the relay. |
| M5 | Realms › Community › Play on one; then Wrath Unleashed › PLAY | That realm's game; then the official realm, logged in by itself. No old line left in the play bar. |
| M6 | Your Den: Places, drop a model on the card (My models), the Dojo | Each opens and works as its own test says. |
| M7 | Mods & Addons: install an addon, turn it off, uninstall; Loadouts: Clean, then back | It shows, goes, comes back; Clean is the game as installed. |
| M8 | Home › Game modes › Azeroth Survivors › Play | It opens in the browser. |
| M9 | Website: Log in with your key, the forum, submit a link, your profile; Log out | Each works; after Log out the site shows Log in. |
| M10 | Delete the fresh folder (or close the sandbox) | Nothing of the test left. |

## E. What CI must confirm (the Rust here wasn't compiled where it was written)

- `cargo test --lib` in `launcher/src-tauri` compiles and passes. In particular, these new tests in `playtest.rs`:
  `several_game_folders_and_checks`, `drops_as_the_game_folder_box_takes_them`, `judge_is_strict` and
  `loadouts_clean_and_back_leave_the_folder_as_it_was`.
- The Windows release build compiles `app.rs` (`play_test_cli --checks`, `realm_switch_check`) and keeps
  `leyline-play-test-checks-v1` in `Leyline.exe`. Check 1 says so after a ship.

## F. Owner decisions

1. **The torrent testing switch** uploads game-client pieces when "Share while downloading" or Nodes is on, and keeps
   sharing after the download. That is P2P carrying game files on a player's PC. Remove sharing (download only), remove
   the switch before release, or keep it as it is for your own testing?
2. **The Visual C++ runtime.** If check 3 FAILs in the sandbox, should the launcher be built with a static runtime (one
   build setting, a slightly bigger exe), or should the download page name the redistributable?
3. **A CI job** running `first_run.py run --no-game` on GitHub's Windows runner on every ship: yes or no? It makes and
   closes one test account per ship against the real website.
4. **An "uninstall" in the launcher** (Settings › Remove Leyline from this PC: the data folder, the window storage and
   what Leyline put in game folders), or keep the list in part A?
5. **`fresh-test` in Autopilot.** It isn't in `READ_ONLY`, because it starts your game and makes a test account.
   Should Autopilot run it by itself after each ship?
