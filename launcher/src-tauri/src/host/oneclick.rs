//! Single player in one click (docs/SINGLE-PLAYER-ONE-CLICK.md §contract): the parts of the `world_*` commands that
//! aren't the engine itself.
//!
//! * error codes: every error those commands give starts with a short code in brackets ([`CODES`]) the page maps to
//!   a button, then one plain sentence;
//! * finding the game folder by itself ([`scan_games`]): the folders players usually install to, a few levels deep;
//! * the disk check before setup ([`need_bytes`], [`free_bytes`]);
//! * the Hero Key's login in a local world ([`hero_name`]): a stable name from the signed-in account, made with a
//!   secret that never leaves this PC;
//! * stopping a world after its game closes ([`stop_after_exit`]);
//! * Repair's file check ([`read_manifest`], [`manifest_bad`], [`mend_from`]);
//! * a setup the launcher closed in the middle of ([`resume_pending`]).

use crate::host::engine::{self, Kind, Step};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// ------------------------------------------------------------------------------------------------ error codes

/// The codes, each with the button the page shows for it.
/// * `folder`: no game folder, or not one of the version the world needs (Pick your game folder)
/// * `disk`: not enough free space, or a folder couldn't be written (Free some space)
/// * `download`: the engine couldn't be fetched or didn't match its checksum (Try again)
/// * `maps`: the maps couldn't be read from the game folder (Pick another game folder / Repair)
/// * `db`: the world's database didn't start, fill or answer (Repair)
/// * `ports`: another world or program holds the ports (Stop it and play)
/// * `antivirus`: a program of the engine went missing right after unpacking (How to allow Leyline / Repair)
/// * `login`: the world's login couldn't be written (Repair)
/// * `other`: anything else (Show the log)
pub const CODES: [&str; 9] = ["folder", "disk", "download", "maps", "db", "ports", "antivirus", "login", "other"];

/// The code an error starts with, if it has one.
pub fn code_of(msg: &str) -> Option<&str> {
    let rest = msg.strip_prefix('[')?;
    let (code, _) = rest.split_once(']')?;
    CODES.contains(&code).then_some(code)
}

/// `[code] message`, unless the message has its code already.
pub fn tag(code: &str, msg: impl Into<String>) -> String {
    let msg = msg.into();
    if code_of(&msg).is_some() {
        return msg;
    }
    let code = if CODES.contains(&code) { code } else { "other" };
    format!("[{code}] {}", msg.trim())
}

/// The code of an error a setup or repair step gave (its words first: a missing program, a port; else the step's).
pub fn classify(step: Step, msg: String) -> String {
    if code_of(&msg).is_some() {
        return msg;
    }
    let low = msg.to_ascii_lowercase();
    let code = if low.contains("isn't in the engine folder") {
        "antivirus"
    } else if low.contains("same ports") || low.contains(" port (") || low.contains("login port") {
        "ports"
    } else if low.contains("no space") || low.contains("not enough space") || low.contains("disk full") || low.contains("os error 112") {
        "disk"
    } else {
        match step {
            Step::Download | Step::Unpack | Step::Verify => "download",
            Step::Maps => "maps",
            Step::Database | Step::Fill => "db",
            Step::Account | Step::Login => "login",
            Step::Ports => "ports",
            Step::Start => {
                if low.contains("database") {
                    "db"
                } else {
                    "other"
                }
            }
        }
    };
    tag(code, msg)
}

// ------------------------------------------------------------------------------------------------ the game folder

/// A folder that could be a game folder: it has a `Data` folder (any case) and a program in it.
pub fn looks_like_game(dir: &Path) -> bool {
    let Ok(rd) = fs::read_dir(dir) else { return false };
    let (mut data, mut exe) = (false, false);
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().to_ascii_lowercase();
        if n == "data" && e.path().is_dir() {
            data = true;
        } else if n.ends_with(".exe") || n.ends_with(".app") {
            exe = true;
        }
    }
    data && exe
}

/// Folders never looked into (system and tool folders: big, never a game).
fn skip(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with('.')
        || n.starts_with('$')
        || ["windows", "node_modules", "appdata", "programdata", "system volume information", "recovery", "msocache", "perflogs", "microsoft", "common files", "windowsapps", "drivers"].contains(&n.as_str())
}

