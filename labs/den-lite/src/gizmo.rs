//! Placement aids for the Den's editor (owner 3 Nov 10:29 - 10:36: "when mousing over ground in game, it shows a
//! preview of where it will land", "show the actual prop moving around ... a live preview", "the 3 directional
//! arrows", "hovering over an object makes it glow; you can click it, select it").
//!
//! The stock 3.3.5a client has no cursor-to-world call for addons, so the aids are built from what it does have:
//!
//! * **The live preview (the ghost)**: while a prop is armed (`wubuild cmd preview <entry>`, or `preview move` for
//!   the selected prop) Den Lite shows a copy of the real prop (same entry and model, a guid of its own, not
//!   selectable). The addon (Gizmo.lua) puts the camera in a known pose (the build camera) and sends where the cursor
//!   is on the screen (`hover <u> <v> <aspect> [<distance>]`, u and v from -1 to 1, about 10 a second); Den Lite,
//!   which knows where the player stands and faces (their movement packets), casts the ray from the camera through
//!   that point onto the ground ([`project`]) and moves the copy there, snapped to the grid and set on the ground.
//!   Before the first hover (or after `follow`) the copy stands 2.5 yards in front of the player and follows them.
//!   `commit` (a left click on the world, Place) puts the real prop exactly where the copy is; the aid stays armed
//!   until `preview off` (Esc, a right click).
//!   A gameobject's position can't be changed after it is created in 3.3.5a (its movement block is sent only with
//!   the create; the values have no position fields, unlike 1.12's GAMEOBJECT_POS_X ..), so each move is a create of
//!   the copy at the new place under the other of two guids and then a destroy of the old one: never a frame without
//!   it (the editor's own moves destroy and create the same guid, with a frame between).
//! * **The exact click (the ground-target cursor)**: the player knows one ground-targeted spell ([`PLACE_SPELL`],
//!   Flare, in the initial spells; no mana for a class without it, so the client's own cost check passes). Gizmo.lua's
//!   secure button (and a key while armed) starts it: the client's own targeting circle follows the mouse and a
//!   click sends CMSG_CAST_SPELL with the exact world point (TARGET_FLAG_DEST_LOCATION). Den Lite never casts it:
//!   it places (or moves) the armed prop there, answers SPELL_FAILED_DONT_REPORT (no message, no cooldown, no
//!   visual) and clears the cooldown. The same click, with the last hover, is one calibration sample for the build
//!   camera ([`Sample`], `<dens>/den-lite.camera-samples`; den-check fits [`Camera`] from them).
//! * **The gizmo**: around the selected prop three small flags (red X, green Y, blue Z: [`GIZMO`]); a right click on
//!   one (CMSG_GAMEOBJ_USE) picks that axis; the mouse wheel (Gizmo.lua's override bindings, only while an axis is
//!   picked) then sends the Builder's own `move <axis> <step>`. They follow the prop and go with the selection.
//! * **Select mode**: `mode build` shows every prop as a goober (GAMEOBJECT_BYTES_1's type byte), which the client
//!   highlights under the mouse and lets you right-click (CMSG_GAMEOBJ_USE: the prop is selected, the gizmo comes
//!   up, the addon opens its menu at the cursor: `wubuild picked <id>`). `mode play` shows them as they are (a chair
//!   sits, decor does nothing). The template's own type (the query answer, which the client caches) never changes.
//!
//! Everything here is pure (the world server sends what it returns), so it is tested without a client.

use crate::den::{self, Change, Den, WorldObject};
use crate::ovmod::{Here, Sel};
use serde::{Deserialize, Serialize};

/// The ground-targeted spell the player knows for the exact click: Flare (a circle that follows the mouse).
pub const PLACE_SPELL: u32 = 1543;
pub const SMSG_INITIAL_SPELLS: u16 = 0x012A;
pub const CMSG_CAST_SPELL: u32 = 0x012E;
pub const SMSG_CAST_FAILED: u16 = 0x0130;
pub const SMSG_CLEAR_COOLDOWN: u16 = 0x01DE;
pub const CMSG_GAMEOBJ_USE: u32 = 0x00B1;
pub const CMSG_GAMEOBJ_REPORT_USE: u32 = 0x0481;
/// SpellCastResult SPELL_FAILED_DONT_REPORT (3.3.5a): the client says nothing.
pub const SPELL_FAILED_DONT_REPORT: u8 = 0x1A;
/// TARGET_FLAG_DEST_LOCATION.
pub const DEST_LOCATION: u32 = 0x40;
pub const GENERIC: u8 = 5;
pub const GOOBER: u8 = 10;
/// GO_FLAG_NOT_SELECTABLE: the client neither highlights nor uses it.
pub const NOT_SELECTABLE: u32 = 0x10;

/// The copy stands this far in front of the player before the cursor says where.
pub const AHEAD: f32 = 2.5;
/// At most this often (ms) the copy is shown at a new place (about 10 a second); the place itself is always kept.
pub const DRAW_MS: u64 = 100;
/// The farthest the ray may land from the player (yards).
pub const REACH: f32 = 60.0;

const HIGH_GAMEOBJECT: u64 = 0xF110;
/// The aids' own object counters (never a prop's: those count up from 1): the copy 0 and 1, the gizmo 0x10 ..= 0x15.
const AID_BASE: u64 = 0xFF_FF00;

/// The gizmo's flags: (entry of ours, display, name, axis). Displays from AzerothCore's gameobject_template: the
/// red Warsong Flag (5913), the green Netherstorm Flag (7153), the blue Silverwing Flag (5912).
pub const GIZMO: [(u32, u32, &str, char); 3] = [(990_001, 5913, "Move along X", 'x'), (990_002, 7153, "Move along Y", 'y'), (990_003, 5912, "Move along Z", 'z')];
const GIZMO_SCALE: f32 = 0.5;
const GIZMO_OFF: f32 = 1.8;

