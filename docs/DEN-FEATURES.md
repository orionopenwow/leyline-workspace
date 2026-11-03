# Dens: the full feature list

Owner, 3 Nov: "full featureset for Dens to work". This is every part of a Den in one place: what works now, what is
next, and the research behind the choices. "Den Lite" is the small local server the launcher starts for Your Den
(`labs/den-lite`); "the full Den" is a real realm with the Den scripts (`ov_den.lua` and friends). The details are in
`labs/den-lite/README.md` and `docs/DEN.md`.

Status words: **done** (built and tested here), **to see** (built, the real game must still show it), **next**
(planned), **full Den** (needs the full Den).

## 1. Getting in

| Feature | Status | Notes |
|---|---|---|
| One button: launcher to standing in your Den | done | the launcher's den-play; the PC den-test passes (in world in about 12 s) |
| Standing and walking on the Den's ground | done | 5 cm above the ground; the den-test's walk and key checks pass |
| Den bar, Den guide, no chat spam | done | toasts in the Den bar; `.den help` opens the guide |
| A fresh computer, through to the world | next | the first-run test (another agent: `tools/first-run`) |

## 2. The editor

| Feature | Status | Notes |
|---|---|---|
| Gallery with 3D tiles (your own models) | done | models from your own GameObjectDisplayInfo.dbc |
| Place, select, move, turn, size, copy, paste, duplicate, delete, undo, redo | done | the PC feature test passes every one |
| Live preview, the gizmo, select by right-click | done, to see | the build twin entries make props glow; left-click select is impossible (the client never says) |
| Exact placement by the game's own ground circle (G) | done | the calibration's truth |
| The build camera (fitted from your clicks) | done, to see | hover calibration on the PC |
| Stamps and Jaina building them piece by piece | done | 8 stamps, the den-test builds each |
| Save, open, autosave, backups | done | |
| Room sizes 1-7, city sizes | done | |
| Shape the ground (Forge) | done | the launcher's terrain delta |
| My models (drag in your own .m2 / .wmo) | done | never uploaded |
| The editor rebuild in progress | next | another agent (Builder.lua, Gizmo.lua, gizmo.rs) |

## 3. Places and friends

| Feature | Status | Notes |
|---|---|---|
| Den anywhere (`.den go`, `.den here`) | done, to see | near and far teleports; the far one's loading screen to see |
| Copy a place from your game (`.den copy lionspride`) | done | made on your PC, never shipped |
| Invite a friend (relay, same network, your own address) | done | the den-test signs a friend in |
| A friend fights beside you, duels | full Den | Den Lite's combat is the host's alone |

## 4. Your character: class, spec, spellbook, bars

| Feature | Status | Notes |
|---|---|---|
| Any class at any level (`.den class mage 80`) | done | every spell the class learns, from your own Spell.dbc |
| Three specs per class (`.den class mage fire`, `.den spec frost`) | done | 30 specs |
| Talents: a level-80 build per spec, shown in the talent frame | done, to see | from your own Talent.dbc; the other trees' abilities left out |
| Every bar filled at once, per spec | done, to see | rotation 1-6, cooldowns, defensives, utility, mounts, buffs, stance and form bars |
| The four extra bars switched on | to see | PLAYER_FIELD_BYTES |
| Named bar loadouts (`.den bars save aoe`) | done | ten per class and spec, kept in the Dens folder |
| The class's own frames (runes, combo points) without a relog | to see | log out and in always works |
| Your own changes to the bar kept | done | per class and spec |

## 5. Gear and mounts

