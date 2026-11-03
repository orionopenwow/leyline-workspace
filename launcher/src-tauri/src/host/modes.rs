//! Modes anyone can host and fork (docs/MODES.md, "Mode packages"): the Host page's first screen.
//!
//! A mode package is a folder with `mode.json` (what the mode is, the ruleset it starts from, the realm scripts it
//! uses by name, the env it sets, its client mods, an optional Den file, and a form of settings) and its picture. The
//! templates are built in (`stream/modepacks.json`, made by `stream/modepack.py --write` from `hosting/kit/modes`);
//! a newer Host pack's `modes/` folder adds or replaces them; the player's own forks live in the kit's
//! `state/modes/<id>/` (kept when the Host pack updates).
//!
//! Hosting a mode (`use_mode`) checks the host's answers against its form and writes `state/mode.json`: the mode's
//! recipe (its settings' defaults set to the answers, so a fork of this realm starts from them) plus `resolved`, the
//! env the world server gets. The kit's setup.py puts that env in `state/realm.env` and the recipe on the realm list
//! (`read_mode`). A Den file goes into the Den's saves and opens there (`dens`). The UI then starts the realm with
//! the mode's ruleset, as for any ruleset (`host_choose`, `host_start_quiet`).
//!
//! The rules are stream/modepack.py's (the tests check the patterns match): never a script of a package's own, only
//! script names the Host pack has; env only the realm scripts' OV_ switches and a few rates, nothing about the server.

use super::dens;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The templates' catalog, built in.
pub const CATALOG: &str = include_str!("../../../../stream/modepacks.json");
pub const FORMAT: &str = "leyline-mode";
pub const PROFILES: &[&str] = &["main", "brawl", "progression", "casual", "dalaran", "vanillaplus", "den", "solo"];
pub const TAGS: &[&str] = &["blizzlike", "boosted", "custom", "pvp", "rp", "hangout", "bots"];
/// The templates (stream/modepack.py ORDER) and their author, the one name a player can't take.
pub const TEMPLATES: &[&str] = &["levelling", "solo-leveling", "duel-ladder", "hide-and-seek", "minigolf-night", "den", "survivors"];
pub const TEAM: &str = "Leyline";
pub const PICTURE_MAX: usize = 256 * 1024;
const PICTURES: &[(&str, &str)] = &[("picture.svg", "image/svg+xml"), ("picture.webp", "image/webp"), ("picture.png", "image/png")];
const ENV_DENY: &[&str] = &["DATABASE", "PASS", "SECRET", "TOKEN", "KEY", "SOAP", "WEBHOOK", "DISCORD", "URL", "FILE", "DIR",
    "PATH", "SAVES", "TEST", "HOST", "LEYLINE", "REALM_NAME", "INVITE", "BRAND", "ADDRESS", "PORT", "BIND"];
const AC_PREFIXES: &[&str] = &["AC_RATE_", "AC_SKILL_GAIN_", "AC_START_", "AC_AUTO_BALANCE_"];
const BRANDS: &[&str] = &["world of warcraft", "warcraft", "lich king", "lichking", "blizzard"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BasedOn {
    pub id: String,
    pub author: String,
}

/// One mode as the Host page shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mode {
    pub id: String,
    pub name: String,
    pub line: String,
    pub version: String,
    pub author: String,
    pub based_on: Option<BasedOn>,
    /// `live` or `coming` (shown, never hosted or forked).
    pub status: String,
    pub profile: Option<String>,
    pub scripts: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub mods: Vec<String>,
    pub den: Option<String>,
    pub tags: Vec<String>,
    pub jam: Option<String>,
    pub credit: Option<Value>,
    /// The form: checked fields (`check_setting`), as JSON for the page.
    pub settings: Vec<Value>,
    /// The picture as a data: URL (the page's CSP takes data: images), or None.
    pub picture: Option<String>,
    /// A fork in this Host pack (it can be changed and removed); else a template.
    pub own: bool,
}

// ---------------------------------------------------------------------------------------------- the rules
pub fn id_ok(s: &str) -> bool {
    let b = s.as_bytes();
    (2..=32).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && !s.ends_with('-')
        && !s.contains("--")
}

pub fn env_name_ok(k: &str) -> bool {
    let up = |rest: &str| (2..=40).contains(&rest.len()) && rest.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_');
    let shape = if let Some(r) = k.strip_prefix("OV_") {
        up(r)
    } else {
        AC_PREFIXES.iter().any(|p| k.strip_prefix(p).is_some_and(up))
    };
    shape && !ENV_DENY.iter().any(|d| k.contains(d))
}

pub fn env_value_ok(v: &str) -> bool {
    (1..=24).contains(&v.len()) && v.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

pub fn script_ok(s: &str) -> bool {
    s.strip_prefix("ov_").and_then(|r| r.strip_suffix(".lua")).is_some_and(|n| {
        (1..=32).contains(&n.len()) && n.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    })
}

/// A realm mod id (realms.rs mod_id_ok: mod:, up:, mode:).
pub fn mod_ok(s: &str) -> bool {
    crate::realms::mod_id_ok(s)
}

fn version_ok(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    (1..=4).contains(&parts.len()) && parts.iter().all(|p| (1..=4).contains(&p.len()) && p.bytes().all(|c| c.is_ascii_digit()))
}

fn key_ok(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=24).contains(&b.len()) && b[0].is_ascii_lowercase() && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
}

