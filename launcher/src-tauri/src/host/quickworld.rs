//! Worlds in three clicks (docs/UNSTOPPABLE.md, "Worlds in three clicks"): Host › a template › Start, made fast by
//! never doing the slow parts twice. Everything here is made on the player's PC from the engine and their own game
//! folder; nothing is downloaded or sent but the engine itself.
//!
//! * **The prepared database** (`<app>/engine-cache/base-<engine sha>/`): the first world filled on this PC (MySQL's
//!   first run and `dbimport`, the slow part: many minutes) is copied here with MySQL stopped, before its own rules
//!   and logins go in. Every later world of any template (Single player, the original world, Boss Rush, a world made
//!   again after a reset) copies it in seconds and adds only its template's own SQL. One per engine: a new engine
//!   makes a new one and the old one goes. The database's root password goes with the copy (`base.json`), the same
//!   generated, local-only password every world keeps in its own `state.json`.
//! * **The maps** are read once from the player's game folder into Single player's `data/` (host/engine.rs; the
//!   original world already shares that folder). Boss Rush, which plays a few raid maps, copies what it needs from
//!   there instead of running the map reader again ([`copy_maps`]).
//! * **Background readiness** (engine.rs `begin_prepare`): after the game folder is found, the engine is fetched and
//!   the maps read quietly, at low priority, so Start later is only the copy and the first start. Settings › "Get
//!   worlds ready in the background" turns it off (settings.rs `world_get_ready`).
//! * **Timings** ([`Timer`]): each setup step's seconds in the world's `logs/setup.log` and in
//!   `<app>/world-timings.log` (one line per step), for phase 0's fresh-PC numbers (docs/smoke/worlds-three-clicks.md).

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const CACHE_DIR: &str = "engine-cache";
pub const BASE_FILE: &str = "base.json";
pub const TIMINGS_FILE: &str = "world-timings.log";
/// Left out of a copied MySQL data folder: its server id (made again on first start), its pid and error files.
const SKIP: [&str; 3] = ["auto.cnf", "mysqld.pid", "mysqld.err"];
/// Room kept free after making the prepared copy.
pub const ROOM: u64 = 2 * 1024 * 1024 * 1024;

/// What `base.json` says about a prepared database.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Base {
    /// The engine (its zip's sha256) whose `dbimport` filled it.
    pub sha: String,
    /// MySQL's root password inside the copy (generated on this PC, never shown or sent).
    pub db_password: String,
    pub made_at: u64,
    pub bytes: u64,
}

pub fn cache_root(app: &Path) -> PathBuf {
    app.join(CACHE_DIR)
}

/// The folder for an engine's prepared database (the sha's first 16 hex characters).
pub fn base_dir(app: &Path, sha: &str) -> PathBuf {
    let short: String = sha.chars().filter(|c| c.is_ascii_hexdigit()).take(16).collect::<String>().to_ascii_lowercase();
    cache_root(app).join(format!("base-{short}"))
}

/// A prepared database for this engine, when one is complete: its `base.json` names this engine, has a password,
/// and its data folder has MySQL's own schema and the world's.
pub fn base_ready(app: &Path, sha: &str) -> Option<Base> {
    if sha.len() < 16 {
        return None;
    }
    let dir = base_dir(app, sha);
    let b: Base = serde_json::from_str(&fs::read_to_string(dir.join(BASE_FILE)).ok()?).ok()?;
    let db = dir.join("db");
    (b.sha == sha && !b.db_password.is_empty() && db.join("mysql").is_dir() && db.join("acore_world").is_dir()).then_some(b)
}

