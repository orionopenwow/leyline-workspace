# Den places: go anywhere, and real places rebuilt in your Den

Owner, 4 Nov: "teleport anywhere from the Den, fast ... a full places list ... searchable; GM-style commands working
in the addon: a Teleports window with categories, search, favourites, recent, 'Back to my Den' always one
click/key", and "a 1:1 recreation of real places as Den builds, e.g. the Lion's Pride Inn (Goldshire) and arenas".

Two parts:

1. **Anywhere, fast**: 221 named places, `.go` and friends, the Teleports window, `.back`, `.den home`, a safe landing
   anywhere (Den Lite: `labs/den-lite/src/places.rs`, `world.rs`; the window: `WrathUnleashedUI/Teleports.lua`).
2. **Real places as Den builds**: an importer that runs on the player's PC and rebuilds a place from their own game
   files (`tools/den-places/den_places.py`, `places.json`).

Nothing from the game is shipped or committed. The game draws every place from the player's own files.

---

## 1. Anywhere, fast

### The list

`tools/den-places/teleports.json` is the one list: each place's name, title, category, other words, and where a
character stands there (map, x, y, z, facing, an area id when known). `python3 tools/den-places/gen_teleports.py`
writes it into:

- `labs/den-lite/src/placelist.rs`: `places::PLACES`, which places.rs includes;
- `WrathUnleashedUI/TeleportsData.lua`: names, titles and categories for the window. It has no coordinates: the window
  sends `.den go <name>`.

`--check` fails when either file is out of date, and the tests run it. The list is also checked for:

- names: 1 to 24 lower-case letters and digits, each used once;
- a known category;
- spots inside a map's 64 x 64 tiles, with a sensible height;
- no words that players never see.

| Category | What | How many |
|---|---|---|
| Cities | the eight capitals, Shattrath and Dalaran | 10 |
| Towns | towns, keeps and camps on every map | 56 |
| Starting areas | the races' first places | 8 |
| Zones | every zone of the four maps | 62 |
| Dungeons | each dungeon's door on its map | 42 |
| Raids | each raid's door | 21 |
| Battlegrounds | inside (the middle) | 4 |
| Arenas | Nagrand, Blade's Edge, Lordaeron, Dalaran's sewers, the Ring of Valor: their first start spot | 5 |
| Sights | the Gurubashi Arena, the Circle of Blood, the Ring of Trials, Nordrassil, the tram... | 12 |
| Your Den | the island | 1 |

**Where the numbers come from.** The numbers are where a character stands at each place:

- Most come from AzerothCore's `game_tele` table. That table is part of AzerothCore, which is GPL-2.0, and its numbers
  are right for this client.
- The arenas use the game's own start spots: the values in the client's WorldSafeLocs, as AzerothCore's
  `battleground_template` / `game_graveyard` name them.
- The cities the first list had keep their numbers, which were already checked in the game.

We took single facts (a map and five numbers for each named place), chose and named the places ourselves, and gave
them our own words and categories. We copied no table, no SQL and no code. As far as we know, coordinates in a game
world are facts and not anyone's creative work.

This is our reading, not legal advice. **TODO(owner): decide whether this is fine.** If it isn't, there are two ways
out, and both keep the commands and the window as they are:

- (a) the launcher reads the spots from the player's own game on their PC (WorldSafeLocs.dbc and AreaTrigger.dbc have
  most of the towns and every dungeon door);
- (b) the Single player host kit exports its own `game_tele` into `<dens>/places-extra.json`.

### The commands (Den Lite, the host)

| Command | What |
|---|---|
| `.go <place>`, `.tele <place>`, `.teleport`, `.tp`, `.den go <place>` | Go to a place. Its name, one of its words (`icc`, `sw`, `bea`), its title (`thunder bluff`, `Un'Goro Crater`), or the start of one (3 letters or more). A name that isn't a place says so and offers up to four that are close. |
| `.go <map> <x> <y> <z>`, `.go xyz <x> <y> <z> [map]`, `.go <x> <y> <z>` | A typed spot. The last form is on the map you are on. Commas pasted from a coordinates addon are fine. |
| `.back` (`.den back`, `.den go back`) | The spot before your last teleport. Ten are kept, and a second `.back` takes you to where you were before the first. |
| `.den home` (`.home`, `.den go den`) | Your Den, from any map, wherever the Den is now (`.den here` moves it). |
| `.den places` | One line per category. `.den places <word>` lists what a search finds: `.den places raid`, `.den places plague`. |

**Teleports.**

- **On the same map:** a near teleport (`MSG_MOVE_TELEPORT_ACK`). There is no loading screen.
- **To another map:** a far one: `SMSG_TRANSFER_PENDING`, then `SMSG_NEW_WORLD`, then the game's
  `MSG_MOVE_WORLDPORT_ACK`, then `arrive_far` sends everything again.

**Speed.** Den Lite adds no wait of its own. It sends a teleport at once and answers the game's acknowledgement at
once. After a far teleport it sends only what the game needs to enter the world, plus the Den's props when the Den is
on that map. Any loading screen is the game reading its own files.

