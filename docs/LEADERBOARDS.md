# Leaderboards: Boss Rush and Duels, signed by the realms

Owner, 12 Nov 2026: "leaderboards with the authentic servers attached to them". Every result on these boards is signed
by the realm that recorded it, and every reader (the website, every launcher) checks the signature before it shows a
row. **Authentic** means a server produced it: the official realm, or a listed community realm that signs its board.
A client never produces a board result. Runs on a player's own PC (Boss Rush, single player) go to a separate,
clearly labelled **Local runs** board, and only when the player presses Share. Boards give bragging rights only:
no rewards, nothing to buy.

## 1. The record format

The same style as the mesh (docs/LEYLINE-MESH.md section 3): Ed25519 over `context + "\n" + canonical JSON` (sorted
keys, no spaces, UTF-8), `realm id = base32(sha256(public key))[:16]`. The key file has the same shape as a hosted
realm's (`wunet/keys.py`: 32 bytes as hex, 0600).

| Context | Signed by | What |
|---|---|---|
| `leyline-board-result-v1` | the realm's key | one result |
| `leyline-board-v1` | the realm's key | the whole board: every result, the week, the featured fight |
| `leyline-board-local-v1` | the player's launcher key (`peer.key`, docs/LEYLINE-FRIENDS.md) | one run on the player's own PC |

```text
board   {"doc": DOC, "sig": hex}
DOC     {"v": 1, "realm": id, "pubkey": hex, "name", "ts", "seq", "week": "YYYY-MM-DD", "featured": {"bossrush": key},
         "results": [{"r": RESULT, "sig": hex}, ...]}
RESULT  Boss Rush: {"v": 1, "realm", "mode": "bossrush", "board": fight key, "diff": "normal"|"heroic",
                    "kind": "group"|"bots", "name" (the leader), "class", "ms", "size", "bots",
                    "party": [{"name"|null, "class", "bot"}], "at", "run", "deaths" (only when counted)}
        Duels:     {"v": 1, "realm", "mode": "duels", "board": "duel"|"arena-1v1", "period": "all"|"week:YYYY-MM-DD",
                    "name", "class", "rating", "wins", "losses", "points", "at"}
LOCAL   {"r": {"v": 1, "peer", "mode": "bossrush", "board", "diff", "engine": "exact"|"light", "name", "class",
               "spec", "ms", "at"}, "pubkey": hex, "sig": hex}
```

**Checks every reader makes** (`boards-export.py check_board`, `stream/boards.py check_board`,
`launcher/src-tauri/src/boards.rs check_board`): the key makes the id; the document is signed by it (so a mirror can't
drop or add a row); every result is signed by it under the result context and names this realm; the board is at most
400 days old. One bad result refuses the whole board. Then only the shown fields are kept, each checked (names 2-12
plain letters with no brand word, class words, numbers in range); anything else in a signed record is ignored.

**Why both levels:** the document signature makes the board whole; the per-result signature makes one row portable
(a profile card, a Discord post, a cross-realm board later) and still provable on its own.

**Official vs community:** a launcher calls a board official only when our own site's index lists it as official
(fetched over https) or its key is pinned in `boards.rs OFFICIAL_KEYS` (TODO(owner), below). A community board is
one a listed community realm sent and the site checked.

Test vector: `launcher/src-tauri/src/boards-vector.json`, made by `stream/tests/test_boards_export.py` (deterministic
Ed25519, seed 7s); the Rust tests verify it, so Python and Rust agree byte for byte.

## 2. The realm side

