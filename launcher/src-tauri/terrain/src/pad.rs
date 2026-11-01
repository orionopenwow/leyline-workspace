//! The Den's white pad (docs/NOGGIT-DEN-PLAN.md 2.1): 2 x 2 chunks (66.7 yards a side) around the room, flattened to
//! the floor height and painted one light texture; "Make the Den bigger" opens the next ring of chunks (4, 16, 36 ...
//! up to the whole tile). And [`Ground`]: the height of the ground at a point from a delta alone, for Den Lite (which
//! has no map files) to stand props and the player on the edited ground.

use crate::adt::{height_in, Adt, ChunkWater, Layer};
use crate::delta::{self, Delta, DiffOptions, Pad, TileRef};
use crate::{b64, chunk_corner, clean_path, err, locate, Result, CHUNK, UNIT};
use std::collections::HashMap;

/// The Den: map 1, the sealed island (docs/DEN.md "The room"; game_tele 424): x, y and the room's floor.
pub const DEN: (f32, f32, f32) = (16226.2, 16257.0, 13.2);
/// The light texture of a new pad (one of the client's own; to choose on a real PC: "whether the pad's texture looks
/// white enough", plan section 9).
pub const PAD_TEXTURE: &str = "Tileset\\Generic\\White.blp";

/// The Den's tile: Kalimdor_1_1.
pub fn den_tile() -> TileRef {
    TileRef { map: 1, dir: "Kalimdor".into(), x: 1, y: 1 }
}

/// The 2 x 2 block's top-left (row, col): the four chunks around the chunk corner nearest the point.
pub fn pad_block(tile: &TileRef, centre: (f32, f32)) -> (u32, u32) {
    let (cx, cy) = chunk_corner(tile.x, tile.y, 0, 0);
    let rf = (cx - centre.0) / CHUNK;
    let cf = (cy - centre.1) / CHUNK;
    let r = (rf.round() as i32 - 1).clamp(0, 14) as u32;
    let c = (cf.round() as i32 - 1).clamp(0, 14) as u32;
    (r, c)
}

/// The chunks open after `ring` rings around the block (ring 0: the 4 of the pad), sorted.
pub fn ring_chunks(block: (u32, u32), ring: u32) -> Vec<u16> {
    let k = ring as i32;
    let (r0, c0) = (block.0 as i32 - k, block.1 as i32 - k);
    let (r1, c1) = (block.0 as i32 + 1 + k, block.1 as i32 + 1 + k);
    let mut v = Vec::new();
    for r in r0.max(0)..=r1.min(15) {
        for c in c0.max(0)..=c1.min(15) {
            v.push((r * 16 + c) as u16);
        }
    }
    v
}

/// How many rings an open area holds around the block (the smallest ring that covers it).
pub fn rings_in(block: (u32, u32), open: &[u16]) -> u32 {
    (0..16).find(|&k| {
        let s = ring_chunks(block, k);
        open.iter().all(|i| s.contains(i))
    }).unwrap_or(15)
}

fn flatten(a: &mut Adt, chunks: &[u16], height: f32, texture: &str) {
    let tex = a.texture_index(texture);
    for &i in chunks {
        let c = &mut a.chunks[i as usize];
        c.set_abs_heights(&[height; 145]);
        c.layers = vec![Layer { texture: tex, flags: 0, effect: 0, alpha: None }];
        c.holes = 0;
        c.recompute_pred_tex();
        if let Some(w) = a.water.as_mut() {
            w[i as usize] = ChunkWater::default();
        }
    }
    for i in 0..256 {
        a.recompute_normals(i);
    }
}

