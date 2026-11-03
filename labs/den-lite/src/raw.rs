//! The two server messages Den Lite writes itself, because the message crate can't say them right for 3.3.5a:
//!
//! - **Gameobject creates** in `SMSG_UPDATE_OBJECT`: the crate's `UpdateGameObjectBuilder` sets only the first of
//!   the four `GAMEOBJECT_PARENTROTATION` floats (its generic setters are crate-private), and a stock client wants
//!   the identity quaternion (0, 0, 0, 1) there, as AzerothCore sends (`gameobject_addon` defaults). The test
//!   client reads these blocks back with the crate's own parser, so the two agree on the layout.
//! - **`SMSG_GAMEOBJECT_QUERY_RESPONSE`**: the crate has 6 data fields; 3.3.5a has 24 (MAX_GAMEOBJECT_DATA).
//! - **Values updates** of single fields (the editor's selection glow, `GAMEOBJECT_DYNAMIC`; the far camera,
//!   `PLAYER_FARSIGHT`): a few words, read back by the headless client's own reader.
//!
//! Everything else goes through `wow_world_messages`.

use crate::den::WorldObject;

pub const SMSG_UPDATE_OBJECT: u16 = 0x00A9;
pub const SMSG_GAMEOBJECT_QUERY_RESPONSE: u16 = 0x005F;

/// 3.3.5a update fields (UpdateFields.h, 12340).
const OBJECT_FIELD_TYPE: usize = 2;
const OBJECT_FIELD_ENTRY: usize = 3;
const OBJECT_FIELD_SCALE_X: usize = 4;
const GAMEOBJECT_DISPLAYID: usize = 8;
const GAMEOBJECT_FLAGS: usize = 9;
const GAMEOBJECT_PARENTROTATION: usize = 10;
/// The dynamic flags (low half: GO_DYNFLAG_LO_*, high half: a transport's path progress).
pub const GAMEOBJECT_DYNAMIC: usize = 14;
const GAMEOBJECT_BYTES_1: usize = 17;
/// The player's far sight (a guid, two words): the client's camera goes to that object (UNIT_END 148 + 0x1DC).
pub const PLAYER_FARSIGHT: usize = 624;
const UPDATETYPE_VALUES: u8 = 0;
const GAMEOBJECT_END: usize = 18;
const TYPEMASK_OBJECT_GAMEOBJECT: u32 = 0x0001 | 0x0020;
const TYPEID_GAMEOBJECT: u8 = 5;
const UPDATETYPE_CREATE_OBJECT: u8 = 2;
/// UPDATEFLAG_LOWGUID | UPDATEFLAG_STATIONARY_POSITION | UPDATEFLAG_ROTATION: what a gameobject carries.
const GO_UPDATE_FLAGS: u16 = 0x0010 | 0x0040 | 0x0200;
const GO_STATE_READY: u8 = 1;

pub fn packed_guid(out: &mut Vec<u8>, guid: u64) {
    let mut mask = 0u8;
    let at = out.len();
    out.push(0);
    for i in 0..8 {
        let b = (guid >> (i * 8)) as u8;
        if b != 0 {
            mask |= 1 << i;
            out.push(b);
        }
    }
    out[at] = mask;
}

/// The rotation as AzerothCore makes it: yaw about z, then pitch about y, then roll about x
/// (`G3D::Matrix3::fromEulerAnglesZYX`), as (x, y, z, w).
pub fn quaternion(yaw: f32, pitch: f32, roll: f32) -> [f32; 4] {
    let (sy, cy) = (yaw / 2.0).sin_cos();
    let (sp, cp) = (pitch / 2.0).sin_cos();
    let (sr, cr) = (roll / 2.0).sin_cos();
    [
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    ]
}

/// The 64-bit packed local rotation (GameObject::UpdatePackedRotation).
pub fn packed_rotation(q: [f32; 4]) -> u64 {
    const PACK_YZ: f32 = (1 << 20) as f32;
    const PACK_X: f32 = (1 << 21) as f32;
    const PACK_YZ_MASK: i64 = (1 << 21) - 1;
    const PACK_X_MASK: i64 = (1 << 22) - 1;
    let sign = if q[3] >= 0.0 { 1 } else { -1 };
    let x = ((q[0] * PACK_X) as i32 * sign) as i64 & PACK_X_MASK;
    let y = ((q[1] * PACK_YZ) as i32 * sign) as i64 & PACK_YZ_MASK;
    let z = ((q[2] * PACK_YZ) as i32 * sign) as i64 & PACK_YZ_MASK;
    (z | (y << 21) | (x << 42)) as u64
}

/// One create block for a gameobject, as it is.
pub fn gameobject_block(out: &mut Vec<u8>, o: &WorldObject) {
    gameobject_block_shown(out, o, o.kind, 0, o.entry)
}

