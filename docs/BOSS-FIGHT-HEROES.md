# Boss fight heroes

What the owner asked for (5 Nov): "boss fight should load into a different character set and only level 80 and
create instantly and be geared with spells on bars and all other gear in inventory and dual spec free and qol for
this minigame". And later the same day: "boss fights should be more instant in the end right now it falsely just loads
the person into the wrath unleashed realm instead of the chamber".

So a boss fight is now played on a **hero**: a level 80 character the realm makes for boss fights only. **Fight now**
on the website makes the hero (or picks the one you have), signs the browser game in to it without showing the
character list, and the hero's first frame in the world is the platform of The Frozen Throne. Your own characters are
never listed, moved, boosted or bound by a boss fight.

The fight itself, the clock, the boards and the lobby are unchanged ([BOSS-FIGHTS.md](BOSS-FIGHTS.md)).

## The separate character set: a second game account

**Choice: a hero account per player, linked to the player's own game account.** The hero account is the player's
account name with `BF` at the end (`WR000123` → `WR000123BF`). A name too long for that becomes `BF` and 14 hex digits
of its hash, so it stays within 16 characters. `BossfightsHeroLogic.h HeroAccountName` and `bossfights.py
hero_account` use the same rule, and both tests check the same values.

Why this and not a flag on characters:

- **Character select shows heroes only.** The 3.3.5a client lists every character of the account it logs in to.
  A flag table can't hide characters from that list without changing the client.
- **Normal characters can't be touched.** A hero's login, its instance binds, its group and its saved place live on
  another account. Nothing in the hero code can load, move or bind one of your own characters, even by mistake.
- **Same sign-in.** The player signs in with the Hero Key as always. The portal gives the hero account the same game
  password as the player's own game account (`ensure_account`, the usual `account create` / `account set password`
  over SOAP). The browser game then signs in to the hero account by itself. There is no new secret to keep.
- **The limits come for free.** The realm's `CharactersPerRealm` (10) is a per-account limit, and so is the hero
  limit (10).

The link is kept in `characters.leyline_bf_hero_accounts` (hero account → the player's account). A hero account that
belongs to another player is refused, and so is a hero account used as the "player's account".

## Instant creation

`.bossfight hero go <ACCOUNT> <boss> <difficulty> <mode> [class [spec [spec2]]] [alliance|horde]` (the website, over
SOAP) and `.bf hero make <class> [spec [spec2]] [alliance|horde]` (in game) make the hero on the spot.

1. **The character.** `Player::Create` runs on a session with no socket, the way mod-playerbots makes its bots. The
   hero never enters the world while it is being made.
   - The race is the plain one for the class and faction (`DefaultRace`).
   - The name is generated (two or three syllables, 2-12 letters, no brand word) and checked against the core's
     name rules and every existing name.
   - The faction is the pick, else the hero account's last one, else the side most of the player's own characters
     are on.
2. **Level 80.** These are GiveLevel's own steps without its level-up hooks, because the realm's Lua reacts to those.
   Skills are raised to the maximum.
3. **Spells.**
   - The class's weapon skills and the class-quest spells come from `ov_main.lua` WEAPON_SPELLS and QUEST_SPELLS.
   - Every class trainer spell the hero can learn is taught, the way `.learn all my trainer` does it but without
     casting. A trainer's "teach" spell is learned through its learn-spell effects.
   - The other faction's versions (portals, mounts, seals) are left out.
4. **Talents.** The spec's premade talents are the PvE builds the bots already use (`realm/playerbots-specs.env`,
   `PlayerbotFactory::InitTalentsBySpecNo`). The trainers then teach the talents' higher ranks.
5. **Dual spec, free.** It is learned with both activate spells. The second spec is filled with its own premade talents
   and ranks. By default it is the spec's partner (fire → frost, prot → fury, holy priest → discipline), or the pick's
   third word.
