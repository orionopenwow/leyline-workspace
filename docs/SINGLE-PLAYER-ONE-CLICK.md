# Single player in one click, no server

Built 6 Nov 2026 as Leyline's answer to a Reddit comment on the 3 Nov post (verbatim but for the game's and its maker's
names, left out here as everywhere players read):

> "I think it would be better if someone makes a Vanilla [game] single player client that people can run with bots with one
> click and it's all packaged together and people don't have to go through all the work to host their own server. Just
> one EXE file that just puts you into the game. Then it can have stuff like LAN, P2P, CO-OP and stuff like that. Kind of
> like how [another game of 2000] was - you have a single player character and you can also connect to a server from within the game
> and have an online character. As long as the game is self sustainable and doesn't require a server, it can change the
> perception for what [the game] is in general."


Owner: "build what you recommend and makes sense and to please this person".

TODO(owner): a reply to the commenter in his voice, if he wants one (Reddit links only, posted by him).

## What the commenter asked for, and what Leyline now does

| They asked | Leyline |
|---|---|
| Vanilla, single player, with bots | **The original world**: a new world kind on the Built-in engine (`Kind::Vanilla`, `realm/profiles/solovanilla.env`): the first era held by mod-individual-progression, level cap 60, the era's races and classes; Single player's companions and dungeon buddies (`.recruit`, `.buddy`), dungeons and raids scaled to your group |
| One click, all packaged, no hosting work | One button on the Single player page (and Home): **Play the original world (level 60) with companions**. It gets the engine (one download, shared with Single player and Boss Rush), reads the maps from the player's own game folder once, fills the database, makes the world's own login, starts it, starts the game and signs in by itself |
| "Just puts you into the game" | **Quick start**: pick a race, class, body and name; the game's own creation screen makes the character (the hybrid login's `make`) and enters the world |
| Single player character and online character, like that game of 2000 | **Characters** page: *On this PC: just you (and friends you invite)* lists each local world's characters with Play (that character, its world started if needed); *Online: a shared realm* lists the official realm and the characters this PC played there, with Play |
| LAN, P2P, co-op | **Invite a friend** from the world's card: one link per friend with their own login inside. Same network: straight in through the launcher's own forwarder. Anywhere else: the Leyline relay (the same private tunnel as Your Den). Nothing opened on the router |
| Self-sustaining, no server | Everything runs on the player's own PC, bound to 127.0.0.1. No account to make anywhere (Leyline's Hero Key is only for the online side and for friends' joins) |

What it is not (honest limits, also in the player guide):

* It plays on the player's own **Wrath game folder** (the client every Leyline ruleset uses), with the first era's
  rules on the server. A true first-era client is scoped below, not built.
* It is not one EXE file: it is Leyline (one installer) plus a one-time engine download plus the player's own game
  folder. Leyline never ships game data, so "all packaged" can only ever be "all packaged except the game".
* The one-click engine is a **Windows** build for now. Elsewhere the card points to Vanilla+ on the Host page (Docker).

## How a player uses it

1. Single player page (or Home › Original world). If no game folder is added yet, the add flow opens first.
2. **Play the original world (level 60) with companions.**
   * First time: the progress line under the card (its own `engine_progress` kind `vanilla`): the engine (skipped
     if Single player or Boss Rush got it already), the maps (skipped if Single player read them: the original world
     shares Single player's `data/`), the database, the login, the start. 10 to 30 minutes.
   * Then the game starts on 127.0.0.1:3724, signed in by itself, the realm "Original world" picked, character
     select (or character creation when there are none).
3. Optional **Quick start** (race, class, male / female, name): the same, then the new character is made and entered.
4. Once set up: Running / Stopped, Stop, **Invite a friend**, **Characters**.
5. Another world running on the same ports (Single player or Boss Rush): the card says so with **Stop it and play**.
   Nothing is stopped unasked.

## How it works

### The world kind (`launcher/src-tauri/src/host/engine.rs`)

`Kind::Vanilla` follows Boss Rush's pattern (docs/BOSS-RUSH.md): its own install `<app>/engine-vanilla/` (database,
saves, logs, back-ups, state with its own generated passwords) on the shared engine files (`<app>/engine/dist`), with:

| | Single player | Original world | Boss Rush |
|---|---|---|---|
| profile | `solo` | `solovanilla` | `bossrush` |
| login | `SOLO` | `ORIGINAL` | `RUSH` (heroes: `RUSHBF`) |
| realm list row | Single player | Original world (TODO(owner): name) | Boss Rush |
| maps | its own, all | **Single player's** (`shares_maps`) | its own, the raid maps only |
| profile layered | before modules.env | after (its lines win) | after |
| profile SQL | main_unbrawl, modern_combos | none | none |
| roaming bots | 20 | 0 (companions only) | 0 |
| progress kind | `engine` | `vanilla` | `bossrush` |

