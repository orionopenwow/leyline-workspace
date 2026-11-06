//! The editor's mouse, as it should behave (owner 5 Nov: "match the movement on the mouse and everything ... compare it
//! and guarantee it works"). This is the expected model the replay harness (replay.rs) holds Den Lite's editor
//! (gizmo.rs) to, frame by frame: written again from the rules in docs/DEN-EDITOR-PARITY.md and DEN-PLACEMENT.md, not
//! from gizmo.rs, so a change to either that makes them disagree shows up as a failed frame.
//!
//! The rules (Noggit's and the owner's LANDSCAPE editor's, as the Den has them):
//!
//! 1. A card picked: armed to place, turned as the last one placed (or as you face). Nothing is shown until the cursor
//!    is over the ground; a first ray past the horizon shows it 5 yards ahead of the camera's focus.
//! 2. While something is in hand, each hover puts its foot where the cursor's ray meets the ground, on the grid when
//!    Snap is on, held a grid step inside the Den's edge, on the top of a prop under it (within 2.5 yards of the
//!    ground, never its own), else on the ground plus its own lift.
//! 3. A still click on a prop picks it up (it doesn't jump: its foot keeps its offset from where the ray met the
//!    ground, at most its own footprint); the next click (commit) puts it down; Esc puts it back where it was.
//! 4. A press and drag keeps the grabbed point under the cursor on the plane at the grabbed height.
//! 5. A turn (Q / E, the wheel) turns what is in hand; the next card starts turned as the last placed.
//! 6. One commit is one undo.
//!
//! Also here: the input script the harness replays (`.moves` text, [`parse_script`]) and a deterministic generator of
//! random scripts ([`fuzz_script`]). Std only: `rustc --test src/movemodel.rs`.

pub type V3 = [f32; 3];

/// The build camera (gizmo.rs Camera's numbers).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cam {
    pub fov: f32,
    pub pivot: f32,
    pub dist: f32,
    pub pitch: f32,
}

impl Default for Cam {
    fn default() -> Self {
        Cam { fov: std::f32::consts::FRAC_PI_2, pivot: 2.0, dist: 12.0, pitch: 0.5 }
    }
}

pub const REACH: f32 = 60.0;
pub const FIRST_AHEAD: f32 = 5.0;
pub const SURFACE_REACH: f32 = 2.5;
pub const TIE: f32 = 0.05;
pub const GROUND_STEP: f32 = 0.5;

/// A prop as the model sees it: where, its turn, its pick box (half sizes as the ray sees them, its bottom over the
/// foot and its height) and its turned footprint over the world's axes (what a carry's offset is held within).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MProp {
    pub id: u64,
    pub at: V3,
    pub o: f32,
    pub half: [f32; 2],
    pub bottom: f32,
    pub height: f32,
    pub ext: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MArmed {
    Place,
    /// Carried (a click) or dragged (a press): the prop, the plane height and the foot's offset from the cursor's point.
    Move { id: u64, carry: bool, z: f32, off: [f32; 2] },
}

/// The model's world: the Den (its middle, floor and reach), the player, the camera, the props, what is in hand.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub cam: Cam,
    pub zoom: Option<f32>,
    pub aspect: f32,
    pub mid: [f32; 2],
    pub floor: f32,
    pub reach: f32,
    pub grid: f32,
    /// The player: x, y, z, facing.
    pub me: [f32; 4],
    pub props: Vec<MProp>,
    pub armed: Option<MArmed>,
    /// Where the copy in hand is: x, y, z, facing.
    pub at: Option<[f32; 4]>,
    pub yaw: f32,
    pub lift: f32,
    pub next_yaw: Option<f32>,
}

fn add(a: V3, b: V3, k: f32) -> V3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn norm(v: V3) -> Option<V3> {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (n > 1e-6 && n.is_finite()).then(|| [v[0] / n, v[1] / n, v[2] / n])
}

