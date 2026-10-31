# Client patches (the Wrath Unleashed browser client)

Our changes to the browser client, as patches against benilla-wotlk (pinned commit `b9a49428`).
Moved here from the repo root on 2026-10-30; nothing was deleted.

| File | What it is |
|---|---|
| `wotlk-web-port-wip.patch` | The web port itself (WebGPU build, page bridge, async loader). `../get-source.sh` applies it. |
| `wotlk-client-20261031a.patch` | The newest incremental client change: the client holds edit mode's free camera (F7) to the realm's grant and refuses it in combat, dead, on a flight or in an instance, whatever `leylineEditMode` says; the golf course editor obeys the same bars. |
| `wotlk-smooth-frames.patch` | The smooth-frames work, on top of the web port. Notes: `SMOOTH-FRAMES.md`. |
| `history/wotlk-client-2026102*.patch` | The earlier incremental changes, 2026-10-25 to 2026-10-29, in name order (`f` ... `o`, then `26a`, `26b`, `29a`). |

Each `wotlk-client-*` patch is a delta on top of the ones before it. None of them is a copy of another, so
the whole series is kept. To rebuild the client tree by hand, apply the web port, then the series in name
order (history first, then the newest).
Checked 2026-10-31: the web port already holds `25f` ... `25o` (they no longer apply on top of it), so on a
fresh `b9a49428` the working order is the web port, then `history/` `26a`, `26b`, `29a`, then `20261031a`.

New incremental patches go here as `wotlk-client-<date><letter>.patch`. When a newer one lands, move the
older one into `history/`.

The dev helpers for the web build (`rebuild.sh`, `smoke.js`, `smoke-serve.py`) are in `../dev/`.
