//! The sign-in cover (owner 3 Nov: PLAY → character select "smooth"; the player never sees the sign-in being typed).
//!
//! While the launcher signs the player in at the game's own login screen by keys (app.rs `keys_sign_in` on the
//! official realm, crate::denclick for Your Den), a Leyline-styled "Logging you in…" screen (ui/cover.html) sits
//! exactly over the game window's picture, follows it if it moves, and steps aside as soon as the sign-in is done:
//!
//! * shown as soon as the game's window exists (before the login screen draws), kept while the keys are typed;
//! * official realm: hidden a moment after the game connects to the realm's world server ([`Net::World`]: the
//!   game's own TCP connections, read from the system's connection table by its process id, no game memory read):
//!   that connection is made only after the login and the realm are done, and character select follows at once;
//! * Your Den: hidden when Den Lite logs that the character is entering the world (its loading screen shows);
//! * hidden at once when anything goes another way: the game closes, the keys didn't reach it, the login server
//!   answered without a world connection (a wrong password, the realm list waiting for the player), Esc, the
//!   cover's "Show the game", or the time limit ([`Watch::max_ms`]). The player is never stranded behind it.
//!
//! The cover never takes the keyboard (Windows: WS_EX_NOACTIVATE, shown without activation); the keys are posted
//! window messages to the game's own window (denclick::win), which need no focus. When it hides, the game window is
//! brought to the front.
//!
//! Everything that decides is here and pure ([`Watch::tick`], [`from_corners`], [`classify`], [`run`] with a
//! [`Screen`]), so every path is tested without a game. The window itself is app.rs (`cover_start`); the Windows
//! calls are [`win`]. It never sees or logs a password.

use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;

/// The cover window's label (app.rs).
pub const LABEL: &str = "signin-cover";
/// The cover's page, in ui/.
pub const PAGE: &str = "cover.html";
/// How often the cover follows the game window and looks at the sign-in (ms).
pub const POLL_MS: u64 = 200;
/// The game's window must open within this (ms from the start), or the cover gives up (it never showed).
pub const WINDOW_MAX: u64 = 60_000;
/// Official realm: the longest the cover stays up, from the game window opening (the owner's smoke run: character
/// select 14 s after the start; the keys go 7 s after the window).
pub const MAX_OFFICIAL: u64 = 25_000;
/// Your Den: the longest (its keys wait for each step; a first start is slower).
pub const MAX_DEN: u64 = 45_000;
/// After the world connection: character select draws its list (ms).
pub const READY_GRACE: u64 = 900;
/// Logged in, but no world connection this long: the realm list wants the player (ms).
pub const REALM_WAIT: u64 = 8_000;
/// The login connection closed with no world connection this long: the login screen said something (ms).
pub const AUTH_LOST: u64 = 1_500;
/// The login server's port (the realmlist's), the same everywhere.
pub const AUTH_PORT: u16 = 3724;
/// Smaller than this (a side, in pixels) is not a game picture to cover.
pub const MIN_SIDE: i32 = 120;

/// A rectangle on the screen, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// The game's picture (its client area) on the screen, from its top-left and bottom-right corners as the system maps
/// them to the screen (Windows ClientToScreen of (0,0) and of the client size, both in the launcher's own DPI
/// awareness, so a game without DPI awareness on a scaled screen still lines up). None when the window is minimised
/// (Windows parks it near -32000) or too small to be the game's picture.
pub fn from_corners(tl: (i32, i32), br: (i32, i32)) -> Option<Rect> {
    let (w, h) = (br.0.saturating_sub(tl.0), br.1.saturating_sub(tl.1));
    if tl.0 <= -30_000 || tl.1 <= -30_000 || w < MIN_SIDE || h < MIN_SIDE {
        return None;
    }
    Some(Rect { x: tl.0, y: tl.1, w, h })
}

/// Where the game is in its connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Net {
    /// No connection to a server.
    None,
    /// Connected to the login server (the login sent, or the realm list up).
    Auth,
    /// Connected to a realm's world server: past the login and the realm list.
    World,
}

/// From the remote ports of the game's open (established) TCP connections: the login port is the login server; any
/// other port is a world server, except the web's (80, 443) which a game can open for other things.
pub fn classify(remote_ports: &[u16]) -> Net {
    if remote_ports.iter().any(|p| *p != AUTH_PORT && *p != 80 && *p != 443) {
        Net::World
    } else if remote_ports.contains(&AUTH_PORT) {
        Net::Auth
    } else {
        Net::None
    }
}

/// Which sign-in the cover is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Mode {
    /// PLAY on the official realm (app.rs keys_sign_in): stops at character select.
    Official,
    /// Your Den's one click (crate::denclick): goes on into the world.
    Den,
}

/// The steps the cover shows, in order.
pub fn steps(mode: Mode) -> Vec<String> {
    let last = match mode {
        Mode::Official => "Character select",
        Mode::Den => "Into your Den",
    };
    ["Opening the game", "Signing in", "Choosing your realm", last].iter().map(|s| s.to_string()).collect()
}

