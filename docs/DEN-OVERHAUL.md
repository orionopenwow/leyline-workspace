# Your Den: the editor overhaul (research and plan)

Owner, 3 Nov: "completely update the functionality of the den ... the UI doesn't look like Noggit or a gallery of
spinning 3D objects that I can place. It is a very unintuitive, bulky, two square-like UI. We need to do a den feature
list polish overhaul, AAA suite, pass, and research plan ... making sure that we have forge level features." Then a
reference screenshot of the look he wants for **both** the Den editor and Forge (section 2).

Status: plan, 3 Nov 2026. Nothing here is built. The "can't move, levitating" bug is being fixed separately and is
only listed here as a P0 dependency. Words: "the game" / "the 3.3.5a client"; no brand word anywhere a player looks
(docs/WORDS.md). Nothing here ships game data: every model path, texture and table is read from the player's (or
host's) **own** game folder on their own PC, as Den Lite, `packs.py` and Forge already do. Nothing from decompiled
clients.

Read first: docs/DEN.md (what is built), docs/NOGGIT-DEN-PLAN.md (Forge and the terrain delta), forge/docs/DEN-MODE.md.

---

## 0. One page

- **Today** the Den has seven separate windows (Den bar, Builder, Spawn menu, Properties, edit mode's three stacked
  bars and card, Den panel, Den home) holding about 80 text buttons. There is not one model picture. Placing on a
  player's own game means "at your feet, then x+ / y- nudges".
