//! The Leyline mesh, launcher side (docs/LEYLINE-MESH.md): the realm list read from the network itself, verified
//! here, so it does not depend on any one server. OFF unless the `mesh_realms` flag is on; with it off nothing in
//! this file runs and the Realms page reads today's listings (`realms::fetch_network`).
//!
//! * Sources: up to two peers from this PC's peers cache (`cache/mesh-peers.json`, learned from signed relay
//!   records), then the seed relays (`SEEDS`). Each answers `GET /api/mesh/snapshot` with every record it holds.
//! * Every record is checked here, whoever sent it (the same rules as `hosting/kit/tools/wunet/mesh.py`): the
//!   realm's own Ed25519 signature over its heartbeat (`realm id = base32(sha256(key))[..16]`), the relay's
//!   signature over each endpoint, the relay chosen by the realm itself, expiry (15 minutes for realms), staff
//!   blocks only from pinned keys. A node can leave records out, but it can't add, change or revive one.
//! * Merge: the newest per realm (`seq`), endpoints merged, ties settled by the smaller sha256 of the signature.
//! * Relay per realm (`choose`): our seed relays and the realm's own relay; with `mesh_relays` also community
//!   relays, by measured round trip and published load.
//! * Commonhold v1 (docs/COMMONHOLD-V1.md): `info` records are capability documents (what a relay or host supports,
//!   its protocol levels `proto_min`/`proto_max`, limits, abuse contact as a peer id). The tolerant reader: fields this
//!   code doesn't know are kept (records are stored as sent), and a kind or document version it doesn't know is
//!   ignored (`Err("ignored: ...")`, counted apart from refusals). `commonhold-v1-vectors.json` is the conformance kit,
//!   made by `hosting/kit/tools/wunet/commonhold.py` and checked by both sides.
//! * The result is a realms.json document (`kind: community`, `source: mesh`) that `realms::list_of` reads like
//!   any other listing.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::flags::{canonical, Flags};

/// The seed relays: always tried, last. TODO(owner): more seeds as they come up.
pub const SEEDS: &[&str] = &[crate::relay!(""), "https://relay.openvanilla.app"];   // the relay on both names (addresses.rs)
/// Staff keys whose signed blocks count (hex Ed25519 public keys). TODO(owner): the staff key, once made.
pub const STAFF_KEYS: &[&str] = &[];
pub const REALM_TTL: u64 = 15 * 60;
pub const ENDPOINT_TTL: u64 = 15 * 60;
pub const NODE_TTL: u64 = 30 * 60;
pub const BLOCK_MAX_TTL: u64 = 90 * 86400;
/// A capability document lives a day (wunet/mesh.py INFO_TTL).
pub const INFO_TTL: u64 = 24 * 3600;
/// The Commonhold protocol levels this code speaks (wunet/mesh.py PROTO_MIN / PROTO).
pub const PROTO_MIN: u64 = 1;
pub const PROTO: u64 = 1;
pub const SKEW: u64 = 300;
pub const MAX_ENDPOINTS: usize = 8;
pub const MAX_RECORDS: usize = 50_000;
/// How many sources a refresh asks (at least one must answer).
pub const SOURCES: usize = 3;
const CTX_HEARTBEAT: &str = "wu-listing-heartbeat-v1";
const CTX_ENDPOINT: &str = "leyline-mesh-endpoint-v1";
const CTX_NODE: &str = "leyline-mesh-node-v1";
const CTX_BLOCK: &str = "leyline-mesh-block-v1";
const CTX_INFO: &str = "leyline-mesh-info-v1";
const REGIONS: &[&str] = &["na-east", "na-west", "sa", "eu", "africa", "me", "asia", "oce"];
const TYPES: &[&str] = &["PvE", "PvP", "RP", "RP-PvP", "Fun"];

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