/// The game folders under `roots`, looking `depth` folders down (a root itself counts as depth 0), at most `cap`
/// folders looked at in all. In the order found; a folder found once is never listed twice.
pub fn scan_games(roots: &[PathBuf], depth: usize, cap: usize) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen = 0usize;
    let mut level: Vec<PathBuf> = roots.iter().filter(|r| r.is_dir()).cloned().collect();
    for d in 0..=depth {
        let mut next = Vec::new();
        for dir in level {
            if seen >= cap {
                return out;
            }
            seen += 1;
            if looks_like_game(&dir) {
                if !out.contains(&dir) {
                    out.push(dir);
                }
                continue; // a game folder's own folders are never another game
            }
            if d == depth {
                continue;
            }
            if let Ok(rd) = fs::read_dir(&dir) {
                let mut kids: Vec<PathBuf> = rd
                    .flatten()
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false) && !skip(&e.file_name().to_string_lossy()))
                    .map(|e| e.path())
                    .collect();
                kids.sort();
                next.extend(kids);
            }
        }
        level = next;
    }
    out
}

/// Where players put game folders: each drive's root, its Games and Program Files folders, and the player's own
/// Desktop, Downloads, Documents and Games (on other systems: the home folder and its Games, Wine's drive C).
pub fn usual_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let home = dirs::home_dir();
    if cfg!(windows) {
        for letter in b'C'..=b'Z' {
            let root = PathBuf::from(format!("{}:\\", letter as char));
            if !root.is_dir() {
                continue;
            }
            v.push(root.clone());
            for sub in ["Games", "Program Files", "Program Files (x86)", "Spiele", "Jeux", "Juegos"] {
                v.push(root.join(sub));
            }
        }
    }
    if let Some(h) = &home {
        for sub in ["Desktop", "Downloads", "Documents", "Games", "OneDrive"] {
            v.push(h.join(sub));
        }
        v.push(h.clone());
        if !cfg!(windows) {
            v.push(h.join(".wine").join("drive_c"));
            v.push(h.join(".wine").join("drive_c").join("Program Files"));
        }
    }
    v.retain(|p| p.is_dir());
    v.dedup();
    v
}

// ------------------------------------------------------------------------------------------------ the disk

/// What a setup takes on the disk, in bytes. From the release's .json when it says (`zip_bytes`, `unpacked_bytes`),
/// else these estimates (to confirm on a real PC: the engine zip was ~150-250 MB in the workflow's first builds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizes {
    /// The engine's download.
    pub zip: u64,
    /// The engine unpacked (programs, MySQL, the SQL tree, our scripts).
    pub unpacked: u64,
    /// The maps read from the game folder (dbc, maps, Cameras).
    pub maps: u64,
    /// The database filled (the world, the characters, the bots).
    pub db: u64,
    /// Room left over for the saves to grow, logs and back-ups.
    pub margin: u64,
}

const GB: u64 = 1024 * 1024 * 1024;
const MB: u64 = 1024 * 1024;

pub const SIZES: Sizes = Sizes { zip: 400 * MB, unpacked: 2 * GB, maps: 2 * GB, db: 3 * GB, margin: 2 * GB };

impl Sizes {
    /// The release's .json (engine.json beside the zip) over the estimates.
    pub fn from_json(v: Option<&serde_json::Value>) -> Sizes {
        let mut s = SIZES;
        if let Some(v) = v {
            if let Some(z) = v.get("zip_bytes").and_then(|x| x.as_u64()).filter(|z| *z > 0) {
                s.zip = z;
            }
            if let Some(u) = v.get("unpacked_bytes").and_then(|x| x.as_u64()).filter(|u| *u > 0) {
                s.unpacked = u;
            }
        }
        s
    }
}

/// What still has to fit: the parts not done yet, plus the margin. The download and the unpacked engine are both on
/// the disk for a moment (the zip goes once unpacked); an update needs room for the new engine beside the old one.
pub fn need_bytes(h: &engine::Have, z: &Sizes) -> u64 {
    let mut n = z.margin;
    if !h.installed || h.update {
        n += z.zip + z.unpacked;
    }
    if !h.maps {
        n += z.maps;
    }
    if !h.filled {
        n += z.db;
    }
    n
}