| Piece | What |
|---|---|
| `realm/custom/lua_scripts/ov_boards.lua` (`OV_BOARDS=1`) | deaths in boss fights (`ov_board_deaths`: every player or companion killed by a creature in a catalog raid map, with the instance); 1v1 arena results (`ov_arena_log`: winner, loser, week, day, seconds) from `BG_EVENT_ON_END`; `.boards` (where the boards are, this week's featured fight). It never reads a table |
| mod-leyline-bossfights (unchanged) | the clears themselves: `leyline_bossfight_runs` (time pull to kill, party, bots, `gm`, `tier`) |
| `ov_duels.lua` (unchanged) | duel ratings and weekly points |
| mod-1v1-arena (unchanged) | the 1v1 arena ratings (`arena_team`, type 1) |
| `realm/custom/boards-export.py` | cron every 5 minutes: fixed SELECTs in a READ ONLY session, then signs the board into `/srv/wrath/portal/duels/boards/<realm id>.json` |

**What goes on the board:** Boss Rush, per fight and difficulty, each leader's best clear of all time and best this
week (only `result = clear`, `gm = 0`, `tier = 0`); deaths counted from `ov_board_deaths` in the run's instance between
its pull and its end. Duels: ratings (at least 3 rated duels), this week's points; 1v1 arena ratings and this week's
wins (at most 3 wins a day against the same opponent count). At most 100 per board. Same privacy as before: names and
class words only, `.armory off` hidden, bots (RNDBOT accounts) never ranked.

**Weeks** start Monday 00:00 UTC everywhere (the boss boards' week). The **featured fight** is the realm's fights in
catalog order, one a week (`featured`: Python, `featured_fight`: Rust, `B.featured`: Lua; tests keep them equal). The
official realm offers one fight, so its featured fight is always The Frozen Throne; a Boss Rush install (all twelve)
turns through them.

**A community realm** runs the same export with its own key (`WU_BOARD_KEY` = its `state/realm.key`,
`WU_DB_CONTAINER`) and `--post https://leylinelauncher.com/wrath/api/boards/submit`. The site takes it only from a realm
listed on the network, one board every 4 minutes. Not yet wired into the host kit's agent (plan, section 5).

## 3. The website (`stream/boards.py`)

| Path | What |
|---|---|
| `GET /wrath/boards` | Boss Rush / Duels / Local runs; this week or all time, fight, difficulty, board, realm, name; every row with "Signed by <realm>" and Official / Community |
| `GET /wrath/api/boards/index.json` | the realms with a board: official, community, and the local board's address |
| `GET /wrath/api/boards/<realm id>.json` | the realm's signed board, byte for byte, as `application/vnd.leyline.board+json` so the site's text filters (brand switch, old names) can never change a signed byte |
| `POST /wrath/api/boards/submit` | a listed community realm's signed board |
| `GET /wrath/api/boards/local.json`, `POST /wrath/api/boards/local` | the local-run board; posting needs the launcher's sign-in (Bearer) and a run signed with the launcher's own key. Each player's best per fight, difficulty and engine; the account is never shown |

## 4. The launcher

* `src-tauri/src/boards.rs`: `boards_get` reads the index and each board, checks every signature, and hands the UI
  rows marked with the realm that signed them (last view cached for offline); `boards_share_local` signs a run with
  `peer.key` and posts it as the signed-in player.
* `ui/boards.js` (+ `boards.css`): the Leaderboards view on the list kit (docs/LAUNCHER-LISTS.md): tabs Boss Rush /
  Duels / Local runs, the choices (When, Fight, Difficulty, Board, Realm), search, the **Only you** chip and the count;
  **That's me** on a row marks that name on that realm as you (kept in `leyline.boards.me`): your rows are lit and
  marked You on every board.
* **Mount points:** any element with `data-board="bossrush" | "duels"`. The Modes page has
  `<div id="md-boards" data-board="bossrush">` (added in `modes.js`'s page, since the modes agent's own mount wasn't in
  this worktree; if theirs lands, keep one of the two). The Boss Rush page has `#br-boards` under its panel
  (`index.html`). `LeylineBoards.mount(el)` mounts one by hand. Nothing is asked of the core until the page opens.
* Boss Rush's page: the week's featured fight gets a **This week** chip on its tile; each best time has **Share**
  (a name for the boards, 2-12 letters) onto the local-run board.

## 5. Deploying (Leyline Control)

| Step | Brings |
|---|---|
| `realm` (Server pass) | `ov_boards.lua`; `boards-export.py` + `ed25519_pure.py` (step 1h: the tables, the key folder `/srv/wrath/leyline` 0700, the cron `/etc/cron.d/wu-boards`, one run, then `--whoami` prints the realm id and public key); `OV_BOARDS=1`, `OV_BOARDS_FIGHTS=frozen-throne` in main.env (a new script and settings: the 5-minute restart) |
| `signin` | `boards.py` (fastloop checks `/wrath/api/boards/index.json`) |
| `ship` | the launcher (boards.rs, boards.js) through the usual CI build |

Afterwards: `scripts-reload` is not enough for the first load (a new script loads with the restart). `check` reads
`/wrath/api/boards/index.json`.

## 6. Built in this slice vs planned

**Built:**

| Mode | Item | Where |
|---|---|---|
| Both | signed boards, the format, the checks, the site page, the launcher view | above |
| Both | local runs: opt-in, signed by the launcher, kept apart and labelled | boards.py, boards.rs, boards.js |
| Boss Rush | weekly featured fight + weekly board (BOSS-RUSH.md #1) | export, Lua, Rust, site, launcher tile |
| Boss Rush | per-fight boards for every catalog fight a realm offers | export, site, launcher |
| Boss Rush | deaths on each clear (results card's story, a first step of #8) | ov_boards.lua, export |
| Boss Rush | online leaderboard, uploaded only when the player says so (#24) | Share, local board |
| Duels | the missing 1v1 arena results and a weekly 1v1 arena board (the "ladder is coming" gap) | ov_boards.lua, export |
| Duels | duel ratings and weekly points on signed boards across realms | export |

(Duel reset already exists: mod-leyline-qol / `main.env` "Duel reset on".)

**Planned, in order (each fits AzerothCore + mod-ale, or the launcher):**

1. **Daily seed (Boss Rush light):** one seed for everyone a day (`sha256(date)`), one attempt counted; the light engine
   is deterministic by seed, so the site can later re-simulate seed + inputs to verify a local run (then a local run
   could earn a "replay verified" mark). Needs Den Lite to take a `seed` in `bossrush.json`.
2. **Friend co-op (exact):** the Boss Rush install opened to a friend through the Den invite path (relay tunnel): two
   heroes, eight bots. Results are still local (the friend's PC isn't a listed realm) unless the host lists its realm.
3. **Fair duels (gear templates):** `.duel fair`: both duelists swapped into the same premade kit (`ov_kits.lua`
   kits, or mod-npc-talent-template), restored after; a separate "Fair" duel board.
4. **Bot backfill for the 1v1 queue:** mod-playerbots arena bots after N minutes in queue; such games never count for
   the boards (the Lua already drops bots).
5. **Spectate:** the core's `.spect` and a "live now" list on `/wrath/boards`.
6. **Seasons:** a season id in the board document (e.g. monthly), "last season's top 3" kept as signed results, titles
   as cosmetics.
7. **Host kit agent:** run the export and `--post` from the agent's own scheduler, so community realms sign with no
   extra step; a `board` capability in the mesh info doc (host_protocol_features F3).
8. **Mesh-carried boards:** boards gossiped like realm records (they are already self-verifying), so they survive the
   site going dark.

## 7. Not verified (needs a real realm)

* `BG_EVENT_ON_END` firing for mod-1v1-arena's arenas, `Map:GetPlayers()` still holding both players then, and
  `Unit:IsDead()` telling the winner (a timeout with both alive is not counted). From mod-ale's docs, not a run.
* `PLAYER_EVENT_ON_KILLED_BY_CREATURE` for companion bots; deaths from falls or the environment are **not** counted
  (no Lua hook for them), so "deaths" means deaths to the boss and its adds.
* The host's own python running `boards-export.py` (standard library, `ed25519_pure.py` beside it: ~4 ms a signature).
* The site's page in the hub frame on the live site, and launcher builds after `ship` (CI compiled and tested the core).

## 8. Owner decisions

* **Pin the official key:** after the `realm` step prints `board realm id … public key …`, put the public key in
  `launcher/src-tauri/src/boards.rs OFFICIAL_KEYS` (until then "official" comes from our site's index over https).
* **Back up the board key** (`/srv/wrath/leyline/board-realm.key`): losing it changes the official realm's board id.
  The server pass copies it into its root-only backup folder; say if it should go anywhere else.
* **Community boards:** open now to any listed realm; say if you want an allow list or a "verified host" label first.
* **Local runs:** open to any signed-in launcher; fine as "not checked", or hide the tab until replay checks exist?
* **Names and words:** "Leaderboards", "Local runs", "That's me", "This week's featured fight", the site's lines:
  TODO(owner) where you want your own words.
* **Featured rotation:** catalog order, one a week; or a hand-picked list.
* **Tests:** `test_duels.py` and the Lua cross-checks need `lua5.1` (texlua has no `-e`); `test_ov_boards.lua` runs
  under either.