/// The camera's eye and the ray through the screen point (u, v) (-1..1, right and up), for a player at `me`.
/// The screen's diagonal spans the field of view; the camera sits `dist` behind the pivot over the player's feet,
/// looking down by `pitch`, turned as the player faces.
pub fn ray(cam: &Cam, me: [f32; 4], u: f32, v: f32, aspect: f32) -> Option<(V3, V3)> {
    if ![u, v, aspect].iter().all(|x| x.is_finite()) {
        return None;
    }
    let (o, p) = (me[3], cam.pitch);
    let fwd = [p.cos() * o.cos(), p.cos() * o.sin(), -p.sin()];
    let right = [o.sin(), -o.cos(), 0.0];
    // up = right x forward
    let up = [right[1] * fwd[2] - right[2] * fwd[1], right[2] * fwd[0] - right[0] * fwd[2], right[0] * fwd[1] - right[1] * fwd[0]];
    let pivot = [me[0], me[1], me[2] + cam.pivot];
    let eye = add(pivot, fwd, -cam.dist);
    let a = aspect.clamp(0.2, 5.0);
    let half_h = (cam.fov / 2.0).tan() / (1.0 + a * a).sqrt();
    let (u, v) = (u.clamp(-1.0, 1.0), v.clamp(-1.0, 1.0));
    let d = [
        fwd[0] + right[0] * u * half_h * a + up[0] * v * half_h,
        fwd[1] + right[1] * u * half_h * a + up[1] * v * half_h,
        fwd[2] + right[2] * u * half_h * a + up[2] * v * half_h,
    ];
    Some((eye, norm(d)?))
}

/// Ray against a turned box: the distance it enters (negative: it starts inside), None: misses.
pub fn box_hit(eye: V3, dir: V3, b: &MProp) -> Option<f32> {
    let (s, c) = b.o.sin_cos();
    let rel = [eye[0] - b.at[0], eye[1] - b.at[1], eye[2] - b.at[2]];
    // into the box's own axes (turned back by its facing)
    let o = [rel[0] * c + rel[1] * s, -rel[0] * s + rel[1] * c, rel[2]];
    let d = [dir[0] * c + dir[1] * s, -dir[0] * s + dir[1] * c, dir[2]];
    let lo = [-b.half[0], -b.half[1], b.bottom];
    let hi = [b.half[0], b.half[1], b.bottom + b.height];
    let mut t0 = f32::NEG_INFINITY;
    let mut t1 = f32::INFINITY;
    for k in 0..3 {
        if d[k].abs() < 1e-9 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
        } else {
            let a = (lo[k] - o[k]) / d[k];
            let z = (hi[k] - o[k]) / d[k];
            t0 = t0.max(a.min(z));
            t1 = t1.min(a.max(z));
        }
    }
    (t0 <= t1 && t1 >= 0.0).then_some(t0)
}

fn volume(b: &MProp) -> f32 {
    b.half[0] * b.half[1] * 4.0 * b.height
}

/// The prop the cursor's ray hits first within `reach` (ties within [`TIE`]: the smaller one, then the newer), or the
/// smallest one the camera is inside; and how far along the ray.
pub fn pick(eye: V3, dir: V3, props: &[MProp], reach: f32) -> Option<(u64, f32)> {
    let smaller = |a: &MProp, b: &MProp| {
        let (va, vb) = (volume(a), volume(b));
        if (va - vb).abs() > 1e-4 { va < vb } else { a.id > b.id }
    };
    let mut best: Option<(&MProp, f32)> = None;
    let mut inside: Option<&MProp> = None;
    let hits: Vec<(&MProp, f32)> = props.iter().filter_map(|p| box_hit(eye, dir, p).map(|t| (p, t))).collect();
    let nearest = hits.iter().filter(|h| h.1 >= 0.0 && h.1 <= reach).map(|h| h.1).fold(f32::INFINITY, f32::min);
    for (p, t) in hits {
        if t < 0.0 {
            if inside.map_or(true, |c| smaller(p, c)) {
                inside = Some(p);
            }
        } else if t <= reach && t - nearest <= TIE && best.map_or(true, |(c, _)| smaller(p, c)) {
            best = Some((p, t));
        }
    }
    best.map(|(p, t)| (p.id, t)).or_else(|| inside.map(|p| (p.id, 0.0)))
}

impl Model {
    pub fn new(mid: [f32; 2], floor: f32, reach: f32) -> Model {
        Model {
            cam: Cam::default(),
            zoom: None,
            aspect: 16.0 / 9.0,
            mid,
            floor,
            reach,
            grid: 0.0,
            me: [mid[0], mid[1], floor, 0.0],
            props: Vec::new(),
            armed: None,
            at: None,
            yaw: 0.0,
            lift: 0.0,
            next_yaw: None,
        }
    }

