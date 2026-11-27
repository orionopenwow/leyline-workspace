//! Launcher-to-launcher connections by key, with no relay of ours (docs/UNSTOPPABLE.md phase 2,
//! docs/leyline/joining.md). This file holds the rules, with no network and no async: the order of ways to try,
//! who may use this PC as a go-between and how much, the sealing of a stream that passes through someone else, the
//! sealed messages kept for a friend who is offline, the relay card, and the invite link's key. The network side
//! (QUIC with hole punching and port mapping, iroh) is `p2p/linknet.rs`, behind the `link` feature.
//!
//! Ways, first that works wins ([`plan`]):
//! 1. **Same network**: the friend's LAN address (from the LAN beacon), straight to their launcher.
//! 2. **Direct**: an address the friend's launcher shared (IPv6, or a port their router opened when they allowed it).
//! 3. **Hole punching**: both launchers reach out at once, helped by a hole-punch helper (a community-run relay
//!    server listed in signed relay cards or typed in Settings). Without a helper this way is skipped.
//! 4. **A mutual friend**: a friend's launcher that is online and allows it (Settings › Network › "Help friends
//!    connect", on by default, capped per day) passes the bytes between two of its friends.
//! 5. **Community relays**: Be a node launchers that chose to carry sealed connections for anyone, capped.
//!
//! Ways 1 to 3 are QUIC, sealed by the two launchers' own keys. Ways 4 and 5 add a second seal end to end
//! ([`Handshake`]) inside, so the go-between only ever sees sealed bytes.
//!
//! The peer key stays the identity. The QUIC key is this PC's *device* key (identity.rs `device_seed`), never the
//! root, so the DHT packet iroh publishes under it can't collide with records under the root key. Every stream
//! starts with both sides' device certs (the root's signature over the device key), so each side knows the other's
//! peer key and peer id (base32(sha256(root key))[:16], unchanged); a device key from any hint is checked that way.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use crate::identity::{self, PeerIdentity};

/// The QUIC protocol name of these connections.
pub const ALPN: &[u8] = b"leyline/link/1";
/// The UDP port tried first (another free port when it is taken; the presence message names the real one).
pub const LINK_PORT: u16 = 41192;
pub const SETTINGS_FILE: &str = "link.json";
pub const HELD_FILE: &str = "link-held.json";
/// Most bytes in a request header or an answer.
pub const HEADER_MAX: usize = 4096;
/// The second seal's context (signed transcript and key derivation).
pub const CTX_SEAL: &str = "leyline-link-seal-v1";
/// The knock a friend's launcher shows to reach a shared world (from the invite's secret).
pub const CTX_KNOCK: &str = "leyline-link-knock-v1";
/// The mesh's relay record context (p2p/mesh.rs): a relay card is a mesh relay record with a `link` part.
const CTX_NODE: &str = "leyline-mesh-node-v1";
/// Most plain bytes in one sealed frame.
pub const FRAME_MAX: usize = 16 * 1024;
/// Most go-betweens of each kind tried for one connection.
pub const MAX_GO_BETWEENS: usize = 3;

// ------------------------------------------------------------------------------------------------ settings

/// Settings › Network. The friends' connections need no flag from the site (there is no server to set one): they
/// are on unless the player turns them off here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkSettings {
    /// Reach friends by their key from anywhere (on by default).
    pub on: bool,
    /// "Help friends connect": pass sealed bytes between two of your friends while you're online.
    pub help_friends: bool,
    /// Most data passed for friends per day, MB.
    pub help_day_mb: u32,
    /// Most friends' connections passed at once.
    pub help_streams: u32,
    /// Let the router open a port for Leyline (UPnP, NAT-PMP or PCP). Off until the player allows it.
    pub router_port: bool,
    /// Use community relays when nothing else works.
    pub community: bool,
    /// Hole-punch helpers typed by the player (relay server URLs), besides those in signed relay cards.
    pub helpers: Vec<String>,
    /// Be a node: carry sealed connections for anyone (within Be a node's caps). Off by default.
    pub community_relay: bool,
}

impl Default for LinkSettings {
    fn default() -> Self {
        LinkSettings { on: true, help_friends: true, help_day_mb: 1024, help_streams: 4, router_port: false, community: true, helpers: Vec::new(), community_relay: false }
    }
}

pub const HELP_DAY_MB: (u32, u32) = (16, 1_000_000);
pub const HELP_STREAMS: (u32, u32) = (1, 64);

/// A helper's URL as typed: http(s), a host, no spaces, at most 200 characters.
pub fn helper_ok(u: &str) -> bool {
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or("");
    let host = rest.split(['/', ':']).next().unwrap_or("");
    u.len() <= 200 && !host.is_empty() && !u.chars().any(char::is_whitespace) && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

pub fn clean(mut s: LinkSettings) -> LinkSettings {
    s.help_day_mb = s.help_day_mb.clamp(HELP_DAY_MB.0, HELP_DAY_MB.1);
    s.help_streams = s.help_streams.clamp(HELP_STREAMS.0, HELP_STREAMS.1);
    s.helpers.retain(|h| helper_ok(h));
    s.helpers.truncate(8);
    s
}

pub fn settings_path(app_dir: &Path) -> PathBuf {
    app_dir.join(SETTINGS_FILE)
}

pub fn load(app_dir: &Path) -> LinkSettings {
    std::fs::read(settings_path(app_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).map(clean).unwrap_or_default()
}

pub fn save(app_dir: &Path, s: &LinkSettings) -> Result<(), String> {
    let b = serde_json::to_vec_pretty(&clean(s.clone())).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&settings_path(app_dir), &b).map_err(|e| format!("Could not save: {e}"))
}

// ------------------------------------------------------------------------------------------------ the order of ways

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Way {
    SameNetwork,
    Direct,
    HolePunch,
    Friend,
    Community,
}

impl Way {
    /// Plain words for the page.
    pub fn words(self) -> &'static str {
        match self {
            Way::SameNetwork => "same network",
            Way::Direct => "direct",
            Way::HolePunch => "hole punching",
            Way::Friend => "through a friend you both know",
            Way::Community => "through a community relay",
        }
    }
    /// Through someone else (sealed a second time end to end).
    pub fn go_between(self) -> bool {
        matches!(self, Way::Friend | Way::Community)
    }
}

