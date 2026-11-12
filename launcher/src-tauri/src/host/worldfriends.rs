//! Friends keep their character on a single player world (docs/SINGLE-PLAYER-ONE-CLICK.md, "Playing with friends").
//!
//! A friend who joins a world on the host's PC (the Built-in engine's Single player / the original world, Leyline
//! Solo) gets a **lasting login** there, tied to the friend's own Leyline identity, so the same characters are there
//! next time. Peer to peer: nothing on a website or a cloud server is asked; the two launchers meet over the
//! invite's own way in (the Leyline relay tunnel or the same network), and the relay only passes bytes through.
//!
//! **Identity.** Each launcher has its own Ed25519 key pair: identity.rs's [`Peer`] (`<app data>/peer.key`, the same
//! key the Friends list signs with). Its peer id is base32(sha256(public key))[:16], as the mesh's realm ids. The
//! friend's login on a world is [`name_for`]: `G` + 15 base32 of sha256(context, the world's realm id, the peer id):
//! the same on every join from that identity, a different name on every world, and never the shape of today's
//! one-time invite logins (`G` + 9) or the host's own (`SOLO`, `HK...`). A Hero Key is not needed.
//!
//! **The handshake** (the claim), on the world's login port, before the game itself logs in. The friend's launcher
//! connects the way its game will (after the relay's knock), and the host's launcher answers it in front of the
//! login server ([`front_start`]: the game's own first byte is 0x00 or 0x02, a claim's is `L`, so one port does both):
//!
//! ```text
//!   friend -> host   LLC1 HELLO\n
//!   host -> friend   LLC1 CHAL <32 hex: 16 random bytes>\n
//!   friend -> host   LLC1 CLAIM <hex: 12-byte nonce || ChaCha20-Poly1305(claim JSON), aad "LLC1">\n
//!   host -> friend   LLC1 OK <login>\n   or   LLC1 NO <why>\n
//! ```
//!
//! The AEAD key is HMAC-SHA256(the invite secret, "leyline-world-claim-v1\n" + realm id): only someone holding the
//! invite link can read or make one; the relay keeps only a hash of that secret, so it can't. The claim JSON:
//! `{"v":1,"pk":<hex public key>,"sig":<hex Ed25519 signature>,"pass":<new password>,"ticket":<G+9 or "">,
//! "tpass":<its password or "">,"old":[<G+9 logins this PC used on the world before>]}`; the signature covers
//! [`signed_text`]: the context, the realm id, the host's challenge, the ticket and sha256(pass), so a claim can't
//! be replayed or moved to another world. The password is the friend launcher's new one-time password: the host
//! sets it on the lasting login (each join a fresh one) and the friend's game signs in with it.
//!
//! **When a claim is let in** ([`admit`]): a known identity while it is invited (an invite opened by it since
//! sharing last stopped) or trusted; a new identity with a fresh invite ticket (the `G` + 9 login in the link, 7
//! days, one identity). An identity that comes with an `old` login which is a kept earlier friend's (a one-time
//! login that has characters) takes that login over, so today's friends keep their characters.
//!
//! **Lifecycle.** Lasting logins never expire. Stop sharing: each gets a random password nobody holds (disabled) and,
//! unless trusted, needs an invite again. Remove a friend: their login is disabled and forgotten, characters kept
//! (the same identity invited again gets them back), or deleted with them. One-time tickets with no characters go
//! when their time is up or sharing stops; one with characters is kept as an earlier friend's login ([`fold`]).

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const MAGIC: &str = "LLC1";
/// In an invite link (`#<secret>~~<ticket>~~...~~lf1`): the world answers claims (a lasting friend's login).
pub const LINK_MARK: &str = "lf1";
pub const CTX: &str = "leyline-world-claim-v1";
pub const BOOK_FILE: &str = "friends.json";
pub const PEER_KEY_FILE: &str = crate::identity::KEY_FILE;
/// The friend's side: the worlds it joined (`<app data>/friend-worlds.json`), for "Join your friend again".
pub const WORLDS_FILE: &str = "friend-worlds.json";
pub const TICKET_DAYS: u64 = 7;
/// At most this many removed logins remembered (so a removed friend's kept characters aren't listed again).
const REMOVED_MAX: usize = 200;
/// At most this many friends' worlds remembered on the friend's side (the newest kept).
pub const WORLDS_MAX: usize = 20;
const LINE_MAX: usize = 4096;
const CLAIMS_PER_MINUTE: u32 = 30;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

pub fn sha_hex(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

fn random(n: usize) -> Result<Vec<u8>, String> {
    use ring::rand::SecureRandom;
    let mut b = vec![0u8; n];
    ring::rand::SystemRandom::new().fill(&mut b).map_err(|_| "no random source on this PC".to_string())?;
    Ok(b)
}

// ------------------------------------------------------------------------------------------------ shapes

fn upper_digits(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Today's one-time invite login: `G` + 9 letters and digits (worldshare::new_guest).
pub fn ticket_ok(u: &str) -> bool {
    u.len() == 10 && u.starts_with('G') && upper_digits(u)
}

/// A lasting friend's login: `G` + 15 base32 (A-Z, 2-7).
pub fn lasting_ok(u: &str) -> bool {
    u.len() == 16 && u.starts_with('G') && u.bytes().skip(1).all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b))
}

/// Any friend's login on a world (the shapes the world's SQL also matches: [`FRIEND_REGEXP`]).
pub fn friend_login_ok(u: &str) -> bool {
    ticket_ok(u) || lasting_ok(u)
}

/// The SQL pattern of every friend's login (one-time and lasting), never the host's own.
pub const FRIEND_REGEXP: &str = "^G[A-Z0-9]{9}$|^G[A-Z2-7]{15}$";

/// A password the game and both consoles take: 8 to 16 capitals and digits.
pub fn pass_ok(p: &str) -> bool {
    (8..=16).contains(&p.len()) && upper_digits(p)
}

/// The friend's lasting login on the world `realm_id` for the peer `peer`.
pub fn name_for(realm_id: &str, peer: &str) -> String {
    let d = Sha256::digest(format!("leyline-world-friend-v1\n{realm_id}\n{peer}").as_bytes());
    format!("G{}", &crate::host::oneclick::base32(&d)[..15])
}

// ------------------------------------------------------------------------------------------------ the peer identity

/// This launcher's own identity: identity.rs's (one key, `<app data>/peer.key`, the same peer id the Friends list
/// uses), so a friend on the Leyline network and a friend with a lasting login on a world are the same person.
pub use crate::identity::PeerIdentity as Peer;

/// The peer id: base32(sha256(public key))[:16], lower case (identity.rs, the mesh's rule).
pub fn peer_id(public: &[u8; 32]) -> String {
    crate::identity::peer_id(public)
}

/// This launcher's identity in `app_dir` (identity.rs `load_or_create`: made the first time, a broken file refused).
pub fn peer_identity(app_dir: &Path) -> Result<Peer, String> {
    crate::identity::load_or_create(app_dir)
}

/// An Ed25519 signature check (identity.rs `verify_join`, the same raw check the join handshake uses).
pub fn verify(public: &[u8], msg: &[u8], sig: &[u8]) -> bool {
    crate::identity::verify_join(msg, public, sig)
}

// ------------------------------------------------------------------------------------------------ the claim

pub fn claim_key(secret: &str, realm_id: &str) -> [u8; 32] {
    let k = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
    let t = ring::hmac::sign(&k, format!("{CTX}\n{realm_id}").as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(t.as_ref());
    out
}

pub fn seal(key: &[u8; 32], plain: &[u8]) -> Result<String, String> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
    let n: [u8; 12] = random(12)?.try_into().map_err(|_| "no random source on this PC".to_string())?;
    let k = LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, key).map_err(|_| "bad key".to_string())?);
    let mut buf = plain.to_vec();
    k.seal_in_place_append_tag(Nonce::assume_unique_for_key(n), Aad::from(MAGIC.as_bytes()), &mut buf).map_err(|_| "sealing failed".to_string())?;
    let mut out = n.to_vec();
    out.extend(buf);
    Ok(hex(&out))
}

