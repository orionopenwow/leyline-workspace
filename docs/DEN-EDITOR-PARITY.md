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

## The second live test (5 Nov): the copy glides, the hover lights, A / D slide

Owner, 5 Nov: "objects should be clickable, highlightable in build and select mode and dynamically move instead of
disappearing, reappearing, you need to make sure it makes the video i gave you" and "when holding d or a the free roam
camera just goes in circles instead of normal expected function". Branch `agent/gr-den-glide`.

| # | Cause | Fix | Where |
|---|---|---|---|
| A1 | A gameobject's place can't change after its create in 3.3.5a, so the copy under the cursor was a gameobject made anew at each spot: about 30 creates a second, seen as a flicker | The copy is a **creature wearing the prop's own model**, made once and glided (SMSG_MONSTER_MOVE: facing angle, Flying, OrientationFixed so it never turns to the way it slides; 80 ms a glide, each from where it is then). The drop makes the real gameobject first, then the creature goes. Card, click-to-carry, press-and-drag and an arrow drag all glide | labs/den-lite `glide.rs`, gizmo.rs `draw` |
| A1b | A creature's model comes from CreatureDisplayInfo.dbc and CreatureModelData.dbc | The launcher's Den patch (patch-T, from the player's OWN tables, nothing shipped) adds for every GameObjectDisplayInfo row with an `.m2` / `.mdx` model a creature display and model row **70000 + that display** naming the same file (scale 1). Layouts (3.3.5a 12340): CreatureDisplayInfo 16 fields (ModelID at 1, scale at 4, alpha at 5), CreatureModelData 28 fields (ModelName at 2, scale at 4, box at 17-22). Refused when a table isn't that shape or already uses ids past 70000. `<dens>/den-creature-models.json` lists the displays covered (not written when a later archive hides the tables); Den Lite glides only those, and only for the Den's owner (a friend's game has no such rows: their copy is the gameobject as before). The ground status says "N prop models can glide" or why not | launcher `host/creaturemodels.rs`, `host/terrain.rs` |
| A1c | A building (WMO) can't be a creature's model; a tilted prop can't lean as a creature | Their copy stays a gameobject made anew, but only when it moved and at most every 150 ms (`WMO_DRAW_MS`) for a building. `gizmo glide off` turns the creature copy off (to compare); the addon is told `glide on|off|none` | gizmo.rs |
| A2 | Props are NOT_SELECTABLE in build mode (the first test's "You are too far away."), so the client's own mouse-over highlight is gone. The 3.3.5a client has no highlight flag for a gameobject (GameObjectDynamicLowFlags: ACTIVATE 0x01, ANIMATE 0x02, NO_INTERACT 0x04, SPARKLE 0x08, STOPPED 0x10; the HIGHLIGHT bit is a later client's) and plays no spell visual on one | The prop under Den Lite's own ray pick gets the sparkle (GAMEOBJECT_DYNAMIC in a values update: no respawn) **and a light under it** (the "Crate Highlight" glow, 0.8 of its footprint, one small object made only when the hovered prop changes, at most every 100 ms). The selection is the stronger one: the full halo, the sparkle and its arrows; the hover's light goes when it is selected or picked up | gizmo.rs `HOVER`, `hover_glow` |
| A3 | Clicks | Unchanged: Den Lite's ray decides what a left click hits; props stay non-usable so the game never eats the click | |
| B | Near your character A / D are the game's TURNLEFT / TURNRIGHT: the character and the camera swing round | In build mode at your character, the keys the player has on turning (A / D and the arrows by default) get STRAFELEFT / STRAFERIGHT override bindings (not Q / E: they turn props). The far view already flies sideways with them; its maths was checked (right = facing minus a quarter turn: (sin o, -cos o)). The tag says "W / S walk, A / D slide" | Gizmo.lua `G.StrafeKeys`, `bindings` |

**The video's flow (C), re-read from the frames:** f17-f18 pick a card, the model is under the cursor at once; f18-f23
it glides with the cursor (now one creature gliding, no flicker); f24 a click places it, selected (sparkle, halo,
arrows); a placed piece pressed and dragged glides and the let-go drops it (mega-placer.js 3); hovering a piece lights
it (mega-placer.js teal "grab"). Not matched: the outline shader (no such thing in the stock client: sparkle and light).

**Needs the owner's live check:** that the stock client draws a gameobject's `.m2` on a creature at the right size and
facing (expected: the same file, scale 1, the creature's origin at its feet as a gameobject's), that the sparkle shows
on a not-selectable prop, and that A / D slide.

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
| 8 | f18–f23: the copy glides with the cursor | Hovers 30 a second while in hand; the copy is one creature gliding to each new spot (glide.rs), a building's copy redrawn at most every 150 ms | **Done 5 Nov (second test):** no flicker for an `.m2` prop |
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

### Key map (built 5 Nov, branch `agent/den-noggit`)

The proposal is built, with the view marks added: see **docs/DEN-TOOLS.md, "Keys (build mode only)"** for the whole
map. In short: 1 / 2 / 3 Move / Turn / Size (not W / T / V: W walks and flies); Q / E turn; Del, Ctrl+C / V / D,
Ctrl+Z / Y, Ctrl+A, Ctrl+drag box; G grid (Shift+G size), R turn step, Shift+R scatter; L slope, Shift+L upright;
F frame, Home back to you, Ctrl+1..3 keep a view, Shift+1..3 go back; right mouse + wheel the flight's speed; Esc
cancel, then deselect, then the game's menu. Every one exists only in build mode.

## Noggit pass (5 Nov, branch `agent/den-noggit`): every reference feature and where it stands

Owner, 5 Nov: "Continue to work on the den until everything functions especially the moving the objects, distinctly
being in play vs build mode, camera working, move objects working and every other convenient and noggit feature
possible build right into the den and base software for the launcher". New files: `DenKeys.lua` (the editor's keys),
`labs/den-lite/src/noggit.rs` (their Den Lite verbs); small hooks in Gizmo.lua, Builder.lua, DenBar.lua, Guide.lua,
gizmo.rs (`cam frame|mark|to`, the `v<speed>` word, `yaw set`), ovmod.rs (the verbs and caps).

Status: **done** (built and tested here; the Rust unit tests are written but not compiled in the cloud), **partial**,
**missing**.

| Reference (video frame, file or Noggit) | Status | The Den |
|---|---|---|
| Play vs build, unmistakable | done | Build: the gold edge (one: DenBar's; the Gizmo's own frame and word hide in a Den), the strip, rail and gallery, the "Build mode" banner, the hint line with the tool's keys, the editor's cursor (casting with something in hand, the hand over a prop), props not selectable (gizmo.rs). Play: every editor frame hidden, the "Play mode" banner, the game's cursor and keys, props usable. F7 toggles |
| No editor key in play mode | done | Every override binding of the Gizmo, DenKeys, the selection panel and the tools' window is gone after F7, after leaving the Den and after a reload (the game drops them); a change asked for in combat (the game refuses it) is done when combat ends. test_wuui_denkeys.lua checks each owner |
| Esc: cancel, deselect, nothing destructive | done | G.Escape; the selection panel's Esc does the same in build mode |
| f01-f04 wrench menu, World Editor (F4) | partial | The Den bar's Build / Play (F7) and Files; no light editor or look sliders |
| f05-f08 the camera rises to a free editor view | partial | Kept at your character on arrival (4 Nov decision); the wheel out, the snap key, or **F** (frame) give the far view |
| Noggit camera: right mouse held looks | done | The game's own mouselook; the pitch kept where the client lets it be read |
| Noggit camera: W / S along the view, A / D strafe, up / down | done | Far view: `cam fly` (pitch included), Space / X and Q / E up / down; near you A / D slide, never turn |
| Noggit camera: Shift fast, Alt slow | done | x3 and x1/3 of the chosen speed |
| Noggit camera: speed presets / O P | done (another way) | Right mouse + wheel: 2 to 96 yd/s, kept, shown in the status line |
| Frame the selection | done | F (`cam frame`): the far view round the whole selection |
| Back to the character | done | Home / Numpad 0 in the far view; zooming back in still does it |
| Camera bookmarks | done | Ctrl+1..3 keep, Shift+1..3 go back (saved settings) |
| Arrow keys pitch / turn | missing | The client has no call to set the pitch; the arrows fly |
| f05+ top bar: undo / redo n, snap, 15 / 45 / 90 | done | The strip; plus Ctrl+Z / Y, G / Shift+G, R |
| Time of day, brush shapes | missing | Forge has the ground's tools |
| Left rail of tools | done | Select, Build, Move, Rotate (and 1 / 2 / 3: Move, Turn, Size), Stamps, Path, Tools |
| Gallery (categories, search, A-Z, pages) | done | Builder gallery |
| Gallery colour-dot filter | missing | |
| f17-f18 a card: the real model under the cursor | done | (5 Nov fixes) |
| Drag a card out of the palette | done | |
| f18-f23 the copy glides | done | glide.rs (creature copy) |
| f24 click places, selected, outline glow | partial | Sparkle, halo and arrows: no outline shader in the stock client |
| PIECE panel: position, pad, rotation, scale, settle, swap | done | The selection panel |
| Status line | done | The pill and the Gizmo's tag (tool, keys, speed) |
| Selection: Shift adds, Del deletes | done | |
| Selection: Ctrl+drag box | done | Ctrl+drag (Den Lite projects every prop with its build camera: `tool box`) |
| Selection: Alt-drag circle, L area | partial | Ctrl+A: every prop within 30 yd of you (`selnear`); no circle drawn |
| Move / rotate / scale modes (W / T / V) | partial | 1 / 2 / 3 tools: Move drags, Turn drags (live, one undo), Size drags (applied on the let-go, no live size: the copy can't be sized while carried) and the wheel; no rotate or scale handles in the world (no ring or box model) |
| Copy / paste | done | Ctrl+C; Ctrl+V puts the copy in hand under the cursor with its turn, then its size |
| Duplicate keeping the offset | done | Ctrl+D (`dupby`: the whole selection, one undo; again: the same offset as the last copy was moved) |
| Delete key | done | Del with a selection, whatever window is open |
| Undo / redo keys | done | Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z in all of build mode |
| Snap to grid, its size | done | G on / off, Shift+G 0.5 / 1 / 2 / 5 yd |
| Angle snap 15 / 45 / 90 | done | R, and the strip |
| Align to the ground's normal / upright | done | L (`slope`: on the ground and tilted to it, the whole selection, one undo), Shift+L upright |
| PLACEMENT: random yaw, scale | done | Shift+R scatter: a random turn on each card (seen before the click), a random size 0.85-1.15 on each placed prop (one more undo step each) |
| Tilt at random | missing | |
| Place again | done | Shift+click keeps the card in hand |
| ARRAY, ring | done | Tools > Arrange |
| Fill box / fill gap | missing | |
| Stamps | done | Tools > Stamps |
| mega-placer.js 1: the ring under the cursor | done | TODO(owner): its three looks |
| mega-placer.js 2-4: armed place, press-drag move, Z / X turn | done | Q / E turn |
| mega-placer.js 5: topmost hit, terrain never picked | done | pick.rs |
| mega-placer.js 6: positions to 0.05 | missing | Free placement keeps the cursor's spot |
| ed-ground.js: on what is under it | done | surface_z |
| build-multi.js: moving several together | partial | The panel's move and the keys move them all; the glide carries one |
| ed-history-core.js: one undo history | done | History tab, 200 deep |
| terrain-editor-core.js: a brush ring | missing | Forge (the client can't change its terrain live) |

**Needs the owner's live check** (Rust not compiled in the cloud; the Lua is tested on the fake client):

1. Press **`den-lite-test`** (noggit.rs's 8 tests and the gizmo.rs / ovmod.rs edits), then **`den-lite`** and
   **`addons`** (DenKeys.lua is a new file in the .toc).
2. In Your Den, build mode: select a chair, Ctrl+D three times (a row a yard apart), move the last one 3 yd, Ctrl+D:
   the next is 3 yd on. Ctrl+Z takes one copy at a time.
3. Ctrl+C a turned, sized prop, Ctrl+V: it is in hand under the cursor turned as it was; click: placed at its size.
4. Press 2, press on a prop and drag right: it turns live in 15° steps; let go; Ctrl+Z undoes it in one go. Press 3,
   drag up: bigger on the let-go.
5. Ctrl+drag on the grass round three props: they are selected. Ctrl+A: everything near you.
6. On a hill (after Forge), L: the prop lies on the slope; Shift+L: upright.
7. F with a selection: the view flies out round it. Hold the right mouse and roll the wheel: the status line's
   speed changes; WASD flies at it. Ctrl+1, fly elsewhere, Shift+1: back. Home: back to you.
8. Check the editor's cursor: with a card in hand over the grass it should be the casting cursor, over a prop the
   hand. (SetCursor from an addon each frame: if it flickers or sticks, say so and it goes.)
9. F7 to play: Q, E, 1, 2, 3, G, R, L, F, Del and Ctrl+Z are the game's own again (try 1: the action bar).

## Live checks for the owner

Press, in Control:

1. **`den-lite`** builds Den Lite (the Rust changes; or `den-lite-test` / `den-lite-all` for its tests first).
2. **`addons`** publishes the Leyline addons (Gizmo.lua, Builder.lua).
3. **`den-move`**. Its new lines (and, after the glide, "card: the copy glides as one creature (no destroy / create)"
   and "select: the copy glides as one creature ..."; they say INFO until the new launcher has rebuilt the Den patch):
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