| Feature | Status | Notes |
|---|---|---|
| Any item's look in any slot (`.den gear`) | done | |
| Gear sets: the class's raid tiers T1-T10 (`.den gearset t7`) | done | found by name in your own ItemSet.dbc; the spec's version (healer, caster, fighter, tank) |
| The slots a set has no piece for, and weapons, from the same era | done | the same pick every time |
| Your own saved sets (`.den gearset save raid`) | done | |
| Weapons as real items (weapon-only spells cast) | done, to see | item objects and the item query answer |
| The tier in your numbers | done | health, power, spell or attack power, crit, hit (T7+ capped), a tank's armour |
| Every mount in your spellbook, on the right bar | done | flying ones with `.den fly on` |
| A gear picker with icons in the panel | next | the server's lists are ready (`wuden gear`) |
| Item names in tooltips | next | Item.dbc has none: "Dojo Staff" until a names table (TODO(owner): ship names? they are a server's data) |

## 6. Combat (the sandbox)

| Feature | Status | Notes |
|---|---|---|
| Casts, cast bars, the global cooldown, cooldowns | done | the game's own messages |
| Damage and healing from your own Spell.dbc, scaled by level and gear | done | |
| Crits, misses, dodges, glancing blows | done | misses only against higher levels (you're hit-capped against your own) |
| Armour on physical hits | done | the heavy dummy and the boss |
| Damage and healing over time, channels, area spells | done | |
| Mana, rage, energy, runic power, combo points | done | runes aren't spent (the game tracks them itself) |
| Buffs and debuffs on the target frame | done | |
| The combat log, misses included | done | |
| Procs (the talents' PROC_TRIGGER_SPELL) | done | their own spell, once a second at most |
| Spell modifiers from talents (ADD_PCT_MODIFIER) | next | each talent's family masks; big |
| Auto attack, Auto Shot | done | |
| Pets: summoned, follow, fight, their bar | done, to see | casters cast their bolt; reagents (Soul Shards) are the game's own check |
| Healing on dummies and yourself | done | |
| DPS and HPS meters, reset | done | |
| Resistances, parries, blocks | next | |

## 7. The Dojo (the test range)

| Feature | Status | Notes |
|---|---|---|
| The ring, built by Jaina | done | |
| Training dummies at your level | done | huge health, refilled |
| Dummies three levels below and above you (light and heavy armour, the skull) | done | `.den dojo levels` |
| A pack of five (area spells) | done | |
| A moving dummy | done | |
| A wounded dummy, and the healing range (five friends) | done | `.den dojo group` |
| A dummy that hits back (tank practice) | done | |
| The practice boss: circles to move out of, a cast to interrupt | done | kicks counted |
| Timed parses (1, 3, 5 min), best per class and spec | done | |
| A practice run: your bar cast for you, a number | done | `.den dojo demo 30`; the den-check's parse |
| The range: a dummy 40 yards out, markers at 5/10/20/30 | done, to see | the markers are the dummy's model, small |
| Reset, refill, cooldowns, invulnerable | done | |
| The panel: Train and Loadout pages | done | WrathUnleashedUI Dojo.lua |
| Duels, a dungeon queue with companions, raid training | full Den | |

## 8. Camera and play

| Feature | Status | Notes |
|---|---|---|
| Build camera and its calibration | done, to see | |
| Fly, speed 1-5 | done | |
| Play mode (F7) | done | |
| Game modes in the Den | full Den | `.den mode` says how |

## What the real game must still show

From the PC den-test (den-features and its Dojo steps, pictures in `logs\den-features`): the talent frame and the
extra bars after `.den class` without a relog; the stance bars as forms change; the weapon items accepted as
equipped (Backstab, Auto Shot); a pet's frame and bar; the boss's skull; the markers' look.

## Owner decisions

- The bar layouts per spec and the default spec per class (a first pass from the classes' usual builds).
- Which gear looks to offer beyond the raid tiers (the legendary weapon looks are there: `.den gear thunderfury`...).
- Item names in tooltips: Den Lite has none ("Dojo Staff"); names are a server's data.

## Research notes

- **Bars**: 3.3.5a's FrameXML: buttons 1-12 the main bar, 13-24 its second page, 25-36 the right bar, 37-48 the
  second right bar, 49-60 bottom right, 61-72 bottom left, 73-120 the bonus (form) bars (warrior: battle 73,
  defensive 85, berserker 97; druid: cat 73, prowl 85, bear 97, moonkin 109; rogue stealth 73; priest shadowform
  73). The client shows the extra bars by PLAYER_FIELD_BYTES' third byte (AzerothCore
  PLAYER_FIELD_BYTES_OFFSET_ACTION_BAR_TOGGLES); the stance bar it builds from the spellbook.
- **Talents**: AzerothCore's Player::BuildPlayerTalentsInfoData (u32 points, u8 specs, u8 active, per spec u8 count,
  u32 talent and u8 rank 0-4, 6 glyphs); Talent.dbc (23 fields) and TalentTab.dbc (24; class mask 20, page 22) from
  DBCfmt.h. Builds are made, not shipped: the spec's tree first, abilities first, the last talent at 51.
- **Pets**: AzerothCore's Player::PetSpellInitialize and CharmInfo::InitPetActionBar (Attack, Follow, Stay, four
  spells, Aggressive, Defensive, Passive; MAKE_UNIT_ACTION_BUTTON); HandlePetAction (pet, button, target).
- **Items**: the item query answer's layout from the message crate's own reader (3.3.5a); item objects
  (TYPEID_ITEM, owner and contained) in PLAYER_FIELD_INV_SLOT_HEAD (324) + 2 x slot.
- **Misses**: 3.3.5a's level-difference table (spells 4/5/6/17 per cent from 0 to +3; weapons 5/5.5/6/8, dodge 6.5 at
  +3, glancing 24 per cent of auto attacks at +3). The sandbox gives no misses at your own level or below.
- **Armour**: damage reduction = armour / (armour + 400 + 85 x level + 4.5 x 85 x (level - 59)) above 59; a raid
  boss's 10643 armour takes about 41 per cent off a level-80 player.