pub fn open(key: &[u8; 32], text: &str) -> Option<Vec<u8>> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
    let b = unhex(text.trim())?;
    if b.len() < 12 + 16 {
        return None;
    }
    let (n, ct) = b.split_at(12);
    let k = LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, key).ok()?);
    let nonce = Nonce::try_assume_unique_for_key(n).ok()?;
    let mut buf = ct.to_vec();
    let plain = k.open_in_place(nonce, Aad::from(MAGIC.as_bytes()), &mut buf).ok()?;
    Some(plain.to_vec())
}

/// What the friend's signature covers.
pub fn signed_text(realm_id: &str, chal: &str, ticket: &str, pass: &str) -> String {
    format!("{CTX}\n{realm_id}\n{chal}\n{ticket}\n{}", sha_hex(pass.as_bytes()))
}

/// The friend's side: the sealed claim for the host's challenge `chal`.
pub fn make_claim(peer: &Peer, realm_id: &str, secret: &str, chal: &str, ticket: Option<(&str, &str)>, pass: &str, old: &[String]) -> Result<String, String> {
    let (t, tp) = ticket.unwrap_or(("", ""));
    let sig = peer.sign(signed_text(realm_id, chal, t, pass).as_bytes());
    let doc = json!({ "v": 1, "pk": peer.public_hex(), "sig": hex(&sig), "pass": pass, "ticket": t, "tpass": tp, "old": old });
    seal(&claim_key(secret, realm_id), doc.to_string().as_bytes())
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ClaimDoc {
    v: u32,
    pk: String,
    sig: String,
    pass: String,
    ticket: String,
    tpass: String,
    old: Vec<String>,
}

/// A claim the host could read and whose signature holds. Its Debug never shows a password.
#[derive(Clone, PartialEq, Eq)]
pub struct Opened {
    pub peer: String,
    pub key: String,
    pub pass: String,
    pub ticket: String,
    pub tpass: String,
    pub old: Vec<String>,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened").field("peer", &self.peer).field("ticket", &self.ticket).field("old", &self.old).finish_non_exhaustive()
    }
}

/// The host's side: the claim read and checked (the signature over this challenge, the shapes).
pub fn open_claim(realm_id: &str, secret: &str, chal: &str, sealed: &str) -> Result<Opened, String> {
    let plain = open(&claim_key(secret, realm_id), sealed).ok_or("That isn't this world's invite.")?;
    let c: ClaimDoc = serde_json::from_slice(&plain).map_err(|_| "The claim didn't read.".to_string())?;
    if c.v != 1 {
        return Err("A newer Leyline made this claim: update Leyline.".into());
    }
    let pk = unhex(&c.pk).filter(|k| k.len() == 32).ok_or("The claim's key didn't read.")?;
    let sig = unhex(&c.sig).filter(|s| s.len() == 64).ok_or("The claim's signature didn't read.")?;
    if !(c.ticket.is_empty() || ticket_ok(&c.ticket)) || !pass_ok(&c.pass) || !(c.tpass.is_empty() || pass_ok(&c.tpass)) {
        return Err("The claim didn't read.".into());
    }
    if !verify(&pk, signed_text(realm_id, chal, &c.ticket, &c.pass).as_bytes(), &sig) {
        return Err("The claim's signature doesn't hold.".into());
    }
    let pk: [u8; 32] = pk.try_into().map_err(|_| "The claim's key didn't read.".to_string())?;
    let old = c.old.into_iter().filter(|u| ticket_ok(u)).take(4).collect();
    Ok(Opened { peer: peer_id(&pk), key: hex(&pk), pass: c.pass, ticket: c.ticket, tpass: c.tpass, old })
}

// ------------------------------------------------------------------------------------------------ the book

/// A friend's lasting login on this world. `legacy`: an earlier one-time login kept because it has characters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Friend {
    pub user: String,
    pub peer: String,
    pub key: String,
    pub made: u64,
    pub seen: u64,
    pub trusted: bool,
    pub enabled: bool,
    pub legacy: bool,
}

/// A one-time invite login: its password's sha256 (the password is only in the link), and the identity that used it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ticket {
    pub user: String,
    pub hash: String,
    pub made: u64,
    pub expires: u64,
    pub by: String,
}

/// `<share>/friends.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Book {
    pub friends: Vec<Friend>,
    pub tickets: Vec<Ticket>,
    /// Logins of removed friends whose characters were kept (their login disabled): never folded back in as an
    /// earlier friend's; the same friend invited again gets theirs back ([`admit`]).
    pub removed: Vec<String>,
}

static BOOK_LOCK: Mutex<()> = Mutex::new(());

pub fn book_path(share: &Path) -> PathBuf {
    share.join(BOOK_FILE)
}

pub fn load(share: &Path) -> Book {
    std::fs::read_to_string(book_path(share)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(share: &Path, b: &Book) -> Result<(), String> {
    std::fs::create_dir_all(share).map_err(|e| e.to_string())?;
    let t = serde_json::to_string_pretty(b).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&book_path(share), t.as_bytes()).map_err(|e| e.to_string())
}

/// A new invite's one-time login kept (its password only as a hash), for [`TICKET_DAYS`].
pub fn add_ticket(share: &Path, user: &str, pass: &str, t: u64) -> Result<(), String> {
    let _g = BOOK_LOCK.lock().map_err(|_| "busy")?;
    let mut b = load(share);
    b.tickets.retain(|k| k.user != user);
    b.tickets.push(Ticket { user: user.into(), hash: sha_hex(pass.as_bytes()), made: t, expires: t + TICKET_DAYS * 86_400, by: String::new() });
    save(share, &b)
}

/// Earlier one-time logins that have characters (`with_chars`) become kept friends' logins (legacy): never deleted,
/// their password left as it was (their old link keeps working while the world is shared), until their friend's
/// launcher takes them over ([`admit`] with `old`). How many were added.
pub fn fold(b: &mut Book, with_chars: &[String], t: u64) -> usize {
    let mut n = 0;
    for u in with_chars.iter().filter(|u| ticket_ok(u)) {
        if !b.friends.iter().any(|f| &f.user == u) && !b.removed.contains(u) {
            b.friends.push(Friend { user: u.clone(), made: t, seen: t, enabled: true, legacy: true, ..Default::default() });
            n += 1;
        }
    }
    n
}

/// One-time logins to delete now: out of time (or all: sharing stopped), never a friend's.
pub fn due_tickets(b: &mut Book, t: u64, all: bool) -> Vec<String> {
    let friends: Vec<String> = b.friends.iter().map(|f| f.user.clone()).collect();
    let (out, keep): (Vec<Ticket>, Vec<Ticket>) = b.tickets.drain(..).partition(|k| all || k.expires <= t);
    b.tickets = keep;
    out.into_iter().map(|k| k.user).filter(|u| !friends.contains(u)).collect()
}

/// The login a claim gets, and whether it has to be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Granted {
    pub user: String,
    pub create: bool,
    pub adopted: bool,
}

