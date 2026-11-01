//! Hosting a realm (a Beacon) on this PC, through the Leyline relay. Leyline does not run a server itself and
//! there are no hosted slots on our servers: the host kit (hosting/kit/) runs the realm in Docker on the
//! player's own PC and its agent talks out to the relay.
//!
//! The launcher's part:
//! * find the host kit folder the player unzipped (`Host a Wrath realm.bat`, or `leyline` and
//!   `host-wrath-realm.sh`), the realm it already hosts (`state/host.json`), and any per-realm seed file the
//!   website gave them (`leyline-<realm id>.bat` / `.sh`, from /wrath/host), in the kit or in Downloads;
//! * copy a seed from Downloads into the kit (that is where it must run);
//! * start the kit's script in a terminal window of its own, so its questions and progress show;
//! * show the realm's status from the relay's public lookup;
//! * stop it, show its logs and its invite link, make a new invite link (`docker`);
//! * the Host pack (`pack`): the kit as a download inside Leyline, installed into app data and updated from
//!   the catalog. A kit the player unzipped themselves keeps working as before;
//! * the host console (`console`): the realm's health, GM controls, Check my realm and the realm tools, once a
//!   realm is set up on this PC; and `leyline link` (a server the player already runs) in a window of its own.

pub mod console;
pub mod dens;
pub mod lite; // Den Lite: the built-in Den server, no Docker (an interface; the program is labs/den-lite)
pub mod terrain; // Your Den's ground: the terrain delta, patch-T.MPQ, the Docker Den's tile (host/terrain.rs)
pub mod forge; // Forge, the Den's terrain editor (GPL, its own program): get it, start it, take what it saves
pub mod yourden; // Your Den: the one Den page, Lite or Docker, Saved Dens, Packs, Share with friends (host/yourden.rs)
pub mod modes; // modes anyone can host and fork: the Host page's first screen (host/modes.rs)
pub mod docker;
pub mod network;
pub mod pack;
pub mod profiles;
pub mod scan;

