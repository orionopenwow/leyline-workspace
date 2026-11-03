//! Picking with the mouse, done by Den Lite (owner 3 Nov: "clicking objects, having them highlighted, moving them
//! around easily").
//!
//! The stock 3.3.5a client never tells the server about a left click on a gameobject, and its own highlight under
//! the mouse goes by the entry's cached template. But Den Lite knows the camera (the build camera, or exactly the
//! far view's: gizmo.rs `Aids::view`) and every prop's place, turn and size (models.rs boxes). So the addon sends
//! where the cursor is on the screen and Den Lite casts the ray itself: through the cursor, against each prop's
//! upright box (its model's width, depth and height, turned with the prop). The nearest hit wins.
//!
//! The drag: the cursor's ray meets a level plane at the height where the prop was grabbed; the prop's foot keeps
//! the offset it had from that point, so the spot you grabbed stays under the cursor.
//!
//! Std only (no crate types): it builds and tests on its own with `rustc --test src/pick.rs`.

pub type V3 = [f32; 3];

/// One prop as the ray sees it: an upright box turned with the prop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub id: u64,
    /// The prop's foot (its model's origin) in the world.
    pub at: V3,
    /// Its turn (radians, about the up axis).
    pub yaw: f32,
    /// Half its width (along its own facing) and half its depth (yards).
    pub half: [f32; 2],
    /// Where its box starts over the foot, and how tall it is (yards).
    pub bottom: f32,
    pub height: f32,
}

/// The least a box is, so a flat rug or a thin pole can still be clicked (yards).
pub const MIN_HALF: f32 = 0.25;
pub const MIN_HEIGHT: f32 = 0.25;
/// Two hits nearer to each other than this along the ray are a tie (yards).
pub const TIE: f32 = 0.05;

impl Body {
    /// A body with the least sizes applied.
    pub fn new(id: u64, at: V3, yaw: f32, half: [f32; 2], bottom: f32, height: f32) -> Body {
        let ok = |v: f32, least: f32| if v.is_finite() { v.max(least) } else { least };
        Body { id, at, yaw: if yaw.is_finite() { yaw } else { 0.0 }, half: [ok(half[0], MIN_HALF), ok(half[1], MIN_HALF)], bottom: if bottom.is_finite() { bottom } else { 0.0 }, height: ok(height, MIN_HEIGHT) }
    }

    fn volume(&self) -> f32 {
        self.half[0] * self.half[1] * 4.0 * self.height
    }
}

fn add(a: V3, b: V3, k: f32) -> V3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

/// Where the ray (start `eye`, unit direction `dir`) is inside the body's box: (enter, exit) distances along it.
/// `enter` is below 0 when the eye is inside. None: it misses, or the box is wholly behind the eye.
pub fn hit(eye: V3, dir: V3, b: &Body) -> Option<(f32, f32)> {
    if !eye.iter().chain(dir.iter()).all(|v| v.is_finite()) {
        return None;
    }
    // into the box's own frame: its foot the origin, its facing +x
    let (s, c) = b.yaw.sin_cos();
    let p = [eye[0] - b.at[0], eye[1] - b.at[1], eye[2] - b.at[2]];
    let o = [p[0] * c + p[1] * s, -p[0] * s + p[1] * c, p[2]];
    let d = [dir[0] * c + dir[1] * s, -dir[0] * s + dir[1] * c, dir[2]];
    let lo = [-b.half[0], -b.half[1], b.bottom];
    let hi = [b.half[0], b.half[1], b.bottom + b.height];
    let (mut t0, mut t1) = (f32::NEG_INFINITY, f32::INFINITY);
    for k in 0..3 {
        if d[k].abs() < 1e-9 {
            // level with this pair of faces: inside them or never
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, z) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        let (a, z) = if a < z { (a, z) } else { (z, a) };
        t0 = t0.max(a);
        t1 = t1.min(z);
        if t0 > t1 {
            return None;
        }
    }
    (t1 >= 0.0).then_some((t0, t1))
}

/// The prop under the cursor: the nearest box the ray enters within `reach` yards. Ties (two boxes entered at the
/// same distance, within [`TIE`]): the smaller box (a cup on a shelf, not the shelf), then the newer prop (the
/// higher id: the copy you just made). A box the eye is inside of (a house round the camera) only counts when
/// nothing else is hit, the smallest such. Returns the prop's id and the distance to the hit.
pub fn pick(eye: V3, dir: V3, bodies: &[Body], reach: f32) -> Option<(u64, f32)> {
    let mut best: Option<(&Body, f32)> = None;
    let mut inside: Option<&Body> = None;
    for b in bodies {
        let Some((t0, _)) = hit(eye, dir, b) else { continue };
        if t0 < 0.0 {
            if inside.map_or(true, |cur| b.volume() < cur.volume()) {
                inside = Some(b);
            }
            continue;
        }
        if t0 > reach {
            continue;
        }
        best = match best {
            None => Some((b, t0)),
            Some((cur, ct)) => {
                let better = if (t0 - ct).abs() <= TIE {
                    let (vb, vc) = (b.volume(), cur.volume());
                    if (vb - vc).abs() > 1e-4 { vb < vc } else { b.id > cur.id }
                } else {
                    t0 < ct
                };
                if better { Some((b, t0)) } else { Some((cur, ct)) }
            }
        };
    }
    best.map(|(b, t)| (b.id, t)).or_else(|| inside.map(|b| (b.id, 0.0)))
}

