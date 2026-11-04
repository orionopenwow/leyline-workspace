# Seeded realm: addendum (zero-setup seeding, one client, a build-your-own-class ruleset)

Written 4 Nov 2026 for the owner's ask:

> "plan for a seeded Turtle WoW, Ascension server, research and tuck it away; it should be P2P self-hosted,
> invisible and user-intuitive, without much setup, and work right away; clever methods."

This adds to `docs/SEEDED-REALM-PLAN.md` (3 Nov). It does not replace it. The lease, epoch, witness, Keeper and
log-shipping design there stays as written. This is a plan. Nothing here is built.

Internal doc. The two server names in the ask are used here only to describe the ideas. They never go on a page, a
realm name or a post. The player-facing names are "Seed", "Keeper" and "build-your-own-class ruleset".

---

## 0. The short answer

1. **Make seeding a launcher feature, not a host kit.** The launcher downloads a prebuilt server for the PC's OS
   (built from public source in our CI, source published beside it), runs a portable database as a child process,
   and extracts map data from the player's **own** client in the background. No Docker. No compile.
2. **Put the first Seed on the one client we already centre on (3.3.5a).** AzerothCore plus
   mod-individual-progression held at its Vanilla stage. That is our existing `vanillaplus` ruleset. It gives us
   Lua (mod-ale), the browser slice and the Den family on one client. Keep the 1.12 CMaNGOS Seed as a second,
   later profile. This flips S1 in the 3 Nov plan: an owner decision (A1).
3. **"Invisible" means one honest opt-in, then silence.** After one plain "Yes", the PC prepares in the background
   while idle and plugged in. It climbs a ladder: archiver, then standby, then (after one more "Yes") Keeper. It
   never hosts a world for strangers without that second "Yes", because a host can edit anything.
4. **A build-your-own-class ruleset is possible server-side, with limits.** On the stock client a character can
   learn existing spells from other classes that use the **same resource** (mana with mana). New spells, mixed
   resources and a mixed talent window need client changes. Those are out. A picker panel in our own addon does
   the UI.
5. **Legal weight is real and rises with each step.** Both projects named in the ask were sued in 2026. One got
   a permanent injunction. The other settled and shut down. Shipping server binaries from our releases adds weight
   compared with building on each PC. The owner should read section 4 before anything is built.

---

## 1. Invisible, zero-setup seeding

### 1.1 What "invisible" can honestly mean

| The wish | What we can do | What we can't, and why |
|---|---|---|
| Help without knowing | Help **without effort**, after one clear opt-in | Without knowing at all. The volunteer holds other players' character data and runs an emulated server. Both carry weight (section 4). They must agree in plain words once |
| No setup | No Docker, no compile, no ports, no typing | Nothing. Hosts and Keepers only dial out (the relay tunnel). Only "carry traffic for others" (be-a-node) needs open ports and a public name, so it stays an advanced switch |
| Works right away | A world that is ready in about a minute once prepared | Instant on first opt-in. The first map extraction from the player's own client takes minutes, and we may not ship that data |
| Any PC can host | Any PC can **archive** and **stand by** | Any PC can **host**. Whoever hosts can edit any row. Hosting stays with Keepers the owner (or a steward) approved |

### 1.2 The five methods, evaluated

| | Method | Effort | Risk | Verdict |
|---|---|---|---|---|
| a | **Prebuilt native server per OS** from our CI, portable database, extraction from the node's own client in the background | L (2 to 3 weeks) | Medium: legal weight of shipping binaries; Windows build of the core; database as a child process | **Yes. The core of the recommendation** |
| b | **Grow Den Lite into a "lite realm"** | Years for a real realm | High | **No for a real Seed.** Yes for instant small rooms (Den, Dojo, minigames), as today |
| c | **WASM / browser-hosted pieces** | M for a toy, impossible for a realm | High | **No for a Seed.** Maybe Den Lite in a tab, later |
| d | **Split roles**: light nodes archive and relay; a few strong PCs host | M on top of the 3 Nov plan | Low | **Yes. Already the 3 Nov shape**; this adds automatic tiers |
| e | **One client for everything**: Vanilla on 3.3.5a via mod-individual-progression | S (the ruleset exists) | Low to medium: it is not true 1.12 | **Yes for the first Seed.** 1.12 CMaNGOS later |

