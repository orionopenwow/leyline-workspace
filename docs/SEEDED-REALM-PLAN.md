# The seeded vanilla realm: a plan

Written 3 Nov 2026 for the owner's ask:

> "research and plan the 'Turtle [...] like' minimum slice seeded realm people can help join as nodes to help run
> through a relay that's essentially self sustaining and no single place at a time, something we can launch with,
> the Open Vanilla tuning and features and addons; tuck this into the future plan before release."

This is a plan. Nothing here is built. It follows the owner's earlier answers:
- **D4:** no new paid server. A lightweight relay on our box keeps the seed alive. No copy of the realm runs there.
- Magnet links in the forum are OK. The browser serves Dalaran, the starting zones and dungeons. The queue is as
  written. Survivors stays, with an off switch.

Internal doc. The name "Turtle" is used here only to describe the idea. It never goes on a page, a realm name or a
post (see section 6).

---

## 0. The recommendation in five lines

1. **Launch a 1.12 "Open Vanilla" Seed on CMaNGOS Classic.** The host kit's `cmangos-1.12.1` profile already
   builds it. Tuning is server-side only: rates, the level cap and save timing. Players use the stock client, their
   own copy, plus approved addons.
2. **The launch slice is levels 1 to 20.** That means the starting zones, the capitals, the 10 to 20 zones and three
   early dungeons, the same set as our 1.12 browser slice. The level cap rises in content phases after launch.
3. **One host at a time, never one place.** One elected host runs the world. Two or more trusted **Keepers** hold
   a live copy of its database. Our relay is a vote (a "witness"), not a copy. A host can hand over to a Keeper in
   minutes, planned or after a crash.
4. **The relay decides who the host is by a signed lease, not by who dialled last.** Every host change has a
   higher number (an epoch). Relays refuse an old host, so two hosts never run at once.
5. **Ship "Seed v0" before release as a Preview.** v0 has one host, one or two Keepers, log shipping every 30 s,
   and a one-click takeover with drills. Automatic failover, the Lua features and higher levels come after release.

---

## 1. What we launch with (the minimum slice)

### 1.1 The world

| | At launch | Why |
|---|---|---|
| **Core** | CMaNGOS Classic at the pinned commit in `hosting/kit/profiles/cmangos-1.12.1/profile.lock` (GPL-2.0, built from source on each node) | It is the owner's 1.12 heritage, and the profile, Hero Key login and relay link already work for it (`wuhost/coredb.py` `CmangosDB`). It is light: about 600 to 900 MB for the world server with no bots, grids unloading and no mmaps (VANILLA-REMASTERED.md, measured budget) |
| **World database** | CMaNGOS `classic-db` at the pinned commit (community SQL), plus **our own** tuning SQL | No game data. Each node's map data (dbc, maps, vmaps) is extracted from **that node's own client** by `extract.sh` |
| **Level cap** | **20** (`MaxPlayerLevel = 20`) | A small world to test, tune and police. Few grids loaded, so less memory. It matches the 1.12 browser slice |
| **Zones that matter** | Every starting zone and capital: Elwynn, Stormwind, Dun Morogh, Ironforge, Teldrassil, Darnassus, Durotar, Orgrimmar, Mulgore, Thunder Bluff, Tirisfal, Undercity. Then Westfall, Loch Modan, Redridge, Darkshore, the Barrens and Silverpine, plus the tram | From `hosting/vanilla-remastered/slice-112.json`, the tiers the owner already chose for "1-20 in the OG style" |
| **Dungeons** | Ragefire Chasm, Wailing Caverns, the Deadmines | The same tiers. Shadowfang Keep and Blackfathom Deeps come with the cap of 25 or 30 |
| **Rest of the world** | Open, no fence. A player over the cap's zones is simply over-levelled for nothing | Without Lua (1.4) there is no cheap fence, and we don't need one: every desktop player has the whole client |
| **Population** | `PlayerLimit = 100` at first (the CMaNGOS default), raised from measured load | Home PCs. The queue (QUEUE.md) shows the line |
| **Bots** | None at launch | They cost CPU and they write state, which makes failover heavier. Maybe later (CMaNGOS Playerbots, `-DBUILD_PLAYERBOTS`) |

### 1.2 The Open Vanilla tuning (server-side only)

This is what the old Open Vanilla realm and `vr-data.sh` set, minus anything that needs a changed client:

