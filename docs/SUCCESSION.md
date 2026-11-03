# Succession: keeping Leyline going if the owner can't

Owner, 3 Nov 2026: "suggest ways we can uphold this if I can no longer maintain Leyline but the larger community still
thrives."

This page is a plan, not a finished system. It covers three things:
- what today depends on one person;
- the mechanisms that would remove each dependency, with their risks;
- what only the owner can decide (marked **D1, D2...**).

The tools themselves are in [TOOLKIT.md](TOOLKIT.md), outages in [RESILIENCE.md](RESILIENCE.md), and rebuilding under
another name in [FORKING.md](FORKING.md).

The hard lines don't change for a successor:
- no game data or game client on any server or over P2P;
- no money, tips, tokens or mining;
- no decompiled clients;
- the game's and its maker's names stay out of what players read.

## 1. Single points of failure today

Ranked by what is lost and how hard it is to get back.

| # | What | Held by | If it's gone | Recoverable? |
|---|---|---|---|---|
| S1 | **Updater signing key** | a GitHub secret only. `backup_pubkeys` is empty | no installed launcher can ever be updated again; every player must reinstall a fork by hand | **no**, unless a spare key exists *before* it's needed |
| S2 | **Hero Key data**: the key table (id and secret hash), the pepper, the site key | the server (`/srv/wrath/portal-data`, `/data/leyline`) and its backups, if made | every player's sign-in, profile, forum and per-realm identity | only from a backup someone else can reach |
| S3 | **The server** (one host: website, sign-in service, relay under both names, mirror, flags, realm, Discord bridge, HQ) | the owner's hosting account | the website and sign-in, the official realm, our relay and our mirror. Launchers keep running; GitHub updates keep working | yes, if the backups exist and `server/` is rebuilt elsewhere |
| S4 | **The GitHub account** `orionopenwow` (workspace repo, public repos, releases, CI, ghcr images, the updater secret) | the owner's login (likely a personal account: **confirm**) | CI builds, the first update source and the image registry. Public clones survive anywhere | the code yes; the repo names and release URLs that launchers ask, no |
| S5 | **Private code**: the website (`stream/`), mod index and curation (`tools/hub-index`, `tools/addon-curate`), release mirror, Control, the sign-in service | the private workspace repo, plus a separate project for the sign-in service | nobody else can run the website, sign-in or daily mod updates, legally or practically | only if published with a licence |
| S6 | **Domains** `leylinelauncher.com`, `openvanilla.app` | the owner's registrar (DNS at Squarespace) | every built-in address: site, flags, relays, mirror, chain | if it lapses or is taken: no. Launchers can only follow a new address through an update (S1) |
| S7 | **Release-log key** | **not made yet** | nothing today (the chain is dormant). Once used, losing it stops the chain growing, and no rotation exists yet | after rotation is built (section 2.3) |
| S8 | **Discord server** | the owner's Discord account | the community's chat, announcements and the bridge channel | Discord transfers ownership only from the owner's own account |
| S9 | **Approvals**: mods, looks, flags and staff in HQ are owner-only; Control runs on the owner's PC | the owner | mods stop being approved, flags stay as they are, nothing deploys | yes, with roles and CI deploys (section 2.2) |
| S10 | **Third-party permissions**: Azeroth Survivors (permission statement still pending), the upstream build pins | the owner's contacts | that mode | depends on the authors; not transferable by us |

Already safe from one person:
- the public launcher, host kit, relay and docs (MIT, cloned anywhere);
- relays and realms run by others (the mesh);
- players' own game folders and mods;
- Den files and recipes on players' PCs.

## 2. The mechanisms

### 2.1 A published network charter

This is a short public page, `docs/leyline/charter.md`, in the public repo. It says:
- the hard lines (above);
- who the stewards are and how they are added or removed;
- what stewards may do alone, and what needs two of them (key ceremonies, a successor statement, changing the
  charter);
- what happens if the owner is gone (section 2.6). The trigger is a signed successor statement, never a rumour;
- that any fork may use the code under its licence, but only a statement signed under the charter can call itself
  "the network's successor" to installed launchers.

**D2:** whether to have a charter, and its words. Those are in the owner's voice, so the draft would be `TODO(owner)`.

### 2.2 Several maintainers (stewards)

Three to five people the owner trusts, each holding one part. Nobody holds everything except the owner.

