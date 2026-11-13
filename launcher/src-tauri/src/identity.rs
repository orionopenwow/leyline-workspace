//! This launcher's own identity on the Leyline network (docs/LEYLINE-FRIENDS.md): one Ed25519 key made on this PC,
//! kept in `<app data>/peer.key`, never sent anywhere. Friends, profile cards, presence and lasting world logins for
//! friends (host side: keyed on the friend's peer id) all stand on it; no website and no server is needed.
//!
//! * `peer.key`: 64 hex characters, the Ed25519 seed (the same shape as a Den's `realm.key`, host/litenet.rs), made
//!   once with the system's random source, owner-only on Mac and Linux. Settings › Your Leyline network key saves it
//!   to a file and brings it back ([`export_text`], [`import`]): it is the only thing that makes "you" you to your
//!   friends, so a reinstall or a new PC keeps your characters on their worlds. Never shown, never logged.
//! * **Peer id** = base32(sha256(public key))[..16], lower case: the mesh's id rule (p2p/mesh.rs `realm_id`), so a
//!   record can't claim someone else's id.
//! * **Signatures**: Ed25519 over `context + "\n" + canonical JSON` (sorted keys, no spaces), as every mesh record.
//! * **Encryption to a peer** ([`seal`] / [`open`]): X25519 keys converted from the Ed25519 ones (the standard
//!   birational map, as libsodium's `crypto_sign_ed25519_*_to_curve25519`), so a friend code (the public key) is all
//!   that is needed to write to someone. An ephemeral X25519 key per message, HKDF-SHA256, ChaCha20-Poly1305 (ring).
//!   ring has no static X25519, so the curve arithmetic is a small port of TweetNaCl's (public domain), checked
//!   against RFC 7748's vectors in the tests.
//! * **Joining a friend's world** ([`join_challenge`], [`join_answer`], [`verify_join`]): the host sends a fresh
//!   challenge, the friend signs it with their peer key, the host checks the signature and that the key's peer id is
//!   a friend's. See docs/LEYLINE-FRIENDS.md, "Lasting world logins".

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The key file's name, in the app data folder.
pub const KEY_FILE: &str = "peer.key";
pub const CTX_JOIN: &str = "leyline-join-v1";
const SEAL_INFO: &[u8] = b"leyline-friends-seal-v1";
/// How long a join challenge is good for.
pub const JOIN_TTL: u64 = 120;

// ------------------------------------------------------------------------------------------------ small helpers

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

/// 32 bytes from 64 hex characters (lower or upper case).
pub fn unhex32(s: &str) -> Option<[u8; 32]> {
    unhex(s).filter(|v| v.len() == 32).and_then(|v| v.try_into().ok())
}

const B32: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";

/// RFC 4648 base32, lower case, no padding.
pub fn b32(data: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in data {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(B32[((buf >> bits) & 31) as usize] as char);
        }
        buf &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// The other way; upper case is read too. None for anything that isn't base32 (or has leftover bits set).
pub fn unb32(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let c = c.to_ascii_lowercase();
        let v = B32.iter().position(|&x| x == c)? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
        buf &= (1 << bits) - 1;
    }
    if buf != 0 || bits >= 5 {
        return None;
    }
    Some(out)
}

/// `peer id = base32(sha256(public key))[..16]`, lower case (the mesh's id rule).
pub fn peer_id(public: &[u8]) -> String {
    crate::p2p::mesh::realm_id(public)
}

/// A peer id: 16 base32 characters.
pub fn peer_id_ok(s: &str) -> bool {
    crate::realms::realm_id_ok(s)
}

fn random<const N: usize>() -> Result<[u8; N], String> {
    use ring::rand::SecureRandom;
    let mut b = [0u8; N];
    ring::rand::SystemRandom::new().fill(&mut b).map_err(|_| "This PC has no random source.".to_string())?;
    Ok(b)
}

/// Random bytes as base32 (tokens and nonces).
pub fn random_b32(n: usize) -> Result<String, String> {
    let b: [u8; 32] = random()?;
    Ok(b32(&b[..n.min(32)]))
}

/// Ed25519 over `context + "\n" + canonical(doc)`, the mesh's rule (p2p/mesh.rs `verify_doc`).
pub fn verify_doc(pub_hex: &str, doc: &Value, context: &str, sig_hex: &str) -> bool {
    crate::p2p::mesh::verify_doc(&pub_hex.to_ascii_lowercase(), doc, context, &sig_hex.to_ascii_lowercase())
}

