//! A place in the player's own game copied into a Den stamp ("copy a place from my game"; owner 3 Nov: "a stamp should
//! also have the ability to create the 1:1 Lion's Pride from Goldshire, for testing how it works").
//!
//! Everything is read on the player's PC from their own files and written into their own Dens folder
//! (`<dens>/stamps-local/<name>.den.json`): the layout is the game's, so it is never shipped or committed. What ships
//! is [`PLACES`]: a name, the map, the tiles to look in, words to find the building's file by, a radius. No positions.
//!
//! How (written from the public format notes: the wowdev wiki's "ADT/v18" MODF / MDDF and "WMO" root MOHD, MODS,
//! MODN, MODD pages):
//! - the tile's building placement (MODF) whose file name has the place's words;
//! - its doodads: the WMO root's sets (MODS): the default set (0) and the placement's own, their entries (MODD:
//!   model name, position and rotation in the building's own space, size);
//! - the tile's loose models (MDDF) within the radius of the building;
//! - each model becomes a stamp piece when the player's GameObjectDisplayInfo has a display with that model file
//!   (Den Lite shows any display through its model carrier: entry [`MODEL_BASE`] + display); the rest are counted as
//!   skipped, by file name;
//! - the building itself is placed when a display's model is that WMO file, else the inside is copied without it.
//!
//! Positions: a placement's world x = PLACE - its z, world y = PLACE - its x, height = its y; its facing in the world
//! is its y rotation + 180 degrees (the same frame as the server's model spawns: the placement frame is the world
//! turned half round). The stamp is in the building's own frame (x its front's way, y its left), so a wrong
//! [`YAW_OFFSET`] turns the whole copy as one, never apart.

use crate::adt::{walk, Adt};
use crate::dbc::Dbc;
use crate::{err, Error, Result, PLACE};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Den Lite's model carrier (labs/den-lite den.rs MODEL_BASE): a stamp entry of this plus a display id shows that
/// display.
pub const MODEL_BASE: u64 = 2_000_000_000;
/// A placement's world facing: its y rotation plus this (degrees).
pub const YAW_OFFSET: f32 = 180.0;
/// At most this many pieces in a copied place.
pub const MAX_PIECES: usize = 400;
/// Pieces farther than this from the building (any way) are left out: Den stamps reach 40 yards.
pub const SPAN: f32 = 38.0;

/// A place to copy: names and file words only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub name: &'static str,
    pub title: &'static str,
    /// The map's folder (World\Maps\<map>).
    pub map: &'static str,
    /// The tiles to look in (x, y of `<map>_<x>_<y>.adt`), in order.
    pub tiles: &'static [(u32, u32)],
    /// Words the building's file name has (lower case; the first word found wins).
    pub wmo_words: &'static [&'static str],
    /// Loose models within this many yards of the building come too.
    pub radius: f32,
}

/// The places `.den copy` knows. TODO(owner): more places (a camp, a farm) once the Lion's Pride looks right.
pub const PLACES: &[Place] = &[Place {
    name: "lionspride",
    title: "The Lion's Pride",
    map: "Azeroth",
    tiles: &[(31, 49), (32, 49), (31, 48), (32, 48)],
    wmo_words: &["goldshire", "lionspride", "lion", "inn"],
    radius: 22.0,
}];

pub fn place(name: &str) -> Option<&'static Place> {
    PLACES.iter().find(|p| p.name == name)
}

/// The archive path of a place's tile.
pub fn tile_path(p: &Place, (x, y): (u32, u32)) -> String {
    format!("World\\Maps\\{0}\\{0}_{1}_{2}.adt", p.map, x, y)
}

/// A model file name as the tables and tiles compare: lower case, backslashes, no extension (.m2 / .mdx / .mdl are
/// one model).
pub fn norm(path: &str) -> String {
    let p = path.trim().replace('/', "\\").to_ascii_lowercase();
    for ext in [".m2", ".mdx", ".mdl"] {
        if let Some(s) = p.strip_suffix(ext) {
            return s.to_string();
        }
    }
    p
}

