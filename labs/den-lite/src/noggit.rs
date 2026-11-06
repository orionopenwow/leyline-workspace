//! The editor's conveniences, Noggit's way (owner 5 Nov: "every other convenient and noggit feature possible build
//! right into the den"). Gizmo.lua's keys send these; each is one undo step when it changes the Den.
//!
//! Builder verbs (`wubuild cmd <verb>`, ovmod.rs `builder_cmd` sends them here first):
//!   * `dupby <dx> <dy> <dz>`: a copy of every selected prop, moved by the offset (yards), as one undo; the copies
//!     become the selection. Ctrl+D: Gizmo.lua sends the offset between the last copy and its original, so
//!     pressing it again carries on the row (Noggit's "duplicate keeps the offset").
//!   * `selnear <yards>`: every prop within that many yards of you selected (Ctrl+A), nearest first.
//!   * `slope`: every selected prop down on the ground and tilted to the ground's slope under it (L); `upright` (the
//!     Builder's own, Shift+L) stands it straight again.
//!
//! Camera helpers (gizmo.rs `cam`): `cam frame` (F) puts the far view round the selection, `cam mark` answers
//! `cammark x,y,z,deg,back` (Ctrl+1..3 keep it in the addon's saved settings) and `cam to x y z deg back` goes back
//! to one (Shift+1..3). The flight's speed rides with `cam fly ... v<yards a second>` (the wheel with the right mouse
//! held picks it).

use crate::den::{self, Den, Prop};
use crate::gizmo::{Aids, Far, FAR_MAX, FLY_MARGIN};
use crate::ovmod::{Here, Out, Sel};

/// The flight's slowest and fastest speeds (yards a second) a `v<speed>` word may ask for.
pub const FLY_MIN: f32 = 1.0;
pub const FLY_MAX: f32 = 150.0;
/// The furthest `dupby` moves a copy (yards each way), and the widest `selnear`.
pub const DUP_REACH: f64 = 50.0;
pub const NEAR_MAX: f32 = 200.0;
/// How far apart the ground is sampled for its slope (yards each side), and the steepest tilt `slope` gives.
pub const SLOPE_SAMPLE: f32 = 1.0;
pub const SLOPE_MAX_DEG: f64 = 60.0;
/// The far view round a framed selection: this many times its size back, never nearer than FRAME_MIN.
pub const FRAME_K: f32 = 2.5;
pub const FRAME_MIN: f32 = 8.0;

/// The verbs this file adds to the state's `caps=` (the editor shows their keys only when Den Lite has them).
pub const CAPS: &str = "dupby,selnear,slope,camframe,cammark,camto,flyspeed";

/// The Builder verbs answered here.
pub fn is_verb(verb: &str) -> bool {
    matches!(verb, "dupby" | "selnear" | "slope")
}

/// `cam fly`'s speed word: `v<yards a second>` (the addon's chosen speed, Shift and Alt already in it).
pub fn fly_speed(word: &str) -> Option<f32> {
    let n = word.strip_prefix('v').or_else(|| word.strip_prefix('V'))?.parse::<f32>().ok()?;
    n.is_finite().then(|| n.clamp(FLY_MIN, FLY_MAX))
}

fn num(w: &[&str], i: usize) -> Option<f64> {
    w.get(i).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite())
}

/// The selection: the first selected and the others (`more`), each once, still in the Den.
fn chosen(d: &Den, sel: &Sel) -> Vec<u64> {
    let mut v: Vec<u64> = Vec::new();
    for g in sel.guid.into_iter().chain(sel.more.iter().copied()) {
        if d.placed(g).is_some() && !v.contains(&g) {
            v.push(g);
        }
    }
    v
}

fn name_of(d: &Den, g: u64) -> &'static str {
    d.placed(g).and_then(|p| den::template(p.obj.entry)).map(|t| t.name).unwrap_or("a prop")
}

/// One of this file's Builder verbs. `can`: may change the Den.
pub fn builder(d: &mut Den, sel: &mut Sel, here: Here, text: &str, can: bool) -> Vec<Out> {
    let w: Vec<&str> = text.split_whitespace().collect();
    let verb = w.first().map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    if !can && verb != "selnear" {
        return vec![Out::Say("Only the Den's owner builds here (they can let friends: .den friends build on).".into())];
    }
    sel.round = None;
    match verb.as_str() {
        "dupby" => dup_by(d, sel, &w),
        "selnear" => sel_near(d, sel, here, &w),
        "slope" => slope(d, sel),
        _ => Vec::new(),
    }
}

