//! Single player's Built-in engine (docs/BUILTIN-ENGINE.md; owner's 4 Nov plan, Phase 2: "no Docker"): the same realm
//! the Docker host kit runs (the playerbot fork of AzerothCore, every module, our Lua scripts, the `solo` profile),
//! built natively for Windows by `.github/workflows/engine-windows.yml` and shipped as one zip on the rolling release
//! `engine-latest`, with a portable MySQL 8.4 inside. Docker stays the fallback engine (the Host pack).
//!
//! The launcher's part, all under `<app data>/engine/` ([`Paths`]):
//! * get the zip (its `.sha256` first, then the zip with its progress), check it, unpack it into `dist/` (never game
//!   data: the zip is refused if it holds any), the update path the same way into `dist.new` then swapped;
//! * read the maps from the player's own game folder with the bundled `map_extractor` into `data/` (theirs, on their
//!   PC, never uploaded), with a progress figure;
//! * the database's first run (`mysqld --initialize-insecure` with a generated root password set by `--init-file`),
//!   `dbimport` to fill it, the profile's SQL, the player's account (generated password, SRP6 like
//!   realm/make_account.py) and the realm list row;
//! * start MySQL, the login server and the world server as children bound to 127.0.0.1 only, with the solo profile's
//!   settings passed as the `AC_*` environment the Docker realm gets (the core reads them natively: [`world_env`]);
//!   health from their ports; a clean stop (SOAP `server shutdown`, `mysqladmin shutdown`); logs; back-ups (SQL dumps
//!   of the saves, zipped).
//!
//! The steps are a small state machine ([`plan`]): each is skipped once done, so Start again resumes where a failure
//! stopped. The pure parts (paths, the config mapping, the state machine, the SQL, the SOAP request) have unit tests;
//! starting the real programs is proven on Windows (docs/BUILTIN-ENGINE.md, "What only Windows or CI can prove").

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The rolling release the workflow publishes (as Den Lite's `den-lite-latest`); `LEYLINE_ENGINE_URL` overrides it.
pub const RELEASE: &str = "https://github.com/orionopenwow/leyline-workspace/releases/download/engine-latest/";
/// The zip for Windows (x64), the only one built today.
pub const ASSET: &str = "leyline-engine-win64.zip";
/// The estimate is ~150-250 MB; anything over 1.5 GB isn't it.
pub const ZIP_MAX: u64 = 1536 * 1024 * 1024;
/// The package layout this launcher reads (engine.json `layout`; the workflow's PKG_LAYOUT).
pub const LAYOUT: u64 = 1;
/// Everything listens here only.
pub const ADDRESS: &str = "127.0.0.1";
/// The login server (the client's default port; Den Lite's too: one at a time).
pub const LOGIN_PORT: u16 = 3724;
pub const WORLD_PORT: u16 = 8085;
/// MySQL, not on 3306 so a MySQL the player installed themselves is left alone.
pub const DB_PORT: u16 = 13306;
/// SOAP, only for the clean stop (`server shutdown`).
pub const SOAP_PORT: u16 = 17878;
/// The client's realmlist: with the port, so it is never Den Lite's bare `127.0.0.1` (app.js DEN_LITE,
/// app.rs `den_ground_root`).
pub const REALM: &str = "127.0.0.1:3724";
/// The realm list row's name; Config.wtf's realmName picks it after login.
pub const REALM_NAME: &str = "Single player";
/// The player's login (Config.wtf fills it in on the login screen).
pub const ACCOUNT: &str = "SOLO";
/// The ruleset (realm/profiles/solo.env; host/profiles.rs SOLO).
pub const PROFILE: &str = "solo";
/// The host kit's solo defaults (hosting/kit/tools/wuhost/setup.py PROFILE_BOTS, PROFILE_ENV).
pub const BOTS: u32 = 20;
pub const PLAYER_LIMIT: u32 = 5;
/// The profile's world SQL, once (host-wrath-realm.sh `profile_sql`: solo).
pub const PROFILE_SQL: [&str; 2] = ["main_unbrawl.sql", "modern_combos.sql"];
/// About how many files map_extractor writes into maps/ for a whole game: only for the progress figure (to confirm on
/// a real game folder; the figure stops at 99 until the extractor says it is done).
pub const MAP_FILES_ABOUT: u64 = 5_000;
const KEEP_BACKUPS: usize = 10;
const EXE: &str = if cfg!(windows) { ".exe" } else { "" };

/// Built for this kind of PC (the workflow makes the Windows x64 zip only).
pub fn supported_on(os: &str, arch: &str) -> bool {
    os == "windows" && arch == "x86_64"
}

pub fn supported() -> bool {
    supported_on(std::env::consts::OS, std::env::consts::ARCH)
}

/// The release folder: `LEYLINE_ENGINE_URL` when it is an https folder (testing a build), else [`RELEASE`].
pub fn release_from(over: Option<&str>) -> String {
    match over.map(str::trim) {
        Some(u) if u.starts_with("https://") && u.ends_with('/') && !u.contains(char::is_whitespace) => u.to_string(),
        _ => RELEASE.to_string(),
    }
}

fn release() -> String {
    release_from(std::env::var("LEYLINE_ENGINE_URL").ok().as_deref())
}

// ------------------------------------------------------------------------------------------------ the two realms

/// Which realm the engine runs: Single player (`solo`, `<app>/engine/`) or Boss Rush's exact engine (`bossrush`,
/// `<app>/engine-bossrush/`, docs/BOSS-RUSH.md): the same engine files (Boss Rush uses Single player's `dist/`, so
/// nothing is downloaded twice), but its own maps (only the raid maps), database, saves, login and logs. One runs
/// at a time: both use this PC's login, world and database ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Single,
    BossRush,
    /// The original world, single player (docs/SINGLE-PLAYER-ONE-CLICK.md, realm/profiles/solovanilla.env): Single
    /// player's companions and dungeon buddies with Vanilla+'s rules (the first era held by mod-individual-progression,
    /// level cap 60). Its own database and saves (`<app>/engine-vanilla/`), Single player's maps (the same full read,
    /// done once for both) and engine files.
    Vanilla,
}

/// The local worlds, in the order the Characters page lists them.
pub const KINDS: [Kind; 3] = [Kind::Single, Kind::Vanilla, Kind::BossRush];

/// The original world's realm list row and login (TODO(owner): his name for it; solovanilla.env OV_REALM_NAME too).
pub const VANILLA_REALM_NAME: &str = "Original world";
pub const VANILLA_ACCOUNT: &str = "ORIGINAL";

impl Kind {
    pub fn profile(self) -> &'static str {
        match self {
            Kind::Single => PROFILE,
            Kind::BossRush => "bossrush",
            Kind::Vanilla => "solovanilla",
        }
    }
    pub fn account(self) -> &'static str {
        match self {
            Kind::Single => ACCOUNT,
            Kind::BossRush => crate::host::bossrush::ACCOUNT,
            Kind::Vanilla => VANILLA_ACCOUNT,
        }
    }
    /// Its realm list row's name (Config.wtf picks it by name).
    pub fn realm_name(self) -> &'static str {
        match self {
            Kind::Single => REALM_NAME,
            // TODO(owner): his name for it (realm/profiles/bossrush.env OV_REALM_NAME says the same)
            Kind::BossRush => "Boss Rush",
            Kind::Vanilla => VANILLA_REALM_NAME,
        }
    }
    /// What the pages call it.
    pub fn title(self) -> &'static str {
        match self {
            Kind::Single => "Single player",
            Kind::BossRush => "Boss Rush",
            Kind::Vanilla => "Original world",
        }
    }
    pub fn folder(self) -> &'static str {
        match self {
            Kind::Single => "engine",
            Kind::BossRush => "engine-bossrush",
            Kind::Vanilla => "engine-vanilla",
        }
    }
    /// The progress line's `kind` (single.js reads "engine").
    pub fn job_kind(self) -> &'static str {
        match self {
            Kind::Single => "engine",
            Kind::BossRush => "bossrush",
            Kind::Vanilla => "vanilla",
        }
    }
    pub fn bots(self) -> u32 {
        match self {
            // a quiet world until the player asks for more (owner 12 Nov; host/spsettings.rs): BOTS is what worlds set
            // up before then had, kept for them in their realm.env
            Kind::Single => 0,
            // no roaming population: companions and dungeon buddies only (the profile's WU_BOTS says the same)
            Kind::BossRush | Kind::Vanilla => 0,
        }
    }
    pub fn player_limit(self) -> u32 {
        match self {
            Kind::Single => PLAYER_LIMIT,
            Kind::BossRush => 2,
            Kind::Vanilla => PLAYER_LIMIT,
        }
    }
    pub fn profile_sql(self) -> &'static [&'static str] {
        match self {
            Kind::Single => &PROFILE_SQL,
            // no modern race and class combos in the original world
            Kind::BossRush | Kind::Vanilla => &[],
        }
    }
    /// Only these maps are read from the game folder (None: every map).
    pub fn only_maps(self) -> Option<Vec<u32>> {
        match self {
            Kind::Single | Kind::Vanilla => None,
            Kind::BossRush => Some(crate::host::bossrush::maps()),
        }
    }
    /// Its profile is layered after modules.env (its own lines win; Single player's profile comes first, as the
    /// Docker kit does).
    pub fn profile_last(self) -> bool {
        matches!(self, Kind::BossRush | Kind::Vanilla)
    }
    /// Its maps are Single player's own (`<app>/engine/data`): the same whole read, made once for both.
    pub fn shares_maps(self) -> bool {
        self == Kind::Vanilla
    }
    /// The word the launcher's commands take (`world_*`, the Characters page).
    pub fn word(self) -> &'static str {
        match self {
            Kind::Single => "single",
            Kind::BossRush => "bossrush",
            Kind::Vanilla => "vanilla",
        }
    }
    pub fn from_word(w: &str) -> Option<Kind> {
        KINDS.into_iter().find(|k| k.word() == w.trim())
    }
}

/// The realm this launcher last started (None: none, or it was stopped).
static ACTIVE: Mutex<Option<Kind>> = Mutex::new(None);

pub fn active() -> Option<Kind> {
    ACTIVE.lock().ok().and_then(|g| *g)
}

/// The other realm is the one running on these ports now: this launcher started it, or its own database's pid
/// file names a live process (a launcher restarted while it ran) and this one's doesn't.
pub fn other_running(p: &Paths) -> Option<Kind> {
    if !port_answers(DB_PORT) && !port_answers(WORLD_PORT) {
        return None;
    }
    if let Some(a) = active() {
        return (a != p.kind).then_some(a);
    }
    if read_pid_alive(&p.pid("mysqld")) {
        return None;
    }
    KINDS.into_iter().filter(|k| *k != p.kind).find(|k| read_pid_alive(&Paths::for_kind(&p.app, *k).pid("mysqld")))
}

fn set_active(k: Option<Kind>) {
    if let Ok(mut g) = ACTIVE.lock() {
        *g = k;
    }
}

// ------------------------------------------------------------------------------------------------ paths

/// Where everything is, under `<app data>/engine/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub kind: Kind,
    pub app: PathBuf,
    pub root: PathBuf,
    /// The unpacked release (replaced on update; nothing of the player's in it).
    pub dist: PathBuf,
    /// dbc/, maps/, Cameras/ from the player's own game folder (kept across updates).
    pub data: PathBuf,
    /// MySQL's data folder: the saves.
    pub db: PathBuf,
    pub logs: PathBuf,
    /// state.json, my.ini, client.cnf, realm.env.
    pub state: PathBuf,
    pub backups: PathBuf,
    pub tmp: PathBuf,
}

impl Paths {
    pub fn new(app_dir: &Path) -> Paths {
        Paths::for_kind(app_dir, Kind::Single)
    }
    /// Boss Rush's install shares Single player's unpacked engine (`<app>/engine/dist`).
    pub fn for_kind(app_dir: &Path, kind: Kind) -> Paths {
        let root = app_dir.join(kind.folder());
        Paths {
            kind,
            app: app_dir.to_path_buf(),
            dist: app_dir.join(Kind::Single.folder()).join("dist"),
            data: if kind.shares_maps() { app_dir.join(Kind::Single.folder()).join("data") } else { root.join("data") },
            db: root.join("db"),
            logs: root.join("logs"),
            state: root.join("state"),
            backups: root.join("backups"),
            tmp: root.join("tmp"),
            root,
        }
    }
    pub fn bin(&self) -> PathBuf {
        self.dist.join("bin")
    }
    fn prog(dir: PathBuf, name: &str) -> PathBuf {
        dir.join(format!("{name}{EXE}"))
    }
    pub fn worldserver(&self) -> PathBuf {
        Self::prog(self.bin(), "worldserver")
    }
    pub fn authserver(&self) -> PathBuf {
        Self::prog(self.bin(), "authserver")
    }
    pub fn dbimport(&self) -> PathBuf {
        Self::prog(self.bin(), "dbimport")
    }
    pub fn extractor(&self) -> PathBuf {
        Self::prog(self.dist.join("tools"), "map_extractor")
    }
    fn mysql_bin(&self) -> PathBuf {
        self.dist.join("mysql").join("bin")
    }
    pub fn mysqld(&self) -> PathBuf {
        Self::prog(self.mysql_bin(), "mysqld")
    }
    pub fn mysql(&self) -> PathBuf {
        Self::prog(self.mysql_bin(), "mysql")
    }
    pub fn mysqladmin(&self) -> PathBuf {
        Self::prog(self.mysql_bin(), "mysqladmin")
    }
    pub fn mysqldump(&self) -> PathBuf {
        Self::prog(self.mysql_bin(), "mysqldump")
    }
    /// The core's `SourceDirectory`: data/sql and modules/<name>/data/sql.
    pub fn source(&self) -> PathBuf {
        self.dist.join("source")
    }
    pub fn lua(&self) -> PathBuf {
        self.dist.join("lua_scripts")
    }
    pub fn profile_env(&self, profile: &str) -> PathBuf {
        self.dist.join("realm").join("profiles").join(format!("{profile}.env"))
    }
    pub fn modules_env(&self) -> PathBuf {
        self.dist.join("realm").join("modules.env")
    }
    pub fn profile_sql(&self, name: &str) -> PathBuf {
        self.dist.join("realm").join("sql").join(name)
    }
    pub fn state_file(&self) -> PathBuf {
        self.state.join("state.json")
    }
    pub fn my_ini(&self) -> PathBuf {
        self.state.join("my.ini")
    }
    pub fn client_cnf(&self) -> PathBuf {
        self.state.join("client.cnf")
    }
    /// The player's own settings over the profile's (the host kit's state/realm.env), optional.
    pub fn realm_env(&self) -> PathBuf {
        self.state.join("realm.env")
    }
    pub fn maps_marker(&self) -> PathBuf {
        self.data.join("leyline-maps.json")
    }
    pub fn pid(&self, which: &str) -> PathBuf {
        self.state.join(format!("{which}.pid"))
    }
}

/// What must be in an unpacked release before it is used.
pub fn required(p: &Paths) -> Vec<PathBuf> {
    vec![
        p.worldserver(),
        p.authserver(),
        p.dbimport(),
        p.extractor(),
        p.mysqld(),
        p.mysql(),
        p.mysqladmin(),
        p.mysqldump(),
        p.source().join("data").join("sql").join("base"),
        p.profile_env(p.kind.profile()),
        p.modules_env(),
        p.bin().join("configs").join("worldserver.conf.dist"),
        p.bin().join("configs").join("authserver.conf.dist"),
        p.bin().join("configs").join("dbimport.conf.dist"),
        p.dist.join("engine.json"),
    ]
}

// ------------------------------------------------------------------------------------------------ state

/// `state/state.json`: the generated secrets and what is done. The database password never leaves this file, the
/// two .cnf/.ini files beside it and the servers' environment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub db_password: String,
    pub account: String,
    /// The player's own login password (shown to them on the Manage card, like Den Lite's).
    pub password: String,
    /// The installed zip's sha256 and engine.json version.
    pub sha256: String,
    pub version: String,
    /// The release's sha256 when last looked (engine_update_check).
    pub latest_sha256: String,
    pub db_made: bool,
    pub filled: bool,
    pub account_made: bool,
    /// The game folder the maps were read from.
    pub maps_from: String,
    /// Boss Rush: the hero account's login password (host/bossrush.rs `account_lines`; made once, kept here).
    pub hero_password: String,
    /// A setup was started and hasn't finished (the launcher closed midway, or a step failed): the next launch offers
    /// to go on (`world_resume_pending`). `setup_client`: the game folder's id it was started with.
    pub setup_pending: bool,
    pub setup_client: String,
    /// The world's databases began filling (dbimport ran at least once). Filling again after a stop midway starts
    /// from empty databases, never trusting half a fill (only before the login is made: nothing of the player's).
    pub fill_started: bool,
    /// The character last played here (a pick, Quick start's new one, or the last one to log out when read).
    pub last_character: String,
    /// The Hero Key logins that played this world on this PC (oneclick::hero_name): their characters are the player's.
    pub hero_accounts: Vec<String>,
    /// The Hero Key login the world's own login's characters moved to, once (empty: not yet).
    pub hero_moved_to: String,
    /// The player chose this world's settings (companions, dungeon buddies, roaming bots, XP: host/spsettings.rs), in
    /// the "Your world" panel or the Single player page: the panel isn't shown again.
    pub settings_chosen: bool,
    /// The world's settings are in its realm.env (written once: a new world's quiet defaults, or what a world set up
    /// before the settings existed already had).
    pub settings_settled: bool,
}