| Asset | Today | Proposed |
|---|---|---|
| GitHub | owner's account | a GitHub **organization** with 2 or more owners. Repos transfer with redirects. Push mirrors to Codeberg and GitLab |
| Domains | owner's registrar | auto-renew for years ahead, transfer lock, a second account holder or a registrar that supports a delegate, and a third independent domain for the mirror and a relay |
| Server | owner | a steward with SSH and a copy of `server/RUNBOOK.md`. Weekly off-server backups (RESILIENCE.md step 4) stored with two stewards, encrypted |
| Discord | owner | 2 or more admins, and the ownership-transfer intent written in the charter |
| Approvals | owner-only in HQ | a "reviewer" role for mods and looks (the HQ code already separates owner and staff). Flags stay owner and two stewards |
| Deploys | Control on the owner's PC | the same step scripts run from CI with a deploy key a steward can rotate |

**D3:** who the stewards are, and whether to move to a GitHub organization.

### 2.3 Signed release log with key rotation and co-signers

This builds on RESILIENCE.md.

1. **Make the release key now** (RESILIENCE.md step 1) and a **spare updater key** (step 2). Both are offline. These two
   are the cheapest and most important actions on this page. Until they exist, S1 cannot be undone.
2. **Rotation (to build).** Add an entry kind `key` to the chain:

       {"kind": "key", "name": "rotate", "new_key": "<hex>", "reason": "...", ...}

   It is signed by the current key. From the next entry on, verifiers accept the new key. Changes needed: the
   `releaselog.py`/`releaselog.rs` check, and the tests.
3. **Co-signers for a lost key (to build).** Branding.json gets `release_log.cosigners` (3 to 5 public keys, one per
   steward) and `release_log.threshold` (for example 3). A `key` entry carrying `threshold` co-signatures replaces the
   key even if the old one is lost. Launchers must have this list built in **before** it is ever needed. That is the
   point of shipping it early.
4. **The updater key is still the gate for installs.** The updater only trusts the keys built into `tauri.conf.json`
   and `backup_pubkeys`. So each steward should also hold one spare **updater** key, listed in `backup_pubkeys` now.
   Any one steward can then sign a release.

**Honest risks:**
- More keys means more ways to leak one. Each extra updater key is a full "push any build" power. Keep them few,
  offline, and give each to a different person.
- A threshold protects the chain, but not the updater (Tauri checks one signature, not k of n). Any single spare
  updater key holder could push a release alone. Watching the signed chain makes that visible, but it does not prevent
  it.
- Co-signers can collude. The charter and public stewards are the defence.
- Rotation code is security-critical and only worth building once the key exists.

**D4:** whether stewards hold spare updater keys, how many, and the co-signer threshold.

### 2.4 Mirrors

- **Code:** push mirrors of the public repo to Codeberg and GitLab. Clones are full copies.
- **Releases:** the release mirror already copies installers, the Host pack, addons and pinned server source with
  sha256 and torrents. Still missing:
  - a `latest.json` per community mirror (each its own chain entry);
  - building from a local folder without GitHub;
  - a mirror on a second domain, not on our server.
- **Players:** "Help share Leyline's own files" (`share_own_files`) is built. Turning it on makes every opted-in
  launcher a seed for signed files.
- **Server source:** already mirrored at our pins and listed in the chain. Host profiles should fall back to the mirrored
  tarball by sha256 (RESILIENCE.md, not done).

The tool `tools/fork-kit` (Control step `fork-check --online`) checks that each address in branding.json answers.

### 2.5 Launchers that follow a community-chosen successor feed

Today a launcher's addresses are fixed at build time. Proposed: a `succession` block in branding.json:

```json
"succession": {
  "feeds": ["https://.../successor.json", "https://<mirror>/successor.json", "relay:/api/mesh/successor"],
  "signers": "release_log"
}
```

The launcher checks the feeds now and then. A `successor.json` is valid only when signed:
- by the release key, or
- by the co-signer threshold (2.3).

It names:
- new update sources and mirrors;
- new relays and a flags URL;
- a website;
- a short message.

