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
//! * single player (owner 12 Nov: "the login overlay for single player and the Den too"): the launcher's keys at a
//!   stock game's login screen (crate::host::localclick) or Leyline's login screen code (crate::hybrid) on this PC's
//!   own world; hidden when the world's database says the character is in the world ([`Signals::set_stage`], read
//!   by localclick from `account.online` and the characters' `online`), or at character select when no character
//!   is entered;
//! * hidden at once when anything goes another way: the game closes, the keys didn't reach it, the login server
//!   answered without a world connection (a wrong password, the realm list waiting for the player), Esc, the
//!   cover's "Show the game", or the time limit ([`Watch::max_ms`]). The player is never stranded behind it. When it
//!   was up and the sign-in didn't get through (the keys failed, the time limit, the login screen answered), it says
//!   so in one short line for [`ERROR_HOLD`] first ([`end_line`]), then goes.
//!
//! The cover never takes the keyboard (Windows: WS_EX_NOACTIVATE, shown without activation); the keys are posted
//! window messages to the game's own window (denclick::win), which need no focus, and where the driver brings the
//! game to the front (SetForegroundWindow, for the create screen's name box) the cover can't be what is in front.
//! The mouse goes through it to the game (WS_EX_LAYERED | WS_EX_TRANSPARENT) except over its "Show the game" link:
//! the page says where the link is ([`set_link`]) and the cover takes clicks only while the pointer is on it
//! ([`hit`]). When it hides, the game window is brought to the front.
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
/// Single player: the longest (the world's database is read once a second; a first entry may make the character on
/// the game's own create screen: localclick CREATE_FIRST and a retry).
pub const MAX_SINGLE: u64 = 60_000;
/// How long the cover says why it is going (a failed sign-in, the time limit) before it goes (ms).
pub const ERROR_HOLD: u64 = 2_400;
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
/// Around the "Show the game" link, the pointer still counts as on it (physical pixels).
pub const LINK_PAD: i32 = 6;

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