**Flying and speed** come back after a far teleport, because a new world forgets them (`moves_again`). So `.den fly on`
and the free camera (the editor's snap out: its camera object is made again after the move, since `gizmo::clear` runs
on arrival) work anywhere.

### A safe landing anywhere

Den Lite has no map files. On its own Den it knows the floor. Anywhere else, only the game knows the ground, from the
player's own files. So:

- A named place puts you 2.5 yards above its spot, and a typed spot 0.5 yards above. The game falls onto its own
  ground, and Den Lite does no fall damage.
- After every teleport, a `places::Landing` watches the moves. Each move on the ground becomes "the last ground".
- A fall more than **400 yards** below the last ground means the character fell through the world: the typed height
  was inside a hill, or the ground isn't where the numbers say. No cliff in the game is that high, so a jump off one
  never counts. When it happens:
  - Den Lite lifts the character back over the last ground with flying on: 30 yards up, then 120, then 270. Each lift
    is a near teleport the game answers, so the character comes down on the real ground with X.
  - After three lifts it takes the character back (`.back`, else the Den).
- Flying down never counts as a fall.

### The Teleports window (`/tp`, `/teleports`)

`WrathUnleashedUI/Teleports.lua` is in the Leyline look (Style.lua). It is tested by `stream/tests/test_wuui_teleports.lua`
(40 checks).

- **Top row:**
  - a search box: any part of a name, best match first, and Enter goes to the first result;
  - **Back to my Den**, always there;
  - **Back**, which is off until Den Lite says where it goes.
- **Tabs:** Favourites, Recent (the last 12), the ten categories, and My Dens (the Dens you keep at other places).
- **Rows:** click a row to go there. The star keeps the place in Favourites. The mouse wheel scrolls.
- **Keys:**
  - **Shift-Home** is Back to my Den in Your Den's realm, unless you set your own key for it under Esc > Key Bindings >
    Leyline travel. There are bindings for Back and for the window as well (Bindings.xml).
  - Outside Your Den's realm the key is given back.
- **Slash commands:** `/tp stormwind` goes straight to the best match, and `/tp home` and `/tp back` also work.
- **Saved data:** Favourites and Recent are kept in `WrathUnleashedUIDB` (`teleFav`, `teleRecent`).

Each press sends one Den command over the addon channel (`wuden cmd`): `go <name>`, `home`, `back` or `places status`.
Den Lite answers with the trip and one line for the window:

```
wuden places   den=<your Den's place>;map=<the map you are on>;back=<where Back goes|->;kept=<a,b>
```

The line is sent when the window opens and after every arrival. Nothing the server sends is run.

### Live checks still to do (need the owner's game)

- [ ] `.go` to a dungeon's door, a raid's door and an arena (its own map): does the game load each one, and does a
  character stand on the ground?
- [ ] A typed spot inside a mountain: a lift, then the ground.
- [ ] Shift-Home from Northrend back to the island.

---

## 2. Real places rebuilt as Den builds

`tools/den-places/den_places.py` runs on the player's PC. It reads only their own game folder, through
`tools/den-check/mpqlite.py`.

### What it reads

1. **The archives**, in the client's order (the last one that has a file wins). `patch-T`, the Den's own patch, is never
   read.
2. **Map.dbc**: the map's folder name for the place's map id. The list's `dir` is used only when the table can't be
   read.
3. **The map.**
   - **The WDT:** which tiles exist. For a map that is a single building (an arena), the WDT's MODF names that
     building.
   - **The tiles (ADT)** that the place's box touches:
     - loose models (MDDF: 36 bytes each, a name through MMID/MMDX, a position, a rotation, a size / 1024);
     - buildings (MODF: 64 bytes, a name through MWID/MWMO, a doodad set);
     - the height chunks (MCNK, MCVT), for the middle's ground.
   - A building that stands on two tiles is in both, and is kept once (by its unique id).
4. **Each building's own root file (WMO)**: its doodad sets (MODS) and doodads (MODN, MODD). The importer uses the
   default set and the placement's own set, put where the building stands.
5. **GameObjectDisplayInfo.dbc**: which display shows each model file. `.m2`, `.mdx` and `.mdl` count as one model,
   and when several displays share a model the lowest is used.

### Frames (shared with leyline-terrain place.rs)

- A placement's world x is PLACE minus its z, its world y is PLACE minus its x, and its height is its y.
- Its facing in the world is its y rotation plus 180 degrees.
- A building's doodads sit in the building's own frame (x its front, y its left, z up). They are turned with the
  building and put where it stands.
- Pieces are kept in world directions, relative to the place's middle:
  - the place's own building, when one matches `focus`, at height 0;
  - otherwise the box's middle, on the ground there.

### What it writes (the player's Dens folder only)

- **`<dens>/<name>.den.json`:** the whole place as a Den.
  - Load it with `.den load <name>` (add `replace` or `add` when your Den has props).
  - Its room size is big enough to hold the place, and never smaller than 64. From that size a Den is open ground with
    no floor tiles or walls, so the place stands on the Den's ground.