/// Where the ray meets the level plane at height `z`: (x, y). None when it runs level, goes away from the plane,
/// or meets it further than `reach` yards along the ray.
pub fn on_plane(eye: V3, dir: V3, z: f32, reach: f32) -> Option<[f32; 2]> {
    if !z.is_finite() || dir[2].abs() < 1e-4 {
        return None;
    }
    let t = (z - eye[2]) / dir[2];
    if !(t > 0.0 && t <= reach) {
        return None;
    }
    let p = add(eye, dir, t);
    p.iter().all(|v| v.is_finite()).then_some([p[0], p[1]])
}

/// A grab: where the cursor's ray first touched the prop (`at`, the hit point), and the foot's offset from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grip {
    /// The height of the plane the drag follows: where the prop was grabbed.
    pub z: f32,
    /// The foot minus the grabbed point (x, y).
    pub off: [f32; 2],
}

/// The grip for a prop whose foot is `foot`, grabbed where the ray hit it `t` yards along.
pub fn grip(eye: V3, dir: V3, t: f32, foot: V3) -> Option<Grip> {
    let p = add(eye, dir, t.max(0.0));
    p.iter().all(|v| v.is_finite()).then_some(Grip { z: p[2], off: [foot[0] - p[0], foot[1] - p[1]] })
}

