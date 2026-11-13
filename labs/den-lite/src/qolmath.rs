//! The geometry of the editor's quality-of-life tools (docs/DEN-NEXT.md, "The builder's comforts", 12 Nov 2026).
//! Std only, so it is tested with plain `rustc --edition 2021 --test src/qolmath.rs` (no crates.io). The Den side of
//! each tool is `qol.rs` and small hooks in gizmo.rs.
//!
//! * [`group_place`]: a group carried by one of its props (the owner's LANDSCAPE editor, build-multi.js: "moving
//!   several selected pieces together"): every other prop keeps its offset from the one in hand, turned with it.
//! * [`magnet`]: a prop in hand near another of its kind snaps edge to edge with it (walls, fences, tables in a row),
//!   turned as it is.
//! * [`fill_gap`], [`fill_box`]: copies of a prop between two (a row) or over the box two make (a floor, a field).
//! * [`nudge_offset`]: the keyboard's nudge, relative to the view but along the grid's axes.
//! * [`in_circle`]: the circle select.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// How near (yards, on the ground) the cursor's spot must come to a snap spot for the magnet to take it.
pub const MAGNET_REACH: f32 = 0.9;
/// How far (yards, middle to middle) a neighbour is looked for: past this nothing snaps.
pub const MAGNET_LOOK: f32 = 12.0;
/// The most copies a fill makes in one go (the Den's 800 still apply).
pub const FILL_MAX: usize = 160;
/// The smallest step a fill takes (yards): a prop with no size never fills a floor with thousands.
pub const FILL_MIN_STEP: f32 = 0.25;

/// Turn (x, y) by `a` radians about the origin.
pub fn rotate(p: [f32; 2], a: f32) -> [f32; 2] {
    let (s, c) = a.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

/// The shortest turn from `a` to `b` (radians, -PI..PI).
pub fn turn_between(a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(TAU);
    if d > PI {
        d - TAU
    } else {
        d
    }
}

/// A prop of a carried group: where it goes (x, y) and its new facing, when the one in hand went from `from` to `to`
/// (x, y, z, facing in radians). Its offset from the one in hand turns with it, as one piece.
pub fn group_place(from: [f32; 4], to: [f32; 4], p: [f32; 2], facing: f32) -> ([f32; 2], f32) {
    let d = turn_between(from[3], to[3]);
    let off = rotate([p[0] - from[0], p[1] - from[1]], d);
    ([to[0] + off[0], to[1] + off[1]], (facing + d).rem_euclid(TAU))
}

/// A neighbour for the magnet: its middle on the ground, facing (radians) and half sizes along its own forward (x)
/// and left (y).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Near {
    pub at: [f32; 2],
    pub facing: f32,
    pub half: [f32; 2],
}