fn hex64(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

/// `realm id = base32(sha256(public key))[..16]`, lower case (wunet/keys.py `realm_id`).
pub fn realm_id(public: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    const A: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let d = Sha256::digest(public);
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for b in d.iter() {
        buf = (buf << 8) | *b as u32;
        bits += 8;
        while bits >= 5 && out.len() < 16 {
            out.push(A[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    out
}

/// Ed25519 over `context + "\n" + canonical(doc)` (wunet/keys.py `verify_doc`).
pub fn verify_doc(pub_hex: &str, doc: &Value, context: &str, sig_hex: &str) -> bool {
    let (Some(pk), Some(sig)) = (unhex(pub_hex), unhex(sig_hex)) else { return false };
    if pk.len() != 32 || sig.len() != 64 {
        return false;
    }
    let msg = format!("{context}\n{}", canonical(doc));
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &pk).verify(msg.as_bytes(), &sig).is_ok()
}

fn int(v: Option<&Value>) -> Option<u64> {
    v.and_then(Value::as_u64)
}

fn fresh(ts: u64, ttl: u64, now: u64) -> bool {
    ts <= now + SKEW && now.saturating_sub(ts) <= ttl
}

/// The relays the realm signed (beat.relay and beat.relays).
pub fn signed_relays(beat: &Value) -> Vec<String> {
    let mut v: Vec<String> = beat.get("relay").and_then(Value::as_str).filter(|h| crate::realms::relay_ok(h)).map(String::from).into_iter().collect();
    if let Some(a) = beat.get("relays").and_then(Value::as_array) {
        v.extend(a.iter().take(MAX_ENDPOINTS).filter_map(Value::as_str).filter(|h| crate::realms::relay_ok(h)).map(String::from));
    }
    v
}

fn endpoint_ok(e: &Value, rid: &str, relays: &[String], now: u64) -> bool {
    let Some(a) = e.get("att") else { return false };
    let host = a.get("relay").and_then(Value::as_str).unwrap_or("");
    let port = |k: &str| int(a.get(k)).is_some_and(|p| (1..=65535).contains(&p));
    a.get("v").and_then(Value::as_u64) == Some(1)
        && a.get("realm_id").and_then(Value::as_str) == Some(rid)
        && relays.iter().any(|r| r == host)
        && port("auth_port")
        && port("world_port")
        && a.get("ws").is_some_and(Value::is_boolean)
        && int(a.get("ts")).is_some_and(|t| fresh(t, ENDPOINT_TTL, now))
        && hex64(a.get("node")).is_some_and(|n| verify_doc(n, a, CTX_ENDPOINT, e.get("sig").and_then(Value::as_str).unwrap_or("")))
}

fn dedupe(eps: Vec<Value>) -> Vec<Value> {
    let mut best: BTreeMap<String, Value> = BTreeMap::new();
    for e in eps {
        let k = e["att"]["node"].as_str().unwrap_or("").to_string();
        let ts = e["att"]["ts"].as_u64().unwrap_or(0);
        if best.get(&k).map_or(true, |b| ts > b["att"]["ts"].as_u64().unwrap_or(0)) {
            best.insert(k, e);
        }
    }
    let mut v: Vec<Value> = best.into_values().collect();
    v.sort_by_key(|e| std::cmp::Reverse(e["att"]["ts"].as_u64().unwrap_or(0)));
    v.truncate(MAX_ENDPOINTS);
    v.sort_by(|a, b| (a["att"]["relay"].as_str(), a["att"]["node"].as_str()).cmp(&(b["att"]["relay"].as_str(), b["att"]["node"].as_str())));
    v
}

/// `(proto_min, proto_max)` of a signed document; missing or malformed: `(1, 1)` (wunet/mesh.py `proto_range`).
pub fn proto_range(doc: &Value) -> (u64, u64) {
    let lo = doc.get("proto_min").map_or(Some(1), Value::as_u64);
    let hi = doc.get("proto_max").map_or(lo, Value::as_u64);
    match (lo, hi) {
        (Some(a), Some(b)) if (1..=65535).contains(&a) && (1..=65535).contains(&b) && a <= b => (a, b),
        _ => (1, 1),
    }
}

/// Whether a document's signer and this launcher share a protocol level.
pub fn speaks(doc: &Value) -> bool {
    let (a, b) = proto_range(doc);
    a <= PROTO && PROTO_MIN <= b
}

/// The version gate of the tolerant reader: v1 is checked, a newer v is ignored, anything else refused.
fn version(doc: Option<&Value>, what: &str) -> Result<(), String> {
    match doc.and_then(|d| d.get("v")).and_then(Value::as_u64) {
        Some(1) => Ok(()),
        Some(v) if v >= 2 && doc.is_some_and(Value::is_object) => Err(format!("ignored: {what} v{v} is newer than this launcher")),
        _ => Err(format!("no {what}")),
    }
}

fn token_ok(s: &str, first_lower_only: bool, max: usize) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && b.len() <= max
        && (b[0].is_ascii_lowercase() || (!first_lower_only && b[0].is_ascii_digit()))
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || (if first_lower_only { *c == b'_' } else { *c == b'.' || *c == b'-' }))
}

fn lang_ok(s: &str) -> bool {
    let (a, b) = s.split_once('-').map(|(a, b)| (a, Some(b))).unwrap_or((s, None));
    (2..=3).contains(&a.len()) && a.bytes().all(|c| c.is_ascii_lowercase())
        && b.map_or(true, |b| (2..=8).contains(&b.len()) && b.bytes().all(|c| c.is_ascii_alphanumeric()))
}

/// The fields of a capability document (wunet/mesh.py `_check_info`). Fields not named here are kept, unchecked.
fn info_fields_ok(d: &Value) -> Result<(), String> {
    let s = |k: &str| d.get(k).map_or(Some(""), Value::as_str);
    let list = |k: &str, max: usize, ok: &dyn Fn(&str) -> bool| match d.get(k) {
        None => true,
        Some(Value::Array(a)) => a.len() <= max && a.iter().all(|x| x.as_str().is_some_and(ok)),
        _ => false,
    };
    if !matches!(d.get("role").and_then(Value::as_str), Some("relay" | "host")) {
        return Err("info role".into());
    }
    let (lo, hi) = (int(d.get("proto_min")), int(d.get("proto_max")));
    if !matches!((lo, hi), (Some(a), Some(b)) if (1..=65535).contains(&a) && (1..=65535).contains(&b) && a <= b) {
        return Err("info proto_min/proto_max".into());
    }
    if !s("software").is_some_and(|x| x.chars().count() <= 40) || !s("version").is_some_and(|x| x.chars().count() <= 32) {
        return Err("info software".into());
    }
    if !list("supported", 32, &|c: &str| token_ok(c, false, 32)) {
        return Err("info supported".into());
    }
    let lim_ok = match d.get("limits") {
        None => true,
        Some(Value::Object(m)) => m.len() <= 16 && m.iter().all(|(k, v)| token_ok(k, true, 24) && v.as_u64().is_some()),
        _ => false,
    };
    if !lim_ok {
        return Err("info limits".into());
    }
    if !s("admin").is_some_and(|a| a.is_empty() || crate::realms::realm_id_ok(a)) {
        return Err("info admin".into());
    }
    if !s("rules_url").is_some_and(|u| u.is_empty() || (u.len() <= 200 && u.starts_with("https://"))) {
        return Err("info rules_url".into());
    }
    if !s("region").is_some_and(|r| r.is_empty() || REGIONS.contains(&r)) {
        return Err("info region".into());
    }
    if !list("languages", 8, &lang_ok) || !list("restricted", 8, &|c: &str| token_ok(c, false, 32)) {
        return Err("info languages/restricted".into());
    }
    Ok(())
}

/// A checked record: (id, version, the record with only its good endpoints). The rules of wunet/mesh.py `check`.
/// `Err` starting "ignored:" is the tolerant reader's answer (a kind or version from a newer spec), not a refusal.
pub fn check(rec: &Value, now: u64, staff: &[&str]) -> Result<(String, u64, Value), String> {
    if canonical(rec).len() > 6000 {
        return Err("too big".into());
    }
    let sig = rec.get("sig").and_then(Value::as_str).unwrap_or("");
    match rec.get("kind").and_then(Value::as_str) {
        Some("realm") => {
            version(rec.get("beat"), "beat")?;
            let beat = &rec["beat"];
            let pk = hex64(beat.get("pubkey")).ok_or("beat pubkey")?;
            let rid = realm_id(&unhex(pk).unwrap_or_default());
            if beat.get("realm_id").and_then(Value::as_str) != Some(rid.as_str()) {
                return Err("realm id does not match the key".into());
            }
            let seq = int(beat.get("seq")).ok_or("beat seq")?;
            let ts = int(beat.get("ts")).ok_or("beat ts")?;
            if !fresh(ts, REALM_TTL, now) {
                return Err("expired".into());
            }
            if !beat.get("record").and_then(|r| r.get("name")).and_then(Value::as_str).is_some_and(|n| !n.trim().is_empty()) {
                return Err("record.name".into());
            }
            if !verify_doc(pk, beat, CTX_HEARTBEAT, sig) {
                return Err("signature".into());
            }
            let relays = signed_relays(beat);
            let eps: Vec<Value> = rec.get("endpoints").and_then(Value::as_array).map(|a| a.iter().filter(|e| endpoint_ok(e, &rid, &relays, now)).cloned().collect()).unwrap_or_default();
            let mut out = rec.clone();
            out["endpoints"] = Value::Array(dedupe(eps));
            Ok((format!("r-{rid}"), seq, out))
        }
        Some("relay") => {
            version(rec.get("node"), "node")?;
            let n = &rec["node"];
            let pk = hex64(n.get("pubkey")).ok_or("node pubkey")?;
            let host = n.get("host").and_then(Value::as_str).filter(|h| crate::realms::relay_ok(h)).ok_or("node host")?;
            let http = n.get("http").and_then(Value::as_str).unwrap_or("");
            let rest = http.strip_prefix("https://").or_else(|| http.strip_prefix("http://")).unwrap_or("");
            let (h, port) = rest.split_once(':').map(|(h, p)| (h, Some(p))).unwrap_or((rest, None));
            if h != host || port.is_some_and(|p| p.parse::<u16>().is_err()) {
                return Err("node http".into());
            }
            let seq = int(n.get("seq")).ok_or("node seq")?;
            if !int(n.get("ts")).is_some_and(|t| fresh(t, NODE_TTL, now)) {
                return Err("expired".into());
            }
            if !verify_doc(pk, n, CTX_NODE, sig) {
                return Err("signature".into());
            }
            Ok((format!("n-{}", realm_id(&unhex(pk).unwrap_or_default())), seq, rec.clone()))
        }
        Some("block") => {
            version(rec.get("block"), "block")?;
            let b = &rec["block"];
            let st = b.get("staff").and_then(Value::as_str).filter(|s| staff.contains(s)).ok_or("not a staff key")?;
            let t = b.get("target").and_then(Value::as_str).unwrap_or("");
            if !(t.len() == 18 && (t.starts_with("r-") || t.starts_with("n-")) && crate::realms::realm_id_ok(&t[2..])) {
                return Err("block target".into());
            }
            let (ts, exp) = (int(b.get("ts")).ok_or("block ts")?, int(b.get("expires")).ok_or("block expires")?);
            if ts > now + SKEW || exp <= now || exp.saturating_sub(ts) > BLOCK_MAX_TTL {
                return Err("block expired or too long".into());
            }
            if !verify_doc(st, b, CTX_BLOCK, sig) {
                return Err("signature".into());
            }
            Ok((format!("b-{t}"), ts, rec.clone()))
        }
        Some("info") => {
            version(rec.get("info"), "info")?;
            let d = &rec["info"];
            let pk = hex64(d.get("pubkey")).ok_or("info pubkey")?;
            let seq = int(d.get("seq")).ok_or("info seq")?;
            let ts = int(d.get("ts")).ok_or("info ts")?;
            info_fields_ok(d)?;
            if !fresh(ts, INFO_TTL, now) {
                return Err("expired".into());
            }
            if !verify_doc(pk, d, CTX_INFO, sig) {
                return Err("signature".into());
            }
            Ok((format!("i-{}", realm_id(&unhex(pk).unwrap_or_default())), seq, rec.clone()))
        }
        Some(k) => Err(format!("ignored: unknown kind {}", k.chars().take(24).collect::<String>())),
        None => Err("ignored: no kind".into()),
    }
}

fn sig_rank(rec: &Value) -> String {
    use sha2::{Digest, Sha256};
    crate::net::hex(&Sha256::digest(rec.get("sig").and_then(Value::as_str).unwrap_or("").as_bytes()))
}

/// The verified records from every source, merged.
#[derive(Debug, Default, Clone)]
pub struct Merged {
    pub recs: BTreeMap<String, (u64, Value)>,
    pub refused: usize,
    /// Records of a kind or version this launcher doesn't know (the tolerant reader): not stored, not refusals.
    pub ignored: usize,
}

/// The id a block names for a record: an info document is blocked with the relay or realm it describes.
fn subject(id: &str, rec: &Value) -> String {
    match id.strip_prefix("i-") {
        Some(k) if rec["info"]["role"].as_str() == Some("relay") => format!("n-{k}"),
        Some(k) => format!("r-{k}"),
        None => id.to_string(),
    }
}

impl Merged {
    /// Offer one record; true when it changed what is known.
    pub fn offer(&mut self, rec: &Value, now: u64, staff: &[&str]) -> bool {
        let (id, ver, mut rec) = match check(rec, now, staff) {
            Ok(x) => x,
            Err(e) => {
                if e.starts_with("ignored:") {
                    self.ignored += 1;
                } else {
                    self.refused += 1;
                }
                return false;
            }
        };
        if !id.starts_with("b-") && self.recs.contains_key(&format!("b-{}", subject(&id, &rec))) {
            self.refused += 1;
            return false;
        }
        if let Some((cv, cur)) = self.recs.get(&id) {
            if ver < *cv {
                return false;
            }
            if ver == *cv {
                if rec.get("sig") == cur.get("sig") {
                    if !id.starts_with("r-") {
                        return false;
                    }
                    let mut all = cur["endpoints"].as_array().cloned().unwrap_or_default();
                    all.extend(rec["endpoints"].as_array().cloned().unwrap_or_default());
                    let merged = Value::Array(dedupe(all));
                    if merged == cur["endpoints"] {
                        return false;
                    }
                    rec["endpoints"] = merged;
                } else if sig_rank(&rec) >= sig_rank(cur) {
                    return false;
                }
            }
        } else if self.recs.len() >= MAX_RECORDS {
            return false;
        }
        if let Some(t) = id.strip_prefix("b-") {
            self.recs.remove(t);
            self.recs.remove(&format!("i-{}", &t[2..]));
        }
        self.recs.insert(id, (ver, rec));
        true
    }

    pub fn realms(&self) -> impl Iterator<Item = &Value> {
        self.recs.iter().filter(|(k, _)| k.starts_with("r-")).map(|(_, (_, v))| v)
    }

    /// The verified capability document of a relay or realm (`n-<id>` or `r-<id>`), if one reached us.
    pub fn info_of(&self, id: &str) -> Option<&Value> {
        self.recs.get(&format!("i-{}", id.get(2..)?)).map(|(_, v)| &v["info"])
    }

    /// host -> the node dict of every verified relay record.
    pub fn relays(&self) -> HashMap<String, Value> {
        self.recs.iter().filter(|(k, _)| k.starts_with("n-")).filter_map(|(_, (_, v))| {
            let n = v.get("node")?;
            Some((n.get("host")?.as_str()?.to_string(), n.clone()))
        }).collect()
    }
}

/// The endpoint to play a realm through (wunet/mesh.py `choose_endpoint`): cost = rtt * (1 + 2 * load), +40 % when
/// the relay is full of realms; without `volunteers` only seed relays and the realm's own relay count; `browser`
/// needs the WebSocket bridge. Unmeasured rtt counts as 250 ms.
pub fn choose<'a>(rec: &'a Value, relays: &HashMap<String, Value>, rtt: &HashMap<String, f64>, volunteers: bool, seeds: &[String], browser: bool) -> Option<&'a Value> {
    let own = rec["beat"]["relay"].as_str().unwrap_or("");
    let mut best: Option<((u64, u8, String), &Value)> = None;
    for e in rec.get("endpoints").and_then(Value::as_array).into_iter().flatten() {
        let a = &e["att"];
        let h = a["relay"].as_str().unwrap_or("");
        let seed = seeds.iter().any(|s| s == h);
        if (browser && a["ws"] != Value::Bool(true)) || (!volunteers && !seed && h != own) {
            continue;
        }
        let n = relays.get(h).cloned().unwrap_or(Value::Null);
        let cap = |k: &str| n["caps"][k].as_u64().unwrap_or(0);
        let load = |k: &str| n["load"][k].as_u64().unwrap_or(0);
        let lf = if cap("streams") > 0 { (load("streams") as f64 / cap("streams") as f64).min(1.0) } else if seed { 0.0 } else { 0.5 };
        let mut cost = rtt.get(h).copied().unwrap_or(250.0) * (1.0 + 2.0 * lf);
        if cap("realms") > 0 && load("realms") >= cap("realms") {
            cost *= 1.4;
        }
        let key = ((cost * 1000.0) as u64, if seed { 0 } else { 1 }, h.to_string());
        if best.as_ref().map_or(true, |(k, _)| key < *k) {
            best = Some((key, e));
        }
    }
    best.map(|(_, e)| e)
}

