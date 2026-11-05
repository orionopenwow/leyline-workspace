//! The Den builder's tools on Den Lite (grand release brief, 4 Nov: "make the Den builder's tools complete and top
//! tier"): what the Builder's Tools window (WrathUnleashedUI BuildTools.lua) asks of the Den, over the same
//! `wubuild cmd` channel as the rest of the Builder (ovmod.rs). The geometry is tools.rs (std only, tested there).
//!
//! The verbs (world.rs sends them here before the placement aids and ovmod.rs see them, [`is_tool`]):
//!
//! * `tool point <u> <v> <aspect> <zoom> [<i>]`: where the cursor meets the ground (the build camera's ray, gizmo.rs):
//!   the Path tool's next point (or point i moved there). The points are kept here, in the session's Sel (an addon
//!   line holds 255 letters, too few for them), and told back as `wubuild pathpts <n>|<group>|<length>|x,y;...`.
//! * `tool box <u1> <v1> <u2> <v2> <aspect> <zoom> [add]`: every prop whose middle is inside that box on the screen is
//!   selected (box select).
//! * `tool stamp <name> <u> <v> <aspect> <zoom> <turn> <none|lr|fb> <size>`: the stamp put down where the cursor meets
//!   the ground, turned, mirrored and sized; clicked again elsewhere, it moves there (still one undo); `stamp again
//!   <turn> <mirror> <size>` puts it down again with new numbers; `stamp keep` keeps it; `stamp cancel` takes it back.
//! * `path show` / `hide` / `undo` / `del <i>` / `clear`: the points' stakes (this player only), the last point or
//!   point i taken away; `path lay <entry> <every> <smooth 0|1> <along|across> <mid 0|1> <sides 0-3> <offset>`: the
//!   props along the points (one group, one undo, the path kept in the Den file's `paths` so it can be edited);
//!   `path edit`: the selection's path's points back (`wubuild pathedit`), and the next Lay lays it again in place
//!   (one undo).
//! * `array <n> <dx> <dy> <dz>`, `ring <n> <radius> [in|out|along|keep] [me]`: copies (one undo).
//! * `align x|y|z|bottom|top [min|max|mid|first]`, `distribute x|y|z`: the selection lined up or spread (one undo).
//! * `land`: the selection put on what is under it (a prop's top or the ground: surface snap).
//! * `solid on|off|auto`: walk through it, keep you on it, or as the game does (docs/DEN-TOOLS.md).
//! * `hide`, `show`: hidden props stay in the Den, not shown (a view, nothing to undo).
//! * `group [name]`, `ungroup`, `selgroup`, `groups`, `layer <id> select|hide|show|lock|unlock|name <name>`: groups,
//!   which the Tools window shows as layers.
//! * `measure`: distances and sizes; `history`: the undo steps by name; `undoto <n>`: n steps back at once.
//! * `stamps`: the stamp library with categories and credits; `stampplan <name>`: its top-down picture; `stamp save
//!   <name> [category] [title...]`: the selection made a stamp (`<dens>/stamps-mine`), its credit kept; `stamp delete
//!   <name>`: one of your own.

use crate::den::{self, Change, Den, DenDoc, Prop};
use crate::gizmo::{self, AidOut, Aids};
use crate::ovmod::{self, Here, Sel};
use crate::tools::{self, Face, Lay, Mirror, RingFace, To, Xf};
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::Mutex;

/// The verbs every Den Lite with these tools answers (added to the state's `caps=`, ovmod.rs).
pub const CAPS: &str = "tool,path,array,ring,align,distribute,land,solid,hide,show,group,ungroup,selgroup,groups,layer,measure,history,undoto,stamps,stampplan,stamp";

/// Whether this `wubuild cmd` is one of the tools' (see the module's list).
pub fn is_tool(text: &str) -> bool {
    let v = text.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    CAPS.split(',').any(|c| c == v)
}

/// Where the tools find and keep things: the Dens folder (the player's own stamps), the stamps folder, who is building
/// (a saved stamp's credit), and the time (unix seconds).
pub struct Ctx<'a> {
    pub dens: &'a Path,
    pub stamps: &'a Path,
    pub who: &'a str,
    pub unix: u64,
}

/// The stamp put down with the mouse and not yet kept: moved, turned, mirrored and sized again while it is the last
/// thing done (Den::last_placed). One Den per Den Lite, so one at a time.
#[derive(Debug, Clone, PartialEq)]
struct Staged {
    name: String,
    at: [f32; 3],
    o: f32,
    xf: Xf,
    guids: Vec<u64>,
}
static STAGED: Mutex<Option<Staged>> = Mutex::new(None);
/// The path's stakes: two banks of guids, so a new set is made before the old one goes (as the carried copy).
static BANK: Mutex<bool> = Mutex::new(false);

/// The path's stakes: Excavation Stake (objects.rs), small; counters below the aids' own (gizmo.rs AID_BASE).
pub const STAKE: u32 = 180_007;
const STAKE_SCALE: f32 = 0.6;
const MARK_BASE: u64 = 0xFF_FE00;
const HIGH_GAMEOBJECT: u64 = 0xF110;

fn stake_guid(bank: bool, i: usize) -> u64 {
    (HIGH_GAMEOBJECT << 48) | ((STAKE as u64 & 0xFF_FFFF) << 24) | (MARK_BASE + if bank { 64 } else { 0 } + i as u64)
}

fn said(s: impl Into<String>) -> AidOut {
    AidOut { say: vec![s.into()], state: true, ..Default::default() }
}

fn changed(ch: Change, msg: impl Into<String>) -> AidOut {
    let mut ch = ch;
    let m: String = msg.into();
    if !m.is_empty() {
        ch.message = m;
    }
    AidOut { change: Some(ch), state: true, ..Default::default() }
}

fn tell(mut o: AidOut, name: &str, data: String) -> AidOut {
    o.addon.push((name.into(), data));
    o
}

fn num(w: &[&str], i: usize) -> Option<f32> {
    w.get(i).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite())
}

/// The selection: the one selected first, then the ones added (each once, only props still there).
pub fn chosen(d: &Den, sel: &Sel) -> Vec<u64> {
    let mut v: Vec<u64> = Vec::new();
    for g in sel.guid.into_iter().chain(sel.more.iter().copied()) {
        if d.placed(g).is_some() && !v.contains(&g) {
            v.push(g);
        }
    }
    v
}

fn name_of(d: &Den, g: u64) -> &'static str {
    d.placed(g).and_then(|p| den::template(p.prop.entry as u32)).map(|t| t.name).unwrap_or("prop")
}

/// Two changes one after the other as one: what the second makes again after the first took it away is made; what
/// the second takes away of what the first made is not made (the client is told the end of it, destroys first).
pub fn merge(a: Change, b: Change) -> Change {
    let mut destroyed = a.destroyed;
    let mut created = a.created;
    for g in b.destroyed {
        created.retain(|o| o.guid != g);
        if !destroyed.contains(&g) {
            destroyed.push(g);
        }
    }
    for o in b.created {
        created.retain(|c| c.guid != o.guid);
        created.push(o);
    }
    let message = if b.message.is_empty() { a.message } else { b.message };
    Change { created, destroyed, skipped: a.skipped + b.skipped, message }
}

/// Where the cursor at (u, v) meets the ground, from the camera the aids know (the build view, the far view).
pub fn ground_at(d: &Den, a: &mut Aids, here: Here, u: f32, v: f32, aspect: f32, zoom: Option<f32>) -> Option<[f32; 3]> {
    if let Some(z) = zoom.filter(|z| (0.0..=60.0).contains(z)) {
        a.zoom = Some(z);
    }
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 16.0 / 9.0 };
    let (from, cam) = a.view(here);
    let (e, dir) = gizmo::ray(&cam, from, u, v, aspect)?;
    crate::pick::on_ground(e, dir, cam.dist + gizmo::REACH, |x, y| d.floor_at(x, y))
}

/// The props marked Solid, as tops for the guard rail (and every shown prop's top for surface snap: `all`).
pub fn tops(d: &Den, all: bool, leave_out: &[u64]) -> Vec<tools::Top> {
    d.props
        .iter()
        .filter(|p| p.prop.hidden != Some(true) && !leave_out.contains(&p.obj.guid) && (all || p.prop.solid == Some(true)))
        .map(|p| {
            let b = gizmo::body_of(&p.obj);
            tools::Top { at: [b.at[0], b.at[1]], yaw: b.yaw, half: [b.half[0] / 1.05, b.half[1] / 1.05], top: b.at[2] + b.bottom + b.height }
        })
        .collect()
}

