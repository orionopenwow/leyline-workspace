//! The Passport (docs/leyline/passport.md, docs/UNSTOPPABLE.md section 2): logging in with Leyline when no website and
//! no server of ours is there. It is the launcher's own peer key (identity.rs, `peer.key`), grown up. Every record is
//! Ed25519 over `context + "\n" + canonical JSON` (sorted keys, no spaces), like every other Leyline record, and test
//! vectors for other launchers are in `p2p/passport-v1-vectors.json`.
//!
//! * **Root key and recovery phrase.** A new launcher makes its root seed FROM a 12-word phrase (BIP-39 English
//!   words, 128 bits, checksum): seed = HKDF-SHA256(salt `leyline-passport-root-v1`, the phrase's 128 bits, info
//!   `root`). The 12 words wait owner-only in `passport/phrase.pending` until the player has seen them once on the
//!   setup screen, then that file is deleted. A `peer.key` made before the Passport has no 12 words (its seed is 256
//!   random bits), so its phrase is **24 words that are the seed itself** (BIP-39 over 256 bits). Either phrase
//!   restores the same root, so the peer id, the friend code, every friendship and every lasting login carry over;
//!   no key rotation is needed for old keys (a rotation would end every friendship signed with the old key).
//! * **Device records** (`leyline-passport-device-v1`): `{root, device, name, created, seq}` signed by the root; and
//!   **revocations** (`leyline-passport-device-revoke-v1`): `{root, device, created, seq}`. A revocation beats every
//!   device record of that device with a lower or equal seq, so an old record can't be replayed back in. Every
//!   device holds the root today (the phrase or the second-device exchange gives it); a device's own key
//!   (`passport/device.key`) names it so it can be listed and revoked. Day-to-day signing by device keys is coming.
//! * **World keys**: one per world, `PeerIdentity::derive("leyline-world-key-v1", realm id)` (HKDF-SHA256 from the
//!   root seed). A host sees only that key, so two hosts can't link a player.
//! * **Login agreement** (`leyline-login-v1`): `{player (world key), realm, account, created, seq}` signed by the
//!   player's world key AND the host's realm key (realm id = base32(sha256(realm public key))[..16]). Both launchers
//!   keep it; a higher seq replaces a lower one; the host can make the account again from the player's copy.
//! * **Guardians** (`leyline-passport-guardians-v1`): 2 to 5 friends' keys, signed by the root. **Rotation**
//!   (`leyline-passport-rotate-v1`): `{kind, old, new}` moves a Passport to a new root, either signed by the old root
//!   (`self`) or approved by 2 of the listed guardians (`guardians`, each approval signing the doc with its own
//!   `ts`). A guardians rotation takes effect [`GUARDIAN_WAIT`] after its newest approval, and a **veto**
//!   (`leyline-passport-veto-v1`, signed by the old root) stops it in that time. Friends' launchers and hosts accept
//!   the new root for existing friendships and lasting logins.
//! * **Second device by code**: a commit-then-reveal X25519 exchange over the friends' LAN/direct listener
//!   (friendnet.rs), with a 6-digit comparison code shown on the new PC and typed on the old one (a man in the middle
//!   wins with odds of one in a million), then the root sealed under the exchanged key. No network: a sealed file
//!   locked with 6 random words (66 bits, PBKDF2-HMAC-SHA256).
//! * **Hero Key alias** (`leyline-passport-herokey-v1`): the old website key imported once as `{root, id, hash}`
//!   (the key's own id and a hash; the key itself is never kept).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::identity::{self, canonical, hex, unhex, unhex32, verify_doc, PeerIdentity};

pub const CTX_DEVICE: &str = "leyline-passport-device-v1";
pub const CTX_DEVICE_REVOKE: &str = "leyline-passport-device-revoke-v1";
pub const CTX_GUARDIANS: &str = "leyline-passport-guardians-v1";
pub const CTX_ROTATE: &str = "leyline-passport-rotate-v1";
pub const CTX_VETO: &str = "leyline-passport-veto-v1";
pub const CTX_LOGIN: &str = "leyline-login-v1";
pub const CTX_HEROKEY: &str = "leyline-passport-herokey-v1";
pub const ROOT_SALT: &str = "leyline-passport-root-v1";
pub const WORLD_SALT: &str = "leyline-world-key-v1";
const PAIR_CTX: &str = "leyline-pair-v1";
const FILE_CTX: &str = "leyline-passport-file-v1";
/// A guardians rotation waits this long after its newest approval, so a device that still has the Passport can veto.
pub const GUARDIAN_WAIT: u64 = 3 * 86400;
pub const GUARDIANS_MIN: usize = 2;
pub const GUARDIANS_MAX: usize = 5;
pub const GUARDIANS_NEEDED: usize = 2;
pub const SKEW: u64 = 300;
/// A pairing waits this long for the other PC.
pub const PAIR_TTL: u64 = 300;
const FILE_ROUNDS: u32 = 200_000;
pub const DIR: &str = "passport";
const MAX_DEVICES: usize = 20;
const MAX_GUARDING: usize = 50;
const MAX_VETOES: usize = 50;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn sha(b: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(b).into()
}

fn hex64(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).map(String::from)
}

fn clip(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

fn random<const N: usize>() -> Result<[u8; N], String> {
    use ring::rand::SecureRandom;
    let mut b = [0u8; N];
    ring::rand::SystemRandom::new().fill(&mut b).map_err(|_| "This PC has no random source.".to_string())?;
    Ok(b)
}

/// HKDF-SHA256 (RFC 5869) with a 32-byte output: HMAC(HMAC(salt, ikm), info || 1).
pub fn hkdf32(salt: &[u8], ikm: &[u8], info: &[u8]) -> [u8; 32] {
    use ring::hmac;
    let prk = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, salt), ikm);
    let mut m = info.to_vec();
    m.push(1);
    let t = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, prk.as_ref()), &m);
    let mut out = [0u8; 32];
    out.copy_from_slice(t.as_ref());
    out
}

fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn aead_seal(key: &[u8; 32], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, String> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
    // every key here is used once (a fresh exchange, a fresh salt), so the zero nonce is safe
    let k = LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, key).map_err(|_| "key")?);
    let mut buf = plain.to_vec();
    k.seal_in_place_append_tag(Nonce::assume_unique_for_key([0u8; 12]), Aad::from(aad), &mut buf).map_err(|_| "seal".to_string())?;
    Ok(buf)
}

fn aead_open(key: &[u8; 32], aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
    let k = LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, key).ok()?);
    let mut buf = sealed.to_vec();
    let n = k.open_in_place(Nonce::assume_unique_for_key([0u8; 12]), Aad::from(aad), &mut buf).ok()?.len();
    buf.truncate(n);
    Some(buf)
}

fn write_secret(path: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::settings::write_atomic(path, bytes).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------ the phrase

static WORDS_TXT: &str = include_str!("passport-words.txt");

/// The 2048 BIP-39 English words.
pub fn words() -> &'static [&'static str] {
    static W: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    W.get_or_init(|| WORDS_TXT.lines().map(str::trim).filter(|l| !l.is_empty()).collect())
}

/// BIP-39 words for 16 bytes (12 words) or 32 bytes (24 words).
pub fn phrase_from_entropy(e: &[u8]) -> String {
    let cs_bits = e.len() / 4;
    let h = sha(e);
    let mut bits: Vec<bool> = e.iter().flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1 == 1)).collect();
    bits.extend((0..cs_bits).map(|i| (h[i / 8] >> (7 - i % 8)) & 1 == 1));
    bits.chunks(11).map(|c| words()[c.iter().fold(0usize, |a, &b| (a << 1) | b as usize)]).collect::<Vec<_>>().join(" ")
}

fn word_index(tok: &str) -> Option<usize> {
    let w = words();
    if let Ok(i) = w.binary_search(&tok) {
        return Some(i);
    }
    // the first four letters are enough (BIP-39 words differ in them)
    if tok.len() >= 4 {
        let m: Vec<usize> = (0..w.len()).filter(|&i| w[i].starts_with(&tok[..4]) && (w[i].starts_with(tok) || tok.len() == 4)).collect();
        if m.len() == 1 {
            return Some(m[0]);
        }
    }
    None
}

/// The 16 or 32 bytes in a 12- or 24-word phrase (any case, any spacing; the checksum checked).
pub fn entropy_from_phrase(phrase: &str) -> Result<Vec<u8>, String> {
    let toks: Vec<String> = phrase.split(|c: char| !c.is_ascii_alphabetic()).filter(|t| !t.is_empty()).map(|t| t.to_ascii_lowercase()).collect();
    if toks.len() != 12 && toks.len() != 24 {
        return Err(format!("A recovery phrase has 12 or 24 words. That has {}.", toks.len()));
    }
    let mut bits = Vec::with_capacity(toks.len() * 11);
    for (n, t) in toks.iter().enumerate() {
        let i = word_index(t).ok_or_else(|| format!("Word {} (\"{}\") isn't a recovery word. Check its spelling.", n + 1, clip(t, 12)))?;
        bits.extend((0..11).rev().map(|k| (i >> k) & 1 == 1));
    }
    let ent_bits = toks.len() * 11 * 32 / 33;
    let e: Vec<u8> = bits[..ent_bits].chunks(8).map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8)).collect();
    let h = sha(&e);
    let ok = bits[ent_bits..].iter().enumerate().all(|(i, &b)| ((h[i / 8] >> (7 - i % 8)) & 1 == 1) == b);
    if !ok {
        return Err("Those words don't fit together: one is wrong or out of place.".into());
    }
    Ok(e)
}

/// The root seed of a 12-word phrase's 128 bits.
pub fn seed_from_entropy(e: &[u8; 16]) -> [u8; 32] {
    hkdf32(ROOT_SALT.as_bytes(), e, b"root")
}

/// The root seed a phrase restores: 12 words through [`seed_from_entropy`]; 24 words are the seed itself (a key made
/// before the Passport).
pub fn seed_from_phrase(phrase: &str) -> Result<[u8; 32], String> {
    let e = entropy_from_phrase(phrase)?;
    Ok(match e.len() {
        16 => seed_from_entropy(&e.try_into().map_err(|_| "phrase")?),
        _ => e.try_into().map_err(|_| "phrase")?,
    })
}

pub fn dir(app: &Path) -> PathBuf {
    app.join(DIR)
}

fn pending_path(app: &Path) -> PathBuf {
    dir(app).join("phrase.pending")
}

/// A new key's 12 words kept (owner-only) until the player has seen them ([`phrase_saved`]).
pub fn keep_pending_phrase(app: &Path, entropy: &[u8; 16]) -> Result<(), String> {
    std::fs::create_dir_all(dir(app)).map_err(|e| e.to_string())?;
    write_secret(&pending_path(app), (hex(entropy) + "\n").as_bytes())
}

/// The 12 words of `root`, while they wait to be seen (only if they really make this root).
pub fn pending_phrase(app: &Path, root: &PeerIdentity) -> Option<String> {
    let t = std::fs::read_to_string(pending_path(app)).ok()?;
    let e: [u8; 16] = unhex(t.trim()).and_then(|v| v.try_into().ok())?;
    same(&seed_from_entropy(&e), &root.secret_seed()).then(|| phrase_from_entropy(&e))
}

/// The phrase for this Passport: its 12 words while they wait, else the 24 words of the seed.
pub fn phrase_for(app: &Path, root: &PeerIdentity) -> String {
    pending_phrase(app, root).unwrap_or_else(|| phrase_from_entropy(&root.secret_seed()))
}

