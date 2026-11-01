//! The Leyline release log ("the chain"), launcher side: an append-only, hash-chained list of every Leyline
//! release, Host pack, Leyline addons pack and pinned upstream server-source tarball, each entry signed with the
//! release key (Ed25519, kept offline by the owner; docs/RESILIENCE.md). Nothing financial: a signed log.
//!
//! The format and rules are `hosting/kit/tools/wunet/releaselog.py`'s (the cross-language vector
//! `release-log-vector.json` is checked by both): `{"format":1,"key":hex,"entries":[{"entry":<canonical JSON
//! text>,"sig":hex}]}`, signature over `"leyline-release-log-v1\n" + text`, `prev` = sha256 of the previous text.
//!
//! What the launcher does with it:
//! * keeps a verified copy in `cache/release-log.json` (only ever a chain that verifies is written);
//! * refreshes it from the addresses in branding.json, the seed relays and (with `mesh_realms` on) mesh peers
//!   (`/api/mesh/chain`): the longest good chain wins, anything not signed by the key, broken, reordered or cut in
//!   the middle is refused whole, a shorter copy never replaces a longer one;
//! * uses it as an update source (the newest launcher entry's mirrors; `update::sources`) and as the allow-list of
//!   files "Help share Leyline's own files" may seed (`p2p::share`).
//!
//! With no key in branding.json (until the owner makes it) none of this runs.

use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const FORMAT: u64 = 1;
pub const CTX: &str = "leyline-release-log-v1\n";
pub const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub const KINDS: &[&str] = &["launcher", "hostpack", "addons", "source", "note"];
pub const MAX_ENTRIES: usize = 100_000;
pub const MAX_TEXT: usize = 16_384;
const MAX_FILES: usize = 64;
const MAX_MIRRORS: usize = 16;
pub const CACHE: &str = "release-log.json";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileRef {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceRef {
    pub repo: String,
    pub commit: String,
}

/// One checked entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub seq: u64,
    pub prev: String,
    pub kind: String,
    pub name: String,
    pub version: String,
    pub files: Vec<FileRef>,
    pub infohash: Option<String>,
    pub magnet: Option<String>,
    pub mirrors: Vec<String>,
    pub time: u64,
    pub source: Option<SourceRef>,
    /// sha256 of this entry's text (the next entry's `prev`).
    pub hash: String,
}

/// A chain that verified, with its wire form (so it can be handed on unchanged).
#[derive(Debug, Clone, PartialEq)]
pub struct Chain {
    pub key: String,
    pub entries: Vec<Entry>,
    pub raw: Vec<Value>,
}

impl Chain {
    pub fn empty(key: &str) -> Chain {
        Chain { key: key.into(), entries: vec![], raw: vec![] }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn head(&self) -> String {
        self.entries.last().map(|e| e.hash.clone()).unwrap_or_else(|| ZERO.into())
    }
    pub fn to_doc(&self) -> Value {
        json!({"format": FORMAT, "key": self.key, "entries": self.raw})
    }
    /// The newest entry of a kind (the last one in the log).
    pub fn newest(&self, kind: &str) -> Option<&Entry> {
        self.entries.iter().rev().find(|e| e.kind == kind)
    }
    /// The newest launcher release above `current`, by version order.
    pub fn newest_launcher_above(&self, current: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.kind == "launcher" && crate::update::is_newer(&e.version, current))
            .max_by(|a, b| crate::update::cmp_versions(&a.version, &b.version).unwrap_or(std::cmp::Ordering::Equal))
    }
    /// Every file the log names, with the entry it is in.
    pub fn files(&self) -> impl Iterator<Item = (&Entry, &FileRef)> {
        self.entries.iter().flat_map(|e| e.files.iter().map(move |f| (e, f)))
    }
}

