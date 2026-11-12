//! Friends' transport (docs/LEYLINE-FRIENDS.md, "Transport"): how the signed, sealed messages of p2p/friends.rs get
//! from one launcher to another with no server of ours in the middle.
//!
//! * **Same network (works with no internet at all):** a signed beacon every 10 s by UDP broadcast on
//!   [`LAN_PORT`] (only while "Be seen on this network" isn't off), and a small TCP listener on [`TCP_PORT`] (or
//!   any free port: the beacon names it) taking one sealed envelope per connection, one JSON line each way.
//! * **Straight across the internet:** the same listener, when a friend chose to share an address that reaches it
//!   (their own port forward; the presence message carries it, sealed, to friends only). Hole punching isn't built.
//! * **Otherwise:** any Leyline relay's friends' mailbox (hosting/relay/wurelay/mailbox.py, `RELAY_FRIENDS_MAILBOX`),
//!   a blind drop box: it sees a peer id and sizes, never who wrote or what. Fetched every minute with a signed proof.
//!
//! Every message is opened and checked here (friends.rs `check_msg`, each nonce once) and limited per address, so
//! a neighbour or a stranger can't flood the cache. Nothing here carries game files.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::friends::{self, Book, Event, Via};
use crate::identity::{self, PeerIdentity};

pub const LAN_PORT: u16 = 41190;
pub const TCP_PORT: u16 = 41191;
pub const CTX_FETCH: &str = "leyline-mailbox-fetch-v1";
/// A friend counts as online this long after their last presence.
pub const ONLINE_WINDOW: u64 = 600;
const BEACON_EVERY: u64 = 10;
const PRESENCE_EVERY: u64 = 240;
const MAILBOX_EVERY: u64 = 60;
const NEAR_MAX: usize = 64;
const SEEN_MAX: usize = 4096;
const CONNS_MAX: usize = 16;
/// Messages per minute from one address (TCP) and beacons per minute from one address (UDP).
pub const PER_MIN: u32 = 30;
const IO: Duration = Duration::from_secs(4);

pub fn now() -> u64 {
    super::mesh::now_secs()
}

// ------------------------------------------------------------------------------------------------ limits

/// At most `per_min` per key each minute; remembers at most 4096 keys.
#[derive(Debug)]
pub struct Limiter {
    per_min: u32,
    hits: HashMap<String, (u64, u32)>,
}

impl Limiter {
    pub fn new(per_min: u32) -> Self {
        Limiter { per_min, hits: HashMap::new() }
    }

    pub fn allow(&mut self, key: &str, now: u64) -> bool {
        let minute = now / 60;
        if self.hits.len() >= 4096 && !self.hits.contains_key(key) {
            self.hits.retain(|_, (m, _)| *m == minute);
            if self.hits.len() >= 4096 {
                return false;
            }
        }
        let h = self.hits.entry(key.to_string()).or_insert((minute, 0));
        if h.0 != minute {
            *h = (minute, 0);
        }
        h.1 += 1;
        h.1 <= self.per_min
    }
}

/// Nonces already taken (a replayed message is dropped).
#[derive(Debug, Default)]
pub struct Seen {
    set: HashSet<String>,
    order: VecDeque<String>,
}

impl Seen {
    /// True the first time.
    pub fn first(&mut self, nonce: &str) -> bool {
        if !self.set.insert(nonce.to_string()) {
            return false;
        }
        self.order.push_back(nonce.to_string());
        if self.order.len() > SEEN_MAX {
            if let Some(o) = self.order.pop_front() {
                self.set.remove(&o);
            }
        }
        true
    }
}

/// Whether an address is this PC's own network: private, link-local, loopback (IPv4 or IPv6 ULA / link-local).
pub fn local_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local(),
        IpAddr::V6(v) => v.is_loopback() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80,
    }
}

// ------------------------------------------------------------------------------------------------ the hub

