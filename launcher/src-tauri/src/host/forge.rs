//! Forge, the Den's terrain editor (owner, 1 Nov: "editor's name is Forge"; docs/NOGGIT-DEN-PLAN.md section 1): a
//! renamed Noggit Red build, GPL-3.0, kept as its own program. The launcher (MIT) never links it: it downloads the
//! release, checks its sha256, unpacks it into `<app dir>/forge/`, starts it with the Den's workspace and reads the
//! files it leaves (host/terrain.rs). That is two programs side by side, which the GPL allows.
//!
//! * **Get Forge**: the `forge-latest` release asset `Forge.zip` of the Leyline workspace repo and its
//!   `Forge.zip.sha256`, GitHub only (the upstream downloader's hosts), at most 1 GB, unpacked with the client
//!   importer's careful rules (no `..`, no links, caps). The page shows the GPL notice and the source link first.
//! * **Open in Forge**: one at a time; when it closes, [`after_close`] runs [`terrain::import`], then patch-T and the
//!   Docker Den's tile again, and keeps a line for the page ("Terrain updated").

use crate::host::terrain;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub const ZIP_URL: &str = "https://github.com/orionopenwow/leyline-workspace/releases/download/forge-latest/Forge.zip";
pub const SHA_URL: &str = "https://github.com/orionopenwow/leyline-workspace/releases/download/forge-latest/Forge.zip.sha256";
/// Where its source is (GPL-3.0 section 6: the source next to the program).
pub const SOURCE_URL: &str = "https://github.com/orionopenwow/leyline-workspace/tree/main/forge";
pub const LICENCE: &str = "Forge is free software under the GNU GPL-3.0, built on Noggit Red by the Noggit contributors. Leyline downloads it from its public release and starts it as its own program.";
const ZIP_MAX: u64 = 1024 * 1024 * 1024;
const CAPS: crate::clientimport::Caps = crate::clientimport::Caps { download: ZIP_MAX, unpacked: 4 * 1024 * 1024 * 1024, files: 50_000 };

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Event {
    /// Goes up by one each time Forge closes (the page shows "Terrain updated" once per new one).
    pub seq: u64,
    pub ok: bool,
    pub changed: bool,
    pub message: String,
    pub chunks: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ForgeState {
    pub installed: bool,
    pub running: bool,
    pub exe: String,
    pub sha256: String,
    pub last: Option<Event>,
    pub source: &'static str,
    pub licence: &'static str,
}

pub fn dir(app: &Path) -> PathBuf {
    app.join("forge")
}

fn exe_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["Forge.exe", "forge.exe"]
    } else {
        &["Forge", "forge", "Forge.exe"]
    }
}

/// The program inside an unpacked folder: at its top, or one folder down (a zip with a top folder).
pub fn find_exe(root: &Path) -> Option<PathBuf> {
    let here = |d: &Path| exe_names().iter().map(|n| d.join(n)).find(|p| p.is_file());
    if let Some(p) = here(root) {
        return Some(p);
    }
    let mut subs: Vec<PathBuf> = fs::read_dir(root).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    subs.sort();
    subs.iter().find_map(|d| here(d))
}

static CHILD: Mutex<Option<Child>> = Mutex::new(None);
static LAST: Mutex<Option<Event>> = Mutex::new(None);
static SEQ: AtomicU64 = AtomicU64::new(0);