fn den_name_ok(s: &str) -> bool {
    s.strip_suffix(".den.json").is_some_and(|n| dens::clean_name(n).as_deref() == Some(n))
}

/// One line of plain text, at most n characters, with no game or publisher name; None otherwise.
pub fn text(v: Option<&Value>, n: usize) -> Option<String> {
    let s = v?.as_str()?.split_whitespace().collect::<Vec<_>>().join(" ");
    let low = s.to_ascii_lowercase();
    if s.is_empty() || s.chars().count() > n || s.chars().any(char::is_control) {
        return None;
    }
    if BRANDS.iter().any(|b| low.contains(b)) || low.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "wow") {
        return None;
    }
    Some(s)
}

fn envs_of(v: Option<&Value>) -> Option<Vec<String>> {
    match v? {
        Value::String(s) => Some(vec![s.clone()]),
        Value::Array(a) if (1..=6).contains(&a.len()) => a.iter().map(|x| x.as_str().map(String::from)).collect(),
        _ => None,
    }
}

fn int(v: Option<&Value>) -> Option<i64> {
    v.and_then(Value::as_i64)
}

/// One field of a mode's form, checked; Err with a plain line.
pub fn check_setting(s: &Value, fixed: &BTreeMap<String, String>) -> Result<Value, String> {
    let key = s.get("key").and_then(Value::as_str).filter(|k| key_ok(k)).ok_or("A setting's key: a-z, 0-9 and _.")?;
    let label = text(s.get("label"), 40).ok_or(format!("Setting {key}: a label of up to 40 characters."))?;
    let envs = envs_of(s.get("env")).ok_or(format!("Setting {key}: env is a setting name or a list of up to 6."))?;
    if let Some(bad) = envs.iter().find(|k| !env_name_ok(k)) {
        return Err(format!("Setting {key}: {bad} is not a setting a mode may change."));
    }
    if let Some(dup) = envs.iter().find(|k| fixed.contains_key(*k)) {
        return Err(format!("Setting {key}: {dup} is already set by the mode."));
    }
    let mut out = json!({ "key": key, "label": label, "env": envs });
    if s.get("help").is_some_and(|h| !h.is_null()) {
        out["help"] = json!(text(s.get("help"), 120).ok_or(format!("Setting {key}: help is one line of up to 120 characters."))?);
    }
    match s.get("type").and_then(Value::as_str) {
        Some("number") => {
            let (lo, hi, d) = (int(s.get("min")), int(s.get("max")), int(s.get("default")));
            match (lo, hi, d) {
                (Some(lo), Some(hi), Some(d)) if 0 <= lo && lo <= d && d <= hi && hi <= 100_000 => {
                    out["type"] = json!("number");
                    out["min"] = json!(lo);
                    out["max"] = json!(hi);
                    out["default"] = json!(d);
                    if let Some(u) = text(s.get("unit"), 16) {
                        out["unit"] = json!(u);
                    }
                }
                _ => return Err(format!("Setting {key}: whole numbers, min <= default <= max.")),
            }
        }
        Some("toggle") => {
            let d = s.get("default").and_then(Value::as_bool).ok_or(format!("Setting {key}: on or off by default."))?;
            out["type"] = json!("toggle");
            out["default"] = json!(d);
        }
        Some("choice") => {
            let opts = s.get("options").and_then(Value::as_array).filter(|a| (2..=8).contains(&a.len()))
                .ok_or(format!("Setting {key}: 2 to 8 options."))?;
            let mut clean: Vec<Value> = Vec::new();
            for o in opts {
                let v = o.get("value").and_then(Value::as_str).filter(|v| env_value_ok(v));
                let l = text(o.get("label"), 32);
                match (v, l) {
                    (Some(v), Some(l)) if !clean.iter().any(|c| c["value"] == v) => clean.push(json!({ "value": v, "label": l })),
                    _ => return Err(format!("Setting {key}: each option has a value and a short name.")),
                }
            }
            let d = s.get("default").and_then(Value::as_str).filter(|d| clean.iter().any(|c| c["value"] == *d))
                .ok_or(format!("Setting {key}: the default is one of the options."))?;
            out["type"] = json!("choice");
            out["default"] = json!(d);
            out["options"] = Value::Array(clean);
        }
        _ => return Err(format!("Setting {key}: number, toggle or choice.")),
    }
    Ok(out)
}

