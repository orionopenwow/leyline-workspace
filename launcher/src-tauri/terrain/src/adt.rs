//! A 3.3.5a map tile (ADT version 18), read and written.
//!
//! Format notes used (public descriptions, no editor code): the wowdev wiki "ADT/v18" page (chunk list, MHDR offsets
//! relative to MHDR's data, MCIN absolute offsets, the 128-byte MCNK header, MCVT's 9/8 interleaved rows, MCNR's 13
//! bytes after the normals, MCLY flags 0x100 "use alpha" and 0x200 "compressed", MCAL's three forms, MH2O's header /
//! instance / attribute layout, MDDF's 36 and MODF's 64-byte entries) and the mangos wiki "ADT Files" page (the same
//! field tables).
//!
//! What is kept exactly and what is made again:
//! * every field we don't change is kept (MCNK flags, area, sound emitters, MCLQ, shadows, MFBO, MTXF, unknown top-level
//!   chunks after the MCNKs);
//! * offsets and sizes (MHDR, MCIN, MCNK sub-offsets, MMID, MWID, MH2O) are always written fresh;
//! * alpha maps are held as 64 x 64 bytes and written in the tile's own form ([`AlphaMode`]): 4096 bytes
//!   uncompressed (big alpha), or 2048 bytes of 4-bit values. Compressed maps are read and written back
//!   uncompressed (flag 0x200 cleared): the client reads both.
//!
//! Our own writer's output reads back and writes again byte for byte (`write(parse(write(t))) == write(t)`).

use crate::{err, Error, Result, CHUNK, PLACE, UNIT};

/// MCLY flag: this layer has an alpha map in MCAL.
pub const LAYER_USE_ALPHA: u32 = 0x100;
/// MCLY flag: its alpha map is run-length compressed (big alpha only).
pub const LAYER_COMPRESSED: u32 = 0x200;
/// MCNK flags.
pub const MCNK_HAS_SHADOW: u32 = 0x1;
pub const MCNK_IMPASSABLE: u32 = 0x2;
pub const MCNK_HAS_MCCV: u32 = 0x40;
/// The 4-bit alpha form's last row and column are real values (else they copy the row / column before them).
pub const MCNK_DO_NOT_FIX_ALPHA: u32 = 0x8000;
/// MHDR flag: an MFBO chunk is there.
pub const MHDR_HAS_MFBO: u32 = 0x1;

/// How the tile stores its alpha maps (the map's WDT says so: MPHD flag 0x4 is the 4096-byte form).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    /// 64 x 64 bytes, 8 bits a value (or compressed).
    Big,
    /// 64 x 32 bytes, 4 bits a value, low nibble first.
    Small,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// Index into [`Adt::textures`].
    pub texture: u32,
    /// MCLY flags as read (the writer sets 0x100 and clears 0x200 itself).
    pub flags: u32,
    pub effect: u32,
    /// 64 x 64 values, row by row; None for the first layer.
    pub alpha: Option<Vec<u8>>,
}

/// One MCNK.
#[derive(Debug, Clone, PartialEq)]
pub struct Mcnk {
    pub flags: u32,
    pub index_x: u32,
    pub index_y: u32,
    pub area: u32,
    pub holes: u16,
    pub pad_3e: u16,
    /// The low-detail texture map: 8 x 8 two-bit layer numbers.
    pub pred_tex: [u8; 16],
    pub no_effect_doodad: [u8; 8],
    /// World x, y of the chunk's corner and the height MCVT's values are relative to.
    pub position: [f32; 3],
    pub unused_78: u32,
    pub unused_7c: u32,
    /// 145 heights relative to `position[2]`.
    pub heights: Vec<f32>,
    /// 145 x 3 signed bytes (x, y, up).
    pub normals: Vec<u8>,
    /// The 13 bytes after the normals.
    pub normals_tail: Vec<u8>,
    pub layers: Vec<Layer>,
    /// MCRF: doodad indices first (`n_doodad_refs` of them), then building indices.
    pub refs: Vec<u32>,
    pub n_doodad_refs: u32,
    /// MCSH (512 bytes), kept as it is.
    pub shadow: Option<Vec<u8>>,
    /// MCCV: 145 x 4 bytes.
    pub colors: Option<Vec<u8>>,
    /// MCLQ with its 8-byte header, as found (sizeLiquid bytes).
    pub mclq: Option<Vec<u8>>,
    /// MCSE with its 8-byte header, and the emitter count.
    pub mcse: Option<Vec<u8>>,
    pub n_sound_emitters: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Doodad {
    /// Index into [`Adt::m2_names`].
    pub name: u32,
    pub uid: u32,
    /// Placement space (see [`crate::PLACE`]).
    pub pos: [f32; 3],
    pub rot: [f32; 3],
    /// 1024 = normal size.
    pub scale: u16,
    pub flags: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WmoPlacement {
    /// Index into [`Adt::wmo_names`].
    pub name: u32,
    pub uid: u32,
    pub pos: [f32; 3],
    pub rot: [f32; 3],
    /// Placement space: lower then upper corner.
    pub lower: [f32; 3],
    pub upper: [f32; 3],
    pub flags: u16,
    pub doodad_set: u16,
    pub name_set: u16,
    pub pad: u16,
}

/// One MH2O instance.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterLayer {
    pub kind: u16,
    /// The instance's second 16-bit field (the vertex format).
    pub format: u16,
    pub min: f32,
    pub max: f32,
    pub x: u8,
    pub y: u8,
    pub w: u8,
    pub h: u8,
    /// w*h bits, row by row, low bit first; None: every cell.
    pub mask: Option<Vec<u8>>,
    /// (w+1)*(h+1) each, when present.
    pub heights: Option<Vec<f32>>,
    pub uv: Option<Vec<[u16; 2]>>,
    pub depth: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChunkWater {
    pub layers: Vec<WaterLayer>,
    /// (fishable, deep) 8 x 8 bit masks.
    pub attributes: Option<(u64, u64)>,
}

/// A whole tile.
#[derive(Debug, Clone, PartialEq)]
pub struct Adt {
    pub version: u32,
    pub mhdr_flags: u32,
    /// MHDR's last 16 bytes (unused), kept.
    pub mhdr_tail: [u8; 16],
    pub alpha: AlphaMode,
    pub textures: Vec<String>,
    /// The doodad names in MMID order (MMDX is written as these, one after the other).
    pub m2_names: Vec<String>,
    pub wmo_names: Vec<String>,
    pub doodads: Vec<Doodad>,
    pub wmos: Vec<WmoPlacement>,
    /// MH2O, one entry per chunk; None when the tile has no MH2O.
    pub water: Option<Vec<ChunkWater>>,
    pub chunks: Vec<Mcnk>,
    pub mfbo: Option<Vec<u8>>,
    pub mtxf: Option<Vec<u8>>,
    /// Other top-level chunks after the MCNKs (tag as read, data), kept in order.
    pub extra: Vec<([u8; 4], Vec<u8>)>,
}

// ------------------------------------------------------------------------------------------------ bytes in, bytes out
fn u16_at(b: &[u8], at: usize) -> Result<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| Error("The map tile ends too early.".into()))
}
fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| Error("The map tile ends too early.".into()))
}
fn f32_at(b: &[u8], at: usize) -> Result<f32> {
    u32_at(b, at).map(f32::from_bits)
}
fn f3_at(b: &[u8], at: usize) -> Result<[f32; 3]> {
    Ok([f32_at(b, at)?, f32_at(b, at + 4)?, f32_at(b, at + 8)?])
}
fn slice(b: &[u8], at: usize, n: usize) -> Result<&[u8]> {
    b.get(at..at.checked_add(n).ok_or_else(|| Error("bad size".into()))?).ok_or_else(|| Error("The map tile ends too early.".into()))
}
/// A tag as written in the file (reversed) to the readable form.
fn tag_at(b: &[u8], at: usize) -> Option<[u8; 4]> {
    b.get(at..at + 4).map(|s| [s[3], s[2], s[1], s[0]])
}

