//! Your Den on the Leyline network, with no Docker: the launcher keeps the same tunnel to the relay that the Host
//! kit's agent keeps (hosting/kit/tools/wuhost/agent.py, wunet/mux.py), for Den Lite's friends' listeners.
//!
//!   friend's game --TCP--> relay (auth_port / world_port) --one outbound wss, multiplexed--> this launcher
//!     --> 127.0.0.1:<Den Lite's friends' login / world port> (labs/den-lite `den-lite.remote`)
//!
//! What it does, and nothing more (the relay protocol is unchanged, so the relay can't be asked for more than it
//! gives the Host kit):
//! * one outbound WebSocket to `wss://<relay>/agent`; the relay's challenge is answered with a hello signed by this
//!   Den's own Ed25519 key (`<den-lite>/state/realm.key`, made on this PC, never sent; the realm id is
//!   base32(sha256(public key))[:16], as wunet/keys.py);
//! * the hello says **private** (only players who opened the invite link get in: the relay keeps sha256 of the
//!   invite secret, never the secret) and **signup off** (the relay never makes accounts here: friends' logins are
//!   Den Lite's own guest logins, labs/den-lite/src/guests.rs);
//! * each player connection the relay opens (OPEN 3724 or 8085) becomes one local connection to Den Lite's
//!   friends' login or world listener, and the bytes go both ways untouched (DATA), until either side closes
//!   (CLOSE). Those listeners take guest logins only: the host's own local login never works through here;
//! * the relay's welcome names the Den's address and ports there; `<dens>/den-lite.public` gets "<relay IPv4>:
//!   <world port>" so friends' realm list sends their game back through the relay, never to this PC's address;
//! * a ping every 20 seconds; no answer for a minute: it reconnects (with a growing pause).
//!
//! The invite link is the relay's own (`https://<relay>/join/<realm id>#<invite secret>`), with the friend's guest
//! login after `~~` (`#<secret>~~<account>.<password>`): the part after `#` never leaves the browser or the
//! launcher, and the relay's join page reads only what comes before the first `~`.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const CTX_RELAY_LOGIN: &str = "wu-relay-login-v1";
pub const VERSION: &str = "leyline-den-lite/1";
const MAX_CHUNK: usize = 32 * 1024;
const PING_EVERY: Duration = Duration::from_secs(20);
const SILENT_MAX: Duration = Duration::from_secs(65);
const OPEN: u8 = 1;
const DATA: u8 = 2;
const CLOSE: u8 = 3;

// ------------------------------------------------------------------------------------------------ keys

/// `<den-lite root>/state/realm.key`: 32 random bytes as hex, made once.
pub fn key_path(root: &Path) -> PathBuf {
    root.join("state").join("realm.key")
}