/// A mode.json (or a realm's mode from the realm list), checked: the first problem as a plain line.
/// `scripts`: the realm scripts the Host pack has (None: not checked here; setup.py and the realm load only what is there).
pub fn check(v: &Value, scripts: Option<&HashSet<String>>) -> Result<Mode, String> {
    let id = v.get("id").and_then(Value::as_str).filter(|i| id_ok(i)).ok_or("The mode's id: a-z, 0-9 and dashes.")?.to_string();
    let name = text(v.get("name"), 32).ok_or("The mode's name: up to 32 letters, and no game names.")?;
    let line = text(v.get("line"), 120).unwrap_or_default();
    let author = text(v.get("author"), 32).ok_or("Who made it: up to 32 letters.")?;
    let based_on = match v.get("based_on") {
        None | Some(Value::Null) => None,
        Some(b) => {
            let bid = b.get("id").and_then(Value::as_str).filter(|i| id_ok(i) && *i != id).ok_or("based_on: the id of another mode.")?;
            Some(BasedOn { id: bid.into(), author: text(b.get("author"), 32).unwrap_or_else(|| "someone".into()) })
        }
    };
    let low = author.to_ascii_lowercase();
    let team = author == TEAM && based_on.is_none() && TEMPLATES.contains(&id.as_str());
    if !team && (low.contains("leyline") || low.split_whitespace().any(|w| ["staff", "admin", "gm", "official"].contains(&w))) {
        return Err("Who made it: pick your own name, not the team's.".into());
    }
    let version = v.get("version").and_then(Value::as_str).filter(|x| version_ok(x)).unwrap_or("1").to_string();
    let status = match v.get("status").and_then(Value::as_str) {
        None | Some("live") => "live",
        Some("coming") => "coming",
        _ => return Err("status: live or coming.".into()),
    }
    .to_string();
    let profile = match v.get("profile").and_then(Value::as_str) {
        Some(p) if PROFILES.contains(&p) => Some(p.to_string()),
        None if status == "coming" => None,
        _ => return Err(format!("The ruleset it starts from: one of {}.", PROFILES.join(", "))),
    };
    let mut sc = Vec::new();
    for s in v.get("scripts").and_then(Value::as_array).map(|a| a.as_slice()).unwrap_or(&[]).iter().take(13) {
        let s = s.as_str().filter(|s| script_ok(s)).ok_or("scripts: realm script names only (ov_<name>.lua).")?;
        if let Some(have) = scripts {
            if !have.contains(s) {
                return Err(format!("{s} is not in your Host pack's realm scripts: update the Host pack."));
            }
        }
        if !sc.iter().any(|x: &String| x == s) {
            sc.push(s.to_string());
        }
    }
    if sc.len() > 12 {
        return Err("scripts: 12 at most.".into());
    }
    let mut env = BTreeMap::new();
    if let Some(m) = v.get("env").and_then(Value::as_object) {
        if m.len() > 24 {
            return Err("env: 24 settings at most.".into());
        }
        for (k, x) in m {
            let x = x.as_str().filter(|x| env_value_ok(x));
            if !env_name_ok(k) || x.is_none() {
                return Err(format!("{k} is not a setting a mode may change."));
            }
            env.insert(k.clone(), x.unwrap_or_default().to_string());
        }
    }
    let mut mods = Vec::new();
    for m in v.get("mods").and_then(Value::as_array).map(|a| a.as_slice()).unwrap_or(&[]).iter().take(9) {
        let m = m.as_str().filter(|m| mod_ok(m)).ok_or("mods: mod ids only (mod:, up: or mode:).")?;
        if !mods.iter().any(|x: &String| x == m) {
            mods.push(m.to_string());
        }
    }
    if mods.len() > 8 {
        return Err("mods: 8 at most.".into());
    }
    let den = match v.get("den") {
        None | Some(Value::Null) => None,
        Some(d) => {
            let d = d.as_str().filter(|d| den_name_ok(d)).ok_or("den: a Den file's name, like room.den.json.")?;
            if profile.as_deref() != Some("den") {
                return Err("A Den file goes with the den ruleset only.".into());
            }
            Some(d.to_string())
        }
    };
    let tags = v.get("tags").and_then(Value::as_array)
        .map(|a| TAGS.iter().filter(|t| a.iter().any(|x| x.as_str() == Some(t))).map(|t| t.to_string()).collect())
        .unwrap_or_default();
    let jam = v.get("jam").and_then(Value::as_str).filter(|j| id_ok(j)).map(String::from);
    let credit = v.get("credit").filter(|c| c.is_object()).and_then(|c| {
        let by = text(c.get("by"), 80)?;
        let src = c.get("source").and_then(Value::as_str).filter(|s| crate::net::plain_https(s) && s.len() <= 200)?;
        let mut o = json!({ "by": by, "source": src });
        if let Some(l) = text(c.get("license"), 60) {
            o["license"] = json!(l);
        }
        Some(o)
    });
    let raw = v.get("settings").and_then(Value::as_array).map(|a| a.as_slice()).unwrap_or(&[]);
    if raw.len() > 12 {
        return Err("A mode's form has 12 settings at most.".into());
    }
    let (mut settings, mut keys, mut used): (Vec<Value>, HashSet<String>, HashSet<String>) = (vec![], HashSet::new(), HashSet::new());
    for s in raw {
        let c = check_setting(s, &env)?;
        let k = c["key"].as_str().unwrap_or_default().to_string();
        let names: Vec<String> = c["env"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default();
        if !keys.insert(k.clone()) || names.iter().any(|n| !used.insert(n.clone())) {
            return Err(format!("Setting {k}: another setting has its key or one of its names."));
        }
        settings.push(c);
    }
    Ok(Mode { id, name, line, version, author, based_on, status, profile, scripts: sc, env, mods, den, tags, jam, credit, settings,
        picture: None, own: false })
}

/// The mode as mode.json (no picture data).
pub fn manifest(m: &Mode) -> Value {
    json!({
        "format": FORMAT, "schema": 1, "id": m.id, "name": m.name, "line": m.line,
        "picture": m.picture.as_ref().map(|_| "picture.svg"), "version": m.version, "author": m.author,
        "based_on": m.based_on.as_ref().map(|b| json!({ "id": b.id, "author": b.author })), "status": m.status,
        "profile": m.profile, "scripts": m.scripts, "env": m.env, "mods": m.mods, "den": m.den, "tags": m.tags,
        "jam": m.jam, "credit": m.credit, "settings": m.settings,
    })
}

// ---------------------------------------------------------------------------------------------- the list
fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

fn svg_safe(data: &[u8]) -> bool {
    let t = String::from_utf8_lossy(data).to_ascii_lowercase();
    let head = t.trim_start();
    (head.starts_with("<svg") || head.starts_with("<?xml"))
        && !["<script", "foreignobject", "javascript:", "@import"].iter().any(|b| t.contains(b))
        && !t.split(|c: char| c.is_whitespace()).any(|w| w.starts_with("on") && w.contains('='))
        && !t.match_indices("href").any(|(i, _)| !t[i..].trim_start_matches(|c| c != '=').trim_start_matches(['=', '"', '\'', ' ']).starts_with('#'))
        && !t.match_indices("url(").any(|(i, _)| !t[i + 4..].trim_start_matches(['"', '\'', ' ']).starts_with('#'))
}

/// The picture of a package folder as a data: URL (checked), or None.
pub fn picture_in(dir: &Path) -> Option<String> {
    for (name, ty) in PICTURES {
        let p = dir.join(name);
        let Ok(meta) = fs::symlink_metadata(&p) else { continue };
        if !meta.is_file() || meta.len() as usize > PICTURE_MAX {
            continue;
        }
        let data = fs::read(&p).ok()?;
        let ok = match *name {
            "picture.svg" => svg_safe(&data),
            "picture.png" => data.starts_with(b"\x89PNG\r\n\x1a\n"),
            _ => data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP",
        };
        return ok.then(|| format!("data:{ty};base64,{}", b64(&data)));
    }
    None
}

/// The realm scripts a kit has (realm-files/lua_scripts), or None when it has none to look at.
pub fn kit_scripts(kit: &Path) -> Option<HashSet<String>> {
    let rd = fs::read_dir(kit.join("realm-files").join("lua_scripts")).ok()?;
    Some(rd.filter_map(|e| e.ok()?.file_name().into_string().ok()).filter(|n| script_ok(n)).collect())
}

pub fn forks_dir(kit: &Path) -> PathBuf {
    kit.join("state").join("modes")
}

fn read_folder(dir: &Path, scripts: Option<&HashSet<String>>, own: bool) -> Option<Mode> {
    let p = dir.join("mode.json");
    let meta = fs::symlink_metadata(&p).ok()?;
    if !meta.is_file() || meta.len() > 16 * 1024 {
        return None;
    }
    let v: Value = serde_json::from_str(fs::read_to_string(&p).ok()?.trim_start_matches('\u{feff}')).ok()?;
    if v.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return None;
    }
    let mut m = check(&v, scripts).ok()?;
    if dir.file_name().and_then(|n| n.to_str()) != Some(m.id.as_str()) {
        return None;
    }
    m.picture = picture_in(dir);
    m.own = own;
    Some(m)
}

