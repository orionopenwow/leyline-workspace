# The Den's ground: Noggit Red, your companion, and Wrath Unleashed

Owner, 1 Nov 04:18: "complete and utter plan to incorporate this into the den ... every model for the den thing that
opens a white zone or template zone, and if Claude is dragged in easily with a Claude prompt, then Jaina appears and
talks and communicates with Claude as an agent, and everything is completely buildable. And Wrath Unleashed works as a
hangout and leveling server. People drag their files in..."

Owner, 1 Nov 04:26: "Dens begin as a Local thing with local files, a very small white space available for expansion,
then person can host to a friend and run around, and then in the future share their sole space with others thats
really it. its a place to test mods and all game modes locally and with friends."

Status: plan, 1 Nov 2026. Nothing below is built yet unless it says **built**. **Update, 1 Nov:** the editor is **Forge**
(owner: "integrate and mesh noggit into the den editor completely ... editor's name is Forge"). It lives in this repo's
`forge/` folder as its own GPL-3.0 program (not a separate fork repo), with the rename, the licence fixes, the Den
mode flags and the terrain delta writer of section 1.3 (**built**, not yet run on Windows): `forge/docs/DEN-MODE.md`,
`forge/CHANGES-FROM-NOGGIT-RED.md`, the delta schema `docs/terrain-delta.schema.json`, CI `tools/github-workflows/forge.yml`
(release `forge-latest`). One more licence finding: Noggit Red's icon font was Font Awesome *Pro* (not
redistributable); Forge ships Font Awesome Free instead. Words used here: "the editor" is our
renamed Noggit Red build; players never see a game brand word (docs/WORDS.md).

---

## One page

**What the player gets.** A Den is local first. The player's own game files make it. It starts as a small white pad
(about 66 x 66 yards) on the sealed island, and it can grow: one 33-yard square at a time, up to a full map tile
(533 x 533 yards), then the tiles around it. The player shapes the ground with **the editor**: raise, lower, smooth,
flatten, paint, holes, water, and any tree, rock or building model in their own game. They place props with the
Builder they already have (F7, `/builder`). Then a friend can visit, through Den Lite or the Docker Den. Sharing with
everyone (the Workshop) comes later.

**The editor is Noggit Red, kept as its own program.** Noggit Red is GPL-3.0. Our launcher is MIT. So we never link
it into the launcher. We keep a public fork, rename it, take out the game's name and two fonts we may not ship, add
three command-line flags, and build it in public CI. The launcher downloads that build, checks its hash, shows its
licence and its source link, and starts it with the Den's folder. This is allowed by the GPL, and it keeps our rules.

**Nothing from the game is shipped.** The editor writes a changed map tile on the player's PC. The launcher turns it
into a **terrain delta**: the player's own heights, paint, holes, water and model placements, as numbers and file
names. A visitor's launcher (or browser tab) rebuilds the tile from **their own** game files plus that delta. Our
server never hosts a Den's tiles.