/// The file's own name (after the last backslash), for the skipped list.
fn short(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

/// model file (normed) -> display id, from the player's GameObjectDisplayInfo.dbc (field 0 the id, 1 the model); the
/// lowest id when several share a model. WMO buildings are in it too (their `.wmo` kept).
pub fn display_index(dbc: &[u8]) -> Result<HashMap<String, u32>> {
    let t = Dbc::parse(dbc)?;
    if t.fields < 2 {
        return err("That isn't the model table.");
    }
    let mut out: HashMap<String, u32> = HashMap::new();
    for r in &t.records {
        let (id, path) = (Dbc::get(r, 0), t.string(Dbc::get(r, 1)));
        if path.is_empty() {
            continue;
        }
        let e = out.entry(norm(&path)).or_insert(id);
        *e = (*e).min(id);
    }
    Ok(out)
}

/// One doodad of a WMO: its model, place in the building's frame, rotation (x, y, z, w), size.
#[derive(Debug, Clone, PartialEq)]
pub struct WmoDoodad {
    pub model: String,
    pub pos: [f32; 3],
    pub rot: [f32; 4],
    pub scale: f32,
}

/// A WMO root's doodad sets (name, first, count) and doodads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WmoDoodads {
    pub sets: Vec<(String, u32, u32)>,
    pub doodads: Vec<WmoDoodad>,
}

fn f(b: &[u8], at: usize) -> Result<f32> {
    b.get(at..at + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| Error("The building file is cut short.".into()))
}
fn u(b: &[u8], at: usize) -> Result<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| Error("The building file is cut short.".into()))
}
fn zname(b: &[u8], off: usize) -> String {
    let rest = b.get(off..).unwrap_or(&[]);
    let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).to_string()
}

/// Reads a WMO root's MODS (32 bytes: a 20-byte name, first, count, pad), MODN (names) and MODD (40 bytes: a 24-bit
/// name offset and 8 flag bits, position, rotation x y z w, size, colour).
pub fn wmo_doodads(root: &[u8]) -> Result<WmoDoodads> {
    let chunks = walk(root)?;
    let data = |t: &[u8; 4]| chunks.iter().find(|c| &c.0 == t).map(|c| &root[c.1..c.1 + c.2]).unwrap_or(&[]);
    if !chunks.iter().any(|c| &c.0 == b"MOHD") {
        return err("That isn't a building's root file (no MOHD).");
    }
    let (mods, modn, modd) = (data(b"MODS"), data(b"MODN"), data(b"MODD"));
    let mut out = WmoDoodads::default();
    for s in mods.chunks_exact(32) {
        out.sets.push((zname(&s[..20], 0), u(s, 20)?, u(s, 24)?));
    }
    for d in modd.chunks_exact(40) {
        let off = (u(d, 0)? & 0x00FF_FFFF) as usize;
        out.doodads.push(WmoDoodad {
            model: zname(modn, off),
            pos: [f(d, 4)?, f(d, 8)?, f(d, 12)?],
            rot: [f(d, 16)?, f(d, 20)?, f(d, 24)?, f(d, 28)?],
            scale: f(d, 32)?,
        });
    }
    Ok(out)
}

/// A placement's world spot (x, y, height) from its placement position.
pub fn world(pos: [f32; 3]) -> [f32; 3] {
    [PLACE - pos[2], PLACE - pos[0], pos[1]]
}

/// The yaw (about the up axis, radians) of a rotation quaternion (x, y, z, w).
pub fn quat_yaw(q: [f32; 4]) -> f32 {
    let [x, y, z, w] = q;
    (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z))
}

/// What a copy made.
#[derive(Debug, Clone, PartialEq)]
pub struct Copied {
    /// The stamp (a `leyline-den` stamp document).
    pub doc: Value,
    pub pieces: usize,
    /// Whether the building itself is in it.
    pub shell: bool,
    /// Models with no display of the same file: (file name, how many).
    pub skipped: Vec<(String, usize)>,
    /// The building's file name.
    pub wmo: String,
}

fn r2(v: f32) -> f64 {
    ((v as f64) * 100.0).round() / 100.0
}

/// The building of `p` in this tile: its placement index, when one of the tile's building files has the words.
pub fn find_building(p: &Place, adt: &Adt) -> Option<usize> {
    for w in p.wmo_words {
        let hit = adt.wmos.iter().position(|m| adt.wmo_names.get(m.name as usize).is_some_and(|n| short(n).to_ascii_lowercase().contains(w)));
        if hit.is_some() {
            return hit;
        }
    }
    None
}

