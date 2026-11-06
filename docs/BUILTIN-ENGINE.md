# Single player, the Built-in engine (no Docker)

Owner's plan, 4 Nov 2026, Phase 2: Single player runs on the player's own Windows PC with **nothing else to
install**. Docker stays as the fallback engine (Phase 1, the Host pack). This page is the research, the design,
and what is built; the last section says what only a Windows PC or CI can prove.

Built in this change:

| Part | Where |
|---|---|
| The engine's Windows build, packaged as one zip on the rolling release `engine-latest` | `.github/workflows/engine-windows.yml` (source copy `tools/github-workflows/engine-windows.yml`) |
| The launcher's side: get, check, unpack, read the maps, first database set-up, the account, start, stop, health, logs, back up, update | `launcher/src-tauri/src/host/engine.rs`, the `engine_*` commands in `app.rs` |
| The Single player page: Built-in recommended (Docker the fallback), its progress lines, Manage with either engine | `launcher/ui/single.js`, `launcher/tests/single.browser.test.cjs` |

## 1. What the Docker realm is today (what the Built-in engine must match)

* **Core:** the playerbot fork of AzerothCore, `mod-playerbots/azerothcore-wotlk`, branch `Playerbot`, pinned in
  `realm/custom/sources.lock` (commit `3f2969dd…`). mod-playerbots' own README is plain about it: *"All
  `mod-playerbots` installations require a custom fork of AzerothCore: mod-playerbots/azerothcore-wotlk (Playerbot
  branch). The standard AzerothCore repository will not work."* It lists Ubuntu, Windows and macOS as supported.
* **Modules** (sources.lock, each at a commit): mod-playerbots, mod-ale (the Lua engine, formerly Eluna),
  mod-autobalance, mod-solo-lfg, mod-transmog, mod-aoe-loot, mod-ah-bot-plus, mod-challenge-modes (+ our patch),
  mod-dungeon-master, mod-guildhouse, mod-individual-progression, mod-1v1-arena, mod-mythic-plus,
  mod-multibot-bridge, mod-questie-bridge, mod-arac (its client files are always removed), and our own
  `mod-sandbox-core` (repo "local").
* **Recipe:** `realm/custom/fetch.sh` fetches each at its commit and applies `realm/custom/patches/*.patch`; the core's
  own `apps/docker/Dockerfile` builds it (`.github/workflows/wu-images.yml`: `CTYPE=Release`, `CWITH_WARNINGS=OFF`,
  scripts and modules static).
* **Runtime** (`hosting/kit/docker-compose.host.yml`): MySQL 8.4 (`mysql:8.4`), a db-import run, authserver,
  worldserver. The realm's settings are **environment variables**: `realm-files/profiles/<profile>.env`, then
  `realm-files/modules.env`, then the host's own `state/realm.env`, then the compose file's own block (database
  logins, data dir, bot count `WU_BOTS`, pathing off, Warden off, SOAP off, player cap). Our Lua scripts
  (`realm/custom/lua_scripts`) are mounted and read their `OV_*` switches from the same environment.
* **Game data:** `map_extractor -i <your game> -o <volume>` from the player's own folder (dbc, maps, Cameras).
  vmaps and mmaps ("pathing") are off by default (`OV_PATHING=0`).
* **Profile SQL** (host-wrath-realm.sh `profile_sql`): `solo` applies `main_unbrawl.sql` and `modern_combos.sql` to
  the world database once.

The key finding for the native build: **AzerothCore reads `AC_*` environment variables as overrides of any config
key on every platform** (src/common/Configuration/Config.cpp: `GetValueDefault` asks the environment first, even for
keys not in the file; `IniKeyToEnvVarKey` turns `vmap.enableLOS` into `VMAP_ENABLE_LOS`). So the native worldserver
takes exactly the same `solo.env` + `modules.env` lines the container does. The profile files stay the single
source of truth; nothing is translated by hand. `engine.rs` `ini_to_env` is a port of the core's function, tested
against the names the compose file uses.

## 2. Research

