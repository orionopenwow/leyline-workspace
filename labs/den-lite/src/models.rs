//! The props' models for the Builder's gallery (WrathUnleashedUI Builder.lua: a tile per prop, its 3D model turning).
//!
//! The model of a gameobject is a path in the game's own `GameObjectDisplayInfo.dbc` (field 0 the display id, field
//! 1 the model's file name). That table is the player's own game data: Den Lite never ships it. The launcher copies it
//! from the player's own game archives into the Dens folder ([`FILE`], `host/lite.rs` at the Den's start); when it
//! isn't there, the catalog has no models and the gallery shows each category's icon instead.
//!
//! Only a path travels to the addon (the client draws its own file), with the model's size from the same row (its
//! bounding box, fields 12-17: the tile's camera fits the whole model). WMOs (a building's own format) can't be drawn
//! in a tile: they are marked `-wmo` so the tile shows a building icon on purpose; a display without a model `-`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// `<dens>/GameObjectDisplayInfo.dbc`: the player's own table, put there by the launcher.
pub const FILE: &str = "GameObjectDisplayInfo.dbc";

static PATHS: OnceLock<HashMap<u32, String>> = OnceLock::new();
/// display id -> the model's size in yards: (width, depth, height, the bottom's height), from the table's box.
static BOXES: OnceLock<HashMap<u32, (f32, f32, f32, f32)>> = OnceLock::new();
/// display ids whose model is a WMO (a building): the tile shows an icon.
static WMOS: OnceLock<std::collections::HashSet<u32>> = OnceLock::new();

/// A model path as the addon takes it: backslashes, letters, digits, spaces and `_ - . '`, no `..`, at most 160
/// letters, not a WMO. None for anything else.
pub fn clean(raw: &str) -> Option<String> {
    let p: String = raw.replace('/', "\\");
    if p.is_empty() || p.len() > 160 || p.contains("..") || p.starts_with('\\') {
        return None;
    }
    if !p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '\\' | '_' | '-' | '.' | ' ' | '\'')) {
        return None;
    }
    if p.to_ascii_lowercase().ends_with(".wmo") {
        return None;
    }
    Some(p)
}

/// display id -> model path, from the bytes of a GameObjectDisplayInfo.dbc. None when it isn't one.
pub fn from_dbc(b: &[u8]) -> Option<HashMap<u32, String>> {
    use leyline_terrain::dbc::Dbc;
    let t = Dbc::parse(b).ok()?;
    if t.fields < 2 {
        return None;
    }
    let mut out = HashMap::new();
    for r in &t.records {
        let (id, off) = (Dbc::get(r, 0), Dbc::get(r, 1));
        if let Some(p) = clean(&t.string(off)) {
            out.insert(id, p);
        }
    }
    Some(out)
}

/// display id -> (width, depth, height, bottom) in yards from the table's bounding box (3.3.5a: fields 12-17,
/// min x y z then max x y z), for rows with a sensible box. None when it isn't the table.
pub fn boxes_from_dbc(b: &[u8]) -> Option<HashMap<u32, (f32, f32, f32, f32)>> {
    use leyline_terrain::dbc::Dbc;
    let t = Dbc::parse(b).ok()?;
    if t.fields < 18 {
        return Some(HashMap::new());
    }
    let mut out = HashMap::new();
    for r in &t.records {
        let v: Vec<f32> = (12..18).map(|i| Dbc::getf(r, i)).collect();
        let (w, d, h) = (v[3] - v[0], v[4] - v[1], v[5] - v[2]);
        let ok = v.iter().all(|x| x.is_finite() && x.abs() < 2000.0) && w >= 0.0 && d >= 0.0 && h >= 0.0 && (w + d + h) > 0.01;
        if ok {
            out.insert(Dbc::get(r, 0), (w, d, h, v[2]));
        }
    }
    Some(out)
}

/// display ids whose model is a WMO (a building's own format).
pub fn wmos_from_dbc(b: &[u8]) -> std::collections::HashSet<u32> {
    use leyline_terrain::dbc::Dbc;
    let Ok(t) = Dbc::parse(b) else { return Default::default() };
    if t.fields < 2 {
        return Default::default();
    }
    t.records.iter().filter(|r| t.string(Dbc::get(r, 1)).to_ascii_lowercase().ends_with(".wmo")).map(|r| Dbc::get(r, 0)).collect()
}

/// Reads `<dens>/GameObjectDisplayInfo.dbc` once (at start); how many models it knows (0: none, the icons then).
pub fn load(dir: &Path) -> usize {
    let b = std::fs::read(dir.join(FILE)).ok();
    let m = b.as_deref().and_then(from_dbc).unwrap_or_default();
    let n = m.len();
    let _ = PATHS.set(m);
    let _ = BOXES.set(b.as_deref().and_then(boxes_from_dbc).unwrap_or_default());
    let _ = WMOS.set(b.as_deref().map(wmos_from_dbc).unwrap_or_default());
    n
}

