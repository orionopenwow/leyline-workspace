# Repo map

The Leyline workspace (`orionopenwow/leyline-workspace`, on the owner's PC `Desktop\Leyline`), as of the launch
cleanup on 2026-11-02. Leyline is the mod launcher and the P2P realm network; our own realm is Wrath Unleashed
(3.3.5a). Rules of thumb are at the bottom.

## What ships where

| Product | Source | Built by | Ships to |
|---|---|---|---|
| **Website, portal and hub** | `stream/` (`wrath_portal.py` and the modules beside it, `www-files/`, `fui-web/`), `docs/leyline/` (served at `/wrath/docs`) | nothing to build | our server, by the steps `signin`, `page`, `foreverui` (`stream/fastloop.sh`, `stream/*.bat`) |
| **Browser game** | `client/` (`get-source.sh`, `patches/series.txt` on benilla-wotlk) | `client-build` step (`client/build-browser-client.sh`) into `play/dist-test.tgz.part*` | our server, `client` step (`client-back` undoes it); streamed files by `slice` |
| **Leyline launcher** | `launcher/` (Tauri 2: `ui/` is the bundled page, `src-tauri/` the Rust core) | GitHub Actions `leyline-launcher.yml` in the public repo `orionopenwow/Leyline` | GitHub releases `launcher-v*` + `latest.json` (the self-updater); `tools/leyline-repo/export.py` (step `leyline`) copies it to the public repo |
| **Den Lite** | `labs/den-lite/` (Rust crate, MIT) | `den-lite.yml` (CI), or the `den-lite` step on the PC | release `den-lite-latest`; the launcher downloads it on the first Play solo |
| **Host pack / host kit** | `hosting/kit/`, `hosting/pack/make-hostpack.py`, `hosting/relay/` | `hostkit` step; realm images by `wu-images.yml` (ghcr.io) | the public Leyline release; the launcher's Host page and `/wrath/host` |
| **Leyline addons** | `addons/`, `stream/fui-web/` (WrathUnleashedUI) | `addons` step (`addons/pack/`) | the public Leyline release; the launcher installs them |
| **Forge** (Den ground editor, GPL-3.0, from Noggit Red) | `forge/` | `forge.yml` (Windows) | release `forge-latest` (`Forge.zip` + source tarball); the launcher starts `Forge.exe` |
| **Azeroth Survivors** | upstream build pinned by `stream/survivors/upstream.lock.json`; `tools/survivors/` (recorder) | `survivors.yml` (`tools/survivors/build-dist.sh`) | its own address, `sv-site` step (`hosting/survivors/`); page `stream/survivors.py` |
| **Realm** (Wrath Unleashed) | `realm/` (AzerothCore in Docker, `custom/` modules and Lua) | `wu-images.yml`; Lua by `realm-scripts.yml` | our server: `scripts`/`realm` steps, live Lua reload from CI |
| **Mods catalog** | `mods/` (`SPEC.md`, `wu-mod.py`, `catalog/`, `template/`) | `mods` step | realm, streamed addons and `/wrath/mods` |
| **GM console** | `gmconsole/` | `gm` step | our server (owner only) |

## Top level

| Path | What it is for |
|---|---|
| `launcher/` | The desktop launcher (see above); `launcher/README.md` |
| `hosting/` | The P2P/relay network and one-click hosting: `kit/` (the `leyline` command, compose files, host agent), `relay/`, `pack/`, `discord/` (bridge), `hangout/`, `vanilla-remastered/`, `build-realm/`, `survivors/`, `mesh-sim/` (experimental), `site/` (join page), `tests/` |
| `stream/` | The website and the browser game page layer; `server/` holds the scripts that run on our server; `tests/` the portal, hub, page and Lua tests |
| `client/` | Our browser client: `get-source.sh`, `patches/` (web port, the series, smooth frames; `history/` the older deltas still in the series), `dev/`, the build |
| `realm/` | The Wrath Unleashed realm: Docker, `custom/` (modules, patches, Lua scripts), profiles, SQL, brand and sampler switches, server checks |
| `mods/`, `addons/` | The mod format, checker and catalog; our in-game addons and their pack |
| `forge/` | Forge, its own GPL program (`forge/README.md`); `forge/src/external/` is vendored third-party code |
| `play/` | Local play on the PC: `Play locally (one click).bat`, `Install Forever UI.bat`, `Play 3.3.5a in browser.bat`, `Play test build.bat` and `Play Azeroth Survivors.bat` (the newest `dist-test` build) |
| `server/` | Our server's first-deploy kit: Caddy, `deploy.sh`, rollback, backups, sign-up service, `RUNBOOK.md` |
| `gmconsole/` | The owner-only GM console |
| `tools/` | Ops tools, one folder per job. `control/control.py` (`STEPS`) is the control window and Autopilot; `github-workflows/` holds the workflow sources `setup\Turn on GitHub builds.bat` installs; `leyline-repo/` the public export; `cleanup/REMOVE.txt` takes moved files off the PC |
| `setup/` | One-time setup bats (Autopilot at start-up, GitHub builds, launcher updates, automatic deploys) |
| `docs/` | Feature docs and plans; `docs/leyline/` is the public player and host guide; `docs/posts/` launch post drafts; `docs/smoke/` smoke reports |
| `design/` | Design sources: theme tokens (`make_tokens.py`), the brand master art (`brand/`), never bundled |
| `labs/` | Kept for later, not run by Autopilot, CI or the export; except `den-lite/` (live, see above) and `done/ov-rename/` (the `ovsite` step uses its script). `labs/README.md` |
| `.github/workflows/` | CI that runs in this repo (hosting tests, realm images, hub index, scout); sources in `tools/github-workflows/` |
| not in git | `control/` and `logs/` (Autopilot state and logs), `.repos/` (public repo working copies), `.launcher/`, `site-art/` (owner screenshots: never committed), `play/dist*` builds |

## Root files

`README.md`; `Leyline Control.bat` (the control window); `UPDATE.bat` with `UPDATES-PENDING.txt` and
`UPDATES-DONE.txt` (the update queue); `Ship to GitHub.bat` (step `ship`); `Ship the Leyline launcher.bat`;
`Open Leyline Launcher.bat`; `.gitignore` (game data, builds, secrets, machine state, caches).

## Rules of thumb

- A file an Autopilot step runs (`STEPS` in `tools/control/control.py`, `UPDATE.bat`) stays at its path; moving it
  means updating `control.py`, the bats that call it, `stream/fastloop.sh`, the workflows and `export.py`
  (`tools/control/test_step_paths.py` checks the step paths).
- The launcher and the site each keep their own copy of the shared art (`launcher/ui/art/`, `stream/www-files/art/`):
  the launcher works offline and the tests check the copies match.
- Public: only what the `ALLOW` list in `tools/leyline-repo/export.py` names goes to the public Leyline repo (the
  launcher, the host kit, the relay and their docs, `docs/leyline/`). Never `labs/` or ops tools.
- Never in the repo: game data, build outputs, secrets, the owner's in-game pictures.
- New experiments go in `labs/<name>/` with a row in `labs/README.md`. Removed files come back from git history.