/// The query answer for the gizmo's entries (type goober: right-click uses it).
pub fn query(entry: u32) -> Option<(u8, u32, &'static str, f32)> {
    GIZMO.iter().find(|g| g.0 == entry).map(|g| (GOOBER, g.1, g.2, GIZMO_SCALE))
}

fn aid_guid(entry: u32, n: u64) -> u64 {
    (HIGH_GAMEOBJECT << 48) | ((entry as u64 & 0xFF_FFFF) << 24) | (AID_BASE + n)
}

fn counter(g: u64) -> u64 {
    g & 0xFF_FFFF
}

pub fn is_ghost(g: u64) -> bool {
    matches!(counter(g).checked_sub(AID_BASE), Some(0 | 1))
}

/// The gizmo flag's axis, for its guid.
pub fn gizmo_axis(g: u64) -> Option<char> {
    let n = counter(g).checked_sub(AID_BASE)?;
    (0x10..0x16).contains(&n).then(|| GIZMO[((n - 0x10) % 3) as usize].3)
}

/// The type and flags the client is told for an object (GAMEOBJECT_BYTES_1's type byte, GAMEOBJECT_FLAGS): the copy
/// plain and not selectable; the gizmo goobers; in build mode every prop a goober (highlighted, right-click uses it).
pub fn shown(o: &WorldObject, build: bool) -> (u8, u32) {
    if is_ghost(o.guid) {
        (GENERIC, NOT_SELECTABLE)
    } else if gizmo_axis(o.guid).is_some() || build {
        (GOOBER, 0)
    } else {
        (o.kind, 0)
    }
}

/// The build camera: where it is against the player (Gizmo.lua sets it on entering the build mode). Calibrated on
/// the owner's PC by den-check's hover calibration (`<dens>/den-lite.camera.json`); these are first guesses.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    /// The field of view across the screen's diagonal (radians).
    pub fov: f32,
    /// The point the camera looks at: this high over the player's feet (yards).
    pub pivot: f32,
    /// How far behind that point (yards; the addon's hover says it when it set the zoom).
    pub dist: f32,
    /// How far it looks down (radians).
    pub pitch: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { fov: std::f32::consts::FRAC_PI_2, pivot: 2.0, dist: 12.0, pitch: 0.5 }
    }
}

pub const CAMERA_FILE: &str = "den-lite.camera.json";
pub const SAMPLES_FILE: &str = "den-lite.camera-samples";

impl Camera {
    /// The calibrated camera from the Dens folder, else the first guesses.
    pub fn load(dir: &std::path::Path) -> Camera {
        std::fs::read_to_string(dir.join(CAMERA_FILE))
            .ok()
            .and_then(|t| serde_json::from_str::<Camera>(&t).ok())
            .filter(|c| [c.fov, c.pivot, c.dist, c.pitch].iter().all(|v| v.is_finite()) && c.fov > 0.1 && c.fov < 3.0)
            .unwrap_or_default()
    }
}

fn add(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Where the cursor at (u, v) on the screen (-1 .. 1, right and up; `aspect` = width / height) points on the ground
/// at height `floor`: the camera `cam.dist` behind and above the point `cam.pivot` over the player, facing as they
/// face, looking down by `cam.pitch`. None: the ray doesn't come down within [`REACH`].
pub fn project(cam: &Camera, here: Here, u: f32, v: f32, aspect: f32, floor: f32) -> Option<(f32, f32)> {
    if ![u, v, aspect, floor].iter().all(|x| x.is_finite()) {
        return None;
    }
    let (so, co) = here.o.sin_cos();
    let fwd = [co, so, 0.0];
    let right = [so, -co, 0.0];
    let (sp, cp) = cam.pitch.sin_cos();
    let pivot = [here.x, here.y, here.z + cam.pivot];
    let eye = [pivot[0] - cam.dist * cp * fwd[0], pivot[1] - cam.dist * cp * fwd[1], pivot[2] + cam.dist * sp];
    let look = [cp * fwd[0], cp * fwd[1], -sp];
    let up = cross(right, look);
    let a = aspect.clamp(0.2, 5.0);
    let tv = (cam.fov / 2.0).tan() / (1.0 + a * a).sqrt();
    let d = add(add(look, right, u.clamp(-1.0, 1.0) * tv * a), up, v.clamp(-1.0, 1.0) * tv);
    if d[2] >= -1e-4 {
        return None;
    }
    let t = (floor - eye[2]) / d[2];
    if t <= 0.0 {
        return None;
    }
    let p = add(eye, d, t);
    let far = ((p[0] - here.x).powi(2) + (p[1] - here.y).powi(2)).sqrt();
    (far <= REACH).then_some((p[0], p[1]))
}

/// What is armed: a catalog prop to place, or the selected prop to move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Armed {
    Place(u32),
    Move(u64),
}

/// One session's aids.
#[derive(Debug, Clone, Default)]
pub struct Aids {
    pub gizmo_on: bool,
    pub axis: Option<char>,
    /// The gizmo flags shown now, and for which prop at which place.
    pub gizmo: Vec<u64>,
    gizmo_for: Option<(u64, [i32; 3])>,
    gizmo_flip: bool,
    pub armed: Option<Armed>,
    /// The copy shown now.
    pub ghost: Option<WorldObject>,
    flip: bool,
    /// Where Commit puts it: x, y, z, facing (radians).
    pub at: Option<[f32; 4]>,
    drawn_ms: u64,
    /// The last hover: u, v, aspect (None: the copy follows the player).
    pub cursor: Option<[f32; 3]>,
    /// The copy's height over the ground (a moved prop keeps its own) and its turn.
    pub lift: f32,
    pub yaw: f32,
    pub cam: Camera,
}

impl Aids {
    pub fn new(cam: Camera) -> Aids {
        Aids { cam, ..Default::default() }
    }
}