| Setting | Value | Note |
|---|---|---|
| XP rates (`Rate.XP.Kill`, `.Quest`, `.Explore`) | 1.5x | The Classic+ "pace", the same as our `vanillaplus` profile |
| Rested XP | 2x | As `vanillaplus` |
| `GridUnload` | 1 | The CMaNGOS default ("Unload grids"). It keeps memory low |
| `mmap.enabled` | 0 at launch | Building the monster paths takes hours on a home PC (`LL_EXTRACT_TIME`). Turn it on in phase 2 when Keepers can build them overnight |
| `PlayerSave.Interval` | **60 000 ms** (down from the default of 900 000 = 15 min) | **This, more than the database copy, bounds what a crash loses** (section 2.5) |
| `SaveRespawnTimeImmediately` | 1 (default) | Respawn timers survive a handoff |
| `WrongPass.MaxCount` | 0 | Every relay player comes from the agent's address (VANILLA-REMASTERED.md); the relay counts per player itself |
| Console, RA, SOAP | off | As the profile already sets |
| Hero Key login | on (`setup.py herokey on`) | Players log in with their Hero Key. The realm gets an `HK...` account with a short-lived verifier, so **no real passwords are on any node's disk** |

**Not at launch: the Classic+ tables and `patch-Z.MPQ`.** The old realm laid our own changed
`SkillRaceClassInfo` / `CreatureModelData` / `CreatureDisplayInfo` tables over the stock ones, with a matching
client archive (`vr-data.sh`, phase `classicplus`). Those files are **built from game files**. Under the rules they
may never travel P2P or be served. If the owner wants them back (decision S3), the only allowed path is a **recipe**,
the way Den packs work (`wuhost/packs.py`: "Leyline ships recipes only ... the player's own archives fill them"):
- a list of field changes;
- applied on each node to its own extracted tables;
- and on each player's PC to their own client.

It also carries legal weight: see section 6.

**Check first (open since VANILLA-REMASTERED.md):** was the old `openvanilla-mangos` image stock CMaNGOS, or our
fork with code changes? If it is a fork, the GPL source must be pinned and published before any node runs it. If
nobody knows, we launch on stock CMaNGOS with our tuning, which this plan assumes.

### 1.3 Addons (all stay **pending** until the owner approves them)

These are already in `tools/hub-index/seeds.json` for 1.12, all on their authors' GitHub. A "Seed starter set" is
a loadout of approved ones:

| Addon | What it gives | Pick for the starter set? |
|---|---|---|
| pfQuest | Quest helper with a map database | yes: the biggest help for new players |
| pfUI | A whole modern interface | optional: big, players choose |
| ShaguTweaks | Small comforts without replacing the interface | yes |
| ShaguPlates, ShaguDPS | Nameplates, a damage meter | yes |
| ZonesLevel, MapTarget | Zone levels on the map | yes |
| Bagzen, ChatSuey, SuperIgnore | Bags, chat, ignore list | optional |
| Immersive Dialog / DialogUI | Quest dialogue look | optional |

Each needs its licence and art checked (the MODS-DAILY rules) before approval. The **TurtleHD Patch** and **Project
Reforged** link-only packs (`external-packs.json`, both still `skip`/pending) are client-content packs made for the
Turtle client. **Recommendation: keep both off for this realm.** They are not addons, and they tie us to that
project (section 6).

The looks presets (`stream/looks.json`: Max, Balanced, Classic, Performance, High, all `builds: 1.12`) are our own
settings changes and work as they are.

### 1.4 Features: launch vs later

Our `ov_*.lua` realm scripts run on **mod-ale (AzerothCore only)**. The pinned CMaNGOS build has no Lua engine.
Eluna supports CMaNGOS Classic and VMaNGOS, but through separate pre-merged forks
([Eluna development guide][eluna]). So:

| Feature | Launch | After launch |
|---|---|---|
| Hero Key login, realm list, join, invite links | **yes** (built) | |
| House rules on the realm list (`rules.txt` style) | **yes** (built) | |
| The queue line and "while you wait" | **yes** (built for the site; `PlayerLimit` on the realm) | |
| "Seeded · community-run" badge, Keepers and uptime on the realm row | **yes** (new, small) | |
| Approved 1.12 addons as one-click installs | **yes** (once approved) | |
| Armory and leaderboards (read-only from the realm DB, as the old Open Vanilla jobs did) | | phase 2: the host's agent exports a JSON, the site reads it |
| QoL from `ov_qol.lua` (free respec, `.xp off`, hearth reset, instant spell learn) | | phase 3: needs an Eluna-enabled CMaNGOS pin, then ported scripts |
| Challenge modes (hardcore, slow XP, "common gear only"): the most loved Turtle ideas that need **no client change** | | phase 3, Eluna |
| Duels season (`ov_duels.lua`) | | phase 3, Eluna |
| Level cap 25, 30, 40, then 60 ("content phases", like VMaNGOS's own patch progression from 1.2 to 1.12 ([vmangos][vmangos])) | | each a ruleset pack bump, announced |
| Browser play for this realm | | **owner decision S4**: our box serves only Dalaran + starters + dungeons (D2), and that slice is for the main realm |
| New races, new zones, new models, transmog windows | **never in this plan** | They need a changed client (section 6) |

### 1.5 Is it "Turtle-like"? What we copy, and what we don't

What made that server popular, per the press:
- it **rebalanced weak specs**;
- it added dungeons, races and story;
- it finished half-made zones;
- its loot felt "rewarding and unique" ([Windows Central][wc]);
- optional levelling **challenges** with titles: slow XP, "vagrant" gear limits, boar-only ([Massively OP][mop]);
- hands-on GMs against bots and "community polling" ([Tales by Travel][tbt]).

| Their idea | Ours |
|---|---|
| Classic pace with modern comforts | 1.5x pace, QoL commands (after Eluna), approved addons |
| Challenge modes | Yes, server-side (phase 3) |
| Spec rebalancing | Possible server-side (our own spell SQL); the owner's call per change |
| Community polls, active GMs | The forum, Discord `realm-seeds`, staff blocks on the mesh |
| New races, zones, models, a custom client and launcher patch | **No.** That is the line that cost them the most (section 6) |
| Donations for in-game rewards | **Never** (our money rule) |

---

## 2. Architecture: no single place at a time

### 2.1 The roles

```
             relays (ours = seed relay + witness; any node can be one)       <- players connect here
                 |  tunnel (wss, outbound from the host)                    \
            [HOST]  world server + login server + MariaDB primary   -- lease, epoch N
                 |  binlog chunks (signed, encrypted) via the relays
        [KEEPER A]  MariaDB replica, server built, data extracted   (warm standby)
        [KEEPER B]  same                                            (warm standby)
        archivers   anyone: keep the encrypted, signed chunks; can't read them
```

| Role | Who | Runs | Holds |
|---|---|---|---|
| **Host** | one Keeper at a time | `realmd`, `mangosd`, MariaDB primary, the agent | the live world |
| **Keeper** | 2 to 4 trusted people, owner-approved, each with a Hero Key and a node key | the same images, built; MariaDB as a replica; world server **stopped** | a database seconds behind, its own extracted data |
| **Witness** | our relay (`RELAY_SEED_WITNESS=1`), and later any steward's relay | a vote in host elections. **No database, no world** | the latest lease and epoch only |
| **Relay** | ours, plus volunteers (be-a-node) | `wurelay` | nothing of the realm; it forwards traffic |
| **Archiver** (optional) | anyone with disk | stores chunks | ciphertext it cannot read |

This fits D4. Our box runs only the relay and the witness vote, a few KB of state. No world, no database copy.

### 2.2 Who holds the truth at any moment: the lease

- The realm has a **seed manifest**, signed by the realm key. It lists:
  - the ruleset pack hash, the core commit and the client build (5875);
  - the Keepers' node keys and the voters (Keepers + witness);
  - the relays.
- The host holds a **lease**: `{realm id, epoch N, host node id, expires (now + 30 s), last GTID}`.
- A lease counts only when a **majority of voters** signed it. With 2 Keepers + witness there are 3 voters, so 2
  must sign. The host's own signature counts.
- The host renews every 10 s.
- **If the host can't renew for 30 s, it stops its own world server** (it fences itself). That is the rule that
  makes split brain impossible, as long as clocks are within a few seconds. This is the usual lease-plus-fencing
  pattern: each leader action carries the term, and "resource servers reject any request from a node claiming
  leadership with a term lower than the highest term they have seen" ([leader election][fence]).
- **Relays check it.** Today a new agent with the same realm key simply replaces the old one
  (`relay.py agent_session`: "replaced by a new connection"). The change:
  - the hello carries the lease;
  - the relay accepts it only if the lease is valid and the epoch is ≥ the highest it has seen for that realm;
  - a higher epoch replaces the old session at once.
- Relays gossip the highest epoch per realm over the mesh. A relay that missed an election still refuses the old host.

Why not a database cluster (Galera, several primaries)? There, the round trip between the farthest nodes "will set
the baseline for transaction commit latency", and it needs "an odd number of nodes and an odd number of locations"
([Galera][galera]). Home PCs on home links would make every save slow. One primary plus replicas is the right shape.

### 2.3 How players reach the current host

- The realm id comes from the realm key, and relays give ports per realm id. So **the address and ports players use
  don't change** when the host changes. The new host dials the same relays.
- Players see a disconnect, then log in again to the same realm. The launcher's play bar shows "Moving to a new
  host… about 2 min" while the realm row says `handoff`.
- **Several relays:**
  - the manifest names 2 or more (ours first);
  - a host keeps one tunnel at v0. Multi-relay agents are designed in LEYLINE-MESH.md and arrive in phase 2;
  - if our relay is down, the host dials the next one on the list, and launchers find it through the mesh (signed
    realm records, the seeds list in the launcher, the peers cache).

### 2.4 Handing over

**Planned** (the host wants to go to bed):
1. The host runs `leyline seed handoff --to <keeper>` (a button on the Host page).
2. The world announces a 5-minute warning.
3. The host saves every player, stops `mangosd`, and stops writes.
4. It ships the last chunk, waits until the Keeper's replica has the same GTID, then releases the lease.
5. The Keeper takes lease N+1 (signed by itself and the witness), promotes its replica, starts `realmd` and
   `mangosd`, and dials the relays.

**Loss: zero.** Downtime is the world server's start, a few minutes (uncertain: CMaNGOS Classic without mmaps;
measure in drill 1).

