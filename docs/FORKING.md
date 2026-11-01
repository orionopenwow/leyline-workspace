# Forking Leyline: run it all under another name

Leyline's code is open source. If Leyline ever disappears, or you want to run your own network, you can rebuild
the launcher, the host kit and the relay under your own name, with your own keys and servers. Nothing in the code
depends on our servers being there: every address and key is in one file.

What a fork must keep: the same hard lines. The network never carries game data or game clients; players always
use their own. No money, tokens or mining.

## 1. One file: `launcher/branding.json`

| Key | What |
|---|---|
| `name`, `identifier` | The app's name and its system id (`org.example.launcher`). |
| `colours` | Background, text, accent, strong accent (`#rrggbb`). |
| `logo`, `emblem` | Picture files in `launcher/` (replace the files themselves). |
| `site` | Your website (the flags file, the realm list and the mirror live under it). |
| `repo`, `updates` | Your public repo (`owner/name`) and where releases are: the GitHub API, downloads, release page, your mirrors (folders ending in `/`), spare updater keys. |
| `release_log` | Your release key's PUBLIC half (hex) and where your chain is published. |
| `relays` | Your relays (the first is the seed every launcher asks). |
| `flags_url` | Your flags file. |

Edit a copy, then apply it:

    python3 launcher/scripts/rebrand.py my-branding.json --dry-run    # what would change
    python3 launcher/scripts/rebrand.py my-branding.json

It writes the addresses into the Rust code, `tauri.conf.json` and the UI, the colours into `ui/style.css`, the name
into the pages and the window title, and the release key into `hosting/kit/tools/wunet/release-log.json`. Then:

    cd launcher/src-tauri && cargo test --no-default-features --lib

`branding::tests` checks that the code says what branding.json says. A few other tests check our own addresses in
data files (`stream/playmodes.json`, the realm and magnet tests); change those addresses to yours too. Words inside
the UI's scripts are not changed by the script: search `launcher/ui` for the old name.

## 2. Your own keys

You need three, made once, kept offline (never in the repo):

1. **The updater key** (signs every launcher release; launchers refuse anything else):

       npx @tauri-apps/cli signer generate -w my-updater.key

   Put the public key (the base64 text) in `launcher/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`, and the
   private key in your CI's secrets (`TAURI_SIGNING_PRIVATE_KEY` and its password). Make a **spare** the same way
   and put its public key in branding.json → `updates.backup_pubkeys`.

2. **The release key** (signs the release log):

       PYTHONPATH=hosting/kit/tools python3 -m wunet.releaselog keygen --out ~/.mylauncher/release-log.key

   It prints the public key only: put it in branding.json → `release_log.public_key` and run rebrand.py again.

3. **Relay and realm keys** are made by each relay and host on first start (`node.key`, the realm key); nothing to
   do.

## 3. Build and publish

- **Launcher:** the workflow in `.github/workflows` (from `tools/github-workflows/leyline-launcher.yml`) builds and
  signs releases on every push; or build by hand: `npm install && npx tauri build` in `launcher/` with
  `TAURI_SIGNING_PRIVATE_KEY` set. Upload the files and `latest.json` to your GitHub release and your mirror.
- **Mirror:** any static web server. Layout: `latest.json` (its downloads pointing at the mirror),
  `launcher-v<version>/<files>`, `host-pack/`, `addons/`, `sources/`, `index.json`, `chain.json`. Every file is
  checked by the launcher (updater signature, chain sha256), so the mirror needs no trust of its own.
- **The release log:** one entry per release:

      PYTHONPATH=hosting/kit/tools python3 -m wunet.releaselog append --log chain.json --key ~/.mylauncher/release-log.key \
          --kind launcher --name MyLauncher --version 1.0.0 --file MyLauncher_1.0.0_x64-setup.exe --file latest.json \
          --mirror https://my.example.org/releases/
      PYTHONPATH=hosting/kit/tools python3 -m wunet.releaselog verify --log chain.json

  Kinds: `launcher`, `hostpack`, `addons`, `source` (with `--source-repo` and `--source-commit`), `note`. Publish
  `chain.json` at the addresses in branding.json.
- **Relay:** `hosting/relay` (Docker). Set `RELAY_RELEASE_LOG_KEY` to your release key's public half (or put it in
  `wunet/release-log.json`) and `RELAY_MESH=1` so relays keep and swap the chain.
- **Host kit:** `hosting/kit` builds server software from its authors' source at the commits in
  `hosting/kit/profiles/*/profile.lock`. Mirror those source tarballs too (they are open source); list them in the
  chain as `source` entries so they can be found and checked if upstream disappears.

## 4. Picking up where Leyline left off

If the original network is gone, a fork can start from the last copy of Leyline's chain that any launcher, relay
or host agent kept (`cache/release-log.json` in the launcher's data folder, `release-log.json` beside a relay's
state). It is checkable by anyone against Leyline's public release key, and it lists every file with its sha256,
so the files people still have can be checked and shared again. A new network signs its own chain with its own key
from then on; launchers rebuilt from the fork trust only the fork's key.
