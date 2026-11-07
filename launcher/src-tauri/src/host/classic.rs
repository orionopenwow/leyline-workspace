//! Leyline Solo's engine kit (docs/LEYLINE-SOLO.md, Phase 1): the first-era world server (CMaNGOS with playerbots,
//! built by `.github/workflows/solo-engine.yml`) set up and run on this PC with no Docker, from the player's own
//! first-era game folder. Ported from Phase 0's tools/solo/solo_proof.py (setup, start, stop, status).
//!
//! Its own module, not another `engine::Kind`: that enum is shaped around the AzerothCore build (its `AC_*`
//! environment settings, programs, databases and package layout); this core has other programs, `.conf` files, one
//! SQL dump and three extractors. The small pure helpers are shared (SRP6 verifier, secrets, the progress line, the
//! port probe, the unzip and download code). One world at a time on this PC: the login and world ports are the
//! other worlds' and Den Lite's too.
//!
//! Everything is under `<app>/engine-classic/` ([`Paths`]). The generated passwords (the database's root, the
//! servers' own database login, the playing account SOLO, the stop account SOLOSTOP) live only in
//! `state/state.json`: the server `.conf` files hold a placeholder and the real database logins reach the servers as
//! environment values of their process (the core's `Config::Reload` takes `Mangosd_<Key>` / `Realmd_<Key>` over the
//! file's value), the mysql client gets its login from a short-lived file removed after each call, and every log line
//! the launcher shows is scrubbed ([`scrub`]).

use crate::host::classicsign::{self, Mode, SignState};
use crate::host::engine::{self, Job};
use crate::host::localworld::{self as lw, LocalChar};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The rolling release of the public Leyline Solo repo; [`URL_ENV`] overrides it (testing a build).
pub const RELEASE: &str = "https://github.com/orionopenwow/leyline-solo/releases/download/solo-engine-latest/";
/// Where the workflow publishes it today (`.github/workflows/solo-engine.yml`): tried when [`RELEASE`] fails.
pub const RELEASE_FALLBACK: &str = "https://github.com/orionopenwow/leyline-workspace/releases/download/solo-engine-latest/";
pub const ASSET: &str = "leyline-solo-engine-win64.zip";
pub const URL_ENV: &str = "LEYLINE_SOLO_ENGINE_URL";
/// About 90 MB today; anything over 1 GB isn't it.
pub const ZIP_MAX: u64 = 1024 * 1024 * 1024;
/// The package layout this launcher reads (solo-engine.json `layout`, the workflow's PKG_LAYOUT).
pub const LAYOUT: u64 = 1;
pub const ADDRESS: &str = "127.0.0.1";
pub const LOGIN_PORT: u16 = 3724;
pub const WORLD_PORT: u16 = 8085;
/// Its own MySQL port (the later-era engine's is 13306; a MySQL the player installed is on 3306).
pub const DB_PORT: u16 = 13336;
/// SOAP, on 127.0.0.1, only for the clean stop and the friends' logins.
pub const SOAP_PORT: u16 = 7878;
pub const ACCOUNT: &str = classicsign::ACCOUNT;
/// The stop account: game master level 3, used only over SOAP on 127.0.0.1; it never plays.
pub const ADMIN: &str = "SOLOSTOP";
pub const REALM_NAME: &str = classicsign::REALM;
pub const BUILD: u32 = 5875;
pub const DBS: [&str; 4] = ["classicmangos", "classiccharacters", "classicrealmd", "classiclogs"];
/// The servers' own database login.
pub const DB_USER: &str = "solo";
/// What the world server writes once it is up.
pub const READY: &str = "World initialized";
/// A friend's login lasts this long.
pub const FRIEND_DAYS: u64 = 7;
/// About how many files the map reader writes into maps/ for the whole first era: only for the progress figure.
pub const MAP_FILES_ABOUT: u64 = 2_500;
const EXE: &str = if cfg!(windows) { ".exe" } else { "" };
const TITLE: &str = "Leyline Solo";

pub fn release_from(over: Option<&str>) -> String {
    match over.map(str::trim) {
        Some(u) if u.starts_with("https://") && u.ends_with('/') && !u.contains(char::is_whitespace) => u.to_string(),
        _ => RELEASE.to_string(),
    }
}

/// The release folders to try, in order: the override alone when it is set, else [`RELEASE`] then
/// [`RELEASE_FALLBACK`].
pub fn releases_from(over: Option<&str>) -> Vec<String> {
    let r = release_from(over);
    if r != RELEASE {
        vec![r]
    } else {
        vec![RELEASE.to_string(), RELEASE_FALLBACK.to_string()]
    }
}

fn releases() -> Vec<String> {
    releases_from(std::env::var(URL_ENV).ok().as_deref())
}

