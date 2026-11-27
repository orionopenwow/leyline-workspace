//! Address records on the public Mainline DHT (docs/leyline/network.md, "Finding each other with no centre").
//!
//! A realm whose host chose to list it, or a Passport root, publishes one small signed pointer under its own Ed25519
//! key: the connection endpoints (iroh endpoint ids) it is served from, address hints the owner chose to publish, the
//! sha256 of its full signed mesh record and where that record can be fetched. Anyone who knows the key can find the
//! packet; nobody else can write it.
//!
//! **Never under the device key** (the Passport's `passport/device.key`, the iroh endpoint key): the transport
//! (p2p/link.rs, iroh) publishes its own packet under that key, and a key has one packet on the DHT, so ours would
//! overwrite it. `set_job` refuses it. The Passport root (`peer.key`) is a different key and carries the Passport
//! pointer.
//!
//! * **Format:** a pkarr signed packet (BEP 44 mutable item, no salt, `seq` = microseconds): the value is a DNS
//!   reply of TXT records named `_leyline.<z-base-32 key>`, one string each: `v=1`, `k=realm|passport`, `t=<unix
//!   time>`, `e=<endpoint id, 64 hex>` (up to 4), `a=<host:port>` (up to 8), `h=<sha256 of the mesh record>`,
//!   `f=peer|https://...` (up to 4). At most 1000
//!   bytes (BEP 44's limit). Any pkarr reader can resolve it; this file reads only what it wrote.
//! * **No game data, no email, no IP beyond the hints:** the packet holds only what [`Hints`] allows, checked on the
//!   way out and on the way in.
//! * **Client only:** the launcher asks the DHT (`mainline`, MIT, the synchronous client); it answers no one's
//!   queries and runs no HTTP relay. Bootstrap: the DHT's usual routers plus the nodes this PC met last time
//!   (`cache/dht-nodes.json`).
//! * **Switch:** `dht` (p2p/flags.rs): on by default, this PC's own setting only. Resolving needs nothing else;
//!   publishing happens only for what the player or host chose ([`publish_realm`], [`publish_passport`]).
//! * **The transport's boundary:** [`lookup`] (peer id or key in; endpoint ids and address hints out).
//!
//! Keys are never logged: errors name a peer id at most.

use mainline::MutableItem;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// BEP 44's limit on a mutable item's value.
pub const MAX_PACKET: usize = 1000;
pub const MAX_ADDRS: usize = 8;
pub const MAX_FETCH: usize = 4;
pub const MAX_ENDPOINTS: usize = 4;
/// How often a published packet is put again (DHT nodes forget items after about two hours).
pub const REPUBLISH: u64 = 30 * 60;
/// A resolved packet is used without asking again for this long.
pub const FRESH: u64 = 5 * 60;
/// A packet older than this (by its own `t=`) is not used at all.
pub const MAX_AGE: u64 = 3 * 3600;
/// Keys remembered in the hints cache.
const CACHE_KEYS: usize = 512;
const TTL: u32 = 300;
const NAME: &str = "_leyline";
const Z32: &[u8] = b"ybndrfg8ejkmcpqxot1uwisza345h769";

fn now() -> u64 {
    super::mesh::now_secs()
}