fn hex_of(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

pub fn entry_hash(text: &str) -> String {
    crate::net::hex(&Sha256::digest(text.as_bytes()))
}

fn https_dir(u: &str) -> bool {
    crate::net::plain_https(u) && u.ends_with('/') && !u.contains("..")
}

fn file_name_ok(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 120
        && !n.contains("..")
        && n.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ' ' | '+' | '-'))
}

fn version_ok(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 48
        && v.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && v.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-'))
}

fn u(v: &Value) -> Option<u64> {
    if v.is_boolean() {
        return None;
    }
    v.as_u64()
}

/// Check one body at position `seq` after `prev` (the same rules as releaselog.py `check_body`).
fn check_body(b: &Value, seq: u64, prev: &str, hash: String) -> Result<Entry, String> {
    let bad = |what: &str| format!("entry {seq}: {what}");
    if b.get("v").and_then(u) != Some(1) {
        return Err(bad("not a release log entry"));
    }
    if b.get("seq").and_then(u) != Some(seq) {
        return Err(bad("out of order"));
    }
    if b.get("prev").and_then(Value::as_str) != Some(prev) {
        return Err(bad("does not follow the entry before it"));
    }
    let kind = b.get("kind").and_then(Value::as_str).filter(|k| KINDS.contains(k)).ok_or_else(|| bad("unknown kind"))?;
    let name = b.get("name").and_then(Value::as_str).filter(|n| !n.is_empty() && n.len() <= 80).ok_or_else(|| bad("name"))?;
    let version = b.get("version").and_then(Value::as_str).filter(|v| version_ok(v)).ok_or_else(|| bad("version"))?;
    let time = b.get("time").and_then(u).ok_or_else(|| bad("time"))?;
    let files_v = b.get("files").and_then(Value::as_array).filter(|a| a.len() <= MAX_FILES).ok_or_else(|| bad("files"))?;
    let mut files = Vec::new();
    for f in files_v {
        let name = f.get("name").and_then(Value::as_str).filter(|n| file_name_ok(n));
        let size = f.get("size").and_then(u);
        let sha = f.get("sha256").and_then(Value::as_str).filter(|s| hex_of(s, 64));
        let (Some(name), Some(size), Some(sha)) = (name, size, sha) else { return Err(bad("a file is not name/size/sha256")) };
        // the hard line: game data and game clients are never in the log, whoever signed it
        if let Some(why) = crate::p2p::checker::name_refusal(name) {
            return Err(bad(&format!("refused, {why}")));
        }
        files.push(FileRef { name: name.into(), size, sha256: sha.into() });
    }
    let mirrors: Vec<String> = match b.get("mirrors") {
        None => vec![],
        Some(Value::Array(a)) if a.len() <= MAX_MIRRORS => {
            let m: Vec<String> = a.iter().filter_map(Value::as_str).filter(|x| https_dir(x)).map(String::from).collect();
            if m.len() != a.len() {
                return Err(bad("mirrors must be https folders ending in /"));
            }
            m
        }
        _ => return Err(bad("mirrors must be https folders ending in /")),
    };
    let infohash = match b.get("infohash") {
        None => None,
        Some(Value::String(s)) if hex_of(s, 40) || hex_of(s, 64) => Some(s.clone()),
        _ => return Err(bad("infohash")),
    };
    let magnet = match b.get("magnet") {
        None => None,
        Some(Value::String(s)) if s.starts_with("magnet:?") && s.len() <= 2048 => Some(s.clone()),
        _ => return Err(bad("magnet")),
    };
    let source = match (kind, b.get("source")) {
        ("source", Some(s)) => {
            let repo = s.get("repo").and_then(Value::as_str).filter(|r| r.starts_with("https://"));
            let commit = s.get("commit").and_then(Value::as_str).filter(|c| hex_of(c, 40));
            match (repo, commit) {
                (Some(r), Some(c)) => Some(SourceRef { repo: r.into(), commit: c.into() }),
                _ => return Err(bad("a source entry needs source.repo (https) and source.commit (40 hex)")),
            }
        }
        ("source", None) => return Err(bad("a source entry needs source.repo (https) and source.commit (40 hex)")),
        (_, Some(_)) => return Err(bad("only source entries name a source")),
        (_, None) => None,
    };
    Ok(Entry { seq, prev: prev.into(), kind: kind.into(), name: name.into(), version: version.into(), files, infohash, magnet, mirrors, time, source, hash })
}

