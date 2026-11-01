# Azeroth Survivors on Leyline

Owner 2026-11-01 ("number 2"): bring Azeroth Survivors to Leyline as its own browser page, with tools you run on
your own PC to pull its files from your own copy of the game.

What it is: a survivors-style roguelike fan game, one player, no server. Pick a hero and a zone, hold out against the
waves. The original game is by Aethus; we host the **open rebuild** by OutThisLife
(<https://github.com/OutThisLife/azeroth-survivors-re>, MIT or Apache-2.0), built **unmodified** at the commit in
`stream/survivors/upstream.lock.json`.

Credit (on `/wrath/survivors` and `/licenses/`): "Azeroth Survivors by Aethus. Open rebuild by OutThisLife, MIT or
Apache-2.0." The permission statement is **TODO(owner)** (`PERMISSION` in `stream/survivors.py`; empty = not shown).

## How it fits together

```
play.openvanilla.app/wrath/survivors        the page (stream/survivors.py): credit, Play / Coming soon / Resting
        | Play (signed in)                  a survivors-mode play session + one-time code
        v
https://<address>/__gate?code=...           wu-portal swaps it for the wu_play cookie on that address, lands on /
https://<address>/...                       Caddy (hosting/survivors/survivors.caddy): forward_auth
                                            /__auth?site=survivors on everything but /licenses/
        v
sv-play (hosting/survivors/docker-compose.yml)   our wenilla-host --www <build> --patch <slice> --only-listed
        /                the upstream build (index.html, wasm, scripts), file for file as the lock says
        /data/<name>     the slice: only the files the game asked for while you recorded; anything else 404
        /licenses/       the upstream licence files and a short credit page (public)
```

- The game reads `GET {origin}/data/<name>` and `/data/__index`, with the base hard-coded to the page's own origin.
  That is why it needs **its own address** and can't live under a path of play.openvanilla.app.
- No game account, no realm: a Survivors session is its own kind ("SV-" + your login, no password). It never opens the
  Wrath stream, and a Wrath session never opens Survivors.
- The game data killswitch (`stream/server/gamedata-switch.sh off`) covers it: Caddy answers 410 for `/data` and the
  manifests and shows a short "resting" page for everything but the licences; `--park` moves its slice away; the page
  on the website says Resting. `on` undoes it.
- Not deployed yet. The catalog entry (`stream/playmodes.json`, id `azeroth-survivors`) stays **coming**; the page shows
  Play as soon as `/srv/wrath/portal/survivors.url` exists (sv-site-up.sh writes it last, after its self-check).

## Your steps

1. **Pick the address.** A subdomain of ours, without "wow" in it. The placeholder is `survivors.openvanilla.app`
   (`hosting/survivors/address.txt`, the first line that is not a comment). TODO(owner): confirm or change it.
2. **DNS.** An A record for that name pointing at the server (the same address play.openvanilla.app has). The server
   step checks it and stops before touching Caddy when it isn't there yet.
3. **Ship the website part.** The "signin" step (it now carries `stream/survivors.py`). `/wrath/survivors` then shows
   the page with "Coming soon".
4. **Put the browser build on your PC.** Copy the build into `tools\survivors\dist\` (the lead hands it over; it is
   not in git). The recorder checks it against the lock and refuses anything else.
   Also needed there: `tools\survivors\bin\wenilla-host.exe`, the upstream host at the locked commit. If it is
   missing, the recorder builds it (needs git and Rust; a few minutes, once).
5. **Record the game's files, on your PC, from your own client.** Run `tools\survivors\Record Survivors files.bat` and
   drag your 1.12 client's `Data` folder in when it asks (remembered after). It starts the upstream host on
   127.0.0.1 only, opens a fresh Edge window and plays the game's own autopilot: every zone once with a full crowd,
   every class once with all its spells, the results screen (about 45 minutes; keep the window visible). Then it asks
   for one pass by hand: on the pick screen click every race, both genders and every class, start a run, then press
   "Done with this pass". It exports exactly the files the game asked for to `tools\survivors\out\slice` and writes
   `names.sha256`. Nothing is uploaded. `... quick` is a 5-minute try-out.
6. **Verify, still on your PC.** `Record Survivors files.bat verify` plays the slice alone through the same host the
   server runs (`stream\bin\wenilla-host.exe --only-listed`). Anything still missing lands in
   `out\verify\missing.txt`; run step 5 again and it adds those. The franchise logo is left out on purpose and shows up
   there; that one is expected.
7. **Upload and turn on.** `hosting\survivors\Put Survivors on Leyline - check.bat` first (read-only: DNS, what is
   ready). Then `Put Survivors on Leyline.bat`: survivors-check on your PC, the upload, survivors-check again on the
   server, sv-play, the Caddy site, a self-check, and only then Play on `/wrath/survivors`. It rolls back if any step
   fails. `noupload` re-serves what is already there.
8. **When you are happy:** set the catalog entry to `"status": "live"` in `stream/playmodes.json` (and its tests) so the
   Modes page and the launcher show it as playable. Until then it says Coming soon there, and the page has Play.

Off again: `Put Survivors on Leyline - off.bat` (Play off first, then the site out, sv-play down; the files stay).

## The browser build

`tools/survivors/build-dist.sh <work dir> --write-lock` (Linux): clones the upstream repo at the locked sha, refuses
local changes, and runs only upstream's wasm build steps (`scripts/web-setup.sh`'s tools and `scripts/web-build.sh`'s
commands). It never runs `make build`, `make native`, `make fetch-data`, `scripts/setup.sh` or `scripts/fetch-data.sh`
(those download the game client). It adds `licenses/` (upstream's licence files, its vendored kira and Lua licences,
and `tools/survivors/licenses-index.html`) and writes every file's SHA-256 into the lock.

**Status (1 Nov):** the cargo step is done (the wasm before wasm-bindgen: `cargo_wasm` in the lock, 165 MB). The
packing is not: wasm-bindgen needs more than 6 GB of memory on this module and the build box gives a session 6.27 GB,
so it was killed every time, even with nothing else running. The lock's `dist` is empty until a machine with about
8 GB free runs `SV_CARGO_WASM=<that wenilla.wasm> bash tools/survivors/build-dist.sh <work dir> --write-lock` (it checks
the file against `cargo_wasm`, then packs) or the whole script. Until then survivors-check refuses every dist ("the
lock lists no dist files yet"), so nothing unlocked can be recorded with or uploaded.

Deviations from upstream's own build are listed in the lock (`deviations`): the build box could not download Rust's
pinned toolchain, so it used the installed stable Rust with the standard library built from source; and gzip
without a name and time (`-n`) so the hashes don't change from one build to the next. The source is unmodified.

## Files

| Path | What |
|---|---|
| `stream/survivors.py` | `/wrath/survivors`, `/wrath/survivors/play`, `/__auth?site=survivors` |
| `stream/survivors/upstream.lock.json` | repo, sha, toolchain, deviations, every dist file's SHA-256 |
| `hosting/survivors/` | `sv-site-up.sh`, `survivors.caddy`, `docker-compose.yml`, `address.txt`, the three .bat files |
| `stream/server/gamedata-caddy.py`, `gamedata-switch.sh` | the killswitch's Survivors block (`add-sv` / `remove-sv` / `status-sv`), parking its slice |
| `tools/survivors/survivors_rec.py` | the logging proxy and the autopilot plan (loopback only) |
| `tools/survivors/survivors_export.py` | recorded names to the slice + `names.sha256` (from the host on your PC, or `--mpqx`) |
| `tools/survivors/survivors_check.py` | survivors-check: slice vs `names.sha256`, dist vs the lock |
| `tools/survivors/Record Survivors files.bat` | record, export, check; `quick`; `verify` |
| `tools/survivors/build-dist.sh`, `licenses-index.html` | the browser build and its licence page |

Tests: `python3 -I stream/tests/test_survivors.py`, `python3 -I tools/survivors/test_survivors_tools.py`,
`python3 -I hosting/tests/test_survivors_site.py`.

## Open questions

- **Permission statement** (TODO(owner)): what Aethus said about hosting it, in your words, for the page.
- **The address** (TODO(owner)): `survivors.openvanilla.app` is a placeholder.
- **The logo**: the slice leaves out `Interface\Glues\Common\Glues-WoW-Logo.blp`. Whether the pick screen misses it
  visibly only shows on your PC (the verify pass). If it does, the fix is in the build, not the slice.
- **Words in the game itself**: the page and our files carry no franchise names, but the game draws its own screens
  from the client's art and tables; check the pick screen and the results screen for any franchise name or logo
  before turning Play on.
- **Coverage**: the autopilot always takes a class's first look and plays one zone per pass; the hand pass covers the
  other races and genders. Rare things (a boss that only comes late, a spell's last rank) may still be missing; the
  game then lacks that one file. The verify pass and a second recording close the gaps.
- **Third-party notices**: `/licenses/` carries upstream's licences and its vendored parts'. The wasm also contains many
  Rust crates (MIT/Apache); a generated list of them (for example with cargo-about) would be more complete.
- **Size**: the slice's size is only known after your recording (the export prints it). The upload is one tarball
  over SSH, not resumable; a very large slice may want the chunked upload `vr-data.ps1` uses.
- **WebGPU only**: the build needs WebGPU (Chrome or Edge on a computer). Upstream's WebGL2 build only shows the glue
  screens, so it is not offered.
