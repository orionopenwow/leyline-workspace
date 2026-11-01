//! The Den: its room, its props, undo, and the `leyline-den` save file (docs/DEN.md, "The save file").
//!
//! Positions in a file are relative: `x`, `y` yards from the Den's middle, `z` from the room's floor, angles in
//! degrees. The same files open in the full Den (`ov_den.lua` `.den load`, checked there by `OVBuild.DenCheck`), and
//! files from there open here. Nothing in a file is ever run: it is JSON, checked field by field.

use crate::objects::OBJECTS;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::f32::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

pub const FORMAT: &str = "leyline-den";
pub const VERSION: u64 = 1;
/// ov_den.lua FILE_MAX, ov_build.lua K.LAYOUT_MAX and K.DEN_PATHS (the launcher's `host/dens.rs` uses the same).
pub const MAX_BYTES: usize = 512 * 1024;
pub const MAX_PROPS: usize = 800;
pub const MAX_PATHS: usize = 100;
/// ov_den_home.lua H.STAMP_MAX and H.STAMP_SPAN.
pub const STAMP_MAX: usize = 60;
pub const STAMP_SPAN: f64 = 40.0;
/// ov_build.lua K.UNDO_DEPTH.
pub const UNDO_DEPTH: usize = 50;
/// A model prop's entry is this plus its display (ov_build.lua `.build models`); it is shown on the carrier.
pub const MODEL_BASE: u64 = 2_000_000_000;
/// OV_BUILD_MODEL_CARRIER: the catalog's Bottle.
pub const MODEL_CARRIER: u32 = 2687;
/// HighGuid::GameObject in a 3.3.5a guid (bits 48..63).
const HIGH_GAMEOBJECT: u64 = 0xF110;

/// Where the Den is: AzerothCore's `game_tele` 424 "GMIsland" on map 1 (ov_build.lua DEN, OV_DEN_X/_Y/_Z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub map: u32,
    pub x: f32,
    pub y: f32,
    /// The room's floor (ov_den.lua measures it with Map:GetHeight; Den Lite has no map files, so it is OV_DEN_Z).
    pub z: f32,
    /// The room's half width (OV_DEN_SIZE; the den profile sets 16: a 32 x 32 yard floor).
    pub half: f32,
}

impl Default for Spot {
    fn default() -> Self {
        Spot { map: 1, x: 16226.2, y: 16257.0, z: 13.2, half: 16.0 }
    }
}

impl Spot {
    /// How far props may lie from the middle (ov_build.lua DEN.r = half + 14, the yard included).
    pub fn reach(&self) -> f32 {
        self.half + 14.0
    }
    pub fn inside(&self, x: f32, y: f32, z: f32) -> bool {
        let (dx, dy) = (x - self.x, y - self.y);
        (dx * dx + dy * dy).sqrt() <= self.reach() && z >= self.z - 12.0 && z <= self.z + 50.0
    }
}

/// A gameobject entry Den Lite can show (src/objects.rs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Template {
    pub entry: u32,
    pub kind: u8,
    pub display: u32,
    pub name: &'static str,
    pub size: f32,
}

pub fn template(entry: u32) -> Option<Template> {
    OBJECTS
        .binary_search_by_key(&entry, |r| r.0)
        .ok()
        .map(|i| {
            let r = OBJECTS[i];
            Template { entry: r.0, kind: r.1, display: r.2, name: r.3, size: r.4 }
        })
}

/// The room's looks (ov_den.lua THEMES): floor, wall and light entries, tile spacing, wall rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub name: &'static str,
    pub floor: u32,
    pub wall: u32,
    pub light: u32,
    pub tile: f32,
    pub rows: u32,
}

pub const LOOKS: &[Look] = &[
    Look { name: "white", floor: 188142, wall: 188142, light: 190664, tile: 4.0, rows: 2 },
    Look { name: "stone", floor: 190024, wall: 19444, light: 180043, tile: 4.0, rows: 1 },
    Look { name: "wood", floor: 190615, wall: 211062, light: 179977, tile: 4.0, rows: 1 },
];