fn score(live: bool, players: u64, bots: u64) -> f64 {
    if !live {
        return 0.0;
    }
    let p = (8.0 * (players as f64).sqrt()).min(40.0);
    let b = ((bots as f64).sqrt() / 2.0).min(5.0);
    ((p + b + 20.0) * 10.0).round() / 10.0
}

/// A verified realm record as a realms.json record (what `realms::parse_community` reads), reached through `ep`.
pub fn view(rec: &Value, ep: Option<&Value>, now: u64) -> Value {
    let beat = &rec["beat"];
    let rid = beat["realm_id"].as_str().unwrap_or("");
    let r = &beat["record"];
    let st = &beat["status"];
    let a = ep.map(|e| &e["att"]);
    let relay = a.and_then(|a| a["relay"].as_str()).or_else(|| beat["relay"].as_str()).unwrap_or("");
    let ts = beat["ts"].as_u64().unwrap_or(0);
    let online = st["online"] == Value::Bool(true) && a.is_some() && now.saturating_sub(ts) <= 180 + 300;
    let n = |k: &str| st[k].as_u64().filter(|x| *x < 1_000_000);
    let text = |k: &str| r[k].as_str().unwrap_or("");
    let kind = Some(text("type")).filter(|t| TYPES.contains(t)).unwrap_or("PvE");
    let discord = r["discord"].as_str().filter(|d| d.starts_with("https://discord.gg/") || d.starts_with("https://discord.com/invite/"));
    let mut profile = Map::new();
    profile.insert("build".into(), r.get("build").cloned().unwrap_or(json!(12340)));
    profile.insert("core".into(), json!(r["core"].as_str().unwrap_or("azerothcore")));
    profile.insert("address".into(), json!(relay));
    profile.insert("port".into(), a.map(|a| a["auth_port"].clone()).unwrap_or(Value::Null));
    json!({
        "schema": 1, "slug": format!("c-{rid}"), "name": text("name"), "official": false,
        "state": if online { "live" } else { "offline" },
        "tagline": text("tagline"), "summary": text("summary"), "type": kind,
        "profile": Value::Object(profile),
        "tags": r.get("tags").cloned().unwrap_or(json!([])),
        "region": r.get("region").cloned().unwrap_or(Value::Null),
        "language": r.get("language").cloned().unwrap_or(Value::Null),
        "maxPlayers": r.get("max_players").cloned().unwrap_or(Value::Null),
        "rules": r.get("rules").cloned().unwrap_or(json!([])),
        "discord": discord,
        "connect": {"kind": "web-signin", "url": format!("https://{relay}/join/{rid}")},
        "status": {"url": format!("https://{relay}/r/{rid}/status.json"), "format": "openvanilla-status-1"},
        "community": {
            "realmId": rid, "relay": relay, "signed": true,
            "lastHeartbeat": {"online": st["online"] == Value::Bool(true), "players": n("players"), "bots": n("bots")},
            "score": score(online, n("players").unwrap_or(0), n("bots").unwrap_or(0)),
            "browser": a.is_some_and(|a| a["ws"] == Value::Bool(true)),
            "endpoints": rec["endpoints"].as_array().map(|x| x.len()).unwrap_or(0),
        },
    })
}

