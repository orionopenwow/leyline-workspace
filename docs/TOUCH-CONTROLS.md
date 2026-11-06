# Touch controls (phones and tablets)

The browser game's touch layer: `stream/www-files/touch.js` (the controls), `touch-driver.js` (the bridge, or the
default keys when the client has no bridge), `input.js` (pointer fixes). Build shown on the debug line:
`TOUCH_BUILD` in touch.js (now `wrath 2026-11-06a ...`). Deployed with the Control step **`page`** (the game page
only: backs up the live page, runs `www-patch.py`, checks `PAGE OK`, puts the old page back if the check fails).

It switches on by itself on a touch screen (a coarse pointer with touch points). `?touch=1` forces it on, `?touch=0`
off (both remembered in this browser).

## Phone test (owner)

About 5 minutes. Phone held sideways, the game open in the phone's browser (or from the Leyline home-screen icon).

1. **The build.** On the character list tap the **Controls** button (top right, the sliders icon) > **Copy debug
   info**, and paste it anywhere (a note, Discord's box). It must start `Leyline touch wrath 2026-11-06a`. If it says
   `05b`, close the tab, open the game again and check once more.
2. **Tap test.** Same sheet > **Tap test**: tap the centre of the 5 gold rings. Expect "5 of 5 hit". Close.
3. **Name and keyboard.** Create New Character. Tap a race and a class: each goes where you tapped. Tap the
   **keyboard** button (top right): your phone's keyboard comes up, and a dark chip at the top says "Type with your
   keyboard". Type a name such as `Testbob`: the chip and the game's name box show it once, no doubled letters.
   Backspace once: one letter goes. Tap the keyboard button again: the keyboard goes away. (No need to create.)
4. **Move and look.** In the world: walk with the stick, double-tap the stick (auto-run on, again: off), hold Jump.
   Drag the world: the camera turns. Pinch in and out: it zooms, and your target does not change when you let go.
5. **Target and fight.** Tap the reticle: the nearest enemy. Tap a mob: it is targeted. Tap Attack.
6. **Two-finger right-click.** Tray (top right) > **Bags**. Put one finger on a usable item (food, a potion, a piece of
   gear) and tap a second finger anywhere near it at the same moment: the item is used or equipped (a right-click).
   Close.
7. **Chat.** Tray > **Chat**: the keyboard comes up by itself and the chat box sits just above it, not under it.
   Send `hi`. Close.
8. **Away and back.** Walk forward with the stick, and while still holding it swipe to the home screen (or pull the
   notifications down). Come back: your character is standing still, not running off.
9. **Settings.** Menu > Controls: drag **Button size** to 120% and **Opacity** to 60% (the controls grow and fade).
   Tap **Touch and hold** until it says right-click, then hold a finger on a bag item for half a second: it is used.
   Set the three back as you like them.
10. **Send.** Menu > Controls > **Copy debug info** once more and paste it to us, with your phone model, the browser,
    and whether you played from the home-screen icon. For any step that did not do what it says: its number, what
    happened, and a screenshot if something sits in the wrong place.

In the pasted line: `letgo` should be 1 or more after step 8, `two` counts the two-finger right-clicks, `kb` is the
keyboard's height the last time it was open, `safe` is the notch, `haptics` says how this phone buzzes (`vibrate`,
`switch` on an iPhone, or `none`).

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
| The rest of the screen | | Tap: click (target, buttons, windows). Double-tap: right-click (talk, loot, open). Touch and hold: hover (tooltips), or a right-click with Touch and hold: right-click. Drag: turn the camera (or drag a window). Pinch: zoom (no click when the fingers lift). |
| Two-finger tap | anywhere, windows too | A right-click under the first finger: use, equip or sell a bag item, talk, loot. Both fingers down and up quickly, without zooming. |
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

## Phone keyboard, background, zoom, notch (5 Nov night)