pub fn look(name: &str) -> Option<Look> {
    LOOKS.iter().copied().find(|l| l.name.eq_ignore_ascii_case(name))
}

/// One object in the world, in world coordinates; `o` (yaw), `pitch`, `roll` in radians.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldObject {
    pub guid: u64,
    /// The entry the client is told (the carrier for a model prop).
    pub entry: u32,
    pub display: u32,
    pub kind: u8,
    pub scale: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
    pub pitch: f32,
    pub roll: f32,
}

/// The room's pieces (ov_den.lua `plan`, without the model bounds: blocks stand at their own size on the floor).
/// The south wall keeps a doorway in its middle.
pub fn room_plan(spot: &Spot, l: &Look) -> Vec<(u32, f32, f32, f32, f32)> {
    let tile = l.tile.clamp(1.0, 10.0);
    let half = spot.half;
    let n = ((half * 2.0 / tile + 0.5).floor() as i32).max(1);
    let offs: Vec<f32> = (1..=n).map(|i| -half + tile * (i as f32 - 0.5)).collect();
    let fz = spot.z;
    let mut out = Vec::new();
    for &ox in &offs {
        for &oy in &offs {
            out.push((l.floor, spot.x + ox, spot.y + oy, fz, 0.0));
        }
    }
    for r in 0..l.rows {
        let z = fz + tile * (r as f32 + 0.5);
        for &o in &offs {
            out.push((l.wall, spot.x + o, spot.y + half, z, 0.0));
            if o.abs() > tile {
                out.push((l.wall, spot.x + o, spot.y - half, z, 0.0));
            }
            out.push((l.wall, spot.x + half, spot.y + o, z, PI / 2.0));
            out.push((l.wall, spot.x - half, spot.y + o, z, PI / 2.0));
        }
    }
    let c = half - 2.0;
    for s in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        out.push((l.light, spot.x + s.0 * c, spot.y + s.1 * c, fz, 0.0));
    }
    out
}

// ------------------------------------------------------------------------------------------------------------------
// the file
// ------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Prop {
    pub entry: u64,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub yaw: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roll: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<u32>,
    /// Stamps only: "door", "mailbox", "house".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Room {
    #[serde(default = "white")]
    pub look: String,
    #[serde(default = "sixteen")]
    pub size: f64,
}

fn white() -> String {
    "white".into()
}
fn sixteen() -> f64 {
    16.0
}

/// A `leyline-den` file. Fields Den Lite doesn't use (paths, notes, door, mailbox, a stamp's title) are kept as they
/// were, so a file opened here and saved again loses nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DenDoc {
    pub format: String,
    pub version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<Room>,
    pub props: Vec<Prop>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A Den file's name: letters, digits, - and _, 1 to 24 of them, lower case (the launcher's `host/dens.rs`
/// `clean_name`). Anything else is refused, not trimmed.
pub fn clean_name(s: &str) -> Option<String> {
    let s = s.trim().to_lowercase();
    let s = s.strip_suffix(".den.json").or_else(|| s.strip_suffix(".json")).unwrap_or(&s).to_string();
    if s.is_empty() || s.len() > 24 || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return None;
    }
    Some(s)
}

fn finite(v: f64, lo: f64, hi: f64) -> bool {
    v.is_finite() && v >= lo && v <= hi
}

