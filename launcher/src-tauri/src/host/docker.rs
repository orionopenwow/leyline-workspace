//! Docker for hosting: is it there and running, and the realm's own compose calls (stop, logs) plus the
//! kit's tools image (a new invite link). Every call is `docker ...` with fixed arguments and the kit folder;
//! nothing from the UI reaches a command line.
//!
//! The `docker` program is looked for beyond PATH (`locate`): a launcher opened before Docker Desktop was installed
//! keeps its old PATH, and on a Mac an app started from the Dock never gets the terminal's PATH. The program found is
//! the one every call runs, with its folder put first on the PATH of what it starts (Docker Desktop's helpers live
//! there). Docker Desktop being on the PC, even while its engine does not answer, is told apart from no Docker at all.

use serde::Serialize;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::HostOs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DockerState {
    /// No `docker` program and no Docker Desktop.
    Missing,
    /// The program is there, the engine is not running (Linux: the service is stopped).
    Stopped,
    /// Docker Desktop is installed but closed (its engine does not answer, or its CLI was not found yet).
    DesktopStopped,
    /// Linux: the engine runs, but this user may not use its socket (not in the docker group).
    NoPermission,
    /// Running, but without `docker compose`.
    NoCompose,
    Ready,
}

#[derive(Debug, Clone, Serialize)]
pub struct DockerInfo {
    pub state: DockerState,
    pub version: Option<String>,
    /// What to do, in plain words, for this OS.
    pub help: String,
    /// The page to get Docker from, for this OS.
    pub get_url: &'static str,
    /// The `docker` program found (PATH or a known install folder).
    pub cli: Option<String>,
    /// Docker Desktop's app, when it is on this PC.
    pub desktop: Option<String>,
    /// The launcher can start it (Docker Desktop on Windows or a Mac): the Start Docker button.
    pub can_start: bool,
}

pub const DESKTOP_URL: &str = "https://docs.docker.com/desktop/";
pub const ENGINE_URL: &str = "https://docs.docker.com/engine/install/";

/// A command's exit code and output (stdout then stderr), or None when it could not start.
pub type Runner<'a> = &'a dyn Fn(&[&str]) -> Option<(i32, String)>;

pub fn help(os: HostOs, state: DockerState) -> (String, &'static str) {
    let url = if os == HostOs::Linux { ENGINE_URL } else { DESKTOP_URL };
    let text = match state {
        DockerState::Missing => match os {
            HostOs::Windows => "Get Docker Desktop: free for personal use. It sets up what it needs.",
            HostOs::Mac => "Get Docker Desktop for Mac (pick Apple chip or Intel): free for personal use.",
            HostOs::Linux => "Install Docker Engine with its compose plugin (docs.docker.com has a guide for your system).",
        },
        DockerState::DesktopStopped => match os {
            HostOs::Linux => "Docker Desktop is installed but closed: open it from your apps.",
            _ => "Docker Desktop is installed but closed.",
        },
        DockerState::Stopped => match os {
            HostOs::Linux => "Docker is installed but not running. In a terminal: sudo systemctl start docker",
            _ => "Docker is installed but not running: open Docker Desktop.",
        },
        DockerState::NoPermission => "Docker runs, but your user can't use it yet. In a terminal: sudo usermod -aG docker $USER, then log out and in.",
        DockerState::NoCompose => match os {
            HostOs::Linux => "This Docker has no compose plugin: install docker-compose-plugin.",
            _ => "This Docker Desktop is too old: update it.",
        },
        DockerState::Ready => "Docker is ready.",
    };
    (text.to_string(), url)
}

/// The state from `docker version` and `docker compose version` alone (no Docker Desktop seen).
pub fn detect(os: HostOs, run: Runner) -> DockerInfo {
    detect_with(os, run, &Located::default())
}

