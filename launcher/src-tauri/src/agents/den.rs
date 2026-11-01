//! Claude in your Den: the Den tools of `Leyline mcp` (agents.rs), owner 1 Nov 2026 (the Den plan, 3.5 and 7). Leyline
//! has no AI of its own: the player's own assistant reads their Den, builds with their approval, and talks through
//! the companion in game (realm/custom/lua_scripts/ov_den_companion.lua). Players read about it in
//! docs/leyline/agents.md; the formats are in docs/DEN.md ("Claude in your Den", "Building brain").
//!
//! * read: `den_status` (the Den realm, the room and its budgets, the saved Den files), `den_catalog` (what can be
//!   placed: the kit's own ov_build.lua catalog, its build points, the stamps), `den_recipes` (the building brain:
//!   recipes, and a recipe's layout), `den_messages` (what the owner said to the companion).
//! * ask: `den_place_layout` (a `leyline-den` layout, or a recipe with settings: checked against the catalog, the
//!   room's size and budgets, then saved as a Den file and opened in the Den), `den_say` (a line the companion says),
//!   `den_recipe_save` (a new recipe in the library).
//!
//! The realm and the launcher talk through files in the kit's saves folder (`<kit>/state/dens`, the realm's
//! OV_DEN_SAVES), JSON only, nothing in them is ever run:
//!   `companion-in.json`     the realm writes: its heartbeat, the owner's spot, the room's budgets, the build in
//!                           progress, and the last lines the owner said to the companion (`asks`, numbered).
//!   `companion-out.json`    the launcher writes: the lines the companion says (numbered; the realm remembers the
//!                           last one it said), and how far the agent has read (`seen`, `read_at`).
//!   `companion-place.json`  the launcher writes: {file, id, label}: a saved Den file to add to the Den, piece by
//!                           piece (the realm removes it when it starts). "replace" uses ov_den.lua's `open.json`.
//!
//! Recipes (the building brain) are files: `<kit>/state/dens/recipes/<name>.recipe.json` (a JSON template) or
//! `<name>.recipe.lua` (a small script), plus the starters built into the launcher (`den-recipes/`). A script runs
//! here only, on the player's PC, in a sandboxed Lua 5.1 (math, string without patterns, table; a memory and an
//! instruction limit; no files, no network, no loading code). Its output is checked like any other layout.

use super::{arg_str, short, Card};
use crate::host::dens;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

pub const IN_FILE: &str = "companion-in.json";
pub const OUT_FILE: &str = "companion-out.json";
pub const PLACE_FILE: &str = "companion-place.json";
/// Letters in one line the companion says (and one line the owner says to it).
pub const SAY_MAX: usize = 200;
/// Lines kept in companion-out.json.
pub const LINES_KEEP: usize = 20;
/// Pieces in one layout from an agent (a Den file holds up to dens::MAX_PROPS).
pub const PIECES_MAX: usize = 400;
/// The realm's heartbeat counts as fresh this many seconds.
pub const FRESH: u64 = 30;
/// Recipe files: the largest, and how many are read.
pub const RECIPE_MAX_BYTES: u64 = 64 * 1024;
pub const RECIPES_MAX: usize = 200;
/// Words the companion never says (the game's brand: the website, the launcher and the Den never use them).
/// What a scripted recipe says in a launcher built without the lua-recipes feature (the default).
pub const NO_SCRIPTS: &str = "This recipe needs scripted recipes, which aren't in this launcher build yet.";
/// Scripted recipes (Lua) are in this build.
pub const SCRIPTS: bool = cfg!(feature = "lua-recipes");
pub const BRAND_WORDS: &[&str] = &["world of warcraft", "warcraft", "lich king", "blizzard", "wow"];

/// The starter recipes, built in (a file of the same name in the player's recipes folder wins).
pub const BUILT_IN: &[(&str, &str)] = &[
    ("tavern.recipe.json", include_str!("../../den-recipes/tavern.recipe.json")),
    ("garden-scatter.recipe.json", include_str!("../../den-recipes/garden-scatter.recipe.json")),
    ("lantern-ring.recipe.json", include_str!("../../den-recipes/lantern-ring.recipe.json")),
    ("path.recipe.json", include_str!("../../den-recipes/path.recipe.json")),
    ("market-row.recipe.json", include_str!("../../den-recipes/market-row.recipe.json")),
];

// ------------------------------------------------------------------------------------------------ the kit

fn read_json(p: &Path, max: u64) -> Option<Value> {
    let m = fs::metadata(p).ok()?;
    if !m.is_file() || m.len() > max {
        return None;
    }
    let t = fs::read_to_string(p).ok()?;
    serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()
}

fn scripts_dir(kit: &Path) -> PathBuf {
    kit.join("realm-files").join("lua_scripts")
}

pub fn recipes_dir(kit: &Path) -> PathBuf {
    dens::dir(kit).join("recipes")
}

/// The kit's realm profile (state/host.json), a checked word; "den" when it doesn't say.
fn profile(kit: &Path) -> String {
    read_json(&kit.join("state").join("host.json"), 64 * 1024)
        .and_then(|v| v.get("profile").and_then(Value::as_str).map(str::to_string))
        .filter(|p| !p.is_empty() && p.len() <= 32 && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') && !p.contains(".."))
        .unwrap_or_else(|| "den".into())
}

fn parse_env(text: &str, into: &mut HashMap<String, String>) {
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            into.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
        }
    }
}

/// The realm's env as the world server gets it: its profile file, then the host's own state/realm.env.
fn realm_env(kit: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();
    let files = [kit.join("realm-files").join("profiles").join(format!("{}.env", profile(kit))), kit.join("state").join("realm.env")];
    for f in files {
        if let Ok(t) = fs::read_to_string(f) {
            parse_env(&t, &mut env);
        }
    }
    env
}

/// The Den's size and budgets (ov_build.lua DEN, X.denInside / X.denOutside, SCALE_MIN / SCALE_MAX).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Limits {
    /// The room's half width inside its walls (yards); the walls stand at x and y = +-half.
    pub half: f64,
    /// The Den's edge: the room plus its yard, a circle this many yards from the room's middle.
    pub r: f64,
    pub room_points: u32,
    pub yard_points: u32,
    pub scale_min: f64,
    pub scale_max: f64,
    pub pieces_max: usize,
}

pub fn limits(kit: &Path) -> Limits {
    let env = realm_env(kit);
    let num = |k: &str| env.get(k).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite());
    let half = num("OV_DEN_SIZE").unwrap_or(12.0).floor().clamp(8.0, 32.0);
    let pts = |k: &str, d: f64| num(k).unwrap_or(d).floor().clamp(0.0, 100000.0) as u32;
    Limits {
        half,
        r: half + 14.0,
        room_points: pts("OV_DEN_INSIDE_POINTS", 1000.0),
        yard_points: pts("OV_DEN_OUTSIDE_POINTS", 300.0),
        scale_min: num("OV_BUILD_SCALE_MIN").unwrap_or(0.5).clamp(0.1, 1.0),
        scale_max: num("OV_BUILD_SCALE_MAX").unwrap_or(2.5).clamp(1.0, 5.0),
        pieces_max: PIECES_MAX,
    }
}

// ------------------------------------------------------------------------------------------------ the catalog

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Entry {
    pub entry: u64,
    pub key: String,
    pub cat: String,
    pub name: String,
    /// Build points (ov_build.lua costOf: BIG_COST, else CAT_COST by category, else 1).
    pub points: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub entries: Vec<Entry>,
    pub categories: Vec<String>,
}

impl Catalog {
    pub fn key(&self, k: &str) -> Option<&Entry> {
        let k = k.trim().to_ascii_lowercase();
        self.entries.iter().find(|e| e.key == k)
    }
    pub fn id(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.entry == id)
    }
}

fn quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(a) = rest.find('"') {
        let Some(b) = rest[a + 1..].find('"') else { break };
        out.push(rest[a + 1..a + 1 + b].to_string());
        rest = &rest[a + 2 + b..];
    }
    out
}

/// The text of a Lua table that starts at `start` (`local NAME = {`), to its first closing brace on a line of
/// its own (`}`) or, with `inline`, to its first `}`.
fn lua_block<'a>(src: &'a str, start: &str, inline: bool) -> Option<&'a str> {
    let a = src.find(start)? + start.len();
    let rest = &src[a..];
    let b = if inline { rest.find('}')? } else { rest.find("\n}")? };
    Some(&rest[..b])
}

fn field_str(line: &str, k: &str) -> Option<String> {
    let pat = format!("{k} = \"");
    let a = line.find(&pat)? + pat.len();
    let b = line[a..].find('"')?;
    Some(line[a..a + b].to_string())
}