/// Reads and checks a Den file's text (the same rules as `OVBuild.DenCheck` and the launcher's check).
pub fn parse(text: &str) -> Result<DenDoc, String> {
    if text.len() > MAX_BYTES {
        return Err("That Den file is too big (512 KB at most).".into());
    }
    let doc: DenDoc = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| "That isn't a Den file (it can't be read).".to_string())?;
    if doc.format != FORMAT {
        return Err("That isn't a Den file.".into());
    }
    if doc.version < 1 {
        return Err("That Den file has no version.".into());
    }
    if doc.version > VERSION {
        return Err("That Den file is from a newer Leyline: update Den Lite first.".into());
    }
    if doc.props.len() > MAX_PROPS {
        return Err(format!("A Den file holds up to {MAX_PROPS} props."));
    }
    if doc.paths.len() > MAX_PATHS {
        return Err("Too many paths in that file.".into());
    }
    for p in &doc.props {
        if !(1..=u32::MAX as u64).contains(&p.entry) {
            return Err("A prop in that file has no entry.".into());
        }
        if !(finite(p.x, -1000.0, 1000.0) && finite(p.y, -1000.0, 1000.0) && finite(p.z, -1000.0, 1000.0)) {
            return Err("A prop in that file has no place.".into());
        }
        for a in [Some(p.yaw), p.pitch, p.roll].into_iter().flatten() {
            if !finite(a, -100000.0, 100000.0) {
                return Err("A prop in that file has a bad angle.".into());
            }
        }
        if let Some(s) = p.scale {
            if !finite(s, 0.0, 100.0) {
                return Err("A prop in that file has a bad size.".into());
            }
        }
    }
    Ok(doc)
}

pub fn read_file(path: &Path) -> Result<DenDoc, String> {
    let meta = fs::metadata(path).map_err(|_| "There's no such Den file.".to_string())?;
    if meta.len() as usize > MAX_BYTES {
        return Err("That Den file is too big (512 KB at most).".into());
    }
    let text = fs::read_to_string(path).map_err(|_| "That Den file can't be read.".to_string())?;
    parse(&text)
}

/// A stamp (den-stamps/<name>.den.json): up to 60 props within 40 yards, x ahead of you, y to your left.
/// The stamps built into Den Lite (realm/custom/lua_scripts/den-stamps at build time), used when the stamps folder
/// hasn't that one: the launcher starts Den Lite without a stamps folder.
pub const BUILTIN_STAMPS: &[(&str, &str)] = &[
    ("campfire", include_str!("../../../realm/custom/lua_scripts/den-stamps/campfire.den.json")),
    ("dock", include_str!("../../../realm/custom/lua_scripts/den-stamps/dock.den.json")),
    ("garden", include_str!("../../../realm/custom/lua_scripts/den-stamps/garden.den.json")),
    ("house", include_str!("../../../realm/custom/lua_scripts/den-stamps/house.den.json")),
    ("inn", include_str!("../../../realm/custom/lua_scripts/den-stamps/inn.den.json")),
    ("market", include_str!("../../../realm/custom/lua_scripts/den-stamps/market.den.json")),
];

/// The stamps there are: the folder's and the built-in ones.
pub fn stamp_names(dir: &Path) -> Vec<String> {
    let mut v = list_names(dir);
    v.extend(BUILTIN_STAMPS.iter().map(|s| s.0.to_string()));
    v.sort();
    v.dedup();
    v
}

pub fn read_stamp(dir: &Path, name: &str) -> Result<DenDoc, String> {
    let name = clean_name(name).ok_or("No such stamp.")?;
    let path = dir.join(format!("{name}.den.json"));
    let doc = if path.is_file() {
        read_file(&path)
    } else {
        BUILTIN_STAMPS.iter().find(|s| s.0 == name).map(|s| parse(s.1)).unwrap_or(Err(String::new()))
    }
    .map_err(|_| format!("No stamp called '{name}'."))?;
    if doc.props.is_empty() || doc.props.len() > STAMP_MAX {
        return Err(format!("The stamp '{name}' isn't right."));
    }
    if doc.props.iter().any(|p| p.x.abs() > STAMP_SPAN || p.y.abs() > STAMP_SPAN || p.z.abs() > STAMP_SPAN) {
        return Err(format!("The stamp '{name}' isn't right."));
    }
    Ok(doc)
}

