# Leyline Solo: the launcher core (Phases 1, 2 and 4)

Leyline Solo is the standalone single player for the **first-era game** (client build 5875) on the player's own game
folder: the CMaNGOS first-era server with playerbots on this PC (no Docker), companions, offline, signing in by
itself, and co-op by invite. Phase 0 (`docs/LEYLINE-SOLO-PHASE0.md`, `tools/solo/solo_proof.py`) proved the parts by
hand; this is the launcher's own version of them, in Rust.

* `launcher/src-tauri/src/host/classic.rs`: the engine kit (Phase 1) and the characters (Phase 3 of the brief).
* `launcher/src-tauri/src/host/classicsign.rs`: the fast sign-in (Phase 2): realmlist.wtf, Config.wtf, the opt-in
  sign-in add-on, byte-exact restore.
* `launcher/src-tauri/src/host/worldshare.rs`: the realm list rewrite now knows the first era's layout (Phase 4).
* `launcher/src-tauri/glue/LeylineSolo.{lua,xml}`: the first-era login screen add-on (Lua 5.0). `tools/solo/glue/`
  holds the same two files for Phase 0's tool (a test keeps them identical).

This page is the **contract the app agent codes against** (`agent/solo-app`). Commands are Tauri commands, called
from the UI with `invoke("<name>", { args })`. Argument names are camelCase on the JS side as Tauri maps them
(`client`, `pick`, `quick`, `mmaps`, `update`, `mode`, `lines`). Every command returns `Result<T, String>`: an error
is one plain sentence for the player.

## Why its own module and not another `engine::Kind`

`engine::Kind` is shaped around the AzerothCore build: `AC_*` environment settings, `worldserver`/`authserver`/
`dbimport`, `acore_*` databases, `engine.json` layout 1, one shared `dist/`. The first-era engine is a different core
(CMaNGOS: `mangosd`/`realmd`, `.conf` files with `Mangosd_`/`Realmd_` environment overrides, the `classic*`
databases, one SQL dump instead of `dbimport`, three extractors). A separate module shares the small pure helpers
(`engine::verifier` for SRP6, `engine::secret`, `engine::Job`, `engine::port_answers`, `engine::stamp`,
`clientimport::unzip`, `upstream::download_progress`) and keeps both readable. It is still "one world at a time":
the login and world ports (3724, 8085) are the same as the other worlds' and Den Lite's.

## Commands