/// "I saved it": the waiting words are deleted (the 24-word phrase still works for a backup later).
pub fn phrase_saved(app: &Path) -> Result<(), String> {
    match std::fs::remove_file(pending_path(app)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// ------------------------------------------------------------------------------------------------ device records

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceInfo {
    pub root: String,
    pub device: String,
    pub id: String,
    pub name: String,
    pub created: u64,
    pub seq: u64,
}

pub fn make_device(root: &PeerIdentity, device_pk: &str, name: &str, created: u64, seq: u64) -> Value {
    let doc = json!({"v": 1, "root": root.public_hex(), "device": device_pk, "name": clip(name, 32), "created": created, "seq": seq});
    json!({"doc": doc, "sig": root.sign_doc(CTX_DEVICE, &doc)})
}

/// A device record signed by its root (`root`: when given, it must be that one).
pub fn check_device(rec: &Value, root: Option<&str>) -> Result<DeviceInfo, String> {
    let d = rec.get("doc").filter(|d| d["v"] == 1).ok_or("no device record")?;
    let r = hex64(d.get("root")).ok_or("root")?;
    let dev = hex64(d.get("device")).ok_or("device")?;
    if root.is_some_and(|x| x != r) {
        return Err("another Passport's device".into());
    }
    let name = d.get("name").and_then(Value::as_str).filter(|n| n.chars().count() <= 32).ok_or("name")?.to_string();
    let created = d.get("created").and_then(Value::as_u64).ok_or("created")?;
    let seq = d.get("seq").and_then(Value::as_u64).ok_or("seq")?;
    if !verify_doc(&r, d, CTX_DEVICE, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
        return Err("device signature".into());
    }
    let id = identity::peer_id(&unhex(&dev).unwrap_or_default());
    Ok(DeviceInfo { root: r, device: dev, id, name, created, seq })
}

pub fn make_device_revoke(root: &PeerIdentity, device_pk: &str, created: u64, seq: u64) -> Value {
    let doc = json!({"v": 1, "root": root.public_hex(), "device": device_pk, "created": created, "seq": seq});
    json!({"doc": doc, "sig": root.sign_doc(CTX_DEVICE_REVOKE, &doc)})
}

/// A revocation: (root, device key, seq).
pub fn check_device_revoke(rec: &Value, root: Option<&str>) -> Result<(String, String, u64), String> {
    let d = rec.get("doc").filter(|d| d["v"] == 1).ok_or("no revocation")?;
    let r = hex64(d.get("root")).ok_or("root")?;
    let dev = hex64(d.get("device")).ok_or("device")?;
    if root.is_some_and(|x| x != r) {
        return Err("another Passport's revocation".into());
    }
    d.get("created").and_then(Value::as_u64).ok_or("created")?;
    let seq = d.get("seq").and_then(Value::as_u64).ok_or("seq")?;
    if !verify_doc(&r, d, CTX_DEVICE_REVOKE, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
        return Err("revocation signature".into());
    }
    Ok((r, dev, seq))
}

// ------------------------------------------------------------------------------------------------ world keys

/// This Passport's key for one world (HKDF from the root seed and the world's realm id).
pub fn world_identity(root: &PeerIdentity, realm_id: &str) -> PeerIdentity {
    root.derive(WORLD_SALT, realm_id)
}

// ------------------------------------------------------------------------------------------------ login agreements

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginInfo {
    pub player: String,
    pub realm: String,
    pub account: String,
    pub host: String,
    pub created: u64,
    pub seq: u64,
}

fn account_ok(a: &str) -> bool {
    (1..=32).contains(&a.len()) && a.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

pub fn login_doc(player_pk: &str, realm_id: &str, account: &str, created: u64, seq: u64) -> Value {
    json!({"v": 1, "player": player_pk, "realm": realm_id, "account": account, "created": created, "seq": seq})
}

/// The host's half: the doc and the realm key's signature (the player signs next).
pub fn login_half(host: &PeerIdentity, doc: &Value) -> Value {
    json!({"doc": doc, "host": host.public_hex(), "sig_host": host.sign_doc(CTX_LOGIN, doc)})
}

/// The player's signature added to a host's half (checked: the host's signature, this world, this key, the account
/// the world gave): the whole record.
pub fn login_countersign(player: &PeerIdentity, half: &Value, realm_id: &str, account: &str) -> Result<Value, String> {
    let d = half.get("doc").ok_or("no login record")?;
    if d["player"].as_str() != Some(player.public_hex().as_str()) || d["realm"].as_str() != Some(realm_id) || d["account"].as_str() != Some(account) {
        return Err("That login record isn't for this world, this key or this account.".into());
    }
    let host = hex64(half.get("host")).ok_or("host")?;
    if !verify_doc(&host, d, CTX_LOGIN, half["sig_host"].as_str().unwrap_or("")) {
        return Err("The world's signature doesn't hold.".into());
    }
    let mut rec = half.clone();
    rec["sig_player"] = json!(player.sign_doc(CTX_LOGIN, d));
    check_login(&rec)?;
    Ok(rec)
}

/// A login agreement signed by both: the player's world key and the realm key whose id is the record's realm.
pub fn check_login(rec: &Value) -> Result<LoginInfo, String> {
    let d = rec.get("doc").filter(|d| d["v"] == 1).ok_or("no login record")?;
    let player = hex64(d.get("player")).ok_or("player")?;
    let host = hex64(rec.get("host")).ok_or("host")?;
    let realm = d.get("realm").and_then(Value::as_str).filter(|r| identity::peer_id_ok(r)).ok_or("realm")?.to_string();
    let account = d.get("account").and_then(Value::as_str).filter(|a| account_ok(a)).ok_or("account")?.to_string();
    let created = d.get("created").and_then(Value::as_u64).ok_or("created")?;
    let seq = d.get("seq").and_then(Value::as_u64).filter(|s| *s >= 1).ok_or("seq")?;
    if identity::peer_id(&unhex(&host).unwrap_or_default()) != realm {
        return Err("signed by another world's key".into());
    }
    let s = |k: &str| rec.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    if !verify_doc(&host, d, CTX_LOGIN, &s("sig_host")) {
        return Err("the world's signature".into());
    }
    if !verify_doc(&player, d, CTX_LOGIN, &s("sig_player")) {
        return Err("the player's signature".into());
    }
    Ok(LoginInfo { player, realm, account, host, created, seq })
}

/// A record kept under `key` in `book`: a newer seq replaces an older one; the same record again changes nothing; an
/// older one, or another record with the same seq (a replay), is refused. Whether the book changed.
pub fn keep_login(book: &mut std::collections::BTreeMap<String, Value>, key: &str, rec: &Value) -> Result<bool, String> {
    let n = check_login(rec)?;
    if let Some(old) = book.get(key) {
        if old == rec {
            return Ok(false);
        }
        let o = check_login(old).map(|o| o.seq).unwrap_or(0);
        if n.seq <= o {
            return Err("an older login record (or one played back): the newer one is kept".into());
        }
    }
    book.insert(key.to_string(), rec.clone());
    Ok(true)
}

pub fn login_book(path: &Path) -> std::collections::BTreeMap<String, Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_login_book(path: &Path, b: &std::collections::BTreeMap<String, Value>) -> Result<(), String> {
    crate::settings::write_atomic(path, serde_json::to_string_pretty(b).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())
}

/// The player's side: `<app>/passport/logins.json`, one record per world (realm id).
pub fn player_logins_path(app: &Path) -> PathBuf {
    dir(app).join("logins.json")
}

/// The host's side: `<world share>/logins.json`, one record per player's world key.
pub fn host_logins_path(share: &Path) -> PathBuf {
    share.join("logins.json")
}

/// The player keeps a record the world signed with it.
pub fn keep_player_login(app: &Path, rec: &Value) -> Result<(), String> {
    let n = check_login(rec)?;
    let p = player_logins_path(app);
    let mut b = login_book(&p);
    if keep_login(&mut b, &n.realm, rec)? {
        save_login_book(&p, &b)?;
    }
    Ok(())
}

/// The player's record for one world, if any.
pub fn player_login(app: &Path, realm_id: &str) -> Option<Value> {
    login_book(&player_logins_path(app)).remove(realm_id).filter(|r| check_login(r).is_ok())
}

/// Which key claims a friend's world, and what the claim carries: a world joined before the Passport (or with the
/// root key) keeps the root key and a v1 claim (so that friend's login keeps working), with the rotation that made
/// this root if there is one; any other world gets this world's own world key, a v2 claim, and this player's copy
/// of the last login agreement there.
pub fn claim_for(app: &Path, root: &PeerIdentity, realm_id: &str) -> (PeerIdentity, crate::host::worldfriends::Passport) {
    use crate::host::worldfriends as wf;
    match wf::worlds(app).get(realm_id) {
        Some(j) if j.key != 2 => {
            let rot = State::load(app, root).rotation;
            (PeerIdentity::from_seed(root.secret_seed()).expect("a seed"), wf::Passport { v2: false, rec: None, rot })
        }
        _ => (world_identity(root, realm_id), wf::Passport { v2: true, rec: player_login(app, realm_id), rot: None }),
    }
}

/// The host's side: the friend's lasting login made again from the player's copy of the agreement (a host whose
/// friends list was lost, its realm key kept): only for this world and the key that signs the claim.
pub fn rebuild_friend(rec: &Value, realm_id: &str, player_pk: &str, t: u64) -> Result<crate::host::worldfriends::Friend, String> {
    let n = check_login(rec)?;
    if n.realm != realm_id || n.player != player_pk {
        return Err("That login record is for another world or another key.".into());
    }
    if !crate::host::worldfriends::friend_login_ok(&n.account) {
        return Err("That login record's account isn't a friend's login.".into());
    }
    Ok(crate::host::worldfriends::Friend {
        user: n.account,
        peer: identity::peer_id(&unhex(&n.player).unwrap_or_default()),
        key: n.player,
        made: n.created,
        seen: t,
        enabled: true,
        ..Default::default()
    })
}

// ------------------------------------------------------------------------------------------------ guardians and rotation

pub fn make_guardians(root: &PeerIdentity, guardians: &[String], created: u64, seq: u64) -> Result<Value, String> {
    let mut g: Vec<String> = guardians.iter().map(|k| k.to_ascii_lowercase()).collect();
    g.sort();
    g.dedup();
    if g.len() < GUARDIANS_MIN || g.len() > GUARDIANS_MAX {
        return Err(format!("Pick {GUARDIANS_MIN} to {GUARDIANS_MAX} friends as guardians."));
    }
    if g.iter().any(|k| hex64(Some(&json!(k))).is_none() || *k == root.public_hex()) {
        return Err("A guardian has to be a friend's key, not yours.".into());
    }
    let doc = json!({"v": 1, "root": root.public_hex(), "guardians": g, "needed": GUARDIANS_NEEDED, "created": created, "seq": seq});
    Ok(json!({"doc": doc, "sig": root.sign_doc(CTX_GUARDIANS, &doc)}))
}

/// A guardians list: (root, guardian keys, seq).
pub fn check_guardians(rec: &Value) -> Result<(String, Vec<String>, u64), String> {
    let d = rec.get("doc").filter(|d| d["v"] == 1).ok_or("no guardians list")?;
    let root = hex64(d.get("root")).ok_or("root")?;
    let g: Vec<String> = d.get("guardians").and_then(Value::as_array).ok_or("guardians")?.iter().map(|k| hex64(Some(k))).collect::<Option<_>>().ok_or("guardian key")?;
    if g.len() < GUARDIANS_MIN || g.len() > GUARDIANS_MAX || g.windows(2).any(|w| w[0] >= w[1]) || g.contains(&root) {
        return Err("guardians list shape".into());
    }
    if d.get("needed").and_then(Value::as_u64) != Some(GUARDIANS_NEEDED as u64) {
        return Err("needed".into());
    }
    d.get("created").and_then(Value::as_u64).ok_or("created")?;
    let seq = d.get("seq").and_then(Value::as_u64).ok_or("seq")?;
    if !verify_doc(&root, d, CTX_GUARDIANS, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
        return Err("guardians signature".into());
    }
    Ok((root, g, seq))
}

/// A rotation the old root signs itself (a key moved on purpose).
pub fn make_rotation_self(old: &PeerIdentity, new_pk: &str, created: u64) -> Value {
    let doc = json!({"v": 1, "kind": "self", "old": old.public_hex(), "new": new_pk, "created": created});
    json!({"doc": doc, "sig": old.sign_doc(CTX_ROTATE, &doc)})
}

fn approval_doc(old: &str, new: &str, ts: u64) -> Value {
    json!({"v": 1, "kind": "guardians", "old": old, "new": new, "ts": ts})
}

/// One guardian's approval of moving `old` to `new`.
pub fn approve_rotation(guardian: &PeerIdentity, old: &str, new: &str, ts: u64) -> Value {
    json!({"guardian": guardian.public_hex(), "ts": ts, "sig": guardian.sign_doc(CTX_ROTATE, &approval_doc(old, new, ts))})
}

/// The rotation record of a guardians recovery: the old root's guardians list and the approvals.
pub fn rotation_bundle(guardians: &Value, new_pk: &str, approvals: &[Value]) -> Result<Value, String> {
    let (old, _, _) = check_guardians(guardians)?;
    let b = json!({"doc": {"v": 1, "kind": "guardians", "old": old, "new": new_pk}, "guardians": guardians, "approvals": approvals});
    check_rotation(&b)?;
    Ok(b)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rotation {
    pub id: String,
    pub old: String,
    pub new: String,
    pub kind: String,
    /// When friends and hosts accept it (a guardians rotation waits [`GUARDIAN_WAIT`] for a veto).
    pub effective: u64,
}

pub fn rotation_id(rec: &Value) -> String {
    hex(&sha(canonical(&rec["doc"]).as_bytes()))[..32].to_string()
}

/// A rotation's signatures: the old root's own, or 2 different guardians from the old root's own list.
pub fn check_rotation(rec: &Value) -> Result<Rotation, String> {
    let d = rec.get("doc").filter(|d| d["v"] == 1).ok_or("no rotation")?;
    let old = hex64(d.get("old")).ok_or("old")?;
    let new = hex64(d.get("new")).ok_or("new")?;
    if old == new {
        return Err("a rotation to the same key".into());
    }
    let id = rotation_id(rec);
    match d.get("kind").and_then(Value::as_str) {
        Some("self") => {
            let created = d.get("created").and_then(Value::as_u64).ok_or("created")?;
            if !verify_doc(&old, d, CTX_ROTATE, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
                return Err("rotation signature".into());
            }
            Ok(Rotation { id, old, new, kind: "self".into(), effective: created })
        }
        Some("guardians") => {
            let (root, list, _) = check_guardians(rec.get("guardians").ok_or("no guardians list")?)?;
            if root != old {
                return Err("another Passport's guardians".into());
            }
            let mut ok: Vec<(String, u64)> = Vec::new();
            for a in rec.get("approvals").and_then(Value::as_array).ok_or("approvals")?.iter().take(GUARDIANS_MAX) {
                let (Some(g), Some(ts)) = (hex64(a.get("guardian")), a.get("ts").and_then(Value::as_u64)) else { continue };
                if !list.contains(&g) || ok.iter().any(|(x, _)| *x == g) {
                    continue;
                }
                if verify_doc(&g, &approval_doc(&old, &new, ts), CTX_ROTATE, a.get("sig").and_then(Value::as_str).unwrap_or("")) {
                    ok.push((g, ts));
                }
            }
            if ok.len() < GUARDIANS_NEEDED {
                return Err(format!("needs {GUARDIANS_NEEDED} guardians, has {}", ok.len()));
            }
            let newest = ok.iter().map(|(_, t)| *t).max().unwrap_or(0);
            Ok(Rotation { id, old, new, kind: "guardians".into(), effective: newest + GUARDIAN_WAIT })
        }
        _ => Err("kind".into()),
    }
}

pub fn make_veto(old: &PeerIdentity, rotation_id: &str, created: u64) -> Value {
    let doc = json!({"v": 1, "root": old.public_hex(), "rotation": rotation_id, "created": created});
    json!({"doc": doc, "sig": old.sign_doc(CTX_VETO, &doc)})
}

/// A veto: (old root, rotation id).
pub fn check_veto(rec: &Value) -> Result<(String, String), String> {
    let d = rec.get("doc").filter(|d| d["v"] == 1).ok_or("no veto")?;
    let root = hex64(d.get("root")).ok_or("root")?;
    let rid = d.get("rotation").and_then(Value::as_str).filter(|r| r.len() == 32 && r.bytes().all(|b| b.is_ascii_hexdigit())).ok_or("rotation")?;
    d.get("created").and_then(Value::as_u64).ok_or("created")?;
    if !verify_doc(&root, d, CTX_VETO, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
        return Err("veto signature".into());
    }
    Ok((root, rid.to_string()))
}

/// Whether a rotation is in force at `now`: its signatures, its waiting time, and no veto from the old root.
pub fn rotation_in_force(rec: &Value, now: u64, vetoes: &[Value]) -> Result<Rotation, String> {
    let r = check_rotation(rec)?;
    if vetoes.iter().filter_map(|v| check_veto(v).ok()).any(|(root, id)| root == r.old && id == r.id) {
        return Err("stopped by the Passport's owner".into());
    }
    if now < r.effective {
        return Err(format!("waiting ({} hours left)", (r.effective - now).div_ceil(3600)));
    }
    Ok(r)
}

/// The text a guardian gives back: their approval and the list it is checked against (`LG1-` + base32 JSON).
pub fn guardian_approval_text(guardian: &PeerIdentity, guardians: &Value, roster: &[String], new_pk: &str, ts: u64) -> Result<String, String> {
    let (old, list, _) = check_guardians(guardians)?;
    if !list.contains(&guardian.public_hex()) {
        return Err("You aren't one of their guardians.".into());
    }
    if hex64(Some(&json!(new_pk))).is_none() {
        return Err("That isn't a recovery code.".into());
    }
    let roster: Vec<&String> = roster.iter().filter(|k| hex64(Some(&json!(k))).is_some()).take(200).collect();
    let body = json!({"v": 1, "approval": approve_rotation(guardian, &old, new_pk, ts), "guardians": guardians, "roster": roster});
    Ok(format!("LG1-{}", identity::b32(canonical(&body).as_bytes())))
}

/// An approval text read: (approval, guardians list, roster).
pub fn parse_approval_text(text: &str) -> Result<(Value, Value, Vec<String>), String> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let b = t.strip_prefix("LG1-").or_else(|| t.strip_prefix("lg1-")).and_then(identity::unb32).ok_or("That isn't a guardian's answer (it starts with LG1-).")?;
    let v: Value = serde_json::from_slice(&b).map_err(|_| "That guardian's answer didn't read.".to_string())?;
    check_guardians(&v["guardians"]).map_err(|_| "That guardian's answer didn't read.".to_string())?;
    let roster = v["roster"].as_array().map(|a| a.iter().filter_map(|k| hex64(Some(k))).collect()).unwrap_or_default();
    Ok((v["approval"].clone(), v["guardians"].clone(), roster))
}

/// The code a recovering PC shows its guardians: `LR1-` + base32 of its new public key.
pub fn recovery_code(new_root: &PeerIdentity) -> String {
    format!("LR1-{}", identity::b32(&new_root.public_key).to_ascii_uppercase())
}

pub fn parse_recovery_code(text: &str) -> Result<String, String> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    t.strip_prefix("lr1-").and_then(identity::unb32).filter(|v| v.len() == 32).map(|v| hex(&v)).ok_or("That isn't a recovery code (it starts with LR1-).".into())
}

// ------------------------------------------------------------------------------------------------ Hero Key alias

/// The old website key, imported once as an alias: its id and a hash, never the key.
pub fn make_herokey_alias(root: &PeerIdentity, pasted: &str, created: u64) -> Result<Value, String> {
    let key = crate::social::find_key(pasted).ok_or("That isn't a Hero Key file (the key starts with ovk1.).")?;
    let id = key[5..15].to_string();
    let hash = hex(&sha(format!("{CTX_HEROKEY}\n{key}").as_bytes()));
    let doc = json!({"v": 1, "root": root.public_hex(), "id": id, "hash": hash, "created": created});
    Ok(json!({"doc": doc, "sig": root.sign_doc(CTX_HEROKEY, &doc)}))
}

/// An alias: (root, Hero Key id).
pub fn check_herokey_alias(rec: &Value) -> Result<(String, String), String> {
    let d = rec.get("doc").filter(|d| d["v"] == 1).ok_or("no alias")?;
    let root = hex64(d.get("root")).ok_or("root")?;
    let id = d.get("id").and_then(Value::as_str).filter(|i| i.len() == 10 && i.bytes().all(|b| b.is_ascii_alphanumeric())).ok_or("id")?;
    hex64(d.get("hash")).ok_or("hash")?;
    if !verify_doc(&root, d, CTX_HEROKEY, rec.get("sig").and_then(Value::as_str).unwrap_or("")) {
        return Err("alias signature".into());
    }
    Ok((root, id.to_string()))
}

// ------------------------------------------------------------------------------------------------ the state on this PC

/// A friend's guardians list naming this player, and the friend keys they asked us to keep for their recovery.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Guarding {
    pub rec: Value,
    pub roster: Vec<String>,
}

/// `<app>/passport/state.json`: records only, no secrets (the root is peer.key; this device's key is device.key).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub v: u32,
    pub name: String,
    pub picture: String,
    pub setup_done: bool,
    pub seq: u64,
    pub devices: Vec<Value>,
    pub revoked: Vec<Value>,
    pub guardians: Option<Value>,
    pub guarding: Vec<Guarding>,
    /// How this root came to be, when it replaced an older one (shown to friends and hosts).
    pub rotation: Option<Value>,
    pub vetoes: Vec<Value>,
    /// Rotations of this root seen from elsewhere (someone asking guardians for it): the player may veto.
    pub alerts: Vec<Value>,
    pub herokey: Option<Value>,
}

