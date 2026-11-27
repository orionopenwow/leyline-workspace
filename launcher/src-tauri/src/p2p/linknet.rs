//! The network side of launcher-to-launcher connections by key (rules in `p2p/link.rs`): QUIC over iroh (MIT OR
//! Apache-2.0), with no public relays or address lookups of iroh's maker. Hole-punch helpers come only from signed
//! relay cards or Settings › Network; address hints come from friends' sealed presence messages, the LAN beacon and
//! the DHT lookup (`link::Lookup`).
//!
//! One endpoint per launcher, keyed by this PC's Passport device key (`passport::DeviceKey`, never the root key). Every
//! stream starts with both sides' Passport device records (`passport::check_device_cert`: the root's signature over the device key),
//! checked against the QUIC key, so each side knows the other's peer key; then a [`Header`]. Services:
//! * `friend`: one sealed friend message (friendnet.rs takes it as from the same network or direct);
//! * `world:<realm id>:login|world`: a shared world's port on 127.0.0.1, behind the invite's knock;
//! * a go-between's `relay` (friends of this PC, or anyone when Be a node carries for the community), capped;
//! * `hold`: a sealed message for an offline friend, handed over when they're reachable.
//!
//! Logs never carry keys or addresses above debug level.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use iroh::endpoint::{presets, Connection, PortmapperConfig, RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, PublicKey, RelayMode, RelayUrl, SecretKey};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use super::link::{self, Budget, Carry, Handshake, Header, Held, Hint, Hints, LinkSettings, Lookup, Sealed, Step, Way, ALPN};
use crate::identity::{self, PeerIdentity};

/// How long one way may take before the next is tried.
pub const STEP_TIMEOUT: Duration = Duration::from_secs(6);
/// Presence and other small messages: the whole try, every way together.
pub const QUICK_TOTAL: Duration = Duration::from_secs(8);

// ------------------------------------------------------------------------------------------------ the runtime

/// The launcher's own small runtime for connections (two worker threads).
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("leyline-link").enable_all().build().expect("a runtime"))
}

// ------------------------------------------------------------------------------------------------ streams

/// A byte stream both ways.
pub trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}
pub type Stream = Box<dyn Duplex>;

/// A QUIC stream that keeps its connection open while it lives.
struct Quic {
    recv: RecvStream,
    send: SendStream,
    _conn: Connection,
}

impl AsyncRead for Quic {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        AsyncRead::poll_read(Pin::new(&mut self.recv), cx, buf)
    }
}

impl AsyncWrite for Quic {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        AsyncWrite::poll_write(Pin::new(&mut self.send), cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.send), cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        AsyncWrite::poll_shutdown(Pin::new(&mut self.send), cx)
    }
}

async fn read_frame<R: AsyncRead + Unpin + ?Sized>(r: &mut R, max: usize) -> Result<Vec<u8>, String> {
    let mut l = [0u8; 2];
    r.read_exact(&mut l).await.map_err(|_| "the other side closed".to_string())?;
    let n = u16::from_be_bytes(l) as usize;
    if n > max {
        return Err("too big".into());
    }
    let mut b = vec![0u8; n];
    r.read_exact(&mut b).await.map_err(|_| "the other side closed".to_string())?;
    Ok(b)
}

async fn read_json<R: AsyncRead + Unpin + ?Sized>(r: &mut R) -> Result<Value, String> {
    serde_json::from_slice(&read_frame(r, link::HEADER_MAX).await?).map_err(|_| "not understood".to_string())
}

async fn write_json<W: AsyncWrite + Unpin + ?Sized, T: serde::Serialize>(w: &mut W, v: &T) -> Result<(), String> {
    w.write_all(&link::frame_header(v)?).await.map_err(|e| e.to_string())?;
    w.flush().await.map_err(|e| e.to_string())
}

