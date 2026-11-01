# Resilience: what keeps working when something goes down

For the owner. Plain words: what survives what, and the few things only you can do.

Hard lines that stay: the network never carries game data or game clients; nothing here involves money, tokens,
coins or mining. "The chain" below is a signed list, nothing more.

## The pieces

| Piece | What it is | Where |
|---|---|---|
| **Update sources** | The launcher looks for a new version in order: GitHub releases, then our own copy on our server, then the copies the release log names. | `launcher/src-tauri/src/update.rs` (`sources`, `candidates`), `app.rs` |
| **The updater key** | Every launcher only installs a download signed by this key (its public half is in `tauri.conf.json`). A mirror can be down or wrong; it can't push a bad build. Never a version lower than (or the same as) the one running. | GitHub secret `TAURI_SIGNING_PRIVATE_KEY` |
| **Spare updater keys** | Extra public keys in `launcher/branding.json` (`updates.backup_pubkeys`). A release signed with one of them installs too. | branding.json; the private halves offline with you |
| **The release mirror** | Our copy of every launcher release (the newest three), the Host pack, the Leyline addons and the upstream server source at our pinned commits, with checksums and torrents. | `https://play.openvanilla.app/wrath/leyline/releases/` (server: `/srv/wrath/portal-data/leyline/releases`) |
| **The release log ("the chain")** | One signed entry per release, Host pack, addons pack and pinned server source: files, sizes, sha256, torrent info-hash, mirrors. Each entry names the one before it by its hash. Launchers, relays and host agents keep a copy and swap it; the longest good chain wins; anything not signed by the release key is refused. | `releaselog.rs`, `hosting/kit/tools/wunet/releaselog.py`; `/wrath/leyline/chain.json`; relays at `/api/mesh/chain` |
| **The release key** | The Ed25519 key that signs the chain. Not in the repo, not on GitHub, not on the server. | Your PC, plus an offline backup |
| **"Help share Leyline's own files"** | Players who opt in keep copies of the files the chain names and share them (upload capped). Only those files, checked by hash; never game data. | `p2p/share.rs`, flag `share_own_files` |
| **The fork kit** | One file for the name, look, addresses and keys, a script that applies it, and how to rebuild everything under another name. | `launcher/branding.json`, `launcher/scripts/rebrand.py`, `docs/FORKING.md` |

## What survives what

**GitHub is down (or the repo is gone).**
- Installed launchers keep updating from our mirror: it serves the same signed files and a `latest.json` that
  points at itself. The download is still checked against the updater key.