fn builtin() -> Vec<(Mode, Option<String>)> {
    let doc: Value = serde_json::from_str(CATALOG).unwrap_or(Value::Null);
    doc.get("modes").and_then(Value::as_array).map(|a| a.as_slice()).unwrap_or(&[]).iter().filter_map(|v| {
        let mut m = check(v, None).ok()?;
        m.picture = v.get("picture_data").and_then(Value::as_str).filter(|d| d.starts_with("data:image/") && d.len() < PICTURE_MAX * 2)
            .map(String::from);
        Some((m, v.get("den_text").and_then(Value::as_str).map(String::from)))
    }).collect()
}

fn folders(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir).map(|r| r.filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())).collect()).unwrap_or_default();
    v.sort();
    v
}

/// Every mode the Host page offers: the templates (built in, or the Host pack's newer copy), then this pack's forks.
pub fn list(kit: Option<&Path>) -> Vec<Mode> {
    let scripts = kit.and_then(kit_scripts);
    let mut out: Vec<Mode> = builtin().into_iter().map(|(m, _)| m).collect();
    if let Some(k) = kit {
        for d in folders(&k.join("modes")) {
            if let Some(m) = read_folder(&d, scripts.as_ref(), false) {
                match out.iter_mut().find(|x| x.id == m.id) {
                    Some(x) => *x = m,
                    None => out.push(m),
                }
            }
        }
        for d in folders(&forks_dir(k)).into_iter().take(100) {
            if let Some(m) = read_folder(&d, scripts.as_ref(), true) {
                if !out.iter().any(|x| x.id == m.id) {
                    out.push(m);
                }
            }
        }
    }
    out
}

fn find(kit: &Path, id: &str) -> Result<Mode, String> {
    list(Some(kit)).into_iter().find(|m| m.id == id).ok_or_else(|| "That mode isn't here any more.".to_string())
}

/// The folder a mode's files are in (a fork's, or the Host pack's template), if it has one.
fn folder_of(kit: &Path, m: &Mode) -> Option<PathBuf> {
    let d = if m.own { forks_dir(kit).join(&m.id) } else { kit.join("modes").join(&m.id) };
    d.is_dir().then_some(d)
}