- **Target**: one editor shell, the same on screen in the game and in Forge:
  - a slim top toolbar;
  - a left tool rail;
  - a floating asset gallery of **spinning 3D model tiles** (the 3.3.5a client's own `Model` frame);
  - a one-line status pill;
  - a small properties popover in place of rows of buttons.
  - Gold on near-black.
- **The hard part is the stock client**: it has no world-click API and no ghost preview. Den Lite is our own server, so
  it can borrow three client behaviours:
  - a **ground-target spell** for click-to-place, with the game's own targeting circle;
  - a **clickable object type** for click-to-select;
  - a **temporary preview object** as the ghost.

  All three are "to verify" on a real client in phase 3. The browser client already has true clicks and handles (its
  engine hooks).
- **Forge** already has the features (it is Noggit Red). It needs the look:
  - a `theme.qss`. The `themes/` folder is missing from the tree and from Forge.zip today, so Forge runs plain Fusion;
  - the asset browser as a grid of previews (it already renders 90 x 90 previews);
  - a time-of-day slider;
  - the status pill.
- **Phases**:
  1. shell;
  2. gallery and catalog v2;
  3. place and select on the stock client;
  4. editing depth (undo, snap, time, stamps);
  5. Forge restyle;
  6. ground round trip;
  7. co-build, gamepad and touch.

  Each phase has tests (section 5).

---

## 1. What's wrong today (UX audit)

Files: `stream/fui-web/Interface/AddOns/WrathUnleashedUI/` `Builder.lua` (1182 lines), `EditMode.lua` (787),
`DenBar.lua` (529), `DenHome.lua` (680), `Style.lua`.

### 1.1 Too many windows, all text

| Window | Size | What's in it | Problem |
|---|---|---|---|
| Den bar (`DenBar.lua`) | 560 x 54, six 84 px buttons | Build (F7), Shape ground, Play (F8), Save, Invite, Help | Boxy, and it duplicates edit mode's Done, Save and Den. A seventh entry point to the same tools |
| Builder (`Builder.lua` `WIN_W, WIN_H = 520, 650`) | 520 x 650 | **36 buttons**: search, "Category: all" (one button that *cycles* through 17 categories), a 9-row text list (20 px rows, a stock spell icon per category, not per prop), Up / Down paging, Spawn / Near / Undo / Delete, Move `x -` `x +` `y -` `y +` `z -` `z +`, Step, Turn +15 / -15, Properties, Group row (5), Build row (Save / Load / Share), Prefab row (3), **Plot row (5: Claim, Info, Add, Visitor, Remove)**, Budget bar | The window the owner called "tall, bulky". Plot rows mean nothing in a Den. Reaching "furniture" takes up to 16 clicks. No picture of anything |
| Spawn menu (`Builder.lua` ~692) | 580 x 540 | tabs, search, a 3 x 12 grid of **text** cells (142 px buttons), Up / Down | The "gallery" is text buttons |
| Properties (`Builder.lua` ~895) | 340 x 440 | X Y Z Turn Pitch Roll Size, each `-` `+`, Step, Angle, 7 tool buttons, Apply, Reset | A form. A separate window from the Builder that it belongs to |
| Edit mode HUD (`EditMode.lua` `build()`) | banner 780 x 58, card 300 x 88, **three 900 x 40 bars stacked** at the bottom | 26 buttons: Props, Duplicate, Undo, Redo, Delete, Snap, Turn, Save build, Done / Spawn menu, Select mine, Box select, Copy, Paste, Clear selection, Properties, Save prefab / Group, Ungroup, Put away, Storage, Floor, Den | A third of the screen is buttons. "Save build" and "Save prefab" only *point at* the Builder window ("Type a name in the Builder's Build box") |
| Den panel, Den home, template shelf | (`EditMode.lua` `buildDen`, `DenHome.lua`) | invite, builders, layouts, look, stamps, grid, GM row, mail | The settings, the gallery (stamps) and the social tools are mixed together |

### 1.2 Specific UX faults

1. **No previews.** No `Model` frame anywhere in the addon (`grep SetModel` finds nothing). Every prop is a name.
   Many catalog names repeat: "Bench" x8, "Chair" x10 (`ov_build.lua` CATALOG keys `bench2..8`, `chair2..10`). Only
   a picture tells them apart.
2. **The catalog wire format has no model.** `wubuild catalog` sends `id,key,category,name,points;` (Builder.lua
   header, Den Lite `ovmod.rs catalog()`). The display id is only a comment in `ov_build.lua`. So the client can't
   show a model even if it wanted to.
3. **Den Lite's catalog is cut off.** It sends "as many as fit the client's 32 parts" (`ovmod.rs`), so on Den Lite
   the gallery silently ends early.
4. **Placing on a player's own game** is "at your feet", then nudges by Step (0.01 to 10 yards, cycled by a button).
   There is no click-to-place, no ghost and no click-to-select. Selection is `.build near` or "Select mine".
5. **Categories are a cycle button**, not a visible list. Paging is Up / Down buttons, not "1-20 of 53".
6. **Feedback is split** between the status line of whichever window is open, the Den bar's toasts, edit mode's note
   line and chat (`[Builder]` lines).
7. **Undo and redo show no counts.** The state carries `undo=<n>;redo=<n>` but only Redo's count is shown, in bar 3.
8. **Snap and angle are cycle buttons** ("Snap 1 yd", "Turn 15 deg"), not a dropdown and toggles.
9. **No time of day, no camera control.** DEN.md: "Time of day is left out" (true on AzerothCore with ALE; Den Lite
   owns its packets, see 3.5).
10. **Look**: `Style.lua` is the launcher's slate with a **frost-blue** accent (#3592cf). The reference is gold or
    amber on near-black, with small-caps labels. Button heights 22 to 26 px with 84 to 220 px widths read as "boxy".
11. **Ground and props are two worlds.** "Shape ground" leaves the game for Forge, and the change shows only after a
    game restart (patch-T). Nothing in game says what will happen, or brings you back to the same spot.
12. **Keys**: F7 / F8 / Ctrl+F7, Q held for the spawn menu, G carry, V duplicate. There is no tool-switch key
    (1-9) and no `[` `]` for brush size. Q is both "spawn menu" and "turn left".

---

## 2. Target design

### 2.1 The reference, mapped

| Reference element | Den (in game, WrathUnleashedUI) | Forge (Qt) |
|---|---|---|
| Top toolbar: mode icon + title ("LANDSCAPE" / "OBJECTS") | `DenShell.lua` top bar, 34 px high, full width, translucent | `QToolBar` top, restyled; mode title label |
| Undo n / Redo n | from the state's `undo=` / `redo=` | `ActionHistoryNavigator` counts |
| Menu button | the Den menu: Save, Save as, Open, Invite, Builders, Room look, Settings, Help | the Forge menu (File / Edit / View collapsed into one) |
| "Snap: off" dropdown | off / 0.5 / 1 / 2 / 5 yd (CVar `leylineEditGrid`, `.build grid`) | object snap + terrain snap |
| Three brush-curve buttons | for the scatter brush (P1): flat / smooth / linear falloff | terrain brush falloff types |
| Strength slider "1x" | scatter density (P1); in object mode, the nudge step | brush speed / strength |
| Rotate / angle toggles, 15 / 45 / 90 | angle snap (`leylineEditAngle`) | rotation snap |
| Time-of-day sun slider "15:00" + reset | Den Lite: game time (3.5); Docker Den: hidden | Forge's light time (today only keys N / B change the time speed) |
| `?`, Files, Export, X | `?` keys card; Files = Saved Dens; Export = Share (code / Workshop); X = leave edit (F7) | Help; Den files; export delta; close |
| Left tool rail | Select, **Ground** (hands off to Forge, 3.6), Paint (Forge), Foliage (scatter brush), Water (Forge), **Objects** (gallery), Move, Rotate, brush sizes 10 / 25 / 50 / 100 | `Toolbar.cpp` (already a left `QToolBar`) with our order and icons |
| "EVERYTHING" asset browser, bottom left | `DenGallery.lua`: category column, search, A-Z, spin toggle, colour chips, "1-20 of 53", 5 x 4 tiles of **spinning models** | `AssetBrowser` restyled to an icon grid (its `PreviewRenderer` already makes 90 x 90 previews) |
| Status pill | `DenShell` bottom-centre pill: tool · radius · strength · selection | `QStatusBar` restyled as a pill (its labels exist: position, selection, area, time, fps) |

### 2.2 Mockup (in game, 1280 x 720 UI)

```
+-----------------------------------------------------------------------------------------------------------+
| [#] OBJECTS   | Undo 4  Redo 0 | (=) | Snap: 1 yd v | (/)(~)(^) | Str [==o----] 1x | [R] 15 45 90 |       |
|               |                |     |              |           |                  |              |       |
|  (sun) ----o------ 15:00 (reset)                                              [?] [Files] [Export] [X]    |
+--+--------------------------------------------------------------------------------------------------------+
|> |                                                                                                        |
|^v|                                                                                                        |
|/ |                                              (the world)                                               |
|* |                                                                                                        |
|~ |                         +----------------+                                                            |
|##|  <- Objects (gold)      | Wooden Chair   |  <- properties popover, next to the selection              |
|--|                         | X 12.0  Y -3.5 |     (drag a number to scrub; Enter types)                  |
|<>|                         | Z  0.0  Turn 90|                                                            |
|()|                         | Size 1.0  Lock |                                                            |
|10|                         | [Dup] [Del] ...|                                                            |
|25|                         +----------------+                                                            |
|50|                                                                                                        |
|00|                                                                                                        |
+--+                                                                                                        |
| +--+---------------------------------------------------+                                                  |
| |* | EVERYTHING   [search.............] [A-Z] [spin]   |                                                  |
| |T | (o)(o)(o)(o)(o)                     1-20 of 53 <>|                                                  |
| |P | +------+ +------+ +------+ +------+ +------+      |                                                  |
| |R | | ~3D~ | | ~3D~ | | ~3D~ | | ~3D~ | | ~3D~ |      |                                                  |
| |F | |Anvil | |Target| |Barrel| |Bedrl | |Birch |      |                                                  |
| |B | +------+ +------+ +------+ +------+ +======+ <gold|                                                  |
| |I | ... 4 rows of 5 ...                                |                                                  |
| |S | Stamps | Recent | Favourites | Storage             |                                                  |
| +--+---------------------------------------------------+                                                  |
|                    ( Objects · Wooden Chair · snap 1 yd · 15° · 3 selected · room 412/1000 )              |
+-----------------------------------------------------------------------------------------------------------+
```

Rail, top to bottom: `>` select, `^v` raise / lower, `/` paint, `*` foliage, `~` water, `##` objects (active, gold),
`--` divider, `<>` move, `()` rotate, `10 25 50 100` brush sizes. The gallery's category column: `*` all, `T` trees and
plants, `P` props, `R` rocks, `F` furniture, `B` buildings, `I` lights, `S` structure (floors, walls, stairs). Then
tabs for Stamps, Recent, Favourites and Storage.

### 2.3 Look (both)

- **Panels**: near-black #0e0f12 at 88% opacity, a 1 px border #2a2d34, 4 px radius where the client allows (a
  `Backdrop` with an 8 px edge file of our own, made in the addon's `Media/`).
- **Accent**: gold #e0a83a; hover #f2c160; press #b8862a. Selected tiles and the active tool get a 2 px gold outline.
- **Text**: #e8e6e1, muted #8d8a84. Labels in small caps: our own font in `Fonts/` (Inter is already OFL in Forge; use
  the same for both).