fn state_path(app: &Path) -> PathBuf {
    dir(app).join("state.json")
}

impl State {
    /// Loaded and re-checked against this root: anything forged or another root's is dropped.
    pub fn load(app: &Path, root: &PeerIdentity) -> State {
        let s: State = std::fs::read_to_string(state_path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        s.checked(root)
    }

    pub fn checked(mut self, root: &PeerIdentity) -> State {
        let r = root.public_hex();
        self.v = 1;
        self.name = clip(&self.name, 32);
        self.picture = clip(&self.picture, 64);
        self.revoked.retain(|x| check_device_revoke(x, Some(&r)).is_ok());
        let revs: Vec<(String, u64)> = self.revoked.iter().filter_map(|x| check_device_revoke(x, Some(&r)).ok().map(|(_, d, s)| (d, s))).collect();
        self.devices.retain(|x| check_device(x, Some(&r)).is_ok_and(|d| !revs.iter().any(|(k, s)| *k == d.device && *s >= d.seq)));
        self.devices.truncate(MAX_DEVICES);
        if self.guardians.as_ref().is_some_and(|g| check_guardians(g).map(|(x, _, _)| x != r).unwrap_or(true)) {
            self.guardians = None;
        }
        self.guarding.retain(|g| check_guardians(&g.rec).is_ok_and(|(_, l, _)| l.contains(&r)));
        self.guarding.truncate(MAX_GUARDING);
        if self.rotation.as_ref().is_some_and(|x| check_rotation(x).map(|k| k.new != r).unwrap_or(true)) {
            self.rotation = None;
        }
        self.vetoes.retain(|v| check_veto(v).is_ok());
        self.vetoes.truncate(MAX_VETOES);
        self.alerts.retain(|a| check_rotation(a).is_ok_and(|k| k.old == r));
        self.alerts.truncate(10);
        if self.herokey.as_ref().is_some_and(|h| check_herokey_alias(h).map(|(x, _)| x != r).unwrap_or(true)) {
            self.herokey = None;
        }
        self
    }

    pub fn save(&self, app: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir(app)).map_err(|e| e.to_string())?;
        crate::settings::write_atomic(&state_path(app), serde_json::to_string_pretty(self).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())
    }

    /// The next seq for a record made here (always rising, and never below the clock).
    pub fn next_seq(&mut self, t: u64) -> u64 {
        self.seq = (self.seq + 1).max(t);
        self.seq
    }

    /// A device record taken in: newer replaces older; refused when revoked at that seq or later, or an older replay.
    pub fn add_device(&mut self, rec: &Value, root: &str) -> Result<DeviceInfo, String> {
        let d = check_device(rec, Some(root))?;
        if self.revoked.iter().filter_map(|x| check_device_revoke(x, Some(root)).ok()).any(|(_, k, s)| k == d.device && s >= d.seq) {
            return Err("That device was removed from your Passport.".into());
        }
        if let Some(i) = self.devices.iter().position(|x| check_device(x, Some(root)).is_ok_and(|o| o.device == d.device)) {
            let o = check_device(&self.devices[i], Some(root))?;
            if o.seq >= d.seq {
                return if self.devices[i] == *rec { Ok(d) } else { Err("an older device record".into()) };
            }
            self.devices[i] = rec.clone();
        } else {
            if self.devices.len() >= MAX_DEVICES {
                return Err(format!("A Passport has at most {MAX_DEVICES} devices. Remove one first."));
            }
            self.devices.push(rec.clone());
        }
        Ok(d)
    }

    /// A revocation taken in: the device's records at that seq or lower go.
    pub fn add_revoke(&mut self, rec: &Value, root: &str) -> Result<(), String> {
        let (_, dev, seq) = check_device_revoke(rec, Some(root))?;
        self.devices.retain(|x| check_device(x, Some(root)).map_or(false, |d| !(d.device == dev && d.seq <= seq)));
        self.revoked.retain(|x| check_device_revoke(x, Some(root)).map_or(false, |(_, k, s)| !(k == dev && s <= seq)));
        self.revoked.push(rec.clone());
        Ok(())
    }
}

/// This PC's device key: `<app>/passport/device.key` (random, owner-only, made once).
pub fn device_identity(app: &Path) -> Result<PeerIdentity, String> {
    let p = dir(app).join("device.key");
    if let Ok(t) = std::fs::read_to_string(&p) {
        return PeerIdentity::from_seed(unhex32(t.trim()).ok_or("This PC's device key doesn't look right.")?);
    }
    let seed: [u8; 32] = random()?;
    std::fs::create_dir_all(dir(app)).map_err(|e| e.to_string())?;
    write_secret(&p, (hex(&seed) + "\n").as_bytes())?;
    PeerIdentity::from_seed(seed)
}

/// This PC's device key seed, for a network endpoint bound to the device (not the root): iroh / pkarr publish under
/// the endpoint's own key, and peers map it to the Passport with [`verify_device_for`]. Never logged or shown.
pub fn device_secret_seed(app: &Path) -> Result<[u8; 32], String> {
    Ok(device_identity(app)?.secret_seed())
}

