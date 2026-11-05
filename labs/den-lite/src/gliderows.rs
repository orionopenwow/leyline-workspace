//! `den-lite glide-rows --game <game folder> --dens <Dens folder> [--undo]`: the gliding copy's creature rows on this
//! PC now, before a launcher that writes them is out (owner 5 Nov: the den-move said "glide none").
//!
//! The rows are the launcher's own (host/creaturemodels.rs, built into Den Lite: lib.rs `creaturemodels`), from the
//! player's OWN tables (CreatureDisplayInfo, CreatureModelData, GameObjectDisplayInfo read from their own archives;
//! nothing shipped). They go in an archive of their own, [`SLOT`] (`Data/patch-S.MPQ`, marked as ours), not in
//! patch-T: the installed launcher writes patch-T anew at every Den start, and would drop them. The marker
//! (`<dens>/den-creature-models.json`, glide.rs) names that archive, so Den Lite glides only while it is there.
//!
//! * Before writing: our earlier archive and marker are copied to `<dens>/den-glide-backup/`.
//! * Refused (nothing written) when the slot holds someone else's archive, the tables aren't the 3.3.5a shape or
//!   already use our numbers, or an archive the game reads later hides the tables.
//! * `--undo`: our archive and the marker taken away (someone else's archive is never touched).

use crate::{creaturemodels, mpq};
use std::fs;
use std::path::{Path, PathBuf};

/// The rows' own archive in the game's Data folder (read before patch-T, the Den's ground).
pub const SLOT: &str = "patch-S.MPQ";
/// What marks it as ours (inside it).
pub const MARK: &str = "Leyline\\DenGlide.txt";
const MARK_TEXT: &[u8] = b"Leyline: creature rows for Your Den's gliding copy, made on this PC from your own game files. den-lite glide-rows --undo takes them away.\r\n";
/// The launcher's Den archive (host/terrain.rs SLOT): the Den's own gameobject displays are in it.
pub const DEN_SLOT: &str = "patch-T.MPQ";
const GODI: &str = "DBFilesClient\\GameObjectDisplayInfo.dbc";
pub const BACKUP: &str = "den-glide-backup";
/// Ours holds two tables; anything much bigger is someone else's.
const OURS_MAX: u64 = 64 * 1024 * 1024;

/// A name in `dir` without regard to case (the launcher's clients.rs `find_ci`).
pub fn find_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    let mut ci = None;
    for e in fs::read_dir(dir).ok()?.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n == name {
            return Some(e.path());
        }
        if ci.is_none() && n.to_ascii_lowercase() == want {
            ci = Some(e.path());
        }
    }
    ci
}

fn data_dir(game: &Path) -> Result<PathBuf, String> {
    find_ci(game, "Data").filter(|d| d.is_dir()).ok_or_else(|| format!("{} has no Data folder.", game.display()))
}