/// The realms.json document for everything verified: each realm through its chosen relay. When that is a community
/// node, `community.fallback` names the relay to use at once if it does not answer: one of ours, or the realm's own.
pub fn realms_doc(m: &Merged, rtt: &HashMap<String, f64>, volunteers: bool, seeds: &[String], now: u64) -> Value {
    let relays = m.relays();
    // a realm that needs a newer protocol than this launcher speaks is not shown (the records are still kept)
    let realms: Vec<Value> = m.realms().filter(|r| speaks(&r["beat"]["record"])).map(|r| {
        let ep = choose(r, &relays, rtt, volunteers, seeds, false);
        let mut v = view(r, ep, now);
        let safe = choose(r, &relays, rtt, false, seeds, false);
        if let (Some(e), Some(f)) = (ep, safe) {
            if e["att"]["relay"] != f["att"]["relay"] {
                v["community"]["fallback"] = json!({"address": f["att"]["relay"], "port": f["att"]["auth_port"],
                                                    "browser": f["att"]["ws"] == Value::Bool(true)});
            }
        }
        v
    }).collect();
    json!({"schema": 1, "kind": "community", "source": "mesh", "realms": realms})
}

/// The seeds' word on community nodes (`GET /api/nodes.json`, hosting/relay/wurelay/nodes.py): the kill switch, and
/// the hosts that passed the seed's last health check.
pub fn node_health(doc: &Value) -> (bool, HashSet<String>) {
    let kill = doc["kill"] == Value::Bool(true);
    let hosts = doc["nodes"].as_array().into_iter().flatten()
        .filter(|n| n["healthy"] == Value::Bool(true))
        .filter_map(|n| n["host"].as_str().map(String::from))
        .collect();
    (kill, hosts)
}