use crate::realms;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const HOST_PAGE: &str = "https://play.openvanilla.app/wrath/host";
pub const WINDOWS_SCRIPT: &str = "Host a Wrath realm.bat";
pub const UNIX_SCRIPT: &str = "host-wrath-realm.sh";
pub const LEYLINE_SCRIPT: &str = "leyline";
pub const LINK_SCRIPT: &str = "Link my server.bat";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Seed {
    pub file_name: String,
    pub path: PathBuf,
    pub realm_id: String,
    /// `bat` or `sh`.
    pub kind: String,
    /// In the kit folder (ready to run) rather than in Downloads.
    pub in_kit: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HostedRealm {
    pub realm_id: String,
    pub name: String,
    pub visibility: String,
    pub relay: String,
    pub profile: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct KitInfo {
    /// The folder the player chose (empty when none yet).
    pub dir: String,
    pub is_kit: bool,
    pub has_windows_script: bool,
    pub has_unix_script: bool,
    pub seeds: Vec<Seed>,
    /// The realm this kit hosts already (it has run once).
    pub realm: Option<HostedRealm>,
    /// What looks wrong, in plain words.
    pub problems: Vec<String>,
}

/// `leyline-<realm id>.bat` or `.sh`, as the website names the per-realm download.
pub fn seed_name(file_name: &str) -> Option<(String, String)> {
    let rest = file_name.strip_prefix("leyline-")?;
    let (rid, kind) = rest.rsplit_once('.')?;
    (realms::realm_id_ok(rid) && matches!(kind, "bat" | "sh")).then(|| (rid.to_string(), kind.to_string()))
}

fn seeds_in(dir: &Path, in_kit: bool) -> Vec<Seed> {
    let mut v: Vec<Seed> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_file())
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    seed_name(&n).map(|(realm_id, kind)| Seed { file_name: n, path: e.path(), realm_id, kind, in_kit })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    v
}

fn hosted(kit: &Path) -> Option<HostedRealm> {
    let v: Value = serde_json::from_str(&fs::read_to_string(kit.join("state").join("host.json")).ok()?).ok()?;
    let g = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").chars().take(64).collect::<String>();
    let rid = g("realm_id");
    if !realms::realm_id_ok(&rid) {
        return None;
    }
    let relay = Some(g("relay")).filter(|r| realms::relay_ok(r)).unwrap_or_else(|| realms::RELAY.into());
    Some(HostedRealm { realm_id: rid, name: g("name"), visibility: g("visibility"), relay, profile: g("profile") })
}

/// Look at the chosen kit folder (and Downloads, for seed files not moved into the kit yet).
pub fn inspect(kit: Option<&Path>, downloads: Option<&Path>) -> KitInfo {
    let mut info = KitInfo::default();
    if let Some(k) = kit {
        info.dir = k.display().to_string();
        info.has_windows_script = k.join(WINDOWS_SCRIPT).is_file();
        info.has_unix_script = k.join(LEYLINE_SCRIPT).is_file() && k.join(UNIX_SCRIPT).is_file();
        info.is_kit = info.has_windows_script || info.has_unix_script;
        if !k.is_dir() {
            info.problems.push("That folder is not there any more.".into());
        } else if !info.is_kit {
            info.problems.push(format!(
                "This folder is not a host kit: it has no \"{WINDOWS_SCRIPT}\" or `leyline` script. Pick the folder you unzipped the kit into."
            ));
        } else if !k.join("realm-files").is_dir() {
            info.problems.push("This kit is missing its realm-files folder: download the full kit again.".into());
        }
        info.seeds = seeds_in(k, true);
        info.realm = hosted(k);
    }
    if let Some(d) = downloads {
        if kit.map(|k| k != d).unwrap_or(true) {
            for s in seeds_in(d, false) {
                if !info.seeds.iter().any(|x| x.file_name == s.file_name) {
                    info.seeds.push(s);
                }
            }
        }
    }
    info
}

/// Copy a seed file into the kit (where it has to run). Only a file named like a seed is copied.
pub fn copy_seed_into_kit(seed: &Path, kit: &Path) -> Result<PathBuf, String> {
    let name = seed.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    seed_name(&name).ok_or("That is not a realm file from the website (leyline-<realm id>.bat or .sh).")?;
    let md = fs::metadata(seed).map_err(|e| e.to_string())?;
    if !md.is_file() || md.len() > 64 * 1024 {
        return Err("That realm file does not look right.".into());
    }
    let dest = kit.join(&name);
    fs::copy(seed, &dest).map_err(|e| format!("Could not copy it into the kit: {e}"))?;
    Ok(dest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    Windows,
    Mac,
    Linux,
}

impl HostOs {
    pub fn current() -> HostOs {
        if cfg!(windows) {
            HostOs::Windows
        } else if cfg!(target_os = "macos") {
            HostOs::Mac
        } else {
            HostOs::Linux
        }
    }
}

/// How to start the kit. Built separately from running it so it can be tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPlan {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Windows: open in a console window of its own.
    pub new_console: bool,
    /// No terminal found (Linux): output goes to this file in the kit's state folder.
    pub log: Option<PathBuf>,
    /// What the UI says once it started.
    pub note: String,
    /// The Host page's choice for the kit's script (`profiles::env_for`): the realm profile and the client folder.
    /// Windows passes it to the console; Mac and Linux put it on the command line (`env K=V ...`), because a
    /// terminal window does not always hand the launcher's environment on.
    pub env: Vec<(String, String)>,
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Start the kit: a seed (first run of a realm made on the website) or the kit's own host script.
/// `seed` is a file name in the kit folder. `has` says whether a program is on PATH (Linux terminals).
pub fn plan(kit: &Path, seed: Option<&str>, os: HostOs, has: impl Fn(&str) -> bool) -> Result<HostPlan, String> {
    let info = inspect(Some(kit), None);
    if let Some(s) = seed {
        let (_, kind) = seed_name(s).ok_or("That is not a realm file from the website.")?;
        if !kit.join(s).is_file() {
            return Err("That realm file is not in the kit folder yet.".into());
        }
        let want = if os == HostOs::Windows { "bat" } else { "sh" };
        if kind != want {
            return Err(format!("This is the {} file: on this PC use the .{want} one from the same page.", if kind == "bat" { "Windows" } else { "Mac / Linux" }));
        }
    }
    let cwd = kit.to_path_buf();
    let env = profiles::env_for(kit);
    match os {
        HostOs::Windows => {
            if !info.has_windows_script {
                return Err(format!("\"{WINDOWS_SCRIPT}\" is not in this folder: pick your host kit folder."));
            }
            let script = seed.unwrap_or(WINDOWS_SCRIPT).to_string();
            Ok(HostPlan {
                program: "cmd.exe".into(),
                args: vec!["/K".into(), script],
                cwd,
                new_console: true,
                log: None,
                env,
                note: "Started in its own window. Leave it open: the first start imports the databases and can take 5 to 20 minutes.".into(),
            })
        }
        HostOs::Mac | HostOs::Linux => {
            if !info.has_unix_script {
                return Err("The `leyline` script is not in this folder: pick your host kit folder.".into());
            }
            let mut cmd: Vec<String> = match seed {
                Some(s) => vec!["bash".into(), s.into()],
                None => vec!["bash".into(), LEYLINE_SCRIPT.into(), "host".into()],
            };
            // Vanilla+ is made to stay up: Docker (and so the realm) comes back after a reboot (docs/leyline/always-on.md).
            let chosen = if seed.is_none() { profiles::read_choice(kit).map(|c| c.profile) } else { None };
            if chosen.as_deref() == Some("vanillaplus") {
                cmd.push("--always-on".into());
            }
            // A Den and Solo leveling are private (docs/DEN.md: leyline host --ruleset den --private; the ruleset comes in the env).
            if chosen.as_deref().is_some_and(profiles::private_profile) {
                cmd.push("--private".into());
            }
            if !env.is_empty() {
                let mut e: Vec<String> = vec!["env".into()];
                e.extend(env.iter().map(|(k, v)| format!("{k}={v}")));
                e.extend(cmd);
                cmd = e;
            }
            let note = "Started in a terminal window. The first start imports the databases and can take 5 to 20 minutes.".to_string();
            Ok(in_terminal(kit, os, has, cmd, note))
        }
    }
}

/// `leyline link` (a server the player already runs, on the Leyline list; only the relay agent runs here) in a window
/// of its own: it asks its four questions there.
pub fn plan_link(kit: &Path, os: HostOs, has: impl Fn(&str) -> bool) -> Result<HostPlan, String> {
    let note = "Started in its own window: answer its four questions. Your server is on the realm list within a minute.".to_string();
    if os == HostOs::Windows {
        if !kit.join(LINK_SCRIPT).is_file() {
            return Err(format!("\"{LINK_SCRIPT}\" is not in this folder: update the Host pack."));
        }
        return Ok(HostPlan { program: "cmd.exe".into(), args: vec!["/K".into(), LINK_SCRIPT.into()], cwd: kit.to_path_buf(),
            new_console: true, log: None, env: Vec::new(), note });
    }
    if !kit.join(LEYLINE_SCRIPT).is_file() {
        return Err("The `leyline` script is not in this folder: pick your host kit folder.".into());
    }
    let cmd = vec!["bash".to_string(), LEYLINE_SCRIPT.into(), "link".into()];
    let p = in_terminal(kit, os, has, cmd, note);
    if p.log.is_some() {
        return Err("Linking asks four questions, and no terminal window was found: run `bash leyline link` in the kit folder.".into());
    }
    Ok(p)
}

/// A Mac or Linux command in a terminal window (Terminal on a Mac; the first terminal found on Linux, else in the
/// background with its output in the kit's state folder).
fn in_terminal(kit: &Path, os: HostOs, has: impl Fn(&str) -> bool, cmd: Vec<String>, note: String) -> HostPlan {
    let cwd = kit.to_path_buf();
    if os == HostOs::Mac {
        let line = format!("cd {} && {}", sh_quote(&kit.display().to_string()), cmd.iter().map(|c| sh_quote(c)).collect::<Vec<_>>().join(" "));
        let esc = line.replace('\\', "\\\\").replace('"', "\\\"");
        return HostPlan {
            program: "osascript".into(),
            args: vec!["-e".into(), format!("tell application \"Terminal\" to do script \"{esc}\""), "-e".into(), "tell application \"Terminal\" to activate".into()],
            cwd,
            new_console: false,
            log: None,
            env: Vec::new(),
            note,
        };
    }
    // Keep the window open at the end so the last lines can be read.
    let mut wrapped: Vec<String> =
        vec!["bash".into(), "-c".into(), "\"$@\"; echo; read -r -p 'Press Enter to close this window. ' _".into(), "leyline".into()];
    wrapped.extend(cmd.iter().cloned());
    let terminals: &[(&str, &[&str])] = &[
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["-x"]),
        ("kitty", &[]),
        ("alacritty", &["-e"]),
        ("xterm", &["-e"]),
    ];
    for (t, pre) in terminals {
        if has(t) {
            let mut args: Vec<String> = pre.iter().map(|s| s.to_string()).collect();
            args.extend(wrapped.iter().cloned());
            return HostPlan { program: t.to_string(), args, cwd, new_console: false, log: None, env: Vec::new(), note };
        }
    }
    let log = kit.join("state").join("leyline-launcher.log");
    HostPlan {
        program: cmd[0].clone(),
        args: cmd[1..].to_vec(),
        cwd,
        new_console: false,
        env: Vec::new(),
        note: format!("No terminal window found, so it runs in the background; its output is in {}.", log.display()),
        log: Some(log),
    }
}

/// A program on PATH.
pub fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

pub fn run(p: &HostPlan) -> Result<(), String> {
    spawn(p).map(|_| ())
}

fn spawn(p: &HostPlan) -> Result<std::process::Child, String> {
    let mut c = Command::new(&p.program);
    c.args(&p.args).current_dir(&p.cwd).envs(p.env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    // the kit's scripts call `docker` themselves: the one the launcher found, even when it is not on PATH
    if let Some(path) = docker::path_env() {
        c.env("PATH", path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if p.new_console {
            c.creation_flags(CREATE_NEW_CONSOLE);
        } else if p.log.is_some() {
            c.creation_flags(CREATE_NO_WINDOW);
        }
    }
    if let Some(log) = &p.log {
        if let Some(d) = log.parent() {
            fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let f = fs::File::create(log).map_err(|e| e.to_string())?;
        let f2 = f.try_clone().map_err(|e| e.to_string())?;
        c.stdin(Stdio::null()).stdout(f).stderr(f2).env("LEYLINE_NONINTERACTIVE", "1");
    }
    c.spawn().map_err(|e| format!("Could not start the host kit: {e}"))
}

// ---------------------------------------------------------------- starting with no window (the launcher shows the steps)

/// The answers the Host page's form gives the kit, so its script asks nothing: the realm's name, public or private,
/// and (for `leyline host` on a Mac or Linux) the account name the host plays as and makes GM.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Answers {
    pub name: Option<String>,
    pub visibility: Option<String>,
    pub gm: Option<String>,
}

const BRANDS: &[&str] = &["world of warcraft", "warcraft", "lich king", "lichking", "blizzard"];

/// A realm name the kit takes (setup.py NAME_RE: 1-32 letters, digits, spaces, _ ' or -) and no game or publisher name.
pub fn name_ok(n: &str) -> bool {
    let t = n.trim();
    let low = t.to_ascii_lowercase();
    (1..=32).contains(&t.chars().count())
        && t.chars().all(|c| c.is_ascii_alphanumeric() || " _'-".contains(c))
        && !BRANDS.iter().any(|b| low.contains(b))
        && !low.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "wow")
}

/// An account name for GM (setup.py GM_RE: 3-16 letters or digits).
pub fn gm_ok(n: &str) -> bool {
    (3..=16).contains(&n.len()) && n.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The env for the answers (the kit's `WU_*` and `leyline`'s `LEYLINE_*` names), each checked first.
fn answer_env(a: &Answers) -> Result<Vec<(String, String)>, String> {
    let mut env = Vec::new();
    if let Some(n) = a.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        if !name_ok(n) {
            return Err("Realm name: up to 32 letters, digits, spaces, _ ' or -, and no game names.".into());
        }
        env.extend([("WU_NAME".to_string(), n.to_string()), ("LEYLINE_NAME".to_string(), n.to_string())]);
    }
    if let Some(v) = a.visibility.as_deref() {
        if v != "public" && v != "private" {
            return Err("Pick public or private.".into());
        }
        env.extend([("WU_VISIBILITY".to_string(), v.to_string()), ("LEYLINE_VISIBILITY".to_string(), v.to_string())]);
    }
    if let Some(g) = a.gm.as_deref().map(str::trim).filter(|g| !g.is_empty()) {
        if !gm_ok(g) {
            return Err("Your account name: 3 to 16 letters or digits.".into());
        }
        env.extend([("WU_GM_NAME".to_string(), g.to_string()), ("LEYLINE_GM".to_string(), g.to_string())]);
    }
    Ok(env)
}

/// The quiet run's output, in the kit's state folder.
pub fn job_log(kit: &Path) -> PathBuf {
    kit.join("state").join("leyline-launcher.log")
}

/// Start the kit's host script with no window: its answers from the Host page, its output to the log the launcher
/// reads for the steps (`progress`). A realm file from the website (a seed) still opens its own window (`plan`).
pub fn plan_quiet(kit: &Path, os: HostOs, a: &Answers) -> Result<HostPlan, String> {
    let info = inspect(Some(kit), None);
    let mine = answer_env(a)?;
    // the form's answers win over the saved choice's (a Den's name and "private")
    let mut env: Vec<(String, String)> = profiles::env_for(kit).into_iter().filter(|(k, _)| !mine.iter().any(|(m, _)| m == k)).collect();
    env.extend(mine);
    env.push(("LEYLINE_NONINTERACTIVE".into(), "1".into()));
    let note = "Setting up your realm. The first start takes 5 to 20 minutes.".to_string();
    let log = Some(job_log(kit));
    match os {
        HostOs::Windows => {
            if !info.has_windows_script {
                return Err(format!("\"{WINDOWS_SCRIPT}\" is not in this folder: pick your host kit folder."));
            }
            Ok(HostPlan { program: "cmd.exe".into(), args: vec!["/C".into(), WINDOWS_SCRIPT.into()], cwd: kit.to_path_buf(),
                new_console: false, log, env, note })
        }
        HostOs::Mac | HostOs::Linux => {
            if !info.has_unix_script {
                return Err("The `leyline` script is not in this folder: pick your host kit folder.".into());
            }
            if !kit.join("state").join("host.json").is_file() && !env.iter().any(|(k, _)| k == "LEYLINE_GM") {
                return Err("Type the account name you'll play as: it becomes the realm's GM.".into());
            }
            let mut args = vec![LEYLINE_SCRIPT.to_string(), "host".into()];
            if profiles::read_choice(kit).map(|c| c.profile).as_deref() == Some("vanillaplus") {
                args.push("--always-on".into());
            }
            Ok(HostPlan { program: "bash".into(), args, cwd: kit.to_path_buf(), new_console: false, log, env, note })
        }
    }
}

/// What the Host page's "Put it on Leyline" gives `leyline link`: the server's name, where its login and world
/// servers answer on this PC, and the client it is for. Every value is checked here and again by setup.py.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct LinkAnswers {
    pub name: String,
    pub auth: String,
    pub world: String,
    /// vanilla, tbc or wrath.
    pub client: String,
    pub core: Option<String>,
}

fn addr_ok(a: &str) -> bool {
    let Some((h, p)) = a.rsplit_once(':') else { return false };
    p.parse::<u16>().is_ok_and(|p| p > 0) && !h.is_empty() && h.len() <= 253
        && h.chars().all(|c| c.is_ascii_alphanumeric() || ".-".contains(c))
}

/// `leyline link` (or "Link my server.bat") with no window and no questions.
pub fn plan_link_quiet(kit: &Path, os: HostOs, a: &LinkAnswers) -> Result<HostPlan, String> {
    if !name_ok(&a.name) {
        return Err("Name: up to 32 letters, digits, spaces, _ ' or -, and no game names.".into());
    }
    if !addr_ok(&a.auth) || !addr_ok(&a.world) {
        return Err("That server address does not look right.".into());
    }
    if !matches!(a.client.as_str(), "vanilla" | "tbc" | "wrath") {
        return Err("Pick the client it's for.".into());
    }
    if hosted(kit).is_some() && !fs::read_to_string(kit.join("state").join("host.json")).unwrap_or_default().contains("\"linked\": true") {
        return Err("This Host pack runs a realm already, and a Host pack holds one. Your server can't be linked from it.".into());
    }
    let mut env = vec![
        ("LEYLINE_LINK_NAME".to_string(), a.name.trim().to_string()),
        ("LEYLINE_LINK_AUTH".into(), a.auth.clone()),
        ("LEYLINE_LINK_WORLD".into(), a.world.clone()),
        ("LEYLINE_LINK_CLIENT".into(), a.client.clone()),
        ("LEYLINE_NONINTERACTIVE".into(), "1".into()),
    ];
    if let Some(c) = a.core.as_deref().filter(|c| matches!(*c, "azerothcore" | "trinitycore" | "cmangos" | "vmangos" | "other")) {
        env.push(("LEYLINE_LINK_CORE".into(), c.to_string()));
    }
    let note = "Linking your server. It shows on the realm list within a minute.".to_string();
    let log = Some(job_log(kit));
    if os == HostOs::Windows {
        if !kit.join(LINK_SCRIPT).is_file() {
            return Err(format!("\"{LINK_SCRIPT}\" is not in this folder: update the Host pack."));
        }
        return Ok(HostPlan { program: "cmd.exe".into(), args: vec!["/C".into(), LINK_SCRIPT.into()], cwd: kit.to_path_buf(),
            new_console: false, log, env, note });
    }
    if !kit.join(LEYLINE_SCRIPT).is_file() {
        return Err("The `leyline` script is not in this folder: pick your host kit folder.".into());
    }
    Ok(HostPlan { program: "bash".into(), args: vec![LEYLINE_SCRIPT.into(), "link".into()], cwd: kit.to_path_buf(), new_console: false, log, env, note })
}

/// The one quiet run at a time: what it is, its log and when it started.
struct Job {
    child: std::process::Child,
    kind: &'static str,
    log: PathBuf,
    started: std::time::Instant,
    code: Option<i32>,
}

static JOB: std::sync::Mutex<Option<Job>> = std::sync::Mutex::new(None);

/// Start a quiet plan (`plan_quiet`, `plan_link_quiet`) and keep it for `progress`. One at a time.
pub fn run_job(p: &HostPlan, kind: &'static str) -> Result<String, String> {
    let mut j = JOB.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(job) = j.as_mut() {
        if job.child.try_wait().ok().flatten().is_none() {
            return Err("Your realm is being set up already: wait for it to finish.".into());
        }
    }
    let log = p.log.clone().ok_or("Not a quiet start.")?;
    let child = spawn(p)?;
    *j = Some(Job { child, kind, log, started: std::time::Instant::now(), code: None });
    Ok(p.note.clone())
}

/// A quiet run, as the Host page shows it: the step it is at, in plain words, and how it ended.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Progress {
    /// No quiet run since the launcher started.
    pub idle: bool,
    /// host or link.
    pub kind: String,
    pub running: bool,
    /// The exit code once it ended (0: done).
    pub code: Option<i32>,
    /// The step it is at, in plain words.
    pub step: String,
    /// 1-based step number and how many there are, for the bar.
    pub n: u32,
    pub of: u32,
    pub secs: u64,
    /// Why it stopped, in plain words (only when it failed).
    pub problem: Option<String>,
}

