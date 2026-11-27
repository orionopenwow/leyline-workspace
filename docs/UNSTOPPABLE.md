# Unstoppable Leyline: the plan after the server

**Status (2026-11-27):** a plan. Phase 2's connections are built and tested on one PC (section 3); nothing else in
phases 1 to 7 is built yet. Section 1 lists what is. Written from the
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
| NAT type check (classifies only) | `p2p/nat.rs` | built |
| Connections by key: same network, direct, hole punching (with a helper), a mutual friend, community relays; sealed messages kept by a shared friend | `p2p/link.rs` (rules), `p2p/linknet.rs` (iroh, feature `link` in `app`) | on for friends; tested in one process only (section 3) |

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

### What is built (27 Nov 2026)

- **Transport:** iroh 1.5 (MIT OR Apache-2.0; parts derived from tailscale under BSD-3), `presets::Minimal`: no
  relays or DNS of iroh's maker. Relays (hole-punch helpers) come only from signed relay cards or Settings ›
  Network; address lookup from friends' sealed presence, the LAN beacon, invite links and the Mainline DHT
  (`iroh-mainline-address-lookup`, which publishes only the helper URL, never addresses). `link::Lookup` is the
  boundary for the DHT records of `p2p/dht.rs` (`link::dht_lookup`, a stub until that lands).
- **Keys:** the QUIC key is a **device key** (`identity.rs` `device_seed`: HKDF of the root seed, "leyline device
  v1", until the Passport's device records replace it in that one function). Every stream starts with both sides'
  device certs (`leyline-device-v1`, signed by the root key), so the peer key and peer id are unchanged and a wrong
  device key from any hint is refused.
- **Go-betweens:** a mutual friend (Settings › Network › Help friends connect: on by default, 1 GB a day, 4 at once)
  or a community relay (Be a node plus "Carry sealed connections for other players", within Be a node's caps).
  A second seal end to end (`leyline-link-seal-v1`: X25519 with both peer keys signing the transcript, then
  ChaCha20-Poly1305 frames) means the go-between sees only sealed bytes and can't stand in for either side.
- **Relay card:** a `leyline-mesh-node-v1` record with host `<peer id>.link.invalid` and a `link` part; passed to
  friends in presence. Not yet on the DHT or the mesh sources.
- **Wired in:** Invite a friend (single player, the original world) puts `~~key=` and `~~dev=` in the link and
  offers the world's two ports behind a knock from the invite's secret; Join tries the key after the same network
  and the host's own address, before the old relay, with a forwarder on 127.0.0.1 and the realm list rewrite
  (`AuthRewriter`). Friend requests, presence and messages try the key after the same network and a shared direct
  address; other messages wait with a shared friend (sealed, 7 days, 32 per person, 64 per sender, 256 in all).
- **Flags:** none from the site. On for friends by default; Settings › Network turns parts off. Port mapping is off
  until the player turns it on.
- **Tested** (`cargo test --features link p2p::link`): the order of ways, caps and refusals, the seal (including
  a go-between standing in), two launchers by key, three with a friend passing 100 KB both ways, a community relay,
  a held message delivered when the friend is back, a guest game reaching a world through 127.0.0.1 with the realm
  list rewritten, a device key that isn't the friend's refused. All on 127.0.0.1 in one process.
- **Not tested:** two PCs on different home networks (the phase 2 gate, for the owner), hole punching (needs a
  helper relay server; none is run by Leyline and none is in the tests), port mapping, the DHT lookup, IPv6, the
  Settings › Network page, Leyline Solo (first-era) invites (not wired), Den invites (still the old relay or the
  host's own address).

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
| Hole punching | decided 27 Nov 2026: iroh, configured with no relays of its maker | Helpers are community-run `iroh-relay` servers named in signed relay cards; who runs the first ones is open |
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
