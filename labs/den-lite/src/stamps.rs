//! Stamps as places (owner, 3 Nov: "Jaina not very good at making stamps ... fix her build ability"): the sizes of
//! their pieces, the order she builds them in, where she stands, and the stamps' other names.
//!
//! - Sizes: the model's real box from the player's own GameObjectDisplayInfo (models.rs) when Den Lite has it, else
//!   our own estimates (tools/den-stamps/footprints.json, the ones the stamp composer lays the stamps out with).
//! - [`settle`]: a stamp put down where the real boxes are known: a small thing over a table or a deck stands on its
//!   real top, and two standing pieces that would be in each other are moved apart (the estimates were a little
//!   small). Without the real boxes the stamp is as the composer wrote it.
//! - [`order`] and [`pace_ms`]: the build goes the way a person builds a place: the building, then rugs and boards,
//!   then the big pieces, the furniture, the small things, last what stands on a table.
//! - [`clear_spot`]: the free spot nearest a stamp's middle (she never stands inside a piece), and [`admire_spot`]:
//!   a step back from it on your side, facing it, for her look at the finished place.
//! - [`canonical`]: "build a tavern", "a blacksmith", "a reading nook": the stamp each word means.
//!
//! docs/DEN-STAMPS.md has the rules and each stamp.

use crate::companion::At;
use crate::den::{template, DenDoc, Prop, WorldObject};
use serde_json::Value;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::OnceLock;

/// Our own size estimates (yards, at each piece's normal size): the composer's table.
pub const FOOTPRINTS: &str = include_str!("../../../tools/den-stamps/footprints.json");

/// What a piece is to a place (footprints.json `kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Shell,
    Floor,
    Fence,
    Table,
    Seat,
    Item,
    Ground,
}

/// A piece's size in the world: along its front (w), side to side (d), its height; `real` when from the model's box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f32,
    pub d: f32,
    pub h: f32,
    pub kind: Kind,
    pub real: bool,
}

fn kind_of(s: &str) -> Kind {
    match s {
        "shell" => Kind::Shell,
        "floor" => Kind::Floor,
        "fence" => Kind::Fence,
        "table" => Kind::Table,
        "seat" => Kind::Seat,
        "item" => Kind::Item,
        _ => Kind::Ground,
    }
}

fn table() -> &'static HashMap<u32, Size> {
    static T: OnceLock<HashMap<u32, Size>> = OnceLock::new();
    T.get_or_init(|| {
        let v: Value = serde_json::from_str(FOOTPRINTS).unwrap_or(Value::Null);
        let mut out = HashMap::new();
        if let Some(m) = v.get("pieces").and_then(Value::as_object) {
            for (k, f) in m {
                let n = |key: &str| f.get(key).and_then(Value::as_f64).unwrap_or(1.0) as f32;
                if let Ok(e) = k.parse::<u32>() {
                    let kind = kind_of(f.get("kind").and_then(Value::as_str).unwrap_or(""));
                    out.insert(e, Size { w: n("w"), d: n("d"), h: n("h"), kind, real: false });
                }
            }
        }
        out
    })
}

/// The estimate for an entry at its normal size (None: not in the table).
pub fn estimate(entry: u32) -> Option<Size> {
    table().get(&entry).copied()
}

/// A piece's size at `scale` (None: its normal size): the real box through `real(display)` when given, else the
/// estimate scaled from the normal size, else a yard cube.
pub fn size_with(entry: u32, scale: Option<f32>, real: impl Fn(u32) -> Option<(f32, f32, f32, f32)>) -> Size {
    let t = template(entry);
    let normal = t.map(|t| t.size).unwrap_or(1.0).max(0.01);
    let s = scale.unwrap_or(normal);
    let est = estimate(entry);
    let kind = est.map(|e| e.kind).unwrap_or(Kind::Ground);
    if let Some((w, d, h, _)) = t.and_then(|t| real(t.display)).filter(|b| b.0 > 0.01 && b.1 > 0.01) {
        return Size { w: w * s, d: d * s, h: h * s, kind, real: true };
    }
    match est {
        Some(e) => Size { w: e.w * s / normal, d: e.d * s / normal, h: e.h * s / normal, kind, real: false },
        None => Size { w: s, d: s, h: s, kind, real: false },
    }
}

