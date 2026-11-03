# labs/: tools and experiments kept for later

Leyline's core today is the launcher, the P2P/relay network with one-click hosting, the website and hub, the
browser game layer with our client patches, the Wrath Unleashed realm, mods, and the Autopilot/ops tools
(see `docs/REPO-MAP.md`). Everything here is **not** part of that core right now, but the owner wants to keep
it for later upgrade and refinement (2026-10-30). Nothing here is run by Autopilot (`tools/control/control.py`),
`UPDATE.bat`, CI or the Leyline export.

Each folder keeps its files as they were (only path fixes where a .bat now sits deeper), moved with `git mv`
(`git log --follow` shows the history).

Taken out in the launch cleanup (2026-11-02; any of them comes back from git history):
`leyline-client-112/` (the private 1.12 client repo maker: paused for good, never used for Vanilla Remastered),
`ov-112-logins/` (the one-time carry-over of 1.12 logins, done) and `players/` (the old tester package,
superseded by the Leyline launcher).

| Folder | What it is | Status | How to revive it |
|---|---|---|---|
| `plans/sandbox-roadmap.md` | The "build anything" sandbox roadmap: Builder, Logic Editor, game-mode kits, GLB importer, player spaces. | A plan, not a commitment. The Builder MVP itself is live on the Hangout (`realm/custom/lua_scripts/ov_build.lua`) and stays in place. Was `docs/sandbox-roadmap.md`. | Move it back to `docs/` when the sandbox is on the roadmap again. |
| `den-lite/` | Den Lite: a tiny local server for the 3.3.5a client (Rust, its own crate, not in the launcher's workspace): a login and a world server on 127.0.0.1, one ready-made character on the Den's open white ground, stamps, undo, the autosave, the game's own key bindings kept and the `leyline-den` save files, for the launcher's Your Den without Docker. | Live (owner 1 Nov 2026): the launcher's Play solo uses it; CI (`den-lite.yml`) builds the release `den-lite-latest` it downloads on first use. Still its own crate here. See `den-lite/README.md`. | `cd labs/den-lite && cargo test` (the test client), `cargo build --release`, then `den-lite --dens <folder> --bind 127.0.0.1`. |

## Experiments that stay where they are

Some prototypes are wired into live scripts, tests or the client build, so moving them would break things.
They stay in place and are listed in `docs/REPO-MAP.md` under "Experimental, in place":
the extra realm profiles (`realm/profiles/brawl.env`, `progression.env`, `casual.env` and their SQL), the mesh
simulator (`hosting/mesh-sim/`), the P2P/torrent and mesh flags (all OFF, `stream/leyline_net.py`), the guest
demo (`stream/demo.py`, OFF), the test-build launchers in `play/` (newest test build, Azeroth Survivors),
and the Hordes of Azeroth work (only on its own branch).

## done/: finished one-shot tools

Tools that did their one job. Kept so the job can be repeated or read later; no Autopilot step points at them any more.

| Folder | What it did | Status | How to rerun |
|---|---|---|---|
| `done/ov-rename/` | `Rename OpenVanilla repo.bat` renamed github.com/orionopenwow/OpenVanilla to `orionopenwow/Leyline-Launcher` (description, homepage, the PC clone's origin); `Land Leyline Launcher branch.bat` landed the `feat-leyline-launcher-rename` branch on its main (fast-forward only). `leyline_rename.py` does both; `test_leyline_rename.py` tests where it finds the clone. | Done 2026-10-29 (steps 1079b `ovrename`, 1079c `ovland`). Was `tools/ov-rename/`; its Autopilot steps were removed on 2026-10-30. | The .bat files work from here (the workspace is three folders up). Rename: run `Rename OpenVanilla repo.bat` (safe to run again). Landing a branch: put its bundle beside the .bat (`feat-leyline-launcher-rename.bundle`, gitignored) and run `Land Leyline Launcher branch.bat`. To make either an Autopilot step again, add it back to `STEPS` in `tools/control/control.py` (but steps must not point into `labs/`: move the folder back to `tools/` first). |

Still in use: `tools/launcher-site/Land Leyline-Launcher site.bat` (the `ovsite` step) runs
`done/ov-rename/leyline_rename.py --land` from here, so keep `done/ov-rename/` until that step is retired or the
script moves to `tools/launcher-site/`.

## Adding to labs/

Put each experiment in its own folder with its files as they were, then add a row above: what it is, its status,
and the exact steps to run it again. Never leave a `labs/` path in `STEPS`, `UPDATE.bat`, a workflow or the
export's `ALLOW` list: anything those need belongs in the core folders.
