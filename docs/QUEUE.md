# The queue: server capacity and "grow the network"

Written 3 Nov 2026. Covers the browser queue on the website (built), the realm's own login queue (configured), the
launcher's "Busy · queue N" (built), and the bots stepping back while there is a line (planned, not built).

## The idea (owner, 3 Nov)

One server carries the website, the browser game stream, the relay and the official realm. When it is full, people
should not just wait: the line tells them how to skip it **and** grow the network:

- **Play on your own PC**: get the launcher and drag and drop your game folder onto it. No streaming, no seat.
- **Host your own realm**: the Host pack (`/wrath/host`, docs/HOST-PACK.md).
- **Become a node**: carry some relay traffic (`/wrath/docs/be-a-node`, docs/LEYLINE-MESH.md).
- **Seed a server**: keep a realm going for others (the roadmap's seeded servers). Seeds stay alive through a
  lightweight relay, not a new server.

The browser keeps serving what it serves now: Dalaran, the starting zones and dungeons. The "skip the line / play on
your own PC" nudge shows only when the browser is full, or after a browser session ends. It is never a zone limit.

## The owner's numbers (approved 3 Nov)

| What | Value | Where it is set |
|---|---|---|
| A line starts at | about **250** real players | browser: `WU_PLAY_CAPACITY=250`; realm: `WU_PLAYER_LIMIT=250` |
| Reconnect grace (you keep your place) | **180 s** | browser: `WU_QUEUE_BEAT=180`; realm: `WU_RECONNECT_GRACE=180` |
| Bots while there is a line | step back to a floor, e.g. **40** | not built yet (see below) |

These are the defaults in the code, so nothing needs setting for them to apply. The realm-side ones only apply
after the worldserver restarts (see "Turning it on").

## 1. The browser queue (website, `stream/wrath_portal.py`)

Streamed play (the slice) is what costs the server bandwidth and CPU per player, so the seats are counted there.

- **A seat** is one game account with a streamed session (`mode "stream"`) whose game asked the server for anything
  in the last `WU_PLAY_IDLE` seconds: `/__auth` (the game's data and its realm connection), `/__env.js`, or the
  `/__gate` posts. An account's tabs share one seat. **Own game files** (`/wrath/play?mode=own`) and guest passes
  take **no** seat.
- **Full:** `/wrath/play` still runs all its usual checks first (sign-in, the slice). Then, if every seat is taken,
  it shows the queue page instead of making a session. No session is made, nothing is sent to the realm, and no
  account is marked while you wait.
- **The line** is first come, first served:
  - one place per game account;
  - at most `WU_QUEUE_PER_IP` places per address (IPv6: per /64), because households and cafes share one;
  - at most `WU_QUEUE_MAX` places in all; past that, "the line is full" and the skip-the-line choices.
- **Heartbeat:** the queue page asks `GET /wrath/api/queue?me=1` every 15 s. Without script, it reloads `/wrath/play`
  every 30 s. A place whose page stops asking is kept for `WU_QUEUE_BEAT` seconds (the reconnect grace), then
  dropped.
- **Admission:** when a seat frees, the first in line becomes "ready" and that seat is held for them for
  `WU_QUEUE_HOLD` seconds. Their page goes back to `/wrath/play` by itself, which does every check again before the
  session is made. If a held seat isn't taken in time, that place is dropped and the next person gets the seat.
- **The wait shown** ("about M min") comes from how fast the line moved in the last 30 minutes. Before there is
  enough history, it uses `WU_QUEUE_SEAT_MIN` minutes per seat.
- **Leave the line:** `POST /wrath/api/queue/leave`, same-origin only. Picking own game files, or "End my browser
  pass", also drops the place.
- **Security:** a place is only a turn.
  - The `wu_q` cookie (HttpOnly, Secure, SameSite=Lax, Path=/wrath) holds a random token. It is never a session:
    `/__auth` and `/__env.js` ignore it.
  - It can't move another account's place, because admission looks the place up by the signed-in account.
  - The existing rate limits (`play:` 30 a minute) still apply, and the queue API has its own (`queue:` 90 a minute
    per address).
- **Memory only:** a portal restart empties the line. Open queue pages put their players back in by themselves, at
  the back. Seats are worked out from the sessions file, which already survives a restart.

### What it serves

- `GET /wrath/api/queue` returns `{"capacity", "in_use", "waiting", "wait_min"}`. It is public and has CORS `*`.
- With `?me=1` and the cookie, it also returns `"you": {"state": "waiting"|"ready", "position", "wait_min", "go"}`.
  This answer is no-store and same-origin only.
- `GET /wrath/status.json` carries the same numbers as `"browser_queue"`, beside the realm's own `"queue"`.

### The pages

- **The queue page** uses the Leyline look (`ly_shell`). It shows "You're #N in line · About M min" and updates
  itself. It has a seats line ("240 of 250 browser seats in use · 12 waiting"), "Use your game folder in the browser
  instead" (when own files are on), Leave the line, and the **Skip the line** panel with its four cards (ids
  `q-pc`, `q-host`, `q-node`, `q-seed`).
- **After "End my browser pass"**, a "Play on your own PC" button (`#end-pc`) goes to the launcher download.
- `TODO(owner)` marks the line's heading and the pitch for growing the network.

**Seed a server** links to `/wrath/docs/roadmap#seeded-servers` when the docs have a `roadmap` page (stream/docs.py
`PAGES`). Otherwise it links to `docs/ROADMAP.md#seeded-servers` on the public Leyline repo. **Note:** `docs/ROADMAP.md`
is not in the public export list (tools/leyline-repo/export.py), so that GitHub link is a 404 until one of these
happens:
- a `docs/leyline/roadmap.md` page with a "Seeded servers" heading is added (and listed in `PAGES`); or
- ROADMAP.md is exported.

### Settings (`/srv/wrath/portal.env`)

| Setting | Default | What it does |
|---|---|---|
| `WU_PLAY_CAPACITY` | 250 | Streamed seats. `0` means no line at all |
| `WU_PLAY_IDLE` | 600 | Seconds with no game traffic before a seat counts as free |
| `WU_QUEUE_BEAT` | 180 | The reconnect grace: seconds a silent place keeps its spot |
| `WU_QUEUE_HOLD` | 120 | Seconds a freed seat waits for the first in line |
| `WU_QUEUE_PER_IP` | 4 | Places per address |
| `WU_QUEUE_MAX` | 500 | The longest line |
| `WU_QUEUE_SEAT_MIN` | 30 | Minutes per seat for the first wait guess |

A known limit: a streamed player who stands still for longer than `WU_PLAY_IDLE` makes no requests and counts as gone.
They keep playing, so the server can briefly hold a few more than capacity. It is a soft cap, on purpose: nobody is
ever kicked.

## 2. The launcher (`launcher/ui/app.js`, `src-tauri/src/status.rs`)

- `realm_status` reads two more fields from the official `status.json`: `queue` (the realm's login queue) and
  `browser_queue.waiting` / `wait_min`.
- When the number for the mode you play in is above 0 (the realm's queue when playing online, the browser line in
  BROWSER mode):
  - the official realm row shows a **"Busy · queue N"** badge;
  - PLAY's line says **"BUSY · QUEUE N"**, with "About M min · or skip the line" or "Or skip the line";
  - a small **card** shows "Host your own realm" (the Host page) and "Become a node" (the docs page, the new
    `beanode` link). In the browser it also shows "Play with your game folder", which switches to online play.
    "×" closes the card until the line is gone.
- Test: `launcher/tests/queue.browser.test.cjs` (4 tests). The Rust test `queues_come_from_the_status_file` runs in CI.

## 3. The realm's own login queue (AzerothCore)

The realm runs in `server/docker-compose.prod.yml`. Settings there win over the env files.

- **`AC_PLAYER_LIMIT: "${WU_PLAYER_LIMIT:-250}"`** (was 1000): the most players in the world at once. Bots don't count
  against it. Past it, the worldserver queues logins itself. Players see their place in the game's own queue
  screen, and `server info` says `Queue: N`, which `status.json` already reports as `"queue"`.
- **`AC_DISCONNECT_TOLERANCE_INTERVAL: "${WU_RECONNECT_GRACE:-180}"`** (new): a player who drops can come back within
  180 s without queueing again. The key is AzerothCore's `DisconnectToleranceInterval` (default 0, off). Check that
  the worldserver log doesn't report it as unknown after the restart.
- GMs (security level 1 and up) skip the queue, as AzerothCore always does.

### Bots step back while there is a line (not built)

mod-playerbots has a fixed count (`AC_PLAYERBOTS_MIN_RANDOM_BOTS` / `MAX_RANDOM_BOTS` in `/srv/wrath/modules.env`;
see server/RUNBOOK.md). The plan:
- a small watcher, beside the status refresh, reads `queue` from `server info`;
- while `queue > 0` for a few minutes, it lowers the random bot count step by step to the floor (40);
- when the line has been empty for a while, it raises the count back.

This needs a way to change the count without restarting the world, and that still has to be checked on this
module's version. A restart would drop every player, so **don't** do it with a restart. Until then, lower the bots by
hand if a line forms (RUNBOOK: "As real players arrive (200-300 online), consider lowering bots").

## Turning it on

1. **Website:** the `signin` step (fastloop) deploys `wrath_portal.py`. With no settings, the line starts at 250
   seats. To test it live, set `WU_PLAY_CAPACITY=1` in portal.env for a minute, then remove it.
2. **Realm:** the two compose values apply at the next `docker compose up -d ac-worldserver` in `/srv/wrath` (the
   owner's step: it restarts the world). Don't use `/opt/wenilla-realm`.
3. **Launcher:** ships with the next `ship`.

## Rollback

- **Browser queue off at once:** `WU_PLAY_CAPACITY=0` in portal.env, then restart wu-portal. Everyone gets in as
  before, and the API reports capacity 0.
- **Code:** revert the queue commits on the branch (`stream/wrath_portal.py`). The `signin` step backs up and restores
  the portal by itself.
- **Realm:** set `WU_PLAYER_LIMIT=1000` and `WU_RECONNECT_GRACE=0` in `/srv/wrath/.env` (or revert the compose lines),
  then restart the worldserver.
- **Launcher:** older status files have no `browser_queue`, so the badge simply never shows.

## Tests

- `python3 -I stream/tests/test_queue.py stream/wrath_portal.py` (80 checks).
- The site, hub, realms, layer, hop, gamedata, demo, brand, network and admin suites are unchanged and pass.
- `cd launcher && node --test tests/queue.browser.test.cjs`