/// One thing a lookup knows about where a launcher can be reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hint {
    Ip(SocketAddr),
    /// The launcher's device key (its QUIC key). Untrusted until the device cert at connect time matches.
    Device([u8; 32]),
    /// A hole-punch helper (relay server URL) the launcher uses.
    Helper(String),
}

/// Where to find a launcher by its key. The DHT records (pkarr, `p2p/dht.rs` when it is there), friends' presence
/// messages and the LAN beacon each give hints through this one boundary.
pub trait Lookup: Send + Sync {
    fn lookup(&self, peer: &[u8; 32]) -> Vec<Hint>;
}

/// No hints (a stand-in while the DHT lookup isn't built).
pub struct NoLookup;
impl Lookup for NoLookup {
    fn lookup(&self, _peer: &[u8; 32]) -> Vec<Hint> {
        Vec::new()
    }
}

/// A fixed table of hints (tests, and hints learned in one place handed to another).
#[derive(Default)]
pub struct Table(pub std::sync::Mutex<std::collections::HashMap<[u8; 32], Vec<Hint>>>);
impl Table {
    pub fn put(&self, peer: [u8; 32], hints: Vec<Hint>) {
        self.0.lock().unwrap().insert(peer, hints);
    }
}
impl Lookup for Table {
    fn lookup(&self, peer: &[u8; 32]) -> Vec<Hint> {
        self.0.lock().unwrap().get(peer).cloned().unwrap_or_default()
    }
}

/// Everything known about how to reach one launcher.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    /// Addresses on this PC's own network (private or link-local).
    pub lan: Vec<SocketAddr>,
    /// Other addresses (public IPv4 or IPv6).
    pub direct: Vec<SocketAddr>,
    /// Hole-punch helpers both sides can use.
    pub helpers: Vec<String>,
    /// The launcher's device keys (needed for ways 1 to 3).
    pub devices: Vec<[u8; 32]>,
    /// This PC can look up a device's addresses itself (the DHT), so "direct" is worth a try with no address.
    pub finder: bool,
    /// Friends of both, online, who may pass the bytes along.
    pub friends: Vec<[u8; 32]>,
    /// Community relays (keys from verified relay cards).
    pub community: Vec<[u8; 32]>,
}

/// An address on this network: private, link-local, CGNAT shared, unique-local or loopback.
pub fn lan_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => a.is_private() || a.is_link_local() || a.is_loopback() || (a.octets()[0] == 100 && (64..128).contains(&a.octets()[1])),
        IpAddr::V6(a) => a.is_loopback() || (a.segments()[0] & 0xfe00) == 0xfc00 || (a.segments()[0] & 0xffc0) == 0xfe80,
    }
}

impl Hints {
    /// Sort hints from lookups into the kinds (duplicates dropped, unusable addresses skipped).
    pub fn add(&mut self, hints: impl IntoIterator<Item = Hint>) {
        for h in hints {
            match h {
                Hint::Ip(a) if a.port() == 0 || a.ip().is_unspecified() || a.ip().is_multicast() => {}
                Hint::Ip(a) if lan_ip(&a.ip()) => {
                    if !self.lan.contains(&a) && self.lan.len() < 8 {
                        self.lan.push(a)
                    }
                }
                Hint::Ip(a) => {
                    if !self.direct.contains(&a) && self.direct.len() < 8 {
                        self.direct.push(a)
                    }
                }
                Hint::Device(d) => {
                    if !self.devices.contains(&d) && self.devices.len() < 4 {
                        self.devices.push(d)
                    }
                }
                Hint::Helper(u) => {
                    if helper_ok(&u) && !self.helpers.contains(&u) && self.helpers.len() < 4 {
                        self.helpers.push(u)
                    }
                }
            }
        }
    }
}

/// One attempt: the way, and the go-between's key for ways 4 and 5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub way: Way,
    pub via: Option<[u8; 32]>,
}