pub fn load_state(p: &Paths) -> State {
    fs::read_to_string(p.state_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_state(p: &Paths, s: &State) -> Result<(), String> {
    fs::create_dir_all(&p.state).map_err(|e| format!("Couldn't make the engine's folder: {e}"))?;
    crate::settings::write_atomic(&p.state_file(), serde_json::to_string_pretty(s).unwrap_or_default().as_bytes())
        .map_err(|e| format!("Couldn't save the engine's state: {e}"))
}

const ABC: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// `n` letters and digits from the system's random source (no look-alikes: no I, O, 0, 1).
pub fn secret(n: usize) -> Result<String, String> {
    use ring::rand::SecureRandom;
    let mut b = vec![0u8; n];
    ring::rand::SystemRandom::new().fill(&mut b).map_err(|_| "no random source on this PC".to_string())?;
    Ok(b.iter().map(|x| ABC[*x as usize % ABC.len()] as char).collect())
}

/// The secrets made once (true when something was made).
pub fn ensure_secrets(s: &mut State) -> Result<bool, String> {
    ensure_secrets_for(s, Kind::Single)
}

/// The same for one of the two realms (its own login name).
pub fn ensure_secrets_for(s: &mut State, kind: Kind) -> Result<bool, String> {
    let mut made = false;
    if s.db_password.len() < 16 {
        s.db_password = secret(24)?;
        made = true;
    }
    if s.account.is_empty() {
        s.account = kind.account().into();
        made = true;
    }
    if s.password.len() < 8 {
        s.password = secret(8)?;
        made = true;
    }
    Ok(made)
}

// ------------------------------------------------------------------------------------------------ the config mapping

/// The core's own `IniKeyToEnvVarKey` (src/common/Configuration/Config.cpp): `vmap.enableLOS` is `VMAP_ENABLE_LOS`.
/// The core reads `AC_` + this from the environment before its .conf, on every platform.
pub fn ini_to_env(key: &str) -> String {
    let b: Vec<char> = key.chars().collect();
    let mut out = String::new();
    for i in 0..b.len() {
        let c = b[i];
        if c == ' ' || c == '.' || c == '-' {
            out.push('_');
            continue;
        }
        if i + 1 < b.len() {
            let n = b[i + 1];
            let (cn, nn) = (c.is_ascii_digit(), n.is_ascii_digit());
            if (!c.is_ascii_uppercase() && n.is_ascii_uppercase()) || (!cn && nn) || (cn && !nn) {
                out.push(c.to_ascii_uppercase());
                out.push('_');
                continue;
            }
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

/// `AC_` + [`ini_to_env`].
pub fn env_name(key: &str) -> String {
    format!("AC_{}", ini_to_env(key))
}

/// The lines of an env file as Docker Compose's `env_file` reads them: `KEY=VALUE`, comments and blanks skipped,
/// a value in matching quotes unquoted.
pub fn parse_env(text: &str) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for raw in text.lines() {
        let l = raw.trim_end_matches('\r').trim_start();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let l = l.strip_prefix("export ").unwrap_or(l);
        let Some((k, val)) = l.split_once('=') else { continue };
        let k = k.trim();
        if k.is_empty() || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            continue;
        }
        let val = val.trim();
        let val = if val.len() >= 2 && ((val.starts_with('"') && val.ends_with('"')) || (val.starts_with('\'') && val.ends_with('\''))) {
            &val[1..val.len() - 1]
        } else {
            val
        };
        v.push((k.to_string(), val.to_string()));
    }
    v
}

/// Set `k` (a later layer wins, as Compose's later env_file and its `environment:` do).
fn put(v: &mut Vec<(String, String)>, k: &str, val: impl Into<String>) {
    let val = val.into();
    if let Some(e) = v.iter_mut().find(|(x, _)| x == k) {
        e.1 = val;
    } else {
        v.push((k.to_string(), val));
    }
}

fn get<'a>(v: &'a [(String, String)], k: &str) -> Option<&'a str> {
    v.iter().find(|(x, _)| x == k).map(|(_, y)| y.as_str())
}

/// A path as the core and MySQL take it on Windows too (forward slashes work everywhere).
fn fwd(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// The core's database login for `db`: host;port;user;password;database.
pub fn db_info(s: &State, db: &str) -> String {
    format!("{ADDRESS};{DB_PORT};root;{};{db}", s.db_password)
}

/// What every program of the core gets: the databases, its logs, the SQL tree and the mysql client (the updater).
fn common_env(p: &Paths, s: &State, v: &mut Vec<(String, String)>) {
    put(v, &env_name("LoginDatabaseInfo"), db_info(s, "acore_auth"));
    put(v, &env_name("WorldDatabaseInfo"), db_info(s, "acore_world"));
    put(v, &env_name("CharacterDatabaseInfo"), db_info(s, "acore_characters"));
    put(v, &env_name("LogsDir"), fwd(&p.logs));
    put(v, &env_name("TempDir"), fwd(&p.tmp));
    put(v, &env_name("SourceDirectory"), fwd(&p.source()));
    put(v, &env_name("MySQLExecutable"), fwd(&p.mysql()));
    put(v, &env_name("Updates.AutoSetup"), "1");
    put(v, &env_name("Updates.EnableDatabases"), "7");
    put(v, &env_name("BindIP"), ADDRESS);
}

/// The world server's environment: the profile (`solo.env`), then `modules.env`, then the player's `realm.env`, then
/// what docker-compose.host.yml's `environment:` block sets, for this PC: the four databases, the data folder, the
/// kit's solo defaults (bots, player cap), pathing and Warden off, the Lua scripts' folder, 127.0.0.1 only, no console
/// window, SOAP on 127.0.0.1 for the clean stop. `realm` may set `WU_BOTS` and `OV_PATHING` as the kit's .env does.
pub fn world_env(p: &Paths, s: &State, profile: &str, modules: &str, realm: Option<&str>) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    let (first, second) = if p.kind.profile_last() { (modules, profile) } else { (profile, modules) };
    for (k, val) in parse_env(first).into_iter().chain(parse_env(second)).chain(realm.map(parse_env).unwrap_or_default()) {
        put(&mut v, &k, val);
    }
    let bots = get(&v, "WU_BOTS").and_then(|b| b.parse::<u32>().ok()).unwrap_or(p.kind.bots()).min(500);
    let pathing = if get(&v, "OV_PATHING") == Some("1") { "1" } else { "0" };
    common_env(p, s, &mut v);
    put(&mut v, "AC_PLAYERBOTS_DATABASE_INFO", db_info(s, "acore_playerbots"));
    put(&mut v, &env_name("DataDir"), fwd(&p.data));
    put(&mut v, &env_name("CloseIdleConnections"), "0");
    put(&mut v, "AC_PLAYERBOTS_MIN_RANDOM_BOTS", bots.to_string());
    put(&mut v, "AC_PLAYERBOTS_MAX_RANDOM_BOTS", bots.to_string());
    for k in ["vmap.enableLOS", "vmap.enableHeight", "vmap.petLOS", "MoveMaps.Enable"] {
        put(&mut v, &env_name(k), pathing);
    }
    put(&mut v, &env_name("Warden.Enabled"), "0");
    put(&mut v, &env_name("PlayerLimit"), p.kind.player_limit().to_string());
    put(&mut v, "AC_ALE_SCRIPT_PATH", fwd(&p.lua()));
    // the realm script for the in-game Leyline tab of the Friends window (lua_scripts/ov_worldfriends.lua): only here
    put(&mut v, "OV_WORLDFRIENDS", "1");
    put(&mut v, &env_name("WorldServerPort"), WORLD_PORT.to_string());
    put(&mut v, &env_name("RealmID"), "1");
    put(&mut v, &env_name("Console.Enable"), "0");
    put(&mut v, &env_name("Ra.Enable"), "0");
    put(&mut v, &env_name("SOAP.Enabled"), "1");
    put(&mut v, &env_name("SOAP.IP"), ADDRESS);
    put(&mut v, &env_name("SOAP.Port"), SOAP_PORT.to_string());
    put(&mut v, &env_name("PidFile"), fwd(&p.pid("world")));
    v.retain(|(k, _)| k != "WU_BOTS");
    v
}

/// The login server's environment (and the kit's: wrong passwords lock the account, not the address).
pub fn auth_env(p: &Paths, s: &State) -> Vec<(String, String)> {
    let mut v = Vec::new();
    common_env(p, s, &mut v);
    put(&mut v, &env_name("Updates.EnableDatabases"), "1");
    put(&mut v, &env_name("RealmServerPort"), LOGIN_PORT.to_string());
    put(&mut v, &env_name("PidFile"), fwd(&p.pid("auth")));
    put(&mut v, &env_name("WrongPass.MaxCount"), "10");
    put(&mut v, &env_name("WrongPass.BanTime"), "600");
    put(&mut v, &env_name("WrongPass.BanType"), "1");
    v
}

/// dbimport's (the Docker realm's db-import run): every database, created when missing.
pub fn dbimport_env(p: &Paths, s: &State) -> Vec<(String, String)> {
    let mut v = Vec::new();
    common_env(p, s, &mut v);
    put(&mut v, &env_name("Updates.AllowedModules"), "all");
    v
}

/// MySQL's settings: 127.0.0.1 only, its own port, no X protocol, no name lookups, no binary log (one PC), a small
/// buffer pool (the kit's solo uses the 512 MB default on a server; a gaming PC wants less).
pub fn my_ini(p: &Paths) -> String {
    format!(
        "[mysqld]\nbasedir={}\ndatadir={}\nport={DB_PORT}\nbind-address={ADDRESS}\nmysqlx=OFF\nskip-name-resolve\nskip-log-bin\n\
         innodb_buffer_pool_size=256M\nmax_connections=64\ncharacter-set-server=utf8mb4\nlog-error={}\npid-file={}\n",
        fwd(&p.dist.join("mysql")),
        fwd(&p.db),
        fwd(&p.logs.join("mysqld.err")),
        fwd(&p.pid("mysqld")),
    )
}

/// The mysql client's login (mysql, mysqladmin, mysqldump `--defaults-extra-file`): never on a command line.
pub fn client_cnf(s: &State) -> String {
    format!("[client]\nuser=root\npassword={}\nhost={ADDRESS}\nport={DB_PORT}\nprotocol=TCP\n", s.db_password)
}

/// Run once by `mysqld --initialize-insecure --init-file`: root gets the generated password, for 127.0.0.1 too
/// (skip-name-resolve: a TCP login from 127.0.0.1 is that host, not localhost).
pub fn init_sql(s: &State) -> String {
    let pw = &s.db_password;
    format!(
        "ALTER USER 'root'@'localhost' IDENTIFIED BY '{pw}';\nCREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED BY '{pw}';\n\
         GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION;\nFLUSH PRIVILEGES;\n"
    )
}

/// The realm list's one row: this PC, the world port, its name.
pub fn realm_sql() -> String {
    realm_sql_named(REALM_NAME)
}

/// The same row with another name (Boss Rush's: Kind::realm_name, letters and spaces only).
pub fn realm_sql_named(name: &str) -> String {
    let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').take(32).collect();
    format!(
        "UPDATE realmlist SET name = '{name}', address = '{ADDRESS}', localAddress = '{ADDRESS}', port = {WORLD_PORT}, gamebuild = 12340 WHERE id = 1;\n"
    )
}

// ------------------------------------------------------------------------------------------------ the account (SRP6)

/// The SRP6 group the core uses (realm/make_account.py N, g = 7).
const N_HEX: &str = "894B645E89E1535BBDAD5B8B290650530801B18EBFBF5E8FAB3C82872A3E9BB7";

/// A tiny unsigned big number, 32-bit limbs, least significant first: just enough for one modpow.
fn big_from_be(b: &[u8]) -> Vec<u32> {
    let mut v: Vec<u32> = b.rchunks(4).map(|c| c.iter().fold(0u32, |a, x| (a << 8) | *x as u32)).collect();
    trim(&mut v);
    v
}

fn trim(v: &mut Vec<u32>) {
    while v.len() > 1 && *v.last().unwrap() == 0 {
        v.pop();
    }
}

fn big_cmp(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
    let (la, lb) = (a.iter().rposition(|x| *x != 0).map_or(0, |i| i + 1), b.iter().rposition(|x| *x != 0).map_or(0, |i| i + 1));
    if la != lb {
        return la.cmp(&lb);
    }
    for i in (0..la).rev() {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    std::cmp::Ordering::Equal
}

fn big_sub(a: &mut Vec<u32>, b: &[u32]) {
    let mut borrow = 0i64;
    for i in 0..a.len() {
        let d = a[i] as i64 - borrow - *b.get(i).unwrap_or(&0) as i64;
        borrow = if d < 0 { 1 } else { 0 };
        a[i] = (d + (borrow << 32)) as u32;
    }
    trim(a);
}

fn big_mul(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut r = vec![0u32; a.len() + b.len()];
    for (i, x) in a.iter().enumerate() {
        let mut carry = 0u64;
        for (j, y) in b.iter().enumerate() {
            let t = r[i + j] as u64 + *x as u64 * *y as u64 + carry;
            r[i + j] = t as u32;
            carry = t >> 32;
        }
        r[i + b.len()] = carry as u32;
    }
    trim(&mut r);
    r
}

/// a mod n, bit by bit (numbers of a few hundred bits: fast enough).
fn big_rem(a: &[u32], n: &[u32]) -> Vec<u32> {
    let mut r: Vec<u32> = vec![0];
    for i in (0..a.len() * 32).rev() {
        // r = r * 2 + bit
        let mut carry = (a[i / 32] >> (i % 32)) & 1;
        for x in r.iter_mut() {
            let nc = *x >> 31;
            *x = (*x << 1) | carry;
            carry = nc;
        }
        if carry != 0 {
            r.push(carry);
        }
        if big_cmp(&r, n) != std::cmp::Ordering::Less {
            big_sub(&mut r, n);
        }
    }
    r
}

fn modpow(g: u32, exp_le: &[u8], n: &[u32]) -> Vec<u32> {
    let mut result: Vec<u32> = vec![1];
    let base = big_rem(&[g], n);
    for byte in exp_le.iter().rev() {
        for bit in (0..8).rev() {
            result = big_rem(&big_mul(&result, &result), n);
            if (byte >> bit) & 1 == 1 {
                result = big_rem(&big_mul(&result, &base), n);
            }
        }
    }
    result
}

/// The account's SRP6 verifier, little-endian, 32 bytes (the core's `account.verifier`), exactly as
/// realm/make_account.py: x = SHA1(salt | SHA1(USER ":" PASS)) read little-endian, v = g^x mod N.
pub fn verifier(user: &str, pass: &str, salt: &[u8; 32]) -> [u8; 32] {
    use crate::p2p::sha1::digest;
    let h1 = digest(format!("{}:{}", user.to_ascii_uppercase(), pass.to_ascii_uppercase()).as_bytes());
    let mut buf = salt.to_vec();
    buf.extend_from_slice(&h1);
    let x = digest(&buf); // little-endian
    let n_be: Vec<u8> = (0..N_HEX.len()).step_by(2).map(|i| u8::from_str_radix(&N_HEX[i..i + 2], 16).unwrap()).collect();
    let v = modpow(7, &x, &big_from_be(&n_be));
    let mut out = [0u8; 32];
    for (i, limb) in v.iter().enumerate().take(8) {
        out[i * 4..i * 4 + 4].copy_from_slice(&limb.to_le_bytes());
    }
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The player's account (GM level 3, as the host kit's GM login: SOAP's clean stop needs it), made or its password
/// set again; its characters are kept (the row is updated, never deleted).
pub fn account_sql(user: &str, pass: &str, salt: &[u8; 32]) -> Result<String, String> {
    if !(3..=16).contains(&user.len()) || !user.bytes().all(|b| b.is_ascii_alphanumeric()) || !pass.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err("That login doesn't look right.".into());
    }
    let u = user.to_ascii_uppercase();
    let v = verifier(&u, pass, salt);
    Ok(format!(
        "INSERT INTO account (username, salt, verifier, expansion) VALUES ('{u}', 0x{s}, 0x{v}, 2)\n  ON DUPLICATE KEY UPDATE salt = VALUES(salt), verifier = VALUES(verifier), expansion = 2;\n\
         REPLACE INTO account_access (id, gmlevel, RealmID) SELECT id, 3, -1 FROM account WHERE username = '{u}';\n",
        s = hex(salt),
        v = hex(&v)
    ))
}

/// Before every Play: the player's login written into the world's database again (a fresh salt and the stored
/// password's verifier, the same row as setup makes, characters kept) and the realm's row pointed back at this PC.
/// A database rebuilt or restored from a backup, a password that drifted, or an account removed by hand would
/// otherwise leave the game at its login screen with "incorrect password"; this heals all of them. Needs the
/// world's database running (it is whenever the world is).
pub fn resync_login(p: &Paths) -> Result<(), String> {
    let s = load_state(p);
    if !s.account_made || s.password.is_empty() {
        return Err(format!("Set {} up first.", p.kind.title()));
    }
    let mut salt = [0u8; 32];
    use ring::rand::SecureRandom;
    ring::rand::SystemRandom::new().fill(&mut salt).map_err(|_| "no random source on this PC".to_string())?;
    sql(p, "acore_auth", &format!("{}{}", account_sql(&s.account, &s.password, &salt)?, realm_sql_named(p.kind.realm_name())))
}

// ------------------------------------------------------------------------------------------------ SOAP (the clean stop)

pub fn base64(b: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                o.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                o.push('=');
            }
        }
    }
    o
}