/// The cover's heading: "Logging you in as Orion…" (an account name in capitals, as the game keeps it, is shown as a
/// name; a website game login like WR000123 as it is), or for the Den "Opening your Den…".
pub fn title(mode: Mode, name: &str) -> String {
    if mode == Mode::Den {
        return "Opening your Den…".into();
    }
    let name: String = name.trim().chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '_' || *c == '-').take(32).collect();
    if name.is_empty() {
        return "Logging you in…".into();
    }
    // a game login such as WR000123 stays as it is; a name in capitals (ORION) reads as a name
    let shown = if name.chars().any(|c| c.is_lowercase() || c.is_ascii_digit()) {
        name
    } else {
        let mut cs = name.chars();
        let first = cs.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
        first + &cs.as_str().to_lowercase()
    };
    format!("Logging you in as {shown}…")
}

/// The keys' side (the official realm's keys thread).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keys {
    Waiting,
    Sent,
    Failed,
}

/// Your Den: where Den Lite's milestone log (crate::denclick::EVENTS) says the start is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DenStage {
    None,
    /// The login screen sent the login (login_challenge).
    Asked,
    /// Logged in: the realm list (auth_ok, realm_list).
    Realm,
    /// Character select (world_auth, char_enum).
    CharSelect,
    /// Entering the world: the loading screen (player_login, in_world).
    Entering,
}

/// The furthest milestone in Den Lite's log lines of this start.
pub fn den_stage(evs: &[crate::denclick::Ev]) -> DenStage {
    evs.iter()
        .map(|e| match e.ev.as_str() {
            "login_challenge" => DenStage::Asked,
            "auth_ok" | "realm_list" => DenStage::Realm,
            "world_auth" | "char_enum" => DenStage::CharSelect,
            "player_login" | "in_world" => DenStage::Entering,
            _ => DenStage::None,
        })
        .max()
        .unwrap_or(DenStage::None)
}

/// What the cover sees at one look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seen {
    /// The game still runs.
    pub alive: bool,
    /// The game's window is open (even when minimised).
    pub present: bool,
    /// Its picture on the screen (None: minimised, or not open).
    pub window: Option<Rect>,
    pub keys: Keys,
    pub net: Net,
    pub den: DenStage,
    /// Esc, or "Show the game".
    pub dismissed: bool,
    /// Your Den: the launcher's sign-in finished (Some(true): in the world; Some(false): it stopped).
    pub finished: Option<bool>,
}

impl Default for Seen {
    fn default() -> Self {
        Seen { alive: true, present: false, window: None, keys: Keys::Waiting, net: Net::None, den: DenStage::None, dismissed: false, finished: None }
    }
}

/// Why the cover went away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum End {
    /// Character select (official) or the world's loading screen (Den).
    Ready,
    /// Esc or "Show the game".
    Dismissed,
    GameClosed,
    NoWindow,
    /// The keys didn't reach the game, or the Den's sign-in stopped.
    Failed,
    /// Logged in, but the realm list waits for the player.
    RealmList,
    /// The login connection closed without a world one (a wrong password, a message on the login screen).
    LoginAnswered,
    Timeout,
}

/// What to do with the cover now.
#[derive(Debug, Clone, PartialEq)]
pub enum Do {
    /// No game window yet: nothing shown.
    Nothing,
    /// Hidden for now (the game is minimised); back when it is.
    Park,
    /// Over this rectangle, with this step lit.
    Show { rect: Rect, step: usize },
    /// Gone for this start.
    Hide(End),
}

/// The cover's decisions for one start. Pure: [`Watch::tick`] with the time and what is seen.
#[derive(Debug, Clone)]
pub struct Watch {
    pub mode: Mode,
    pub start: u64,
    pub step: usize,
    window_at: Option<u64>,
    auth_at: Option<u64>,
    auth_lost_at: Option<u64>,
    world_at: Option<u64>,
    end: Option<End>,
    /// What happened, for the log: (ms from the start, what). Never a password.
    pub log: Vec<(u64, String)>,
}

impl Watch {
    pub fn new(mode: Mode, start: u64) -> Watch {
        Watch { mode, start, step: 0, window_at: None, auth_at: None, auth_lost_at: None, world_at: None, end: None, log: Vec::new() }
    }

    pub fn max_ms(&self) -> u64 {
        match self.mode {
            Mode::Official => MAX_OFFICIAL,
            Mode::Den => MAX_DEN,
        }
    }

    pub fn ended(&self) -> Option<&End> {
        self.end.as_ref()
    }

    fn note(&mut self, now: u64, what: impl Into<String>) {
        self.log.push((now.saturating_sub(self.start), what.into()));
    }

    fn finish(&mut self, now: u64, e: End) -> Do {
        self.note(now, format!("hide: {e:?}"));
        self.end = Some(e.clone());
        Do::Hide(e)
    }

    fn to_step(&mut self, now: u64, s: usize) {
        if s > self.step {
            self.step = s;
            let name = steps(self.mode).get(s).cloned().unwrap_or_default();
            self.note(now, format!("step: {name}"));
        }
    }