/// The sealed stream on top of `inner`: plain bytes on the returned end, sealed frames on `inner`.
fn sealed_stream(inner: Stream, s: Sealed) -> Stream {
    let (mine, theirs) = tokio::io::duplex(64 * 1024);
    let (mut ir, mut iw) = tokio::io::split(inner);
    let (mut pr, mut pw) = tokio::io::split(theirs);
    let Sealed { mut tx, mut rx } = s;
    tokio::spawn(async move {
        let mut b = vec![0u8; link::FRAME_MAX];
        loop {
            match pr.read(&mut b).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let Ok(f) = tx.seal(&b[..n]) else { break };
                    if iw.write_all(&f).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = iw.shutdown().await;
    });
    tokio::spawn(async move {
        loop {
            let Ok(f) = read_frame(&mut ir, link::FRAME_MAX + 16).await else { break };
            let Ok(p) = rx.open(&f) else { break };
            if pw.write_all(&p).await.is_err() {
                break;
            }
        }
        let _ = pw.shutdown().await;
    });
    Box::new(mine)
}

async fn seal_as_dialer(me: &PeerIdentity, to: &[u8; 32], mut s: Stream) -> Result<Stream, String> {
    let (hs, m1) = Handshake::start()?;
    s.write_all(&m1).await.map_err(|e| e.to_string())?;
    let mut m2 = [0u8; 96];
    s.read_exact(&mut m2).await.map_err(|_| "sealing failed".to_string())?;
    let (m3, keys) = hs.finish(me, to, &m2)?;
    s.write_all(&m3).await.map_err(|e| e.to_string())?;
    Ok(sealed_stream(s, keys))
}

async fn seal_as_answerer(me: &PeerIdentity, from: &[u8; 32], mut s: Stream) -> Result<Stream, String> {
    let mut m1 = [0u8; 32];
    s.read_exact(&mut m1).await.map_err(|_| "sealing failed".to_string())?;
    let (m2, pending) = Handshake::answer(me, from, &m1)?;
    s.write_all(&m2).await.map_err(|e| e.to_string())?;
    let mut m3 = [0u8; 64];
    s.read_exact(&mut m3).await.map_err(|_| "sealing failed".to_string())?;
    Ok(sealed_stream(s, pending.confirm(&m3)?))
}

// ------------------------------------------------------------------------------------------------ the friends book

/// What the connection side needs from the friends list (friendnet.rs implements it; tests use a table).
pub trait Book: Send + Sync {
    fn is_friend(&self, pk: &[u8; 32]) -> bool;
    /// Friends seen online lately (go-betweens and held messages).
    fn online(&self) -> Vec<[u8; 32]>;
    /// One sealed friend message in, from the launcher with key `from`.
    fn take(&self, env: &Value, from: &[u8; 32]) -> Result<(), String>;
}

// ------------------------------------------------------------------------------------------------ the endpoint

/// How to make an endpoint.
#[derive(Debug, Clone)]
pub struct Options {
    /// Where to listen (None: [`link::LINK_PORT`] on every address, or any free port).
    pub bind: Option<SocketAddr>,
    pub step_timeout: Duration,
    /// Look up and publish on the Mainline DHT (iroh-mainline-address-lookup: only the hole-punch helper is
    /// published, never addresses). Off in tests (no internet).
    pub dht: bool,
    /// This PC's device key and record (None: the Passport's, from the app folder; a fresh one with no folder).
    pub device: Option<crate::passport::DeviceKey>,
}

impl Default for Options {
    fn default() -> Self {
        Options { bind: None, step_timeout: STEP_TIMEOUT, dht: true, device: None }
    }
}

/// A world port this launcher offers to friends holding its invite.
#[derive(Clone)]
struct Offer {
    port: u16,
    knock: String,
}

pub struct Link {
    pub ep: Endpoint,
    me: PeerIdentity,
    app: Option<PathBuf>,
    pub cfg: Mutex<LinkSettings>,
    book: Arc<dyn Book>,
    lookups: Mutex<Vec<Arc<dyn Lookup>>>,
    /// Community relays (keys from verified relay cards) and their helpers.
    community: Mutex<Vec<[u8; 32]>>,
    card_helpers: Mutex<Vec<String>>,
    /// Be a node is running with its relay switch on (p2p/node.rs).
    pub node_relay: AtomicBool,
    pub friend_budget: Mutex<Budget>,
    pub community_budget: Mutex<Budget>,
    pub held: Mutex<Held>,
    offers: Mutex<HashMap<String, Offer>>,
    step_timeout: Duration,
    /// The way the last connection to each friend took (Settings › Network shows it).
    pub last_way: Mutex<HashMap<String, Way>>,
    /// Peer key -> device key, learned from checked device certs (and invite links, checked at connect time).
    devices: Mutex<HashMap<[u8; 32], [u8; 32]>>,
    my_cert: Value,
    device: crate::passport::DeviceKey,
    dht: bool,
}

fn device_of(c: &Connection) -> [u8; 32] {
    *c.remote_id().as_bytes()
}

/// The other side's Passport device record, checked against the QUIC key: its peer key (the Passport root).
fn check_hello(me: &crate::passport::DeviceKey, v: &Value, c: &Connection) -> Result<[u8; 32], String> {
    me.check_peer(&v["cert"], &device_of(c)).ok_or_else(|| "the other side isn't who it says".into())
}

fn now() -> u64 {
    super::friendnet::now()
}

impl Link {
    /// A new endpoint for `me`. Needs the runtime (call inside it).
    pub async fn bind(me: PeerIdentity, app: Option<PathBuf>, cfg: LinkSettings, book: Arc<dyn Book>, opts: Options) -> Result<Arc<Link>, String> {
        let cfg = link::clean(cfg);
        let device = match (opts.device.clone(), app.as_deref()) {
            (Some(d), _) => d,
            (None, Some(a)) => crate::passport::DeviceKey::for_app(a, &me)?,
            (None, None) => crate::passport::DeviceKey::fresh(&me)?,
        };
        if device.public == me.public_key {
            return Err("The connection key must be a device key, never the Passport itself.".into());
        }
        let helpers: Vec<RelayUrl> = cfg.helpers.iter().filter_map(|h| h.parse().ok()).collect();
        let base = || {
            let b = Endpoint::builder(presets::Minimal)
                .secret_key(SecretKey::from_bytes(&device.secret()))
                .alpns(vec![ALPN.to_vec()])
                .relay_mode(if helpers.is_empty() { RelayMode::Disabled } else { RelayMode::custom(helpers.clone()) })
                .portmapper_config(if cfg.router_port { PortmapperConfig::default() } else { PortmapperConfig::Disabled });
            if opts.dht {
                b.address_lookup(iroh_mainline_address_lookup::DhtAddressLookup::builder())
            } else {
                b
            }
        };
        let ep = match opts.bind {
            Some(a) => base().clear_ip_transports().bind_addr(a).map_err(|e| e.to_string())?.bind().await.map_err(|e| e.to_string())?,
            None => {
                let fixed = base()
                    .bind_addr(SocketAddr::from(([0, 0, 0, 0], link::LINK_PORT)))
                    .and_then(|b| b.bind_addr(SocketAddr::from(([0u16; 8], link::LINK_PORT))))
                    .map_err(|e| e.to_string())?;
                match fixed.bind().await {
                    Ok(ep) => ep,
                    Err(_) => base().bind().await.map_err(|e| format!("Couldn't open the friends' connection port: {e}"))?,
                }
            }
        };
        debug_assert_eq!(ep.id().as_bytes(), &device.public, "the endpoint id is the device key");
        let my_cert = device.record.clone();
        let help = cfg.help_day_mb as u64 * 1024 * 1024;
        let held = app.as_deref().map(Held::load).unwrap_or_default();
        let l = Arc::new(Link {
            ep,
            friend_budget: Mutex::new(Budget::new(help, cfg.help_streams)),
            community_budget: Mutex::new(Budget::new(0, 0)),
            cfg: Mutex::new(cfg),
            me,
            app,
            book,
            lookups: Mutex::new(Vec::new()),
            community: Mutex::new(Vec::new()),
            card_helpers: Mutex::new(Vec::new()),
            node_relay: AtomicBool::new(false),
            held: Mutex::new(held),
            offers: Mutex::new(HashMap::new()),
            step_timeout: opts.step_timeout,
            last_way: Mutex::new(HashMap::new()),
            devices: Mutex::new(HashMap::new()),
            my_cert,
            device,
            dht: opts.dht,
        });
        let a = l.clone();
        tokio::spawn(async move { a.accept_loop().await });
        Ok(l)
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.me.public_key
    }

    /// The Passport pointer on the DHT (p2p/dht.rs `publish_passport`, under the root): this device while the player
    /// keeps "findable by my Passport key" on, withdrawn otherwise. Only with an app folder (not in tests).
    pub fn publish_pointer(&self) {
        let Some(app) = &self.app else { return };
        let cfg = self.cfg.lock().unwrap().clone();
        let eps = link::passport_pointer(&cfg, &identity::hex(&self.device.public));
        let root = super::dht::Key::from_identity(Arc::new(self.me.duplicate()));
        let _ = super::dht::publish_passport(app, root, eps, Vec::new()); // refused only for a bad key: nothing to tell
    }

    pub fn device_key(&self) -> [u8; 32] {
        self.device.public
    }

    /// `peer`'s device key, as an invite link or a hint said (checked when connecting).
    pub fn learn_device(&self, peer: [u8; 32], device: [u8; 32]) {
        let mut d = self.devices.lock().unwrap();
        if d.len() > 4096 {
            d.clear();
        }
        d.insert(peer, device);
    }

    fn hello(&self) -> Value {
        serde_json::json!({"cert": self.my_cert})
    }

    pub fn add_lookup(&self, l: Arc<dyn Lookup>) {
        self.lookups.lock().unwrap().push(l);
    }

    /// Community relays from verified relay cards (`link::check_relay_card`).
    pub fn set_community(&self, keys: Vec<[u8; 32]>, helpers: Vec<String>) {
        *self.community.lock().unwrap() = keys;
        *self.card_helpers.lock().unwrap() = helpers.into_iter().filter(|h| link::helper_ok(h)).take(8).collect();
    }

    /// Be a node's relay: on with its caps (daily MB, streams at once), or off.
    pub fn set_node_relay(&self, on: bool, day_bytes: u64, streams: u32) {
        let mut b = self.community_budget.lock().unwrap();
        b.cap = day_bytes;
        b.max_live = streams;
        self.node_relay.store(on, Ordering::Relaxed);
    }

    pub fn set_cfg(&self, s: LinkSettings) {
        let s = link::clean(s);
        {
            let mut b = self.friend_budget.lock().unwrap();
            b.cap = s.help_day_mb as u64 * 1024 * 1024;
            b.max_live = s.help_streams;
        }
        *self.cfg.lock().unwrap() = s;
    }

    /// Where friends can reach this launcher: its addresses (local, IPv6, port-mapped) and helpers.
    pub fn my_hints(&self) -> (Vec<SocketAddr>, Vec<String>) {
        let a = self.ep.addr();
        let ips: Vec<SocketAddr> = a.ip_addrs().copied().filter(|x| !x.ip().is_loopback() && !x.ip().is_unspecified()).take(8).collect();
        let helpers: Vec<String> = a.relay_urls().map(|u| u.to_string()).take(2).collect();
        (ips, helpers)
    }

    /// Everything known about reaching `peer`.
    pub fn hints_for(&self, peer: &[u8; 32]) -> Hints {
        let mut h = Hints::default();
        let lookups: Vec<Arc<dyn Lookup>> = self.lookups.lock().unwrap().clone();
        for l in lookups {
            // the DHT lookup may block a few seconds: let the runtime move other work off this thread meanwhile
            let multi = tokio::runtime::Handle::try_current().is_ok_and(|r| r.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread);
            h.add(if multi { tokio::task::block_in_place(|| l.lookup(peer)) } else { l.lookup(peer) });
        }
        let mine: Vec<Hint> = self.cfg.lock().unwrap().helpers.iter().chain(self.card_helpers.lock().unwrap().iter()).cloned().map(Hint::Helper).collect();
        h.add(mine);
        if let Some(d) = self.devices.lock().unwrap().get(peer) {
            h.add([Hint::Device(*d)]);
        }
        h.finder = self.dht;
        h.friends = self.book.online().into_iter().filter(|k| k != peer && *k != self.me.public_key).collect();
        h.community = self.community.lock().unwrap().iter().filter(|k| *k != peer && **k != self.me.public_key).copied().collect();
        h
    }

    // -------------------------------------------------------------------------------------------- dialing

    async fn connect_addr(&self, addr: EndpointAddr) -> Result<Connection, String> {
        match tokio::time::timeout(self.step_timeout, self.ep.connect(addr, ALPN)).await {
            Ok(Ok(c)) => Ok(c),
            Ok(Err(_)) => Err("no answer".into()),
            Err(_) => Err("no answer in time".into()),
        }
    }

    /// One QUIC connection to `peer` by the way `way` (ways 1 to 3 only).
    async fn connect_way(&self, peer: &[u8; 32], way: Way, h: &Hints) -> Result<Connection, String> {
        let dev = *h.devices.first().ok_or("their device key isn't known")?;
        let _ = peer;
        let id = PublicKey::from_bytes(&dev).map_err(|_| "bad key".to_string())?;
        let mut a = EndpointAddr::new(id);
        match way {
            Way::SameNetwork => h.lan.iter().for_each(|x| a = a.clone().with_ip_addr(*x)),
            Way::Direct => h.direct.iter().for_each(|x| a = a.clone().with_ip_addr(*x)),
            Way::HolePunch => {
                h.lan.iter().chain(h.direct.iter()).for_each(|x| a = a.clone().with_ip_addr(*x));
                h.helpers.iter().filter_map(|u| u.parse::<RelayUrl>().ok()).for_each(|u| a = a.clone().with_relay_url(u));
            }
            Way::Friend | Way::Community => return Err("not a direct way".into()),
        }
        if a.is_empty() && !(way == Way::Direct && self.dht) {
            return Err("nothing to try".into());
        }
        self.connect_addr(a).await
    }

    /// A go-between itself, by the first of ways 1 to 3 that works.
    async fn connect_any(&self, peer: &[u8; 32]) -> Result<(Connection, Way), String> {
        let h = self.hints_for(peer);
        let mut last = String::from("not reachable");
        for way in [Way::SameNetwork, Way::Direct, Way::HolePunch] {
            let usable = !h.devices.is_empty()
                && match way {
                    Way::SameNetwork => !h.lan.is_empty(),
                    Way::Direct => !h.direct.is_empty() || h.finder,
                    _ => !h.helpers.is_empty(),
                };
            if usable {
                match self.connect_way(peer, way, &h).await {
                    Ok(c) => return Ok((c, way)),
                    Err(e) => last = e,
                }
            }
        }
        Err(last)
    }

    /// A stream on `c` to the launcher with peer key `root`: certs both ways (theirs must be `root`'s), then the
    /// request, then its answer.
    async fn open(&self, c: Connection, root: &[u8; 32], header: &Header) -> Result<Stream, String> {
        let (send, recv) = c.open_bi().await.map_err(|e| e.to_string())?;
        let c2 = c.clone();
        let mut s: Stream = Box::new(Quic { recv, send, _conn: c });
        write_json(&mut s, &self.hello()).await?;
        if check_hello(&self.device, &read_json(&mut s).await?, &c2)? != *root {
            return Err("the other side isn't who it should be".into());
        }
        self.learn_device(*root, device_of(&c2));
        write_json(&mut s, header).await?;
        link::answer_result(&read_json(&mut s).await?)?;
        Ok(s)
    }

    async fn try_step(&self, peer: &[u8; 32], step: &Step, h: &Hints, svc: &str, knock: &str) -> Result<Stream, String> {
        let header = Header::Svc { svc: svc.into(), knock: knock.into() };
        match (step.way, step.via) {
            (Way::Friend | Way::Community, Some(via)) => {
                let (c, _) = self.connect_any(&via).await?;
                let relay = Header::Relay { to: identity::hex(peer), svc: svc.into(), knock: knock.into() };
                let s = tokio::time::timeout(self.step_timeout, self.open(c, &via, &relay)).await.map_err(|_| "no answer in time".to_string())??;
                tokio::time::timeout(self.step_timeout, seal_as_dialer(&self.me, peer, s)).await.map_err(|_| "no answer in time".to_string())?
            }
            (way, _) => {
                let c = self.connect_way(peer, way, h).await?;
                tokio::time::timeout(self.step_timeout, self.open(c, peer, &header)).await.map_err(|_| "no answer in time".to_string())?
            }
        }
    }

    /// A stream to `peer`'s service `svc`, trying the ways in order. `total` bounds the whole try; `go_betweens`
    /// allows ways 4 and 5. The way used, or what each way said.
    pub async fn dial(&self, peer: &[u8; 32], svc: &str, knock: &str, total: Duration, go_betweens: bool) -> Result<(Stream, Way), String> {
        if !link::svc_ok(svc) {
            return Err("unknown service".into());
        }
        let cfg = self.cfg.lock().unwrap().clone();
        let h = self.hints_for(peer);
        let (steps, skipped) = link::plan(peer, &h, &cfg);
        let start = Instant::now();
        let mut tried: Vec<String> = Vec::new();
        for step in steps.iter().filter(|s| go_betweens || !s.way.go_between()) {
            let left = total.saturating_sub(start.elapsed());
            if left.is_zero() {
                tried.push("out of time".into());
                break;
            }
            match tokio::time::timeout(left, self.try_step(peer, step, &h, svc, knock)).await {
                Ok(Ok(s)) => {
                    self.last_way.lock().unwrap().insert(identity::peer_id(peer), step.way);
                    return Ok((s, step.way));
                }
                Ok(Err(e)) => tried.push(format!("{}: {e}", step.way.words())),
                Err(_) => tried.push(format!("{}: out of time", step.way.words())),
            }
        }
        let mut all = tried;
        all.extend(skipped);
        Err(if all.is_empty() { "No way to reach them right now.".into() } else { format!("Not reached ({}).", all.join("; ")) })
    }

    /// One sealed friend message to `peer` (an envelope from friends.rs).
    pub async fn send_envelope(&self, peer: &[u8; 32], env: &Value, total: Duration, go_betweens: bool) -> Result<Way, String> {
        let (mut s, way) = self.dial(peer, "friend", "", total, go_betweens).await?;
        s.write_all(format!("{env}\n").as_bytes()).await.map_err(|e| e.to_string())?;
        s.flush().await.map_err(|e| e.to_string())?;
        let mut line = Vec::new();
        let mut b = [0u8; 1];
        while line.len() < 1024 {
            match tokio::time::timeout(self.step_timeout, s.read(&mut b)).await {
                Ok(Ok(1)) if b[0] == b'\n' => break,
                Ok(Ok(1)) => line.push(b[0]),
                _ => return Err("no answer".into()),
            }
        }
        let v: Value = serde_json::from_slice(&line).map_err(|_| "no answer".to_string())?;
        link::answer_result(&v).map(|_| way)
    }

    /// Hand a sealed message for `to` (offline) to an online friend who is also `to`'s friend: the first who takes it.
    pub async fn hold(&self, to: &[u8; 32], env: &Value) -> Result<(), String> {
        let mut tried = Vec::new();
        for f in self.book.online().into_iter().filter(|k| k != to && *k != self.me.public_key).take(4) {
            let r = async {
                let (c, _) = self.connect_any(&f).await?;
                let mut s = tokio::time::timeout(self.step_timeout, self.open(c, &f, &Header::Hold { to: identity::hex(to) })).await.map_err(|_| "no answer in time".to_string())??;
                let b = env.to_string().into_bytes();
                if b.len() > super::friends::ENVELOPE_MAX {
                    return Err("too big".to_string());
                }
                s.write_all(&(b.len() as u16).to_be_bytes()).await.map_err(|e| e.to_string())?;
                s.write_all(&b).await.map_err(|e| e.to_string())?;
                s.flush().await.map_err(|e| e.to_string())?;
                link::answer_result(&read_json(&mut s).await?)
            }
            .await;
            match r {
                Ok(()) => return Ok(()),
                Err(e) => tried.push(e),
            }
        }
        Err(if tried.is_empty() { "no friend online to keep it".into() } else { format!("no friend could keep it ({})", tried.join("; ")) })
    }

    /// Messages kept here go to friends who are reachable now. Returns how many went.
    pub async fn deliver_held(&self) -> usize {
        let waiting = self.held.lock().unwrap().waiting();
        let mut sent = 0;
        for to_hex in waiting {
            let Some(to) = identity::unhex32(&to_hex) else { continue };
            let envs = self.held.lock().unwrap().take_for(&to_hex);
            let mut back = Vec::new();
            for e in envs {
                if back.is_empty() && self.send_envelope(&to, &e, QUICK_TOTAL, false).await.is_ok() {
                    sent += 1;
                } else {
                    back.push(e);
                }
            }
            let mut h = self.held.lock().unwrap();
            for e in back {
                h.items.push(link::HeldItem { to: to_hex.clone(), from: String::new(), env: e, at: now() });
            }
        }
        if let Some(app) = &self.app {
            let _ = self.held.lock().unwrap().save(app);
        }
        sent
    }

    // -------------------------------------------------------------------------------------------- serving

    /// Offer a shared world's two ports to friends holding its invite.
    pub fn offer_world(&self, realm_id: &str, secret: &str, login: u16, world: u16) {
        let knock = link::knock(realm_id, secret);
        let mut o = self.offers.lock().unwrap();
        o.insert(format!("world:{realm_id}:login"), Offer { port: login, knock: knock.clone() });
        o.insert(format!("world:{realm_id}:world"), Offer { port: world, knock });
    }

    pub fn withdraw_world(&self, realm_id: &str) {
        self.offers.lock().unwrap().retain(|k, _| !k.starts_with(&format!("world:{realm_id}:")));
    }

    async fn accept_loop(self: Arc<Self>) {
        while let Some(incoming) = self.ep.accept().await {
            let me = self.clone();
            tokio::spawn(async move {
                let Ok(c) = incoming.await else { return };
                if c.alpn() != ALPN {
                    return;
                }
                loop {
                    let Ok((send, recv)) = c.accept_bi().await else { break };
                    let me2 = me.clone();
                    let c2 = c.clone();
                    tokio::spawn(async move {
                        let c3 = c2.clone();
                        let mut s: Stream = Box::new(Quic { recv, send, _conn: c2 });
                        let hello = async {
                            let from = check_hello(&me2.device, &read_json(&mut s).await?, &c3)?;
                            write_json(&mut s, &me2.hello()).await?;
                            Ok::<[u8; 32], String>(from)
                        };
                        let Ok(Ok(from)) = tokio::time::timeout(me2.step_timeout, hello).await else { return };
                        me2.learn_device(from, device_of(&c3));
                        let _ = me2.serve(from, s).await;
                    });
                }
            });
        }
    }

    /// What a service would answer before any bytes pass.
    fn precheck(&self, svc: &str, knock: &str) -> Result<Option<u16>, String> {
        if !link::svc_ok(svc) {
            return Err("unknown service".into());
        }
        if svc == "friend" {
            return Ok(None);
        }
        match self.offers.lock().unwrap().get(svc) {
            Some(o) if link::same(&o.knock, knock) => Ok(Some(o.port)),
            Some(_) => Err("that invite isn't good here".into()),
            None => Err("that world isn't shared right now".into()),
        }
    }

    async fn serve(self: Arc<Self>, from: [u8; 32], mut s: Stream) -> Result<(), String> {
        let h: Header = serde_json::from_value(tokio::time::timeout(self.step_timeout, read_json(&mut s)).await.map_err(|_| "slow".to_string())??).map_err(|_| "not understood".to_string())?;
        match h {
            Header::Svc { svc, knock } => {
                let pre = self.precheck(&svc, &knock);
                write_json(&mut s, &link::answer(&pre.as_ref().map(|_| ()).map_err(Clone::clone))).await?;
                self.run_service(from, pre?, s).await
            }
            Header::Relayed { from: claimed, svc, knock } => {
                let claimed = identity::unhex32(&claimed).ok_or("bad key")?;
                let pre = self.precheck(&svc, &knock);
                write_json(&mut s, &link::answer(&pre.as_ref().map(|_| ()).map_err(Clone::clone))).await?;
                let port = pre?;
                // the seal proves who is really at the other end; the go-between only passes sealed bytes
                let s = tokio::time::timeout(self.step_timeout, seal_as_answerer(&self.me, &claimed, s)).await.map_err(|_| "slow".to_string())??;
                self.run_service(claimed, port, s).await
            }
            Header::Relay { to, svc, knock } => self.carry(from, &to, &svc, &knock, s).await,
            Header::Hold { to } => {
                let to_k = identity::unhex32(&to).ok_or("bad key")?;
                let (ff, tf) = (self.book.is_friend(&from), self.book.is_friend(&to_k));
                let pre = if ff && tf { Ok(()) } else { Err("only between friends of this PC".to_string()) };
                write_json(&mut s, &link::answer(&pre)).await?;
                pre?;
                let r = async {
                    let b = read_frame(&mut s, super::friends::ENVELOPE_MAX).await?;
                    let env: Value = serde_json::from_slice(&b).map_err(|_| "not understood".to_string())?;
                    let mut held = self.held.lock().unwrap();
                    held.put(&identity::hex(&from), &to, env, now(), ff, tf)?;
                    if let Some(app) = &self.app {
                        let _ = held.save(app);
                    }
                    Ok::<(), String>(())
                }
                .await;
                write_json(&mut s, &link::answer(&r)).await
            }
        }
    }

    async fn run_service(&self, from: [u8; 32], port: Option<u16>, mut s: Stream) -> Result<(), String> {
        match port {
            None => {
                // one friend message: a JSON line in, a JSON line back
                let mut line = Vec::new();
                let mut b = [0u8; 1024];
                while !line.ends_with(b"\n") {
                    let n = tokio::time::timeout(self.step_timeout, s.read(&mut b)).await.map_err(|_| "slow".to_string())?.map_err(|e| e.to_string())?;
                    if n == 0 || line.len() + n > super::friends::ENVELOPE_MAX + 2 {
                        return Err("too big or cut off".into());
                    }
                    line.extend_from_slice(&b[..n]);
                }
                let r = serde_json::from_slice::<Value>(&line).map_err(|_| "not JSON".to_string()).and_then(|env| self.book.take(&env, &from).map_err(|_| "refused".to_string()));
                s.write_all(format!("{}\n", link::answer(&r)).as_bytes()).await.map_err(|e| e.to_string())?;
                s.flush().await.map_err(|e| e.to_string())
            }
            Some(port) => {
                let mut up = tokio::net::TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], port))).await.map_err(|e| e.to_string())?;
                let _ = up.set_nodelay(true);
                let _ = tokio::io::copy_bidirectional(&mut s, &mut up).await;
                Ok(())
            }
        }
    }

    /// A go-between's part: check, reach `to`, pass its answer back, then sealed bytes both ways within the caps.
    async fn carry(&self, from: [u8; 32], to_hex: &str, svc: &str, knock: &str, mut a: Stream) -> Result<(), String> {
        let to = identity::unhex32(to_hex).ok_or("bad key")?;
        let cfg = self.cfg.lock().unwrap().clone();
        let book = self.book.clone();
        let kind = link::may_carry(&cfg, self.node_relay.load(Ordering::Relaxed), &self.me.public_key, &from, &to, &|k: &[u8; 32]| book.is_friend(k));
        let kind = match kind {
            Ok(k) => k,
            Err(e) => return write_json(&mut a, &link::answer(&Err(e))).await,
        };
        let budget = match kind {
            Carry::Friends => &self.friend_budget,
            Carry::Community => &self.community_budget,
        };
        let admitted = budget.lock().unwrap().admit(now());
        if let Err(e) = admitted {
            return write_json(&mut a, &link::answer(&Err(e))).await;
        }
        let r = self.carry_admitted(&from, &to, svc, knock, a, budget).await;
        budget.lock().unwrap().done();
        r
    }

    async fn carry_admitted(&self, from: &[u8; 32], to: &[u8; 32], svc: &str, knock: &str, mut a: Stream, budget: &Mutex<Budget>) -> Result<(), String> {
        let b = async {
            let (c, _) = self.connect_any(to).await?;
            let hdr = Header::Relayed { from: identity::hex(from), svc: svc.into(), knock: knock.into() };
            // the answer (ok or why not) goes back to the dialer as it is
            match self.open(c, to, &hdr).await {
                Ok(s) => Ok::<(Stream, Value), String>((s, link::answer(&Ok(())))),
                Err(e) if e.contains("isn't who") => Err(e),
                Err(e) => Err(format!("they said: {e}")),
            }
        };
        let (b, ans) = match tokio::time::timeout(self.step_timeout, b).await {
            Ok(Ok(x)) => x,
            Ok(Err(e)) => return write_json(&mut a, &link::answer(&Err(format!("the go-between couldn't reach them ({e})")))).await,
            Err(_) => return write_json(&mut a, &link::answer(&Err("the go-between couldn't reach them in time".into()))).await,
        };
        write_json(&mut a, &ans).await?;
        if link::answer_result(&ans).is_err() {
            return Ok(());
        }
        splice(a, b, budget).await;
        Ok(())
    }
}