/// z-base-32 of a key (pkarr's public-key form, 52 letters for 32 bytes).
pub fn z32(data: &[u8]) -> String {
    let (mut out, mut buf, mut bits) = (String::new(), 0u32, 0u32);
    for b in data {
        buf = (buf << 8) | *b as u32;
        bits += 8;
        while bits >= 5 {
            out.push(Z32[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(Z32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

// ------------------------------------------------------------------------------------------------ what a packet says

/// What one packet says. Every field is checked both ways ([`Hints::check`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    /// "realm" or "passport".
    pub kind: String,
    /// When it was made (unix seconds).
    pub ts: u64,
    /// The connection endpoints (iroh endpoint ids: a device's Ed25519 public key, 64 lower-case hex) to dial.
    pub endpoints: Vec<String>,
    /// Where the key's owner can be reached: `host:port`, `a.b.c.d:port` or `[v6]:port`, as the owner chose.
    pub addrs: Vec<String>,
    /// sha256 (hex) of the owner's full signed mesh record, so a reader knows which record is current.
    pub record_sha256: Option<String>,
    /// Where the full record can be fetched: `peer` (ask the owner at its addresses) or an https address.
    pub fetch: Vec<String>,
}

pub fn addr_ok(a: &str) -> bool {
    let (host, port) = match a.rsplit_once(':') {
        Some(x) => x,
        None => return false,
    };
    if !port.parse::<u16>().is_ok_and(|p| p > 0) || a.len() > 100 || host.is_empty() {
        return false;
    }
    if let Some(v6) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return v6.parse::<std::net::Ipv6Addr>().is_ok();
    }
    host.parse::<std::net::Ipv4Addr>().is_ok()
        || (host.len() <= 253 && host.contains('.') && host.split('.').all(|l| !l.is_empty() && l.len() <= 63
            && l.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') && !l.starts_with('-') && !l.ends_with('-')))
}

fn fetch_ok(f: &str) -> bool {
    f == "peer" || (crate::net::plain_https(f) && f.len() <= 200 && !f.contains(char::is_whitespace))
}

impl Hints {
    pub fn check(&self) -> Result<(), String> {
        if !matches!(self.kind.as_str(), "realm" | "passport") {
            return Err("kind".into());
        }
        let hex64 = |e: &String| e.len() == 64 && e.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c));
        if self.endpoints.len() > MAX_ENDPOINTS || !self.endpoints.iter().all(hex64) {
            return Err("endpoint ids".into());
        }
        if self.addrs.len() > MAX_ADDRS || !self.addrs.iter().all(|a| addr_ok(a)) {
            return Err("address hints".into());
        }
        if self.fetch.len() > MAX_FETCH || !self.fetch.iter().all(|f| fetch_ok(f)) {
            return Err("fetch pointers".into());
        }
        if self.record_sha256.as_ref().is_some_and(|h| h.len() != 64 || !h.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))) {
            return Err("record hash".into());
        }
        Ok(())
    }

    fn strings(&self) -> Vec<String> {
        let mut v = vec!["v=1".to_string(), format!("k={}", self.kind), format!("t={}", self.ts)];
        v.extend(self.endpoints.iter().map(|e| format!("e={e}")));
        v.extend(self.addrs.iter().map(|a| format!("a={a}")));
        v.extend(self.record_sha256.iter().map(|h| format!("h={h}")));
        v.extend(self.fetch.iter().map(|f| format!("f={f}")));
        v
    }

    pub fn to_json(&self) -> Value {
        json!({"kind": self.kind, "ts": self.ts, "endpoints": self.endpoints, "addrs": self.addrs, "record_sha256": self.record_sha256, "fetch": self.fetch})
    }

    fn from_json(v: &Value) -> Option<Hints> {
        let list = |k: &str| v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default();
        let h = Hints {
            kind: v.get("kind")?.as_str()?.to_string(),
            ts: v.get("ts")?.as_u64()?,
            endpoints: list("endpoints"),
            addrs: list("addrs"),
            record_sha256: v.get("record_sha256").and_then(Value::as_str).map(String::from),
            fetch: list("fetch"),
        };
        h.check().ok().map(|_| h)
    }
}

// ------------------------------------------------------------------------------------------------ the DNS packet

/// The packet's bytes: a DNS reply, the first name written out and the rest pointing at it.
pub fn encode(h: &Hints, key: &[u8; 32]) -> Result<Vec<u8>, String> {
    h.check()?;
    let strings = h.strings();
    let mut p = vec![0, 0, 0x84, 0, 0, 0];
    p.extend((strings.len() as u16).to_be_bytes());
    p.extend([0, 0, 0, 0]);
    for (i, s) in strings.iter().enumerate() {
        if s.len() > 255 {
            return Err("a hint is too long".into());
        }
        if i == 0 {
            for label in [NAME.to_string(), z32(key)] {
                p.push(label.len() as u8);
                p.extend(label.as_bytes());
            }
            p.push(0);
        } else {
            p.extend([0xC0, 12]);
        }
        p.extend(16u16.to_be_bytes());
        p.extend(1u16.to_be_bytes());
        p.extend(TTL.to_be_bytes());
        p.extend(((s.len() + 1) as u16).to_be_bytes());
        p.push(s.len() as u8);
        p.extend(s.as_bytes());
    }
    if p.len() > MAX_PACKET {
        return Err(format!("the packet is {} bytes (at most {MAX_PACKET})", p.len()));
    }
    Ok(p)
}

fn read_name(p: &[u8], mut i: usize) -> Option<(String, usize)> {
    let (mut labels, mut end, mut jumps) = (Vec::new(), None, 0);
    loop {
        let len = *p.get(i)? as usize;
        if len == 0 {
            return Some((labels.join("."), end.unwrap_or(i + 1)));
        }
        if len & 0xC0 == 0xC0 {
            let to = ((len & 0x3F) << 8) | *p.get(i + 1)? as usize;
            end.get_or_insert(i + 2);
            jumps += 1;
            if jumps > 8 || to >= i {
                return None;
            }
            i = to;
            continue;
        }
        labels.push(String::from_utf8(p.get(i + 1..i + 1 + len)?.to_vec()).ok()?.to_ascii_lowercase());
        i += 1 + len;
    }
}

/// What a packet under `key` says: only `_leyline.<key>` TXT records are read, everything else is skipped.
pub fn decode(p: &[u8], key: &[u8; 32]) -> Result<Hints, String> {
    if p.len() > MAX_PACKET || p.len() < 12 {
        return Err("packet size".into());
    }
    let want = format!("{NAME}.{}", z32(key));
    let count = |at: usize| u16::from_be_bytes([p[at], p[at + 1]]) as usize;
    let (qd, an) = (count(4), count(6));
    let mut i = 12;
    for _ in 0..qd {
        i = read_name(p, i).ok_or("question")?.1 + 4;
    }
    let mut strings = Vec::new();
    for _ in 0..an.min(64) {
        let (name, j) = read_name(p, i).ok_or("record name")?;
        let hdr = p.get(j..j + 10).ok_or("record")?;
        let (ty, rdlen) = (u16::from_be_bytes([hdr[0], hdr[1]]), u16::from_be_bytes([hdr[8], hdr[9]]) as usize);
        let rd = p.get(j + 10..j + 10 + rdlen).ok_or("record data")?;
        i = j + 10 + rdlen;
        if ty != 16 || name != want {
            continue;
        }
        let mut k = 0;
        while k < rd.len() {
            let n = rd[k] as usize;
            let s = rd.get(k + 1..k + 1 + n).ok_or("text")?;
            strings.push(String::from_utf8(s.to_vec()).map_err(|_| "text")?);
            k += 1 + n;
        }
    }
    let mut h = Hints::default();
    let mut v1 = false;
    for s in strings {
        let Some((k, v)) = s.split_once('=') else { continue };
        match k {
            "v" => v1 = v == "1",
            "k" => h.kind = v.to_string(),
            "t" => h.ts = v.parse().map_err(|_| "time")?,
            "a" if h.addrs.len() < MAX_ADDRS => h.addrs.push(v.to_string()),
            "e" if h.endpoints.len() < MAX_ENDPOINTS => h.endpoints.push(v.to_string()),
            "h" => h.record_sha256 = Some(v.to_string()),
            "f" if h.fetch.len() < MAX_FETCH => h.fetch.push(v.to_string()),
            _ => {}   // a newer writer's field
        }
    }
    if !v1 {
        return Err("not a Leyline packet".into());
    }
    h.check()?;
    Ok(h)
}

// ------------------------------------------------------------------------------------------------ signing

/// A key that can sign packets. The private half stays in the closure (a seed, or the launcher's identity).
#[derive(Clone)]
pub struct Key {
    pub public: [u8; 32],
    sign: Arc<dyn Fn(&[u8]) -> [u8; 64] + Send + Sync>,
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Key").field("peer_id", &crate::identity::peer_id(&self.public)).finish()
    }
}

impl Key {
    pub fn new(public: [u8; 32], sign: Arc<dyn Fn(&[u8]) -> [u8; 64] + Send + Sync>) -> Key {
        Key { public, sign }
    }

    /// From a 32-byte Ed25519 seed (a realm's `realm.key`).
    pub fn from_seed(seed: [u8; 32]) -> Result<Key, String> {
        use ring::signature::{Ed25519KeyPair, KeyPair};
        let kp = Ed25519KeyPair::from_seed_unchecked(&seed).map_err(|_| "bad key".to_string())?;
        let public: [u8; 32] = kp.public_key().as_ref().try_into().map_err(|_| "bad key".to_string())?;
        Ok(Key::new(public, Arc::new(move |m: &[u8]| {
            let mut o = [0u8; 64];
            o.copy_from_slice(kp.sign(m).as_ref());
            o
        })))
    }

    /// From an identity (a Passport root's, never the device's: see the file's head).
    pub fn from_identity(me: Arc<crate::identity::PeerIdentity>) -> Key {
        let public = me.public_key;
        Key::new(public, Arc::new(move |m: &[u8]| me.sign(m)))
    }
}

/// BEP 44's signed bytes for an item with no salt.
pub fn signable(seq: i64, value: &[u8]) -> Vec<u8> {
    let mut s = format!("3:seqi{seq}e1:v{}:", value.len()).into_bytes();
    s.extend_from_slice(value);
    s
}

/// The signed item for `hints`, `seq` = microseconds (pkarr's convention: always above the last one).
pub fn item(key: &Key, h: &Hints, seq: i64) -> Result<MutableItem, String> {
    let v = encode(h, &key.public)?;
    let sig = (key.sign)(&signable(seq, &v));
    Ok(MutableItem::new_signed_unchecked(key.public, sig, &v, seq, None))
}

/// Check an item from the DHT again here (the client checks too): the key's signature over seq and value, no salt.
pub fn verify(it: &MutableItem, key: &[u8; 32]) -> Result<Hints, String> {
    if it.key() != key || it.salt().is_some() {
        return Err("another key".into());
    }
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key)
        .verify(&signable(it.seq(), it.value()), it.signature())
        .map_err(|_| "signature".to_string())?;
    decode(it.value(), key)
}

