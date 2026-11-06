//! Play with friends from a single player world (docs/SINGLE-PLAYER-ONE-CLICK.md, "Co-op"): Invite a friend for the
//! Built-in engine's worlds (Single player, the original world), as Your Den's invite works for Den Lite.
//!
//! The engine stays as it is: every server listens on 127.0.0.1 only, and its realm list says 127.0.0.1:8085 (so the
//! player's own game always goes straight in, even with no internet). A friend comes in one of two ways, each
//! through the launcher, which is the only thing that listens anywhere else while sharing is on:
//!
//! * **Same network (LAN):** a small forwarder on this PC's own network address ([`Lan`]: `<LAN ip>:3724` and a
//!   world port beside it) passes the friend's game to the engine's 127.0.0.1 ports;
//! * **From anywhere:** the Leyline relay tunnel (host/litenet.rs, the same as Your Den's: one outbound connection,
//!   private, the relay only passes the game's bytes through), to the same 127.0.0.1 ports.
//!
//! Both ways reach the login server from this PC itself, so its realm list would send the friend's game to
//! 127.0.0.1: back to the friend's own PC. [`AuthRewriter`] follows the login server's answers on such a
//! connection (the login protocol's own messages, in order) and writes the realm list's address as the way the
//! friend came: the LAN forwarder's world port, or the relay's address for this world. Nothing else is changed;
//! anything it doesn't recognise is passed through untouched from there on.
//!
//! The friend's login: one per invite, made over the engine's console (SOAP, 127.0.0.1 only) with a generated
//! password, carried in the link after `~~` as Your Den's guest logins are ([`guest_lines`]). Friends' accounts
//! are players (no GM rights) and are never listed on the Characters page.

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CMD_CHALLENGE: u8 = 0x00;
const CMD_PROOF: u8 = 0x01;
const CMD_RECONNECT_CHALLENGE: u8 = 0x02;
const CMD_RECONNECT_PROOF: u8 = 0x03;
const CMD_REALM_LIST: u8 = 0x10;
/// A realm list larger than this isn't one of ours: passed through.
const LIST_MAX: usize = 16 * 1024;

/// Follows the login server's side of one connection and rewrites the realm list's loopback addresses.
#[derive(Debug, Clone)]
pub struct AuthRewriter {
    target: String,
    buf: Vec<u8>,
    /// Lost the thread (an unknown message): everything passes through from here on.
    through: bool,
    pub rewrote: usize,
}

impl AuthRewriter {
    /// `target`: "ip:port" the friend's game should go to for the world.
    pub fn new(target: &str) -> AuthRewriter {
        AuthRewriter { target: target.to_string(), buf: Vec::new(), through: false, rewrote: 0 }
    }

    /// The bytes to send on for these (whole messages only; a part waits for the rest).
    pub fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        if self.through {
            return data.to_vec();
        }
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            match message_len(&self.buf) {
                Len::Need => break,
                Len::Unknown => {
                    self.through = true;
                    out.append(&mut self.buf);
                    break;
                }
                Len::Whole(n) => {
                    let msg: Vec<u8> = self.buf.drain(..n).collect();
                    if msg[0] == CMD_REALM_LIST {
                        match rewrite_list(&msg, &self.target) {
                            Some((m, k)) => {
                                self.rewrote += k;
                                out.extend(m);
                            }
                            None => out.extend(msg),
                        }
                    } else {
                        out.extend(msg);
                    }
                }
            }
        }
        out
    }
}

enum Len {
    Need,
    Unknown,
    Whole(usize),
}

