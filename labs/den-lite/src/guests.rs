//! Friends in your Den: one guest login per invite (owner 1 Nov 14:37: "invite a friend through the Leyline network
//! ... no hassle for the friend to join").
//!
//! The host's own login (any name, the fixed local password) is for this PC only: it is refused on the remote
//! listeners (what the relay tunnel reaches) and from any peer that isn't loopback. A friend gets a login of their
//! own, made here for one invite:
//!   * the account: `G` + 9 random letters and digits; the password: 16 random letters and digits (the OS's random
//!     source). Both are printed once, for the invite link, and never written down here;
//!   * `<dens>/guests.json` keeps only its SRP6 salt and verifier (what a login server keeps), the friend's character
//!     name, when it was made, when it expires (24 hours by default) and whether it was taken back;
//!   * `den-lite guest add|list|revoke` (the launcher's Invite friends runs them) make, list and take back logins.
//!
//! Each guest has one ready-made character (the name the host gave, or one of [`NAMES`]), a guid of its own.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use wow_srp::normalized_string::NormalizedString;
use wow_srp::server::SrpVerifier;

pub const FILE: &str = "guests.json";
pub const DEFAULT_HOURS: u64 = 24;
pub const MAX_HOURS: u64 = 24 * 7;
/// At most this many live guest logins at once (older ones are dropped first).
pub const MAX_GUESTS: usize = 20;
/// Names for a friend's character when the host gives none.
pub const NAMES: &[&str] = &["Rowan", "Briar", "Ash", "Fen", "Moss", "Reed", "Wren", "Sorrel"];
/// Guest guids start here (the host's character is 1).
pub const GUID_BASE: u64 = 100;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guest {
    pub user: String,
    pub name: String,
    /// Hex, little endian (wow_srp's database values).
    pub salt: String,
    pub verifier: String,
    pub made: u64,
    pub expires: u64,
    #[serde(default)]
    pub revoked: bool,
    /// Its character's guid (stable for this login).
    pub guid: u64,
}

impl Guest {
    pub fn live(&self, now: u64) -> bool {
        !self.revoked && now < self.expires
    }
}

/// What `guest add` hands back, once.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Made {
    pub user: String,
    pub pass: String,
    pub name: String,
    pub expires: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Doc {
    #[serde(default)]
    guests: Vec<Guest>,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut o = [0u8; 32];
    for (i, b) in o.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(o)
}

pub fn list(dir: &Path) -> Vec<Guest> {
    let Ok(t) = fs::read_to_string(dir.join(FILE)) else { return Vec::new() };
    if t.len() > 256 * 1024 {
        return Vec::new();
    }
    serde_json::from_str::<Doc>(&t).map(|d| d.guests).unwrap_or_default()
}

fn save(dir: &Path, g: &[Guest]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!(".{FILE}.tmp"));
    let text = serde_json::to_string_pretty(&Doc { guests: g.to_vec() }).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).and_then(|_| fs::rename(&tmp, dir.join(FILE))).map_err(|e| format!("Couldn't write {FILE}: {e}"))
}

/// A live guest by account name (any case).
pub fn find(dir: &Path, user: &str, now: u64) -> Option<Guest> {
    list(dir).into_iter().find(|g| g.user.eq_ignore_ascii_case(user) && g.live(now))
}

/// Its SRP6 verifier, for the login server.
pub fn verifier(g: &Guest) -> Option<SrpVerifier> {
    let user = NormalizedString::new(&g.user).ok()?;
    Some(SrpVerifier::from_database_values(user, unhex32(&g.verifier)?, unhex32(&g.salt)?))
}

/// Random letters and digits from the OS's random source (A-Z and 2-9: nothing that reads alike).
pub fn random_word(n: usize) -> Result<String, String> {
    const ABC: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).map_err(|e| format!("no random source: {e}"))?;
    Ok(buf.iter().map(|b| ABC[(*b as usize) % ABC.len()] as char).collect())
}

/// A character name: 2 to 12 letters, first one upper case.
pub fn name_ok(s: &str) -> bool {
    (2..=12).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphabetic())
}