- **Sizes**: rail 40 px, buttons 28 x 28 icon-only with a tooltip (name and key), top bar 34 px, tiles 88 x 104.
- **Icons**: one icon set for both, ours (drawn, or OFL Font Awesome Free glyphs exported to BLP for the addon and
  used as glyphs in Forge). Never stock spell icons for tools.
- **A new "editor" palette in `Style.lua`** (`S.Editor`). The launcher keeps its frost-blue. Owner's call in 6.1.

### 2.4 Interaction rules

- **One gallery click** picks a prop. The cursor becomes "place" (the targeting circle on the stock client, 3.2).
  **Click the ground** places it. **Shift+click** places another and keeps placing. **Esc or a right-click** stops.
- **Q / E** rotate by the angle snap while placing or with a selection (Q is no longer the spawn menu; the gallery is
  always one key away: **Tab**).
- **Click a prop** to select it. Shift+click adds. **Ctrl+A** selects mine. A drag box selects (browser client).
- **The properties popover** opens next to the selection: X Y Z, Turn, Pitch, Roll, Size; drag a number to scrub;
  Lock, Hide, Group, Duplicate, Delete as icons. The popover replaces both the x- / x+ rows and the Properties
  window.
- **Arrow keys** nudge relative to the camera by the snap (or 0.25 yd when snap is off). **PgUp / PgDn** move up and
  down. **Shift** gives 10x, **Alt** gives 0.1x.
- **Undo and redo**: Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z, with counts on the toolbar. Everything is one entry per user
  action, as today.
- **Feedback**: every realm answer goes to the status pill (and a toast for errors), never to chat.
- **Play (F8)** hides the whole shell. The shell is the Den bar's replacement: in Your Den it is on whenever edit
  mode is on (F7), and only a small "Build (F7)" chip shows when it is off.

### 2.5 Keyboard map (shown by `?`)

| Key | Action | Key | Action |
|---|---|---|---|
| F7 | edit mode on / off | F8 | play mode |
| 1-6 | rail tools (select, ground, paint, foliage, water, objects) | W / R | move / rotate tool (browser handles) |
| Tab | gallery open / close; typing goes to its search | / | search |
| Click | select / place | Shift+click | add to selection / keep placing |
| Q / E | rotate by angle snap | Arrows, PgUp / PgDn | nudge (Shift 10x, Alt 0.1x) |
| Ctrl+Z / Ctrl+Y | undo / redo | Ctrl+C / Ctrl+V / Ctrl+D | copy / paste / duplicate |
| Del | delete | Ctrl+G / Ctrl+Shift+G | group / ungroup |
| G | carry (browser) | End | drop to floor |
| `[` / `]` | brush size | Ctrl+S | save |
| H | hide selection | L | lock selection |

Bindings stay override bindings, set only in the Den and only in edit mode, and given back on leaving (as DenBar does
today). A key the player has bound to something else is left alone, and `?` shows the fallback.

### 2.6 Gamepad and touch (later, P2)