/// The game's own archives in the order it reads them (the launcher's host/terrain.rs `archive_order`): the base
/// archives, the locale's, `patch`, `patch-<digit or A..S>`, the locale's patches. Never ours ([`SLOT`]) or the Den's
/// ([`DEN_SLOT`]): the player's own tables are read from these.
pub fn archive_order(game: &Path) -> Vec<PathBuf> {
    let Ok(data) = data_dir(game) else { return Vec::new() };
    let mut names: Vec<(String, PathBuf)> = Vec::new();
    let mut locale: Option<PathBuf> = None;
    if let Ok(rd) = fs::read_dir(&data) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            let p = e.path();
            if p.is_dir() && n.len() == 4 && n.chars().all(|c| c.is_ascii_alphabetic()) && locale.is_none() {
                locale = Some(p);
            } else if p.is_file() && n.to_ascii_lowercase().ends_with(".mpq") {
                names.push((n.to_ascii_lowercase(), p));
            }
        }
    }
    let rank = |n: &str| -> Option<(u32, String)> {
        if n == SLOT.to_ascii_lowercase() || n == DEN_SLOT.to_ascii_lowercase() {
            return None;
        }
        let stem = n.strip_suffix(".mpq")?;
        Some(match stem {
            "common" => (0, String::new()),
            "common-2" => (1, String::new()),
            "expansion" => (2, String::new()),
            "lichking" => (3, String::new()),
            "patch" => (20, String::new()),
            s => {
                let x = s.strip_prefix("patch-")?;
                let mut it = x.chars();
                let c = it.next()?.to_ascii_uppercase();
                if it.next().is_some() || !(c.is_ascii_digit() || ('A'..='S').contains(&c)) {
                    return None;
                }
                (21, c.to_string())
            }
        })
    };
    let mut base: Vec<(u32, String, PathBuf)> = names.into_iter().filter_map(|(n, p)| rank(&n).map(|(r, k)| (r, k, p))).collect();
    if let Some(l) = locale {
        let loc = l.file_name().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if let Ok(rd) = fs::read_dir(&l) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                let r = if n == format!("locale-{loc}.mpq") {
                    Some((10, String::new()))
                } else if n == format!("expansion-locale-{loc}.mpq") {
                    Some((11, String::new()))
                } else if n == format!("lichking-locale-{loc}.mpq") {
                    Some((12, String::new()))
                } else if n == format!("patch-{loc}.mpq") {
                    Some((30, "1".into()))
                } else if n == format!("patch-{loc}-2.mpq") {
                    Some((30, "2".into()))
                } else if n == format!("patch-{loc}-3.mpq") {
                    Some((30, "3".into()))
                } else {
                    None
                };
                if let Some((r, k)) = r {
                    base.push((r, k, e.path()));
                }
            }
        }
    }
    base.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    base.into_iter().map(|x| x.2).collect()
}

/// The archives the game reads after ours that could hide our tables: patch-T and patch-U ..= patch-Z.
fn after_slot(game: &Path) -> Vec<PathBuf> {
    let Ok(data) = data_dir(game) else { return Vec::new() };
    let mut v: Vec<PathBuf> = fs::read_dir(&data)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            n.len() == "patch-t.mpq".len() && n.starts_with("patch-") && n.ends_with(".mpq") && (b't'..=b'z').contains(&n.as_bytes()[6])
        })
        .map(|e| e.path())
        .collect();
    v.sort();
    v
}

/// The slot's path, and whether ours is there. Err: someone else's archive has it.
pub fn slot(game: &Path) -> Result<(PathBuf, bool), String> {
    let data = data_dir(game)?;
    let Some(p) = find_ci(&data, SLOT) else { return Ok((data.join(SLOT), false)) };
    let len = fs::metadata(&p).map(|m| m.len()).unwrap_or(u64::MAX);
    if len <= OURS_MAX && fs::read(&p).ok().is_some_and(|b| mpq::read(&b, MARK).is_some()) {
        Ok((p, true))
    } else {
        Err(format!("Data/{SLOT} belongs to another mod, so Leyline leaves it alone (the gliding copy needs that slot)."))
    }
}

/// What was done, for the Control step's log and den-check (one line of JSON).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Done {
    pub ok: bool,
    pub said: String,
    pub archive: String,
    pub displays: usize,
    pub backup: Option<String>,
}

fn copy_aside(dens: &Path, from: &Path, name: &str) -> Option<String> {
    if !from.is_file() {
        return None;
    }
    let dir = dens.join(BACKUP);
    fs::create_dir_all(&dir).ok()?;
    let to = dir.join(name);
    fs::copy(from, &to).ok()?;
    Some(to.display().to_string())
}

