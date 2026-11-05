//! The companion in the air and around things (owner 3 Nov: "when user flies have Jaina fly with, more
//! dynamically"; "fix Jaina collision"; "she fades away after a certain distance").
//!
//! * **Flying with you.** While you fly (`.den fly on` and more than [`AIRBORNE`] above the ground) she flies too:
//!   the creature is told it flies (SMSG_SPLINE_MOVE_SET_FLYING, SMSG_SPLINE_MOVE_GRAVITY_DISABLE: its packed guid,
//!   as AzerothCore's Creature::SetCanFly / SetDisableGravity tell the players around), then every [`SKY_EVERY_MS`]
//!   that her spot moved more than a yard, a curved path to it ([`sky_body`]: SMSG_MONSTER_MOVE with MoveSplineFlag
//!   Flying 0x2000 | Catmullrom 0x40000, so the points go uncompressed, as AzerothCore's PacketBuilder writes them),
//!   [`FOLLOW`] yards behind you and [`ABOVE`] above your height, at your own flight speed. You land: she lands beside
//!   you (a path down to the ground, then SMSG_SPLINE_MOVE_UNSET_FLYING and GRAVITY_ENABLE).
//! * **Around props.** Her walk to her spot goes around a prop standing in the straight line (a waypoint beside it,
//!   [`path`]), never ends inside one, and never closer than [`PERSONAL`] yards to you.
//! * **Hopping when far** (owner 4 Nov: "flying with flight.rs when far, landing"): following on foot and more than
//!   [`HOP`] yards from her spot (but not past `companion::FAR`, where she is put back at once), she hops: told it
//!   flies, an arc up over whatever stands between ([`hop_path`]), and down beside you, told it walks again.
//! * **Smoother walks.** A waypoint not needed (the straight line past it is clear) is dropped ([`smooth`]), and of
//!   the two sides of a prop she takes the shorter way round.
//! * **Keeping up.** She stayed in the Den's own circle (22 yards at size 1): past it she went, which looked like
//!   fading away. Now she follows you anywhere within [`AREA`] yards of the Den, and past [`crate::companion::FAR`]
//!   yards (a jump, fast flying) she is put back behind you at once.

use crate::companion::{spot, At, ANGLE};
use crate::raw::packed_guid;
use std::f32::consts::PI;

pub const SMSG_MONSTER_MOVE: u16 = 0x0DD;
pub const SMSG_SPLINE_MOVE_SET_FLYING: u16 = 0x422;
pub const SMSG_SPLINE_MOVE_UNSET_FLYING: u16 = 0x423;
pub const SMSG_SPLINE_MOVE_GRAVITY_DISABLE: u16 = 0x4D3;
pub const SMSG_SPLINE_MOVE_GRAVITY_ENABLE: u16 = 0x4D4;
/// MoveSplineFlag (3.3.5a).
pub const SPLINE_FLYING: u32 = 0x0000_2000;
pub const SPLINE_CATMULLROM: u32 = 0x0004_0000;

/// You count as flying this high above the ground (with flying on).
pub const AIRBORNE: f32 = 1.5;
/// How often her flight is steered, and how far her spot must have moved.
pub const SKY_EVERY_MS: u64 = 300;
pub const SKY_SLACK: f32 = 1.0;
/// Behind you and a little above, in the air.
pub const FOLLOW: f32 = 2.5;
pub const ABOVE: f32 = 0.8;
/// She never ends a walk closer to you than this.
pub const PERSONAL: f32 = 2.0;
/// She follows you this far from the Den's middle (the island); further, she waits in the Den.
pub const AREA: f32 = 300.0;
/// Room around a prop.
pub const CLEAR: f32 = 0.6;
/// Further than this from her spot (on foot): a hop through the air. Her hop's speed and its arc's height.
pub const HOP: f32 = 24.0;
pub const HOP_SPEED: f32 = 14.0;
pub const HOP_TOP_MIN: f32 = 3.0;
pub const HOP_TOP_MAX: f32 = 10.0;

/// A prop's footprint: a circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Block {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

