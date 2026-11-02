//! My Den, one click into the world on the player's own desktop game (owner, 1 Nov 16:39: "1 click My Den and we're
//! in: past the login screen, past character select or create, straight into the white island").
//!
//! Two ways in, and the launcher picks per start without asking the player:
//!
//! 1. **Our login screen code** (`glue/LeylineHybrid.lua` in `Data/patch-Z.MPQ`, crate::hybrid): signs in as
//!    [`GLUE_ACCOUNT`] with the one-start handoff, picks "Your Den", makes a character if there is none and enters the
//!    world. It loads only where the game accepts changed login screen files (a game folder with Forever UI's
//!    login screens in it today; a stock game program refuses changed login screen code: docs/HYBRID-CLIENT.md, T1).
//! 2. **The launcher's keys** ([`Driver`]): on a stock game (no login screen code) the launcher watches Den Lite's
//!    milestone log (`den-lite.events`, labs/den-lite/src/events.rs) and sends the game window exactly what a player
//!    would type, only for the step that didn't happen by itself: the local password `den` + Enter at the login
//!    screen (the account `DEN` is already filled in from Config.wtf), Enter at the realm list, Enter at character
//!    select (Den Lite's ready-made character is selected). Windows only (posted window messages, no focus taken).
//!
//! Both stop as soon as Den Lite says the player is in the world; every step has a timeout, and then the game is
//! left as it is (the normal screen), with the reason in `den-click.log`.
//!
//! While the game runs the launcher also answers what the game asks of it (`den-lite.ask`, labs/den-lite ovmod.rs):
//! "Invite a friend" (a link for one friend, back into the Den panel) and "Shape the ground" (Forge opens).
//!
//! In the owner's den-test (`den-play --test`) the asks are answered without side effects outside this PC: the invite
//! link is made without starting the relay tunnel and left for the test in [`INVITE_FILE`] (the test reads it, never
//! prints its secret, and removes it), and "Shape the ground" gets Forge's files ready without opening Forge
//! ([`FORGE_REPORT`]). The test types through Den Lite's own command files (labs/den-lite testctl.rs); only when that
//! Den Lite is too old does it ask the launcher to type a line into the game window ([`TYPE_FILE`], Windows only).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The account our login screen code signs in with (Den Lite takes any name with its local password), so Den
/// Lite's log shows which way the game came in (the launcher's keys use `DEN`, as Config.wtf fills in).
pub const GLUE_ACCOUNT: &str = "DENAUTO";
/// Den Lite's milestone log in its Dens folder (labs/den-lite/src/events.rs FILE).
pub const EVENTS: &str = "den-lite.events";
/// `<den-lite root>/den-click.log`: what the launcher did for the last start (times in ms from the start).
pub const LOG: &str = "den-click.log";
/// `<den-lite root>/den-click.json`: the last start's report (the `den-test` step reads it).
pub const REPORT: &str = "den-click.json";
/// In the launcher program, so tools/den-check can tell a launcher that has `den-play` from an older one.
pub const CLI_MARK: &str = "leyline-den-play-v1";
/// In the launcher program when den-play --test answers the asks as a test (FORGE_REPORT, INVITE_FILE, TYPE_FILE).
pub const TEST_MARK: &str = "leyline-den-test-v2";
/// `<den-lite root>/den-click.forge.json`: the test's "Shape the ground": Forge's files made, Forge not opened.
pub const FORGE_REPORT: &str = "den-click.forge.json";
/// `<den-lite root>/den-click.invite`: the test's invite link (`{"link": ...}`); the den-test removes it at once.
pub const INVITE_FILE: &str = "den-click.invite";
/// `<den-lite root>/den-click.type`: `{"id": n, "line": ".den place chair"}`, a chat line to type (the fallback).
pub const TYPE_FILE: &str = "den-click.type";
/// `<den-lite root>/den-click.typed`: `{"id": n, "ok": bool}`, after it was typed.
pub const TYPED_FILE: &str = "den-click.typed";

/// One line of Den Lite's log.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Ev {
    pub t: u64,
    pub ev: String,
    #[serde(default)]
    pub who: String,
    #[serde(default)]
    pub detail: String,
}