- **Gamepad** (the browser client's gamepad layer, if any): the left stick moves; the right stick turns the camera; A
  places; B cancels; the bumpers rotate; the triggers change category; the d-pad nudges.
- **Touch**: the touch-controls plan (Claude Doc `RotJG1ctmhZTouXrukoHGm`): tap a tile, tap the ground; the
  gallery as a bottom sheet; a two-finger twist rotates.

---

## 3. Research: what the stock client can and can't do

### 3.1 Model previews in the addon (the spinning tiles)

- The 3.3.5a client's `Model` frame (also `PlayerModel` and `DressUpModel`) has `SetModel(path)`, `SetFacing`,
  `SetPosition`, `SetModelScale`, `SetCamera`, `SetLight`, `SetFogColor`, `SetSequence` and more [S1].
- `SetModel` takes the file path with the `.mdx` ending, although the file in the archive ends in `.m2` [S2]. For
  example: `"Character\\NightElf\\Female\\NightElfFemale.mdx"` [S2]. To verify which ending the
  `GameObjectDisplayInfo.dbc` `ModelName` field holds in our players' files. The addon swaps `.m2` for `.mdx` either
  way.
- `SetModel` "seems to preload the specified mesh into some internal cache" [S2].
- A forum note says `SetModel` "tends to load the mesh, but not any skins" [S3]. Gameobject models take their
  textures from the model itself (no skin variations as creatures have), so most props should draw. **To verify per
  category**, with a fallback icon when a tile draws nothing.
- **Spinning**: one `OnUpdate` driver for the whole page turns each visible tile's `SetFacing` by about 0.6 rad/s.
  Only the hovered tile spins when the "spin" toggle is off. A tile's camera fits the model: `SetPosition` and
  `SetModelScale` from the display's bounding radius. Den Lite's `objects.rs` already has each object's size;
  `ov_build.lua` has `OVBuild.ModelBox` from the host's dbc.
- **Cost**: we found no published numbers for many `Model` frames on this client. Each frame is its own 3D draw.
  - Plan: 20 frames (5 x 4), reused across pages (never created per item), hidden frames cleared with `ClearModel()`.
  - Measure FPS with 20, 30 and 40 frames in phase 2 and pick the page size from that.
  - The editor palette is open only in edit mode, so world draw cost is already lower then.
- **The paths come from the player's own files.** The model path is in `GameObjectDisplayInfo.dbc`, which is game
  data. We never ship it. Sources by server:
  - **Docker Den**: `ov_build.lua` already reads the host's own dbc (`X.readDisplays`, `OVBuild.ModelBox`). It adds
    the path to a new reply.
  - **Den Lite**: the launcher reads the player's own `GameObjectDisplayInfo.dbc` with `mpq.rs` at Den start. It
    writes `<dens>/models.json` (display -> path, bounds) on the player's PC, and Den Lite reads it.
  - **Without either**: tiles show the category icon (today's look, smaller).
- **Wire**: `catalog` lines gain the display id (`id,key,cat,name,points,display`, version 2, the old reader ignores
  the extra field). Paths travel **on demand per page**:
  - `wubuild paths <id,id,...>` (up to 20) -> `wubuild paths i/n display=path;...`;
  - the addon caches them in `WrathUnleashedUIDB` per realm.

  This keeps every message well under the client's addon-message size, and fixes Den Lite's 32-part cut-off (also: a
  paged `catalog <page>`).

### 3.2 Placing with a click on a stock client

The client has no API for "where did the mouse hit the ground". What exists:

| Trick | How | Verdict |
|---|---|---|
| **Ground-target spell** | Den Lite gives the character a ground-targeted spell from the player's own `Spell.dbc`. Ground-targeted spells show the game's own targeting circle and send a cast with a destination. In build mode, Den Lite reads the destination from the cast packet, places the picked prop there, and never casts. A gallery tile is a `SecureActionButton` (`type = "spell"`): the click is a hardware event, and the tile's `PreClick` records the picked prop. Shift keeps the circle up for the next placement | **P0, the main path on Den Lite. To verify**: which spell id (one every class can be given, with a circle about 1-3 yards wide and no cast time); that the client lets an unlearned-in-class spell be cast once Den Lite sends it in `SMSG_INITIAL_SPELLS` (today empty, `world.rs`); out of combat only (always, in a Den). Docker Den: the same through ALE spell-cast events, if they expose the destination (to verify), else "at your feet" as today |
| **Temporary preview object** (the ghost) | While a prop is picked, Den Lite spawns that prop, unsaved, 3 yards ahead of the character, and moves it as the character walks or turns (as the stamp's stakes do, `ov_den_home.lua`, at most every 0.25 s). Enter places it there | **P0 fallback and preview**. It is a real model, not see-through. The stock client has no alpha per object |
| **Browser client's edit mode** | true clicks, handles, box select (engine hooks 1-11, DEN.md) | built; the shell drives the same `EditMode` calls |
| **At your feet + nudge** | today | kept as the last fallback |

### 3.3 Selecting with a click on a stock client

- The client makes a gameobject clickable by its **type**, which the server tells it in the object query answer.
- In build mode Den Lite answers the query for every Den prop as a usable type (a "goober", gear cursor), and treats
  the use packet (`CMSG_GAMEOBJ_USE`) as "select this prop" instead of using it.
- Chairs would no longer seat you in build mode; Play (F8) gives the real types back by re-sending the objects.
- **P0 on Den Lite; to verify** on a real client: the cursor, the range (the client limits use distance: we may need
  to grant a long reach by being close, or fall back to "select nearest under the crosshair"). Docker Den: not
  possible without core changes; keep "near" and Select mine.
- **Showing the selection**: there are no outlines on a stock client. Use a small marker object above the selection
  (the stamp stake model, or a light shaft from our catalog), plus the popover. The browser client draws real
  highlights.

### 3.4 What players expect (sandbox builders)

| Game | Pattern | What we take |
|---|---|---|
| Garry's Mod | the spawn menu: a tree of categories on the left, a grid of model icons, spawnlists that addons add, each entry a model path [S4] | the gallery's category column and grid; "addon lists" = Packs and Workshop items as categories |
| Fortnite Creative | prefabs (whole buildings) and galleries (themed sets of pieces); search by theme; a grid that snaps pieces together; UEFN lets you turn cell snap off and set a grid size [S5] | stamps = prefabs; "Galleries" = themed categories (Farm, Keep, Forest); snap off / sizes on the top bar |
| The Sims | free placement with Alt; F5 cycles half and quarter tile snapping; `[` `]` change brush size; eyedropper (E) [S6] | Alt = off-grid while placing; `[` `]` sizes; **eyedropper**: click a placed prop to pick the same one (P1) |
| Minecraft creative | a Search tab first, then category tabs; a hotbar the player arranges; middle-click picks the block you look at [S7] | search first; a **hotbar of 9 favourites** (keys 1-9 in object mode, P1); middle-click = eyedropper |
| Valheim (build mods) | a resizable grid with no row limit; tabs Most Used, Pinned, Recent, Can Build [S8] | Recent, Favourites (pinned), Most used; resizable gallery (P2) |
| The game's own newest housing | (DEN.md table: decor catalog, click to place, 15° turns, grid, budgets, layouts) | already matched in function; this plan matches it in feel |

### 3.5 Time of day on Den Lite

- The client takes the game time from the server's login time packet (`SMSG_LOGIN_SETTIMESPEED`), which Den Lite
  sends itself.
- Sending it again with another time should move the sun and sky at once. The Den's sky is our white sky in patch-T,
  so to verify how much it changes.
- **P1, Den Lite only, to verify**. On AzerothCore it stays "left out" (DEN.md: the core resends the real time).

### 3.6 The ground from inside the game

- The client reads map tiles from its archives at start. A changed tile (patch-T) shows only after a game restart, so
  live sculpting **in** the game is not possible on the stock client.
- The rail's Ground, Paint and Water tools therefore **hand off**:
  - they save where the camera is;
  - they open Forge on that spot (a new `--at <x,y,z>` flag);
  - when Forge closes, the launcher offers "Restart the game to see your ground", and logs back into the Den at the
    same spot.
- The browser client could reload a tile live (engine hook 12, new: reload a tile from the own-files chain).

---

## 4. Feature list ("Forge-level"), prioritized

**Where**: **Den** = in game (WrathUnleashedUI + Den Lite / ov_build.lua); **Forge** = the Qt editor; **both**.
**Stock** = works on a player's own unmodified 3.3.5a client (yes / trick / no). P0 = this overhaul's first release;
P1 = next; P2 = later.

### 4.1 Objects

| # | Feature | Pri | Where | Stock | Notes |
|---|---|---|---|---|---|
| O1 | Asset gallery with spinning 3D tiles, category column, search, A-Z, paging "1-20 of N" | P0 | both | yes (Model frames) | 3.1; Forge: AssetBrowser grid |
| O2 | Catalog v2: display id + lazy model paths from own files | P0 | Den | yes | 3.1; ov_build + Den Lite + launcher `models.json` |
| O3 | Click the ground to place, Shift to keep placing | P0 | both | trick (ground-target spell) | 3.2; browser: built |
| O4 | Click a prop to select, Shift-click to add | P0 | both | trick (Den Lite clickable type) | 3.3 |
| O5 | Properties popover (X Y Z, turn, pitch, roll, size, scrub, lock, hide, group, dup, delete) | P0 | Den | yes | replaces Properties window and nudge rows |
| O6 | Q / E rotate by angle snap; arrows / PgUp / PgDn nudge, camera-relative | P0 | both | yes (override bindings) | |
| O7 | Grid snap dropdown, angle 15 / 45 / 90 toggles | P0 | both | yes | CVars exist |
| O8 | Ghost preview (a temporary real object ahead of you) | P0 | Den | trick | 3.2; browser: hook 11 |
| O9 | Eyedropper (middle-click / E on a prop picks the same) | P1 | both | trick (via O4) | |
| O10 | Favourites hotbar 1-9, Recent, Most used | P1 | Den | yes | |
| O11 | Colour-swatch filter chips | P1 | both | yes | needs a colour tag per prop: computed once from the model's texture on the player's PC (launcher), or hand-tagged in the catalog |
| O12 | Multi-select ops: align, distribute, stack, measure, group | P1 | both | yes (built as commands) | move into the popover |
| O13 | Surface snap (walls, table tops) | P2 | Den (browser) | no | engine hook 7 |
| O14 | Gizmo handles: move, rotate, scale | P2 | Den (browser), Forge | no on stock | browser hooks 6 and 8; Forge has ImGuizmo |
| O15 | Any model from own files beyond GameObjectDisplayInfo | P2 | Forge (ground models) | n/a | NOGGIT-DEN-PLAN 2.4 |

### 4.2 Terrain, paint, water

| # | Feature | Pri | Where | Stock | Notes |
|---|---|---|---|---|---|
| T1 | Rail tools Ground / Paint / Water hand off to Forge at the camera's spot; restart-and-return | P0 | both | yes (restart) | 3.6; Forge `--at` |
| T2 | Raise / lower / smooth / flatten / ramp, brush sizes 10-100, falloff curves, strength | P0 | Forge | n/a | all in Noggit Red; restyle only |
| T3 | Texture paint (4 layers), swapper, own textures | P0 | Forge | n/a | |
| T4 | Holes, water (MH2O), vertex shading | P1 | Forge | n/a | delta v1 limits: one liquid layer (DEN-MODE.md) |
| T5 | Foliage scatter brush: N props in a radius, random turn and size, density | P1 | Den (props) + Forge (ground models) | yes | the recipe engine's garden-scatter, as a brush |
| T6 | Live tile reload | P2 | Den (browser) | no | hook 12 |

### 4.3 Lighting and time of day

| # | Feature | Pri | Where | Stock | Notes |
|---|---|---|---|---|---|
| L1 | Time-of-day slider + reset | P1 | both | trick (Den Lite only) | 3.5; Forge: a slider on its time |
| L2 | Weather (rain, snow) | built | Den (Docker) | yes | add to the menu |
| L3 | Light editor (sky colours) | P2 | Forge | needs patch-T + restart | Red's LightEditor exists |

### 4.4 Prefabs, stamps, recipes

| # | Feature | Pri | Where | Stock | Notes |
|---|---|---|---|---|---|
| P1a | Stamps as a gallery tab with preview tiles (the stamp's largest piece spinning) | P0 | Den | yes | stakes still mark the area |
| P1b | Save selection as prefab from the popover (one name field, no Builder window) | P1 | Den | yes | built as `.build prefab save` |
| P1c | Recipes (assistant-built) in the gallery | P2 | Den | yes | den.rs recipes |

### 4.5 Save, share, undo

| # | Feature | Pri | Where | Stock | Notes |
|---|---|---|---|---|---|
| S1 | Undo n / Redo n on the toolbar, Ctrl+Z / Ctrl+Y | P0 | both | yes | state has the counts |
| S2 | Autosave line in the status pill ("Saved 4 s ago") | P0 | Den | yes | Den bar has it today |
| S3 | Files (Saved Dens) and Export (code, Workshop) in the top bar | P1 | both | yes | built in the launcher; in game only `.den save` |
| S4 | One history across ground and props | P2 | both | n/a | delta versions + prop undo |

### 4.6 Co-build

| # | Feature | Pri | Where | Stock | Notes |
|---|---|---|---|---|---|
| C1 | Who is building (names in the pill), each builder's selection marker | P1 | Den (Docker) | yes | Den Lite is one player today |
| C2 | Den Lite friends building | P2 | Den | yes | Den Lite README lists what more players need |

### 4.7 Shell and polish

| # | Feature | Pri | Where | Stock | Notes |
|---|---|---|---|---|---|
| U1 | One shell: top bar + rail + gallery + pill; retire Den bar and the 3 HUD bars; Builder window kept only as "Advanced" (or removed) | P0 | Den | yes | |
| U2 | Editor palette (gold on near-black), our icon set | P0 | both | yes | |
| U3 | Walking and flying in the Den work (the levitation bug) | P0 | Den | yes | another agent's fix; a dependency |
| U4 | Build camera: fly on, farther zoom in edit mode | P1 | Den | yes | `.den gm fly`; zoom CVar to verify |
| U5 | All answers in the pill, none in chat | P0 | Den | yes | |
| U6 | Gamepad, touch | P2 | Den (browser) | n/a | 2.6 |

### 4.8 The top 10 P0s

1. **U1** One shell (top bar, rail, status pill), replacing the Den bar, edit mode's three bars and the Builder
   window.
2. **O1** The gallery of spinning 3D tiles.
3. **O2** Catalog v2 with model paths from the player's own files (and Den Lite's full catalog).
4. **O3** Click the ground to place (ground-target spell on Den Lite; browser: built).
5. **O4** Click a prop to select (Den Lite clickable type).
6. **O5** The properties popover, with **O6** keyboard nudges and Q / E rotation.
7. **O7 + S1** The snap dropdown, angle toggles, and Undo n / Redo n.
8. **O8** The ghost preview.
9. **U2 + T1/T2** The gold editor look in both, and the Ground / Paint / Water rail tools handing off to Forge at the
   spot.
10. **U3 + U5** Movement fixed (dependency) and every answer in the pill instead of chat.

---

## 5. Phased build plan

Each phase ends with tests in the repo's style:

- **Lua**: fake 3.3.5a API tests, `lua5.1 stream/tests/test_wuui_<name>.lua` (as `test_wuui_builder.lua`: fake frames,
  real round trips through `ov_build.lua` / `ov_modapi.lua`, and a brand-word check on every shown string). The fake
  API grows a `Model` frame type that records `SetModel`, `SetFacing` and `ClearModel`.
- **Rust**: `cargo test` in `labs/den-lite` and `launcher/src-tauri`. Run them only in `wt-theme`, because of the
  disk.
- **Launcher**: Playwright `node --test launcher/tests/<file>.browser.test.cjs`.
- **Forge**: CI (`forge.yml`) plus `tools/check_ui_words.py`.

### Phase 1: the shell (3-4 days)

- New `DenShell.lua`:
  - the top bar, the rail, the status pill, the `?` keys card;
  - the editor palette in `Style.lua` (`S.Editor`, `S.IconButton`);
  - icons in `Media/editor/` (our own BLPs).
- `EditMode.lua`: its HUD becomes the shell (the card and the three bars go; the banner becomes the top bar's mode
  title). The hooks the browser client calls are unchanged.
- `DenBar.lua`:
  - the bar goes;
  - toasts and autosave move to the pill;
  - its `wuden` protocol and key handling stay;
  - out of edit mode only a "Build (F7)" chip shows.
- `Builder.lua`:
  - the window stays, reachable from the menu as "Advanced (all commands)";
  - the Plot row is hidden in a Den;
  - Q is freed from the spawn menu (Tab instead).
- Tests:
  - `test_wuui_denshell.lua` (new):
    - F7 shows exactly one top bar, one rail and one pill, and no button wider than 120 px;
    - every rail and top-bar button sends the same command its old button did;
    - Undo / Redo counts follow `undo=` / `redo=`;
    - realm answers land in the pill, not chat;
    - F8 hides everything;
    - no brand words.
  - Update `test_wuui_denbar.lua`, `test_wuui_builder.lua` and `test_wuui_denquiet.lua` for the moves.
  - Screenshots: the browser client's own Playwright harness, if it can run the addon headless; else the owner's
    `den-test` Control step with a screenshot.

### Phase 2: the gallery and catalog v2 (4-5 days)

- `DenGallery.lua`:
  - 20 reused `Model` tiles, the spin driver, the category column, search, A-Z, "1-20 of N", the Stamps / Recent /
    Favourites / Storage tabs;
  - the gold outline on the picked tile; double-click places at the target (or at your feet).
- Wire:
  - `catalog` v2 (display field) and a paged `catalog <page>`;
  - `paths <ids>` -> `paths`.
- `ov_build.lua`: the display per catalog entry (CATALOG comments become data), paths from `X.readDisplays`.
- Den Lite (`ovmod.rs`): paging, `paths` from `<dens>/models.json`.
- Launcher (`host/lite.rs` or a new `host/denmodels.rs`): at Den start, read the player's own
  `GameObjectDisplayInfo.dbc` with `mpq.rs`, and write `models.json` with only the displays Den Lite knows
  (`objects.rs`). The file stays on the PC.
- Measure FPS with 20, 30 and 40 tiles on the owner's PC; fix the page size.
- Tests:
  - Lua (`test_wuui_gallery.lua`):
    - 20 Model frames created once and reused across pages;
    - `SetModel` gets `.mdx` paths;
    - hidden tiles call `ClearModel`;
    - search and categories filter;
    - paging text;
    - a tile without a path shows the icon;
    - paths are asked for once per page and cached.
  - `test_ov_build.lua`: catalog v2 fields, `paths` answers only known displays and only to builders, and stays
    within the size limit.
  - Rust (den-lite):
    - the paged catalog reaches every object;
    - `paths` reads `models.json` and refuses unknown ids;
    - a missing file gives "no paths" (not an error).
  - Rust (launcher): `models.json` from a synthetic dbc made by our own writer (no game data in tests).

### Phase 3: place and select on the stock client (5-7 days, Den Lite)

- **Ground-target placement** (3.2):
  - Den Lite: a build spell in `SMSG_INITIAL_SPELLS`; `CMSG_CAST_SPELL` with a destination places the picked prop
    there, answers a cast failure (silently) so nothing is cast, and keeps "placing" when Shift is held;
  - addon: gallery tiles as secure buttons, out of combat only.
- **Click-to-select** (3.3): the clickable object type in build mode, use = select, and the real types back on F8.
- **The ghost** (O8): a temporary object 3 yards ahead while a prop is picked; Enter places it.
- **The popover and nudges**: `DenProps.lua` (fields, scrub, icons), arrow / PgUp / PgDn override bindings in edit
  mode.
- Docker Den: ALE spell-cast destination if it exists (to verify); otherwise the ghost and "at your feet".
- Tests:
  - Rust (den-lite):
    - a cast packet with a destination places at that point (on the ground height from the terrain delta);
    - a cast outside the Den is refused;
    - the object query answers the clickable type only in build mode;
    - a use selects and doesn't seat;
    - the ghost moves at most 4 times a second and is never saved.
  - Lua (`test_wuui_denprops.lua`):
    - the popover fills from the state;
    - a scrub sends one `.build move` on release;
    - arrows send camera-relative nudges;
    - the bindings are released on leaving.
  - Real PC: a new `den-test` round (the owner's Control step) that places by a click and selects by a click. This is
    the "to verify" list for 3.2 and 3.3.

### Phase 4: editing depth (4 days)

- The snap dropdown and angle toggles (real dropdowns).
- Eyedropper, the 1-9 hotbar, Most used.
- The scatter brush (T5) with the top bar's curve buttons and strength.
- Time of day on Den Lite (L1).
- Stamps with previews (P1a); prefab save from the popover (P1b).
- Tests:
  - Lua: the dropdown sets the CVar and the realm's grid; the hotbar keeps 9 per character; the scatter sends one
    command with radius, count and seed.
  - Rust: scatter is deterministic per seed and inside the budgets; the time packet is sent on the slider's release
    and again at login.

### Phase 5: Forge restyle (4-5 days)

- `forge/themes/Leyline/theme.qss` (new) and a packaging line in `forge.yml` so `themes/` ships in Forge.zip. Make
  "Leyline" the default theme (`SettingsPanel.cpp` default "Dark" -> "Leyline", applied at start, not only when the
  setting changes).
- QSS covers:
  - `QMainWindow`, `QToolBar` (34 px top, 40 px left rail);
  - `QToolButton:checked` (gold outline);
  - `QDockWidget` (floating, translucent title);
  - `QStatusBar` (the pill: rounded, centred via a container widget);
  - `QSlider` (gold handle), `QComboBox`, `QLineEdit`, `QScrollBar`, `QTabBar`, `QListView` (`::item:selected` gold
    border).
- Code changes (all in `forge/src`, each listed in `CHANGES-FROM-NOGGIT-RED.md`):
  1. `Toolbar.cpp`: the rail order and icons as in 2.1; brush-size presets 10 / 25 / 50 / 100 as rail buttons.
  2. A top `QToolBar` built in `MapView.cpp` with:
     - the mode title;
     - Undo n / Redo n (from `ActionHistoryNavigator`);
     - the snap combo;
     - falloff buttons and the strength slider (bound to the active tool's existing settings);
     - the rotation snap toggles;
     - the time slider (sets the world's time instead of only N / B speed keys);
     - `?`, Files, Export, X.
  3. `AssetBrowser`: `listfileTree` -> a `QListView` in `IconMode` (90 x 90 previews already made by
     `PreviewRenderer`), a category column (Den picks: trees, rocks, plants, buildings, props, by path prefix), a
     search box, A-Z sort and paging. It docks bottom-left, floating.
  4. The status bar labels are joined into one pill text ("Raise / Lower · r 6 · strength 1.5 · falloff 4 · selection
     cleared").
  5. `--at <x,y,z>` (T1): open the camera there.
- Tests:
  - CI builds;
  - `check_ui_words.py` passes on the new strings;
  - a CI step checks that `themes/Leyline/theme.qss` is in Forge.zip;
  - a unit test (Qt Test, if the build has it; else a small CLI `--check-theme`) loads the QSS and checks that it
    parses.
  - The owner checks it by eye against the reference screenshot.

### Phase 6: the ground round trip (3 days)

- Rail Ground / Paint / Water -> `.den forge at <x y z>` -> the launcher opens Forge with `--at`.
- On close: "Restart the game to see your ground", Restart, log back into the Den at the same spot (Den Lite keeps
  the spot; `denclick.rs` signs in).
- Tests:
  - Playwright (`den.browser.test.cjs`): the Forge-closed banner, the Restart button calling the core's restart,
    "Later" keeping the game running;
  - Rust (launcher): `--at` is passed and checked (numbers only, inside the Den);
  - Lua: the rail's Ground tool sends `forge at` with the camera's spot and shows "Opening Forge" in the pill.

### Phase 7: co-build, gamepad, touch (P1/P2, later)

- C1 builder names and markers (Docker Den).
- Den Lite friends (C2).
- Gamepad and touch (2.6), with the touch-controls plan.

**Order and size**: 1 -> 2 -> 3 is the owner-visible overhaul (about 2.5 weeks). Phase 4 and phase 5 can run in
parallel with phase 3, as separate agents (Forge is a separate tree). Phase 6 comes after 5.

---

## 6. Open questions for the owner

1. **The accent**: gold for the Den editor and Forge only (the launcher stays frost-blue), or gold everywhere?
2. **The Builder window**: keep it as "Advanced (all commands)" from the menu, or remove it once the shell covers
   everything?
3. **The borrowed spell for click-to-place** (3.2): fine to give the character a ground-targeted spell in Your Den
   only (it is never cast; we only read where you clicked)? Its circle's look is the game's own.
4. **Click-to-select** makes chairs not seat you while editing (F8 gives them back). OK?
5. **The ghost** is a real, solid object (the client can't draw see-through objects). OK as the preview on your own
   game, with the true ghost only in the browser client?
6. **Docker Den parity**: Den Lite gets click-to-place, click-to-select and time of day first. Is the Docker Den (for
   friends) fine with "at your feet" plus the ghost until its engine side is done?
7. **The gallery's categories**: the reference shows all, trees, plants, rocks, food, buildings, items, tools. Our
   catalog has 17 (seating, tables, barrels ...). Merge into about 8 visual groups (2.2), or keep ours?
8. **Colour chips** need a colour per prop. Compute them on your PC from the model's texture, or hand-tag the
   catalog?
9. **Time of day**: Den Lite only to begin with. Should the white sky stay white at night, or follow the time?
10. **Names**: "Objects", "Ground", "Paint", "Foliage", "Water" on the rail, and the shell's mode titles in capitals as
    in the reference. TODO(owner) for any wording in your voice.

---

## Sources (read 3 Nov 2026)

- [S1] An addon-programming reference site, "PlayerModel" widget reference: the Model / PlayerModel methods (`SetModel`, `SetFacing`,
  `SetPosition`, `SetCamera`, `SetModelScale`, `SetLight`, `SetFogColor`, `SetSequence` ...). Link withheld here
  because the domain holds a brand word; it is in the session report.
- [S2] The game's API wiki and AddOn Studio, "API Model SetModel": `.mdx` ending, the preload
  cache, the example path. Links withheld for the same reason.
- [S3] An addon forum thread (showthread p=270882): `SetModel` loads the mesh, not skins; `SetUnit` memory notes.
- [S4] Garry's Mod wiki, `spawnmenu.AddPropCategory`: https://wiki.facepunch.com/gmod/spawnmenu.AddPropCategory
- [S5] Epic Games, "Using Prefabs and Galleries in Fortnite Creative":
  https://dev.epicgames.com/documentation/en-us/fortnite/using-prefabs-and-galleries-in-fortnite-creative
- [S6] SimsCommunity, "The Sims 4: Build Mode Tool Tips for Beginners":
  https://simscommunity.info/2016/06/16/the-sims-4-build-mode-tool-tips
- [S7] minecraft.how, "Creative inventory guide": https://minecraft.how/blog/post/creative-inventory-minecraft-guide
- [S8] Thunderstore, AAABuildMenu (Valheim): https://thunderstore.io/c/valheim/p/Azumatt/AAABuildMenu/
- [S9] Noggit basics: the 3.3.5a modding wiki's "Your First: Map Edit" (raise / lower with Shift / Ctrl + click,
  4 texture layers per chunk, the asset browser and Ctrl+V to place, the gizmo's move / rotate / scale, M2s scale but
  WMOs don't). Link in the session report.
- [S10] A modding forum thread on Noggit Red's rough edges (multi-object rotation, texture swapping): to check in Forge
  before the restyle.
- In this repo:
  - Forge's own source: `forge/src/noggit/tool_enums.hpp` (`editing_mode`: ground, flatten_blur, paint, holes,
    areaid, flags, water, mccv, object, minimap, stamp, light, scripting);
  - `Red/AssetBrowser` (`PreviewRenderer(90, 90)`);
  - `Red/ActionHistoryNavigator`, `Red/ViewportGizmo`, `Red/LightEditor`, `Red/StampMode`, `Red/BrushStack`;
  - `ui/SettingsPanel.cpp` (themes read from `./themes/<name>/theme.qss`; none in the tree);
  - `application.cpp` (Fusion style);
  - `MapView.cpp` (time speed only by N / B; the status bar labels).
