//! The builder's comforts (docs/DEN-NEXT.md, "The builder's comforts", 12 Nov 2026). Owner: "creates and suggests
//! features for the den and makes it more user intuitive, adding quality of life features and everything, including
//! Noggit and the editor that follows terrain, click and drag to move". The geometry is std-only `qolmath.rs`.
//!
//! Builder verbs (`wubuild cmd <verb>`, ovmod.rs `builder_cmd` sends them here first):
//!   * `fill gap`: copies of the first selected prop in the gap between it and the second, as many as fit, on the
//!     ground (each keeps the first's height over the ground and its turn); one undo; the copies selected.
//!   * `fill box [<step>]`: the box the first two selected props make (opposite corners) filled with copies of the
//!     first, every `<step>` yards (its own footprint when not given); one undo.
//!
//! Aid verbs (gizmo.rs `cmd`; they need the camera or what is in hand):
//!   * `nudge <fwd> <right> <up> [<turn degrees>]`: the selection moved relative to the view, along the grid's axes
//!     (qolmath `nudge_offset`); a level nudge keeps each prop's height over the ground (it follows hills); one undo.
//!   * `selcircle <u0> <v0> <u1> <v1> <aspect> [<zoom>] [add]`: Alt+drag: every prop whose foot is in the circle on
//!     the ground round where the press was, out to where the let-go is; `add` keeps what was selected.
//!   * `gizmo slope on|off`: props placed or moved lie on the ground's slope (noggit.rs `slope_tilt`), the copy in
//!     hand leaning too where the game draws it as an object.
//!   * `gizmo magnet on|off`: in hand near another prop of its kind, the copy snaps edge to edge with it.
//!   * `hover ... free` (Alt held): no grid and no magnet for that hover (the precise spot).
//!
//! Group move (gizmo.rs `drag` / `commit`): pressing on one prop of a multi-selection and dragging it (or a click
//! picking it up) carries the whole selection: on the let-go every other selected prop keeps its offset from it, turned
//! with it, at its own height over the ground where it lands; one undo for them all.

use crate::den::{self, Den, Prop};
use crate::gizmo::{self, AidOut, Aids};
use crate::ovmod::{Here, Out, Sel};
use crate::qolmath;

/// The verbs this file adds to the state's `caps=` (the editor shows their keys only when Den Lite has them).
pub const CAPS: &str = "groupmove,slopefollow,magnet,nudge,selcircle,fillgap,fillbox,free";

/// The farthest one nudge moves (yards each way) and turns (degrees).
pub const NUDGE_MAX: f32 = 10.0;
pub const NUDGE_TURN_MAX: f32 = 90.0;

/// The Builder verbs answered here.
pub fn is_verb(verb: &str) -> bool {
    verb == "fill"
}

/// The aid verbs answered here (gizmo.rs `is_aid` lists them too).
pub fn is_aid(verb: &str) -> bool {
    matches!(verb, "nudge" | "selcircle")
}

fn num(w: &[&str], i: usize) -> Option<f32> {
    w.get(i).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite())
}

/// The selection: the first selected and the others, each once, still in the Den.
pub fn chosen(d: &Den, sel: &Sel) -> Vec<u64> {
    let mut v: Vec<u64> = Vec::new();
    for g in sel.guid.into_iter().chain(sel.more.iter().copied()) {
        if d.placed(g).is_some() && !v.contains(&g) {
            v.push(g);
        }
    }
    v
}

const OWNER_ONLY: &str = "Only the Den's owner builds here (they can let friends: .den friends build on).";

/// One of this file's Builder verbs.
pub fn builder(d: &mut Den, sel: &mut Sel, _here: Here, text: &str, can: bool) -> Vec<Out> {
    let w: Vec<&str> = text.split_whitespace().collect();
    if !can {
        return vec![Out::Say(OWNER_ONLY.into())];
    }
    match w.get(1).map(|v| v.to_ascii_lowercase()).as_deref() {
        Some("gap") => fill(d, sel, None, true),
        Some("box") => fill(d, sel, num(&w, 2), false),
        _ => vec![Out::Say("Usage: fill gap | fill box [<step yards>] (select two props first)".into())],
    }
}

