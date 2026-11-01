//! Mods & Addons › Graphics (owner 31 Oct 22:58: "add graphics tab for gfx tweaks"): the game client's own graphics
//! settings, read from and written to its `WTF/Config.wtf` (`SET name "value"` lines, the same file the game's Video
//! options write). Only a short, checked list per client version; every value is a number inside its range, so
//! nothing else can be written. The first change keeps a copy (`Config.wtf.leyline-backup`), and "Back to how it
//! was" puts it back. Changes take effect the next time the game starts.

use crate::clients::{Build, ClientProfile};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// One setting: its name in Config.wtf, what the player sees, its kind and range, and the game's default.
#[derive(Debug, Clone, Serialize)]
pub struct Tweak {
    pub key: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    /// "slider", "toggle" or "choice"
    pub kind: &'static str,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// For "choice": the allowed values and their words.
    pub choices: &'static [(&'static str, &'static str)],
    pub default: &'static str,
}

const fn slider(key: &'static str, label: &'static str, help: &'static str, min: f64, max: f64, step: f64, default: &'static str) -> Tweak {
    Tweak { key, label, help, kind: "slider", min, max, step, choices: &[], default }
}
const fn toggle(key: &'static str, label: &'static str, help: &'static str, default: &'static str) -> Tweak {
    Tweak { key, label, help, kind: "toggle", min: 0.0, max: 1.0, step: 1.0, choices: &[], default }
}
const fn choice(key: &'static str, label: &'static str, help: &'static str, choices: &'static [(&'static str, &'static str)], default: &'static str) -> Tweak {
    Tweak { key, label, help, kind: "choice", min: 0.0, max: 0.0, step: 0.0, choices, default }
}

const AA: &[(&str, &str)] = &[("1", "Off"), ("2", "2x"), ("4", "4x"), ("8", "8x")];
const FILTER: &[(&str, &str)] = &[("0", "Bilinear"), ("1", "Trilinear"), ("2", "2x"), ("3", "4x"), ("4", "8x"), ("5", "16x")];
const WEATHER: &[(&str, &str)] = &[("0", "Off"), ("1", "Low"), ("2", "Medium"), ("3", "High")];
const SPELLS: &[(&str, &str)] = &[("0", "Low"), ("1", "Medium"), ("2", "High")];
const SHADOWS: &[(&str, &str)] = &[("0", "Off"), ("1", "Low"), ("2", "Medium"), ("3", "High"), ("4", "Ultra")];
const TEXTURES: &[(&str, &str)] = &[("1", "Low"), ("0", "High")];

/// 3.3.5a: the classic Video options plus a few the menu hides.
const WOTLK: &[Tweak] = &[
    slider("farclip", "View distance", "How far the world is drawn.", 177.0, 1277.0, 50.0, "1277"),
    slider("groundEffectDensity", "Ground clutter", "Grass and small plants.", 16.0, 256.0, 8.0, "64"),
    slider("groundEffectDist", "Ground clutter distance", "How far grass is drawn.", 70.0, 140.0, 5.0, "70"),
    slider("environmentDetail", "Environment detail", "Trees and doodads in the distance.", 0.5, 1.5, 0.1, "1"),
    choice("extShadowQuality", "Shadows", "Character and world shadows.", SHADOWS, "0"),
    choice("baseMip", "Texture resolution", "High uses more video memory.", TEXTURES, "0"),
    choice("textureFilteringMode", "Texture filtering", "Sharper textures at an angle.", FILTER, "1"),
    choice("gxMultisample", "Anti-aliasing", "Smoother edges; costs frames.", AA, "1"),
    choice("weatherDensity", "Weather", "Rain and snow.", WEATHER, "2"),
    choice("spellEffectLevel", "Spell detail", "Particles from spells.", SPELLS, "2"),
    toggle("ffxGlow", "Full-screen glow", "The soft glow over bright areas.", "1"),
    toggle("ffxDeath", "Death effect", "The grey screen when you die.", "1"),
    toggle("gxVSync", "Vertical sync", "No tearing; caps frames to the screen.", "1"),
    slider("maxFPS", "Frame cap", "0 is no cap.", 0.0, 300.0, 10.0, "0"),
    slider("maxFPSBk", "Frame cap in the background", "When the game isn't in front.", 0.0, 120.0, 5.0, "30"),
    toggle("gxWindow", "Windowed", "Play in a window.", "0"),
    toggle("gxMaximize", "Windowed full screen", "A borderless window over the whole screen (with Windowed).", "0"),
];