/// The rows into [`SLOT`] and the marker into `dens` (see the module's note).
pub fn prepare(game: &Path, dens: &Path) -> Result<Done, String> {
    let (path, _ours) = slot(game)?;
    let order = archive_order(game);
    if order.is_empty() {
        return Err(format!("{} has no game archives in Data.", game.display()));
    }
    // the gameobject displays: the Den's patch-T (the player's own with the Den's models: host/denmodels.rs) when it
    // has them, else the game's own
    let den_t = data_dir(game).ok().and_then(|d| find_ci(&d, DEN_SLOT));
    let godi = den_t
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| mpq::read(&b, GODI))
        .or_else(|| mpq::read_own(&order, GODI).ok().flatten())
        .ok_or("your game has no GameObjectDisplayInfo")?;
    let (files, displays) = creaturemodels::patch_files(&order, &godi)?;
    // an archive read after ours with these tables would hide them: then nothing is written
    for p in after_slot(game) {
        let Ok(mut a) = mpq::GameArchive::open(&p) else { continue };
        for (name, _) in &files {
            if a.read(name).ok().flatten().is_some() {
                return Err(format!("{} (read after ours) has {}: it would hide the rows, so nothing was written.", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), name));
            }
        }
    }
    let backup = copy_aside(dens, &path, SLOT);
    let _ = copy_aside(dens, &dens.join(creaturemodels::MARKER), creaturemodels::MARKER);
    let mut all = files;
    all.push((MARK.to_string(), MARK_TEXT.to_vec()));
    let bytes = mpq::build(&all)?;
    crate::settings::write_atomic(&path, &bytes).map_err(|e| format!("Couldn't write Data/{SLOT} (is the game still running?): {e}"))?;
    // the marker: the launcher's, with the archive it needs
    let mut v: serde_json::Value = serde_json::from_str(&creaturemodels::marker(&displays)).map_err(|e| e.to_string())?;
    v["archive"] = serde_json::json!(path.display().to_string());
    v["by"] = serde_json::json!("den-lite glide-rows");
    crate::settings::write_atomic(&dens.join(creaturemodels::MARKER), v.to_string().as_bytes()).map_err(|e| format!("Couldn't write the marker: {e}"))?;
    Ok(Done { ok: true, said: format!("{} prop models can glide (Data/{SLOT}, from your own game's tables).", displays.len()), archive: path.display().to_string(), displays: displays.len(), backup })
}

