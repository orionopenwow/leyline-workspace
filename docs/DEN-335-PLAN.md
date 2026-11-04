# Your Den on 3.3.5a only: the plan (research, 4 Nov 2026)

Internal doc. Version numbers are fine here; never in player-facing text.

The owner's ask (4 Nov): "Your Den should have never been on 1.12; we need to port everything over to the 3.3.5
client. Not now, but deep research it and store the plan for later."

## 0. The short answer

**The Den is already on 3.3.5a (build 12340), end to end.** Den Lite, the launcher's Den flow, the den-test, the
Docker Den, the Builders' Realm, the Leyline UI and the browser client all speak 3.3.5a. Git history shows Den Lite
never spoke 1.12: no commit in `labs/den-lite` ever had build 5875 (`git log -S5875 -- labs/den-lite` is empty).

What is left of 1.12 near the Den is small:

1. **Packs** (`hosting/kit/tools/wuhost/packs.py`) still accept a 1.12 (or 2.4.3) game folder as the source and copy
   its models into a client patch for the 3.3.5a game. 1.12 models (M2 v256) do not load in 3.3.5a. This is the one
   real 1.12 dependency.
2. **DEN.md** still tells players "any version you have: 1.12, 2.4.3 or 3.3.5a" for packs.
3. **The guard is only in some places.** The launcher refuses a non-3.3.5a folder for Your Den, but Den Lite itself
   does not check the build of the folder or the dbc it is given.
4. **Unclear why the owner sees 1.12.** See section 6, question 1. The likely causes are things that *look* old (the
   GM island, a level 1 human in a white shirt, the original character models that 3.3.5a still uses) or a 1.12
   folder selected in the play bar, which makes Your Den answer "plays with a different game folder".

So the "port" is not a rewrite. It is: remove the 1.12 pack path, add hard build checks, fix the docs, and prove
"fresh install, through to in the Den" on 3.3.5a with the first-run test.

## 1. What is on which version today

### 1a. Already 3.3.5a (evidence)

