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

## Realm side

Leyline's "Boss fights" mode puts a 10-player raid straight onto the platform of the ICC raid's final fight. Players see
the fight as **The Frozen Throne** and never see the boss's own name. Each run happens in a fresh instance where every
earlier boss is already DONE. The clock is server-side, and the best times go on the website's boards.

| Piece | Where |
|---|---|
| The module (C++) | `realm/custom/modules/mod-leyline-bossfights` (`sources.lock`: `mod-leyline-bossfights local - -`) |
| Its pure helpers and their test | `src/BossfightsLogic.h`, `tests/test_bossfights_logic.cpp` (`g++ -std=c++17 -I../src test_bossfights_logic.cpp`) |
| Go link | `realm/custom/lua_scripts/ov_go.lua`, the kinds `bf:...` and `arena:1v1` (tests: `stream/tests/test_ov_go.lua`) |
| Launcher Play | `realm/custom/lua_scripts/ov_playmode.lua`, the modes `frozen-throne` and `arena-queue` |
| Boards export | `realm/custom/bossfights-export.py` (tests: `stream/tests/test_bossfights_export.py`) |
| Server pass | `realm/serverpass.sh`: step 1g (export and cron) and step 3 (the settings below) |
| New realm image live | Control step `realm-image`: `realm/Update realm image.bat` runs `realm/realm-image.sh` |

### Commands

Player commands (security 0). `.bf` is a short alias for the four player words.

| Command | Does |
|---|---|
| `.bossfight join <boss> <normal\|heroic> <bots\|group\|group-only>` | Starts or queues a run. The defaults are `frozen-throne normal bots`. See the cases below the table. |
| `.bossfight leave` | Leaves the queue or the run. You go back to where you joined from, and your binds to the map are removed. If the leader leaves, the run ends for everyone. |
| `.bossfight status` | Says where you are and sends the panel's current state again (`Q`, `S` or `T`). |
| `.bossfight list` | Lists the boss fights and the modes. |
| `.bossfight start <leader> <boss> <difficulty> <mode>` | GM or console. Starts a run for `<leader>` with their own group; bots fill the seats unless the mode is `group-only`. A start by a GM in game marks the run `gm`. A start from the console or SOAP does not. |
| `.bossfight reset <character>` | GM or console. Takes the character out of any run or queue, sends them home and unbinds them from map 631. |

What `join` does depends on the player:

- **Mode `bots`:** starts at once with the player and 9 bots.
- **Mode `group` or `group-only`:** puts the player in that boss and difficulty's lobby.
- **Leading a group of their own:** starts at once with that group. Bots fill the rest unless the mode is `group-only`.
- **In a group but not its leader:** refused. The leader starts it for everyone.
- **Already in a run for the same boss and difficulty, after a wipe or before the pull:** restarts in the same instance. Everyone is resurrected and put back in front of the paladin, and the group and bots stay. This is the in-game panel's "Try again".

`.bf` alone is also the core's battlefield (Wintergrasp) GM table: `start`, `stop`, `switch`, `timer`, `enable` and
`queue`. Sub-commands merge by name, so `.bf` only carries `join`, `leave`, `status` and `list`. A second `start` would be
a duplicate command, which stops the server at load.

**Conditions for `join`:**
- level 80 (`LeylineBossfight.MinLevel`);
- out of combat and alive;
- not in a battleground, an arena or their queue;
- not inside an instance.

**The lobby (`group` / `group-only`).** There is one lobby per boss and difficulty, and per faction if the realm doesn't
allow cross-faction groups. The first player in it is the leader.
- Ten players start at once.
- With fewer, a `group` lobby fills the empty seats with bots once the leader has waited `LeylineBossfight.LobbyWaitSec`
  (default 180 s).
- A `group-only` leader waits for ten players. That lobby closes after `LeylineBossfight.LobbyMaxWaitSec` (default 1800 s).
- With bots off, every lobby behaves like `group-only`.

### The start, the clock, wipes and the end