/// The state, knowing what `locate` found: Docker Desktop on the PC turns "no program" or "no engine" into
/// "installed but closed", which the launcher can start.
pub fn detect_with(os: HostOs, run: Runner, found: &Located) -> DockerInfo {
    let desktop = found.desktop.is_some();
    let state;
    let mut version = None;
    match run(&["version", "--format", "{{.Server.Version}}"]) {
        None => state = if desktop { DockerState::DesktopStopped } else { DockerState::Missing },
        Some((0, out)) if !out.trim().is_empty() => {
            version = Some(out.trim().lines().next().unwrap_or("").chars().take(40).collect());
            state = match run(&["compose", "version"]) {
                Some((0, _)) => DockerState::Ready,
                _ => DockerState::NoCompose,
            };
        }
        Some((_, out)) => {
            let o = out.to_ascii_lowercase();
            state = if os == HostOs::Linux && o.contains("permission denied") && o.contains("docker.sock") {
                DockerState::NoPermission
            } else if desktop {
                DockerState::DesktopStopped
            } else {
                DockerState::Stopped
            };
        }
    }
    let (help, get_url) = help(os, state);
    let can_start = state == DockerState::DesktopStopped && os != HostOs::Linux;
    DockerInfo {
        state,
        version,
        help,
        get_url,
        cli: found.cli.as_ref().map(|p| p.display().to_string()),
        desktop: found.desktop.as_ref().map(|p| p.display().to_string()),
        can_start,
    }
}

// ---------------------------------------------------------------- finding Docker beyond PATH

/// What `locate` found: the `docker` program and Docker Desktop's app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Located {
    pub cli: Option<PathBuf>,
    pub desktop: Option<PathBuf>,
}

/// Where to look: this PC's PATH, home and install folders, and how to ask whether a file is there (a fake in the
/// tests).
pub struct Places<'a> {
    pub os: HostOs,
    pub path: Vec<PathBuf>,
    pub home: Option<PathBuf>,
    /// Windows: %LOCALAPPDATA% (the per-user install: Programs\DockerDesktop).
    pub local_app_data: Option<PathBuf>,
    /// Windows: %ProgramFiles% and %ProgramW6432%.
    pub program_files: Vec<PathBuf>,
    /// Windows: Docker Desktop's InstallPath from the registry (HKLM, then HKCU).
    pub registry: Vec<PathBuf>,
    pub exists: &'a dyn Fn(&Path) -> bool,
}

impl Places<'_> {
    /// Docker Desktop's install folders on Windows, the registry's first.
    fn win_installs(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self.registry.clone();
        v.extend(self.program_files.iter().map(|p| p.join("Docker").join("Docker")));
        if let Some(l) = &self.local_app_data {
            v.push(l.join("Programs").join("DockerDesktop"));
        }
        v
    }
}

/// The folders a `docker` program is looked for in, in order, after PATH.
pub fn cli_dirs(p: &Places) -> Vec<PathBuf> {
    let home = |rel: &str| p.home.as_ref().map(|h| h.join(rel));
    match p.os {
        HostOs::Windows => p.win_installs().into_iter().map(|d| d.join("resources").join("bin")).collect(),
        HostOs::Mac => [Some(PathBuf::from("/usr/local/bin")), Some(PathBuf::from("/opt/homebrew/bin")), home(".docker/bin"),
            Some(PathBuf::from("/Applications/Docker.app/Contents/Resources/bin")), home("Applications/Docker.app/Contents/Resources/bin")]
            .into_iter().flatten().collect(),
        HostOs::Linux => [Some(PathBuf::from("/usr/bin")), Some(PathBuf::from("/usr/local/bin")), home(".docker/bin"),
            Some(PathBuf::from("/opt/docker-desktop/bin")), Some(PathBuf::from("/snap/bin"))]
            .into_iter().flatten().collect(),
    }
}

/// Docker Desktop's app, where it installs itself.
pub fn desktop_apps(p: &Places) -> Vec<PathBuf> {
    match p.os {
        HostOs::Windows => p.win_installs().into_iter().map(|d| d.join("Docker Desktop.exe")).collect(),
        HostOs::Mac => [Some(PathBuf::from("/Applications/Docker.app")), p.home.as_ref().map(|h| h.join("Applications/Docker.app"))]
            .into_iter().flatten().collect(),
        HostOs::Linux => vec![PathBuf::from("/opt/docker-desktop/bin/com.docker.backend"), PathBuf::from("/opt/docker-desktop")],
    }
}