/// The launcher's request in `<dens>/open.json`: `{"file": "<name>", "id": "<letters>"}` (host/dens.rs `open_doc`),
/// checked: a Den file name and an id of letters, digits, - and _ (up to 64).
pub fn open_request(dir: &Path) -> Option<(String, String)> {
    let p = dir.join("open.json");
    if fs::metadata(&p).ok()?.len() > 4096 {
        return None;
    }
    let v: Value = serde_json::from_str(fs::read_to_string(&p).ok()?.trim_start_matches('\u{feff}')).ok()?;
    let file = clean_name(v.get("file")?.as_str()?)?;
    let id = v.get("id")?.as_str()?.to_string();
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return None;
    }
    Some((file, id))
}

pub fn list_names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".den.json")).and_then(clean_name))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// "2026-11-01T12:00:00Z" for a unix time.
pub fn iso_utc(unix: u64) -> String {
    let (y, mo, d, h, mi, s, _) = civil(unix);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// (year, month 1-12, day 1-31, hour, minute, second, weekday 0 = Sunday) for a unix time, UTC.
pub fn civil(unix: u64) -> (i64, u32, u32, u32, u32, u32, u32) {
    let days = (unix / 86400) as i64;
    let secs = unix % 86400;
    let weekday = ((days + 4).rem_euclid(7)) as u32;
    // Howard Hinnant's civil_from_days
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d, (secs / 3600) as u32, ((secs % 3600) / 60) as u32, (secs % 60) as u32, weekday)
}

// ------------------------------------------------------------------------------------------------------------------
// the Den in memory
// ------------------------------------------------------------------------------------------------------------------

/// A placed prop: what the file says (relative) and the object the client sees.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub prop: Prop,
    pub obj: WorldObject,
}

#[derive(Debug, Clone, PartialEq)]
enum Undo {
    /// These props were placed (a stamp, a load in add mode): undo takes them away.
    Placed(Vec<u64>),
    /// A load that replaced the Den: undo takes the new props away and puts the old ones back.
    Replaced { added: Vec<u64>, removed: Vec<Prop>, look: String },
}

/// What a change did, for the world server to tell the client: objects to create and guids to destroy.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Change {
    pub created: Vec<WorldObject>,
    pub destroyed: Vec<u64>,
    pub skipped: usize,
    pub message: String,
}

#[derive(Debug)]
pub struct Den {
    pub spot: Spot,
    pub look: Look,
    pub room: Vec<WorldObject>,
    pub props: Vec<Placed>,
    pub paths: Vec<Value>,
    pub extra: Map<String, Value>,
    next: u32,
    undo: Vec<Undo>,
}

fn quiet_round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

impl Den {
    pub fn new(spot: Spot) -> Self {
        let mut den = Den { spot, look: LOOKS[0], room: Vec::new(), props: Vec::new(), paths: Vec::new(), extra: Map::new(), next: 1, undo: Vec::new() };
        den.build_room();
        den
    }

    fn guid(&mut self, entry: u32) -> u64 {
        let c = self.next;
        self.next += 1;
        (HIGH_GAMEOBJECT << 48) | ((entry as u64 & 0xFF_FFFF) << 24) | c as u64
    }

    /// Puts the room up in the current look; returns the guids of the old room (to destroy).
    pub fn build_room(&mut self) -> Vec<u64> {
        let old: Vec<u64> = self.room.iter().map(|o| o.guid).collect();
        let plan = room_plan(&self.spot, &self.look);
        let mut room = Vec::with_capacity(plan.len());
        for (entry, x, y, z, o) in plan {
            if let Some(t) = template(entry) {
                let guid = self.guid(entry);
                room.push(WorldObject { guid, entry, display: t.display, kind: t.kind, scale: t.size, x, y, z, o, pitch: 0.0, roll: 0.0 });
            }
        }
        self.room = room;
        old
    }