/// Den Lite's log lines from `since` (unix ms) on: this start's, not an earlier game's.
pub fn events_since(den_dir: &Path, since: u64) -> Vec<Ev> {
    std::fs::read_to_string(den_dir.join(EVENTS))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Ev>(l).ok())
        .filter(|e| e.t >= since)
        // a friend coming in through the tunnel is not this start
        .filter(|e| e.detail != "remote")
        .collect()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// What the launcher sends the game window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Key {
    /// Clear the password box, type the local password, Enter.
    Password,
    /// Enter (the realm list's OK, character select's Enter World).
    Enter,
    /// A short step forward (the test's "first movement").
    Forward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Wait,
    Send(Key),
    /// In the world.
    Done,
    GiveUp(String),
}

/// How the start ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Outcome {
    InWorld,
    /// The game closed before it asked to sign in, with our login screen code in: the game refused it.
    GlueRefused,
    Stopped(String),
}

/// Timings (ms). The login screen takes a few seconds after the window opens (longer on a first start).
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    pub first_password: u64,
    pub first_password_glue: u64,
    pub retry: u64,
    pub realm_wait: u64,
    pub enter_wait: u64,
    pub enter_wait_glue: u64,
    pub window_max: u64,
    pub total_max: u64,
}

/// The test's step: after the Leyline UI answers, and again if no movement shows.
const MOVE_SETTLE: u64 = 2_500;
const MOVE_NO_UI: u64 = 8_000;
const MOVE_RETRY: u64 = 3_000;
const MOVE_TRIES: u32 = 3;

pub const TIMING: Timing = Timing {
    first_password: 5_000,
    first_password_glue: 20_000,
    retry: 4_500,
    realm_wait: 6_000,
    enter_wait: 2_500,
    enter_wait_glue: 5_000,
    window_max: 90_000,
    total_max: 150_000,
};

/// The launcher's side of the sign-in: from Den Lite's log and whether the game's window is there, the next key
/// (or none). Pure, so every path is tested without a game.
#[derive(Debug, Clone)]
pub struct Driver {
    pub glue: bool,
    pub test: bool,
    pub start: u64,
    pub t: Timing,
    window_at: Option<u64>,
    last_send: u64,
    pub passwords: u32,
    pub realm_enters: u32,
    pub enters: u32,
    moves: u32,
    /// What it did, for the log: (ms from start, what).
    pub log: Vec<(u64, String)>,
}

fn last<'a>(evs: &'a [Ev], name: &str) -> Option<&'a Ev> {
    evs.iter().rev().find(|e| e.ev == name)
}

impl Driver {
    pub fn new(start: u64, glue: bool, test: bool) -> Driver {
        Driver { glue, test, start, t: TIMING, window_at: None, last_send: 0, passwords: 0, realm_enters: 0, enters: 0, moves: 0, log: Vec::new() }
    }

    fn note(&mut self, now: u64, what: impl Into<String>) {
        self.log.push((now.saturating_sub(self.start), what.into()));
    }

    fn send(&mut self, now: u64, k: Key, why: &str) -> Act {
        self.last_send = now;
        self.note(now, format!("send {k:?}: {why}"));
        Act::Send(k)
    }

    fn give_up(&mut self, now: u64, why: &str) -> Act {
        self.note(now, format!("stop: {why}"));
        Act::GiveUp(why.to_string())
    }