1. **Raid.** The module makes a raid group with the leader (or uses the leader's own group), adds the other players and
   sets the raid difficulty to 10 normal or 10 heroic. It removes every member's binds to map 631 (all four difficulties).
2. **Bots.** If bots are filling seats, the module adds them. They are mod-playerbots addclass bots. The default seats are
   2 tanks (warrior and paladin, protection), 3 healers (holy paladin, discipline priest, restoration shaman) and
   5 damage (unholy death knight, fire mage, affliction warlock, marksmanship hunter, combat rogue). Each real player
   takes the first seat of their role (worked out from class and the talent tree with the most points).
   - Each bot gets its seat's premade talents, then all its gear removed and regeared at one item level
     (`LeylineBossfight.BotGearLevel`, 251). Every bot fights with the same kit.
   - The start waits up to `BotWaitSec` (45 s) for the bots, then goes ahead without the ones that didn't arrive.
3. **Fresh instance.** The leader is teleported in first, which makes a new instance at the chosen difficulty
   (`TELE_TO_GM_MODE` skips the entry requirements, as the core's own teleport spells do).
4. **Earlier bosses DONE.** Every earlier boss is set DONE through the instance script: boss ids 0 to 11 and 13 (the
   Sindragosa gauntlet) in `icecrown_citadel.h`. That covers the Plagueworks (Putricide), the Crimson Hall (Blood-Queen)
   and the Frostwing Halls (Sindragosa) checks.
   - The call is the **base** `InstanceScript::SetBossState`. It sets the state, the doors and the save, but none of the
     raid script's extras: no gunship spawn, no unlocked loot chests, and the heroic "final fight available" flag stays as it is.
   - A boss whose creature never loaded (state `TO_BE_DECIDED`) takes `NOT_STARTED` first.
   - If a boss's AI resets its state (its creature loaded), it is set DONE again.
5. **Everyone in.** Everyone else is teleported to the platform, 12 yards east of the paladin, facing him
   (`BossfightsLogic.h`: 517.0, -2124.6, 840.86).
   - Until the pull, anyone off the platform comes back. That is more than 75 yards from its centre, below z 820, dead,
     or a released ghost outside.
   - Players start the fight by speaking to the paladin, as in the normal raid.
6. **Clock start.** The clock starts when the boss's state goes `IN_PROGRESS` (`GlobalScript::OnBeforeSetBossState`).
   The fight's own `JustEngagedWith` does this after it has checked the earlier bosses, so a refused pull never starts the
   clock. The roster is everyone in the instance at that moment.
7. **Clock stop.** The clock stops when the boss **starts casting spell 72350** (`AllSpellScript::OnSpellPrepare`, after
   the cast checks). That is the 10% cast that begins the outro, the same end point as the public ICC speedrun rules. If
   that cast is never seen, the boss's DONE stops it instead. The long outro is not timed.
8. **Wipe.** A wipe is the boss's own evade, which sets FAIL. It is recorded as a `wipe` row. The raid stays in the
   instance and `join` again restarts it there. A raid that never tries again is sent home after `WipeIdleSec` (900 s).
9. **After a clear.**
   - The bots are logged out 15 s after the clear.
   - The players are sent back to where they joined from after `ReturnAfterClearSec` (120 s), or earlier with `.bossfight leave`.
   - Their binds to map 631 are removed once they are out.
10. **Leaving.** A player who leaves the raid group or the instance (hearth, logout) is out of the run. A released ghost
    outside stays in and is brought back when the raid isn't fighting.
    - The leader leaving, logging out, or the start failing ends the run: bots out, everyone home, unbound.
    - If the boss was fighting at that moment, an `abandon` row is written.
11. **After a restart.** A player who logs in inside one of the module's old instances with no live run (after a server
    restart, say) is sent home. The instances are remembered in `leyline_bossfight_instances` for 14 days.

**Heroic caveat.** The raid script lowers its heroic attempt counter (50) on each heroic wipe. After 50 wipes in one
instance the boss is hidden; `.bossfight leave` and a new `join` give a fresh instance. A player who logs back in inside a
heroic instance without the heroic entry requirement may be moved out by the core's login check (unverified).

### Addon messages (the in-game panel)

The messages use the prefix **`LYBF`**. Each is a whisper from the player to themself, in `LANG_ADDON`. The client splits
the text at the first tab and fires `CHAT_MSG_ADDON` with prefix `LYBF`, distribution `WHISPER`, and the player as sender.
Fields are tab-separated:

| Message | When |
|---|---|
| `Q\t<boss>\t<diff>\t<mode>\t<waiting>\t<needed>\t<eta_s>` | Queued in a lobby. Sent on joining, whenever the lobby changes, and every 15 s. `mode` is the leader's choice (`group` or `group-only`). `waiting` is the real players in the lobby; `needed` is `10 - waiting`. `eta_s` is the seconds until bots fill the seats, or `-1` when there is no estimate (group-only, or bots off). |
| `S\t<boss>\t<diff>` | Starting. Sent at the start, again after a restart, and on `.bossfight status` or a login during a run that isn't fighting. |
| `T\t<ms>` | The timer: `0` at the pull, then every 5 s while fighting, and on `.bossfight status` or a login during the fight. |
| `R\t<clear\|wipe>\t<ms>\t<rank_hint or ->` | The result. `rank_hint` is this time's place among the eligible clears of the same boss, difficulty and kind (bots or not): the count of faster ones plus 1. It is `-` for a wipe or a GM run. |
| `L` | Left or cancelled: left the run or the queue, the run ended, or the lobby closed. |

In those fields:
- `<boss>` is `frozen-throne`;
- `<diff>` is `normal` or `heroic`;
- `<mode>` is `bots`, `group` or `group-only`.

### Go link and Play

- **`ov_go.lua`.** The portal records `ovgo set <ACCOUNT> <go>` (SOAP; the reply starts `ovgo ok`). There are two new
  kinds:
  - `bf:<boss>:<normal|heroic>:<bots|group|group-only>`, for example `bf:frozen-throne:normal:bots`. At the next login
    the character runs `.bossfight join <boss> <diff> <mode>` (`Player:RunCommand`).
  - `arena:1v1`. At the next login the character runs `.q1v1 unrated` (mod-1v1-arena).

  Both follow ov_go.lua's usual rules:
  - level 80 (a lower character waits, and reaching 80 starts it);
  - not in combat, dead or inside an instance (`.go` tries again);
  - a brand new character's first login waits for `.go`;
  - the intent is used once and lasts 30 minutes.

  The places (`h:`, `q:`, `b:`) work as before.
- **`ov_playmode.lua`.** The launcher's Desktop Play sends `wuplay go <id>`, which is the same as `.playmode <id>`:
  - `frozen-throne` runs `.bossfight join frozen-throne normal bots`;
  - `arena-queue` runs `.q1v1 unrated`.

  `stream/playmodes.json` must list both as live `realm` modes. The website branch adds them.
  `stream/tests/test_playmodes.py`'s "the table = the catalog's live realm modes" fails on this branch alone, and passes
  once both branches are in.

### The table

The table is `characters.leyline_bossfight_runs`. The module makes it at start, and so does
`data/sql/db-characters/base/leyline_bossfights.sql` and `bossfights-export.py --schema`. There is one row per attempt.

| Column | |
|---|---|
| `id` | auto |
| `boss`, `difficulty`, `mode` | `frozen-throne`; `normal` or `heroic`; `bots`, `group` or `group-only` (what was asked for) |
| `size`, `bots` | members in the instance at the pull; how many of them were bots |
| `ms` | the time in milliseconds: pull to the 10% cast (clear), to the evade (wipe), or to the leaving (abandon) |
| `started_at`, `ended_at` | unix seconds |
| `leader_guid`, `instance_id` | not exported |
| `roster` | `Name:class:bot\|player`, comma separated. Class words: `warrior paladin hunter rogue priest deathknight shaman mage warlock druid`. |
| `bot_buffs` | the setting `Playerbots.EnableICCBuffs` at the pull |
| `gm` | 1 if a GM (security above player, or GM mode on) was in the raid, or a member used a GM command during the run. Such runs never reach the boards. |
| `tier` | 0 for now (tiers above heroic come later) |
| `wipes_before` | wipes earlier in the same instance |
| `result` | `clear`, `wipe` or `abandon` |

`leyline_bossfight_instances (instance_id, boss, created_at)` holds the instances the module made.

### Boards export

`bossfights-export.py` runs every 5 minutes from cron (`/etc/cron.d/wu-bossfights`, installed by server pass step 1g).
It reads the tables in a READ ONLY session and writes two files into the folder the duel boards use,
**`/srv/wrath/portal/duels/`**. The portal sees them at `/app/duels/bossfights.json` and `/app/duels/arena.json`.

```json
{"updated": 1793700000, "week_start": "2026-11-02", "week_end": "2026-11-09",
 "bosses": {"frozen-throne": {
   "normal": {"group": [ROW...], "bots": [ROW...], "weekly": [ROW + "kind"...]},
   "heroic": {"group": [], "bots": [], "weekly": []}}}}
```

Each `ROW` looks like this:

```json
{"rank": 1, "ms": 312345, "time": "5:12.3", "date": "2026-11-05", "leader": "Orion",
 "roster": [{"name": "Orion", "class": "deathknight", "bot": false}, {"name": null, "class": "mage", "bot": false}],
 "size": 10, "bots": 9, "bot_buffs": false}
```

**The boards:**
- `group`: clears with no bots, the best time of each set of players (the same names count once).
- `bots`: clears with any bot in the raid, the best time of each leader.
- `weekly`: this week's clears (from Monday 00:00 UTC), either kind, the best of each leader. Each row has
  `"kind": "group"|"bots"`.

Each board is ranked by `ms`, at most 100 rows. Only `result = clear`, `gm = 0` and `tier = 0` runs count.

**Privacy:**
- character names and class words only (no guids, account names or ids);
- a character with `.armory off` shows as `"name": null`;
- a name that isn't 2-12 plain letters, or carries a brand word (`stream/hub.py BANNED_RE`), shows as null;
- a leader like that drops the row.

`arena.json` is `{"updated", "ladder": [{"name", "class", "rating", "wins", "losses"}]}`. It comes from mod-1v1-arena's
teams (`arena_team.type = 1`, the captain, at least one game this season), with RNDBOT accounts and opted-out characters
left out.

### Settings

The realm runs on environment settings: AzerothCore reads `AC_<KEY>` for every config key. The three below are in
`realm/profiles/main.env`, and server pass step 3 sets them live.

| Setting | Value | Why |
|---|---|---|
| `LEYLINE_BOSSFIGHT_BOTS` | `1` | The module may add bots, **only to its own raids**, even with `AC_PLAYERBOTS_MAX_ADDED_BOTS=0`. It raises the bot cap for the moment of its own add call and puts it back at once. The module logs its bots out when the fight ends, the leader leaves, or the start gives up. `0` means no bots: `bots` is refused and lobbies wait for ten players. **The owner's decision** (bots are off everywhere else since 31 Oct). The conf key `LeylineBossfight.Bots` (`AC_LEYLINE_BOSSFIGHT_BOTS`) does the same; the plain variable wins. |
| `AC_PLAYERBOTS_ENABLE_ICCBUFFS` | `0` | The bot module's heroic ICC "assist" auras (more damage, less damage taken, threat changes, for the whole raid, real players too) are off, so times compare. Each run records the setting anyway (`bot_buffs`). |
| `AC_AUTO_BALANCE_DISABLE_PER_INSTANCE` | `631` | No group-size scaling in ICC, so the fight is always a fixed 10-player fight (mod-autobalance counts bots as players). This applies to every ICC instance on the realm. |

The module's other keys (`conf/mod_leyline_bossfights.conf.dist`; defaults built in) are `LeylineBossfight.Enable`,
`MinLevel`, `LobbyWaitSec`, `LobbyMaxWaitSec`, `BotGearLevel`, `BotWaitSec`, `ReturnAfterClearSec`, `WipeIdleSec` and
`BotSeats`. The bots' premade talents come from `realm/playerbots-specs.env` (server pass step 3b).

### What the owner presses (Leyline Control)

1. **`ship`.** It saves to GitHub. The push to main changes `realm/custom/modules/**` and `sources.lock`, so GitHub's
   **wu-images** workflow builds new realm images with the module and publishes
   `ghcr.io/orionopenwow/wu-ac-wotlk-*:stable`. This takes an hour or more. `builds-why` shows why if it fails.
2. **`realm-image`** (new), once the build has passed. On the server it:
   - pulls the three `:stable` images (nothing changes if that fails, for example while the packages aren't public);
   - runs the backup (`wu-backup`) and keeps the running images as `:rollback`;
   - warns in game for 5 minutes, then restarts on the new images;
   - checks that the world server says `ready...` and `mod-leyline-bossfights: on (...)`;
   - if anything is wrong, puts the `:rollback` images back and restarts on them by itself.

   `"Update realm image.bat" now` skips the warning.
3. **`realm`** (the server pass, a 5-minute restart). It:
   - uploads `ov_go.lua`, `ov_playmode.lua` and `bossfights-export.py`;
   - makes the two tables;
   - installs the export and its cron, and runs it once;
   - sets the three settings above in the live profile.

After that, test in game with a level-80 character: `.bossfight join frozen-throne normal bots`, then speak to the
paladin, and check that the timer runs and the result line appears. Then check `.bossfight status` and `.bossfight leave`.
The website shows the boards from `/app/duels/bossfights.json` within 5 minutes.

Without the new image, the realm has no `.bossfight` command. In that case ov_go.lua's `bf:` intent runs a command the
core doesn't know (the player sees the core's "no such command"), and the export writes empty boards.

### Not verified yet

- The module has not been compiled into a worldserver here; the wu-images build is the first real compile. It does pass
  a syntax-only compile (`g++ -fsyntax-only`) of `Bossfights.cpp` against the pinned core's and mod-playerbots' headers,
  with and without mod-playerbots, and its pure helpers pass their unit test.
- Not run in game:
  - the instance start;
  - the base-class boss states being enough for the fight's prerequisite check;
  - the bots' talents and gear;
  - the timer hooks;
  - the wipe and restart;
  - the heroic entry path.
- The Lua tests (`test_ov_go.lua`, `test_ov_playmode.lua`) were written but not run here (no Lua interpreter); the lua-check
  workflow runs them.
- `realm-image.sh` has not run on the server. It needs the ghcr packages to be pullable from the server.