    /// The object for a relative prop, or None when this entry isn't known here or it falls outside the Den.
    fn object_for(&mut self, p: &Prop) -> Option<WorldObject> {
        let (entry, display, kind, size) = if p.entry >= MODEL_BASE {
            let display = p.model.map(|m| m as u64).unwrap_or(p.entry - MODEL_BASE);
            let t = template(MODEL_CARRIER)?;
            (MODEL_CARRIER, u32::try_from(display).ok()?, t.kind, 1.0)
        } else {
            let t = template(u32::try_from(p.entry).ok()?)?;
            (t.entry, t.display, t.kind, t.size)
        };
        let (x, y, z) = (self.spot.x + p.x as f32, self.spot.y + p.y as f32, self.spot.z + p.z as f32);
        if !self.spot.inside(x, y, z) {
            return None;
        }
        let guid = self.guid(entry);
        Some(WorldObject {
            guid,
            entry,
            display,
            kind,
            scale: p.scale.map(|s| s.clamp(0.1, 5.0) as f32).unwrap_or(size),
            x,
            y,
            z,
            o: (p.yaw as f32).to_radians().rem_euclid(2.0 * PI),
            pitch: p.pitch.unwrap_or(0.0).to_radians() as f32,
            roll: p.roll.unwrap_or(0.0).to_radians() as f32,
        })
    }

    fn push_undo(&mut self, u: Undo) {
        self.undo.push(u);
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
    }

    fn max_group(&self) -> u64 {
        self.props.iter().filter_map(|p| p.prop.group).max().unwrap_or(0)
    }

    /// Places props given relative to the Den; returns the change (and pushes no undo).
    fn place(&mut self, props: &[Prop]) -> (Vec<WorldObject>, usize) {
        let mut made = Vec::new();
        let mut skipped = 0;
        for p in props {
            if self.props.len() >= MAX_PROPS {
                skipped += 1;
                continue;
            }
            match self.object_for(p) {
                Some(obj) => {
                    let mut prop = p.clone();
                    prop.role = None;
                    made.push(obj.clone());
                    self.props.push(Placed { prop, obj });
                }
                None => skipped += 1,
            }
        }
        (made, skipped)
    }

    /// `.den stamp <name> now`: the stamp's pieces turned by `o` (15 degree steps) around the player at (x, y, z).
    pub fn stamp(&mut self, st: &DenDoc, x: f32, y: f32, z: f32, o: f32) -> Change {
        let step = PI / 12.0;
        let o = ((o / step).round() * step).rem_euclid(2.0 * PI);
        let (c, s) = (o.cos() as f64, o.sin() as f64);
        let group = self.max_group() + 1;
        let props: Vec<Prop> = st
            .props
            .iter()
            .map(|e| {
                let (wx, wy) = (e.x * c - e.y * s, e.x * s + e.y * c);
                let mut p = e.clone();
                p.x = quiet_round((x - self.spot.x) as f64 + wx);
                p.y = quiet_round((y - self.spot.y) as f64 + wy);
                p.z = quiet_round((z - self.spot.z) as f64 + e.z);
                p.yaw = quiet_round((o.to_degrees() as f64 + e.yaw).rem_euclid(360.0));
                p.group = Some(group);
                p
            })
            .collect();
        let (made, skipped) = self.place(&props);
        let title = st.title.clone().or(st.name.clone()).unwrap_or_else(|| "The stamp".into());
        let message = if made.is_empty() {
            format!("{title} doesn't fit here: nothing was placed.")
        } else {
            self.push_undo(Undo::Placed(made.iter().map(|o| o.guid).collect()));
            let left = if skipped > 0 { format!(" ({skipped} left out: outside the Den or unknown here)") } else { String::new() };
            format!("{title} is down ({} props){left}. .build undo takes it back.", made.len())
        };
        Change { created: made, destroyed: Vec::new(), skipped, message }
    }

    /// `.build undo`: the last change taken back.
    pub fn undo(&mut self) -> Change {
        let Some(u) = self.undo.pop() else {
            return Change { message: "Nothing to undo.".into(), ..Default::default() };
        };
        match u {
            Undo::Placed(guids) => {
                self.props.retain(|p| !guids.contains(&p.obj.guid));
                let n = guids.len();
                Change { destroyed: guids, message: format!("Undone: {n} props taken away."), ..Default::default() }
            }
            Undo::Replaced { added, removed, look: old } => {
                self.props.retain(|p| !added.contains(&p.obj.guid));
                let mut destroyed = added;
                let mut created = Vec::new();
                if old != self.look.name {
                    if let Some(l) = look(&old) {
                        self.look = l;
                        destroyed.extend(self.build_room());
                        created.extend(self.room.clone());
                    }
                }
                let (made, _) = self.place(&removed);
                let n = made.len();
                created.extend(made);
                Change { created, destroyed, skipped: 0, message: format!("Undone: the Den is back as it was ({n} props).") }
            }
        }
    }

