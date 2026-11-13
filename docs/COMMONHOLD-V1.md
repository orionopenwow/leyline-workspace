# Commonhold v1: the Leyline network protocol (draft)

**Status:** draft, started 12 Nov 2026. It writes down what the code already does
(`hosting/kit/tools/wunet/mesh.py`, `launcher/src-tauri/src/p2p/mesh.rs`, `hosting/relay/wurelay/`), so that someone
else can write a relay, a host or a launcher that works with ours. Where this text and the code disagree, the
conformance vectors decide, then this text is fixed.

Sources: [LEYLINE-MESH.md](LEYLINE-MESH.md) (records, gossip, relays), [LEYLINE-FRIENDS.md](LEYLINE-FRIENDS.md)
(peer ids, friends, sealed boxes, mailbox), the WULY header in `realm/custom/lua_scripts/ov_leyline.lua` (the
in-game list), and the conformance kit (`hosting/kit/tools/wunet/commonhold.py`).

The hard lines hold for every implementation: **the network never carries game data or a game client**, only
signed facts (who is up, where to connect) and open content that passed the checker. Nothing in the protocol takes
or passes payments.

---

## 1. Words

| Word | Meaning |
|---|---|
| **key** | an Ed25519 key pair. Public keys travel as 64 lower-case hex characters. |
| **id** | `base32(sha256(public key))`, lower case, first 16 characters (`a-z`, `2-7`). Realm ids, node ids and peer ids are all made this way. An id is always **recomputed from the key**, never trusted from the wire. |
| **peer id** | the id of a launcher's own key (friends, mailbox, abuse contact). |
| **relay** (full node) | a server that tunnels realms, holds every record and gossips. |
| **host** | a realm's agent: dials one relay outbound, signs a heartbeat every 60 s. |
| **light client** | a launcher: pulls snapshots from several relays and checks every record itself. |

## 2. Signing

A signature is Ed25519 over the bytes `context + "\n" + canonical(document)`, sent as hex.

**Canonical JSON:** object keys sorted by code point, no whitespace, `","` and `":"` as separators, UTF-8, non-ASCII
characters written as themselves (not `\u` escapes). Signed documents hold only objects, arrays, strings, integers,
booleans and null: **no floating-point numbers**.

The context names the record kind and its version, so a signature for one kind is never accepted as another:

| Context | Signed by | Document |
|---|---|---|
| `wu-listing-heartbeat-v1` | the realm key | the heartbeat (`beat`) |
| `leyline-mesh-endpoint-v1` | the relay's node key | an endpoint attestation (`att`) |
| `leyline-mesh-node-v1` | the relay's node key | the relay record (`node`) |
| `leyline-mesh-block-v1` | a pinned staff key | a block (`block`) |
| `leyline-mesh-info-v1` | the relay's node key or the realm key | the capability document (`info`) |
| `leyline-profile-v1`, `leyline-friendship-v1`, `leyline-friend-*-v1`, `leyline-mailbox-fetch-v1` | see LEYLINE-FRIENDS.md | friends and mailbox |

## 3. Records

Every record is one JSON object `{"kind": ..., <document>: {...}, "sig": "<hex>"}`, at most **6 000 bytes** in
canonical form. Each document carries `"v": 1`.

| Kind | Document | Id | Version (higher wins) | Lives |
|---|---|---|---|---|
| `realm` | `beat` + `endpoints` | `r-<realm id>` | `beat.seq` | 15 min after `beat.ts` |
| `relay` | `node` | `n-<node id>` | `node.seq` | 30 min after `node.ts` |
| `block` | `block` | `b-<target>` | `block.ts` | until `block.expires`, at most 90 days |
| `info` | `info` | `i-<id of its key>` | `info.seq` | 24 h after `info.ts` |

A record more than **300 s in the future** is refused. Field lists for `realm`, `relay` and `block` are in
LEYLINE-MESH.md §3 and the header of `wunet/mesh.py`. The rules a reader must apply:

* **Realm:** `beat.realm_id` must equal the id of `beat.pubkey`; `beat.record.name` is a non-empty string; the
  signature covers the whole beat.
* **Endpoint** (inside a realm record): signed by a relay node key; `att.realm_id` is the realm's id; `att.relay` is a
  host the realm itself named in `beat.relay` or `beat.relays`; ports 1 to 65535; at most 15 minutes old. **A bad
  endpoint is dropped; the realm record stays.** At most 8 endpoints, the newest per node.
