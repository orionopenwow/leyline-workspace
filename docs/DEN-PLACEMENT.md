# Your Den: placing things with the mouse (assessment and fixes, 4 Nov 2026)

Owner, 4 Nov: "completely assess and fix the move mouse to place objects". This is the whole chain from the cursor to
the prop on the ground in Den Lite, what was wrong in each link, and what changed (branch `agent/gr-fix-den`). The
Rust changes were not compiled here (crates.io is blocked in the cloud): press **`den-lite-test`**, then **`den-move`**
and the hover calibration on the PC.

## The chain

```
Gizmo.lua (the game)                                   Den Lite (gizmo.rs, pick.rs)
  G.Cursor(): u, v in -1..1, aspect = w / h   ──hover u v aspect zoom──▶  Aids::view(): the camera the ray starts from
  G.zoom: the wheel, counted (G.Pose)          ──cam zoom n──────────▶    (player + zoom, or the far view's focus +
  BUTTON1 over a prop: ours (LeylineGizmoGrab)                             zoom + pull-back, where the object is now)
  a still click: `click ... carry`             ──click / drag──────▶      ray() → pick::pick (prop boxes) / on_ground
  the wheel, Q / E: `preview turn`                                         hover_at(): grid snap, the Den's edge
  `edge`, `hovered`, `carried`, `aim` ◀──────────────────────────────    draw(): the copy, at most every DRAW_MS (33)
den_check.py: hover calibration fits fov, pivot, pitch from exact clicks (G + Flare) → den-lite.camera.json
```

## What was wrong, and what changed

| # | Link | Problem | Fix | Where |
|---|---|---|---|---|
| 1 | Pickup | A click put the foot where the ray met the ground, behind the spot clicked on the prop's body: a 3 yd prop seen at 30 degrees jumped about 2.5 yd | The foot keeps its offset from that ground point (at most the prop's turned footprint, so a click high on a tall prop seen from the side doesn't hold it far ahead); the copy starts exactly where the prop stood | gizmo.rs `drag`, `carry_offset`, `hover`; den-move "picked up without a jump" |
| 2 | Wheel | Turned a dragged prop but zoomed a carried one (and a gallery card) | The wheel turns anything in hand; Shift+wheel 5 degrees; Ctrl+wheel zooms | Gizmo.lua `G.Wheel` |
| 3 | Keys | Q had three owners (Gizmo turn, Den home's stamp turn, edit mode's hold-Q gallery), all priority override bindings: the last bound won | Q / E always turn what you hold or have selected; while Den home's stamp mark is out (or edit mode's prop waits for its click) the Gizmo lets Q / E go and Den home re-asks it; the browser's hold-Q gallery only with nothing carried (the client's own rule) | Gizmo.lua `G.OwnsTurnKeys`, DenHome.lua `H.UpdateKeys` |
| 4 | Arrival | Build mode began in the far build view (`cam build`), where W A S D fly the camera: you couldn't walk on arrival | Build mode starts at your character (the editor still opens by itself, DEN.md 3 Nov; owner 3 Nov: "infinite camera zoom out from character"); the far view only when you wheel out past the game's end or press the snap key; zooming back in returns. The tag says "WASD walks · wheel out past the end for the far view" | Gizmo.lua `G.CameraIn`, `showTag` |
| 5 | Camera turn | The 3.3.5a API has no camera-yaw call (only `GetPlayerFacing`). A left-drag (CAMERAORSELECTORMOVE) turned the camera alone: Den Lite's ray (near view: the player's facing) and the flight's forward stayed on the old facing, and the code snapped the camera back after each drag | In build mode BUTTON1 over bare ground is TURNORACTION (a right-drag's turn): you turn with the camera, so the facing is the view's. After a camera drag only the pitch is put back (SetView 2 is kept relative to you); in the far view the new facing is sent at once. A turn is told from a click by the facing (the cursor stays still in mouselook). Not over a unit or a named object (a click there targets / uses), checked each frame | Gizmo.lua `G.TurnsWithCamera`, `G.MouseUp` |
| 6 | Far view | While flying, Den Lite cast the ray from the glide's target, 450 ms ahead (about 7 yd at flight speed): the copy and the glow were off while moving | The ray starts where the camera object is at that moment of its glide | gizmo.rs `Aids::view`, `Aids.clock` |
| 7 | Calibration | Samples kept the player's place and the zoom, but since the build view the ray is cast from the far focus at zoom + 14 yd: a calibration in the build view (where `gizmo build on` puts it) fitted a wrong camera | Samples keep the pose the ray was cast from; den-check accepts distances up to the far view's 1500 yd | gizmo.rs `sample`, den_check `camera_error` |
| 8 | Window shape | The field of view is fitted as a diagonal one; if the game keeps it another way, a calibration at 16:9 is off at 4:3 | The calibration keeps its aspect in `den-lite.camera.json`; den-move notes a different window shape | den_check `calibrated_aspect`, `aspect_note` |
| 9 | Arrows | X / Y arrows placed by the model's own half sizes: a long prop turned 90 degrees had its X arrow inside it (and unclickable behind the prop) | Placed outside the prop's turned (world) box, for drawing and for the hit test | gizmo.rs `world_extent` |
| 10 | Den edge | Carry stopped at the edge with no word; an arrow drag stopped silently | `edge 1 / 0` to the addon; the tag says "Edge of your Den: it stays inside" | gizmo.rs `edge_note`, Gizmo.lua |
| 11 | Use while carrying | A click on another prop while carrying could also reach Den Lite as a use (right-click select): a new selection and menu under the drop | Den Lite ignores a use while something is in hand | gizmo.rs `use_object` |
| 12 | Core | One panic while a lock was held poisoned it, and every `lock().unwrap()` then stopped the Den for everyone | Poison-tolerant `lk()` everywhere in world/login/lib; a `panic` line in den-lite.events | lib.rs `Lock`, `note_panics` |
| 13 | Bags | The bags question ("How would you like your bags?") came up over the editor on the first Den visit | Never in Your Den; it waits for a real realm | LooseBags Choice.lua |