/// 2.4.3: the same family, without the later shadow and filtering settings.
const TBC: &[Tweak] = &[
    slider("farclip", "View distance", "How far the world is drawn.", 177.0, 777.0, 50.0, "777"),
    slider("groundEffectDensity", "Ground clutter", "Grass and small plants.", 16.0, 256.0, 8.0, "64"),
    slider("groundEffectDist", "Ground clutter distance", "How far grass is drawn.", 70.0, 140.0, 5.0, "70"),
    choice("baseMip", "Texture resolution", "High uses more video memory.", TEXTURES, "0"),
    choice("gxMultisample", "Anti-aliasing", "Smoother edges; costs frames.", AA, "1"),
    choice("weatherDensity", "Weather", "Rain and snow.", WEATHER, "2"),
    choice("spellEffectLevel", "Spell detail", "Particles from spells.", SPELLS, "2"),
    toggle("ffxGlow", "Full-screen glow", "The soft glow over bright areas.", "1"),
    toggle("ffxDeath", "Death effect", "The grey screen when you die.", "1"),
    toggle("gxVSync", "Vertical sync", "No tearing; caps frames to the screen.", "1"),
    slider("maxFPS", "Frame cap", "0 is no cap.", 0.0, 300.0, 10.0, "0"),
    toggle("gxWindow", "Windowed", "Play in a window.", "0"),
    toggle("gxMaximize", "Windowed full screen", "A borderless window over the whole screen (with Windowed).", "0"),
];

/// 1.12.1: the few its Config.wtf takes for sure.
const VANILLA: &[Tweak] = &[
    slider("farclip", "View distance", "How far the world is drawn.", 177.0, 777.0, 50.0, "777"),
    toggle("gxVSync", "Vertical sync", "No tearing; caps frames to the screen.", "1"),
    toggle("gxWindow", "Windowed", "Play in a window.", "0"),
    toggle("gxMaximize", "Windowed full screen", "A borderless window over the whole screen (with Windowed).", "0"),
];

pub fn tweaks(build: Build) -> &'static [Tweak] {
    match build {
        Build::V335a => WOTLK,
        Build::V243 => TBC,
        Build::V1121 => VANILLA,
        _ => &[],
    }
}

