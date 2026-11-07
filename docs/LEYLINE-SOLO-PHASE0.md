# Leyline Solo, Phase 0: in the world with a bot party

Leyline Solo is a standalone single player for the **first-era game** (client build 5875) on the player's **own**
game folder: a local first-era server with companion bots, offline, signing in by itself. Phase 0 builds no app. It
proves the three risky parts by hand on the owner's PC before Phase 1 writes one.

| # | The risk | How Phase 0 proves it | Where |
|---|---|---|---|
| 1 | CMaNGOS Classic **with playerbots** builds natively for Windows on GitHub CI and runs with no Docker | the `solo-engine` workflow builds it, sets up a fresh database from the package, starts the login server and signs the local account in (CI smoke), and publishes the zip | `.github/workflows/solo-engine.yml` (source copy `tools/github-workflows/solo-engine.yml`) |
| 2 | The engine's own extractors read the maps from the player's own first-era folder | `solo-setup` runs `ad`, `vmap_extractor` and `vmap_assembler` (and, with `--mmaps`, `MoveMapGen`) against the owner's folder, read-only | `tools/solo/solo_proof.py setup` |
| 3 | **The fast sign-in**: the first-era login screen takes our own sign-in add-on from a patch archive | `solo-signin` puts our add-on in `Data\patch-Z.MPQ`, with the login screen's own file list plus one line | `tools/solo/glue/`, `solo_proof.py signin` |

**The gate:** the owner opens his own game, it signs in by itself and lands on character select, he makes a
character, calls a party of companions, walks and fights a little, and sends a screenshot.

## What was built