/// The record with the endpoints on community nodes the seeds don't vouch for left out (our relays and the realm's
/// own relay always stay).
pub fn keep_healthy(rec: &Value, healthy: &HashSet<String>, seeds: &[String]) -> Value {
    let own = rec["beat"]["relay"].as_str().unwrap_or("");
    let mut r = rec.clone();
    if let Some(eps) = r.get_mut("endpoints").and_then(Value::as_array_mut) {
        eps.retain(|e| {
            let h = e["att"]["relay"].as_str().unwrap_or("");
            h == own || seeds.iter().any(|s| s == h) || healthy.contains(h)
        });
    }
    r
}

// ------------------------------------------------------------------------------------------- sources and peers
pub fn peers_path(app_dir: &Path) -> PathBuf {
    app_dir.join("cache").join("mesh-peers.json")
}

pub fn load_peers(app_dir: &Path) -> Vec<String> {
    std::fs::read(peers_path(app_dir)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("peers").and_then(Value::as_array).cloned())
        .map(|a| a.iter().filter_map(Value::as_str).filter(|u| crate::net::plain_https(u)).take(64).map(String::from).collect())
        .unwrap_or_default()
}

/// Remember mesh peers from verified relay records (https only, at most 64, the newest first).
pub fn learn_peers(app_dir: &Path, m: &Merged) {
    let mut nodes: Vec<(u64, String)> = m.relays().into_values()
        .filter(|n| n["mesh"] == Value::Bool(true))
        .filter_map(|n| Some((n["ts"].as_u64()?, n["http"].as_str().filter(|u| crate::net::plain_https(u))?.to_string())))
        .collect();
    nodes.sort_by(|a, b| b.cmp(a));
    let mut peers: Vec<String> = nodes.into_iter().map(|(_, u)| u).collect();
    for u in load_peers(app_dir) {
        if !peers.contains(&u) {
            peers.push(u);
        }
    }
    peers.truncate(64);
    let _ = std::fs::create_dir_all(app_dir.join("cache"));
    let _ = crate::settings::write_atomic(&peers_path(app_dir), json!({"peers": peers}).to_string().as_bytes());
}

/// Who to ask, in order: two cached peers (a different pair each minute), then the seeds.
pub fn sources(app_dir: &Path, seeds: &[&str]) -> Vec<String> {
    let mut peers: Vec<String> = load_peers(app_dir).into_iter().filter(|p| !seeds.contains(&p.as_str())).collect();
    let mut out = Vec::new();
    if !peers.is_empty() {
        let k = (now_secs() / 60) as usize % peers.len();
        peers.rotate_left(k);
        out.extend(peers.into_iter().take(2));
    }
    out.extend(seeds.iter().map(|s| s.to_string()));
    out
}

/// Ask sources for their snapshot until `SOURCES` answered; merge what verifies. None when nobody answered.
pub fn gather(sources: &[String], fetch: &dyn Fn(&str) -> Result<Value, String>, now: u64, staff: &[&str]) -> Option<Merged> {
    let mut m = Merged::default();
    let mut answered = 0;
    for s in sources {
        if answered >= SOURCES {
            break;
        }
        let Ok(doc) = fetch(&format!("{}/api/mesh/snapshot", s.trim_end_matches('/'))) else { continue };
        let Some(recs) = doc.get("records").and_then(Value::as_array) else { continue };
        answered += 1;
        for r in recs.iter().take(MAX_RECORDS) {
            m.offer(r, now, staff);
        }
    }
    (answered > 0).then_some(m)
}

/// Round trips to each relay (a TCP connect to port 443), in parallel, at most `timeout` each.
pub fn probe(hosts: &[String], timeout: Duration) -> HashMap<String, f64> {
    use std::net::{TcpStream, ToSocketAddrs};
    let handles: Vec<_> = hosts.iter().take(16).cloned().map(|h| std::thread::spawn(move || {
        let addr = (h.as_str(), 443).to_socket_addrs().ok()?.next()?;
        let t = Instant::now();
        TcpStream::connect_timeout(&addr, timeout).ok()?;
        Some((h, t.elapsed().as_secs_f64() * 1000.0))
    })).collect();
    handles.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
}

/// The realm list from the mesh, or None (flag off, or no source answered): the caller then uses today's listings.
pub fn realms_doc_for(app_dir: &Path, flags: &Flags, seeds: &[&str], fetch: &dyn Fn(&str) -> Result<Value, String>,
                      rtt: &dyn Fn(&[String]) -> HashMap<String, f64>) -> Option<Value> {
    if !flags.mesh_realms {
        return None;
    }
    let now = now_secs();
    let m = gather(&sources(app_dir, seeds), fetch, now, STAFF_KEYS)?;
    learn_peers(app_dir, &m);
    let seeds_h: Vec<String> = seeds.iter().filter_map(|s| s.split("://").nth(1)).map(|h| h.split(':').next().unwrap_or(h).to_string()).collect();
    // community nodes: only the ones a seed checked lately, and none at all while the Leyline team's switch is off
    let (mut volunteers, mut m) = (flags.mesh_relays, m);
    if volunteers {
        let health = seeds.iter().find_map(|s| fetch(&format!("{}/api/nodes.json", s.trim_end_matches('/'))).ok());
        match health.as_ref().map(node_health) {
            Some((false, healthy)) => {
                for (k, (_, r)) in m.recs.iter_mut() {
                    if k.starts_with("r-") {
                        *r = keep_healthy(r, &healthy, &seeds_h);
                    }
                }
            }
            _ => volunteers = false,   // the switch is off, or no seed answered: our relays only
        }
    }
    let hosts: Vec<String> = if volunteers { m.relays().into_keys().collect() } else { vec![] };
    let doc = realms_doc(&m, &rtt(&hosts), volunteers, &seeds_h, now);
    Some(doc)
}

