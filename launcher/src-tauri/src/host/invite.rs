//! One-click invite links to a world on this PC (docs/UNSTOPPABLE.md, "Worlds in three clicks"; the friend's side:
//! one Join).
//!
//! **The link.** `leyline://join/<invite>`, or on the static site `https://leylinelauncher.com/j/#<invite>` (a page
//! that opens the launcher with the same `leyline://` link and offers "Get Leyline" when it doesn't open; the invite
//! sits after `#`, so it never reaches a server's log).
//!
//! **The invite** is `lw1.<base64url of a small JSON document>.<base64url of an Ed25519 signature>`, signed by the
//! host's peer key (identity.rs, the key Friends signs with) over [`CTX`] + "\n" + canonical(document):
//!
//! ```text
//!   {"v":1, "pk":<host public key, hex>, "w":<the world's realm id>, "k":<single|vanilla|bossrush>,
//!    "n":<host's name or "">, "s":<the world's invite secret>, "c":<one-time code: USER.PASS>,
//!    "l":<same-network hint ip:port, private addresses only, or absent>, "x":<expires, unix seconds>,
//!    "d":<the host's Passport device key, hex: its connection key (p2p/linknet.rs), or absent>}
//! ```
//!
//! `s` and `c` are what the lasting-login handshake already takes (host/worldfriends.rs): the claim's AEAD key comes
//! from `s`, and `c` is the one-time ticket a new friend claims their lasting login with (one identity, 7 days). So an
//! opened invite becomes the link `join_invite` already understands ([`Invite::legacy_link`]) and joins the same way.
//!
//! **How the friend reaches the host:** the same-network hint first, then the transport by the host's key from
//! anywhere ([`dial_by_key`]: p2p/link.rs `guest_join`, a forwarder on 127.0.0.1 here; the device key comes from the
//! invite's `d`, friends' presence or the host's DHT pointer, and is checked against `pk` at connect time), then
//! whatever the old link's ways still do. [`Invite::legacy_link`] carries `~~key=` (and `~~dev=`) so `join_invite`
//! takes the key way after the same network. Never a public address in the invite.

use serde::Serialize;
use serde_json::{json, Map, Value};

/// The signature's context: an invite can't be taken for any other signed record.
pub const CTX: &str = "leyline-world-invite-v1";
pub const PREFIX: &str = "lw1.";
/// The longest invite a link may carry (a real one is about 450 characters).
pub const MAX: usize = 900;
/// The site's page for the https form (tools/site-static/build.py `join_page`).
pub const SITE_PAGE: &str = "https://leylinelauncher.com/j/#";
pub const SCHEME_PREFIX: &str = "leyline://join/";
/// Good for as long as its one-time code (host/worldfriends.rs TICKET_DAYS).
pub const DAYS: u64 = crate::host::worldfriends::TICKET_DAYS;
const KINDS: [&str; 3] = ["single", "vanilla", "bossrush"];

/// What an invite says, once its signature and shape are checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Invite {
    /// The host's public key (hex) and its peer id (what the Friends list shows).
    pub pk: String,
    pub peer: String,
    pub world: String,
    pub kind: String,
    pub name: String,
    #[serde(skip)]
    pub secret: String,
    #[serde(skip)]
    pub code: String,
    #[serde(skip)]
    pub lan: Option<String>,
    /// The host's device key (hex), when the invite carries it.
    #[serde(skip)]
    pub device: Option<String>,
    pub expires: u64,
}

/// What goes into a new invite (the host's side).
pub struct Make<'a> {
    pub world: &'a str,
    pub kind: &'a str,
    pub name: &'a str,
    pub secret: &'a str,
    pub user: &'a str,
    pub pass: &'a str,
    pub lan: Option<&'a str>,
    /// The host's Passport device key (hex), when its connections run.
    pub device: Option<&'a str>,
    pub now: u64,
}