/// The catalog's model field for a display (Builder.lua parseCatalog): `<path>,<w>x<d>x<h>x<bottom>` (the size in
/// yards, two decimals), `<path>` without a box, `-wmo` for a building (the tile shows an icon), `-` when the table
/// has no model for it; None when there is no table at all (the old five fields).
pub fn tile(display: u32) -> Option<String> {
    let paths = PATHS.get()?;
    if paths.is_empty() {
        return None;
    }
    Some(tile_field(paths.get(&display).map(String::as_str), BOXES.get().and_then(|b| b.get(&display)).copied(), WMOS.get().is_some_and(|w| w.contains(&display))))
}

/// [`tile`] from its parts.
pub fn tile_field(path: Option<&str>, size: Option<(f32, f32, f32, f32)>, wmo: bool) -> String {
    match (path, size) {
        (Some(p), Some((w, d, h, z))) => format!("{p},{w:.2}x{d:.2}x{h:.2}x{z:.2}"),
        (Some(p), None) => p.to_string(),
        (None, _) if wmo => "-wmo".into(),
        (None, _) => "-".into(),
    }
}

/// A display's size in yards (width, depth, height, the bottom's height) from the player's own table, if Den Lite
/// read it (stamps.rs fits stamps to it).
pub fn box_of(display: u32) -> Option<(f32, f32, f32, f32)> {
    BOXES.get()?.get(&display).copied()
}

/// Whether a display is a building (a WMO: the game makes it solid all over, floors and walls; buildtools.rs Solid).
pub fn is_wmo(display: u32) -> bool {
    WMOS.get().is_some_and(|w| w.contains(&display))
}

/// The model path of a display id, if the table was there.
pub fn of(display: u32) -> Option<&'static str> {
    PATHS.get()?.get(&display).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dbc(rows: &[(u32, &str)]) -> Vec<u8> {
        let mut strings = vec![0u8];
        let mut recs = Vec::new();
        for (id, p) in rows {
            let off = strings.len() as u32;
            strings.extend_from_slice(p.as_bytes());
            strings.push(0);
            recs.extend_from_slice(&id.to_le_bytes());
            recs.extend_from_slice(&off.to_le_bytes());
        }
        let mut b = b"WDBC".to_vec();
        for v in [rows.len() as u32, 2, 8, strings.len() as u32] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&recs);
        b.extend_from_slice(&strings);
        b
    }

    #[test]
    fn paths_from_the_table_wmos_and_odd_paths_left_out() {
        let m = from_dbc(&dbc(&[(39, "World\\Generic\\Human\\Passive Doodads\\Chairs\\Chair01.mdx"), (9007, "World/wmo/Azeroth/Buildings/Inn.wmo"), (5, "..\\x.m2"), (6, "World/Generic/Barrel's.m2")])).unwrap();
        assert_eq!(m.get(&39).map(String::as_str), Some("World\\Generic\\Human\\Passive Doodads\\Chairs\\Chair01.mdx"));
        assert!(!m.contains_key(&9007) && !m.contains_key(&5));
        assert_eq!(m.get(&6).map(String::as_str), Some("World\\Generic\\Barrel's.m2"));
        assert!(from_dbc(b"nope").is_none());
        assert_eq!(clean("a;b"), None);
        assert_eq!(clean("a,b"), None);
    }

    #[test]
    fn the_tiles_field_a_path_and_its_size_a_building_or_nothing() {
        assert_eq!(tile_field(Some("World\\A.mdx"), Some((1.0, 0.5, 2.25, -0.1)), false), "World\\A.mdx,1.00x0.50x2.25x-0.10");
        assert_eq!(tile_field(Some("World\\A.mdx"), None, false), "World\\A.mdx");
        assert_eq!(tile_field(None, None, true), "-wmo");
        assert_eq!(tile_field(None, None, false), "-");
        // a table of 19 fields (3.3.5a): the box from fields 12-17, WMOs found by their path
        let mut strings = vec![0u8];
        let mut recs = Vec::new();
        for (id, p, bx) in [(39u32, "World\\Chair01.mdx", [-0.5f32, -0.5, 0.0, 0.5, 0.5, 1.2]), (9007, "World\\wmo\\Inn.wmo", [0.0; 6])] {
            let off = strings.len() as u32;
            strings.extend_from_slice(p.as_bytes());
            strings.push(0);
            let mut row = vec![0u8; 19 * 4];
            row[0..4].copy_from_slice(&id.to_le_bytes());
            row[4..8].copy_from_slice(&off.to_le_bytes());
            for (i, v) in bx.iter().enumerate() {
                row[(12 + i) * 4..(13 + i) * 4].copy_from_slice(&v.to_le_bytes());
            }
            recs.extend_from_slice(&row);
        }
        let mut b = b"WDBC".to_vec();
        for v in [2u32, 19, 76, strings.len() as u32] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&recs);
        b.extend_from_slice(&strings);
        let boxes = boxes_from_dbc(&b).unwrap();
        assert_eq!(boxes.get(&39), Some(&(1.0, 1.0, 1.2, 0.0)));
        assert!(!boxes.contains_key(&9007), "an empty box isn't a size");
        let w = wmos_from_dbc(&b);
        assert!(w.contains(&9007) && !w.contains(&39));
        assert!(from_dbc(&b).unwrap().contains_key(&39));
    }
}
