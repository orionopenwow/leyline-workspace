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
//!   sits, decor does nothing). The client keeps each entry's query answer (its type among it) and goes by that,
//!   not by the type byte alone, so in build mode a prop is shown under its build twin's entry ([`twin`]: the entry
//!   with [`BUILD_TWIN`] set), whose query answer is the same model, name and size as a goober. The client asks
//!   for each twin once and keeps it; the prop's own entry (play mode) keeps its own answer.
//!
//! * **The mouse, done by Den Lite** (owner 3 Nov 2026: "clicking objects, having them highlighted, moving them
//!   around easily"): the client never says a left click on a gameobject happened, and its own highlight goes by its
//!   cached template, so Den Lite picks itself ([`crate::pick`]): in build mode the addon's hover comes even with
//!   nothing armed, and Den Lite casts the cursor's ray against every prop's box. The prop under the cursor glows
//!   (GAMEOBJECT_DYNAMIC, as the selection: at most one change per [`HOVER_MS`]; the one it leaves stops, unless it
//!   is the selection, which also has the halo) and the addon is told its name (`hovered <id> <name>`, a tooltip).
//!   `click <u> <v> <aspect> [<zoom>] [add]` (a short, still left click) selects the prop under the cursor, or
//!   clears the selection on empty ground; `add` (Shift) adds it. `drag <u> <v> <aspect> [<zoom>]` (the mouse
//!   pressed on a prop and moved): that prop is selected and its copy follows the cursor, the spot you grabbed staying
//!   under it ([`pick::Grip`]: a level plane at the grabbed height), snapped to the grid; `commit` (the mouse let go)
//!   puts the real one there (one undo); `preview off` (Esc) puts it back; `preview turn` (the wheel, Q / E) turns it.
//! * **The build view**: `cam build` puts the view on the far-sight object up and back over where you stand (a pose
//!   Den Lite knows exactly, so the ray is true): the editor's own camera, from entering build mode until leaving.
//!
//! Everything here is pure (the world server sends what it returns), so it is tested without a client.

use crate::den::{self, Change, Den, WorldObject};
use crate::ovmod::{Here, Sel};
use crate::pick;
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

/// GAMEOBJECT_DYNAMIC's low half (3.3.5a GameObjectDynamicLowFlags): ACTIVATE (the game lets you use it) and SPARKLE
/// (the glitter round a quest object). The selected prop gets both in a values update, and only it (cleared when it
/// is let go): the client then draws the sparkle round its whole model. The high half (a transport's path) stays 0.
pub const DYN_ACTIVATE: u32 = 0x01;
pub const DYN_SPARKLE: u32 = 0x08;
pub const GLOW: u32 = DYN_ACTIVATE | DYN_SPARKLE;
/// The light under the selected prop (the halo): the game's own "Crate Highlight" (gameobject 190117, display 2233,
/// the glow it puts round a quest crate), as an object of ours, sized to the prop's footprint, not selectable.
pub const HALO: (u32, u32, &str) = (990_010, 2233, "Selected");

/// The copy stands this far in front of the player when asked to (`follow`; the keyboard's place).
pub const AHEAD: f32 = 2.5;
/// At most this often (ms) the copy is shown at a new place (about 10 a second); the place itself is always kept.
pub const DRAW_MS: u64 = 100;
/// The farthest the ray may land from where the camera looks from (yards, past the camera's own distance).
pub const REACH: f32 = 60.0;
/// The farthest a drag along an axis goes in one go (yards).
pub const DRAG_MAX: f32 = 60.0;

/// Snap out: the overview pulls the build camera back this far along its own line of sight (yards past the
/// client's own zoom), so the screen's middle stays on the same spot of ground.
pub const SNAP_BACK: f32 = 35.0;
/// Zoom past the client's limit: each wheel notch out pulls back this much more, and a quarter of how far out it is
/// (so a long way out goes fast); the farthest (yards).
pub const FAR_STEP: f32 = 8.0;
pub const FAR_MAX: f32 = 1500.0;
/// The build view (`cam build`, on entering build mode): this far back past the client's own zoom, up the camera's
/// line of sight from where you stand: a high three-quarter view over your Den.
pub const BUILD_BACK: f32 = 14.0;
/// The hover's glow changes at most this often (ms).
pub const HOVER_MS: u64 = 100;

const HIGH_GAMEOBJECT: u64 = 0xF110;
/// The aids' own object counters (never a prop's: those count up from 1): the copy 0 and 1, the gizmo 0x10 ..= 0x15,
/// the halo 0x20 and 0x21.
const AID_BASE: u64 = 0xFF_FF00;

/// The gizmo's handles: (entry of ours, display, name, axis). Displays from AzerothCore's gameobject_template: the
/// red Warsong Flag (5913), the green Netherstorm Flag (7153), the blue Silverwing Flag (5912). The stock client has
/// no colour tint for a gameobject and no arrow model in red, green and blue, so the three coloured flags are the
/// handles: at the prop's foot along X and Y, and over its top for Z (up).
pub const GIZMO: [(u32, u32, &str, char); 3] = [(990_001, 5913, "Drag along X", 'x'), (990_002, 7153, "Drag along Y", 'y'), (990_003, 5912, "Drag up and down (Z)", 'z')];
const GIZMO_SCALE: f32 = 0.5;
/// How far out from the prop's side a handle stands (yards), and the least.
const GIZMO_MARGIN: f32 = 0.8;
const GIZMO_OFF: f32 = 1.8;

/// The query answer for the aids' entries (handles: goobers, right-click uses them; the halo: plain).
pub fn query(entry: u32) -> Option<(u8, u32, &'static str, f32)> {
    if entry == HALO.0 {
        return Some((GENERIC, HALO.1, HALO.2, 1.0));
    }
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

pub fn is_halo(g: u64) -> bool {
    matches!(counter(g).checked_sub(AID_BASE), Some(0x20 | 0x21))
}

/// The gizmo handle's axis, for its guid.
pub fn gizmo_axis(g: u64) -> Option<char> {
    let n = counter(g).checked_sub(AID_BASE)?;
    (0x10..0x16).contains(&n).then(|| GIZMO[((n - 0x10) % 3) as usize].3)
}

/// The bit that makes a prop's entry its build twin (above every entry the game or Den Lite uses; the guid keeps
/// the prop's own entry, so selection and right-click go by the guid as before).
pub const BUILD_TWIN: u32 = 0x0100_0000;

/// The prop's entry in build mode: the twin the client is told is a goober.
pub fn twin(entry: u32) -> u32 {
    entry | BUILD_TWIN
}

/// For a queried entry: the prop's own entry when it is a build twin.
pub fn untwin(entry: u32) -> Option<u32> {
    (entry & BUILD_TWIN != 0).then_some(entry & !BUILD_TWIN)
}

/// The query answer for any entry: a twin is its prop's template as a goober; else the prop's own; else an aid's.
pub fn query_any(entry: u32) -> Option<(u8, u32, &'static str, f32)> {
    if let Some(own) = untwin(entry) {
        return den::template(own).map(|t| (GOOBER, t.display, t.name, t.size));
    }
    den::template(entry).map(|t| (t.kind, t.display, t.name, t.size)).or_else(|| query(entry))
}

/// The type, flags and entry the client is told for an object (GAMEOBJECT_BYTES_1's type byte, GAMEOBJECT_FLAGS,
/// OBJECT_FIELD_ENTRY): the copy and the halo plain and not selectable; the handles goobers; in build mode every prop
/// a goober under its build twin's entry (highlighted under the mouse, right-click uses it).
pub fn shown(o: &WorldObject, build: bool) -> (u8, u32, u32) {
    if is_ghost(o.guid) || is_halo(o.guid) {
        (GENERIC, NOT_SELECTABLE, o.entry)
    } else if gizmo_axis(o.guid).is_some() {
        (GOOBER, 0, o.entry)
    } else if build {
        (GOOBER, 0, twin(o.entry))
    } else {
        (o.kind, 0, o.entry)
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
    /// In the far view: how high over the far-sight object's own place the camera looks at (yards; the object is
    /// put this much lower, so the look stays on the same line). Calibrated like the rest.
    #[serde(default)]
    pub far_pivot: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { fov: std::f32::consts::FRAC_PI_2, pivot: 2.0, dist: 12.0, pitch: 0.5, far_pivot: 0.0 }
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
            .filter(|c| [c.fov, c.pivot, c.dist, c.pitch, c.far_pivot].iter().all(|v| v.is_finite()) && c.fov > 0.1 && c.fov < 3.0)
            .unwrap_or_default()
    }
}

fn add(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Where the camera is and which way it looks (a unit vector): `cam.dist` behind and above the point `cam.pivot`
/// over `here`, facing as `here` faces, looking down by `cam.pitch`.
pub fn eye_of(cam: &Camera, here: Here) -> ([f32; 3], [f32; 3]) {
    let (so, co) = here.o.sin_cos();
    let (sp, cp) = cam.pitch.sin_cos();
    let pivot = [here.x, here.y, here.z + cam.pivot];
    let look = [cp * co, cp * so, -sp];
    (add(pivot, look, -cam.dist), look)
}

/// The ray from the camera through the cursor at (u, v) on the screen (-1 .. 1, right and up; `aspect` = width /
/// height): its start and its direction (a unit vector). None for numbers that aren't.
pub fn ray(cam: &Camera, here: Here, u: f32, v: f32, aspect: f32) -> Option<([f32; 3], [f32; 3])> {
    if ![u, v, aspect].iter().all(|x| x.is_finite()) {
        return None;
    }
    let (so, co) = here.o.sin_cos();
    let right = [so, -co, 0.0];
    let (eye, look) = eye_of(cam, here);
    let up = cross(right, look);
    let a = aspect.clamp(0.2, 5.0);
    let tv = (cam.fov / 2.0).tan() / (1.0 + a * a).sqrt();
    let d = add(add(look, right, u.clamp(-1.0, 1.0) * tv * a), up, v.clamp(-1.0, 1.0) * tv);
    let n = dot(d, d).sqrt();
    (n > 1e-6).then(|| (eye, [d[0] / n, d[1] / n, d[2] / n]))
}

/// Where the cursor at (u, v) points on the ground at height `floor` (see [`ray`]). None: the ray doesn't come
/// down within [`REACH`] (past the camera's own distance) of `here`.
pub fn project(cam: &Camera, here: Here, u: f32, v: f32, aspect: f32, floor: f32) -> Option<(f32, f32)> {
    if !floor.is_finite() {
        return None;
    }
    let (eye, d) = ray(cam, here, u, v, aspect)?;
    if d[2] >= -1e-4 {
        return None;
    }
    let t = (floor - eye[2]) / d[2];
    if t <= 0.0 {
        return None;
    }
    let p = add(eye, d, t);
    let far = ((p[0] - here.x).powi(2) + (p[1] - here.y).powi(2)).sqrt();
    (far <= REACH + cam.dist).then_some((p[0], p[1]))
}

/// The unit vector of an axis (x, y: the world's; z: up).
pub fn axis_dir(axis: char) -> [f32; 3] {
    match axis {
        'x' => [1.0, 0.0, 0.0],
        'y' => [0.0, 1.0, 0.0],
        _ => [0.0, 0.0, 1.0],
    }
}

/// How far along the axis line (through `p`, unit direction `a`) the point nearest the ray (start `e`, unit
/// direction `d`) is: what a drag on a handle follows. None when the ray runs (nearly) along the axis.
pub fn along(p: [f32; 3], a: [f32; 3], e: [f32; 3], d: [f32; 3]) -> Option<f32> {
    let w = [p[0] - e[0], p[1] - e[1], p[2] - e[2]];
    let b = dot(a, d);
    let den = 1.0 - b * b;
    if den < 1e-3 {
        return None;
    }
    let s = (b * dot(d, w) - dot(a, w)) / den;
    s.is_finite().then_some(s)
}

/// What is armed: a catalog prop to place, the selected prop to move with the cursor, or the selected prop dragged
/// along one axis from where it was (`from`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Armed {
    Place(u32),
    Move(u64),
    Axis { guid: u64, axis: char, from: [f32; 3] },
}

/// The far view (snap out, or zoom past the client's limit): the camera's focus (the spot the screen's middle looks
/// at, and the way it faces) and how far it is pulled back past the client's own zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Far {
    pub focus: Here,
    pub back: f32,
    /// Snap out (one key, and back) or the wheel's zoom.
    pub snap: bool,
}