/// The pad as a delta on the player's own tile: `ring` rings (0 = the 4 chunks) flattened to `pad.height` and
/// painted `pad.texture`; the rest of the tile as it is.
pub fn make_pad(base: &[u8], tile: TileRef, pad: Pad, ring: u32) -> Result<Delta> {
    if clean_path(&pad.texture).is_none() || !pad.height.is_finite() {
        return err("The pad's texture or height isn't right.");
    }
    let a = Adt::parse(base, None)?;
    let centre = pad.centre.map(|c| (c[0], c[1])).unwrap_or((DEN.0, DEN.1));
    let open = ring_chunks(pad_block(&tile, centre), ring);
    let mut e = a.clone();
    flatten(&mut e, &open, pad.height, &pad.texture);
    let mut o = DiffOptions::new(tile);
    o.open = Some(open);
    o.pad = Some(Pad { centre: Some([centre.0, centre.1]), ..pad });
    o.origin = Some(crate::flat::ORIGIN);
    o.made_by = Some("leyline pad".into());
    Ok(delta::diff_tiles(&a, &e, &o)?.0)
}

/// "Make the Den bigger": the next ring opened, its new chunks flattened to the pad (chunks already edited stay).
pub fn grow(base: &[u8], d: &Delta) -> Result<Delta> {
    let pad = d.pad.clone().ok_or_else(|| crate::Error("This ground has no pad to grow.".into()))?;
    let centre = pad.centre.map(|c| (c[0], c[1])).unwrap_or((DEN.0, DEN.1));
    let block = pad_block(&d.tile, centre);
    let now = rings_in(block, &d.open);
    let open = ring_chunks(block, now + 1);
    if open.len() == d.open.len() {
        return err("Your Den already covers the whole tile.");
    }
    let a = Adt::parse(base, None)?;
    let mut e = a.clone();
    delta::apply_to(d, &mut e)?;
    let new: Vec<u16> = open.iter().copied().filter(|i| !d.open.contains(i)).collect();
    flatten(&mut e, &new, pad.height, &pad.texture);
    let mut o = DiffOptions::new(d.tile.clone());
    o.open = Some(open);
    o.pad = Some(pad);
    o.origin = d.origin;
    o.base = d.base.clone();
    o.made_by = Some("leyline grow".into());
    Ok(delta::diff_tiles(&a, &e, &o)?.0)
}

/// The ground's height from a delta's chunks (the ones that carry heights). Points elsewhere: None (the caller keeps
/// its own floor).
#[derive(Debug, Clone, Default)]
pub struct Ground {
    tile: Option<(u32, u32)>,
    chunks: HashMap<u16, ((f32, f32), Vec<f32>)>,
}