fn random(n: usize) -> Result<Vec<u8>, String> {
    use ring::rand::SecureRandom;
    let mut b = vec![0u8; n];
    ring::rand::SystemRandom::new().fill(&mut b).map_err(|_| "no random source on this PC".to_string())?;
    Ok(b)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn load_or_create_key(path: &Path) -> Result<[u8; 32], String> {
    if let Ok(t) = std::fs::read_to_string(path) {
        let t = t.trim();
        let mut k = [0u8; 32];
        if t.len() != 64 {
            return Err("Your Den's network key file doesn't look right.".into());
        }
        for (i, b) in k.iter_mut().enumerate() {
            *b = u8::from_str_radix(&t[i * 2..i * 2 + 2], 16).map_err(|_| "Your Den's network key file doesn't look right.")?;
        }
        return Ok(k);
    }
    let k: [u8; 32] = random(32)?.try_into().map_err(|_| "no key")?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    crate::settings::write_atomic(path, (hex(&k) + "\n").as_bytes()).map_err(|e| e.to_string())?;
    Ok(k)
}

fn pair(seed: &[u8; 32]) -> Result<ring::signature::Ed25519KeyPair, String> {
    ring::signature::Ed25519KeyPair::from_seed_unchecked(seed).map_err(|_| "bad key".to_string())
}

pub fn public_key(seed: &[u8; 32]) -> Result<[u8; 32], String> {
    use ring::signature::KeyPair;
    pair(seed)?.public_key().as_ref().try_into().map_err(|_| "bad key".to_string())
}

/// base32(sha256(public key))[:16], lower case (wunet/keys.py realm_id).
pub fn realm_id(public: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    const ABC: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let d = Sha256::digest(public);
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in d.iter() {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 && out.len() < 16 {
            bits -= 5;
            out.push(ABC[((buf >> bits) & 31) as usize] as char);
        }
    }
    out
}

/// The bytes a signature covers: keys sorted, no spaces, UTF-8 (wunet/keys.py canonical; Python's json escaping).
pub fn canonical(v: &Value) -> String {
    let mut o = String::new();
    fn put(v: &Value, o: &mut String) {
        match v {
            Value::Object(m) => {
                let mut ks: Vec<&String> = m.keys().collect();
                ks.sort();
                o.push('{');
                for (i, k) in ks.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    o.push_str(&serde_json::to_string(k).unwrap_or_default());
                    o.push(':');
                    put(&m[*k], o);
                }
                o.push('}');
            }
            Value::Array(a) => {
                o.push('[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    put(x, o);
                }
                o.push(']');
            }
            other => o.push_str(&serde_json::to_string(other).unwrap_or_default()),
        }
    }
    put(v, &mut o);
    o
}

pub fn sign_doc(seed: &[u8; 32], doc: &Value, ctx: &str) -> Result<String, String> {
    let msg = format!("{ctx}\n{}", canonical(doc));
    Ok(hex(pair(seed)?.sign(msg.as_bytes()).as_ref()))
}

fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(s.as_bytes()))
}

// ------------------------------------------------------------------------------------------------ invites

/// The invite secret (the relay's knock): 24 letters and digits, as the Host kit's (`realms::parse_invite` takes it).
pub fn new_secret() -> Result<String, String> {
    const ABC: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    Ok(random(24)?.iter().map(|b| ABC[*b as usize % ABC.len()] as char).collect())
}

/// The link a friend opens: the relay's join link with the guest login after `~~`.
pub fn invite_link(relay: &str, rid: &str, secret: &str, guest: Option<(&str, &str)>) -> String {
    let mut l = format!("https://{relay}/join/{rid}#{secret}");
    if let Some((u, p)) = guest {
        l.push_str(&format!("~~{u}.{p}"));
    }
    l
}

/// The guest login in an invite link (`~~<account>.<password>`), when it has one of the shapes Den Lite makes.
pub fn guest_in(link: &str) -> Option<(String, String)> {
    let frag = link.split_once('#')?.1;
    let g = frag.split("~~").nth(1)?;
    let (u, p) = g.split_once('.')?;
    let ok = |s: &str, n: std::ops::RangeInclusive<usize>| n.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    (u.starts_with('G') && ok(u, 4..=16) && ok(p, 8..=16)).then(|| (u.to_string(), p.to_string()))
}

// ------------------------------------------------------------------------------------------------ WebSocket client

trait Io: Read + Write + Send {}
impl<T: Read + Write + Send> Io for T {}

/// Where the relay's agent endpoint is: `wss://<host>/agent` (tests: `ws://127.0.0.1:<port>/agent`).
#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub path: String,
}

impl Endpoint {
    pub fn relay(name: &str) -> Endpoint {
        Endpoint { host: name.into(), port: 443, tls: true, path: "/agent".into() }
    }
}

