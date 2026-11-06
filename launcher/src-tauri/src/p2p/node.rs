//! Be a node (docs/BE-A-NODE.md). The owner, 5 Nov 2026: "create the ability to allow someone to be a node on the
//! network without playing the game but have it on in the client launcher already as a setting, intuitively in the
//! host section".
//!
//! One switch at the top of the Host page. While it is on, the launcher works for the network with no game running,
//! no Docker and no open ports:
//!
//! * **Share Leyline's own files** (`share::node_share`): the files the signed release log names (installers, Host
//!   packs, addons, upstream server source), each checked against the log, seeded over BitTorrent with the upload
//!   cap. Never game data, never a game client (the same allow-list and refusal list as "Help share").
//! * **Carry players** stays the full community relay (Docker, a public name, open ports): "Advanced" on the card,
//!   unchanged (`host::network`). Its "players carried" count shows on the card when it runs.
//!
//! What this file holds: the settings (`NodeSettings`), the state machine (`decide`: off, not offered, stopped, paused,
//! resting, the daily cap, full, running), the daily data budget and the stats (`Stats`), the connections cap
//! (`peers_check`), "start with Windows" (`autostart_args`), and the loop that drives it (`tick`, through `Env` so the
//! tests can drive it without a network). The network code itself stays in `share.rs`.
//!
//! Who can stop it:
//! * this PC: the switch (off), Pause, "only while idle and plugged in" (the carry ticker's rule, `resting`), the
//!   daily cap, and `LEYLINE_NO_NODE=1` in the environment (a hard stop for a shared or managed PC);
//! * the Leyline team: the site's `be_a_node` flag (`flags.rs`). Off there = every node stops at its next look at the
//!   site (every 30 minutes while it runs). The site alone never turns a node on.

use super::flags::Flags;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE: &str = "node.json";
pub const STATS_FILE: &str = "node-stats.json";
/// "In use": a key or the mouse in the last five minutes (the carry ticker's rule, host/network.rs).
pub const IDLE_SECS: u64 = 300;
/// How often the node looks at the PC (idle, power), the engine's counters and the caps.
pub const TICK_SECS: u64 = 30;
/// How often it reads the site's switches again while it runs (the owner's kill switch reaches it within this).
pub const FLAGS_EVERY_SECS: u64 = 30 * 60;
/// How often the shared files are checked and fetched again (hashing them all is not free).
pub const FILL_EVERY_SECS: u64 = 6 * 3600;
/// Over the connections cap twice in a row: sharing rests this long, then tries again.
pub const FULL_BACKOFF_SECS: u64 = 120;
/// Set (to anything but 0) and the node never runs on this PC.
pub const ENV_OFF: &str = "LEYLINE_NO_NODE";
/// The command line flag "start with Windows" uses: Leyline opens minimized, the node running.
pub const ARG: &str = "--node";

// ------------------------------------------------------------------------------------------------ settings

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeSettings {
    /// The big switch.
    pub on: bool,
    /// Paused by the person (stays on, does nothing until Resume).
    pub paused: bool,
    /// Upload for everything the node sends, KB/s.
    pub upload_kbps: u32,
    /// Most peers connected at once.
    pub max_peers: u32,
    /// Most data sent per day, MB. Reached: the node rests until midnight (UTC).
    pub day_mb: u32,
    /// Only while this PC is idle and plugged in.
    pub only_idle: bool,
    /// Closing the window keeps Leyline running (minimized) while the node is on.
    pub keep_running: bool,
    /// Start Leyline (minimized, the node running) when Windows starts.
    pub start_with_windows: bool,
}

impl Default for NodeSettings {
    fn default() -> Self {
        NodeSettings { on: false, paused: false, upload_kbps: 256, max_peers: 20, day_mb: 2048, only_idle: true, keep_running: false, start_with_windows: false }
    }
}

pub const UPLOAD_KBPS: (u32, u32) = (16, 100_000);
pub const MAX_PEERS: (u32, u32) = (1, 200);
pub const DAY_MB: (u32, u32) = (50, 1_000_000);