#[derive(Debug, Clone)]
pub struct Near {
    pub addr: SocketAddr,
    pub name: Option<String>,
    pub seen: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct NetState {
    pub running: bool,
    /// The UDP port beacons are heard on (None: another program has it; this PC still sends its own).
    pub lan_port: Option<u16>,
    pub tcp_port: Option<u16>,
    pub mailbox: Option<String>,
    pub last_error: String,
    /// Things that happened, newest first, for the page's notice ("Bo accepted", …).
    pub notices: Vec<String>,
}

/// Everything the transport works with: this PC's identity, the book, who's near, and the limits.
pub struct Hub {
    pub app: PathBuf,
    pub me: PeerIdentity,
    pub book: Mutex<Book>,
    pub near: Mutex<HashMap<String, Near>>,
    seen: Mutex<Seen>,
    limit: Mutex<Limiter>,
    pub state: Mutex<NetState>,
    /// What this player is playing now and the world they have open to friends (the launcher sets them).
    pub playing: Mutex<(Option<String>, Option<Value>)>,
    /// An address of this PC that friends may use straight from the internet (only if the player shares one).
    pub direct: Mutex<Option<String>>,
    stop: AtomicBool,
    /// Which start the running threads belong to: a Stop then a quick Start never leaves the old threads running
    /// beside the new ones (each thread ends once its generation isn't the current one).
    gen: AtomicU64,
    conns: AtomicUsize,
}

impl Hub {
    pub fn new(app: &Path, me: PeerIdentity) -> Arc<Hub> {
        let (book, _) = Book::load(app, &me, now());
        Arc::new(Hub {
            app: app.to_path_buf(),
            me,
            book: Mutex::new(book),
            near: Mutex::new(HashMap::new()),
            seen: Mutex::new(Seen::default()),
            limit: Mutex::new(Limiter::new(PER_MIN)),
            state: Mutex::new(NetState::default()),
            playing: Mutex::new((None, None)),
            direct: Mutex::new(None),
            stop: AtomicBool::new(false),
            gen: AtomicU64::new(0),
            conns: AtomicUsize::new(0),
        })
    }

    pub fn save(&self) -> Result<(), String> {
        self.book.lock().unwrap().save(&self.app)
    }

    /// The threads of start number `gen` keep going.
    fn live(&self, gen: u64) -> bool {
        !self.stop.load(Ordering::Relaxed) && self.gen.load(Ordering::SeqCst) == gen
    }

    /// The world this player had open to friends closed (its sharing stopped): friends stop seeing it at once.
    /// True when it was the one shown.
    pub fn world_closed(&self, kind: &str) -> bool {
        let mut p = self.playing.lock().unwrap();
        if p.1.as_ref().and_then(|o| o["kind"].as_str()) != Some(kind) {
            return false;
        }
        *p = (None, None);
        true
    }

    fn notice(&self, t: String) {
        let mut s = self.state.lock().unwrap();
        s.notices.insert(0, t);
        s.notices.truncate(10);
    }

    fn name_of(&self, pid: &str) -> String {
        let b = self.book.lock().unwrap();
        let v = b.view(&self.me, now(), &[], ONLINE_WINDOW);
        v["friends"].as_array().into_iter().flatten().chain(v["incoming"].as_array().into_iter().flatten())
            .find(|f| f["peer_id"] == pid).and_then(|f| f["name"].as_str()).filter(|n| !n.is_empty()).map(String::from)
            .unwrap_or_else(|| format!("Hero {}", &pid[..4.min(pid.len())]))
    }

    /// One envelope in: opened, checked, its nonce taken once, applied to the book, saved.
    pub fn take(&self, env: &Value, via: Via, now: u64) -> Result<Event, String> {
        let rec = friends::open_envelope(&self.me, env)?;
        friends::check_msg(&rec, &self.me.public_hex(), now)?;
        let nonce = rec["msg"]["nonce"].as_str().unwrap_or("").to_string();
        if !self.seen.lock().unwrap().first(&nonce) {
            return Ok(Event::Ignored("seen"));
        }
        let ev = self.book.lock().unwrap().receive(&self.me, &rec, via, now)?;
        if !matches!(ev, Event::Ignored(_)) {
            self.save()?;
        }
        let say = match &ev {
            Event::Request(p) => Some(format!("{} wants to be friends.", self.name_of(p))),
            Event::Accepted(p) => Some(format!("{} accepted: you're friends now.", self.name_of(p))),
            Event::Invite(p) => Some(format!("{} invited you to their world.", self.name_of(p))),
            _ => None,
        };
        if let Some(s) = say {
            self.notice(s);
        }
        Ok(ev)
    }

