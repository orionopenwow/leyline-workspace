# Boss fights

Leyline's instant boss fights. The featured fight is the last fight of the citadel, which players see as
**The Frozen Throne**. The boss is never named on screen. Other sections (the realm, the board) are written
separately.

## In-game panel

The addon side is `stream/fui-web/Interface/AddOns/WrathUnleashedUI/BossFights.lua`, listed in the `.toc` after
`BotBar.lua`. It uses the editor look (`Style.lua` `S.Ed*`: near-black, with gold for the current pick).

**Ways to open it:** `/bf` or `/bossfights` (`/bf leave`, `/bf status`, `/bf go` and `/bf queue` also work). On
Leyline realms there is also an **Esc > Boss fights** button (`GameMenuButtonLeylineBossFights`, placed by
`W.LayoutGameMenu`). On phones it is the tray's **Menu > Boss fights** (`stream/www-files/touch.js` `MENU`). That
button runs `WrathUnleashedUI.BossFights.Show(true)` through the bridge's Lua, or types `/bf` when there is no
bridge.

**The window** (`LeylineBossFights`) has:
- The Frozen Throne (one tile per fight; the picked one is gold);
- Normal / Heroic;
- **Go now with companions**, which sends `join <boss> <diff> bots`;
- **Queue with players**, which sends `join ... group`;
- **Players only**, which turns Queue into `join ... group-only`.

The buttons are off while you are already in a fight. On another server they are off, and the window says
"Boss fights are on Leyline realms." The boss tile's tip is `TODO(owner)` copy.

**The bar** (`LeylineBossFightBar`) shows the fight and difficulty, what is happening, the live timer and
**Leave**. It sits at the top middle under the row of unit frames (y -108), clear of the player frame, the target
frame, the buffs and the minimap, and away from the raid frames on the left and the boss frames on the right.
`DenLayout.lua`'s slots belong to the Den editor, and nothing is docked outside the Den. Inside the Den, the bar
sits under the strip and the toast stack. With touch controls on, it sits under touch.js's unit frames. The bar
stays from the join until `L`.

**The card** (`LeylineBossFightResult`):
- **Clear:** CLEARED, the fight and difficulty, the time, the realm's hint, "See the board:
  leylinelauncher.com/wrath/bossfights", and the address in a box to copy. The buttons are Done and Leave.
- **Wipe:** WIPED and the time. The buttons are **Try again** and **Leave**. Try again re-sends the same `join`,
  and the realm keeps you in the instance.

**Commands.** These are typed as chat (`SendChatMessage(".bossfight ...", "SAY")`, as `.bug` is):
`.bossfight join <boss> <normal|heroic> <bots|group|group-only>`, `.bossfight leave`, `.bossfight status`.
If a join gets no answer within 8 s, the bar gives up and says so in chat.

**Realm to addon.** Addon messages with prefix `LYBF`, whispered to yourself (lines from anyone else are
ignored), tab-separated:

| Line | Panel |
|---|---|
| `Q boss diff mode waiting needed eta_s` | Queued. `group`: "Waiting for players... bots fill the rest in ~Ns" (counts down here, then "any moment"). `group-only`: "Waiting for players...". `bots`: "Your companions are getting ready...". `waiting` and `needed` are never shown. |
| `S boss diff` | "Starting: on your way in..." |
| `T ms` | The timer is reset to the realm's time and runs here between ticks (mm:ss.t). |
| `R clear\|wipe ms hint\|-` | The timer stops at `ms` and the card opens. The hint is plain text: colour codes are removed, and a hint with a game-brand word is dropped. |
| `L` | Back to idle. The bar and the card close. |

Bad lines (unknown kind, bad difficulty or mode, non-numeric times) are ignored. The boss id → name table is
`BF.BOSSES` (`frozen-throne` → "The Frozen Throne"). An unknown id shows "Boss fight".

**Reload.** At login and on `/reload`, `.bossfight status` is sent once the realm's hello marks it as a Leyline
realm. It is sent again after every later loading screen. The realm's answer (`Q`, `S`, `T` or `R`) puts the bar,
the timer and the card back. The last pick (fight, difficulty, mode, Players only) is kept per character in
`WrathUnleashedUIChar.bossfights`, so Try again works after a reload.

**Phones.** touch.js sets `__OWTOUCH.applied` and `__OWTOUCH.cssH`, so UIParent's height divided by `cssH` gives
UI units per CSS pixel. The window, the bar and the card are each scaled so their smallest control (36 units)
is at least 44 CSS px, capped at the screen's width. The bar picks up the new scale within a second when touch is
turned on or off.

**Tests.**
- `texlua stream/tests/test_wuui_bossfights.lua` covers every message type, the timer, the ETA, a reload while
  engaged and while queued, another server, the Esc button, the bar's place, the 44 px rule at phone scale, and
  no game-brand word in any text the panel shows or in its strings.
- `NODE_PATH=$(npm root -g) node --test stream/tests/touch-taps.browser.test.cjs` covers Menu > Boss fights on both
  paths and the button's size.

**Deploy.**
- Browser game addon: Control `foreverui`, which copies `stream/fui-web` into the stream.
- Desktop addon download: `addons`.
- touch.js: `page`.