`other_running` now looks at every other kind (`KINDS`), so any two worlds refuse to start over each other.
`Kind::word` / `from_word` give the commands' names (`single`, `vanilla`, `bossrush`).

The engine zip's packaging copies `realm/profiles/*.env`, so `solovanilla.env` ships with the next engine build
(engine-windows workflow; its path filter includes `realm/profiles/**`). **Until that build, the original world can't
start** (its `required()` files include the profile): the setup's Unpack step will say the engine is missing it.

### The profile (`realm/profiles/solovanilla.env`)

Vanilla+'s first-era lines exactly (progression limit 7, later races at 8, Death Knights at 13, cap 60) and solo.env's
companions (the `OV_COMPANIONS_*` lines, 9 added bots, loot always passed, autobalance from one player), at 2x
experience (TODO(owner): his pace). No roaming population (`WU_BOTS=0`, random bots off), and
`AC_PLAYERBOTS_RANDOM_BOT_MAX_LEVEL=60` so any bots added later stay in the era. Death Knights stay locked (no
`CHARACTER_CREATING_MIN_LEVEL_FOR_HEROIC_CHARACTER` override). The bot talent specs are `realm/playerbots-specs.env`
byte for byte.

### Signing in, Quick start, picking a character (`hybrid.rs`, `glue/LeylineHybrid.lua`)

`world_play` hands the game a one-time session (the hybrid login: the world's own login and its generated password,
never one the player typed) with the realm's name. Two new optional fields:

* `pick = "<Name>"`: on character select, that character is selected (`GetCharacterInfo` / `SelectCharacter`) before
  entering; not in the list after 3 s: the last one, as before.
* `make = { name, race, class, sex }`: the create screen opens first; the race and class are chosen by their file names
  (tried by index until `GetNameForRace` / `GetSelectedClass` give them), the character is made with the stock
  `CreateCharacter`, then entered by name. A race the realm doesn't offer: nothing is made, the stock screen stays.

The pure checks (`host/localworld.rs`): the era's races and classes and which classes each race can be (`COMBOS`, no
Death Knights in Quick start), `quick_ok`, a suggested free name. `session_lua` only writes names of the game's shape.

### Characters (`host/localworld.rs`, app.rs `local_characters`, `online_characters`)

* On this PC: the running world's characters are read over its own database (`chars_sql`: the player's own account
  only, never the bots' or a friend's, not deleted ones) and kept in `<world>/state/characters.json`; a stopped world
  shows that kept list ("As they were when this world last ran"). The list is refreshed when a world is stopped from
  the launcher. Boss Rush lists its heroes (the hero account).
* Online: the official realm's characters live on the realm and there is no account API for them in the portal. The
  player's own game folder remembers each character it played per realm (`WTF/Account/<ACCOUNT>/<Realm>/<Name>/`):
  only those folder names are read.

### Co-op: Invite a friend from a world (`host/worldshare.rs`, app.rs `world_invite`)

The engine stays bound to 127.0.0.1 and its realm list says `127.0.0.1:8085`. Friends come through the launcher:

* **The relay:** the world's own relay key and invite secret (`<world>/share/`), the litenet tunnel (Your Den's: one
  outbound wss, private, signup off) pointed at the engine's ports by `<share>/den-lite.remote`. One tunnel at a time on
  a PC: Your Den's gives way (it can't run beside a world anyway: both use port 3724).
* **The same network:** a forwarder on the PC's own private address (`Lan`: port 3724 and 8085, or ports beside them
  when taken), passing private-address peers only to the engine's 127.0.0.1 ports.

Both reach the login server from this PC, so the realm list would send the friend's game to 127.0.0.1 (their own PC).
`AuthRewriter` follows the login server's messages (logon challenge, proof, reconnect, realm list, by their own
lengths) and rewrites loopback realm addresses to the way the friend came (the relay's address for this world, or the
forwarder's world port). Anything it doesn't recognise passes untouched from there on.

The friend's login (`G` + 9, password 12, the shape `litenet::guest_in` reads) is made over SOAP (`account create`,
`account set addon … 2`): a player, no GM rights. The link is the relay's join link with `~~<login>` and `~~lan=…`, so
the friend's launcher takes it exactly as a Den invite (`join_invite`: same network first, then the relay). The Ready
for friends checklist (`host_ready` kind `single` / `vanilla`) checks the world's login, the tunnel, the relay and a
test login through it. Stopping the world (or Stop sharing) closes the forwarder and the world's tunnel.