/// The HTTP request for one console command over the core's SOAP (urn:AC executeCommand).
pub fn soap_request(user: &str, pass: &str, command: &str) -> String {
    let esc = command.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         xmlns:ns1=\"urn:AC\"><SOAP-ENV:Body><ns1:executeCommand><command>{esc}</command></ns1:executeCommand></SOAP-ENV:Body></SOAP-ENV:Envelope>"
    );
    format!(
        "POST / HTTP/1.1\r\nHost: {ADDRESS}:{SOAP_PORT}\r\nAuthorization: Basic {}\r\nContent-Type: text/xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        base64(format!("{user}:{pass}").as_bytes()),
        body.len()
    )
}

fn soap(s: &State, command: &str) -> Result<String, String> {
    let addr: SocketAddr = ([127, 0, 0, 1], SOAP_PORT).into();
    let mut c = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|e| e.to_string())?;
    let _ = c.set_read_timeout(Some(Duration::from_secs(10)));
    c.write_all(soap_request(&s.account, &s.password, command).as_bytes()).map_err(|e| e.to_string())?;
    let mut out = String::new();
    let _ = c.read_to_string(&mut out);
    if out.starts_with("HTTP/1.1 200") || out.starts_with("HTTP/1.0 200") {
        Ok(out)
    } else {
        Err(out.lines().next().unwrap_or("no answer").to_string())
    }
}

// ------------------------------------------------------------------------------------------------ the state machine

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Download,
    Unpack,
    Maps,
    Database,
    Fill,
    Account,
    Start,
    /// Repair only: every engine file checked (and the broken or missing ones fetched again).
    Verify,
    /// Repair only: the login written again and the sign-in given another chance.
    Login,
    /// Repair only: the login, world and database ports answer.
    Ports,
}

impl Step {
    /// What the page shows for it (single.js's progress line).
    pub fn label(self) -> &'static str {
        match self {
            Step::Download => "Getting the engine (one-time download)",
            Step::Unpack => "Unpacking the engine (one time)",
            Step::Maps => "Reading the maps from your game folder (one time)",
            Step::Database => "Making the world's database (one time)",
            Step::Fill => "Filling the world: placing creatures, quests and items into its database (one time)",
            Step::Account => "Creating your local login",
            Step::Start => "Starting your world",
            Step::Verify => "Checking the engine's files",
            Step::Login => "Setting up your login again",
            Step::Ports => "Checking the world answers",
        }
    }
}

/// What is done already.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Have {
    pub installed: bool,
    /// An update is wanted (and the release has a newer one).
    pub update: bool,
    pub maps: bool,
    pub db: bool,
    pub filled: bool,
    pub account: bool,
    pub running: bool,
}

/// The steps still to do, in order. An update stops the servers, so Start follows it.
pub fn plan(h: &Have) -> Vec<Step> {
    let mut v = Vec::new();
    if !h.installed || h.update {
        v.extend([Step::Download, Step::Unpack]);
    }
    if !h.maps {
        v.push(Step::Maps);
    }
    if !h.db {
        v.push(Step::Database);
    }
    if !h.filled {
        v.push(Step::Fill);
    }
    if !h.account {
        v.push(Step::Account);
    }
    if !h.running || !v.is_empty() {
        v.push(Step::Start);
    }
    v
}

/// Set up: everything but running.
pub fn set_up(h: &Have) -> bool {
    h.installed && h.maps && h.db && h.filled && h.account
}

pub fn installed(p: &Paths) -> bool {
    required(p).iter().all(|f| f.exists())
}

pub fn maps_done(p: &Paths) -> bool {
    p.maps_marker().is_file() && p.data.join("dbc").is_dir() && p.data.join("maps").is_dir()
}

/// MySQL's data folder has been made (its own `mysql` schema is there).
pub fn db_made(p: &Paths) -> bool {
    p.db.join("mysql").is_dir() || p.db.join("mysql.ibd").is_file()
}

pub fn have(p: &Paths, s: &State, update: bool) -> Have {
    // a database folder that's gone (removed by hand, a clean-up tool) takes its fill and login with it
    let db = s.db_made && db_made(p);
    Have {
        installed: installed(p),
        update: update && !s.latest_sha256.is_empty() && s.latest_sha256 != s.sha256,
        maps: maps_done(p),
        db,
        filled: s.filled && db,
        account: s.account_made && db,
        running: port_answers(DB_PORT) && port_answers(LOGIN_PORT) && port_answers(WORLD_PORT) && other_running(p).is_none(),
    }
}

// ------------------------------------------------------------------------------------------------ status

#[derive(Debug, Clone, Default, Serialize)]
pub struct EngineStatus {
    /// Built for this PC (Windows x64).
    pub supported: bool,
    pub installed: bool,
    /// Not here yet; Start gets it.
    pub can_get: bool,
    pub version: String,
    /// The release has a newer one (from the last engine_update_check).
    pub update: bool,
    pub maps: bool,
    pub db: bool,
    pub filled: bool,
    pub account_made: bool,
    /// Everything done once: Single player is on this PC (the Manage card).
    pub set_up: bool,
    /// All three answer.
    pub running: bool,
    pub db_up: bool,
    pub auth_up: bool,
    pub world_up: bool,
    /// A setup or start job is going.
    pub busy: bool,
    /// The player's login, once made.
    pub account: String,
    pub password: String,
    pub realm: String,
    pub root: String,
    pub backups: usize,
    /// Which realm this status is of, its realm list name, and the realm that is running now if it is the other one
    /// (both use the same ports: one at a time).
    pub kind: Kind,
    pub realm_name: String,
    pub other_running: Option<Kind>,
}

pub fn status(p: &Paths) -> EngineStatus {
    let s = load_state(p);
    let h = have(p, &s, false);
    let (db_up, auth_up, world_up) = (port_answers(DB_PORT), port_answers(LOGIN_PORT), port_answers(WORLD_PORT));
    // the other realm's servers answering these ports aren't this one's
    let other = other_running(p);
    let (db_up, auth_up, world_up) = if other.is_some() { (false, false, false) } else { (db_up, auth_up, world_up) };
    let su = set_up(&h);
    EngineStatus {
        supported: supported(),
        installed: h.installed,
        can_get: !h.installed && supported(),
        version: s.version.clone(),
        update: h.installed && !s.latest_sha256.is_empty() && s.latest_sha256 != s.sha256,
        maps: h.maps,
        db: h.db,
        filled: h.filled,
        account_made: h.account,
        set_up: su,
        running: su && db_up && auth_up && world_up,
        db_up: su && db_up,
        auth_up: su && auth_up,
        world_up: su && world_up,
        busy: progress().running,
        account: if h.account { s.account.clone() } else { String::new() },
        password: if h.account { s.password.clone() } else { String::new() },
        realm: REALM.into(),
        root: p.root.display().to_string(),
        backups: list_backups(p).len(),
        kind: p.kind,
        realm_name: p.kind.realm_name().into(),
        other_running: other,
    }
}

// ------------------------------------------------------------------------------------------------ the job

/// The page's progress line, the same shape as the Host pack's `host_progress` (single.js reads either).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Job {
    pub idle: bool,
    pub kind: String,
    pub running: bool,
    pub code: Option<i32>,
    pub step: String,
    /// Which step (1-based) of `of`.
    pub n: usize,
    pub of: usize,
    pub secs: u64,
    pub problem: Option<String>,
    /// How far this step is, when it can tell (the download, the maps).
    pub pct: Option<u8>,
    /// Its last line of detail.
    pub detail: String,
}

static JOB: Mutex<Option<(Job, Instant)>> = Mutex::new(None);

pub fn progress() -> Job {
    match JOB.lock().ok().and_then(|g| g.clone()) {
        Some((mut j, t)) => {
            if j.running {
                j.secs = t.elapsed().as_secs();
            }
            j
        }
        None => Job { idle: true, kind: Kind::Single.job_kind().into(), ..Default::default() },
    }
}

fn job(f: impl FnOnce(&mut Job)) {
    if let Ok(mut g) = JOB.lock() {
        if let Some((j, t)) = g.as_mut() {
            f(j);
            if !j.running {
                j.secs = t.elapsed().as_secs();
            }
        }
    }
}

/// Percent of `done` in `total`, below 100 until the step itself says it is done.
pub fn pct(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    ((done.saturating_mul(100) / total).min(99)) as u8
}

/// Start setting up / starting (in the background; follow it with [`progress`]). `game`: the game folder the maps
/// are read from (needed only while they aren't). `update`: take the release's newer engine first.
pub fn begin(app_dir: &Path, game: Option<PathBuf>, update: bool) -> Result<String, String> {
    begin_kind(app_dir, Kind::Single, game, update)
}

/// [`begin`] for one of the two realms. The other one running is said, never stopped here (the page asks first).
pub fn begin_kind(app_dir: &Path, kind: Kind, game: Option<PathBuf>, update: bool) -> Result<String, String> {
    begin_setup(app_dir, kind, game, update, None)
}

/// [`begin_kind`], remembering the game folder's id (`client`) for a setup that has to go on after a restart
/// (`world_resume_pending`). Errors start with their code (oneclick::CODES).
pub fn begin_setup(app_dir: &Path, kind: Kind, game: Option<PathBuf>, update: bool, client: Option<&str>) -> Result<String, String> {
    use crate::host::oneclick::tag;
    let title = kind.title();
    if !supported() {
        return Err(tag("other", match kind {
            Kind::Single => "The Built-in engine is made for Windows PCs for now: Docker runs Single player here.".to_string(),
            Kind::BossRush => "Boss Rush's exact engine is made for Windows PCs for now: the light version runs here.".to_string(),
            Kind::Vanilla => "The Built-in engine is made for Windows PCs for now: Docker runs the original world here.".to_string(),
        }));
    }
    let p = Paths::for_kind(app_dir, kind);
    if let Some(o) = other_running(&p) {
        return Err(tag("ports", format!("{} is running on this PC and uses the same ports. Stop it first, then press Start again.", o.title())));
    }
    let mut s = load_state(&p);
    // the world's settings in its realm.env before its server first reads it (host/spsettings.rs)
    if crate::host::spsettings::settle(&p, &mut s) {
        let _ = save_state(&p, &s);
    }
    let h = have(&p, &s, update);
    let steps = plan(&h);
    if steps.is_empty() {
        return Ok(format!("{title} is running."));
    }
    if steps.contains(&Step::Maps) && game.is_none() {
        return Err(tag("folder", format!("Pick your game folder first: {title} reads its maps from it once.")));
    }
    if steps.iter().any(|x| *x != Step::Start) {
        s.setup_pending = true;
        if let Some(c) = client.filter(|c| !c.is_empty()) {
            s.setup_client = c.to_string();
        }
        if s.maps_from.is_empty() {
            if let Some(g) = &game {
                s.maps_from = g.display().to_string();
            }
        }
        // the state folder may not exist yet on a first setup
        let _ = fs::create_dir_all(&p.state);
        let _ = save_state(&p, &s);
    }
    {
        let mut g = JOB.lock().map_err(|_| "busy")?;
        if g.as_ref().is_some_and(|(j, _)| j.running) {
            return Ok(format!("{} is starting already.", Kind::of_job(&g.as_ref().map(|x| x.0.kind.clone()).unwrap_or_default()).title()));
        }
        let first = steps[0];
        *g = Some((
            Job { idle: false, kind: kind.job_kind().into(), running: true, step: first.label().into(), n: 1, of: steps.len(), ..Default::default() },
            Instant::now(),
        ));
    }
    let msg = if steps.contains(&Step::Download) { "Getting the engine…".to_string() } else { format!("Starting {title}…") };
    std::thread::spawn(move || {
        let r = run(&p, game.as_deref(), &steps);
        if r.is_ok() {
            let mut s = load_state(&p);
            if s.setup_pending {
                s.setup_pending = false;
                let _ = save_state(&p, &s);
            }
        }
        setup_log(&p, &match &r {
            Ok(()) => "done".to_string(),
            Err(e) => format!("stopped: {e}"),
        });
        job(|j| {
            j.running = false;
            j.pct = None;
            match r {
                Ok(()) => {
                    j.code = Some(0);
                    j.step = "Ready".into();
                    j.n = j.of;
                }
                Err(e) => {
                    j.code = Some(1);
                    j.problem = Some(e);
                }
            }
        });
    });
    Ok(msg)
}

/// The steps Repair takes, in order: the engine's files, whatever setup is missing (the maps only when their marker
/// or folders are gone; never the database when it is there), start, the login, the ports.
pub fn repair_plan(h: &Have) -> Vec<Step> {
    let mut v = vec![Step::Verify];
    if !h.maps {
        v.push(Step::Maps);
    }
    if !h.db {
        v.push(Step::Database);
    }
    if !h.filled {
        v.push(Step::Fill);
    }
    if !h.account {
        v.push(Step::Account);
    }
    v.extend([Step::Start, Step::Login, Step::Ports]);
    v
}

/// Repair a world (in the background, on [`progress`] as setup is): stop it (everyone saved), check every engine
/// file and fetch again only what's broken or missing, read the maps again only if they're gone, set up what's
/// missing, start it, write its login again, check its ports. The saves (`db/`) and back-ups are never touched; a
/// database that is there is never filled again.
pub fn begin_repair(app_dir: &Path, kind: Kind, game: Option<PathBuf>) -> Result<String, String> {
    use crate::host::oneclick::tag;
    if !supported() {
        return Err(tag("other", format!("{} runs on Windows PCs for now, so there's nothing to repair here.", kind.title())));
    }
    let p = Paths::for_kind(app_dir, kind);
    if let Some(o) = other_running(&p) {
        return Err(tag("ports", format!("{} is running on this PC and uses the same ports: stop it first, then press Repair again.", o.title())));
    }
    let s = load_state(&p);
    let h = have(&p, &s, false);
    let steps = repair_plan(&h);
    if steps.contains(&Step::Maps) && game.is_none() {
        return Err(tag("folder", format!("Pick your game folder first: {} reads its maps from it again.", kind.title())));
    }
    {
        let mut g = JOB.lock().map_err(|_| tag("other", "Busy: try again in a moment."))?;
        if g.as_ref().is_some_and(|(j, _)| j.running) {
            return Err(tag("other", "A world is setting up or starting already: wait for it, then press Repair."));
        }
        *g = Some((
            Job { idle: false, kind: kind.job_kind().into(), running: true, step: steps[0].label().into(), n: 1, of: steps.len(), ..Default::default() },
            Instant::now(),
        ));
    }
    std::thread::spawn(move || {
        setup_log(&p, "repair");
        let r = repair(&p, game.as_deref(), &steps);
        if r.is_ok() {
            let mut s = load_state(&p);
            s.setup_pending = false;
            let _ = save_state(&p, &s);
        }
        setup_log(&p, &match &r {
            Ok(()) => "repaired".to_string(),
            Err(e) => format!("repair stopped: {e}"),
        });
        job(|j| {
            j.running = false;
            j.pct = None;
            match r {
                Ok(()) => {
                    j.code = Some(0);
                    j.step = "Repaired".into();
                    j.n = j.of;
                }
                Err(e) => {
                    j.code = Some(1);
                    j.problem = Some(e);
                }
            }
        });
    });
    Ok(format!("Repairing {}…", kind.title()))
}

fn repair(p: &Paths, game: Option<&Path>, steps: &[Step]) -> Result<(), String> {
    use crate::host::oneclick::{classify, tag};
    for d in [&p.root, &p.logs, &p.state, &p.tmp] {
        fs::create_dir_all(d).map_err(|e| tag("disk", format!("Couldn't make the engine's folder: {e}")))?;
    }
    heal_partial(p);
    // files in use can't be mended: the world stops first (everyone saved)
    if (port_answers(WORLD_PORT) || port_answers(DB_PORT) || port_answers(LOGIN_PORT)) && other_running(p).is_none() {
        job(|j| j.detail = "Stopping the world first (everyone is saved)".into());
        stop(p).map_err(|e| tag("other", e))?;
    }
    let mut s = load_state(p);
    if ensure_secrets_for(&mut s, p.kind).map_err(|e| tag("other", e))? {
        save_state(p, &s).map_err(|e| tag("disk", e))?;
    }
    let mut want = String::new();
    for (i, step) in steps.iter().enumerate() {
        job(|j| {
            j.n = i + 1;
            j.step = step.label().into();
            j.pct = None;
            j.detail.clear();
        });
        setup_log(p, step.label());
        let r = match step {
            Step::Verify => verify_files(p, &mut s),
            Step::Login => resync_login(p),
            Step::Ports => {
                if port_answers(DB_PORT) && port_answers(LOGIN_PORT) && port_answers(WORLD_PORT) {
                    Ok(())
                } else {
                    Err(tag("ports", format!("{} started but doesn't answer on its ports: another program may be holding them, so restart the PC and press Play.", p.kind.title())))
                }
            }
            other => run_step(p, game, *other, &mut s, &mut want),
        };
        r.map_err(|e| classify(*step, e))?;
    }
    Ok(())
}