/// The caps within their bounds.
pub fn clean(mut s: NodeSettings) -> NodeSettings {
    s.upload_kbps = s.upload_kbps.clamp(UPLOAD_KBPS.0, UPLOAD_KBPS.1);
    s.max_peers = s.max_peers.clamp(MAX_PEERS.0, MAX_PEERS.1);
    s.day_mb = s.day_mb.clamp(DAY_MB.0, DAY_MB.1);
    s
}

pub fn day_bytes(s: &NodeSettings) -> u64 {
    s.day_mb as u64 * 1024 * 1024
}

pub fn settings_path(app_dir: &Path) -> PathBuf {
    app_dir.join(SETTINGS_FILE)
}

pub fn load(app_dir: &Path) -> NodeSettings {
    std::fs::read(settings_path(app_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).map(clean).unwrap_or_default()
}

pub fn save(app_dir: &Path, s: &NodeSettings) -> Result<(), String> {
    let b = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&settings_path(app_dir), &b).map_err(|e| format!("Could not save: {e}"))
}

/// The hard stop for this PC (`LEYLINE_NO_NODE`).
pub fn killed_by_env(v: Option<&str>) -> bool {
    v.is_some_and(|v| !v.trim().is_empty() && v.trim() != "0")
}

/// Be a node owns the share folder (share.rs `run` then leaves it alone): on here and offered by the site.
pub fn owns_share(app_dir: &Path, site: &Flags) -> bool {
    site.be_a_node && load(app_dir).on && !killed_by_env(std::env::var(ENV_OFF).ok().as_deref())
}

// ------------------------------------------------------------------------------------------------ the state machine

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rest {
    InUse,
    Battery,
}

/// "Only while idle and plugged in": why the node rests now, if it does. On battery, or a key or the mouse in the
/// last five minutes. Unknown (another OS) never rests. The same rule as the carry ticker (host/network.rs).
pub fn resting(only_idle: bool, idle_secs: Option<u64>, on_ac: Option<bool>) -> Option<Rest> {
    if !only_idle {
        return None;
    }
    if on_ac == Some(false) {
        return Some(Rest::Battery);
    }
    if idle_secs.is_some_and(|i| i < IDLE_SECS) {
        return Some(Rest::InUse);
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// The switch is off.
    Off,
    /// `LEYLINE_NO_NODE` on this PC.
    Stopped,
    /// The site does not offer it (not open yet, or the Leyline team stopped every node).
    NotOffered,
    /// Paused by the person.
    Paused,
    RestingInUse,
    RestingBattery,
    /// Today's data is used up: resting until midnight (UTC).
    DayCap,
    /// Over the connections cap: resting a couple of minutes.
    Full,
    Running,
}

impl State {
    pub fn sharing(self) -> bool {
        self == State::Running
    }
}

/// What the node looks at each time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inputs {
    pub offered: bool,
    pub killed: bool,
    pub idle_secs: Option<u64>,
    pub on_ac: Option<bool>,
    pub used_today: u64,
    pub full_until: u64,
    pub now: u64,
}

pub fn decide(s: &NodeSettings, i: &Inputs) -> State {
    if !s.on {
        return State::Off;
    }
    if i.killed {
        return State::Stopped;
    }
    if !i.offered {
        return State::NotOffered;
    }
    if s.paused {
        return State::Paused;
    }
    match resting(s.only_idle, i.idle_secs, i.on_ac) {
        Some(Rest::InUse) => State::RestingInUse,
        Some(Rest::Battery) => State::RestingBattery,
        None if i.used_today >= day_bytes(s) => State::DayCap,
        None if i.now < i.full_until => State::Full,
        None => State::Running,
    }
}

// ------------------------------------------------------------------------------------------------ stats and caps

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    /// When this PC first became a node (unix seconds; 0 = never).
    pub since: u64,
    /// Seconds spent running (sharing), all time.
    pub uptime_secs: u64,
    /// Bytes sent to other players, all time.
    pub shared_bytes: u64,
    /// The day (unix days, UTC) `today_bytes` counts.
    pub day: u64,
    pub today_bytes: u64,
    /// Files this PC shares now.
    pub files: u32,
    /// Peers connected at the last look.
    pub peers_now: u32,
    /// The last network check's answer (nat.rs kind: open, endpoint_independent, symmetric, unknown, blocked), or "".
    /// Never the address itself.
    pub network: String,
    pub full_until: u64,
    pub last_fill: u64,
    pub last_flags: u64,
    pub last_tick: u64,
    pub state: String,
    /// Looks in a row over the connections cap.
    pub peer_strikes: u32,
    /// The engine's counter at the last look (it starts again at 0 with Leyline, so it is not kept on disk).
    #[serde(skip)]
    pub engine_seen: u64,
}

