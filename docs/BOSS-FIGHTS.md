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

## Website

The site side is `stream/bossfights.py`, loaded by `stream/wrath_portal.py` beside `duels.py`. Its data (the
fight's name, raid, size, tier and platform spot, what comes next, the arena queue) is in `stream/modes_data.py`
(`BOSS_FIGHTS`, `BOSS_FIGHTS_NEXT`, `ARENA_QUEUE`). These are not ov_go.lua places, so the generated `PLACES`
table in `ov_go.lua` does not change.

### Pages

| Address | What it is |
|---|---|
| `/wrath/bossfights` | The Frozen Throne. It has the hero (the tagline is `TODO(owner)`), Normal / Heroic (`?d=heroic`, plain links with no script), **Fight now with companions** and **Queue with players**. Then how it works (10 players, go now or queue, normal or heroic, the clock), the top 10 *With players* and *With companions*, and *Coming next* (Vault of Archavon, Onyxia, Sartharion, Halion). |
| `/wrath/arena` | Duels and arena. It has **Queue 1v1 arena**, the duel corner (`/wrath/h/duels`), the duel leaderboards and the 1v1 ladder. |
| `/wrath/api/bossfights` | The boards as JSON, for the launcher (CORS `*`, cached a minute). |
| `/wrath/leaderboards?board=bosses` (`&d=heroic`) | The **Boss fights** tab: with players, with companions, and this week. |
| `/wrath/leaderboards?board=arena` | The **1v1 arena** tab. Until an arena export exists it says the ladder is coming. |

Both pages are in the More menu as **Boss fights** and **Duels and arena**, after Leaderboards. `/wrath/modes`
shows The Frozen Throne first under Boss fights (Fight now, Queue with players) and **Queue 1v1 arena** under
Duels.

The pages use the site's own art only (the Leyline crystal over an icy gradient). There are no game screenshots
and nothing hotlinked.

**Signed out:** every button goes to Log in first (`profiles.signin_url`), with `next=/wrath?go=<go>`. After
Log in the player lands back on the same go. A bare `/wrath?go=<go>` signed out keeps the go too
(`wrath_portal.go_signin`). There is one limit: when a Hero Key saved in the browser signs in by itself, the realm
site lands on plain `/wrath`, so the go is lost on that path only.

### Go kinds

| Go | Means |
|---|---|
| `bf:<boss>:<difficulty>:<mode>` | `<boss>` is `frozen-throne`; difficulty is `normal` or `heroic`; mode is `bots` (go now with companions), `group` (queue with players, companions fill the gaps) or `group-only` (players only). |
| `arena:1v1` | The unrated 1v1 arena queue. |

`bossfights.check_go` checks them, and `wrath-page.js` `goLink` checks them again (`GO_RULES.bf`,
`GO_RULES.arena`). The longest one, `bf:frozen-throne:heroic:group-only`, is 34 characters, under the go limit
of 40.

**What a signed-in player's go does:** the portal sends one SOAP console command,

    ovgo set <ACCOUNT> bf:frozen-throne:normal:bots
    ovgo set <ACCOUNT> arena:1v1

which is the same intent `ov_go.lua` keeps for the other go links (30 minutes, used once). It is sent at most
20 a minute per account, and the same one is sent once per 2 minutes. The portal logs a warning, and the page
still works, when the realm doesn't answer `ovgo ok`.

**For the realm agent:**
- `ov_go.lua` has to accept the two new kinds in `ovgo set`.
- At login it has to run `.bossfight join <boss> <difficulty> <mode>`, or `.q1v1 unrated` for `arena:1v1`.

**The mode catalog** (`stream/playmodes.json`, which feeds the launcher's Modes page and `/wrath/modes.json`) has
two new entries:
- `frozen-throne` ("Boss fights: The Frozen Throne");
- `arena-queue` ("1v1 arena queue").

Both are kind `realm` with `{"go": "mode:<id>"}`, so the launcher's schema and `modes.rs` take them as they are.
`playmodes.GO_AS` (the same as `bossfights.MODE_GO`) maps them to their go:
- The site's Play links the go directly.
- `?go=mode:frozen-throne` from the launcher's browser Play becomes `bf:frozen-throne:normal:bots`, both in the
  intent and on the way to the stream site.
- They are never sent as `ovplay go`.

The launcher's **desktop** Play writes the LeylinePlay addon, which sends `wuplay go frozen-throne` or `wuplay go
arena-queue`. For that path, `ov_playmode.lua` would map those ids to the same commands. `test_playmodes.py`
accepts that table with or without them.

### The boards' data

The file is `<WU_DUELS_DIR>/bossfights.json`. On the host that is `/srv/wrath/portal/duels/bossfights.json`, which
`wu-portal` sees as `/app/duels/bossfights.json`, the folder `duels-export.py` writes. The realm's
`realm/custom/bossfights-export.py` writes it:

    {"updated": "2026-11-05T19:00:00Z",
     "bosses": {"frozen-throne": {"normal": {"group": [row], "bots": [row], "weekly": [row]}, "heroic": {...}}}}
    row = {"rank", "ms", "time": "m:ss.t", "date": "YYYY-MM-DD", "leader", "roster": [{"name", "class", "bot"}],
           "size", "bots"}

**What the site reads, and how it checks it** (`bossfights.clean`):
- It reads only the known boss and the two difficulties.
- In a row it reads only `ms`, `date`, `leader`, `roster`, `size` and `bots`.
- `ms` must be 1 ms to 6 h. The shown time is worked out from `ms`; the file's `time` and `rank` are not trusted,
  and rows are re-ranked by `ms`.
- A name must be 2-12 plain letters and must not hold a brand word (`hub.banned`).
- `class` is a class id or a class name.
- Companions (`bot: true`) are counted, never named.
- A leader who isn't a shown player of the roster falls back to the first shown player. A run with no player to
  show is left out.
- A missing, broken or older-shaped file gives empty boards, never an error. The last good copy stays while a new
  one is being written.

**The 1v1 ladder** is optional: `<WU_DUELS_DIR>/arena.json`, in the form `{"updated": iso, "ladder": [{"name",
"class", "rating", "wins", "losses"}]}`. mod-1v1-arena's rated teams could feed it, through an export like
`armory-export.py`'s arena query. Until that file exists, the tab says the ladder is coming.

### Browser play

- **Carrying the go:** `go_param` carries `bf:` and `arena:` through `/wrath/play` and `/__gate` to the stream
  site, like the share-link kinds.
- **Warm by destination** (`stream/www-files/stages.js`):
  - With `window.wuGo` a boss fight, character select's `focus:<adt>` is told to the engine as the fight's own
    tile, `BOSS_TILES['frozen-throne'] = ['IcecrownCitadel', 35, 31]`. That tile is the platform's centre
    (631: 503.63, -2124.66) through `world_to_tile`.
  - `boot.js` then warms the streamed tier holding it (`icc-toc`) while the player is still picking a character.
  - Only `focus:` is changed. `port:` is the realm's real teleport.
  - Data saving: with Save-Data or a 2g/3g connection (`dataSaving`, the same rule as `ahead.js`), nothing extra is
    warmed.
- **The loading note** (`wu-loading.js` `addGoNote`): the loading cover and the boot overlay show one line above
  the tips: "Boss fight: The Frozen Throne — getting your raid ready" (with "(heroic)" when it is heroic). There
  is no seat count and no place in line.

### The slice

`stream/slices/mini.json` tier `icc-toc` (order 13 in mini, 9 in sampler) lists map 631 as a whole map, so the
Frozen Throne platform is streamed already. No new slice is needed.

That tier is large, though: it holds the whole citadel plus the Forge of Souls, Pit of Saron, Halls of
Reflection and Trial of the Crusader. A fight-only session pays for all of it the first time.

**Proposal (not built):** a small `frozen-throne` tier ahead of `icc-toc` in `order`:

    {"key": "frozen-throne", "kind": "raid", "name": "The Frozen Throne",
     "maps": [{"id": 631, "box": [380, 640, -2660, -1990]}], "marginTiles": 1,
     "notes": "The platform (503.6, -2124.7, 840.9) and the room inside the blade (495.6, -2517.0, 1050.0)."}

- `box` is [x min, x max, y min, y max] around the platform and the heroic room. The platform is on ADT 35_31,
  close to the line with 36; the room is on 36_31. `marginTiles` 1 takes the neighbours.
- The realm fence (`ov_slices.lua` `allowedMaps`) would then allow only that box of 631 for this tier. `icc-toc`
  after it still allows the whole map.
- Check with "Size report.bat" after "Make mini slice.bat". Also check in game that the teleport and the platform
  spawns are inside the box.
- `BOSS_TILES` stays right either way.

### Tests

- `python3 -I stream/tests/test_bossfights.py stream/wrath_portal.py`: the go kinds, the SOAP intent against a
  fake realm, Log in keeping the go, the data rules, the pages, the JSON, the tabs, the nav, `/wrath/modes`, no
  brand words, the deploy wiring and the slice.
- `node stream/tests/stages-dest.test.cjs`: the boss tile, data saving, and `port:` left alone.
- `node stream/tests/loading.browser.test.cjs`: the note on the cover.

### Deploy

- `bossfights.py` rides with the portal. The sign-in bat (`stream/Update sign-in and warm-up.bat`) packs it, and
  `stream/fastloop.sh` installs it, backs it up, puts it back on a failed check, and prints
  `/wrath/api/bossfights` in the self-check.
- That is Leyline Control's `signin` step. The `www-files` changes (stages.js, wu-loading.js and .css,
  wrath-page.js) go out with the `page` step ("Game page only").
