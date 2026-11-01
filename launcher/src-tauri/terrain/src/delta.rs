//! The terrain delta (`<den>.terrain.json`; docs/terrain-delta-leyline.schema.json; Forge's form: crate::flat, docs/NOGGIT-DEN-PLAN.md 2.5): a Den's
//! ground as numbers and file names only. [`diff`] makes one from the player's own tile and the edited tile;
//! [`apply`] rebuilds the edited tile from anyone's own tile plus the delta. Data only: read by a strict parser
//! (unknown keys refused), checked field by field, never run.

use crate::adt::{self, Adt, AlphaMode, ChunkWater, Doodad, Layer, WaterLayer, WmoPlacement};
use crate::{b64, clean_path, err, Error, Result, PLACE};
use serde::{Deserialize, Deserializer, Serialize};

pub const FORMAT: &str = "leyline-terrain";
pub const VERSION: u32 = 1;
/// The largest delta (bytes of JSON).
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_MODELS: usize = 8192;
/// Heights a delta may hold (world yards).
pub const MAX_HEIGHT: f32 = 20_000.0;
/// Where ids of models Leyline adds begin (plan 2.4).
pub const UID_BASE: u32 = 0x7F00_0000;
/// Two heights closer than this are the same.
const EPS: f32 = 1e-3;

fn own() -> String {
    "own".into()
}
fn is_own(s: &str) -> bool {
    s == "own"
}
/// `null` is Some(None), a value Some(Some(v)), a missing key None.
fn double<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> std::result::Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileRef {
    pub map: u32,
    pub dir: String,
    pub x: u32,
    pub y: u32,
}

