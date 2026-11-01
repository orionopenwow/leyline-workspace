//! leyline-terrain: Leyline's own code for the Den's ground (docs/NOGGIT-DEN-PLAN.md, phase 2). MIT.
//!
//! * [`adt`]: read and write a 3.3.5a map tile (ADT, version 18): MVER, MHDR, MCIN, MTEX, MMDX/MMID, MWMO/MWID, MDDF,
//!   MODF, MH2O, the 256 MCNK (MCVT heights, MCCV, MCNR normals recomputed, MCLY + MCAL alpha layers in the 2048 and
//!   4096 forms and the compressed form, MCRF, MCSH, MCLQ and MCSE kept as they are, holes), MFBO, MTXF;
//! * [`wdt`]: the map's tile table (MPHD, MAIN, MWMO, MODF);
//! * [`delta`]: the terrain delta (`<den>.terrain.json`, docs/terrain-delta-leyline.schema.json): numbers and file names only.
//!   [`delta::apply`] rebuilds a tile from the player's own tile plus a delta, [`delta::diff`] makes a delta from two
//!   tiles;
//! * [`flat`]: the same delta in the form Forge writes and reads (whole chunks, additions and moves of models by id),
//!   turned into ours and back;
//! * [`pad`]: the Den's white pad (2 x 2 chunks flattened and painted, grown by rings) and [`pad::Ground`], the ground
//!   height at a point from a delta alone (Den Lite).
//!
//! Written from the public descriptions of the formats (the wowdev wiki's "ADT/v18", "WDT" and "MH2O" pages, the
//! mangos wiki's "ADT Files" page) and our own maths. No code from any map editor is in here (the editor Leyline
//! starts, Forge, is a separate GPL program: docs/NOGGIT-DEN-PLAN.md section 1).
//!
//! Never shipped with game data: tests build their tiles with [`adt::Adt::blank`] and by hand.

pub mod adt;
pub mod b64;
pub mod delta;
pub mod flat;
pub mod pad;
pub mod wdt;

/// One map tile is 533.333 yards a side (1600 feet).
pub const TILE: f32 = 1600.0 / 3.0;
/// One chunk: a 16th of a tile.
pub const CHUNK: f32 = TILE / 16.0;
/// One step between two outer height vertices: an 8th of a chunk.
pub const UNIT: f32 = CHUNK / 8.0;
/// Model placement space: world x = PLACE - placement z, world y = PLACE - placement x, world z = placement y.
pub const PLACE: f32 = 32.0 * TILE;

/// An error with a plain sentence for the player (or the log).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn err<T>(s: impl Into<String>) -> Result<T> {
    Err(Error(s.into()))
}

/// The world corner (highest x, highest y) of chunk (row, col) of the tile `<dir>_<x>_<y>.adt`. The chunk covers
/// the 33.3-yard square towards lower x and y from there (rows run towards lower world x, columns towards lower y).
pub fn chunk_corner(tile_x: u32, tile_y: u32, row: u32, col: u32) -> (f32, f32) {
    ((32.0 - tile_y as f32) * TILE - row as f32 * CHUNK, (32.0 - tile_x as f32) * TILE - col as f32 * CHUNK)
}

/// The tile (file name numbers x, y) and the chunk (row, col) a world point lies in, or None off the map.
pub fn locate(wx: f32, wy: f32) -> Option<((u32, u32), (u32, u32))> {
    let fx = 32.0 - wy / TILE; // file x from world y
    let fy = 32.0 - wx / TILE;
    if !(0.0..64.0).contains(&fx) || !(0.0..64.0).contains(&fy) {
        return None;
    }
    let (tx, ty) = (fx.floor() as u32, fy.floor() as u32);
    let col = (((fx - tx as f32) * 16.0).floor() as u32).min(15);
    let row = (((fy - ty as f32) * 16.0).floor() as u32).min(15);
    Some(((tx, ty), (row, col)))
}

/// A file name inside the archives: printable, no drive, no `..`, at most 260 bytes. Returned with backslashes.
pub fn clean_path(p: &str) -> Option<String> {
    let n = p.replace('/', "\\");
    let ok = !n.is_empty()
        && n.len() <= 260
        && n.bytes().all(|b| (0x20..0x7f).contains(&b) && !matches!(b, b':' | b'"' | b'<' | b'>' | b'|'))
        && !n.starts_with('\\')
        && n.split('\\').all(|s| !s.is_empty() && s != "." && s != "..");
    ok.then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dens_point_is_in_kalimdor_1_1() {
        // docs/DEN.md "The room": map 1, (16226.2, 16257.0); denslice.py's grid maths gives tile 1, 1.
        let ((tx, ty), (row, col)) = locate(16226.2, 16257.0).unwrap();
        assert_eq!((tx, ty), (1, 1));
        let (cx, cy) = chunk_corner(tx, ty, row, col);
        assert!(cx >= 16226.2 && cx - CHUNK < 16226.2, "{cx}");
        assert!(cy >= 16257.0 && cy - CHUNK < 16257.0, "{cy}");
        assert_eq!((row, col), (9, 8));
        assert!(locate(20000.0, 0.0).is_none());
    }

    #[test]
    fn paths_are_checked() {
        assert_eq!(clean_path("Tileset/Generic/White.blp").as_deref(), Some("Tileset\\Generic\\White.blp"));
        for bad in ["", "..\\x", "c:\\x", "\\abs", "a\\\\b", "caf\u{e9}.blp"] {
            assert!(clean_path(bad).is_none(), "{bad:?}");
        }
    }
}
