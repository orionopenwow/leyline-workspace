//! Gossip (docs/LEYLINE-MESH.md, "Gossip"): launchers hand each other the mesh records they hold (realms, relays,
//! info documents, blocks) and the tail of the signed release log, so the network keeps working when every server
//! is gone.
//!
//! * **Carried by** friends' sealed messages today (p2p/friendnet.rs: kind `mesh`, to friends and, while "Be seen on
//!   this network" isn't off, to launchers on the same network); the new transport (p2p/link.rs) can carry the same
//!   [`body`] and hand what arrives to [`take`].
//! * **Checked in one place:** every record goes through `mesh::Merged::offer` (signature, expiry, `seq`, the smaller
//!   signature on a tie, blocks); the release-log entries through `releaselog::extend` (the release key decides).
//!   A friend can leave records out, never add, change or revive one.
//! * **Caps:** one message per peer every [`EVERY`] seconds, at most [`BUDGET`] bytes of records in it (and the
//!   friends' own 8 KB message cap); from one sender at most one message per [`MIN_GAP`] seconds and [`MAX_IN`]
//!   records; the store keeps at most [`STORE_MAX`] records (`cache/mesh-records.json`).
//! * Nothing here is game data, an email or an address beyond what the records themselves sign.

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::flags::canonical;
use super::mesh::{self, Merged};

/// How often a launcher gossips to each friend (seconds).
pub const EVERY: u64 = 15 * 60;
/// Bytes of records in one message (a realm record is at most 6000).
pub const BUDGET: usize = 6500;
/// Bytes of release-log entries in one message.
pub const CHAIN_BUDGET: usize = 1200;
/// Least time between two messages taken from one sender.
pub const MIN_GAP: u64 = 5 * 60;
/// Most records taken from one message.
pub const MAX_IN: usize = 64;
pub const STORE_MAX: usize = 2000;
/// Most peers gossiped to each round.
pub const MAX_PEERS: usize = 16;

pub fn store_path(app: &Path) -> PathBuf {
    app.join("cache").join("mesh-records.json")
}

/// The records this PC holds, checked again now (expired ones fall away), with this build's council.
pub fn load_store(app: &Path, now: u64) -> Merged {
    let mut m = Merged::new();
    let raw = std::fs::metadata(store_path(app)).ok().filter(|x| x.len() <= 32 << 20).and_then(|_| std::fs::read(store_path(app)).ok());
    if let Some(v) = raw.and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
        for r in v.get("records").and_then(Value::as_array).into_iter().flatten().take(STORE_MAX) {
            m.offer(r, now, mesh::STAFF_KEYS);
        }
    }
    m
}

/// Keep what verified: blocks first, then the newest of the rest, at most [`STORE_MAX`]. The mirrors' format.
pub fn save_store(app: &Path, m: &Merged) {
    let recs = ordered(m, &HashSet::new());
    let recs: Vec<Value> = recs.into_iter().take(STORE_MAX).cloned().collect();
    let _ = std::fs::create_dir_all(app.join("cache"));
    let _ = crate::settings::write_atomic(&store_path(app), json!({"v": 1, "records": recs}).to_string().as_bytes());
}

/// Records in the order they are passed on: blocks, relays, info documents, realms; newest first in each.
fn ordered<'a>(m: &'a Merged, blocked: &HashSet<String>) -> Vec<&'a Value> {
    let rank = |k: &str| match &k[..2] {
        "b-" => 0,
        "n-" => 1,
        "i-" => 2,
        _ => 3,
    };
    let mut v: Vec<(u8, std::cmp::Reverse<u64>, &String, &Value)> = m.recs.iter()
        .filter(|(k, _)| !blocked.contains(k.as_str()))
        .map(|(k, (ver, r))| (rank(k), std::cmp::Reverse(*ver), k, r)).collect();
    v.sort_by(|a, b| (a.0, a.1, a.2).cmp(&(b.0, b.1, b.2)));
    v.into_iter().map(|(_, _, _, r)| r).collect()
}

/// As many records as fit `budget` bytes (canonical JSON), in passing-on order, leaving out what this player blocked.
pub fn pack(m: &Merged, blocked: &HashSet<String>, budget: usize) -> Vec<Value> {
    let mut used = 0;
    let mut out = Vec::new();
    for r in ordered(m, blocked) {
        let n = canonical(r).len() + 1;
        if used + n > budget {
            continue;   // a smaller one further on may still fit
        }
        used += n;
        out.push(r.clone());
    }
    out
}