**(a) Prebuilt native server.**
- Today the 1.12 profile compiles on each PC: "40-90 minutes and about 5 GB of disk", then "30-60 minutes, plus 1-3
  hours for the monster paths" (`hosting/kit/profiles/cmangos-1.12.1/profile.lock`). That is the slow part.
- The 3.3.5a kit already skips the compile: it uses the AzerothCore project's own prebuilt tools image
  (`acore/ac-wotlk-tools`, `hosting/kit/host-wrath-realm.sh`) and extracts "dbc + maps ... (a few minutes)".
  So prebuilt AzerothCore binaries are normal practice for that project. What remains is Docker.
- The plan: a CI job builds `worldserver`, `authserver` and `map_extractor` for Windows and Linux at the pinned
  commit, with our pinned module set. The release carries the binaries, their sha256, and a source archive of the
  exact tree. AzerothCore states "The AzerothCore source code is released under the GNU GPL v2"
  ([AzerothCore README][ac]). The modules are GPL or MIT. Publishing the full corresponding source with each
  binary meets the GPL. mod-individual-progression is MIT ([module page][ip]).
- The launcher downloads them on first opt-in and checks the sha256, exactly as it already does for Den Lite
  (`launcher/src-tauri/src/host/lite.rs`, `place()`: "refused, and the download deleted, otherwise").
- **The database.** Our AzerothCore stacks run MySQL 8.4 (`hosting/kit/docker-compose.host.yml`); the CMaNGOS
  profiles run MariaDB 11.4. Both ship a no-install ZIP for Windows. MySQL: "This format does not include an
  automated installer" ([MySQL 8.4 docs][mysqlzip]). MariaDB's ZIP is "useful for portable installations", and
  `bin\mariadb-install-db.exe` "without parameters" creates a data directory ([MariaDB ZIP docs][mdbzip]). The
  launcher starts it on `127.0.0.1` on a free port, with its data directory under the app folder, and stops it
  with the launcher's seed service. An embedded library is not needed.
- **The world database, fast.** The AzerothCore world DB is the project's own SQL (open, not game data). CI imports
  it once at the pinned commit and publishes a compressed physical snapshot. A node restores it in about a minute
  instead of running the updater. Every node then has byte-identical world tables, which makes attestation simple.
- **Map data, never shipped.** The node runs `map_extractor` against its own game folder, which the launcher already
  knows ("Game folders"). It runs at low priority, only while idle and plugged in, and pauses when the player
  comes back. vmaps and mmaps stay off at first, as the kit already does (`OV_PATHING:-0`). The AzerothCore wiki
  calls mmaps "Optional but extremely recommended" ([AzerothCore setup][acsetup]); Keepers can build them overnight
  later. The AzerothCore wiki also offers prebuilt enUS data downloads ([AzerothCore setup][acsetup]); **we never use
  or point to those**: game data comes from each player's own client only.
- Memory: the AzerothCore realm uses "about 4 GB" and wants 8 GB in the PC, 16 GB to keep using it
  (`docs/leyline/always-on.md`). A standby runs only the database (about 0.5 GB, 3 Nov plan 2.10).

**(b) Den Lite toward a "lite realm".**
- Den Lite is about 22 000 lines of Rust (`labs/den-lite/src`). It has login, the world handshake, movement,
  props, a class picker and a sandbox combat.
- A real Seed needs creatures with AI, quests, loot, inventory, groups, instances, pathing, map heights and a
  character store. Den Lite has no map files (README, unknown 4: "the floor is the fixed height"). Its own notes
  say "For a realm with more than that, the full Docker Den is the way."
- Rewriting a world core is years of work and would duplicate what AzerothCore does under the GPL.
- **Keep it for what it is good at:** instant rooms with no data and no database. One clever use: it is the
  cheapest place to **test** what the stock client accepts (section 2.4).