    /// The people near: (public key hex, name if they show it). Old beacons are forgotten.
    pub fn near_list(&self, now: u64) -> Vec<(String, Option<String>)> {
        let mut n = self.near.lock().unwrap();
        n.retain(|_, v| now.saturating_sub(v.seen) <= 3 * BEACON_EVERY + 5);
        n.iter().map(|(k, v)| (k.clone(), v.name.clone())).collect()
    }

    /// A beacon heard from `from`.
    pub fn hear(&self, bytes: &[u8], from: SocketAddr, now: u64) -> bool {
        if !local_ip(&from.ip()) || !self.limit.lock().unwrap().allow(&format!("u{}", from.ip()), now) {
            return false;
        }
        let Ok((pk, port, name)) = friends::check_beacon(bytes, now) else { return false };
        if pk == self.me.public_hex() {
            return false;
        }
        let mut n = self.near.lock().unwrap();
        if n.len() >= NEAR_MAX && !n.contains_key(&pk) {
            if let Some(old) = n.iter().min_by_key(|(_, v)| v.seen).map(|(k, _)| k.clone()) {
                n.remove(&old);
            }
        }
        n.insert(pk, Near { addr: SocketAddr::new(from.ip(), port), name, seen: now });
        true
    }

    /// Send a signed message to the key `to` (hex): the same network first, then a direct address the friend
    /// shared, then the mailbox (their card's relay, or `relay` for someone not yet a friend). Returns the way used.
    pub fn deliver(&self, to: &str, msg: &Value, relay: Option<&str>) -> Result<&'static str, String> {
        let pk = identity::unhex32(to).ok_or("bad key")?;
        let kind = msg["msg"]["kind"].as_str().unwrap_or("");
        let pid = identity::peer_id(&pk);
        let mut tried = Vec::new();
        let near = self.near.lock().unwrap().get(to).map(|n| n.addr);
        if let Some(a) = near {
            match send_tcp(a, &friends::envelope(&pk, msg, None)?) {
                Ok(()) => return Ok("lan"),
                Err(e) => tried.push(format!("same network: {e}")),
            }
        }
        let (direct, mailbox) = {
            let b = self.book.lock().unwrap();
            let direct = b.friends.iter().find(|f| f.presence.as_ref().and_then(|p| p["msg"]["from"].as_str()) == Some(to))
                .and_then(|f| f.presence.as_ref()).and_then(|p| p["msg"]["body"]["addr"].as_str()).and_then(|a| a.parse::<SocketAddr>().ok());
            (direct, b.mailbox_of(&self.me, &pid))
        };
        if let Some(a) = direct.filter(|a| Some(*a) != near) {
            match send_tcp(a, &friends::envelope(&pk, msg, None)?) {
                Ok(()) => return Ok("direct"),
                Err(e) => tried.push(format!("direct: {e}")),
            }
        }
        if let Some(r) = mailbox.or(relay.map(String::from)).filter(|r| crate::realms::relay_ok(r)) {
            let tag = matches!(kind, "presence" | "card").then(|| friends::tag_for(&self.me.public_hex(), to, kind));
            match mailbox_post(&r, &friends::envelope(&pk, msg, tag.as_deref())?) {
                Ok(()) => return Ok("relay"),
                Err(e) => tried.push(format!("relay mailbox: {e}")),
            }
        }
        if tried.is_empty() {
            return Err("They aren't on your network right now and don't read a relay mailbox. It goes through when you're both on the same network.".into());
        }
        Err(format!("Not delivered ({}).", tried.join("; ")))
    }