/// The newest release-log entries that fit `budget` bytes: `{"key", "from", "entries"}` (`from` = the first one's seq).
pub fn chain_tail(c: &crate::releaselog::Chain, budget: usize) -> Option<Value> {
    let mut take = 0;
    let mut used = 0;
    for e in c.raw.iter().rev() {
        let n = canonical(e).len() + 1;
        if used + n > budget {
            break;
        }
        used += n;
        take += 1;
    }
    (take > 0).then(|| json!({"key": c.key, "from": c.raw.len() - take, "entries": c.raw[c.raw.len() - take..]}))
}

/// What this PC gossips now: its records (less what its player blocked) and the release log's tail.
pub fn body(app: &Path, now: u64) -> Value {
    let m = load_store(app, now);
    let mut b = json!({"recs": pack(&m, &mesh::local_blocks(app), BUDGET)});
    if let Some(k) = crate::releaselog::trusted_key() {
        if let Some(t) = chain_tail(&crate::releaselog::load(app, k), CHAIN_BUDGET) {
            b["chain"] = t;
        }
    }
    b
}

/// A release-log tail from a peer: taken when it extends our copy and verifies under `key`. True when it did.
pub fn take_chain(app: &Path, key: &str, t: &Value) -> bool {
    if t.get("key").and_then(Value::as_str) != Some(key) {
        return false;
    }
    let (Some(from), Some(entries)) = (t.get("from").and_then(Value::as_u64), t.get("entries").and_then(Value::as_array)) else { return false };
    let ours = crate::releaselog::load(app, key);
    let (from, have) = (from as usize, ours.len());
    if from > have || from + entries.len() <= have {
        return false;   // a gap we can't fill from this, or nothing new
    }
    match crate::releaselog::extend(&ours, &entries[have - from..]) {
        (Some(c), crate::releaselog::Outcome::Adopted) => crate::releaselog::save(app, &c).is_ok(),
        _ => false,
    }
}

static LAST_FROM: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);

/// Whether a message from `from` may be taken now (one per [`MIN_GAP`]; remembers at most 4096 senders).
pub fn allow(from: &str, now: u64) -> bool {
    let mut g = LAST_FROM.lock().unwrap_or_else(|e| e.into_inner());
    let m = g.get_or_insert_with(HashMap::new);
    if m.get(from).is_some_and(|t| now.saturating_sub(*t) < MIN_GAP) {
        return false;
    }
    if m.len() >= 4096 {
        m.retain(|_, t| now.saturating_sub(*t) < MIN_GAP);
        if m.len() >= 4096 {
            return false;
        }
    }
    m.insert(from.to_string(), now);
    true
}

/// What a gossip message brought.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Took {
    pub new: usize,
    pub refused: usize,
    pub chain: bool,
}

/// Take a gossip body from `from` (a peer id, already checked to be a friend or on this network): every record
/// offered to the store (the mesh's rules), the release-log tail tried. Err when rate-limited or malformed.
pub fn take(app: &Path, from: &str, body: &Value, now: u64) -> Result<Took, String> {
    let recs = body.get("recs").and_then(Value::as_array).ok_or("no records")?;
    if !allow(from, now) {
        return Err("too soon".into());
    }
    take_unlimited(app, recs, body.get("chain"), now)
}