/// Whether `o` comes in at `t`, and as whom (the book changed to match; saved by the caller once the world agreed).
pub fn admit(b: &mut Book, realm_id: &str, o: &Opened, t: u64) -> Result<Granted, String> {
    let thash = sha_hex(o.tpass.as_bytes());
    let ti = if o.ticket.is_empty() { None } else { b.tickets.iter().position(|k| k.user == o.ticket && k.hash == thash) };
    let good = ti.is_some_and(|i| b.tickets[i].expires > t && (b.tickets[i].by.is_empty() || b.tickets[i].by == o.peer));
    let g = if let Some(i) = b.friends.iter().position(|f| f.peer == o.peer) {
        let f = &mut b.friends[i];
        if !(good || f.trusted || f.enabled) {
            return Err("Your friend's world isn't open to you right now: ask them to invite you again.".into());
        }
        f.enabled = true;
        f.seen = t;
        Granted { user: f.user.clone(), create: false, adopted: false }
    } else {
        if !good {
            return Err("This invite was used already or ran out: ask your friend for a new one.".into());
        }
        if let Some(i) = b.friends.iter().position(|f| f.legacy && f.peer.is_empty() && o.old.contains(&f.user)) {
            let f = &mut b.friends[i];
            f.peer = o.peer.clone();
            f.key = o.key.clone();
            f.enabled = true;
            f.seen = t;
            Granted { user: f.user.clone(), create: false, adopted: true }
        } else if let Some(u) = o.old.iter().find(|u| b.removed.contains(u)).cloned() {
            // an earlier friend's login removed with its characters kept, and that friend invited again: theirs again
            b.removed.retain(|x| *x != u);
            b.friends.push(Friend { user: u.clone(), peer: o.peer.clone(), key: o.key.clone(), made: t, seen: t, enabled: true, legacy: true, ..Default::default() });
            Granted { user: u, create: false, adopted: true }
        } else {
            let user = name_for(realm_id, &o.peer);
            if b.friends.iter().any(|f| f.user == user) {
                return Err("That login is taken on this world.".into());
            }
            // removed before with their characters kept: the login is still there (a new password only)
            let back = b.removed.contains(&user);
            b.removed.retain(|x| *x != user);
            b.friends.push(Friend { user: user.clone(), peer: o.peer.clone(), key: o.key.clone(), made: t, seen: t, enabled: true, ..Default::default() });
            Granted { user, create: !back, adopted: back }
        }
    };
    if let (Some(i), true) = (ti, good) {
        b.tickets[i].by = o.peer.clone();
    }
    Ok(g)
}

// ------------------------------------------------------------------------------------------------ the console lines

/// Which console a world has: the Built-in engine's (AzerothCore) or Leyline Solo's (CMaNGOS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    Wrath,
    Classic,
}

pub fn create_lines(era: Era, user: &str, pass: &str) -> Result<Vec<String>, String> {
    if !friend_login_ok(user) || !pass_ok(pass) {
        return Err("That login doesn't look right.".into());
    }
    let mut v = vec![format!("account create {user} {pass}")];
    if era == Era::Wrath {
        v.push(format!("account set addon {user} 2"));
    }
    v.push(password_line(user, pass)?);
    Ok(v)
}

pub fn password_line(user: &str, pass: &str) -> Result<String, String> {
    if !friend_login_ok(user) || !pass_ok(pass) {
        return Err("That login doesn't look right.".into());
    }
    Ok(format!("account set password {user} {pass} {pass}"))
}

pub fn delete_line(user: &str) -> Result<String, String> {
    if !friend_login_ok(user) {
        return Err("That login doesn't look right.".into());
    }
    Ok(format!("account delete {user}"))
}

/// The console's answer to `account set password`.
pub fn changed(answer: &str) -> Result<(), String> {
    if answer.to_ascii_lowercase().contains("chang") {
        Ok(())
    } else if answer.trim().is_empty() {
        Err("the world gave no answer".into())
    } else {
        Err(answer.trim().chars().take(120).collect())
    }
}

pub type Console = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;
pub type WithChars = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

/// A world as its claims need it: its realm id and invite secret, its share folder, its console, and the
/// friend-shaped logins that have characters there.
#[derive(Clone)]
pub struct World {
    pub realm_id: String,
    pub secret: String,
    pub share: PathBuf,
    pub era: Era,
    pub console: Console,
    pub with_chars: WithChars,
}

fn run(w: &World, line: &str) -> Result<(), String> {
    let a = (w.console)(line)?;
    if line.starts_with("account create") {
        crate::host::worldshare::created(&a)
    } else if line.starts_with("account set password") {
        changed(&a)
    } else {
        Ok(())
    }
}

/// One claim, answered: the login to use (made or given the claim's password), or why not.
pub fn handle(w: &World, chal: &str, sealed: &str) -> Result<String, String> {
    let o = open_claim(&w.realm_id, &w.secret, chal, sealed)?;
    let _g = BOOK_LOCK.lock().map_err(|_| "busy")?;
    let mut b = load(&w.share);
    fold(&mut b, &(w.with_chars)(), now());
    let g = admit(&mut b, &w.realm_id, &o, now())?;
    let lines = if g.create { create_lines(w.era, &g.user, &o.pass)? } else { vec![password_line(&g.user, &o.pass)?] };
    for (i, l) in lines.iter().enumerate() {
        if let Err(e) = run(w, l) {
            // made before (a world restored from a backup, a book lost): the login is there, so a new password only
            if i == 0 && g.create && run(w, &password_line(&g.user, &o.pass)?).is_ok() {
                break;
            }
            return Err(format!("The world didn't take your login: {e}"));
        }
    }
    save(&w.share, &b)?;
    Ok(g.user)
}

/// A random password nobody holds: the login can't be used until its friend's next claim.
fn scramble(w: &World, user: &str) -> Result<(), String> {
    let r = crate::host::engine::secret(16)?;
    run(w, &password_line(user, &r)?)
}

/// Sharing stopped: every lasting login disabled (a password nobody holds), not invited any more unless trusted;
/// one-time logins with no characters deleted (one with characters kept as an earlier friend's). How many logins
/// were disabled or deleted.
pub fn share_stopped(w: &World) -> usize {
    let Ok(_g) = BOOK_LOCK.lock() else { return 0 };
    let mut b = load(&w.share);
    let t = now();
    fold(&mut b, &(w.with_chars)(), t);
    let mut n = 0;
    for f in b.friends.iter_mut().filter(|f| !f.legacy) {
        f.enabled = false;
        if scramble(w, &f.user).is_ok() {
            n += 1;
        }
    }
    for u in due_tickets(&mut b, t, true) {
        if delete_line(&u).and_then(|l| run(w, &l)).is_ok() {
            n += 1;
        }
    }
    let _ = save(&w.share, &b);
    n
}

/// One-time logins out of time removed (those with characters kept as earlier friends' logins).
pub fn sweep(w: &World) -> usize {
    let Ok(_g) = BOOK_LOCK.lock() else { return 0 };
    let mut b = load(&w.share);
    let t = now();
    fold(&mut b, &(w.with_chars)(), t);
    let mut n = 0;
    for u in due_tickets(&mut b, t, false) {
        if delete_line(&u).and_then(|l| run(w, &l)).is_ok() {
            n += 1;
        }
    }
    let _ = save(&w.share, &b);
    n
}

/// What the host sees when the world couldn't make a change (most often: it isn't running).
fn world_down(e: String) -> String {
    format!("Your world has to be running for that (start it, then try again). It said: {e}")
}

/// The host's choice on one friend: "trust", "untrust", "remove" (characters kept: the login disabled and
/// forgotten; the same identity invited again gets it back) or "delete" (the login and its characters deleted).
pub fn set_friend(w: &World, user: &str, action: &str) -> Result<String, String> {
    let _g = BOOK_LOCK.lock().map_err(|_| "busy")?;
    let mut b = load(&w.share);
    fold(&mut b, &(w.with_chars)(), now());
    let i = b.friends.iter().position(|f| f.user == user).ok_or("That friend isn't on this world.")?;
    let msg = match action {
        "trust" => {
            b.friends[i].trusted = true;
            "Trusted: they can come back any time this world is shared, without a new invite.".to_string()
        }
        "untrust" => {
            b.friends[i].trusted = false;
            "Not trusted: they need an invite again after you stop sharing.".to_string()
        }
        "remove" => {
            scramble(w, user).map_err(world_down)?;
            b.friends.remove(i);
            b.removed.retain(|x| x != user);
            b.removed.push(user.to_string());
            let n = b.removed.len();
            if n > REMOVED_MAX {
                b.removed.drain(..n - REMOVED_MAX);
            }
            "Removed. Their characters stay in your world: invite them again and they get them back.".to_string()
        }
        "delete" => {
            run(w, &delete_line(user)?).map_err(world_down)?;
            b.friends.remove(i);
            b.removed.retain(|x| x != user);
            "Removed, and their characters on your world are deleted.".to_string()
        }
        _ => return Err("Trust, untrust, remove or delete.".into()),
    };
    save(&w.share, &b)?;
    Ok(msg)
}