    /// Presence for every friend (and my card, so names stay current).
    pub fn presence_all(&self, online: bool) {
        let (share, keys, card) = {
            let b = self.book.lock().unwrap();
            (b.profile.share_playing, b.keys(&self.me), friends::make_card(&self.me, &b.profile, now()))
        };
        let (playing, open) = self.playing.lock().unwrap().clone();
        let mut body = json!({"online": online, "card": card});
        if share && online {
            if let Some(p) = playing {
                body["playing"] = json!(p);
            }
            if let Some(o) = open {
                body["open"] = o;
            }
        }
        if let Some(a) = self.direct.lock().unwrap().clone().filter(|_| online) {
            body["addr"] = json!(a);
        }
        for k in keys {
            if self.stop.load(Ordering::Relaxed) && online {
                break;
            }
            if let Ok(m) = friends::make_msg(&self.me, &k, "presence", body.clone(), now()) {
                let _ = self.deliver(&k, &m, None);
            }
        }
    }

    /// Fetch and apply this PC's mailbox at its relay.
    pub fn mailbox_poll(&self) -> Result<usize, String> {
        let (on, relay) = {
            let b = self.book.lock().unwrap();
            (b.profile.mailbox, b.profile.relay.clone())
        };
        if !on || !crate::realms::relay_ok(&relay) {
            return Ok(0);
        }
        let boxes = mailbox_fetch(&self.me, &relay, now())?;
        let mut n = 0;
        for e in boxes.iter().take(200) {
            if self.take(e, Via::Mailbox, now()).is_ok() {
                n += 1;
            }
        }
        Ok(n)
    }