/// A mode's Den file text: from its folder, else the built-in catalog.
fn den_text(kit: &Path, m: &Mode) -> Option<String> {
    let name = m.den.as_ref()?;
    if let Some(d) = folder_of(kit, m) {
        if let Ok(t) = fs::read_to_string(d.join(name)) {
            return Some(t);
        }
    }
    builtin().into_iter().find(|(b, _)| b.id == m.id).and_then(|(_, t)| t)
}

fn write_folder(kit: &Path, m: &Mode, den: Option<&str>) -> Result<(), String> {
    let d = forks_dir(kit).join(&m.id);
    fs::create_dir_all(&d).map_err(|e| format!("Couldn't make the mode's folder: {e}"))?;
    let mut man = manifest(m);
    if let Some(pic) = &m.picture {
        let (ty, data) = pic.strip_prefix("data:").and_then(|r| r.split_once(";base64,")).ok_or("A broken picture.")?;
        let name = PICTURES.iter().find(|(_, t)| *t == ty).map(|(n, _)| *n).ok_or("A broken picture.")?;
        let bytes = crate::update::b64decode(data).ok_or("A broken picture.")?;
        crate::settings::write_atomic(&d.join(name), &bytes).map_err(|e| e.to_string())?;
        man["picture"] = json!(name);
    }
    if let (Some(name), Some(t)) = (&m.den, den) {
        crate::settings::write_atomic(&d.join(name), t.as_bytes()).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&man).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&d.join("mode.json"), text.as_bytes()).map_err(|e| format!("Couldn't save the mode: {e}"))
}

// ---------------------------------------------------------------------------------------------- fork, change, remove
/// "Fork this mode": a copy in this Host pack with based_on set (its id, its author), named as yours, its picture and
/// Den file kept. `listed` is a listed realm's mode (the realm list carries the recipe, never files): its picture comes
/// from the mode it is based on when that is here.
pub fn fork(kit: &Path, id: &str, listed: Option<&Value>, author: Option<&str>) -> Result<Mode, String> {
    let all = list(Some(kit));
    let (src, den) = match listed {
        Some(v) => {
            let mut m = check(v, kit_scripts(kit).as_ref())?;
            if m.id != id {
                return Err("That realm's mode doesn't match.".into());
            }
            let base = m.based_on.as_ref().and_then(|b| all.iter().find(|x| x.id == b.id)).or_else(|| all.iter().find(|x| x.id == m.id));
            m.picture = base.and_then(|b| b.picture.clone());
            m.den = None;
            (m, None)
        }
        None => {
            let m = all.iter().find(|m| m.id == id).cloned().ok_or("That mode isn't here any more.")?;
            let den = den_text(kit, &m);
            (m, den)
        }
    };
    if src.status != "live" || src.profile.is_none() {
        return Err(format!("{} isn't ready to host yet, so it can't be forked.", src.name));
    }
    let base_id: String = src.id.chars().take(26).collect::<String>().trim_end_matches('-').to_string();
    let new_id = (2..100).map(|n| format!("{base_id}-{n}")).find(|i| !all.iter().any(|m| &m.id == i))
        .ok_or("Too many copies of this mode: remove one first.")?;
    let author = author.and_then(|a| text(Some(&json!(a)), 32)).unwrap_or_else(|| "Host".into());
    let mut m = src.clone();
    m.id = new_id;
    m.name = format!("My {}", src.name).chars().take(32).collect::<String>().trim().to_string();
    m.based_on = Some(BasedOn { id: src.id.clone(), author: src.author.clone() });
    m.author = author;
    m.version = "1".into();
    m.own = true;
    m.jam = None;
    check(&manifest(&m), None)?;
    write_folder(kit, &m, den.as_deref())?;
    Ok(m)
}

/// Change a fork: its name, line, your name, the jam it is entered in, and its settings' defaults ({key: value}).
pub fn save(kit: &Path, id: &str, edits: &Value) -> Result<Mode, String> {
    let mut m = find(kit, id)?;
    if !m.own {
        return Err("A template can't be changed: fork it first.".into());
    }
    let den = den_text(kit, &m);
    let mut man = manifest(&m);
    for k in ["name", "line", "author"] {
        if let Some(v) = edits.get(k) {
            man[k] = v.clone();
        }
    }
    if let Some(j) = edits.get("jam") {
        man["jam"] = if j.as_str().is_some_and(|s| s.trim().is_empty()) { Value::Null } else { j.clone() };
        if !man["jam"].is_null() && !man["jam"].as_str().is_some_and(id_ok) {
            return Err("The jam: its short name, like first-jam.".into());
        }
    }
    if let Some(d) = edits.get("defaults").and_then(Value::as_object) {
        let vals = values_for(&m, &Value::Object(d.clone()))?;
        for s in man["settings"].as_array_mut().into_iter().flatten() {
            if let Some(v) = s["key"].as_str().and_then(|k| vals.get(k)) {
                s["default"] = v.clone();
            }
        }
    }
    let pic = m.picture.take();
    let mut n = check(&man, None)?;
    n.picture = pic;
    n.own = true;
    let vparts: Vec<u32> = m.version.split('.').filter_map(|p| p.parse().ok()).collect();
    n.version = format!("{}", vparts.first().copied().unwrap_or(1).saturating_add(1).min(9999));
    write_folder(kit, &n, den.as_deref())?;
    Ok(n)
}