/// The engine's files checked: against engine.json's `files` list (size and sha256 of each) when the engine has one;
/// all fine: nothing is downloaded. Otherwise (something broken or missing, or an engine without the list) the
/// release is fetched (checked against its sha256) and only the files that differ or are missing are put back; the
/// rest, and anything of the player's, stay. A release newer than the installed engine is installed whole (an update).
fn verify_files(p: &Paths, s: &mut State) -> Result<(), String> {
    use crate::host::oneclick as oc;
    let list = oc::read_manifest(&p.dist);
    if installed(p) {
        if let Some(m) = &list {
            let bad = oc::manifest_bad(&p.dist, m);
            if bad.is_empty() {
                job(|j| j.detail = format!("All {} files are fine", m.len()));
                return Ok(());
            }
            setup_log(p, &format!("{} engine files broken or missing", bad.len()));
            job(|j| j.detail = format!("{} files to fetch again", bad.len()));
        }
    }
    let want = download(p)?;
    let zip = p.tmp.join(ASSET);
    let latest = crate::host::forge::sha_in(&want).unwrap_or_default();
    if !p.dist.exists() || s.sha256.is_empty() || latest != s.sha256 {
        let r = place(p, &zip, &want, s);
        let _ = fs::remove_file(&zip);
        r?;
        save_state(p, s)?;
    } else {
        let r = unpack_checked(p, &zip, &want);
        let _ = fs::remove_file(&zip);
        let (new, _, _) = r?;
        let mended = oc::mend_from(&new, &p.dist);
        let _ = fs::remove_dir_all(&new);
        let n = mended?;
        make_confs(&p.bin());
        setup_log(p, &format!("{n} engine files put back"));
        job(|j| j.detail = format!("{n} files put back"));
    }
    if let Some(gone) = required(p).into_iter().find(|f| !f.exists()) {
        return Err(oc::tag("antivirus", format!(
            "{} disappeared right after it was put back, which is what a virus scanner does: allow Leyline's engine folder in it, then press Repair.",
            gone.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
        )));
    }
    Ok(())
}

impl Kind {
    /// The realm a progress line's `kind` is of.
    pub fn of_job(k: &str) -> Kind {
        KINDS.into_iter().find(|x| x.job_kind() == k).unwrap_or(Kind::Single)
    }
}

/// A line in the world's setup log (logs/setup.log): never a password.
pub fn setup_log(p: &Paths, line: &str) {
    let _ = fs::create_dir_all(&p.logs);
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(p.logs.join("setup.log")) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "{} {line}", stamp(now));
    }
}

fn run(p: &Paths, game: Option<&Path>, steps: &[Step]) -> Result<(), String> {
    use crate::host::oneclick::{classify, tag};
    for d in [&p.root, &p.logs, &p.state, &p.tmp] {
        fs::create_dir_all(d).map_err(|e| tag("disk", format!("Couldn't make the engine's folder: {e}")))?;
    }
    let mut s = load_state(p);
    if ensure_secrets_for(&mut s, p.kind).map_err(|e| tag("other", e))? {
        save_state(p, &s).map_err(|e| tag("disk", e))?;
    }
    heal_partial(p);
    let mut want = String::new();
    for (i, step) in steps.iter().enumerate() {
        job(|j| {
            j.n = i + 1;
            j.step = step.label().into();
            j.pct = None;
            j.detail.clear();
        });
        setup_log(p, step.label());
        run_step(p, game, *step, &mut s, &mut want).map_err(|e| classify(*step, e))?;
    }
    Ok(())
}

/// What a setup stopped midway (the launcher closed, the PC slept) can leave, cleared before going on, so nothing
/// half-done is trusted: a part-downloaded zip and a part-unpacked `dist.new` go (the zip is checked against its
/// sha256 anyway); an engine moved aside for an update but not replaced comes back; a map reader or database filler of
/// ours still running from before is ended (their pid files). Half-read maps (no marker) are read again by
/// `extract_maps`, and a half fill starts from empty databases (`Step::Fill`).
pub fn heal_partial(p: &Paths) {
    let _ = fs::remove_file(p.tmp.join(ASSET));
    let _ = fs::remove_dir_all(p.root.join("dist.new"));
    // Single player's install holds the engine files every world shares
    let single = Paths::for_kind(&p.app, Kind::Single);
    let old = single.root.join("dist.old");
    if !single.dist.exists() && old.is_dir() {
        let _ = fs::rename(&old, &single.dist);
    }
    for (which, program) in [("dbimport", "dbimport"), ("extract", "map_extractor")] {
        let f = p.pid(which);
        if let Some(pid) = read_pid(&f) {
            // only when that process is still ours (a pid number can be reused after a restart)
            if pid_runs(pid, program) {
                kill_pid(pid);
            }
        }
        let _ = fs::remove_file(&f);
    }
}

/// Process `pid` is running and is `program` (its file name without .exe).
fn pid_runs(pid: u32, program: &str) -> bool {
    let want = program.to_ascii_lowercase();
    if cfg!(windows) {
        let mut c = Command::new("tasklist");
        c.args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"]);
        no_window(&mut c);
        c.output().map(|o| String::from_utf8_lossy(&o.stdout).to_ascii_lowercase().contains(&format!("\"{want}.exe\""))).unwrap_or(false)
    } else {
        fs::read_to_string(format!("/proc/{pid}/comm")).map(|n| n.trim().to_ascii_lowercase().starts_with(&want[..want.len().min(15)])).unwrap_or(false)
    }
}

/// One setup step.
fn run_step(p: &Paths, game: Option<&Path>, step: Step, s: &mut State, want: &mut String) -> Result<(), String> {
    match step {
        Step::Download => *want = download(p)?,
        Step::Unpack => {
            if port_answers(WORLD_PORT) || port_answers(DB_PORT) {
                stop(p)?;
            }
            let zip = p.tmp.join(ASSET);
            let r = place(p, &zip, want, s);
            let _ = fs::remove_file(&zip);
            r?;
            save_state(p, s)?;
            // every program was there a moment ago (place checks): one gone now was taken by a virus scanner
            if let Some(gone) = required(p).into_iter().find(|f| !f.exists()) {
                return Err(crate::host::oneclick::tag("antivirus", format!(
                    "{} disappeared right after unpacking, which is what a virus scanner does: allow Leyline's engine folder in it, then press Repair.",
                    gone.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                )));
            }
        }
        Step::Maps => {
            extract_maps(p, game.ok_or("Pick your game folder first.")?)?;
            s.maps_from = game.map(|g| g.display().to_string()).unwrap_or_default();
            save_state(p, s)?;
        }
        Step::Database => {
            init_db(p, s)?;
            s.db_made = true;
            save_state(p, s)?;
        }
        Step::Fill => {
            start_db(p)?;
            if s.fill_started && !s.account_made {
                // a fill that stopped midway: from empty databases again (nothing of the player's is in them yet)
                setup_log(p, "the last fill stopped midway: starting it again from empty databases");
                sql(p, "mysql", &drop_sql())?;
            }
            s.fill_started = true;
            save_state(p, s)?;
            fill(p, s)?;
            s.filled = true;
            save_state(p, s)?;
        }
        Step::Account => {
            start_db(p)?;
            let mut salt = [0u8; 32];
            use ring::rand::SecureRandom;
            ring::rand::SystemRandom::new().fill(&mut salt).map_err(|_| "no random source on this PC".to_string())?;
            sql(p, "acore_auth", &account_sql(&s.account, &s.password, &salt)?)?;
            sql(p, "acore_auth", &realm_sql_named(p.kind.realm_name()))?;
            s.account_made = true;
            save_state(p, s)?;
        }
        Step::Start => start(p, s)?,
        Step::Verify | Step::Login | Step::Ports => {}
    }
    Ok(())
}

/// The world's databases, for a fill that has to start again from empty ones (only ever before the login is made).
pub fn drop_sql() -> String {
    ["acore_world", "acore_characters", "acore_auth", "acore_playerbots"].iter().map(|d| format!("DROP DATABASE IF EXISTS `{d}`;\n")).collect()
}

// ------------------------------------------------------------------------------------------------ get and unpack

/// The download's error in words a player can act on: no release yet (404, hosting audit 5 Nov: `engine-latest` was
/// never published) says to use Docker for now instead of "the author's site answered 404".
pub fn get_error(e: &str) -> String {
    if e.ends_with("answered 404") {
        "Leyline's Built-in engine isn't published yet, so it can't be downloaded. Choose Docker on this page for now.".to_string()
    } else {
        format!("Couldn't get the engine: {e}")
    }
}

/// The release's checksum, then the zip into tmp/ with its progress. The checksum text, for [`place`].
fn download(p: &Paths) -> Result<String, String> {
    let base = release();
    let shaf = p.tmp.join(format!("{ASSET}.sha256"));
    crate::upstream::download(&format!("{base}{ASSET}.sha256"), &shaf, 4096).map_err(|e| get_error(&e))?;
    let want = fs::read_to_string(&shaf).unwrap_or_default();
    let _ = fs::remove_file(&shaf);
    if crate::host::forge::sha_in(&want).is_none() {
        return Err("The engine's checksum file doesn't look right, so nothing was downloaded. Try again later.".into());
    }
    let zip = p.tmp.join(ASSET);
    let last = std::cell::Cell::new(Instant::now().checked_sub(Duration::from_secs(1)).unwrap_or_else(Instant::now));
    crate::upstream::download_progress(&format!("{base}{ASSET}"), &zip, ZIP_MAX, &|got, size| {
        if last.get().elapsed() >= Duration::from_millis(500) {
            last.set(Instant::now());
            job(|j| {
                j.pct = size.map(|t| pct(got, t));
                j.detail = format!("{} MB{}", got / 1_048_576, size.map(|t| format!(" of {} MB", t / 1_048_576)).unwrap_or_default());
            });
        }
    })
    .map_err(|e| format!("The engine didn't download: {e}"))?;
    Ok(want)
}

/// A file type that is game data: never in the engine (the workflow refuses it too).
pub fn is_game_data(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [".dbc", ".db2", ".mpq", ".m2", ".wmo", ".adt", ".blp", ".vmtree", ".mmap", ".map", ".vmtile"].iter().any(|e| n.ends_with(e))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
}

/// engine.json's version, when the layout is ours.
pub fn read_meta(dist: &Path) -> Result<String, String> {
    let v: serde_json::Value = fs::read_to_string(dist.join("engine.json")).ok().and_then(|t| serde_json::from_str(&t).ok())
        .ok_or("The engine's download has no engine.json, so it wasn't used.")?;
    if v.get("name").and_then(|n| n.as_str()) != Some("leyline-engine") {
        return Err("That isn't Leyline's engine, so it wasn't used.".into());
    }
    if v.get("layout").and_then(|n| n.as_u64()) != Some(LAYOUT) {
        return Err("This engine is newer than this launcher: update Leyline, then try again.".into());
    }
    Ok(v.get("version").and_then(|n| n.as_str()).unwrap_or("").chars().take(40).collect())
}

/// Each `configs/*.conf.dist` (and `configs/modules/`) copied to its `.conf` once; the environment does the rest.
pub fn make_confs(bin: &Path) -> usize {
    let mut n = 0;
    for dir in [bin.join("configs"), bin.join("configs").join("modules")] {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            let name = path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            if let Some(base) = name.strip_suffix(".dist") {
                let to = dir.join(base);
                if !to.exists() && fs::copy(&path, &to).is_ok() {
                    n += 1;
                }
            }
        }
    }
    n
}

const CAPS: crate::clientimport::Caps = crate::clientimport::Caps { download: ZIP_MAX, unpacked: 6 * 1024 * 1024 * 1024, files: 60_000 };

/// The downloaded zip put in place once its sha256 is the one the release names: unpacked into `dist.new`, checked
/// (no game data, our layout, every program), then swapped with `dist` (the old one removed only after). The
/// player's data, database, state and back-ups are never touched.
pub fn place(p: &Paths, zip: &Path, want_text: &str, s: &mut State) -> Result<(), String> {
    let (new, version, want) = unpack_checked(p, zip, want_text)?;
    let probe = Paths { dist: new.clone(), ..p.clone() };
    make_confs(&probe.bin());
    let old = p.root.join("dist.old");
    let _ = fs::remove_dir_all(&old);
    if p.dist.exists() {
        fs::rename(&p.dist, &old).map_err(|e| format!("Couldn't replace the old engine (is it still running?): {e}"))?;
    }
    if let Err(e) = fs::rename(&new, &p.dist) {
        let _ = fs::rename(&old, &p.dist);
        return Err(format!("Couldn't put the engine in place: {e}"));
    }
    let _ = fs::remove_dir_all(&old);
    s.sha256 = want.clone();
    s.latest_sha256 = want;
    s.version = version;
    Ok(())
}

