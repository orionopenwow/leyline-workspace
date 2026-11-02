# Client patches (the Wrath Unleashed browser client)

Our changes to the browser client, as patches against benilla-wotlk (pinned commit `b9a49428`).
Moved here from the repo root on 2026-10-30; nothing was deleted.

| File | What it is |
|---|---|
| `wotlk-web-port-wip.patch` | The web port itself (WebGPU build, page bridge, async loader). `../get-source.sh` applies it. |
| `wotlk-client-20261101a.patch` | The newest incremental client change. Items: the destroy box asks only for an item visibly on the cursor and still in its slot; an item held with no icon, an icon that did not load, or one the server moved away goes back to its slot instead, and `DeleteCursorItem` never destroys a slot whose item changed (a worn bow was lost this way). Nameplates: the stock look is the default again (red / yellow / green fill under the `Nameplate-Border` art, class colour for enemy players, the target a brighter fill, others dimmed by `nameplateNonTargetAlpha`); in the modern look the flat border no longer draws over the bar. |
| `series.txt` | The apply order, one patch per line: what `../Build browser client.bat` applies. Add each new patch here. |
| `wotlk-smooth-frames.patch` | The smooth-frames work, on top of the web port. Notes: `SMOOTH-FRAMES.md`. |
| `history/wotlk-client-2026102*.patch`, `history/wotlk-client-20261031*.patch` | The earlier incremental changes still in the series, 2026-10-26 to 2026-10-31, in name order (`26a`, `26b`, `29a`, `31a`: edit mode's free camera held to the realm's grant, `31b`: fast load, the body released once the ground under it is in). |

Each `wotlk-client-*` patch is a delta on top of the ones before it. To rebuild the client tree by hand, apply
the web port, then `series.txt` in order (history first, then the newest): on a fresh `b9a49428` that is the web
port, then `history/` `26a`, `26b`, `29a`, `31a`, `31b`, then `20261101a` (checked 2026-11-01 with `git apply`
on the web port's tree).
The older `20261025f` ... `20261025o` patches were folded into the web port (they no longer applied on top of
it, checked 2026-10-31) and were taken out of the repo on 2026-11-02; `git log -- client/patches/history` has them.

New incremental patches go here as `wotlk-client-<date><letter>.patch`. When a newer one lands, move the
older one into `history/` and update `series.txt`.

The dev helpers for the web build (`rebuild.sh`, `smoke.js`, `smoke-serve.py`) are in `../dev/`.