/// The kit's steps (its "== ..." lines), in the order they come, and the words the Host page shows for each.
const STEPS: &[(&[&str], &str)] = &[
    (&["helper image"], "Getting the helper (about a minute)"),
    (&["your realm", "your beacon", "link a server"], "Saving your settings"),
    (&["realm images", "server: building"], "Downloading the realm (the biggest step)"),
    (&["game data", "server data"], "Reading the game data from your game folder"),
    (&["starting the realm", "starting the ", "starting the helper", "starting the agent"], "Starting the servers (the first time: 5 to 20 minutes)"),
    (&["world changes"], "Setting up the ruleset"),
    (&["connecting to the relay"], "Connecting to the network"),
    (&["make your gm account", "done"], "Done"),
];
const LINK_STEPS: &[(&[&str], &str)] = &[
    (&["helper image"], "Getting the helper (about a minute)"),
    (&["link a server"], "Saving your server's details"),
    (&["starting the helper", "starting the agent"], "Connecting your server to the network"),
    (&["done"], "Done"),
];

/// A line safe to show: no passwords, keys, tokens or links with a secret, no game or publisher names and no client
/// versions (the Host page's own words take their place).
pub fn safe_line(l: &str) -> Option<String> {
    let t = docker::plain(l).trim().trim_start_matches("Problem:").trim().to_string();
    let low = t.to_ascii_lowercase();
    let secret = ["password", "secret", "token", "key", "account:", "host-link", "login", "#"].iter().any(|w| low.contains(w));
    let brand = BRANDS.iter().any(|b| low.contains(b)) || low.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "wow");
    let version = t.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.')).any(|w| {
        let parts: Vec<&str> = w.split('.').collect();
        parts.len() >= 3 && parts[0].chars().all(|c| c.is_ascii_digit()) && !parts[0].is_empty()
    });
    (!t.is_empty() && t.len() <= 200 && !secret && !brand && !version).then_some(t)
}