/// What the world server does with the far-sight object: made there and the view moved to it; moved; taken away
/// and the view given back to the player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FarOut {
    Show([f32; 4]),
    Move([f32; 4], [f32; 4]),
    Off,
}

/// One session's aids.
#[derive(Debug, Clone, Default)]
pub struct Aids {
    pub gizmo_on: bool,
    pub axis: Option<char>,
    /// The gizmo's handles and the halo shown now, and for which prop as it was (place, turn, size).
    pub gizmo: Vec<u64>,
    gizmo_for: Option<(u64, [i32; 5])>,
    gizmo_flip: bool,
    /// The prop that glows now (GAMEOBJECT_DYNAMIC set).
    pub glowing: Option<u64>,
    pub armed: Option<Armed>,
    /// The copy shown now.
    pub ghost: Option<WorldObject>,
    flip: bool,
    /// Where Commit puts it: x, y, z, facing (radians).
    pub at: Option<[f32; 4]>,
    drawn_ms: u64,
    /// The last hover: u, v, aspect (None: no cursor yet).
    pub cursor: Option<[f32; 3]>,
    /// The copy follows the player (`follow`: the keyboard's way); otherwise it waits for the cursor.
    pub follow: bool,
    /// The copy's height over the ground (a moved prop keeps its own) and its turn.
    pub lift: f32,
    pub yaw: f32,
    pub cam: Camera,
    /// The client's own zoom (yards from the pivot; the addon says it in each hover and `cam zoom`).
    pub zoom: Option<f32>,
    pub far: Option<Far>,
    /// Where the far-sight object is now (x, y, z, facing), when it is shown.
    pub far_at: Option<[f32; 4]>,
    /// The drag's last distance along its axis (for the status line).
    pub dragged: f32,
    /// Build mode (`mode build`): the hover picks and glows.
    pub build: bool,
    /// The prop under the cursor now (it glows), and when that last changed.
    pub hovered: Option<u64>,
    hover_ms: Option<u64>,
    /// A mouse drag (`drag`): the moved prop's copy follows the cursor keeping this grip.
    pub grip: Option<pick::Grip>,
    /// Where the selection is (its foot) and where on the screen its middle is (u, v), after each verb: the
    /// den-test clicks and drags there.
    pub sel_at: Option<[f32; 3]>,
    pub sel_uv: Option<[f32; 2]>,
}

impl Aids {
    pub fn new(cam: Camera) -> Aids {
        Aids { cam, ..Default::default() }
    }

    /// The camera the cursor's ray starts from: the build camera around the player at the client's zoom, or in the
    /// far view the same camera pulled back along its line of sight from the far focus.
    pub fn view(&self, here: Here) -> (Here, Camera) {
        let mut cam = self.cam;
        if let Some(z) = self.zoom {
            cam.dist = z;
        }
        match self.far {
            Some(f) => {
                cam.dist += f.back;
                (f.focus, cam)
            }
            None => (here, cam),
        }
    }
}

/// Where the far-sight object goes for a far view: the client's camera keeps its own zoom round it, so it is put
/// `back` yards further back along the camera's line of sight from the focus's pivot (and `far_pivot` lower). The
/// camera's own distance doesn't come into it.
pub fn far_spot(cam: &Camera, f: &Far) -> [f32; 4] {
    let (eye_dir_pivot, look) = eye_of(&Camera { dist: 0.0, ..*cam }, f.focus);
    let p = add(eye_dir_pivot, look, -f.back);
    [p[0], p[1], p[2] - cam.far_pivot, f.focus.o]
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
    /// Values updates of GAMEOBJECT_DYNAMIC (guid, flags), after the creates: the selection's glow on and off.
    pub glow: Vec<(u64, u32)>,
    /// The far-sight object (the snap-out and far-zoom camera).
    pub far: Option<FarOut>,
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
        if o.far.is_some() {
            self.far = o.far;
        }
        self.create.extend(o.create);
        self.destroy.extend(o.destroy);
        self.glow.extend(o.glow);
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
    matches!(v.as_str(), "preview" | "hover" | "follow" | "commit" | "axis" | "grab" | "gizmo" | "mode" | "where" | "cam" | "click" | "drag")
}

/// The addon's `aim` line: what is armed (`place,<entry>`, `move,<prop id>`, `axis,<prop id>,<x|y|z>` or `off`)
/// and the exact click's spell.
fn aim(a: &Aids) -> String {
    match a.armed {
        Some(Armed::Place(e)) => format!("place,{e},{PLACE_SPELL}"),
        Some(Armed::Move(g)) => format!("move,{},{PLACE_SPELL}", g & 0xFF_FFFF),
        Some(Armed::Axis { guid, axis, .. }) => format!("axis,{},{axis}", guid & 0xFF_FFFF),
        None => "off".into(),
    }
}

/// A prop's name for the status line.
fn name_of(d: &Den, g: u64) -> &'static str {
    d.placed(g).and_then(|p| den::template(p.prop.entry as u32)).map(|t| t.name).unwrap_or("prop")
}

/// One aid verb. `now`: milliseconds (the copy's throttle). `can`: may build.
pub fn cmd(d: &mut Den, sel: &mut Sel, a: &mut Aids, here: Here, text: &str, can: bool, now: u64) -> AidOut {
    let out = aid_verb(d, sel, a, here, text, can, now);
    // where the selection is, on the ground and on the screen (the den-test reads them)
    let obj = sel.guid.and_then(|g| d.placed(g)).map(|p| p.obj.clone());
    a.sel_at = obj.as_ref().map(|o| [o.x, o.y, o.z]);
    a.sel_uv = obj.and_then(|o| {
        let (from, cam) = a.view(here);
        let h = extent(&o).2;
        screen(&cam, from, [o.x, o.y, o.z + h / 2.0], a.cursor.map(|c| c[2]).unwrap_or(16.0 / 9.0))
    });
    out
}

fn aid_verb(d: &mut Den, sel: &mut Sel, a: &mut Aids, here: Here, text: &str, can: bool, now: u64) -> AidOut {
    let w: Vec<&str> = text.split_whitespace().collect();
    let verb = w.first().map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let arg = w.get(1).map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let num = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
    let builds = matches!(verb.as_str(), "preview" | "commit" | "grab" | "drag") && arg != "off";
    if builds && !can {
        return AidOut::default().said("Only the Den's owner builds here (they can let friends: .den friends build on).");
    }
    match verb.as_str() {
        "preview" => match arg.as_str() {
            "off" | "cancel" => {
                let mut out = cancel(d, a);
                out.merge(sync(d, sel, a));
                out
            }
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
                a.follow = false;
                // the real one hidden while its copy moves (this player only); Commit or Esc shows it again
                out.destroy.push(p.obj.guid);
                out.merge(draw(d, a, now, true));
                out.merge(sync(d, sel, a));
                out.tell("aim", aim(a))
            }
            // a gallery pick: armed at once; the real prop shows where the cursor is (the hover numbers may ride
            // along: `preview <entry> <u> <v> <aspect> <zoom>`), and not before the cursor is over the ground
            _ => match arg.parse::<u32>().ok().filter(|e| den::template(*e).is_some()) {
                Some(entry) => {
                    let mut out = cancel(d, a);
                    out.addon.clear();
                    a.armed = Some(Armed::Place(entry));
                    a.lift = 0.0;
                    a.yaw = here.o;
                    a.cursor = None;
                    a.follow = false;
                    a.at = None;
                    out.merge(sync(d, sel, a));
                    if let (Some(u), Some(v)) = (num(2), num(3)) {
                        out.merge(hover(d, sel, a, here, u, v, num(4), num(5), now, true));
                    }
                    out.tell("aim", aim(a))
                }
                None => AidOut::default().said("Pick a prop in the gallery first."),
            },
        },
        "hover" => {
            let (Some(u), Some(v)) = (num(1), num(2)) else { return AidOut::default() };
            hover(d, sel, a, here, u, v, num(3), num(4), now, false)
        }
        "follow" => {
            a.cursor = None;
            a.follow = true;
            if matches!(a.armed, Some(Armed::Place(_) | Armed::Move(_))) {
                a.at = Some(ahead(d, sel, here));
                if let Some(at) = a.at.as_mut() {
                    at[2] += a.lift;
                    at[3] = a.yaw;
                }
                return draw(d, a, now, true);
            }
            AidOut::default()
        }
        "commit" => commit(d, sel, a, now, arg == "keep"),
        "axis" => {
            a.axis = match arg.as_str() {
                "x" | "y" | "z" => arg.chars().next(),
                _ => None,
            };
            AidOut::default().tell("axis", a.axis.map(String::from).unwrap_or_else(|| "-".into()))
        }
        // a handle grabbed (right-click on it, or the panel): the selection follows the cursor along that axis;
        // `grab nudge <yards>` moves it on by a step (the wheel), `grab off` puts it back
        "grab" => match arg.as_str() {
            "off" => {
                let mut out = cancel(d, a);
                out.merge(sync(d, sel, a));
                out
            }
            "nudge" => match (a.armed, a.at, num(2)) {
                (Some(Armed::Axis { axis, from, .. }), Some(at), Some(v)) if v.abs() <= 50.0 => {
                    let ad = axis_dir(axis);
                    let s = dot([at[0] - from[0], at[1] - from[1], at[2] - from[2]], ad) + v;
                    drag_to(d, sel, a, s, now, true)
                }
                _ => AidOut::default(),
            },
            "x" | "y" | "z" => grab(d, sel, a, arg.chars().next().unwrap_or('x'), now),
            _ => AidOut::default().said("Usage: grab x|y|z (then move the mouse; click drops it, Esc puts it back)"),
        },
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
            a.build = build;
            // every prop is made again in the new look (its glow with it): the gizmo, the halo and the glow anew
            a.gizmo_for = None;
            a.glowing = None;
            a.hovered = None;
            if !build && a.far.is_some() {
                // leaving build mode: the view is the player's again
                a.far = None;
                out.merge(far_update(a));
            }
            out.merge(sync(d, sel, a));
            out.tell("mode", if build { "build".into() } else { "play".into() })
        }
        "where" => AidOut::default().tell("ghost", where_line(a)),
        "cam" => cam(d, sel, a, here, &w, now),
        // a short, still left click on the world (Gizmo.lua): select what is under the cursor (`add`: Shift adds it)
        "click" => {
            let (Some(u), Some(v)) = (num(1), num(2)) else { return AidOut::default().said("Usage: click <u> <v> <aspect> [<zoom>] [add]") };
            let add = w.iter().skip(3).any(|x| x.eq_ignore_ascii_case("add"));
            click(d, sel, a, here, u, v, num(3), num(4), add)
        }
        // the mouse pressed on a prop and moved: it follows the cursor until `commit` (let go) or `preview off` (Esc)
        "drag" => {
            let (Some(u), Some(v)) = (num(1), num(2)) else { return AidOut::default().said("Usage: drag <u> <v> <aspect> [<zoom>]") };
            drag(d, sel, a, here, u, v, num(3), num(4), now)
        }
        _ => AidOut::default(),
    }
}