**(c) WASM / browser.**
- A browser tab cannot listen for connections. It can dial out over WebSocket, which is what our host tunnel does,
  so a tab could in theory host behind the relay.
- AzerothCore plus MySQL in WASM is not realistic: native threads, a 4 GB wasm32 memory ceiling against a realm
  that uses about 4 GB, and a database engine.
- A tab can't run the extractor against a game folder without asking for folder access each time, and the tab
  must stay open.
- Den Lite is pure Rust over sockets and could compile to WASM. That is a Den feature for later, not seeding.

**(d) Split roles.** The 3 Nov plan already has Host, Keeper, Witness, Relay and Archiver. This addendum adds
**automatic tiers** from what the launcher measures, so nobody configures a role:

| Tier | Needs | Does | Player sees |
|---|---|---|---|
| **Archiver** | any PC, about 1 to 5 GB disk, outbound only | keeps the encrypted, signed chunks; can't read them | "Helping keep 2 realms safe" |
| **Standby** | 8 GB RAM, the game folder, extraction done, outbound only | holds a database replica, world stopped | "Ready to step in" |
| **Keeper** | 16 GB RAM, plugged in, good uplink, 14 days as Standby with 95 % up, **plus a second Yes and approval** | may be elected host | "Keeper of Seed name" |
| Relay | public name and open ports (be-a-node) | carries traffic | advanced switch, unchanged |

**(e) One client: Vanilla on 3.3.5a, compared with the 1.12 plan.**

| | 1.12 CMaNGOS Seed (3 Nov plan) | Vanilla on 3.3.5a (AzerothCore + mod-individual-progression) |
|---|---|---|
| Client players need | a separate 1.12 client | the one client the launcher, Den and browser use |
| Exists today | the `cmangos-1.12.1` profile | the `vanillaplus` ruleset (`ProgressionLimit = 7`, cap 60, 1.5x XP; `hosting/INSTALL-OPERATE.md` 4c) |
| Our Lua (`ov_*.lua`) | no engine on the pinned core | runs now on mod-ale |
| Browser play | no | yes (the 3.3.5a slice) |
| Memory per host | about 0.6 to 0.9 GB | about 4 GB |
| Build and extract | compile 40 to 90 min plus extract | prebuilt today (Docker); native with method (a) |
| Feel | true 1.12 talents, spells and interface | Vanilla content and tuning, but the later client's talent window, interface and spell ranks |
| Client patch needed | none | none. The module's MPQ is **optional** (it only restores old reagents and recipes; "Without the patch the module still works") ([module page][ip]). We don't use it |
| Build-your-own-class ruleset | not possible (no Lua) | possible (section 2) |
| Den family | 1.12 is being dropped for the Den (`docs/DEN-335-PLAN.md`) | same client |

**Recommendation:** the first Seed is Vanilla on 3.3.5a. It costs more memory per host, so only 16 GB Keepers host.
The 1.12 CMaNGOS Seed stays a second profile for the "true 1.12" crowd, once method (a) also builds CMaNGOS in CI.

### 1.3 The recommended flow: screens and words

All player words here are drafts. Lines in the owner's voice stay `TODO(owner)`.

**Where it is offered.** One card, once: on the Realms page after the player's third session, and always in
Settings > Network. Never a popup during play. Never asked again after "Not now", unless they open Settings.

**The card (opt-in 1):**

> **Help keep community realms alive**
> Your PC can keep a safe copy of a community realm while you're away. It runs only when this PC is idle and
> plugged in. It uses up to 5 GB of disk and a little upload. It never shares your game files.
> What it means: your PC holds other players' characters for that realm, and it helps run a fan-made server.
> `TODO(owner): one sentence on why it matters.`
> [Yes, help] [Not now] [What does this do?]

**Then, in the background, with no further screens:**
1. Download the Seed engine (server, database, extractor). sha256 checked. Paused on metered networks.
2. Restore the world snapshot.
3. Extract map data from the player's own game folder at low priority. Pause when they use the PC.
4. Join as Archiver right away, Standby once 1 to 3 are done.