pub fn running() -> bool {
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

pub fn state(app: &Path) -> ForgeState {
    let d = dir(app);
    let exe = find_exe(&d);
    let sha = fs::read_to_string(d.join("leyline-forge.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("sha256").and_then(|s| s.as_str()).map(str::to_string))
        .unwrap_or_default();
    ForgeState {
        installed: exe.is_some(),
        running: running(),
        exe: exe.map(|p| p.display().to_string()).unwrap_or_default(),
        sha256: sha,
        last: LAST.lock().ok().and_then(|g| g.clone()),
        source: SOURCE_URL,
        licence: LICENCE,
    }
}

/// The sha256 a `.sha256` file names: its first 64 hex digits (`<hex>  Forge.zip` or the hex alone).
pub fn sha_in(text: &str) -> Option<String> {
    let t = text.split_whitespace().next()?.to_ascii_lowercase();
    (t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit())).then_some(t)
}

/// Unpacks a checked zip into `<app>/forge/` (the old one replaced only once the new one is whole).
pub fn install_zip(app: &Path, zip: &Path, sha: &str) -> Result<PathBuf, String> {
    let d = dir(app);
    let tmp = app.join("forge.new");
    let _ = fs::remove_dir_all(&tmp);
    crate::clientimport::unzip(zip, &tmp, &CAPS, &crate::clientimport::Job::default()).map_err(|e| format!("Forge's download couldn't be unpacked: {e}"))?;
    if find_exe(&tmp).is_none() {
        let _ = fs::remove_dir_all(&tmp);
        return Err("Forge's download has no Forge program in it, so nothing was installed.".into());
    }
    let meta = serde_json::json!({ "sha256": sha, "from": ZIP_URL, "licence": "GPL-3.0", "source": SOURCE_URL });
    fs::write(tmp.join("leyline-forge.json"), serde_json::to_string_pretty(&meta).unwrap()).map_err(|e| e.to_string())?;
    if d.exists() {
        fs::remove_dir_all(&d).map_err(|e| format!("Couldn't replace the old Forge (is it open?): {e}"))?;
    }
    fs::rename(&tmp, &d).map_err(|e| format!("Couldn't put Forge in place: {e}"))?;
    #[cfg(unix)]
    if let Some(exe) = find_exe(&d) {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&exe, fs::Permissions::from_mode(0o755));
    }
    find_exe(&d).ok_or_else(|| "Forge isn't where it was unpacked.".into())
}

/// Get Forge: the release zip and its checksum, checked, then unpacked.
pub fn install(app: &Path) -> Result<String, String> {
    if running() {
        return Err("Close Forge first, then get it again.".into());
    }
    let scratch = app.join("tmp");
    fs::create_dir_all(&scratch).map_err(|e| format!("Couldn't make a download folder: {e}"))?;
    let shaf = scratch.join("Forge.zip.sha256");
    crate::upstream::download(SHA_URL, &shaf, 4096).map_err(|e| format!("Couldn't get Forge's checksum: {e}"))?;
    let want = fs::read_to_string(&shaf).ok().as_deref().and_then(sha_in).ok_or("Forge's checksum file doesn't look right, so nothing was downloaded.")?;
    let _ = fs::remove_file(&shaf);
    let zip = scratch.join("Forge.zip");
    let got = crate::upstream::download(ZIP_URL, &zip, ZIP_MAX).map_err(|e| format!("Forge didn't download: {e}"));
    let res = got.and_then(|(_, sha)| {
        if sha != want {
            return Err("Forge's download doesn't match its checksum, so nothing was installed. Try again later.".to_string());
        }
        install_zip(app, &zip, &sha)
    });
    let _ = fs::remove_file(&zip);
    res.map(|p| p.display().to_string())
}

fn set_last(ok: bool, changed: bool, message: String, chunks: usize) {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut g) = LAST.lock() {
        *g = Some(Event { seq, ok, changed, message, chunks });
    }
}

