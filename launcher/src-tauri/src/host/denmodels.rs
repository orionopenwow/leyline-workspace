//! Your Den's "My models": the player's own model files dropped on the Your Den card (owner 3 Nov: "drag and drop
//! new 3d models"), kept on this PC and put in the Den's own patch.
//!
//! * The files are checked and renamed under `World\Leyline\Den\<slug>\` by [`super::denfiles`] and kept in
//!   `<dens>/my-models/<slug>/`.
//! * `<dens>/my-models.json` lists them: each one's gameobject entry (980001...) and display id (60001...), which
//!   Den Lite reads at its start (labs/den-lite `mymodels.rs`: a template each, the Builder's "My models").
//! * When a Den starts, [`patch_files`] gives host/terrain.rs the files and the player's own
//!   `GameObjectDisplayInfo.dbc` with a row per model, for `Data/patch-T.MPQ` (taken out whenever the player plays
//!   anywhere else), and host/lite.rs copies the same table to the Dens folder for the gallery.
//!
//! Nothing is uploaded or sent to friends (P2P never carries game files): a friend sees a stand-in where a model is.

use super::denfiles::{self, ModelKind};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const INDEX: &str = "my-models.json";
pub const FOLDER: &str = "my-models";
pub const FORMAT: &str = "leyline-den-models";
pub const ENTRY_BASE: u32 = 980_000;
/// labs/den-lite mymodels.rs MAX.
pub const MAX_MODELS: usize = 200;
pub const TABLE: &str = "DBFilesClient\\GameObjectDisplayInfo.dbc";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub entry: u32,
    pub display: u32,
    pub name: String,
    pub slug: String,
    /// "m2" or "wmo".
    pub kind: String,
    /// The path the table names.
    pub model: String,
    #[serde(rename = "box")]
    pub bbox: [f32; 6],
    /// Archive paths in the patch (the file of each is `<dens>/my-models/<slug>/<last part>`).
    pub files: Vec<String>,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub unused: Vec<String>,
    #[serde(default)]
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Index {
    format: String,
    version: u32,
    models: Vec<Model>,
}

/// The list in `<dens>/my-models.json` (none: empty).
pub fn list(dens: &Path) -> Vec<Model> {
    fs::read_to_string(dens.join(INDEX))
        .ok()
        .and_then(|t| serde_json::from_str::<Index>(t.trim_start_matches('\u{feff}')).ok())
        .filter(|i| i.format == FORMAT)
        .map(|i| i.models.into_iter().filter(|m| denfiles::slug(&m.slug) == m.slug && m.files.iter().all(|f| f.starts_with(denfiles::DEN_PATH))).collect())
        .unwrap_or_default()
}

fn write_list(dens: &Path, models: &[Model]) -> Result<(), String> {
    let i = Index { format: FORMAT.into(), version: 1, models: models.to_vec() };
    let t = serde_json::to_string_pretty(&i).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&dens.join(INDEX), t.as_bytes()).map_err(|e| format!("Couldn't keep the model list: {e}"))
}

fn folder(dens: &Path, slug: &str) -> PathBuf {
    dens.join(FOLDER).join(slug)
}