/// Where the foot goes for the cursor's ray now: the ray on the grip's plane, plus the offset.
pub fn follow(eye: V3, dir: V3, g: &Grip, reach: f32) -> Option<[f32; 2]> {
    on_plane(eye, dir, g.z, reach).map(|p| [p[0] + g.off[0], p[1] + g.off[1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(v: V3) -> V3 {
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / n, v[1] / n, v[2] / n]
    }

    fn cube(id: u64, x: f32, y: f32, half: f32, height: f32) -> Body {
        Body::new(id, [x, y, 0.0], 0.0, [half, half], 0.0, height)
    }

    #[test]
    fn a_ray_straight_down_hits_the_box_under_it_at_its_top() {
        let b = cube(1, 10.0, 5.0, 1.0, 2.0);
        let (t0, t1) = hit([10.0, 5.0, 20.0], [0.0, 0.0, -1.0], &b).unwrap();
        assert!((t0 - 18.0).abs() < 1e-4 && (t1 - 20.0).abs() < 1e-4, "{t0} {t1}");
        assert!(hit([12.5, 5.0, 20.0], [0.0, 0.0, -1.0], &b).is_none(), "beside it");
        assert_eq!(pick([10.0, 5.0, 20.0], [0.0, 0.0, -1.0], &[b], 100.0), Some((1, 18.0)));
    }

    #[test]
    fn the_nearest_of_two_wins_from_either_side() {
        let a = cube(1, 5.0, 0.0, 0.5, 1.0);
        let b = cube(2, 10.0, 0.0, 0.5, 1.0);
        let along = unit([1.0, 0.0, 0.0]);
        assert_eq!(pick([0.0, 0.0, 0.5], along, &[b, a], 100.0).map(|p| p.0), Some(1));
        let back = unit([-1.0, 0.0, 0.0]);
        assert_eq!(pick([20.0, 0.0, 0.5], back, &[a, b], 100.0).map(|p| p.0), Some(2));
    }

    #[test]
    fn behind_the_camera_and_too_far_are_not_picked() {
        let b = cube(1, -5.0, 0.0, 1.0, 2.0);
        assert_eq!(pick([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], &[b], 100.0), None, "behind");
        let far = cube(2, 300.0, 0.0, 1.0, 2.0);
        assert_eq!(pick([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], &[far], 100.0), None, "past reach");
        assert_eq!(pick([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], &[far], 400.0).map(|p| p.0), Some(2), "within a longer reach");
    }

    #[test]
    fn ties_go_to_the_smaller_box_then_the_newer_prop() {
        // a rug (wide, flat) and a crate standing on the same spot: from straight above the crate's top is higher,
        // so it wins anyway; from the side at floor height both are entered at the same face distance
        let rug = Body::new(1, [0.0, 0.0, 0.0], 0.0, [2.0, 2.0], 0.0, 0.25);
        let crate_ = Body::new(2, [0.0, 0.0, 0.0], 0.0, [2.0, 2.0], 0.0, 1.0);
        let dir = unit([1.0, 0.0, 0.0]);
        assert_eq!(pick([-10.0, 0.0, 0.1], dir, &[crate_, rug], 100.0).map(|p| p.0), Some(1), "the smaller box");
        // two copies in the same place: the newer
        let a = cube(7, 0.0, 0.0, 1.0, 1.0);
        let b = cube(9, 0.0, 0.0, 1.0, 1.0);
        assert_eq!(pick([0.0, 0.0, 10.0], [0.0, 0.0, -1.0], &[b, a], 100.0).map(|p| p.0), Some(9));
        assert_eq!(pick([0.0, 0.0, 10.0], [0.0, 0.0, -1.0], &[a, b], 100.0).map(|p| p.0), Some(9));
    }

    #[test]
    fn a_box_round_the_camera_counts_only_when_nothing_else_is_hit() {
        let house = Body::new(1, [0.0, 0.0, 0.0], 0.0, [10.0, 10.0], 0.0, 8.0);
        let chair = cube(2, 4.0, 0.0, 0.5, 1.0);
        let eye = [0.0, 0.0, 3.0];
        let down_to_chair = unit([4.0, 0.0, -2.5]);
        assert_eq!(pick(eye, down_to_chair, &[house, chair], 100.0).map(|p| p.0), Some(2));
        assert_eq!(pick(eye, unit([-1.0, 0.0, -0.2]), &[house, chair], 100.0), Some((1, 0.0)), "only the house: it");
    }

    #[test]
    fn a_turned_box_is_hit_as_turned() {
        // 4 long, 0.5 deep, turned a quarter: it runs along y
        let b = Body::new(1, [0.0, 0.0, 0.0], std::f32::consts::FRAC_PI_2, [2.0, 0.25], 0.0, 1.0);
        assert!(hit([0.0, 1.5, 5.0], [0.0, 0.0, -1.0], &b).is_some(), "along its length (y now)");
        assert!(hit([1.5, 0.0, 5.0], [0.0, 0.0, -1.0], &b).is_none(), "not across it");
        // its bottom raised (a lamp's shade hanging over its foot)
        let shade = Body::new(2, [0.0, 0.0, 0.0], 0.0, [0.5, 0.5], 2.0, 0.5);
        assert!(hit([-5.0, 0.0, 1.0], [1.0, 0.0, 0.0], &shade).is_none(), "under it");
        assert!(hit([-5.0, 0.0, 2.2], [1.0, 0.0, 0.0], &shade).is_some());
    }

    #[test]
    fn the_least_sizes_and_nonsense() {
        let b = Body::new(1, [0.0; 3], f32::NAN, [0.0, f32::NAN], f32::INFINITY, -3.0);
        assert_eq!((b.yaw, b.half, b.bottom, b.height), (0.0, [MIN_HALF, MIN_HALF], 0.0, MIN_HEIGHT));
        assert!(hit([f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], &b).is_none());
        assert_eq!(pick([0.0, 0.0, 5.0], [0.0, 0.0, -1.0], &[], 100.0), None);
    }

    #[test]
    fn the_ray_on_a_level_plane() {
        let p = on_plane([0.0, 0.0, 10.0], unit([1.0, 0.0, -1.0]), 0.0, 100.0).unwrap();
        assert!((p[0] - 10.0).abs() < 1e-4 && p[1].abs() < 1e-4);
        assert!(on_plane([0.0, 0.0, 10.0], [1.0, 0.0, 0.0], 0.0, 100.0).is_none(), "level: never");
        assert!(on_plane([0.0, 0.0, 10.0], unit([1.0, 0.0, 1.0]), 0.0, 100.0).is_none(), "away from it");
        assert!(on_plane([0.0, 0.0, 10.0], unit([1.0, 0.0, -0.01]), 0.0, 100.0).is_none(), "too far along");
    }

    #[test]
    fn a_drag_keeps_the_spot_you_grabbed_under_the_cursor() {
        // a crate 2 tall at (10, 0); grabbed on its top near its corner, seen from up and back
        let eye = [0.0, 0.0, 12.0];
        let foot = [10.0, 0.0, 0.0];
        let b = Body::new(1, foot, 0.0, [1.0, 1.0], 0.0, 2.0);
        let d0 = unit([10.5 - eye[0], 0.4 - eye[1], 2.0 - eye[2]]);
        let (_, t) = pick(eye, d0, &[b], 100.0).unwrap();
        let g = grip(eye, d0, t, foot).unwrap();
        assert!((g.z - 2.0).abs() < 1e-3, "the plane at the grabbed height: {}", g.z);
        assert!((g.off[0] + 0.5).abs() < 1e-3 && (g.off[1] + 0.4).abs() < 1e-3, "{:?}", g.off);
        // the cursor not moved: the foot stays
        let f = follow(eye, d0, &g, 100.0).unwrap();
        assert!((f[0] - 10.0).abs() < 1e-3 && f[1].abs() < 1e-3, "{f:?}");
        // the cursor moved to where a point 3 yards on (and 1 to the side) is at that height: the foot moves the same
        let d1 = unit([13.5 - eye[0], 1.4 - eye[1], 2.0 - eye[2]]);
        let f = follow(eye, d1, &g, 100.0).unwrap();
        assert!((f[0] - 13.0).abs() < 1e-3 && (f[1] - 1.0).abs() < 1e-3, "{f:?}");
        // the cursor up past the horizon: no answer (the prop stays where it last was)
        assert!(follow(eye, unit([1.0, 0.0, 0.2]), &g, 100.0).is_none());
    }
}
