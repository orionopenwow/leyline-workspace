//! The login server (port 3724): SRP6 with `wow_srp`, the messages from `wow_login_messages` (protocol 8, the one
//! 3.3.5a 12340 speaks), and a realm list of one realm, "Your Den".
//!
//! Any account name logs in. The password is a fixed local one (`--password`, default "den"): SRP6 needs the
//! password's verifier *before* the client proves anything, so a server can't learn a new password at the first
//! login. The launcher fills it in for the player (it writes the realmlist and can pass the account), so nobody types
//! it. Den Lite only listens on 127.0.0.1.
//!
//! Friends (guests.rs): a guest account logs in with its own SRP6 verifier from `<dens>/guests.json`. The remote
//! listener (`remote`: what the relay tunnel reaches) takes guests only, and so does any peer that isn't loopback:
//! the host's fixed local login never works from outside this PC. Its realm list names the address friends reach the
//! world server at (`<dens>/den-lite.public`, written by the launcher's tunnel), never 127.0.0.1.

use crate::{Lock, Shared};
use std::io::{self, Write};
use crate::wire::Stream;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;
use wow_login_messages::all::{Population, ProtocolVersion, Version};
use wow_login_messages::helper::{read_initial_message, InitialMessage};
use wow_login_messages::version_8::opcodes::ClientOpcodeMessage;
use wow_login_messages::version_8::{
    AccountFlag, CMD_AUTH_LOGON_CHALLENGE_Server, CMD_AUTH_LOGON_CHALLENGE_Server_SecurityFlag, CMD_AUTH_LOGON_PROOF_Server,
    CMD_AUTH_RECONNECT_CHALLENGE_Server, CMD_REALM_LIST_Server, Realm, RealmCategory, RealmType, Realm_RealmFlag,
};
use wow_login_messages::Message;
use wow_srp::normalized_string::NormalizedString;
use wow_srp::server::SrpVerifier;
use wow_srp::{PublicKey, GENERATOR, LARGE_SAFE_PRIME_LITTLE_ENDIAN};

pub const CLIENT_BUILD: u16 = 12340;

/// Who logged in: the host (the fixed local login, this PC only) or a guest (its own login).
#[derive(Debug, Clone, PartialEq)]
pub enum Who {
    Host,
    Guest(crate::guests::Guest),
}

pub fn serve(listener: TcpListener, shared: Arc<Shared>, remote: bool) {
    for stream in listener.incoming().flatten() {
        let shared = shared.clone();
        std::thread::spawn(move || {
            let _ = stream.set_nodelay(true);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
            let remote = remote || !stream.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false);
            if let Err(e) = session(Stream::tcp(stream), &shared, remote, None) {
                shared.log(&format!("login: {e}"));
            }
        });
    }
}

/// `<dens>/den-lite.direct-host`: the player's own address on the internet ("Your own address" in the launcher: a
/// name or an IP with the router's ports 3724 and 8085 sent to this PC, or their own server). Friends who come in
/// from the internet on the direct listener are sent there; friends on the same network to this PC's own address.
pub const DIRECT_HOST_FILE: &str = "den-lite.direct-host";

/// A host name or IP address as typed: letters, digits, dots, - and : (IPv6), up to 253.
pub fn host_ok(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253 && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':')
}

pub fn direct_host(dir: &std::path::Path) -> Option<String> {
    let t = std::fs::read_to_string(dir.join(DIRECT_HOST_FILE)).ok()?;
    let t = t.trim();
    host_ok(t).then(|| t.to_string())
}

/// A friend on this network (a private, link-local or loopback address).
pub fn same_network(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local(),
        std::net::IpAddr::V6(v) => v.is_loopback() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80,
    }
}

/// The world address the direct listener's realm list names: this PC's address on the network the friend came in
/// on (same network), else the player's own address (`public`), else this PC's address anyway.
pub fn direct_address(local: std::net::IpAddr, peer: std::net::IpAddr, public: Option<&str>, world_port: u16) -> String {
    match public.filter(|_| !same_network(peer)) {
        Some(h) if h.contains(':') && !h.starts_with('[') => format!("[{h}]:{world_port}"),
        Some(h) => format!("{h}:{world_port}"),
        None => std::net::SocketAddr::new(local, world_port).to_string(),
    }
}