/// How long the login server's message at the start of `b` is (its own formats, the client of this build).
fn message_len(b: &[u8]) -> Len {
    let Some(&cmd) = b.first() else { return Len::Need };
    let need = |n: usize| if b.len() >= n { Len::Whole(n) } else { Len::Need };
    match cmd {
        CMD_CHALLENGE => {
            if b.len() < 3 {
                return Len::Need;
            }
            if b[2] != 0 {
                return Len::Whole(3);
            }
            // cmd, 0, ok, B[32], g_len, g, N_len, N, salt[32], crc salt[16], security flags (+ their parts)
            let mut at = 35;
            if b.len() <= at {
                return Len::Need;
            }
            at += 1 + b[at] as usize;
            if b.len() <= at {
                return Len::Need;
            }
            at += 1 + b[at] as usize + 32 + 16;
            if b.len() <= at {
                return Len::Need;
            }
            let flags = b[at];
            at += 1;
            if flags & 0x01 != 0 {
                at += 20;
            }
            if flags & 0x02 != 0 {
                at += 12;
            }
            if flags & 0x04 != 0 {
                at += 1;
            }
            need(at)
        }
        CMD_PROOF => {
            if b.len() < 2 {
                return Len::Need;
            }
            need(if b[1] == 0 { 32 } else { 4 })
        }
        CMD_RECONNECT_CHALLENGE => {
            if b.len() < 2 {
                return Len::Need;
            }
            need(if b[1] == 0 { 34 } else { 2 })
        }
        CMD_RECONNECT_PROOF => need(4),
        CMD_REALM_LIST => {
            if b.len() < 3 {
                return Len::Need;
            }
            let size = u16::from_le_bytes([b[1], b[2]]) as usize;
            if size > LIST_MAX {
                return Len::Unknown;
            }
            need(3 + size)
        }
        _ => Len::Unknown,
    }
}

/// A loopback realm address ("127.x.x.x:port").
fn loopback(addr: &str) -> bool {
    addr.rsplit_once(':').and_then(|(h, _)| h.parse::<IpAddr>().ok()).is_some_and(|ip| ip.is_loopback())
}

/// The realm list message with each loopback address replaced by `target` (and its size again); None when it
/// doesn't read as one.
pub fn rewrite_list(msg: &[u8], target: &str) -> Option<(Vec<u8>, usize)> {
    let body = msg.get(3..)?;
    // u32 unused, u16 count, the realms, then 0x10 0x00
    if body.len() < 6 {
        return None;
    }
    let count = u16::from_le_bytes([body[4], body[5]]) as usize;
    let mut out = body[..6].to_vec();
    let mut at = 6;
    let mut changed = 0;
    let cstr = |at: usize| -> Option<(Vec<u8>, usize)> {
        let end = body.get(at..)?.iter().position(|c| *c == 0)?;
        Some((body[at..at + end].to_vec(), at + end + 1))
    };
    for _ in 0..count {
        let head = body.get(at..at + 3)?;
        let flags = head[2];
        out.extend_from_slice(head);
        at += 3;
        let (name, next) = cstr(at)?;
        out.extend(&name);
        out.push(0);
        at = next;
        let (addr, next) = cstr(at)?;
        let addr_s = String::from_utf8(addr.clone()).ok()?;
        if loopback(&addr_s) {
            out.extend(target.as_bytes());
            changed += 1;
        } else {
            out.extend(&addr);
        }
        out.push(0);
        at = next;
        // population (f32), characters, time zone, id; with "specify build": major, minor, bugfix, build (u16)
        let tail = 4 + 3 + if flags & 0x04 != 0 { 5 } else { 0 };
        out.extend_from_slice(body.get(at..at + tail)?);
        at += tail;
    }
    out.extend_from_slice(body.get(at..)?);
    let mut m = vec![CMD_REALM_LIST];
    m.extend_from_slice(&u16::try_from(out.len()).ok()?.to_le_bytes());
    m.extend(out);
    Some((m, changed))
}

// ------------------------------------------------------------------------------------------------ the friend's login

/// The console lines that make a friend's login: a player account (no GM rights) that plays this game's expansion.
pub fn guest_lines(user: &str, pass: &str) -> Result<[String; 2], String> {
    let ok = |s: &str, n: std::ops::RangeInclusive<usize>| n.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if !(user.starts_with('G') && ok(user, 4..=16) && ok(pass, 8..=16)) {
        return Err("That login doesn't look right.".into());
    }
    Ok([format!("account create {user} {pass}"), format!("account set addon {user} 2")])
}

