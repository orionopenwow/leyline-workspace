# Friends on the Leyline network, with no server

**Status (2026-11-12):** built and tested on one PC (Rust unit tests, a loopback two-launcher test, the UI in
Chromium, the relay mailbox in Python). Not yet tried between two real PCs. The relay mailbox is **off** until
`RELAY_FRIENDS_MAILBOX=1` is set on a relay (the owner deploys).

The owner's words (12 Nov): "working friends and persistent profiles on the Leyline network, even if it's local to
local, recognized in the cache and signed by each other instead of a Hetzner server or completely requiring me to do
anything in the cloud."

So: every friendship is signed by the two people in it, kept on their own PCs, and checked on those PCs every time
it is read. No website, account or server decides who is whose friend. The website's sign-in (Hero Key, Google)
keeps working for what it already does (the official realm, ratings, the forum). Friends don't need it.

| Code | What it is |
|---|---|
| `launcher/src-tauri/src/identity.rs` | this launcher's key, peer id, signatures, sealed boxes, the join handshake |
| `launcher/src-tauri/src/p2p/friends.rs` | the records, friend codes, the cache (`Book`) |
| `launcher/src-tauri/src/p2p/friendnet.rs` | the transport: same network, direct, relay mailbox; limits |
| `app.rs` `friends_*` | the commands the page uses |
| `launcher/ui/friends.js`, `friends.css` | the Friends section (`window.LeylineFriends.mount(el)`) |
| `hosting/relay/wurelay/mailbox.py` | the relay's blind mailbox (flag `RELAY_FRIENDS_MAILBOX`) |

## 1. Identity

* `<app data>/peer.key`: 64 hex characters, an Ed25519 seed made on this PC with the system's random source.
  Owner-only on Mac and Linux. It is never sent anywhere and never logged (`PeerIdentity`'s Debug shows only the
  peer id). A broken file is refused, never silently replaced, because a new key is a new person to your friends.
* **Peer id** = base32(sha256(public key))[..16], lower case. This is the mesh's id rule (`docs/LEYLINE-MESH.md`
  section 3), so no record can claim someone else's id.
* **Encryption keys:** the X25519 key pair is derived from the Ed25519 one by the standard map (libsodium's
  `crypto_sign_ed25519_*_to_curve25519`). So a friend code, which is only the public key, is enough to write to
  someone. `ring` has no static X25519, so `identity.rs` carries a small port of TweetNaCl's curve code (public
  domain). The tests check it against RFC 7748's vectors and against the Ed25519 side.
* **Backup:** there is no settings export in the launcher today. When one is added, it must carry `peer.key`.
* **Hero Key attestation (optional, not built):** a card could carry a site-signed binding of the peer key to a
  Hero Key. It would be shown as an extra and never required. The site has no signing endpoint for that yet.

### For other code: the drop-in API

```rust
identity::KEY_FILE                               // "peer.key", in settings::app_dir()
identity::peer_identity() -> Result<PeerIdentity, String>
identity::load_or_create(app: &Path) -> Result<PeerIdentity, String>
PeerIdentity { pub peer_id: String, pub public_key: [u8; 32], .. }
    .sign(&[u8]) -> [u8; 64]   .sign_doc(ctx, &Value) -> hex   .public_hex()
p2p::friends::Book::load(&app, &me, now).0.is_friend(&me, peer_id)
```

## 2. Records

Every signature is Ed25519 over `context + "\n" + canonical JSON` (sorted keys, no spaces), exactly as the mesh.