/// `--undo`: our archive and the marker away.
pub fn undo(game: &Path, dens: &Path) -> Result<Done, String> {
    let (path, ours) = slot(game)?;
    if ours {
        let _ = copy_aside(dens, &path, SLOT);
        fs::remove_file(&path).map_err(|e| format!("Couldn't take Data/{SLOT} away (is the game still running?): {e}"))?;
    }
    let m = dens.join(creaturemodels::MARKER);
    let was = fs::read_to_string(&m).unwrap_or_default();
    // only a marker that points at our archive (the launcher's own marker stays: its rows are in patch-T)
    if was.contains("den-lite glide-rows") {
        let _ = fs::remove_file(&m);
    }
    Ok(Done { ok: true, said: if ours { format!("Data/{SLOT} taken away: the copy is a gameobject again.") } else { "Nothing to take away.".into() }, archive: path.display().to_string(), displays: 0, backup: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use leyline_terrain::dbc::Dbc;

    fn table(fields: u32, rows: &[Vec<u32>], strings: &[u8]) -> Vec<u8> {
        Dbc {
            fields,
            record_size: fields * 4,
            records: rows
                .iter()
                .map(|r| {
                    let mut v = vec![0u8; fields as usize * 4];
                    for (i, x) in r.iter().enumerate() {
                        Dbc::set(&mut v, i, *x);
                    }
                    v
                })
                .collect(),
            strings: if strings.is_empty() { vec![0] } else { strings.to_vec() },
        }
        .write()
    }

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("den-lite-gliderows-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("game/Data")).unwrap();
        fs::create_dir_all(p.join("dens")).unwrap();
        p
    }

    /// A game whose patch-3 has the three tables (a chair .mdx and an inn .wmo).
    fn game(root: &Path) {
        let godi = table(19, &[vec![39, 1], vec![9007, 27]], b"\0World\\Generic\\Chair01.mdx\0World\\wmo\\Inn.wmo\0");
        let cdi = table(16, &[vec![1, 1]], &[0]);
        let cmd = table(28, &[vec![1, 0, 1]], b"\0Creature\\Bear\\Bear.mdx\0");
        let a = mpq::build(&[(GODI.to_string(), godi), (creaturemodels::DISPLAY_TABLE.into(), cdi), (creaturemodels::MODEL_TABLE.into(), cmd)]).unwrap();
        fs::write(root.join("game/Data/patch-3.MPQ"), a).unwrap();
    }

    #[test]
    fn the_rows_go_in_their_own_archive_with_the_launchers_rows_and_undo_takes_them_away() {
        let r = tmp("rows");
        game(&r);
        let (g, dens) = (r.join("game"), r.join("dens"));
        let done = prepare(&g, &dens).unwrap();
        assert_eq!(done.displays, 1, "the chair, not the inn");
        let a = fs::read(g.join("Data").join(SLOT)).unwrap();
        assert!(mpq::read(&a, MARK).is_some());
        let d = Dbc::parse(&mpq::read(&a, creaturemodels::DISPLAY_TABLE).unwrap()).unwrap();
        assert!(d.row(1).is_some() && d.row(70_039).is_some() && d.row(79_007).is_none());
        // exactly the launcher's tables (one source)
        let order = archive_order(&g);
        let godi = mpq::read_own(&order, GODI).unwrap().unwrap();
        let (want, _) = creaturemodels::patch_files(&order, &godi).unwrap();
        for (name, bytes) in &want {
            assert_eq!(&mpq::read(&a, name).unwrap(), bytes, "{name}");
        }
        // Den Lite glides those displays while the archive is there
        let set = crate::glide::read(&dens).unwrap();
        assert!(set.contains(&39) && !set.contains(&9007));
        // again: ours is backed up first and written again
        let again = prepare(&g, &dens).unwrap();
        assert!(again.backup.is_some_and(|b| Path::new(&b).is_file()));
        // undo: the archive and our marker gone; Den Lite's copy a gameobject again
        undo(&g, &dens).unwrap();
        assert!(!g.join("Data").join(SLOT).exists() && crate::glide::read(&dens).is_none());
        // the archive gone by hand: the marker alone glides nothing
        prepare(&g, &dens).unwrap();
        fs::remove_file(g.join("Data").join(SLOT)).unwrap();
        assert!(crate::glide::read(&dens).is_none());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn someone_elses_archive_or_a_later_one_with_the_tables_is_left_alone() {
        let r = tmp("other");
        game(&r);
        let (g, dens) = (r.join("game"), r.join("dens"));
        fs::write(g.join("Data").join(SLOT), mpq::build(&[("x\\y.txt".into(), b"theirs".to_vec())]).unwrap()).unwrap();
        assert!(prepare(&g, &dens).unwrap_err().contains("another mod"));
        assert!(undo(&g, &dens).is_err());
        assert_eq!(mpq::read(&fs::read(g.join("Data").join(SLOT)).unwrap(), "x\\y.txt").unwrap(), b"theirs");
        fs::remove_file(g.join("Data").join(SLOT)).unwrap();
        // patch-Z (read after ours) with a creature table: it would hide ours
        fs::write(g.join("Data/patch-Z.MPQ"), mpq::build(&[(creaturemodels::MODEL_TABLE.into(), table(28, &[vec![1]], &[]))]).unwrap()).unwrap();
        assert!(prepare(&g, &dens).unwrap_err().contains("hide"));
        assert!(!g.join("Data").join(SLOT).exists() && crate::glide::read(&dens).is_none());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn the_games_own_order_never_has_ours_or_the_dens() {
        let r = tmp("order");
        for n in ["common.MPQ", "patch.MPQ", "patch-2.MPQ", "patch-S.MPQ", "patch-T.MPQ", "patch-A.MPQ"] {
            fs::write(r.join("game/Data").join(n), b"x").unwrap();
        }
        let names: Vec<String> = archive_order(&r.join("game")).iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["common.MPQ", "patch.MPQ", "patch-2.MPQ", "patch-A.MPQ"]);
        let _ = fs::remove_dir_all(&r);
    }
}