/// The attempts in order, and plain words for the ways skipped. `target` is never its own go-between.
pub fn plan(target: &[u8; 32], h: &Hints, s: &LinkSettings) -> (Vec<Step>, Vec<String>) {
    let (mut steps, mut skipped) = (Vec::new(), Vec::new());
    if !s.on {
        return (steps, vec!["Reaching friends from anywhere is off in Settings › Network.".into()]);
    }
    let step = |way, via| Step { way, via };
    let known = !h.devices.is_empty();
    if !known {
        skipped.push("Same network, direct and hole punching: their device key isn't known yet.".into());
    } else if h.lan.is_empty() {
        skipped.push("Same network: not seen on this network.".into());
    } else {
        steps.push(step(Way::SameNetwork, None));
    }
    if known && h.direct.is_empty() && !h.finder {
        skipped.push("Direct: no address shared.".into());
    } else if known {
        steps.push(step(Way::Direct, None));
    }
    if !known {
    } else if h.helpers.is_empty() {
        skipped.push("Hole punching: no hole-punch helper known.".into());
    } else {
        steps.push(step(Way::HolePunch, None));
    }
    let friends: Vec<_> = h.friends.iter().filter(|k| *k != target).take(MAX_GO_BETWEENS).collect();
    if friends.is_empty() {
        skipped.push("A friend you both know: none online.".into());
    }
    steps.extend(friends.into_iter().map(|k| step(Way::Friend, Some(*k))));
    if !s.community {
        skipped.push("Community relays: off in Settings › Network.".into());
    } else {
        let c: Vec<_> = h.community.iter().filter(|k| *k != target && !h.friends.contains(k)).take(MAX_GO_BETWEENS).collect();
        if c.is_empty() {
            skipped.push("Community relays: none known.".into());
        }
        steps.extend(c.into_iter().map(|k| step(Way::Community, Some(*k))));
    }
    (steps, skipped)
}

// ------------------------------------------------------------------------------------------------ requests

/// The first thing on every stream: what the dialer wants. Length-prefixed JSON ([`frame_header`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "want", rename_all = "snake_case")]
pub enum Header {
    /// A service of this launcher.
    Svc { svc: String, #[serde(default)] knock: String },
    /// Pass this stream on to `to` (hex key), for its service `svc`.
    Relay { to: String, svc: String, #[serde(default)] knock: String },
    /// From a go-between: `from` (hex key) wants `svc`. A sealed handshake follows, which proves `from`.
    Relayed { from: String, svc: String, #[serde(default)] knock: String },
    /// Keep this sealed message for your friend `to` (hex key) until they're online.
    Hold { to: String },
}

/// A service name: `friend` (one sealed friend message), or `world:<realm id>:login|world` (a shared world's port).
pub fn svc_ok(s: &str) -> bool {
    if s == "friend" {
        return true;
    }
    let mut p = s.split(':');
    matches!((p.next(), p.next(), p.next(), p.next()), (Some("world"), Some(rid), Some("login" | "world"), None) if crate::realms::realm_id_ok(rid))
}

/// `u16 length (big end) + bytes`, at most [`HEADER_MAX`].
pub fn frame_header<T: Serialize>(h: &T) -> Result<Vec<u8>, String> {
    let b = serde_json::to_vec(h).map_err(|e| e.to_string())?;
    if b.len() > HEADER_MAX {
        return Err("too big".into());
    }
    let mut out = (b.len() as u16).to_be_bytes().to_vec();
    out.extend(b);
    Ok(out)
}

/// An answer: `{"ok":true}` or `{"ok":false,"error":"..."}`.
pub fn answer(r: &Result<(), String>) -> Value {
    match r {
        Ok(()) => json!({"ok": true}),
        Err(e) => json!({"ok": false, "error": e}),
    }
}

pub fn answer_result(v: &Value) -> Result<(), String> {
    if v["ok"] == true {
        Ok(())
    } else {
        Err(v["error"].as_str().unwrap_or("refused").chars().take(200).collect())
    }
}

/// The knock for a shared world: sha256 of the context, the realm id and the invite's secret (hex). Only someone
/// holding the invite can make it.
pub fn knock(realm_id: &str, secret: &str) -> String {
    use sha2::{Digest, Sha256};
    identity::hex(&Sha256::digest(format!("{CTX_KNOCK}\n{realm_id}\n{secret}").as_bytes()))
}

/// Same length, compared in full.
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ------------------------------------------------------------------------------------------------ go-between rules

/// Who a go-between carries for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Carry {
    Friends,
    Community,
}

/// A daily data cap and a cap on streams at once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Budget {
    pub day: u64,
    pub used: u64,
    pub cap: u64,
    pub live: u32,
    pub max_live: u32,
    pub refused: u64,
}

impl Budget {
    pub fn new(cap_bytes: u64, max_live: u32) -> Budget {
        Budget { cap: cap_bytes, max_live, ..Default::default() }
    }
    fn roll(&mut self, now: u64) {
        let d = now / 86_400;
        if d != self.day {
            self.day = d;
            self.used = 0;
        }
    }
    /// A new stream may start (and counts as live until [`Budget::done`]).
    pub fn admit(&mut self, now: u64) -> Result<(), String> {
        self.roll(now);
        if self.used >= self.cap {
            self.refused += 1;
            return Err("this go-between's data for today is used up".into());
        }
        if self.live >= self.max_live {
            self.refused += 1;
            return Err("this go-between is carrying as many connections as it allows".into());
        }
        self.live += 1;
        Ok(())
    }
    /// `n` more bytes passed; false once today's cap is reached (the stream is cut).
    pub fn charge(&mut self, n: u64, now: u64) -> bool {
        self.roll(now);
        self.used = self.used.saturating_add(n);
        self.used <= self.cap
    }
    pub fn done(&mut self) {
        self.live = self.live.saturating_sub(1);
    }
}

/// May this PC pass a stream from `from` to `to`? Friends first (both must be this PC's friends, and "Help friends
/// connect" on), then community (Be a node's relay switch on and the node running). Never to or from itself.
pub fn may_carry(s: &LinkSettings, node_relay_running: bool, me: &[u8; 32], from: &[u8; 32], to: &[u8; 32], is_friend: &dyn Fn(&[u8; 32]) -> bool) -> Result<Carry, String> {
    if from == to || from == me || to == me {
        return Err("not a connection this PC passes".into());
    }
    if s.on && s.help_friends && is_friend(from) && is_friend(to) {
        return Ok(Carry::Friends);
    }
    if s.community_relay && node_relay_running {
        return Ok(Carry::Community);
    }
    Err("this PC doesn't pass connections for you".into())
}

// ------------------------------------------------------------------------------------------------ the second seal

/// The end-to-end seal for a stream through a go-between: X25519 with fresh keys on both sides, the transcript signed
/// by both peer keys (so the go-between can't stand in for either), then ChaCha20-Poly1305 frames each way.
///
/// 1. dialer -> 32 bytes: its fresh public key;
/// 2. answerer -> 32 + 64: its fresh public key, its signature;
/// 3. dialer -> 64: its signature.
///
/// Signatures are over `CTX_SEAL + "\n" + role + sha256(ctx, both fresh keys, dialer key, answerer key)`.
pub struct Handshake {
    eph: ring::agreement::EphemeralPrivateKey,
    eph_pub: [u8; 32],
}

/// Keys for both directions once the handshake is done.
pub struct Sealed {
    pub tx: Sealer,
    pub rx: Opener,
}

fn transcript(e_i: &[u8], e_r: &[u8], pk_i: &[u8; 32], pk_r: &[u8; 32]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(CTX_SEAL.as_bytes());
    h.update(e_i);
    h.update(e_r);
    h.update(pk_i);
    h.update(pk_r);
    h.finalize().into()
}

fn to_sign(role: &str, t: &[u8; 32]) -> Vec<u8> {
    let mut m = format!("{CTX_SEAL}\n{role}").into_bytes();
    m.extend_from_slice(t);
    m
}

fn verify(pk: &[u8; 32], msg: &[u8], sig: &[u8]) -> bool {
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, pk).verify(msg, sig).is_ok()
}