/// `fill gap` / `fill box`: copies of the first selected between it and the second.
fn fill(d: &mut Den, sel: &mut Sel, step: Option<f32>, gap: bool) -> Vec<Out> {
    let all = chosen(d, sel);
    if all.len() < 2 {
        return vec![Out::Say(if gap {
            "Select two props (Shift+click the second): Fill gap puts copies of the first between them.".into()
        } else {
            "Select two props at opposite corners (Shift+click the second): Fill box covers the box between them.".into()
        })];
    }
    let (Some(a), Some(b)) = (d.placed(all[0]).cloned(), d.placed(all[1]).cloned()) else { return Vec::new() };
    let (hx, hy, _) = gizmo::extent(&a.obj);
    let spots = if gap {
        let dir = (b.obj.y - a.obj.y).atan2(b.obj.x - a.obj.x);
        let width = 2.0 * qolmath::half_along([hx, hy], a.obj.o, dir);
        qolmath::fill_gap([a.obj.x, a.obj.y], [b.obj.x, b.obj.y], width)
    } else {
        // the footprint on the world's axes, as the prop is turned (gizmo.rs `world_extent`)
        let (wx, wy, _) = gizmo::world_extent(&a.obj);
        let st = match step.filter(|s| *s > 0.0) {
            Some(s) => [s, s],
            None => [2.0 * wx, 2.0 * wy],
        };
        match qolmath::fill_box([a.obj.x, a.obj.y], [b.obj.x, b.obj.y], st) {
            Some(v) => v,
            None => return vec![Out::Say(format!("That box needs more than {} copies: a bigger step, or a smaller box.", qolmath::FILL_MAX))],
        }
    };
    if spots.is_empty() {
        return vec![Out::Say("No room for a copy between those two.".into())];
    }
    let lift = a.obj.z - d.floor_at(a.obj.x, a.obj.y);
    let spot = d.spot;
    let props: Vec<Prop> = spots
        .iter()
        .map(|p| {
            let mut q = a.prop.clone();
            q.x = (p[0] - spot.x) as f64;
            q.y = (p[1] - spot.y) as f64;
            q.z = (d.floor_at(p[0], p[1]) + lift - spot.z) as f64;
            q.locked = None;
            q.group = None;
            q.role = None;
            q
        })
        .collect();
    let mut c = d.place_many(&props);
    if c.created.is_empty() {
        return vec![Out::Say("No room for the copies (the Den's edge, or its 800 props).".into())];
    }
    let made: Vec<u64> = c.created.iter().map(|o| o.guid).collect();
    sel.guid = made.first().copied();
    sel.more = made.iter().skip(1).copied().collect();
    let name = den::template(a.obj.entry).map(|t| t.name).unwrap_or("prop");
    c.message = match c.skipped {
        0 => format!("Filled with {} {name} (one undo).", made.len()),
        s => format!("Filled with {} {name}; {s} didn't fit.", made.len()),
    };
    vec![Out::Change(c)]
}

/// The tilt (degrees: pitch, roll) a prop facing `yaw_deg` gets on the ground at (x, y) (noggit.rs `slope_tilt`).
pub fn tilt_at(d: &Den, x: f32, y: f32, yaw_deg: f64) -> (Option<f64>, Option<f64>) {
    let (gx, gy) = crate::noggit::ground_grad(|a, b| d.floor_at(a, b), x, y);
    let (p, r) = crate::noggit::slope_tilt(yaw_deg, gx, gy);
    ((p != 0.0).then_some(p), (r != 0.0).then_some(r))
}