fn last(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

/// A drop added (or put in place of the model with the same name): checked, kept, listed. The name shown is
/// `label` (or the model file's), up to 32 letters.
pub fn add(dens: &Path, files: &[(String, Vec<u8>)], label: Option<&str>) -> Result<Model, String> {
    let p = denfiles::prepare(files, label)?;
    let mut models = list(dens);
    let old = models.iter().position(|m| m.slug == p.slug);
    if old.is_none() && models.len() >= MAX_MODELS {
        return Err(format!("Your Den holds up to {MAX_MODELS} of your own models: remove one first."));
    }
    let entry = old.map(|i| models[i].entry).unwrap_or_else(|| (ENTRY_BASE + 1..).find(|e| !models.iter().any(|m| m.entry == *e)).unwrap_or(ENTRY_BASE + 1));
    let display = old.map(|i| models[i].display).unwrap_or_else(|| (denfiles::DISPLAY_BASE + 1..).find(|d| !models.iter().any(|m| m.display == *d)).unwrap_or(denfiles::DISPLAY_BASE + 1));
    let dir = folder(dens, &p.slug);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the model's folder: {e}"))?;
    for (path, bytes) in &p.files {
        fs::write(dir.join(last(path)), bytes).map_err(|e| format!("Couldn't keep {}: {e}", last(path)))?;
    }
    let name: String = label.filter(|l| !l.trim().is_empty()).unwrap_or(&p.slug).chars().filter(|c| c.is_ascii_alphanumeric() || " -_".contains(*c)).take(32).collect();
    let m = Model {
        entry,
        display,
        name: if name.trim().is_empty() { p.slug.clone() } else { name.trim().to_string() },
        slug: p.slug.clone(),
        kind: if p.kind == ModelKind::M2 { "m2" } else { "wmo" }.into(),
        model: p.model_path.clone(),
        bbox: p.bbox,
        files: p.files.iter().map(|f| f.0.clone()).collect(),
        missing: p.missing.clone(),
        unused: p.unused.clone(),
        bytes: p.files.iter().map(|f| f.1.len() as u64).sum(),
    };
    match old {
        Some(i) => models[i] = m.clone(),
        None => models.push(m.clone()),
    }
    write_list(dens, &models)?;
    Ok(m)
}

/// A model taken away (its files too). Ok(false): there was none by that name.
pub fn remove(dens: &Path, slug: &str) -> Result<bool, String> {
    let mut models = list(dens);
    let before = models.len();
    models.retain(|m| m.slug != slug);
    if models.len() == before {
        return Ok(false);
    }
    write_list(dens, &models)?;
    let _ = fs::remove_dir_all(folder(dens, &denfiles::slug(slug)));
    Ok(true)
}

/// The player's own table with a row per model (None: no models, or the table isn't one Leyline knows; the patch then
/// leaves the table alone).
pub fn table(dens: &Path, stock: &[u8]) -> Option<Vec<u8>> {
    let models = list(dens);
    let places = place_rows(dens);
    if models.is_empty() && places.is_empty() {
        return None;
    }
    let mine: Vec<(u32, String, [f32; 6])> = models.iter().map(|m| (m.display, m.model.clone(), m.bbox)).collect();
    let mut rows = mine.clone();
    rows.extend(places.into_iter().filter(|r| !mine.iter().any(|m| m.0 == r.0)));
    // a bad place row never costs the player's own models their rows
    denfiles::table_with(stock, &rows).ok().or_else(|| (!mine.is_empty()).then(|| denfiles::table_with(stock, &mine).ok()).flatten())
}

/// `<dens>/place-models.json` (tools/den-places den_places.py `--add-models`, made on this PC from the player's own
/// game files): a display row for a model of their own game that their table has none for, so a place rebuilt in
/// the Den (`.den load <place>`) shows every piece. Each row only names a file the player's game already has (no file
/// goes in the patch for it). Display ids 62001 to 69999; paths as models.rs `clean` takes them, `.wmo` too.
pub const PLACE_ROWS: &str = "place-models.json";
pub const PLACE_ROWS_FORMAT: &str = "leyline-den-place-models";
pub const PLACE_IDS: std::ops::RangeInclusive<u32> = 62_001..=69_999;
/// At most this many (a Den holds 800 props; several places).
pub const MAX_PLACE_ROWS: usize = 4000;

pub fn place_rows(dens: &Path) -> Vec<(u32, String, [f32; 6])> {
    let Ok(t) = fs::read_to_string(dens.join(PLACE_ROWS)) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')) else { return Vec::new() };
    if v["format"] != PLACE_ROWS_FORMAT {
        return Vec::new();
    }
    let ok_path = |p: &str| {
        !p.is_empty()
            && p.len() <= 200
            && !p.contains("..")
            && !p.starts_with('\\')
            && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '\\' | '_' | '-' | '.' | ' ' | '\''))
            && [".m2", ".mdx", ".mdl", ".wmo"].iter().any(|e| p.to_ascii_lowercase().ends_with(e))
    };
    let mut out: Vec<(u32, String, [f32; 6])> = Vec::new();
    for r in v["models"].as_array().map(Vec::as_slice).unwrap_or(&[]).iter().take(MAX_PLACE_ROWS) {
        let id = r["display"].as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| PLACE_IDS.contains(n));
        let path = r["model"].as_str().map(|p| p.replace('/', "\\")).filter(|p| ok_path(p));
        let mut bbox = [0f32; 6];
        if let Some(b) = r["box"].as_array().filter(|b| b.len() == 6) {
            for (i, x) in b.iter().enumerate() {
                bbox[i] = x.as_f64().map(|f| f as f32).filter(|f| f.is_finite() && f.abs() < 10_000.0).unwrap_or(0.0);
            }
        }
        if let (Some(id), Some(path)) = (id, path) {
            if !out.iter().any(|o| o.0 == id) {
                out.push((id, path, bbox));
            }
        }
    }
    out
}