    fn cam_now(&self) -> Cam {
        Cam { dist: self.zoom.unwrap_or(self.cam.dist), ..self.cam }
    }

    fn ray(&self, u: f32, v: f32) -> Option<(V3, V3)> {
        ray(&self.cam_now(), self.me, u, v, self.aspect)
    }

    fn inside(&self, x: f32, y: f32) -> bool {
        ((x - self.mid[0]).powi(2) + (y - self.mid[1]).powi(2)).sqrt() <= self.reach
    }

    fn snap(&self, v: f32, mid: f32) -> f32 {
        if self.grid > 0.0 { mid + ((v - mid) as f64 / self.grid as f64).round() as f32 * self.grid } else { v }
    }

    /// The highest top of a prop under (x, y) a carried thing may stand on (not the carried one), else the ground +
    /// the lift.
    fn surface(&self, x: f32, y: f32, own: Option<u64>) -> f32 {
        let ground = self.floor;
        let limit = ground + self.lift.max(0.0) + SURFACE_REACH + 0.5;
        let mut top: Option<f32> = None;
        for p in self.props.iter().filter(|p| Some(p.id) != own) {
            let (s, c) = p.o.sin_cos();
            let (dx, dy) = (x - p.at[0], y - p.at[1]);
            let (lx, ly) = (dx * c + dy * s, -dx * s + dy * c);
            let (hx, hy) = (p.half[0] / 1.05, p.half[1] / 1.05);
            let t = p.at[2] + p.bottom + p.height;
            if lx.abs() <= hx && ly.abs() <= hy && t <= limit {
                top = Some(top.map_or(t, |m: f32| m.max(t)));
            }
        }
        match top.filter(|t| *t > ground + 0.05) {
            Some(t) => t,
            None => ground + self.lift,
        }
    }

    /// The cursor's ground point (x, y) for the copy in hand: on the grid, a grid step inside the edge.
    fn put(&mut self, x: f32, y: f32) {
        let own = match self.armed {
            Some(MArmed::Move { id, .. }) => Some(id),
            _ => None,
        };
        let r = (self.reach - self.grid.max(0.5)).max(1.0);
        let (dx, dy) = (x - self.mid[0], y - self.mid[1]);
        let n = (dx * dx + dy * dy).sqrt();
        let (x, y) = if n > r { (self.mid[0] + dx * r / n, self.mid[1] + dy * r / n) } else { (x, y) };
        let (sx, sy) = (self.snap(x, self.mid[0]), self.snap(y, self.mid[1]));
        let z = self.surface(sx, sy, own);
        if self.at.is_none() || self.inside(sx, sy) {
            self.at = Some([sx, sy, z, self.yaw]);
        }
    }

    /// Where the ray meets the flat ground, within the reach from the player (the screen-space projection), or None.
    fn project(&self, u: f32, v: f32) -> Option<[f32; 2]> {
        let (e, d) = self.ray(u, v)?;
        if d[2] >= -1e-4 {
            return None;
        }
        let t = (self.floor - e[2]) / d[2];
        if t <= 0.0 {
            return None;
        }
        let p = add(e, d, t);
        let far = ((p[0] - self.me[0]).powi(2) + (p[1] - self.me[1]).powi(2)).sqrt();
        (far <= REACH + self.cam_now().dist).then_some([p[0], p[1]])
    }

    /// Where the ray meets the ground along its own length (the carry's way), or None.
    fn ground_along(&self, e: V3, d: V3) -> Option<[f32; 2]> {
        let reach = self.cam_now().dist + REACH;
        if e[2] <= self.floor {
            return Some([e[0], e[1]]);
        }
        if d[2] >= 0.0 {
            return None;
        }
        let t = (self.floor - e[2]) / d[2];
        // the ground is searched in half-yard steps up to the reach (and the step that crosses it)
        let last = (reach / GROUND_STEP).floor() * GROUND_STEP + GROUND_STEP;
        (t <= last.max(reach)).then(|| {
            let p = add(e, d, t);
            [p[0], p[1]]
        })
    }

    fn plane(&self, e: V3, d: V3, z: f32) -> Option<[f32; 2]> {
        if d[2].abs() < 1e-4 {
            return None;
        }
        let t = (z - e[2]) / d[2];
        (t > 0.0 && t <= self.cam_now().dist + REACH).then(|| {
            let p = add(e, d, t);
            [p[0], p[1]]
        })
    }