impl TileRef {
    /// The tile's file inside the archives: `World\Maps\<dir>\<dir>_<x>_<y>.adt`.
    pub fn adt_path(&self) -> String {
        format!("World\\Maps\\{0}\\{0}_{1}_{2}.adt", self.dir, self.x, self.y)
    }
    pub fn wdt_path(&self) -> String {
        format!("World\\Maps\\{0}\\{0}.wdt", self.dir)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pad {
    pub height: f32,
    pub texture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centre: Option<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Made {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerDelta {
    pub tex: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub flags: u32,
    /// The ground effect (left out: the tile's own layer of that texture keeps its effect, else none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<String>,
}
fn is_zero(v: &u32) -> bool {
    *v == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterLayerDelta {
    #[serde(rename = "type")]
    pub kind: u16,
    #[serde(default)]
    pub format: u16,
    pub min: f32,
    pub max: f32,
    #[serde(default)]
    pub x: u8,
    #[serde(default)]
    pub y: u8,
    #[serde(default = "eight")]
    pub w: u8,
    #[serde(default = "eight")]
    pub h: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heights: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<String>,
}
fn eight() -> u8 {
    8
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fishable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep: Option<String>,
    pub layers: Vec<WaterLayerDelta>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ChunkDelta {
    pub i: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xy: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heights: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<LayerDelta>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holes: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impassable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double")]
    pub colors: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double")]
    pub water: Option<Option<WaterDelta>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,
    pub pos: [f64; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rot: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_set: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[f64; 6]>,
}

impl Model {
    pub fn is_wmo(&self) -> bool {
        self.path.to_ascii_lowercase().ends_with(".wmo")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delta {
    pub format: String,
    pub version: u32,
    #[serde(default = "own", skip_serializing_if = "is_own")]
    pub base: String,
    pub tile: TileRef,
    pub open: Vec<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pad: Option<Pad>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<[f64; 3]>,
    #[serde(default)]
    pub chunks: Vec<ChunkDelta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub made: Option<Made>,
}

impl Delta {
    /// An empty delta for a tile (nothing changed yet).
    pub fn new(tile: TileRef, open: Vec<u16>) -> Delta {
        Delta { format: FORMAT.into(), version: VERSION, base: own(), tile, open, pad: None, origin: None, chunks: Vec::new(), models: None, made: None }
    }

    /// Reads and checks a delta's text.
    pub fn parse(text: &str) -> Result<Delta> {
        if text.len() > MAX_BYTES {
            return err("That ground file is too big (4 MB at most).");
        }
        let d: Delta = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| Error(format!("That isn't a ground file: {e}")))?;
        d.check()?;
        Ok(d)
    }

    pub fn to_json(&self) -> Result<String> {
        let t = serde_json::to_string_pretty(self).map_err(|e| Error(e.to_string()))?;
        if t.len() > MAX_BYTES {
            return err("The ground changes are too big for one file (4 MB at most): change a smaller area.");
        }
        Ok(t)
    }

    /// Nothing differs from the base.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty() && self.models.is_none()
    }

    /// Every rule of the format, field by field.
    pub fn check(&self) -> Result<()> {
        if self.format != FORMAT {
            return err("That isn't a ground file.");
        }
        if self.version != VERSION {
            return err(if self.version > VERSION { "That ground file is from a newer Leyline." } else { "That ground file has no version." });
        }
        let base_ok = self.base == "own"
            || self.base == "blank"
            || self.base.strip_prefix("pack:").is_some_and(|p| !p.is_empty() && p.len() <= 24 && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        if !base_ok {
            return err("That ground file's base isn't one Leyline knows.");
        }
        let t = &self.tile;
        if t.x > 63 || t.y > 63 || t.map > 65535 || t.dir.is_empty() || t.dir.len() > 64 || !t.dir.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ' ' | '\'' | '-')) {
            return err("That ground file's tile isn't right.");
        }
        let mut seen = [false; 256];
        for &i in &self.open {
            if i > 255 || std::mem::replace(&mut seen[i as usize], true) {
                return err("That ground file's open area isn't right.");
            }
        }
        if let Some(p) = &self.pad {
            if !p.height.is_finite() || p.height.abs() > MAX_HEIGHT || clean_path(&p.texture).is_none() || p.centre.is_some_and(|c| !c.iter().all(|v| v.is_finite())) {
                return err("That ground file's pad isn't right.");
            }
        }
        if self.origin.is_some_and(|o| !o.iter().all(|v| v.is_finite() && v.abs() < 1e6)) {
            return err("That ground file's origin isn't right.");
        }
        if self.chunks.len() > 256 {
            return err("That ground file has too many chunks.");
        }
        let mut had = [false; 256];
        for c in &self.chunks {
            let i = c.i as usize;
            if i > 255 || std::mem::replace(&mut had[i], true) {
                return err("That ground file names a chunk twice.");
            }
            if !seen[i] {
                return err(format!("That ground file changes chunk {i}, outside its open area."));
            }
            check_chunk(c).map_err(|e| Error(format!("Chunk {i}: {e}")))?;
        }
        if let Some(ms) = &self.models {
            if ms.len() > MAX_MODELS {
                return err(format!("A ground file holds up to {MAX_MODELS} models."));
            }
            for m in ms {
                let low = m.path.to_ascii_lowercase();
                let ext_ok = [".m2", ".mdx", ".mdl", ".wmo"].iter().any(|e| low.ends_with(e));
                let fin = m.pos.iter().all(|v| v.is_finite() && v.abs() < 1e6)
                    && m.rot.map_or(true, |r| r.iter().all(|v| v.is_finite()))
                    && m.bounds.map_or(true, |b| b.iter().all(|v| v.is_finite() && v.abs() < 1e6));
                if clean_path(&m.path).is_none() || !ext_ok || !fin {
                    return err(format!("A model in that ground file isn't right ({}).", m.path.chars().take(80).collect::<String>()));
                }
                if m.scale.is_some_and(|s| !(s > 0.0 && s <= 64.0)) {
                    return err("A model's scale is out of range (above 0, at most 64).");
                }
            }
        }
        Ok(())
    }
}

fn check_chunk(c: &ChunkDelta) -> Result<()> {
    if let Some(h) = &c.heights {
        let v = b64::to_f32s(h, 145).ok_or_else(|| Error("its heights aren't 145 numbers".into()))?;
        if !v.iter().all(|x| x.is_finite() && x.abs() <= MAX_HEIGHT) {
            return err("a height is out of range");
        }
    }
    if c.xy.is_some_and(|xy| !xy.iter().all(|v| v.is_finite())) {
        return err("its corner isn't right");
    }
    if let Some(ls) = &c.layers {
        if ls.is_empty() || ls.len() > 4 {
            return err("a chunk has 1 to 4 texture layers");
        }
        for (k, l) in ls.iter().enumerate() {
            if clean_path(&l.tex).is_none() {
                return err("a texture name isn't right");
            }
            match (k, &l.alpha) {
                (0, Some(_)) => return err("the first layer has no alpha map"),
                (0, None) => {}
                (_, None) => return err("every layer after the first needs its alpha map"),
                (_, Some(a)) => {
                    if b64::decode(a).map(|v| v.len()) != Some(4096) {
                        return err("an alpha map is 64 x 64 bytes");
                    }
                }
            }
        }
    }
    if let Some(Some(cv)) = &c.colors {
        if b64::decode(cv).map(|v| v.len()) != Some(580) {
            return err("its vertex colours are 145 x 4 bytes");
        }
    }
    if let Some(Some(w)) = &c.water {
        water_from(w)?;
    }
    Ok(())
}

fn hex64(s: &Option<String>) -> Result<Option<u64>> {
    match s {
        None => Ok(None),
        Some(h) if h.len() == 16 => u64::from_str_radix(h, 16).map(Some).map_err(|_| Error("a water mask isn't 16 hex digits".into())),
        Some(_) => err("a water mask isn't 16 hex digits"),
    }
}

fn water_from(w: &WaterDelta) -> Result<ChunkWater> {
    if w.layers.is_empty() || w.layers.len() > 8 {
        return err("water has 1 to 8 layers");
    }
    let (f, d) = (hex64(&w.fishable)?, hex64(&w.deep)?);
    let attributes = if f.is_some() || d.is_some() { Some((f.unwrap_or(0), d.unwrap_or(0))) } else { None };
    let mut layers = Vec::new();
    for l in &w.layers {
        if !(1..=8).contains(&l.w) || !(1..=8).contains(&l.h) || l.x as u32 + l.w as u32 > 8 || l.y as u32 + l.h as u32 > 8 {
            return err("a water layer lies outside its chunk");
        }
        if !(l.min.is_finite() && l.max.is_finite() && l.min.abs() <= MAX_HEIGHT && l.max.abs() <= MAX_HEIGHT) {
            return err("a water height is out of range");
        }
        let n = (l.w as usize + 1) * (l.h as usize + 1);
        let bytes = |s: &Option<String>, len: usize, what: &str| -> Result<Option<Vec<u8>>> {
            match s {
                None => Ok(None),
                Some(t) => b64::decode(t).filter(|v| v.len() == len).map(Some).ok_or_else(|| Error(format!("a water layer's {what} has the wrong size"))),
            }
        };
        let heights = bytes(&l.heights, n * 4, "heights")?.map(|b| b.chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect::<Vec<f32>>());
        if heights.as_ref().is_some_and(|h| !h.iter().all(|v| v.is_finite() && v.abs() <= MAX_HEIGHT)) {
            return err("a water height is out of range");
        }
        layers.push(WaterLayer {
            kind: l.kind,
            format: l.format,
            min: l.min,
            max: l.max,
            x: l.x,
            y: l.y,
            w: l.w,
            h: l.h,
            mask: bytes(&l.mask, (l.w as usize * l.h as usize).div_ceil(8), "mask")?,
            heights,
            uv: bytes(&l.uv, n * 4, "uv")?.map(|b| b.chunks(4).map(|c| [u16::from_le_bytes([c[0], c[1]]), u16::from_le_bytes([c[2], c[3]])]).collect()),
            depth: bytes(&l.depth, n, "depth")?,
        });
    }
    Ok(ChunkWater { layers, attributes })
}

fn water_to(w: &ChunkWater) -> Option<WaterDelta> {
    if w.layers.is_empty() {
        return None;
    }
    Some(WaterDelta {
        fishable: w.attributes.map(|a| format!("{:016x}", a.0)),
        deep: w.attributes.map(|a| format!("{:016x}", a.1)),
        layers: w
            .layers
            .iter()
            .map(|l| WaterLayerDelta {
                kind: l.kind,
                format: l.format,
                min: l.min,
                max: l.max,
                x: l.x,
                y: l.y,
                w: l.w,
                h: l.h,
                mask: l.mask.as_deref().map(b64::encode),
                heights: l.heights.as_deref().map(b64::f32s),
                uv: l.uv.as_ref().map(|u| b64::encode(&u.iter().flat_map(|p| p[0].to_le_bytes().into_iter().chain(p[1].to_le_bytes())).collect::<Vec<u8>>())),
                depth: l.depth.as_deref().map(b64::encode),
            })
            .collect(),
    })
}

pub(crate) fn norm(p: &str) -> String {
    p.replace('/', "\\").to_ascii_lowercase()
}

/// A tile's placements as delta models (world coordinates minus `origin`), in file order: doodads, then buildings.
pub fn models_of(a: &Adt, origin: [f64; 3]) -> Vec<Model> {
    let p = PLACE as f64;
    let mut out = Vec::with_capacity(a.doodads.len() + a.wmos.len());
    for d in &a.doodads {
        out.push(Model {
            path: a.m2_names.get(d.name as usize).cloned().unwrap_or_default(),
            uid: Some(d.uid),
            pos: [p - d.pos[2] as f64 - origin[0], p - d.pos[0] as f64 - origin[1], d.pos[1] as f64 - origin[2]],
            rot: Some(d.rot),
            scale: Some(d.scale as f32 / 1024.0),
            flags: Some(d.flags),
            set: None,
            name_set: None,
            bounds: None,
        });
    }
    for w in &a.wmos {
        let lo = [p - w.upper[2] as f64 - origin[0], p - w.upper[0] as f64 - origin[1], w.lower[1] as f64 - origin[2]];
        let hi = [p - w.lower[2] as f64 - origin[0], p - w.lower[0] as f64 - origin[1], w.upper[1] as f64 - origin[2]];
        out.push(Model {
            path: a.wmo_names.get(w.name as usize).cloned().unwrap_or_default(),
            uid: Some(w.uid),
            pos: [p - w.pos[2] as f64 - origin[0], p - w.pos[0] as f64 - origin[1], w.pos[1] as f64 - origin[2]],
            rot: Some(w.rot),
            scale: None,
            flags: Some(w.flags),
            set: Some(w.doodad_set),
            name_set: Some(w.name_set),
            bounds: Some([lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]]),
        });
    }
    out
}

/// Replaces a tile's placements with `models` (names lists made again, ids given to models without one).
pub fn set_models(a: &mut Adt, models: &[Model], origin: [f64; 3]) {
    let p = PLACE as f64;
    let mut used: std::collections::HashSet<u32> = models.iter().filter_map(|m| m.uid).collect();
    let mut next = UID_BASE;
    let mut uid = |m: &Model| {
        m.uid.unwrap_or_else(|| {
            while used.contains(&next) {
                next = next.wrapping_add(1);
            }
            used.insert(next);
            next
        })
    };
    let (mut m2, mut wmo): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let index = |list: &mut Vec<String>, path: &str| -> u32 {
        let n = norm(path);
        match list.iter().position(|x| norm(x) == n) {
            Some(i) => i as u32,
            None => {
                list.push(path.replace('/', "\\"));
                (list.len() - 1) as u32
            }
        }
    };
    let (mut doodads, mut wmos) = (Vec::new(), Vec::new());
    for m in models {
        let (wx, wy, wz) = (m.pos[0] + origin[0], m.pos[1] + origin[1], m.pos[2] + origin[2]);
        let pos = [(p - wy) as f32, wz as f32, (p - wx) as f32];
        let rot = m.rot.unwrap_or([0.0; 3]);
        if m.is_wmo() {
            let b = m.bounds.unwrap_or([m.pos[0] - 10.0, m.pos[1] - 10.0, m.pos[2] - 10.0, m.pos[0] + 10.0, m.pos[1] + 10.0, m.pos[2] + 10.0]);
            let (lo, hi) = ([b[0] + origin[0], b[1] + origin[1], b[2] + origin[2]], [b[3] + origin[0], b[4] + origin[1], b[5] + origin[2]]);
            wmos.push(WmoPlacement {
                name: index(&mut wmo, &m.path),
                uid: uid(m),
                pos,
                rot,
                lower: [(p - hi[1]) as f32, lo[2] as f32, (p - hi[0]) as f32],
                upper: [(p - lo[1]) as f32, hi[2] as f32, (p - lo[0]) as f32],
                flags: m.flags.unwrap_or(0),
                doodad_set: m.set.unwrap_or(0),
                name_set: m.name_set.unwrap_or(0),
                pad: 0,
            });
        } else {
            let scale = (m.scale.unwrap_or(1.0) * 1024.0).round().clamp(1.0, 65535.0) as u16;
            doodads.push(Doodad { name: index(&mut m2, &m.path), uid: uid(m), pos, rot, scale, flags: m.flags.unwrap_or(0) });
        }
    }
    a.m2_names = m2;
    a.wmo_names = wmo;
    a.doodads = doodads;
    a.wmos = wmos;
    a.recompute_refs();
}

/// What [`diff`] needs besides the two tiles.
#[derive(Debug, Clone)]
pub struct DiffOptions {
    pub tile: TileRef,
    /// The chunks that may change (None: whatever changed).
    pub open: Option<Vec<u16>>,
    pub origin: Option<[f64; 3]>,
    pub pad: Option<Pad>,
    pub base: String,
    pub made_by: Option<String>,
    pub made_at: Option<String>,
    /// The map's alpha form (its WDT), when known.
    pub alpha: Option<AlphaMode>,
}

impl DiffOptions {
    pub fn new(tile: TileRef) -> DiffOptions {
        DiffOptions { tile, open: None, origin: None, pad: None, base: own(), made_by: None, made_at: None, alpha: None }
    }
}

/// What [`diff`] saw.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiffReport {
    /// Chunks in the delta.
    pub changed: Vec<u16>,
    /// Chunks that changed outside the open area: left out.
    pub ignored: Vec<u16>,
    pub models_changed: bool,
}

fn layers_of(a: &Adt, c: &adt::Mcnk) -> Vec<(String, u32, u32, Option<Vec<u8>>)> {
    c.layers
        .iter()
        .map(|l| (norm(a.textures.get(l.texture as usize).map(String::as_str).unwrap_or("")), l.flags & !(adt::LAYER_USE_ALPHA | adt::LAYER_COMPRESSED), l.effect, l.alpha.clone()))
        .collect()
}

/// The delta that turns `base` into `edited` (both whole tiles, the base from the player's own files).
pub fn diff(base: &[u8], edited: &[u8], o: &DiffOptions) -> Result<(Delta, DiffReport)> {
    let a = Adt::parse(base, o.alpha)?;
    let b = Adt::parse(edited, o.alpha.or(Some(a.alpha)))?;
    diff_tiles(&a, &b, o)
}

pub fn diff_tiles(a: &Adt, b: &Adt, o: &DiffOptions) -> Result<(Delta, DiffReport)> {
    let open_set: Option<std::collections::HashSet<u16>> = o.open.as_ref().map(|v| v.iter().copied().collect());
    let mut rep = DiffReport::default();
    let mut chunks = Vec::new();
    for i in 0..256usize {
        let (ca, cb) = (&a.chunks[i], &b.chunks[i]);
        let mut d = ChunkDelta { i: i as u16, xy: Some([cb.position[0], cb.position[1]]), ..Default::default() };
        let (ha, hb) = (ca.abs_heights(), cb.abs_heights());
        if ha.iter().zip(&hb).any(|(x, y)| (x - y).abs() > EPS) {
            d.heights = Some(b64::f32s(&hb));
        }
        let (la, lb) = (layers_of(a, ca), layers_of(b, cb));
        if la != lb {
            d.layers = Some(
                cb.layers
                    .iter()
                    .enumerate()
                    .map(|(k, l)| LayerDelta {
                        tex: b.textures.get(l.texture as usize).cloned().unwrap_or_default(),
                        flags: l.flags & !(adt::LAYER_USE_ALPHA | adt::LAYER_COMPRESSED),
                        effect: Some(l.effect),
                        alpha: if k == 0 { None } else { Some(b64::encode(l.alpha.as_deref().unwrap_or(&[0; 4096]))) },
                    })
                    .collect(),
            );
        }
        if ca.holes != cb.holes {
            d.holes = Some(cb.holes);
        }
        if ca.area != cb.area {
            d.area = Some(cb.area);
        }
        if (ca.flags ^ cb.flags) & adt::MCNK_IMPASSABLE != 0 {
            d.impassable = Some(cb.flags & adt::MCNK_IMPASSABLE != 0);
        }
        if ca.colors != cb.colors {
            d.colors = Some(cb.colors.as_deref().map(b64::encode));
        }
        let empty = ChunkWater::default();
        let wa = a.water.as_ref().map(|w| &w[i]).unwrap_or(&empty);
        let wb = b.water.as_ref().map(|w| &w[i]).unwrap_or(&empty);
        if wa != wb {
            d.water = Some(water_to(wb));
        }
        let changed = d.heights.is_some() || d.layers.is_some() || d.holes.is_some() || d.area.is_some() || d.impassable.is_some() || d.colors.is_some() || d.water.is_some();
        if !changed {
            continue;
        }
        if open_set.as_ref().is_some_and(|s| !s.contains(&(i as u16))) {
            rep.ignored.push(i as u16);
            continue;
        }
        rep.changed.push(i as u16);
        chunks.push(d);
    }
    let origin = o.origin.unwrap_or([0.0; 3]);
    let (ma, mb) = (models_of(a, origin), models_of(b, origin));
    let same = |x: &Model, y: &Model| norm(&x.path) == norm(&y.path) && x.uid == y.uid && x.pos == y.pos && x.rot == y.rot && x.scale == y.scale && x.flags == y.flags && x.set == y.set && x.name_set == y.name_set && x.bounds == y.bounds;
    let models = if ma.len() != mb.len() || ma.iter().zip(&mb).any(|(x, y)| !same(x, y)) {
        rep.models_changed = true;
        Some(mb)
    } else {
        None
    };
    let open = match &o.open {
        Some(v) => {
            let mut v = v.clone();
            v.sort_unstable();
            v.dedup();
            v
        }
        None => rep.changed.clone(),
    };
    let made = (o.made_by.is_some() || o.made_at.is_some()).then(|| Made { by: o.made_by.clone(), at: o.made_at.clone() });
    let d = Delta {
        format: FORMAT.into(),
        version: VERSION,
        base: o.base.clone(),
        tile: o.tile.clone(),
        open,
        pad: o.pad.clone(),
        origin: o.origin,
        chunks,
        models,
        made,
    };
    d.check()?;
    d.to_json()?; // the size limit
    Ok((d, rep))
}

/// The tile with the delta in it. `base`: the player's own tile (None for a delta whose base is "blank").
pub fn apply(delta: &Delta, base: Option<&[u8]>, alpha: Option<AlphaMode>) -> Result<Vec<u8>> {
    let mut a = match (delta.base.as_str(), base) {
        ("blank", _) => {
            let (z, tex) = delta.pad.as_ref().map(|p| (p.height, p.texture.clone())).unwrap_or((0.0, "Tileset\\Generic\\Black.blp".into()));
            let mut t = Adt::blank(delta.tile.x, delta.tile.y, z, &tex);
            if let Some(m) = alpha {
                t.alpha = m;
            }
            t
        }
        (_, Some(b)) => Adt::parse(b, alpha)?,
        (_, None) => return err("That ground needs the tile from your own game files."),
    };
    apply_to(delta, &mut a)?;
    Ok(a.write())
}

/// The delta, into a tile already read.
pub fn apply_to(delta: &Delta, a: &mut Adt) -> Result<()> {
    delta.check()?;
    let mut moved = [false; 256];
    for c in &delta.chunks {
        let i = c.i as usize;
        if let Some(h) = &c.heights {
            let v = b64::to_f32s(h, 145).ok_or_else(|| Error("bad heights".into()))?;
            a.chunks[i].set_abs_heights(&v);
            moved[i] = true;
        }
        if let Some(ls) = &c.layers {
            let mut layers = Vec::with_capacity(ls.len());
            let before: Vec<(String, u32)> = a.chunks[i].layers.iter().map(|l| (norm(a.textures.get(l.texture as usize).map(String::as_str).unwrap_or("")), l.effect)).collect();
            for (k, l) in ls.iter().enumerate() {
                let tex = clean_path(&l.tex).ok_or_else(|| Error("bad texture".into()))?;
                let effect = l.effect.unwrap_or_else(|| before.iter().find(|(t, _)| *t == norm(&tex)).map(|(_, e)| *e).unwrap_or(0));
                let texture = a.texture_index(&tex);
                let alpha = if k == 0 { None } else { Some(b64::decode(l.alpha.as_deref().unwrap_or("")).filter(|v| v.len() == 4096).ok_or_else(|| Error("bad alpha".into()))?) };
                layers.push(Layer { texture, flags: l.flags & !(adt::LAYER_USE_ALPHA | adt::LAYER_COMPRESSED), effect, alpha });
            }
            let ch = &mut a.chunks[i];
            ch.layers = layers;
            ch.recompute_pred_tex();
        }
        let ch = &mut a.chunks[i];
        if let Some(h) = c.holes {
            ch.holes = h;
        }
        if let Some(ar) = c.area {
            ch.area = ar;
        }
        if let Some(imp) = c.impassable {
            ch.flags = if imp { ch.flags | adt::MCNK_IMPASSABLE } else { ch.flags & !adt::MCNK_IMPASSABLE };
        }
        if let Some(cv) = &c.colors {
            ch.colors = match cv {
                None => None,
                Some(t) => Some(b64::decode(t).filter(|v| v.len() == 580).ok_or_else(|| Error("bad colours".into()))?),
            };
        }
        if let Some(w) = &c.water {
            let cw = match w {
                None => ChunkWater::default(),
                Some(w) => water_from(w)?,
            };
            if a.water.is_none() && !cw.layers.is_empty() {
                a.water = Some(vec![ChunkWater::default(); 256]);
            }
            if let Some(all) = a.water.as_mut() {
                all[i] = cw;
            }
        }
    }
    // normals again where heights moved, and around them (a vertex on a chunk's edge leans on its neighbour)
    let mut redo = [false; 256];
    for i in 0..256 {
        if moved[i] {
            let (r, c) = ((i / 16) as i32, (i % 16) as i32);
            for dr in -1..=1 {
                for dc in -1..=1 {
                    let (rr, cc) = (r + dr, c + dc);
                    if (0..16).contains(&rr) && (0..16).contains(&cc) {
                        redo[(rr * 16 + cc) as usize] = true;
                    }
                }
            }
        }
    }
    for (i, r) in redo.iter().enumerate() {
        if *r {
            a.recompute_normals(i);
        }
    }
    if let Some(ms) = &delta.models {
        set_models(a, ms, delta.origin.unwrap_or([0.0; 3]));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adt::tests::sample;
    use crate::adt::{vertex_uv, ChunkWater};

    fn den_tile() -> TileRef {
        TileRef { map: 1, dir: "Kalimdor".into(), x: 1, y: 1 }
    }

    /// The sample tile changed the way an editor would: a hill, paint, a hole, a pond, a tree moved, a rock added.
    fn edited(base: &Adt) -> Adt {
        let mut e = base.clone();
        for i in [151usize, 152] {
            for k in 0..145 {
                let (u, v) = vertex_uv(k);
                e.chunks[i].heights[k] += 10.0 - ((u - 4.0).powi(2) + (v - 4.0).powi(2)).sqrt();
            }
        }
        let sand = e.texture_index("Tileset\\Leyline\\Sand.blp");
        e.chunks[152].layers.push(Layer { texture: sand, flags: 0, effect: 0, alpha: Some((0..4096).map(|p| (p / 64 * 4) as u8).collect()) });
        e.chunks[152].recompute_pred_tex();
        e.chunks[153].holes = 0x0001;
        e.chunks[153].area = 12;
        e.chunks[153].flags |= adt::MCNK_IMPASSABLE;
        let w = e.water.as_mut().unwrap();
        w[136] = ChunkWater::default(); // a pond taken away
        w[154] = ChunkWater {
            layers: vec![WaterLayer { kind: 1, format: 0, min: 12.0, max: 12.0, x: 0, y: 0, w: 8, h: 8, mask: None, heights: Some(vec![12.0; 81]), uv: None, depth: Some(vec![40; 81]) }],
            attributes: Some((u64::MAX, 1)),
        };
        e.chunks[152].colors = Some(vec![127; 580]);
        e.doodads[0].pos[1] += 2.0;
        e.m2_names.push("World\\Leyline\\Bush.m2".into());
        e.doodads.push(Doodad { name: 2, uid: UID_BASE + 9, pos: [PLACE - 16250.0, 15.0, PLACE - 16230.0], rot: [0.0; 3], scale: 512, flags: 0 });
        e.recompute_refs();
        for i in 0..256 {
            e.recompute_normals(i);
        }
        e
    }

    #[test]
    fn diff_then_apply_gives_the_edited_tile() {
        let base = sample();
        let ed = edited(&base);
        let mut o = DiffOptions::new(den_tile());
        o.origin = Some([16226.2, 16257.0, 13.2]);
        let (d, rep) = diff(&base.write(), &ed.write(), &o).unwrap();
        assert_eq!(rep.changed, vec![136, 151, 152, 153, 154]);
        assert!(rep.models_changed && rep.ignored.is_empty());
        assert_eq!(d.open, rep.changed, "no open area given: the changed chunks");
        // numbers and names only: the JSON has no tile bytes in it, and reads back
        let text = d.to_json().unwrap();
        assert!(!text.contains("REVM") && text.len() < 200_000, "{}", text.len());
        let back = Delta::parse(&text).unwrap();
        assert_eq!(back, d);
        // rebuilt from the base: the same ground as the edit (nothing left to diff)
        let rebuilt = apply(&back, Some(&base.write()), None).unwrap();
        let (again, rep2) = diff(&ed.write(), &rebuilt, &o).unwrap();
        assert!(again.is_empty(), "{rep2:?}");
        // the same layout as the edit (heights may differ in the last bit: absolute in the delta, relative in the tile)
        assert_eq!(rebuilt.len(), ed.write().len());
        // the same delta on the same base, twice: the same bytes (a friend's tile hashes the same)
        assert_eq!(apply(&back, Some(&base.write()), None).unwrap(), rebuilt);
    }

    #[test]
    fn heights_alpha_holes_and_water_land_where_they_should() {
        let base = Adt::blank(1, 1, 13.2, "Tileset\\Leyline\\White.blp");
        let mut d = Delta::new(den_tile(), vec![0, 17]);
        let hill: Vec<f32> = (0..145).map(|k| 20.0 + vertex_uv(k).0).collect();
        let alpha: Vec<u8> = (0..4096).map(|p| (p % 251) as u8).collect();
        d.chunks.push(ChunkDelta {
            i: 17,
            heights: Some(b64::f32s(&hill)),
            layers: Some(vec![
                LayerDelta { tex: "Tileset/Leyline/Grass.blp".into(), flags: 0, effect: Some(3), alpha: None },
                LayerDelta { tex: "Tileset\\Leyline\\White.blp".into(), flags: 0x40, effect: None, alpha: Some(b64::encode(&alpha)) },
            ]),
            holes: Some(0x8001),
            water: Some(Some(WaterDelta {
                fishable: Some("00000000000000ff".into()),
                deep: None,
                layers: vec![WaterLayerDelta { kind: 1, format: 2, min: 21.0, max: 21.0, x: 2, y: 2, w: 2, h: 3, mask: Some(b64::encode(&[0b11_1011])), heights: None, uv: None, depth: Some(b64::encode(&[5; 12])) }],
            })),
            ..Default::default()
        });
        let t = Adt::parse(&apply(&d, Some(&base.write()), Some(AlphaMode::Big)).unwrap(), Some(AlphaMode::Big)).unwrap();
        let c = &t.chunks[17];
        assert!(c.abs_heights().iter().zip(&hill).all(|(a, b)| (a - b).abs() < 1e-4));
        assert_eq!(t.textures, vec!["Tileset\\Leyline\\White.blp".to_string(), "Tileset\\Leyline\\Grass.blp".to_string()]);
        assert_eq!((c.layers[0].texture, c.layers[0].effect, c.layers[1].texture), (1, 3, 0));
        assert_eq!(c.layers[1].alpha.as_deref(), Some(&alpha[..]));
        assert_eq!(c.layers[1].flags, 0x40 | adt::LAYER_USE_ALPHA);
        assert_eq!(c.holes, 0x8001);
        let w = &t.water.as_ref().unwrap()[17];
        assert_eq!(w.attributes, Some((0xff, 0)));
        assert_eq!((w.layers[0].x, w.layers[0].w, w.layers[0].h), (2, 2, 3));
        assert_eq!(w.layers[0].depth.as_deref(), Some(&[5u8; 12][..]));
        assert!(t.water.as_ref().unwrap()[0].layers.is_empty());
        // the untouched chunk 0 is the base's; the hill's neighbours got new normals, the far ones didn't
        assert_eq!(t.chunks[0].abs_heights(), base.chunks[0].abs_heights());
        assert_ne!(t.chunks[17].normals, base.chunks[17].normals);
        assert_eq!(t.chunks[200].normals, base.chunks[200].normals);
        // 4-bit tiles keep 16 levels of the same alpha
        let s = Adt::parse(&apply(&d, Some(&{ let mut b = base.clone(); b.alpha = AlphaMode::Small; b.write() }), Some(AlphaMode::Small)).unwrap(), Some(AlphaMode::Small)).unwrap();
        assert_eq!(s.chunks[17].layers[1].alpha.as_deref(), Some(&adt::quantize_4bit(&alpha)[..]));
    }

    #[test]
    fn models_by_name_and_place() {
        let base = sample();
        let origin = [16226.2, 16257.0, 13.2];
        let ms = models_of(&base, origin);
        assert_eq!(ms.len(), 3);
        assert_eq!(ms[0].path, "World\\Leyline\\Tree.m2");
        assert!(ms[0].pos.iter().all(|v| v.abs() < 1.0), "the tree stands at the Den's middle: {:?}", ms[0].pos);
        assert!(ms[2].is_wmo() && ms[2].bounds.is_some());
        let mut d = Delta::new(den_tile(), vec![]);
        d.origin = Some(origin);
        let mut new = ms.clone();
        new.remove(1);
        new.push(Model { path: "World/Leyline/Tree.m2".into(), uid: None, pos: [5.0, 0.0, 0.0], rot: None, scale: Some(1.5), flags: None, set: None, name_set: None, bounds: None });
        new.push(Model { path: "World\\Leyline\\Tower.wmo".into(), uid: None, pos: [-20.0, 0.0, 0.0], rot: None, scale: None, flags: None, set: Some(2), name_set: None, bounds: None });
        d.models = Some(new);
        let t = Adt::parse(&apply(&d, Some(&base.write()), None).unwrap(), None).unwrap();
        assert_eq!(t.m2_names, vec!["World\\Leyline\\Tree.m2".to_string()], "one name for both trees");
        assert_eq!(t.wmo_names.len(), 2);
        assert_eq!(t.doodads.len(), 2);
        assert_eq!(t.doodads[1].scale, 1536);
        assert_eq!(t.doodads[1].uid, UID_BASE, "new ids from the Den's range");
        assert_eq!(t.wmos[1].uid, UID_BASE + 2, "the next free one (the tree has +1)");
        assert_eq!(t.wmos[1].doodad_set, 2);
        let back = models_of(&t, origin);
        assert!((back[1].pos[0] - 5.0).abs() < 1e-3 && back[1].pos[1].abs() < 1e-3);
        assert_eq!(back[0].pos, ms[0].pos, "an unchanged placement keeps its exact numbers");
        // the new doodad is in its chunk's refs
        let ((_, _), (row, col)) = crate::locate((origin[0] + 5.0) as f32, origin[1] as f32).unwrap();
        let ch = &t.chunks[(row * 16 + col) as usize];
        assert!(ch.refs[..ch.n_doodad_refs as usize].contains(&1));
    }

    #[test]
    fn closed_chunks_bad_sizes_and_big_files_are_refused() {
        let base = sample().write();
        let mut d = Delta::new(den_tile(), vec![1]);
        d.chunks.push(ChunkDelta { i: 2, holes: Some(1), ..Default::default() });
        assert!(apply(&d, Some(&base), None).unwrap_err().0.contains("outside its open area"));
        d.chunks[0].i = 1;
        assert!(apply(&d, Some(&base), None).is_ok());
        d.chunks[0].heights = Some(b64::f32s(&[1.0; 144]));
        assert!(apply(&d, Some(&base), None).unwrap_err().0.contains("145"));
        d.chunks[0].heights = Some(b64::f32s(&[f32::NAN; 145]));
        assert!(apply(&d, Some(&base), None).is_err());
        d.chunks[0].heights = None;
        d.chunks[0].layers = Some(vec![LayerDelta { tex: "a.blp".into(), flags: 0, effect: None, alpha: Some(b64::encode(&[0; 4096])) }]);
        assert!(d.check().unwrap_err().0.contains("first layer"));
        d.chunks[0].layers = Some(vec![LayerDelta { tex: "a.blp".into(), flags: 0, effect: None, alpha: None }; 2]);
        assert!(d.check().unwrap_err().0.contains("alpha"));
        d.chunks[0].layers = Some(vec![LayerDelta { tex: "..\\a.blp".into(), flags: 0, effect: None, alpha: None }]);
        assert!(d.check().is_err());
        d.chunks[0].layers = None;
        d.chunks.push(ChunkDelta { i: 1, ..Default::default() });
        assert!(d.check().unwrap_err().0.contains("twice"));
        // the text: unknown keys, other formats, too big
        assert!(Delta::parse(r#"{"format":"leyline-terrain","version":1,"tile":{"map":1,"dir":"Kalimdor","x":1,"y":1},"open":[],"run":"rm -rf"}"#).is_err());
        assert!(Delta::parse(r#"{"format":"leyline-den","version":1,"tile":{"map":1,"dir":"Kalimdor","x":1,"y":1},"open":[]}"#).is_err());
        assert!(Delta::parse(r#"{"format":"leyline-terrain","version":2,"tile":{"map":1,"dir":"Kalimdor","x":1,"y":1},"open":[]}"#).unwrap_err().0.contains("newer"));
        assert!(Delta::parse(r#"{"format":"leyline-terrain","version":1,"tile":{"map":1,"dir":"../x","x":1,"y":1},"open":[]}"#).is_err());
        assert!(Delta::parse(r#"{"format":"leyline-terrain","version":1,"tile":{"map":1,"dir":"Kalimdor","x":1,"y":1},"open":[3,3]}"#).is_err());
        assert!(Delta::parse(&format!("{{\"format\":\"leyline-terrain\",\"pad\":\"{}\"}}", "x".repeat(5 * 1024 * 1024))).unwrap_err().0.contains("4 MB"));
        let ok = Delta::parse(r#"{"format":"leyline-terrain","version":1,"tile":{"map":1,"dir":"Kalimdor","x":1,"y":1},"open":[3],"chunks":[{"i":3,"water":null,"colors":null}]}"#).unwrap();
        assert_eq!(ok.chunks[0].water, Some(None), "null is 'no water'");
        assert_eq!(ok.base, "own");
        // a model that isn't a model
        let mut m = Delta::new(den_tile(), vec![]);
        m.models = Some(vec![Model { path: "World\\x.exe".into(), uid: None, pos: [0.0; 3], rot: None, scale: None, flags: None, set: None, name_set: None, bounds: None }]);
        assert!(m.check().is_err());
        // changes outside the open area are reported, not taken
        let s = sample();
        let mut e = s.clone();
        e.chunks[5].holes = 3;
        e.chunks[9].holes = 3;
        let mut o = DiffOptions::new(den_tile());
        o.open = Some(vec![9, 10]);
        let (d, rep) = diff_tiles(&s, &e, &o).unwrap();
        assert_eq!((rep.changed, rep.ignored), (vec![9], vec![5]));
        assert_eq!(d.open, vec![9, 10]);
    }

    #[test]
    fn a_blank_base_needs_no_game_files() {
        let mut d = Delta::new(den_tile(), vec![0]);
        d.base = "blank".into();
        d.pad = Some(Pad { height: 13.2, texture: "Tileset\\Leyline\\White.blp".into(), centre: None });
        let t = Adt::parse(&apply(&d, None, None).unwrap(), None).unwrap();
        assert!(t.chunks.iter().all(|c| c.abs_heights().iter().all(|h| (h - 13.2).abs() < 1e-4)));
        d.base = "own".into();
        assert!(apply(&d, None, None).unwrap_err().0.contains("own game files"));
    }
}
