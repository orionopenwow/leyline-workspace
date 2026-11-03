# Host a realm

From nothing to a running realm, in the Leyline launcher's **Host** page. It takes four steps and one press of **Host**. New to the network? Read [How the Leyline network works](network.md) first.

## What you need

- A PC that stays on while people play.
- Docker (free). It runs your realm's servers in their own space.
- Your own game folder, the Wrath-era game {{v:3.3.5a}}. The server reads the data it needs from it, on your PC.
- About 10 GB free.
- No open ports.

The top of the Host page has one line that says if this PC is ready. If something is missing it has one button that fixes it: **Get the Host pack**, **Start Docker** or **Get Docker**.

## What the server software is

Your realm runs on **AzerothCore**, an open-source realm server. We pack it with its modules (bots, dungeon scaling, transmog, the duel arena and more) and our own realm scripts into a **realm image**. **Docker** runs that image on your PC.

| Part | What it does |
|---|---|
| Database | Accounts, characters and the world. It stays on your PC. |
| Login server | Checks a player's account when they log in. |
| World server | The game world itself. |
| Leyline agent | Connects out to the Leyline relay on port 443. |

The **Host pack** is the folder that holds all of this and its tools. Leyline puts it in its own folder when you press **Get the Host pack**.

**The relay.** Players never connect to your PC directly. Your realm connects out to the Leyline relay, and the relay passes players' connections to it. So you open no ports, and nobody sees your address.

**Game files.** The game data a server needs (maps and tables) is read from your own game folder, on your PC. Leyline never sends game files, and the relay never carries them.

## Step 1: Private or Public

- **Private**: just you and friends you invite. Friends open your invite link once and make their own account there. Nobody else can find it.
- **Public**: on the Community realms list. Anyone can join.