/// Verify a whole chain against the release key. Any fault refuses all of it.
pub fn verify(doc: &Value, key_hex: &str) -> Result<Chain, String> {
    let pk = unhex(key_hex).filter(|k| k.len() == 32 && hex_of(key_hex, 64)).ok_or("no release key to check against")?;
    if doc.get("format").and_then(u) != Some(FORMAT) {
        return Err("not a release log".into());
    }
    match doc.get("key") {
        None => {}
        Some(Value::String(k)) if k == key_hex => {}
        _ => return Err("signed with another key".into()),
    }
    let ents = doc.get("entries").and_then(Value::as_array).filter(|a| a.len() <= MAX_ENTRIES).ok_or("no entries")?;
    let key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, pk);
    let mut prev = ZERO.to_string();
    let mut out: Vec<Entry> = Vec::with_capacity(ents.len());
    for (i, e) in ents.iter().enumerate() {
        let (Some(text), Some(sig)) = (e.get("entry").and_then(Value::as_str), e.get("sig").and_then(Value::as_str)) else {
            return Err(format!("entry {i}: not entry/sig"));
        };
        if text.len() > MAX_TEXT {
            return Err(format!("entry {i}: too long"));
        }
        let sig = unhex(sig).filter(|s| s.len() == 64).ok_or_else(|| format!("entry {i}: signature is not hex"))?;
        let msg = [CTX.as_bytes(), text.as_bytes()].concat();
        if key.verify(&msg, &sig).is_err() {
            return Err(format!("entry {i}: the signature does not verify"));
        }
        let body: Value = serde_json::from_str(text).map_err(|_| format!("entry {i}: not JSON"))?;
        let hash = entry_hash(text);
        let ent = check_body(&body, i as u64, &prev, hash.clone())?;
        if let Some(last) = out.last() {
            if ent.time + 300 < last.time {
                return Err(format!("entry {i}: earlier than the entry before it"));
            }
        }
        out.push(ent);
        prev = hash;
    }
    Ok(Chain { key: key_hex.into(), entries: out, raw: ents.clone() })
}

/// What happened when a peer's chain was offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Theirs extends ours: taken.
    Adopted,
    /// Theirs is a longer good chain that forks from ours (both signed by the key): the newest head wins.
    Replaced,
    /// Theirs is not longer: ours stays.
    Kept,
    /// Theirs does not verify.
    Refused(String),
}

/// Choose between our (good) chain and a peer's document.
pub fn choose(ours: &Chain, theirs: &Value) -> (Option<Chain>, Outcome) {
    let t = match verify(theirs, &ours.key) {
        Ok(t) => t,
        Err(e) => return (None, Outcome::Refused(e)),
    };
    if t.len() <= ours.len() {
        return (None, Outcome::Kept);
    }
    let extends = t.raw[..ours.len()] == ours.raw[..];
    (Some(t), if extends { Outcome::Adopted } else { Outcome::Replaced })
}

/// Our chain plus entries a peer sent after our head.
pub fn extend(ours: &Chain, more: &[Value]) -> (Option<Chain>, Outcome) {
    let mut all = ours.raw.clone();
    all.extend(more.iter().cloned());
    choose(ours, &json!({"format": FORMAT, "key": ours.key, "entries": all}))
}

// ------------------------------------------------------------------------------------------------ this PC's copy

pub fn cache_path(app_dir: &Path) -> PathBuf {
    app_dir.join("cache").join(CACHE)
}