/// One create block for a gameobject with the type and flags the client is told (the editor's build mode shows
/// props as goobers: gizmo.rs `shown`).
pub fn gameobject_block_shown(out: &mut Vec<u8>, o: &WorldObject, kind: u8, flags: u32, entry: u32) {
    out.push(UPDATETYPE_CREATE_OBJECT);
    packed_guid(out, o.guid);
    out.push(TYPEID_GAMEOBJECT);
    // movement block
    out.extend_from_slice(&GO_UPDATE_FLAGS.to_le_bytes());
    for v in [o.x, o.y, o.z, o.o] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&((o.guid & 0xFF_FFFF) as u32).to_le_bytes());
    let q = quaternion(o.o, o.pitch, o.roll);
    out.extend_from_slice(&packed_rotation(q).to_le_bytes());
    // values
    let mut fields = [None::<u32>; GAMEOBJECT_END];
    fields[0] = Some(o.guid as u32);
    fields[1] = Some((o.guid >> 32) as u32);
    fields[OBJECT_FIELD_TYPE] = Some(TYPEMASK_OBJECT_GAMEOBJECT);
    fields[OBJECT_FIELD_ENTRY] = Some(entry);
    fields[OBJECT_FIELD_SCALE_X] = Some(o.scale.to_bits());
    fields[GAMEOBJECT_DISPLAYID] = Some(o.display);
    fields[GAMEOBJECT_FLAGS] = Some(flags);
    // the parent rotation: identity (no transport)
    fields[GAMEOBJECT_PARENTROTATION + 3] = Some(1.0f32.to_bits());
    // state ready, the type, art kit 0, anim progress 100
    fields[GAMEOBJECT_BYTES_1] = Some(GO_STATE_READY as u32 | (kind as u32) << 8 | 100 << 24);
    let mut mask = 0u32;
    for (i, f) in fields.iter().enumerate() {
        if f.is_some() {
            mask |= 1 << i;
        }
    }
    out.push(1); // one mask block covers fields 0..32
    out.extend_from_slice(&mask.to_le_bytes());
    for f in fields.iter().flatten() {
        out.extend_from_slice(&f.to_le_bytes());
    }
}

/// SMSG_UPDATE_OBJECT bodies creating these objects, `per` blocks a message.
pub fn gameobject_creates(objs: &[WorldObject], per: usize) -> Vec<Vec<u8>> {
    objs.chunks(per.max(1))
        .map(|chunk| {
            let mut out = Vec::with_capacity(4 + chunk.len() * 80);
            out.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            for o in chunk {
                gameobject_block(&mut out, o);
            }
            out
        })
        .collect()
}

/// As [`gameobject_creates`], each object's type and flags from `shown`.
pub fn gameobject_creates_shown(objs: &[WorldObject], per: usize, shown: impl Fn(&WorldObject) -> (u8, u32, u32)) -> Vec<Vec<u8>> {
    objs.chunks(per.max(1))
        .map(|chunk| {
            let mut out = Vec::with_capacity(4 + chunk.len() * 80);
            out.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            for o in chunk {
                let (kind, flags, entry) = shown(o);
                gameobject_block_shown(&mut out, o, kind, flags, entry);
            }
            out
        })
        .collect()
}