Friends' logins don't expire (Den Lite's guests do after 24 hours): a link keeps working while its world shares.
TODO: an expiry or a "remove friends' logins" button, once the owner says which.

## Commands (app.rs)

| Command | Does |
|---|---|
| `world_status {kind}` | the world's EngineStatus |
| `world_setup {client, kind}`, `world_start {kind}` | set up (once) and start / start; progress: `engine_progress` |
| `world_stop {kind}`, `world_logs`, `world_backup` | as Single player's |
| `world_choices {kind}` | Quick start's races (with their classes) and a free name |
| `world_play {client, kind, pick?, quick?, first?}` | the game, signed in by itself; `pick` a character, `quick` {name, race, class, female}, `first` the first entry (§contract) |
| `world_find_folder`, `world_preflight`, `world_resume_pending`, `world_repair`, `set_world_stop_on_exit` | one click's holes (§contract) |
| `local_characters`, `online_characters` | the Characters page |
| `world_invite {kind}`, `world_share_stop {kind}` | Invite a friend from a world |

`engine_play` (Single player's Play) now goes through `world_play`: it signs in by itself too.

## Contract: one click, the core's side (12 Nov 2026)

The holes behind "single player in one click" closed in the Rust core (`host/oneclick.rs`, `host/engine.rs`,
`hybrid.rs`, app.rs). The page (`vanilla.js` `LeylineWorlds.run`) uses these; `kind` is a world's word (`single`,
`vanilla`, `bossrush`).

| Command | Args | Returns |
|---|---|---|
| `world_find_folder` | `kind` | `{picked: id \| null, found: [{id, name, path, build, added}]}`: the game folders already added that fit the world (the Wrath folder for every local world); none: the PC's usual places are looked through (each drive's root, Games, Program Files; the player's Desktop, Downloads, Documents, Games; three folders down, at most 20,000 folders) and each fitting, complete folder is added as a game folder exactly as adding one by hand (`added: true`). Exactly one fitting: `picked` |
| `world_preflight` | `kind, client` | `{ok, free_gb, need_gb, message}`: the disk check (below). `message` is the `[disk]` error, empty when it fits |
| `world_setup` | `client, kind` | as before, plus: `client` `""` takes the folder the world's setup started with, else the only fitting one; refused with the `[disk]` message when short (the check runs only while the world isn't set up) |
| `world_resume_pending` | | the word of a world whose setup was started and didn't finish (the launcher closed midway, or a step stopped), else `null`. `world_setup {client: "", kind}` goes on from where it was |
| `world_repair` | `kind, client` | a line to show; the job is on `engine_progress` like setup (its steps below) |
| `world_play` | `client, kind, pick?, quick?, first?` | as before, plus `hero: bool`. `first: true`: no characters on this login yet: Quick start with `quick` (a random race, class and body when none is sent) and straight into the world; with characters: the last one played. Neither `pick`, `quick` nor `first`: the last character played when it's known and still there, else character select |
| `set_world_stop_on_exit` | `on` | Settings › Stop my world when the game closes (`get_state`'s `settings.world_stop_on_exit`, on by default) |

### Error codes

Every error these commands give (and every `engine_progress` `problem` of a setup or a repair) starts with a code in
brackets, then one plain sentence. The page maps the code to a button:

| Code | When | Button |
|---|---|---|
| `[folder]` | no game folder, more than one and none picked, or not of the world's version | Pick your game folder |
| `[disk]` | not enough free space (the disk check), or a folder couldn't be written | Free some space |
| `[download]` | the engine couldn't be fetched, or didn't match its checksum | Try again |
| `[maps]` | the maps couldn't be read from the game folder | Pick another game folder / Repair |
| `[db]` | the world's database didn't start, fill or answer; the back-up before moving characters failed | Repair |
| `[ports]` | another world or program holds the login, world or database port | Stop it and play |
| `[antivirus]` | a program of the engine went missing right after unpacking (or putting back), or isn't there when started | How to allow Leyline / Repair |
| `[login]` | the world's login, or the Hero Key's, couldn't be written | Repair |
| `[other]` | anything else | Show the log |

### Finding the game folder

`oneclick::scan_games`: a folder is a candidate when it has a `Data` folder and a program; it's then checked as adding
it by hand does (`clients::detect`), and kept only when it's of the world's version and complete. System and tool
folders (Windows, AppData, node_modules, dot folders…) are never looked in, and a game folder's own folders aren't
looked through for another one. Nothing is read beyond folder names and the file checks `detect` does.

### The disk check

Need = what's left to do + a 2 GB margin (`oneclick::need_bytes`): the engine download and its unpacked files (when
not installed, or for an update), the maps (when not read), the filled database (when not filled). Sizes come from
the release's `.json` (`zip_bytes`, `unpacked_bytes`, which the engine-windows workflow now writes) when the engine
is still to be downloaded; else these estimates (`oneclick::SIZES`, to confirm on a real PC): download 400 MB,
unpacked 2 GB, maps 2 GB, database 3 GB. The free space is that of the drive Leyline keeps its worlds on (Windows:
`GetDiskFreeSpaceExW`). Free space that can't be told never stops a setup. The original world shares Single
player's engine and maps, so after Single player its need is the database and the margin only.

### A setup cut short