// ------------------------------------------------------------------------------------------------ the DHT

/// Where items are put and got: the real DHT, or a test network.
pub trait Board: Send + Sync {
    fn put(&self, item: MutableItem) -> Result<(), String>;
    fn get(&self, key: &[u8; 32]) -> Option<MutableItem>;
}

#[allow(deprecated)] // mainline keeps its synchronous API (deprecated for its async one); this file needs no runtime
impl Board for mainline::Dht {
    fn put(&self, item: MutableItem) -> Result<(), String> {
        self.put_mutable(item, None).map(|_| ()).map_err(|e| e.to_string())
    }
    fn get(&self, key: &[u8; 32]) -> Option<MutableItem> {
        self.get_mutable_most_recent(key, None)
    }
}

pub fn nodes_path(app: &Path) -> PathBuf {
    app.join("cache").join("dht-nodes.json")
}

fn load_nodes(app: &Path) -> Vec<String> {
    std::fs::read(nodes_path(app)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("nodes").and_then(Value::as_array).cloned()).unwrap_or_default()
        .iter().filter_map(Value::as_str).filter(|a| a.parse::<std::net::SocketAddrV4>().is_ok()).take(64).map(String::from).collect()
}

/// Remember nodes this PC met, for the next start (no router needed then).
#[allow(deprecated)]
pub fn save_nodes(app: &Path, dht: &mainline::Dht) {
    let nodes: Vec<String> = dht.to_bootstrap().into_iter().take(64).collect();
    if !nodes.is_empty() {
        let _ = std::fs::create_dir_all(app.join("cache"));
        let _ = crate::settings::write_atomic(&nodes_path(app), json!({"nodes": nodes}).to_string().as_bytes());
    }
}