pub fn exe_name(os: HostOs) -> &'static str {
    if os == HostOs::Windows { "docker.exe" } else { "docker" }
}

/// Find the `docker` program (PATH first) and Docker Desktop.
pub fn locate(p: &Places) -> Located {
    let exe = exe_name(p.os);
    let cli = p.path.iter().chain(cli_dirs(p).iter()).map(|d| d.join(exe)).find(|f| (p.exists)(f));
    let desktop = desktop_apps(p).into_iter().find(|f| (p.exists)(f));
    let desktop = if p.os == HostOs::Linux { desktop.map(|_| PathBuf::from("/opt/docker-desktop")) } else { desktop };
    Located { cli, desktop }
}

/// Docker Desktop's InstallPath from `reg query` output (`    InstallPath    REG_SZ    C:\Program Files\Docker\Docker`).
pub fn parse_reg_install(out: &str) -> Option<PathBuf> {
    out.lines().find_map(|l| {
        let l = l.trim();
        let rest = l.strip_prefix("InstallPath")?.trim_start();
        let v = rest.strip_prefix("REG_SZ")?.trim();
        (!v.is_empty() && v.len() < 260 && !v.contains('"')).then(|| PathBuf::from(v))
    })
}

/// This PC's places. The registry is read (two fixed `reg query` calls) only on Windows and only when the usual
/// install folders hold no Docker Desktop.
pub fn system_places<'a>(os: HostOs, exists: &'a dyn Fn(&Path) -> bool) -> Places<'a> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let mut p = Places {
        os,
        path: std::env::var_os("PATH").map(|v| std::env::split_paths(&v).collect()).unwrap_or_default(),
        home: dirs::home_dir(),
        local_app_data: var("LOCALAPPDATA"),
        program_files: ["ProgramW6432", "ProgramFiles"].iter().filter_map(|k| var(k)).fold(Vec::new(), |mut v, d| {
            if !v.contains(&d) {
                v.push(d);
            }
            v
        }),
        registry: Vec::new(),
        exists,
    };
    if os == HostOs::Windows && p.program_files.is_empty() {
        p.program_files.push(PathBuf::from(r"C:\Program Files"));
    }
    if os == HostOs::Windows && locate(&p).desktop.is_none() {
        for hive in [r"HKLM\SOFTWARE\Docker Inc.\Docker Desktop", r"HKCU\SOFTWARE\Docker Inc.\Docker Desktop"] {
            if let Some((0, out)) = run_capture("reg", &["query", hive, "/v", "InstallPath"], None, Duration::from_secs(3)) {
                p.registry.extend(parse_reg_install(&out));
            }
        }
    }
    p
}

static FOUND: Mutex<Option<Located>> = Mutex::new(None);

/// Look for Docker on this PC again (every Docker check does), and keep what was found for every later call.
pub fn relocate() -> Located {
    let os = HostOs::current();
    let exists = |f: &Path| f.exists();
    let l = locate(&system_places(os, &exists));
    *FOUND.lock().unwrap_or_else(|e| e.into_inner()) = Some(l.clone());
    l
}

fn found() -> Located {
    let cached = FOUND.lock().unwrap_or_else(|e| e.into_inner()).clone();
    cached.unwrap_or_else(relocate)
}

/// The program every Docker call runs: the one found, else plain `docker` (PATH).
pub fn program() -> PathBuf {
    found().cli.unwrap_or_else(|| PathBuf::from("docker"))
}

/// PATH with the found `docker` program's folder first, for what the launcher starts (the kit's scripts, Docker
/// itself). None when no program was found beyond PATH.
pub fn path_env() -> Option<OsString> {
    let dir = found().cli?.parent()?.to_path_buf();
    with_dir_first(&dir, std::env::var_os("PATH"))
}

pub fn with_dir_first(dir: &Path, path: Option<OsString>) -> Option<OsString> {
    let mut parts: Vec<PathBuf> = vec![dir.to_path_buf()];
    if let Some(p) = path {
        parts.extend(std::env::split_paths(&p).filter(|d| d != dir));
    }
    std::env::join_paths(parts).ok()
}