/// A host's name as an invite may carry it: letters, digits, spaces and `'-_.`, at most 24, trimmed; else "".
pub fn clean_name(n: &str) -> String {
    let t: String = n.trim().chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '\'' | '-' | '_' | '.')).take(24).collect();
    t.trim().to_string()
}

fn secret_ok(s: &str) -> bool {
    (16..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_uppercase() || ('2'..='9').contains(&c))
}

fn code_ok(c: &str) -> bool {
    c.split_once('.').is_some_and(|(u, p)| {
        let ok = |s: &str, n: std::ops::RangeInclusive<usize>| n.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        u.starts_with('G') && ok(u, 4..=16) && ok(p, 8..=16)
    })
}

fn lan_ok(l: &str) -> bool {
    l.parse::<std::net::SocketAddr>().is_ok_and(|a| crate::p2p::friendnet::local_ip(&a.ip()) && !a.ip().is_loopback())
}

/// The document a host signs (checked here first, so a bad one is never signed).
pub fn doc(m: &Make) -> Result<Value, String> {
    if !crate::realms::realm_id_ok(m.world) || !KINDS.contains(&m.kind) || !secret_ok(m.secret) {
        return Err("That world can't make an invite.".into());
    }
    let code = format!("{}.{}", m.user, m.pass);
    if !code_ok(&code) {
        return Err("The friend's login isn't one an invite can carry.".into());
    }
    let mut d = Map::new();
    d.insert("v".into(), json!(1));
    d.insert("w".into(), json!(m.world));
    d.insert("k".into(), json!(m.kind));
    d.insert("n".into(), json!(clean_name(m.name)));
    d.insert("s".into(), json!(m.secret));
    d.insert("c".into(), json!(code));
    if let Some(l) = m.lan.filter(|l| lan_ok(l)) {
        d.insert("l".into(), json!(l));
    }
    if let Some(k) = m.device.and_then(crate::identity::unhex32) {
        d.insert("d".into(), json!(crate::identity::hex(&k)));
    }
    d.insert("x".into(), json!(m.now + DAYS * 86_400));
    Ok(Value::Object(d))
}

/// A signed invite: `lw1.<doc>.<sig>`. The document gets the signer's key as `pk`.
pub fn make(me: &crate::identity::PeerIdentity, m: &Make) -> Result<String, String> {
    let mut d = doc(m)?;
    d["pk"] = json!(me.public_hex());
    let sig = crate::identity::unhex(&me.sign_doc(CTX, &d)).ok_or("bad signature")?;
    let body = crate::host::bossrush::b64url(crate::identity::canonical(&d).as_bytes());
    Ok(format!("{PREFIX}{body}.{}", crate::host::bossrush::b64url(&sig)))
}

/// The two links for one invite: the launcher's own and the site's (the invite after `#`).
pub fn links(invite: &str) -> (String, String) {
    (format!("{SCHEME_PREFIX}{invite}"), format!("{SITE_PAGE}{invite}"))
}

/// The invite in whatever was pasted or opened: the bare invite, a `leyline://join/` link or the site's link.
pub fn find(text: &str) -> Option<&str> {
    let t = text.trim();
    let t = t.strip_prefix(SCHEME_PREFIX).or_else(|| t.split_once("/j/#").map(|(_, r)| r)).unwrap_or(t);
    let t = t.trim_end_matches('/');
    (t.starts_with(PREFIX) && t.len() <= MAX && t.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))).then_some(t)
}

/// Shape only (deeplink.rs takes a link with this before anything is opened): `lw1.<b64url>.<b64url>`.
pub fn shape_ok(invite: &str) -> bool {
    find(invite) == Some(invite) && invite.split('.').count() == 3
}

