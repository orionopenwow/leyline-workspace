//! Den Lite: a tiny local game server for the 3.3.5a client (build 12340), built for the Leyline launcher's
//! "Your Den". It gives a player an instant white room to build in, with no Docker and no database: a login server
//! on 127.0.0.1:3724, a world server on 127.0.0.1:8085, one ready-made character, the Den's room as gameobjects,
//! stamps, undo and the `leyline-den` save files the full Den (`ov_den.lua`) reads and writes.
//!
//! See README.md for what works, what is verified only by the test client, and what multi-player would need.

pub mod accdata;
pub mod cheers;
pub mod creatures;
pub mod classes;
pub mod combat;
pub mod companion;
pub mod den;
pub mod dojo;
pub mod events;
pub mod farsight;
pub mod gizmo;
pub mod flight;
pub mod gear;
pub mod gearsets;
pub mod guests;
pub mod ovmod;
pub mod login;
pub mod models;
pub mod movewatch;
pub mod notice;
pub mod objects;
pub mod places;
pub mod play;
pub mod mymodels;
pub mod raw;
pub mod specs;
pub mod spells;
pub mod stamps;
pub mod testctl;
pub mod world;

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    Male,
    Female,
}

/// The ready-made character.
#[derive(Debug, Clone)]
pub struct Hero {
    pub name: String,
    pub gender: Gender,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: IpAddr,
    /// 0 picks a free port (the tests do).
    pub login_port: u16,
    pub world_port: u16,
    /// The fixed local password every account name logs in with (see login.rs for why it can't be "set on first use").
    pub password: String,
    pub realm_name: String,
    pub hero: Hero,
    /// Where `.den save` writes and `.den load` reads (`<name>.den.json`): the host kit's `state/dens`.
    pub den_dir: PathBuf,
    /// The stamps (`realm/custom/lua_scripts/den-stamps`).
    pub stamps_dir: PathBuf,
    pub spot: den::Spot,
    /// The game's folder, for `.den mods` (the launcher's `<game>/Leyline/mods.json`); None: not shown.
    pub client: Option<PathBuf>,
    /// Friends' listeners (guests only: what the launcher's relay tunnel connects to), on `bind`; 0 picks a free
    /// port; None: no friends. Written to `<dens>/den-lite.remote` ("<login port> <world port>").
    pub remote_ports: Option<(u16, u16)>,
    /// Friends on the same network or through the player's own address (port forwarding, their own server): a
    /// direct listener pair on this LAN address of the PC (never 127.0.0.1), guests only (login.rs serve_direct).
    /// None: off. Written to `<dens>/den-lite.direct` ("<ip> <login port> <world port>", or "off <why>").
    pub friends_lan: Option<IpAddr>,
    pub friends_lan_ports: (u16, u16),
    /// The companion is at the player's side from login (owner 1 Nov: "she's the main helper of the game");
    /// `--no-companion`: only when called.
    pub companion: bool,
    pub verbose: bool,
    /// The den-test's commands through the Dens folder (testctl.rs); only with a loopback `bind`.
    pub test_control: bool,
    /// How long what Den Lite says on entering waits for the Den bar's hello before it goes in the chat (notice.rs),
    /// from the game being in the world; and at most from Enter World (a long loading screen).
    pub ui_wait_ms: u64,
    pub ui_load_ms: u64,
    /// The encouragement moments: at most one every this many seconds (notice.rs cheer_ok).
    pub cheer_gap: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
            login_port: 3724,
            world_port: 8085,
            password: "den".into(),
            realm_name: "Your Den".into(),
            hero: Hero { name: "Wren".into(), gender: Gender::Female },
            den_dir: PathBuf::from("dens"),
            stamps_dir: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("den-stamps")))
                .unwrap_or_else(|| PathBuf::from("den-stamps")),
            spot: den::Spot::default(),
            client: None,
            remote_ports: Some((0, 0)),
            friends_lan: None,
            friends_lan_ports: (3724, 8085),
            companion: true,
            verbose: false,
            test_control: false,
            ui_wait_ms: 4000,
            ui_load_ms: 30_000,
            cheer_gap: 180,
        }
    }
}

