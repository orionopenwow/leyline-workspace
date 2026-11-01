# Den Lite (prototype)

A tiny game server for the 3.3.5a client (build 12340), meant to be built into the Leyline launcher's **Your Den**:
an instant white room to build in, with no Docker and no database. The player's own stock client connects to it on
this PC like to any server (realmlist `127.0.0.1`). Owner's go-ahead: 1 Nov 2026. Status: **prototype, verified only
by its own headless test client** (below); not yet tried with a real game client.

It opens the same Den files as the full Den (`docs/DEN.md`, "The save file": format `leyline-den`, version 1) and the
same stamps (`realm/custom/lua_scripts/den-stamps/*.den.json`), so a Den made here moves to the Docker Den later to
share with friends, and back.

## Run it

```
cargo build --release            # its own crate: not in the launcher's workspace
den-lite --dens <folder> --bind 127.0.0.1 [--stamps <folder>] [--password den] [--name Wren] [--gender male|female] [--client <game folder>]
```

- **Login** on 127.0.0.1:3724, **world** on 127.0.0.1:8085 (`--login-port`, `--world-port`). The realm list has one
  realm, "Your Den", at the world server's address.
- **Account and password.** Any account name logs in (the launcher fills in `DEN`), with **one fixed local password,
  `den` by default** (`--password`, or `DEN_LITE_PASSWORD`). "Any password" and "the password you set the first time"
  can't be done: SRP6 makes the server send values built from the password's verifier *before* the client proves
  anything, and both sides then derive the session key (which also keys the world server's header encryption) from
  the password. A server that doesn't know the password can't finish a login. An empty password isn't an option
  either: the stock login screen won't log in without one. So the player types `den` (the launcher should say so
  next to the button, or set a password of its own with `--password`).
- **Den files**: `.den save <name>` writes `<dens>/<name>.den.json`; `.den load <name>` reads it. Names are letters,
  digits, - and _ (up to 24), as the launcher's `host/dens.rs`.
- **The launcher's Open**: while the player is in the world, `<dens>/open.json` (`{"file": "<name>", "id": "<id>"}`,
  `host/dens.rs` `open_doc`) is checked every 2 seconds and at login; a new id opens that file, replacing the Den (a
  copy of what was there is saved as `backup`). Each id opens once, also after a restart (`<dens>/.open-done`).
- **Stamps** come from `--stamps` (default `den-stamps` beside the program); the six in this repo are also built in.
- **`--client <game folder>`** (optional): `.den mods` then also lists what the launcher's mod manager put in that
  game (`<game>/Leyline/mods.json`). The launcher's `host/lite.rs` doesn't pass it yet.

## In the game

