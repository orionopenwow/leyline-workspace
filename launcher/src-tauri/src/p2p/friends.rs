//! Friends on the Leyline network, with no server (docs/LEYLINE-FRIENDS.md; owner 12 Nov: "working friends and
//! persistent profiles on the Leyline network, even if it's local to local, recognized in the cache and signed by
//! each other instead of a Hetzner server").
//!
//! Every fact here is a small record signed by a player's own peer key (crate::identity), checked on this PC every
//! time it is loaded or received. Nothing is trusted because of where it came from.
//!
//! * **Profile card** (`leyline-profile-v1`): name, status, a few worlds, the relay where its owner reads its
//!   mailbox; seq + ts + exp (at most 30 days), at most 2 KB.
//! * **Friendship** (`leyline-friendship-v1`): `{a, b, ts}` (the two public keys, sorted) signed by BOTH. The asker
//!   signs it in the request, the other side countersigns on Accept. Either alone can't make one.
//! * **Revocation** (`leyline-friend-revoke-v1`): signed by either side, names the friendship; kept 60 days so an
//!   old friendship can't be replayed back in.
//! * **Messages** (`leyline-friend-msg-v1`): `{kind, from, to, ts, nonce, body}` signed by the sender, then sealed to
//!   the recipient (identity::seal) inside an envelope that shows only the recipient's peer id. Kinds: request,
//!   accept, decline, revoke, presence, invite, card. Good for 10 minutes; each nonce once (friendnet.rs).
//! * **LAN beacon** (`leyline-friend-beacon-v1`): `{pubkey, port, ts, name?}` signed, broadcast on the local
//!   network only.
//!
//! The cache is `<app>/cache/friends.json` ([`Book`]). Loading it re-checks every record and drops anything forged,
//! expired, oversized or over the caps.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::identity::{self, canonical, verify_doc, PeerIdentity};

pub const CTX_CARD: &str = "leyline-profile-v1";
pub const CTX_FRIENDSHIP: &str = "leyline-friendship-v1";
pub const CTX_REVOKE: &str = "leyline-friend-revoke-v1";
pub const CTX_MSG: &str = "leyline-friend-msg-v1";
pub const CTX_BEACON: &str = "leyline-friend-beacon-v1";

pub const CACHE_FILE: &str = "friends.json";
pub const SKEW: u64 = 300;
pub const CARD_TTL: u64 = 30 * 86400;
pub const CARD_MAX: usize = 2048;
pub const MSG_MAX: usize = 8 * 1024;
/// Under the relay's 16 KB body limit (hosting/relay/wurelay/httpd.py MAX_BODY).
pub const ENVELOPE_MAX: usize = 15 * 1024;
pub const MSG_TTL: u64 = 600;
pub const BEACON_MAX: usize = 600;
pub const BEACON_TTL: u64 = 60;
pub const MAX_FRIENDS: usize = 200;
pub const MAX_REQUESTS_IN: usize = 50;
pub const MAX_REQUESTS_OUT: usize = 50;
pub const MAX_TOKENS: usize = 8;
pub const MAX_REVOKED: usize = 500;
pub const REQUEST_TTL: u64 = 14 * 86400;
pub const REVOKE_KEEP: u64 = 60 * 86400;
/// A friend code's rendezvous token is good this long.
pub const CODE_TTL: u64 = 24 * 3600;
pub const CACHE_MAX: u64 = 2 * 1024 * 1024;
const KINDS: &[&str] = &["request", "accept", "decline", "revoke", "presence", "invite", "card"];
const WORLD_KINDS: &[&str] = &["single", "vanilla", "original", "den", "realm", "solo", "survivors", "other"];