/// A prop's footprint from its model's size (models.rs tile field `path,WxDxHxZ`) and scale; no size known: a
/// small one.
pub fn block_of(x: f32, y: f32, scale: f32, tile: Option<&str>) -> Block {
    let size = tile.and_then(|t| t.split_once(',')).and_then(|(_, s)| {
        let v: Vec<f32> = s.split('x').filter_map(|n| n.parse().ok()).collect();
        (v.len() == 4).then(|| v[0].max(v[1]))
    });
    let r = (size.unwrap_or(1.2) * 0.5 * scale.max(0.1)).clamp(0.3, 8.0);
    Block { x, y, r }
}

/// Is a flier at `p` in the air (flying on, above the ground).
pub fn airborne(fly_on: bool, p: At, ground: f32) -> bool {
    fly_on && p.z > ground + AIRBORNE
}

/// Her spot in the air: behind you as on the ground, at your height and a little above.
pub fn sky_spot(p: At) -> At {
    let s = spot(p);
    At { z: p.z + ABOVE, ..s }
}

/// Within reach of the Den for following (the island): horizontal distance from its middle.
pub fn near_den(mid_x: f32, mid_y: f32, p: At) -> bool {
    ((p.x - mid_x).powi(2) + (p.y - mid_y).powi(2)).sqrt() <= AREA
}

fn d2(a: At, b: At) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// The length of a path from `from` through `points`.
pub fn length(from: At, points: &[At]) -> f32 {
    let mut at = from;
    let mut n = 0.0;
    for p in points {
        n += ((p.x - at.x).powi(2) + (p.y - at.y).powi(2) + (p.z - at.z).powi(2)).sqrt();
        at = *p;
    }
    n
}

/// Where a walk may end: not inside a prop (pushed out of it) and not within PERSONAL yards of you.
pub fn safe_end(to: At, player: Option<At>, blocks: &[Block]) -> At {
    let mut t = to;
    let far = At { x: 1.0e9, y: 0.0, z: 0.0, o: 0.0 };
    let player = player.unwrap_or(far);
    for _ in 0..3 {
        let dp = d2(t, player);
        if dp < PERSONAL {
            let (ux, uy) = if dp > 1e-3 { ((t.x - player.x) / dp, (t.y - player.y) / dp) } else { ((player.o + ANGLE).cos(), (player.o + ANGLE).sin()) };
            t = At { x: player.x + ux * PERSONAL, y: player.y + uy * PERSONAL, ..t };
        }
        let mut moved = false;
        for b in blocks {
            let d = ((t.x - b.x).powi(2) + (t.y - b.y).powi(2)).sqrt();
            if d < b.r + CLEAR {
                let (ux, uy) = if d > 1e-3 { ((t.x - b.x) / d, (t.y - b.y) / d) } else { (1.0, 0.0) };
                t = At { x: b.x + ux * (b.r + CLEAR), y: b.y + uy * (b.r + CLEAR), ..t };
                moved = true;
            }
        }
        if !moved && d2(t, player) >= PERSONAL - 1e-3 {
            break;
        }
    }
    t
}

/// The first prop the straight line from `a` to `b` passes through (nearest along the line).
fn crossing(a: At, b: At, blocks: &[Block]) -> Option<(Block, f32)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-6 {
        return None;
    }
    blocks
        .iter()
        .filter_map(|blk| {
            let t = (((blk.x - a.x) * dx + (blk.y - a.y) * dy) / len2).clamp(0.0, 1.0);
            let (cx, cy) = (a.x + dx * t, a.y + dy * t);
            let d = ((blk.x - cx).powi(2) + (blk.y - cy).powi(2)).sqrt();
            // not the one she stands in or ends at (they are pushed out of elsewhere)
            let end = ((blk.x - b.x).powi(2) + (blk.y - b.y).powi(2)).sqrt() < blk.r || ((blk.x - a.x).powi(2) + (blk.y - a.y).powi(2)).sqrt() < blk.r;
            (d < blk.r + CLEAR * 0.5 && !end && t > 0.0 && t < 1.0).then_some((*blk, t))
        })
        .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal))
}