/// What the world server does for the aids: in this order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AidOut {
    /// A change to the Den (for everyone).
    pub change: Option<Change>,
    /// Build mode on or off: every prop shown again (for everyone).
    pub look: Option<bool>,
    /// This player's own objects: made first, then the old ones taken away.
    pub create: Vec<WorldObject>,
    pub destroy: Vec<u64>,
    /// `wubuild <name> <data>` to the addon.
    pub addon: Vec<(String, String)>,
    pub say: Vec<String>,
    /// The Builder's state line too.
    pub state: bool,
}

impl AidOut {
    fn tell(mut self, name: &str, data: String) -> Self {
        self.addon.push((name.into(), data));
        self
    }
    fn said(mut self, s: impl Into<String>) -> Self {
        self.say.push(s.into());
        self
    }
    fn merge(&mut self, o: AidOut) {
        if o.change.is_some() {
            self.change = o.change;
        }
        if o.look.is_some() {
            self.look = o.look;
        }
        self.create.extend(o.create);
        self.destroy.extend(o.destroy);
        self.addon.extend(o.addon);
        self.say.extend(o.say);
        self.state |= o.state;
    }
}

fn snap(g: f64, v: f32, mid: f32) -> f32 {
    if g > 0.0 { mid + (((v - mid) as f64 / g).round() * g) as f32 } else { v }
}

/// The aids' verbs (`wubuild cmd <verb> ...`); everything else is the Builder's.
pub fn is_aid(text: &str) -> bool {
    let v = text.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    matches!(v.as_str(), "preview" | "hover" | "follow" | "commit" | "axis" | "gizmo" | "mode" | "where")
}

/// The addon's `aim` line: what is armed (`place,<entry>`, `move,<prop id>` or `off`) and the exact click's spell.
fn aim(a: &Aids) -> String {
    match a.armed {
        Some(Armed::Place(e)) => format!("place,{e},{PLACE_SPELL}"),
        Some(Armed::Move(g)) => format!("move,{},{PLACE_SPELL}", g & 0xFF_FFFF),
        None => "off".into(),
    }
}

/// One aid verb. `now`: milliseconds (the copy's throttle). `can`: may build.
pub fn cmd(d: &mut Den, sel: &mut Sel, a: &mut Aids, here: Here, text: &str, can: bool, now: u64) -> AidOut {
    let w: Vec<&str> = text.split_whitespace().collect();
    let verb = w.first().map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let arg = w.get(1).map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let num = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
    let builds = matches!(verb.as_str(), "preview" | "commit") && arg != "off";
    if builds && !can {
        return AidOut::default().said("Only the Den's owner builds here (they can let friends: .den friends build on).");
    }
    match verb.as_str() {
        "preview" => match arg.as_str() {
            "off" | "cancel" => cancel(d, a),
            "turn" => {
                a.yaw = (a.yaw + num(2).unwrap_or(15.0).to_radians()).rem_euclid(std::f32::consts::TAU);
                if let Some(at) = a.at.as_mut() {
                    at[3] = a.yaw;
                }
                draw(d, a, now, true)
            }
            "move" => {
                let Some(p) = sel.guid.and_then(|g| d.placed(g)).cloned() else {
                    return AidOut::default().said("Select a prop first (right-click it in build mode, or Near).");
                };
                if p.prop.locked == Some(true) {
                    return AidOut::default().said(den::LOCKED);
                }
                let mut out = cancel(d, a);
                out.addon.clear();
                a.armed = Some(Armed::Move(p.obj.guid));
                a.lift = p.obj.z - d.floor_at(p.obj.x, p.obj.y);
                a.yaw = p.obj.o;
                a.at = Some([p.obj.x, p.obj.y, p.obj.z, p.obj.o]);
                a.cursor = None;
                // the real one hidden while its copy moves (this player only); Commit or Esc shows it again
                out.destroy.push(p.obj.guid);
                out.merge(draw(d, a, now, true));
                out.tell("aim", aim(a))
            }
            _ => match arg.parse::<u32>().ok().filter(|e| den::template(*e).is_some()) {
                Some(entry) => {
                    let mut out = cancel(d, a);
                    out.addon.clear();
                    a.armed = Some(Armed::Place(entry));
                    a.lift = 0.0;
                    a.yaw = here.o;
                    a.cursor = None;
                    a.at = Some(ahead(d, sel, here));
                    out.merge(draw(d, a, now, true));
                    out.tell("aim", aim(a))
                }
                None => AidOut::default().said("Pick a prop in the gallery first."),
            },
        },
        "hover" => {
            let (Some(u), Some(v)) = (num(1), num(2)) else { return AidOut::default() };
            if a.armed.is_none() {
                return AidOut::default();
            }
            let aspect = num(3).unwrap_or(16.0 / 9.0);
            let mut cam = a.cam;
            if let Some(dist) = num(4).filter(|d| (0.0..=50.0).contains(d)) {
                cam.dist = dist;
            }
            a.cursor = Some([u, v, aspect]);
            // the ground under the player first, then under where the ray lands (twice: the ground may slope)
            let mut floor = d.floor_at(here.x, here.y);
            let mut hit = None;
            for _ in 0..2 {
                hit = project(&cam, here, u, v, aspect, floor);
                if let Some((x, y)) = hit {
                    floor = d.floor_at(x, y);
                }
            }
            match hit {
                Some((x, y)) => {
                    set_at(d, sel, a, x, y);
                    draw(d, a, now, false)
                }
                None => AidOut::default(),
            }
        }
        "follow" => {
            a.cursor = None;
            if a.armed.is_some() {
                a.at = Some(ahead(d, sel, here));
                return draw(d, a, now, true);
            }
            AidOut::default()
        }
        "commit" => commit(d, sel, a, now),
        "axis" => {
            a.axis = match arg.as_str() {
                "x" | "y" | "z" => arg.chars().next(),
                _ => None,
            };
            AidOut::default().tell("axis", a.axis.map(String::from).unwrap_or_else(|| "-".into()))
        }
        "gizmo" => {
            a.gizmo_on = arg != "off";
            if !a.gizmo_on {
                a.axis = None;
            }
            sync(d, sel, a)
        }
        "mode" => {
            let build = arg == "build";
            let mut out = if build { AidOut::default() } else { cancel(d, a) };
            out.look = Some(build);
            out.tell("mode", if build { "build".into() } else { "play".into() })
        }
        "where" => AidOut::default().tell("ghost", where_line(a)),
        _ => AidOut::default(),
    }
}

