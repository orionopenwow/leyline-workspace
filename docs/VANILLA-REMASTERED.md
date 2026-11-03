# Vanilla Remastered: the 1.12 realm back, as an official Leyline Beacon

Owner decision (2026-10-30 20:38): Vanilla Remastered is visible at launch as an **optional official server**.
"Stream it temporarily until someone takes over via P2P": we host and stream it for now, as before, and a community
host takes it over through the Leyline P2P network later. The GitHub package cleanup never touches the 1.12 images.

This page is the owner's runbook (the same text is in the Projects doc `claude/vanilla-remastered-revival.md`).
Nothing here runs by itself: each step is an Autopilot step that Claude queues and the owner's Control window runs.

## What it is

The old Open Vanilla 1.12 realm (CMaNGOS Classic, Classic+ "pace", client build 5875), on the same server box as the
Wrath Unleashed, under a new name. Its own database (`wenilla-realm-mariadb-1`), its own logins (the old 1.12
accounts), its own characters. Players reach it only through the Leyline relay, the way they reach the Hangout.

Why it can't simply be switched back on: when the Wrath realm was deployed (`server/deploy.sh`) it took the box's game
ports (3724 and 8085), and `wu-portal` took the name `mangosd` on the website's network (the website's account bridge
to the Wrath realm). The old `wenilla-realm-mangosd-1` / `wenilla-realm-realmd-1` containers publish those ports and
carry that name. Starting them would fail, or worse, split the website's new accounts between two servers. So the old
containers stay exactly as they are (stopped, `restart=no`), and the `ov112-on` step runs **copies** of them: the same
image ids (what was live before, never pulled again), settings, environment and volumes, as `vr-realmd` and
`vr-mangosd`, on their own internal network `vr-112`, with no published port and no route out.

## The steps, in order

| # | Step | What it does | What it touches |
|---|---|---|---|
| 1 | `ov112-realm-check` | Read-only. The old containers (present, stopped, their ports), the database, memory and disk, and a report on the 1.12 site: its Caddy routes, the browser game's address settings (names and values of address-like settings only, nothing secret), what `/`, `/play`, `/game`, `/data/`, `/ws/3724` answer, and the game data killswitch. Ends `OV112 REALM DRY`. | Nothing. |
| 2 | `ov112-on` | Backup, start, self-check, roll back on failure (details below). Ends `OV112 REALM ON`. | New: network `vr-112`, containers `vr-realmd`, `vr-mangosd`, env files `/root/vr112-env/*.env` (0600), backup `/root/vr112-on-<time>/`. Changed: `wenilla-realm-mariadb-1` joins `vr-112` (its usual names). Not touched: the old containers, Duels, the Herald proxies, the 1.12 timers, the website, Caddy, the Wrath realm. |
| 3 | `vr-beacon` | Lists it on the Leyline network as the official Beacon "Vanilla Remastered", like the Hangout: signup off, its own sign-in. Ends `VR BEACON OK`. | New: `wu-vr-agent` (64 MB, read-only), image `wu-vr-tools:1`, `/srv/wrath/vr-beacon/` (its key, made there, never copied), `/srv/wrath/portal/vr-realm.id`, `/srv/wrath/vr-realm.id`. Changed: the 1.12 realm list row's address and port (backed up first). |

Undo, in this order: `vr-beacon-off` (off Leyline; the realm list row goes back), then `ov112-realm-off` (the two game
servers stop; kept, not removed). `ov112-undo` is the full undo of tonight's `ov112-off` (Duels, Herald, the timers):
not needed for Vanilla Remastered, and not to be used for it.

### ov112-on in detail (`tools/ov112-off/start-112-realm.sh`)

1. **Checks.** Root and Docker; `wenilla-realm-realmd-1` and `wenilla-realm-mangosd-1` exist and are **stopped** (a
   running old container and a copy must never share the database: it refuses); the database is up; at least 1500 MB
   of memory available (`VR_MIN_FREE_MB`) and 3000 MB of disk for the backup; a python image for the self-check
   (`python:3.12-alpine` or `wu-hangout-tools:1`, already on the box; nothing is pulled).
2. **Backup.** `mariadb-dump --all-databases` of `wenilla-realm-mariadb-1` into `/root/vr112-on-<time>/mariadb-all.sql.gz`
   (0600, size-checked), and what existed before. The step changes no data; the dump is for a hand restore only.