    pub fn step(&mut self, now: u64, window: bool, evs: &[Ev]) -> Act {
        if let Some(w) = last(evs, "in_world") {
            if !self.test || last(evs, "first_move").is_some() {
                return Act::Done;
            }
            // The world is still loading when the server says "in the world": wait until the Leyline UI has
            // answered (its load screen is gone) or a few seconds, then step, and again if no movement shows.
            let ui = last(evs, "builder_ready").or_else(|| last(evs, "addon")).map(|e| e.t);
            let ready_at = ui.map(|t| t + MOVE_SETTLE).unwrap_or(w.t + MOVE_NO_UI).max(w.t + MOVE_SETTLE);
            if self.moves >= MOVE_TRIES {
                return if now >= self.last_send + MOVE_RETRY { Act::Done } else { Act::Wait };
            }
            if now < ready_at || (self.moves > 0 && now < self.last_send + MOVE_RETRY) {
                return Act::Wait;
            }
            self.moves += 1;
            let why = if self.moves == 1 { "in the world: one step for the test".to_string() } else { format!("in the world: step again ({} of {MOVE_TRIES})", self.moves) };
            return self.send(now, Key::Forward, &why);
        }
        if now.saturating_sub(self.start) > self.t.total_max {
            return self.give_up(now, "not in the world after 150 seconds");
        }
        let Some(win) = self.window_at.or_else(|| window.then_some(now)) else {
            if now.saturating_sub(self.start) > self.t.window_max {
                return self.give_up(now, "the game window didn't open");
            }
            return Act::Wait;
        };
        if self.window_at.is_none() {
            self.window_at = Some(win);
            self.note(now, "the game window is open");
        }
        let since_send = now.saturating_sub(self.last_send);
        if let Some(pl) = last(evs, "player_login") {
            if now.saturating_sub(pl.t.max(self.last_send)) > 40_000 {
                return self.give_up(now, "entering the world took too long");
            }
            return Act::Wait;
        }
        if let Some(ce) = last(evs, "char_enum") {
            let wait = if self.enters == 0 { if self.glue { self.t.enter_wait_glue } else { self.t.enter_wait } } else { self.t.retry };
            if now.saturating_sub(ce.t.max(self.last_send)) < wait {
                return Act::Wait;
            }
            if self.enters >= 5 {
                return self.give_up(now, "character select didn't enter the world");
            }
            self.enters += 1;
            return self.send(now, Key::Enter, "character select: Enter World");
        }
        if let Some(ok) = last(evs, "auth_ok") {
            if last(evs, "world_auth").is_some_and(|w| w.t >= ok.t) {
                // the character list is on its way
                return if now.saturating_sub(ok.t) > 20_000 { self.give_up(now, "no character list") } else { Act::Wait };
            }
            let from = last(evs, "realm_list").map(|r| r.t).unwrap_or(ok.t).max(self.last_send);
            if now.saturating_sub(from) < self.t.realm_wait {
                return Act::Wait;
            }
            if self.realm_enters >= 3 {
                return self.give_up(now, "the realm list didn't open your Den");
            }
            self.realm_enters += 1;
            return self.send(now, Key::Enter, "realm list: OK");
        }
        // signing in: wait for a challenge we didn't cause, or one of ours that is still being answered
        let challenge = last(evs, "login_challenge");
        let failed = last(evs, "auth_fail");
        if let Some(c) = challenge {
            let refused = failed.is_some_and(|f| f.t >= c.t);
            if !refused && now.saturating_sub(c.t) < 8_000 {
                return Act::Wait;
            }
        }
        let first = win + if self.glue { self.t.first_password_glue } else { self.t.first_password };
        if now < first || since_send < self.t.retry {
            return Act::Wait;
        }
        if self.passwords >= 8 {
            return self.give_up(now, "the login screen didn't take the password");
        }
        self.passwords += 1;
        self.send(now, Key::Password, if failed.is_some() { "login screen: the password again" } else { "login screen: the local password" })
    }
}

/// The game window to send keys to, and whether the game still runs.
pub trait Game {
    fn window(&mut self) -> bool;
    fn alive(&mut self) -> bool;
    fn send(&mut self, k: Key, password: &str) -> bool;
}

/// The start's report (`<den-lite root>/den-click.json`).
#[derive(Debug, Clone, Serialize, Default)]
pub struct Report {
    pub phase: String,
    pub pid: u32,
    pub glue: bool,
    pub start: u64,
    pub outcome: String,
    pub actions: Vec<(u64, String)>,
    pub events: Vec<Ev>,
}

pub fn write_report(root: &Path, r: &Report) {
    let _ = crate::settings::write_atomic(&root.join(REPORT), serde_json::to_string_pretty(r).unwrap_or_default().as_bytes());
    let mut log = String::new();
    for (t, a) in &r.actions {
        log.push_str(&format!("{:>7} ms  {a}\n", t));
    }
    for e in &r.events {
        log.push_str(&format!("{:>7} ms  den-lite: {} {} {}\n", e.t.saturating_sub(r.start), e.ev, e.who, e.detail));
    }
    log.push_str(&format!("outcome: {}\n", r.outcome));
    let _ = std::fs::write(root.join(LOG), log);
}