## 5 Nov: the owner's live test

See **DEN-EDITOR-PARITY.md** for the details:

- **Props never take the game's click in build mode.** They are sent with `GO_FLAG_NOT_SELECTABLE`; the game had said
  "You are too far away." A click on a prop picks it up into the glide, and a still right click opens its menu.
- **A picked card's copy appears at once**, and a card can be dragged out of the gallery and dropped.
- **The cursor's ring** shows where a click lands and what it does (place, grab or nothing).
- **Carried props sit on what is under them.** Q / E turn the prop under the cursor, and the turn carries to the next
  card.
- **The far view flies like Noggit:** forward along the view's pitch, Alt slow, Q / E up and down. The camera's pitch
  stays after a drag where the game lets the addon read it.

### The second live test (5 Nov)

- **The copy in hand glides.** It is a creature wearing the prop's own model (the launcher's Den patch gives every prop
  model a creature row, 70000 + its display, from the player's own tables), made once and glided with splines: no
  destroy and create while it moves. Buildings keep the gameobject copy, at most every 150 ms. See
  DEN-EDITOR-PARITY.md, "The second live test".
- **Hover lights the prop** under Den Lite's ray: the sparkle and a light under it; the selection keeps the stronger
  halo and its arrows.
- **A / D slide** in build mode at your character (they turned you and the camera round).

## Checked and fine

- **UI scale and aspect.** `G.Cursor` divides by UIParent's effective scale and size, so u, v are right at any UI
  scale or window shape (test at 0.64 and 5:4). Off the window: nothing is sent.
- **Clicks vs drags.** 0.3 s and 6 px; a quick flick is a drag; a lost mouse-up is caught by `IsMouseButtonDown`.
- **Picking.** Real model boxes with a margin and a minimum size; ties don't depend on list order (pick.rs).
- **Latency.** Hovers 30 a second while holding something; the copy glides to each new spot at most every 33 ms (one
  creature, 5 Nov), or, for a building, is made under a second guid before the old one goes, at most every 150 ms.
- **Slopes on the Den's own ground.** `on_ground` marches the ray over the edited heights, so a carried prop climbs
  hills made in Forge.

## Still open (not fixed here)

- **Walking while carrying in the near view.** Den Lite knows the player's place from movement packets: start and
  stop come at once, but while walking only a heartbeat every 500 ms. The copy lags up to about 3.5 yd while you walk,
  then is exact when you stop. Fix: extrapolate `here` in world.rs from the last packet's flags (forward, back, strafe,
  the `.den speed` factor) and its time, capped at 0.6 s.
- **Camera collision and the zoom's ease.** The game pulls its camera in when something is between it and you, and the
  wheel's zoom eases over a few frames; Den Lite assumes the counted zoom. Rare on the open island; noticeable indoors.
- **Hover glow during a fast sweep.** At most one glow change per 100 ms; a change inside that is dropped and the next
  hover decides again, so the glow is one prop behind during a sweep and right once the cursor rests (the settle hover).
- **Away from home** (other agent's area: places): the ground is flat at the spot's height; props float or sink in
  cities.
- **The far view's pitch** is fixed (no orbit up or down); a frame-the-selection key (F) and smooth zoom are
  suggestions in review/den.md section 2.

## Live checks for the owner (after `den-lite-test`)

1. Launcher > Your Den > **Open my Den**. You arrive in Build mode (gold frame) at your character: W walks. No bags
   question.
2. Left-drag on the ground: you turn with the camera; let go and point at a prop: it glows where you point.
3. Click a chair: it lifts where it stands (no jump), follows the mouse; the wheel turns it; Ctrl+wheel zooms; click
   to put it down.
4. Carry it past the Den's edge: it stops and the tag says "Edge of your Den".
5. Wheel out past the end: the far view; W A S D fly where you look, Space / X up and down; carry a prop while flying:
   the copy stays under the cursor. Wheel back in: your character again.
6. Mark a stamp in Den home, press Q: only the stamp turns.
7. Control: `den-move` (the new "picked up without a jump" line), then the hover calibration once (the build-view fix).