impl Stats {
    /// A new day starts the daily count again.
    pub fn roll(&mut self, now: u64) {
        let d = now / 86_400;
        if d != self.day {
            self.day = d;
            self.today_bytes = 0;
        }
    }

    /// The engine's counter now (bytes sent since Leyline started): add what is new since the last look.
    pub fn add_uploaded(&mut self, engine_total: u64, now: u64) {
        self.roll(now);
        // a smaller counter than last time: the engine started again from 0
        let delta = if engine_total >= self.engine_seen { engine_total - self.engine_seen } else { engine_total };
        self.engine_seen = engine_total;
        self.shared_bytes = self.shared_bytes.saturating_add(delta);
        self.today_bytes = self.today_bytes.saturating_add(delta);
    }

    /// Time running since the last look. A long gap (the PC slept, Leyline was closed) is not uptime.
    pub fn add_uptime(&mut self, running: bool, now: u64) {
        if running && self.last_tick > 0 && now > self.last_tick {
            self.uptime_secs += (now - self.last_tick).min(TICK_SECS * 4);
        }
        self.last_tick = now;
    }
}

/// The connections cap: over it at two looks in a row, sharing rests `FULL_BACKOFF_SECS` (the engine is stopped, which
/// drops every connection, and started again after). Returns true when it just became full.
pub fn peers_check(st: &mut Stats, live: u32, cap: u32, now: u64) -> bool {
    st.peers_now = live;
    if live <= cap {
        st.peer_strikes = 0;
        return false;
    }
    st.peer_strikes += 1;
    if st.peer_strikes >= 2 {
        st.peer_strikes = 0;
        st.full_until = now + FULL_BACKOFF_SECS;
        return true;
    }
    false
}

pub fn stats_path(app_dir: &Path) -> PathBuf {
    app_dir.join(STATS_FILE)
}

