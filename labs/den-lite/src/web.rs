//! Den Lite inside a web page (5 Nov, the phone's solo play: docs/PHONE-SOLO.md). Built for `wasm32-wasip1`
//! (`cargo build --release --target wasm32-wasip1 --features web --bin den-lite-web`) and run in a Web Worker by
//! `stream/www-files/solo/den-lite-worker.js`, which gives it a small in-memory file system (WASI; the Dens folder is
//! `/dens`, kept in the browser's IndexedDB by the page) and the browser client's sockets.
//!
//! No sockets and no threads in there, so this is a single-threaded loop around the same sessions the PC runs:
//!
//! * The page's fake WebSocket (`solo.js`) turns each socket the client opens into a numbered connection and puts its
//!   bytes into a shared ring; [`pump`] takes one record at a time through the `leyline` imports below (`ly_recv`
//!   blocks the worker with `Atomics.wait` until a record comes or the time is up).
//! * A new connection runs its whole session at once (login.rs `session` on port 3724, world.rs `session` on any
//!   other), with a [`PageWire`] as its stream. When that session waits for bytes, its read calls [`pump`] again, so
//!   a connection opened meanwhile (the world after the login) runs inside it, and the companion's and the Dojo's
//!   beats ([`tick`]) keep their clocks. A session ends when its socket closes; the one it ran inside goes on.
//! * What a session writes goes straight to the page (`ly_send`), one message per write, as the PC's TCP does.
//!
//! Known limit of running sessions inside each other: the outer one waits while an inner one runs. The client only
//! talks to its login connection before it opens the world one (and the solo realm list has one realm), so it never
//! needs both at once; "Change realm" from the character list would wait for the world connection to close.

use crate::wire::{Stream, Wire};
use crate::{Config, Gender, Shared};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[link(wasm_import_module = "leyline")]
extern "C" {
    /// One record from the page into `buf` (at most `cap` bytes), waiting up to `timeout_ms` for it: its length, 0
    /// when none came, or minus the length it needs when `cap` is too small (the record is kept for the next call).
    fn ly_recv(buf: *mut u8, cap: u32, timeout_ms: u32) -> i32;
    /// Bytes a session wrote on connection `conn`: to the client's socket as one message.
    fn ly_send(conn: u32, ptr: *const u8, len: u32);
    /// Connection `conn` is closed on this side.
    fn ly_close(conn: u32);
}

/// A record's kind (its first byte; then the connection's number, u32 little endian, then the rest).
pub const OPEN: u8 = 1; // the rest: the port the client dialled (u16 little endian)
pub const DATA: u8 = 2; // the rest: bytes from the client
pub const CLOSE: u8 = 3; // the client closed it
pub const STOP: u8 = 4; // the page is going: every session ends

pub const LOGIN_PORT: u16 = 3724;
pub const WORLD_PORT: u16 = 8085;
/// How many sessions may run inside each other (a client that keeps opening sockets is refused past it).
const MAX_DEPTH: u32 = 8;
const COMPANION_MS: u64 = 500;
const DOJO_MS: u64 = 50;

#[derive(Default)]
struct Inbox {
    bytes: VecDeque<u8>,
    closed: bool,
}

struct Clock {
    started: Instant,
    companion_at: u64,
    dojo_at: u64,
    beats: u64,
}

thread_local! {
    static CONNS: RefCell<HashMap<u32, Inbox>> = RefCell::new(HashMap::new());
    static SHARED: RefCell<Option<Arc<Shared>>> = const { RefCell::new(None) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    static TICKING: Cell<bool> = const { Cell::new(false) };
    static STOPPED: Cell<bool> = const { Cell::new(false) };
    static CLOCK: RefCell<Option<Clock>> = const { RefCell::new(None) };
    static BUF: RefCell<Vec<u8>> = RefCell::new(vec![0u8; 64 * 1024]);
}

/// One client socket, as a session's stream.
pub struct PageWire {
    id: u32,
}

impl Read for PageWire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            let got = CONNS.with(|c| {
                let mut c = c.borrow_mut();
                match c.get_mut(&self.id) {
                    None => Some(0),
                    Some(i) if !i.bytes.is_empty() => {
                        let n = buf.len().min(i.bytes.len());
                        for (k, b) in i.bytes.drain(..n).enumerate() {
                            buf[k] = b;
                        }
                        Some(n)
                    }
                    Some(i) if i.closed => Some(0),
                    Some(_) => None,
                }
            });
            if let Some(n) = got {
                return Ok(n);
            }
            if STOPPED.with(|s| s.get()) {
                return Ok(0);
            }
            pump(1000);
        }
    }
}