struct KeyLen(usize);
impl ring::hkdf::KeyType for KeyLen {
    fn len(&self) -> usize {
        self.0
    }
}

fn keys(shared: &[u8], t: &[u8; 32], dialer: bool) -> Result<Sealed, String> {
    let prk = ring::hkdf::Salt::new(ring::hkdf::HKDF_SHA256, t).extract(shared);
    let key = |info: &[u8]| -> Result<ring::aead::LessSafeKey, String> {
        let mut k = [0u8; 32];
        let infos = [info];
        prk.expand(&infos, KeyLen(32)).and_then(|o| o.fill(&mut k)).map_err(|_| "key".to_string())?;
        Ok(ring::aead::LessSafeKey::new(ring::aead::UnboundKey::new(&ring::aead::CHACHA20_POLY1305, &k).map_err(|_| "key".to_string())?))
    };
    let (i2r, r2i) = (key(b"dialer to answerer")?, key(b"answerer to dialer")?);
    let (tx, rx) = if dialer { (i2r, r2i) } else { (r2i, i2r) };
    Ok(Sealed { tx: Sealer { key: tx, n: 0 }, rx: Opener { key: rx, n: 0 } })
}

fn fresh() -> Result<(ring::agreement::EphemeralPrivateKey, [u8; 32]), String> {
    let rng = ring::rand::SystemRandom::new();
    let k = ring::agreement::EphemeralPrivateKey::generate(&ring::agreement::X25519, &rng).map_err(|_| "This PC has no random source.".to_string())?;
    let p: [u8; 32] = k.compute_public_key().map_err(|_| "key".to_string())?.as_ref().try_into().map_err(|_| "key".to_string())?;
    Ok((k, p))
}

fn agree(k: ring::agreement::EphemeralPrivateKey, peer: &[u8]) -> Result<Vec<u8>, String> {
    ring::agreement::agree_ephemeral(k, &ring::agreement::UnparsedPublicKey::new(&ring::agreement::X25519, peer), |s| s.to_vec()).map_err(|_| "sealing failed".to_string())
}

impl Handshake {
    /// The dialer's first message.
    pub fn start() -> Result<(Handshake, [u8; 32]), String> {
        let (eph, eph_pub) = fresh()?;
        Ok((Handshake { eph, eph_pub }, eph_pub))
    }

    /// The answerer: message 1 in, message 2 out, and what message 3 must prove.
    pub fn answer(me: &PeerIdentity, dialer: &[u8; 32], m1: &[u8]) -> Result<(Vec<u8>, Pending), String> {
        if m1.len() != 32 {
            return Err("sealing failed".into());
        }
        let (eph, eph_pub) = fresh()?;
        let t = transcript(m1, &eph_pub, dialer, &me.public_key);
        let shared = agree(eph, m1)?;
        let mut m2 = eph_pub.to_vec();
        m2.extend_from_slice(&me.sign(&to_sign("answerer", &t)));
        Ok((m2, Pending { t, shared, dialer: *dialer }))
    }

    /// The dialer: message 2 in (checked against the key it meant to reach), message 3 out, the keys.
    pub fn finish(self, me: &PeerIdentity, answerer: &[u8; 32], m2: &[u8]) -> Result<(Vec<u8>, Sealed), String> {
        if m2.len() != 96 {
            return Err("sealing failed".into());
        }
        let t = transcript(&self.eph_pub, &m2[..32], &me.public_key, answerer);
        if !verify(answerer, &to_sign("answerer", &t), &m2[32..]) {
            return Err("the other side isn't who it should be".into());
        }
        let shared = agree(self.eph, &m2[..32])?;
        Ok((me.sign(&to_sign("dialer", &t)).to_vec(), keys(&shared, &t, true)?))
    }
}