/// `nudge <fwd> <right> <up> [<turn>]`: the selection moved by the offset relative to the view (on the grid's axes),
/// one undo; a level nudge keeps each prop's height over the ground.
pub fn nudge(d: &mut Den, sel: &mut Sel, a: &mut Aids, here: Here, w: &[&str], can: bool) -> AidOut {
    let mut out = AidOut::default();
    if !can {
        out.say.push(OWNER_ONLY.into());
        return out;
    }
    let f = |i| num(w, i).unwrap_or(0.0).clamp(-NUDGE_MAX, NUDGE_MAX);
    let (fwd, right, up) = (f(1), f(2), f(3));
    let turn = num(w, 4).unwrap_or(0.0).clamp(-NUDGE_TURN_MAX, NUDGE_TURN_MAX) as f64;
    let all = chosen(d, sel);
    if all.is_empty() {
        out.say.push("Select a prop first: the nudge keys move the selection.".into());
        return out;
    }
    // the view's facing: the far view's, else the player's (the build camera looks the way they face)
    let (from, _) = a.view(here);
    let off = qolmath::nudge_offset(from.o, fwd, right, up);
    let spot = d.spot;
    let mut list = Vec::new();
    for g in &all {
        let Some(p) = d.placed(*g) else { continue };
        let (x0, y0) = (p.obj.x, p.obj.y);
        let (x1, y1) = (x0 + off[0], y0 + off[1]);
        let lift = p.obj.z - d.floor_at(x0, y0);
        let z1 = d.floor_at(x1, y1) + lift + off[2];
        let mut q = p.prop.clone();
        q.x = (x1 - spot.x) as f64;
        q.y = (y1 - spot.y) as f64;
        q.z = (z1 - spot.z) as f64;
        q.yaw = (q.yaw + turn).rem_euclid(360.0);
        list.push((*g, q));
    }
    match d.edit_each(list) {
        Ok(mut c) => {
            c.message = String::new();
            out.change = Some(c);
            out.state = true;
            gizmo::remade(a);
            out.merge(gizmo::sync(d, sel, a));
            for m in &sel.more {
                out.glow.push((*m, gizmo::GLOW));
            }
            out.addon.push(("nudged".into(), format!("{:.2},{:.2},{:.2},{turn:.0}", off[0], off[1], off[2])));
            out
        }
        Err(e) => {
            out.say.push(e);
            out
        }
    }
}

/// `selcircle <u0> <v0> <u1> <v1> <aspect> [<zoom>] [add]`: the props whose foot is in the circle on the ground
/// round the press's spot out to the let-go's, the nearest the middle first.
pub fn sel_circle(d: &Den, sel: &mut Sel, a: &mut Aids, here: Here, w: &[&str]) -> AidOut {
    let mut out = AidOut { state: true, ..Default::default() };
    let (Some(u0), Some(v0), Some(u1), Some(v1)) = (num(w, 1), num(w, 2), num(w, 3), num(w, 4)) else {
        out.say.push("Usage: selcircle <u0> <v0> <u1> <v1> <aspect> [<zoom>] [add]".into());
        return out;
    };
    let aspect = num(w, 5).filter(|x| *x > 0.0).unwrap_or(16.0 / 9.0);
    if let Some(z) = num(w, 6).filter(|z| (0.0..=60.0).contains(z)) {
        a.zoom = Some(z);
    }
    let add = w.iter().skip(5).any(|x| x.eq_ignore_ascii_case("add"));
    let (from, cam) = a.view(here);
    let ground = |u: f32, v: f32| {
        gizmo::ray(&cam, from, u, v, aspect).and_then(|(e, dir)| crate::pick::on_ground(e, dir, cam.dist + gizmo::REACH, |x, y| d.floor_at(x, y)))
    };
    let (Some(c), Some(e)) = (ground(u0, v0), ground(u1, v1)) else {
        out.say.push("Circle select: start and end on the ground.".into());
        return out;
    };
    let r = ((e[0] - c[0]).powi(2) + (e[1] - c[1]).powi(2)).sqrt();
    let mut hits: Vec<(f32, u64)> = d
        .props
        .iter()
        .filter(|p| p.prop.hidden != Some(true) && qolmath::in_circle([c[0], c[1]], r, [p.obj.x, p.obj.y]))
        .map(|p| ((p.obj.x - c[0]).powi(2) + (p.obj.y - c[1]).powi(2), p.obj.guid))
        .collect();
    hits.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal).then(x.1.cmp(&y.1)));
    let mut now: Vec<u64> = if add { chosen(d, sel) } else { Vec::new() };
    for (_, g) in &hits {
        if !now.contains(g) {
            now.push(*g);
        }
    }
    // the glow of what was selected and isn't any more
    for g in chosen(d, sel) {
        if !now.contains(&g) {
            out.glow.push((g, 0));
        }
    }
    sel.guid = now.first().copied();
    sel.more = now.iter().skip(1).copied().collect();
    sel.round = None;
    for g in now.iter().skip(1) {
        out.glow.push((*g, gizmo::GLOW));
    }
    out.say.push(match (hits.len(), now.len()) {
        (0, 0) => format!("Nothing in that circle ({r:.1} yards across the middle)."),
        (h, n) if add => format!("{h} in the circle; {n} selected."),
        (h, _) => format!("{h} selected in the circle."),
    });
    gizmo::remade(a);
    out.merge(gizmo::sync(d, sel, a));
    out.addon.push(("selected".into(), sel.guid.map(|g| (g & 0xFF_FFFF).to_string()).unwrap_or_else(|| "-".into())));
    out
}