/// `dupby <dx> <dy> <dz>`: the selection copied by the offset, one undo; the copies are the selection then.
fn dup_by(d: &mut Den, sel: &mut Sel, w: &[&str]) -> Vec<Out> {
    let (dx, dy, dz) = (num(w, 1).unwrap_or(1.0), num(w, 2).unwrap_or(0.0), num(w, 3).unwrap_or(0.0));
    if dx.abs() > DUP_REACH || dy.abs() > DUP_REACH || dz.abs() > DUP_REACH {
        return vec![Out::Say(format!("Duplicate moves a copy at most {DUP_REACH:.0} yards."))];
    }
    let all = chosen(d, sel);
    if all.is_empty() {
        return vec![Out::Say("Select a prop first (Near picks the closest).".into())];
    }
    let props: Vec<Prop> = all
        .iter()
        .filter_map(|g| d.placed(*g))
        .map(|p| {
            let mut q = p.prop.clone();
            q.x += dx;
            q.y += dy;
            q.z += dz;
            // a copy is never locked, and not part of the original's path or stamp
            q.locked = None;
            q.group = None;
            q
        })
        .collect();
    let mut c = d.place_many(&props);
    if c.created.is_empty() {
        return vec![Out::Say("No room for the copy there (the Den's edge, or its 800 props).".into())];
    }
    let made: Vec<u64> = c.created.iter().map(|o| o.guid).collect();
    sel.guid = made.first().copied();
    sel.more = made.iter().skip(1).copied().collect();
    let n = made.len();
    c.message = match (n, c.skipped) {
        (1, 0) => "Duplicated (Ctrl+D again carries on the row).".into(),
        (n, 0) => format!("Duplicated {n} (one undo)."),
        (n, s) => format!("Duplicated {n}; {s} didn't fit."),
    };
    vec![Out::Change(c)]
}

/// `selnear <yards>`: every shown prop within that far of you, the nearest first.
fn sel_near(d: &Den, sel: &mut Sel, here: Here, w: &[&str]) -> Vec<Out> {
    let r = num(w, 1).map(|v| v as f32).unwrap_or(30.0).clamp(1.0, NEAR_MAX);
    let mut near: Vec<(f32, u64)> = d
        .props
        .iter()
        .filter(|p| p.prop.hidden != Some(true))
        .map(|p| (((p.obj.x - here.x).powi(2) + (p.obj.y - here.y).powi(2) + (p.obj.z - here.z).powi(2)).sqrt(), p.obj.guid))
        .filter(|(dist, _)| *dist <= r)
        .collect();
    near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
    let Some(first) = near.first().map(|n| n.1) else {
        sel.guid = None;
        sel.more.clear();
        return vec![Out::Say(format!("No props within {r:.0} yards of you."))];
    };
    sel.guid = Some(first);
    sel.more = near.iter().skip(1).map(|n| n.1).collect();
    vec![Out::Say(format!("Selected {} within {r:.0} yards.", near.len()))]
}

/// The ground's slope at (x, y): how much it rises a yard east (x) and a yard north (y), from heights either side.
pub fn ground_grad(floor: impl Fn(f32, f32) -> f32, x: f32, y: f32) -> (f64, f64) {
    let h = SLOPE_SAMPLE;
    let gx = (floor(x + h, y) - floor(x - h, y)) / (2.0 * h);
    let gy = (floor(x, y + h) - floor(x, y - h)) / (2.0 * h);
    let ok = |v: f32| if v.is_finite() { v as f64 } else { 0.0 };
    (ok(gx), ok(gy))
}

/// The pitch and roll (degrees) that stand a prop facing `yaw_deg` square on ground rising (gx, gy). The game turns a
/// prop by yaw about up, then pitch about its left, then roll about its forward (raw.rs `quaternion`, AzerothCore's
/// ZYX): a positive pitch dips its nose, a positive roll lifts its left side. Its up must be the ground's normal,
/// (-gf, -gl, 1) in its own forward, left, up: so pitch = -atan(gf), roll = asin(gl / |n|).
pub fn slope_tilt(yaw_deg: f64, gx: f64, gy: f64) -> (f64, f64) {
    let (s, c) = yaw_deg.to_radians().sin_cos();
    let gf = gx * c + gy * s;
    let gl = -gx * s + gy * c;
    let n = (gf * gf + gl * gl + 1.0).sqrt();
    let pitch = (-gf).atan().to_degrees();
    let roll = (gl / n).asin().to_degrees();
    let tidy = |v: f64| {
        let v = v.clamp(-SLOPE_MAX_DEG, SLOPE_MAX_DEG);
        if v.abs() < 0.05 {
            0.0
        } else {
            (v * 10.0).round() / 10.0
        }
    };
    (tidy(pitch), tidy(roll))
}