/// The release key this build trusts (branding.json), or None until the owner has made it.
pub fn trusted_key() -> Option<&'static str> {
    let k = crate::branding::get().release_log.public_key.as_str();
    (!k.is_empty()).then_some(k)
}

/// The cached copy, verified again (a file changed on disk is not trusted). An empty chain when there is none.
pub fn load(app_dir: &Path, key: &str) -> Chain {
    std::fs::read(cache_path(app_dir))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| verify(&v, key).ok())
        .unwrap_or_else(|| Chain::empty(key))
}

fn save(app_dir: &Path, c: &Chain) -> Result<(), String> {
    let _ = std::fs::create_dir_all(app_dir.join("cache"));
    crate::settings::write_atomic(&cache_path(app_dir), c.to_doc().to_string().as_bytes()).map_err(|e| e.to_string())
}

/// Where to ask: branding.json's addresses (our server, the GitHub release), then the seed relays' and mesh
/// peers' `/api/mesh/chain`. A whole document, or (`/api/mesh/chain?from=n`) the entries after `n`.
pub fn sources(app_dir: &Path, mesh_on: bool) -> Vec<String> {
    let b = crate::branding::get();
    let mut out: Vec<String> = b.release_log.urls.clone();
    let relays: Vec<String> = if mesh_on {
        crate::p2p::mesh::sources(app_dir, crate::p2p::mesh::SEEDS)
    } else {
        crate::p2p::mesh::SEEDS.iter().map(|s| s.to_string()).collect()
    };
    out.extend(relays.into_iter().map(|r| format!("{}/api/mesh/chain?from=", r.trim_end_matches('/'))));
    out
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SyncView {
    pub entries: usize,
    pub head: String,
    /// One line per source asked: what came of it.
    pub notes: Vec<String>,
}

/// Ask each source; keep the best good chain; write it when it changed. `fetch` returns a JSON document.
pub fn sync(app_dir: &Path, key: &str, sources: &[String], fetch: &dyn Fn(&str) -> Result<Value, String>) -> SyncView {
    let mut ours = load(app_dir, key);
    let start = ours.head();
    let mut notes = Vec::new();
    for s in sources.iter().take(12) {
        let incremental = s.ends_with("?from=");
        let url = if incremental { format!("{s}{}", ours.len()) } else { s.clone() };
        let doc = match fetch(&url) {
            Ok(d) => d,
            Err(e) => {
                notes.push(format!("{s}: {e}"));
                continue;
            }
        };
        let (new, what) = if incremental {
            let more: Vec<Value> = doc.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
            if doc.get("key").and_then(Value::as_str) != Some(key) {
                (None, Outcome::Refused("another key".into()))
            } else {
                match extend(&ours, &more) {
                    // a fork, or more pages: ask for the whole chain from the start
                    (None, Outcome::Refused(_)) if doc.get("seq").and_then(Value::as_u64).is_some_and(|n| n as usize + 1 > ours.len()) => {
                        match fetch(&format!("{s}0")) {
                            Ok(full) => {
                                let all = full.get("entries").cloned().unwrap_or(Value::Null);
                                choose(&ours, &json!({"format": FORMAT, "key": key, "entries": all}))
                            }
                            Err(e) => (None, Outcome::Refused(e)),
                        }
                    }
                    r => r,
                }
            }
        } else {
            choose(&ours, &doc)
        };
        notes.push(format!("{s}: {what:?}"));
        if let Some(c) = new {
            ours = c;
        }
    }
    if ours.head() != start {
        if let Err(e) = save(app_dir, &ours) {
            notes.push(format!("could not save the release log: {e}"));
        }
    }
    SyncView { entries: ours.len(), head: ours.head(), notes }
}

/// The refresh the app runs with the update check: nothing without a trusted key.
pub fn refresh(app_dir: &Path, mesh_on: bool) -> Option<SyncView> {
    let key = trusted_key()?;
    let srcs = sources(app_dir, mesh_on);
    Some(sync(app_dir, key, &srcs, &|u| crate::net::get_json(u, crate::net::TIMEOUT)))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    /// THE DEV KEY, FOR TESTS ONLY: "leyline-dev-test-key-not-for-use" (wunet/releaselog.py DEV_TEST_SEED). Anyone
    /// can read it, so it signs nothing real; branding.json never names its public half.
    pub const DEV_TEST_SEED: &[u8; 32] = b"leyline-dev-test-key-not-for-use";
    const OTHER_SEED: &[u8; 32] = b"some-one-else-who-has-no-the-key";
    const VECTOR: &str = include_str!("release-log-vector.json");

    pub fn pair(seed: &[u8; 32]) -> Ed25519KeyPair {
        Ed25519KeyPair::from_seed_unchecked(seed).unwrap()
    }
    pub fn pub_hex(seed: &[u8; 32]) -> String {
        crate::net::hex(pair(seed).public_key().as_ref())
    }

    /// Append one signed entry (the test-side twin of releaselog.py `append`).
    pub fn append(doc: &Value, seed: &[u8; 32], kind: &str, name: &str, version: &str, files: Value, extra: Value) -> Value {
        let k = pair(seed);
        let ents = doc["entries"].as_array().cloned().unwrap_or_default();
        let prev = ents.last().map(|e| entry_hash(e["entry"].as_str().unwrap())).unwrap_or_else(|| ZERO.into());
        let mut body = json!({"v": 1, "seq": ents.len(), "prev": prev, "kind": kind, "name": name, "version": version,
                              "files": files, "mirrors": [], "time": 1761900000 + ents.len() as u64 * 60});
        for (a, b) in extra.as_object().cloned().unwrap_or_default() {
            body[a] = b;
        }
        let text = crate::p2p::flags::canonical(&body);
        let sig = crate::net::hex(k.sign([CTX.as_bytes(), text.as_bytes()].concat().as_slice()).as_ref());
        let mut all = ents;
        all.push(json!({"entry": text, "sig": sig}));
        json!({"format": 1, "key": pub_hex(seed), "entries": all})
    }

    pub fn launcher_chain(seed: &[u8; 32], versions: &[&str], mirror: &str) -> Value {
        let mut d = json!({"format": 1, "key": pub_hex(seed), "entries": []});
        for v in versions {
            d = append(&d, seed, "launcher", "Leyline", v,
                json!([{"name": format!("Leyline_{v}_x64-setup.exe"), "size": 10, "sha256": "ab".repeat(32)},
                       {"name": "latest.json", "size": 10, "sha256": "cd".repeat(32)}]),
                json!({"mirrors": [mirror]}));
        }
        d
    }

    #[test]
    fn python_signed_chain_verifies_here() {
        let v: Value = serde_json::from_str(VECTOR).unwrap();
        let key = v["pub"].as_str().unwrap();
        assert_eq!(key, pub_hex(DEV_TEST_SEED), "the same dev seed on both sides");
        let c = verify(&v["chain"], key).unwrap();
        assert_eq!(c.len(), 3);
        assert_eq!(c.entries[0].kind, "launcher");
        assert_eq!(c.entries[0].mirrors.len(), 2);
        assert_eq!(c.entries[0].infohash.as_deref(), Some("33".repeat(20).as_str()));
        assert_eq!(c.entries[2].source.as_ref().unwrap().commit, "5731249195520323cb5fb9aa29761d4f033272ce");
        assert_eq!(c.entries[1].prev, c.entries[0].hash);
        // and Rust's own signing makes the same bytes (Ed25519 is deterministic, the JSON canonical)
        let first = &v["chain"]["entries"][0];
        let body: Value = serde_json::from_str(first["entry"].as_str().unwrap()).unwrap();
        assert_eq!(crate::p2p::flags::canonical(&body), first["entry"].as_str().unwrap());
        let sig = crate::net::hex(pair(DEV_TEST_SEED).sign(format!("{CTX}{}", first["entry"].as_str().unwrap()).as_bytes()).as_ref());
        assert_eq!(sig, first["sig"].as_str().unwrap());
        // the dev key is never the trusted one
        assert_ne!(crate::branding::get().release_log.public_key, key);
    }

    #[test]
    fn tampering_reordering_and_other_keys_are_refused() {
        let key = pub_hex(DEV_TEST_SEED);
        let good = launcher_chain(DEV_TEST_SEED, &["0.3.1", "0.3.2", "0.3.3"], "https://m.example.org/r/");
        assert_eq!(verify(&good, &key).unwrap().len(), 3);
        let mut t = good.clone();
        let s = t["entries"][1]["entry"].as_str().unwrap().replace("0.3.2", "9.9.9");
        t["entries"][1]["entry"] = s.into();
        assert!(verify(&t, &key).unwrap_err().contains("signature"));
        let mut t = good.clone();
        let e = t["entries"].as_array_mut().unwrap();
        e.remove(1);
        assert!(verify(&t, &key).unwrap_err().contains("out of order"), "a hole in the middle");
        let mut t = good.clone();
        t["entries"].as_array_mut().unwrap().swap(0, 1);
        assert!(verify(&t, &key).unwrap_err().contains("out of order"));
        let other = launcher_chain(OTHER_SEED, &["0.3.1"], "https://m.example.org/r/");
        assert!(verify(&other, &key).unwrap_err().contains("another key"));
        let mut other = other;
        other["key"] = key.clone().into();
        assert!(verify(&other, &key).unwrap_err().contains("signature"));
        assert!(verify(&good, "").is_err());
        assert!(verify(&json!({"format": 2, "entries": []}), &key).is_err());
    }

    #[test]
    fn game_data_and_odd_entries_are_refused_even_when_signed() {
        let key = pub_hex(DEV_TEST_SEED);
        let empty = json!({"format": 1, "key": key, "entries": []});
        for name in ["common.MPQ", "Wow.exe", "Spell.dbc", "../up.zip", "a/b.zip"] {
            let d = append(&empty, DEV_TEST_SEED, "addons", "x", "1", json!([{"name": name, "size": 1, "sha256": "ab".repeat(32)}]), json!({}));
            assert!(verify(&d, &key).is_err(), "{name}");
        }
        let d = append(&empty, DEV_TEST_SEED, "source", "core", "1", json!([]), json!({}));
        assert!(verify(&d, &key).unwrap_err().contains("source"));
        let d = append(&empty, DEV_TEST_SEED, "launcher", "x", "1", json!([]), json!({"mirrors": ["http://plain.example.org/"]}));
        assert!(verify(&d, &key).unwrap_err().contains("mirrors"));
        let d = append(&empty, DEV_TEST_SEED, "money", "x", "1", json!([]), json!({}));
        assert!(verify(&d, &key).unwrap_err().contains("kind"));
    }

    #[test]
    fn forks_truncation_and_the_newest_head() {
        let key = pub_hex(DEV_TEST_SEED);
        let m = "https://m.example.org/r/";
        let two = verify(&launcher_chain(DEV_TEST_SEED, &["0.3.1", "0.3.2"], m), &key).unwrap();
        let four = launcher_chain(DEV_TEST_SEED, &["0.3.1", "0.3.2", "0.3.3", "0.3.4"], m);
        let (c, o) = choose(&two, &four);
        assert_eq!((o, c.unwrap().len()), (Outcome::Adopted, 4));
        let four_c = verify(&four, &key).unwrap();
        // a truncated copy (the end cut off) is good but shorter: kept out
        let (c, o) = choose(&four_c, &two.to_doc());
        assert!(c.is_none() && o == Outcome::Kept);
        // a signed fork that is longer replaces ours; a shorter or equal one does not
        let fork = launcher_chain(DEV_TEST_SEED, &["0.3.1", "0.4.0", "0.4.1", "0.4.2", "0.4.3"], m);
        let (c, o) = choose(&four_c, &fork);
        assert_eq!((o, c.unwrap().len()), (Outcome::Replaced, 5));
        let fork_eq = launcher_chain(DEV_TEST_SEED, &["0.3.1", "0.4.0", "0.4.1", "0.4.2"], m);
        assert_eq!(choose(&four_c, &fork_eq).1, Outcome::Kept);
        // a fork by someone without the key is refused whole
        let mut evil = launcher_chain(OTHER_SEED, &["0.3.1", "0.3.2", "0.3.3", "0.3.4", "9.0.0"], m);
        evil["key"] = key.clone().into();
        assert!(matches!(choose(&four_c, &evil).1, Outcome::Refused(_)));
        // a good chain with a forged entry on the end
        let mut spliced = four.clone();
        let tail = launcher_chain(OTHER_SEED, &["0.3.1", "0.3.2", "0.3.3", "0.3.4", "9.0.0"], m)["entries"][4].clone();
        spliced["entries"].as_array_mut().unwrap().push(tail.clone());
        assert!(matches!(choose(&four_c, &spliced).1, Outcome::Refused(_)));
        assert!(matches!(extend(&four_c, &[tail]).1, Outcome::Refused(_)));
    }

    #[test]
    fn syncs_from_sources_and_keeps_only_good_chains_on_disk() {
        let d = tempfile::tempdir().unwrap();
        let key = pub_hex(DEV_TEST_SEED);
        let m = "https://m.example.org/r/";
        let three = launcher_chain(DEV_TEST_SEED, &["0.3.1", "0.3.2", "0.3.3"], m);
        let five = launcher_chain(DEV_TEST_SEED, &["0.3.1", "0.3.2", "0.3.3", "0.3.4", "0.3.5"], m);
        let mut evil = launcher_chain(OTHER_SEED, &["1", "2", "3", "4", "5", "6"], m);
        evil["key"] = key.clone().into();
        let (t, f, e) = (three.clone(), five.clone(), evil.clone());
        let fetch = move |u: &str| -> Result<Value, String> {
            match u {
                "https://down.example.org/chain.json" => Err("offline".into()),
                "https://site.example.org/chain.json" => Ok(t.clone()),
                "https://evil.example.org/chain.json" => Ok(e.clone()),
                // a mesh node: entries after `from`
                x if x.starts_with("https://relay.example.org/api/mesh/chain?from=") => {
                    let n: usize = x.rsplit('=').next().unwrap().parse().unwrap();
                    let all = f["entries"].as_array().unwrap();
                    Ok(json!({"key": f["key"], "seq": all.len() - 1, "entries": all[n.min(all.len())..].to_vec()}))
                }
                _ => Err("404".into()),
            }
        };
        let srcs: Vec<String> = ["https://down.example.org/chain.json", "https://site.example.org/chain.json", "https://evil.example.org/chain.json",
                                 "https://relay.example.org/api/mesh/chain?from="].iter().map(|s| s.to_string()).collect();
        let v = sync(d.path(), &key, &srcs, &fetch);
        assert_eq!(v.entries, 5, "{:?}", v.notes);
        assert!(v.notes[0].contains("offline") && v.notes[2].contains("Refused"), "{:?}", v.notes);
        assert_eq!(load(d.path(), &key).len(), 5);
        assert_eq!(load(d.path(), &key).newest_launcher_above("0.3.2").unwrap().version, "0.3.5");
        assert!(load(d.path(), &key).newest_launcher_above("0.3.5").is_none());
        // a cache changed on disk is not trusted
        std::fs::write(cache_path(d.path()), evil.to_string()).unwrap();
        assert!(load(d.path(), &key).is_empty());
        // the shipped build trusts no key until the owner makes one, so nothing runs
        if crate::branding::get().release_log.public_key.is_empty() {
            assert!(refresh(d.path(), false).is_none());
        }
    }
}
