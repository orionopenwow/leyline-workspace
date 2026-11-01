# Leyline mod package, format 0.1

A Leyline mod is a folder, or a zip of one, with a `mod.json` at the top. It can change the UI (a standard 3.3.5a addon), the realm (mod-ale Lua on the AzerothCore 3.3.5a server), or both. `wu-mod.py check` enforces every rule on this page.

```
dalaran-dice/
  mod.json            required
  ui/DalaranDice/     a 3.3.5a addon: DalaranDice.toc, .lua, .xml   (kind "ui" or "both")
  realm/*.lua         mod-ale Lua                                    (kind "realm" or "both")
  assets/             your own or licensed files, with assets/CREDITS.txt
  tests/              optional fake-ALE tests (not installed)
  README.md, LICENSE  optional
```

## mod.json

| Field | Required | Rule |
|---|---|---|
| `id` | yes | 3-32 chars, lowercase letters, digits and single dashes, starts with a letter: `dalaran-dice` |
| `name` | yes | up to 48 chars |
| `version` | yes | semver `MAJOR.MINOR.PATCH` |
| `author` | yes | text |
| `licence` | yes | SPDX id or expression: `MIT`, `GPL-3.0-or-later`, `MIT OR CC-BY-4.0`, `LicenseRef-…` |
| `kind` | yes | `ui`, `realm` or `both`; must match the folders present |
| `wu` | yes | lowest Leyline version the mod works with: `"1.0"` |
| `description` | yes | up to 300 chars |
| `category` | no | the Workshop shelf, one id from `categories.json` (below). Default from `kind`: `ui` and `both` give `addons`, `realm` gives `realm-scripts`. Anything else is an error |
| `permissions` | no | list from the fixed vocabulary below |
| `screenshots` | no | `.png/.jpg/.webp` paths inside the package |
| `links` | no | `https://` URLs, or `{ "label": "…", "url": "https://…" }` |

## Categories

