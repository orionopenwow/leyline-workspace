# Be a node: one switch on the Host page

**Status (2026-11-05):** built behind the site flag `be_a_node` (off). Phase 1 below is in the launcher; phases 2
and 3 are designed, not built. The Rust code has unit tests that have not been run yet (cargo can't run where it
was written); the launcher UI has browser tests with the fake core.

The owner, 5 Nov 2026: "create the ability to allow someone to be a node on the network without playing the game
but have it on in the client launcher already as a setting, intuitively in the host section".

## What the player sees

At the top of the Host page, above every Host tab, a card:

* **Be a node**, one line in plain words, and one big **On/Off** switch. (`TODO(owner)`: the line and the thank-you
  line in your words; drafts are in `ui/node.js` and `p2p/node.rs`.)
* While on: what it is doing now (working, resting while you use this PC, resting on battery, today's limit used
  up, paused, stopped by the Leyline team) and the stats: shared today, shared in all, time as a node, and players
  carried now when the full relay runs.
* **Pause / Resume**.
* **Limits** (closed until asked): upload KB/s, connections at most, MB per day, "only while this PC is idle and
  plugged in" (on), "keep running when I close the window" (off), "start with Windows" (off, Windows only).
* **Advanced: full relay**: what carrying players needs (Docker, a public name, open ports), a button to the
  existing Help carry the network box on the Network tab, its players carried, and **Can other players reach this
  PC?** (the STUN check: only its kind is kept and shown, never the address).
* Not offered by the website: the card shows, the switch is greyed, "Not open yet: it opens for everyone at once."

Settings has a short **Network** part with **Open Be a node**. The play bar's queue card "Become a node" and the
Host flow's "Be a node ›" open the card too. Settings > Advanced no longer lists `be_a_node` as a flag row: the
card is its switch.

No home address is shown anywhere: not on the card, not in the stats, not in the network check.

## What a home PC with no open ports can do, honestly

A home PC behind a router accepts no incoming connections unless the router is told to (UPnP, NAT-PMP, port
forwarding) and gets no TLS certificate without a public name. That decides what a node can do.

| Role | Needs | Reduces load on our servers? | State |
|---|---|---|---|
| **Share Leyline's own files** (installers, Host packs, addons, upstream server source, exactly what the signed release log names) over BitTorrent | outbound only; incoming helps | **Not yet, by itself.** Players' launchers don't download from a swarm yet (`transfer.rs` uses `NoSwarm`; the torrent engine is only wired for seeding and the testing switch). Today it gives resilience (copies on many PCs, reachable by any BitTorrent client through the info-hashes in the signed log and DHT) and serves standard BitTorrent clients. It starts cutting our load the day phase 2's download side ships and `torrent_downloads` is on. | **built (phase 1)** |
| **Hold and re-serve the realm list** (verified mesh records) | a reachable port, and launchers that accept a home peer | Barely: a snapshot is small and every launcher refresh still includes a seed (the eclipse defense). Its value is keeping the list alive when our seeds are down. | designed (phase 2) |
| **Relay players' game connections** | a reachable port; for the browser also a public name with TLS | **Yes**, this is where the bytes are (each player streams tens of KB/s for hours). | the **full relay** (Docker + public name) exists as Advanced; a built-in one is phase 3 |

What phase 1 deliberately does **not** do: poll the mesh on its own. A node that pulls a snapshot every few minutes
but serves nobody would add load to our seeds, not take it away.

**Cost to us of phase 1:** each new node first fetches its copies (up to `share::DISK_CAP`, 3 GB) from the mirrors
the signed log names, in the log's order. Put GitHub's release before our own mirror in the log so a wave of new
nodes doesn't land on our box. A node pays that back only once other peers download from it.

## How it works (phase 1, `launcher/src-tauri/src/p2p/node.rs`)

* **Settings** `<app data>/node.json`, **stats** `<app data>/node-stats.json`.
* **The loop** (`node::start`, every 30 s while on, no game needed): read the PC (idle, power), the engine's
  counters and the site's switch (the saved copy; fetched again every 30 minutes), decide the state, start or stop
  sharing, count.
* **State machine** (`decide`, in this order): off; stopped (`LEYLINE_NO_NODE` on this PC); not offered (site);
  paused; resting in use / on battery (the carry ticker's rule, now one function `node::resting` used by both);
  today's cap; full (connections cap); running.
* **Sharing** (`share::node_share`): the full pass (plan, tidy, fetch, torrents checked against the log's
  info-hash) on start and every 6 hours, else the last pass's torrents; seeded by the `torrent-engine` at the upload
  cap. Resting or paused stops the engine and keeps the files, so it resumes without fetching. While the node owns
  the folder, the update check's "Help share" pass leaves it alone.
* **Caps:** upload KB/s (the engine's own rate limit; a change restarts the engine with it); MB per day (counted from
  the engine's real uploaded bytes, `share::engine_counts`; reached = rest until 00:00 UTC; overshoot at most one
  look, 30 s × the cap); connections (the engine's live peers above the cap at two looks in a row = stop for 2
  minutes, which drops every connection, then start again: coarse, because librqbit has no per-session peer cap we
  can set without new API).
* **Stats:** shared today and in all (real engine bytes), time running (a gap longer than 2 minutes, a sleeping PC,
  is not uptime), files shared, peers now, the network check's kind, players carried by the full relay (its own
  status file).