| Command | Args | Returns | Notes |
|---|---|---|---|
| `classic_status` | none | `ClassicStatus` | Cheap; poll it on the page |
| `classic_setup` | `client: String` (a game folder id of the first era), `mmaps?: bool`, `update?: bool` | `String` (what started) | Background job: whatever isn't done yet, then start. Follow `classic_progress` |
| `classic_start` | none | `String` | The same job, set up already (no game folder needed) |
| `classic_progress` | none | `Job` (`kind: "classic"`) | Same shape as `engine_progress`: `idle, kind, running, code, step, n, of, secs, problem, pct, detail` |
| `classic_stop` | none | `String` | Friends' logins removed first, the world saved over SOAP, then down; the sign-in session scrubbed |
| `classic_logs` | `lines?: usize` (20 to 2000, default 200) | `String` | Last lines of each log, labelled, passwords replaced by `[hidden]` |
| `classic_update_check` | none | `ClassicStatus` | Looks at the release's `.sha256`; `update: true` when newer |
| `classic_signin_mode` | none | `SigninMode` | `{ mode: "prefill" \| "addon", addon_approved: false }` |
| `classic_set_signin_mode` | `mode: "prefill" \| "addon"` | `String` | Saved as `solo_signin` in settings.json. Switching to prefill takes our archive out |
| `classic_play` | `client: String`, `pick?: String` (a character's name), `quick?: { name?: String, race: u8, class: u8, female?: bool }` | `ClassicPlay` | Writes the sign-in (backups first), then starts the game program from its folder |
| `classic_password` | none | `String` | SOLO's password, **for the UI only**: shown once on request and copied by the UI. Never logged, never in another answer |
| `classic_signin_off` | none | `String` | Every file put back as it was (byte for byte), our archive removed. Close the game first |
| `classic_characters` | none | `ClassicChars` | Read now while running (and kept), else the kept list |
| `classic_choices` | none | `ClassicChoices` | Quick start: the first era's races, their classes, a free name |
| `classic_invite` | none | `ClassicInvite` | Starts sharing: one friend's login (7 days), LAN forwarder, relay tunnel; the link |
| `classic_friends` | none | `Vec<Friend>` | The friends' logins now (`user`, `expires` unix seconds); never their passwords |
| `classic_share_stop` | none | `String` | Sharing off and every friend's login removed |
| `host_ready` | `kind: "classic"` | `HostReady` | The existing checklist, with the first-era world as a kind |

There are **no push events**: the page polls `classic_progress` while `running` (as it does `engine_progress`) and
`classic_status` otherwise.

### Shapes

```text
ClassicStatus {
  supported: bool        // Windows x64 (the only engine built)
  installed, can_get: bool
  version: String        // solo-engine.json's version
  update: bool
  maps, vmaps, mmaps: bool
  db: bool               // made and imported
  accounts: bool         // SOLO and the stop account made
  set_up: bool           // all of the above (mmaps optional)
  running, db_up, login_up, world_up: bool
  busy: bool             // a setup/start job is going
  account: String        // "SOLO" once made
  realm_name: String     // "Leyline Solo" (TODO(owner))
  root: String           // <app>/engine-classic
  game: String           // the folder the maps were read from
  other_running: Option<String>   // the title of another world on these ports ("Single player", "Your Den"...)
  signin: SigninStatus
  sharing: bool          // the LAN forwarder or the tunnel is this world's
  friends: usize         // friends' logins alive now
}
SigninStatus { on: bool, mode: String, game: String, addon: bool, why: String }
   // addon: our archive is in the folder now; why: why the add-on isn't used (empty when it is or wasn't asked)
SigninMode { mode: String, addon_approved: bool }
ClassicPlay {
  message: String
  mode: String           // the mode used for this start
  auto: bool             // the add-on signs in by itself this time (addon mode and it could be written)
  account: String        // "SOLO" (prefilled on the login screen)
  password_needed: bool  // prefill: the UI offers "Show password" (classic_password)
  made: String           // Quick start's character's name (addon mode only; empty otherwise)
}
ClassicChar {           // LocalChar (host/localworld.rs) plus:
  name, race, class, female, level, played, last, race_label, class_label, side
  companion: bool        // made or brought in by `.bot` (see "Companions" below)
  online: bool
}
ClassicChars { running: bool, live: bool, at: u64, chars: Vec<ClassicChar> }
ClassicChoices { races: Vec<RaceChoice>, first_era: true, name: String }
ClassicInvite { link, code, name, expires: u64, tunnel: TunnelState, ways: Vec<InviteWay> }   // as DenInvite
Friend { user: String, expires: u64 }
```

## Phase 1: the engine kit

Everything is under `<app>/engine-classic/`: `dist/` (the unpacked zip), `data/` (maps from the player's folder),
`db/` (MySQL's data), `run/` (the server `.conf` files), `logs/`, `state/` (`state.json`, `characters.json`, pid
files), `tmp/`, `share/` (the invite's relay key and secret), `signin-backup/`.

| Step (`Job.step`) | What it does |
|---|---|
| Getting the engine | `leyline-solo-engine-win64.zip.sha256`, then the zip, from the rolling release `solo-engine-latest` of `orionopenwow/leyline-workspace`. `LEYLINE_SOLO_ENGINE_URL` (an https folder ending in `/`) overrides it for testing a build |
| Checking and unpacking it | sha256 checked, unpacked into `dist.new`, refused if it holds any game file or lacks a program, then swapped in |
| Reading the maps | `tools/ad -i <game> -o data` (dbc and maps) from the selected first-era folder, read only |
| Reading the line of sight | `vmap_extractor -d <game>/Data -o data`, then `vmap_assembler data/Buildings data/vmaps` |
| Building the monster paths | `MoveMapGen` (only with `mmaps: true`; 1 to 3 hours) |
| Setting up the database | `mysqld --initialize-insecure` with a one-time init file that sets the generated root password |
| Filling the world | `sql/solo-databases.sql` imported (5 to 15 minutes), the servers' own login `solo`, the realm list row |
| Making your accounts | `SOLO` (to play, level 0) and `SOLOSTOP` (level 3, only for the clean stop over SOAP on 127.0.0.1), SRP6 verifiers as the core makes them |
| Starting the world | MySQL (13336), realmd (3724), mangosd (8085, SOAP 7878), all on 127.0.0.1; up when mangosd says "World initialized" and the ports answer |

* **The Solo profile** is `dist/conf/solo*.conf` layered over the core's `.conf.dist`, then the launcher's own lines
  (paths, ports, 127.0.0.1 only, SOAP on). Level cap 60, companions on, random bots off, the experience rate
  `TODO(owner)` (2x in `tools/solo/conf/solo.conf`).
* **Passwords** (root, the servers' `solo` login, SOLO, SOLOSTOP) are generated once and live **only in
  `state/state.json`**. The server `.conf` files hold a placeholder: the real database logins reach mangosd and
  realmd as `Mangosd_*DatabaseInfo` / `Realmd_LoginDatabaseInfo` environment values of the child process (the
  core's `Config::Reload` takes an environment value over the file's, `src/shared/Config/Config.cpp` at the pin).
  The mysql client gets its login from a short-lived `--defaults-extra-file` in `state/`, removed after each call.
  Every log line shown is scrubbed of all four.
* **One world at a time:** setup and start refuse while another world answers these ports (the AzerothCore worlds,
  Den Lite, or another server), naming it.
* **Clean stop:** `server shutdown 1` over SOAP as SOLOSTOP, up to 120 s, then the process is ended; then realmd,
  then `mysqladmin shutdown`.

## Phase 2: the fast sign-in

`classic_play` writes, backing each file up first (once, into `signin-backup/`):

1. `realmlist.wtf` (the folder's root) = `set realmlist 127.0.0.1`;
2. `WTF/Config.wtf`: `SET accountName "SOLO"` and `SET realmName "Leyline Solo"` (and `realmList` when the file has
   that line);
3. **addon mode only:** `Data/patch-Z.MPQ` (ours, never a slot holding someone else's) with the player's own
   `Interface\GlueXML\GlueXML.toc` read from their archives (the last one read wins) plus one line
   `LeylineSolo.xml`, our `LeylineSolo.xml`/`.lua`, and `LeylineSoloSession.lua` (every byte `\ddd`-escaped).

then starts the game program from the folder (`launch::plan`, Wine elsewhere).

* **`prefill`** (the default for now): steps 1 and 2. The login screen shows SOLO; the player types the password once
  (the UI shows it on request with `classic_password` and copies it). No archive.
* **`addon`** (opt-in): steps 1 to 3. `TODO(owner)`: copying the player's own login-screen file list into our archive
  isn't approved yet, so it stays opt-in and `addon_approved` is false. If the list can't be read (no archive has it,
  or it is packed in a way not read here), the start falls back to prefill and `SigninStatus.why` says so.
* The session carries `user`, `pass`, `realm`, and optionally `pick` (a character's name: enter that one) or `make`
  (`{ name, race, class, sex }`: make it on the create screen, then enter it), as the later-era hybrid login does.
  The password leaves the archive as soon as the game lets go of it (it is rewritten with an empty session), and on
  `classic_stop`.
* **`classic_signin_off`** puts every file back **byte for byte** with its old modified time (or removes it when it
  wasn't there), and removes our archive. If the game rewrote `Config.wtf` since (it does on close), the player's
  own new settings are kept and only our three lines go back to what they were; the answer says which.

## Characters and companions

`classic_characters` reads, while the world runs, SOLO's characters only:

```sql
SELECT c.guid, c.name, c.race, c.class, c.gender, c.level, c.totaltime, c.logout_time, c.online,
       (SELECT COUNT(*) FROM ai_playerbot_db_store d WHERE d.guid = c.guid),
       COALESCE((SELECT g.leaderGuid FROM group_member m JOIN `groups` g ON g.groupId = m.groupId
                 WHERE m.memberGuid = c.guid LIMIT 1), 0)
FROM classiccharacters.characters c JOIN classicrealmd.account a ON a.id = c.account
WHERE a.username = 'SOLO' ORDER BY c.guid
```

**How companions are told apart.** The playerbots module at the pin (`6823423`) makes a companion as a normal
character on the master's own account (`PlayerbotHolder::GetOrCreateAccount` returns the master's account id), so
the account doesn't tell. Two marks do:

1. `ai_playerbot_db_store` has rows for its guid: the module saves a bot's strategies there when it logs out while
   grouped with its master (`PlayerbotMgr.cpp`, `sPlayerbotDbStore.Save`). A character the player only ever played
   has none.
2. Live: it is online and in a group whose leader is **another** online SOLO character (one account has one player
   session, so every other online character on it is a bot).

The one online SOLO character that leads (or isn't in a group with another online SOLO character) is the player's,
whatever the store says. Friends' accounts and SOLOSTOP are never listed. The list is kept in
`state/characters.json` for when the world is stopped.

Quick start choices are `localworld::choices(true)`: the eight first-era races and their classes, no later races, no
Death Knights.

## Phase 4: invite a friend

The engine stays on 127.0.0.1. `classic_invite`:

* makes one friend's login over SOAP (`account create G######### <12 letters>`; SOAP runs at the console's level in
  CMaNGOS, `MaNGOSsoap.cpp`), **valid 7 days**, kept in `state.json` (`friends`: user and expiry, the password only
  in the link). Expired ones are deleted over SOAP (`account delete`) whenever the status is read with the world up,
  at start, and **all of them on stop** and on `classic_share_stop`;
* starts the same-network forwarder (`worldshare::lan_start_layout`, first era) and the relay tunnel
  (`litenet`, with `share/den-lite.layout` = `classic` so its rewriter uses the first era's realm list);
* returns the link with the friend's login after `~~`, as the later-era worlds' invites do.

**The realm list rewrite for build 5875** (`worldshare::Layout::Classic`), read from the realmd source at the pin
(`src/realmd/AuthSocket.cpp`, `LoadRealmlist` case 5875): `u8 0x10, u16 size`, then `u32 unused`, `u8 count`, per
realm `u32 type, u8 flags, cstring name, cstring "address:port", f32 population, u8 characters, u8 category, u8 unk`,
then `u16 0x0002`. The other login messages' lengths at this build: a good proof is 26 bytes (`cmd, error, M2[20],
u32 flags`) and a refused one 2; a reconnect proof 4 (2 when refused).

## Tested vs. needs a real PC

Unit-tested on CI ("launcher check"): the paths and state, the step plan, the conf layering and that no `.conf` holds
a password, SRP6 against the core's own sample rows, the SOAP request, the realm-list rewrite on crafted first-era
streams (whole and a byte at a time), the friends' expiry, Config.wtf and realmlist edits with byte-exact restore,
the session's Lua escaping, the toc line, the add-on against a fake first-era login screen (Lua 5.1 on CI), the
characters SQL and the companion rule, state never in a log line.

Only a real PC proves: the engine programs starting from the launcher, the extractors on a real folder, the add-on
on the real login screen (Phase 0's open question), the companions marks at run time, a friend joining over LAN and
over the relay.