/// The guard rail (world.rs, on each move of the player's game): the height to put the player back at, or None.
/// On a prop marked Solid that the game let you sink into, back on its top; fallen through the ground of a Den on
/// its own island, back on the ground. Not while flying, swimming or on a transport.
pub fn guard(d: &Den, was: [f32; 3], now: [f32; 3], flags: u32) -> Option<f32> {
    use crate::movewatch::{CAN_FLY, FLYING, ON_TRANSPORT, SWIMMING};
    if flags & (FLYING | CAN_FLY | SWIMMING | ON_TRANSPORT) != 0 || !now.iter().chain(was.iter()).all(|v| v.is_finite()) {
        return None;
    }
    if !d.inside(now[0], now[1], d.floor_at(now[0], now[1])) {
        return None;
    }
    if let Some(z) = tools::top_hold(was, now, &tops(d, false, &[])) {
        return Some(z);
    }
    let ground = d.floor_at(now[0], now[1]);
    (d.spot.is_home() && tools::through_ground(now[2], ground)).then_some(ground + tools::LIFT)
}

/// The selection's extra state for the Builder (ovmod.rs `state`): its Solid, what the game does with its model, its
/// group and the tools' caps.
pub fn state_extra(d: &Den, sel: &Sel) -> String {
    let Some(p) = sel.guid.and_then(|g| d.placed(g)) else { return String::new() };
    let solid = match p.prop.solid {
        Some(true) => "on",
        Some(false) => "off",
        None => "auto",
    };
    format!("solid={solid};coll={};group={};hidden={};", collide_word(d, p.obj.guid), p.prop.group.unwrap_or(0), (p.prop.hidden == Some(true)) as u8)
}

fn collide_of(d: &Den, g: u64) -> tools::Collide {
    let Some(p) = d.placed(g) else { return tools::Collide::Unknown };
    let name = den::template(p.prop.entry as u32).map(|t| t.name).unwrap_or("");
    let wmo = crate::models::is_wmo(p.obj.display);
    tools::client_collision(ovmod::category(name).unwrap_or("decor"), wmo)
}

fn collide_word(d: &Den, g: u64) -> &'static str {
    match collide_of(d, g) {
        tools::Collide::Model => "model",
        tools::Collide::None => "none",
        tools::Collide::Unknown => "unknown",
    }
}

/// The next free group number.
fn next_group(d: &Den) -> u64 {
    d.props.iter().filter_map(|p| p.prop.group).max().unwrap_or(0) + 1
}

/// The Den file's notes on its groups (`extra.groups`: `{"<id>": {"name": .., "stamp": .., "credit": ..}}`).
fn group_note(d: &mut Den, g: u64) -> &mut Map<String, Value> {
    let gs = d.extra.entry("groups").or_insert_with(|| Value::Object(Map::new()));
    if !gs.is_object() {
        *gs = Value::Object(Map::new());
    }
    let m = gs.as_object_mut().expect("an object");
    let e = m.entry(g.to_string()).or_insert_with(|| Value::Object(Map::new()));
    if !e.is_object() {
        *e = Value::Object(Map::new());
    }
    e.as_object_mut().expect("an object")
}

fn group_name(d: &Den, g: u64) -> Option<String> {
    d.extra.get("groups")?.get(g.to_string())?.get("name")?.as_str().map(str::to_string)
}

/// Plain words for a name a player typed (no separators the addon's lines use).
fn plain(s: &str, max: usize) -> String {
    s.chars().filter(|c| c.is_alphanumeric() || " '-_.".contains(*c)).take(max).collect::<String>().trim().to_string()
}

/// One tools verb. `can`: may build (a visitor only looks: measure, history, stamps, select).
pub fn cmd(d: &mut Den, sel: &mut Sel, a: &mut Aids, here: Here, text: &str, can: bool, ctx: &Ctx) -> AidOut {
    let w: Vec<&str> = text.split_whitespace().collect();
    let verb = w.first().map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let sub = w.get(1).map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let looks = match verb.as_str() {
        "measure" | "history" | "stamps" | "stampplan" | "groups" | "selgroup" => true,
        "tool" => sub == "box" || sub == "point",
        "path" => sub == "show" || sub == "hide" || sub == "edit",
        "layer" => w.get(2).is_some_and(|x| x.eq_ignore_ascii_case("select")),
        _ => false,
    };
    if !can && !looks {
        return said("Only the Den's owner builds here (they can let friends: .den friends build on).");
    }
    match verb.as_str() {
        "tool" => tool(d, sel, a, here, &w, ctx),
        "path" => path(d, sel, &w),
        "array" => array(d, sel, &w),
        "ring" => ring(d, sel, here, &w),
        "align" | "distribute" => align(d, sel, &verb, &w),
        "land" => land(d, sel),
        "solid" => solid(d, sel, &sub),
        "hide" | "show" => hide(d, sel, verb == "hide"),
        "group" | "ungroup" | "selgroup" | "groups" | "layer" => groups(d, sel, &verb, &w),
        "measure" => measure(d, sel, here),
        "history" => history(d),
        "undoto" => {
            let n = w.get(1).and_then(|n| n.parse::<usize>().ok()).unwrap_or(1).clamp(1, den::UNDO_DEPTH);
            let mut ch = Change::default();
            let mut done = 0;
            for _ in 0..n {
                if d.undo_depth() == 0 {
                    break;
                }
                ch = merge(ch, d.undo());
                done += 1;
            }
            *STAGED.lock().unwrap_or_else(|e| e.into_inner()) = None;
            let o = changed(ch, format!("Undone {done} step{}.", if done == 1 { "" } else { "s" }));
            let h = history_line(d);
            tell(o, "history", h)
        }
        "stamps" => tell(AidOut::default(), "stamps", stamp_list(ctx)),
        "stampplan" => match den::read_stamp(ctx.stamps, w.get(1).copied().unwrap_or("")) {
            Ok(doc) => tell(AidOut::default(), "stampplan", format!("{}|{}", den::clean_name(w[1]).unwrap_or_default(), plan_line(&doc))),
            Err(e) => said(e),
        },
        "stamp" => stamp(d, sel, &w, ctx),
        _ => said("The Builder's tools don't know that."),
    }
}

// ---------------------------------------------------------------- tool: the mouse's ground point, box, stamp

fn tool(d: &mut Den, sel: &mut Sel, a: &mut Aids, here: Here, w: &[&str], ctx: &Ctx) -> AidOut {
    match w.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("point") => match (num(w, 2), num(w, 3), num(w, 4)) {
            (Some(u), Some(v), Some(asp)) => match ground_at(d, a, here, u, v, asp, num(w, 5)) {
                Some(p) if d.inside(p[0], p[1], p[2]) => {
                    // `tool point <u> <v> <aspect> <zoom> [<i>]`: the path's next point, or point i moved there
                    let i = w.get(6).and_then(|i| i.parse::<usize>().ok());
                    let mut out = path_point(d, sel, p, i);
                    out.addon.push(("point".into(), format!("{:.2},{:.2},{:.2}", p[0], p[1], p[2])));
                    out
                }
                Some(_) => said("That is outside your Den."),
                None => said("Click on the ground."),
            },
            _ => said("tool point <u> <v> <aspect> <zoom>"),
        },
        Some("box") => match (num(w, 2), num(w, 3), num(w, 4), num(w, 5), num(w, 6)) {
            (Some(u1), Some(v1), Some(u2), Some(v2), Some(asp)) => {
                if let Some(z) = num(w, 7).filter(|z| (0.0..=60.0).contains(z)) {
                    a.zoom = Some(z);
                }
                let (from, cam) = a.view(here);
                let inside: Vec<u64> = d
                    .props
                    .iter()
                    .filter(|p| p.prop.hidden != Some(true))
                    .filter(|p| {
                        let b = gizmo::body_of(&p.obj);
                        let mid = [b.at[0], b.at[1], b.at[2] + b.bottom + b.height / 2.0];
                        gizmo::screen(&cam, from, mid, asp).is_some_and(|s| tools::in_rect(s, [u1, v1], [u2, v2]))
                    })
                    .map(|p| p.obj.guid)
                    .collect();
                let add = w.get(8).is_some_and(|x| x.eq_ignore_ascii_case("add"));
                let mut all = if add { chosen(d, sel) } else { Vec::new() };
                for g in inside {
                    if !all.contains(&g) {
                        all.push(g);
                    }
                }
                select(sel, &all);
                said(if all.is_empty() { "Nothing in the box.".to_string() } else { format!("{} selected.", all.len()) })
            }
            _ => said("tool box <u1> <v1> <u2> <v2> <aspect> <zoom> [add]"),
        },
        Some("stamp") => {
            let name = w.get(2).copied().unwrap_or("");
            match (num(w, 3), num(w, 4), num(w, 5)) {
                (Some(u), Some(v), Some(asp)) => match ground_at(d, a, here, u, v, asp, num(w, 6)) {
                    Some(p) => stage(d, ctx, name, p, here.o, xf_of(w, 7)),
                    None => said("Click on the ground where the stamp goes."),
                },
                _ => said("tool stamp <name> <u> <v> <aspect> <zoom> <turn> <mirror> <size>"),
            }
        }
        _ => said("tool point | box | stamp"),
    }
}