fn field_num(line: &str, k: &str) -> Option<u64> {
    let pat = format!("{k} = ");
    let a = line.find(&pat)? + pat.len();
    line[a..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

/// Reads the catalog out of ov_build.lua (the realm's own allowlist), as the realm does: CATALOG, the Den's buildings,
/// CATEGORIES, and the build points. The realm drops entries its world database doesn't have and checks every
/// piece again; this is the launcher's first check, and what an agent may pick from.
pub fn parse_catalog(src: &str) -> Catalog {
    let mut cat = Catalog::default();
    if let Some(b) = lua_block(src, "local CATALOG = {", false) {
        for l in b.lines() {
            let (Some(id), Some(key), Some(c), Some(name)) = (field_num(l, "id"), field_str(l, "key"), field_str(l, "cat"), field_str(l, "name")) else { continue };
            cat.entries.push(Entry { entry: id, key, cat: c, name, points: 1 });
        }
    }
    if let Some(b) = lua_block(src, "local CATEGORIES = {", true) {
        cat.categories = quoted(b);
    }
    // the Den's buildings: { 19006, "house", "House" }, ... (ov_build.lua adds them in a Den)
    if let Some(a) = src.find("CATALOG[#CATALOG + 1] = { id = b[1], key = b[2], cat = \"buildings\"") {
        let head = &src[..a];
        if let Some(s) = head.rfind("for _, b in ipairs({") {
            for t in head[s..].split('{').skip(2) {
                let t = t.split('}').next().unwrap_or("");
                let id: Option<u64> = t.split(',').next().and_then(|n| n.trim().parse().ok());
                let q = quoted(t);
                if let (Some(id), [k, n]) = (id, q.as_slice()) {
                    cat.entries.push(Entry { entry: id, key: k.clone(), cat: "buildings".into(), name: n.clone(), points: 1 });
                }
            }
            if !cat.categories.iter().any(|c| c == "buildings") {
                cat.categories.push("buildings".into());
            }
        }
    }
    let mut cat_cost = HashMap::new();
    if let Some(b) = lua_block(src, "local CAT_COST = {", true) {
        for p in b.split(',') {
            if let Some((k, v)) = p.split_once('=') {
                if let Ok(n) = v.trim().parse::<u32>() {
                    cat_cost.insert(k.trim().to_string(), n);
                }
            }
        }
    }
    let mut big = HashMap::new();
    if let Some(b) = lua_block(src, "local BIG_COST = {", true) {
        for l in b.lines() {
            let l = l.split("--").next().unwrap_or("");
            for p in l.split(',') {
                if let Some((k, v)) = p.split_once('=') {
                    let k = k.trim().trim_start_matches('[').trim_end_matches(']');
                    if let (Ok(id), Ok(n)) = (k.parse::<u64>(), v.trim().parse::<u32>()) {
                        big.insert(id, n);
                    }
                }
            }
        }
    }
    for e in cat.entries.iter_mut() {
        e.points = big.get(&e.entry).or_else(|| cat_cost.get(&e.cat)).copied().unwrap_or(1);
    }
    // one entry per key and per id (the first wins, as in the realm's index)
    let mut seen_k = std::collections::HashSet::new();
    let mut seen_i = std::collections::HashSet::new();
    cat.entries.retain(|e| seen_k.insert(e.key.clone()) && seen_i.insert(e.entry));
    cat
}

/// The kit's catalog (its realm scripts, as the realm runs them).
pub fn catalog(kit: &Path) -> Result<Catalog, String> {
    let src = fs::read_to_string(scripts_dir(kit).join("ov_build.lua"))
        .map_err(|_| "The player's realm files have no Builder (realm-files/lua_scripts/ov_build.lua): update the Host pack.".to_string())?;
    let c = parse_catalog(&src);
    if c.entries.is_empty() {
        return Err("The Builder's catalog couldn't be read from the realm files.".into());
    }
    Ok(c)
}

#[derive(Debug, Clone, Serialize)]
pub struct Stamp {
    pub name: String,
    pub title: String,
    pub line: String,
    pub pieces: usize,
}

/// The stamps shipped with the kit (den-stamps/*.den.json; `.den stamp <name>` in game).
pub fn stamps(kit: &Path) -> Vec<Stamp> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(scripts_dir(kit).join("den-stamps")) else { return out };
    for e in rd.flatten() {
        let f = e.file_name().to_string_lossy().to_string();
        let Some(name) = f.strip_suffix(".den.json").and_then(dens::clean_name) else { continue };
        let Some(v) = read_json(&e.path(), dens::MAX_BYTES) else { continue };
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(|t| short(t, 160)).unwrap_or_default();
        out.push(Stamp { title: s("title"), line: s("line"), pieces: v.get("props").and_then(Value::as_array).map_or(0, Vec::len), name });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// ------------------------------------------------------------------------------------------------ checking a layout

/// A layout checked against the catalog and the Den: the Den file to write, and what the card says.
#[derive(Debug, Clone)]
pub struct Checked {
    pub doc: Value,
    pub pieces: usize,
    pub room_points: u32,
    pub yard_points: u32,
    /// (name, how many), most first.
    pub kinds: Vec<(String, usize)>,
    /// min x, min y, max x, max y
    pub bounds: [f64; 4],
}

fn r3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn num_of(v: &Value) -> Option<f64> {
    v.as_f64().filter(|f| f.is_finite())
}

/// Checks a layout: `{"props": [{"entry" or "key", "x", "y", "z", "yaw", "pitch", "roll", "scale", "group"}]}`, x and y
/// in yards from the room's middle, z from its floor, angles in degrees. Every piece must be a catalog entry inside
/// the Den; the room's and the yard's budgets must hold it (`used`: points in use now, room and yard, when known).
pub fn check_layout(cat: &Catalog, lim: &Limits, layout: &Value, name: &str, used: Option<(u32, u32)>) -> Result<Checked, String> {
    let o = layout.as_object().ok_or("The layout must be a JSON object with a \"props\" list.")?;
    if let Some(f) = o.get("format").and_then(Value::as_str) {
        if f != dens::FORMAT {
            return Err("That isn't a leyline-den layout.".into());
        }
    }
    if o.get("version").and_then(Value::as_u64).is_some_and(|v| v > dens::VERSION) {
        return Err("That layout is from a newer Leyline.".into());
    }
    if o.get("paths").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
        return Err("Paths aren't taken from an agent: put the path's pieces in props (the \"path\" recipe makes them).".into());
    }
    let props = o.get("props").or_else(|| o.get("pieces")).and_then(Value::as_array).ok_or("The layout has no \"props\" list.")?;
    if props.is_empty() {
        return Err("The layout has no pieces.".into());
    }
    if props.len() > lim.pieces_max {
        return Err(format!("That's {} pieces; one build holds up to {}.", props.len(), lim.pieces_max));
    }
    let mut out = Vec::with_capacity(props.len());
    let (mut pin, mut pout) = (0u32, 0u32);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for (i, p) in props.iter().enumerate() {
        let n = i + 1;
        let p = p.as_object().ok_or(format!("Piece {n} isn't an object."))?;
        let e = match (p.get("entry"), p.get("key")) {
            (Some(v), _) => {
                let id = v.as_u64().ok_or(format!("Piece {n}: entry must be a whole number."))?;
                cat.id(id).ok_or(format!("Piece {n}: entry {id} isn't in the Den catalog (den_catalog lists what can be placed)."))?
            }
            (None, Some(k)) => {
                let k = k.as_str().unwrap_or("");
                cat.key(k).ok_or(format!("Piece {n}: \"{}\" isn't in the Den catalog (den_catalog lists what can be placed).", short(k, 40)))?
            }
            _ => return Err(format!("Piece {n} has no entry or key.")),
        };
        let coord = |k: &str, req: bool| -> Result<Option<f64>, String> {
            match p.get(k) {
                None | Some(Value::Null) => if req { Err(format!("Piece {n} has no {k}.")) } else { Ok(None) },
                Some(v) => num_of(v).map(Some).ok_or(format!("Piece {n}: {k} must be a number.")),
            }
        };
        let (x, y) = (coord("x", true)?.unwrap_or(0.0), coord("y", true)?.unwrap_or(0.0));
        let z = coord("z", false)?.unwrap_or(0.0);
        if (x * x + y * y).sqrt() > lim.r {
            return Err(format!("Piece {n} ({}) is outside the Den: it reaches {:.0} yards from the room's middle (x {:.1}, y {:.1}).",
                e.name, lim.r, x, y));
        }
        if !(-10.0..=20.0).contains(&z) {
            return Err(format!("Piece {n}: z is height above the floor, -10 to 20 yards (it says {z:.1})."));
        }
        let mut q = Map::new();
        q.insert("entry".into(), json!(e.entry));
        q.insert("x".into(), json!(r3(x)));
        q.insert("y".into(), json!(r3(y)));
        q.insert("z".into(), json!(r3(z)));
        for k in ["yaw", "pitch", "roll"] {
            if let Some(a) = coord(k, false)? {
                if a.abs() > 100000.0 {
                    return Err(format!("Piece {n}: {k} is out of range."));
                }
                let a = ((a % 360.0) + 360.0) % 360.0;
                if k == "yaw" || a.abs() > 0.001 {
                    q.insert(k.into(), json!((a * 100.0).round() / 100.0));
                }
            }
        }
        if let Some(s) = coord("scale", false)? {
            if s < lim.scale_min - 1e-9 || s > lim.scale_max + 1e-9 {
                return Err(format!("Piece {n}: size {s} is outside this Den's {} to {}.", lim.scale_min, lim.scale_max));
            }
            if (s - 1.0).abs() > 0.001 {
                q.insert("scale".into(), json!((s * 100.0).round() / 100.0));
            }
        }
        if let Some(g) = p.get("group") {
            let g = g.as_u64().filter(|g| (1..=100000).contains(g)).ok_or(format!("Piece {n}: group must be a whole number from 1."))?;
            q.insert("group".into(), json!(g));
        }
        if x.abs() <= lim.half && y.abs() <= lim.half {
            pin += e.points;
        } else {
            pout += e.points;
        }
        *kinds.entry(e.name.clone()).or_default() += 1;
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        out.push(Value::Object(q));
    }
    let (uin, uout) = used.unwrap_or((0, 0));
    if uin + pin > lim.room_points {
        return Err(format!("That needs {pin} of the room's {} decor points{}. Fewer or lighter pieces, or put some in the yard.",
            lim.room_points, if uin > 0 { format!(" ({uin} in use)") } else { String::new() }));
    }
    if uout + pout > lim.yard_points {
        return Err(format!("That needs {pout} of the yard's {} decor points{}. Fewer or lighter pieces, or put some inside the room.",
            lim.yard_points, if uout > 0 { format!(" ({uout} in use)") } else { String::new() }));
    }
    let mut kinds: Vec<(String, usize)> = kinds.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let doc = json!({"format": dens::FORMAT, "version": dens::VERSION, "name": name, "kind": "agent",
        "room": {"size": lim.half}, "props": out});
    Ok(Checked { doc, pieces: props.len(), room_points: pin, yard_points: pout, kinds, bounds: b })
}

// ------------------------------------------------------------------------------------------------ expressions

/// A steady random number in [0, 1) from a seed and two numbers (splitmix64): the same inputs, the same number.
pub fn rand01(seed: f64, a: f64, b: f64) -> f64 {
    let mut x = seed.to_bits() ^ a.to_bits().rotate_left(21) ^ b.to_bits().rotate_left(42) ^ 0x9E37_79B9_7F4A_7C15;
    for _ in 0..2 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x = z ^ (z >> 31);
    }
    (x >> 11) as f64 / (1u64 << 53) as f64
}

struct Expr<'a> {
    s: &'a [u8],
    i: usize,
    env: &'a HashMap<String, f64>,
    seed: f64,
}

/// Evaluates a recipe expression: numbers, names, + - * / % ^, comparisons (< > <= >= == !=), && || !, `c ? a : b`,
/// and sin cos tan atan2 (degrees), sqrt abs floor ceil round min max clamp rand. Nothing else; no side effects.
pub fn eval(src: &str, env: &HashMap<String, f64>, seed: f64) -> Result<f64, String> {
    if src.len() > 300 {
        return Err("an expression is too long".into());
    }
    let mut e = Expr { s: src.as_bytes(), i: 0, env, seed };
    let v = e.ternary(0)?;
    e.ws();
    if e.i < e.s.len() {
        return Err(format!("can't read \"{}\" in \"{}\"", short(&src[e.i..], 20), short(src, 60)));
    }
    if !v.is_finite() {
        return Err(format!("\"{}\" doesn't give a number", short(src, 60)));
    }
    Ok(v)
}

impl Expr<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, t: &str) -> bool {
        self.ws();
        if self.s[self.i..].starts_with(t.as_bytes()) {
            // "<" is not "<=", "=" never alone
            let after = self.s.get(self.i + t.len()).copied();
            if (t == "<" || t == ">" || t == "!") && after == Some(b'=') {
                return false;
            }
            if (t == "&" || t == "|") && after == Some(t.as_bytes()[0]) {
                return false;
            }
            self.i += t.len();
            return true;
        }
        false
    }
    fn deep(d: usize) -> Result<(), String> {
        if d > 40 { Err("an expression is too deep".into()) } else { Ok(()) }
    }
    fn ternary(&mut self, d: usize) -> Result<f64, String> {
        Self::deep(d)?;
        let c = self.or(d + 1)?;
        if self.eat("?") {
            let a = self.ternary(d + 1)?;
            if !self.eat(":") {
                return Err("a ? needs its :".into());
            }
            let b = self.ternary(d + 1)?;
            return Ok(if c != 0.0 { a } else { b });
        }
        Ok(c)
    }
    fn or(&mut self, d: usize) -> Result<f64, String> {
        let mut v = self.and(d)?;
        while self.eat("||") {
            let r = self.and(d)?;
            v = f64::from(v != 0.0 || r != 0.0);
        }
        Ok(v)
    }
    fn and(&mut self, d: usize) -> Result<f64, String> {
        let mut v = self.cmp(d)?;
        while self.eat("&&") {
            let r = self.cmp(d)?;
            v = f64::from(v != 0.0 && r != 0.0);
        }
        Ok(v)
    }
    fn cmp(&mut self, d: usize) -> Result<f64, String> {
        let a = self.add(d)?;
        for op in ["<=", ">=", "==", "!=", "<", ">"] {
            if self.eat(op) {
                let b = self.add(d)?;
                return Ok(f64::from(match op {
                    "<=" => a <= b,
                    ">=" => a >= b,
                    "==" => (a - b).abs() < 1e-9,
                    "!=" => (a - b).abs() >= 1e-9,
                    "<" => a < b,
                    _ => a > b,
                }));
            }
        }
        Ok(a)
    }
    fn add(&mut self, d: usize) -> Result<f64, String> {
        let mut v = self.mul(d)?;
        loop {
            if self.eat("+") {
                v += self.mul(d)?;
            } else if self.eat("-") {
                v -= self.mul(d)?;
            } else {
                return Ok(v);
            }
        }
    }
    fn mul(&mut self, d: usize) -> Result<f64, String> {
        let mut v = self.unary(d)?;
        loop {
            if self.eat("*") {
                v *= self.unary(d)?;
            } else if self.eat("/") {
                let r = self.unary(d)?;
                if r == 0.0 {
                    return Err("a division by zero".into());
                }
                v /= r;
            } else if self.eat("%") {
                let r = self.unary(d)?;
                if r == 0.0 {
                    return Err("a % by zero".into());
                }
                v -= (v / r).floor() * r; // Lua's: the sign of the divisor
            } else {
                return Ok(v);
            }
        }
    }
    fn unary(&mut self, d: usize) -> Result<f64, String> {
        Self::deep(d)?;
        if self.eat("-") {
            return Ok(-self.unary(d + 1)?);
        }
        if self.eat("!") {
            return Ok(f64::from(self.unary(d + 1)? == 0.0));
        }
        let b = self.atom(d + 1)?;
        if self.eat("^") {
            return Ok(b.powf(self.unary(d + 1)?));
        }
        Ok(b)
    }
    fn atom(&mut self, d: usize) -> Result<f64, String> {
        self.ws();
        let start = self.i;
        let c = *self.s.get(self.i).ok_or("an expression ends too soon")?;
        if c == b'(' {
            self.i += 1;
            let v = self.ternary(d + 1)?;
            if !self.eat(")") {
                return Err("a ( needs its )".into());
            }
            return Ok(v);
        }
        if c.is_ascii_digit() || c == b'.' {
            while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') {
                self.i += 1;
            }
            let t = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
            return t.parse().map_err(|_| format!("\"{t}\" isn't a number"));
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_') {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("").to_string();
            if self.eat("(") {
                let mut args = Vec::new();
                if !self.eat(")") {
                    loop {
                        args.push(self.ternary(d + 1)?);
                        if self.eat(")") {
                            break;
                        }
                        if !self.eat(",") {
                            return Err(format!("{name}( needs its )"));
                        }
                    }
                }
                return self.call(&name, &args);
            }
            if name == "pi" {
                return Ok(std::f64::consts::PI);
            }
            return self.env.get(&name).copied().ok_or(format!("\"{name}\" isn't a setting or a value here"));
        }
        Err(format!("can't read \"{}\"", c as char))
    }
    fn call(&self, f: &str, a: &[f64]) -> Result<f64, String> {
        let need = |n: usize| if a.len() == n { Ok(()) } else { Err(format!("{f} takes {n} number{}", if n == 1 { "" } else { "s" })) };
        let rad = |x: f64| x.to_radians();
        Ok(match f {
            "sin" => { need(1)?; rad(a[0]).sin() }
            "cos" => { need(1)?; rad(a[0]).cos() }
            "tan" => { need(1)?; rad(a[0]).tan() }
            "atan2" => { need(2)?; a[0].atan2(a[1]).to_degrees() }
            "sqrt" => { need(1)?; if a[0] < 0.0 { return Err("sqrt of a negative number".into()) } a[0].sqrt() }
            "abs" => { need(1)?; a[0].abs() }
            "floor" => { need(1)?; a[0].floor() }
            "ceil" => { need(1)?; a[0].ceil() }
            "round" => { need(1)?; a[0].round() }
            "min" | "max" if !a.is_empty() => a.iter().copied().fold(if f == "min" { f64::MAX } else { f64::MIN }, |m, x| if f == "min" { m.min(x) } else { m.max(x) }),
            "clamp" => { need(3)?; a[0].max(a[1]).min(a[2]) }
            "rand" if a.len() == 1 || a.len() == 2 => rand01(self.seed, a[0], a.get(1).copied().unwrap_or(0.0)),
            _ => return Err(format!("\"{f}\" isn't a function recipes have")),
        })
    }
}