pub fn remove(kit: &Path, id: &str) -> Result<(), String> {
    if !id_ok(id) {
        return Err("That isn't a mode.".into());
    }
    let d = forks_dir(kit).join(id);
    if !d.is_dir() {
        return Err("Only your own modes can be removed.".into());
    }
    fs::remove_dir_all(&d).map_err(|e| format!("Couldn't remove it: {e}"))
}

// ---------------------------------------------------------------------------------------------- host it
/// The host's answers to the mode's form, checked; a missing one takes the default.
pub fn values_for(m: &Mode, given: &Value) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for s in &m.settings {
        let key = s["key"].as_str().unwrap_or_default();
        let label = s["label"].as_str().unwrap_or(key);
        let v = given.get(key).cloned().unwrap_or_else(|| s["default"].clone());
        let ok = match s["type"].as_str() {
            Some("number") => {
                let n = v.as_i64().or_else(|| v.as_str().and_then(|t| t.trim().parse().ok()));
                match n {
                    Some(n) if n >= s["min"].as_i64().unwrap_or(0) && n <= s["max"].as_i64().unwrap_or(0) => json!(n),
                    _ => return Err(format!("{label}: {} to {}.", s["min"], s["max"])),
                }
            }
            Some("toggle") => json!(v.as_bool().ok_or(format!("{label}: on or off."))?),
            _ => {
                let opts = s["options"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                if !opts.iter().any(|o| o["value"] == v) {
                    return Err(format!("{label}: pick one."));
                }
                v
            }
        };
        out.insert(key.to_string(), ok);
    }
    Ok(out)
}

/// The env the realm gets: the mode's fixed env, then each field's names set to the host's answer.
pub fn env_for(m: &Mode, values: &Map<String, Value>) -> BTreeMap<String, String> {
    let mut env = m.env.clone();
    for s in &m.settings {
        let key = s["key"].as_str().unwrap_or_default();
        let v = values.get(key).unwrap_or(&s["default"]);
        let t = match v {
            Value::Bool(b) => if *b { "1".to_string() } else { "0".to_string() },
            Value::String(x) => x.clone(),
            other => other.to_string(),
        };
        for k in s["env"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            env.insert(k.to_string(), t.clone());
        }
    }
    env
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UseResult {
    /// The ruleset to start (host_choose).
    pub profile: String,
    /// The mode's name, for the realm name field.
    pub name: String,
    /// The Den file opened, if the mode has one.
    pub den: Option<String>,
}

pub fn mode_file(kit: &Path) -> PathBuf {
    kit.join("state").join("mode.json")
}

/// Host this mode: its answers checked, `state/mode.json` written (the kit's setup.py reads it at the start), its Den
/// file added to the saves and opened. Returns the ruleset to start.
pub fn use_mode(kit: &Path, id: &str, given: &Value) -> Result<UseResult, String> {
    let m = find(kit, id)?;
    let profile = m.profile.clone().filter(|_| m.status == "live").ok_or(format!("{} isn't ready to host yet.", m.name))?;
    let vals = values_for(&m, given)?;
    let resolved = env_for(&m, &vals);
    let mut man = manifest(&m);
    for s in man["settings"].as_array_mut().into_iter().flatten() {
        if let Some(v) = s["key"].as_str().and_then(|k| vals.get(k)) {
            s["default"] = v.clone();
        }
    }
    let o = man.as_object_mut().expect("an object");
    o.remove("picture");
    o.remove("status");
    o.remove("format");
    o.remove("schema");
    o.insert("resolved".into(), json!(resolved));
    let den = match (&m.den, den_text(kit, &m)) {
        (Some(_), Some(t)) => {
            let n = dens::add(kit, &format!("{}.den.json", m.id.chars().take(24).collect::<String>().replace('-', "_")), &t)?;
            dens::open(kit, &n)?;
            Some(n)
        }
        _ => None,
    };
    fs::create_dir_all(kit.join("state")).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&man).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&mode_file(kit), text.as_bytes()).map_err(|e| format!("Couldn't save the mode: {e}"))?;
    Ok(UseResult { profile, name: m.name, den })
}

/// No mode: a plain ruleset from the Host page's own picker (the realm list then shows no mode).
pub fn clear(kit: &Path) {
    let _ = fs::remove_file(mode_file(kit));
}

