# Client patches (the Wrath Unleashed browser client)

Our changes to the browser client, as patches against benilla-wotlk (pinned commit `b9a49428`).
Moved here from the repo root on 2026-10-30; nothing was deleted.

| File | What it is |
|---|---|
| `wotlk-web-port-wip.patch` | The web port itself (WebGPU build, page bridge, async loader). `../get-source.sh` applies it. |
| `wotlk-client-20261031b.patch` | The newest incremental client change, fast load: the body is released and the loading cover lifts once the ground under the player is in (hulls and buildings that reach the body), not every collider of the streamed window; the hulls under the body build first in the browser; the page's line names what the load waits on (`load:<what>`); a refused first Enter World is tried once more; Dalaran's nearest files start downloading at character select; `window.__wenilla_entry` times the entry. |
| `series.txt` | The apply order, one patch per line: what `../Build browser client.bat` applies. Add each new patch here. |
| `wotlk-smooth-frames.patch` | The smooth-frames work, on top of the web port. Notes: `SMOOTH-FRAMES.md`. |
| `history/wotlk-client-2026102*.patch`, `history/wotlk-client-20261031a.patch` | The earlier incremental changes, 2026-10-25 to 2026-10-31, in name order (`f` ... `o`, then `26a`, `26b`, `29a`, `31a`: edit mode's free camera held to the realm's grant). |

Each `wotlk-client-*` patch is a delta on top of the ones before it. None of them is a copy of another, so
the whole series is kept. To rebuild the client tree by hand, apply the web port, then the series in name
order (history first, then the newest).
Checked 2026-10-31: the web port already holds `25f` ... `25o` (they no longer apply on top of it), so on a
fresh `b9a49428` the working order is the web port, then `history/` `26a`, `26b`, `29a`, `31a`, then `20261031b`
(`series.txt`; checked 2026-10-31 by `bash ../build-browser-client.sh --source-only`).

New incremental patches go here as `wotlk-client-<date><letter>.patch`. When a newer one lands, move the
older one into `history/` and update `series.txt`.

The dev helpers for the web build (`rebuild.sh`, `smoke.js`, `smoke-serve.py`) are in `../dev/`.
