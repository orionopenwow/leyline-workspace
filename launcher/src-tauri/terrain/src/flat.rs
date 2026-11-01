//! The delta in the form Forge writes and reads: docs/terrain-delta.schema.json as the Forge side defined it
//! (forge/docs/DEN-MODE.md, "The terrain delta"; only its documentation was read, never its code). Same format name
//! and version as [`crate::delta`]; differences: the tile is in `base` (`own:<map>/<x>/<y>`), each chunk entry is the
//! chunk's whole state, water is one flat layer with a 64-bit cell mask, and `models` are additions and moves by uid
//! (positions `x, y, z` from the Den's middle, rotations `rx, ry, rz`).
//!
//! Leyline keeps its own richer form ([`crate::delta::Delta`]) as the Den's ground; this module turns Forge's file into
//! it ([`apply_flat`] then a diff against the player's own tile) and makes the file Forge is handed ([`to_flat`]), so
//! Forge keeps the Den's base, open area and pad.

use crate::adt::{Adt, ChunkWater, Layer, WaterLayer};
use crate::delta::{self, models_of, norm, set_models, Delta, Made, Model, TileRef};
use crate::{b64, clean_path, err, Error, Result};
use serde::{Deserialize, Serialize};

/// The Den's middle, the default origin of Forge's model positions (`--origin`).
pub const ORIGIN: [f64; 3] = [16226.2, 16257.0, 13.2];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatPad {
    pub height: f32,
    pub texture: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatLayer {
    pub tex: String,
    pub flags: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatWater {
    #[serde(rename = "type")]
    pub kind: u16,
    pub min: f32,
    pub max: f32,
    pub mask: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatChunk {
    pub i: u16,
    pub heights: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normals: Option<bool>,
    pub layers: Vec<FlatLayer>,
    pub holes: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water: Option<FlatWater>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colors: Option<String>,
    pub area: u32,
    pub impassable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatModel {
    pub path: String,
    pub uid: u32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub rx: f32,
    pub ry: f32,
    pub rz: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flat {
    pub format: String,
    pub version: u32,
    pub base: String,
    pub open: Vec<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pad: Option<FlatPad>,
    pub chunks: Vec<FlatChunk>,
    pub models: Vec<FlatModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub made: Option<Made>,
}

/// The folder name of a map the Den may use (the client's own Map.dbc names; the Den is map 1).
pub fn map_dir(map: u32) -> Option<&'static str> {
    match map {
        0 => Some("Azeroth"),
        1 => Some("Kalimdor"),
        530 => Some("Expansion01"),
        571 => Some("Northrend"),
        _ => None,
    }
}

impl Flat {
    pub fn parse(text: &str) -> Result<Flat> {
        if text.len() > delta::MAX_BYTES {
            return err("That ground file is too big (4 MB at most).");
        }
        let f: Flat = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| Error(format!("That isn't a ground file: {e}")))?;
        if f.format != delta::FORMAT || f.version != delta::VERSION {
            return err("That isn't a ground file Leyline reads.");
        }
        let mut seen = [false; 256];
        for &i in &f.open {
            if i > 255 || std::mem::replace(&mut seen[i as usize], true) {
                return err("That ground file's open area isn't right.");
            }
        }
        if f.chunks.len() > 256 || f.models.len() > delta::MAX_MODELS {
            return err("That ground file is too big.");
        }
        for c in &f.chunks {
            if c.i > 255 || b64::to_f32s(&c.heights, 145).map_or(true, |h| !h.iter().all(|v| v.is_finite() && v.abs() <= delta::MAX_HEIGHT)) {
                return err(format!("Chunk {} of that ground file has bad heights.", c.i));
            }
            if c.layers.len() > 4 {
                return err("A chunk has at most 4 texture layers.");
            }
            for (k, l) in c.layers.iter().enumerate() {
                let alpha_ok = match (&l.alpha, k) {
                    (None, 0) => true,
                    (Some(a), k) if k > 0 => b64::decode(a).is_some_and(|v| v.len() == 4096),
                    _ => false,
                };
                if clean_path(&l.tex).is_none() || !alpha_ok {
                    return err(format!("Chunk {} of that ground file has a bad texture layer.", c.i));
                }
            }
            if c.water.as_ref().is_some_and(|w| b64::decode(&w.mask).map_or(true, |m| m.len() != 8) || !w.min.is_finite() || !w.max.is_finite()) {
                return err(format!("Chunk {} of that ground file has bad water.", c.i));
            }
            if c.colors.as_ref().is_some_and(|cv| b64::decode(cv).map_or(true, |v| v.len() != 580)) {
                return err(format!("Chunk {} of that ground file has bad colours.", c.i));
            }
        }
        for m in &f.models {
            let fin = [m.x, m.y, m.z].iter().all(|v| v.is_finite() && v.abs() < 1e6) && [m.rx, m.ry, m.rz].iter().all(|v| v.is_finite());
            let low = m.path.to_ascii_lowercase();
            if clean_path(&m.path).is_none() || !fin || !(m.scale > 0.0 && m.scale <= 64.0) || ![".m2", ".mdx", ".mdl", ".wmo"].iter().any(|e| low.ends_with(e)) {
                return err("A model in that ground file isn't right.");
            }
        }
        Ok(f)
    }

    /// The tile `base` names (`own:<map>/<x>/<y>`), when it names one.
    pub fn tile(&self) -> Option<TileRef> {
        let rest = self.base.strip_prefix("own:")?;
        let v: Vec<u32> = rest.split('/').map(|p| p.parse().ok()).collect::<Option<_>>()?;
        if v.len() != 3 || v[1] > 63 || v[2] > 63 {
            return None;
        }
        Some(TileRef { map: v[0], dir: map_dir(v[0])?.into(), x: v[1], y: v[2] })
    }
}

fn mask_of(l: &WaterLayer) -> u64 {
    let mut m = 0u64;
    for r in 0..l.h as usize {
        for c in 0..l.w as usize {
            let bit = r * l.w as usize + c;
            let on = l.mask.as_ref().map_or(true, |mk| mk.get(bit / 8).is_some_and(|b| b >> (bit % 8) & 1 == 1));
            if on {
                m |= 1u64 << ((l.y as usize + r) * 8 + l.x as usize + c);
            }
        }
    }
    m
}

/// Forge's file into a tile (the tile Forge was given: the player's own plus the Den's ground).
pub fn apply_flat(f: &Flat, a: &mut Adt, origin: [f64; 3]) -> Result<()> {
    let mut moved = [false; 256];
    for c in &f.chunks {
        let i = c.i as usize;
        let h = b64::to_f32s(&c.heights, 145).ok_or_else(|| Error("bad heights".into()))?;
        if a.chunks[i].abs_heights().iter().zip(&h).any(|(x, y)| (x - y).abs() > 1e-3) {
            a.chunks[i].set_abs_heights(&h);
            moved[i] = true;
        }
        let before: Vec<(String, u32)> = a.chunks[i].layers.iter().map(|l| (norm(a.textures.get(l.texture as usize).map(String::as_str).unwrap_or("")), l.effect)).collect();
        let mut layers = Vec::new();
        for (k, l) in c.layers.iter().enumerate() {
            let tex = clean_path(&l.tex).ok_or_else(|| Error("bad texture".into()))?;
            let effect = before.iter().find(|(t, _)| *t == norm(&tex)).map(|(_, e)| *e).unwrap_or(0);
            let texture = a.texture_index(&tex);
            let alpha = if k == 0 { None } else { Some(b64::decode(l.alpha.as_deref().unwrap_or("")).filter(|v| v.len() == 4096).ok_or_else(|| Error("bad alpha".into()))?) };
            layers.push(Layer { texture, flags: l.flags & !(crate::adt::LAYER_USE_ALPHA | crate::adt::LAYER_COMPRESSED), effect, alpha });
        }
        if layers.is_empty() {
            layers = a.chunks[i].layers.clone();
        }
        let ch = &mut a.chunks[i];
        let same_layers = ch.layers.len() == layers.len() && ch.layers.iter().zip(&layers).all(|(x, y)| x.texture == y.texture && x.flags & !0x300 == y.flags && x.alpha == y.alpha);
        if !same_layers {
            ch.layers = layers;
            ch.recompute_pred_tex();
        }
        ch.holes = c.holes;
        ch.area = c.area;
        ch.flags = if c.impassable { ch.flags | crate::adt::MCNK_IMPASSABLE } else { ch.flags & !crate::adt::MCNK_IMPASSABLE };
        ch.colors = match &c.colors {
            None => None,
            Some(t) => Some(b64::decode(t).filter(|v| v.len() == 580).ok_or_else(|| Error("bad colours".into()))?),
        };
        // water: one flat layer. The tile's own water stays as it is when Forge's says the same.
        let now = a.water.as_ref().map(|w| w[i].clone()).unwrap_or_default();
        let want = match &c.water {
            None => ChunkWater::default(),
            Some(w) => {
                let m = u64::from_le_bytes(b64::decode(&w.mask).filter(|v| v.len() == 8).ok_or_else(|| Error("bad water".into()))?.try_into().unwrap());
                let same = now.layers.first().is_some_and(|l| l.kind == w.kind && l.min == w.min && l.max == w.max && mask_of(l) == m);
                if same {
                    now.clone()
                } else {
                    ChunkWater {
                        layers: vec![WaterLayer { kind: w.kind, format: 0, min: w.min, max: w.max, x: 0, y: 0, w: 8, h: 8, mask: Some(m.to_le_bytes().to_vec()), heights: None, uv: None, depth: None }],
                        attributes: now.attributes,
                    }
                }
            }
        };
        if want != now {
            if a.water.is_none() {
                a.water = Some(vec![ChunkWater::default(); 256]);
            }
            a.water.as_mut().unwrap()[i] = want;
        }
    }
    let mut redo = [false; 256];
    for i in (0..256).filter(|&i| moved[i]) {
        let (r, c) = ((i / 16) as i32, (i % 16) as i32);
        for (dr, dc) in (-1..=1).flat_map(|a| (-1..=1).map(move |b| (a, b))) {
            if (0..16).contains(&(r + dr)) && (0..16).contains(&(c + dc)) {
                redo[((r + dr) * 16 + c + dc) as usize] = true;
            }
        }
    }
    for (i, r) in redo.iter().enumerate() {
        if *r {
            a.recompute_normals(i);
        }
    }
    if !f.models.is_empty() {
        let mut list = models_of(a, origin);
        for m in &f.models {
            let pos = [m.x, m.y, m.z];
            match list.iter_mut().find(|x| x.uid == Some(m.uid)) {
                Some(x) => {
                    if let Some(b) = x.bounds.as_mut() {
                        let d = [pos[0] - x.pos[0], pos[1] - x.pos[1], pos[2] - x.pos[2]];
                        for k in 0..6 {
                            b[k] += d[k % 3];
                        }
                    }
                    x.path = m.path.clone();
                    x.pos = pos;
                    x.rot = Some([m.rx, m.ry, m.rz]);
                    if !x.is_wmo() {
                        x.scale = Some(m.scale);
                    }
                }
                None => list.push(Model {
                    path: m.path.clone(),
                    uid: Some(m.uid),
                    pos,
                    rot: Some([m.rx, m.ry, m.rz]),
                    scale: (!m.path.to_ascii_lowercase().ends_with(".wmo")).then_some(m.scale),
                    flags: None,
                    set: None,
                    name_set: None,
                    bounds: None,
                }),
            }
        }
        set_models(a, &list, origin);
    }
    Ok(())
}

/// The Den's ground from Forge's file: Forge's changes applied to the tile it was given (`given`: the player's own
/// tile plus `current`), then the difference from the player's own tile (`own`), inside `current`'s open area.
pub fn to_delta(f: &Flat, own: &Adt, current: &Delta) -> Result<(Delta, delta::DiffReport)> {
    if let Some(t) = f.tile() {
        if t.map != current.tile.map || t.x != current.tile.x || t.y != current.tile.y {
            return err("The editor saved another tile than your Den's.");
        }
    }
    let mut given = own.clone();
    delta::apply_to(current, &mut given)?;
    let origin = current.origin.unwrap_or(ORIGIN);
    apply_flat(f, &mut given, origin)?;
    let mut o = delta::DiffOptions::new(current.tile.clone());
    o.open = Some(current.open.clone());
    o.pad = current.pad.clone();
    o.origin = Some(origin);
    o.base = current.base.clone();
    o.made_by = Some("forge".into());
    o.made_at = f.made.as_ref().and_then(|m| m.at.clone());
    delta::diff_tiles(own, &given, &o)
}

/// The file Forge is handed with the tile (so it keeps the Den's base, open area and pad): every open chunk's whole
/// state, and the models that differ from the player's own tile.
pub fn to_flat(current: &Delta, own: &Adt) -> Result<Flat> {
    let mut t = own.clone();
    delta::apply_to(current, &mut t)?;
    let origin = current.origin.unwrap_or(ORIGIN);
    let mut chunks = Vec::new();
    let mut open = current.open.clone();
    open.sort_unstable();
    for &i in &open {
        let c = &t.chunks[i as usize];
        let w = t.water.as_ref().and_then(|w| w[i as usize].layers.first().cloned());
        chunks.push(FlatChunk {
            i,
            heights: b64::f32s(&c.abs_heights()),
            normals: Some(false),
            layers: c
                .layers
                .iter()
                .enumerate()
                .map(|(k, l)| FlatLayer {
                    tex: t.textures.get(l.texture as usize).cloned().unwrap_or_default(),
                    flags: l.flags & !(crate::adt::LAYER_USE_ALPHA | crate::adt::LAYER_COMPRESSED),
                    alpha: (k > 0).then(|| b64::encode(l.alpha.as_deref().unwrap_or(&[0; 4096]))),
                })
                .collect(),
            holes: c.holes,
            water: w.map(|l| FlatWater { kind: l.kind, min: l.min, max: l.max, mask: b64::encode(&mask_of(&l).to_le_bytes()) }),
            colors: c.colors.as_deref().map(b64::encode),
            area: c.area,
            impassable: c.flags & crate::adt::MCNK_IMPASSABLE != 0,
        });
    }
    let base_models = models_of(own, origin);
    let models = models_of(&t, origin)
        .into_iter()
        .filter(|m| !base_models.iter().any(|b| b == m))
        .map(|m| FlatModel {
            uid: m.uid.unwrap_or(0),
            x: m.pos[0],
            y: m.pos[1],
            z: m.pos[2],
            rx: m.rot.map_or(0.0, |r| r[0]),
            ry: m.rot.map_or(0.0, |r| r[1]),
            rz: m.rot.map_or(0.0, |r| r[2]),
            scale: m.scale.unwrap_or(1.0),
            path: m.path,
        })
        .collect();
    let base = match current.base.as_str() {
        "own" => format!("own:{}/{}/{}", current.tile.map, current.tile.x, current.tile.y),
        b => b.to_string(),
    };
    Ok(Flat {
        format: delta::FORMAT.into(),
        version: delta::VERSION,
        base,
        open,
        pad: current.pad.as_ref().map(|p| FlatPad { height: p.height, texture: p.texture.clone() }),
        chunks,
        models,
        made: Some(Made { by: Some("leyline".into()), at: None }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adt::tests::sample;
    use crate::pad;

    fn current(own: &Adt) -> Delta {
        let p = delta::Pad { height: pad::DEN.2, texture: pad::PAD_TEXTURE.into(), centre: None };
        pad::make_pad(&own.write(), pad::den_tile(), p, 0).unwrap()
    }

    #[test]
    fn forges_file_reads_and_turns_into_the_dens_ground() {
        let own = sample();
        let cur = current(&own);
        // what Leyline hands Forge, as Forge's form: it reads back the same
        let given = to_flat(&cur, &own).unwrap();
        let text = serde_json::to_string(&given).unwrap();
        assert!(text.contains("\"base\":\"own:1/1/1\"") && text.contains("\"normals\":false"));
        let back = Flat::parse(&text).unwrap();
        assert_eq!(back, given);
        assert_eq!(back.tile(), Some(pad::den_tile()));
        // handed back unchanged: the same ground
        let same = to_delta(&back, &own, &cur).unwrap().0;
        assert_eq!((&same.chunks, &same.models, &same.open, &same.pad), (&cur.chunks, &cur.models, &cur.open, &cur.pad));
        // Forge raised chunk 136 by 4 yards, put water in 151 and a new tree by the room, moved the hut
        let mut f = back.clone();
        let c = f.chunks.iter_mut().find(|c| c.i == 136).unwrap();
        let h: Vec<f32> = b64::to_f32s(&c.heights, 145).unwrap().iter().map(|v| v + 4.0).collect();
        c.heights = b64::f32s(&h);
        f.chunks.iter_mut().find(|c| c.i == 151).unwrap().water = Some(FlatWater { kind: 1, min: 12.0, max: 12.0, mask: b64::encode(&u64::MAX.to_le_bytes()) });
        f.models = vec![
            FlatModel { path: "World\\Leyline\\Tree.m2".into(), uid: 0x7F00_0100, x: 3.0, y: -2.0, z: 0.5, rx: 0.0, ry: 30.0, rz: 0.0, scale: 1.25 },
            FlatModel { path: "World\\Leyline\\Hut.wmo".into(), uid: 0x7F00_0003, x: -6.2 + 5.0, y: -7.0, z: -0.2, rx: 0.0, ry: 45.0, rz: 0.0, scale: 1.0 },
        ];
        // a chunk outside the open area is left out
        let mut far = f.chunks[0].clone();
        far.i = 0;
        far.holes = 0xFFFF;
        f.chunks.push(far);
        let (d, rep) = to_delta(&f, &own, &cur).unwrap();
        assert_eq!(rep.ignored, vec![0]);
        assert_eq!(d.open, cur.open);
        let t = Adt::parse(&delta::apply(&d, Some(&own.write()), None).unwrap(), None).unwrap();
        assert!((t.chunks[136].abs_heights()[0] - (pad::DEN.2 + 4.0)).abs() < 1e-3);
        assert_eq!(t.water.as_ref().unwrap()[151].layers[0].kind, 1);
        assert_eq!(t.chunks[0].holes, own.chunks[0].holes);
        let ms = models_of(&t, ORIGIN);
        assert_eq!(ms.len(), 4, "the base's three and the new tree");
        let tree = ms.iter().find(|m| m.uid == Some(0x7F00_0100)).unwrap();
        assert!((tree.pos[0] - 3.0).abs() < 1e-3 && tree.scale == Some(1.25));
        let hut = ms.iter().find(|m| m.uid == Some(0x7F00_0003)).unwrap();
        let old = models_of(&own, ORIGIN).into_iter().find(|m| m.uid == Some(0x7F00_0003)).unwrap();
        assert!((hut.pos[0] - old.pos[0] - 5.0).abs() < 1e-3);
        assert!((hut.bounds.unwrap()[0] - old.bounds.unwrap()[0] - 5.0).abs() < 1e-2, "its bounds moved with it");
    }

    /// With LEYLINE_TERRAIN_SAMPLES=<dir>: both forms written there, to check against the JSON schemas
    /// (docs/terrain-delta.schema.json, docs/terrain-delta-leyline.schema.json) with any schema validator.
    #[test]
    fn samples_for_the_schemas() {
        let Ok(dir) = std::env::var("LEYLINE_TERRAIN_SAMPLES") else { return };
        let own = sample();
        let mut cur = current(&own);
        cur.made = Some(Made { by: Some("leyline".into()), at: Some("2026-11-01T05:00:00Z".into()) });
        let mut t = own.clone();
        delta::apply_to(&cur, &mut t).unwrap();
        t.chunks[136].holes = 3;
        t.doodads[0].pos[1] += 1.0;
        let mut o = delta::DiffOptions::new(pad::den_tile());
        o.open = Some(cur.open.clone());
        o.pad = cur.pad.clone();
        o.origin = Some(ORIGIN);
        let rich = delta::diff_tiles(&own, &t, &o).unwrap().0;
        std::fs::write(format!("{dir}/leyline.terrain.json"), rich.to_json().unwrap()).unwrap();
        std::fs::write(format!("{dir}/forge.terrain.json"), serde_json::to_string_pretty(&to_flat(&rich, &own).unwrap()).unwrap()).unwrap();
    }

    #[test]
    fn forges_file_is_checked() {
        let ok = r#"{"format":"leyline-terrain","version":1,"base":"own:1/1/1","open":[1],"chunks":[],"models":[]}"#;
        assert!(Flat::parse(ok).is_ok());
        for bad in [
            r#"{"format":"leyline-terrain","version":1,"base":"own:1/1/1","open":[1],"chunks":[]}"#,
            r#"{"format":"leyline-terrain","version":1,"base":"own:1/1/1","open":[1,1],"chunks":[],"models":[]}"#,
            r#"{"format":"leyline-terrain","version":1,"base":"own:1/1/1","open":[1],"chunks":[{"i":1,"heights":"AAAA","layers":[],"holes":0,"area":0,"impassable":false}],"models":[]}"#,
            r#"{"format":"leyline-terrain","version":1,"base":"own:1/1/1","open":[1],"chunks":[],"models":[{"path":"x.exe","uid":1,"x":0,"y":0,"z":0,"rx":0,"ry":0,"rz":0,"scale":1}]}"#,
            r#"{"format":"leyline-terrain","version":1,"base":"own:1/1/1","open":[1],"chunks":[],"models":[],"run":"x"}"#,
        ] {
            assert!(Flat::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(Flat::parse(ok).unwrap().tile().unwrap().dir, "Kalimdor");
        let mut f = Flat::parse(ok).unwrap();
        f.base = "own:1/2/1".into();
        let own = sample();
        assert!(to_delta(&f, &own, &current(&own)).unwrap_err().0.contains("another tile"));
    }
}
