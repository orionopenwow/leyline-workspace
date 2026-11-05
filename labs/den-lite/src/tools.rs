//! The Den builder's tools, the geometry (grand release brief, 4 Nov: "make the Den builder's tools complete and top
//! tier": paths, stamps you turn, mirror and size before they go down, arrays, align and distribute, box select,
//! snaps, measure, and props you can stand on). buildtools.rs does the Den's side of each with these.
//!
//! - **Paths** ([`smooth`], [`lay`]): the points you clicked on the ground, smoothed into a curve that passes through
//!   every one of them (Catmull-Rom, centripetal: no loops or overshoot at a sharp corner), then a prop every `every`
//!   yards along it by its length (not by the points), each turned along the path or across it, in the middle of the
//!   path or on one or both sides. Fences go on the middles of the steps, so a fence that is `every` long joins the
//!   next one end to end.
//! - **Arrays** ([`line_array`], [`ring_array`]): n copies along a step, or round a circle (facing in, out or along).
//! - **Align and distribute** ([`align_to`], [`distribute`]).
//! - **Box select** ([`in_rect`]): a prop's middle on the screen inside the box you drew.
//! - **Stamps** ([`transform`], [`centre_of`], [`plan`]): a stamp turned, mirrored and sized before it goes down;
//!   a selection made into a stamp around its middle; the small top-down picture the gallery shows of a stamp.
//! - **Collision** ([`client_collision`], [`pass`], [`top_hold`]): what the game does and what Den Lite can do about
//!   it (docs/DEN-TOOLS.md has the whole story).
//!
//! Std only (no crate types): it builds and tests on its own with `rustc --edition 2021 --test src/tools.rs`.

use std::f32::consts::PI;

pub type P2 = [f32; 2];
pub type P3 = [f32; 3];

// ------------------------------------------------------------------------------------------------------------------
// paths
// ------------------------------------------------------------------------------------------------------------------

/// The most props one path lays (a fence round the whole Den at 2 yd is about 120).
pub const PATH_MAX: usize = 160;
/// The most points a path has.
pub const POINTS_MAX: usize = 64;
/// The least spacing (yards): closer than this is one prop inside the next.
pub const EVERY_MIN: f32 = 0.25;
/// Steps of the smoothed curve between two clicked points (fine enough that its length is right to a few cm).
pub const SMOOTH_STEPS: usize = 16;

/// The curve through the points: Catmull-Rom, centripetal (alpha 0.5), `per` steps between each two points, the
/// first and last points kept. Two points or fewer are a straight line as given. Repeated points are left out (a
/// double click must not make a knot).
pub fn smooth(points: &[P2], per: usize) -> Vec<P2> {
    let mut p: Vec<P2> = Vec::with_capacity(points.len());
    for q in points.iter().filter(|q| q[0].is_finite() && q[1].is_finite()) {
        if p.last().is_none_or(|l| dist2(*l, *q) > 1e-4) {
            p.push(*q);
        }
    }
    if p.len() < 3 || per < 2 {
        return p;
    }
    let n = p.len();
    // the ends: a point mirrored past each end, so the curve leaves the first point toward the second
    let ext = |i: isize| -> P2 {
        if i < 0 {
            [2.0 * p[0][0] - p[1][0], 2.0 * p[0][1] - p[1][1]]
        } else if i as usize >= n {
            [2.0 * p[n - 1][0] - p[n - 2][0], 2.0 * p[n - 1][1] - p[n - 2][1]]
        } else {
            p[i as usize]
        }
    };
    let mut out = Vec::with_capacity((n - 1) * per + 1);
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (ext(i as isize - 1), p[i], p[i + 1], ext(i as isize + 2));
        // centripetal knots: t_{k+1} = t_k + |p_{k+1} - p_k|^0.5
        let knot = |a: P2, b: P2| dist2(a, b).sqrt().sqrt().max(1e-4);
        let t0 = 0.0;
        let t1 = t0 + knot(p0, p1);
        let t2 = t1 + knot(p1, p2);
        let t3 = t2 + knot(p2, p3);
        for s in 0..per {
            let t = t1 + (t2 - t1) * s as f32 / per as f32;
            let lerp = |a: P2, b: P2, ta: f32, tb: f32| -> P2 {
                let k = (t - ta) / (tb - ta);
                [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k]
            };
            let a1 = lerp(p0, p1, t0, t1);
            let a2 = lerp(p1, p2, t1, t2);
            let a3 = lerp(p2, p3, t2, t3);
            let b1 = lerp(a1, a2, t0, t2);
            let b2 = lerp(a2, a3, t1, t3);
            out.push(lerp(b1, b2, t1, t2));
        }
    }
    out.push(p[n - 1]);
    out
}