/// Top-level chunks: (readable tag, data start, data length).
pub(crate) fn walk(b: &[u8]) -> Result<Vec<([u8; 4], usize, usize)>> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while p + 8 <= b.len() {
        let tag = tag_at(b, p).unwrap();
        let n = u32_at(b, p + 4)? as usize;
        if p + 8 + n > b.len() {
            return err(format!("The map tile's {} chunk runs past its end.", String::from_utf8_lossy(&tag)));
        }
        out.push((tag, p + 8, n));
        p += 8 + n;
    }
    Ok(out)
}

pub(crate) fn put(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&[tag[3], tag[2], tag[1], tag[0]]);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}
fn p32(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn p16(o: &mut Vec<u8>, v: u16) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn pf(o: &mut Vec<u8>, v: f32) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn set32(o: &mut [u8], at: usize, v: u32) {
    o[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

pub(crate) fn zstrings(b: &[u8]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, &c) in b.iter().enumerate() {
        if c == 0 {
            if i > start {
                out.push((start, String::from_utf8_lossy(&b[start..i]).to_string()));
            }
            start = i + 1;
        }
    }
    if start < b.len() {
        out.push((start, String::from_utf8_lossy(&b[start..]).to_string()));
    }
    out
}

fn name_at(b: &[u8], off: usize) -> String {
    let rest = b.get(off..).unwrap_or(&[]);
    let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).to_string()
}

// ------------------------------------------------------------------------------------------------ alpha maps
/// Run-length compressed 64 x 64 (wowdev ADT/v18 MCAL): each control byte's top bit says fill (repeat the next byte)
/// or copy (the next bytes as they are), its low 7 bits how many.
pub fn alpha_decompress(src: &[u8]) -> Result<(Vec<u8>, usize)> {
    let mut out = Vec::with_capacity(4096);
    let mut p = 0;
    while out.len() < 4096 {
        let c = *src.get(p).ok_or_else(|| Error("A compressed alpha map ends too early.".into()))?;
        p += 1;
        let n = (c & 0x7F) as usize;
        if c & 0x80 != 0 {
            let v = *src.get(p).ok_or_else(|| Error("A compressed alpha map ends too early.".into()))?;
            p += 1;
            out.extend(std::iter::repeat(v).take(n));
        } else {
            out.extend_from_slice(slice(src, p, n)?);
            p += n;
        }
    }
    out.truncate(4096);
    Ok((out, p))
}

/// Run-length compression the client reads (runs stay inside a row of 64).
pub fn alpha_compress(a: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for row in a.chunks(64) {
        let mut i = 0;
        while i < row.len() {
            let mut run = 1;
            while i + run < row.len() && row[i + run] == row[i] && run < 127 {
                run += 1;
            }
            if run >= 2 {
                out.push(0x80 | run as u8);
                out.push(row[i]);
                i += run;
            } else {
                let start = i;
                while i < row.len() && i - start < 127 && !(i + 1 < row.len() && row[i + 1] == row[i]) {
                    i += 1;
                }
                if i == start {
                    i += 1;
                }
                out.push((i - start) as u8);
                out.extend_from_slice(&row[start..i]);
            }
        }
    }
    out
}

/// The 4-bit form to 64 x 64 bytes (each value times 17). Without `do_not_fix`, the last column and row repeat the
/// ones before them (the "63 x 63" maps the wowdev wiki describes).
pub fn alpha_from_4bit(src: &[u8], do_not_fix: bool) -> Vec<u8> {
    let mut a = vec![0u8; 4096];
    for (i, &b) in src.iter().take(2048).enumerate() {
        a[i * 2] = (b & 0x0F) * 17;
        a[i * 2 + 1] = (b >> 4) * 17;
    }
    if !do_not_fix {
        for r in 0..64 {
            a[r * 64 + 63] = a[r * 64 + 62];
        }
        for c in 0..64 {
            a[63 * 64 + c] = a[62 * 64 + c];
        }
    }
    a
}

pub fn alpha_to_4bit(a: &[u8]) -> Vec<u8> {
    let q = |v: u8| ((v as u16 + 8) / 17).min(15) as u8;
    (0..2048).map(|i| q(a[i * 2]) | q(a[i * 2 + 1]) << 4).collect()
}

/// The alpha a 4-bit tile actually shows for these values (what a write then read gives).
pub fn quantize_4bit(a: &[u8]) -> Vec<u8> {
    alpha_from_4bit(&alpha_to_4bit(a), true)
}

// ------------------------------------------------------------------------------------------------ vertices
/// (u, v) of height vertex `k` in outer steps from the chunk's corner: u towards lower world x (rows), v towards lower
/// world y (columns). Outer vertices sit on whole steps, inner ones half a step in.
pub fn vertex_uv(k: usize) -> (f32, f32) {
    let (block, rem) = (k / 17, k % 17);
    if rem < 9 {
        (block as f32, rem as f32)
    } else {
        (block as f32 + 0.5, (rem - 9) as f32 + 0.5)
    }
}
pub fn outer(r: usize, c: usize) -> usize {
    r * 17 + c
}
pub fn inner(r: usize, c: usize) -> usize {
    r * 17 + 9 + c
}

/// The height at (u, v) steps inside a chunk of 145 heights (the four triangles of each square, around its centre).
pub fn height_in(h: &[f32], u: f32, v: f32) -> f32 {
    let u = u.clamp(0.0, 8.0);
    let v = v.clamp(0.0, 8.0);
    let (i, j) = ((u.floor() as usize).min(7), (v.floor() as usize).min(7));
    let (fu, fv) = (u - i as f32, v - j as f32);
    let c = (0.5f32, 0.5f32, h[inner(i, j)]);
    let p = |di: usize, dj: usize| (di as f32, dj as f32, h[outer(i + di, j + dj)]);
    let (dx, dy) = (fu - 0.5, fv - 0.5);
    let (a, b) = if dx.abs() <= dy.abs() {
        if dy < 0.0 { (p(0, 0), p(1, 0)) } else { (p(0, 1), p(1, 1)) }
    } else if dx < 0.0 {
        (p(0, 0), p(0, 1))
    } else {
        (p(1, 0), p(1, 1))
    };
    // the plane through c, a, b at (fu, fv)
    let (ux, uy, uz) = (a.0 - c.0, a.1 - c.1, a.2 - c.2);
    let (vx, vy, vz) = (b.0 - c.0, b.1 - c.1, b.2 - c.2);
    let (nx, ny, nz) = (uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx);
    if nz.abs() < 1e-9 {
        return c.2;
    }
    c.2 - (nx * (fu - c.0) + ny * (fv - c.1)) / nz
}

impl Mcnk {
    /// A flat chunk at (row, col) of a tile whose chunk corner is (x, y), height z, one layer of texture 0.
    pub fn flat(row: u32, col: u32, corner: (f32, f32), z: f32) -> Mcnk {
        let mut normals = Vec::with_capacity(435);
        for _ in 0..145 {
            normals.extend_from_slice(&[0, 0, 127]);
        }
        Mcnk {
            flags: 0,
            index_x: col,
            index_y: row,
            area: 0,
            holes: 0,
            pad_3e: 0,
            pred_tex: [0; 16],
            no_effect_doodad: [0; 8],
            position: [corner.0, corner.1, z],
            unused_78: 0,
            unused_7c: 0,
            heights: vec![0.0; 145],
            normals,
            normals_tail: vec![0; 13],
            layers: vec![Layer { texture: 0, flags: 0, effect: 0, alpha: None }],
            refs: Vec::new(),
            n_doodad_refs: 0,
            shadow: None,
            colors: None,
            mclq: None,
            mcse: None,
            n_sound_emitters: 0,
        }
    }

    /// Absolute world heights.
    pub fn abs_heights(&self) -> Vec<f32> {
        self.heights.iter().map(|h| h + self.position[2]).collect()
    }

    pub fn set_abs_heights(&mut self, abs: &[f32]) {
        self.heights = abs.iter().map(|h| h - self.position[2]).collect();
    }

    /// The low-detail texture map from the alpha maps: per 8 x 8 cell, the topmost layer that covers most of it.
    pub fn recompute_pred_tex(&mut self) {
        let mut out = [0u8; 16];
        for cr in 0..8 {
            for cc in 0..8 {
                let mut best = 0usize;
                for (li, l) in self.layers.iter().enumerate().skip(1) {
                    let Some(a) = &l.alpha else { continue };
                    let mut sum = 0u32;
                    for r in 0..8 {
                        for c in 0..8 {
                            sum += a[(cr * 8 + r) * 64 + cc * 8 + c] as u32;
                        }
                    }
                    if sum >= 64 * 128 {
                        best = li;
                    }
                }
                let bit = (cr * 8 + cc) * 2;
                out[bit / 8] |= (best as u8 & 3) << (bit % 8);
            }
        }
        self.pred_tex = out;
    }
}

impl Adt {
    /// A whole flat tile `<dir>_<x>_<y>.adt` at height `z`, painted with one texture, no models, no water.
    pub fn blank(tile_x: u32, tile_y: u32, z: f32, texture: &str) -> Adt {
        let mut chunks = Vec::with_capacity(256);
        for row in 0..16 {
            for col in 0..16 {
                chunks.push(Mcnk::flat(row, col, crate::chunk_corner(tile_x, tile_y, row, col), z));
            }
        }
        Adt {
            version: 18,
            mhdr_flags: 0,
            mhdr_tail: [0; 16],
            alpha: AlphaMode::Big,
            textures: vec![texture.to_string()],
            m2_names: Vec::new(),
            wmo_names: Vec::new(),
            doodads: Vec::new(),
            wmos: Vec::new(),
            water: None,
            chunks,
            mfbo: None,
            mtxf: None,
            extra: Vec::new(),
        }
    }

    /// Reads a tile. `alpha`: the map's form from its WDT when known (else it is told from the layers' sizes).
    pub fn parse(b: &[u8], alpha: Option<AlphaMode>) -> Result<Adt> {
        let top = walk(b)?;
        let find = |t: &[u8; 4]| top.iter().find(|c| &c.0 == t).map(|c| (c.1, c.2));
        let (vs, _) = find(b"MVER").ok_or_else(|| Error("That isn't a map tile (no MVER).".into()))?;
        let version = u32_at(b, vs)?;
        if version != 18 {
            return err(format!("That map tile is version {version}; only version 18 tiles are read."));
        }
        let (hs, hn) = find(b"MHDR").ok_or_else(|| Error("That isn't a map tile (no MHDR).".into()))?;
        if hn < 64 {
            return err("The map tile's MHDR is too short.");
        }
        let mhdr_flags = u32_at(b, hs)?;
        let mut mhdr_tail = [0u8; 16];
        mhdr_tail.copy_from_slice(slice(b, hs + 48, 16)?);
        let data = |t: &[u8; 4]| find(t).map(|(s, n)| &b[s..s + n]).unwrap_or(&[]);
        let textures: Vec<String> = zstrings(data(b"MTEX")).into_iter().map(|(_, s)| s).collect();
        let (mmdx, mmid, mwmo, mwid) = (data(b"MMDX"), data(b"MMID"), data(b"MWMO"), data(b"MWID"));
        let m2_names: Vec<String> = mmid.chunks_exact(4).map(|c| name_at(mmdx, u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize)).collect();
        let wmo_names: Vec<String> = mwid.chunks_exact(4).map(|c| name_at(mwmo, u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize)).collect();
        let mddf = data(b"MDDF");
        let mut doodads = Vec::with_capacity(mddf.len() / 36);
        for e in mddf.chunks_exact(36) {
            doodads.push(Doodad {
                name: u32_at(e, 0)?,
                uid: u32_at(e, 4)?,
                pos: f3_at(e, 8)?,
                rot: f3_at(e, 20)?,
                scale: u16_at(e, 32)?,
                flags: u16_at(e, 34)?,
            });
        }
        let modf = data(b"MODF");
        let mut wmos = Vec::with_capacity(modf.len() / 64);
        for e in modf.chunks_exact(64) {
            wmos.push(WmoPlacement {
                name: u32_at(e, 0)?,
                uid: u32_at(e, 4)?,
                pos: f3_at(e, 8)?,
                rot: f3_at(e, 20)?,
                lower: f3_at(e, 32)?,
                upper: f3_at(e, 44)?,
                flags: u16_at(e, 56)?,
                doodad_set: u16_at(e, 58)?,
                name_set: u16_at(e, 60)?,
                pad: u16_at(e, 62)?,
            });
        }
        let water = match find(b"MH2O") {
            Some((s, n)) => Some(parse_mh2o(&b[s..s + n])?),
            None => None,
        };
        // MCNKs: in MCIN order when MCIN is there (offsets absolute), else in file order.
        let mut spots: Vec<(usize, usize)> = top.iter().filter(|c| &c.0 == b"MCNK").map(|c| (c.1 - 8, c.2)).collect();
        if let Some((s, n)) = find(b"MCIN") {
            if n >= 4096 {
                let mut v = Vec::with_capacity(256);
                for i in 0..256 {
                    let off = u32_at(b, s + i * 16)? as usize;
                    let len = u32_at(b, s + i * 16 + 4)? as usize;
                    if tag_at(b, off) != Some(*b"MCNK") {
                        return err("The map tile's chunk table points at something that isn't a chunk.");
                    }
                    let dn = u32_at(b, off + 4)? as usize;
                    let _ = len;
                    v.push((off, dn));
                }
                spots = v;
            }
        }
        if spots.len() != 256 {
            return err(format!("The map tile has {} chunks, not 256.", spots.len()));
        }
        // the alpha form: the WDT's, else from the layer sizes
        let mode = match alpha {
            Some(m) => m,
            None => guess_alpha(b, &spots).unwrap_or(AlphaMode::Big),
        };
        let mut chunks = Vec::with_capacity(256);
        for &(off, n) in &spots {
            chunks.push(parse_mcnk(slice(b, off, n + 8)?, mode)?);
        }
        let mut mfbo = None;
        let mut mtxf = None;
        let mut extra = Vec::new();
        let last_mcnk = top.iter().rposition(|c| &c.0 == b"MCNK").unwrap_or(0);
        for (i, (t, s, n)) in top.iter().enumerate() {
            match t {
                b"MFBO" => mfbo = Some(b[*s..s + n].to_vec()),
                b"MTXF" => mtxf = Some(b[*s..s + n].to_vec()),
                _ if i > last_mcnk && t != b"MCNK" => extra.push((*t, b[*s..s + n].to_vec())),
                _ => {}
            }
        }
        Ok(Adt { version, mhdr_flags, mhdr_tail, alpha: mode, textures, m2_names, wmo_names, doodads, wmos, water, chunks, mfbo, mtxf, extra })
    }

    /// The tile's bytes.
    pub fn write(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(512 * 1024);
        put(&mut o, b"MVER", &self.version.to_le_bytes());
        let mhdr_at = o.len();
        put(&mut o, b"MHDR", &[0u8; 64]);
        let rel = |at: usize| (at - (mhdr_at + 8)) as u32;
        let mcin_at = o.len();
        put(&mut o, b"MCIN", &[0u8; 4096]);
        let mut offs = [0u32; 11];
        offs[0] = rel(mcin_at);
        let names = |v: &[String]| -> (Vec<u8>, Vec<u8>) {
            let mut blob = Vec::new();
            let mut ids = Vec::new();
            for n in v {
                ids.extend_from_slice(&(blob.len() as u32).to_le_bytes());
                blob.extend_from_slice(n.as_bytes());
                blob.push(0);
            }
            (blob, ids)
        };
        let mut mtex = Vec::new();
        for t in &self.textures {
            mtex.extend_from_slice(t.as_bytes());
            mtex.push(0);
        }
        offs[1] = rel(o.len());
        put(&mut o, b"MTEX", &mtex);
        let (mmdx, mmid) = names(&self.m2_names);
        offs[2] = rel(o.len());
        put(&mut o, b"MMDX", &mmdx);
        offs[3] = rel(o.len());
        put(&mut o, b"MMID", &mmid);
        let (mwmo, mwid) = names(&self.wmo_names);
        offs[4] = rel(o.len());
        put(&mut o, b"MWMO", &mwmo);
        offs[5] = rel(o.len());
        put(&mut o, b"MWID", &mwid);
        let mut mddf = Vec::with_capacity(self.doodads.len() * 36);
        for d in &self.doodads {
            p32(&mut mddf, d.name);
            p32(&mut mddf, d.uid);
            d.pos.iter().chain(d.rot.iter()).for_each(|v| pf(&mut mddf, *v));
            p16(&mut mddf, d.scale);
            p16(&mut mddf, d.flags);
        }
        offs[6] = rel(o.len());
        put(&mut o, b"MDDF", &mddf);
        let mut modf = Vec::with_capacity(self.wmos.len() * 64);
        for w in &self.wmos {
            p32(&mut modf, w.name);
            p32(&mut modf, w.uid);
            w.pos.iter().chain(&w.rot).chain(&w.lower).chain(&w.upper).for_each(|v| pf(&mut modf, *v));
            p16(&mut modf, w.flags);
            p16(&mut modf, w.doodad_set);
            p16(&mut modf, w.name_set);
            p16(&mut modf, w.pad);
        }
        offs[7] = rel(o.len());
        put(&mut o, b"MODF", &modf);
        if let Some(w) = &self.water {
            offs[9] = rel(o.len());
            put(&mut o, b"MH2O", &write_mh2o(w));
        }
        let mut mcin = Vec::with_capacity(4096);
        for c in &self.chunks {
            let at = o.len();
            let bytes = write_mcnk(c, self.alpha);
            o.extend_from_slice(&bytes);
            p32(&mut mcin, at as u32);
            p32(&mut mcin, bytes.len() as u32);
            p32(&mut mcin, 0);
            p32(&mut mcin, 0);
        }
        o[mcin_at + 8..mcin_at + 8 + 4096].copy_from_slice(&mcin);
        if let Some(m) = &self.mfbo {
            offs[8] = rel(o.len());
            put(&mut o, b"MFBO", m);
        }
        if let Some(m) = &self.mtxf {
            offs[10] = rel(o.len());
            put(&mut o, b"MTXF", m);
        }
        for (t, d) in &self.extra {
            put(&mut o, t, d);
        }
        let flags = if self.mfbo.is_some() { self.mhdr_flags | MHDR_HAS_MFBO } else { self.mhdr_flags & !MHDR_HAS_MFBO };
        let h = mhdr_at + 8;
        set32(&mut o, h, flags);
        for (i, v) in offs.iter().enumerate() {
            set32(&mut o, h + 4 + i * 4, *v);
        }
        o[h + 48..h + 64].copy_from_slice(&self.mhdr_tail);
        o
    }

    /// The texture's index, added when the tile doesn't list it yet (names compared without regard to case or slash).
    pub fn texture_index(&mut self, path: &str) -> u32 {
        let key = path.replace('/', "\\").to_ascii_lowercase();
        if let Some(i) = self.textures.iter().position(|t| t.replace('/', "\\").to_ascii_lowercase() == key) {
            return i as u32;
        }
        self.textures.push(path.replace('/', "\\"));
        (self.textures.len() - 1) as u32
    }

    pub fn chunk(&self, i: usize) -> &Mcnk {
        &self.chunks[i]
    }

    /// The height of the global outer vertex grid (129 x 129 over the tile), R rows and C columns from the corner.
    fn grid(&self, r: usize, c: usize) -> f32 {
        let (cr, cc) = ((r / 8).min(15), (c / 8).min(15));
        let ch = &self.chunks[cr * 16 + cc];
        ch.heights[outer(r - cr * 8, c - cc * 8)] + ch.position[2]
    }

    /// MCNR made again for chunk `i` from the heights (central differences over the whole tile, one-sided at its edge):
    /// n = (dh/du, dh/dv, step) normalised, as (world x, world y, up) * 127. To verify in game: the two flat components'
    /// signs (u and v run towards lower world x and y).
    pub fn recompute_normals(&mut self, i: usize) {
        let (row, col) = (i / 16, i % 16);
        let mut n = Vec::with_capacity(435);
        let abs = self.chunks[i].abs_heights();
        for k in 0..145 {
            let (u, v) = vertex_uv(k);
            let (du, dv) = if u.fract() == 0.0 {
                let (gr, gc) = (row * 8 + u as usize, col * 8 + v as usize);
                let du = (self.grid((gr + 1).min(128), gc) - self.grid(gr.saturating_sub(1), gc)) / ((gr + 1).min(128) - gr.saturating_sub(1)) as f32;
                let dv = (self.grid(gr, (gc + 1).min(128)) - self.grid(gr, gc.saturating_sub(1))) / ((gc + 1).min(128) - gc.saturating_sub(1)) as f32;
                (du, dv)
            } else {
                let (r, c) = (u as usize, v as usize);
                let du = ((abs[outer(r + 1, c)] + abs[outer(r + 1, c + 1)]) - (abs[outer(r, c)] + abs[outer(r, c + 1)])) / 2.0;
                let dv = ((abs[outer(r, c + 1)] + abs[outer(r + 1, c + 1)]) - (abs[outer(r, c)] + abs[outer(r + 1, c)])) / 2.0;
                (du, dv)
            };
            let (x, y, z) = (du, dv, UNIT);
            let len = (x * x + y * y + z * z).sqrt();
            for c in [x / len, y / len, z / len] {
                n.push(((c * 127.0).round().clamp(-127.0, 127.0) as i8) as u8);
            }
        }
        self.chunks[i].normals = n;
    }

    /// MCRF made again for every chunk from the placements: a doodad belongs to the chunk it stands in, a building to
    /// every chunk its bounds touch.
    pub fn recompute_refs(&mut self) {
        let corner = |c: &Mcnk| (c.position[0], c.position[1]);
        for ch in self.chunks.iter_mut() {
            let (cx, cy) = corner(ch);
            let inside = |x: f32, y: f32| x <= cx && x > cx - CHUNK && y <= cy && y > cy - CHUNK;
            let mut refs: Vec<u32> = Vec::new();
            for (i, d) in self.doodads.iter().enumerate() {
                let (wx, wy) = (PLACE - d.pos[2], PLACE - d.pos[0]);
                if inside(wx, wy) {
                    refs.push(i as u32);
                }
            }
            let nd = refs.len() as u32;
            for (i, w) in self.wmos.iter().enumerate() {
                let (x1, x2) = (PLACE - w.upper[2].max(w.lower[2]), PLACE - w.lower[2].min(w.upper[2]));
                let (y1, y2) = (PLACE - w.upper[0].max(w.lower[0]), PLACE - w.lower[0].min(w.upper[0]));
                if x1 <= cx && x2 > cx - CHUNK && y1 <= cy && y2 > cy - CHUNK {
                    refs.push(i as u32);
                }
            }
            ch.refs = refs;
            ch.n_doodad_refs = nd;
        }
    }
}

fn guess_alpha(b: &[u8], spots: &[(usize, usize)]) -> Option<AlphaMode> {
    for &(off, _) in spots {
        let h = off + 8;
        let n_layers = u32_at(b, h + 0x0C).ok()? as usize;
        if n_layers < 2 {
            continue;
        }
        let ofs_mcly = u32_at(b, h + 0x1C).ok()? as usize;
        let size_alpha = u32_at(b, h + 0x28).ok()? as usize;
        let mut compressed = false;
        for l in 0..n_layers.min(4) {
            let f = u32_at(b, off + ofs_mcly + 8 + l * 16 + 4).ok()?;
            compressed |= f & LAYER_COMPRESSED != 0;
        }
        if compressed {
            return Some(AlphaMode::Big);
        }
        let per = size_alpha.saturating_sub(8) / (n_layers - 1);
        return Some(if per >= 4096 { AlphaMode::Big } else { AlphaMode::Small });
    }
    None
}

fn sub<'a>(b: &'a [u8], ofs: u32, tag: &[u8; 4]) -> Option<&'a [u8]> {
    let ofs = ofs as usize;
    if ofs == 0 || tag_at(b, ofs) != Some(*tag) {
        return None;
    }
    let n = u32::from_le_bytes(b.get(ofs + 4..ofs + 8)?.try_into().ok()?) as usize;
    b.get(ofs + 8..ofs + 8 + n)
}