6. **Gear.**
   - The spec's PvE kit from `ov_kits.lua` (Heroes' T7, ilvl 200) is worn. Each piece goes in the next free slot its
     type allows: both rings, both trinkets, the off hand with dual wield or Titan's Grip.
   - Four 20-slot bags go on.
   - **Every other kit of the class** goes into the bags: the other PvE specs and every PvP kit, with the faction's
     medallion.
   - Then the class's reagents and tools (ov_main.lua CLASS_ITEMS: totems, symbols, arrows and bullets), five
     flasks for each spec's role, and five Fish Feasts. Warlocks also get ten soul shards, and a hunter's ammo is
     selected.
   - The fullest class uses 76 of the 96 bag slots (the unit test prints it).
7. **Bars.** Both specs get `ov_main.lua`'s layouts (BARS): the rotation on bar 1, cooldowns and the PvP medallion
   bottom left, utility bottom right, and the stance and form bars. Each spell is put at the highest rank the hero
   knows in that spec. The boost's mount buttons hold the flask and the food instead. The first spec's buttons are
   saved with the character, and the second spec's go straight into `character_action`.
8. **Saved in one transaction**, committed before the reply, because the browser game asks for the character list a
   few seconds later. The hero is saved on the fight's platform, in no instance.

All of this data comes from the realm's own Lua tables. `tools/gen_hero_data.py` writes `src/BossfightsHeroData.h`
from `ov_kits.lua` and `ov_main.lua`, and `--check` fails when the header is stale. (`gen_kits.py`, which makes
`ov_kits.lua`, is not in this repository.)

**Glyphs:** none yet. The premade glyph item lists exist, but mapping items to glyph slots isn't worth the risk for now.

## Loading straight onto the platform

The owner's test hit an old server: the browser game went to character select and into the normal realm. That can't
happen on the hero path, at three levels:

1. **The website asks first.** `bossfight hero go` is a command only the module knows. On a realm image without it,
   the console answers with a fault. The portal then shows "Boss fights aren't open on the realm yet: the realm update
   that adds them hasn't gone live. Nothing was started, and none of your characters were moved." It makes no session
   and starts no game.
2. **No character list.** The session names the hero, and `/__env.js` carries `"char": "<hero>"` and `"bf": "1"`. The
   web client seats a named character without its character list, as it does for guest passes and the game-folder
   reseat. `wrath-page.js` keeps its sign-in cover up past the character list: "Getting your hero to The Frozen
   Throne…".
3. **The first frame is the chamber.** At login, AzerothCore would put a character on a raid map with no instance at
   the raid's **entrance**. It also lets nobody into a raid instance without a raid group (`CheckInstanceLoginValid`),
   and it sends such a character to its homebind. The module goes around both:
   - While the hero is offline it has no bind to the boss maps, and no stale group (`PrepareOffline` and the logout
     hook), so its login makes a fresh instance.
   - `OnPlayerLoadFromDB` runs after the map is chosen and before the player is added to it. There the hero gets a
     raid group of its own and is put on the platform.
   - The heroic entry requirement is waived for heroes (`OnPlayerNotAvoidSatisfy`). The login check passes, and the
     first frame is the platform.
   - The waiting join (the website's pick, else the hero's last one) runs 1.5 s after login, and the run starts in
     that very instance.

**A fresh instance for the next run** (after a clear, or for the other difficulty) is a far teleport within the same
map (`TeleportTo` with `newInstance`). The hero's own bind is dropped once it has left the old instance. A hero joining
another player's queue run moves the same way and lands in its leader's instance: a grouped player goes where its
leader is bound. There is only a loading screen, and never a frame of the open world.

**Old path, still checked.** A `bf:` go for a normal character (heroes off, or the desktop launcher's
LeylinePlay addon) checks the module's heartbeat first: `characters.leyline_bossfight_live`, refreshed every minute.
Without it, `ov_go.lua` and `ov_playmode.lua` say the same plain words, forget the go and move nobody.

## The minigame's rules (QoL)