/// Her walk from `from` to `to` with you at `player`: the points after `from` (waypoints around props, then the
/// end, made safe). Straight when nothing is in the way: one point.
pub fn path(from: At, to: At, player: Option<At>, blocks: &[Block]) -> Vec<At> {
    let end = safe_end(to, player, blocks);
    let mut points = Vec::new();
    let mut at = from;
    for _ in 0..4 {
        let Some((blk, _)) = crossing(at, end, blocks) else { break };
        // beside it, on the side the line passes (else the left), clear of it; when the waypoint on that side stands
        // in another prop and the other side's doesn't, the other side
        let (dx, dy) = (end.x - at.x, end.y - at.y);
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (nx, ny) = (-dy / len, dx / len);
        let side = if (blk.x - at.x) * nx + (blk.y - at.y) * ny > 0.0 { -1.0 } else { 1.0 };
        let r = blk.r + CLEAR + 0.4;
        let beside = |sd: f32| At { x: blk.x + nx * sd * r, y: blk.y + ny * sd * r, z: (at.z + end.z) / 2.0, o: end.o };
        let inside = |w: At| blocks.iter().any(|b| *b != blk && ((w.x - b.x).powi(2) + (w.y - b.y).powi(2)).sqrt() < b.r + CLEAR * 0.5);
        let w = if inside(beside(side)) && !inside(beside(-side)) { beside(-side) } else { beside(side) };
        points.push(w);
        at = w;
    }
    points.push(end);
    smooth(from, points, blocks)
}

/// The waypoints she doesn't need: one whose neighbours see each other past every prop goes.
pub fn smooth(from: At, mut points: Vec<At>, blocks: &[Block]) -> Vec<At> {
    let mut i = 0;
    while points.len() > 1 && i < points.len() - 1 {
        let prev = if i == 0 { from } else { points[i - 1] };
        if crossing(prev, points[i + 1], blocks).is_none() {
            points.remove(i);
        } else {
            i += 1;
        }
    }
    points
}

/// Her hop from `from` to `to`: points along an arc (a parabola up to a fifth of the way, between HOP_TOP_MIN and
/// HOP_TOP_MAX yards over the straight line), ending at `to`; how long at HOP_SPEED.
pub fn hop_path(from: At, to: At) -> (Vec<At>, u32) {
    let top = (d2(from, to) * 0.2).clamp(HOP_TOP_MIN, HOP_TOP_MAX);
    let o = (to.y - from.y).atan2(to.x - from.x).rem_euclid(2.0 * PI);
    let at = |t: f32| At {
        x: from.x + (to.x - from.x) * t,
        y: from.y + (to.y - from.y) * t,
        z: from.z + (to.z - from.z) * t + top * 4.0 * t * (1.0 - t),
        o,
    };
    let pts = vec![at(0.25), at(0.5), at(0.75), At { o, ..to }];
    let ms = ((length(from, &pts) / HOP_SPEED) * 1000.0).max(600.0) as u32;
    (pts, ms)
}

