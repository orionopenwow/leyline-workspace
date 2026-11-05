//! Your Den's gliding copy (owner 5 Nov, after his live test: "objects should ... dynamically move instead of
//! disappearing, reappearing"): the creature rows that let the game draw a prop's model on a creature.
//!
//! Den Lite (labs/den-lite `glide.rs`) shows the prop in hand as a creature, because a creature glides
//! (SMSG_MONSTER_MOVE) where a gameobject can only be made again at each spot (a flicker). A creature's model comes
//! from two of the game's tables:
//!
//! * `CreatureDisplayInfo.dbc` (3.3.5a, 16 fields of 4 bytes): id, ModelID (a CreatureModelData id), SoundID,
//!   ExtendedDisplayInfoID, CreatureModelScale (float), CreatureModelAlpha (0..255), TextureVariation 1..3 (strings),
//!   PortraitTextureName (string), SizeClass, BloodID, NPCSoundID, ParticleColorID, CreatureGeosetData,
//!   ObjectEffectPackageID.
//! * `CreatureModelData.dbc` (3.3.5a, 28 fields): id, Flags, ModelName (string), SizeClass, ModelScale (float), BloodID,
//!   FootprintTextureID, FootprintTextureLength, FootprintTextureWidth, FootprintParticleScale (floats),
//!   FoleyMaterialID, FootstepShakeSize, DeathThudShakeSize, SoundID, CollisionWidth, CollisionHeight, MountHeight
//!   (floats), GeoBoxMin x y z, GeoBoxMax x y z (floats), WorldEffectScale, AttachedEffectScale,
//!   MissileCollisionRadius, MissileCollisionPush, MissileCollisionRaise (floats).
//!
//! (wowdev wiki "DB/CreatureDisplayInfo" and "DB/CreatureModelData", 3.3.5.12340; the same columns the stream's
//! slice tools read: stream/tests/test_slice_models.py, ModelID at field 1, ModelName at field 2.)
//!
//! For every gameobject display (`GameObjectDisplayInfo.dbc`, the player's own, with the Den's own rows from
//! host/denmodels.rs) whose model is an `.m2` / `.mdx` / `.mdl` (a building's `.wmo` can't be a creature's model),
//! both tables get a row [`DISPLAY_BASE`] + that display id naming the same file, at scale 1. The tables are the
//! player's OWN, read from their own archives and put in the Den's patch-T on this PC (taken out whenever they play
//! anywhere else); nothing is shipped. Den Lite is told which displays have rows by [`MARKER`] in the Dens folder
//! (written only when the tables really went in and no later archive hides them).
//!
//! Den Lite builds this same file (labs/den-lite lib.rs, `#[path]`) for its `glide-rows` command (gliderows.rs: the
//! rows on this PC before a launcher with them is out), so both write exactly the same rows.

use leyline_terrain::dbc::Dbc;
use std::path::Path;

/// labs/den-lite glide.rs DISPLAY_BASE: creature display (and model) `DISPLAY_BASE + d` wears gameobject display `d`.
pub const DISPLAY_BASE: u32 = 70_000;
pub const DISPLAY_TABLE: &str = "DBFilesClient\\CreatureDisplayInfo.dbc";
pub const MODEL_TABLE: &str = "DBFilesClient\\CreatureModelData.dbc";
/// labs/den-lite glide.rs MARKER / FORMAT.
pub const MARKER: &str = "den-creature-models.json";
pub const FORMAT: &str = "leyline-den-creature-models";
const DISPLAY_FIELDS: u32 = 16;
const MODEL_FIELDS: u32 = 28;

/// A gameobject display the creature rows are made for: its id, model file and box (min x y z, max x y z).
#[derive(Debug, Clone, PartialEq)]
pub struct Prop {
    pub display: u32,
    pub model: String,
    pub bbox: [f32; 6],
}

/// The gameobject displays of a GameObjectDisplayInfo table that a creature can wear: an `.m2`-family model (not a
/// `.wmo`), an id below [`DISPLAY_BASE`], a sane path.
pub fn props(godi: &[u8]) -> Result<Vec<Prop>, String> {
    let t = Dbc::parse(godi).map_err(|e| format!("GameObjectDisplayInfo: {}", e.0))?;
    if t.fields < 2 || t.record_size != t.fields * 4 {
        return Err("GameObjectDisplayInfo isn't the shape Leyline knows.".into());
    }
    let mut out = Vec::new();
    for r in &t.records {
        let id = Dbc::get(r, 0);
        let path = t.string(Dbc::get(r, 1));
        let low = path.to_ascii_lowercase();
        if id == 0 || id >= DISPLAY_BASE || path.is_empty() || path.len() > 250 || low.contains("..") || ![".m2", ".mdx", ".mdl"].iter().any(|e| low.ends_with(e)) {
            continue;
        }
        let mut bbox = [0f32; 6];
        if t.fields >= 18 {
            for (i, b) in bbox.iter_mut().enumerate() {
                let v = Dbc::getf(r, 12 + i);
                *b = if v.is_finite() && v.abs() < 10_000.0 { v } else { 0.0 };
            }
        }
        out.push(Prop { display: id, model: path, bbox });
    }
    Ok(out)
}