/// Runs the sign-in for one start until the player is in the world, the game closes or a step times out. `tick`
/// is called every loop (the asks of the game, the test's hold).
pub fn drive(game: &mut dyn Game, den_dir: &Path, root: &Path, mut d: Driver, password: &str, pid: u32, tick: &mut dyn FnMut()) -> Outcome {
    let mut rep = Report { phase: "signing in".into(), pid, glue: d.glue, start: d.start, ..Default::default() };
    loop {
        let now = now_ms();
        let evs = events_since(den_dir, d.start);
        if !game.alive() {
            let asked = evs.iter().any(|e| e.ev == "login_challenge");
            let out = if d.glue && !asked && now.saturating_sub(d.start) < 120_000 {
                Outcome::GlueRefused
            } else {
                Outcome::Stopped("the game closed".into())
            };
            rep.outcome = format!("{out:?}");
            rep.phase = "done".into();
            rep.actions = d.log.clone();
            rep.events = evs;
            write_report(root, &rep);
            return out;
        }
        let win = game.window();
        let act = d.step(now, win, &evs);
        let out = match act {
            Act::Send(k) => {
                if !game.send(k, password) {
                    d.log.push((now.saturating_sub(d.start), "the keys didn't reach the game window".into()));
                }
                None
            }
            Act::Done => Some(Outcome::InWorld),
            Act::GiveUp(why) => Some(Outcome::Stopped(why)),
            Act::Wait => None,
        };
        rep.actions = d.log.clone();
        rep.events = evs;
        if let Some(o) = out {
            rep.outcome = format!("{o:?}");
            rep.phase = if o == Outcome::InWorld { "in_world".into() } else { "stopped".into() };
            write_report(root, &rep);
            return o;
        }
        write_report(root, &rep);
        tick();
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

// ------------------------------------------------------------------------------------------------ the game's asks

/// What the game asked the launcher (labs/den-lite ovmod.rs ASK_FILE): (id, "invite" | "forge"), taken.
pub fn take_ask(den_dir: &Path) -> Option<(u64, String)> {
    let p = den_dir.join("den-lite.ask");
    let t = std::fs::read_to_string(&p).ok()?;
    let _ = std::fs::remove_file(&p);
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    let id = v.get("id")?.as_u64()?;
    let what = v.get("ask")?.as_str()?;
    matches!(what, "invite" | "forge").then(|| (id, what.to_string()))
}

/// The launcher's answer for Den Lite to pass into the game (ANSWER_FILE).
pub fn answer(den_dir: &Path, id: u64, invite: Option<&str>, say: Option<&str>) {
    let mut v = serde_json::json!({ "id": id });
    if let Some(l) = invite {
        v["invite"] = l.into();
    }
    if let Some(s) = say {
        v["say"] = s.into();
    }
    let _ = crate::settings::write_atomic(&den_dir.join("den-lite.answer"), v.to_string().as_bytes());
}

/// The test's chat line to type (TYPE_FILE), taken: only `.den` / `.build` commands, one plain line.
pub fn take_type(root: &Path) -> Option<(u64, String)> {
    let p = root.join(TYPE_FILE);
    let t = std::fs::read_to_string(&p).ok()?;
    let _ = std::fs::remove_file(&p);
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    let id = v.get("id")?.as_u64()?;
    let line = v.get("line")?.as_str()?.trim().to_string();
    let ok = line.len() <= 120
        && line.chars().all(|c| c == ' ' || c.is_ascii_graphic())
        && (line.starts_with(".den ") || line == ".den" || line.starts_with(".build "));
    ok.then_some((id, line))
}

// ------------------------------------------------------------------------------------------------ the CLI

/// `Leyline den-play [--test] [--client <id>] [--hold <seconds>]`: My Den from the command line (the owner's
/// `den-test` step), the same code as the button.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cli {
    pub test: bool,
    pub client: Option<String>,
    /// Test only: after the world, wait up to this long for `<root>/den-click.shot` (the step's screenshot), then
    /// close the game.
    pub hold: u64,
}

pub fn parse_cli(args: &[String]) -> Option<Cli> {
    if args.get(1).map(String::as_str) != Some("den-play") {
        return None;
    }
    let mut c = Cli { hold: 30, ..Default::default() };
    let mut it = args.iter().skip(2);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--test" => c.test = true,
            "--client" => c.client = it.next().cloned(),
            "--hold" => c.hold = it.next().and_then(|h| h.parse().ok()).unwrap_or(30).min(600),
            _ => {}
        }
    }
    Some(c)
}

pub fn shot_marker(root: &Path) -> PathBuf {
    root.join("den-click.shot")
}

// ------------------------------------------------------------------------------------------------ Windows

#[cfg(windows)]
pub mod win {
    //! The game's window found by its process id, and keys posted to it (no focus taken, nothing else touched).
    use super::Key;