/// The other selected props carried with the one in hand (`main`), when it went from `from` to `to` (x, y, z,
/// facing): each keeps its offset (turned with it) and its height over the ground where it lands; with `slope`, each
/// lies on the ground's slope there. (guid, its new prop) for [`Den::edit_each`].
pub fn group_moves(d: &Den, main: u64, others: &[u64], from: [f32; 4], to: [f32; 4], slope: bool) -> Vec<(u64, Prop)> {
    let spot = d.spot;
    let mut v = Vec::new();
    for g in others {
        if *g == main {
            continue;
        }
        let Some(p) = d.placed(*g) else { continue };
        if p.prop.locked == Some(true) {
            continue;
        }
        let (xy, facing) = qolmath::group_place(from, to, [p.obj.x, p.obj.y], p.obj.o);
        let lift = p.obj.z - d.floor_at(p.obj.x, p.obj.y);
        let z = d.floor_at(xy[0], xy[1]) + lift;
        let mut q = p.prop.clone();
        q.x = (xy[0] - spot.x) as f64;
        q.y = (xy[1] - spot.y) as f64;
        q.z = (z - spot.z) as f64;
        q.yaw = (facing.to_degrees() as f64).rem_euclid(360.0);
        if slope {
            let (pi, ro) = tilt_at(d, xy[0], xy[1], q.yaw);
            q.pitch = pi;
            q.roll = ro;
        }
        v.push((*g, q));
    }
    v
}

