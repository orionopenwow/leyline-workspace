# Your Den's editor against the owner's LANDSCAPE editor (5 Nov 2026)

Owner, 5 Nov, after testing Your Den live: "study this video and use it as reference for how the editor in the den
should function". The reference material:

- the video's frames (`/home/claude/landscape-ref/f01..f37.jpg`, 1 a second, his LANDSCAPE editor on Spawn);
- his notes on the editor's files;
- his own placing code, `design/spawn-reference/mega-placer.js` (kept there as the spec).

His three bugs from the live test come first, then the parity table. Branch `agent/gr-den-editor`. The Rust was not
compiled here: press **`den-lite-test`** (or `den-lite-all`) on the PC.

## The three bugs (5 Nov) and what changed

| # | Owner | Cause | Fix |
|---|---|---|---|
| 1 | "ive selected chair but it does not appear on mouse and start moving on ground as mouse moves around" | (a) The card's click could come before Gizmo.lua's half-second check had turned build mode on, and then `G.Arm` did nothing. (b) A first ray that found no ground (the cursor over the horizon, or past REACH) drew no copy at all. (c) The game's own click went to props (bug 2), so their glow and the BUTTON1 hand-over never came. | `G.Arm` turns build mode on when it is wanted. The cursor's place rides along with `preview <id> u v aspect zoom` when it is over the world. Over the gallery, the first hover on the world draws the copy (every frame while a card is in hand). Den Lite draws the first copy `FIRST_AHEAD` (5 yd) in front of the view when the ray finds no ground. Dragging a card out of the gallery also works (below). |
| 2 | "when select arrow is active i cannot left click object to select it ... it should go into the slide mode" (screenshot: "You are too far away.") | In build mode props were sent as goobers with flags 0, so the game took the click as a use, checked its own range from the character and refused. The click never reached the editor. | **Root:** in build mode Den Lite sends every prop, and every handle, with `GO_FLAG_NOT_SELECTABLE` (0x10, gizmo.rs `shown`). The game no longer highlights, tooltips or uses them, so every click on the world is Gizmo.lua's: a still left click sends `click … carry` (picked up, glides, the next click puts it down), Shift adds, Esc puts it back. Play mode sends flags 0 again (a chair is sat on). **Belt and braces:** a `CMSG_GAMEOBJ_USE` on a prop in build mode now picks it up too (`carry_prop`, `carried <id>`), and is ignored while something is in hand. The right-click menu, which came from the game's use, now comes from a still right click (`click … menu` → `picked`). |
| 3 | "free roam camera still needs kinks sorted out and work like noggit camera functionality" | Forward was flat over the ground. Pitch was always snapped back after a drag. There was no slow speed and no Q / E. | Noggit's controls mapped onto the far view (below). The pitch you turn to stays where the game lets the addon read it. |

### Noggit's camera, and the Den's far view

From noggit3 (`src/noggit/camera.cpp`, `MapView.cpp`, the open-source map editor):

- **Mouse:** holding the right mouse button looks round (yaw and pitch). The wheel goes to the tool, not the camera.
- **Moving:** W / S move along the view direction, pitch included. A / D strafe. Q / E move up and down in world space.
- **Arrow keys:** ↑ / ↓ pitch, ← / → turn.
- **Speed:** Shift+1 to 4 set it to 15, 50, 200 or 800; O / P halve or double it.

| Noggit | The Den's far view (wheel out past the end, or the snap key) |
|---|---|
| Right mouse held: look | The game's own right-drag (mouselook): it turns your facing and the camera's pitch. Den Lite's ray and the flight follow the facing. **Pitch:** the 3.3.5a API has no GetCameraPitch. After a camera drag Gizmo.lua saves view 2 and reads the pitch from the first CVar that holds one (`cameraPitchD`, `cameraPitch`, `cameraSavedPitch`). The pose's sign tells which way is down, and Den Lite is told `cam pitch <radians past the pose>`, so the pitch stays as in Noggit. If no CVar reads, the pose's pitch is put back after the drag, as before. A guessed pitch would put the copy off the cursor. `gizmo report` says which CVar worked (`pitch src=…`). **This needs the owner's live check.** |
| W / S along the view, pitch included | `cam fly` forward = the look direction with its pitch: looking down, W goes down toward what you look at. The ground (1 yd) and 250 yd still bound it. |
| A / D strafe | Same (a quarter turn over the ground). |
| Q / E world up / down | Space / X always. Q / E too while they turn nothing: with a selection, something in hand, or a prop under the cursor, Q / E turn it (the 4 Nov decision). |
| Shift faster | Shift: 48 yd/s. |
| Alt / slower | Alt: 5 yd/s (`cam fly … s`). Normal: 16 yd/s. |
| Smooth motion | Den Lite glides its camera object 450 ms ahead and eases out on a let-go. |
| Wheel | Zooms in and out of the far view (further each notch); Ctrl+wheel zooms while something is in hand. |
| Arrow keys pitch / turn | Not matched: the arrows fly (as W A S D). The client has no call to set the pitch, and a turn is the mouse's. |
| Can't match | Roll; a pitch the game won't let an addon read (then it snaps back after a drag); speed presets (two modifiers instead). |