/// A listed realm's mode (realms.rs), checked: the credit always, the recipe only when it is a whole mode (then the
/// Realms page offers Fork this mode). None when it isn't a mode.
pub fn from_listing(v: Option<&Value>) -> Option<Value> {
    let v = v?.as_object()?;
    let id = v.get("id").and_then(Value::as_str).filter(|i| id_ok(i))?;
    let full = check(&Value::Object(v.clone()), None).ok().filter(|m| m.status == "live" && m.profile.is_some());
    let mut out = json!({ "id": id, "forkable": full.is_some() });
    if let Some(n) = text(v.get("name"), 32) {
        out["name"] = json!(n);
    }
    if let Some(a) = text(v.get("author"), 32) {
        out["author"] = json!(a);
    }
    if let Some(b) = v.get("based_on").filter(|b| b.is_object()) {
        if let Some(bid) = b.get("id").and_then(Value::as_str).filter(|i| id_ok(i) && *i != id) {
            out["based_on"] = json!({ "id": bid, "author": text(b.get("author"), 32) });
        }
    }
    if let Some(m) = full {
        let mut man = manifest(&m);
        man.as_object_mut().expect("an object").remove("picture");
        out["recipe"] = man;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit() -> tempfile::TempDir {
        let k = tempfile::tempdir().unwrap();
        let ls = k.path().join("realm-files").join("lua_scripts");
        fs::create_dir_all(&ls).unwrap();
        for s in ["ov_duels.lua", "ov_qol.lua", "ov_den.lua", "ov_build.lua", "ov_events.lua", "ov_main.lua", "ov_companions.lua",
            "ov_golf.lua", "ov_modes.lua", "ov_playmode.lua"] {
            fs::write(ls.join(s), "-- x").unwrap();
        }
        k
    }

    #[test]
    fn the_templates_are_built_in() {
        let l = list(None);
        let ids: Vec<&str> = l.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, TEMPLATES, "every template, in order");
        let d = l.iter().find(|m| m.id == "duel-ladder").unwrap();
        assert_eq!((d.profile.as_deref(), d.author.as_str(), d.own), (Some("brawl"), TEAM, false));
        assert!(d.picture.as_deref().unwrap().starts_with("data:image/svg+xml;base64,"));
        assert_eq!(d.env.get("OV_DUELS").map(String::as_str), Some("1"));
        let s = l.iter().find(|m| m.id == "survivors").unwrap();
        assert_eq!((s.status.as_str(), s.profile.as_deref()), ("coming", None));
        assert!(s.credit.is_some());
    }

    #[test]
    fn rules_match_the_package_checker() {
        for ok in ["OV_DUELS", "OV_EVENTS_HIDE_EVERY_H", "AC_RATE_XP_KILL", "AC_START_PLAYER_LEVEL", "AC_AUTO_BALANCE_ENABLE_GLOBAL"] {
            assert!(env_name_ok(ok), "{ok}");
        }
        for bad in ["AC_LOGIN_DATABASE_INFO", "OV_DISCORD_WEBHOOK", "AC_SOAP_ENABLED", "OV_DEN_SAVES", "OV_DUELS_TEST", "WU_BOTS",
            "AC_PLAYER_LIMIT", "OV_X", "ov_duels", "OV_HOST_INVITE", "OV_REALM_NAME", "OV_LEYLINE_BRAND"] {
            assert!(!env_name_ok(bad), "{bad}");
        }
        for (s, ok) in [("ov_duels.lua", true), ("../ov_duels.lua", false), ("ov_x.sh", false), ("evil.lua", false)] {
            assert_eq!(script_ok(s), ok, "{s}");
        }
        for (s, ok) in [("duel-ladder", true), ("a", false), ("Duel", false), ("x--y", false), ("x-", false), ("1x", false)] {
            assert_eq!(id_ok(s), ok, "{s}");
        }
        assert!(env_value_ok("1.5") && !env_value_ok("$(x)") && !env_value_ok("a b"));
    }

    #[test]
    fn fork_save_use_remove() {
        let k = kit();
        let f = fork(k.path(), "duel-ladder", None, Some("Ann")).unwrap();
        assert_eq!((f.id.as_str(), f.name.as_str(), f.author.as_str(), f.own), ("duel-ladder-2", "My Duel ladder", "Ann", true));
        assert_eq!(f.based_on, Some(BasedOn { id: "duel-ladder".into(), author: TEAM.into() }));
        assert!(forks_dir(k.path()).join("duel-ladder-2").join("picture.svg").is_file(), "its picture comes with it");
        let again = fork(k.path(), "duel-ladder", None, None).unwrap();
        assert_eq!((again.id.as_str(), again.author.as_str()), ("duel-ladder-3", "Host"));
        assert!(list(Some(k.path())).iter().any(|m| m.id == "duel-ladder-2" && m.own), "listed after the templates");
        // change it: name, your name, the jam, a default
        let s = save(k.path(), "duel-ladder-2", &json!({ "name": "Ann's duels", "jam": "first-jam", "defaults": { "min_s": 40 } })).unwrap();
        assert_eq!((s.name.as_str(), s.jam.as_deref(), s.version.as_str()), ("Ann's duels", Some("first-jam"), "2"));
        assert_eq!(s.settings[0]["default"], 40);
        assert!(save(k.path(), "duel-ladder", &json!({ "name": "x" })).unwrap_err().contains("fork it first"));
        assert!(save(k.path(), "duel-ladder-2", &json!({ "name": "Warcraft duels" })).is_err(), "no game names");
        assert!(save(k.path(), "duel-ladder-2", &json!({ "author": "Leyline" })).is_err(), "not the team's name");
        assert!(save(k.path(), "duel-ladder-2", &json!({ "defaults": { "min_s": 9999 } })).is_err());
        // host it
        let u = use_mode(k.path(), "duel-ladder-2", &json!({ "streaks": false })).unwrap();
        assert_eq!((u.profile.as_str(), u.name.as_str(), u.den), ("brawl", "Ann's duels", None));
        let w: Value = serde_json::from_str(&fs::read_to_string(mode_file(k.path())).unwrap()).unwrap();
        assert_eq!(w["resolved"]["OV_DUELS_MIN_S"], "40");
        assert_eq!(w["resolved"]["OV_DUELS_STREAKS"], "0");
        assert_eq!(w["resolved"]["OV_DUELS"], "1");
        assert_eq!(w["based_on"]["id"], "duel-ladder");
        assert_eq!(w["settings"][3]["default"], false, "a fork of this realm starts from its answers");
        assert!(w.get("picture").is_none());
        assert!(use_mode(k.path(), "duel-ladder", &json!({ "min_s": 1 })).unwrap_err().contains("5 to 120"));
        assert!(use_mode(k.path(), "survivors", &json!({})).unwrap_err().contains("isn't ready"));
        assert!(fork(k.path(), "survivors", None, None).is_err(), "a coming mode can't be forked");
        remove(k.path(), "duel-ladder-3").unwrap();
        assert!(remove(k.path(), "duel-ladder").is_err(), "a template stays");
        assert!(remove(k.path(), "../state").is_err());
        clear(k.path());
        assert!(!mode_file(k.path()).exists());
    }

    #[test]
    fn a_den_mode_brings_its_den_file() {
        let k = kit();
        let d = forks_dir(k.path()).join("my-room");
        fs::create_dir_all(&d).unwrap();
        let den = r#"{"format":"leyline-den","version":1,"name":"room","props":[{"entry":1,"x":0,"y":0,"z":0}]}"#;
        fs::write(d.join("room.den.json"), den).unwrap();
        fs::write(d.join("mode.json"), json!({ "format": FORMAT, "schema": 1, "id": "my-room", "name": "My room", "line": "", "version": "1",
            "author": "Ann", "based_on": { "id": "den", "author": "Leyline" }, "profile": "den", "scripts": ["ov_den.lua"], "den": "room.den.json",
            "settings": [] }).to_string()).unwrap();
        let u = use_mode(k.path(), "my-room", &json!({})).unwrap();
        assert_eq!(u.den.as_deref(), Some("my_room"));
        assert!(dens::dir(k.path()).join("open.json").is_file(), "opened in the Den");
        let f = fork(k.path(), "my-room", None, Some("Bo")).unwrap();
        assert!(forks_dir(k.path()).join(&f.id).join("room.den.json").is_file(), "a fork keeps the Den file");
    }

    #[test]
    fn bad_packages_are_left_out() {
        let k = kit();
        let mk = |id: &str, v: Value| {
            let d = forks_dir(k.path()).join(id);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("mode.json"), v.to_string()).unwrap();
        };
        let base = |id: &str| json!({ "format": FORMAT, "schema": 1, "id": id, "name": "X mode", "version": "1", "author": "Ann",
            "profile": "brawl", "scripts": ["ov_duels.lua"] });
        mk("good-one", base("good-one"));
        let mut v = base("bad-env");
        v["env"] = json!({ "AC_LOGIN_DATABASE_INFO": "x" });
        mk("bad-env", v);
        let mut v = base("bad-script");
        v["scripts"] = json!(["ov_nothere.lua"]);
        mk("bad-script", v);
        mk("wrong-folder", base("other-id"));
        let mut v = base("team-name");
        v["author"] = json!("Leyline Staff");
        mk("team-name", v);
        let d = forks_dir(k.path()).join("good-one");
        fs::write(d.join("picture.svg"), "<svg onload=\"x()\"></svg>").unwrap();
        let l = list(Some(k.path()));
        let own: Vec<&str> = l.iter().filter(|m| m.own).map(|m| m.id.as_str()).collect();
        assert_eq!(own, ["good-one"]);
        assert_eq!(l.iter().find(|m| m.id == "good-one").unwrap().picture, None, "an unsafe picture is not shown");
    }

    #[test]
    fn listed_modes_credit_and_fork() {
        let k = kit();
        let tpl = list(None).into_iter().find(|m| m.id == "duel-ladder").unwrap();
        let mut rec = manifest(&tpl);
        rec["id"] = json!("ann-duels");
        rec["name"] = json!("Ann duels");
        rec["author"] = json!("Ann");
        rec["based_on"] = json!({ "id": "duel-ladder", "author": "Leyline" });
        let l = from_listing(Some(&rec)).unwrap();
        assert_eq!((l["forkable"].as_bool(), l["based_on"]["id"].as_str()), (Some(true), Some("duel-ladder")));
        let f = fork(k.path(), "ann-duels", Some(&l["recipe"]), Some("Bo")).unwrap();
        assert_eq!(f.based_on.as_ref().unwrap().id, "ann-duels");
        assert!(f.picture.is_some(), "the picture of the mode it is based on");
        let credit = from_listing(Some(&json!({ "id": "x-y", "name": "Somebody" }))).unwrap();
        assert_eq!(credit["forkable"], false);
        assert!(from_listing(Some(&json!({ "id": "../x" }))).is_none());
        assert!(fork(k.path(), "other", Some(&l["recipe"]), None).is_err(), "the id must match");
    }

    #[test]
    fn b64_round_trip() {
        for s in [&b""[..], b"a", b"ab", b"abc", b"<svg>\x00\xff"] {
            assert_eq!(crate::update::b64decode(&b64(s)).unwrap(), s);
        }
    }
}