/// The full Docker check: find it, then ask it.
pub fn check(timeout: Duration) -> DockerInfo {
    let l = relocate();
    detect_with(HostOs::current(), &|a| run_docker(a, None, timeout), &l)
}

// ---------------------------------------------------------------- starting Docker Desktop

/// Start Docker Desktop: `docker desktop start --detach` where its CLI has that (Docker Desktop 4.37 and later),
/// else the app itself (`Docker Desktop.exe`, or `open -a` on a Mac). Linux is only told how: the launcher never
/// runs sudo. Fixed arguments only; the paths are the ones `locate` found. The caller then watches `docker info`.
/// `run` runs the found `docker`; `launch` starts a program and does not wait for it.
pub fn start_with(os: HostOs, found: &Located, run: Runner, launch: &dyn Fn(&Path, &[&str]) -> Result<(), String>) -> Result<String, String> {
    const STARTING: &str = "Starting Docker. This usually takes under a minute.";
    if os == HostOs::Linux {
        return Err(if found.desktop.is_some() {
            "Open Docker Desktop from your apps. This page sees it once it runs.".into()
        } else {
            "In a terminal: sudo systemctl start docker. This page sees it once it runs.".into()
        });
    }
    if found.cli.is_some() && matches!(run(&["desktop", "version"]), Some((0, _))) {
        if let Some((0, _)) = run(&["desktop", "start", "--detach"]) {
            return Ok(STARTING.into());
        }
    }
    let app = found.desktop.as_deref().ok_or("Docker Desktop is not on this PC: get it first.")?;
    match os {
        HostOs::Windows => launch(app, &[])?,
        _ => launch(Path::new("/usr/bin/open"), &["-a", &app.display().to_string()])?,
    }
    Ok(STARTING.into())
}

/// `start_with` on this PC.
pub fn start() -> Result<String, String> {
    let l = relocate();
    let launch = |prog: &Path, args: &[&str]| -> Result<(), String> {
        let mut c = Command::new(prog);
        c.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if prog.ends_with("open") {
            // `open` hands the app to the system and ends at once
            return c.status().map(|_| ()).map_err(|e| format!("Could not open Docker Desktop: {e}"));
        }
        c.spawn().map(|_| ()).map_err(|e| format!("Could not open Docker Desktop: {e}"))
    };
    start_with(HostOs::current(), &l, &|a| run_docker(a, None, Duration::from_secs(30)), &launch)
}

/// Run `docker` with `args` in `cwd`, at most `timeout`; output capped at 256 KB.
pub fn run_docker(args: &[&str], cwd: Option<&Path>, timeout: Duration) -> Option<(i32, String)> {
    let prog = program();
    run_capture_env(prog.as_os_str(), args, cwd, timeout, path_env())
}

/// Run `program` with `args`, at most `timeout` (then it is killed, code -2); None when it cannot start.
pub fn run_capture(program: &str, args: &[&str], cwd: Option<&Path>, timeout: Duration) -> Option<(i32, String)> {
    run_capture_env(std::ffi::OsStr::new(program), args, cwd, timeout, None)
}

/// `run_capture` with its own PATH (the found Docker's folder first).
pub fn run_capture_env(program: &std::ffi::OsStr, args: &[&str], cwd: Option<&Path>, timeout: Duration, path: Option<OsString>) -> Option<(i32, String)> {
    let mut c = Command::new(program);
    c.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(p) = path {
        c.env("PATH", p);
    }
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = c.spawn().ok()?;
    let mut out = child.stdout.take()?;
    let mut err = child.stderr.take()?;
    let t1 = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = (&mut out).take(256 * 1024).read_to_end(&mut b);
        b
    });
    let t2 = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = (&mut err).take(64 * 1024).read_to_end(&mut b);
        b
    });
    let start = Instant::now();
    let code = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s.code().unwrap_or(-1),
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break -2;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break -1,
        }
    };
    let mut text = String::from_utf8_lossy(&t1.join().unwrap_or_default()).to_string();
    text.push_str(&String::from_utf8_lossy(&t2.join().unwrap_or_default()));
    Some((code, text))
}

/// Drop terminal colour codes.
pub fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if c == '\n' || c == '\t' || !c.is_control() {
            out.push(c);
        }
    }
    out
}