// ------------------------------------------------------------------------------------------------ the front

struct Front {
    stop: Arc<AtomicBool>,
    port: u16,
    to: u16,
    world: Arc<Mutex<Option<World>>>,
}

static FRONT: Mutex<Option<Front>> = Mutex::new(None);
static RATE: Mutex<(u64, u32)> = Mutex::new((0, 0));

/// The claim answerer in front of the world's login server, on 127.0.0.1 (the tunnel and the same-network forwarder
/// point at its port; the game's own bytes pass through to `to_login` untouched). Running for this login port: the
/// world it answers for is updated. Its port.
pub fn front_start(to_login: u16, w: World) -> Result<u16, String> {
    let mut g = FRONT.lock().map_err(|_| "busy")?;
    if let Some(f) = g.as_ref() {
        if f.to == to_login && !f.stop.load(Ordering::Relaxed) {
            if let Ok(mut x) = f.world.lock() {
                *x = Some(w);
            }
            return Ok(f.port);
        }
        f.stop.store(true, Ordering::Relaxed);
    }
    let f = spawn_front(to_login, w)?;
    let port = f.port;
    *g = Some(f);
    Ok(port)
}

fn spawn_front(to_login: u16, w: World) -> Result<Front, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = l.local_addr().map_err(|e| e.to_string())?.port();
    let _ = l.set_nonblocking(true);
    let stop = Arc::new(AtomicBool::new(false));
    let world = Arc::new(Mutex::new(Some(w)));
    {
        let (stop, world) = (stop.clone(), world.clone());
        std::thread::spawn(move || front_loop(l, to_login, world, stop));
    }
    Ok(Front { stop, port, to: to_login, world })
}

pub fn front_stop() {
    if let Ok(mut g) = FRONT.lock() {
        if let Some(f) = g.take() {
            f.stop.store(true, Ordering::Relaxed);
        }
    }
}

fn front_loop(l: TcpListener, to: u16, world: Arc<Mutex<Option<World>>>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        match l.accept() {
            Ok((c, _)) => {
                let (world, stop) = (world.clone(), stop.clone());
                std::thread::spawn(move || front_one(c, to, world, stop));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn front_one(c: TcpStream, to: u16, world: Arc<Mutex<Option<World>>>, stop: Arc<AtomicBool>) {
    let _ = c.set_nonblocking(false);
    let _ = c.set_read_timeout(Some(Duration::from_secs(15)));
    let mut first = [0u8; 1];
    if !matches!(c.peek(&mut first), Ok(1)) {
        return;
    }
    if first[0] == b'L' {
        let w = world.lock().ok().and_then(|g| g.clone());
        if let Some(w) = w {
            let _ = converse(c, &|chal: &str, sealed: &str| handle(&w, chal, sealed));
        }
        return;
    }
    let _ = c.set_read_timeout(None);
    if let Ok(up) = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], to)), Duration::from_secs(3)) {
        crate::host::worldshare::relay_pair(c, up, None, stop);
    }
}

fn read_line(c: &mut impl Read, max: usize) -> Option<String> {
    let mut out = Vec::new();
    let mut b = [0u8; 1];
    while out.len() < max {
        match c.read(&mut b) {
            Ok(1) if b[0] == b'\n' => return String::from_utf8(out).ok(),
            Ok(1) => out.push(b[0]),
            _ => return None,
        }
    }
    None
}

fn rate_ok() -> bool {
    let Ok(mut g) = RATE.lock() else { return false };
    let m = now() / 60;
    if g.0 != m {
        *g = (m, 0);
    }
    g.1 += 1;
    g.1 <= CLAIMS_PER_MINUTE
}

/// The host's side of one claim conversation (`answer`: challenge, sealed claim -> login or why not).
pub fn converse<S: Read + Write>(mut c: S, answer: &dyn Fn(&str, &str) -> Result<String, String>) -> Result<(), String> {
    if !rate_ok() {
        let _ = c.write_all(format!("{MAGIC} NO Too many tries: wait a minute.\n").as_bytes());
        return Err("busy".into());
    }
    let hello = read_line(&mut c, 64).ok_or("no hello")?;
    if hello.trim_end_matches('\r') != format!("{MAGIC} HELLO") {
        return Err("not a claim".into());
    }
    let chal = hex(&random(16)?);
    c.write_all(format!("{MAGIC} CHAL {chal}\n").as_bytes()).map_err(|e| e.to_string())?;
    let line = read_line(&mut c, LINE_MAX).ok_or("no claim")?;
    let sealed = line.trim_end_matches('\r').strip_prefix(&format!("{MAGIC} CLAIM ")).ok_or("not a claim")?.to_string();
    let reply = match answer(&chal, &sealed) {
        Ok(u) => format!("{MAGIC} OK {u}\n"),
        Err(e) => format!("{MAGIC} NO {}\n", e.replace('\n', " ").replace('\r', " ").chars().take(160).collect::<String>()),
    };
    c.write_all(reply.as_bytes()).map_err(|e| e.to_string())
}

/// The friend's side of the conversation over `c`: (lasting login, its new one-time password).
pub fn claim_with<S: Read + Write>(mut c: S, peer: &Peer, realm_id: &str, secret: &str, ticket: Option<(&str, &str)>, old: &[String]) -> Result<(String, String), String> {
    let pass = crate::host::engine::secret(12)?;
    c.write_all(format!("{MAGIC} HELLO\n").as_bytes()).map_err(|e| e.to_string())?;
    let l = read_line(&mut c, 128).ok_or("The world didn't answer (an older Leyline on your friend's PC).")?;
    let chal = l.trim_end_matches('\r').strip_prefix(&format!("{MAGIC} CHAL ")).filter(|h| h.len() == 32 && unhex(h).is_some()).ok_or("The world didn't answer as expected.")?.to_string();
    let sealed = make_claim(peer, realm_id, secret, &chal, ticket, &pass, old)?;
    c.write_all(format!("{MAGIC} CLAIM {sealed}\n").as_bytes()).map_err(|e| e.to_string())?;
    let r = read_line(&mut c, 256).ok_or("The world didn't answer.")?;
    let r = r.trim_end_matches('\r');
    if let Some(u) = r.strip_prefix(&format!("{MAGIC} OK ")) {
        if friend_login_ok(u.trim()) {
            return Ok((u.trim().to_string(), pass));
        }
    }
    Err(r.strip_prefix(&format!("{MAGIC} NO ")).unwrap_or("The world didn't answer as expected.").chars().take(160).collect())
}

/// [`claim_with`] over TCP to the world's login address ("host" or "host:port"; 3724 when none).
pub fn claim_at(addr: &str, peer: &Peer, realm_id: &str, secret: &str, ticket: Option<(&str, &str)>, old: &[String]) -> Result<(String, String), String> {
    let a = if addr.rsplit_once(':').is_some_and(|(h, p)| !h.is_empty() && p.parse::<u16>().is_ok()) { addr.to_string() } else { format!("{addr}:3724") };
    let down = |e: String| format!("it didn't answer ({e}). Ask your friend to start their world and share it (Invite a friend).");
    let sa = a.to_socket_addrs().map_err(|e| down(e.to_string()))?.next().ok_or_else(|| down("no address".into()))?;
    let c = TcpStream::connect_timeout(&sa, Duration::from_secs(5)).map_err(|e| down(e.to_string()))?;
    let _ = c.set_read_timeout(Some(Duration::from_secs(8)));
    let _ = c.set_write_timeout(Some(Duration::from_secs(8)));
    claim_with(c, peer, realm_id, secret, ticket, old)
}