    /// The whole Den as a file (the room is its look; everyone's decor is in it).
    pub fn to_doc(&self, name: &str, unix: u64) -> DenDoc {
        DenDoc {
            format: FORMAT.into(),
            version: VERSION,
            name: Some(name.into()),
            saved: Some(iso_utc(unix)),
            room: Some(Room { look: self.look.name.into(), size: self.spot.half as f64 }),
            props: self.props.iter().map(|p| p.prop.clone()).collect(),
            paths: self.paths.clone(),
            extra: self.extra.clone(),
            ..Default::default()
        }
    }

    /// `.den save <name>`: writes `<dir>/<name>.den.json`.
    pub fn save(&self, dir: &Path, name: &str, unix: u64) -> Result<(PathBuf, usize), String> {
        let name = clean_name(name).ok_or("Den names are letters, digits, - and _ (up to 24).")?;
        fs::create_dir_all(dir).map_err(|_| "The Dens folder can't be made.".to_string())?;
        let path = dir.join(format!("{name}.den.json"));
        let text = serde_json::to_string_pretty(&self.to_doc(&name, unix)).map_err(|e| e.to_string())?;
        let tmp = dir.join(format!(".{name}.den.json.tmp"));
        fs::write(&tmp, text.as_bytes()).and_then(|_| fs::rename(&tmp, &path)).map_err(|_| "That Den file can't be written.".to_string())?;
        Ok((path, self.props.len()))
    }

    /// Opens a checked file: replace (the Den's decor goes; one undo brings it back) or add (next to it).
    pub fn open(&mut self, doc: &DenDoc, replace: bool) -> Change {
        let mut destroyed = Vec::new();
        let mut created = Vec::new();
        let old_look = self.look.name.to_string();
        let mut removed = Vec::new();
        if replace {
            for p in self.props.drain(..) {
                destroyed.push(p.obj.guid);
                removed.push(p.prop);
            }
            self.paths = doc.paths.clone();
            self.extra = doc.extra.clone();
            self.extra.retain(|k, _| k == "notes" || k == "door" || k == "mailbox");
            if let Some(l) = doc.room.as_ref().and_then(|r| look(&r.look)) {
                if l.name != self.look.name {
                    self.look = l;
                    destroyed.extend(self.build_room());
                    created.extend(self.room.clone());
                }
            }
        } else {
            self.paths.extend(doc.paths.iter().cloned());
        }
        let shift = if replace { 0 } else { self.max_group() };
        let props: Vec<Prop> = doc
            .props
            .iter()
            .map(|p| {
                let mut p = p.clone();
                p.group = p.group.map(|g| g + shift);
                p
            })
            .collect();
        let (made, skipped) = self.place(&props);
        let added: Vec<u64> = made.iter().map(|o| o.guid).collect();
        let n = made.len();
        created.extend(made);
        if replace {
            self.push_undo(Undo::Replaced { added, removed, look: old_look });
        } else if !added.is_empty() {
            self.push_undo(Undo::Placed(added));
        }
        let left = if skipped > 0 { format!(" {skipped} left out (outside the Den or unknown here).") } else { String::new() };
        Change { created, destroyed, skipped, message: format!("Opened: {n} props.{left}") }
    }