pub fn gb(bytes: u64) -> f64 {
    (bytes as f64 / GB as f64 * 10.0).round() / 10.0
}

/// The free space where `path` is (the nearest folder of it that exists), in bytes. None when it can't be told.
pub fn free_bytes(path: &Path) -> Option<u64> {
    let mut at = path.to_path_buf();
    while !at.exists() {
        at = at.parent()?.to_path_buf();
    }
    free_at(&at)
}

#[cfg(windows)]
fn free_at(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: a NUL-terminated path and three out-parameters that live for the call
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) };
    (ok != 0).then_some(avail)
}

#[cfg(not(windows))]
fn free_at(path: &Path) -> Option<u64> {
    let out = std::process::Command::new("df").arg("-Pk").arg(path).output().ok()?;
    parse_df(&String::from_utf8_lossy(&out.stdout))
}

/// `df -Pk`'s free column (KiB) of its one row, in bytes.
pub fn parse_df(text: &str) -> Option<u64> {
    let row = text.lines().nth(1)?;
    row.split_whitespace().nth(3)?.parse::<u64>().ok().map(|k| k * 1024)
}

/// The disk check's answer (`world_preflight`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Preflight {
    pub ok: bool,
    pub free_gb: f64,
    pub need_gb: f64,
    /// Empty when it fits; else the `[disk]` error setup gives.
    pub message: String,
    /// The engine's one-time download in MB (0: nothing to download, it's here already), for the "Your world" panel
    /// (oneclick.js); `download_known`: from the release's .json rather than the estimate.
    pub download_mb: u64,
    pub download_known: bool,
}

/// Fits or not. Free space that can't be told never stops a setup.
pub fn preflight(free: Option<u64>, need: u64, title: &str) -> Preflight {
    match free {
        Some(f) if f < need => Preflight {
            ok: false,
            free_gb: gb(f),
            need_gb: gb(need),
            message: tag("disk", format!(
                "Setting up {title} needs about {} GB free on this drive and it has {} GB, so free some space, then press Play again.",
                gb(need),
                gb(f)
            )),
            download_mb: 0,
            download_known: false,
        },
        f => Preflight { ok: true, free_gb: f.map(gb).unwrap_or(0.0), need_gb: gb(need), message: String::new(), download_mb: 0, download_known: false },
    }
}

// ------------------------------------------------------------------------------------------------ the Hero Key's login