**The assistant is a helper inside your own Den.** The player drags a ready prompt into Claude (or presses "Add for
me", **built**). On the first Den call the companion appears next to the player, says hello, and passes the player's
words to the assistant. New tools let the assistant shape the ground too (`den_terrain_plan`, `den_terrain_apply`),
always behind an Approve card, and always as a delta the player can undo.

**Wrath Unleashed stays the hangout and levelling realm.** No Dens on it. Turn on the social hub, events and duels;
keep bots off; switch the Builders' Realm off to free about 1 GB on the 7.6 GB box.

**First two steps, shippable in days:**

1. **Phase 1, 2 days.** The launcher's Den page gets "Shape the ground" (behind Settings > Testing). It runs the
   editor (the owner's own Noggit build for now) on a Den workspace, picks up the changed island tile, and writes it
   into the player's own `Data/patch-T.MPQ` and the Docker Den's server data. Smoke test: raise a hill, save, play,
   walk up the hill.
2. **Phase 2, 3 days.** The `leyline-terrain` crate (our own MIT Rust code): read and write a 3.3.5a map tile, make
   the white pad, make and apply a terrain delta. Den files carry the delta. Smoke test: a delta made on one PC
   rebuilds the same tile on another, and Den Lite stands props on the new ground.

**Licence verdict, short:** Noggit Red is "GPL3, as found in the COPYING file" (README; COPYING is the plain GNU GPL
version 3 text, no added terms; 623 source files carry "licensed under GNU General Public License (version 3)").
Shipping it as a separate, renamed, unmodified-in-spirit program with its full source on a public fork is fine.
Linking it, or pasting its code, into the MIT launcher, Den Lite or the browser client is not. Two files in its
`resources/` must not be shipped at all (Segoe UI fonts), the splash says "All rights reserved", and one bundled
library has no licence in the tree. Details in section 1.

---

## 0. What was read

- Noggit Red source (`noggit-red-master`, 2474 files), unpacked into a new folder and only read: README, COPYING,
  CMakeLists.txt, `src/noggit` (MPQ.cpp, MapTile.cpp, map_index.cpp, World.cpp, application.cpp, the `ui/` tools,
  `Red/` modules), `src/noggit/scripting`, `scripts/` (global.d.ts, the example brushes, docs), every folder under
  `src/external`, `resources/`, `media/`. No build script was run.
- Leyline: docs/DEN.md, docs/HYBRID-CLIENT.md, docs/GAME-DATA-ON-OUR-SERVER.md, docs/BUILD-REALM.md, docs/MODES.md,
  docs/leyline/agents.md, labs/den-lite (README, src), launcher/src-tauri/src/agents/den.rs and agents.rs,
  launcher/src-tauri/src/mpq.rs and clientimport.rs, realm/custom/lua_scripts/ov_den.lua, ov_build.lua,
  ov_den_companion.lua, hosting/kit/tools/wuhost/mpq.py, packs.py, denslice.py, realm/profiles/*.env.
- The owner's Spawn world editor README (`Wrldesdit/world-editor-package/README.md`): used for the keys and the
  copilot flow (section 3).

## 1. Licence and distribution

### 1.1 What each part is under (read in the source tree)

| Part | Licence (as found) | Ship it? |
|---|---|---|
| Noggit Red itself | GPL-3.0 (README, COPYING plain GPL v3, file headers "Noggit3 ... GPL version 3") | Yes, as a separate program, with source |
| `src/external/NodeEditor` | BSD-3-Clause (Dmitry Pinaev) | Yes, keep its LICENSE |
| `src/external/QtAdvancedDockingSystem` | LGPL-2.1 | Yes, keep its LICENSE |
| `src/external/libnoise` | LGPL-2.1 | Yes |
| `src/external/qt-color-widgets` | LGPL-3.0 | Yes |
| `src/external/qtgradienteditor` | Nokia header: "Technology Preview License ... may not be distributed" **or alternatively** LGPL-2.1 with the Nokia exception | Yes, under the LGPL-2.1 choice; add the LGPL-2.1 text (the `LICENSE.LGPL` it names is not in the tree) |
| `src/external/framelesshelper` | MIT | Yes |
| `src/external/imguizmo` | MIT | Yes |
| `src/external/qtimgui` (+ Dear ImGui) | ImGui MIT (`imgui/LICENSE.txt`); the qtimgui wrapper has no LICENSE in the tree | Yes after adding the wrapper's upstream licence (to check upstream) |
| `src/external/tsl` (robin-map) | MIT | Yes |
| `src/external/tracy` | BSD-3-Clause | Yes |
| `src/external/glm` | no licence file in the tree (upstream: MIT or the Happy Bunny licence) | Yes after adding upstream's copying.txt |
| `src/external/PNG2BLP` | no licence header on its own files; bundled libpng under the libpng licence | **To resolve**: find its author and licence, or drop it (it is used once: `World.cpp` minimap/texture export) |
| `src/external/imguipiemenu` | no header, no licence | **To resolve** or drop (one pie menu in `MapView.cpp`) |
| `src/external/dbcd-cpp` | empty git submodule (gitlab prophecy-rp/dbcd-cpp), licence unknown; `src/noggit` never includes it | Drop it from CMakeLists (to verify the build without it) |
| Fetched at build time | nlohmann_json (MIT), lodepng (zlib), FastNoise2 fork (MIT), sol2 fork (MIT); system: Qt 5 (LGPL-3), Boost (BSL-1.0), StormLib (MIT), Lua 5.1 (MIT), OpenGL | Yes; Qt as DLLs, with the LGPL notice and the Qt source offer |
| `resources/segoeui.ttf`, `segoeuisb.ttf` (in `resources.qrc`, so inside the exe) | Microsoft's Segoe UI: not free to redistribute | **No.** Replace with an OFL font (Inter or Noto Sans) |
| `media/splash.png` | "© 2020 Team Red. All rights reserved." | **No.** Our own splash |
| `media/noggit.ico`, `noggit_icon.png`, `resources/noggit_font.ttf`, `font_awesome.otf` | project art / icon font; Font Awesome is OFL | Replace the icon (rename anyway); keep the icon fonts |

### 1.2 What the GPL lets us do, and the choice

- **Run it, change it, rename it, ship it**, if every copy we ship comes with its Corresponding Source (GPL-3 §4-6),
  says it was changed and when (§5a), stays GPL-3 (§5c), and keeps its "Appropriate Legal Notices" (§0, §5d: the
  copyright, no warranty, the licence) in its interactive screen. The plain GPL-3 has no trademark or naming clause
  and no added §7 terms here, so a new name and icon are allowed. We keep the Noggit authors' names and a line
  "Built on Noggit Red, by the Noggit contributors, GPL-3.0" in our About box.
- **Link it into the launcher, Den Lite or the browser client: no.** Those are MIT. A program that links GPL code is
  one work under the GPL. Running the editor as its own process and talking through files and command-line flags is
  "mere aggregation" (two programs), which the GPL allows.
- **Port its code into our crates: no.** We write `leyline-terrain` from the public format notes (wowdev wiki's ADT
  and WDT pages) and our own maths. Ideas (a brush falloff, how an alpha map is laid out) are free; Noggit's code is
  not. If we ever do want a Noggit file in our code, that crate becomes GPL-3 and stays out of the launcher.
- **Lua scripts we write for the editor** (the job brush, 3.3) are our own files that the editor runs; we license them
  MIT and keep them in our repo.

**Verdict.** Option A, "a separate GPL program the launcher downloads and starts, with a source offer", is the one.

### 1.3 How we ship it

- **The fork.** A public GitHub repo, `<owner org>/den-terrain-editor` (name: owner's call; working name "Leyline
  Terrain"), forked from the Noggit Red source the owner attached, with these changes, each in its own commit and
  listed in `CHANGES-LEYLINE.md` (§5a):
  1. Name and look: `setApplicationName` / `setOrganizationName` ("Leyline Terrain" / "Leyline"; this also moves its
     settings away from any real Noggit install), window title, "&Noggit" menu, icon, splash.
  2. No game words a player sees (each found in the source): `About.ui` "WoW map editor for 3.3.5a" and "World of
     Warcraft is (C) Blizzard Entertainment"; `SettingsPanel.cpp` "WoW Client Path"; `MapCreationWizard.cpp`
     expansion names ("Wrath of the Lich King"); the map list's names come from the player's own Map.dbc, so the
     editor shows our names for the Den's maps (a small table: map 1 "The island" when opened from the Den). Code
     names (`BlizzardRandomizer`, archive names like `lichking.MPQ`) are not seen and stay.
  3. Fonts: Segoe UI out, an OFL font in (`resources.qrc`, `DefaultStyle.json`).
  4. Flags (today `main` reads no arguments at all, only `argv[0]`): `--game <dir>` (sets `project/game_path`),
     `--project <dir>` (sets `project/path`), `--map <id>` and `--tile <x>,<y>` (open there), `--scripts <dir>`,
     `--locked-tiles <file>` (only these tiles may be saved; others refuse with a line). This matters: **today, with no
     project path set, Noggit writes into the game folder itself** (`application.cpp`: `project/path` defaults to the
     game path). Ours always gets a Den workspace.
  5. Script writes: `write_file` asks once per file (`script_filesystem.cpp get_writable_path`); ours allows the Den
     workspace's `jobs/` folder without asking and still asks for anything else.
  6. Build: drop `dbcd-cpp`; resolve or drop PNG2BLP and the pie menu; `USE_SQL` stays off.
  7. Licence files: `COPYING`, every external's LICENSE (plus the missing ones above), `SOURCE.txt` (the fork's URL
     and the exact tag), `CHANGES-LEYLINE.md`, in the zip's root.
- **The build.** GitHub Actions on the fork (Windows, MSVC, Qt 5.15 LGPL, Boost, StormLib built from source),
  artefact `leyline-terrain-<tag>-win64.zip` and its sha256, attached to a release with the source tarball of the same
  tag and the Qt source link (or the Qt tarball itself) for the LGPL. Linux later.
- **The launcher.** A tool entry like the clients and Den Lite: `<app data>/Leyline/tools/terrain/<tag>/`. "Get the
  editor" shows one screen: what it is, "Free software under the GPL-3.0. Source:" (the link), Install. It downloads
  the pinned release, checks the sha256 pinned in the launcher's build, unpacks with clientimport's careful unzip
  rules (no `..`, no links, caps). Our server never hosts it: GitHub releases only.
- **Game data.** The editor reads the player's own archives with StormLib, read-only (MPQ.cpp opens with
  `STREAM_FLAG_READ_ONLY`). It never downloads anything. It reads 3.3.5a MPQs only (`application.cpp` archive list:
  common, expansion, lichking, patch, locale); there is no CASC in this version. That matches Dens (3.3.5a).

## 2. The Den's ground: from a small white pad to real terrain

### 2.1 Where, and how it grows

- The Den stays where it is: the sealed island, map 1, `(16226.2, 16257.0, 13.2)` (DEN.md, "The room"). Its tile is
  grid (1, 1): `Kalimdor_1_1.adt` (denslice.py's grid maths: gx = 31 - floor(16226.2 / 533.33) = 1, the same for y).
- **The white pad.** The first time the player opens "Shape the ground", `leyline-terrain` makes the Den's tile: the
  island tile from the player's own archives with a **pad** of 2 x 2 chunks (one chunk is 33.33 yards; the pad is
  66.7 yards a side) around the room flattened to the floor height and painted one light texture, the rest of the
  tile left as it is. That is the "very small white space".
- **Growing.** The Den keeps a list of open chunks (`terrain.open`, chunk indices). "Make the Den bigger" opens the
  ring of chunks around it: 4, then 16, 36 ... up to all 256 chunks of the tile (533 yards), then the 8 tiles around
  it (denslice already stages those 3 x 3 tiles for the server). Only open chunks can be changed: the editor's
  `--locked-tiles` and our importer refuse the rest. `ov_den.lua`'s edge (`OV_DEN_SIZE`, the half width) follows the
  open area, up to the tile.
- **A template zone.** Instead of the pad, start from a pack's land (`packs.py` recipes, **built**: goldshire is map
  0, tiles 49-50 x 31-32): `leyline-terrain relocate` copies that tile's heights, paint, water and placements onto the
  island tile, moving the absolute positions (MCNK positions, MDDF/MODF positions) by the offset. Made from the
  player's own files like every pack. A Den file then says `"terrain": {"base": "pack:goldshire"}`.
- **Bigger spaces later** (not in phases 1-6): a map of its own (a new Map.dbc row and WDT, the editor's Map Creation
  Wizard does this) needs a changed Map.dbc on the client and the server, built from each player's own dbc. Owner's
  call after phase 6.

### 2.2 Editing with the editor (what Noggit Red already has, read in the source)

| Tool | In Noggit Red (source) | In the Den |
|---|---|---|
| Sculpt: raise, lower, flatten, blur, ramp, vertex select | `ui/terrain_tool`, `FlattenTool`, `Brush`, node editor `Nodes/World/Terrain/*` (raise/lower, flatten, blur, orient, move vertices) | yes, open chunks only |
| Texture paint, up to 4 layers, swapper, palette | `ui/texturing_tool`, `texture_swapper`, `TexturePicker`, `alphamap.cpp`, `texture_set.cpp` | yes; textures are the player's own |
| Holes | `ui/hole_tool` (whole chunk with Alt) | yes |
| Water | `ui/Water`, `ChunkWater`, `TileWater`, `liquid_layer` (MH2O) | yes: a brook or a pond under the dock (DEN.md listed "no flowing water" as impossible; this makes it possible) |
| Vertex shading | `ui/shader_tool` (MCCV) | yes |
| Impassable flag, area id | `ui/ZoneIDBrowser`, script `chunk:set_impassable` / `set_area_id` | area stays 876 (the Den's area) |
| Object placement: any M2 or WMO of the client | `ui/object_palette`, `Red/AssetBrowser` (browses the archives), `ObjectEditor`, `ModelImport`, `Red/StampMode` | yes, as **ground models** (2.4) |
| Minimap, horizon, lights | `MinimapCreator`, `map_horizon`, `Red/LightEditor` | later |
| Scripting (Lua 5.1, brushes) | `src/noggit/scripting`, `scripts/global.d.ts` | the job brush (3.3) |
| Node editor (visual graphs) | `Red/NodeEditor`, 215 node files | available as is; not needed by us |

How it saves (read in the source): `MapTile::saveTile` writes the whole ADT (MHDR, MCIN, MTEX, MMDX, MMID, MWMO, MWID,
MDDF, MODF, MH2O, the 256 MCNK, MFBO) to `project/path`, as loose files under `World/Maps/<map>/`. The UID fix
(`todo_1.4.md`: "UIDs get recalced now and saved in this and all surrounding adts") can also write neighbouring
tiles: our importer takes only open tiles and says which others it ignored.

### 2.3 How the edited ground reaches each game

The editor writes `<den workspace>/World/Maps/Kalimdor/Kalimdor_1_1.adt`. The launcher's watcher (on the editor
closing, or a "Use my changes" button) runs `leyline-terrain diff` against the player's own original tile and saves
the **terrain delta** (`<den name>.terrain.json`, 2.5). Then, each from the player's **own** files plus the delta:

| Who | How the tile gets there | Built? |
|---|---|---|
| The player's own game (desktop) | `leyline-terrain build` makes the tile; the launcher's `mpq.rs` (**built**, `mpq::build`) writes `Data/patch-T.MPQ` with it, before each start of a Den, and takes it out when the player plays another realm (`hybrid.rs`'s prepare step does the same for `patch-Z.MPQ`). T is above packs' L and below the hybrid's Z (the client reads numbers, then letters, the last wins: HYBRID-CLIENT.md §1). The client reads its archives only at start, so a change needs a restart of the game | to build (phase 1) |
| The Docker Den (server heights, water, holes) | the tile goes to `state/packs/den-terrain/server/World/Maps/Kalimdor/Kalimdor_1_1.adt`, `packs/tiles.json` gets the tile, `packs/restage` is touched: the next start re-stages and map_extractor makes the `.map` (packs.py and denslice.py, **built**). Pathing stays off (no vmaps, `OV_PATHING=0`): players collide on their own client; the server only needs heights | to build (phase 1) |
| Den Lite | it has no map files today (the floor is fixed at 13.2, README risk 4). It reads the delta's heights (`leyline-terrain` as a dependency, MIT) to stand props and stamps on the ground | to build (phase 2) |
| A friend visiting (desktop) | the friend's launcher gets the Den file and its delta (phase 5), builds the same tile from the friend's own archives, writes their own `patch-T.MPQ` | phase 5 |
| A friend in the browser | own-files only: the browser client already reads the player's archives in the tab (`ownfiles*.js`, `chain.js`, `mpq.js`: GAME-DATA-ON-OUR-SERVER.md); `leyline-terrain` compiled to WASM applies the delta in the tab and hands the tile to the client's file chain as an overlay. **The Hetzner slice server never serves Den tiles**: they are made from game data, and the game-data killswitch closes `/data` anyway. The local `wenilla-host --patch` folder on the player's own PC may serve it | phase 6 |

### 2.4 Every model: props and ground models

There are two kinds of placed things, and the plan keeps both:

| | **Props** (server gameobjects, **built**) | **Ground models** (in the map tile, new) |
|---|---|---|
| What | `ov_build.lua` catalog, `.build find` (safe templates), `.build models` (any M2 row of the host's GameObjectDisplayInfo.dbc on the carrier template, entry 2000000000 + display) | any `.m2` or `.wmo` in the player's own archives (the editor's asset browser; MDDF/MODF entries) |
| Limit | only models that have a GameObjectDisplayInfo row (packs.py `layout_of` already drops the others and counts them as `unknown`); WMO rows are left out of `.build models` | none in the client; WMO doodad sets, scale (M2 only; WMOs can't scale) |
| Selectable in F7, undo, budgets, friends build | yes | no: edited in the editor or by `den_terrain_apply`; undo is the delta's history |
| Collision | client side by model; server: none | client side by model (the client collides with ADT models itself) |
| Saved in | `.den.json` `props` | the delta's `models` |
| Visitors see it | the server sends it | only once their tile is rebuilt (2.3) |

- **Moving between them.** "Bake into the ground" turns selected props into ground models (their model path from
  GameObjectDisplayInfo, same position): good for trees and rocks you never move. "Lift out" turns a ground model
  into a prop when its model has a display row (packs.py's `display_ids` lookup, **built**), else says why not.
- **More display rows** (any model as a selectable prop) means adding rows to GameObjectDisplayInfo.dbc on the server
  and every client. Possible from each player's own dbc plus a row list (names and ids only), like a pack. Owner's
  call; not in the phases.
- **Unique ids.** Ground models get ids from a high range (from 0x7F000000 up, kept in the delta) so they never meet
  the island's own ids or other Kalimdor tiles' (the client merges instances with the same id). To verify in game.

### 2.5 Formats

**The terrain delta** (`<name>.terrain.json`, format `leyline-terrain`, version 1). Data only, read by a strict parser,
never run; at most 4 MB.

```json
{
 "format": "leyline-terrain", "version": 1,
 "base": "own:1/1/1",               // map/gx/gy from the player's own archives; or "pack:goldshire", or "blank"
 "open": [119, 120, 135, 136],       // chunk indices that may differ from the base (0-255, 16 x 16)
 "pad": {"height": 13.2, "texture": "<path in the client>"},
 "chunks": [
  {"i": 119, "heights": "<base64 145 x f32 LE, absolute>", "normals": false,
   "layers": [{"tex": "<path>", "flags": 0, "alpha": "<base64 64x64 u8>"}],
   "holes": 0, "water": {"type": 1, "min": 12.0, "max": 12.0, "mask": "<base64 8 bytes>"},
   "colors": "<base64 145 x 4 u8>", "area": 876, "impassable": false}
 ],
 "models": [{"path": "<m2 or wmo path>", "uid": 2130706433, "x": 0, "y": 0, "z": 0, "rx": 0, "ry": 90, "rz": 0, "scale": 1.0}],
 "made": {"by": "editor", "at": "2026-11-02T10:00:00Z"}
}
```

(The `//` notes above are for reading here only; the real file is plain JSON.) Numbers and file names only, like packs' recipes: the delta never holds a byte of a game file. A chunk that is the
same as the base is not in the file. Positions in `models` are yards from the Den's middle (like `.den.json`), so a
template moves with the Den.

**The Den file** (`.den.json`, version 1, **built**) gets one optional key: `"terrain": {"file": "<name>.terrain.json",
"sha256": "<hex>"}`. Older readers ignore it (the same way `packs` was added). `.den save` keeps it; `.den load`
of a file with terrain the realm doesn't have tells the launcher to build it (the realm can't: tiles need a restart).

## 3. Your companion and your assistant, inside your own Den

What is **built** (den.rs, ov_den_companion.lua): `den_status`, `den_catalog`, `den_recipes`, `den_messages` (read);
`den_place_layout`, `den_say`, `den_recipe_save` (Approve card); the companion (world entry 32364, a temporary summon)
called with `.den jaina`, following, saying lines and build progress; the files `companion-in.json`,
`companion-out.json`, `companion-place.json`; "Add for me" for Claude Code and the Claude app (agents.md).

What is missing, and what we add:

| Gap | Fix | Where |
|---|---|---|
| The owner must type `.den jaina` | **Auto-come**: when the assistant's first Den call of a session arrives and the owner is in the Den, the launcher writes `"come": <id>` in companion-out.json; the realm summons her once per id and she says a hello line. `.den jaina away` still sends her off and stops auto-come until the next session | den.rs, ov_den_companion.lua |
| "Dragged in with a prompt" | The Den page's **Bring your assistant** card: a short ready prompt ("You are my Den helper. Use the leyline Den tools: read den_status and den_catalog, then wait for what I say to my companion with den_wait ...") that can be dragged or copied into Claude, next to "Add for me". If Leyline isn't connected yet, the card connects first | launcher/ui/den.js |
| The assistant has to poll | `den_wait` (read): waits up to 50 seconds for a new ask, then returns it (or "nothing yet"), so a Claude session can sit and listen | den.rs |
| Come / go from the assistant | `den_companion` (Ask): `come` or `away` | den.rs |
| The assistant can't touch the ground | `den_terrain_status` (read: open chunks, heights around a point, layers, water), `den_terrain_plan` (read: preview of ops, which chunks change), `den_terrain_apply` (Ask: the card shows the ops, the chunks, "the Den restarts your game to show it"); `den_terrain_grow` (Ask: open the next ring) | den.rs, `leyline-terrain` |
| Ground models from the assistant | ops `model add/remove/move` in `den_terrain_apply`, paths checked against the player's own archive list | same |
| The same ops for the human | the **job brush**: `scripts/leyline_job.lua` (ours, MIT) in the editor reads `jobs/next.txt` with `read_file`, and on one left click applies it with the scripting API (`select_origin`, `vert:set_height` / `add_height`, `tex:set_alpha`, `chunk:set_hole`, `vert:set_water`, `add_m2`, `add_wmo`: all in `global.d.ts`), then writes `jobs/done.txt`. The player then saves (Ctrl+S). Use: "Claude, put a pond here" while the editor is open | the fork's scripts folder |
| Den Lite has no companion | **Built** (1 Nov): `.den companion`, a unit create, follow by `SMSG_MONSTER_MOVE`, monster say, and `companion-in.json` / `companion-out.json` (den.rs uses Den Lite's folder while it runs); not `companion-place.json` yet (labs/den-lite README) | labs/den-lite (phase 4b) |

**The ops** (one list for the assistant and the job brush, checked in `leyline-terrain`): `raise|lower {x, y, radius,
amount, falloff}`, `flatten {x, y, radius, height}`, `smooth {x, y, radius, strength}`, `ramp {from, to, width}`,
`paint {x, y, radius, texture, strength}`, `hole {chunk|x,y, on}`, `water {x, y, radius, height, type}`, `model
add|move|remove`. Limits: open chunks only, heights within -50 to +200 yards of the floor, 64 ops a call, 4 MB delta.
Every apply writes a new delta version and keeps the old one: undo is "back to version N" (the card says so).

**Never** (unchanged from agents.md): the assistant never runs code in the editor or the realm, never reads game
files' bytes (only names), never makes anything public.

**The owner's Spawn editor as the UX guide.** Keys we copy in the Den where they fit: `[` / `]` brush size (the
editor's own keys differ: our fork maps them), T rotate and V scale (the browser F7 gizmo, hooks 6 and 8), U duplicate
(today V), F4 the dock. The copilot's "ghost preview, accept, reject": our Approve card shows a picture of the
change (a top-down height and paint preview from `den_terrain_plan`, rendered by `leyline-terrain` as a small PNG)
and, in the browser client, the ghost of each prop (hook 11, DEN.md).

## 4. Drag and drop: the player's own files

- **Built:** dropping a game folder, a zip of it or the player's own link on the launcher adds a client
  (`clientimport.rs`: careful unzip, caps, no torrents). Packs come from that folder (`leyline pack build`).
- **New on the Den page, "Drop files here":**
  - a game folder or zip: added as a client (clientimport), then "Make the Den from this game" (the pad);
  - a `.den.json` (and its `.terrain.json`): Add a Den file (**built** for `.den.json`; the terrain file is new);
  - a `.recipe.json` / `.recipe.lua`: into the building library (checked like `den_recipe_save`);
  - a single map tile (`.adt`) from the player's own editing: imported as the Den's tile **only** through `diff`
    (it becomes a delta; the tile itself is never copied anywhere else);
  - an `.m2`, `.blp` or `.wmo` the player made themselves: into the Den workspace's own folder, so the editor and
    `patch-T.MPQ` carry it (their own work; shown only to them unless they share the Den, and then it is their file
    to share; `wu-mod.py`'s rule still refuses game formats on our Workshop server).
- **Where files go:** only the player's own PC (the Den workspace, `state/`), and their own Den or hosted realm.
  Never our server. A friend gets a Den by the host sending the Den file (phase 5), and builds the tile from their own
  game.

## 5. Dens are local first, then a friend, then sharing

1. **Local.** Den Lite (no Docker) or the Docker Den on the player's PC, their own files, the pad. A place to test mods
   and game modes: the Docker Den can run any mode's ruleset on the Den's own realm (MODES.md templates: Classic
   levelling, Duel ladder, Hide and seek, Minigolf night ...) with the Den as the home; Den Lite is for building only
   (no creatures, combat or quests: its README).
2. **A friend.** The Docker Den through the relay with the invite link (**built**, DEN.md "Invite a friend"); Den Lite
   is one player only (its README lists what more players need). The friend's launcher builds the Den's tile from
   their own game (phase 5).
3. **Everyone, later.** The Workshop's `den` format (**built** for `.den.json`) gets the terrain file beside it.

## 6. Wrath Unleashed: the hangout and levelling realm (no Dens)

- **On:** the levelling rates of `main.env` (XP 3x, rest 2x, reputation 2x, **built**), `OV_EVENTS=1` with stock events
  and trivia, `OV_DUELS=1` and duel reset, autolearn, the social panel (`ov_social.lua`, `/lfriends`, launcher
  Friends: SOCIAL.md), the Leyline window's Den of the week (a link only).
- **Turn on:** `OV_QOL_HUB=1` (`.qol hub dalaran` and the other hubs; dalaran.env has it, main.env has it off) so
  people can meet in one place; a weekly hangout event in `ov_events.lua` (owner picks the night).
- **Keep off:** `OV_DEN`, `OV_BUILDREALM` and the Builders' Realm's second world server, playerbots
  (`AC_PLAYERBOTS_MAX_ADDED_BOTS=0` stays), boosts beyond the one-time kit, anything that loads extra map data.
- **Memory (7.6 GB box).** The Builders' Realm world server is capped at 1 GB (BUILD-REALM.md; the brief says 1.5 GB:
  to check with `build-realm-check`); switching it off frees that. The main world server, MariaDB, the portal and
  Caddy share the rest. **To measure** on the box (`build-realm-check` prints memory): the main world server's peak
  with 50 players in Dalaran; aim to keep 1.5 GB available. `server/server-report.txt` in the repo is an old report
  (7.6 GiB total, 1.5 GiB available, swap full) and must not be taken as today's numbers.

## 7. The code: crates, files, sizes

| Piece | Files | Licence | Size (estimate) |
|---|---|---|---|
| `labs/leyline-terrain` (new crate, no deps beyond serde and base64) | `adt.rs` (read and write a 3.3.5a ADT: MVER, MHDR, MCIN, MTEX, MMDX/MMID, MWMO/MWID, MDDF, MODF, MH2O, MCNK with MCVT, MCNR, MCLY, MCRF, MCSH, MCAL (2048 and 4096 forms), MCLQ passthrough, MCSE, MCCV; MFBO, MTXF), `delta.rs` (diff, apply, check), `pad.rs` (the pad, growing), `ops.rs` (the ops), `relocate.rs`, `preview.rs` (PNG), `wasm.rs` | MIT | about 2,500 lines + 800 lines of tests |
| Launcher | `host/terrain.rs` (workspace, watcher, patch-T), `tools/terrain_editor.rs` (download, hash, start with flags), `agents/den.rs` (+6 tools), `ui/den.js` (Shape the ground, Bring your assistant, Drop files here) | MIT | about 1,200 lines |
| Realm | `ov_den_companion.lua` (auto-come), `ov_den.lua` (edge follows open area; `terrain` key kept) | ours | about 150 lines |
| Kit | `packs.py`: a `den-terrain` pack from a delta (calls the launcher's CLI `leyline terrain build`, or a small Python twin for the kit) | ours | about 150 lines |
| Den Lite | heights from the delta; the companion (4b) | MIT | about 500 lines |
| Browser client | apply a delta in the own-files chain (the crate as WASM) | ours | phase 6 |
| The fork | the 7 changes in 1.3, `scripts/leyline_job.lua` | GPL-3 (fork), MIT (our script) | about 400 lines changed |

## 8. Phases, each with its smoke test

**Phase 1, 2 days: shape the ground, on your own PC (testing switch).**
Build: `host/terrain.rs` makes `<app data>/Leyline/den-terrain/<den>/` and starts an editor the player points to
(Settings > Testing > "Terrain editor program": for the owner, upstream Noggit Red set to that project folder by hand;
no download yet, nothing shipped). On close, every `.adt` under the workspace for an open tile goes into
`Data/patch-T.MPQ` (`mpq::build`), and for a Docker Den into `state/packs/den-terrain/server/...` + `tiles.json` +
`restage`. Hidden from players until phase 3.
Smoke test (owner, real PC): open map 1 at tile 1,1 in the editor, raise a 10-yard hill 20 yards south of the room,
paint it, save, close. Launcher: "Den ground: 1 tile". Start the Docker Den (re-stages) and the game: walk up the hill
(client); `.gps` on top says z about 10 above the floor (server heights). Remove patch-T: the hill is gone.
Unit tests: patch-T contains exactly the open tiles; others refused; the file goes away when another realm is played.

**Phase 2, 3 days: `leyline-terrain`, the pad and the delta.**
Build the crate (adt, delta, pad, ops), the `terrain` key in `.den.json` (launcher `host/dens.rs` checks it; ov_den
keeps it), Den Lite reads heights.
Tests, here, with no game data: a synthetic tile made by our own writer round-trips byte for byte; diff(base,
edited) then apply(base, delta) gives the edited tile; a delta with a closed chunk, a bad size or 5 MB is refused;
the pad is flat to 1 cm. Smoke test (real PC): make the pad from your own island tile, play: a flat light square
around the room; copy the Den file and delta to a second PC with its own game: the same tile (sha256 of the rebuilt
tile matches); in Den Lite a stamp placed on the hill stands on it.

**Phase 3, 1 week: the renamed editor, downloaded by the launcher.**
The fork with the 7 changes, CI, release; the launcher's Get the editor, hash check, licence screen, start with
`--game --project --map 1 --tile 1,1 --locked-tiles`. Smoke test: fresh PC, Get the editor, Shape the ground opens
on the island at the Den; no game word anywhere on screen (search the UI strings in CI for the brand list in den.rs
`BRAND_WORDS`); save of a closed tile says "only your Den's ground can be saved"; Help > About shows GPL-3, the source
link and the Noggit authors.

**Phase 4, 1 week: the assistant shapes the ground; the companion comes by herself.**
`den_wait`, `den_companion`, auto-come, the Bring your assistant card, `den_terrain_status|plan|apply|grow`, the
preview PNG, the job brush. 4b: the companion in Den Lite.
Smoke test: drag the prompt into Claude Code; in game the companion appears within 10 seconds and says hello; say
"Jaina, make a pond by the door"; Claude calls `den_terrain_plan`, the card shows a picture and "3 chunks"; Approve;
"restart your game to see it"; the pond is there; "Jaina, undo that"; it is gone.

**Phase 5, 1 week: host it for a friend.**
The Den file and its delta go to the friend: the host's launcher bundles them (Save a copy, one `.den.json` with the
delta inside up to 4 MB), the friend drops it on their launcher (or opens it from the invite page later), their
launcher builds `patch-T.MPQ` from their own game and joins with the invite. Smoke test: two PCs, two own games; the
friend sees the hill and the pond and stands on them; the friend's `patch-T` holds the same tile hash.

**Phase 6: browser visitors and the Workshop.**
The crate as WASM in the own-files chain; the Workshop's `den` items carry the delta. Smoke test: a browser tab with
the player's own folder dropped in visits the Den and sees the hill; our server's logs show no `/data` request for
the tile.

## 9. Verified and to verify

**Verified (in the source, here):** Noggit Red's licence and every bundled external's (1.1); it reads 3.3.5a MPQs
only, read-only, newest archive first, loose project files first (`MPQ.cpp`); it saves whole ADTs as loose files under
`project/path`, which defaults to the game folder (`application.cpp`); it parses no command-line flags; its Lua 5.1
scripting is brush events with the functions listed in `global.d.ts` and asks before writing a file; the strings that
name the game (1.3); the Segoe UI fonts and the "All rights reserved" splash; `dbcd-cpp` is an empty submodule that
`src/noggit` never includes. On our side: the Den tile is `Kalimdor_1_1`; packs.py and denslice.py already stage a
pack's server tiles and restage; `mpq.rs` builds archives; den.rs has the seven tools and no wait or terrain tool;
Den Lite has a fixed floor height and no creatures; WU main.env has the hub off and bots off.

**To verify:** the fork builds on Windows CI without dbcd-cpp, PNG2BLP or the pie menu; PNG2BLP's and the pie menu's
licences; the client loads `patch-T.MPQ` over the base tile and shows the edit; the extractor converts the edited
tile; ground model ids from 0x7F000000 don't clash; whether the pad's texture looks white enough (pick from the
player's own textures); the client reloads nothing without a restart (expected); Den Lite's companion messages
on a real client; WU's memory with the Builders' Realm off.

## 10. Owner's calls

1. The editor's name (working name "Leyline Terrain") and the fork's GitHub home.
2. Drop or replace the pie menu and the PNG2BLP export if their licences can't be found.
3. Whether a Den may later carry its own map (Map.dbc row) for spaces bigger than 3 x 3 tiles.
4. Whether more GameObjectDisplayInfo rows (any model as a selectable prop) are worth a client dbc patch.
5. Wrath Unleashed: the hub on, and the weekly hangout night.