/// One MCNK, from its tag to its end.
fn parse_mcnk(b: &[u8], mode: AlphaMode) -> Result<Mcnk> {
    if b.len() < 8 + 128 {
        return err("A chunk of the map tile is too short.");
    }
    let h = 8;
    let r = |at: usize| u32_at(b, h + at);
    let flags = r(0x00)?;
    let n_layers = r(0x0C)? as usize;
    let n_doodad_refs = r(0x10)?;
    let (ofs_mcvt, ofs_mcnr, ofs_mcly, ofs_mcrf, ofs_mcal) = (r(0x14)?, r(0x18)?, r(0x1C)?, r(0x20)?, r(0x24)?);
    let size_alpha = r(0x28)? as usize;
    let (ofs_mcsh, size_shadow) = (r(0x2C)?, r(0x30)? as usize);
    let area = r(0x34)?;
    let n_wmo_refs = r(0x38)?;
    let holes = u16_at(b, h + 0x3C)?;
    let pad_3e = u16_at(b, h + 0x3E)?;
    let mut pred_tex = [0u8; 16];
    pred_tex.copy_from_slice(slice(b, h + 0x40, 16)?);
    let mut no_effect_doodad = [0u8; 8];
    no_effect_doodad.copy_from_slice(slice(b, h + 0x50, 8)?);
    let (ofs_mcse, n_sound_emitters) = (r(0x58)?, r(0x5C)?);
    let (ofs_mclq, size_liquid) = (r(0x60)? as usize, r(0x64)? as usize);
    let position = f3_at(b, h + 0x68)?;
    let ofs_mccv = r(0x74)?;
    let (unused_78, unused_7c) = (r(0x78)?, r(0x7C)?);
    let _ = (size_shadow, ofs_mcsh);

    let mcvt = sub(b, ofs_mcvt, b"MCVT").ok_or_else(|| Error("A chunk of the map tile has no heights (MCVT).".into()))?;
    if mcvt.len() < 580 {
        return err("A chunk's heights (MCVT) are too short.");
    }
    let heights: Vec<f32> = mcvt[..580].chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let (normals, normals_tail) = match sub(b, ofs_mcnr, b"MCNR") {
        Some(n) if n.len() >= 435 => {
            let after = ofs_mcnr as usize + 8 + n.len();
            let tail = if n.len() >= 448 { n[435..448].to_vec() } else { b.get(after..after + 13).map(<[u8]>::to_vec).unwrap_or(vec![0; 13]) };
            (n[..435].to_vec(), tail)
        }
        _ => {
            let mut v = Vec::with_capacity(435);
            (0..145).for_each(|_| v.extend_from_slice(&[0, 0, 127]));
            (v, vec![0; 13])
        }
    };
    let mcly = sub(b, ofs_mcly, b"MCLY").unwrap_or(&[]);
    let mcal = sub(b, ofs_mcal, b"MCAL").unwrap_or(&[]);
    let _ = size_alpha;
    let mut layers = Vec::new();
    for l in 0..n_layers.min(4).min(mcly.len() / 16) {
        let e = &mcly[l * 16..l * 16 + 16];
        let (texture, lflags, offset, effect) = (u32_at(e, 0)?, u32_at(e, 4)?, u32_at(e, 8)? as usize, u32_at(e, 12)?);
        let alpha = if l > 0 && lflags & LAYER_USE_ALPHA != 0 {
            let src = mcal.get(offset..).unwrap_or(&[]);
            Some(if lflags & LAYER_COMPRESSED != 0 {
                alpha_decompress(src)?.0
            } else if mode == AlphaMode::Big {
                if src.len() < 4096 {
                    return err("A chunk's alpha map (MCAL) is too short.");
                }
                src[..4096].to_vec()
            } else {
                if src.len() < 2048 {
                    return err("A chunk's alpha map (MCAL) is too short.");
                }
                alpha_from_4bit(&src[..2048], flags & MCNK_DO_NOT_FIX_ALPHA != 0)
            })
        } else if l > 0 {
            Some(vec![0u8; 4096])
        } else {
            None
        };
        layers.push(Layer { texture, flags: lflags, effect, alpha });
    }
    let mcrf = sub(b, ofs_mcrf, b"MCRF").unwrap_or(&[]);
    let n_refs = (n_doodad_refs as usize).saturating_add(n_wmo_refs as usize).min(1 << 20);
    let refs: Vec<u32> = mcrf.chunks_exact(4).take(n_refs).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let shadow = if flags & MCNK_HAS_SHADOW != 0 { sub(b, ofs_mcsh, b"MCSH").map(<[u8]>::to_vec) } else { None };
    let colors = sub(b, ofs_mccv, b"MCCV").filter(|c| c.len() >= 580).map(|c| c[..580].to_vec());
    let mclq = if ofs_mclq != 0 && size_liquid >= 8 && tag_at(b, ofs_mclq) == Some(*b"MCLQ") {
        b.get(ofs_mclq..ofs_mclq + size_liquid).map(<[u8]>::to_vec)
    } else {
        None
    };
    let mcse = if ofs_mcse != 0 && tag_at(b, ofs_mcse as usize) == Some(*b"MCSE") {
        let o = ofs_mcse as usize;
        let n = u32_at(b, o + 4)? as usize;
        b.get(o..o + 8 + n).map(<[u8]>::to_vec)
    } else {
        None
    };
    let nd = n_doodad_refs.min(refs.len() as u32);
    Ok(Mcnk {
        flags,
        index_x: r(0x04)?,
        index_y: r(0x08)?,
        area,
        holes,
        pad_3e,
        pred_tex,
        no_effect_doodad,
        position,
        unused_78,
        unused_7c,
        heights,
        normals,
        normals_tail,
        layers,
        refs,
        n_doodad_refs: nd,
        shadow,
        colors,
        mclq,
        mcse,
        n_sound_emitters,
    })
}

