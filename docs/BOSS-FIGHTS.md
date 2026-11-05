# Boss fights (realm side)

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

## Commands

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

## The start, the clock, wipes and the end

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

## Addon messages (the in-game panel)

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

## Go link and Play

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

## The table

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

## Boards export

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

## Settings

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

## What the owner presses (Leyline Control)

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

## Not verified yet

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