* **Relay:** `node.host` is a DNS name (never an IP literal); `node.http` is `http(s)://<that host>[:port]`.
  `"seed"` is never believed from the record.
* **Block:** valid only when `block.staff` is one of the reader's pinned staff keys. While it lasts, the target's
  records (and the target's `info`) are dropped and refused.
* **Info:** see §4.

### Merge rules

For one id: verify fully, or refuse. A blocked target is refused. A lower version is ignored. At an equal version:
identical bytes merge their endpoints (realm records only); different bytes (the key holder equivocating) keep the
record with the **smaller sha256 of its `sig` string**, so every node keeps the same one. A newer realm beat keeps
the old record's endpoints that are still fresh and still named by the new beat.

## 4. The capability document (`info`)

Each relay and each host may publish one. It is a **claim**, not proof: a reader treats it as what the signer says
about itself (a later probe record will check reachability; not in v1).

```json
{"kind": "info", "sig": "<hex>", "info": {
  "v": 1, "pubkey": "<hex>", "role": "relay", "seq": 1762900000000, "ts": 1762900000,
  "proto_min": 1, "proto_max": 1,
  "software": "wurelay", "version": "2026.11",
  "supported": ["listing", "mailbox", "mesh", "ws-bridge"],
  "limits": {"realms": 25, "streams": 512, "kbps": 256, "record_bytes": 6000, "push_per_min": 120},
  "admin": "<peer id>", "rules_url": "https://relay.example.org/rules",
  "region": "eu", "languages": ["en", "de"], "restricted": []}}
```

| Field | Rule |
|---|---|
| `role` | `relay` or `host` |
| `proto_min`, `proto_max` | integers 1 to 65535, `proto_min <= proto_max`: the protocol levels the signer speaks (§5) |
| `software`, `version` | free text, at most 40 and 32 characters |
| `supported` | at most 32 capability ids, each `[a-z0-9][a-z0-9.-]{0,31}` (§4.1). Unknown ids are kept and ignored. |
| `limits` | at most 16 entries, names `[a-z][a-z0-9_]{0,23}`, non-negative integers |
| `admin` | `""` or a peer id: where abuse reports go (a sealed message through the friends mailbox) |
| `rules_url` | `""` or an `https://` address, at most 200 characters |
| `region` | `""` or one of `na-east na-west sa eu africa me asia oce` |
| `languages` | at most 8 tags like `en`, `pt-BR` |
| `restricted` | at most 8 ids: `invite` (invite only), `herokey` (Hero Key sign-in required) |

**Where to get it:** a relay serves its own at `GET /.well-known/leyline` and `GET /api/mesh/info`
(`{"info": <record>, "proto": [min, max]}`), and gossips it like any record. A host sends its own beside a heartbeat
(`POST /api/listing/heartbeat {"beat", "sig", "info"}`) on its first beat and hourly; the relay passes it on only
when its key is the beat's key and its role is `host`. Relays re-sign theirs with every relay record (each 5 minutes).

### 4.1 Capability ids

| Id | Means |
|---|---|
| `mesh` | holds records and gossips (`/api/mesh/*`) |
| `listing` | takes heartbeats and serves `/api/listing/realms.json` |
| `ws-bridge` | the browser bridge `wss://<relay>/ws/<realm>/auth` and `/world` |
| `mailbox` | the friends mailbox `/api/mailbox/<peer id>` |
| `chain` | a copy of the signed release log, `/api/mesh/chain` |
| `node-registry` | the community node registry, `/api/nodes.json` |
| `direct` | the realm (or relay) offers direct connections |
| `herokey` | the realm takes Hero Key sign-in |
| `den-v1`, `wuly-v2`, `sealed-relay` | reserved: Your Den sharing, the in-game list v2, the end-to-end sealed path (designed, not built) |

New ids are added by a proposal (§8); an id is never reused for a different meaning.

## 5. Versions: the tolerant reader

1. **Unknown fields are kept.** A reader never strips a field it doesn't know, inside a signed document or beside
   it, and passes the record on exactly as received. (The signature covers the signed document; unsigned fields beside
   it are kept too, within the 6 000-byte cap.)
2. **Unknown kinds are ignored, not refused.** A record whose `kind` a reader doesn't know is not stored and not
   forwarded, and not held against the sender. The same for a known kind whose document `v` is higher than the reader
   knows. (Implementations answer "ignored", counted apart from "refused".)
3. **`proto_min` / `proto_max`.** The protocol level of this document is **1**. A capability document says which
   levels its signer speaks; a realm may put the same pair in `beat.record`. A missing or malformed pair reads as
   `1..1`. A launcher does not **show** a realm it shares no level with, but every node still keeps and gossips it.
4. **Contexts are versioned** (`-v1`). A breaking change to a document is a new context and a new `v`; both are
   served side by side until the old one is retired.
5. **The in-game list (WULY)** follows the same rule: a `V:<n>` line gives the version, and a window ignores every line
   it doesn't know. A line letter is never reused.

## 6. HTTP surface of a relay

| Path | Answer |
|---|---|
| `GET /.well-known/leyline`, `GET /api/mesh/info` | the relay's capability document |
| `GET /api/mesh/node` | `{"node": <relay record>, "epoch"}` |
| `GET /api/mesh/snapshot` | every record it holds (`Cache-Control: max-age=30`) |
| `GET /api/mesh/records?since=<cursor>&epoch=<e>` / `?buckets=0a,ff` | what changed since a cursor / the records in some buckets |
| `GET /api/mesh/summary` | 256 bucket hashes (bucket = first byte of sha256(id)) |
| `POST /api/mesh/push` | `{"records": [...], "from": "<https base>"}`: at most 200 records, 120 a minute per address, 14 KB |
| `GET /api/mesh/realms.json` | the realm list built from verified records only |
| `GET /api/mesh/chain?from=<n>` | the signed release log |

Gossip timing, relay choice (cost = rtt × (1 + 2 × load)) and fallbacks are in LEYLINE-MESH.md §4 and §5.

## 7. Conformance

* **Vectors:** `launcher/src-tauri/src/p2p/commonhold-v1-vectors.json`, made by
  `python3 -m wunet.commonhold make` (from `hosting/kit/tools`). Fixed test keys and a fixed clock, so the file is
  the same bytes every time. It packages the original cross-language vector (`p2p/mesh-vector.json`, 29 Oct) and adds:
  valid realm, relay and info records; tampered; tampered endpoint; a stolen id; expired; from the future; wrong
  relay; unknown fields kept; unknown kinds and newer versions ignored; a bad `proto_min`/`proto_max`; blocks only
  from a pinned key; and merge cases (equivocation in both orders, endpoint merge, newer wins, a block dropping a realm).
* **What passing means:** for every `check` case, the same result (`ok` with the same id, version, number of good
  endpoints and the listed fields still present; `refused`; or `ignored`); for every `merge` case, the same record
  left under each id.
* **Runners:** `python3 -m wunet.commonhold check` (Python) and the `commonhold_vectors` test in `p2p/mesh.rs` (Rust).
  Both run in CI.
* **A live node:** `python3 -m wunet.commonhold probe https://relay.example.org` checks its capability document,
  that its relay record uses the same key, and every record in its snapshot.

## 8. Changing this spec

Changes go through a **Leyline Proposal** (an issue with the proposal template): the problem, the change, the new
context or capability id, and new vectors. A proposal that would carry game data, take payments, or put the game's or
its maker's names in front of players is closed at triage. A change lands when both runners pass the new vectors.
`TODO(owner)`: who approves proposals, and where they are filed.

## 9. Not in v1

Probe records (verified liveness), presence-proofed player counts, subscribable block lists, the sealed path,
multi-relay agents, the DHT. They are designed in LEYLINE-MESH.md and the network plan; each will arrive as a new
kind (ignored by v1 readers until then).

## 10. Owner decisions this draft waits on

* **Staff keys:** none is pinned yet (`STAFF_KEYS` in `wunet/mesh.py` and `p2p/mesh.rs`), so no block is valid
  anywhere. Making and pinning them, with an offline backup, is the owner's.
* **Our relay's abuse contact:** `RELAY_ADMIN_PEER` is empty until the owner names a peer id for it.
* **Publishing:** whether this document joins the public Leyline repo (it is not in `tools/leyline-repo/export.py`
  ALLOW yet) and under which licence.