/// The magnet: the spot (x, y) and facing that put a prop of half size `half` (its own forward, left) edge to edge
/// with the nearest neighbour, when the cursor's spot `want` is within [`MAGNET_REACH`] of one of the four spots
/// round it (in front, behind, left, right of it, turned as it is). None: no neighbour near enough (the cursor's own
/// spot stands).
pub fn magnet(want: [f32; 2], half: [f32; 2], near: &[Near]) -> Option<([f32; 2], f32)> {
    let mut best: Option<(f32, [f32; 2], f32)> = None;
    for n in near {
        let dx = want[0] - n.at[0];
        let dy = want[1] - n.at[1];
        if dx * dx + dy * dy > MAGNET_LOOK * MAGNET_LOOK || !n.facing.is_finite() {
            continue;
        }
        let (s, c) = n.facing.sin_cos();
        let fwd = [c, s];
        let left = [-s, c];
        let gx = n.half[0] + half[0];
        let gy = n.half[1] + half[1];
        for (dir, gap) in [(fwd, gx), ([-fwd[0], -fwd[1]], gx), (left, gy), ([-left[0], -left[1]], gy)] {
            let spot = [n.at[0] + dir[0] * gap, n.at[1] + dir[1] * gap];
            let e = ((want[0] - spot[0]).powi(2) + (want[1] - spot[1]).powi(2)).sqrt();
            if e <= MAGNET_REACH && best.map_or(true, |b| e < b.0) {
                best = Some((e, spot, n.facing));
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

/// A prop's half width along a direction on the ground (`dir` radians), from its half sizes along its own forward
/// and left and its facing: the half the row's step needs.
pub fn half_along(half: [f32; 2], facing: f32, dir: f32) -> f32 {
    let a = dir - facing;
    (half[0] * a.cos()).abs() + (half[1] * a.sin()).abs()
}

/// Fill the gap between two props (their middles `a` and `b`, x, y) with copies `width` yards wide along the line:
/// as many as fit, spread evenly between them (neither end is a new copy). At most [`FILL_MAX`].
pub fn fill_gap(a: [f32; 2], b: [f32; 2], width: f32) -> Vec<[f32; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt();
    let w = if width.is_finite() { width.max(FILL_MIN_STEP) } else { 1.0 };
    if !len.is_finite() || len < w * 1.5 {
        return Vec::new();
    }
    // as many steps as the length holds at about the prop's width, so the copies meet without overlapping much
    let steps = ((len / w).round() as usize).clamp(1, FILL_MAX + 1);
    (1..steps).map(|i| {
        let k = i as f32 / steps as f32;
        [a[0] + dx * k, a[1] + dy * k]
    }).take(FILL_MAX).collect()
}

/// Fill the box two props make (opposite corners `a` and `b`) on the world's axes, every `step` yards (x, y), from
/// `a`'s corner: every spot but the two corners' own. At most [`FILL_MAX`] (None: it would make more).
pub fn fill_box(a: [f32; 2], b: [f32; 2], step: [f32; 2]) -> Option<Vec<[f32; 2]>> {
    let sx = if step[0].is_finite() { step[0].max(FILL_MIN_STEP) } else { 1.0 };
    let sy = if step[1].is_finite() { step[1].max(FILL_MIN_STEP) } else { 1.0 };
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    if !dx.is_finite() || !dy.is_finite() {
        return Some(Vec::new());
    }
    let nx = (dx.abs() / sx + 0.25).floor() as usize;
    let ny = (dy.abs() / sy + 0.25).floor() as usize;
    if (nx + 1) * (ny + 1) > FILL_MAX + 2 {
        return None;
    }
    let (ux, uy) = (sx * dx.signum(), sy * dy.signum());
    let mut out = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            let p = [a[0] + ux * i as f32, a[1] + uy * j as f32];
            let at = |q: [f32; 2]| (p[0] - q[0]).abs() < 0.1 && (p[1] - q[1]).abs() < 0.1;
            if !at(a) && !at(b) {
                out.push(p);
            }
        }
    }
    Some(out)
}

/// The keyboard's nudge (Numpad 8 / 2 / 4 / 6, Ctrl+arrows): `fwd` yards the way the view faces and `right` to its
/// right, but along the grid's axes (the view's facing rounded to the nearest quarter turn, so a nudge never leaves the
/// grid), and `up` yards up. (dx, dy, dz) in the world.
pub fn nudge_offset(view: f32, fwd: f32, right: f32, up: f32) -> [f32; 3] {
    let q = if view.is_finite() { (view / FRAC_PI_2).round() * FRAC_PI_2 } else { 0.0 };
    let (s, c) = q.sin_cos();
    let tidy = |v: f32| if v.abs() < 1e-5 { 0.0 } else { v };
    // forward (c, s); right a quarter turn clockwise (s, -c)
    [tidy(fwd * c + right * s), tidy(fwd * s - right * c), up]
}

/// Whether (x, y) is inside the circle round `c` of radius `r` (the circle select, on the ground).
pub fn in_circle(c: [f32; 2], r: f32, p: [f32; 2]) -> bool {
    r.is_finite() && (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) <= r * r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
    }

    #[test]
    fn a_carried_group_keeps_its_shape_and_turns_as_one() {
        // a chair 2 yards east of the table in hand; the table moves 5 north: the chair too
        let from = [10.0, 10.0, 0.0, 0.0];
        let (p, f) = group_place(from, [10.0, 15.0, 0.0, 0.0], [12.0, 10.0], 1.0);
        assert!(close(p, [12.0, 15.0]) && (f - 1.0).abs() < 1e-5);
        // the table turned a quarter left where it stood: the chair goes round it to the north, turned too
        let (p, f) = group_place(from, [10.0, 10.0, 0.0, FRAC_PI_2], [12.0, 10.0], 0.0);
        assert!(close(p, [10.0, 12.0]), "{p:?}");
        assert!((f - FRAC_PI_2).abs() < 1e-5);
        // across the turn's wrap: 350 to 10 degrees is 20 degrees, not -340
        let (p, _) = group_place([0.0, 0.0, 0.0, 350f32.to_radians()], [0.0, 0.0, 0.0, 10f32.to_radians()], [1.0, 0.0], 0.0);
        let want = rotate([1.0, 0.0], 20f32.to_radians());
        assert!(close(p, want), "{p:?} {want:?}");
        assert!((turn_between(3.0, -3.0) - (TAU - 6.0)).abs() < 1e-5);
    }

    #[test]
    fn the_magnet_puts_a_prop_edge_to_edge_with_its_neighbour() {
        // a wall 4 long (half 2) and 0.5 deep (half 0.25), facing east at the origin
        let wall = Near { at: [0.0, 0.0], facing: 0.0, half: [2.0, 0.25] };
        // the cursor near the spot in front of it (4 yards east): snapped there, facing as the wall
        let (p, f) = magnet([4.4, 0.3], [2.0, 0.25], &[wall]).unwrap();
        assert!(close(p, [4.0, 0.0]) && f == 0.0, "{p:?}");
        // beside it (the left, north): half depths apart
        let (p, _) = magnet([0.2, 0.6], [2.0, 0.25], &[wall]).unwrap();
        assert!(close(p, [0.0, 0.5]), "{p:?}");
        // a turned wall (facing north): in front is north
        let turned = Near { facing: FRAC_PI_2, ..wall };
        let (p, f) = magnet([0.1, -3.7], [2.0, 0.25], &[turned]).unwrap();
        assert!(close(p, [0.0, -4.0]) && (f - FRAC_PI_2).abs() < 1e-6, "{p:?}");
        // too far from every snap spot: none
        assert!(magnet([2.0, 2.0], [2.0, 0.25], &[wall]).is_none());
        assert!(magnet([50.0, 0.0], [2.0, 0.25], &[wall]).is_none());
        // two neighbours: the nearest spot wins
        let other = Near { at: [8.5, 0.0], ..wall };
        let (p, _) = magnet([4.3, 0.0], [2.0, 0.25], &[wall, other]).unwrap();
        assert!(close(p, [4.5, 0.0]), "{p:?}: 4.5 is the second wall's back spot, 0.2 away; the first's front is 0.3");
        assert!(magnet([1.0, 1.0], [1.0, 1.0], &[]).is_none());
    }

    #[test]
    fn half_along_turns_the_footprint() {
        assert!((half_along([2.0, 0.5], 0.0, 0.0) - 2.0).abs() < 1e-5);
        assert!((half_along([2.0, 0.5], 0.0, FRAC_PI_2) - 0.5).abs() < 1e-5);
        assert!((half_along([2.0, 0.5], FRAC_PI_2, FRAC_PI_2) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn fill_gap_spreads_copies_evenly_between_the_two() {
        // 10 yards apart, 2 wide: 4 copies between, 2 yards apart
        let v = fill_gap([0.0, 0.0], [10.0, 0.0], 2.0);
        assert_eq!(v.len(), 4);
        for (i, p) in v.iter().enumerate() {
            assert!(close(*p, [2.0 * (i + 1) as f32, 0.0]), "{v:?}");
        }
        // a diagonal: on the line
        let v = fill_gap([0.0, 0.0], [3.0, 4.0], 1.0);
        assert_eq!(v.len(), 4);
        assert!(v.iter().all(|p| (p[0] * 4.0 - p[1] * 3.0).abs() < 1e-3));
        // too close for one more: none; a wild width; capped
        assert!(fill_gap([0.0, 0.0], [2.5, 0.0], 2.0).is_empty());
        assert!(fill_gap([0.0, 0.0], [f32::NAN, 0.0], 2.0).is_empty());
        assert_eq!(fill_gap([0.0, 0.0], [1000.0, 0.0], 0.0).len(), FILL_MAX);
    }

    #[test]
    fn fill_box_covers_the_box_but_the_corners() {
        // 4 x 2 at 2 yards: 3 x 2 spots, less the two corners
        let v = fill_box([0.0, 0.0], [4.0, 2.0], [2.0, 2.0]).unwrap();
        assert_eq!(v.len(), 4, "{v:?}");
        assert!(v.contains(&[2.0, 0.0]) && v.contains(&[0.0, 2.0]) && v.contains(&[4.0, 0.0]) && v.contains(&[2.0, 2.0]));
        // from the other corner the other way round: the same spots
        let mut w = fill_box([4.0, 2.0], [0.0, 0.0], [2.0, 2.0]).unwrap();
        w.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut v2 = v.clone();
        v2.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(w, v2);
        // a step that doesn't divide it: from the first corner
        let v = fill_box([0.0, 0.0], [5.0, 0.0], [2.0, 2.0]).unwrap();
        assert_eq!(v, vec![[2.0, 0.0], [4.0, 0.0]]);
        // too many: refused (None), never a part of it
        assert!(fill_box([0.0, 0.0], [100.0, 100.0], [1.0, 1.0]).is_none());
        assert!(fill_box([0.0, 0.0], [100.0, 100.0], [0.0, 0.0]).is_none(), "the smallest step still counts");
    }

    #[test]
    fn a_nudge_follows_the_view_on_the_grid_axes() {
        // facing east: forward is +x, right is -y
        assert_eq!(nudge_offset(0.0, 1.0, 0.0, 0.0), [1.0, 0.0, 0.0]);
        assert_eq!(nudge_offset(0.0, 0.0, 1.0, 0.0), [0.0, -1.0, 0.0]);
        // facing 80 degrees (nearly north): forward is +y, right is +x, exactly on the grid
        let v = nudge_offset(80f32.to_radians(), 0.5, 0.0, 0.0);
        assert!((v[0]).abs() < 1e-5 && (v[1] - 0.5).abs() < 1e-5, "{v:?}");
        let v = nudge_offset(80f32.to_radians(), 0.0, 2.0, -0.25);
        assert!((v[0] - 2.0).abs() < 1e-5 && v[1].abs() < 1e-5 && v[2] == -0.25, "{v:?}");
        // facing south-west-ish (200 degrees): forward is -x
        let v = nudge_offset(200f32.to_radians(), 1.0, 0.0, 0.0);
        assert!((v[0] + 1.0).abs() < 1e-5 && v[1].abs() < 1e-5, "{v:?}");
        assert_eq!(nudge_offset(f32::NAN, 1.0, 0.0, 0.0), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn the_circle_takes_what_is_inside() {
        assert!(in_circle([0.0, 0.0], 5.0, [3.0, 4.0]));
        assert!(!in_circle([0.0, 0.0], 4.9, [3.0, 4.0]));
        assert!(!in_circle([0.0, 0.0], f32::NAN, [0.0, 0.0]));
    }
}