/// `x,y,z,deg` of where Commit goes, or `-`.
pub fn where_line(a: &Aids) -> String {
    match a.at.filter(|_| a.armed.is_some()) {
        Some(p) => format!("{:.2},{:.2},{:.2},{:.0}", p[0], p[1], p[2], p[3].to_degrees()),
        None => "-".into(),
    }
}

/// In front of the player, on the grid and the ground.
fn ahead(d: &Den, sel: &Sel, here: Here) -> [f32; 4] {
    let (x, y) = (here.x + AHEAD * here.o.cos(), here.y + AHEAD * here.o.sin());
    let (x, y) = (snap(sel.grid, x, d.spot.x), snap(sel.grid, y, d.spot.y));
    [x, y, d.floor_at(x, y), here.o]
}

fn set_at(d: &Den, sel: &Sel, a: &mut Aids, x: f32, y: f32) {
    let (x, y) = (snap(sel.grid, x, d.spot.x), snap(sel.grid, y, d.spot.y));
    a.at = Some([x, y, d.floor_at(x, y) + a.lift, a.yaw]);
}

/// The player moved: the copy that isn't following the cursor follows them.
pub fn follow_player(d: &Den, sel: &Sel, a: &mut Aids, here: Here, now: u64) -> AidOut {
    if a.armed.is_none() || a.cursor.is_some() {
        return AidOut::default();
    }
    let mut at = ahead(d, sel, here);
    at[2] += a.lift;
    if matches!(a.armed, Some(Armed::Place(_))) {
        a.yaw = here.o;
    }
    at[3] = a.yaw;
    a.at = Some(at);
    draw(d, a, now, false)
}

/// The copy at `a.at`: made under the other guid, then the old one taken away. Not more often than [`DRAW_MS`]
/// unless `force`.
fn draw(d: &Den, a: &mut Aids, now: u64, force: bool) -> AidOut {
    let mut out = AidOut::default();
    let (Some(armed), Some(at)) = (a.armed, a.at) else { return out };
    if !force && now.saturating_sub(a.drawn_ms) < DRAW_MS {
        return out;
    }
    if !d.inside(at[0], at[1], at[2]) {
        return out;
    }
    let base = match armed {
        Armed::Place(e) => match den::template(e) {
            Some(t) => WorldObject { guid: 0, entry: e, display: t.display, kind: t.kind, scale: t.size, x: 0.0, y: 0.0, z: 0.0, o: 0.0, pitch: 0.0, roll: 0.0 },
            None => return out,
        },
        Armed::Move(g) => match d.placed(g) {
            Some(p) => p.obj.clone(),
            None => return out,
        },
    };
    a.flip = !a.flip;
    let obj = WorldObject { guid: aid_guid(base.entry, a.flip as u64), x: at[0], y: at[1], z: at[2], o: at[3], ..base };
    if let Some(old) = a.ghost.take() {
        out.destroy.push(old.guid);
    }
    out.create.push(obj.clone());
    a.ghost = Some(obj);
    a.drawn_ms = now;
    out
}

/// Esc: the copy taken away, a hidden prop shown again, nothing armed.
fn cancel(d: &Den, a: &mut Aids) -> AidOut {
    let mut out = AidOut::default();
    if let Some(g) = a.ghost.take() {
        out.destroy.push(g.guid);
    }
    if let Some(Armed::Move(g)) = a.armed {
        if let Some(p) = d.placed(g) {
            out.create.push(p.obj.clone());
        }
    }
    a.armed = None;
    a.at = None;
    a.cursor = None;
    out.tell("aim", "off".into())
}

/// Place (or move) the armed prop where the copy is. It stays armed for the next click.
pub fn commit(d: &mut Den, sel: &mut Sel, a: &mut Aids, now: u64) -> AidOut {
    let (Some(armed), Some(at)) = (a.armed, a.at) else {
        return AidOut::default().said("Pick a prop in the gallery (or Move a selected one) first.");
    };
    let mut out = AidOut { state: true, ..Default::default() };
    match armed {
        Armed::Place(entry) => match d.spawn(entry, at[0], at[1], at[2], at[3]) {
            Ok(obj) => {
                sel.guid = Some(obj.guid);
                out.change = Some(Change { created: vec![obj], message: String::new(), ..Default::default() });
            }
            Err(e) => return out.said(e),
        },
        Armed::Move(g) => {
            let spot = d.spot;
            let yaw = (at[3].to_degrees() as f64).rem_euclid(360.0);
            match d.edit(g, |p| {
                p.x = (at[0] - spot.x) as f64;
                p.y = (at[1] - spot.y) as f64;
                p.z = (at[2] - spot.z) as f64;
                p.yaw = yaw;
            }) {
                Ok(c) => {
                    out.change = Some(c);
                    // shown to everyone by the change; hidden again here while its copy keeps moving
                    out.destroy.push(g);
                }
                Err(e) => return out.said(e),
            }
        }
    }
    let _ = now;
    out.merge(sync(d, sel, a));
    out
}

