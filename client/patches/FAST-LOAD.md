# Fast load (client 20261031b)

Today, browser, warm cache (owner's numbers, 31 Oct): character select ~20 s, Enter World ~50-60 s, ~20 s of it on
"Still loading the ground under you". This note says what 20261031b changes, why, and how to measure it after deploy.

## Where the time went (read from the code, not measured yet)

- **The ground wait (~20 s).** The settle hold released the body only when `colliders_pending == 0` (every pending
  collider of the whole streamed window) and, after its stall budget, also `wmo_pending == 0` (every unspawned WMO in
  the 3x3 tiles). In Dalaran that is the city hull (~1 M triangles, ~1 µs each on the one wasm thread, split into
  24k-triangle pieces) plus thousands of prop hulls, built at 12 ms a frame behind the cover. The browser build queue
  was FIFO, so the floor under the player could be among the last built.
- **The cover (~30 s).** It lifted on `presentable()`, the same global collider count, so it also waited for every
  far hull before it could clear on its own (else the 8-20 s stall rules lifted it).
- **The Dalaran files.** The tier prefetch for the selected character started only after the whole interface
  prefetch (up to 160 MB), at low priority.
- **The bounce to character select.** A refused `CMSG_PLAYER_LOGIN` (`SMSG_CHARACTER_LOGIN_FAILED`) clears the pick
  and shows select. The cause is not confirmed; the client now says which (`bounce:refused:<code>`) and retries once.

## What 20261031b changes

| Where | Change |
|---|---|
| `benilla-world` collider.rs, web_budget.rs | Each hull build carries its ground-plane footprint. `body_colliders_pending` counts only hulls within 12 yd of the body (or the entry point before the avatar exists). In the browser those build first (an urgent queue, re-sorted when the body moves 16 yd). |
| `benilla-adt`, `benilla-formats`, terrain_stream.rs | MODF bounding boxes are read; `body_wmo_pending` counts only unspawned buildings whose box reaches the body (unknown box: counts). |
| world_focus.rs, loading_screen.rs | The settle hold releases on `ground_ready_at_body()`; its stall rule gives up only when no hull or building under the body is pending. The cover clears on `presentable_at_body()`. Far hulls keep building behind the reveal; the void guard still freezes a body over a hole. |
| loading_screen.rs | `load:<what>` to the page: `ground`, `buildings`, `models`, `walls`, `finishing`. |
| char_select | A first refusal of a pick is sent again once after 2 s (`entry-retry`); a second refusal stays on select. |
| web/boot.js | An early pass of the focus tier's 600 nearest files (4 at a time, low priority) as soon as character select names the tile; Enter World pushes the nearest unwarmed ones at full priority; the cover and the ground line say what is loading; `window.__wenilla_entry` keeps the entry's timeline. |
| stream wu-loading.js/.css | The page's cover shows that line and fills its bar from the client's own fraction (`cover:<pct>`), no numbers. |

## Measurement plan (main session, after deploy: patch -> `client-build` on the PC -> `client`)

Same browser, same character in Dalaran, warm cache (one full load first), three runs each, old build vs new
(the old one is `www.old` on the server for one swap back if needed). For each run, in the console after the body
is free:

1. `copy(JSON.stringify(__wenilla_entry))`: `coverOffMs` (Enter World to cover off), `groundWaitMs` (time on
   "Still loading the ground"), `worldMs`, and the `waiting on <what>` marks (what the cover waited on, in order).
   The console line `boot: entry timeline: ...` has the same.
2. `copy(JSON.stringify(__wenilla_loadtrace, null, 1))`: `enterToCoverLiftedMs`, the ten longest tasks (shader
   compiles show as pipeline warm, collider builds as `collider build (N triangles)`).
3. The client log lines `loading screen: cleared (... s)` or `lifted after ... with no progress` (which rule lifted
   it) and `settle: ...` (what still held the body).
4. Character select: `boot: world prefetch (early) for ...` appears within a second of the list; then `(full)`.
   `boot: interface prefetch done in N ms`.
5. The bounce: if Enter World ever lands back on select, `__wenilla_bounces` holds `{why}`; the code
   (`refused:<n>`) names the server's reason. A refusal followed by `entering again` and a normal entry is the retry
   working; a bounce with no `bounce:` entry is a dropped connection (look for `Disconnected — reconnecting`).

Expected (estimates, not measured): the ground wait drops from ~20 s to under 3 s (it now waits for a handful of
pieces under the player); the cover clears several seconds sooner in Dalaran (no wait on far hulls); character select
is unchanged in time but Dalaran's terrain and buildings are cached before the click on a first visit.

Watch for: a player walking through a far wall in the first seconds (its hull still building), and anyone falling
under Dalaran (the void guard and the server-side rescue still apply). `?noprefetch=1` turns off every prefetch for
an A/B.