fn select(sel: &mut Sel, all: &[u64]) {
    sel.guid = all.first().copied();
    sel.more = all.iter().skip(1).copied().collect();
    sel.round = None;
}

/// `<turn> <none|lr|fb> <size>` from word `i` on.
fn xf_of(w: &[&str], i: usize) -> Xf {
    let mirror = match w.get(i + 1).map(|m| m.to_ascii_lowercase()).as_deref() {
        Some("lr") => Mirror::LeftRight,
        Some("fb") => Mirror::FrontBack,
        _ => Mirror::None,
    };
    Xf { turn: num(w, i).unwrap_or(0.0).clamp(-360.0, 360.0), mirror, scale: num(w, i + 2).unwrap_or(1.0) }
}

/// The stamp's pieces mirrored and sized (tools.rs `transform`).
pub fn transformed(doc: &DenDoc, xf: &Xf) -> DenDoc {
    let mut out = doc.clone();
    for p in &mut out.props {
        let size = p.scale.unwrap_or_else(|| den::template(p.entry as u32).map(|t| t.size as f64).unwrap_or(1.0)) as f32;
        let (x, y, z, yaw, s) = tools::transform(p.x as f32, p.y as f32, p.z as f32, p.yaw as f32, size, xf);
        p.x = x as f64;
        p.y = y as f64;
        p.z = z as f64;
        p.yaw = yaw as f64;
        if (xf.scale - 1.0).abs() > 1e-3 {
            p.scale = Some(s as f64);
        }
        if xf.mirror != Mirror::None {
            if let Some(r) = p.roll {
                p.roll = Some(-r);
            }
        }
    }
    out
}

/// The stamp put down at `at` (or moved there, when it is the staged one and nothing was done since).
fn stage(d: &mut Den, ctx: &Ctx, name: &str, at: [f32; 3], o: f32, xf: Xf) -> AidOut {
    let doc = match den::read_stamp(ctx.stamps, name) {
        Ok(doc) => doc,
        Err(e) => return said(e),
    };
    let key = den::clean_name(name).unwrap_or_default();
    let mut st = STAGED.lock().unwrap_or_else(|e| e.into_inner());
    let mut ch = Change::default();
    if let Some(s) = st.as_ref() {
        if d.last_placed() == Some(&s.guids[..]) {
            ch = d.undo();
        }
    }
    let xdoc = transformed(&doc, &xf);
    let o = o + xf.turn.to_radians();
    let (fx, fy, _) = d.fit_stamp(&xdoc, at[0], at[1], o);
    let down = d.stamp(&xdoc, fx, fy, at[2], o);
    let guids: Vec<u64> = down.created.iter().map(|o| o.guid).collect();
    if guids.is_empty() {
        *st = None;
        return changed(merge(ch, down), "");
    }
    // the stamp's credit goes with its pieces' group (kept when a selection of them is saved as a new stamp)
    if let Some(g) = d.placed(guids[0]).and_then(|p| p.prop.group) {
        let credit = doc.extra.get("credit").cloned();
        let note = group_note(d, g);
        note.insert("stamp".into(), json!(key));
        if let Some(c) = credit {
            note.insert("credit".into(), c);
        }
    }
    *st = Some(Staged { name: key.clone(), at, o: o - xf.turn.to_radians(), xf, guids });
    let title = doc.title.clone().unwrap_or(key);
    let mut out = changed(merge(ch, down), format!("{title} is down: click elsewhere to move it, turn, mirror or size it, Enter keeps it."));
    out.addon.push(("staged".into(), format!("{}|{:.0}|{}|{:.2}", st.as_ref().map(|s| s.name.as_str()).unwrap_or(""), xf.turn, mirror_word(xf.mirror), xf.scale)));
    out
}

fn mirror_word(m: Mirror) -> &'static str {
    match m {
        Mirror::None => "none",
        Mirror::LeftRight => "lr",
        Mirror::FrontBack => "fb",
    }
}

// ---------------------------------------------------------------- paths

/// A path's numbers (`<every> <smooth> <face> <mid> <sides> <offset>` from word `i`).
fn lay_of(w: &[&str], i: usize) -> Lay {
    Lay {
        every: num(w, i).unwrap_or(3.0),
        smooth: w.get(i + 1).is_none_or(|s| *s != "0"),
        face: if w.get(i + 2).is_some_and(|s| s.eq_ignore_ascii_case("across")) { Face::Across } else { Face::Along },
        mid: w.get(i + 3).is_some_and(|s| *s == "1"),
        sides: w.get(i + 4).and_then(|s| s.parse::<u8>().ok()).filter(|s| *s <= 3).unwrap_or(0),
        offset: num(w, i + 5).unwrap_or(0.0),
    }
}

/// The props of a path (relative to the Den): one every `lay.every` along the points (world x, y), on the ground.
pub fn path_props(d: &Den, entry: u32, pts: &[tools::P2], l: &Lay, group: u64) -> Vec<Prop> {
    tools::lay(pts, l)
        .into_iter()
        .map(|(p, yaw)| Prop {
            entry: entry as u64,
            x: (p[0] - d.spot.x) as f64,
            y: (p[1] - d.spot.y) as f64,
            z: (d.floor_at(p[0], p[1]) - d.spot.z) as f64,
            yaw: (yaw.to_degrees() as f64).rem_euclid(360.0),
            group: Some(group),
            ..Default::default()
        })
        .collect()
}

fn path_record(d: &Den, group: u64, entry: u32, l: &Lay, pts: &[tools::P2]) -> Value {
    let rel: Vec<Value> = pts.iter().map(|p| json!([((p[0] - d.spot.x) * 100.0).round() / 100.0, ((p[1] - d.spot.y) * 100.0).round() / 100.0])).collect();
    let face = if l.face == Face::Across { "across" } else { "along" };
    json!({
        "group": group, "entry": entry, "every": l.every, "smooth": l.smooth, "face": face,
        "mid": l.mid, "sides": l.sides, "offset": l.offset, "points": rel, "tool": "lite"
    })
}

/// The Path tool's points back to the addon (`wubuild pathpts <n>|<group or 0>|x,y;...`) and their stakes.
fn points_out(d: &Den, sel: &Sel) -> AidOut {
    let mut out = stakes(d, &sel.path);
    let len = tools::length(&tools::smooth(&sel.path, tools::SMOOTH_STEPS));
    out.addon.push(("pathpts".into(), format!("{}|{}|{:.1}|{}", sel.path.len(), sel.path_group.unwrap_or(0), len, tools::write_points(&sel.path))));
    out
}

/// A point added to the path (or point `i` moved there): the Path tool's click on the ground.
pub fn path_point(d: &Den, sel: &mut Sel, p: [f32; 3], i: Option<usize>) -> AidOut {
    match i {
        Some(i) if i < sel.path.len() => sel.path[i] = [p[0], p[1]],
        _ if sel.path.len() >= tools::POINTS_MAX => return said(format!("A path has up to {} points.", tools::POINTS_MAX)),
        _ => sel.path.push([p[0], p[1]]),
    }
    points_out(d, sel)
}