/// Copy a folder's tree (files and folders only; links and the [`SKIP`] files left out). Returns the bytes copied.
pub fn copy_tree(from: &Path, to: &Path) -> Result<u64, String> {
    fs::create_dir_all(to).map_err(|e| format!("Couldn't make {}: {e}", to.display()))?;
    let mut n = 0u64;
    for e in fs::read_dir(from).map_err(|e| format!("Couldn't read {}: {e}", from.display()))?.flatten() {
        let name = e.file_name();
        let ft = e.file_type().map_err(|e| e.to_string())?;
        let dst = to.join(&name);
        if ft.is_dir() {
            n += copy_tree(&e.path(), &dst)?;
        } else if ft.is_file() && !SKIP.contains(&name.to_string_lossy().as_ref()) {
            n += fs::copy(e.path(), &dst).map_err(|er| format!("Couldn't copy {}: {er}", name.to_string_lossy()))?;
        }
    }
    Ok(n)
}

/// Bytes in a folder's files.
pub fn size_of(dir: &Path) -> u64 {
    let Ok(rd) = fs::read_dir(dir) else { return 0 };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => size_of(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// Copy `from` to `to` through `<to>.part` and a rename, so a copy stopped midway is never taken for a whole one.
pub fn copy_whole(from: &Path, to: &Path) -> Result<u64, String> {
    let part = to.with_extension("part");
    let _ = fs::remove_dir_all(&part);
    let n = copy_tree(from, &part).inspect_err(|_| {
        let _ = fs::remove_dir_all(&part);
    })?;
    let _ = fs::remove_dir_all(to);
    fs::rename(&part, to).map_err(|e| format!("Couldn't finish the copy: {e}"))?;
    Ok(n)
}

/// Keep a stopped world's database as this engine's prepared one (the MySQL server must be stopped). Other engines'
/// prepared databases go. `free`: the disk's free bytes, when known: no copy without [`ROOM`] to spare after it.
pub fn save_base(app: &Path, sha: &str, db: &Path, db_password: &str, free: Option<u64>, now: u64) -> Result<Base, String> {
    if sha.len() < 16 || db_password.is_empty() {
        return Err("no engine to key it by".into());
    }
    let need = size_of(db);
    if free.is_some_and(|f| f < need.saturating_add(ROOM)) {
        return Err(format!("not enough room for the prepared copy ({} MB)", need / 1_048_576));
    }
    let dir = base_dir(app, sha);
    let _ = fs::remove_dir_all(&dir);
    let bytes = copy_whole(db, &dir.join("db"))?;
    let b = Base { sha: sha.to_string(), db_password: db_password.to_string(), made_at: now, bytes };
    crate::settings::write_atomic(&dir.join(BASE_FILE), serde_json::to_string_pretty(&b).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())?;
    // only the newest engine's is kept
    if let Ok(rd) = fs::read_dir(cache_root(app)) {
        for e in rd.flatten() {
            let p = e.path();
            if p != dir && e.file_name().to_string_lossy().starts_with("base-") {
                let _ = fs::remove_dir_all(p);
            }
        }
    }
    Ok(b)
}

/// A new world's database from the prepared one: copied into `db` (which must hold no saves). Returns the copy's
/// password (the world's `state.json` takes it) and the bytes copied.
pub fn use_base(app: &Path, sha: &str, db: &Path) -> Result<(String, u64), String> {
    let b = base_ready(app, sha).ok_or("no prepared database for this engine")?;
    if db.exists() {
        let saves = fs::read_dir(db).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("acore"))).unwrap_or(false);
        if saves {
            return Err("The world's database folder already has saves in it: it was left as it is.".into());
        }
    }
    let n = copy_whole(&base_dir(app, sha).join("db"), db)?;
    Ok((b.db_password, n))
}

/// Boss Rush's maps from Single player's (`from` and `to` are the two worlds' `data/` folders): `dbc` and `Cameras`
/// whole, and the map files `keep` says it plays. Returns the files copied.
pub fn copy_maps(from: &Path, to: &Path, keep: impl Fn(&str) -> bool) -> Result<usize, String> {
    let mut n = 0usize;
    for d in ["dbc", "Cameras"] {
        let src = from.join(d);
        if src.is_dir() {
            copy_whole(&src, &to.join(d))?;
            n += fs::read_dir(to.join(d)).map(|rd| rd.count()).unwrap_or(0);
        }
    }
    let dst = to.join("maps");
    let _ = fs::remove_dir_all(&dst);
    fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
    for e in fs::read_dir(from.join("maps")).map_err(|e| format!("Single player's maps aren't readable: {e}"))?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if e.file_type().is_ok_and(|t| t.is_file()) && (!name.ends_with(".map") || keep(&name)) {
            fs::copy(e.path(), dst.join(&name)).map_err(|er| format!("Couldn't copy {name}: {er}"))?;
            n += 1;
        }
    }
    Ok(n)
}