/// The Passport root (hex) a device key belongs to: a good device record naming `device_pk`, not revoked by any of
/// `revocations` at that seq or later.
pub fn verify_device_for(rec: &Value, device_pk: &str, revocations: &[Value]) -> Result<String, String> {
    let d = check_device(rec, None)?;
    if d.device != device_pk.to_ascii_lowercase() {
        return Err("that device record names another key".into());
    }
    if revocations.iter().filter_map(|r| check_device_revoke(r, Some(&d.root)).ok()).any(|(_, k, s)| k == d.device && s >= d.seq) {
        return Err("that device was removed from its Passport".into());
    }
    Ok(d.root)
}

/// This PC's own device record (signed by the root), for a peer to check with [`verify_device_for`].
pub fn this_device_record(app: &Path, root: &PeerIdentity) -> Result<Value, String> {
    let mut st = State::load(app, root);
    let dev = ensure_this_device(app, root, &mut st)?;
    st.save(app)?;
    st.devices.iter().find(|x| check_device(x, Some(&root.public_hex())).is_ok_and(|d| d.device == dev)).cloned().ok_or("no device record".into())
}

/// A short default name for this PC.
pub fn default_device_name() -> String {
    let n = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
    let n = clip(&n, 32);
    if n.is_empty() { "This PC".into() } else { n }
}

/// The state with this PC's own device record in it (made when missing).
pub fn ensure_this_device(app: &Path, root: &PeerIdentity, st: &mut State) -> Result<String, String> {
    let dev = device_identity(app)?;
    let r = root.public_hex();
    let has = st.devices.iter().any(|x| check_device(x, Some(&r)).is_ok_and(|d| d.device == dev.public_hex()));
    if !has {
        let t = now();
        let seq = st.next_seq(t);
        let rec = make_device(root, &dev.public_hex(), &default_device_name(), t, seq);
        st.add_device(&rec, &r)?;
    }
    Ok(dev.public_hex())
}

/// What the Passport page shows (never a key).
pub fn view(app: &Path, root: &PeerIdentity) -> Value {
    let mut st = State::load(app, root);
    let this = ensure_this_device(app, root, &mut st).unwrap_or_default();
    let _ = st.save(app);
    let r = root.public_hex();
    let devices: Vec<Value> = st
        .devices
        .iter()
        .filter_map(|x| check_device(x, Some(&r)).ok())
        .map(|d| json!({"id": d.id, "name": d.name, "created": d.created, "this": d.device == this}))
        .collect();
    let guardians: Vec<Value> = st
        .guardians
        .as_ref()
        .and_then(|g| check_guardians(g).ok())
        .map(|(_, l, _)| l.iter().map(|k| json!({"peer_id": identity::peer_id(&unhex(k).unwrap_or_default())})).collect())
        .unwrap_or_default();
    let guarding: Vec<Value> = st
        .guarding
        .iter()
        .filter_map(|g| check_guardians(&g.rec).ok())
        .map(|(root, _, _)| json!({"peer_id": identity::peer_id(&unhex(&root).unwrap_or_default())}))
        .collect();
    let alerts: Vec<Value> = st.alerts.iter().filter_map(|a| check_rotation(a).ok()).map(|k| json!({"id": k.id, "effective": k.effective})).collect();
    let recovering = recovery_load(app).map(|r| json!({"code": r.code, "approvals": r.approvals.len(), "needed": GUARDIANS_NEEDED}));
    json!({
        "peer_id": root.peer_id,
        "short": root.peer_id[..4].to_ascii_uppercase(),
        "name": st.name,
        "picture": st.picture,
        "setup_done": st.setup_done,
        "phrase_waiting": pending_phrase(app, root).is_some(),
        "phrase_words": if pending_phrase(app, root).is_some() { 12 } else { 24 },
        "devices": devices,
        "guardians": guardians,
        "guarding": guarding,
        "alerts": alerts,
        "recovering": recovering,
        "rotated": st.rotation.is_some(),
        "herokey": st.herokey.as_ref().and_then(|h| check_herokey_alias(h).ok()).map(|(_, id)| json!({"id": id})),
    })
}

// ------------------------------------------------------------------------------------------------ a second device by code

/// The new PC's side of a pairing (it shows the code).
pub struct PairNew {
    eph: [u8; 32],
    epk: [u8; 32],
    device: String,
    name: String,
    commit: Option<([u8; 32], String)>,
    peer_epk: Option<[u8; 32]>,
    pub code: Option<String>,
    pub made: u64,
}

/// The old PC's side (it has the Passport; the player types the code here).
pub struct PairOld {
    eph: [u8; 32],
    epk: [u8; 32],
    sid: String,
    peer_epk: Option<[u8; 32]>,
    peer_device: String,
    peer_name: String,
    code: Option<String>,
}

/// What the new PC gets: the root seed, the device records it should know, the Passport's name.
pub struct PairPayload {
    pub seed: [u8; 32],
    pub devices: Vec<Value>,
    pub name: String,
}

impl std::fmt::Debug for PairPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairPayload").field("devices", &self.devices.len()).finish_non_exhaustive()
    }
}

fn sas(epk_old: &[u8; 32], epk_new: &[u8; 32], device_new: &str) -> String {
    let mut m = format!("{PAIR_CTX}\n").into_bytes();
    m.extend_from_slice(epk_old);
    m.extend_from_slice(epk_new);
    m.extend_from_slice(device_new.as_bytes());
    let h = sha(&m);
    format!("{:06}", u32::from_be_bytes([h[0], h[1], h[2], h[3]]) % 1_000_000)
}

fn pair_key(eph: &[u8; 32], peer: &[u8; 32], epk_old: &[u8; 32], epk_new: &[u8; 32]) -> Result<[u8; 32], String> {
    let shared = identity::x25519(eph, peer);
    if shared == [0u8; 32] {
        return Err("bad key".into());
    }
    let mut info = epk_old.to_vec();
    info.extend_from_slice(epk_new);
    Ok(hkdf32(PAIR_CTX.as_bytes(), &shared, &info))
}

fn epk_of(v: &Value, k: &str) -> Option<[u8; 32]> {
    hex64(v.get(k)).and_then(|h| unhex32(&h))
}

impl PairNew {
    pub fn new(device_pk: &str, name: &str, t: u64) -> Result<PairNew, String> {
        let eph: [u8; 32] = random()?;
        Ok(PairNew { eph, epk: identity::x25519_base(&eph), device: device_pk.into(), name: clip(name, 32), commit: None, peer_epk: None, code: None, made: t })
    }

    /// Step 1: the old PC's commitment; the answer is this PC's exchange key and device key.
    pub fn on_commit(&mut self, m: &Value) -> Result<Value, String> {
        if self.commit.is_some() {
            return Err("busy".into());
        }
        let c = hex64(m.get("c")).and_then(|h| unhex32(&h)).ok_or("commit")?;
        let sid = m.get("sid").and_then(Value::as_str).filter(|s| (8..=32).contains(&s.len())).ok_or("sid")?;
        self.commit = Some((c, sid.to_string()));
        Ok(json!({"ok": true, "epk": hex(&self.epk), "device": self.device, "name": self.name}))
    }

    /// Step 2: the old PC's exchange key, checked against its commitment; the code to show is known now.
    pub fn on_reveal(&mut self, m: &Value) -> Result<Value, String> {
        let (c, sid) = self.commit.clone().ok_or("no commit")?;
        if m.get("sid").and_then(Value::as_str) != Some(sid.as_str()) || self.peer_epk.is_some() {
            return Err("sid".into());
        }
        let e = epk_of(m, "epk").ok_or("epk")?;
        if !same(&sha(&e), &c) {
            return Err("commit".into());
        }
        self.peer_epk = Some(e);
        self.code = Some(sas(&e, &self.epk, &self.device));
        Ok(json!({"ok": true}))
    }

    /// Step 3: the sealed Passport, opened.
    pub fn on_seal(&mut self, m: &Value) -> Result<PairPayload, String> {
        let (_, sid) = self.commit.clone().ok_or("no commit")?;
        let pe = self.peer_epk.ok_or("no reveal")?;
        if m.get("sid").and_then(Value::as_str) != Some(sid.as_str()) {
            return Err("sid".into());
        }
        let b = m.get("box").and_then(Value::as_str).and_then(unhex).ok_or("box")?;
        let k = pair_key(&self.eph, &pe, &pe, &self.epk)?;
        let plain = aead_open(&k, PAIR_CTX.as_bytes(), &b).ok_or("That didn't open: start again on both PCs.")?;
        payload_of(&plain)
    }
}

fn payload_of(plain: &[u8]) -> Result<PairPayload, String> {
    let v: Value = serde_json::from_slice(plain).map_err(|_| "That didn't read.".to_string())?;
    let seed = v["seed"].as_str().and_then(unhex32).ok_or("That didn't read.")?;
    let root = PeerIdentity::from_seed(seed)?;
    let devices = v["devices"].as_array().map(|a| a.iter().filter(|d| check_device(d, Some(&root.public_hex())).is_ok()).cloned().collect()).unwrap_or_default();
    Ok(PairPayload { seed, devices, name: clip(v["name"].as_str().unwrap_or(""), 32) })
}

impl PairOld {
    /// Step 1 out: the commitment to this PC's exchange key.
    pub fn begin() -> Result<(PairOld, Value), String> {
        let eph: [u8; 32] = random()?;
        let epk = identity::x25519_base(&eph);
        let sid = identity::random_b32(10)?;
        let m = json!({"pair": "commit", "sid": sid, "c": hex(&sha(&epk))});
        Ok((PairOld { eph, epk, sid, peer_epk: None, peer_device: String::new(), peer_name: String::new(), code: None }, m))
    }

    /// Step 1's answer in, step 2 out (the reveal). The code is known here now too.
    pub fn on_answer(&mut self, a: &Value) -> Result<Value, String> {
        if a["ok"] != true {
            return Err("The other PC isn't waiting to join (or another one answered first).".into());
        }
        let e = epk_of(a, "epk").ok_or("answer")?;
        let dev = hex64(a.get("device")).ok_or("answer")?;
        self.peer_epk = Some(e);
        self.peer_name = clip(a["name"].as_str().unwrap_or(""), 32);
        self.code = Some(sas(&self.epk, &e, &dev));
        self.peer_device = dev;
        Ok(json!({"pair": "reveal", "sid": self.sid, "epk": hex(&self.epk)}))
    }

    pub fn peer_name(&self) -> &str {
        &self.peer_name
    }

    /// The code typed: when it matches, step 3 (the Passport sealed to the exchange, and the new PC's device record).
    pub fn confirm(&self, typed: &str, root: &PeerIdentity, st: &mut State) -> Result<(Value, Value), String> {
        let code = self.code.as_ref().ok_or("Not ready yet.")?;
        let typed: String = typed.chars().filter(|c| c.is_ascii_digit()).collect();
        if !same(typed.as_bytes(), code.as_bytes()) {
            return Err("That code doesn't match the one on your other PC. Nothing was sent. Start again on both PCs.".into());
        }
        let pe = self.peer_epk.ok_or("Not ready yet.")?;
        let t = now();
        let seq = st.next_seq(t);
        let name = if self.peer_name.is_empty() { "Another PC" } else { self.peer_name.as_str() };
        let rec = make_device(root, &self.peer_device, name, t, seq);
        st.add_device(&rec, &root.public_hex())?;
        let plain = json!({"seed": hex(&root.secret_seed()), "devices": st.devices, "name": st.name});
        let k = pair_key(&self.eph, &pe, &self.epk, &pe)?;
        let b = aead_seal(&k, PAIR_CTX.as_bytes(), canonical(&plain).as_bytes())?;
        Ok((json!({"pair": "seal", "sid": self.sid, "box": hex(&b)}), rec))
    }
}

/// The new PC's pairing while it waits (one at a time), and what happened last.
static PAIR_NEW: std::sync::Mutex<Option<PairNew>> = std::sync::Mutex::new(None);
static PAIR_DONE: std::sync::Mutex<Option<Result<String, String>>> = std::sync::Mutex::new(None);

/// "Add this PC to my Passport": this PC waits for the other one (friendnet's listener carries the messages).
pub fn pair_wait(app: &Path) -> Result<(), String> {
    let dev = device_identity(app)?;
    *PAIR_NEW.lock().map_err(|_| "busy")? = Some(PairNew::new(&dev.public_hex(), &default_device_name(), now())?);
    *PAIR_DONE.lock().map_err(|_| "busy")? = None;
    Ok(())
}

pub fn pair_cancel() {
    if let Ok(mut g) = PAIR_NEW.lock() {
        *g = None;
    }
}