/// The local secret the Hero Key's login names are made with (`<app data>/local-hero.key`, 32 random bytes, made
/// once, never sent anywhere): another PC makes other names for the same account, and the name doesn't give the
/// account away.
pub const SECRET_FILE: &str = "local-hero.key";
/// The signed-in account's id as last read from the website (`me`), so an offline Play keeps the same login.
pub const CACHE_FILE: &str = "local-hero.json";

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 base32, no padding.
pub fn base32(b: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &x in b {
        buf = (buf << 8) | x as u32;
        bits += 8;
        while bits >= 5 {
            out.push(B32[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// The world's login for a Hero Key account: `HK` + the first 14 of base32(HMAC-SHA256(local secret, "leyline-local-
/// world:" + the website's address + ":" + the account's id)). 16 characters, A-Z and 2-7 only: the game's login
/// shape, the same on every Play on this PC, never the same as the world's own login (`SOLO`, `ORIGINAL`) or a
/// friend's (`G`...).
pub fn hero_name(secret: &[u8], site: &str, account: &str) -> String {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret);
    let tag = ring::hmac::sign(&key, format!("leyline-local-world:{site}:{account}").as_bytes());
    format!("HK{}", &base32(tag.as_ref())[..14])
}

/// The local secret, made the first time.
pub fn local_secret(app_dir: &Path) -> Result<Vec<u8>, String> {
    let f = app_dir.join(SECRET_FILE);
    if let Ok(b) = fs::read(&f) {
        if b.len() == 32 {
            return Ok(b);
        }
    }
    use ring::rand::SecureRandom;
    let mut b = vec![0u8; 32];
    ring::rand::SystemRandom::new().fill(&mut b).map_err(|_| "This PC has no random source.".to_string())?;
    fs::create_dir_all(app_dir).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&f, &b).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&f, fs::Permissions::from_mode(0o600));
    }
    Ok(b)
}

/// The account's id in the website's `me` answer (`{"me": {"pid": ...}}`), a number or text.
pub fn account_id(me: &serde_json::Value) -> Option<String> {
    let pid = me.get("me").and_then(|m| m.get("pid"))?;
    let id = match pid {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')).then_some(id)
}

/// The kept id, for this website only.
pub fn cached_id(text: &str, site: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    (v.get("site")?.as_str()? == site).then(|| v.get("id")?.as_str().map(String::from))?
}

/// The signed-in Hero Key's login name for local worlds: None when signed out (the world's own login is used).
/// Asks the website who is signed in (8 s at most); offline, the id kept from the last time.
pub fn hero_identity(app_dir: &Path) -> Option<String> {
    crate::social::load_token(app_dir)?;
    let site = crate::social::base();
    let cache = app_dir.join(CACHE_FILE);
    let id = match crate::social::call(app_dir, "me", &serde_json::json!({})) {
        Ok(v) => {
            let id = account_id(&v)?;
            let _ = crate::settings::write_atomic(&cache, serde_json::json!({ "site": site, "id": id }).to_string().as_bytes());
            id
        }
        Err(e) if e == "signed_out" => {
            let _ = fs::remove_file(&cache);
            return None;
        }
        Err(_) => cached_id(&fs::read_to_string(&cache).ok()?, &site)?,
    };
    let secret = local_secret(app_dir).ok()?;
    Some(hero_name(&secret, &site, &id))
}

/// [`hero_identity`] without the network (a command that can't wait): signed in and the id kept from the last time.
pub fn hero_identity_cached(app_dir: &Path) -> Option<String> {
    crate::social::load_token(app_dir)?;
    let site = crate::social::base();
    let id = cached_id(&fs::read_to_string(app_dir.join(CACHE_FILE)).ok()?, &site)?;
    Some(hero_name(&local_secret(app_dir).ok()?, &site, &id))
}

// ------------------------------------------------------------------------------------------------ stop on exit

/// How long after the game closes the world is stopped.
pub const STOP_AFTER_SECS: u64 = 60;

/// Stop the world now, a minute after its game closed: the setting is on, no Play started a game since, no other
/// game Leyline started is running, the world is still up, and no friend is in it (`friends`: None when the world
/// couldn't be asked: then it stays up).
pub fn stop_after_exit(setting: bool, play_then: u64, play_now: u64, other_game: bool, world_up: bool, friends: Option<u32>) -> bool {
    setting && play_then == play_now && !other_game && world_up && friends == Some(0)
}

// ------------------------------------------------------------------------------------------------ Repair's files

/// engine.json's `files`: each file's path (with `/`) and its size and sha256 (the engine-windows workflow writes
/// it). None for an engine without the list.
pub fn read_manifest(dist: &Path) -> Option<BTreeMap<String, (u64, String)>> {
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(dist.join("engine.json")).ok()?).ok()?;
    let files = v.get("files")?.as_object()?;
    let mut m = BTreeMap::new();
    for (k, e) in files {
        let size = e.get(0).and_then(|x| x.as_u64())?;
        let sha = e.get(1).and_then(|x| x.as_str())?.to_ascii_lowercase();
        if k.is_empty() || k.contains("..") || k.starts_with('/') || k.contains('\\') || sha.len() != 64 {
            return None;
        }
        m.insert(k.clone(), (size, sha));
    }
    (!m.is_empty()).then_some(m)
}

fn rel_path(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(root.to_path_buf(), |p, part| p.join(part))
}

/// The files of the list that are missing or not as listed (the size first, then the sha256).
pub fn manifest_bad(dist: &Path, m: &BTreeMap<String, (u64, String)>) -> Vec<String> {
    m.iter()
        .filter(|(rel, (size, sha))| {
            let f = rel_path(dist, rel);
            match fs::metadata(&f) {
                Ok(md) if md.is_file() && md.len() == *size => crate::net::sha256_file(&f).map(|h| h != *sha).unwrap_or(true),
                _ => true,
            }
        })
        .map(|(rel, _)| rel.clone())
        .collect()
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(x), Ok(y)) if x.is_file() && y.is_file() && x.len() == y.len() => {
            matches!((crate::net::sha256_file(a), crate::net::sha256_file(b)), (Ok(h1), Ok(h2)) if h1 == h2)
        }
        _ => false,
    }
}