/// [`size_with`] from the models Den Lite read at its start.
pub fn size(entry: u32, scale: Option<f32>) -> Size {
    size_with(entry, scale, crate::models::box_of)
}

/// A world piece's size (its scale is set: the normal one when the file said none).
pub fn size_of(o: &WorldObject) -> Size {
    size(o.entry, Some(o.scale))
}

// ---------------------------------------------------------------- boxes

/// A turned box on the ground: middle, half sizes, facing (radians).
#[derive(Debug, Clone, Copy)]
pub struct Bx {
    pub x: f32,
    pub y: f32,
    pub hw: f32,
    pub hd: f32,
    pub o: f32,
}

impl Bx {
    fn corners(&self, grow: f32) -> [(f32, f32); 4] {
        let (s, c) = self.o.sin_cos();
        let (a, b) = (self.hw + grow, self.hd + grow);
        [(a, b), (-a, b), (-a, -b), (a, -b)].map(|(u, v)| (self.x + u * c - v * s, self.y + u * s + v * c))
    }

    /// (x, y) inside, `pad` yards around it counted in.
    pub fn holds(&self, x: f32, y: f32, pad: f32) -> bool {
        let (s, c) = self.o.sin_cos();
        let (dx, dy) = (x - self.x, y - self.y);
        let (u, v) = (dx * c + dy * s, -dx * s + dy * c);
        u.abs() <= self.hw + pad && v.abs() <= self.hd + pad
    }
}

/// Two boxes closer than `gap` (the separating axis test, as the composer's).
pub fn meet(a: &Bx, b: &Bx, gap: f32) -> bool {
    let (pa, pb) = (a.corners(gap / 2.0), b.corners(gap / 2.0));
    for poly in [&pa, &pb] {
        for i in 0..4 {
            let ((x1, y1), (x2, y2)) = (poly[i], poly[(i + 1) % 4]);
            let (nx, ny) = (y2 - y1, x1 - x2);
            let proj = |p: &[(f32, f32); 4]| p.iter().map(|(x, y)| nx * x + ny * y).fold((f32::MAX, f32::MIN), |m, v| (m.0.min(v), m.1.max(v)));
            let (ra, rb) = (proj(&pa), proj(&pb));
            if ra.1 <= rb.0 + 1e-6 || rb.1 <= ra.0 + 1e-6 {
                return false;
            }
        }
    }
    true
}

fn bx(x: f32, y: f32, o: f32, s: &Size) -> Bx {
    Bx { x, y, hw: s.w / 2.0, hd: s.d / 2.0, o }
}

// ---------------------------------------------------------------- settle

/// [`settle_with`] with the models Den Lite read.
pub fn settle(doc: &DenDoc) -> DenDoc {
    settle_with(doc, crate::models::box_of)
}

