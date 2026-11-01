# The Builders' Realm

An official realm just for building, open to approved builders (owner, 31 Oct). Its name for now is
**Builders' Realm** (TODO(owner): the final name; change `WU_BUILD_NAME` / `OV_BUILDREALM_NAME`, then run
`build-realm-on` again).

It runs on the main realm's server as a second, small world server. The main realm is never restarted for it.

## For builders

- **Getting in.** You need to be an approved builder: a GM, someone with the builder role on the main realm
  (a GM gave it with `.build grant <name>` there), or an account the team added here (`.buildrealm allow
  <account>`). Sign in with your usual website login: same account, same password. Anyone else is told the
  realm is for approved builders and logged out after a few seconds.
- **Where.** Everyone starts in one big shared room (64 by 64 yards inside its walls, plus a yard outside).
  Press **F7** to build (or type `/builder`). Everything you place is kept at once; nothing is lost if the
  realm restarts.
- **Saving.** `.den save <name>` keeps the whole room as a file. `.den load <name>` opens one again (it asks
  before replacing what is there, and keeps a copy called "backup"). `.den files` lists them.
- **Taking it home.** Save files are Den files: the team copies them to a PC with the `build-realm-saves` step,
  and the launcher's Saved Dens (Add a Den file) opens them in your own Den.
- **Commands.** `.buildrealm` (who may build here), `.den` (the room), `.build help` (every build command).

## For the owner: tonight

Run these in Leyline Control, in this order. None of them restarts the main realm.

| Step | What it does | Time |
|---|---|---|
| `scripts` | Puts the newest realm scripts on the server (a live reload on the main realm, no restart). Only needed if `build-realm-check` says the scripts are older than the Den's save files. | 1 min |
| `build-realm-check` | Read-only. Memory, disk, the main realm, the relay, and what turning it on would do. | 1 min |
| `build-realm-on` | Backup, its own databases, its login and world servers, a self-check (login answers, world greets and stays up, main realm still fine and not restarted), then the Leyline list. Any failure takes everything this run made away again. | 3 to 10 min (the world server loads for 1 to 5 minutes) |
| `signin` | The play page learns that a hop to the Builders' Realm keeps the player's own login (`build-realm.id`). | 2 min |

Then, in game on the Builders' Realm (from the Leyline realm list in the browser game, or the launcher), log in
with your GM account and check: you start in the white room, F7 opens build mode, `.den save test` works.

Other steps:

