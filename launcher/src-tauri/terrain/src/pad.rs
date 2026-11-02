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
/// The pad's texture: our own white texture (made by [`crate::blp::solid`], never a game file), put in the player's
/// patch-T beside the tile by the launcher (owner 1 Nov: "a completely white plane"). The game's own
/// `Tileset\Generic\White.blp` isn't in every game's archives (it wasn't in the owner's).
pub const PAD_TEXTURE: &str = "Tileset\\Leyline\\DenWhite.blp";
/// Our texture's size (a side, in pixels) and colour (BGRA).
pub const PAD_TEXTURE_SIZE: u32 = 256;
pub const PAD_TEXTURE_BGRA: [u8; 4] = [255, 255, 255, 255];
/// Textures earlier pads were painted with: a chunk painted only with one of these (or the pad's own) is the pad's.
const OLD_PAD_TEXTURES: &[&str] = &["Tileset\\Generic\\White.blp"];

/// Our texture's bytes (what goes in patch-T at [`PAD_TEXTURE`]).
pub fn pad_texture() -> Vec<u8> {
    crate::blp::solid(PAD_TEXTURE_SIZE, PAD_TEXTURE_BGRA).expect("a power of two")
}

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

/// The area (AreaTable id) of the chunk the point stands in, in the player's own tile (0: none).
pub fn area_at(a: &Adt, tile: &TileRef, at: (f32, f32)) -> u32 {
    match locate(at.0, at.1) {
        Some(((tx, ty), (r, c))) if (tx, ty) == (tile.x, tile.y) => a.chunks[(r * 16 + c) as usize].area,
        _ => 0,
    }
}

/// The chunks flattened to `height` and painted `texture` only; their water, holes, vertex colours and shadows gone,
/// the island's models and buildings on them taken away, and the open area one zone (`area`, the Den's: the game then
/// shows the Den's name everywhere on it).
fn flatten(a: &mut Adt, chunks: &[u16], height: f32, texture: &str, area: u32) {
    let tex = a.texture_index(texture);
    for &i in chunks {
        let c = &mut a.chunks[i as usize];
        c.set_abs_heights(&[height; 145]);
        c.layers = vec![Layer { texture: tex, flags: 0, effect: 0, alpha: None }];
        c.holes = 0;
        c.colors = None;
        c.shadow = None;
        if area != 0 {
            c.area = area;
        }
        c.recompute_pred_tex();
        if let Some(w) = a.water.as_mut() {
            w[i as usize] = ChunkWater::default();
        }
    }
    a.clear_placements(chunks);
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
    flatten(&mut e, &open, pad.height, &pad.texture, area_at(&a, &tile, centre));
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
    flatten(&mut e, &new, pad.height, &pad.texture, area_at(&a, &d.tile, centre));
    let mut o = DiffOptions::new(d.tile.clone());
    o.open = Some(open);
    o.pad = Some(pad);
    o.origin = d.origin;
    o.base = d.base.clone();
    o.made_by = Some("leyline grow".into());
    Ok(delta::diff_tiles(&a, &e, &o)?.0)
}

/// What [`whiten`] changed on a ground made before.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Whitened {
    /// Chunks painted our white (they were the old pad's texture), or their vertex colours taken off.
    pub repainted: usize,
    /// The island's own models and buildings taken off the open area.
    pub models: usize,
    /// Chunks put in the Den's zone.
    pub area: usize,
}