/// The player's own CreatureDisplayInfo and CreatureModelData with a row per prop: (display table, model table, the
/// displays that got rows). Refused (the tables left alone) when either isn't the 3.3.5a shape, or already has an id
/// at or past [`DISPLAY_BASE`] (another mod's rows: never two rows with one id).
pub fn tables(display_stock: &[u8], model_stock: &[u8], props: &[Prop]) -> Result<(Vec<u8>, Vec<u8>, Vec<u32>), String> {
    let mut cdi = Dbc::parse(display_stock).map_err(|e| format!("CreatureDisplayInfo: {}", e.0))?;
    let mut cmd = Dbc::parse(model_stock).map_err(|e| format!("CreatureModelData: {}", e.0))?;
    if !cdi.shaped(DISPLAY_FIELDS) {
        return Err(format!("CreatureDisplayInfo has {} fields, not {DISPLAY_FIELDS}: left alone.", cdi.fields));
    }
    if !cmd.shaped(MODEL_FIELDS) {
        return Err(format!("CreatureModelData has {} fields, not {MODEL_FIELDS}: left alone.", cmd.fields));
    }
    if cdi.max_id() >= DISPLAY_BASE || cmd.max_id() >= DISPLAY_BASE {
        return Err("The game's creature tables already use Leyline's numbers (another mod): left alone.".into());
    }
    let mut done = Vec::with_capacity(props.len());
    for p in props {
        let Some(id) = p.display.checked_add(DISPLAY_BASE).filter(|_| p.display > 0 && p.display < DISPLAY_BASE) else { continue };
        if done.contains(&p.display) {
            continue;
        }
        // the model's row: its file, scale 1, its box (a creature's collision is never used: it can't be touched)
        let name = cmd.add_string(&p.model);
        let mut m = vec![0u8; (MODEL_FIELDS * 4) as usize];
        Dbc::set(&mut m, 0, id);
        Dbc::set(&mut m, 2, name);
        Dbc::setf(&mut m, 4, 1.0);
        let b = p.bbox;
        let (w, h) = (((b[3] - b[0]).abs().max((b[4] - b[1]).abs())).clamp(0.1, 50.0), (b[5] - b[2]).abs().clamp(0.1, 50.0));
        Dbc::setf(&mut m, 14, w);
        Dbc::setf(&mut m, 15, h);
        for (i, v) in b.iter().enumerate() {
            Dbc::setf(&mut m, 17 + i, *v);
        }
        Dbc::setf(&mut m, 23, 1.0);
        Dbc::setf(&mut m, 24, 1.0);
        cmd.records.push(m);
        // the display's row: that model, scale 1, fully opaque, no textures of its own (the model's are used)
        let mut d = vec![0u8; (DISPLAY_FIELDS * 4) as usize];
        Dbc::set(&mut d, 0, id);
        Dbc::set(&mut d, 1, id);
        Dbc::setf(&mut d, 4, 1.0);
        Dbc::set(&mut d, 5, 255);
        cdi.records.push(d);
        done.push(p.display);
    }
    Ok((cdi.write(), cmd.write(), done))
}

/// `<dens>/den-creature-models.json`: the displays Den Lite may glide.
pub fn marker(displays: &[u32]) -> String {
    serde_json::json!({ "format": FORMAT, "version": 1, "base": DISPLAY_BASE, "displays": displays }).to_string()
}

/// The two tables for the Den's patch-T, from the player's own archives (`order`, the game's reading order without
/// our slots) and the gameobject table the patch has (`godi`: the player's own with the Den's rows). Err: why not
/// (the patch then goes without them, and Den Lite's copy is a gameobject as before).
pub fn patch_files(order: &[std::path::PathBuf], godi: &[u8]) -> Result<(Vec<(String, Vec<u8>)>, Vec<u32>), String> {
    let read = |name: &str| crate::mpq::read_own(order, name).ok().flatten().ok_or_else(|| format!("your game has no {}", name.rsplit('\\').next().unwrap_or(name)));
    let (cdi, cmd) = (read(DISPLAY_TABLE)?, read(MODEL_TABLE)?);
    let (cdi, cmd, done) = tables(&cdi, &cmd, &props(godi)?)?;
    if done.is_empty() {
        return Err("no prop models to give a creature".into());
    }
    Ok((vec![(DISPLAY_TABLE.to_string(), cdi), (MODEL_TABLE.to_string(), cmd)], done))
}