fn path(d: &mut Den, sel: &mut Sel, w: &[&str]) -> AidOut {
    let sub = w.get(1).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    match sub.as_str() {
        "show" => points_out(d, sel),
        "hide" => stakes(d, &[]),
        "undo" => {
            sel.path.pop();
            points_out(d, sel)
        }
        "del" => {
            if let Some(i) = w.get(2).and_then(|i| i.parse::<usize>().ok()).filter(|i| *i < sel.path.len()) {
                sel.path.remove(i);
            }
            points_out(d, sel)
        }
        "clear" => {
            sel.path.clear();
            sel.path_group = None;
            points_out(d, sel)
        }
        "lay" => {
            let Some(entry) = w.get(2).and_then(|e| e.parse::<u32>().ok()).filter(|e| den::template(*e).is_some()) else {
                return said("Pick the prop the path is made of first (a fence, a lamp, a road tile...).");
            };
            let l = lay_of(w, 3);
            let pts = sel.path.clone();
            if pts.is_empty() {
                return said("Click on the ground where the path goes first.");
            }
            // laid again when it is a path being edited (its group still there), else a new one
            let relay = sel.path_group.filter(|g| d.props.iter().any(|p| p.prop.group == Some(*g)));
            let g = relay.unwrap_or_else(|| next_group(d));
            let props = path_props(d, entry, &pts, &l, g);
            let room = den::MAX_PROPS.saturating_sub(d.props.len());
            let old: Vec<u64> = if relay.is_some() { members(d, g) } else { Vec::new() };
            if props.len() > room + old.len() {
                return said(format!("That path is {} props; your Den has room for {}. Space them wider.", props.len(), room + old.len()));
            }
            let ch = if relay.is_some() { d.replace_props(&old, &props) } else { d.place_many(&props) };
            let n = ch.created.len();
            // the path kept with the Den (its points, so it can be edited and laid again)
            let rec = path_record(d, g, entry, &l, &pts);
            d.paths.retain(|p| p.get("group").and_then(Value::as_u64) != Some(g));
            if d.paths.len() < den::MAX_PATHS {
                d.paths.push(rec);
            }
            let left = if ch.skipped > 0 { format!(" ({} left out: outside your Den)", ch.skipped) } else { String::new() };
            let len = tools::length(&tools::smooth(&pts, if l.smooth { tools::SMOOTH_STEPS } else { 0 }));
            let mut out = changed(ch, format!("Path {}: {n} {} along {len:.0} yd{left}. Undo takes it back.", if relay.is_some() { "laid again" } else { "laid" }, name_by_entry(entry)));
            out.destroy = stakes(d, &[]).destroy;
            sel.path.clear();
            sel.path_group = None;
            out.addon.push(("pathpts".into(), "0|0|0|".into()));
            // the new path's props selected (one group: Move, Delete, Hide take them all)
            select(sel, &members(d, g));
            out
        }
        "edit" => {
            let Some(g) = sel.guid.and_then(|g| d.placed(g)).and_then(|p| p.prop.group) else {
                return said("Select a prop of the path first (a click on one of its fences or lamps).");
            };
            let Some(rec) = d.paths.iter().find(|p| p.get("group").and_then(Value::as_u64) == Some(g) && p.get("points").is_some()).cloned() else {
                return said("That prop isn't on a path made with the Path tool.");
            };
            sel.path = rec
                .get("points")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|p| Some([p.get(0)?.as_f64()? as f32 + d.spot.x, p.get(1)?.as_f64()? as f32 + d.spot.y])).take(tools::POINTS_MAX).collect())
                .unwrap_or_default();
            sel.path_group = Some(g);
            let f = |k: &str| rec.get(k).cloned().unwrap_or(Value::Null);
            let line = format!(
                "{g}|{}|{}|{}|{}|{}|{}|{}",
                f("entry").as_u64().unwrap_or(0),
                short(f("every").as_f64().unwrap_or(3.0)),
                (f("smooth") != Value::Bool(false)) as u8,
                f("face").as_str().unwrap_or("along"),
                (f("mid") == Value::Bool(true)) as u8,
                f("sides").as_u64().unwrap_or(0),
                short(f("offset").as_f64().unwrap_or(0.0)),
            );
            let mut out = points_out(d, sel);
            out.addon.push(("pathedit".into(), line));
            out.say.push("The path's points are back: click a point's number then the ground to move it, add or take away points, then Lay again.".into());
            out
        }
        _ if sub.parse::<u64>().is_ok() => said("Den Lite lays paths with the Path tool: click the ground along the way, then Lay."),
        _ => said("path show | hide | undo | del <n> | clear | lay | edit"),
    }
}

/// A number as the addon's boxes show it: whole numbers without a point, else two decimals.
fn short(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 { format!("{}", v.round() as i64) } else { format!("{v:.2}") }
}

fn name_by_entry(e: u32) -> &'static str {
    den::template(e).map(|t| t.name).unwrap_or("props")
}

/// The path's stakes at these points (world x, y; on the ground): made in the other bank, then the old ones go.
fn stakes(d: &Den, pts: &[tools::P2]) -> AidOut {
    let mut bank = BANK.lock().unwrap_or_else(|e| e.into_inner());
    let old = *bank;
    *bank = !old;
    let display = den::template(STAKE).map(|t| t.display).unwrap_or(2270);
    let create = pts
        .iter()
        .take(tools::POINTS_MAX)
        .enumerate()
        .map(|(i, p)| den::WorldObject {
            guid: stake_guid(!old, i),
            entry: STAKE,
            display,
            kind: 5,
            scale: STAKE_SCALE,
            x: p[0],
            y: p[1],
            z: d.floor_at(p[0], p[1]),
            o: 0.0,
            pitch: 0.0,
            roll: 0.0,
        })
        .collect();
    let destroy = (0..tools::POINTS_MAX).map(|i| stake_guid(old, i)).collect();
    AidOut { create, destroy, ..Default::default() }
}

// ---------------------------------------------------------------- arrays

fn copy_of(p: &Prop) -> Prop {
    let mut c = p.clone();
    c.group = None;
    c.locked = None;
    c.role = None;
    c
}

fn array(d: &mut Den, sel: &mut Sel, w: &[&str]) -> AidOut {
    let all = chosen(d, sel);
    if all.is_empty() {
        return said("Select a prop first (Near picks the closest).");
    }
    let n = w.get(1).and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
    let step = [num(w, 2).unwrap_or(0.0), num(w, 3).unwrap_or(0.0), num(w, 4).unwrap_or(0.0)];
    if n == 0 || step.iter().all(|s| s.abs() < 0.01) {
        return said("array <copies> <x step> <y step> <z step>: say how many and how far apart.");
    }
    let mut props = Vec::new();
    for off in tools::line_array(n, step) {
        for g in &all {
            let mut c = copy_of(&d.placed(*g).expect("chosen").prop);
            c.x += off[0] as f64;
            c.y += off[1] as f64;
            c.z += off[2] as f64;
            props.push(c);
        }
    }
    many(d, props, "copies in a row")
}

fn ring(d: &mut Den, sel: &mut Sel, here: Here, w: &[&str]) -> AidOut {
    let all = chosen(d, sel);
    let Some(first) = all.first().and_then(|g| d.placed(*g)).map(|p| p.prop.clone()) else {
        return said("Select a prop first (Near picks the closest).");
    };
    let n = w.get(1).and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
    let r = num(w, 2).unwrap_or(0.0);
    if n == 0 || !(0.5..=60.0).contains(&r) {
        return said("ring <copies> <radius> [in|out|along|keep] [me]: how many, and how far out (half a yard to 60).");
    }
    let face = match w.get(3).map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("out") => RingFace::Out,
        Some("along") => RingFace::Along,
        Some("keep") => RingFace::Keep,
        _ => RingFace::In,
    };
    // round you (me), else round the selection's first prop
    let me = w.iter().any(|x| x.eq_ignore_ascii_case("me"));
    let mid = if me { [(here.x - d.spot.x) as f64, (here.y - d.spot.y) as f64] } else { [first.x, first.y] };
    let yaw0 = (first.yaw as f32).to_radians();
    let mut props = Vec::new();
    for (off, yaw) in tools::ring_array(n, r, 0.0, face, yaw0) {
        let turn = yaw - yaw0;
        let (s, c) = turn.sin_cos();
        for g in &all {
            let p = &d.placed(*g).expect("chosen").prop;
            let mut cp = copy_of(p);
            // the selection as one piece: each prop's offset from the first turned with the copy
            let (dx, dy) = ((p.x - first.x) as f32, (p.y - first.y) as f32);
            cp.x = mid[0] + off[0] as f64 + (dx * c - dy * s) as f64;
            cp.y = mid[1] + off[1] as f64 + (dx * s + dy * c) as f64;
            cp.yaw = (p.yaw + turn.to_degrees() as f64).rem_euclid(360.0);
            props.push(cp);
        }
    }
    many(d, props, "copies round a ring")
}

/// Props placed at once (one undo), on the ground where they were lifted off it by the copy, the new ones selected.
fn many(d: &mut Den, props: Vec<Prop>, what: &str) -> AidOut {
    let room = den::MAX_PROPS.saturating_sub(d.props.len());
    if props.len() > room {
        return said(format!("That is {} props; your Den has room for {room}.", props.len()));
    }
    let ch = d.place_many(&props);
    let n = ch.created.len();
    let left = if ch.skipped > 0 { format!(" ({} left out: outside your Den)", ch.skipped) } else { String::new() };
    changed(ch, format!("{n} {what}{left}. Undo takes them all back."))
}

// ---------------------------------------------------------------- align, distribute, land