/// Check an invite: its shape, the host's signature over it, every field, and that it hasn't run out.
pub fn open(invite: &str, now: u64) -> Result<Invite, String> {
    const BAD: &str = "That invite link isn't complete. Ask your friend to copy it again.";
    let inv = find(invite).ok_or(BAD)?;
    let mut parts = inv.split('.');
    let (_, body, sig) = (parts.next(), parts.next().ok_or(BAD)?, parts.next().ok_or(BAD)?);
    if parts.next().is_some() {
        return Err(BAD.into());
    }
    let raw = crate::host::bossrush::un_b64url(body).ok_or(BAD)?;
    let sig = crate::host::bossrush::un_b64url(sig).ok_or(BAD)?;
    let d: Value = serde_json::from_slice(&raw).map_err(|_| BAD.to_string())?;
    let o = d.as_object().ok_or(BAD)?;
    let allowed = ["v", "pk", "w", "k", "n", "s", "c", "l", "x", "d"];
    if o.keys().any(|k| !allowed.contains(&k.as_str())) || o.get("v") != Some(&json!(1)) {
        return Err("That invite is from a newer Leyline. Update Leyline, then open it again.".into());
    }
    let s = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("");
    let pk = s("pk").to_ascii_lowercase();
    let pk_bytes = crate::identity::unhex32(&pk).ok_or(BAD)?;
    if sig.len() != 64 || !crate::identity::verify_doc(&pk, &d, CTX, &crate::identity::hex(&sig)) {
        return Err("That invite's signature doesn't match: it was changed after your friend made it. Ask for a new one.".into());
    }
    let (world, kind, secret, code) = (s("w"), s("k"), s("s"), s("c"));
    let lan = o.get("l").and_then(Value::as_str).map(str::to_string);
    if !crate::realms::realm_id_ok(world) || !KINDS.contains(&kind) || !secret_ok(secret) || !code_ok(code) || lan.as_deref().is_some_and(|l| !lan_ok(l)) {
        return Err(BAD.into());
    }
    let name = s("n");
    if clean_name(name) != name {
        return Err(BAD.into());
    }
    let device = match o.get("d") {
        None => None,
        Some(v) => Some(v.as_str().and_then(crate::identity::unhex32).filter(|k| *k != pk_bytes).map(|k| crate::identity::hex(&k)).ok_or(BAD)?),
    };
    let expires = o.get("x").and_then(Value::as_u64).ok_or(BAD)?;
    if expires < now {
        return Err("That invite has run out (they last a week). Ask your friend for a new one.".into());
    }
    Ok(Invite {
        peer: crate::identity::peer_id(&pk_bytes),
        pk,
        world: world.into(),
        kind: kind.into(),
        name: name.into(),
        secret: secret.into(),
        code: code.into(),
        lan,
        device,
        expires,
    })
}

impl Invite {
    /// The link `join_invite` joins with: the world's realm id and secret, the one-time code, the same-network hint
    /// and the mark that the world answers claims (litenet::invite_link, denways::with_ways, worldfriends::with_mark).
    pub fn legacy_link(&self) -> String {
        let (u, p) = self.code.split_once('.').unwrap_or_default();
        let base = crate::host::litenet::invite_link(crate::realms::RELAY, &self.world, &self.secret, Some((u, p)));
        let l = crate::host::worldfriends::with_mark(&crate::host::denways::with_ways(&base, self.lan.as_deref(), None));
        // the host's key (and device key) for the way from anywhere (p2p/link.rs `key_in`, `device_in`)
        match &self.device {
            Some(d) => crate::p2p::link::with_key(&l, &self.pk, d),
            None if crate::p2p::link::key_in(&l).is_none() => format!("{l}~~key={}", self.pk),
            None => l,
        }
    }

    /// "Join Bo's world?" (the name the Friends list knows them by wins over the one in the link).
    pub fn title(&self, known: Option<&str>) -> String {
        let n = known.map(clean_name).filter(|n| !n.is_empty()).unwrap_or_else(|| self.name.clone());
        if n.is_empty() {
            "Join your friend's world?".into()
        } else {
            format!("Join {n}'s world?")
        }
    }
}