/// Writes (or, with None, takes away) the marker in the Dens folder.
pub fn write_marker(dens: &Path, displays: Option<&[u32]>) {
    let p = dens.join(MARKER);
    match displays {
        Some(d) => {
            let _ = std::fs::create_dir_all(dens);
            let _ = crate::settings::write_atomic(&p, marker(d).as_bytes());
        }
        None => {
            let _ = std::fs::remove_file(&p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table of `fields` fields: rows of u32s (padded with 0), and its strings.
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

    /// A 19-field GameObjectDisplayInfo: a chair (.mdx, a box), a barrel (.m2), an inn (.wmo), a display past our base.
    fn godi() -> Vec<u8> {
        let s = b"\0World\\Generic\\Chair01.mdx\0World\\Generic\\Barrel.m2\0World\\wmo\\Inn.wmo\0World\\High.m2\0";
        let at = |n: &[u8]| s.windows(n.len()).position(|w| w == n).unwrap() as u32;
        let mut chair = vec![39, at(b"World\\Generic\\Chair01")];
        chair.resize(12, 0);
        chair.extend([(-0.5f32).to_bits(), (-0.4f32).to_bits(), 0f32.to_bits(), 0.5f32.to_bits(), 0.4f32.to_bits(), 1.2f32.to_bits()]);
        table(19, &[chair, vec![40, at(b"World\\Generic\\Barrel")], vec![9007, at(b"World\\wmo")], vec![80_000, at(b"World\\High")]], s)
    }

    #[test]
    fn the_props_a_creature_can_wear_m2_family_only() {
        let p = props(&godi()).unwrap();
        assert_eq!(p.iter().map(|p| p.display).collect::<Vec<_>>(), vec![39, 40], "not the building, not past our base");
        assert_eq!(p[0].model, "World\\Generic\\Chair01.mdx");
        assert_eq!(p[0].bbox, [-0.5, -0.4, 0.0, 0.5, 0.4, 1.2]);
        assert!(props(b"nope").is_err());
    }

    #[test]
    fn a_row_in_each_table_per_prop_with_the_3_3_5a_layouts() {
        let cdi = table(16, &[vec![1, 1], vec![30_000, 2]], &[]);
        let cmd = table(28, &[vec![1, 0, 1]], b"\0Creature\\Bear\\Bear.mdx\0");
        let (d, m, done) = tables(&cdi, &cmd, &props(&godi()).unwrap()).unwrap();
        assert_eq!(done, vec![39, 40]);
        let d = Dbc::parse(&d).unwrap();
        let m = Dbc::parse(&m).unwrap();
        assert!(d.shaped(16) && m.shaped(28) && d.record_size == 64 && m.record_size == 112);
        assert_eq!((d.records.len(), m.records.len()), (4, 3), "the stock rows kept, two more each");
        let row = d.row(70_039).expect("display 70039");
        assert_eq!((Dbc::get(row, 1), Dbc::getf(row, 4), Dbc::get(row, 5), Dbc::get(row, 6), Dbc::get(row, 9)), (70_039, 1.0, 255, 0, 0));
        let mrow = m.row(70_039).expect("model 70039");
        assert_eq!(m.string(Dbc::get(mrow, 2)), "World\\Generic\\Chair01.mdx");
        assert_eq!(Dbc::getf(mrow, 4), 1.0);
        assert_eq!((17..23).map(|i| Dbc::getf(mrow, i)).collect::<Vec<_>>(), vec![-0.5, -0.4, 0.0, 0.5, 0.4, 1.2]);
        assert_eq!((Dbc::getf(mrow, 14), Dbc::getf(mrow, 15), Dbc::getf(mrow, 23)), (1.0, 1.2, 1.0));
        assert_eq!(m.string(Dbc::get(m.row(1).unwrap(), 2)), "Creature\\Bear\\Bear.mdx", "the game's own row as it was");
        assert_eq!(m.string(Dbc::get(m.row(70_040).unwrap(), 2)), "World\\Generic\\Barrel.m2");
        // a table of another shape, or already past our base: left alone
        assert!(tables(&table(15, &[vec![1]], &[]), &cmd, &[]).is_err());
        assert!(tables(&cdi, &table(27, &[vec![1]], &[]), &[]).is_err());
        assert!(tables(&table(16, &[vec![70_001]], &[]), &cmd, &[]).unwrap_err().contains("another mod"));
        // the marker Den Lite reads
        let v: serde_json::Value = serde_json::from_str(&marker(&done)).unwrap();
        assert_eq!((v["format"].as_str(), v["base"].as_u64(), v["displays"][1].as_u64()), (Some(FORMAT), Some(70_000), Some(40)));
        // a folder of its own (no tempfile crate: Den Lite builds this file too, labs/den-lite lib.rs `creaturemodels`)
        let dir = std::env::temp_dir().join(format!("leyline-creaturemodels-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_marker(&dir, Some(&done));
        assert!(dir.join(MARKER).is_file());
        write_marker(&dir, None);
        assert!(!dir.join(MARKER).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
