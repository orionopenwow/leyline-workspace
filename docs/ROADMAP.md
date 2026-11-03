# Leyline master roadmap

Written 3 Nov 2026 from the owner's 03:00 brain-dump. It covers game modes, the Den and Dojo, seeded servers,
Vanilla Remastered, mods over P2P, and the community. One page to steer by; the detail lives in the docs it links.

Built alongside this, so not repeated here:
- **the Den "Dojo" training grounds** (branch `agent/dojo`);
- **the server queue and the "become a node / host / seed" prompts** (branch `agent/queue`);
- **the toolkit and succession doc** (`docs/TOOLKIT.md`, branch `agent/toolkit`).

Standing rules apply to every line below:
- no brand words to players, except the legal line;
- no game data hosted or shipped, except the approved browser slices;
- P2P never carries game files;
- nothing from decompiled clients, wowdecomp or Waystone;
- no money links;
- owner-voice copy stays `TODO(owner)`;
- mods stay **pending** until the owner approves them.

Legend:
- **P0**: needed for the phase it sits in. **P1**: next. **P2**: later.
- **Effort**: S is a day or less, M is 2 to 5 days, L is a week or more.
- **Lives in**: L = launcher, DL = Den Lite, AC = AzerothCore realm (C++ modules / Lua), W = website/portal,
  R = relay/mesh, K = host kit.

---

## 1. Tonight's launch (Phase 0)

### 1.1 The minimal feature set

Only what is built and has a smoke test. Each row passes its test, or it is hidden tonight.