- Host packs, the addons and the server source for every supported profile are on our mirror, listed in the chain.
- New launcher releases: CI runs on GitHub, so nothing new is *built* until you build one elsewhere (any PC can:
  `npx tauri build` in `launcher/` with the updater key). If the main updater key went with GitHub (it lives only
  in GitHub's secrets today), sign with a spare key (see "What you must do", step 2). Put the files on the mirror by
  hand or with a small change to the mirror tool's source.
- The code itself is open source: every clone of the public repo is a full copy.

**Our server is down.**
- Updates come from GitHub as before.
- The chain is also attached to the GitHub release `release-log`, and relays keep their own copies.
- Players who turned on "Help share Leyline's own files" keep sharing the installers, Host packs and source.
- What does not survive: the website, the sign-in, the realm list from our site (the mesh can carry it when
  `mesh_realms` is on), and our relay. Back up the databases (below).

**Both are down.**
- Installed launchers keep running and keep their verified copy of the chain.
- Players who share keep the signed files moving between them (torrents). Anyone can check any copy against the
  chain, and the chain against the release key's public half that every launcher has built in.
- A community can rebuild and run everything under another name with its own keys: `docs/FORKING.md`.

## What you must do

1. **Make the release key (once) and back it up offline.**
   On your PC, from the workspace folder:

       cd hosting\kit\tools
       py -3 -m wunet.releaselog keygen --out %USERPROFILE%\.leyline\release-log.key

   It prints only the PUBLIC key (it refuses to overwrite an existing key). Then:
   - put that public key in `launcher/branding.json` → `release_log.public_key`, and in
     `hosting/kit/tools/wunet/release-log.json` → `public_key` (or run `launcher/scripts/rebrand.py` with the
     edited branding.json, which writes both), and ship;
   - copy `%USERPROFILE%\.leyline\release-log.key` to two offline places (a USB stick in a drawer, a printed copy of
     the file's text). It is a small text file. Anyone with it can sign releases: never put it in the repo, a chat,
     a screenshot or a cloud folder. Losing it means the chain can't grow (installed launchers would need an update
     that trusts a new key).
   Until you do this, the chain is dormant: nothing is signed and launchers ignore it. Everything else works.

2. **Make a spare updater key and keep it offline.** Today the only updater signing key lives in GitHub's secrets
   (`setup\Turn on launcher updates.bat` wipes its own copy). If GitHub went away, no new launcher could be signed
   for the launchers already installed. Make a spare:

       npx @tauri-apps/cli signer generate -w E:\leyline-spare-updater.key

   (to a USB stick; choose a password and write it down with the stick). Put the **public** key it prints (the
   base64 text in the `.pub` file) in `launcher/branding.json` → `updates.backup_pubkeys`, and ship. From then on,
   every installed launcher also accepts a release signed with the spare.

3. **Turn on the mirror step.** After each ship, once GitHub has finished building the release (about 20 minutes),
   run the Control step **release-mirror** (`tools/release-mirror/Mirror releases.bat`). It copies the new files
   to our server with checksums and torrents and, with the release key on the PC, signs the new chain entries and
   attaches `chain.json` to the GitHub release `release-log`. **release-mirror-check** says what it would copy.
   The first run copies the source tarballs too (a few hundred MB, once).

4. **Back up the server's databases.** Everything players made lives in `/srv/wrath/portal-data/` on the server
   (`*.sqlite3`: forum, profiles, Hero Keys, social; `leyline/`: flags, the mirror and its chain; `network/`) and in
   the realm's MySQL databases. Keep a copy off the server, for example weekly:

       ssh root@play.openvanilla.app "tar -czf - -C /srv/wrath portal-data --exclude=portal-data/leyline/releases" > portal-data-YYYYMMDD.tgz

   and the realm's MySQL databases (auth, characters, world: a `mysqldump` of each, copied to your PC the same
   way). Keep the copies offline; they contain players' accounts, so treat them like the release key.

5. **Turn sharing on when you are ready** (optional). In Leyline HQ's flags (or `stream/leyline_net.py
   write_flags`): `share_own_files` on. Then each player still has to turn it on for their own PC (Host > Advanced,
   with an upload cap above 0). Turning the site flag off stops it everywhere at the next check.

## Flags

| Flag | Default | Meaning |
|---|---|---|
| `share_own_files` | off | Offer "Help share Leyline's own files" to players (each PC still opts in, with an upload cap). |
| `mesh_realms` | off | Also lets launchers fetch the chain from mesh peers (they always ask our site, GitHub and the seed relays). |
| `torrent_downloads` | off | Packages over BitTorrent web seeds first (unchanged). |

## Deploying this

1. **ship** (the launcher changes go out as the next release; `stream/leyline_net.py` and `stream/wrath_portal.py`
   with the next **signin** step: the site serves `/wrath/leyline/releases/` and `/wrath/leyline/chain.json`, a
   wu-portal restart, no realm restart).
2. **relay** (relays keep and swap the chain; only with `RELAY_RELEASE_LOG_KEY` or the key in
   `wunet/release-log.json`).
3. **release-mirror** after GitHub has built the release.

## What is not done yet

- The host profiles still fetch server source from upstream git only; falling back to our mirrored tarball (by its
  sha256 in the chain) when upstream is gone is the next step. The tarballs are mirrored and logged already, so a
  host can fetch one by hand today.
- One mirror per release: the mirrored `latest.json` points at our server. Community mirrors listed in the chain
  need their own manifest (and so their own entry).
- New releases without GitHub need a small change to the mirror tool (copy from a local build folder instead of
  the GitHub API).
- Host packs do not carry a copy of the chain yet (`wunet/release-log-chain.json` is read when it is there).
- The torrent seeding test with two real engines needs IPv6 (it is marked ignored on the build box, like
  clienttorrent's).