/// The presets: (id, name, values). Settings a version doesn't have are skipped.
pub const PRESETS: &[(&str, &str, &[(&str, &str)])] = &[
    ("low", "Low", &[("farclip", "377"), ("groundEffectDensity", "16"), ("groundEffectDist", "70"), ("environmentDetail", "0.5"),
        ("extShadowQuality", "0"), ("baseMip", "1"), ("textureFilteringMode", "0"), ("gxMultisample", "1"), ("weatherDensity", "0"),
        ("spellEffectLevel", "0"), ("ffxGlow", "0"), ("ffxDeath", "0")]),
    // Classic (Looks, owner 1 Nov 14:38): the game as it looked on day one: the original view, no extra shadows, plain
    // filtering and no smoothing, with the soft full-screen glow.
    ("classic", "Classic", &[("farclip", "777"), ("groundEffectDensity", "64"), ("groundEffectDist", "70"), ("environmentDetail", "1"),
        ("extShadowQuality", "0"), ("baseMip", "0"), ("textureFilteringMode", "1"), ("gxMultisample", "1"), ("weatherDensity", "2"),
        ("spellEffectLevel", "2"), ("ffxGlow", "1"), ("ffxDeath", "1")]),
    ("balanced", "Balanced", &[("farclip", "777"), ("groundEffectDensity", "64"), ("groundEffectDist", "100"), ("environmentDetail", "1"),
        ("extShadowQuality", "1"), ("baseMip", "0"), ("textureFilteringMode", "2"), ("gxMultisample", "1"), ("weatherDensity", "1"),
        ("spellEffectLevel", "1"), ("ffxGlow", "1"), ("ffxDeath", "1")]),
    ("high", "High", &[("farclip", "1027"), ("groundEffectDensity", "128"), ("groundEffectDist", "120"), ("environmentDetail", "1.2"),
        ("extShadowQuality", "3"), ("baseMip", "0"), ("textureFilteringMode", "4"), ("gxMultisample", "4"), ("weatherDensity", "2"),
        ("spellEffectLevel", "2"), ("ffxGlow", "1"), ("ffxDeath", "1")]),
    ("ultra", "Ultra", &[("farclip", "1277"), ("groundEffectDensity", "256"), ("groundEffectDist", "140"), ("environmentDetail", "1.5"),
        ("extShadowQuality", "4"), ("baseMip", "0"), ("textureFilteringMode", "5"), ("gxMultisample", "8"), ("weatherDensity", "3"),
        ("spellEffectLevel", "2"), ("ffxGlow", "1"), ("ffxDeath", "1")]),
];

/// Is `value` allowed for `t`? Numbers only, inside the range (or one of the choices).
pub fn check(t: &Tweak, value: &str) -> Result<String, String> {
    let v = value.trim();
    match t.kind {
        "choice" => t
            .choices
            .iter()
            .find(|(c, _)| *c == v)
            .map(|(c, _)| c.to_string())
            .ok_or_else(|| format!("{}: pick one of the choices.", t.label)),
        "toggle" => match v {
            "0" | "1" => Ok(v.to_string()),
            _ => Err(format!("{}: on or off.", t.label)),
        },
        _ => {
            let n: f64 = v.parse().map_err(|_| format!("{}: a number, please.", t.label))?;
            if !n.is_finite() || n < t.min || n > t.max {
                return Err(format!("{}: between {} and {}.", t.label, t.min, t.max));
            }
            // whole numbers stay whole; the rest to one decimal
            Ok(if t.step >= 1.0 { format!("{}", n.round() as i64) } else { format!("{:.1}", n) })
        }
    }
}

pub fn config_path(c: &ClientProfile) -> PathBuf {
    crate::clients::find_rel(&c.path, "WTF/Config.wtf")
        .unwrap_or_else(|| crate::clients::find_ci(&c.path, "WTF").unwrap_or_else(|| c.path.join("WTF")).join("Config.wtf"))
}

/// `SET name "value"` lines (names without regard to case).
pub fn parse(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let l = line.trim();
        let Some(rest) = l.strip_prefix("SET ").or_else(|| l.strip_prefix("set ")) else { continue };
        let mut parts = rest.splitn(2, char::is_whitespace);
        let (Some(k), Some(v)) = (parts.next(), parts.next()) else { continue };
        out.insert(k.to_ascii_lowercase(), v.trim().trim_matches('"').to_string());
    }
    out
}

/// `text` with each of `set` written: an existing line replaced in place, a new one added at the end.
pub fn apply(text: &str, set: &BTreeMap<String, String>) -> String {
    let mut left: BTreeMap<String, (String, String)> =
        set.iter().map(|(k, v)| (k.to_ascii_lowercase(), (k.clone(), v.clone()))).collect();
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        let key = l
            .strip_prefix("SET ")
            .or_else(|| l.strip_prefix("set "))
            .and_then(|r| r.split_whitespace().next())
            .map(|k| k.to_ascii_lowercase());
        if let Some(k) = key {
            if let Some((name, v)) = left.remove(&k) {
                out.push(format!("SET {name} \"{v}\""));
                continue;
            }
        }
        out.push(line.to_string());
    }
    for (_, (name, v)) in left {
        out.push(format!("SET {name} \"{v}\""));
    }
    let mut s = out.join(nl);
    s.push_str(nl);
    s
}