fn hex64(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

/// Plain text a player typed: 1..=max characters (0 allowed when `empty`), no control characters.
fn text_ok(v: Option<&Value>, max: usize, empty: bool) -> bool {
    v.and_then(Value::as_str).is_some_and(|s| (empty || !s.trim().is_empty()) && s.chars().count() <= max && !s.chars().any(char::is_control))
}

fn clip(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

fn sha_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    crate::net::hex(&Sha256::digest(s.as_bytes()))
}

fn pid_of(pub_hex: &str) -> String {
    identity::peer_id(&identity::unhex(pub_hex).unwrap_or_default())
}

// ------------------------------------------------------------------------------------------------ profile cards

/// What the player edits (Friends › Your profile).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub status: String,
    /// Tell friends what you're playing.
    pub share_playing: bool,
    /// The local network: "off" (no beacon), "friends" (a beacon without your name: friends recognise you, others see
    /// a key), "everyone" (your name too, so people near you can add you).
    pub lan: String,
    /// Friends' messages through a Leyline relay's mailbox when you're not on the same network.
    pub mailbox: bool,
    /// The relay whose mailbox you read.
    pub relay: String,
    /// The worlds you show on your card.
    pub worlds: Vec<CardWorld>,
    /// The card's version: one more on every change.
    pub seq: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { name: String::new(), status: String::new(), share_playing: true, lan: "friends".into(), mailbox: true, relay: crate::realms::RELAY.into(), worlds: vec![], seq: 0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct CardWorld {
    pub kind: String,
    pub name: String,
    /// "friends": friends may ask to join; "ask": ask first. Never a link (invites are sent one by one, sealed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite_hint: Option<String>,
}

fn world_ok(w: &Value) -> bool {
    w.get("kind").and_then(Value::as_str).is_some_and(|k| WORLD_KINDS.contains(&k))
        && text_ok(w.get("name"), 40, false)
        && w.get("invite_hint").map_or(true, |h| matches!(h.as_str(), Some("friends") | Some("ask")))
        && w.as_object().is_some_and(|o| o.keys().all(|k| ["kind", "name", "invite_hint"].contains(&k.as_str())))
}

/// This player's card, signed.
pub fn make_card(me: &PeerIdentity, p: &Profile, now: u64) -> Value {
    let name = match clip(&p.name, 32) {
        n if n.is_empty() => format!("Hero {}", &me.peer_id[..4]),
        n => n,
    };
    let worlds: Vec<Value> = p
        .worlds
        .iter()
        .filter(|w| WORLD_KINDS.contains(&w.kind.as_str()) && !clip(&w.name, 40).is_empty())
        .take(4)
        .map(|w| {
            let mut o = json!({"kind": w.kind, "name": clip(&w.name, 40)});
            if let Some(h) = w.invite_hint.as_deref().filter(|h| *h == "friends" || *h == "ask") {
                o["invite_hint"] = json!(h);
            }
            o
        })
        .collect();
    let mut card = json!({"v": 1, "pubkey": me.public_hex(), "peer_id": me.peer_id, "name": name, "status": clip(&p.status, 80),
        "worlds": worlds, "seq": p.seq, "ts": now, "exp": now + CARD_TTL});
    if p.mailbox && crate::realms::relay_ok(&p.relay) {
        card["mailbox"] = json!(p.relay);
    }
    let sig = me.sign_doc(CTX_CARD, &card);
    json!({"card": card, "sig": sig})
}

/// A checked card: (peer id, public key hex, seq).
pub fn check_card(rec: &Value, now: u64) -> Result<(String, String, u64), String> {
    if canonical(rec).len() > CARD_MAX {
        return Err("card too big".into());
    }
    let c = rec.get("card").filter(|c| c.get("v").and_then(Value::as_u64) == Some(1)).ok_or("no card")?;
    let pk = hex64(c.get("pubkey")).ok_or("card key")?;
    let pid = pid_of(pk);
    if c.get("peer_id").and_then(Value::as_str) != Some(pid.as_str()) {
        return Err("card id does not match its key".into());
    }
    if !text_ok(c.get("name"), 32, false) || !text_ok(c.get("status"), 80, true) {
        return Err("card text".into());
    }
    if c.get("avatar").is_some_and(|a| hex64(Some(a)).is_none()) {
        return Err("card avatar".into());
    }
    if !c.get("worlds").and_then(Value::as_array).is_some_and(|w| w.len() <= 4 && w.iter().all(world_ok)) {
        return Err("card worlds".into());
    }
    if c.get("mailbox").is_some_and(|m| !m.as_str().is_some_and(crate::realms::relay_ok)) {
        return Err("card mailbox".into());
    }
    let seq = c.get("seq").and_then(Value::as_u64).ok_or("card seq")?;
    let ts = c.get("ts").and_then(Value::as_u64).ok_or("card ts")?;
    let exp = c.get("exp").and_then(Value::as_u64).ok_or("card exp")?;
    if ts > now + SKEW || exp <= now || exp < ts || exp - ts > CARD_TTL {
        return Err("card expired".into());
    }
    if !verify_doc(pk, c, CTX_CARD, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
        return Err("card signature".into());
    }
    Ok((pid, pk.to_string(), seq))
}

// ------------------------------------------------------------------------------------------------ friendships

/// The friendship document for two keys (sorted, so both sides make the same one).
pub fn friendship_doc(x: &str, y: &str, ts: u64) -> Value {
    let (a, b) = if x <= y { (x, y) } else { (y, x) };
    json!({"v": 1, "a": a, "b": b, "ts": ts})
}

/// A friendship's id: sha256 of its document.
pub fn friendship_id(doc: &Value) -> String {
    sha_hex(&canonical(doc))[..32].to_string()
}

/// A doubly signed friendship with `me`: the other side's public key (hex).
pub fn check_friendship(rec: &Value, me: &str, now: u64) -> Result<String, String> {
    let d = rec.get("doc").filter(|d| d.get("v").and_then(Value::as_u64) == Some(1)).ok_or("no friendship")?;
    let (a, b) = (hex64(d.get("a")).ok_or("a")?, hex64(d.get("b")).ok_or("b")?);
    if a >= b {
        return Err("keys not in order".into());
    }
    if !d.get("ts").and_then(Value::as_u64).is_some_and(|t| t <= now + SKEW) {
        return Err("friendship time".into());
    }
    if me != a && me != b {
        return Err("not my friendship".into());
    }
    let s = |k: &str| rec.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    if !verify_doc(a, d, CTX_FRIENDSHIP, &s("sig_a")) || !verify_doc(b, d, CTX_FRIENDSHIP, &s("sig_b")) {
        return Err("friendship signature".into());
    }
    Ok(if me == a { b } else { a }.to_string())
}

/// A revocation of friendship `fid`, by one of its two keys.
pub fn make_revoke(me: &PeerIdentity, fid: &str, now: u64) -> Value {
    let doc = json!({"v": 1, "friendship": fid, "by": me.public_hex(), "ts": now});
    let sig = me.sign_doc(CTX_REVOKE, &doc);
    json!({"doc": doc, "sig": sig})
}

/// Checked revocation: (friendship id, by).
pub fn check_revoke(rec: &Value) -> Result<(String, String), String> {
    let d = rec.get("doc").filter(|d| d.get("v").and_then(Value::as_u64) == Some(1)).ok_or("no revoke")?;
    let by = hex64(d.get("by")).ok_or("by")?;
    let fid = d.get("friendship").and_then(Value::as_str).filter(|f| f.len() == 32 && f.bytes().all(|b| b.is_ascii_hexdigit())).ok_or("friendship id")?;
    d.get("ts").and_then(Value::as_u64).ok_or("ts")?;
    if !verify_doc(by, d, CTX_REVOKE, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
        return Err("revoke signature".into());
    }
    Ok((fid.to_string(), by.to_string()))
}

// ------------------------------------------------------------------------------------------------ messages

/// A signed message from `me` to the key `to` (hex).
pub fn make_msg(me: &PeerIdentity, to: &str, kind: &str, body: Value, now: u64) -> Result<Value, String> {
    let msg = json!({"v": 1, "kind": kind, "from": me.public_hex(), "to": to, "ts": now, "nonce": identity::random_b32(12)?, "body": body});
    let sig = me.sign_doc(CTX_MSG, &msg);
    let rec = json!({"msg": msg, "sig": sig});
    if canonical(&rec).len() > MSG_MAX {
        return Err("That message is too big.".into());
    }
    Ok(rec)
}

/// The signature and shape of a message (no freshness: stored ones are re-checked this way). (from, kind)
pub fn check_msg_sig(rec: &Value, me: &str) -> Result<(String, String), String> {
    if canonical(rec).len() > MSG_MAX {
        return Err("message too big".into());
    }
    let m = rec.get("msg").filter(|m| m.get("v").and_then(Value::as_u64) == Some(1)).ok_or("no message")?;
    let from = hex64(m.get("from")).ok_or("from")?;
    if m.get("to").and_then(Value::as_str) != Some(me) {
        return Err("not for me".into());
    }
    let kind = m.get("kind").and_then(Value::as_str).filter(|k| KINDS.contains(k)).ok_or("kind")?;
    m.get("ts").and_then(Value::as_u64).ok_or("ts")?;
    m.get("nonce").and_then(Value::as_str).filter(|n| (8..=32).contains(&n.len())).ok_or("nonce")?;
    if !m.get("body").is_some_and(Value::is_object) {
        return Err("body".into());
    }
    if !verify_doc(from, m, CTX_MSG, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
        return Err("message signature".into());
    }
    Ok((from.to_string(), kind.to_string()))
}

/// A message just received: signed, for me, fresh.
pub fn check_msg(rec: &Value, me: &str, now: u64) -> Result<(String, String), String> {
    let r = check_msg_sig(rec, me)?;
    let ts = rec["msg"]["ts"].as_u64().unwrap_or(0);
    if ts > now + SKEW || now.saturating_sub(ts) > MSG_TTL {
        return Err("message too old".into());
    }
    Ok(r)
}

/// An envelope on the wire: the recipient's peer id, the sealed message (base32), and for the relay's mailbox an
/// optional opaque tag (one kept per tag: the newest presence replaces the last).
pub fn envelope(to_pub: &[u8; 32], msg: &Value, tag: Option<&str>) -> Result<Value, String> {
    let sealed = identity::seal(to_pub, canonical(msg).as_bytes())?;
    let mut e = json!({"v": 1, "to": identity::peer_id(to_pub), "box": identity::b32(&sealed)});
    if let Some(t) = tag {
        e["tag"] = json!(t);
    }
    if canonical(&e).len() > ENVELOPE_MAX {
        return Err("That message is too big.".into());
    }
    Ok(e)
}

/// The mailbox tag for a kind of message from `from` to `to`: opaque to the relay, the same every time.
/// The mailbox tag that lets a newer presence or card replace the older one (hosting/relay/wurelay/mailbox.py). Made
/// from the sender's own deterministic signature, so only the sender can make it: anyone who knows two friend codes
/// could compute a tag made from public keys alone, and post junk under it to wipe the real box. The recipient never
/// needs to make a tag; only the relay compares them.
pub fn tag_for(me: &PeerIdentity, to: &str, kind: &str) -> String {
    let sig = me.sign(format!("leyline-friends-tag-v2\n{to}\n{kind}").as_bytes());
    sha_hex(&crate::identity::hex(&sig))[..32].to_string()
}

/// Open an envelope for `me`: the signed message inside (checked by the caller with [`check_msg`]).
pub fn open_envelope(me: &PeerIdentity, e: &Value) -> Result<Value, String> {
    if canonical(e).len() > ENVELOPE_MAX {
        return Err("envelope too big".into());
    }
    if e.get("to").and_then(Value::as_str) != Some(me.peer_id.as_str()) {
        return Err("not for me".into());
    }
    let b = e.get("box").and_then(Value::as_str).and_then(identity::unb32).ok_or("box")?;
    let plain = me.open(&b)?;
    serde_json::from_slice(&plain).map_err(|_| "box contents".to_string())
}

// ------------------------------------------------------------------------------------------------ LAN beacons

pub fn make_beacon(me: &PeerIdentity, port: u16, name: Option<&str>, now: u64) -> Value {
    let mut b = json!({"v": 1, "pubkey": me.public_hex(), "port": port, "ts": now});
    if let Some(n) = name.map(|n| clip(n, 32)).filter(|n| !n.is_empty()) {
        b["name"] = json!(n);
    }
    let sig = me.sign_doc(CTX_BEACON, &b);
    json!({"b": b, "sig": sig})
}

/// A beacon heard on the local network: (public key hex, port, name).
pub fn check_beacon(bytes: &[u8], now: u64) -> Result<(String, u16, Option<String>), String> {
    if bytes.len() > BEACON_MAX {
        return Err("beacon too big".into());
    }
    let rec: Value = serde_json::from_slice(bytes).map_err(|_| "not a beacon")?;
    let b = rec.get("b").filter(|b| b.get("v").and_then(Value::as_u64) == Some(1)).ok_or("not a beacon")?;
    let pk = hex64(b.get("pubkey")).ok_or("beacon key")?;
    let port = b.get("port").and_then(Value::as_u64).filter(|p| (1..=65535).contains(p)).ok_or("beacon port")? as u16;
    let ts = b.get("ts").and_then(Value::as_u64).ok_or("beacon ts")?;
    if ts > now + 30 || now.saturating_sub(ts) > BEACON_TTL {
        return Err("beacon old".into());
    }
    if b.get("name").is_some() && !text_ok(b.get("name"), 32, false) {
        return Err("beacon name".into());
    }
    if !verify_doc(pk, b, CTX_BEACON, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
        return Err("beacon signature".into());
    }
    Ok((pk.to_string(), port, b.get("name").and_then(Value::as_str).map(String::from)))
}

// ------------------------------------------------------------------------------------------------ friend codes

/// A friend code: `LF1-<base32(public key + 5-byte token)>` and, after `@`, the relay whose mailbox its owner reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Code {
    pub pubkey: [u8; 32],
    pub token: String,
    pub relay: Option<String>,
}

pub fn format_code(pubkey: &[u8; 32], token: &[u8; 5], relay: Option<&str>) -> String {
    let mut b = pubkey.to_vec();
    b.extend_from_slice(token);
    let mut s = format!("LF1-{}", identity::b32(&b));
    if let Some(r) = relay.filter(|r| crate::realms::relay_ok(r)) {
        s.push('@');
        s.push_str(r);
    }
    s
}

pub fn parse_code(text: &str) -> Result<Code, String> {
    let t = text.trim();
    let bad = || "That isn't a Leyline friend code (it starts with LF1-).".to_string();
    let rest = t.strip_prefix("LF1-").or_else(|| t.strip_prefix("lf1-")).ok_or_else(bad)?;
    let (body, relay) = match rest.split_once('@') {
        Some((b, r)) => (b, Some(r.to_ascii_lowercase())),
        None => (rest, None),
    };
    if relay.as_deref().is_some_and(|r| !crate::realms::relay_ok(r)) {
        return Err(bad());
    }
    let raw = identity::unb32(body).filter(|v| v.len() == 37).ok_or_else(bad)?;
    let pubkey: [u8; 32] = raw[..32].try_into().map_err(|_| bad())?;
    identity::ed_to_x_public(&pubkey).ok_or_else(bad)?;
    Ok(Code { pubkey, token: identity::b32(&raw[32..]), relay })
}

// ------------------------------------------------------------------------------------------------ the book

/// How a message came in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// From this PC's own local network (a private, link-local or loopback address).
    Lan,
    /// Straight from the internet to this PC.
    Direct,
    /// From a relay's mailbox.
    Mailbox,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Friend {
    /// The doubly signed friendship.
    pub friendship: Value,
    /// Their latest card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<Value>,
    /// Their latest presence message (signed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence: Option<Value>,
    /// Their latest world invite to me (signed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite: Option<Value>,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Outgoing {
    pub to: String,
    pub doc: Value,
    pub sig: String,
    pub ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
    /// The friend code's token (sent again if the request is re-sent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Token {
    pub token: String,
    pub exp: u64,
}

/// `<app>/cache/friends.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Book {
    pub v: u32,
    pub profile: Profile,
    pub friends: Vec<Friend>,
    /// Requests to me: the signed request messages.
    pub incoming: Vec<Value>,
    pub outgoing: Vec<Outgoing>,
    pub revoked: Vec<Value>,
    pub tokens: Vec<Token>,
}