/// The answerer's state until message 3.
pub struct Pending {
    t: [u8; 32],
    shared: Vec<u8>,
    dialer: [u8; 32],
}

impl Pending {
    pub fn confirm(self, m3: &[u8]) -> Result<Sealed, String> {
        if m3.len() != 64 || !verify(&self.dialer, &to_sign("dialer", &self.t), m3) {
            return Err("the other side isn't who it says".into());
        }
        keys(&self.shared, &self.t, false)
    }
}

fn nonce(n: u64) -> ring::aead::Nonce {
    let mut b = [0u8; 12];
    b[4..].copy_from_slice(&n.to_be_bytes());
    ring::aead::Nonce::assume_unique_for_key(b)
}

pub struct Sealer {
    key: ring::aead::LessSafeKey,
    n: u64,
}

impl Sealer {
    /// One frame: `u16 length (big end) + sealed bytes`, at most [`FRAME_MAX`] plain bytes.
    pub fn seal(&mut self, plain: &[u8]) -> Result<Vec<u8>, String> {
        if plain.len() > FRAME_MAX {
            return Err("too big".into());
        }
        let mut b = plain.to_vec();
        self.key.seal_in_place_append_tag(nonce(self.n), ring::aead::Aad::empty(), &mut b).map_err(|_| "seal".to_string())?;
        self.n += 1;
        let mut out = (b.len() as u16).to_be_bytes().to_vec();
        out.extend(b);
        Ok(out)
    }
}

pub struct Opener {
    key: ring::aead::LessSafeKey,
    n: u64,
}

impl Opener {
    /// The sealed bytes of one frame (without its length): the plain bytes, or an error for anything changed,
    /// dropped, repeated or reordered.
    pub fn open(&mut self, sealed: &[u8]) -> Result<Vec<u8>, String> {
        let mut b = sealed.to_vec();
        let n = self.key.open_in_place(nonce(self.n), ring::aead::Aad::empty(), &mut b).map_err(|_| "a sealed frame didn't open".to_string())?.len();
        self.n += 1;
        b.truncate(n);
        Ok(b)
    }
}

// ------------------------------------------------------------------------------------------------ held messages

/// Sealed friend messages this PC keeps for a friend who is offline, handed over when they're reachable. Only from a
/// friend, only for a friend; the messages are sealed to the recipient, so this PC can't read them.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Held {
    pub items: Vec<HeldItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeldItem {
    pub to: String,
    pub from: String,
    pub env: Value,
    pub at: u64,
}

pub const HELD_PER_TO: usize = 32;
pub const HELD_PER_FROM: usize = 64;
pub const HELD_TOTAL: usize = 256;
pub const HELD_TTL: u64 = 7 * 86_400;