/// The transport's dial-by-key (p2p/link.rs `guest_join`): this PC's forwarder address on 127.0.0.1 for the host's
/// world, reached by the host's key from anywhere (same network, direct, hole punching, a mutual friend or a
/// community relay). None when this build has no connections. Dials: only at Join, never when a card is shown.
pub fn dial_by_key(inv: &Invite) -> Option<Result<String, String>> {
    crate::p2p::link::guest_join(&inv.legacy_link(), &inv.world, &inv.secret).map(|r| r.map(|(addr, _)| addr))
}

/// The ways a Join tries, in order, for the card: the same-network hint ("ip:port"), then "key" (by the host's key
/// from anywhere, [`dial_by_key`], which `join_invite` runs after the same network). Nothing is dialed here.
pub fn ways(inv: &Invite) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    if let Some(l) = &inv.lan {
        v.push(l.clone());
    }
    v.push("key".into());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me(n: u8) -> crate::identity::PeerIdentity {
        crate::identity::PeerIdentity::from_seed([n; 32]).unwrap()
    }
    fn m() -> Make<'static> {
        Make {
            world: "abcdefghijk2345a",
            kind: "single",
            name: "Bo",
            secret: "ABCDEFGHJKLMNPQRSTUVWXYZ",
            user: "GABC12345",
            pass: "PASS1234WORD",
            lan: Some("192.168.1.20:3724"),
            device: None,
            now: 1_000,
        }
    }

    #[test]
    fn an_invite_opens_to_what_the_host_signed() {
        let inv = make(&me(7), &m()).unwrap();
        assert!(inv.starts_with("lw1.") && inv.len() < 600, "{} {inv}", inv.len());
        assert!(shape_ok(&inv));
        let o = open(&inv, 2_000).unwrap();
        assert_eq!(o.pk, me(7).public_hex());
        assert_eq!(o.peer, me(7).peer_id);
        assert_eq!((o.world.as_str(), o.kind.as_str(), o.name.as_str()), ("abcdefghijk2345a", "single", "Bo"));
        assert_eq!(o.lan.as_deref(), Some("192.168.1.20:3724"));
        assert_eq!(o.expires, 1_000 + 7 * 86_400);
        assert_eq!(o.title(None), "Join Bo's world?");
        assert_eq!(o.title(Some("Bodil")), "Join Bodil's world?");
        // what the secret-bearing fields never do: show up in what the page gets
        let shown = serde_json::to_string(&o).unwrap();
        assert!(!shown.contains("ABCDEFGHJKLMNPQRSTUVWXYZ") && !shown.contains("PASS1234WORD"), "{shown}");
    }

    #[test]
    fn the_opened_invite_is_the_link_join_invite_takes() {
        let o = open(&make(&me(7), &m()).unwrap(), 2_000).unwrap();
        let l = o.legacy_link();
        let (relay, rid, secret) = crate::realms::parse_invite(&l).expect("a join link");
        assert_eq!((relay.as_str(), rid.as_str(), secret.as_str()), (crate::realms::RELAY, "abcdefghijk2345a", "ABCDEFGHJKLMNPQRSTUVWXYZ"));
        assert_eq!(crate::host::litenet::guest_in(&l), Some(("GABC12345".into(), "PASS1234WORD".into())));
        assert!(crate::host::worldfriends::marked(&l));
        assert_eq!(crate::host::denways::ways_in(&l).0.as_deref(), Some("192.168.1.20:3724"));
        assert_eq!(ways(&o), vec!["192.168.1.20:3724".to_string(), "key".to_string()], "same network first, then by key");
        // the key way: the host's key in the link join_invite takes (p2p/link.rs guest_join reads it)
        assert_eq!(crate::p2p::link::key_in(&l), Some(me(7).public_key));
        assert_eq!(crate::p2p::link::device_in(&l), None, "no device key in this invite: the DHT or presence gives it");
    }

    #[test]
    fn an_invite_can_carry_the_hosts_device_key() {
        let dev = crate::identity::hex(&[9u8; 32]);
        let o = open(&make(&me(7), &Make { device: Some(&dev), ..m() }).unwrap(), 2_000).unwrap();
        assert_eq!(o.device.as_deref(), Some(dev.as_str()));
        let l = o.legacy_link();
        assert_eq!(crate::p2p::link::key_in(&l), Some(me(7).public_key));
        assert_eq!(crate::p2p::link::device_in(&l), Some([9u8; 32]));
        assert!(crate::realms::parse_invite(&l).is_some(), "still the link join_invite takes");
        assert!(!serde_json::to_string(&o).unwrap().contains(&dev), "not shown to the page");
        // the root itself is never a device key; a bad one is never signed
        let root = me(7).public_hex();
        assert!(open(&make(&me(7), &Make { device: Some(&root), ..m() }).unwrap(), 2_000).is_err());
        let o = open(&make(&me(7), &Make { device: Some("nope"), ..m() }).unwrap(), 2_000).unwrap();
        assert_eq!(o.device, None);
    }

    #[test]
    fn a_changed_or_old_invite_is_refused() {
        let inv = make(&me(7), &m()).unwrap();
        assert!(open(&inv, 1_000 + 8 * 86_400).unwrap_err().contains("run out"));
        // another key's signature on the same document
        let other = make(&me(8), &m()).unwrap();
        let (_, sig8) = other.rsplit_once('.').unwrap();
        let (head, _) = inv.rsplit_once('.').unwrap();
        assert!(open(&format!("{head}.{sig8}"), 2_000).unwrap_err().contains("signature"));
        // a field changed after signing
        let (pre, rest) = inv.split_at(4);
        let (body, sig) = rest.split_once('.').unwrap();
        let mut d: Value = serde_json::from_slice(&crate::host::bossrush::un_b64url(body).unwrap()).unwrap();
        d["n"] = json!("Mallory");
        let forged = format!("{pre}{}.{sig}", crate::host::bossrush::b64url(crate::identity::canonical(&d).as_bytes()));
        assert!(open(&forged, 2_000).unwrap_err().contains("signature"));
        for bad in ["", "lw1.", "lw1.abc", "lw2.a.b", "lw1.a.b.c", "lw1.!!.xx"] {
            assert!(open(bad, 2_000).is_err(), "{bad}");
        }
    }

    #[test]
    fn found_in_every_form_it_travels_in() {
        let inv = make(&me(7), &m()).unwrap();
        let (app, site) = links(&inv);
        assert_eq!(app, format!("leyline://join/{inv}"));
        assert_eq!(site, format!("https://leylinelauncher.com/j/#{inv}"));
        for t in [inv.clone(), app, site, format!("  {inv}\n")] {
            assert_eq!(find(&t), Some(inv.as_str()), "{t}");
        }
        assert_eq!(find("https://example.com/"), None);
        assert_eq!(find(&format!("{inv}<script>")), None);
    }

    #[test]
    fn a_bad_world_or_address_is_never_signed() {
        let k = me(7);
        assert!(make(&k, &Make { world: "NOPE", ..m() }).is_err());
        assert!(make(&k, &Make { kind: "den", ..m() }).is_err());
        assert!(make(&k, &Make { user: "SOLO", ..m() }).is_err());
        // a public address is never put in: the invite carries no address but a same-network one
        let o = open(&make(&k, &Make { lan: Some("8.8.8.8:3724"), ..m() }).unwrap(), 2_000).unwrap();
        assert_eq!(o.lan, None);
        let o = open(&make(&k, &Make { lan: None, name: "  <b>Bo</b>  ", ..m() }).unwrap(), 2_000).unwrap();
        assert_eq!(o.name, "bBob");
        assert_eq!(ways(&o), vec!["key".to_string()]);
    }
}