/// Which way each prop on a path faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// Along the path (a fence, a road tile, a bench seen side on).
    Along,
    /// Across it (a lamp's front to the path, a bench facing the walk).
    Across,
}

/// How a path is laid: a prop every `every` yards; on the middles of the steps (`mid`: fences) or on the marks;
/// smoothed or straight between the points; on the path's line, or `offset` yards to its left, right or both sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lay {
    pub every: f32,
    pub smooth: bool,
    pub face: Face,
    pub mid: bool,
    /// 0: on the line; 1: left; 2: right; 3: both sides.
    pub sides: u8,
    pub offset: f32,
}

impl Default for Lay {
    fn default() -> Self {
        Lay { every: 3.0, smooth: true, face: Face::Along, mid: false, sides: 0, offset: 0.0 }
    }
}

/// The length of a line through the points.
pub fn length(line: &[P2]) -> f32 {
    line.windows(2).map(|w| dist2(w[0], w[1]).sqrt()).sum()
}

/// Where the props of a path go: (x, y) and the turn (radians, the game's: 0 along +x, counter-clockwise). At most
/// [`PATH_MAX`]. The first prop is at the path's start (on the marks) or half a step in (`mid`); the last mark is
/// at the path's end when what is left after the last whole step is more than a quarter step (a fence never stops
/// short of where you clicked by much, and a lamp row ends with a lamp at the end).
pub fn lay(points: &[P2], l: &Lay) -> Vec<(P2, f32)> {
    let line = if l.smooth { smooth(points, SMOOTH_STEPS) } else { smooth(points, 0) };
    let every = if l.every.is_finite() { l.every.max(EVERY_MIN) } else { 3.0 };
    let total = length(&line);
    let mut marks: Vec<f32> = Vec::new();
    if line.len() == 1 {
        marks.push(0.0);
    } else if total > 0.0 {
        let whole = (total / every).floor() as usize;
        if l.mid {
            // one fence per whole step, then one for what is left when it is more than a quarter step
            let n = whole + usize::from(total - whole as f32 * every > every / 4.0);
            let n = n.max(1);
            let step = total / n as f32;
            marks.extend((0..n).map(|i| (i as f32 + 0.5) * step));
        } else {
            marks.extend((0..=whole).map(|i| i as f32 * every));
            if total - whole as f32 * every > every / 4.0 {
                marks.push(total);
            }
        }
    }
    let mut out = Vec::new();
    for m in marks {
        let (at, dir) = point_at(&line, m);
        let yaw = dir[1].atan2(dir[0]);
        let yaw = match l.face {
            Face::Along => yaw,
            Face::Across => yaw + PI / 2.0,
        };
        let left = [-dir[1], dir[0]];
        let off = if l.offset.is_finite() { l.offset.clamp(0.0, 20.0) } else { 0.0 };
        let sides: &[f32] = match l.sides {
            1 => &[1.0],
            2 => &[-1.0],
            3 => &[1.0, -1.0],
            _ => &[0.0],
        };
        for &s in sides {
            if out.len() >= PATH_MAX {
                return out;
            }
            // a prop on the right of the path faces the other way across it (both rows face the walk)
            let y = if s < 0.0 && l.face == Face::Across { yaw + PI } else { yaw };
            out.push(([at[0] + left[0] * off * s, at[1] + left[1] * off * s], wrap(y)));
        }
    }
    out
}

/// The point `d` yards along the line and the line's direction there (a unit vector; +x for a single point).
pub fn point_at(line: &[P2], d: f32) -> (P2, P2) {
    if line.len() < 2 {
        return (line.first().copied().unwrap_or([0.0, 0.0]), [1.0, 0.0]);
    }
    let mut left = d.max(0.0);
    for w in line.windows(2) {
        let len = dist2(w[0], w[1]).sqrt();
        if len < 1e-6 {
            continue;
        }
        let dir = [(w[1][0] - w[0][0]) / len, (w[1][1] - w[0][1]) / len];
        if left <= len {
            return ([w[0][0] + dir[0] * left, w[0][1] + dir[1] * left], dir);
        }
        left -= len;
    }
    // past the end: the last point, the last step's direction
    let n = line.len();
    let (a, b) = (line[n - 2], line[n - 1]);
    let len = dist2(a, b).sqrt().max(1e-6);
    (b, [(b[0] - a[0]) / len, (b[1] - a[1]) / len])
}

/// `x,y;x,y;...` (the addon's points) read: at most [`POINTS_MAX`], numbers only.
pub fn parse_points(s: &str) -> Vec<P2> {
    s.split(';')
        .filter_map(|p| {
            let (x, y) = p.trim().split_once(',')?;
            let (x, y) = (x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?);
            (x.is_finite() && y.is_finite() && x.abs() < 1.0e5 && y.abs() < 1.0e5).then_some([x, y])
        })
        .take(POINTS_MAX)
        .collect()
}

