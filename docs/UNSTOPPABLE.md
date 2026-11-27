# Unstoppable Leyline: the plan after the server

**Status (2026-11-27):** a plan. Nothing in phases 1 to 7 is built yet; section 1 lists what is. Written from the
owner's grand plan of 27 Nov 2026, the day after the only server (Hetzner) was shut down ([SERVERLESS.md](SERVERLESS.md),
[STATIC-SITE.md](STATIC-SITE.md)). Player pages: `docs/leyline/way-forward.md`, `passport.md`, `joining.md`,
`characters.md`, `guide-play-with-friends.md`.

**The goal:** Leyline needs no server of ours for anything. Not to log in, not to find friends or worlds, not to
keep characters, not to ship releases. The test (phase 5's gate): our site and our relays are off for a week and
nothing a player does breaks.

**Rules that hold throughout:** we never keep keys, emails, IP addresses or passwords. We never ship, host or share
game files. Every record is signed and every source is optional. Player pages say **Works today** or **Coming** in
a status line at the top, and never name the game, its maker or a client version in prose.

## 1. What is built today

| Part | Where | State |
|---|---|---|
| Peer key and peer id (Ed25519 in `peer.key`; id = base32(sha256(pk))[:16]) | `launcher/src-tauri/src/identity.rs` | on; Settings › Your Leyline network key saves and brings it back |
| Friend codes, signed friendships and revocations, profile cards, sealed messages | `p2p/friends.rs`, `p2p/friendnet.rs`, [LEYLINE-FRIENDS.md](LEYLINE-FRIENDS.md) | on; the relay mailbox went with the server, so off-LAN delivery needs a shared direct address |
| Friends keeping characters on a host's world (lasting login `G` + 15 from realm id and peer id) | `host/worldfriends.rs` | on |
| LAN joins (LAN finding, forwarder to the engine's 127.0.0.1 ports, realm-list rewrite) | `host/worldshare.rs` | on |
| One-click single player and the original world | `host/oneclick.rs`, `host/engine.rs`, [SINGLE-PLAYER-ONE-CLICK.md](SINGLE-PLAYER-ONE-CLICK.md) | on |
| Your Den with "Your own address" (port forward) | `host/denways.rs`, [DEN-INVITE.md](DEN-INVITE.md) | on |
| Mesh record format (`leyline-mesh-*-v1`), merge rules, blocks | `p2p/mesh.rs`, [LEYLINE-MESH.md](LEYLINE-MESH.md), [COMMONHOLD-V1.md](COMMONHOLD-V1.md) | built, off |
| Signed release log and file sharing over BitTorrent | `p2p/share.rs`, `p2p/torrent.rs`, [P2P-AND-TORRENT.md](P2P-AND-TORRENT.md) | built, off |
| Be a node | `p2p/node.rs`, `host/network.rs`, [BE-A-NODE.md](BE-A-NODE.md) | built, off |
| NAT type check (classifies only; no hole punching) | `p2p/nat.rs` | built |

Everything below is to build.

## 2. The Passport (phase 1)

Log in with no server.

- **Root key** = today's peer key. Peer id unchanged, so existing friendships and lasting logins carry over.
- **Device keys**: one per PC or phone, each with a `device` record signed by the root key. Revocable from any
  other device. Day-to-day signing uses the device key; the root key signs only device and recovery changes.
- **World keys**: one per world, derived from the root key and the world's realm id (HKDF, context
  `leyline-world-key-v1`), so two hosts can't link the same player. The account name stays derived from the world
  key, as `host/worldfriends.rs` `name_for` already does from the peer id.
- **Profile card**: today's `leyline-profile-v1`, signed by the root (or a device) key.
- **Recovery phrase**: 12 words (BIP-39 wordlist, 128 bits) that regenerate the root seed. Shown once.
- **Guardians**: 2 to 5 friends. Any 2 co-sign a `recovery` record that moves the Passport to a new root key. A
  waiting period during which any existing device can veto. Shares are sealed to each guardian.
- **Second device**: QR code or a 6-digit code (a short authenticated exchange, SPAKE2 or similar, over LAN or the
  join path), ending in a signed `device` record.
- **Login agreement**: the friend's launcher and the host's launcher sign one `login` record (Passport, world, account
  name, time); both keep a copy. The game password is random, made by the player's launcher, typed by the launcher,
  never shown. This replaces the one-time invite logins.
- **Hero Key**: optional legacy. A one-time import binds an old Hero Key to the Passport with a signed record; nothing
  needs it after that.

## 3. Connections with no relay of ours (phase 2)

Dial by Passport key. The stock game client still talks to 127.0.0.1; a local forwarder on each side carries it
(the `worldshare.rs` shape, with the address rewrite).

Order of attempts, first that works wins:

1. **Same network**: the LAN finding and forwarder already in `host/worldshare.rs`.
2. **Direct**: IPv6, or a port opened by UPnP or NAT-PMP with the player's OK.
3. **Hole punching**: QUIC with simultaneous open. Reported rates: iroh about 90% direct in production
   (https://pinggy.io/blog/iroh_1_0_dial_keys_not_ips/); a study of 4.4M attempts about 70%
   (https://arxiv.org/html/2510.27500v1).
4. **A mutual friend as relay**: opt-in, sealed end to end, capped.
5. **Community relays**: pass encrypted bytes only (the Be a node kit, retargeted).

Friend requests and messages go direct, or wait sealed with a shared friend or in a community relay's blind mailbox
(today's `mailbox.py` design, run by anyone).

Invite links carry the host's Passport key and world id, never an address, and open the launcher on Join
(`leyline://join?...`).

## 4. Worlds in three clicks (phase 3)

Host › template › Start, under a minute on a PC that has played before. Templates cover single player, the original
world and Your Den first. The invite link opens the friend's launcher on Join. Phase 0's timings are the baseline.

## 5. Characters that survive any server (phase 4)

- **Signed saves**: on leaving a world, host and player both sign a `save` record (character data hash, previous
  save hash, world id, time): a hash chain. The full save lives on the player's PC and the host's.
- **Sealed backups**: the newest save, sealed to the player, on 3 friends.
- **Witnesses**: launchers that keep only save hashes, so a rollback or a forked chain shows.
- **Re-home**: a new world accepts a character from its chain, under its own rules.
- **Visits**: play on a friend's world for a while; the home chain is untouched.
- **Economy rules**: gold never travels; no cross-world trading; items the new world doesn't know go to a vault.

## 6. A network with no centre (phase 5)

Signed records from six sources, merged by the same rules (newest valid `seq`, expiry, blocks):

1. other launchers (gossip);
2. the Mainline DHT, via pkarr records keyed by the publisher's key;
3. static mirrors: the site, a Codeberg copy, the repo;
4. community relays and nodes;
5. the LAN;
6. friends.

**Blocks** need 2 of a 3-person council's signatures. **Spare signing keys** are held by trusted people, so a lost
key or a lost owner doesn't end the project. A **fork kit** ([FORKING.md](FORKING.md)) lets anyone start their own
copy with their own keys.

## 7. Record types

All Ed25519 over `context + "\n" + canonical JSON` (sorted keys, no spaces), as today.

| Record | Signed by | Status |
|---|---|---|
| `leyline-profile-v1` (Passport card) | root or device key | built |
| `leyline-friendship-v1`, `leyline-friend-revoke-v1`, `leyline-friend-msg-v1` | both friends / the sender | built |
| `leyline-mesh-node-v1`, `-endpoint-v1`, `-info-v1` (relays, realms) | node / realm key | built, off |
| `leyline-mesh-block-v1` (blocks) | today one pinned staff key; becomes 2 of 3 council keys | built, off; to change |
| Release log entries (releases) | release key | built, off |
| `device`, `recovery`, `guardian` | root key / 2 guardians | to build |
| `login` (Passport, world, account) | player launcher and host launcher | to build |
| `save` (hash-chained) | player and host | to build |
| `witness` | witness launcher | to build |
| `invite` (key and world, no address) | host | to build |

The names of the new contexts are fixed when each phase starts; each gets test vectors beside
`p2p/mesh-vector.json`.

## 8. Build order and gates

| Phase | Work | Gate |
|---|---|---|
| 0 | Measure: fresh-PC timings, a two-PC friends test, screenshots | Numbers written down in `docs/smoke/` |
| 1 | Passport | Log in, add a device, recover with the phrase and with 2 guardians, all with no network but the two PCs |
| 2 | Connections | Two PCs on different home networks join with no relay of ours; each fallback forced in turn |
| 3 | Three-click worlds | Host › template › Start under a minute on a played PC |
| 4 | Characters that survive | Host PC wiped; character re-homed to another world from the chain and a friend's backup |
| 5 | Network with no centre | Our site and relays off for a week; nothing breaks |
| 6 | Open doors | AzerothCore, TrinityCore and CMaNGOS adapters; Commonhold spec v1; another launcher reads our records |
| 7 | Final review pass (Claude Fable) | Every page, record and claim checked |

### Chaos tests (run before each gate from phase 2 on)

- Our site down; GitHub down; DNS for our name gone.
- No community relays at all; one hostile community relay (drops, delays, replays, forges).
- A guardian offline; a guardian hostile; a lost root key with only the phrase.
- A host PC wiped mid-session; a host rolling a save back.
- A forged block with one council signature; a stale release record.
- Two PCs behind symmetric NAT (hole punch fails; fallbacks must carry it).

## 9. Decisions still open

| Decision | Options | Notes |
|---|---|---|
| Hole punching | iroh (Rust, QUIC, its own relays as fallback) vs our own on `p2p/nat.rs` | iroh is proven but brings its relay design; our own is smaller and fits the forwarder |
| Passport naming | "Passport" or another word | Player-facing; also the record names |
| Hero Key retirement | one-time import window, then read-only, then gone | Date to set |
| Guardians | default count; asked at first run or later | 2 of N fixed |
| Backups default | sealed backups on friends on or off by default | Storage on friends' PCs |
| Spare key holders | who holds spare signing keys | Trusted people, named by the owner |
| Block council | the three people; terms; how one is replaced | Replaces the single staff key |
| Legal read | a lawyer's read of characters moving between worlds and community relays | Before phase 4 and 5 ship |
| Codeberg mirror | set up and keep in sync with the repo | One of the static mirrors |

## 10. Player docs

Each phase updates `docs/leyline/way-forward.md` (the phase table and "What works today") and the page for its part.
The static site keeps the server note (`tools/site-static/build.py` `DOCS_NOTE_WORDS`) for pages that still describe
what closed.
