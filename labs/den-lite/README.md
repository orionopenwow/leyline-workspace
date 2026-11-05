# Den Lite (prototype)

A tiny game server for the 3.3.5a client (build 12340), built into the Leyline launcher's **Your Den**: one click
into an instant open world (the game's own grass since 3 Nov) to build in, with no Docker and no database. The player's own stock client connects to it
on this PC like to any server (realmlist `127.0.0.1`). Owner's go-ahead: 1 Nov 2026. Status (2 Nov): **in use** behind
the launcher's Play solo. CI builds it (`den-lite.yml`) into the release `den-lite-latest`, and the launcher downloads
it on first use (`launcher/src-tauri/src/host/lite.rs`). The owner's real game logs in to it (the den-test on his PC,
2 Nov); the fixes that real game needed (the addon list, the white ground, the key bindings) are noted below.

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
  either: the stock login screen won't log in without one. The player never types it: the launcher's one-click
  sign-in (`launcher/src-tauri/src/denclick.rs`) enters the account and `den` and enters the world.
- **The Den keeps itself**: within 2 seconds of any change (and at logout) the whole Den is written to
  `<dens>/autosave.den.json`, and Den Lite opens it when it starts, so a restart brings back the room as it was
  (nothing to undo after it). A damaged autosave is left as it is and the Den starts empty.
- **Den files**: `.den save <name>` writes `<dens>/<name>.den.json`; `.den load <name>` reads it. Names are letters,
  digits, - and _ (up to 24), as the launcher's `host/dens.rs`.
- **The launcher's Open**: while the player is in the world, `<dens>/open.json` (`{"file": "<name>", "id": "<id>"}`,
  `host/dens.rs` `open_doc`) is checked every 2 seconds and at login; a new id opens that file, replacing the Den (a
  copy of what was there is saved as `backup`). Each id opens once, also after a restart (`<dens>/.open-done`).
- **The game's own settings** (`src/accdata.rs`, 2 Nov): every `CMSG_REQUEST_ACCOUNT_DATA` gets an answer (empty the
  first time, so the game uses its default keys), and what the game uploads (key bindings, macros, chat layout) is kept
  in `<dens>/account-data/`, so the keys someone sets are there the next time. Before this the game had no key
  bindings at all and W did nothing.
- **Stamps** come from `--stamps` (default `den-stamps` beside the program); the six in this repo are also built in.
- **`--client <game folder>`** (optional): `.den mods` then also lists what the launcher's mod manager put in that
  game (`<game>/Leyline/mods.json`). The launcher's `host/lite.rs` passes the game folder Your Den plays with.

## In the game