fn align(d: &mut Den, sel: &mut Sel, verb: &str, w: &[&str]) -> AidOut {
    let all = chosen(d, sel);
    if all.len() < 2 || (verb == "distribute" && all.len() < 3) {
        return said(if verb == "distribute" { "Select three props or more (Shift+click adds one, or the Box)." } else { "Select two props or more (Shift+click adds one, or the Box)." });
    }
    let axis = w.get(1).map(|a| a.to_ascii_lowercase()).unwrap_or_else(|| "x".into());
    let ends = |p: &den::Placed| {
        let b = gizmo::body_of(&p.obj);
        (b.bottom as f64, (b.bottom + b.height) as f64)
    };
    let props: Vec<(u64, Prop, (f64, f64))> = all.iter().filter_map(|g| d.placed(*g).map(|p| (*g, p.prop.clone(), ends(p)))).collect();
    let get = |p: &Prop, t: (f64, f64)| -> f32 {
        (match axis.as_str() {
            "y" => p.y,
            "z" => p.z,
            "bottom" => p.z + t.0,
            "top" => p.z + t.1,
            _ => p.x,
        }) as f32
    };
    let vals: Vec<f32> = props.iter().map(|(_, p, t)| get(p, *t)).collect();
    let new: Vec<f32> = if verb == "distribute" {
        tools::distribute(&vals)
    } else {
        let to = match w.get(2).map(|t| t.to_ascii_lowercase()).as_deref() {
            Some("min") => To::Min,
            Some("max") => To::Max,
            Some("mid") => To::Mid,
            Some("first") => To::First,
            _ if axis == "bottom" => To::Min,
            _ if axis == "top" => To::Max,
            _ => To::First,
        };
        let Some(v) = tools::align_to(&vals, to) else { return said("Nothing to line up.") };
        vec![v; vals.len()]
    };
    let list: Vec<(u64, Prop)> = props
        .into_iter()
        .zip(vals.iter().zip(new.iter()))
        .map(|((g, mut p, _), (old, nv))| {
            let dv = (*nv - *old) as f64;
            match axis.as_str() {
                "y" => p.y += dv,
                "z" | "bottom" | "top" => p.z += dv,
                _ => p.x += dv,
            }
            (g, p)
        })
        .collect();
    match d.edit_each(list) {
        Ok(ch) => {
            let n = ch.created.len();
            changed(ch, format!("{} {n} along {axis}. Undo puts them back.", if verb == "distribute" { "Spread" } else { "Lined up" }))
        }
        Err(e) => said(e),
    }
}

fn land(d: &mut Den, sel: &mut Sel) -> AidOut {
    let all = chosen(d, sel);
    if all.is_empty() {
        return said("Select a prop first (Near picks the closest).");
    }
    let surf = tops(d, true, &all);
    let list: Vec<(u64, Prop)> = all
        .iter()
        .filter_map(|g| d.placed(*g).map(|p| (*g, p.prop.clone(), [p.obj.x, p.obj.y, p.obj.z], gizmo::body_of(&p.obj).bottom)))
        .map(|(g, mut p, at, bottom)| {
            let ground = d.floor_at(at[0], at[1]);
            let top = tools::top_under([at[0], at[1]], at[2] + bottom, &surf).filter(|t| *t > ground).unwrap_or(ground);
            p.z = (top - bottom - d.spot.z) as f64;
            (g, p)
        })
        .collect();
    match d.edit_each(list) {
        Ok(ch) => changed(ch, "On what is under it. Undo puts it back."),
        Err(e) => said(e),
    }
}

// ---------------------------------------------------------------- solid, hide

fn solid(d: &mut Den, sel: &mut Sel, how: &str) -> AidOut {
    let all = chosen(d, sel);
    if all.is_empty() {
        return said("Select a prop first (Near picks the closest).");
    }
    let want = match how {
        "on" => Some(true),
        "off" => Some(false),
        "auto" | "" => None,
        _ => return said("solid on | off | auto"),
    };
    let coll = collide_of(d, all[0]);
    let list: Vec<(u64, Prop)> = all.iter().filter_map(|g| d.placed(*g).map(|p| (*g, Prop { solid: want, ..p.prop.clone() }))).collect();
    let msg = match want {
        Some(false) => "Solid off: you walk through it (your game is told it is an open door; if you still bump into it, its model doesn't allow it).".to_string(),
        Some(true) => match coll {
            tools::Collide::Model => format!("Solid on: {} is solid in your game, and Den Lite keeps you on its top.", name_of(d, all[0])),
            _ => format!("Solid on: Den Lite keeps you on top of {} (jump onto it). Its model may have no sides you bump into: your game decides that.", name_of(d, all[0])),
        },
        None => "Solid as the game makes it (from its model).".to_string(),
    };
    match d.edit_each(list) {
        Ok(ch) => changed(ch, msg),
        Err(e) => said(e),
    }
}

fn hide(d: &mut Den, sel: &mut Sel, on: bool) -> AidOut {
    let mut ch = Change::default();
    if on {
        let all = chosen(d, sel);
        if all.is_empty() {
            return said("Select a prop first (Near picks the closest).");
        }
        for p in d.props.iter_mut().filter(|p| all.contains(&p.obj.guid)) {
            p.prop.hidden = Some(true);
            ch.destroyed.push(p.obj.guid);
        }
        select(sel, &[]);
        ch.message = format!("Hidden {}: still in your Den, not shown. Show all brings them back.", ch.destroyed.len());
    } else {
        for p in d.props.iter_mut().filter(|p| p.prop.hidden == Some(true)) {
            p.prop.hidden = None;
            ch.created.push(p.obj.clone());
        }
        ch.message = format!("Shown {}.", ch.created.len());
    }
    changed(ch, "")
}

// ---------------------------------------------------------------- groups and layers

fn groups(d: &mut Den, sel: &mut Sel, verb: &str, w: &[&str]) -> AidOut {
    let all = chosen(d, sel);
    match verb {
        "group" => {
            if all.len() < 2 {
                return said("Select two props or more to group them (Shift+click adds one, or the Box).");
            }
            let g = next_group(d);
            for p in d.props.iter_mut().filter(|p| all.contains(&p.obj.guid)) {
                p.prop.group = Some(g);
            }
            let name = plain(&w[1..].join(" "), 24);
            let name = if name.is_empty() { format!("Group {g}") } else { name };
            group_note(d, g).insert("name".into(), json!(name));
            let o = said(format!("Grouped {} as {name}: a click on one selects them all with Select group.", all.len()));
            tell(o, "groups", groups_line(d))
        }
        "ungroup" => {
            if all.is_empty() {
                return said("Select a prop first (Near picks the closest).");
            }
            for p in d.props.iter_mut().filter(|p| all.contains(&p.obj.guid)) {
                p.prop.group = None;
            }
            tell(said(format!("Ungrouped {}.", all.len())), "groups", groups_line(d))
        }
        "selgroup" => {
            let Some(g) = sel.guid.and_then(|g| d.placed(g)).and_then(|p| p.prop.group) else {
                return said("That prop isn't in a group.");
            };
            let members = members(d, g);
            select(sel, &members);
            said(format!("Selected the group: {} props.", members.len()))
        }
        "groups" => tell(AidOut::default(), "groups", groups_line(d)),
        _ => {
            // layer <id> select|hide|show|lock|unlock|name <name>
            let Some(g) = w.get(1).and_then(|g| g.parse::<u64>().ok()) else { return said("layer <group> select|hide|show|lock|unlock|name <name>") };
            let m = members(d, g);
            if m.is_empty() {
                return said("No such group.");
            }
            let what = w.get(2).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            let out = match what.as_str() {
                "select" => {
                    select(sel, &m);
                    said(format!("Selected {}.", m.len()))
                }
                "hide" | "show" => {
                    let mut ch = Change::default();
                    for p in d.props.iter_mut().filter(|p| m.contains(&p.obj.guid)) {
                        let hidden = p.prop.hidden == Some(true);
                        if what == "hide" && !hidden {
                            p.prop.hidden = Some(true);
                            ch.destroyed.push(p.obj.guid);
                        } else if what == "show" && hidden {
                            p.prop.hidden = None;
                            ch.created.push(p.obj.clone());
                        }
                    }
                    if what == "hide" {
                        sel.more.retain(|x| !m.contains(x));
                        if sel.guid.is_some_and(|x| m.contains(&x)) {
                            select(sel, &[]);
                        }
                    }
                    changed(ch, if what == "hide" { "Layer hidden." } else { "Layer shown." })
                }
                "lock" | "unlock" => {
                    let n = m.iter().filter(|g| d.set_lock(**g, what == "lock").is_ok()).count();
                    said(format!("{} {n}.", if what == "lock" { "Locked" } else { "Unlocked" }))
                }
                "name" => {
                    let name = plain(&w[3..].join(" "), 24);
                    if name.is_empty() {
                        return said("layer <group> name <a name>");
                    }
                    group_note(d, g).insert("name".into(), json!(name));
                    said(format!("Named it {name}."))
                }
                _ => said("layer <group> select|hide|show|lock|unlock|name <name>"),
            };
            tell(out, "groups", groups_line(d))
        }
    }
}