On a valid statement, the launcher **shows** it and asks the player before switching addresses. It never switches
without that click. It also only installs updates signed with an updater key it already trusts (the stewards' spares).

**Risks:**
- A stolen release key could redirect players. Asking the player, plus the fact that updates still need a trusted
  updater key, limits the damage to addresses (relays, website).
- A successor feed is a high-value target, so keep it to signed static files on several hosts.
- Without a spare updater key, a successor can change addresses but can never ship a new launcher. Players would have
  to reinstall by hand.

**D5:** whether to build the successor feed, and whether it asks the player or switches by itself (recommended: asks).

### 2.6 A dead-man switch (evaluate, don't rush)

The idea:
- The owner, or the release key, signs a heartbeat: a `note` entry in the chain, or a signed `heartbeat.json`, every
  week or so.
- If launchers see no valid heartbeat for N days (for example 90), they show a banner with the community's fallback
  list. That list is signed and built in, or the last valid successor statement. It never switches anything by itself.

| For | Against |
|---|---|
| Players find the community even if nobody can post an announcement | **False triggers:** a long holiday, a lost key or a server outage looks the same as "gone". A banner saying the network is over, while it isn't, harms it |
| No one has to "declare" the owner gone | Heartbeats need the key online regularly. Automating them (CI signs) puts the key in CI, which is worse than the risk it covers |
| Cheap once the successor feed exists | A network-level block of heartbeats (a censored region) triggers it falsely. It only shows a pre-signed list, so the harm is confusion, not hijack |
| | A fallback list built in today goes stale. Stewards must re-sign it |
| | Scammers already post "Leyline is dead, download X". Only showing signed lists is the defence |

**Recommendation:**
1. Build the successor feed (2.5) first. A deliberate statement by the stewards covers the realistic cases.
2. Add the dead-man banner only with a long window (90 days or more).
3. Let any steward's co-signature count as a heartbeat, so one person's absence alone never triggers it.
4. Show it as information with a link, never a switch.

**D6:** whether to have a dead-man switch at all, and N.

### 2.7 The mod index and hub as reproducible data

- `hub_index.py build` is deterministic: same seeds, same bytes. The seeds, candidates, blocklist and holding list are
  in git, and the daily cloud jobs (`scout.yml`, `hub-index.yml`) run on any fork's CI with the default token.
- Proposed:
  - publish the tools (S5);
  - add a chain `note` entry per day with the index's sha256, so any mirror that rebuilds it can prove it matches;
  - publish the approved lists (`stream/upstream-addons.json`, `stream/looks.json`) beside the index.
- Approvals move to reviewers (2.2).
- Licence for the index data: **D7** (suggested CC0 for our own descriptions; the listed projects keep their own
  licences, as now).

### 2.8 Identity that outlives one server (Hero Key)

- Back up the key table, pepper and site key to two stewards, encrypted (S2). This is what lets a successor site accept
  the **same** Hero Keys and keep the **same** per-realm accounts (TOOLKIT.md, tool 6).
- Pin more than one site key in the host kit (`SITE_KEYS` is a list). Realms then accept a successor's assertions
  without a kit release.
- Publish the sign-in service's source (it is a separate project today). Otherwise "the same keys" also means "the same
  binary nobody else has".
- If none of this exists, realms keep working with their join-page accounts. Hero Key on realms fails closed: it is off
  until a key is pinned.

**D8:** whether the Hero Key backup may leave the server, and to whom. It holds players' account data, so that is a
privacy decision as well as a technical one.

## 3. What only the owner can decide

| | Decision | Why it can't wait |
|---|---|---|
| **D1** | Publish the private parts with a licence: `stream/` (website), `tools/hub-index`, `tools/addon-curate`, `tools/release-mirror`, `tools/fork-kit`, the sign-in service. Which licence (MIT like the rest?), and which repo (add to the export's `ALLOW`, or a second public repo) | without it, nobody can legally run the website, sign-in or mod updates |
| **D2** | A network charter: yes or no, and its words (`TODO(owner)`) | it is what makes a successor legitimate |
| **D3** | Stewards: who; a GitHub organization; a second Discord admin; a second registrar contact | the S4, S6 and S8 risks only change with people |
| **D4** | Spare updater keys: make one now (RESILIENCE.md step 2); stewards each hold one; co-signer threshold | **S1 is permanent if the only key is lost.** Everything else is recoverable |
| **D5** | Build the successor feed; ask or auto-switch | must ship in launchers before it is needed |
| **D6** | Dead-man switch: yes or no, and the window | see the risks in 2.6 |
| **D7** | Licence for the mod index data | needed before others mirror it |
| **D8** | Hero Key and server backups: where they go and who holds them | privacy, and S2 |
| — | Make the release key (RESILIENCE.md step 1) | the chain stays dormant until then |

## 4. Order of work (when the owner says go)

1. **The owner, about 30 minutes:** make the spare updater key and the release key, and put the public halves in
   branding.json. `fork-check` then shows no TODO lines. Make an off-server backup.
2. **Cloud:** publish the private tools under the chosen licence (D1). Extend `tools/leyline-repo/export.py` `ALLOW`,
   with its brand and secret checks.
3. **Cloud:** chain rotation and co-signers (`key` entries) with tests. Then the successor feed in the launcher, asking
   the player, with tests.
4. **Stewards:** GitHub organization, push mirrors, a second relay and mirror on another domain, Discord admins.
5. **Later, if D6 is yes:** the heartbeat and the banner.

## 5. Checking it

`python3 -I tools/fork-kit/fork_kit.py check --online` (Control step `fork-check`, read-only).

On today's Leyline it is READY WITH TODOS. The two TODO lines are the spare updater key and the release-log key.