/// A ground made before (the pad painted the island's own texture, the island's trees and buildings still standing
/// on it): the pad's chunks painted our white texture and their vertex colours gone, the island's own models (the
/// ones in the player's tile, by id) off the open area, the open area in the Den's zone. What the player made stays:
/// chunks painted otherwise, heights, holes, models added in Forge. None: nothing to do (it runs at every Den start).
pub fn whiten(base: &[u8], d: &Delta) -> Result<Option<(Delta, Whitened)>> {
    let Some(pad) = d.pad.clone() else { return Ok(None) };
    if d.base == "blank" {
        return Ok(None);
    }
    let a = Adt::parse(base, None)?;
    let mut e = a.clone();
    delta::apply_to(d, &mut e)?;
    let mut w = Whitened::default();
    let old: Vec<String> = OLD_PAD_TEXTURES.iter().map(|t| delta::norm(t)).chain([delta::norm(&pad.texture)]).collect();
    let tex = e.texture_index(PAD_TEXTURE);
    let centre = pad.centre.map(|c| (c[0], c[1])).unwrap_or((DEN.0, DEN.1));
    let area = area_at(&a, &d.tile, centre);
    for &i in &d.open {
        let name = e.chunks[i as usize].layers.first().and_then(|l| e.textures.get(l.texture as usize)).map(|t| delta::norm(t)).unwrap_or_default();
        let c = &mut e.chunks[i as usize];
        if c.layers.len() == 1 && c.layers[0].texture != tex && old.contains(&name) {
            c.layers = vec![Layer { texture: tex, flags: 0, effect: 0, alpha: None }];
            c.recompute_pred_tex();
            w.repainted += 1;
        } else if c.layers.len() == 1 && c.layers[0].texture == tex && c.colors.is_some() {
            w.repainted += 1;
        }
        if c.layers.len() == 1 && c.layers[0].texture == tex {
            c.colors = None;
        }
        if area != 0 && c.area != area {
            c.area = area;
            w.area += 1;
        }
    }
    // the island's own placements standing on the open area (the chunks' references say which)
    let own_uids: std::collections::HashSet<u32> = a.doodads.iter().map(|x| x.uid).chain(a.wmos.iter().map(|x| x.uid)).collect();
    let (mut on_d, mut on_w) = (vec![false; e.doodads.len()], vec![false; e.wmos.len()]);
    for &i in &d.open {
        let c = &e.chunks[i as usize];
        let k = (c.n_doodad_refs as usize).min(c.refs.len());
        c.refs[..k].iter().for_each(|&r| on_d.get_mut(r as usize).into_iter().for_each(|x| *x = true));
        c.refs[k..].iter().for_each(|&r| on_w.get_mut(r as usize).into_iter().for_each(|x| *x = true));
    }
    let nd = e.doodads.len();
    let all = delta::models_of(&e, [0.0; 3]);
    let kept: Vec<delta::Model> = all
        .iter()
        .enumerate()
        .filter(|(k, m)| {
            let on = if *k < nd { on_d[*k] } else { on_w[*k - nd] };
            !(on && m.uid.is_some_and(|u| own_uids.contains(&u)))
        })
        .map(|(_, m)| m.clone())
        .collect();
    if kept.len() != all.len() {
        w.models = all.len() - kept.len();
        delta::set_models(&mut e, &kept, [0.0; 3]);
    }
    if w == Whitened::default() && delta::norm(&pad.texture) == delta::norm(PAD_TEXTURE) {
        return Ok(None);
    }
    let mut o = DiffOptions::new(d.tile.clone());
    o.open = Some(d.open.clone());
    o.pad = Some(Pad { texture: PAD_TEXTURE.into(), ..pad });
    o.origin = d.origin;
    o.base = d.base.clone();
    o.made_by = Some("leyline white".into());
    Ok(Some((delta::diff_tiles(&a, &e, &o)?.0, w)))
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
        // nothing of the island stands on the pad: the tree in the Den's chunk went; the hut is still referred to by
        // the chunks beside the pad, the rock is elsewhere
        let paths: Vec<&str> = d.models.as_ref().unwrap().iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, ["World\\Leyline\\Rock.m2", "World\\Leyline\\Hut.wmo"]);
        assert!(d.open.iter().all(|&i| t.chunks[i as usize].refs[..t.chunks[i as usize].n_doodad_refs as usize].is_empty()));
        assert!(t.chunks.iter().all(|c| c.area == 876), "the Den's zone");
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
    fn the_whole_tile_is_white_and_empty() {
        let mut s = sample();
        s.chunks[100].area = 9; // another zone on the tile
        let base = s.write();
        let d = make_pad(&base, den_tile(), pad(), 15).unwrap();
        assert_eq!(d.models, Some(vec![]), "every model and building taken off");
        let t = Adt::parse(&delta::apply(&d, Some(&base), None).unwrap(), None).unwrap();
        assert!(t.doodads.is_empty() && t.wmos.is_empty() && t.m2_names.is_empty() && t.wmo_names.is_empty());
        for c in &t.chunks {
            assert_eq!(c.layers.len(), 1);
            assert_eq!(t.textures[c.layers[0].texture as usize], PAD_TEXTURE);
            assert!(c.refs.is_empty() && c.colors.is_none() && c.shadow.is_none() && c.area == 876);
        }
        assert!(t.water.as_ref().unwrap().iter().all(|w| w.layers.is_empty()));
        let b = pad_texture();
        assert_eq!(crate::blp::header(&b).unwrap().width, PAD_TEXTURE_SIZE);
    }

    #[test]
    fn a_ground_from_before_is_whitened_keeping_what_the_player_made() {
        // the ground as it was made before: the 4 chunks flat, painted the island's grass, its tree and hut still
        // there; then the player added a bush in Forge and raised chunk 136
        let s = sample();
        let base = s.write();
        let mut e = s.clone();
        let grass = e.texture_index("Tileset\\Leyline\\Grass.blp");
        for i in [135usize, 136, 151, 152] {
            e.chunks[i].set_abs_heights(&[DEN.2; 145]);
            e.chunks[i].layers = vec![Layer { texture: grass, flags: 0, effect: 0, alpha: None }];
            e.chunks[i].colors = Some(vec![9; 580]);
        }
        e.chunks[136].heights.iter_mut().for_each(|h| *h += 4.0);
        let mut ms = delta::models_of(&e, [0.0; 3]);
        ms.push(delta::Model { path: "World\\Leyline\\Bush.m2".into(), uid: Some(delta::UID_BASE + 0x100), pos: [DEN.0 as f64 - 3.0, DEN.1 as f64 - 3.0, DEN.2 as f64], rot: None, scale: None, flags: None, set: None, name_set: None, bounds: None });
        delta::set_models(&mut e, &ms, [0.0; 3]);
        let mut o = DiffOptions::new(den_tile());
        o.open = Some(vec![135, 136, 151, 152]);
        o.pad = Some(Pad { height: DEN.2, texture: "Tileset\\Leyline\\Grass.blp".into(), centre: None });
        let old = delta::diff_tiles(&Adt::parse(&base, None).unwrap(), &e, &o).unwrap().0;
        let (d, w) = whiten(&base, &old).unwrap().expect("something to do");
        assert_eq!(w, Whitened { repainted: 4, models: 2, area: 0 });
        assert_eq!(d.pad.as_ref().unwrap().texture, PAD_TEXTURE);
        let t = Adt::parse(&delta::apply(&d, Some(&base), None).unwrap(), None).unwrap();
        for i in [135usize, 136, 151, 152] {
            assert_eq!(t.textures[t.chunks[i].layers[0].texture as usize], PAD_TEXTURE);
            assert!(t.chunks[i].colors.is_none());
        }
        assert!((t.chunks[136].abs_heights()[0] - (DEN.2 + 4.0)).abs() < 0.01, "the raised chunk stays raised");
        let left = delta::models_of(&t, [0.0; 3]);
        let names: Vec<&str> = left.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(names, ["World\\Leyline\\Rock.m2", "World\\Leyline\\Bush.m2"], "the tree and the hut went, the player's bush stayed");
        // once is enough
        assert!(whiten(&base, &d).unwrap().is_none());
        // a ground with no pad, or on a blank base: left alone
        assert!(whiten(&base, &Delta::new(den_tile(), vec![1])).unwrap().is_none());
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