/// Bytes both ways until either side closes or today's cap is reached.
async fn splice(a: Stream, b: Stream, budget: &Mutex<Budget>) {
    let (mut ar, mut aw) = tokio::io::split(a);
    let (mut br, mut bw) = tokio::io::split(b);
    let stop = AtomicBool::new(false);
    tokio::join!(pass(&mut ar, &mut bw, &stop, budget), pass(&mut br, &mut aw, &stop, budget));
}

async fn pass(r: &mut tokio::io::ReadHalf<Stream>, w: &mut tokio::io::WriteHalf<Stream>, stop: &AtomicBool, budget: &Mutex<Budget>) {
    let mut buf = vec![0u8; 32 * 1024];
    while !stop.load(Ordering::Relaxed) {
        let n = match r.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let ok = budget.lock().unwrap().charge(n as u64, now());
        if !ok || w.write_all(&buf[..n]).await.is_err() {
            break;
        }
    }
    let _ = w.shutdown().await;
    stop.store(true, Ordering::Relaxed);
}

/// Run `f` to its end on the connection runtime, from any thread (a fresh thread, so never inside another runtime).
pub fn block<F: std::future::Future + Send>(f: F) -> F::Output
where
    F::Output: Send,
{
    std::thread::scope(|s| s.spawn(|| runtime().block_on(f)).join().expect("the connection runtime"))
}