pub fn load_stats(app_dir: &Path) -> Stats {
    std::fs::read(stats_path(app_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_stats(app_dir: &Path, st: &Stats) {
    if let Ok(b) = serde_json::to_vec_pretty(st) {
        let _ = crate::settings::write_atomic(&stats_path(app_dir), &b);
    }
}

// ------------------------------------------------------------------------------------------------ start with Windows

pub const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
pub const RUN_VALUE: &str = "Leyline node";

/// `reg.exe` arguments that add (or remove) Leyline from the user's own start-up list: no admin, no new crate.
pub fn autostart_args(exe: &str, on: bool) -> Vec<String> {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    if on {
        let cmd = format!("\"{exe}\" {ARG}");
        v(&["add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &cmd, "/f"])
    } else {
        v(&["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
    }
}

/// Whether "start with Windows" can work on this PC.
pub fn autostart_supported() -> bool {
    cfg!(windows)
}

/// Add or remove the start-up entry.
pub fn set_autostart(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let out = std::process::Command::new("reg")
            .args(autostart_args(&exe.display().to_string(), on))
            .stdin(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Could not change start with Windows: {e}"))?;
        // removing an entry that is not there is fine
        if !out.status.success() && on {
            return Err("Could not add Leyline to start with Windows.".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if on {
            return Err("Start with Windows works on Windows only.".into());
        }
        Ok(())
    }
}

/// Leyline was started by "start with Windows" (or with `--node` by hand).
pub fn started_as_node<I: IntoIterator<Item = String>>(args: I) -> bool {
    args.into_iter().skip(1).take(8).any(|a| a == ARG)
}

/// Close the window: keep Leyline running (minimized) instead? Only while the node is on and the person asked for it.
pub fn keep_running_on_close(app_dir: &Path) -> bool {
    let s = load(app_dir);
    s.on && s.keep_running
}

// ------------------------------------------------------------------------------------------------ the loop

/// What the node needs from the world: the clock, the PC, the site's switch, the engine. The real one is `Real`; the
/// tests use their own.
pub trait Env {
    fn now(&self) -> u64;
    fn idle_secs(&self) -> Option<u64>;
    fn on_ac(&self) -> Option<bool>;
    fn killed(&self) -> bool;
    /// The site's switches: fetched again when `refresh`, else the saved copy.
    fn site(&self, app_dir: &Path, refresh: bool) -> Flags;
    /// (bytes sent since Leyline started, peers connected now)
    fn engine(&self) -> (u64, u32);
    /// Share: with `refill` check and fetch the files first. The number of files shared (None: no trusted release
    /// log, so nothing is shared).
    fn share(&self, app_dir: &Path, refill: bool, upload_kbps: u32) -> Option<usize>;
    fn stop_share(&self, app_dir: &Path);
}

pub struct Real;

impl Env for Real {
    fn now(&self) -> u64 {
        super::mesh::now_secs()
    }
    fn idle_secs(&self) -> Option<u64> {
        crate::host::network::idle_secs()
    }
    fn on_ac(&self) -> Option<bool> {
        crate::host::network::on_ac()
    }
    fn killed(&self) -> bool {
        killed_by_env(std::env::var(ENV_OFF).ok().as_deref())
    }
    fn site(&self, app_dir: &Path, refresh: bool) -> Flags {
        if refresh {
            super::flags::load(app_dir, super::flags::URL).site
        } else {
            super::flags::cached_site(app_dir)
        }
    }
    fn engine(&self) -> (u64, u32) {
        super::share::engine_counts()
    }
    fn share(&self, app_dir: &Path, refill: bool, upload_kbps: u32) -> Option<usize> {
        super::share::node_share(app_dir, refill, upload_kbps)
    }
    fn stop_share(&self, app_dir: &Path) {
        super::share::node_stop(app_dir)
    }
}

/// What the Host page's card shows.
#[derive(Debug, Clone, Serialize)]
pub struct View {
    pub settings: NodeSettings,
    pub state: State,
    pub offered: bool,
    pub stats: Stats,
    /// This build can upload (the torrent engine is built in).
    pub engine: bool,
    /// The signed release log is trusted here, so there is something to share.
    pub has_files: bool,
    pub autostart: bool,
    pub started_as_node: bool,
}

fn lock() -> &'static std::sync::Mutex<()> {
    static L: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    L.get_or_init(Default::default)
}

/// One look: read the switches and the PC, decide, start or stop sharing, count. Returns the state now.
pub fn tick(app_dir: &Path, env: &dyn Env) -> (State, Stats) {
    let _g = lock().lock();
    let s = load(app_dir);
    let mut st = load_stats(app_dir);
    st.engine_seen = ENGINE_SEEN.load(std::sync::atomic::Ordering::Relaxed);
    let now = env.now();
    // the engine's counter: what was sent since the last look counts, whatever the state is now
    let (sent, peers) = env.engine();
    st.add_uploaded(sent, now);
    ENGINE_SEEN.store(st.engine_seen, std::sync::atomic::Ordering::Relaxed);
    let refresh = s.on && now.saturating_sub(st.last_flags) >= FLAGS_EVERY_SECS;
    let site = env.site(app_dir, refresh);
    if refresh {
        st.last_flags = now;
    }
    let was_running = st.state == "running";
    if was_running && peers_check(&mut st, peers, s.max_peers, now) {
        env.stop_share(app_dir);
    }
    let inputs = Inputs { offered: site.be_a_node, killed: env.killed(), idle_secs: env.idle_secs(), on_ac: env.on_ac(),
                          used_today: st.today_bytes, full_until: st.full_until, now };
    let state = decide(&s, &inputs);
    if state.sharing() {
        if st.since == 0 {
            st.since = now;
        }
        let refill = now.saturating_sub(st.last_fill) >= FILL_EVERY_SECS;
        match env.share(app_dir, refill, s.upload_kbps) {
            Some(n) => {
                st.files = n as u32;
                if refill {
                    st.last_fill = now;
                }
            }
            None => st.files = 0,
        }
    } else {
        env.stop_share(app_dir);
        st.peers_now = 0;
    }
    st.add_uptime(state.sharing(), now);
    st.state = serde_json::to_value(state).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
    save_stats(app_dir, &st);
    (state, st)
}

static ENGINE_SEEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LOOPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Start the node's loop (once), if the switch is on. It ends by itself once the switch is off.
pub fn start(app_dir: PathBuf) {
    use std::sync::atomic::Ordering;
    if !load(&app_dir).on || LOOPING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || loop {
        let (state, _) = tick(&app_dir, &Real);
        if state == State::Off {
            LOOPING.store(false, Ordering::SeqCst);
            // turned on again in the moment between: keep going
            if load(&app_dir).on && !LOOPING.swap(true, Ordering::SeqCst) {
                continue;
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(TICK_SECS));
    });
}

/// Keep the network check's answer (its kind only, never the address).
pub fn remember_network(app_dir: &Path, kind: &str) {
    let _g = lock().lock();
    let mut st = load_stats(app_dir);
    st.network = kind.chars().filter(|c| c.is_ascii_lowercase() || *c == '_').take(24).collect();
    save_stats(app_dir, &st);
}

/// Stop sharing at once (Leyline is quitting); the switch stays as it is for the next start.
pub fn stop_now(app_dir: &Path) {
    Real.stop_share(app_dir);
}

/// The card's view now (one look first, so a change shows at once).
pub fn view(app_dir: &Path, env: &dyn Env, started_as_node: bool) -> View {
    let (state, stats) = tick(app_dir, env);
    View {
        settings: load(app_dir),
        state,
        offered: env.site(app_dir, false).be_a_node,
        stats,
        engine: cfg!(torrent_engine),
        has_files: crate::releaselog::trusted_key().is_some(),
        autostart: autostart_supported(),
        started_as_node,
    }
}

/// Change the settings (the switch, Pause, the caps, keep running, start with Windows). Turning on needs the site's
/// offer; turning off always works. "Start with Windows" follows the switch: off takes Leyline out of start-up too.
pub fn set(app_dir: &Path, env: &dyn Env, want: NodeSettings) -> Result<String, String> {
    let want = clean(want);
    let before = load(app_dir);
    if want.on && !before.on {
        if env.killed() {
            return Err("Be a node is switched off on this PC (LEYLINE_NO_NODE).".into());
        }
        if !env.site(app_dir, true).be_a_node {
            return Err("Be a node is not open yet. It opens for everyone at once.".into());
        }
    }
    let autostart = want.on && want.start_with_windows;
    if autostart != (before.on && before.start_with_windows) {
        set_autostart(autostart)?;
    }
    save(app_dir, &want)?;
    Ok(if !want.on {
        "Off. This PC is not a node now.".into()
    } else if want.paused {
        "Paused. Resume when you like.".into()
    } else if !before.on {
        "On: this PC is a node now. Thank you.".into() // TODO(owner): the thank-you line in your words
    } else {
        "Saved.".into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Fake {
        now: Cell<u64>,
        idle: Cell<Option<u64>>,
        ac: Cell<Option<bool>>,
        offered: Cell<bool>,
        /// the saved copy of the site's switch (what a look without `refresh` sees)
        cached: Cell<bool>,
        killed: Cell<bool>,
        sent: Cell<u64>,
        peers: Cell<u32>,
        files: Cell<Option<usize>>,
        log: RefCell<Vec<String>>,
    }

    // the tests run one at a time where they share the engine counter (ENGINE_SEEN is per process)
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static S: std::sync::Mutex<()> = std::sync::Mutex::new(());
        S.lock().unwrap_or_else(|e| e.into_inner())
    }

    impl Fake {
        fn new() -> Fake {
            ENGINE_SEEN.store(0, std::sync::atomic::Ordering::Relaxed);
            Fake { now: Cell::new(1_762_300_000), idle: Cell::new(Some(3600)), ac: Cell::new(Some(true)), offered: Cell::new(true), cached: Cell::new(true),
                   killed: Cell::new(false), sent: Cell::new(0), peers: Cell::new(0), files: Cell::new(Some(3)), log: RefCell::new(vec![]) }
        }
        fn later(&self, secs: u64) {
            self.now.set(self.now.get() + secs);
        }
    }

    impl Env for Fake {
        fn now(&self) -> u64 {
            self.now.get()
        }
        fn idle_secs(&self) -> Option<u64> {
            self.idle.get()
        }
        fn on_ac(&self) -> Option<bool> {
            self.ac.get()
        }
        fn killed(&self) -> bool {
            self.killed.get()
        }
        fn site(&self, _: &Path, refresh: bool) -> Flags {
            if refresh {
                self.log.borrow_mut().push("flags".into());
                self.cached.set(self.offered.get());
            }
            Flags { be_a_node: self.cached.get(), ..Default::default() }
        }
        fn engine(&self) -> (u64, u32) {
            (self.sent.get(), self.peers.get())
        }
        fn share(&self, _: &Path, refill: bool, kbps: u32) -> Option<usize> {
            self.log.borrow_mut().push(format!("share refill={refill} kbps={kbps}"));
            self.files.get()
        }
        fn stop_share(&self, _: &Path) {
            self.log.borrow_mut().push("stop".into());
        }
    }

    fn on() -> NodeSettings {
        NodeSettings { on: true, ..Default::default() }
    }

    #[test]
    fn settings_default_off_and_are_clamped() {
        let d = tempfile::tempdir().unwrap();
        let s = load(d.path());
        assert!(!s.on && s.only_idle && !s.keep_running && !s.start_with_windows, "everything optional starts off");
        let c = clean(NodeSettings { upload_kbps: 0, max_peers: 9999, day_mb: 1, ..Default::default() });
        assert_eq!((c.upload_kbps, c.max_peers, c.day_mb), (16, 200, 50));
        std::fs::write(settings_path(d.path()), br#"{"on":true,"upload_kbps":999999999}"#).unwrap();
        let s = load(d.path());
        assert!(s.on && s.upload_kbps == 100_000 && s.max_peers == 20, "missing fields take the defaults, big ones are capped");
        std::fs::write(settings_path(d.path()), b"not json").unwrap();
        assert_eq!(load(d.path()), NodeSettings::default());
    }

    #[test]
    fn the_idle_rule_is_the_carry_tickers() {
        assert_eq!(resting(true, Some(30), Some(true)), Some(Rest::InUse));
        assert_eq!(resting(true, Some(3600), Some(false)), Some(Rest::Battery));
        assert_eq!(resting(true, Some(30), Some(false)), Some(Rest::Battery), "battery says more than in use");
        assert_eq!(resting(true, Some(3600), Some(true)), None);
        assert_eq!(resting(true, None, None), None, "unknown never rests");
        assert_eq!(resting(false, Some(1), Some(false)), None);
        assert_eq!(resting(true, Some(IDLE_SECS), Some(true)), None, "five minutes idle is idle");
    }

    #[test]
    fn decide_in_order() {
        let i = Inputs { offered: true, idle_secs: Some(3600), on_ac: Some(true), now: 1000, ..Default::default() };
        assert_eq!(decide(&NodeSettings::default(), &i), State::Off);
        assert_eq!(decide(&on(), &i), State::Running);
        assert_eq!(decide(&on(), &Inputs { killed: true, ..i.clone() }), State::Stopped, "this PC's hard stop wins");
        assert_eq!(decide(&on(), &Inputs { offered: false, ..i.clone() }), State::NotOffered, "the owner's switch for everyone");
        assert_eq!(decide(&NodeSettings { paused: true, ..on() }, &i), State::Paused);
        assert_eq!(decide(&on(), &Inputs { idle_secs: Some(10), ..i.clone() }), State::RestingInUse);
        assert_eq!(decide(&on(), &Inputs { on_ac: Some(false), ..i.clone() }), State::RestingBattery);
        assert_eq!(decide(&NodeSettings { only_idle: false, ..on() }, &Inputs { idle_secs: Some(10), ..i.clone() }), State::Running);
        assert_eq!(decide(&on(), &Inputs { used_today: day_bytes(&on()), ..i.clone() }), State::DayCap);
        assert_eq!(decide(&on(), &Inputs { full_until: 1001, ..i.clone() }), State::Full);
        assert_eq!(decide(&on(), &Inputs { full_until: 1000, ..i.clone() }), State::Running, "the rest is over");
        assert!(State::Running.sharing() && !State::Full.sharing() && !State::Paused.sharing());
    }

    #[test]
    fn stats_count_deltas_days_and_uptime() {
        let mut st = Stats::default();
        let day = 1_762_300_000 / 86_400 * 86_400;
        st.add_uploaded(1000, day + 10);
        st.add_uploaded(1500, day + 40);
        assert_eq!((st.shared_bytes, st.today_bytes), (1500, 1500));
        st.add_uploaded(200, day + 70); // the engine started again from 0
        assert_eq!(st.shared_bytes, 1700);
        st.add_uploaded(300, day + 86_400 + 5); // a new day
        assert_eq!((st.shared_bytes, st.today_bytes), (1800, 100));
        let mut st = Stats::default();
        st.add_uptime(true, 100);
        assert_eq!(st.uptime_secs, 0, "the first look starts the clock");
        st.add_uptime(true, 130);
        st.add_uptime(false, 160);
        assert_eq!(st.uptime_secs, 30, "resting is not uptime");
        st.add_uptime(true, 190);
        st.add_uptime(true, 190 + 8 * 3600);
        assert_eq!(st.uptime_secs, 60 + TICK_SECS * 4, "a sleeping PC's gap is not uptime");
    }

    #[test]
    fn the_connections_cap_needs_two_looks() {
        let mut st = Stats::default();
        assert!(!peers_check(&mut st, 25, 20, 100), "once over can be a moment");
        assert!(!peers_check(&mut st, 10, 20, 130), "and back under resets");
        assert!(!peers_check(&mut st, 25, 20, 160));
        assert!(peers_check(&mut st, 25, 20, 190));
        assert_eq!(st.full_until, 190 + FULL_BACKOFF_SECS);
        assert_eq!(st.peers_now, 25);
    }

    #[test]
    fn autostart_is_the_users_own_run_key() {
        let a = autostart_args(r"C:\Program Files\Leyline\Leyline.exe", true);
        assert_eq!(a[0], "add");
        assert!(a.contains(&RUN_KEY.to_string()) && RUN_KEY.starts_with("HKCU"), "this user only, no admin");
        assert!(a.contains(&"\"C:\\Program Files\\Leyline\\Leyline.exe\" --node".to_string()), "{a:?}");
        assert_eq!(autostart_args("x", false), ["delete", RUN_KEY, "/v", RUN_VALUE, "/f"]);
        assert!(started_as_node(["Leyline".to_string(), "--node".to_string()]));
        assert!(!started_as_node(["--node".to_string()]), "the program name is not an argument");
        assert!(!started_as_node(["Leyline".to_string(), "leyline://install/mod:x".to_string()]));
        assert!(killed_by_env(Some("1")) && !killed_by_env(Some("0")) && !killed_by_env(Some(" ")) && !killed_by_env(None));
    }

    #[test]
    fn tick_shares_rests_and_counts() {
        let _s = serial();
        let d = tempfile::tempdir().unwrap();
        let f = Fake::new();
        // off: nothing is shared, nothing fetched from the site
        let (state, _) = tick(d.path(), &f);
        assert_eq!(state, State::Off);
        assert_eq!(*f.log.borrow(), ["stop"]);
        // on: the switches are read again, the files checked and shared at the cap
        save(d.path(), &NodeSettings { upload_kbps: 300, ..on() }).unwrap();
        f.log.borrow_mut().clear();
        let (state, st) = tick(d.path(), &f);
        assert_eq!(state, State::Running);
        assert_eq!(*f.log.borrow(), ["flags", "share refill=true kbps=300"]);
        assert_eq!((st.files, st.since > 0, st.state.as_str()), (3, true, "running"));
        // the next look: no refill, no site fetch; what the engine sent is counted
        f.log.borrow_mut().clear();
        f.later(TICK_SECS);
        f.sent.set(5 * 1024 * 1024);
        let (_, st) = tick(d.path(), &f);
        assert_eq!(*f.log.borrow(), ["share refill=false kbps=300"]);
        assert_eq!((st.shared_bytes, st.today_bytes, st.uptime_secs), (5 * 1024 * 1024, 5 * 1024 * 1024, TICK_SECS));
        // someone uses the PC: sharing stops (the files stay)
        f.idle.set(Some(5));
        f.later(TICK_SECS);
        f.log.borrow_mut().clear();
        assert_eq!(tick(d.path(), &f).0, State::RestingInUse);
        assert_eq!(*f.log.borrow(), ["stop"]);
        f.idle.set(Some(3600));
        f.later(TICK_SECS);
        assert_eq!(tick(d.path(), &f).0, State::Running);
        // the stats survive a restart of Leyline (the engine counter starts again at 0)
        ENGINE_SEEN.store(0, std::sync::atomic::Ordering::Relaxed);
        f.sent.set(1024);
        f.later(TICK_SECS);
        let (_, st) = tick(d.path(), &f);
        assert_eq!(st.shared_bytes, 5 * 1024 * 1024 + 1024);
        assert_eq!(load_stats(d.path()).shared_bytes, st.shared_bytes);
    }

    #[test]
    fn tick_obeys_the_daily_cap_the_peers_cap_and_the_owners_switch() {
        let _s = serial();
        let d = tempfile::tempdir().unwrap();
        let f = Fake::new();
        save(d.path(), &NodeSettings { day_mb: 50, max_peers: 5, ..on() }).unwrap();
        assert_eq!(tick(d.path(), &f).0, State::Running);
        // today's 50 MB are sent: resting until the next day
        f.sent.set(50 * 1024 * 1024);
        f.later(TICK_SECS);
        assert_eq!(tick(d.path(), &f).0, State::DayCap);
        f.later(86_400);
        assert_eq!(tick(d.path(), &f).0, State::Running, "a new day");
        // too many peers twice: sharing stops for a while, then starts again
        f.peers.set(9);
        f.later(TICK_SECS);
        assert_eq!(tick(d.path(), &f).0, State::Running);
        f.later(TICK_SECS);
        f.log.borrow_mut().clear();
        assert_eq!(tick(d.path(), &f).0, State::Full);
        assert_eq!(f.log.borrow()[0], "stop");
        f.peers.set(0);
        f.later(FULL_BACKOFF_SECS);
        assert_eq!(tick(d.path(), &f).0, State::Running);
        // the Leyline team turns it off for everyone: the next look at the site stops it
        f.offered.set(false);
        f.later(TICK_SECS);
        assert_eq!(tick(d.path(), &f).0, State::Running, "the saved copy until the next look at the site");
        f.later(FLAGS_EVERY_SECS);
        f.log.borrow_mut().clear();
        assert_eq!(tick(d.path(), &f).0, State::NotOffered);
        assert_eq!(*f.log.borrow(), ["flags", "stop"]);
        // and this PC's hard stop
        f.offered.set(true);
        f.killed.set(true);
        f.later(TICK_SECS);
        assert_eq!(tick(d.path(), &f).0, State::Stopped);
    }

    #[test]
    fn set_needs_the_offer_to_turn_on_and_never_to_turn_off() {
        let _s = serial();
        let d = tempfile::tempdir().unwrap();
        let f = Fake::new();
        f.offered.set(false);
        assert!(set(d.path(), &f, on()).unwrap_err().contains("not open yet"));
        assert!(!load(d.path()).on, "nothing saved when refused");
        f.offered.set(true);
        assert!(set(d.path(), &f, NodeSettings { upload_kbps: 1, ..on() }).unwrap().starts_with("On"));
        assert_eq!(load(d.path()).upload_kbps, UPLOAD_KBPS.0, "the caps are clamped");
        assert_eq!(set(d.path(), &f, NodeSettings { paused: true, ..on() }).unwrap(), "Paused. Resume when you like.");
        f.offered.set(false);
        assert_eq!(set(d.path(), &f, NodeSettings::default()).unwrap(), "Off. This PC is not a node now.");
        f.killed.set(true);
        f.offered.set(true);
        assert!(set(d.path(), &f, on()).unwrap_err().contains("LEYLINE_NO_NODE"));
        // keep running only while on
        save(d.path(), &NodeSettings { keep_running: true, ..on() }).unwrap();
        assert!(keep_running_on_close(d.path()));
        save(d.path(), &NodeSettings { keep_running: true, ..Default::default() }).unwrap();
        assert!(!keep_running_on_close(d.path()));
    }

    #[test]
    fn owns_share_needs_the_switch_and_the_offer() {
        let d = tempfile::tempdir().unwrap();
        let site = Flags { be_a_node: true, ..Default::default() };
        assert!(!owns_share(d.path(), &site));
        save(d.path(), &on()).unwrap();
        assert!(owns_share(d.path(), &site));
        assert!(!owns_share(d.path(), &Flags::default()), "withdrawn: Help share's own rules again");
    }
}