#[derive(Debug, Serialize)]
pub struct State {
    pub client: String,
    pub build: Build,
    pub file: String,
    pub tweaks: Vec<Tweak>,
    /// What Config.wtf says now (missing: the game's default).
    pub values: BTreeMap<String, String>,
    pub presets: Vec<(String, String)>,
    pub has_backup: bool,
}

fn backup_of(p: &Path) -> PathBuf {
    p.with_file_name("Config.wtf.leyline-backup")
}

pub fn read(c: &ClientProfile) -> State {
    let p = config_path(c);
    let have = fs::read_to_string(&p).map(|t| parse(&t)).unwrap_or_default();
    let list = tweaks(c.build);
    let values = list
        .iter()
        .map(|t| (t.key.to_string(), have.get(&t.key.to_ascii_lowercase()).cloned().unwrap_or_else(|| t.default.to_string())))
        .collect();
    State {
        client: c.name.clone(),
        build: c.build,
        file: p.display().to_string(),
        tweaks: list.to_vec(),
        values,
        presets: if list.is_empty() { vec![] } else { PRESETS.iter().map(|(id, n, _)| (id.to_string(), n.to_string())).collect() },
        has_backup: backup_of(&p).exists(),
    }
}

/// Write `values` (checked) into the client's Config.wtf; the first time, keep a copy of the file as it was.
pub fn write(c: &ClientProfile, values: &BTreeMap<String, String>) -> Result<State, String> {
    let list = tweaks(c.build);
    if list.is_empty() {
        return Err("This game version keeps its graphics in the game's own Video options.".into());
    }
    let mut set = BTreeMap::new();
    for (k, v) in values {
        let t = list.iter().find(|t| t.key.eq_ignore_ascii_case(k)).ok_or_else(|| format!("Leyline doesn't change {k}."))?;
        set.insert(t.key.to_string(), check(t, v)?);
    }
    let p = config_path(c);
    let old = fs::read_to_string(&p).unwrap_or_default();
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| format!("Couldn't open the game's settings folder: {e}"))?;
    }
    let b = backup_of(&p);
    if !b.exists() {
        fs::write(&b, &old).map_err(|e| format!("Couldn't keep a copy of the settings: {e}"))?;
    }
    let tmp = p.with_extension("wtf.leyline-tmp");
    fs::write(&tmp, apply(&old, &set)).map_err(|e| format!("Couldn't save the settings: {e}"))?;
    fs::rename(&tmp, &p).map_err(|e| format!("Couldn't save the settings: {e}"))?;
    if set.keys().any(|k| k.eq_ignore_ascii_case("gxWindow") || k.eq_ignore_ascii_case("gxMaximize")) {
        let _ = fs::write(window_chosen_mark(&p), b"the player set the window on the Graphics page\n");
    }
    Ok(read(c))
}

/// The player chose the window on the Graphics page: launches leave gxWindow / gxMaximize as they set them.
fn window_chosen_mark(p: &Path) -> PathBuf {
    p.with_file_name("Config.wtf.leyline-window")
}