fn connect(ep: &Endpoint) -> Result<Box<dyn Io>, String> {
    let addr = (ep.host.as_str(), ep.port).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address for the relay")?;
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10)).map_err(|e| format!("the relay didn't answer: {e}"))?;
    let _ = tcp.set_nodelay(true);
    tcp.set_read_timeout(Some(Duration::from_millis(50))).map_err(|e| e.to_string())?;
    let mut io: Box<dyn Io> = if ep.tls {
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        let cfg = rustls::ClientConfig::builder_with_provider(rustls::crypto::ring::default_provider().into())
            .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
            .map_err(|e| e.to_string())?
            .with_root_certificates(roots)
            .with_no_client_auth();
        let name = rustls::pki_types::ServerName::try_from(ep.host.clone()).map_err(|e| e.to_string())?;
        let conn = rustls::ClientConnection::new(Arc::new(cfg), name).map_err(|e| e.to_string())?;
        Box::new(rustls::StreamOwned::new(conn, tcp))
    } else {
        if !(ep.host == "127.0.0.1" || ep.host == "localhost") {
            return Err("only a relay on this PC may be reached without TLS".into());
        }
        Box::new(tcp)
    };
    let key = base64(&random(16)?);
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nUser-Agent: {VERSION}\r\n\r\n",
        ep.path, ep.host
    );
    io.write_all(req.as_bytes()).map_err(|e| format!("the relay didn't answer: {e}"))?;
    io.flush().map_err(|e| e.to_string())?;
    let mut head = Vec::new();
    let t0 = Instant::now();
    while !head.ends_with(b"\r\n\r\n") {
        if t0.elapsed() > Duration::from_secs(15) || head.len() > 8192 {
            return Err("the relay didn't take the connection".into());
        }
        let mut b = [0u8; 1];
        match io.read(&mut b) {
            Ok(0) => return Err("the relay closed the connection".into()),
            Ok(_) => head.push(b[0]),
            Err(e) if timeout(&e) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    let first = String::from_utf8_lossy(&head);
    if !first.starts_with("HTTP/1.1 101") {
        return Err(format!("the relay refused: {}", first.lines().next().unwrap_or("")));
    }
    Ok(io)
}

fn timeout(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut)
}

fn base64(b: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                o.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                o.push('=');
            }
        }
    }
    o
}

/// One client frame (masked, as a client must).
pub fn ws_frame(op: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0x80 | op];
    let n = payload.len();
    if n < 126 {
        f.push(0x80 | n as u8);
    } else if n < 65536 {
        f.push(0x80 | 126);
        f.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        f.push(0x80 | 127);
        f.extend_from_slice(&(n as u64).to_be_bytes());
    }
    let mask: [u8; 4] = random(4).ok().and_then(|v| v.try_into().ok()).unwrap_or([0x5a, 0x17, 0xc3, 0x9e]);
    f.extend_from_slice(&mask);
    f.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    f
}

/// The next whole frame at the start of `buf` (taken out): (opcode, payload). Fragments are joined.
pub fn ws_take(buf: &mut Vec<u8>) -> Option<(u8, Vec<u8>)> {
    let mut at = 0;
    let mut op0 = None;
    let mut out = Vec::new();
    loop {
        if buf.len() < at + 2 {
            return None;
        }
        let (b0, b1) = (buf[at], buf[at + 1]);
        let mut p = at + 2;
        let mut n = (b1 & 0x7f) as usize;
        if n == 126 {
            if buf.len() < p + 2 {
                return None;
            }
            n = u16::from_be_bytes([buf[p], buf[p + 1]]) as usize;
            p += 2;
        } else if n == 127 {
            if buf.len() < p + 8 {
                return None;
            }
            n = u64::from_be_bytes(buf[p..p + 8].try_into().ok()?) as usize;
            p += 8;
        }
        let mask = if b1 & 0x80 != 0 {
            if buf.len() < p + 4 {
                return None;
            }
            let m = [buf[p], buf[p + 1], buf[p + 2], buf[p + 3]];
            p += 4;
            Some(m)
        } else {
            None
        };
        if buf.len() < p + n {
            return None;
        }
        let mut data = buf[p..p + n].to_vec();
        if let Some(m) = mask {
            for (i, b) in data.iter_mut().enumerate() {
                *b ^= m[i % 4];
            }
        }
        let op = b0 & 0x0f;
        let fs = at;
        at = p + n;
        if op >= 8 {
            // a control frame (it may sit between fragments): taken out and returned on its own
            buf.drain(fs..at);
            return Some((op, data));
        }
        if op0.is_none() {
            op0 = Some(op);
        }
        out.extend(data);
        if b0 & 0x80 != 0 {
            buf.drain(..at);
            return Some((op0.unwrap_or(op), out));
        }
    }
}