// ------------------------------------------------------------------------------------------------ the launcher's link

static LINK: Mutex<Option<Arc<Link>>> = Mutex::new(None);

pub fn current() -> Option<Arc<Link>> {
    LINK.lock().ok().and_then(|l| l.clone())
}

/// The friends list as the connection side sees it.
struct HubBook(Arc<super::friendnet::Hub>);

impl Book for HubBook {
    fn is_friend(&self, pk: &[u8; 32]) -> bool {
        self.0.book.lock().unwrap().is_friend(&self.0.me, &identity::peer_id(pk))
    }
    fn online(&self) -> Vec<[u8; 32]> {
        let t = now();
        let b = self.0.book.lock().unwrap();
        b.friends
            .iter()
            .filter(|f| t.saturating_sub(f.last_seen) <= super::friendnet::ONLINE_WINDOW)
            .filter(|f| f.presence.as_ref().is_some_and(|p| p["msg"]["body"]["online"] == true))
            .filter_map(|f| f.presence.as_ref().and_then(|p| p["msg"]["from"].as_str()).and_then(identity::unhex32))
            .collect()
    }
    fn take(&self, env: &Value, from: &[u8; 32]) -> Result<(), String> {
        let rec = super::friends::open_envelope(&self.0.me, env)?;
        // the message must be signed by the launcher that brought it (or came sealed through a go-between from it)
        if rec["msg"]["from"].as_str() != Some(identity::hex(from).as_str()) {
            return Err("refused".into());
        }
        self.0.take(env, super::friends::Via::Direct, now()).map(|_| ())
    }
}