fn write_mcnk(c: &Mcnk, mode: AlphaMode) -> Vec<u8> {
    let mut o = vec![0u8; 8 + 128];
    o[0..4].copy_from_slice(b"KNCM");
    let mut flags = c.flags;
    let at = |o: &Vec<u8>| o.len() as u32;
    let ofs_mcvt = at(&o);
    let mut hv = Vec::with_capacity(580);
    c.heights.iter().for_each(|v| pf(&mut hv, *v));
    put(&mut o, b"MCVT", &hv);
    let mut ofs_mccv = 0;
    if let Some(cv) = &c.colors {
        ofs_mccv = at(&o);
        put(&mut o, b"MCCV", cv);
        flags |= MCNK_HAS_MCCV;
    } else {
        flags &= !MCNK_HAS_MCCV;
    }
    let ofs_mcnr = at(&o);
    put(&mut o, b"MCNR", &c.normals);
    o.extend_from_slice(&c.normals_tail);
    // MCLY + MCAL
    let mut mcly = Vec::with_capacity(c.layers.len() * 16);
    let mut mcal = Vec::new();
    for (i, l) in c.layers.iter().enumerate() {
        let mut lf = l.flags & !(LAYER_USE_ALPHA | LAYER_COMPRESSED);
        let mut off = 0u32;
        if i > 0 {
            lf |= LAYER_USE_ALPHA;
            off = mcal.len() as u32;
            let a = l.alpha.clone().unwrap_or_else(|| vec![0; 4096]);
            match mode {
                AlphaMode::Big => mcal.extend_from_slice(&a[..4096]),
                AlphaMode::Small => mcal.extend_from_slice(&alpha_to_4bit(&a)),
            }
        }
        p32(&mut mcly, l.texture);
        p32(&mut mcly, lf);
        p32(&mut mcly, off);
        p32(&mut mcly, l.effect);
    }
    if mode == AlphaMode::Small {
        flags |= MCNK_DO_NOT_FIX_ALPHA; // we write all 64 x 64 values
    }
    let ofs_mcly = at(&o);
    put(&mut o, b"MCLY", &mcly);
    let ofs_mcrf = at(&o);
    let mut mcrf = Vec::with_capacity(c.refs.len() * 4);
    c.refs.iter().for_each(|r| p32(&mut mcrf, *r));
    put(&mut o, b"MCRF", &mcrf);
    let (mut ofs_mcsh, mut size_sh) = (0, 0);
    if let Some(s) = &c.shadow {
        ofs_mcsh = at(&o);
        put(&mut o, b"MCSH", s);
        size_sh = s.len() as u32 + 8;
        flags |= MCNK_HAS_SHADOW;
    } else {
        flags &= !MCNK_HAS_SHADOW;
    }
    let ofs_mcal = at(&o);
    put(&mut o, b"MCAL", &mcal);
    let size_alpha = mcal.len() as u32 + 8;
    let (mut ofs_mclq, mut size_lq) = (0, 0);
    if let Some(q) = &c.mclq {
        ofs_mclq = at(&o);
        o.extend_from_slice(q);
        size_lq = q.len() as u32;
    }
    let mut ofs_mcse = 0;
    if let Some(s) = &c.mcse {
        ofs_mcse = at(&o);
        o.extend_from_slice(s);
    }
    let size = (o.len() - 8) as u32;
    set32(&mut o, 4, size);
    let h = 8;
    let n_refs = c.refs.len() as u32;
    let nd = c.n_doodad_refs.min(n_refs);
    for (off, v) in [
        (0x00, flags),
        (0x04, c.index_x),
        (0x08, c.index_y),
        (0x0C, c.layers.len() as u32),
        (0x10, nd),
        (0x14, ofs_mcvt),
        (0x18, ofs_mcnr),
        (0x1C, ofs_mcly),
        (0x20, ofs_mcrf),
        (0x24, ofs_mcal),
        (0x28, size_alpha),
        (0x2C, ofs_mcsh),
        (0x30, size_sh),
        (0x34, c.area),
        (0x38, n_refs - nd),
        (0x58, ofs_mcse),
        (0x5C, if c.mcse.is_some() { c.n_sound_emitters } else { 0 }),
        (0x60, ofs_mclq),
        (0x64, size_lq),
        (0x74, ofs_mccv),
        (0x78, c.unused_78),
        (0x7C, c.unused_7c),
    ] {
        set32(&mut o, h + off, v);
    }
    o[h + 0x3C..h + 0x3E].copy_from_slice(&c.holes.to_le_bytes());
    o[h + 0x3E..h + 0x40].copy_from_slice(&c.pad_3e.to_le_bytes());
    o[h + 0x40..h + 0x50].copy_from_slice(&c.pred_tex);
    o[h + 0x50..h + 0x58].copy_from_slice(&c.no_effect_doodad);
    for (i, v) in c.position.iter().enumerate() {
        o[h + 0x68 + i * 4..h + 0x6C + i * 4].copy_from_slice(&v.to_le_bytes());
    }
    o
}