    pub fn tick(&mut self, now: u64, s: &Seen) -> Do {
        if let Some(e) = &self.end {
            return Do::Hide(e.clone());
        }
        if s.dismissed {
            return self.finish(now, End::Dismissed);
        }
        if !s.alive {
            return self.finish(now, End::GameClosed);
        }
        if s.keys == Keys::Failed {
            return self.finish(now, End::Failed);
        }
        match s.finished {
            Some(true) => return self.finish(now, End::Ready),
            Some(false) => return self.finish(now, End::Failed),
            None => {}
        }
        let Some(at) = self.window_at.or_else(|| s.present.then_some(now)) else {
            if now.saturating_sub(self.start) > WINDOW_MAX {
                return self.finish(now, End::NoWindow);
            }
            return Do::Nothing;
        };
        if self.window_at.is_none() {
            self.window_at = Some(at);
            self.note(now, "the game window is open: cover up");
        }
        if now.saturating_sub(at) > self.max_ms() {
            return self.finish(now, End::Timeout);
        }
        match self.mode {
            Mode::Official => {
                if s.keys == Keys::Sent {
                    self.to_step(now, 1);
                }
                match s.net {
                    Net::Auth => {
                        if self.auth_at.is_none() {
                            self.auth_at = Some(now);
                            self.note(now, "connected to the login server");
                        }
                        self.auth_lost_at = None;
                        self.to_step(now, 2);
                    }
                    // a world connection counts only after the login (nothing else of the game's comes first)
                    Net::World if self.auth_at.is_some() || s.keys == Keys::Sent => {
                        if self.world_at.is_none() {
                            self.world_at = Some(now);
                            self.note(now, "connected to the realm's world server");
                        }
                        self.to_step(now, 3);
                    }
                    Net::World => {}
                    Net::None => {
                        if self.auth_at.is_some() && self.world_at.is_none() && self.auth_lost_at.is_none() {
                            self.auth_lost_at = Some(now);
                        }
                    }
                }
                if let Some(w) = self.world_at {
                    if now.saturating_sub(w) >= READY_GRACE {
                        return self.finish(now, End::Ready);
                    }
                } else if let Some(l) = self.auth_lost_at {
                    if now.saturating_sub(l) >= AUTH_LOST {
                        return self.finish(now, End::LoginAnswered);
                    }
                } else if let Some(a) = self.auth_at {
                    if now.saturating_sub(a) >= REALM_WAIT {
                        return self.finish(now, End::RealmList);
                    }
                }
            }
            Mode::Den => match s.den {
                DenStage::Entering => return self.finish(now, End::Ready),
                DenStage::CharSelect => self.to_step(now, 3),
                DenStage::Realm => self.to_step(now, 2),
                DenStage::Asked => self.to_step(now, 1),
                DenStage::None => {}
            },
        }
        match s.window {
            Some(rect) => Do::Show { rect, step: self.step },
            None => Do::Park,
        }
    }
}

// ------------------------------------------------------------------------------------------------ shared state

/// The keys' and the Den's sign-in news for the cover (set by the thread that signs in).
#[derive(Debug, Default)]
pub struct Signals {
    keys: AtomicU8,
    finished: AtomicU8,
}

impl Signals {
    pub fn keys_sent(&self) {
        self.keys.store(1, Ordering::Relaxed);
    }
    pub fn keys_failed(&self) {
        self.keys.store(2, Ordering::Relaxed);
    }
    pub fn keys(&self) -> Keys {
        match self.keys.load(Ordering::Relaxed) {
            1 => Keys::Sent,
            2 => Keys::Failed,
            _ => Keys::Waiting,
        }
    }
    /// The Den's sign-in returned: in the world, or stopped.
    pub fn finish(&self, in_world: bool) {
        self.finished.store(if in_world { 1 } else { 2 }, Ordering::Relaxed);
    }
    pub fn finished(&self) -> Option<bool> {
        match self.finished.load(Ordering::Relaxed) {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    }
}

/// What the cover's page shows (ui/cover.html asks for it: app.rs `cover_state`).
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct View {
    /// This start's number: "Show the game" names it, so an old page can't hide a newer start's cover.
    pub id: u64,
    pub title: String,
    pub steps: Vec<String>,
    pub step: usize,
    pub shown: bool,
}

static VIEW: Mutex<View> = Mutex::new(View { id: 0, title: String::new(), steps: Vec::new(), step: 0, shown: false });
static NEXT: AtomicU64 = AtomicU64::new(1);
static DISMISSED: AtomicU64 = AtomicU64::new(0);

/// A new start's cover: its number (a newer start takes the page over; an older one then stops).
pub fn begin(mode: Mode, title: String) -> u64 {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut v) = VIEW.lock() {
        *v = View { id, title, steps: steps(mode), step: 0, shown: false };
    }
    id
}

pub fn view() -> View {
    VIEW.lock().map(|v| v.clone()).unwrap_or_default()
}

fn set_view(id: u64, shown: bool, step: usize) {
    if let Ok(mut v) = VIEW.lock() {
        if v.id == id {
            v.shown = shown;
            v.step = step;
        }
    }
}

/// The current start (this one's page is the cover's).
pub fn current() -> u64 {
    VIEW.lock().map(|v| v.id).unwrap_or(0)
}