**Settings > Network > Realm helping** (the only place it shows):
- One line of state: "Ready to step in · Keeping Vanilla Seed · 1.2 GB used".
- Caps: disk, upload, "Only while idle and plugged in" (on by default).
- [Pause for today] [Turn off and delete everything].
- A small badge on the profile: "Realm helper". Optional, off by default.

**Opt-in 2, only when the PC qualifies** (a quiet note in Settings, plus one notification):

> **You can become a Keeper**
> Your PC has been ready for 14 days. A Keeper may run the realm for everyone when it is their turn. Hosting uses
> about 4 GB of memory and stays on while the realm needs it. You can hand over at any time. The Leyline team
> approves Keepers.
> `TODO(owner): the plain-words note on holding others' data and the legal risk.`
> [Ask to be a Keeper] [Not now]

**When a Keeper's PC is about to sleep or shut down,** the launcher catches the OS's sleep and shutdown notices and
does a planned handoff (3 Nov plan 2.4). If the OS gives too little time, the lease runs out and another Keeper
takes over; with live replication the loss is about 60 s (3 Nov plan 2.5).

**What players of the realm see:** the realm row's "Seeded · community-run" badge, and "Moving to a new host…" on the
play bar during a handoff. Nothing else.

### 1.4 Effort and risk for the recommendation

| # | Piece | Eff | Notes |
|---|---|---|---|
| E1 | CI builds AzerothCore (pinned commit + our module set) for Windows and Linux; source archive and sha256 in the same release | M | Windows build of the core is the main unknown; AzerothCore has a `windows-build` CI badge ([AzerothCore README][ac]) |
| E2 | CI world-DB snapshot for MySQL 8.4 | S | |
| E3 | Launcher `host/seed.rs`: download and check (as `lite.rs`), start and stop the portable database and the servers, idle/plugged gate | M | |
| E4 | Background extraction from the player's own folder, pausable | S | the extractor already runs in the kit |
| E5 | Tiers, the two opt-ins, Settings panel, sleep/shutdown handoff hook | M | |
| E6 | Port the 3 Nov replication from MariaDB tools to MySQL 8.4 for the AzerothCore Seed (GTID, semisync, backup tool) | M | both have GTID and semisync; the backup tool differs |
| E7 | `first-run-test`-style proof: fresh profile (`LEYLINE_APP_DIR`), opt-in, prepared, takes over in a drill | M | shares the fresh-machine test work |

About 4 to 5 weeks of agent work on top of the 3 Nov phase 0.

| Risk | What we do |
|---|---|
| Shipping server binaries adds legal weight over "built on your own PC" | Owner decision A5. Fallback: CI builds, but the launcher compiles locally (slow path) |
| A database left running or corrupted on someone's PC | Child process with a lock file; stop on launcher exit; "delete everything" button; replica only, rebuilt from a snapshot if damaged |
| The player's PC feels slower | Idle and plugged-in gate; low priority; disk and upload caps; never hosts without opt-in 2 |
| Extraction differs by client language | Attest hashes of the extracted tables (3 Nov plan 2.7); a node that differs stays an Archiver |
| Volunteers didn't understand the weight | Plain words in both opt-ins, written by the owner |

---

## 2. A build-your-own-class ruleset

### 2.1 What the stock client allows (server-side only)

The rule: no client modification shipped, no new game data, no decompiled code. Our own **addons** are allowed (they
are approved mods, like WrathUnleashedUI). That gives this split:

