# Race and class combinations (Wrath Unleashed)

The official realm offers more race/class pairs at character creation than the original game. It does this on the
server, in the browser client and in the desktop game. One setting, the realm's **preset**, drives all three:

| Preset | Pairs | What it is |
|---|---|---|
| `modern` (default) | 83 | Every pair today's retail game allows among these ten races and ten classes |
| `all` | 100 | Every race with every class (mod-arac's own set) |
| `off` | 62 | The original pairs only (the website and launcher add nothing; the server keeps what it has) |

## The table

`yes` = an original pair (62). **new** = added by `modern` (21). `all` = only with the `all` preset (17). Ids are
the game's race and class ids. Death Knights are every race already (stock), and their rules are unchanged.

| Race | Warrior | Paladin | Hunter | Rogue | Priest | Death Knight | Shaman | Mage | Warlock | Druid |
|---|---|---|---|---|---|---|---|---|---|---|
| Human (1) | yes | yes | **new** | yes | yes | yes | all | yes | yes | all |
| Orc (2) | yes | all | yes | yes | **new** | yes | yes | **new** | yes | all |
| Dwarf (3) | yes | yes | yes | yes | yes | yes | **new** | **new** | **new** | all |
| Night Elf (4) | yes | all | yes | yes | yes | yes | all | **new** | **new** | yes |
| Undead (5) | yes | all | **new** | yes | yes | yes | all | yes | yes | all |
| Tauren (6) | yes | **new** | yes | **new** | **new** | yes | yes | **new** | **new** | yes |
| Gnome (7) | yes | all | **new** | yes | **new** | yes | all | yes | yes | all |
| Troll (8) | yes | all | yes | yes | yes | yes | yes | yes | **new** | **new** |
| Blood Elf (10) | **new** | yes | yes | yes | yes | yes | all | yes | yes | all |
| Draenei (11) | yes | yes | yes | **new** | yes | yes | yes | yes | **new** | all |

- New with `modern` (21): Human Hunter, Orc Priest, Orc Mage, Dwarf Shaman, Dwarf Mage, Dwarf Warlock, Night Elf Mage, Night Elf Warlock, Undead Hunter, Tauren Paladin, Tauren Rogue, Tauren Priest, Tauren Mage, Tauren Warlock, Gnome Hunter, Gnome Priest, Troll Warlock, Troll Druid, Blood Elf Warrior, Draenei Rogue, Draenei Warlock.
- Only with `all` (17): Human Shaman, Human Druid, Orc Paladin, Orc Druid, Dwarf Druid, Night Elf Paladin, Night Elf Shaman, Undead Paladin, Undead Shaman, Undead Druid, Gnome Paladin, Gnome Shaman, Gnome Druid, Troll Paladin, Blood Elf Shaman, Blood Elf Druid, Draenei Druid.

Source for "today's retail": the race/class table for the current expansion (judgehype.com, Feb 2026), the same 83
as `realm/modern_combos.sql`. **Owner decision when it goes live:** retail patch 12.2 (announced Sept 2026,
roadmap early 2027) adds Night Elf, Undead and Troll Paladins. Adding them to `modern` means the three pairs in
`realm/modern_combos.sql`, `realm/charfix_world.sql` (start rows, gear donor, bars, skills), the browser client's
`MODERN_RACE_CLASS`, `launcher/src-tauri/src/combos.rs` `MODERN` and `DONORS`, and this table.
`stream/tests/test_race_class_combos.py` fails until all of them agree.

## How each part does it

**Server** (the realm image already has mod-arac, `realm/custom/sources.lock`; it is data only, so there is no image
rebuild). `realm/charfix.sh` (Control `combos`):
- `modern`: `realm/charfix_world.sql` writes exactly the 83 pairs' start rows in each race's own zone (Death
  Knights: Acherus). It also writes their starting gear (the same class's outfit from a neighbouring race), action
  bars, weapon skills, class skill rows and class quests.
- `all`: on top of that, mod-arac's own `data/sql/db-world/arac.sql`, at the pinned commit. It comes from the
  realm's sources copy, else from GitHub, and its sha256 is checked before it runs. It adds the other 17 pairs'
  start rows, bars, spells and totems for the new shaman races, and opens class quests to every race.
  `ov_main.lua`'s start kit fills any missing gear at first login.
- Creation masks stay open (`AC_CHARACTER_CREATING_DISABLED*=0`), Death Knights open from the first character.
- The preset is kept in the realm profile (`OV_RACE_CLASS_COMBOS`, `realm/profiles/main.env`) and written to
  `/srv/wrath/portal-data/leyline/race-class-combos.txt` for the website. Backup first;
  `realm/Rollback character fix.bat` undoes it (the file follows the restored profile).
- Going back from `all` to `modern` removes the 17 pairs' start rows and restores the class quests. mod-arac's
  weapon-skill change (`playercreateinfo_skills` race masks) stays until a rollback.

**Website and browser client.** `stream/wrath_portal.py` `combos_preset()` reads that file (else
`WU_RACE_CLASS_COMBOS`, else modern):
- `/__env.js` sends `moderncombos=1` (and `allcombos=1` for `all`) for this realm and its Beacon only. Community
  realms and the Builders' Realm get nothing.
- `?moderncombos=0` on the page turns the extra pairs off; `?allcombos=0` keeps the 83; `?moderncombos=1` asks for
  them anywhere.
- The browser client shows the 100 with `allcombos=1` from client patch `wotlk-client-20261105a` on. Older builds
  show the 83.
- The network listing (`/wrath/realms.json`, `sample.combos`) tells the launcher.

**Desktop game (launcher).** `launcher/src-tauri/src/combos.rs`, before PLAY on the official realm:
- It reads the player's OWN `CharBaseInfo.dbc`, `SkillRaceClassInfo.dbc` and `CharStartOutfit.dbc` from their
  archives. It adds the preset's missing pairs, opens the class-bound skill rows to the new races, and copies an
  outfit row per new pair and sex.
- It writes them to `Data/patch-Y.MPQ` and `Data/<locale>/patch-<locale>-Y.MPQ`.
- The archive is rewritten only when the preset, the format or the player's tables change. It is taken out before
  any other realm. It is never written over someone else's archive in that slot, and mods no longer get slot Y.
- Only 3.3.5a game folders.

**What is not used:** mod-arac's `Patch-A.MPQ` and `patch-contents/DBFilesContent/*.dbc`. The module is AGPL-3.0,
but those files are modified copies of the game's own tables (game data), so they are never shipped or hosted. The
hub scout rejected the repo for that reason. Only mod-arac's SQL runs, on our own server, fetched from its project.
The launcher's rows are made from each player's own files.

## The owner's presses

1. `combos`: the server pass for the preset. A backup, the world tables, the profile, the website's preset file,
   then a world server restart (2-10 min). It ends with `CHARFIX OK: 83 race/class pairs open (modern)`. Safe to run
   again. For every race with every class, press `combos-all` instead; `combos-modern` goes back.
   `combos-check` is a read-only report.
2. `signin`: the website. `/__env.js` follows the preset (community realms no longer get the switch, and
   `?moderncombos=0` works), and the realm list carries `sample.combos`. Not `page`: no page-layer file changed.
3. `ship`: the launcher with `combos.rs`. Players get it from the Leyline repo's CI build after `leyline`.
4. `foreverui` and `addons`: the in-game realm guide's new line (Start and boost page) reaches the browser and
   desktop players. The site's notes page already comes with `signin`. Optional; nothing else needs them.
5. Only for `all` in the browser: `client-build` (about an hour), then `client`. Without it, the browser offers the
   83 under `all`.

To try it: a new character with a new pair (a Tauren Paladin, a Human Hunter, a Dwarf Shaman) in the browser and in
the desktop game started with PLAY from the launcher. Each should start in its race's zone with its class's gear.