pub fn canonical(v: &Value) -> String {
    crate::p2p::flags::canonical(v)
}

// ------------------------------------------------------------------------------------------------ the identity

/// This launcher's identity: `(peer_id, public_key, sign)`. The private half stays inside.
pub struct PeerIdentity {
    pub peer_id: String,
    pub public_key: [u8; 32],
    seed: [u8; 32],
}

impl std::fmt::Debug for PeerIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // never the seed
        f.debug_struct("PeerIdentity").field("peer_id", &self.peer_id).finish()
    }
}

impl PeerIdentity {
    pub fn from_seed(seed: [u8; 32]) -> Result<Self, String> {
        use ring::signature::KeyPair;
        let kp = ring::signature::Ed25519KeyPair::from_seed_unchecked(&seed).map_err(|_| "bad key".to_string())?;
        let public_key: [u8; 32] = kp.public_key().as_ref().try_into().map_err(|_| "bad key".to_string())?;
        Ok(PeerIdentity { peer_id: peer_id(&public_key), public_key, seed })
    }

    /// The public key as 64 hex characters (what records carry).
    pub fn public_hex(&self) -> String {
        hex(&self.public_key)
    }

    /// Ed25519 signature over `msg`.
    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        let kp = ring::signature::Ed25519KeyPair::from_seed_unchecked(&self.seed).expect("checked in from_seed");
        let mut out = [0u8; 64];
        out.copy_from_slice(kp.sign(msg).as_ref());
        out
    }

    /// Signature (hex) over `context + "\n" + canonical(doc)`.
    pub fn sign_doc(&self, context: &str, doc: &Value) -> String {
        hex(&self.sign(format!("{context}\n{}", canonical(doc)).as_bytes()))
    }

    /// This identity's X25519 secret (from the Ed25519 seed: sha512, the first half; clamped when used).
    fn x_secret(&self) -> [u8; 32] {
        let d = ring::digest::digest(&ring::digest::SHA512, &self.seed);
        let mut s = [0u8; 32];
        s.copy_from_slice(&d.as_ref()[..32]);
        s
    }

    /// Open a box sealed to this identity ([`seal`]).
    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String> {
        open_with(&self.x_secret(), &ed_to_x_public(&self.public_key).ok_or("bad key")?, sealed)
    }
}

/// `<app>/peer.key`.
pub fn key_path(app: &Path) -> PathBuf {
    app.join(KEY_FILE)
}

/// This PC's identity: the key in `<app>/peer.key`, made the first time.
pub fn load_or_create(app: &Path) -> Result<PeerIdentity, String> {
    // one at a time: the Friends list and a world's claim can both ask on a first run, and two new keys would be two
    // people (the second write would replace the key the first one is already signing with)
    static MAKING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = MAKING.lock().unwrap_or_else(|e| e.into_inner());
    let path = key_path(app);
    match std::fs::read_to_string(&path) {
        Ok(t) => {
            let seed = unhex32(t.trim()).ok_or("Your Leyline network key (peer.key) doesn't look right. Move it away to make a new one (your friends will need to add you again).")?;
            PeerIdentity::from_seed(seed)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let seed: [u8; 32] = random()?;
            std::fs::create_dir_all(app).map_err(|e| e.to_string())?;
            crate::settings::write_atomic(&path, (hex(&seed) + "\n").as_bytes()).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            PeerIdentity::from_seed(seed)
        }
        Err(e) => Err(format!("Leyline couldn't read its network key: {e}")),
    }
}

/// The first line of a saved key file: what it is and the warning, in plain words.
pub const EXPORT_HEAD: &str = "# Leyline network key: whoever has this file is you to your friends and on their worlds. Keep it private and never send it to anyone.";

/// The key file's text for Settings › Save my key (made first when this PC has none yet).
pub fn export_text(app: &Path) -> Result<String, String> {
    load_or_create(app)?;
    let seed = unhex32(std::fs::read_to_string(key_path(app)).map_err(|e| e.to_string())?.trim()).ok_or("Your Leyline network key doesn't look right.")?;
    Ok(format!("{EXPORT_HEAD}\n{}\n", hex(&seed)))
}

/// The seed in a saved key file (its comment lines skipped), or a bare key.
pub fn parse_export(text: &str) -> Option<[u8; 32]> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))?;
    unhex32(line)
}

