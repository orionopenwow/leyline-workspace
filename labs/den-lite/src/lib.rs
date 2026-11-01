//! Den Lite: a tiny local game server for the 3.3.5a client (build 12340), built for the Leyline launcher's
//! "Your Den". It gives a player an instant white room to build in, with no Docker and no database: a login server
//! on 127.0.0.1:3724, a world server on 127.0.0.1:8085, one ready-made character, the Den's room as gameobjects,
//! stamps, undo and the `leyline-den` save files the full Den (`ov_den.lua`) reads and writes.
//!
//! See README.md for what works, what is verified only by the test client, and what multi-player would need.

pub mod cheers;
pub mod companion;
pub mod den;
pub mod login;
pub mod objects;
pub mod play;
pub mod raw;
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
    pub verbose: bool,
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
            verbose: false,
        }
    }
}

pub struct Shared {
    pub cfg: Config,
    /// Account (upper case) -> the SRP6 session key the login server agreed, for the world server's auth session.
    pub sessions: Mutex<HashMap<String, [u8; 40]>>,
    pub den: Mutex<den::Den>,
    /// The companion (one: Den Lite has one player in the world at a time).
    pub companion: Mutex<companion::Companion>,
    /// The player in the world now: the session's number, its writer and where the player stands.
    pub live: Mutex<Option<(u64, world::Writer, companion::At)>>,
    world: Mutex<SocketAddr>,
    opened: Mutex<std::collections::HashSet<String>>,
}

impl Shared {
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
    /// The realm list's address for the world server.
    pub fn world_address(&self) -> String {
        self.world.lock().unwrap().to_string()
    }
}

pub struct Running {
    pub login: SocketAddr,
    pub world: SocketAddr,
    pub shared: Arc<Shared>,
}

/// Binds both ports and serves them on background threads.
pub fn start(cfg: Config) -> io::Result<Running> {
    let login = TcpListener::bind((cfg.bind, cfg.login_port))?;
    let world = TcpListener::bind((cfg.bind, cfg.world_port))?;
    let (la, wa) = (login.local_addr()?, world.local_addr()?);
    let mut d = den::Den::new(cfg.spot);
    if let Some(Err(e)) = d.reload_ground(&cfg.den_dir) {
        if cfg.verbose {
            eprintln!("den-lite: the ground file isn't used: {e}");
        }
    }
    let shared = Arc::new(Shared {
        den: Mutex::new(d),
        companion: Mutex::new(companion::Companion::restore(&cfg.den_dir)),
        live: Mutex::new(None),
        cfg,
        sessions: Mutex::new(HashMap::new()),
        world: Mutex::new(wa),
        opened: Mutex::new(std::collections::HashSet::new()),
    });
    let s = shared.clone();
    std::thread::spawn(move || login::serve(login, s));
    let s = shared.clone();
    std::thread::spawn(move || world::serve(world, s));
    let s = shared.clone();
    std::thread::spawn(move || world::companion_ticker(s));
    Ok(Running { login: la, world: wa, shared })
}