You can switch later: Realm tools, **Public or private**. Public realms can also help carry the network: see [Be a node](#be-a-node).

## Step 2: Pick a template

A template fills in every setting. Change anything after.

| Template | What it is | World | Start | XP | Bots | Type | Best for |
|---|---|---|---|---|---|---|---|
| **Pure vanilla** | The first era as it was: the original rules, 1x everything, to level 60. | The first era only, to 60 | Level 1 | 1x | none | PvE | Purists |
| **Vanilla+** | The first era with comforts: 1.5x, faster rested, spells on level up, dungeons that fit your group. | The first era only, to 60 | Level 1 | 1.5x | none | PvE | Friends with jobs |
| **Original rules, every era** | 1x from level 1. Each character moves through the eras in turn, at their own pace. | Every era in turn, to 80 | Level 1 | 1x | 20 | PvE | The long road |
| **Classic levelling** | Level 1 to 80 at 3x, with a one-time boost for alts and a mount from level 1. | The whole world, to 80 | Level 1 or a boost | 3x | 40 | PvE | Most groups |
| **Fun realm** | Fast and loud: 10x, a boost to 80, a mount at level 1, gold to spend and events every hour. | The whole world, to 80 | Level 1 or a boost | 10x | 60 | PvE | Weekend chaos |
| **RP realm** | A normal pace, names that fit the world, no open-world PvP, and gentle evening events. | The whole world, to 80 | Level 1 | 1x | none | RP | Storytellers |
| **Hardcore** | 1x, no boost, no bots, no scaling. Players pick one life at the Shrine of Challenge. | The whole world, to 80 | Level 1 | 1x | none | PvE | Challenge seekers |
| **Solo with bots** | Just you, at 2x. Helper bots fill your group and dungeons scale to you. Private only. | Just me, with helper bots | Level 1 | 2x | 4 helpers | PvE | Playing alone |
| **PvP and duel ladder** | Everyone at 80. Open-world PvP, rated duels with weekly points, and 3x honor. | Everyone starts at 80 | Everyone at 80 | 3x | none | PvP | Fighters |
| **Event night** | A light hangout in Dalaran: hide and seek, trivia, minigolf and friends' game modes. | A light hangout in Dalaran | Level 1 or a boost | 3x | none | PvE | Game nights |
| **Blank** | No settings of ours: the Classic levelling ruleset exactly as it ships. Change what you like. | The whole world, to 80 | Level 1 or a boost | 3x | 40 | PvE | Doing it yourself |

"Solo with bots" is private only: helper bots need the solo ruleset, which runs for one player's PC.

**Your own templates.** Change the settings, then **Save as my template**. It shows as a card with the others.

**Share a code.** **Share as a code** copies a short code (it starts with `LLR1.`). A friend pastes it into **Paste a settings code** and gets your settings. A code holds settings only: never names, links or keys.

## Step 3: The settings

Every slider and switch is a real setting of your realm. Change one, and the summary says **Custom (from** the template**)**. A setting that does nothing on the world you picked is greyed out, and says why. Settings marked **Coming** are shown but not applied yet.

How they reach the realm:

- **World** picks the ruleset (a set of the server's own settings).
- **World server settings** go in the Host pack's `state/mode.json`, and the realm reads them each time it starts.
- **Realm options** (realm type, roaming bots, player limit, who can make an account) go in `state/options.json`. The realm reads that once. After that, Realm tools change them.
- **House rules** go in `state/rules.txt`. The realm list and the join page show them.

## Every setting

### Levels

| Setting | Takes | What it does |
|---|---|---|
| World and top level | The whole world, to 80, Every era in turn, to 80, The first era only, to 60, A light hangout in Dalaran, Everyone starts at 80, Just me, with helper bots | Picks the realm's ruleset. Just me is private only. |
| One-time boost to 80 | on or off | A new character may jump to 80 with a gear kit, or start at 1. Only on: The whole world, to 80, A light hangout in Dalaran. |
| Riding and a mount from level 1 | on or off | Comes with the boost choice. Only on: The whole world, to 80, A light hangout in Dalaran. Needs: One-time boost to 80. |
| Starting gold | 0 to 1000 gold | World server setting. |
| Learn class spells on level up | on or off | Off: visit trainers, as in the original game. |
| Any start level | coming | Coming. Today: level 1, the boost, or Everyone starts at 80. |

### Rates

| Setting | Takes | What it does |
|---|---|---|
| Experience | 0.5 to 10x | World server setting. |
| Rested bonus builds | 1 to 4x | World server setting. |
| Item drops | 1 to 5x | World server setting. |
| Gold from monsters | 1 to 5x | World server setting. |
| Reputation | 1 to 5x | World server setting. |
| Honor | 1 to 5x | World server setting. |
| Profession skill-ups | 1 to 5 pt | Skill points per skill-up. |

### World

| Setting | Takes | What it does |
|---|---|---|
| Roaming bots | 0 to 200 bots | Computer players who quest and queue. Each one uses some of your PC. Only on: The whole world, to 80, Every era in turn, to 80, The first era only, to 60, Everyone starts at 80, Just me, with helper bots. |
| Helper bots per player | 1 to 4 bots | Type .recruit party in the game to fill your group. Only on: Just me, with helper bots. |
| Dungeons scale to your group | on or off | World server setting. |
| Realm type | PvE: fight when you choose, PvP: open-world fights, RP: role-play, PvE, RP-PvP | Realm option. |
| Health and cooldowns back after a duel | on or off | World server setting. |
| Duel ladder (ratings, weekly points) | on or off | Level 80 duels count. .duel top in the game. |
| Events clock | on or off | Holidays and contests, announced 5 minutes ahead. |
| Trivia every other hour | on or off | Needs: Events clock. |
| Hide and seek in Dalaran | on or off | Needs: Events clock. |
| Teleports to the hubs (.hub) | on or off | World server setting. |
| Free repairs | on or off | World server setting. |
| Weather and time of day | coming | Change them live in the GM command center. A realm-wide default is coming. |

### Players

| Setting | Takes | What it does |
|---|---|---|
| Player limit | 0 to 500 players | 0 = the ruleset's own. When it's full, players wait in a queue. |
| Who can make an account | Only people with my invite link, Anyone, on my join page, Nobody new for now | Realm option. |

### Rules

| Setting | Takes | What it does |
|---|---|---|
| Names that fit the world | on or off | A house rule. |
| Hardcore challenge welcome | on or off | Each player picks one life for a character at the Shrine. Forcing it on everyone is coming. |
| Star out bad words in public chat | on or off | World server setting. |
| Level needed to talk in channels | coming | Coming. |

### Addons and modes

| Setting | Takes | What it does |
|---|---|---|
| Addons the realm suggests | None, Quester, Raider, Tidy interface | Players are offered them when they join. Only reviewed catalog addons. |
| Minigolf, Bumper Balls and Coin Dash | on or off | The game modes players built, in the hangout. Only on: A light hangout in Dalaran. |

## Ask Claude

Press **Ask Claude** and describe the realm you want, in plain words: "a cozy RP realm for 10 friends, slow levelling, no PvP". There are two ways to get an answer.

**Your own assistant.** If you connected Claude on the Agents page, press **Ask my assistant**, then tell Claude "Fill my Leyline realm settings". It reads your words and the list of settings. A card in Leyline shows what it wants to fill: press **Approve**. Leyline pays for nothing: it is your own assistant.

**Any AI chat.** Press **Copy the prompt** and paste it into any AI chat. Paste its whole answer back into the box and press **Fill the settings**. The prompt holds only your words and the settings list.

Either way:

- The settings land in the panel. Nothing is hosted until you press **Host**.
- Each choice gets one short line saying why.
- Unknown settings are left out, and numbers are kept inside their range. The panel says what was changed or left out.
- Who can join (Private or Public, and who can make an account) is always your own choice: an answer can't set it.

## Step 4: Host

Type your realm's name (and on macOS or Linux, your account name: it gets GM). Press **Host**. What happens:

1. Your settings are saved in the Host pack.
2. Docker starts the database, the login server and the world server.
3. The game data is read from your own game folder.
4. The realm connects out to the relay.
5. Private: you get an invite link. Public: it is on the Community realms list within a minute.

The first start takes 5 to 20 minutes. After that it starts in a minute or two.

## Your running realm

The realm's card shows if it is online and how many people play. It has:

- **Copy invite link** (Private) or **Copy its page** (Public).
- **GM command center**: teleport, spawn, time, weather and more, live.
- **Edit settings**: the same panel. **Save and restart** applies them (about a minute; anyone playing is dropped). **Save for the next start** waits.
- **Stop**: characters, accounts and your invite link are kept.
- **Open the console**: services, the health check, GM controls and Realm tools.

## Invite friends

Private realms: copy the invite link from the realm's card, or Realm tools, **Invite links**. Friends open it once and make their own account there. Leyline never makes accounts and never sees passwords. **New link** stops the old one working.

## Go public

Realm tools, **Public or private**. A public realm is on the Community realms list within a minute. Claim it with your Hero Key so it shows as yours: see [Hero Key](hero-key.md).

## Be a node

Optional. A node carries other players' connections to the network from your PC. It needs a public name and open ports, unlike hosting. See [Help carry the network](be-a-node.md).

## More ways to host

Under **More ways to host** on the Host page:

- **Game modes**: host a mode, fork it (a copy that is yours to change), or play it solo.
- **Other rulesets and older game versions**: pick a ruleset yourself, including realms for other game versions.
- **Put a server I already run on Leyline**: see [Keep your realm up 24/7](always-on.md) and [Bring your server](join-network.md).

## Backups and updates

- **Backups**: stop the realm, then copy the Host pack folder. Its `state` folder holds your realm's key and links. Characters live in Docker's storage for the realm.
- **Updates**: when a new Host pack is out, update it under **Host pack & settings**, then start your realm again. Its settings, characters and links are kept.

## When something's wrong

| What you see | What to do |
|---|---|
| The top line says Docker is closed | Press **Start Docker**, or open Docker Desktop yourself. |
| "Get the Host pack first" | Press **Get the Host pack** on the top line. |
| Setting up stopped | The card says why. Fix it, then press **Host** again: it is safe to run twice. |
| Friends can't get in | Run the health check on the console. Each line says what to fix. |
| A setting didn't change | Settings apply when the realm starts: use **Save and restart**. |
| Something else | Realm tools, **Server logs**. Or ask on the Discord. |