/// The page's "Show the game" link in the cover's own pixels: its CSS box (from getBoundingClientRect) times the
/// page's devicePixelRatio, a little bigger all round. None for a box that can't be one.
pub fn link_from_css(x: f64, y: f64, w: f64, h: f64, scale: f64) -> Option<Rect> {
    let ok = |v: f64| v.is_finite() && v.abs() < 100_000.0;
    if ![x, y, w, h, scale].iter().all(|v| ok(*v)) || w <= 0.0 || h <= 0.0 || scale <= 0.0 {
        return None;
    }
    let (x0, y0) = ((x * scale).floor() as i32 - LINK_PAD, (y * scale).floor() as i32 - LINK_PAD);
    let (x1, y1) = (((x + w) * scale).ceil() as i32 + LINK_PAD, ((y + h) * scale).ceil() as i32 + LINK_PAD);
    Some(Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
}

/// The pointer (screen pixels) is on the link (`link`: in the cover's own pixels; `cover`: where the cover is).
pub fn hit(link: Rect, cover: Rect, cursor: (i32, i32)) -> bool {
    let (cx, cy) = (cursor.0 - cover.x, cursor.1 - cover.y);
    cx >= 0 && cy >= 0 && cx < cover.w && cy < cover.h && cx >= link.x && cy >= link.y && cx < link.x + link.w && cy < link.y + link.h
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
    /// Single player on this PC's own world (app.rs world_play_now: crate::host::localclick's keys or our login
    /// screen code): into the world, or character select when no character is entered.
    Single,
}

/// The steps the cover shows, in order.
pub fn steps(mode: Mode) -> Vec<String> {
    let last = match mode {
        Mode::Official => "Character select",
        Mode::Den => "Into your Den",
        Mode::Single => return ["Starting your world", "Signing in", "Entering the world"].iter().map(|s| s.to_string()).collect(),
    };
    ["Opening the game", "Signing in", "Choosing your realm", last].iter().map(|s| s.to_string()).collect()
}

/// The cover's heading: "Logging you in as Orion…" (an account name in capitals, as the game keeps it, is shown as a
/// name; a website game login like WR000123 as it is), or for the Den "Opening your Den…".
pub fn title(mode: Mode, name: &str) -> String {
    match mode {
        Mode::Den => return "Opening your Den…".into(),
        // the world's own login (a Hero Key id or SOLO), not a name to greet
        Mode::Single => return "Logging you in…".into(),
        Mode::Official => {}
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

/// Your Den: where Den Lite's milestone log (crate::denclick::EVENTS) says the start is. Single player: the same
/// milestones, from the world's database (crate::host::localclick, [`Signals::set_stage`]).
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
            Mode::Single => MAX_SINGLE,
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
            // the world's database: logged in, character select (the character is being entered), in the world
            Mode::Single => match s.den {
                DenStage::Entering => return self.finish(now, End::Ready),
                DenStage::CharSelect => self.to_step(now, 2),
                DenStage::Realm | DenStage::Asked => self.to_step(now, 1),
                DenStage::None => {
                    if s.keys == Keys::Sent {
                        self.to_step(now, 1);
                    }
                }
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
    stage: AtomicU8,
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
    /// Single player: how far the sign-in is (crate::host::localclick, from the world's database). Only ever moves on.
    pub fn set_stage(&self, s: DenStage) {
        self.stage.fetch_max(s as u8, Ordering::Relaxed);
    }
    pub fn stage(&self) -> DenStage {
        match self.stage.load(Ordering::Relaxed) {
            1 => DenStage::Asked,
            2 => DenStage::Realm,
            3 => DenStage::CharSelect,
            4 => DenStage::Entering,
            _ => DenStage::None,
        }
    }
}

/// The one short line the cover shows before it goes when it was up and the sign-in didn't get through; None: it
/// goes at once (done, the player's own Esc, the game closed, nothing was shown).
pub fn end_line(mode: Mode, end: &End) -> Option<&'static str> {
    match end {
        End::Ready | End::Dismissed | End::GameClosed | End::NoWindow => None,
        End::Failed => Some(match mode {
            Mode::Single => "Leyline couldn't log you in by itself this time. The game is yours from here.",
            _ => "Leyline couldn't finish logging you in. The game is yours from here.",
        }),
        End::Timeout => Some("This is taking longer than usual, so here's the game."),
        End::LoginAnswered => Some("The login screen has a message for you."),
        End::RealmList => Some("Pick your realm in the game."),
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
    /// Why it is going (empty while the sign-in goes on): [`end_line`].
    pub error: String,
}

static VIEW: Mutex<View> = Mutex::new(View { id: 0, title: String::new(), steps: Vec::new(), step: 0, shown: false, error: String::new() });
static NEXT: AtomicU64 = AtomicU64::new(1);
static LINK: Mutex<(u64, Option<Rect>)> = Mutex::new((0, None));
static DISMISSED: AtomicU64 = AtomicU64::new(0);

/// A new start's cover: its number (a newer start takes the page over; an older one then stops).
pub fn begin(mode: Mode, title: String) -> u64 {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut v) = VIEW.lock() {
        *v = View { id, title, steps: steps(mode), step: 0, shown: false, error: String::new() };
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

fn set_error(id: u64, line: &str) {
    if let Ok(mut v) = VIEW.lock() {
        if v.id == id {
            v.error = line.to_string();
        }
    }
}

/// The page's "Show the game" link for this start (app.rs `cover_link`; [`link_from_css`]).
pub fn set_link(id: u64, r: Option<Rect>) {
    if let Ok(mut l) = LINK.lock() {
        *l = (id, r);
    }
}

fn link(id: u64) -> Option<Rect> {
    LINK.lock().ok().and_then(|l| if l.0 == id { l.1 } else { None })
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
    /// The pointer on the screen.
    fn cursor(&mut self) -> Option<(i32, i32)>;
    /// The cover takes clicks (true: the pointer is on its link) or lets them through to the game (false).
    fn clickable(&mut self, on: bool);
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
    let mut clicks = false;
    loop {
        let t = now();
        let (alive, present, window) = screen.game();
        let net = if w.mode == Mode::Official && present { screen.net() } else { Net::None };
        let den_stage = match den {
            Some((dir, since)) if w.mode == Mode::Den => den_stage(&crate::denclick::events_since(dir, since)),
            _ if w.mode == Mode::Single => sig.stage(),
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
                // clicks only on "Show the game"; everywhere else they go through to the game
                let on = match (link(id), screen.cursor()) {
                    (Some(l), Some(c)) => hit(l, rect, c),
                    _ => false,
                };
                if on != clicks {
                    screen.clickable(on);
                    clicks = on;
                }
                set_view(id, true, step);
            }
            Do::Hide(e) => {
                // up, and the sign-in didn't get through: one line saying so first (Esc, Show the game, a closed
                // game or a newer start cut it short)
                if let (Some(line), Some(_), false) = (end_line(w.mode, &e), last, replaced) {
                    set_error(id, line);
                    let until = now().saturating_add(ERROR_HOLD);
                    while now() < until {
                        sleep(POLL_MS);
                        let (alive, ..) = screen.game();
                        if !alive || dismissed(id) || current() != id || screen.esc() {
                            break;
                        }
                    }
                }
                let replaced = current() != id;
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
    const WS_EX_TRANSPARENT: isize = 0x0000_0020;
    const WS_EX_LAYERED: isize = 0x0008_0000;
    const LWA_ALPHA: u32 = 0x0000_0002;
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
        fn GetCursorPos(p: *mut Pt) -> i32;
        fn SetLayeredWindowAttributes(h: Hwnd, key: u32, alpha: u8, flags: u32) -> i32;
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

    /// The cover never takes the keyboard (WS_EX_NOACTIVATE: not by a click either), stays off the taskbar and
    /// Alt+Tab (WS_EX_TOOLWINDOW), and lets the mouse through to the game (WS_EX_LAYERED | WS_EX_TRANSPARENT, fully
    /// opaque) until the pointer is on its link ([`clickable`]).
    pub fn prepare(cover: Hwnd) {
        // SAFETY: changing our own window's extended style; a layered window draws only once its alpha is set
        unsafe {
            let ex = GetWindowLongPtrW(cover, GWL_EXSTYLE);
            SetWindowLongPtrW(cover, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT);
            SetLayeredWindowAttributes(cover, 0, 255, LWA_ALPHA);
        }
    }

    /// Clicks on the cover (the pointer on "Show the game") or through it to the game. Never activates it.
    pub fn clickable(cover: Hwnd, on: bool) {
        // SAFETY: changing our own window's extended style
        unsafe {
            let ex = GetWindowLongPtrW(cover, GWL_EXSTYLE) | WS_EX_NOACTIVATE | WS_EX_LAYERED;
            SetWindowLongPtrW(cover, GWL_EXSTYLE, if on { ex & !WS_EX_TRANSPARENT } else { ex | WS_EX_TRANSPARENT });
        }
    }

    /// The pointer on the screen (physical pixels: the launcher is DPI aware, as client_rect's corners are).
    pub fn cursor() -> Option<(i32, i32)> {
        let mut p = Pt::default();
        // SAFETY: a pointer position query into our own struct
        (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
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
        fn cursor(&mut self) -> Option<(i32, i32)> {
            cursor()
        }
        fn clickable(&mut self, on: bool) {
            clickable(self.cover, on)
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
    fn the_link_in_the_covers_pixels_and_the_pointer_on_it() {
        // a 150% screen: the CSS box times 1.5, padded
        let l = link_from_css(20.0, 400.0, 100.0, 30.0, 1.5).unwrap();
        assert_eq!(l, Rect { x: 30 - LINK_PAD, y: 600 - LINK_PAD, w: 150 + 2 * LINK_PAD, h: 45 + 2 * LINK_PAD });
        for bad in [(0.0, 0.0, 0.0, 10.0, 1.0), (0.0, 0.0, 10.0, 10.0, 0.0), (f64::NAN, 0.0, 10.0, 10.0, 1.0), (0.0, 0.0, f64::INFINITY, 10.0, 1.0)] {
            assert_eq!(link_from_css(bad.0, bad.1, bad.2, bad.3, bad.4), None, "{bad:?}");
        }
        let link = Rect { x: 24, y: 640, w: 160, h: 40 };
        // the cover over a game on a second screen to the left
        let cover = Rect { x: -1920, y: 0, w: 1920, h: 1080 };
        assert!(hit(link, cover, (-1920 + 30, 650)));
        assert!(hit(link, cover, (-1920 + 24, 640)), "its corner");
        assert!(!hit(link, cover, (-1920 + 184, 650)), "just past it");
        assert!(!hit(link, cover, (30, 650)), "the same spot on the other screen");
        assert!(!hit(link, cover, (-1920 + 30, 1200)), "off the cover");
        assert!(!hit(Rect { x: 1900, y: 0, w: 100, h: 40 }, cover, (5, 10)), "a link past the cover's edge only counts on the cover");
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

    #[test]
    fn single_player_follows_the_worlds_database_into_the_world() {
        assert_eq!(title(Mode::Single, "SOLO"), "Logging you in…", "the world's login is not a name to greet");
        assert_eq!(steps(Mode::Single), ["Starting your world", "Signing in", "Entering the world"]);
        let mut w = Watch::new(Mode::Single, 0);
        assert_eq!(w.tick(500, &seen(false)), Do::Nothing);
        assert_eq!(w.tick(1_000, &seen(true)), Do::Show { rect: R, step: 0 }, "covered as soon as the window opens");
        assert_eq!(w.tick(7_000, &Seen { keys: Keys::Sent, ..seen(true) }), Do::Show { rect: R, step: 1 }, "the keys went");
        assert_eq!(w.tick(8_000, &Seen { den: DenStage::Realm, ..seen(true) }), Do::Show { rect: R, step: 1 });
        assert_eq!(w.tick(9_000, &Seen { den: DenStage::CharSelect, ..seen(true) }), Do::Show { rect: R, step: 2 });
        assert!(matches!(w.tick(40_000, &Seen { den: DenStage::CharSelect, ..seen(true) }), Do::Show { .. }), "a first entry may make the character");
        assert_eq!(w.tick(41_000, &Seen { den: DenStage::Entering, ..seen(true) }), Do::Hide(End::Ready));
        // character select was the goal (no character entered): the follower says it's done
        let mut w = Watch::new(Mode::Single, 0);
        w.tick(1_000, &seen(true));
        assert_eq!(w.tick(9_000, &Seen { den: DenStage::CharSelect, finished: Some(true), ..seen(true) }), Do::Hide(End::Ready));
        // the keys didn't reach the game, the follower gave up, the time limit
        let mut w = Watch::new(Mode::Single, 0);
        w.tick(1_000, &seen(true));
        assert_eq!(w.tick(8_000, &Seen { keys: Keys::Failed, ..seen(true) }), Do::Hide(End::Failed));
        let mut w = Watch::new(Mode::Single, 0);
        w.tick(1_000, &seen(true));
        assert_eq!(w.tick(8_000, &Seen { finished: Some(false), ..seen(true) }), Do::Hide(End::Failed));
        let mut w = Watch::new(Mode::Single, 0);
        w.tick(1_000, &seen(true));
        assert!(matches!(w.tick(1_000 + MAX_SINGLE, &seen(true)), Do::Show { .. }));
        assert_eq!(w.tick(1_100 + MAX_SINGLE, &seen(true)), Do::Hide(End::Timeout));
        // single player never looks at connections
        let mut w = Watch::new(Mode::Single, 0);
        w.tick(1_000, &seen(true));
        assert!(matches!(w.tick(30_000, &Seen { net: Net::World, ..seen(true) }), Do::Show { step: 0, .. }));
    }

    #[test]
    fn the_stage_only_moves_on_and_the_end_lines_are_plain() {
        let sig = Signals::default();
        assert_eq!(sig.stage(), DenStage::None);
        sig.set_stage(DenStage::CharSelect);
        sig.set_stage(DenStage::Asked);
        assert_eq!(sig.stage(), DenStage::CharSelect, "an older read never takes it back");
        sig.set_stage(DenStage::Entering);
        assert_eq!(sig.stage(), DenStage::Entering);
        for mode in [Mode::Official, Mode::Den, Mode::Single] {
            for e in [End::Ready, End::Dismissed, End::GameClosed, End::NoWindow] {
                assert_eq!(end_line(mode, &e), None, "{e:?} goes at once");
            }
            for e in [End::Failed, End::Timeout, End::LoginAnswered, End::RealmList] {
                let l = end_line(mode, &e).expect("a line");
                assert!(l.len() < 100, "short: {l}");
                for w in ["World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard", "Sign in", "sign in", "password"] {
                    assert!(!l.contains(w), "{l}");
                }
            }
        }
        for s in steps(Mode::Single) {
            assert!(!s.contains("WoW") && !s.contains("Warcraft"), "{s}");
        }
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
        pointer: Vec<(u64, (i32, i32))>,
        clicks: Vec<bool>,
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
        fn cursor(&mut self) -> Option<(i32, i32)> {
            let t = self.t.get();
            self.pointer.iter().rev().find(|(at, _)| t >= *at).map(|(_, p)| *p)
        }
        fn clickable(&mut self, on: bool) {
            self.clicks.push(on);
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
        Fake { t: Default::default(), script, alive_until: u64::MAX, esc_at: None, shows: vec![], parks: 0, hidden: None, pointer: vec![], clicks: vec![] }
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

    #[test]
    fn run_follows_single_players_signals_into_the_world() {
        let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let mut f = fake(vec![]);
        let t = f.t.clone();
        let sig = Signals::default();
        let id = begin(Mode::Single, title(Mode::Single, "SOLO"));
        let (end, log) = run(&mut f, Watch::new(Mode::Single, 0), &sig, None, id, &mut || t.get(), &mut |ms| {
            t.set(t.get() + ms);
            match t.get() {
                7_000 => sig.keys_sent(),
                8_000 => sig.set_stage(DenStage::Realm),
                9_000 => sig.set_stage(DenStage::CharSelect),
                12_000 => sig.set_stage(DenStage::Entering),
                _ => {}
            }
        });
        assert_eq!(end, End::Ready);
        assert!(t.get() >= 12_000 && t.get() < 12_500, "gone as soon as the character is in the world: {}", t.get());
        assert_eq!(f.hidden, Some(true));
        assert!(log.iter().any(|(_, a)| a == "step: Entering the world"), "{log:?}");
        assert_eq!(view().error, "", "no line when it went through");
    }

    #[test]
    fn run_says_why_for_a_moment_when_the_sign_in_didnt_get_through() {
        let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
        // the keys failed while it was up: the line, held, then hidden with the game in front
        let mut f = fake(vec![]);
        let t = f.t.clone();
        let sig = Signals::default();
        let id = begin(Mode::Single, title(Mode::Single, ""));
        let mut seen_line = String::new();
        let (end, _) = run(&mut f, Watch::new(Mode::Single, 0), &sig, None, id, &mut || t.get(), &mut |ms| {
            t.set(t.get() + ms);
            if t.get() == 8_000 {
                sig.keys_failed();
            }
            if t.get() == 9_000 {
                seen_line = view().error;
            }
        });
        assert_eq!(end, End::Failed);
        assert_eq!(seen_line, end_line(Mode::Single, &End::Failed).unwrap());
        assert!(t.get() >= 8_000 + ERROR_HOLD && t.get() < 8_000 + ERROR_HOLD + 600, "held, then gone: {}", t.get());
        assert_eq!(f.hidden, Some(true));
        // Esc cuts the line short
        let mut f = fake(vec![]);
        f.esc_at = Some(8_600);
        let t = f.t.clone();
        let sig = Signals::default();
        let id = begin(Mode::Single, title(Mode::Single, ""));
        let (end, _) = run(&mut f, Watch::new(Mode::Single, 0), &sig, None, id, &mut || t.get(), &mut |ms| {
            t.set(t.get() + ms);
            if t.get() == 8_000 {
                sig.keys_failed();
            }
        });
        assert_eq!(end, End::Failed);
        assert!(t.get() < 9_000, "{}", t.get());
        // failed before the window opened: nothing was shown, nothing to say
        let mut f = fake(vec![]);
        let t = f.t.clone();
        let sig = Signals::default();
        sig.keys_failed();
        let id = begin(Mode::Single, title(Mode::Single, ""));
        let (end, _) = run(&mut f, Watch::new(Mode::Single, 0), &sig, None, id, &mut || t.get(), &mut |ms| t.set(t.get() + ms));
        assert_eq!(end, End::Failed);
        assert!(t.get() < 500, "{}", t.get());
        assert_eq!(view().error, "");
    }

    #[test]
    fn run_takes_clicks_only_while_the_pointer_is_on_the_link() {
        let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let mut f = fake(vec![]);
        // the cover is over R (100, 80); the link is at (24, 640) in the cover's own pixels
        f.pointer = vec![(0, (500, 500)), (2_000, (100 + 30, 80 + 650)), (3_000, (900, 300))];
        f.esc_at = Some(4_000);
        let t = f.t.clone();
        let id = begin(Mode::Single, title(Mode::Single, ""));
        set_link(id, Some(Rect { x: 24, y: 640, w: 160, h: 40 }));
        let (end, _) = run(&mut f, Watch::new(Mode::Single, 0), &Signals::default(), None, id, &mut || t.get(), &mut |ms| t.set(t.get() + ms));
        assert_eq!(end, End::Dismissed);
        assert_eq!(f.clicks, vec![true, false], "clickable on the link, through again off it; told only on a change");
        // another start's link doesn't count
        let mut f = fake(vec![]);
        f.pointer = vec![(0, (100 + 30, 80 + 650))];
        f.esc_at = Some(2_000);
        let t = f.t.clone();
        let old = begin(Mode::Single, "x".into());
        let id = begin(Mode::Single, "y".into());
        set_link(old, Some(Rect { x: 24, y: 640, w: 160, h: 40 }));
        run(&mut f, Watch::new(Mode::Single, 0), &Signals::default(), None, id, &mut || t.get(), &mut |ms| t.set(t.get() + ms));
        assert!(f.clicks.is_empty());
    }
}