/// `slope`: each selected prop down on the ground and tilted to its slope there; one undo for them all.
fn slope(d: &mut Den, sel: &Sel) -> Vec<Out> {
    let all = chosen(d, sel);
    if all.is_empty() {
        return vec![Out::Say("Select a prop first (Near picks the closest).".into())];
    }
    let spot_z = d.spot.z as f64;
    let mut list = Vec::new();
    for g in &all {
        let Some(p) = d.placed(*g) else { continue };
        let (x, y) = (p.obj.x, p.obj.y);
        let (gx, gy) = ground_grad(|a, b| d.floor_at(a, b), x, y);
        let (pitch, roll) = slope_tilt(p.prop.yaw, gx, gy);
        let mut q = p.prop.clone();
        q.z = d.floor_at(x, y) as f64 - spot_z;
        q.pitch = (pitch != 0.0).then_some(pitch);
        q.roll = (roll != 0.0).then_some(roll);
        list.push((*g, q));
    }
    match d.edit_each(list) {
        Ok(mut c) => {
            let n = c.created.len();
            c.message = if n == 1 {
                format!("{} lies on the ground's slope (Shift+L stands it upright).", name_of(d, all[0]))
            } else {
                format!("{n} lie on the ground's slope (one undo).")
            };
            vec![Out::Change(c)]
        }
        Err(e) => vec![Out::Say(e)],
    }
}

/// The selection's middle (its feet's average) and how far it spreads (yards), for `cam frame`.
pub fn sel_bounds(d: &Den, sel: &Sel) -> Option<([f32; 3], f32)> {
    let all = chosen(d, sel);
    if all.is_empty() {
        return None;
    }
    let objs: Vec<_> = all.iter().filter_map(|g| d.placed(*g)).map(|p| &p.obj).collect();
    let k = objs.len() as f32;
    let mid = [objs.iter().map(|o| o.x).sum::<f32>() / k, objs.iter().map(|o| o.y).sum::<f32>() / k, objs.iter().map(|o| o.z).sum::<f32>() / k];
    let r = objs
        .iter()
        .map(|o| {
            let b = crate::gizmo::body_of(o);
            let reach = b.half[0].max(b.half[1]).max(b.height / 2.0).max(1.0);
            ((o.x - mid[0]).powi(2) + (o.y - mid[1]).powi(2)).sqrt() + reach
        })
        .fold(1.0f32, f32::max);
    Some((mid, r))
}

/// `cam frame` (F): the far view round the selection, as the view faces now (the far view's, else yours).
pub fn frame(d: &Den, sel: &Sel, a: &Aids, here: Here) -> Option<Far> {
    let (mid, r) = sel_bounds(d, sel)?;
    let o = a.far.map(|f| f.focus.o).unwrap_or(here.o);
    Some(Far { focus: Here { x: mid[0], y: mid[1], z: mid[2], o }, back: (r * FRAME_K).clamp(FRAME_MIN, FAR_MAX), snap: false })
}

/// `cam mark`: where the view is (the far view's focus and distance, else over you at the build distance), as
/// `x,y,z,deg,back` for the addon to keep.
pub fn mark(d: &Den, a: &Aids, here: Here) -> String {
    let (f, back) = match a.far {
        Some(f) => (f.focus, f.back),
        None => (Here { z: d.floor_at(here.x, here.y), ..here }, crate::gizmo::BUILD_BACK),
    };
    format!("{:.2},{:.2},{:.2},{:.0},{:.1}", f.x, f.y, f.z, f.o.to_degrees().rem_euclid(360.0), back)
}