| Record | Context | Signed by | Holds | Limits |
|---|---|---|---|---|
| Profile card | `leyline-profile-v1` | its owner | `pubkey, peer_id, name (32), status (80), avatar? (sha256), worlds (at most 4: kind, name, invite_hint "friends"/"ask"), mailbox? (relay host), seq, ts, exp` | 2 KB; `exp - ts` at most 30 days; a higher `seq` (then a newer `ts`) replaces |
| Friendship | `leyline-friendship-v1` | **both** (`sig_a`, `sig_b`) | `{v, a, b, ts}`, the two public keys sorted | one per key pair; refused if revoked |
| Revocation | `leyline-friend-revoke-v1` | either side | `{friendship: id, by, ts}` (id = sha256 of the friendship doc, 32 hex) | kept 60 days, at most 500 |
| Message | `leyline-friend-msg-v1` | the sender | `{kind, from, to, ts, nonce, body}` | 8 KB; good for 10 minutes; each nonce once |
| Envelope (wire) | none (sealed) | | `{v, to: peer id, box: base32(sealed message), tag?}` | 15 KB |
| LAN beacon | `leyline-friend-beacon-v1` | its owner | `{pubkey, port, ts, name?}` | 600 bytes; 60 s |

Message kinds: `request`, `accept`, `decline`, `revoke`, `presence`, `invite`, `card`.

**Sealed box:** an ephemeral X25519 key, then X25519 with the recipient's key, then HKDF-SHA256 (salt = ephemeral
public + recipient public, info `leyline-friends-seal-v1`), then ChaCha20-Poly1305. The output is
`ephemeral public || ciphertext || tag`. Only the recipient opens it. Inside, the sender's signature says who wrote
it, and the signed `to` stops a message being forwarded to someone else.

## 3. Becoming friends

1. **Bo shares a friend code:** `LF1-<base32(public key + 5-byte token)>@<relay>`. The token is good for 24 hours,
   with at most 8 at a time. The relay is where Bo reads his mailbox, and is left out if he turned the mailbox off.
2. **Ari pastes it.** Her launcher sends a `request`: her card, the friendship document signed by her, and Bo's
   token. It goes by the same network when Bo's beacon is heard, else to Bo's relay mailbox. If neither works, it
   is kept and sent again when Leyline next starts.
3. **Bo's launcher takes a request only if** it carries one of his live tokens, **or** it came from his own local
   network while he is visible there ("People near you": no code needed). A request from the internet or a mailbox
   without a token is dropped, because anyone can learn a public key.
4. **Bo presses Accept.** He countersigns, keeps the doubly signed friendship, and sends `accept` with it and his
   card. Ari's launcher keeps it only if it matches a request she still has waiting. So nobody can make a friendship
   with her that she didn't ask for, even with her signature on an old document.
5. **Either can Remove.** That signs a revocation, drops the friend, sends `revoke`, and keeps the revocation so the
   old friendship can't be replayed back in.

**Cache:** `<app data>/cache/friends.json`, at most 2 MB. On every load:

* each friendship is checked: both signatures, with me as one side, not revoked;
* each card, presence and invite is checked against its friend's key;
* requests are checked, at most 50, 14 days each;
* at most 200 friends.

Anything that fails is dropped and counted.

## 4. Transport

| Path | When | How |
|---|---|---|
| **Same network** | both PCs on one home or office network, **with or without internet** | A signed beacon every 10 s by UDP broadcast to port **41190**. A TCP listener on **41191** (or any free port: the beacon says which) takes one sealed envelope per connection, one JSON line each way. Beacons are taken only from private, link-local or loopback addresses. |
| **Direct, across the internet** | a friend shares an address that reaches their listener (their own port forward) | That address rides inside their sealed presence, to friends only. Hole punching (`p2p/nat.rs` only classifies) is **not built**. |
| **Relay mailbox** | otherwise | `POST https://<relay>/api/mailbox/<peer id>` with the envelope. The owner fetches every 60 s with `GET …?pub&ts&relay&sig`: a signature over `leyline-mailbox-fetch-v1` `{v, peer_id, relay, ts}`, within 5 minutes, for that relay's own name. Boxes are handed over once. Any Leyline relay running the flag works. Presence and card boxes carry an opaque tag, so the newest replaces the last. |