/// "Show the game" (or Esc on the page): this start's cover goes. 0: whatever is up.
pub fn dismiss(id: u64) {
    let id = if id == 0 { current() } else { id };
    DISMISSED.store(id, Ordering::Relaxed);
}

fn dismissed(id: u64) -> bool {
    DISMISSED.load(Ordering::Relaxed) == id
}

// ------------------------------------------------------------------------------------------------ the loop

/// The screen side of the cover: the game's window and connections, the keyboard, and the cover window itself.
pub trait Screen {
    /// (the game runs, its window is open, its picture on the screen)
    fn game(&mut self) -> (bool, bool, Option<Rect>);
    fn net(&mut self) -> Net;
    /// Esc went down since the last look.
    fn esc(&mut self) -> bool;
    /// The cover over this rectangle (moved only when it changed), without taking the keyboard.
    fn show(&mut self, r: Rect);
    /// Hidden for now.
    fn park(&mut self);
    /// Gone; `front`: the game window to the front.
    fn hide(&mut self, front: bool);
}

/// Runs one start's cover until it hides. `den`: Den Lite's Dens folder and the start (unix ms) to read its
/// milestones from. `now` gives the time (ms) and `sleep` waits between looks: the real clock and a real wait in the
/// launcher, a fake clock in the tests.
pub fn run(screen: &mut dyn Screen, mut w: Watch, sig: &Signals, den: Option<(&Path, u64)>, id: u64, now: &mut dyn FnMut() -> u64, sleep: &mut dyn FnMut(u64)) -> (End, Vec<(u64, String)>) {
    let _ = screen.esc(); // an earlier Esc is not this start's
    let mut last: Option<Rect> = None;
    loop {
        let t = now();
        let (alive, present, window) = screen.game();
        let net = if w.mode == Mode::Official && present { screen.net() } else { Net::None };
        let den_stage = match den {
            Some((dir, since)) if w.mode == Mode::Den => den_stage(&crate::denclick::events_since(dir, since)),
            _ => DenStage::None,
        };
        let shown = last.is_some();
        // a newer start took the page over: this one stops
        let replaced = current() != id;
        let seen = Seen {
            alive,
            present,
            window,
            keys: sig.keys(),
            net,
            den: den_stage,
            dismissed: dismissed(id) || replaced || (shown && screen.esc()),
            finished: sig.finished(),
        };
        match w.tick(t, &seen) {
            Do::Nothing => {}
            Do::Park => {
                if last.take().is_some() {
                    screen.park();
                }
                set_view(id, false, w.step);
            }
            Do::Show { rect, step } => {
                if last != Some(rect) {
                    screen.show(rect);
                    last = Some(rect);
                }
                set_view(id, true, step);
            }
            Do::Hide(e) => {
                // the game to the front unless it closed; a newer start has the cover window now: left to it
                if !replaced {
                    screen.hide(alive && present);
                }
                set_view(id, false, w.step);
                return (e, w.log);
            }
        }
        sleep(POLL_MS);
    }
}

/// The cover's log line for `<data>/signin-cover.log`: what happened, when (never a password).
pub fn log_text(mode: Mode, end: &End, log: &[(u64, String)]) -> String {
    let mut s = format!("sign-in cover ({mode:?})\n");
    for (t, a) in log {
        s.push_str(&format!("{t:>7} ms  {a}\n"));
    }
    s.push_str(&format!("end: {end:?}\n"));
    s
}

// ------------------------------------------------------------------------------------------------ Windows

#[cfg(windows)]
pub mod win {
    //! The game's picture on the screen, its connections, Esc, and the cover window placed over the game without
    //! taking the keyboard.
    use super::{classify, from_corners, Net, Rect};

    type Hwnd = isize;

    #[repr(C)]
    #[derive(Default)]
    struct Rc {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Pt {
        x: i32,
        y: i32,
    }

    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    const HWND_TOPMOST: Hwnd = -1;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    const SWP_NOOWNERZORDER: u32 = 0x0200;
    const SW_HIDE: i32 = 0;
    const SW_RESTORE: i32 = 9;
    const VK_ESCAPE: i32 = 0x1B;
    const AF_INET: u32 = 2;
    const TCP_TABLE_OWNER_PID_CONNECTIONS: i32 = 4;
    const MIB_TCP_STATE_ESTAB: u32 = 5;
    const NO_ERROR: u32 = 0;

    extern "system" {
        fn GetClientRect(h: Hwnd, r: *mut Rc) -> i32;
        fn ClientToScreen(h: Hwnd, p: *mut Pt) -> i32;
        fn IsIconic(h: Hwnd) -> i32;
        fn SetWindowPos(h: Hwnd, after: Hwnd, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
        fn ShowWindow(h: Hwnd, cmd: i32) -> i32;
        fn GetAsyncKeyState(vk: i32) -> i16;
        fn SetForegroundWindow(h: Hwnd) -> i32;
    }
    #[cfg(target_pointer_width = "64")]
    extern "system" {
        fn GetWindowLongPtrW(h: Hwnd, i: i32) -> isize;
        fn SetWindowLongPtrW(h: Hwnd, i: i32, v: isize) -> isize;
    }
    #[cfg(target_pointer_width = "32")]
    extern "system" {
        #[link_name = "GetWindowLongW"]
        fn GetWindowLongPtrW(h: Hwnd, i: i32) -> isize;
        #[link_name = "SetWindowLongW"]
        fn SetWindowLongPtrW(h: Hwnd, i: i32, v: isize) -> isize;
    }
    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetExtendedTcpTable(table: *mut core::ffi::c_void, size: *mut u32, order: i32, af: u32, class: i32, reserved: u32) -> u32;
    }