/// `cam to <x> <y> <z> <deg> <back>`: a kept view again, held near the Den (within FLY_MARGIN of its edge).
pub fn cam_to(d: &Den, w: &[&str]) -> Option<Far> {
    let n = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
    let (x, y, z, deg, back) = (n(2)?, n(3)?, n(4)?, n(5)?, n(6)?);
    let (ox, oy) = (x - d.spot.x, y - d.spot.y);
    let (dist, r) = ((ox * ox + oy * oy).sqrt(), d.spot.reach() + FLY_MARGIN);
    let (x, y) = if dist > r { (d.spot.x + ox * r / dist, d.spot.y + oy * r / dist) } else { (x, y) };
    let ground = d.floor_at(x, y);
    let z = z.clamp(ground - 5.0, ground + 250.0);
    Some(Far { focus: Here { x, y, z, o: deg.to_radians().rem_euclid(std::f32::consts::TAU) }, back: back.clamp(2.0, FAR_MAX), snap: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::den::Spot;
    use crate::ovmod::builder_cmd;

    fn here(d: &Den) -> Here {
        Here { x: d.spot.x, y: d.spot.y, z: d.spot.z, o: 0.0 }
    }

    fn placed_two(d: &mut Den, sel: &mut Sel) -> (u64, u64) {
        let h = here(d);
        builder_cmd(d, sel, h, "spawn 2413", true);
        let a = sel.guid.unwrap();
        builder_cmd(d, sel, Here { x: h.x + 6.0, ..h }, "spawn 2413", true);
        let b = sel.guid.unwrap();
        (a, b)
    }

    #[test]
    fn fly_speed_words() {
        assert_eq!(fly_speed("v24"), Some(24.0));
        assert_eq!(fly_speed("V2.5"), Some(2.5));
        assert_eq!(fly_speed("v0"), Some(FLY_MIN));
        assert_eq!(fly_speed("v9999"), Some(FLY_MAX));
        assert_eq!(fly_speed("vnan"), None);
        assert_eq!(fly_speed("1"), None, "the old fast word stays the old one's");
        assert_eq!(fly_speed("s"), None);
    }

    #[test]
    fn dupby_copies_the_selection_by_the_offset_in_one_undo() {
        let mut d = Den::new(Spot::default());
        let mut sel = Sel::default();
        let (a, b) = placed_two(&mut d, &mut sel);
        sel.guid = Some(a);
        sel.more = vec![b];
        let h = here(&d);
        let undo0 = d.undo_depth();
        let o = builder_cmd(&mut d, &mut sel, h, "dupby 2 -1 0.5", true);
        assert!(matches!(&o[0], Out::Change(c) if c.created.len() == 2 && c.message.contains("2")), "{o:?}");
        assert_eq!(d.props.len(), 4);
        assert_eq!(d.undo_depth(), undo0 + 1, "one undo");
        let (pa, pc) = (d.placed(a).unwrap().prop.clone(), d.props[2].prop.clone());
        assert!((pc.x - pa.x - 2.0).abs() < 1e-6 && (pc.y - pa.y + 1.0).abs() < 1e-6 && (pc.z - pa.z - 0.5).abs() < 1e-6);
        assert_eq!(sel.guid, Some(d.props[2].obj.guid), "the copies are the selection");
        assert_eq!(sel.more, vec![d.props[3].obj.guid], "both of them (the cleanup after a verb keeps them)");
        d.undo();
        assert_eq!(d.props.len(), 2, "one undo takes both copies");
        // nothing selected, a wild offset, a visitor
        let mut none = Sel::default();
        assert!(matches!(&builder_cmd(&mut d, &mut none, h, "dupby 1 0 0", true)[0], Out::Say(s) if s.contains("Select")));
        sel.guid = Some(a);
        assert!(matches!(&builder_cmd(&mut d, &mut sel, h, "dupby 500 0 0", true)[0], Out::Say(s) if s.contains("at most")));
        assert!(matches!(&builder_cmd(&mut d, &mut sel, h, "dupby 1 0 0", false)[0], Out::Say(s) if s.contains("owner")));
        assert_eq!(d.props.len(), 2);
    }

    #[test]
    fn dupby_copy_is_unlocked_and_ungrouped() {
        let mut d = Den::new(Spot::default());
        let mut sel = Sel::default();
        let h = here(&d);
        builder_cmd(&mut d, &mut sel, h, "spawn 2413", true);
        builder_cmd(&mut d, &mut sel, h, "lock", true);
        builder_cmd(&mut d, &mut sel, h, "dupby 1 0 0", true);
        assert_eq!(d.props.len(), 2);
        assert_eq!(d.props[1].prop.locked, None);
    }

    #[test]
    fn selnear_takes_what_is_within_reach_nearest_first() {
        let mut d = Den::new(Spot::default());
        let mut sel = Sel::default();
        let (a, b) = placed_two(&mut d, &mut sel);
        let h = here(&d);
        let o = builder_cmd(&mut d, &mut sel, h, "selnear 30", false);
        assert!(matches!(&o[0], Out::Say(s) if s.contains("Selected 2")), "{o:?}: a visitor may select");
        assert_eq!((sel.guid, sel.more.clone()), (Some(a), vec![b]), "the nearest first");
        let o = builder_cmd(&mut d, &mut sel, Here { x: h.x + 2.0, ..h }, "selnear 5", true);
        assert!(matches!(&o[0], Out::Say(s) if s.contains("Selected 1")), "{o:?}");
        assert!(sel.more.is_empty());
        let o = builder_cmd(&mut d, &mut sel, Here { x: h.x - 100.0, ..h }, "selnear 5", true);
        assert!(matches!(&o[0], Out::Say(s) if s.contains("No props")));
        assert_eq!(sel.guid, None);
    }

    #[test]
    fn slope_tilt_follows_the_ground() {
        // flat: upright
        assert_eq!(slope_tilt(0.0, 0.0, 0.0), (0.0, 0.0));
        // rising to the east (1 in 1), facing east: the nose goes up 45 degrees (a negative pitch), no roll
        let (p, r) = slope_tilt(0.0, 1.0, 0.0);
        assert!((p + 45.0).abs() < 0.1 && r == 0.0, "{p} {r}");
        // the same ground, facing north: east is its right, so the right side is high and the left low (roll < 0)
        let (p, r) = slope_tilt(90.0, 1.0, 0.0);
        assert!(p.abs() < 0.1 && (r + 45.0).abs() < 0.1, "{p} {r}");
        // facing west: the nose dips
        let (p, _) = slope_tilt(180.0, 1.0, 0.0);
        assert!((p - 45.0).abs() < 0.1);
        // a cliff is held to the steepest tilt
        let (p, _) = slope_tilt(0.0, 100.0, 0.0);
        assert_eq!(p, -SLOPE_MAX_DEG);
        // the gradient from a height function
        let (gx, gy) = ground_grad(|x, y| 0.5 * x - 0.25 * y, 10.0, 10.0);
        assert!((gx - 0.5).abs() < 1e-5 && (gy + 0.25).abs() < 1e-5);
        let (gx, gy) = ground_grad(|_, _| f32::NAN, 0.0, 0.0);
        assert_eq!((gx, gy), (0.0, 0.0), "no heights: flat");
    }

    #[test]
    fn slope_on_a_flat_den_lands_and_straightens_in_one_undo() {
        let mut d = Den::new(Spot::default());
        let mut sel = Sel::default();
        let (a, b) = placed_two(&mut d, &mut sel);
        let h = here(&d);
        sel.guid = Some(a);
        sel.more = vec![b];
        builder_cmd(&mut d, &mut sel, h, "move z 2", true);
        builder_cmd(&mut d, &mut sel, h, "tilt pitch 20", true);
        let undo0 = d.undo_depth();
        let o = builder_cmd(&mut d, &mut sel, h, "slope", true);
        assert!(matches!(&o[0], Out::Change(c) if c.created.len() == 2), "{o:?}");
        assert_eq!(d.undo_depth(), undo0 + 1, "one undo");
        for g in [a, b] {
            let p = &d.placed(g).unwrap().prop;
            assert!(p.z.abs() < 1e-3 && p.pitch.is_none() && p.roll.is_none(), "{p:?}");
        }
        assert_eq!(sel.more, vec![b], "the selection kept");
    }

    #[test]
    fn frame_mark_and_to() {
        let mut d = Den::new(Spot::default());
        let mut sel = Sel::default();
        let (a, b) = placed_two(&mut d, &mut sel);
        let h = here(&d);
        let a_ = Aids::default();
        assert!(frame(&d, &Sel::default(), &a_, h).is_none(), "nothing selected: nothing to frame");
        sel.guid = Some(a);
        sel.more = vec![b];
        let f = frame(&d, &sel, &a_, h).unwrap();
        let (pa, pb) = (d.placed(a).unwrap().obj.clone(), d.placed(b).unwrap().obj.clone());
        assert!((f.focus.x - (pa.x + pb.x) / 2.0).abs() < 1e-3, "the middle of the two");
        assert!(f.back >= FRAME_MIN && f.back > 4.0 * FRAME_K, "back far enough to see both: {}", f.back);
        // a mark near you, then to it again
        let m = mark(&d, &a_, h);
        let w: Vec<String> = std::iter::once("cam".to_string()).chain(std::iter::once("to".to_string())).chain(m.split(',').map(|s| s.to_string())).collect();
        let w: Vec<&str> = w.iter().map(|s| s.as_str()).collect();
        let f = cam_to(&d, &w).unwrap();
        assert!((f.focus.x - h.x).abs() < 0.01 && (f.back - crate::gizmo::BUILD_BACK).abs() < 0.1);
        // far off: held near the Den; nonsense: none
        let f = cam_to(&d, &["cam", "to", "99999", "16257", "13", "0", "40"]).unwrap();
        assert!((f.focus.x - d.spot.x - d.spot.reach() - FLY_MARGIN).abs() < 0.5);
        assert!(cam_to(&d, &["cam", "to", "1", "2"]).is_none());
        assert!(cam_to(&d, &["cam", "to", "nan", "2", "3", "4", "5"]).is_none());
    }

    #[test]
    fn the_camera_verbs_through_gizmo() {
        use crate::gizmo::{cmd, Aids};
        let mut d = Den::new(Spot::default());
        let mut sel = Sel::default();
        let (a, _) = placed_two(&mut d, &mut sel);
        let h = here(&d);
        let mut aids = Aids::default();
        sel.guid = Some(a);
        sel.more.clear();
        let out = cmd(&mut d, &mut sel, &mut aids, h, "cam frame", true, 1000);
        let f = aids.far.expect("the far view round the selection");
        let pa = d.placed(a).unwrap().obj.clone();
        assert!((f.focus.x - pa.x).abs() < 1e-3 && (f.focus.y - pa.y).abs() < 1e-3, "{f:?}");
        assert!(out.addon.iter().any(|x| x.0 == "cam" && x.1.starts_with("far,")), "{:?}", out.addon);
        let out = cmd(&mut d, &mut sel, &mut aids, h, "cam mark", true, 1100);
        let m = out.addon.iter().find(|x| x.0 == "cammark").map(|x| x.1.clone()).expect("cammark");
        assert_eq!(m.split(',').count(), 5, "{m}");
        cmd(&mut d, &mut sel, &mut aids, h, "cam off", true, 1200);
        assert!(aids.far.is_none());
        let back: Vec<&str> = m.split(',').collect();
        cmd(&mut d, &mut sel, &mut aids, h, &format!("cam to {}", back.join(" ")), true, 1300);
        let g = aids.far.expect("back to the mark");
        assert!((g.focus.x - f.focus.x).abs() < 0.02 && (g.back - f.back).abs() < 0.1);
        let mut empty = Sel::default();
        let out = cmd(&mut d, &mut empty, &mut Aids::default(), h, "cam frame", true, 1400);
        assert!(out.say.iter().any(|s| s.contains("Select a prop")), "{:?}", out.say);
        // the next placement's turn set outright (Ctrl+V: the copied prop's turn)
        let out = cmd(&mut d, &mut sel, &mut aids, h, "yaw set 90", true, 1500);
        assert!(out.addon.contains(&("yaw".to_string(), "90".to_string())), "{:?}", out.addon);
        let out = cmd(&mut d, &mut sel, &mut aids, h, "yaw 15", true, 1600);
        assert!(out.addon.contains(&("yaw".to_string(), "105".to_string())), "relative still: {:?}", out.addon);
        // the far view's flight at the addon's own speed (the far view is on again: `cam to`)
        assert!(aids.far.is_some() && aids.far_at.is_some());
        let out = cmd(&mut d, &mut sel, &mut aids, h, "cam fly 1 0 0 0 v4", true, 2000);
        assert!(out.far.is_some());
        let slow = aids.fly_v;
        let out = cmd(&mut d, &mut sel, &mut aids, h, "cam fly 1 0 0 0 v40", true, 2100);
        assert!(out.far.is_some());
        let n = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        assert!((n(slow) - 4.0).abs() < 0.01 && (n(aids.fly_v) - 40.0).abs() < 0.01, "{slow:?} {:?}", aids.fly_v);
    }
}