/// Read the steps out of the run's output.
pub fn read_progress(text: &str, kind: &str, running: bool, code: Option<i32>, secs: u64) -> Progress {
    let table = if kind == "link" { LINK_STEPS } else { STEPS };
    let mut at = 0usize;
    for l in text.lines() {
        let l = docker::plain(l);
        let Some(h) = l.trim().strip_prefix("== ") else { continue };
        let h = h.to_ascii_lowercase();
        if let Some(i) = table.iter().position(|(keys, _)| keys.iter().any(|k| h.starts_with(k))) {
            at = at.max(i + 1);
        }
    }
    let step = if at == 0 { "Getting ready".to_string() } else { table[at - 1].1.to_string() };
    let failed = !running && code.is_some_and(|c| c != 0);
    let problem = failed.then(|| {
        let last = text.lines().rev().filter_map(safe_line).find(|l| !l.starts_with("==") && !l.to_ascii_lowercase().starts_with("press "));
        let base = match (kind, at) {
            ("link", _) => "Linking stopped.",
            (_, 0..=2) => "Setting up stopped before the realm was downloaded.",
            (_, 3) => "Downloading the realm stopped. Is this PC online?",
            (_, 4) => "Your game folder was not found or could not be read. Add it on the Clients page, then try again.",
            _ => "Your realm did not start.",
        };
        match last {
            Some(l) => format!("{base} It said: {l}"),
            None => base.to_string(),
        }
    });
    Progress { idle: false, kind: kind.into(), running, code, step, n: at as u32, of: table.len() as u32, secs, problem }
}

/// The quiet run now.
pub fn progress() -> Progress {
    let mut j = JOB.lock().unwrap_or_else(|e| e.into_inner());
    let Some(job) = j.as_mut() else { return Progress { idle: true, ..Default::default() } };
    if job.code.is_none() {
        if let Ok(Some(st)) = job.child.try_wait() {
            job.code = Some(st.code().unwrap_or(-1));
        }
    }
    let text = fs::read(&job.log).map(|b| {
        let tail = &b[b.len().saturating_sub(64 * 1024)..];
        String::from_utf8_lossy(tail).to_string()
    }).unwrap_or_default();
    read_progress(&text, job.kind, job.code.is_none(), job.code, job.started.elapsed().as_secs())
}

/// The hosted realm as the relay sees it, plus its live counts.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HostStatus {
    pub realm_id: String,
    pub online: bool,
    pub name: String,
    pub visibility: Option<String>,
    pub address: Option<String>,
    pub players: Option<u64>,
    pub bots: Option<u64>,
    /// On the public realm list now.
    pub listed: bool,
}

/// Ask the relay (`base` = `https://<relay>`) about the realm, and read its status file when it is up.
pub fn status(base: &str, rid: &str, listed: bool) -> Result<HostStatus, String> {
    let r = realms::lookup(base, rid).map_err(|e| {
        if e.contains("unknown realm") {
            "The relay has not seen this realm yet: start it, and it shows up within a minute.".to_string()
        } else {
            format!("Could not reach the relay: {e}")
        }
    })?;
    let mut out = HostStatus { realm_id: rid.into(), online: r.online, name: r.name, visibility: r.visibility, address: r.address, listed, ..Default::default() };
    if r.online {
        if let Ok(v) = crate::net::get_json(&format!("{base}/r/{rid}/status.json"), realms::LOOKUP_TIMEOUT) {
            out.players = v.get("players").and_then(Value::as_u64);
            out.bots = v.get("bots").and_then(Value::as_u64);
        }
    }
    Ok(out)
}

/// The realm's invite link, as the kit last wrote it (`state/invite.txt`): `https://<relay or site>/join/<id>`
/// plus `#<secret>` for a private realm. Only a link of that shape is handed to the UI.
pub fn invite(kit: &Path) -> Option<String> {
    let t = fs::read_to_string(kit.join("state").join("invite.txt")).ok()?;
    let link = t.lines().next()?.trim().to_string();
    let rest = link.strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    let (path, frag) = match path.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (path, None),
    };
    let rid = path.strip_prefix("join/")?.trim_end_matches('/');
    let frag_ok = frag.map_or(true, |f| (16..=40).contains(&f.len()) && f.chars().all(|c| c.is_ascii_uppercase() || ('2'..='9').contains(&c)));
    (realms::relay_ok(host) && realms::realm_id_ok(rid) && frag_ok && link.len() <= 200).then_some(link)
}

const DOCKER_SHORT: std::time::Duration = std::time::Duration::from_secs(20);
const DOCKER_LONG: std::time::Duration = std::time::Duration::from_secs(180);

fn need_kit(kit: &Path) -> Result<(), String> {
    if !kit.join("docker-compose.host.yml").is_file() {
        return Err("This folder has no docker-compose.host.yml: pick your host kit, or get the Host pack.".into());
    }
    Ok(())
}

fn docker_in(kit: &Path, args: Vec<String>, timeout: std::time::Duration) -> Result<String, String> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match docker::run_docker(&a, Some(kit), timeout) {
        None => Err("Docker is not installed (or not on PATH).".into()),
        Some((0, out)) => Ok(docker::plain(&out)),
        Some((-2, _)) => Err("Docker took too long to answer.".into()),
        Some((_, out)) => Err(format!("Docker said: {}", docker::plain(&out).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("an error").trim())),
    }
}

/// Stop the realm (`docker compose stop`): characters, accounts and the invite link are kept.
pub fn stop(kit: &Path) -> Result<String, String> {
    need_kit(kit)?;
    docker_in(kit, docker::compose_args(kit, &["stop"]), DOCKER_LONG)?;
    Ok("Stopped. Characters, accounts and your invite link are kept; start it again any time.".into())
}

/// The last lines of every service's log.
pub fn logs(kit: &Path, lines: u32) -> Result<String, String> {
    need_kit(kit)?;
    let n = lines.clamp(20, 1000).to_string();
    docker_in(kit, docker::compose_args(kit, &["logs", "--tail", &n, "--no-color", "--timestamps"]), DOCKER_SHORT)
}

