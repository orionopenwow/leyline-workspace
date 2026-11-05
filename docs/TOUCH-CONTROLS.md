# Touch controls (phones and tablets)

The browser game's touch layer: `stream/www-files/touch.js` (the controls), `touch-driver.js` (the bridge, or the
default keys when the client has no bridge), `input.js` (pointer fixes). Build shown on the debug line:
`TOUCH_BUILD` in touch.js (now `wrath 2026-11-05b ...`). Deployed with the Control step **`page`** (the game page
only: backs up the live page, runs `www-patch.py`, checks `PAGE OK`, puts the old page back if the check fails).

It switches on by itself on a touch screen (a coarse pointer with touch points). `?touch=1` forces it on, `?touch=0`
off (both remembered in this browser).

## In the world

| Control | Where | What it does |
|---|---|---|
| Stick | bottom left (bottom right when Left-handed) | Walks. Classic: sideways strafes. Simple: sideways turns. Double-tap it: auto-run on or off. It floats to where the thumb lands in its zone. |
| Attack / Interact | the big round key, bottom right | Shows what a tap does with the target (sword, loot bag, speech bubble, quest marks, vendor, trainer...). No target: the nearest enemy. Enemy: attack. Anyone else: a hint (double-tap them). |
| Nearest enemy | the reticle on the ring, up and right of Attack | Tap: the nearest enemy, then the next nearest each tap (Tab). Hold: clear the target. |
| Four ability keys | on the ring round Attack | Tap: use the action (real icons, cooldowns, out of range red, no mana blue). Swipe them sideways, or tap the dots, for actions 5 to 8 and 9 to 12. Hold: its tooltip with Move and Remove. Something on the cursor (dragged out of the Spellbook or bags): the next key tapped takes it. |
| Jump | the pill under Attack | Held while the thumb is on it (swims up while held). |
| Tray | top right | Opens Bags, Quests, Chat and Menu along the top edge; folds away after 5 s. Lights up for whispers, party, raid and guild chat. |
| Close | top centre, when a window is open | Closes the open windows (Escape). |
| Stances, forms, pet bar | a short row along the bottom | Tap to cast; hold a pet action: autocast. A Leave button in a vehicle. |
| Cast bar | over the ring | Your cast filling, or your channel draining, with the spell's name. |
| Ground spells | after a key whose spell needs a place | "Tap where it should land" at the top: the next tap in the world places it. The X cancels it. |
| Target frame | top left, next to your own | Swipe it left: next enemy. Swipe it right: last enemy (Shift-Tab). A tap still clicks it. |
| The rest of the screen | | Tap: click (target, buttons, windows). Double-tap: right-click (talk, loot, open). Touch and hold: hover (tooltips). Drag: turn the camera (or drag a window). Pinch: zoom. |
| Soft lock | | A tap on open ground (no one under the finger) keeps your target, where the game would clear it. On by default. |

The game's own frames are rearranged for a small screen: player and target frames along the top left, the minimap
top right, the quest tracker under one of them, action bars and chat hidden (the ring and the chat sheet replace
them). All put back when touch is turned off.

## On the character screens

Login, realm list, character list and character creation take taps as mouse clicks:

- **Snap:** the red buttons (Enter World, Create New Character, Accept, Back...) are found on screen; a tap just
  above or below one clicks its centre (never sideways onto a neighbour).
- **Button magnet:** a tap within 10 px of a button's edge, where the game says nothing is under the cursor, clicks
  the button's centre. With something under the finger the tap stays where it is. On character creation it never
  moves a tap sideways: only up or down onto a button the finger is already over.
- **Loupe:** touch and hold shows what is under the finger, magnified; slide and lift to click there.
- **Ring:** every click flashes a small ring where it went (green: snapped or magnet).
- **Keyboard button** (top right) for typing a name; the **Controls button** next to it opens Menu > Controls
  (so the tap test and the debug copy work before you are in the world).
- A phone held upright is asked to turn sideways; sideways, one tap goes full screen ("Not now" skips it).
  "Add Leyline to your home screen" installs it (full screen, landscape, the Leyline icon).

## Menu > Controls