**On a crash:**
1. The lease runs out (30 s).
2. **v0:** a Keeper presses **Take over** in the launcher (or Control's `seed-takeover`). It works only if the
   witness signs epoch N+1, so two Keepers can't both win: the witness signs one N+1 only.
3. **v1:** the Keepers elect by themselves. A Keeper that has the highest GTID and sees the lease expired asks for
   votes. A Raft-style rule: one vote per epoch, and a voter refuses a candidate behind its own last known GTID.
4. The new host promotes and starts as above. The old host, when it comes back, sees epoch N+1. It **must not**
   start its world. It rejoins as a replica from the new host's snapshot.

MariaDB's own docs warn that promoting the logically-behind server after a crash can lose data, and that a crashed
primary must come back as a replica ([MariaDB semisync][semisync]). The GTID rule and the rejoin rule handle this.

### 2.5 How much a crash loses, and how to bound it

There are two separate losses:

| Loss | Cause | Bound at v0 | Bound at v1 |
|---|---|---|---|
| **In-memory, never saved** | The world server keeps players in memory and saves them every `PlayerSave.Interval` (CMaNGOS default: 15 minutes) | **60 s** (we set it) | 60 s, or 30 s if the database copes |
| **Saved, not yet copied** | Writes that reached the host's DB but no Keeper | **≤ 30 s**: binlog chunks are shipped every 30 s or every 1 MB | **about 0**: live **semisynchronous** replication with `AFTER_SYNC`, where a commit waits for one replica. MariaDB says failover "should be lossless" then ([semisync][semisync]) |
| A Keeper is far behind | The link was down | The election refuses a stale Keeper. If only stale Keepers are left, a person decides (stay down, or accept the loss) | same |

**So a crash loses at most about 90 s of play at v0, and about 60 s at v1.** Planned handoffs lose nothing.

Caveats:
- Semisync falls back to async after `rpl_semi_sync_master_timeout` (10 s by default) when no replica answers. The
  bound then grows to the chunk interval. Show it in `seed-check`.
- Trades or loot in the last minute can be lost or doubled. A doubled item is a known risk of any rollback. Keep the
  window small. Phase 2 adds a post-failover audit report (items created in the replayed minutes).

### 2.6 How the copies move (and what never moves)

- **The database only:** the world DB, the characters DB and the realm's account DB.
- **Snapshots:** a hot physical backup (`mariadb-backup`) or a dump, then binlog chunks from the recorded position.
  This is standard point-in-time recovery: restore the backup, then replay the binary log "to a specific point in
  time" ([MariaDB PITR][pitr]).
- **Signed:** each chunk carries `{realm id, epoch, seq, GTID range, sha256, prev sha256}`, signed by the host's
  node key. That is a hash chain, so a missing or edited chunk is seen.
- **Encrypted:** the encryption is to the Keepers' keys (X25519). Relays and archivers carry ciphertext.
- **Content-addressed:** a chunk is named by its hash, the same idea as IPFS (content "referenced through a unique
  value" from its hash ([Filebase][ipfs])). Any relay or archiver can serve it, and the receiver checks it. The
  mesh's existing `transfer.rs`/checker rules stay: only open content and this new `seed-chunk` kind. The kind's
  checker refuses anything with game-file magic bytes.
- **At v0, chunks go through the relay as a small mailbox** (`POST /api/seed/<realm>/chunk`, kept 24 h, capped at
  e.g. 200 MB per realm). That works because Keepers, like hosts, are behind NAT. **v1** carries live replication
  over a tunnel between two agents through the relay, with TLS between the MariaDB servers pinned to node keys.
- **Never moved:** the client, the extracted map data, or any MPQ/DBC. Each node runs `extract.sh` on its own client.
  The attestation (below) compares **hashes** of the extracted tables, never the files.

Sizes, to check in drill 1 (estimates):
- the `classic-db` world DB is a few hundred MB once;
- the characters and accounts are small for a few hundred players (tens of MB);
- binlog traffic is about a few KB/s per active player at a 60 s save.

### 2.7 How nodes join

1. A player opens **Host > Network > Keep a Seed** in the launcher. It requires the Hero Key, the Host pack and
   Docker (the same as hosting today; always-on.md: 8 GB RAM is enough).
2. The kit builds the `cmangos-1.12.1` images and runs `extract.sh` on **their own** 1.12 client (40 to 90 min,
   once).
3. **Attestation:** the node signs `{pack hash, core commit, image digest, extracted-table hashes}`. The relay
   compares it with the manifest. That is the ROADMAP 3.3 attestation, reused.
4. The node becomes a **Candidate**. It can relay and archive, but can't vote or host.
5. **Keeper** status is granted by the owner or a steward in HQ: a signed manifest update. Suggested bar: 14 days as
   a candidate with 95 % uptime, and one passed drill.
6. Everyone else can help without trust: run a relay (be-a-node), or archive chunks.

### 2.8 Trust and cheating, honestly

- **The host can cheat.** Whoever runs the world server can edit any row: gold, items, levels. No signature scheme
  stops that on a single-host server. So:
  - only **Keepers** (a few named, trusted people) ever host;
  - the chunk chain is an audit trail. Every Keeper holds every change, signed by the host that made it. A bad
    edit is found, rolled back with point-in-time recovery, and that Keeper is removed from the manifest, plus
    blocked on the mesh with a staff block;
  - phase 2 adds an "odd rows" report on the Keepers' side: GM-level changes, items with no loot source, gold jumps.
- **Relays can read game traffic** (LEYLINE-MESH.md section 9). That stays true until the sealed path is built.
  For this realm, players use our relay and Keeper relays only, the same policy as `mesh_relays` off.
- **Players' privacy:**
  - nodes hold character data and `HK...` accounts (no real passwords; the verifier is short-lived);
  - the account table's last-IP is the agent's address, because everyone comes through the relay;
  - Keepers agree to a short "you hold other players' character data; don't share it" note (`TODO(owner)` words).
- **Sybil and spam:** candidates are free, but Keepers are hand-picked, so floods don't matter for hosting.

### 2.9 Not one place: the relay and the witness

- Several relays already gossip (the mesh). Any Keeper can also run a relay. The manifest lists 2 or more.
- The witness is one of three votes. If our box is down, host + Keeper still form a majority, so **the realm keeps
  running and can still fail over**.
- Phase 2 adds a second witness on a steward's relay, giving 5 voters: 3 Keepers + 2 witnesses.
- Launchers find the realm through the mesh, the seed list and the peers cache. LEYLINE-MESH section 2 already works
  with our seeds down.
- **What still depends on our box:**
  - the Hero Key site that signs log-in assertions. Players who already have an `HK...` account keep logging in
    with it until its verifier expires. Fix: SUCCESSION.md 2.8 (identity outlives one server);
  - the realm key, which signs the manifest. Keep it offline with the owner, plus a sealed copy with a steward
    (SUCCESSION.md).

### 2.10 Costs

- **Zero new paid servers.**
- Our box adds the witness vote and the chunk mailbox: a few MB of RAM and up to about 1 GB of disk at the caps
  above.
- Each Keeper's PC:
  - while hosting: about 1 to 1.5 GB of RAM (world plus database), 2 cores, about 15 GB of disk with the client and
    its build;
  - as a standby: about 0.5 GB (the replica only);
  - upload: tens of KB/s.
- Seeding is opt-in, uses the same "only while idle and plugged in" switch as be-a-node, and is never a nag.

---

## 3. Phased plan

Sizes: **S** is a day or less, **M** 2 to 5 days, **L** a week or more of agent work. The owner's own test time is
listed separately.

### Phase 0: before release, "Seed v0 (Preview)", about 3 weeks of agent work

| # | Milestone | Eff | Lives in | Test / Control step |
|---|---|---|---|---|
| 0.1 | **Ruleset pack `openvanilla-seed`** for `cmangos-1.12.1`: the conf overlay from 1.2 (cap 20, rates, save 60 s, PlayerLimit 100, no mmaps), house rules, our tuning SQL (empty at first) | S | K | `hosting/tests/test_seed_pack.py` (keys allowed, no game-file names, conf values) |
| 0.2 | **Hero Key on for cmangos** in that pack; the `hktest` probe against a 1.12 realm | S | K | extend `test_herokey_sso.py` |
| 0.3 | **Seed manifest + lease format** in `wunet` (Python) with test vectors | M | K, R | `test_seed_lease.py`: majority, expiry, epoch order, the witness signs one N+1 only, clock skew ±5 s |
| 0.4 | **Relay gate**: hello carries the lease; refuse stale epochs; gossip the highest epoch; `RELAY_SEED_WITNESS=1` vote endpoint | M | R | `test_seed_relay.py` (two agents, same realm, old one refused); the `relay` step deploys it with a self-check |
| 0.5 | **Log shipping**: binlog + GTID on in the profile's MariaDB; the agent ships signed, encrypted chunks every 30 s to the relay mailbox; the Keeper applies them | M | K, R | `test_seed_logship.py` (chain breaks found, replay to the same GTID, refuses foreign magic bytes) |
| 0.6 | **Planned handoff and one-click takeover** (`leyline seed handoff`, `leyline seed takeover`, Host page buttons); the old host's self-fence; the old host rejoins as a replica | M | K, L | `test_seed_handoff_e2e.py`: in-process relay + 2 fake agents + 2 MariaDB containers where Docker exists, else fakes |
| 0.7 | **Seed badge and status**: realm row "Seeded · community-run", host name, Keepers online, replication lag; the play bar "Moving to a new host" | S | W, L | Playwright `realms` + a new `seed.browser.test.cjs` |
| 0.8 | **Control steps** | S | tools | `seed-check` (read-only: lease, epoch, host, lag, last chunk age, semisync on or off); `seed-drill` (planned handoff on the **test** seed and back, ends `SEED DRILL OK`); `seed-witness` (turns the witness on in the relay, with backup and rollback) |
| 0.9 | **Drills with real PCs**: the owner's PC as host and 1 to 2 volunteer Keepers. Drill 1: planned handoff. Drill 2: pull the host's plug. Measure downtime and loss | owner + 2 volunteers, about 2 evenings | | `seed-drill`, then `seed-check` logs |

**Go/no-go for showing it at release (as Preview):**
- drill 2 loses ≤ 90 s and returns in ≤ 5 min;
- `seed-check` is green for 72 h;
- the relay refuses a stale host in the test;
- no game file is in any chunk (the checker test);
- the brand check is clean.

**If release comes before this:** ship the Seed hidden, or ship "**Tier 0**" (about 1 week): one host, snapshots and
chunks to one Keeper, and a manual `seed-takeover` gated by the witness. That is 0.1, 0.2, 0.3 (lease only), 0.5
and 0.8. The badge waits.

### Phase 1: weeks 1 to 4 after release

- Automatic election among Keepers (v1, section 2.4). **M**
- Live semisync replication over agent-to-agent tunnels through the relay, TLS pinned to node keys. **L**
- Multi-relay host tunnels (from LEYLINE-MESH, needed anyway). **L**
- Armory and leaderboard export for the site. **S**
- Keeper onboarding in HQ (candidate, then Keeper). **S**
- A second witness on a steward's relay. **S**
- Raise the cap to 30 and add Shadowfang Keep and Blackfathom Deeps. **S**

### Phase 2: weeks 4 to 10

- An Eluna-enabled CMaNGOS pin (or VMaNGOS, if the owner prefers its progression accuracy), then port `ov_qol`,
  challenge modes and duels. **L**
- mmaps built by Keepers overnight. **S**
- The odd-rows audit report. **M**
- Caps to 40, then 60, with raids last.
- **Graduation to Official** by the rule already in DISCORD-PLAN.md: 30 days at 99 % up, on 5 Keepers in 2 regions,
  with a weekly failover drill, then a staff flip.

### Phase 3: only with the owner's yes

- The Classic+ race/class tables as a **recipe** (S3).
- Browser play for this realm (S4).
- Bots.

### Risks

| Risk | How likely | What we do |
|---|---|---|
| **Legal action** (section 6). Spreading hosting does not spread or remove the risk; it adds volunteers to it | real | The hard lines, no client changes, no money, a takedown switch (staff block + manifest retire), and the owner's legal read before release |
| Too few trusted Keepers. Then it is one PC with a backup | high at first | Owner's PC + 2 volunteers is enough for v0; the "keep a Seed" prompt in the queue and on the Host page |
| Split brain from clock skew or a bug | low, very costly | Self-fence at lease expiry, epoch checks at every relay, the drills, a skew check in `seed-check` |
| A host cheats or leaks data | medium | Keepers only, signed chain, rollback, removal |
| Duplicated items after a crash rollback | medium | 60 s saves, the odd-rows report, honest notice in the house rules |
| World server start is slow on home PCs (handoff > 5 min) | unknown | Measure in drill 1; keep mmaps off; later a "hot" standby that pre-loads maps |
| Home uplinks too slow or flaky for the host | medium | Election prefers the Keeper with the best measured rtt to the relays; PlayerLimit from load |
| The CMaNGOS pin and the old Open Vanilla image differ | unknown | The "check first" in 1.2 |
| A full queue on day 1 | medium | The queue line; that is a good problem |

---

## 4. Owner decisions (S1 to S8)

| # | Question | Recommendation |
|---|---|---|
| **S1** | Launch on **1.12 CMaNGOS** (the Open Vanilla line; Lua features later) or on **`vanillaplus`** (AzerothCore, the newer client; all our Lua works, about 4 GB per host, browser slice exists)? | **1.12 CMaNGOS**. `vanillaplus` stays the fallback if drill 1 fails |
| **S2** | The launch cap and slice: **1 to 20** with the three dungeons? | Yes |
| **S3** | The old Classic+ race/class tables: drop them, or rebuild them later as a recipe made on each PC from the player's own client? | Drop for launch; decide after a legal read |
| **S4** | Browser play for the Seed: none (desktop only), or the 1.12 browser slice on our box? | None at launch (D2 keeps our box to the main realm) |
| **S5** | Who may be a Keeper: owner-picked only, or after 14 days as a candidate plus a drill, approved by you or a steward? | The second, with you approving |
| **S6** | May our relay keep the encrypted chunk mailbox (ciphertext, 24 h, ≤ 200 MB a realm), or should it be a vote only, with Keepers holding everything? | Mailbox yes: it is not a realm copy and it can't be read |
| **S7** | The realm's name and house-rule words, and the Keeper's "you hold others' data" note | `TODO(owner)`. Never "Turtle" or anything like it |
| **S8** | TurtleHD Patch and Project Reforged links: keep them pending/skipped for good? | Yes: keep both off |
| also | Can the owner find out whether the old 1.12 image had code changes (1.2 "check first")? | Needed before 0.1 |

---

## 5. What this reuses

| Piece | State | Where |
|---|---|---|
| 1.12 host profile (build from source, own-client extract) | built | `hosting/kit/profiles/cmangos-1.12.1` |
| Hero Key for cmangos realms | built | `wuhost/coredb.py`, `wunet/herokey.py` |
| Relay tunnel, ports per realm id, join pages | built | `hosting/relay/wurelay/relay.py` |
| Mesh records, gossip, staff blocks, node registry, health checks | built | `wurelay/meshnode.py`, `nodes.py`, `wunet/mesh.py` |
| Be-a-node (caps, idle switch, stop every node) | built | `docs/leyline/be-a-node.md` |
| Ruleset attestation and Seed badge | designed | ROADMAP 3.3 |
| Multi-relay agents, sealed path | designed | LEYLINE-MESH |
| Lease, witness, log shipping, handoff | **new (this plan)** | |

---

## 6. Legal and brand notes

- **What happened to the server this is modelled on:**
  - the maker sued it in 2025;
  - in April 2026 a US court issued a **permanent injunction** ([PC Gamer][pcg]);
  - the reported terms bar the operators from running private servers, from "building or distributing modified or
    'remastered' clients", and from successor projects ([Icy Veins][iv]);
  - it shut down in May 2026 after about eight years. Before that, it took **donations for in-game rewards**
    ([Windows Central][wc]).
- **Our lines already avoid the worst of that:**
  - no money of any kind;
  - no client, changed or not;
  - no game data on our servers or over P2P;
  - no brand words to players;
  - no decompiled code;
  - nothing from that project (its code, client, patch packs or name).
- **What our lines don't change:** running an emulated server for this game is still the thing that was sued.
  Spreading it over volunteers' PCs doesn't make it legal. It spreads the exposure to the volunteers. The Keeper
  sign-up must say so in plain words (`TODO(owner)`).
- **Recommendation:** a short legal read by the owner before the Seed is shown, and a one-switch retire:
  - a staff block on the realm id;
  - a "retired" manifest;
  - relays refuse its hosts;
  - launchers hide it within a minute (the same machinery as "Stop every node").
- **Words for players:**
  - "Seeded · community-run", "Keeper", "host";
  - no client version in prose (the build is a data field and badge only);
  - the legal line only where it already appears.

---

## Sources (read 3 Nov 2026)

- [wc]: Windows Central, on why that server was popular and its shutdown (April 2026),
  https://www.windowscentral.com/gaming/pc-gaming/world-of-warcraft-finally-kills-pirate-server-turtle-wow
- [mop]: Massively OP, "Six ideas ... Classic Plus should borrow" (races, combos, challenges, professions),
  https://massivelyop.com/?p=590057
- [tbt]: Tales by Travel, shutdown guide (timeline, injunction scope, GM policing), https://www.talesbytravel.com/blogs/gaming-blog/turtle-wow-shutdown-guide
- [pcg]: PC Gamer, the injunction (12 April 2026),
  https://www.pcgamer.com/games/world-of-warcraft/blizzard-wins-injunction-against-vanilla-world-of-warcraft-private-server
- [iv]: Icy Veins, the settlement terms,
  https://wp-prod.icy-veins.com/turtle-wow-is-over-blizzard-settlement-forces-shutdown/
- [semisync]: MariaDB, Semisynchronous Replication (AFTER_SYNC vs AFTER_COMMIT, the timeout fallback),
  https://mariadb.com/docs/server/ha-and-performance/standard-replication/semisynchronous-replication
- [pitr]: MariaDB, Point-in-Time Recovery with mariadb-backup,
  https://mariadb.com/docs/server/server-usage/backup-and-restore/mariadb-backup/point-in-time-recovery-pitr-mariadb-backup
- [galera]: MariaDB, Galera Cluster deployment variants (WAN commit latency, odd number of nodes),
  https://mariadb.com/docs/galera-cluster/galera-architecture/galera-cluster-deployment-variants
- [fence]: Leader election with epoch fencing, https://www.techinterview.org/post/3233469928/lld-leader-election/
- [ipfs]: Filebase, IPFS content addressing, https://filebase.com/blog/ipfs-content-addressing-explained/
- [eluna]: Eluna development guide (supported cores), https://deepwiki.com/ElunaLuaEngine/Eluna/5-development-guide
- [vmangos]: VMaNGOS core (progression from 1.2 to 1.12, GPL-2.0), https://github.com/vmangos/core
- CMaNGOS `mangosd.conf.dist.in` (`PlayerSave.Interval` default 900 000 ms, `PlayerLimit` 100, `GridUnload` 1,
  `MaxPlayerLevel` 60), https://github.com/cmangos/mangos-classic/blob/master/src/mangosd/mangosd.conf.dist.in
- AzerothCore memory guidance (about 4 GB for 1 to 10 players up to 16 GB for 100+; maps cached up to 11 GB),
  https://git.nea.moe/github/wiki.git/tree/docs/memory-usage.md
- EVE Online, moving solar systems between server nodes at downtime (the planned-handoff pattern at MMO scale),
  https://www.eveonline.com/article/qd92g0/fleet-fight-notification-tool

[wc]: https://www.windowscentral.com/gaming/pc-gaming/world-of-warcraft-finally-kills-pirate-server-turtle-wow
[mop]: https://massivelyop.com/?p=590057
[tbt]: https://www.talesbytravel.com/blogs/gaming-blog/turtle-wow-shutdown-guide
[pcg]: https://www.pcgamer.com/games/world-of-warcraft/blizzard-wins-injunction-against-vanilla-world-of-warcraft-private-server
[iv]: https://wp-prod.icy-veins.com/turtle-wow-is-over-blizzard-settlement-forces-shutdown/
[semisync]: https://mariadb.com/docs/server/ha-and-performance/standard-replication/semisynchronous-replication
[pitr]: https://mariadb.com/docs/server/server-usage/backup-and-restore/mariadb-backup/point-in-time-recovery-pitr-mariadb-backup
[galera]: https://mariadb.com/docs/galera-cluster/galera-architecture/galera-cluster-deployment-variants
[fence]: https://www.techinterview.org/post/3233469928/lld-leader-election/
[ipfs]: https://filebase.com/blog/ipfs-content-addressing-explained/
[eluna]: https://deepwiki.com/ElunaLuaEngine/Eluna/5-development-guide
[vmangos]: https://github.com/vmangos/core
