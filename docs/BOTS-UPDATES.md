# Bots always up to date (Single player Phase 4)

Owner, 4 Nov: Single player's bots should keep up with mod-playerbots without ever breaking a player's realm.
New upstream bots are built and tested as a **canary** every week. They become the **stable** bots only when the
owner merges the pull request the canary opens. Players then get **Update the bots** on the Single player page.
Restarting is the player's choice and never happens by itself.

## The pins

| What | Where |
|---|---|
| mod-playerbots and its core (the `Playerbot` branch of mod-playerbots' AzerothCore fork) | `realm/custom/sources.lock`, lines `mod-playerbots` and `core` |
| The bots manifest that the Host pack and the launcher read | `realm/bots.json` (`{"format":1, playerbots, core, date, changes}`) |
| The pins of the tested canary and its smoke report | `realm/bots-candidate.json`, written only in the canary's pull request |
| The pins inside the images | labels `org.leyline.playerbots` and `org.leyline.core` on the `wu-ac-wotlk-*` images (wu-images.yml, bots-canary.yml) |

`realm/bots/pins.py`: `show`, `manifest`, `check` (bots.json must match sources.lock; tested), `bump`, `same`.

## The weekly canary (`.github/workflows/bots-canary.yml`)

1. **bump.** It reads the newest commits of the two branches. If they equal the pins, it stops (`force` overrides this).
   It then runs the **compat guard** on the new source and fails in minutes if anything is missing.
   It also regenerates `realm/playerbots-specs.env` from the new `playerbots.conf.dist`.
2. **images.** It builds the realm images the same way wu-images.yml does, tagged `:canary` and `:canary-<commit>`.
   It never touches `:stable`, and no game data enters the build context.
3. **smoke.** It runs the bot smoke test on the canary images.
   - A realm needs dbc + maps from a game folder, and we never ship those. So this job runs only on a **self-hosted runner** with labels `self-hosted, leyline-bots-smoke`. That runner must hold the owner's own extracted `ov-client-data` volume.
   - It runs only when the repo variable `BOTS_SMOKE_RUNNER` is `on`.
   - Without that runner, Control's **`bots-smoke`** step runs the same test on the PC (`realm\Bots smoke test.bat [tag]`).
4. **propose.** The branch `bots/canary` holds sources.lock, bots.json, playerbots-specs.env and bots-candidate.json.
   - Smoke green: a pull request.
   - Smoke not run: a **draft** pull request, titled "smoke test pending".
   - Smoke red: no pull request, and the run fails.
   - It never merges. Commits are made as Orion, using only the built-in token. The repo setting "Allow GitHub Actions to create pull requests" must be on.

**Promoting the canary is the owner merging the pull request.** wu-images.yml then builds `:stable` from main,
labelled with the new pins. Upstream commit messages are never shown to players. Only a count of changes reaches them.

## The compat guard (`realm/bots/compat.py`)

ov_companions.lua has no bot AI of its own. It works only through the following, read from the script itself so
that a new use is covered automatically:

- `.playerbots bot` subcommands (`add`, `addclass`, `init=auto`, `remove`);
- whispers to bots (`follow`, `summon`, `ll`, `talents`, `maintenance`, `autogear`, `co`/`nc`, `formation`, `rti`, ...);
- strategies (`passive`, `cc`, `dps assist`, `mark rti`);
- formations and the premade spec names;
- every `AC_PLAYERBOTS_*` setting in modules.env, profiles/*.env and playerbots-specs.env. A renamed key is silently ignored by the core, which would break, for example, "bots always pass on loot";
- the mod-ale methods it calls. The required set is IsPlayerbot, RunCommand, GetBotAI, HandleCommand, ChangeStrategy, GetMaster, IsAddclassBot, IsTank, IsHealer and Whisper. Other methods have fallbacks in the script and are only noted.

Usage: `python3 realm/bots/compat.py --playerbots DIR [--ale DIR] [--json]`.

On 4 Nov it passed against both the pinned e88cc40 and the then-newest upstream master (05a1f6f).

## The bot smoke test (`realm/bots/bots_smoke.py smoke`, `bots-smoke.sh`)

It needs no game client. It brings up a throwaway realm (`realm/bots/docker-compose.smoke.yml`) with these settings:

- the Single player ruleset (solo.env + modules.env);
- SOAP on;
- no random bots;
- an addclass pool of 5;
- no recruit cooldown;
- ports on 127.0.0.1 only;
- the game data volume read-only;
- only ov_companions.lua (and the dungeons table and modapi), plus the test-only probe `realm/bots/smoke/ov_bots_probe.lua`.

The console's first login is an administrator account written into acore_auth with a random password, the way the
core's `account create` stores it. The console needs an account before it will answer anything, which is why this
first one is written directly. Nothing is printed, and everything is removed at the end. The headless player is
`hosting/kit/tools/wunet/wire335` plus chat, teleport acks and the probe's `OVSMOKE {json}` lines.

The test runs these checks, in order:

| Check | What it proves |
|---|---|
| compat_static | the guard above, when a mod-playerbots source is given |
| console, account | SOAP answers. The test account is made by `account create` and `account set gmlevel ... 3 -1`, with a random password that is never printed |
| character, world | a human warrior is made, enters the world, and the probe answers |
| addclass, remove | mod-playerbots itself: `.playerbots bot addclass rogue` joins our group, then `remove` takes it out |
| dungeon | `.go xyz` to The Deadmines door (from ov_companions_dungeons.lua): we are on map 36 |
| recruit | ov_companions' `.recruit party`: 3 or more bots, on our map and instance, alive, and we are their master |
| compat_live | every required mod-ale method exists on a live bot and its PlayerbotAI |
| follow | after a 15-yard hop, every bot comes within 12 yards |
| pull, fight | a hostile practice mob (entry 38) attacks us: bots enter combat and kill it |
| dismiss | `.dismiss all` leaves the group with no bots |

These are console checks (bot count, group, alive, map and instance, combat, distance). They do not judge how well the bots play.

## Launcher

- `host/bots.rs`, with the commands `bots_status` and `bots_update`:
  - the engine pins come from the image labels, else from `cache/bots-engine.json`, which the last pull wrote;
  - the latest pins come from the Host pack listing's `bots` when that pack is newer, else from the kit's `realm-files/bots.json`;
  - prebuilt images only. When the engine is unknown, nothing is offered.
- `ui/singleupdate.js`, inside the Manage card:
  - **New bots** shows the date chip and **Update the bots**. When the bots come with a newer Host pack, the pack is updated first.
  - It then asks **Restart now** or **Later**.
  - **Bots updated** appears once after the bots change, and **Got it** clears it.

## Tests

- `hosting/tests/test_bots_updates.py`: the guard, the pins, the candidate, the packets, the whole player against a fake realm and a fake SOAP, the compose file, and the script's no-data refusal.
- `realm/bots/tests/test_ov_bots_probe.lua`
- `tools/github-workflows/test_workflows.py` (bots-canary)
- `launcher/tests/singleupdate.browser.test.cjs`
- unit tests in `host/bots.rs`