/// The points written back (`x,y;...`, two decimals: what the addon edits).
pub fn write_points(p: &[P2]) -> String {
    p.iter().map(|q| format!("{:.2},{:.2}", q[0], q[1])).collect::<Vec<_>>().join(";")
}

// ------------------------------------------------------------------------------------------------------------------
// arrays
// ------------------------------------------------------------------------------------------------------------------

/// The most copies one array makes.
pub const ARRAY_MAX: usize = 60;

/// n copies along a step: the offsets from the original, 1 step, 2 steps ... n steps.
pub fn line_array(n: usize, step: P3) -> Vec<P3> {
    (1..=n.min(ARRAY_MAX)).map(|i| [step[0] * i as f32, step[1] * i as f32, step[2] * i as f32]).collect()
}

/// Which way the copies round a ring face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingFace {
    In,
    Out,
    Along,
    /// As the original faces.
    Keep,
}

/// n copies round a circle of `radius` about a middle: each one's offset from the middle and its turn (radians;
/// `keep`: the original's). The first is at `start` (radians from +x).
pub fn ring_array(n: usize, radius: f32, start: f32, face: RingFace, keep: f32) -> Vec<(P2, f32)> {
    let n = n.clamp(1, ARRAY_MAX);
    (0..n)
        .map(|i| {
            let a = start + 2.0 * PI * i as f32 / n as f32;
            let off = [radius * a.cos(), radius * a.sin()];
            let yaw = match face {
                RingFace::Out => a,
                RingFace::In => a + PI,
                RingFace::Along => a + PI / 2.0,
                RingFace::Keep => keep,
            };
            (off, wrap(yaw))
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------------------------
// align and distribute
// ------------------------------------------------------------------------------------------------------------------

/// Where to line up to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum To {
    /// The least (left, front, the lowest bottom).
    Min,
    Max,
    /// The middle between the least and the most.
    Mid,
    /// The first selected (the one the rest were added to).
    First,
}

/// The value every one lines up to.
pub fn align_to(vals: &[f32], to: To) -> Option<f32> {
    let v: Vec<f32> = vals.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
    Some(match to {
        To::Min => lo,
        To::Max => hi,
        To::Mid => (lo + hi) / 2.0,
        To::First => v[0],
    })
}

/// The same values spread evenly between the least and the most, each keeping its place in the order (ties by
/// where they were in the list): the new value for each, in the list's order. Fewer than 3: as they are.
pub fn distribute(vals: &[f32]) -> Vec<f32> {
    let n = vals.len();
    if n < 3 || vals.iter().any(|v| !v.is_finite()) {
        return vals.to_vec();
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| vals[*a].partial_cmp(&vals[*b]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(b)));
    let (lo, hi) = (vals[order[0]], vals[order[n - 1]]);
    let mut out = vals.to_vec();
    for (k, &i) in order.iter().enumerate() {
        out[i] = lo + (hi - lo) * k as f32 / (n - 1) as f32;
    }
    out
}

// ------------------------------------------------------------------------------------------------------------------
// box select, snaps
// ------------------------------------------------------------------------------------------------------------------

/// A point on the screen (u, v) inside the box with corners a and b (either way round).
pub fn in_rect(p: P2, a: P2, b: P2) -> bool {
    let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
    let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
    p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1
}

/// `v` on a grid of `g` round `origin` (g 0 or less: as it is).
pub fn snap_grid(v: f32, g: f32, origin: f32) -> f32 {
    if g > 0.0 && g.is_finite() && v.is_finite() { origin + ((v - origin) / g).round() * g } else { v }
}

/// An angle (degrees) to the nearest `step` (0: as it is), 0 to 360.
pub fn snap_angle(deg: f32, step: f32) -> f32 {
    let d = if step > 0.0 && step.is_finite() { (deg / step).round() * step } else { deg };
    d.rem_euclid(360.0)
}

// ------------------------------------------------------------------------------------------------------------------
// stamps
// ------------------------------------------------------------------------------------------------------------------

/// Mirror a stamp: across its front-to-back line (left and right swap: y -> -y) or its side-to-side line (front and
/// back swap: x -> -x). Only where the pieces stand and how they are turned is mirrored: the game draws a model only
/// as it is made (no negative size), so a lopsided piece is turned to match, not flipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirror {
    None,
    LeftRight,
    FrontBack,
}

/// What is done to a stamp before it goes down: its turn (degrees), a mirror, its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xf {
    pub turn: f32,
    pub mirror: Mirror,
    pub scale: f32,
}

impl Default for Xf {
    fn default() -> Self {
        Xf { turn: 0.0, mirror: Mirror::None, scale: 1.0 }
    }
}

