# Solo on this device (the phone's solo play, no server)

Owner, 5 Nov 2026: "continue to develop and work on our version of the mobile on the phone option and solo play on
the phone without a server, cleverly simulated and replicated 1:1 place ... create own, package well, polish with our
aesthetic, make it optimized".

**What it is.** The browser game, with its server inside the same page. Den Lite (`labs/den-lite`, our own 3.3.5a
server: SRP6 login, the realm list, the world, the companion, the Dojo's combat, classes, gear, building) is built to
WebAssembly and runs in a Web Worker. The client's sockets are fake ones that talk to that worker. After the page has
loaded, nothing goes to a game server: no realm, no relay, no login server. The client still streams the approved
slice's game files from our page, as it always does.

Status (5 Nov, branch `agent/phone-solo`):

| Part | State |
|---|---|
| Den Lite's transport (`src/wire.rs`), sessions over any byte stream | written; `wire.rs` built and unit-tested on its own |
| Den Lite in the page (`src/web.rs`, bin `den-lite-web`, wasm32-wasip1) | written, type-checked against stubs; **not compiled yet** (see "Not built here") |
| The page side (`stream/www-files/solo/`): ring, WASI, worker, fake sockets, saves | built and tested (node + Chromium) |
| The whole way in through the page side (login, realm list, world auth, character list, enter world, ping) | tested end to end against a stand-in server; CI runs the same tests against the real wasm |
| Entry points: the site's realm list, the phone app | built and tested |
| CI build + release of the wasm, the Control step that fetches it | written and tested (workflow tests) |

## How it fits together

```
 game page (cross-origin isolated)                          Web Worker (solo/den-lite-worker.js)
 ┌──────────────────────────────────────────┐               ┌─────────────────────────────────────────────┐
 │ client (benilla wasm)                    │               │ Den Lite (den-lite-web.wasm, wasm32-wasip1) │
 │   new WebSocket('{page}/ws/3724')  ──┐   │   ring (SAB)  │   web.rs: pump() <- ly_recv  (Atomics.wait) │
 │   new WebSocket('{page}/ws/8085')  ──┤   │ ────────────> │     login::session / world::session         │
 │                                      v   │  OPEN/DATA/   │     over PageWire (wire::Wire)              │
 │ solo.js: SoloSocket (fake WebSocket) │   │  CLOSE/STOP   │   companion / Dojo beats from its own clock │
 │          Bus (one worker, one ring)  │   │               │                                             │
 │          Store (IndexedDB)  <────────┼───┼── postMessage │   ly_send -> {t:'d', c, b} (transferred)     │
 │                                      └───┼── <───────────│   solo-wasi.js: in-memory /dens, clocks,    │
 └──────────────────────────────────────────┘   files/drain │     random, sleep, stdout -> console        │
                                                            └─────────────────────────────────────────────┘
```

1. **Asking for solo.** `?solo=1`, `?go=solo` (`?go=solo:phone` is what the site sends; `go_param` in
   `stream/wrath_portal.py` carries it through sign-in to the game page), or `env.solo` (`"phone"`). `?solo=0` never.
2. **wrath-early.js** (its `leyline-solo` block, before the hop block): checks the browser can (a Worker,
   SharedArrayBuffer, a cross-origin-isolated page; the game page already is one for `sections.js`), makes the env this
   page's own (no hop) and writes `solo/solo-ring.js` + `solo/solo.js` into the page right after itself, so WebSocket is
   replaced before the page's module script (the client) runs. A browser that can't: one line, then the page plays on
   the realm as before. `window.__wuSoloEarly` = `{wanted, ok, why}`.
3. **solo.js `install`**: the env signs the client in to the page's own Den Lite with the device's account (`SOLO` /
   `den`, Den Lite's fixed local password) and seats the ready-made character (`char`: no login form and no character
   list, as the guest pass does). The worker starts at once, while the client downloads (a fast start). Only sockets to
   this page's own `/ws/<port>` are solo; any other WebSocket is a real one.
4. **The fake socket** behaves as a WebSocket does for the client: `open` before any message (what the server says
   first is held until then), binary messages one per server write, `binaryType`, `on*` and `addEventListener`,
   `close` both ways, `instanceof WebSocket` holds. The client reassembles its stream as it does from wu-play.
5. **The ring** (`solo-ring.js`): one SharedArrayBuffer, the page writes, the worker reads. Den Lite's loop blocks the
   worker (`Atomics.wait`), so its event loop never runs and a postMessage could never reach it: that is why the
   client-to-server way is shared memory. Records never split; a full ring keeps the rest in the page (a backlog) until
   the worker says it drained. The other way is postMessage with the buffer transferred (no copy).
6. **Den Lite in the worker** (`labs/den-lite/src/web.rs`): no threads, no sockets. Each new connection runs its whole
   session at once (login on 3724, world on any other port) over a `PageWire`. When a session waits for bytes, its read
   calls `pump()` again, so the world connection runs inside the login connection's wait, and the companion's (500 ms)
   and the Dojo's (50 ms) beats keep their clocks there. The same `login::session` / `world::session` code the PC runs.
7. **Files** (`solo-wasi.js`): a small WASI preview 1 with an in-memory file system. `/dens` is Den Lite's Dens folder.
   What changes there (the Den's autosave, the class and looks, the key bindings the game uploads) is sent to the page,
   which keeps it in IndexedDB (`leyline-solo`), and handed back to the worker at the next start. The events log is not
   kept and is cut to its newest half past 256 KB.
8. **Hidden page**: the worker is paused (no beats, no CPU) until the page is shown again. **Leaving the page**: a STOP
   record; Den Lite saves and ends.
9. **Saves**: `window.__wuSolo.exportSave()` (a JSON file: `{format: "leyline-solo", version: 1, files}`, the Dens folder
   only), `importSave(text)` (replaces this browser's solo world; paths outside `/dens/` are refused),
   `reset()`. `leyline:solo` events on `window` (`installed`, `ready`, `saved`, `failed`) for the touch layer and
   the page; nothing depends on them.

### Where the world is

Den Lite's own Den spot is the GM island, which is **not in the streamed slice**, so in the page it starts at
Northshire Abbey (map 0, the human start; the slice's Northshire Valley tier). `DEN_LITE_SPOT="map x y z"` in the
worker's env moves it (solo.js `SPOT`). Den Lite reads no game files: its tables are our own or from AzerothCore's
open database (`creature_models.bin`: `tools/gen_creature_models.py`; the stamps, places and modes are our JSON and
Lua). The worker never fetches game data. The client streams the same approved slice it always does.

### Entry points

- **The site's realm list** (`account_page`): "Solo" card, **Play solo on this device**:
  `/wrath/play?mini=mini&go=solo:phone` (sign-in and the streamed session as for Play now; the game page plays solo).
- **The phone app** (`launcher/ui/phone.js`, `index.html` `#ph-solo`): **Play solo on this phone** opens the same address
  in the app's web view; no WebGPU in the web view: the same Open in Chrome line as PLAY. No Rust change was needed in
  `mobile.rs` (the address is the site's).
- **The touch layer** (`touch.js`, another agent's) is untouched: solo is the same page and the same client, so the
  touch controls work as they do on the realm. It may listen for `leyline:solo` events.

## Tests

| Test | What |
|---|---|
| `stream/tests/solo.node.test.cjs` | SHA-1 / HMAC / RC4 vectors, SRP6 both ways, the ring (order, wrap, full, drain), the worker's file system (create, append, rename into place, list, remove, the log cut), save copies, the wrath-early hook, and **the whole way in through the real `den-lite-worker.js` in a node worker thread and solo.js's fake sockets** |
| `stream/tests/solo.browser.test.cjs` | Chromium, a phone-sized cross-origin-isolated page like the game's with `?go=solo`: through wrath-early.js into the world, not one socket to a server, other sockets real, a hidden page pauses the worker, saves kept across reloads, a save copy restored, a bad one refused |
| `launcher/tests/phone.browser.test.cjs` | the phone app's Play solo button (and its no-WebGPU line) |
| `stream/tests/test_bossfights.py` | `go_param` carries `solo:phone` (and nothing like it) |
| `tools/github-workflows/test_workflows.py` | the den-lite-web workflow (order: Den Lite's tests, the wasm, the solo tests against it, then the release) |
| `tools/solo/test_solo_wasm.py` | the Control step's checksum and wasm magic checks |

`stream/tests/solo/wire335.js` is a JS test client for the 3.3.5a login and world handshakes (both sides, synchronous
SHA-1 so it runs in a worker), `stub-engine.js` a stand-in server on the same engine contract as Den Lite's wasm.
With `DEN_LITE_WASM=<file>` (or the wasm in `labs/den-lite/target/wasm32-wasip1/release/`) both page tests run against
**Den Lite itself** instead; CI does exactly that.

### Numbers (5 Nov, this container, stand-in server)

- Worker ready: 62 ms after install (Chromium), 106-123 ms (node).
- In the world: 75 ms from the client's first socket (Chromium, phone viewport), 139 ms (node), through SRP6, the
  realm list, the world auth with its header encryption, the character list and enter world.
- Ring: 1 MiB shared; records never split; one copy each way (into the ring, out of the worker by transfer).
- Den Lite's wasm size, memory and its own enter-world time: **not measured yet** (not built here). CI prints the size
  (`ls -la`); the node test prints the real enter-world time when it runs against the wasm. Release profile is
  `opt-level = "s"`, thin LTO, stripped. A first guess from the crate (the message crates are large): 3-6 MB raw,
  ~1-2 MB brotli. If it is too big, `wasm-opt -Oz` and dropping unused message types are the next steps.

## Not built here, and what CI does

This session could not download crates (crates.io) or the wasm32 standard library (static.rust-lang.org), so Den Lite
was not compiled. What was checked: `wire.rs` compiled and its tests pass with plain `rustc`; `web.rs` type-checks
against stubs of the crate's types. The edits to `login.rs`, `world.rs` and `lib.rs` are type renames
(`TcpStream` -> `wire::Stream`, which has the same methods), `start()` split into `build_shared()`, and the two ticker
bodies made functions (`companion_step`, `dojo_step`). The native build's `Cargo.lock` is unchanged (the wasm build is a
feature and its own bin with `required-features`).

`den-lite-web.yml` (CI) runs Den Lite's own tests first, so a mistake in those edits stops there, before anything is
released. Then it builds the wasm with the WASI SDK as its C compiler (zlib, through flate2), runs both solo tests
against it, and puts `den-lite-web.wasm` + `.sha256` on `den-lite-latest`. **The same `cargo test` also runs in
`den-lite.yml`, which releases the PC programs: if the edits broke the native build, that release stops too (nothing
half-built ships), but it means the PC Den Lite isn't updated until it is fixed.**

## Gaps (honest)

- **Not compiled yet** (above). Expect a round of small compile fixes in CI.
- **"1:1"**: solo is Den Lite's world, not the realm's: the ready-made character, the companion, the Dojo's dummies and
  sandbox combat, classes and specs, gear looks, mounts, building, `.go` to the 221 places. **No quests, no loot, no
  experience, no realm creatures**: Den Lite has none yet. The terrain and the city around you are the real slice (the
  client streams it), but the area name the server sends is still the Den's (`AREA` in `world.rs`), and nothing
  populates Northshire. Creatures from the realm's spawn tables would need our own spawn data for the slice (from
  AzerothCore's open database, like `creature_models.bin`), plus their AI and loot: a real next step, not done.
- **Offline**: the solo layer itself needs no network once loaded, but **the game page does not start offline yet**: it
  needs its session (`/__env.js`, `/__gate`) and the slice's files. `wrath-sw.js` keeps world data, not the page, the
  client's sync reads, `DBFilesClient` or `Interface` (by design there). "Download this section" (`sections.js`) keeps
  whole tiers, which covers most of the files. Next: a solo page on the game origin that needs no session (its env
  written by solo.js), the page and `solo/*` in the service worker's cache, and a downloaded Northshire tier.
- **Change realm** from the character list would wait (sessions run inside each other; the outer login connection is
  only read again when the world one closes). There is one realm, so the client has no reason to.
- **The Android web view**: SharedArrayBuffer there depends on the web view honouring cross-origin isolation. Not
  verified on a device. If it doesn't, the page says so in one line and plays on the realm; Open in Chrome is the way.
- **Memory on phones**: the worker holds Den Lite's heap plus the Dens folder in memory (kilobytes to a few MB). The
  client is still the big one.
- **Save copy UI**: `exportSave` / `importSave` / `reset` exist on `window.__wuSolo`, but no button calls them yet (the
  game page's dock and the touch layer belong to other work; a small panel there is the next step). TODO(owner): the
  words for that panel.

## Research and references (read, not copied)

| Project | Licence | What we took |
|---|---|---|
| [marcocastignoli/wow-mobile](https://github.com/marcocastignoli/wow-mobile) (the owner's pointer) | LGPL-2.1 | Nothing in code. It is a Winlator fork (Wine + Box64) that runs the player's own 3.3.5a client on Android against a remote realm, with ConsolePortLK touch bindings. Our approach differs: no emulation, the browser client, and the server in the page. Its lesson for us: a first launch that sets up bindings and config by itself. |
| [bjorn3/browser_wasi_shim](https://github.com/bjorn3/browser_wasi_shim) | MIT or Apache-2.0 | The idea of a small JS WASI with an in-memory file system and preopened folder. Our `solo-wasi.js` is our own (written from the WASI preview 1 definitions) because Den Lite blocks in its worker and needs change notifications for IndexedDB; this one would also have been fine to vendor. |
| [WebAssembly/WASI](https://github.com/WebAssembly/WASI) preview 1 (witx) | Apache-2.0 / W3C | The ABI: struct layouts, errno values, rights. |
| [alexmojaki/sync-message](https://github.com/alexmojaki/sync-message) | not checked | The known pattern for a worker that must block for the page's messages: Atomics on shared memory, or a service worker channel as the fallback where shared memory is missing. We use the Atomics way; the service worker way is the fallback to build if the Android web view lacks shared memory. |
| [gtker/wow_messages](https://github.com/gtker/wow_messages), [gtker/wow_srp](https://github.com/gtker/wow_srp) | MIT or Apache-2.0 | Already Den Lite's dependencies (the message formats, SRP6, the header encryption). `wire335.js` was checked against what they do. |
| [azerothcore/azerothcore-wotlk](https://github.com/azerothcore/azerothcore-wotlk) | GPL-2.0 (AGPL parts) | Reference only, as before for Den Lite: the login order, `playercreateinfo` for the Northshire start. No code. |
| wowdev wiki (SRP6, packets) | CC | Reference for the test client. |

Never used: wowdecomp, Waystone, decompiled clients, leaked builds.

## Packaging

**Web**: `stream/www-files/solo/` is our own code (MIT like the rest of the page layer). `den-lite-web.wasm` is not in
git (`.gitignore`): CI builds it, Control's `solo-wasm` step downloads it into `stream/www-files/solo/` (checksum and
wasm magic checked), and the `page` step puts all of `solo/` on the game page (`www-patch.py` `install_solo`). Without
the wasm the page still works; `?go=solo` then fails to start the worker and the bus reports it
(`__wuSolo.status().failed`).

**Android**: nothing to package in the APK: the app opens the site's address and the page brings its own server. A
later offline APK could carry the page layer and the wasm in its assets (Tauri serves them from the app's own
origin, where the app can send COOP/COEP itself), but the client and the slice would still come from our page.

Owner presses, in order:
1. `setup\Turn on GitHub builds.bat` (puts `den-lite-web.yml` in `.github/workflows/`; double-click, not a step).
2. `ship` (the branch's commits to GitHub: CI builds and tests Den Lite and the wasm).
3. `solo-wasm` (once CI is green: the wasm into `stream\www-files\solo`).
4. `page` (the game page with `solo/`), and `signin` (the site's realm list card and `go=solo`).
5. Try it: Play solo on this device on the site's realm list, and Play solo on this phone in the app.

## Sky Island (next, not built)

The owner's next idea: a ~60-yard floating island with a big oak, opening at once from a static page (no log in, no
character select), with combat and the build tools, and a one-click friend invite (WebRTC, the relay for the first
handshake). It sits on this:

1. **The world is a Den save.** Den Lite opens `/dens/autosave.den.json` as its whole world; Sky Island is a shipped
   `.den.json` (the island's pieces, the oak) put into `/dens/` by the page before the worker starts (the Bus already
   hands the worker its files), with `DEN_LITE_SPOT` set to the island's middle. Bounded by the Den's sizes
   (`.den size`, up to 512 yards).
2. **A local player with no login.** Today the page fills in the login and the character for the client (env
   `user`/`pass`/`char`), so the player sees neither the login form nor the character list; SRP6 still runs, in the page,
   in a few milliseconds. A true no-handshake path would need the client to skip it too (a client change: a "local"
   env flag that opens the world socket directly with a session key the page gives both sides). The server side door:
   `web.rs` could register that key in `Shared::sessions` before the world socket opens.
3. **A friend over WebRTC.** The transport is `wire::Wire` on the server and the record ring on the page. A friend's
   page is one more producer: a data channel whose messages become OPEN/DATA/CLOSE records for a new connection
   number (`Bus.connect` for a remote socket), with the relay only for the first handshake. Den Lite already does
   friends (guests.rs: their own logins, one Den, everyone sees each other), so the host's page runs the world and the
   friend's page runs only the client.
