# Boss Rush

Working name: **Boss Rush**. TODO(owner): the final name and the tile's line, his words.

What the owner asked for (5 Nov 2026, verbatim):

1. "lets now isolate bossfights into its own launched mini game mode thing wrapped by itself plays on 3.3.5 game files
   its like Den but boss fight and can select any fight any raid light weight only needs the relevant slices and armor
   and more, suggest more features, build it out, deep research and make it"
2. "boss rush should be in the actual raid, like [the citadel's last boss] on his actual platform exactly as-is" (the boss's name
   left out here, as everywhere players read)
3. "1:1 to the game"

So Boss Rush has **two engines behind one launcher tile**, with the exact one first:

| | **Exact** (the main one) | **Light version** (labelled "not exact") |
|---|---|---|
| What fights | The game server's own boss scripts (AzerothCore's, the core the realm already runs), in the real raid map | Our own recreation of each fight's mechanics (an encounter script, `labs/den-lite/src/bossrush.rs`) at the same real spot on the same real map |
| Where it runs | The player's own PC: the Built-in engine (`launcher/src-tauri/src/host/engine.rs`) as its own small install, `Kind::BossRush` | The player's own PC: Den Lite, Your Den's small server |
| How 1:1 | As 1:1 as AzerothCore's scripts are (they are the community's recreation of the live fights; some details differ, see "Not verified") | Not 1:1. Positions, the boss's look and scale, the platform and its edge are the real ones; the abilities are ours |
| First time | One set-up: the engine (shared with Single player), the raid maps from the game folder, a database | Nothing beyond Your Den's own small program |
| Start | A minute or two (the world server starts) | Seconds |
| Windows only? | Yes, for now (the Built-in engine is a Windows build) | No |

Everything plays on the player's own 3.3.5a game files. Leyline never ships game data: the maps are read from the
player's own folder on their own PC, and both engines only tell the player's game where to stand and what is there.

## 1. How it is used

### The tile

The launcher's side menu has **Boss Rush** under Single player (a shield mark), and Home has a row for it ("Pick a
fight"). The page (`launcher/ui/bossrush.js`, `bossrush.css`):

1. **Engine:** *Exact, the real fight* (recommended) or *Light version, not exact*. A fight without a light version has
   the second greyed out.
2. **The exact engine's card:** Set up Boss Rush (one time), its steps with progress (the same progress line as Single
   player's), Start, Stop. When Single player is running on this PC (both use the same ports) it says so, with
   "Stop it and start Boss Rush". Not on Windows: "the light version runs here".
3. **The fights:** a tile each: the title, the raid's own place name, a line about it, Normal or Normal · Heroic, "Light
   too", and the best time.
4. **Difficulty:** Normal / Heroic (Heroic only where the raid has one).
5. **Your hero:** the class, then its specs (Tank / Healer / Damage in the tooltip). Level 80, geared, spells on the
   bars, both specs ready (free respec between pulls). Bot companions take the other nine seats.
6. **Fight.** Exact: the hero is made at once (or picked), and the game starts signed in to the hero, onto the
   platform. Light: Den Lite gets the fight, then Your Den's own one-click start opens it.
7. **The result card** after a run (CLEARED or WIPED, the time, New best) with **Try again**, the **best times** board
   (kept on this PC; each cleared best has **Share**: onto the website's local-run board, docs/LEADERBOARDS.md), and
   **Share this fight**: the fight as a code (`ley1.fight.r.<...>.<check>`) to copy, or a friend's code pasted to pick
   their fight.
8. **Leaderboards** under the page (`#br-boards`, ui/boards.js): the realms' signed boards (Boss Rush first, Duels,
   Local runs). This week's featured fight (the catalog in turn, one a week) has a **This week** chip on its tile.

### What happens on Fight (exact)

`bossrush_play` (launcher `app.rs`):

1. The hero account's login (`RUSHBF`, the module's own rule: the player's account `RUSH` + `BF`) is created once
   and its password set over the realm's console (SOAP, 127.0.0.1 only). The password is made once and kept in
   `<app>/engine-bossrush/state/state.json`; it is never shown unless the game can't be signed in by itself.
2. `bossfight hero go RUSH <fight> <difficulty> bots <class> <spec>`: the module makes or picks the hero, keeps the
   fight as its waiting go, and saves it on that fight's platform (docs/BOSS-FIGHT-HEROES.md).
3. The game's Config.wtf gets the hero account and the realm "Boss Rush"; our login screen code (`crate::hybrid`) signs
   in by itself where the game has Forever UI's login screens; otherwise the page shows the login to type once.
4. The hero's login lands on the platform (`OnPlayerLoadFromDB`), the run starts (earlier bosses DONE, the bots), and
   the clock runs from the pull to the kill. Each attempt is in the module's own table; the launcher's board reads
   the new ones (`bossrush_board`).

### What happens on Fight (light)

`bossrush_play` writes `<dens>/bossrush.json` (`{"fight","diff","class","spec","id"}`); Den Lite reads it within 2
seconds (each id once) and runs `.den bossrush <fight> <difficulty> <class> <spec>`. In the game the same command
works by hand:

