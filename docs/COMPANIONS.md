# Bots for dungeons and raids

No group? Bring bots. They fight, heal and follow you, and they are always labelled as bots.

## Commands

| Command | What it does |
|---|---|
| `.recruit party` | fills your group to 5: 1 tank, 1 healer and damage |
| `.recruit raid` | fills your group to 10: 2 tanks, 3 healers and 5 damage, and makes it a raid |
| `.recruit tank`, `healer` or `dps` `[n]` | adds bots of one role. `.recruit dps mage` picks the class. |
| `.recruit alt <name>` | brings one of your own characters as a bot, with its own gear, spec and level |
| `.companions alts` | lists the characters you can bring |
| `.companions save` | remembers the alts with you as your usual party |
| `.recruit usual` | brings your usual party back |
| `.buddy [class]` | a levelling buddy that keeps to your level. `.buddy off` sends it home. |
| `.dismiss [name\|all]` | sends bots home |
| `.companions` | help, the limits and who is with you |

The fills count what your group already has. Bring a friend who tanks, and `.recruit raid` adds one tank, not two.

In party chat you can tell bots: `follow`, `stay`, `attack`, `summon`.

## The Bots window

Type `/comp` (or `/companions`), or click the bots button on the minimap. The window has six tabs:
**Your bots**, **Party**, **Raid**, **Your alts**, **Buddy** and **Dungeons**. `/comp party`, `/comp dungeons` and so
on open a tab straight away.

- **Party:** "Fill my party" brings 1 tank, 1 healer and damage. Pick a class and a spec for each slot, or leave
  Any. Your picks are kept for this character, and Fill party and the Dungeons tab use them too.
- **Raid:** fills a 10-player raid (2 tanks, 3 healers, 5 damage) and shows which roles you have.
- **Your alts:** your other characters. Bring one, then "Save who is with me as my usual party". "Bring my usual
  party" brings them all back.
- **Buddy:** one levelling buddy. Pick a class or leave Any, then bring it or send it home.
- **Dungeons:** the dungeons for your level (tick "Every dungeon" to see them all), each with its Dungeon Finder
  picture. Press Go and bots fill your party, then you are taken to the door and your bots join you there. You can
  take one trip every 5 minutes. Not in combat, not as a raid, and only the group leader can do it.
- **Your bots:** each bot's role, class, spec, item level, health and mana, and what it is doing: following,
  staying, passive or dead. Every one is marked as a bot.
- **Orders:** Follow, Stay, Attack my target, Passive (then Fight), Assist tank (damage bots focus the tank's
  skull target), Crowd control, a formation, and Prepare for raid (spells, gear and buffs).
- **Group:** Fill party, Fill raid, My usual party, Dismiss all.
- **Click a bot** to pick its spec, swap its role (tank, healer or damage), give it fresh gear, or send it home.
  Your alts keep their own talents and gear.

## The bot bar and the orders bar

While bots are with you, two small bars show up. Drag either one to move it; it stays where you put it.

- **The bot bar** shows each bot's portrait, its name in its class colour, its role, its health and mana, and one
  word for what it is doing: following, fighting, resting, staying, passive or dead. Left-click a bot to target it
  (this works in combat too). Right-click it for its orders: follow, stay, attack my target, passive, gear up, its
  spec and role, or send it home. `+` opens the Bots window.
- **The orders bar** gives every bot one order at once: Follow, Stay, Attack my target, Passive (press again to
  Fight) and Pull (your tank bot opens the fight on your target, and the others join in). To give them keys, go to
  Esc > Key Bindings > Bots. The key you set shows on the button.

`/comp bar` and `/comp orders` turn the bars off and on again. The first time you have bots, three short tips
show you around. They don't show again, but `/comp tips` brings them back.

The window never types in your chat. "Hide bot chatter" (on by default) hides your bots' achievement lines and
their replies right after an order.

If the realm restarts while you have bots, they come back with you. The realm tells you, and `.dismiss all` or
the window sends them home.

## Rules

- Up to 4 bots in a party, 9 in a raid. Your alts count too.
- One `.recruit` every 30 seconds. Only the group leader recruits.
- Not in battlegrounds or arenas, not in combat and not while dead.
- Inside a dungeon or raid you can refill a slot between pulls, once nobody in the group is fighting.
- Bots leave when you log out.
- Only characters on your own account can be brought as alts.
- Raids are 10-player. 25-player raids are off for now.

## Loot

Bots always pass. They never roll need or greed, under group loot or need before greed, and that includes your alts. Under master loot they pass too.

One thing they still do: items below the group's loot threshold (grey and white) go round-robin, and a bot takes its turn like anyone. Want every item? Set loot to Free for All. Bots never loot then.

## Where bots know the fights

Bots get boss tactics on their own when they enter these places.

**Raids**

- Molten Core, Blackwing Lair, Onyxia's Lair, Ruins of Ahn'Qiraj
- Karazhan, Gruul's Lair, Magtheridon's Lair, Serpentshrine Cavern, Tempest Keep, Mount Hyjal, Black Temple, Zul'Aman
- Naxxramas, The Obsidian Sanctum, The Eye of Eternity, Vault of Archavon, Ulduar, Icecrown Citadel, The Ruby Sanctum

**Dungeons**

- Hellfire Ramparts, The Underbog, Auchenai Crypts, Sethekk Halls, The Mechanar, Magisters' Terrace
- Utgarde Keep, The Nexus, Azjol-Nerub, Ahn'kahet: The Old Kingdom, Drak'Tharon Keep, The Violet Hold, Gundrak, Halls of Stone, Halls of Lightning, The Oculus, Utgarde Pinnacle, The Culling of Stratholme, Trial of the Champion, The Forge of Souls, Pit of Saron

Everywhere else bots still fight, heal and follow. They just don't know special boss moves.