| Command | What it does |
|---|---|
| `.den` (`.den commands`) | the commands |
| `.den help` (`.den guide`) | opens the Leyline UI's Den guide (a paged help window: WrathUnleashedUI `Guide.lua`, also `/den guide`); without the UI, the commands |
| `.den place <name>` | one piece 3 yards in front of you, facing as you face (`chair`, `lantern`, `house`...: whole name first, then a word's start) |
| `.den find <word>` | the pieces whose name has that word (12 shown) |
| `.den stamps` | the stamps |
| `.den stamp <name> now [degrees]` | puts a stamp down where you stand, facing as you face (15 degree steps); `x` ahead of you, `y` to your left, as `ov_den_home.lua`. Near the Den's edge it is moved toward the middle until all of it is inside (`Den::fit_stamp`; every built-in stamp fits the size 1 room) |
| `.build undo` | takes the last stamp or load back (50 deep) |
| `.den save <name>` | the whole Den to a file (the room is its look; the props are in it) |
| `.den load <name> [replace\|add]` | opens a file; asks first when the Den has props; `replace` keeps a copy as `backup`, and one undo brings the old Den back |
| `.den files` | the saved Dens |
| `.den room white\|stone\|wood` | the look: `white` is the open white plane (no room pieces; the default since 1 Nov), `stone` and `wood` a room (`ov_den.lua` THEMES) |
| `.den size [1-4]` | how big the Den is; with a number, the room rebuilt at that size (smaller only when every prop still fits) |
| `.den grow` | the next size: what counts as inside the Den grows (a stone or wood room is rebuilt bigger) |
| `.den companion` (`.den jaina`) | your companion comes and follows you; again: back to your side. She is there from login by default (`--no-companion`: only when called) |
| `.den companion stay\|follow\|come\|tips` | or say `Jaina, stay` (`wait`), `Jaina, follow`, `Jaina, come` (she walks over), `Jaina, tips` (`what next`): anywhere in the Den, never written down for the assistant |
| `.den jaina quiet\|talk\|chatty\|idea\|yes\|no\|tidy\|pet\|pets away`, or say `Jaina, quiet`, `Jaina, help`, `Jaina, build a cozy corner`, `Jaina, yes`, `Jaina, tidy up`, `Jaina, a rabbit`, `Jaina, what did I build?` | her brain (4 Nov, `src/jaina.rs`, docs/JAINA.md): how much she says unasked, a small set as a sparkling preview you keep or send away, tidying only her own untouched pieces, critters, what she remembers of this visit. The Dojo panel's Jaina rows send these |
| `.den companion away` (`Jaina, go away`) | she goes until called again (leaving the Den or logging out only hides her: she is back when you are) |
| `.den ask <words>`, or say `Jaina, <words>` within 10 yards of her | written down for your own assistant (Leyline's Agents page: `den_messages`); its `den_say` lines are hers |
| `.den mode [id]` | Leyline's game modes and Host templates; with an id, what it is and how to run it on this PC (the full Den) |
| `.den mods` | the addons your game loaded, and (with `--client`) the launcher's mods in it |
| `.den where` | where you stand against the ground under you, what your game did with its character (took it, moved, walked, the time sync) and every kind of message it sent; the den-test asks it |
| `.den diag [tag]` | asks the Leyline UI for its view of the character (into the events as `wudiag` lines, after `at:<tag>`); the den-test's before / after |
| `.den dojo` (`.dojo`) | the Dojo: a training ring in front of you, built by your companion piece by piece, with three training dummies in it (below, "The Dojo") |
| `.den dojo dummies [1-5]` / `clear` | training dummies to hit (below, "Sandbox combat") / the dummies down (`.build undo` takes the ring back) |
| `.den dojo pack\|mover\|healer\|tank\|boss` | the other dummies: five for area spells, one walking a circle, a wounded friendly one to heal, one that hits back, the practice boss |
| `.den dojo parse 1\|3\|5`, `reset`, `refill`, `cooldowns`, `fight on\|off`, `best`, `dps` | a timed parse (best per class kept), everything full again, your power, your cooldowns, dummies hitting back, your bests, your numbers now |
| `.den gear <item>...` / `<slot> <item\|none>` / `list <slot> [page]` / `reset` | any item's look on you, in its slot (several at once; a second one-hand weapon in the off hand); looks: `thunderfury`, `shadowmourne`, `warglaives`, `sulfuras`, `valanyr` |
| `.den mounts` / `list [page]` / `off` | every mount your game has, in your spellbook; cast one to ride (again, or its buff right-clicked, to get off); flying ones fly with `.den fly on` |
| `.den class <class> [spec] [level]` | any class (warrior, paladin, hunter, rogue, priest, deathknight, shaman, mage, warlock, druid) and spec at any level (80 if none): its spellbook, talents and every bar at once; `.den class` alone: the list |
| `.den spec [<spec>]` | the class's three specs, or that spec at once (its talents, spellbook and bars) |
| `.den bars [list\|save <name>\|use <name>\|reset\|delete <name>]` | named bar loadouts for the class and spec, kept in the Dens folder |
| `.den gearset [list\|t1-t10\|<yours>\|save <name>\|delete <name>\|off]` | a whole look at once: the class and spec's raid tier from your own game's tables, or one of your saved sets; weapons are real items |
| `.den dojo levels\|light\|heavy\|group\|range` | dummies three levels below and above you, the healing range (five friends), distance markers |
| `.den dojo demo [seconds] [kind]`, `invulnerable on\|off`, `meter reset`, `pet off` | a practice run (your main bar cast for you, a number at the end), nothing hurts you, the meters again, your pet away |
| `.den fly on\|off` / `.den speed 1-5` | fly (jump, then Space to rise, X to sink) / run and fly faster |
| `.den jaina build <stamp>`, or say `Jaina, build a garden` (a tavern, a forge, a reading nook...) | she walks into its middle and builds it the way a person would (big pieces first, small last, things on tables at the end), casting at each piece, then steps back to look at it (`dojo` is the training ring; docs/DEN-STAMPS.md) |
| `.go <place>` (`.tele`, `.den go`) / `.go <map> <x> <y> <z>` / `.back` / `.den home` | anywhere your game has: 221 places (every zone, cities, towns, dungeon and raid doors, battlegrounds, arenas: `.den places [word]`, the Teleports window `/tp`); `.back` where you were; `.den home` your Den from any map; a fall through the world after a teleport is lifted back (docs/DEN-PLACES.md) |
| `.den here [name]` | makes your Den here: the Den that was where you were is kept for that place, and the one kept for this place (if any) opens |
| `.den size city` | the biggest Den (sizes 5 to 7: 128, 256 and 512 yards across, open ground) |
| `.den copy lionspride` | the Lion's Pride copied from your own game files (made by the launcher into `stamps-local` when it starts the Den; never shipped): docs/DEN-STAMPS.md |

The character is a ready-made human (female by default, `--gender male`), level 1, in a white shirt, trousers and
boots, standing in the middle of the Den on the GM island (map 1, 16226.2, 16257.0, floor 13.2: `game_tele` 424), **on**
the ground there (5 cm above it; see "Standing and walking" below).
No chat spam (owner 1 Nov 23:46; `src/notice.rs`): with the Leyline UI's Den bar (WrathUnleashedUI `DenBar.lua`, its
hello `wuden hello`), what Den Lite says that nobody typed for (the welcome, Jaina's one greeting, the encouragement
moments, the Builder's and the bar's answers, the launcher's answers, a friend in your Den) is a toast in the bar
(`wuden toast <kind>|<text>`), and the bar hears every save (`wuden saved auto|<name>`) and a status line. Without a
hello within `--ui-wait` milliseconds (4000) of the game being in the world (its first SET_ACTIVE_MOVER, time sync
answer or move after the loading screen; at most `--ui-load`, 30000, after Enter World), the old way: the welcome and five
quick-start lines in the chat. Typed `.den` commands always answer in the chat. Other chat is said back as your own
line. Creatures: the companion and the Dojo's dummies; a sandbox combat against the dummies ("Sandbox combat"); no
quests or loot.

### The Leyline UI's Den panel and Builder, and asks for the launcher

The Leyline UI (WrathUnleashedUI: `/den`, F7 or `/builder`) talks to Den Lite over the realm's addon channel
(`OVMOD`, `src/ovmod.rs`), as it talks to the full Den's `ov_den.lua` / `ov_build.lua`: the Den panel's info, Save,
Load, the room's look; the Builder's catalog (the props Den Lite knows, in the Builder's categories; all of it: what
doesn't fit one message follows as `catmore`; with each prop's model path for the gallery's 3D tiles when the launcher
put the player's own `GameObjectDisplayInfo.dbc` in the Dens folder, `src/models.rs`), Spawn, Near,
move x/y/z, exact numbers, turn, tilt, size, Delete and Undo (all autosaved). **Invite a friend** and **Shape the
ground** (also `.den invite`, `.den forge`) are asked of the launcher through `<dens>/den-lite.ask`; its answer
(`den-lite.answer`) comes back into the game: the link into the Den panel, or a line.

### The milestone log

`<dens>/den-lite.events` (new at every start; `src/events.rs`): one JSON line per step on the way in
(`login_challenge`, `auth_ok`, `realm_list`, `world_auth`, `char_enum`, `char_create`, `player_login`, `spawn`,
`in_world`, `mover`, `first_move`, `move_flags`, `landed`, `walked`, `builder_ready`, `ask`, `peer_seen`, `logout`,
`moves`), and what happens in the Den (`den_change`, `den_save`,
`den_load`, `autosave`, `answer`, `test_cmd`; the Dojo's `dojo` and `dojo_ack`), never a password or an invite link. The launcher's one-click sign-in
(`launcher/src-tauri/src/denclick.rs`) and the owner's `den-test` (tools/den-check) read it.

### The den-test's commands (`src/testctl.rs`)

Started for a test (`--test-control`, or `DEN_LITE_TEST_CONTROL=1`, which the launcher's `den-play --test` sets; only
on 127.0.0.1), Den Lite takes chat commands from `<dens>/den-lite.test-cmd` (`{"id": n, "cmd": ".den place chair"}`)
while the player is in the world and runs them as if they had been typed, so the test never depends on the game
window having the keyboard. Only `.den` / `.build` commands. Each one's lines and changes go to
`<dens>/den-lite.test-out` (one JSON line: `said`, `created`, `destroyed`, `props`, `look`, `size`).

### Standing and walking (3 Nov)

The owner's report (3 Nov, the real game, screenshots): in the Den the character **floated above its own shadow** on
the white ground and **couldn't move**. What the code shows:

- **The ground and the spawn are the same height.** The launcher's white ground is the whole tile flattened to 13.2
  (`leyline-terrain` `pad::DEN`, `host/terrain.rs` `first_ground`; the tile's heights are relative to each chunk's own
  base and the crate writes them so; a test checks the whole tile with every chunk on its own base), and Den Lite reads
  the same delta (`Den::floor_at`). But Den Lite put the player **a whole yard above** it (`floor + 1.0`, in the
  character list, Enter World and the bind point alike).
- **A game that moves its character drops that yard at once and lands** (`MSG_MOVE_FALL_LAND`). One that hangs a
  yard up *and* won't walk is not moving its character at all: it doesn't have control of it (or it is rooted), so it
  neither falls nor walks. The den-test couldn't tell: its "first movement" passed on *any* movement packet (a
  landing or a turn), and the launcher's test stopped stepping forward at the first one.

So (`src/movewatch.rs`, `world.rs`):

1. The player comes in **on** the ground (`Den::spawn_z`: `movewatch::SPAWN_LIFT`, 5 cm, above the ground under the
   spot), so nothing floats even in a game that isn't moving its character.
2. Control is said **again once the game is in the world**: the first `SMSG_CLIENT_CONTROL_UPDATE` goes during the
   loading screen; when the game sends `CMSG_SET_ACTIVE_MOVER` (it takes its character once the world is on screen)
   Den Lite sends the control update once more and an unroot (`SMSG_FORCE_MOVE_UNROOT`, as AzerothCore's
   `SetRooted(false)`). Both change nothing for a game that already moves its character.
3. The launcher's Den ground marks every open chunk walkable (the chunk flag "impassable" off; `pad::flatten`, and
   `pad::whiten` for a ground made before).
4. **What the game did is written down** (`movewatch::Watch`, the events log): `spawn` (z against the ground under it,
   ground file or fixed floor, the nearest prop), `mover` (it took its character), `first_move` (which packet, the
   flags that hold a character up or still, z against the ground), `move_flags` (root, no gravity, hover, flying...),
   `landed`, `walked` (a yard or more from where it came in) and `moves` (a summary at logout or when the game
   closes). `.den where` says the same in the game, and the den-test's command channel answers it with `pos`,
   `ground`, `on_ground` and `me`.
5. **The den-test checks walking** (tools/den-check `walk_lines`): the launcher steps forward until Den Lite logs
   `walked` (denclick.rs: not just any first move), then the test passes only if the game took its character, the
   step walked it, nothing held it up or still, and its feet are within half a yard of the ground; the Leyline UI's
   own view of the character (`wudiag`: its speed, falling, control, the forward key) is printed too.

If the owner's next den-test still fails, its lines say which: no `mover` and no movement (the game never took its
character: see the `control=` line), a `ROOT` / `DISABLE_GRAVITY` flag, movement but no walk (the keys line), or a
character off the ground.

**3 Nov, later: on the ground, `control=1`, W bound, and still not one movement packet** (the launcher's posted keys
and the owner's own W alike). Den Lite's login was compared, message by message and byte by byte, with AzerothCore
(`WorldSession::HandlePlayerLoginFromDB`, `Player::SendInitialPacketsBefore/AfterAddToMap`,
`Object::BuildCreateUpdateBlockForPlayer`, `Object::BuildMovementUpdate`, `Unit::BuildMovementPacket`, the
`UpdateFields.h` indices) and with the message crate's own `examples/wrath_server`. The fields, the speeds (order and
values), the movement flags (none) and the update mask were right; three things differed, now as AzerothCore:

1. **The time sync came before the player existed.** AzerothCore (and the crate's example) send the player's create
   first (`Map::AddPlayerToMap`), then `SMSG_TIME_SYNC_REQ` (`SendInitialPacketsAfterAddToMap`: `ResetTimeSync`,
   `SendTimeSync`). Den Lite sent it before the create, so the game's only time sync named no character of its own.
2. **The movement block's time was 0.** AzerothCore writes its clock (`GameTime::GetGameTimeMS`) as the movement's
   time; Den Lite now writes `movewatch::server_ms` (milliseconds since it started, from 1000).
3. **The update flags were SELF | LIVING (0x21).** Every AzerothCore unit carries LIVING | STATIONARY_POSITION
   (`Unit::Unit`: `m_updateFlag = (UPDATEFLAG_LIVING | UPDATEFLAG_STATIONARY_POSITION)`), its own copy adds SELF: 0x61
   (`movewatch::player_update_flags`; the Living block is what follows either way).

And the control update that went during the loading screen is gone (AzerothCore sends none at login); it is said
when the game takes its character (`CMSG_SET_ACTIVE_MOVER`), with the unroot.

Which of these the real game needed is for the den-test to show, so it also says now: **every opcode the game sends
once in the world** (`client_op` events, `.den where`'s "Your game sent: ..."), whether it **answered the time sync**
(`time_sync`), and **W pressed a second way**: tools/den-check brings the game to the front and presses W and the up
arrow with `SendInput` (scan codes, as a keyboard), asks the Leyline UI for its view before and after (`.den diag
<tag>` -> `wuden diag`, WrathUnleashedUI `W.DenDiag`: speed, falling, dead, control, mounted, on a flight, flying,
swimming, W's and Up's binding with the override bindings, a protected call the game blocked) and says which way moved
the character. The launcher's ground changed the same day (no edited tables or texture of ours: docs/DEN.md "The
ground"), which takes the ground out of the question too.

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
along the way). A jump of more than 40 yards puts her back at your side; going more than 300 yards from the Den sends
her off (she waits in the Den).

**Flying, props, keeping up (3 Nov, `src/flight.rs`).** The owner: "when user flies have Jaina fly with, more
dynamically", "fix Jaina collision", "she fades away after a certain distance".

- *Flying with you.* With `.den fly on` and you more than 1.5 yards above the ground, she flies: she is told so
  (`SMSG_SPLINE_MOVE_SET_FLYING`, `SMSG_SPLINE_MOVE_GRAVITY_DISABLE`: her packed guid, as AzerothCore's
  `Creature::SetCanFly` / `SetDisableGravity` tell the players around), then every 300 ms that her spot moved more than a
  yard, a curved path to 2.5 yards behind you and 0.8 above your height (`SMSG_MONSTER_MOVE`, spline flags Flying
  0x2000 | Catmullrom 0x40000, every point written whole as AzerothCore's `PacketBuilder` does for such paths), at
  your own flight speed (`.den speed`). You land: she flies down beside you, then `UNSET_FLYING` and `GRAVITY_ENABLE`.
- *Around props.* Her walk to her spot goes around a prop standing in the straight line (a waypoint beside it, from the
  prop's model size when the launcher copied `GameObjectDisplayInfo.dbc`, else a small circle), never ends inside one
  and never closer than 2 yards to you; such a walk is a curved path (Catmullrom). The Dojo's builds still walk her
  straight to the spot the build wants.
- *Fading away* was Den Lite sending her off as soon as you were past the Den's own circle (22 yards at size 1), and
  the game removing her. She now follows anywhere within 300 yards of the Den, and past 40 yards (a jump, fast flying)
  she is put back behind you at once.

To see on the real game: whether the game flies her from the spline flags alone (else her create needs the flying
movement flags too), how the curves look, and her landing.

Her manners (owner 1 Nov: "smarter companions"; `companion.rs`, all deterministic and unit tested): she never stands
in your way (closer than 1.2 yards and stopped, she steps back to her spot), `stay` keeps her where she is (and a jump
across the room doesn't pull her), `come` walks her over and she follows again, and past 30 yards while following she
is put back at your side. Lines are few: one greeting at login (a toast in the Den bar), then only answers when spoken
to (no tips or nudges nobody asked for, and nothing when she rejoins you). With no assistant connected she
is still a guide: an ask her built-in guide knows (`GUIDE`: placing, stamps, undo, save, load, grow, looks, mods,
modes, Forge, friends, levelling, the assistant) gets its answer at once, and is still written down; with someone
reading her notes (`read_at`), the assistant answers instead. `tips` picks the next step that fits the Den (a first
piece, a first save, more room), then the general tips in turn. "Jaina, help" (since 4 Nov) suggests a small set in
front of you instead (docs/JAINA.md). "Jaina, build a garden" is hers too: she builds it ("The Dojo", below).

**Her brain (4 Nov, `src/jaina.rs` and `src/jaina_world.rs`; docs/JAINA.md).** Needs, moods and a utility choice once a
second (seeded, unit tested): she reacts to what you do (placing, deleting, saving, growing, flying, friends coming),
mostly with a gesture; looks around the Den when you stand still; hops through the air when far; turns to you before
a line. What she says unasked is rare (none in the first minute and a half, then at most one every three minutes and
eight a visit, a toast in the Den bar) and none at all in quiet mode. Her sets (`THEMES`: in season first) are a
preview until you say yes; critters (three at most) are never saved; "tidy up" asks first and takes only what she put
down and you haven't moved.

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

### The Dojo (3 Nov)

The owner's ask (3 Nov): a training room in the Den, "the Dojo", with dummies, flying, and Jaina building sets while
she casts. What Den Lite does (`src/dojo.rs`; the full Den's `ov_dojo.lua` does what needs combat; docs/DEN.md, "The
Dojo", has both side by side):

- **Fly** (`.den fly on|off`): `SMSG_MOVE_SET_CAN_FLY` / `SMSG_MOVE_UNSET_CAN_FLY` (packed guid, a counter), as
  AzerothCore's `Unit::SetCanFly` tells a player's game. **Speed** (`.den speed 1-5`): `SMSG_FORCE_RUN_SPEED_CHANGE`
  (packed guid, the counter, a byte 0, the speed) and `SMSG_FORCE_FLIGHT_SPEED_CHANGE` (no byte), times the normal 7.
  The game's acks (`CMSG_MOVE_SET_CAN_FLY_ACK` 0x345, `CMSG_FORCE_RUN_SPEED_CHANGE_ACK` 0x0E3,
  `CMSG_FORCE_FLIGHT_SPEED_CHANGE_ACK` 0x382: packed guid, the counter, the movement info without its guid, a float)
  are read before the message crate sees them: the `dojo_ack` event, and where the character is. Its flying moves
  (`CMSG_MOVE_SET_FLY` 0x346, ascend 0x359 / 0x35A, descend 0x3A7) are read by hand like the others (`raw_move`);
  `CMSG_MOVE_SET_FLY` isn't passed on to friends. A new time in the world starts on foot at the normal speed.
- **Jaina builds a set** (`.den jaina build garden`, "Jaina, build a garden", "make a campfire", the panel's buttons):
  the stamp goes into the Den at once (saved, one `.build undo`), its middle far enough ahead of you that it all fits
  in front, on the ground (also when you fly). With her out: she walks to a spot between you and it (`SMSG_MONSTER_MOVE`)
  and waits there; a ticker (`dojo_ticker`, every 50 ms) then sends one `SMSG_SPELL_START` from her (the whole build as
  its cast time, aimed at the middle) and, every 250 ms, the nearest piece left: her turn to it (`SMSG_MONSTER_MOVE`,
  facing angle), an `SMSG_SPELL_GO` aimed at its spot (`dojo::BUILD_SPELL`: only its look; the game draws it from its own
  Spell.dbc for any unit it knows, no aura needed) and its create. A piece taken back meanwhile (undo) never appears.
  Done: she follows again, "There. What shall we try next?", and the line. Without her: all at once, as a stamp.
  Friends in the Den see it at once.
- **The ring** (`.den dojo`): the stamp `dojo::DOJO_STAMP` (11 fences with the opening towards you, 4 torches, 2
  banners, 2 weapon racks, a target tent, 2 benches; all props Den Lite knows), built by her as above, then three
  **training dummies** in it: creature creates like hers with the game's training dummy entry (31144,
  `dojo::DUMMIES`), now targets for the sandbox combat (below): attackable (faction 7), 5 million health, refilled.
  `.den dojo dummies [1-5]` puts them up anywhere, `.den dojo clear` takes them down; they are shown again at login.
  Duels and a new look say they need the full Den.
- **The Den bar's Dojo panel** (WrathUnleashedUI `Dojo.lua`): its buttons send `wuden cmd dojo <...>` (as `.den dojo
  <...>`); Den Lite answers `wuden dojo server=lite;fly=0|1;speed=<n>;dummies=<n>;ring=0|1;building=0|1`.

#### The Dojo as a training ground (plan and what is built, owner 3 Nov 10:36: "fully complete the Dojo features")

What a training ground needs, and where each part stands:

| Part | What it does | Den Lite | Command / panel |
|---|---|---|---|
| Single target | the training dummy: hit it, your damage per second | built | `.den dojo dummies [n]`, Dummies |
| Pack of five | five weaker dummies close together: area spells | built | `.den dojo pack`, Pack |
| Moving target | one dummy walking a 4-yard circle: keep in range | built | `.den dojo mover`, Mover |
| Healing practice | a friendly wounded dummy losing health every second; your healing per second | built | `.den dojo healer`, Healer |
| Tank practice | a dummy that hits back once hit (lightly); `fight on`: all dummies do | built | `.den dojo tank`, `.den dojo fight on`, Tank, Fight |
| Damage meter | the readout in the Dojo panel (`wuden dps`), the numbers in the chat when a fight ends | built | hit a dummy; `.den dojo dps` |
| Timed parse | 1, 3 or 5 minutes from your first hit; the best per class kept (`<dens>/dojo-best.json`) | built | `.den dojo parse 1\|3\|5`, 1m 3m 5m |
| Boss practice | the practice boss (twice the size, it falls): every 9 s it marks the ground under you (a cast from it at your feet, 2.5 s), then the circle lands; get out. Your time and how often it caught you; the best time per class kept | built | `.den dojo boss`, Boss |
| Reset | every dummy full (the boss back up), cooldowns ready, health and power full | built | `.den dojo reset`, Reset |
| Refill / cooldowns | power and health full; every cooldown ready | built | `.den dojo refill`, `cooldowns`, Refill |
| Class, level, respec | any class at any level, free and at once | built | `.den class <class> [level]`, the class picker |
| Personal bests | per class: parses and the boss time | built | `.den dojo best` |
| Duel arena | a ring for you and a friend: countdown, win or lose | **next**: needs combat between two players' sessions (Den Lite's combat is the host's alone); the full Den has duels | `.den dojo duel` says so |
| A new look | the barber | the full Den | `.den dojo look` |

The panel (WrathUnleashedUI `Dojo.lua`): a Practice row (Pack, Mover, Healer, Tank, Boss), a Session row (1m, 3m, 5m,
Reset, Refill, Fight) and a class picker (the class, then Be) send the same `.den dojo` / `.den class` commands; on Den
Lite they are on when the Dojo line says `combat=1` (`;combat=1;class=<word>;level=<n>;fight=0|1;parse=<m|->;best=<boss
seconds|->` after the line above).

Tested here: `dojo.rs`'s unit tests (the bodies byte for byte, the acks, the build's steps and timing, the ring, the
dummies' spots, the kinds' places and the walker's circle), combat.rs's (each dummy kind, the parse, the boss's circles
and fall, reset), sandbox.rs's (the bests) and the headless clients (`the_dojo_flies_speeds_and_jaina_builds_piece_by_piece`,
`the_class_picker_and_the_dummies_take_spells_and_swings`). To see on the real
game: the flying and the speed; whether her cast shows from `SMSG_SPELL_START` / `GO` without anything else (else
`SMSG_PLAY_SPELL_VISUAL`, a kit id, is the fallback to try), and her turning; the dummies' model (display 27510).

Next (plan only): a dungeon queue with companions and raid training (the full Den), Jaina building recipes the
assistant writes, and `.glb` models as props (docs/DEN.md, "The Dojo").

### Placing with the mouse: pick, follow, click, selected; the handles; snap out (3 Nov)

The owner's asks (3 Nov): "show the actual prop moving around, not a placement circle", "the 3 directional arrows",
"hovering over an object makes it glow", and then his own editor's video: "it should function like this ... until it
works identically", "the 'snap out' camera mode working for sure too and infinite camera zoom out from character". The
stock 3.3.5a client gives an addon **no cursor-to-world call** and no gameobject hover event, so Den Lite
(`src/gizmo.rs`, `src/farsight.rs`) and the Leyline UI (`Gizmo.lua`, `Builder.lua`) build the video's flow from what
the client does have. In the video's order:

1. **Pick a card** (the gallery, bottom left: the category column, search, A-Z, Spin, named cards; the picked card
   gets a gold edge): `preview <entry>`. The selection panel shows it **in hand · click the ground** with its model on
   top. Nothing appears in front of you: the prop shows only once the cursor is over the ground (`follow` still puts
   it in front, for the keyboard's Enter).
2. **The real prop follows the cursor**: while the cursor is over the world the addon sends its screen point about 10
   times a second (`hover <u> <v> <aspect> <zoom>`); Den Lite casts the ray from the build camera through it onto the
   ground (`gizmo::project`, snapped to the grid, standing on the ground) and shows a copy of the real prop (same entry
   and model; not selectable) there. A gameobject's place is only in its create (`UPDATEFLAG_STATIONARY_POSITION`; no
   position in its update fields since 2.0), so each move is a create under the other of two guids, then the destroy
   of the old one: never a frame without it, at most one redraw per 100 ms.
3. **One left click places it and ends placing** (`commit`): the real prop where the copy was, and it is the
   **selection at once**: Den Lite answers `placed <id>`, `aim off` and the state, puts the glow, the handles and the
   halo on it, and the panel opens on it. **Shift+click** (`commit keep`) places it and keeps it in hand; Esc or a right
   click puts it away (`preview off`).
4. **The glow** (why this one): in 3.3.5a a gameobject's highlight is drawn by the client from `GAMEOBJECT_DYNAMIC`'s
   low flags, the ones a server sets on a quest object: `GO_DYNFLAG_LO_ACTIVATE` (0x01) and `GO_DYNFLAG_LO_SPARKLE`
   (0x08, the glitter round the whole model). Den Lite sends them in a values update (`raw::gameobject_dynamic`) to the
   selected prop only, again after every create of it (a move, a turn or a size makes it anew and a create clears it),
   and 0 when it is let go. Under it a **halo**: the game's own "Crate Highlight" model (display 2233, the glow round a
   quest crate) as an object of Den Lite's (entry 990010), as wide as the prop's footprint. Build mode still shows every
   prop as a goober under its **build twin's** entry (the client highlights it under the mouse; a right-click sends
   `CMSG_GAMEOBJ_USE`: selected, `picked <id>`, the menu at the cursor). A left click on a gameobject tells the server
   nothing in this client, so selecting stays a right-click; a left click on empty ground lets go (`selclear`).
5. **The handles**: red X, green Y at the prop's foot along the world's axes and blue over its top (up), set out from
   its box (the player's own `GameObjectDisplayInfo`, else its size). The client can't tint a gameobject and has no red,
   green and blue arrow models, so the handles are the three coloured flags (Warsong 5913, Netherstorm 7153,
   Silverwing 5912). **Right-click a handle**: grabbed (`grab x|y|z`; the real prop hidden for you, its glowing copy in
   its place); the hover stream then moves it **along that axis to the point of the axis line nearest the cursor's ray**
   (`gizmo::along`), live, at most a redraw per 100 ms, and the status says "Moving Cottage +4.45 yd along X"; the wheel
   steps it by the panel's step (`grab nudge`); a left click drops it (one change, one undo: "Moved Cottage +4.45 yd
   along X."), Esc or a right click puts it back (`grab off`).
6. **The selection panel** (the video's, in the Leyline look, our own icons from `tools/den-ui/icons/make_icons.py`):
   the model on top (right-drag turns it, the wheel zooms) with its size, the name and "selected", three tabs (the
   panel must fit above the action bars on a 768-unit screen): **Piece**: POSITION X / Y / Z in red, green and blue
   (type and Enter, or - / + and the wheel); MOVE · TURN · SIZE from where you stand (the pad: turn left, forward,
   turn right / left, bring here, right / smaller, back, bigger; lift up and down; the step 0.1 / 0.5 / 1 / 5 yd: Den
   Lite's `step fwd|back|left|right|up|down <yd>`, `grow`, `bring`); ROTATION · SIZE (turn, tilt, roll by 15 / 45 /
   90; size; the sliders); the quick buttons (tilt, upright, to ground, settle, random, centre, duplicate, reset,
   delete; Apply). **Swap · keys**: SWAP WITH the gallery's last pick (`swap <entry>`; All of this kind: `swapkind
   <entry>`, every prop of its kind, one undo), Lock, Copy, Paste, More, and the keys. **Select**: Nearest, Next, All of
   its kind, Clear; Shift+right-click adds (`seladd`). **Keys only while something is selected**: Del delete, U copy,
   Ins swap, Shift+Ins all of its kind, `,` `.` turn, Page Up / Down lift, Home bring here, End to ground, Ctrl+Z /
   Ctrl+Shift+Z undo and redo, Tab, Esc, numpad + / - size. Never WASD, Q / E or the arrows.
7. **Snap out**: one key (J, or the first of K, F9, F10 nothing else uses) or **View** on the rail. Den Lite makes an
   invisible unit nobody can click (`farsight.rs`: entry 990020, display 11686) up and back over where you stand, sets
   the player's **`PLAYER_FARSIGHT`** (field 624) on it, as the game's own Far Sight and Mind Vision do, and the client's
   camera goes there; the same key gives it back (far sight 0, the object gone). The object sits on the build camera's
   own line of sight, 35 yards further back (`gizmo::far_spot`), so the middle of the screen stays on the same ground
   and the ray stays exact (`Aids::view`: the build camera with the extra distance, from the far focus). While it is
   out the arrow keys pan it (`cam pan`, a short flying spline), and placing, dragging and selecting work from there.
8. **Zoom out without end**: in build mode the addon zooms the game's camera itself (2 yards a notch) so it always
   knows the distance (sent as `cam zoom`, used by the ray), with `cameraDistanceMax` raised to 50 (given back on
   leaving). Past the game's own limit each notch goes to Den Lite (`cam far 1`): the same far-sight object, further back
   each notch (8 yards plus a quarter of how far out it is, up to 1500); zooming in comes back the same way and, at 0,
   the view is yours again.

**The exact click** stays as the fallback and the calibration's truth: while placing, G starts the ground-target cursor
of Flare (1543, taught in `SMSG_INITIAL_SPELLS`); its `CMSG_CAST_SPELL` carries the exact point; Den Lite places there
(and stays in hand, for the calibration's six clicks), answers `SPELL_FAILED_DONT_REPORT` and clears the cooldown.

**Nothing holds the keyboard**: the override bindings (Esc, G, the wheel, the snap key, the arrows while snapped out, the
selection's keys) exist only in build mode and only while needed. A camera drag while placing, dragging or snapped
out puts the build camera's pose back (view 2, saved at the counted zoom).

The verbs (`caps=` lists them all): `preview <entry> [u v aspect zoom]`, `preview move|turn|off`, `hover`, `follow`,
`commit [keep]`, `grab x|y|z|nudge <yd>|off`, `axis`, `gizmo on|off`, `mode build|play`, `where`, `cam snap [off]|far
<n>|pan <dir> <yd>|zoom <yd>|off|where`, and the panel's `step`, `grow`, `bring`, `ground`, `settle`, `upright`,
`random`, `centre`, `reset`, `swap`, `swapkind`, `seladd`. Answers: `aim`, `axis`, `picked`, `placed`, `drag`, `moved`,
`cam`, `mode`, `ghost`, `state`.

What is proven here, and what needs the owner's PC: the ray, the axis-drag point, place-then-select, the glow's values
update, the far view's pose and its far-sight packets are unit-tested in `gizmo.rs`, `raw.rs` and `ovmod.rs`, and over
the wire in the headless client (`the_editors_pick_place_select_drag_and_snap_out`); the UI in
`stream/tests/test_wuui_gizmo.lua`, `_builder`, `_denlayout`; den-check's run in `test_den_check.py`. **On the real
game, to see** (den-check's `Den features.bat` photographs each step): that the sparkle and the halo show on the
selected prop; that `PLAYER_FARSIGHT` on a unit of Den Lite's moves the camera (and the flying spline moves it); that
the camera's target height on that object matches (`far_pivot` in the camera file, 0 until calibrated); that
`cameraDistanceMax` 50 really lets the camera to 50 yards (the zoom count assumes it); that `CameraZoomIn` / `Out`,
`SaveView` / `SetView` / `ResetView` aren't protected; the flags' colours. **The build camera's numbers are first
guesses** (field of view 90 degrees across the diagonal, pivot 2 yards over the feet, 0.5 rad down) until **den-check's
hover calibration** runs on the owner's PC (the real cursor at six points, at each the copy's place and, by pressing G
and clicking, the game's own point; the camera fitted, saved to `<dens>/den-lite.camera.json`, used from the next Den
start). Until then the copy can land a few yards from the cursor; the exact click (G) is always exact.
#### The mouse, picked by Den Lite: hover, click, drag; the build view (3 Nov, evening)

The owner (3 Nov): "still need to fundamentally build the feature of clicking objects, having them highlighted, moving
them around easily, clear and distinct build mode camera and intuitive features". Right-click selecting (above) leans on
the client's own goober highlight, which goes by its cached template, and **the client never tells the realm about a
left click on a gameobject**. But Den Lite already knows the camera (the build camera's pose, or exactly the far view's)
and every prop's place, turn and size. So Den Lite picks by itself (`src/pick.rs`, std only, its own tests;
`gizmo.rs` `click`, `drag`, the hover):

* **The ray pick** (`pick::pick`): the cursor's ray (`gizmo::ray`, from `Aids::view`) against each prop's **upright box**:
  its model's width, depth and height and bottom (`models::box_of`, from the player's own `GameObjectDisplayInfo`, at
  its size, 5% larger so an edge is easy to hit; a guess from its size when the table isn't there), turned with the
  prop. The nearest box entered wins, within the ray's reach (the camera's distance + 60 yards). Ties (entered within
  0.05 yd): the smaller box, then the newer prop. A box the camera is inside (a house round the far camera) counts only
  when nothing else is hit. Hidden props are never picked; locked ones are picked and selected but not dragged.
* **The glow under the cursor**: in build mode (`mode build`) the addon's hover goes even with nothing in hand (about
  10 a second, once more when the cursor comes to rest). The prop under it gets the selection's glow (the same
  `GAMEOBJECT_DYNAMIC` values update, to that player only); the one it left loses it unless it is selected (the
  selection also has its halo and handles). At most one change per 100 ms. The addon is told `hovered <id> <name>` (or
  `-`) and shows the name by the cursor.
* **Click** (`click <u> <v> <aspect> [<zoom>] [add]`, a left press and let-go under 0.3 s and 6 px): the prop under the
  cursor is the selection (its glow, handles, halo; `selected <id>`; the panel follows the state); on empty ground the
  selection is cleared (`selected -`). `add` (Shift) adds it to the selection (it glows too). Right-click still selects
  and opens the menu.
* **Press and drag** (`drag <u> <v> <aspect> [<zoom>]`, sent when the mouse pressed on a prop has moved 6 px, with the
  point where it was pressed): that prop is the selection; the real one is hidden for you and its glowing copy follows
  the hover stream. The ray at the press hit the prop's box at some point P; **the drag follows a level plane at P's
  height and keeps the foot's offset from P** (`pick::grip`, `pick::follow`), so the spot you grabbed stays under the
  cursor. The foot is snapped to the grid (when Snap is on) and set on the ground at its own height over it. Let go:
  `commit` (the real prop there, one undo step, selected again). Esc: `preview off` (back where it was). The wheel
  and Q / E while dragging: `preview turn ±15`. Nothing under the press: `grabbed -`.
* **Sharing the mouse with the camera** (what 3.3.5a allows): `WorldFrame`'s `OnMouseDown` / `OnMouseUp` can be hooked
  but not stopped, and `CameraOrSelectOrMoveStart` / `Stop` (the left-drag camera) are protected. What an addon may do
  is override-bind a mouse button outside combat (`SetOverrideBindingClick(owner, true, "BUTTON1", button)`). So while
  Den Lite says the cursor is over a prop, or a prop is dragged, `BUTTON1` is bound to `LeylineGizmoGrab` (registered
  for down and up clicks): the press is the addon's and the camera doesn't turn. Over empty ground BUTTON1 is the game's
  (left-drag turns the camera; right-drag as always). The binding never changes while the button is held; a let-go the
  game kept from the addon is caught by `IsMouseButtonDown` on the next frame; the hook and the binding seeing the same
  press count once.
* **The build view** (`cam build`): entering build mode the addon frees the cursor (`MouselookStop`), poses the build
  camera at its counted zoom and asks for the build view: the far-sight object up and back over where you stand
  (`BUILD_BACK` 14 yards past the client's zoom, on the camera's own line of sight), a pose Den Lite knows exactly
  whatever the player does after. A gold frame round the screen and **BUILD** at the top say which mode it is. WASD
  and the arrows pan the view while it is out (held: on every 0.2 s), the wheel zooms (past the client's limit the far
  view goes further, as above), the snap key toggles the 35-yard overview. Leaving build mode (`mode play`) gives the
  view back (Den Lite does it too, so it is never left out).
* **One hint line** in build mode: "Click a prop to select · drag to move · Shift adds · Q E turn · Del deletes · F7
  leaves Build". The guide's Build and Play, Placing, Selecting and Keys pages say the same.

Verbs added: `click`, `drag`, `cam build` (caps: `click,drag`; the addon falls back to right-click selecting without
them). Answers added: `hovered`, `selected`, `grabbed`. The den-test's view adds `hovered`, `sel_at`, `sel_uv` (where
the selection's middle is on the screen), `build`, `grip`; the addon's test actions add `gizmo click|press|moveto|
release|drag|turn`.

Tested here: `pick.rs` (the box hit, nearest, ties, behind the camera, too far, a box round the camera, turned boxes,
the drag's grip), `gizmo.rs` (click selects / adds / clears and the glows; the hover glow on and off and its 100 ms;
press-drag-let-go moves it with the grabbed spot under the cursor, on the grid, turned, one undo; Esc; locked;
visitors; the build view's pose and picking from it; `screen` is `ray`'s inverse), the headless client
(`the_mouse_hovers_clicks_and_drags_props_in_build_mode`), `test_wuui_gizmo.lua` (click vs drag thresholds, BUTTON1 only
over a prop and never changed while held, the lost let-go, Q / E and WASD only in build mode, the frame, the hint) and
den-check's `mouse_steps` (hover glow, click select, drag N yards, a picture each). **Only the real game proves**: that
BUTTON1 override-bound to a button really keeps the left-drag camera still and its down and up both arrive (else the
WorldFrame hook still selects and drags, but the camera turns too); that the glow shows on a prop that isn't selected;
how well the boxes fit (a building with no model box gets a small guessed one: pick it by its foot); the build view's
pitch (the camera calibration).

### Any gear, every mount (3 Nov, owner 10:40: "allow all mounts, all gear")

- **Gear** (`src/gear.rs`): the look only. The character's visible item fields (`PLAYER_VISIBLE_ITEM_n_ENTRYID`,
  field 283 + 2 n, every one of the 19 slots) in a values update, to you and to friends in the Den, kept in
  `<dens>/den-lite.gear.json` and in your create at the next login; the game draws each from its own `Item.dbc` by
  entry, as the ready-made outfit. Which slot an item goes in: the player's own `Item.dbc` (its inventory type),
  copied by the launcher. That table has no names (they are a server's), so `.den gear list <slot>` lists entries
  (and sends `wuden gear slot=..;page=..;pages=..;ids=..` for a picker; the game's own icons show them). The
  character list still shows the outfit. TODO(owner): which ready-made looks (armour sets) to offer.
- **Mounts** (`src/creatures.rs`, combat.rs): `.den mounts` puts every spell with the mounted aura (78) from your own
  `Spell.dbc` in your spellbook (`SMSG_INITIAL_SPELLS` again, with the class's), kept (`<dens>/den-lite.mounts`). Cast
  one: the aura, `UNIT_FIELD_MOUNTDISPLAYID` (69) with the creature's model and `UNIT_FLAG_MOUNT`, and the speeds its
  other auras give (32: run, 207: flight; per cent more) with `SMSG_FORCE_RUN_SPEED_CHANGE` /
  `SMSG_FORCE_FLIGHT_SPEED_CHANGE`; again, or `CMSG_CANCEL_MOUNT_AURA` / the buff right-clicked: off, the speeds back
  to normal. The creature -> model table is generated from AzerothCore's own world database
  (`creature_template_model`, `tools/gen_creature_models.py`, azerothcore-wotlk 5c5fe0b of 2 Nov 2026; 29947
  creatures, 105 KB), as `objects.rs` is from its gameobject_template. Mount names with a game brand word are left
  out. `.den mounts list` sends `wuden mounts page=..;pages=..;ids=..` for a list in the panel.
- Not built yet: the Play panel's gear picker and mount list (the server's answers above are ready for them).
- To see on the real game: the spellbook taking the mounts without a relog, the mount's model and animation, the
  speeds, flying mounts with `.den fly on`; gear in every slot (weapons sheathed, shields, tabards).

### Sandbox combat (3 Nov)

The owner (screenshot of three dummies that did nothing): "have training dummies be active, make all spells work, ...
allow class picker, any spell". Den Lite is not a game server with a combat engine; this is a sandbox: enough of the
game's own messages that your spells and swings land on the Dojo's dummies and the game shows them as a real fight.

**What works.**

- **Class picker** (`src/classes.rs`): `.den class mage 80` makes the ready-made character a mage at level 80 (any of
  the ten classes, levels 1-80), at once in the world: its fields (`UNIT_FIELD_BYTES_0`'s class and power, level,
  health, mana / rage / energy / runic power, attack time, damage, attack power) in an `SMSG_UPDATE_OBJECT` values
  update, `SMSG_INITIAL_SPELLS` with every spell the class learns up to that level, and `SMSG_ACTION_BUTTONS` with its
  main spells (the class's list by name, then other spells that hurt or heal). The spells: every ability of the
  class's skill lines in the player's own `SkillLineAbility.dbc` (class mask, the human race's mask), each at the
  highest rank the level allows (the table's next rank, else the same name's highest level), talents' abilities too
  (no talent points: the abilities only), plus the auto attack (and Auto Shot). What AzerothCore sends a level-80
  character is the same pair of messages, filled from its trainers and talents. The choice is kept in
  `<dens>/den-lite.class.json` with your own changes to the bar (`CMSG_SET_ACTION_BUTTON`), per class; at login the
  character list, the create and both messages are the class's. The character stays human (its look as before).
- **Targets**: `CMSG_SET_SELECTION` (u64). **Auto attack**: `CMSG_ATTACKSWING` (u64) -> `SMSG_ATTACKSTART` (u64
  attacker, u64 victim), a swing every attack time (`SMSG_ATTACKERSTATEUPDATE`: hit info, packed attacker and victim,
  damage, overkill, one physical part, victim state, two u32), out of reach `SMSG_ATTACKSWING_NOTINRANGE`, a friendly
  unit `SMSG_ATTACKSWING_CANT_ATTACK`; `CMSG_ATTACKSTOP` -> `SMSG_ATTACKSTOP`. Rage from swings.
- **Spells**: `CMSG_CAST_SPELL` (cast count, spell, flags, targets) -> checked (known, the global cooldown, its
  cooldown, power, a hostile target in range for a harmful spell) -> `SMSG_SPELL_START` (the cast time from
  `SpellCastTimes.dbc`, the power left) -> after the cast time `SMSG_SPELL_GO` (the units hit) -> the effects: damage
  (`SMSG_SPELLNONMELEEDAMAGELOG`), heals (`SMSG_SPELLHEALLOG`), the unit's health (`SMSG_UPDATE_OBJECT` values), the
  cooldown (`SMSG_SPELL_COOLDOWN`), auras (`SMSG_AURA_UPDATE` with the duration from `SpellDuration.dbc`: buffs on you,
  damage and healing over time ticking with `SMSG_PERIODICAURALOG`, a stance or form setting the stance bar's byte),
  combo points (`SMSG_UPDATE_COMBO_POINTS`, finishers use them), channelled spells (`MSG_CHANNEL_START` / `UPDATE`, the
  channel fields, ticks; a spell that triggers another each tick sends that spell's own `SMSG_SPELL_GO`, so its
  missiles fly), area spells (every hostile dummy in the radius from `SpellRadius.dbc`, around you or the place you
  aimed). Refused: `SMSG_CAST_FAILED` (not known 63, not ready 67, no power 85, out of range 97, friendly 115, bad
  targets 12, in progress 105). Moving (start forward, back, strafe, a jump) or `CMSG_CANCEL_CAST` stops a cast with
  a cast time: `SMSG_SPELL_FAILURE`, `SMSG_SPELL_FAILED_OTHER` (interrupted 40). `CMSG_CANCEL_AURA` takes a buff off.
  Power comes back every second (mana 3 %, energy 10, rage and runic power 3), health too when nothing hit you for 5 s.
- **The dummies count**: the training dummy has 5 million health and refills at a tenth; the meter's line goes to the
  Dojo panel (`wuden dps dps=..;total=..;secs=..;dummy=..`, once a second) and, 5 s after your last hit, to the chat
  ("12345 damage in 20.4 s: 605 per second."). Nothing hurts you unless a dummy hits back, and you never fall (back
  to full instead).

**What is approximate.** Talents, gear tiers, misses, armour, procs and pets: the next section (3 Nov, evening); no resistances,
parries or blocks; a 10 % critical chance without gear or talents (spells 1.5 times, swings 2 times). Damage: the spell's base points and dice from your own `Spell.dbc` with its points
per level, plus a spell power of 15 a level (casters; 5 others) times the cast time over 3.5 s; weapon strikes: a
weapon's damage by level plus attack power (30 a level for melee) over 14 per second of swing. Health and mana by
level (`classes::stats`). Effects other than damage, heals, auras, energize and combo points (summons, teleports,
dispels, pets, totems, crowd control) succeed visually and do nothing. Death knights' runes aren't spent. Spells that
need a weapon equipped (many warrior and rogue strikes, hunters' shots) may be refused by the game itself: Den Lite's
character carries no items.

**What needs the full Den** (real AzerothCore): real stats from items, every talent's own effect (spell modifiers),
pets with their own spells and talents, creatures that fight back for real, duels and fighting a friend, quests and
loot.

**The tables** (all the player's own, `src/spells.rs`): `Spell.dbc`, `SkillLineAbility.dbc`, `SpellCastTimes.dbc`,
`SpellDuration.dbc`, `SpellRadius.dbc`, checked against their 3.3.5a shapes. The launcher copies them from the player's
own game archives into the Dens folder at Den Lite's start, as `GameObjectDisplayInfo.dbc` (`launcher/src-tauri/src/
host/lite.rs` `game_tables`); never shipped. Without them `.den class` says so and only the auto attack works.

**Tested here**: unit tests on every message body and the formulas (combat.rs, classes.rs, spells.rs on small made-up
tables), and the headless client (`the_class_picker_and_the_dummies_take_spells_and_swings`): it picks mage 80 (the
fields, the spells, the bar), puts a dummy up, is refused an unknown spell, casts Fireball (start, 2.5 s, the go at the
dummy, the damage log, its health down), is interrupted by moving, swings and stops, resets (full again), and puts up
the pack and the wounded dummy.

**Only the real game can show**: whether `SMSG_INITIAL_SPELLS` sent again in the world replaces the spellbook (else the
new class's spells are there after logging out and in, which is always right); the class's own frames (runes, combo
points, the stance bar) after a class change without relogging; whether the game casts weapon strikes and shots with no
weapon; whether the dummies' neutral faction lets every spell target them; how the missiles, the boss's circle (a
ground-targeted fire spell's own look, `combat::BOSS_SPELL`) and the auras look; whether the spellbook shows the
spells without the class's skill lines in the player's fields.

### Specs, bars, gear sets and the test range (3 Nov, evening)

Owner (3 Nov): "spellbook placement loadout instant on bars, gear sets selector, full featureset for Dens to work and
better combat and test range dojo features". The whole list, what is next and the research: docs/DEN-FEATURES.md.

- **Specs and bars** (`src/specs.rs`, `classes.rs`): every class has its three specs (`.den class mage fire`,
  `.den spec frost`). A spec gets every bar filled at once (3.3.5a's button numbers): the rotation on 1-6 and area
  spells or big cooldowns on 7-12, cooldowns and defensives on the bottom left bar, utility (interrupts, crowd
  control, the auto attack) on the bottom right, mounts on the right bar, buffs, heals and every other spell on the
  second right bar and the main bar's second page; the stance and form bars (a warrior's three stances, a druid's
  cat, prowl, bear and moonkin, a rogue's stealth, a priest's shadowform) get the spec's own list. The four extra bars
  are switched on (PLAYER_FIELD_BYTES' third byte). TODO(owner): the layouts are a first pass from the usual builds.
- **Talents**: with the player's own `Talent.dbc` and `TalentTab.dbc` (the launcher now copies them, `host/lite.rs`
  GAME_TABLES), the spec's tree is filled top to bottom as a level-80 player would (abilities first, every rank, the
  tree's last talent at 51, the rest in the second tree; 5 points a row and every prerequisite kept), the talents'
  spells are learnt, the other trees' abilities left out, and the game's talent frame is told (SMSG_TALENTS_INFO, as
  AzerothCore's Player::BuildPlayerTalentsInfoData). Their passives count in the sandbox: damage and healing per
  cent, critical chance, procs (PROC_TRIGGER_SPELL: their own spell cast at the target, once a second at most).
  Spell modifiers (SPELL_AURA_ADD_PCT_MODIFIER and the like) don't.
- **Bar loadouts**: `.den bars save aoe`, `use`, `reset`, `delete`; up to ten per class and spec, kept in
  `den-lite.class.json` with the player's own button changes (now per class and spec).
- **Gear sets** (`src/gearsets.rs`): `.den gearset t7` puts the class's raid set of that tier on (T1-T10: found by name
  in the player's own `ItemSet.dbc`, the spec's version first: healer, caster, fighter or tank), the slots the set
  has no piece for and the weapons from the same era's items of the right kind (Item.dbc), at once for everyone in the
  Den. `.den gearset save <name>` keeps what you wear (`den-lite.gearsets.json`). The tier also counts in the numbers
  (`classes::geared`: health, power, spell or attack power, weapon damage, critical chance, hit: tier 7 and up is
  hit-capped; a tank's armour and avoidance).
- **Weapons are real items** (`src/pets.rs`): the main hand, off hand and ranged are created as item objects in the
  player's inventory slots, and the game's CMSG_ITEM_QUERY_SINGLE gets its answer ("Dojo Staff": Item.dbc has no
  names), so spells that need a weapon can be cast.
- **Better combat** (`src/combat.rs`): misses and dodges against higher levels only (spells 5, 6, 17 per cent; weapons
  5.5, 6, 8 and a 6.5 per cent dodge at +3; less your hit), glancing blows (auto attacks at +3), armour on physical
  hits (the heavy dummy and the boss: about two fifths off), the combat log's miss lines (SMSG_SPELLLOGMISS and the
  go's miss list), runic power from rune strikes, healing on its own meter (`dps` and `hps` in the panel's readout,
  `.den dojo meter reset`), your own heals counted.
- **Pets** (`src/pets.rs`, combat.rs): a summon your class has (Summon Imp, Call Pet, Raise Dead, Summon Water
  Elemental, Feral Spirit...) brings a pet: its create (summoned and created by you), its bar (SMSG_PET_SPELLS: Attack,
  Follow, Stay, its spell, Aggressive, Defensive, Passive, as AzerothCore's CharmInfo), it follows you, fights what
  you fight (casters cast their bolt), its damage counts with yours; its bar's buttons work (CMSG_PET_ACTION),
  Dismiss Pet sends it away. Spells with a reagent (a warlock's Soul Shard) are refused by the game itself.
- **The test range**: `.den dojo levels` (a dummy three levels below you without armour and a boss-level one with
  heavy armour and the skull), `group` (the healing range: five friends losing health, the first faster, a big hit
  on one now and then), `range` (a dummy 40 yards out, small markers named by their distance: 5, 10, 20, 30 yards),
  `demo [seconds] [kind]` (a practice run: your main bar cast for you at the dummy, cooldowns and damage over time
  first, ending with your number; the den-check's timed parse), `invulnerable on` (nothing hurts you). The boss now
  also casts at you every 15 seconds (3 s, a shadow bolt's look): kick it (any interrupt, silence or stun) or it hits
  for a quarter of your health; its result says how many you interrupted. Bests are kept per class and spec.
- **The Dojo panel** (WrathUnleashedUI `Dojo.lua`): two pages, Train (the old rows, the Range row: Levels, Group,
  Range, Run, Safe) and Loadout (class and spec pickers and Be, Mounts; your saved bars: Use, Save, Reset; gear: the
  tiers your game has and your sets: Wear, Save, Off; a name box for Save). Den Lite answers `wuden loadout
  class=..;spec=..;specs=..;bars=..;saved=..;set=..;tiers=..;mine=..`.

Tested here: unit tests (specs.rs: every class's three specs, the bars, a level-80 build on made-up talent trees, the
talents message byte for byte; gearsets.rs: tiers by role, the filled slots and weapons, saved sets; pets.rs: the
creates, the pet bar, the item answer field by field against the message crate's own reading; combat.rs: misses and
armour only against higher levels, procs once a second, the boss's cast kicked, healing meters and the healing range,
a pet that follows and fights, a practice run's number; classes.rs: loadouts, gear tiers), the headless clients
(`a_class_and_spec_fill_the_spellbook_talents_and_every_bar_and_loadouts_come_back`,
`a_gear_set_changes_every_visible_slot_and_its_weapons_are_real_items`,
`a_practice_run_has_a_number_the_boss_is_kicked_and_a_pet_comes_and_goes`), the Lua panel tests and the den-check's
Dojo steps (`dojo_features`). This container can't fetch crates: the std-side modules' tests ran here under plain
rustc with a stand-in JSON crate; CI's `cargo test --locked` must confirm the rest (sandbox.rs, world.rs hooks, the
headless clients).

**Only the real game can show**: whether the game takes the talent frame from SMSG_TALENTS_INFO without a relog,
shows the four extra bars from the field, fills the stance bars as it swaps forms; whether it accepts the item
objects as equipped weapons (Backstab, Auto Shot), the pet's create, frame and bar, and the boss's skull; how the
markers look (the dummy's model, small).

## How it works

| File | |
|---|---|
| `src/login.rs` | the login server: SRP6 (`wow_srp`), protocol 8 (`wow_login_messages`), the realm list |
| `src/world.rs` | the world server: auth session, character list, the login sequence, movement, chat commands, open.json, the companion's messages and her ticker |
| `src/den.rs` | the room plan (`ov_den.lua` `plan`) and its sizes (rings), stamps, undo, the save file and its checks (`OVBuild.DenCheck`, `host/dens.rs`) |
| `src/notice.rs` | the Den bar's toasts instead of chat lines: who has the bar, what waits for its hello, the old way after the wait, cheers at most one every three minutes |
| `src/companion.rs` | the companion: following, asks and lines, the `companion-in.json` / `companion-out.json` files |
| `src/play.rs` | `.den mode` (the built-in mode lists) and `.den mods` (the client's addon list, the launcher's `mods.json`) |
| `src/dojo.rs` | the Dojo: fly and speed (and the game's acks), the training ring and dummies, Jaina's build piece by piece (her casts and turns) |
| `src/gizmo.rs` | the editor's placement aids: the live preview (the build camera's ray, the copy's two-guid moves), the exact click (Flare's ground-target cast), the gizmo's flags, build mode's goober look and right-click selection; the calibration samples |
| `src/dojo.rs` | the Dojo: fly and speed (and the game's acks), the training ring and dummies (their kinds, the walker's circle), Jaina's build piece by piece (her casts and turns) |
| `src/spells.rs` | the player's own spell tables (Spell, SkillLineAbility, SpellCastTimes, SpellDuration, SpellRadius) |
| `src/classes.rs` | the class picker: the ten classes, their spells at a level, their bars, their numbers, the choice kept |
| `src/combat.rs` | the sandbox combat: casts, swings, damage and heals, auras, channels, cooldowns, the dummies' kinds, the meter, parses, the boss |
| `src/sandbox.rs` | world.rs's side of it (a child module): the game's combat messages in, the answers out, `.den class`, the Dojo's verbs, the bests, the tick |
| `src/flight.rs` | the companion in the air, around props and keeping up |
| `src/raw.rs` | the two messages written by hand (gameobject creates, the gameobject query answer); why is at the top of the file |
| `src/models.rs` | the props' model paths for the Builder's gallery, from the player's own `GameObjectDisplayInfo.dbc` that the launcher copies into the Dens folder (`host/lite.rs` `model_table`); none: icons |
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
   `SMSG_BINDPOINTUPDATE`, `SMSG_INITIAL_SPELLS` (none; the class's after `.den class`), `SMSG_ACTION_BUTTONS` (144
   empty; the class's bar), `SMSG_INITIALIZE_FACTIONS`
   (128), `SMSG_LOGIN_SETTIMESPEED`, `SMSG_UPDATE_OBJECT` (the player: create type 3, flags self + living +
   stationary position (0x61, AzerothCore's), the movement time from Den Lite's clock, speeds, and the fields for
   race/class/gender, health, level, faction 1, display 49 or 50, bounding radius, combat reach, cast speed, hover
   height, unit flags, player bytes, the visible items), then `SMSG_TIME_SYNC_REQ` (after the player, as AzerothCore),
   `SMSG_INIT_WORLD_STATES`
   (map 1, area 876, none), then the room and the props (`SMSG_UPDATE_OBJECT`, 40 gameobjects a message: create type
   2, flags 0x250 with position, guid counter and packed rotation; guid, type, entry, scale, display, flags, parent
   rotation (0, 0, 0, 1), bytes 1), and a system line.
5. Afterwards: movement packets (`MSG_MOVE_*`) keep the player's position; `CMSG_GAMEOBJECT_QUERY` ->
   the query answer (type, display, name, 24 data fields, size); `CMSG_NAME_QUERY`, `CMSG_PING`,
   `CMSG_UPDATE_ACCOUNT_DATA`, `CMSG_LOGOUT_REQUEST` (instant), `CMSG_MESSAGECHAT` (the commands);
   `CMSG_SET_ACTIVE_MOVER` -> `SMSG_CLIENT_CONTROL_UPDATE` (the game controls its character) and `SMSG_FORCE_MOVE_UNROOT`.
6. The companion: her create (`SMSG_UPDATE_OBJECT`, create type 2, object type unit, flags living, the unit fields
   above); `CMSG_CREATURE_QUERY` -> `SMSG_CREATURE_QUERY_RESPONSE`; `SMSG_MONSTER_MOVE` (her walks); `SMSG_MESSAGECHAT`
   monster say (her lines); `SMSG_DESTROY_OBJECT` when she goes. All through `wow_world_messages`.

### Verified: the test client, then the real game

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

That proves the server, the crates and the test client agree. **The real game** (the owner's den-test, 1 and 2 Nov)
then needed: the addon list read as the message crate hands it over, with none listed back when the game's key CRCs
aren't standard (else the game never asked for its characters, or called its combat-log addon Banned); movement
packets read by hand where the crate can't; an answer to the game's ask for its own settings (`src/accdata.rs`: no key
bindings meant W did nothing); and the launcher's white ground (docs/DEN.md, "The ground"; grass since 3 Nov). The unknowns as first
written down, before any real game connected:

1. Whether the client leaves the loading screen with this set of messages (no spells, no inventory items, no
   `SMSG_SET_PROFICIENCY` and friends), or wants something else first.
2. The player's create: only visible items, no item objects in the inventory fields; the character panel will be
   empty and the model should still be dressed (the client draws visible items from its own Item.dbc). Unknown:
   whether a missing field (power, stats, attack times) trips the UI.
3. The gameobjects: that the client shows them from the create alone, or only once it has its query answer (both are
   sent), and that 0x250 with the packed rotation is what it wants (AzerothCore's flags; the crate calls 0x10
   "high guid" after mangos, AzerothCore "low guid": same bytes).
4. The ground: Den Lite has no map files, so the floor is the fixed height 13.2 (or the launcher's ground file's
   heights) and stamps stand on your feet's height; the white blocks stand at their own size on the ground (the full
   Den sizes them from the host's dbc).
5. Movement is not checked (the client is trusted: fine for one player on their own PC); since 3 Nov it is watched
   for the log ("Standing and walking").
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

### Friends in your Den (owner 1 Nov 14:37: "invite a friend through the Leyline network ... no hassle")

The launcher's Your Den › **Invite friends** (app.rs `den_invite`):

1. starts Den Lite if it isn't running, and its **tunnel** (launcher `host/litenet.rs`): the same outbound wss to the
   relay the Host kit's agent keeps, signed with this Den's own key, **private** (only people who opened a link get
   in; the relay keeps sha256 of the invite secret) and **signup off**. Each connection the relay opens goes to Den
   Lite's **friends' listeners** (`Config::remote_ports`, `<dens>/den-lite.remote`), bytes untouched; the relay's
   welcome becomes `<dens>/den-lite.public`, the address in friends' realm list;
2. runs `den-lite guest add`: one **guest login per invite** (`src/guests.rs`: random account and password, printed
   once; `guests.json` keeps only the SRP6 salt and verifier, the character's name and guid, the expiry, 24 hours,
   and whether it was taken back);
3. copies the link `https://<relay>/join/<realm id>#<invite secret>~~<account>.<password>`. The friend pastes it
   in their launcher (Realms › Got an invite link?): `join_invite` knocks at the relay, points their game at it,
   fills in the account and hands the login to the hybrid login file for one start, so the game opens at
   character select on the friend's own ready-made character.

**Stop sharing** takes the tunnel down and every friend's login back; stopping Den Lite takes the tunnel down too.

The rules: the host's fixed local login is refused on the friends' listeners, from any non-loopback peer, and a
host session can't enter the friends' world listener. Friends see the host, the props and each other, their moves
and what they say; they build only after the host says `.den friends build on` (`.den friends`: who's here), and
never use the host's saves, files, companion or assistant.

Proved here: two headless clients in one Den (tests/client.rs), the guest command, and end to end on one PC
(hosting/tests/test_den_lite_tunnel.py): the real relay, the launcher's tunnel, the real Den Lite and the Host kit's
3.3.5a test client, which is cut off without the knock, refused as DEN through the relay, and enters the world with
its own login. **Two real PCs are needed** for: a real friend's game drawing the host and its moves, both behind
their own routers, the relay's own TLS name, the hybrid login on the friend's side, and the speed.

### More players: what is still simple

Everyone in the Den sees everyone (no visibility ranges: the Den is small), one character per login, no character
store (a guest's character is made fresh at each login, in its spot by the host's), no emotes or animations passed
on beyond movement and say, and the companion is the host's alone (friends don't see her). For a realm with more
than that, the full Docker Den is the way.

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

## Next steps

Done since the prototype: CI builds it for Windows (`den-lite.yml`, release `den-lite-latest`), the launcher gets it
on first use and passes `--client`, and the launcher signs in by itself (no password to show).

1. `.den home`, the stamp mark (stakes), and model props tested. Building from the assistant (`companion-place.json`).
2. Later: move to the workspace once stable; the browser client could connect too (out of scope here).

### Den anywhere (3 Nov, owner: "whole cities or even teleport anywhere")

`src/places.rs`. The player's own game has every map, so a Den can be anywhere: Den Lite only says where to stand
(numbers only: the named places are `game_tele`'s coordinates; no game data). `.den go` on the same map is a **near
teleport** (`MSG_MOVE_TELEPORT_ACK` with a counter; the game answers with the same opcode); to another map a **far**
one (`SMSG_TRANSFER_PENDING`, `SMSG_NEW_WORLD`; the game's loading screen, then `MSG_MOVE_WORLDPORT_ACK`), after which
everything is sent again in AzerothCore's `HandleMoveWorldportAck` order (`world.rs arrive_far`): bind point,
spells, factions, the clock, the player's own create (the same 0x61 flags and server-clock time as at login), the
time sync, the world states, then the Den's objects and dummies if the Den is on that map, and friends on it. The
watch of the game's moves starts again, so its `CMSG_SET_ACTIVE_MOVER` gets the control update and unroot once more.
Named places put you 2.5 yards above their height and the game falls onto its own ground (Den Lite has no
collision; the landing's `MSG_MOVE_FALL_LAND` gives the height, and `.den here` uses where you stand).

`.den here` moves the Den's spot (map, middle, floor); props are relative to it. The Den that was there is kept in
`<dens>/places/<m<map>_<x>_<y>>.den.json` and the one kept for the new spot opens. The autosave carries the place
(`place` in the file), so a restart opens the Den where it was; `.den load` of a file keeps the Den where it is.
Objects go only to games on the Den's map; the companion stays in the Den. The launcher's grass ground (the island
tile) is used only while the Den is on its map. City sizes (5-7) are open ground (no stone or wood floors).

To see on the real game: the far teleport's loading screen and the character walking after it (the den-test's walk
lines after `.den go stormwind`), and whether a named place's height lands the character on the ground everywhere.

### My models (3 Nov, owner: "drag and drop new 3d models")

`src/mymodels.rs`. The launcher's Your Den card takes the player's own `.m2` (with its `.skin` and `.blp` files) or a
building's `.wmo` (with its groups), checks them (launcher `host/denfiles.rs`: names, sizes, magic bytes, the format
this game reads), puts them under `World\Leyline\Den\<name>\` in the Den's patch with a row each in the player's
own `GameObjectDisplayInfo.dbc` there, and writes `<dens>/my-models.json`. Den Lite reads it at its start: entries
980001..., the generic type, the row's display, the Builder's "My models" tab. Nothing is uploaded: a friend's game
gets the model carrier as a stand-in in each one's place (their game hasn't the files or the rows).

### Friends: three ways in (3 Nov)

Besides the relay tunnel's listeners, Den Lite opens a **direct listener** for friends on this PC's network address
(`--friends-lan <ip>`, or when the launcher's Invite writes `<dens>/den-lite.direct-want`; ports 3724 and 8085, guests
only). Its realm list sends a friend on the same network to this PC's address and a friend from the internet to the
player's own address (`<dens>/den-lite.direct-host`, set in the launcher), and `<dens>/den-lite.direct` says whether
it is open. The launcher's link carries every way; the friend's launcher tries same network, own address, relay.
Player guide: docs/DEN-INVITE.md.
