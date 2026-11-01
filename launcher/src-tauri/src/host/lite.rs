//! Den Lite: the tiny built-in Den server (no Docker, no database; `labs/den-lite`, a separate crate still being
//! prototyped). Your Den starts it and plays straight into the white room; "Share with friends" moves the same Den to
//! the full Docker Den (host/yourden.rs `move_to_kit`).
//!
//! This is only the launcher's side of it: the [`DenLite`] interface and a [`Binary`] that looks for the program and
//! runs it. Nothing here links the prototype's code. Until a `den-lite` program is found, `available()` is false and
//! the launcher offers the Docker Den instead.
//!
//! The contract the program is expected to keep (to be confirmed with labs/den-lite before it ships):
//! * `den-lite --dens <dir> --bind 127.0.0.1` listens on 127.0.0.1:3724 (login) and 127.0.0.1:8085 (world);
//! * it reads and writes `<name>.den.json` files ("leyline-den", host/dens.rs) in `<dir>`, and opens the one named by
//!   `<dir>/open.json` as the Docker Den's realm does;
//! * it takes one login, the account [`ACCOUNT`] with the password [`PASSWORD`] (SRP6 needs a password the server
//!   knows before the client proves anything; it only ever listens on this PC), passed as `--password`.

use serde::Serialize;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// The login port Den Lite listens on (the client's default).
pub const LOGIN_PORT: u16 = 3724;
/// What the client's realmlist points at for Den Lite.
pub const ADDRESS: &str = "127.0.0.1";
/// The one account Den Lite takes (written into the client's Config.wtf so the login screen has it filled in).
pub const ACCOUNT: &str = "DEN";
/// Den Lite's local password (labs/den-lite `--password`; shown on the Den page beside the account).
pub const PASSWORD: &str = "den";
/// `<den dir>/den-lite.alive`: "<pid> <unix seconds> <login port>", written by Den Lite every 2 seconds. It tells a
/// Den Lite left running from an earlier launcher (not this launcher's child) from another server on the login port.
pub const ALIVE: &str = "den-lite.alive";
/// `<den dir>/den-lite.stop`: Den Lite stops when this file appears (how a Den Lite that isn't our child is stopped).
pub const STOP: &str = "den-lite.stop";

/// Den Lite's heartbeat in `den_dir` is under 10 seconds old.
pub fn beat_fresh(den_dir: &Path) -> bool {
    let Ok(t) = std::fs::read_to_string(den_dir.join(ALIVE)) else { return false };
    let Some(at) = t.split_whitespace().nth(1).and_then(|a| a.parse::<u64>().ok()) else { return false };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    now.abs_diff(at) < 10
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LiteStatus {
    /// The program is on this PC.
    pub available: bool,
    /// It runs and its login port answers.
    pub running: bool,
    pub address: String,
    pub account: String,
    pub password: String,
    /// Where its Den files live (`<root>/state/dens`, the same layout as a Host pack's).
    pub den_dir: String,
}

/// The launcher's view of Den Lite: start, stop, status and where its Den files are.
pub trait DenLite: Send + Sync {
    fn available(&self) -> bool;
    /// Start it on `den_dir` (nothing happens when it already runs).
    fn start(&self) -> Result<(), String>;
    fn stop(&self) -> Result<(), String>;
    fn status(&self) -> LiteStatus;
    /// The folder of its Den files.
    fn den_dir(&self) -> PathBuf;
}

/// The root of Den Lite's files: `<app dir>/den-lite`, laid out like a Host pack (`state/dens`), so host/dens.rs reads
/// and writes it unchanged.
pub fn root(app_dir: &Path) -> PathBuf {
    app_dir.join("den-lite")
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "den-lite.exe"
    } else {
        "den-lite"
    }
}

/// Where the program may be, in order: `$LEYLINE_DEN_LITE`, `<app dir>/den-lite/bin/`, beside the launcher.
pub fn candidates(app_dir: &Path, env: Option<&str>, launcher_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(e) = env.map(str::trim).filter(|e| !e.is_empty()) {
        v.push(PathBuf::from(e));
    }
    v.push(root(app_dir).join("bin").join(exe_name()));
    if let Some(d) = launcher_dir {
        v.push(d.join(exe_name()));
    }
    v
}

/// The launch line for `exe` on `den_dir`; with the game folder, `.den mods` also lists the launcher's mods in it.
pub fn command_line(den_dir: &Path, client: Option<&Path>) -> Vec<String> {
    let mut v = vec!["--dens".into(), den_dir.display().to_string(), "--bind".into(), ADDRESS.into(), "--password".into(), PASSWORD.into()];
    if let Some(c) = client {
        v.extend(["--client".into(), c.display().to_string()]);
    }
    v
}

