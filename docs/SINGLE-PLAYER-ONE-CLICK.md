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
| `world_play {client, kind, pick?, quick?}` | the game, signed in by itself; `pick` a character, `quick` {name, race, class, female} |
| `local_characters`, `online_characters` | the Characters page |
| `world_invite {kind}`, `world_share_stop {kind}` | Invite a friend from a world |

`engine_play` (Single player's Play) now goes through `world_play`: it signs in by itself too.

## Files

| File | What |
|---|---|
| `realm/profiles/solovanilla.env` | the original world's ruleset |
| `launcher/src-tauri/src/host/engine.rs` | `Kind::Vanilla`, `KINDS`, `word`, shared maps, `refresh_chars` |
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

## Tests

* Rust (CI, `launcher check`): engine.rs (the original world's own install and shared maps, the profile over
  modules.env), localworld.rs (the era's picks, names, the characters' SQL and rows, the online names), worldshare.rs
  (the realm list rewrite, the login stream message by message and a byte at a time, friends' logins, the forwarder
  end to end with a stand-in login server), hybrid.rs (`pick` / `make` handed over only in the game's shapes),
  ready.rs (an engine world's checklist).
* Lua (CI now installs Lua 5.1, so the glue tests run there): `glue/test_hybrid_glue.lua` +4 (pick, pick not found,
  make with others in the list, a race the realm doesn't offer).
* `cd launcher && node --test tests/vanilla.browser.test.cjs` (7): one click, Home, Quick start, another world running,
  not on Windows, Characters, Play with friends.
* `python3 -I hosting/tests/test_solovanilla_profile.py` (6), `python3 stream/tests/test_docs.py stream/wrath_portal.py`.

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