static CLIENT: OnceLock<Mutex<Option<Arc<mainline::Dht>>>> = OnceLock::new();

/// The DHT client, started once (a UDP socket on any port; bootstrap from the usual routers and the cached nodes).
pub fn client(app: &Path) -> Result<Arc<mainline::Dht>, String> {
    let mut g = CLIENT.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(d) = g.as_ref() {
        return Ok(d.clone());
    }
    let cached = load_nodes(app);
    let d = Arc::new(mainline::Dht::builder().extra_bootstrap(&cached).build().map_err(|e| format!("The DHT didn't start: {e}"))?);
    *g = Some(d.clone());
    Ok(d)
}

// ------------------------------------------------------------------------------------------------ the hints cache

pub fn cache_path(app: &Path) -> PathBuf {
    app.join("cache").join("dht-hints.json")
}

fn load_cache(app: &Path) -> Map<String, Value> {
    std::fs::read(cache_path(app)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("keys").and_then(Value::as_object).cloned()).unwrap_or_default()
}

fn save_cache(app: &Path, m: &Map<String, Value>) {
    let _ = std::fs::create_dir_all(app.join("cache"));
    let _ = crate::settings::write_atomic(&cache_path(app), json!({"v": 1, "keys": m}).to_string().as_bytes());
}

fn remember(app: &Path, key: &[u8; 32], seq: i64, h: &Hints, at: u64) {
    let _g = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut m = load_cache(app);
    let k = crate::net::hex(key);
    if m.get(&k).and_then(|e| e["seq"].as_i64()).is_some_and(|old| old > seq) {
        return;
    }
    m.insert(k, json!({"seq": seq, "at": at, "hints": h.to_json()}));
    if m.len() > CACHE_KEYS {
        let mut by_age: Vec<(u64, String)> = m.iter().map(|(k, e)| (e["at"].as_u64().unwrap_or(0), k.clone())).collect();
        by_age.sort();
        for (_, k) in by_age.into_iter().take(m.len() - CACHE_KEYS) {
            m.remove(&k);
        }
    }
    save_cache(app, &m);
}

static CACHE_LOCK: Mutex<()> = Mutex::new(());

/// The cached packet of a key: (when it was resolved here, what it says), if it isn't too old.
pub fn cached(app: &Path, key: &[u8; 32], now: u64) -> Option<(u64, Hints)> {
    let m = load_cache(app);
    let e = m.get(&crate::net::hex(key))?;
    let h = Hints::from_json(&e["hints"])?;
    (now.saturating_sub(h.ts) <= MAX_AGE).then(|| (e["at"].as_u64().unwrap_or(0), h))
}