pub struct Shared {
    pub cfg: Config,
    /// Account (upper case) -> the SRP6 session key the login server agreed, for the world server's auth session.
    /// ... with who it is and whether it came in on the remote listener.
    pub sessions: Mutex<HashMap<String, ([u8; 40], login::Who, bool)>>,
    /// Everyone in the world now (world.rs Peer): the host and their guests.
    pub players: Mutex<Vec<world::Peer>>,
    /// Friends may build (`.den friends build on`); off at every start.
    pub guests_build: std::sync::atomic::AtomicBool,
    remote_world: Mutex<Option<SocketAddr>>,
    pub den: Mutex<den::Den>,
    /// The companion (one: Den Lite has one player in the world at a time).
    pub companion: Mutex<companion::Companion>,
    /// The player in the world now: the session's number, its writer and where the player stands.
    pub live: Mutex<Option<(u64, world::Writer, companion::At)>>,
    world: Mutex<SocketAddr>,
    /// The Den as the autosave last wrote it (den::Den::autosave_key).
    autosaved: Mutex<String>,
    opened: Mutex<std::collections::HashSet<String>>,
    /// `<dens>/den-lite.events`: the milestones on the way in (events.rs).
    pub events: events::Events,
    /// Who has the Den bar, and what waits for its hello (notice.rs).
    pub notices: Mutex<notice::Notices>,
    /// When the last encouragement moment was said (unix seconds; notice::cheer_ok).
    pub last_cheer: Mutex<u64>,
    /// The stamp the Leyline UI's Den home marked last (`.den stamp <name>` puts it down at once; then `turn <deg>`,
    /// `here`, `place` and `cancel` work on it, as on the full Den): its name, where, the facing, the extra turn.
    pub last_stamp: Mutex<Option<(String, f32, f32, f32, f32, f32)>>,
    /// When the autosave last wrote (unix seconds; 0: not in this run), for the Den bar's "Saved 12 s ago".
    pub autosaved_at: std::sync::atomic::AtomicU64,
    /// What the host's game did with its character this time in the world (movewatch.rs: `.den where`).
    pub moves: Mutex<Option<movewatch::Watch>>,
    /// The Dojo (dojo.rs): fly, speed, the training dummies and Jaina's build in progress (the host's).
    pub dojo: Mutex<dojo::Dojo>,
    /// The editor's build mode (gizmo.rs `mode build`): props are shown as goobers the client highlights and lets
    /// you right-click; off: as they are.
    pub build_look: std::sync::atomic::AtomicBool,
    /// How often the client asked for a build twin's template (gizmo.rs `twin`): it does once per prop kind, the
    /// first time build mode shows it; the den-test reads it to know the client took the goober look.
    pub twin_queries: std::sync::atomic::AtomicU32,
    /// The host's placement aids as the den-test reads them (gizmo::view).
    pub aid_view: Mutex<serde_json::Value>,
    /// The class picked (`.den class`, classes.rs; kept in the Dens folder) and the sandbox combat (combat.rs).
    pub pick: Mutex<Option<classes::Pick>>,
    pub combat: Mutex<combat::Combat>,
    /// The character's looks (`.den gear`, gear.rs; kept in the Dens folder).
    pub gear: Mutex<Option<gear::Gear>>,
    /// Each connection in the world (world.rs Writer address): the map its player is on and whether it is a guest
    /// (a friend's game gets the player's own models as stand-ins: mymodels.rs).
    pub seen: Mutex<HashMap<usize, (u32, bool)>>,
    /// `.den go` trips waiting for the game's answer, by session (places.rs).
    pub trips: Mutex<HashMap<u64, places::Trip>>,
    /// The Den's map (den.rs Spot::map), kept apart so sending objects never waits on the Den's lock.
    pub den_map: std::sync::atomic::AtomicU32,
}

impl Shared {
    /// One milestone in `<dens>/den-lite.events` (events.rs; never anything secret).
    pub fn event(&self, ev: &str, who: &str, detail: &str) {
        self.events.note(ev, who, detail);
    }

    pub fn log(&self, s: &str) {
        if self.cfg.verbose {
            eprintln!("den-lite: {s}");
        }
    }
    /// An open.json id seen for the first time (here or in `<dens>/.open-done`): true, and it is kept as done.
    pub fn mark_opened(&self, dir: &std::path::Path, id: &str) -> bool {
        let done = dir.join(".open-done");
        let mut seen = self.opened.lock().unwrap();
        if seen.contains(id) || std::fs::read_to_string(&done).map(|t| t.lines().any(|l| l == id)).unwrap_or(false) {
            seen.insert(id.to_string());
            return false;
        }
        seen.insert(id.to_string());
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&done) {
            let _ = writeln!(f, "{id}");
        }
        true
    }
    /// The autosave (den::AUTOSAVE): written when the Den changed since the last one. True when it wrote. A Den
    /// whose autosave didn't open is never written over until the player changes something.
    pub fn autosave(&self) -> bool {
        let d = self.den.lock().unwrap_or_else(|e| e.into_inner());
        let key = d.autosave_key();
        let mut last = self.autosaved.lock().unwrap_or_else(|e| e.into_inner());
        if *last == key {
            return false;
        }
        let ok = d.save(&self.cfg.den_dir, den::AUTOSAVE, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t| t.as_secs()).unwrap_or(0)).is_ok();
        if ok {
            *last = key;
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t| t.as_secs()).unwrap_or(0);
            self.autosaved_at.store(t, std::sync::atomic::Ordering::Relaxed);
            self.event("autosave", "", &format!("{} props", d.props.len()));
        }
        ok
    }

    /// The friends' world listener, as a last resort for their realm list (the tunnel's address belongs there).
    pub fn remote_world_address(&self) -> String {
        self.remote_world.lock().unwrap().map(|a| a.to_string()).unwrap_or_else(|| self.world_address())
    }

    /// The realm list's address for the world server.
    pub fn world_address(&self) -> String {
        self.world.lock().unwrap().to_string()
    }
}

