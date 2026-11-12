//! P2P and torrent plumbing, built and tested but OFF until the flags say otherwise (`flags`).
//!
//! * `flags`: the site's flags file (sha256-checked) plus this PC's overrides. Everything defaults to off.
//! * `transfer`: one download path for Leyline's own packages (mods, theme packs, the Host pack, launcher
//!   updates): HTTPS, or a torrent's swarm / web seeds when `torrent_downloads` is on. Every result passes the
//!   refusal list (`checker`: never game data, never a client) and a sha256 check.
//! * `torrent`, `bencode`, `sha1`: single-file `.torrent` metainfo, web seeds and the piece check.
//! * `magnet`: shared `magnet:` links, opened only for Leyline's own checked packages (`magnet_links`).
//! * `nat`, `direct`: the NAT check (STUN) and the rule for when a direct connection to a hosted realm may
//!   replace the relay (`p2p_hosting`).
//! * `mesh`: the realm list from the Leyline mesh (signed records from peers and seed relays, verified here) and
//!   the relay chosen per realm (`mesh_realms`, `mesh_relays`). See docs/LEYLINE-MESH.md.
//! * `share`: "Help share Leyline's own files" (`share_own_files`): seed only what the signed release log names
//!   (installers, Host packs, addons, upstream server source). See docs/RESILIENCE.md.
//!
//! * `node`: "Be a node" (`be_a_node`): the Host page's one switch. While on, the launcher shares Leyline's own files
//!   with no game running, within caps, only while idle and plugged in. See docs/BE-A-NODE.md.
//!
//! * `friends`, `friendnet`: friends with no server: signed cards and friendships, sealed messages, the same network
//!   or a relay's blind mailbox (docs/LEYLINE-FRIENDS.md).
//!
//! See docs/P2P-AND-TORRENT.md.

pub mod bencode;
pub mod checker;
pub mod direct;
pub mod flags;
pub mod friendnet;
pub mod friends;
pub mod magnet;
pub mod mesh;
pub mod nat;
pub mod node;
pub mod sha1;
pub mod share;
pub mod torrent;
pub mod transfer;

#[cfg(all(feature = "app", desktop))] // desktop only (mobile.rs)
pub mod cmds;