- `build-realm-off`: off the Leyline list, then stopped. If builders are online they get a 5-minute countdown in
  game first (the world server's own shutdown countdown, which saves everyone). Its databases and save files stay;
  `build-realm-on` brings it back as it was.
- `build-realm-saves`: copies every save to `Documents\Leyline\Builders Realm saves\` on this PC, and sends the
  files you put in its `send` folder to the realm.
- Running `build-realm-on` again (for example after changing `realm/profiles/build.env`) restarts only the
  Builders' Realm, with the same 5-minute countdown for builders online.

### Who may build: the rule

The simplest safe rule: **builders only, nobody else gets in.** The approved builder list is the one that already
exists: the builder role on the main realm (`.build grant <name>` / `.build revoke <name>`, kept in
`mod_builder_roles`). An account with any character on that list may come in. The team can add more accounts on
the Builders' Realm itself: `.buildrealm allow <account>`, `.buildrealm deny <account>`, `.buildrealm list`.
Every approved builder gets the builder role on each character there as they log in; it is checked again at every
login, so taking the role away on the main realm counts at their next login.

To let everyone else look around (no building), set `OV_BUILDREALM_VISITORS=1` in `realm/profiles/build.env` and
run `build-realm-on` again.

### Publishing a build to the main realm

On the Builders' Realm, a GM types `.buildrealm publish <name>`. The whole room becomes a saved build on the main
realm, with a 6-letter code, and the realm says the code. On the main realm, a GM stands in a build area and types
`.build load <code>`: it is pasted where they stand, facing the way they face, with every check a paste has.
Nothing restarts or reloads. Sizes and tilts don't carry over yet (the main realm's saved builds don't keep them),
and props the main realm doesn't allow (any-object and model props, buildings) are left out unless the main realm
turns those on (`OV_BUILD_ANY`, `OV_BUILD_MODELS`, `OV_BUILD_BUILDINGS` in `main.env`: a main realm restart, so
only with the 5-minute warning).

## How it is made

| Piece | What |
|---|---|
| `ov-ac-build-world` | A second world server from the main realm's own image. The same world database (no script writes to it) and client data; its own characters database `acore_characters_build`; realm id 2; the realm scripts folder read-only; `realm/profiles/build.env`. Capped at 1 GB of memory. No published port. |
| `wu-build-auth` | A login server for it, from the main one's image. Its login database `acore_auth_build` is views of the main login database's tables (the same accounts, passwords, bans and GM rights) with its own realm list holding only this realm, so the main realm's list keeps exactly one realm (the browser game connects by itself only when it sees exactly one). No IP bans (every relay player arrives from the agent; the relay counts wrong passwords per player). 96 MB. |
| `wu-build-agent` | The host kit's agent in "official" mode: the tunnel to the relay on this server and the listing heartbeat (64 MB, read-only, no capabilities). Listed as "Builders' Realm", tag custom, 20 players at most. |
| `ov_buildrealm.lua` | The door: who comes in, the builder role at login, `.buildrealm`. Only on with `OV_BUILDREALM=1`. |
| `ov_den.lua`, `ov_build.lua` | The room and all the building (a big Den: `OV_DEN_SIZE=32`). |
| Files on the server | `/srv/wrath/build-realm/`: `world.env`, `auth.env` (0600), `dens/` (saves), `logs/`, `beacon/state/` (the realm's key, 0600), `backups/` (the last five). `/srv/wrath/portal/build-realm.id` and `/srv/wrath/build-realm.id`: its realm id on the relay. |

Lean settings (build.env): nothing preloaded, idle areas unload after a minute, short sight, one map thread,
no playerbots, no autobalance, no dungeon or arena modules, no events, duels, boosts or Discord lines, 20 players.

**Memory.** Estimated, not measured on the server yet: the world server 0.6 to 0.9 GB (capped at 1 GB: if it needs
more, the self-check sees it run out and rolls back; `WU_BUILD_WORLD_MEM` raises the cap), the login server about
20 MB, the agent about 30 MB, the database a few MB more. About 1 GB in all. `build-realm-check` and
`build-realm-on` print what is free and in use; it needs 1.3 GB available to start (`WU_BUILD_MIN_FREE_MB`).
**Disk**: a few MB for the empty databases, plus the backups (a few MB each).

**Ports.** None new. Players reach it only through the Leyline relay: the browser game hops through the relay's
bridge; the launcher and desktop games use the relay's port pair for this realm (its realm list points there).

## Risks and things to check

- **Not tried on the live server yet.** Tested here against a stubbed Docker and database
  (`hosting/tests/test_build_realm.py`) and a stubbed game engine (`stream/tests/test_ov_buildrealm.lua`).
- **Login through views.** The login server and world server write to the main login database through the
  views (last login, character counts per realm, uptime). The database allows that for simple views; the first real
  login and character creation are the proof. If character creation fails, see `docker logs ov-ac-build-world`.
- **Memory.** If the world server needs more than 1 GB it is stopped by the cap and the step rolls back. Raising the
  cap eats into what the other realm on the server needs.
- **GM rights** for all realms (RealmID -1) count here too; rights given for realm 1 only don't.
- **The room says "Den"** in a few messages (it is a Den underneath).
- **Hops.** The play page needs the `signin` step to give a hop to the Builders' Realm the player's own login.
  The launcher and the in-game realm browser show it from the Leyline list; how the launcher signs in to it is the
  launcher's own flow (to verify).

Tests: `python3 -I hosting/tests/test_build_realm.py`, `lua5.1 stream/tests/test_ov_buildrealm.lua`,
`python3 -I stream/tests/test_hop.py stream/wrath_portal.py`.