    /// Requests still waiting go out again (start-up: the other side may be reachable now).
    fn resend_requests(&self) {
        let out: Vec<(String, Option<String>)> = self.book.lock().unwrap().outgoing.iter().map(|o| (o.to.clone(), o.relay.clone())).collect();
        for (to, relay) in out {
            let m = self.book.lock().unwrap().request_again(&self.me, &to, now());
            if let Some(m) = m {
                let _ = self.deliver(&to, &m, relay.as_deref());
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ wire

/// One envelope to a listener: a JSON line out, a JSON line back.
pub fn send_tcp(addr: SocketAddr, env: &Value) -> Result<(), String> {
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(IO)).ok();
    s.set_write_timeout(Some(IO)).ok();
    s.write_all(format!("{env}\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(s.take(1024)).read_line(&mut line).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(line.trim()).map_err(|_| "no answer".to_string())?;
    if v["ok"] == true {
        Ok(())
    } else {
        Err(v["error"].as_str().unwrap_or("refused").to_string())
    }
}

/// One connection to the listener: read one envelope line (at most ENVELOPE_MAX), apply it, answer.
fn serve_conn(hub: &Hub, mut s: TcpStream, peer: SocketAddr) {
    s.set_read_timeout(Some(IO)).ok();
    s.set_write_timeout(Some(IO)).ok();
    let t = now();
    let answer = if !hub.limit.lock().unwrap().allow(&format!("t{}", peer.ip()), t) {
        json!({"ok": false, "error": "slow down"})
    } else {
        let mut line = Vec::new();
        let r = s.try_clone().map(|c| BufReader::new(c.take(friends::ENVELOPE_MAX as u64 + 2)).read_until(b'\n', &mut line));
        match r {
            Ok(Ok(_)) if line.ends_with(b"\n") => match serde_json::from_slice::<Value>(&line) {
                Ok(env) => {
                    let via = if local_ip(&peer.ip()) { Via::Lan } else { Via::Direct };
                    match hub.take(&env, via, t) {
                        Ok(_) => json!({"ok": true}),
                        // never say more than "no" to someone we can't identify
                        Err(_) => json!({"ok": false, "error": "refused"}),
                    }
                }
                Err(_) => json!({"ok": false, "error": "not JSON"}),
            },
            _ => json!({"ok": false, "error": "too big or too slow"}),
        }
    };
    let _ = s.write_all(format!("{answer}\n").as_bytes());
}

fn mailbox_post(relay: &str, env: &Value) -> Result<(), String> {
    let to = env["to"].as_str().unwrap_or("");
    let v = crate::net::post_json(&format!("https://{relay}/api/mailbox/{to}"), env, Duration::from_secs(8))?;
    if v["ok"] == true {
        Ok(())
    } else {
        Err(v["error"].as_str().unwrap_or("refused").to_string())
    }
}

/// The signed proof that this PC holds the mailbox's key: (query string).
pub fn fetch_query(me: &PeerIdentity, relay: &str, ts: u64) -> String {
    let doc = json!({"v": 1, "peer_id": me.peer_id, "relay": relay, "ts": ts});
    format!("pub={}&ts={ts}&relay={relay}&sig={}", me.public_hex(), me.sign_doc(CTX_FETCH, &doc))
}

fn mailbox_fetch(me: &PeerIdentity, relay: &str, now: u64) -> Result<Vec<Value>, String> {
    let url = format!("https://{relay}/api/mailbox/{}?{}", me.peer_id, fetch_query(me, relay, now));
    let v = crate::net::get_json(&url, Duration::from_secs(8))?;
    Ok(v["boxes"].as_array().cloned().unwrap_or_default())
}

// ------------------------------------------------------------------------------------------------ the service

static HUB: Mutex<Option<Arc<Hub>>> = Mutex::new(None);

/// The hub if one was made (never makes one: for closing down and for other parts that only tell it things).
pub fn current() -> Option<Arc<Hub>> {
    HUB.lock().ok().and_then(|h| h.clone())
}

/// The running hub, or a new one (not started) for this PC's identity.
pub fn hub(app: &Path) -> Result<Arc<Hub>, String> {
    let mut h = HUB.lock().unwrap();
    if let Some(x) = h.as_ref() {
        return Ok(x.clone());
    }
    let x = Hub::new(app, identity::load_or_create(app)?);
    *h = Some(x.clone());
    Ok(x)
}

/// Start listening and beaconing (once). Ports in use: any free one for TCP (the beacon says which), and beacons
/// still go out without hearing others.
pub fn start(hub: &Arc<Hub>) -> NetState {
    if hub.state.lock().unwrap().running {
        return hub.state.lock().unwrap().clone();
    }
    let gen = hub.gen.fetch_add(1, Ordering::SeqCst) + 1;
    hub.stop.store(false, Ordering::Relaxed);
    let tcp = TcpListener::bind(("0.0.0.0", TCP_PORT)).or_else(|_| TcpListener::bind(("0.0.0.0", 0)));
    let udp = UdpSocket::bind(("0.0.0.0", LAN_PORT)).ok();
    let sender = UdpSocket::bind(("0.0.0.0", 0)).ok();
    {
        let mut s = hub.state.lock().unwrap();
        s.running = true;
        s.lan_port = udp.as_ref().map(|_| LAN_PORT);
        s.tcp_port = tcp.as_ref().ok().and_then(|l| l.local_addr().ok()).map(|a| a.port());
        s.last_error = match &tcp {
            Err(e) => format!("Friends can't reach this PC on your network ({e})."),
            Ok(_) => String::new(),
        };
    }
    if let Ok(l) = tcp {
        let h = hub.clone();
        std::thread::spawn(move || {
            l.set_nonblocking(true).ok();
            while h.live(gen) {
                match l.accept() {
                    Ok((s, peer)) => {
                        s.set_nonblocking(false).ok();
                        if h.conns.load(Ordering::Relaxed) >= CONNS_MAX {
                            continue;
                        }
                        h.conns.fetch_add(1, Ordering::Relaxed);
                        let h2 = h.clone();
                        std::thread::spawn(move || {
                            serve_conn(&h2, s, peer);
                            h2.conns.fetch_sub(1, Ordering::Relaxed);
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(150)),
                }
            }
        });
    }
    if let Some(u) = udp {
        let h = hub.clone();
        std::thread::spawn(move || {
            u.set_read_timeout(Some(Duration::from_secs(1))).ok();
            let mut buf = [0u8; 1024];
            while h.live(gen) {
                if let Ok((n, from)) = u.recv_from(&mut buf) {
                    if h.book.lock().unwrap().profile.lan != "off" {
                        h.hear(&buf[..n], from, now());
                    }
                }
            }
        });
    }
    let h = hub.clone();
    std::thread::spawn(move || {
        if let Some(s) = &sender {
            s.set_broadcast(true).ok();
        }
        h.resend_requests();
        let mut tick: u64 = 0;
        while h.live(gen) {
            let lan = h.book.lock().unwrap().profile.lan.clone();
            let port = h.state.lock().unwrap().tcp_port;
            if tick % BEACON_EVERY == 0 && lan != "off" {
                if let (Some(s), Some(p)) = (&sender, port) {
                    let name = (lan == "everyone").then(|| h.book.lock().unwrap().profile.name.clone());
                    let b = friends::make_beacon(&h.me, p, name.as_deref(), now());
                    let _ = s.send_to(b.to_string().as_bytes(), ("255.255.255.255", LAN_PORT));
                }
            }
            if tick % MAILBOX_EVERY == 5 {
                let r = h.mailbox_poll();
                let mut s = h.state.lock().unwrap();
                s.mailbox = Some(match r {
                    Ok(_) => "ok".into(),
                    Err(e) => format!("not reached ({e})"),
                });
            }
            if tick % PRESENCE_EVERY == 15 {
                h.presence_all(true);
            }
            std::thread::sleep(Duration::from_secs(1));
            tick += 1;
        }
    });
    hub.state.lock().unwrap().clone()
}

/// Stop: friends see "offline" (sent once), the listener and beacons end.
pub fn stop(hub: &Arc<Hub>) {
    if !hub.state.lock().unwrap().running {
        return;
    }
    hub.presence_all(false);
    hub.stop.store(true, Ordering::Relaxed);
    hub.gen.fetch_add(1, Ordering::SeqCst);
    hub.state.lock().unwrap().running = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::friends::{parse_code, Profile};

    fn hub_at(seed: u8, name: &str) -> (tempfile::TempDir, Arc<Hub>) {
        let d = tempfile::tempdir().unwrap();
        let h = Hub::new(d.path(), PeerIdentity::from_seed([seed; 32]).unwrap());
        h.book.lock().unwrap().profile = Profile { name: name.into(), mailbox: false, ..Default::default() };
        (d, h)
    }

    #[test]
    fn a_quick_stop_and_start_leaves_one_set_of_threads() {
        let (_d, a) = hub_at(1, "Ari");
        let g1 = a.gen.fetch_add(1, Ordering::SeqCst) + 1;
        assert!(a.live(g1));
        a.stop.store(true, Ordering::Relaxed);
        a.gen.fetch_add(1, Ordering::SeqCst);
        assert!(!a.live(g1), "stopped");
        // started again before the old threads woke: they still end, the new ones run
        let g2 = a.gen.fetch_add(1, Ordering::SeqCst) + 1;
        a.stop.store(false, Ordering::Relaxed);
        assert!(!a.live(g1) && a.live(g2));
    }

    #[test]
    fn a_closed_world_is_no_longer_shown_to_friends() {
        let (_d, a) = hub_at(1, "Ari");
        *a.playing.lock().unwrap() = (Some("Single player".into()), Some(json!({"kind": "single", "name": "Single player", "invite_hint": "friends"})));
        assert!(!a.world_closed("vanilla"), "another world: kept");
        assert!(a.playing.lock().unwrap().1.is_some());
        assert!(a.world_closed("single"));
        assert_eq!(*a.playing.lock().unwrap(), (None, None));
    }

    #[test]
    fn limiter_and_nonces() {
        let mut l = Limiter::new(3);
        assert!((0..3).all(|_| l.allow("a", 120)));
        assert!(!l.allow("a", 121));
        assert!(l.allow("b", 121), "per key");
        assert!(l.allow("a", 180), "a new minute");
        let mut s = Seen::default();
        assert!(s.first("x"));
        assert!(!s.first("x"));
        for i in 0..SEEN_MAX + 1 {
            s.first(&i.to_string());
        }
        assert!(s.set.len() <= SEEN_MAX);
    }

    #[test]
    fn local_addresses() {
        for a in ["192.168.1.5", "10.0.0.1", "172.16.3.4", "127.0.0.1", "169.254.1.1", "::1", "fe80::1", "fd00::5"] {
            assert!(local_ip(&a.parse().unwrap()), "{a}");
        }
        for a in ["8.8.8.8", "2001:db8::1", "100.100.1.1"] {
            assert!(!local_ip(&a.parse().unwrap()), "{a}");
        }
    }

    #[test]
    fn beacons_heard_only_from_the_local_network_and_capped() {
        let (_d, a) = hub_at(1, "Ari");
        let b = PeerIdentity::from_seed([2; 32]).unwrap();
        let t = now();
        let beacon = friends::make_beacon(&b, 5555, Some("Bo"), t).to_string();
        assert!(!a.hear(beacon.as_bytes(), "8.8.8.8:41190".parse().unwrap(), t), "not from the internet");
        assert!(a.hear(beacon.as_bytes(), "192.168.1.9:41190".parse().unwrap(), t));
        assert_eq!(a.near.lock().unwrap()[&b.public_hex()].addr, "192.168.1.9:5555".parse().unwrap());
        // my own beacon isn't someone near
        assert!(!a.hear(friends::make_beacon(&a.me, 1, None, t).to_string().as_bytes(), "192.168.1.2:1".parse().unwrap(), t));
        // a flood from one address stops at the limit
        let ok = (0..PER_MIN + 10).filter(|_| a.hear(beacon.as_bytes(), "192.168.1.10:1".parse().unwrap(), t)).count();
        assert_eq!(ok as u32, PER_MIN);
        // many keys: at most NEAR_MAX remembered
        for s in 10..(10 + NEAR_MAX as u8 + 8) {
            let k = PeerIdentity::from_seed([s; 32]).unwrap();
            a.hear(friends::make_beacon(&k, 1, None, t).to_string().as_bytes(), format!("10.0.0.{s}:1").parse().unwrap(), t);
        }
        assert!(a.near.lock().unwrap().len() <= NEAR_MAX);
    }

    /// Two launchers on one PC, nothing but loopback TCP: add by code, accept, presence, invite, revoke.
    #[test]
    fn local_to_local_over_tcp() {
        let (_da, a) = hub_at(1, "Ari");
        let (_db, b) = hub_at(2, "Bo");
        let listen = |h: &Arc<Hub>| {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = l.local_addr().unwrap().port();
            let h = h.clone();
            std::thread::spawn(move || {
                for c in l.incoming().flatten() {
                    let p = c.peer_addr().unwrap();
                    serve_conn(&h, c, p);
                }
            });
            port
        };
        let (pa, pb) = (listen(&a), listen(&b));
        // each heard the other's beacon on the local network
        let t = now();
        a.hear(friends::make_beacon(&b.me, pb, Some("Bo"), t).to_string().as_bytes(), "127.0.0.1:41190".parse().unwrap(), t);
        b.hear(friends::make_beacon(&a.me, pa, Some("Ari"), t).to_string().as_bytes(), "127.0.0.1:41190".parse().unwrap(), t);
        // people near: names shown
        let near = a.near_list(t);
        assert_eq!(a.book.lock().unwrap().view(&a.me, t, &near, ONLINE_WINDOW)["near"][0]["name"], "Bo");
        // Ari adds Bo by Bo's code
        let code = b.book.lock().unwrap().new_code(&b.me, t).unwrap();
        let c = parse_code(&code).unwrap();
        let req = a.book.lock().unwrap().request(&a.me, &c.pubkey, Some(&c.token), None, t).unwrap();
        assert_eq!(a.deliver(&b.me.public_hex(), &req, None).unwrap(), "lan");
        assert_eq!(b.book.lock().unwrap().incoming.len(), 1);
        assert_eq!(b.state.lock().unwrap().notices[0], "Ari wants to be friends.");
        // the same message again: dropped (nonce), still one request
        assert!(a.deliver(&b.me.public_hex(), &req, None).is_ok());
        assert_eq!(b.book.lock().unwrap().incoming.len(), 1);
        // Bo accepts
        let (to, acc) = b.book.lock().unwrap().accept(&b.me, &a.me.peer_id, now()).unwrap();
        assert_eq!(b.deliver(&to, &acc, None).unwrap(), "lan");
        assert!(a.book.lock().unwrap().is_friend(&a.me, &b.me.peer_id));
        assert!(b.book.lock().unwrap().is_friend(&b.me, &a.me.peer_id));
        // presence with what Bo plays
        *b.playing.lock().unwrap() = (Some("Single player".into()), Some(json!({"kind": "single", "name": "Bo's world", "invite_hint": "friends"})));
        b.presence_all(true);
        let v = a.book.lock().unwrap().view(&a.me, now(), &[], ONLINE_WINDOW);
        assert_eq!((v["friends"][0]["online"].clone(), v["friends"][0]["playing"].clone()), (json!(true), json!("Single player")));
        // an invite, sealed: only Ari can read the link
        let link = "https://relay.leylinelauncher.com/join/abcdefghijk2345a#SECRET~~G1.pw";
        let m = friends::make_msg(&b.me, &a.me.public_hex(), "invite", json!({"world": {"kind": "single", "name": "Bo's world"}, "link": link}), now()).unwrap();
        b.deliver(&a.me.public_hex(), &m, None).unwrap();
        assert!(a.book.lock().unwrap().friends[0].invite.is_some());
        // saved on both sides: a fresh hub reads the same friend back
        let again = Hub::new(&a.app, PeerIdentity::from_seed([1; 32]).unwrap());
        assert!(again.book.lock().unwrap().is_friend(&again.me, &b.me.peer_id));
        // a stranger's junk and a forged envelope are refused
        let junk = json!({"v": 1, "to": a.me.peer_id, "box": "aaaa"});
        assert!(send_tcp(format!("127.0.0.1:{pa}").parse().unwrap(), &junk).is_err());
        // revoke
        let (to, rv) = a.book.lock().unwrap().revoke(&a.me, &b.me.peer_id, now()).unwrap();
        a.deliver(&to, &rv, None).unwrap();
        assert!(!b.book.lock().unwrap().is_friend(&b.me, &a.me.peer_id));
    }

    #[test]
    fn nowhere_to_send_says_so() {
        let (_d, a) = hub_at(1, "Ari");
        let b = PeerIdentity::from_seed([2; 32]).unwrap();
        let m = friends::make_msg(&a.me, &b.public_hex(), "presence", json!({"online": true}), now()).unwrap();
        assert!(a.deliver(&b.public_hex(), &m, None).unwrap_err().contains("same network"));
    }

    #[test]
    fn mailbox_proof_shape() {
        let me = PeerIdentity::from_seed([4; 32]).unwrap();
        let q = fetch_query(&me, "relay.leylinelauncher.com", 1000);
        assert!(q.starts_with(&format!("pub={}&ts=1000&relay=relay.leylinelauncher.com&sig=", me.public_hex())));
        let sig = q.rsplit("sig=").next().unwrap();
        let doc = json!({"v": 1, "peer_id": me.peer_id, "relay": "relay.leylinelauncher.com", "ts": 1000});
        assert!(identity::verify_doc(&me.public_hex(), &doc, CTX_FETCH, sig));
        assert!(!q.contains(&identity::hex(&[4u8; 32])), "never the secret");
    }
}