/// The least and most a stamp is sized.
pub const STAMP_SCALE: (f32, f32) = (0.25, 3.0);

/// One piece (x ahead, y to the left, z up from the stamp's foot; its turn in degrees; its size) mirrored and sized
/// (the turn of the whole stamp is Den::stamp's: it turns round the player's spot in 15 degree steps). A piece's
/// own size stays within the game's 0.1 to 5.
pub fn transform(x: f32, y: f32, z: f32, yaw: f32, size: f32, xf: &Xf) -> (f32, f32, f32, f32, f32) {
    let s = if xf.scale.is_finite() { xf.scale.clamp(STAMP_SCALE.0, STAMP_SCALE.1) } else { 1.0 };
    let (x, y, yaw) = match xf.mirror {
        Mirror::None => (x, y, yaw),
        Mirror::LeftRight => (x, -y, -yaw),
        Mirror::FrontBack => (-x, y, 180.0 - yaw),
    };
    (x * s, y * s, z * s, yaw.rem_euclid(360.0), (size * s).clamp(0.1, 5.0))
}

/// The middle a selection is saved round: the middle of its pieces' spread on the ground, and the lowest foot (so
/// the stamp stands where it is put down).
pub fn centre_of(pts: &[P3]) -> P3 {
    if pts.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in pts {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2]]
}

/// A piece for the stamp's picture: where it stands (x ahead, y left), its footprint (w along its front, d across),
/// its turn (degrees) and a colour class (0 building or floor, 1 furniture, 2 small thing, 3 plant, 4 light).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub d: f32,
    pub yaw: f32,
    pub class: u8,
}

/// The stamp's top-down picture: each piece's box corners fitted into a square 0..`px` (north up: +x, the stamp's
/// front, goes up the picture; +y, its left, goes left). One line per piece, `cx,cy,w,h,class` in whole pixels of
/// its turned box's bounds; at most 60 pieces (the biggest first, so the house is there before its cups).
pub fn plan(pieces: &[Piece], px: f32) -> Vec<[i32; 5]> {
    if pieces.is_empty() || px <= 0.0 {
        return Vec::new();
    }
    // each piece's turned box bounds in the stamp's own (x, y)
    let b: Vec<(f32, f32, f32, f32, u8, f32)> = pieces
        .iter()
        .map(|p| {
            let (s, c) = p.yaw.to_radians().sin_cos();
            let (hw, hd) = (p.w.max(0.2) / 2.0, p.d.max(0.2) / 2.0);
            let ex = (c * hw).abs() + (s * hd).abs();
            let ey = (s * hw).abs() + (c * hd).abs();
            (p.x, p.y, ex, ey, p.class, p.w * p.d)
        })
        .collect();
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for &(x, y, ex, ey, _, _) in &b {
        lo = [lo[0].min(x - ex), lo[1].min(y - ey)];
        hi = [hi[0].max(x + ex), hi[1].max(y + ey)];
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1.0);
    let k = (px - 4.0).max(1.0) / span;
    let (mx, my) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
    let mut order: Vec<usize> = (0..b.len()).collect();
    order.sort_by(|a, c| b[*c].5.partial_cmp(&b[*a].5).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(c)));
    order
        .into_iter()
        .take(60)
        .map(|i| {
            let (x, y, ex, ey, class, _) = b[i];
            // the picture's x goes right = the stamp's -y (its right); its y goes up = the stamp's +x (its front)
            let cx = px / 2.0 + (my - y) * k;
            let cy = px / 2.0 + (x - mx) * k;
            [cx.round() as i32, cy.round() as i32, ((2.0 * ey * k).round() as i32).max(2), ((2.0 * ex * k).round() as i32).max(2), class as i32]
        })
        .collect()
}

/// A stamp's category from its file (`category`) or its name: the gallery's Stamps tab groups by it.
pub fn stamp_category(given: Option<&str>, name: &str) -> &'static str {
    const CATS: &[&str] = &["buildings", "gardens", "markets", "camps", "water", "rooms", "paths", "mine", "places"];
    if let Some(c) = given.map(str::to_ascii_lowercase) {
        if let Some(k) = CATS.iter().find(|k| **k == c) {
            return k;
        }
    }
    let n = name.to_ascii_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| n.contains(x));
    if has(&["house", "inn", "smithy", "tavern", "shrine", "tower", "hut"]) {
        "buildings"
    } else if has(&["garden", "grove", "park", "flower"]) {
        "gardens"
    } else if has(&["market", "stall", "shop"]) {
        "markets"
    } else if has(&["camp", "fire", "dojo", "ring"]) {
        "camps"
    } else if has(&["dock", "pier", "pond", "well"]) {
        "water"
    } else if has(&["nook", "room", "study", "kitchen"]) {
        "rooms"
    } else if has(&["path", "road", "fence"]) {
        "paths"
    } else {
        "places"
    }
}