/// The compose file for the realm this kit hosts, as the kit's own scripts pick it from `state/host.json`: a
/// profile for another client (host any client) has its own, `profiles/<profile>/docker-compose.<name>.yml`;
/// the 3.3.5a profiles (and any kit without that folder) use `docker-compose.host.yml`.
pub fn compose_file(kit: &Path) -> std::path::PathBuf {
    let default = kit.join("docker-compose.host.yml");
    let profile = std::fs::read_to_string(kit.join("state").join("host.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("profile").and_then(|p| p.as_str()).map(str::to_string));
    let Some(p) = profile.filter(|p| super::profiles::id_ok(p)) else {
        return default;
    };
    let mut found: Vec<std::path::PathBuf> = std::fs::read_dir(kit.join("profiles").join(&p))
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|f| {
                    let n = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    f.is_file() && n.starts_with("docker-compose.") && n.ends_with(".yml")
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found.into_iter().next().unwrap_or(default)
}

/// The compose arguments for the kit (the same as the kit's own scripts use).
pub fn compose_args(kit: &Path, tail: &[&str]) -> Vec<String> {
    let mut a = vec![
        "compose".to_string(),
        "-f".into(),
        compose_file(kit).display().to_string(),
        "--project-directory".into(),
        kit.display().to_string(),
    ];
    a.extend(tail.iter().map(|s| s.to_string()));
    a
}

/// The kit's tools image, as "Realm tools.bat" runs it: `docker run --rm -v <kit>:/kit ... wuhost.setup <cmd>`.
pub fn tools_args(kit: &Path, cmd: &[&str]) -> Vec<String> {
    let mut a = vec![
        "run".to_string(),
        "--rm".into(),
        "-v".into(),
        format!("{}:/kit", kit.display()),
        "-e".into(),
        "WU_KIT=/kit".into(),
        "wu-host-tools:1".into(),
        "python".into(),
        "-m".into(),
        "wuhost.setup".into(),
    ];
    a.extend(cmd.iter().map(|s| s.to_string()));
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_and_help() {
        let missing = detect(HostOs::Windows, &|_| None);
        assert_eq!(missing.state, DockerState::Missing);
        assert!(missing.help.contains("Docker Desktop") && missing.get_url == DESKTOP_URL);
        let stopped = detect(HostOs::Linux, &|a| Some(if a[0] == "version" { (1, "Cannot connect to the Docker daemon".into()) } else { (0, String::new()) }));
        assert_eq!(stopped.state, DockerState::Stopped);
        assert!(stopped.help.contains("systemctl") && stopped.get_url == ENGINE_URL);
        let ready = detect(HostOs::Mac, &|a| Some(if a[0] == "version" { (0, "27.3.1\n".into()) } else { (0, "Docker Compose version v2".into()) }));
        assert_eq!((ready.state, ready.version.as_deref()), (DockerState::Ready, Some("27.3.1")));
        let old = detect(HostOs::Mac, &|a| Some(if a[0] == "version" { (0, "19.0".into()) } else { (1, "unknown command".into()) }));
        assert_eq!(old.state, DockerState::NoCompose);
    }

    /// A path from its parts, as this OS joins them (the tests run on Linux too).
    fn j(parts: &[&str]) -> PathBuf {
        parts.iter().collect()
    }
    fn places(os: HostOs, exists: &dyn Fn(&Path) -> bool) -> Places<'_> {
        Places {
            os,
            path: vec![if os == HostOs::Windows { j(&[r"C:\Windows", "system32"]) } else { PathBuf::from("/usr/sbin") }],
            home: Some(if os == HostOs::Windows { j(&[r"C:\Users", "me"]) } else if os == HostOs::Mac { "/Users/me".into() } else { "/home/me".into() }),
            local_app_data: (os == HostOs::Windows).then(|| j(&[r"C:\Users", "me", "AppData", "Local"])),
            program_files: if os == HostOs::Windows { vec![PathBuf::from(r"C:\Program Files")] } else { vec![] },
            registry: vec![],
            exists,
        }
    }
    /// A fake disk: only these files are there.
    fn disk(files: Vec<PathBuf>) -> impl Fn(&Path) -> bool {
        move |f: &Path| files.iter().any(|x| x == f)
    }

    #[test]
    fn docker_desktop_is_found_beyond_path() {
        let pf = r"C:\Program Files";
        let app = j(&[pf, "Docker", "Docker", "Docker Desktop.exe"]);
        let cli = j(&[pf, "Docker", "Docker", "resources", "bin", "docker.exe"]);
        // Windows, all users: not on PATH, Docker Desktop in Program Files
        let d = disk(vec![app.clone(), cli.clone()]);
        let l = locate(&places(HostOs::Windows, &d));
        assert_eq!((l.cli.as_ref(), l.desktop.as_ref()), (Some(&cli), Some(&app)));
        // the engine does not answer: installed but closed, and the launcher can start it
        let i = detect_with(HostOs::Windows, &|a| Some(if a[0] == "version" { (1, "error during connect: open //./pipe/dockerDesktopLinuxEngine".into()) } else { (0, String::new()) }), &l);
        assert_eq!((i.state, i.can_start), (DockerState::DesktopStopped, true));
        assert!(i.help.contains("installed but closed") && i.cli.unwrap().ends_with("docker.exe"));
        // the program itself could not start, but the app is there: still closed, never "get Docker"
        assert_eq!(detect_with(HostOs::Windows, &|_| None, &l).state, DockerState::DesktopStopped);
        // Windows, one user (Docker Desktop Installer.exe install --user)
        let d = disk(vec![j(&[r"C:\Users", "me", "AppData", "Local", "Programs", "DockerDesktop", "Docker Desktop.exe"])]);
        let l = locate(&places(HostOs::Windows, &d));
        assert!(l.desktop.is_some() && l.cli.is_none());
        assert_eq!(detect_with(HostOs::Windows, &|_| None, &l).state, DockerState::DesktopStopped);
        // the registry's own install folder (installed with --installation-dir)
        let rcli = j(&[r"D:\Apps", "Docker", "resources", "bin", "docker.exe"]);
        let d = disk(vec![j(&[r"D:\Apps", "Docker", "Docker Desktop.exe"]), rcli.clone()]);
        let mut p = places(HostOs::Windows, &d);
        p.registry = vec![j(&[r"D:\Apps", "Docker"])];
        assert_eq!(locate(&p).cli, Some(rcli));
        // PATH first
        let on_path = j(&[r"C:\Windows", "system32", "docker.exe"]);
        let d = disk(vec![on_path.clone(), cli.clone()]);
        assert_eq!(locate(&places(HostOs::Windows, &d)).cli, Some(on_path));
        // nothing at all
        let d = disk(vec![]);
        let l = locate(&places(HostOs::Windows, &d));
        assert_eq!(l, Located::default());
        let i = detect_with(HostOs::Windows, &|_| None, &l);
        assert_eq!((i.state, i.can_start), (DockerState::Missing, false));
    }

    #[test]
    fn a_mac_app_from_the_dock_finds_docker_desktops_cli() {
        // the Dock's PATH has no docker; Docker Desktop's default links are in ~/.docker/bin, the app in /Applications
        let d = disk(["/Users/me/.docker/bin/docker", "/Applications/Docker.app", "/Applications/Docker.app/Contents/Resources/bin/docker"].map(PathBuf::from).to_vec());
        let l = locate(&places(HostOs::Mac, &d));
        assert_eq!(l.cli.as_deref(), Some(Path::new("/Users/me/.docker/bin/docker")));
        assert_eq!(l.desktop.as_deref(), Some(Path::new("/Applications/Docker.app")));
        let i = detect_with(HostOs::Mac, &|a| Some(if a[0] == "version" { (0, "27.3.1".into()) } else { (0, "v2".into()) }), &l);
        assert_eq!((i.state, i.cli.as_deref()), (DockerState::Ready, Some("/Users/me/.docker/bin/docker")));
        // only inside the app
        let d = disk(["/Applications/Docker.app", "/Applications/Docker.app/Contents/Resources/bin/docker"].map(PathBuf::from).to_vec());
        assert!(locate(&places(HostOs::Mac, &d)).cli.unwrap().starts_with("/Applications/Docker.app"));
        // Homebrew
        let d = disk(vec![PathBuf::from("/opt/homebrew/bin/docker")]);
        let l = locate(&places(HostOs::Mac, &d));
        assert_eq!((l.cli.as_deref(), l.desktop.is_none()), (Some(Path::new("/opt/homebrew/bin/docker")), true));
        assert_eq!(detect_with(HostOs::Mac, &|_| Some((1, "Cannot connect to the Docker daemon".into())), &l).state, DockerState::Stopped);
    }

    #[test]
    fn linux_stopped_service_and_no_permission() {
        let d = disk(vec![PathBuf::from("/usr/bin/docker")]);
        let l = locate(&places(HostOs::Linux, &d));
        assert_eq!(l.cli.as_deref(), Some(Path::new("/usr/bin/docker")));
        let i = detect_with(HostOs::Linux, &|_| Some((1, "Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?".into())), &l);
        assert_eq!((i.state, i.can_start), (DockerState::Stopped, false));
        assert!(i.help.contains("systemctl start docker") && !i.help.contains("pkexec"));
        let i = detect_with(HostOs::Linux, &|_| Some((1, "permission denied while trying to connect to the Docker daemon socket at unix:///var/run/docker.sock".into())), &l);
        assert_eq!(i.state, DockerState::NoPermission, "never 'stopped'");
        assert!(i.help.contains("usermod"));
        // ~/.docker/bin (rootless or Desktop's links)
        let d = disk(vec![PathBuf::from("/home/me/.docker/bin/docker")]);
        assert!(locate(&places(HostOs::Linux, &d)).cli.is_some());
    }

    #[test]
    fn the_registry_and_path() {
        let out = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Docker Inc.\\Docker Desktop\r\n    InstallPath    REG_SZ    C:\\Program Files\\Docker\\Docker\r\n\r\n";
        assert_eq!(parse_reg_install(out), Some(PathBuf::from(r"C:\Program Files\Docker\Docker")));
        assert_eq!(parse_reg_install("ERROR: The system was unable to find the specified registry key or value."), None);
        assert_eq!(parse_reg_install("    InstallPath    REG_DWORD    0x1"), None);
        let p = with_dir_first(Path::new("/opt/d"), Some(std::env::join_paths(["/usr/bin", "/opt/d", "/bin"]).unwrap())).unwrap();
        let parts: Vec<PathBuf> = std::env::split_paths(&p).collect();
        assert_eq!(parts, vec![PathBuf::from("/opt/d"), "/usr/bin".into(), "/bin".into()]);
    }

    #[test]
    fn start_docker_desktop() {
        use std::cell::RefCell;
        let app = j(&[r"C:\Program Files", "Docker", "Docker", "Docker Desktop.exe"]);
        let cli = j(&[r"C:\Program Files", "Docker", "Docker", "resources", "bin", "docker.exe"]);
        let found = Located { cli: Some(cli), desktop: Some(app.clone()) };
        let ran: RefCell<Vec<String>> = RefCell::new(vec![]);
        let launched: RefCell<Vec<String>> = RefCell::new(vec![]);
        let launch = |p: &Path, a: &[&str]| { launched.borrow_mut().push(format!("{} {}", p.display(), a.join(" "))); Ok(()) };
        // 4.37 and later: its own start
        let new = |a: &[&str]| { ran.borrow_mut().push(a.join(" ")); Some((0, String::new())) };
        assert!(start_with(HostOs::Windows, &found, &new, &launch).unwrap().contains("Starting Docker"));
        assert_eq!(*ran.borrow(), vec!["desktop version".to_string(), "desktop start --detach".into()]);
        assert!(launched.borrow().is_empty());
        // older: `docker desktop` is an unknown command, so the app itself
        ran.borrow_mut().clear();
        let old = |a: &[&str]| { ran.borrow_mut().push(a.join(" ")); Some((1, "docker: 'desktop' is not a docker command.".into())) };
        start_with(HostOs::Windows, &found, &old, &launch).unwrap();
        assert_eq!(*ran.borrow(), vec!["desktop version".to_string()], "never a start the CLI doesn't have");
        assert_eq!(launched.borrow().last().unwrap().trim(), app.display().to_string());
        // no CLI found yet (per-user install): the app
        let only_app = Located { cli: None, desktop: Some(app.clone()) };
        start_with(HostOs::Windows, &only_app, &|_| panic!("no docker to run"), &launch).unwrap();
        // a Mac: open -a with the app found
        let mac = Located { cli: None, desktop: Some("/Applications/Docker.app".into()) };
        start_with(HostOs::Mac, &mac, &|_| None, &launch).unwrap();
        assert_eq!(launched.borrow().last().unwrap(), "/usr/bin/open -a /Applications/Docker.app");
        // Linux: told, never run (no sudo from the launcher)
        let n = launched.borrow().len();
        let e = start_with(HostOs::Linux, &Located { cli: Some("/usr/bin/docker".into()), desktop: None }, &|_| panic!("nothing runs"), &launch).unwrap_err();
        assert!(e.contains("systemctl start docker"));
        assert_eq!(launched.borrow().len(), n);
        // nothing to start
        assert!(start_with(HostOs::Windows, &Located::default(), &|_| None, &launch).unwrap_err().contains("get it first"));
    }

    #[test]
    fn args_and_output() {
        let k = Path::new("/home/me/hostpack/kit");
        let a = compose_args(k, &["logs", "--tail", "200", "--no-color"]);
        assert_eq!(a[0..2], ["compose", "-f"]);
        assert!(a[2].ends_with("docker-compose.host.yml") && a.last().unwrap() == "--no-color");
        let t = tools_args(k, &["new-invite"]);
        assert_eq!(t[3], "/home/me/hostpack/kit:/kit");
        assert_eq!(t.last().unwrap(), "new-invite");
        assert_eq!(plain("\u{1b}[32mok\u{1b}[0m\r\nnext\u{7}"), "ok\nnext");
    }

    #[test]
    fn a_profile_for_another_client_uses_its_compose_file() {
        let d = std::env::temp_dir().join(format!("wu-compose-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("state")).unwrap();
        std::fs::create_dir_all(d.join("profiles/cmangos-2.4.3")).unwrap();
        std::fs::write(d.join("profiles/cmangos-2.4.3/docker-compose.cmangos-2.4.3.yml"), "services: {}").unwrap();
        assert!(compose_file(&d).ends_with("docker-compose.host.yml"), "no host.json yet: the 3.3.5a default");
        std::fs::write(d.join("state/host.json"), r#"{"profile":"dalaran"}"#).unwrap();
        assert!(compose_file(&d).ends_with("docker-compose.host.yml"));
        std::fs::write(d.join("state/host.json"), r#"{"profile":"cmangos-2.4.3"}"#).unwrap();
        assert!(compose_file(&d).ends_with("profiles/cmangos-2.4.3/docker-compose.cmangos-2.4.3.yml"));
        assert!(compose_args(&d, &["stop"])[2].ends_with("docker-compose.cmangos-2.4.3.yml"));
        std::fs::write(d.join("state/host.json"), r#"{"profile":"../../etc"}"#).unwrap();
        assert!(compose_file(&d).ends_with("docker-compose.host.yml"), "never a path from host.json");
        std::fs::remove_file(d.join("profiles/cmangos-2.4.3/docker-compose.cmangos-2.4.3.yml")).unwrap();
        std::fs::write(d.join("state/host.json"), r#"{"profile":"cmangos-2.4.3"}"#).unwrap();
        assert!(compose_file(&d).ends_with("docker-compose.host.yml"), "an old kit without the folder");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn capture_and_timeout() {
        assert!(run_capture("leyline-no-such-program-xyz", &[], None, Duration::from_secs(1)).is_none());
        let (c, out) = run_capture("sh", &["-c", "echo hi; echo err >&2; exit 3"], None, Duration::from_secs(5)).unwrap();
        assert_eq!((c, out.as_str()), (3, "hi\nerr\n"));
        let (c, _) = run_capture("sh", &["-c", "sleep 5"], None, Duration::from_millis(200)).unwrap();
        assert_eq!(c, -2);
    }
}
