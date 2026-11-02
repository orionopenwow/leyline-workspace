# The hybrid client: the player's own desktop game, started like the browser game

Owner, 1 Nov: "it should be the custom browser type thing we have but a hybrid ... robust launcher login and wrath
server test and join server thing in the client and the browser and have them both working the same way and with
the forever u.i as the default for the launcher as well 1:1".

Goal: when Leyline starts the player's own stock desktop client (build 12340), it behaves like the browser game at
wrath.openvanilla.app: no intro movie, no login typing, straight to character select (or into the world), our
loading screens, Forever UI as the interface, and the same "hop to another realm" flow.

Status (2026-11-01, branch `agent/hybrid-client`): research done; a first prototype is built and tested on Linux
(the MPQ writer, the glue sign-in hook, launcher wiring behind a switch that is off). **Nothing here has run on a
real Windows PC yet.** The one test that decides most of the design is T1 below.

Words: this document is for the owner and contributors. Nothing a player sees (glue text, launcher text) carries a
game brand name or a version number; the glue's tip lines are the browser's own list (`stream/www-files/wu-loading.js`).

## My Den, one click (1 Nov, after the owner's test at 16:39)

The owner's start stopped at the login screen with only `DEN` filled in because `hybrid::write` needs Forever UI's
login screens in the game folder (their `GlueXML.toc`): on his folder the archive was never written, so nothing
signed in (and even then the Den's handoff stopped at character select). Now (`launcher/src-tauri/src/denclick.rs`):

- **Stock game (any folder):** the launcher signs in with its own keys, posted to the game window only for the step
  Den Lite's milestone log (`den-lite.events`) says didn't happen: the local password at the login screen, OK at the
  realm list, Enter World at character select (Den Lite's ready-made character). Every step has a timeout.
- **Where the game takes changed login screens** (Forever UI's glue in the folder): our login screen code goes first
  (account `DENAUTO`, "Your Den", a character made if there is none, straight into the world), the keys only fill in.
  A game that closes on it (a game program that checks its login screen files, T1) is remembered
  (`<game>/Leyline/glue-refused`) and started again without it. The archive goes in `Data\patch-Z.MPQ` and
  `Data\<locale>\patch-<locale>-Z.MPQ` (whichever folder wins). The handoff is taken out when the game closes.
- **Testing on the owner's PC:** Control `den-check` (read-only) and `den-test` (unattended: `Leyline.exe den-play
  --test`, a screenshot, logs in `tools\den-check\logs`), `den-test-invite` (a test friend joins too).

## Verdicts

| # | Question | Verdict | What decides it |
|---|---|---|---|
| 1 | Custom glue (login screens) from a patch archive on a stock exe | **Unverified, contested.** Art: works. Addons: work. Changed glue code: sources disagree | T1 |
| 1b | Building that archive from our files and the Forever UI files already on the PC, no game data | **Works** (built: `mpq.rs`, tested) | T2 (open it in an MPQ tool) |
| 2 | Instant login with no typing and no stored real password | **Works if 1 works** (built: one-time login in a per-start glue file, taken out after) | T1, T3 |
| 2b | Realm picked and last character entered automatically | **Likely works** (stock `realmName` CVar + glue `ChangeRealm`/`EnterWorld`, built behind `enter`) | T4 |
| 3 | Our loading screens | **Partly.** Glue-time cover (spinning mark, bar, tips): built, needs 1. World loading screens: art only (static image + the game's bar), no spin, no live tips | T6 |
| 4 | Realm hop like the browser's | **Not in one process.** Closest: Join in game, the game closes, Leyline signs in to the new realm and starts it again, straight to that realm (about 10 to 20 s) | T7 (after build) |
| 5 | Window and no-movie defaults | **Done** (`gfx.rs prepare_launch`): `movie 0`, `readEULA/readTOS/readScanning/readContest 1`, `gxWindow 1` + `gxMaximize 1` | T5 |

## 1. Custom glue and art from a patch archive

### What the client reads

- The client reads its archives in a fixed order and the last one wins. Custom patches are `Data/patch-<4..9, A..Z>.MPQ`
  (numbers, then letters; higher wins) and the same per locale, `Data/<locale>/patch-<locale>-<X>.MPQ`
  ([wotlkdev: client theory](https://wotlkdev.github.io/wiki/theory/client); the community guide says the same, "Patches are
  initialized in numerical, then alphabetical order":
  [A Guide to Modernizing the WotLK Client](https://github.imc.re/Bobblybook/A-Guide-to-Modernizing-the-WotLK-Client)). The
  two sources disagree on whether root or locale patches win over each other on English clients
  ([warcraft-rs patch chain summary](https://warcraft-rs.readthedocs.io/en/latest/guides/wow-patch-chain-summary.html) says
  locale wins; wotlkdev says root wins on English clients). Forever UI's own installer puts everything, its
  `GlueXML.toc` included, in `Data/patch-Z.MPQ`, "the archive the game reads last"
  ([mod-forever-ui](https://github.com/AbyssalRealm/mod-forever-ui)). We use the same slot.
- Loose files: the client reads a loose file only when no archive has that path. So new art (Forever UI's
  `Interface/ForeverUI/*.blp`, addon textures) works loose, but a changed `GlueXML.toc` must come from an archive: a
  loose one is ignored (our own `play/Install Forever UI.bat` notes this: "the archives' own wins").
- AddOns (`Interface/AddOns/*`) are loose and unchecked: Forever UI's in-game addon and WrathUnleashedUI work on a
  stock exe. Texture and model patches in archives work on a stock exe (the common HD packs).

### The contested part: changed glue code on a stock exe

- **For:** Forever UI's installer writes `Interface\GlueXML` (with its own `GlueXML.toc`) into `Data\patch-Z.MPQ` and
  "does not patch `Wow.exe`"; its optional exe patch is only for character-create swatches
  ([mod-forever-ui](https://github.com/AbyssalRealm/mod-forever-ui)). Glue skins such as
  [ElvUI Glues](https://github.com/Marotheit/ElvUI-Glues-WotLK) ship as `patch-7.MPQ` with no exe mentioned.
- **Against:** a 2026 tested report: "the stock exe rejects changed login files", "the stock exe and the single
  0x1F41BF byte both refuse the archive", only a five-spot exe patch loads it
  ([WoWAndroWhere PR #17](https://github.com/danielnoam/WoWAndroWhere/pull/17)). Project Reforged ships an "Interface
  Signature Bypass" that "removes the signature check that triggers the 'Your game/login interface files are corrupt'
  error" ([patcher](https://github.com/Stormhand-dev/WoW-3.3.5-Patcher---Project-Reforged)). A modder who changed
  `AccountLogin.xml` got "my interface files are corrupted" and the game closed
  ([archive thread](https://elijahketchersid.com/forums/thread-90337.html)); another says editing those files "requires
  the GlueXML check to be off" ([archive thread](https://elijahketchersid.com/forums/thread-85622.html)).
- **Our reading:** the stock exe very probably checks the glue code and refuses changed files with "Your login
  interface files are corrupt". Forever UI's claim may hold only for players whose exe was already patched. **T1
  decides.** We never ship or make a changed exe in this prototype.

### If T1 fails (the stock exe refuses changed glue)

Pick one, owner's call:

- **B1, the interface-check patch** on the player's own exe: a few known bytes, only on an exe that matches build 12340
  byte for byte, a backup first, undone with the switch (the way Forever UI's optional patcher and the PR above do
  it). It is not a decompiled client, but it changes the player's game program and the byte list comes from other
  people's reverse engineering: an owner decision. Everything below then works as designed.
- **B2, stock only, no glue code:** Forever UI in game (the addon), Forever UI's login art only where it is new
  files, our loading art (section 3), the account name filled in (`SET accountName`, as today), and the launcher types
  the one-time password into the game window itself (`PostMessage` `WM_CHAR` + Enter to the game's window, the way
  multibox tools drive background game windows). No glue cover, no realm pick, no auto enter world.
  Fragile timing (the password box must have focus); a one-time password limits the harm of a mistyped send.

### The archive (built)

`launcher/src-tauri/src/mpq.rs`: a writer for the archive format the client reads (format version 0, 4 KB sectors,
zlib per sector when smaller, encrypted hash and block tables, a `(listfile)`), plus a reader for exactly that subset.
Tests: round trips of small, big, empty, incompressible and exactly-two-sector files, case-insensitive names, refused
names (`..`, absolute, `:`, non-ASCII, `(listfile)`, repeats), byte-identical output for the same input, and the two
well-known table keys (`(hash table)` = `0xC3AF3770`, `(block table)` = `0xEC83B3A3`), which only come out right when
the crypt table and string hash are the format's.

`launcher/src-tauri/src/hybrid.rs` decides what goes in `Data/patch-Z.MPQ`:

| path in the archive | from |
|---|---|
| `Interface\GlueXML\GlueXML.toc` | Forever UI's own toc in the player's `Interface/GlueXML` (its installer or `play\Install Forever UI.bat` put it there), plus one line `LeylineHybrid.xml` after its last `ForeverUI*.xml` line. Refused if it doesn't list Forever UI's glue (never a stock toc). |
| `Interface\GlueXML\ForeverUI*.lua/.xml` | the player's own copies of Forever UI's glue files (nothing else from that folder) |
| `Interface\GlueXML\LeylineHybrid.xml`, `LeylineHybrid.lua` | ours: `launcher/src-tauri/glue/` |
| `Interface\Glues\Leyline\LeylineMark.tga` | ours: the Leyline mark (`make-wuui-media.py`) |
| `Interface\GlueXML\LeylineSession.lua` | ours, per start: the one-time login, or one line that sets nothing |

No game data: the only third-party files are Forever UI's, read from the player's PC where the player put them; we
never host or download them in this prototype. The slot is used only when it is free or holds an archive Leyline
wrote (it has `LeylineHybrid.lua` inside and is under 32 MB); Forever UI's own 330 MB `patch-Z.MPQ` is never touched,
and then the switch says so and the old way is used. (Later: Leyline installs Forever UI itself, art loose and glue in
our archive, so the two never compete for the slot. The mod manager hands patch slots out from `4` to `Z`
(`mods.rs PATCH_SLOTS`); `Z` should be taken off that list when this ships.)

## 2. Instant login

### Chosen: a one-time login in a per-start glue file

1. Hero Key sign-in as today (`herokey.rs`, docs/HERO-KEY-SSO.md): the website makes a one-time password `P` for this
   player and this realm, the realm's agent sets its SRP verifier and scrambles it after `login_ttl` (600 s for the
   launcher; at most 900).
2. With Settings > Testing > **Sign in by itself** on (`Settings.hybrid_login`, off by default), `point_and_play_with`
   writes the archive with `LeylineSession.lua` = `LEYLINE_SESSION = { user = "\072\075...", pass = "...", realm = nil,
   enter = false }`, every byte a `\ddd` escape (no plain text to grep), right before starting the game.
3. `LeylineHybrid.lua` (loaded after Forever UI's glue) copies the table into a local and clears the global at load.
   When the login screen (`AccountLogin`) shows, it calls `DefaultServerLogin(user, pass)` once (what the stock Login
   button calls: `DefaultServerLogin(AccountLoginAccountEdit:GetText(), AccountLoginPasswordEdit:GetText())`,
   [AccountLogin.lua as quoted here](https://elijahketchersid.com/forums/thread-53100.html)) and drops the password.
   A cover (dark screen, the spinning mark, a gliding bar, a tip every 7 s) stays up until character select.
4. Anything unexpected takes the cover down and stops: an error dialog (`GlueDialog`), no login screen in 20 s, no
   character select in 45 s, any Lua error (every step is `pcall`-guarded). The stock screen is then as it was, so the
   player can always sign in by hand. Tested in `glue/test_hybrid_glue.lua` against a stand-in for the glue screens
   (9 cases; `hybrid.rs` runs it under Lua 5.1 when one is installed).
5. The session is taken out again: `scrub_when_closed` retries every 15 s from 90 s after the start (the rewrite fails
   while the game holds the archive open, so on Windows it lands when the game closes), and every plain start scrubs
   a session left from before, so a start Leyline didn't sign in for never signs in by itself.
6. The launcher's realm card says "The game signs you in by itself" and shows no password; if the archive couldn't be
   written it says why and shows the password as before.

### Why not the other channels

- **A CVar in Config.wtf** (`SET leylineToken`): as far as we know the client keeps only CVars it knows: an unknown
  `SET` line is not readable from glue with `GetCVar` and is dropped when the client writes the file back on exit, and
  we found no source that glue can register a CVar and pick a stored value up. Unverified, and not needed: the
  archive is a channel we control.
- **The real password in Config.wtf or anywhere**: never. The one-time `P` stops working after one login window and is
  out of the file after that start.
- **A command-line argument**: the client takes none for a login.

### Security

| | |
|---|---|
| Lifetime | `P` works for `login_ttl` (the realm scrambles the verifier after it); the archive holds it for one start only |
| One use | each Hero Key sign-in makes a new `P`; the nonce is single-use at the realm |
| On disk | in the player's own `Data/patch-Z.MPQ`, as escaped bytes, then rewritten without it. File permissions are the game folder's (the player's own) |
| Who can read it | anything running as the player during that start; the same as the browser keeping the session in memory, a bit longer. The glue keeps no copy after the call |
| Logs | the launcher never logs the session; the UI no longer shows `P` when the game has it |

### Realm and character

- **Realm:** the stock client goes straight to the realm named by `SET realmName` when that realm is on the list
  (the name it remembers from the last visit; T4 confirms). A
  Beacon has one realm, so this is usually moot. With several, the glue picks `realm` by name on the realm list
  (`GetRealmCategories`, `GetNumRealms`, `GetRealmInfo`, `ChangeRealm`; guarded, falls back to the list). T4.
- **Character:** `enter = true` enters the world with the selected character (the stock client selects
  `lastCharacterIndex`) once character select is up and has a character. Used for hops; a plain start stays on
  character select, like the browser. T4.

### The official realm (not built)

The browser gets its game login from the website (`/api/play`, `wrath_portal.py signed_in_game_login`). The launcher
needs the same through its own token: a `social::call(app_dir, "play_login", {})` endpoint on the portal that answers
`{user, pass}` for the signed-in player (better: a one-time pass like Hero Key's), then
`point_and_play_with(..., Some(Session { .. }))` for `OFFICIAL_REALM`. Portal work, owner's call on whether the
website should hand the game password to the launcher at all.

## 3. Loading screens

- **Browser:** `wu-loading.js` puts the page's own cover (spinning mark, bar, tips) over the client's loading screens.
- **Desktop, glue time** (sign-in to character select, and Enter World until the game's loading screen):
  `LeylineHybrid.lua`'s cover, the same look and the same tips. Built; needs T1.
- **Desktop, world loading screens** (Enter World, teleports, instances): drawn by the game itself, not by Lua. It shows
  one picture per map (the paths come from the client's own loading-screen table, under
  `Interface\Glues\LoadingScreens\`, with a wide variant) and its own bar. What we can do without an exe patch:
  - replace each picture with **our own art** under the same path in our archive: a dark background, the Leyline mark,
    and a tip baked into the picture. Each map's picture can carry a different tip, and the launcher can rewrite them
    each start, so tips change between sessions. No spinning (it's one still image) and no live tip rotation;
  - restyle the bar with our own textures under the bar's paths (`Interface\Glues\LoadingBar\...`; T6 confirms names).
  - Steps: (1) a BLP2 writer (palettized, 8-bit, our own pixels only) in `hybrid.rs`; (2) the list of picture paths,
    read from **the player's own archives** at run time (their `(listfile)` entries under `Interface\Glues\LoadingScreens`),
    never shipped: needs the reader extended to encrypted/compressed list files, or a fixed list of names checked on the
    owner's PC; (3) tips drawn by the build script into our art (`make-wuui-media.py` style, Pillow), not at run time.

## 4. Realm hop

- **Browser now** (`docs/leyline-realm-hopping.md`): Join in the Leyline window or on character select sets
  `WUUI_LEYLINE_HOP`; the page takes it, gets a Hero Key assertion, redeems it at the relay and reloads the game
  against the new realm's bridge, straight to its character select.
- **Desktop now:** the in-game window lists this server's realms (`ov_leyline.lua`) and, for another realm, puts
  `leyline://realm/<id>` in a copy box for the launcher.
- **The constraint:** the client reads its realm address, its login and its glue only at start; glue Lua has no
  network and no way to tell the launcher anything; in-game Lua can only write SavedVariables (on reload, logout and
  quit). So a hop to another server cannot happen inside one running client.
- **Closest equivalent (to build):**
  1. Join in the Leyline window (desktop, no page): `WrathUnleashedUIDB.leylineHop = { id = <realm id>, at = time() }`,
     a "The game restarts on <realm>" line, then `Quit()` (the client writes SavedVariables as it quits).
  2. Leyline waits on the game process (it holds the `Child`); when it exits, it reads
     `WTF/Account/<ACCOUNT>/SavedVariables/WrathUnleashedUI.lua` with a strict pattern (no Lua is run), takes the hop
     only if `id` is a live listed realm id and `at` is within two minutes of the exit, and clears it.
  3. Hero Key sign-in for that realm, `point_and_play_with(..., Some(Session { enter: true, .. }))`, a different
     client if that realm needs another version (`realms::pick_client`).
  4. The game starts on the new realm, signs in by itself and enters the world: about 10 to 20 s on an SSD.
  - "Back to Wrath" is the same with the official realm (after section 2's portal step).
  - On character select, the glue's REALMS button keeps the stock list of this server's realms (stock `ChangeRealm`).

## 5. Window and start defaults

`gfx.rs launch_values`, every launch: `movie 0` (no intro movie), `readEULA 1`, `readTOS 1`, `readScanning 1`,
`readContest 1`, `showToolsUI 0`, and unless the player chose the window on the Graphics page, `gxWindow 1` +
`gxMaximize 1` (a window over the whole screen with no border, so the screen's resolution doesn't switch). These are
the names the client itself writes into `WTF/Config.wtf`. Also used: `accountName` (`herokey.rs`), `realmList`
(`clients.rs`, kept in step with `realmlist.wtf`), `realmName` and `lastCharacterIndex` (the client's own, kept).
To check on the PC (T5): that a brand-new folder with no `Config.wtf` doesn't override these on its first hardware
check (`hwDetect`), and that the window covers the taskbar.

## Prototype: what is on the branch

- `launcher/src-tauri/src/mpq.rs`, `hybrid.rs`, `glue/LeylineHybrid.{xml,lua}`, `glue/LeylineMark.tga` (our mark),
  `glue/test_hybrid_glue.lua`.
- `settings.rs`: `hybrid_login` (default off). `app.rs`: `point_and_play_with` (writes or scrubs the session),
  `scrub_when_closed`, `set_hybrid_login` (off takes our archive out of every client), `hybrid_status`.
  `herokey.rs`: `Login.auto` / `auto_why`. UI: Settings > Testing > "Sign in by itself"; the realm card's message.
- Tests: `cargo test --no-default-features --lib` (all 333 pass; 13 new across `mpq::` and `hybrid::`), and
  `lua5.1 launcher/src-tauri/glue/test_hybrid_glue.lua launcher/src-tauri/glue/LeylineHybrid.lua`.

## Risks

- T1 fails on a stock exe: B1 (exe patch, owner decision) or B2 (stock only, typed by the launcher).
- Forever UI's glue changes upstream: our toc line goes after its last `ForeverUI*.xml`; a toc without one is refused.
- The game keeps the archive open: the scrub waits for it to close (by design). A crash of the launcher before then
  leaves the expired session in the file until the next start scrubs it.
- Two copies of the game from one folder: the second start can't rewrite the archive and falls back to typing.
- Archive priority across root and locale folders isn't settled (sources disagree); T1 also shows which toc wins.

## What the owner tests on a Windows PC

Use a copy of the game folder for T1 to T4. Build the launcher from this branch the usual way (`Ship the Leyline launcher.bat` makes the installer).

- **T1, the deciding test.** Install Forever UI with its own installer (or `play\Install Forever UI.bat`) and start
  the game from its own exe, no Leyline: do Forever UI's login screens show, or "Your login interface files are
  corrupt"? Then remove its `patch-Z.MPQ`, turn on Settings > Testing > Sign in by itself, and use Sign in with your
  Hero Key on a Beacon (the `herokey_sso` flag on): same question for our archive.
- **T2.** Open `Data\patch-Z.MPQ` (ours) in Ladik's MPQ Editor: it opens, lists 6 or more files, and
  `Interface\GlueXML\GlueXML.toc` ends with `ForeverUIGlue.xml` then `LeylineHybrid.xml`.
- **T3.** With T1 passing: the game opens, the dark cover with the spinning mark shows, and it lands on character
  select with nothing typed. Close the game, wait 20 s, open the archive again: `LeylineSession.lua` is the one line
  `LEYLINE_SESSION = nil`. Start the game from its own exe: the normal login screen, no sign-in.
- **T3b.** Turn the switch off: `Data\patch-Z.MPQ` is gone.
- **T4.** On a realm with two realms on its list: does it go to the right one? (Needs the realm name in the session;
  today `realm = nil`.) And `enter = true` (change it in a test build): does it enter the world?
- **T5.** A copy with no `WTF\Config.wtf`: the first start is windowed and borderless, no intro movie, no legal screens.
- **T6.** List in the MPQ editor the paths under `Interface\Glues\LoadingScreens\` and `Interface\Glues\LoadingBar\`
  in your own archives, and send the names only (no files): that is the list our loading art replaces.
- **T7** (after the hop is built): Join a Beacon in the Leyline window; the game closes and reopens on it, in the world.