fn members(d: &Den, g: u64) -> Vec<u64> {
    d.props.iter().filter(|p| p.prop.group == Some(g)).map(|p| p.obj.guid).collect()
}

/// `id,name,count,hidden,locked;...`: every group in the Den (the Tools window's layers), at most 40.
pub fn groups_line(d: &Den) -> String {
    let mut ids: Vec<u64> = d.props.iter().filter_map(|p| p.prop.group).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter()
        .take(40)
        .map(|g| {
            let m: Vec<&den::Placed> = d.props.iter().filter(|p| p.prop.group == Some(g)).collect();
            let name = group_name(d, g)
                .or_else(|| d.extra.get("groups").and_then(|x| x.get(g.to_string())).and_then(|x| x.get("stamp")).and_then(Value::as_str).map(str::to_string))
                .unwrap_or_else(|| format!("Group {g}"));
            let hidden = m.iter().all(|p| p.prop.hidden == Some(true));
            let locked = m.iter().all(|p| p.prop.locked == Some(true));
            format!("{g},{},{},{},{};", plain(&name, 24).replace(',', ""), m.len(), hidden as u8, locked as u8)
        })
        .collect()
}

// ---------------------------------------------------------------- measure, history

fn measure(d: &Den, sel: &Sel, here: Here) -> AidOut {
    let all = chosen(d, sel);
    let Some(p) = all.first().and_then(|g| d.placed(*g)) else { return said("Select a prop to measure (or two: the distance between them).") };
    let a = [p.obj.x, p.obj.y, p.obj.z];
    let (b, from) = match all.get(1).and_then(|g| d.placed(*g)) {
        Some(q) if all.len() == 2 => ([q.obj.x, q.obj.y, q.obj.z], name_of(d, q.obj.guid)),
        _ => ([here.x, here.y, here.z], "you"),
    };
    let dist = tools::dist3(a, b);
    let flat = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let bd = gizmo::body_of(&p.obj);
    let (w, dp, h) = (bd.half[0] / 1.05 * 2.0, bd.half[1] / 1.05 * 2.0, bd.height);
    let feet: Vec<[f32; 3]> = all.iter().filter_map(|g| d.placed(*g)).map(|p| [p.obj.x, p.obj.y, p.obj.z]).collect();
    let sp = tools::spread(&feet);
    let group = if all.len() > 2 { format!(" The {} selected spread {:.1} x {:.1} yd, {:.1} high.", all.len(), sp[0], sp[1], sp[2]) } else { String::new() };
    let line = format!(
        "{:.2} yd from {from} ({:.2} on the flat, {:.2} up). {} is {w:.1} x {dp:.1} x {h:.1} yd.{group}",
        dist,
        flat,
        b[2] - a[2],
        name_of(d, p.obj.guid)
    );
    let data = format!("d={dist:.2};flat={flat:.2};up={:.2};w={w:.2};dp={dp:.2};h={h:.2};n={};sx={:.2};sy={:.2};sz={:.2}", b[2] - a[2], all.len(), sp[0], sp[1], sp[2]);
    tell(AidOut { say: vec![line], ..Default::default() }, "measure", data)
}

/// `<undo>|<redo>|name;name;...` (the newest first, at most 30).
fn history_line(d: &Den) -> String {
    let names: Vec<String> = d.history().into_iter().take(30).map(|n| plain(&n, 40).replace(';', "")).collect();
    format!("{}|{}|{}", d.undo_depth(), d.redo_depth(), names.join(";"))
}

fn history(d: &Den) -> AidOut {
    tell(AidOut::default(), "history", history_line(d))
}

// ---------------------------------------------------------------- the stamp library

/// The class a piece is drawn in on the stamp's picture (tools.rs `plan`).
fn class_of(entry: u32) -> u8 {
    let name = den::template(entry).map(|t| t.name).unwrap_or("");
    match ovmod::category(name).unwrap_or("decor") {
        "buildings" | "structure" | "fences" | "tents" => 0,
        "seating" | "tables" | "furniture" => 1,
        "nature" => 3,
        "lights" | "fires" => 4,
        _ => 2,
    }
}

/// The stamp's picture: `cx,cy,w,h,class;...` in a 64 square.
pub fn plan_line(doc: &DenDoc) -> String {
    let pieces: Vec<tools::Piece> = doc
        .props
        .iter()
        .map(|p| {
            let s = crate::stamps::size(p.entry as u32, p.scale.map(|s| s as f32));
            tools::Piece { x: p.x as f32, y: p.y as f32, w: s.w, d: s.d, yaw: p.yaw as f32, class: class_of(p.entry as u32) }
        })
        .collect();
    tools::plan(&pieces, 64.0).iter().map(|r| format!("{},{},{},{},{}", r[0], r[1], r[2], r[3], r[4])).collect::<Vec<_>>().join(";")
}

fn credit_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => plain(s, 40),
        Some(Value::Object(o)) => plain(o.get("by").and_then(Value::as_str).unwrap_or(""), 40),
        _ => String::new(),
    }
}

/// `name,title,category,pieces,origin,credit;...` (origin: `ours` built in, `mine` saved here, `copied` a place
/// copied from the player's game): the gallery's Stamps tab, grouped by category.
pub fn stamp_list(ctx: &Ctx) -> String {
    let mine = den::list_names(&ctx.dens.join(den::MINE_DIR));
    let builtin: Vec<&str> = den::BUILTIN_STAMPS.iter().map(|s| s.0).collect();
    let mut out = String::new();
    for name in den::stamp_names(ctx.stamps).into_iter().take(80) {
        let Ok(doc) = den::read_stamp(ctx.stamps, &name) else { continue };
        let origin = if mine.contains(&name) {
            "mine"
        } else if builtin.contains(&name.as_str()) || ctx.stamps.join(format!("{name}.den.json")).is_file() {
            "ours"
        } else {
            "copied"
        };
        let cat = tools::stamp_category(doc.extra.get("category").and_then(Value::as_str), &name);
        let title = plain(doc.title.as_deref().unwrap_or(&name), 30).replace(',', "");
        let credit = credit_text(doc.extra.get("credit")).replace(',', "");
        out.push_str(&format!("{name},{title},{cat},{},{origin},{credit};", doc.props.len()));
    }
    out
}

fn stamp(d: &mut Den, sel: &mut Sel, w: &[&str], ctx: &Ctx) -> AidOut {
    let sub = w.get(1).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    match sub.as_str() {
        "keep" => {
            *STAGED.lock().unwrap_or_else(|e| e.into_inner()) = None;
            tell(said("Kept. Undo takes the whole stamp back."), "staged", "-".into())
        }
        "cancel" => {
            let s = STAGED.lock().unwrap_or_else(|e| e.into_inner()).take();
            let out = match s {
                Some(s) if d.last_placed() == Some(&s.guids[..]) => changed(d.undo(), "Taken back."),
                Some(_) => said("It was kept (something was done since): Undo takes it back."),
                None => said("No stamp waiting."),
            };
            tell(out, "staged", "-".into())
        }
        "again" => {
            let s = STAGED.lock().unwrap_or_else(|e| e.into_inner()).clone();
            match s {
                Some(s) => stage(d, ctx, &s.name, s.at, s.o, xf_of(w, 2)),
                None => said("Put a stamp down first (pick one in Stamps, then click the ground)."),
            }
        }
        "save" => save_stamp(d, sel, w, ctx),
        "delete" => {
            let Some(name) = w.get(2).and_then(|n| den::clean_name(n)) else { return said("stamp delete <name>") };
            let p = ctx.dens.join(den::MINE_DIR).join(format!("{name}.den.json"));
            if !p.is_file() {
                return said("Only a stamp you saved here can be deleted.");
            }
            match std::fs::remove_file(&p) {
                Ok(()) => tell(said(format!("Deleted your stamp {name}.")), "stamps", stamp_list(ctx)),
                Err(_) => said("That stamp can't be deleted."),
            }
        }
        _ => said("stamp save | delete | keep | cancel | again (and the Stamps tab's click)"),
    }
}