/// The exact click: the cast's destination (CMSG_CAST_SPELL). Places or moves the armed prop there.
pub fn dest(d: &mut Den, sel: &mut Sel, a: &mut Aids, x: f32, y: f32, now: u64, can: bool) -> AidOut {
    if !can {
        return AidOut::default().said("Only the Den's owner builds here (they can let friends: .den friends build on).");
    }
    if a.armed.is_none() {
        return AidOut::default().said("Pick a prop in the gallery (or Move a selected one) first.");
    }
    set_at(d, sel, a, x, y);
    let mut out = draw(d, a, now, true);
    out.merge(commit(d, sel, a, now));
    out
}

/// A right-click on a gameobject (CMSG_GAMEOBJ_USE): a gizmo flag picks its axis; in build mode a prop is selected
/// (the addon opens its menu at the cursor: `picked <id>`); in play mode a prop is left to the game.
pub fn use_object(d: &Den, sel: &mut Sel, a: &mut Aids, guid: u64, build: bool) -> AidOut {
    if let Some(ax) = gizmo_axis(guid) {
        a.axis = Some(ax);
        return AidOut::default().tell("axis", ax.to_string());
    }
    if !build || d.placed(guid).is_none() {
        return AidOut::default();
    }
    if sel.guid != Some(guid) {
        sel.more.clear();
        sel.round = None;
    }
    sel.guid = Some(guid);
    let mut out = AidOut { state: true, ..Default::default() };
    out.merge(sync(d, sel, a));
    out.tell("picked", (guid & 0xFF_FFFF).to_string())
}

/// The gizmo where it should be: around the selected prop while it is on (not while that prop is being moved).
pub fn sync(d: &Den, sel: &Sel, a: &mut Aids) -> AidOut {
    let mut out = AidOut::default();
    let moving = matches!(a.armed, Some(Armed::Move(g)) if Some(g) == sel.guid);
    let want = sel
        .guid
        .filter(|_| a.gizmo_on && !moving)
        .and_then(|g| d.placed(g))
        .map(|p| (p.obj.guid, [(p.obj.x * 100.0) as i32, (p.obj.y * 100.0) as i32, (p.obj.z * 100.0) as i32], p.obj.clone()));
    if want.as_ref().map(|w| (w.0, w.1)) == a.gizmo_for {
        return out;
    }
    out.destroy.append(&mut a.gizmo);
    a.gizmo_for = None;
    if let Some((g, q, o)) = want {
        a.gizmo_flip = !a.gizmo_flip;
        let r = GIZMO_OFF * o.scale.max(0.5);
        let at = [(r, 0.0, 0.0, 0.0f32), (0.0, r, 0.0, std::f32::consts::FRAC_PI_2), (0.0, 0.0, r + 1.0, 0.0)];
        for (i, (e, display, _, _)) in GIZMO.iter().enumerate() {
            let n = 0x10 + i as u64 + if a.gizmo_flip { 3 } else { 0 };
            let (dx, dy, dz, yaw) = at[i];
            let f = WorldObject { guid: aid_guid(*e, n), entry: *e, display: *display, kind: GOOBER, scale: GIZMO_SCALE, x: o.x + dx, y: o.y + dy, z: o.z + dz, o: yaw, pitch: 0.0, roll: 0.0 };
            a.gizmo.push(f.guid);
            out.create.push(f);
        }
        a.gizmo_for = Some((g, q));
    }
    out
}

/// Everything of the aids taken away (logout, the player left).
pub fn clear(a: &mut Aids) -> Vec<u64> {
    let mut v: Vec<u64> = a.gizmo.drain(..).collect();
    v.extend(a.ghost.take().map(|g| g.guid));
    a.gizmo_for = None;
    a.armed = None;
    a.at = None;
    v
}

// ---------------------------------------------------------------- the packets

/// SMSG_INITIAL_SPELLS: u8 0, u16 count, each u32 spell and u16 0, u16 cooldowns (none).
pub fn initial_spells_body(spells: &[u32]) -> Vec<u8> {
    let mut b = vec![0u8];
    b.extend_from_slice(&(spells.len() as u16).to_le_bytes());
    for s in spells {
        b.extend_from_slice(&s.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
    }
    b.extend_from_slice(&0u16.to_le_bytes());
    b
}

/// A cast from the client (CMSG_CAST_SPELL, 3.3.5a): its count, the spell and, when it has one, the destination.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cast {
    pub count: u8,
    pub spell: u32,
    pub dest: Option<[f32; 3]>,
}

fn skip_packed(b: &[u8], at: &mut usize) -> Option<()> {
    let mask = *b.get(*at)?;
    *at += 1 + mask.count_ones() as usize;
    (*at <= b.len()).then_some(())
}

/// u8 cast count, u32 spell, u8 cast flags, u32 target mask; then a packed guid for a unit, corpse or gameobject
/// target; one for an item; for a source location a packed transport guid and x, y, z; the same for a destination.
pub fn parse_cast(b: &[u8]) -> Option<Cast> {
    let count = *b.first()?;
    let spell = u32::from_le_bytes(b.get(1..5)?.try_into().ok()?);
    let mask = u32::from_le_bytes(b.get(6..10)?.try_into().ok()?);
    let mut at = 10;
    let f = |b: &[u8], at: usize| b.get(at..at + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    // UNIT, UNIT_RAID, UNIT_PARTY, UNIT_ENEMY, UNIT_ALLY, CORPSE_ENEMY, UNIT_DEAD, GAMEOBJECT, GAMEOBJECT_ITEM,
    // CORPSE_ALLY, UNIT_MINIPET, UNIT_PASSENGER
    if mask & (0x2 | 0x4 | 0x8 | 0x80 | 0x100 | 0x200 | 0x400 | 0x800 | 0x4000 | 0x8000 | 0x1_0000 | 0x10_0000) != 0 {
        skip_packed(b, &mut at)?;
    }
    // ITEM, TRADE_ITEM, GAMEOBJECT_ITEM
    if mask & (0x10 | 0x1000 | 0x4000) != 0 {
        skip_packed(b, &mut at)?;
    }
    if mask & 0x20 != 0 {
        skip_packed(b, &mut at)?;
        at += 12;
    }
    let dest = if mask & DEST_LOCATION != 0 {
        skip_packed(b, &mut at)?;
        let p = [f(b, at)?, f(b, at + 4)?, f(b, at + 8)?];
        p.iter().all(|v| v.is_finite() && v.abs() < 1.0e6).then_some(p)
    } else {
        None
    };
    Some(Cast { count, spell, dest })
}

/// SMSG_CAST_FAILED: u8 count, u32 spell, u8 result (DONT_REPORT: nothing on screen).
pub fn cast_failed_body(c: &Cast) -> Vec<u8> {
    let mut b = vec![c.count];
    b.extend_from_slice(&c.spell.to_le_bytes());
    b.push(SPELL_FAILED_DONT_REPORT);
    b
}

/// SMSG_CLEAR_COOLDOWN: u32 spell, u64 the player.
pub fn clear_cooldown_body(spell: u32, guid: u64) -> Vec<u8> {
    let mut b = spell.to_le_bytes().to_vec();
    b.extend_from_slice(&guid.to_le_bytes());
    b
}

/// One calibration sample: the last hover and where the exact click landed, with the player's place and facing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub u: f32,
    pub v: f32,
    pub aspect: f32,
    pub dist: f32,
    pub player: [f32; 4],
    pub dest: [f32; 3],
}