### Building on Windows
The pinned fork carries its own Windows CI (`.github/workflows/windows_build.yml` at our commit), which we follow:
`windows-latest` with MSVC (VS 2022) and CMake from the runner image, **Boost 1.87.0** prebuilt binaries
(`boost_1_87_0-msvc-14.3-64.exe`, BOOST_ROOT `C:\local\boost_1_87_0`), the **MySQL 8.4.9** Windows zip for the client
headers and `libmysql` (`cdn.mysql.com/archives/mysql-8.4/mysql-8.4.9-winx64.zip`, `C:\tools\mysql\current`), and the
runner image's OpenSSL. `apps/compiler` passes `-DMYSQL_INCLUDE_DIR` / `-DMYSQL_LIBRARY` on Windows and installs with
`cmake --install`. On Windows the core looks for its configs in `configs/` beside the program, or the `-c` file.

### mod-ale (Lua) on Windows
mod-ale builds its Lua from source at configure time (`LUA_VERSION` = `lua52` by default, also `luajit`, `lua53`,
`lua54`; `LUA_STATIC` on by default) with an MSVC path for LuaJIT (`msvcbuild.bat`). The Docker image uses the
default, so the Windows build does too: **Lua 5.2, static**. Our scripts are written for it.

### mod-playerbots on Windows
Windows is a supported platform (README, its own windows_build badge). The bots' database `acore_playerbots` is
made and updated **by the worldserver itself** (mod-playerbots `PlayerbotsDatabaseScript`: `ModuleDBUpdater::Create`
when it is missing, then its own updates from `modules/mod-playerbots/data/sql/playerbots`), which needs the
`MySQLExecutable` and `SourceDirectory` settings to point at a mysql client and the SQL tree. Both ship in the zip.

### The databases (first run)
The core's updater (src/server/database/Updater) imports `SourceDirectory/data/sql/base/db_{auth,world,characters}`,
then updates, then each module's `modules/<name>/data/sql/db-*`, by running the **mysql command-line client**
(`MySQLExecutable`). With `Updates.AutoSetup = 1` it creates missing databases. The Docker realm runs `dbimport`
once (the db-import image: `data/` + `modules/` + `dbimport`); the native engine does the same with `dbimport.exe`, so
the first run has its own step and its own log. Later updates are applied by the worldserver on start, as today.

