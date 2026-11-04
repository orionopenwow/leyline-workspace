# Client patches (the Wrath Unleashed browser client)

Our changes to the browser client, as patches against benilla-wotlk (pinned commit `b9a49428`).
Moved here from the repo root on 2026-10-30; nothing was deleted.

| File | What it is |
|---|---|
| `wotlk-web-port-wip.patch` | The web port itself (WebGPU build, page bridge, async loader). `../get-source.sh` applies it. |
| `wotlk-client-20261104b.patch` | The newest incremental client change. Nameplates: a plate now stands on its bottom centre two thirds of a yard above its unit's head point, name above the bar, so it floats over the head instead of covering the model (it hung from its top there before), and it is a fixed size in the interface's 768-unit space at 0.8 of the stock frame by default: a 116 px health bar and a 14 px name at 1080p (`vplates.rs`). The new `nameplateScale` CVar (default 1, 0.25 to 3; 1.25 is the stock frame) tunes the size without a rebuild (`cvars/table.rs`). The look is unchanged. Tests: `vplates`. |
| `series.txt` | The apply order, one patch per line: what `../Build browser client.bat` applies. Add each new patch here. |
| `wotlk-smooth-frames.patch` | The smooth-frames work, on top of the web port. Notes: `SMOOTH-FRAMES.md`. |
| `history/wotlk-client-2026102*.patch`, `history/wotlk-client-20261031*.patch`, `history/wotlk-client-20261101a.patch`, `history/wotlk-client-20261104a.patch` | The earlier incremental changes still in the series, 2026-10-26 to 2026-11-04, in name order (`26a`, `26b`, `29a`, `31a`: edit mode's free camera held to the realm's grant, `31b`: fast load, the body released once the ground under it is in, `01a`: the item destroy guard and the stock plate look back as the default, `04a`: plates seated and sized in the window's logical pixels and the world camera re-read on every re-stamp, so plates keep their size under a browser zoom or a display hop). |

Each `wotlk-client-*` patch is a delta on top of the ones before it. To rebuild the client tree by hand, apply
the web port, then `series.txt` in order (history first, then the newest): on a fresh `b9a49428` that is the web
port, then `history/` `26a`, `26b`, `29a`, `31a`, `31b`, `01a`, `04a`, then `20261104b` (checked 2026-11-04 with `git apply`
on the web port's tree).
The older `20261025f` ... `20261025o` patches were folded into the web port (they no longer applied on top of
it, checked 2026-10-31) and were taken out of the repo on 2026-11-02; `git log -- client/patches/history` has them.

New incremental patches go here as `wotlk-client-<date><letter>.patch`. When a newer one lands, move the
older one into `history/` and update `series.txt`.

The dev helpers for the web build (`rebuild.sh`, `smoke.js`, `smoke-serve.py`) are in `../dev/`.