| # | Feature | What players get | Smoke test that must pass |
|---|---|---|---|
| 1 | **Launcher with the sign-in screen** (1309a) | Log in with a Hero Key or Google, then add game folders and play | CI "leyline launcher" green (`gh run list -R orionopenwow/leyline`). Playwright `gate`, `firstrun`, `realms` and `playbar` pass. The owner's own install shows the sign-in screen first |
| 2 | **Website and Hero Key** | leylinelauncher.com: Log in, Create a Hero Key, realm list, forum, leaderboards | `signin` ends `FASTLOOP OK`. `audit` shows no new red against the last 88/93 |
| 3 | **Official realm, desktop** (Wrath Unleashed) | PLAY reaches character select by itself; companions; Dungeon Finder solo | `client-smoke` reaches the world (its screenshots in `logs\client-smoke`). `check` is green: worldserver up, update diff under 100 ms |
| 4 | **Your Den, solo** (Den Lite) | One click into the white world: build, save, autosave, invite | `den-test` passes (round 7 passed on 2 Nov; run it again on tonight's build) |
| 5 | **Community realms over the relay** | Realm list: Official and Community tabs, switching both ways | `check` (relay `/api/listing/realms.json` answers, mesh counts shown). By hand: official, then a community realm, then back |
| 6 | **Duels with weekly points** (`ov_duels.lua`, already on in `main.env`) | `.duel top`; `/wrath/leaderboards` | `check` plus one duel by hand on the realm, then the row shows on the site within the export interval |
| 7 | **Browser play in Dalaran** (the sampler slice as it is) | Play in browser from the site; own game folder for the rest | `slice-health` and `sampler-check` (read-only); the audit's browser walk reaches the world |
| 8 | **Server queue** (only if `agent/queue` lands and passes) | A fair line when the realm is full, with a "while you wait" card | The queue branch's own tests, plus `check` showing `PlayerLimit` as set. If it isn't merged and deployed by go time, leave AzerothCore's default limit of 1000 alone |

### 1.2 Hide, or label "Preview", tonight

| Thing | Tonight | Why |
|---|---|---|
| Dojo / training grounds | **Preview** badge on the Den page, off unless its own tests pass | New today; Den Lite has no combat (see 3.2) |
| Azeroth Survivors | **Coming soon** (as now) | Permission statement still `TODO(owner)`; the local-run decision is in 2.2 |
| Vanilla Remastered | Hidden from Official | Not relaunched; plan in 3.4 |
| Seeded servers | Not shown | Plan only (3.3) |
| Boss Rush / mod-dungeon-master | Hidden | Upstream calls itself early development |
| P2P flags (`p2p_hosting`, `torrent_*`, `magnet_links`, `mesh_relays`, `relay_volunteer`) | **All off** in `flags.json` | Built, but not proven with real swarms or open ports |
| Settings > Testing: torrents of game clients | Off (the default; the site can't turn it on) | Rule |
| Hordes of Azeroth, browser-game mod installs | Coming soon | Not built |
| Companion "creates the set" in game | Not shown | Phase 4 |

### 1.3 Go/no-go checklist (all must be yes)

1. `signin` log ends `FASTLOOP OK`, and the backup it took is listed in the log.
2. `audit` is at least 88/93, every red is known and none is new; `brand-check` is clean.
3. `den-test` passes on tonight's launcher and Den Lite.
4. `client-smoke` reaches the world on Wrath Unleashed, and its sign-in is taken out afterwards.
5. `check` is green: realm up, relay up, `/api/mesh/node` answers, memory headroom on the AX42.
6. Launcher CI green; `latest.json` points at tonight's version; an old launcher self-updates to it.
7. `flags.json` hash valid, with every P2P flag off.
8. The rollback path is known for each part:
   - the site: the `signin` backup;
   - the realm: the `realm` backup, with `scripts-reload` for Lua;
   - the launcher: the previous release;
   - game data: `gamedata-off`.
9. Nothing player-facing shows a brand word or a client version in prose (`audit` and `brand-check`).
10. The owner has played 10 minutes:
    - sign-in, then Wrath Unleashed to character select;
    - then a community realm and back;
    - then Your Den.

**No-go on any red in rows 1 to 5 of 1.1.** Rows 6 to 8 may slip; hide them instead.

---

## 2. Recommendations

### 2.1 What our one server serves

The box is one Hetzner AX42. Recommendation: **it serves the front door, not the world.**

| Serve | Why |
|---|---|
| Website, portal, forum, Hero Key login, admin | Our code and our database: small, and needed by everyone |
| Login server and the official realm (Wrath Unleashed) | The flagship. Players bring their own client |
| Our relay (seed node) and the mesh seed registry | Lets every community realm be reached; it carries about 2 000 players today (LEYLINE-MESH.md) |
| **A Dalaran-only browser slice** | One city to try the game in a tab. After that, the page asks for the player's own game folder (drag and drop, the own-files flow) or points them to get the client |
| The release mirror and the signed release log | Resilience (RESILIENCE.md) |

Do **not** serve:
- the full mini slice: today's budget is 7 424 MB;
- the Survivors slice (2.2);
- the 1.12 world;
- any mod bytes beyond the checked catalog.

How to shrink the browser slice to Dalaran:
- Add `stream/slices/dalaran.json`: the sampler less every tier after `dalaran`. Its tests copy `test_sampler.py`.
- Add a `slice-dalaran` step and a browser fence on the realm (the sampler's `ov_slices.lua` already fences).
- When a streamed player steps out of Dalaran, the page shows the existing "This realm needs your own game folder"
  card (GAME-DATA-ON-OUR-SERVER.md).
- Effort M. Recommended for Phase 1, after the owner says yes (decision D2).

### 2.2 Azeroth Survivors: run it locally, don't serve it

**Recommendation: stop planning to serve Survivors from our box. Make it a launcher mode that runs on the player's
PC from their own game folder.**

Reasons:
- It is single-player with no server (SURVIVORS.md), so serving it buys nothing.
- Serving it means hosting a recorded game-data slice on its own address. That is the very thing the owner wants
  off the box.
- The build is WebGPU-only, and its open rebuild is MIT or Apache-2.0, so we may ship the unmodified build.
- The launcher can serve the build on `127.0.0.1`, as Den Lite does.
- `/data/<name>` would read straight from the player's own archives (the launcher already reads MPQ for Den packs),
  so nothing is recorded and nothing is uploaded.

Plan:
- Add `survivors` to the launcher's local modes:
  - fetch the pinned build from our release (no game data in it, checked by the refusal list);
  - start a local file server;
  - open the system browser at `http://127.0.0.1:<port>`.
- Effort M.
- Keep `/wrath/survivors` as an info page with "Play with Leyline".
- The permission statement stays `TODO(owner)`; nothing goes live without it.
- Retire `sv-site` after that, with the owner's yes, since removal deletes things.

### 2.3 The queue (built in `agent/queue`; the design it should meet)

What other servers show us:
- Big launches gate on login queues.
- Population labels are relative, so a "Medium" realm can still have a queue ([Wowhead][wh-queue]).
- The lesson: **show the real number**: your place in line and the estimated wait, never a vague label.

The layers:
1. **The engine's own line.**
   - AzerothCore's `PlayerLimit` is "Maximum number of players in the world. Excluding Mods, GMs and Admins."
     Its default is 1000 ([worldserver.conf.dist][ac-conf]).
   - Players past it wait in the client's own queue screen.
   - `DisconnectToleranceInterval` (default 60 s) lets a dropped player skip the line on reconnect. Keep it, and
     consider 180 s.
   - Set `PlayerLimit` from the measured load. The RUNBOOK guidance:
     - keep the world update diff under 100 to 150 ms;
     - start at about 250 real players while bots run.
2. **Bots give way first.**
   - Random bots cost CPU per character.
   - When the line is above 0, step random bots down, 25 every 5 minutes, to a floor (say 40).
   - Step them back up when the line has been empty for 15 minutes. This is in `ov_` Lua plus a SOAP call.
3. **Leyline shows it.**
   - `status.json` carries `queue: {length, eta_s}`.
   - The launcher's realm row and play bar show "Queue: 12 (about 4 min)". The website shows the same.
4. **While you wait** (the queue agent's prompts):
   - "Play your Den or the Dojo while you wait" (one click; you keep your place).
   - "Join a **Seeded** server with the same rules now", listing seeds with room, by ping.
   - "Help the network grow: host a realm, seed this ruleset, or relay for others": the Host page, the seed flow
     (3.3), and "Help the network" (`relay_volunteer`).
   - Shown only while there is a line, at most once per session, and never as a nag.

### 2.4 Forum links to mods, downloaded in the launcher

Goal: helpful people drop a link in the forum, and players get the mod in the launcher with no friction. Two rules
hold that back:
- `magnet_links` today opens only info-hashes of packages already in our catalog (P2P-AND-TORRENT.md);
- found mods stay pending until the owner approves them.

The flow:
1. **The forum recognises a link.** A post holding a `magnet:` link, a `.torrent`, or a GitHub release link shows a
   card. W, effort S.
   - If the link is a catalog package (btih or URL match), the card says **"Install with Leyline"**
     (`leyline://install/<id>`, which already exists).
   - If not, the card says **"Suggest for Leyline"**.
2. **"Suggest for Leyline" files a candidate** in the mods pipeline (`addons-curate` / MODS-DAILY), with the
   poster's name and the post link. W plus tools, effort S. Nothing downloads for players yet.
3. **The curator fetches it on the PC, in a sandbox folder.** Tools, effort M.
   - librqbit is already in the launcher behind `torrent-engine`, and it supports magnets, DHT and selecting
     files by name ([rqbit][rqbit]).
   - Checks, in order:
     - `mods::safe_rel` paths;
     - the refusal list (no MPQ/BLP/M2/ADT/DBC, no client);
     - the size caps;
     - the `wu-mod.py` checker;
     - a malware scan with ClamAV (free) or Windows Defender's command line;
     - the sha256 is recorded.
4. **The owner approves** in the Admin Console. The entry gains `btih`, `sha256` and an HTTPS mirror on the catalog
   site. Only then can the launcher fetch it.
5. **Players install.**
   - From the forum card or the catalog, the launcher fetches by torrent when `torrent_downloads` is on, otherwise
     over HTTPS.
   - The sha256 is checked again, then the mod is installed.
   - Seeding back follows the existing caps.
   - Registering Leyline as the PC's `magnet:` handler is decision D5.
6. **Credit.** The catalog page says "Found by <forum name>". W, effort S.

WebTorrent is not the engine here: in the browser it reaches only WebRTC peers, never ordinary BitTorrent peers
([WebTorrent][webtorrent]). It is fine for a later "browser seeds Leyline's own files" idea, never for mods off DHT.

---

## 3. Feature catalogue

### 3.1 Game modes

| Feature | Pri | Eff | Depends on | Lives in |
|---|---|---|---|---|
| **Duels**: weekly season, Elo, anti-farm (built) | P0 | - | `OV_DUELS=1` | AC Lua, W |
| Duel arena "ring" in Dalaran: auto-reset, spectator ring, `/duel queue` matchmaking by rating | P1 | M | ov_duels | AC Lua |
| 1v1 arena ladder on the site (mod-1v1-arena is pinned) | P1 | S | arena export | AC, W |
| **Boss Fights leaderboards**: time-to-kill per boss, group size, ilvl bracket; weekly board | P0 (Phase 2) | M | encounter hooks (mod-ale) and an export like duels-export.py | AC Lua, W |
| **Boss Rush**: chained bosses with rising difficulty and leaderboards. mod-dungeon-master (pinned) already has a roguelike mode and leaderboards, but calls itself early development ([repo][dm]) | P1 | M | soak test on a Docker Den first | AC module |
| Keystone dungeons (mod-mythic-plus) and challenge modes: leaderboards on the site | P1 | S | export | AC, W |
| **Dungeon queue solo** (mod-solo-lfg: "use dungeon finder solo or in groups less than and up to 5 players" ([repo][sololfg])) with mod-autobalance scaling ([repo][autobal]) and companions filling roles | P0 (on) | - | playerbots | AC |
| Seeded-server mode tags ("Official ruleset", "Seeded") in Modes | P1 | S | 3.3 | W, L |
| Minigames (Bumper Balls, Coin Dash, the Mode editor): built | P0 | - | - | AC Lua |
| Weekly event rotation (hide and seek, minigolf night, a boss of the week) | P2 | S | ov_events | AC Lua, W |

### 3.2 Den and Dojo

**The engine fact that shapes this.** Den Lite is a small server for building. It has no combat, creature AI,
spells or dungeons. So the Dojo has two homes:
- **Den Lite** gives instant, offline things: build, fly, looks, models, paint.
- **An AzerothCore Dojo** has the real fight systems. It runs either as the Docker Den with a `dojo` profile, or as a
  private phased Dojo on the official realm. Its fights are real: dummies, duels, bosses, dungeons, companions.

The `agent/dojo` branch picks the first slice; this table is the whole menu.

| Feature | Pri | Eff | Depends on | Lives in |
|---|---|---|---|---|
| **Dojo entrance in the Den**: a gate in your Den, "Enter the Dojo"; one launcher card with Build / Train / Play | P0 (Ph 2) | S | Dojo profile | L, DL |
| **Training dummies**: plain, a tank dummy that hits back, a healer dummy that takes damage, a moving dummy; live DPS/HPS readout; reset | P0 | M | AC Lua (creature scripts) | AC |
| Dummy parse to the site: your best 60 s per spec | P1 | S | dummies | AC, W |
| **NPC area**: trainer, vendor (test gear sets only), barber and look changer, transmog, profession tables | P0 | S | den profile | AC |
| **Duel in the Den**: ring with reset (health, mana, cooldowns), invite a friend to duel | P0 | S | ov_duels reset code | AC |
| **Customize character**: looks, race and gender preview, gear set presets, saved "loadouts" | P1 | M | NPC area | AC, L |
| **Fly around**: flight in your Den (and Dojo), speed slider | P0 | S (DL: set-can-fly), S (AC: GM fly in the den profile) | - | DL, AC |
| **Spawn bosses**: a boss picker (by dungeon), level/scale slider, "reset fight" | P1 | M | mod-autobalance, creature templates the realm already has | AC |
| **Raid and dungeon with companions** in training mode: pick a dungeon, your companions fill tank/heal/dps, no loot, no lockout | P1 | L | playerbots, mod-solo-lfg, a Dojo instance flag | AC |
| **Queue for a dungeon from the Dojo**: the real Dungeon Finder, solo-LFG | P1 | S | solo-lfg (on) | AC, L |
| **Free mode** ("do anything"): GM-lite commands scoped to your Dojo: spawn, level, gear, teleport inside, time of day | P1 | M | Dojo instance flag | AC |
| **Scenario cards**: "Practice interrupts", "Tank a pack", "Heal through", with goals and a grade | P2 | M | dummies | AC Lua |
| **Combat log replay**: a timeline of your last pull with the damage breakdown, in the launcher | P2 | M | parse export | L, AC |
| **Easy object placement** with the new editor shell (DEN-OVERHAUL phases 1 to 4: gallery, click to place and select, popover, snap, undo) | P0 (Ph 1-2) | L | DEN-OVERHAUL | DL, AC addon |
| **Ground paint and textures** out of the box: Paint tool in the rail, hand-off to Forge at the spot, starter brushes; "make a texture" = recolour or tile our own open textures | P1 | M | Forge restyle, terrain delta | Forge, DL |
| **Your own 3D models** (.glb/.gltf/.obj): see the note below | P2 | L | model pipeline, P2P sharing | L, DL |
| **The companion creates sets**: say "Jaina, build a tavern"; she walks to the spot, casts (a visual only), pieces appear in a staged order with a 0.2 s fade-in each; progress lines | P2 | M | den_place_layout (built), companion (built), DL: spell-visual packets | DL, AC Lua, L |
| Dojo leaderboards (dummy DPS, Boss Rush, scenario grades) on the site | P2 | S | exports | W |

**Your own 3D models: what it takes.**
1. **Convert.** The 3.3.5a client draws M2 models (plus `.skin` and BLP textures), not glTF.
   - wow.export goes one way only, from game models to glTF ([deepwiki][wowexport]).
   - The path is a converter of our own, glTF to M2 v264 + skin + BLP, built on an open format library such as
     `wow-m2`. It is MIT/Apache, "parsing, validating, and converting", with WotLK "Full" support ([docs.rs][wowm2]).
   - Format libraries are fine; nothing from decompiled clients.
   - Blender-side tools for these formats are old and WMO-centred ([guide][wbs]).
2. **Deliver.**
   - Models load from MPQ patch archives, so **every viewer needs the file**. This is the same as mod-arac's
     `Patch-A.MPQ`, which each PC must have (server/RUNBOOK.md).
   - The launcher builds a per-Den patch MPQ (`patch-leyline-den.MPQ`) from the player's models only, and writes it
     into the player's game folder.
   - Visitors get the same models over P2P before they join.
3. **P2P rule.**
   - The refusal list today blocks `.m2`/`.blp`/`.mpq` by name and by bytes. It must stay that way for anything
     that came from the game.
   - The opening is a new transfer kind, `user_model`. It carries only the **source glTF/GLB plus a signed
     manifest**, never M2 or MPQ.
   - Each viewer's launcher converts it locally.
   - Caps: 5 MB and 20 k triangles each, 50 models per Den.
   - A model is reported and blocked through the Workshop rules.
   - Decision D7.

### 3.3 Seeded servers (official rulesets, run P2P by the community)

A **Seed** is a realm that runs one of our official rulesets, unchanged and verified, on a community host. Players
get "Official rules, community-run" and extra capacity. We keep one small always-on **anchor seed** per ruleset on
a slice of the AX42, or on a small extra Hetzner box only after the owner OKs the spend.

| Feature | Pri | Eff | Depends on | Lives in |
|---|---|---|---|---|
| **Ruleset packs**: main, brawl, vanillaplus, dalaran, den as signed packs (profile env, Lua, SQL, module pins) in the release log | P0 (Ph 3) | M | release log (built) | K, R |
| **"Seed this ruleset"**: one click in the launcher's Host page, which runs the host kit with the pack, no edits allowed | P0 | M | packs | L, K |
| **Ruleset attestation**: the host agent signs `{pack hash, core commit, module pins, config hash}` every 10 min; the relay checks it against the log; a mismatch drops the Seed badge (it stays a Community realm) | P0 | M | mesh records | K, R |
| **Seed badge and listing**: "Seeded · Official rules" row, ping, room, uptime score; Official tab lists anchor plus seeds | P0 | S | attestation | W, L, R |
| **Health and uptime score**: relay probes, crash count, update diff; low scores sink in the list | P1 | S | node registry (built) | R |
| **Anchor seed on our box**: a capped container (CPU 1, 1.5 GB, 40 players) that keeps each ruleset reachable | P1 | S | packs | server |
| **Overflow routing**: when the official realm queues, offer seeds with room (2.3) | P1 | S | queue | L, W |
| **Seasons**: seeds can run "fresh start" seasons with a common start date and a site leaderboard across seeds | P2 | M | exports | W, AC |
| **Characters stay on their seed** (no transfer in v1). Later: an opt-in character copy between attested seeds of the same ruleset, signed by both | P2 | L | attestation, consent | K, R |
| **Host safety**: backups to the host's disk, auto-restart, staff block feed, report a seed, "Stop every node" (built) | P0 | S | node registry | K, R |
| **Host credit**: "Seeded by <name>", a host profile badge, host leaderboard by player-hours | P1 | S | profiles | W |
| **Seed retirement**: the cutover runbook from VANILLA-REMASTERED.md (announce, freeze, export, hand over) as a launcher flow | P2 | M | - | L, K |
| **Small slice for hosts without a PC**: documented guide to a small rented box with the kit (no links to paid services from us; the owner decides) | P2 | S | - | docs |

### 3.4 Vanilla mode

Options, in the order recommended:
1. **"Vanilla+ on Wrath" now, through the `vanillaplus` profile.**
   - It already exists: the first era's level cap, faster pace, queue announcers, played with the 3.3.5a client.
   - Zero extra data, and it fits the Seeds plan, so it is the first Vanilla seed.
   - mod-individual-progression (pinned) can add era progression.
2. **Vanilla Remastered (1.12.1 client) as a Seed.**
   - The `cmangos-1.12.1` host-kit profile builds it from public source, with data from the host's own client
     (VANILLA-REMASTERED.md, "A community host takes it over").
   - We relaunch it ourselves only if the owner wants it: `ov112-on`, then `vr-beacon`.
   - **Plan, don't launch.**
   - "Remastered" here means the looks that come from mods: HD packs and Performance/High looks (pending approval),
     the openvanilla tweaks as a ruleset pack, and the OG UI.
3. **Link and encourage.** A "Vanilla" tab in Modes that lists community 1.12 realms on the network, plus a forum
   board for vanilla hosts.

| Feature | Pri | Eff | Depends on | Lives in |
|---|---|---|---|---|
| Vanilla+ ruleset pack and its anchor seed | P1 | S | 3.3 packs | K, AC |
| Vanilla tab in Modes (1.12 realms plus Vanilla+ seeds) | P1 | S | - | W, L |
| Remastered look bundle (approved HD/looks mods as one install) | P2 | S | owner approvals | L |
| Vanilla Remastered 1.12 as a community Seed (cutover plan) | P2 | M | consent on accounts (VR doc) | K, R |
| Browser levels 1-20 slice for 1.12 | Hold | - | the "serve only Dalaran" rule (2.1) says no | - |

### 3.5 Mods and P2P

| Feature | Pri | Eff | Depends on | Lives in |
|---|---|---|---|---|
| Forum link to "Install / Suggest for Leyline" (2.4) | P1 | S | - | W |
| Curator sandbox fetch with malware scan | P1 | M | librqbit (in), ClamAV | tools |
| Catalog `btih` plus HTTPS mirror for approved mods; `torrent_downloads` on | P1 | S | real-swarm test | L, W |
| `librqbit` as the `Swarm` for Leyline's own packages (today only for the testing switch) | P1 | M | flags | L |
| `share_own_files` on (players seed launcher releases, Host packs) | P2 | S | release key made | L |
| `user_model` transfer kind (glTF only, converted locally) | P2 | L | 3.2 | L |
| `magnet:` OS handler | P2 | S | D5 | L |
| Mods daily, 10 a day, all pending | P0 (running) | - | owner approvals | tools |

### 3.6 Community

| Feature | Pri | Eff | Depends on | Lives in |
|---|---|---|---|---|
| "Become a node / host / seed" prompts (queue, Host page, after 10 h played) | P0 (Ph 1) | S | `agent/queue` | L, W |
| `mesh_realms` on (the realm list from the mesh, falls back safely) | P1 | S | - | R, W |
| `relay_volunteer` on, then `mesh_relays` after the sealed path or the privacy notice | P1/P2 | S/L | LEYLINE-MESH decisions | R, L |
| Host and creator profiles with credit (found mods, seeds, Dens) | P1 | S | profiles | W |
| Den showcase: best Dens weekly on the hub, "visit this Den" | P2 | M | Den invites | W, L |
| Toolkit and succession (anyone can run Leyline on): docs/TOOLKIT.md | P1 | - | `agent/toolkit` | docs |
| Discord TODO(owner) lines, forum boards: Seeds, Vanilla, Dojo | P1 | S | owner copy | W |

---

## 4. Phases

| Phase | When | Goal | Contents | Exit criteria |
|---|---|---|---|---|
| **0: Launch** | Tonight | The minimal set live and stable | 1.1 rows 1 to 7 (8 if ready); everything in 1.2 hidden | Every go/no-go item yes. 24 h with no rollback; no new red in `audit` the next morning |
| **1: Hold the line** | Week 1 | Survive the first crowd; shrink what we serve | Queue live (2.3) with bots giving way; become-a-node prompts; Dalaran-only slice (D2); first-run test (`first-run-test` step, HANDOFF item 2); Chumpton history wipe; Survivors local mode built behind Preview (D1); DEN-OVERHAUL phase 1 shell; UX fixes (empty banner, Settings default tab, Installed tab) | A queue of at least 20 handled with a correct ETA; the server box serves at most the Dalaran slice of game data; the fresh-machine test green in CI and on the PC; `den-test` green on the new shell |
| **2: Dojo and leaderboards** | Weeks 2-3 | A reason to stay | Dojo v1 on AzerothCore (dummies, NPC area, duel ring, fly); Boss Fights leaderboards; arena and keystone boards; the forum Suggest flow; DEN-OVERHAUL phases 2 and 3 (gallery, click to place) | A new player goes from the Den to the Dojo, hits a dummy, and sees their number on the site in under 5 minutes; `den-test` plus a new `dojo-test` step green |
| **3: Seeds alpha** | Weeks 3-5 | Capacity grows without us | Ruleset packs, attestation, Seed badge, anchor seeds (main, vanillaplus), overflow routing, `mesh_realms` on, `relay_volunteer` on, `torrent_downloads` for approved mods | 3 community seeds attested for 7 days; a full official realm sends players to a seed and they reach the world; a tampered config loses its badge in tests |
| **4: Make it yours** | Weeks 5-8 | Creation tools | Boss spawns and companion dungeons in the Dojo; free mode; ground paint and textures; the companion builds sets with the "casting" staging; Vanilla tab, the Vanilla+ seed, the Remastered look bundle; Boss Rush if stable | The owner builds a scene by voice through the companion; a Vanilla+ seed has 25 or more players at peak |
| **5: Hand it to the network** | After week 8 | Leyline runs without our box being special | `user_model` glTF pipeline and P2P sharing; `mesh_relays` (sealed path); `share_own_files`; Vanilla Remastered as a community Seed (if D6); seasons across seeds; succession per TOOLKIT.md | With our box off for one hour (a drill), players still find realms, update the launcher and join seeds |

---

## 5. Decisions the owner must make

| # | Decision | Recommendation |
|---|---|---|
| **D1** | Survivors: stop serving it, make it a local launcher mode from the player's own game folder? Then retire `sv-site` (it deletes things) | **Yes, local only** |
| **D2** | The browser serves **Dalaran only** (new `dalaran.json` slice), with the own-files card after that, replacing the sampler and mini slices | **Yes**, in Phase 1 |
| **D3** | The queue numbers: `PlayerLimit` (start at 250 real players), the bot floor while a line exists (40), the reconnect grace (180 s) | As written, then tune from `check` |
| **D4** | Seeded servers: may community hosts run official rulesets under a "Seeded · Official rules" badge, and do we run an anchor seed on the AX42? A second box would cost money, so ask first | **Yes**, anchor on the AX42 first |
| **D5** | Forum mod links: Suggest, then curator scan, then your approval, then the catalog (never a direct install of an unlisted link)? And may Leyline register as the PC's `magnet:` handler? | **Yes** to the flow; the handler **later** (Phase 3) |
| D6 | Vanilla: Vanilla+ seed first; Vanilla Remastered 1.12 relaunch, or only as a community Seed? Account moves need player consent | Vanilla+ first; VR as a community Seed |
| D7 | Player-made 3D models: allow glTF uploads converted on each PC, with caps and reporting? | Yes, in Phase 5 |
| D8 | The Dojo's home: Docker Den `dojo` profile, a private phase on the official realm, or both? | Both: the phase on the official realm for everyone, Docker for friends' Dojos |
| D9 | Boss Rush: run mod-dungeon-master (early-development upstream) on the official realm, or seeds only? | Seeds and the Dojo first |
| D10 | Names and copy: "Dojo", "Seed", "Azeroth Survivors"; the companion's in-game name stays hers, while the launcher keeps "your companion" | TODO(owner) |

---

## Sources (read 3 Nov 2026)

- [ac-conf]: AzerothCore `worldserver.conf.dist` (`PlayerLimit`, `DisconnectToleranceInterval`, `Network.Threads`),
  https://raw.githubusercontent.com/azerothcore/azerothcore-wotlk/master/src/server/apps/worldserver/worldserver.conf.dist
- [wh-queue]: Wowhead, why medium-population realms have login queues,
  https://www.wowhead.com/news=294750/blizzard-explains-why-medium-population-classic-servers-have-login-queues
- [sololfg]: mod-solo-lfg, https://github.com/azerothcore/mod-solo-lfg
- [autobal]: mod-autobalance, https://github.com/azerothcore/mod-autobalance
- [dm]: mod-dungeon-master, https://github.com/trauntrow/mod-dungeon-master
- mod-playerbots ("capable of running most raids and battlegrounds"), https://github.com/mod-playerbots/mod-playerbots
- NPCBots (an AzerothCore core fork: we stay on playerbots), https://github.com/trickerer/AzerothCore-wotlk-with-NPCBots
- [rqbit]: rqbit / librqbit (magnets, DHT, selective download, library use), https://github.com/ikatson/rqbit
- [webtorrent]: WebTorrent (browser = WebRTC peers only), https://github.com/webtorrent/webtorrent
- [wowm2]: wow-m2 crate (MIT/Apache, WotLK read/write/convert), https://docs.rs/crate/wow-m2/latest
- [wowexport]: wow.export glTF export (one-way), https://deepwiki.com/Kruithne/wow.export/7.2-gltf-export
- [wbs]: Blender WMO addon guide (old; M2 export not covered), https://elijahketchersid.com/forums/thread-126.html

[ac-conf]: https://raw.githubusercontent.com/azerothcore/azerothcore-wotlk/master/src/server/apps/worldserver/worldserver.conf.dist
[wh-queue]: https://www.wowhead.com/news=294750/blizzard-explains-why-medium-population-classic-servers-have-login-queues
[sololfg]: https://github.com/azerothcore/mod-solo-lfg
[autobal]: https://github.com/azerothcore/mod-autobalance
[dm]: https://github.com/trauntrow/mod-dungeon-master
[rqbit]: https://github.com/ikatson/rqbit
[webtorrent]: https://github.com/webtorrent/webtorrent
[wowm2]: https://docs.rs/crate/wow-m2/latest
[wowexport]: https://deepwiki.com/Kruithne/wow.export/7.2-gltf-export
[wbs]: https://elijahketchersid.com/forums/thread-126.html