    /// A card picked (rule 1). `uv`: the cursor's place when it was over the world.
    pub fn card(&mut self, uv: Option<[f32; 2]>) {
        self.armed = Some(MArmed::Place);
        self.lift = 0.0;
        self.yaw = self.next_yaw.unwrap_or(self.me[3]);
        self.at = None;
        if let Some([u, v]) = uv {
            self.hover(u, v);
        }
    }

    /// The cursor moved (rule 2, and 4 for a drag).
    pub fn hover(&mut self, u: f32, v: f32) {
        match self.armed {
            None => {}
            Some(MArmed::Place) => {
                let hit = self.project(u, v);
                let hit = match hit {
                    None if self.at.is_none() => Some([self.me[0] + FIRST_AHEAD * self.me[3].cos(), self.me[1] + FIRST_AHEAD * self.me[3].sin()]),
                    h => h,
                };
                if let Some([x, y]) = hit {
                    self.put(x, y);
                }
            }
            Some(MArmed::Move { carry, z, off, .. }) => {
                let Some((e, d)) = self.ray(u, v) else { return };
                let on = if carry { self.ground_along(e, d).map(|p| [p[0] + off[0], p[1] + off[1]]) } else { None };
                if let Some([x, y]) = on.or_else(|| self.plane(e, d, z).map(|p| [p[0] + off[0], p[1] + off[1]])) {
                    self.put(x, y);
                }
            }
        }
    }

    /// A still click with `carry` (rule 3), or a press that starts a drag (`carry` false, rule 4). True: picked up.
    pub fn grab(&mut self, u: f32, v: f32, carry: bool) -> bool {
        let Some((e, d)) = self.ray(u, v) else { return false };
        let reach = self.cam_now().dist + REACH;
        let Some((id, t)) = pick(e, d, &self.props, reach) else {
            if carry {
                self.armed = None;
                self.at = None;
            }
            return false;
        };
        let Some(p) = self.props.iter().find(|p| p.id == id).copied() else { return false };
        let hit = add(e, d, t.max(0.0));
        let mut off = [p.at[0] - hit[0], p.at[1] - hit[1]];
        if carry {
            off = match self.ground_along(e, d) {
                Some(g) => {
                    let raw = [p.at[0] - g[0], p.at[1] - g[1]];
                    let r = p.ext[0].max(p.ext[1]).max(0.5);
                    let n = (raw[0] * raw[0] + raw[1] * raw[1]).sqrt();
                    if n > r { [raw[0] * r / n, raw[1] * r / n] } else { raw }
                }
                None => [0.0, 0.0],
            };
        }
        self.armed = Some(MArmed::Move { id, carry, z: hit[2], off });
        self.lift = p.at[2] - self.floor;
        self.yaw = p.o;
        self.at = Some([p.at[0], p.at[1], p.at[2], p.o]);
        true
    }

    /// Q / E, the wheel (rule 5).
    pub fn turn(&mut self, deg: f32) {
        self.yaw = (self.yaw + deg.to_radians()).rem_euclid(std::f32::consts::TAU);
        if let Some(a) = self.at.as_mut() {
            a[3] = self.yaw;
        }
    }

    /// The click that puts it down (rule 3, 6). The new prop's id is `new_id` for a place.
    pub fn commit(&mut self, new_id: u64, body: impl Fn([f32; 4]) -> MProp) -> Option<u64> {
        let (armed, at) = (self.armed?, self.at?);
        if !self.inside(at[0], at[1]) {
            return None;
        }
        let id = match armed {
            MArmed::Place => {
                self.next_yaw = Some(at[3]);
                let mut p = body(at);
                p.id = new_id;
                self.props.push(p);
                new_id
            }
            MArmed::Move { id, .. } => {
                let mut p = body(at);
                p.id = id;
                if let Some(q) = self.props.iter_mut().find(|q| q.id == id) {
                    *q = p;
                }
                id
            }
        };
        self.armed = None;
        self.at = None;
        Some(id)
    }

    /// Esc: nothing in hand; a moved prop stays where it was.
    pub fn cancel(&mut self) {
        self.armed = None;
        self.at = None;
    }
}