// ------------------------------------------------------------------------------------------------ the tunnel

/// What the page shows about the tunnel.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TunnelState {
    pub running: bool,
    pub connected: bool,
    pub relay: String,
    pub realm_id: String,
    pub address: String,
    pub auth_port: u16,
    pub world_port: u16,
    pub streams: usize,
    pub last_error: String,
}

/// What the tunnel needs.
#[derive(Debug, Clone)]
pub struct Setup {
    pub endpoint: Endpoint,
    /// The relay's own name, as its challenge says it (wss host name for the real one).
    pub relay_name: String,
    pub seed: [u8; 32],
    pub name: String,
    pub invite_secret: String,
    /// Den Lite's Dens folder (`den-lite.remote`, `den-lite.public`).
    pub den_dir: PathBuf,
}

/// Den Lite's friends' listeners (`<dens>/den-lite.remote`: "<login port> <world port>").
pub fn remote_ports(den_dir: &Path) -> Option<(u16, u16)> {
    let t = std::fs::read_to_string(den_dir.join("den-lite.remote")).ok()?;
    let mut w = t.split_whitespace().map(|x| x.parse::<u16>().ok());
    Some((w.next()??, w.next()??))
}

fn ipv4(name: &str) -> String {
    (name, 0).to_socket_addrs().ok().and_then(|mut a| a.find(|x| x.is_ipv4())).map(|a| a.ip().to_string()).unwrap_or_else(|| name.to_string())
}

/// The hello the relay checks (relay.py check_hello), signed.
pub fn hello(s: &Setup, challenge: &str) -> Result<Value, String> {
    let public = public_key(&s.seed)?;
    let h = json!({
        "realm_id": realm_id(&public), "pubkey": hex(&public), "challenge": challenge, "relay": s.relay_name,
        "name": s.name, "visibility": "private", "invite_sha256": sha256_hex(&s.invite_secret), "signup": "off",
        "version": VERSION, "ts": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
    });
    let sig = sign_doc(&s.seed, &h, CTX_RELAY_LOGIN)?;
    Ok(json!({"hello": h, "sig": sig}))
}

enum Out {
    Frame(u8, Vec<u8>),
}