/// What every launch sets (owner 1 Nov 02:50): no intro movie, the legal screens already read, and a borderless
/// window over the whole screen (gxWindow 1, gxMaximize 1) so the game doesn't switch the screen's resolution;
/// unless the player picked the window on the Graphics page, which then stays as they set it.
pub fn launch_values(window_chosen: bool) -> BTreeMap<String, String> {
    let mut set: BTreeMap<String, String> =
        [("movie", "0"), ("readEULA", "1"), ("readTOS", "1"), ("readScanning", "1"), ("readContest", "1"), ("showToolsUI", "0")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
    if !window_chosen {
        set.insert("gxWindow".into(), "1".into());
        set.insert("gxMaximize".into(), "1".into());
    }
    set
}

/// Before a launch: `launch_values` written into Config.wtf (a copy of the file as it was is kept the first time).
/// Builds Leyline has no settings for are left alone.
pub fn prepare_launch(c: &ClientProfile) -> Result<(), String> {
    if tweaks(c.build).is_empty() {
        return Ok(());
    }
    let p = config_path(c);
    let old = fs::read_to_string(&p).unwrap_or_default();
    let new = apply(&old, &launch_values(window_chosen_mark(&p).exists()));
    if new == old {
        return Ok(());
    }
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| format!("Couldn't open the game's settings folder: {e}"))?;
    }
    let b = backup_of(&p);
    if !b.exists() {
        fs::write(&b, &old).map_err(|e| format!("Couldn't keep a copy of the settings: {e}"))?;
    }
    let tmp = p.with_extension("wtf.leyline-tmp");
    fs::write(&tmp, new).map_err(|e| format!("Couldn't save the settings: {e}"))?;
    fs::rename(&tmp, &p).map_err(|e| format!("Couldn't save the settings: {e}"))
}

pub fn preset(c: &ClientProfile, id: &str) -> Result<State, String> {
    let (_, _, vals) = PRESETS.iter().find(|(p, _, _)| *p == id).ok_or("No such preset.")?;
    let list = tweaks(c.build);
    let set: BTreeMap<String, String> = vals
        .iter()
        .filter_map(|(k, v)| {
            let t = list.iter().find(|t| t.key == *k)?;
            // a value outside this version's range is clamped into it (1.12 and 2.4.3 see less far)
            let v = if t.kind == "slider" {
                let n: f64 = v.parse().ok()?;
                let n = n.clamp(t.min, t.max);
                if t.step >= 1.0 { format!("{}", n.round() as i64) } else { format!("{n:.1}") }
            } else {
                v.to_string()
            };
            Some((k.to_string(), v))
        })
        .collect();
    write(c, &set)
}

/// Put the copy kept at the first change back.
pub fn restore(c: &ClientProfile) -> Result<State, String> {
    let p = config_path(c);
    let b = backup_of(&p);
    let old = fs::read_to_string(&b).map_err(|_| "There's no earlier copy to go back to.".to_string())?;
    fs::write(&p, old).map_err(|e| format!("Couldn't put the settings back: {e}"))?;
    let _ = fs::remove_file(&b);
    Ok(read(c))
}

#[cfg(feature = "app")]
pub mod commands {
    use super::*;
    use tauri::State as TState;

    type Core<'a> = TState<'a, crate::app::Core>;

    #[tauri::command]
    pub fn gfx_get(core: Core, client: String) -> Result<State, String> {
        Ok(read(&core.client(&client)?))
    }
    #[tauri::command]
    pub fn gfx_set(core: Core, client: String, values: BTreeMap<String, String>) -> Result<State, String> {
        write(&core.client(&client)?, &values)
    }
    #[tauri::command]
    pub fn gfx_preset(core: Core, client: String, preset: String) -> Result<State, String> {
        super::preset(&core.client(&client)?, &preset)
    }
    #[tauri::command]
    pub fn gfx_restore(core: Core, client: String) -> Result<State, String> {
        restore(&core.client(&client)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(dir: &Path, build: Build) -> ClientProfile {
        let mut c: ClientProfile = serde_json::from_value(serde_json::json!({"id":"c1","name":"Mine","path":dir,"build":"3.3.5a"})).unwrap();
        c.build = build;
        c
    }

    #[test]
    fn a_launch_skips_the_movie_and_opens_a_borderless_window_unless_the_player_chose_one() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("WTF")).unwrap();
        let cfg = d.path().join("WTF/Config.wtf");
        fs::write(&cfg, "SET realmlist \"x\"\r\nSET gxWindow \"0\"\r\nSET gxResolution \"1024x768\"\r\n").unwrap();
        let c = client(d.path(), Build::V335a);
        prepare_launch(&c).unwrap();
        let v = parse(&fs::read_to_string(&cfg).unwrap());
        assert_eq!((v["movie"].as_str(), v["readeula"].as_str(), v["readtos"].as_str()), ("0", "1", "1"));
        assert_eq!((v["gxwindow"].as_str(), v["gxmaximize"].as_str()), ("1", "1"));
        assert_eq!(v["realmlist"], "x", "the rest kept");
        assert!(fs::read_to_string(backup_of(&cfg)).unwrap().contains("gxWindow \"0\""), "a copy of the file as it was");
        // the Graphics page's choice wins from then on
        write(&c, &BTreeMap::from([("gxWindow".to_string(), "0".to_string())])).unwrap();
        prepare_launch(&c).unwrap();
        assert_eq!(parse(&fs::read_to_string(&cfg).unwrap())["gxwindow"], "0");
        // a version Leyline has no settings for is left alone
        let e = tempfile::tempdir().unwrap();
        prepare_launch(&client(e.path(), Build::V548)).unwrap();
        assert!(!e.path().join("WTF/Config.wtf").exists());
    }