/// The new PC's pairing now: "waiting", "show" (with the code), "done" / "failed" (with words).
pub fn pair_status() -> Value {
    if let Some(r) = PAIR_DONE.lock().ok().and_then(|g| g.clone()) {
        return match r {
            Ok(m) => json!({"state": "done", "message": m}),
            Err(e) => json!({"state": "failed", "message": e}),
        };
    }
    let g = PAIR_NEW.lock().ok();
    match g.as_ref().and_then(|g| g.as_ref()) {
        Some(p) if now().saturating_sub(p.made) > PAIR_TTL => json!({"state": "failed", "message": "Time ran out. Start again on both PCs."}),
        Some(p) => match &p.code {
            Some(c) => json!({"state": "show", "code": c}),
            None => json!({"state": "waiting"}),
        },
        None => json!({"state": "off"}),
    }
}

/// One pairing message to this PC (friendnet.rs's listener hands over any line with a "pair" field). Strangers get
/// "refused" and nothing else.
pub fn pair_serve(app: &Path, m: &Value) -> Value {
    let refused = json!({"ok": false, "error": "refused"});
    let Ok(mut g) = PAIR_NEW.lock() else { return refused };
    let Some(p) = g.as_mut() else { return refused };
    if now().saturating_sub(p.made) > PAIR_TTL {
        *g = None;
        return refused;
    }
    let r = match m.get("pair").and_then(Value::as_str) {
        Some("commit") => p.on_commit(m),
        Some("reveal") => p.on_reveal(m),
        Some("seal") => match p.on_seal(m).and_then(|pl| install_payload(app, &pl)) {
            Ok(msg) => {
                *g = None;
                if let Ok(mut d) = PAIR_DONE.lock() {
                    *d = Some(Ok(msg));
                }
                Ok(json!({"ok": true}))
            }
            Err(e) => {
                *g = None;
                if let Ok(mut d) = PAIR_DONE.lock() {
                    *d = Some(Err(e));
                }
                Err("refused".into())
            }
        },
        _ => Err("refused".into()),
    };
    r.unwrap_or(refused)
}

/// The Passport from another PC made this PC's (the key it had kept aside), with its device records.
pub fn install_payload(app: &Path, p: &PairPayload) -> Result<String, String> {
    let root = PeerIdentity::from_seed(p.seed)?;
    let short = identity::import_seed(app, p.seed)?;
    let mut st = State::load(app, &root);
    for d in &p.devices {
        let _ = st.add_device(d, &root.public_hex());
    }
    if st.name.is_empty() {
        st.name = p.name.clone();
    }
    st.setup_done = true;
    ensure_this_device(app, &root, &mut st)?;
    st.save(app)?;
    let _ = phrase_saved(app);
    Ok(format!("Done: this PC is Hero {short}, your Passport. Restart Leyline to finish."))
}

/// One pairing message to the new PC at `addr`: a JSON line out, a JSON line back.
pub fn pair_send(addr: std::net::SocketAddr, m: &Value) -> Result<Value, String> {
    use std::io::{BufRead, BufReader, Read, Write};
    let mut s = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(2)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    s.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    s.write_all(format!("{m}\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(s.take(4096)).read_line(&mut line).map_err(|e| e.to_string())?;
    serde_json::from_str(line.trim()).map_err(|_| "no answer".to_string())
}

/// The old PC's pairing in progress (one at a time) and where the new PC is.
static PAIR_OLD: std::sync::Mutex<Option<(PairOld, std::net::SocketAddr, u64)>> = std::sync::Mutex::new(None);

/// "Add a device" on the PC that has the Passport: the first of `addrs` (people near on this network, or an address
/// typed) that is waiting to join answers; both PCs know the code now. The new PC's name.
pub fn pair_find(addrs: &[std::net::SocketAddr]) -> Result<String, String> {
    for a in addrs {
        let Ok((mut old, m1)) = PairOld::begin() else { continue };
        let Ok(ans) = pair_send(*a, &m1) else { continue };
        if ans["ok"] != true {
            continue;
        }
        let m2 = old.on_answer(&ans)?;
        let r = pair_send(*a, &m2)?;
        if r["ok"] != true {
            return Err("The other PC stopped. Start again on both PCs.".into());
        }
        let name = old.peer_name().to_string();
        *PAIR_OLD.lock().map_err(|_| "busy")? = Some((old, *a, now()));
        return Ok(if name.is_empty() { "your other PC".into() } else { name });
    }
    Err("No PC on your network is waiting to join. On the new PC, open Passport and pick Add this PC, then try again.".into())
}

/// The code from the new PC's screen typed here: when it matches, the Passport goes over, sealed. Nothing is sent
/// when it doesn't (the pairing ends; start again on both).
pub fn pair_confirm(app: &Path, root: &PeerIdentity, code: &str) -> Result<String, String> {
    let (old, addr, at) = PAIR_OLD.lock().map_err(|_| "busy")?.take().ok_or("Start with Add a device.")?;
    if now().saturating_sub(at) > PAIR_TTL {
        return Err("Time ran out. Start again on both PCs.".into());
    }
    let mut st = State::load(app, root);
    let before = st.clone();
    let (m3, _) = old.confirm(code, root, &mut st)?;
    let r = pair_send(addr, &m3)?;
    if r["ok"] != true {
        let _ = before.save(app);
        return Err("Your other PC didn't take it. Start again on both PCs.".into());
    }
    st.save(app)?;
    Ok(format!("Done: {} is part of your Passport now.", if old.peer_name().is_empty() { "your other PC" } else { old.peer_name() }))
}

// ------------------------------------------------------------------------------------------------ a second device by file

fn file_key(words: &str, salt: &[u8], rounds: u32) -> [u8; 32] {
    let norm: Vec<String> = words.split(|c: char| !c.is_ascii_alphabetic()).filter(|t| !t.is_empty()).map(|t| t.to_ascii_lowercase()).collect();
    let mut out = [0u8; 32];
    ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA256, std::num::NonZeroU32::new(rounds.max(1)).expect("non-zero"), salt, norm.join(" ").as_bytes(), &mut out);
    out
}

pub const FILE_HEAD: &str = "# Leyline Passport for your other PC. It only opens with the 6 words Leyline showed you. Delete this file once your other PC has it.";

fn file_export_with(root: &PeerIdentity, st: &State, rounds: u32) -> Result<(String, String), String> {
    let pick: [u8; 12] = random()?;
    let w = words();
    let six: Vec<&str> = pick.chunks(2).map(|c| w[(u16::from_be_bytes([c[0], c[1]]) as usize) % 2048]).collect();
    let six = six.join(" ");
    let salt: [u8; 16] = random()?;
    let plain = json!({"seed": hex(&root.secret_seed()), "devices": st.devices, "name": st.name});
    let b = aead_seal(&file_key(&six, &salt, rounds), FILE_CTX.as_bytes(), canonical(&plain).as_bytes())?;
    let mut raw = salt.to_vec();
    raw.extend(b);
    Ok((format!("{FILE_HEAD}\nLPF1.{}\n", identity::b32(&raw)), six))
}

/// The sealed file for another PC, and the 6 words that open it (shown once on this PC).
pub fn file_export(root: &PeerIdentity, st: &State) -> Result<(String, String), String> {
    file_export_with(root, st, FILE_ROUNDS)
}

fn file_import_with(text: &str, six: &str, rounds: u32) -> Result<PairPayload, String> {
    let line = text.lines().map(str::trim).find(|l| l.starts_with("LPF1.")).ok_or("That isn't a Leyline Passport file.")?;
    let raw = identity::unb32(&line[5..]).filter(|r| r.len() > 16 + 16).ok_or("That Passport file is damaged.")?;
    let plain = aead_open(&file_key(six, &raw[..16], rounds), FILE_CTX.as_bytes(), &raw[16..]).ok_or("Those words don't open that file. Check them on your other PC.")?;
    payload_of(&plain)
}

/// A sealed file opened with its 6 words.
pub fn file_import(text: &str, six: &str) -> Result<PairPayload, String> {
    file_import_with(text, six, FILE_ROUNDS)
}

// ------------------------------------------------------------------------------------------------ recovery

/// A recovery in progress on this PC: the new root's 12-word entropy (secret, owner-only), the code for the guardians
/// and the approvals pasted so far.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Recovery {
    pub entropy: String,
    pub code: String,
    pub approvals: Vec<String>,
}

impl std::fmt::Debug for Recovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recovery").field("code", &self.code).field("approvals", &self.approvals.len()).finish_non_exhaustive()
    }
}

fn recovery_path(app: &Path) -> PathBuf {
    dir(app).join("recovery.pending")
}

pub fn recovery_load(app: &Path) -> Option<Recovery> {
    std::fs::read_to_string(recovery_path(app)).ok().and_then(|t| serde_json::from_str(&t).ok())
}

impl Recovery {
    fn root(&self) -> Result<PeerIdentity, String> {
        let e: [u8; 16] = unhex(&self.entropy).and_then(|v| v.try_into().ok()).ok_or("recovery")?;
        PeerIdentity::from_seed(seed_from_entropy(&e))
    }
}

/// "Ask 2 guardians": a new Passport key is made (from new 12 words) and its code shown to the guardians.
pub fn recovery_start(app: &Path) -> Result<String, String> {
    if let Some(r) = recovery_load(app) {
        return Ok(r.code);
    }
    let e: [u8; 16] = random()?;
    let root = PeerIdentity::from_seed(seed_from_entropy(&e))?;
    let r = Recovery { entropy: hex(&e), code: recovery_code(&root), approvals: Vec::new() };
    std::fs::create_dir_all(dir(app)).map_err(|e| e.to_string())?;
    write_secret(&recovery_path(app), serde_json::to_string(&r).map_err(|e| e.to_string())?.as_bytes())?;
    Ok(r.code)
}

pub fn recovery_cancel(app: &Path) {
    let _ = std::fs::remove_file(recovery_path(app));
}

/// A guardian's answer pasted. Ok(None): kept, more needed. Ok(Some((rotation, roster))): 2 guardians agree, this
/// PC's key is the new Passport now (the old one kept aside), and the new 12 words wait for the phrase screen.
pub fn recovery_add(app: &Path, text: &str) -> Result<Option<(Value, Vec<String>)>, String> {
    let mut r = recovery_load(app).ok_or("Start with Ask 2 guardians.")?;
    let new_root = r.root()?;
    let (a, g, _) = parse_approval_text(text)?;
    let (old, _, _) = check_guardians(&g)?;
    // the approval has to be a listed guardian's good signature for this PC's new key
    let list = check_guardians(&g)?.1;
    let ts = a["ts"].as_u64().unwrap_or(0);
    let g_pk = hex64(a.get("guardian")).ok_or("That guardian's answer didn't read.")?;
    if !list.contains(&g_pk) || !verify_doc(&g_pk, &approval_doc(&old, &new_root.public_hex(), ts), CTX_ROTATE, a["sig"].as_str().unwrap_or("")) {
        return Err("That answer is for another recovery code. Give your guardian the code shown here.".into());
    }
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if !r.approvals.contains(&t) {
        r.approvals.push(t);
    }
    // the answers that agree with this one (the same old Passport)
    let mut approvals = Vec::new();
    let mut roster: Vec<String> = Vec::new();
    let mut guardians = g.clone();
    for x in &r.approvals {
        if let Ok((a2, g2, ro)) = parse_approval_text(x) {
            if check_guardians(&g2).is_ok_and(|(o, _, _)| o == old) {
                if check_guardians(&g2).map(|k| k.2).unwrap_or(0) > check_guardians(&guardians).map(|k| k.2).unwrap_or(0) {
                    guardians = g2.clone();
                }
                approvals.push(a2);
                for k in ro {
                    if !roster.contains(&k) {
                        roster.push(k);
                    }
                }
            }
        }
    }
    write_secret(&recovery_path(app), serde_json::to_string(&r).map_err(|e| e.to_string())?.as_bytes())?;
    let Ok(bundle) = rotation_bundle(&guardians, &new_root.public_hex(), &approvals) else { return Ok(None) };
    // two guardians agree: this PC becomes the new Passport
    let e: [u8; 16] = unhex(&r.entropy).and_then(|v| v.try_into().ok()).ok_or("recovery")?;
    identity::import_seed(app, new_root.secret_seed())?;
    keep_pending_phrase(app, &e)?;
    let mut st = State::load(app, &new_root);
    st.rotation = Some(bundle.clone());
    st.setup_done = false;
    ensure_this_device(app, &new_root, &mut st)?;
    st.save(app)?;
    recovery_cancel(app);
    if let Some(gl) = check_guardians(&guardians).ok().map(|k| k.1) {
        for k in gl {
            if !roster.contains(&k) {
                roster.push(k);
            }
        }
    }
    Ok(Some((bundle, roster)))
}