/// Hints from the friends list: the LAN beacon (same network) and the `link` part of friends' sealed presence.
struct HubHints(Arc<super::friendnet::Hub>);

impl Lookup for HubHints {
    fn lookup(&self, peer: &[u8; 32]) -> Vec<Hint> {
        let hex = identity::hex(peer);
        let mut out = Vec::new();
        let b = self.0.book.lock().unwrap();
        let body = b.friends.iter().filter_map(|f| f.presence.as_ref()).find(|p| p["msg"]["from"].as_str() == Some(hex.as_str())).map(|p| p["msg"]["body"].clone());
        drop(b);
        let port = body.as_ref().and_then(|b| b["link"]["port"].as_u64()).filter(|p| (1..=65535).contains(p)).map(|p| p as u16).unwrap_or(link::LINK_PORT);
        if let Some(n) = self.0.near.lock().unwrap().get(&hex) {
            out.push(Hint::Ip(SocketAddr::new(n.addr.ip(), port)));
        }
        if let Some(b) = body {
            if let Some((root, dev)) = crate::passport::check_device_cert(&b["link"]["cert"]) {
                if root == *peer {
                    out.push(Hint::Device(dev));
                }
            }
            out.extend(b["link"]["ips"].as_array().into_iter().flatten().filter_map(Value::as_str).filter_map(|s| s.parse().ok()).take(8).map(Hint::Ip));
            out.extend(b["link"]["helpers"].as_array().into_iter().flatten().filter_map(Value::as_str).take(2).map(|s| Hint::Helper(s.to_string())));
        }
        out
    }
}

/// The `link` part of this launcher's presence (sealed to each friend): where it can be reached.
pub fn presence_part() -> Option<Value> {
    let l = current()?;
    let (ips, helpers) = l.my_hints();
    let port = l.ep.bound_sockets().first().map(|a| a.port()).unwrap_or(link::LINK_PORT);
    let relays: Vec<Value> = relay_card_now().into_iter().collect();
    Some(serde_json::json!({"v": 1, "port": port, "cert": l.my_cert, "ips": ips.iter().map(|a| a.to_string()).collect::<Vec<_>>(), "helpers": helpers, "relays": relays}))
}

/// Be a node's look (node.rs `tick`): the relay for other players runs while this PC chose it (Settings › Network,
/// "Carry sealed connections for other players"), the node switch is on, not paused, and not resting. It needs no
/// site flag: there is no server to set one. Caps are Be a node's own (data per day, connections at once).
pub fn node_tick(s: &super::node::NodeSettings, state: super::node::State, killed: bool) {
    use super::node::State;
    let Some(l) = current() else { return };
    let chosen = l.cfg.lock().unwrap().community_relay;
    let on = chosen && s.on && !s.paused && !killed && !matches!(state, State::RestingBattery | State::RestingInUse | State::DayCap);
    l.set_node_relay(on, super::node::day_bytes(s), s.max_peers);
}

/// This PC's relay card while its relay runs (`link::relay_card`): friends pass it on with presence.
pub fn relay_card_now() -> Option<Value> {
    let l = current()?;
    if !l.node_relay.load(Ordering::Relaxed) {
        return None;
    }
    let b = l.community_budget.lock().unwrap().clone();
    let helper = l.my_hints().1.into_iter().next();
    Some(link::relay_card(&l.me, (b.live as u64, b.max_live as u64), 0, helper.as_deref(), now()))
}