**Why not the mesh gossip:** the mesh replicates every record to every node, which would turn private friend data
into a public directory. Friends' records go only to the two people in each friendship.

**The relay change:** `RELAY_FRIENDS_MAILBOX=1`, default off, in `wurelay/relay.py` and `wurelay/mailbox.py`. It
keeps boxes in memory only, for 24 h at most.

* Caps: 16 KB a box, 64 boxes per recipient, 20 000 boxes in all.
* Rate limits: 60 posts and 20 fetches a minute per address.

## 5. Presence and invites

* **Presence:** every 4 minutes, and at once after a profile change, to every friend. It carries online/offline
  (offline is sent when Friends stops) and the sender's current card.
  * Only if the player shares it: what they're playing, and the world they have open to friends.
  * Only if the player shares one: a direct address.
  * The page shows a friend as online for 10 minutes after their last presence, or while their beacon is heard.
* **Invite to my world:** this runs the existing `world_invite` (host/worldshare.rs: the relay tunnel, private,
  plus the same-network way). The link, which holds the friend's guest login, goes to that one friend, signed and
  sealed. The relay can't read it.
* **Join:** the friend's page shows Join on that friend while the invite is under 24 hours old. It opens the usual
  Join card (`LeylineSolo.offerJoin`, then `join_invite`).

### Lasting world logins (the host side: the social-ui work)

A host recognises a returning friend by peer id, not by a password:

1. The host sends `identity::join_challenge(&host, world, now)`: `{v, host, world, nonce, ts}`.
2. The friend answers with `identity::join_answer(&me, &challenge)`: `{pubkey, sig}`, signed with context
   `leyline-join-v1`.
3. The host runs `identity::check_join(&host.peer_id, &challenge, &answer, now)`. It checks the signature, that the
   challenge is this host's, and that it is under 120 s old, and returns the friend's peer id. The raw check is
   `identity::verify_join(join_bytes(&challenge), pubkey, sig)`.
4. The host then checks `Book::is_friend(&me, &peer_id)`.

**Not yet wired:** a channel that carries the challenge between the two launchers. It can be one more message kind
on the transport above.

## 6. Privacy: who sees what

| Who | Sees |
|---|---|
| A relay running the mailbox | the recipient's peer id, the poster's and fetcher's addresses, sizes, times, and an opaque per-pair tag on presence and card boxes (it can tell that the same pair is still talking). **Never** who sent a box, names, cards, invites or links. |
| A neighbour on the same network | with "Friends only" (the default): a stable public key and port in the beacon, but no name. With "Everyone": the name too. With "Nobody": nothing. Envelopes over TCP are sealed. |
| Friends | your card, presence and invites, plus what you play and a direct address only if you share them. |
| Leyline's website and servers | nothing. Friends never touch them. |

## 7. Abuse limits

* **Per message:** a valid sender signature, `to` = me, under 10 minutes old, a nonce not seen before (the last
  4096 are kept).
* **Per address:** 30 TCP messages and 30 beacons a minute. At most 16 connections at once.
* **What's kept:** at most 64 people near, 50 requests waiting, 50 sent, 200 friends, 8 codes. Cache files over
  2 MB are not read.
* **Who can write:** requests only with a live code, or from the same network. Everything else (presence, invites,
  cards, revokes) only from friends.

## 8. What needs a real two-PC test

1. Two PCs on one network, **internet unplugged**:
   * each sees the other under People near you (with "Everyone");
   * add, accept, presence, Invite, Join, Remove.
2. The Windows firewall prompt for UDP 41190 / TCP 41191: allow on private networks.
3. Two PCs on different networks, with a relay running `RELAY_FRIENDS_MAILBOX=1`:
   * add by code;
   * accept arrives within a minute;
   * presence and invites follow.
4. Two launchers on one PC: only one can hear beacons (UDP 41190 is taken). The second still sends.
5. Broadcast to 255.255.255.255 on networks that filter it (some Wi-Fi guest networks): use the code and mailbox
   instead.