3. **Start.** The internal network `vr-112`; the database joined to it under its usual names; `vr-realmd`, then
   `vr-mangosd` (`restart unless-stopped`). Run again: containers that are already up are left alone.
4. **Self-check**, any failure rolls back everything this run made:
   - the login server answers a 1.12.1 logon challenge for a made-up account (`probe112.py logon`);
   - the world server greets with `SMSG_AUTH_CHALLENGE` within 10 minutes (`VR_WORLD_WAIT`), then has not restarted;
   - the 1.12 site (`wenilla-realm-realm-1:8090`) still answers;
   - on the website's network, `mangosd` is still only `wu-portal` (the website's accounts don't split);
   - the Wrath realm's containers are still up (reported).

### vr-beacon in detail (`hosting/vanilla-remastered/vr-beacon-up.sh`)

The Hangout's script (`hosting/hangout/hangout-up.sh`) with these differences:
- `setup.py official --name "Vanilla Remastered" --auth-service vr-realmd:3724 --world-service vr-mangosd:8085`, then
  the listing record says what the realm is: core `cmangos`, build `5875`, ruleset `custom`, tag `custom`, profile
  `cmangos-1.12.1`. (`setup.py official` writes the Hangout's AzerothCore / 3.3.5a shape; the record is what the agent
  sends as is.)
- No second login server. Instead it checks the 1.12 login server's `WrongPass.MaxCount`: every relay player arrives
  from the agent's address, so a per-IP lock would lock everyone out at once. Above 0, it refuses (set it to 0, or
  `VR_ALLOW_WRONGPASS_LOCK=1`). The relay counts wrong passwords per player IP itself.
- **The realm list row** (`classicrealmd.realmlist`): after the tunnel is up, its address and port become the relay's
  IPv4 and this realm's relay world port, so a stock 1.12.1 client that logged in through the relay comes back through
  it. The row is saved first (`/srv/wrath/vr-beacon/backups/<time>/realmlist.row`) and put back on a failure and on
  `vr-beacon-off`. One row only; with several, it asks for `VR_REALM_ID`. No account is made or changed.
- Health: the tunnel; the relay sees it online; the world server answers through the agent; **a 1.12.1 logon through
  the relay's own port for this realm** answers; the listing has it. Then `vr-realm.id` for the website and the
  in-game list.
- No status URL (players and bots show as unknown on the list, as nothing reports them yet).

## How players reach it

- **Leyline launcher / realm list.** It is listed by the relay as a normal Beacon (1.12.1, CMaNGOS, "own client files").
  It shows as **Official** only once the website reads `vr-realm.id` (workstream A, see "Website changes" below);
  until then it is listed like any community realm. Players need their own 1.12.1 client.
- **A stock 1.12.1 client.** `realmlist relay.openvanilla.app:<auth port>` (the step prints it), or open its join page,
  then `realmlist relay.openvanilla.app`. Old accounts sign in with their old login. The old direct address
  (`play.openvanilla.app`) now reaches the Wrath login server and won't take a 1.12 client.
- **In the browser, levels 1-20** (31 Oct, below): the OG 1.12 site and game page again, as `vr-site` at
  `https://vanilla.openvanilla.app/`, playing the levels 1-20 slice. The realm card shows "Play in your browser" once
  the page is up (`vr-site.url`).
- **New players.** Through the page: the OG site's own sign-up (its "Play now" Hero Key and its account creation),
  which now reaches the Vanilla Remastered world server (`MANGOSD_HOST=vr-mangosd`) instead of the Wrath bridge.
  Through the relay with a stock client: returning players only (signup off on the Beacon).

## Levels 1-20 in the browser (owner 2026-10-31)

Owner: "Vanilla Remastered planned with minimal slices for 1-20 leveling with the style we had in the og project ...
compact for the server and working with all other features too." Yes to a second 1.12 browser slice (levels 1-20 zones
only, behind the same game data switch); the look is the OG Open Vanilla site and in-game UI as they were.

This morning's `ov112-data-remove` emptied the world server's data (volume `wenilla-realm_gamedata`, 2.4 GB) and the old
browser stream (`/srv/client/Data`, 5.1 GB; `play.openvanilla.app/data/*` answers 410). Three steps put back only what
is needed, from the owner's own 1.12.1 client on his PC.