// ------------------------------------------------------------------------------------------------ recipes

#[derive(Debug, Clone)]
pub enum Body {
    Template(Value),
    Script(String),
}

#[derive(Debug, Clone)]
pub struct Recipe {
    pub name: String,
    pub title: String,
    pub about: String,
    /// "built in" or "yours" (a file in the recipes folder)
    pub source: &'static str,
    pub file: String,
    /// The settings: name -> {default, min, max, whole, piece, about}
    pub params: Map<String, Value>,
    pub body: Body,
}

impl Recipe {
    pub fn kind(&self) -> &'static str {
        match self.body {
            Body::Template(_) => "template",
            Body::Script(_) => "script",
        }
    }
    pub fn brief(&self) -> Value {
        json!({"name": self.name, "title": self.title, "about": self.about, "kind": self.kind(), "source": self.source,
            "file": self.file, "params": self.params, "usable": !matches!(self.body, Body::Script(_)) || SCRIPTS})
    }
}

/// A recipe file's name: `<name>.recipe.json` or `<name>.recipe.lua`, the name a checked word.
fn recipe_name(file: &str) -> Option<(String, bool)> {
    let (n, lua) = if let Some(n) = file.strip_suffix(".recipe.json") { (n, false) } else { (file.strip_suffix(".recipe.lua")?, true) };
    dens::clean_name(n).filter(|c| c == n).map(|c| (c, lua))
}

/// Reads one recipe from its text (`lua`: a script). The script runs once, sandboxed, to read its `recipe` table.
pub fn parse_recipe(file: &str, text: &str, lua: bool, source: &'static str) -> Result<Recipe, String> {
    let fallback = recipe_name(file).map(|r| r.0).unwrap_or_default();
    let (meta, body) = if lua {
        #[cfg(feature = "lua-recipes")]
        let meta = script_meta(text)?;
        // without Lua: listed by its file name, refused when used (run_recipe)
        #[cfg(not(feature = "lua-recipes"))]
        let meta = json!({"about": format!("A scripted recipe. {NO_SCRIPTS}")});
        (meta, Body::Script(text.to_string()))
    } else {
        let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("not JSON ({e})"))?;
        if v.get("format").and_then(Value::as_str) != Some("leyline-recipe") {
            return Err("not a leyline-recipe".into());
        }
        if v.get("version").and_then(Value::as_u64).unwrap_or(0) != 1 {
            return Err("a recipe version this Leyline doesn't know".into());
        }
        if !v.get("pieces").is_some_and(Value::is_array) {
            return Err("a template needs a \"pieces\" list".into());
        }
        (v.clone(), Body::Template(v))
    };
    let s = |k: &str, n: usize| meta.get(k).and_then(Value::as_str).map(|t| short(t, n)).unwrap_or_default();
    // the file's name is the recipe's name (a copy saved as "tavern-2" is "tavern-2"); else the one inside
    let name = if fallback.is_empty() { dens::clean_name(&s("name", 40)).unwrap_or_default() } else { fallback };
    if name.is_empty() {
        return Err("a recipe needs a name".into());
    }
    let params = meta.get("params").and_then(Value::as_object).cloned().unwrap_or_default();
    if params.len() > 24 {
        return Err("a recipe has up to 24 settings".into());
    }
    for (k, p) in &params {
        if dens::clean_name(k).as_deref() != Some(k.as_str()) || !p.is_object() {
            return Err(format!("the setting \"{}\" isn't right", short(k, 30)));
        }
    }
    let title = Some(s("title", 60)).filter(|t| !t.is_empty()).unwrap_or_else(|| name.clone());
    Ok(Recipe { title, about: s("about", 300), source, file: file.to_string(), params, body, name })
}

/// The recipe library: the built-in starters, then the player's own files (`<kit>/state/dens/recipes`), which win
/// by name. Broken files are left out (`problems` says why).
pub fn library(kit: Option<&Path>) -> (Vec<Recipe>, Vec<String>) {
    let mut by: BTreeMap<String, Recipe> = BTreeMap::new();
    let mut problems = Vec::new();
    for (f, t) in BUILT_IN {
        match parse_recipe(f, t, f.ends_with(".lua"), "built in") {
            Ok(r) => {
                by.insert(r.name.clone(), r);
            }
            Err(e) => problems.push(format!("{f}: {e}")),
        }
    }
    if let Some(rd) = kit.and_then(|k| fs::read_dir(recipes_dir(k)).ok()) {
        let mut files: Vec<_> = rd.flatten().collect();
        files.sort_by_key(|e| e.file_name());
        for e in files.into_iter().take(RECIPES_MAX) {
            let f = e.file_name().to_string_lossy().to_string();
            let Some((_, lua)) = recipe_name(&f) else { continue };
            if e.metadata().map(|m| m.len() > RECIPE_MAX_BYTES).unwrap_or(true) {
                problems.push(format!("{f}: too big (64 KB at most)"));
                continue;
            }
            match fs::read_to_string(e.path()).map_err(|e| e.to_string()).and_then(|t| parse_recipe(&f, &t, lua, "yours")) {
                Ok(r) => {
                    by.insert(r.name.clone(), r);
                }
                Err(e) => problems.push(format!("{f}: {e}")),
            }
        }
    }
    (by.into_values().collect(), problems)
}

#[derive(Debug, Clone, PartialEq)]
pub enum PVal {
    Num(f64),
    Key(String),
}

/// The settings for one run: each setting's default, or what was given (checked: a number in range, whole when it
/// must be, a catalog key for a piece).
pub fn settings(r: &Recipe, given: &Value, cat: &Catalog) -> Result<BTreeMap<String, PVal>, String> {
    let given = match given {
        Value::Null => Map::new(),
        Value::Object(o) => o.clone(),
        _ => return Err("params must be an object of settings.".into()),
    };
    for k in given.keys() {
        if !r.params.contains_key(k) {
            let have: Vec<&str> = r.params.keys().map(String::as_str).collect();
            return Err(format!("The recipe \"{}\" has no setting \"{}\" (it has: {}).", r.name, short(k, 30), have.join(", ")));
        }
    }
    let mut out = BTreeMap::new();
    for (k, spec) in &r.params {
        let v = given.get(k).or_else(|| spec.get("default")).cloned().unwrap_or(Value::Null);
        if spec.get("piece").and_then(Value::as_bool) == Some(true) {
            let key = v.as_str().ok_or(format!("{k}: a catalog key, like \"lantern\"."))?;
            let e = cat.key(key).ok_or(format!("{k}: \"{}\" isn't in the Den catalog.", short(key, 40)))?;
            out.insert(k.clone(), PVal::Key(e.key.clone()));
            continue;
        }
        let mut n = v.as_f64().filter(|f| f.is_finite()).ok_or(format!("{k}: a number, please."))?;
        if spec.get("whole").and_then(Value::as_bool) == Some(true) {
            if given.contains_key(k) && n.fract() != 0.0 {
                return Err(format!("{k}: a whole number, please."));
            }
            n = n.round();
        }
        let lo = spec.get("min").and_then(Value::as_f64).unwrap_or(-1000.0);
        let hi = spec.get("max").and_then(Value::as_f64).unwrap_or(1000.0);
        if n < lo || n > hi {
            return Err(format!("{k}: {n} is outside {lo} to {hi}."));
        }
        out.insert(k.clone(), PVal::Num(n));
    }
    Ok(out)
}

fn seed_of(p: &BTreeMap<String, PVal>) -> f64 {
    match p.get("seed") {
        Some(PVal::Num(n)) => *n,
        _ => 1.0,
    }
}

/// Runs a recipe: its pieces, as a layout's props ({key, x, y, z, yaw, scale}), not yet checked.
pub fn run_recipe(r: &Recipe, p: &BTreeMap<String, PVal>) -> Result<Vec<Value>, String> {
    if matches!(r.body, Body::Script(_)) && !SCRIPTS {
        return Err(NO_SCRIPTS.into());
    }
    let out = match &r.body {
        Body::Template(t) => run_template(t, p),
        Body::Script(s) => run_script(s, p),
    }
    .map_err(|e| format!("The recipe \"{}\" didn't work: {e}", r.name))?;
    if out.is_empty() {
        return Err(format!("The recipe \"{}\" made no pieces with those settings.", r.name));
    }
    Ok(out)
}