/// SMSG_MONSTER_MOVE along a path, uncompressed (Flying and/or Catmullrom): packed guid, a byte 0, the start, the
/// spline id, type 0 (normal), the flags, the duration, the count, every point after the start.
pub fn sky_body(guid: u64, from: At, points: &[At], id: u32, flags: u32, ms: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(32 + points.len() * 12);
    packed_guid(&mut b, guid);
    b.push(0);
    for v in [from.x, from.y, from.z] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&id.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&flags.to_le_bytes());
    b.extend_from_slice(&ms.to_le_bytes());
    b.extend_from_slice(&(points.len() as u32).to_le_bytes());
    for p in points {
        for v in [p.x, p.y, p.z] {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    b
}

/// SMSG_SPLINE_MOVE_SET_FLYING and the others: the packed guid.
pub fn guid_body(guid: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(9);
    packed_guid(&mut b, guid);
    b
}

/// Her flight from where she is to her spot: a curve through a point between, lifted a little; how long at
/// `speed` yards a second (at least SKY_EVERY_MS, so the next steer meets it).
pub fn sky_path(from: At, to: At, speed: f32) -> (Vec<At>, u32) {
    let mid = At { x: (from.x + to.x) / 2.0, y: (from.y + to.y) / 2.0, z: (from.z + to.z) / 2.0 + 0.5, o: to.o };
    let pts = vec![mid, to];
    let ms = ((length(from, &pts) / speed.max(1.0)) * 1000.0).max(SKY_EVERY_MS as f32) as u32;
    (pts, ms)
}

/// Her landing beside you: down to the ground under her spot.
pub fn landing(from: At, p: At, ground: f32) -> (Vec<At>, u32) {
    let s = spot(p);
    let to = At { z: ground, o: (p.y - s.y).atan2(p.x - s.x).rem_euclid(2.0 * PI), ..s };
    let pts = vec![to];
    let ms = ((length(from, &pts) / 7.0) * 1000.0).max(400.0) as u32;
    (pts, ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32) -> At {
        At { x, y, z: 0.0, o: 0.0 }
    }

    #[test]
    fn a_prop_in_the_way_gets_a_waypoint_beside_it() {
        let blocks = [Block { x: 5.0, y: 0.0, r: 1.0 }];
        let player = Some(At { x: 20.0, y: 0.0, z: 0.0, o: 0.0 });
        let p = path(at(0.0, 0.0), at(10.0, 0.0), player, &blocks);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!((p[0].x - 5.0).abs() < 1e-3 && p[0].y.abs() >= 1.0 + CLEAR, "beside it: {:?}", p[0]);
        assert_eq!((p[1].x, p[1].y), (10.0, 0.0));
        // nothing in the way: straight
        assert_eq!(path(at(0.0, 0.0), at(10.0, 0.0), player, &[Block { x: 5.0, y: 4.0, r: 1.0 }]).len(), 1);
        // the two segments are clear of it
        for (a, b) in [(at(0.0, 0.0), p[0]), (p[0], p[1])] {
            assert!(crossing(a, b, &blocks).is_none());
        }
    }

    #[test]
    fn smoother_walks_and_the_clear_side() {
        // two props side by side across the line: the waypoint beside the first on the near side would stand in the
        // second, so she goes round the other way
        let blocks = [Block { x: 5.0, y: 0.2, r: 1.0 }, Block { x: 5.0, y: -2.4, r: 1.0 }];
        let p = path(at(0.0, 0.0), at(10.0, 0.0), None, &blocks);
        assert!(p.len() >= 2, "{p:?}");
        assert!(p[0].y > 0.0, "round the free side: {:?}", p[0]);
        let mut a = at(0.0, 0.0);
        for q in &p {
            assert!(crossing(a, *q, &blocks).is_none(), "{a:?} -> {q:?}");
            a = *q;
        }
        // a waypoint with a clear view past it goes
        let pts = smooth(at(0.0, 0.0), vec![at(5.0, 3.0), at(10.0, 0.0)], &[]);
        assert_eq!(pts, vec![at(10.0, 0.0)]);
        let pts = smooth(at(0.0, 0.0), vec![at(5.0, 3.0), at(10.0, 0.0)], &[Block { x: 5.0, y: 0.0, r: 1.0 }]);
        assert_eq!(pts.len(), 2, "still needed");
    }

    #[test]
    fn a_hop_arcs_up_and_lands_where_it_should() {
        let from = At { x: 0.0, y: 0.0, z: 10.0, o: 0.0 };
        let to = At { x: 30.0, y: 0.0, z: 10.0, o: 0.0 };
        let (pts, ms) = hop_path(from, to);
        assert_eq!(pts.len(), 4);
        assert_eq!((pts[3].x, pts[3].y, pts[3].z), (30.0, 0.0, 10.0));
        assert!((pts[1].z - 16.0).abs() < 1e-3, "the top: a fifth of 30 yards: {:?}", pts[1]);
        assert!(pts[0].z > 10.0 && pts[2].z > 10.0 && pts[0].x < pts[1].x && pts[1].x < pts[2].x);
        assert!(ms > 2000 && ms < 3000, "{ms}");
        let (short, ms) = hop_path(from, At { x: 5.0, ..from });
        assert!((short[1].z - 13.0).abs() < 1e-3 && ms >= 600, "at least HOP_TOP_MIN up");
        let (long, _) = hop_path(from, At { x: 200.0, ..from });
        assert!((long[1].z - 20.0).abs() < 1e-3, "at most HOP_TOP_MAX up");
    }

    #[test]
    fn she_never_ends_inside_a_prop_or_on_top_of_you() {
        let blocks = [Block { x: 0.0, y: 0.0, r: 1.0 }];
        let e = safe_end(at(0.2, 0.0), Some(at(30.0, 0.0)), &blocks);
        assert!(((e.x).powi(2) + e.y.powi(2)).sqrt() >= 1.0 + CLEAR - 1e-3, "{e:?}");
        let player = at(10.0, 0.0);
        let e = safe_end(at(10.5, 0.0), Some(player), &[]);
        assert!((d2(e, player) - PERSONAL).abs() < 1e-3, "{e:?}");
        assert_eq!(safe_end(at(10.5, 0.0), None, &[]), at(10.5, 0.0), "no player known: as asked");
        assert!(block_of(0.0, 0.0, 1.0, Some("a\\b.m2,4.00x2.00x1.00x0.00")).r == 2.0);
        assert!(block_of(0.0, 0.0, 2.0, None).r > 1.0);
        assert!(block_of(0.0, 0.0, 1.0, Some("-wmo")).r > 0.0);
    }

    #[test]
    fn in_the_air_behind_and_above_you() {
        let p = At { x: 0.0, y: 0.0, z: 30.0, o: 0.0 };
        assert!(airborne(true, p, 13.2) && !airborne(false, p, 13.2) && !airborne(true, At { z: 14.0, ..p }, 13.2));
        let s = sky_spot(p);
        assert!((s.z - 30.8).abs() < 1e-4);
        assert!((d2(s, p) - FOLLOW).abs() < 1e-3);
        let (pts, ms) = sky_path(At { x: 10.0, ..s }, s, 21.0);
        assert_eq!(pts.len(), 2);
        assert!(pts[0].z > s.z, "the curve's middle is lifted");
        assert!(ms >= SKY_EVERY_MS as u32 && ms < 1000, "{ms}");
        let (down, ms) = landing(s, At { z: 13.2, ..p }, 13.2);
        assert_eq!(down[0].z, 13.2);
        assert!(ms > 2000, "a 17 yard drop at 7 a second: {ms}");
        assert!(near_den(0.0, 0.0, At { x: 250.0, ..p }) && !near_den(0.0, 0.0, At { x: 400.0, ..p }));
    }

    #[test]
    fn the_flying_move_body() {
        let from = At { x: 1.0, y: 2.0, z: 3.0, o: 0.0 };
        let pts = [At { x: 4.0, y: 5.0, z: 6.0, o: 0.0 }, At { x: 7.0, y: 8.0, z: 9.0, o: 0.0 }];
        let b = sky_body(1, from, &pts, 9, SPLINE_FLYING | SPLINE_CATMULLROM, 600);
        assert_eq!(&b[..3], &[1, 1, 0]);
        assert_eq!(f32::from_le_bytes(b[3..7].try_into().unwrap()), 1.0);
        assert_eq!(u32::from_le_bytes(b[15..19].try_into().unwrap()), 9);
        assert_eq!(b[19], 0, "normal");
        assert_eq!(u32::from_le_bytes(b[20..24].try_into().unwrap()), 0x42000);
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()), 600);
        assert_eq!(u32::from_le_bytes(b[28..32].try_into().unwrap()), 2);
        assert_eq!(f32::from_le_bytes(b[b.len() - 4..].try_into().unwrap()), 9.0);
        assert_eq!(b.len(), 32 + 24);
        assert_eq!(guid_body(crate::companion::GUID).len(), 1 + 5);
    }
}