/// The selection as a stamp (`<dens>/stamps-mine/<name>.den.json`): round its middle, the lowest foot at 0; the
/// credit is the builder's, and the credits of stamps its pieces came from are kept beside it.
pub fn save_stamp(d: &Den, sel: &Sel, w: &[&str], ctx: &Ctx) -> AidOut {
    let Some(name) = w.get(2).and_then(|n| den::clean_name(n)) else {
        return said("Stamp names are letters, digits, - and _ (up to 24).");
    };
    if den::BUILTIN_STAMPS.iter().any(|s| s.0 == name) {
        return said(format!("{name} is one of Leyline's own stamps: pick another name."));
    }
    let all = chosen(d, sel);
    if all.is_empty() {
        return said("Select what goes in the stamp first (the Box, or Shift+click).");
    }
    if all.len() > den::LOCAL_MAX {
        return said(format!("A stamp holds up to {} props.", den::LOCAL_MAX));
    }
    let placed: Vec<&den::Placed> = all.iter().filter_map(|g| d.placed(*g)).collect();
    let mid = tools::centre_of(&placed.iter().map(|p| [p.obj.x, p.obj.y, p.obj.z]).collect::<Vec<_>>());
    let props: Vec<Prop> = placed
        .iter()
        .map(|p| {
            let mut c = copy_of(&p.prop);
            c.hidden = None;
            c.x = ((p.obj.x - mid[0]) as f64 * 1000.0).round() / 1000.0;
            c.y = ((p.obj.y - mid[1]) as f64 * 1000.0).round() / 1000.0;
            c.z = ((p.obj.z - mid[2]) as f64 * 1000.0).round() / 1000.0;
            c
        })
        .collect();
    if props.iter().any(|p| p.x.abs() > den::STAMP_SPAN || p.y.abs() > den::STAMP_SPAN || p.z.abs() > den::STAMP_SPAN) {
        return said(format!("A stamp fits in {} yards each way from its middle: select less.", den::STAMP_SPAN));
    }
    // the stamps its pieces came from (their groups' notes): their credits are kept
    let mut from: Vec<Value> = Vec::new();
    for g in placed.iter().filter_map(|p| p.prop.group) {
        if let Some(n) = d.extra.get("groups").and_then(|x| x.get(g.to_string())) {
            if let Some(st) = n.get("stamp") {
                let e = json!({ "stamp": st, "credit": n.get("credit").cloned().unwrap_or(Value::Null) });
                if !from.contains(&e) {
                    from.push(e);
                }
            }
        }
    }
    let cat = w.get(3).map(|c| tools::stamp_category(Some(*c), &name)).unwrap_or("mine");
    let title = plain(&w.iter().skip(4).copied().collect::<Vec<_>>().join(" "), 30);
    let mut extra = Map::new();
    extra.insert("category".into(), json!(cat));
    extra.insert("credit".into(), json!({ "by": plain(ctx.who, 24), "made": den::iso_utc(ctx.unix) }));
    if !from.is_empty() {
        extra.insert("from".into(), Value::Array(from));
    }
    let doc = DenDoc {
        format: den::FORMAT.into(),
        version: den::VERSION,
        kind: Some("stamp".into()),
        name: Some(name.clone()),
        title: Some(if title.is_empty() { name.clone() } else { title }),
        saved: Some(den::iso_utc(ctx.unix)),
        props,
        extra,
        ..Default::default()
    };
    let dir = ctx.dens.join(den::MINE_DIR);
    let text = match serde_json::to_string_pretty(&doc) {
        Ok(t) => t,
        Err(e) => return said(e.to_string()),
    };
    let path = dir.join(format!("{name}.den.json"));
    let tmp = dir.join(format!(".{name}.den.json.tmp"));
    let ok = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&tmp, text.as_bytes())).and_then(|_| std::fs::rename(&tmp, &path));
    if ok.is_err() {
        return said("That stamp can't be written in your Dens folder.");
    }
    let mut out = said(format!("Saved {} props as the stamp {name} (Stamps > Mine).", doc.props.len()));
    out.addon.push(("stamps".into(), stamp_list(ctx)));
    out.addon.push(("stampplan".into(), format!("{name}|{}", plan_line(&doc))));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::den::Spot;

    fn here() -> Here {
        let s = Spot::default();
        Here { x: s.x, y: s.y, z: s.z, o: 0.0 }
    }

    fn ctx(dir: &Path) -> Ctx<'_> {
        Ctx { dens: dir, stamps: dir, who: "Orion", unix: 1_790_000_000 }
    }

    fn put(d: &mut Den, entry: u32, dx: f32, dy: f32) -> u64 {
        let s = d.spot;
        d.spawn(entry, s.x + dx, s.y + dy, s.z, 0.0).unwrap().guid
    }

    fn run(d: &mut Den, sel: &mut Sel, text: &str) -> AidOut {
        let dir = std::env::temp_dir();
        cmd(d, sel, &mut Aids::new(gizmo::Camera::default()), here(), text, true, &ctx(&dir))
    }

    #[test]
    fn the_tools_verbs_are_known_and_said_in_caps() {
        assert!(is_tool("path lay 2413 3 1 along 0 0 0"));
        assert!(is_tool("ALIGN x") && is_tool("tool point 0 0 1.7 12"));
        assert!(!is_tool("move x 1") && !is_tool("hover 0 0") && !is_tool(""));
        assert!(CAPS.split(',').all(|c| !gizmo::is_aid(c)), "none of them is a placement aid's");
    }

    #[test]
    fn a_path_is_laid_on_the_ground_as_one_group_and_one_undo() {
        let mut d = Den::new(Spot::default());
        let mut sel = Sel::default();
        let s = d.spot;
        path_point(&d, &mut sel, [s.x, s.y, s.z], None);
        let out = path_point(&d, &mut sel, [s.x + 9.0, s.y, s.z], None);
        assert!(out.addon.iter().any(|a| a.0 == "pathpts" && a.1.starts_with("2|0|9.0|")), "{:?}", out.addon);
        assert_eq!(out.create.len(), 2, "a stake at each point");
        let out = run(&mut d, &mut sel, "path lay 2413 3 1 along 0 0 0");
        let ch = out.change.expect("a change");
        assert_eq!(ch.created.len(), 4, "0, 3, 6, 9");
        assert_eq!(out.destroy.len(), tools::POINTS_MAX, "the stakes go");
        assert_eq!(d.props.len(), 4);
        let g = d.props[0].prop.group;
        assert!(g.is_some() && d.props.iter().all(|p| p.prop.group == g), "one group");
        assert_eq!(d.paths.len(), 1, "the path kept with its points");
        assert_eq!(chosen(&d, &sel).len(), 4, "the new path selected");
        assert!(sel.path.is_empty(), "the points used");
        d.undo();
        assert!(d.props.is_empty(), "one undo takes it all");
        // the points: one moved, the last taken away, all cleared
        path_point(&d, &mut sel, [s.x, s.y, s.z], None);
        path_point(&d, &mut sel, [s.x + 1.0, s.y, s.z], None);
        path_point(&d, &mut sel, [s.x + 5.0, s.y, s.z], Some(0));
        assert_eq!(sel.path[0], [s.x + 5.0, s.y]);
        run(&mut d, &mut sel, "path undo");
        assert_eq!(sel.path.len(), 1);
        run(&mut d, &mut sel, "path clear");
        assert!(sel.path.is_empty());
    }

    #[test]
    fn a_path_is_edited_and_laid_again() {
        let mut d = Den::new(Spot::default());
        let mut sel = Sel::default();
        let s = d.spot;
        path_point(&d, &mut sel, [s.x, s.y, s.z], None);
        path_point(&d, &mut sel, [s.x + 4.0, s.y, s.z], None);
        run(&mut d, &mut sel, "path lay 2413 2 0 along 0 0 0");
        assert_eq!(d.props.len(), 3);
        let g = d.props[0].prop.group.unwrap();
        let out = run(&mut d, &mut sel, "path edit");
        let line = &out.addon.iter().find(|a| a.0 == "pathedit").expect("the path back").1;
        assert_eq!(line, &format!("{g}|2413|2|0|along|0|0|0"));
        assert_eq!(sel.path.len(), 2, "its points back");
        assert!(!out.create.is_empty(), "its stakes shown");
        path_point(&d, &mut sel, [s.x + 8.0, s.y, s.z], Some(1));
        run(&mut d, &mut sel, "path lay 2413 2 0 along 0 0 0");
        assert_eq!(d.props.len(), 5, "longer: five, in place of the three");
        assert_eq!(d.paths.len(), 1, "still one path");
        d.undo();
        assert_eq!(d.props.len(), 3, "undo: the old path back");
    }

    #[test]
    fn arrays_and_rings_are_one_undo_and_rings_face_in() {
        let mut d = Den::new(Spot::default());
        let mut sel = Sel { guid: Some(put(&mut d, 2413, 0.0, 0.0)), ..Default::default() };
        run(&mut d, &mut sel, "array 3 2 0 0");
        assert_eq!(d.props.len(), 4);
        d.undo();
        assert_eq!(d.props.len(), 1);
        run(&mut d, &mut sel, "ring 4 5");
        assert_eq!(d.props.len(), 5);
        let first = &d.props[1];
        assert!((first.prop.x - 5.0).abs() < 0.01 && (first.prop.yaw - 180.0).abs() < 0.5, "{:?}", first.prop);
        assert!(run(&mut d, &mut sel, "ring 4 500").change.is_none(), "too far out: refused");
    }

    #[test]
    fn align_and_distribute_the_selection() {
        let mut d = Den::new(Spot::default());
        let a = put(&mut d, 2413, 0.0, 0.0);
        let b = put(&mut d, 2413, 3.0, 2.0);
        let c = put(&mut d, 2413, 10.0, -1.0);
        let mut sel = Sel { guid: Some(a), more: vec![b, c], ..Default::default() };
        run(&mut d, &mut sel, "align y");
        assert!(d.props.iter().all(|p| p.prop.y.abs() < 0.01), "all on the first's y");
        run(&mut d, &mut sel, "distribute x");
        let xs: Vec<f64> = d.props.iter().map(|p| p.prop.x).collect();
        assert!((xs[1] - 5.0).abs() < 0.01, "{xs:?}");
        d.undo();
        assert!((d.props[1].prop.x - 3.0).abs() < 0.01, "one undo");
        let mut one = Sel { guid: Some(a), ..Default::default() };
        assert!(run(&mut d, &mut one, "align x").change.is_none(), "two needed");
    }

    #[test]
    fn solid_off_is_the_walk_through_twin_and_undo_puts_it_back() {
        let mut d = Den::new(Spot::default());
        let g = put(&mut d, 2413, 0.0, 0.0);
        let mut sel = Sel { guid: Some(g), ..Default::default() };
        run(&mut d, &mut sel, "solid off");
        let p = d.placed(g).unwrap();
        assert_eq!(p.prop.solid, Some(false));
        assert!(tools::is_pass(p.obj.entry) && p.obj.kind == tools::DOOR, "told the game it is an open door");
        assert_eq!(p.obj.guid, g, "the same object");
        assert!(state_extra(&d, &sel).starts_with("solid=off;coll="));
        assert_eq!(den::template(p.obj.entry).map(|t| t.kind), Some(tools::DOOR), "its query answer is a door's");
        d.undo();
        assert!(!tools::is_pass(d.placed(g).unwrap().obj.entry));
    }

    #[test]
    fn the_guard_keeps_you_on_a_solid_prop_and_out_of_the_ground() {
        let mut d = Den::new(Spot::default());
        let g = put(&mut d, 2413, 0.0, 0.0);
        let s = d.spot;
        let top = tops(&d, true, &[])[0].top;
        // not marked Solid: the game's own business
        assert_eq!(guard(&d, [s.x, s.y, top], [s.x, s.y, top - 1.0], 0), None);
        let mut sel = Sel { guid: Some(g), ..Default::default() };
        run(&mut d, &mut sel, "solid on");
        assert_eq!(guard(&d, [s.x, s.y, top], [s.x, s.y, top - 1.0], 0), Some(top + tools::LIFT));
        assert_eq!(guard(&d, [s.x, s.y, top], [s.x, s.y, top - 1.0], crate::movewatch::FLYING), None, "flying: free");
        // through the ground of the Den's island
        let fell = guard(&d, [s.x + 6.0, s.y, s.z], [s.x + 6.0, s.y, s.z - 20.0], 0);
        assert_eq!(fell, Some(d.floor_at(s.x + 6.0, s.y) + tools::LIFT));
    }

    #[test]
    fn hide_show_and_groups_as_layers() {
        let mut d = Den::new(Spot::default());
        let a = put(&mut d, 2413, 0.0, 0.0);
        let b = put(&mut d, 2413, 2.0, 0.0);
        let mut sel = Sel { guid: Some(a), more: vec![b], ..Default::default() };
        let out = run(&mut d, &mut sel, "group Porch");
        let line = &out.addon.iter().find(|x| x.0 == "groups").unwrap().1;
        let g = d.placed(a).unwrap().prop.group.unwrap();
        assert_eq!(line, &format!("{g},Porch,2,0,0;"));
        let out = run(&mut d, &mut sel, &format!("layer {g} hide"));
        assert_eq!(out.change.unwrap().destroyed.len(), 2);
        assert_eq!(d.everything().len(), d.room.len(), "hidden: not sent");
        assert!(sel.guid.is_none(), "a hidden prop isn't selected");
        let out = run(&mut d, &mut sel, "show");
        assert_eq!(out.change.unwrap().created.len(), 2);
        let mut one = Sel { guid: Some(b), ..Default::default() };
        run(&mut d, &mut one, "selgroup");
        assert_eq!(chosen(&d, &one).len(), 2);
    }

    #[test]
    fn history_and_undo_to_a_point() {
        let mut d = Den::new(Spot::default());
        let mut sel = Sel::default();
        put(&mut d, 2413, 0.0, 0.0);
        put(&mut d, 2413, 2.0, 0.0);
        put(&mut d, 2413, 4.0, 0.0);
        let h = history_line(&d);
        assert!(h.starts_with("3|0|Placed "), "{h}");
        let out = run(&mut d, &mut sel, "undoto 2");
        assert_eq!(d.props.len(), 1);
        assert_eq!(out.change.unwrap().destroyed.len(), 2);
        assert_eq!(d.redo_depth(), 2);
    }

    #[test]
    fn merged_changes_end_as_the_den_does() {
        let o = |g: u64| den::WorldObject { guid: g, entry: 1, display: 1, kind: 5, scale: 1.0, x: 0.0, y: 0.0, z: 0.0, o: 0.0, pitch: 0.0, roll: 0.0 };
        let a = Change { created: vec![o(1), o(2)], destroyed: vec![9], ..Default::default() };
        let b = Change { created: vec![o(2), o(3)], destroyed: vec![1], ..Default::default() };
        let m = merge(a, b);
        assert_eq!(m.destroyed, vec![9, 1]);
        assert_eq!(m.created.iter().map(|o| o.guid).collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn a_selection_saved_as_a_stamp_with_its_credit() {
        // the Dens folder and, as the stamps folder, its own stamps' (no global folders set: tests run side by side)
        let dir = std::env::temp_dir().join(format!("den-tools-{}", std::process::id()));
        let mine = dir.join(den::MINE_DIR);
        let _ = std::fs::create_dir_all(&mine);
        let cx = Ctx { dens: &dir, stamps: &mine, who: "Orion", unix: 1_790_000_000 };
        let mut d = Den::new(Spot::default());
        let a = put(&mut d, 2413, 0.0, 0.0);
        let b = put(&mut d, 2413, 4.0, 2.0);
        let sel = Sel { guid: Some(a), more: vec![b], ..Default::default() };
        let out = save_stamp(&d, &sel, &["stamp", "save", "porch", "gardens", "My", "porch"], &cx);
        assert!(out.say[0].starts_with("Saved 2 props"), "{:?}", out.say);
        let doc = den::read_stamp(&mine, "porch").expect("read back");
        assert_eq!(doc.title.as_deref(), Some("My porch"));
        assert_eq!(doc.extra.get("credit").and_then(|c| c.get("by")).and_then(Value::as_str), Some("Orion"));
        assert!((doc.props[0].x + 2.0).abs() < 0.01 && (doc.props[1].y - 1.0).abs() < 0.01, "round its middle");
        assert!(stamp_list(&cx).contains("porch,My porch,gardens,2,mine,Orion;"), "{}", stamp_list(&cx));
        assert!(!plan_line(&doc).is_empty());
        assert!(save_stamp(&d, &sel, &["stamp", "save", "garden"], &cx).say[0].contains("Leyline's own"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stamp_mirrored_and_sized_before_it_goes_down() {
        let doc = DenDoc { props: vec![Prop { entry: 2413, x: 3.0, y: 1.0, yaw: 30.0, ..Default::default() }], ..Default::default() };
        let t = transformed(&doc, &Xf { mirror: Mirror::LeftRight, scale: 2.0, turn: 0.0 });
        assert_eq!((t.props[0].x, t.props[0].y, t.props[0].yaw), (6.0, -2.0, 330.0));
        assert!(t.props[0].scale.is_some());
        assert_eq!(xf_of(&["tool", "stamp", "garden", "0", "0", "1.7", "12", "45", "fb", "1.5"], 7), Xf { turn: 45.0, mirror: Mirror::FrontBack, scale: 1.5 });
    }
}
