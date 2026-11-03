//! "My models" (owner 3 Nov: "drag and drop new 3d models"): the player's own model files, dropped on the launcher's
//! Your Den card, become props here.
//!
//! The launcher (launcher/src-tauri/src/host/denmodels.rs) keeps the files on this PC, puts them in the Den's own
//! patch (`Data/patch-T.MPQ`, under `World\Leyline\Den\...` only), adds one row per model to the player's own
//! `GameObjectDisplayInfo.dbc` there (and to the copy in the Dens folder that models.rs reads for the gallery), and
//! writes `<dens>/my-models.json`:
//!
//! ```json
//! {"format": "leyline-den-models", "version": 1,
//!  "models": [{"entry": 980001, "display": 60001, "name": "Lighthouse", "wmo": false}]}
//! ```
//!
//! Den Lite reads it at its start: each model is a gameobject template of its own (entry [`ENTRY_BASE`] + n, the
//! generic type, the row's display) in the Builder's "My models" category. Nothing is uploaded or shared: a friend's
//! game hasn't these files or rows, so a friend sees each one as a plain stand-in ([`stand_in`]) instead of a model
//! their game can't find.

use crate::den::Template;
use std::path::Path;
use std::sync::RwLock;

pub const FILE: &str = "my-models.json";
pub const FORMAT: &str = "leyline-den-models";
/// The entries of the player's own models: 980001 ..= 980000 + MAX.
pub const ENTRY_BASE: u32 = 980_000;
pub const MAX: usize = 200;
/// The Builder's category (Builder.lua CATS "mymodels", shown as "My models").
pub const CATEGORY: &str = "mymodels";
/// The generic gameobject type (GAMEOBJECT_TYPE_GENERIC).
const KIND: u8 = 5;
/// What a friend sees in a model's place: the model carrier (den.rs MODEL_CARRIER, the catalog's Bottle), which
/// every game has.
pub const STAND_IN: u32 = crate::den::MODEL_CARRIER;

static MINE: RwLock<Vec<Template>> = RwLock::new(Vec::new());

/// A model's name as the Builder shows it: letters, digits, spaces, - and _, 1 to 32 of them.
pub fn clean_name(s: &str) -> Option<String> {
    let n: String = s.trim().chars().filter(|c| c.is_ascii_alphanumeric() || " -_".contains(*c)).take(32).collect();
    let n = n.trim().to_string();
    (!n.is_empty()).then_some(n)
}

/// The models in a my-models.json's text, checked (unknown or bad rows left out).
pub fn parse(text: &str) -> Vec<Template> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) else { return Vec::new() };
    if v.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Vec::new();
    }
    let mut out: Vec<Template> = Vec::new();
    for m in v.get("models").and_then(|m| m.as_array()).map(Vec::as_slice).unwrap_or(&[]).iter().take(MAX) {
        let entry = m.get("entry").and_then(|e| e.as_u64()).unwrap_or(0);
        let display = m.get("display").and_then(|e| e.as_u64()).unwrap_or(0);
        let Some(name) = m.get("name").and_then(|n| n.as_str()).and_then(clean_name) else { continue };
        let entry_ok = entry > ENTRY_BASE as u64 && entry <= (ENTRY_BASE as usize + MAX) as u64;
        if !entry_ok || display == 0 || display > 1_000_000 || out.iter().any(|t| t.entry as u64 == entry) {
            continue;
        }
        let name: &'static str = Box::leak(name.into_boxed_str());
        out.push(Template { entry: entry as u32, kind: KIND, display: display as u32, name, size: 1.0 });
    }
    out
}

/// Reads `<dens>/my-models.json` (at the start); how many models.
pub fn load(dir: &Path) -> usize {
    let list = std::fs::read_to_string(dir.join(FILE)).map(|t| parse(&t)).unwrap_or_default();
    let n = list.len();
    *MINE.write().unwrap_or_else(|e| e.into_inner()) = list;
    n
}

/// A model's template.
pub fn template(entry: u32) -> Option<Template> {
    if entry <= ENTRY_BASE {
        return None;
    }
    MINE.read().unwrap_or_else(|e| e.into_inner()).iter().find(|t| t.entry == entry).copied()
}

/// Every model (for the catalog and `.den find`).
pub fn all() -> Vec<Template> {
    MINE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn is_mine(entry: u32) -> bool {
    template(entry).is_some()
}

/// An object as a friend's game gets it: the player's own model becomes the stand-in (their game hasn't the files).
pub fn stand_in(o: &crate::den::WorldObject) -> crate::den::WorldObject {
    if !is_mine(o.entry) {
        return o.clone();
    }
    let t = crate::den::template(STAND_IN).expect("the carrier is a known prop");
    crate::den::WorldObject { entry: t.entry, display: t.display, kind: t.kind, ..o.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_models_are_read_checked_and_stand_in_for_friends() {
        let text = r#"{"format":"leyline-den-models","version":1,"models":[
            {"entry":980001,"display":60001,"name":"Qyzz beacon!"},
            {"entry":980001,"display":60002,"name":"twice"},
            {"entry":12,"display":60003,"name":"not ours"},
            {"entry":980002,"display":0,"name":"no display"},
            {"entry":980003,"display":60004,"name":"   "},
            {"entry":980004,"display":60005,"name":"Qyzzspire","wmo":true}]}"#;
        let l = parse(text);
        assert_eq!(l.iter().map(|t| (t.entry, t.display, t.name)).collect::<Vec<_>>(), [(980001, 60001, "Qyzz beacon"), (980004, 60005, "Qyzzspire")]);
        assert!(parse(r#"{"format":"x","models":[{"entry":980001,"display":1,"name":"a"}]}"#).is_empty());
        assert!(parse("oops").is_empty());
        let dir = std::env::temp_dir().join(format!("den-mymodels-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), text).unwrap();
        assert_eq!(load(&dir), 2);
        let t = crate::den::template(980004).unwrap();
        assert_eq!((t.display, t.name), (60005, "Qyzzspire"));
        assert!(crate::den::find_objects("qyzzspire").iter().any(|o| o.0 == 980004), "`.den place qyzzspire` finds it");
        let o = crate::den::WorldObject { guid: 9, entry: 980004, display: 60005, kind: 5, scale: 1.0, x: 1.0, y: 2.0, z: 3.0, o: 0.0, pitch: 0.0, roll: 0.0 };
        let s = stand_in(&o);
        assert_eq!((s.entry, s.guid, s.x), (STAND_IN, 9, 1.0), "the same object, a model every game has");
        assert!(crate::ovmod::catalog_parts(|_| None).concat().contains("980004,qyzzspire,mymodels,Qyzzspire,1;"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