The one list is `mods/categories.json` (id, name, one line, icon name, in this order, plus `kind_defaults`).
`wu-mod.py` checks `category` against it, and `stream/workshop.py` reads the same file to draw the Workshop's
category grid and the pages `/wrath/mods/c/<id>`. To add or rename a category, change that file only.
The `mods` step sends it in the kit beside wu-mod.py and publishes it next to catalog.json in `$WU/portal/mods`
(the portal's `/app/mods`); in a checkout workshop.py falls back to `../mods/categories.json`.

| Id | Name | What goes there |
|---|---|---|
| `addons` | Addons and UI | interfaces, unit frames, nameplates, meters, quest helpers, bag and chat mods, UI skins |
| `graphics` | Shaders and graphics | lighting, sky, water, post effects, colour grades, graphics presets for our engines |
| `sandbox` | Sandbox and game modes | Garry's Mod-style kits such as Prop Hunt, hide and seek, races, murder mystery, tower defense, jump maps |
| `maps` | Custom maps | new zones, arenas, dungeons and housing plots |
| `builds` | Builds | saved Builder creations shared as codes |
| `encounters` | Boss fights and encounters | community bosses, encounter scripts, raid replays |
| `models` | Models and props | original creatures, mounts, props and buildings |
| `realm-scripts` | Realm scripts | server-side features such as QoL, events, economies, professions and companions |
| `presets` | Server presets | ready realm recipes such as a Dalaran hangout, duels-only, raid night, RP and hardcore |
| `audio` | Sounds and music | original ambience, music packs and UI sounds |
| `tools` | Tools | developer HUD plugins, map viewers, log analysers and OBS overlays |
| `conversions` | Total conversions | whole new games built from the categories above |
| `themes` | Launcher themes | Leyline launcher theme packs (`theme.json` + CSS + images, see `docs/THEMES.md`); not for `mod.json` packages |

## Permissions (realm only)

A realm mod may only use what it declares. Undeclared use is an error; declared but unused is a warning.

| Permission | Covers |
|---|---|
| `commands` | `RegisterPlayerEvent(42, …)` (dot commands) |
| `chat` | player chat events 18-22, server event 30 (addon messages), `OVMod.Send/On`, `SendWorldMessage`, `:SendAddonMessage`, `:SendUnitSay/Yell/Emote/Whisper`, `:Say/:Yell` |
| `gossip` | `Register{Creature,GameObject,Item,Player}GossipEvent` |
| `spawns` | `PerformIngameSpawn`, `:SpawnCreature`, `:SummonGameObject` |
| `events` | every other `Register*Event` (player, server, creature, gameobject, item, group, guild, BG, map, instance, spell, ticket) |
| `db` | `CharDBExecute/Query/QueryAsync`, `WorldDBExecute/Query/QueryAsync` |
| `timers` | `CreateLuaEvent`, `:RegisterEvent` |

`:SendBroadcastMessage` to a player needs no permission. Event ids must be numbers (or a `local NAME = 42` constant) so the checker can read them.

## Realm rules

1. **Events match permissions** (table above).
2. **SQL only on your own tables**, named `mod_<id with _ for ->_*` (`mod_dalaran_dice_rolls`), in the realm's characters or world database, through the `db` calls. The SQL must be a string literal (or `string.format` of one) so the table is visible. No other database names (`acore_auth.x`), no second statement after `;`, no `GRANT`, `LOAD DATA`, `INTO OUTFILE`, `SET GLOBAL`, `CREATE/DROP DATABASE`. Create tables from Lua with `CREATE TABLE IF NOT EXISTS`; `.sql` files are not run.
3. **Forbidden:** `io.*`, `os.*` other than `os.time/clock/date/difftime`, `loadstring`, `load`, `loadfile`, `dofile`, `require`, `module`, `debug.*`, `package.*`, `setfenv/getfenv`, `_G`, `string.dump`, `collectgarbage`, `HttpRequest`, `RunCommand`, `AuthDB*`, `RegisterPacketEvent`, `ReloadALE/ReloadEluna`, `Ban`, `Kick`. To talk to your addon use the realm's `OVMod` global (from `ov_modapi.lua`) without `require`, and check `if OVMod then`.
4. **`can` hooks return true or false explicitly.** Player events 48 CAN_INIT_TRADE, 49 CAN_SEND_MAIL, 50 CAN_JOIN_LFG, 55 CAN_GROUP_INVITE, 59 CAN_RESURRECT and 60 CAN_UPDATE_SKILL run through mod-ale's `CallAllFunctionsBool` with a default of false. A handler that returns nothing, `nil` or a non-boolean counts as **false and blocks the action for every player**. That is how the Dungeon Finder broke on our live realm. The checker requires the handler to end with a `return`, and every `return` to be `true`, `false`, a `not …` or a comparison.
5. **Globals** (warning): all realm scripts share one Lua state, so use `local`. Names starting with `DALARAN_DICE`/`DalaranDice` (your id) are allowed.

## UI rules

- Each addon is `ui/<Name>/<Name>.toc` with `## Interface: 30300`. Every file the TOC lists must exist (names are case-sensitive in the browser client). XML must be well formed, and `<Script file>`/`<Include file>` must point to real files. Lua must parse as Lua 5.1.
- The addon runs in the client's addon sandbox with the stock 3.3.5a API.

## Assets

Only files you made or may redistribute. Every file in `assets/` is listed in `assets/CREDITS.txt` (name, author, licence, source). Blizzard game data is never allowed anywhere in a package: `.mpq .blp .m2 .wmo .adt .dbc` (and `.skin .anim .wdt .wdl .db2 .bls`), by extension or by file signature. Use `.tga` for addon textures. Limits: 10 MB a file, 50 MB a package, 2000 files.

## Packing and install

`wu-mod.py pack <dir>` checks the mod, then writes `<id>-<version>.zip` (sorted entries under `<id>/`, fixed timestamps and modes, so the same files always give the same bytes) and `<id>-<version>.zip.sha256`. A realm installs `realm/` under its mod-ale `lua_scripts/mods/<id>/`, and the client installs each `ui/<Name>/` as an addon.

## The catalog

Approved mods live in `mods/catalog/<id>/` (the folder name is the mod id). `wu-mod.py catalog` checks every one
(one failure and nothing is written), packs each into `dist/`, and writes `mods/catalog.json`: per mod its id, name,
version, author, licence, kind, category (always set, the default filled in), wu, description, permissions, screenshots, links, `addons` (the `ui/` folder names),
`realm_files`, `zip`, `sha256` and `size`. The same files always give the same JSON. Addon folder names must be unique
across the catalog and may not be `ForeverUI`, `WrathUnleashedUI` or `WUModCatalog`. `--lua <dir>` also writes the
`WUModCatalog` addon (`WU_MOD_CATALOG = { ... }`), which the in-game Workshop tab reads.

The `mods` step (`mods/Put mods on Wrath.bat`, Autopilot step `mods`) puts the whole catalog live in one SSH
connection (`stream/server/mods-install.sh`): realm halves into `lua_scripts/mods/<id>/` with a live `reload ale`
(rolled back on a refused reload or a Lua error from `mods/`), addon halves into the streamed client
(`stream/server/mods-overlay.py`, beside Forever UI's overlay), and catalog.json, categories.json (from the kit), zips and screenshots to the site,
where players browse them at `/wrath/mods`.

## How this fits the larger plans

- This is the MVP from the community release plan ("Leyline mod package"). Later steps there: `wu-mod test` against a local test realm, human review for anything realm-side, and signed packages once approved.
- The client Open Mod API (`claude/open-mod-api-plan.md`) describes `mod.toml` and signed `.ovpack` packages with client permissions (`ui`, `packets`, `data`, `render`, `wasm`, `compat`). Those cover the client; the permissions here cover the realm. When the M1 loader lands, `mod.json` should map onto `mod.toml` field by field rather than become a second format.
- Realm-to-addon messages use the existing OVMOD channel (`ov_modapi.lua`), the server half of `Mod.Packet`.

## Upstream addons (the addon marketplace)

Third-party addons for the 3.3.5a client that the Workshop lists and the Leyline launcher installs **straight from the
author's own repo** (all.json `source: "upstream"`, id `up:<slug>`). They are not mod packages and need no `mod.json`.
We never host them: the launcher downloads the author's release asset, or the commit archive pinned to a 40-hex commit.
A copy on our site (`install.from: "mirror"`) is allowed only when `mirror_allowed` is true: an open licence **and** no
`.blp` files. Every entry is curated by `tools/addon-curate/curate.py` and only published once a person has reviewed it.

**What is never listed** (owner, 2026-10-31): paid or cracked products; anything with native code, injection or memory
editing (`.dll`, `.asi`, `.exe`, or that needs a patched client); UIs that bundle stock game art by their own TOC
(link-out only, or left out); re-upload collections of other people's addons (never an install source). Full interface
replacements (ElvUI and the like) are allowed with `replaces_ui: true`, which the launcher shows as "Replaces your whole
interface; may conflict with the Leyline UI". Display names never carry the game's brand words; the launcher also cleans
TOC titles it shows in My Mods.

**The rules** are one file, `launcher/src-tauri/src/upstream-rules.json`, built into the launcher (`upstream.rs`) and read
by curate.py; `upstream-cases.json` beside it is run by both test suites so they cannot disagree.

| Rule | Upstream addons (curated) | Mod packages |
|---|---|---|
| Game data `.mpq .m2 .wmo .adt .dbc .skin .anim .wdt .wdl .db2 .bls`, native code `.dll .asi .exe .so .dylib`, archives `.zip .rar .7z` (by extension or first bytes), anywhere in the download | refused, whole entry | refused |
| `.blp` textures | only when the entry says `blp: "author-art"` (the author's own art, from upstream only, never mirrored; names like stock interface art are flagged for the reviewer) | refused |
| Folder names | letters, digits, `_ . + ! -`, may start with `!` or `_` (`!BugGrabber`, `xCT+`); not `Blizzard_*` or the realm's own | starts with a letter |
| `## Interface:` | holds 30300, alone or in a list (`30300, 11508`) | exactly 30300 |
| Dev files (`.pkgmeta`, `.luacheckrc`, `.editorconfig`, `.github/`, `.yml`, `README`, `LICENSE`, screenshots, scripts) | skipped, never installed | error |
| Repo layout | addon folders up to two levels deep, or the repo **is** the addon (its `.toc` at the top: the folder is named after the `.toc` for this client, `Questie-335`, `pfQuest-wotlk`) | `ui/<Name>/` |
| Size | 32 MB a file, 256 MB, 12000 files | 10 MB, 50 MB, 2000 |
| Updates | when the catalog's `install.rev` differs from the installed one (versions such as `r736` or `3x05` are not compared) | semver |

**Checks at install** (launcher, `upstream.rs`): `install.url` must be the entry's own `repo` in one of these shapes:
`https://github.com/<o>/<r>/releases/download/<tag>/<file>.zip`, `https://github.com/<o>/<r>/archive/<commit>.zip`,
`https://gitlab.com/<o>/<r>/-/archive/<commit>/<r>-<commit>.zip`, `https://codeberg.org/<o>/<r>/archive/<commit>.zip`
(or our own `/wrath/mods/` for a mirror). Redirects are followed by hand, https only, to `objects.githubusercontent.com`,
`release-assets.githubusercontent.com`, `codeload.github.com`, `gitlab.com` or `codeberg.org`. Then the zip's sha256 when
the entry has one (release assets and mirrors; GitHub's commit archives are not byte-stable), and always every file:
each installed file's sha256 against `install.files`, or the digest of the whole set (`install.files_digest`: sha256 of
the sorted `<path>\0<sha256>\n` lines). Only then is anything moved into the client, through the mod manager, as
`up:<slug>` in `Leyline/mods.json`, so Remove, turn off and on, and keeping the player's edited files work as for any mod.
An entry that is not reviewed, claims native code, or whose install block does not check out is shown as a plain link.

### The entry (`mods/upstream.json`, written by curate.py)

| Field | Meaning |
|---|---|
| `id`, `source` | `up:<slug>`, `"upstream"` |
| `name`, `display_name` | the main addon folder; the clean name we show |
| `description` | our own words, one or two sentences |
| `category`, `tags` | `addons`; sub-categories: `full-ui unit-frames raid-frames action-bars nameplates meters boss-mods raid-tools auras cooldowns combat-text quests maps minimap bags auction gear tooltips chat social class professions pvp skins libraries quality-of-life` |
| `builds` | `["3.3.5a"]` |
| `author`, `author_url`, `repo` | the repo owner and the canonical repo link (the credit and the Source link) |
| `licence`, `licence_source` | SPDX-style id from the repo's licence file (or the TOC's `X-License`), `""` when none; the file it came from |
| `mirror_allowed` | open licence and no `.blp`: only then may we host a copy |
| `version` | the main addon's TOC version (or the short commit) |
| `install` | `from` (`release-asset`, `commit-archive`, `mirror`), `url`, `mirror_url?`, `rev` (tag or 40-hex commit), `sha256?` (byte-stable downloads only), `size`, `file_count`, `strip_prefix` (the archive's top folder), `folders` (archive folder → addon folder; `""` = the repo is the addon), `skip` (what curation left out, informational), `files` (`<Addon>/<path>` → sha256), `files_digest` |
| `addons`, `toc` | the addon folders it installs, and each one's Interface, title, version, author |
| `requires`, `conflicts` | other `up:` ids it needs (from the TOCs' `## Dependencies`, plus the seed's), and ones installing the same folders |
| `replaces_ui`, `native` | a whole-interface replacement; always `false` |
| `blp`, `blp_files` | `none` or `author-art`, and which files |
| `flags`, `notes` | for the reviewer: TOC files that are missing, textures named like stock art, brand words, the seed's notes |
| `review` | `{by, at, notes}`: empty `by` is pending; only reviewed entries are published |
| `compat` | `{realm: "Wrath Unleashed", status: works/untested/broken, checked_at}` |
| `pending_update` | a newer upstream rev of an approved entry, with the file diff; the approved install stays live until it is approved |

all.json carries the reviewed entries without `files` and `skip` (the digest stands for the list); see `all-json.md`.
`failed` (with the reason) and `excluded` (with the owner's rule) list what is not in.

### Curating and reviewing

```
python3 -I tools/addon-curate/curate.py run [--only ID ...]      # resolve, download, check, hash: new entries are pending
python3 -I tools/addon-curate/curate.py list                     # id, status, licence, files, flags; and the failures
python3 -I tools/addon-curate/curate.py approve up:skada --by Owner [--notes "..."] [--works]
python3 -I tools/addon-curate/curate.py unapprove up:skada
```

`approve` (and `run`) write `stream/upstream-addons.json`, the reviewed entries in the all.json shape, which
`stream/upstream_addons.py` merges into `/wrath/mods/all.json` (checked again there). The seeds are
`tools/addon-curate/seeds-addons.json` (repo, our name and words, tags, `replaces_ui`, `requires`, or `exclude` with the
reason). For the Admin Console, curate.py has `pending_entries(path)` and `approve_entry(path, id, by, notes, works)`.

## Listed mods (List your mod)

Any signed-in player can list a mod for one-click install (players' guide: `docs/LIST-YOUR-MOD.md`). The website
(`stream/listings.py`) and the launcher (`launcher/src-tauri/src/listing.rs`, `ui/listmod.js`) both offer it. We never
host the files: the launcher installs from the author's own repo (`launcher/src-tauri/src/listed.rs`).

**The link.** A public repo on GitHub, GitLab or Codeberg: the repo, `/tree/<branch>`, `/commit/<sha>`,
`/releases/tag/<tag>`, `/releases/latest`, or one release file `/releases/download/<tag>/<file>` (`.zip` or `.MPQ`).
It is pinned to one exact download: the release file (GitHub; its sha256 is recorded, release files are byte-stable)
or the archive of a 40-hex commit. A repo or branch link follows the branch (the latest release for a release link);
a commit or tag link never moves.

**The check** reads the download once (then deletes it) and detects the format:

| Format | Detected by | Client install | Goes live at once (default policy) |
|---|---|---|---|
| `addon` | addon folders whose `.toc` holds `Interface: 30300` (the curated addons' layout rules) | `Interface/AddOns/<Name>/` | yes, unless `.blp` art, stock-art names or a missing TOC file |
| `package` | `mod.json` at the top | its `ui/` addons and `native/` files; `realm/` only on realms | yes, unless native code |
| `patch` | `.MPQ` files at the top (or in `Data/`), or one `.MPQ` release file | `Data/patch-<X>.MPQ`, a free letter | no: a moderator first |
| `native` | `.dll/.asi/.exe` at the top (with their `.ini/.cfg/...`) | the client folder, after the player's OK | no: a moderator first |
| `realm-script` | `.lua` with `Register*Event` (no `.toc`) | none (for realm owners) | yes |
| `game-mode` | one `.lua` with `Mode.Title` and `On*` callbacks, at most 14 KB | none (for the Modes editor) | yes |
| `loose-data` | loose `Interface/`, `World/`... game files | refused: pack them as a patch | |

The rules are the shared `launcher/src-tauri/src/upstream-rules.json` (`upstream-cases.json` runs in the listings tests
too). Refused anywhere in the download: game data (`.m2 .wmo .adt .dbc .skin .anim .wdt .wdl .db2 .bls`, by extension
or first bytes; `.MPQ` only as a patch and only with MPQ bytes), nested archives, programs other than a native mod's (or a
package's `native/`), unsafe paths, links, passwords, case twins; realm and mode Lua that uses what Realm rules 3 forbids
(`io`, `os` beyond time, `load*`, `require`, `debug`, `_G`, `HttpRequest`, `RunCommand`...); `.blp` in a mod package.
Caps: 32 MB a file (1 GB a patch), the curated caps for the rest, `WU_LISTING_FETCH_MAX` (300 MB) read per check.

**The record** is a community project (`stream/projects.py`, `listing` object): `format`, `install` (the block the
launcher checks: `kind` `addon` or `files`, `from`, `url`, `rev`, `sha256`, `strip_prefix`, `files` path -> sha256,
`files_digest`, `folders` and `blp` for addons), `plan`, `flags`, `state` (`community` or `reviewed`), `follow`,
`pending_update`. all.json carries it as `source: "listed"` (`mods/all-json.md`).

**Policy** (`$WU_PORTAL_DATA/listings-policy.json`, set by the owner in Leyline HQ › Needs you): which formats go live
without a moderator (`auto_live`), `report_hide` (independent reports that hide a listing until a moderator looks,
default 3), `big_mb` (a big pack waits, default 64), `new_account_days` (3) and `new_account_per_day` (1). A listing
that is flagged, from a new account, or caught by the hub's word rules waits in **Needs you** (Approve / Take down with
a reason). A missing open licence never holds a listing: it installs from the author only.

**Updates.** About hourly, listings that follow a branch or the latest release are asked again. A new rev is checked
like a new listing: clean and allowed by the policy, it replaces the install (players see Update); flagged or failing, it
waits as `pending_update` (Needs you: Approve the update / Keep the old one) while the old install stays live, like
curate.py's `pending_update`.

**Ratings, reports, rankings.** Ratings and Hot / Rising / Top come from `stream/hub_rank.py` (one rating per account,
changeable). Reports take a reason (`broken malware stolen gamedata cheat rude spam other`) and a note; one per account,
never the author's own; HQ shows them with their reasons. **Abuse limits:** 10 checks an hour per account and 400 a day
in all (a check downloads), the Workshop's links-a-day limit (3), one a day for new accounts, site-wide sanctions
(HQ's "no submissions") refuse both check and submit. Authors edit (description, category) and remove their own;
moderators take down.

**API** (`/wrath/api/listings/<action>`, JSON; the launcher's `Authorization: Bearer` token, or the site cookie with
`X-Leyline-CSRF` from `GET /wrath/api/listings/mine`): `check {url}`, `submit {t, category, description, builds?}`,
`report {id, reason, note?}`, `edit {id, description?, category?}`, `remove {id}`; `GET mine`. The page
`/wrath/mods/list` does the same with plain forms.

## Looks (graphics packs, UI looks and graphics presets)

A main section of the site (`/wrath/looks`, owner 1 Nov 14:38) and the launcher (Mods › Discover › Looks). One list,
`stream/looks.json`, edited with `tools/looks/looks.py` (`check`, `list`, `approve`, `unapprove`, `pin`); the rules are
`stream/looks.py` (`problems`). Each entry is an all.json project with `source: "look"`, id `look:<slug>`, and a `look`
object: `type` (`graphics-pack`, `ui-look`, `preset`), `preset` (a launcher graphics preset id: gfx.rs `PRESETS`),
`uses` (curated addons `up:<slug>` a UI look installs), `changes` (words from `looks.CHANGES`), `previews`, `featured`,
`order`, `note`. Only reviewed entries (`review.by`) are served. all.json adds `look.how` and `credits`.

| How it installs | When | What one click does (launcher `looks.rs`) |
|---|---|---|
| preset | `type: preset` | the game's `WTF/Config.wtf`, through gfx.rs (a copy of the file is kept first) |
| graphics pack | an `install` block (listed-mod shape, `kind: files`): the entry's own GitHub release file (`.MPQ` or `.zip`), its sha256, every `.MPQ` in it with its sha256 (`looks.py pin`) | the patches into free `Data/patch-<X>.MPQ` slots (never T, the Den's ground, or Z, the hybrid client's), as `look:<slug>` in `Leyline/mods.json` |
| UI look | `look.uses`, every one reviewed and published in `stream/upstream-addons.json` | each addon from its author (`upstream.rs`); the look remembers which it put in |
| link only | anything else (no licence, no release file, a file host, native code, our checks don't cover that client yet) | opens the author's page |

**Back to the original look** (launcher): removes every look Leyline put in that client (and only those addons it put in
for a look) and puts the game's settings file back as it was before Leyline first changed it.

**Pictures**: only ones we made (`basis: "ours"`) or the author's under an open licence (`basis: "licence"`), served from
our own `/wrath/art/looks/`; otherwise a drawn placeholder and a link to the author's page. Never game data, never a copy
of anyone's pack: nothing is hosted, ever. Words follow the site rules: no brand words, no client numbers (the chips say
which clients, in the hub's era words on the site).