/// Copies place `p` from a tile: `wmo_root(path)` reads a building's root file from the player's archives,
/// `displays` is [`display_index`] of their GameObjectDisplayInfo.
pub fn copy_place(p: &Place, adt: &Adt, wmo_root: impl Fn(&str) -> Option<Vec<u8>>, displays: &HashMap<String, u32>) -> Result<Copied> {
    let i = find_building(p, adt).ok_or_else(|| Error(format!("{} isn't in this map tile.", p.title)))?;
    let m = &adt.wmos[i];
    let wmo_path = adt.wmo_names.get(m.name as usize).cloned().unwrap_or_default();
    let o = world(m.pos);
    let yaw = (m.rot[1] + YAW_OFFSET).to_radians();
    let (s, c) = yaw.sin_cos();
    // world -> the building's frame (x its front's way, y its left), and a facing in it (degrees)
    let local = |w: [f32; 3]| {
        let (dx, dy) = (w[0] - o[0], w[1] - o[1]);
        [dx * c + dy * s, -dx * s + dy * c, w[2] - o[2]]
    };
    let face = |world_yaw: f32| ((world_yaw - yaw).to_degrees()).rem_euclid(360.0);
    let mut props: Vec<Value> = Vec::new();
    let mut skipped: HashMap<String, usize> = HashMap::new();
    let mut add = |model: &str, at: [f32; 3], yaw_deg: f32, scale: f32, props: &mut Vec<Value>, role: Option<&str>| {
        if at.iter().any(|v| !v.is_finite() || v.abs() > SPAN) || props.len() >= MAX_PIECES {
            return;
        }
        match displays.get(&norm(model)) {
            Some(&d) => {
                let mut e = json!({"entry": MODEL_BASE + d as u64, "x": r2(at[0]), "y": r2(at[1]), "z": r2(at[2]), "yaw": r2(yaw_deg)});
                if (scale - 1.0).abs() > 0.01 && scale.is_finite() {
                    e["scale"] = json!(r2(scale.clamp(0.1, 5.0)));
                }
                if let Some(r) = role {
                    e["role"] = json!(r);
                }
                props.push(e);
            }
            None => *skipped.entry(short(model)).or_insert(0) += 1,
        }
    };
    // the building
    let shell = displays.contains_key(&norm(&wmo_path));
    if shell {
        add(wmo_path.as_str(), [0.0, 0.0, 0.0], 0.0, 1.0, &mut props, Some("house"));
    }
    // its doodads: the default set and the placement's own
    if let Some(root) = wmo_root(wmo_path.as_str()) {
        let w = wmo_doodads(&root)?;
        let mut sets = vec![0usize];
        if m.doodad_set != 0 {
            sets.push(m.doodad_set as usize);
        }
        for k in sets {
            let Some(&(_, first, count)) = w.sets.get(k) else { continue };
            for d in w.doodads.iter().skip(first as usize).take(count as usize) {
                // already in the building's frame
                add(d.model.as_str(), d.pos, quat_yaw(d.rot).to_degrees().rem_euclid(360.0), d.scale, &mut props, None);
            }
        }
    }
    // the loose models round it
    for d in &adt.doodads {
        let at = world(d.pos);
        if ((at[0] - o[0]).powi(2) + (at[1] - o[1]).powi(2)).sqrt() > p.radius {
            continue;
        }
        let name = adt.m2_names.get(d.name as usize).cloned().unwrap_or_default();
        add(name.as_str(), local(at), face((d.rot[1] + YAW_OFFSET).to_radians()), d.scale as f32 / 1024.0, &mut props, None);
    }
    let mut skipped: Vec<(String, usize)> = skipped.into_iter().collect();
    skipped.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let n = props.len();
    let line = format!(
        "Copied from your own game files: {} pieces{}{}.",
        n,
        if shell { ", the building too" } else { "; the building has no model here, so only its inside" },
        if skipped.is_empty() { String::new() } else { format!(", {} models with no match left out", skipped.iter().map(|s| s.1).sum::<usize>()) }
    );
    let doc = json!({
        "format": "leyline-den", "version": 1, "kind": "stamp", "local": true, "name": p.name, "title": p.title, "line": line,
        "focus": [0.0, 0.0], "source": short(&wmo_path),
        "skipped": skipped.iter().map(|(f, k)| json!({"file": f, "count": k})).collect::<Vec<_>>(),
        "props": props,
    });
    Ok(Copied { doc, pieces: n, shell, skipped, wmo: short(&wmo_path) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adt::{put, Doodad, WmoPlacement};

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

    /// A made-up building: two sets (the default: a table; set 1: a chair and a lamp nobody has a display for).
    fn root() -> Vec<u8> {
        let mut o = Vec::new();
        put(&mut o, b"MVER", &17u32.to_le_bytes());
        put(&mut o, b"MOHD", &[0u8; 64]);
        let mut mods = Vec::new();
        for (name, first, count) in [("Set_$DefaultGlobal", 0u32, 1u32), ("Set_Tavern", 1, 2)] {
            let mut n = [0u8; 20];
            n[..name.len()].copy_from_slice(name.as_bytes());
            mods.extend_from_slice(&n);
            for v in [first, count, 0] {
                mods.extend_from_slice(&v.to_le_bytes());
            }
        }
        put(&mut o, b"MODS", &mods);
        let modn = b"Made\\Up\\Table.m2\0Made\\Up\\Chair.M2\0Made\\Up\\Lamp.m2\0".to_vec();
        put(&mut o, b"MODN", &modn);
        let mut modd = Vec::new();
        let half = std::f32::consts::FRAC_1_SQRT_2;
        for (off, pos, rot, scale) in [
            (0u32, [2.0f32, 0.0, 0.5], [0.0f32, 0.0, 0.0, 1.0], 1.0f32),
            (17 | 0x0100_0000, [2.0, 1.5, 0.5], [0.0, 0.0, half, half], 1.5),
            (34, [-1.0, -1.0, 2.0], [0.0, 0.0, 0.0, 1.0], 1.0),
        ] {
            modd.extend_from_slice(&off.to_le_bytes());
            for v in pos.iter().chain(rot.iter()).chain([scale].iter()) {
                modd.extend_from_slice(&v.to_le_bytes());
            }
            modd.extend_from_slice(&0u32.to_le_bytes());
        }
        put(&mut o, b"MODD", &modd);
        o
    }

    fn tile(set: u16) -> Adt {
        let mut a = Adt::blank(31, 49, 50.0, "x.blp");
        a.wmo_names = vec!["Made\\Up\\Shed.wmo".into(), "Made\\Up\\GoldshireInn.wmo".into()];
        a.m2_names = vec!["Made\\Up\\Barrel.mdx".into(), "Made\\Up\\Rock.m2".into()];
        let at = [PLACE - 100.0, 60.0, PLACE - 200.0]; // world (200, 100), height 60
        let wmo = |name: u32, pos: [f32; 3]| WmoPlacement {
            name, uid: name, pos, rot: [0.0, 90.0, 0.0], lower: pos, upper: pos, flags: 0, doodad_set: set, name_set: 0, pad: 0,
        };
        a.wmos = vec![wmo(0, [PLACE - 300.0, 60.0, PLACE - 300.0]), wmo(1, at)];
        let dd = |name: u32, pos: [f32; 3], ry: f32| Doodad { name, uid: 9, pos, rot: [0.0, ry, 0.0], scale: 2048, flags: 0 };
        // a barrel 5 yards north of the inn (world +x), a rock far away
        a.doodads = vec![dd(0, [PLACE - 100.0, 61.0, PLACE - 205.0], 0.0), dd(1, [PLACE - 100.0, 60.0, PLACE - 400.0], 0.0)];
        a
    }

    #[test]
    fn names_are_compared_without_case_slashes_or_extension() {
        assert_eq!(norm("World/Generic/Chair.MDX"), "world\\generic\\chair");
        assert_eq!(norm("world\\generic\\chair.m2"), "world\\generic\\chair");
        assert_eq!(norm("Inn.wmo"), "inn.wmo");
        let ix = display_index(&dbc(&[(40, "Made\\Up\\Table.mdx"), (12, "made/up/table.m2"), (7, "Made\\Up\\Inn.wmo")])).unwrap();
        assert_eq!(ix.get("made\\up\\table"), Some(&12), "the lowest id");
        assert_eq!(ix.get("made\\up\\inn.wmo"), Some(&7));
        assert!(display_index(b"nope").is_err());
    }

    #[test]
    fn a_buildings_doodad_sets_are_read() {
        let w = wmo_doodads(&root()).unwrap();
        assert_eq!(w.sets, vec![(String::from("Set_$DefaultGlobal"), 0u32, 1u32), (String::from("Set_Tavern"), 1, 2)]);
        assert_eq!(w.doodads.len(), 3);
        assert_eq!(w.doodads[1].model, "Made\\Up\\Chair.M2", "the name offset is the low 24 bits");
        assert_eq!(w.doodads[1].scale, 1.5);
        assert!((quat_yaw(w.doodads[1].rot) - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!(wmo_doodads(&[]).is_err());
    }

    #[test]
    fn placement_space_to_the_world() {
        assert_eq!(world([PLACE - 100.0, 60.0, PLACE - 200.0]), [200.0, 100.0, 60.0]);
        assert_eq!(tile_path(&PLACES[0], (31, 49)), "World\\Maps\\Azeroth\\Azeroth_31_49.adt");
        assert!(place("lionspride").is_some() && place("nowhere").is_none());
    }

    #[test]
    fn the_inn_is_copied_in_its_own_frame_with_its_set_and_whats_round_it() {
        let ix = display_index(&dbc(&[(5, "Made\\Up\\Table.mdx"), (6, "Made\\Up\\Chair.mdx"), (7, "Made\\Up\\Barrel.m2"), (8, "Made\\Up\\GoldshireInn.wmo")])).unwrap();
        let r = root();
        let c = copy_place(&PLACES[0], &tile(1), |p| (p == "Made\\Up\\GoldshireInn.wmo").then(|| r.clone()), &ix).unwrap();
        assert!(c.shell);
        assert_eq!(c.wmo, "GoldshireInn.wmo");
        let props = c.doc["props"].as_array().unwrap();
        // the inn, the table (set 0), the chair (set 1), the barrel; the lamp has no display, the rock is too far
        assert_eq!(c.pieces, 4);
        assert_eq!(c.skipped, vec![("Lamp.m2".to_string(), 1)]);
        let entry = |d: u64| props.iter().find(|p| p["entry"].as_u64() == Some(MODEL_BASE + d)).unwrap();
        assert_eq!(entry(8)["role"], "house");
        assert_eq!((entry(8)["x"].as_f64(), entry(8)["yaw"].as_f64()), (Some(0.0), Some(0.0)));
        assert_eq!((entry(5)["x"].as_f64(), entry(5)["y"].as_f64(), entry(5)["z"].as_f64()), (Some(2.0), Some(0.0), Some(0.5)));
        assert_eq!(entry(6)["yaw"].as_f64(), Some(90.0));
        assert_eq!(entry(6)["scale"].as_f64(), Some(1.5));
        // the barrel: 5 yards world +x of the inn; the inn faces 90 + 180 = 270 degrees (world -y), so world +x is
        // to its left... its frame: x = dx cos + dy sin = 0, y = -dx sin + dy cos = 5
        let b = entry(7);
        assert!(b["x"].as_f64().unwrap().abs() < 0.01 && (b["y"].as_f64().unwrap() - 5.0).abs() < 0.01, "{b}");
        assert_eq!(b["z"].as_f64(), Some(1.0));
        assert_eq!(b["scale"].as_f64(), Some(2.0));
        assert_eq!(b["yaw"].as_f64(), Some(270.0), "facing 180 in the world, 270 - 180... in the inn's frame");
        assert_eq!(c.doc["kind"], "stamp");
        assert_eq!(c.doc["local"], true);
        let text = c.doc.to_string().to_lowercase();
        for w in ["warcraft", "blizzard", "lich king"] {
            assert!(!text.contains(w));
        }
    }

    #[test]
    fn without_a_building_model_the_inside_only_and_the_default_set_alone() {
        let ix = display_index(&dbc(&[(5, "Made\\Up\\Table.mdx"), (6, "Made\\Up\\Chair.mdx")])).unwrap();
        let r = root();
        let c = copy_place(&PLACES[0], &tile(0), |_| Some(r.clone()), &ix).unwrap();
        assert!(!c.shell);
        assert_eq!(c.pieces, 1, "set 0 only: the table");
        assert!(c.doc["line"].as_str().unwrap().contains("only its inside"));
        assert!(c.skipped.iter().any(|s| s.0 == "Barrel.mdx"));
        // no inn in the tile
        let mut t = tile(0);
        t.wmo_names = vec!["Made\\Up\\Shed.wmo".into()];
        t.wmos.truncate(1);
        assert!(copy_place(&PLACES[0], &t, |_| None, &ix).unwrap_err().0.contains("isn't in this map tile"));
    }
}