    /// The game's picture on the screen (its client area), None when minimised.
    pub fn client_rect(h: Hwnd) -> Option<Rect> {
        // SAFETY: plain user32 queries on the game's window handle, into our own structs
        unsafe {
            if IsIconic(h) != 0 {
                return None;
            }
            let mut r = Rc::default();
            if GetClientRect(h, &mut r) == 0 {
                return None;
            }
            let mut tl = Pt { x: 0, y: 0 };
            let mut br = Pt { x: r.right, y: r.bottom };
            if ClientToScreen(h, &mut tl) == 0 || ClientToScreen(h, &mut br) == 0 {
                return None;
            }
            from_corners((tl.x, tl.y), (br.x, br.y))
        }
    }

    /// The remote ports of the process's established IPv4 TCP connections (the game speaks IPv4 only).
    pub fn remote_ports(pid: u32) -> Vec<u16> {
        let mut size = 0u32;
        // SAFETY: the first call only asks for the size; the second fills a buffer of that size (u32-aligned)
        unsafe {
            GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_CONNECTIONS, 0);
            for _ in 0..3 {
                if size == 0 {
                    return Vec::new();
                }
                let mut buf = vec![0u32; (size as usize).div_ceil(4) + 6];
                let mut got = (buf.len() * 4) as u32;
                let r = GetExtendedTcpTable(buf.as_mut_ptr().cast(), &mut got, 0, AF_INET, TCP_TABLE_OWNER_PID_CONNECTIONS, 0);
                if r != NO_ERROR {
                    size = got.max(size + 1024); // the table grew in between: once more
                    continue;
                }
                // MIB_TCPTABLE_OWNER_PID: the count, then rows of six u32: state, local addr, local port, remote
                // addr, remote port (network order in the low 16 bits), owning pid
                let n = buf[0] as usize;
                return (0..n)
                    .filter_map(|i| buf.get(1 + i * 6..1 + i * 6 + 6))
                    .filter(|row| row[5] == pid && row[0] == MIB_TCP_STATE_ESTAB)
                    .map(|row| u16::from_be((row[4] & 0xFFFF) as u16))
                    .collect();
            }
            Vec::new()
        }
    }

    pub fn net(pid: u32) -> Net {
        classify(&remote_ports(pid))
    }

    /// Esc went down since the last call (or is down now).
    pub fn esc() -> bool {
        // SAFETY: a key state query
        let s = unsafe { GetAsyncKeyState(VK_ESCAPE) } as u16;
        s & 0x8001 != 0
    }

    /// The cover never takes the keyboard and stays off the taskbar and Alt+Tab.
    pub fn prepare(cover: Hwnd) {
        // SAFETY: changing our own window's extended style
        unsafe {
            let ex = GetWindowLongPtrW(cover, GWL_EXSTYLE);
            SetWindowLongPtrW(cover, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW);
        }
    }

    /// The cover over `r`, on top, shown without activation.
    pub fn place(cover: Hwnd, r: Rect) {
        // SAFETY: moving and showing our own window
        unsafe {
            SetWindowPos(cover, HWND_TOPMOST, r.x, r.y, r.w, r.h, SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOOWNERZORDER);
        }
    }

    pub fn hide(cover: Hwnd) {
        // SAFETY: hiding our own window
        unsafe { ShowWindow(cover, SW_HIDE) };
    }

    /// The game window in front (restored if it was minimised).
    pub fn front(game: Hwnd) {
        // SAFETY: the game's own window, to the front
        unsafe {
            if IsIconic(game) != 0 {
                ShowWindow(game, SW_RESTORE);
            }
            SetForegroundWindow(game);
        }
    }

    /// The real screen: the game by its process id, the cover by its window handle.
    pub struct Desktop {
        pub pid: u32,
        pub cover: Hwnd,
    }