| Idea | Server-side only? | How, or why not |
|---|---|---|
| Learn existing spells from other classes | **Yes, with limits** | mod-ale `LearnSpell` (our own scripts already use it). The spell must exist in the client already |
| Mixed resources (a mana caster using rage abilities) | **No** | The client shows one power bar per class. A published classless module gives every hero "mana, rage and energy" only with a patched executable ([classless-wildcard][cw]). So: picks only from classes that share the character's resource |
| Extra talent points | **Yes** | mod-ale `Player:AddBonusTalent( count )`: "Add bonus talents count to the Player" ([ALE API][bonus]) |
| Talents from another class's tree in the talent window | **No** | The talent window reads the client's own class tables. Changing it needs client data changes |
| Other classes' **passive** talent effects, as "perks" | **To test** | A talent rank is a spell. Learning it server-side may apply the passive. Whether it shows anywhere is to test (2.4) |
| A picker panel ("choose your next ability") | **Yes** | Our own addon plus addon messages (mod-ale `SendAddonMessage`, already used in our scripts) |
| Wear any armour, use any weapon | **Partly** | Server checks can be relaxed in a module; the client may still grey items out. To test |
| New spells, icons, items, races | **No** | New client data. Out |
| Free respec, rebuild at a trainer | **Yes** | mod-ale (`ResetTalents` is already used in our scripts) |
| Challenge modes, slow XP, "one-class purist" titles | **Yes** | Lua, like the 3 Nov plan's phase 3 |

**Both public modules for this need client changes, so we don't use them.**
- classless-wildcard: "Every player must run the client patch"; its installer patches the game executable
  ([classless-wildcard][cw]).
- spelldraft: players "install a `patch-P.mpq` archive" and the server needs DBC files ([spelldraft][sd]).
- Their licences (GPL) would let us read them. We still write our own, smaller design, to keep it server-side only.

### 2.2 The ruleset, v1

Name: **build-your-own-class ruleset** (player word: "Open build"; `TODO(owner)` for the final name).

1. Every character keeps its class at creation. The class sets its resource, armour type and talent window.
2. Every few levels the character gets an **Ability pick**: choose 1 of 3 offered abilities. The pool is every
   existing trainer spell of every class **that uses the same resource** at or below the character's level.
   Mana classes share a large pool. Rage and energy classes get a smaller pool plus resource-free abilities.
3. **Bonus talent points** at set levels (`AddBonusTalent`), spent in the character's own talent window.
4. **Perks:** a short, curated list of other classes' passive talent ranks, offered as picks. Only the ones the test
   (2.4) shows work cleanly.
5. A **Rebuild** at any trainer: forget picks and choose again (free or a gold cost; the owner decides).
6. The picker is a panel in our own addon. Without the addon, `.build pick 1|2|3` in chat does the same.
7. Balance: creature health and damage up a step on this ruleset (the classless module's own notes say heroes end
   "much stronger" and advise raising creature rates ([classless-wildcard][cw])). Tuned from play.

It is a ruleset pack on the same Seed engine: `vanillaplus` plus a script folder and an addon. It could also run on
an ordinary community realm.

### 2.3 What it is not

- No classless "hero" class. That needs a new class in client data.
- No resource mixing, no new spells, no new talent trees. Each of those needs a client patch.
- Nothing from the sued projects: no code, no assets, no names, no "points" shop. In particular, the September 2026
  public upload of one project's server code ([Kami Labs][kami]) is never read or used.

### 2.4 Test first, cheaply: Den Lite

Den Lite already has a class picker and a sandbox combat (`src/classes.rs`, `src/combat.rs`), and the owner's real
client logs in to it. Before any realm work, a half-day test there answers the unknowns with the stock client:
- Does a warrior's spellbook show a learned mage spell, and does the client let it cast?
- Does a mana class cast another mana class's spell cleanly (cast bar, cooldown, tooltip)?
- Does a learned passive talent rank from another class show anywhere?
- Does the client grey out an item the server allows?

Then confirm the same on a test AzerothCore realm with mod-ale.

---

## 3. Phased plan

Sizes as in the 3 Nov plan: S a day or less, M 2 to 5 days, L a week or more.