impl Held {
    pub fn load(app: &Path) -> Held {
        std::fs::read(app.join(HELD_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    pub fn save(&self, app: &Path) -> Result<(), String> {
        let b = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        crate::settings::write_atomic(&app.join(HELD_FILE), &b).map_err(|e| e.to_string())
    }
    pub fn sweep(&mut self, now: u64) {
        self.items.retain(|i| now.saturating_sub(i.at) <= HELD_TTL);
    }
    /// Keep `env` (a sealed envelope addressed to `to`) from `from`, within the caps.
    pub fn put(&mut self, from: &str, to: &str, env: Value, now: u64, from_friend: bool, to_friend: bool) -> Result<(), String> {
        if !from_friend || !to_friend {
            return Err("only between friends of this PC".into());
        }
        if env["to"].as_str() != identity::unhex32(to).map(|k| identity::peer_id(&k)).as_deref() {
            return Err("that message isn't for them".into());
        }
        if env.to_string().len() > super::friends::ENVELOPE_MAX {
            return Err("too big".into());
        }
        self.sweep(now);
        if self.items.iter().any(|i| i.env == env) {
            return Ok(());
        }
        if self.items.iter().filter(|i| i.to == to).count() >= HELD_PER_TO {
            return Err("this PC is keeping as many messages for them as it allows".into());
        }
        if self.items.iter().filter(|i| i.from == from).count() >= HELD_PER_FROM {
            return Err("this PC is keeping as many of your messages as it allows".into());
        }
        if self.items.len() >= HELD_TOTAL {
            return Err("this PC is keeping as many messages as it allows".into());
        }
        self.items.push(HeldItem { to: to.into(), from: from.into(), env, at: now });
        Ok(())
    }
    /// Who has messages waiting.
    pub fn waiting(&self) -> Vec<String> {
        let mut v: Vec<String> = self.items.iter().map(|i| i.to.clone()).collect();
        v.sort();
        v.dedup();
        v
    }
    /// The messages for `to`, oldest first (taken out).
    pub fn take_for(&mut self, to: &str) -> Vec<Value> {
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.items).into_iter().partition(|i| i.to == to);
        self.items = rest;
        mine.into_iter().map(|i| i.env).collect()
    }
}

// ------------------------------------------------------------------------------------------------ relay cards

/// A community relay's card: a mesh relay record (`leyline-mesh-node-v1`, p2p/mesh.rs) whose host is the node's
/// own name under `.link.invalid` (it has no DNS name; it is reached by key), with a `link` part saying it carries
/// sealed connections, and the hole-punch helper it offers if any.
pub fn relay_card(me: &PeerIdentity, streams: (u64, u64), kbps: u64, helper: Option<&str>, now: u64) -> Value {
    let host = format!("{}.link.invalid", me.peer_id);
    let mut link = json!({"v": 1, "alpn": String::from_utf8_lossy(ALPN)});
    if let Some(h) = helper.filter(|h| helper_ok(h)) {
        link["helper"] = json!(h);
    }
    let n = json!({"v": 1, "pubkey": me.public_hex(), "host": host, "http": format!("http://{host}"), "seq": now * 1000, "ts": now,
        "region": "", "volunteer": true, "seed": false, "ws": false, "mesh": true,
        "caps": {"realms": 0, "streams": streams.1, "kbps": kbps}, "load": {"realms": 0, "streams": streams.0}, "link": link});
    json!({"kind": "relay", "sig": me.sign_doc(CTX_NODE, &n), "node": n})
}

/// A relay card's key and helper, when the record verifies (the mesh's own check) and carries a `link` part.
pub fn check_relay_card(rec: &Value, now: u64) -> Option<([u8; 32], Option<String>)> {
    let (_, _, rec) = super::mesh::check(rec, now, super::mesh::STAFF_KEYS).ok()?;
    let n = &rec["node"];
    if n["link"]["v"].as_u64() != Some(1) || n["link"]["alpn"].as_str() != Some(std::str::from_utf8(ALPN).ok()?) {
        return None;
    }
    let pk = identity::unhex32(n["pubkey"].as_str()?)?;
    Some((pk, n["link"]["helper"].as_str().filter(|h| helper_ok(h)).map(String::from)))
}

// ------------------------------------------------------------------------------------------------ invite links

/// The invite link with the host's peer key and device key, so a friend's launcher can dial it from anywhere.
/// Never an address. (The device key is checked against the peer key at connect time.)
pub fn with_key(link: &str, pub_hex: &str, device_hex: &str) -> String {
    if identity::unhex32(pub_hex).is_none() || identity::unhex32(device_hex).is_none() || key_in(link).is_some() {
        return link.to_string();
    }
    format!("{link}~~key={}~~dev={}", pub_hex.to_ascii_lowercase(), device_hex.to_ascii_lowercase())
}

fn part(link: &str, name: &str) -> Option<[u8; 32]> {
    let (_, frag) = link.split_once('#')?;
    frag.split("~~").find_map(|p| p.strip_prefix(name)).and_then(identity::unhex32)
}

pub fn key_in(link: &str) -> Option<[u8; 32]> {
    part(link, "key=")
}

pub fn device_in(link: &str) -> Option<[u8; 32]> {
    part(link, "dev=")
}

// ------------------------------------------------------------------------------------------------ for the app

/// The DHT address lookup (pkarr records, `p2p/dht.rs`) once that module is there; none until then.
pub fn dht_lookup() -> Option<std::sync::Arc<dyn Lookup>> {
    None
}

/// Offer a shared world's ports (the login front and the world port on 127.0.0.1) to friends holding its invite,
/// from anywhere. Starts this PC's connections if they aren't running.
pub fn offer_world(realm_id: &str, secret: &str, login: u16, world: u16) -> Result<(), String> {
    #[cfg(all(feature = "link", desktop))]
    {
        super::linknet::ensure()?.offer_world(realm_id, secret, login, world);
        Ok(())
    }
    #[cfg(not(all(feature = "link", desktop)))]
    {
        let _ = (realm_id, secret, login, world);
        Err("not in this build".into())
    }
}

pub fn withdraw_world(realm_id: &str) {
    #[cfg(all(feature = "link", desktop))]
    if let Some(l) = super::linknet::current() {
        l.withdraw_world(realm_id);
    }
    #[cfg(not(all(feature = "link", desktop)))]
    let _ = realm_id;
}

/// The invite link with this PC's key when its connections run.
pub fn with_my_key(link: &str) -> String {
    #[cfg(all(feature = "link", desktop))]
    if let Some(l) = super::linknet::current() {
        return with_key(link, &identity::hex(&l.public_key()), &identity::hex(&l.device_key()));
    }
    link.to_string()
}

/// A friend's world from anywhere, by the host key in the invite link: this PC's forwarder on 127.0.0.1 and the way
/// it reached the host. None when the link has no key (an older invite) or this build has no connections.
pub fn guest_join(link: &str, realm_id: &str, secret: &str) -> Option<Result<(String, Way), String>> {
    let host = key_in(link)?;
    #[cfg(all(feature = "link", desktop))]
    {
        let l = match super::linknet::ensure() {
            Ok(l) => l,
            Err(e) => return Some(Err(e)),
        };
        if let Some(d) = device_in(link) {
            l.learn_device(host, d);
        }
        let (rid, secret) = (realm_id.to_string(), secret.to_string());
        Some(super::linknet::block(async move { super::linknet::guest_start(&l, host, &rid, &secret, crate::host::worldshare::Layout::Wrath).await }))
    }
    #[cfg(not(all(feature = "link", desktop)))]
    {
        let _ = (host, realm_id, secret);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(b: u8) -> [u8; 32] {
        [b; 32]
    }

    #[test]
    fn the_ways_come_in_order_and_skips_are_said() {
        let s = LinkSettings::default();
        let mut h = Hints::default();
        h.add([Hint::Ip("192.168.1.9:41192".parse().unwrap()), Hint::Ip("[2001:db8::5]:41192".parse().unwrap()), Hint::Ip("203.0.113.4:5000".parse().unwrap()),
            Hint::Helper("https://helper.example.net".into()), Hint::Ip("0.0.0.0:1".parse().unwrap()), Hint::Helper("not a url".into()), Hint::Device(k(8))]);
        assert_eq!(h.lan.len(), 1);
        assert_eq!(h.direct.len(), 2, "IPv6 and a public IPv4 are direct; unspecified is dropped");
        assert_eq!(h.helpers, vec!["https://helper.example.net".to_string()]);
        h.friends = vec![k(2), k(9), k(3), k(4), k(5)];
        h.community = vec![k(3), k(7)];
        let (steps, skipped) = plan(&k(9), &h, &s);
        let ways: Vec<Way> = steps.iter().map(|s| s.way).collect();
        assert_eq!(ways, vec![Way::SameNetwork, Way::Direct, Way::HolePunch, Way::Friend, Way::Friend, Way::Friend, Way::Community]);
        assert_eq!(steps[3].via, Some(k(2)));
        assert!(steps.iter().all(|s| s.via != Some(k(9))), "never the target as its own go-between");
        assert_eq!(steps[6].via, Some(k(7)), "a friend who also runs a relay is tried once, as a friend");
        assert!(skipped.is_empty());
        // nothing known: only skips, in plain words
        let (steps, skipped) = plan(&k(9), &Hints::default(), &s);
        assert!(steps.is_empty());
        assert_eq!(skipped.len(), 3);
        // addresses but no device key: only go-betweens
        let (steps, _) = plan(&k(9), &Hints { devices: vec![], ..h.clone() }, &s);
        assert!(steps.iter().all(|s| s.way.go_between()));
        // a device key and a DHT finder: direct is worth a try with no address
        let (steps, _) = plan(&k(9), &Hints { devices: vec![k(8)], finder: true, ..Hints::default() }, &s);
        assert_eq!(steps.iter().map(|s| s.way).collect::<Vec<_>>(), vec![Way::Direct]);
        // community off; everything off
        let (steps, _) = plan(&k(9), &h, &LinkSettings { community: false, ..s.clone() });
        assert!(steps.iter().all(|s| s.way != Way::Community));
        let (steps, skipped) = plan(&k(9), &h, &LinkSettings { on: false, ..s });
        assert!(steps.is_empty() && skipped[0].contains("off"));
        assert!(Way::Friend.go_between() && Way::Community.go_between() && !Way::HolePunch.go_between());
    }

    #[test]
    fn friends_transport_is_on_by_default_and_needs_no_site_flag() {
        let s = LinkSettings::default();
        assert!(s.on && s.help_friends && s.community && !s.community_relay && !s.router_port);
        let d = tempfile::tempdir().unwrap();
        assert_eq!(load(d.path()), s, "no file: the defaults");
        save(d.path(), &LinkSettings { help_day_mb: 1, help_streams: 999, helpers: vec!["https://ok.example".into(), "bad url".into()], ..s }).unwrap();
        let l = load(d.path());
        assert_eq!((l.help_day_mb, l.help_streams, l.helpers.len()), (HELP_DAY_MB.0, HELP_STREAMS.1, 1));
    }

    #[test]
    fn who_a_go_between_carries_for() {
        let s = LinkSettings::default();
        let friends = |x: &[u8; 32]| *x == k(2) || *x == k(3);
        let me = k(1);
        assert_eq!(may_carry(&s, false, &me, &k(2), &k(3), &friends), Ok(Carry::Friends));
        assert!(may_carry(&s, false, &me, &k(2), &k(4), &friends).is_err(), "the other side isn't my friend");
        assert!(may_carry(&s, false, &me, &k(2), &k(2), &friends).is_err());
        assert!(may_carry(&s, false, &me, &k(2), &me, &friends).is_err());
        let off = LinkSettings { help_friends: false, ..s.clone() };
        assert!(may_carry(&off, false, &me, &k(2), &k(3), &friends).is_err(), "Help friends connect off");
        let node = LinkSettings { community_relay: true, ..s };
        assert_eq!(may_carry(&node, true, &me, &k(5), &k(6), &friends), Ok(Carry::Community));
        assert!(may_carry(&node, false, &me, &k(5), &k(6), &friends).is_err(), "the node isn't running");
    }

    #[test]
    fn caps_refuse_and_cut() {
        let mut b = Budget::new(1000, 2);
        let day = 20_000 * 86_400;
        assert!(b.admit(day).is_ok() && b.admit(day).is_ok());
        assert!(b.admit(day).unwrap_err().contains("as many connections"));
        b.done();
        assert!(b.charge(600, day));
        assert!(!b.charge(600, day), "over the day's cap: cut");
        assert!(b.admit(day).unwrap_err().contains("used up"));
        assert_eq!(b.refused, 2);
        assert!(b.admit(day + 86_400).is_ok(), "a new day");
    }

    #[test]
    fn the_second_seal_works_and_the_go_between_cant_stand_in() {
        let a = PeerIdentity::from_seed([1; 32]).unwrap();
        let b = PeerIdentity::from_seed([2; 32]).unwrap();
        let mallory = PeerIdentity::from_seed([3; 32]).unwrap();
        let (hs, m1) = Handshake::start().unwrap();
        let (m2, pending) = Handshake::answer(&b, &a.public_key, &m1).unwrap();
        let (m3, mut sa) = hs.finish(&a, &b.public_key, &m2).unwrap();
        let mut sb = pending.confirm(&m3).unwrap();
        let f = sa.tx.seal(b"hello").unwrap();
        assert_eq!(u16::from_be_bytes([f[0], f[1]]) as usize, f.len() - 2);
        assert!(!f.windows(5).any(|w| w == b"hello"), "sealed");
        assert_eq!(sb.rx.open(&f[2..]).unwrap(), b"hello");
        let g = sb.tx.seal(b"back").unwrap();
        assert_eq!(sa.rx.open(&g[2..]).unwrap(), b"back");
        // a repeated or changed frame doesn't open
        assert!(sb.rx.open(&f[2..]).is_err());
        let mut h = sa.tx.seal(b"x").unwrap();
        h[3] ^= 1;
        assert!(sb.rx.open(&h[2..]).is_err());
        assert!(sa.tx.seal(&vec![0u8; FRAME_MAX + 1]).is_err());
        // someone in the middle answering as b is caught by the dialer
        let (hs, m1) = Handshake::start().unwrap();
        let (m2, _) = Handshake::answer(&mallory, &a.public_key, &m1).unwrap();
        assert!(hs.finish(&a, &b.public_key, &m2).is_err());
        // and dialing as a without a's key is caught by the answerer
        let (hs, m1) = Handshake::start().unwrap();
        let (m2, pending) = Handshake::answer(&b, &a.public_key, &m1).unwrap();
        drop(hs);
        let forged = mallory.sign(&to_sign("dialer", &transcript(&m1, &m2[..32], &a.public_key, &b.public_key)));
        assert!(pending.confirm(&forged).is_err());
    }

    #[test]
    fn held_messages_have_caps_and_go_only_between_friends() {
        let b = PeerIdentity::from_seed([2; 32]).unwrap();
        let to = b.public_hex();
        let env = |n: u32| json!({"v": 1, "to": b.peer_id, "box": format!("sealed{n}")});
        let mut h = Held::default();
        assert!(h.put("aa", &to, env(0), 100, false, true).is_err());
        assert!(h.put("aa", &to, json!({"to": "someoneelse"}), 100, true, true).is_err());
        for n in 0..HELD_PER_TO as u32 {
            h.put("aa", &to, env(n), 100, true, true).unwrap();
        }
        assert!(h.put("aa", &to, env(0), 100, true, true).is_ok(), "the same message again is kept once");
        assert!(h.put("aa", &to, env(999), 100, true, true).unwrap_err().contains("for them"));
        assert_eq!(h.waiting(), vec![to.clone()]);
        let d = tempfile::tempdir().unwrap();
        h.save(d.path()).unwrap();
        let mut h2 = Held::load(d.path());
        assert_eq!(h2.take_for(&to).len(), HELD_PER_TO);
        assert!(h2.items.is_empty());
        h.sweep(100 + HELD_TTL + 1);
        assert!(h.items.is_empty(), "old messages are dropped");
    }

    #[test]
    fn relay_cards_are_mesh_relay_records() {
        let me = PeerIdentity::from_seed([4; 32]).unwrap();
        let now = super::super::mesh::now_secs();
        let card = relay_card(&me, (1, 20), 256, Some("https://helper.example.net"), now);
        assert!(super::super::mesh::check(&card, now, &[]).is_ok(), "the mesh reads it as a relay record");
        assert_eq!(check_relay_card(&card, now), Some((me.public_key, Some("https://helper.example.net".into()))));
        let mut t = card.clone();
        t["node"]["caps"]["streams"] = json!(999);
        assert!(check_relay_card(&t, now).is_none(), "tampered");
        assert!(check_relay_card(&card, now + 3 * 3600).is_none(), "expired");
    }

    #[test]
    fn requests_and_links() {
        assert!(svc_ok("friend") && svc_ok("world:kzdvvj2umnduyauf:login") && svc_ok("world:kzdvvj2umnduyauf:world"));
        assert!(!svc_ok("world:kzdvvj2umnduyauf:soap") && !svc_ok("world:x:login") && !svc_ok("shell"));
        let f = frame_header(&Header::Relay { to: "ab".into(), svc: "friend".into(), knock: String::new() }).unwrap();
        let back: Header = serde_json::from_slice(&f[2..]).unwrap();
        assert_eq!(back, Header::Relay { to: "ab".into(), svc: "friend".into(), knock: String::new() });
        assert!(frame_header(&json!({"x": "y".repeat(HEADER_MAX)})).is_err());
        assert_eq!(answer_result(&answer(&Err("no".into()))), Err("no".into()));
        assert!(answer_result(&answer(&Ok(()))).is_ok());
        let kn = knock("kzdvvj2umnduyauf", "S3CRET");
        assert_eq!(kn.len(), 64);
        assert!(same(&kn, &knock("kzdvvj2umnduyauf", "S3CRET")) && !same(&kn, &knock("kzdvvj2umnduyauf", "other")));
        let base = "https://relay.example/join/kzdvvj2umnduyauf#ABCD~~GABC.PASS";
        let (pk, dev) = (identity::hex(&[7u8; 32]), identity::hex(&[6u8; 32]));
        let l = with_key(base, &pk, &dev);
        assert_eq!((key_in(&l), device_in(&l)), (Some([7u8; 32]), Some([6u8; 32])));
        assert_eq!(with_key(&l, &pk, &dev), l, "once");
        assert_eq!(with_key(base, "zz", &dev), base);
        assert_eq!(key_in(base), None);
        assert!(crate::host::denways::ways_in(&l) == (None, None), "the other ways' reader skips it");
    }
}
