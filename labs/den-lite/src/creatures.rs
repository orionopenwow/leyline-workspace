//! Creature entry -> model (display id), for mounts: a mount spell's aura (SPELL_AURA_MOUNTED, 78) names a creature
//! entry, and the game shows the mount from the model the server puts in UNIT_FIELD_MOUNTDISPLAYID. The table is
//! generated from AzerothCore's own world database (`creature_template_model`; tools/gen_creature_models.py, as
//! objects.rs from its gameobject_template): the open project's data, not the game's files.

use std::collections::HashMap;
use std::sync::OnceLock;

static DATA: &[u8] = include_bytes!("creature_models.bin");
static MAP: OnceLock<HashMap<u32, u32>> = OnceLock::new();

fn varint(b: &[u8], at: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    for shift in (0..35).step_by(7) {
        let x = *b.get(*at)?;
        *at += 1;
        v |= ((x & 0x7F) as u32) << shift;
        if x & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// The table's bytes read: entry -> display.
pub fn decode(b: &[u8]) -> HashMap<u32, u32> {
    let mut m = HashMap::new();
    if b.len() < 8 || &b[..4] != b"LDCM" {
        return m;
    }
    let n = u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize;
    let (mut at, mut entry) = (8usize, 0u32);
    for _ in 0..n {
        let (Some(d), Some(disp)) = (varint(b, &mut at), varint(b, &mut at)) else { break };
        entry = entry.wrapping_add(d);
        m.insert(entry, disp);
    }
    m
}

/// A creature's model, when the table knows it.
pub fn display(entry: u32) -> Option<u32> {
    MAP.get_or_init(|| decode(DATA)).get(&entry).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_reads_back() {
        let m = decode(DATA);
        assert!(m.len() > 20_000, "{}", m.len());
        // AzerothCore's first rows: (1, 0, 10045), (3, 0, 987)
        assert_eq!(display(1), Some(10045));
        assert_eq!(display(3), Some(987));
        assert_eq!(display(2), None);
        assert!(decode(b"LDCX").is_empty());
    }
}