// ------------------------------------------------------------------------------------------------------------------
// the input script
// ------------------------------------------------------------------------------------------------------------------

/// One line of a `.moves` script.
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    Cam(Cam),
    /// The player moved: x, y (relative to the Den's middle), facing (degrees).
    At(f32, f32, f32),
    Grid(f32),
    Zoom(f32),
    Aspect(f32),
    /// A prop already there: a catalog word, x, y (relative), facing (degrees).
    Prop(String, f32, f32, f32),
    Mode(bool),
    Card(String, Option<[f32; 2]>),
    Hover(f32, f32),
    Click(f32, f32, bool),
    Drag(f32, f32),
    Turn(f32),
    Commit,
    Esc,
    Undo,
    Wait(u64),
    /// Where the copy in hand must be: x, y, z (relative), facing (degrees, optional), within the tolerance.
    ExpectGhost([f32; 3], Option<f32>, f32),
    /// The n-th prop (1-based, in the order placed) or the last one.
    ExpectProp(Option<usize>, [f32; 3], Option<f32>, f32),
    ExpectProps(usize),
    ExpectNothing,
}

pub const MAX_SCRIPT: usize = 64 * 1024;
pub const MAX_LINES: usize = 4000;

fn f(w: &[&str], i: usize) -> Result<f32, String> {
    w.get(i).and_then(|s| s.parse::<f32>().ok()).filter(|v| v.is_finite()).ok_or_else(|| format!("a number at word {}", i + 1))
}

/// Reads a script: one command a line, `#` comments. Every problem names its line.
pub fn parse_script(text: &str) -> Result<Vec<(usize, Cmd)>, String> {
    if text.len() > MAX_SCRIPT {
        return Err("a script is at most 64 KB".into());
    }
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if out.len() >= MAX_LINES {
            return Err(format!("line {n}: at most {MAX_LINES} commands"));
        }
        let w: Vec<&str> = line.split_whitespace().collect();
        let e = |m: String| format!("line {n}: {m}");
        let tol = |i: usize| w.get(i).map(|_| f(&w, i)).transpose().map(|t| t.unwrap_or(0.05));
        let cmd = match w[0].to_ascii_lowercase().as_str() {
            "cam" => {
                let mut c = Cam::default();
                let mut k = 1;
                while k + 1 < w.len() + 1 && k < w.len() {
                    let v = f(&w, k + 1).map_err(e)?;
                    match w[k] {
                        "fov" => c.fov = v,
                        "pivot" => c.pivot = v,
                        "dist" => c.dist = v,
                        "pitch" => c.pitch = v,
                        o => return Err(e(format!("cam: fov, pivot, dist or pitch, not {o}"))),
                    }
                    k += 2;
                }
                Cmd::Cam(c)
            }
            "at" => Cmd::At(f(&w, 1).map_err(e)?, f(&w, 2).map_err(e)?, if w.get(3) == Some(&"face") { f(&w, 4).map_err(e)? } else { 0.0 }),
            "grid" => Cmd::Grid(f(&w, 1).map_err(e)?.clamp(0.0, 10.0)),
            "zoom" => Cmd::Zoom(f(&w, 1).map_err(e)?.clamp(0.0, 60.0)),
            "aspect" => Cmd::Aspect(f(&w, 1).map_err(e)?.clamp(0.2, 5.0)),
            "prop" => Cmd::Prop(w.get(1).ok_or_else(|| e("prop <word> <x> <y> [face <deg>]".into()))?.to_string(), f(&w, 2).map_err(e)?, f(&w, 3).map_err(e)?, if w.get(4) == Some(&"face") { f(&w, 5).map_err(e)? } else { 0.0 }),
            "mode" => Cmd::Mode(w.get(1) == Some(&"build")),
            "card" => {
                let word = w.get(1).ok_or_else(|| e("card <word> [u v]".into()))?.to_string();
                let uv = if w.len() >= 4 { Some([f(&w, 2).map_err(e)?, f(&w, 3).map_err(e)?]) } else { None };
                Cmd::Card(word, uv)
            }
            "hover" => Cmd::Hover(f(&w, 1).map_err(e)?, f(&w, 2).map_err(e)?),
            "click" => Cmd::Click(f(&w, 1).map_err(e)?, f(&w, 2).map_err(e)?, w.get(3).is_some_and(|x| x.eq_ignore_ascii_case("carry"))),
            "drag" => Cmd::Drag(f(&w, 1).map_err(e)?, f(&w, 2).map_err(e)?),
            "turn" => Cmd::Turn(f(&w, 1).map_err(e)?.clamp(-360.0, 360.0)),
            "commit" => Cmd::Commit,
            "esc" => Cmd::Esc,
            "undo" => Cmd::Undo,
            "wait" => Cmd::Wait(f(&w, 1).map_err(e)?.clamp(0.0, 600_000.0) as u64),
            "expect" => match w.get(1).copied() {
                Some("ghost") => {
                    let p = [f(&w, 2).map_err(e)?, f(&w, 3).map_err(e)?, f(&w, 4).map_err(e)?];
                    Cmd::ExpectGhost(p, w.get(5).map(|_| f(&w, 5)).transpose().map_err(e)?, tol(6).map_err(e)?)
                }
                Some("prop") => {
                    let which = match w.get(2).copied() {
                        Some("last") => None,
                        Some(k) => Some(k.parse::<usize>().ok().filter(|k| *k >= 1).ok_or_else(|| e("expect prop <n|last> ...".into()))?),
                        None => return Err(e("expect prop <n|last> x y z [deg] [tol]".into())),
                    };
                    let p = [f(&w, 3).map_err(e)?, f(&w, 4).map_err(e)?, f(&w, 5).map_err(e)?];
                    Cmd::ExpectProp(which, p, w.get(6).map(|_| f(&w, 6)).transpose().map_err(e)?, tol(7).map_err(e)?)
                }
                Some("props") => Cmd::ExpectProps(f(&w, 2).map_err(e)? as usize),
                Some("nothing") => Cmd::ExpectNothing,
                _ => return Err(e("expect ghost|prop|props|nothing".into())),
            },
            o => return Err(e(format!("unknown command '{o}'"))),
        };
        out.push((n, cmd));
    }
    Ok(out)
}