impl Ground {
    pub fn from_delta(d: &Delta) -> Result<Ground> {
        d.check()?;
        let mut chunks = HashMap::new();
        for c in &d.chunks {
            let Some(h) = &c.heights else { continue };
            let v = b64::to_f32s(h, 145).ok_or_else(|| crate::Error("bad heights".into()))?;
            let (row, col) = ((c.i / 16) as u32, (c.i % 16) as u32);
            let corner = c.xy.map(|xy| (xy[0], xy[1])).unwrap_or_else(|| chunk_corner(d.tile.x, d.tile.y, row, col));
            chunks.insert(c.i, (corner, v));
        }
        Ok(Ground { tile: Some((d.tile.x, d.tile.y)), chunks })
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn height_at(&self, x: f32, y: f32) -> Option<f32> {
        let ((tx, ty), (row, col)) = locate(x, y)?;
        if self.tile != Some((tx, ty)) {
            return None;
        }
        let ((cx, cy), h) = self.chunks.get(&((row * 16 + col) as u16))?;
        Some(height_in(h, (cx - x) / UNIT, (cy - y) / UNIT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adt::tests::sample;

    fn pad() -> Pad {
        Pad { height: DEN.2, texture: PAD_TEXTURE.into(), centre: None }
    }

    #[test]
    fn the_pad_is_four_chunks_around_the_room_and_flat_to_a_centimetre() {
        let tile = den_tile();
        let block = pad_block(&tile, (DEN.0, DEN.1));
        assert_eq!(block, (8, 7), "the room is near the corner of chunks rows 8-9, columns 7-8");
        assert_eq!(ring_chunks(block, 0), vec![135, 136, 151, 152]);
        let base = sample().write();
        let d = make_pad(&base, tile, pad(), 0).unwrap();
        assert_eq!(d.open, vec![135, 136, 151, 152]);
        assert_eq!(d.chunks.len(), 4);
        let t = Adt::parse(&delta::apply(&d, Some(&base), None).unwrap(), None).unwrap();
        for &i in &d.open {
            let c = &t.chunks[i as usize];
            assert!(c.abs_heights().iter().all(|h| (h - DEN.2).abs() < 0.01), "chunk {i} flat");
            assert_eq!(c.layers.len(), 1);
            assert_eq!(t.textures[c.layers[0].texture as usize], PAD_TEXTURE);
            assert_eq!(c.holes, 0);
            assert!(t.water.as_ref().unwrap()[i as usize].layers.is_empty(), "no water on the pad");
        }
        let s = sample();
        assert_eq!(t.chunks[0].abs_heights(), s.chunks[0].abs_heights(), "the rest as it was");
        // the room's middle stands on the pad
        let g = Ground::from_delta(&d).unwrap();
        assert!((g.height_at(DEN.0, DEN.1).unwrap() - DEN.2).abs() < 0.01);
        // the pad is about 66.7 yards a side: 30 yards from the middle still on it, 40 not
        assert!(g.height_at(DEN.0 + 20.0, DEN.1 - 20.0).is_some());
        assert!(g.height_at(DEN.0 + 60.0, DEN.1).is_none());
    }

    #[test]
    fn it_grows_by_rings_keeping_what_was_made() {
        let base = sample().write();
        let mut d = make_pad(&base, den_tile(), pad(), 0).unwrap();
        // a hill on the pad first
        let mut a = Adt::parse(&delta::apply(&d, Some(&base), None).unwrap(), None).unwrap();
        a.chunks[136].heights.iter_mut().for_each(|h| *h += 9.0);
        let mut o = DiffOptions::new(den_tile());
        o.open = Some(d.open.clone());
        o.pad = d.pad.clone();
        d = delta::diff(&base, &a.write(), &o).unwrap().0;
        let mut sizes = vec![d.open.len()];
        for _ in 0..8 {
            match grow(&base, &d) {
                Ok(n) => {
                    d = n;
                    sizes.push(d.open.len());
                }
                Err(e) => {
                    assert!(e.0.contains("whole tile"));
                    break;
                }
            }
        }
        assert_eq!(&sizes[..4], &[4, 16, 36, 64]);
        assert_eq!(*sizes.last().unwrap(), 256);
        let t = Adt::parse(&delta::apply(&d, Some(&base), None).unwrap(), None).unwrap();
        assert!((t.chunks[136].abs_heights()[0] - (DEN.2 + 9.0)).abs() < 0.01, "the hill stayed");
        assert!((t.chunks[0].abs_heights()[60] - DEN.2).abs() < 0.01, "the far corner flattened by the last ring");
    }

    #[test]
    fn ground_heights_between_vertices() {
        let mut d = Delta::new(den_tile(), vec![152]);
        // a slope rising 1 yard per step towards lower world x
        let h: Vec<f32> = (0..145).map(|k| 13.2 + crate::adt::vertex_uv(k).0).collect();
        d.chunks.push(delta::ChunkDelta { i: 152, heights: Some(b64::f32s(&h)), ..Default::default() });
        let g = Ground::from_delta(&d).unwrap();
        let (cx, cy) = chunk_corner(1, 1, 9, 8);
        let z = g.height_at(cx - 3.0 * UNIT, cy - 1.0).unwrap();
        assert!((z - 16.2).abs() < 1e-3, "{z}");
        assert!(g.height_at(cx + 1.0, cy - 1.0).is_none(), "another chunk");
        assert!(g.height_at(100.0, 100.0).is_none(), "another tile");
        assert!(Ground::default().height_at(DEN.0, DEN.1).is_none());
    }
}