    #[test]
    fn values_are_checked() {
        let t = WOTLK.iter().find(|t| t.key == "farclip").unwrap();
        assert_eq!(check(t, "1000").unwrap(), "1000");
        assert!(check(t, "5000").is_err());
        assert!(check(t, "1e3; rm").is_err());
        let aa = WOTLK.iter().find(|t| t.key == "gxMultisample").unwrap();
        assert_eq!(check(aa, "4").unwrap(), "4");
        assert!(check(aa, "3").is_err());
        let env = WOTLK.iter().find(|t| t.key == "environmentDetail").unwrap();
        assert_eq!(check(env, "1.25").unwrap(), "1.2");
    }

    #[test]
    fn writes_in_place_keeps_the_rest_and_a_backup() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("WTF")).unwrap();
        fs::write(d.path().join("WTF/Config.wtf"), "SET realmlist \"x\"\r\nSET farclip \"777\"\r\nSET gxApi \"d3d9\"\r\n").unwrap();
        let c = client(d.path(), Build::V335a);
        let s = read(&c);
        assert_eq!(s.values["farclip"], "777");
        assert_eq!(s.values["gxMultisample"], "1", "missing: the game's default");
        let mut v = BTreeMap::new();
        v.insert("farclip".to_string(), "1277".to_string());
        v.insert("gxMultisample".to_string(), "8".to_string());
        let s = write(&c, &v).unwrap();
        let t = fs::read_to_string(d.path().join("WTF/Config.wtf")).unwrap();
        assert_eq!(t, "SET realmlist \"x\"\r\nSET farclip \"1277\"\r\nSET gxApi \"d3d9\"\r\nSET gxMultisample \"8\"\r\n");
        assert!(s.has_backup);
        // something not on the list, or out of range: refused, nothing written
        let mut bad = BTreeMap::new();
        bad.insert("realmlist".to_string(), "evil".to_string());
        assert!(write(&c, &bad).is_err());
        assert!(fs::read_to_string(d.path().join("WTF/Config.wtf")).unwrap().contains("realmlist \"x\""));
        // a preset, then back to how it was
        let s = preset(&c, "low").unwrap();
        assert_eq!(s.values["farclip"], "377");
        let s = restore(&c).unwrap();
        assert_eq!(s.values["farclip"], "777");
        assert!(!s.has_backup);
    }

    #[test]
    fn older_versions_get_their_own_list_and_clamped_presets() {
        let d = tempfile::tempdir().unwrap();
        let c = client(d.path(), Build::V1121);
        let s = preset(&c, "ultra").unwrap();
        assert_eq!(s.values["farclip"], "777", "1.12 sees up to 777");
        assert!(!s.values.contains_key("gxMultisample"));
        assert!(fs::read_to_string(d.path().join("WTF/Config.wtf")).unwrap().contains("SET farclip \"777\""));
        let c = client(d.path(), Build::V434);
        assert!(read(&c).tweaks.is_empty());
        assert!(write(&c, &BTreeMap::new()).is_err());
    }
}