/// Resolve one key: the cache while fresh, else the DHT (checked here), else the cache while not too old.
pub fn resolve_with(board: &dyn Board, app: &Path, key: &[u8; 32], now: u64) -> Option<Hints> {
    let c = cached(app, key, now);
    if let Some((at, h)) = &c {
        if now.saturating_sub(*at) <= FRESH {
            return Some(h.clone());
        }
    }
    if let Some(it) = board.get(key) {
        if let Ok(h) = verify(&it, key) {
            if now.saturating_sub(h.ts) <= MAX_AGE && h.ts <= now + super::mesh::SKEW {
                remember(app, key, it.seq(), &h, now);
                return Some(h);
            }
        }
    }
    c.map(|(_, h)| h)
}

/// Every 64-hex key this PC knows whose peer id is `pid` (the DHT cache, friends, the records friends sent): the
/// peer id is the key's hash, so a match is that key, whoever wrote the file.
pub fn key_for(app: &Path, pid: &str) -> Option<[u8; 32]> {
    fn walk(v: &Value, pid: &str, out: &mut Option<[u8; 32]>) {
        if out.is_some() {
            return;
        }
        let try_str = |s: &str, out: &mut Option<[u8; 32]>| {
            if s.len() == 64 {
                if let Some(k) = crate::identity::unhex32(s).filter(|k| crate::identity::peer_id(k) == pid) {
                    *out = Some(k);
                }
            }
        };
        match v {
            Value::String(s) => try_str(s, out),
            Value::Array(a) => a.iter().for_each(|x| walk(x, pid, out)),
            Value::Object(m) => m.iter().for_each(|(k, x)| {
                try_str(k, out);
                walk(x, pid, out)
            }),
            _ => {}
        }
    }
    let mut found = None;
    walk(&Value::Object(load_cache(app)), pid, &mut found);
    for p in [super::friends::cache_path(app), super::gossip::store_path(app)] {
        if found.is_some() {
            break;
        }
        if let Some(v) = std::fs::metadata(&p).ok().filter(|m| m.len() <= 4 << 20).and_then(|_| std::fs::read(&p).ok()).and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
            walk(&v, pid, &mut found);
        }
    }
    found
}

fn key_of(app: &Path, peer: &str) -> Option<[u8; 32]> {
    let p = peer.trim().to_ascii_lowercase();
    if p.len() == 64 {
        return crate::identity::unhex32(&p);
    }
    crate::identity::peer_id_ok(&p).then(|| key_for(app, &p)).flatten()
}

/// What [`lookup`] found for a peer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Endpoint ids to dial (64 hex). A realm's or Passport's pointer names its devices; a key with no pointer of ours
    /// is given back as itself (a device key is its own endpoint id: the transport resolves its packet itself).
    pub endpoints: Vec<String>,
    /// Address hints the owner signed (`host:port`), to try beside the endpoints.
    pub addrs: Vec<String>,
}

fn found(key: &[u8; 32], h: Option<Hints>) -> Found {
    match h {
        Some(h) => Found { endpoints: h.endpoints, addrs: h.addrs },
        None => Found { endpoints: vec![crate::net::hex(key)], addrs: vec![] },
    }
}

/// The lookup with its parts given (tests, and the transport when it holds its own board).
pub fn lookup_with(board: &dyn Board, app: &Path, peer: &str, now: u64) -> Found {
    let Some(key) = key_of(app, peer) else { return Found::default() };
    found(&key, resolve_with(board, app, &key, now))
}

/// **The transport's lookup boundary** (p2p/link.rs): `peer` (a 16-letter peer id, or a 64-hex public key: a
/// device, a realm or a Passport root) in; the endpoint ids to dial and the address hints its owner signed out.
/// Empty when the key is unknown here; with the `dht` switch off only what a key says by itself. Blocking: up to a
/// few seconds when the DHT has to be asked (answers are cached for [`FRESH`]).
pub fn lookup(peer: &str) -> Found {
    let app = crate::settings::app_dir();
    let Some(key) = key_of(&app, peer) else { return Found::default() };
    if !super::flags::current(&app).dht {
        return found(&key, None);
    }
    match client(&app) {
        Ok(d) => lookup_with(d.as_ref(), &app, peer, now()),
        Err(_) => found(&key, cached(&app, &key, now()).map(|(_, h)| h)),
    }
}

/// The device key of this PC (the Passport's `device.key`, the transport's endpoint key), if it has one: never
/// published under here (iroh's own address record lives there).
fn device_public(app: &Path) -> Option<[u8; 32]> {
    crate::passport::device_public_if_any(app)
}