impl Write for PageWire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let open = CONNS.with(|c| c.borrow().get(&self.id).is_some_and(|i| !i.closed));
        if !open {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "the socket is closed"));
        }
        if !buf.is_empty() {
            // SAFETY: the page copies the bytes out of memory before it returns
            unsafe { ly_send(self.id, buf.as_ptr(), buf.len() as u32) };
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Wire for PageWire {
    fn clone_wire(&self) -> io::Result<Box<dyn Wire>> {
        Ok(Box::new(PageWire { id: self.id }))
    }
    fn close(&self) {
        let was_open = CONNS.with(|c| c.borrow_mut().get_mut(&self.id).map(|i| !std::mem::replace(&mut i.closed, true)).unwrap_or(false));
        if was_open {
            // SAFETY: a plain call into the page
            unsafe { ly_close(self.id) };
        }
    }
}

fn now_ms() -> u64 {
    CLOCK.with(|c| c.borrow().as_ref().map(|k| k.started.elapsed().as_millis() as u64).unwrap_or(0))
}

/// Milliseconds until the next beat is due.
fn until_beat() -> u64 {
    let now = now_ms();
    CLOCK.with(|c| c.borrow().as_ref().map(|k| k.companion_at.min(k.dojo_at).saturating_sub(now)).unwrap_or(1000))
}

/// The companion's and the Dojo's beats that are due (world.rs companion_step, dojo_step), never inside each other.
fn tick() {
    if TICKING.with(|t| t.replace(true)) {
        return;
    }
    let Some(shared) = SHARED.with(|s| s.borrow().clone()) else {
        TICKING.with(|t| t.set(false));
        return;
    };
    let now = now_ms();
    let (companion, dojo, beats) = CLOCK.with(|c| {
        let mut c = c.borrow_mut();
        let Some(k) = c.as_mut() else { return (false, false, 0) };
        let companion = now >= k.companion_at;
        if companion {
            k.beats += 1;
            // a worker paused while the page was hidden doesn't make up the beats it missed
            k.companion_at = (k.companion_at + COMPANION_MS).max(now);
        }
        let dojo = now >= k.dojo_at;
        if dojo {
            k.dojo_at = (k.dojo_at + DOJO_MS).max(now);
        }
        (companion, dojo, k.beats)
    });
    if dojo {
        crate::world::dojo_step(&shared);
    }
    if companion {
        crate::world::companion_step(&shared, beats);
    }
    TICKING.with(|t| t.set(false));
}

/// One record from the page, waiting at most `max_wait_ms` (less when a beat is due first), then the beats due.
pub fn pump(max_wait_ms: u32) {
    let wait = (max_wait_ms as u64).min(until_beat()) as u32;
    if let Some((kind, conn, rest)) = recv(wait) {
        handle(kind, conn, rest);
    }
    tick();
}

fn recv(wait: u32) -> Option<(u8, u32, Vec<u8>)> {
    BUF.with(|b| {
        let mut b = b.borrow_mut();
        loop {
            // SAFETY: the page writes at most `cap` bytes into the buffer
            let n = unsafe { ly_recv(b.as_mut_ptr(), b.len() as u32, wait) };
            if n < 0 {
                let need = (-(n as i64)) as usize;
                b.resize(need.next_power_of_two(), 0);
                continue;
            }
            let n = n as usize;
            if n < 5 {
                return None;
            }
            let conn = u32::from_le_bytes([b[1], b[2], b[3], b[4]]);
            return Some((b[0], conn, b[5..n].to_vec()));
        }
    })
}

fn handle(kind: u8, conn: u32, rest: Vec<u8>) {
    match kind {
        OPEN => {
            let port = if rest.len() >= 2 { u16::from_le_bytes([rest[0], rest[1]]) } else { 0 };
            CONNS.with(|c| c.borrow_mut().insert(conn, Inbox::default()));
            run_session(conn, port);
        }
        DATA => CONNS.with(|c| {
            if let Some(i) = c.borrow_mut().get_mut(&conn) {
                i.bytes.extend(rest);
            }
        }),
        CLOSE => CONNS.with(|c| {
            if let Some(i) = c.borrow_mut().get_mut(&conn) {
                i.closed = true;
            }
        }),
        STOP => {
            STOPPED.with(|s| s.set(true));
            CONNS.with(|c| c.borrow_mut().values_mut().for_each(|i| i.closed = true));
        }
        _ => {}
    }
}

/// A connection's whole session, here and now (inside whatever session's read was waiting).
fn run_session(conn: u32, port: u16) {
    let depth = DEPTH.with(|d| d.get());
    let Some(shared) = SHARED.with(|s| s.borrow().clone()) else { return };
    if depth >= MAX_DEPTH {
        shared.event("page_refused", "", &format!("{port}"));
        PageWire { id: conn }.close();
        CONNS.with(|c| c.borrow_mut().remove(&conn));
        return;
    }
    DEPTH.with(|d| d.set(depth + 1));
    shared.event("page_socket", "", &format!("{port} open"));
    let s = Stream::new(PageWire { id: conn });
    let r = if port == LOGIN_PORT { crate::login::session(s, &shared, false, None) } else { crate::world::session(s, &shared, false) };
    if let Err(e) = r {
        if e.kind() != io::ErrorKind::UnexpectedEof {
            shared.log(&format!("page {port}: {e}"));
        }
    }
    PageWire { id: conn }.close();
    CONNS.with(|c| c.borrow_mut().remove(&conn));
    shared.event("page_socket", "", &format!("{port} closed"));
    DEPTH.with(|d| d.set(depth));
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

/// The page's settings (the worker's environment): the Dens folder, the hero, the password the page logs in with,
/// the realm's name, and where the world is (a Den save opened from the Dens folder is the whole world; DEN_LITE_SPOT
/// "map x y z" puts it somewhere else, e.g. a starting zone the client streams).
pub fn config_from_env() -> Config {
    let mut cfg = Config {
        den_dir: PathBuf::from(env("DEN_LITE_DENS").unwrap_or_else(|| "/dens".into())),
        stamps_dir: PathBuf::from("/stamps"),
        remote_ports: None,
        friends_lan: None,
        realm_name: env("DEN_LITE_REALM").unwrap_or_else(|| "Solo".into()),
        verbose: env("DEN_LITE_VERBOSE").is_some(),
        ..Config::default()
    };
    if let Some(p) = env("DEN_LITE_PASSWORD") {
        cfg.password = p;
    }
    if let Some(n) = env("DEN_LITE_NAME").filter(|n| n.len() <= 12 && n.chars().all(|c| c.is_ascii_alphabetic())) {
        cfg.hero.name = n;
    }
    if env("DEN_LITE_GENDER").as_deref() == Some("male") {
        cfg.hero.gender = Gender::Male;
    }
    if env("DEN_LITE_NO_COMPANION").is_some() {
        cfg.companion = false;
    }
    if let Some(s) = env("DEN_LITE_SPOT") {
        let v: Vec<f32> = s.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if v.len() == 4 && v[0] >= 0.0 && v[1..].iter().all(|x| x.is_finite()) {
            cfg.spot = crate::den::Spot { map: v[0] as u32, x: v[1], y: v[2], z: v[3], ..cfg.spot };
        }
    }
    cfg
}

/// The worker's whole life: the Den opened from the Dens folder, then records and beats until the page stops it.
pub fn main() {
    let cfg = config_from_env();
    let _ = std::fs::create_dir_all(&cfg.den_dir);
    let world_addr: SocketAddr = ([127, 0, 0, 1], WORLD_PORT).into();
    let shared = crate::build_shared(cfg, LOGIN_PORT, world_addr, None);
    CLOCK.with(|c| *c.borrow_mut() = Some(Clock { started: Instant::now(), companion_at: COMPANION_MS, dojo_at: DOJO_MS, beats: 0 }));
    SHARED.with(|s| *s.borrow_mut() = Some(shared.clone()));
    shared.event("page_ready", "", "");
    while !STOPPED.with(|s| s.get()) {
        pump(1000);
    }
    // the last autosave before the page goes
    let _ = shared.autosave();
}
