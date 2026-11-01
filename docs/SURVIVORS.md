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

1. **Pick the address.** A subdomain of ours, without "wow" in it. The placeholder is `survivors.leylinelauncher.com`
   (`hosting/survivors/address.txt`, the first line that is not a comment). TODO(owner): confirm or change it.
2. **DNS.** An A record for that name pointing at the server (the same address as the website, play.leylinelauncher.com after the domain move). The server
   step checks it and stops before touching Caddy when it isn't there yet.
3. **Ship the website part.** The "signin" step (it now carries `stream/survivors.py`). `/wrath/survivors` then shows
   the page with "Coming soon".
4. **Turn on the survivors build** ("setup\Turn on GitHub builds.bat" puts `survivors.yml` in the workspace repo;
   it runs once by itself, or Actions > survivors > Run workflow). It makes the release `survivors-latest`: the
   browser build, the lock with every file's SHA-256, and the upstream host for Windows, each with a `.sha256`.
   Nothing to build or copy by hand: the recorder (step 5) downloads them into `tools\survivors\dist\` and
   `tools\survivors\bin\` (`survivors_fetch.py`), checks every hash, and refuses a build that is not the locked one.
   The first download also writes the build's file list into `stream/survivors/upstream.lock.json`: ship that (the
   server checks the upload against it).
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

**Where it is built: GitHub Actions** (`tools/github-workflows/survivors.yml`, in the workspace repo
orionopenwow/leyline-workspace). wasm-bindgen needs more than 6 GB of memory on this module, more than our build box
gives (it was killed every time there, 1 Nov). The workspace repo is public, so its `ubuntu-24.04` runners have
16 GB; if the repo goes private again its standard runners have 7 GB, which is too tight: then use a larger runner
(`ubuntu-24.04-16core`-style labels on a paid plan) or a self-hosted one for the `dist` job.
The workflow runs on a change to the lock, `build-dist.sh` or `licenses-index.html`, or by hand:
- `dist` (ubuntu-24.04): `build-dist.sh` with the pinned toolchain (`SV_TOOLCHAIN=pinned`: Rust 1.98.1, prebuilt
  wasm32 std) and brotli, `--write-lock`; refuses to publish if the committed lock already lists the build and this one
  differs; survivors-check; then `survivors-dist.tgz` (sorted, time 0, gzip -n) and `upstream.lock.json`, each with a
  `.sha256`, on `survivors-latest`, and the lock as the build artifact `survivors-lock`.
- `windows-host` (windows-2022): the upstream `wenilla-host.exe` and `mpqx.exe` at the same commit
  (`cargo build --release --locked`), as `wenilla-host-windows-x64.exe` / `mpqx-windows-x64.exe` + `.sha256`.

Before the move to CI, the build box compiled the wasm (its hash is the lock's `cargo_wasm` until the first CI run
writes its own); `SV_CARGO_WASM=<wenilla.wasm> build-dist.sh ...` can still pack such a cargo build on a big machine.

Deviations from upstream's own build are written into the lock (`deviations`) by the run that made it: gzip without a
name and time (`-n`) so the hashes don't change from one build to the next, `licenses/` added next to the build, and,
only on a machine that can't download Rust's pinned toolchain, stable Rust with the standard library built from
source. The source is unmodified.

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
| `tools/survivors/survivors_fetch.py` | the owner's PC: the CI release into `dist\` and `bin\`, every hash checked |
| `tools/github-workflows/survivors.yml` | the CI build (release `survivors-latest`) |

Tests: `python3 -I stream/tests/test_survivors.py`, `python3 -I tools/survivors/test_survivors_tools.py`,
`python3 -I hosting/tests/test_survivors_site.py`.

## Open questions

- **Permission statement** (TODO(owner)): what Aethus said about hosting it, in your words, for the page.
- **The address** (TODO(owner)): `survivors.leylinelauncher.com` is a placeholder (the address is config: `address.txt`, nothing else names it).
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