    type Hwnd = isize;
    const WM_KEYDOWN: u32 = 0x0100;
    const WM_KEYUP: u32 = 0x0101;
    const WM_CHAR: u32 = 0x0102;
    const VK_BACK: u32 = 0x08;
    const VK_RETURN: u32 = 0x0D;
    const VK_W: u32 = 0x57;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    extern "system" {
        fn EnumWindows(cb: extern "system" fn(Hwnd, isize) -> i32, lp: isize) -> i32;
        fn GetWindowThreadProcessId(h: Hwnd, pid: *mut u32) -> u32;
        fn IsWindowVisible(h: Hwnd) -> i32;
        fn GetClassNameW(h: Hwnd, buf: *mut u16, n: i32) -> i32;
        fn PostMessageW(h: Hwnd, msg: u32, w: usize, l: isize) -> i32;
        fn MapVirtualKeyW(code: u32, map: u32) -> u32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn GetExitCodeProcess(h: isize, code: *mut u32) -> i32;
        fn CloseHandle(h: isize) -> i32;
    }

    struct Find {
        pid: u32,
        found: Hwnd,
    }

    extern "system" fn each(h: Hwnd, lp: isize) -> i32 {
        // SAFETY: lp is the &mut Find passed to EnumWindows below, alive for the whole call
        let f = unsafe { &mut *(lp as *mut Find) };
        let mut pid = 0u32;
        let mut buf = [0u16; 64];
        // SAFETY: plain user32 queries on a window handle EnumWindows gave us
        let (vis, n) = unsafe {
            GetWindowThreadProcessId(h, &mut pid);
            (IsWindowVisible(h), GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32))
        };
        let class = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        if pid == f.pid && vis != 0 && class.starts_with("GxWindowClass") {
            f.found = h;
            return 0;
        }
        1
    }

    /// The game's main window (class GxWindowClass*), when it is open.
    pub fn window(pid: u32) -> Option<Hwnd> {
        let mut f = Find { pid, found: 0 };
        // SAFETY: the callback only reads its own Find through lp
        unsafe { EnumWindows(each, &mut f as *mut Find as isize) };
        (f.found != 0).then_some(f.found)
    }

    pub fn alive(pid: u32) -> bool {
        // SAFETY: a query-only handle, closed again
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h == 0 {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(h, &mut code);
            CloseHandle(h);
            ok != 0 && code == STILL_ACTIVE
        }
    }

    fn key(h: Hwnd, vk: u32, hold_ms: u64) -> bool {
        // SAFETY: posting key messages to the game's own window
        unsafe {
            let scan = MapVirtualKeyW(vk, 0) as isize;
            let down = 1 | (scan << 16);
            let up = down | (1 << 30) | (1 << 31);
            let a = PostMessageW(h, WM_KEYDOWN, vk as usize, down);
            if hold_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(hold_ms));
            }
            let b = PostMessageW(h, WM_KEYUP, vk as usize, up);
            a != 0 && b != 0
        }
    }

    /// Enter (opens the chat box), the line, Enter: as a player types a command.
    pub fn type_chat(pid: u32, line: &str) -> bool {
        let Some(h) = window(pid) else { return false };
        let gap = || std::thread::sleep(std::time::Duration::from_millis(30));
        let mut ok = key(h, VK_RETURN, 0);
        std::thread::sleep(std::time::Duration::from_millis(250));
        for c in line.encode_utf16() {
            // SAFETY: a character message to the game's own window
            ok &= unsafe { PostMessageW(h, WM_CHAR, c as usize, 1) } != 0;
            gap();
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
        ok & key(h, VK_RETURN, 0)
    }

    pub fn send(pid: u32, k: Key, password: &str) -> bool {
        let Some(h) = window(pid) else { return false };
        let gap = || std::thread::sleep(std::time::Duration::from_millis(30));
        match k {
            Key::Password => {
                let mut ok = true;
                for _ in 0..16 {
                    ok &= key(h, VK_BACK, 0);
                    gap();
                }
                for c in password.encode_utf16() {
                    // SAFETY: a character message to the game's own window
                    ok &= unsafe { PostMessageW(h, WM_CHAR, c as usize, 1) } != 0;
                    gap();
                }
                std::thread::sleep(std::time::Duration::from_millis(120));
                ok & key(h, VK_RETURN, 0)
            }
            Key::Enter => key(h, VK_RETURN, 0),
            Key::Forward => key(h, VK_W, 400),
        }
    }
}

/// A chat line typed into the game window (the test's fallback; Windows only).
pub fn type_chat(pid: u32, line: &str) -> bool {
    #[cfg(windows)]
    {
        win::type_chat(pid, line)
    }
    #[cfg(not(windows))]
    {
        let _ = (pid, line);
        false
    }
}