| Phase | What | Eff | Gate |
|---|---|---|---|
| **0. Decide** | Owner reads sections 1.1 and 4, answers A1 to A7 | owner | |
| **1. Seed engine** | E1 to E4: CI binaries with source, world snapshot, `host/seed.rs`, background extraction. A local "Host this realm with no Docker" button is the first visible use | L | a fresh profile goes from opt-in to "Ready" with no Docker; game data never leaves the PC (checker test) |
| **2. Seeding, invisible** | E5 to E7 plus the 3 Nov phase 0 (lease, relay gate, log shipping, handoff) on the AzerothCore Seed | L | the 3 Nov go/no-go (crash loss ≤ 90 s, back in ≤ 5 min, 72 h green) |
| **3. Open build test** | The Den Lite test (2.4), then the same on a test realm | S + S | the stock client shows and casts same-resource picks |
| **4. Open build ruleset** | Lua scripts, addon panel, bonus talents, perks, rebuild, creature tuning, a ruleset pack | M to L | owner playtest; brand check; no client file touched |
| **5. True 1.12 Seed** | CI builds CMaNGOS Classic too; same engine | M | owner's yes |

---

## 4. Legal context

- **The Classic+ server:** a US court issued a permanent injunction in April 2026 ([PC Gamer][pcg]). The order told the
  developers to "cease development, programming, coding, operation, updates, support, and maintenance"
  ([GIGAZINE][gig]). Servers went offline "at 0:00 on May 15, 2026" ([GIGAZINE][gig]). The settlement terms
  reportedly also bar "building or distributing modified or 'remastered' clients" ([Icy Veins][iv]).
- **The classless server:** the publisher filed suit on 12 June 2026 in a California court ([Aftermath][aft]). The
  claims: copyright infringement for distributing copies of the game software, removing security protections so
  the software would run, DMCA violations, false designation of origin over trademarks, and earning "millions"
  from an in-game currency ([Aftermath][aft]). Its pitch was a "Classless game that allows you to imagine and build
  the character of your dreams" ([Aftermath][aft]). The parties settled; the terms "are confidential"; the realms
  went off on 5 September; the court dismissed the case on 6 October 2026 ([Kami Labs][kami]).
- **What our lines avoid:** the claims named above were about distributing the client, defeating its protections,
  money, trademarks and a custom client. Our rules forbid each of those.
- **What our lines don't change:** running an emulated server is still what was sued. A ruleset that looks like the
  classless server's main feature draws attention. Shipping server binaries from our org (method a) is one more
  act of distribution than building on each PC. Spreading hosting spreads the exposure to volunteers; it doesn't
  remove it. Hence the plain words in both opt-ins and the one-switch retire from the 3 Nov plan (section 6).
- This is not legal advice. The 3 Nov recommendation stands: a short legal read by the owner before any Seed is
  shown.

---

## 5. Owner decisions

| # | Question | Recommendation |
|---|---|---|
| **A1** | First Seed on 3.3.5a (Vanilla via `vanillaplus`) instead of 1.12 CMaNGOS? Flips S1 | Yes. 1.12 later as a second profile |
| **A2** | Where the opt-in appears: Realms card after the third session plus Settings? | Yes. Never during play, never repeated |
| **A3** | Keepers: approved by you or a steward after 14 days as Standby plus a second Yes? | Yes. Never automatic |
| **A4** | Minimum to host: 16 GB RAM, plugged in, measured uplink? | Yes |
| **A5** | Ship prebuilt server binaries from our GitHub releases (with source), or build on each PC? | Prebuilt, after your legal read. Fallback: local build |
| **A6** | The build-your-own-class ruleset: go after the Den Lite test? Same-resource picks only? | Test first (S). Then decide |
| **A7** | Player names: "Seed", "Keeper", "Realm helper", "Open build"? | `TODO(owner)` |

---

## Sources (read 4 Nov 2026)

