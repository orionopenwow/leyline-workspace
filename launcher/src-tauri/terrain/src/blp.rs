//! Our own textures, made by code (never a game file): a BLP2 texture of one colour, for the Den's white ground.
//!
//! The form (wowdev wiki "BLP", the public description): "BLP2", type 1, then compression (1 = palette), alpha depth
//! (0 = none), alpha type, "has mips"; width and height; 16 mip offsets and 16 mip sizes (from the file's start); a
//! palette of 256 BGRA colours; then each mip level's pixels, one palette index a pixel, largest first, down to 1 x 1.
//! The client wants every mip level of a terrain texture.

use crate::{err, Result};

pub const HEADER: usize = 148;
pub const PALETTE: usize = 1024;

/// What a BLP2 header says (for tests and checks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub compression: u8,
    pub alpha_depth: u8,
    pub alpha_type: u8,
    pub has_mips: u8,
    pub width: u32,
    pub height: u32,
    /// (offset, size) of each mip level present.
    pub mips: Vec<(u32, u32)>,
}

/// A square texture `size` pixels a side (a power of two, 1 to 4096), every pixel `bgra`, with all its mip levels.
pub fn solid(size: u32, bgra: [u8; 4]) -> Result<Vec<u8>> {
    if !size.is_power_of_two() || size > 4096 {
        return err("A texture's side is a power of two, 4096 at most.");
    }
    let levels = size.trailing_zeros() + 1; // 256: 256, 128, ... 1 = 9 levels
    let mut o = Vec::new();
    o.extend_from_slice(b"BLP2");
    o.extend_from_slice(&1u32.to_le_bytes());
    o.extend_from_slice(&[1, 0, 0, 1]); // palette, no alpha, alpha type 0, mips
    o.extend_from_slice(&size.to_le_bytes());
    o.extend_from_slice(&size.to_le_bytes());
    let (mut offs, mut sizes) = ([0u32; 16], [0u32; 16]);
    let mut at = (HEADER + PALETTE) as u32;
    for l in 0..levels as usize {
        let s = (size >> l).max(1);
        offs[l] = at;
        sizes[l] = s * s;
        at += s * s;
    }
    offs.iter().chain(sizes.iter()).for_each(|v| o.extend_from_slice(&v.to_le_bytes()));
    // the palette: every entry the colour (index 0 is the one the pixels use)
    for _ in 0..256 {
        o.extend_from_slice(&bgra);
    }
    o.resize(at as usize, 0);
    Ok(o)
}

/// The same as [`solid`] in the form the game's own terrain textures use: DXT1 (compression 2, no alpha), every
/// 4 x 4 block the one colour (RGB565 `c`, both end colours, every index 0), every mip level down to 1 x 1 (a level
/// under 4 pixels is still one block). The owner's game drew the palette form as bright green (2 Nov 01:37).
pub fn solid_dxt1(size: u32, c: u16) -> Result<Vec<u8>> {
    if !size.is_power_of_two() || size > 4096 {
        return err("A texture's side is a power of two, 4096 at most.");
    }
    let levels = size.trailing_zeros() + 1;
    let mut o = Vec::new();
    o.extend_from_slice(b"BLP2");
    o.extend_from_slice(&1u32.to_le_bytes());
    o.extend_from_slice(&[2, 0, 0, 1]); // DXT, no alpha, alpha type 0 (DXT1), mips
    o.extend_from_slice(&size.to_le_bytes());
    o.extend_from_slice(&size.to_le_bytes());
    let (mut offs, mut sizes) = ([0u32; 16], [0u32; 16]);
    let mut at = (HEADER + PALETTE) as u32;
    for l in 0..levels as usize {
        let s = (size >> l).max(1);
        let blocks = s.div_ceil(4) * s.div_ceil(4);
        offs[l] = at;
        sizes[l] = blocks * 8;
        at += blocks * 8;
    }
    offs.iter().chain(sizes.iter()).for_each(|v| o.extend_from_slice(&v.to_le_bytes()));
    o.resize(HEADER + PALETTE, 0); // the palette, unused by DXT
    let block = [c.to_le_bytes()[0], c.to_le_bytes()[1], c.to_le_bytes()[0], c.to_le_bytes()[1], 0, 0, 0, 0];
    while o.len() < at as usize {
        o.extend_from_slice(&block);
    }
    Ok(o)
}

/// The header of a BLP2 texture, when it is one.
pub fn header(b: &[u8]) -> Option<Header> {
    if b.len() < HEADER || &b[..4] != b"BLP2" {
        return None;
    }
    let u = |at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
    let mips = (0..16).map(|l| (u(20 + l * 4), u(84 + l * 4))).filter(|m| m.0 != 0 && m.1 != 0).collect();
    Some(Header { compression: b[8], alpha_depth: b[9], alpha_type: b[10], has_mips: b[11], width: u(12), height: u(16), mips })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_white_texture_with_every_mip_level_in_the_right_place() {
        let b = solid(256, [255, 255, 255, 255]).unwrap();
        let h = header(&b).unwrap();
        assert_eq!((h.compression, h.alpha_depth, h.alpha_type, h.has_mips, h.width, h.height), (1, 0, 0, 1, 256, 256));
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 1, "type 1");
        assert_eq!(h.mips.len(), 9, "256 down to 1");
        assert_eq!(h.mips[0], (1172, 65536), "right after the 148-byte header and the 1024-byte palette");
        let mut at = 1172;
        for (l, (off, size)) in h.mips.iter().enumerate() {
            let s = 256u32 >> l;
            assert_eq!((*off, *size), (at, s * s), "level {l}");
            at += s * s;
        }
        assert_eq!(h.mips[8].1, 1, "1 x 1 last");
        assert_eq!(b.len() as u32, at, "nothing after the last level");
        // the palette's first colour is white, every pixel uses it
        assert_eq!(&b[148..152], &[255, 255, 255, 255]);
        assert!(b[1172..].iter().all(|&p| p == 0));
        // the unused mip slots are zero
        assert!(b[20 + 9 * 4..84].iter().all(|&x| x == 0) && b[84 + 9 * 4..148].iter().all(|&x| x == 0));
        let small = solid(64, [250, 250, 250, 255]).unwrap();
        assert_eq!(header(&small).unwrap().mips.len(), 7);
        assert_eq!(small.len(), 1172 + 4096 + 1024 + 256 + 64 + 16 + 4 + 1);
        assert!(solid(100, [0; 4]).is_err() && solid(8192, [0; 4]).is_err());
        assert!(header(b"BLP1....").is_none());
    }

    #[test]
    fn a_white_dxt1_texture_with_every_mip_level() {
        let b = solid_dxt1(256, 0xFFFF).unwrap();
        let h = header(&b).unwrap();
        assert_eq!((h.compression, h.alpha_depth, h.alpha_type, h.has_mips, h.width, h.height), (2, 0, 0, 1, 256, 256));
        assert_eq!(h.mips.len(), 9);
        assert_eq!(h.mips[0], (1172, 64 * 64 * 8), "64 x 64 blocks of 8 bytes after the header and palette");
        assert_eq!(h.mips[6].1, 8, "4 x 4: one block");
        assert_eq!(h.mips[8].1, 8, "1 x 1: still one block");
        let end = h.mips[8].0 + h.mips[8].1;
        assert_eq!(b.len() as u32, end);
        assert_eq!(&b[1172..1180], &[0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]);
    }
}