/// What a received message did (for the UI's notice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Request(String),
    Accepted(String),
    Declined(String),
    Revoked(String),
    Presence(String),
    Invite(String),
    Card(String),
    Ignored(&'static str),
}

pub fn cache_path(app: &Path) -> PathBuf {
    app.join("cache").join(CACHE_FILE)
}

impl Book {
    /// Load and re-check everything; returns the book and how many records were dropped.
    pub fn load(app: &Path, me: &PeerIdentity, now: u64) -> (Book, usize) {
        let path = cache_path(app);
        let raw = match std::fs::metadata(&path) {
            Ok(m) if m.len() <= CACHE_MAX => std::fs::read(&path).ok(),
            _ => None,
        };
        let book: Book = raw.and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        book.checked(me, now)
    }

    /// The same book with only what checks out.
    pub fn checked(mut self, me: &PeerIdentity, now: u64) -> (Book, usize) {
        let mine = me.public_hex();
        let mut dropped = 0usize;
        self.v = 1;
        // revocations: signed, recent, at most MAX_REVOKED
        let n = self.revoked.len();
        self.revoked.retain(|r| check_revoke(r).is_ok() && r["doc"]["ts"].as_u64().is_some_and(|t| now.saturating_sub(t) <= REVOKE_KEEP));
        self.revoked.truncate(MAX_REVOKED);
        dropped += n - self.revoked.len();
        let gone: Vec<String> = self.revoked.iter().filter_map(|r| check_revoke(r).ok().map(|(f, _)| f)).collect();
        // friends: a good doubly signed friendship with me, not revoked, one per key, at most MAX_FRIENDS
        let mut seen = std::collections::HashSet::new();
        let mut keep = Vec::new();
        for mut f in std::mem::take(&mut self.friends) {
            let Ok(other) = check_friendship(&f.friendship, &mine, now) else {
                dropped += 1;
                continue;
            };
            if gone.contains(&friendship_id(&f.friendship["doc"])) || !seen.insert(other.clone()) || keep.len() >= MAX_FRIENDS {
                dropped += 1;
                continue;
            }
            let pid = pid_of(&other);
            if f.card.as_ref().is_some_and(|c| check_card(c, now).map(|(p, _, _)| p != pid).unwrap_or(true)) {
                f.card = None;
                dropped += 1;
            }
            for slot in [&mut f.presence, &mut f.invite] {
                if slot.as_ref().is_some_and(|m| check_msg_sig(m, &mine).map(|(from, _)| from != other).unwrap_or(true)) {
                    *slot = None;
                    dropped += 1;
                }
            }
            keep.push(f);
        }
        self.friends = keep;
        // requests to me: signed, recent, not from a friend, one per key
        let n = self.incoming.len();
        let mut from_seen = std::collections::HashSet::new();
        let friends: Vec<String> = self.friend_keys(&mine);
        self.incoming.retain(|m| {
            check_msg_sig(m, &mine).is_ok_and(|(from, kind)| kind == "request" && !friends.contains(&from) && from_seen.insert(from))
                && m["msg"]["ts"].as_u64().is_some_and(|t| now.saturating_sub(t) <= REQUEST_TTL)
                && request_parts(m, &mine, now).is_ok()
        });
        self.incoming.truncate(MAX_REQUESTS_IN);
        dropped += n - self.incoming.len();
        let n = self.outgoing.len();
        self.outgoing.retain(|o| o.to.len() == 64 && o.to.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) && now.saturating_sub(o.ts) <= REQUEST_TTL && verify_doc(&mine, &o.doc, CTX_FRIENDSHIP, &o.sig) && !friends.contains(&o.to));
        self.outgoing.truncate(MAX_REQUESTS_OUT);
        dropped += n - self.outgoing.len();
        self.tokens.retain(|t| t.exp > now && t.token.len() == 8);
        self.tokens.truncate(MAX_TOKENS);
        if !["off", "friends", "everyone"].contains(&self.profile.lan.as_str()) {
            self.profile.lan = "friends".into();
        }
        (self, dropped)
    }

    pub fn save(&self, app: &Path) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if body.len() as u64 > CACHE_MAX {
            return Err("The friends list is too big to save.".into());
        }
        crate::settings::write_atomic(&cache_path(app), &body).map_err(|e| e.to_string())
    }

    fn friend_keys(&self, mine: &str) -> Vec<String> {
        self.friends.iter().filter_map(|f| check_friendship(&f.friendship, mine, u64::MAX - SKEW).ok()).collect()
    }

    /// The other side's key of a friend entry (the friendship was checked on load).
    fn other(f: &Friend, mine: &str) -> String {
        let (a, b) = (f.friendship["doc"]["a"].as_str().unwrap_or(""), f.friendship["doc"]["b"].as_str().unwrap_or(""));
        if a == mine { b } else { a }.to_string()
    }

    fn find(&self, mine: &str, pid: &str) -> Option<usize> {
        self.friends.iter().position(|f| pid_of(&Self::other(f, mine)) == pid)
    }

    /// Whether `peer_id` is a friend (lasting world logins: host/…, the social-ui side).
    pub fn is_friend(&self, me: &PeerIdentity, peer_id: &str) -> bool {
        self.find(&me.public_hex(), peer_id).is_some()
    }

    /// A friend's public key (hex), by peer id.
    pub fn friend_key(&self, me: &PeerIdentity, peer_id: &str) -> Option<String> {
        let mine = me.public_hex();
        self.find(&mine, peer_id).map(|i| Self::other(&self.friends[i], &mine))
    }

    /// Every friend's public key (hex).
    pub fn keys(&self, me: &PeerIdentity) -> Vec<String> {
        let mine = me.public_hex();
        self.friends.iter().map(|f| Self::other(f, &mine)).collect()
    }

    /// A friend's mailbox relay (from their card), if they read one.
    pub fn mailbox_of(&self, me: &PeerIdentity, peer_id: &str) -> Option<String> {
        let mine = me.public_hex();
        let f = &self.friends[self.find(&mine, peer_id)?];
        f.card.as_ref().and_then(|c| c["card"]["mailbox"].as_str()).map(String::from)
    }

    /// A new friend code (its token good for [`CODE_TTL`]).
    pub fn new_code(&mut self, me: &PeerIdentity, now: u64) -> Result<String, String> {
        let raw: [u8; 5] = identity::unb32(&identity::random_b32(5)?).and_then(|v| v.try_into().ok()).ok_or("random")?;
        self.tokens.retain(|t| t.exp > now);
        if self.tokens.len() >= MAX_TOKENS {
            self.tokens.remove(0);
        }
        self.tokens.push(Token { token: identity::b32(&raw), exp: now + CODE_TTL });
        let relay = Some(self.profile.relay.as_str()).filter(|_| self.profile.mailbox);
        Ok(format_code(&me.public_key, &raw, relay))
    }

    /// Ask `to` (their public key) to be friends: the request message, my half of the friendship in it.
    pub fn request(&mut self, me: &PeerIdentity, to: &[u8; 32], token: Option<&str>, relay: Option<&str>, now: u64) -> Result<Value, String> {
        let (mine, theirs) = (me.public_hex(), identity::hex(to));
        if mine == theirs {
            return Err("That's your own code.".into());
        }
        if self.find(&mine, &identity::peer_id(to)).is_some() {
            return Err("You're already friends.".into());
        }
        if self.friends.len() >= MAX_FRIENDS {
            return Err(format!("You have {MAX_FRIENDS} friends, the most Leyline keeps. Remove one first."));
        }
        self.outgoing.retain(|o| o.to != theirs);
        if self.outgoing.len() >= MAX_REQUESTS_OUT {
            self.outgoing.remove(0);
        }
        let doc = friendship_doc(&mine, &theirs, now);
        let sig = me.sign_doc(CTX_FRIENDSHIP, &doc);
        let card = make_card(me, &self.profile, now);
        let mut body = json!({"friendship": doc, "sig": sig, "card": card});
        if let Some(t) = token {
            body["token"] = json!(t);
        }
        self.outgoing.push(Outgoing { to: theirs.clone(), doc, sig, ts: now, relay: relay.map(String::from), token: token.map(String::from) });
        make_msg(me, &theirs, "request", body, now)
    }

    /// The request message again (re-sent while it waits), if one is waiting for `to`.
    pub fn request_again(&self, me: &PeerIdentity, to: &str, now: u64) -> Option<Value> {
        let o = self.outgoing.iter().find(|o| o.to == to)?;
        let mut body = json!({"friendship": o.doc, "sig": o.sig, "card": make_card(me, &self.profile, now)});
        if let Some(t) = &o.token {
            body["token"] = json!(t);
        }
        make_msg(me, to, "request", body, now).ok()
    }

    /// Accept a waiting request: the friendship countersigned and kept; the accept message for the asker.
    pub fn accept(&mut self, me: &PeerIdentity, peer_id: &str, now: u64) -> Result<(String, Value), String> {
        let mine = me.public_hex();
        let i = self.incoming.iter().position(|m| m["msg"]["from"].as_str().map(pid_of).as_deref() == Some(peer_id)).ok_or("That request isn't here any more.")?;
        let m = self.incoming.remove(i);
        let (from, doc, sig_from, card) = request_parts(&m, &mine, now)?;
        let mut rec = json!({"doc": doc});
        let my_sig = me.sign_doc(CTX_FRIENDSHIP, &doc);
        if doc["a"].as_str() == Some(mine.as_str()) {
            rec["sig_a"] = json!(my_sig);
            rec["sig_b"] = json!(sig_from);
        } else {
            rec["sig_a"] = json!(sig_from);
            rec["sig_b"] = json!(my_sig);
        }
        check_friendship(&rec, &mine, now)?;
        if self.friends.len() >= MAX_FRIENDS {
            return Err(format!("You have {MAX_FRIENDS} friends, the most Leyline keeps. Remove one first."));
        }
        self.friends.push(Friend { friendship: rec.clone(), card, presence: None, invite: None, last_seen: m["msg"]["ts"].as_u64().unwrap_or(now) });
        self.outgoing.retain(|o| o.to != from);
        let body = json!({"friendship": rec, "card": make_card(me, &self.profile, now)});
        Ok((from.clone(), make_msg(me, &from, "accept", body, now)?))
    }

    /// Decline a waiting request: dropped here; the decline message for the asker (sending it is optional).
    pub fn decline(&mut self, me: &PeerIdentity, peer_id: &str, now: u64) -> Result<(String, Value), String> {
        let i = self.incoming.iter().position(|m| m["msg"]["from"].as_str().map(pid_of).as_deref() == Some(peer_id)).ok_or("That request isn't here any more.")?;
        let m = self.incoming.remove(i);
        let from = m["msg"]["from"].as_str().unwrap_or("").to_string();
        let fid = friendship_id(&m["msg"]["body"]["friendship"]);
        Ok((from.clone(), make_msg(me, &from, "decline", json!({"friendship": fid}), now)?))
    }

    /// Cancel a request I sent.
    pub fn cancel(&mut self, peer_id: &str) {
        self.outgoing.retain(|o| pid_of(&o.to) != peer_id);
    }

    /// Stop being friends: the revocation kept (so the friendship can't come back) and sent.
    pub fn revoke(&mut self, me: &PeerIdentity, peer_id: &str, now: u64) -> Result<(String, Value), String> {
        let mine = me.public_hex();
        let i = self.find(&mine, peer_id).ok_or("That friend isn't on your list.")?;
        let f = self.friends.remove(i);
        let other = Self::other(&f, &mine);
        let r = make_revoke(me, &friendship_id(&f.friendship["doc"]), now);
        self.revoked.insert(0, r.clone());
        self.revoked.truncate(MAX_REVOKED);
        Ok((other.clone(), make_msg(me, &other, "revoke", json!({"revoke": r}), now)?))
    }

    /// Whether a token from a friend code is one of mine and still good.
    fn token_ok(&self, t: &str, now: u64) -> bool {
        self.tokens.iter().any(|x| x.token == t && x.exp > now)
    }

    /// A message received and checked (identity, freshness and the nonce by the caller): what it changes here.
    pub fn receive(&mut self, me: &PeerIdentity, rec: &Value, via: Via, now: u64) -> Result<Event, String> {
        let mine = me.public_hex();
        let (from, kind) = check_msg(rec, &mine, now)?;
        let from_pid = pid_of(&from);
        let body = &rec["msg"]["body"];
        let friend = self.find(&mine, &from_pid);
        match (kind.as_str(), friend) {
            ("request", Some(_)) => Ok(Event::Ignored("already friends")),
            ("request", None) => {
                let (_, doc, _, _) = request_parts(rec, &mine, now)?;
                let fid = friendship_id(&doc);
                if self.revoked.iter().any(|r| check_revoke(r).is_ok_and(|(f, _)| f == fid)) {
                    return Ok(Event::Ignored("revoked"));
                }
                // who may ask: someone with my friend code (its token), or someone on this local network when I'm
                // visible there. From the internet or a mailbox without a token: refused (anyone can learn a key).
                let token = body.get("token").and_then(Value::as_str);
                let allowed = token.is_some_and(|t| self.token_ok(t, now)) || (via == Via::Lan && self.profile.lan != "off");
                if !allowed {
                    return Ok(Event::Ignored("no valid friend code"));
                }
                self.incoming.retain(|m| m["msg"]["from"].as_str() != Some(from.as_str()));
                if self.incoming.len() >= MAX_REQUESTS_IN {
                    return Ok(Event::Ignored("too many requests waiting"));
                }
                self.incoming.insert(0, rec.clone());
                Ok(Event::Request(from_pid))
            }
            ("accept", _) => {
                let f = body.get("friendship").ok_or("no friendship")?;
                let other = check_friendship(f, &mine, now)?;
                if other != from {
                    return Err("friendship of someone else".into());
                }
                // only a friendship I asked for (my request still waiting, the same document)
                let Some(o) = self.outgoing.iter().position(|o| o.to == from && o.doc == f["doc"]) else {
                    return Ok(Event::Ignored("no request of mine"));
                };
                self.outgoing.remove(o);
                if friend.is_some() {
                    return Ok(Event::Ignored("already friends"));
                }
                if self.friends.len() >= MAX_FRIENDS {
                    return Ok(Event::Ignored("too many friends"));
                }
                let card = body.get("card").filter(|c| check_card(c, now).is_ok_and(|(p, _, _)| p == from_pid)).cloned();
                self.incoming.retain(|m| m["msg"]["from"].as_str() != Some(from.as_str()));
                self.friends.push(Friend { friendship: f.clone(), card, presence: None, invite: None, last_seen: now });
                Ok(Event::Accepted(from_pid))
            }
            ("decline", _) => {
                let before = self.outgoing.len();
                self.outgoing.retain(|o| o.to != from);
                Ok(if before != self.outgoing.len() { Event::Declined(from_pid) } else { Event::Ignored("no request of mine") })
            }
            ("revoke", Some(i)) => {
                let r = body.get("revoke").ok_or("no revoke")?;
                let (fid, by) = check_revoke(r)?;
                if by != from || fid != friendship_id(&self.friends[i].friendship["doc"]) {
                    return Err("revoke of another friendship".into());
                }
                self.friends.remove(i);
                self.revoked.insert(0, r.clone());
                self.revoked.truncate(MAX_REVOKED);
                Ok(Event::Revoked(from_pid))
            }
            ("presence", Some(i)) => {
                if !presence_ok(body) {
                    return Err("presence".into());
                }
                let f = &mut self.friends[i];
                if f.presence.as_ref().and_then(|p| p["msg"]["ts"].as_u64()).is_some_and(|t| t > rec["msg"]["ts"].as_u64().unwrap_or(0)) {
                    return Ok(Event::Ignored("older presence"));
                }
                f.last_seen = f.last_seen.max(rec["msg"]["ts"].as_u64().unwrap_or(0));
                f.presence = Some(rec.clone());
                if let Some(c) = body.get("card") {
                    Self::take_card(f, c, &from_pid, now);
                }
                Ok(Event::Presence(from_pid))
            }
            ("invite", Some(i)) => {
                if !invite_ok(body) {
                    return Err("invite".into());
                }
                self.friends[i].invite = Some(rec.clone());
                self.friends[i].last_seen = self.friends[i].last_seen.max(rec["msg"]["ts"].as_u64().unwrap_or(0));
                Ok(Event::Invite(from_pid))
            }
            ("card", Some(i)) => {
                let c = body.get("card").ok_or("no card")?;
                Self::take_card(&mut self.friends[i], c, &from_pid, now);
                Ok(Event::Card(from_pid))
            }
            _ => Ok(Event::Ignored("not a friend")),
        }
    }

    fn take_card(f: &mut Friend, c: &Value, pid: &str, now: u64) {
        let Ok((p, _, seq)) = check_card(c, now) else { return };
        let old = f.card.as_ref().and_then(|o| o["card"]["seq"].as_u64());
        if p == pid && old.map_or(true, |o| seq > o || (seq == o && c["card"]["ts"].as_u64() > f.card.as_ref().and_then(|o| o["card"]["ts"].as_u64()))) {
            f.card = Some(c.clone());
        }
    }

    /// The list as the Friends page shows it: friends (online first), requests both ways, my card and code state.
    pub fn view(&self, me: &PeerIdentity, now: u64, near: &[(String, Option<String>)], online_window: u64) -> Value {
        let mine = me.public_hex();
        let mut friends: Vec<Value> = self
            .friends
            .iter()
            .map(|f| {
                let other = Self::other(f, &mine);
                let pid = pid_of(&other);
                let card = f.card.as_ref().map(|c| &c["card"]);
                let p = f.presence.as_ref().map(|p| (&p["msg"]["body"], p["msg"]["ts"].as_u64().unwrap_or(0)));
                let online = p.is_some_and(|(b, ts)| b["online"].as_bool() == Some(true) && now.saturating_sub(ts) <= online_window)
                    || near.iter().any(|(k, _)| *k == other);
                let invite = f.invite.as_ref().filter(|m| now.saturating_sub(m["msg"]["ts"].as_u64().unwrap_or(0)) <= 24 * 3600).map(|m| {
                    json!({"world": m["msg"]["body"]["world"], "ts": m["msg"]["ts"]})
                });
                json!({
                    "peer_id": pid,
                    "name": card.and_then(|c| c["name"].as_str()).unwrap_or(""),
                    "status": card.and_then(|c| c["status"].as_str()).unwrap_or(""),
                    "worlds": card.map(|c| c["worlds"].clone()).unwrap_or(json!([])),
                    "online": online,
                    "near": near.iter().any(|(k, _)| *k == other),
                    "last_seen": f.last_seen,
                    "playing": p.filter(|_| online).and_then(|(b, _)| b["playing"].as_str()),
                    "open": p.filter(|_| online).map(|(b, _)| b["open"].clone()).filter(|o| o.is_object()),
                    "invite": invite,
                    "since": f.friendship["doc"]["ts"],
                })
            })
            .collect();
        friends.sort_by(|a, b| {
            (b["online"].as_bool(), b["last_seen"].as_u64()).cmp(&(a["online"].as_bool(), a["last_seen"].as_u64()))
                .then_with(|| a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase()))
        });
        let incoming: Vec<Value> = self
            .incoming
            .iter()
            .map(|m| {
                let c = &m["msg"]["body"]["card"]["card"];
                json!({"peer_id": pid_of(m["msg"]["from"].as_str().unwrap_or("")), "name": c["name"], "status": c["status"],
                    "ts": m["msg"]["ts"], "via": if m["msg"]["body"].get("token").is_some() { "code" } else { "near" }})
            })
            .collect();
        let outgoing: Vec<Value> = self.outgoing.iter().map(|o| json!({"peer_id": pid_of(&o.to), "ts": o.ts})).collect();
        let friend_keys: Vec<String> = self.keys(me);
        let near_people: Vec<Value> = near
            .iter()
            .filter(|(k, n)| n.is_some() && !friend_keys.contains(k) && *k != mine)
            .map(|(k, n)| json!({"peer_id": pid_of(k), "name": n, "asked": self.outgoing.iter().any(|o| o.to == *k)}))
            .collect();
        json!({"me": {"peer_id": me.peer_id, "profile": self.profile}, "friends": friends, "incoming": incoming, "outgoing": outgoing, "near": near_people})
    }
}