/// An SMSG_UPDATE_OBJECT body changing only these fields of one object (one block of UPDATETYPE_VALUES: its packed
/// guid, as many mask words as the highest field needs, the values in field order).
pub fn values_body(guid: u64, fields: &[(usize, u32)]) -> Vec<u8> {
    let mut f = fields.to_vec();
    f.sort_by_key(|x| x.0);
    f.dedup_by_key(|x| x.0);
    let blocks = f.last().map(|x| x.0 / 32 + 1).unwrap_or(0);
    let mut out = Vec::with_capacity(16 + 4 * blocks + 4 * f.len());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.push(UPDATETYPE_VALUES);
    packed_guid(&mut out, guid);
    out.push(blocks as u8);
    let mut mask = vec![0u32; blocks];
    for (i, _) in &f {
        mask[i / 32] |= 1 << (i % 32);
    }
    for m in mask {
        out.extend_from_slice(&m.to_le_bytes());
    }
    for (_, v) in &f {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// A gameobject's dynamic flags (the selection's glow: gizmo::GLOW on, 0 off).
pub fn gameobject_dynamic(guid: u64, flags: u32) -> Vec<u8> {
    values_body(guid, &[(GAMEOBJECT_DYNAMIC, flags)])
}

/// The player's far sight on `target` (0: back to the player's own view).
pub fn farsight(player: u64, target: u64) -> Vec<u8> {
    values_body(player, &[(PLAYER_FARSIGHT, target as u32), (PLAYER_FARSIGHT + 1, (target >> 32) as u32)])
}

fn cstring(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

/// SMSG_GAMEOBJECT_QUERY_RESPONSE for a known entry (WorldSession::HandleGameObjectQueryOpcode), or the
/// "not found" form (entry | 0x80000000).
pub fn gameobject_query_response(entry: u32, found: Option<(u8, u32, &str, f32)>) -> Vec<u8> {
    let mut out = Vec::with_capacity(160);
    match found {
        None => out.extend_from_slice(&(entry | 0x8000_0000).to_le_bytes()),
        Some((kind, display, name, size)) => {
            out.extend_from_slice(&entry.to_le_bytes());
            out.extend_from_slice(&(kind as u32).to_le_bytes());
            out.extend_from_slice(&display.to_le_bytes());
            cstring(&mut out, name);
            for _ in 0..3 {
                cstring(&mut out, "");
            }
            for _ in 0..3 {
                cstring(&mut out, ""); // icon, cast bar caption, unk1
            }
            out.extend_from_slice(&[0u8; 24 * 4]); // data0..23: every Lite prop is plain decor
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&[0u8; 6 * 4]); // quest items
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_type_and_flags_the_client_is_told() {
        let o = WorldObject { guid: 0xF110_0009_6D00_0001, entry: 2413, display: 39, kind: 7, scale: 1.0, x: 1.0, y: 2.0, z: 3.0, o: 0.0, pitch: 0.0, roll: 0.0 };
        let (mut a, mut b) = (Vec::new(), Vec::new());
        gameobject_block(&mut a, &o);
        gameobject_block_shown(&mut b, &o, 10, 0x10, 2413 | 0x0100_0000);
        assert_eq!(a.len(), b.len());
        // the values end: ..., flags (field 9), the rotation (10 ..= 13, only 13 set), bytes_1 (17)
        let tail = |v: &[u8], k: usize| u32::from_le_bytes(v[v.len() - 4 * k..v.len() - 4 * k + 4].try_into().unwrap());
        assert_eq!((tail(&a, 1) >> 8) & 0xFF, 7, "as it is: a chair");
        assert_eq!((tail(&b, 1) >> 8) & 0xFF, 10, "a goober");
        assert_eq!((tail(&a, 3), tail(&b, 3)), (0, 0x10), "the flags");
        // the entry (field 3): sixth from the end (entry, scale, display, flags, rotation, bytes_1); b's the build twin's
        assert_eq!((tail(&a, 6), tail(&b, 6)), (2413, 2413 | 0x0100_0000));
        assert_eq!(gameobject_creates_shown(std::slice::from_ref(&o), 8, |o| (o.kind, 0, o.entry)), gameobject_creates(std::slice::from_ref(&o), 8));
    }

    #[test]
    fn values_updates_of_one_field() {
        // the glow: one mask word, bit 14, the flags
        let b = gameobject_dynamic(0xF110_0009_6D00_0001, 0x09);
        let mut want = 1u32.to_le_bytes().to_vec();
        want.push(0);
        packed_guid(&mut want, 0xF110_0009_6D00_0001);
        want.push(1);
        want.extend_from_slice(&(1u32 << 14).to_le_bytes());
        want.extend_from_slice(&9u32.to_le_bytes());
        assert_eq!(b, want);
        // far sight: 20 mask words (fields up to 639), bits 16 and 17 of the 20th, the guid's two halves
        let b = farsight(7, 0xF130_00F1_C200_0001);
        let at = 4 + 1 + 2 + 1;
        assert_eq!((b[4], b[7]), (0, 20));
        let word = |k: usize| u32::from_le_bytes(b[at + 4 * k..at + 4 * k + 4].try_into().unwrap());
        assert!((0..19).all(|k| word(k) == 0) && word(19) == 0b11 << 16);
        assert_eq!((word(20), word(21)), (0xC200_0001, 0xF130_00F1));
        assert_eq!(b.len(), at + 4 * 22);
        let off = farsight(7, 0);
        assert_eq!(&off[off.len() - 8..], &[0u8; 8]);
    }

    #[test]
    fn packed_guids() {
        let mut v = Vec::new();
        packed_guid(&mut v, 0xF110_0000_0000_0001);
        assert_eq!(v, vec![0b1100_0001, 0x01, 0x10, 0xF1]);
    }

    #[test]
    fn rotation_packs_a_quarter_turn() {
        let q = quaternion(std::f32::consts::FRAC_PI_2, 0.0, 0.0);
        assert!((q[2] - 0.70710677).abs() < 1e-5 && (q[3] - 0.70710677).abs() < 1e-5);
        let p = packed_rotation(q);
        assert_eq!(p >> 42, 0);
        assert_eq!(p & ((1 << 21) - 1), (0.70710677f32 * (1 << 20) as f32) as u64);
        assert_eq!(packed_rotation(quaternion(0.0, 0.0, 0.0)), 0);
    }
}