/// The magnet's neighbours for what is in hand: every shown prop of the same entry (not the one carried, nor the
/// group carried with it), near enough.
pub fn magnet_spot(d: &Den, entry: u32, skip: &[u64], want: [f32; 2], half: [f32; 2]) -> Option<([f32; 2], f32)> {
    let near: Vec<qolmath::Near> = d
        .props
        .iter()
        .filter(|p| p.obj.entry == entry && p.prop.hidden != Some(true) && !skip.contains(&p.obj.guid))
        .filter(|p| (p.obj.x - want[0]).abs() < qolmath::MAGNET_LOOK && (p.obj.y - want[1]).abs() < qolmath::MAGNET_LOOK)
        .map(|p| {
            let (hx, hy, _) = gizmo::extent(&p.obj);
            qolmath::Near { at: [p.obj.x, p.obj.y], facing: p.obj.o, half: [hx, hy] }
        })
        .collect();
    qolmath::magnet(want, half, &near)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::den::Spot;
    use crate::gizmo::{cmd, screen, Armed};
    use crate::ovmod::builder_cmd;

    fn den() -> (Den, Here) {
        let d = Den::new(Spot::default());
        let s = d.spot;
        (d, Here { x: s.x, y: s.y, z: s.z, o: 0.0 })
    }

    fn line(u: f32, v: f32) -> String {
        format!("{u:.4} {v:.4} 1.7778 12")
    }

    fn uv(a: &Aids, here: Here, p: [f32; 3]) -> [f32; 2] {
        let (from, cam) = a.view(here);
        screen(&cam, from, p, 1.7778).expect("on the screen")
    }

    #[test]
    fn the_verbs_and_caps() {
        assert!(is_verb("fill") && !is_verb("dupby"));
        assert!(is_aid("nudge") && is_aid("selcircle") && !is_aid("hover"));
        assert!(gizmo::is_aid("nudge 1 0 0") && gizmo::is_aid("selcircle 0 0 0.1 0.1 1.7"));
        let (d, _) = den();
        let st = crate::ovmod::state(&d, &Sel::default(), "owner");
        for v in CAPS.split(',') {
            assert!(st.contains(v), "{v} in the state's caps");
        }
    }

    #[test]
    fn dragging_one_of_a_selection_carries_them_all_in_one_undo() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let table = d.spawn(2413, here.x + 6.0, here.y, here.z, 0.0).unwrap();
        let chair = d.spawn(2413, here.x + 8.0, here.y, here.z, 0.0).unwrap();
        let stool = d.spawn(2413, here.x + 6.0, here.y + 3.0, here.z, 0.0).unwrap();
        cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        sel.guid = Some(table.guid);
        sel.more = vec![chair.guid, stool.guid];
        // press on the CHAIR (one of the extras) and drag: it is the one in hand, the others stay selected
        let [u, v] = uv(&a, here, [chair.x, chair.y, chair.z + 0.5]);
        cmd(&mut d, &mut sel, &mut a, here, &format!("drag {}", line(u, v)), true, 100);
        assert_eq!(a.armed, Some(Armed::Move(chair.guid)));
        assert_eq!(sel.guid, Some(chair.guid));
        let mut more = sel.more.clone();
        more.sort();
        let mut want = vec![table.guid, stool.guid];
        want.sort();
        assert_eq!(more, want, "the table and the stool still selected");
        // it goes 3 yards north and turns a quarter left
        a.at = Some([chair.x, chair.y + 3.0, chair.z, std::f32::consts::FRAC_PI_2]);
        a.yaw = std::f32::consts::FRAC_PI_2;
        let undo = d.undo_depth();
        let out = cmd(&mut d, &mut sel, &mut a, here, "commit", true, 400);
        assert!(out.change.is_some(), "{:?}", out.say);
        assert_eq!(d.undo_depth(), undo + 1, "one undo for the group");
        let at = |g: u64| d.placed(g).unwrap().obj.clone();
        let c = at(chair.guid);
        assert!((c.x - chair.x).abs() < 0.02 && (c.y - (chair.y + 3.0)).abs() < 0.02);
        // the table was 2 yards west of the chair: turned a quarter left with it, it is 2 yards south of it now
        let t = at(table.guid);
        assert!((t.x - c.x).abs() < 0.02 && (t.y - (c.y - 2.0)).abs() < 0.02, "{t:?} {c:?}");
        assert!((t.o - std::f32::consts::FRAC_PI_2).abs() < 0.01, "turned with it");
        // the stool was 2 west and 3 north: now 3 west and 2 south
        let s = at(stool.guid);
        assert!((s.x - (c.x - 3.0)).abs() < 0.02 && (s.y - (c.y - 2.0)).abs() < 0.02, "{s:?}");
        assert!(sel.guid == Some(chair.guid) && sel.more.len() == 2, "still all selected");
        // one undo puts all three back
        d.undo();
        assert!((at(table.guid).x - table.x).abs() < 0.02 && (at(stool.guid).y - stool.y).abs() < 0.02 && (at(chair.guid).y - chair.y).abs() < 0.02);
    }

    #[test]
    fn a_locked_extra_stays_and_a_single_drag_is_as_before() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let one = d.spawn(2413, here.x + 6.0, here.y, here.z, 0.0).unwrap();
        let two = d.spawn(2413, here.x + 9.0, here.y, here.z, 0.0).unwrap();
        d.set_lock(two.guid, true).unwrap();
        cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        sel.guid = Some(one.guid);
        sel.more = vec![two.guid];
        let [u, v] = uv(&a, here, [one.x, one.y, one.z + 0.5]);
        cmd(&mut d, &mut sel, &mut a, here, &format!("drag {}", line(u, v)), true, 100);
        a.at = Some([one.x + 1.0, one.y, one.z, 0.0]);
        cmd(&mut d, &mut sel, &mut a, here, "commit", true, 200);
        assert!((d.placed(one.guid).unwrap().obj.x - (one.x + 1.0)).abs() < 0.02);
        assert!((d.placed(two.guid).unwrap().obj.x - two.x).abs() < 0.02, "locked: it stays");
        // a drag of a prop not in the selection: it alone, the selection is it
        let three = d.spawn(2413, here.x + 6.0, here.y + 4.0, here.z, 0.0).unwrap();
        sel.guid = Some(one.guid);
        sel.more = vec![two.guid];
        let [u, v] = uv(&a, here, [three.x, three.y, three.z + 0.5]);
        cmd(&mut d, &mut sel, &mut a, here, &format!("drag {}", line(u, v)), true, 300);
        assert!(sel.guid == Some(three.guid) && sel.more.is_empty());
    }

    #[test]
    fn slope_follow_tilts_what_is_placed_on_a_hill_and_flat_ground_stays_upright() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        let out = cmd(&mut d, &mut sel, &mut a, here, "gizmo slope on", true, 0);
        assert!(a.slope_on && out.addon.contains(&("slope".into(), "on".into())));
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 10);
        a.at = Some([here.x + 5.0, here.y, here.z, 0.0]);
        let undo = d.undo_depth();
        cmd(&mut d, &mut sel, &mut a, here, "commit", true, 20);
        assert_eq!(d.undo_depth(), undo + 1, "placing with the tilt is one undo");
        let p = d.props.last().unwrap();
        assert_eq!((p.prop.pitch, p.prop.roll), (None, None), "flat: upright");
        assert_eq!(tilt_at(&d, here.x, here.y, 0.0), (None, None));
        let off = cmd(&mut d, &mut sel, &mut a, here, "gizmo slope off", true, 30);
        assert!(!a.slope_on && off.addon.contains(&("slope".into(), "off".into())));
    }

    #[test]
    fn the_magnet_snaps_a_card_edge_to_edge_and_alt_frees_it() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let first = d.spawn(2413, here.x + 6.0, here.y, here.z, 0.0).unwrap();
        let (hx, _, _) = gizmo::extent(&first);
        cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        let out = cmd(&mut d, &mut sel, &mut a, here, "gizmo magnet on", true, 0);
        assert!(a.magnet_on && out.addon.contains(&("magnet".into(), "on".into())));
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 10);
        // the cursor a little off the spot in front of the first chair
        let want = [first.x + 2.0 * hx + 0.3, first.y + 0.2, first.z];
        let [u, v] = uv(&a, here, want);
        cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(u, v)), true, 100);
        let at = a.at.unwrap();
        assert!((at[0] - (first.x + 2.0 * hx)).abs() < 0.02 && (at[1] - first.y).abs() < 0.02, "snapped: {at:?}");
        assert!(a.snapped, "the addon's pill says it snapped");
        // Alt held (`free`): the cursor's own spot
        cmd(&mut d, &mut sel, &mut a, here, &format!("hover {} free", line(u, v)), true, 200);
        let at = a.at.unwrap();
        assert!((at[0] - want[0]).abs() < 0.1 && (at[1] - want[1]).abs() < 0.1, "free: {at:?}");
        assert!(!a.snapped);
        // and the grid is off for it too
        sel.grid = 2.0;
        cmd(&mut d, &mut sel, &mut a, here, &format!("hover {} free", line(u, v)), true, 300);
        let at = a.at.unwrap();
        assert!((at[0] - want[0]).abs() < 0.1, "no grid while free: {at:?}");
        assert_eq!(sel.grid, 2.0, "the grid is kept");
    }

    #[test]
    fn fill_gap_and_fill_box_make_copies_in_one_undo() {
        let (mut d, here) = den();
        let mut sel = Sel::default();
        let a = d.spawn(2413, here.x, here.y + 5.0, here.z, 0.0).unwrap();
        let b = d.spawn(2413, here.x + 8.0, here.y + 5.0, here.z, 0.0).unwrap();
        // one selected: says what to do
        sel.guid = Some(a.guid);
        assert!(matches!(&builder_cmd(&mut d, &mut sel, here, "fill gap", true)[0], Out::Say(s) if s.contains("two")));
        sel.more = vec![b.guid];
        let undo = d.undo_depth();
        let o = builder_cmd(&mut d, &mut sel, here, "fill gap", true);
        let Out::Change(c) = &o[0] else { panic!("{o:?}") };
        assert!(!c.created.is_empty() && c.message.contains("one undo"), "{c:?}");
        assert_eq!(d.undo_depth(), undo + 1);
        assert!(c.created.iter().all(|o| o.x > a.x && o.x < b.x && (o.y - a.y).abs() < 0.01), "between them, on the line");
        assert_eq!(sel.guid, Some(c.created[0].guid), "the copies are the selection");
        let n = c.created.len();
        d.undo();
        assert_eq!(d.props.len(), 2, "one undo takes all {n}");
        // the box: a 4 by 4 square every 2 yards: 9 spots less the two corners
        let c2 = d.spawn(2413, here.x + 4.0, here.y + 9.0, here.z, 0.0).unwrap();
        sel.guid = Some(a.guid);
        sel.more = vec![c2.guid];
        let o = builder_cmd(&mut d, &mut sel, here, "fill box 2", true);
        let Out::Change(c) = &o[0] else { panic!("{o:?}") };
        assert_eq!(c.created.len(), 7, "{c:?}");
        // too big a box: refused, nothing made
        let before = d.props.len();
        sel.guid = Some(a.guid);
        sel.more = vec![c2.guid];
        let o = builder_cmd(&mut d, &mut sel, here, "fill box 0.25", true);
        assert!(matches!(&o[0], Out::Say(s) if s.contains("more than")), "{o:?}");
        assert_eq!(d.props.len(), before);
        // a visitor
        assert!(matches!(&builder_cmd(&mut d, &mut sel, here, "fill gap", false)[0], Out::Say(s) if s.contains("owner")));
    }

    #[test]
    fn the_nudge_moves_the_selection_on_the_view_axes_in_one_undo() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let one = d.spawn(2413, here.x + 3.0, here.y, here.z, 0.0).unwrap();
        let two = d.spawn(2413, here.x + 5.0, here.y, here.z, 0.0).unwrap();
        sel.guid = Some(one.guid);
        sel.more = vec![two.guid];
        let undo = d.undo_depth();
        // the player faces east (o = 0): forward is +x, right is -y
        let out = cmd(&mut d, &mut sel, &mut a, here, "nudge 1 0.5 0.25 15", true, 0);
        assert!(out.change.is_some() && out.addon.iter().any(|(n, _)| n == "nudged"), "{out:?}");
        assert_eq!(d.undo_depth(), undo + 1);
        let p = d.placed(one.guid).unwrap().obj.clone();
        assert!((p.x - (one.x + 1.0)).abs() < 0.01 && (p.y - (one.y - 0.5)).abs() < 0.01 && (p.z - (one.z + 0.25)).abs() < 0.01, "{p:?}");
        assert!((p.o - 15f32.to_radians()).abs() < 0.01);
        assert!((d.placed(two.guid).unwrap().obj.x - (two.x + 1.0)).abs() < 0.01, "the whole selection");
        // facing north: forward is +y
        let north = Here { o: std::f32::consts::FRAC_PI_2, ..here };
        let before = d.placed(one.guid).unwrap().obj.clone();
        cmd(&mut d, &mut sel, &mut a, north, "nudge 2 0 0", true, 10);
        let p = d.placed(one.guid).unwrap().obj.clone();
        assert!((p.y - (before.y + 2.0)).abs() < 0.01 && (p.x - before.x).abs() < 0.01, "{p:?}");
        // a wild nudge is held to the most; nothing selected; a visitor
        cmd(&mut d, &mut sel, &mut a, here, "nudge 500 0 0", true, 20);
        let q = d.placed(one.guid).unwrap().obj.clone();
        assert!((q.x - (p.x + NUDGE_MAX)).abs() < 0.02, "{q:?}");
        let mut none = Sel::default();
        assert!(cmd(&mut d, &mut none, &mut a, here, "nudge 1 0 0", true, 30).say[0].contains("Select"));
        assert!(cmd(&mut d, &mut sel, &mut a, here, "nudge 1 0 0", false, 40).say[0].contains("owner"));
    }

    #[test]
    fn circle_select_takes_the_props_round_the_press() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let near1 = d.spawn(2413, here.x + 8.0, here.y, here.z, 0.0).unwrap();
        let near2 = d.spawn(2413, here.x + 9.0, here.y + 1.0, here.z, 0.0).unwrap();
        let far = d.spawn(2413, here.x + 8.0, here.y + 8.0, here.z, 0.0).unwrap();
        cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        let c = uv(&a, here, [near1.x, near1.y, near1.z]);
        let e = uv(&a, here, [near1.x + 2.5, near1.y, near1.z]);
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("selcircle {} {} {} {} 1.7778 12", c[0], c[1], e[0], e[1]), true, 10);
        assert_eq!(sel.guid, Some(near1.guid), "the nearest the middle first");
        assert_eq!(sel.more, vec![near2.guid]);
        assert!(out.say[0].contains("2 selected"), "{:?}", out.say);
        // add: the far one too, with what was selected
        let c = uv(&a, here, [far.x, far.y, far.z]);
        let e = uv(&a, here, [far.x + 1.0, far.y, far.z]);
        cmd(&mut d, &mut sel, &mut a, here, &format!("selcircle {} {} {} {} 1.7778 12 add", c[0], c[1], e[0], e[1]), true, 20);
        assert_eq!(chosen(&d, &sel).len(), 3);
        // not add: only the far one
        cmd(&mut d, &mut sel, &mut a, here, &format!("selcircle {} {} {} {} 1.7778 12", c[0], c[1], e[0], e[1]), true, 30);
        assert_eq!((sel.guid, sel.more.len()), (Some(far.guid), 0));
    }
}