// ------------------------------------------------------------------------------------------------------------------
// measure
// ------------------------------------------------------------------------------------------------------------------

pub fn dist3(a: P3, b: P3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The spread of the points: (width along x, depth along y, height along z).
pub fn spread(pts: &[P3]) -> P3 {
    if pts.is_empty() {
        return [0.0; 3];
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in pts {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]
}

// ------------------------------------------------------------------------------------------------------------------
// collision
// ------------------------------------------------------------------------------------------------------------------

/// What the game does when you walk into a prop. The game itself decides collision, from the model's own file: an
/// M2 model (most props) blocks you only where its maker gave it collision triangles, and a WMO (a building) is
/// solid all over, with floors you walk on and doors you walk through. A server can't add collision to a model that
/// has none, and can't make a model's collision bigger or smaller than the model (it grows with the prop's size).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collide {
    /// Solid in the game: a building, and the kinds of prop that are made solid (crates, barrels, fences, walls,
    /// tables, floors, stairs, rocks).
    Model,
    /// Walked through in the game: the kinds that are made without collision (plants, flowers, banners, rugs, small
    /// things on a table, lights' flames).
    None,
    /// Not known for this kind: try it.
    Unknown,
}

/// The game's collision for a prop, guessed from its Builder category (ovmod.rs `category`) and its model being a
/// building (models.rs `wmos`). A guess: the model file decides, and Den Lite doesn't read model files.
pub fn client_collision(cat: &str, wmo: bool) -> Collide {
    if wmo {
        return Collide::Model;
    }
    match cat {
        "buildings" | "structure" | "fences" | "tables" | "crates" | "barrels" | "tents" => Collide::Model,
        "nature" | "banners" | "lights" | "fires" => Collide::None,
        _ => Collide::Unknown,
    }
}

/// A prop's "walk through" twin: the entry the game is told for a prop with Solid off. The game answers the twin's
/// query as a door (type 0) and the prop is sent open (GO state active): the game lets you through an open door's
/// model (as AzerothCore's GameObject::SetGoState turns a door's collision off when it opens). Above every entry the
/// game or Den Lite uses, and above gizmo.rs's build twin bit; the object's guid keeps the prop's own entry.
pub const PASS_TWIN: u32 = 0x0200_0000;
/// GAMEOBJECT_TYPE_DOOR, and the open state (GO_STATE_ACTIVE).
pub const DOOR: u8 = 0;
pub const GO_STATE_ACTIVE: u8 = 0;

pub fn pass(entry: u32) -> u32 {
    entry | PASS_TWIN
}

pub fn is_pass(entry: u32) -> bool {
    entry & PASS_TWIN != 0
}

pub fn unpass(entry: u32) -> u32 {
    entry & !PASS_TWIN
}

/// A solid prop's top, for the guard rail: its foot (x, y), its turn (radians), half its width and depth, and the
/// height of its top (world z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Top {
    pub at: P2,
    pub yaw: f32,
    pub half: P2,
    pub top: f32,
}

impl Top {
    /// (x, y) over this top (in its turned box, `pad` yards smaller each way so an edge doesn't catch you).
    pub fn over(&self, p: P2, pad: f32) -> bool {
        let (s, c) = self.yaw.sin_cos();
        let (dx, dy) = (p[0] - self.at[0], p[1] - self.at[1]);
        let (lx, ly) = (dx * c + dy * s, -dx * s + dy * c);
        lx.abs() <= (self.half[0] - pad).max(0.05) && ly.abs() <= (self.half[1] - pad).max(0.05)
    }
}

/// How far the feet may sink into a top you were standing on before the guard puts you back on it (yards): more
/// than a step's wobble, less than a fall through.
pub const SINK: f32 = 0.35;
/// Fallen this far below the ground under you: through the world; the guard puts you back on the ground.
pub const FALL_DEPTH: f32 = 6.0;
/// The feet go this far above a top when the guard puts them back (on it, a hair above).
pub const LIFT: f32 = 0.05;

/// The guard rail for a prop marked Solid: the feet were on or above its top and are now more than [`SINK`] under
/// it while over it (the game let you fall into it: its model has no collision, or the game hasn't loaded it yet).
/// The height to put the feet back at, the highest such top.
pub fn top_hold(was: P3, now: P3, tops: &[Top]) -> Option<f32> {
    tops.iter()
        .filter(|t| t.over([now[0], now[1]], 0.1) && was[2] >= t.top - 0.15 && now[2] < t.top - SINK && now[2] > t.top - 4.0)
        .map(|t| t.top + LIFT)
        .fold(None, |a: Option<f32>, z| Some(a.map_or(z, |m| m.max(z))))
}