/// What happens once Forge has closed: its work into the ground, then patch-T and the Docker Den's tile.
pub fn after_close(client: &Path, den_root: &Path, docker_kit: Option<&Path>, ws: &terrain::Workspace) -> Event {
    match terrain::import(client, den_root, ws) {
        Err(e) => set_last(false, false, e, 0),
        Ok(r) if !r.changed => set_last(true, false, "Forge closed. Nothing changed in your Den's ground.".into(), 0),
        Ok(r) => {
            let mut line = format!("Terrain updated: {} part{} of your Den changed{}.", r.chunks, if r.chunks == 1 { "" } else { "s" }, if r.models { ", and its models" } else { "" });
            if r.ignored > 0 {
                line.push_str(&format!(" {} change{} outside your Den's ground were left out.", r.ignored, if r.ignored == 1 { "" } else { "s" }));
            }
            if !r.other_tiles.is_empty() {
                line.push_str(" Only your Den's ground can be saved: other tiles were left out.");
            }
            let notes = terrain::after_change(client, den_root, docker_kit);
            for n in &notes {
                line.push(' ');
                line.push_str(n);
            }
            line.push_str(" Start your game again to see it.");
            set_last(true, true, line, r.chunks);
        }
    }
    LAST.lock().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// Open in Forge: the workspace made, Forge started on it, and a watcher that runs [`after_close`] when it closes.
pub fn open(app: &Path, client: &Path, den_root: &Path, docker_kit: Option<PathBuf>) -> Result<String, String> {
    if running() {
        return Err("Forge is already open.".into());
    }
    let exe = find_exe(&dir(app)).ok_or("Forge isn't on this PC yet: Get Forge first.")?;
    let (ws, d) = terrain::workspace(app, client, den_root)?;
    let child = Command::new(&exe)
        .args(terrain::forge_args(client, &ws, &d))
        .current_dir(exe.parent().unwrap_or(app))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Forge didn't start: {e}"))?;
    if let Ok(mut g) = CHILD.lock() {
        *g = Some(child);
    }
    let (client, den_root) = (client.to_path_buf(), den_root.to_path_buf());
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if !running() {
                break;
            }
        }
        after_close(&client, &den_root, docker_kit.as_deref(), &ws);
    });
    Ok("Forge is open on your Den. Shape the ground, save (Ctrl+S), then close Forge: Leyline picks up your changes.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::terrain::tests::game;

    #[test]
    fn checksums_and_where_the_program_is() {
        assert_eq!(sha_in(&format!("{}  Forge.zip\n", "AB".repeat(32))).as_deref(), Some("ab".repeat(32).as_str()));
        assert_eq!(sha_in(&"0".repeat(64)).as_deref(), Some("0".repeat(64).as_str()));
        for bad in ["", "abc", &"g".repeat(64), &"0".repeat(63)] {
            assert_eq!(sha_in(bad), None, "{bad}");
        }
        let d = tempfile::tempdir().unwrap();
        assert_eq!(find_exe(d.path()), None);
        fs::create_dir_all(d.path().join("Forge-1.0")).unwrap();
        fs::write(d.path().join("Forge-1.0").join(exe_names()[0]), b"x").unwrap();
        assert_eq!(find_exe(d.path()), Some(d.path().join("Forge-1.0").join(exe_names()[0])));
        fs::write(d.path().join(exe_names()[0]), b"x").unwrap();
        assert_eq!(find_exe(d.path()), Some(d.path().join(exe_names()[0])), "the top first");
    }

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (n, b) in files {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut w, b).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn a_checked_zip_is_installed_and_a_bad_one_is_not() {
        let app = tempfile::tempdir().unwrap();
        let z = app.path().join("f.zip");
        fs::write(&z, zip_of(&[("Forge/COPYING", b"GPL"), ("Forge/readme.txt", b"hi")])).unwrap();
        assert!(install_zip(app.path(), &z, "x").unwrap_err().contains("no Forge program"));
        assert!(!dir(app.path()).exists());
        fs::write(&z, zip_of(&[("../evil", b"x")])).unwrap();
        assert!(install_zip(app.path(), &z, "x").is_err());
        let name = format!("Forge/{}", exe_names()[0]);
        fs::write(&z, zip_of(&[(name.as_str(), b"#!/bin/sh\n"), ("Forge/COPYING", b"GPL")])).unwrap();
        let exe = install_zip(app.path(), &z, "abc").unwrap();
        assert!(exe.starts_with(dir(app.path())));
        let s = state(app.path());
        assert!(s.installed && !s.running && s.sha256 == "abc");
        assert!(s.licence.contains("GPL-3.0") && s.source.starts_with("https://github.com/"));
        // again: replaced whole
        install_zip(app.path(), &z, "def").unwrap();
        assert_eq!(state(app.path()).sha256, "def");
    }

    /// Forge's whole round, with a stand-in program (a shell script) that raises the pad by 5 yards and saves.
    #[cfg(unix)]
    #[test]
    fn open_runs_forge_on_the_workspace_and_its_save_becomes_the_ground() {
        use leyline_terrain::adt::Adt;
        use std::os::unix::fs::PermissionsExt;
        let g = game();
        let app = tempfile::tempdir().unwrap();
        let dens = tempfile::tempdir().unwrap();
        // the stand-in reads --project and writes a prepared tile there (what a save in Forge does)
        let (ws, d) = terrain::workspace(app.path(), g.path(), dens.path()).unwrap();
        let mut t = Adt::parse(&fs::read(&ws.adt).unwrap(), None).unwrap();
        for i in d.open.iter() {
            t.chunks[*i as usize].heights.iter_mut().for_each(|h| *h += 5.0);
        }
        let prepared = app.path().join("prepared.adt");
        fs::write(&prepared, t.write()).unwrap();
        fs::create_dir_all(dir(app.path())).unwrap();
        let exe = dir(app.path()).join(exe_names()[0]);
        fs::write(&exe, format!("#!/bin/sh\nwhile [ \"$1\" != \"--project\" ]; do shift; done\necho \"$@\" > \"{}/args.txt\"\ncp \"{}\" \"$2/World/Maps/Kalimdor/Kalimdor_1_1.adt\"\n", app.path().display(), prepared.display())).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        let line = open(app.path(), g.path(), dens.path(), None).unwrap();
        assert!(line.contains("Forge is open"));
        let mut ev = None;
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Some(e) = state(app.path()).last.filter(|e| e.changed) {
                ev = Some(e);
                break;
            }
        }
        let ev = ev.expect("Forge closed and the ground was taken");
        assert!(ev.ok && ev.message.starts_with("Terrain updated: 4 parts"), "{}", ev.message);
        let args = fs::read_to_string(app.path().join("args.txt")).unwrap();
        assert!(args.contains("--map 1 --tile 1,1 --locked-tiles") && args.trim_end().ends_with("--name yourden --origin 16226.2,16257,13.2"), "{args}");
        let ground = leyline_terrain::pad::Ground::from_delta(&terrain::load(dens.path()).unwrap().unwrap()).unwrap();
        let z = ground.height_at(leyline_terrain::pad::DEN.0, leyline_terrain::pad::DEN.1).unwrap();
        assert!((z - (leyline_terrain::pad::DEN.2 + 5.0)).abs() < 0.01, "{z}");
        assert!(g.path().join("Data").join(terrain::SLOT).is_file(), "patch-T made again");
    }
}