    impl super::Screen for Desktop {
        fn game(&mut self) -> (bool, bool, Option<Rect>) {
            let alive = crate::denclick::win::alive(self.pid);
            match crate::denclick::win::window(self.pid) {
                Some(h) => (alive, true, client_rect(h)),
                None => (alive, false, None),
            }
        }
        fn net(&mut self) -> Net {
            net(self.pid)
        }
        fn esc(&mut self) -> bool {
            esc()
        }
        fn show(&mut self, r: Rect) {
            place(self.cover, r)
        }
        fn park(&mut self) {
            hide(self.cover)
        }
        fn hide(&mut self, front_game: bool) {
            hide(self.cover);
            if front_game {
                if let Some(h) = crate::denclick::win::window(self.pid) {
                    front(h);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect { x: 100, y: 80, w: 1280, h: 720 };

    fn seen(present: bool) -> Seen {
        Seen { present, window: present.then_some(R), ..Default::default() }
    }

    #[test]
    fn the_games_picture_from_its_corners() {
        assert_eq!(from_corners((100, 80), (1380, 800)), Some(R));
        // a game without DPI awareness on a 150% screen: the corners come back in the launcher's (physical) pixels
        assert_eq!(from_corners((150, 120), (2070, 1200)), Some(Rect { x: 150, y: 120, w: 1920, h: 1080 }));
        assert_eq!(from_corners((-32000, -32000), (-31840, -31972)), None, "minimised");
        assert_eq!(from_corners((0, 0), (100, 600)), None, "too small to be the game");
        assert_eq!(from_corners((500, 500), (400, 400)), None, "upside down");
        // a second screen to the left has negative coordinates, which are fine
        assert_eq!(from_corners((-1920, 0), (0, 1080)), Some(Rect { x: -1920, y: 0, w: 1920, h: 1080 }));
    }

    #[test]
    fn the_games_connections() {
        assert_eq!(classify(&[]), Net::None);
        assert_eq!(classify(&[3724]), Net::Auth);
        assert_eq!(classify(&[3724, 443]), Net::Auth, "the web is not a realm");
        assert_eq!(classify(&[8085]), Net::World);
        assert_eq!(classify(&[3724, 8086]), Net::World);
    }

    #[test]
    fn titles_and_steps() {
        assert_eq!(title(Mode::Official, "ORION"), "Logging you in as Orion…");
        assert_eq!(title(Mode::Official, "Orion"), "Logging you in as Orion…");
        assert_eq!(title(Mode::Official, "WR000123"), "Logging you in as WR000123…");
        assert_eq!(title(Mode::Official, "  "), "Logging you in…");
        assert_eq!(title(Mode::Official, "<b>x</b>"), "Logging you in as bxb…", "only a name's letters");
        assert_eq!(title(Mode::Den, "DEN"), "Opening your Den…");
        assert_eq!(steps(Mode::Official), ["Opening the game", "Signing in", "Choosing your realm", "Character select"]);
        assert_eq!(steps(Mode::Den)[3], "Into your Den");
        for s in steps(Mode::Official).iter().chain(&[title(Mode::Official, "x")]) {
            for w in ["World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard"] {
                assert!(!s.contains(w), "{s}");
            }
        }
    }

    #[test]
    fn official_window_keys_world_connection_then_it_hides() {
        let mut w = Watch::new(Mode::Official, 0);
        assert_eq!(w.tick(500, &seen(false)), Do::Nothing, "no window yet: nothing shown");
        assert_eq!(w.tick(1_000, &seen(true)), Do::Show { rect: R, step: 0 }, "the window: covered at once, before the keys");
        let mut s = seen(true);
        s.keys = Keys::Sent;
        assert_eq!(w.tick(8_000, &s), Do::Show { rect: R, step: 1 }, "signing in");
        s.net = Net::Auth;
        assert_eq!(w.tick(8_400, &s), Do::Show { rect: R, step: 2 });
        // the game leaves the login server for the world server
        s.net = Net::None;
        assert_eq!(w.tick(8_600, &s), Do::Show { rect: R, step: 2 }, "a moment between the two connections");
        s.net = Net::World;
        assert_eq!(w.tick(8_800, &s), Do::Show { rect: R, step: 3 }, "character select is drawing");
        assert_eq!(w.tick(9_800, &s), Do::Hide(End::Ready));
        assert_eq!(w.tick(9_900, &s), Do::Hide(End::Ready), "stays gone");
        let log = log_text(Mode::Official, &End::Ready, &w.log);
        assert!(log.contains("world server") && log.contains("end: Ready"), "{log}");
    }

    #[test]
    fn it_follows_the_window_and_steps_aside_when_minimised() {
        let mut w = Watch::new(Mode::Official, 0);
        w.tick(1_000, &seen(true));
        let mut s = seen(true);
        s.window = Some(Rect { x: 300, ..R });
        assert_eq!(w.tick(1_200, &s), Do::Show { rect: Rect { x: 300, ..R }, step: 0 });
        s.window = None; // minimised: still open
        assert_eq!(w.tick(1_400, &s), Do::Park);
        s.window = Some(R);
        assert_eq!(w.tick(1_600, &s), Do::Show { rect: R, step: 0 });
    }

    #[test]
    fn every_other_way_hides_it_at_once() {
        // the game closes
        let mut w = Watch::new(Mode::Official, 0);
        w.tick(1_000, &seen(true));
        assert_eq!(w.tick(2_000, &Seen { alive: false, ..seen(true) }), Do::Hide(End::GameClosed));
        // the keys didn't reach the game
        let mut w = Watch::new(Mode::Official, 0);
        w.tick(1_000, &seen(true));
        assert_eq!(w.tick(8_000, &Seen { keys: Keys::Failed, ..seen(true) }), Do::Hide(End::Failed));
        // Esc / Show the game
        let mut w = Watch::new(Mode::Official, 0);
        w.tick(1_000, &seen(true));
        assert_eq!(w.tick(3_000, &Seen { dismissed: true, ..seen(true) }), Do::Hide(End::Dismissed));
        // a wrong password: the login server answers and the connection closes, no world
        let mut w = Watch::new(Mode::Official, 0);
        w.tick(1_000, &seen(true));
        w.tick(8_000, &Seen { keys: Keys::Sent, net: Net::Auth, ..seen(true) });
        let gone = Seen { keys: Keys::Sent, ..seen(true) };
        assert!(matches!(w.tick(8_500, &gone), Do::Show { .. }));
        assert_eq!(w.tick(10_100, &gone), Do::Hide(End::LoginAnswered));
        // logged in, but the realm list waits for the player
        let mut w = Watch::new(Mode::Official, 0);
        w.tick(1_000, &seen(true));
        let auth = Seen { keys: Keys::Sent, net: Net::Auth, ..seen(true) };
        w.tick(8_000, &auth);
        assert!(matches!(w.tick(15_000, &auth), Do::Show { step: 2, .. }));
        assert_eq!(w.tick(16_100, &auth), Do::Hide(End::RealmList));
        // nothing at all happens: the time limit
        let mut w = Watch::new(Mode::Official, 0);
        w.tick(1_000, &seen(true));
        assert!(matches!(w.tick(25_000, &seen(true)), Do::Show { .. }));
        assert_eq!(w.tick(26_100, &seen(true)), Do::Hide(End::Timeout));
        // the window never opens
        let mut w = Watch::new(Mode::Official, 0);
        assert_eq!(w.tick(59_000, &seen(false)), Do::Nothing);
        assert_eq!(w.tick(61_000, &seen(false)), Do::Hide(End::NoWindow));
        // an unrelated connection before the login is not the world
        let mut w = Watch::new(Mode::Official, 0);
        w.tick(1_000, &seen(true));
        assert!(matches!(w.tick(2_000, &Seen { net: Net::World, ..seen(true) }), Do::Show { step: 0, .. }));
        assert!(matches!(w.tick(4_000, &Seen { net: Net::World, ..seen(true) }), Do::Show { .. }));
    }

    fn ev(t: u64, name: &str) -> crate::denclick::Ev {
        crate::denclick::Ev { t, ev: name.into(), who: "DEN".into(), detail: String::new() }
    }

    #[test]
    fn the_den_follows_den_lites_milestones_into_the_loading_screen() {
        assert_eq!(den_stage(&[]), DenStage::None);
        assert_eq!(den_stage(&[ev(1, "login_challenge"), ev(2, "auth_fail")]), DenStage::Asked);
        assert_eq!(den_stage(&[ev(1, "auth_ok"), ev(2, "char_enum")]), DenStage::CharSelect);
        assert_eq!(den_stage(&[ev(1, "char_enum"), ev(2, "player_login")]), DenStage::Entering);
        let mut w = Watch::new(Mode::Den, 0);
        assert_eq!(w.tick(1_000, &seen(true)), Do::Show { rect: R, step: 0 });
        assert_eq!(w.tick(6_000, &Seen { den: DenStage::Asked, ..seen(true) }), Do::Show { rect: R, step: 1 });
        assert_eq!(w.tick(7_000, &Seen { den: DenStage::Realm, ..seen(true) }), Do::Show { rect: R, step: 2 });
        assert_eq!(w.tick(9_000, &Seen { den: DenStage::CharSelect, ..seen(true) }), Do::Show { rect: R, step: 3 });
        assert!(matches!(w.tick(30_000, &Seen { den: DenStage::CharSelect, ..seen(true) }), Do::Show { .. }), "the Den gets longer");
        assert_eq!(w.tick(31_000, &Seen { den: DenStage::Entering, ..seen(true) }), Do::Hide(End::Ready));
        // the Den's sign-in stopped: gone at once
        let mut w = Watch::new(Mode::Den, 0);
        w.tick(1_000, &seen(true));
        assert_eq!(w.tick(2_000, &Seen { finished: Some(false), ..seen(true) }), Do::Hide(End::Failed));
        // the Den doesn't look at connections
        let mut w = Watch::new(Mode::Den, 0);
        w.tick(1_000, &seen(true));
        assert!(matches!(w.tick(5_000, &Seen { net: Net::World, keys: Keys::Sent, ..seen(true) }), Do::Show { step: 0, .. }));
    }

    /// A fake screen for `run`: a game whose window opens, whose keys go, and whose connections follow a script.
    struct Fake {
        t: std::rc::Rc<std::cell::Cell<u64>>,
        script: Vec<(u64, Net)>,
        alive_until: u64,
        esc_at: Option<u64>,
        shows: Vec<Rect>,
        parks: u32,
        hidden: Option<bool>,
    }

    impl Screen for Fake {
        fn game(&mut self) -> (bool, bool, Option<Rect>) {
            let t = self.t.get();
            let present = t >= 1_000;
            (t < self.alive_until, present, present.then_some(R))
        }
        fn net(&mut self) -> Net {
            let t = self.t.get();
            self.script.iter().rev().find(|(at, _)| t >= *at).map(|(_, n)| *n).unwrap_or(Net::None)
        }
        fn esc(&mut self) -> bool {
            self.esc_at.is_some_and(|e| self.t.get() >= e)
        }
        fn show(&mut self, r: Rect) {
            self.shows.push(r);
        }
        fn park(&mut self) {
            self.parks += 1;
        }
        fn hide(&mut self, front: bool) {
            self.hidden = Some(front);
        }
    }

    fn fake(script: Vec<(u64, Net)>) -> Fake {
        Fake { t: Default::default(), script, alive_until: u64::MAX, esc_at: None, shows: vec![], parks: 0, hidden: None }
    }

    // the page's state is one for the whole launcher: the runs one at a time
    static ONE: Mutex<()> = Mutex::new(());

    #[test]
    fn run_covers_a_whole_official_start_and_puts_the_game_in_front() {
        let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let mut f = fake(vec![(8_200, Net::Auth), (9_000, Net::None), (9_200, Net::World)]);
        let t = f.t.clone();
        let sig = Signals::default();
        let id = begin(Mode::Official, title(Mode::Official, "ORION"));
        let (end, log) = run(&mut f, Watch::new(Mode::Official, 0), &sig, None, id, &mut || t.get(), &mut |ms| {
            t.set(t.get() + ms);
            if t.get() >= 8_000 {
                sig.keys_sent();
            }
        });
        assert_eq!(end, End::Ready);
        assert_eq!(f.shows, vec![R], "moved once, not every look");
        assert_eq!(f.hidden, Some(true), "the game in front after");
        assert!(t.get() >= 10_100 && t.get() < 11_000, "hidden right after the world connection: {}", t.get());
        assert!(log.iter().any(|(_, a)| a == "step: Character select"));
        let v = view();
        assert_eq!((v.id, v.shown, v.step, v.title.as_str()), (id, false, 3, "Logging you in as Orion…"));
    }

    #[test]
    fn run_hides_on_esc_on_a_closed_game_and_when_a_newer_start_takes_over() {
        let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
        // Esc while it is up
        let mut f = fake(vec![]);
        f.esc_at = Some(3_000);
        let t = f.t.clone();
        let id = begin(Mode::Official, "x".into());
        let (end, _) = run(&mut f, Watch::new(Mode::Official, 0), &Signals::default(), None, id, &mut || t.get(), &mut |ms| t.set(t.get() + ms));
        assert_eq!((end, f.hidden), (End::Dismissed, Some(true)));
        assert!(t.get() < 3_400);
        // "Show the game" on the page
        let mut f = fake(vec![]);
        let t = f.t.clone();
        let id = begin(Mode::Official, "x".into());
        let (end, _) = run(&mut f, Watch::new(Mode::Official, 0), &Signals::default(), None, id, &mut || t.get(), &mut |ms| {
            t.set(t.get() + ms);
            if t.get() == 2_000 {
                dismiss(0);
            }
        });
        assert_eq!(end, End::Dismissed);
        // the game closes: hidden, and nothing put in front
        let mut f = fake(vec![]);
        f.alive_until = 4_000;
        let t = f.t.clone();
        let id = begin(Mode::Official, "x".into());
        let (end, _) = run(&mut f, Watch::new(Mode::Official, 0), &Signals::default(), None, id, &mut || t.get(), &mut |ms| t.set(t.get() + ms));
        assert_eq!((end, f.hidden), (End::GameClosed, Some(false)));
        // a newer start: the older cover stops, leaving the window to the newer one and its game where it is
        let mut f = fake(vec![]);
        let t = f.t.clone();
        let id = begin(Mode::Official, "x".into());
        let (end, _) = run(&mut f, Watch::new(Mode::Official, 0), &Signals::default(), None, id, &mut || t.get(), &mut |ms| {
            t.set(t.get() + ms);
            if t.get() == 2_000 {
                begin(Mode::Den, "y".into());
            }
        });
        assert_eq!((end, f.hidden), (End::Dismissed, None));
    }

    #[test]
    fn run_follows_den_lites_log() {
        let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let mut f = fake(vec![]);
        let t = f.t.clone();
        let id = begin(Mode::Den, title(Mode::Den, "DEN"));
        let d = dir.path().to_path_buf();
        let (end, log) = run(&mut f, Watch::new(Mode::Den, 0), &Signals::default(), Some((dir.path(), 0)), id, &mut || t.get(), &mut |ms| {
            t.set(t.get() + ms);
            let line = match t.get() {
                6_000 => "login_challenge",
                7_000 => "auth_ok",
                9_000 => "char_enum",
                12_000 => "player_login",
                _ => return,
            };
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(d.join(crate::denclick::EVENTS)).unwrap();
            writeln!(file, "{}", serde_json::json!({"t": t.get(), "ev": line})).unwrap();
        });
        assert_eq!(end, End::Ready);
        assert!(t.get() >= 12_000 && t.get() < 12_500);
        assert!(log.iter().any(|(_, a)| a == "step: Into your Den"), "{log:?}");
    }
}