// ------------------------------------------------------------------------------------------------ timings

/// Seconds per setup step, written as each one ends.
pub struct Timer {
    log: PathBuf,
    world: String,
    started: Instant,
    step: Option<(String, Instant)>,
}

/// One timing line: `<unix secs> <world> <step> <seconds, one decimal>` (steps are single words).
pub fn timing_line(at: u64, world: &str, step: &str, secs: f64) -> String {
    format!("{at} {world} {step} {secs:.1}")
}

/// The timings file's lines, newest last, as (world, step, seconds).
pub fn read_timings(text: &str) -> Vec<(String, String, f64)> {
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let (_, world, step, secs) = (w.next()?, w.next()?, w.next()?, w.next()?.parse().ok()?);
            Some((world.to_string(), step.to_string(), secs))
        })
        .collect()
}

impl Timer {
    pub fn new(app: &Path, world: &str) -> Timer {
        Timer { log: app.join(TIMINGS_FILE), world: world.to_string(), started: Instant::now(), step: None }
    }
    /// A step begins (the one before, if any, ends). Returns the line for the step that ended.
    pub fn step(&mut self, name: &str) -> Option<String> {
        let ended = self.end();
        self.step = Some((name.to_string(), Instant::now()));
        ended
    }
    /// The current step ends: its line, also appended to the timings file.
    pub fn end(&mut self) -> Option<String> {
        let (name, t) = self.step.take()?;
        Some(self.write(&name, t.elapsed().as_secs_f64()))
    }
    /// The whole run (`how`: "fresh", "prepared" or "start"), after the last step.
    pub fn total(&mut self, how: &str) -> String {
        self.end();
        let secs = self.started.elapsed().as_secs_f64();
        self.write(&format!("total-{how}"), secs)
    }
    fn write(&self, step: &str, secs: f64) -> String {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let line = timing_line(now, &self.world, step, secs);
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&self.log) {
            let _ = writeln!(f, "{line}");
        }
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("leyline-qw-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// A stand-in MySQL data folder: its own schema, the world's, a server id and a pid file.
    fn fake_db(at: &Path) {
        for (f, body) in [("mysql/user.ibd", "u"), ("acore_world/creature.ibd", "creatures"), ("acore_auth/account.ibd", "a"), ("ibdata1", "data"), ("auto.cnf", "uuid"), ("mysqld.pid", "1")] {
            let p = at.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
    }

    #[test]
    fn the_first_world_is_kept_and_the_next_one_copies_it() {
        let app = tmp("base");
        let first = app.join("engine/db");
        fake_db(&first);
        assert_eq!(base_ready(&app, SHA), None, "nothing kept yet");
        let b = save_base(&app, SHA, &first, "ROOTPW", Some(u64::MAX), 7).unwrap();
        assert_eq!((b.sha.as_str(), b.db_password.as_str(), b.made_at), (SHA, "ROOTPW", 7));
        assert_eq!(base_ready(&app, SHA), Some(b.clone()));
        assert_eq!(base_ready(&app, &SHA.replace('0', "f")), None, "another engine's isn't this one's");
        // a second world: copied, the server id and pid left out, the password handed over
        let second = app.join("engine-vanilla/db");
        let (pw, n) = use_base(&app, SHA, &second).unwrap();
        assert_eq!(pw, "ROOTPW");
        assert!(n > 0);
        assert_eq!(fs::read_to_string(second.join("acore_world/creature.ibd")).unwrap(), "creatures");
        assert!(second.join("mysql").is_dir() && second.join("ibdata1").is_file());
        assert!(!second.join("auto.cnf").exists() && !second.join("mysqld.pid").exists());
        assert!(!second.with_extension("part").exists(), "no half copy left");
    }

    #[test]
    fn saves_are_never_overwritten_and_a_half_copy_is_never_trusted() {
        let app = tmp("safe");
        let first = app.join("engine/db");
        fake_db(&first);
        save_base(&app, SHA, &first, "PW", None, 1).unwrap();
        // a world that has saves keeps them
        let r = use_base(&app, SHA, &first);
        assert!(r.unwrap_err().contains("saves"));
        assert_eq!(fs::read_to_string(first.join("acore_world/creature.ibd")).unwrap(), "creatures");
        // a prepared copy with its base.json gone (stopped midway) isn't one
        fs::remove_file(base_dir(&app, SHA).join(BASE_FILE)).unwrap();
        assert_eq!(base_ready(&app, SHA), None);
        assert!(use_base(&app, SHA, &app.join("engine-bossrush/db")).is_err());
    }

    #[test]
    fn only_the_newest_engine_is_kept_and_room_is_checked() {
        let app = tmp("newest");
        let db = app.join("engine/db");
        fake_db(&db);
        save_base(&app, SHA, &db, "PW", None, 1).unwrap();
        let other = SHA.replace('1', "e");
        save_base(&app, &other, &db, "PW", None, 2).unwrap();
        assert!(base_ready(&app, &other).is_some());
        assert!(!base_dir(&app, SHA).exists(), "the old engine's copy went");
        // no copy that would leave the disk nearly full
        let e = save_base(&app, SHA, &db, "PW", Some(1024), 3).unwrap_err();
        assert!(e.contains("room"), "{e}");
        assert!(save_base(&app, "short", &db, "PW", None, 3).is_err());
        assert!(save_base(&app, SHA, &db, "", None, 3).is_err());
    }

    #[test]
    fn boss_rush_copies_only_the_maps_it_plays() {
        let app = tmp("maps");
        let from = app.join("engine/data");
        for f in ["dbc/Spell.dbc", "Cameras/a.m2", "maps/0004331.map", "maps/5330000.map", "maps/5330001.map", "maps/readme.txt"] {
            let p = from.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, "x").unwrap();
        }
        let to = app.join("engine-bossrush/data");
        let n = copy_maps(&from, &to, |name| name.starts_with("533")).unwrap();
        assert_eq!(n, 5);
        assert!(to.join("dbc/Spell.dbc").is_file() && to.join("Cameras/a.m2").is_file());
        assert!(to.join("maps/5330000.map").is_file() && to.join("maps/readme.txt").is_file());
        assert!(!to.join("maps/0004331.map").exists());
        // the real filter (host/bossrush.rs keeps_map_file) on the real names
        let n2 = copy_maps(&from, &app.join("br2"), crate::host::bossrush::keeps_map_file).unwrap();
        assert!(n2 >= 3, "dbc, Cameras and the text file at least");
    }

    #[test]
    fn timings_are_one_line_per_step() {
        let app = tmp("timings");
        let mut t = Timer::new(&app, "single");
        assert_eq!(t.step("download"), None);
        let l = t.step("maps").unwrap();
        assert!(l.contains(" single download "), "{l}");
        let total = t.total("fresh");
        assert!(total.contains(" single total-fresh "), "{total}");
        let read = read_timings(&fs::read_to_string(app.join(TIMINGS_FILE)).unwrap());
        let steps: Vec<&str> = read.iter().map(|(_, s, _)| s.as_str()).collect();
        assert_eq!(steps, ["download", "maps", "total-fresh"]);
        assert!(read.iter().all(|(w, _, secs)| w == "single" && *secs >= 0.0));
        assert_eq!(timing_line(5, "vanilla", "copy", 12.345), "5 vanilla copy 12.3");
        assert!(read_timings("junk\n1 a b notanumber\n").is_empty());
    }
}