/// The https fetch pointers of every cached packet (extra record sources for the mesh: p2p/mesh.rs `gather_all`).
pub fn cached_pointers(app: &Path, now: u64) -> Vec<String> {
    let mut out: Vec<String> = load_cache(app).values().filter_map(|e| Hints::from_json(&e["hints"]))
        .filter(|h| now.saturating_sub(h.ts) <= MAX_AGE).flat_map(|h| h.fetch).filter(|f| f.starts_with("https://")).collect();
    out.sort();
    out.dedup();
    out.truncate(16);
    out
}

// ------------------------------------------------------------------------------------------------ publishing

/// Put one packet now.
pub fn publish_with(board: &dyn Board, key: &Key, h: &Hints, now_us: i64) -> Result<i64, String> {
    let it = item(key, h, now_us)?;
    board.put(it)?;
    Ok(now_us)
}

fn now_us() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_micros() as i64).unwrap_or(0)
}

/// What is published from this PC, put again every [`REPUBLISH`] by one thread while there is anything.
#[derive(Default)]
struct Jobs {
    map: HashMap<[u8; 32], (Key, Hints)>,
    running: bool,
}

static JOBS: OnceLock<Mutex<Jobs>> = OnceLock::new();

fn jobs() -> std::sync::MutexGuard<'static, Jobs> {
    JOBS.get_or_init(|| Mutex::new(Jobs::default())).lock().unwrap_or_else(|e| e.into_inner())
}

/// Publish `h` under `key` now and keep it published (or stop, with `None`). Starts the republish thread.
fn set_job(app: &Path, key: Key, h: Option<Hints>) -> Result<(), String> {
    if let Some(h) = &h {
        h.check()?;
        if device_public(app) == Some(key.public) {
            return Err("Not under this PC's device key: the connection's own DHT record lives there.".into());
        }
    }
    let start = {
        let mut j = jobs();
        match h {
            Some(h) => {
                j.map.insert(key.public, (key, h));
            }
            None => {
                j.map.remove(&key.public);
            }
        }
        let s = !j.running && !j.map.is_empty();
        j.running |= s;
        s
    };
    if start {
        let app = app.to_path_buf();
        std::thread::spawn(move || republish_loop(&app));
    }
    Ok(())
}

fn republish_loop(app: &Path) {
    let mut last = 0u64;
    let mut seen: HashMap<[u8; 32], Hints> = HashMap::new();
    loop {
        let list: Vec<(Key, Hints)> = {
            let mut j = jobs();
            if j.map.is_empty() || !super::flags::current(app).dht {
                j.running = false;
                return;
            }
            j.map.values().cloned().collect()
        };
        let due = now().saturating_sub(last) >= REPUBLISH;
        let changed: Vec<&(Key, Hints)> = list.iter().filter(|(k, h)| due || seen.get(&k.public) != Some(h)).collect();
        if !changed.is_empty() {
            if let Ok(d) = client(app) {
                for (k, h) in changed {
                    let put = Hints { ts: now(), ..h.clone() };
                    if publish_with(d.as_ref(), k, &put, now_us()).is_ok() {
                        seen.insert(k.public, h.clone());   // the job as set (ts 0), to notice a change
                    }
                }
                save_nodes(app, &d);
                if due {
                    last = now();
                }
            }
        }
        std::thread::sleep(Duration::from_secs(20));
    }
}

/// A Passport root's pointer: the devices (endpoint ids) it is used from and the hints the player chose to share.
/// Under the Passport root key, never the device key. `endpoints` empty: withdrawn.
pub fn publish_passport(app: &Path, root: Key, endpoints: Vec<String>, addrs: Vec<String>) -> Result<(), String> {
    if endpoints.is_empty() {
        return set_job(app, root, None);
    }
    set_job(app, root, Some(Hints { kind: "passport".into(), ts: 0, endpoints, addrs, record_sha256: None, fetch: vec!["peer".into()] }))
}

/// A realm's pointer under the realm key, only while its host lists it (`listed` false: withdrawn): the endpoints
/// that serve it, the hints the host chose, the hash of its full signed mesh record (so readers know which one is
/// current) and `fetch`, where to get that record.
pub fn publish_realm(app: &Path, key: Key, listed: bool, endpoints: Vec<String>, addrs: Vec<String>, record: Option<&Value>, fetch: Vec<String>) -> Result<(), String> {
    if !listed {
        return set_job(app, key, None);
    }
    let record_sha256 = record.map(record_hash);
    set_job(app, key, Some(Hints { kind: "realm".into(), ts: 0, endpoints, addrs, record_sha256, fetch }))
}