/// A stamp fitted to the real boxes (`real(display)`): unchanged when none of its pieces has one.
/// - A piece raised a little (0.2 to 3 yards: on something) over a table or a floor piece stands on its real top.
/// - Two standing pieces (not a building, a rug or board, a fence's own run, a raised one) closer than a hand's width
///   are moved apart: the smaller one, away from the other, a tenth of a yard at a time, at most 1.5 yards.
pub fn settle_with(doc: &DenDoc, real: impl Fn(u32) -> Option<(f32, f32, f32, f32)>) -> DenDoc {
    let mut out = doc.clone();
    let sizes: Vec<Size> = out.props.iter().map(|p| size_with(p.entry as u32, p.scale.map(|s| s as f32), &real)).collect();
    if !sizes.iter().any(|s| s.real) {
        return out;
    }
    let boxes = |props: &[Prop]| -> Vec<Bx> {
        props.iter().zip(&sizes).map(|(p, s)| bx(p.x as f32, p.y as f32, (p.yaw as f32).to_radians(), s)).collect()
    };
    // things on tables and decks
    let b = boxes(&out.props);
    for i in 0..out.props.len() {
        let z = out.props[i].z as f32;
        if !(0.2..=3.0).contains(&z) {
            continue;
        }
        let (x, y) = (out.props[i].x as f32, out.props[i].y as f32);
        let under = (0..out.props.len())
            .filter(|&j| j != i && matches!(sizes[j].kind, Kind::Table | Kind::Floor) && (out.props[j].z as f32) < z - 0.1 && b[j].holds(x, y, 0.0))
            .max_by(|&a, &c| out.props[a].z.partial_cmp(&out.props[c].z).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(j) = under.filter(|&j| sizes[j].real) {
            out.props[i].z = ((out.props[j].z as f32 + sizes[j].h) * 100.0).round() as f64 / 100.0;
        }
    }
    // standing pieces apart
    let standing = |i: usize, props: &[Prop]| -> bool {
        !matches!(sizes[i].kind, Kind::Shell | Kind::Floor) && props[i].z.abs() < 0.2 && props[i].role.as_deref() != Some("house")
    };
    for _ in 0..6 {
        let mut moved = false;
        for i in 0..out.props.len() {
            for j in 0..out.props.len() {
                if i == j || !standing(i, &out.props) || !standing(j, &out.props) {
                    continue;
                }
                if sizes[i].kind == Kind::Fence && sizes[j].kind == Kind::Fence {
                    continue;
                }
                // move the smaller (i) away from the bigger (j)
                if sizes[i].w * sizes[i].d > sizes[j].w * sizes[j].d || out.props[i].role.is_some() {
                    continue;
                }
                let b = boxes(&out.props);
                if !meet(&b[i], &b[j], 0.05) {
                    continue;
                }
                let (mut dx, mut dy) = ((out.props[i].x - out.props[j].x) as f32, (out.props[i].y - out.props[j].y) as f32);
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1e-3 {
                    (dx, dy) = (1.0, 0.0);
                } else {
                    (dx, dy) = (dx / len, dy / len);
                }
                let (x0, y0) = (out.props[i].x, out.props[i].y);
                for k in 1..=15 {
                    let step = 0.1 * k as f32;
                    let t = Bx { x: x0 as f32 + dx * step, y: y0 as f32 + dy * step, ..b[i] };
                    if !meet(&t, &b[j], 0.05) || k == 15 {
                        out.props[i].x = ((t.x * 100.0).round() / 100.0) as f64;
                        out.props[i].y = ((t.y * 100.0).round() / 100.0) as f64;
                        moved = true;
                        break;
                    }
                }
            }
        }
        if !moved {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------- the build

/// A stamp's focal point (x ahead, y left of where it is put down): its `focus` (the composer writes it: the fire,
/// the bar, the forge), else the middle of its pieces (a building's own box left out).
pub fn focus(st: &DenDoc) -> (f64, f64) {
    let f = st.extra.get("focus").and_then(Value::as_array).and_then(|a| Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)));
    if let Some((x, y)) = f.filter(|(x, y)| x.is_finite() && y.is_finite() && x.abs() <= 40.0 && y.abs() <= 40.0) {
        return (x, y);
    }
    let ps: Vec<&Prop> = st.props.iter().filter(|p| p.role.as_deref() != Some("house")).collect();
    if ps.is_empty() {
        return (0.0, 0.0);
    }
    let (x0, x1) = ps.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.x), m.1.max(p.x)));
    let (y0, y1) = ps.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.y), m.1.max(p.y)));
    ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// The order a piece goes up in: 0 the building, 1 rugs and boards, 2 the big pieces (tents, stalls, fences, a bed),