/// The claim answerer runs (the world is shared through it).
pub fn front_running() -> bool {
    FRONT.lock().ok().is_some_and(|g| g.as_ref().is_some_and(|f| !f.stop.load(Ordering::Relaxed)))
}

// ------------------------------------------------------------------------------------------------ the game's Leyline tab

// The in-game Leyline tab of the Friends window (WrathUnleashedUI/WorldFriends.lua) talks to the realm script
// (lua_scripts/ov_worldfriends.lua, on only in this PC's worlds: OV_WORLDFRIENDS=1), and the script and the launcher
// meet in three small tables of the world's characters database (the game can't reach the launcher; the launcher
// already reads and writes that database):
//   ov_world_asks     the game's asks (the host's "Invite a friend"): the launcher runs them within seconds and
//                     writes the answer ("ok:<text>" / "err:<text>");
//   ov_world_share    k/v: sharing (0/1) and link (this world's invite link, for the host only);
//   ov_world_friends  the lasting friends (account, login, trusted, enabled): the script lists them with their
//                     characters and online state, and tells a friend from the host.

pub const INGAME_TABLES: &str = "CREATE TABLE IF NOT EXISTS ov_world_share (k VARCHAR(16) NOT NULL PRIMARY KEY, \
    v VARCHAR(512) NOT NULL DEFAULT '', at INT UNSIGNED NOT NULL DEFAULT 0); \
    CREATE TABLE IF NOT EXISTS ov_world_friends (account INT UNSIGNED NOT NULL PRIMARY KEY, username VARCHAR(32) NOT NULL, \
    trusted TINYINT NOT NULL DEFAULT 0, enabled TINYINT NOT NULL DEFAULT 0); \
    CREATE TABLE IF NOT EXISTS ov_world_asks (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, kind VARCHAR(16) NOT NULL, \
    account INT UNSIGNED NOT NULL DEFAULT 0, made INT UNSIGNED NOT NULL DEFAULT 0, done TINYINT NOT NULL DEFAULT 0, \
    answer VARCHAR(255) NOT NULL DEFAULT '');";

/// The asks waiting (oldest first, a few at a time).
pub const ASKS_SQL: &str = "SELECT id, kind FROM ov_world_asks WHERE done = 0 ORDER BY id LIMIT 4";

pub fn parse_asks(rows: &str) -> Vec<(u64, String)> {
    rows.lines()
        .filter_map(|l| {
            let (id, kind) = l.trim_end_matches('\r').split_once('\t')?;
            let kind = kind.trim();
            (kind.len() <= 16 && kind.bytes().all(|b| b.is_ascii_lowercase())).then(|| Some((id.trim().parse().ok()?, kind.to_string())))?
        })
        .collect()
}

/// Only the characters an invite link has (so it can go into SQL and the game as it is).
pub fn link_ok(l: &str) -> bool {
    l.starts_with("https://") && l.len() <= 500 && l.bytes().all(|b| b.is_ascii_alphanumeric() || b"/:.#~=_-".contains(&b))
}

fn plain_text(t: &str, n: usize) -> String {
    t.chars().filter(|c| c.is_ascii_alphanumeric() || " .,:!?()-".contains(*c)).take(n).collect()
}

/// Sharing on with this link, or off (the link cleared).
pub fn share_sql(on: bool, link: &str, t: u64) -> String {
    let link = if on && link_ok(link) { link } else { "" };
    format!("REPLACE INTO ov_world_share (k, v, at) VALUES ('sharing', '{}', {t}), ('link', '{link}', {t});", if on { 1 } else { 0 })
}

/// Sharing on or off, the link left as it is.
pub fn sharing_sql(on: bool, t: u64) -> String {
    format!("REPLACE INTO ov_world_share (k, v, at) VALUES ('sharing', '{}', {t});", if on { 1 } else { 0 })
}

pub fn done_sql(id: u64, ok: bool, text: &str) -> String {
    format!("UPDATE ov_world_asks SET done = 1, answer = '{}:{}' WHERE id = {id};", if ok { "ok" } else { "err" }, plain_text(text, 200))
}

/// The friends table as the book has it (one transaction: the script never reads it half written).
pub fn friends_sql(friends: &[Friend]) -> String {
    let mut s = String::from("START TRANSACTION; DELETE FROM ov_world_friends; ");
    for f in friends.iter().filter(|f| friend_login_ok(&f.user)).take(50) {
        s.push_str(&format!(
            "INSERT IGNORE INTO ov_world_friends (account, username, trusted, enabled) SELECT id, username, {}, {} FROM acore_auth.account WHERE username = '{}'; ",
            f.trusted as u8, f.enabled as u8, f.user
        ));
    }
    s.push_str("COMMIT;");
    s
}

// ------------------------------------------------------------------------------------------------ the friend's side

/// The link says its world answers claims.
pub fn marked(link: &str) -> bool {
    link.split_once('#').is_some_and(|(_, f)| f.split("~~").skip(2).any(|p| p == LINK_MARK))
}

pub fn with_mark(link: &str) -> String {
    format!("{link}~~{LINK_MARK}")
}

/// A world this launcher joined as a friend. Its Debug never shows the invite secret.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Joined {
    pub relay: String,
    pub secret: String,
    pub lan: String,
    pub user: String,
    pub old: Vec<String>,
    pub name: String,
    pub at: u64,
}

impl std::fmt::Debug for Joined {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Joined").field("relay", &self.relay).field("user", &self.user).field("name", &self.name).finish_non_exhaustive()
    }
}

/// What the launcher's page sees of one (never the secret).
#[derive(Debug, Clone, Serialize)]
pub struct JoinedView {
    pub realm_id: String,
    pub name: String,
    pub user: String,
    pub at: u64,
}