/// `<dens>/den-lite.remote`: "<login port> <world port>" of the friends' listeners (the launcher's tunnel reads it).
pub const REMOTE_FILE: &str = "den-lite.remote";
/// `<dens>/den-lite.direct`: "<ip> <login port> <world port>" of the friends' direct listeners, or "off <why>"
/// (the launcher's Invite reads it: the same-network and own-address ways).
pub const DIRECT_FILE: &str = "den-lite.direct";

pub struct Running {
    pub login: SocketAddr,
    pub world: SocketAddr,
    /// Friends' listeners (Config::remote_ports): login, world.
    pub remote: Option<(SocketAddr, SocketAddr)>,
    pub shared: Arc<Shared>,
}

/// `<dens>/den-lite.direct-want`: "<this PC's network address>" written by the launcher's Invite (the first time a
/// friend is invited the direct way opens, so playing alone never asks the firewall anything).
pub const DIRECT_WANT: &str = "den-lite.direct-want";

/// The friends' direct listener (same network, or the player's own address): guests only, on this PC's network
/// address (never 127.0.0.1). A failure never stops the Den: `<dens>/den-lite.direct` says why. Once per run.
pub fn open_direct(shared: &Arc<Shared>, ip: IpAddr, (lp, wp): (u16, u16)) -> Result<(u16, u16), String> {
    static OPEN: std::sync::OnceLock<(u16, u16)> = std::sync::OnceLock::new();
    if let Some(p) = OPEN.get() {
        return Ok(*p);
    }
    let file = shared.cfg.den_dir.join(DIRECT_FILE);
    if ip.is_loopback() || ip.is_unspecified() {
        let _ = std::fs::write(&file, "off not an address on a network\n");
        return Err("not an address on a network".into());
    }
    match (TcpListener::bind((ip, lp)), TcpListener::bind((ip, wp))) {
        (Ok(l), Ok(w)) => {
            let lport = l.local_addr().map(|a| a.port()).unwrap_or(lp);
            let wport = w.local_addr().map(|a| a.port()).unwrap_or(wp);
            let s = shared.clone();
            std::thread::spawn(move || login::serve_direct(l, s, wport));
            let s = shared.clone();
            std::thread::spawn(move || world::serve(w, s, true));
            let _ = std::fs::write(&file, format!("{ip} {lport} {wport}\n"));
            shared.event("direct_open", "", &format!("{lport} {wport}"));
            let _ = OPEN.set((lport, wport));
            Ok((lport, wport))
        }
        (a, b) => {
            let why = a.err().or(b.err()).map(|e| e.to_string()).unwrap_or_default();
            let _ = std::fs::write(&file, format!("off {why}\n"));
            Err(why)
        }
    }
}

/// The launcher's ask for the direct way (`<dens>/den-lite.direct-want`), taken once it is read.
pub fn direct_wanted(shared: &Arc<Shared>) {
    let p = shared.cfg.den_dir.join(DIRECT_WANT);
    let Ok(t) = std::fs::read_to_string(&p) else { return };
    let _ = std::fs::remove_file(&p);
    match t.trim().parse::<IpAddr>() {
        Ok(ip) => {
            let ports = shared.cfg.friends_lan_ports;
            let _ = open_direct(shared, ip, ports);
        }
        Err(_) => {
            let _ = std::fs::write(shared.cfg.den_dir.join(DIRECT_FILE), "off not an address\n");
        }
    }
}