pub fn sample(a: &Aids, here: Here, dest: [f32; 3]) -> Option<Sample> {
    let c = a.cursor?;
    Some(Sample { u: c[0], v: c[1], aspect: c[2], dist: a.cam.dist, player: [here.x, here.y, here.z, here.o], dest })
}

/// What the den-test reads of the aids (`.den` commands' test-out: "aids").
pub fn view(a: &Aids, sel: &Sel, build: bool) -> serde_json::Value {
    serde_json::json!({
        "armed": aim(a), "ghost": a.ghost.as_ref().map(|g| [g.x, g.y, g.z, g.o]), "at": a.at, "cursor": a.cursor,
        "axis": a.axis.map(String::from), "gizmo": a.gizmo.len(), "sel": sel.guid.map(|g| g & 0xFF_FFFF), "mode": if build { "build" } else { "play" },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn den() -> (Den, Here) {
        let d = Den::new(den::Spot::default());
        let s = d.spot;
        (d, Here { x: s.x, y: s.y, z: s.z, o: 0.0 })
    }

    #[test]
    fn the_ray_through_the_middle_lands_ahead_and_the_sides_mirror() {
        let cam = Camera::default();
        let h = Here { x: 100.0, y: 50.0, z: 10.0, o: 0.0 };
        // the middle of the screen looks at the pivot: on the ground pivot / tan(pitch) ahead
        let (x, y) = project(&cam, h, 0.0, 0.0, 16.0 / 9.0, 10.0).unwrap();
        assert!((x - (100.0 + cam.pivot / cam.pitch.tan())).abs() < 0.01 && (y - 50.0).abs() < 0.01, "{x} {y}");
        // right of the middle is to the player's right (facing +x: -y); left mirrors it
        let (_, yr) = project(&cam, h, 0.5, 0.0, 16.0 / 9.0, 10.0).unwrap();
        let (_, yl) = project(&cam, h, -0.5, 0.0, 16.0 / 9.0, 10.0).unwrap();
        assert!(yr < 50.0 && yl > 50.0 && ((50.0 - yr) - (yl - 50.0)).abs() < 0.01);
        // lower on the screen is nearer; the top (above the horizon) doesn't land
        let (xn, _) = project(&cam, h, 0.0, -0.8, 16.0 / 9.0, 10.0).unwrap();
        assert!(xn < x);
        assert!(project(&cam, h, 0.0, 1.0, 16.0 / 9.0, 10.0).is_none());
        // facing north-west (+y): the same point turned
        let (x2, y2) = project(&cam, Here { o: std::f32::consts::FRAC_PI_2, ..h }, 0.0, 0.0, 16.0 / 9.0, 10.0).unwrap();
        assert!((x2 - 100.0).abs() < 0.01 && (y2 - (50.0 + cam.pivot / cam.pitch.tan())).abs() < 0.01);
        assert!(project(&cam, h, f32::NAN, 0.0, 1.0, 10.0).is_none());
    }

    #[test]
    fn preview_hover_and_commit_place_the_prop_where_the_copy_was() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let out = cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 1000);
        assert_eq!(out.create.len(), 1);
        let g0 = out.create[0].clone();
        assert!(is_ghost(g0.guid) && g0.entry == 2413);
        assert!((g0.x - (here.x + AHEAD)).abs() < 0.01, "in front before the cursor says");
        assert_eq!(shown(&g0, true), (GENERIC, NOT_SELECTABLE), "the copy never glows");
        assert!(out.addon.contains(&("aim".into(), format!("place,2413,{PLACE_SPELL}"))));
        assert!(d.props.is_empty(), "nothing placed yet");
        // the cursor: the copy moves there, made under the other guid before the old one goes
        let out = cmd(&mut d, &mut sel, &mut a, here, "hover 0.2 -0.3 1.7778 12", true, 1200);
        assert_eq!((out.create.len(), out.destroy.clone()), (1, vec![g0.guid]));
        let g1 = out.create[0].clone();
        assert_ne!(g1.guid, g0.guid);
        let want = project(&Camera { dist: 12.0, ..Camera::default() }, here, 0.2, -0.3, 1.7778, d.floor_at(here.x, here.y)).unwrap();
        assert!((g1.x - want.0).abs() < 0.01 && (g1.y - want.1).abs() < 0.01);
        // too soon after: kept, not shown
        let out = cmd(&mut d, &mut sel, &mut a, here, "hover 0.1 -0.3 1.7778 12", true, 1250);
        assert!(out.create.is_empty());
        let kept = a.at.unwrap();
        // commit: the real prop where the copy was told to be; still armed
        let out = cmd(&mut d, &mut sel, &mut a, here, "commit", true, 1300);
        let ch = out.change.unwrap();
        assert_eq!(ch.created.len(), 1);
        assert!((ch.created[0].x - kept[0]).abs() < 0.01 && (ch.created[0].y - kept[1]).abs() < 0.01);
        assert_eq!(d.props.len(), 1);
        assert!(sel.guid == Some(ch.created[0].guid) && a.armed == Some(Armed::Place(2413)) && out.state);
        // the grid snaps the copy
        sel.grid = 2.0;
        cmd(&mut d, &mut sel, &mut a, here, "hover 0.13 -0.31 1.7778 12", true, 2000);
        let at = a.at.unwrap();
        assert!((((at[0] - d.spot.x) / 2.0).round() * 2.0 - (at[0] - d.spot.x)).abs() < 0.01);
        // Esc: the copy goes, nothing armed
        let out = cmd(&mut d, &mut sel, &mut a, here, "preview off", true, 2100);
        assert_eq!(out.destroy.len(), 1);
        assert!(a.armed.is_none() && a.ghost.is_none() && out.addon.contains(&("aim".into(), "off".into())));
        assert!(cmd(&mut d, &mut sel, &mut a, here, "hover 0 0 1 12", true, 3000).create.is_empty(), "nothing armed: no copy");
        // a visitor can't arm
        assert!(cmd(&mut d, &mut sel, &mut a, here, "preview 2413", false, 4000).say[0].contains("owner"));
        assert!(cmd(&mut d, &mut sel, &mut a, here, "preview 7", true, 4000).say[0].contains("gallery"));
    }

    #[test]
    fn the_copy_follows_the_player_until_the_cursor_takes_over() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 0);
        let moved = Here { x: here.x + 3.0, o: std::f32::consts::FRAC_PI_2, ..here };
        let out = follow_player(&d, &sel, &mut a, moved, 500);
        assert_eq!(out.create.len(), 1);
        assert!((out.create[0].x - moved.x).abs() < 0.01 && (out.create[0].y - (moved.y + AHEAD)).abs() < 0.01);
        assert!(follow_player(&d, &sel, &mut a, moved, 520).create.is_empty(), "throttled");
        cmd(&mut d, &mut sel, &mut a, here, "hover 0 0 1.7 12", true, 1000);
        assert!(follow_player(&d, &sel, &mut a, here, 2000).create.is_empty(), "the cursor leads now");
        cmd(&mut d, &mut sel, &mut a, here, "follow", true, 3000);
        assert!(!follow_player(&d, &sel, &mut a, moved, 4000).create.is_empty());
    }

    #[test]
    fn moving_a_selected_prop_hides_it_moves_its_copy_and_commit_puts_it_there() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let obj = d.spawn(2413, here.x + 3.0, here.y, here.z, 0.0).unwrap();
        assert!(cmd(&mut d, &mut sel, &mut a, here, "preview move", true, 0).say[0].contains("Select"));
        sel.guid = Some(obj.guid);
        let out = cmd(&mut d, &mut sel, &mut a, here, "preview move", true, 0);
        assert!(out.destroy.contains(&obj.guid), "the real one hidden");
        assert!(out.create.len() == 1 && is_ghost(out.create[0].guid) && out.create[0].display == obj.display);
        cmd(&mut d, &mut sel, &mut a, here, "hover -0.2 -0.2 1.7778 12", true, 500);
        let at = a.at.unwrap();
        let out = cmd(&mut d, &mut sel, &mut a, here, "commit", true, 600);
        let p = d.placed(obj.guid).unwrap();
        assert!((p.obj.x - at[0]).abs() < 0.01 && (p.obj.y - at[1]).abs() < 0.01, "moved, the same prop");
        assert_eq!(d.props.len(), 1);
        assert!(out.change.unwrap().created[0].guid == obj.guid && out.destroy.contains(&obj.guid), "shown to all, hidden here again");
        // Esc shows it again
        let out = cmd(&mut d, &mut sel, &mut a, here, "preview off", true, 700);
        assert!(out.create.iter().any(|o| o.guid == obj.guid));
        // one undo takes the move back
        d.undo();
        assert!((d.placed(obj.guid).unwrap().obj.x - (here.x + 3.0)).abs() < 0.01);
    }

    #[test]
    fn the_gizmo_follows_the_selection_and_its_flags_pick_an_axis() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let obj = d.spawn(2413, here.x + 3.0, here.y, here.z, 0.0).unwrap();
        sel.guid = Some(obj.guid);
        assert!(sync(&d, &sel, &mut a).create.is_empty(), "off until the addon turns it on");
        let out = cmd(&mut d, &mut sel, &mut a, here, "gizmo on", true, 0);
        assert_eq!(out.create.len(), 3);
        let x = out.create.iter().find(|o| o.entry == 990_001).unwrap();
        assert!((x.x - obj.x - GIZMO_OFF).abs() < 0.01 && x.display == 5913);
        assert!(out.create.iter().all(|o| shown(o, false) == (GOOBER, 0)), "right-click uses them");
        assert!(sync(&d, &sel, &mut a).create.is_empty(), "nothing changed: nothing sent");
        // the prop moves: new flags made, the old ones taken away
        let old = a.gizmo.clone();
        d.edit(obj.guid, |p| p.x += 1.0).unwrap();
        let out = sync(&d, &sel, &mut a);
        assert_eq!(out.destroy, old);
        assert!(out.create.iter().all(|o| !old.contains(&o.guid)) && out.create.len() == 3);
        // a flag right-clicked: its axis
        let y = out.create.iter().find(|o| o.entry == 990_002).unwrap().guid;
        let o = use_object(&d, &mut sel, &mut a, y, false);
        assert_eq!(o.addon, vec![("axis".to_string(), "y".to_string())]);
        assert_eq!(a.axis, Some('y'));
        assert_eq!(query(990_003), Some((GOOBER, 5912, "Move along Z", GIZMO_SCALE)));
        assert_eq!(query(2413), None);
        // deselected: they go
        sel.guid = None;
        assert_eq!(sync(&d, &sel, &mut a).destroy.len(), 3);
        assert!(a.gizmo.is_empty());
    }

    #[test]
    fn right_clicking_a_prop_in_build_mode_selects_it() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let obj = d.spawn(2413, here.x + 3.0, here.y, here.z, 0.0).unwrap();
        let out = cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        assert_eq!(out.look, Some(true));
        assert_eq!(shown(&obj, true), (GOOBER, 0), "highlighted, usable");
        assert_eq!(shown(&obj, false), (obj.kind, 0), "in play as it is");
        cmd(&mut d, &mut sel, &mut a, here, "gizmo on", true, 0);
        assert_eq!(use_object(&d, &mut sel, &mut a, obj.guid, false), AidOut::default(), "play mode: left to the game");
        assert_eq!(sel.guid, None);
        let out = use_object(&d, &mut sel, &mut a, obj.guid, true);
        assert_eq!(sel.guid, Some(obj.guid));
        assert!(out.state && out.create.len() == 3);
        assert!(out.addon.contains(&("picked".into(), (obj.guid & 0xFF_FFFF).to_string())));
        assert_eq!(use_object(&d, &mut sel, &mut a, 12345, true), AidOut::default(), "not ours: nothing");
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, "mode play", true, 0).look, Some(false));
    }

    #[test]
    fn the_exact_click_reads_the_cast_and_places_there() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        // count 3, Flare, flags 0, DEST_LOCATION, transport guid 0, x y z
        let mut b = vec![3u8];
        b.extend_from_slice(&PLACE_SPELL.to_le_bytes());
        b.push(0);
        b.extend_from_slice(&DEST_LOCATION.to_le_bytes());
        b.push(0);
        for v in [here.x + 4.0, here.y - 2.0, here.z] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        let c = parse_cast(&b).unwrap();
        assert_eq!((c.count, c.spell), (3, PLACE_SPELL));
        assert_eq!(c.dest, Some([here.x + 4.0, here.y - 2.0, here.z]));
        assert_eq!(cast_failed_body(&c), vec![3, 0x07, 0x06, 0, 0, SPELL_FAILED_DONT_REPORT]);
        // a unit target and a destination: the guid skipped
        let mut u = vec![1u8, 0x07, 0x06, 0, 0, 0];
        u.extend_from_slice(&(0x2 | DEST_LOCATION).to_le_bytes());
        u.extend_from_slice(&[0b0000_0011, 7, 9, 0]);
        u.extend_from_slice(&[1.0f32.to_le_bytes(), 2.0f32.to_le_bytes(), 3.0f32.to_le_bytes()].concat());
        assert_eq!(parse_cast(&u).unwrap().dest, Some([1.0, 2.0, 3.0]));
        assert_eq!(parse_cast(&b[..12]).map(|c| c.dest), None, "cut short");
        // nothing armed: said; armed: the prop where the click was
        assert!(dest(&mut d, &mut sel, &mut a, 0.0, 0.0, 0, true).say[0].contains("Pick"));
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 0);
        a.cursor = Some([0.1, -0.2, 1.7]);
        let s = sample(&a, here, c.dest.unwrap()).unwrap();
        assert_eq!((s.u, s.dest[0]), (0.1, here.x + 4.0));
        let out = dest(&mut d, &mut sel, &mut a, here.x + 4.0, here.y - 2.0, 10, true);
        let made = &out.change.unwrap().created[0];
        assert!((made.x - (here.x + 4.0)).abs() < 0.01 && (made.y - (here.y - 2.0)).abs() < 0.01 && (made.z - d.floor_at(made.x, made.y)).abs() < 0.01);
        assert!(dest(&mut d, &mut sel, &mut a, 0.0, 0.0, 0, false).say[0].contains("owner"));
        assert_eq!(initial_spells_body(&[PLACE_SPELL]), vec![0, 1, 0, 0x07, 0x06, 0, 0, 0, 0, 0, 0]);
        assert_eq!(clear_cooldown_body(PLACE_SPELL, 5).len(), 12);
    }

    #[test]
    fn the_verbs_and_the_camera_file() {
        assert!(is_aid("preview 2413") && is_aid("HOVER 0 0") && is_aid("mode build") && !is_aid("move x 1") && !is_aid("spawn 1"));
        let dir = std::env::temp_dir().join(format!("den-cam-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(Camera::load(&dir), Camera::default());
        std::fs::write(dir.join(CAMERA_FILE), r#"{"fov":1.2,"pivot":1.8,"dist":14.0,"pitch":0.6}"#).unwrap();
        assert_eq!(Camera::load(&dir), Camera { fov: 1.2, pivot: 1.8, dist: 14.0, pitch: 0.6 });
        std::fs::write(dir.join(CAMERA_FILE), r#"{"fov":9,"pivot":1.8,"dist":14.0,"pitch":0.6}"#).unwrap();
        assert_eq!(Camera::load(&dir), Camera::default(), "nonsense: the guesses");
        let _ = std::fs::remove_dir_all(&dir);
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, "where", true, 0).addon, vec![("ghost".to_string(), "-".to_string())]);
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, "axis z", true, 0).addon, vec![("axis".to_string(), "z".to_string())]);
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, "axis off", true, 0).addon, vec![("axis".to_string(), "-".to_string())]);
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 0);
        cmd(&mut d, &mut sel, &mut a, here, "preview turn 90", true, 0);
        assert!((a.at.unwrap()[3] - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert!(where_line(&a).ends_with(",90"));
        assert_eq!(clear(&mut a).len(), 1);
    }
}