/// Fallen through the ground (the feet [`FALL_DEPTH`] under it).
pub fn through_ground(z: f32, ground: f32) -> bool {
    z.is_finite() && ground.is_finite() && z < ground - FALL_DEPTH
}

/// The highest top under (x, y) that is no higher than `from` + 0.5 (surface snap: "put it on what is under it"):
/// None when there is no top there.
pub fn top_under(p: P2, from: f32, tops: &[Top]) -> Option<f32> {
    tops.iter().filter(|t| t.over(p, 0.0) && t.top <= from + 0.5).map(|t| t.top).fold(None, |a: Option<f32>, z| Some(a.map_or(z, |m| m.max(z))))
}

// ------------------------------------------------------------------------------------------------------------------

fn dist2(a: P2, b: P2) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

/// An angle (radians) to 0 .. 2 pi.
pub fn wrap(a: f32) -> f32 {
    a.rem_euclid(2.0 * PI)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32, e: f32) -> bool {
        (a - b).abs() <= e
    }

    #[test]
    fn a_curve_goes_through_every_point_and_two_points_are_a_line() {
        let pts = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let c = smooth(&pts, 8);
        assert_eq!(c.len(), 3 * 8 + 1);
        for (i, p) in pts.iter().enumerate() {
            assert!(near(c[i * 8][0], p[0], 1e-3) && near(c[i * 8][1], p[1], 1e-3), "through point {i}: {:?}", c[i * 8]);
        }
        // centripetal: no loop past the corner (stays within a yard of the square's hull)
        assert!(c.iter().all(|p| p[0] > -1.5 && p[0] < 11.5 && p[1] > -1.5 && p[1] < 11.5), "{c:?}");
        assert_eq!(smooth(&[[0.0, 0.0], [5.0, 0.0]], 8), vec![[0.0, 0.0], [5.0, 0.0]]);
        // a double click is one point
        assert_eq!(smooth(&[[0.0, 0.0], [0.0, 0.0], [5.0, 0.0]], 8).len(), 2);
        assert!(smooth(&[[f32::NAN, 0.0], [1.0, 1.0]], 8).len() == 1);
    }

    #[test]
    fn lamps_every_three_yards_with_one_at_the_end() {
        let l = Lay { every: 3.0, smooth: false, ..Default::default() };
        let v = lay(&[[0.0, 0.0], [10.0, 0.0]], &l);
        let xs: Vec<f32> = v.iter().map(|p| p.0[0]).collect();
        assert_eq!(xs, vec![0.0, 3.0, 6.0, 9.0, 10.0], "0..9, then the end (1 yd left: more than a quarter step)");
        assert!(v.iter().all(|p| near(p.1, 0.0, 1e-4)), "facing along +x");
        // 9.5: half a yard left over is less than a quarter of 3: no extra lamp
        let v = lay(&[[0.0, 0.0], [9.5, 0.0]], &l);
        assert_eq!(v.len(), 4);
    }

    #[test]
    fn fences_join_end_to_end_on_the_middles() {
        let l = Lay { every: 2.0, smooth: false, mid: true, ..Default::default() };
        let v = lay(&[[0.0, 0.0], [0.0, 8.0]], &l);
        let ys: Vec<f32> = v.iter().map(|p| p.0[1]).collect();
        assert_eq!(ys, vec![1.0, 3.0, 5.0, 7.0]);
        assert!(v.iter().all(|p| near(p.1, PI / 2.0, 1e-4)), "along +y");
        // 9 yards: 4 whole steps and one more (1 yd > half... a quarter): 5 fences spread evenly, 1.8 apart
        let v = lay(&[[0.0, 0.0], [9.0, 0.0]], &l);
        assert_eq!(v.len(), 5);
        assert!(near(v[1].0[0] - v[0].0[0], 1.8, 1e-3));
        // a short click still gets one
        assert_eq!(lay(&[[0.0, 0.0], [0.3, 0.0]], &l).len(), 1);
    }

    #[test]
    fn both_sides_face_the_walk_and_the_count_is_capped() {
        let l = Lay { every: 4.0, smooth: false, face: Face::Across, sides: 3, offset: 2.0, ..Default::default() };
        let v = lay(&[[0.0, 0.0], [8.0, 0.0]], &l);
        assert_eq!(v.len(), 6, "three marks, two sides");
        assert!(near(v[0].0[1], 2.0, 1e-4) && near(v[1].0[1], -2.0, 1e-4), "left then right");
        // the left row faces +y + 90 (= pi/2 across), the right row the other way: both toward the path
        assert!(near(v[0].1, PI / 2.0, 1e-4) && near(v[1].1, 3.0 * PI / 2.0, 1e-4), "{:?}", (v[0].1, v[1].1));
        let long = lay(&[[0.0, 0.0], [5000.0, 0.0]], &Lay { every: 0.01, ..Default::default() });
        assert_eq!(long.len(), PATH_MAX, "capped (and 0.01 is raised to the least spacing)");
    }

    #[test]
    fn a_smoothed_path_is_spaced_by_its_length() {
        let pts = [[0.0, 0.0], [10.0, 5.0], [20.0, 0.0]];
        let l = Lay { every: 2.0, ..Default::default() };
        let v = lay(&pts, &l);
        for w in v.windows(2).take(v.len() - 2) {
            let d = dist2(w[0].0, w[1].0).sqrt();
            assert!(d > 1.9 && d <= 2.0001, "spacing along the curve, chord a little under: {d}");
        }
        let total = length(&smooth(&pts, SMOOTH_STEPS));
        assert!(total > 22.3 && total < 24.0, "{total}");
    }

    #[test]
    fn points_read_and_written() {
        assert_eq!(parse_points("1,2; 3.5 ,-4;x,1;9e9,0;"), vec![[1.0, 2.0], [3.5, -4.0]]);
        assert_eq!(write_points(&[[1.0, 2.0], [3.456, -4.0]]), "1.00,2.00;3.46,-4.00");
        assert_eq!(parse_points(&"1,1;".repeat(200)).len(), POINTS_MAX);
    }

    #[test]
    fn arrays_in_a_line_and_round_a_ring() {
        assert_eq!(line_array(3, [2.0, 0.0, 0.5]), vec![[2.0, 0.0, 0.5], [4.0, 0.0, 1.0], [6.0, 0.0, 1.5]]);
        assert_eq!(line_array(500, [1.0, 0.0, 0.0]).len(), ARRAY_MAX);
        let r = ring_array(4, 5.0, 0.0, RingFace::In, 0.0);
        assert_eq!(r.len(), 4);
        assert!(near(r[1].0[0], 0.0, 1e-4) && near(r[1].0[1], 5.0, 1e-4));
        assert!(near(r[0].1, PI, 1e-4), "facing the middle");
        assert!(near(ring_array(4, 5.0, 0.0, RingFace::Out, 0.0)[1].1, PI / 2.0, 1e-4));
        assert!(near(ring_array(4, 5.0, 0.0, RingFace::Keep, 1.0)[3].1, 1.0, 1e-4));
        assert_eq!(ring_array(0, 5.0, 0.0, RingFace::In, 0.0).len(), 1);
    }

    #[test]
    fn align_and_distribute() {
        let v = [3.0, 1.0, 7.0];
        assert_eq!(align_to(&v, To::Min), Some(1.0));
        assert_eq!(align_to(&v, To::Max), Some(7.0));
        assert_eq!(align_to(&v, To::Mid), Some(4.0));
        assert_eq!(align_to(&v, To::First), Some(3.0));
        assert_eq!(align_to(&[], To::Min), None);
        assert_eq!(distribute(&[0.0, 10.0, 1.0, 9.0]), vec![0.0, 10.0, 10.0 / 3.0, 20.0 / 3.0]);
        assert_eq!(distribute(&[5.0, 1.0]), vec![5.0, 1.0], "two: as they are");
        // ties keep the list's order
        assert_eq!(distribute(&[0.0, 2.0, 2.0, 6.0]), vec![0.0, 2.0, 4.0, 6.0]);
    }

    #[test]
    fn box_and_snaps() {
        assert!(in_rect([0.1, 0.2], [0.5, 0.5], [-0.5, -0.5]));
        assert!(!in_rect([0.6, 0.2], [0.5, 0.5], [-0.5, -0.5]));
        assert_eq!(snap_grid(10.4, 1.0, 0.5), 10.5);
        assert_eq!(snap_grid(10.4, 0.0, 0.5), 10.4);
        assert_eq!(snap_angle(52.0, 45.0), 45.0);
        assert_eq!(snap_angle(-10.0, 15.0), 345.0);
    }

    #[test]
    fn a_stamp_mirrored_and_sized() {
        let xf = Xf { mirror: Mirror::LeftRight, scale: 2.0, ..Default::default() };
        assert_eq!(transform(3.0, 1.0, 0.5, 30.0, 1.0, &xf), (6.0, -2.0, 1.0, 330.0, 2.0));
        let xf = Xf { mirror: Mirror::FrontBack, ..Default::default() };
        assert_eq!(transform(3.0, 1.0, 0.0, 30.0, 1.0, &xf), (-3.0, 1.0, 0.0, 150.0, 1.0));
        // a piece's size stays the game's, the stamp's within its own bounds
        let big = Xf { scale: 99.0, ..Default::default() };
        assert_eq!(transform(1.0, 0.0, 0.0, 0.0, 4.0, &big).4, 5.0);
        assert_eq!(transform(1.0, 0.0, 0.0, 0.0, 1.0, &big).0, STAMP_SCALE.1);
        // mirroring twice is the stamp again
        let m = Xf { mirror: Mirror::LeftRight, ..Default::default() };
        let (x, y, z, yaw, _) = transform(2.0, 3.0, 0.0, 70.0, 1.0, &m);
        let back = transform(x, y, z, yaw, 1.0, &m);
        assert!(near(back.1, 3.0, 1e-5) && near(back.3, 70.0, 1e-3));
    }

    #[test]
    fn a_selection_saved_round_its_middle_and_its_picture() {
        assert_eq!(centre_of(&[[0.0, 0.0, 2.0], [10.0, 4.0, 1.0], [4.0, -2.0, 3.0]]), [5.0, 1.0, 1.0]);
        let pieces = [
            Piece { x: 0.0, y: 0.0, w: 10.0, d: 10.0, yaw: 0.0, class: 0 },
            Piece { x: 4.0, y: 0.0, w: 1.0, d: 1.0, yaw: 45.0, class: 2 },
        ];
        let p = plan(&pieces, 64.0);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0][4], 0, "the biggest first");
        assert_eq!((p[0][0], p[0][1]), (32, 32), "the house in the middle (the cup is inside its spread)");
        assert!(p[1][1] > p[0][1], "the piece ahead is higher up the picture");
        assert!(p.iter().all(|r| r[0] >= 0 && r[0] <= 64 && r[1] >= 0 && r[1] <= 64));
        assert!(plan(&[], 64.0).is_empty());
    }

    #[test]
    fn stamp_categories() {
        assert_eq!(stamp_category(Some("Gardens"), "x"), "gardens");
        assert_eq!(stamp_category(Some("nonsense"), "smithy"), "buildings");
        assert_eq!(stamp_category(None, "my-dock"), "water");
        assert_eq!(stamp_category(None, "porch"), "places");
    }

    #[test]
    fn measure() {
        assert_eq!(dist3([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]), 5.0);
        assert_eq!(spread(&[[0.0, 0.0, 0.0], [3.0, -1.0, 2.0]]), [3.0, 1.0, 2.0]);
    }

    #[test]
    fn collision_guesses_and_the_walk_through_twin() {
        assert_eq!(client_collision("crates", false), Collide::Model);
        assert_eq!(client_collision("nature", false), Collide::None);
        assert_eq!(client_collision("nature", true), Collide::Model, "a building is solid whatever its name");
        assert_eq!(client_collision("decor", false), Collide::Unknown);
        let e = 2413;
        assert!(is_pass(pass(e)) && !is_pass(e));
        assert_eq!(unpass(pass(e)), e);
        assert_eq!(pass(e) & 0x0100_0000, 0, "not the build twin's bit");
        assert_eq!(pass(e) & 0xFF_FFFF, e, "the guid's 24 bits of entry are the prop's own");
    }

    #[test]
    fn the_guard_puts_you_back_on_a_solid_top() {
        let t = Top { at: [0.0, 0.0], yaw: 0.0, half: [2.0, 1.0], top: 1.0 };
        // standing on it, then sunk half a yard: back on top
        assert_eq!(top_hold([0.5, 0.0, 1.0], [0.5, 0.2, 0.5], &[t]), Some(1.0 + LIFT));
        // a step's wobble: left alone
        assert_eq!(top_hold([0.5, 0.0, 1.0], [0.5, 0.0, 0.8], &[t]), None);
        // walking under or into it from beside (never above its top): left alone
        assert_eq!(top_hold([0.5, 0.0, 0.0], [0.5, 0.0, 0.0], &[t]), None);
        // off its edge: falling is falling
        assert_eq!(top_hold([0.5, 0.0, 1.0], [0.5, 1.5, 0.3], &[t]), None);
        // turned 90 degrees: its long side along y
        let r = Top { yaw: PI / 2.0, ..t };
        assert!(r.over([0.0, 1.5], 0.1) && !r.over([1.5, 0.0], 0.1));
        // two tops: the higher one that you were on
        let hi = Top { top: 3.0, ..t };
        assert_eq!(top_hold([0.0, 0.0, 3.0], [0.0, 0.0, 2.0], &[t, hi]), Some(3.0 + LIFT));
        assert!(through_ground(-10.0, 0.0) && !through_ground(-2.0, 0.0));
        assert_eq!(top_under([0.0, 0.0], 2.0, &[t, hi]), Some(1.0), "the table's top, not the shelf above");
        assert_eq!(top_under([5.0, 0.0], 2.0, &[t]), None);
    }
}