* **The engine** (`solo-engine.yml`, windows-2022, MSVC). It builds CMaNGOS Classic at the host kit's pin
  (`hosting/kit/profiles/cmangos-1.12.1/profile.lock`: core `5731249`, world database `603ece1`). Playerbots come from
  `cmangos/playerbots` at `6823423` (`tools/solo/solo.lock`). That is the last module commit on or before the core
  commit's date: the module follows the core's master, and newer commits use core changes the pin lacks. The build
  uses `BUILD_PLAYERBOTS=ON` and `BUILD_EXTRACTORS=ON`, Release. The core's own `FetchContent` is pointed at our pinned
  checkout, so it never takes the module's `master`. The databases are made by the project's own installer
  (`InstallFullDB.sh -InstallAll`, which includes playerbots' SQL), then dumped into one file the PC imports in one go.
  The job fails if any game-data file is in the sources or in the package. The core's Warden module binaries
  (`warden_modules`) are not ours to ship, so they are left out and Warden is off.
  * Release `solo-engine-latest` holds `leyline-solo-engine-win64.zip` (about 90 MB to download, about 360 MB
    unpacked), its `.zip.sha256` and `leyline-solo-engine-win64.json`.
  * The zip holds:
    * `bin/`: mangosd, realmd and their DLLs, with `conf-dist/` holding the core's, playerbots' and anticheat's `.conf.dist`;
    * `tools/`: ad, vmap_extractor, vmap_assembler, MoveMapGen, `config.json` and `offmesh.txt`;
    * `mysql/`: a trimmed MySQL 8.4, the Built-in engine's;
    * `sql/solo-databases.sql`;
    * `source/`: the raw SQL of the core, playerbots and the world database;
    * `conf/`: the Solo profile;
    * `LICENSES/`, `NOTICE.txt`, `SOURCES.txt` (every source at its commit) and `solo-engine.json`.
* **The Solo profile** (`tools/solo/conf/`):
  * `solo.conf`:
    * only 127.0.0.1;
    * level cap 60;
    * normal (not PvP) world;
    * 2x experience (`TODO(owner)`: the rate);
    * SOAP on 127.0.0.1 only, for the clean stop.
  * `solo-aiplayerbot.conf`: a quiet world. There are no random bots: no bot accounts are made, no bots are logged in, and no bots queue for groups or battlegrounds. Companions are the player's own, through the commands below. The chat model is off.
  * `solo-anticheat.conf`: Warden is off.
* **The tool** `tools/solo/solo_proof.py` (standard library only, Windows Python 3), with its MPQ module `mpq112.py`
  and the add-on in `glue/`. Its home is `%LOCALAPPDATA%\Leyline\solo-proof\`:
  * `engine\`;
  * `data\`: your maps;
  * `db\`;
  * `run\`: the server settings;
  * `logs\`;
  * `state.json`.
* **The Control steps:**
  * `solo-check` (read-only);
  * `solo-setup`;
  * `solo-start`;
  * `solo-stop`;
  * `solo-status` (read-only);
  * `solo-signin`;
  * `solo-signin-off`.

### Where the game folder comes from

The tool looks in three places, in this order:

1. The step's argument.
2. `tools\solo\folder.txt`: one line, your first-era game folder's path. It is git-ignored and is yours to fill.
3. The launcher's saved game folders (`%APPDATA%\Leyline\settings.json`): the first folder whose build is the first
   era, with the selected one first.

So if the launcher already lists your first-era folder, there is nothing to do. Otherwise put the path in `folder.txt`.

### Passwords

`solo-setup` generates four passwords:

* the database's root password;
* the server programs' own database login;
* the playing account `SOLO`;
* the stop account `SOLOSTOP` (game master level 3, used only for "server shutdown" over SOAP on 127.0.0.1; it never plays).

They are kept in `state.json` and in the server settings files beside it. They are never printed or logged: every
server line the tool shows is scrubbed, and the CI smoke fails if a password is in any log. Accounts are stored as
SRP6 verifiers, made exactly as the core makes them (tested against the core's own sample rows). `solo-signin` also
writes `SOLO password.txt` in that folder, for you to open yourself if you ever have to type it.
`solo-signin-off` deletes that file.

## The owner's 5-minute checklist

Press each step in Control and wait for its last line.

1. **`solo-check`**: every line PASS (or INFO). It names the folder it found and where it found it.
   "the login screen's file list: read from your patch.MPQ" (or another archive) means the add-on can be built.
2. **`solo-setup`**: wait; it says how long each part took.
   * Download and unpack: a few minutes.
   * Maps: a few minutes.
   * vmaps: 10 to 30 minutes.
   * Database import: 5 to 15 minutes.
   * It ends `SETUP OK`.
   * The monster paths are skipped for now. Creatures then walk straight at you. `setup --mmaps` adds them later, in 1 to 3 hours.
3. **`solo-start`**: it ends `START OK` once the world server says "World initialized". `solo-status` shows the same
   and also signs `SOLO` in on the login server.
4. **`solo-signin`**: it ends `SIGNIN ON`. It makes three changes to your game folder, backing up first:
   * `realmlist.wtf` is set to 127.0.0.1;
   * `WTF\Config.wtf` gets the account name `SOLO` and the realm "Leyline Solo";
   * our archive `Data\patch-Z.MPQ` is added.
5. **Open the game from your own folder** (its own program, not the launcher).
   * A yellow line at the top says *Leyline Solo: signing you in to your own world...*
   * **Does it land on character select by itself?** Screenshot.
6. **Make a character**, enter the world, then type in chat:
   * `.bot group`: four companions of the right classes at your level join your party. Screenshot.
   * Walk to a few monsters and fight. Screenshot with the party in the fight.
7. **`solo-stop`**, then **`solo-signin-off`**: it ends `SIGNIN OFF: your game folder is as it was.`
   * Every file it changed is back byte for byte, and our archive is gone.
   * **Close the game first.** It holds the archive open, and it rewrites Config.wtf when it closes.
   * The game's own settings changes from this test go with the restore.

### The companion commands (playerbots at `6823423`, typed in chat by the player)

| Command | What it does |
|---|---|
| `.bot group` | Makes 4 new companions (a tank, a healer and damage dealers) at your level on your own account and adds them to your party. `.bot group size=3` makes a smaller party |
| `.bot create class=priest level=10` | Makes one companion; add `race=`, `gender=`, `name=` as you like |
| `.bot add <Name>` (also `.bot login <Name>`) | Brings one of your own characters in as a companion and adds it to your party |
| `.bot remove <Name>` (also `logout`, `rm`) | Sends it away |
| `.bot list` | Your companions now in the world |
| `.bot gear <Name>`, `.bot prepare <Name>` | Best gear; gear, food and potions |
| `.bot levelup <Name>` | One level up |
| `.bot delete <Name>` | Deletes that companion character |
| `.bot help <command>` | The module's own help |
| In party chat: `follow`, `stay`, `attack`, `flee`, `summon`, `release`, `revive` | Orders to the whole party |

Companions are real characters on the `SOLO` account. They count toward its 10 characters per realm and show on
character select.

## What each result means for Phase 1

| Result | Means | Phase 1 |
|---|---|---|
| CI green, `solo-setup` OK, `START OK` | The engine works with no Docker on a real PC | The launcher installs this zip as a second Built-in engine (`engine.rs`: a CMaNGOS core beside AzerothCore) |
| Maps OK from your folder | The extractors read a real first-era folder | Extraction becomes a progress step in the launcher; monster paths optional |
| Signs in by itself to character select | Our glue add-on in a patch archive works on the stock first-era login screen | The launcher's one click: write the one-time session, start the game, scrub after |
| You see the yellow line but it stops (it says why) | The add-on runs; one step needs a fix (an API name or a dialog) | A small fix in `LeylineSolo.lua` |
| No yellow line, the login screen as usual, account name prefilled | The game didn't read our archive (that slot isn't loaded), or didn't take the changed file list | Try another slot (see below); else the fallback |
| "Login interface files are corrupt" (or similar) | The stock login screen checks its file list and refuses a changed one, as the later client does | **The fallback** (below); no patched program, ever |
| `.bot group` gives a party that follows and fights | Companions work at this pin | The launcher's Single player page wraps these commands |

### If the add-on is ignored or refused: the fallback

`solo-signin` already prefills the account name in Config.wtf and points `realmlist.wtf` at this PC, so the
fallback is ready the moment the add-on doesn't run.

1. The login screen shows `SOLO` in the account box.
2. Type the password once. It is in `%LOCALAPPDATA%\Leyline\solo-proof\SOLO password.txt`; open the file yourself.
3. The game then remembers the account name, and the single realm on this PC is picked by itself.

`solo-signin --no-addon` does the fallback only.

If nothing shows at all, the game may not read a lettered slot. Then try a digit slot:
`tools\solo\Solo sign-in.bat --slot patch-3.MPQ`, or ask Claude to queue it. The tool never touches an archive that
isn't ours: a slot already holding someone else's archive is refused. `solo-signin-off` removes whichever slot was used.

## Why `patch-Z.MPQ`

* The game reads its patch archives after its base archives, so a later name overrides an earlier one.
* `Z` sorts after every digit and letter, so our login screen file list is the one read.
* The later client's hybrid sign-in already uses this slot name (`launcher/src-tauri/src/hybrid.rs`).
* The game only looks at `patch-<one character>.MPQ` names, so "Leyline" can't be in the file name. Our archive is
  marked from inside instead: it holds `Interface\GlueXML\LeylineSolo.lua`, and the tool recognises it by that and by
  the checksum in `state.json`.

The archive holds only:

* the login screen's own file list, read on your PC from your own archives (the one the game reads last wins), with
  one line added, `LeylineSolo.xml`;
* our `LeylineSolo.xml` and `LeylineSolo.lua`;
* `LeylineSoloSession.lua`, the sign-in for this PC's own world, every byte escaped.

No game file is copied in, and no program is patched. `solo-stop` takes the password back out (the session goes empty).

## Not verified until the hand test

* **Everything on the PC side has only run in tests on fake folders**, plus the CI smoke on a Windows runner:
  * a fresh database from the package;
  * the login server;
  * `SOLO` signing in with a full first-era logon, and a wrong password refused.

  The world server cannot start in CI: it needs maps, and maps come only from a player's own game.
* **The add-on on the real login screen.** The calls are the stock first-era ones as commonly documented:
  * `DefaultServerLogin`;
  * `GetNumRealms`, `GetRealmInfo`, `ChangeRealm(i)`;
  * `GetNumCharacters`, `GetCharacterInfo`, `SelectCharacter`, `EnterWorld`;
  * `GlueDialog.which`;
  * the `arg1` handler.

  It is written in Lua 5.0 only and is tested against a fake login screen, not the real one. Whether the stock login
  screen accepts a changed file list is exactly what step 5 finds out.
* **Whether the game reads `patch-Z.MPQ`** (letters beyond the digit slots), from community reports only.
* **`.bot group` for a non-game-master account at this pin.** It is read from the module's source (`HandleGroup`, no
  security check); the account is level 0.
* **The clean stop over SOAP** (`server shutdown 1` as `SOLOSTOP`). If it doesn't answer, `solo-stop` ends the
  process after 90 seconds, and the last minutes of play may not be saved.
* **The extractors' Windows command lines** follow the core's own `ExtractResources.sh` (`ad -i <game> -o <data>`,
  `vmap_extractor -d <game>\Data -o <data>`).

## TODO(owner)

* The Solo experience rate (2x for now, `tools/solo/conf/solo.conf`).
* The name "Leyline Solo" on the realm list and in the add-on's line.
