//! A map's WDT (wowdev wiki "WDT"): MVER (18), MPHD (flags and 28 more bytes), MAIN (64 x 64 entries of 8 bytes:
//! flags, bit 0 = the tile has an ADT, then an unused value), MWMO and MODF (a map that is one building). Entry
//! [y][x] is the tile `<dir>_<x>_<y>.adt`. MPHD flag 0x4 means the map's tiles keep 4096-byte alpha maps.

use crate::adt::{put, walk, AlphaMode};
use crate::{err, Error, Result};

pub const MPHD_BIG_ALPHA: u32 = 0x4;

#[derive(Debug, Clone, PartialEq)]
pub struct Wdt {
    pub version: u32,
    pub flags: u32,
    /// MPHD's other 28 bytes, kept.
    pub mphd_rest: Vec<u8>,
    /// 4096 entries of (flags, second value), row y then column x.
    pub main: Vec<(u32, u32)>,
    /// Every other chunk after MAIN (MWMO, MODF, ...), kept as it is.
    pub rest: Vec<([u8; 4], Vec<u8>)>,
}

impl Wdt {
    pub fn parse(b: &[u8]) -> Result<Wdt> {
        let top = walk(b)?;
        let get = |t: &[u8; 4]| top.iter().find(|c| &c.0 == t).map(|c| &b[c.1..c.1 + c.2]);
        let mver = get(b"MVER").filter(|d| d.len() >= 4).ok_or_else(|| Error("That isn't a map's WDT (no MVER).".into()))?;
        let mphd = get(b"MPHD").filter(|d| d.len() >= 4).ok_or_else(|| Error("That isn't a map's WDT (no MPHD).".into()))?;
        let main = get(b"MAIN").filter(|d| d.len() >= 64 * 64 * 8).ok_or_else(|| Error("That map's WDT has no tile table.".into()))?;
        let rd = |s: &[u8], at: usize| u32::from_le_bytes([s[at], s[at + 1], s[at + 2], s[at + 3]]);
        let rest = top.iter().filter(|c| !matches!(&c.0, b"MVER" | b"MPHD" | b"MAIN")).map(|c| (c.0, b[c.1..c.1 + c.2].to_vec())).collect();
        Ok(Wdt {
            version: rd(mver, 0),
            flags: rd(mphd, 0),
            mphd_rest: mphd[4..].to_vec(),
            main: (0..4096).map(|i| (rd(main, i * 8), rd(main, i * 8 + 4))).collect(),
            rest,
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(33 * 1024);
        put(&mut o, b"MVER", &self.version.to_le_bytes());
        let mut mphd = self.flags.to_le_bytes().to_vec();
        mphd.extend_from_slice(&self.mphd_rest);
        put(&mut o, b"MPHD", &mphd);
        let main: Vec<u8> = self.main.iter().flat_map(|(a, b)| a.to_le_bytes().into_iter().chain(b.to_le_bytes())).collect();
        put(&mut o, b"MAIN", &main);
        for (t, d) in &self.rest {
            put(&mut o, t, d);
        }
        o
    }

    /// An empty map: no tiles, the given flags, an empty MWMO.
    pub fn empty(flags: u32) -> Wdt {
        Wdt { version: 18, flags, mphd_rest: vec![0; 28], main: vec![(0, 0); 4096], rest: vec![(*b"MWMO", Vec::new())] }
    }

    pub fn has_tile(&self, x: u32, y: u32) -> bool {
        x < 64 && y < 64 && self.main[(y * 64 + x) as usize].0 & 1 != 0
    }

    pub fn set_tile(&mut self, x: u32, y: u32, on: bool) -> Result<()> {
        if x >= 64 || y >= 64 {
            return err("A map has 64 x 64 tiles.");
        }
        let e = &mut self.main[(y * 64 + x) as usize].0;
        *e = if on { *e | 1 } else { *e & !1 };
        Ok(())
    }

    pub fn alpha_mode(&self) -> AlphaMode {
        if self.flags & MPHD_BIG_ALPHA != 0 {
            AlphaMode::Big
        } else {
            AlphaMode::Small
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_and_round_trip() {
        let mut w = Wdt::empty(0);
        assert_eq!(w.alpha_mode(), AlphaMode::Small);
        w.set_tile(1, 1, true).unwrap();
        w.set_tile(63, 0, true).unwrap();
        assert!(w.set_tile(64, 0, true).is_err());
        let b = w.write();
        assert_eq!(&b[..4], b"REVM");
        let back = Wdt::parse(&b).unwrap();
        assert_eq!(back, w);
        assert!(back.has_tile(1, 1) && back.has_tile(63, 0) && !back.has_tile(0, 63) && !back.has_tile(2, 1));
        // entry [y][x]: tile (63, 0) is the 64th entry
        assert_eq!(back.main[63].0 & 1, 1);
        let mut big = Wdt::empty(MPHD_BIG_ALPHA | 0x2);
        big.rest.push((*b"MODF", vec![0; 64]));
        assert_eq!(Wdt::parse(&big.write()).unwrap().alpha_mode(), AlphaMode::Big);
        assert_eq!(Wdt::parse(&big.write()).unwrap().write(), big.write());
        assert!(Wdt::parse(b"junk").is_err());
        assert!(Wdt::parse(&b[..100]).is_err());
    }
}