/// Where a point in the world is on the screen (u, v from -1 to 1, right and up) for the camera: the inverse of
/// [`ray`]. None behind the camera.
pub fn screen(cam: &Camera, here: Here, p: [f32; 3], aspect: f32) -> Option<[f32; 2]> {
    let (so, co) = here.o.sin_cos();
    let right = [so, -co, 0.0];
    let (eye, look) = eye_of(cam, here);
    let up = cross(right, look);
    let rel = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
    let z = dot(rel, look);
    if z <= 1e-3 {
        return None;
    }
    let a = aspect.clamp(0.2, 5.0);
    let tv = (cam.fov / 2.0).tan() / (1.0 + a * a).sqrt();
    let (u, v) = (dot(rel, right) / z / (tv * a), dot(rel, up) / z / tv);
    (u.is_finite() && v.is_finite()).then_some([u, v])
}

/// Each prop as the cursor's ray sees it: its model's box (width, depth, height, bottom) at its size, turned with
/// it, a little larger so an edge is easy to hit; a guess from its size when the model's box isn't known. Hidden
/// props are left out.
pub fn bodies(d: &Den) -> Vec<pick::Body> {
    d.props.iter().filter(|p| p.prop.hidden != Some(true)).map(|p| body_of(&p.obj)).collect()
}

fn body_of(o: &WorldObject) -> pick::Body {
    let s = if o.scale.is_finite() && o.scale > 0.0 { o.scale } else { 1.0 };
    let (hx, hy, h, bottom) = match crate::models::box_of(o.display) {
        Some((w, dp, h, z)) if w > 0.0 && dp > 0.0 && h > 0.0 => (w / 2.0 * s, dp / 2.0 * s, h * s, z * s),
        _ => {
            let (hx, hy, h) = extent(o);
            (hx, hy, h, 0.0)
        }
    };
    pick::Body::new(o.guid, [o.x, o.y, o.z], o.o, [hx * 1.05, hy * 1.05], bottom, h)
}

/// The cursor's ray from the camera now (the far view's when it is on) and how far along it a prop may be picked.
fn cursor_ray(a: &mut Aids, here: Here, u: f32, v: f32, aspect: Option<f32>, zoom: Option<f32>) -> Option<([f32; 3], [f32; 3], f32)> {
    if let Some(z) = zoom.filter(|z| (0.0..=60.0).contains(z)) {
        a.zoom = Some(z);
    }
    let aspect = aspect.filter(|x| x.is_finite() && *x > 0.0).unwrap_or(16.0 / 9.0);
    a.cursor = Some([u, v, aspect]);
    let (from, cam) = a.view(here);
    let (e, d) = ray(&cam, from, u, v, aspect)?;
    Some((e, d, cam.dist + REACH))
}

/// The prop under the cursor and how far along the ray it was hit.
pub fn under(d: &Den, a: &mut Aids, here: Here, u: f32, v: f32, aspect: Option<f32>, zoom: Option<f32>) -> Option<(u64, f32, [f32; 3], [f32; 3])> {
    let (e, dir, reach) = cursor_ray(a, here, u, v, aspect, zoom)?;
    pick::pick(e, dir, &bodies(d), reach).map(|(g, t)| (g, t, e, dir))
}

fn selected(sel: &Sel, g: u64) -> bool {
    sel.guid == Some(g) || sel.more.contains(&g)
}

/// The addon's `hovered` line: `<id> <name>` or `-`.
fn hovered_line(d: &Den, g: Option<u64>) -> String {
    g.map(|g| format!("{} {}", g & 0xFF_FFFF, name_of(d, g))).unwrap_or_else(|| "-".into())
}

/// The hover in build mode with nothing armed: the prop under the cursor glows, the one it left stops (not the
/// selection's own glow), the addon is told its name. At most one change per [`HOVER_MS`] (the addon sends the
/// cursor once more when it comes to rest, so the last one is never lost).
fn hover_glow(d: &Den, sel: &Sel, a: &mut Aids, hit: Option<u64>, now: u64) -> AidOut {
    let mut out = AidOut::default();
    if hit == a.hovered || a.hover_ms.is_some_and(|t| now.saturating_sub(t) < HOVER_MS) {
        return out;
    }
    if let Some(old) = a.hovered.filter(|g| d.placed(*g).is_some() && a.glowing != Some(*g) && !selected(sel, *g)) {
        out.glow.push((old, 0));
    }
    if let Some(new) = hit.filter(|g| a.glowing != Some(*g)) {
        out.glow.push((new, GLOW));
    }
    a.hovered = hit;
    a.hover_ms = Some(now);
    out.tell("hovered", hovered_line(d, hit))
}

/// A click on the world: the prop under the cursor is the selection (its glow, the handles and the halo, the panel:
/// `selected <id>`); with `add` it joins the selection; empty ground clears it (`selected -`).
#[allow(clippy::too_many_arguments)]
fn click(d: &Den, sel: &mut Sel, a: &mut Aids, here: Here, u: f32, v: f32, aspect: Option<f32>, zoom: Option<f32>, add: bool) -> AidOut {
    let hit = under(d, a, here, u, v, aspect, zoom).map(|h| h.0);
    let mut out = AidOut { state: true, ..Default::default() };
    match hit {
        Some(g) if add && sel.guid.is_some_and(|s| s != g) => {
            if !sel.more.contains(&g) {
                sel.more.push(g);
                out.glow.push((g, GLOW));
            }
            out.say.push(format!("{} selected.", 1 + sel.more.len()));
        }
        Some(g) => {
            if sel.guid != Some(g) {
                // the extra ones go (their glow with them, unless under the cursor)
                for m in std::mem::take(&mut sel.more) {
                    if m != g && d.placed(m).is_some() {
                        out.glow.push((m, 0));
                    }
                }
                sel.round = None;
            }
            sel.guid = Some(g);
        }
        None if add => return AidOut::default(),
        None => {
            for m in std::mem::take(&mut sel.more) {
                if d.placed(m).is_some() {
                    out.glow.push((m, 0));
                }
            }
            sel.guid = None;
            sel.round = None;
        }
    }
    // the hovered one is the selection now (or nothing is): the hover starts again from here
    a.hovered = hit;
    out.merge(sync(d, sel, a));
    out.tell("selected", sel.guid.map(|g| (g & 0xFF_FFFF).to_string()).unwrap_or_else(|| "-".into()))
}

/// The mouse pressed on a prop and moved: it is the selection, the real one hidden here and its glowing copy follows
/// the cursor keeping where it was grabbed. Nothing under the cursor: `grabbed -` (the addon leaves the drag to the
/// camera).
#[allow(clippy::too_many_arguments)]
fn drag(d: &Den, sel: &mut Sel, a: &mut Aids, here: Here, u: f32, v: f32, aspect: Option<f32>, zoom: Option<f32>, now: u64) -> AidOut {
    let Some((g, t, e, dir)) = under(d, a, here, u, v, aspect, zoom) else {
        return AidOut::default().tell("grabbed", "-".into());
    };
    let Some(p) = d.placed(g).cloned() else { return AidOut::default() };
    if p.prop.locked == Some(true) {
        return AidOut::default().said(den::LOCKED).tell("grabbed", "-".into());
    }
    let mut out = cancel(d, a);
    out.addon.clear();
    if !selected(sel, g) {
        sel.more.clear();
        sel.round = None;
    }
    // one prop follows the mouse (the extra ones stay where they are)
    sel.guid = Some(g);
    let o = &p.obj;
    a.armed = Some(Armed::Move(g));
    a.lift = o.z - d.floor_at(o.x, o.y);
    a.yaw = o.o;
    a.at = Some([o.x, o.y, o.z, o.o]);
    a.follow = false;
    a.grip = pick::grip(e, dir, t, [o.x, o.y, o.z]);
    a.hovered = None;
    out.destroy.push(g);
    out.merge(draw(d, a, now, true));
    out.merge(sync(d, sel, a));
    out.state = true;
    out.tell("aim", aim(a)).tell("grabbed", (g & 0xFF_FFFF).to_string())
}

/// The far-sight object where the far view now wants it: shown, moved, or taken away (and the addon's `cam` line).
fn far_update(a: &mut Aids) -> AidOut {
    let to = a.far.map(|f| far_spot(&a.cam, &f));
    let far = match (a.far_at, to) {
        (None, Some(t)) => Some(FarOut::Show(t)),
        (Some(f), Some(t)) if f != t => Some(FarOut::Move(f, t)),
        (Some(_), None) => Some(FarOut::Off),
        _ => None,
    };
    a.far_at = to;
    AidOut { far, ..Default::default() }.tell("cam", cam_line(a))
}