    /// Every object a client entering the Den is sent: the room, then the props.
    pub fn everything(&self) -> Vec<WorldObject> {
        self.room.iter().cloned().chain(self.props.iter().map(|p| p.obj.clone())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_room_has_a_floor_walls_a_doorway_and_lights() {
        let spot = Spot::default();
        let plan = room_plan(&spot, &LOOKS[0]);
        let floor = plan.iter().filter(|p| p.3 == spot.z && p.0 == 188142).count();
        let lights = plan.iter().filter(|p| p.0 == 190664).count();
        assert_eq!(floor, 64, "8 x 8 tiles of 4 yards");
        assert_eq!(lights, 4);
        assert_eq!(plan.len(), 64 + 2 * (8 + 6 + 8 + 8) + 4);
        let doorway = plan.iter().any(|p| p.0 == 188142 && p.3 > spot.z && (p.2 - (spot.y - 16.0)).abs() < 0.01 && (p.1 - spot.x).abs() < 4.0);
        assert!(!doorway, "the south wall's middle stays open");
    }

    #[test]
    fn the_looks_and_stamps_entries_are_known() {
        for l in LOOKS {
            assert!(template(l.floor).is_some() && template(l.wall).is_some() && template(l.light).is_some());
        }
        assert_eq!(template(188142).unwrap().display, 6752);
        assert_eq!(template(190664).unwrap().display, 7976);
        assert!(template(1).is_none());
    }

    #[test]
    fn names_are_words() {
        assert_eq!(clean_name("House"), Some("house".into()));
        assert_eq!(clean_name("my-den_2.den.json"), Some("my-den_2".into()));
        assert_eq!(clean_name("../x"), None);
        assert_eq!(clean_name(""), None);
        assert_eq!(clean_name(&"a".repeat(25)), None);
    }

    #[test]
    fn files_are_checked() {
        assert!(parse(r#"{"format":"leyline-den","version":1,"props":[]}"#).is_ok());
        assert!(parse(r#"{"format":"other","version":1,"props":[]}"#).is_err());
        assert!(parse(r#"{"format":"leyline-den","version":2,"props":[]}"#).unwrap_err().contains("newer"));
        assert!(parse(r#"{"format":"leyline-den","version":1,"props":[{"entry":0,"x":0,"y":0}]}"#).is_err());
        assert!(parse(r#"{"format":"leyline-den","version":1,"props":[{"entry":5,"x":5000,"y":0}]}"#).is_err());
        let many = format!(r#"{{"format":"leyline-den","version":1,"props":[{}]}}"#, vec![r#"{"entry":1,"x":0,"y":0}"#; 801].join(","));
        assert!(parse(&many).is_err());
    }

    #[test]
    fn the_doc_example_round_trips() {
        let text = r#"{
 "format": "leyline-den", "version": 1, "name": "house", "saved": "2026-10-31T22:50:37Z",
 "room": {"look": "white", "size": 16},
 "props": [
  {"entry": 19006, "x": 0, "y": 24, "z": 0, "yaw": 90},
  {"entry": 2413, "x": 3, "y": 3, "z": 0, "yaw": 90, "pitch": 10, "roll": 0, "scale": 1.5, "group": 1, "locked": true},
  {"entry": 2000004523, "model": 4523, "x": -3, "y": -4, "z": 0, "yaw": 0, "hidden": true}
 ],
 "paths": [{"group": 2, "entry": 190023, "every": 4, "from": [-10, 20, -0.2], "to": [10, 20, 0.2]}],
 "notes": [{"from": "Ann", "text": "hi"}]
}"#;
        let doc = parse(text).unwrap();
        let mut den = Den::new(Spot::default());
        let c = den.open(&doc, true);
        assert_eq!(c.created.len(), 3);
        let model = den.props.iter().find(|p| p.prop.entry == 2000004523).unwrap();
        assert_eq!((model.obj.entry, model.obj.display), (MODEL_CARRIER, 4523));
        let back = den.to_doc("house", 0);
        assert_eq!(back.props, doc.props);
        assert_eq!(back.paths, doc.paths);
        assert_eq!(back.extra.get("notes"), doc.extra.get("notes"));
    }

    #[test]
    fn civil_time() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_793_404_800), "2026-10-31T00:00:00Z");
        assert_eq!(civil(1_793_404_800).6, 6, "a Saturday");
    }
}