/// The friends' direct listener (`--friends-lan`, a LAN address of this PC): guests only, the realm list pointing
/// back at this PC the way the friend came (direct_address).
pub fn serve_direct(listener: TcpListener, shared: Arc<Shared>, world_port: u16) {
    for stream in listener.incoming().flatten() {
        let shared = shared.clone();
        std::thread::spawn(move || {
            let _ = stream.set_nodelay(true);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
            let (Ok(local), Ok(peer)) = (stream.local_addr(), stream.peer_addr()) else { return };
            let public = direct_host(&shared.cfg.den_dir);
            let address = direct_address(local.ip(), peer.ip(), public.as_deref(), world_port);
            shared.event("direct", "", if same_network(peer.ip()) { "same network" } else { "internet" });
            if let Err(e) = session(Stream::tcp(stream), &shared, true, Some(address)) {
                shared.log(&format!("login: {e}"));
            }
        });
    }
}

fn send<M: Message>(s: &mut Stream, m: &M) -> io::Result<()> {
    let mut v = Vec::new();
    m.write(&mut v)?;
    s.write_all(&v)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// One login connection, from its first message to its end (also the page's own Den Lite: web.rs).
pub fn session(mut s: Stream, shared: &Shared, remote: bool, address: Option<String>) -> io::Result<()> {
    let first = read_initial_message(&mut s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let challenge = match first {
        InitialMessage::Logon(c) => c,
        InitialMessage::Reconnect(_) => {
            // A reconnect needs the earlier session's SRP state; the client falls back to a full login.
            send(&mut s, &CMD_AUTH_RECONNECT_CHALLENGE_Server::FailUnknownAccount)?;
            return Ok(());
        }
    };
    let wanted = Version { major: 3, minor: 3, patch: 5, build: CLIENT_BUILD };
    if challenge.protocol_version != ProtocolVersion::Eight || challenge.version != wanted {
        send(&mut s, &CMD_AUTH_LOGON_CHALLENGE_Server::FailVersionInvalid)?;
        return Ok(());
    }
    let Ok(user) = NormalizedString::new(&challenge.account_name) else {
        send(&mut s, &CMD_AUTH_LOGON_CHALLENGE_Server::FailUnknownAccount)?;
        return Ok(());
    };
    shared.event("login_challenge", &user.to_string(), if remote { "remote" } else { "local" });
    // a guest's own login, else the host's fixed local one (this PC only)
    let guest = crate::guests::find(&shared.cfg.den_dir, &user.to_string(), now());
    let verifier = match &guest {
        Some(g) => crate::guests::verifier(g),
        None if remote => None,
        None => {
            let pass = NormalizedString::new(&shared.cfg.password).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad local password"))?;
            Some(SrpVerifier::from_username_and_password(user.clone(), pass))
        }
    };
    let Some(verifier) = verifier else {
        shared.log(&format!("login: {user} refused (not a guest, from outside this PC)"));
        send(&mut s, &CMD_AUTH_LOGON_CHALLENGE_Server::FailUnknownAccount)?;
        return Ok(());
    };
    let who = guest.map(Who::Guest).unwrap_or(Who::Host);
    let proof = verifier.into_proof();
    send(
        &mut s,
        &CMD_AUTH_LOGON_CHALLENGE_Server::Success {
            crc_salt: rand_bytes(),
            generator: vec![GENERATOR],
            large_safe_prime: LARGE_SAFE_PRIME_LITTLE_ENDIAN.to_vec(),
            salt: *proof.salt(),
            security_flag: CMD_AUTH_LOGON_CHALLENGE_Server_SecurityFlag::empty(),
            server_public_key: *proof.server_public_key(),
        },
    )?;
    let p = match ClientOpcodeMessage::read(&mut s) {
        Ok(ClientOpcodeMessage::CMD_AUTH_LOGON_PROOF(p)) => p,
        Ok(_) | Err(_) => return Ok(()),
    };
    let Ok(client_key) = PublicKey::from_le_bytes(p.client_public_key) else {
        send(&mut s, &CMD_AUTH_LOGON_PROOF_Server::FailIncorrectPassword)?;
        return Ok(());
    };
    let (server, server_proof) = match proof.into_server(client_key, p.client_proof) {
        Ok(v) => v,
        Err(_) => {
            shared.log(&format!("login: wrong password for {}", user));
            shared.event("auth_fail", &user.to_string(), "wrong password");
            send(&mut s, &CMD_AUTH_LOGON_PROOF_Server::FailIncorrectPassword)?;
            return Ok(());
        }
    };
    shared.sessions.lk().insert(user.to_string(), (*server.session_key(), who, remote));
    shared.log(&format!("login: {} signed in", user));
    shared.event("auth_ok", &user.to_string(), if remote { "remote" } else { "local" });
    send(
        &mut s,
        &CMD_AUTH_LOGON_PROOF_Server::Success { account_flag: AccountFlag::empty(), hardware_survey_id: 0, server_proof, unknown: 0 },
    )?;
    loop {
        match ClientOpcodeMessage::read(&mut s) {
            Ok(ClientOpcodeMessage::CMD_REALM_LIST(_)) => {
                let list = match &address {
                    Some(a) => realm_list_at(shared, a.clone()),
                    None => realm_list(shared, remote),
                };
                send(&mut s, &list)?;
                shared.event("realm_list", &user.to_string(), "");
            }
            Ok(_) => {}
            Err(_) => return Ok(()),
        }
    }
}

/// `<dens>/den-lite.public`: "host:port", where friends reach the world server (the relay's address and this
/// Den's world port there), written by the launcher's tunnel.
pub const PUBLIC_FILE: &str = "den-lite.public";

/// The world address a friend's client is sent to: the tunnel's, when it is a plain "host:port".
pub fn public_address(dir: &std::path::Path) -> Option<String> {
    let t = std::fs::read_to_string(dir.join(PUBLIC_FILE)).ok()?;
    let t = t.trim();
    let (h, p) = t.rsplit_once(':')?;
    let ok = !h.is_empty() && h.len() <= 253 && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') && p.parse::<u16>().is_ok_and(|p| p > 0);
    ok.then(|| t.to_string())
}

pub fn realm_list(shared: &Shared, remote: bool) -> CMD_REALM_LIST_Server {
    let address = if remote { public_address(&shared.cfg.den_dir).unwrap_or_else(|| shared.remote_world_address()) } else { shared.world_address() };
    realm_list_at(shared, address)
}

/// The one realm, at this world address.
pub fn realm_list_at(shared: &Shared, address: String) -> CMD_REALM_LIST_Server {
    CMD_REALM_LIST_Server {
        realms: vec![Realm {
            realm_type: RealmType::PlayerVsEnvironment,
            locked: false,
            flag: Realm_RealmFlag::empty(),
            name: shared.cfg.realm_name.clone(),
            address,
            population: Population::GreenRecommended,
            number_of_characters_on_realm: 1,
            category: RealmCategory::One,
            realm_id: 1,
        }],
    }
}

pub fn rand_bytes<const N: usize>() -> [u8; N] {
    // Not a secret (a CRC salt / a seed's filler): the time and a counter are enough.
    use std::sync::atomic::{AtomicU64, Ordering};
    static C: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);
    let mut x = C.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
        ^ std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let mut out = [0u8; N];
    for b in out.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
    out
}

#[cfg(test)]
mod direct_tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn a_friend_is_sent_back_the_way_they_came() {
        let lan: IpAddr = "192.168.1.20".parse().unwrap();
        let friend_lan: IpAddr = "192.168.1.31".parse().unwrap();
        let friend_net: IpAddr = "81.2.69.160".parse().unwrap();
        assert_eq!(direct_address(lan, friend_lan, Some("my.example.net"), 8085), "192.168.1.20:8085", "same network: this PC");
        assert_eq!(direct_address(lan, friend_net, Some("my.example.net"), 8085), "my.example.net:8085", "the internet: your own address");
        assert_eq!(direct_address(lan, friend_net, None, 8085), "192.168.1.20:8085", "no own address set: this PC's");
        assert_eq!(direct_address(lan, friend_net, Some("2001:db8::1"), 8085), "[2001:db8::1]:8085");
        assert!(same_network("10.0.0.2".parse().unwrap()) && same_network("fe80::1".parse().unwrap()) && !same_network(friend_net));
        assert!(host_ok("den.example.org") && !host_ok("a b") && !host_ok("x/y") && !host_ok(""));
    }
}