- [ac]: AzerothCore repository README (GPL v2 line; `windows-build` CI badge), https://github.com/azerothcore/azerothcore-wotlk/
- [acsetup]: AzerothCore wiki, Linux server setup (dbc, maps, vmaps; mmaps "optional but extremely recommended";
  prebuilt enUS data offered, which we don't use), https://www.azerothcore.org/wiki/linux-server-setup
- [ip]: mod-individual-progression (MIT; the optional client MPQ; works without it),
  https://github.com/ZhengPeiRu21/mod-individual-progression
- [mysqlzip]: MySQL 8.4, choosing an install package (no-install ZIP),
  https://dev.mysql.com/doc/refman/8.4/en/windows-choosing-package.html
- [mdbzip]: MariaDB, installing the Windows ZIP packages,
  https://mariadb.com/docs/server/server-management/install-and-upgrade-mariadb/installing-mariadb/binary-packages/installing-mariadb-windows-zip-packages
- [bonus]: ALE (AzerothCore Lua Engine) API, `Player:AddBonusTalent`, https://www.azerothcore.org/eluna/Player/AddBonusTalent.html
- ALE overview ("a fork of the Eluna Lua Engine"; repo azerothcore/mod-ale), https://www.azerothcore.org/eluna/
- [cw]: mod-classless-wildcard wiki (client patch required; one resource bar; creature tuning),
  https://github.com/DustinHendrickson/mod-classless-wildcard/wiki
- [sd]: mod-spelldraft (needs `patch-P.mpq`, DBC files and mod-ale), https://github.com/bdodroid/mod-spelldraft
- [pcg]: PC Gamer, the injunction (April 2026),
  https://www.pcgamer.com/games/world-of-warcraft/blizzard-wins-injunction-against-vanilla-world-of-warcraft-private-server
- [gig]: GIGAZINE, the order's wording and the shutdown date, https://gigazine.net/gsc_news/en/20260421-turtle-wow-down/
- [iv]: Icy Veins, the settlement terms, https://wp-prod.icy-veins.com/turtle-wow-is-over-blizzard-settlement-forces-shutdown/
- [aft]: Aftermath, the June 2026 lawsuit, https://aftermath.site/blizzard-project-ascension-lawsuit/
- [kami]: Kami Labs, after the shutdown (settlement, dismissal, the 7 September code upload),
  https://kami-labs.fr/en/ascension/ascension-coa-apres-fermeture-proces/

Not read (blocked during research): the AzerothCore requirements page (supported database versions) and the ALE
pages for `LearnSpell` and `SendAddonMessage`. Our own scripts already call both methods (`realm/custom/lua_scripts`).
Check the database versions before E1.

Repo facts cited: `hosting/kit/profiles/cmangos-1.12.1/profile.lock`, `hosting/kit/host-wrath-realm.sh`,
`hosting/kit/docker-compose.host.yml`, `hosting/INSTALL-OPERATE.md` 4c, `docs/leyline/always-on.md`,
`docs/leyline/be-a-node.md`, `docs/DEN-335-PLAN.md`, `labs/den-lite/README.md`, `launcher/src-tauri/src/host/lite.rs`.

[ac]: https://github.com/azerothcore/azerothcore-wotlk/
[acsetup]: https://www.azerothcore.org/wiki/linux-server-setup
[ip]: https://github.com/ZhengPeiRu21/mod-individual-progression
[mysqlzip]: https://dev.mysql.com/doc/refman/8.4/en/windows-choosing-package.html
[mdbzip]: https://mariadb.com/docs/server/server-management/install-and-upgrade-mariadb/installing-mariadb/binary-packages/installing-mariadb-windows-zip-packages
[bonus]: https://www.azerothcore.org/eluna/Player/AddBonusTalent.html
[cw]: https://github.com/DustinHendrickson/mod-classless-wildcard/wiki
[sd]: https://github.com/bdodroid/mod-spelldraft
[pcg]: https://www.pcgamer.com/games/world-of-warcraft/blizzard-wins-injunction-against-vanilla-world-of-warcraft-private-server
[gig]: https://gigazine.net/gsc_news/en/20260421-turtle-wow-down/
[iv]: https://wp-prod.icy-veins.com/turtle-wow-is-over-blizzard-settlement-forces-shutdown/
[aft]: https://aftermath.site/blizzard-project-ascension-lawsuit/
[kami]: https://kami-labs.fr/en/ascension/ascension-coa-apres-fermeture-proces/