| Setting | What it does |
|---|---|
| Classic / Simple / Keyboard & mouse | How the stick and drags behave; Keyboard & mouse turns touch off (a corner button brings it back). |
| Tap test | Five gold rings, one at a time; tap the centre of each. Says how many hit (within 22 px), how far and which way your taps land, the pointer factor and, once the game answers, how far the game's own cursor lands from your finger. Nothing is clicked in the game. Copy result puts the debug line (with the test) on the clipboard. |
| Copy debug info | Copies one line for us: the build, the screen, bridge or keys, pixel ratio, buffer and page sizes, full screen, the factor, the tap mode, buttons found, taps, dropped and re-sent releases, the game update length, the layout, the last 10 taps with timings, errors, the tap test result, the browser. |
| Edit layout | In the world: drag the stick, the ring (by Attack, a key, the reticle or the dots), Jump and the tray. Ring − / + resize the ring. Phone / Tablet start from a preset, Reset goes back to it, Done saves. Saved per character (on this device). Copy code gives a short layout code (`L1` + 18 letters and digits); paste one and Use code to load it (another phone, a friend's layout). Everything is kept on screen. |
| Layout: Auto / Phone / Tablet | Auto picks Tablet on a screen 600 px or more on its short side. Tablet: a bigger ring, both thumbs a little higher. On a short screen a preset gives up its lift, then its size, so the ring clears the tray. |
| Left-handed | The stick on the right, the ring on the left (the tray stays top right). |
| Vibration | The short buzz on each press, on or off. |
| Soft lock | On: a tap on open ground keeps the target. |
| Bigger interface | A slider for the game's interface size (70% to 140%). |

A controller connecting hides the touch controls and hands over to the bridge's gamepad mapper. The first time on a
device a "Controller connected" card shows what each button does (the mapper's own list when the bridge gives one,
else a standard layout in `PAD_MAP`; to be checked against the mapper once it is in the client). Unplug it and touch
comes back.

## The debug line (`?touchdebug=1`)

A green block bottom left:

```
touch <build> · <screen> · bridge|keys · full screen
dpr <pixel ratio> buffer <canvas buffer> css <canvas css size>
factor <x>,<y> buttons <found> taps <n> mode <tapmode> raw
tap 400,300 -> 402,296 snapped          (or "dropped: <why>", "magnet")
game took: d-h u-h ...  errors <n>      (h: the game handled the event, -: it did not)
last: <the last 3 taps>  frame <ms>ms resent <n>
```

A tap in "last": `how x,y>px,py f<finger ms> h<press to release ms> R` (R: the release was sent twice). `how` is one
of tap, snap, magnet, aim (the loupe), drag (a rolled tap), window, right (double-tap), kept (soft lock), ground,
swipe, dropped.

Other switches for finding what a phone accepts: `?tapmode=pointer` (default), `both` (each pointer event followed by
its mouse event), `touchid` (press and release as the finger's own pointer); `?tapscale=<n>` forces the factor;
`?tapframe=<ms>` forces the game update length (every release is re-sent when it is longer than the gap);
`?taptest=1` opens the tap test at once.

## Why taps were missing, and what changed (5 Nov)

The ring showed in the right place but the game did not press. Three fixes and two tools:

1. A press and a release sent closer together than one game update can be read in the same update, and the game
   clicks nothing. The page now measures the update length (page frames, the bridge's answers); a release sent
   within it is sent again after one more settle.
2. The button magnet (above) for near misses where the game hovers nothing.
3. Every tap is recorded with its timings, so a report says what happened.
4. The tap test measures the finger and the game's cursor on the player's own phone.
5. Copy debug info: one line to paste to us.

## Test on your phone

Open the game on the phone (landscape). Add `?touchdebug=1` to the address if you can.

1. **Character list.** Tap the Controls button (top right, next to the keyboard) > **Tap test**. Tap the centre of
   each of the five rings. Expect: "5 of 5 hit", taps a few px off. Note what it says about the game.
2. Tap **Copy result** and paste it to us in Discord (whatever the result).
3. **Close**, then tap **Enter World** and the character names once each. Expect: each tap clicks the first time.
   If a button only lights up, tap **Copy debug info** in Controls right after and send it.
4. **Character creation:** tap a race, a class, the arrows, Accept. Expect: each one goes where you tapped, never
   the box next to it.
5. **In the world:** walk with the stick, double-tap it (auto-run), Jump.
6. Tap the **reticle**: an enemy is targeted; again: the next one. **Swipe the target frame** left and right.
7. Tap open ground with a target: the target stays (soft lock). Tap a mob: it is targeted.
8. Cast something with a cast time: the **cast bar** fills over the ring. Put a ground spell on a key (an area spell such as
   Flare, Rain of Fire or Death and Decay), tap it, then tap the ground: it lands there. Try the X once.
9. Menu > Controls > **Edit layout**: drag the ring somewhere else, Ring +, Done. Reload the page: it is still there.
   Copy code and send it if you like it. Try **Left-handed** once.
10. If you have a controller: connect it. Expect the "Controller connected" card and the touch controls gone;
    unplug it, they come back.

What to send us: the tap test's Copy result line, a Copy debug info line from any screen where a tap did nothing,
the phone and browser, and a screenshot if something sits in the wrong place.

## Tests

`NODE_PATH=$(npm root -g) node --test stream/tests/touch-*.browser.test.cjs` (headless Chromium with touch, a fake
page and a fake bridge, `stream/tests/touch-harness.cjs`):

- `touch-taps`: one scripted click per tap, focus kept, a stray mouse move, a real mouse on a touch PC, Boss fights.
- `touch-taps2`: the release re-send, the button magnet (and no sideways magnet on creation), the tap log, Copy
  debug info, the tap test with and without the bridge, `?taptest=1`.
- `touch-combat`: nearest enemy (bridge and Tab), target frame swipes, soft lock, the cast bar, ground spells.
- `touch-layout`: six screen sizes x Auto / Phone / Tablet x right / left-handed: every control on screen, 6 px
  apart, the keys evenly on one ring; snapping rules on the character screens.
- `touch-edit`: Edit layout (drag, clamp, resize, per character, reload, reset, codes), the Controls toggles and the
  slider, the controller card.
- `touch-fullscreen`: full screen on the lift, the home screen button, Not now.

Also `python3 stream/tests/test_brand.py` (no old names in the strings people see).