fn tidy_name(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase(),
        None => String::new(),
    }
}

/// A new guest login for one invite. `name`: the friend's character (else one of NAMES, not the host's), `hours`:
/// how long it works (1 to a week). Expired and taken-back logins are tidied away.
pub fn add(dir: &Path, name: Option<&str>, host_name: &str, hours: u64, now: u64) -> Result<Made, String> {
    let mut all: Vec<Guest> = list(dir).into_iter().filter(|g| g.live(now)).collect();
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) if name_ok(n) => tidy_name(n),
        Some(_) => return Err("A character name is 2 to 12 letters.".into()),
        None => {
            let taken: Vec<String> = all.iter().map(|g| g.name.clone()).chain([host_name.to_string()]).collect();
            NAMES.iter().find(|n| !taken.iter().any(|t| t.eq_ignore_ascii_case(n))).unwrap_or(&NAMES[0]).to_string()
        }
    };
    if name.eq_ignore_ascii_case(host_name) {
        return Err("That's your own character's name: pick another for your friend.".into());
    }
    let user = format!("G{}", random_word(9)?);
    let pass = random_word(16)?;
    let v = SrpVerifier::from_username_and_password(
        NormalizedString::new(&user).map_err(|_| "bad account name")?,
        NormalizedString::new(&pass).map_err(|_| "bad password")?,
    );
    let guid = (GUID_BASE..).find(|n| !all.iter().any(|g| g.guid == *n)).unwrap_or(GUID_BASE);
    let expires = now + hours.clamp(1, MAX_HOURS) * 3600;
    all.push(Guest { user: user.clone(), name: name.clone(), salt: hex(v.salt()), verifier: hex(v.password_verifier()), made: now, expires, revoked: false, guid });
    let n = all.len();
    all.drain(..n.saturating_sub(MAX_GUESTS));
    save(dir, &all)?;
    Ok(Made { user, pass, name, expires })
}

/// Take a login back (`all`: every one). How many were taken back.
pub fn revoke(dir: &Path, user: &str, now: u64) -> Result<usize, String> {
    let mut all = list(dir);
    let mut n = 0;
    for g in all.iter_mut().filter(|g| g.live(now) && (user == "all" || g.user.eq_ignore_ascii_case(user))) {
        g.revoked = true;
        n += 1;
    }
    save(dir, &all)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("den-lite-guests-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_login_per_invite_kept_only_as_a_verifier() {
        let d = dir("add");
        let a = add(&d, Some("rowan"), "Wren", 24, 1000).unwrap();
        let b = add(&d, None, "Wren", 24, 1000).unwrap();
        assert!(a.user.starts_with('G') && a.user.len() == 10 && a.pass.len() == 16 && a.user != b.user && a.pass != b.pass);
        assert_eq!((a.name.as_str(), b.name.as_str()), ("Rowan", "Briar"), "the host's own name and a taken one are skipped");
        let text = fs::read_to_string(d.join(FILE)).unwrap();
        assert!(!text.contains(&a.pass) && !text.contains(&b.pass), "never the password");
        let g = find(&d, &a.user.to_lowercase(), 2000).expect("live");
        assert_eq!((g.name.as_str(), g.guid), ("Rowan", GUID_BASE));
        assert_eq!(find(&d, &b.user, 2000).unwrap().guid, GUID_BASE + 1);
        assert!(verifier(&g).is_some());
        // expiry and taking back
        assert!(find(&d, &a.user, 1000 + 24 * 3600).is_none(), "expired");
        assert_eq!(revoke(&d, &b.user, 2000).unwrap(), 1);
        assert!(find(&d, &b.user, 2000).is_none());
        assert_eq!(revoke(&d, "all", 2000).unwrap(), 1);
        assert!(list(&d).iter().all(|g| !g.live(2000)));
        assert!(add(&d, Some("Wren"), "Wren", 24, 1000).is_err() && add(&d, Some("x1"), "Wren", 24, 1000).is_err());
        let _ = fs::remove_dir_all(&d);
    }
}