fn port_answers(port: u16) -> bool {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

static CHILD: Mutex<Option<Child>> = Mutex::new(None);

/// The program on this PC (found by [`candidates`]), run as a child of the launcher.
pub struct Binary {
    pub exe: Option<PathBuf>,
    pub root: PathBuf,
    /// The game folder it is started for (`--client`), if known.
    pub client: Option<PathBuf>,
}

impl Binary {
    pub fn find(app_dir: &Path) -> Binary {
        let env = std::env::var("LEYLINE_DEN_LITE").ok();
        let here = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        let exe = candidates(app_dir, env.as_deref(), here.as_deref()).into_iter().find(|p| p.is_file());
        Binary { exe, root: root(app_dir), client: None }
    }

    /// Started for this game folder: `.den mods` lists the launcher's mods in it.
    pub fn with_client(mut self, game: &Path) -> Binary {
        self.client = Some(game.to_path_buf());
        self
    }

    fn child_alive() -> bool {
        let Ok(mut g) = CHILD.lock() else { return false };
        match g.as_mut().map(|c| c.try_wait()) {
            Some(Ok(None)) => true,
            Some(_) => {
                *g = None;
                false
            }
            None => false,
        }
    }
}

impl DenLite for Binary {
    fn available(&self) -> bool {
        self.exe.is_some()
    }

    fn start(&self) -> Result<(), String> {
        let exe = self.exe.as_ref().ok_or("Den Lite isn't on this PC yet.")?;
        let dir = self.den_dir();
        if Self::child_alive() || beat_fresh(&dir) {
            return Ok(()); // ours, or one left running from before the launcher was closed
        }
        if port_answers(LOGIN_PORT) {
            return Err(format!("Another server on this PC is using the login port ({LOGIN_PORT}), so your Den can't start. Stop that server, then press Play again."));
        }
        std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the Den folder: {e}"))?;
        let child = Command::new(exe)
            .args(command_line(&dir, self.client.as_deref()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Den Lite didn't start: {e}"))?;
        if let Ok(mut g) = CHILD.lock() {
            *g = Some(child);
        }
        Ok(())
    }

    fn stop(&self) -> Result<(), String> {
        if let Ok(mut g) = CHILD.lock() {
            if let Some(mut c) = g.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        // one left running from an earlier launcher: asked to stop through its folder, then a few seconds to go
        let dir = self.den_dir();
        if beat_fresh(&dir) {
            std::fs::write(dir.join(STOP), b"").map_err(|e| format!("Couldn't stop Den Lite: {e}"))?;
            for _ in 0..30 {
                if !port_answers(LOGIN_PORT) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            let _ = std::fs::remove_file(dir.join(ALIVE));
        }
        Ok(())
    }

    fn status(&self) -> LiteStatus {
        let available = self.available();
        LiteStatus {
            available,
            running: available && (Self::child_alive() || beat_fresh(&self.den_dir())) && port_answers(LOGIN_PORT),
            address: ADDRESS.into(),
            account: ACCOUNT.into(),
            password: PASSWORD.into(),
            den_dir: self.den_dir().display().to_string(),
        }
    }

    fn den_dir(&self) -> PathBuf {
        crate::host::dens::dir(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn found_where_expected_or_not_at_all() {
        let app = tempfile::tempdir().unwrap();
        let c = candidates(app.path(), Some(" /opt/dl "), Some(Path::new("/apps/leyline")));
        assert_eq!(c[0], PathBuf::from("/opt/dl"));
        assert_eq!(c[1], app.path().join("den-lite").join("bin").join(exe_name()));
        assert_eq!(c[2], Path::new("/apps/leyline").join(exe_name()));
        assert_eq!(candidates(app.path(), Some(""), None).len(), 1, "an empty override is no override");
        let b = Binary { exe: None, root: root(app.path()), client: None };
        assert!(!b.available());
        assert!(b.start().unwrap_err().contains("isn't on this PC"));
        let s = b.status();
        assert!(!s.available && !s.running);
        assert_eq!(s.account, ACCOUNT);
        assert_eq!(b.den_dir(), app.path().join("den-lite").join("state").join("dens"), "the same layout as a Host pack");
    }

    #[test]
    fn a_den_lite_from_an_earlier_launcher_is_known_by_its_heartbeat() {
        let app = tempfile::tempdir().unwrap();
        let b = Binary { exe: Some(app.path().join("den-lite")), root: root(app.path()), client: None };
        let dir = b.den_dir();
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!beat_fresh(&dir), "no heartbeat");
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        std::fs::write(dir.join(ALIVE), format!("4242 {now} 3724\n")).unwrap();
        assert!(beat_fresh(&dir));
        assert!(b.start().is_ok(), "it runs already: nothing to start");
        std::fs::write(dir.join(ALIVE), format!("4242 {} 3724\n", now - 60)).unwrap();
        assert!(!beat_fresh(&dir), "a minute old: it stopped (or the PC restarted)");
        std::fs::write(dir.join(ALIVE), "garbage").unwrap();
        assert!(!beat_fresh(&dir));
        // stop: the stop file for the one that isn't ours
        std::fs::write(dir.join(ALIVE), format!("4242 {now} 3724\n")).unwrap();
        b.stop().unwrap();
        assert!(dir.join(STOP).exists() && !dir.join(ALIVE).exists());
    }

    #[test]
    fn its_command_line() {
        assert_eq!(command_line(Path::new("/d"), None), ["--dens", "/d", "--bind", "127.0.0.1", "--password", "den"]);
        assert_eq!(command_line(Path::new("/d"), Some(Path::new("/games/my game")))[6..], ["--client", "/games/my game"], "the game folder for .den mods");
    }
}