/// A random script that a player could have made (cards, hovers, carries, drags, turns, commits, Esc, undo, a grid,
/// walking), from `seed`: the same seed, the same script. `words`: the catalog words to place.
pub fn fuzz_script(seed: u64, steps: usize, words: &[&str]) -> String {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut rnd = move |n: u64| {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s % n.max(1)
    };
    let uv = |r: &mut dyn FnMut(u64) -> u64| (r(2001) as f32 / 1000.0 - 1.0, r(1601) as f32 / 1000.0 - 1.0);
    let mut out = String::from("mode build\n");
    let mut armed = false;
    let mut dragging = false;
    for _ in 0..steps {
        let k = rnd(100);
        let line = if !armed {
            match k {
                0..=29 => {
                    armed = true;
                    let w = words[rnd(words.len() as u64) as usize];
                    let (u, v) = uv(&mut rnd);
                    if rnd(2) == 0 { format!("card {w} {u:.3} {v:.3}") } else { format!("card {w}") }
                }
                30..=49 => {
                    let (u, v) = uv(&mut rnd);
                    armed = true;
                    dragging = false;
                    format!("click {u:.3} {v:.3} carry")
                }
                50..=59 => {
                    let (u, v) = uv(&mut rnd);
                    armed = true;
                    dragging = true;
                    format!("drag {u:.3} {v:.3}")
                }
                60..=64 => format!("grid {}", [0.0, 0.5, 1.0, 2.0][rnd(4) as usize]),
                65..=69 => format!("at {} {} face {}", rnd(41) as i32 - 20, rnd(41) as i32 - 20, rnd(360)),
                70..=74 => "undo".into(),
                75..=79 => format!("zoom {}", 4 + rnd(30)),
                _ => {
                    let (u, v) = uv(&mut rnd);
                    format!("hover {u:.3} {v:.3}")
                }
            }
        } else {
            match k {
                0..=59 => {
                    let (u, v) = uv(&mut rnd);
                    format!("hover {u:.3} {v:.3}")
                }
                60..=69 => format!("turn {}", [15, -15, 45, 90, 5][rnd(5) as usize]),
                70..=89 => {
                    armed = false;
                    "commit".into()
                }
                _ => {
                    armed = false;
                    "esc".into()
                }
            }
        };
        let _ = dragging;
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32, e: f32) -> bool {
        (a - b).abs() <= e
    }

    #[test]
    fn the_middle_of_the_screen_looks_at_the_pivot_and_the_sides_mirror() {
        let cam = Cam::default();
        let me = [100.0, 50.0, 10.0, 0.0];
        let mut m = Model::new([100.0, 50.0], 10.0, 30.0);
        m.me = me;
        let p = m.project(0.0, 0.0).unwrap();
        assert!(near(p[0], 100.0 + cam.pivot / cam.pitch.tan(), 0.01) && near(p[1], 50.0, 0.01), "{p:?}");
        let r = m.project(0.5, 0.0).unwrap();
        let l = m.project(-0.5, 0.0).unwrap();
        assert!(r[1] < 50.0 && l[1] > 50.0 && near(50.0 - r[1], l[1] - 50.0, 0.01));
        assert!(m.project(0.0, 1.0).is_none(), "the top of the screen is sky");
        m.me[3] = std::f32::consts::FRAC_PI_2;
        let q = m.project(0.0, 0.0).unwrap();
        assert!(near(q[0], 100.0, 0.01) && near(q[1], 50.0 + cam.pivot / cam.pitch.tan(), 0.01));
    }

    #[test]
    fn a_card_glides_snaps_stays_inside_and_turns() {
        let mut m = Model::new([0.0, 0.0], 0.0, 22.0);
        m.card(None);
        assert_eq!(m.at, None, "nothing until the cursor is over the ground");
        m.hover(0.0, 1.0);
        let a = m.at.unwrap();
        assert!(near(a[0], FIRST_AHEAD, 1e-4) && near(a[1], 0.0, 1e-4), "the sky: 5 yards ahead {a:?}");
        m.hover(0.1, -0.2);
        let b = m.at.unwrap();
        m.grid = 1.0;
        m.hover(0.1, -0.2);
        let c = m.at.unwrap();
        assert!(near(c[0], b[0].round(), 1e-4) && near(c[1], b[1].round(), 1e-4), "on the grid");
        // far past the edge: held a grid step inside
        m.me = [0.0, 0.0, 0.0, 0.0];
        m.zoom = Some(40.0);
        m.hover(0.0, 0.3);
        let e = m.at.unwrap();
        assert!((e[0] * e[0] + e[1] * e[1]).sqrt() <= 22.0, "{e:?}");
        m.turn(90.0);
        assert!(near(m.at.unwrap()[3], std::f32::consts::FRAC_PI_2, 1e-5));
        let id = m.commit(7, |at| MProp { id: 0, at: [at[0], at[1], at[2]], o: at[3], half: [0.5, 0.5], bottom: 0.0, height: 1.0, ext: [0.5, 0.5] });
        assert_eq!(id, Some(7));
        assert_eq!(m.next_yaw, Some(std::f32::consts::FRAC_PI_2));
        m.card(None);
        assert!(near(m.yaw, std::f32::consts::FRAC_PI_2, 1e-6), "the next card starts turned as the last");
    }

    #[test]
    fn a_click_picks_up_without_a_jump_and_drops_on_a_table() {
        let mut m = Model::new([0.0, 0.0], 0.0, 30.0);
        let table = MProp { id: 1, at: [6.0, 0.0, 0.0], o: 0.0, half: [1.05, 1.05], bottom: 0.0, height: 1.0, ext: [1.0, 1.0] };
        // its footprint wide enough that the click's offset is kept whole (a tall prop's is held within it: the rule's limit)
        let chair = MProp { id: 2, at: [6.0, 4.0, 0.0], o: 0.3, half: [0.4, 0.4], bottom: 0.0, height: 1.2, ext: [3.0, 3.0] };
        m.props = vec![table, chair];
        // find a screen point on the chair
        let mut on = None;
        'f: for i in 0..200 {
            for j in 0..200 {
                let (u, v) = (i as f32 / 100.0 - 1.0, j as f32 / 100.0 - 1.0);
                if let Some((e, d)) = m.ray(u, v) {
                    if pick(e, d, &m.props, 72.0).map(|p| p.0) == Some(2) {
                        on = Some((u, v));
                        break 'f;
                    }
                }
            }
        }
        let (u, v) = on.expect("the chair is on screen");
        assert!(m.grab(u, v, true));
        let a = m.at.unwrap();
        assert_eq!([a[0], a[1], a[2]], chair.at, "no jump at the pickup");
        m.hover(u, v);
        let b = m.at.unwrap();
        assert!(near(b[0], 6.0, 0.01) && near(b[1], 4.0, 0.01), "the same cursor, the same spot {b:?}");
        // carry it over the table: it stands on the table's top
        let mut over = None;
        for i in 0..400 {
            let (u2, v2) = (u, v - i as f32 / 400.0);
            m.hover(u2, v2);
            let at = m.at.unwrap();
            if (at[0] - 6.0).abs() < 0.6 && at[1].abs() < 0.6 {
                over = Some(at);
                break;
            }
        }
        if let Some(at) = over {
            assert!(near(at[2], 1.0, 1e-4), "on the table {at:?}");
        }
        m.cancel();
        assert_eq!(m.props[1].at, chair.at, "Esc: back where it was");
        // a click on empty sky picks nothing
        assert!(!m.grab(0.0, 1.0, true));
        assert_eq!(m.armed, None);
    }

    #[test]
    fn scripts_read_and_problems_name_their_line() {
        let s = parse_script("# a test\ncam fov 1.4 pitch 0.6\nat 1 2 face 90\nprop chair 3 0\nmode build\ncard chair 0.1 -0.2\nhover 0 0\nturn 15\ncommit\nexpect prop last 1 2 0 15 0.1\nexpect props 2\nexpect nothing\n").unwrap();
        assert_eq!(s.len(), 11);
        assert_eq!(s[0].1, Cmd::Cam(Cam { fov: 1.4, pitch: 0.6, ..Cam::default() }));
        assert_eq!(s[4].1, Cmd::Card("chair".into(), Some([0.1, -0.2])));
        assert_eq!(s[8].1, Cmd::ExpectProp(None, [1.0, 2.0, 0.0], Some(15.0), 0.1));
        for (bad, want) in [("hover x 0", "line 1"), ("fly 1", "unknown"), ("expect prop 0 1 1 1", "n|last"), ("cam zoom 2", "cam:")] {
            assert!(parse_script(bad).unwrap_err().contains(want), "{bad}");
        }
        let f = fuzz_script(42, 300, &["chair", "lantern"]);
        assert_eq!(f, fuzz_script(42, 300, &["chair", "lantern"]), "the same seed, the same script");
        assert_ne!(f, fuzz_script(43, 300, &["chair", "lantern"]));
        assert!(parse_script(&f).is_ok());
    }

    /// The model's own invariants on random scripts: never anything off the Den, never a number that isn't.
    #[test]
    fn fuzzed_scripts_keep_the_copy_inside_and_finite() {
        for seed in 0..60u64 {
            let mut m = Model::new([0.0, 0.0], 0.0, 22.0);
            m.props.push(MProp { id: 1, at: [5.0, 0.0, 0.0], o: 0.0, half: [1.0, 1.0], bottom: 0.0, height: 1.0, ext: [1.0, 1.0] });
            let mut next = 10;
            for (_, c) in parse_script(&fuzz_script(seed, 200, &["chair"])).unwrap() {
                match c {
                    Cmd::Card(_, uv) => m.card(uv),
                    Cmd::Hover(u, v) => m.hover(u, v),
                    Cmd::Click(u, v, carry) => {
                        m.grab(u, v, carry);
                    }
                    Cmd::Drag(u, v) => {
                        m.grab(u, v, false);
                    }
                    Cmd::Turn(d) => m.turn(d),
                    Cmd::Commit => {
                        next += 1;
                        m.commit(next, |at| MProp { id: 0, at: [at[0], at[1], at[2]], o: at[3], half: [0.5, 0.5], bottom: 0.0, height: 1.0, ext: [0.5, 0.5] });
                    }
                    Cmd::Esc => m.cancel(),
                    Cmd::Grid(g) => m.grid = g,
                    Cmd::Zoom(z) => m.zoom = Some(z),
                    Cmd::At(x, y, o) => m.me = [x, y, 0.0, o.to_radians()],
                    _ => {}
                }
                if let Some(a) = m.at {
                    assert!(a.iter().all(|v| v.is_finite()), "seed {seed}: {a:?}");
                }
                for p in &m.props {
                    assert!((p.at[0].powi(2) + p.at[1].powi(2)).sqrt() <= 22.0 + 1e-3, "seed {seed}: a prop off the Den {p:?}");
                }
            }
        }
    }
}
