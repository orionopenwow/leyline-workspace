# Editing the WrathUnleashed client

Three ways to change the client. The first two need no build at all.

| What you change | Where | How it shows up |
|---|---|---|
| UI, frames, Lua, addons | your WoW install's `Interface\AddOns\` (e.g. `OVArmory`, `ForeverUI`) | `/reload` in game, or reload the page |
| Any game file: DBC tables, textures, stock FrameXML/GlueXML | `WrathUnleashed\patch\<game path>` | reload the page (never cached) |
| The engine itself (Rust: rendering, networking, the Lua API) | the client source, below | a new build in `play\dist` |

## Engine changes (Rust)

1. `bash get-source.sh` (Git Bash or WSL) clones benilla-wotlk at the pinned commit and applies
   `patches\wotlk-web-port-wip.patch` onto branch `wrathunleashed`; the later
   client changes are in `patches\` (see `patches/README.md`).
2. Edit, then either:
   - **build on your PC** (Linux/WSL, ~16 GB RAM): `bash scripts/web-setup.sh` once, then
     `bash scripts/web-build.sh`; copy `web/dist` into `WrathUnleashed\play\dist`; or
   - **let GitHub build it:** push the branch to your own fork with `github/web-build.yml`
     copied to `.github/workflows/`. Each push produces `client-dist` and `wenilla-host`
     artifacts to download.
3. Or ask Claude: it builds in the cloud and drops `play\dist\wasm.part.*`; the one-click
   launcher puts the build together on the next start.

## One-click build on this PC (Leyline Control step `client-build`)

`Build browser client.bat` builds the browser client here and puts it in `play\` exactly as every
shipped build came (`dist-test.tgz.part00`, `part01`, ... of 9,969,155 bytes, `dist-test.parts.txt`
with the part count, `wenilla_bg.wasm.br`); then the `client` step ships it. Deploy order: a new
patch in `patches\` (and its line in `patches\series.txt`) -> `client-build` -> `client`.

- **What it needs:** Windows 10 or newer, 16 GB of memory with a page file Windows may grow (the
  last link step takes most of the memory), about 25 GB free on the drive of `%LOCALAPPDATA%`, and
  the internet. Git for Windows is used when installed; otherwise a portable Git is downloaded once.
- **What it does** (`build-browser-client.sh`, run with Git Bash): checks memory and disk first and
  stops with a plain line if either is short; downloads once, into
  `%LOCALAPPDATA%\Leyline\client-build\tools`, rustup with the toolchain benilla-wotlk pins
  (rust-src, the wasm32 target; Visual Studio's linker when it is installed, else the GNU
  toolchain, so no Visual Studio is needed), wasm-bindgen at the `Cargo.lock` version, wasi-sdk 27,
  binaryen (wasm-opt) and brotli; checks out benilla-wotlk at `b9a49428` and applies
  `patches\series.txt` in order; builds the release profile (fat LTO, `-Z build-std`, simd128, as
  the shipped builds were made); then wasm-bindgen, the page files, `wasm-opt -O3`, brotli `-q 5`,
  and the parts. Nothing is installed system-wide, nothing asks a question, and no game data is
  read or written.
- **How long:** the first run about an hour (downloads and the full build); the final link alone
  is 20-40 minutes. Later runs reuse the tools and the build folder.
- **Switches:** `"Build browser client.bat" /dryrun` runs the checks and prints what it would
  download, build and write, changing nothing; `/source` only prepares the patched source (what
  `get-source.sh` does), which is also the check that the patch list still applies.
- **If it stops:** the client in `play\` is unchanged. The window's last lines say why (memory,
  disk, a download, a patch that no longer applies, or the build). Tests:
  `python3 -I client/tests/test_build_browser_client.py`.

Useful switches on the page address: `?moderncombos=1` (retail race/class pairs),
`?cinematics=1` (cinematics are off by default), `?boottrace=1` (record the load list),
`?gpudebug=1` (WebGPU errors), and any CVar as `?name=value` (`farclip=777`).