/// A hover: the cursor's ray from the camera (the far view's when it is on). Placing or moving: the copy where it
/// lands on the ground. Dragging along an axis: the point of the axis nearest the ray.
#[allow(clippy::too_many_arguments)]
fn hover(d: &Den, sel: &Sel, a: &mut Aids, here: Here, u: f32, v: f32, aspect: Option<f32>, zoom: Option<f32>, now: u64, force: bool) -> AidOut {
    if let Some(z) = zoom.filter(|z| (0.0..=60.0).contains(z)) {
        a.zoom = Some(z);
    }
    let Some(armed) = a.armed else {
        // nothing armed, in build mode: what is under the cursor glows
        if !a.build {
            return AidOut::default();
        }
        let hit = under(d, a, here, u, v, aspect, None).map(|h| h.0);
        return hover_glow(d, sel, a, hit, now);
    };
    let aspect = aspect.unwrap_or(16.0 / 9.0);
    a.cursor = Some([u, v, aspect]);
    a.follow = false;
    let (from, cam) = a.view(here);
    if let Armed::Axis { axis, from: start, .. } = armed {
        let Some((e, dir)) = ray(&cam, from, u, v, aspect) else { return AidOut::default() };
        return match along(start, axis_dir(axis), e, dir) {
            Some(s) => drag_to(d, sel, a, s, now, force),
            None => AidOut::default(),
        };
    }
    // a mouse drag: the grabbed spot stays under the cursor (the foot on the grid and the ground)
    if let (Armed::Move(_), Some(g)) = (armed, a.grip) {
        let Some((e, dir)) = ray(&cam, from, u, v, aspect) else { return AidOut::default() };
        return match pick::follow(e, dir, &g, cam.dist + REACH) {
            Some([x, y]) => {
                set_at(d, sel, a, x, y);
                draw(d, a, now, force)
            }
            None => AidOut::default(),
        };
    }
    // the ground under the camera's focus first, then under where the ray lands (twice: the ground may slope)
    let mut floor = d.floor_at(from.x, from.y);
    let mut hit = None;
    for _ in 0..2 {
        hit = project(&cam, from, u, v, aspect, floor);
        if let Some((x, y)) = hit {
            floor = d.floor_at(x, y);
        }
    }
    match hit {
        Some((x, y)) => {
            set_at(d, sel, a, x, y);
            draw(d, a, now, force)
        }
        None => AidOut::default(),
    }
}

/// The dragged copy `s` yards along its axis from where the prop was (snapped to the grid, at most [`DRAG_MAX`]).
fn drag_to(d: &Den, sel: &Sel, a: &mut Aids, s: f32, now: u64, force: bool) -> AidOut {
    let Some(Armed::Axis { axis, from, .. }) = a.armed else { return AidOut::default() };
    let s = if sel.grid > 0.0 { ((s as f64 / sel.grid).round() * sel.grid) as f32 } else { s };
    let s = s.clamp(-DRAG_MAX, DRAG_MAX);
    let p = add(from, axis_dir(axis), s);
    if !d.inside(p[0], p[1], p[2]) {
        return AidOut::default();
    }
    a.dragged = s;
    a.at = Some([p[0], p[1], p[2], a.yaw]);
    // the copy redrawn at most every DRAW_MS; the status (and where a click drops it) at every hover
    draw(d, a, now, force).tell("drag", format!("{axis},{s:+.2}"))
}