/// The downloaded zip checked against the release's sha256 and unpacked into `dist.new`, then checked there (no
/// game data, our layout, every program): `(dist.new, its version, its sha256)`. Nothing else is touched.
pub fn unpack_checked(p: &Paths, zip: &Path, want_text: &str) -> Result<(PathBuf, String, String), String> {
    let want = crate::host::forge::sha_in(want_text).ok_or("The engine's checksum file doesn't look right, so nothing was installed.")?;
    let got = crate::net::sha256_file(zip).map_err(|e| format!("Couldn't read the engine's download: {e}"))?;
    if got != want {
        return Err("The engine's download doesn't match its checksum, so nothing was installed. Try again later.".into());
    }
    let new = p.root.join("dist.new");
    let _ = fs::remove_dir_all(&new);
    crate::clientimport::unzip(zip, &new, &CAPS, &crate::clientimport::Job::default()).map_err(|e| format!("The engine couldn't be unpacked: {e}"))?;
    let refuse = |why: String| {
        let _ = fs::remove_dir_all(&new);
        Err(why)
    };
    let mut files = Vec::new();
    walk(&new, &mut files);
    if let Some(bad) = files.iter().find(|f| is_game_data(&f.to_string_lossy())) {
        return refuse(format!("The engine's download holds a game file ({}), which it never should, so it wasn't used.", bad.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    }
    let version = match read_meta(&new) {
        Ok(v) => v,
        Err(e) => return refuse(e),
    };
    let probe = Paths { dist: new.clone(), ..p.clone() };
    if let Some(missing) = required(&probe).into_iter().find(|f| !f.exists()) {
        return refuse(format!("The engine's download is missing {}, so it wasn't used.", missing.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    }
    Ok((new, version, want))
}

/// The release's .json (engine.json beside the zip, `leyline-engine-win64.json`), for the disk check's sizes.
pub fn release_meta(p: &Paths) -> Option<serde_json::Value> {
    fs::create_dir_all(&p.tmp).ok()?;
    let f = p.tmp.join("release.json");
    let got = crate::upstream::download(&format!("{}{}", release(), ASSET.replace(".zip", ".json")), &f, 4 * 1024 * 1024);
    let text = fs::read_to_string(&f).ok();
    let _ = fs::remove_file(&f);
    got.ok()?;
    serde_json::from_str(&text?).ok()
}

/// The disk check before a setup (docs/SINGLE-PLAYER-ONE-CLICK.md §contract): what's left to do, from the release's
/// .json (only fetched when the engine is still to be downloaded) or the estimates, against the free space where
/// Leyline keeps its worlds.
pub fn preflight(p: &Paths) -> crate::host::oneclick::Preflight {
    use crate::host::oneclick as oc;
    let s = load_state(p);
    let h = have(p, &s, false);
    let meta = if !h.installed { release_meta(p) } else { None };
    let sizes = oc::Sizes::from_json(meta.as_ref());
    let need = oc::need_bytes(&h, &sizes);
    let mut r = oc::preflight(oc::free_bytes(&p.app), need, p.kind.title());
    if !h.installed {
        r.download_mb = sizes.zip.div_ceil(1024 * 1024);
        r.download_known = meta.as_ref().and_then(|m| m.get("zip_bytes")).and_then(|x| x.as_u64()).is_some_and(|z| z > 0);
    }
    r
}

/// The release's checksum now; true when it isn't the installed one.
pub fn check_update(p: &Paths) -> Result<bool, String> {
    fs::create_dir_all(&p.tmp).map_err(|e| e.to_string())?;
    let shaf = p.tmp.join(format!("{ASSET}.sha256"));
    crate::upstream::download(&format!("{}{ASSET}.sha256", release()), &shaf, 4096).map_err(|e| format!("Couldn't look for an engine update: {e}"))?;
    let t = fs::read_to_string(&shaf).unwrap_or_default();
    let _ = fs::remove_file(&shaf);
    let latest = crate::host::forge::sha_in(&t).ok_or("The engine's checksum file doesn't look right.")?;
    let mut s = load_state(p);
    s.latest_sha256 = latest;
    save_state(p, &s)?;
    Ok(installed(p) && s.latest_sha256 != s.sha256)
}

// ------------------------------------------------------------------------------------------------ the maps

/// The game folder has its archives (`Data/*.MPQ`, any case).
pub fn game_ok(game: &Path) -> Result<(), String> {
    let data = fs::read_dir(game).ok().and_then(|rd| rd.flatten().map(|e| e.path()).find(|x| x.is_dir() && x.file_name().is_some_and(|n| n.eq_ignore_ascii_case("data"))));
    let Some(data) = data else { return Err("That game folder has no Data folder, so its maps can't be read. Pick the folder the game itself is in.".into()) };
    let has = fs::read_dir(&data).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().to_ascii_lowercase().ends_with(".mpq"))).unwrap_or(false);
    if has {
        Ok(())
    } else {
        Err("That game folder's Data folder has no game archives, so its maps can't be read.".into())
    }
}

fn count(dir: &Path) -> u64 {
    fs::read_dir(dir).map(|rd| rd.count() as u64).unwrap_or(0)
}

/// map_extractor on the player's own game folder into data/ (dbc, maps, Cameras), as the kit's `client_data`; its
/// lines into logs/extract.log and the page's detail, a % from the files written.
fn extract_maps(p: &Paths, game: &Path) -> Result<(), String> {
    game_ok(game)?;
    fs::create_dir_all(&p.data).map_err(|e| format!("Couldn't make the maps folder: {e}"))?;
    let _ = fs::remove_file(p.maps_marker());
    // no marker: whatever is there is from a read that didn't finish, so it's read again from nothing
    for d in ["maps", "dbc", "Cameras"] {
        let _ = fs::remove_dir_all(p.data.join(d));
    }
    let mut cmd = Command::new(p.extractor());
    cmd.arg("-i").arg(game).arg("-o").arg(&p.data);
    // Boss Rush: only the raid maps (and the start tiles the world server checks), when the engine's extractor
    // takes `-m` (engine.json "map_filter", realm/custom/patches/core.patch); else all of them, trimmed after.
    let only = p.kind.only_maps();
    let filter = only.is_some() && map_filter(&p.dist);
    if filter {
        cmd.arg("-m").arg(crate::host::bossrush::maps_arg());
    }
    cmd.current_dir(&p.data).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    no_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("The map reader didn't start: {e}"))?;
    let _ = fs::write(p.pid("extract"), child.id().to_string());
    let log = fs::File::create(p.logs.join("extract.log")).ok();
    let err = child.stderr.take();
    let elog = log.as_ref().and_then(|f| f.try_clone().ok());
    let eh = std::thread::spawn(move || {
        if let (Some(e), Some(mut l)) = (err, elog) {
            for line in BufReader::new(e).lines().map_while(Result::ok) {
                let _ = writeln!(l, "{line}");
            }
        }
    });
    let mut last = Instant::now().checked_sub(Duration::from_secs(5)).unwrap_or_else(Instant::now);
    if let Some(out) = child.stdout.take() {
        let mut log = log;
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(l) = log.as_mut() {
                let _ = writeln!(l, "{line}");
            }
            if last.elapsed() >= Duration::from_secs(1) {
                last = Instant::now();
                let n = count(&p.data.join("maps"));
                let shown: String = line.trim().chars().take(80).collect();
                job(|j| {
                    j.pct = Some(pct(n, MAP_FILES_ABOUT));
                    j.detail = shown;
                });
            }
        }
    }
    let st = child.wait().map_err(|e| e.to_string())?;
    let _ = eh.join();
    let _ = fs::remove_file(p.pid("extract"));
    if !st.success() {
        return Err(format!("The maps couldn't be read from your game folder (the map reader stopped, code {}). The log has its lines.", st.code().unwrap_or(-1)));
    }
    if only.is_some() {
        trim_maps(&p.data.join("maps"));
    }
    let maps = count(&p.data.join("maps"));
    if maps == 0 || count(&p.data.join("dbc")) == 0 {
        return Err("The map reader finished but wrote no maps. Is this the right game folder?".into());
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let marker = serde_json::json!({ "from": game.display().to_string(), "at": now, "maps": maps, "only": only });
    fs::write(p.maps_marker(), marker.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

/// The unpacked engine's extractor takes `-m` (engine.json `"map_filter": 1`).
pub fn map_filter(dist: &Path) -> bool {
    fs::read_to_string(dist.join("engine.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("map_filter").and_then(|x| x.as_u64()))
        .unwrap_or(0)
        >= 1
}

/// Boss Rush's maps folder trimmed to what it plays (host/bossrush.rs keeps_map_file): how many files went.
pub fn trim_maps(maps: &Path) -> usize {
    let mut n = 0;
    let Ok(rd) = fs::read_dir(maps) else { return 0 };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.ends_with(".map") && !crate::host::bossrush::keeps_map_file(&name) && fs::remove_file(e.path()).is_ok() {
            n += 1;
        }
    }
    n
}

// ------------------------------------------------------------------------------------------------ processes

struct Procs {
    db: Option<Child>,
    auth: Option<Child>,
    world: Option<Child>,
}

static PROCS: Mutex<Procs> = Mutex::new(Procs { db: None, auth: None, world: None });

fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

pub fn port_answers(port: u16) -> bool {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

fn alive(c: &mut Option<Child>) -> bool {
    match c.as_mut().map(|c| c.try_wait()) {
        Some(Ok(None)) => true,
        Some(_) => {
            *c = None;
            false
        }
        None => false,
    }
}

#[derive(Clone, Copy)]
enum Which {
    Db,
    Auth,
    World,
}

/// One of our children is running (the lock taken only for the look, never while waiting).
fn child_alive(w: Which) -> bool {
    let Ok(mut g) = PROCS.lock() else { return false };
    match w {
        Which::Db => alive(&mut g.db),
        Which::Auth => alive(&mut g.auth),
        Which::World => alive(&mut g.world),
    }
}

/// A program of the engine started in `cwd` with `env`, its output appended to `log`, no window.
fn spawn(exe: &Path, args: &[&str], cwd: &Path, env: &[(String, String)], log: &Path) -> Result<Child, String> {
    if !exe.is_file() {
        return Err(format!("{} isn't in the engine folder: get the engine again.", exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    }
    let f = fs::OpenOptions::new().create(true).append(true).open(log).map_err(|e| format!("Couldn't open the log: {e}"))?;
    let f2 = f.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::from(f)).stderr(Stdio::from(f2));
    // only our own AC_ settings: none left over from the PC's environment
    for (k, _) in std::env::vars() {
        if k.starts_with("AC_") {
            cmd.env_remove(k);
        }
    }
    cmd.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    no_window(&mut cmd);
    cmd.spawn().map_err(|e| format!("{} didn't start: {e}", exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()))
}

/// Wait until `port` answers (true), the child ends, or `secs` pass.
fn wait_port(port: u16, secs: u64, mut child_ok: impl FnMut() -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(secs) {
        if port_answers(port) {
            return true;
        }
        if !child_ok() {
            return port_answers(port);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

fn wait_closed(port: u16, secs: u64) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(secs) {
        if !port_answers(port) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

fn tail_of(file: &Path, n: usize) -> String {
    let t = fs::read(file).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
    let lines: Vec<&str> = t.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// The database's first run: my.ini, then `mysqld --initialize-insecure` with the init file that sets root's
/// generated password (the file is removed after). A half-made empty folder from a failed try is made again.
fn init_db(p: &Paths, s: &State) -> Result<(), String> {
    fs::write(p.my_ini(), my_ini(p)).map_err(|e| e.to_string())?;
    fs::write(p.client_cnf(), client_cnf(s)).map_err(|e| e.to_string())?;
    if db_made(p) {
        return Ok(());
    }
    if p.db.exists() {
        let has_data = fs::read_dir(&p.db).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("acore"))).unwrap_or(false);
        if has_data {
            return Err("The database folder has saves in it but isn't complete: it was left as it is. Ask for help on Discord.".into());
        }
        fs::remove_dir_all(&p.db).map_err(|e| format!("Couldn't clear the half-made database folder: {e}"))?;
    }
    let init = p.state.join("init.sql");
    fs::write(&init, init_sql(s)).map_err(|e| e.to_string())?;
    let ini = format!("--defaults-file={}", p.my_ini().display());
    let initf = format!("--init-file={}", fwd(&init));
    let r = (|| {
        let mut c = spawn(&p.mysqld(), &[&ini, "--initialize-insecure", &initf, "--console"], &p.root, &[], &p.logs.join("mysqld-init.log"))?;
        let t = Instant::now();
        loop {
            if let Some(st) = c.try_wait().map_err(|e| e.to_string())? {
                return if st.success() && db_made(p) {
                    Ok(())
                } else {
                    Err(format!("The database couldn't be set up. {}", tail_of(&p.logs.join("mysqld.err"), 3)))
                };
            }
            if t.elapsed() > Duration::from_secs(600) {
                let _ = c.kill();
                return Err("Setting up the database took too long.".into());
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    })();
    let _ = fs::remove_file(&init);
    r
}

/// MySQL up (ours from earlier, or started now).
pub fn start_db(p: &Paths) -> Result<(), String> {
    if let Some(o) = other_running(p) {
        return Err(format!("{} is running on this PC and uses the same ports. Stop it first, then press Start again.", o.title()));
    }
    if port_answers(DB_PORT) {
        return Ok(());
    }
    if !p.my_ini().is_file() {
        return Err("The database isn't set up yet.".into());
    }
    let ini = format!("--defaults-file={}", p.my_ini().display());
    let child = spawn(&p.mysqld(), &[&ini], &p.root, &[], &p.logs.join("mysqld.out.log"))?;
    PROCS.lock().map_err(|_| "busy")?.db = Some(child);
    let ok = wait_port(DB_PORT, 120, || child_alive(Which::Db));
    if !ok {
        return Err(format!("The database didn't start. {}", tail_of(&p.logs.join("mysqld.err"), 3)));
    }
    Ok(())
}

fn mysql_cmd(p: &Paths, exe: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(exe);
    c.arg(format!("--defaults-extra-file={}", p.client_cnf().display())).args(args);
    no_window(&mut c);
    c
}

/// SQL into a database through the bundled mysql client (the password from client.cnf, never the command line).
pub fn sql(p: &Paths, db: &str, text: &str) -> Result<(), String> {
    let mut c = mysql_cmd(p, &p.mysql(), &["--default-character-set=utf8mb4", db]);
    let mut child = c.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("The database client didn't start: {e}"))?;
    child.stdin.take().ok_or("no stdin")?.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("The database said no: {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("").chars().take(200).collect::<String>()))
    }
}

/// One query's rows (the mysql client's `-N -B`: tab separated, no header).
pub fn query(p: &Paths, db: &str, text: &str) -> Result<String, String> {
    let out = mysql_cmd(p, &p.mysql(), &["--default-character-set=utf8mb4", "-N", "-B", "-e", text, db])
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().map_err(|e| format!("The database client didn't start: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(format!("The database said no: {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("").chars().take(200).collect::<String>()))
    }
}

/// One console command over the realm's SOAP (127.0.0.1 only), as the player's own GM login: the answer's text.
pub fn console(p: &Paths, command: &str) -> Result<String, String> {
    let s = load_state(p);
    if !s.account_made {
        return Err(format!("Set {} up first.", p.kind.title()));
    }
    soap(&s, command).map(|body| crate::host::bossrush::soap_text(&body))
}

// ------------------------------------------------------------------------------------------------ characters

/// The list of this world's characters kept beside its saves (host/localworld.rs).
pub fn chars_file(p: &Paths) -> PathBuf {
    p.state.join("characters.json")
}

/// The accounts whose characters the Characters page shows: the player's own (Boss Rush: the hero account, where
/// the heroes are). Never the bots' accounts or a friend's.
pub fn char_accounts(p: &Paths, s: &State) -> Vec<String> {
    match p.kind {
        Kind::BossRush => vec![crate::host::bossrush::hero_account(&s.account)],
        Kind::Single | Kind::Vanilla => {
            let mut v = vec![s.account.clone()];
            v.extend(s.hero_accounts.iter().filter(|h| !h.is_empty() && **h != s.account).cloned());
            v
        }
    }
}

/// This world's characters read now (it must be the one running), and kept for when it isn't.
pub fn refresh_chars(p: &Paths) -> Result<Vec<crate::host::localworld::LocalChar>, String> {
    use crate::host::localworld as lw;
    let s = load_state(p);
    if !s.account_made || other_running(p).is_some() || !port_answers(DB_PORT) {
        return Err(format!("{} isn't running.", p.kind.title()));
    }
    let accounts = char_accounts(p, &s);
    let refs: Vec<&str> = accounts.iter().map(String::as_str).collect();
    let rows = query(p, "acore_characters", &lw::chars_sql(&refs)?)?;
    let chars = lw::parse_chars(&rows);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    lw::write_kept(&chars_file(p), &lw::Kept { at: now, chars: chars.clone() })?;
    let last = last_of(&chars, &s.last_character);
    if last != s.last_character {
        let mut s2 = load_state(p);
        s2.last_character = last;
        let _ = save_state(p, &s2);
    }
    Ok(chars)
}

/// The character to go back to: the one that logged out last; else the one kept, while it's still there; else none.
pub fn last_of(chars: &[crate::host::localworld::LocalChar], kept: &str) -> String {
    if let Some(c) = chars.iter().filter(|c| c.last > 0).max_by_key(|c| c.last) {
        return c.name.clone();
    }
    chars.iter().find(|c| c.name.eq_ignore_ascii_case(kept)).map(|c| c.name.clone()).unwrap_or_default()
}

/// One account's characters in this world (it must be running).
pub fn chars_of(p: &Paths, account: &str) -> Result<Vec<crate::host::localworld::LocalChar>, String> {
    use crate::host::localworld as lw;
    Ok(lw::parse_chars(&query(p, "acore_characters", &lw::chars_sql(&[account])?)?))
}

/// Remember the character played (a pick or Quick start's new one).
pub fn set_last_character(p: &Paths, name: &str) {
    let mut s = load_state(p);
    if s.last_character != name {
        s.last_character = name.to_string();
        let _ = save_state(p, &s);
    }
}

// ------------------------------------------------------------------------------------------------ the Hero Key's login

/// The SQL that moves one login's characters (and its key bindings and tutorial flags) to another login in the same
/// world, then counts both logins' characters for the realm list again. Both must exist; nothing is deleted.
pub fn move_sql(from: &str, to: &str) -> Result<String, String> {
    let ok = |a: &str| (3..=16).contains(&a.len()) && a.bytes().all(|b| b.is_ascii_alphanumeric());
    if !ok(from) || !ok(to) || from.eq_ignore_ascii_case(to) {
        return Err("That login doesn't look right.".into());
    }
    let (f, t) = (from.to_ascii_uppercase(), to.to_ascii_uppercase());
    Ok(format!(
        "UPDATE characters c JOIN acore_auth.account f ON f.id = c.account AND f.username = '{f}' JOIN acore_auth.account t ON t.username = '{t}' SET c.account = t.id;\n\
         UPDATE IGNORE account_data d JOIN acore_auth.account f ON f.id = d.accountId AND f.username = '{f}' JOIN acore_auth.account t ON t.username = '{t}' SET d.accountId = t.id;\n\
         UPDATE IGNORE account_tutorial d JOIN acore_auth.account f ON f.id = d.accountId AND f.username = '{f}' JOIN acore_auth.account t ON t.username = '{t}' SET d.accountId = t.id;\n\
         REPLACE INTO acore_auth.realmcharacters (realmid, acctid, numchars) SELECT 1, a.id, (SELECT COUNT(*) FROM characters x WHERE x.account = a.id) FROM acore_auth.account a WHERE a.username IN ('{f}', '{t}');\n"
    ))
}

/// The Hero Key's login in this world for one Play (docs/SINGLE-PLAYER-ONE-CLICK.md §contract): made or updated
/// with a fresh password for this Play only (returned, never stored or logged), the realm row pointed back at this PC.
/// The first time any Hero Key plays this world, the world's own login's characters move to it, inside a back-up
/// taken first (no back-up, no move: they stay where they are). Needs the world running.
pub fn hero_login(p: &Paths, hero: &str) -> Result<String, String> {
    use crate::host::oneclick::tag;
    let mut s = load_state(p);
    if !s.account_made {
        return Err(tag("login", format!("Set {} up first.", p.kind.title())));
    }
    let pass = secret(12).map_err(|e| tag("other", e))?;
    let mut salt = [0u8; 32];
    use ring::rand::SecureRandom;
    ring::rand::SystemRandom::new().fill(&mut salt).map_err(|_| tag("other", "This PC has no random source."))?;
    sql(p, "acore_auth", &format!("{}{}", account_sql(hero, &pass, &salt)?, realm_sql_named(p.kind.realm_name())))
        .map_err(|e| tag("login", format!("Your Hero Key's login couldn't be made in {} ({e}).", p.kind.title())))?;
    let hero = hero.to_ascii_uppercase();
    if !s.hero_accounts.contains(&hero) {
        s.hero_accounts.push(hero.clone());
    }
    if s.hero_moved_to.is_empty() {
        let mine = chars_of(p, &s.account).map_err(|e| tag("db", e))?;
        if !mine.is_empty() {
            backup(p).map_err(|e| tag("db", format!("Your characters weren't moved to your Hero Key because the back-up before it failed ({e}).")))?;
            sql(p, "acore_characters", &move_sql(&s.account, &hero)?).map_err(|e| tag("db", format!("Your characters couldn't be moved to your Hero Key ({e}).")))?;
            setup_log(p, &format!("{} characters moved to this PC's Hero Key login (a back-up was taken first)", mine.len()));
        }
        s.hero_moved_to = hero;
    }
    save_state(p, &s).map_err(|e| tag("disk", e))?;
    Ok(pass)
}

/// Friends playing in this world now: online characters on friends' logins (one-time `G` + 9, worldshare::new_guest,
/// and lasting `G` + 15, worldfriends::name_for), never the player's own or the bots'.
pub const FRIENDS_SQL: &str = "SELECT COUNT(*) FROM characters c JOIN acore_auth.account a ON a.id = c.account \
    WHERE c.online <> 0 AND a.username REGEXP '^G[A-Z0-9]{9}$|^G[A-Z2-7]{15}$'";

/// Friends' logins with characters here, each with its characters: `username, name, level, online` rows.
pub const FRIEND_CHARS_SQL: &str = "SELECT a.username, c.name, c.level, c.online FROM characters c JOIN acore_auth.account a \
    ON a.id = c.account WHERE a.username REGEXP '^G[A-Z0-9]{9}$|^G[A-Z2-7]{15}$' ORDER BY a.username, c.guid";

/// A friend's character on this world (the Characters page's "Friends").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FriendChar {
    pub user: String,
    pub name: String,
    pub level: u8,
    pub online: bool,
}

pub fn parse_friend_chars(rows: &str) -> Vec<FriendChar> {
    rows.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.trim_end_matches('\r').split('\t').collect();
            if f.len() < 4 || !crate::host::worldfriends::friend_login_ok(f[0]) || !crate::hybrid::char_name_ok(f[1]) {
                return None;
            }
            Some(FriendChar { user: f[0].into(), name: f[1].into(), level: f[2].trim().parse::<u32>().unwrap_or(0).min(255) as u8, online: f[3].trim() != "0" })
        })
        .collect()
}

/// Friends' characters on this world now (it must be running).
pub fn friend_chars(p: &Paths) -> Result<Vec<FriendChar>, String> {
    Ok(parse_friend_chars(&query(p, "acore_characters", FRIEND_CHARS_SQL)?))
}

pub fn friends_online(p: &Paths) -> Result<u32, String> {
    query(p, "acore_characters", FRIENDS_SQL)?.trim().parse::<u32>().map_err(|_| "The world's answer didn't look right.".to_string())
}

/// Boss Rush's hero account password: made once and kept in state.json.
pub fn hero_password(p: &Paths) -> Result<String, String> {
    let mut s = load_state(p);
    if s.hero_password.len() < 8 {
        s.hero_password = secret(10)?;
        save_state(p, &s)?;
    }
    Ok(s.hero_password)
}

fn sql_file(p: &Paths, db: &str, file: &Path) -> Result<(), String> {
    let f = fs::File::open(file).map_err(|e| e.to_string())?;
    let out = mysql_cmd(p, &p.mysql(), &["--default-character-set=utf8mb4", db]).stdin(Stdio::from(f)).stdout(Stdio::null()).stderr(Stdio::piped())
        .output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{} didn't apply: {}", file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("").chars().take(200).collect::<String>()))
    }
}

/// dbimport (every database made and filled), then the profile's world SQL once.
fn fill(p: &Paths, s: &State) -> Result<(), String> {
    let log = p.logs.join("dbimport.out.log");
    let mut c = spawn(&p.dbimport(), &["-c", "configs/dbimport.conf"], &p.bin(), &dbimport_env(p, s), &log)?;
    let _ = fs::write(p.pid("dbimport"), c.id().to_string());
    let t = Instant::now();
    loop {
        if let Some(st) = c.try_wait().map_err(|e| e.to_string())? {
            let _ = fs::remove_file(p.pid("dbimport"));
            if !st.success() {
                return Err(format!("Filling the world stopped (code {}). {}", st.code().unwrap_or(-1), tail_of(&log, 2)));
            }
            break;
        }
        if t.elapsed() > Duration::from_secs(3600) {
            let _ = c.kill();
            return Err("Filling the world took over an hour, so it was stopped.".into());
        }
        job(|j| j.detail = format!("{} min", t.elapsed().as_secs() / 60));
        std::thread::sleep(Duration::from_secs(1));
    }
    for name in p.kind.profile_sql() {
        let f = p.profile_sql(name);
        if f.is_file() {
            sql_file(p, "acore_world", &f)?;
        }
    }
    Ok(())
}

/// Den Lite is running on this PC (it takes the login port too).
fn den_lite_running(p: &Paths) -> bool {
    crate::host::lite::beat_fresh(&crate::host::dens::dir(&crate::host::lite::root(&p.app)))
}

/// MySQL, the login server, the world server; up when their ports answer (the first world start can take minutes:
/// the bots are made).
pub fn start(p: &Paths, s: &State) -> Result<(), String> {
    let ours_auth = child_alive(Which::Auth);
    let ours_world = child_alive(Which::World);
    let title = p.kind.title();
    if let Some(o) = other_running(p) {
        return Err(format!("{} is running on this PC and uses the same ports. Stop it first, then press Start again.", o.title()));
    }
    if !ours_auth && port_answers(LOGIN_PORT) && !read_pid_alive(&p.pid("auth")) {
        return Err(if den_lite_running(p) {
            "Your Den is running on this PC and uses the same login port. Stop it on the Your Den page, then press Start again.".into()
        } else {
            format!("Another server on this PC is using the login port ({LOGIN_PORT}), so {title} can't start. Stop that server, then press Start again.")
        });
    }
    if !ours_world && port_answers(WORLD_PORT) && !read_pid_alive(&p.pid("world")) {
        return Err(format!("Another server on this PC is using the world port ({WORLD_PORT}), so {title} can't start. Stop that server, then press Start again."));
    }
    set_active(Some(p.kind));
    start_db(p)?;
    make_confs(&p.bin());
    if !port_answers(LOGIN_PORT) {
        job(|j| j.detail = "The login server".into());
        let c = spawn(&p.authserver(), &["-c", "configs/authserver.conf"], &p.bin(), &auth_env(p, s), &p.logs.join("auth.out.log"))?;
        PROCS.lock().map_err(|_| "busy")?.auth = Some(c);
        if !wait_port(LOGIN_PORT, 60, || child_alive(Which::Auth)) {
            return Err(format!("The login server didn't start. {}", tail_of(&p.logs.join("auth.out.log"), 3)));
        }
    }
    if !port_answers(WORLD_PORT) {
        job(|j| j.detail = "The world (the first start can take several minutes)".into());
        let profile = fs::read_to_string(p.profile_env(p.kind.profile())).map_err(|_| format!("The engine has no {} profile: get the engine again.", p.kind.profile()))?;
        let modules = fs::read_to_string(p.modules_env()).unwrap_or_default();
        let realm = fs::read_to_string(p.realm_env()).ok();
        let env = world_env(p, s, &profile, &modules, realm.as_deref());
        let c = spawn(&p.worldserver(), &["-c", "configs/worldserver.conf"], &p.bin(), &env, &p.logs.join("world.out.log"))?;
        PROCS.lock().map_err(|_| "busy")?.world = Some(c);
        if !wait_port(WORLD_PORT, 45 * 60, || child_alive(Which::World)) {
            return Err(format!("The world didn't start. {}", tail_of(&p.logs.join("world.out.log"), 3)));
        }
    }
    Ok(())
}

fn read_pid(file: &Path) -> Option<u32> {
    fs::read_to_string(file).ok()?.trim().parse().ok()
}

/// The process in a pid file is running (one left from an earlier launcher).
fn read_pid_alive(file: &Path) -> bool {
    let Some(pid) = read_pid(file) else { return false };
    if cfg!(windows) {
        let mut c = Command::new("tasklist");
        c.args(["/FI", &format!("PID eq {pid}"), "/NH"]);
        no_window(&mut c);
        c.output().map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string())).unwrap_or(false)
    } else {
        Path::new(&format!("/proc/{pid}")).exists()
    }
}

fn kill_pid(pid: u32) {
    let mut c = if cfg!(windows) {
        let mut c = Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/F"]);
        c
    } else {
        let mut c = Command::new("kill");
        c.arg(pid.to_string());
        c
    };
    no_window(&mut c);
    let _ = c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

/// Everyone saved and everything down: the world by SOAP's `server shutdown`, the login server, then MySQL by
/// `mysqladmin shutdown`. Whatever doesn't go in time is ended (ours by its handle, an earlier launcher's by its pid).
pub fn stop(p: &Paths) -> Result<String, String> {
    // the characters as they are now, for the Characters page while it is stopped
    if port_answers(WORLD_PORT) && other_running(p).is_none() {
        let _ = refresh_chars(p);
    }
    let s = load_state(p);
    let mut g = PROCS.lock().map_err(|_| "busy")?;
    let mut forced = false;
    if port_answers(WORLD_PORT) || alive(&mut g.world) {
        let asked = s.account_made && soap(&s, "server shutdown 1").is_ok();
        let t = Instant::now();
        while asked && t.elapsed() < Duration::from_secs(120) && (alive(&mut g.world) || port_answers(WORLD_PORT)) {
            std::thread::sleep(Duration::from_millis(500));
        }
        if alive(&mut g.world) || port_answers(WORLD_PORT) {
            forced = true;
            if let Some(mut c) = g.world.take() {
                let _ = c.kill();
                let _ = c.wait();
            } else if let Some(pid) = read_pid(&p.pid("world")) {
                kill_pid(pid);
            }
        }
    }
    if let Some(mut c) = g.auth.take() {
        let _ = c.kill();
        let _ = c.wait();
    } else if port_answers(LOGIN_PORT) {
        if let Some(pid) = read_pid(&p.pid("auth")).filter(|_| read_pid_alive(&p.pid("auth"))) {
            kill_pid(pid);
        }
    }
    if port_answers(DB_PORT) {
        let _ = mysql_cmd(p, &p.mysqladmin(), &["shutdown"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        if !wait_closed(DB_PORT, 60) {
            forced = true;
            if let Some(mut c) = g.db.take() {
                let _ = c.kill();
                let _ = c.wait();
            } else if let Some(pid) = read_pid(&p.pid("mysqld")) {
                kill_pid(pid);
            }
        }
    }
    g.db = None;
    g.world = None;
    set_active(None);
    Ok(if forced {
        "Stopped. Something didn't stop by itself in time and was ended: your characters were saved when you last left a zone or logged out.".into()
    } else {
        "Stopped. Everyone was saved.".into()
    })
}

// ------------------------------------------------------------------------------------------------ logs and back-ups

/// The last lines of each log, labelled: setup, world, login, database.
pub fn logs(p: &Paths, lines: usize) -> String {
    let each = (lines / 4).max(10);
    let mut out = Vec::new();
    for (label, file) in [("setup", "setup.log"), ("world", "Server.log"), ("world", "world.out.log"), ("login", "Auth.log"), ("database", "mysqld.err"), ("maps", "extract.log")] {
        let t = tail_of(&p.logs.join(file), each);
        for l in t.lines().filter(|l| !l.trim().is_empty()) {
            out.push(format!("{label}: {l}"));
        }
    }
    out.join("\n")
}

/// A UTC date and time for names: 2026-11-04 21:05:09 (days to a civil date, Howard Hinnant's algorithm).
pub fn stamp(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// The back-up's file name for a time.
pub fn backup_name(unix: u64) -> String {
    format!("single-player-{}.zip", stamp(unix).replace(' ', "-").replace(':', ""))
}

pub fn list_backups(p: &Paths) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(&p.backups)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|x| x.file_name().is_some_and(|n| n.to_string_lossy().starts_with("single-player-") && n.to_string_lossy().ends_with(".zip"))).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// The newest [`KEEP_BACKUPS`] kept; the names to remove.
pub fn prune(names: &[PathBuf]) -> Vec<PathBuf> {
    names[..names.len().saturating_sub(KEEP_BACKUPS)].to_vec()
}

/// The saves (accounts, characters, the bots) dumped and zipped into backups/; MySQL started for it when it was
/// down, and stopped again after. The path of the zip.
pub fn backup(p: &Paths) -> Result<PathBuf, String> {
    let s = load_state(p);
    if !(s.db_made && db_made(p)) {
        return Err("There is nothing to back up yet: start Single player once first.".into());
    }
    let was_up = port_answers(DB_PORT);
    start_db(p)?;
    let r = (|| {
        let out = mysql_cmd(p, &p.mysql(), &["-N", "-e", "SHOW DATABASES"]).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
        let have = String::from_utf8_lossy(&out.stdout).to_string();
        let dbs: Vec<&str> = ["acore_auth", "acore_characters", "acore_playerbots"].into_iter().filter(|d| have.lines().any(|l| l.trim() == *d)).collect();
        if dbs.is_empty() {
            return Err("There is nothing to back up yet.".to_string());
        }
        fs::create_dir_all(&p.tmp).map_err(|e| e.to_string())?;
        let dump = p.tmp.join("backup.sql");
        let f = fs::File::create(&dump).map_err(|e| e.to_string())?;
        let mut args = vec!["--single-transaction", "--routines", "--default-character-set=utf8mb4", "--databases"];
        args.extend(dbs.iter().copied());
        let st = mysql_cmd(p, &p.mysqldump(), &args).stdin(Stdio::null()).stdout(Stdio::from(f)).stderr(Stdio::null()).status().map_err(|e| e.to_string())?;
        if !st.success() {
            let _ = fs::remove_file(&dump);
            return Err("The back-up didn't work: the database didn't give its saves.".into());
        }
        fs::create_dir_all(&p.backups).map_err(|e| e.to_string())?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let to = p.backups.join(backup_name(now));
        let about = serde_json::json!({ "what": "Leyline Single player back-up", "at": stamp(now), "engine": s.version, "databases": dbs, "account": s.account });
        let r = zip_files(&to, &[("saves.sql", dump.as_path())], &about.to_string());
        let _ = fs::remove_file(&dump);
        r?;
        for old in prune(&list_backups(p)) {
            let _ = fs::remove_file(old);
        }
        Ok(to)
    })();
    if !was_up && !port_answers(LOGIN_PORT) {
        let _ = mysql_cmd(p, &p.mysqladmin(), &["shutdown"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    r
}

fn zip_files(to: &Path, files: &[(&str, &Path)], about: &str) -> Result<(), String> {
    let f = fs::File::create(to).map_err(|e| format!("Couldn't write the back-up: {e}"))?;
    let mut w = zip::ZipWriter::new(f);
    let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    let res = (|| -> Result<(), String> {
        for (name, path) in files {
            w.start_file(*name, opt).map_err(|e| e.to_string())?;
            let mut r = fs::File::open(path).map_err(|e| e.to_string())?;
            std::io::copy(&mut r, &mut w).map_err(|e| e.to_string())?;
        }
        w.start_file("about.json", opt).map_err(|e| e.to_string())?;
        w.write_all(about.as_bytes()).map_err(|e| e.to_string())?;
        w.finish().map_err(|e| e.to_string())?;
        Ok(())
    })();
    if res.is_err() {
        let _ = fs::remove_file(to);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st() -> State {
        State { db_password: "PW".repeat(12), account: ACCOUNT.into(), password: "ABCD2345".into(), ..Default::default() }
    }

    #[test]
    fn boss_rush_has_its_own_install_on_the_same_engine_files() {
        let app = Path::new("/app");
        let (single, rush) = (Paths::new(app), Paths::for_kind(app, Kind::BossRush));
        assert_eq!(rush.root, Path::new("/app/engine-bossrush"));
        assert_eq!(rush.dist, single.dist, "the engine is downloaded once");
        for (a, b) in [(&rush.data, &single.data), (&rush.db, &single.db), (&rush.state, &single.state), (&rush.logs, &single.logs)] {
            assert_ne!(a, b, "its own maps, database, saves and logs");
        }
        assert!(required(&rush).contains(&rush.profile_env("bossrush")));
        let mut s = State::default();
        assert!(ensure_secrets_for(&mut s, Kind::BossRush).unwrap());
        assert_eq!(s.account, "RUSH");
        assert_eq!(crate::host::bossrush::hero_account(&s.account), "RUSHBF");
        assert!(realm_sql_named(Kind::BossRush.realm_name()).contains("name = 'Boss Rush'"));
        assert!(realm_sql_named("x'; DROP").contains("name = 'x DROP'"), "letters and spaces only");
        assert_eq!(realm_sql(), realm_sql_named(REALM_NAME));
        assert_eq!((Kind::of_job("bossrush"), Kind::of_job("engine")), (Kind::BossRush, Kind::Single));
        assert_eq!(Kind::BossRush.only_maps(), Some(vec![249, 615, 616, 624, 631, 724]));
        assert_eq!(Kind::Single.only_maps(), None);
        assert!(Kind::BossRush.profile_sql().is_empty());
    }

    #[test]
    fn the_original_world_has_its_own_saves_and_single_players_maps() {
        let app = Path::new("/app");
        let (single, van) = (Paths::new(app), Paths::for_kind(app, Kind::Vanilla));
        assert_eq!(van.root, Path::new("/app/engine-vanilla"));
        assert_eq!(van.dist, single.dist, "the engine is downloaded once");
        assert_eq!(van.data, single.data, "the maps are read once for both");
        for (a, b) in [(&van.db, &single.db), (&van.state, &single.state), (&van.logs, &single.logs), (&van.backups, &single.backups)] {
            assert_ne!(a, b, "its own database, saves and logs");
        }
        assert_ne!(Paths::for_kind(app, Kind::BossRush).data, single.data, "Boss Rush keeps its own trimmed maps");
        assert!(required(&van).contains(&van.profile_env("solovanilla")));
        let mut s = State::default();
        assert!(ensure_secrets_for(&mut s, Kind::Vanilla).unwrap());
        assert_eq!(s.account, "ORIGINAL");
        assert!(realm_sql_named(Kind::Vanilla.realm_name()).contains("name = 'Original world'"));
        assert_eq!(Kind::of_job("vanilla"), Kind::Vanilla);
        assert_eq!(Kind::from_word("vanilla"), Some(Kind::Vanilla));
        assert_eq!(Kind::from_word("single"), Some(Kind::Single));
        assert_eq!(Kind::from_word("bossrush"), Some(Kind::BossRush));
        assert_eq!(Kind::from_word("den"), None);
        assert!(Kind::Vanilla.profile_sql().is_empty(), "no modern combos in the original world");
        assert_eq!((Kind::Vanilla.bots(), Kind::Vanilla.only_maps()), (0, None));
        assert_eq!(char_accounts(&van, &s), ["ORIGINAL"]);
        let rush = Paths::for_kind(app, Kind::BossRush);
        assert_eq!(char_accounts(&rush, &State { account: "RUSH".into(), ..Default::default() }), ["RUSHBF"], "the heroes");
        assert_eq!(chars_file(&van), Path::new("/app/engine-vanilla/state/characters.json"));
        let jobs: Vec<&str> = KINDS.iter().map(|k| k.job_kind()).collect();
        assert_eq!(jobs, ["engine", "vanilla", "bossrush"], "each world's progress line is its own");
    }

    #[test]
    fn the_original_worlds_profile_holds_the_first_era_with_companions() {
        let p = Paths::for_kind(Path::new("C:\\L"), Kind::Vanilla);
        let (Ok(van), Ok(modules)) = (
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../realm/profiles/solovanilla.env")),
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../realm/modules.env")),
        ) else {
            return;
        };
        let v = world_env(&p, &st(), &van, &modules, None);
        let g = |k: &str| get(&v, k).map(str::to_string);
        assert_eq!(g("AC_MAX_PLAYER_LEVEL").as_deref(), Some("60"));
        assert_eq!(g("AC_INDIVIDUAL_PROGRESSION_ENABLE").as_deref(), Some("1"));
        assert_eq!(g("AC_INDIVIDUAL_PROGRESSION_PROGRESSION_LIMIT").as_deref(), Some("7"));
        assert_eq!(g("OV_COMPANIONS").as_deref(), Some("1"), "the companions and dungeon buddies");
        assert_eq!(g("AC_PLAYERBOTS_MIN_RANDOM_BOTS").as_deref(), Some("0"), "no roaming population");
        assert_eq!(g("AC_PLAYERBOTS_RANDOM_BOT_AUTOLOGIN").as_deref(), Some("0"), "the profile wins over modules.env's 1");
        assert_eq!(g("AC_PLAYER_LIMIT").as_deref(), Some("5"));
        assert_eq!(g("OV_REALM_NAME").as_deref(), Some(VANILLA_REALM_NAME));
        assert!(g("AC_DATA_DIR").unwrap().ends_with("engine/data"), "Single player's maps");
        assert!(g("AC_LOGIN_DATABASE_INFO").unwrap().contains(";13306;"));
        assert_eq!(g("AC_BIND_IP").as_deref(), Some("127.0.0.1"), "this PC only: friends come through the launcher");
    }

    #[test]
    fn boss_rush_profile_wins_over_modules_env() {
        let p = Paths::for_kind(Path::new("C:\\L"), Kind::BossRush);
        let (Ok(rush), Ok(modules)) = (
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../realm/profiles/bossrush.env")),
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../realm/modules.env")),
        ) else {
            return;
        };
        let v = world_env(&p, &st(), &rush, &modules, None);
        let g = |k: &str| get(&v, k).map(str::to_string);
        assert_eq!(g("AC_AUTO_BALANCE_ENABLE_GLOBAL").as_deref(), Some("0"), "modules.env says 1; Boss Rush's profile wins");
        assert_eq!(g("AC_LEYLINE_BOSSFIGHT_ALL_FIGHTS").as_deref(), Some("1"));
        assert_eq!(g("AC_PLAYERBOTS_MIN_RANDOM_BOTS").as_deref(), Some("0"), "no roaming bots");
        assert_eq!(g("AC_PLAYERBOTS_RANDOM_BOT_AUTOLOGIN").as_deref(), Some("0"));
        assert_eq!(g("AC_MYTHIC_PLUS_ENABLE").as_deref(), Some("0"));
        assert_eq!(g("AC_PLAYER_LIMIT").as_deref(), Some("2"));
        assert_eq!(g("AC_ALE_ENABLED").as_deref(), Some("true"), "modules.env's own lines stay");
        assert!(g("AC_DATA_DIR").unwrap().ends_with("engine-bossrush/data"));
        assert!(g("AC_ALE_SCRIPT_PATH").unwrap().ends_with("engine/dist/lua_scripts"));
        assert_eq!(env_name("LeylineBossfight.AllFights"), "AC_LEYLINE_BOSSFIGHT_ALL_FIGHTS");
        assert_eq!(env_name("Wintergrasp.KickVoAPlayers"), "AC_WINTERGRASP_KICK_VO_APLAYERS");
        assert!(!v.iter().any(|(k, _)| k == "WU_BOTS"));
    }

    #[test]
    fn boss_rush_trims_a_full_map_read() {
        let d = tempfile::tempdir().unwrap();
        for f in ["6313531.map", "0004331.map", "0003232.map", "5712020.map", "notes.txt"] {
            std::fs::write(d.path().join(f), b"x").unwrap();
        }
        assert_eq!(trim_maps(d.path()), 2);
        let mut left: Vec<String> = std::fs::read_dir(d.path()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        left.sort();
        assert_eq!(left, ["0004331.map", "6313531.map", "notes.txt"]);
        std::fs::write(d.path().join("engine.json"), r#"{"name":"leyline-engine","map_filter":1}"#).unwrap();
        assert!(map_filter(d.path()));
        std::fs::write(d.path().join("engine.json"), r#"{"name":"leyline-engine"}"#).unwrap();
        assert!(!map_filter(d.path()), "an older engine: read everything, then trim");
    }

    #[test]
    fn the_folders_and_programs() {
        let p = Paths::new(Path::new("/app"));
        assert_eq!(p.root, Path::new("/app/engine"));
        assert_eq!(p.dist, Path::new("/app/engine/dist"));
        assert_eq!(p.worldserver(), Path::new(&format!("/app/engine/dist/bin/worldserver{EXE}")));
        assert_eq!(p.extractor(), Path::new(&format!("/app/engine/dist/tools/map_extractor{EXE}")));
        assert_eq!(p.mysqld(), Path::new(&format!("/app/engine/dist/mysql/bin/mysqld{EXE}")));
        assert_eq!(p.profile_env("solo"), Path::new("/app/engine/dist/realm/profiles/solo.env"));
        for kept in [&p.data, &p.db, &p.state, &p.backups] {
            assert!(!kept.starts_with(&p.dist), "the player's things are outside what an update replaces: {kept:?}");
        }
        assert!(required(&p).contains(&p.mysqldump()), "back-ups need it");
    }

    #[test]
    fn missing_release_says_use_docker() {
        assert!(get_error("the author's site answered 404").contains("Choose Docker"));
        assert_eq!(get_error("timed out"), "Couldn't get the engine: timed out");
    }

    #[test]
    fn only_on_windows_for_now_and_the_release_address() {
        assert!(supported_on("windows", "x86_64"));
        assert!(!supported_on("linux", "x86_64") && !supported_on("macos", "aarch64") && !supported_on("windows", "aarch64"));
        assert_eq!(release_from(None), RELEASE);
        assert!(RELEASE.ends_with("/releases/download/engine-latest/"));
        assert_eq!(release_from(Some("https://example.org/e/")), "https://example.org/e/");
        assert_eq!(release_from(Some("http://example.org/e/")), RELEASE, "https only");
        assert_eq!(release_from(Some("https://example.org/e")), RELEASE, "a folder");
    }

    #[test]
    fn ini_keys_as_the_core_names_them_in_the_environment() {
        // the core's own examples (Config.cpp), and the names docker-compose.host.yml uses
        assert_eq!(ini_to_env("SomeConfig"), "SOME_CONFIG");
        assert_eq!(ini_to_env("myNestedConfig.opt1"), "MY_NESTED_CONFIG_OPT_1");
        assert_eq!(ini_to_env("LogDB.Opt.ClearTime"), "LOG_DB_OPT_CLEAR_TIME");
        assert_eq!(env_name("vmap.enableLOS"), "AC_VMAP_ENABLE_LOS");
        assert_eq!(env_name("vmap.enableHeight"), "AC_VMAP_ENABLE_HEIGHT");
        assert_eq!(env_name("vmap.petLOS"), "AC_VMAP_PET_LOS");
        assert_eq!(env_name("MoveMaps.Enable"), "AC_MOVE_MAPS_ENABLE");
        assert_eq!(env_name("Warden.Enabled"), "AC_WARDEN_ENABLED");
        assert_eq!(env_name("PlayerLimit"), "AC_PLAYER_LIMIT");
        assert_eq!(env_name("DataDir"), "AC_DATA_DIR");
        assert_eq!(env_name("LogsDir"), "AC_LOGS_DIR");
        assert_eq!(env_name("LoginDatabaseInfo"), "AC_LOGIN_DATABASE_INFO");
        assert_eq!(env_name("CloseIdleConnections"), "AC_CLOSE_IDLE_CONNECTIONS");
        assert_eq!(env_name("BindIP"), "AC_BIND_IP");
        assert_eq!(env_name("MySQLExecutable"), "AC_MY_SQLEXECUTABLE");
        assert_eq!(env_name("SOAP.IP"), "AC_SOAP_IP");
        assert_eq!(env_name("WrongPass.MaxCount"), "AC_WRONG_PASS_MAX_COUNT");
        assert_eq!(env_name("Updates.EnableDatabases"), "AC_UPDATES_ENABLE_DATABASES");
    }

    #[test]
    fn env_files_read_as_compose_reads_them() {
        let v = parse_env("# c\n\nA=1\r\nB = two words\nexport C=\"q\"\nD='s'\nbad key=1\n=x\nE=a=b\n  # indented comment\n");
        assert_eq!(v, vec![("A".into(), "1".into()), ("B".into(), "two words".into()), ("C".into(), "q".into()), ("D".into(), "s".into()), ("E".into(), "a=b".into())]);
    }

    #[test]
    fn the_solo_profile_mapped_to_the_native_world_server() {
        let p = Paths::new(Path::new("C:\\Users\\me\\AppData\\Roaming\\Leyline"));
        // the realm's files are in the workspace, not in the public Leyline repo (its export leaves realm/ out)
        let (Ok(solo), Ok(modules)) = (
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../realm/profiles/solo.env")),
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../realm/modules.env")),
        ) else {
            return;
        };
        let s = st();
        let v = world_env(&p, &s, &solo, &modules, None);
        let g = |k: &str| get(&v, k).map(str::to_string);
        // the profile's own lines, as the container gets them
        assert_eq!(g("AC_RATE_XP_KILL").as_deref(), Some("2"));
        assert_eq!(g("OV_COMPANIONS").as_deref(), Some("1"), "our Lua switches too (os.getenv)");
        assert_eq!(g("OV_REALM_NAME"), parse_env(&solo).into_iter().find(|(k, _)| k == "OV_REALM_NAME").map(|x| x.1));
        assert_eq!(g("AC_PLAYERBOTS_PREMADE_SPEC_NAME_1__0").as_deref(), Some("arms pve"), "values with spaces");
        assert_eq!(g("AC_AUTO_BALANCE_MIN_PLAYERS").as_deref(), Some("1"));
        // modules.env's, with its Docker database line replaced
        assert_eq!(g("AC_ALE_ENABLED").as_deref(), Some("true"));
        assert_eq!(g("AC_PLAYERBOTS_DATABASE_INFO"), Some(format!("127.0.0.1;13306;root;{};acore_playerbots", s.db_password)));
        assert!(!v.iter().any(|(_, x)| x.contains("ac-database") || x.contains("ov-local-test")), "no container names or test passwords");
        // the compose block, for this PC
        assert_eq!(g("AC_LOGIN_DATABASE_INFO"), Some(format!("127.0.0.1;13306;root;{};acore_auth", s.db_password)));
        assert_eq!(g("AC_PLAYERBOTS_MIN_RANDOM_BOTS").as_deref(), Some("0"), "a quiet world until the player asks (host/spsettings.rs), not modules.env's 30");
        assert_eq!(g("AC_PLAYERBOTS_MAX_RANDOM_BOTS").as_deref(), Some("0"));
        assert_eq!(g("AC_PLAYER_LIMIT").as_deref(), Some("5"));
        assert_eq!(g("AC_WARDEN_ENABLED").as_deref(), Some("0"));
        assert_eq!(g("AC_MOVE_MAPS_ENABLE").as_deref(), Some("0"));
        assert_eq!(g("AC_DATA_DIR").as_deref(), Some("C:/Users/me/AppData/Roaming/Leyline/engine/data"));
        assert_eq!(g("AC_ALE_SCRIPT_PATH").as_deref(), Some("C:/Users/me/AppData/Roaming/Leyline/engine/dist/lua_scripts"));
        assert_eq!(g("AC_MY_SQLEXECUTABLE"), Some(format!("C:/Users/me/AppData/Roaming/Leyline/engine/dist/mysql/bin/mysql{EXE}")));
        assert_eq!(g("AC_SOURCE_DIRECTORY").as_deref(), Some("C:/Users/me/AppData/Roaming/Leyline/engine/dist/source"));
        assert_eq!(g("AC_CONSOLE_ENABLE").as_deref(), Some("0"), "no console window");
        // nothing listens anywhere but this PC
        assert_eq!(g("AC_BIND_IP").as_deref(), Some("127.0.0.1"));
        assert_eq!(g("AC_SOAP_IP").as_deref(), Some("127.0.0.1"));
        assert_eq!(g("AC_RA_ENABLE").as_deref(), Some("0"));
        assert_eq!(get(&auth_env(&p, &s), "AC_BIND_IP"), Some("127.0.0.1"));
        assert!(my_ini(&p).contains("bind-address=127.0.0.1\n") && my_ini(&p).contains("mysqlx=OFF"));
        assert!(!my_ini(&p).contains(&s.db_password), "the password is not in my.ini");
        // each key once
        let mut keys: Vec<&String> = v.iter().map(|(k, _)| k).collect();
        keys.sort();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n);
    }

    #[test]
    fn the_players_own_settings_win_and_set_the_bots_and_pathing() {
        let p = Paths::new(Path::new("/a"));
        let v = world_env(&p, &st(), "AC_RATE_XP_KILL=2\n", "AC_RATE_XP_KILL=9\nAC_X=1\n", Some("AC_RATE_XP_KILL=3\nWU_BOTS=5\nOV_PATHING=1\n"));
        assert_eq!(get(&v, "AC_RATE_XP_KILL"), Some("3"), "profile < modules.env < realm.env");
        assert_eq!(get(&v, "AC_X"), Some("1"));
        assert_eq!(get(&v, "AC_PLAYERBOTS_MAX_RANDOM_BOTS"), Some("5"));
        assert_eq!(get(&v, "WU_BOTS"), None, "the kit's own variable isn't passed on");
        assert_eq!(get(&v, "AC_VMAP_ENABLE_LOS"), Some("1"));
        // but never the databases, the address or the console
        let v = world_env(&p, &st(), "", "", Some("AC_BIND_IP=0.0.0.0\nAC_LOGIN_DATABASE_INFO=x\nAC_CONSOLE_ENABLE=1\nWU_BOTS=lots\n"));
        assert_eq!(get(&v, "AC_BIND_IP"), Some("127.0.0.1"));
        assert!(get(&v, "AC_LOGIN_DATABASE_INFO").unwrap().starts_with("127.0.0.1;13306;root;"));
        assert_eq!(get(&v, "AC_CONSOLE_ENABLE"), Some("0"));
        assert_eq!(get(&v, "AC_PLAYERBOTS_MIN_RANDOM_BOTS"), Some("0"), "not a number: the quiet default");
    }

    #[test]
    fn the_state_machine() {
        let none = Have::default();
        assert_eq!(plan(&none), [Step::Download, Step::Unpack, Step::Maps, Step::Database, Step::Fill, Step::Account, Step::Start]);
        let mut h = Have { installed: true, maps: true, ..Default::default() };
        assert_eq!(plan(&h), [Step::Database, Step::Fill, Step::Account, Step::Start], "after a failure: from where it stopped");
        h.db = true;
        h.filled = true;
        h.account = true;
        assert!(set_up(&h));
        assert_eq!(plan(&h), [Step::Start], "set up, stopped: only Start");
        h.running = true;
        assert!(plan(&h).is_empty(), "running: nothing to do");
        h.update = true;
        assert_eq!(plan(&h), [Step::Download, Step::Unpack, Step::Start], "an update: the new engine, then start again");
        h.update = false;
        h.maps = false;
        assert_eq!(plan(&h), [Step::Maps, Step::Start], "maps gone: read again, then restart");
        assert!(!set_up(&h));
        for s in plan(&none) {
            assert!(!s.label().is_empty());
            assert!(!s.label().to_lowercase().contains("warcraft") && !s.label().contains("3.3.5"));
        }
    }

    #[test]
    fn repair_checks_files_then_only_whats_missing_and_never_refills_saves() {
        let done = Have { installed: true, maps: true, db: true, filled: true, account: true, ..Default::default() };
        assert_eq!(repair_plan(&done), [Step::Verify, Step::Start, Step::Login, Step::Ports]);
        let no_maps = Have { maps: false, ..done };
        assert_eq!(repair_plan(&no_maps), [Step::Verify, Step::Maps, Step::Start, Step::Login, Step::Ports]);
        assert!(!repair_plan(&done).contains(&Step::Fill), "a database that is there is never filled again");
        for st in [Step::Verify, Step::Login, Step::Ports] {
            assert!(!st.label().is_empty() && !st.label().to_lowercase().contains("warcraft"));
        }
    }

    #[test]
    fn a_database_folder_gone_takes_its_fill_and_login_with_it() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path());
        let s = State { db_made: true, filled: true, account_made: true, ..st() };
        let h = have(&p, &s, false);
        assert!(!h.db && !h.filled && !h.account, "no db folder: fill and login again");
        std::fs::create_dir_all(p.db.join("mysql")).unwrap();
        let h = have(&p, &s, false);
        assert!(h.db && h.filled && h.account);
        let sql = drop_sql();
        for db in ["acore_world", "acore_characters", "acore_auth", "acore_playerbots"] {
            assert!(sql.contains(&format!("DROP DATABASE IF EXISTS `{db}`;")));
        }
    }

    #[test]
    fn a_setup_cut_short_leaves_nothing_half_done_trusted() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::for_kind(d.path(), Kind::Vanilla);
        let single = Paths::new(d.path());
        std::fs::create_dir_all(&p.tmp).unwrap();
        std::fs::write(p.tmp.join(ASSET), b"half a zip").unwrap();
        std::fs::create_dir_all(p.root.join("dist.new").join("bin")).unwrap();
        // an update moved the engine aside and stopped before putting the new one in
        std::fs::create_dir_all(single.root.join("dist.old").join("bin")).unwrap();
        std::fs::write(single.root.join("dist.old").join("engine.json"), b"{}").unwrap();
        std::fs::create_dir_all(&p.state).unwrap();
        std::fs::write(p.pid("dbimport"), std::process::id().to_string()).unwrap();
        assert!(!pid_runs(std::process::id(), "dbimport"), "this test isn't the database filler: never ended");
        heal_partial(&p);
        assert!(!p.tmp.join(ASSET).exists() && !p.root.join("dist.new").exists());
        assert!(single.dist.join("engine.json").is_file(), "the engine put back where it was");
        assert!(!p.pid("dbimport").exists());
    }

    #[test]
    fn characters_move_to_the_hero_keys_login_by_sql_that_deletes_nothing() {
        let q = move_sql("SOLO", "HKABCDEFGHIJKLMN").unwrap();
        assert!(q.contains("f.username = 'SOLO'") && q.contains("t.username = 'HKABCDEFGHIJKLMN'"));
        assert!(q.contains("SET c.account = t.id"));
        assert!(q.contains("UPDATE IGNORE account_data") && q.contains("realmcharacters"));
        assert!(!q.to_ascii_uppercase().contains("DELETE") && !q.to_ascii_uppercase().contains("DROP"));
        assert!(move_sql("SOLO", "solo").is_err(), "not onto itself");
        assert!(move_sql("SO'LO", "HKA").is_err() && move_sql("SOLO", "H K").is_err());
        assert!(FRIENDS_SQL.contains("^G[A-Z0-9]{9}$") && FRIENDS_SQL.contains("online"));
        assert!(FRIENDS_SQL.contains("^G[A-Z2-7]{15}$"), "lasting friends' logins count too");
        let rows = "GABCDE2345\tWren\t12\t1\nGABCDEFGHIJKLMNO\tAsh\t80\t0\nSOLO\tMine\t5\t1\nGABCDE2345\tBad;name\t1\t0\n";
        let v = parse_friend_chars(rows);
        assert_eq!(v.len(), 2, "only friends' logins and real names");
        assert_eq!((v[0].user.as_str(), v[0].name.as_str(), v[0].level, v[0].online), ("GABCDE2345", "Wren", 12, true));
        assert!(!v[1].online && v[1].level == 80);
        let s = State { account: "SOLO".into(), hero_accounts: vec!["HKA2345".into(), "SOLO".into()], ..st() };
        let p = Paths::for_kind(std::path::Path::new("x"), Kind::Single);
        assert_eq!(char_accounts(&p, &s), vec!["SOLO".to_string(), "HKA2345".to_string()], "the player's logins, each once");
    }

    #[test]
    fn the_last_character_is_the_last_to_log_out() {
        use crate::host::localworld::LocalChar;
        let c = |n: &str, last: u64| LocalChar { name: n.into(), last, ..Default::default() };
        assert_eq!(last_of(&[c("Wren", 5), c("Odo", 9), c("Isra", 0)], ""), "Odo");
        assert_eq!(last_of(&[c("Wren", 0), c("Odo", 0)], "odo"), "Odo", "none logged out yet: the kept one, while it's there");
        assert_eq!(last_of(&[c("Wren", 0)], "Odo"), "", "the kept one is gone");
        assert_eq!(last_of(&[], "Odo"), "");
    }

    #[test]
    fn have_reads_the_folders_and_the_state() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path());
        let mut s = st();
        let h = have(&p, &s, true);
        assert!(!h.installed && !h.maps && !h.db && !h.update);
        for f in required(&p) {
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            if f.extension().is_none() && f.ends_with("base") {
                std::fs::create_dir_all(&f).unwrap();
            } else {
                std::fs::write(&f, b"x").unwrap();
            }
        }
        assert!(installed(&p));
        std::fs::create_dir_all(p.data.join("dbc")).unwrap();
        std::fs::create_dir_all(p.data.join("maps")).unwrap();
        assert!(!maps_done(&p), "no marker: the extractor didn't finish");
        std::fs::write(p.maps_marker(), "{}").unwrap();
        assert!(maps_done(&p));
        std::fs::create_dir_all(p.db.join("mysql")).unwrap();
        assert!(!have(&p, &s, false).db, "the state says so too");
        s.db_made = true;
        s.sha256 = "a".repeat(64);
        s.latest_sha256 = "b".repeat(64);
        let h = have(&p, &s, true);
        assert!(h.installed && h.maps && h.db && h.update);
        assert!(!have(&p, &s, false).update, "only when asked");
        save_state(&p, &s).unwrap();
        assert_eq!(load_state(&p), s);
    }

    #[test]
    fn secrets_made_once() {
        let mut s = State::default();
        assert!(ensure_secrets(&mut s).unwrap());
        assert_eq!(s.db_password.len(), 24);
        assert_eq!(s.account, ACCOUNT);
        assert_eq!(s.password.len(), 8);
        assert!(s.password.bytes().all(|b| ABC.contains(&b)));
        let keep = s.clone();
        assert!(!ensure_secrets(&mut s).unwrap());
        assert_eq!(s, keep);
        assert_ne!(secret(24).unwrap(), secret(24).unwrap());
        assert!(crate::herokey::prefill_ok(ACCOUNT), "the login screen takes it");
    }

    #[test]
    fn the_account_as_make_account_py_makes_it() {
        // python3 realm/make_account.py logic with a fixed salt (bytes 0..32 / all 0xff)
        let salt: [u8; 32] = std::array::from_fn(|i| i as u8);
        assert_eq!(hex(&verifier("SOLO", "ABCD2345", &salt)), "0e6aa9757c3c488d236a85cfe960b324653387bee83996fd579f926fb0a3fd6b");
        assert_eq!(hex(&verifier("solo", "abcd2345", &salt)), hex(&verifier("SOLO", "ABCD2345", &salt)), "upper-cased, as the client does");
        assert_eq!(hex(&verifier("PLAYER1", "ZZZZ9999", &[0xff; 32])), "611929222fc15f53108771e90f7c8c5e3a53fee5839b608d833ac6460c79634e");
        let q = account_sql("solo", "ABCD2345", &salt).unwrap();
        assert!(q.contains("VALUES ('SOLO', 0x000102"));
        assert!(q.contains("0x0e6aa975"));
        assert!(q.contains("ON DUPLICATE KEY UPDATE"), "characters kept: the row is updated, never deleted");
        assert!(!q.to_uppercase().contains("DELETE"));
        assert!(q.contains("SELECT id, 3, -1"));
        assert!(account_sql("so'lo", "x", &salt).is_err());
        assert!(account_sql("SOLO", "a'b", &salt).is_err());
        assert!(realm_sql().contains("address = '127.0.0.1'") && realm_sql().contains("port = 8085") && realm_sql().contains("'Single player'"));
        let i = init_sql(&st());
        assert!(i.contains("'root'@'127.0.0.1'") && i.contains("'root'@'localhost'"));
        assert!(client_cnf(&st()).contains("host=127.0.0.1\nport=13306\n"));
    }

    #[test]
    fn soap_for_the_clean_stop() {
        assert_eq!(base64(b"SOLO:ABCD2345"), "U09MTzpBQkNEMjM0NQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"abc"), "YWJj");
        let r = soap_request("SOLO", "ABCD2345", "server shutdown 1");
        assert!(r.starts_with("POST / HTTP/1.1\r\nHost: 127.0.0.1:17878\r\n"));
        assert!(r.contains("Authorization: Basic U09MTzpBQkNEMjM0NQ==\r\n"));
        assert!(r.contains("<command>server shutdown 1</command>") && r.contains("urn:AC"));
        let body = r.split("\r\n\r\n").nth(1).unwrap();
        assert!(r.contains(&format!("Content-Length: {}\r\n", body.len())));
        assert!(soap_request("a", "b", "x<y>&").contains("x&lt;y&gt;&amp;"));
    }

    #[test]
    fn never_game_data() {
        for bad in ["Data/common.MPQ", "dbc/Spell.dbc", "maps/0002331.map", "x.M2", "a.wmo", "t.adt", "i.blp", "v.vmtree", "m.mmap"] {
            assert!(is_game_data(bad), "{bad}");
        }
        for ok in ["bin/worldserver.exe", "source/data/sql/base/db_world/creature.sql", "lua_scripts/ov_main.lua", "realm/profiles/solo.env", "engine.json"] {
            assert!(!is_game_data(ok), "{ok}");
        }
    }

    fn engine_zip(extra: &[(&str, &[u8])], layout: u64) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let p = Paths::new(Path::new("/x"));
        let opt = zip::write::SimpleFileOptions::default();
        for f in required(&p) {
            let rel = f.strip_prefix(&p.dist).unwrap().to_string_lossy().replace('\\', "/");
            if rel == "engine.json" {
                continue;
            }
            if rel.ends_with("/base") {
                w.start_file(format!("{rel}/db_auth/x.sql"), opt).unwrap();
            } else {
                w.start_file(rel, opt).unwrap();
            }
            w.write_all(b"x").unwrap();
        }
        w.start_file("bin/configs/modules/playerbots.conf.dist", opt).unwrap();
        w.write_all(b"x").unwrap();
        w.start_file("engine.json", opt).unwrap();
        w.write_all(format!("{{\"name\":\"leyline-engine\",\"version\":\"2026.11.04-abc1234\",\"layout\":{layout}}}").as_bytes()).unwrap();
        for (n, b) in extra {
            w.start_file(*n, opt).unwrap();
            w.write_all(b).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn unpacked_only_when_whole_checked_and_without_game_data() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path());
        std::fs::create_dir_all(&p.root).unwrap();
        let zip = d.path().join("e.zip");
        let mut s = st();
        let sha_of = |z: &Path| crate::net::sha256_file(z).unwrap();
        // the checksum
        std::fs::write(&zip, engine_zip(&[], LAYOUT)).unwrap();
        assert!(place(&p, &zip, &format!("{}  {ASSET}", "0".repeat(64)), &mut s).unwrap_err().contains("doesn't match"));
        assert!(place(&p, &zip, "<html>", &mut s).unwrap_err().contains("doesn't look right"));
        assert!(!p.dist.exists());
        // game data inside: refused, nothing left
        std::fs::write(&zip, engine_zip(&[("data/dbc/Spell.dbc", b"WDBC")], LAYOUT)).unwrap();
        assert!(place(&p, &zip, &sha_of(&zip), &mut s).unwrap_err().contains("game file"));
        assert!(!p.dist.exists() && !p.root.join("dist.new").exists());
        // a newer layout: update the launcher first
        std::fs::write(&zip, engine_zip(&[], LAYOUT + 1)).unwrap();
        assert!(place(&p, &zip, &sha_of(&zip), &mut s).unwrap_err().contains("update Leyline"));
        // good: in place, its confs made, the state says what is installed
        std::fs::write(&zip, engine_zip(&[], LAYOUT)).unwrap();
        let sha = sha_of(&zip);
        place(&p, &zip, &format!("{sha}  {ASSET}\n"), &mut s).unwrap();
        assert!(installed(&p));
        assert!(p.bin().join("configs").join("worldserver.conf").is_file() && p.bin().join("configs").join("modules").join("playerbots.conf").is_file());
        assert_eq!((s.sha256.as_str(), s.version.as_str()), (sha.as_str(), "2026.11.04-abc1234"));
        // an update replaces dist only: the player's maps, saves and state stay
        for (dir, f) in [(&p.data, "x"), (&p.db, "ibdata1"), (&p.state, "keep")] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(f), b"mine").unwrap();
        }
        std::fs::write(p.bin().join("configs").join("worldserver.conf"), b"edited").unwrap();
        std::fs::write(&zip, engine_zip(&[("bin/new.dll", b"n")], LAYOUT)).unwrap();
        place(&p, &zip, &sha_of(&zip), &mut s).unwrap();
        assert!(p.bin().join("new.dll").is_file());
        assert_eq!(std::fs::read(p.db.join("ibdata1")).unwrap(), b"mine");
        assert_eq!(std::fs::read(p.data.join("x")).unwrap(), b"mine");
        assert!(!p.root.join("dist.old").exists());
    }

    #[test]
    fn the_game_folder_needs_its_archives() {
        let d = tempfile::tempdir().unwrap();
        assert!(game_ok(d.path()).unwrap_err().contains("no Data folder"));
        std::fs::create_dir_all(d.path().join("Data")).unwrap();
        assert!(game_ok(d.path()).unwrap_err().contains("no game archives"));
        std::fs::write(d.path().join("Data").join("common.MPQ"), b"").unwrap();
        assert!(game_ok(d.path()).is_ok());
    }

    #[test]
    fn progress_figures_backups_and_names() {
        assert_eq!(pct(0, 100), 0);
        assert_eq!(pct(50, 100), 50);
        assert_eq!(pct(100, 100), 99, "100 only once the step says it is done");
        assert_eq!(pct(9000, MAP_FILES_ABOUT), 99);
        assert_eq!(pct(5, 0), 0);
        assert_eq!(stamp(0), "1970-01-01 00:00:00");
        assert_eq!(stamp(1_793_822_709), "2026-11-04 20:05:09");
        assert_eq!(stamp(951_782_400), "2000-02-29 00:00:00");
        assert_eq!(backup_name(1_793_822_709), "single-player-2026-11-04-200509.zip");
        let names: Vec<PathBuf> = (0..13).map(|i| PathBuf::from(backup_name(1_793_822_709 + i * 60))).collect();
        let gone = prune(&names);
        assert_eq!(gone.len(), 3);
        assert_eq!(gone, names[..3].to_vec(), "the oldest go");
        assert!(prune(&names[..4]).is_empty());
        let d = tempfile::tempdir().unwrap();
        let sql = d.path().join("s.sql");
        std::fs::write(&sql, b"CREATE DATABASE x;").unwrap();
        let to = d.path().join("b.zip");
        zip_files(&to, &[("saves.sql", sql.as_path())], "{}").unwrap();
        let mut ar = zip::ZipArchive::new(std::fs::File::open(&to).unwrap()).unwrap();
        assert_eq!(ar.len(), 2);
        let mut t = String::new();
        ar.by_name("saves.sql").unwrap().read_to_string(&mut t).unwrap();
        assert_eq!(t, "CREATE DATABASE x;");
    }

    #[test]
    fn nothing_running_and_no_job_at_first() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path());
        let s = status(&p);
        assert!(!s.installed && !s.set_up && !s.running && s.account.is_empty() && s.password.is_empty());
        assert_eq!(s.can_get, supported());
        assert_eq!(s.realm, REALM);
        assert_ne!(REALM, crate::host::lite::ADDRESS, "never Den Lite's address");
        let j = progress();
        assert!(j.idle || j.kind == "engine");
        if !supported() {
            assert!(begin(d.path(), None, false).unwrap_err().contains("Windows"));
        }
        assert!(logs(&p, 40).is_empty());
        assert!(backup(&p).unwrap_err().contains("nothing to back up"));
    }
}