/// A new invite link (the old one stops letting anyone new in). Needs the realm set up once.
pub fn new_invite(kit: &Path) -> Result<String, String> {
    need_kit(kit)?;
    if !kit.join("state").join("host.json").is_file() {
        return Err("Start your realm once first: the invite link is made on the first start.".into());
    }
    docker_in(kit, docker::tools_args(kit, &["new-invite"]), DOCKER_SHORT)?;
    invite(kit).ok_or_else(|| "The new invite link was not written: try \"Realm tools\" in the kit.".into())
}

/// "Help the network: relay for others" (docs/LEYLINE-MESH.md): the kit's community relay compose file, with the
/// settings `setup.py volunteer-relay` writes. Runs only when both the site offers it and this PC turned it on
/// (`p2p::flags::volunteer_allowed`). `public_host` is this PC's public DNS name (the relay needs TLS).
pub const VOLUNTEER_COMPOSE: &str = "docker-compose.volunteer-relay.yml";

pub fn volunteer_args(kit: &Path, on: bool) -> Vec<String> {
    let mut a = vec!["compose".to_string(), "-f".into(), kit.join(VOLUNTEER_COMPOSE).display().to_string(),
        "--project-directory".into(), kit.display().to_string()];
    a.extend(if on { vec!["up".to_string(), "-d".into(), "--build".into()] } else { vec!["down".to_string()] });
    a
}

pub fn volunteer_relay(kit: &Path, flags: &crate::p2p::flags::Flags, local: &crate::p2p::flags::Local, on: bool,
                       public_host: Option<&str>, region: Option<&str>) -> Result<String, String> {
    if !kit.join(VOLUNTEER_COMPOSE).is_file() {
        return Err("This host kit has no community relay yet: update the Host pack.".into());
    }
    if !on {
        docker_in(kit, volunteer_args(kit, false), DOCKER_LONG)?;
        let _ = docker_in(kit, docker::tools_args(kit, &["volunteer-relay", "--off"]), DOCKER_SHORT);
        return Ok("Stopped relaying for others.".into());
    }
    if !crate::p2p::flags::volunteer_allowed(flags, local) {
        return Err("Turn on \"Help the network: relay for others\" for this PC first (Settings > Advanced).".into());
    }
    let host = public_host.map(str::trim).filter(|h| crate::realms::relay_ok(h))
        .ok_or("Give this PC's public DNS name (like relay.example.org): relaying for others needs one.")?;
    let mut cmd = vec!["volunteer-relay", "--host", host];
    if let Some(r) = region.filter(|r| crate::realms::REGIONS.contains(r)) {
        cmd.extend(["--region", r]);
    }
    docker_in(kit, docker::tools_args(kit, &cmd), DOCKER_SHORT)?;
    docker_in(kit, volunteer_args(kit, true), DOCKER_LONG)?;
    Ok(format!("Relaying for others as {host}. It needs ports 80, 443 and 41000-41051 open to the internet."))
}

/// The realm list details a host sets: the network's filters (rules tags, region, language, the player limit) and
/// whether direct connections are allowed (off by default; the network only lists it when its operator turns
/// direct connections on). Kept in the kit's `state/beacon.json`, which `setup.py` reads on every start and on
/// `rules` (hosting/kit/tools/wuhost/setup.py `EDITABLE`); the relay checks them again.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListingDetails {
    pub tags: Vec<String>,
    pub region: Option<String>,
    pub language: Option<String>,
    pub max_players: Option<u64>,
    pub direct: bool,
}

fn beacon_path(kit: &Path) -> PathBuf {
    kit.join("state").join("beacon.json")
}

/// The details as the kit has them now (defaults before the first start).
pub fn listing_details(kit: &Path) -> ListingDetails {
    let v: Value = fs::read_to_string(beacon_path(kit)).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
        .unwrap_or(Value::Null);
    ListingDetails {
        tags: v.get("tags").and_then(Value::as_array)
            .map(|a| realms::TAGS.iter().filter(|t| a.iter().any(|x| x.as_str() == Some(t))).map(|t| t.to_string()).collect())
            .unwrap_or_default(),
        region: v.get("region").and_then(Value::as_str).filter(|r| realms::REGIONS.contains(r)).map(String::from),
        language: v.get("language").and_then(Value::as_str).filter(|l| realms::lang_ok(l)).map(String::from),
        max_players: v.get("max_players").and_then(Value::as_u64).filter(|n| (1..=10000).contains(n)),
        direct: v.get("direct").and_then(Value::as_bool).unwrap_or(false),
    }
}

/// Check the details and put them in `state/beacon.json` (the rest of that file is kept).
pub fn write_listing_details(kit: &Path, d: &ListingDetails) -> Result<(), String> {
    if let Some(bad) = d.tags.iter().find(|t| !realms::TAGS.contains(&t.as_str())) {
        return Err(format!("\"{bad}\" is not a tag. Tags: {}.", realms::TAGS.join(", ")));
    }
    if d.region.as_deref().is_some_and(|r| !realms::REGIONS.contains(&r)) {
        return Err("Pick a region from the list.".into());
    }
    if d.language.as_deref().is_some_and(|l| !realms::lang_ok(l)) {
        return Err("Language: a code like en, de or pt-BR.".into());
    }
    if d.max_players.is_some_and(|n| !(1..=10000).contains(&n)) {
        return Err("Player limit: 1 to 10000, or empty.".into());
    }
    let p = beacon_path(kit);
    let mut doc = fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
        .filter(Value::is_object).unwrap_or_else(|| serde_json::json!({}));
    let tags: Vec<&str> = realms::TAGS.iter().copied().filter(|t| d.tags.iter().any(|x| x == t)).collect();
    let o = doc.as_object_mut().expect("an object");
    o.insert("tags".into(), serde_json::json!(tags));
    o.insert("region".into(), serde_json::json!(d.region));
    o.insert("language".into(), serde_json::json!(d.language));
    o.insert("max_players".into(), serde_json::json!(d.max_players));
    o.insert("direct".into(), serde_json::json!(d.direct));
    fs::create_dir_all(p.parent().expect("state/")).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&p, text.as_bytes()).map_err(|e| format!("Could not write state/beacon.json: {e}"))
}