pub fn worlds(app_dir: &Path) -> BTreeMap<String, Joined> {
    std::fs::read_to_string(app_dir.join(WORLDS_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn worlds_view(app_dir: &Path) -> Vec<JoinedView> {
    let mut v: Vec<JoinedView> = worlds(app_dir).into_iter().map(|(rid, j)| JoinedView { realm_id: rid, name: j.name, user: j.user, at: j.at }).collect();
    v.sort_by(|a, b| b.at.cmp(&a.at));
    v
}

/// Kept (or updated) after a join: the login it used, and every one-time login it used before (for [`admit`]'s
/// `old`).
pub fn remember(app_dir: &Path, realm_id: &str, mut j: Joined) -> Result<(), String> {
    let mut all = worlds(app_dir);
    if let Some(was) = all.get(realm_id) {
        for u in was.old.iter().chain(std::iter::once(&was.user)) {
            if ticket_ok(u) && !j.old.contains(u) {
                j.old.push(u.clone());
            }
        }
    }
    j.old.retain(|u| ticket_ok(u));
    j.old.truncate(4);
    all.insert(realm_id.to_string(), j);
    // the newest WORLDS_MAX kept
    while all.len() > WORLDS_MAX {
        let Some(oldest) = all.iter().min_by_key(|(_, w)| w.at).map(|(k, _)| k.clone()) else { break };
        all.remove(&oldest);
    }
    write_worlds(app_dir, &all)
}

fn write_worlds(app_dir: &Path, all: &BTreeMap<String, Joined>) -> Result<(), String> {
    let t = serde_json::to_string_pretty(all).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&app_dir.join(WORLDS_FILE), t.as_bytes()).map_err(|e| e.to_string())
}

/// A remembered friend's world forgotten on this PC (the friend's own world keeps the character).
pub fn forget(app_dir: &Path, realm_id: &str) -> Result<(), String> {
    let mut all = worlds(app_dir);
    if all.remove(realm_id).is_none() {
        return Err("This PC hasn't joined that world.".into());
    }
    write_worlds(app_dir, &all)
}

/// The link to join a remembered world again (no ticket: the claim alone).
pub fn again_link(realm_id: &str, j: &Joined) -> String {
    let mut l = format!("{}~~", crate::host::litenet::invite_link(&j.relay, realm_id, &j.secret, None));
    if !j.lan.is_empty() {
        l.push_str(&format!("~~lan={}", j.lan));
    }
    with_mark(&l)
}

/// The account name the game folder signs in with now (`WTF/Config.wtf`), when it is a one-time friend's login: a
/// friend whose older launcher joined with it brings it as `old`.
pub fn config_ticket(game: &Path) -> Option<String> {
    let t = std::fs::read_to_string(game.join("WTF").join("Config.wtf")).ok()?;
    t.lines().find_map(|l| {
        let l = l.trim();
        let rest = l.strip_prefix("SET ")?;
        let (k, v) = rest.split_once(' ')?;
        (k.eq_ignore_ascii_case("accountName")).then(|| v.trim().trim_matches('"').to_ascii_uppercase())
    }).filter(|u| ticket_ok(u))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(n: u8) -> Peer {
        Peer::from_seed([n; 32]).unwrap()
    }

    const RID: &str = "kzdvvj2umnduyauf";
    const SECRET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";

    fn claim(p: &Peer, chal: &str, ticket: Option<(&str, &str)>, pass: &str, old: &[String]) -> Opened {
        let s = make_claim(p, RID, SECRET, chal, ticket, pass, old).unwrap();
        open_claim(RID, SECRET, chal, &s).unwrap()
    }

    #[test]
    fn the_lasting_login_is_derived_from_the_peer_and_the_world() {
        let a = peer(1);
        assert_eq!(a.peer_id.len(), 16);
        assert_eq!(a.peer_id, peer(1).peer_id, "the same key: the same peer id");
        let n = name_for(RID, &a.peer_id);
        assert!(lasting_ok(&n) && friend_login_ok(&n) && !ticket_ok(&n), "{n}");
        assert_eq!(n, name_for(RID, &peer(1).peer_id), "the same on every join");
        assert_ne!(n, name_for("aaaaaaaaaaaaaaaa", &a.peer_id), "another world: another name");
        assert_ne!(n, name_for(RID, &peer(2).peer_id), "another friend: another name");
        assert!(ticket_ok("GABCDE2345") && !lasting_ok("GABCDE2345"));
        assert!(!friend_login_ok("SOLO") && !friend_login_ok("HKABCDEFGHIJKLMN") && !friend_login_ok("G1; drop"));
        let d = tempfile::tempdir().unwrap();
        let p1 = peer_identity(d.path()).unwrap();
        assert_eq!(p1.peer_id, peer_identity(d.path()).unwrap().peer_id, "peer.key is made once and kept");
        assert!(d.path().join(PEER_KEY_FILE).is_file());
        // one identity: the Friends list (identity.rs) and a world's lasting logins read the same key, the same id
        let me = crate::identity::load_or_create(d.path()).unwrap();
        assert_eq!((me.peer_id.as_str(), me.public_key), (p1.peer_id.as_str(), p1.public_key));
        assert_eq!(name_for(RID, &me.peer_id), name_for(RID, &p1.peer_id));
        // a claim signed with it opens as that friend's peer id
        let s = make_claim(&me, RID, SECRET, "00ff", None, "PASSWORD2345", &[]).unwrap();
        assert_eq!(open_claim(RID, SECRET, "00ff", &s).unwrap().peer, me.peer_id);
        // a broken key file is refused here too, never replaced (a new key would be a new person to every friend)
        std::fs::write(d.path().join(PEER_KEY_FILE), "nope").unwrap();
        assert!(peer_identity(d.path()).is_err());
    }

    #[test]
    fn a_claim_is_sealed_with_the_invite_and_signed_over_the_challenge() {
        let a = peer(3);
        let s = make_claim(&a, RID, SECRET, "00ff", Some(("GABCDE2345", "ABCDEFGH2345")), "PASSWORD2345", &[]).unwrap();
        assert!(!s.contains("PASSWORD2345") && !s.to_ascii_uppercase().contains(&hex(b"PASSWORD2345").to_ascii_uppercase()), "nothing readable");
        let o = open_claim(RID, SECRET, "00ff", &s).unwrap();
        assert_eq!((o.peer.as_str(), o.ticket.as_str(), o.pass.as_str()), (a.peer_id.as_str(), "GABCDE2345", "PASSWORD2345"));
        assert!(!format!("{o:?}").contains("PASSWORD2345") && !format!("{o:?}").contains("ABCDEFGH2345"), "Debug never shows a password");
        assert!(open_claim(RID, "ANOTHERSECRETANOTHERSEC", "00ff", &s).is_err(), "another invite can't read it");
        assert!(open_claim("aaaaaaaaaaaaaaaa", SECRET, "00ff", &s).is_err(), "another world can't");
        assert_eq!(open_claim(RID, SECRET, "11ee", &s).unwrap_err(), "The claim's signature doesn't hold.", "a replay to another challenge");
        let mut bad = s.clone();
        let last = bad.pop().unwrap();
        bad.push(if last == '0' { '1' } else { '0' });
        assert!(open_claim(RID, SECRET, "00ff", &bad).is_err(), "tampered");
        assert!(make_claim(&a, RID, SECRET, "00ff", None, "bad pass", &[]).and_then(|s| open_claim(RID, SECRET, "00ff", &s)).is_err());
    }

    #[test]
    fn who_comes_in_and_as_whom() {
        let (a, b2) = (peer(5), peer(6));
        let mut book = Book::default();
        let t = 1_000_000;
        book.tickets.push(Ticket { user: "GTICKET234".into(), hash: sha_hex(b"TPASS2345678"), made: t, expires: t + 100, by: String::new() });
        // no ticket, unknown: out
        assert!(admit(&mut book, RID, &claim(&a, "aa", None, "PASSWORD2345", &[]), t).is_err());
        // the ticket: a new lasting login
        let g = admit(&mut book, RID, &claim(&a, "aa", Some(("GTICKET234", "TPASS2345678")), "PASSWORD2345", &[]), t).unwrap();
        assert_eq!(g, Granted { user: name_for(RID, &a.peer_id), create: true, adopted: false });
        // the same identity again, no ticket, while invited: the same login, made already
        let g2 = admit(&mut book, RID, &claim(&a, "bb", None, "PASSWORD9999", &[]), t + 5).unwrap();
        assert_eq!((g2.user, g2.create), (g.user.clone(), false));
        // the ticket is that identity's now: nobody else's
        assert!(admit(&mut book, RID, &claim(&b2, "cc", Some(("GTICKET234", "TPASS2345678")), "PASSWORD2345", &[]), t).is_err());
        // a wrong ticket password, an old ticket: out
        book.tickets.push(Ticket { user: "GTICKET999".into(), hash: sha_hex(b"TPASS2345678"), made: t, expires: t + 100, by: String::new() });
        assert!(admit(&mut book, RID, &claim(&b2, "cc", Some(("GTICKET999", "WRONG2345678")), "PASSWORD2345", &[]), t).is_err());
        assert!(admit(&mut book, RID, &claim(&b2, "cc", Some(("GTICKET999", "TPASS2345678")), "PASSWORD2345", &[]), t + 101).is_err());
        // sharing stopped: not invited until a new ticket, unless trusted
        book.friends[0].enabled = false;
        assert!(admit(&mut book, RID, &claim(&a, "dd", None, "PASSWORD2345", &[]), t + 10).is_err());
        book.friends[0].trusted = true;
        assert!(admit(&mut book, RID, &claim(&a, "dd", None, "PASSWORD2345", &[]), t + 10).is_ok());
    }

    #[test]
    fn todays_one_time_logins_with_characters_are_kept_and_taken_over() {
        let mut book = Book::default();
        let t = 2_000_000;
        book.tickets.push(Ticket { user: "GOLD234567".into(), hash: sha_hex(b"X"), made: 1, expires: 2, by: String::new() });
        book.tickets.push(Ticket { user: "GNOCHAR234".into(), hash: sha_hex(b"X"), made: 1, expires: 2, by: String::new() });
        book.tickets.push(Ticket { user: "GNEW234567".into(), hash: sha_hex(b"NEWPASS23456"), made: t, expires: t + 100, by: String::new() });
        assert_eq!(fold(&mut book, &["GOLD234567".into(), "SOLO".into()], t), 1, "only friend-shaped logins");
        assert_eq!(fold(&mut book, &["GOLD234567".into()], t), 0, "once");
        assert!(book.friends[0].legacy && book.friends[0].enabled);
        // out of time: the one without characters goes, the kept friend's never
        assert_eq!(due_tickets(&mut book.clone(), t, false), vec!["GNOCHAR234".to_string()]);
        assert_eq!(due_tickets(&mut book.clone(), t, true), vec!["GNOCHAR234".to_string(), "GNEW234567".to_string()]);
        // a new launcher of that friend brings the old login: it takes it over (its characters stay theirs)
        let a = peer(7);
        let g = admit(&mut book, RID, &claim(&a, "aa", Some(("GNEW234567", "NEWPASS23456")), "PASSWORD2345", &["GOLD234567".into()]), t).unwrap();
        assert_eq!(g, Granted { user: "GOLD234567".into(), create: false, adopted: true });
        assert_eq!(book.friends[0].peer, a.peer_id);
        // and the next time it is known by its identity
        assert_eq!(admit(&mut book, RID, &claim(&a, "bb", None, "PASSWORD2345", &[]), t).unwrap().user, "GOLD234567");
    }

    #[test]
    fn the_console_lines_for_a_friends_login() {
        let u = name_for(RID, &peer(1).peer_id);
        let v = create_lines(Era::Wrath, &u, "PASSWORD2345").unwrap();
        assert_eq!(v, vec![format!("account create {u} PASSWORD2345"), format!("account set addon {u} 2"), format!("account set password {u} PASSWORD2345 PASSWORD2345")]);
        assert_eq!(create_lines(Era::Classic, "GABCDE2345", "PASSWORD2345").unwrap().len(), 2);
        assert!(create_lines(Era::Wrath, "SOLO", "PASSWORD2345").is_err() && password_line(&u, "a b").is_err() && delete_line("ORIGINAL").is_err());
        assert_eq!(delete_line(&u).unwrap(), format!("account delete {u}"));
        assert!(changed("The password was changed").is_ok() && changed("").is_err() && changed("Account not exist").is_err());
    }

    /// A stand-in world: its console answers and remembers.
    fn world(share: &Path, log: Arc<Mutex<Vec<String>>>) -> World {
        World {
            realm_id: RID.into(),
            secret: SECRET.into(),
            share: share.to_path_buf(),
            era: Era::Wrath,
            console: Arc::new(move |l: &str| {
                log.lock().unwrap().push(l.to_string());
                Ok(if l.starts_with("account create") { "Account created".into() } else if l.starts_with("account set password") { "The password was changed".into() } else { "ok".into() })
            }),
            with_chars: Arc::new(Vec::<String>::new),
        }
    }

    #[test]
    fn the_whole_lifecycle_over_the_wire() {
        let d = tempfile::tempdir().unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let w = world(d.path(), log.clone());
        add_ticket(d.path(), "GTICKET234", "TPASS2345678", now()).unwrap();
        assert!(!std::fs::read_to_string(book_path(d.path())).unwrap().contains("TPASS2345678"), "a ticket's password is never kept");
        let front = spawn_front(1, w.clone()).unwrap();
        let port = front.port;
        let a = peer(9);
        let (u, p) = claim_at(&format!("127.0.0.1:{port}"), &a, RID, SECRET, Some(("GTICKET234", "TPASS2345678")), &[]).unwrap();
        assert_eq!(u, name_for(RID, &a.peer_id));
        assert!(pass_ok(&p));
        assert!(log.lock().unwrap().iter().any(|l| l == &format!("account create {u} {p}")));
        // again, with no ticket: a fresh one-time password on the same login
        let (u2, p2) = claim_at(&format!("127.0.0.1:{port}"), &a, RID, SECRET, None, &[]).unwrap();
        assert_eq!(u2, u);
        assert_ne!(p2, p);
        assert!(log.lock().unwrap().last().unwrap() == &format!("account set password {u} {p2} {p2}"));
        // the wrong invite: refused, nothing made
        let n = log.lock().unwrap().len();
        assert!(claim_at(&format!("127.0.0.1:{port}"), &peer(10), RID, "ZZZZZZZZZZZZZZZZZZZZZZZZ", None, &[]).is_err());
        // nobody there: said in plain words
        let gone = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let e = claim_at(&format!("127.0.0.1:{gone}"), &a, RID, SECRET, None, &[]).unwrap_err();
        assert!(e.contains("start their world and share it"), "{e}");
        assert_eq!(log.lock().unwrap().len(), n);
        // sharing stops: disabled (a password nobody holds), the used ticket's login deleted, not invited without a
        // new ticket
        let n = log.lock().unwrap().len();
        assert_eq!(share_stopped(&w), 2);
        let after: Vec<String> = log.lock().unwrap()[n..].to_vec();
        assert!(after[0].starts_with(&format!("account set password {u} ")) && !after[0].contains(&p2), "{after:?}");
        assert_eq!(after[1], "account delete GTICKET234");
        assert!(claim_at(&format!("127.0.0.1:{port}"), &a, RID, SECRET, None, &[]).is_err());
        // trusted: back in without one
        assert!(set_friend(&w, &u, "trust").is_ok());
        assert!(claim_at(&format!("127.0.0.1:{port}"), &a, RID, SECRET, None, &[]).is_ok());
        // removed, characters kept: forgotten; delete: the console deletes the login
        assert!(set_friend(&w, &u, "remove").is_ok() && load(d.path()).friends.is_empty());
        assert!(set_friend(&w, &u, "delete").is_err(), "not a friend any more");
        front.stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn the_games_own_bytes_pass_through_the_front() {
        let srv = TcpListener::bind("127.0.0.1:0").unwrap();
        let sp = srv.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut c, _)) = srv.accept() {
                let mut b = [0u8; 4];
                if c.read_exact(&mut b).is_ok() {
                    let _ = c.write_all(&b);
                }
            }
        });
        let d = tempfile::tempdir().unwrap();
        let front = spawn_front(sp, world(d.path(), Arc::new(Mutex::new(Vec::new())))).unwrap();
        let mut c = TcpStream::connect(("127.0.0.1", front.port)).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        c.write_all(&[0x00, 0x08, 0x2a, 0x00]).unwrap();
        let mut got = [0u8; 4];
        c.read_exact(&mut got).unwrap();
        assert_eq!(got, [0x00, 0x08, 0x2a, 0x00], "a login challenge reaches the login server untouched");
        front.stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn the_friends_side_remembers_its_worlds() {
        let d = tempfile::tempdir().unwrap();
        let j = Joined { relay: "relay.x".into(), secret: SECRET.into(), lan: "192.168.1.5:3724".into(), user: "GOLD234567".into(), name: "Single player".into(), at: 5, ..Default::default() };
        remember(d.path(), RID, j.clone()).unwrap();
        remember(d.path(), RID, Joined { user: name_for(RID, "x"), at: 9, ..j.clone() }).unwrap();
        let k = worlds(d.path()).remove(RID).unwrap();
        assert_eq!(k.old, vec!["GOLD234567".to_string()], "the one-time login it used before");
        assert!(!format!("{k:?}").contains(SECRET), "Debug never shows the invite secret");
        let l = again_link(RID, &k);
        assert!(marked(&l) && crate::host::litenet::guest_in(&l).is_none());
        assert_eq!(crate::realms::parse_invite(&l).map(|x| x.2), Some(SECRET.to_string()));
        assert_eq!(crate::host::denways::ways_in(&l).0.as_deref(), Some("192.168.1.5:3724"));
        assert!(!marked("https://r/join/x#AAAA~~GABC.DEF") && marked(&with_mark("https://r/join/x#AAAA~~GABC.DEF")));
        let v = worlds_view(d.path());
        assert_eq!(v.len(), 1);
        assert!(!serde_json::to_string(&v).unwrap().contains(SECRET));
        // the account an older launcher wrote into the game folder
        let g = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(g.path().join("WTF")).unwrap();
        std::fs::write(g.path().join("WTF").join("Config.wtf"), "SET gxApi \"d3d9\"\r\nSET accountName \"GOLD234567\"\r\n").unwrap();
        assert_eq!(config_ticket(g.path()).as_deref(), Some("GOLD234567"));
        std::fs::write(g.path().join("WTF").join("Config.wtf"), "SET accountName \"HKABCDEFGHIJKLMN\"\r\n").unwrap();
        assert_eq!(config_ticket(g.path()), None);
    }
    #[test]
    fn a_removed_friend_stays_removed_and_comes_back_to_their_characters() {
        let d = tempfile::tempdir().unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut w = world(d.path(), log.clone());
        // an earlier one-time friend with characters, and a lasting one
        w.with_chars = Arc::new(|| vec!["GOLD234567".to_string()]);
        let a = peer(11);
        add_ticket(d.path(), "GTICKET234", "TPASS2345678", now()).unwrap();
        let chal = "00ff";
        let u = handle(&w, chal, &make_claim(&a, RID, SECRET, chal, Some(("GTICKET234", "TPASS2345678")), "PASSWORD2345", &[]).unwrap()).unwrap();
        assert_eq!(load(d.path()).friends.len(), 2, "the kept earlier friend and the new one");
        // removed with characters kept: not listed again, even though the earlier login still has characters
        assert!(set_friend(&w, "GOLD234567", "remove").unwrap().contains("characters stay"));
        assert!(set_friend(&w, &u, "remove").is_ok());
        assert!(load(d.path()).friends.is_empty(), "{:?}", load(d.path()));
        assert!(sweep(&w) <= 1 && load(d.path()).friends.is_empty(), "never folded back in");
        // the same identity invited again: its login is still there, so only a new password (no second create)
        add_ticket(d.path(), "GTICKET567", "TPASS2345678", now()).unwrap();
        let n = log.lock().unwrap().len();
        let u2 = handle(&w, chal, &make_claim(&a, RID, SECRET, chal, Some(("GTICKET567", "TPASS2345678")), "PASSWORD7777", &[]).unwrap()).unwrap();
        assert_eq!(u2, u);
        let after: Vec<String> = log.lock().unwrap()[n..].to_vec();
        assert_eq!(after, vec![format!("account set password {u} PASSWORD7777 PASSWORD7777")]);
        // the earlier friend's newer launcher brings its old login: theirs again
        let b2 = peer(12);
        add_ticket(d.path(), "GTICKET890", "TPASS2345678", now()).unwrap();
        let u3 = handle(&w, chal, &make_claim(&b2, RID, SECRET, chal, Some(("GTICKET890", "TPASS2345678")), "PASSWORD8888", &["GOLD234567".into()]).unwrap()).unwrap();
        assert_eq!(u3, "GOLD234567");
        assert!(load(d.path()).removed.is_empty());
        // delete: gone with the characters, and not remembered as removed
        assert!(set_friend(&w, &u3, "delete").unwrap().contains("deleted"));
        assert!(log.lock().unwrap().last().unwrap() == "account delete GOLD234567");
        assert!(!load(d.path()).removed.contains(&"GOLD234567".to_string()));
    }

    #[test]
    fn a_login_made_before_gets_a_new_password_instead_of_failing() {
        let d = tempfile::tempdir().unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut w = world(d.path(), log.clone());
        let l2 = log.clone();
        w.console = Arc::new(move |l: &str| {
            l2.lock().unwrap().push(l.to_string());
            if l.starts_with("account create") { Ok("Account name is in use".into()) } else { Ok("The password was changed".into()) }
        });
        add_ticket(d.path(), "GTICKET234", "TPASS2345678", now()).unwrap();
        let a = peer(13);
        let u = handle(&w, "aa", &make_claim(&a, RID, SECRET, "aa", Some(("GTICKET234", "TPASS2345678")), "PASSWORD2345", &[]).unwrap()).unwrap();
        assert_eq!(u, name_for(RID, &a.peer_id));
        assert_eq!(log.lock().unwrap().last().unwrap(), &format!("account set password {u} PASSWORD2345 PASSWORD2345"));
    }

    #[test]
    fn a_change_the_world_cant_make_says_to_start_it() {
        let d = tempfile::tempdir().unwrap();
        let mut w = world(d.path(), Arc::new(Mutex::new(Vec::new())));
        let mut b = Book::default();
        b.friends.push(Friend { user: name_for(RID, "x"), enabled: true, ..Default::default() });
        save(d.path(), &b).unwrap();
        w.console = Arc::new(|_: &str| Err("not running".to_string()));
        let e = set_friend(&w, &name_for(RID, "x"), "remove").unwrap_err();
        assert!(e.contains("has to be running"), "{e}");
        assert_eq!(load(d.path()).friends.len(), 1, "nothing changed");
        assert!(set_friend(&w, &name_for(RID, "x"), "trust").is_ok(), "trust needs no running world");
        assert!(set_friend(&w, &name_for(RID, "x"), "nope").is_err());
    }

    #[test]
    fn the_friends_side_keeps_the_newest_worlds_and_forgets_one() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..(WORLDS_MAX + 5) {
            let rid = format!("{:a>16}", i);
            remember(d.path(), &rid, Joined { relay: "relay.x".into(), secret: SECRET.into(), user: name_for(&rid, "x"), name: "W".into(), at: 100 + i as u64, ..Default::default() }).unwrap();
        }
        let all = worlds(d.path());
        assert_eq!(all.len(), WORLDS_MAX);
        assert!(!all.contains_key(&format!("{:a>16}", 0)) && all.contains_key(&format!("{:a>16}", WORLDS_MAX + 4)), "the oldest went");
        let one = format!("{:a>16}", WORLDS_MAX + 4);
        forget(d.path(), &one).unwrap();
        assert!(!worlds(d.path()).contains_key(&one));
        assert!(forget(d.path(), &one).is_err());
    }

    #[test]
    fn the_games_tab_and_the_launcher_meet_in_the_worlds_database() {
        assert_eq!(parse_asks("3\tinvite\n4\tx;drop\n5\tinvite\r\nbad\n"), vec![(3, "invite".to_string()), (5, "invite".to_string())]);
        let l = "https://relay.leylinelauncher.com/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~GABCDE2345.PASSWORD2345~~lan=192.168.1.5:3724~~lf1";
        assert!(link_ok(l) && !link_ok("https://x/'; DROP") && !link_ok("javascript:x"));
        assert!(share_sql(true, l, 9).contains(l) && share_sql(false, l, 9).contains("('link', '', 9)"));
        assert!(!share_sql(true, "https://x/'; x", 9).contains("x/'"));
        assert_eq!(done_sql(4, false, "It's 'bad'; DROP"), "UPDATE ov_world_asks SET done = 1, answer = 'err:Its bad DROP' WHERE id = 4;");
        let f = vec![
            Friend { user: name_for(RID, "a"), trusted: true, enabled: true, ..Default::default() },
            Friend { user: "SOLO'; x".into(), ..Default::default() },
        ];
        let q = friends_sql(&f);
        assert!(q.starts_with("START TRANSACTION; DELETE FROM ov_world_friends;") && q.ends_with("COMMIT;"));
        assert!(q.contains(&format!("SELECT id, username, 1, 1 FROM acore_auth.account WHERE username = '{}'", f[0].user)) && !q.contains("SOLO"));
        assert!(INGAME_TABLES.contains("ov_world_asks") && INGAME_TABLES.contains("ov_world_share") && INGAME_TABLES.contains("ov_world_friends"));
    }
}