/// Community relays learned from friends' presence (verified relay cards).
fn relays_from_friends(hub: &super::friendnet::Hub) -> (Vec<[u8; 32]>, Vec<String>) {
    let t = now();
    let b = hub.book.lock().unwrap();
    let mut keys = Vec::new();
    let mut helpers = Vec::new();
    for f in &b.friends {
        let Some(p) = &f.presence else { continue };
        for c in p["msg"]["body"]["link"]["relays"].as_array().into_iter().flatten().take(4) {
            if let Some((k, h)) = link::check_relay_card(c, t) {
                if !keys.contains(&k) {
                    keys.push(k);
                }
                helpers.extend(h);
            }
        }
    }
    keys.truncate(16);
    (keys, helpers)
}

/// Start the launcher's endpoint for the friends hub (once; again after a stop). Off in Settings › Network: none.
pub fn start_for(hub: &Arc<super::friendnet::Hub>) -> Result<(), String> {
    if current().is_some() {
        return Ok(());
    }
    let cfg = link::load(&hub.app);
    if !cfg.on {
        return Err("Reaching friends from anywhere is off in Settings › Network.".into());
    }
    let (h1, h2, h3) = (hub.clone(), hub.clone(), hub.clone());
    let me = hub.me.duplicate();
    let app = hub.app.clone();
    let l = block(async move { Link::bind(me, Some(app), cfg, Arc::new(HubBook(h1)), Options::default()).await })?;
    l.add_lookup(Arc::new(HubHints(h2)));
    if let Some(dht) = super::link::dht_lookup() {
        l.add_lookup(dht);
    }
    *LINK.lock().unwrap() = Some(l.clone());
    l.publish_pointer();
    // messages kept for friends go out when they're reachable
    runtime().spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if current().map_or(true, |c| !Arc::ptr_eq(&c, &l)) {
                break;
            }
            let (keys, helpers) = relays_from_friends(&h3);
            l.set_community(keys, helpers);
            l.deliver_held().await;
        }
    });
    Ok(())
}

/// The link, started for this PC's identity when it isn't running (sharing a world needs it even with the Friends
/// list closed).
pub fn ensure() -> Result<Arc<Link>, String> {
    if let Some(l) = current() {
        return Ok(l);
    }
    start_for(&super::friendnet::hub(&crate::settings::app_dir())?)?;
    current().ok_or_else(|| "not started".to_string())
}

pub fn stop() {
    if let Some(l) = LINK.lock().ok().and_then(|mut g| g.take()) {
        block(async move { l.ep.close().await });
    }
}

// ------------------------------------------------------------------------------------------------ joining a world

/// A guest's forwarder: the stock game connects to 127.0.0.1 here; each connection is dialed to the host by key.
struct Guest {
    stop: Arc<AtomicBool>,
    host: [u8; 32],
    login: u16,
    world: u16,
}

static GUEST: Mutex<Option<Guest>> = Mutex::new(None);

async fn listen(prefer: &[u16]) -> Result<tokio::net::TcpListener, String> {
    for p in prefer {
        if let Ok(l) = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], *p))).await {
            return Ok(l);
        }
    }
    Err("no free port on this PC for the game".into())
}

/// Start (or keep) the forwarder for the host `host`'s world `realm_id`: 127.0.0.1 ports for login and world (the
/// game's own when free). The login stream's realm list is rewritten to this PC's world port (AuthRewriter), so the
/// game never leaves 127.0.0.1. Checks the host answers first. ("127.0.0.1:<login>", the way).
pub async fn guest_start(l: &Arc<Link>, host: [u8; 32], realm_id: &str, secret: &str, layout: crate::host::worldshare::Layout) -> Result<(String, Way), String> {
    let knock = link::knock(realm_id, secret);
    let (svc_login, svc_world) = (format!("world:{realm_id}:login"), format!("world:{realm_id}:world"));
    // the host answers? (a stream opened and closed first)
    let way = l.dial(&host, &svc_login, &knock, Duration::from_secs(30), true).await.map(|(_, w)| w)?;
    let running = GUEST.lock().map_err(|_| "busy")?.as_ref().filter(|x| x.host == host && !x.stop.load(Ordering::Relaxed)).map(|x| x.login);
    if let Some(lp) = running {
        return Ok((format!("127.0.0.1:{lp}"), way));
    }
    guest_stop();
    let (ll, wl) = (listen(&[3724, 13724, 23724, 0]).await?, listen(&[8085, 18085, 28085, 0]).await?);
    let lp = ll.local_addr().map_err(|e| e.to_string())?.port();
    let wp = wl.local_addr().map_err(|e| e.to_string())?.port();
    let stop = Arc::new(AtomicBool::new(false));
    for (lst, svc, rewrite) in [(ll, svc_login, true), (wl, svc_world, false)] {
        let (l, stop, knock) = (l.clone(), stop.clone(), knock.clone());
        tokio::spawn(async move {
            while !stop.load(Ordering::Relaxed) {
                let Ok(Ok((mut c, _))) = tokio::time::timeout(Duration::from_millis(500), lst.accept()).await else { continue };
                let (l, svc, knock) = (l.clone(), svc.clone(), knock.clone());
                tokio::spawn(async move {
                    let _ = c.set_nodelay(true);
                    let Ok((up, _)) = l.dial(&host, &svc, &knock, Duration::from_secs(30), true).await else { return };
                    if rewrite {
                        let rw = crate::host::worldshare::AuthRewriter::with_layout(&format!("127.0.0.1:{wp}"), layout);
                        pass_rewritten(c, up, rw).await;
                    } else {
                        let mut up = up;
                        let _ = tokio::io::copy_bidirectional(&mut c, &mut up).await;
                    }
                });
            }
        });
    }
    *GUEST.lock().map_err(|_| "busy")? = Some(Guest { stop, host, login: lp, world: wp });
    Ok((format!("127.0.0.1:{lp}"), way))
}

pub fn guest_stop() {
    if let Some(x) = GUEST.lock().ok().and_then(|mut g| g.take()) {
        x.stop.store(true, Ordering::Relaxed);
    }
}

/// The game's login connection: as is to the host; the host's answers through the realm list rewriter.
async fn pass_rewritten(c: tokio::net::TcpStream, up: Stream, mut rw: crate::host::worldshare::AuthRewriter) {
    let (mut cr, mut cw) = c.into_split();
    let (mut ur, mut uw) = tokio::io::split(up);
    let a = async {
        let _ = tokio::io::copy(&mut cr, &mut uw).await;
        let _ = uw.shutdown().await;
    };
    let b = async {
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            match ur.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if cw.write_all(&rw.feed(&buf[..n])).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = cw.shutdown().await;
    };
    tokio::join!(a, b);
}