/// One connection to the relay, until it ends (Ok on `stop`).
pub fn run_once(s: &Setup, stop: &AtomicBool, state: &Mutex<TunnelState>) -> Result<(), String> {
    let mut io = connect(&s.endpoint)?;
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    let t0 = Instant::now();
    // the challenge, then the hello, then the welcome
    let mut read_doc = |io: &mut Box<dyn Io>, buf: &mut Vec<u8>| -> Result<Value, String> {
        loop {
            if let Some((op, p)) = ws_take(buf) {
                if op == 1 {
                    return serde_json::from_slice(&p).map_err(|_| "the relay's answer didn't look right".to_string());
                }
                if op == 8 {
                    return Err("the relay closed the connection".into());
                }
                continue;
            }
            if t0.elapsed() > Duration::from_secs(20) {
                return Err("the relay didn't answer".into());
            }
            match io.read(&mut chunk) {
                Ok(0) => return Err("the relay closed the connection".into()),
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(e) if timeout(&e) => {}
                Err(e) => return Err(e.to_string()),
            }
        }
    };
    let ch = read_doc(&mut io, &mut buf)?;
    let challenge = ch.get("challenge").and_then(Value::as_str).ok_or("the relay sent no challenge")?.to_string();
    if ch.get("relay").and_then(Value::as_str) != Some(s.relay_name.as_str()) {
        return Err("that isn't the relay it says it is".into());
    }
    io.write_all(&ws_frame(1, hello(s, &challenge)?.to_string().as_bytes())).map_err(|e| e.to_string())?;
    let w = read_doc(&mut io, &mut buf)?;
    if let Some(e) = w.get("error").and_then(Value::as_str) {
        return Err(format!("the relay refused: {e}"));
    }
    let wel = w.get("welcome").ok_or("the relay sent no welcome")?;
    let address = wel.get("address").and_then(Value::as_str).unwrap_or("").to_string();
    let port = |k: &str| wel.get(k).and_then(Value::as_u64).and_then(|p| u16::try_from(p).ok()).unwrap_or(0);
    let (auth_port, world_port) = (port("auth_port"), port("world_port"));
    if address.is_empty() || world_port == 0 {
        return Err("the relay's welcome didn't look right".into());
    }
    let public_addr = format!("{}:{world_port}", ipv4(&address));
    std::fs::write(s.den_dir.join("den-lite.public"), format!("{public_addr}\n")).map_err(|e| e.to_string())?;
    if let Ok(mut st) = state.lock() {
        st.connected = true;
        st.address = address.clone();
        (st.auth_port, st.world_port) = (auth_port, world_port);
        st.last_error.clear();
    }
    // the streams: local connections to Den Lite's friends' listeners
    let (tx, rx) = mpsc::channel::<Out>();
    let mut locals: HashMap<u32, TcpStream> = HashMap::new();
    let (mut last_in, mut last_ping, mut ping_n) = (Instant::now(), Instant::now(), 0u64);
    let res = loop {
        if stop.load(Ordering::Relaxed) {
            let _ = io.write_all(&ws_frame(8, &1000u16.to_be_bytes()));
            break Ok(());
        }
        // from the relay
        match io.read(&mut chunk) {
            Ok(0) => break Err("the relay closed the connection".to_string()),
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                last_in = Instant::now();
            }
            Err(e) if timeout(&e) => {}
            Err(e) => break Err(e.to_string()),
        }
        let mut quit = None;
        while let Some((op, p)) = ws_take(&mut buf) {
            match op {
                2 if p.len() >= 5 => {
                    let (kind, sid) = (p[0], u32::from_be_bytes([p[1], p[2], p[3], p[4]]));
                    let body = &p[5..];
                    match kind {
                        OPEN => {
                            let svc = if body.len() >= 2 { u16::from_be_bytes([body[0], body[1]]) } else { 0 };
                            let local = remote_ports(&s.den_dir).and_then(|(l, w)| match svc {
                                3724 => Some(l),
                                8085 => Some(w),
                                _ => None,
                            });
                            match local.and_then(|lp| TcpStream::connect(("127.0.0.1", lp)).ok()) {
                                Some(c) => {
                                    let _ = c.set_nodelay(true);
                                    if let Ok(r) = c.try_clone() {
                                        let tx = tx.clone();
                                        std::thread::spawn(move || pump(r, sid, tx));
                                    }
                                    locals.insert(sid, c);
                                }
                                None => {
                                    let _ = io.write_all(&ws_frame(2, &[&[CLOSE][..], &sid.to_be_bytes()].concat()));
                                }
                            }
                        }
                        DATA => {
                            let dead = locals.get_mut(&sid).is_some_and(|c| c.write_all(body).is_err());
                            if dead {
                                if let Some(c) = locals.remove(&sid) {
                                    let _ = c.shutdown(std::net::Shutdown::Both);
                                }
                                let _ = io.write_all(&ws_frame(2, &[&[CLOSE][..], &sid.to_be_bytes()].concat()));
                            }
                        }
                        CLOSE => {
                            if let Some(c) = locals.remove(&sid) {
                                let _ = c.shutdown(std::net::Shutdown::Both);
                            }
                        }
                        _ => {}
                    }
                }
                1 => {
                    let Ok(doc) = serde_json::from_slice::<Value>(&p) else { continue };
                    if let Some(n) = doc.get("req").cloned() {
                        // status: what the relay may show; every other request: not here
                        let res = if doc.get("op").and_then(Value::as_str) == Some("status") {
                            json!({"res": n, "ok": true, "status": {"online": true, "players": null, "core": "den-lite"}})
                        } else {
                            json!({"res": n, "ok": false, "error": "not on this realm"})
                        };
                        let _ = io.write_all(&ws_frame(1, res.to_string().as_bytes()));
                    }
                }
                8 => quit = Some(Err("the relay closed the connection".to_string())),
                9 => {
                    let _ = io.write_all(&ws_frame(10, &p));
                }
                _ => {}
            }
        }
        if let Some(q) = quit {
            break q;
        }
        // to the relay: what the local connections read
        while let Ok(Out::Frame(kind, f)) = rx.try_recv() {
            if kind == CLOSE {
                let sid = u32::from_be_bytes([f[1], f[2], f[3], f[4]]);
                if locals.remove(&sid).is_none() {
                    continue; // the relay closed it first
                }
            }
            if io.write_all(&ws_frame(2, &f)).is_err() {
                break;
            }
        }
        if last_ping.elapsed() >= PING_EVERY {
            ping_n += 1;
            last_ping = Instant::now();
            let _ = io.write_all(&ws_frame(1, json!({"ping": ping_n}).to_string().as_bytes()));
        }
        if last_in.elapsed() > SILENT_MAX {
            break Err("the relay stopped answering".to_string());
        }
        if let Ok(mut st) = state.lock() {
            st.streams = locals.len();
        }
    };
    for (_, c) in locals {
        let _ = c.shutdown(std::net::Shutdown::Both);
    }
    let _ = std::fs::remove_file(s.den_dir.join("den-lite.public"));
    if let Ok(mut st) = state.lock() {
        st.connected = false;
        st.streams = 0;
    }
    res
}

