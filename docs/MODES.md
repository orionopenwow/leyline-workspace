# Leyline Modes: write your own minigame

> This page has two kinds of mode. **Minigames** (below) are Lua scripts that run inside a game, in the host's client.
> **Mode packages** ([at the end](#mode-packages-realms-anyone-can-host-and-fork)) are whole realms: a ruleset plus settings, hosted
> from the launcher and forked by anyone.


A **mode** is one Lua file. Open `/modes` in game:
- **Play solo** runs a mode on your own.
- **Host here** opens a game where you stand and invites your party.
- **Bumper Balls** and **Coin Dash** ship as built-in examples, and **Pressure Plates** is made only of devices.
- **Mode editor** (or `/modes edit`) writes modes in game: line numbers, a monospace face, keyword colours and an error console. See [The script editor](#the-script-editor) below.
- **Coin Collect** and **King of the Hill** are two short examples written only against the public API (`/modes collect`, `/modes hill`).
- **Device editor** (or `/modes devices`) builds a mode from parts with no code. See [Devices](#devices) below.

## How it runs

- **The host's client runs the script.** It runs in a sandbox: the safe parts of `string`, `math` and `table`, plus the API below. The sandbox has no game UI, no chat, no network and no way to load more code.
- **Everyone else only draws what the host sends** and sends back the keys the mode asked for. Joining a mode never runs its code on your client.
- **Each call into the script has a 30 ms budget** (loading it, each callback, each timer). A script that loops forever is stopped, only that mode ends, and the editor shows the line.
- **Limits:** 300 entities at most, and 14 KB of script.
- **Publishing:** published modes are reviewed by a GM before others can see them, but you can always host your own.

## The script

```lua
Mode.Title   = "Coin Dash"                       -- shown in /modes and on the banner
Mode.Keys    = { "W", "A", "S", "D", "SPACE" }   -- captured from every player while they play (Esc lets go)
Mode.Players = { 1, 8 }                          -- min, max

function OnStart(players) end      -- players = { "Ann", "Bo" }; build the arena here
function OnTick(dt) end            -- 20 times a second, dt = 0.05
function OnKey(player, key, down) end
function OnLeave(player) end
function OnJoin(player) end        -- someone joined a game that was already running
```

Allowed keys: W A S D Q E F R SPACE UP DOWN LEFT RIGHT 1 2 3 4.

## The API

The full, versioned list is **[CREATOR-API.md](CREATOR-API.md)** (also as a web page, `creator-api.html`). Both are made by `lua5.1 tools/creator-api/gen.lua` from `ModeAPI.lua`, the one table the sandbox is built from; the tests fail when they drift apart. The short version:

| Call | What it does |
|---|---|
| `Ent.Spawn{ shape = "box"\|"ball"\|"cyl", x, y, z, sx, sy, sz, yaw, color = "ff8800" }` → id | Adds a shape. Sizes are full widths in yards, and yaw is in degrees. |
| `Ent.Move(id, x, y, z [, yaw])` | Moves a shape. Players see it eased smoothly. |
| `Ent.Set(id, { color = "...", sx = 2, ... })` | Changes any field. |
| `Ent.Remove(id)` / `Ent.Get(id)` | Removes a shape, or reads its fields. |
| `Players.List()` / `Players.Count()` | The players in the game. |
| `HUD.Banner(text)` / `HUD.Say(text)` | Sets the top banner, or posts a line under it for everyone. |
| `Score.Set(p, n)` / `Score.Add(p, n)` / `Score.Get(p)` | The scoreboard on everyone's screen. |
| `Camera.Follow(player, id [, distance, height])` / `Camera.Top([height])` | Sets that player's camera, or a top-down view for everyone. |
| `Timer.After(seconds, fn)` | Runs `fn` later. |
| `Random(n)` | Returns 1..n from a per-game seed. |
| `Mode.End(text)` | Ends the game and shows everyone the text. |
| `print(...)` | Writes to the host's screen, for debugging. |

**Coordinates:** yards, in a frame placed where the host stood. +x is right, +y is up and +z is forward, so build around `0, 0, 0`.

Version 2 added `Ent.Clear`/`Ent.Count`, `Players.Has`, `HUD.Tell`/`HUD.Text`, `Score.Top`/`Score.Reset`, `Timer.Every`/`Timer.Cancel`, `State` (values with change events) and `Vec` (ground-plane helpers). Set `Mode.Api = 2` in a script that needs them; an older client then says so instead of running it.

## The script editor

`/modes edit` opens it.
- **Play** runs a copy of the script here, alone. **Play Here** asks the realm to host a copy where you stand and invite your party nearby. The realm checks it first and never lists or stores it. **Stop** ends either and brings the editor back. A test never changes your text or your saved draft.
- **Save** keeps the script in this browser and on your account (the realm's `mod_mode_drafts`). **Save while a test runs hot-reloads it**: the new script takes over in place, keeping `State` values and scores; the old shapes go and `OnStart` runs again. If the new script has an error, the old one keeps running.
- **The console** lists errors with their line (click one to jump there) and `print` output. **Check** looks for problems without running anything.
- **Publish** saves, then sends the script for GM review.

**Before anything runs**, the editor, the sandbox and the realm all read the script as text and refuse one that names a blocked global (`loadstring`, `setfenv`, `debug`, `os` and the rest), `string.dump`, or holds a compiled chunk, with the line. The realm never compiles or runs a script.

## Example

`Modes.lua` → `M.BUILTIN` is Bumper Balls, about 120 lines. It has:
- an arena and one ball per player
- WASD rolling and a Space dash
- ball-to-ball bumps
- rounds and scores
- a follow camera

## Publishing

1. Write the script and test it with **Play solo**.
2. Publish from `/modes`. It stays "in review" until a GM runs `.modes approve <key>`.
3. Once approved, it appears for everyone on the realm.

**GM commands:** `.modes pending`, `.modes check <key>` (the realm's check, the size and which parts of the API it uses), `.modes approve <key>`, `.modes reject <key>`.

## Devices

A **device mode** is built from parts, with no code. You place devices, set their properties, and wire them together. A **wire** says "when device A does *event*, call *function* on device B", with an optional argument.

**Pressure Plates** is the sample. Step on the three pads in order: the door opens, the timer stops and your time shows. Walk through the door to finish. Open it with **Sample** in the device editor, or play it with `/modes plates`.

### The device editor

Open it with `/modes devices`, or with **Device editor** in the mode editor. The layout is drawn in front of you while you edit.

- **Add:** pick a kind, then **At my spot** (where you stood when the editor opened) or **Click to place** (click the ground; right-click cancels). Points snap to half a yard.
- **Devices:** click one to select it. It turns white and gets a marker. **Delete**, **Move: click** and **Move to my spot** act on the selected device.
- **Properties:** the selected device's settings. Press Enter to apply one. On/off settings take `0` or `1`.
- **Wires:** pick the source device and its event, the target device and its function, and an optional argument, then **Add wire**. Each wire in the list has an **x** to delete it.
- **Play** tests the layout where the preview is. **Stop** (or **Leave**) brings you back to the editor. Play runs a copy, so nothing that happens in a test changes your layout.
- **Save** keeps the layout in this browser and on your account. **Open mine** goes through your saved layouts.
- **Publish** sends it for GM review, the same as a script. **As script** shows the layout as mode text.

### The devices

Every device has `name`, `x`, `y` and `z`. Coordinates are yards in the arena: +z is forward, and the floor is at `y = 0`.

| Device | Properties | Events | Functions |
|---|---|---|---|
| **Spawn Point** | `speed` 7, `cam` 9 (follow distance, 0 = none), `color` (blank = one per player) | `spawned` | `respawn`, `respawnall` |
| **Trigger Volume** | `w` 3, `d` 3, `h` 3, `on` 1, `once` 0, `show` 1, `color`, `offcolor`, `hitcolor` | `enter`, `leave`, `denied` (walked in while off) | `enable`, `disable`, `reset` |
| **Button** | `size` 1, `range` 2.5, `on` 1, `cooldown` 0.5, `color` | `pressed`, `denied` | `enable`, `disable`, `reset` |
| **Timer** | `seconds` 30, `dir` down/up, `auto` 0, `show` 1 (on the banner), `label` "Time" | `started`, `stopped`, `done` | `start`, `stop`, `reset`, `add` (seconds) |
| **Counter** | `start` 0, `target` 3, `show` 0, `label` | `changed`, `reached` | `add` (default 1), `sub`, `set`, `reset` |
| **Score Keeper** | `points` 1, `target` 0 (none) | `reached` | `add` (points), `sub`, `reset`, `win` (ends the game, text optional) |
| **Teleporter** | `r` 1.5, `tx`, `ty`, `tz` (the target), `on` 1, `show` 1, `color` | `teleported` | `enable`, `disable`, `send`, `reset` |
| **Round Manager** | `rounds` 1, `delay` 1, `wait` 3, `auto` 1, `endtext` "Game over" | `roundstart`, `roundend`, `gameover` | `start`, `endround` (text optional), `finish` (text optional) |
| **HUD Message** | `text` "Hello", `where` say/banner | `shown` | `show` (the argument replaces the text), `clear` |
| **Mover** | `shape` box/ball/cyl, `sx` `sy` `sz` 2, `yaw`, `color`, `path`, `speed` 3, `loop` once/loop/pingpong, `auto` 0, `solid` 1 | `arrived`, `returned` | `start`, `stop`, `back`, `reset` |

How they behave:
- **Players:** each player is a ball at a Spawn Point, rolled with WASD. With more players than spawn points, they share. A layout with a Button also takes E, which presses a button within `range`.
- **Who did it:** events remember the player who set them off. Score Keeper `add` gives the points to that player, and Teleporter `send` moves that player.
- **Text:** in HUD text, `endtext` and wire arguments, `{player}` is that player and `{6}` is device 6's value. The value is the time for a Timer, the number for a Counter, the round for a Round Manager, and the points for a Score Keeper.
- **The area:** `set area=` is half the arena's width (4 to 60 yards; the default is 20). Players and teleports stay inside it.
- **Mover paths** are points relative to where the mover starts, like `0,3,0/4,3,0` (up to 8). A mover with no path is a plain shape, such as a wall. A solid mover blocks players.
- **Rounds:** each round resets every device except Round Managers and Score Keepers, and puts the players back at their spawn. Without a Round Manager, the game starts once and ends when a Score Keeper's `win` runs.

### Limits and safety

- A layout has at most 64 devices and 128 wires.
- Wires run on the host, inside the mode sandbox, under the same 30 ms guard as scripts.
- At most 64 events go through wires each tick. If a wire loop makes more, the rest are dropped, the host sees a note, and the game keeps running.
- The layout is data, not code. The realm marks a mode that is only devices as `(devices)` in `.modes pending`. It is still reviewed like any mode.

### The layout as text

**As script** shows a layout like this:

```lua
Mode.Title = "Pressure Plates"
Mode.Players = { 1, 8 }
Devices.Run[=[
set area=14 floor=34384a
d 1 spawn name=Start z=-9
d 2 trigger name=Pad%201 x=-6 z=-2 once=1 color=ff5a3c
d 5 counter name=Pads%20done
w 2 enter 5 add
]=]
```

- `d <id> <kind> key=value ...` is a device. It lists only the settings that differ from the defaults. Spaces and symbols in values are written as `%20` and so on.
- `w <from> <event> <to> <function> [argument]` is a wire.

A script can use devices too. `Devices.Run` sets up `OnStart`, `OnTick`, `OnKey`, `OnLeave` and `OnJoin`. After it:
- `Devices.On(id, event, function(player) ... end)` runs your code on a device event.
- `Devices.Call(id, fn, arg, player)` calls a device's function.

## Mode packages: realms anyone can host and fork

A **mode package** is a realm mode: a ruleset to start from, the realm scripts it switches on, and a short form of
settings. The launcher's **Host** page opens on a grid of them. Pick one, name your realm, change its settings, and press
**Host**. The realm runs on your PC like any other realm, then shows **You're live** with the link to share.

**Fork** copies a mode into your Host pack as your own. Its card and your realm on the realm list then say
"based on *X* by *Y*". Fork a mode from its card, or from any realm on the Realms page that shows its mode.

### The templates

They live in `hosting/kit/modes/<id>/` and ship with the Host pack. Each one only switches on what the realm scripts
already do.

| Mode | Ruleset | Scripts | What its form sets |
|---|---|---|---|
| Classic levelling | main | `ov_main.lua`, `ov_qol.lua`, `ov_events.lua` | levelling and reputation speed, the one-time boost, events in chat |
| Solo leveling | solo | `ov_companions.lua`, `ov_qol.lua` | levelling speed, helper bots in your party |
| Duel ladder | brawl | `ov_duels.lua`, `ov_qol.lua` | the shortest duel that counts, rated duels per pair a day, points for a win, streak lines |
| Hide and seek | dalaran | `ov_events.lua` | how often a round runs, trivia, the world's own events, players needed |
| Minigolf night | dalaran | `ov_golf.lua`, `ov_modes.lua`, `ov_playmode.lua`, `ov_qol.lua` | hub teleports, duel reset |
| My Den | den | `ov_den.lua`, `ov_build.lua` | the room's look, the building budget |
| Survivors | (none) | (none) | not a realm mode: Azeroth Survivors runs in the browser at its own address (`docs/SURVIVORS.md`); coming soon until the owner turns it on |

### The folder

```
mode.json          what the mode is and how a realm runs it
picture.svg        its card picture (or picture.webp / picture.png), optional
<name>.den.json    a Den file for its map (den ruleset only), optional
README.md          a few lines for people who fork it, optional
```

`mode.json`:

```json
{
 "format": "leyline-mode", "schema": 1,
 "id": "duel-ladder", "name": "Duel ladder", "line": "Everyone at the top level. Duels count toward a rating.",
 "picture": "picture.svg", "version": "1.0", "author": "Leyline",
 "based_on": null,
 "status": "live",
 "profile": "brawl",
 "scripts": ["ov_duels.lua", "ov_qol.lua"],
 "env": {"OV_DUELS": "1", "OV_DUEL_RESET": "1"},
 "mods": [],
 "den": null,
 "tags": ["pvp"],
 "jam": null,
 "settings": [
  {"key": "min_s", "label": "Shortest duel that counts", "type": "number", "min": 5, "max": 120, "default": 15,
   "unit": "seconds", "env": "OV_DUELS_MIN_S"},
  {"key": "streaks", "label": "Win streak lines", "type": "toggle", "default": true, "env": "OV_DUELS_STREAKS"},
  {"key": "look", "label": "The room's look", "type": "choice", "default": "white", "env": "OV_DEN_THEME",
   "options": [{"value": "white", "label": "White"}, {"value": "wood", "label": "Wood"}]}
 ]
}
```

- **id**: a-z, 0-9 and single dashes, 2 to 32 long. **name** up to 32, **line** up to 120, **author** up to 32.
- **based_on**: `null`, or `{"id", "author"}` of the mode it was forked from. Fork sets it.
- **status**: `live`, or `coming` (shown, never hosted or forked).
- **profile**: the ruleset it starts from: main, brawl, progression, casual, dalaran, vanillaplus, den or solo.
- **scripts**: realm scripts by file name. Each one must ship in the Host pack. A package never carries a script of its own.
- **env**: fixed settings. Only the realm scripts' `OV_` switches and the `AC_RATE_`, `AC_SKILL_GAIN_`, `AC_START_` and
  `AC_AUTO_BALANCE_` settings. Nothing about the server itself: no databases, files, folders, ports, addresses, keys,
  the realm's name or tests. Values are letters, digits, `.`, `_` or `-`.
- **mods**: client mods by their realm list id (`mod:<id>`, `up:<slug>`, `mode:<id>`). They join the realm's mod list.
- **den**: a Den file in the folder. When you host the mode, it opens in your Den.
- **settings**: at most 12 fields. Each sets its `env` name (or a list of up to 6) to the host's answer. A toggle sets
  `1` or `0`. A field never sets a name that **env** already sets.
- **jam**: the id of the mode jam it is entered in.

### The checks

`python3 -I stream/modepack.py --check <folder>` checks a package. The launcher, the host kit and the realm list check it
again with the same rules:

- **Sizes:** `mode.json` 16 KB, a picture 256 KB, a Den file 512 KB, `README.md` 16 KB, the folder 1 MB and 6 files.
- **Paths:** only the file names above. No folders, hidden files or links.
- **Pictures:** an SVG has no scripts, event attributes or outside links.
- **Words:** the name, line, author and labels go through the one blocklist (`stream/namecheck.py`). Game and publisher
  names are refused. The team's name is kept for the templates.

### How a mode reaches the realm

1. **Host** checks your answers and writes `state/mode.json` in the Host pack (`launcher/src-tauri/src/host/modes.rs`).
2. The host kit's `setup.py` (`read_mode`) puts the mode's env in `state/realm.env`, before your own Realm tools settings,
   so those still win. It also puts the mode's credit and recipe on your realm's listing and adds its mods to the mod list.
3. The relay lists `mode` on your realm (`hosting/relay/wurelay/listing.py` `clean_mode`). The whole recipe is listed
   while it fits in 4 KB. Otherwise only the credit is listed, and the realm can't be forked from the list.

Picking a plain ruleset with **Pick a ruleset yourself** removes `state/mode.json`.

### On the website

- `/wrath/modes` has a **Host a mode** section: every template, with the live realms running it.
- `/wrath/modes/<id>` shows a mode: what it sets, the realms running it now, and its forks. A fork that a listed realm
  runs has a page too.
- `/wrath/modes/jam` is the **mode jam**: its theme, how to enter, and the entries. The entries are realms on the list
  running a mode whose `jam` is the jam's id. Staff picks are marked. It is for bragging rights only, with no prizes.
  Staff set the jam (id, theme, line, end date, picks) in the `jam` block of `stream/modepacks.json`.
- `/wrath/modes/hosted.json` lists, per template, the realms running it, the players on them and its forks.

`stream/modepacks.json` is made from the templates with `python3 -I stream/modepack.py --write`. The launcher builds it
in, and the site serves it. `--verify` fails when it is out of date.

