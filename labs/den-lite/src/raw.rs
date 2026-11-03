//! The two server messages Den Lite writes itself, because the message crate can't say them right for 3.3.5a:
//!
//! - **Gameobject creates** in `SMSG_UPDATE_OBJECT`: the crate's `UpdateGameObjectBuilder` sets only the first of
//!   the four `GAMEOBJECT_PARENTROTATION` floats (its generic setters are crate-private), and a stock client wants
//!   the identity quaternion (0, 0, 0, 1) there, as AzerothCore sends (`gameobject_addon` defaults). The test
//!   client reads these blocks back with the crate's own parser, so the two agree on the layout.
//! - **`SMSG_GAMEOBJECT_QUERY_RESPONSE`**: the crate has 6 data fields; 3.3.5a has 24 (MAX_GAMEOBJECT_DATA).
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
const GAMEOBJECT_BYTES_1: usize = 17;
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

/// One create block for a gameobject.
pub fn gameobject_block(out: &mut Vec<u8>, o: &WorldObject) {
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
    fields[OBJECT_FIELD_ENTRY] = Some(o.entry);
    fields[OBJECT_FIELD_SCALE_X] = Some(o.scale.to_bits());
    fields[GAMEOBJECT_DISPLAYID] = Some(o.display);
    fields[GAMEOBJECT_FLAGS] = Some(0);
    // the parent rotation: identity (no transport)
    fields[GAMEOBJECT_PARENTROTATION + 3] = Some(1.0f32.to_bits());
    // state ready, the type, art kit 0, anim progress 100
    fields[GAMEOBJECT_BYTES_1] = Some(GO_STATE_READY as u32 | (o.kind as u32) << 8 | 100 << 24);
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