/// A new friend's login: G + 9 letters and digits, a 12-character password (the shapes litenet::guest_in reads).
pub fn new_guest() -> Result<(String, String), String> {
    Ok((format!("G{}", crate::host::engine::secret(9)?), crate::host::engine::secret(12)?))
}

/// The console's answer to `account create`: made, or already there (fine), else why not.
pub fn created(answer: &str) -> Result<(), String> {
    let a = answer.to_ascii_lowercase();
    if a.contains("created") || a.contains("already exist") {
        Ok(())
    } else if a.trim().is_empty() {
        Err("the world gave no answer".into())
    } else {
        Err(answer.trim().chars().take(120).collect())
    }
}

// ------------------------------------------------------------------------------------------------ files

/// Where a world's sharing keeps its things: its relay key, its invite secret, the tunnel's files.
pub fn dir(world_root: &Path) -> PathBuf {
    world_root.join("share")
}

/// `<share>/den-lite.remote`: the engine's own ports for the tunnel (litenet routes OPEN 3724 / 8085 by it).
pub fn remote_text() -> String {
    format!("{} {}\n", crate::host::engine::LOGIN_PORT, crate::host::engine::WORLD_PORT)
}

/// The invite secret, made once (`<share>/invite.secret`).
pub fn invite_secret(share: &Path) -> Result<String, String> {
    let f = share.join("invite.secret");
    if let Ok(t) = std::fs::read_to_string(&f) {
        let t = t.trim().to_string();
        if t.len() >= 16 {
            return Ok(t);
        }
    }
    let s = crate::host::litenet::new_secret()?;
    std::fs::create_dir_all(share).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&f, s.as_bytes()).map_err(|e| e.to_string())?;
    Ok(s)
}

// ------------------------------------------------------------------------------------------------ same network

/// The same-network forwarder: `<ip>:<login>` and `<ip>:<world>` to the engine's 127.0.0.1 ports, the login
/// stream's realm list rewritten to `<ip>:<world>`.
pub struct Lan {
    stop: Arc<AtomicBool>,
    pub ip: IpAddr,
    pub login: u16,
    pub world: u16,
}

static LAN: Mutex<Option<Lan>> = Mutex::new(None);

fn bind(ip: IpAddr, ports: &[u16]) -> Option<TcpListener> {
    ports.iter().find_map(|p| TcpListener::bind(SocketAddr::new(ip, *p)).ok())
}

/// Starts it on this PC's network address (again while it runs: the same). The game's own ports first (a friend's
/// game then needs no port in its realm list), others beside them when those are taken.
pub fn lan_start(ip: IpAddr, to_login: u16, to_world: u16) -> Result<(IpAddr, u16, u16), String> {
    let mut g = LAN.lock().map_err(|_| "busy")?;
    if let Some(l) = g.as_ref() {
        if l.ip == ip && !l.stop.load(Ordering::Relaxed) {
            return Ok((l.ip, l.login, l.world));
        }
        l.stop.store(true, Ordering::Relaxed);
    }
    let login = bind(ip, &[to_login, 13724, 23724]).ok_or("this PC's network address didn't take the login port (a firewall or another program)")?;
    let world = bind(ip, &[to_world, 18085, 28085]).ok_or("this PC's network address didn't take the world port")?;
    let (lp, wp) = (login.local_addr().map_err(|e| e.to_string())?.port(), world.local_addr().map_err(|e| e.to_string())?.port());
    let stop = Arc::new(AtomicBool::new(false));
    let target = format!("{ip}:{wp}");
    for (l, to, rewrite) in [(login, to_login, true), (world, to_world, false)] {
        let (stop, target) = (stop.clone(), target.clone());
        let _ = l.set_nonblocking(true);
        std::thread::spawn(move || accept_loop(l, to, rewrite.then_some(target), stop));
    }
    *g = Some(Lan { stop, ip, login: lp, world: wp });
    Ok((ip, lp, wp))
}

pub fn lan_stop() {
    if let Ok(mut g) = LAN.lock() {
        if let Some(l) = g.take() {
            l.stop.store(true, Ordering::Relaxed);
        }
    }
}