/// Settings › Bring back my key: this PC becomes the identity in `text` (a file saved by [`export_text`]). The key
/// it had is kept beside it (`peer.key.before-<unix seconds>`), never deleted. The new peer id's first four
/// characters (what friends see as "Hero XXXX"), or why not.
pub fn import(app: &Path, text: &str) -> Result<String, String> {
    let seed = parse_export(text).ok_or("That file isn't a saved Leyline network key.")?;
    let id = PeerIdentity::from_seed(seed)?;
    let path = key_path(app);
    if let Ok(t) = std::fs::read_to_string(&path) {
        if unhex32(t.trim()) == Some(seed) {
            return Ok(id.peer_id.chars().take(4).collect::<String>().to_ascii_uppercase());
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        std::fs::copy(&path, app.join(format!("{KEY_FILE}.before-{now}"))).map_err(|e| format!("Your current key couldn't be kept aside, so nothing changed: {e}"))?;
    }
    std::fs::create_dir_all(app).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&path, (hex(&seed) + "\n").as_bytes()).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(id.peer_id.chars().take(4).collect::<String>().to_ascii_uppercase())
}

/// This PC's identity, in the normal app data folder.
pub fn peer_identity() -> Result<PeerIdentity, String> {
    load_or_create(&crate::settings::app_dir())
}

// ------------------------------------------------------------------------------------------------ joining a world

/// A host's challenge to a friend joining its world: fresh, for one world, good for [`JOIN_TTL`] seconds.
pub fn join_challenge(host: &PeerIdentity, world: &str, now: u64) -> Result<Value, String> {
    Ok(json!({"v": 1, "host": host.peer_id, "world": world, "nonce": random_b32(16)?, "ts": now}))
}

/// The friend's answer: their public key and their signature over the challenge.
pub fn join_answer(me: &PeerIdentity, challenge: &Value) -> Value {
    json!({"pubkey": me.public_hex(), "sig": me.sign_doc(CTX_JOIN, challenge)})
}

/// The bytes a join signature covers.
pub fn join_bytes(challenge: &Value) -> Vec<u8> {
    format!("{CTX_JOIN}\n{}", canonical(challenge)).into_bytes()
}

/// Whether `sig` is `pubkey`'s Ed25519 signature over `challenge` (the bytes of [`join_bytes`]).
pub fn verify_join(challenge: &[u8], pubkey: &[u8], sig: &[u8]) -> bool {
    pubkey.len() == 32 && sig.len() == 64 && ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, pubkey).verify(challenge, sig).is_ok()
}

/// The whole host-side check: the answer signs this challenge, the challenge is this host's and fresh. Returns the
/// joiner's peer id; the caller then checks it is a friend's (p2p/friends.rs `Book::is_friend`).
pub fn check_join(host_peer: &str, challenge: &Value, answer: &Value, now: u64) -> Result<String, String> {
    if challenge.get("host").and_then(Value::as_str) != Some(host_peer) {
        return Err("not this host's challenge".into());
    }
    let ts = challenge.get("ts").and_then(Value::as_u64).ok_or("no time")?;
    if ts > now + 30 || now.saturating_sub(ts) > JOIN_TTL {
        return Err("challenge expired".into());
    }
    let pk = answer.get("pubkey").and_then(Value::as_str).and_then(unhex32).ok_or("no key")?;
    let sig = answer.get("sig").and_then(Value::as_str).and_then(unhex).ok_or("no signature")?;
    if !verify_join(&join_bytes(challenge), &pk, &sig) {
        return Err("signature".into());
    }
    Ok(peer_id(&pk))
}

// ------------------------------------------------------------------------------------------------ X25519 (TweetNaCl)