/// 3 furniture and middling things, 4 small things, 5 what stands on a table.
pub fn tier(s: &Size, raised: bool) -> u8 {
    match s.kind {
        Kind::Shell => 0,
        Kind::Floor => 1,
        _ if raised && s.kind == Kind::Item => 5,
        Kind::Item => 4,
        Kind::Fence => 2,
        _ if s.w * s.d >= 2.5 || s.h >= 2.8 => 2,
        Kind::Table | Kind::Seat => 3,
        _ if s.w * s.d >= 0.6 => 3,
        _ => 4,
    }
}

/// How long she takes over a piece of that tier before the next appears: a building or a tent takes longer.
pub fn pace_ms(tier: u8) -> u64 {
    match tier {
        0 => 700,
        1 | 2 => 400,
        3 => 250,
        _ => 180,
    }
}

/// The pieces in build order: by tier, then nearest `from` first (round the place, not back and forth). `ground`:
/// the floor's height there (a piece more than 0.2 above it is on something).
pub fn order(pieces: &mut [WorldObject], from: At, ground: f32) {
    let key = |o: &WorldObject| {
        let s = size_of(o);
        (tier(&s, o.z > ground + 0.2), ((o.x - from.x).powi(2) + (o.y - from.y).powi(2)).sqrt())
    };
    pieces.sort_by(|a, b| key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal));
}

/// Her own room: half a yard round her.
pub const HER: f32 = 0.5;

fn blocked(x: f32, y: f32, pieces: &[WorldObject], ground: f32) -> bool {
    pieces.iter().any(|o| {
        let s = size_of(o);
        // she stands on rugs and boards, and under nothing hung high
        !matches!(s.kind, Kind::Floor | Kind::Shell) && o.z < ground + 1.8 && bx(o.x, o.y, o.o, &s).holds(x, y, HER)
    })
}

/// The free spot nearest `mid` (outward in half-yard rings, 16 ways round each), facing `mid`; `mid` itself when free.
pub fn clear_spot(mid: At, pieces: &[WorldObject]) -> At {
    let ground = mid.z;
    if !blocked(mid.x, mid.y, pieces, ground) {
        return mid;
    }
    for ring in 1..=30 {
        let r = ring as f32 * 0.5;
        for k in 0..16 {
            // start on the side she came from (behind `mid`'s facing), so she needn't walk round
            let a = mid.o + PI + k as f32 * (2.0 * PI / 16.0);
            let (x, y) = (mid.x + r * a.cos(), mid.y + r * a.sin());
            if !blocked(x, y, pieces, ground) {
                let o = (mid.y - y).atan2(mid.x - x).rem_euclid(2.0 * PI);
                return At { x, y, z: mid.z, o };
            }
        }
    }
    mid
}