/// sha256 (hex) of a mesh record's canonical JSON.
pub fn record_hash(rec: &Value) -> String {
    use sha2::{Digest, Sha256};
    crate::net::hex(&Sha256::digest(super::flags::canonical(rec).as_bytes()))
}

/// Resolve the realm keys the mesh holds, a few at a time, so their pointers and hints are cached (p2p/mesh.rs).
pub fn refresh_realm_keys(board: &dyn Board, app: &Path, keys: &[[u8; 32]], now: u64) -> usize {
    keys.iter().take(32).filter(|k| resolve_with(board, app, k, now).is_some()).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn hints() -> Hints {
        Hints { kind: "realm".into(), ts: now(), endpoints: vec!["cd".repeat(32)], addrs: vec!["203.0.113.7:41191".into(), "[2001:db8::1]:3724".into(), "realm.example.org:443".into()],
                record_sha256: Some("ab".repeat(32)), fetch: vec!["peer".into(), "https://mirror.example.org/mesh/snapshot.json".into()] }
    }

    #[test]
    fn z_base_32_matches_pkarr() {
        // the key of seed [0; 32] (3b6a27bc...), z-base-32 as pkarr writes it (worked out apart, in Python)
        let k = Key::from_seed([0; 32]).unwrap();
        assert_eq!(z32(&k.public), "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo");
        assert_eq!(z32(&[0xff]), "9h");
    }

    #[test]
    fn packets_round_trip_and_stay_small() {
        let k = Key::from_seed([7; 32]).unwrap();
        let p = encode(&hints(), &k.public).unwrap();
        assert!(p.len() <= MAX_PACKET, "{}", p.len());
        assert_eq!(decode(&p, &k.public).unwrap(), hints());
        // read under another key: not ours
        assert!(decode(&p, &Key::from_seed([8; 32]).unwrap().public).is_err());
        // four endpoints, four long addresses and two pointers fit; past 1000 bytes is refused, never cut
        let mut big = hints();
        big.addrs = (0..4).map(|i| format!("[2001:db8:1234:5678:9abc:def0:1234:{i}]:41191")).collect();
        big.fetch = (0..2).map(|i| format!("https://mirror-{i}.example.org/mesh/snapshot.json")).collect();
        big.endpoints = (0..MAX_ENDPOINTS).map(|i| format!("{i:x}").repeat(64)).collect();
        let p = encode(&big, &k.public).unwrap();
        assert!(p.len() <= MAX_PACKET, "{}", p.len());
        assert_eq!(decode(&p, &k.public).unwrap(), big);
        big.fetch = (0..MAX_FETCH).map(|i| format!("https://mirror-{i}.example.org/{}/mesh/snapshot.json", "long".repeat(30))).collect();
        assert!(encode(&big, &k.public).unwrap_err().contains("bytes"));
        // what a packet may never carry
        for bad in [Hints { kind: "email".into(), ..hints() }, Hints { kind: "launcher".into(), ..hints() },
                    Hints { endpoints: vec!["not-an-id".into()], ..hints() }, Hints { addrs: vec!["someone@example.org".into()], ..hints() },
                    Hints { addrs: vec!["no-port.example.org".into()], ..hints() }, Hints { fetch: vec!["http://plain.example.org/x".into()], ..hints() },
                    Hints { record_sha256: Some("xyz".into()), ..hints() }, Hints { addrs: vec!["1.2.3.4:1".into(); 9], ..hints() }] {
            assert!(encode(&bad, &k.public).is_err(), "{bad:?}");
        }
        // junk and truncation are refused, never a panic
        for n in 0..p.len() {
            let _ = decode(&p[..n], &k.public);
        }
        let mut loopy = p.clone();
        loopy[12] = 0xC0;
        loopy[13] = 12;
        assert!(decode(&loopy, &k.public).is_err());
    }

    #[test]
    fn signatures_are_checked_here() {
        let k = Key::from_seed([7; 32]).unwrap();
        let it = item(&k, &hints(), 1_000).unwrap();
        assert_eq!(verify(&it, &k.public).unwrap(), hints());
        let v = it.value().to_vec();
        let forged = MutableItem::new_signed_unchecked(k.public, *it.signature(), &v, 1_001, None);
        assert_eq!(verify(&forged, &k.public).unwrap_err(), "signature");
        let other = Key::from_seed([9; 32]).unwrap();
        assert!(verify(&it, &other.public).is_err());
        // the same bytes mainline signs (its MutableItem::new with a dalek key)
        let dalek = MutableItem::new(mainline::SigningKey::from_bytes(&[7; 32]), &v, 1_000, None);
        assert_eq!(dalek.signature(), it.signature());
    }

    /// The whole path on an in-process DHT (no internet): publish under a realm key, resolve from another client,
    /// the transport's lookup by peer id, the cache when the DHT is gone.
    #[test]
    fn publish_and_resolve_on_a_test_dht() {
        let testnet = mainline::Testnet::builder(5).build().unwrap();
        let client = || mainline::Dht::builder().bootstrap(&testnet.bootstrap).bind_address(Ipv4Addr::LOCALHOST).build().unwrap();
        let (a, b) = (client(), client());
        let k = Key::from_seed([7; 32]).unwrap();
        let mut h = hints();
        publish_with(&a, &k, &h, 10).unwrap();
        let d = tempfile::tempdir().unwrap();
        let t = now();
        assert_eq!(resolve_with(&b, d.path(), &k.public, t), Some(h.clone()));
        // a newer packet replaces it (after the cache's fresh window)
        h.addrs = vec!["198.51.100.2:41191".into()];
        publish_with(&a, &k, &h, 20).unwrap();
        assert_eq!(resolve_with(&b, d.path(), &k.public, t + FRESH + 1).unwrap().addrs, h.addrs);
        // the boundary: a peer id is found through the cache, a key works as is, an unknown peer gives nothing
        let pid = crate::identity::peer_id(&k.public);
        let want = Found { endpoints: h.endpoints.clone(), addrs: h.addrs.clone() };
        assert_eq!(lookup_with(&b, d.path(), &pid, t + FRESH + 2), want);
        assert_eq!(lookup_with(&b, d.path(), &crate::net::hex(&k.public), t + FRESH + 2), want);
        assert_eq!(lookup_with(&b, d.path(), "aaaaaaaaaaaaaaaa", t), Found::default(), "an unknown peer id");
        // a key with no pointer of ours (a device): itself, for the transport to resolve
        let dev = Key::from_seed([4; 32]).unwrap();
        assert_eq!(lookup_with(&b, d.path(), &crate::net::hex(&dev.public), t).endpoints, vec![crate::net::hex(&dev.public)]);
        // nothing on the DHT for this key
        assert!(resolve_with(&b, d.path(), &Key::from_seed([3; 32]).unwrap().public, t).is_none());
        // the DHT gone: the cached packet is still used while it isn't too old, then never
        struct Gone;
        impl Board for Gone {
            fn put(&self, _: MutableItem) -> Result<(), String> { Err("gone".into()) }
            fn get(&self, _: &[u8; 32]) -> Option<MutableItem> { None }
        }
        assert_eq!(resolve_with(&Gone, d.path(), &k.public, t + 3 * FRESH).unwrap().addrs, h.addrs);
        assert!(resolve_with(&Gone, d.path(), &k.public, t + MAX_AGE + 60).is_none());
        assert!(cached_pointers(d.path(), t).contains(&"https://mirror.example.org/mesh/snapshot.json".to_string()));
        assert!(publish_with(&Gone, &k, &h, 30).is_err());
    }

    #[test]
    fn never_under_the_device_key() {
        let d = tempfile::tempdir().unwrap();
        let me = crate::identity::load_or_create(d.path()).unwrap();
        let dev = crate::passport::DeviceKey::for_app(d.path(), &me).unwrap();
        let device = Key::from_seed(dev.secret()).unwrap();
        assert_eq!(device.public, dev.public);
        assert_ne!(device.public, me.public_key, "the Passport root is not the device key");
        let e = publish_passport(d.path(), device.clone(), vec!["ab".repeat(32)], vec![]).unwrap_err();
        assert!(e.contains("device key"), "{e}");
        assert!(publish_realm(d.path(), device, true, vec![], vec![], None, vec![]).is_err());
        assert!(publish_realm(d.path(), Key::from_seed([5; 32]).unwrap(), false, vec![], vec![], None, vec![]).is_ok(), "withdrawing is always fine");
    }

    #[test]
    fn a_packet_signed_by_someone_else_is_not_cached() {
        struct Liar(MutableItem);
        impl Board for Liar {
            fn put(&self, _: MutableItem) -> Result<(), String> { Ok(()) }
            fn get(&self, _: &[u8; 32]) -> Option<MutableItem> { Some(self.0.clone()) }
        }
        let (k, other) = (Key::from_seed([7; 32]).unwrap(), Key::from_seed([8; 32]).unwrap());
        let d = tempfile::tempdir().unwrap();
        let liar = Liar(item(&other, &hints(), 5).unwrap());
        assert!(resolve_with(&liar, d.path(), &k.public, now()).is_none());
        let stale = Liar(item(&k, &Hints { ts: 1, ..hints() }, 5).unwrap());
        assert!(resolve_with(&stale, d.path(), &k.public, now()).is_none(), "too old by its own time");
        assert!(!cache_path(d.path()).exists());
    }
}
