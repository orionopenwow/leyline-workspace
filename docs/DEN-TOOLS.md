# Den builder tools

Grand release brief, 4 Nov 2026: "make the Den builder's tools complete and top tier": props you can stand on and
walk into, a Path tool, better stamps, and the editor tools a level editor has (box select, align, arrays, snaps,
layers, history, measure, search, favourites).

Where it lives:

| Piece | File |
|---|---|
| The Tools window (Select, Arrange, Path, Stamps, Layers, History) | `stream/fui-web/Interface/AddOns/WrathUnleashedUI/BuildTools.lua` |
| The geometry (paths, arrays, align, box, stamp mirror and size, pictures, collision, guard) | `labs/den-lite/src/tools.rs` (std only) |
| Den Lite's side of every tool | `labs/den-lite/src/buildtools.rs` |
| Small hooks | `den.rs` (Prop `solid`, undo 200 deep, `history`, `place_many`, `edit_each`, `replace_props`, `stamps-mine`), `raw.rs` (an open door's state), `world.rs` (one dispatch, the guard rail), `ovmod.rs` (caps, the selection's extra state, `Sel.path`), `models.rs` (`is_wmo`), `Gizmo.lua` (a world click goes to a waiting tool first), `DenBar.lua` (the rail's Tools; Path and Stamps open the window on Den Lite), `Builder.lua` (the Stamps tab lists Den Lite's library; search by category; the More menu by caps) |

Open it: the rail's **Tools** (the grid icon), the rail's **Path** or **Stamps** in Your Den, or `/tools [tab]`.

## What's done

### Select
- **Box select**: two clicks on the screen (its corners); Shift on the second adds to what is selected. Den Lite
  projects every prop's middle with the build camera it knows (gizmo.rs `screen`), so the box is what you see.