| Rule | How |
|---|---|
| Free respec between pulls | `.bf hero spec <spec>` (or a spec chip on the panel): new talents, ranks, kit and bars for the spec that is on. Not in combat, not during the fight. |
| Swap spec instantly | `.bf hero swap` (or Swap spec on the panel). The talent pane's own switch works too, and either way that spec's kit goes on (`OnPlayerAfterSpecSlotChanged`). |
| No run back on a wipe | In a run of heroes only, everyone is up and on the platform again 8 s after the wipe. No Try again is needed. |
| Cooldowns, durability, health and power back | On every restart (any boss fight run, not only heroes). |
| No XP | `OnPlayerGiveXP` → 0 for heroes. |
| No loot clutter | A heroes-only run's loot is cleared before it is shown. |
| A clean economy | Heroes can't trade, mail, bid at auction, or join the Dungeon Finder, battleground or arena queues. Kits are soulbound anyway. |
| Heroes stay at the fight | A teleport off the boss maps is refused ("Heroes stay at The Frozen Throne. Log out to the character list to leave."). A released ghost stays at its body, and the module brings it back. Its "home" is the platform: `.bossfight leave` and the end of a run leave it there. GMs are exempt. |
| A raid of its own | A hero on the platform without a group gets a raid group of its own again within a second, so the core never evicts it. |
| 10 heroes per player | `.bf hero delete <name>` (offline heroes only) frees a place. |

## Commands

Player (in game, `.bf` works for `.bossfight`):

| Command | Does |
|---|---|
| `.bf hero` | Lists your heroes (from your own account or from a hero). |
| `.bf hero make <class> [spec [second spec]] [alliance\|horde]` | Makes one at once. On a hero: log out to the character list to play it. On your own account: play it from the website's Fight now. |
| `.bf hero delete <name>` | Deletes a hero that isn't logged in. |
| `.bf hero spec <spec>` | On a hero: a free respec of the spec that is on. |
| `.bf hero swap` | On a hero: the other spec, now. |

Console / SOAP (the website):

| Command | Reply |
|---|---|
| `bossfight hero go <ACCOUNT> <boss> <normal\|heroic> <bots\|group\|group-only> [class [spec [spec2]]] [faction]` | `bfhero ok <name> <class> <spec> <spec2> <made\|ready> <HERO ACCOUNT>` or `bfhero no <why>` |
| `bossfight hero make <ACCOUNT> <class> [spec [spec2]] [faction]` | the same |
| `bossfight hero list <ACCOUNT>` | `bfhero list <n> <HERO ACCOUNT>`, then `bfhero hero <name> <class> <spec> <spec2> <current\|->` lines |
| `bossfight hero delete <ACCOUNT> <name>` | `bfhero ok deleted` or `bfhero no <why>` |
| `bossfight hero live` | `bfhero live <version>` |

`<ACCOUNT>` is always the player's **own** account. With no class, `go` uses the current hero, else the last pick,
else a fire mage. A class and spec that already have a hero use it; otherwise a new hero is made.

## The website

- `/wrath/bossfights` has a **Your hero** section: the player's heroes (the current one marked), then every class
  with its specs as buttons (Tank / Healer / Damage). Each is a plain link,
  `/wrath?go=bf:frozen-throne:<difficulty>:bots&hero=<class>-<spec>`. Signed out, it goes through Log in, which keeps
  both the go and the pick (`go_signin`). **Fight now with companions** without a pick uses the current hero. The list
  comes from `bossfight hero list` and is cached for 30 s.