type Gf = [i64; 16];
const GF0: Gf = [0; 16];
const GF1: Gf = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const K121665: Gf = [0xDB41, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn car(o: &mut Gf) {
    for i in 0..16 {
        o[i] += 1 << 16;
        let c = o[i] >> 16;
        if i < 15 {
            o[i + 1] += c - 1;
        } else {
            o[0] += 38 * (c - 1);
        }
        o[i] -= c << 16;
    }
}

fn sel(p: &mut Gf, q: &mut Gf, b: i64) {
    let c = !(b - 1);
    for i in 0..16 {
        let t = c & (p[i] ^ q[i]);
        p[i] ^= t;
        q[i] ^= t;
    }
}

fn pack(n: &Gf) -> [u8; 32] {
    let mut t = *n;
    car(&mut t);
    car(&mut t);
    car(&mut t);
    let mut m = GF0;
    for _ in 0..2 {
        m[0] = t[0] - 0xffed;
        for i in 1..15 {
            m[i] = t[i] - 0xffff - ((m[i - 1] >> 16) & 1);
            m[i - 1] &= 0xffff;
        }
        m[15] = t[15] - 0x7fff - ((m[14] >> 16) & 1);
        let b = (m[15] >> 16) & 1;
        m[14] &= 0xffff;
        sel(&mut t, &mut m, 1 - b);
    }
    let mut o = [0u8; 32];
    for i in 0..16 {
        o[2 * i] = (t[i] & 0xff) as u8;
        o[2 * i + 1] = (t[i] >> 8) as u8;
    }
    o
}

fn unpack(n: &[u8; 32]) -> Gf {
    let mut o = GF0;
    for i in 0..16 {
        o[i] = n[2 * i] as i64 + ((n[2 * i + 1] as i64) << 8);
    }
    o[15] &= 0x7fff;
    o
}

fn add(a: &Gf, b: &Gf) -> Gf {
    let mut o = GF0;
    for i in 0..16 {
        o[i] = a[i] + b[i];
    }
    o
}

fn sub(a: &Gf, b: &Gf) -> Gf {
    let mut o = GF0;
    for i in 0..16 {
        o[i] = a[i] - b[i];
    }
    o
}

fn mul(a: &Gf, b: &Gf) -> Gf {
    let mut t = [0i64; 31];
    for i in 0..16 {
        for j in 0..16 {
            t[i + j] += a[i] * b[j];
        }
    }
    for i in 0..15 {
        t[i] += 38 * t[i + 16];
    }
    let mut o = GF0;
    o.copy_from_slice(&t[..16]);
    car(&mut o);
    car(&mut o);
    o
}

fn sq(a: &Gf) -> Gf {
    mul(a, a)
}

fn inv(i: &Gf) -> Gf {
    let mut c = *i;
    for a in (0..=253).rev() {
        c = sq(&c);
        if a != 2 && a != 4 {
            c = mul(&c, i);
        }
    }
    c
}

/// X25519(scalar, u): RFC 7748.
pub fn x25519(n: &[u8; 32], p: &[u8; 32]) -> [u8; 32] {
    let mut z = *n;
    z[31] = (n[31] & 127) | 64;
    z[0] &= 248;
    let x = unpack(p);
    let (mut a, mut b, mut c, mut d) = (GF1, x, GF0, GF1);
    for i in (0..=254usize).rev() {
        let r = ((z[i >> 3] >> (i & 7)) & 1) as i64;
        sel(&mut a, &mut b, r);
        sel(&mut c, &mut d, r);
        let mut e = add(&a, &c);
        a = sub(&a, &c);
        c = add(&b, &d);
        b = sub(&b, &d);
        d = sq(&e);
        let f = sq(&a);
        a = mul(&c, &a);
        c = mul(&b, &e);
        e = add(&a, &c);
        a = sub(&a, &c);
        b = sq(&a);
        c = sub(&d, &f);
        a = mul(&c, &K121665);
        a = add(&a, &d);
        c = mul(&c, &a);
        a = mul(&d, &f);
        d = mul(&b, &x);
        b = sq(&e);
        sel(&mut a, &mut b, r);
        sel(&mut c, &mut d, r);
    }
    pack(&mul(&a, &inv(&c)))
}

/// X25519(scalar, 9): the public key of an X25519 secret.
pub fn x25519_base(n: &[u8; 32]) -> [u8; 32] {
    let mut nine = [0u8; 32];
    nine[0] = 9;
    x25519(n, &nine)
}

/// An Ed25519 public key's X25519 public key: u = (1 + y) / (1 - y). None for the one y with no u (y = 1).
pub fn ed_to_x_public(ed: &[u8; 32]) -> Option<[u8; 32]> {
    let y = unpack(ed);
    let den = sub(&GF1, &y);
    if pack(&den) == [0u8; 32] {
        return None;
    }
    Some(pack(&mul(&add(&GF1, &y), &inv(&den))))
}

// ------------------------------------------------------------------------------------------------ sealed boxes

fn aead_key(shared: &[u8; 32], eph: &[u8; 32], to: &[u8; 32]) -> Result<ring::aead::LessSafeKey, String> {
    let mut salt = [0u8; 64];
    salt[..32].copy_from_slice(eph);
    salt[32..].copy_from_slice(to);
    let prk = ring::hkdf::Salt::new(ring::hkdf::HKDF_SHA256, &salt).extract(shared);
    let info = [SEAL_INFO];
    let okm = prk.expand(&info, &ring::aead::CHACHA20_POLY1305).map_err(|_| "key".to_string())?;
    Ok(ring::aead::LessSafeKey::new(ring::aead::UnboundKey::from(okm)))
}

/// Seal `plain` to the holder of Ed25519 public key `to`: `ephemeral X25519 public (32) || ciphertext || tag (16)`.
/// Only that key's holder can open it; nothing in it says who sent it (the sender signs inside).
pub fn seal(to: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    let tx = ed_to_x_public(to).ok_or("That key can't be written to.")?;
    let eph_secret: [u8; 32] = random()?;
    seal_with(&eph_secret, &tx, plain)
}

fn seal_with(eph_secret: &[u8; 32], tx: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    let eph = x25519_base(eph_secret);
    let shared = x25519(eph_secret, tx);
    if shared == [0u8; 32] {
        return Err("That key can't be written to.".into());
    }
    let key = aead_key(&shared, &eph, tx)?;
    let mut buf = plain.to_vec();
    key.seal_in_place_append_tag(ring::aead::Nonce::assume_unique_for_key([0u8; 12]), ring::aead::Aad::from(&tx[..]), &mut buf)
        .map_err(|_| "seal".to_string())?;
    let mut out = eph.to_vec();
    out.extend_from_slice(&buf);
    Ok(out)
}

fn open_with(secret: &[u8; 32], mine: &[u8; 32], sealed: &[u8]) -> Result<Vec<u8>, String> {
    if sealed.len() < 32 + 16 {
        return Err("too short".into());
    }
    let eph: [u8; 32] = sealed[..32].try_into().map_err(|_| "too short")?;
    let shared = x25519(secret, &eph);
    if shared == [0u8; 32] {
        return Err("bad box".into());
    }
    let key = aead_key(&shared, &eph, mine)?;
    let mut buf = sealed[32..].to_vec();
    let n = key
        .open_in_place(ring::aead::Nonce::assume_unique_for_key([0u8; 12]), ring::aead::Aad::from(&mine[..]), &mut buf)
        .map_err(|_| "This message isn't for this PC, or it was changed on the way.".to_string())?
        .len();
    buf.truncate(n);
    Ok(buf)
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_saved_key_brings_the_same_identity_back_and_keeps_the_old_one() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let me = load_or_create(a.path()).unwrap();
        let text = export_text(a.path()).unwrap();
        assert!(text.starts_with("# Leyline network key") && text.contains("Keep it private"), "the warning comes with it");
        assert_eq!(parse_export(&text).map(|s| PeerIdentity::from_seed(s).unwrap().peer_id), Some(me.peer_id.clone()));
        // a new PC that already made its own key: the saved one replaces it, the old one is kept aside
        let other = load_or_create(b.path()).unwrap();
        assert_ne!(other.peer_id, me.peer_id);
        let short = import(b.path(), &text).unwrap();
        assert_eq!(short, me.peer_id[..4].to_ascii_uppercase());
        assert_eq!(load_or_create(b.path()).unwrap().peer_id, me.peer_id, "the same you to your friends");
        assert!(std::fs::read_dir(b.path()).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("peer.key.before-")));
        // again: nothing changes; a file that isn't one: refused, nothing touched
        assert!(import(b.path(), &text).is_ok());
        assert!(import(b.path(), "hello").is_err());
        assert!(import(b.path(), &"z".repeat(64)).is_err());
        assert_eq!(load_or_create(b.path()).unwrap().peer_id, me.peer_id);
        // never shown: the identity's debug view has no seed
        assert!(!format!("{me:?}").contains(&text.lines().nth(1).unwrap()[..16]));
    }

    use super::*;

    fn h32(s: &str) -> [u8; 32] {
        unhex32(s).unwrap()
    }

    #[test]
    fn rfc7748_vectors() {
        // RFC 7748 section 5.2, the first vector
        let k = h32("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4");
        let u = h32("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c");
        assert_eq!(hex(&x25519(&k, &u)), "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552");
        // section 6.1: Alice's public key, and the shared secret both ways
        let a = h32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        assert_eq!(hex(&x25519_base(&a)), "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
        let bp = h32("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
        assert_eq!(hex(&x25519(&a, &bp)), "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
    }

    #[test]
    fn ed_to_x_matches_the_secret_side() {
        for s in 0u8..6 {
            let id = PeerIdentity::from_seed([s.wrapping_mul(37).wrapping_add(1); 32]).unwrap();
            assert_eq!(ed_to_x_public(&id.public_key).unwrap(), x25519_base(&id.x_secret()), "seed {s}");
        }
    }

    #[test]
    fn b32_round_trip() {
        for n in 0..40 {
            let v: Vec<u8> = (0..n).map(|i| (i * 7 + 3) as u8).collect();
            assert_eq!(unb32(&b32(&v)).unwrap(), v);
        }
        assert_eq!(b32(b"foobar"), "mzxw6ytboi");
        assert!(unb32("mzxw6ytboj").is_none(), "leftover bits");
        assert!(unb32("ab!").is_none());
        assert_eq!(unb32("MZXW6YTBOI").unwrap(), b"foobar");
    }

    #[test]
    fn key_made_once_kept_and_private() {
        let d = tempfile::tempdir().unwrap();
        let a = load_or_create(d.path()).unwrap();
        let b = load_or_create(d.path()).unwrap();
        assert_eq!(a.peer_id, b.peer_id);
        assert_eq!(a.public_key, b.public_key);
        assert!(peer_id_ok(&a.peer_id));
        assert_eq!(a.peer_id, peer_id(&a.public_key));
        let text = std::fs::read_to_string(d.path().join(KEY_FILE)).unwrap();
        assert_eq!(text.trim().len(), 64);
        assert!(!format!("{a:?}").contains(text.trim()), "Debug never shows the seed");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(d.path().join(KEY_FILE)).unwrap().permissions().mode() & 0o777, 0o600);
        }
        // a second PC has another identity
        let d2 = tempfile::tempdir().unwrap();
        assert_ne!(load_or_create(d2.path()).unwrap().peer_id, a.peer_id);
        // a broken file is refused, never silently replaced
        std::fs::write(d.path().join(KEY_FILE), "nope").unwrap();
        assert!(load_or_create(d.path()).is_err());
    }

    #[test]
    fn sign_and_verify_docs() {
        let id = PeerIdentity::from_seed([5; 32]).unwrap();
        let doc = json!({"v": 1, "b": "x", "a": [1, 2]});
        let sig = id.sign_doc("ctx-v1", &doc);
        assert!(verify_doc(&id.public_hex(), &doc, "ctx-v1", &sig));
        assert!(!verify_doc(&id.public_hex(), &doc, "ctx-v2", &sig));
        assert!(!verify_doc(&id.public_hex(), &json!({"v": 1, "b": "y", "a": [1, 2]}), "ctx-v1", &sig));
    }

    #[test]
    fn seal_round_trip_and_tamper() {
        let a = PeerIdentity::from_seed([1; 32]).unwrap();
        let b = PeerIdentity::from_seed([2; 32]).unwrap();
        let boxed = seal(&b.public_key, b"hello friend").unwrap();
        assert_eq!(b.open(&boxed).unwrap(), b"hello friend");
        assert!(a.open(&boxed).is_err(), "only the recipient opens it");
        let mut bad = boxed.clone();
        let n = bad.len();
        bad[n - 1] ^= 1;
        assert!(b.open(&bad).is_err());
        let mut bad = boxed.clone();
        bad[40] ^= 1;
        assert!(b.open(&bad).is_err());
        assert!(b.open(&boxed[..20]).is_err());
        // two seals of the same text differ (a new key each time)
        assert_ne!(seal(&b.public_key, b"x").unwrap(), seal(&b.public_key, b"x").unwrap());
        assert_eq!(b.open(&seal(&b.public_key, b"").unwrap()).unwrap(), b"");
    }

    #[test]
    fn join_handshake() {
        let host = PeerIdentity::from_seed([3; 32]).unwrap();
        let friend = PeerIdentity::from_seed([4; 32]).unwrap();
        let ch = join_challenge(&host, "single", 1000).unwrap();
        let ans = join_answer(&friend, &ch);
        assert_eq!(check_join(&host.peer_id, &ch, &ans, 1010).unwrap(), friend.peer_id);
        assert!(check_join(&host.peer_id, &ch, &ans, 1000 + JOIN_TTL + 1).is_err(), "expired");
        assert!(check_join(&friend.peer_id, &ch, &ans, 1010).is_err(), "another host's challenge");
        let mut other = ch.clone();
        other["world"] = json!("den");
        assert!(check_join(&host.peer_id, &other, &ans, 1010).is_err(), "signed for another world");
        let sig = unhex(ans["sig"].as_str().unwrap()).unwrap();
        assert!(verify_join(&join_bytes(&ch), &friend.public_key, &sig));
        assert!(!verify_join(&join_bytes(&ch), &host.public_key, &sig));
    }
}