/// A request's parts: (from, the friendship document, the asker's signature on it, the asker's card if good).
fn request_parts(m: &Value, mine: &str, now: u64) -> Result<(String, Value, String, Option<Value>), String> {
    let from = m["msg"]["from"].as_str().ok_or("from")?.to_string();
    let b = &m["msg"]["body"];
    let doc = b.get("friendship").cloned().ok_or("no friendship")?;
    let sig = b.get("sig").and_then(Value::as_str).ok_or("no signature")?.to_string();
    if doc != friendship_doc(&from, mine, doc["ts"].as_u64().unwrap_or(0)) || doc["ts"].as_u64().is_none() {
        return Err("friendship of someone else".into());
    }
    if !verify_doc(&from, &doc, CTX_FRIENDSHIP, &sig) {
        return Err("friendship signature".into());
    }
    if b.get("token").is_some_and(|t| !t.as_str().is_some_and(|t| t.len() == 8)) {
        return Err("token".into());
    }
    let pid = pid_of(&from);
    let card = b.get("card").filter(|c| check_card(c, now).is_ok_and(|(p, _, _)| p == pid)).cloned();
    if card.is_none() {
        return Err("a request carries its card".into());
    }
    Ok((from, doc, sig, card))
}

/// A presence body: online, and (only when the sender shares them) what they play, an open world, and a direct
/// address of theirs.
pub fn presence_ok(b: &Value) -> bool {
    b.get("online").is_some_and(Value::is_boolean)
        && b.get("playing").map_or(true, |p| text_ok(Some(p), 40, false))
        && b.get("open").map_or(true, world_ok)
        && b.get("addr").map_or(true, |a| a.as_str().is_some_and(|a| a.len() <= 64 && a.parse::<std::net::SocketAddr>().is_ok()))
        && b.get("card").map_or(true, Value::is_object)
}

