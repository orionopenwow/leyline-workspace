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
//! * it takes one login, the account [`ACCOUNT`], with any password (it only ever listens on this PC).

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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LiteStatus {
    /// The program is on this PC.
    pub available: bool,
    /// It runs and its login port answers.
    pub running: bool,
    pub address: String,
    pub account: String,
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

/// The launch line for `exe` on `den_dir`.
pub fn command_line(den_dir: &Path) -> Vec<String> {
    vec!["--dens".into(), den_dir.display().to_string(), "--bind".into(), ADDRESS.into()]
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
}

impl Binary {
    pub fn find(app_dir: &Path) -> Binary {
        let env = std::env::var("LEYLINE_DEN_LITE").ok();
        let here = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        let exe = candidates(app_dir, env.as_deref(), here.as_deref()).into_iter().find(|p| p.is_file());
        Binary { exe, root: root(app_dir) }
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
        if Self::child_alive() || port_answers(LOGIN_PORT) {
            return Ok(());
        }
        let dir = self.den_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the Den folder: {e}"))?;
        let child = Command::new(exe)
            .args(command_line(&dir))
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
        Ok(())
    }

    fn status(&self) -> LiteStatus {
        let available = self.available();
        LiteStatus {
            available,
            running: available && Self::child_alive() && port_answers(LOGIN_PORT),
            address: ADDRESS.into(),
            account: ACCOUNT.into(),
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
        let b = Binary { exe: None, root: root(app.path()) };
        assert!(!b.available());
        assert!(b.start().unwrap_err().contains("isn't on this PC"));
        let s = b.status();
        assert!(!s.available && !s.running);
        assert_eq!(s.account, ACCOUNT);
        assert_eq!(b.den_dir(), app.path().join("den-lite").join("state").join("dens"), "the same layout as a Host pack");
    }

    #[test]
    fn its_command_line() {
        assert_eq!(command_line(Path::new("/d")), ["--dens", "/d", "--bind", "127.0.0.1"]);
    }
}