| Component | What it speaks / reads | Evidence |
|---|---|---|
| Den Lite login server | SRP6 login, client build 12340 only | `labs/den-lite/src/login.rs:2`, `:31` (`CLIENT_BUILD: u16 = 12340`) |
| Den Lite world server | gtker crates with the `wrath` feature and the wrath header crypto | `labs/den-lite/Cargo.toml` (`wow_srp` `wrath-header`; `wow_world_messages` `wrath`) |
| Den Lite gameobjects | 3.3.5a update fields (no position fields in values) | `labs/den-lite/src/gizmo.rs:17` |
| Den Lite account data (key bindings) | 3.3.5a `CMSG_REQUEST_ACCOUNT_DATA` / per-character slots | `labs/den-lite/src/accdata.rs`; README "The game's own settings" |
| Den Lite object and creature tables | AzerothCore wotlk world DB (`gameobject_template`, `creature_template_model`) | `labs/den-lite/tools/gen_objects.py`, `tools/gen_creature_models.py` header |
| Den Lite models, spells, talents, tier sets | the player's own 3.3.5a dbc, copied by the launcher | `launcher/src-tauri/src/host/lite.rs:173-245` (`model_table`, `game_tables`) |
| Den Lite integration test | a 3.3.5a test client | `labs/den-lite/tests/client.rs:24-27` (`wrath_header`, `wow_world_messages::wrath`) |
| Launcher "Play solo" for the Den | refuses any folder that is not 3.3.5a | `launcher/src-tauri/src/app.rs:1113` (`Build::V335a`) |
| Launcher `den-play` CLI (den-test) | picks a 3.3.5a folder only | `launcher/src-tauri/src/app.rs:1152-1153` |
| Den page | asks for a 3.3.5a folder | `launcher/ui/den.js:35` (`BUILD = "3.3.5a"`), `:70`, `:187` |
| Den terrain (Forge, ground delta) | ADT version 18 with MH2O, 3.3.5a archive order | `launcher/src-tauri/terrain/src/adt.rs:1`, `:475`; `host/terrain.rs:80` |
| Forge | Noggit Red build (3.3.5a editor) | `launcher/src-tauri/src/host/forge.rs:1-10` |
| den-check / den-test headless client | `WoW\0 3.3.5` build 12340 | `tools/den-check/wowclient.py:1`, `:90`, `:162`; folder pick `den_check.py:169` |
| Realm-side Den (Docker Den, Builders' Realm) | mod-ale on AzerothCore 3.3.5a | `realm/custom/lua_scripts/ov_den.lua:1`, `ov_build.lua:1`; `realm/profiles/den.env:19`, `build.env:29` |
| Light Den data slice | reads the player's own 3.3.5a archives, AzerothCore extractor | `docs/DEN.md` "A light local Den"; `hosting/kit/tools/wuhost/denslice.py` |
| Leyline UI (DenBar, Builder, Gizmo, Guide, Dojo, EditMode) | `## Interface: 30300` | `stream/fui-web/Interface/AddOns/WrathUnleashedUI/WrathUnleashedUI.toc:1` |
| Browser client and its Den hooks | benilla-wotlk web port (3.3.5a) | `client/get-source.sh`, `client/README.md`; `docs/DEN.md` "Engine hooks for the browser client" |
| Pack layouts | target build 12340 | `hosting/kit/tools/wuhost/packs.py:51` (`TARGET_BUILD = 12340`) |

### 1b. Still 1.12-aware (what to change)

| Component | What is 1.12-specific | Evidence | Why it is there |
|---|---|---|---|
| Pack builder | takes a 1.12 / 2.4.3 folder as source; copies its M2 (v256-263) and other client files into `packs/client/patch-L.MPQ` for the 3.3.5a game | `hosting/kit/tools/wuhost/packs.py:193-212` (`m2_deps` both layouts), `:349` (`source = game.build()`), `:368` (`patch` when `source != TARGET_BUILD`) | 1 Nov: "make a pack from any folder you have". 1.12 M2 keep their skins inside the file; 3.3.5a needs `.skin` files and v264. Those models will not draw. 1.12 display ids also differ from 3.3.5a, so the pack's layout props can point at wrong models |
| DEN.md packs section | tells players any version works | `docs/DEN.md:397` | same |
| Den Lite inputs | no check that `--client` and the copied dbc are 3.3.5a | `labs/den-lite/src/models.rs:41` reads any `GameObjectDisplayInfo.dbc` shape; `lite.rs:315` passes whatever folder it was given | the launcher guards it today; a hand start or a future caller would not |
| solo.js "another version's realm" | 1.12 folder -> `cmangos-1.12.1` Host profile | `launcher/ui/solo.js:56` | not the Den (a full 1.12 realm). Keep, but make sure the Den card never offers it |

### 1c. 1.12 things that are *not* the Den (leave alone)

| Thing | Where | Note |
|---|---|---|
| The old Open Vanilla site and realm images | `/opt/wenilla-realm`, `docs/VANILLA-REMASTERED.md` | the "1.12 images" in the handoff. Not used by the Den. House rule: no `ghclean` on them, no `docker compose up` there |
| Seeded vanilla realm plan | `docs/SEEDED-REALM-PLAN.md`, `cmangos-1.12.1` profile | its own project |
| Game folder detection for 1.12 | `launcher/src-tauri/src/clients.rs`, `docs/leyline/launcher.md:13` | players still add 1.12 folders for 1.12 realms |
| The `wenilla-host` name in the browser stack | `docs/GAME-DATA-ON-OUR-SERVER.md:25,39`, `docs/SURVIVORS.md` | only a program name; the browser client is the 3.3.5a port (`play/Play 3.3.5a in browser.bat`) |

## 2. Target picture

- Your Den runs only with a 3.3.5a game folder, everywhere: Den Lite, the Docker Den, the Builders' Realm, the
  browser Den hooks, Forge, packs.
- Every program that takes a game folder or game tables for the Den checks the build (12340) and says so in one plain
  line when it is wrong ("Your Den needs your Wrath game folder").
- Packs read only the 3.3.5a folder the player plays the Den with. No client patch is made, because nothing needs
  converting. (`--client-patch` goes away, or stays `never`.)
- A player with only a 1.12 folder sees, on the Den card, "Add your Wrath game folder to build a Den", not an error
  after pressing Play.
- One test proves it: fresh profile, add the 3.3.5a folder, press Your Den, land in the Den, place, save, reload.

## 3. Options

| Option | What it is | Effort | Risk | Verdict |
|---|---|---|---|---|
| **A. Keep Den Lite on 3.3.5a; remove the 1.12 leftovers; add guards and the proof** | sections 4 and 5 below | S (2-3 days) | low: no protocol work, only removals and checks | **Recommended** |
| B. Run the Den as a mode on a local AzerothCore (the Docker Den) only | drop Den Lite; every Den is the `den` Host profile | M to set as default, but the player pays: Docker, 5-20 min first database import, about 4 GB | high for first-run: Docker is the step most new players fail | No. It already exists as "Share with friends"; keep it as that |
| C. Run the Den on the official realm (Builders' Realm / `OV_DEN=1`) | log in to the official server's Den | S (it exists: `realm/profiles/build.env`) | needs internet and a Hero Key; not "your own room on this PC"; server load | No as the default. Keep the Builders' Realm as the shared build space |
| D. Teach Den Lite 1.12 as a second protocol | the opposite direction | L | doubles every packet path | No: the owner wants 3.3.5a only |

Upstream that shortens the work (already in use, nothing new needed):
- gtker `wow_messages` (`/home/claude/gtker/wow_messages`): one definition set for 1.12, 2.4.3 and 3.3.5a; Den Lite
  uses its `wrath` side.
- AzerothCore (`/home/claude/azerothcore`): the reference for 3.3.5a packets and the source of Den Lite's tables
  (world DB SQL, MIT/GPL boundary already respected: data rows only, no code).
- No other Rust 3.3.5a emulator is needed. Den Lite is already the small one.

## 4. Recommended path (Option A), in phases

Each phase ends with tests that must pass before the next.

### Phase 1: say it plainly (docs, half a day)

- `docs/DEN.md:397`: packs read "your own 3.3.5a folder, the one you play the Den with".
- `labs/den-lite/README.md`: one line up top, "3.3.5a only; there is no 1.12 Den".
- `docs/HYBRID-CLIENT.md`, `docs/FIRST-RUN-TEST.md`: Den steps name the Wrath folder.
- Test: `python3 stream/tests/test_brand.py stream/wrath_portal.py` (docs that become player text), a grep that no Den
  doc says the Den runs on 1.12.

### Phase 2: packs on 3.3.5a only (1 day)

- `packs.py build`: refuse a source that is not 12340 with one line ("Packs are made from your Wrath game folder").
  Remove the `client_patch` path and `patch-L.MPQ`, and the v256 branch of `m2_deps`.
- The launcher side that calls the pack builder (`host/pack.rs`, `ui/hostflow*.js` if they offer a folder): offer only
  3.3.5a folders.
- Tests: `hosting/tests/` pack tests updated: a 5875 fixture is refused, a 12340 fixture builds with no client files;
  `python3 -I hosting/tests/test_den_packs.py`.

### Phase 3: hard guards in Den Lite and the launcher (1 day)

- Den Lite: `--client <folder>` checked (the exe's build, as `clients.rs` reads it, or `component.wow-*.txt`); a wrong
  build stops at start with one line and an event `bad_client` in `den-lite.events`. The dbc loaders (`models.rs`,
  `specs.rs`, `gearsets.rs`, `spells.rs`) check the 3.3.5a record size and field count and ignore a table that does
  not match (today they would read garbage from a 1.12 table).
- Launcher: `lite.rs` `model_table`, `game_tables`, `copy_places` refuse a non-3.3.5a folder (belt and braces beside
  `app.rs:1113`).
- Den page (`den.js`): with only a 1.12 folder, the card says what to add before Play is pressed.
- Tests: `cargo test` in `labs/den-lite` (a 1.12-shaped dbc fixture is ignored; `--client` with a 1.12 exe stops);
  `cargo test --lib -q` in `launcher/src-tauri` (lite.rs refuses); `node --test tests/den.browser.test.cjs` (a 1.12-only
  fake shows the add-folder line).

### Phase 4: the proof, fresh install to in the Den (2 days; shares work with HANDOFF §8.2)

- `tools/first-run/first_run.py` with `LEYLINE_APP_DIR` set to an empty folder: add the 3.3.5a folder, then
  `Leyline den-play --test` (exists: `app.rs:1128`), which gets Den Lite on first use, signs in and enters the world.
- Pass when `den-lite.events` has, in order: `login_challenge`, `auth_ok`, `realm_list`, `world_auth`, `char_enum`,
  `player_login`, `in_world`, `builder_ready`, then the den-test's build / save / load steps (`tools/den-check/den_check.py`
  STEPS) and an `autosave`.
- Also run it once with a 1.12 folder as the only folder: pass when it stops with the add-folder line, nothing started.
- A Control step `first-run-test` (the handoff already plans it); the owner presses it on his PC. Then `den-test` and
  `den-check` as today.

### Phase 5: the rest of the Den family (as each is next touched)

- Docker Den (`den` profile): `denslice.py` already reads 3.3.5a; add the same build check before extraction.
- Builders' Realm: nothing; it is the official 3.3.5a server.
- Browser Den hooks (`docs/DEN.md` "Engine hooks"): already the 3.3.5a port; nothing to move.

## 5. What breaks for current Den users, and their saved Dens

- **Saved Dens move as they are.** The file (`leyline-den` version 1) holds catalog entries from the AzerothCore
  world DB and 3.3.5a display ids. Nothing in it is 1.12. No conversion needed. Den Lite, the Docker Den and the
  Builders' Realm read the same file.
- **Account data** (`<dens>/account-data/`, key bindings) is 3.3.5a already. Kept.
- **Packs made from a 1.12 folder** (only if someone did): their `pack-<id>.den.json` layout may point at the wrong
  models, and a `patch-L.MPQ` may sit in the player's Wrath `Data` folder. Migration:
  - on start, the launcher reads `state/packs/*/pack.json`; any `source_build` other than 12340 is marked "made from
    another game version: rebuild from your Wrath folder" with a one-click rebuild;
  - the launcher offers to remove the `patch-L.MPQ` it made (only the one it made: match by its own manifest, never
    another patch; ask first, house rule on deleting).
  - Dens that already placed such props: the props stay (entries are valid), only the look may be off; a rebuild of
    the pack and `.den load pack-<id> add` brings the right ones.
- **Players with only a 1.12 folder** lose nothing they had: Your Den already refused that folder. They now get a
  clearer line earlier.
- No change to the official realm, the community realms or the 1.12 realms.

## 6. Open questions for the owner

1. **What did you see that looked like 1.12?** A screenshot helps. Possible causes: a 1.12 folder chosen in the play
   bar (Your Den then refuses), the GM island and the level 1 human in a white shirt, or the original character models
   (3.3.5a still uses them). If it is the look, the fix is the Den's start (place, character, gear), not the client.
2. **Remove packs from 1.12 and 2.4.3 folders entirely** (recommended), or keep them for 1.12 realms only (never the
   Den)?
3. **Should the Den's default character look newer** (a level 80, a Wrath-era outfit, a Wrath-era start place such as
   Dalaran's edge) so it does not read as "old game"? This is a design call, not a port.
4. **The `cmangos-1.12.1` "Play solo" path** (`solo.js:56`): keep it for 1.12 realms, or hide it until the seeded
   realm ships?
5. **Phase 4 shares the fresh-machine test** with HANDOFF §8.2: build them together (recommended)?