- **`<dens>/stamps-local/<name>.den.json`:** the same place as a stamp, when it fits one. `.den copy <name>`, and Jaina
  builds it piece by piece.
- **`<dens>/place-models.json`**, with `--add-models`: see the next part.

The importer never writes over a file it didn't make (its files say `"source": "den-places"`). That includes the
launcher's own `stamps-local/lionspride.den.json` from `.den copy lionspride`.

### Models that have no display

The player's model table has displays only for the models the game uses as objects. A tree, a rock or a building
usually has none. Without `--add-models`, those pieces are left out and counted: the Den file's `skipped` lists them
by file name, and its `line` says how many.

With `--add-models`:

- Each such model gets a row of our own in `<dens>/place-models.json` (display ids 62001 to 69999, above the game's own
  and the player's dropped models' 60001 and up). Each row names only the file the player's game already has, plus its
  box (read from the M2's header or the WMO's MOHD).
- The launcher adds the rows to the Den's own patch table when the Den starts (`launcher/src-tauri/src/host/denmodels.rs`
  `place_rows`, beside the dropped models' rows). No file goes into the patch for them.
- A bad row is dropped, and a table that can't take the rows keeps the dropped models' rows.
- The game picks up the new rows **after a restart**, as with any change to the patch.

### The places (`tools/den-places/places.json`)

Each place has a name, a title, a map id, a box (x0, y0, x1, y1, in world yards) and optionally `focus` (words from the
file name of the place's own building) and `radius`. Nothing of the game's layout is in it.

| Name | What | Becomes |
|---|---|---|
| `lionspride` | the Lion's Pride Inn: the inn, its doodads, what stands within 24 yards | a stamp and a Den |
| `goldshire` | the village round the inn (200 x 200 yards) | a Den |
| `nagrandarena`, `bladesedgearena`, `ruinsoflordaeron`, `dalaransewers`, `ringofvalor` | the five arenas: each map's building (a WDT MODF, or the tile's MODF) and its doodads | a Den |
| `gurubashiarena`, `circleofblood`, `ringoftrials` | the open-air rings | a Den |

```
python3 tools/den-places/den_places.py --list
python3 tools/den-places/den_places.py --game "C:\Games\Wrath" --dens "<the Dens folder>" --place lionspride
python3 tools/den-places/den_places.py --game ... --dens ... --all --add-models
```

Then, in the Den:

1. `.den size city` (or let the file's room size do it).
2. `.den load goldshire replace`.
3. `.den fly on`, or the editor's snap out, to look round it.

### Limits

- **What can be shown.**
  - Any model that has a display row: the game's own, or ours with `--add-models`. Den Lite shows it through its model
    carrier (an entry of `MODEL_BASE + display`).
  - M2 doodads are shown as they are in the world.
  - **A building (WMO) as an object:** the game draws object displays that are buildings (its ships and Wintergrasp's
    keeps are such), but two things still need a live look:
    - whether a building shown as an object also draws its own doodad set (if it does, its doodads would show twice:
      leave `doodads` off for that place);
    - its lighting and collision. An object has no collision in Den Lite either way.
- **How big a place can be.**
  - As a Den: **800 pieces** (den.rs MAX_PROPS), within **256 yards** of the middle (a city-size Den's half width), and
    within the same height above or below. The file is at most 512 KB (it is written compact when it would be bigger).
    When a place has more, the building and the biggest pieces are kept first, and the line says how many were left
    out.
  - As a stamp: **400 pieces within 40 yards** (den.rs LOCAL_MAX, STAMP_SPAN).
  - A place's box is at most 512 yards across.
- **Pitch and roll** of loose models and doodads are not kept (almost every one stands upright). Their facing and size
  are kept.
- **Archives:** mpqlite opens stored, zlib and bzip2 files. An encrypted or imploded file is reported in "couldn't read",
  never guessed.
- **Not checked in the game yet:** the facing convention (the 180 degrees, shared with place.rs; a wrong one turns
  every piece, never just some). The tests check the arithmetic, not the game's own convention. **Live check:** load
  `lionspride` next to `.go goldshire` and compare.

### Tests

`python3 tools/den-places/test_den_places.py` (13 tests). Every game file in them is made up by the tests and written
into MPQ archives of our own (`write_mpq`, stored and zlib-sector files): a tile, a second tile with the same inn, a
building with two doodad sets, a map of one building, both tables, and an M2 header. The tests cover:

- the archive order (patch-T left out);
- every reader;
- tiles and ground heights;
- the inn 1:1 (positions, facings, sizes, the set, the radius, the duplicate placement, a stamp);
- a village as a Den;
- rows of our own, and the same rows again on a second run;
- an arena that is one building;
- never writing over the player's own file;
- the command line;
- no brand words;
- both lists.

The launcher's `denmodels.rs` has a Rust test for the rows (it can't be built in the cloud: no cargo there).