/// A local connection's bytes, up the tunnel, then CLOSE.
fn pump(mut r: TcpStream, sid: u32, tx: mpsc::Sender<Out>) {
    let mut b = vec![0u8; MAX_CHUNK];
    loop {
        match r.read(&mut b) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let mut f = vec![DATA];
                f.extend_from_slice(&sid.to_be_bytes());
                f.extend_from_slice(&b[..n]);
                if tx.send(Out::Frame(DATA, f)).is_err() {
                    return;
                }
            }
        }
    }
    let mut f = vec![CLOSE];
    f.extend_from_slice(&sid.to_be_bytes());
    let _ = tx.send(Out::Frame(CLOSE, f));
}

struct Handle {
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<TunnelState>>,
}

static TUNNEL: Mutex<Option<Handle>> = Mutex::new(None);

/// Starts the tunnel (once; again while it runs: nothing), reconnecting with a growing pause until stopped.
pub fn start(s: Setup) -> TunnelState {
    let mut g = TUNNEL.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(h) = g.as_ref() {
        if !h.stop.load(Ordering::Relaxed) {
            return h.state.lock().map(|x| x.clone()).unwrap_or_default();
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    let public = public_key(&s.seed).unwrap_or([0; 32]);
    let state = Arc::new(Mutex::new(TunnelState { running: true, relay: s.relay_name.clone(), realm_id: realm_id(&public), ..Default::default() }));
    let (st2, stop2) = (state.clone(), stop.clone());
    std::thread::spawn(move || {
        let mut pause = 2u64;
        while !stop2.load(Ordering::Relaxed) {
            let t = Instant::now();
            if let Err(e) = run_once(&s, &stop2, &st2) {
                if let Ok(mut x) = st2.lock() {
                    x.last_error = e;
                }
            }
            if t.elapsed() > Duration::from_secs(60) {
                pause = 2;
            }
            for _ in 0..pause * 10 {
                if stop2.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            pause = (pause * 2).min(60);
        }
        if let Ok(mut x) = st2.lock() {
            x.running = false;
        }
    });
    let out = state.lock().map(|x| x.clone()).unwrap_or_default();
    *g = Some(Handle { stop, state });
    out
}

pub fn stop() {
    if let Some(h) = TUNNEL.lock().unwrap_or_else(|e| e.into_inner()).take() {
        h.stop.store(true, Ordering::Relaxed);
    }
}

pub fn state() -> TunnelState {
    TUNNEL.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|h| h.state.lock().ok().map(|x| x.clone())).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same key, id and signature as the Host kit's Python (wunet/keys.py) for the same seed and document.
    #[test]
    fn keys_and_signatures_match_the_host_kits() {
        let seed: [u8; 32] = core::array::from_fn(|i| i as u8);
        let p = public_key(&seed).unwrap();
        assert_eq!(hex(&p), "03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8");
        assert_eq!(realm_id(&p), "kzdvvj2umnduyauf");
        let doc = json!({"b": 1, "a": "x\u{e9}\"", "c": [true, null], "n": {"z": 2, "y": "q"}});
        assert_eq!(canonical(&doc), "{\"a\":\"x\u{e9}\\\"\",\"b\":1,\"c\":[true,null],\"n\":{\"y\":\"q\",\"z\":2}}");
        assert_eq!(sign_doc(&seed, &doc, CTX_RELAY_LOGIN).unwrap(),
            "cf2250505a43dbbf12429cc3de8972777fd6ed23d0e60003c257a0773137730d531ce22ba0202cf0bbe02677ff9c1057ceec352c7b7c2b160a9921701a279e04");
        let d = tempfile::tempdir().unwrap();
        let k = load_or_create_key(&key_path(d.path())).unwrap();
        assert_eq!(load_or_create_key(&key_path(d.path())).unwrap(), k, "made once, then kept");
    }

    #[test]
    fn the_invite_link_carries_the_knock_and_the_guest_login() {
        let s = new_secret().unwrap();
        assert!(s.len() == 24);
        let l = invite_link("relay.openvanilla.app", "kzdvvj2umnduyauf", &s, Some(("GABCDE2345", "ABCDEFGH23456789")));
        let (relay, rid, secret) = crate::realms::parse_invite(&l).expect("the launcher's own invite parser takes it");
        assert_eq!((relay.as_str(), rid.as_str(), secret.as_str()), ("relay.openvanilla.app", "kzdvvj2umnduyauf", s.as_str()));
        assert_eq!(guest_in(&l), Some(("GABCDE2345".into(), "ABCDEFGH23456789".into())));
        assert_eq!(guest_in(&invite_link("r.x", "kzdvvj2umnduyauf", &s, None)), None);
        assert_eq!(guest_in("https://r/join/x#AAAA~~DEN.den"), None, "only a guest login of Den Lite's shape");
        assert_eq!(guest_in("https://r/join/x#AAAA~HOSTKEY"), None, "the Host kit's host link isn't a guest login");
    }

    #[test]
    fn websocket_frames_both_ways() {
        let f = ws_frame(2, b"hello");
        let mut buf = f.clone();
        assert_eq!(ws_take(&mut buf), Some((2, b"hello".to_vec())), "masked frames read back");
        assert!(buf.is_empty());
        // unmasked server frames, a long one, a fragmented one, and half a frame
        let mut s = vec![0x82, 126];
        s.extend_from_slice(&300u16.to_be_bytes());
        s.extend(std::iter::repeat_n(7u8, 300));
        s.extend_from_slice(&[0x01, 2, b'a', b'b', 0x80, 1, b'c']);
        s.extend_from_slice(&[0x81, 5, b'x']);
        assert_eq!(ws_take(&mut s).map(|(o, p)| (o, p.len())), Some((2, 300)));
        assert_eq!(ws_take(&mut s), Some((1, b"abc".to_vec())));
        assert_eq!(ws_take(&mut s), None, "incomplete");
        assert_eq!(s, vec![0x81, 5, b'x']);
        assert_eq!(base64(b"any carnal pleas"), "YW55IGNhcm5hbCBwbGVhcw==");
    }
}