/// An invite body: the world and a relay invite link (`https://<relay>/join/<id>#...`).
pub fn invite_ok(b: &Value) -> bool {
    b.get("world").is_some_and(world_ok) && b.get("link").and_then(Value::as_str).is_some_and(invite_link_ok)
}

pub fn invite_link_ok(l: &str) -> bool {
    if l.len() > 600 || l.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let Some(rest) = l.strip_prefix("https://") else { return false };
    let Some((host, path)) = rest.split_once('/') else { return false };
    let Some(p) = path.strip_prefix("join/") else { return false };
    crate::realms::relay_ok(host) && p.len() > 17 && crate::realms::realm_id_ok(&p[..16]) && p.as_bytes()[16] == b'#'
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn id(n: u8) -> PeerIdentity {
        PeerIdentity::from_seed([n; 32]).unwrap()
    }

    fn named(n: &str) -> Profile {
        Profile { name: n.into(), ..Default::default() }
    }

    #[test]
    fn card_sign_verify_tamper() {
        let a = id(1);
        let mut p = named("Ari");
        p.status = "around tonight".into();
        p.worlds = vec![CardWorld { kind: "single".into(), name: "My world".into(), invite_hint: Some("friends".into()) }];
        let c = make_card(&a, &p, NOW);
        let (pid, pk, seq) = check_card(&c, NOW).unwrap();
        assert_eq!((pid.as_str(), pk.as_str(), seq), (a.peer_id.as_str(), a.public_hex().as_str(), 0));
        let mut t = c.clone();
        t["card"]["name"] = json!("Mallory");
        assert!(check_card(&t, NOW).is_err(), "changed name");
        let mut t = c.clone();
        t["card"]["peer_id"] = json!(id(2).peer_id);
        assert!(check_card(&t, NOW).is_err(), "someone else's id");
        assert!(check_card(&c, NOW + CARD_TTL + 1).is_err(), "expired");
        // a forged card for a's key signed by b
        let b = id(2);
        let mut forged = c.clone();
        forged["sig"] = json!(b.sign_doc(CTX_CARD, &c["card"]));
        assert!(check_card(&forged, NOW).is_err());
        // oversized
        let mut big = c.clone();
        big["card"]["junk"] = json!("x".repeat(CARD_MAX));
        assert!(check_card(&big, NOW).unwrap_err().contains("big"));
        // an empty name gets a default; control characters never reach a card
        let c2 = make_card(&a, &named("\u{7}"), NOW);
        assert!(check_card(&c2, NOW).is_ok());
        assert!(c2["card"]["name"].as_str().unwrap().starts_with("Hero "));
    }

    fn befriend(a: &PeerIdentity, ba: &mut Book, b: &PeerIdentity, bb: &mut Book) {
        let code = bb.new_code(b, NOW).unwrap();
        let c = parse_code(&code).unwrap();
        assert_eq!(c.pubkey, b.public_key);
        let req = ba.request(a, &c.pubkey, Some(&c.token), c.relay.as_deref(), NOW).unwrap();
        assert_eq!(bb.receive(b, &req, Via::Mailbox, NOW + 1).unwrap(), Event::Request(a.peer_id.clone()));
        let (to, acc) = bb.accept(b, &a.peer_id, NOW + 2).unwrap();
        assert_eq!(to, a.public_hex());
        assert_eq!(ba.receive(a, &acc, Via::Mailbox, NOW + 3).unwrap(), Event::Accepted(b.peer_id.clone()));
    }

    #[test]
    fn friendship_is_mutual_and_survives_reload() {
        let (a, b) = (id(1), id(2));
        let (mut ba, mut bb) = (Book { profile: named("Ari"), ..Default::default() }, Book { profile: named("Bo"), ..Default::default() });
        befriend(&a, &mut ba, &b, &mut bb);
        assert!(ba.is_friend(&a, &b.peer_id) && bb.is_friend(&b, &a.peer_id));
        assert!(ba.outgoing.is_empty() && bb.incoming.is_empty());
        // both hold the same doubly signed record
        assert_eq!(ba.friends[0].friendship, bb.friends[0].friendship);
        assert_eq!(check_friendship(&ba.friends[0].friendship, &a.public_hex(), NOW).unwrap(), b.public_hex());
        // each has the other's card
        assert_eq!(ba.friends[0].card.as_ref().unwrap()["card"]["name"], "Bo");
        assert_eq!(bb.friends[0].card.as_ref().unwrap()["card"]["name"], "Ari");
        // saved and loaded: everything still checks
        let d = tempfile::tempdir().unwrap();
        ba.save(d.path()).unwrap();
        let (again, dropped) = Book::load(d.path(), &a, NOW + 10);
        assert_eq!(dropped, 0);
        assert!(again.is_friend(&a, &b.peer_id));
        // the view: friend shown, by name
        let v = again.view(&a, NOW + 10, &[], 600);
        assert_eq!(v["friends"][0]["name"], "Bo");
        assert_eq!(v["friends"][0]["online"], false);
    }

    #[test]
    fn one_signature_is_not_a_friendship() {
        let (a, b, m) = (id(1), id(2), id(3));
        let doc = friendship_doc(&a.public_hex(), &b.public_hex(), NOW);
        let sa = a.sign_doc(CTX_FRIENDSHIP, &doc);
        let (ka, kb) = if a.public_hex() < b.public_hex() { ("sig_a", "sig_b") } else { ("sig_b", "sig_a") };
        let half = json!({"doc": doc, ka: sa, kb: m.sign_doc(CTX_FRIENDSHIP, &doc)});
        assert!(check_friendship(&half, &a.public_hex(), NOW).is_err());
        let full = json!({"doc": doc, ka: sa, kb: b.sign_doc(CTX_FRIENDSHIP, &doc)});
        assert!(check_friendship(&full, &a.public_hex(), NOW).is_ok());
        assert!(check_friendship(&full, &m.public_hex(), NOW).is_err(), "not m's");
        // a forged friendship in the cache is dropped on load
        let d = tempfile::tempdir().unwrap();
        let book = Book { friends: vec![Friend { friendship: half, ..Default::default() }, Friend { friendship: full, ..Default::default() }], ..Default::default() };
        book.save(d.path()).unwrap();
        let (l, dropped) = Book::load(d.path(), &a, NOW);
        assert_eq!((l.friends.len(), dropped), (1, 1));
        // an accept I never asked for is ignored, even fully signed
        let mut bb = Book::default();
        let m2 = make_msg(&b, &a.public_hex(), "accept", json!({"friendship": l.friends[0].friendship}), NOW).unwrap();
        assert_eq!(bb.receive(&a, &m2, Via::Lan, NOW).unwrap(), Event::Ignored("no request of mine"));
        assert!(bb.friends.is_empty());
    }

    #[test]
    fn requests_need_a_code_or_the_local_network() {
        let (a, b) = (id(1), id(2));
        let mut ba = Book { profile: named("Ari"), ..Default::default() };
        let mut bb = Book { profile: named("Bo"), ..Default::default() };
        // no token through the mailbox / internet: refused
        let req = ba.request(&a, &b.public_key, None, None, NOW).unwrap();
        assert_eq!(bb.receive(&b, &req, Via::Mailbox, NOW).unwrap(), Event::Ignored("no valid friend code"));
        assert_eq!(bb.receive(&b, &req, Via::Direct, NOW).unwrap(), Event::Ignored("no valid friend code"));
        // a made-up token: refused
        let req2 = ba.request(&a, &b.public_key, Some("abcdefgh"), None, NOW).unwrap();
        assert_eq!(bb.receive(&b, &req2, Via::Mailbox, NOW).unwrap(), Event::Ignored("no valid friend code"));
        // the local network: allowed, unless I'm invisible there
        assert_eq!(bb.receive(&b, &req, Via::Lan, NOW).unwrap(), Event::Request(a.peer_id.clone()));
        let mut hidden = Book { profile: Profile { lan: "off".into(), ..named("Hid") }, ..Default::default() };
        assert_eq!(hidden.receive(&b, &req, Via::Lan, NOW).unwrap(), Event::Ignored("no valid friend code"));
        // an expired code's token
        let code = parse_code(&bb.new_code(&b, NOW).unwrap()).unwrap();
        let req3 = ba.request(&a, &b.public_key, Some(&code.token), None, NOW + CODE_TTL + 5).unwrap();
        let mut bb2 = bb.clone();
        bb2.incoming.clear();
        assert_eq!(bb2.receive(&b, &req3, Via::Mailbox, NOW + CODE_TTL + 6).unwrap(), Event::Ignored("no valid friend code"));
        // decline: gone here, the asker's request dropped there
        let (_, dec) = bb.decline(&b, &a.peer_id, NOW + 1).unwrap();
        assert!(bb.incoming.is_empty());
        assert_eq!(ba.receive(&a, &dec, Via::Lan, NOW + 2).unwrap(), Event::Declined(b.peer_id.clone()));
        assert!(ba.outgoing.is_empty());
    }

    #[test]
    fn revocation_ends_it_both_sides_and_sticks() {
        let (a, b) = (id(1), id(2));
        let (mut ba, mut bb) = (Book { profile: named("Ari"), ..Default::default() }, Book { profile: named("Bo"), ..Default::default() });
        befriend(&a, &mut ba, &b, &mut bb);
        let old = ba.friends[0].clone();
        let (to, rv) = ba.revoke(&a, &b.peer_id, NOW + 10).unwrap();
        assert_eq!(to, b.public_hex());
        assert!(!ba.is_friend(&a, &b.peer_id));
        assert_eq!(bb.receive(&b, &rv, Via::Mailbox, NOW + 11).unwrap(), Event::Revoked(a.peer_id.clone()));
        assert!(!bb.is_friend(&b, &a.peer_id));
        // the old friendship written back into the cache is dropped on load
        ba.friends.push(old);
        let d = tempfile::tempdir().unwrap();
        ba.save(d.path()).unwrap();
        let (l, dropped) = Book::load(d.path(), &a, NOW + 20);
        assert!(l.friends.is_empty());
        assert_eq!(dropped, 1);
        // a revoke from a stranger does nothing
        let (c, mut bc) = (id(3), Book::default());
        let r = make_revoke(&c, "00000000000000000000000000000000", NOW);
        let m = make_msg(&c, &a.public_hex(), "revoke", json!({"revoke": r}), NOW).unwrap();
        assert_eq!(bc.receive(&a, &m, Via::Lan, NOW).unwrap(), Event::Ignored("not a friend"));
    }

    #[test]
    fn presence_invites_and_cards_only_from_friends() {
        let (a, b, c) = (id(1), id(2), id(3));
        let (mut ba, mut bb) = (Book { profile: named("Ari"), ..Default::default() }, Book { profile: named("Bo"), ..Default::default() });
        befriend(&a, &mut ba, &b, &mut bb);
        let pres = make_msg(&b, &a.public_hex(), "presence", json!({"online": true, "playing": "Single player", "open": {"kind": "single", "name": "Bo's world", "invite_hint": "friends"}}), NOW + 20).unwrap();
        assert_eq!(ba.receive(&a, &pres, Via::Lan, NOW + 20).unwrap(), Event::Presence(b.peer_id.clone()));
        let v = ba.view(&a, NOW + 30, &[], 600);
        assert_eq!(v["friends"][0]["online"], true);
        assert_eq!(v["friends"][0]["playing"], "Single player");
        assert_eq!(ba.view(&a, NOW + 2000, &[], 600)["friends"][0]["online"], false, "last seen long ago");
        // an older presence doesn't replace a newer one
        let older = make_msg(&b, &a.public_hex(), "presence", json!({"online": false}), NOW + 10).unwrap();
        assert_eq!(ba.receive(&a, &older, Via::Lan, NOW + 21).unwrap(), Event::Ignored("older presence"));
        // an invite
        let link = "https://relay.leylinelauncher.com/join/abcdefghijk2345a#SECRETSECRET~~G1.pw";
        let inv = make_msg(&b, &a.public_hex(), "invite", json!({"world": {"kind": "single", "name": "Bo's world"}, "link": link}), NOW + 22).unwrap();
        assert_eq!(ba.receive(&a, &inv, Via::Mailbox, NOW + 22).unwrap(), Event::Invite(b.peer_id.clone()));
        assert!(ba.view(&a, NOW + 23, &[], 600)["friends"][0]["invite"].is_object());
        let bad = make_msg(&b, &a.public_hex(), "invite", json!({"world": {"kind": "single", "name": "x"}, "link": "https://evil.example/x"}), NOW + 22).unwrap();
        assert!(ba.receive(&a, &bad, Via::Mailbox, NOW + 22).is_err());
        // a stranger's presence and invite are ignored
        let sp = make_msg(&c, &a.public_hex(), "presence", json!({"online": true}), NOW).unwrap();
        assert_eq!(ba.receive(&a, &sp, Via::Lan, NOW).unwrap(), Event::Ignored("not a friend"));
        // a newer card replaces the old one; one with someone else's id doesn't
        let mut p = named("Bo the Bold");
        p.seq = 5;
        let card = make_card(&b, &p, NOW + 30);
        let cm = make_msg(&b, &a.public_hex(), "card", json!({"card": card}), NOW + 30).unwrap();
        assert_eq!(ba.receive(&a, &cm, Via::Lan, NOW + 30).unwrap(), Event::Card(b.peer_id.clone()));
        assert_eq!(ba.view(&a, NOW + 30, &[], 600)["friends"][0]["name"], "Bo the Bold");
        let cm2 = make_msg(&b, &a.public_hex(), "card", json!({"card": make_card(&c, &Profile { seq: 9, ..named("Imposter") }, NOW)}), NOW + 31).unwrap();
        ba.receive(&a, &cm2, Via::Lan, NOW + 31).unwrap();
        assert_eq!(ba.view(&a, NOW + 31, &[], 600)["friends"][0]["name"], "Bo the Bold");
        // the stored presence and invite are re-checked on load
        let d = tempfile::tempdir().unwrap();
        ba.friends[0].invite.as_mut().unwrap()["msg"]["body"]["link"] = json!("https://relay.leylinelauncher.com/join/abcdefghijk2345a#CHANGED");
        ba.save(d.path()).unwrap();
        let (l, dropped) = Book::load(d.path(), &a, NOW + 40);
        assert_eq!(dropped, 1);
        assert!(l.friends[0].invite.is_none() && l.friends[0].presence.is_some());
    }

    #[test]
    fn mailbox_tags_only_the_sender_can_make() {
        let (a, b, c) = (id(1), id(2), id(3));
        let t = tag_for(&a, &b.public_hex(), "presence");
        assert_eq!(t.len(), 32);
        assert_eq!(t, tag_for(&a, &b.public_hex(), "presence"), "the same each time, so a newer box replaces the old one");
        assert_ne!(t, tag_for(&c, &b.public_hex(), "presence"), "another key can't make a's tag");
        assert_ne!(t, tag_for(&a, &b.public_hex(), "card"));
        assert_ne!(t, tag_for(&a, &c.public_hex(), "presence"));
    }

    #[test]
    fn messages_and_envelopes() {
        let (a, b, c) = (id(1), id(2), id(3));
        let m = make_msg(&a, &b.public_hex(), "presence", json!({"online": true}), NOW).unwrap();
        assert_eq!(check_msg(&m, &b.public_hex(), NOW).unwrap(), (a.public_hex(), "presence".to_string()));
        assert!(check_msg(&m, &c.public_hex(), NOW).is_err(), "for b only");
        assert!(check_msg(&m, &b.public_hex(), NOW + MSG_TTL + 1).is_err(), "too old");
        let mut t = m.clone();
        t["msg"]["body"]["online"] = json!(false);
        assert!(check_msg(&t, &b.public_hex(), NOW).is_err(), "changed");
        assert!(make_msg(&a, &b.public_hex(), "presence", json!({"x": "y".repeat(MSG_MAX)}), NOW).is_err(), "too big");
        let e = envelope(&b.public_key, &m, Some(&tag_for(&a, "b", "presence"))).unwrap();
        assert_eq!(e["to"], json!(b.peer_id));
        assert!(!e.to_string().contains("presence"), "nothing readable on the way");
        assert_eq!(open_envelope(&b, &e).unwrap(), m);
        assert!(open_envelope(&c, &e).is_err());
        let mut wrong = e.clone();
        wrong["to"] = json!(c.peer_id);
        assert!(open_envelope(&c, &wrong).is_err(), "c can't open b's box");
    }

    #[test]
    fn beacons() {
        let a = id(1);
        let bytes = make_beacon(&a, 41191, Some("Ari"), NOW).to_string();
        assert_eq!(check_beacon(bytes.as_bytes(), NOW + 5).unwrap(), (a.public_hex(), 41191, Some("Ari".into())));
        assert!(check_beacon(bytes.as_bytes(), NOW + BEACON_TTL + 1).is_err());
        let mut v: Value = serde_json::from_str(&bytes).unwrap();
        v["b"]["port"] = json!(1);
        assert!(check_beacon(v.to_string().as_bytes(), NOW).is_err(), "changed port");
        assert!(check_beacon(b"not json", NOW).is_err());
        assert!(check_beacon(&vec![b' '; BEACON_MAX + 1], NOW).is_err());
        let anon = make_beacon(&a, 41191, None, NOW).to_string();
        assert_eq!(check_beacon(anon.as_bytes(), NOW).unwrap().2, None);
    }

    #[test]
    fn codes() {
        let a = id(1);
        let c = format_code(&a.public_key, &[1, 2, 3, 4, 5], Some("relay.leylinelauncher.com"));
        assert!(c.starts_with("LF1-") && c.ends_with("@relay.leylinelauncher.com"));
        let p = parse_code(&format!("  {c}\n")).unwrap();
        assert_eq!((p.pubkey, p.relay.as_deref()), (a.public_key, Some("relay.leylinelauncher.com")));
        assert_eq!(p.token.len(), 8);
        assert!(parse_code(&format_code(&a.public_key, &[0; 5], None)).unwrap().relay.is_none());
        assert!(parse_code("LF1-abc").is_err());
        assert!(parse_code("DEN-1234").is_err());
        assert!(parse_code(&format!("{}@not a host", &c[..c.find('@').unwrap()])).is_err());
    }

    #[test]
    fn caps_on_load() {
        let a = id(1);
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("cache")).unwrap();
        // an oversized cache file is not read at all
        std::fs::write(cache_path(d.path()), vec![b' '; CACHE_MAX as usize + 1]).unwrap();
        let (b, _) = Book::load(d.path(), &a, NOW);
        assert!(b.friends.is_empty());
        // broken JSON: an empty book, never a crash
        std::fs::write(cache_path(d.path()), "{nope").unwrap();
        assert!(Book::load(d.path(), &a, NOW).0.friends.is_empty());
        // too many requests: only MAX_REQUESTS_IN kept
        let mut bk = Book { profile: named("Ari"), ..Default::default() };
        for n in 10..(10 + MAX_REQUESTS_IN as u8 + 5) {
            let x = id(n);
            let mut bx = Book { profile: named("x"), ..Default::default() };
            let req = bx.request(&x, &a.public_key, None, None, NOW).unwrap();
            bk.incoming.push(req);
        }
        let (l, dropped) = bk.checked(&a, NOW);
        assert_eq!(l.incoming.len(), MAX_REQUESTS_IN);
        assert_eq!(dropped, 5);
    }

    #[test]
    fn invite_links() {
        assert!(invite_link_ok("https://relay.leylinelauncher.com/join/abcdefghijk2345a#X"));
        assert!(!invite_link_ok("http://relay.leylinelauncher.com/join/abcdefghijk2345a#X"));
        assert!(!invite_link_ok("https://relay.leylinelauncher.com/join/abcdefghijk2345a"));
        assert!(!invite_link_ok("https://relay.leylinelauncher.com/x/abcdefghijk2345a#X"));
        assert!(!invite_link_ok("https://user@evil/join/abcdefghijk2345a#X"));
    }
}