`state.json` now has `setup_pending` (set when a setup step is planned, cleared when it finishes) and `setup_client`.
Before any setup or repair runs, `engine::heal_partial` clears what can't be trusted: a part-downloaded zip and a
part-unpacked `dist.new` go (the download is checked against its sha256 again anyway); an engine moved aside for an
update and not replaced (`dist.old` with no `dist`) comes back; a map reader or database filler of ours still running
from before is ended (their new pid files, `state/extract.pid`, `state/dbimport.pid`). Maps without their marker are
read again from nothing (the half-read `maps/`, `dbc/`, `Cameras/` are removed first). A fill that stopped midway
(`fill_started` without `filled`) starts again from empty databases, and only before the login is made (nothing of
the player's is in them). A database folder that's gone takes its `filled` and login with it (`have`), so Play sets
them up again instead of starting a broken world.

### Repair

`engine::begin_repair` (steps: Checking the engine's files, then only what's missing of Maps / Database / Fill /
Account, Starting the realm, Setting up your login again, Checking the world answers):

1. The world is stopped first (everyone saved) when it runs; the sign-in in that game folder gets another chance.
2. The engine's files: against engine.json's `files` (size and sha256 of each; written by the engine-windows
   workflow from now on). All fine: nothing is downloaded. Else (or an engine without the list, as today's): the
   release is downloaded and checked against its sha256, unpacked aside, and only the files that are missing or
   different are put back (`oneclick::mend_from`); the `.conf` files and anything else stay. A release newer than
   the installed engine is installed whole (an update).
3. The maps are read again only when their marker or folders are gone. A database that is there is never filled
   again; the saves (`db/`) and back-ups are never touched.
4. Start, the login written again (`resync_login`), the three ports checked.

### The refused auto sign-in gets another chance

`<game>/Leyline/glue-refused` is now a small JSON (`at`, the launcher's `version`, `why`). It holds for 7 days and
only for the launcher that wrote it: after a launcher update, a week, or Repair (`hybrid::clear_refused`), the next
start tries the sign-in again (a game that refuses it again writes a fresh marker). An old text marker is from an
older launcher, so it is taken away on the first look.

### Stop the world when the game closes

After a world's Play starts the game, a watcher waits for that game to close, then 60 s, then saves and stops the
world (and its sharing) when: the setting is on, no Play came since, no other game Leyline started is running, the
world is still up (and no setup or repair is going), and no friend is in it: online characters on friends' logins
(`G` + 9, `engine::FRIENDS_SQL`). A world that can't be asked stays up.

### The Hero Key's login in a local world

Signed in on Home (the social session), each Play of Single player or the original world (Boss Rush keeps its hero
login) uses a login of the Hero Key's own in that world, the same on every Play on this PC:

`HK` + the first 14 characters of base32( HMAC-SHA256( local secret, "leyline-local-world:" + the website's address
+ ":" + the account's id ) ): 16 characters, A-Z and 2-7.

* The account's id is `me.pid` from the website (asked once per Play, 8 s at most; kept in `<app data>/local-hero.json`
  so an offline Play keeps the same login; a revoked session forgets it). `engine_play` (a command that can't wait)
  uses the kept id only.
* The local secret is 32 random bytes in `<app data>/local-hero.key`, made once, never sent anywhere: another PC
  makes other names for the same account, and the name doesn't give the account away.
* The login is made or updated with a fresh password for this Play only (`engine::hero_login`: never stored, never
  logged), with the same rights as the world's own login.
* The first time any Hero Key plays a world, the world's own login's characters (and its key bindings and tutorial
  flags) move to it (`engine::move_sql`: updates only, nothing deleted), inside a back-up taken first; if the back-up
  fails, nothing moves and Play says `[db]`. `state.json` keeps `hero_moved_to` and `hero_accounts` (the Characters
  page lists the characters of the world's own login and every Hero Key login that played here).
* Signed out: the world's own login, as before.

### First entry and the last character

`state.json` keeps `last_character`: the character of a Play with a pick or Quick start, or the one that logged out
last whenever the characters are read (`engine::last_of`). `world_play` with nothing picked goes to it while it's
still one of this login's characters.

## Files

| File | What |
|---|---|
| `realm/profiles/solovanilla.env` | the original world's ruleset |
| `realm/custom/lua_scripts/ov_companions.lua` | bots that join by themselves, dungeon bots, the choice per character ("Companions settings") |
| `stream/fui-web/Interface/AddOns/WrathUnleashedUI/Companions.lua` | the welcome line with the Bots window's key |
| `launcher/src-tauri/src/host/engine.rs` | `Kind::Vanilla`, `KINDS`, `word`, shared maps, `refresh_chars` |
| `launcher/src-tauri/src/host/oneclick.rs` | one click's holes: error codes, finding the game folder, the disk check, the Hero Key's login name, stop on exit, Repair's file check, resume |
| `launcher/src-tauri/src/host/localworld.rs` | races, classes, combos, Quick start checks, characters SQL and parsing, online names |
| `launcher/src-tauri/src/host/worldshare.rs` | the realm list rewriter, the same-network forwarder, friends' logins |
| `launcher/src-tauri/src/host/litenet.rs` | `Setup.realm_list`: the rewriter on login streams |
| `launcher/src-tauri/src/host/ready.rs` | the checklist names a single player world as one |
| `launcher/src-tauri/src/hybrid.rs`, `glue/LeylineHybrid.lua` | `pick` and `make` |
| `launcher/src-tauri/src/app.rs` | the `world_*` commands, the Characters commands, `world_invite` |
| `launcher/ui/vanilla.js` | `window.LeylineWorlds` (every local world's play path) and the original world's card |
| `launcher/ui/characters.js` | the Characters page |
| `launcher/ui/hostfriends.js` | Play with friends: the single player worlds' tiles |
| `launcher/ui/index.html`, `single.css` | the card's place, the page, Home's buttons |
| `docs/leyline/single-player.md` | the player guide (site docs, `stream/docs.py`) |

## Companions settings

Built 11 Nov 2026 (agent `sp-companions`). A new player never lands alone and never needs a chat command. All of it
is `realm/custom/lua_scripts/ov_companions.lua`, section "single player", and only runs where the profile turns it on:
`solo.env` and `solovanilla.env` do; `main.env` (the official realm) and every other profile never name the keys, and
the script's defaults are **off**, so the official realm never does any of it (also: `OV_COMPANIONS=0` there turns
every bot feature off whatever else is set). In game, players always read **bots** (the owner's rule for the realm
scripts); "companions" is the launcher's word for the settings.

### What the player sees

* **First entry.** About 6 s after a character first enters the world, bots join it by themselves (default 2). One
  welcome line names them and their roles and says how to change them. With the Bots window add-on (WrathUnleashedUI,
  always there in Leyline's game folders) the add-on prints the line, with the window's key and the minimap button:
  *"Your bots are here: Anna the Priest (healer) and Bo the Warrior (tank). They follow you, fight with you and leave
  the loot to you. Change them in the Bots window: press Ctrl-B or click the Bots button by the minimap."* The window
  gets **Ctrl-B** once, only if that key is free and the window has no key yet (Esc > Key Bindings > Bots > Bots window).
  Without the add-on the realm says it in chat and points at `.companions` (a fallback only).
* **The party covers what the player is not** (their role from their talents; none yet = damage):

  | You | 1st bot | 2nd | 3rd, 4th |
  |---|---|---|---|
  | damage | healer | tank | damage |
  | tank | healer | damage | damage |
  | healer | tank | damage | damage |

  Two bots for a damage dealer = a healer that keeps you up and a tank that holds what you pull: the most forgiving
  open-world group, and a dungeon only needs two more. Classes are picked at random within the role (no Death Knight
  below its start level, and none at all in the original world, cap 60). They **keep to the player's level** as the
  player levels (re-levelled and re-geared by the bot module, their role's spec re-applied), and always pass on loot.
* **Each login.** The same party comes back (the classes and specs that were with the character at logout, at most the
  "how many" setting): *"Your bots are back: ..."*. Not after a crash: the bot module brings those back itself.
* **Sent home on purpose = remembered per character.** Dismissing them all (the Bots window's Dismiss, the buddy's
  off, or `.dismiss`) is kept: *"They won't join you by themselves now. Bring bots back any time from the Bots
  window."* Nothing joins that character by itself until the player brings bots again (any fill, buddy or dungeon
  pick); the party with them at the next logout is then remembered. Dismissing one bot just remembers the smaller party.
* **Dungeons.** Entering a 5-player dungeon (walking in, the Bots window's Dungeons tab, or the Dungeon Finder alone
  through mod-solo-lfg) fills the free slots with **dungeon bots** (1 tank, 1 healer, damage, minus what the group
  already has): *"Dungeon bots join you: ... They head home when you leave."* The Bots window tags them "dungeon bot".
  Leaving to the open world sends just those home and brings the rest of the party over. A ghost running back from
  the graveyard keeps them; they are never part of the remembered party.
* In a fight, dead, or with other bots still arriving: they wait and try again every 10 s (4 times).

### The keys (for sp-ui and sp-core)

The launcher writes these in the world's **`<world>/state/realm.env`** (`host/engine.rs` `Paths::realm_env`), which
`world_env` layers last, over the profile and `modules.env`. The world server reads its environment when it starts:
**a change applies at the next start of that world** (say so next to the settings, or restart it for the player).
The Lua reads them with `os.getenv` at each use; "1" on, "0" off.

| Setting (launcher words) | Key | Values | Default (profile) | Notes |
|---|---|---|---|---|
| Companions join me by themselves | `OV_COMPANIONS_AUTO` | `1` / `0` | `0` (a new world, owner 12 Nov) | off: no first-entry party, nothing at login; the Bots window still works |
| How many follow me | `OV_COMPANIONS_FOLLOW` | `0`-`4` | `0` (the script's own default is 2 when AUTO is on and FOLLOW unnamed) | clamped to 0..4 and to `OV_COMPANIONS_PER_PLAYER` (4); `0` = none join by themselves |
| Dungeon buddies | `OV_COMPANIONS_DUNGEON_AUTO` | `1` / `0` | `1` | fills 5-player dungeons on entry; raids never (the Bots window's Raid fill does) |
| Roaming bots in the world | `WU_BOTS` | `0`-`500` | `0` (a new world; worlds set up before 12 Nov keep Single player's old `20`, `engine.rs` `BOTS`, written into their realm.env once) | the engine turns it into `AC_PLAYERBOTS_MIN/MAX_RANDOM_BOTS` and drops the key. The original world's profile also sets `AC_PLAYERBOTS_RANDOM_BOT_AUTOLOGIN=0`: to give it roaming bots, write `AC_PLAYERBOTS_RANDOM_BOT_AUTOLOGIN=1` too (unverified that 0 keeps them all out; they stay held to 60 by `AC_PLAYERBOTS_RANDOM_BOT_MAX_LEVEL`) |

### Your world: the settings panel (12 Nov, agent `sp-settings`)

Owner, 12 Nov: "lets not start immediately with a party and a filled world, let the user customize those settings in
the single player option menu before loading their world and give them an overview of what happens". So:

* **A new world starts quiet.** `host/spsettings.rs` `settle` writes the quiet defaults into the world's realm.env
  once, before its server first reads it (`engine::begin_setup`): companions off (`AUTO=0`, `FOLLOW=0`), dungeon
  buddies on, `WU_BOTS=0`. A world set up before (its `state.json` `filled`, not `settings_settled`) gets what it
  had instead (`AUTO=1`, `FOLLOW=2`, dungeon buddies on, Single player `WU_BOTS=20`), only for keys its realm.env
  doesn't name. The profiles say the same (they reach players with the next engine release).
* **Commands.** `world_settings_get({kind})` → `{kind, companions_auto, companions_follow, dungeon_auto,
  roaming_bots, xp_rate, chosen, set_up, running}`; `world_settings_set({kind, companions_auto?, companions_follow?,
  dungeon_auto?, roaming_bots?, xp_rate?})` → `{settings, restart, message}`: writes only those keys (XP: all four
  `AC_RATE_XP_*`), keeps every other line, validates (follow 0-4, bots 0-500, XP 0.5-10), makes the state folder when
  the world isn't set up yet, and records `settings_chosen`. Single player and the original world only.
* **The panel** (oneclick.js view `world`): the first Play of a world not set up whose settings weren't chosen shows
  "Your world" before any download: what happens (the engine once, the maps read once, filling the world once, the
  local login, starting) and why the first time takes 5-20 minutes and later starts take seconds; the settings;
  Set up my world (saves, then the usual flow) or Cancel. The same settings sit in the Single player page's settings
  (`#oc-set`), with Restart when the world runs.
* `world_preflight` also gives `download_mb` (0 when the engine is here) and `download_known` (from the release's .json).

Turning bots off entirely is the existing `OV_COMPANIONS=0` (also hides the Bots window's controls: "Bots are off on
this realm"). The per-character choice is in the world's characters database, table `mod_companions_auto` (guid, mode
`new`/`on`/`off`, party `tank=warrior:prot pve,healer=priest:holy pve,buddy=mage`, at), made by the script.

Tests: `stream/tests/test_ov_companions.lua` (section "single player", 46 checks: off by default and with main.env,
first entry, roles, level keeping, login return, dismiss kept, dungeon fill and leave, ghosts, the count, the first
era, combat retry, the window's welcome, crash), `stream/tests/test_wuui_companions.lua` (the welcome line, Ctrl-B
once and only if free, the dungeon bot tag), `hosting/tests/test_solo_profile.py` (the keys in solo.env, off
everywhere else, documented here), `hosting/tests/test_solovanilla_profile.py` (the same keys as solo.env).

Not verified in a real game: the Dungeon Finder alone (mod-solo-lfg) group accepting bots added by `addclass` inside
the dungeon (the fill runs; whether the module lets them into a Dungeon Finder group is from reading); the map change
event (28) firing for the instance teleport of the Dungeon Finder; Ctrl-B being unbound in the stock key set (the
add-on checks before taking it); bots following the player out of an instance before the 2.5 s bring-over.

## Tests

* Rust (CI, `launcher check`): oneclick.rs (codes, the folder scan on stand-in folders, the disk sums, the Hero Key's
  login name, stop on exit, Repair's file list and mending, resume), engine.rs (Repair's plan, a database folder gone,
  a setup cut short healed, the characters' move SQL, the last character), hybrid.rs (the refusal's expiry),
  settings.rs (stop on exit on by default); engine.rs (the original world's own install and shared maps, the profile over
  modules.env), localworld.rs (the era's picks, names, the characters' SQL and rows, the online names), worldshare.rs
  (the realm list rewrite, the login stream message by message and a byte at a time, friends' logins, the forwarder
  end to end with a stand-in login server), hybrid.rs (`pick` / `make` handed over only in the game's shapes),
  ready.rs (an engine world's checklist).
* Lua (CI now installs Lua 5.1, so the glue tests run there): `glue/test_hybrid_glue.lua` +4 (pick, pick not found,
  make with others in the list, a race the realm doesn't offer).
* `cd launcher && node --test tests/vanilla.browser.test.cjs` (7): one click, Home, Quick start, another world running,
  not on Windows, Characters, Play with friends.
* `python3 -I hosting/tests/test_solovanilla_profile.py` (6), `python3 stream/tests/test_docs.py stream/wrath_portal.py`.

## The playtest on the owner's PC (Control: `sp-playtest`)

Built 11 Nov 2026 (owner: "make sure you do your own playtests on my pc and level up before releasing and then
release"). Nobody clicks anything:

* `Leyline play-test --fresh --game <folder> --single <single|vanilla> [--hold 20] [--level 10]
  [--checks levelup,companions,stoponexit,repair]` (`src/sptest.rs` the pure parts, app.rs `single_play_test_cli`):
  world_find_folder, the disk check, world_setup (its steps on stdout and `<data>/sp-play-test.log`), world_play with
  `first: true` (a random Quick start), then waits for `characters.online = 1` on the login it played. Checks:
  `companions` (bots of `mod_companions_owned` in the character's `group_member` group and online within 30 s, as
  many as the world's env says), `levelup` (**server-driven**: `.levelup <name> 1` over SOAP on the test's own
  127.0.0.1 world, `.saveall`, each level read back; the companions' levels must follow), `stoponexit` (the game
  killed, the world's ports closed within 150 s), `repair` (a `.txt`/`.sql`/`.dist`/`.lua` of the engine deleted,
  Repair, the file back, Play into the world again). Report: `<data>/sp-play-test.json` (no password, no key); exit 0
  only when every step passed.
* The test build: the launcher check workflow's `launcher-test-windows` job (only on `feat-sp-playtest` and
  `feat-launcher-test`, after the tests pass) puts `Leyline.exe`, `Leyline.exe.sha256` and `launcher-test.json` on the
  rolling prerelease `launcher-test` of leyline-workspace.
* `tools/first-run/sp_playtest.py` (`Single player playtest.bat`): refuses your own data folders, busy world ports
  (your own world running) and a drive under 12 GB free; downloads and checks the test build; takes the Wrath folder
  from your launcher's settings (else `tools/first-run/folder.txt`, else `--game`); a test Hero Key from the website's
  test-account API when it gives one; keeps the game folder's touched files aside and puts them back; screenshots at
  each phase; results in `logs\sp-playtest\<time>\` and `logs\sp-playtest\latest.txt`. `--both` runs the original
  world after Single player on the same engine and maps.

## Not verified (what only a Windows PC and a real game can prove)

* **Nothing has been played.** The original world needs an engine build that has `solovanilla.env` (the next
  engine-windows run) and a Windows PC.
* **mod-individual-progression with companions:** the module holds players in the first era; whether mod-playerbots'
  added companions follow it (their gear and levels) is from reading, not play. `AC_PLAYERBOTS_RANDOM_BOT_MAX_LEVEL` is
  the module's key name as the fork's other keys are written (`Playerbots.*`), not checked in its conf.
* **Quick start's screens:** `GetNameForRace` / `GetSelectedClass` returning the file names, and the index order of
  `SetSelectedRace` / `SetSelectedClass`, are from the game's own login-screen code as commonly documented; the
  loop tolerates either order, but it has not run in the game.
* **The realm list rewrite** follows the login protocol's message sizes for this client build as AzerothCore sends
  them (security flags, the realm list's build field); tested against made-up streams, not a capture.
* **The same-network forwarder** binding the PC's own address on ports the engine holds on 127.0.0.1 (allowed on
  Windows for distinct addresses; falls back to ports beside them), and the firewall prompt naming Leyline.
* **Character select order:** `pick` selects by name, so the order doesn't matter; the Characters page lists them in
  the database's order (oldest first).

## A true first-era client ("A true 1.12 client"): what it would take

Scoping only (owner: research, don't build). Today's original world is the first era's *rules* on the later client
the player already uses for every Leyline realm. A true first-era client means the player's own **1.12 game folder**
and a server core of that era.

### The core

| Core | What | Licence | Bots | Notes |
|---|---|---|---|---|
| **CMaNGOS Classic** (`cmangos/mangos-classic`) | the classic member of the CMaNGOS family; builds on Windows, Linux, macOS (its CI runs all three) | GPL-2.0-or-later, with an OpenSSL linking exception ([repo](https://github.com/CMaNGOS/mangos-classic)) | yes: the ike3/celguar playerbots, now developed at `cmangos/playerbots`, built in with `BUILD_PLAYERBOTS` ([mangosbot-bots](https://www.github.com/celguar/mangosbot-bots)) | **already in Leyline**: the Host kit's `cmangos-1.12.1` profile builds it from source at a pinned commit (`hosting/kit/profiles/cmangos-1.12.1/profile.lock`) with `-DBUILD_PLAYERBOTS=OFF`, and its `extract.sh` reads dbc, maps, vmaps and mmaps from the host's own 1.12 folder with the core's own tools |
| **VMaNGOS** (`vmangos/core`) | a "progressive" first-era core: content patch by patch, 1.2 to 1.12 ([described here](https://github.com/coolzoom/vmangos-pbotaddon)) | GPL-2.0 (to confirm against its LICENSE before any build) | its own simple **party bots** (`.partybot add <class>`, `PlayerBotMgr.cpp`), and battleground bots | closest to "as it first was"; its bots are simpler than playerbots, which suits single player companions |

A precedent that the whole idea works: the SPP Classics repack (CMaNGOS with the ike3 playerbots, an installer that
sets up 1.12, 2.4.3 and 3.3.5 single player) ([listing](https://gittrend.io/repo/celguar/spp-classics-cmangos)).
Leyline would not take its files (licences and provenance unchecked); it shows the shape players already like.

### What Leyline would build (the Built-in engine's pattern, a second engine)

1. **An engine-windows job for CMaNGOS Classic:** the pinned core with `BUILD_PLAYERBOTS=ON` and its extractors, the
   same portable MySQL, the classic-db world SQL (GPL-2.0, `InstallFullDB.sh` at a pinned commit), zipped as
   `leyline-engine-classic-win64.zip` on its own rolling release. The same refusal of any game file in the zip.
2. **`engine.rs`: a `Core` beside `Kind`:** CMaNGOS's programs (`realmd`, `mangosd`), its config keys (no `AC_*`
   environment: CMaNGOS reads `.conf` files, so the launcher writes `mangosd.conf` / `realmd.conf` from a template
   instead of passing an environment), its databases (`classicrealmd`, `classiccharacters`, `classicmangos`), its SOAP
   or remote console for the clean stop and for friends' logins, its SRP6 account rows.
3. **Data from the player's own 1.12 folder:** the core's `ad` (maps and dbc), `vmap_extractor` + `vmap_assembler`
   and `MoveMapGen` (the monster paths) from `extract.sh`, as a progress step. The kit's own estimate is 30 to 60 minutes
   plus 1 to 3 hours for the paths; Leyline could make paths optional (straight-line movement) for a fast first start.
4. **The launcher's side:** a "Vanilla 1.12.1" game folder is already a thing (`clients.rs`, the era-ordered names);
   the world kind would require that build instead of the Wrath one; the hybrid login glue would need a 1.12 version
   (its login screen functions differ), or the first version signs in by Config.wtf prefill only.
5. **Bots:** playerbots' own commands (`.bot add`, the `playerbot` config), or VMaNGOS's party bots; the Leyline
   companions script (`ov_companions.lua`) is AzerothCore Lua (mod-ale) and would not carry over: CMaNGOS has no Lua
   engine, so the companion commands would be the bot module's own, wrapped by the launcher's page.
6. **Co-op:** the same `worldshare` path; the realm list rewriter needs the 1.12 login protocol's message sizes (the
   proof answer is shorter, the realm list has no lock byte), a small table per build.

### Effort, roughly

| Piece | Days (one person) |
|---|---|
| Windows build job for CMaNGOS Classic with playerbots, portable MySQL, the world SQL, the zip | 3-5 (first green build is the long pole) |
| `engine.rs` core abstraction (CMaNGOS programs, conf files, databases, accounts, console) | 4-6 |
| Extraction step with progress, optional paths | 2-3 |
| Launcher pages, a 1.12 folder requirement, Characters for that core | 2-3 |
| A 1.12 hybrid login glue (or prefill only for a first version) | 2-4 |
| Co-op on that core (rewriter table, friends' logins) | 1-2 |
| Play-testing on Windows with a real 1.12 folder | 3-5 |
| **Total** | **about 3 to 5 weeks**, most of the risk in the Windows build and in the first real play |

Licences: everything above is GPL-2.0(-or-later) source built by us and shipped as our own build (with the licences
and the source links in the zip, as the Built-in engine does today: `LICENSES/`). No game data, ever: the player's own
1.12 folder is read on their own PC. Never used: decompiled clients, wowdecomp, Waystone, any leaked source.

Recommendation: ship the original world on the Wrath client now (built here), and take the CMaNGOS Classic engine as
its own project once the Built-in engine has been proven on real Windows PCs, starting from the Host kit's
`cmangos-1.12.1` profile (the source pins and the extraction are already done there).