/// The release's `.sha256` text from the first folder that has it, and that folder.
fn get_sha(p: &Paths) -> Result<(String, String), String> {
    let shaf = p.tmp.join(format!("{ASSET}.sha256"));
    let mut last = String::new();
    for base in releases() {
        match crate::upstream::download(&format!("{base}{ASSET}.sha256"), &shaf, 4096) {
            Ok(_) => {
                let t = fs::read_to_string(&shaf).unwrap_or_default();
                let _ = fs::remove_file(&shaf);
                return Ok((t, base));
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ------------------------------------------------------------------------------------------------ paths

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub app: PathBuf,
    pub root: PathBuf,
    /// The unpacked release (replaced on update; nothing of the player's in it).
    pub dist: PathBuf,
    /// dbc, maps, vmaps (and mmaps) from the player's own game folder.
    pub data: PathBuf,
    /// MySQL's data: the saves.
    pub db: PathBuf,
    /// The server settings made from the package's (no passwords in them).
    pub run: PathBuf,
    pub logs: PathBuf,
    /// state.json, characters.json, my.ini, the pid files.
    pub state: PathBuf,
    pub tmp: PathBuf,
    /// The invite's relay key and secret, the tunnel's files.
    pub share: PathBuf,
    /// The sign-in's backups of the player's own files.
    pub backup: PathBuf,
}

impl Paths {
    pub fn new(app_dir: &Path) -> Paths {
        let root = app_dir.join("engine-classic");
        Paths {
            app: app_dir.to_path_buf(),
            dist: root.join("dist"),
            data: root.join("data"),
            db: root.join("db"),
            run: root.join("run"),
            logs: root.join("logs"),
            state: root.join("state"),
            tmp: root.join("tmp"),
            share: root.join("share"),
            backup: root.join("signin-backup"),
            root,
        }
    }
    fn prog(dir: PathBuf, name: &str) -> PathBuf {
        dir.join(format!("{name}{EXE}"))
    }
    pub fn bin(&self) -> PathBuf {
        self.dist.join("bin")
    }
    pub fn mangosd(&self) -> PathBuf {
        Self::prog(self.bin(), "mangosd")
    }
    pub fn realmd(&self) -> PathBuf {
        Self::prog(self.bin(), "realmd")
    }
    pub fn tool(&self, name: &str) -> PathBuf {
        Self::prog(self.dist.join("tools"), name)
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
    pub fn conf_dist(&self, name: &str) -> PathBuf {
        self.bin().join("conf-dist").join(format!("{name}.conf.dist"))
    }
    pub fn profile(&self, name: &str) -> PathBuf {
        self.dist.join("conf").join(name)
    }
    pub fn dump(&self) -> PathBuf {
        self.dist.join("sql").join("solo-databases.sql")
    }
    pub fn meta(&self) -> PathBuf {
        self.dist.join("solo-engine.json")
    }
    pub fn state_file(&self) -> PathBuf {
        self.state.join("state.json")
    }
    pub fn chars_file(&self) -> PathBuf {
        self.state.join("characters.json")
    }
    pub fn my_ini(&self) -> PathBuf {
        self.state.join("my.ini")
    }
    pub fn pid(&self, which: &str) -> PathBuf {
        self.state.join(format!("{which}.pid"))
    }
    pub fn maps_marker(&self) -> PathBuf {
        self.data.join("leyline-maps.json")
    }
    pub fn vmaps_marker(&self) -> PathBuf {
        self.data.join("vmaps").join(".done")
    }
    pub fn mmaps_marker(&self) -> PathBuf {
        self.data.join("mmaps").join(".done")
    }
}

/// What must be in an unpacked release before it is used.
pub fn required(p: &Paths) -> Vec<PathBuf> {
    let mut v = vec![p.mangosd(), p.realmd(), p.mysqld(), p.mysql(), p.mysqladmin(), p.dump(), p.meta()];
    for t in ["ad", "vmap_extractor", "vmap_assembler", "MoveMapGen"] {
        v.push(p.tool(t));
    }
    for c in ["mangosd", "realmd", "aiplayerbot", "anticheat"] {
        v.push(p.conf_dist(c));
    }
    for c in ["solo.conf", "solo-realmd.conf", "solo-aiplayerbot.conf", "solo-anticheat.conf"] {
        v.push(p.profile(c));
    }
    v
}

// ------------------------------------------------------------------------------------------------ state

/// A friend's login: its name and when it stops (its password is only ever in the invite link).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Friend {
    pub user: String,
    pub made: u64,
    pub expires: u64,
}

/// `state/state.json`. Its Debug never shows a password.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub root_pw: String,
    pub db_pw: String,
    pub solo_pw: String,
    pub admin_pw: String,
    pub sha256: String,
    pub latest_sha256: String,
    pub version: String,
    pub maps_from: String,
    pub db_made: bool,
    pub filled: bool,
    pub accounts_made: bool,
    pub signin: SignState,
    pub friends: Vec<Friend>,
    /// Friends' logins still to delete (their time ran out while the world was stopped).
    pub gone: Vec<String>,
    /// Other adventurers roam the world (random bots); off: just the player and their companions.
    pub roaming: bool,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("passwords", &"[hidden]")
            .field("version", &self.version)
            .field("db_made", &self.db_made)
            .field("filled", &self.filled)
            .field("accounts_made", &self.accounts_made)
            .field("signin", &self.signin)
            .field("friends", &self.friends)
            .field("roaming", &self.roaming)
            .finish()
    }
}

impl State {
    pub fn secrets(&self) -> Vec<&str> {
        [&self.root_pw, &self.db_pw, &self.solo_pw, &self.admin_pw].into_iter().map(String::as_str).filter(|s| !s.is_empty()).collect()
    }
}

/// `text` with every password of this install replaced by `[hidden]`.
pub fn scrub(text: &str, s: &State) -> String {
    let mut t = text.to_string();
    for x in s.secrets() {
        t = t.replace(x, "[hidden]");
    }
    t
}

pub fn load_state(p: &Paths) -> State {
    fs::read_to_string(p.state_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_state(p: &Paths, s: &State) -> Result<(), String> {
    fs::create_dir_all(&p.state).map_err(|e| format!("Couldn't make Leyline Solo's folder: {e}"))?;
    crate::settings::write_atomic(&p.state_file(), serde_json::to_string_pretty(s).unwrap_or_default().as_bytes())
        .map_err(|e| format!("Couldn't save Leyline Solo's state: {e}"))?;
    private(&p.state_file());
    Ok(())
}

/// Only this user may read it (Unix; on Windows the app data folder is the user's own).
fn private(f: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(f, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = f;
}

/// The four passwords made once (the game sends account passwords upper-cased, at most 16 characters). True when
/// one was made.
pub fn ensure_secrets(s: &mut State) -> Result<bool, String> {
    let mut made = false;
    for (v, n) in [(&mut s.root_pw, 20), (&mut s.db_pw, 20), (&mut s.solo_pw, 16), (&mut s.admin_pw, 16)] {
        if v.len() < 12 {
            *v = engine::secret(n)?;
            made = true;
        }
    }
    Ok(made)
}

// ------------------------------------------------------------------------------------------------ the server settings

/// The `Key = Value` lines of a settings file, in order (comments and anything else skipped).
pub fn overrides(text: &str) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let l = line.trim_end_matches('\r');
        let Some((k, val)) = l.split_once('=') else { continue };
        let k = k.trim_end();
        let ok = k.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
        if ok && !l.starts_with(char::is_whitespace) {
            v.push((k.to_string(), val.trim().to_string()));
        }
    }
    v
}

fn put(v: &mut Vec<(String, String)>, k: &str, val: impl Into<String>) {
    let val = val.into();
    match v.iter_mut().find(|(x, _)| x == k) {
        Some(e) => e.1 = val,
        None => v.push((k.to_string(), val)),
    }
}

/// The core's `.conf.dist` with every `Key = Value` of the layers in its place (a later layer wins), the rest added
/// at the end, as Phase 0's `apply_conf` and the host kit's run-server.sh.
pub fn apply_conf(dist: &str, layers: &[Vec<(String, String)>]) -> String {
    let mut o: Vec<(String, String)> = Vec::new();
    for l in layers {
        for (k, v) in l {
            put(&mut o, k, v.clone());
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut lines = Vec::new();
    for line in dist.lines() {
        let l = line.trim_end_matches('\r');
        let key = overrides(l).into_iter().next().map(|(k, _)| k);
        match key.and_then(|k| o.iter().find(|(x, _)| *x == k).cloned()) {
            Some((k, v)) => {
                lines.push(format!("{k} = {v}"));
                seen.insert(k);
            }
            None => lines.push(l.to_string()),
        }
    }
    for (k, v) in &o {
        if !seen.contains(k) {
            lines.push(format!("{k} = {v}"));
        }
    }
    lines.join("\n") + "\n"
}

fn fwd(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// The core's database login: host;port;user;password;database.
pub fn db_info(s: &State, db: &str) -> String {
    format!("{ADDRESS};{DB_PORT};{DB_USER};{};{db}", s.db_pw)
}

/// What the `.conf` files say instead (the environment gives the real one).
pub fn db_placeholder(db: &str) -> String {
    format!("\"{ADDRESS};{DB_PORT};{DB_USER};(set by Leyline);{db}\"")
}

const WORLD_DBS: [(&str, &str); 4] = [
    ("LoginDatabaseInfo", "classicrealmd"),
    ("WorldDatabaseInfo", "classicmangos"),
    ("CharacterDatabaseInfo", "classiccharacters"),
    ("LogsDatabaseInfo", "classiclogs"),
];

/// The launcher's own lines for mangosd.conf, after the Solo profile: this PC's paths and ports, 127.0.0.1 only,
/// SOAP on for the clean stop, the monster paths when they were built.
pub fn mangosd_layer(p: &Paths, mmaps: bool) -> Vec<(String, String)> {
    let mut v = Vec::new();
    put(&mut v, "DataDir", format!("\"{}\"", fwd(&p.data)));
    put(&mut v, "LogsDir", format!("\"{}\"", fwd(&p.logs)));
    put(&mut v, "PidFile", "\"\"");
    for (k, db) in WORLD_DBS {
        put(&mut v, k, db_placeholder(db));
    }
    put(&mut v, "mmap.enabled", if mmaps { "1" } else { "0" });
    put(&mut v, "SOAP.Enabled", "1");
    put(&mut v, "SOAP.IP", format!("\"{ADDRESS}\""));
    put(&mut v, "SOAP.Port", SOAP_PORT.to_string());
    put(&mut v, "WorldServerPort", WORLD_PORT.to_string());
    put(&mut v, "BindIP", format!("\"{ADDRESS}\""));
    put(&mut v, "Console.Enable", "0");
    v
}

pub fn realmd_layer(p: &Paths) -> Vec<(String, String)> {
    let mut v = Vec::new();
    put(&mut v, "LogsDir", format!("\"{}\"", fwd(&p.logs)));
    put(&mut v, "PidFile", "\"\"");
    put(&mut v, "LoginDatabaseInfo", db_placeholder("classicrealmd"));
    put(&mut v, "RealmServerPort", LOGIN_PORT.to_string());
    put(&mut v, "BindIP", format!("\"{ADDRESS}\""));
    v
}

/// The world server's process environment: its real database logins (the core's `Mangosd_` + key).
pub fn mangosd_env(s: &State) -> Vec<(String, String)> {
    WORLD_DBS.iter().map(|(k, db)| (format!("Mangosd_{k}"), db_info(s, db))).collect()
}

pub fn realmd_env(s: &State) -> Vec<(String, String)> {
    vec![("Realmd_LoginDatabaseInfo".to_string(), db_info(s, "classicrealmd"))]
}

/// Roaming bots on (the setting "Other adventurers roam the world"): the module's own random bots, logged in at
/// start. TODO(owner): how many.
pub fn roaming_layer(on: bool) -> Vec<(String, String)> {
    if !on {
        return Vec::new();
    }
    [("AiPlayerbot.RandomBotAutologin", "1"), ("AiPlayerbot.RandomBotLoginAtStartup", "1"), ("AiPlayerbot.RandomBotAutoCreate", "1"),
        ("AiPlayerbot.MinRandomBots", "20"), ("AiPlayerbot.MaxRandomBots", "40"), ("AiPlayerbot.RandomBotAccountCount", "8")]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// The world's experience rate as the profile sets it ("2x"), for the page.
pub fn xp_rate(p: &Paths) -> String {
    let t = fs::read_to_string(p.profile("solo.conf")).unwrap_or_default();
    overrides(&t).into_iter().find(|(k, _)| k == "Rate.XP.Kill").map(|(_, v)| format!("{}x", v.trim_matches('"'))).unwrap_or_default()
}

/// The four `.conf` files for `run/`: (name, text). None of them holds a password.
pub fn confs(p: &Paths, mmaps: bool) -> Result<Vec<(&'static str, String)>, String> {
    confs_with(p, mmaps, false)
}

pub fn confs_with(p: &Paths, mmaps: bool, roaming: bool) -> Result<Vec<(&'static str, String)>, String> {
    let rd = |f: PathBuf| fs::read(&f).map(|b| String::from_utf8_lossy(&b).to_string()).map_err(|_| format!("The engine is missing {}: get it again.", f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    Ok(vec![
        ("mangosd.conf", apply_conf(&rd(p.conf_dist("mangosd"))?, &[overrides(&rd(p.profile("solo.conf"))?), mangosd_layer(p, mmaps)])),
        ("realmd.conf", apply_conf(&rd(p.conf_dist("realmd"))?, &[overrides(&rd(p.profile("solo-realmd.conf"))?), realmd_layer(p)])),
        ("aiplayerbot.conf", apply_conf(&rd(p.conf_dist("aiplayerbot"))?, &[overrides(&rd(p.profile("solo-aiplayerbot.conf"))?), roaming_layer(roaming)])),
        ("anticheat.conf", apply_conf(&rd(p.conf_dist("anticheat"))?, &[overrides(&rd(p.profile("solo-anticheat.conf"))?)])),
    ])
}

fn write_confs(p: &Paths, mmaps: bool, roaming: bool) -> Result<(), String> {
    fs::create_dir_all(&p.run).map_err(|e| e.to_string())?;
    for (name, text) in confs_with(p, mmaps, roaming)? {
        fs::write(p.run.join(name), text).map_err(|e| format!("Couldn't write {name}: {e}"))?;
    }
    // the servers load OpenSSL's legacy provider from their working folder (Phase 0: the core's Main.cpp on Windows)
    let leg = p.bin().join("legacy.dll");
    if leg.is_file() {
        let _ = fs::copy(&leg, p.run.join("legacy.dll"));
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------ the database

/// MySQL's settings: 127.0.0.1 only, its own port, no X protocol, no name lookups, no binary log, a small pool.
pub fn my_ini(p: &Paths) -> String {
    format!(
        "[mysqld]\nbasedir={}\ndatadir={}\nport={DB_PORT}\nbind-address={ADDRESS}\nmysqlx=OFF\nskip-name-resolve\nskip-log-bin\n\
         sql_mode=NO_ENGINE_SUBSTITUTION\nmax_allowed_packet=256M\ninnodb_buffer_pool_size=256M\nmax_connections=64\n\
         character-set-server=utf8mb4\nlog-error={}\npid-file={}\n",
        fwd(&p.dist.join("mysql")),
        fwd(&p.db),
        fwd(&p.logs.join("mysqld.err")),
        fwd(&p.pid("mysqld")),
    )
}

pub fn client_cnf(s: &State) -> String {
    format!("[client]\nuser=root\npassword={}\nhost={ADDRESS}\nport={DB_PORT}\nprotocol=TCP\n", s.root_pw)
}

/// Run once by `mysqld --initialize-insecure --init-file`: root gets the generated password, for 127.0.0.1 too.
pub fn init_sql(s: &State) -> String {
    let pw = &s.root_pw;
    format!(
        "ALTER USER 'root'@'localhost' IDENTIFIED BY '{pw}';\nCREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED BY '{pw}';\n\
         GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION;\nFLUSH PRIVILEGES;\n"
    )
}

/// The servers' own login, on the four databases only.
pub fn grants_sql(s: &State) -> String {
    let pw = &s.db_pw;
    let mut t = format!("CREATE USER IF NOT EXISTS '{DB_USER}'@'127.0.0.1' IDENTIFIED BY '{pw}';\nALTER USER '{DB_USER}'@'127.0.0.1' IDENTIFIED BY '{pw}';\n");
    for d in DBS {
        t += &format!("GRANT ALL PRIVILEGES ON `{d}`.* TO '{DB_USER}'@'127.0.0.1';\n");
    }
    t + "FLUSH PRIVILEGES;\n"
}

/// The realm list's one row: this PC, the world port, the first era's build.
pub fn realm_sql() -> String {
    format!(
        "DELETE FROM realmlist WHERE id <> 1;\nINSERT INTO realmlist (id, name, address, port, icon, realmflags, timezone, allowedSecurityLevel, population, realmbuilds) \
         VALUES (1, '{REALM_NAME}', '{ADDRESS}', {WORLD_PORT}, 0, 0, 1, 0, 0, '{BUILD}') ON DUPLICATE KEY UPDATE name = VALUES(name), address = VALUES(address), \
         port = VALUES(port), realmbuilds = VALUES(realmbuilds);\n"
    )
}

/// The login file's name for one call (removed when dropped).
struct Cnf(PathBuf);

impl Drop for Cnf {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn cnf(p: &Paths, s: &State) -> Result<Cnf, String> {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    fs::create_dir_all(&p.state).map_err(|e| e.to_string())?;
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let f = p.state.join(format!("client-{}-{n}.cnf", std::process::id()));
    fs::write(&f, client_cnf(s)).map_err(|e| e.to_string())?;
    private(&f);
    Ok(Cnf(f))
}

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

/// SQL through the bundled mysql client, on its stdin (never a command line), `-N -B` rows back.
pub fn sql(p: &Paths, s: &State, db: Option<&str>, text: &str) -> Result<String, String> {
    let c = cnf(p, s)?;
    let mut cmd = Command::new(p.mysql());
    cmd.arg(format!("--defaults-extra-file={}", c.0.display())).args(["-N", "-B", "--default-character-set=utf8mb4"]);
    if let Some(d) = db {
        cmd.arg(d);
    }
    no_window(&mut cmd);
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("The database client didn't start: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let body = text.to_string();
    let w = std::thread::spawn(move || {
        let _ = stdin.write_all(body.as_bytes());
    });
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = w.join();
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(format!("The database said no: {}", scrub(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or(""), s).chars().take(200).collect::<String>()))
    }
}

/// A whole SQL file into the server (the package's dump), the file on stdin.
fn sql_file(p: &Paths, s: &State, file: &Path) -> Result<(), String> {
    let c = cnf(p, s)?;
    let f = fs::File::open(file).map_err(|e| format!("The engine's database file is missing: {e}"))?;
    let mut cmd = Command::new(p.mysql());
    cmd.arg(format!("--defaults-extra-file={}", c.0.display())).arg("--default-character-set=utf8mb4");
    no_window(&mut cmd);
    let mut child = cmd.stdin(Stdio::from(f)).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("The database client didn't start: {e}"))?;
    let t = Instant::now();
    loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            return if st.success() {
                Ok(())
            } else {
                Err(format!("Filling the world stopped: {}", scrub(err.lines().last().unwrap_or(""), s).chars().take(200).collect::<String>()))
            };
        }
        if t.elapsed() > Duration::from_secs(3600) {
            let _ = child.kill();
            return Err("Filling the world took over an hour, so it was stopped.".into());
        }
        job(|j| j.detail = format!("{} min", t.elapsed().as_secs() / 60));
        std::thread::sleep(Duration::from_secs(1));
    }
}

// ------------------------------------------------------------------------------------------------ accounts (SRP6)

fn hex_be(le: &[u8]) -> String {
    let mut b: Vec<u8> = le.iter().rev().copied().collect();
    while b.len() > 1 && b[0] == 0 {
        b.remove(0);
    }
    b.iter().map(|x| format!("{x:02X}")).collect()
}

/// (s, v) as CMaNGOS stores them (`AccountMgr::CreateAccount`, `SRP6::CalculateVerifier`): upper-case hex of the
/// numbers. `salt` is the salt's bytes as hashed (little-endian), its top byte non-zero so its hex is 64 characters.
pub fn srp6_row(user: &str, pass: &str, salt: &[u8; 32]) -> (String, String) {
    let v = engine::verifier(user, pass, salt);
    (hex_be(salt), hex_be(&v))
}

/// 32 random bytes, the top one (the last, little-endian) non-zero.
pub fn new_salt() -> Result<[u8; 32], String> {
    use ring::rand::SecureRandom;
    let rng = ring::rand::SystemRandom::new();
    loop {
        let mut b = [0u8; 32];
        rng.fill(&mut b).map_err(|_| "no random source on this PC".to_string())?;
        if b[31] != 0 {
            return Ok(b);
        }
    }
}

fn login_ok(user: &str) -> bool {
    (3..=16).contains(&user.len()) && user.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// The account made, or its password and level set again (its characters are kept: the row is updated).
pub fn account_sql(user: &str, pass: &str, gm: u8, salt: &[u8; 32]) -> Result<String, String> {
    if !login_ok(user) || pass.is_empty() || !pass.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
        return Err("That login doesn't look right.".into());
    }
    let (s, v) = srp6_row(user, pass, salt);
    Ok(format!(
        "INSERT INTO account (username, gmlevel, v, s, joindate) VALUES ('{user}', {gm}, '{v}', '{s}', NOW()) \
         ON DUPLICATE KEY UPDATE gmlevel = VALUES(gmlevel), v = VALUES(v), s = VALUES(s), locked = 0, failed_logins = 0;\n\
         INSERT INTO realmcharacters (realmid, acctid, numchars) SELECT realmlist.id, account.id, 0 FROM realmlist, account \
         LEFT JOIN realmcharacters ON acctid = account.id WHERE acctid IS NULL;\n"
    ))
}

// ------------------------------------------------------------------------------------------------ SOAP

/// The HTTP request for one console command over the core's SOAP (urn:MaNGOS executeCommand).
pub fn soap_request(user: &str, pass: &str, command: &str) -> String {
    let esc = command.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         xmlns:ns1=\"urn:MaNGOS\"><SOAP-ENV:Body><ns1:executeCommand><command>{esc}</command></ns1:executeCommand></SOAP-ENV:Body></SOAP-ENV:Envelope>"
    );
    format!(
        "POST / HTTP/1.1\r\nHost: {ADDRESS}:{SOAP_PORT}\r\nAuthorization: Basic {}\r\nContent-Type: text/xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        engine::base64(format!("{user}:{pass}").as_bytes()),
        body.len()
    )
}

/// The answer's text, or why not (a fault, no answer).
pub fn soap_answer(raw: &str) -> Result<String, String> {
    let text = crate::host::bossrush::soap_text(raw);
    if raw.contains("<faultstring>") || !(raw.starts_with("HTTP/1.1 200") || raw.starts_with("HTTP/1.0 200")) {
        Err(if text.trim().is_empty() { raw.lines().next().unwrap_or("no answer").to_string() } else { text.trim().to_string() })
    } else {
        Ok(text)
    }
}

/// One console command as the stop account (SOAP runs at the console's level: `MaNGOSsoap.cpp`).
pub fn soap(s: &State, command: &str) -> Result<String, String> {
    let addr: SocketAddr = ([127, 0, 0, 1], SOAP_PORT).into();
    let mut c = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|e| format!("the world's console didn't answer ({e})"))?;
    let _ = c.set_read_timeout(Some(Duration::from_secs(15)));
    c.write_all(soap_request(ADMIN, &s.admin_pw, command).as_bytes()).map_err(|e| e.to_string())?;
    let mut out = String::new();
    let _ = c.read_to_string(&mut out);
    soap_answer(&out).map_err(|e| scrub(&e, s))
}

// ------------------------------------------------------------------------------------------------ the steps

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Download,
    Unpack,
    Maps,
    Vmaps,
    Mmaps,
    Database,
    Fill,
    Accounts,
    Start,
}

impl Step {
    pub fn label(self) -> &'static str {
        match self {
            Step::Download => "Getting the engine (a one-time download)",
            Step::Unpack => "Checking and unpacking it",
            Step::Maps => "Reading the maps from your game folder",
            Step::Vmaps => "Reading the line of sight (10 to 30 minutes)",
            Step::Mmaps => "Building the monster paths (1 to 3 hours)",
            Step::Database => "Setting up the database",
            Step::Fill => "Filling the world (5 to 15 minutes the first time)",
            Step::Accounts => "Making your account",
            Step::Start => "Starting the world",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Have {
    pub installed: bool,
    pub update: bool,
    pub maps: bool,
    pub vmaps: bool,
    pub mmaps: bool,
    /// The monster paths were asked for this time.
    pub want_mmaps: bool,
    pub db: bool,
    pub filled: bool,
    pub accounts: bool,
    pub running: bool,
}

/// The steps still to do, in order (each skipped once done, so Start again resumes where a failure stopped).
pub fn plan(h: &Have) -> Vec<Step> {
    let mut v = Vec::new();
    if !h.installed || h.update {
        v.extend([Step::Download, Step::Unpack]);
    }
    if !h.maps {
        v.push(Step::Maps);
    }
    if !h.vmaps {
        v.push(Step::Vmaps);
    }
    if h.want_mmaps && !h.mmaps {
        v.push(Step::Mmaps);
    }
    if !h.db {
        v.push(Step::Database);
    }
    if !h.filled {
        v.push(Step::Fill);
    }
    if !h.accounts {
        v.push(Step::Accounts);
    }
    if !h.running || !v.is_empty() {
        v.push(Step::Start);
    }
    v
}

pub fn set_up(h: &Have) -> bool {
    h.installed && h.maps && h.vmaps && h.db && h.filled && h.accounts
}

pub fn installed(p: &Paths) -> bool {
    required(p).iter().all(|f| f.exists())
}

pub fn maps_done(p: &Paths) -> bool {
    (p.maps_marker().is_file() || p.data.join(".maps-done").is_file()) && p.data.join("dbc").is_dir() && p.data.join("maps").is_dir()
}

pub fn db_made(p: &Paths) -> bool {
    p.db.join("mysql").is_dir() || p.db.join("mysql.ibd").is_file()
}

pub fn have(p: &Paths, s: &State, update: bool, want_mmaps: bool) -> Have {
    let (_, login, world) = ours_up(p);
    Have {
        installed: installed(p),
        update: update && !s.latest_sha256.is_empty() && s.latest_sha256 != s.sha256,
        maps: maps_done(p),
        vmaps: p.vmaps_marker().is_file(),
        mmaps: p.mmaps_marker().is_file(),
        want_mmaps,
        db: s.db_made && db_made(p),
        filled: s.filled,
        accounts: s.accounts_made,
        running: login && world && engine::port_answers(DB_PORT) && engine::port_answers(LOGIN_PORT) && engine::port_answers(WORLD_PORT),
    }
}

// ------------------------------------------------------------------------------------------------ the job

static JOB: Mutex<Option<(Job, Instant)>> = Mutex::new(None);
pub const JOB_KIND: &str = "classic";

pub fn progress() -> Job {
    match JOB.lock().ok().and_then(|g| g.clone()) {
        Some((mut j, t)) => {
            if j.running {
                j.secs = t.elapsed().as_secs();
            }
            j
        }
        None => Job { idle: true, kind: JOB_KIND.into(), ..Default::default() },
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

fn busy() -> bool {
    JOB.lock().ok().is_some_and(|g| g.as_ref().is_some_and(|(j, _)| j.running))
}

/// A first-era game folder: its Data folder has the first era's archives and none of a later era's.
pub fn game_ok(game: &Path) -> Result<(), String> {
    engine::game_ok(game)?;
    let d = crate::clients::find_ci(game, "Data").ok_or("That game folder has no Data folder.")?;
    let has = |n: &str| crate::clients::find_ci(&d, n).is_some();
    if ["common.MPQ", "expansion.MPQ", "lichking.MPQ", "common-2.MPQ"].iter().any(|n| has(n)) {
        return Err("That's a later game folder: Leyline Solo plays with the first-era game. Pick that folder.".into());
    }
    if !["dbc.MPQ", "terrain.MPQ"].iter().all(|n| has(n)) {
        return Err("That game folder hasn't the first era's maps (Data\\dbc.MPQ and Data\\terrain.MPQ).".into());
    }
    Ok(())
}

/// Set up (whatever isn't done) and start, in the background; follow it with [`progress`]. `game`: the first-era
/// folder the maps are read from (needed while they aren't). `mmaps`: build the monster paths too. `update`: take
/// the release's newer engine first.
pub fn begin(app_dir: &Path, game: Option<PathBuf>, mmaps: bool, update: bool) -> Result<String, String> {
    if !engine::supported() {
        return Err("Leyline Solo's engine is made for Windows PCs for now.".into());
    }
    let p = Paths::new(app_dir);
    if let Some(o) = other_running(&p) {
        return Err(format!("{o} is running on this PC and uses the same ports (ports busy). Stop it first, then press Start again."));
    }
    let s = load_state(&p);
    let steps = plan(&have(&p, &s, update, mmaps));
    if steps.is_empty() {
        return Ok(format!("{TITLE} is running."));
    }
    let needs_game = steps.iter().any(|x| matches!(x, Step::Maps | Step::Vmaps | Step::Mmaps));
    if needs_game {
        match &game {
            None => return Err(format!("Pick your first-era game folder first: {TITLE} reads its maps from it once.")),
            Some(g) => game_ok(g)?,
        }
    }
    {
        let mut g = JOB.lock().map_err(|_| "busy")?;
        if g.as_ref().is_some_and(|(j, _)| j.running) {
            return Ok(format!("{TITLE} is starting already."));
        }
        *g = Some((Job { idle: false, kind: JOB_KIND.into(), running: true, step: steps[0].label().into(), n: 1, of: steps.len(), ..Default::default() }, Instant::now()));
    }
    let msg = if steps.contains(&Step::Download) { "Getting the engine…".to_string() } else { format!("Starting {TITLE}…") };
    std::thread::spawn(move || {
        let r = run(&p, game.as_deref(), &steps);
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

fn setup_log(p: &Paths, line: &str) {
    let _ = fs::create_dir_all(&p.logs);
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(p.logs.join("setup.log")) {
        let _ = writeln!(f, "{} {line}", engine::stamp(now()));
    }
}

fn run(p: &Paths, game: Option<&Path>, steps: &[Step]) -> Result<(), String> {
    for d in [&p.root, &p.logs, &p.state, &p.tmp, &p.run] {
        fs::create_dir_all(d).map_err(|e| format!("Couldn't make Leyline Solo's folder: {e}"))?;
    }
    let mut s = load_state(p);
    if ensure_secrets(&mut s)? {
        save_state(p, &s)?;
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
            Step::Download => download(p).map(|w| want = w),
            Step::Unpack => {
                if engine::port_answers(WORLD_PORT) && ours_up(p).2 {
                    stop(p)?;
                    s = load_state(p);
                }
                let zip = p.tmp.join(ASSET);
                let r = place(p, &zip, &want, &mut s);
                let _ = fs::remove_file(&zip);
                r.and_then(|_| save_state(p, &s))
            }
            Step::Maps => {
                let g = game.ok_or("Pick your game folder first.")?;
                extract_maps(p, g).and_then(|_| {
                    s.maps_from = g.display().to_string();
                    save_state(p, &s)
                })
            }
            Step::Vmaps => extract_vmaps(p, game.ok_or("Pick your game folder first.")?),
            Step::Mmaps => build_mmaps(p),
            Step::Database => init_db(p, &s).and_then(|_| {
                s.db_made = true;
                save_state(p, &s)
            }),
            Step::Fill => start_db(p).and_then(|_| fill(p, &s)).and_then(|_| {
                s.filled = true;
                save_state(p, &s)
            }),
            Step::Accounts => start_db(p).and_then(|_| make_accounts(p, &s)).and_then(|_| {
                s.accounts_made = true;
                save_state(p, &s)
            }),
            Step::Start => start(p),
        };
        r.map_err(|e| scrub(&e, &s))?;
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------ get and unpack

fn get_error(e: &str) -> String {
    if e.ends_with("answered 404") {
        "Leyline Solo's engine isn't published yet, so it can't be downloaded. Try again later.".to_string()
    } else {
        format!("Couldn't get the engine: {e}")
    }
}

fn download(p: &Paths) -> Result<String, String> {
    let (want, base) = get_sha(p).map_err(|e| get_error(&e))?;
    if crate::host::forge::sha_in(&want).is_none() {
        return Err("The engine's checksum file doesn't look right, so nothing was downloaded. Try again later.".into());
    }
    let zip = p.tmp.join(ASSET);
    let last = std::cell::Cell::new(Instant::now().checked_sub(Duration::from_secs(1)).unwrap_or_else(Instant::now));
    crate::upstream::download_progress(&format!("{base}{ASSET}"), &zip, ZIP_MAX, &|got, size| {
        if last.get().elapsed() >= Duration::from_millis(500) {
            last.set(Instant::now());
            job(|j| {
                j.pct = size.map(|t| engine::pct(got, t));
                j.detail = format!("{} MB{}", got / 1_048_576, size.map(|t| format!(" of {} MB", t / 1_048_576)).unwrap_or_default());
            });
        }
    })
    .map_err(|e| format!("The engine didn't download: {e}"))?;
    Ok(want)
}

/// solo-engine.json's version, when the package is ours and of this layout.
pub fn read_meta(dist: &Path) -> Result<String, String> {
    let v: serde_json::Value = fs::read_to_string(dist.join("solo-engine.json")).ok().and_then(|t| serde_json::from_str(&t).ok())
        .ok_or("The engine's download has no solo-engine.json, so it wasn't used.")?;
    if v.get("name").and_then(|n| n.as_str()) != Some("leyline-solo-engine") {
        return Err("That isn't Leyline Solo's engine, so it wasn't used.".into());
    }
    if v.get("layout").and_then(|n| n.as_u64()) != Some(LAYOUT) {
        return Err("This engine is newer than this launcher: update Leyline, then try again.".into());
    }
    if v.get("build").and_then(|n| n.as_u64()).is_some_and(|b| b != BUILD as u64) {
        return Err("That engine is for another game version, so it wasn't used.".into());
    }
    Ok(v.get("version").and_then(|n| n.as_str()).unwrap_or("").chars().take(40).collect())
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

const CAPS: crate::clientimport::Caps = crate::clientimport::Caps { download: ZIP_MAX, unpacked: 3 * 1024 * 1024 * 1024, files: 40_000 };

/// The downloaded zip put in place once its sha256 is the one the release names: unpacked into `dist.new`, checked
/// (no game data, our layout, every program), then swapped with `dist`. The maps, saves and state are never touched.
pub fn place(p: &Paths, zip: &Path, want_text: &str, s: &mut State) -> Result<(), String> {
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
    if let Some(bad) = files.iter().find(|f| engine::is_game_data(&f.to_string_lossy())) {
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

/// The release's checksum now; true when it isn't the installed one.
pub fn check_update(p: &Paths) -> Result<bool, String> {
    fs::create_dir_all(&p.tmp).map_err(|e| e.to_string())?;
    let (t, _) = get_sha(p).map_err(|e| format!("Couldn't look for an engine update: {e}"))?;
    let latest = crate::host::forge::sha_in(&t).ok_or("The engine's checksum file doesn't look right.")?;
    let mut s = load_state(p);
    s.latest_sha256 = latest;
    save_state(p, &s)?;
    Ok(installed(p) && s.latest_sha256 != s.sha256)
}

// ------------------------------------------------------------------------------------------------ the maps

fn count(dir: &Path) -> u64 {
    fs::read_dir(dir).map(|rd| rd.count() as u64).unwrap_or(0)
}

/// One of the engine's tools to its end, its output in `logs/<log>`, the page's detail every second (`detail`).
fn run_tool(p: &Paths, exe: &Path, args: &[String], cwd: &Path, log: &str, detail: &dyn Fn(u64) -> (Option<u8>, String)) -> Result<(), String> {
    if !exe.is_file() {
        return Err(format!("{} isn't in the engine folder: get the engine again.", exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    }
    let f = fs::OpenOptions::new().create(true).append(true).open(p.logs.join(log)).map_err(|e| e.to_string())?;
    let f2 = f.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::from(f)).stderr(Stdio::from(f2));
    no_window(&mut cmd);
    let mut c = cmd.spawn().map_err(|e| format!("{} didn't start: {e}", exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()))?;
    let t = Instant::now();
    loop {
        if let Some(st) = c.try_wait().map_err(|e| e.to_string())? {
            return if st.success() {
                Ok(())
            } else {
                Err(format!("{} stopped (code {}); logs\\{log} has its lines.", exe.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), st.code().unwrap_or(-1)))
            };
        }
        let (pct, d) = detail(t.elapsed().as_secs());
        job(|j| {
            j.pct = pct;
            j.detail = d;
        });
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// The command lines Phase 0 runs (the core's own ExtractResources.sh): `ad -i <game> -o <data>`; `vmap_extractor
/// -d <game>/Data -o <data>`, `vmap_assembler <data>/Buildings <data>/vmaps`; MoveMapGen.
pub fn tool_args(p: &Paths, game: &Path, which: &str) -> Vec<String> {
    let s = |x: &Path| x.display().to_string();
    let data_in = crate::clients::find_ci(game, "Data").unwrap_or_else(|| game.join("Data"));
    match which {
        "ad" => vec!["-i".into(), s(game), "-o".into(), s(&p.data)],
        "vmap_extractor" => vec!["-d".into(), s(&data_in), "-o".into(), s(&p.data)],
        "vmap_assembler" => vec![s(&p.data.join("Buildings")), s(&p.data.join("vmaps"))],
        "MoveMapGen" => vec![
            "--silent".into(),
            "--configInputPath".into(),
            s(&p.dist.join("tools").join("config.json")),
            "--offMeshInput".into(),
            s(&p.dist.join("tools").join("offmesh.txt")),
            "--workdir".into(),
            format!("{}{}", s(&p.data), std::path::MAIN_SEPARATOR),
            "--buildGameObjects".into(),
            "--threads".into(),
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).to_string(),
        ],
        _ => Vec::new(),
    }
}

fn extract_maps(p: &Paths, game: &Path) -> Result<(), String> {
    game_ok(game)?;
    fs::create_dir_all(&p.data).map_err(|e| format!("Couldn't make the maps folder: {e}"))?;
    let _ = fs::remove_file(p.maps_marker());
    let maps = p.data.join("maps");
    run_tool(p, &p.tool("ad"), &tool_args(p, game, "ad"), &p.data, "extract-maps.log", &|_| {
        let n = count(&maps);
        (Some(engine::pct(n, MAP_FILES_ABOUT)), format!("{n} map files"))
    })?;
    let n = count(&maps);
    if n == 0 || count(&p.data.join("dbc")) == 0 {
        return Err("The map reader finished but wrote no maps. Is this your first-era game folder?".into());
    }
    let marker = serde_json::json!({ "from": game.display().to_string(), "at": now(), "maps": n });
    fs::write(p.maps_marker(), marker.to_string()).map_err(|e| e.to_string())
}

fn extract_vmaps(p: &Paths, game: &Path) -> Result<(), String> {
    let _ = fs::remove_dir_all(p.data.join("Buildings"));
    fs::create_dir_all(p.data.join("vmaps")).map_err(|e| e.to_string())?;
    let mins = |s: u64| -> (Option<u8>, String) { (None, format!("{} min", s / 60)) };
    run_tool(p, &p.tool("vmap_extractor"), &tool_args(p, game, "vmap_extractor"), &p.data, "extract-vmaps.log", &mins)?;
    run_tool(p, &p.tool("vmap_assembler"), &tool_args(p, game, "vmap_assembler"), &p.data, "extract-vmaps.log", &mins)?;
    let _ = fs::remove_dir_all(p.data.join("Buildings"));
    fs::write(p.vmaps_marker(), "ok").map_err(|e| e.to_string())
}

fn build_mmaps(p: &Paths) -> Result<(), String> {
    fs::create_dir_all(p.data.join("mmaps")).map_err(|e| e.to_string())?;
    let game = PathBuf::new();
    run_tool(p, &p.tool("MoveMapGen"), &tool_args(p, &game, "MoveMapGen"), &p.dist.join("tools"), "extract-mmaps.log", &|s| (None, format!("{} min", s / 60)))?;
    fs::write(p.mmaps_marker(), "ok").map_err(|e| e.to_string())
}

// ------------------------------------------------------------------------------------------------ processes

struct Procs {
    db: Option<Child>,
    login: Option<Child>,
    world: Option<Child>,
}

static PROCS: Mutex<Procs> = Mutex::new(Procs { db: None, login: None, world: None });

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

fn read_pid(file: &Path) -> Option<u32> {
    fs::read_to_string(file).ok()?.trim().parse().ok()
}

fn pid_alive(pid: u32) -> bool {
    if cfg!(windows) {
        let mut c = Command::new("tasklist");
        c.args(["/FI", &format!("PID eq {pid}"), "/NH"]);
        no_window(&mut c);
        c.output().map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().any(|w| w == pid.to_string())).unwrap_or(false)
    } else {
        Path::new(&format!("/proc/{pid}")).exists()
    }
}

fn pid_file_alive(file: &Path) -> bool {
    read_pid(file).is_some_and(pid_alive)
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

/// Ours is running: (database, login server, world server): our child, or the process in our pid file (a launcher
/// restarted while it ran).
pub fn ours_up(p: &Paths) -> (bool, bool, bool) {
    let (mut db, mut login, mut world) = (false, false, false);
    if let Ok(mut g) = PROCS.lock() {
        db = alive(&mut g.db);
        login = alive(&mut g.login);
        world = alive(&mut g.world);
    }
    (db || pid_file_alive(&p.pid("mysqld")), login || pid_file_alive(&p.pid("realmd")), world || pid_file_alive(&p.pid("mangosd")))
}

/// Another world is on these ports now (never this one): its name, for "stop it first".
pub fn other_running(p: &Paths) -> Option<String> {
    let (login, world) = (engine::port_answers(LOGIN_PORT), engine::port_answers(WORLD_PORT));
    if !login && !world {
        return None;
    }
    let (_, our_login, our_world) = ours_up(p);
    if (!login || our_login) && (!world || our_world) {
        return None;
    }
    if let Some(k) = engine::active() {
        return Some(k.title().into());
    }
    if crate::host::lite::beat_fresh(&crate::host::dens::dir(&crate::host::lite::root(&p.app))) {
        return Some("Your Den".into());
    }
    if let Some(k) = engine::KINDS.into_iter().find(|k| pid_file_alive(&engine::Paths::for_kind(&p.app, *k).pid("mysqld"))) {
        return Some(k.title().into());
    }
    Some("Another server".into())
}

/// A program of the engine, its output appended to `log`, its own environment (none of the PC's `Mangosd_`,
/// `Realmd_`, `PlayerBots_` or `Anticheat_` values), no window.
fn spawn(exe: &Path, args: &[String], cwd: &Path, env: &[(String, String)], log: &Path) -> Result<Child, String> {
    if !exe.is_file() {
        return Err(format!("{} isn't in the engine folder: get the engine again.", exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    }
    let f = fs::OpenOptions::new().create(true).append(true).open(log).map_err(|e| format!("Couldn't open the log: {e}"))?;
    let f2 = f.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::from(f)).stderr(Stdio::from(f2));
    for (k, _) in std::env::vars() {
        if ["Mangosd_", "Realmd_", "PlayerBots_", "Anticheat_", "AHBot_"].iter().any(|x| k.starts_with(x)) {
            cmd.env_remove(k);
        }
    }
    cmd.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    no_window(&mut cmd);
    cmd.spawn().map_err(|e| format!("{} didn't start: {e}", exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()))
}

fn wait_port(port: u16, secs: u64, mut ok: impl FnMut() -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(secs) {
        if engine::port_answers(port) {
            return true;
        }
        if !ok() {
            return engine::port_answers(port);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

fn wait_closed(port: u16, secs: u64) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(secs) {
        if !engine::port_answers(port) {
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

fn child_alive(which: u8) -> bool {
    let Ok(mut g) = PROCS.lock() else { return false };
    match which {
        0 => alive(&mut g.db),
        1 => alive(&mut g.login),
        _ => alive(&mut g.world),
    }
}

fn init_db(p: &Paths, s: &State) -> Result<(), String> {
    fs::create_dir_all(&p.logs).map_err(|e| e.to_string())?;
    fs::write(p.my_ini(), my_ini(p)).map_err(|e| e.to_string())?;
    if db_made(p) {
        return Ok(());
    }
    if p.db.exists() {
        let has = fs::read_dir(&p.db).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("classic"))).unwrap_or(false);
        if has {
            return Err("The database folder has saves in it but isn't complete: it was left as it is. Ask for help on Discord.".into());
        }
        fs::remove_dir_all(&p.db).map_err(|e| format!("Couldn't clear the half-made database folder: {e}"))?;
    }
    let init = p.state.join("init.sql");
    fs::write(&init, init_sql(s)).map_err(|e| e.to_string())?;
    private(&init);
    let args = vec![format!("--defaults-file={}", p.my_ini().display()), "--initialize-insecure".into(), format!("--init-file={}", fwd(&init)), "--console".into()];
    let r = (|| {
        let mut c = spawn(&p.mysqld(), &args, &p.root, &[], &p.logs.join("mysqld-init.log"))?;
        let t = Instant::now();
        loop {
            if let Some(st) = c.try_wait().map_err(|e| e.to_string())? {
                return if st.success() && db_made(p) { Ok(()) } else { Err(format!("The database couldn't be set up. {}", tail_of(&p.logs.join("mysqld.err"), 3))) };
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
    if engine::port_answers(DB_PORT) {
        return Ok(());
    }
    if !p.my_ini().is_file() {
        fs::write(p.my_ini(), my_ini(p)).map_err(|e| e.to_string())?;
    }
    let args = vec![format!("--defaults-file={}", p.my_ini().display())];
    let c = spawn(&p.mysqld(), &args, &p.root, &[], &p.logs.join("mysqld.out.log"))?;
    PROCS.lock().map_err(|_| "busy")?.db = Some(c);
    if !wait_port(DB_PORT, 120, || child_alive(0)) {
        return Err(format!("The database didn't start. {}", tail_of(&p.logs.join("mysqld.err"), 3)));
    }
    Ok(())
}

/// The package's dump imported once, the servers' login, the realm list row; the world checked.
fn fill(p: &Paths, s: &State) -> Result<(), String> {
    sql_file(p, s, &p.dump())?;
    sql(p, s, None, &grants_sql(s))?;
    sql(p, s, Some("classicrealmd"), &realm_sql())?;
    let n = sql(p, s, Some("classicmangos"), "SELECT COUNT(*) FROM creature_template;")?;
    if n.trim().parse::<u64>().unwrap_or(0) < 1000 {
        return Err("The world database looks empty after filling it. Press Start again; if it stays so, ask for help on Discord.".into());
    }
    Ok(())
}

fn make_accounts(p: &Paths, s: &State) -> Result<(), String> {
    sql(p, s, None, &grants_sql(s))?;
    for (user, pw, gm) in [(ACCOUNT, &s.solo_pw, 0u8), (ADMIN, &s.admin_pw, 3u8)] {
        sql(p, s, Some("classicrealmd"), &account_sql(user, pw, gm, &new_salt()?)?)?;
    }
    Ok(())
}

fn world_log(p: &Paths) -> PathBuf {
    p.logs.join("Server.log")
}

/// The world server said it is up (its log, or its own output).
pub fn ready_seen(p: &Paths) -> bool {
    [world_log(p), p.logs.join("mangosd.out.log")].iter().any(|f| fs::read(f).map(|b| String::from_utf8_lossy(&b).contains(READY)).unwrap_or(false))
}

/// MySQL, the login server, the world server, all on 127.0.0.1; up when the world server says so and the ports
/// answer. Then friends' logins whose time ran out are removed.
pub fn start(p: &Paths) -> Result<(), String> {
    if let Some(o) = other_running(p) {
        return Err(format!("{o} is running on this PC and uses the same ports (ports busy). Stop it first, then press Start again."));
    }
    let mut s = load_state(p);
    write_confs(p, p.mmaps_marker().is_file(), s.roaming)?;
    fs::write(p.my_ini(), my_ini(p)).map_err(|e| e.to_string())?;
    start_db(p)?;
    let (_, our_login, our_world) = ours_up(p);
    if !our_login && engine::port_answers(LOGIN_PORT) {
        return Err(format!("Another server on this PC is using the login port ({LOGIN_PORT}), so {TITLE} can't start (ports busy). Stop that server, then press Start again."));
    }
    if !our_world && engine::port_answers(WORLD_PORT) {
        return Err(format!("Another server on this PC is using the world port ({WORLD_PORT}), so {TITLE} can't start (ports busy). Stop that server, then press Start again."));
    }
    if !engine::port_answers(LOGIN_PORT) {
        job(|j| j.detail = "The login server".into());
        let c = spawn(&p.realmd(), &["-c".into(), p.run.join("realmd.conf").display().to_string()], &p.run, &realmd_env(&s), &p.logs.join("realmd.out.log"))?;
        let _ = fs::write(p.pid("realmd"), c.id().to_string());
        PROCS.lock().map_err(|_| "busy")?.login = Some(c);
        if !wait_port(LOGIN_PORT, 60, || child_alive(1)) {
            return Err(format!("The login server didn't start. {}", scrub(&tail_of(&p.logs.join("realmd.out.log"), 3), &s)));
        }
    }
    if !engine::port_answers(WORLD_PORT) {
        job(|j| j.detail = "The world (the first start takes a minute or two)".into());
        for f in [world_log(p), p.logs.join("mangosd.out.log")] {
            let _ = fs::remove_file(f);
        }
        let args = vec!["-c".into(), p.run.join("mangosd.conf").display().to_string(), "-p".into(), p.run.join("aiplayerbot.conf").display().to_string()];
        let c = spawn(&p.mangosd(), &args, &p.run, &mangosd_env(&s), &p.logs.join("mangosd.out.log"))?;
        let _ = fs::write(p.pid("mangosd"), c.id().to_string());
        PROCS.lock().map_err(|_| "busy")?.world = Some(c);
        let t = Instant::now();
        loop {
            if ready_seen(p) && engine::port_answers(WORLD_PORT) {
                break;
            }
            if !child_alive(2) {
                return Err(format!("The world server stopped. {}", scrub(&tail_of(&world_log(p), 3), &s)));
            }
            if t.elapsed() > Duration::from_secs(30 * 60) {
                return Err(format!("The world server didn't say it was ready in time. {}", scrub(&tail_of(&world_log(p), 3), &s)));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let _ = sweep_friends(p, &mut s, now(), false);
    save_state(p, &s)
}

/// Everyone saved and everything down: friends' logins removed, the world by SOAP's `server shutdown` (120 s, then
/// ended), the login server, then MySQL by `mysqladmin shutdown`. The password leaves the sign-in archive too.
pub fn stop(p: &Paths) -> Result<String, String> {
    let mut s = load_state(p);
    let (_, our_login, our_world) = ours_up(p);
    let mut forced = false;
    if our_world && engine::port_answers(WORLD_PORT) {
        let _ = refresh_chars(p);
        let _ = sweep_friends(p, &mut s, now(), true);
    }
    let mut g = PROCS.lock().map_err(|_| "busy")?;
    if our_world || alive(&mut g.world) {
        let asked = s.accounts_made && soap(&s, "server shutdown 1").is_ok();
        let t = Instant::now();
        while asked && t.elapsed() < Duration::from_secs(120) && (alive(&mut g.world) || pid_file_alive(&p.pid("mangosd"))) {
            std::thread::sleep(Duration::from_millis(500));
        }
        if alive(&mut g.world) || pid_file_alive(&p.pid("mangosd")) {
            forced = true;
            if let Some(mut c) = g.world.take() {
                let _ = c.kill();
                let _ = c.wait();
            } else if let Some(pid) = read_pid(&p.pid("mangosd")) {
                kill_pid(pid);
            }
        }
    }
    if let Some(mut c) = g.login.take() {
        let _ = c.kill();
        let _ = c.wait();
    } else if our_login {
        if let Some(pid) = read_pid(&p.pid("realmd")) {
            kill_pid(pid);
        }
    }
    let _ = fs::remove_file(p.pid("realmd"));
    let _ = fs::remove_file(p.pid("mangosd"));
    if engine::port_answers(DB_PORT) && s.db_made {
        if let Ok(c) = cnf(p, &s) {
            let mut cmd = Command::new(p.mysqladmin());
            cmd.arg(format!("--defaults-extra-file={}", c.0.display())).arg("shutdown");
            no_window(&mut cmd);
            let _ = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        }
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
    drop(g);
    let _ = classicsign::scrub_session(&mut s.signin);
    save_state(p, &s)?;
    Ok(if forced {
        "Stopped. Something didn't stop by itself in time and was ended: your characters were saved when you last left a zone or logged out.".into()
    } else {
        "Stopped. Everyone was saved.".into()
    })
}

/// The last lines of each log, labelled, no password in them.
pub fn logs(p: &Paths, lines: usize) -> String {
    let s = load_state(p);
    let each = (lines / 6).max(10);
    let mut out = Vec::new();
    for (label, file) in [("setup", "setup.log"), ("world", "Server.log"), ("world", "mangosd.out.log"), ("login", "Realmd.log"), ("login", "realmd.out.log"),
        ("database", "mysqld.err"), ("maps", "extract-maps.log"), ("vmaps", "extract-vmaps.log"), ("mmaps", "extract-mmaps.log")] {
        for l in tail_of(&p.logs.join(file), each).lines().filter(|l| !l.trim().is_empty()) {
            out.push(format!("{label}: {}", scrub(l, &s)));
        }
    }
    out.join("\n")
}

// ------------------------------------------------------------------------------------------------ status

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct SigninStatus {
    pub on: bool,
    pub mode: String,
    pub game: String,
    /// Our archive is in the folder now.
    pub addon: bool,
    pub why: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ClassicStatus {
    pub supported: bool,
    pub installed: bool,
    pub can_get: bool,
    pub version: String,
    pub update: bool,
    pub maps: bool,
    pub vmaps: bool,
    pub mmaps: bool,
    pub db: bool,
    pub accounts: bool,
    pub set_up: bool,
    pub running: bool,
    pub db_up: bool,
    pub login_up: bool,
    pub world_up: bool,
    pub busy: bool,
    pub account: String,
    pub realm_name: String,
    pub root: String,
    pub game: String,
    pub other_running: Option<String>,
    pub signin: SigninStatus,
    pub sharing: bool,
    pub friends: usize,
    /// The setup/start job's progress line while one runs (the same as classic_progress).
    pub progress: Option<Job>,
    /// "2x" (the profile's experience rate; TODO(owner)).
    pub xp_rate: String,
    pub logs_dir: String,
    pub roaming: bool,
}

pub fn status(p: &Paths) -> ClassicStatus {
    let s = load_state(p);
    let h = have(p, &s, false, false);
    let (db, login, world) = ours_up(p);
    let su = set_up(&h);
    let other = other_running(p);
    let t = now();
    ClassicStatus {
        supported: engine::supported(),
        installed: h.installed,
        can_get: !h.installed && engine::supported(),
        version: s.version.clone(),
        update: h.installed && !s.latest_sha256.is_empty() && s.latest_sha256 != s.sha256,
        maps: h.maps,
        vmaps: h.vmaps,
        mmaps: h.mmaps,
        db: h.db && h.filled,
        accounts: h.accounts,
        set_up: su,
        running: su && h.running,
        db_up: su && db && engine::port_answers(DB_PORT),
        login_up: su && login && engine::port_answers(LOGIN_PORT),
        world_up: su && world && engine::port_answers(WORLD_PORT),
        busy: busy(),
        account: if h.accounts { ACCOUNT.into() } else { String::new() },
        realm_name: REALM_NAME.into(),
        root: p.root.display().to_string(),
        game: s.maps_from.clone(),
        other_running: other,
        signin: SigninStatus {
            on: s.signin.on,
            mode: s.signin.mode.clone(),
            game: s.signin.game.clone(),
            addon: s.signin.on && !s.signin.added.is_empty(),
            why: s.signin.why.clone(),
        },
        sharing: sharing(p),
        friends: s.friends.iter().filter(|f| f.expires > t).count(),
        progress: busy().then(progress),
        xp_rate: xp_rate(p),
        logs_dir: p.logs.display().to_string(),
        roaming: s.roaming,
    }
}

// ------------------------------------------------------------------------------------------------ play

/// What a start's sign-in did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Signed {
    pub mode: Mode,
    /// The add-on signs in by itself this time.
    pub auto: bool,
    pub why: String,
    pub lines: Vec<String>,
}

/// The sign-in written for a start in `game` (the world must be running): prefill, or the add-on with this start's
/// `pick` / `make`.
pub fn sign_in(p: &Paths, game: &Path, mode: Mode, pick: Option<String>, make: Option<crate::hybrid::NewChar>) -> Result<Signed, String> {
    let mut s = load_state(p);
    if !s.accounts_made {
        return Err(format!("Set {TITLE} up first: press Start on its page."));
    }
    let session = (mode == Mode::Addon).then(|| classicsign::Session { user: ACCOUNT.into(), pass: s.solo_pw.clone(), realm: REALM_NAME.into(), pick, make });
    let r = classicsign::install(game, &p.backup, &mut s.signin, mode, session.as_ref());
    save_state(p, &s)?;
    let r = r?;
    Ok(Signed { mode, auto: r.addon, why: r.why, lines: r.lines })
}

/// The roaming-bots setting (taken at the next start).
pub fn set_roaming(p: &Paths, on: bool) -> Result<(), String> {
    let mut s = load_state(p);
    if s.roaming != on {
        s.roaming = on;
        save_state(p, &s)?;
    }
    Ok(())
}

/// SOLO's password, for the page to show once and copy (never logged, never in another answer).
pub fn password(p: &Paths) -> Result<String, String> {
    let s = load_state(p);
    if !s.accounts_made || s.solo_pw.is_empty() {
        return Err(format!("Set {TITLE} up first: press Start on its page."));
    }
    Ok(s.solo_pw)
}

/// The password out of the sign-in archive (the game let go of it). Err while the game holds it.
pub fn scrub_signin(p: &Paths) -> Result<bool, String> {
    let mut s = load_state(p);
    let r = classicsign::scrub_session(&mut s.signin);
    save_state(p, &s)?;
    r
}

/// Sign-in off: every file put back as it was.
pub fn signin_off(p: &Paths) -> Result<String, String> {
    let mut s = load_state(p);
    let r = classicsign::restore(&p.backup, &mut s.signin);
    save_state(p, &s)?;
    r.map(|lines| lines.join(" "))
}

// ------------------------------------------------------------------------------------------------ characters

/// One character of the first-era world: the Characters page's row, and whether it is a companion.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassicChar {
    #[serde(flatten)]
    pub base: LocalChar,
    pub companion: bool,
    pub online: bool,
    /// Where it is, in words ("" when not one of the named zones), and the zone's id.
    pub zone: String,
    pub zone_id: u32,
}

/// The first era's zones and cities by their area id (the characters table's `zone`), for the Characters page.
pub const ZONES: &[(u32, &str)] = &[
    (1, "Dun Morogh"), (3, "Badlands"), (4, "Blasted Lands"), (8, "Swamp of Sorrows"), (10, "Duskwood"), (11, "Wetlands"),
    (12, "Elwynn Forest"), (14, "Durotar"), (15, "Dustwallow Marsh"), (16, "Azshara"), (17, "The Barrens"), (28, "Western Plaguelands"),
    (33, "Stranglethorn Vale"), (36, "Alterac Mountains"), (38, "Loch Modan"), (40, "Westfall"), (41, "Deadwind Pass"), (44, "Redridge Mountains"),
    (45, "Arathi Highlands"), (46, "Burning Steppes"), (47, "The Hinterlands"), (51, "Searing Gorge"), (85, "Tirisfal Glades"),
    (130, "Silverpine Forest"), (139, "Eastern Plaguelands"), (141, "Teldrassil"), (148, "Darkshore"), (215, "Mulgore"),
    (267, "Hillsbrad Foothills"), (331, "Ashenvale"), (357, "Feralas"), (361, "Felwood"), (400, "Thousand Needles"), (405, "Desolace"),
    (406, "Stonetalon Mountains"), (440, "Tanaris"), (490, "Un'Goro Crater"), (493, "Moonglade"), (618, "Winterspring"), (1377, "Silithus"),
    (1497, "Undercity"), (1519, "Stormwind City"), (1537, "Ironforge"), (1637, "Orgrimmar"), (1638, "Thunder Bluff"), (1657, "Darnassus"),
];

pub fn zone_label(id: u32) -> &'static str {
    ZONES.iter().find(|z| z.0 == id).map(|z| z.1).unwrap_or("")
}

/// A row of [`chars_sql`] before the companion rule.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    pub guid: u64,
    pub ch: LocalChar,
    pub online: bool,
    /// Rows the playerbots module saved for it (`ai_playerbot_db_store`).
    pub store: u64,
    /// Its group's leader (0: none).
    pub leader: u64,
    pub zone: u32,
}

/// SOLO's characters (never the stop account's, a friend's or a bot account's), with the two companion marks:
/// the playerbots module's saved strategies for the guid, and the group's leader.
pub fn chars_sql() -> String {
    format!(
        "SELECT c.guid, c.name, c.race, c.class, c.gender, c.level, c.totaltime, c.logout_time, c.online, \
         (SELECT COUNT(*) FROM classiccharacters.ai_playerbot_db_store d WHERE d.guid = c.guid), \
         COALESCE((SELECT g.leaderGuid FROM classiccharacters.group_member m JOIN classiccharacters.`groups` g ON g.groupId = m.groupId \
         WHERE m.memberGuid = c.guid LIMIT 1), 0), c.zone \
         FROM classiccharacters.characters c JOIN classicrealmd.account a ON a.id = c.account WHERE a.username = '{ACCOUNT}' ORDER BY c.guid;"
    )
}

pub fn parse_rows(rows: &str) -> Vec<Row> {
    let mut v = Vec::new();
    for line in rows.lines() {
        let f: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
        if f.len() < 12 || !crate::hybrid::char_name_ok(f[1]) {
            continue;
        }
        let n = |i: usize| f[i].trim().parse::<u64>().ok();
        let (Some(guid), Some(race), Some(class), Some(gender), Some(level)) = (n(0), n(2), n(3), n(4), n(5)) else { continue };
        let (race, class) = (race as u8, class as u8);
        v.push(Row {
            guid,
            ch: LocalChar {
                name: f[1].to_string(),
                race,
                class,
                female: gender == 1,
                level: level.min(255) as u8,
                played: n(6).unwrap_or(0),
                last: n(7).unwrap_or(0),
                race_label: lw::race_label(race).into(),
                class_label: lw::class_label(class).into(),
                side: lw::side(race).into(),
            },
            online: n(8).unwrap_or(0) == 1,
            store: n(9).unwrap_or(0),
            leader: n(10).unwrap_or(0),
            zone: n(11).unwrap_or(0) as u32,
        });
    }
    v
}

/// Which of SOLO's characters are companions (docs/LEYLINE-SOLO.md, "Companions"). The playerbots module makes a
/// companion as a normal character on the player's own account, so the account doesn't tell. Live: one account has
/// one player session, so an online character in a group led by **another** online SOLO character is a bot, and the
/// online one that leads (or is the only one online) is the player, whatever else says. Otherwise: the module saved
/// its strategies for it (`ai_playerbot_db_store`), which it does only for bots.
pub fn mark_companions(rows: &[Row]) -> Vec<ClassicChar> {
    let online: Vec<u64> = rows.iter().filter(|r| r.online).map(|r| r.guid).collect();
    rows.iter()
        .map(|r| {
            let led_by_other = r.online && r.leader != 0 && r.leader != r.guid && online.contains(&r.leader);
            let the_player = r.online && !led_by_other && (online.len() == 1 || rows.iter().any(|o| o.guid != r.guid && o.online && o.leader == r.guid));
            let companion = if led_by_other {
                true
            } else if the_player {
                false
            } else {
                r.store > 0
            };
            ClassicChar { base: r.ch.clone(), companion, online: r.online, zone: zone_label(r.zone).into(), zone_id: r.zone }
        })
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kept {
    pub at: u64,
    pub chars: Vec<ClassicChar>,
}

pub fn read_kept(p: &Paths) -> Kept {
    fs::read_to_string(p.chars_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// The characters read now (the world must be running) and kept for when it isn't.
pub fn refresh_chars(p: &Paths) -> Result<Vec<ClassicChar>, String> {
    let s = load_state(p);
    if !s.accounts_made || !engine::port_answers(DB_PORT) || !ours_up(p).0 {
        return Err(format!("{TITLE} isn't running."));
    }
    let chars = mark_companions(&parse_rows(&sql(p, &s, None, &chars_sql())?));
    let k = Kept { at: now(), chars: chars.clone() };
    fs::create_dir_all(&p.state).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&p.chars_file(), serde_json::to_string_pretty(&k).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())?;
    Ok(chars)
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ClassicChars {
    pub running: bool,
    pub live: bool,
    pub at: u64,
    pub chars: Vec<ClassicChar>,
}

pub fn characters(p: &Paths) -> ClassicChars {
    let st = status(p);
    let fresh = if st.running { refresh_chars(p).ok() } else { None };
    let kept = read_kept(p);
    ClassicChars { running: st.running, live: fresh.is_some(), at: if fresh.is_some() { now() } else { kept.at }, chars: fresh.unwrap_or(kept.chars) }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassicChoices {
    pub races: Vec<lw::RaceChoice>,
    pub first_era: bool,
    pub name: String,
}

/// Quick start's choices: the first era's races and their classes, a free name.
pub fn choices(p: &Paths) -> ClassicChoices {
    let taken: Vec<String> = read_kept(p).chars.into_iter().map(|c| c.base.name).collect();
    ClassicChoices { races: lw::choices(true), first_era: true, name: lw::name_for("", &taken).unwrap_or_default() }
}

// ------------------------------------------------------------------------------------------------ friends

/// The console line that makes a friend's login (a player: no rights), the shape `worldshare::new_guest` makes.
pub fn friend_create(user: &str, pass: &str) -> Result<String, String> {
    let ok = |s: &str, n: std::ops::RangeInclusive<usize>| n.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if !(user.starts_with('G') && ok(user, 4..=16) && ok(pass, 8..=16)) || user == ACCOUNT || user == ADMIN {
        return Err("That login doesn't look right.".into());
    }
    Ok(format!("account create {user} {pass}"))
}

pub fn friend_delete(user: &str) -> Result<String, String> {
    if !(user.starts_with('G') && (4..=16).contains(&user.len()) && user.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())) || user == ACCOUNT || user == ADMIN {
        return Err("That login doesn't look right.".into());
    }
    Ok(format!("account delete {user}"))
}

/// The console's answer to `account delete`: gone (or already not there).
pub fn deleted(answer: &str) -> bool {
    let a = answer.to_ascii_lowercase();
    a.contains("deleted") || a.contains("not exist") || a.contains("not found")
}

/// The friends whose time is up at `t` (or all of them), and the ones kept.
pub fn due(friends: &[Friend], t: u64, all: bool) -> (Vec<Friend>, Vec<Friend>) {
    friends.iter().cloned().partition(|f| all || f.expires <= t)
}

/// Friends' logins whose time ran out (or all, on stop) deleted over the console; the ones it couldn't reach stay
/// in `gone` for the next start. How many went.
pub fn sweep_friends(p: &Paths, s: &mut State, t: u64, all: bool) -> usize {
    let (out, keep) = due(&s.friends, t, all);
    s.friends = keep;
    let mut todo: Vec<String> = s.gone.drain(..).collect();
    todo.extend(out.into_iter().map(|f| f.user));
    let mut n = 0;
    let up = engine::port_answers(SOAP_PORT) && ours_up(p).2;
    for u in todo {
        let gone = up && friend_delete(&u).ok().is_some_and(|line| soap(s, &line).map(|a| deleted(&a)).unwrap_or(false));
        if gone {
            n += 1;
        } else if !s.gone.contains(&u) {
            s.gone.push(u);
        }
    }
    let _ = save_state(p, s);
    n
}

/// A new friend's login, made over the console (the world must be running), kept for [`FRIEND_DAYS`]: (user,
/// password, when it stops). The password is only returned, never kept.
pub fn add_friend(p: &Paths) -> Result<(String, String, u64), String> {
    let mut s = load_state(p);
    if !ours_up(p).2 || !engine::port_answers(WORLD_PORT) {
        return Err(format!("Start {TITLE} first: friends join it while it runs."));
    }
    let _ = sweep_friends(p, &mut s, now(), false);
    let (user, pass) = crate::host::worldshare::new_guest()?;
    let a = soap(&s, &friend_create(&user, &pass)?).map_err(|e| format!("The world didn't make your friend's login: {e}"))?;
    crate::host::worldshare::created(&a).map_err(|e| format!("The world didn't make your friend's login: {e}"))?;
    let t = now();
    let expires = t + FRIEND_DAYS * 86_400;
    s.friends.push(Friend { user: user.clone(), made: t, expires });
    save_state(p, &s)?;
    Ok((user, pass, expires))
}

/// Every friend's login removed (sharing off).
pub fn remove_friends(p: &Paths) -> usize {
    let mut s = load_state(p);
    sweep_friends(p, &mut s, now(), true)
}

pub fn friends(p: &Paths) -> Vec<Friend> {
    let t = now();
    load_state(p).friends.into_iter().filter(|f| f.expires > t).collect()
}

/// The share folder ready for the tunnel: the engine's ports, the first era's realm list layout.
pub fn share_files(p: &Paths) -> Result<(), String> {
    fs::create_dir_all(&p.share).map_err(|e| e.to_string())?;
    fs::write(p.share.join("den-lite.remote"), format!("{LOGIN_PORT} {WORLD_PORT}\n")).map_err(|e| e.to_string())?;
    fs::write(p.share.join(crate::host::worldshare::LAYOUT_FILE), "classic\n").map_err(|e| e.to_string())
}

/// This world is shared now (its same-network forwarder, or its tunnel).
pub fn sharing(p: &Paths) -> bool {
    use crate::host::{litenet, worldshare as ws};
    if ws::lan_layout() == Some(ws::Layout::Classic) {
        return true;
    }
    // only an existing key is read (status is polled: it never makes one)
    if !p.share.join("den-lite.remote").is_file() || !litenet::key_path(&p.share).is_file() {
        return false;
    }
    let mine = litenet::load_or_create_key(&litenet::key_path(&p.share)).and_then(|k| litenet::public_key(&k)).map(|k| litenet::realm_id(&k));
    let t = litenet::state();
    mine.is_ok_and(|m| t.running && t.realm_id == m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st() -> State {
        State { root_pw: "ROOTPW23456789ABCDEF".into(), db_pw: "DBPW23456789ABCDEFGH".into(), solo_pw: "SOLOPW7777777777".into(), admin_pw: "ADMINPW888888888".into(), ..Default::default() }
    }

    fn le(hex: &str) -> [u8; 32] {
        let mut b: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
        b.reverse();
        b.try_into().unwrap()
    }

    #[test]
    fn srp6_rows_are_the_cores_own_sample_accounts() {
        // realmd.sql's sample rows (their passwords are their names), as Phase 0's test has them
        for (user, s, v) in [
            ("ADMINISTRATOR", "8EB5DE915AA3D805FA7099CF61C0BB8A77990EA869078A0C5B9EEE55828F4505", "312B99EEF1C0196BB73B79D114CE161C5D089319E6EF54FAA6117DAB8B672C14"),
            ("PLAYER", "EBA23AF194D89B8061CA7FEBA06D336B1C38D8FBDABA76F2C51D45141362D881", "3738EC7E7C731FD431C716990C6D97CA5C1D50EF0DA7DE9819076DE1D03AA891"),
        ] {
            assert_eq!(srp6_row(user, &user.to_ascii_lowercase(), &le(s)), (s.to_string(), v.to_string()), "{user}");
        }
        for _ in 0..10 {
            let salt = new_salt().unwrap();
            let (s, _) = srp6_row("SOLO", "ABC", &salt);
            assert_eq!(s.len(), 64);
            assert_ne!(&s[..2], "00");
        }
        let q = account_sql("SOLO", "ABCDEFGH23456789", 0, &le("8EB5DE915AA3D805FA7099CF61C0BB8A77990EA869078A0C5B9EEE55828F4505")).unwrap();
        assert!(q.starts_with("INSERT INTO account (username, gmlevel, v, s, joindate) VALUES ('SOLO', 0, '") && q.contains("realmcharacters"));
        assert!(!q.contains("ABCDEFGH23456789"), "the password is never in the SQL");
        assert!(account_sql("so lo", "X", 0, &[1; 32]).is_err() && account_sql("SOLO", "a'b", 0, &[1; 32]).is_err());
    }

    #[test]
    fn its_own_install_beside_the_other_worlds() {
        let p = Paths::new(Path::new("/app"));
        assert_eq!(p.root, Path::new("/app/engine-classic"));
        assert_ne!(p.root, engine::Paths::new(Path::new("/app")).root);
        assert!(required(&p).contains(&p.dump()) && required(&p).contains(&p.tool("vmap_assembler")));
        assert_eq!(release_from(None), RELEASE);
        assert_eq!(release_from(Some("https://example.org/b/")), "https://example.org/b/");
        assert_eq!(release_from(Some("http://example.org/b/")), RELEASE, "https only");
        assert_eq!(release_from(Some("https://example.org/b")), RELEASE, "a folder");
        assert_eq!(releases_from(None), [RELEASE, RELEASE_FALLBACK], "the public repo's release, then the workspace's");
        assert_eq!(releases_from(Some("https://example.org/b/")), ["https://example.org/b/"], "an override alone");
        assert!(RELEASE.contains("/leyline-solo/") && RELEASE_FALLBACK.contains("/leyline-workspace/"));
        assert_ne!(DB_PORT, engine::DB_PORT, "its own database port");
        assert_eq!((LOGIN_PORT, WORLD_PORT), (engine::LOGIN_PORT, engine::WORLD_PORT), "one world at a time");
    }

    #[test]
    fn the_plan_skips_what_is_done_and_mmaps_only_when_asked() {
        let none = Have::default();
        assert_eq!(plan(&none), [Step::Download, Step::Unpack, Step::Maps, Step::Vmaps, Step::Database, Step::Fill, Step::Accounts, Step::Start]);
        assert!(plan(&Have { want_mmaps: true, ..none }).contains(&Step::Mmaps));
        let done = Have { installed: true, maps: true, vmaps: true, db: true, filled: true, accounts: true, ..none };
        assert!(set_up(&done));
        assert_eq!(plan(&done), [Step::Start]);
        assert!(plan(&Have { running: true, ..done }).is_empty());
        assert_eq!(plan(&Have { running: true, want_mmaps: true, ..done }), [Step::Mmaps, Step::Start]);
        assert_eq!(plan(&Have { update: true, running: true, ..done }), [Step::Download, Step::Unpack, Step::Start]);
        for s in [Step::Download, Step::Maps, Step::Vmaps, Step::Mmaps, Step::Fill, Step::Start] {
            assert!(!s.label().is_empty());
        }
    }

    #[test]
    fn secrets_are_made_once_and_never_shown() {
        let mut s = State::default();
        assert!(ensure_secrets(&mut s).unwrap());
        let first = s.clone();
        assert!(!ensure_secrets(&mut s).unwrap());
        assert_eq!(s, first);
        for x in s.secrets() {
            assert!(x.len() >= 16 && x.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()), "the game's shape");
        }
        assert_eq!(s.secrets().len(), 4);
        let dbg = format!("{s:?}");
        for x in s.secrets() {
            assert!(!dbg.contains(x), "never in a debug line");
        }
        let line = format!("login {} failed for {} and {}", s.db_pw, s.solo_pw, s.admin_pw);
        assert_eq!(scrub(&line, &s), "login [hidden] failed for [hidden] and [hidden]");
        // the state file itself holds them (and only it, of everything written to disk: see the conf test)
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path());
        save_state(&p, &s).unwrap();
        assert_eq!(load_state(&p), s);
    }

    #[test]
    fn the_server_settings_hold_no_password_and_the_environment_does() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path());
        fs::create_dir_all(p.bin().join("conf-dist")).unwrap();
        fs::create_dir_all(p.dist.join("conf")).unwrap();
        fs::write(p.conf_dist("mangosd"), "# the core's\nLoginDatabaseInfo = \"127.0.0.1;3306;mangos;mangos;classicrealmd\"\nDataDir = \".\"\nRate.XP.Kill = 1\n").unwrap();
        fs::write(p.conf_dist("realmd"), "LoginDatabaseInfo = \"x\"\nRealmServerPort = 3724\n").unwrap();
        fs::write(p.conf_dist("aiplayerbot"), "AiPlayerbot.Enabled = 1\nAiPlayerbot.RandomBotAutologin = 1\n").unwrap();
        fs::write(p.conf_dist("anticheat"), "Warden.Enabled = 1\n").unwrap();
        fs::write(p.profile("solo.conf"), "# ours\nRate.XP.Kill = 2\nMaxPlayerLevel = 60\n").unwrap();
        fs::write(p.profile("solo-realmd.conf"), "").unwrap();
        fs::write(p.profile("solo-aiplayerbot.conf"), "AiPlayerbot.RandomBotAutologin = 0\n").unwrap();
        fs::write(p.profile("solo-anticheat.conf"), "Warden.Enabled = 0\n").unwrap();
        let s = st();
        write_confs(&p, false, false).unwrap();
        let m = fs::read_to_string(p.run.join("mangosd.conf")).unwrap();
        assert!(m.contains("# the core's\nLoginDatabaseInfo = \"127.0.0.1;13336;solo;(set by Leyline);classicrealmd\"\n"), "{m}");
        assert!(m.contains("Rate.XP.Kill = 2") && m.contains("MaxPlayerLevel = 60") && m.contains("SOAP.Port = 7878") && m.contains("mmap.enabled = 0"));
        assert!(m.contains(&format!("DataDir = \"{}\"", fwd(&p.data))));
        assert!(fs::read_to_string(p.run.join("aiplayerbot.conf")).unwrap().contains("AiPlayerbot.RandomBotAutologin = 0"), "no random bots");
        assert!(fs::read_to_string(p.run.join("anticheat.conf")).unwrap().contains("Warden.Enabled = 0"));
        let roam = confs_with(&p, false, true).unwrap().into_iter().find(|c| c.0 == "aiplayerbot.conf").unwrap().1;
        assert!(roam.contains("AiPlayerbot.RandomBotAutologin = 1") && roam.contains("AiPlayerbot.MaxRandomBots = 40"), "roaming bots when asked");
        assert_eq!(xp_rate(&p), "2x");
        for f in fs::read_dir(&p.run).unwrap().flatten() {
            let t = fs::read_to_string(f.path()).unwrap();
            for x in s.secrets() {
                assert!(!t.contains(x), "{:?} holds a password", f.file_name());
            }
        }
        let env = mangosd_env(&s);
        assert_eq!(env.len(), 4);
        assert!(env.iter().any(|(k, v)| k == "Mangosd_LoginDatabaseInfo" && v == "127.0.0.1;13336;solo;DBPW23456789ABCDEFGH;classicrealmd"));
        assert_eq!(realmd_env(&s)[0].0, "Realmd_LoginDatabaseInfo");
        // the database's own files: the root password only in the short-lived login file
        assert!(!my_ini(&p).contains(&s.root_pw));
        let c = cnf(&p, &s).unwrap();
        assert!(fs::read_to_string(&c.0).unwrap().contains(&s.root_pw));
        let path = c.0.clone();
        drop(c);
        assert!(!path.exists(), "removed after the call");
        assert!(grants_sql(&s).contains("GRANT ALL PRIVILEGES ON `classicrealmd`.* TO 'solo'@'127.0.0.1'"));
        assert!(realm_sql().contains("'Leyline Solo', '127.0.0.1', 8085") && realm_sql().contains("'5875'"));
    }

    #[test]
    fn apply_conf_puts_each_key_in_its_place() {
        let dist = "# head\nA = 1\nB.c = \"x\"\n  Indented = 9\nNot a key\n";
        let out = apply_conf(dist, &[vec![("A".into(), "2".into()), ("New".into(), "3".into())], vec![("A".into(), "4".into())]]);
        assert_eq!(out, "# head\nA = 4\nB.c = \"x\"\n  Indented = 9\nNot a key\nNew = 3\n");
        assert_eq!(overrides("A = 1\n#B = 2\nC.d=\"e\"\n"), vec![("A".to_string(), "1".to_string()), ("C.d".to_string(), "\"e\"".to_string())]);
    }

    #[test]
    fn the_soap_request_is_the_cores_and_answers_are_read() {
        let r = soap_request("SOLOSTOP", "PW", "account create G1 <x>&");
        assert!(r.starts_with("POST / HTTP/1.1\r\nHost: 127.0.0.1:7878\r\n"));
        assert!(r.contains(&format!("Authorization: Basic {}", engine::base64(b"SOLOSTOP:PW"))));
        assert!(r.contains("xmlns:ns1=\"urn:MaNGOS\"") && r.contains("<command>account create G1 &lt;x&gt;&amp;</command>"));
        let body = r.split("\r\n\r\n").nth(1).unwrap();
        assert!(r.contains(&format!("Content-Length: {}\r\n", body.len())));
        assert_eq!(soap_answer("HTTP/1.1 200 OK\r\n\r\n<result>Account created: G1&#xD;</result>").unwrap(), "Account created: G1\r");
        assert!(soap_answer("HTTP/1.1 200 OK\r\n\r\n<faultstring>Command not found</faultstring>").unwrap_err().contains("not found"));
        assert!(soap_answer("HTTP/1.1 401 Unauthorized\r\n\r\n").is_err());
        assert!(soap_answer("").is_err());
    }

    #[test]
    fn the_tools_run_on_the_players_folder_as_phase_0_does() {
        let p = Paths::new(Path::new("/app"));
        let g = Path::new("/games/first");
        assert_eq!(tool_args(&p, g, "ad"), ["-i", "/games/first", "-o", "/app/engine-classic/data"]);
        assert_eq!(tool_args(&p, g, "vmap_extractor"), ["-d", "/games/first/Data", "-o", "/app/engine-classic/data"]);
        assert_eq!(tool_args(&p, g, "vmap_assembler"), ["/app/engine-classic/data/Buildings", "/app/engine-classic/data/vmaps"]);
        let m = tool_args(&p, g, "MoveMapGen");
        assert!(m.contains(&"--buildGameObjects".to_string()) && m.contains(&"/app/engine-classic/data/".to_string()));
    }

    #[test]
    fn a_first_era_folder_is_told_from_a_later_one() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("Data")).unwrap();
        assert!(game_ok(d.path()).is_err(), "no archives");
        for n in ["dbc.MPQ", "terrain.MPQ", "interface.MPQ"] {
            fs::write(d.path().join("Data").join(n), b"x").unwrap();
        }
        assert!(game_ok(d.path()).is_ok());
        fs::write(d.path().join("Data/lichking.MPQ"), b"x").unwrap();
        assert!(game_ok(d.path()).unwrap_err().contains("later"));
    }

    #[test]
    fn the_engines_own_package_is_checked() {
        let d = tempfile::tempdir().unwrap();
        let w = |j: &str| fs::write(d.path().join("solo-engine.json"), j).unwrap();
        w(r#"{"name": "leyline-solo-engine", "version": "2026.11.05-abc", "layout": 1, "build": 5875}"#);
        assert_eq!(read_meta(d.path()).unwrap(), "2026.11.05-abc");
        w(r#"{"name": "leyline-engine", "version": "x", "layout": 1}"#);
        assert!(read_meta(d.path()).is_err(), "the other engine");
        w(r#"{"name": "leyline-solo-engine", "version": "x", "layout": 2}"#);
        assert!(read_meta(d.path()).unwrap_err().contains("newer"));
        w(r#"{"name": "leyline-solo-engine", "version": "x", "layout": 1, "build": 12340}"#);
        assert!(read_meta(d.path()).is_err());
    }

    #[test]
    fn the_characters_query_is_solos_own_and_companions_are_told_apart() {
        let q = chars_sql();
        assert!(q.contains("WHERE a.username = 'SOLO'") && q.contains("ai_playerbot_db_store") && q.contains("`groups`") && q.ends_with("ORDER BY c.guid;"));
        // guid name race class gender level played last online store leader
        let rows = "1\tWren\t1\t1\t0\t12\t3600\t1762400000\t1\t0\t1\t12\n\
                    2\tBrakka\t2\t1\t0\t12\t10\t0\t1\t4\t1\t12\n\
                    3\tIsra\t5\t5\t1\t12\t10\t0\t1\t0\t1\t12\n\
                    4\tOdo\t3\t2\t0\t9\t0\t0\t0\t6\t0\t1\n\
                    5\tMirel\t4\t11\t1\t20\t50\t0\t0\t0\t0\t99999\n\
                    bad\trow\n6\tlower\t1\t1\t0\t1\t0\t0\t0\t0\t0\t1\n";
        let r = parse_rows(rows);
        assert_eq!(r.len(), 5);
        let c = mark_companions(&r);
        let comp: Vec<(&str, bool)> = c.iter().map(|x| (x.base.name.as_str(), x.companion)).collect();
        assert_eq!(comp, [("Wren", false), ("Brakka", true), ("Isra", true), ("Odo", true), ("Mirel", false)],
            "Wren leads the online party (the player); Brakka and Isra are led by her; Odo has the module's saved strategies; Mirel neither");
        assert_eq!(c[0].base.race_label, "Human");
        assert_eq!((c[0].zone.as_str(), c[3].zone.as_str(), c[4].zone.as_str(), c[4].zone_id), ("Elwynn Forest", "Dun Morogh", "", 99999));
        assert!(c[2].base.female && c[0].online && !c[3].online);
        // the player playing a former companion alone: the player, whatever the store says
        let alone = parse_rows("4\tOdo\t3\t2\t0\t9\t0\t0\t1\t6\t0\t1\n");
        assert!(!mark_companions(&alone)[0].companion);
        // stored and read back as the page gets them (flattened)
        let j = serde_json::to_value(&c[1]).unwrap();
        assert_eq!((j["name"].as_str(), j["companion"].as_bool(), j["class_label"].as_str(), j["zone"].as_str(), j["level"].as_u64()),
            (Some("Brakka"), Some(true), Some("Warrior"), Some("Elwynn Forest"), Some(12)), "the page's shape: name, level, race_label, class_label, zone");
        let ch = ClassicChoices { races: lw::choices(true), first_era: true, name: "Wren".into() };
        assert_eq!(ch.races.len(), 8, "the first era's races only");
    }

    #[test]
    fn friends_last_seven_days_and_are_removed_on_stop() {
        let (user, pass) = crate::host::worldshare::new_guest().unwrap();
        assert_eq!(friend_create(&user, &pass).unwrap(), format!("account create {user} {pass}"));
        assert!(friend_create("SOLO", &pass).is_err() && friend_create("SOLOSTOP", &pass).is_err() && friend_create("GA; x", &pass).is_err());
        assert_eq!(friend_delete("GABCDE2345").unwrap(), "account delete GABCDE2345");
        assert!(friend_delete("SOLO").is_err());
        assert!(deleted("Account deleted: GABC") && deleted("Account GABC does not exist") && !deleted("") && !deleted("Syntax"));
        let f = |u: &str, e: u64| Friend { user: u.into(), made: 0, expires: e };
        let list = vec![f("GAAAA1", 100), f("GBBBB2", 1000)];
        let (out, keep) = due(&list, 500, false);
        assert_eq!((out, keep.clone()), (vec![f("GAAAA1", 100)], vec![f("GBBBB2", 1000)]));
        let (out, keep) = due(&list, 500, true);
        assert_eq!((out.len(), keep.len()), (2, 0), "all of them on stop");
        assert_eq!(FRIEND_DAYS * 86_400, 604_800);
        // with no world up, they wait in `gone` for the next start
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path());
        let mut s = st();
        s.friends = list;
        assert_eq!(sweep_friends(&p, &mut s, 500, false), 0);
        assert_eq!((s.friends.len(), s.gone.clone()), (1, vec!["GAAAA1".to_string()]));
        sweep_friends(&p, &mut s, 500, true);
        assert_eq!((s.friends.len(), s.gone.len()), (0, 2));
        assert_eq!(load_state(&p).gone.len(), 2, "kept");
    }

    #[test]
    fn the_share_folder_says_the_first_eras_layout() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path());
        share_files(&p).unwrap();
        assert_eq!(crate::host::litenet::remote_ports(&p.share), Some((LOGIN_PORT, WORLD_PORT)));
        assert_eq!(crate::host::worldshare::layout_in(&p.share), crate::host::worldshare::Layout::Classic);
    }

    #[test]
    fn nothing_starts_or_signs_in_before_it_is_set_up() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path());
        assert!(password(&p).is_err());
        assert!(sign_in(&p, d.path(), Mode::Prefill, None, None).is_err());
        assert!(signin_off(&p).unwrap().contains("Nothing"));
        let st = status(&p);
        assert!(!st.set_up && !st.running && st.account.is_empty() && st.realm_name == REALM_NAME);
        assert_eq!(progress().kind, JOB_KIND);
        assert!(characters(&p).chars.is_empty());
    }
}
