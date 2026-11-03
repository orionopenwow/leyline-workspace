//! The props' models for the Builder's gallery (WrathUnleashedUI Builder.lua: a tile per prop, its 3D model turning).
//!
//! The model of a gameobject is a path in the game's own `GameObjectDisplayInfo.dbc` (field 0 the display id, field
//! 1 the model's file name). That table is the player's own game data: Den Lite never ships it. The launcher copies it
//! from the player's own game archives into the Dens folder ([`FILE`], `host/lite.rs` at the Den's start); when it
//! isn't there, the catalog has no models and the gallery shows each category's icon instead.
//!
//! Only a path travels to the addon (the client draws its own file). WMOs (a building's own format) are left out: a
//! tile can't draw one.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// `<dens>/GameObjectDisplayInfo.dbc`: the player's own table, put there by the launcher.
pub const FILE: &str = "GameObjectDisplayInfo.dbc";

static PATHS: OnceLock<HashMap<u32, String>> = OnceLock::new();

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

/// Reads `<dens>/GameObjectDisplayInfo.dbc` once (at start); how many models it knows (0: none, the icons then).
pub fn load(dir: &Path) -> usize {
    let m = std::fs::read(dir.join(FILE)).ok().and_then(|b| from_dbc(&b)).unwrap_or_default();
    let n = m.len();
    let _ = PATHS.set(m);
    n
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
}