| Command | What it does |
|---|---|
| `.den` | the commands |
| `.den place <name>` | one piece 3 yards in front of you, facing as you face (`chair`, `lantern`, `house`...: whole name first, then a word's start) |
| `.den find <word>` | the pieces whose name has that word (12 shown) |
| `.den stamps` | the stamps |
| `.den stamp <name> now [degrees]` | puts a stamp down where you stand, facing as you face (15 degree steps); `x` ahead of you, `y` to your left, as `ov_den_home.lua` |
| `.build undo` | takes the last stamp or load back (50 deep) |
| `.den save <name>` | the whole Den to a file (the room is its look; the props are in it) |
| `.den load <name> [replace\|add]` | opens a file; asks first when the Den has props; `replace` keeps a copy as `backup`, and one undo brings the old Den back |
| `.den files` | the saved Dens |
| `.den room white\|stone\|wood` | the room's look (`ov_den.lua` THEMES) |
| `.den size [1-4]` | how big the Den is; with a number, the room rebuilt at that size (smaller only when every prop still fits) |
| `.den grow` | the next size: the white room's floor and walls rebuilt bigger, and what counts as inside the Den follows |
| `.den companion` (`.den jaina`) | your companion comes and follows you; again: back to your side |
| `.den companion away` | she goes (also when you log out or leave the Den) |
| `.den ask <words>`, or say `Jaina, <words>` within 10 yards of her | written down for your own assistant (Leyline's Agents page: `den_messages`); its `den_say` lines are hers |
| `.den mode [id]` | Leyline's game modes and Host templates; with an id, what it is and how to run it on this PC (the full Den) |
| `.den mods` | the addons your game loaded, and (with `--client`) the launcher's mods in it |

The character is a ready-made human (female by default, `--gender male`), level 1, in a white shirt, trousers and
boots, standing in the middle of the Den on the GM island (map 1, 16226.2, 16257.0, floor 13.2: `game_tele` 424). At
every login five chat lines are a quick start (place a chair, put down the inn, undo, save, grow and the companion).
Other chat is said back as your own line. One creature, the companion; no combat, spells, quests or loot.

### Growing: the white room's sizes

A new Den is the smallest room: **size 1, 16 x 16 yards** (props up to 22 yards from its middle: the room and a yard
around it). `.den grow` rebuilds it a ring bigger: 32, 48, then 64 yards across (half widths 8, 16, 24, 32: the full
Den's `OV_DEN_SIZE` range, so a file opens at the same size there). Each size's room and yard sit inside the launcher's
white pad (`leyline-terrain` `pad`: 2 x 2 chunks, then one chunk more each side per ring): sizes 1 and 2 in the pad's
first ring, 3 and 4 in its second. When Den Lite has the Den's ground file and the room grows past it, it says so (the
launcher's Make the Den bigger opens the next ring). The size is the file's `room.size`: `.den save` keeps it, a
replacing `.den load` opens the room at the smallest size that holds it (a full-Den file's 12 opens at 16) and undo
puts the old size back.

### The companion, and "Ask Claude"

`.den companion` brings one creature: Jaina (the owner's choice; the game's own model and name, in game only), entry
32364 and model 30865 as `ov_den_companion.lua` (AzerothCore's `creature_template`), level 80, friendly to everyone
(faction 35), immune to players and creatures (unit flags 832), no gossip, vendor or quests. The client learns her name
from this server's answer to its creature query ("Jaina", "<Your companion>" under it). She stands 2.5 yards behind you,
a little to the left, and follows loosely: twice a second the server checks, and when her spot is more than 2 yards
from where she is (or is going) she walks or runs there (`SMSG_MONSTER_MOVE`, one point; the server keeps her position
along the way). A jump of more than 30 yards puts her back at your side; leaving the Den sends her off.

She talks through the same files as the full Den, in the Dens folder, so the launcher's Den tools for your assistant
(`launcher/src-tauri/src/agents/den.rs`) work unchanged: `companion-in.json` (written every 10 seconds and after each
ask: the heartbeat `at`, `"server": "den-lite"`, `companion`, `owner` with where you stand, `room` with its size, `said`,
and the last 20 `asks` with `where`) and `companion-out.json` (`den_say`'s numbered lines: the new ones under 5 minutes
old are said, one every 3 seconds, while she is out; `read_at` within 10 minutes makes her answer "Let me think about
that" instead of "nobody is reading my notes"). The launcher's tools use Den Lite's folder while its heartbeat is fresh
(and when there is no Host pack); `den_catalog`, `den_recipes` and `den_place_layout` say they need the full Den (Den
Lite builds by hand), and Den Lite doesn't take `companion-place.json`.

### Game modes and mods

Den Lite runs no game mode, and doesn't pretend to. `.den mode` lists the Host templates (`stream/modepacks.json`) and
the Modes page's modes (`stream/playmodes.json`), both built in; `.den mode <id>` says what one is and how to run it on
this PC: its Host template (Leyline › Host › Host a mode › <name> › Host, with the ruleset and scripts), an in-game
command on a realm that has it, or "coming". `.den mods` lists the addons the game loaded (the names the client sends
when it logs in; the stock `Blizzard_` ones are only counted) and, with `--client`, the launcher's mod list for that
game. Addons are client-side, so the Den is a quiet place to try them; mods with realm parts need the full Den.

## How it works

| File | |
|---|---|
| `src/login.rs` | the login server: SRP6 (`wow_srp`), protocol 8 (`wow_login_messages`), the realm list |
| `src/world.rs` | the world server: auth session, character list, the login sequence, movement, chat commands, open.json, the companion's messages and her ticker |
| `src/den.rs` | the room plan (`ov_den.lua` `plan`) and its sizes (rings), stamps, undo, the save file and its checks (`OVBuild.DenCheck`, `host/dens.rs`) |
| `src/companion.rs` | the companion: following, asks and lines, the `companion-in.json` / `companion-out.json` files |
| `src/play.rs` | `.den mode` (the built-in mode lists) and `.den mods` (the client's addon list, the launcher's `mods.json`) |
| `src/raw.rs` | the two messages written by hand (gameobject creates, the gameobject query answer); why is at the top of the file |
| `src/objects.rs` | the gameobject entries Lite can show (415: the catalog of `ov_build.lua`, the looks, the stamps, the starters) with type, display id, name and size, generated by `tools/gen_objects.py` from AzerothCore's world database `gameobject_template` |
| `tests/client.rs` | the headless test client |

### The login sequence (what the world server sends)

1. On connect: `SMSG_AUTH_CHALLENGE` (server seed).
2. `CMSG_AUTH_SESSION` (the client's proof, checked against the login server's session key) ->
   `SMSG_AUTH_RESPONSE` (AUTH_OK, expansion 2), `SMSG_ADDON_INFO` (one entry per addon the client listed),
   `SMSG_CLIENTCACHE_VERSION`, `SMSG_TUTORIAL_FLAGS` (all seen). From here headers are encrypted.
3. `CMSG_READY_FOR_ACCOUNT_DATA_TIMES` -> `SMSG_ACCOUNT_DATA_TIMES` (global mask 0x15).
   `CMSG_CHAR_ENUM` -> `SMSG_CHAR_ENUM` (one character, its outfit's display ids). `CMSG_REALM_SPLIT` -> `SMSG_REALM_SPLIT`.
4. `CMSG_PLAYER_LOGIN` -> in AzerothCore's order (`WorldSession::HandlePlayerLogin`,
   `Player::SendInitialPacketsBeforeAddToMap` / `AfterAddToMap`): `SMSG_LOGIN_VERIFY_WORLD`,
   `SMSG_ACCOUNT_DATA_TIMES` (per-character mask 0xEA), `SMSG_FEATURE_SYSTEM_STATUS`, `SMSG_MOTD`,
   `SMSG_BINDPOINTUPDATE`, `SMSG_INITIAL_SPELLS` (none), `SMSG_ACTION_BUTTONS` (144 empty), `SMSG_INITIALIZE_FACTIONS`
   (128), `SMSG_LOGIN_SETTIMESPEED`, `SMSG_TIME_SYNC_REQ`, `SMSG_UPDATE_OBJECT` (the player: create type 3, flags
   self + living, speeds, and the fields for race/class/gender, health, level, faction 1, display 49 or 50, bounding
   radius, combat reach, cast speed, hover height, unit flags, player bytes, the visible items), `SMSG_INIT_WORLD_STATES`
   (map 1, area 876, none), then the room and the props (`SMSG_UPDATE_OBJECT`, 40 gameobjects a message: create type
   2, flags 0x250 with position, guid counter and packed rotation; guid, type, entry, scale, display, flags, parent
   rotation (0, 0, 0, 1), bytes 1), and a system line.
5. Afterwards: movement packets (`MSG_MOVE_*`) keep the player's position; `CMSG_GAMEOBJECT_QUERY` ->
   the query answer (type, display, name, 24 data fields, size); `CMSG_NAME_QUERY`, `CMSG_PING`,
   `CMSG_UPDATE_ACCOUNT_DATA`, `CMSG_LOGOUT_REQUEST` (instant), `CMSG_MESSAGECHAT` (the commands).
6. The companion: her create (`SMSG_UPDATE_OBJECT`, create type 2, object type unit, flags living, the unit fields
   above); `CMSG_CREATURE_QUERY` -> `SMSG_CREATURE_QUERY_RESPONSE`; `SMSG_MONSTER_MOVE` (her walks); `SMSG_MESSAGECHAT`
   monster say (her lines); `SMSG_DESTROY_OBJECT` when she goes. All through `wow_world_messages`.

### Verified, and only by the test client

`cargo test` runs a headless client made from the same crates: it logs in with SRP6 (and is refused with a wrong
password), reads the realm list, does the auth session with header encryption, checks the character list (name, race,
outfit), enters the world (the order above), reads the player's create and the room's 128 objects with the crate's own
parser (entries, display ids 6752 and 7976, scale, positions, the walls' packed rotation), walks (a heartbeat), puts
the campfire stamp down and checks its 10 objects and where they stand, queries an object, saves a file and reads it
back, undoes the stamp (10 destroys), opens the file (the same props in the same places), opens it again (asks, then
replaces and keeps a backup, then undo), changes the look, opens a file through `open.json`, pings, asks a name and logs
out. Every stamp and both starter places open with nothing left out. A second run checks the sizes (grow, the room's
48 then 128 objects, a prop 25 yards out that fits only at size 2 and stops shrinking, save and load at the file's
size), the companion (her create with entry, model, faction and flags read by the crate, the creature query's name, an
ask by "Jaina, ..." written to `companion-in.json` with where you stood, the busy answer, a `den_say`-style line from
`companion-out.json` said in her voice, "someone is listening" after `read_at`, her walk after you run 12 yards, her
going when you leave the Den), `.den mode minigolf` and `.den mods` (the two addons the test client lists). The
launcher's side: `agents::den` tests that its tools use Den Lite's folder while its heartbeat is fresh (status, messages,
`den_say` into Den Lite's `companion-out.json`, the catalog's "needs the full Den").

That proves the server, the crates and the test client agree. **A real 3.3.5a client has not connected to it**: what
it accepts, draws and does is to verify on a PC with the game. The riskiest unknowns:

1. Whether the client leaves the loading screen with this set of messages (no spells, no inventory items, no
   `SMSG_SET_PROFICIENCY` and friends), or wants something else first.
2. The player's create: only visible items, no item objects in the inventory fields; the character panel will be
   empty and the model should still be dressed (the client draws visible items from its own Item.dbc). Unknown:
   whether a missing field (power, stats, attack times) trips the UI.
3. The gameobjects: that the client shows them from the create alone, or only once it has its query answer (both are
   sent), and that 0x250 with the packed rotation is what it wants (AzerothCore's flags; the crate calls 0x10
   "high guid" after mangos, AzerothCore "low guid": same bytes).
4. The ground: Den Lite has no map files, so the floor is the fixed height 13.2 and stamps stand on your feet's
   height; the white blocks stand at their own size on the ground (the full Den sizes them from the host's dbc).
5. Movement is not checked at all (the client is trusted: fine for one player on their own PC).
6. `SMSG_ADDON_INFO` sends no public key; a client addon with a non-standard CRC may be marked as changed.
7. The companion: whether the client draws her from this unit create (no auras, no power fields, no `UNIT_FIELD_BYTES_2`),
   shows "Jaina" from the query answer, and animates her walks from a one-point `SMSG_MONSTER_MOVE` with no spline
   flags (run or walk is the client's from the duration); whether model 30865 is the one to show; and whether monster
   say lines appear as "Jaina says: ..." in the chat frame and a bubble.
8. Growing: a size 4 room is 384 objects in one go (the client's limit on gameobjects near you is to see).

### One player only; what more players would need

The Den is shared state, so two connections see the same props, but nobody sees anybody else. More players need:
creating each player for the others (the out-of-view create, without the self flag) and destroying them on logout;
forwarding every `MSG_MOVE_*` to the others with the mover's guid; chat to everyone; sending each Den change to every
session (the per-session writer in `world.rs` is already shareable for that); several characters per account and a
character store; and visibility (who is near whom). For friends, the full Docker Den is the way ("share with friends").

## Crates and licences

| Crate | Version | Licence | Used for |
|---|---|---|---|
| `wow_srp` (gtker) | 0.7.0 | MIT OR Apache-2.0 | SRP6 login, the 3.3.5 world header encryption (`wrath-header`) |
| `wow_login_messages` (gtker) | 0.5.0 | MIT OR Apache-2.0 | the login protocol (version 8) |
| `wow_world_messages` (gtker) | 0.3.0 | MIT OR Apache-2.0 | the world messages (feature `wrath`: 3.3.5) |
| `serde`, `serde_json` | 1 | MIT OR Apache-2.0 | the Den files |
| `flate2` | 1 | MIT OR Apache-2.0 | the client's addon list (zlib; `wow_world_messages` already needs it with libz-sys, zlib licence) |

All without default features where possible. Release build (`opt-level = "s"`, thin LTO, stripped): about 1.3 MB on
Linux x86-64. Note for Windows builds: `libz-sys` builds zlib with the C compiler (MSVC is there for Tauri anyway).

## Next steps (into the launcher's Your Den button)

1. Build `den-lite` for Windows in the launcher's release and put it where `launcher/src-tauri/src/host/lite.rs`
   looks (`$LEYLINE_DEN_LITE`, `<app data>/Leyline/den-lite/bin/`, beside the launcher); it already takes
   `--dens <dir> --bind 127.0.0.1` and honours `open.json`.
2. The launcher shows the password (`den`) on the Your Den page, or passes its own `--password` and shows that.
3. Try it with a real client on a PC with the game (the unknowns above), and fix what it wants.
4. Then: `.den home`, the stamp mark (stakes), and model props tested. Building from the assistant (`companion-place.json`)
   and the launcher passing `--client`.
5. Later: move to the workspace once stable; the browser client could connect too (out of scope here).