fn run_template(t: &Value, p: &BTreeMap<String, PVal>) -> Result<Vec<Value>, String> {
    let seed = seed_of(p);
    let mut env: HashMap<String, f64> = HashMap::new();
    let mut keys: HashMap<String, String> = HashMap::new();
    for (k, v) in p {
        match v {
            PVal::Num(n) => {
                env.insert(k.clone(), *n);
            }
            PVal::Key(s) => {
                keys.insert(k.clone(), s.clone());
            }
        }
    }
    let lets = |list: Option<&Value>, env: &mut HashMap<String, f64>| -> Result<(), String> {
        for l in list.and_then(Value::as_array).into_iter().flatten() {
            let (Some(n), Some(e)) = (l.get(0).and_then(Value::as_str), l.get(1)) else { return Err("a let is [name, expression]".into()) };
            let v = match e {
                Value::String(s) => eval(s, env, seed)?,
                v => num_of(v).ok_or("a let's value must be a number or an expression")?,
            };
            env.insert(n.to_string(), v);
        }
        Ok(())
    };
    lets(t.get("let"), &mut env)?;
    let mut out = Vec::new();
    let mut steps = 0usize;
    for spec in t.get("pieces").and_then(Value::as_array).into_iter().flatten() {
        let loops: Vec<(String, String)> = spec.get("repeat").and_then(Value::as_array).into_iter().flatten()
            .map(|l| {
                let n = l.get(0).and_then(Value::as_str).unwrap_or("").to_string();
                let c = match l.get(1) {
                    Some(Value::String(s)) => s.clone(),
                    Some(v) => v.to_string(),
                    None => String::new(),
                };
                (n, c)
            })
            .collect();
        if loops.len() > 3 || loops.iter().any(|(n, c)| n.is_empty() || c.is_empty()) {
            return Err("repeat is up to 3 [name, count] pairs".into());
        }
        piece_loop(spec, &loops, 0, &mut env.clone(), &keys, seed, &lets, &mut out, &mut steps)?;
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn piece_loop(
    spec: &Value,
    loops: &[(String, String)],
    depth: usize,
    env: &mut HashMap<String, f64>,
    keys: &HashMap<String, String>,
    seed: f64,
    lets: &dyn Fn(Option<&Value>, &mut HashMap<String, f64>) -> Result<(), String>,
    out: &mut Vec<Value>,
    steps: &mut usize,
) -> Result<(), String> {
    if depth < loops.len() {
        let (name, count) = &loops[depth];
        let n = eval(count, env, seed)?.floor();
        if !(0.0..=PIECES_MAX as f64).contains(&n) {
            return Err(format!("repeat {name}: {n} times is too many"));
        }
        for i in 0..n as usize {
            env.insert(name.clone(), i as f64);
            piece_loop(spec, loops, depth + 1, env, keys, seed, lets, out, steps)?;
        }
        return Ok(());
    }
    *steps += 1;
    if *steps > 20 * PIECES_MAX {
        return Err("it runs too long".into());
    }
    let mut env = env.clone();
    lets(spec.get("let"), &mut env)?;
    if let Some(w) = spec.get("when") {
        let ok = match w {
            Value::String(s) => eval(s, &env, seed)? != 0.0,
            Value::Bool(b) => *b,
            v => num_of(v).unwrap_or(0.0) != 0.0,
        };
        if !ok {
            return Ok(());
        }
    }
    let key = if let Some(pick) = spec.get("pick") {
        let list: Vec<&str> = pick.get("keys").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
        let by = pick.get("by").and_then(Value::as_str).ok_or("pick needs keys and by")?;
        if list.is_empty() {
            return Err("pick needs keys".into());
        }
        let i = eval(by, &env, seed)?.floor().rem_euclid(list.len() as f64) as usize;
        list[i].to_string()
    } else {
        let k = spec.get("key").and_then(Value::as_str).ok_or("a piece needs a key (or pick)")?;
        match k.strip_prefix('$') {
            Some(p) => keys.get(p).cloned().ok_or(format!("\"${p}\" isn't a piece setting"))?,
            None => k.to_string(),
        }
    };
    let mut q = Map::new();
    q.insert("key".into(), json!(key));
    for k in ["x", "y", "z", "yaw", "scale"] {
        let v = match spec.get(k) {
            None | Some(Value::Null) => continue,
            Some(Value::String(s)) => eval(s, &env, seed).map_err(|e| format!("{k}: {e}"))?,
            Some(v) => num_of(v).ok_or(format!("{k} must be a number or an expression"))?,
        };
        q.insert(k.into(), json!(v));
    }
    if out.len() >= PIECES_MAX {
        return Err(format!("it makes more than {PIECES_MAX} pieces"));
    }
    out.push(Value::Object(q));
    Ok(())
}

// ------------------------------------------------------------------------------------------------ the Lua sandbox

#[cfg(feature = "lua-recipes")]
/// A Lua 5.1 with math, string (no patterns, no dump, no rep) and table only: no files, no os, no loading code, a
/// 16 MB memory limit and an instruction limit. `rand(a, b)` (and math.random) are the steady random numbers.
fn sandbox(seed: f64) -> Result<mlua::Lua, String> {
    use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Value as LV, VmState};
    let e = |x: mlua::Error| x.to_string();
    let lua = Lua::new_with(StdLib::MATH | StdLib::STRING | StdLib::TABLE, LuaOptions::default()).map_err(e)?;
    lua.set_memory_limit(16 << 20).map_err(e)?;
    let g = lua.globals();
    for n in ["dofile", "loadfile", "load", "loadstring", "require", "module", "getfenv", "setfenv", "collectgarbage", "newproxy",
        "gcinfo", "print", "rawset", "rawget", "rawequal", "coroutine", "os", "io", "debug", "package", "_VERSION"] {
        g.set(n, LV::Nil).map_err(e)?;
    }
    let string: mlua::Table = g.get("string").map_err(e)?;
    for n in ["dump", "find", "match", "gmatch", "gsub", "rep"] {
        string.set(n, LV::Nil).map_err(e)?;
    }
    let rand = lua.create_function(move |_, (a, b): (f64, Option<f64>)| Ok(rand01(seed, a, b.unwrap_or(0.0)))).map_err(e)?;
    g.set("rand", rand).map_err(e)?;
    let n = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let random = lua.create_function(move |_, (lo, hi): (Option<f64>, Option<f64>)| {
        n.set(n.get() + 1);
        let r = rand01(seed, n.get() as f64, -1.0);
        Ok(match (lo, hi) {
            (None, _) => r,
            (Some(m), None) => (r * m.floor().max(1.0)).floor() + 1.0,
            (Some(a), Some(b)) => a.floor() + (r * (b.floor() - a.floor() + 1.0).max(1.0)).floor(),
        })
    }).map_err(e)?;
    let math: mlua::Table = g.get("math").map_err(e)?;
    math.set("random", random).map_err(e)?;
    math.set("randomseed", lua.create_function(|_, _: mlua::MultiValue| Ok(())).map_err(e)?).map_err(e)?;
    let ticks = std::rc::Rc::new(std::cell::Cell::new(0u32));
    lua.set_hook(HookTriggers::new().every_nth_instruction(1000), move |_, _| {
        ticks.set(ticks.get() + 1);
        if ticks.get() > 5000 {
            Err(mlua::Error::runtime("the recipe runs too long"))
        } else {
            Ok(VmState::Continue)
        }
    });
    Ok(lua)
}

#[cfg(feature = "lua-recipes")]
fn lua_err(e: mlua::Error) -> String {
    let s = e.to_string();
    let first = s.lines().next().unwrap_or("").replace("runtime error: ", "");
    short(&first, 200)
}

#[cfg(feature = "lua-recipes")]
fn load_script(lua: &mlua::Lua, src: &str) -> Result<(), String> {
    if src.len() as u64 > RECIPE_MAX_BYTES {
        return Err("too big (64 KB at most)".into());
    }
    if src.as_bytes().first() == Some(&0x1b) {
        return Err("only Lua text, not compiled code".into());
    }
    lua.load(src).set_name("recipe").set_mode(mlua::ChunkMode::Text).exec().map_err(lua_err)
}

#[cfg(feature = "lua-recipes")]
fn lua_to_json(v: mlua::Value, depth: usize) -> Value {
    match v {
        mlua::Value::Boolean(b) => json!(b),
        mlua::Value::Integer(i) => json!(i),
        mlua::Value::Number(n) if n.is_finite() => json!(n),
        mlua::Value::String(s) => json!(s.to_str().map(|t| t.to_string()).unwrap_or_default()),
        mlua::Value::Table(t) if depth < 4 => {
            let mut m = Map::new();
            for (k, v) in t.pairs::<mlua::Value, mlua::Value>().flatten().take(64) {
                let k = match k {
                    mlua::Value::String(s) => s.to_str().map(|t| t.to_string()).unwrap_or_default(),
                    mlua::Value::Integer(i) => i.to_string(),
                    _ => continue,
                };
                m.insert(k, lua_to_json(v, depth + 1));
            }
            Value::Object(m)
        }
        _ => Value::Null,
    }
}

#[cfg(feature = "lua-recipes")]
/// A script recipe's `recipe` table (name, title, about, params), read by running the script's definitions once.
fn script_meta(src: &str) -> Result<Value, String> {
    let lua = sandbox(1.0)?;
    load_script(&lua, src)?;
    let r: mlua::Value = lua.globals().get("recipe").map_err(lua_err)?;
    if !r.is_table() {
        return Err("a script recipe sets a global table `recipe` (name, title, params)".into());
    }
    let f: mlua::Value = lua.globals().get("build").map_err(lua_err)?;
    if !f.is_function() {
        return Err("a script recipe has a function build(params) that returns its pieces".into());
    }
    Ok(lua_to_json(r, 0))
}

#[cfg(not(feature = "lua-recipes"))]
fn run_script(_src: &str, _p: &BTreeMap<String, PVal>) -> Result<Vec<Value>, String> {
    Err(NO_SCRIPTS.into())
}

#[cfg(feature = "lua-recipes")]
fn run_script(src: &str, p: &BTreeMap<String, PVal>) -> Result<Vec<Value>, String> {
    let lua = sandbox(seed_of(p))?;
    load_script(&lua, src)?;
    let t = lua.create_table().map_err(lua_err)?;
    for (k, v) in p {
        match v {
            PVal::Num(n) => t.set(k.as_str(), *n).map_err(lua_err)?,
            PVal::Key(s) => t.set(k.as_str(), s.as_str()).map_err(lua_err)?,
        }
    }
    let build: mlua::Function = lua.globals().get("build").map_err(|_| "no build function".to_string())?;
    let list: mlua::Table = build.call(t).map_err(lua_err)?;
    let mut out = Vec::new();
    for item in list.sequence_values::<mlua::Table>() {
        let item = item.map_err(|_| "build() must return a list of pieces (tables)".to_string())?;
        if out.len() >= PIECES_MAX {
            return Err(format!("it makes more than {PIECES_MAX} pieces"));
        }
        let mut q = Map::new();
        let key: Option<String> = item.get("key").ok();
        q.insert("key".into(), json!(key.ok_or("a piece needs a key")?));
        for k in ["x", "y", "z", "yaw", "scale"] {
            let v: Option<f64> = item.get(k).map_err(|_| format!("a piece's {k} must be a number"))?;
            if let Some(v) = v {
                q.insert(k.into(), json!(v));
            }
        }
        out.push(Value::Object(q));
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------------ the companion's files

fn companion_in(kit: &Path) -> Option<Value> {
    read_json(&dens::dir(kit).join(IN_FILE), 256 * 1024).filter(Value::is_object)
}

fn is_fresh(v: &Value) -> bool {
    v.get("at").and_then(Value::as_u64).is_some_and(|t| super::now().abs_diff(t) <= FRESH)
}

fn out_doc(kit: &Path) -> Value {
    read_json(&dens::dir(kit).join(OUT_FILE), 256 * 1024)
        .filter(|v| v.get("lines").is_some_and(Value::is_array))
        .unwrap_or_else(|| json!({"format": "leyline-companion", "lines": [], "seen": 0, "read_at": 0}))
}

fn write_out(kit: &Path, v: &Value) -> Result<(), String> {
    let b = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&dens::dir(kit).join(OUT_FILE), &b).map_err(|e| format!("Couldn't write to the Den's folder: {e}"))
}

/// A line for the companion to say: numbered after the last one; the last LINES_KEEP are kept.
pub fn push_line(kit: &Path, text: &str) -> Result<u64, String> {
    let mut v = out_doc(kit);
    let lines = v["lines"].as_array().cloned().unwrap_or_default();
    let n = lines.iter().filter_map(|l| l.get("n").and_then(Value::as_u64)).max().unwrap_or(0) + 1;
    let mut lines: Vec<Value> = lines.into_iter().rev().take(LINES_KEEP - 1).collect::<Vec<_>>().into_iter().rev().collect();
    lines.push(json!({"n": n, "at": super::now(), "text": text}));
    v["lines"] = Value::Array(lines);
    write_out(kit, &v)?;
    Ok(n)
}

/// What the owner said to the companion, after `after` (numbered by the realm).
fn asks(kit: &Path) -> Vec<Value> {
    companion_in(kit).and_then(|v| v.get("asks").and_then(Value::as_array).cloned()).unwrap_or_default()
        .into_iter()
        .filter(|a| a.get("n").and_then(Value::as_u64).is_some())
        .map(|a| {
            let text = a.get("text").and_then(Value::as_str).map(|t| short(&clean_line(t), SAY_MAX)).unwrap_or_default();
            let mut o = json!({"n": a["n"], "at": a.get("at").cloned().unwrap_or(Value::Null), "text": text});
            if let Some(w) = a.get("where").filter(|w| w.is_object()) {
                let mut m = Map::new();
                for k in ["x", "y", "z", "yaw"] {
                    if let Some(v) = w.get(k).and_then(num_of) {
                        m.insert(k.into(), json!((v * 10.0).round() / 10.0));
                    }
                }
                o["where"] = Value::Object(m);
            }
            o
        })
        .collect()
}

/// A line in plain text: no control characters, no `|` (the game's chat codes), single spaces.
pub fn clean_line(s: &str) -> String {
    s.chars().map(|c| if c.is_control() || c == '|' { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The brand word in a line, if there is one (whole words for "wow").
pub fn brand_word(s: &str) -> Option<&'static str> {
    let low = s.to_lowercase();
    let words: Vec<&str> = low.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let joined = words.join(" ");
    BRAND_WORDS.iter().copied().find(|b| if *b == "wow" { words.contains(&"wow") } else { joined.contains(b) || low.contains(b) })
}

fn say_text(args: &Value) -> Result<String, String> {
    let t = clean_line(arg_str(args, "text").ok_or("Say what the companion should say (text).")?);
    if t.is_empty() {
        return Err("Say what the companion should say (text).".into());
    }
    if t.chars().count() > SAY_MAX {
        return Err(format!("One line is up to {SAY_MAX} letters: say it in two."));
    }
    if let Some(w) = brand_word(&t) {
        return Err(format!("The companion never says \"{w}\" (the game's brand stays out of Leyline): say it another way."));
    }
    Ok(t)
}

// ------------------------------------------------------------------------------------------------ the tools

/// The kit, for the Den tools.
fn den_kit(app_dir: &Path) -> Result<PathBuf, String> {
    super::need_kit(app_dir).map_err(|_| "The player has no Den on this PC yet: Leyline › Host › Your Den sets one up.".to_string())
}

fn used_now(kit: &Path) -> Option<(u32, u32)> {
    let v = companion_in(kit).filter(is_fresh)?;
    let r = v.get("room")?;
    Some((r.get("room_points")?.as_u64()? as u32, r.get("yard_points")?.as_u64()? as u32))
}

/// The layout a den_place_layout call asks for: its own, or a recipe's with settings. (layout, label, recipe line)
fn wanted(kit: &Path, cat: &Catalog, args: &Value) -> Result<(Value, String, Option<String>, String), String> {
    let name_arg = arg_str(args, "name");
    if let Some(rn) = arg_str(args, "recipe") {
        let (lib, _) = library(Some(kit));
        let rn = rn.to_ascii_lowercase();
        let r = lib.iter().find(|r| r.name == rn).ok_or(format!("There is no recipe \"{}\" (den_recipes lists them).", short(&rn, 40)))?;
        let p = settings(r, args.get("params").unwrap_or(&Value::Null), cat)?;
        let props = run_recipe(r, &p)?;
        let shown: Vec<String> = p.iter().map(|(k, v)| match v {
            PVal::Num(n) => format!("{k} {}", (n * 100.0).round() / 100.0),
            PVal::Key(s) => format!("{k} {s}"),
        }).collect();
        let name = name_arg.and_then(dens::clean_name).unwrap_or_else(|| r.name.clone());
        let label = format!("the {}", r.title.to_lowercase());
        return Ok((json!({"props": props}), label, Some(format!("From the recipe \"{}\": {}.", r.title, shown.join(", "))), name));
    }
    if args.get("params").is_some() {
        return Err("params go with a recipe.".into());
    }
    let layout = args.get("layout").ok_or("Give a layout (a leyline-den object with props) or a recipe.")?.clone();
    let name = name_arg.and_then(dens::clean_name)
        .or_else(|| layout.get("name").and_then(Value::as_str).and_then(dens::clean_name))
        .unwrap_or_else(|| "agent-build".into());
    let label = arg_str(args, "label").map(|l| short(&clean_line(l), 40)).filter(|l| !l.is_empty()).unwrap_or_else(|| name.replace(['-', '_'], " "));
    Ok((layout, label, None, name))
}

fn mode(args: &Value) -> Result<&'static str, String> {
    match arg_str(args, "mode").unwrap_or("add") {
        "add" => Ok("add"),
        "replace" => Ok("replace"),
        _ => Err("mode is \"add\" (next to what is there) or \"replace\" (the whole Den).".into()),
    }
}

/// Everything den_place_layout will do, checked: (checked layout, label, name, mode, the card).
pub fn plan_place(kit: &Path, args: &Value) -> Result<(Checked, String, String, &'static str, Card), String> {
    let cat = catalog(kit)?;
    let lim = limits(kit);
    let mode = mode(args)?;
    let (layout, label, recipe, name) = wanted(kit, &cat, args)?;
    if mode == "add" {
        if companion_in(kit).is_none() {
            return Err("Adding to the Den needs the companion script on the Den's realm (OV_DEN_COMPANION=1, on in the den profile) \
                and the realm started once since. mode \"replace\" opens the layout as the whole Den instead.".into());
        }
        if dens::dir(kit).join(PLACE_FILE).exists() {
            return Err("The last build is still waiting to go into the Den (it goes in when the player is in their Den). Try again after.".into());
        }
    }
    let used = if mode == "add" { used_now(kit) } else { None };
    let c = check_layout(&cat, &lim, &layout, &name, used)?;
    let mut kinds: Vec<String> = c.kinds.iter().take(6).map(|(n, k)| format!("{k} {n}")).collect();
    if c.kinds.len() > 6 {
        kinds.push(format!("{} more kinds", c.kinds.len() - 6));
    }
    let [x0, y0, x1, y1] = c.bounds;
    let mut lines = vec![
        format!("{} pieces: {}.", c.pieces, kinds.join(", ")),
        format!("Decor points: {} of the room's {}, {} of the yard's {}{}.", c.room_points, lim.room_points, c.yard_points, lim.yard_points,
            used.map(|(a, b)| format!(" ({a} and {b} in use now)")).unwrap_or_default()),
        format!("Where: around x {:.0}, y {:.0} from the room's middle, {:.0} by {:.0} yards.", (x0 + x1) / 2.0, (y0 + y1) / 2.0, x1 - x0, y1 - y0),
    ];
    if let Some(r) = recipe {
        lines.push(short(&r, 300));
    }
    lines.push(if mode == "add" {
        "Added to what is in your Den, a few pieces at a time, while you're in it. Ctrl+Z takes each part back.".into()
    } else {
        "Replaces everything in your Den. A copy of what is there now is kept as \"backup\".".into()
    });
    lines.push(format!("Saved on your PC as a Den file, \"{name}\" (Host › Saved Dens)."));
    let card = Card { title: format!("Build {} in your Den", short(&label, 40)), lines };
    Ok((c, label, name, mode, card))
}

/// Check an ask tool's arguments and say what will happen (None: not a Den tool).
pub fn describe(app_dir: &Path, name: &str, args: &Value) -> Option<Result<Card, String>> {
    Some(match name {
        "den_place_layout" => den_kit(app_dir).and_then(|k| plan_place(&k, args)).map(|p| p.4),
        "den_say" => den_kit(app_dir).and_then(|k| {
            let t = say_text(args)?;
            let out = companion_in(&k).filter(is_fresh).and_then(|v| v.get("companion").and_then(Value::as_bool)) == Some(true);
            Ok(Card {
                title: "Your companion says a line in your Den".into(),
                lines: vec![format!("\"{t}\""), if out { "Everyone in your Den hears it.".into() } else {
                    "Your companion isn't out right now: it says this when you call it (.den jaina in game), if that's soon.".into()
                }],
            })
        }),
        "den_recipe_save" => den_kit(app_dir).and_then(|k| plan_recipe(&k, args)).map(|p| p.3),
        _ => return None,
    })
}

/// A new recipe to save: (file name, text, is it a script, the card).
fn plan_recipe(kit: &Path, args: &Value) -> Result<(String, String, bool, Card), String> {
    let name = arg_str(args, "name").and_then(dens::clean_name).ok_or("Give the recipe a name: letters, digits, - and _.")?;
    let (text, lua) = match (args.get("recipe"), arg_str(args, "script")) {
        (Some(r), None) if r.is_object() => {
            let mut r = r.clone();
            r["format"] = json!("leyline-recipe");
            r["version"] = json!(1);
            r["name"] = json!(name);
            (serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?, false)
        }
        (None, Some(_)) if !SCRIPTS => {
            return Err("Scripted recipes aren't in this launcher build yet: save it as a JSON template (recipe) instead.".into())
        }
        (None, Some(s)) => (s.to_string(), true),
        _ => return Err("Give either recipe (a JSON template) or script (Lua text), not both.".into()),
    };
    if text.len() as u64 > RECIPE_MAX_BYTES {
        return Err("A recipe is up to 64 KB.".into());
    }
    let file = format!("{name}.recipe.{}", if lua { "lua" } else { "json" });
    let r = parse_recipe(&file, &text, lua, "yours").map_err(|e| format!("That recipe isn't right: {e}"))?;
    let cat = catalog(kit)?;
    let p = settings(&r, &Value::Null, &cat)?;
    let c = check_layout(&cat, &limits(kit), &json!({"props": run_recipe(&r, &p)?}), &name, None)
        .map_err(|e| format!("With its default settings the recipe doesn't fit a Den: {e}"))?;
    let setting_words: Vec<String> = r.params.iter().map(|(k, s)| format!("{k} ({})", s.get("default").map(|d| d.to_string().trim_matches('"').to_string()).unwrap_or_default())).collect();
    let mut lines = vec![
        format!("\"{}\"{}", r.title, if r.about.is_empty() { String::new() } else { format!(": {}", short(&r.about, 200)) }),
        if lua { format!("A small script ({} lines). It runs only on your PC, in a sandbox: no files, no network.", text.lines().count()) }
        else { "A layout template (numbers and pieces, nothing that runs).".into() },
        format!("Settings: {}.", if setting_words.is_empty() { "none".into() } else { setting_words.join(", ") }),
        format!("With them it makes {} pieces ({} room points, {} yard points). Nothing is placed now.", c.pieces, c.room_points, c.yard_points),
    ];
    let free = free_name(kit, &name)?;
    let file = format!("{free}.recipe.{}", if lua { "lua" } else { "json" });
    if free != name {
        lines.push(format!("A recipe named \"{name}\" is there already: this one is saved as \"{free}\", next to it."));
    }
    lines.push(format!("Saved in your building library: state\\dens\\recipes\\{file}"));
    Ok((file, text, lua, Card { title: format!("Save the recipe \"{name}\" to your building library"), lines }))
}

/// A recipe name nothing in the library has yet: `name`, else `name-2`, `name-3`...
fn free_name(kit: &Path, name: &str) -> Result<String, String> {
    let (lib, _) = library(Some(kit));
    let dir = recipes_dir(kit);
    for i in 1..100 {
        let n = if i == 1 { name.to_string() } else { format!("{}-{i}", &name[..name.len().min(21)]) };
        let there = lib.iter().any(|r| r.name == n) || ["json", "lua"].iter().any(|e| dir.join(format!("{n}.recipe.{e}")).exists());
        if !there {
            return Ok(n);
        }
    }
    Err("Too many recipes with that name.".into())
}

/// Does an approved Den request (the launcher, after the player's click; describe ran again just before).
pub fn run(app_dir: &Path, tool: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match tool {
        "den_place_layout" => den_kit(app_dir).and_then(|k| {
            let (c, label, name, mode, _) = plan_place(&k, args)?;
            let text = serde_json::to_string_pretty(&c.doc).map_err(|e| e.to_string())? + "\n";
            let saved = dens::add(&k, &format!("{name}.den.json"), &text)?;
            if mode == "replace" {
                dens::open(&k, &saved)?;
                Ok(format!("Saved as the Den file \"{saved}\" and opening it as the whole Den (a copy of what was there is kept as \"backup\"). It opens when the player is in their Den."))
            } else {
                let id = format!("{:x}{:x}", super::now(), std::process::id());
                let b = serde_json::to_vec(&json!({"file": saved, "id": id, "label": label})).map_err(|e| e.to_string())?;
                crate::settings::write_atomic(&dens::dir(&k).join(PLACE_FILE), &b).map_err(|e| format!("Couldn't write to the Den's folder: {e}"))?;
                Ok(format!("Saved as the Den file \"{saved}\"; the Den adds its {} pieces while the player is in it, and the companion says how far along it is. den_status shows it too.", c.pieces))
            }
        }),
        "den_say" => den_kit(app_dir).and_then(|k| {
            let t = say_text(args)?;
            let n = push_line(&k, &t)?;
            Ok(format!("Line {n} is waiting for the realm: the companion says it within a few seconds when it's out."))
        }),
        "den_recipe_save" => den_kit(app_dir).and_then(|k| {
            let (file, text, lua, _) = plan_recipe(&k, args)?;
            let p = recipes_dir(&k).join(&file);
            if p.exists() {
                return Err("A recipe with that name was just saved: ask again.".into());
            }
            // the name inside a template is the file's name (a script keeps its own; the file name wins on reading)
            let text = if lua { text } else {
                let mut v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
                v["name"] = json!(file.trim_end_matches(".recipe.json"));
                serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?
            };
            crate::settings::write_atomic(&p, text.as_bytes()).map_err(|e| format!("Couldn't save it: {e}"))?;
            Ok(format!("Saved in the building library as {file}. den_recipes lists it; den_place_layout uses it by name."))
        }),
        _ => return None,
    })
}

/// A read tool's answer (None: not a Den tool).
pub fn read(app_dir: &Path, name: &str, args: &Value) -> Option<Result<Value, String>> {
    Some(match name {
        "den_status" => den_kit(app_dir).map(|k| status(&k)),
        "den_catalog" => den_kit(app_dir).and_then(|k| catalog_view(&k, args)),
        "den_recipes" => den_kit(app_dir).and_then(|k| recipes_view(&k, args)),
        "den_messages" => den_kit(app_dir).and_then(|k| messages(&k, args)),
        _ => return None,
    })
}

const HOW: &str = "Positions are yards from the room's middle: x and y on the floor (the walls stand at x and y = +-half; the \
doorway is in the middle of the wall at y = -half), z is height above the floor. yaw is degrees. Inside the walls counts \
against the room's points, outside (the yard, up to r yards from the middle) against the yard's.";

fn status(kit: &Path) -> Value {
    let lim = limits(kit);
    let set_up = kit.join("state").join("host.json").is_file();
    let (den_realm, running) = if set_up {
        let c = crate::host::console::console(kit);
        (c.profile == "den", c.services.iter().find(|s| s.role == "world").map(|s| s.state == "ok"))
    } else {
        (false, None)
    };
    let cin = companion_in(kit);
    let fresh = cin.as_ref().is_some_and(is_fresh);
    let pick = |k: &str| cin.as_ref().and_then(|v| v.get(k)).cloned().unwrap_or(Value::Null);
    let seen = out_doc(kit).get("seen").and_then(Value::as_u64).unwrap_or(0);
    let unread = asks(kit).iter().filter(|a| a["n"].as_u64().unwrap_or(0) > seen).count();
    let files: Vec<Value> = dens::list(kit).into_iter().take(50).map(|f| json!({"name": f.name, "props": f.props, "saved": f.saved})).collect();
    json!({
        "den_set_up": set_up, "den_realm": den_realm, "realm_running": running,
        "companion": {"script_on": cin.is_some(), "realm_heard_from": fresh, "out": fresh && pick("companion") == json!(true),
            "unread_messages": unread, "said_up_to": pick("said")},
        "owner": if fresh { pick("owner") } else { Value::Null },
        "room": pick("room"),
        "building": pick("placing"),
        "waiting": {"build": dens::dir(kit).join(PLACE_FILE).exists(), "open": dens::dir(kit).join("open.json").exists()},
        "saved_dens": files,
        "limits": lim,
        "how": HOW,
    })
}

fn catalog_view(kit: &Path, args: &Value) -> Result<Value, String> {
    let cat = catalog(kit)?;
    let want = arg_str(args, "category").map(str::to_ascii_lowercase);
    let q = arg_str(args, "query").map(str::to_lowercase);
    let lim = limits(kit);
    if want.is_none() && q.is_none() {
        let cats: Vec<Value> = cat.categories.iter().map(|c| {
            let in_c: Vec<&Entry> = cat.entries.iter().filter(|e| &e.cat == c).collect();
            json!({"category": c, "count": in_c.len(), "points": in_c.first().map(|e| e.points),
                "examples": in_c.iter().take(8).map(|e| e.key.clone()).collect::<Vec<_>>()})
        }).collect();
        return Ok(json!({"categories": cats, "stamps": stamps(kit), "limits": lim, "how": HOW,
            "next": "den_catalog with a category (or a query) lists its pieces: key, entry, name, points."}));
    }
    let hits: Vec<Value> = cat.entries.iter()
        .filter(|e| want.as_deref().map_or(true, |c| e.cat == c))
        .filter(|e| q.as_deref().map_or(true, |q| e.key.contains(q) || e.name.to_lowercase().contains(q)))
        .take(200)
        .map(|e| json!({"key": e.key, "entry": e.entry, "name": e.name, "category": e.cat, "points": e.points}))
        .collect();
    Ok(json!({"pieces": hits, "limits": lim}))
}

fn recipes_view(kit: &Path, args: &Value) -> Result<Value, String> {
    let (lib, problems) = library(Some(kit));
    let Some(name) = arg_str(args, "name").map(str::to_ascii_lowercase) else {
        return Ok(json!({"recipes": lib.iter().map(Recipe::brief).collect::<Vec<_>>(), "problems": problems,
            "folder": "state\\dens\\recipes (in the player's Host pack folder)",
            "next": "den_recipes with a name (and params) shows its layout; den_place_layout with recipe and params builds it, \
                with no layout to write; den_recipe_save adds a new recipe."}));
    };
    let r = lib.iter().find(|r| r.name == name).ok_or(format!("There is no recipe \"{}\".", short(&name, 40)))?;
    let cat = catalog(kit)?;
    let p = settings(r, args.get("params").unwrap_or(&Value::Null), &cat)?;
    let c = check_layout(&cat, &limits(kit), &json!({"props": run_recipe(r, &p)?}), &r.name, None)?;
    Ok(json!({"recipe": r.brief(), "pieces": c.pieces, "room_points": c.room_points, "yard_points": c.yard_points,
        "kinds": c.kinds.iter().map(|(n, k)| json!({"name": n, "count": k})).collect::<Vec<_>>(), "layout": c.doc}))
}

fn messages(kit: &Path, args: &Value) -> Result<Value, String> {
    let all = asks(kit);
    let mut out = out_doc(kit);
    let seen = out.get("seen").and_then(Value::as_u64).unwrap_or(0);
    let after = args.get("after").and_then(Value::as_u64).unwrap_or(seen);
    let list: Vec<Value> = all.iter().filter(|a| a["n"].as_u64().unwrap_or(0) > after).cloned().collect();
    let latest = all.iter().filter_map(|a| a["n"].as_u64()).max().unwrap_or(0);
    // the realm sees someone is listening (read_at), and the companion knows these were heard
    out["seen"] = json!(latest.max(seen));
    out["read_at"] = json!(super::now());
    let _ = write_out(kit, &out);
    let cin = companion_in(kit);
    Ok(json!({"messages": list, "latest": latest,
        "note": "The player's own words to their companion in game: what they'd like. Answer with den_say; build with den_place_layout. \
            `where` is where the player stood (yards from the room's middle; yaw is the way they faced).",
        "companion_out": cin.as_ref().is_some_and(is_fresh) && cin.as_ref().and_then(|v| v.get("companion")) == Some(&json!(true))}))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Read at test time, not include_str!: the public Leyline export leaves realm/ out.
    fn repo_file(rel: &str) -> &'static str {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
        Box::leak(fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())).into_boxed_str())
    }
    #[allow(non_snake_case)]
    fn OV_BUILD() -> &'static str { repo_file("realm/custom/lua_scripts/ov_build.lua") }
    #[allow(non_snake_case)]
    fn DEN_ENV() -> &'static str { repo_file("realm/profiles/den.env") }

    /// A kit with the repo's realm scripts, stamps and den profile.
    pub(crate) fn kit_at(k: &Path) {
        let ls = k.join("realm-files").join("lua_scripts");
        fs::create_dir_all(ls.join("den-stamps")).unwrap();
        fs::write(ls.join("ov_build.lua"), OV_BUILD()).unwrap();
        let stamps = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../realm/custom/lua_scripts/den-stamps");
        for e in fs::read_dir(stamps).unwrap().flatten() {
            fs::copy(e.path(), ls.join("den-stamps").join(e.file_name())).unwrap();
        }
        fs::create_dir_all(k.join("realm-files").join("profiles")).unwrap();
        fs::write(k.join("realm-files").join("profiles").join("den.env"), DEN_ENV()).unwrap();
        fs::create_dir_all(dens::dir(k)).unwrap();
    }

    #[test]
    fn the_catalog_from_the_realm_script() {
        let c = parse_catalog(OV_BUILD());
        assert!(c.entries.len() > 350, "{}", c.entries.len());
        let lantern = c.key("lantern").unwrap();
        assert_eq!((lantern.entry, lantern.cat.as_str(), lantern.points), (179977, "lights", 2));
        assert_eq!(c.key("chair").unwrap().points, 1);
        assert_eq!(c.key("rug").unwrap().points, 1, "BIG_COST: rugs are flat");
        assert_eq!(c.key("thegreatforge").unwrap().points, 6);
        assert_eq!(c.key("tavern").map(|e| (e.entry, e.cat.as_str(), e.points)), Some((19367, "buildings", 10)));
        assert!(c.categories.iter().any(|x| x == "buildings") && c.categories.iter().any(|x| x == "seating"));
        assert!(c.id(19007).is_some(), "the inn stamp's building");
        // every stamp's pieces are catalog entries
        for s in ["inn", "garden", "market", "campfire", "dock", "house"] {
            let t = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../realm/custom/lua_scripts/den-stamps/{s}.den.json"))).unwrap();
            let v: Value = serde_json::from_str(&t).unwrap();
            for p in v["props"].as_array().unwrap() {
                assert!(c.id(p["entry"].as_u64().unwrap()).is_some(), "{s}: {}", p["entry"]);
            }
        }
    }

    #[test]
    fn limits_from_the_den_profile() {
        let d = tempfile::tempdir().unwrap();
        kit_at(d.path());
        let l = limits(d.path());
        assert_eq!((l.half, l.r, l.room_points, l.yard_points, l.scale_min, l.scale_max), (16.0, 30.0, 1000, 300, 0.1, 5.0));
        fs::write(d.path().join("state").join("realm.env"), "OV_DEN_SIZE=99\nOV_DEN_OUTSIDE_POINTS=50\n").unwrap();
        let l = limits(d.path());
        assert_eq!((l.half, l.yard_points), (32.0, 50), "the host's own settings win, clamped like the realm");
    }

    #[test]
    fn expressions() {
        let mut env = HashMap::new();
        env.insert("n".to_string(), 4.0);
        let ev = |s: &str| eval(s, &env, 1.0);
        assert_eq!(ev("1 + 2 * 3").unwrap(), 7.0);
        assert_eq!(ev("(1 + 2) * 3 - -1").unwrap(), 10.0);
        assert_eq!(ev("2 ^ 3 ^ 2").unwrap(), 512.0);
        assert_eq!(ev("-7 % 3").unwrap(), 2.0, "Lua's modulo");
        assert!((ev("cos(180)").unwrap() + 1.0).abs() < 1e-9);
        assert!((ev("atan2(1, 0)").unwrap() - 90.0).abs() < 1e-9);
        assert_eq!(ev("n > 3 && n < 5 ? 1 : 2").unwrap(), 1.0);
        assert_eq!(ev("!(n == 4) || n != 4").unwrap(), 0.0);
        assert_eq!(ev("max(1, n, 3) + min(2, 9) + clamp(12, 0, 10)").unwrap(), 16.0);
        let r = ev("rand(3, 4)").unwrap();
        assert!((0.0..1.0).contains(&r) && r == ev("rand(3, 4)").unwrap() && r != ev("rand(4, 3)").unwrap());
        for bad in ["1 / 0", "nope + 1", "os(1)", "1 +", "(1", "1 = 1", "sqrt(-1)", "a.b", "1 ? 2"] {
            assert!(ev(bad).is_err(), "{bad}");
        }
        assert!(ev(&"(".repeat(100)).is_err());
    }

    #[test]
    fn checking_a_layout() {
        let d = tempfile::tempdir().unwrap();
        kit_at(d.path());
        let (cat, lim) = (catalog(d.path()).unwrap(), limits(d.path()));
        let ok = json!({"props": [{"key": "Table", "x": 1, "y": 2}, {"entry": 2413, "x": 2, "y": 2, "yaw": -90, "scale": 1.5},
            {"key": "lantern", "x": 20, "y": 0, "z": 0.5}]});
        let c = check_layout(&cat, &lim, &ok, "t", None).unwrap();
        assert_eq!((c.pieces, c.room_points, c.yard_points), (3, 2, 2), "the lantern at 20 yards is in the yard");
        assert_eq!(c.doc["props"][0], json!({"entry": 181075, "x": 1.0, "y": 2.0, "z": 0.0}));
        assert_eq!(c.doc["props"][1]["yaw"], 270.0);
        assert_eq!(c.doc["props"][1]["scale"], 1.5);
        assert_eq!(c.doc["format"], "leyline-den");
        assert!(dens::check(&c.doc.to_string()).is_ok(), "a Den file the launcher and the realm read");
        let bad = |props: Value| check_layout(&cat, &lim, &json!({"props": props}), "t", None).unwrap_err();
        assert!(bad(json!([{"key": "dragon", "x": 0, "y": 0}])).contains("isn't in the Den catalog"));
        assert!(bad(json!([{"entry": 1, "x": 0, "y": 0}])).contains("isn't in the Den catalog"));
        assert!(bad(json!([{"key": "chair", "x": 40, "y": 0}])).contains("outside the Den"));
        assert!(bad(json!([{"key": "chair", "x": 0, "y": 0, "z": 99}])).contains("height"));
        assert!(bad(json!([{"key": "chair", "x": 0, "y": 0, "scale": 9}])).contains("size"));
        assert!(bad(json!([{"key": "chair", "x": "near", "y": 0}])).contains("number"));
        assert!(bad(json!([{"key": "chair"}])).contains("no x"));
        assert!(bad(json!([])).contains("no pieces"));
        let many: Vec<Value> = (0..401).map(|_| json!({"key": "chair", "x": 0, "y": 0})).collect();
        assert!(bad(Value::Array(many)).contains("up to 400"));
        // the yard's budget: 300 points; buildings cost 10
        let houses: Vec<Value> = (0..31).map(|i| json!({"key": "house", "x": 25, "y": (i % 5) as f64 - 2.0})).collect();
        assert!(bad(Value::Array(houses)).contains("yard's 300 decor points"));
        let e = check_layout(&cat, &lim, &json!({"props": [{"key": "chair", "x": 0, "y": 0}]}), "t", Some((1000, 0))).unwrap_err();
        assert!(e.contains("(1000 in use)"), "{e}");
        assert!(check_layout(&cat, &lim, &json!({"props": [{"key": "chair", "x": 0, "y": 0}], "paths": [{}]}), "t", None).is_err());
    }

    #[test]
    fn the_starter_recipes_fit_a_den() {
        let d = tempfile::tempdir().unwrap();
        kit_at(d.path());
        let (cat, lim) = (catalog(d.path()).unwrap(), limits(d.path()));
        let (lib, problems) = library(Some(d.path()));
        assert!(problems.is_empty(), "{problems:?}");
        let names: Vec<&str> = lib.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["garden-scatter", "lantern-ring", "market-row", "path", "tavern"]);
        for r in &lib {
            let p = settings(r, &Value::Null, &cat).unwrap();
            let props = run_recipe(r, &p).unwrap();
            let c = check_layout(&cat, &lim, &json!({"props": props}), &r.name, None).unwrap_or_else(|e| panic!("{}: {e}", r.name));
            assert!(c.pieces >= 6, "{}: {}", r.name, c.pieces);
            assert!(!r.about.is_empty() && !r.title.is_empty());
            for w in [&r.title, &r.about] {
                assert!(brand_word(w).is_none());
            }
        }
        let get = |n: &str| lib.iter().find(|r| r.name == n).unwrap();
        let ring = |params: Value| run_recipe(get("lantern-ring"), &settings(get("lantern-ring"), &params, &cat).unwrap()).unwrap();
        let r = ring(json!({"count": 4, "radius": 2, "fire": 1, "light": "torch"}));
        assert_eq!(r.len(), 5);
        assert_eq!(r[0]["key"], "torch");
        assert!((r[1]["y"].as_f64().unwrap() - 2.0).abs() < 1e-9 && r[1]["x"].as_f64().unwrap().abs() < 1e-9);
        assert_eq!(r[4]["key"], "campfire");
        assert!(settings(get("lantern-ring"), &json!({"count": 99}), &cat).unwrap_err().contains("outside"));
        assert!(settings(get("lantern-ring"), &json!({"count": 4.5}), &cat).unwrap_err().contains("whole"));
        assert!(settings(get("lantern-ring"), &json!({"light": "dragon"}), &cat).unwrap_err().contains("catalog"));
        assert!(settings(get("lantern-ring"), &json!({"colour": 1}), &cat).unwrap_err().contains("no setting"));
        // the tavern: 4 tables, 16 seats, 4 kegs, 4 lanterns
        let t = run_recipe(get("tavern"), &settings(get("tavern"), &Value::Null, &cat).unwrap()).unwrap();
        assert_eq!(t.len(), 28);
        assert_eq!(t.iter().filter(|p| p["key"] == "chair").count(), 16);
        // the path: from (0,-12) to (0,-24), 1.6 yards apart: 8 stones, lights at 0 and 4
        let p = run_recipe(get("path"), &settings(get("path"), &Value::Null, &cat).unwrap()).unwrap();
        assert_eq!(p.iter().filter(|x| x["key"] == "platform").count(), 8);
        assert_eq!(p.iter().filter(|x| x["key"] == "lantern").count(), 2);
        // the garden: steady (the same settings, the same garden), another seed another garden
        let g = get("garden-scatter");
        assert_eq!(g.kind(), "template", "a template: no Lua needed in the default build");
        let a = run_recipe(g, &settings(g, &Value::Null, &cat).unwrap()).unwrap();
        assert_eq!(a, run_recipe(g, &settings(g, &Value::Null, &cat).unwrap()).unwrap());
        assert_ne!(a, run_recipe(g, &settings(g, &json!({"seed": 7}), &cat).unwrap()).unwrap());
        assert_eq!(a.iter().filter(|x| x["key"] != "woodfence2").count(), 20);
    }

    #[cfg(not(feature = "lua-recipes"))]
    #[test]
    fn without_lua_a_script_is_listed_but_refused() {
        let t = setup();
        fs::create_dir_all(recipes_dir(t.kit.path())).unwrap();
        fs::write(recipes_dir(t.kit.path()).join("garden-script.recipe.lua"), include_str!("../../den-recipes/examples/garden-scatter-script.recipe.lua")).unwrap();
        let (lib, problems) = library(Some(t.kit.path()));
        assert!(problems.is_empty(), "{problems:?}");
        let r = lib.iter().find(|r| r.name == "garden-script").expect("listed");
        assert_eq!((r.kind(), r.brief()["usable"].clone()), ("script", json!(false)));
        assert_eq!(run_recipe(r, &BTreeMap::new()).unwrap_err(), NO_SCRIPTS);
        let mut s = server(t.app.path());
        heartbeat(t.kit.path(), json!({}));
        let (ok, text) = call(&mut s, "den_place_layout", json!({"recipe": "garden-script"}));
        assert!(!ok && text == NO_SCRIPTS, "{text}");
        let (ok, text) = call(&mut s, "den_recipe_save", json!({"name": "x", "script": "recipe = {}"}));
        assert!(!ok && text.contains("aren't in this launcher build"), "{text}");
        assert!(crate::agents::pending(t.app.path()).is_empty());
    }

    #[cfg(feature = "lua-recipes")]
    #[test]
    fn the_example_script_runs() {
        let src = include_str!("../../den-recipes/examples/garden-scatter-script.recipe.lua");
        let r = parse_recipe("garden-script.recipe.lua", src, true, "yours").unwrap();
        let d = tempfile::tempdir().unwrap();
        kit_at(d.path());
        let cat = catalog(d.path()).unwrap();
        let props = run_recipe(&r, &settings(&r, &Value::Null, &cat).unwrap()).unwrap();
        assert!(check_layout(&cat, &limits(d.path()), &json!({"props": props}), "g", None).is_ok());
        assert_eq!(props, run_recipe(&r, &settings(&r, &Value::Null, &cat).unwrap()).unwrap(), "steady");
    }

    #[cfg(feature = "lua-recipes")]
    #[test]
    fn the_sandbox_holds() {
        let meta = "recipe = { name = 'x', params = {} }\n";
        for (body, why) in [
            ("function build() return io.open('/etc/passwd') end", "io"),
            ("function build() return os.execute('x') end", "os"),
            ("function build() return loadstring('return 1')() end", "loadstring"),
            ("function build() return require('x') end", "require"),
            ("function build() return dofile('x') end", "dofile"),
            ("function build() return string.rep('x', 9) end", "rep"),
            ("function build() return ('x'):find('x') end", "patterns"),
            ("function build() while true do end end", "forever"),
            ("function build() local t = {} for i = 1, 1e8 do t[i] = i end return t end", "memory"),
            ("function build() return { { x = 1, y = 2 } } end", "no key"),
        ] {
            let src = format!("{meta}{body}");
            let r = parse_recipe("x.recipe.lua", &src, true, "yours").unwrap();
            assert!(run_script(&src, &settings(&r, &Value::Null, &Catalog::default()).unwrap()).is_err(), "{why}");
        }
        assert!(parse_recipe("x.recipe.lua", "os.exit(1)", true, "yours").is_err());
        assert!(parse_recipe("x.recipe.lua", "recipe = {}", true, "yours").is_err(), "no build()");
        assert!(parse_recipe("x.recipe.lua", "\x1bLua", true, "yours").is_err(), "no compiled code");
        // math.random is steady too
        let src = "recipe = { name = 'r', params = {} }\nfunction build() return { { key = 'chair', x = math.random(10), y = math.random() } } end";
        let a = run_script(src, &BTreeMap::new()).unwrap();
        assert_eq!(a, run_script(src, &BTreeMap::new()).unwrap());
    }

    #[test]
    fn brand_words_and_clean_lines() {
        assert_eq!(clean_line("|cffff0000red|r\nnext\tline  "), "cffff0000red r next line");
        assert_eq!(brand_word("Wow, what a tavern"), Some("wow"));
        assert_eq!(brand_word("like in World of  Warcraft"), Some("world of warcraft"));
        assert_eq!(brand_word("the LICH-KING's throne"), Some("lich king"));
        assert_eq!(brand_word("a blizzard of snow"), Some("blizzard"));
        assert_eq!(brand_word("a wowza moment, wowed"), None);
        assert_eq!(brand_word("Placing the tavern: 12 of 40 pieces"), None);
    }

    // ---------------------------------------------------------------- through Leyline mcp, like an agent

    struct Setup {
        app: tempfile::TempDir,
        kit: tempfile::TempDir,
    }

    fn setup() -> Setup {
        let (app, kit) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        kit_at(kit.path());
        let mut s = crate::settings::Settings::default();
        s.host_kit = kit.path().display().to_string();
        crate::settings::save(app.path(), &s).unwrap();
        crate::agents::set_allow(app.path(), true).unwrap();
        crate::agents::beat(app.path());
        Setup { app, kit }
    }

    fn server(d: &Path) -> crate::agents::Server {
        let mut s = crate::agents::Server::new(d.to_path_buf());
        s.ask_timeout = std::time::Duration::from_millis(1500);
        s.run_timeout = std::time::Duration::from_secs(5);
        s.poll = std::time::Duration::from_millis(20);
        s
    }

    fn call(s: &mut crate::agents::Server, name: &str, args: Value) -> (bool, String) {
        let r = s.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}}).to_string()).unwrap();
        (r["result"]["isError"] == json!(false), r["result"]["content"][0]["text"].as_str().unwrap_or("").to_string())
    }

    /// The launcher's side: approve (or deny) the next card, doing the work as the launcher does.
    fn launcher(app: PathBuf, yes: bool) -> std::thread::JoinHandle<Option<crate::agents::Pending>> {
        std::thread::spawn(move || {
            for _ in 0..300 {
                if let Some(p) = crate::agents::pending(&app).into_iter().next() {
                    if yes {
                        let _ = crate::agents::approve(&app, &p.id, |p| {
                            crate::agents::describe(&app, &p.tool, &p.args)?;
                            run(&app, &p.tool, &p.args).unwrap()
                        });
                    } else {
                        crate::agents::deny(&app, &p.id).unwrap();
                    }
                    return Some(p);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            None
        })
    }

    fn heartbeat(kit: &Path, extra: Value) {
        let mut v = json!({"format": "leyline-companion", "at": crate::agents::now(), "companion": true,
            "room": {"props": 3, "room_points": 10, "room_cap": 1000, "yard_points": 0, "yard_cap": 300, "half": 16}, "asks": []});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        fs::write(dens::dir(kit).join(IN_FILE), v.to_string()).unwrap();
    }

    #[test]
    fn reading_the_den() {
        let t = setup();
        let mut s = server(t.app.path());
        let (ok, text) = call(&mut s, "den_status", json!({}));
        assert!(ok, "{text}");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["companion"]["script_on"], false);
        assert_eq!(v["limits"]["half"], 16.0);
        heartbeat(t.kit.path(), json!({"owner": {"inside": true, "x": 1.5, "y": -2, "z": 0, "yaw": 90}}));
        let v: Value = serde_json::from_str(&call(&mut s, "den_status", json!({})).1).unwrap();
        assert_eq!((v["companion"]["out"].clone(), v["owner"]["x"].clone(), v["room"]["room_points"].clone()), (json!(true), json!(1.5), json!(10)));
        let v: Value = serde_json::from_str(&call(&mut s, "den_catalog", json!({})).1).unwrap();
        assert!(v["categories"].as_array().unwrap().iter().any(|c| c["category"] == "lights"));
        assert!(v["stamps"].as_array().unwrap().iter().any(|st| st["name"] == "inn" && st["pieces"].as_u64().unwrap() > 5));
        let v: Value = serde_json::from_str(&call(&mut s, "den_catalog", json!({"query": "lantern"})).1).unwrap();
        assert!(v["pieces"].as_array().unwrap().iter().any(|p| p["key"] == "lantern" && p["points"] == 2));
        let v: Value = serde_json::from_str(&call(&mut s, "den_recipes", json!({})).1).unwrap();
        assert_eq!(v["recipes"].as_array().unwrap().len(), 5);
        let v: Value = serde_json::from_str(&call(&mut s, "den_recipes", json!({"name": "market-row", "params": {"stalls": 2}})).1).unwrap();
        assert_eq!(v["layout"]["format"], "leyline-den");
        assert_eq!(v["pieces"], 2 + 2 + 4 + 1);
        let (ok, text) = call(&mut s, "den_recipes", json!({"name": "nope"}));
        assert!(!ok && text.contains("no recipe"));
        // no kit: a plain answer
        crate::settings::save(t.app.path(), &crate::settings::Settings::default()).unwrap();
        let (ok, text) = call(&mut s, "den_status", json!({}));
        assert!(!ok && text.contains("no Den on this PC"), "{text}");
    }

    #[test]
    fn building_a_recipe_through_the_card() {
        let t = setup();
        let mut s = server(t.app.path());
        let args = json!({"recipe": "lantern-ring", "params": {"count": 6, "fire": 1}});
        let (ok, text) = call(&mut s, "den_place_layout", args.clone());
        assert!(!ok && text.contains("companion script"), "adding needs the realm's companion script: {text}");
        heartbeat(t.kit.path(), json!({}));
        let h = launcher(t.app.path().to_path_buf(), true);
        let (ok, text) = call(&mut s, "den_place_layout", args.clone());
        let p = h.join().unwrap().expect("a card");
        assert!(ok, "{text}");
        assert_eq!(p.card.title, "Build the lantern ring in your Den");
        assert!(p.card.lines[0].starts_with("7 pieces: 6 Lantern, 1 Campfire"), "{:?}", p.card.lines);
        assert!(p.card.lines.iter().any(|l| l.contains("(10 and 0 in use now)")), "{:?}", p.card.lines);
        assert!(p.card.lines.iter().any(|l| l.contains("recipe \"Lantern ring\"") && l.contains("count 6")));
        let file = dens::text(t.kit.path(), "lantern-ring").unwrap();
        let v: Value = serde_json::from_str(&file).unwrap();
        assert_eq!(v["props"].as_array().unwrap().len(), 7);
        let place: Value = serde_json::from_str(&fs::read_to_string(dens::dir(t.kit.path()).join(PLACE_FILE)).unwrap()).unwrap();
        assert_eq!((place["file"].clone(), place["label"].clone()), (json!("lantern-ring"), json!("the lantern ring")));
        // the realm hasn't taken it yet: one at a time
        let (ok, text) = call(&mut s, "den_place_layout", args.clone());
        assert!(!ok && text.contains("still waiting"), "{text}");
        fs::remove_file(dens::dir(t.kit.path()).join(PLACE_FILE)).unwrap();
        // a second one never writes over the first file
        let h = launcher(t.app.path().to_path_buf(), true);
        assert!(call(&mut s, "den_place_layout", args).0);
        h.join().unwrap();
        assert!(dens::text(t.kit.path(), "lantern-ring-2").is_ok());
        fs::remove_file(dens::dir(t.kit.path()).join(PLACE_FILE)).unwrap(); // the realm took it
        // replace: its own layout, through ov_den.lua's open.json
        let h = launcher(t.app.path().to_path_buf(), true);
        let (ok, text) = call(&mut s, "den_place_layout", json!({"mode": "replace", "name": "Cosy", "layout": {"props": [{"key": "bench", "x": 0, "y": 3}]}}));
        let p = h.join().unwrap().unwrap();
        assert!(ok, "{text}");
        assert!(p.card.lines.iter().any(|l| l.contains("Replaces everything")));
        let open: Value = serde_json::from_str(&fs::read_to_string(dens::dir(t.kit.path()).join("open.json")).unwrap()).unwrap();
        assert_eq!(open["file"], "cosy");
        // a bad layout never makes a card
        let (ok, text) = call(&mut s, "den_place_layout", json!({"layout": {"props": [{"key": "dragon", "x": 0, "y": 0}]}}));
        assert!(!ok && text.contains("catalog"));
        assert_eq!(crate::agents::read_audit(t.app.path(), 1)[0].outcome, "refused");
        // denied: nothing written
        let h = launcher(t.app.path().to_path_buf(), false);
        let (ok, _) = call(&mut s, "den_place_layout", json!({"name": "nope", "layout": {"props": [{"key": "bench", "x": 0, "y": 3}]}}));
        h.join().unwrap();
        assert!(!ok && dens::text(t.kit.path(), "nope").is_err() && !dens::dir(t.kit.path()).join(PLACE_FILE).exists());
    }

    #[test]
    fn talking_through_the_companion() {
        let t = setup();
        let mut s = server(t.app.path());
        let (ok, text) = call(&mut s, "den_say", json!({"text": "Wow, look at it"}));
        assert!(!ok && text.contains("never says"), "{text}");
        heartbeat(t.kit.path(), json!({"asks": [
            {"n": 1, "at": 5, "text": "Jaina, put a |cffff0000well|r here", "where": {"x": 3.14159, "y": -2, "z": 0, "yaw": 180, "evil": "x"}},
            {"n": 2, "at": 6, "text": "and a bench"}]}));
        let v: Value = serde_json::from_str(&call(&mut s, "den_messages", json!({})).1).unwrap();
        assert_eq!(v["messages"].as_array().unwrap().len(), 2);
        assert_eq!(v["messages"][0]["text"], "Jaina, put a cffff0000well r here");
        assert_eq!(v["messages"][0]["where"], json!({"x": 3.1, "y": -2.0, "z": 0.0, "yaw": 180.0}));
        let v: Value = serde_json::from_str(&call(&mut s, "den_messages", json!({})).1).unwrap();
        assert_eq!(v["messages"], json!([]), "read once");
        let v: Value = serde_json::from_str(&call(&mut s, "den_messages", json!({"after": 1})).1).unwrap();
        assert_eq!(v["messages"][0]["n"], 2);
        let out: Value = serde_json::from_str(&fs::read_to_string(dens::dir(t.kit.path()).join(OUT_FILE)).unwrap()).unwrap();
        assert_eq!(out["seen"], 2);
        assert!(out["read_at"].as_u64().unwrap() > 0, "the realm sees someone is listening");
        let h = launcher(t.app.path().to_path_buf(), true);
        let (ok, text) = call(&mut s, "den_say", json!({"text": "A well, here?\nI'll draw one up."}));
        let p = h.join().unwrap().unwrap();
        assert!(ok, "{text}");
        assert_eq!(p.card.title, "Your companion says a line in your Den");
        assert_eq!(p.card.lines[0], "\"A well, here? I'll draw one up.\"");
        for i in 0..25 {
            push_line(t.kit.path(), &format!("line {i}")).unwrap();
        }
        let out: Value = serde_json::from_str(&fs::read_to_string(dens::dir(t.kit.path()).join(OUT_FILE)).unwrap()).unwrap();
        let lines = out["lines"].as_array().unwrap();
        assert_eq!(lines.len(), LINES_KEEP);
        assert_eq!(lines.last().unwrap()["n"], 26);
        assert_eq!(out["seen"], 2, "the read mark stays");
    }

    #[test]
    fn saving_a_recipe() {
        let t = setup();
        let mut s = server(t.app.path());
        let row = json!({"title": "Bench row", "params": {"n": {"default": 5, "min": 1, "max": 20, "whole": true}},
            "pieces": [{"repeat": [["i", "n"]], "key": "bench", "x": "i * 2", "y": 4}]});
        let h = launcher(t.app.path().to_path_buf(), true);
        let (ok, text) = call(&mut s, "den_recipe_save", json!({"name": "bench-row", "recipe": row.clone()}));
        let p = h.join().unwrap().unwrap();
        assert!(ok, "{text}");
        assert!(p.card.lines.iter().any(|l| l.contains("makes 5 pieces")), "{:?}", p.card.lines);
        assert!(recipes_dir(t.kit.path()).join("bench-row.recipe.json").is_file());
        // the same name as a built-in: saved next to it, never over it (a script, with Lua in the build)
        let two = json!({"title": "Two chairs", "pieces": [{"key": "chair", "x": 0, "y": 0}, {"key": "chair", "x": 1, "y": 0}]});
        let script = "recipe = { title = 'Two chairs', params = {} }\nfunction build(p) return { { key = 'chair', x = 0, y = 0 }, { key = 'chair', x = 1, y = 0 } } end";
        let args = if SCRIPTS { json!({"name": "tavern", "script": script}) } else { json!({"name": "tavern", "recipe": two}) };
        let h = launcher(t.app.path().to_path_buf(), true);
        let (ok, text) = call(&mut s, "den_recipe_save", args);
        let p = h.join().unwrap().unwrap();
        assert!(ok, "{text}");
        assert!(p.card.lines.iter().any(|l| l.contains("saved as \"tavern-2\"")), "{:?}", p.card.lines);
        let (lib, _) = library(Some(t.kit.path()));
        assert!(lib.iter().any(|r| r.name == "tavern" && r.source == "built in"));
        assert!(lib.iter().any(|r| r.name == "tavern-2" && r.kind() == if SCRIPTS { "script" } else { "template" }));
        // and it builds by name, no layout written
        heartbeat(t.kit.path(), json!({}));
        let h = launcher(t.app.path().to_path_buf(), true);
        assert!(call(&mut s, "den_place_layout", json!({"recipe": "bench-row", "params": {"n": 3}})).0);
        h.join().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&dens::text(t.kit.path(), "bench-row").unwrap()).unwrap()["props"].as_array().unwrap().len(), 3);
        // broken ones never get a card
        for bad in [json!({"name": "x", "recipe": {"pieces": [{"key": "dragon", "x": 0, "y": 0}]}}),
            json!({"name": "x", "script": "while true do end"}), json!({"name": "x"}), json!({"name": "../x", "recipe": row})] {
            assert!(!call(&mut s, "den_recipe_save", bad.clone()).0, "{bad}");
        }
    }
}
