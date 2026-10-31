# Game mods and the mods a realm uses

Owner, 31 Oct: "'Mods' should also have a simple section, trim search, and should have a client mod section like
future skating and golf etc things that mod in game, we need to ensure these mods work on servers that allow it too
like minecraft style adaptations and downloads and like gmod too".

The model is Garry's Mod and Minecraft servers: **a realm says which mods it runs, and a player who joins is offered the
ones their game needs, in one click.** Players never hunt for files, and a realm can never push anything a player's
launcher would not install from the Workshop anyway.

## The Mods page

Website `/wrath/leyline/catalog` and the launcher's Mods › Discover open on the **Simple view**: one search box and a few
short rows of big cards (Popular, Game modes, Make it look nicer, Quality of life). **Browse everything**
(`/catalog?view=all`, or any search or filter) is the full catalog: one search box, four smart chips (Everything, Addons,
Game modes, Works on Wrath Unleashed), and Sort, Show, Client, From, Kind and the addon kinds under **More filters**.
The rows are `Hub.simple_rows` (stream/hub.py) and `launcher/ui/simple.js`, with the same three lists. Routes, `all.json`
and `catalog.json` are unchanged. A **List your mod** button (`data-slot="list-your-mod"`; the launcher calls
`window.LeylineListMod.open()` when it exists) is the entry point for the submission flow, which lives elsewhere.

## Game mods

**Game mods** (website tab `/catalog?view=game`, launcher Discover › Game mods) are mods that change how you play in game:

| Kind | Id | Where it comes from | What it needs |
|---|---|---|---|
| Leyline Mode | `mode:<id>` | `stream/playmodes.json`, kind `realm` (minigolf, bumper balls, coin dash, community minigames) | a realm with the Leyline realm scripts (`ov_modes.lua`, `ov_golf.lua`, `ov_playmode.lua`); plays through the Leyline addons |
| Workshop mod | `mod:<id>`, `up:<slug>` | the Workshop catalog, categories `sandbox`, `maps`, `encounters`, `conversions` | `realm`/`both` kinds: a realm that installed it; `ui`/`both`: files in your game, installed by the launcher |

Each card says what it adds, whether it needs a realm that allows it, how it reaches your game, and **Play it on:** the
live realms whose mod list names it. A future mode (skating and the like) shows as **Coming** only once it is a real
entry (`status: "coming"` in playmodes.json or the catalog): nothing is made up. The logic is `stream/gamemods.py`
`entries()` / `runs_on()` and `launcher/ui/gamemods.js`.

## The realm mod list

A realm's listing record carries `mods`:

```json
"mods": [{"id": "mode:minigolf"}, {"id": "mod:dalaran-dice", "version": "0.1.0"}, {"id": "up:skada"}]
```

- `id`: `mod:` + a letter and 2-31 of `a-z 0-9 -`; `up:` + 1-64 of `a-z 0-9 . _ -`; `mode:` + a letter and 1-31 of
  `a-z 0-9 -`. `version` (optional): 1-32 of `A-Z a-z 0-9 . + -`. At most 32 entries, each id once; anything else in an
  entry is dropped. A plain string is read as `{"id": ...}`.
- The same rules in four places, checked to match by `stream/tests/test_gamemods.py`: `stream/gamemods.py`
  (`clean_mods`), the relay's `hosting/relay/wurelay/listing.py` (`clean_mods`, on every heartbeat), the host kit's
  `hosting/kit/tools/wuhost/setup.py`, and the launcher's `realms.rs` (`parse_mods`) and `ui/gamemods.js`.
- **Community realms:** the host kit puts it in the signed heartbeat's record. Every AzerothCore realm the kit makes
  lists the built-in Leyline Modes by itself (its scripts include them; `BUILTIN_MODES`), then the host's own:

  ```
  leyline mods                          # list
  leyline mods add mod:dalaran-dice@0.1.0 up:skada
  leyline mods remove up:skada
  ```

  (`bash host-wrath-realm.sh mods ...` on the older script; or edit `"mods"` in `state/beacon.json`, then
  `leyline rules`). A linked server (`leyline link`) and the other cores list only what the host adds.
- **Our realm (Wrath Unleashed):** from its own config, not a heartbeat: the live Leyline Modes it runs and every
  packaged Workshop mod the `mods` step installs on it (`all.json` `on_wrath`), `Hub.official_mods()`. It goes out in
  `/wrath/realms.json`'s `sample` block (`sample.mods`), which the launcher reads into the official realm.

Realm cards show **Uses N mods** (site: the list, which ones go in your game, and "Get the launcher to add them
automatically when you join"; launcher: the names on hover).

## Joining a realm from the launcher

On PLAY (the Realms page, and the play bar for the official realm or a listed realm address), `LeylineGameMods.beforeJoin`
(`launcher/ui/gamemods.js`):

1. Reads the client's installed mods (`mods_installed`) and the realm's list.
2. Keeps only **catalog** mods with a **client part** that this client lacks and the core can check: a packaged mod with
   its download and sha256, or a reviewed upstream addon with its install block, made for this client. Realm-only mods
   (`kind: "realm"`) need nothing on the client; ids the catalog doesn't know are left out; a URL or path never passes
   the id check. A realm that runs Leyline Modes also brings the Leyline addons into the same offer when they are
   missing (so `addons.js` doesn't ask a second time).
3. If anything is left, asks once: **"<Realm> uses X, Y"** with **Get them and play**, **Play without them** and
   **Cancel** (stops PLAY). Nothing installs before the click.
4. Installs each through the existing `mod_install` command with the project id only: the core looks the id up in its
   own cached Workshop catalog and checks the download's hash and every file (`mods.rs`, `upstream.rs`). A mod with native
   code (`.dll`, `.asi`, `.exe`) still asks its own one-time OK through `withNativeOk`; saying no skips that one mod. A
   mod that fails to install is reported and PLAY goes on.

## Safety

- A realm can only name ids. It never supplies a URL, a file, a hash or code; the launcher installs only what is in its
  own Workshop catalog, with the catalog's checks.
- Only catalog mods can be required; unknown ids are shown by their id on the site and never offered by the launcher.
- Native code keeps its per-mod consent. Nothing installs without the player's click.
- Text shown to players goes through the brand filter (`neutral`, `banned`) on the site; names come from the catalog and
  playmodes.json, which are checked at their source.

## Later

- **Skating and other new modes**: add the mode to `stream/playmodes.json` (and its realm script); it then shows on Game
  mods, as Coming while `status` is `coming`, and realms with it list it.
- **Host page UI** in the launcher for the mod list (today: `leyline mods` and `state/beacon.json`).
- **Version pinning on join**: the version is shown, not enforced; the launcher installs the catalog's current version.
- **Required vs optional** per mod (GMod's "required" addons), and a realm-side check that a `mod:` id is really
  installed on the realm (today the host's word, like the rest of the listing).
- **Per-realm Workshop collections** from the website's host page (claude/server-creator-platform-plan.md).