### A database server with no installer
| | MySQL Community 8.4 LTS (chosen) | MariaDB 11.4 LTS |
|---|---|---|
| Windows "no-install" zip | yes, `mysql-8.4.x-winx64.zip`; `mysqld --initialize-insecure` then run | yes, `mariadb-11.4.x-winx64.zip`; `mariadb-install-db` |
| Same as the Docker realm | **yes** (`mysql:8.4`), same SQL dialect, same `libmysql` we link | close, but not what we test against |
| Licence | GPLv2 (Oracle's Community edition) | GPLv2 (server), LGPL client |
| Size | zip ~250–300 MB; trimmed to mysqld, mysql, mysqladmin, mysqldump, their DLLs and `share/`: ~120–170 MB unpacked (to confirm in CI) | zip ~80–100 MB |

MySQL 8.4 wins on "the same as Docker and the same version the fork's own Windows CI links" (one version to test).
Its size is the cost; MariaDB is the fallback if size matters more later (one constant in the workflow, plus the
`--initialize` step).

### Licences of what the zip holds
* AzerothCore: GPL-2.0-or-later (headers: "either version 2 of the License, or (at your option) any later version").
* mod-playerbots: GPL-2.0. mod-ale: **GPL-3.0** (so the combined worldserver is distributed under GPL-3.0, which
  AzerothCore's "or later" allows). Other modules: their own (mostly AGPL/GPL/MIT); each module's licence file is
  copied into `LICENSES/` by the workflow.
* MySQL Community Server: GPL-2.0. OpenSSL 3: Apache-2.0. Boost: BSL-1.0 (linked statically). Lua 5.2: MIT.
* Microsoft VC++ runtime DLLs: app-local copies from the VS redist folder, as Microsoft's redistribution terms allow.

**GPL duty:** binaries must come with the corresponding source or a written offer. The zip carries `SOURCES.txt`:
every repository and exact commit (the sources.lock lines), our patches, the MySQL and Boost versions and download
addresses, and the workflow that builds it. The same sources are public at those commits, and our own module and
scripts are in the public Leyline repo. (The Docker images already distribute the same binaries; nothing new in
kind.) TODO(owner): confirm he is happy with the release notes' licence line.

### No game data, ever
The zip holds programs, SQL and our configs only. The workflow removes mod-arac's client files and **fails** if any
game-data file type (`.dbc .db2 .mpq .m2 .wmo .adt .blp .vmtree .mmap .map`) is in the source tree or the package,
exactly as wu-images does. The launcher refuses a zip with such a file (`engine.rs` `is_game_data`). The maps are
read on the player's PC from their own game folder by the bundled `map_extractor.exe`, into the app data folder,
and never leave it.

## 3. What runs where

```
<app data>\engine\
  dist\                    the unpacked release (replaced on update; nothing of the player's in it)
    engine.json            name, version, core + module commits, MySQL / Boost / Lua versions, layout 1
    bin\                   authserver.exe, worldserver.exe, dbimport.exe, their DLLs, configs\*.conf(.dist)
    tools\                 map_extractor.exe, vmap4_extractor.exe, vmap4_assembler.exe, mmaps_generator.exe
    mysql\bin\             mysqld.exe, mysql.exe, mysqladmin.exe, mysqldump.exe (+ share\)
    source\data\sql\       the core's SQL; source\modules\<name>\data\sql\ each module's
    lua_scripts\           realm/custom/lua_scripts
    realm\                 profiles\*.env, modules.env, sql\ (the profile SQL)
    LICENSES\, SOURCES.txt
  data\                    dbc\, maps\, Cameras\ from the player's own game folder (kept across updates)
  db\                      the MySQL data folder: the saves
  logs\                    mysqld.err, auth.log, world.log, dbimport.log, extract.log, setup.log
  state\                   state.json (generated passwords, what is done), my.ini, client.cnf, realm.env (optional)
  backups\                 single-player-<date>.zip (SQL dumps of accounts, characters, bots; the newest 10)
```

Everything listens on **127.0.0.1 only**: MySQL on 13306 (not 3306, so an installed MySQL is left alone; mysqlx off;
`skip-name-resolve`; the root login only for `127.0.0.1` and `localhost`), the login server on 3724 (the client's
default), the world on 8085, SOAP on 127.0.0.1:17878 (only for a clean stop). Children of the launcher with no console
window. The client's realmlist is `127.0.0.1:3724`, deliberately not Den Lite's bare `127.0.0.1`, so the play bar
and `den_ground_root` never mistake one for the other. Den Lite also uses port 3724: one at a time, and the engine
says so instead of failing.

### The settings (solo profile to the native config)
`engine.rs` `world_env`: `solo.env`, then `modules.env`, then `state\realm.env` (if the player has one), then the
compose file's own block, translated: the four database logins (`127.0.0.1;13306;root;<generated>;acore_*`),
`DataDir`, `LogsDir`, `TempDir`, `SourceDirectory`, `MySQLExecutable`, the ALE script path, the kit's solo defaults
(20 roaming bots, `PlayerLimit` 5, pathing off, Warden off), `BindIP` 127.0.0.1, the console off (no window),
SOAP on 127.0.0.1 only. The worldserver is started with `-c configs\worldserver.conf` (the shipped `.dist` copied
once) and that environment. Unit tests check the layering, the translation and that nothing listens elsewhere.

### First run (the state machine, `engine.rs` `plan`)
| Step | Shown as | What |
|---|---|---|
| Download | Getting the engine (a one-time download) | `leyline-engine-win64.zip` with its byte progress; `.sha256` first |
| Unpack | Checking and unpacking it | sha256 must match, then unpack (no paths out of the folder, no game data), layout check, the `.conf` files |
| Maps | Reading the maps from your game folder | `map_extractor.exe -i <game> -o data`, a % from the files written; skipped when done for the same folder |
| Database | Setting up the database | `mysqld --initialize-insecure --init-file` (sets the generated root password for 127.0.0.1 and localhost), my.ini |
| Fill | Filling the world (the first time takes a few minutes) | mysqld started, `dbimport.exe`, then the solo profile's SQL |
| Account | Making your account | the player's login `SOLO` with a generated password (SRP6 salt + verifier as `realm/make_account.py`, GM level 3, like the host kit's GM login) and the realm list row (`Single player`, 127.0.0.1:8085) |
| Start | Starting the realm | mysqld, authserver, worldserver; healthy when 13306, 3724 and 8085 answer (the first world start can take minutes) |

Every step is skipped once done (state.json and the files say so), so Start again resumes where a failure stopped.
Later starts are only "Start". Stop: `server shutdown 1` over SOAP (everyone saved), the auth server stopped,
`mysqladmin shutdown`; a process that doesn't go in time is ended.

### The player's login
Made automatically; the player never chooses or types a password at set-up. The account name is written into the
game's Config.wtf (as the Den does), so the login screen has it filled in. The generated password is kept in
`state\state.json` and shown on the Manage card with Copy. **Follow-up:** typing it in for them at the login screen
(the Den's Windows key driver, `denclick.rs`, or our login screen code) so Play goes straight in.

### Update path
`engine_update_check` reads the release's `.sha256`; a different one than installed means an update. Update = stop,
download, check, unpack into `dist.new`, swap with `dist` (the old one removed only after the swap), start. `data\`,
`db\`, `state\` and `backups\` are never touched. New SQL in the new core or modules is applied by the worldserver's
own updater on its next start (`Updates.EnableDatabases = 7`, the playerbots updater for its database), exactly as a
Docker image update does. A back-up is made before an update.

### Sizes (estimates, to confirm on the first CI run; `engine.json` and the release notes will carry the real ones)
* zip: ~150–250 MB (worldserver with playerbots ~60–110 MB, MySQL trimmed ~120–170 MB, SQL ~100+ MB before deflate).
* unpacked: ~500–700 MB. Maps from the player's game (dbc + maps): ~1–1.5 GB. Database after the first fill: ~1–1.5 GB.
* So about **3–4 GB of disk** in all, 2–3 GB RAM when running with 20 bots (MySQL 256 MB buffer pool).

## 4. Risks
* **First CI run unknowns:** the exact DLL set (the smoke step runs each program and fails on a missing DLL), the
  MySQL trim (the smoke starts the trimmed mysqld and imports every database), build time (MSVC on a 4-core runner:
  2–4 hours; the job has 350 minutes) and disk (the build goes to `C:\` where the runner has the most room).
* **OpenSSL** comes from the runner image (as the fork's CI does); its version moves with the image. Pinned
  alternative: a fixed OpenSSL build in the workflow (follow-up if it ever breaks).
* **The release lives on the workspace repo** like Den Lite's `den-lite-latest`. If that repo is private, players
  can't download it: the same open point as Den Lite (`host/lite.rs RELEASE`). `LEYLINE_ENGINE_URL` overrides the
  address for testing.
* **Not code-signed:** Windows SmartScreen does not run for child processes started by the launcher, but antivirus
  may look at new programs. Same as Den Lite today.
* **Ports:** 3724 / 8085 in use (Den Lite, another server): the engine says which and stops.
* **Lua scripts with container paths:** `ov_den.lua` uses `/azerothcore/dens`; the solo profile has the Den off
  (`OV_DEN=0`), so it isn't reached. Any script that assumes Linux paths needs a look before other profiles run here.
* **GM level 3** for the player's own account (needed for SOAP's clean stop, and the host kit gives hosts a GM login
  too). On their own PC that's by design.

## 5. Follow-ups
* **Open to friends:** moving this world to the Docker host kit (the relay, invites). The databases are plain MySQL:
  `engine_backup`'s dump restores into the kit's `wu-host-database` (`mysql acore_characters < dump`), plus the
  same `realm.env`. Not built: one button "Move to the Host pack" that makes a back-up, sets the kit up with `solo`,
  imports the dump into the kit's database before its first start. Documented, not done.
* Auto-login at the login screen (see "The player's login").
* Pathing (vmaps/mmaps) as an option: the extractors are in the zip; `mmaps_generator` takes hours.
* A Linux build (the same workflow on ubuntu, a tar.gz).

## 6. What only Windows or CI can prove
* The workflow's first run: the compile, the DLL set, the MySQL trim, the smoke (mysqld up, dbimport fills every
  database, the auth server answers on 3724), the release upload.
* The native start on a real PC: `map_extractor.exe` on a real game folder, the first worldserver start (bots, the
  playerbots database), the client logging in at 127.0.0.1:3724, SOAP's clean stop, the back-up's mysqldump.
* Cargo here can't build (no crates.io): `engine.rs` was checked with rustfmt as a parse check; its unit tests run in
  the launcher's own CI (launcher-check) and the owner's `cargo test`.