fn take_unlimited(app: &Path, recs: &[Value], chain: Option<&Value>, now: u64) -> Result<Took, String> {
    static STORE: Mutex<()> = Mutex::new(());
    let _g = STORE.lock().unwrap_or_else(|e| e.into_inner());
    let mut m = load_store(app, now);
    let before = m.refused;
    let blocked = mesh::local_blocks(app);
    let mut t = Took::default();
    for r in recs.iter().take(MAX_IN) {
        if m.offer(r, now, mesh::STAFF_KEYS) {
            t.new += 1;
        }
    }
    t.refused = m.refused - before;
    if t.new > 0 {
        save_store(app, &m.without(&blocked));
    }
    if let (Some(c), Some(k)) = (chain, crate::releaselog::trusted_key()) {
        t.chain = take_chain(app, k, c);
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::flags::canonical;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn realm(seed: u8, seq: u64, ts: u64, name: &str) -> Value {
        let k = Ed25519KeyPair::from_seed_unchecked(&[seed; 32]).unwrap();
        let pk = crate::net::hex(k.public_key().as_ref());
        let beat = json!({"v": 1, "realm_id": mesh::realm_id(k.public_key().as_ref()), "pubkey": pk, "seq": seq, "ts": ts,
            "relay": "relay.example.org", "record": {"name": name}, "status": {"online": true}});
        let sig = crate::net::hex(k.sign(format!("wu-listing-heartbeat-v1\n{}", canonical(&beat)).as_bytes()).as_ref());
        json!({"kind": "realm", "sig": sig, "beat": beat, "endpoints": []})
    }

    #[test]
    fn records_pass_between_launchers_and_are_checked_on_the_way_in() {
        let now = mesh::now_secs();
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let mut m = Merged::new();
        for i in 1..=3 {
            assert!(m.offer(&realm(i, 1, now, &format!("Realm {i}")), now, &[]));
        }
        save_store(a.path(), &m);
        let body = body(a.path(), now);
        assert_eq!(body["recs"].as_array().unwrap().len(), 3);
        // a forged record rides along: refused, the good ones taken
        let mut forged = realm(4, 1, now, "Real");
        forged["beat"]["record"]["name"] = json!("Forged");
        let mut recs = body["recs"].as_array().unwrap().clone();
        recs.push(forged);
        let t = take(b.path(), "bbbbbbbbbbbbbbbb", &json!({"recs": recs}), now).unwrap();
        assert_eq!((t.new, t.refused), (3, 1));
        assert_eq!(load_store(b.path(), now).realms().count(), 3);
        // the same sender again at once: limited
        assert_eq!(take(b.path(), "bbbbbbbbbbbbbbbb", &json!({"recs": []}), now + 10).unwrap_err(), "too soon");
        assert!(take(b.path(), "bbbbbbbbbbbbbbbb", &json!({"recs": []}), now + MIN_GAP).is_ok());
        assert!(take(b.path(), "cccccccccccccccc", &json!({"nothing": 1}), now).is_err());
        // an older seq never replaces a newer one, whoever sends it
        let t = take_unlimited(b.path(), &[realm(1, 0, now, "Older")], None, now).unwrap();
        assert_eq!(t.new, 0);
        // expired records fall away from the store on the next load
        assert_eq!(load_store(b.path(), now + mesh::REALM_TTL + 1).realms().count(), 0);
    }

    #[test]
    fn packing_keeps_to_the_budget_and_leaves_out_local_blocks() {
        let now = mesh::now_secs();
        let mut m = Merged::new();
        for i in 1..=40 {
            m.offer(&realm(i, 1, now, &"x".repeat(200)), now, &[]);
        }
        let p = pack(&m, &HashSet::new(), BUDGET);
        let size: usize = p.iter().map(|r| canonical(r).len() + 1).sum();
        assert!(size <= BUDGET && p.len() < 40 && !p.is_empty(), "{size} bytes, {} records", p.len());
        let rid = p[0]["beat"]["realm_id"].as_str().unwrap().to_string();
        let blocked: HashSet<String> = [format!("r-{rid}")].into_iter().collect();
        assert!(pack(&m, &blocked, BUDGET).iter().all(|r| r["beat"]["realm_id"] != rid.as_str()));
        // a friend's message stays under the friends' message cap
        let me = crate::identity::PeerIdentity::from_seed([1; 32]).unwrap();
        let msg = crate::p2p::friends::make_msg(&me, &"ab".repeat(32), "mesh", json!({"recs": p}), now).unwrap();
        assert!(canonical(&msg).len() <= crate::p2p::friends::MSG_MAX);
    }

    #[test]
    fn the_release_log_tail_extends_a_copy_the_key_signed() {
        use crate::releaselog::tests::{launcher_chain, pub_hex, DEV_TEST_SEED};
        let key = pub_hex(DEV_TEST_SEED);
        let full = launcher_chain(DEV_TEST_SEED, &["0.3.1", "0.3.2", "0.3.3"], "https://m.example.org/");
        let chain = crate::releaselog::verify(&full, &key).unwrap();
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        // b holds the first entry; a's tail carries the rest
        let first = json!({"format": 1, "key": key, "entries": [full["entries"][0].clone()]});
        crate::releaselog::save(b.path(), &crate::releaselog::verify(&first, &key).unwrap()).unwrap();
        crate::releaselog::save(a.path(), &chain).unwrap();
        let t = chain_tail(&chain, 10_000).unwrap();
        assert_eq!(t["from"], 0);
        assert!(take_chain(b.path(), &key, &t));
        assert_eq!(crate::releaselog::load(b.path(), &key).len(), 3);
        assert!(!take_chain(b.path(), &key, &t), "nothing new");
        // another key's chain, or a tail with a gap, is never taken
        assert!(!take_chain(a.path(), &"cd".repeat(32), &t));
        let c = tempfile::tempdir().unwrap();
        let small = chain_tail(&chain, canonical(&full["entries"][2]).len() + 1).unwrap();
        assert_eq!(small["from"], 2);
        assert!(!take_chain(c.path(), &key, &small), "a gap");
        // a tampered entry: refused whole
        let mut bad = t.clone();
        bad["entries"][2]["sig"] = json!("00".repeat(64));
        let d = tempfile::tempdir().unwrap();
        assert!(!take_chain(d.path(), &key, &bad));
        assert!(crate::releaselog::load(d.path(), &key).is_empty());
    }
}