- **Group** (every prop in the selection's group: a path, a stamp, a group you made), **Of its kind**, **Clear**.
- **Snap**: grid off / 0.5 / 1 / 2 yd, turn step 15 / 45 / 90 (the Builder's own snap and step).
- **Land** (surface snap): the selection goes onto what is under it: a table's top, a floor, a crate, else the ground.
- **Solid**: Solid, Walk through, As the game (see Collision below), with a line saying what the game does with this
  prop's model.
- **Lock / Unlock / Hide / Show all**: hidden props stay in the Den (and its file) but aren't sent to the game.
- **Measure**: from you to the prop, or between two selected; its size; the spread of a bigger selection.
- **Replace** with the gallery's pick: this one, or every prop of its kind (one undo).

### Arrange
- **Align** X / Y / Z (to the first selected), **Bottoms** (the lowest), **Tops** (the highest); **Spread** X / Y / Z
  (evenly between the two ends, each keeping its place in the order). One undo each.
- **Array**: n copies a step apart (x, y, z yards); the whole selection is copied as one piece. One undo.
- **Ring**: n copies round a circle of a radius, round the selected prop or round you, facing in, out, along the
  circle or as it was. A selection of several props goes round as one piece. One undo.
- **Copy, Paste, Duplicate** (the Builder's own).

### Path
- Pick the prop in the gallery, **Add points**, click the ground along the way (right-click stops). Each point is
  shown as a small stake (this player only). The curve passes through every point (Catmull-Rom, centripetal: no
  loops or overshoot at a sharp corner).
- **Every** n yards along its real length; **Smooth** or straight; **Along** or **Across** the path; **On the
  middles** (fences: a fence n yards long joins the next end to end); on the **Line**, **Left**, **Right** or
  **Both** sides, **Off the line** by n yards (the right-hand row faces back across, so both rows face the walk).
- Every prop stands on the ground where it is (the Den's ground from Forge, or the room's floor).
- Presets: **Fence** (2 yd, middles, along), **Lamps** (6 yd, both sides, across, 3 yd off), **Road** (2 yd, along).
- **Undo point**, **Move point** n (then a click), **Delete point** n, **Clear**.
- **Lay**: one group, one undo. The path's points are kept in the Den file's `paths`.
- **Edit a laid path**: select one of its props, its points and numbers come back, change them, **Lay again**: the old
  path is replaced in place, one undo puts the old one back.
- Up to 64 points and 160 props a path (the Den's 800 still apply).

### Stamps
- **The library by category**: Buildings, Gardens, Markets, Camps, Water, Rooms, Paths, Places, Mine; Leyline's own,
  yours, and places copied from your own game. Each row says how many pieces and who made it (its credit).
- **A picture of each stamp**: a top-down plan drawn from the pieces' real footprints (the player's own model table
  when Den Lite has it, else our estimates), coloured by kind (buildings and floors, furniture, small things, plants,
  lights).
- **Before it goes down**: turn (15 degree steps), mirror (left to right, front to back), size (a quarter to three
  times).
- **Place with the mouse**: a click on the ground puts the stamp there (moved inside the Den if it would stick out);
  another click moves it; changing the turn, mirror or size puts it down again; **Enter keeps it**, **Esc takes it
  back**. Until it is kept it is one undo step, so moving it never leaves copies behind.
- The gallery's Stamps tab lists the same library; a double-click there places with the mouse too (Den Lite).
- **Save the selection as a stamp**: a name, a title and the category shown; saved in `<Dens>/stamps-mine`, round
  the selection's middle, its lowest foot at 0. Its credit is the builder's name and the date; when its pieces came
  from other stamps, their names and credits are kept beside it (`from`), for the marketplace (another agent) to
  show. **Delete** removes one of your own (never Leyline's).

### Layers
- Every group in the Den as a row (paths and stamps are groups by themselves): **select**, **Hide / Show**,
  **Lock / Unlock**; **Group the selection** under a name; **Ungroup**. Names are kept in the Den file
  (`extra.groups`).

### History
- The undo steps by name, newest first ("Placed 12", "Changed Wooden Chair", "Deleted Barrel", "Replaced 3 with 5");
  a click goes back to just before that step (one change sent). The undo history is now **200** steps deep (was 50).

### The gallery
- Search matches the name, the key and the category ("light", "fence", "seat"); Favourites and Recent were already
  there (right-click a tile: a favourite).
- The More menu shows Den Lite's align, measure, group, hide and show now (by what Den Lite says it does, its caps).

## Collision: what the game does, and what Den Lite can do

What the game does (the 3.3.5a client):

- **Collision is the model's own.** A gameobject blocks you, and you can stand on it, where its model file has
  collision: an M2 model (most props) only where its maker gave it collision triangles; a WMO (a building) all over,
  with floors you walk on, stairs, and doorways you walk through. Crates, barrels, fences, walls, tables, floor
  tiles, stairs and rocks usually have it; plants, flowers, banners, rugs, flames and small things on a table usually
  don't. The game grows the collision with the prop's size and turns it with the prop.
- **The server can't add collision.** A model without collision triangles can't be made solid by anything the server
  sends: there is no flag for it, and the server can't send a model.
- **The server can take it away, the one way the game knows: an open door.** A gameobject of type door (0) in the
  open state (GO_STATE_ACTIVE) is walked through, as AzerothCore's `GameObject::SetGoState` turns a door's collision
  off when it opens. So **Walk through** tells the game the prop is a door, sent open: the prop's entry gets a twin
  (tools.rs `PASS_TWIN`, the bit above gizmo.rs's build twin) whose query answer is the prop's own model, name and
  size as a door. The prop keeps its object and its place; a chair made walk-through can't be sat on.
- Den Lite doesn't read model files, so whether a prop is solid in the game is a guess from its kind (tools.rs
  `client_collision`) and from its model being a building (models.rs `is_wmo`); the Tools window says it as a guess.

What Den Lite adds (the guard rail, world.rs `guard_rail`, buildtools.rs `guard`):

- **Solid** marks a prop for the guard: when the game lets you sink into its top (you were on it and went down
  through it), or a jump lands you inside it (a top a jump reaches, about 1.9 yd), Den Lite puts you back on its top
  with a near teleport. So a rug, a platform or a plant marked Solid can be stood on and jumped onto even without
  collision in its model.
- **Falling through the Den's ground** (6 yd under it, on the Den's own island): put back on the ground.
- At most once every 1.5 s each, never while flying, swimming or on a transport, and written to the events
  (`guard`). A prop with collision in its model never trips it (the game holds you up first).

What can't be done (said in the window and here):

- **Walking into the side of a prop whose model has no collision**: the game lets you through and only the game
  could stop you there. A server push back at every step would make walking jerk; Den Lite doesn't do it.
- **Making a prop's collision bigger or smaller than its model**, or giving a WMO no floors.
- **Mirroring a model**: the game draws a model only as it is made (no negative size), so a mirrored stamp mirrors
  where its pieces stand and how they are turned; a lopsided piece is turned to match, not flipped.
- **A stamp's live copy following the cursor before the click**: the carried copy (gizmo.rs) is one prop; a whole
  stamp follows by clicks (put down, click elsewhere to move, Enter keeps). A multi-piece carried copy is the
  placement aids' (the Gizmo agent's) to add.
- **Hide** is a view: hidden props aren't sent, but a redo of a step that changed a hidden prop shows it until the
  next login. Undo of a hide isn't a step (as Lock).

## Den Lite's verbs (over `wubuild cmd`)

`tool point|box|stamp ...`, `path show|hide|undo|del|clear|lay|edit`, `array`, `ring`, `align`, `distribute`,
`land`, `solid on|off|auto`, `hide`, `show`, `group`, `ungroup`, `selgroup`, `groups`, `layer <id> ...`, `measure`,
`history`, `undoto <n>`, `stamps`, `stampplan <name>`, `stamp save|delete|keep|cancel|again`. The module comment in
buildtools.rs has each one's words. Answers: `wubuild point | pathpts | pathedit | staged | stamps | stampplan |
groups | history | measure`, and the state line carries `solid=;coll=;group=;hidden=` for the selection and the
tools' verbs in `caps=`. The path's points are kept in the session on Den Lite's side (an addon line holds 255
letters, too few for 64 points).

## Tests

| Where | What | Here |
|---|---|---|
| `labs/den-lite/src/tools.rs` | 15 std-only tests: curves through every point, spacing by length, fences end to end, both sides facing the walk, caps, arrays, rings, align, distribute, box, snaps, mirror and size, a selection's middle, the stamp's picture, categories, measure, collision guesses, the walk-through twin's bits, the guard (sink, wobble, sides, edges, turned tops, two tops, jumps, the ground, surface snap) | **pass** (`rustc --edition 2021 --test src/tools.rs`) |
| `labs/den-lite/src/buildtools.rs` | 12 crate tests: verbs and caps, a path laid as one group and one undo, its points moved/undone/cleared, edited and laid again, arrays and rings, align and distribute, Solid off as an open door and undo, the guard on a Solid prop, a jump, flying and the ground, hide/show and layers, history and undo to a point, merged changes, a selection saved as a stamp with its credit, a stamp mirrored and sized | **unrun** (the crate needs crates.io); type-checked here against stand-ins of the crate's types |
| `labs/den-lite/src/raw.rs` | a prop with Solid off is sent as an open door | **unrun** (crate) |
| `labs/den-lite/src/ovmod.rs` | the existing test now checks `stack` (still the full Den's) and the tools' caps | **unrun** (crate) |
| `stream/tests/test_wuui_buildtools.lua` | 52 checks: tabs, snap and Solid states, Land, Replace, Box select's two clicks through Gizmo.lua's real click, the Path tool (presets, points, move point, undo, Lay, edit), a right-click stops, the stamp library, categories, rows, the gallery's list, the picture, turn/mirror/size, place with the mouse, Enter/Esc, save, a bad name, layers, history, measure, arrays, rings, align, parts, other mods ignored, no brand words, nothing sent elsewhere | **pass** |

Unchanged and passing: test_wuui_builder 218, denbar 75, gizmo 121, denhome 89, denlayout 138, dojo 69,
denquiet 7, gallery 48, test_brand 236.

## Needs a live check (the owner's PC: `den-lite-test`, then the game)

1. Press **den-lite-test**: the new crate tests and the den.rs / world.rs / raw.rs / ovmod.rs edits were type-checked
   against stand-ins here, not compiled with the real crates.
2. **Walk through**: select a crate, Tools > Select > Walk through, walk into it: you should go through. If you still
   bump into it, the game keeps collision for doors of that model; say so and the button becomes "not on this prop".
3. **Solid**: place a rug or a plant, Solid, jump onto it: you should end on its top (a small hop up).
4. **Path**: pick a fence, Tools > Path > Fence, Add points, click four spots round a curve, Lay: fences end to end on
   the ground, following hills. Ctrl+Z: all gone. Lay again, select one fence, Edit a laid path, Move point 2, Lay
   again: in place of the old one.
5. **Stamps**: Tools > Stamps, pick Garden, Mirror L/R, Turn +15, Place with the mouse, click the ground, click
   elsewhere (it moves), Bigger (it is put down again bigger), Enter. Ctrl+Z takes the whole garden.
6. **Save a stamp**: Box-select a few props, Stamps > name "porch", Save; it shows under Mine with your name; place it.
7. **Box select** in the far view (View, then zoom out): the box should match what you see.

## Owner decisions

- Whether **Walk through** should stay a door twin if the live check shows door-open collision still blocks for some
  models (then it is a per-model list).
- Whether hidden props should be hidden for guests too in a shared Den (they are now: not sent to anyone).
- The stamp categories' names (Buildings, Gardens, Markets, Camps, Water, Rooms, Paths, Places, Mine).
