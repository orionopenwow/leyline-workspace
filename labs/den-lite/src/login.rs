//! The login server (port 3724): SRP6 with `wow_srp`, the messages from `wow_login_messages` (protocol 8, the one
//! 3.3.5a 12340 speaks), and a realm list of one realm, "Your Den".
//!
//! Any account name logs in. The password is a fixed local one (`--password`, default "den"): SRP6 needs the
//! password's verifier *before* the client proves anything, so a server can't learn a new password at the first
//! login. The launcher fills it in for the player (it writes the realmlist and can pass the account), so nobody types
//! it. Den Lite only listens on 127.0.0.1.

use crate::Shared;
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
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

pub fn serve(listener: TcpListener, shared: Arc<Shared>) {
    for stream in listener.incoming().flatten() {
        let shared = shared.clone();
        std::thread::spawn(move || {
            let _ = stream.set_nodelay(true);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
            if let Err(e) = session(stream, &shared) {
                shared.log(&format!("login: {e}"));
            }
        });
    }
}

fn send<M: Message>(s: &mut TcpStream, m: &M) -> io::Result<()> {
    let mut v = Vec::new();
    m.write(&mut v)?;
    s.write_all(&v)
}

fn session(mut s: TcpStream, shared: &Shared) -> io::Result<()> {
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
    let pass = NormalizedString::new(&shared.cfg.password).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad local password"))?;
    let proof = SrpVerifier::from_username_and_password(user.clone(), pass).into_proof();
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
            send(&mut s, &CMD_AUTH_LOGON_PROOF_Server::FailIncorrectPassword)?;
            return Ok(());
        }
    };
    shared.sessions.lock().unwrap().insert(user.to_string(), *server.session_key());
    shared.log(&format!("login: {} signed in", user));
    send(
        &mut s,
        &CMD_AUTH_LOGON_PROOF_Server::Success { account_flag: AccountFlag::empty(), hardware_survey_id: 0, server_proof, unknown: 0 },
    )?;
    loop {
        match ClientOpcodeMessage::read(&mut s) {
            Ok(ClientOpcodeMessage::CMD_REALM_LIST(_)) => send(&mut s, &realm_list(shared))?,
            Ok(_) => {}
            Err(_) => return Ok(()),
        }
    }
}

pub fn realm_list(shared: &Shared) -> CMD_REALM_LIST_Server {
    CMD_REALM_LIST_Server {
        realms: vec![Realm {
            realm_type: RealmType::PlayerVsEnvironment,
            locked: false,
            flag: Realm_RealmFlag::empty(),
            name: shared.cfg.realm_name.clone(),
            address: shared.world_address(),
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