/// Someone is moving MY root with my guardians (a rotate message reached this key): kept for the page, which offers
/// to stop it ([`make_veto`]).
pub fn keep_alert(app: &Path, me: &PeerIdentity, rot: &Value) -> Result<(), String> {
    let r = check_rotation(rot)?;
    if r.old != me.public_hex() {
        return Err("not my key".into());
    }
    let mut st = State::load(app, me);
    if !st.alerts.iter().any(|a| rotation_id(a) == r.id) {
        st.alerts.insert(0, rot.clone());
    }
    st.save(app)
}

/// A friend's guardians list naming me (sent by them), kept so I can approve their recovery later.
pub fn keep_guarding(app: &Path, me: &PeerIdentity, from: &str, rec: &Value, roster: &[String]) -> Result<(), String> {
    let (root, list, seq) = check_guardians(rec)?;
    if root != from {
        return Err("a guardians list sent by someone else".into());
    }
    let mut st = State::load(app, me);
    if let Some(i) = st.guarding.iter().position(|g| check_guardians(&g.rec).is_ok_and(|(r, _, _)| r == root)) {
        if check_guardians(&st.guarding[i].rec).map(|k| k.2).unwrap_or(0) > seq {
            return Ok(());
        }
        st.guarding.remove(i);
    }
    if list.contains(&me.public_hex()) {
        let roster = roster.iter().filter(|k| hex64(Some(&json!(k))).is_some()).take(200).cloned().collect();
        st.guarding.insert(0, Guarding { rec: rec.clone(), roster });
    }
    st.save(app)
}

// ------------------------------------------------------------------------------------------------ with the friends' hub

use crate::p2p::friendnet::Hub;

fn name_in(view: &Value, pid: &str) -> String {
    view["friends"].as_array().into_iter().flatten().find(|f| f["peer_id"] == pid).and_then(|f| f["name"].as_str()).filter(|n| !n.is_empty()).map(String::from).unwrap_or_else(|| format!("Hero {}", pid.get(..4).unwrap_or("").to_ascii_uppercase()))
}

/// The Passport page: [`view`] with friends' names on guardians and on the friends who chose me, and the friends
/// that can be picked as guardians.
pub fn page(h: &Hub) -> Value {
    let mut v = view(&h.app, &h.me);
    let fv = h.book.lock().map(|b| b.view(&h.me, now(), &[], 600)).unwrap_or_default();
    for k in ["guardians", "guarding"] {
        if let Some(a) = v[k].as_array_mut() {
            for g in a {
                let pid = g["peer_id"].as_str().unwrap_or("").to_string();
                g["name"] = json!(name_in(&fv, &pid));
            }
        }
    }
    v["friends"] = json!(fv["friends"].as_array().into_iter().flatten().map(|f| json!({"peer_id": f["peer_id"], "name": name_in(&fv, f["peer_id"].as_str().unwrap_or(""))})).collect::<Vec<_>>());
    v["near"] = json!(h.near_list(now()).len());
    v
}

/// The setup screen: name and picture kept; the phrase to show (12 words for a new key, 24 for an older one).
pub fn setup(h: &Hub, name: &str, picture: &str) -> Result<Vec<String>, String> {
    let mut st = State::load(&h.app, &h.me);
    st.name = clip(name, 32);
    st.picture = clip(picture, 64);
    ensure_this_device(&h.app, &h.me, &mut st)?;
    st.save(&h.app)?;
    if !st.name.is_empty() {
        if let Ok(mut b) = h.book.lock() {
            if b.profile.name.trim().is_empty() {
                b.profile.name = st.name.clone();
            }
        }
        let _ = h.save();
    }
    Ok(phrase_for(&h.app, &h.me).split(' ').map(String::from).collect())
}

/// "I saved it" and Done: the waiting words deleted, setup finished.
pub fn finish(h: &Hub) -> Result<(), String> {
    phrase_saved(&h.app)?;
    let mut st = State::load(&h.app, &h.me);
    st.setup_done = true;
    st.save(&h.app)
}

/// Guardians chosen from friends (2 to 5, by peer id): the list signed and sent to each, with my friends' keys so a
/// recovery can tell them.
pub fn set_guardians(h: &Hub, peer_ids: &[String]) -> Result<String, String> {
    let (keys, roster) = {
        let b = h.book.lock().map_err(|_| "busy")?;
        let keys: Vec<String> = peer_ids.iter().map(|p| b.friend_key(&h.me, p).ok_or("A guardian has to be on your friends list.".to_string())).collect::<Result<_, _>>()?;
        (keys, b.keys(&h.me))
    };
    let mut st = State::load(&h.app, &h.me);
    let t = now();
    let seq = st.next_seq(t);
    let rec = make_guardians(&h.me, &keys, t, seq)?;
    st.guardians = Some(rec.clone());
    st.save(&h.app)?;
    let mut sent = 0;
    for k in &keys {
        if let Ok(m) = crate::p2p::friends::make_msg(&h.me, k, "guardian", json!({"guardians": rec, "roster": roster}), t) {
            if h.deliver(k, &m, None).is_ok() {
                sent += 1;
            }
        }
    }
    Ok(if sent == keys.len() {
        format!("Your {} guardians are set, and their launchers have the list.", keys.len())
    } else {
        format!("Your {} guardians are set. {} of them get the list when you're both online.", keys.len(), keys.len() - sent)
    })
}

/// A device taken off this Passport.
pub fn revoke_device(app: &Path, root: &PeerIdentity, id: &str) -> Result<String, String> {
    let mut st = State::load(app, root);
    let r = root.public_hex();
    let d = st.devices.iter().filter_map(|x| check_device(x, Some(&r)).ok()).find(|d| d.id == id).ok_or("That device isn't on your Passport.")?;
    if d.device == device_identity(app)?.public_hex() {
        return Err("This is the PC you're on. Remove it from another one.".into());
    }
    let t = now();
    let seq = st.next_seq(t).max(d.seq + 1);
    st.add_revoke(&make_device_revoke(root, &d.device, t, seq), &r)?;
    st.save(app)?;
    Ok(format!("{} is off your Passport. If it was lost, also make a new phrase on a PC you trust.", d.name))
}

/// A guardian helps: the code from the friend's new PC and which friend it is; the answer text to send back.
pub fn help_friend(h: &Hub, code: &str, peer_id: &str) -> Result<String, String> {
    let new_pk = parse_recovery_code(code)?;
    let st = State::load(&h.app, &h.me);
    let g = st.guarding.iter().find(|g| check_guardians(&g.rec).is_ok_and(|(r, _, _)| identity::peer_id(&unhex(&r).unwrap_or_default()) == peer_id)).ok_or("That friend hasn't named you a guardian.")?;
    guardian_approval_text(&h.me, &g.rec, &g.roster, &new_pk, now())
}

/// After a guardians recovery (this hub is the new key's): each friend of the old key is asked to keep the
/// friendship with the new one, and the old key is told (so a device that still has it can stop a theft).
pub fn announce_rotation(h: &Hub, rotation: &Value, roster: &[String]) -> usize {
    let t = now();
    let r = match check_rotation(rotation) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    let mut n = 0;
    for k in roster.iter().filter(|k| **k != h.me.public_hex() && **k != r.old) {
        let m = h.book.lock().ok().and_then(|mut b| b.rotate_request(&h.me, k, rotation, t).ok());
        if let Some(m) = m {
            if h.deliver(k, &m, None).is_ok() {
                n += 1;
            }
        }
    }
    let _ = h.save();
    if let Ok(m) = crate::p2p::friends::make_msg(&h.me, &r.old, "rotate", json!({"rotation": rotation}), t) {
        let _ = h.deliver(&r.old, &m, None);
    }
    n
}

/// "That isn't me": a rotation of my key stopped; the veto goes to every friend.
pub fn veto(h: &Hub, rotation_id: &str) -> Result<String, String> {
    let mut st = State::load(&h.app, &h.me);
    let i = st.alerts.iter().position(|a| rotation_id == self::rotation_id(a)).ok_or("Nothing to stop.")?;
    st.alerts.remove(i);
    let v = make_veto(&h.me, rotation_id, now());
    st.vetoes.insert(0, v.clone());
    st.save(&h.app)?;
    let keys = h.book.lock().map(|b| b.keys(&h.me)).unwrap_or_default();
    let mut n = 0;
    for k in &keys {
        if let Ok(m) = crate::p2p::friends::make_msg(&h.me, k, "rotate", json!({"veto": v}), now()) {
            if h.deliver(k, &m, None).is_ok() {
                n += 1;
            }
        }
    }
    Ok(format!("Stopped. {n} of your {} friends heard now; the rest hear when you're both online.", keys.len()))
}