| Command | Does |
|---|---|
| `.den bossrush` | the light fights and the commands |
| `.den bossrush <fight> [normal\|heroic] [class spec]` | the hero that class and spec at 80 (`.den class`), then off to the fight's platform |
| `.den bossrush again` | ready again now |
| `.den bossrush phase <n\|end>` | practice: the boss's health set for that phase, or for the killing blow (not on the board) |
| `.den bossrush status` | the fight, the phase, the boss's health, the adds |
| `.den bossrush leave` | home to your Den |

A clear or a wipe is appended to `<dens>/bossrush-results.tsv` (the board's own line format), and the fight is ready
again a few seconds later: hit the boss to try again.

## 2. The catalog

Every fight is one row of the module's table (`realm/custom/modules/mod-leyline-bossfights/src/BossfightsLogic.h`
`Bosses()`). The launcher's copy (`host/bossrush.rs FIGHTS`) is checked against it by a test. Player-facing names are
our own descriptive names; the boss's own name never shows. The bosses' spots are their rows in the pinned core's
`data/sql/base/db_world/creature.sql`; the raid lands 25 yards in front of the boss (along its facing), turned to it,
unless said otherwise. Ids are each raid's own header in the pinned core (`3f2969dd`).

| Key | Title | Raid (map) | Boss (entry, instance id) | Arrival (x, y, z, facing) | Heroic | Before the fight | Light |
|---|---|---|---|---|---|---|---|
| `frozen-throne` | The Frozen Throne | Icecrown Citadel (631) | 36597, 12 | 517.0, -2124.6, 840.86, π (12 yd east of the paladin) | yes | bosses 0-11 and 13 DONE; the clock stops at the 10% cast 72350 | yes |
| `bone-lord` | The Bone Lord | Icecrown Citadel (631) | 36612, 0 | -376.37, 2211.14, 42.08, π | yes | none | yes |
| `cult-mistress` | The Cult Mistress | Icecrown Citadel (631) | 36855, 1 | -609.71, 2212.26, 52.0, 3.18 | yes | 0 DONE (her hall's doors) | yes |
| `twilight-destroyer` | The Twilight Destroyer | The Ruby Sanctum (724) | 39863, 6 | 3131.0, 533.37, 72.99, 0.02 | yes | 0 and 2 DONE quietly; 1 DONE through the script (it starts the boss's intro) | yes |
| `obsidian-sanctum` | The Obsidian Sanctum | The Obsidian Sanctum (615) | 28860, 0 | 3245.26, 526.30, 58.62, 1.52 | no | drakes 1-3 DONE and their creatures taken away: the boss alone | yes |
| `obsidian-three` | The Obsidian Sanctum, three drakes | The Obsidian Sanctum (615) | 28860, 0 | as above | no | none: all three drakes join (the hardest version) | no |
| `brood-mother` | The Brood Mother's Lair | Onyxia's Lair (249) | 10184, 0 | -35.48, -216.72, -87.73, 6.18 | no | none | yes |
| `stone-watcher` | The Stone Watcher | Vault of Archavon (624) | 31125, 0 | 113.89, -101.09, 91.40, 6.27 | no | none (the Wintergrasp lock is off in the profile) | yes |
| `storm-watcher` | The Storm Watcher | Vault of Archavon (624) | 33993, 1 | -219.16, -263.57, 91.55, 4.73 | no | none | no |
| `flame-watcher` | The Flame Watcher | Vault of Archavon (624) | 35013, 2 | -218.96, 79.39, 91.83, 1.55 | no | none | no |
| `frost-watcher` | The Frost Watcher | Vault of Archavon (624) | 38433, 3 | -43.77, -263.71, 91.25, 4.73 | no | none | no |
| `spell-weaver` | The Spellweaver's Eye | The Eye of Eternity (616) | 28859, 0 | 734.40, 1301.27, 266.20, 0 (20 yd west of the disc's middle, `eye_of_eternity.h CenterPos`) | no | none | no |

| `icc-gunship` | The Gunship Battle | Icecrown Citadel (631) | ships, 2 | alliance -467.29, 2466.70, 190.46, 3.10; horde -474.19, 1916.34, 216.33, 3.17 (game_tele 1958 / 1957) | yes | 0 DONE, 1 DONE through the script (brings the ships) | no |
| `icc-deathbringer` | The Deathbringer | Icecrown Citadel (631) | 37813, 3 | -559.59, 2211.55, 539.29, 6.27 (game_tele 1959: the rise's landing) | yes | 0-2 DONE | no |
| `icc-bloat` | The Bloated Horror | Icecrown Citadel (631) | 36626, 4 | 4267.71, 3077.06, 359.5, π/2 (door 201371) | yes | 0-3 DONE | no |
| `icc-ooze` | The Ooze Horror | Icecrown Citadel (631) | 36627, 5 | 4445.72, 3077.06, 359.5, π/2 (door 201370) | yes | 0-3 DONE | no |
| `icc-professor` | The Mad Professor | Icecrown Citadel (631) | 36678, 6 | 4356.76, 3147.91, 388.4, π/2 (door 201372) | yes | 0-5 DONE; the trap event DONE (`SetData(254, 3)`: the lab's door) | no |
| `icc-council` | The Blood Council | Icecrown Citadel (631) | 37970, 7 | 4573.03, 2769.33, 361.3, 0 (door 201376) | yes | 0-3 and 14 (the trash that opens the hall) DONE | no |
| `icc-blood-queen` | The Blood Queen | Icecrown Citadel (631) | 37955, 8 | 4526.99, 2769.85, 403.8, 0 (door 201746) | yes | 0-3, 14, 7 DONE | no |
| `icc-dreamwalker` | The Dreamwalker | Icecrown Citadel (631) | 36789, 10 | 4300.61, 2484.49, 366.4, π (door 201375) | yes | 0-3 and 9 (the ambush) DONE | no |
| `icc-frost-queen` | The Frost Queen | Icecrown Citadel (631) | 36853, 11 | 4266.2, 2484.56, 210.5, 0 (door 201373) | yes | 0-3, 9, 10, 13 (the gauntlet) DONE; the two frost wyrms (37533, 37534) killed through the core, so the script calls her | no |

Since start anywhere (12 Nov) `bone-lord` and `cult-mistress` also start 8 yards outside their rooms' doors (201857 at
-338.09, 2211.47; 201563 at -520.44, 2211.47, its floor 62.8), not inside the hall.

"Heroic" is the game's own heroic 10 (raid difficulty 2): the citadel, the Ruby Sanctum (and the Coliseum, not in
the catalog yet) have one; the others are normal only in this version of the game, so the page and the module refuse
heroic for them.

**Only the featured fight on the official realm.** The module offers the rest only with
`LeylineBossfight.AllFights = 1` (`AC_LEYLINE_BOSSFIGHT_ALL_FIGHTS`), which only Boss Rush's profile sets. The
official realm keeps exactly The Frozen Throne (the boards, `ov_go.lua` and the website list only it:
`stream/tests/test_bossfights_export.py` checks that).

### What each fight's script does before it can be fought (read in the pinned core)

* **The Frozen Throne:** the existing module path (docs/BOSS-FIGHTS.md): the paladin's gossip starts it.
* **The Bone Lord:** `boss_lord_marrowgar.cpp`: a plain pull.
* **The Cult Mistress:** `boss_lady_deathwhisper.cpp`: her intro on line of sight, then the pull; the cult's adds from
  `SummonPositions` (both side doors and the upper one).
* **The Twilight Destroyer:** `instance_ruby_sanctum.cpp`: `SetBossState(DATA_GENERAL_ZARITHRIAN, DONE)` calls the
  controller's `ACTION_INTRO_HALION`, which brings the boss in. The quiet base `SetBossState` the module uses for
  prerequisites skips that, so this fight's Zarithrian goes through the script's own `SetBossState` (BossDef
  `scripted`).
* **The Obsidian Sanctum:** `boss_sartharion.cpp JustEngagedWith`: a drake whose boss state is DONE is left out of the
  fight; the module also takes their creatures away (BossDef `despawn`), so nothing patrols over the lake.
* **The Brood Mother's Lair, the watchers:** plain pulls. The vault's `IsEncounterInProgress` kicks players during a
  Wintergrasp battle only when `Wintergrasp.KickVoAPlayers` is on; Boss Rush's profile turns it and Wintergrasp off.
* **The Spellweaver's Eye:** starts when the raid is on the disc. Its last phase puts everyone on drakes (vehicles).
  The heroes' access requirements are waived (`OnPlayerNotAvoidSatisfy`), so no key is needed.

Considered and left out of this first catalog (each needs more than prerequisites): the frost dragon of the citadel's
wing (spawned by the gauntlet's two dragons dying), the plague wing's professor (the wing's event), Ulduar's keepers
and its last fights (the keepers' lights, the sigils), the Coliseum (the event chain of its announcer), the
necropolis's last fight (the four wings and the console). Each is a row away once checked in game.

## 2b. Start anywhere

Owner, 12 Nov: "Boss Rush should allow you to start in front of any boss or any raid wing or the entire raid itself with
companions or with friends in a one-click UI that's convenient and then tested by you at least a full boss fight run
through — [the final fight]."

**The tree** (`BossfightsStarts.h Raids()`; the launcher's copy `host/bossrush.rs RAIDS`, checked against it by a test):

| Raid | Whole raid (from its entrance) | Wings (from their first boss) | Bosses |
|---|---|---|---|
| Icecrown Citadel | `icc-raid` (areatrigger_teleport 5670: 76.86, 2211.37, 30.0) | `icc-spire` The Lower Spire (0, 1, 3), `icc-plague` The Plagueworks (4, 5, 6), `icc-crimson` The Crimson Hall (7, 8), `icc-frostwing` The Frostwing Halls (10, the gauntlet 13, 11); The Frozen Throne is its one boss | all 12 above |
| The Ruby Sanctum | `rs-raid` (5869) : the three guardians, then the dragon | (one fight) | `twilight-destroyer` |
| Vault of Archavon | `voa-raid` (5258): the four watchers, any order | (one room each) | the four watchers |
| The Obsidian Sanctum, Onyxia's Lair, The Eye of Eternity | (one fight each) | | their fights |

A wing or the whole raid is a row of its own for the run code (`Scopes()`): the start, the earlier bosses DONE, a
`chain` of boss ids. The clock starts on the first pull of any boss in it and stops on the last boss's end (the final
fight's 10% cast); a FAIL of any of its bosses, or everyone dead away from a boss for 10 s (the trash), is a wipe. The
gunship (2) is set DONE for the Lower Spire wing and the whole citadel: companions can't cross between the ships (it
stays a start of its own). The ambush before the dreamer (9) is a step of the whole raid, not a start.

**Retry from this boss:** after a wipe the raid stands again at the start in front of the first boss of the run still
standing (`RetrySpot`); a step with no start of its own (the gauntlet, the sanctum's guardians) goes back to the
nearest earlier one that has one, else to the run's own start. Heroes are back up 8 s after a wipe by themselves (as
before); the page's Retry does it at once; after a clear, Again plays the same start in a fresh instance.

**Size:** 10 or 25 (`normal25` / `heroic25` in a join and a hero's waiting go; raid difficulty 1 and 3). Companions
fill 2 tanks, 3 healers, 5 damage (10) or 3, 6, 16 (25), less the players' roles (`DefaultSeats25`, `BotSeats`). The
official realm keeps 10 and the featured fight only (`AllFights`). The hero table's go columns are 64 now.

**Companions or friends:** companions are the module's own mod-playerbots addclass bots (as before; `ov_companions.lua`
is off on Boss Rush's profile). With friends: the host's hero waits in a lobby for the start (`group`, or `group-only`
without companions); **Invite a friend** (`bossrush_invite`) is the worlds' own invite link (the relay tunnel, the same
network: `world_invite_now`, which Boss Rush may use now); when the friend's launcher claims its login, the host's
launcher answers with a hero made for them on their own hero account, with the claim's password, waiting at the same
start (`worldfriends::World::login_for`, app.rs `bossrush_friend_login`). **Start now** (`bossfight rush go`) starts
with whoever is there; companions take the rest (`LobbyWaitSec` 600 on Boss Rush's profile, else it starts by itself).

**The page** (`ui/bossstart.js`, `bossstart.css`, its own part `#bs` above Boss Rush's card): a tab per raid; the whole
raid, each wing and each boss as list kit rows grouped by wing (search, keys); Normal / Heroic; 10 / 25; With
companions (their roles as tank / healer / damage marks with counts) or With friends (the friend's hero, Invite a
friend, companions in the seats left or players only); your hero; one **Start** (`bossrush_start_at`); then the raid
as the realm says it (`bossrush_state` every 4 s): waiting for friends with Start now, ready, fighting with the
bosses down and the clock, wiped with **Retry from this boss**, cleared with Again, Leave.

**The console words** (SOAP, 127.0.0.1; `bossfight rush <verb> <ACCOUNT> [arg]`, security administrator, console
only): `status` (`bfrush run key=… diff=… phase=… map=… inst=… x= y= z= dist=… members= bots= inside= step=… done=a/b
boss=… wipes= clears= marks=… last=… ms=…`, or `bfrush lobby …`, `bfrush offline`), `retry`, `go [nobots]`, `pull` (the
final fight's paladin spoken to as a player would, any other boss pulled onto the hero), `autopilot on|off`
(mod-playerbots' own AI plays the hero: the playtest), `leave`.

### The boss playtest

`Leyline play-test --bossrush [--boss <start>] [--size 10|25] [--diff normal|heroic] [--hero <class>-<spec>]
[--fight-max <s>] [--checks bossstart,bossfight]` (app.rs `bossrush_play_test_cli`, sptest.rs), run by
`tools/first-run/sp_playtest.py --bossrush` in a fresh data folder (Control: **`sp-playtest-boss`**):

1. the game folder, the disk check, Boss Rush set up from the game folder and started (the page's own `begin_kind`);
2. the start picker's one click (`bossrush_exact` with `start_go_line`): the hero made, the game signed in;
3. **bossstart:** within 7 minutes, on the realm's word: the start's map, an instance of its own, phase ready, the hero
   within 15 yards of the start, `size` members of whom `size - 1` companions, all inside;
4. **bossfight** (default: the final fight): `autopilot on`, `pull`, then every 3 s the status: the engage (within 4
   minutes: the paladin walks up and talks first), the boss's phase casts (`transition1`, `quake`, `transition2`, `end`:
   `PhaseMarks`), bosses down, the outcome (a kill or a wipe, with its time), then Retry (a wipe: ready again by
   itself within 30 s, else `retry`; a clear: `retry` gives a fresh instance) until the hero is back within 15 yards of
   the start. It passes when the fight ran from the pull to an end and Retry worked; the line says KILL or WIPE as it
   was. Bots that can't win don't fail it.
5. the game closed, the realm stopped, the game folder put back. Screenshots: entering, in the world, in front of the
   boss, fighting, the fight's end, retried. `logs\boss-playtest\<time>\` (`br-play-test.json` has the timeline).

## 3. Lightweight: only the raid maps

The exact engine is the same realm the Docker kit and Single player run, so it still needs the world database (a
few minutes to fill, once) and the game's DBC tables. What it does not need is the open world:

* **Maps:** `map_extractor -m 249,615,616,624,631,724` (a new option, `realm/custom/patches/core.patch`: only those
  maps, plus the eight tiles of the continents the world server checks before it starts: `World.cpp`'s "map files
  for starting areas"). An engine built before the patch reads everything; Boss Rush then deletes the other map files
  (`engine.rs trim_maps`). `engine.json` says which (`"map_filter": 1`).
* **No vmaps, no mmaps** (pathing is off, as on the realm).
* **No roaming bots, no open-world modules** (`realm/profiles/bossrush.env`): only the module's own bot companions.

Sizes and times: **not measured yet** (no Windows engine run here). From the extractor's own layout, the catalog's six
raid maps are a few dozen `.map` tiles, against the full extraction's ~5,000; the DBC tables (~150 MB) and the
database (~1-1.5 GB, docs/BUILTIN-ENGINE.md) are unchanged. So the first set-up is the engine download (shared with
Single player: none if Single player is set up), the DBC read and the map read of minutes less, and the database
fill. Control's `boss-rush` step prints the real numbers on the PC (map files kept, others).

### Per fight: the game files it needs (and the browser slice for later)

The player's own client draws each fight from its own archives. For browser play later (not built now), each fight's
slice is its map's tiles and the objects on them, and the models of the creatures in it:

| Fight | Map files (client) | Area to slice |
|---|---|---|
| The Frozen Throne | `World\Maps\IcecrownCitadel\` (ADT 35_31) and the citadel's WMO | box 380..640, -2660..-1990 (docs/BOSS-FIGHTS.md "slice"): the platform and the room inside the blade |
| The Bone Lord, The Cult Mistress | `World\Maps\IcecrownCitadel\` | the lower spire's first two halls, x -700..-340, y 2100..2330 |
| The Twilight Destroyer | `World\Maps\ChamberOfAspectsRed\` | the sanctum's ring, around 3156, 533 (r 80) |
| The Obsidian Sanctum (both) | `World\Maps\ChamberOfAspectsBlack\` | the lake and the island, around 3246, 551 (r 120) |
| The Brood Mother's Lair | `World\Maps\OnyxiaLairInstance\` | the cave, around -10, -219 (r 100) |
| The watchers | `World\Maps\WintergraspRaid\` | the vault's four rooms (r 300 around its middle) |
| The Spellweaver's Eye | `World\Maps\NexusRaid\` | the disc, around 754, 1301 (r 120) |

The folder names are the maps' internal names from Map.dbc as the extractor prints them; they are read from the
player's own files, never shipped. TODO: confirm each name against a real extraction log (Control `boss-rush` lists
the map ids it kept).

## 4. Heroes and companions

**Exact:** the module's heroes, unchanged (docs/BOSS-FIGHT-HEROES.md): a separate hero account, an instant level 80
(trainer spells, the premade talents, dual spec free, the spec's PvE kit from `ov_kits.lua` worn, every other kit in
four bags, bars from `ov_main.lua`, flasks and food), no XP, no trade, auto retry 8 s after a wipe, free respec. What
changed for Boss Rush: a hero now waits at **its own fight's** platform (the fight of its waiting go), not always at
the featured one (`BossfightHeroes.cpp HomeOf`). The companions are mod-playerbots' own addclass bots in the default
seats (2 tanks, 3 healers, 5 damage, minus the player's role).

**Light:** `.den class <class> <spec> 80` (the player's own Spell.dbc and Talent tables, as the Dojo uses) for the
hero, and nine companions of our own (bossrush.rs `raid`): two tanks, two healers, the rest damage. They are simple and
readable: the tank stands between the boss and the raid, the healer heals whoever is lowest, the damage dealers hit
the boss or the nearest add going for the healer, half of them join a soak, and anyone in a ring steps out (most of
the time).

## 5. The light version's encounter format

Plain text, one statement a line (the tale script's style). Parsed and checked (`bossrush.rs parse`): 8 KB at most, 6
phases, 8 abilities a phase, 12 adds; every number in range; the raid must land on its own floor; no brand words, no
links; nothing in it runs as code. An error says the line and why.

```text
fight frozen-throne
title The Frozen Throne
map 631
start 517.0 -2124.6 841.4 3.14          # where the raid lands: x y z facing
edge 503.6 -2124.7 48 835               # the floor that counts: middle, radius, lowest height (off it: a fall)
boss 36597 at 470.0 -2124.4 840.9 face 0 health 6000000 hits 9000 every 2.5 scale 1
heroic health 1.4 damage 1.3
phase until 70                           # while the boss is above 70%
  say "Cold creeps across the platform."
  every 20 circle "Creeping plague" r 6 warn 3 hit 14000 at random
  every 30 adds 37695 3 at edge
phase until 40
  say "The edge gives way! Stay near the middle."
  edge 503.6 -2124.7 36 835              # the platform shrinks
  every 15 pulse "Bitter winter" hit 3500
  every 24 soak "Dark ground" r 5 warn 4 hit 30000 need 3
phase until 0
  every 12 cone "Soul harvest" angle 60 length 22 warn 2 hit 30000
enrage 900
```

| Statement | Meaning |
|---|---|
| `boss <entry> at x y z face o health n hits n every s scale s` | the creature (its model is the game's own, by the open AzerothCore table `creatures.rs`), its spot, health for ten, its swings at its target |
| `heroic health k damage k` | heroic's multipliers |
| `phase until <pct>` | a phase lasts while the boss is above that; the last is `until 0` |
| `say`, `edge` (in a phase) | said / changed as the phase begins |
| `every <s> circle "<name>" r <yd> warn <s> hit <n> at random\|tank\|healer` | a ring under someone: leave it before it lands |
| `every <s> cone "<name>" angle <deg> length <yd> warn <s> hit <n>` | the front of the boss, toward the tank |
| `every <s> pulse "<name>" hit <n>` | everyone |
| `every <s> soak "<name>" r <yd> warn <s> hit <n> need <k>` | `k` must stand in it (the hit is shared), else everyone takes all of it |
| `every <s> adds <entry> <n> at edge\|boss` | adds; they go for the healer |
| `enrage <s>` | after this long, a wipe |

Health scales with the raid (a tenth of `health` a member, three tenths at least) and heroic. The fight is a seeded
state machine (`Fight`): the same seed plays the same fight (the tests rely on it).

The seven light fights are in `bossrush.rs CATALOG`; their numbers are ours (a fight of a few minutes for a hero and
nine companions), their abilities loosely after each fight's best-known mechanics, named in our own words.

## 6. Research

### What makes a boss fight fun and readable

From the encounter-design writing of the last two decades (designers' talks and postmortems on raid and action-game
bosses; summarised here in our words, nothing copied):

* **Telegraph, then punish.** Every dangerous thing is shown before it lands (a ring on the ground, a cast bar, a
  frontal glow), long enough to react (1.5 to 4 s), and the punishment is proportional. Unfair is "it hit me and I
  never saw it"; fun is "I saw it and stood there".
* **One new idea per phase.** Phases by health give a fight a story and a rhythm; each adds one mechanic (or changes
  the arena: the platform's edge falling away) rather than many at once.
* **Roles have jobs.** Tanks face the boss away and move it; healers triage a pulse and a spike; damage dealers switch
  to adds. A fight that only asks "do damage" is a dummy.
* **Movement mechanics in three shapes cover most fights:** get out (rings, cones), get in (soaks, stacking), and get
  away from each other (spreading). Boss Rush's light format has the first two; spreading is a later statement.
* **Soft enrage before hard enrage.** Damage that grows with time punishes slow raids gradually; a hard enrage is a
  clear limit.
* **Readable failure.** A wipe card that says *why* ("you fell off the edge", "not enough of you stood in it") makes
  the next try better. Both engines say why.
* **Short loops.** Instant retry (the heroes' auto retry, the light version's Try again) matters more than anything
  else for practice.

### Open-source references (read only; nothing copied into Leyline's own files)

| Project | What it is | Licence | Used for |
|---|---|---|---|
| AzerothCore (the mod-playerbots fork pinned in `realm/custom/sources.lock`) | the 3.3.5a server the realm and the exact engine run | GPL-2.0-or-later | the exact engine runs its boss scripts as they are; ids, spots and how each fight starts were read in `src/server/scripts/Northrend/...`, `Kalimdor/OnyxiasLair` and `data/sql/base/db_world/creature.sql` |
| TrinityCore (3.3.5 branch) | the server AzerothCore came from | GPL-2.0 | the same scripts' older form: background only |
| cMaNGOS / ScriptDev2 | another open server family with its own encounter scripts | GPL-2.0 | background only |
| mod-playerbots | the bots the exact engine's companions are | GPL-2.0 (docs/BUILTIN-ENGINE.md) | the companions, as the module already uses them |
| Boss-timer addons (Deadly Boss Mods, BigWigs) | the community's own warning addons | not checked here | the warning conventions players know (ring, cast bar, "move!"); nothing read from their code |

Never used: wowdecomp, Waystone, any decompiled client, any leaked source.

### Ranked list of more features (35)

Ranked by fun for the effort, highest first. "E" = exact engine, "L" = light version.

1. **Weekly rotation** (E, L): a featured fight each week, the board for that week. **Built** (12 Nov,
   docs/LEADERBOARDS.md): `featured_fight` / `featured` / `B.featured`, the tile's chip, the boards' line.
2. **Co-op with a friend** (E): the hero queue already takes a group; the Boss Rush install's realm opened to a friend
   through the Den invite path (relay tunnel) so two heroes fight with eight bots.
3. **Co-op with a friend** (L): Den Lite's friends listener: a guest joins the host's fight.
4. **Practice a phase** (E): start at a phase's health (the light version has it; the exact one needs the script's
   own phase entry points per boss).
5. **Tiers beyond heroic** (E, L): mythic-style levels that scale the boss's health and damage (the owner's own wish:
   "tiers beyond heroic that also scale the gear"; mod-mythic-plus or the module's own multipliers).
6. **Affixes** (L, then E): a weekly twist (more adds, faster rings, a shrinking floor, healing halved).
7. **Ghost replays** (L): record the player's path and the fight's events (the fight is deterministic by seed), play a
   best run back as a ghost.
8. **Results card with the fight's story** (E, L): who died to what, the damage and healing per companion. First
   step built: deaths per clear on the signed boards (ov_boards.lua).
9. **Encounter editor in the Den** (L): write a fight in the format, place its rings on the Den's floor, share it as a
   code (`ley1.fight`).
10. **Jaina commentary** (L): Jaina (`jaina.rs`) calls the mechanics in her own voice and coaches after a wipe.
11. **The rest of the citadel** (E): the frost dragon, the plague wing, the blood wing: each a catalog row once its
    start is checked in game.
12. **The Coliseum** (E): its event chain started by the module.
13. **Ulduar's keepers and the end** (E): the lights and the sigils set by the module.
14. **The necropolis's last fight** (E).
15. **Speedrun splits** (E, L): the time per phase, against your best.
16. **Daily challenge** (L): one seed for everyone that day, the same rings in the same places. Planned next
    (docs/LEADERBOARDS.md section 6: a seed in `bossrush.json`, replay checks for local runs).
17. **Companion orders** (E, L): "tank, move it there", "healer, focus me", from a small panel.
18. **Pick your companions** (E, L): which classes fill the seats (the module's `BotSeats` already reads a config).
19. **No-companion mode** (E): solo or players only (the module's group-only).
20. **Gear tiers for the hero** (E): T7, T8, T9, T10 looks and numbers (the kits table grows).
21. **Glyphs** (E): the premade glyphs (the hero docs list them as not done).
22. **Hard modes** (E): the Sanctum's drakes already; the vault's and Ulduar's next.
23. **A boss rush proper** (E, L): several fights back to back on one clock.
24. **Leaderboard online** (E): the website's boards for Boss Rush runs, uploaded only when the player says so.
    **Built**: signed realm boards for every catalog fight a realm offers, and the separate local-run board (Share),
    docs/LEADERBOARDS.md.
25. **Spectate** (L): a friend watches the host's fight.
26. **Training dummies with a fight's mechanics** (L): the Dojo's practice boss given a catalog fight's rings.
27. **Accessibility** (L): longer warnings, a colour-blind ring look, a "story" difficulty.
28. **Touch play** (L): the light version in the browser on a phone (Den Lite already runs in a web page; the slices
    above say what to stream).
29. **Browser play** (E): the streamed browser game on the slices above.
30. **Hero presets** (E, L): a named hero per role, one click.
31. **Achievements** (E, L): no deaths, under par time, every fight cleared.
32. **Fight intros** (L): a short camera move over the arena before the pull.
33. **Sound and music cues** (L): the game's own sounds played at a ring's landing (sound ids only).
34. **Stats export** (E, L): a run as a small file to compare with friends.
35. **The owner's own fights** (L): a fight of his in the catalog, written in the format.

## 7. Files

| File | What |
|---|---|
| `realm/custom/modules/mod-leyline-bossfights/src/BossfightsLogic.h` | the catalog (12 fights), `AllFights`, heroic per fight, `FindBossOnMap`, the start hints |
| `.../src/Bossfights.cpp` | events matched to the run's own fight; scripted prerequisites and creatures taken away once; start hints; `AllFights` |
| `.../src/BossfightHeroes.cpp`, `.h` | heroes wait at their own fight (`HomeOf`); `HopInto` takes the map |
| `.../tests/test_bossfights_logic.cpp` | 52 checks (was 40): the catalog, fences, keys, the citadel's three fights told apart, heroic, AllFights |
| `realm/profiles/bossrush.env` | the exact engine's realm |
| `realm/custom/patches/core.patch` | `map_extractor -m` |
| `.github/workflows/engine-windows.yml` (+ `tools/github-workflows/`) | `"map_filter": 1` in engine.json |
| `launcher/src-tauri/src/host/bossrush.rs` | std only: the catalog, the pick, the console lines and answers, the map keep list, the board, the share code |
| `launcher/src-tauri/src/host/engine.rs` | `Kind::BossRush`: its own install on the shared engine files, the lighter map read, one realm at a time |
| `launcher/src-tauri/src/app.rs` | `bossrush_catalog`, `_setup`, `_start`, `_stop`, `_logs`, `_play`, `_board`, `_record`, `_code` |
| `launcher/ui/bossrush.js`, `bossrush.css`, `index.html` | the page, the side menu item, Home's row |
| `realm/custom/modules/mod-leyline-bossfights/src/BossfightsStarts.h` | start anywhere: raids, wings, steps, the wing and whole-raid rows, retry spots, phase marks, the status line |
| `launcher/ui/bossstart.js`, `bossstart.css` | start anywhere's picker and the raid's state (`#bs` on the page) |
| `launcher/src-tauri/src/sptest.rs`, `playtest.rs`, app.rs `bossrush_play_test_cli` | the boss playtest |
| `tools/first-run/sp_playtest.py --bossrush`, `Boss Rush playtest.bat` | Control `sp-playtest-boss` |
| `labs/den-lite/src/bossrush.rs` | std only: the encounter format, the fight, the light catalog |
| `labs/den-lite/src/bossrush_world.rs` | the world glue (a child of world.rs) |
| `labs/den-lite/src/combat.rs` | `Kind::Raid`, `set_health`, `raid_hit`, `raid_heal`, `refill_me` |
| `tools/boss-rush/` | Control's `boss-rush` (read-only check) and `boss-rush-test` |
| `launcher/ui/boards.js`, `boards.css`, `src-tauri/src/boards.rs` | the Leaderboards view under the page, Share onto the local-run board (docs/LEADERBOARDS.md) |
| `realm/custom/lua_scripts/ov_boards.lua`, `realm/custom/boards-export.py`, `stream/boards.py` | deaths per clear, the signed boards, the website's page |

## 8. Tests

* `g++ -std=c++17 -Wall -Wextra -Werror -I../src test_bossfights_logic.cpp -o t && ./t` (in the module's `tests/`): 52.
  The heroes' test (57) is unchanged and passes.
* `rustc --edition 2021 --test launcher/src-tauri/src/host/bossrush.rs` (8) and in `cargo test` with engine.rs's
  three new tests (the install's paths, the profile winning over modules.env, the trim).
* `rustc --edition 2021 --test labs/den-lite/src/bossrush.rs` (11): the catalog parses, bad scripts say why, health
  scaling, pull / phases / the edge / a clear, rings, soaks and cones, wipes (edge, zero health, enrage), nine
  companions clearing a short fight the same way every time, adds, the launcher's ask, practice jumps.
* `labs/den-lite/tests/client.rs boss_rush_light_lands_on_the_platform_pulls_wipes_and_clears`: a test client logs in,
  `.den bossrush frozen-throne` (a far teleport to map 631 at the arrival spot), the boss (entry 36597, hostile) and
  nine friendly companions go up, the boss's name is ours, a swing pulls, a step off the edge wipes (on the board),
  Try again comes by itself onto the arrival spot, a practice phase 2, the killing blow clears (not on the board), the
  launcher's ask file sends it to the vault, leave goes home.
* `cd launcher && node --test tests/bossrush.browser.test.cjs` (7), and the nav / forum tests' menu lists.
* `python3 -I hosting/tests/test_bossrush_profile.py` (5), `python3 -I tools/boss-rush/test_boss_rush.py` (4),
  `python3 -I stream/tests/test_bossfights_export.py` (20).

## 9. Not verified (what only a real game or a Windows run can prove)

* **Nothing has been played.** The exact engine needs an engine build with the module's changes and the core patch
  (the Windows engine workflow) and a Windows PC; the light version needs the owner's real game.
* **The C++ changes are only syntax-checked:** `g++ -std=gnu++20 -fsyntax-only -Wall -Wextra` of `Bossfights.cpp`,
  `BossfightHeroes.cpp` and the loader against the pinned core (`3f2969dd`, its own `deps/`, Boost headers from the
  boostorg repositories, an empty `mysql.h`), without mod-playerbots: clean, and a planted error is caught. The
  patched `map_extractor/System.cpp` the same way. Not a real build: the first compile and link is the engine-windows
  / wu-images workflow.
* **Each fight's start** beyond the featured one is from reading the scripts, not from play: the Ruby Sanctum's intro
  through the script's `SetBossState`, the Sanctum's drakes left out when DONE, the Eye's disc start and its vehicle
  phase with bots (mod-playerbots may not ride the drakes), the vault without Wintergrasp.
* **The arrival spots** 25 yards in front of each boss are computed, not stood on: on a raised dais or over a slope the
  hero may land a little high and fall to the floor (harmless) or, worst case, inside a wall. Each needs one look in
  game; the numbers are one row each.
* **The map read's trim**: the start-tile file names follow the core's `ComputeGridCoord`; that the world server then
  starts with only those continent tiles is from reading `World.cpp`, not a run.
* **Which hero the game enters:** the hybrid login enters "the last character"; with several heroes on the account the
  game may pick another hero than the one just made (the fight is right either way: it is the account's waiting go).
* **The light version in a real game:** the raid maps loaded by a far teleport outside an instance (Den Lite already
  does this for any map with `.den go`, but not yet for these maps), the boss's real model at its scale, the rings
  shown with a plain ground cast's look, the companions using the player race's models, collision with the platform's
  edge (the game's own collision; Den Lite only checks the edge's circle and height).
* **Light version limits (by design, honest):** no death screen or ghost (a wipe resets), companions don't cast real
  spells (their damage and healing are numbers), no line of sight, no threat table, no boss movement beyond its spot.
* **Sizes and times** of the exact engine's first set-up: not measured (section 3).
* **Start anywhere (12 Nov), all from reading the pinned core and its world database, nothing stood on yet:** each
  start's spot 8 yards outside its door (a raised doorstep may put the raid a little high, harmless); the gunship's
  spots on the ships (transports: the deck may have moved; untested); the frost wyrms killed through `KillSelf` calling
  the queen in (their grid loaded first); the lab's door through the trap event's `SetData`; the whole raid's walk
  with bots between bosses (the trash, the ambush's event, the gauntlet); 25-player raids (the addclass pool must
  hold enough bots per class: three priests, three shamans...); a friend's claim answered with their hero (the
  friend's game signing in to the hero account over the relay); `autopilot` (mod-playerbots' self bot) playing a
  hero; `pull` on the final fight (the paladin's gossip as its script takes it). The syntax-only compile of the
  module (with and without mod-playerbots) is clean; the launcher's Rust is checked by CI only (no local cargo
  build); the boss playtest is what proves the fight.