- `/wrath?go=bf:...` goes straight to `/wrath/play` (streamed, or with the player's game folder). `/wrath/play` runs
  `hero_login`: it sets the hero account's password, sends `bossfight hero go`, and opens a session on the hero account
  that names the hero. The player's own account gets no `ovgo` for a boss fight any more. The arena queue is unchanged.
- `WU_BF_HEROES=0` in `portal.env` turns heroes off on the site. Boss fights then work as before, on the player's own
  level 80 character.
- Hero accounts are not stream-marked (`ovstream mark`): the module keeps heroes on the boss maps itself.

## The in-game panel

`BossFights.lua` takes one new LYBF line, `H <name> <class> <spec> <spec2> <0|1>`, which the realm sends to a hero at
login, on `.bossfight status` and after a spec change. The window then shows a hero row: "Hero: Kalinra, Fire Mage
(other spec: Frost)", its class's specs as chips (a free respec), and Swap spec. These are disabled while the fight is
on. `/bf hero ...` types `.bossfight hero ...`. A normal character sees no hero row.

## Tables (characters DB)

The module makes these at startup. `data/sql/db-characters/base/leyline_bossfight_heroes.sql` has the same tables.

| Table | Holds |
|---|---|
| `leyline_bf_hero_accounts` | hero account → the player's account, the current hero, the last pick (class, specs, faction, go), the waiting go and when it lapses (30 min) |
| `leyline_bf_heroes` | hero guid → hero account, class, spec, second spec |
| `leyline_bossfight_live` | one row, refreshed every minute: the heartbeat `ov_go.lua` and `ov_playmode.lua` check |

## Files

| File | What |
|---|---|
| `realm/custom/modules/mod-leyline-bossfights/src/BossfightHeroes.cpp` | creation, the login path, commands, QoL hooks |
| `.../src/BossfightHeroes.h` | the seam between the two .cpp files |
| `.../src/BossfightsHeroLogic.h` | pure: the account rule, the class and spec table, names, the kit and bag plan, bars, slots, replies, LYBF `H` |
| `.../src/BossfightsHeroData.h` | GENERATED by `tools/gen_hero_data.py` from `ov_kits.lua` and `ov_main.lua` |
| `.../src/Bossfights.cpp` | the hero hooks in the run: waiting room, hops, auto retry, refresh, `hero` subcommand, heartbeat |
| `.../tests/test_bossfights_heroes.cpp` | `g++ -std=c++17 -Wall -Wextra -Werror -I../src test_bossfights_heroes.cpp -o th && ./th` |
| `realm/custom/lua_scripts/ov_go.lua`, `ov_playmode.lua` | the heartbeat check |
| `stream/bossfights.py` | `hero_account`, `hero_play`, the replies, the picker |
| `stream/wrath_portal.py` | `hero_go`, `hero_login`, the session's `char`, `go_signin` keeping the pick, the not-live page |
| `stream/www-files/wrath-page.js` | `heroPass`: the cover stays up onto the platform |
| `stream/fui-web/Interface/AddOns/WrathUnleashedUI/BossFights.lua` | the hero row |

## Tests

- `g++ ... tests/test_bossfights_heroes.cpp` (57) and `tests/test_bossfights_logic.cpp` (40).
- `python3 -I stream/tests/test_bossfights_heroes.py stream/wrath_portal.py` (49): the account rule (the same values
  as the C++), the pick, the console line, the replies, Fight now's hero path against a fake realm (made, not live, no,
  down), the seated session, Log in keeping the pick, the picker, the table equal to the module's, the generated
  header up to date, and the page's cover.
- `python3 -I stream/tests/test_bossfights.py stream/wrath_portal.py` (109): unchanged, except that a boss fight with
  heroes on sends no `ovgo`. With `WU_BF_HEROES=0` the old `ovgo` is checked.
- `python3 -I stream/tests/test_bossfights_export.py`: the brand-word scan now covers the new sources.
- `texlua stream/tests/test_ov_go.lua realm/custom/lua_scripts/ov_go.lua` (75),
  `texlua stream/tests/test_ov_playmode.lua realm/custom/lua_scripts` (50) and
  `texlua stream/tests/test_wuui_bossfights.lua` (84). texlua (Lua 5.3) runs them here; they were written for lua5.1.