/// Grab the selection's handle for `axis`: its real object hidden (for this player), a copy dragged instead.
fn grab(d: &Den, sel: &Sel, a: &mut Aids, axis: char, now: u64) -> AidOut {
    let Some(p) = sel.guid.and_then(|g| d.placed(g)).cloned() else {
        return AidOut::default().said("Select a prop first (right-click it in build mode, or Near).");
    };
    if p.prop.locked == Some(true) {
        return AidOut::default().said(den::LOCKED);
    }
    let mut out = cancel(d, a);
    out.addon.clear();
    a.armed = Some(Armed::Axis { guid: p.obj.guid, axis, from: [p.obj.x, p.obj.y, p.obj.z] });
    a.axis = Some(axis);
    a.yaw = p.obj.o;
    a.at = Some([p.obj.x, p.obj.y, p.obj.z, p.obj.o]);
    a.dragged = 0.0;
    a.follow = false;
    out.destroy.push(p.obj.guid);
    out.merge(draw(d, a, now, true));
    out.merge(sync(d, sel, a));
    out.tell("aim", aim(a)).tell("axis", axis.to_string())
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

/// The player moved: a copy put in front of them (`follow`) follows them.
pub fn follow_player(d: &Den, sel: &Sel, a: &mut Aids, here: Here, now: u64) -> AidOut {
    if !a.follow || !matches!(a.armed, Some(Armed::Place(_) | Armed::Move(_))) {
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
/// unless `force`. A dragged prop's copy glows as the selection does.
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
        Armed::Move(g) | Armed::Axis { guid: g, .. } => match d.placed(g) {
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
    if matches!(armed, Armed::Axis { .. }) || a.grip.is_some() {
        out.glow.push((obj.guid, GLOW));
    }
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
    if let Some(Armed::Move(g) | Armed::Axis { guid: g, .. }) = a.armed {
        if let Some(p) = d.placed(g) {
            out.create.push(p.obj.clone());
            // made again: its glow too, and the gizmo comes back
            a.glowing = None;
            a.gizmo_for = None;
        }
    }
    if matches!(a.armed, Some(Armed::Axis { .. })) {
        a.axis = None;
        out.addon.push(("axis".into(), "-".into()));
    }
    a.armed = None;
    a.at = None;
    a.cursor = None;
    a.follow = false;
    a.grip = None;
    out.tell("aim", "off".into())
}

/// A click: place (or move, or drop the dragged) prop where the copy is. Placing ends with it (the new prop is the
/// selection: its glow, the gizmo, the panel), unless `keep` (Shift+click: place more of it).
pub fn commit(d: &mut Den, sel: &mut Sel, a: &mut Aids, now: u64, keep: bool) -> AidOut {
    let (Some(armed), Some(at)) = (a.armed, a.at) else {
        return AidOut::default().said("Pick a prop in the gallery (or Move a selected one) first.");
    };
    let mut out = AidOut { state: true, ..Default::default() };
    let keep = keep && matches!(armed, Armed::Place(_));
    match armed {
        Armed::Place(entry) => match d.spawn(entry, at[0], at[1], at[2], at[3]) {
            Ok(obj) => {
                sel.guid = Some(obj.guid);
                sel.more.clear();
                sel.round = None;
                out.addon.push(("placed".into(), (obj.guid & 0xFF_FFFF).to_string()));
                out.change = Some(Change { created: vec![obj], message: String::new(), ..Default::default() });
            }
            Err(e) => return out.said(e),
        },
        Armed::Move(g) | Armed::Axis { guid: g, .. } => {
            let spot = d.spot;
            let yaw = (at[3].to_degrees() as f64).rem_euclid(360.0);
            let name = name_of(d, g);
            match d.edit(g, |p| {
                p.x = (at[0] - spot.x) as f64;
                p.y = (at[1] - spot.y) as f64;
                p.z = (at[2] - spot.z) as f64;
                p.yaw = yaw;
            }) {
                Ok(c) => {
                    if let Armed::Axis { axis, .. } = armed {
                        let s = a.dragged;
                        out.say.push(format!("Moved {name} {s:+.2} yd along {}.", axis.to_ascii_uppercase()));
                        out.addon.push(("moved".into(), format!("{axis},{s:+.2}")));
                    }
                    out.change = Some(c);
                }
                Err(e) => return out.said(e),
            }
        }
    }
    if !keep {
        // the copy goes (after the real one is made: no frame without it); nothing armed
        if let Some(g) = a.ghost.take() {
            out.destroy.push(g.guid);
        }
        if matches!(armed, Armed::Axis { .. }) {
            a.axis = None;
            out.addon.push(("axis".into(), "-".into()));
        }
        a.armed = None;
        a.at = None;
        a.cursor = None;
        a.follow = false;
        a.grip = None;
        out.addon.push(("aim".into(), "off".into()));
    }
    // the prop was made anew: its glow and the gizmo anew
    a.gizmo_for = None;
    a.glowing = None;
    let _ = now;
    out.merge(sync(d, sel, a));
    out
}

/// The exact click: the cast's destination (CMSG_CAST_SPELL). Places or moves the armed prop there.
pub fn dest(d: &mut Den, sel: &mut Sel, a: &mut Aids, x: f32, y: f32, now: u64, can: bool) -> AidOut {
    if !can {
        return AidOut::default().said("Only the Den's owner builds here (they can let friends: .den friends build on).");
    }
    if !matches!(a.armed, Some(Armed::Place(_) | Armed::Move(_))) {
        return AidOut::default().said("Pick a prop in the gallery (or Move a selected one) first.");
    }
    set_at(d, sel, a, x, y);
    let mut out = draw(d, a, now, true);
    out.merge(commit(d, sel, a, now, true));
    out
}

/// A right-click on a gameobject (CMSG_GAMEOBJ_USE): a gizmo handle grabs its axis (the prop then follows the cursor
/// along it); in build mode a prop is selected (the addon opens its menu at the cursor: `picked <id>`); in play mode
/// a prop is left to the game.
pub fn use_object(d: &Den, sel: &mut Sel, a: &mut Aids, guid: u64, build: bool, now: u64) -> AidOut {
    if let Some(ax) = gizmo_axis(guid) {
        a.axis = Some(ax);
        if sel.guid.is_some_and(|g| d.placed(g).is_some()) {
            return grab(d, sel, a, ax, now);
        }
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

/// The prop's footprint and height in yards (half width, half depth, height), from the player's own model table when
/// Den Lite has it, else a guess from its size.
fn extent(o: &WorldObject) -> (f32, f32, f32) {
    match crate::models::box_of(o.display) {
        Some((w, dp, h, _)) if w > 0.0 && dp > 0.0 && h > 0.0 => (w / 2.0 * o.scale, dp / 2.0 * o.scale, h * o.scale),
        _ => (GIZMO_OFF * o.scale.max(0.5) - GIZMO_MARGIN, GIZMO_OFF * o.scale.max(0.5) - GIZMO_MARGIN, 2.0 * o.scale.max(0.5)),
    }
}

/// The gizmo where it should be: around the selected prop while it is on (not while that prop is being moved): the
/// three handles at its foot (X, Y) and over its top (Z), the halo under it, and its glow. Made anew whenever the
/// prop is (a move, a turn, a size), as the prop's own create clears its glow.
pub fn sync(d: &Den, sel: &Sel, a: &mut Aids) -> AidOut {
    let mut out = AidOut::default();
    let moving = matches!(a.armed, Some(Armed::Move(g) | Armed::Axis { guid: g, .. }) if Some(g) == sel.guid);
    let on = sel.guid.filter(|_| a.gizmo_on).and_then(|g| d.placed(g));
    let want = on.filter(|_| !moving).map(|p| {
        let o = &p.obj;
        let key = [(o.x * 100.0) as i32, (o.y * 100.0) as i32, (o.z * 100.0) as i32, (o.o * 100.0) as i32, (o.scale * 100.0) as i32];
        (o.guid, key, o.clone())
    });
    // the glow: on the selection (also while it is dragged: its copy carries it), off the one let go
    let glow_on = on.map(|p| p.obj.guid);
    if a.glowing != glow_on {
        if let Some(old) = a.glowing.filter(|g| d.placed(*g).is_some() && a.hovered != Some(*g)) {
            out.glow.push((old, 0));
        }
    }
    if want.as_ref().map(|w| (w.0, w.1)) == a.gizmo_for {
        if a.glowing != glow_on {
            if let Some(g) = glow_on.filter(|_| !moving) {
                out.glow.push((g, GLOW));
            }
            a.glowing = glow_on;
        }
        return out;
    }
    a.glowing = glow_on;
    out.destroy.append(&mut a.gizmo);
    a.gizmo_for = None;
    if let Some((g, q, o)) = want {
        a.gizmo_flip = !a.gizmo_flip;
        let flip = if a.gizmo_flip { 3 } else { 0 };
        let (hx, hy, h) = extent(&o);
        let at = [(hx + GIZMO_MARGIN, 0.0, 0.0, 0.0f32), (0.0, hy + GIZMO_MARGIN, 0.0, std::f32::consts::FRAC_PI_2), (0.0, 0.0, h + 0.5, 0.0)];
        for (i, (e, display, _, _)) in GIZMO.iter().enumerate() {
            let n = 0x10 + i as u64 + flip;
            let (dx, dy, dz, yaw) = at[i];
            let f = WorldObject { guid: aid_guid(*e, n), entry: *e, display: *display, kind: GOOBER, scale: GIZMO_SCALE, x: o.x + dx, y: o.y + dy, z: o.z + dz, o: yaw, pitch: 0.0, roll: 0.0 };
            a.gizmo.push(f.guid);
            out.create.push(f);
        }
        // the halo: as wide as the prop's footprint (its own model's box when known, else about 2 yards)
        let r = (hx.max(hy) * 2.0).clamp(0.5, 30.0);
        let base = crate::models::box_of(HALO.1).map(|b| b.0.max(b.1)).filter(|v| *v > 0.1).unwrap_or(2.0);
        let halo = WorldObject { guid: aid_guid(HALO.0, 0x20 + (a.gizmo_flip as u64)), entry: HALO.0, display: HALO.1, kind: GENERIC, scale: (r / base).clamp(0.2, 15.0), x: o.x, y: o.y, z: o.z + 0.05, o: o.o, pitch: 0.0, roll: 0.0 };
        a.gizmo.push(halo.guid);
        out.create.push(halo);
        out.glow.push((g, GLOW));
        a.gizmo_for = Some((g, q));
    }
    out
}

/// The far view (`cam ...`): snap out (one key and back), zoom past the client's limit, pan, the client's zoom.
fn cam(d: &Den, sel: &Sel, a: &mut Aids, here: Here, w: &[&str], now: u64) -> AidOut {
    let arg = w.get(1).map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let num = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
    let _ = (sel, now);
    let was = a.far;
    match arg.as_str() {
        // snap out: the overview over where you stand (as you face); again (or `cam snap off`): back to you
        "snap" => {
            let off = w.get(2).is_some_and(|v| v.eq_ignore_ascii_case("off"));
            a.far = if off || was.is_some_and(|f| f.snap) { None } else { Some(Far { focus: Here { z: d.floor_at(here.x, here.y), ..here }, back: SNAP_BACK, snap: true }) };
        }
        // the wheel past the client's own limit: `cam far <notches>` (+ out, - in); back to 0: the view is yours again
        "far" => {
            let n = num(2).unwrap_or(1.0).clamp(-20.0, 20.0);
            let mut back = was.map(|f| f.back).unwrap_or(0.0);
            // out: back + FAR_STEP + back / 4; in: the same step undone
            for _ in 0..(n.abs().round() as i32) {
                back = if n > 0.0 { back * 1.25 + FAR_STEP } else { ((back - FAR_STEP) / 1.25).max(0.0) };
            }
            back = back.min(FAR_MAX);
            a.far = if back < 0.5 { None } else { Some(Far { back, snap: false, ..was.unwrap_or(Far { focus: Here { z: d.floor_at(here.x, here.y), ..here }, back, snap: false }) }) };
        }
        // the arrow keys in the far view: the focus moves over the ground (forward is the way the view faces)
        "pan" => {
            let Some(mut f) = was else { return AidOut::default() };
            let v = num(3).unwrap_or(4.0).clamp(-200.0, 200.0);
            let Some((dx, dy, _)) = crate::ovmod::step_offset(&w.get(2).map(|s| s.to_ascii_lowercase()).unwrap_or_default(), f.focus.o, v as f64) else {
                return AidOut::default().said("Usage: cam pan fwd|back|left|right <yards>");
            };
            let (x, y) = (f.focus.x + dx as f32, f.focus.y + dy as f32);
            if ((x - d.spot.x).powi(2) + (y - d.spot.y).powi(2)).sqrt() > FAR_MAX {
                return AidOut::default();
            }
            f.focus = Here { x, y, z: d.floor_at(x, y), ..f.focus };
            a.far = Some(f);
        }
        // the client's own zoom now (yards; the addon keeps count of the wheel), for the ray
        "zoom" => {
            if let Some(z) = num(2).filter(|z| (0.0..=60.0).contains(z)) {
                a.zoom = Some(z);
            }
            return AidOut::default();
        }
        // the build view: up and back over where you stand, a pose Den Lite knows (entering build mode)
        "build" => {
            a.far = Some(Far { focus: Here { z: d.floor_at(here.x, here.y), ..here }, back: BUILD_BACK, snap: false });
        }
        "off" | "near" => a.far = None,
        "where" => return AidOut::default().tell("cam", cam_line(a)),
        _ => return AidOut::default().said("Usage: cam build | snap | far <n> | pan <dir> <yards> | zoom <yards> | off"),
    }
    far_update(a)
}

/// The addon's `cam` line: `far,<back>,<snap 0|1>,<facing deg>` or `near`.
pub fn cam_line(a: &Aids) -> String {
    match a.far {
        Some(f) => format!("far,{:.1},{},{:.0}", f.back, f.snap as u8, f.focus.o.to_degrees().rem_euclid(360.0)),
        None => "near".into(),
    }
}

/// Everything of the aids taken away (logout, the player left): the guids to destroy, and whether the far view was on.
pub fn clear(a: &mut Aids) -> Vec<u64> {
    let mut v: Vec<u64> = a.gizmo.drain(..).collect();
    v.extend(a.ghost.take().map(|g| g.guid));
    a.gizmo_for = None;
    a.glowing = None;
    a.armed = None;
    a.at = None;
    a.far = None;
    a.far_at = None;
    a.grip = None;
    a.hovered = None;
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
    Some(Sample { u: c[0], v: c[1], aspect: c[2], dist: a.zoom.unwrap_or(a.cam.dist), player: [here.x, here.y, here.z, here.o], dest })
}

/// What the den-test reads of the aids (`.den` commands' test-out: "aids").
pub fn view(a: &Aids, sel: &Sel, build: bool) -> serde_json::Value {
    serde_json::json!({
        "armed": aim(a), "ghost": a.ghost.as_ref().map(|g| [g.x, g.y, g.z, g.o]), "at": a.at, "cursor": a.cursor,
        "axis": a.axis.map(String::from), "gizmo": a.gizmo.len(), "sel": sel.guid.map(|g| g & 0xFF_FFFF), "mode": if build { "build" } else { "play" },
        "glow": a.glowing.map(|g| g & 0xFF_FFFF), "drag": a.dragged, "cam": cam_line(a), "far_at": a.far_at, "zoom": a.zoom,
        "hovered": a.hovered.map(|g| g & 0xFF_FFFF), "sel_at": a.sel_at, "sel_uv": a.sel_uv, "build": a.build, "grip": a.grip.is_some(),
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
        // a gallery pick arms at once; nothing shows until the cursor is over the ground (no "in front of you" copy)
        let out = cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 1000);
        assert!(out.create.is_empty() && a.ghost.is_none(), "no copy before the cursor says where");
        assert!(out.addon.contains(&("aim".into(), format!("place,2413,{PLACE_SPELL}"))));
        assert!(d.props.is_empty(), "nothing placed yet");
        // the cursor: the real prop's copy goes where the ray lands
        let out = cmd(&mut d, &mut sel, &mut a, here, "hover 0.2 -0.3 1.7778 12", true, 1200);
        assert_eq!(out.create.len(), 1);
        let g0 = out.create[0].clone();
        assert!(is_ghost(g0.guid) && g0.entry == 2413 && g0.display == den::template(2413).unwrap().display, "the real model");
        assert_eq!(shown(&g0, true), (GENERIC, NOT_SELECTABLE, g0.entry), "the copy never takes the mouse");
        let want = project(&Camera { dist: 12.0, ..Camera::default() }, here, 0.2, -0.3, 1.7778, d.floor_at(here.x, here.y)).unwrap();
        assert!((g0.x - want.0).abs() < 0.01 && (g0.y - want.1).abs() < 0.01);
        assert!((g0.z - d.floor_at(g0.x, g0.y)).abs() < 0.01, "standing on the ground");
        // it follows: made under the other guid before the old one goes; at most a redraw per 100 ms
        let out = cmd(&mut d, &mut sel, &mut a, here, "hover 0.1 -0.3 1.7778 12", true, 1250);
        assert!(out.create.is_empty(), "too soon: kept, not shown");
        let out = cmd(&mut d, &mut sel, &mut a, here, "hover 0.1 -0.35 1.7778 12", true, 1400);
        assert_eq!((out.create.len(), out.destroy.clone()), (1, vec![g0.guid]));
        assert_ne!(out.create[0].guid, g0.guid);
        let kept = a.at.unwrap();
        // one click: placed exactly there, placing ends, the new prop is the selection (glow, gizmo, halo)
        a.gizmo_on = true;
        let out = cmd(&mut d, &mut sel, &mut a, here, "commit", true, 1500);
        let ch = out.change.clone().unwrap();
        assert_eq!(ch.created.len(), 1);
        let made = ch.created[0].clone();
        assert!((made.x - kept[0]).abs() < 0.01 && (made.y - kept[1]).abs() < 0.01);
        assert_eq!(d.props.len(), 1);
        assert!(sel.guid == Some(made.guid) && a.armed.is_none() && a.ghost.is_none() && out.state);
        assert!(out.addon.contains(&("aim".into(), "off".into())) && out.addon.contains(&("placed".into(), (made.guid & 0xFF_FFFF).to_string())));
        assert!(out.destroy.iter().any(|g| is_ghost(*g)), "the copy goes");
        assert!(out.glow.contains(&(made.guid, GLOW)), "it glows: {:?}", out.glow);
        assert_eq!(out.create.iter().filter(|o| gizmo_axis(o.guid).is_some()).count(), 3, "the gizmo's handles");
        assert_eq!(out.create.iter().filter(|o| is_halo(o.guid)).count(), 1, "the halo under it");
        assert!(cmd(&mut d, &mut sel, &mut a, here, "hover 0 -0.2 1.7 12", true, 3000).create.is_empty(), "nothing armed: no copy");
        // Shift+click keeps placing
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413 0.0 -0.4 1.7778 12", true, 4000);
        assert!(a.ghost.is_some(), "the hover rode along with the pick: the copy is there at once");
        let out = cmd(&mut d, &mut sel, &mut a, here, "commit keep", true, 4100);
        assert!(out.change.is_some() && a.armed == Some(Armed::Place(2413)) && a.ghost.is_some() && d.props.len() == 2);
        // the grid snaps the copy
        sel.grid = 2.0;
        cmd(&mut d, &mut sel, &mut a, here, "hover 0.13 -0.31 1.7778 12", true, 5000);
        let at = a.at.unwrap();
        assert!((((at[0] - d.spot.x) / 2.0).round() * 2.0 - (at[0] - d.spot.x)).abs() < 0.01);
        // Esc: the copy goes, nothing armed
        let out = cmd(&mut d, &mut sel, &mut a, here, "preview off", true, 5100);
        assert!(out.destroy.iter().any(|g| is_ghost(*g)));
        assert!(a.armed.is_none() && a.ghost.is_none() && out.addon.contains(&("aim".into(), "off".into())));
        // a visitor can't arm
        assert!(cmd(&mut d, &mut sel, &mut a, here, "preview 2413", false, 6000).say[0].contains("owner"));
        assert!(cmd(&mut d, &mut sel, &mut a, here, "preview 7", true, 6000).say[0].contains("gallery"));
    }

    #[test]
    fn follow_puts_the_copy_in_front_and_it_follows_the_player() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 0);
        let moved = Here { x: here.x + 3.0, o: std::f32::consts::FRAC_PI_2, ..here };
        assert!(follow_player(&d, &sel, &mut a, moved, 500).create.is_empty(), "the cursor's: nothing in front");
        let out = cmd(&mut d, &mut sel, &mut a, here, "follow", true, 600);
        assert!((out.create[0].x - (here.x + AHEAD)).abs() < 0.01);
        let out = follow_player(&d, &sel, &mut a, moved, 1000);
        assert_eq!(out.create.len(), 1);
        assert!((out.create[0].x - moved.x).abs() < 0.01 && (out.create[0].y - (moved.y + AHEAD)).abs() < 0.01);
        assert!(follow_player(&d, &sel, &mut a, moved, 1020).create.is_empty(), "throttled");
        cmd(&mut d, &mut sel, &mut a, here, "hover 0 -0.3 1.7 12", true, 2000);
        assert!(follow_player(&d, &sel, &mut a, here, 3000).create.is_empty(), "the cursor leads now");
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
        assert!(out.change.unwrap().created[0].guid == obj.guid && !out.destroy.contains(&obj.guid), "shown to all, and here");
        assert!(a.armed.is_none(), "one click ends it");
        // Esc while moving shows it again where it was
        cmd(&mut d, &mut sel, &mut a, here, "preview move", true, 700);
        let out = cmd(&mut d, &mut sel, &mut a, here, "preview off", true, 800);
        assert!(out.create.iter().any(|o| o.guid == obj.guid));
        // one undo takes the move back
        d.undo();
        assert!((d.placed(obj.guid).unwrap().obj.x - (here.x + 3.0)).abs() < 0.01);
    }

    #[test]
    fn the_gizmo_and_the_glow_follow_the_selection() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let obj = d.spawn(2413, here.x + 3.0, here.y, here.z, 0.0).unwrap();
        let other = d.spawn(2413, here.x - 3.0, here.y, here.z, 0.0).unwrap();
        sel.guid = Some(obj.guid);
        assert!(sync(&d, &sel, &mut a).create.is_empty(), "off until the addon turns it on");
        let out = cmd(&mut d, &mut sel, &mut a, here, "gizmo on", true, 0);
        assert_eq!(out.create.len(), 4, "three handles and the halo");
        let x = out.create.iter().find(|o| o.entry == 990_001).unwrap();
        assert!((x.x - obj.x - GIZMO_OFF).abs() < 0.01 && (x.z - obj.z).abs() < 0.01 && x.display == 5913, "at its foot, along x");
        let z = out.create.iter().find(|o| o.entry == 990_003).unwrap();
        assert!(z.z > obj.z + 1.0 && (z.x - obj.x).abs() < 0.01, "over its top");
        assert!(out.create.iter().filter(|o| !is_halo(o.guid)).all(|o| shown(o, false) == (GOOBER, 0, o.entry)), "right-click uses the handles");
        let halo = out.create.iter().find(|o| is_halo(o.guid)).unwrap();
        assert_eq!((shown(halo, true), halo.display), ((GENERIC, NOT_SELECTABLE, HALO.0), HALO.1));
        assert_eq!(out.glow, vec![(obj.guid, GLOW)], "the selection glows");
        assert_eq!(a.glowing, Some(obj.guid));
        assert_eq!(sync(&d, &sel, &mut a), AidOut::default(), "nothing changed: nothing sent");
        // the prop turned (made anew: its glow is gone): new handles, the glow again
        let old = a.gizmo.clone();
        d.edit(obj.guid, |p| p.yaw += 45.0).unwrap();
        let out = sync(&d, &sel, &mut a);
        assert_eq!(out.destroy, old);
        assert!(out.create.len() == 4 && out.glow == vec![(obj.guid, GLOW)]);
        // another prop selected: the first stops glowing
        sel.guid = Some(other.guid);
        let out = sync(&d, &sel, &mut a);
        assert!(out.glow.contains(&(obj.guid, 0)) && out.glow.contains(&(other.guid, GLOW)), "{:?}", out.glow);
        // deselected: everything goes, the glow off
        sel.guid = None;
        let out = sync(&d, &sel, &mut a);
        assert_eq!((out.destroy.len(), out.glow.clone()), (4, vec![(other.guid, 0)]));
        assert!(a.gizmo.is_empty() && a.glowing.is_none());
        assert_eq!(query(990_003), Some((GOOBER, 5912, "Drag up and down (Z)", GIZMO_SCALE)));
        assert_eq!(query(HALO.0), Some((GENERIC, HALO.1, HALO.2, 1.0)));
        assert_eq!(query(2413), None);
        // build mode's look makes every prop again: the glow comes back with it
        sel.guid = Some(obj.guid);
        sync(&d, &sel, &mut a);
        let out = cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        assert!(out.look == Some(true) && out.glow.contains(&(obj.guid, GLOW)));
    }

    #[test]
    fn the_nearest_point_on_an_axis_to_the_cursors_ray() {
        // a ray straight down onto (5, 2): along x through the origin, the nearest point is 5 along
        let s = along([0.0, 0.0, 0.0], axis_dir('x'), [5.0, 2.0, 10.0], [0.0, 0.0, -1.0]).unwrap();
        assert!((s - 5.0).abs() < 1e-4, "{s}");
        // up (z): a level ray at height 3 passing over the axis line meets it at 3
        let s = along([1.0, 1.0, 0.0], axis_dir('z'), [-10.0, 1.0, 3.0], [1.0, 0.0, 0.0]).unwrap();
        assert!((s - 3.0).abs() < 1e-4, "{s}");
        // a ray along the axis itself: no answer
        assert!(along([0.0; 3], axis_dir('y'), [0.0, -5.0, 0.0], [0.0, 1.0, 0.0]).is_none());
        // the camera's own ray through the middle of the screen hits the pivot's ground point
        let cam = Camera::default();
        let h = Here { x: 10.0, y: 20.0, z: 0.0, o: 0.0 };
        let (e, d) = ray(&cam, h, 0.0, 0.0, 1.7778).unwrap();
        assert!((dot(d, d) - 1.0).abs() < 1e-4);
        let s = along([10.0 + cam.pivot / cam.pitch.tan(), 0.0, 0.0], axis_dir('y'), e, d).unwrap();
        assert!((s - 20.0).abs() < 0.01, "{s}");
    }

    #[test]
    fn dragging_a_handle_moves_the_prop_along_its_axis_and_a_click_drops_it() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let obj = d.spawn(2413, here.x + 6.0, here.y, here.z, 0.0).unwrap();
        sel.guid = Some(obj.guid);
        let out = cmd(&mut d, &mut sel, &mut a, here, "gizmo on", true, 0);
        let xh = out.create.iter().find(|o| o.entry == 990_001).unwrap().guid;
        // a right-click on the red handle: grabbed; the real prop hidden here, its glowing copy in its place
        let out = use_object(&d, &mut sel, &mut a, xh, true, 100);
        assert!(matches!(a.armed, Some(Armed::Axis { axis: 'x', .. })));
        assert!(out.destroy.contains(&obj.guid) && out.destroy.contains(&xh), "the prop and the handles hidden while it moves");
        let copy = out.create.iter().find(|o| is_ghost(o.guid)).unwrap();
        assert!((copy.x - obj.x).abs() < 0.01 && out.glow.contains(&(copy.guid, GLOW)));
        assert!(out.addon.contains(&("aim".into(), format!("axis,{},x", obj.guid & 0xFF_FFFF))));
        // the cursor moves: the copy goes to the point of the x line nearest the ray, y and z unchanged
        let (u, v) = (0.25, -0.1);
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("hover {u} {v} 1.7778 12"), true, 300);
        let (e, dir) = ray(&Camera { dist: 12.0, ..Camera::default() }, here, u, v, 1.7778).unwrap();
        let s = along([obj.x, obj.y, obj.z], [1.0, 0.0, 0.0], e, dir).unwrap();
        let c = out.create.iter().find(|o| is_ghost(o.guid)).unwrap();
        assert!((c.x - (obj.x + s)).abs() < 0.01 && (c.y - obj.y).abs() < 1e-4 && (c.z - obj.z).abs() < 1e-4, "{} {}", c.x, obj.x + s);
        assert!(out.addon.iter().any(|(n, t)| n == "drag" && t.starts_with("x,")));
        // the wheel: on by a step
        cmd(&mut d, &mut sel, &mut a, here, "grab nudge 1", true, 500);
        assert!((a.at.unwrap()[0] - (obj.x + s + 1.0)).abs() < 0.01);
        // a click drops it: one change, the message, one undo step
        let undo = d.undo_depth();
        let out = cmd(&mut d, &mut sel, &mut a, here, "commit", true, 600);
        let moved = d.placed(obj.guid).unwrap().obj.x;
        assert!((moved - (obj.x + s + 1.0)).abs() < 0.01);
        assert_eq!(d.undo_depth(), undo + 1);
        assert_eq!(out.say, vec![format!("Moved Wooden Chair {:+.2} yd along X.", s + 1.0)]);
        assert!(out.addon.contains(&("moved".into(), format!("x,{:+.2}", s + 1.0))));
        assert!(a.armed.is_none() && a.axis.is_none() && out.glow.contains(&(obj.guid, GLOW)) && out.create.len() == 4, "dropped: selected again");
        // grab, then Esc: back where it was
        cmd(&mut d, &mut sel, &mut a, here, "grab z", true, 700);
        cmd(&mut d, &mut sel, &mut a, here, "hover 0 0.3 1.7778 12", true, 900);
        let out = cmd(&mut d, &mut sel, &mut a, here, "grab off", true, 1000);
        assert!(out.create.iter().any(|o| o.guid == obj.guid && (o.x - moved).abs() < 0.01), "put back");
        assert!((d.placed(obj.guid).unwrap().obj.z - obj.z).abs() < 0.01 && a.armed.is_none());
        // a locked prop can't be grabbed
        d.set_lock(obj.guid, true).unwrap();
        assert!(cmd(&mut d, &mut sel, &mut a, here, "grab x", true, 1100).say[0].contains("locked"));
    }

    #[test]
    fn snap_out_and_zoom_past_the_limit_keep_the_cursor_ray_true() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        cmd(&mut d, &mut sel, &mut a, here, "cam zoom 12", true, 0);
        let near = a.view(here);
        let out = cmd(&mut d, &mut sel, &mut a, here, "cam snap", true, 0);
        let Some(FarOut::Show(at)) = out.far else { panic!("{:?}", out.far) };
        assert!(out.addon.contains(&("cam".into(), "far,35.0,1,0".into())));
        // the far-sight object: back and up along the camera's line of sight
        let (sp, cp) = a.cam.pitch.sin_cos();
        assert!((at[0] - (here.x - cp * SNAP_BACK)).abs() < 0.01 && (at[2] - (here.z + a.cam.pivot + sp * SNAP_BACK)).abs() < 0.01);
        // from the far view the middle of the screen still lands on the same ground, and the view's camera sits
        // where the client puts it: the client's zoom behind the far-sight object
        let (fh, fc) = a.view(here);
        assert!((fc.dist - (12.0 + SNAP_BACK)).abs() < 1e-4);
        let p0 = project(&near.1, near.0, 0.0, 0.0, 1.7778, here.z).unwrap();
        let p1 = project(&fc, fh, 0.0, 0.0, 1.7778, here.z).unwrap();
        assert!((p0.0 - p1.0).abs() < 0.01 && (p0.1 - p1.1).abs() < 0.01);
        let (eye, _) = eye_of(&fc, fh);
        let (eye2, _) = eye_of(&Camera { dist: 12.0, pivot: 0.0, ..a.cam }, Here { x: at[0], y: at[1], z: at[2], o: at[3] });
        assert!((0..3).all(|k| (eye[k] - eye2[k]).abs() < 0.01), "{eye:?} {eye2:?}");
        // placing from up there: the copy lands where the far view's ray says
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 100);
        cmd(&mut d, &mut sel, &mut a, here, "hover 0.3 -0.2 1.7778 12", true, 300);
        let want = project(&fc, fh, 0.3, -0.2, 1.7778, d.floor_at(here.x, here.y)).unwrap();
        assert!((a.at.unwrap()[0] - want.0).abs() < 0.05 && (a.at.unwrap()[1] - want.1).abs() < 0.05);
        cmd(&mut d, &mut sel, &mut a, here, "preview off", true, 400);
        // the arrows pan it; the player walking doesn't
        let out = cmd(&mut d, &mut sel, &mut a, here, "cam pan fwd 10", true, 500);
        assert!(matches!(out.far, Some(FarOut::Move(_, t)) if (t[0] - (at[0] + 10.0)).abs() < 0.01));
        // the same key again: back to the player's own view
        let out = cmd(&mut d, &mut sel, &mut a, here, "cam snap", true, 600);
        assert_eq!(out.far, Some(FarOut::Off));
        assert!(a.far.is_none() && a.far_at.is_none() && out.addon.contains(&("cam".into(), "near".into())));
        // the wheel past the client's limit: each notch further, and back in to the player
        let out = cmd(&mut d, &mut sel, &mut a, here, "cam far 1", true, 700);
        assert!(matches!(out.far, Some(FarOut::Show(_))) && (a.far.unwrap().back - FAR_STEP).abs() < 1e-4);
        cmd(&mut d, &mut sel, &mut a, here, "cam far 3", true, 800);
        let b4 = a.far.unwrap().back;
        assert!(b4 > 4.0 * FAR_STEP, "each notch a bit further: {b4}");
        cmd(&mut d, &mut sel, &mut a, here, "cam far 40", true, 900);
        cmd(&mut d, &mut sel, &mut a, here, "cam far 20", true, 900);
        assert!(a.far.unwrap().back <= FAR_MAX);
        cmd(&mut d, &mut sel, &mut a, here, "cam off", true, 950);
        cmd(&mut d, &mut sel, &mut a, here, "cam far 2", true, 1000);
        let out = cmd(&mut d, &mut sel, &mut a, here, "cam far -2", true, 1100);
        assert_eq!(out.far, Some(FarOut::Off), "all the way in: the view is the player's again");
        assert!(cmd(&mut d, &mut sel, &mut a, here, "cam pan fwd 3", true, 1200).far.is_none(), "near: nothing to pan");
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, "cam where", true, 1300).addon, vec![("cam".to_string(), "near".to_string())]);
    }

    #[test]
    fn right_clicking_a_prop_in_build_mode_selects_it() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let obj = d.spawn(2413, here.x + 3.0, here.y, here.z, 0.0).unwrap();
        let out = cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        assert_eq!(out.look, Some(true));
        assert_eq!(shown(&obj, true), (GOOBER, 0, twin(obj.entry)), "highlighted, usable: under its build twin");
        assert_eq!(shown(&obj, false), (obj.kind, 0, obj.entry), "in play as it is");
        // the client asks for the twin: the same model, name and size, a goober; the prop's own entry as it is
        let t = den::template(obj.entry).unwrap();
        assert_eq!(query_any(twin(obj.entry)), Some((GOOBER, t.display, t.name, t.size)));
        assert_eq!(query_any(obj.entry), Some((t.kind, t.display, t.name, t.size)));
        assert_eq!(untwin(obj.entry), None);
        assert_eq!(query_any(GIZMO[0].0), query(GIZMO[0].0));
        assert_eq!(query_any(HALO.0), query(HALO.0));
        cmd(&mut d, &mut sel, &mut a, here, "gizmo on", true, 0);
        assert_eq!(use_object(&d, &mut sel, &mut a, obj.guid, false, 0), AidOut::default(), "play mode: left to the game");
        assert_eq!(sel.guid, None);
        let out = use_object(&d, &mut sel, &mut a, obj.guid, true, 0);
        assert_eq!(sel.guid, Some(obj.guid));
        assert!(out.state && out.create.len() == 4 && out.glow == vec![(obj.guid, GLOW)]);
        assert!(out.addon.contains(&("picked".into(), (obj.guid & 0xFF_FFFF).to_string())));
        assert_eq!(use_object(&d, &mut sel, &mut a, 12345, true, 0), AidOut::default(), "not ours: nothing");
        // a handle with nothing selected only names its axis
        sel.guid = None;
        let h = aid_guid(GIZMO[1].0, 0x11);
        assert_eq!(use_object(&d, &mut sel, &mut a, h, true, 0).addon, vec![("axis".to_string(), "y".to_string())]);
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
        a.zoom = Some(12.0);
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
        assert!(is_aid("preview 2413") && is_aid("HOVER 0 0") && is_aid("mode build") && is_aid("grab x") && is_aid("cam snap") && !is_aid("move x 1") && !is_aid("spawn 1"));
        let dir = std::env::temp_dir().join(format!("den-cam-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(Camera::load(&dir), Camera::default());
        std::fs::write(dir.join(CAMERA_FILE), r#"{"fov":1.2,"pivot":1.8,"dist":14.0,"pitch":0.6}"#).unwrap();
        assert_eq!(Camera::load(&dir), Camera { fov: 1.2, pivot: 1.8, dist: 14.0, pitch: 0.6, far_pivot: 0.0 });
        std::fs::write(dir.join(CAMERA_FILE), r#"{"fov":9,"pivot":1.8,"dist":14.0,"pitch":0.6}"#).unwrap();
        assert_eq!(Camera::load(&dir), Camera::default(), "nonsense: the guesses");
        let _ = std::fs::remove_dir_all(&dir);
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, "where", true, 0).addon, vec![("ghost".to_string(), "-".to_string())]);
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, "axis z", true, 0).addon, vec![("axis".to_string(), "z".to_string())]);
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, "axis off", true, 0).addon, vec![("axis".to_string(), "-".to_string())]);
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 0);
        cmd(&mut d, &mut sel, &mut a, here, "hover 0 -0.3 1.7778 12", true, 0);
        cmd(&mut d, &mut sel, &mut a, here, "preview turn 90", true, 0);
        assert!((a.at.unwrap()[3] - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert!(where_line(&a).ends_with(",90"));
        assert_eq!(clear(&mut a).len(), 1);
    }

    /// Where the prop's middle is on the screen now (what the mouse would point at).
    fn uv_of(a: &Aids, here: Here, o: &WorldObject) -> [f32; 2] {
        let (from, cam) = a.view(here);
        screen(&cam, from, [o.x, o.y, o.z + extent(o).2 / 2.0], 16.0 / 9.0).unwrap()
    }

    fn line(u: f32, v: f32) -> String {
        format!("{u:.4} {v:.4} 1.7778 12")
    }

    #[test]
    fn the_screen_point_of_a_spot_is_where_its_ray_goes() {
        let cam = Camera { dist: 12.0, ..Camera::default() };
        let h = Here { x: 10.0, y: -4.0, z: 3.0, o: 0.7 };
        for (u, v) in [(0.0, 0.0), (0.4, -0.3), (-0.8, 0.5)] {
            let (e, d) = ray(&cam, h, u, v, 1.6).unwrap();
            let p = add(e, d, 9.0);
            let s = screen(&cam, h, p, 1.6).unwrap();
            assert!((s[0] - u).abs() < 1e-3 && (s[1] - v).abs() < 1e-3, "{u} {v} -> {s:?}");
        }
        let (e, d) = ray(&cam, h, 0.0, 0.0, 1.6).unwrap();
        assert!(screen(&cam, h, add(e, d, -5.0), 1.6).is_none(), "behind the camera");
    }

    #[test]
    fn a_click_selects_what_is_under_the_cursor_and_empty_ground_clears() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let near = d.spawn(2413, here.x + 6.0, here.y + 1.5, here.z, 0.0).unwrap();
        let far = d.spawn(2413, here.x + 9.0, here.y - 3.0, here.z, 0.0).unwrap();
        cmd(&mut d, &mut sel, &mut a, here, "gizmo on", true, 0);
        cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        let [u, v] = uv_of(&a, here, &near);
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("click {}", line(u, v)), true, 10);
        assert_eq!(sel.guid, Some(near.guid));
        assert!(out.state && out.glow.contains(&(near.guid, GLOW)) && out.create.iter().filter(|o| gizmo_axis(o.guid).is_some()).count() == 3, "{out:?}");
        assert!(out.addon.contains(&("selected".into(), (near.guid & 0xFF_FFFF).to_string())));
        // the other one: it is the selection now, the first stops glowing
        let [u2, v2] = uv_of(&a, here, &far);
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("click {}", line(u2, v2)), true, 20);
        assert_eq!(sel.guid, Some(far.guid));
        assert!(out.glow.contains(&(near.guid, 0)) && out.glow.contains(&(far.guid, GLOW)), "{:?}", out.glow);
        // Shift adds the first back: both selected, both glow
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("click {} add", line(u, v)), true, 30);
        assert_eq!((sel.guid, sel.more.clone()), (Some(far.guid), vec![near.guid]));
        assert!(out.glow.contains(&(near.guid, GLOW)) && out.say == vec!["2 selected.".to_string()]);
        // the sky (above the horizon): nothing there, the selection cleared, every glow off
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("click {}", line(0.0, 0.95)), true, 40);
        assert!(sel.guid.is_none() && sel.more.is_empty());
        assert!(out.glow.contains(&(near.guid, 0)) && out.glow.contains(&(far.guid, 0)), "{:?}", out.glow);
        assert!(out.addon.contains(&("selected".into(), "-".into())));
        // Shift+click on nothing changes nothing
        cmd(&mut d, &mut sel, &mut a, here, &format!("click {}", line(u, v)), true, 50);
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, &format!("click {} add", line(0.0, 0.95)), true, 60), AidOut::default());
        assert_eq!(sel.guid, Some(near.guid));
        // a hidden prop is never picked
        d.props.iter_mut().find(|p| p.obj.guid == near.guid).unwrap().prop.hidden = Some(true);
        cmd(&mut d, &mut sel, &mut a, here, &format!("click {}", line(u, v)), true, 70);
        assert_ne!(sel.guid, Some(near.guid));
        assert!(is_aid("click 0 0") && is_aid("drag 0 0"));
    }

    #[test]
    fn in_build_mode_the_prop_under_the_cursor_glows_and_stops_when_left() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let chair = d.spawn(2413, here.x + 6.0, here.y, here.z, 0.0).unwrap();
        let [u, v] = uv_of(&a, here, &chair);
        // play mode: the hover does nothing
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(u, v)), true, 0), AidOut::default());
        cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(u, v)), true, 1000);
        assert_eq!(out.glow, vec![(chair.guid, GLOW)]);
        assert_eq!(out.addon, vec![("hovered".to_string(), format!("{} Wooden Chair", chair.guid & 0xFF_FFFF))]);
        assert_eq!(a.hovered, Some(chair.guid));
        // still on it: nothing sent
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(u + 0.001, v)), true, 1200), AidOut::default());
        // off it too soon after: kept until the next hover (at most one change per HOVER_MS)
        assert_eq!(cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(0.0, 0.95)), true, 1050), AidOut::default());
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(0.0, 0.95)), true, 1300);
        assert_eq!(out.glow, vec![(chair.guid, 0)]);
        assert_eq!(out.addon, vec![("hovered".to_string(), "-".to_string())]);
        // the selection keeps its glow when the cursor leaves it
        cmd(&mut d, &mut sel, &mut a, here, "gizmo on", true, 1400);
        cmd(&mut d, &mut sel, &mut a, here, &format!("click {}", line(u, v)), true, 1500);
        cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(u, v)), true, 1700);
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(0.0, 0.95)), true, 1900);
        assert!(out.glow.is_empty() && out.addon == vec![("hovered".to_string(), "-".to_string())], "{out:?}");
        // while something is armed the hover moves that, it doesn't pick
        cmd(&mut d, &mut sel, &mut a, here, "preview 2413", true, 2000);
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(u, v)), true, 2200);
        assert!(out.addon.iter().all(|(n, _)| n != "hovered"));
    }

    #[test]
    fn press_and_drag_moves_the_prop_under_the_cursor_and_letting_go_is_one_undo() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let chair = d.spawn(2413, here.x + 6.0, here.y, here.z, 0.0).unwrap();
        cmd(&mut d, &mut sel, &mut a, here, "gizmo on", true, 0);
        cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        // nothing under the cursor: the camera keeps the drag
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("drag {}", line(0.0, 0.95)), true, 10);
        assert_eq!(out.addon, vec![("grabbed".to_string(), "-".to_string())]);
        assert!(a.armed.is_none());
        // pressed on the chair (not selected yet): selected, hidden here, its glowing copy where it stands
        let [u, v] = uv_of(&a, here, &chair);
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("drag {}", line(u, v)), true, 100);
        assert_eq!((sel.guid, a.armed), (Some(chair.guid), Some(Armed::Move(chair.guid))));
        assert!(out.destroy.contains(&chair.guid) && out.state);
        let copy = out.create.iter().find(|o| is_ghost(o.guid)).unwrap();
        assert!((copy.x - chair.x).abs() < 1e-3 && (copy.y - chair.y).abs() < 1e-3 && out.glow.contains(&(copy.guid, GLOW)));
        assert!(out.addon.contains(&("grabbed".into(), (chair.guid & 0xFF_FFFF).to_string())));
        assert!(out.addon.contains(&("aim".into(), format!("move,{},{PLACE_SPELL}", chair.guid & 0xFF_FFFF))));
        // the cursor moves: the copy moves as the grabbed spot does on its level (the same screen point of a spot 3
        // yards further and 2 to the left at the grabbed height lands the foot 3 and 2 on)
        let g = a.grip.unwrap();
        let (from, cam) = a.view(here);
        let spot = [chair.x - g.off[0] + 3.0, chair.y - g.off[1] + 2.0, g.z];
        let [u2, v2] = screen(&cam, from, spot, 16.0 / 9.0).unwrap();
        cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(u2, v2)), true, 300);
        let at = a.at.unwrap();
        assert!((at[0] - (chair.x + 3.0)).abs() < 0.02 && (at[1] - (chair.y + 2.0)).abs() < 0.02, "{at:?}");
        assert!((at[2] - chair.z).abs() < 1e-3, "on the ground");
        // the wheel turns it while it is held
        cmd(&mut d, &mut sel, &mut a, here, "preview turn 90", true, 350);
        // let go: the real chair there, turned, the same prop; one undo step
        let undo = d.undo_depth();
        let out = cmd(&mut d, &mut sel, &mut a, here, "commit", true, 400);
        let p = d.placed(chair.guid).unwrap().obj.clone();
        assert!((p.x - at[0]).abs() < 0.02 && (p.y - at[1]).abs() < 0.02 && (p.o - std::f32::consts::FRAC_PI_2).abs() < 0.01, "{p:?}");
        assert_eq!(d.undo_depth(), undo + 1);
        assert!(a.armed.is_none() && a.grip.is_none() && out.glow.contains(&(chair.guid, GLOW)), "dropped: selected and glowing");
        d.undo();
        assert!((d.placed(chair.guid).unwrap().obj.x - chair.x).abs() < 0.02, "one undo puts it back");
        // the grid: the foot lands on it
        sel.grid = 1.0;
        let [u, v] = uv_of(&a, here, &d.placed(chair.guid).unwrap().obj.clone());
        cmd(&mut d, &mut sel, &mut a, here, &format!("drag {}", line(u, v)), true, 500);
        cmd(&mut d, &mut sel, &mut a, here, &format!("hover {}", line(u + 0.07, v - 0.03)), true, 700);
        let at = a.at.unwrap();
        assert!((((at[0] - d.spot.x).round()) - (at[0] - d.spot.x)).abs() < 0.01, "{at:?}");
        // Esc while dragging: back where it was, nothing changed
        let out = cmd(&mut d, &mut sel, &mut a, here, "preview off", true, 800);
        assert!(out.create.iter().any(|o| o.guid == chair.guid) && a.grip.is_none());
        assert!((d.placed(chair.guid).unwrap().obj.x - chair.x).abs() < 0.02);
        // a locked prop isn't dragged; a visitor can't drag
        d.set_lock(chair.guid, true).unwrap();
        let out = cmd(&mut d, &mut sel, &mut a, here, &format!("drag {}", line(u, v)), true, 900);
        assert!(out.say[0].contains("locked") && a.armed.is_none());
        assert!(cmd(&mut d, &mut sel, &mut a, here, &format!("drag {}", line(u, v)), false, 1000).say[0].contains("owner"));
    }

    #[test]
    fn the_build_view_is_a_known_pose_and_picking_is_exact_from_it() {
        let (mut d, here) = den();
        let (mut sel, mut a) = (Sel::default(), Aids::default());
        let chair = d.spawn(2413, here.x + 8.0, here.y + 2.0, here.z, 0.0).unwrap();
        cmd(&mut d, &mut sel, &mut a, here, "mode build", true, 0);
        cmd(&mut d, &mut sel, &mut a, here, "cam zoom 12", true, 0);
        let out = cmd(&mut d, &mut sel, &mut a, here, "cam build", true, 0);
        assert!(matches!(out.far, Some(FarOut::Show(_))), "{:?}", out.far);
        assert_eq!(a.far.map(|f| (f.back, f.snap)), Some((BUILD_BACK, false)));
        assert!(out.addon.contains(&("cam".into(), format!("far,{BUILD_BACK:.1},0,0"))));
        // the player walks off: the view (and the ray) stay
        let walked = Here { x: here.x - 5.0, o: 2.0, ..here };
        let [u, v] = uv_of(&a, walked, &chair);
        cmd(&mut d, &mut sel, &mut a, walked, &format!("click {}", line(u, v)), true, 10);
        assert_eq!(sel.guid, Some(chair.guid));
        assert_eq!(a.sel_uv.map(|s| ((s[0] - u).abs() < 1e-3, (s[1] - v).abs() < 1e-3)), Some((true, true)), "the den-test reads it");
        // snap toggles the overview and back to the near build camera; leaving build mode gives the view back
        cmd(&mut d, &mut sel, &mut a, here, "cam build", true, 20);
        let out = cmd(&mut d, &mut sel, &mut a, here, "mode play", true, 30);
        assert_eq!(out.far, Some(FarOut::Off));
        assert!(a.far.is_none() && !a.build && out.addon.contains(&("cam".into(), "near".into())));
    }
}