/// The old Hero Key file's text imported once, as an alias.
pub fn import_herokey(app: &Path, root: &PeerIdentity, text: &str) -> Result<String, String> {
    let mut st = State::load(app, root);
    if st.herokey.is_some() {
        return Err("Your Passport already has your old Hero Key.".into());
    }
    let rec = make_herokey_alias(root, text, now())?;
    let id = check_herokey_alias(&rec)?.1;
    st.herokey = Some(rec);
    st.save(app)?;
    Ok(format!("Done: your old Hero Key ({id}) is part of your Passport now. You can delete the file."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> PeerIdentity {
        PeerIdentity::from_seed([n; 32]).unwrap()
    }

    #[test]
    fn bip39_vectors_and_round_trips() {
        assert_eq!(words().len(), 2048);
        assert_eq!(words()[0], "abandon");
        assert_eq!(words()[2047], "zoo");
        assert!(words().windows(2).all(|w| w[0] < w[1]), "sorted, so binary search works");
        // the BIP-39 reference vectors (trezor/python-mnemonic vectors.json)
        assert_eq!(phrase_from_entropy(&[0u8; 16]), "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        assert_eq!(phrase_from_entropy(&[0x7f; 16]), "legal winner thank year wave sausage worth useful legal winner thank yellow");
        assert_eq!(phrase_from_entropy(&[0x80; 16]), "letter advice cage absurd amount doctor acoustic avoid letter advice cage above");
        assert_eq!(phrase_from_entropy(&[0xff; 16]), "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong");
        assert_eq!(
            phrase_from_entropy(&[0u8; 32]),
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art"
        );
        for n in 0..20u8 {
            let e: Vec<u8> = (0..16).map(|i| i * 13 + n).collect();
            assert_eq!(entropy_from_phrase(&phrase_from_entropy(&e)).unwrap(), e);
            let e32: Vec<u8> = (0..32).map(|i| i * 7 + n).collect();
            assert_eq!(entropy_from_phrase(&phrase_from_entropy(&e32)).unwrap(), e32);
        }
        // any case, any spacing, the first four letters
        assert_eq!(entropy_from_phrase("  LEGAL winner, thank year\nwave saus worth usef legal winn thank yell ").unwrap(), vec![0x7f; 16]);
        // wrong checksum, a word that isn't one, the wrong count: refused in plain words
        assert!(entropy_from_phrase("legal winner thank year wave sausage worth useful legal winner thank thank").unwrap_err().contains("don't fit"));
        assert!(entropy_from_phrase("legal winner thank year wave sausage worth useful legal winner thank xyzzy").unwrap_err().contains("Word 12"));
        assert!(entropy_from_phrase("legal winner").unwrap_err().contains("12 or 24"));
    }

    #[test]
    fn the_phrase_restores_the_same_root() {
        let e = [9u8; 16];
        let seed = seed_from_entropy(&e);
        assert_eq!(seed_from_phrase(&phrase_from_entropy(&e)).unwrap(), seed);
        assert_ne!(seed, [9u8; 32]);
        // a key made before the Passport: its 24 words are the seed
        let old = id(42);
        assert_eq!(seed_from_phrase(&phrase_from_entropy(&old.secret_seed())).unwrap(), old.secret_seed());
        // a new launcher's key is made from its 12 words, which wait until seen, then go
        let d = tempfile::tempdir().unwrap();
        let me = identity::load_or_create(d.path()).unwrap();
        let p = pending_phrase(d.path(), &me).expect("the 12 words wait");
        assert_eq!(p.split(' ').count(), 12);
        assert_eq!(PeerIdentity::from_seed(seed_from_phrase(&p).unwrap()).unwrap().peer_id, me.peer_id);
        assert_eq!(phrase_for(d.path(), &me), p);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(pending_path(d.path())).unwrap().permissions().mode() & 0o777, 0o600);
        }
        phrase_saved(d.path()).unwrap();
        assert!(pending_phrase(d.path(), &me).is_none());
        let p24 = phrase_for(d.path(), &me);
        assert_eq!(p24.split(' ').count(), 24, "afterwards the 24-word phrase still backs it up");
        assert_eq!(PeerIdentity::from_seed(seed_from_phrase(&p24).unwrap()).unwrap().peer_id, me.peer_id);
        // a pending file of another key is never shown for this one
        keep_pending_phrase(d.path(), &[1; 16]).unwrap();
        assert!(pending_phrase(d.path(), &me).is_none());
        // restoring on another PC: the same root
        let d2 = tempfile::tempdir().unwrap();
        identity::import_seed(d2.path(), seed_from_phrase(&p).unwrap()).unwrap();
        assert_eq!(identity::load_or_create(d2.path()).unwrap().peer_id, me.peer_id);
    }

    #[test]
    fn device_records_sign_tamper_replay_and_revoke() {
        let root = id(1);
        let other = id(2);
        let dev = id(3).public_hex();
        let r = root.public_hex();
        let rec = make_device(&root, &dev, "Desk PC", 1000, 5);
        let d = check_device(&rec, Some(&r)).unwrap();
        assert_eq!((d.name.as_str(), d.seq, d.id.as_str()), ("Desk PC", 5, id(3).peer_id.as_str()));
        // tamper, wrong key, another Passport
        let mut bad = rec.clone();
        bad["doc"]["name"] = json!("Evil");
        assert!(check_device(&bad, Some(&r)).is_err());
        let forged = json!({"doc": rec["doc"], "sig": other.sign_doc(CTX_DEVICE, &rec["doc"])});
        assert!(check_device(&forged, Some(&r)).is_err(), "signed by a key that isn't the root");
        assert!(check_device(&rec, Some(&other.public_hex())).is_err());
        let mut wrong_ctx = rec.clone();
        wrong_ctx["sig"] = json!(root.sign_doc(CTX_DEVICE_REVOKE, &rec["doc"]));
        assert!(check_device(&wrong_ctx, Some(&r)).is_err(), "another record type's signature");
        // the state: newer replaces, older refused, revoked stays out
        let mut st = State::default();
        st.add_device(&rec, &r).unwrap();
        assert!(st.add_device(&make_device(&root, &dev, "Old name", 900, 4), &r).is_err(), "an older record");
        st.add_device(&make_device(&root, &dev, "New name", 1100, 6), &r).unwrap();
        assert_eq!(st.devices.len(), 1);
        assert_eq!(check_device(&st.devices[0], None).unwrap().name, "New name");
        let rv = make_device_revoke(&root, &dev, 1200, 7);
        assert_eq!(check_device_revoke(&rv, Some(&r)).unwrap().2, 7);
        let mut badrv = rv.clone();
        badrv["doc"]["seq"] = json!(99);
        assert!(check_device_revoke(&badrv, Some(&r)).is_err());
        st.add_revoke(&rv, &r).unwrap();
        assert!(st.devices.is_empty());
        assert!(st.add_device(&rec, &r).is_err(), "a revoked device's old record can't be played back in");
        assert!(st.add_device(&make_device(&root, &dev, "x", 1150, 7), &r).is_err(), "nor one at the same seq");
        st.add_device(&make_device(&root, &dev, "Back", 1300, 8), &r).unwrap();
        // a peer maps a device key to its Passport (the connections layer binds endpoints to device keys)
        let back = make_device(&root, &dev, "Back", 1300, 8);
        assert_eq!(verify_device_for(&back, &dev, &[]).unwrap(), r);
        assert!(verify_device_for(&back, &id(9).public_hex(), &[]).is_err(), "another device key");
        assert!(verify_device_for(&rec, &dev, &[rv.clone()]).is_err(), "revoked at that seq");
        assert!(verify_device_for(&back, &dev, &[rv.clone()]).is_ok(), "a newer record after the revocation");
        // the state re-checked on load: another root's records dropped
        let mut s2 = st.clone();
        s2.devices.push(make_device(&other, &dev, "x", 1, 1));
        assert_eq!(s2.checked(&root).devices.len(), 1);
    }

    #[test]
    fn world_keys_differ_per_world_and_never_link() {
        let root = id(7);
        let a = world_identity(&root, "kzdvvj2umnduyauf");
        let b = world_identity(&root, "aaaaaaaaaaaaaaaa");
        assert_ne!(a.public_key, b.public_key);
        assert_ne!(a.public_key, root.public_key);
        assert_eq!(a.public_key, world_identity(&root, "kzdvvj2umnduyauf").public_key, "the same every time");
        assert_ne!(a.public_key, world_identity(&id(8), "kzdvvj2umnduyauf").public_key);
        // the login name on that world comes from the world key's peer id
        let n = crate::host::worldfriends::name_for("kzdvvj2umnduyauf", &a.peer_id);
        assert_ne!(n, crate::host::worldfriends::name_for("kzdvvj2umnduyauf", &root.peer_id));
    }

    fn realm() -> PeerIdentity {
        id(20)
    }

    #[test]
    fn login_agreement_both_sign_both_keep() {
        let host = realm();
        let rid = host.peer_id.clone();
        let player = world_identity(&id(21), &rid);
        let doc = login_doc(&player.public_hex(), &rid, "GABCDEFGHJKLMNP2", 1000, 1);
        let half = login_half(&host, &doc);
        let rec = login_countersign(&player, &half, &rid, "GABCDEFGHJKLMNP2").unwrap();
        let n = check_login(&rec).unwrap();
        assert_eq!((n.realm.as_str(), n.account.as_str(), n.seq), (rid.as_str(), "GABCDEFGHJKLMNP2", 1));
        // the player won't sign another world's, another key's or another account's record
        assert!(login_countersign(&player, &half, "aaaaaaaaaaaaaaaa", "GABCDEFGHJKLMNP2").is_err());
        assert!(login_countersign(&player, &half, &rid, "GOTHER2345").is_err());
        assert!(login_countersign(&id(22), &half, &rid, "GABCDEFGHJKLMNP2").is_err());
        // one signature alone, tampered, signed by a key that isn't the realm's
        assert!(check_login(&half).is_err());
        let mut t = rec.clone();
        t["doc"]["account"] = json!("GZZZZZZZZZZZZZZZ");
        assert!(check_login(&t).is_err());
        let fake = id(23);
        let mut wrong = login_half(&fake, &doc);
        wrong["sig_player"] = json!(player.sign_doc(CTX_LOGIN, &doc));
        assert!(check_login(&wrong).unwrap_err().contains("another world"));
        // seq: newer replaces, the same changes nothing, older or a same-seq different record refused
        let mut book = std::collections::BTreeMap::new();
        assert!(keep_login(&mut book, &rid, &rec).unwrap());
        assert!(!keep_login(&mut book, &rid, &rec).unwrap());
        let rec2 = login_countersign(&player, &login_half(&host, &login_doc(&player.public_hex(), &rid, "GABCDEFGHJKLMNP2", 2000, 2)), &rid, "GABCDEFGHJKLMNP2").unwrap();
        assert!(keep_login(&mut book, &rid, &rec2).unwrap());
        assert!(keep_login(&mut book, &rid, &rec).is_err(), "an old record played back");
        let same_seq = login_countersign(&player, &login_half(&host, &login_doc(&player.public_hex(), &rid, "GABCDEFGHJKLMNP2", 2001, 2)), &rid, "GABCDEFGHJKLMNP2").unwrap();
        assert!(keep_login(&mut book, &rid, &same_seq).is_err());
        // the host rebuilds the friend from the player's copy, for this world and this key only
        let f = rebuild_friend(&rec2, &rid, &player.public_hex(), 3000).unwrap();
        assert_eq!((f.user.as_str(), f.peer.as_str(), f.enabled), ("GABCDEFGHJKLMNP2", player.peer_id.as_str(), true));
        assert!(rebuild_friend(&rec2, "aaaaaaaaaaaaaaaa", &player.public_hex(), 3000).is_err());
        assert!(rebuild_friend(&rec2, &rid, &id(30).public_hex(), 3000).is_err());
        // the player's own store
        let d = tempfile::tempdir().unwrap();
        keep_player_login(d.path(), &rec).unwrap();
        keep_player_login(d.path(), &rec2).unwrap();
        assert!(keep_player_login(d.path(), &rec).is_err());
        assert_eq!(player_login(d.path(), &rid), Some(rec2));
    }

    #[test]
    fn guardians_rotation_needs_two_and_waits_for_a_veto() {
        let old = id(40);
        let new = id(41);
        let (g1, g2, g3, stranger) = (id(42), id(43), id(44), id(45));
        let list = vec![g1.public_hex(), g2.public_hex(), g3.public_hex()];
        let gl = make_guardians(&old, &list, 100, 1).unwrap();
        let (root, l, _) = check_guardians(&gl).unwrap();
        assert_eq!((root, l.len()), (old.public_hex(), 3));
        // shapes: 1 or 6 guardians, yourself, tampered, signed by a stranger
        assert!(make_guardians(&old, &list[..1], 100, 1).is_err());
        assert!(make_guardians(&old, &[list.clone(), vec![id(46).public_hex(), id(47).public_hex(), id(48).public_hex()]].concat(), 100, 1).is_err());
        assert!(make_guardians(&old, &[g1.public_hex(), old.public_hex()], 100, 1).is_err());
        let mut t = gl.clone();
        t["doc"]["guardians"][0] = json!(stranger.public_hex());
        assert!(check_guardians(&t).is_err());
        let forged = json!({"doc": gl["doc"], "sig": stranger.sign_doc(CTX_GUARDIANS, &gl["doc"])});
        assert!(check_guardians(&forged).is_err());
        // one guardian: not enough; the same guardian twice: still one; a stranger: not counted
        let (o, n) = (old.public_hex(), new.public_hex());
        let a1 = approve_rotation(&g1, &o, &n, 1000);
        let a2 = approve_rotation(&g2, &o, &n, 1100);
        assert!(rotation_bundle(&gl, &n, &[a1.clone()]).unwrap_err().contains("needs 2"));
        assert!(rotation_bundle(&gl, &n, &[a1.clone(), a1.clone()]).is_err());
        assert!(rotation_bundle(&gl, &n, &[a1.clone(), approve_rotation(&stranger, &o, &n, 1000)]).is_err());
        // an approval for another new key doesn't count for this one
        assert!(rotation_bundle(&gl, &n, &[a1.clone(), approve_rotation(&g2, &o, &id(49).public_hex(), 1000)]).is_err());
        // two: good, in force after the wait
        let b = rotation_bundle(&gl, &n, &[a1.clone(), a2.clone()]).unwrap();
        let r = check_rotation(&b).unwrap();
        assert_eq!((r.old.as_str(), r.new.as_str(), r.effective), (o.as_str(), n.as_str(), 1100 + GUARDIAN_WAIT));
        assert!(rotation_in_force(&b, 1200, &[]).unwrap_err().contains("waiting"));
        assert!(rotation_in_force(&b, 1100 + GUARDIAN_WAIT, &[]).is_ok());
        // a veto by the old root stops it; a veto by anyone else doesn't
        let v = make_veto(&old, &r.id, 1300);
        assert_eq!(check_veto(&v).unwrap(), (o.clone(), r.id.clone()));
        assert!(rotation_in_force(&b, 1100 + GUARDIAN_WAIT, &[v.clone()]).is_err());
        assert!(rotation_in_force(&b, 1100 + GUARDIAN_WAIT, &[make_veto(&g1, &r.id, 1300)]).is_ok());
        // tampering with the new key breaks every approval
        let mut tb = b.clone();
        tb["doc"]["new"] = json!(stranger.public_hex());
        assert!(check_rotation(&tb).is_err());
        // a self rotation: the old root's own signature, at once
        let s = make_rotation_self(&old, &n, 500);
        assert_eq!(rotation_in_force(&s, 500, &[]).unwrap().kind, "self");
        let fs = json!({"doc": s["doc"], "sig": new.sign_doc(CTX_ROTATE, &s["doc"])});
        assert!(check_rotation(&fs).is_err(), "the new key can't sign its own rotation");
    }

    #[test]
    fn recovery_with_guardian_answers() {
        let old = id(50);
        let (g1, g2) = (id(51), id(52));
        let gl = make_guardians(&old, &[g1.public_hex(), g2.public_hex()], 100, 1).unwrap();
        let d = tempfile::tempdir().unwrap();
        let code = recovery_start(d.path()).unwrap();
        assert_eq!(recovery_start(d.path()).unwrap(), code, "the same code until it is done");
        let new_pk = parse_recovery_code(&code).unwrap();
        let roster = vec![id(60).public_hex()];
        let t1 = guardian_approval_text(&g1, &gl, &roster, &new_pk, 1000).unwrap();
        assert!(guardian_approval_text(&id(53), &gl, &roster, &new_pk, 1000).is_err(), "not a guardian");
        // an answer for another code is refused
        let other = guardian_approval_text(&g2, &gl, &roster, &id(54).public_hex(), 1000).unwrap();
        assert!(recovery_add(d.path(), &other).is_err());
        assert!(recovery_add(d.path(), &t1).unwrap().is_none(), "one guardian isn't enough");
        assert!(recovery_add(d.path(), &t1).unwrap().is_none(), "nor the same one twice");
        let t2 = guardian_approval_text(&g2, &gl, &[], &new_pk, 1001).unwrap();
        let (bundle, ro) = recovery_add(d.path(), &t2).unwrap().expect("two agree");
        let r = check_rotation(&bundle).unwrap();
        assert_eq!((r.old, r.new.clone()), (old.public_hex(), new_pk.clone()));
        assert!(ro.contains(&id(60).public_hex()) && ro.contains(&g1.public_hex()), "friends to tell: the roster and the guardians");
        let me = identity::load_or_create(d.path()).unwrap();
        assert_eq!(me.public_hex(), new_pk, "this PC is the new Passport");
        assert_eq!(pending_phrase(d.path(), &me).unwrap().split(' ').count(), 12, "its new 12 words wait to be written down");
        assert!(State::load(d.path(), &me).rotation.is_some());
        assert!(recovery_load(d.path()).is_none());
    }

    #[test]
    fn a_second_device_by_code() {
        let root = id(70);
        let mut st = State { name: "Ari".into(), ..Default::default() };
        let r = root.public_hex();
        st.add_device(&make_device(&root, &id(71).public_hex(), "Desk", 1, 1), &r).unwrap();
        let newdev = id(72).public_hex();
        let mut b = PairNew::new(&newdev, "Laptop", now()).unwrap();
        let (mut a, m1) = PairOld::begin().unwrap();
        let ans = b.on_commit(&m1).unwrap();
        assert!(b.on_commit(&m1).is_err(), "one pairing at a time");
        let m2 = a.on_answer(&ans).unwrap();
        assert_eq!(a.peer_name(), "Laptop");
        assert!(b.code.is_none(), "no code before the reveal");
        b.on_reveal(&m2).unwrap();
        let code = b.code.clone().unwrap();
        assert_eq!(code.len(), 6);
        // a wrong code: nothing sent
        let wrong = if code == "000000" { "111111" } else { "000000" };
        assert!(a.confirm(wrong, &root, &mut st.clone()).is_err());
        let (m3, rec) = a.confirm(&format!("{} {}", &code[..3], &code[3..]), &root, &mut st).unwrap();
        assert_eq!(check_device(&rec, Some(&r)).unwrap().device, newdev);
        assert!(!m3.to_string().contains(&hex(&root.secret_seed())), "the seed is sealed");
        let p = b.on_seal(&m3).unwrap();
        assert_eq!(p.seed, root.secret_seed());
        assert_eq!((p.devices.len(), p.name.as_str()), (2, "Ari"));
        assert!(!format!("{p:?}").contains(&hex(&root.secret_seed())[..16]));
    }

    #[test]
    fn a_second_device_over_the_network_end_to_end() {
        use std::io::{BufRead, BufReader, Write};
        let (old_app, new_app) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let root = identity::load_or_create(old_app.path()).unwrap();
        let mut st = State::load(old_app.path(), &root);
        st.name = "Ari".into();
        ensure_this_device(old_app.path(), &root, &mut st).unwrap();
        st.save(old_app.path()).unwrap();
        let before = identity::load_or_create(new_app.path()).unwrap();
        // the new PC's listener (friendnet.rs hands "pair" lines to pair_serve the same way)
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let np = new_app.path().to_path_buf();
        std::thread::spawn(move || {
            for c in l.incoming().flatten().take(4) {
                let mut line = String::new();
                BufReader::new(c.try_clone().unwrap()).read_line(&mut line).unwrap();
                let v: Value = serde_json::from_str(line.trim()).unwrap();
                let mut c = c;
                c.write_all(format!("{}\n", pair_serve(&np, &v)).as_bytes()).unwrap();
            }
        });
        // nobody waiting yet: refused
        assert!(pair_find(&[addr]).is_err());
        pair_wait(new_app.path()).unwrap();
        assert_eq!(pair_status()["state"], "waiting");
        let name = pair_find(&[addr]).unwrap();
        assert!(!name.is_empty());
        let code = pair_status()["code"].as_str().unwrap().to_string();
        assert!(pair_confirm(old_app.path(), &root, &code).unwrap().starts_with("Done"));
        assert_eq!(pair_status()["state"], "done");
        let now_new = identity::load_or_create(new_app.path()).unwrap();
        assert_eq!(now_new.peer_id, root.peer_id, "the new PC is the same Passport");
        assert_ne!(before.peer_id, root.peer_id);
        let nst = State::load(new_app.path(), &now_new);
        assert_eq!((nst.name.as_str(), nst.devices.len()), ("Ari", 2), "both devices known on the new PC");
        assert_eq!(State::load(old_app.path(), &root).devices.len(), 2, "and on the old one");
    }

    #[test]
    fn a_pairing_man_in_the_middle_gets_a_different_code() {
        // M sits between: it must commit to the new PC before it sees the old PC's key, so the codes differ
        let mut b = PairNew::new(&id(80).public_hex(), "New", now()).unwrap();
        let (mut a, m1) = PairOld::begin().unwrap();
        let (mut m_old, m_commit) = PairOld::begin().unwrap(); // M's face to the new PC
        let mut m_new = PairNew::new(&id(81).public_hex(), "New", now()).unwrap(); // M's face to the old PC
        let ans_b = b.on_commit(&m_commit).unwrap();
        let ans_m = m_new.on_commit(&m1).unwrap();
        let reveal_a = a.on_answer(&ans_m).unwrap();
        let reveal_m = m_old.on_answer(&ans_b).unwrap();
        m_new.on_reveal(&reveal_a).unwrap();
        b.on_reveal(&reveal_m).unwrap();
        assert_ne!(a.code, b.code, "the code the player reads off the new PC doesn't match the old PC's");
        // and a reveal that doesn't match its commitment is refused
        let mut b2 = PairNew::new(&id(82).public_hex(), "New", now()).unwrap();
        let (_, c1) = PairOld::begin().unwrap();
        let (mut other, _) = PairOld::begin().unwrap();
        let ans = b2.on_commit(&c1).unwrap();
        let mut r = other.on_answer(&ans).unwrap();
        r["sid"] = c1["sid"].clone();
        assert!(b2.on_reveal(&r).is_err());
    }

    #[test]
    fn a_second_device_by_file() {
        let root = id(90);
        let st = State { name: "Bo".into(), ..Default::default() };
        let (text, six) = file_export_with(&root, &st, 10).unwrap();
        assert!(text.starts_with("# Leyline Passport"));
        assert_eq!(six.split(' ').count(), 6);
        assert!(!text.contains(&hex(&root.secret_seed())));
        let p = file_import_with(&text, &six.to_ascii_uppercase(), 10).unwrap();
        assert_eq!((p.seed, p.name.as_str()), (root.secret_seed(), "Bo"));
        assert!(file_import_with(&text, "abandon abandon abandon abandon abandon abandon", 10).is_err());
        assert!(file_import_with("hello", &six, 10).is_err());
        let mut bad = text.clone();
        bad.insert_str(text.find("LPF1.").unwrap() + 7, "a");
        assert!(file_import_with(&bad, &six, 10).is_err());
    }

    #[test]
    fn hero_key_alias_keeps_no_key() {
        let root = id(95);
        let key = format!("ovk1.ABCDEFGHIJ.{}", "x".repeat(43));
        let rec = make_herokey_alias(&root, &format!("Leyline Hero Key\n{key}\n"), 1000).unwrap();
        assert_eq!(check_herokey_alias(&rec).unwrap(), (root.public_hex(), "ABCDEFGHIJ".to_string()));
        assert!(!rec.to_string().contains(&"x".repeat(43)), "never the key itself");
        assert!(make_herokey_alias(&root, "hello", 1).is_err());
        let mut t = rec.clone();
        t["doc"]["id"] = json!("ZZZZZZZZZZ");
        assert!(check_herokey_alias(&t).is_err());
    }

    #[test]
    fn the_state_never_holds_a_secret_and_view_shows_no_key() {
        let d = tempfile::tempdir().unwrap();
        let me = identity::load_or_create(d.path()).unwrap();
        let v = view(d.path(), &me);
        assert_eq!(v["phrase_waiting"], true);
        assert_eq!(v["devices"].as_array().unwrap().len(), 1, "this PC's own device record is made");
        assert_eq!(v["devices"][0]["this"], true);
        let seed = hex(&me.secret_seed());
        let file = std::fs::read_to_string(state_path(d.path())).unwrap();
        assert!(!file.contains(&seed) && !v.to_string().contains(&seed), "never the seed");
        assert!(!v.to_string().contains(&me.public_hex()), "the page gets ids only");
        let ph = pending_phrase(d.path(), &me).unwrap();
        let first3: String = ph.split(' ').take(3).collect::<Vec<_>>().join(" ");
        assert!(!v.to_string().contains(&first3), "no phrase in the view");
    }

    /// The conformance vectors for other launchers (Commonhold): fixed seeds, so every signature is fixed.
    fn vectors() -> Value {
        let root = id(1);
        let host = id(2);
        let (g1, g2) = (id(3), id(4));
        let new_root = id(5);
        let rid = host.peer_id.clone();
        let wk = world_identity(&root, &rid);
        let entropy = [0x11u8; 16];
        let dev = make_device(&root, &id(6).public_hex(), "Desk PC", 1764200000, 1);
        let revoke = make_device_revoke(&root, &id(6).public_hex(), 1764200100, 2);
        let gl = make_guardians(&root, &[g1.public_hex(), g2.public_hex()], 1764200000, 1).unwrap();
        let (o, n) = (root.public_hex(), new_root.public_hex());
        let bundle = rotation_bundle(&gl, &n, &[approve_rotation(&g1, &o, &n, 1764300000), approve_rotation(&g2, &o, &n, 1764300100)]).unwrap();
        let one = json!({"doc": bundle["doc"], "guardians": gl, "approvals": [bundle["approvals"][0]]});
        let doc = login_doc(&wk.public_hex(), &rid, &crate::host::worldfriends::name_for(&rid, &wk.peer_id), 1764200000, 1);
        let login = login_countersign(&wk, &login_half(&host, &doc), &rid, doc["account"].as_str().unwrap()).unwrap();
        let mut tampered = login.clone();
        tampered["doc"]["seq"] = json!(2);
        json!({
            "format": "leyline-passport-v1-vectors",
            "about": "Ed25519 over context + \"\\n\" + canonical JSON (sorted keys, no spaces). Seeds are 32 bytes of the byte shown. world key = Ed25519 seed HKDF-SHA256(salt \"leyline-world-key-v1\", root seed, info realm id). 12-word root seed = HKDF-SHA256(salt \"leyline-passport-root-v1\", 128-bit entropy, info \"root\"); 24 words are the seed itself.",
            "phrase": {"entropy": hex(&entropy), "words": phrase_from_entropy(&entropy), "root_seed": hex(&seed_from_entropy(&entropy)), "root_public": PeerIdentity::from_seed(seed_from_entropy(&entropy)).unwrap().public_hex()},
            "keys": {"root_seed_byte": 1, "root_public": o, "host_seed_byte": 2, "realm_id": rid, "world_public": wk.public_hex(), "world_peer_id": wk.peer_id, "guardian_seed_bytes": [3, 4], "new_root_public": n},
            "cases": [
                {"name": "device", "context": CTX_DEVICE, "record": dev, "expect": "ok"},
                {"name": "device-revoke", "context": CTX_DEVICE_REVOKE, "record": revoke, "expect": "ok"},
                {"name": "guardians", "context": CTX_GUARDIANS, "record": gl, "expect": "ok"},
                {"name": "rotation-two-guardians", "context": CTX_ROTATE, "record": bundle, "expect": "ok", "effective": 1764300100 + GUARDIAN_WAIT},
                {"name": "rotation-one-guardian", "context": CTX_ROTATE, "record": one, "expect": "refused"},
                {"name": "login", "context": CTX_LOGIN, "record": login, "expect": "ok"},
                {"name": "login-tampered", "context": CTX_LOGIN, "record": tampered, "expect": "refused"},
            ]
        })
    }

    #[test]
    fn conformance_vectors_match_the_file() {
        let v = vectors();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/p2p/passport-v1-vectors.json");
        if std::env::var("LEYLINE_WRITE_VECTORS").is_ok() {
            std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
        }
        let file: Value = serde_json::from_str(include_str!("p2p/passport-v1-vectors.json")).unwrap();
        assert_eq!(file, v, "regenerate with LEYLINE_WRITE_VECTORS=1");
        for c in file["cases"].as_array().unwrap() {
            let r = &c["record"];
            let ok = match c["name"].as_str().unwrap() {
                "device" => check_device(r, None).is_ok(),
                "device-revoke" => check_device_revoke(r, None).is_ok(),
                "guardians" => check_guardians(r).is_ok(),
                n if n.starts_with("rotation") => check_rotation(r).is_ok(),
                _ => check_login(r).is_ok(),
            };
            assert_eq!(ok, c["expect"] == "ok", "{}", c["name"]);
        }
    }
}