/// Save the details, then tell a running realm (the kit's `rules` tool re-reads beacon.json; the agent sends it with
/// its next heartbeat). Without Docker or before the first start, they apply on the next start.
pub fn set_listing_details(kit: &Path, d: &ListingDetails) -> Result<String, String> {
    need_kit(kit)?;
    write_listing_details(kit, d)?;
    if !kit.join("state").join("host.json").is_file() {
        return Ok("Saved. They show on the realm list once your realm has started.".into());
    }
    match docker_in(kit, docker::tools_args(kit, &["rules"]), DOCKER_SHORT) {
        Ok(_) => Ok("Saved. The realm list shows them within a minute.".into()),
        Err(e) => Ok(format!("Saved. They show on the realm list from the next start ({e}).")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::testutil::touch;
    use crate::net::testserver::serve_routes;

    const RID: &str = "abcdefghijk2345a";

    fn kit() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), WINDOWS_SCRIPT, b"@echo off");
        touch(d.path(), LEYLINE_SCRIPT, b"#!/usr/bin/env bash");
        touch(d.path(), UNIX_SCRIPT, b"#!/usr/bin/env bash");
        touch(d.path(), "realm-files/profiles/main.env", b"");
        d
    }

    #[test]
    fn finds_the_kit_its_realm_and_seeds() {
        let k = kit();
        let dl = tempfile::tempdir().unwrap();
        touch(k.path(), &format!("leyline-{RID}.sh"), b"#!/bin/bash");
        touch(dl.path(), &format!("leyline-{RID}.bat"), b"@echo off");
        touch(dl.path(), "leyline-NOTANID.bat", b"x");
        touch(k.path(), "state/host.json", format!(r#"{{"realm_id":"{RID}","name":"Frost Haven","visibility":"public","relay":"relay.openvanilla.app","profile":"dalaran"}}"#).as_bytes());
        let i = inspect(Some(k.path()), Some(dl.path()));
        assert!(i.is_kit && i.has_windows_script && i.has_unix_script && i.problems.is_empty(), "{:?}", i.problems);
        assert_eq!(i.seeds.len(), 2);
        assert!(i.seeds.iter().any(|s| s.in_kit && s.kind == "sh") && i.seeds.iter().any(|s| !s.in_kit && s.kind == "bat"));
        assert_eq!(i.realm.unwrap().name, "Frost Haven");

        let not = tempfile::tempdir().unwrap();
        let i = inspect(Some(not.path()), None);
        assert!(!i.is_kit && !i.problems.is_empty());

        let s = copy_seed_into_kit(&dl.path().join(format!("leyline-{RID}.bat")), k.path()).unwrap();
        assert!(s.is_file());
        assert!(copy_seed_into_kit(&dl.path().join("leyline-NOTANID.bat"), k.path()).is_err());
    }

    #[test]
    fn start_plans_per_os() {
        let k = kit();
        touch(k.path(), &format!("leyline-{RID}.bat"), b"@echo off");
        touch(k.path(), &format!("leyline-{RID}.sh"), b"#!/bin/bash");
        let w = plan(k.path(), None, HostOs::Windows, |_| false).unwrap();
        assert_eq!((w.program.as_str(), w.args.clone(), w.new_console), ("cmd.exe", vec!["/K".to_string(), WINDOWS_SCRIPT.to_string()], true));
        let seed = format!("leyline-{RID}.bat");
        assert_eq!(plan(k.path(), Some(&seed), HostOs::Windows, |_| false).unwrap().args[1], seed);
        assert!(plan(k.path(), Some(&seed), HostOs::Linux, |_| true).unwrap_err().contains(".sh"));

        let l = plan(k.path(), None, HostOs::Linux, |t| t == "konsole").unwrap();
        assert_eq!(l.program, "konsole");
        assert_eq!(&l.args[l.args.len() - 3..], &["bash", "leyline", "host"]);
        let sh = format!("leyline-{RID}.sh");
        let bg = plan(k.path(), Some(&sh), HostOs::Linux, |_| false).unwrap();
        assert_eq!((bg.program.as_str(), bg.args.clone()), ("bash", vec![sh.clone()]));
        assert!(bg.log.is_some());

        let m = plan(k.path(), Some(&sh), HostOs::Mac, |_| false).unwrap();
        assert_eq!(m.program, "osascript");
        assert!(m.args[1].starts_with("tell application \"Terminal\" to do script \"cd '") && m.args[1].contains(&sh), "{}", m.args[1]);

        let not = tempfile::tempdir().unwrap();
        assert!(plan(not.path(), None, HostOs::Windows, |_| false).is_err());
        assert!(plan(k.path(), Some("leyline-zzzzzzzzzzzzzzzz.sh"), HostOs::Linux, |_| false).unwrap_err().contains("not in the kit"));
    }

    #[test]
    fn the_host_pages_choice_reaches_the_script() {
        let k = kit();
        let games = tempfile::tempdir().unwrap();
        touch(k.path(), "profiles/cmangos-2.4.3/profile.lock", b"LL_PROFILE=cmangos-2.4.3\n");
        let c = crate::clients::ClientProfile {
            id: "c1".into(), name: "my 2.4.3".into(), path: games.path().to_path_buf(), build: crate::clients::Build::V243,
            realmlist: String::new(), locale: None, exe_version: None, exe: None,
        };
        profiles::choose(k.path(), "cmangos-2.4.3", Some(&c)).unwrap();
        let w = plan(k.path(), None, HostOs::Windows, |_| false).unwrap();
        assert!(w.env.contains(&("WU_PROFILE".into(), "cmangos-2.4.3".into())));
        assert!(w.env.contains(&("WU_CLIENT_DIR".into(), games.path().display().to_string())));
        let l = plan(k.path(), None, HostOs::Linux, |t| t == "konsole").unwrap();
        let at = l.args.iter().position(|a| a == "env").expect("env on the command line");
        assert_eq!(l.args[at + 1], "WU_PROFILE=cmangos-2.4.3");
        assert_eq!(&l.args[l.args.len() - 3..], &["bash", "leyline", "host"]);
        let bg = plan(k.path(), None, HostOs::Linux, |_| false).unwrap();
        assert_eq!((bg.program.as_str(), bg.args.last().map(String::as_str)), ("env", Some("host")));
        let m = plan(k.path(), None, HostOs::Mac, |_| false).unwrap();
        assert!(m.args[1].contains("'env' 'WU_PROFILE=cmangos-2.4.3'"), "{}", m.args[1]);
    }

    #[test]
    fn vanilla_plus_starts_always_on_and_link_opens_a_window() {
        let k = kit();
        profiles::choose(k.path(), "vanillaplus", None).unwrap();
        let l = plan(k.path(), None, HostOs::Linux, |t| t == "konsole").unwrap();
        assert_eq!(&l.args[l.args.len() - 4..], &["bash", "leyline", "host", "--always-on"]);
        let seed = format!("leyline-{RID}.sh");
        touch(k.path(), &seed, b"#!/bin/bash");
        assert!(!plan(k.path(), Some(&seed), HostOs::Linux, |t| t == "konsole").unwrap().args.contains(&"--always-on".to_string()), "a seed has its own answers");
        profiles::choose(k.path(), "main", None).unwrap();
        assert!(!plan(k.path(), None, HostOs::Linux, |t| t == "konsole").unwrap().args.contains(&"--always-on".to_string()));
        assert!(!plan(k.path(), None, HostOs::Linux, |t| t == "konsole").unwrap().args.contains(&"--private".to_string()), "only a Den");
        // My Den: leyline host --ruleset den --private (the ruleset and private in the env too, for the Windows kit)
        profiles::choose(k.path(), "den", None).unwrap();
        let d = plan(k.path(), None, HostOs::Linux, |t| t == "konsole").unwrap();
        assert_eq!(&d.args[d.args.len() - 4..], &["bash", "leyline", "host", "--private"]);
        assert!(d.args.contains(&"LEYLINE_RULESET=den".to_string()) && d.args.contains(&"LEYLINE_VISIBILITY=private".to_string()), "{:?}", d.args);
        let w = plan(k.path(), None, HostOs::Windows, |_| false).unwrap();
        assert!(w.env.contains(&("WU_PROFILE".into(), "den".into())) && w.env.contains(&("WU_VISIBILITY".into(), "private".into())));
        assert!(!plan(k.path(), Some(&seed), HostOs::Linux, |t| t == "konsole").unwrap().args.contains(&"--private".to_string()), "a seed has its own answers");
        // Solo leveling: private too
        profiles::choose(k.path(), "solo", None).unwrap();
        let s = plan(k.path(), None, HostOs::Linux, |t| t == "konsole").unwrap();
        assert_eq!(&s.args[s.args.len() - 4..], &["bash", "leyline", "host", "--private"]);
        assert!(s.args.contains(&"LEYLINE_RULESET=solo".to_string()) && s.args.contains(&"LEYLINE_VISIBILITY=private".to_string()), "{:?}", s.args);
        profiles::choose(k.path(), "den", None).unwrap();

        assert!(plan_link(k.path(), HostOs::Windows, |_| false).unwrap_err().contains(LINK_SCRIPT));
        touch(k.path(), LINK_SCRIPT, b"@echo off");
        let w = plan_link(k.path(), HostOs::Windows, |_| false).unwrap();
        assert_eq!((w.program.as_str(), w.args.clone(), w.new_console), ("cmd.exe", vec!["/K".to_string(), LINK_SCRIPT.to_string()], true));
        let x = plan_link(k.path(), HostOs::Linux, |t| t == "xterm").unwrap();
        assert_eq!((x.program.as_str(), &x.args[x.args.len() - 3..]), ("xterm", &["bash".to_string(), "leyline".into(), "link".into()][..]));
        assert!(plan_link(k.path(), HostOs::Linux, |_| false).unwrap_err().contains("bash leyline link"), "its questions need a window");
        assert!(plan_link(k.path(), HostOs::Mac, |_| false).unwrap().args[1].contains("'leyline' 'link'"));
    }

    #[test]
    fn a_quiet_start_takes_the_forms_answers_and_opens_no_window() {
        let k = kit();
        profiles::choose(k.path(), "dalaran", None).unwrap();
        let a = Answers { name: Some("Frost Haven".into()), visibility: Some("public".into()), gm: Some("hostguy".into()) };
        let w = plan_quiet(k.path(), HostOs::Windows, &a).unwrap();
        assert_eq!((w.program.as_str(), w.args.clone(), w.new_console), ("cmd.exe", vec!["/C".to_string(), WINDOWS_SCRIPT.to_string()], false));
        assert_eq!(w.log.as_deref(), Some(job_log(k.path()).as_path()), "no window: its output goes to the log the launcher reads");
        for kv in [("WU_NAME", "Frost Haven"), ("WU_VISIBILITY", "public"), ("WU_GM_NAME", "hostguy"), ("WU_PROFILE", "dalaran"), ("LEYLINE_NONINTERACTIVE", "1")] {
            assert!(w.env.contains(&(kv.0.to_string(), kv.1.to_string())), "{kv:?} in {:?}", w.env);
        }
        let l = plan_quiet(k.path(), HostOs::Linux, &a).unwrap();
        assert_eq!((l.program.as_str(), l.args.clone()), ("bash", vec!["leyline".to_string(), "host".into()]));
        assert!(l.env.contains(&("LEYLINE_NAME".into(), "Frost Haven".into())) && l.env.contains(&("LEYLINE_GM".into(), "hostguy".into())));
        assert!(!l.args.iter().any(|x| x.contains("Frost")), "answers never on a command line");
        // leyline host needs the host's account name (it never makes one)
        let no_gm = Answers { gm: None, ..a.clone() };
        assert!(plan_quiet(k.path(), HostOs::Mac, &no_gm).unwrap_err().contains("account name"));
        assert!(plan_quiet(k.path(), HostOs::Windows, &no_gm).is_ok(), "the Windows kit makes its GM login itself");
        // checked here before anything runs
        for bad in ["", "x".repeat(33).as_str(), "My \"realm\" & co", "WoW Classic", "Lich King Fans", "a/b"] {
            if bad.is_empty() { continue; }
            assert!(plan_quiet(k.path(), HostOs::Windows, &Answers { name: Some(bad.into()), ..a.clone() }).is_err(), "{bad}");
        }
        assert!(plan_quiet(k.path(), HostOs::Windows, &Answers { visibility: Some("secret".into()), ..a.clone() }).is_err());
        assert!(plan_quiet(k.path(), HostOs::Linux, &Answers { gm: Some("no spaces".into()), ..a.clone() }).is_err());
        assert!(name_ok("Dalaran Hangout") && name_ok("O'Neil's-Den_2") && !name_ok("wow") && name_ok("Wowzers"));
        // a Den: the form's private and name win over the saved choice's, once each
        profiles::choose(k.path(), "den", None).unwrap();
        let d = plan_quiet(k.path(), HostOs::Windows, &Answers { name: Some("Our Den".into()), visibility: Some("private".into()), gm: None }).unwrap();
        assert_eq!(d.env.iter().filter(|(k, _)| k == "WU_NAME").collect::<Vec<_>>(), vec![&("WU_NAME".to_string(), "Our Den".to_string())]);
        // Vanilla+ stays up
        profiles::choose(k.path(), "vanillaplus", None).unwrap();
        assert_eq!(plan_quiet(k.path(), HostOs::Linux, &a).unwrap().args.last().unwrap(), "--always-on");
    }

    #[test]
    fn a_quiet_link_takes_what_was_found() {
        let k = kit();
        let a = LinkAnswers { name: "My Server".into(), auth: "127.0.0.1:3724".into(), world: "127.0.0.1:8086".into(), client: "wrath".into(), core: Some("azerothcore".into()) };
        assert!(plan_link_quiet(k.path(), HostOs::Windows, &a).unwrap_err().contains(LINK_SCRIPT));
        touch(k.path(), LINK_SCRIPT, b"@echo off");
        let w = plan_link_quiet(k.path(), HostOs::Windows, &a).unwrap();
        assert_eq!((w.program.as_str(), w.args.clone(), w.new_console, w.log.is_some()), ("cmd.exe", vec!["/C".to_string(), LINK_SCRIPT.to_string()], false, true));
        for kv in [("LEYLINE_LINK_NAME", "My Server"), ("LEYLINE_LINK_AUTH", "127.0.0.1:3724"), ("LEYLINE_LINK_WORLD", "127.0.0.1:8086"), ("LEYLINE_LINK_CLIENT", "wrath"), ("LEYLINE_LINK_CORE", "azerothcore")] {
            assert!(w.env.contains(&(kv.0.to_string(), kv.1.to_string())), "{kv:?}");
        }
        let l = plan_link_quiet(k.path(), HostOs::Linux, &a).unwrap();
        assert_eq!(l.args, vec!["leyline".to_string(), "link".into()]);
        for bad in [
            LinkAnswers { auth: "127.0.0.1:3724; rm -rf /".into(), ..a.clone() },
            LinkAnswers { world: "host".into(), ..a.clone() },
            LinkAnswers { client: "cata".into(), ..a.clone() },
            LinkAnswers { name: "World of Warcraft".into(), ..a.clone() },
        ] {
            assert!(plan_link_quiet(k.path(), HostOs::Linux, &bad).is_err(), "{bad:?}");
        }
        assert!(!plan_link_quiet(k.path(), HostOs::Linux, &LinkAnswers { core: Some("$(evil)".into()), ..a.clone() }).unwrap().env.iter().any(|(k, _)| k == "LEYLINE_LINK_CORE"));
        // a kit that hosts a realm keeps it
        touch(k.path(), "state/host.json", format!(r#"{{"realm_id":"{RID}","name":"Frost Haven"}}"#).as_bytes());
        assert!(plan_link_quiet(k.path(), HostOs::Linux, &a).unwrap_err().contains("runs a realm already"));
        touch(k.path(), "state/host.json", format!("{{\n \"realm_id\": \"{RID}\",\n \"linked\": true\n}}").as_bytes());
        assert!(plan_link_quiet(k.path(), HostOs::Linux, &a).is_ok(), "linking again is fine");
    }

    #[test]
    fn the_steps_in_plain_words_and_never_a_secret() {
        let out = "\n== Helper image\n\n== Your realm\n\n== Realm images\n  Using the prebuilt images.\n\n== Game data for the server (from your own 3.3.5a install; nothing is downloaded or shared)\n";
        let p = read_progress(out, "host", true, None, 42);
        assert_eq!((p.step.as_str(), p.n, p.of, p.running, p.problem.clone()), ("Reading the game data from your game folder", 4, 8, true, None));
        assert!(!p.step.contains("3.3.5a"));
        let p = read_progress("== Helper image\n", "host", false, Some(1), 3);
        assert!(p.problem.unwrap().starts_with("Setting up stopped"));
        // a failure says the kit's last line only when it is safe to show
        let p = read_progress("== Realm images\n  Could not download the ready-made realm images (is this PC online?).\n", "host", false, Some(1), 9);
        assert!(p.problem.unwrap().contains("It said: Could not download"), "a plain line is passed on");
        let p = read_progress("== Game data\nNo 3.3.5a game files in \"C:\\Games\\World of Warcraft 3.3.5a\".\n", "host", false, Some(1), 9);
        let t = p.problem.unwrap();
        assert!(t.contains("Clients page") && !t.contains("3.3.5a") && !t.contains("Warcraft"), "{t}");
        for bad in ["  password: Q2W3E4R5T6Y7", "Your GM login: state\\host-login.txt", "https://relay.openvanilla.app/join/abc#ABCDEFGHJKMNPQRS",
            "claim token XYZ", "Your realm key", "\u{1b}[31mProblem: World of Warcraft folder\u{1b}[0m"] {
            assert_eq!(safe_line(bad), None, "{bad}");
        }
        assert_eq!(safe_line("\u{1b}[31mProblem: Docker is not running.\u{1b}[0m").as_deref(), Some("Docker is not running."));
        let done = read_progress("== Helper image\n== Your realm\n== Starting the realm\n== Connecting to the relay\n== Done\n", "host", false, Some(0), 600);
        assert_eq!((done.step.as_str(), done.problem), ("Done", None));
        let l = read_progress("== Helper image\n== Link a server you already run\n== Starting the helper\n", "link", true, None, 5);
        assert_eq!((l.step.as_str(), l.n, l.of), ("Connecting your server to the network", 3, 4));
        assert!(progress().idle || !progress().kind.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_quiet_run_is_followed_to_its_end() {
        let k = kit();
        let p = HostPlan { program: "sh".into(), args: vec!["-c".into(), "echo '== Helper image'; echo '== Your realm'; exit 0".into()],
            cwd: k.path().to_path_buf(), new_console: false, log: Some(job_log(k.path())), env: vec![], note: "Setting up.".into() };
        assert_eq!(run_job(&p, "host").unwrap(), "Setting up.");
        let mut last = progress();
        for _ in 0..100 {
            if !last.running { break; }
            std::thread::sleep(std::time::Duration::from_millis(30));
            last = progress();
        }
        assert_eq!((last.running, last.code, last.n), (false, Some(0), 2));
        assert_eq!(last.step, "Saving your settings");
    }

    #[cfg(unix)]
    #[test]
    fn a_background_start_runs_in_the_kit() {
        let k = kit();
        touch(k.path(), &format!("leyline-{RID}.sh"), b"echo started > state/ran.txt\n");
        let p = plan(k.path(), Some(&format!("leyline-{RID}.sh")), HostOs::Linux, |_| false).unwrap();
        run(&p).unwrap();
        for _ in 0..50 {
            if k.path().join("state/ran.txt").exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert_eq!(fs::read_to_string(k.path().join("state/ran.txt")).unwrap().trim(), "started");
    }

    #[test]
    fn invite_links_from_the_kit() {
        let k = kit();
        assert_eq!(invite(k.path()), None);
        touch(k.path(), "state/invite.txt", format!("https://relay.openvanilla.app/join/{RID}#ABCDEFGHJKMNPQRSTVWX\n").as_bytes());
        assert_eq!(invite(k.path()).unwrap(), format!("https://relay.openvanilla.app/join/{RID}#ABCDEFGHJKMNPQRSTVWX"));
        touch(k.path(), "state/invite.txt", format!("https://openvanilla.app/join/{RID}").as_bytes());
        assert!(invite(k.path()).is_some(), "a public realm's link has no secret");
        touch(k.path(), "state/invite.txt", b"javascript:alert(1)");
        assert_eq!(invite(k.path()), None);
        touch(k.path(), "state/invite.txt", format!("https://evil.example.com:22/join/{RID}").as_bytes());
        assert_eq!(invite(k.path()), None);
        assert!(stop(k.path()).unwrap_err().contains("docker-compose.host.yml"));
        let f = crate::p2p::flags::Flags::default();
        let l = crate::p2p::flags::Local::default();
        assert!(volunteer_relay(k.path(), &f, &l, true, Some("relay.example.org"), None).unwrap_err().contains("update the Host pack"));
        touch(k.path(), VOLUNTEER_COMPOSE, b"services: {}");
        assert!(volunteer_relay(k.path(), &f, &l, true, Some("relay.example.org"), None).unwrap_err().contains("Settings > Advanced"), "off by default");
        let on = crate::p2p::flags::Flags { relay_volunteer: true, ..Default::default() };
        let mut lo = crate::p2p::flags::Local::default();
        lo.overrides.insert("relay_volunteer".into(), true);
        assert!(volunteer_relay(k.path(), &on, &lo, true, Some("not a host"), None).unwrap_err().contains("DNS name"));
        let a = volunteer_args(k.path(), true);
        assert!(a[2].ends_with(VOLUNTEER_COMPOSE) && a[5..] == ["up", "-d", "--build"]);
        assert_eq!(volunteer_args(k.path(), false).last().unwrap(), "down");
        touch(k.path(), "docker-compose.host.yml", b"services: {}");
        assert!(new_invite(k.path()).unwrap_err().contains("Start your realm once"));
    }

    #[test]
    fn listing_details_in_beacon_json() {
        let d = kit();
        assert_eq!(listing_details(d.path()), ListingDetails::default(), "nothing set yet");
        std::fs::create_dir_all(d.path().join("state")).unwrap();
        std::fs::write(d.path().join("state").join("beacon.json"), r#"{"name":"Frost Haven","tagline":"Duels","tags":["rp","x"]}"#).unwrap();
        assert_eq!(listing_details(d.path()).tags, vec!["rp".to_string()]);
        let want = ListingDetails { tags: vec!["rp".into(), "pvp".into()], region: Some("eu".into()), language: Some("pt-BR".into()),
            max_players: Some(80), direct: false };
        write_listing_details(d.path(), &want).unwrap();
        let back = listing_details(d.path());
        assert_eq!(back.tags, vec!["pvp".to_string(), "rp".to_string()], "in the listing's order");
        assert_eq!((back.region, back.language, back.max_players, back.direct), (Some("eu".into()), Some("pt-BR".into()), Some(80), false));
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(d.path().join("state").join("beacon.json")).unwrap()).unwrap();
        assert_eq!((doc["name"].as_str(), doc["tagline"].as_str()), (Some("Frost Haven"), Some("Duels")), "the rest is kept");
        for bad in [
            ListingDetails { tags: vec!["pay2win".into()], ..Default::default() },
            ListingDetails { region: Some("mars".into()), ..Default::default() },
            ListingDetails { language: Some("english".into()), ..Default::default() },
            ListingDetails { max_players: Some(0), ..Default::default() },
        ] {
            assert!(write_listing_details(d.path(), &bad).is_err(), "{bad:?}");
        }
        assert_eq!(listing_details(d.path()).region.as_deref(), Some("eu"), "a refused change writes nothing");
        // before the first start: saved, applied later (no Docker call)
        touch(d.path(), "docker-compose.host.yml", b"services: {}");
        let msg = set_listing_details(d.path(), &ListingDetails { direct: true, ..Default::default() }).unwrap();
        assert!(msg.contains("once your realm has started") && listing_details(d.path()).direct);
    }

    #[test]
    fn status_from_the_relay() {
        let info = format!(r#"{{"realm_id":"{RID}","online":true,"name":"Frost Haven","visibility":"public","address":"relay.openvanilla.app","auth_port":40002}}"#);
        let base = serve_routes(vec![
            (Box::leak(format!("/api/realms/{RID}").into_boxed_str()), "200 OK", info.into_bytes()),
            (Box::leak(format!("/r/{RID}/status.json").into_boxed_str()), "200 OK", br#"{"online":true,"players":2,"bots":0}"#.to_vec()),
        ]);
        let s = status(&base, RID, true).unwrap();
        assert!(s.online && s.listed);
        assert_eq!((s.players, s.address.as_deref()), (Some(2), Some("relay.openvanilla.app:40002")));
        let none = serve_routes(vec![(Box::leak(format!("/api/realms/{RID}").into_boxed_str()), "404 Not Found", br#"{"error":"unknown realm"}"#.to_vec())]);
        assert!(status(&none, RID, false).unwrap_err().contains("has not seen this realm"));
    }
}