/// (ip, login port) while it runs.
pub fn lan_state() -> Option<(IpAddr, u16)> {
    LAN.lock().ok().and_then(|g| g.as_ref().filter(|l| !l.stop.load(Ordering::Relaxed)).map(|l| (l.ip, l.login)))
}

fn accept_loop(l: TcpListener, to: u16, rewrite: Option<String>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        match l.accept() {
            Ok((c, from)) => {
                // the same network only: a private or link-local address
                if !crate::host::denways::private(from.ip()) {
                    continue;
                }
                let (rw, stop) = (rewrite.clone(), stop.clone());
                std::thread::spawn(move || {
                    let _ = c.set_nonblocking(false);
                    if let Ok(up) = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], to)), Duration::from_secs(3)) {
                        relay_pair(c, up, rw.map(|t| AuthRewriter::new(&t)), stop);
                    }
                });
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// Bytes both ways until either side closes; the server's side through the rewriter when there is one.
fn relay_pair(client: TcpStream, server: TcpStream, mut rw: Option<AuthRewriter>, stop: Arc<AtomicBool>) {
    let _ = client.set_nodelay(true);
    let _ = server.set_nodelay(true);
    let (Ok(mut c2), Ok(mut s2)) = (client.try_clone(), server.try_clone()) else { return };
    let up = std::thread::spawn(move || {
        let mut b = vec![0u8; 32 * 1024];
        let mut client = client;
        let mut server = server;
        loop {
            match client.read(&mut b) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if server.write_all(&b[..n]).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = server.shutdown(std::net::Shutdown::Both);
    });
    let mut b = vec![0u8; 32 * 1024];
    while !stop.load(Ordering::Relaxed) {
        match s2.read(&mut b) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let out = match rw.as_mut() {
                    Some(r) => r.feed(&b[..n]),
                    None => b[..n].to_vec(),
                };
                if c2.write_all(&out).is_err() {
                    break;
                }
            }
        }
    }
    let _ = c2.shutdown(std::net::Shutdown::Both);
    let _ = up.join();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A realm list as the login server sends it to this client: one realm, ours.
    fn list(addr: &str, flags: u8) -> Vec<u8> {
        let mut body = vec![0, 0, 0, 0, 1, 0];
        body.extend([0, 0, flags]);
        body.extend(b"Original world\0");
        body.extend(addr.as_bytes());
        body.push(0);
        body.extend(0.0f32.to_le_bytes());
        body.extend([2, 1, 1]);
        if flags & 0x04 != 0 {
            body.extend([3, 3, 5, 0x3c, 0x30]);
        }
        body.extend([0x10, 0x00]);
        let mut m = vec![0x10];
        m.extend((body.len() as u16).to_le_bytes());
        m.extend(body);
        m
    }

    fn challenge_ok(flags: u8) -> Vec<u8> {
        let mut m = vec![0x00, 0x00, 0x00];
        m.extend([7u8; 32]);
        m.extend([1, 7]);
        m.push(32);
        m.extend([9u8; 32]);
        m.extend([5u8; 32]);
        m.extend([6u8; 16]);
        m.push(flags);
        if flags & 0x04 != 0 {
            m.push(1);
        }
        m
    }

    #[test]
    fn the_realm_list_gets_the_friends_way_in_and_nothing_else_changes() {
        let (m, n) = rewrite_list(&list("127.0.0.1:8085", 0), "203.0.113.9:40123").unwrap();
        assert_eq!(n, 1);
        assert_eq!(m, list("203.0.113.9:40123", 0), "the same list with the new address and its size");
        let (m, n) = rewrite_list(&list("127.0.0.1:8085", 0x04), "192.168.1.20:8085").unwrap();
        assert_eq!((m, n), (list("192.168.1.20:8085", 0x04), 1), "the build after it too");
        let (m, n) = rewrite_list(&list("198.51.100.4:8085", 0), "x:1").unwrap();
        assert_eq!((m, n), (list("198.51.100.4:8085", 0), 0), "an address that isn't this PC's is left");
        assert!(rewrite_list(&[0x10, 3, 0, 1, 2, 3], "x:1").is_none(), "not a list");
    }

    #[test]
    fn the_login_stream_is_followed_message_by_message() {
        let mut stream = challenge_ok(0x04);
        let mut proof = vec![0x01, 0x00];
        proof.extend([3u8; 30]);
        stream.extend(&proof);
        stream.extend(list("127.0.0.1:8085", 0));
        let mut want = challenge_ok(0x04);
        want.extend(&proof);
        want.extend(list("10.0.0.5:8085", 0));
        // whole, and a byte at a time
        let mut r = AuthRewriter::new("10.0.0.5:8085");
        assert_eq!(r.feed(&stream), want);
        assert_eq!(r.rewrote, 1);
        let mut r = AuthRewriter::new("10.0.0.5:8085");
        let mut got = Vec::new();
        for b in &stream {
            got.extend(r.feed(&[*b]));
        }
        assert_eq!(got, want);
        // the list again later (the game asks every few seconds on the realm list)
        assert_eq!(r.feed(&list("127.0.0.1:8085", 0)), list("10.0.0.5:8085", 0));
        // a failed login: three bytes, then whatever follows passes
        let mut r = AuthRewriter::new("x:1");
        assert_eq!(r.feed(&[0x00, 0x00, 0x04]), vec![0x00, 0x00, 0x04]);
        // something it doesn't know: everything through untouched from there
        let mut r = AuthRewriter::new("x:1");
        let odd = [0x77u8, 1, 2, 3];
        assert_eq!(r.feed(&odd), odd.to_vec());
        assert_eq!(r.feed(&list("127.0.0.1:8085", 0)), list("127.0.0.1:8085", 0));
    }

    #[test]
    fn a_friends_login_is_a_player_of_the_guest_shape() {
        let [a, b] = guest_lines("GABCDE2345", "ABCDEFGH2345").unwrap();
        assert_eq!(a, "account create GABCDE2345 ABCDEFGH2345");
        assert_eq!(b, "account set addon GABCDE2345 2");
        assert!(guest_lines("SOLO", "ABCDEFGH2345").is_err(), "never the host's own login");
        assert!(guest_lines("GABC; x", "ABCDEFGH2345").is_err());
        let (u, p) = new_guest().unwrap();
        assert!(guest_lines(&u, &p).is_ok());
        let link = crate::host::litenet::invite_link("relay.x", "kzdvvj2umnduyauf", "S", Some((&u, &p)));
        assert_eq!(crate::host::litenet::guest_in(&link), Some((u, p)), "the friend's launcher reads it as a Den's guest login");
        assert!(created("Account created: GABC").is_ok() && created("Account already exists").is_ok());
        assert!(created("").is_err() && created("Too long").is_err());
    }

    #[test]
    fn the_same_network_way_passes_the_game_through_with_the_realm_list_rewritten() {
        // a stand-in login server on 127.0.0.1 that sends a realm list when it gets any byte
        let srv = TcpListener::bind("127.0.0.1:0").unwrap();
        let sp = srv.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut c, _)) = srv.accept() {
                let mut b = [0u8; 5];
                if c.read_exact(&mut b).is_ok() {
                    let _ = c.write_all(&list("127.0.0.1:8085", 0));
                }
            }
        });
        // the forwarder on 127.0.0.2 stands in for the LAN address (loopback is "private" for this test's check)
        let (client, server) = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            let p = l.local_addr().unwrap().port();
            let c = TcpStream::connect(("127.0.0.1", p)).unwrap();
            let (inbound, _) = l.accept().unwrap();
            (c, inbound)
        };
        let up = TcpStream::connect(("127.0.0.1", sp)).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        std::thread::spawn(move || relay_pair(server, up, Some(AuthRewriter::new("192.168.7.7:8085")), stop));
        let mut client = client;
        client.write_all(&[0x10, 0, 0, 0, 0]).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let want = list("192.168.7.7:8085", 0);
        let mut got = vec![0u8; want.len()];
        client.read_exact(&mut got).unwrap();
        assert_eq!(got, want);
    }
}