/// What the Den's patch-T gets for the models: every kept file, and the table (when `stock` is the player's own).
pub fn patch_files(dens: &Path, stock: Option<&[u8]>) -> Vec<(String, Vec<u8>)> {
    let models = list(dens);
    let mut out = Vec::new();
    for m in &models {
        for f in &m.files {
            if let Ok(b) = fs::read(folder(dens, &m.slug).join(last(f))) {
                out.push((f.clone(), b));
            }
        }
    }
    if let Some(t) = stock.and_then(|s| table(dens, s)) {
        out.push((TABLE.to_string(), t));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::denfiles::fake;

    #[test]
    fn dropped_models_are_kept_listed_and_go_in_the_patch() {
        let d = tempfile::tempdir().unwrap();
        let drop = vec![("Tower.m2".to_string(), fake::m2(1, &["x\\tower.blp"])), ("Tower00.skin".to_string(), b"SKIN".to_vec()), ("tower.blp".to_string(), fake::blp())];
        let m = add(d.path(), &drop, Some("Light house")).unwrap();
        assert_eq!((m.entry, m.display, m.name.as_str(), m.slug.as_str()), (980001, 60001, "Light house", "light_house"));
        assert!(d.path().join("my-models/light_house/light_house.m2").is_file());
        let k = add(d.path(), &[("Keep.wmo".into(), fake::wmo_root(1, &[])), ("Keep_000.wmo".into(), fake::wmo_group())], None).unwrap();
        assert_eq!((k.entry, k.display, k.kind.as_str()), (980002, 60002, "wmo"));
        // the same name again: put in its place, the same numbers
        assert_eq!(add(d.path(), &drop, Some("Light house")).unwrap().entry, 980001);
        assert_eq!(list(d.path()).len(), 2);
        // Den Lite's file: format, entries, displays, names
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(d.path().join(INDEX)).unwrap()).unwrap();
        assert_eq!(v["format"], FORMAT);
        assert_eq!(v["models"][1]["entry"], 980002);
        // the patch: every file under our folder, and the table with two rows more
        let stock = fake::table(19, &[1, 9000]);
        let files = patch_files(d.path(), Some(&stock));
        assert!(files.iter().filter(|f| f.0 != TABLE).all(|f| f.0.starts_with("World\\Leyline\\Den\\")));
        let t = &files.iter().find(|f| f.0 == TABLE).unwrap().1;
        assert_eq!(u32::from_le_bytes(t[4..8].try_into().unwrap()), 4);
        assert!(patch_files(d.path(), None).iter().all(|f| f.0 != TABLE), "no table of the player's: none of ours");
        assert!(remove(d.path(), "keep").unwrap());
        assert!(!d.path().join("my-models/keep").exists());
        assert!(!remove(d.path(), "keep").unwrap());
        assert!(table(&tempfile::tempdir().unwrap().path().join("none"), &stock).is_none(), "no models: the table as it is");
    }

    #[test]
    fn a_rebuilt_places_rows_join_the_table_and_a_bad_one_costs_nothing() {
        let d = tempfile::tempdir().unwrap();
        let stock = fake::table(19, &[1, 9000]);
        let rows = r#"{"format": "leyline-den-place-models", "version": 1, "models": [
            {"display": 62001, "model": "World\\Azeroth\\Rock.mdx", "box": [-1, -1, 0, 1, 1, 2]},
            {"display": 62002, "model": "World\\wmo\\Inn.wmo", "box": [0, 0, 0, 0, 0, 0]},
            {"display": 62002, "model": "World\\again.m2"},
            {"display": 5, "model": "World\\Low.m2"},
            {"display": 62003, "model": "..\\up.m2"},
            {"display": 62004, "model": "World\\Readme.txt"}]}"#;
        fs::write(d.path().join(PLACE_ROWS), rows).unwrap();
        let got = place_rows(d.path());
        assert_eq!(got.iter().map(|r| r.0).collect::<Vec<_>>(), vec![62001, 62002], "ours only, once each, files only");
        assert_eq!(got[0].2, [-1.0, -1.0, 0.0, 1.0, 1.0, 2.0]);
        let t = table(d.path(), &stock).unwrap();
        assert_eq!(u32::from_le_bytes(t[4..8].try_into().unwrap()), 4, "the stock two and the place's two");
        // with a dropped model too: all of them; a stock table already past our numbers: the player's own models still
        let drop = vec![("Tower.m2".to_string(), fake::m2(1, &["x\\tower.blp"])), ("Tower00.skin".to_string(), b"SKIN".to_vec()), ("tower.blp".to_string(), fake::blp())];
        add(d.path(), &drop, Some("Tower")).unwrap();
        assert_eq!(u32::from_le_bytes(table(d.path(), &stock).unwrap()[4..8].try_into().unwrap()), 5);
        let big = fake::table(19, &[1, 63000]);
        assert!(table(d.path(), &big).is_none() || u32::from_le_bytes(table(d.path(), &big).unwrap()[4..8].try_into().unwrap()) == 3);
        fs::write(d.path().join(PLACE_ROWS), "{}").unwrap();
        assert!(place_rows(d.path()).is_empty());
    }
}