## Parity with the video and the editor's files

Each row gives what the frames (or the owner's file notes) show, what the Den has, the gap, and what changed today.

| # | LANDSCAPE (frame / file) | The Den now | Gap / done 5 Nov |
|---|---|---|---|
| 1 | f01–f04: in the game, a wrench button opens a small menu: World Editor (F4), Light Editor, Look Sliders, Hide / Show Props, Files, Test Box | Den bar: Build / Play (F7), the rail, Files on the top strip | Same idea; no light editor or look sliders (the Den's look is the Builder's Look) |
| 2 | f05–f08 (camera-router.js, camera-editor.js): the editor opens and the camera rises to a free editor view | Build mode starts at your character; the wheel out past the end, or the snap key, gives the far view (owner 4 Nov: "infinite camera zoom out") | Kept: the 4 Nov decision so W walks on arrival |
| 3 | f05+: top bar: Undo n / Redo n, menu, Snap: off ▾, three brush shapes, size 1×, 15° / 45° / 90°, time of day 15:00, ?, Files, Export | Den bar strip: Undo / Redo n, menu, Snap, 15 / 45 / 90, Files, Invite | Time of day and brush shapes: no (Forge has the ground's own tools) |
| 4 | Left rail of tools: select arrow, raise / lower, paint, water, … and 10 / 25 / 50 / 100 | Rail: Select, Build, Move, Rotate, Stamps, Path, Tools | Terrain brushes are Forge's (DEN.md "Terrain"); no brush ring yet (see 21) |
| 5 | Gallery bottom left: EVERYTHING, colour dots, search, A–Z, a filter, ×; category column; tiles with names; "1–20 of 53" | Builder gallery: Everything and every category, search, A–Z, Spin, pages | Colour-dot filter: no |
| 6 | f17–f18: click a card (Birch): the real model is under the cursor at once; panel "BIRCH · in hand · click the ground" with its picture | Card → `preview` → the real prop under the cursor; panel "in hand" | **Fixed today** (bug 1): build mode at once, the cursor's place with the pick, a copy even when the first ray finds no ground |
| 7 | build-tool-core.js: drag a card from the palette and drop it under the cursor | A tile dragged out of the window was placed in front of you | **Done:** in Den Lite's build mode the drag picks the card up, its copy glides over the world (Gizmo.lua counts the world as under the cursor while a card is dragged out of the window), and letting go on the ground puts it down (one undo) and selects it. Let go over the window: still in hand |
| 8 | f18–f23: the copy glides with the cursor | Hovers 30 a second while in hand, the copy redrawn every 33 ms | Same |
| 9 | f24: a click places it; it is selected with a white outline glow and the move arrows | `commit`: placed, selected, the sparkle glow, halo, red / green / blue handles | No outline shader in the stock client: sparkle and halo instead |
| 10 | f24+: panel PIECE: Position X Y Z in red / green / blue with ± steps; Move · Turn · Size pad with 0.1 / 0.5 / 1 / 5; Rotation, Scale; Settle; Swap with (picture, Swap, "All in Longhouse"); key help; Selection n | Builder's selection panel: position, the pad, turn, size, Swap / Swap all of its kind, Land (settle), lock | Same set; "All in <area>" is "all of its kind" |
| 11 | Bottom status line: "Raise/Lower · r 8 · strength 1.5 · falloff 4 · Moved Cottage +4.43 m · 1 selected" | The pill: "Place · Wooden Chair · snap off · 15° · …" plus the Gizmo tag | Same |
| 12 | Selection help: Ctrl+drag box, Alt drag circle, L area, Shift adds, Del deletes | Shift adds, Del deletes, Box select in Tools | Circle select and area select: no |
| 13 | ARRAY: ×5 every 4 m; Fill box, Fill gap | Tools > Arrange: array, ring | Fill box / gap: no |
| 14 | PLACEMENT: random Yaw ±180°, Tilt, Scale ±10%; Stamp 1–8 | Stamps | Random turn / size while placing: no |
| 15 | mega-placer.js 1: a ring where the cursor meets the ground or a floor, every frame; gold place, teal grab, grey nothing | — | **Done:** Den Lite's ring (`gizmo ring on`, sent by Gizmo.lua on entering build mode): one object only you see, not selectable, made at the cursor's ground point (or a prop's top within 2.5 yd of the ground) on every hover. The client can't tint a gameobject, so each kind is its own entry (`RING`: Place here, Grab, Nothing here), all the "Crate Highlight" glow for now. TODO(owner): pick three looks in the Den, then set their displays in gizmo.rs `RING`. The addon is told `ring place|grab|none` |
| 16 | mega-placer.js 2: armed, a click drops a copy on the ring, one commit; Esc disarms; the turn carries to the next placement | Click places (one undo), Esc puts it away | **Done:** `next_yaw`: the next card starts turned as the last placed one |
| 17 | mega-placer.js 3: press on a placed piece and drag: it rides the ground keeping the grab offset and its lift; one commit on let-go (undo returns it) | Press and drag (`drag`, grip kept), and click to pick up and click to put down (`click … carry`) | Both kept: past 6 px it is a drag (let go drops it); a still click picks it up into the glide, the next click puts it down |
| 18 | mega-placer.js 4: Z / X turn the piece under the cursor or the dragged one 15°; mid-drag it lands with the drop; with nothing there, the next placement's yaw | Q / E turn what is in hand or selected | **Done:** Q / E over another prop turn that one (`turnat <id> <deg>`, one undo). Mid-drag the turn lands with the drop, as before. The `yaw <deg>` verb sets the next placement's turn; the keys only reach it with a card in hand, because with nothing anywhere Q / E strafe as in the game |
| 19 | mega-placer.js 5: hits resolve to the topmost object; terrain and players never pickable; the ring and the ghost ignored | Props are single objects; only props are picked (pick.rs); the copy, the halo and the ring are not selectable and not in the ray's list | Same |
| 20 | mega-placer.js 6: positions rounded to 0.05 | Snap grid when Snap is on; free placement keeps the cursor's exact spot | Not done: rounding free placement would move each piece up to 2.5 cm off the cursor; the save keeps its own precision |
| 21 | lib/ed-ground.js: a moved piece sits on the terrain or on whatever is under it | `on_ground` over the Den's edited heights | **Done:** `surface_z`: in hand, a prop sits on the highest top under it within 2.5 yd of the ground (tables, crates, decks, never a roof over you indoors), never its own |
| 22 | ed-gizmo-core.js / ed-gizmo.js: move, rotate and scale handles (W / T / V) with hover highlight | Move handles (X, Y, Z flags; hover names them; click grabs the axis); turn with Q / E or the wheel; size in the panel | Rotate and scale handles: no (no ring or box model to show them; the panel does it). **Key map proposal** below |
| 23 | build-multi.js: moving several selected pieces together | A drag moves the one under the cursor; the panel's move moves the whole selection | Gap: the glide carries one prop (the others stay) |
| 24 | ed-history-core.js: one undo history for all of it | Den Lite's undo covers placing, moving, turning, tools and stamps (History tab) | Same |
| 25 | terrain-editor-core.js: a brush ring under the cursor (raise, lower, paint, foliage) | Forge (the launcher) edits the Den's ground | Gap: no in-game brush (the client can't change its terrain live) |

### Key map (proposal for the owner)

The handle-mode keys W / T / V clash with W as fly-forward in the far view, and with W walking near your character.
The Den keeps W A S D for moving the camera everywhere. Proposed:

| Key | Does | When |
|---|---|---|
| 1 / 2 / 3 | Handle mode: move / turn / size | Only while a prop is selected and the right mouse isn't held. **Not built**: there are no turn or size handles yet |
| Q / E | Turn 15° (Shift+wheel 5°) | What is in hand, the prop under the cursor, else the selection. Flies up / down in the far view when none of these |
| Space / X | Fly up / down | Far view |
| Shift / Alt | Fast / slow flight | Far view |
| G | The exact click (the ground-target circle) | A card or prop in hand |
| Del, Ins, Esc | Delete, swap, put back | As now |

## Live checks for the owner

Press, in Control:

1. **`den-lite`** builds Den Lite (the Rust changes; or `den-lite-test` / `den-lite-all` for its tests first).
2. **`addons`** publishes the Leyline addons (Gizmo.lua, Builder.lua).
3. **`den-move`**. Its new lines:
   - "build mode: props never take the game's click";
   - "card: the copy glides under the cursor";
   - "card: a click puts it down, selected";
   - "select: a click on a prop picks it up, the next puts it down";
   - "play mode: props usable again".

Then by hand, in Your Den:

1. Build mode. Click the Wooden Chair card, then move the mouse off the gallery over the grass: the chair glides
   under the cursor at once, with a ring under it. Left click: placed, sparkling, with its arrows.
2. Drag a card out of the gallery and let go on the grass: placed there.
3. Click the placed chair (any tool): no "You are too far away."; it lifts and glides; click again to put it down;
   Esc puts it back. Shift+click adds to the selection. A still right click opens its menu.
4. Hover another prop and press Q: that prop turns. Carry a chair over a table: it sits on the table.
5. Wheel out past the end into the far view:
   - W flies where you look (down toward the ground when looking down); Shift is fast, Alt slow; Q / E or
     Space / X go up and down.
   - Type `/wudebug` and right-drag to look down further, then let go. The "camera pitch" line names the CVar that
     was found. If one was, the pitch stays and the copy is still under the cursor. If none was, it says "not
     readable" and the pitch snaps back as before. Tell Claude which.
6. Play mode: sit on the chair (it is usable again).