/// The game started with this process id, on this PC.
pub struct Process(pub u32);

impl Game for Process {
    fn window(&mut self) -> bool {
        #[cfg(windows)]
        {
            win::window(self.0).is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
    fn alive(&mut self) -> bool {
        #[cfg(windows)]
        {
            win::alive(self.0)
        }
        #[cfg(not(windows))]
        {
            std::path::Path::new(&format!("/proc/{}", self.0)).exists()
        }
    }
    fn send(&mut self, k: Key, password: &str) -> bool {
        #[cfg(windows)]
        {
            win::send(self.0, k, password)
        }
        #[cfg(not(windows))]
        {
            let _ = (k, password);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(t: u64, name: &str) -> Ev {
        Ev { t, ev: name.into(), who: "DEN".into(), detail: String::new() }
    }

    /// A fake game for `drive`: it "reacts" to keys by adding the milestones Den Lite would log.
    struct Fake {
        dir: PathBuf,
        alive_until: u64,
        window_from: u64,
        sent: Vec<Key>,
        stock: bool,
    }

    impl Fake {
        fn log(&self, name: &str) {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(self.dir.join(EVENTS)).unwrap();
            writeln!(f, "{}", serde_json::json!({"t": now_ms(), "ev": name, "who": "DEN"})).unwrap();
        }
    }

    impl Game for Fake {
        fn window(&mut self) -> bool {
            now_ms() >= self.window_from
        }
        fn alive(&mut self) -> bool {
            now_ms() < self.alive_until
        }
        fn send(&mut self, k: Key, _: &str) -> bool {
            self.sent.push(k);
            match (k, self.sent.iter().filter(|x| **x == Key::Enter).count()) {
                (Key::Password, _) => ["login_challenge", "auth_ok", "realm_list"].iter().for_each(|e| self.log(e)),
                // a stock client with realmName set goes on by itself; here the first Enter is the realm list's
                (Key::Enter, 1) if self.stock => ["world_auth", "char_enum"].iter().for_each(|e| self.log(e)),
                (Key::Enter, _) => ["player_login", "in_world"].iter().for_each(|e| self.log(e)),
                (Key::Forward, _) => self.log("first_move"),
            }
            true
        }
    }

    #[test]
    fn a_stock_game_is_signed_in_by_the_launchers_keys_step_by_step() {
        let mut d = Driver::new(0, false, false);
        assert_eq!(d.step(1_000, false, &[]), Act::Wait, "no window yet");
        assert_eq!(d.step(2_000, true, &[]), Act::Wait, "the login screen is still loading");
        assert_eq!(d.step(7_000, true, &[]), Act::Send(Key::Password));
        assert_eq!(d.step(8_000, true, &[]), Act::Wait, "not again at once");
        // nothing reached Den Lite (the screen wasn't up yet): again after the retry gap, the box cleared first
        assert_eq!(d.step(11_600, true, &[]), Act::Send(Key::Password));
        let mut evs = vec![ev(11_700, "login_challenge"), ev(11_800, "auth_ok"), ev(11_900, "realm_list")];
        assert_eq!(d.step(12_000, true, &evs), Act::Wait, "the stock game goes to the realm named in Config.wtf");
        // ... unless the realm list stays up: Enter (its OK)
        assert_eq!(d.step(18_000, true, &evs), Act::Send(Key::Enter));
        evs.extend([ev(18_200, "world_auth"), ev(18_300, "char_enum")]);
        assert_eq!(d.step(19_000, true, &evs), Act::Wait, "character select is loading the character");
        assert_eq!(d.step(21_000, true, &evs), Act::Send(Key::Enter), "Enter World");
        evs.push(ev(21_100, "player_login"));
        assert_eq!(d.step(22_000, true, &evs), Act::Wait);
        evs.push(ev(25_000, "in_world"));
        assert_eq!(d.step(25_100, true, &evs), Act::Done);
        assert_eq!((d.passwords, d.realm_enters, d.enters), (2, 1, 1));
    }

    #[test]
    fn our_login_screen_code_goes_first_and_the_keys_only_fill_in() {
        let mut d = Driver::new(0, true, false);
        d.step(1_000, true, &[]);
        assert_eq!(d.step(10_000, true, &[]), Act::Wait, "the login screen code gets 20 s");
        let mut evs = vec![ev(4_000, "login_challenge"), ev(4_100, "auth_ok"), ev(4_150, "realm_list"), ev(4_300, "world_auth"), ev(4_400, "char_enum")];
        assert_eq!(d.step(6_000, true, &evs), Act::Wait, "it enters the world by itself");
        evs.extend([ev(6_500, "player_login"), ev(9_000, "in_world")]);
        assert_eq!(d.step(9_100, true, &evs), Act::Done);
        assert!(d.log.iter().all(|(_, a)| !a.starts_with("send")), "no key was needed: {:?}", d.log);
        // its sign-in stalls at character select: one Enter after 5 s
        let mut d = Driver::new(0, true, false);
        d.step(1_000, true, &[]);
        let evs = vec![ev(4_000, "login_challenge"), ev(4_100, "auth_ok"), ev(4_300, "world_auth"), ev(4_400, "char_enum")];
        assert_eq!(d.step(8_000, true, &evs), Act::Wait);
        assert_eq!(d.step(9_500, true, &evs), Act::Send(Key::Enter));
    }

    #[test]
    fn a_wrong_password_is_typed_again_and_every_step_gives_up_in_the_end() {
        let mut d = Driver::new(0, false, false);
        d.step(1_000, true, &[]);
        assert_eq!(d.step(6_100, true, &[]), Act::Send(Key::Password));
        let evs = vec![ev(6_200, "login_challenge"), ev(6_300, "auth_fail")];
        assert_eq!(d.step(7_000, true, &evs), Act::Wait);
        assert_eq!(d.step(11_000, true, &evs), Act::Send(Key::Password), "typed again after a refusal");
        let mut n = 0;
        let mut t = 11_000;
        while n < 20 {
            t += 5_000;
            if let Act::GiveUp(why) = d.step(t, true, &evs) {
                assert!(why.contains("password"), "{why}");
                break;
            }
            n += 1;
        }
        assert!(n < 20 && d.passwords == 8);
        // no window at all
        let mut d = Driver::new(0, false, false);
        assert!(matches!(d.step(91_000, false, &[]), Act::GiveUp(w) if w.contains("window")));
        // stuck at character select
        let mut d = Driver::new(0, false, false);
        d.step(1, true, &[]);
        let evs = vec![ev(10, "auth_ok"), ev(20, "world_auth"), ev(30, "char_enum")];
        let mut last = Act::Wait;
        for i in 1..60 {
            last = d.step(i * 3_000, true, &evs);
            if matches!(last, Act::GiveUp(_)) {
                break;
            }
        }
        assert!(matches!(last, Act::GiveUp(w) if w.contains("character select")) && d.enters == 5);
        // the overall limit
        let mut d = Driver::new(0, false, false);
        assert!(matches!(d.step(151_000, true, &[]), Act::GiveUp(_)));
    }

    #[test]
    fn the_test_takes_one_step_in_the_world() {
        let mut d = Driver::new(0, false, true);
        let mut evs = vec![ev(10, "in_world")];
        assert_eq!(d.step(100, true, &evs), Act::Wait, "the world is still loading");
        evs.push(ev(1_000, "builder_ready"));
        assert_eq!(d.step(2_000, true, &evs), Act::Wait, "the UI just answered");
        assert_eq!(d.step(3_600, true, &evs), Act::Send(Key::Forward));
        assert_eq!(d.step(4_000, true, &evs), Act::Wait, "waits to see it");
        assert_eq!(d.step(6_700, true, &evs), Act::Send(Key::Forward), "again when nothing moved");
        evs.push(ev(6_900, "first_move"));
        assert_eq!(d.step(7_000, true, &evs), Act::Done);
        // without the UI: a longer wait, then at most three steps
        let mut d = Driver::new(0, false, true);
        let evs = vec![ev(10, "in_world")];
        assert_eq!(d.step(5_000, true, &evs), Act::Wait);
        let sent = (0..40).map(|i| d.step(8_100 + i * 500, true, &evs)).filter(|a| *a == Act::Send(Key::Forward)).count();
        assert_eq!(sent, 3);
        assert_eq!(d.step(40_000, true, &evs), Act::Done);
        // not a test: done at once
        let mut d = Driver::new(0, false, false);
        assert_eq!(d.step(100, true, &evs), Act::Done);
    }

    #[test]
    fn drive_runs_a_whole_start_against_a_fake_game_and_reports_it() {
        for stock in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let start = now_ms();
            // an earlier game's lines are not this start's
            std::fs::write(dir.path().join(EVENTS), format!("{}\n", serde_json::json!({"t": start - 10_000, "ev": "in_world"}))).unwrap();
            let mut g = Fake { dir: dir.path().to_path_buf(), alive_until: start + 20_000, window_from: start, sent: vec![], stock };
            let mut d = Driver::new(start, false, true);
            d.t = Timing { first_password: 100, first_password_glue: 100, retry: 300, realm_wait: 400, enter_wait: 200, enter_wait_glue: 200, window_max: 5_000, total_max: 15_000 };
            let mut ticks = 0;
            let out = drive(&mut g, dir.path(), dir.path(), d, "den", 42, &mut || ticks += 1);
            assert_eq!(out, Outcome::InWorld, "stock: {stock}");
            assert_eq!(g.sent[0], Key::Password);
            assert!(g.sent.contains(&Key::Enter) && g.sent.last() == Some(&Key::Forward), "{:?}", g.sent);
            assert!(ticks > 0);
            let rep: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(REPORT)).unwrap()).unwrap();
            assert_eq!(rep["phase"], "in_world");
            assert_eq!(rep["pid"], 42);
            let log = std::fs::read_to_string(dir.path().join(LOG)).unwrap();
            assert!(log.contains("send Password") && log.contains("den-lite: in_world"), "{log}");
            assert!(!log.contains(" den\n") && !log.contains("\"den\""), "the password is never written");
        }
        // with our login screen code in, a game that closes before it asks to sign in refused it
        let dir = tempfile::tempdir().unwrap();
        let start = now_ms();
        let mut g = Fake { dir: dir.path().to_path_buf(), alive_until: start + 300, window_from: start, sent: vec![], stock: true };
        let out = drive(&mut g, dir.path(), dir.path(), Driver::new(start, true, false), "den", 1, &mut || {});
        assert_eq!(out, Outcome::GlueRefused);
    }

    #[test]
    fn the_games_asks_and_the_cli() {
        let d = tempfile::tempdir().unwrap();
        assert!(take_ask(d.path()).is_none());
        std::fs::write(d.path().join("den-lite.ask"), r#"{"id":7,"ask":"invite"}"#).unwrap();
        assert_eq!(take_ask(d.path()), Some((7, "invite".into())));
        assert!(!d.path().join("den-lite.ask").exists(), "taken once");
        std::fs::write(d.path().join("den-lite.ask"), r#"{"id":7,"ask":"rm -rf"}"#).unwrap();
        assert!(take_ask(d.path()).is_none());
        answer(d.path(), 7, Some("https://relay.openvanilla.app/join/x#Y"), None);
        let a: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(d.path().join("den-lite.answer")).unwrap()).unwrap();
        assert_eq!(a["invite"], "https://relay.openvanilla.app/join/x#Y");
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(parse_cli(&args("Leyline")), None);
        assert_eq!(parse_cli(&args("Leyline mcp")), None);
        assert_eq!(parse_cli(&args("Leyline den-play")), Some(Cli { test: false, client: None, hold: 30 }));
        assert_eq!(parse_cli(&args("Leyline den-play --test --client c335a-1 --hold 45")), Some(Cli { test: true, client: Some("c335a-1".into()), hold: 45 }));
        // events: this start's lines only, and never a friend's
        std::fs::write(d.path().join(EVENTS), "{\"t\":5,\"ev\":\"auth_ok\"}\n{\"t\":20,\"ev\":\"auth_ok\",\"detail\":\"remote\"}\nnot json\n{\"t\":30,\"ev\":\"char_enum\"}\n").unwrap();
        let e = events_since(d.path(), 10);
        assert_eq!(e.iter().map(|e| e.ev.as_str()).collect::<Vec<_>>(), ["char_enum"]);
        // the test's typed line: Den commands only
        assert!(take_type(d.path()).is_none());
        std::fs::write(d.path().join(TYPE_FILE), r#"{"id":3,"line":".den place chair"}"#).unwrap();
        assert_eq!(take_type(d.path()), Some((3, ".den place chair".into())));
        assert!(!d.path().join(TYPE_FILE).exists());
        for bad in ["/run os.exit()", "hello", ".den\nx", ".server info"] {
            std::fs::write(d.path().join(TYPE_FILE), serde_json::json!({"id": 4, "line": bad}).to_string()).unwrap();
            assert_eq!(take_type(d.path()), None, "{bad}");
        }
    }
}