* **Keep running when the window closes** (`app.rs on_window_event`): closing the main window minimizes it while the
  node is on and the box is ticked. No tray icon: that needs Tauri's `tray-icon` feature, a new crate in the tree
  (next step if wanted). The card gets **Quit Leyline** while the box is ticked.
* **Start with Windows:** `reg add HKCU\...\Run /v "Leyline node" /d "\"<exe>\" --node"`: this user only, no admin, no
  new crate. `--node` opens Leyline minimized with the node running. Turning the node off removes the entry.

### Who can stop it

* **This PC:** the switch, Pause, the idle rule, the daily cap, and `LEYLINE_NO_NODE=1` (a hard stop for a shared or
  managed PC; the switch then refuses to turn on).
* **The Leyline team:** `be_a_node` in the website's flags (HQ > Network lists it with the other network-wide
  switches). Off there: every node stops at its next look at the site, within 30 minutes; its saved copy keeps it
  stopped offline. A missing or tampered flags file counts as off for a node that has no good saved copy. The site
  alone never turns a node on: each PC turns it on itself.

## Phase 2: the download side, and the realm list (designed)

1. **A real `Swarm` for players' downloads** (the biggest lever): `transfer::fetch` with the librqbit engine for
   Leyline's own packages, web seeds and HTTPS as fallback, every piece and the whole file checked as today. Then
   `torrent_downloads` on: launcher updates and Host packs come from nodes first. This is what makes phase 1 save
   our bandwidth.
2. **UPnP / NAT-PMP** port mapping for the node's BitTorrent port (recent librqbit versions have a UPnP port-forwarding option: check 9.1;
   else a small SSDP/SOAP client, no crate), so home nodes accept incoming peers.
3. **Home records peers:** a node with a mapped port serves `GET /api/mesh/snapshot` from its verified store. Needed:
   (a) launchers accept a signed `peer` record with a plain `http://` base (integrity comes from the record
   signatures, not TLS; a peer can only leave records out, and the seed is still asked), today they accept https
   peers only (`mesh::load_peers`); (b) a reachability probe from a seed before a peer is listed; (c) **an owner
   decision:** a listed home peer's address is visible to every launcher that reads the list, as with any
   BitTorrent peer. The card would say so before this role turns on, and it would be its own opt-in.

## Phase 3: carrying players from a home PC (designed)

* The relay path today is the Python `wurelay` in Docker with Caddy for TLS. A built-in Rust relay would need: the
  agent tunnel (hosts dial the relay over wss), the auth and world port forwarding, the realm-list rewrite for its
  own endpoint (LEYLINE-MESH.md section 5), signed endpoint records, caps and the pause file. That is a port of
  `wurelay`, a large job.
* Desktop players connect over plain TCP, so a node with a mapped port and **no public name** could carry desktop
  players (not browser players, who need TLS). Its address would be in the realm's endpoint records: the same owner
  decision as 2(c).
* Until then, "Advanced: full relay" is the way to carry players, and the card shows its players carried.

## What only a live test on real PCs can prove

* The Rust compiles and its unit tests pass (`cargo test --lib -q`, `p2p::node`, `p2p::flags`, `p2p::share`,
  `host::network`).
* librqbit's counters (`stats().uploaded_bytes`, `live.snapshot.peer_stats.live`) move as expected while seeding,
  and the connections cap's stop/start doesn't thrash.
* Closing the window minimizes (and Quit Leyline quits) on Windows with the custom title bar; the cover window
  still closes normally.
* `reg add` / `reg delete` for start with Windows, and `--node` opening minimized at login.
* The idle and battery reads on a laptop (the carry ticker's functions).
* Seeding through a home router with no port forwarding: how many peers actually connect.

## Files

`launcher/src-tauri/src/p2p/node.rs` (new), `p2p/share.rs` (node_share, node_stop, engine_counts, the seeder's key
holds the cap), `p2p/flags.rs` (`be_a_node`, `cached_site`), `p2p/cmds.rs` (`node_state`, `node_set`,
`node_check_network`, `node_quit`), `app.rs` (commands, the loop at start, `--node`, keep running on close),
`host/network.rs` (`should_pause` uses `node::resting`), `launcher/ui/node.js` + `node.css` (new), `ui/host.js`,
`ui/hostflow.js`, `ui/app.js`, `ui/index.html`, `stream/leyline_net.py` and `stream/hq.py` (the switch),
`launcher/tests/node.browser.test.cjs` (new).