- **Name box:** the keyboard button focuses a hidden box when the finger lifts (a phone raises its keyboard only for a
  focus inside a user gesture; a finger's press is not one). Each change of that box goes to the game as its
  difference, so a keyboard that composes words (Android's) or autocorrects sends each letter once. A chip at the top
  shows what was typed while the keyboard covers the game's box. Enter sends Enter and puts the keyboard away.
- **Chat:** Tray > Chat focuses its box inside the tap, so the keyboard comes up; the sheet is lifted above the
  keyboard (the visual viewport) and kept to the visible height. When a box lets go, a page that iPhone Safari left
  scrolled is put back.
- **In the background** (another app, a call, the lock button): the stick, Jump, a camera drag and a pressed window
  button are let go, and the screen lock is asked for again on return. The action poll, the layout upkeep and the
  debug line pause while the page is hidden.
- **Lost lifts:** a first finger while others are still counted (the browser dropped a lift) clears them, so the
  stick never jams and a tap is never taken for half a pinch.
- **No page zoom:** while touch is on the viewport tag gets `maximum-scale=1,user-scalable=no` (Android), iPhone's
  gesture events are cancelled, the panels only pan, every text box is 16 px or more, no pull-to-refresh.
- **Notch:** the corner buttons (keyboard, Controls, the touch button, the debug line) and the game's own player
  frame, target frame, minimap, tracker and buffs keep clear of a side notch.
- **No bridge (the live client today):** the screen-switch button no longer covers the open tray's Bags, nor hides
  under the Controls button on the character screens.

## Menu > Controls

| Setting | What it does |
|---|---|
| Classic / Simple / Keyboard & mouse | How the stick and drags behave; Keyboard & mouse turns touch off (a corner button brings it back). |
| Tap test | Five gold rings, one at a time; tap the centre of each. Says how many hit (within 22 px), how far and which way your taps land, the pointer factor and, once the game answers, how far the game's own cursor lands from your finger. Nothing is clicked in the game. Copy result puts the debug line (with the test) on the clipboard. |
| Copy debug info | Copies one line for us: the build, the screen, bridge or keys, pixel ratio, buffer and page sizes, full screen, the factor, the tap mode, buttons found, taps, dropped and re-sent releases, the game update length, the layout, the last 10 taps with timings, errors, the tap test result, Button size, Opacity, the hold mode, the notch, the visual viewport and the keyboard's height, the orientation, how the phone buzzes, two-finger taps, lost lifts and let-gos, the browser. |
| Edit layout | In the world: drag the stick, the ring (by Attack, a key, the reticle or the dots), Jump and the tray. Ring − / + resize the ring. Phone / Tablet start from a preset, Reset goes back to it, Done saves. Saved per character (on this device). Copy code gives a short layout code (`L1` + 18 letters and digits); paste one and Use code to load it (another phone, a friend's layout). Everything is kept on screen. |
| Layout: Auto / Phone / Tablet | Auto picks Tablet on a screen 600 px or more on its short side. Tablet: a bigger ring, both thumbs a little higher. On a short screen a preset gives up its lift, then its size, so the ring clears the tray. |
| Left-handed | The stick on the right, the ring on the left (the tray stays top right). |
| Vibration | The short buzz on each press, on or off. 12 ms where the phone has a vibration motor for pages (Android); an iPhone (iOS 18 and later) gets Safari's switch tick instead. |
| Touch and hold | Tooltip (the hover, as before) or Right-click (half a second on an item, a unit or a button right-clicks it, with a buzz; a window button is then not shown pushed while the finger is on it, and a quick tap still clicks). Not while placing a ground spell. |
| Keyboard hides touch | On (default): a hardware keyboard moving you (W A S D, Q E, arrows, Space) hides the stick and the ring until the next touch; the tray stays. |
| Button size | 80% to 140%: the stick and the ring (an edited ring keeps its own size; as big as fits on a short screen). |
| Opacity | 30% to 100%: the controls over the world (a pressed key shows at full strength). |
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

## Longer phone test (tap problems)

The 5 Nov tap checklist, for when taps on the character screens miss. Open the game on the phone (landscape). Add
`?touchdebug=1` to the address if you can.

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
- `touch-mobile`: an iPhone with its notch, an Android phone and a tablet, landscape and portrait (every control inside
  the safe area, the game frames moved in, one scripted click per tap, the turn-sideways cover, a turn mid-game); the
  composing keyboard; focus inside the lift; the chat sheet over the keyboard; pinches; the two-finger tap (world and
  a bag item); Touch and hold: right-click; the page hidden; lost lifts; a hardware keyboard; size and opacity;
  vibration and the iPhone switch; no page zoom; the debug line; the tray and corner buttons with no bridge; the
  installed app.

Also `python3 stream/tests/test_brand.py` (no old names in the strings people see).