// ------------------------------------------------------------------------------------------------ MH2O
fn vertex_count(l: &WaterLayer) -> usize {
    (l.w as usize + 1) * (l.h as usize + 1)
}

/// MH2O (wowdev "ADT/v18", MH2O): 256 headers of (instances offset, layer count, attributes offset), each instance
/// 24 bytes (type, format, min, max, x, y, w, h, mask offset, vertex offset), attributes 16 bytes (fishable, deep).
/// Offsets are from the start of MH2O's data. Vertex data by format: 0 heights + depth, 1 heights + uv, 2 depth,
/// 3 heights + uv + depth.
fn parse_mh2o(b: &[u8]) -> Result<Vec<ChunkWater>> {
    let mut out = Vec::with_capacity(256);
    for i in 0..256 {
        let (oi, n, oa) = (u32_at(b, i * 12)? as usize, u32_at(b, i * 12 + 4)? as usize, u32_at(b, i * 12 + 8)? as usize);
        let mut cw = ChunkWater::default();
        if n > 0 && oi > 0 {
            for k in 0..n.min(16) {
                let e = oi + k * 24;
                let xywh = slice(b, e + 12, 4)?;
                let (x, y, w, h) = (xywh[0], xywh[1], xywh[2], xywh[3]);
                let mut l = WaterLayer {
                    kind: u16_at(b, e)?,
                    format: u16_at(b, e + 2)?,
                    min: f32_at(b, e + 4)?,
                    max: f32_at(b, e + 8)?,
                    x,
                    y,
                    w,
                    h,
                    mask: None,
                    heights: None,
                    uv: None,
                    depth: None,
                };
                if !(1..=8).contains(&w) || !(1..=8).contains(&h) || x as u32 + w as u32 > 8 || y as u32 + h as u32 > 8 {
                    return err("The map tile's water has a layer out of its chunk.");
                }
                let (om, ov) = (u32_at(b, e + 16)? as usize, u32_at(b, e + 20)? as usize);
                if om > 0 {
                    l.mask = Some(slice(b, om, (w as usize * h as usize).div_ceil(8))?.to_vec());
                }
                if ov > 0 {
                    let nv = vertex_count(&l);
                    let mut p = ov;
                    let (has_h, has_uv, has_d) = match l.format {
                        1 => (true, true, false),
                        2 => (false, false, true),
                        3 => (true, true, true),
                        _ => (true, false, true),
                    };
                    if has_h {
                        let s = slice(b, p, nv * 4)?;
                        l.heights = Some(s.chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
                        p += nv * 4;
                    }
                    if has_uv {
                        let s = slice(b, p, nv * 4)?;
                        l.uv = Some(s.chunks(4).map(|c| [u16::from_le_bytes([c[0], c[1]]), u16::from_le_bytes([c[2], c[3]])]).collect());
                        p += nv * 4;
                    }
                    if has_d {
                        l.depth = Some(slice(b, p, nv)?.to_vec());
                    }
                }
                cw.layers.push(l);
            }
            if oa > 0 {
                let s = slice(b, oa, 16)?;
                cw.attributes = Some((u64::from_le_bytes(s[..8].try_into().unwrap()), u64::from_le_bytes(s[8..].try_into().unwrap())));
            }
        }
        out.push(cw);
    }
    Ok(out)
}

fn write_mh2o(w: &[ChunkWater]) -> Vec<u8> {
    let mut o = vec![0u8; 256 * 12];
    for (i, cw) in w.iter().enumerate().take(256) {
        if cw.layers.is_empty() {
            continue;
        }
        let oi = o.len();
        o.extend(std::iter::repeat(0u8).take(cw.layers.len() * 24));
        let mut oa = 0;
        if let Some((f, d)) = cw.attributes {
            oa = o.len();
            o.extend_from_slice(&f.to_le_bytes());
            o.extend_from_slice(&d.to_le_bytes());
        }
        for (k, l) in cw.layers.iter().enumerate() {
            let mut om = 0;
            if let Some(m) = &l.mask {
                om = o.len();
                o.extend_from_slice(m);
            }
            let mut ov = 0;
            if l.heights.is_some() || l.uv.is_some() || l.depth.is_some() {
                ov = o.len();
                l.heights.iter().flatten().for_each(|v| pf(&mut o, *v));
                l.uv.iter().flatten().for_each(|v| {
                    p16(&mut o, v[0]);
                    p16(&mut o, v[1]);
                });
                if let Some(d) = &l.depth {
                    o.extend_from_slice(d);
                }
            }
            let e = oi + k * 24;
            o[e..e + 2].copy_from_slice(&l.kind.to_le_bytes());
            o[e + 2..e + 4].copy_from_slice(&l.format.to_le_bytes());
            o[e + 4..e + 8].copy_from_slice(&l.min.to_le_bytes());
            o[e + 8..e + 12].copy_from_slice(&l.max.to_le_bytes());
            o[e + 12..e + 16].copy_from_slice(&[l.x, l.y, l.w, l.h]);
            set32(&mut o, e + 16, om as u32);
            set32(&mut o, e + 20, ov as u32);
        }
        set32(&mut o, i * 12, oi as u32);
        set32(&mut o, i * 12 + 4, cw.layers.len() as u32);
        set32(&mut o, i * 12 + 8, oa as u32);
    }
    o
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic tile with something of everything: hills, four layers, holes, water, doodads and a building.
    pub fn sample() -> Adt {
        let mut a = Adt::blank(1, 1, 13.2, "Tileset\\Leyline\\White.blp");
        let grass = a.texture_index("Tileset\\Leyline\\Grass.blp");
        let rock = a.texture_index("Tileset\\Leyline\\Rock.blp");
        for (i, c) in a.chunks.iter_mut().enumerate() {
            for (k, h) in c.heights.iter_mut().enumerate() {
                let (u, v) = vertex_uv(k);
                *h = ((i % 16) as f32 + v / 8.0).sin() * 3.0 + ((i / 16) as f32 + u / 8.0).cos() * 2.0;
            }
            c.area = 876;
            if i % 7 == 0 {
                let alpha: Vec<u8> = (0..4096).map(|p| ((p % 64) * 4) as u8).collect();
                c.layers.push(Layer { texture: grass, flags: 0x40, effect: 12, alpha: Some(alpha) });
                c.layers.push(Layer { texture: rock, flags: 0, effect: 0, alpha: Some(vec![255; 4096]) });
                c.recompute_pred_tex();
            }
            if i % 11 == 0 {
                c.holes = 0b1010_0000_0000_0101;
            }
            if i == 40 {
                c.colors = Some((0..580).map(|p| p as u8).collect());
                c.shadow = Some(vec![0xAA; 512]);
            }
        }
        let mut water = vec![ChunkWater::default(); 256];
        water[136] = ChunkWater {
            layers: vec![WaterLayer {
                kind: 1,
                format: 0,
                min: 11.0,
                max: 12.5,
                x: 1,
                y: 2,
                w: 3,
                h: 2,
                mask: Some(vec![0b0011_1111]),
                heights: Some((0..12).map(|i| 11.0 + i as f32 * 0.1).collect()),
                uv: None,
                depth: Some((0..12).map(|i| i as u8 * 3).collect()),
            }],
            attributes: Some((0xFF, 0x0F00)),
        };
        water[137] = ChunkWater {
            layers: vec![WaterLayer { kind: 2, format: 2, min: 0.0, max: 0.0, x: 0, y: 0, w: 8, h: 8, mask: None, heights: None, uv: None, depth: Some(vec![9; 81]) }],
            attributes: None,
        };
        a.water = Some(water);
        a.m2_names = vec!["World\\Leyline\\Tree.m2".into(), "World\\Leyline\\Rock.m2".into()];
        a.wmo_names = vec!["World\\Leyline\\Hut.wmo".into()];
        a.doodads = vec![
            Doodad { name: 0, uid: 0x7F00_0001, pos: [PLACE - 16257.0, 14.0, PLACE - 16226.2], rot: [0.0, 90.0, 0.0], scale: 1024, flags: 0 },
            Doodad { name: 1, uid: 0x7F00_0002, pos: [PLACE - 16100.0, 20.0, PLACE - 16100.0], rot: [1.0, 2.0, 3.0], scale: 2048, flags: 1 },
        ];
        a.wmos = vec![WmoPlacement {
            name: 0,
            uid: 0x7F00_0003,
            pos: [PLACE - 16250.0, 13.0, PLACE - 16220.0],
            rot: [0.0, 45.0, 0.0],
            lower: [PLACE - 16290.0, 0.0, PLACE - 16240.0],
            upper: [PLACE - 16230.0, 30.0, PLACE - 16200.0],
            flags: 0,
            doodad_set: 1,
            name_set: 0,
            pad: 0,
        }];
        a.recompute_refs();
        a.mfbo = Some(vec![1; 36]);
        a.extra = vec![(*b"MXYZ", vec![1, 2, 3])];
        for i in 0..256 {
            a.recompute_normals(i);
        }
        a
    }

    #[test]
    fn our_tiles_round_trip_byte_for_byte() {
        for mode in [AlphaMode::Big, AlphaMode::Small] {
            let mut t = sample();
            t.alpha = mode;
            if mode == AlphaMode::Small {
                for c in t.chunks.iter_mut() {
                    for l in c.layers.iter_mut().skip(1) {
                        l.alpha = Some(quantize_4bit(l.alpha.as_ref().unwrap()));
                    }
                }
            }
            let bytes = t.write();
            let back = Adt::parse(&bytes, Some(mode)).unwrap();
            let mut want = t.clone();
            for c in want.chunks.iter_mut() {
                if mode == AlphaMode::Small {
                    c.flags |= MCNK_DO_NOT_FIX_ALPHA;
                }
                if c.colors.is_some() {
                    c.flags |= MCNK_HAS_MCCV;
                }
                if c.shadow.is_some() {
                    c.flags |= MCNK_HAS_SHADOW;
                }
                for l in c.layers.iter_mut().skip(1) {
                    l.flags |= LAYER_USE_ALPHA;
                }
            }
            want.mhdr_flags |= MHDR_HAS_MFBO;
            assert_eq!(back, want, "{mode:?}");
            assert_eq!(back.write(), bytes, "write(parse(write(t))) == write(t)");
            // the form is told from the sizes when the WDT isn't known
            assert_eq!(Adt::parse(&bytes, None).unwrap().alpha, mode);
        }
    }

    #[test]
    fn the_layout_is_the_formats() {
        let t = sample();
        let b = t.write();
        assert_eq!(&b[0..4], b"REVM", "tags are stored reversed");
        assert_eq!(u32_at(&b, 8).unwrap(), 18);
        assert_eq!(&b[12..16], b"RDHM");
        let mhdr = 20;
        let mcin = mhdr + u32_at(&b, mhdr + 4).unwrap() as usize;
        assert_eq!(&b[mcin..mcin + 4], b"NICM");
        let mtex = mhdr + u32_at(&b, mhdr + 8).unwrap() as usize;
        assert_eq!(&b[mtex..mtex + 4], b"XETM");
        assert_eq!(&b[mhdr + u32_at(&b, mhdr + 0x24).unwrap() as usize..][..4], b"OBFM");
        assert_eq!(&b[mhdr + u32_at(&b, mhdr + 0x28).unwrap() as usize..][..4], b"O2HM");
        assert_eq!(u32_at(&b, mhdr).unwrap() & 1, 1, "MFBO flag");
        // chunk 17 through MCIN: its index and position
        let at = u32_at(&b, mcin + 8 + 17 * 16).unwrap() as usize;
        assert_eq!(&b[at..at + 4], b"KNCM");
        assert_eq!(u32_at(&b, at + 8 + 4).unwrap(), 1, "IndexX");
        assert_eq!(u32_at(&b, at + 8 + 8).unwrap(), 1, "IndexY");
        let ofs = u32_at(&b, at + 8 + 0x14).unwrap() as usize;
        assert_eq!(&b[at + ofs..at + ofs + 4], b"TVCM", "MCVT offset is from the MCNK's tag");
        let ofs = u32_at(&b, at + 8 + 0x18).unwrap() as usize;
        assert_eq!(u32_at(&b, at + ofs + 4).unwrap(), 435, "MCNR's size leaves out its 13 extra bytes");
        assert_eq!(&b[at + ofs + 8 + 448..][..4], b"YLCM", "MCLY right after them");
    }

    #[test]
    fn alpha_forms() {
        let a: Vec<u8> = (0..4096).map(|i| ((i * 7) % 256) as u8).collect();
        let (d, used) = alpha_decompress(&alpha_compress(&a)).unwrap();
        assert_eq!(d, a);
        assert_eq!(used, alpha_compress(&a).len());
        let flat = vec![200u8; 4096];
        assert!(alpha_compress(&flat).len() <= 64 * 2, "runs");
        assert_eq!(alpha_decompress(&alpha_compress(&flat)).unwrap().0, flat);
        // the 4-bit form keeps 16 levels; without the flag the last row and column copy the ones before
        let q = quantize_4bit(&a);
        assert!(q.iter().zip(&a).all(|(x, y)| (*x as i32 - *y as i32).abs() <= 8));
        assert_eq!(quantize_4bit(&q), q);
        let mut ramp = vec![0u8; 4096];
        ramp.iter_mut().enumerate().for_each(|(i, v)| *v = if i % 64 == 63 || i / 64 == 63 { 0 } else { 255 });
        let fixed = alpha_from_4bit(&alpha_to_4bit(&ramp), false);
        assert_eq!(fixed[63], 255, "last column copies column 62");
        assert_eq!(fixed[63 * 64 + 5], 255, "last row copies row 62");
        assert!(alpha_decompress(&[0x85]).is_err());
    }

    #[test]
    fn a_hand_made_tile_with_compressed_alpha_reads() {
        // a tile written our way, then one layer's alpha swapped for the compressed form by hand
        let mut t = Adt::blank(0, 0, 0.0, "a.blp");
        t.chunks[3].layers.push(Layer { texture: 0, flags: 0, effect: 0, alpha: Some(vec![77; 4096]) });
        let mut b = t.write();
        let mcin = 20 + u32_at(&b, 24).unwrap() as usize + 8;
        let at = u32_at(&b, mcin + 3 * 16).unwrap() as usize;
        let ofs_mcly = u32_at(&b, at + 8 + 0x1C).unwrap() as usize;
        let ofs_mcal = u32_at(&b, at + 8 + 0x24).unwrap() as usize;
        let fl = at + ofs_mcly + 8 + 16 + 4;
        let f = u32_at(&b, fl).unwrap() | LAYER_COMPRESSED;
        b[fl..fl + 4].copy_from_slice(&f.to_le_bytes());
        let z = alpha_compress(&vec![33; 4096]);
        b[at + ofs_mcal + 8..at + ofs_mcal + 8 + z.len()].copy_from_slice(&z);
        let back = Adt::parse(&b, Some(AlphaMode::Big)).unwrap();
        assert_eq!(back.chunks[3].layers[1].alpha.as_deref(), Some(&[33u8; 4096][..]));
        // written back uncompressed, flag cleared
        let again = Adt::parse(&back.write(), Some(AlphaMode::Big)).unwrap();
        assert_eq!(again.chunks[3].layers[1].flags & LAYER_COMPRESSED, 0);
        assert_eq!(again.chunks[3].layers[1].alpha, back.chunks[3].layers[1].alpha);
    }

    #[test]
    fn heights_between_vertices() {
        let mut h = vec![0.0f32; 145];
        // a plane: h = 2u + 3v
        for (k, v) in h.iter_mut().enumerate() {
            let (a, b) = vertex_uv(k);
            *v = 2.0 * a + 3.0 * b;
        }
        for (u, v) in [(0.0, 0.0), (3.3, 4.7), (7.99, 0.1), (8.0, 8.0), (0.5, 0.5), (2.25, 6.75)] {
            assert!((height_in(&h, u, v) - (2.0 * u + 3.0 * v)).abs() < 1e-3, "({u},{v})");
        }
        // a spike in one centre: only its square rises
        let mut s = vec![0.0f32; 145];
        s[inner(2, 3)] = 8.0;
        assert!((height_in(&s, 2.5, 3.5) - 8.0).abs() < 1e-4);
        assert!(height_in(&s, 2.75, 3.5) > 3.0);
        assert_eq!(height_in(&s, 5.0, 5.0), 0.0);
    }

    #[test]
    fn normals_point_up_and_lean_away_from_slopes() {
        let mut t = Adt::blank(1, 1, 5.0, "a.blp");
        t.recompute_normals(0);
        assert!(t.chunks[0].normals.chunks(3).all(|n| n == [0, 0, 127]));
        // rising towards higher u (lower world x): the normal leans to +u in our (x, y, up) bytes
        for k in 0..145 {
            t.chunks[17].heights[k] = vertex_uv(k).0;
        }
        t.recompute_normals(17);
        let n = &t.chunks[17].normals[outer(4, 4) * 3..outer(4, 4) * 3 + 3];
        assert!((n[0] as i8) > 10 && (n[1] as i8) == 0 && (n[2] as i8) > 100, "{n:?}");
    }

    #[test]
    fn refs_follow_placements() {
        let t = sample();
        let den = 9 * 16 + 8; // the chunk the Den stands in
        assert!(t.chunks[den].refs[..t.chunks[den].n_doodad_refs as usize].contains(&0));
        let with_hut: Vec<usize> = (0..256).filter(|&i| t.chunks[i].refs[t.chunks[i].n_doodad_refs as usize..].contains(&0)).collect();
        assert!(with_hut.len() >= 4 && with_hut.contains(&den), "{with_hut:?}");
    }

    #[test]
    fn broken_tiles_are_refused_not_panicked_on() {
        let b = sample().write();
        for n in [0, 7, 12, 100, 5000, b.len() / 2, b.len() - 3] {
            let _ = Adt::parse(&b[..n], None); // never a panic
        }
        assert!(Adt::parse(b"nope", None).is_err());
        let mut v17 = b.clone();
        v17[8] = 17;
        assert!(Adt::parse(&v17, None).unwrap_err().0.contains("version 17"));
        let mut bad = b.clone();
        let mcin = 20 + u32_at(&b, 24).unwrap() as usize + 8;
        bad[mcin..mcin + 4].copy_from_slice(&3u32.to_le_bytes());
        assert!(Adt::parse(&bad, None).is_err());
        // random bytes in the middle: an error or a tile, never a panic
        let mut x = 7u32;
        for _ in 0..200 {
            let mut c = b.clone();
            for _ in 0..20 {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let p = (x as usize >> 3) % c.len();
                c[p] = (x >> 24) as u8;
            }
            let _ = Adt::parse(&c, None);
        }
    }
}