- **Syntax-only compile:** `g++ -std=gnu++20 -fsyntax-only` of `Bossfights.cpp`, `BossfightHeroes.cpp` and the loader
  against the pinned core (`git archive` of mod-playerbots/azerothcore-wotlk 3f2969d) and mod-playerbots, with and
  without playerbots, with `-Wall -Wextra`. It is clean. Boost headers came from the boostorg modular repositories
  (about 160 MB in /tmp). This is not a real build: it doesn't link and it isn't the wu-images CI.

## What the owner presses (Leyline Control)

1. **`ship`**: saves to GitHub. The module's change makes GitHub's **wu-images** workflow build new realm images
   (an hour or more; `builds-why` shows why if it fails).
2. **`realm-image`**, once that build has passed. It pulls the images, backs up, warns in game for 5 minutes, restarts
   on the new images, checks `mod-leyline-bossfights: on (...)`, and rolls back by itself if anything is wrong.
3. **`realm`** (the server pass): uploads `ov_go.lua` and `ov_playmode.lua`, the heartbeat check.
4. **`signin`**: the portal (`bossfights.py`, `wrath_portal.py`). Fight now plays heroes from then on. Before step 2 is
   live it shows the "aren't open yet" page, which is the right answer on the old image.
5. **`page`**: `wrath-page.js` (the cover onto the platform).
6. **`foreverui`**: `BossFights.lua` in the browser game. **`addons`** does the same for the desktop download.

Steps 4-6 can go before 1-3: on the old realm, Fight now says plainly that boss fights aren't open yet.

**Then test in the browser:** `/wrath/bossfights` → pick Fire Mage → you land on the platform with no character list,
level 80, geared, bars filled, bots arriving. Then: `.bf hero` lists the hero. Wipe once (the raid is up again in 8 s,
cooldowns ready). Use Swap spec. Then Fight now again after a clear: it should give a fresh instance with only a
loading screen.

## Not verified

- **Nothing has run in game.** The module has only had a syntax-only compile here; the wu-images build is the first
  real compile and link.
- **Making a hero offline** (`Player::Create` on a session with no socket, then trainer spells, `LearnTalent`, the
  equip and store calls and `SaveToDB`) follows what mod-playerbots does for its bots. Some of it, though, only bots
  have done: the trainer pass, the second spec's talents and the bars.
  - The bars rely on `HasSpell` per spec while the second spec is set active without `ActivateSpec`.
  - `InitTalentsBySpecNo` also writes a "specNo" value for the hero in playerbots' own table (harmless, unverified).
- **The login path:**
  - `Group::Create` inside `OnPlayerLoadFromDB`, before the player is in the world;
  - the `Relocate` to the platform after the map is chosen;
  - `CheckInstanceLoginValid` passing with that group.

  These come from reading the pinned sources (`Player::LoadFromDB`, `HandlePlayerLoginFromDB`,
  `MapMgr::PlayerCannotEnter`). This is the first thing to watch: a failure here shows as the hero at the citadel's
  entrance, or refused.
- **The same-map hop** (`TeleportTo` with `newInstance`, then unbinding) for a fresh instance and for joining a
  queue run.
- **Spec swap** gear moves (`SwapItem`) for every class, in particular two-handers against off hands.
- **Hunters have no pet** (Tame Beast is taught, but there is nothing to tame on the platform). Beast Mastery hunters
  are weaker than they should be.
- **The browser client seating `char` for a normal session** (not a guest pass): the client supports it (the
  game-folder reseat uses it), but it hasn't been tried with a hero account.
- The realm list's character count for hero accounts (`realmcharacters`) isn't updated on creation (cosmetic).
- A fake session writes `totaltime` for the hero account when it is dropped (cosmetic).
- **Desktop players** (launcher, own client) don't get heroes from the Play button. The LeylinePlay addon still runs
  `.bossfight join` on the character they are on. They can log in to `<ACCOUNT>BF` only if they know its password: the
  website sets it to their realm account's own game password.
- Heroes on the boards run in kit gear (ilvl 200) while the bots are geared at 251. The boards don't tell heroes from
  other characters.