// ------------------------------------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A friends list for tests: who is a friend, who is online, and the messages taken.
    #[derive(Default)]
    struct TestBook {
        friends: Vec<[u8; 32]>,
        online: Vec<[u8; 32]>,
        got: Mutex<Vec<(Value, [u8; 32])>>,
    }

    impl Book for TestBook {
        fn is_friend(&self, pk: &[u8; 32]) -> bool {
            self.friends.contains(pk)
        }
        fn online(&self) -> Vec<[u8; 32]> {
            self.online.clone()
        }
        fn take(&self, env: &Value, from: &[u8; 32]) -> Result<(), String> {
            self.got.lock().unwrap().push((env.clone(), *from));
            Ok(())
        }
    }

    fn id(n: u8) -> PeerIdentity {
        PeerIdentity::from_seed([n; 32]).unwrap()
    }

    async fn node(n: u8, book: TestBook, cfg: LinkSettings) -> (Arc<Link>, Arc<TestBook>) {
        let book = Arc::new(book);
        let opts = Options { bind: Some("127.0.0.1:0".parse().unwrap()), step_timeout: Duration::from_secs(5), dht: false, device: None };
        let l = Link::bind(id(n), None, cfg, book.clone(), opts).await.unwrap();
        (l, book)
    }

    fn addr_of(l: &Link) -> SocketAddr {
        *l.ep.bound_sockets().iter().find(|a| a.is_ipv4()).unwrap()
    }

    /// Everyone can find everyone at their 127.0.0.1 address (as the same network).
    fn table(nodes: &[&Arc<Link>]) -> Arc<link::Table> {
        let t = Arc::new(link::Table::default());
        for n in nodes {
            t.put(n.public_key(), vec![Hint::Ip(addr_of(n)), Hint::Device(n.device_key())]);
        }
        t
    }

    #[test]
    fn the_endpoint_key_is_the_passport_device_key() {
        let me = id(7);
        let d = tempfile::tempdir().unwrap();
        // a launcher with no Passport device record yet makes one on first run, and the same key every run after
        let dev = crate::passport::DeviceKey::for_app(d.path(), &me).unwrap();
        let sk = SecretKey::from_bytes(&dev.secret());
        assert_eq!(sk.public().as_bytes(), &dev.public, "iroh and ring agree on the key");
        assert_ne!(dev.public, me.public_key, "never the root key");
        assert_eq!(crate::passport::check_device_cert(&dev.record), Some((me.public_key, dev.public)));
        assert_eq!(dev.check_peer(&dev.record, &dev.public), Some(me.public_key));
        assert_eq!(crate::passport::DeviceKey::for_app(d.path(), &me).unwrap().public, dev.public, "the same every time");
        let st = crate::passport::State::load(d.path(), &me);
        assert!(st.devices.iter().any(|r| crate::passport::check_device_cert(r) == Some((me.public_key, dev.public))), "on the Passport page");
        // a record naming another key, or forged, is refused
        let other = crate::passport::DeviceKey::fresh(&id(8)).unwrap();
        assert_eq!(dev.check_peer(&dev.record, &other.public), None);
        let mut forged = dev.record.clone();
        forged["doc"]["device"] = serde_json::json!(identity::hex(&other.public));
        assert!(crate::passport::check_device_cert(&forged).is_none());
        // a device this Passport removed can't connect as it
        let mut st = crate::passport::State::load(d.path(), &me);
        let seq = st.next_seq(1);
        let rev = crate::passport::make_device_revoke(&me, &identity::hex(&dev.public), 1, seq);
        st.add_revoke(&rev, &me.public_hex()).unwrap();
        let mut checker = crate::passport::DeviceKey::fresh(&me).unwrap();
        checker.revoked = vec![rev];
        assert_eq!(checker.check_peer(&dev.record, &dev.public), None);
    }

    /// A DHT of our own in memory (p2p/dht.rs `Board`).
    #[derive(Default)]
    struct MemBoard(Mutex<HashMap<[u8; 32], mainline::MutableItem>>);
    impl super::super::dht::Board for MemBoard {
        fn put(&self, item: mainline::MutableItem) -> Result<(), String> {
            self.0.lock().unwrap().insert(*item.key(), item);
            Ok(())
        }
        fn get(&self, key: &[u8; 32]) -> Option<mainline::MutableItem> {
            self.0.lock().unwrap().get(key).cloned()
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_passport_pointer_on_the_dht_is_enough_to_reach_a_friend() {
        use super::super::dht;
        let (a, _) = node(1, TestBook::default(), LinkSettings::default()).await;
        let (b, bbook) = node(2, TestBook::default(), LinkSettings::default()).await;
        let (m, _) = node(3, TestBook::default(), LinkSettings::default()).await;
        // b's Passport pointer (under its root): its device key and its address; nothing else tells a where b is
        let board = Arc::new(MemBoard::default());
        let h = dht::Hints { kind: "passport".into(), ts: now(), endpoints: vec![identity::hex(&b.device_key())], addrs: vec![addr_of(&b).to_string()], record_sha256: None, fetch: vec!["peer".into()] };
        dht::publish_with(board.as_ref(), &dht::Key::from_identity(Arc::new(id(2))), &h, 1).unwrap();
        let d = tempfile::tempdir().unwrap();
        let (dir, bb) = (d.path().to_path_buf(), board.clone());
        a.add_lookup(Arc::new(link::DhtLookup::with(move |peer| dht::lookup_with(bb.as_ref(), &dir, peer, now()))));
        let env = json!({"v": 1, "to": id(2).peer_id, "box": "sealed"});
        a.send_envelope(&b.public_key(), &env, Duration::from_secs(20), false).await.unwrap();
        assert_eq!(bbook.got.lock().unwrap()[0], (env.clone(), a.public_key()));
        // a pointer for c (key 4) naming m's device and address: m's device record doesn't vouch for c, refused
        let h = dht::Hints { endpoints: vec![identity::hex(&m.device_key())], addrs: vec![addr_of(&m).to_string()], ..h };
        dht::publish_with(board.as_ref(), &dht::Key::from_identity(Arc::new(id(4))), &h, 2).unwrap();
        assert!(a.send_envelope(&id(4).public_key, &env, Duration::from_secs(8), false).await.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_device_key_that_isnt_theirs_is_refused() {
        let (a, _) = node(1, TestBook::default(), LinkSettings::default()).await;
        let (b, _) = node(2, TestBook::default(), LinkSettings::default()).await;
        let (m, mbook) = node(3, TestBook::default(), LinkSettings::default()).await;
        // a hint says b is at m's device: the cert at connect time doesn't match b's key
        let t = Arc::new(link::Table::default());
        t.put(b.public_key(), vec![Hint::Ip(addr_of(&m)), Hint::Device(m.device_key())]);
        a.add_lookup(t);
        let env = json!({"v": 1, "to": id(2).peer_id, "box": "x"});
        let e = a.send_envelope(&b.public_key(), &env, Duration::from_secs(10), false).await.unwrap_err();
        assert!(e.contains("isn't who it should be"), "{e}");
        assert!(mbook.got.lock().unwrap().is_empty());
        for l in [a, b, m] {
            l.ep.close().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_launchers_connect_by_key_and_pass_bytes() {
        let (a, _) = node(1, TestBook::default(), LinkSettings::default()).await;
        let (b, bbook) = node(2, TestBook::default(), LinkSettings::default()).await;
        a.add_lookup(table(&[&b]));
        // a friend message
        let env = json!({"v": 1, "to": id(2).peer_id, "box": "sealed"});
        let way = a.send_envelope(&b.public_key(), &env, Duration::from_secs(20), false).await.unwrap();
        assert_eq!(way, Way::SameNetwork);
        assert_eq!(bbook.got.lock().unwrap()[0], (env, a.public_key()), "taken, from the dialer's key");
        // a world port behind the knock: an echo server on 127.0.0.1
        let echo = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = echo.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut c, _) = echo.accept().await.unwrap();
            let (mut r, mut w) = c.split();
            let _ = tokio::io::copy(&mut r, &mut w).await;
        });
        b.offer_world("kzdvvj2umnduyauf", "S3CRET", port, port);
        let k = link::knock("kzdvvj2umnduyauf", "S3CRET");
        let (mut s, _) = a.dial(&b.public_key(), "world:kzdvvj2umnduyauf:login", &k, Duration::from_secs(20), false).await.unwrap();
        s.write_all(b"hello world").await.unwrap();
        let mut got = [0u8; 11];
        s.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"hello world");
        // refusals: a wrong knock, a world not shared, an unknown service
        let e = a.dial(&b.public_key(), "world:kzdvvj2umnduyauf:login", &link::knock("kzdvvj2umnduyauf", "x"), Duration::from_secs(20), false).await.err().unwrap();
        assert!(e.contains("invite isn't good"), "{e}");
        b.withdraw_world("kzdvvj2umnduyauf");
        let e = a.dial(&b.public_key(), "world:kzdvvj2umnduyauf:login", &k, Duration::from_secs(20), false).await.err().unwrap();
        assert!(e.contains("isn't shared"), "{e}");
        assert!(a.dial(&b.public_key(), "shell", "", Duration::from_secs(5), false).await.is_err());
        a.ep.close().await;
        b.ep.close().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_mutual_friend_passes_sealed_bytes_and_respects_caps() {
        let (ka, kb, kc) = (id(1).public_key, id(2).public_key, id(3).public_key);
        // a and b can't reach each other; c is friends with both and online
        let (a, _) = node(1, TestBook { friends: vec![kc], online: vec![kc], ..Default::default() }, LinkSettings::default()).await;
        let (b, bbook) = node(2, TestBook { friends: vec![kc], ..Default::default() }, LinkSettings::default()).await;
        let (c, _) = node(3, TestBook { friends: vec![ka, kb], ..Default::default() }, LinkSettings { help_streams: 1, ..Default::default() }).await;
        a.add_lookup(table(&[&c]));
        c.add_lookup(table(&[&b]));
        let env = json!({"v": 1, "to": id(2).peer_id, "box": "only-b-can-open"});
        let way = a.send_envelope(&kb, &env, Duration::from_secs(30), true).await.unwrap();
        assert_eq!(way, Way::Friend);
        assert_eq!(bbook.got.lock().unwrap()[0], (env.clone(), ka), "b knows it came from a: the seal proves it");
        // a world through the friend: the bytes pass, sealed end to end
        let echo = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = echo.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut c, _)) = echo.accept().await {
                tokio::spawn(async move {
                    let (mut r, mut w) = c.split();
                    let _ = tokio::io::copy(&mut r, &mut w).await;
                });
            }
        });
        b.offer_world("kzdvvj2umnduyauf", "S", port, port);
        let k = link::knock("kzdvvj2umnduyauf", "S");
        let (mut s, way) = a.dial(&kb, "world:kzdvvj2umnduyauf:world", &k, Duration::from_secs(30), true).await.unwrap();
        assert_eq!(way, Way::Friend);
        let big = vec![7u8; 100_000];
        let (mut r, mut w) = tokio::io::split(&mut s);
        let send = async {
            w.write_all(&big).await.unwrap();
        };
        let mut got = vec![0u8; big.len()];
        let recv = async { r.read_exact(&mut got).await.unwrap() };
        tokio::join!(send, recv);
        assert_eq!(got, big);
        assert!(c.friend_budget.lock().unwrap().used >= 200_000, "the friend counted what it passed (sealed, so a bit more)");
        // caps: one stream at a time here, so a second is refused while the first is open
        let e = a.dial(&kb, "world:kzdvvj2umnduyauf:world", &k, Duration::from_secs(30), true).await.err().unwrap();
        assert!(e.contains("as many connections"), "{e}");
        drop(s);
        // a day's data used up: refused
        tokio::time::sleep(Duration::from_millis(300)).await;
        {
            let mut bud = c.friend_budget.lock().unwrap();
            bud.live = 0;
            bud.used = bud.cap;
        }
        let e = a.send_envelope(&kb, &env, Duration::from_secs(30), true).await.err().unwrap();
        assert!(e.contains("used up"), "{e}");
        // Help friends connect off: refused
        c.friend_budget.lock().unwrap().used = 0;
        c.set_cfg(LinkSettings { help_friends: false, ..Default::default() });
        let e = a.send_envelope(&kb, &env, Duration::from_secs(30), true).await.err().unwrap();
        assert!(e.contains("doesn't pass"), "{e}");
        for l in [a, b, c] {
            l.ep.close().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_stranger_cant_use_a_friend_as_go_between_but_a_community_relay_carries_anyone() {
        let (ka, kb) = (id(1).public_key, id(2).public_key);
        let (a, _) = node(1, TestBook::default(), LinkSettings::default()).await;
        let (b, bbook) = node(2, TestBook::default(), LinkSettings::default()).await;
        let (n, _) = node(4, TestBook::default(), LinkSettings { community_relay: true, ..Default::default() }).await;
        a.add_lookup(table(&[&n]));
        n.add_lookup(table(&[&b]));
        a.set_community(vec![n.public_key()], vec![]);
        let env = json!({"v": 1, "to": id(2).peer_id, "box": "x"});
        // the node's relay isn't running yet
        let e = a.send_envelope(&kb, &env, Duration::from_secs(30), true).await.err().unwrap();
        assert!(e.contains("doesn't pass"), "{e}");
        n.set_node_relay(true, 10 * 1024 * 1024, 4);
        assert_eq!(a.send_envelope(&kb, &env, Duration::from_secs(30), true).await.unwrap(), Way::Community);
        assert_eq!(bbook.got.lock().unwrap()[0].1, ka);
        assert!(n.community_budget.lock().unwrap().used > 0);
        for l in [a, b, n] {
            l.ep.close().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn messages_wait_with_a_shared_friend_until_the_other_is_back() {
        let (ka, kb, kc) = (id(1).public_key, id(2).public_key, id(3).public_key);
        let (a, _) = node(1, TestBook { friends: vec![kc], online: vec![kc], ..Default::default() }, LinkSettings::default()).await;
        let (c, _) = node(3, TestBook { friends: vec![ka, kb], ..Default::default() }, LinkSettings::default()).await;
        a.add_lookup(table(&[&c]));
        let env = json!({"v": 1, "to": id(2).peer_id, "box": "for-b"});
        // b is offline: a's direct try fails, the friend keeps it
        assert!(a.send_envelope(&kb, &env, Duration::from_secs(3), false).await.is_err());
        a.hold(&kb, &env).await.unwrap();
        assert_eq!(c.held.lock().unwrap().waiting(), vec![identity::hex(&kb)]);
        // a stranger's message for b isn't kept
        let (x, _) = node(9, TestBook { online: vec![kc], ..Default::default() }, LinkSettings::default()).await;
        x.add_lookup(table(&[&c]));
        assert!(x.hold(&kb, &json!({"v": 1, "to": id(2).peer_id, "box": "spam"})).await.unwrap_err().contains("only between friends"));
        // b comes back: the friend hands it over
        let (b, bbook) = node(2, TestBook { friends: vec![kc], ..Default::default() }, LinkSettings::default()).await;
        c.add_lookup(table(&[&b]));
        assert_eq!(c.deliver_held().await, 1);
        assert_eq!(bbook.got.lock().unwrap()[0].0, env);
        assert!(c.held.lock().unwrap().items.is_empty());
        for l in [a, b, c, x] {
            l.ep.close().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_guest_game_reaches_the_hosts_world_through_127_0_0_1() {
        // the host's stand-in login server: answers any bytes with a realm list naming 127.0.0.1:8085
        let srv = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let sp = srv.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut c, _)) = srv.accept().await {
                tokio::spawn(async move {
                    let mut b = [0u8; 5];
                    if c.read_exact(&mut b).await.is_ok() {
                        let mut body = vec![0, 0, 0, 0, 1, 0, 0, 0, 0];
                        body.extend(b"W\0127.0.0.1:8085\0");
                        body.extend(0.0f32.to_le_bytes());
                        body.extend([1, 1, 1, 0x10, 0x00]);
                        let mut m = vec![0x10];
                        m.extend((body.len() as u16).to_le_bytes());
                        m.extend(body);
                        let _ = c.write_all(&m).await;
                    }
                });
            }
        });
        let (host, _) = node(2, TestBook::default(), LinkSettings::default()).await;
        let (guest, _) = node(1, TestBook::default(), LinkSettings::default()).await;
        guest.add_lookup(table(&[&host]));
        host.offer_world("kzdvvj2umnduyauf", "S", sp, sp);
        let (addr, way) = guest_start(&guest, host.public_key(), "kzdvvj2umnduyauf", "S", crate::host::worldshare::Layout::Wrath).await.unwrap();
        assert_eq!(way, Way::SameNetwork);
        assert!(addr.starts_with("127.0.0.1:"));
        let mut game = tokio::net::TcpStream::connect(addr.as_str()).await.unwrap();
        game.write_all(&[0x10, 0, 0, 0, 0]).await.unwrap();
        let mut got = vec![0u8; 3];
        game.read_exact(&mut got).await.unwrap();
        let n = u16::from_le_bytes([got[1], got[2]]) as usize;
        let mut body = vec![0u8; n];
        game.read_exact(&mut body).await.unwrap();
        let wp = GUEST.lock().unwrap().as_ref().unwrap().world;
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains(&format!("127.0.0.1:{wp}")), "the realm list names this PC's own world port: {text}");
        guest_stop();
        host.ep.close().await;
        guest.ep.close().await;
    }
}