/// Every file of a fresh unpack (`from`) that is missing or different in `to` copied over; the rest of `to` (the
/// `.conf` files made from their `.dist`, anything else) stays. How many were put back.
pub fn mend_from(from: &Path, to: &Path) -> Result<usize, String> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, base, out);
                } else if let Ok(r) = p.strip_prefix(base) {
                    out.push(r.to_path_buf());
                }
            }
        }
    }
    let mut files = Vec::new();
    walk(from, from, &mut files);
    let mut n = 0;
    for rel in files {
        let (src, dst) = (from.join(&rel), to.join(&rel));
        if same_file(&src, &dst) {
            continue;
        }
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Couldn't make {}: {e}", parent.display()))?;
        }
        fs::copy(&src, &dst).map_err(|e| format!("Couldn't put back {} (is the world still running?): {e}", rel.display()))?;
        n += 1;
    }
    Ok(n)
}

// ------------------------------------------------------------------------------------------------ resume

/// A world whose setup was started and didn't finish (the launcher closed midway, or a step stopped), set up not yet:
/// the first one, in the Characters page's order. None while a setup is going.
pub fn resume_pending(app_dir: &Path) -> Option<Kind> {
    if engine::progress().running {
        return None;
    }
    engine::KINDS.into_iter().find(|k| {
        let p = engine::Paths::for_kind(app_dir, *k);
        let s = engine::load_state(&p);
        s.setup_pending && !engine::set_up(&engine::have(&p, &s, false))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_starts_with_its_code() {
        assert_eq!(tag("disk", "Not enough room."), "[disk] Not enough room.");
        assert_eq!(tag("db", "[ports] Taken."), "[ports] Taken.", "a coded one keeps its code");
        assert_eq!(tag("nonsense", "x"), "[other] x");
        assert_eq!(code_of("[maps] The maps."), Some("maps"));
        assert_eq!(code_of("[nope] x"), None);
        assert_eq!(code_of("plain"), None);
        assert!(classify(Step::Start, "worldserver.exe isn't in the engine folder: get the engine again.".into()).starts_with("[antivirus] "));
        assert!(classify(Step::Start, "Single player is running on this PC and uses the same ports.".into()).starts_with("[ports] "));
        assert!(classify(Step::Start, "Another server on this PC is using the world port (8085), so it can't start.".into()).starts_with("[ports] "));
        assert!(classify(Step::Download, "The engine didn't download: timed out".into()).starts_with("[download] "));
        assert!(classify(Step::Maps, "The map reader stopped".into()).starts_with("[maps] "));
        assert!(classify(Step::Fill, "Filling the world stopped".into()).starts_with("[db] "));
        assert!(classify(Step::Start, "The database didn't start.".into()).starts_with("[db] "));
        assert!(classify(Step::Account, "The database said no".into()).starts_with("[login] "));
        assert!(classify(Step::Unpack, "write failed: There is not enough space on the disk. (os error 112)".into()).starts_with("[disk] "));
        for c in CODES {
            assert!(c.len() <= 10 && c.bytes().all(|b| b.is_ascii_lowercase()));
        }
    }

    fn fake_game(root: &Path) {
        crate::clients::testutil::fake_335a(root, true);
    }

    #[test]
    fn finds_game_folders_a_few_levels_down_and_nowhere_else() {
        let d = tempfile::tempdir().unwrap();
        let games = d.path().join("Games");
        let a = games.join("My Game");
        fake_game(&a);
        let deep = d.path().join("a").join("b").join("c").join("Game");
        fake_game(&deep);
        // not a game: a Data folder with no program, a program with no Data
        fs::create_dir_all(d.path().join("Stuff").join("Data")).unwrap();
        fs::create_dir_all(d.path().join("Tool")).unwrap();
        fs::write(d.path().join("Tool").join("tool.exe"), b"MZ").unwrap();
        // skipped folders are never looked in
        fake_game(&d.path().join("node_modules").join("g"));
        let found = scan_games(&[d.path().to_path_buf()], 2, 10_000);
        assert_eq!(found, vec![a.clone()], "two levels down only: {found:?}");
        let found = scan_games(&[d.path().to_path_buf()], 4, 10_000);
        assert!(found.contains(&a) && found.contains(&deep));
        assert!(!found.iter().any(|f| f.starts_with(d.path().join("node_modules"))));
        // the same folder through two roots: once
        let found = scan_games(&[d.path().to_path_buf(), games.clone()], 2, 10_000);
        assert_eq!(found.iter().filter(|f| **f == a).count(), 1);
        // the cap stops the look
        assert!(scan_games(&[d.path().to_path_buf()], 4, 1).is_empty());
        assert!(crate::clients::detect(&a).is_ok_and(|x| x.build == crate::clients::Build::V335a));
        assert!(scan_games(&[d.path().join("missing")], 3, 100).is_empty());
    }

    #[test]
    fn the_disk_check_counts_only_whats_left_to_do() {
        let none = engine::Have::default();
        let z = SIZES;
        assert_eq!(need_bytes(&none, &z), z.zip + z.unpacked + z.maps + z.db + z.margin);
        let done = engine::Have { installed: true, maps: true, db: true, filled: true, account: true, ..Default::default() };
        assert_eq!(need_bytes(&done, &z), z.margin, "set up: the margin only");
        let upd = engine::Have { update: true, ..done };
        assert_eq!(need_bytes(&upd, &z), z.margin + z.zip + z.unpacked);
        // the original world shares Single player's engine and maps
        let shared = engine::Have { installed: true, maps: true, ..Default::default() };
        assert_eq!(need_bytes(&shared, &z), z.db + z.margin);
        let j = serde_json::json!({ "zip_bytes": 123, "unpacked_bytes": 456 });
        let from = Sizes::from_json(Some(&j));
        assert_eq!((from.zip, from.unpacked, from.maps), (123, 456, z.maps));
        assert_eq!(Sizes::from_json(Some(&serde_json::json!({ "zip_bytes": 0 }))), SIZES);
        assert_eq!(Sizes::from_json(None), SIZES);
        let short = preflight(Some(3 * GB), 9 * GB, "Single player");
        assert!(!short.ok && short.message.starts_with("[disk] ") && short.message.contains("9 GB") && short.message.contains("3 GB"));
        assert!(short.message.matches(". ").count() == 0, "one sentence: {}", short.message);
        let fits = preflight(Some(20 * GB), 9 * GB, "Single player");
        assert!(fits.ok && fits.message.is_empty() && fits.free_gb == 20.0);
        assert!(preflight(None, 9 * GB, "x").ok, "unknown free space never stops a setup");
        assert_eq!(parse_df("Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/vda 100 50 2048 50% /\n"), Some(2048 * 1024));
        assert_eq!(parse_df(""), None);
        assert!(free_bytes(&std::env::temp_dir().join("no").join("such")).is_some_and(|f| f > 0), "the nearest folder that exists");
    }

    #[test]
    fn the_hero_keys_login_is_stable_private_and_the_games_shape() {
        assert_eq!(base32(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32(b""), "");
        let a = hero_name(&[7u8; 32], "https://site.example", "42");
        assert_eq!(a, hero_name(&[7u8; 32], "https://site.example", "42"), "the same on every Play");
        assert_eq!(a.len(), 16);
        assert!(a.starts_with("HK") && a.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()));
        assert_ne!(a, hero_name(&[8u8; 32], "https://site.example", "42"), "another PC: another name");
        assert_ne!(a, hero_name(&[7u8; 32], "https://site.example", "43"), "another account: another name");
        assert_ne!(a, hero_name(&[7u8; 32], "https://test.example", "42"), "another site: another name");
        assert!(engine::account_sql(&a, "ABCDEFGH2345", &[1u8; 32]).is_ok(), "a login the world takes");
        assert!(!a.contains("42"));
        assert_eq!(account_id(&serde_json::json!({"me": {"pid": 17}})), Some("17".into()));
        assert_eq!(account_id(&serde_json::json!({"me": {"pid": "p_ab-1"}})), Some("p_ab-1".into()));
        assert_eq!(account_id(&serde_json::json!({"me": {"pid": "a b"}})), None);
        assert_eq!(account_id(&serde_json::json!({"me": {}})), None);
        assert_eq!(cached_id(r#"{"site":"https://a","id":"9"}"#, "https://a"), Some("9".into()));
        assert_eq!(cached_id(r#"{"site":"https://a","id":"9"}"#, "https://b"), None, "another site's id is never used");
        let d = tempfile::tempdir().unwrap();
        let s1 = local_secret(d.path()).unwrap();
        assert_eq!(s1.len(), 32);
        assert_eq!(local_secret(d.path()).unwrap(), s1, "made once");
        assert!(hero_identity(d.path()).is_none(), "signed out: the world's own login");
    }

    #[test]
    fn the_world_stops_only_when_nobody_needs_it() {
        assert!(stop_after_exit(true, 3, 3, false, true, Some(0)));
        assert!(!stop_after_exit(false, 3, 3, false, true, Some(0)), "the setting off");
        assert!(!stop_after_exit(true, 3, 4, false, true, Some(0)), "played again since");
        assert!(!stop_after_exit(true, 3, 3, true, true, Some(0)), "another game of ours running");
        assert!(!stop_after_exit(true, 3, 3, false, false, Some(0)), "already stopped");
        assert!(!stop_after_exit(true, 3, 3, false, true, Some(1)), "a friend is in it");
        assert!(!stop_after_exit(true, 3, 3, false, true, None), "couldn't ask: it stays up");
    }

    #[test]
    fn repair_finds_and_puts_back_only_the_broken_files() {
        let d = tempfile::tempdir().unwrap();
        let dist = d.path().join("dist");
        let fresh = d.path().join("fresh");
        for root in [&dist, &fresh] {
            fs::create_dir_all(root.join("bin").join("configs")).unwrap();
            fs::write(root.join("bin").join("worldserver.exe"), b"world v1").unwrap();
            fs::write(root.join("bin").join("authserver.exe"), b"auth v1").unwrap();
            fs::write(root.join("bin").join("configs").join("worldserver.conf.dist"), b"dist").unwrap();
        }
        let sha = |b: &[u8]| {
            use sha2::{Digest, Sha256};
            Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect::<String>()
        };
        let doc = serde_json::json!({ "name": "leyline-engine", "files": {
            "bin/worldserver.exe": [8, sha(b"world v1")],
            "bin/authserver.exe": [7, sha(b"auth v1")],
            "bin/configs/worldserver.conf.dist": [4, sha(b"dist")],
        }});
        fs::write(dist.join("engine.json"), doc.to_string()).unwrap();
        let m = read_manifest(&dist).unwrap();
        assert!(manifest_bad(&dist, &m).is_empty(), "all fine: nothing to fetch");
        // the player's own .conf (made from .dist) is no business of the list
        fs::write(dist.join("bin").join("configs").join("worldserver.conf"), b"mine").unwrap();
        fs::write(dist.join("bin").join("worldserver.exe"), b"world v!").unwrap(); // same size, other bytes
        fs::remove_file(dist.join("bin").join("authserver.exe")).unwrap();
        let bad = manifest_bad(&dist, &m);
        assert_eq!(bad, vec!["bin/authserver.exe".to_string(), "bin/worldserver.exe".to_string()]);
        assert_eq!(mend_from(&fresh, &dist).unwrap(), 2, "only the two");
        assert!(manifest_bad(&dist, &m).is_empty());
        assert_eq!(fs::read(dist.join("bin").join("configs").join("worldserver.conf")).unwrap(), b"mine", "kept");
        assert_eq!(mend_from(&fresh, &dist).unwrap(), 0, "nothing left to put back");
        // a list that doesn't look right is no list
        fs::write(dist.join("engine.json"), r#"{"files": {"../x": [1, "aa"]}}"#).unwrap();
        assert!(read_manifest(&dist).is_none());
        fs::write(dist.join("engine.json"), r#"{"name": "leyline-engine"}"#).unwrap();
        assert!(read_manifest(&dist).is_none(), "today's engines have none: Repair fetches the release and compares");
    }

    #[test]
    fn a_setup_cut_short_is_offered_again() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(resume_pending(d.path()), None);
        let p = engine::Paths::for_kind(d.path(), Kind::Vanilla);
        let mut s = engine::State { setup_pending: true, setup_client: "c335a-1".into(), ..Default::default() };
        engine::save_state(&p, &s).unwrap();
        assert_eq!(resume_pending(d.path()), Some(Kind::Vanilla));
        s.setup_pending = false;
        engine::save_state(&p, &s).unwrap();
        assert_eq!(resume_pending(d.path()), None);
    }
}