/// Binds both ports and serves them on background threads.
pub fn start(mut cfg: Config) -> io::Result<Running> {
    // the test's commands come from a file anyone on this PC could write: never on a Den others can reach
    if cfg.test_control && !cfg.bind.is_loopback() {
        eprintln!("den-lite: test commands are only for a Den on 127.0.0.1; they stay off");
        cfg.test_control = false;
    }
    let login = TcpListener::bind((cfg.bind, cfg.login_port))?;
    let world = TcpListener::bind((cfg.bind, cfg.world_port))?;
    let (la, wa) = (login.local_addr()?, world.local_addr()?);
    let remote = match cfg.remote_ports {
        Some((lp, wp)) => Some((TcpListener::bind((cfg.bind, lp))?, TcpListener::bind((cfg.bind, wp))?)),
        None => None,
    };
    let remote_addrs = match &remote {
        Some((l, w)) => Some((l.local_addr()?, w.local_addr()?)),
        None => None,
    };
    if let Some((l, w)) = remote_addrs {
        let _ = std::fs::create_dir_all(&cfg.den_dir);
        let _ = std::fs::write(cfg.den_dir.join(REMOTE_FILE), format!("{} {}\n", l.port(), w.port()));
    }
    let _ = std::fs::create_dir_all(&cfg.den_dir);
    let _ = std::fs::remove_file(cfg.den_dir.join(DIRECT_FILE));
    den::set_local_stamps(&cfg.den_dir);
    let mut d = den::Den::new(cfg.spot);
    if let Some(Err(e)) = d.reload_ground(&cfg.den_dir) {
        if cfg.verbose {
            eprintln!("den-lite: the ground file isn't used: {e}");
        }
    }
    // the Den as it was when Den Lite last ran (den::AUTOSAVE)
    if let Some(Err(e)) = d.open_autosave(&cfg.den_dir) {
        eprintln!("den-lite: the autosave didn't open ({e}); it is kept as it is and the Den starts empty");
    }
    // the props' models for the Builder's gallery (the player's own table, put in the Dens folder by the launcher)
    let known = models::load(&cfg.den_dir);
    // the player's own models, dropped on the launcher's Your Den card (mymodels.rs)
    let mine = mymodels::load(&cfg.den_dir);
    if cfg.verbose {
        eprintln!("den-lite: {known} prop models known for the gallery");
    }
    let autosaved = d.autosave_key();
    let den_map = d.spot.map;
    let pick = classes::Pick::load(&cfg.den_dir);
    let looks = gear::Gear::load(&cfg.den_dir);
    let events = events::Events::new(&cfg.den_dir);
    events.note("start", "", &format!("{}{}", la.port(), if cfg.test_control { " test" } else { "" }));
    events.note("models", "", &known.to_string());
    events.note("my_models", "", &mine.to_string());
    let shared = Arc::new(Shared {
        events,
        den: Mutex::new(d),
        companion: Mutex::new(companion::Companion::restore(&cfg.den_dir)),
        autosaved: Mutex::new(autosaved),
        live: Mutex::new(None),
        cfg,
        sessions: Mutex::new(HashMap::new()),
        players: Mutex::new(Vec::new()),
        guests_build: std::sync::atomic::AtomicBool::new(false),
        remote_world: Mutex::new(remote_addrs.map(|r| r.1)),
        world: Mutex::new(wa),
        opened: Mutex::new(std::collections::HashSet::new()),
        notices: Mutex::new(notice::Notices::default()),
        last_cheer: Mutex::new(0),
        last_stamp: Mutex::new(None),
        autosaved_at: std::sync::atomic::AtomicU64::new(0),
        moves: Mutex::new(None),
        dojo: Mutex::new(dojo::Dojo::default()),
        build_look: std::sync::atomic::AtomicBool::new(false),
        twin_queries: std::sync::atomic::AtomicU32::new(0),
        aid_view: Mutex::new(serde_json::Value::Null),
        pick: Mutex::new(pick),
        combat: Mutex::new(combat::Combat::new(world::PLAYER_GUID)),
        gear: Mutex::new(looks),
        seen: Mutex::new(HashMap::new()),
        trips: Mutex::new(HashMap::new()),
        den_map: std::sync::atomic::AtomicU32::new(den_map),
    });
    let s = shared.clone();
    std::thread::spawn(move || login::serve(login, s, false));
    let s = shared.clone();
    std::thread::spawn(move || world::serve(world, s, false));
    if let Some((rl, rw)) = remote {
        let s = shared.clone();
        std::thread::spawn(move || login::serve(rl, s, true));
        let s = shared.clone();
        std::thread::spawn(move || world::serve(rw, s, true));
    }
    if let Some(ip) = shared.cfg.friends_lan {
        let ports = shared.cfg.friends_lan_ports;
        let _ = open_direct(&shared, ip, ports);
    }
    let s = shared.clone();
    std::thread::spawn(move || world::companion_ticker(s));
    let s = shared.clone();
    std::thread::spawn(move || world::dojo_ticker(s));
    Ok(Running { login: la, world: wa, remote: remote_addrs, shared })
}