/// A step back to look at it: on the line from the stamp's middle to `you`, just past its last piece on that side
/// (a yard and a half out), free of pieces, facing the middle.
pub fn admire_spot(mid: At, pieces: &[WorldObject], you: At) -> At {
    let (mut dx, mut dy) = (you.x - mid.x, you.y - mid.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.5 {
        (dx, dy) = (-mid.o.cos(), -mid.o.sin());
    } else {
        (dx, dy) = (dx / len, dy / len);
    }
    let reach = pieces
        .iter()
        .map(|o| {
            let s = size_of(o);
            (o.x - mid.x) * dx + (o.y - mid.y) * dy + s.w.max(s.d) / 2.0
        })
        .fold(0.0f32, f32::max);
    let r = (reach + 1.5).clamp(2.0, if len >= 0.5 { len.max(2.0) } else { 30.0 });
    let spot = At { x: mid.x + dx * r, y: mid.y + dy * r, z: mid.z, o: (-dy).atan2(-dx).rem_euclid(2.0 * PI) };
    let free = clear_spot(At { o: (spot.o + PI).rem_euclid(2.0 * PI), ..spot }, pieces);
    At { o: (mid.y - free.y).atan2(mid.x - free.x).rem_euclid(2.0 * PI), ..free }
}

// ---------------------------------------------------------------- names

/// Every stamp's other names: "Jaina, build a tavern" builds the inn.
pub const NAMES: &[(&str, &[&str])] = &[
    ("campfire", &["camp", "campsite", "camp-site", "fire", "bonfire", "firepit", "campfire"]),
    ("inn", &["inn", "tavern", "pub", "bar", "alehouse", "taproom"]),
    ("market", &["market", "marketplace", "bazaar", "stalls", "stall", "shops", "shop", "fair"]),
    ("garden", &["garden", "farm", "plot", "field", "allotment", "vegetables", "veggies", "crops", "patch"]),
    ("dock", &["dock", "docks", "pier", "jetty", "wharf", "harbor", "harbour", "boardwalk", "quay"]),
    ("house", &["house", "home", "cottage", "cabin", "hut"]),
    ("smithy", &["smithy", "forge", "blacksmith", "smith", "anvil", "workshop", "smithing"]),
    ("nook", &["nook", "reading", "library", "study", "books", "bookcorner", "reading-nook"]),
    ("shrine", &["shrine", "altar", "temple", "statue", "chapel", "sanctuary"]),
    ("dojo", &["dojo", "ring", "training", "arena"]),
];

/// The stamp a word means ("tavern" -> "inn"); None: no stamp is called that.
pub fn canonical(word: &str) -> Option<&'static str> {
    let w = word.trim().to_lowercase();
    let w = w.trim_end_matches("'s");
    NAMES.iter().find(|(_, names)| names.contains(&w) || names.contains(&w.trim_end_matches('s'))).map(|(n, _)| *n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::den::{parse, read_stamp, BUILTIN_STAMPS};
    use std::path::Path;

    fn obj(entry: u32, x: f32, y: f32, z: f32, o: f32) -> WorldObject {
        let t = template(entry).unwrap();
        WorldObject { guid: (x * 100.0) as u64 ^ entry as u64, entry, display: t.display, kind: t.kind, scale: t.size, x, y, z, o, pitch: 0.0, roll: 0.0 }
    }

    #[test]
    fn the_estimates_cover_every_stamp_piece_and_are_sane() {
        for (name, text) in BUILTIN_STAMPS {
            let doc = parse(text).unwrap();
            for p in &doc.props {
                let e = estimate(p.entry as u32).unwrap_or_else(|| panic!("{name}: {} has no size estimate", p.entry));
                assert!(e.w > 0.0 && e.d > 0.0 && e.h > 0.0 && !e.real);
                assert!(template(p.entry as u32).is_some(), "{name}: {} is an object Den Lite knows", p.entry);
            }
        }
        assert_eq!(estimate(1953).unwrap().kind, Kind::Ground);
        assert_eq!(estimate(181075).unwrap().kind, Kind::Table);
        // the estimate follows a piece's size; the real box wins when there is one
        let half = size_with(179969, Some(0.5), |_| None);
        assert!((half.w - 0.5).abs() < 1e-5 && !half.real);
        let real = size_with(179969, None, |d| (d == template(179969).unwrap().display).then_some((2.0, 1.5, 1.0, 0.0)));
        assert!(real.real && (real.w - 2.0).abs() < 1e-5 && (real.d - 1.5).abs() < 1e-5);
        assert_eq!(size_with(1, None, |_| None).w, 1.0, "unknown: a yard");
    }

    #[test]
    fn boxes_meet_as_the_composers_do() {
        let a = Bx { x: 0.0, y: 0.0, hw: 0.5, hd: 0.5, o: 0.0 };
        assert!(meet(&a, &Bx { x: 0.95, ..a }, 0.1));
        assert!(!meet(&a, &Bx { x: 1.2, ..a }, 0.1));
        assert!(meet(&a, &Bx { x: 1.2, o: PI / 4.0, ..a }, 0.1), "turned, its corner reaches");
        assert!(a.holds(0.4, -0.4, 0.0) && !a.holds(0.6, 0.0, 0.0) && a.holds(0.6, 0.0, 0.2));
    }

    #[test]
    fn without_real_boxes_a_stamp_is_as_the_composer_wrote_it() {
        for (_, text) in BUILTIN_STAMPS {
            let doc = parse(text).unwrap();
            assert_eq!(settle_with(&doc, |_| None), doc);
        }
    }

    #[test]
    fn with_real_boxes_things_stand_on_the_table_and_pieces_move_apart() {
        let doc = parse(
            r#"{"format":"leyline-den","version":1,"kind":"stamp","props":[
            {"entry":181075,"x":0,"y":0,"z":0,"yaw":0},
            {"entry":2562,"x":0.2,"y":0.1,"z":0.95,"yaw":0},
            {"entry":179969,"x":3,"y":0,"z":0,"yaw":0},
            {"entry":179967,"x":3.6,"y":0,"z":0,"yaw":0},
            {"entry":19007,"x":3,"y":0,"z":0,"yaw":0,"role":"house"}]}"#,
        )
        .unwrap();
        let t = template(181075).unwrap().display;
        let crate_d = template(179969).unwrap().display;
        let barrel = template(179967).unwrap().display;
        let real = move |d: u32| {
            if d == t {
                Some((1.4, 2.2, 1.1, 0.0))
            } else if d == crate_d {
                Some((1.2, 1.2, 1.0, 0.0))
            } else if d == barrel {
                Some((0.8, 0.8, 1.2, 0.0))
            } else {
                None
            }
        };
        let s = settle_with(&doc, real);
        assert!((s.props[1].z - 1.1).abs() < 1e-6, "the bread on the real table top: {}", s.props[1].z);
        let size = |i: usize| size_with(s.props[i].entry as u32, None, real);
        let b = |i: usize| bx(s.props[i].x as f32, s.props[i].y as f32, (s.props[i].yaw as f32).to_radians(), &size(i));
        assert!(!meet(&b(2), &b(3), 0.04), "crate and barrel apart: {:?} {:?}", s.props[2], s.props[3]);
        assert!(s.props[3].x > 3.6, "the smaller one (the barrel) moved, away from the crate");
        assert_eq!((s.props[2].x, s.props[4].x), (3.0, 3.0), "the bigger and the building stay");
    }

    #[test]
    fn builds_go_building_floor_big_furniture_small_then_on_tables() {
        let ground = 10.0;
        let mut ps = vec![
            obj(2562, 1.0, 0.0, ground + 0.95, 0.0), // bread on the table
            obj(2413, 1.0, 1.0, ground, 0.0),        // a chair
            obj(184592, 5.0, 0.0, ground, 0.0),      // a tent
            obj(181396, 0.5, 0.0, ground, 0.0),      // a rug
            obj(181075, 1.0, 0.0, ground, 0.0),      // the table
            obj(19007, 9.0, 0.0, ground, 0.0),       // a building
            obj(1558, 4.0, 4.0, ground, 0.0),        // a candle on the ground
        ];
        order(&mut ps, At { x: 0.0, y: 0.0, z: ground, o: 0.0 }, ground);
        let e: Vec<u32> = ps.iter().map(|o| o.entry).collect();
        assert_eq!(e, vec![19007, 181396, 184592, 181075, 2413, 1558, 2562]);
        assert!(pace_ms(0) > pace_ms(2) && pace_ms(2) > pace_ms(3) && pace_ms(3) > pace_ms(5));
    }

    #[test]
    fn she_stands_clear_of_every_piece_and_steps_back_to_look() {
        let doc = read_stamp(Path::new("no-such-folder"), "campfire").unwrap();
        let mid = At { x: 100.0, y: 100.0, z: 5.0, o: 0.0 };
        let ps: Vec<WorldObject> = doc.props.iter().map(|p| obj(p.entry as u32, mid.x + p.x as f32, mid.y + p.y as f32, mid.z + p.z as f32, (p.yaw as f32).to_radians())).collect();
        assert!(blocked(mid.x, mid.y, &ps, mid.z), "the fire is in the middle");
        let s = clear_spot(mid, &ps);
        assert!(!blocked(s.x, s.y, &ps, mid.z));
        let d = ((s.x - mid.x).powi(2) + (s.y - mid.y).powi(2)).sqrt();
        assert!(d > 0.9 && d < 2.6, "beside the fire, inside the ring: {d}");
        let to_mid = (mid.y - s.y).atan2(mid.x - s.x).rem_euclid(2.0 * PI);
        assert!((s.o - to_mid).abs() < 1e-4, "facing the middle");
        // you stand 12 yards back (-x): she steps back towards you, past the seats, facing the fire
        let you = At { x: mid.x - 12.0, ..mid };
        let a = admire_spot(mid, &ps, you);
        assert!(!blocked(a.x, a.y, &ps, mid.z));
        assert!(a.x < mid.x - 3.0 && a.x > you.x, "between the camp and you: {a:?}");
        let to_mid = (mid.y - a.y).atan2(mid.x - a.x).rem_euclid(2.0 * PI);
        assert!((a.o - to_mid).abs() < 1e-4);
        // an empty place: the middle itself
        assert_eq!(clear_spot(mid, &[]), mid);
    }

    #[test]
    fn every_stamp_has_a_free_spot_near_its_middle() {
        for (name, _) in BUILTIN_STAMPS {
            let doc = read_stamp(Path::new("no-such-folder"), name).unwrap();
            let mid = At { x: 0.0, y: 0.0, z: 0.0, o: 0.0 };
            let ps: Vec<WorldObject> = doc.props.iter().map(|p| obj(p.entry as u32, p.x as f32, p.y as f32, p.z as f32, (p.yaw as f32).to_radians())).collect();
            let s = clear_spot(mid, &ps);
            assert!(!blocked(s.x, s.y, &ps, 0.0), "{name}");
            assert!((s.x * s.x + s.y * s.y).sqrt() < 6.0, "{name}: {s:?}");
        }
    }

    #[test]
    fn a_stamps_focus_is_its_own_or_the_middle_of_its_pieces() {
        let inn = read_stamp(Path::new("no-such-folder"), "inn").unwrap();
        let (x, y) = focus(&inn);
        assert!(x > 8.0 && x < 14.0 && y.abs() < 3.6, "the inn's bar, inside: {x} {y}");
        let camp = read_stamp(Path::new("no-such-folder"), "campfire").unwrap();
        assert_eq!(focus(&camp), (0.0, 0.0), "the fire");
        let plain = parse(r#"{"format":"leyline-den","version":1,"props":[{"entry":1,"x":2,"y":0},{"entry":1,"x":6,"y":-4},
            {"entry":1,"x":30,"y":0,"role":"house"}]}"#).unwrap();
        assert_eq!(focus(&plain), (4.0, -2.0));
        let bad = parse(r#"{"format":"leyline-den","version":1,"focus":[1e9,0],"props":[{"entry":1,"x":2,"y":2}]}"#).unwrap();
        assert_eq!(focus(&bad), (2.0, 2.0));
    }

    #[test]
    fn other_names_find_the_stamp() {
        for (w, n) in [("tavern", "inn"), ("Pub", "inn"), ("camp", "campfire"), ("bonfire", "campfire"), ("farm", "garden"),
                       ("pier", "dock"), ("cottage", "house"), ("forge", "smithy"), ("blacksmith's", "smithy"), ("reading", "nook"),
                       ("library", "nook"), ("altar", "shrine"), ("temple", "shrine"), ("bazaar", "market"), ("stalls", "market"),
                       ("training", "dojo"), ("smithy", "smithy"), ("shrines", "shrine")] {
            assert_eq!(canonical(w), Some(n), "{w}");
        }
        assert_eq!(canonical("castle"), None);
        // every stamp is reachable by its own name, and every name but the Dojo is a built-in stamp
        for (n, names) in NAMES {
            assert!(names.contains(n));
            assert!(*n == "dojo" || BUILTIN_STAMPS.iter().any(|s| s.0 == *n), "{n}");
        }
        for (n, _) in BUILTIN_STAMPS {
            assert_eq!(canonical(n), Some(*n));
        }
        assert!(!crate::companion::branded(&format!("{NAMES:?}")));
    }
}