### Tonight's order (Leyline Control steps)

| # | Step | About | What it does |
|---|---|---|---|
| 1 | `vr-data-check` | 1 min, read-only | Finds the PC's 1.12.1 Data folder (`%USERPROFILE%\Games\WoW-1.12.1\Data`, then `Desktop\OpenVanilla\Data`, or the step's argument; `Desktop\OpenVanilla\Data` has no archives, only loose tables and the interface), the archives' size, and the server's disk, memory, extractors and what is done. Ends `VR DATA CHECK OK`. |
| 2 | `vr-data` | upload (5.1 GB: ~10 min at 10 MB/s, ~45 min at 2 MB/s) + 50-90 min on the server | See below. Ends `VR DATA OK`. Resumable: run it again after any stop. |
| 3 | `ov112-realm-check`, `ov112-on` | 1 min, then 2-10 min | As above (the realm's servers as `vr-realmd` / `vr-mangosd`). |
| 4 | `vr-beacon` | 3 min | As above (Leyline). `vr-data` already set `WrongPass.MaxCount = 0`, so it no longer refuses. |
| 5 | **owner**: DNS | 2 min | An A record `vanilla` -> the server's address at the registrar (Squarespace). `vr-site-check` prints the exact address. |
| 6 | `vr-site-check`, `vr-site` | 1 min, then 3 min | The page. Ends `VR SITE ON`. |
| 7 | `signin` | 2 min | Ships `stream/network.py`: the realm card's "Play in your browser" button. |
| (any time) | `vr-test` | 1 min | Self-check with a fresh TEST account. Ends `VR SITE TEST OK`. |

No realm restart of the Wrath realm in any of these. The Vanilla Remastered world server is stopped during `vr-data`
(it refuses to run while `vr-mangosd` runs) and started by `ov112-on`; a later settings change to it means a restart with
the 5-minute warning (`server restart 300` on its console: it announces the countdown itself).

### vr-data (`hosting/vanilla-remastered/vr-data.ps1` on the PC, `vr-data.sh` on the server)

1. **Upload**: the base archives, `patch.MPQ` and `patch-2..9.MPQ` (never other servers' letter patches) to
   `/srv/vr112/upload` (0600) in 256 MB pieces, each SSH login at least 6 s apart. An interrupted piece is sent again
   from the last whole piece; each archive counts only when its SHA-256 on the server matches the PC's. Disk is checked
   first (the peak: archives 5.1 GB + data 1.3 GB + the larger of the 1.5 GB temporary vmap source and the 2.8 GB slice
   budget, + 1.5 GB margin = about 10.8 GB; 13 GB free this afternoon, before the Builders' Realm).
2. **The job** runs on the server as the systemd unit `vr-data` (log `/srv/vr112/vr-data.log`), so a dropped connection
   doesn't stop it; running the step again follows it. Each phase leaves a marker in `/srv/vr112/state` and is skipped
   once done:
   - **extract**: CMaNGOS Classic's own extractors (`ad`, `vmap_extractor`, `vmap_assembler`) from the realm's own
     images (the `vr-mangosd` image first; nothing pulled), with no network and the archives read-only, into
     `wenilla-realm_gamedata`: `dbc/`, `maps/`, `vmaps/` (the temporary `Buildings/` is deleted at once). **mmaps are
     skipped** (hours on this box; `VR_MMAPS=1` builds the two continents) and turned off in the settings. If the
     images hold no extractors, it stops here and says so (then: build them from CMaNGOS Classic at the image's commit,
     not done tonight).
   - **classicplus**: the Classic+ server tables (`/opt/openvanilla-classicplus/dbc`: SkillRaceClassInfo,
     CreatureModelData, CreatureDisplayInfo) over the stock ones, stock kept as `.orig`, as `openvanilla_server_dbc.sh`
     did. `patch-Z.MPQ` (ours, the Classic+ client content) joins the upload folder for the slice.
   - **slice**: `vr-slice.py` in the `vr-mpq-tools:1` image (StormLib from its authors' public source, tag v9.26, built
     on the box). `slice-112.json`: a base (interface, tables, fonts, characters, items, spells, the shared sounds,
     transports, the racial mounts' and pets' creature folders) and tiers in order: the Alliance and night elf starts and
     capitals, the Horde and undead starts and capitals, Westfall, Loch Modan, Redridge, Darkshore, the Barrens,
     Silverpine, the tram, Ragefire Chasm / Wailing Caverns / the Deadmines, then music and ambience. A zone is its
     world-map rectangle in `WorldMapArea.dbc` (+1 tile), its tiles' textures, models and buildings walked like the
     OG stream and the Wrath slice, its minimap tiles, and the realm's own spawns inside those tiles (their model folders,
     baked NPC skins and creature sounds; a read-only query on `classicmangos`). Tiers go in until `budgetMB` (2800) or
     until the disk would keep less than 3 GB; the report says which tiers are in. It writes NEW archives with the
     original names into `/srv/vr112/slice/Data`, each file in the archive it came from (`patch-Z.MPQ` copied whole),
     so the old site's own `/data` serves the slice and nothing else.
   - **settings** (every file backed up to `/root/vr112-conf-<time>/`): `mangosd.conf` `GridUnload = 1`,
     `mmap.enabled = 0` (no mmaps); `aiplayerbot.conf` `MinRandomBots` / `MaxRandomBots = 0` (`VR_BOTS=N` for a
     handful) and `RandomBotAutologin = 0`; `realmd.conf` `WrongPass.MaxCount = 0`; memory limits `vr-mangosd` 1100 MB
     (+512 MB swap), `vr-realmd` 96 MB. It reports `SOAP.Enabled` (the page's sign-up and the self-check need it). If
     the image's start script rewrites these from its environment, it says so: check `docker logs vr-mangosd`.
   - **clean**: the uploaded archives are deleted (`VR_KEEP_UPLOAD=1` keeps them). The extracted data and the slice stay.

### vr-site (`hosting/vanilla-remastered/vr-site-up.sh`)

`vr-site` is a copy of the old site container `wenilla-realm-realm-1` (same image id: the site, sign-in and browser
client build that were live before, never the private `leyline-client-112` merge), which keeps running untouched:
- its game connection: `REALMD_HOST=vr-realmd`, `MANGOSD_HOST=vr-mangosd` on `vr-112` (the realm's internal network);
  it is also on the website's network (alias `vr-site` only) so Caddy reaches it;
- its game data: `CLIENT_DATA` = `/srv/vr112/slice/Data`, read-only;
- its own volumes are copies (`vr-site-<name>`, made once); bind mounts are shared as they were; the game accounts
  are in the 1.12 database as before;
- address settings that named `play.openvanilla.app` now name its own address (their names are printed, never values).

**Address: `https://vanilla.openvanilla.app/`, a subdomain, not a path.** The page reads `/data/...` and `/ws/...` from
the root of its own address (the OG `prefetch.js`, `sw.js`, `syncread.js`, `warmup.js` all build `'/data/' + name`), and
on `play.openvanilla.app` the root, `/data` (410 since this morning) and `/ws` belong to other pages, so a path would need
a different client build. No existing subdomain was free for it (the Wrath stream uses `wrath.`, the relay `relay.`).
The owner adds the DNS record; until then `vr-site` stops before touching Caddy and prints the record.
`VR_SITE_HOST=<name>` uses another name. Google / Discord sign-in on the new address need its callback added at
Google / Discord (the owner's consoles); the OG "Play now" Hero Key sign-in needs nothing.

Self-check (also the `vr-test` step): the page answers inside and at its address; what `/data/__index` answers without
sign-in (reported: if the OG site serves its files without sign-in, it says so); brand words on `/`, `/play`, `/game`,
`/key`, `/login` (reported, the OG pages are as they were); then a **fresh TEST account** (`VRTEST` + 6 digits, a random
password, never printed) made through the realm's own account command (SOAP `account create`, with the SOAP login from
the site's own settings, handed over in a 0600 file and deleted at once), a full 1.12.1 SRP6 logon and realm list on
`vr-realmd` and through the relay's port for this realm (when on Leyline), the world server's greeting, and the account
deleted. Never the Chumpton account, never the owner's password. Any failure rolls back what the run made.

Off: `vr-site-off` (container removed, its Caddy site out; copied volumes kept).

### Memory and disk budget

| | Memory | Disk |
|---|---|---|
| `vr-mangosd` (no random bots, grids unload, no mmaps) | limit 1100 MB (+512 swap); expect 600-900 MB | data ~1.2 GB (dbc, maps, vmaps) |
| `vr-realmd` | 96 MB | - |
| `vr-site` | 256 MB | slice <= 2.8 GB (budget) |
| `wu-vr-agent` | 64 MB | - |
| During `vr-data` only | extractors 1.5 GB limit (the realm is stopped then) | archives 5.1 GB + 1.5 GB temporary, deleted at the end |

## The game data killswitch and the 1.12 stream

One switch for both (owner 2026-10-31). `gamedata-off` (`stream/server/gamedata-switch.sh off`) now also puts a 410
block for `/data` into the Vanilla Remastered page's Caddy site when it is installed (`/srv/vr112/site-host`), checks
`https://<its address>/data/__index` answers 410, and with `--park` moves `/srv/vr112/slice/Data` off the served folder
(`vr-site` restarts on an empty placeholder); `gamedata-on` undoes both; `status` shows it. `vr-site` puts the block in
at once when the switch is already on. The Survivors slice rules stay as they are. The old site's own `/data` on
`play.openvanilla.app` stays 410 (`ov112-data-remove`'s block).

## Checks after launch (by hand or the read-only `check` step)

- `docker ps --filter label=leyline.vr=1` shows `vr-realmd`, `vr-mangosd` up, and `wu-vr-agent` after `vr-beacon`.
- `https://relay.openvanilla.app/r/<realm id>/status.json` says online, client 1.12.1.
- A test login with an old 1.12 account through the relay reaches the character list (never the Chumpton account).
- `free -m`: the Wrath realm keeps its headroom; `docker stats vr-mangosd` for the 1.12 world server's memory.
- The Wrath site and the Hangout still work (`/wrath`, a Hangout login): the 1.12 copies must not have changed them.

## Rollback

| What | How |
|---|---|
| A step failed | It already rolled back what it made; the last lines say why. Nothing to do. |
| Off Leyline | `vr-beacon-off`: the agent goes, `vr-realm.id` goes, the realm list row goes back. The key stays. |
| 1.12 game servers off | `ov112-realm-off` (refuses while it is on Leyline): `vr-mangosd` first (it saves players), then `vr-realmd`; kept with `restart=no`. `ov112-on` starts the same containers again. |
| Data damage | By hand only: stop both (`ov112-realm-off`), restore `/root/vr112-on-<time>/mariadb-all.sql.gz` into `wenilla-realm-mariadb-1`. Ask the owner first. |
| Remove the copies for good | `docker rm vr-mangosd vr-realmd` and `docker network rm vr-112` (the volumes are the old containers' and stay). Deleting: ask the owner first. |

## Risks

1. **Two world servers on one database.** The old containers must stay stopped while the copies run. `ov112-on`
   refuses when an old one runs, but nothing stops someone starting an old one later: never run `server/rollback.sh`,
   `start-112-jobs.sh` with the old 1.12 realm in its backup, or `docker compose up` in `/opt/wenilla-realm` while
   Vanilla Remastered is on. (`deploy.sh` uses `up --no-deps realm`, which is safe.)
2. **Memory and CPU** next to the Wrath realm and its bots. The step wants 1500 MB free; the 1.12 world server with its
   random bots may want more. Watch `free -m` after launch.
3. **Shared volumes.** The copies use the old containers' volumes. `docker compose down -v` in `/opt/wenilla-realm`
   would delete the 1.12 world.
4. **The in-game realm name** comes from the 1.12 realm list row (the step prints it and warns on brand words). Players
   see it in the client. It is left as it is (open question).
5. **Version numbers on site pages.** The relay's records carry 1.12.1; if the website shows a "1.12.1 · CMaNGOS" tag
   on the realm cards, that breaks "no client version numbers except the version chip" (a website change).
6. **The killswitch gap** above.
7. **Images.** The copies run the image ids that were live; nothing is pulled. The GitHub cleanup now refuses to delete
   `openvanilla-*`, `openwow-*`, `*mangos*` and the other 1.12 packages (`tools/github-cleanup/gh_cleanup.py`
   `PROTECTED_PACKAGE_WORDS`), and the local `PLAN.json` no longer lists them.
8. **The 1.12 site's own pages** (`wenilla-realm-realm-1`) are as they were; they were not checked for brand words here.

## A community host takes it over later (P2P)

The goal: someone in the community runs Vanilla Remastered on their own machine through the Leyline network, and the
official flag moves to them. What they need, and the order:

1. **The host kit's `cmangos-1.12.1` profile** (`hosting/kit/profiles/cmangos-1.12.1`, "Host a profile realm.bat" or
   the launcher's Host page). It builds CMaNGOS Classic from its authors' public source at a pinned commit and extracts
   the server data from **the host's own 1.12.1 client** (`extract.sh`). We never send game data. Its
   `leyline-realmd.conf` already sets `WrongPass.MaxCount = 0`.
   **Check first:** whether the `openvanilla-mangos` image was built from stock CMaNGOS or from our fork with Classic+
   code changes. If it has code changes, the profile needs that source pinned too (and as GPL code we should publish it
   anyway), or the takeover runs stock CMaNGOS with our database only.
2. **The realm database export**, made on our box at a freeze: `classicmangos` (the world, with the Classic+ content),
   `classiccharacters` (the characters), and `classicrealmd` (accounts). Handed over privately, never in a repo, a
   release or a download. The accounts table holds every player's login verifier and email: moving it to another
   person's machine needs a notice to players and their consent (or only characters move, and players claim them with
   a fresh login on the new host). Owner's call.
3. **The official flag transfer.** "Official" is ours to give, not something a host can set: the relay marks every
   Beacon `official: false`; the website marks a realm id official (`vr-realm.id` today, or the admin console's realm
   overrides). The new host's agent makes its own key, so its realm id is new. Transfer: staff verify the new realm
   (Verified host), the website's official id becomes the new one, then we run `vr-beacon-off` and, after a quiet
   period, `ov112-realm-off`. Players' join links and relay port change with the id; the realm list moves them.
   (Handing over our `realm.key` would keep the id, but the key never leaves our box by design.)
4. **Cutover order.** Announce; freeze (world server shutdown with a warning); final export; the host imports and
   links (`leyline link --core cmangos --build 5875`, or the profile's own setup); staff check a login and a character
   on it; flip the official id; `vr-beacon-off`; keep our backup until the owner says.

## Website changes this needs (not made here)

For workstream A (`stream/network.py`) and B (`stream/hub.py`):
- Read the Vanilla Remastered id like the Hangout's (`/app/vr-realm.id`, i.e. `/srv/wrath/portal/vr-realm.id`, or a
  `WU_VR_REALM` setting) and mark that relay record `official`, listed as optional (not the default realm), with a
  line like "Official, hosted by Leyline for now". Shown even while its record is missing, like the Hangout? Owner's call.
- No "1.12.1" text on the realm cards (only the version chip), and its join note should say "your own game client"
  without a version number.
- The in-game Leyline window (`realm/custom/leyline-fetch.py`) reads `/srv/wrath/hangout-realm.id` for the Hangout;
  the same for `/srv/wrath/vr-realm.id` if it should be marked there.
- The game data killswitch: a block for the 1.12 site (`stream/server/gamedata-switch.sh`).

## Files

- Levels 1-20 in the browser: `hosting/vanilla-remastered/vr-data.ps1`, `vr-data.sh`, `vr-slice.py`, `slice-112.json`,
  `mpq-tools.Dockerfile`, `vr-site-up.sh`, `probe112.py account`, and the .bat files `Vanilla Remastered data*.bat`,
  `Vanilla Remastered browser page*.bat`, `Vanilla Remastered self-test.bat`; the 1.12 part of
  `stream/server/gamedata-switch.sh` / `gamedata-caddy.py`; the card button in `stream/network.py` (`vr-site.url`).
  Tests: `hosting/vanilla-remastered/tests/test_vr_data_site.py`.

- `tools/ov112-off/start-112-realm.sh` and its three .bat files (`Turn on the 1.12 realm - check.bat`,
  `Turn on the 1.12 realm.bat`, `Turn off the 1.12 realm.bat`)
- `hosting/vanilla-remastered/vr-beacon-up.sh`, `probe112.py`, `rules.txt`, `Put Vanilla Remastered on Leyline.bat`,
  `Take Vanilla Remastered off Leyline.bat`
- Tests: `python3 -I -m unittest discover -s hosting/vanilla-remastered/tests` (stub Docker; nothing deployed)