/// What `realms::fetch_all` calls: the real network.
pub fn fetch_doc(app_dir: &Path, flags: &Flags) -> Option<Value> {
    realms_doc_for(app_dir, flags, SEEDS, &|u| crate::net::get_json(u, crate::net::TIMEOUT), &|h| probe(h, Duration::from_millis(1500)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::testserver::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    const VECTOR: &str = include_str!("mesh-vector.json");
    const VTS: u64 = 1761700000;

    struct Key(Ed25519KeyPair);
    impl Key {
        fn new(seed: u8) -> Key {
            Key(Ed25519KeyPair::from_seed_unchecked(&[seed; 32]).unwrap())
        }
        fn pubhex(&self) -> String {
            crate::net::hex(self.0.public_key().as_ref())
        }
        fn rid(&self) -> String {
            realm_id(self.0.public_key().as_ref())
        }
        fn sign(&self, ctx: &str, doc: &Value) -> String {
            crate::net::hex(self.0.sign(format!("{ctx}\n{}", canonical(doc)).as_bytes()).as_ref())
        }
        fn realm(&self, seq: u64, ts: u64, name: &str, relay: &str, eps: Vec<Value>) -> Value {
            let beat = json!({"v": 1, "realm_id": self.rid(), "pubkey": self.pubhex(), "seq": seq, "ts": ts, "relay": relay,
                "record": {"name": name, "type": "PvP"}, "status": {"online": true, "players": 4, "bots": 9}});
            json!({"kind": "realm", "sig": self.sign(CTX_HEARTBEAT, &beat), "beat": beat, "endpoints": eps})
        }
        fn ep(&self, rid: &str, relay: &str, ts: u64, port: u64, ws: bool) -> Value {
            let att = json!({"v": 1, "realm_id": rid, "relay": relay, "node": self.pubhex(), "auth_port": port, "world_port": port + 1, "ws": ws, "ts": ts});
            json!({"att": att, "sig": self.sign(CTX_ENDPOINT, &att)})
        }
        fn node(&self, host: &str, http: &str, ts: u64, streams: (u64, u64)) -> Value {
            let n = json!({"v": 1, "pubkey": self.pubhex(), "host": host, "http": http, "seq": ts * 1000, "ts": ts, "region": "",
                "volunteer": true, "seed": false, "ws": true, "mesh": true, "caps": {"realms": 25, "streams": streams.1, "kbps": 256},
                "load": {"realms": 1, "streams": streams.0}});
            json!({"kind": "relay", "sig": self.sign(CTX_NODE, &n), "node": n})
        }
    }

    #[test]
    fn python_signed_records_verify_here() {
        let v: Value = serde_json::from_str(VECTOR).unwrap();
        let recs = v["records"].as_array().unwrap();
        let (id, seq, rec) = check(&recs[0], VTS + 10, &[]).unwrap();
        assert_eq!((id.as_str(), seq), ("r-kzdvvj2umnduyauf", 1761700000123));
        assert_eq!(rec["endpoints"].as_array().unwrap().len(), 1, "the relay's endpoint verifies too");
        assert!(check(&recs[1], VTS + 10, &[]).unwrap().0.starts_with("n-"));
        let mut t = recs[0].clone();
        t["beat"]["status"]["players"] = json!(13);
        assert_eq!(check(&t, VTS + 10, &[]).unwrap_err(), "signature");
        assert_eq!(check(&recs[0], VTS + REALM_TTL + 1, &[]).unwrap_err(), "expired");
        let mut t = recs[0].clone();
        t["endpoints"][0]["att"]["auth_port"] = json!(1);
        assert_eq!(check(&t, VTS + 10, &[]).unwrap().2["endpoints"].as_array().unwrap().len(), 0, "a tampered endpoint is dropped");
    }

    #[test]
    fn merge_rules() {
        let (r, a, b) = (Key::new(1), Key::new(2), Key::new(3));
        let now = now_secs();
        let rid = r.rid();
        let mut m = Merged::default();
        let first = r.realm(5, now, "One", "a.example.org", vec![a.ep(&rid, "a.example.org", now, 40000, true)]);
        assert!(m.offer(&first, now, &[]));
        assert!(!m.offer(&first, now, &[]), "nothing new");
        let rogue = b.ep(&rid, "b.example.org", now, 40000, true);
        assert!(!m.offer(&r.realm(5, now, "One", "a.example.org", vec![rogue]), now, &[]), "a relay the realm never chose");
        assert!(!m.offer(&r.realm(4, now, "Old", "a.example.org", vec![]), now, &[]));
        assert!(m.offer(&r.realm(6, now, "Two", "a.example.org", vec![]), now, &[]));
        assert_eq!(m.realms().next().unwrap()["beat"]["record"]["name"], "Two");
        let mut forged = r.realm(9, now, "Three", "a.example.org", vec![]);
        forged["beat"]["record"]["name"] = json!("Forged");
        assert!(!m.offer(&forged, now, &[]));
        assert_eq!(m.refused, 1);
        // equivocation: the same answer whatever the order
        let (x, y) = (r.realm(7, now, "X", "a.example.org", vec![]), r.realm(7, now, "Y", "a.example.org", vec![]));
        let (mut m1, mut m2) = (Merged::default(), Merged::default());
        m1.offer(&x, now, &[]);
        m1.offer(&y, now, &[]);
        m2.offer(&y, now, &[]);
        m2.offer(&x, now, &[]);
        assert_eq!(m1.realms().next().unwrap()["sig"], m2.realms().next().unwrap()["sig"]);
        // staff blocks: only pinned keys
        let staff = Key::new(9);
        let blk = json!({"v": 1, "target": format!("r-{rid}"), "reason": "spam", "ts": now, "expires": now + 3600, "staff": staff.pubhex()});
        let rec = json!({"kind": "block", "sig": staff.sign(CTX_BLOCK, &blk), "block": blk});
        assert!(!m.offer(&rec, now, &[]));
        let sk = staff.pubhex();
        assert!(m.offer(&rec, now, &[sk.as_str()]));
        assert_eq!(m.realms().count(), 0);
        assert!(!m.offer(&r.realm(99, now, "Back", "a.example.org", vec![]), now, &[sk.as_str()]), "blocked");
    }

    #[test]
    fn relay_choice() {
        let (r, seed, v1, v2) = (Key::new(1), Key::new(2), Key::new(3), Key::new(4));
        let now = now_secs();
        let rid = r.rid();
        let beat = json!({"v": 1, "realm_id": rid, "pubkey": r.pubhex(), "seq": 1, "ts": now, "relay": "relay.openvanilla.app",
            "relays": ["v1.example.org", "v2.example.org"], "record": {"name": "Many"}, "status": {"online": true}});
        let rec = json!({"kind": "realm", "sig": r.sign(CTX_HEARTBEAT, &beat), "beat": beat, "endpoints": [
            seed.ep(&rid, "relay.openvanilla.app", now, 40000, true), v1.ep(&rid, "v1.example.org", now, 41000, true),
            v2.ep(&rid, "v2.example.org", now, 41010, false)]});
        let mut m = Merged::default();
        assert!(m.offer(&rec, now, &[]));
        m.offer(&v1.node("v1.example.org", "https://v1.example.org", now, (10, 100)), now, &[]);
        m.offer(&v2.node("v2.example.org", "https://v2.example.org", now, (0, 100)), now, &[]);
        let rtt: HashMap<String, f64> = [("relay.openvanilla.app", 120.0), ("v1.example.org", 30.0), ("v2.example.org", 25.0)].into_iter().map(|(a, b)| (a.to_string(), b)).collect();
        let seeds = vec!["relay.openvanilla.app".to_string()];
        let (rec, rel) = (m.realms().next().unwrap().clone(), m.relays());
        let pick = |vol: bool, br: bool| choose(&rec, &rel, &rtt, vol, &seeds, br).unwrap()["att"]["relay"].as_str().unwrap().to_string();
        assert_eq!(pick(false, false), "relay.openvanilla.app", "community relays off: the seed");
        assert_eq!(pick(true, false), "v2.example.org");
        assert_eq!(pick(true, true), "v1.example.org", "the browser needs the bridge");
        // the list view goes through parse_community like any listing
        let doc = realms_doc(&m, &rtt, true, &seeds, now);
        let realms = crate::realms::parse_community(&doc);
        assert_eq!(realms.len(), 1);
        assert_eq!((realms[0].address.as_deref(), realms[0].state.as_str(), realms[0].kind.as_str()), (Some("v2.example.org:41010"), "live", "PvE"));
        assert_eq!(realms[0].join_url.as_deref(), Some(format!("https://v2.example.org/join/{rid}").as_str()));
        // a community node in use: our relay is named as the instant fallback
        let r0 = &doc["realms"][0]["community"]["fallback"];
        assert_eq!((r0["address"].as_str(), r0["port"].as_u64()), (Some("relay.openvanilla.app"), Some(40000)));
        assert!(realms_doc(&m, &rtt, false, &seeds, now)["realms"][0]["community"].get("fallback").is_none(), "already ours");
        // only nodes a seed vouches for: v2 failed its health check, so v1 is the pick
        let (kill, healthy) = node_health(&json!({"kill": false, "nodes": [{"host": "v1.example.org", "healthy": true},
                                                                           {"host": "v2.example.org", "healthy": false}]}));
        assert!(!kill);
        let kept = keep_healthy(&rec, &healthy, &seeds);
        let hosts: Vec<&str> = kept["endpoints"].as_array().unwrap().iter().map(|e| e["att"]["relay"].as_str().unwrap()).collect();
        assert_eq!(hosts, ["relay.openvanilla.app", "v1.example.org"], "our relay and the healthy node stay");
        assert_eq!(choose(&kept, &rel, &rtt, true, &seeds, false).unwrap()["att"]["relay"], "v1.example.org");
        assert!(node_health(&json!({"kill": true, "nodes": []})).0);
    }

    #[test]
    fn the_kill_switch_means_our_relays_only() {
        let (r, seed, v) = (Key::new(1), Key::new(2), Key::new(3));
        let now = now_secs();
        let rid = r.rid();
        let beat = json!({"v": 1, "realm_id": rid, "pubkey": r.pubhex(), "seq": 1, "ts": now, "relay": "relay.openvanilla.app",
            "relays": ["v1.example.org"], "record": {"name": "Many"}, "status": {"online": true}});
        let rec = json!({"kind": "realm", "sig": r.sign(CTX_HEARTBEAT, &beat), "beat": beat, "endpoints": [
            seed.ep(&rid, "relay.openvanilla.app", now, 40000, true), v.ep(&rid, "v1.example.org", now, 41000, true)]});
        let snap = json!({"v": 1, "records": [rec, v.node("v1.example.org", "https://v1.example.org", now, (0, 100))]});
        let d = tempfile::tempdir().unwrap();
        let fetch_with = |kill: bool| {
            let snap = snap.clone();
            move |u: &str| -> Result<Value, String> {
                if u.ends_with("/api/nodes.json") {
                    return Ok(json!({"kill": kill, "nodes": [{"host": "v1.example.org", "healthy": true}]}));
                }
                if u.ends_with("/api/mesh/snapshot") { Ok(snap.clone()) } else { Err("no".into()) }
            }
        };
        let rtt = |_: &[String]| -> HashMap<String, f64> { [("relay.openvanilla.app".to_string(), 200.0), ("v1.example.org".to_string(), 10.0)].into_iter().collect() };
        let on = Flags { mesh_realms: true, mesh_relays: true, ..Default::default() };
        let seeds = ["https://relay.openvanilla.app"];
        let doc = realms_doc_for(d.path(), &on, &seeds, &fetch_with(false), &rtt).unwrap();
        assert_eq!(doc["realms"][0]["profile"]["address"], "v1.example.org", "the closer healthy node");
        let doc = realms_doc_for(d.path(), &on, &seeds, &fetch_with(true), &rtt).unwrap();
        assert_eq!(doc["realms"][0]["profile"]["address"], "relay.openvanilla.app", "the team's switch: ours only");
    }

    #[test]
    fn bootstrap_peers_and_the_flag() {
        let (r, n) = (Key::new(1), Key::new(2));
        let now = now_secs();
        let rid = r.rid();
        let snap = json!({"v": 1, "records": [r.realm(1, now, "Frost Haven", "relay.openvanilla.app",
            vec![n.ep(&rid, "relay.openvanilla.app", now, 40002, true)]), n.node("relay.openvanilla.app", "https://relay.openvanilla.app", now, (5, 4096)),
            Key::new(5).node("v5.example.org", "https://v5.example.org", now, (0, 512)), json!({"kind": "realm", "beat": {}})]});
        let base = serve_routes(vec![("/api/mesh/snapshot", "200 OK", snap.to_string().into_bytes())]);
        let d = tempfile::tempdir().unwrap();
        let seeds_owned = [dead_url(), base.clone()];
        let seeds: Vec<&str> = seeds_owned.iter().map(|s| s.as_str()).collect();
        let fetch = |u: &str| crate::net::get_json(u, Duration::from_secs(3));
        let no_rtt = |_: &[String]| HashMap::new();
        let off = Flags::default();
        assert!(realms_doc_for(d.path(), &off, &seeds, &fetch, &no_rtt).is_none(), "flag off: the mesh is never asked");
        let on = Flags { mesh_realms: true, ..Default::default() };
        let doc = realms_doc_for(d.path(), &on, &seeds, &fetch, &no_rtt).unwrap();
        let realms = crate::realms::parse_community(&doc);
        assert_eq!(realms.len(), 1);
        assert_eq!(realms[0].address.as_deref(), Some("relay.openvanilla.app:40002"));
        let peers = load_peers(d.path());
        assert_eq!(peers.len(), 2, "learned from the signed relay records: {peers:?}");
        assert!(sources(d.path(), &seeds).len() == 4 && sources(d.path(), &seeds)[2] == seeds[0], "two cached peers, then the seeds");
        let dead = [dead_url()];
        let dead: Vec<&str> = dead.iter().map(|s| s.as_str()).collect();
        let e = tempfile::tempdir().unwrap();
        assert!(realms_doc_for(e.path(), &on, &dead, &fetch, &no_rtt).is_none(), "nobody answered: today's listings");
    }

    /// The Commonhold v1 conformance vectors (made by hosting/kit/tools/wunet/commonhold.py, checked there too).
    #[test]
    fn commonhold_vectors() {
        let v: Value = serde_json::from_str(include_str!("commonhold-v1-vectors.json")).unwrap();
        assert_eq!(v["format"], "commonhold-v1-vectors");
        let has = |doc: &Value, path: &str| {
            let mut d = doc;
            for p in path.split('.') {
                match d.get(p) {
                    Some(x) => d = x,
                    None => return false,
                }
            }
            true
        };
        let mut n = 0;
        for c in v["cases"].as_array().unwrap() {
            let (name, now) = (c["name"].as_str().unwrap(), c["now"].as_u64().unwrap());
            let case_staff: Vec<String> = c["staff"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            for (i, t) in c["check"].as_array().cloned().unwrap_or_default().iter().enumerate() {
                n += 1;
                let staff: Vec<String> = t["staff"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_else(|| case_staff.clone());
                let staff: Vec<&str> = staff.iter().map(String::as_str).collect();
                let exp = &t["expect"];
                let got = check(&t["record"], now, &staff);
                match exp["result"].as_str().unwrap() {
                    "ok" => {
                        let (id, ver, rec) = got.unwrap_or_else(|e| panic!("{name} [{i}]: refused: {e}"));
                        assert_eq!((id.as_str(), ver), (exp["id"].as_str().unwrap(), exp["version"].as_u64().unwrap()), "{name} [{i}]");
                        if let Some(k) = exp["endpoints"].as_u64() {
                            assert_eq!(rec["endpoints"].as_array().map_or(0, Vec::len) as u64, k, "{name} [{i}]: endpoints");
                        }
                        for p in exp["keeps"].as_array().cloned().unwrap_or_default() {
                            assert!(has(&rec, p.as_str().unwrap()), "{name} [{i}]: dropped {p}");
                        }
                    }
                    "ignored" => assert!(got.as_ref().is_err_and(|e| e.starts_with("ignored:")), "{name} [{i}]: {got:?}"),
                    _ => assert!(got.as_ref().is_err_and(|e| !e.starts_with("ignored:")), "{name} [{i}]: expected a refusal, got {got:?}"),
                }
            }
            if let Some(recs) = c["merge"].as_array() {
                n += 1;
                let staff: Vec<&str> = case_staff.iter().map(String::as_str).collect();
                let mut m = Merged::default();
                for r in recs {
                    m.offer(r, now, &staff);
                }
                for (id, sig) in c["final"].as_object().unwrap() {
                    assert_eq!(m.recs.get(id).map(|(_, r)| r["sig"].clone()).unwrap_or(Value::Null), *sig, "{name}: {id}");
                }
                for (id, k) in c["final_endpoints"].as_object().cloned().unwrap_or_default() {
                    assert_eq!(m.recs[&id].1["endpoints"].as_array().unwrap().len() as u64, k.as_u64().unwrap(), "{name}: {id}");
                }
            }
        }
        assert!(n >= 20, "every case ran ({n})");
    }

    #[test]
    fn info_records_and_the_tolerant_reader() {
        let (relay, realm) = (Key::new(21), Key::new(22));
        let now = 1_800_000_000u64;
        let doc = |seq: u64, extra: Value| {
            let mut d = json!({"v": 1, "pubkey": relay.pubhex(), "role": "relay", "seq": seq, "ts": now, "proto_min": 1, "proto_max": 2,
                "software": "wurelay", "version": "2026.11", "supported": ["mesh", "ws-bridge"], "limits": {"realms": 25},
                "admin": realm.rid(), "rules_url": "", "region": "eu", "languages": ["en"], "restricted": []});
            for (k, v) in extra.as_object().unwrap() {
                d[k] = v.clone();
            }
            json!({"kind": "info", "sig": relay.sign(CTX_INFO, &d), "info": d})
        };
        let mut m = Merged::default();
        assert!(m.offer(&doc(5, json!({"x_new": [1]})), now, &[]));
        let rid = format!("n-{}", relay.rid());
        assert_eq!(m.info_of(&rid).unwrap()["x_new"], json!([1]), "unknown fields are kept");
        assert!(!m.offer(&doc(4, json!({})), now, &[]), "older");
        for bad in [json!({"role": "server"}), json!({"proto_min": 3}), json!({"admin": "Bob"}), json!({"limits": {"realms": -1}}),
                    json!({"rules_url": "http://x.example.org"}), json!({"languages": ["english!"]}), json!({"supported": ["UPPER"]})] {
            assert!(check(&doc(9, bad.clone()), now, &[]).is_err(), "{bad}");
        }
        assert_eq!(check(&doc(9, json!({})), now + INFO_TTL + 1, &[]).unwrap_err(), "expired");
        let mut t = doc(9, json!({}));
        t["info"]["limits"]["realms"] = json!(999);
        assert_eq!(check(&t, now, &[]).unwrap_err(), "signature");
        // unknown kinds and newer versions: ignored, not refused
        let refused = m.refused;
        assert!(!m.offer(&json!({"kind": "probe", "probe": {"v": 1}}), now, &[]));
        let mut newer = realm.realm(1, now, "Next", "relay.test", vec![]);
        newer["beat"]["v"] = json!(2);
        assert!(!m.offer(&newer, now, &[]));
        assert_eq!((m.ignored, m.refused), (2, refused));
        // a realm whose record needs a newer protocol is kept but not shown
        let beat = json!({"v": 1, "realm_id": realm.rid(), "pubkey": realm.pubhex(), "seq": 3, "ts": now, "relay": "relay.test",
            "record": {"name": "Future", "proto_min": PROTO + 1, "proto_max": PROTO + 1}, "status": {"online": true}});
        assert!(m.offer(&json!({"kind": "realm", "sig": realm.sign(CTX_HEARTBEAT, &beat), "beat": beat, "endpoints": []}), now, &[]));
        assert_eq!(m.realms().count(), 1);
        assert_eq!(realms_doc(&m, &HashMap::new(), false, &[], now)["realms"].as_array().unwrap().len(), 0);
        assert_eq!(proto_range(&json!({})), (1, 1));
        assert_eq!(proto_range(&json!({"proto_min": 2, "proto_max": 1})), (1, 1));
        assert!(speaks(&json!({"proto_min": 1, "proto_max": 9})));
    }

    #[test]
    fn realm_ids_match_python() {
        // python3 -c 'from wunet import keys; print(keys.realm_id(keys.public_key(bytes(range(32)))))'
        let k = Ed25519KeyPair::from_seed_unchecked(&(0u8..32).collect::<Vec<u8>>()).unwrap();
        assert_eq!(realm_id(k.public_key().as_ref()), "kzdvvj2umnduyauf");
    }
}
