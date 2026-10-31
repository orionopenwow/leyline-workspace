//! A Den's save files (docs/DEN.md, "The save file"): `<kit>/state/dens/<name>.den.json`, versioned JSON layouts the
//! realm writes (`ov_den.lua`, `.den save`) and reads (`.den load`, and `open.json` from here). The Host page's
//! Saved Dens lists them, gives a copy of one to save (its text, for a download: they are small), adds a file the host
//! picked on this PC, and opens one: "Open a saved Den" writes `open.json`, and the realm opens that file (replacing
//! what is in the Den, keeping a copy as "backup") as soon as its owner is in the room, at once or at the next login.
//!
//! Names are checked words (letters, digits, - and _); a file is checked for its shape here and again, prop by prop,
//! by the realm. Nothing in a file is ever run, here or there.

use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The largest Den file (ov_den.lua FILE_MAX).
pub const MAX_BYTES: u64 = 512 * 1024;
/// Props in one file (ov_build.lua K.LAYOUT_MAX) and paths (K.DEN_PATHS).
pub const MAX_PROPS: usize = 800;
pub const MAX_PATHS: usize = 100;
/// The file format ov_den.lua writes; a newer one needs a newer host kit.
pub const FORMAT: &str = "leyline-den";
pub const VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DenFile {
    pub name: String,
    pub bytes: u64,
    pub props: usize,
    /// When the realm saved it (UTC, as in the file), or "" when the file doesn't say.
    pub saved: String,
}

pub fn dir(kit: &Path) -> PathBuf {
    kit.join("state").join("dens")
}

/// A Den file's name: letters, digits, - and _, 1 to 24 of them, lower case (".den.json" may follow). Anything else
/// is refused, not trimmed.
pub fn clean_name(s: &str) -> Option<String> {
    let s = s.trim().to_lowercase();
    let s = s.strip_suffix(".den.json").or_else(|| s.strip_suffix(".json")).unwrap_or(&s).to_string();
    if s.is_empty() || s.len() > 24 || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return None;
    }
    Some(s)
}

/// Checks a Den file's shape: (props, saved, its own name).
pub fn check(text: &str) -> Result<(usize, String, String), String> {
    if text.len() as u64 > MAX_BYTES {
        return Err("That Den file is too big (512 KB at most).".into());
    }
    let doc: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| "That isn't a Den file (it can't be read).".to_string())?;
    if doc.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err("That isn't a Den file.".into());
    }
    match doc.get("version").and_then(Value::as_u64) {
        Some(v) if v >= 1 && v <= VERSION => {}
        Some(v) if v > VERSION => return Err("That Den file is from a newer Leyline: update the Host pack first.".into()),
        _ => return Err("That Den file has no version.".into()),
    }
    let props = doc.get("props").and_then(Value::as_array).ok_or("That Den file has no props list.")?;
    if props.len() > MAX_PROPS {
        return Err(format!("A Den file holds up to {MAX_PROPS} props."));
    }
    if props.iter().any(|p| !p.is_object() || !p.get("entry").map_or(false, Value::is_u64)) {
        return Err("A prop in that Den file isn't a prop.".into());
    }
    match doc.get("paths") {
        None | Some(Value::Null) => {}
        Some(Value::Array(a)) if a.len() <= MAX_PATHS => {}
        _ => return Err("That Den file's paths aren't right.".into()),
    }
    let saved = doc.get("saved").and_then(Value::as_str).filter(|s| s.len() <= 32).unwrap_or("").to_string();
    let name = doc.get("name").and_then(Value::as_str).and_then(clean_name).unwrap_or_default();
    Ok((props.len(), saved, name))
}

fn read(p: &Path) -> Result<String, String> {
    let meta = fs::metadata(p).map_err(|_| "That Den file isn't there any more.".to_string())?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err("That Den file is too big (512 KB at most).".into());
    }
    fs::read_to_string(p).map_err(|_| "That Den file can't be read.".to_string())
}

/// The Den files in the kit's saves folder, newest first (backup and broken files too: a broken one shows 0 props).
pub fn list(kit: &Path) -> Vec<DenFile> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir(kit)) else { return out };
    for e in rd.flatten() {
        let file = e.file_name().to_string_lossy().to_string();
        if !file.ends_with(".den.json") {
            continue;
        }
        let Some(name) = clean_name(&file) else { continue };
        let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
        let (props, saved) = read(&e.path()).ok().and_then(|t| check(&t).ok()).map(|(n, s, _)| (n, s)).unwrap_or((0, String::new()));
        out.push(DenFile { name, bytes, props, saved });
    }
    out.sort_by(|a, b| b.saved.cmp(&a.saved).then(a.name.cmp(&b.name)));
    out
}

/// One file's text (Save a copy).
pub fn text(kit: &Path, name: &str) -> Result<String, String> {
    let name = clean_name(name).ok_or("That isn't a Den file's name.")?;
    let t = read(&dir(kit).join(format!("{name}.den.json")))?;
    check(&t)?;
    Ok(t)
}

fn write(p: &Path, text: &str) -> Result<(), String> {
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| format!("Couldn't write the Den file: {e}"))?;
    fs::rename(&tmp, p).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Couldn't write the Den file: {e}")
    })
}

/// Adds a Den file the host picked (its file name and text): checked, then saved under its name (or the name in it),
/// never over another file ("house-2" when "house" is taken). Returns the name it has now.
pub fn add(kit: &Path, file_name: &str, text: &str) -> Result<String, String> {
    let (_, _, own) = check(text)?;
    let base = clean_name(file_name).or_else(|| (!own.is_empty()).then_some(own)).unwrap_or_else(|| "den".into());
    let d = dir(kit);
    fs::create_dir_all(&d).map_err(|e| format!("Couldn't make the saves folder: {e}"))?;
    for i in 1..100 {
        let name = if i == 1 { base.clone() } else { format!("{}-{i}", &base[..base.len().min(21)]) };
        let p = d.join(format!("{name}.den.json"));
        if !p.exists() {
            write(&p, text)?;
            return Ok(name);
        }
    }
    Err("Too many Den files with that name: rename it first.".into())
}

/// The open.json the realm looks for: { "file": name, "id": a new id each time }.
pub fn open_doc(name: &str, id: &str) -> String {
    serde_json::json!({ "file": name, "id": id }).to_string()
}

/// "Open a saved Den": the realm opens this file the next time its owner is in the Den (within seconds when the
/// realm is up and they are in it; else at the next login).
pub fn open(kit: &Path, name: &str) -> Result<String, String> {
    let name = clean_name(name).ok_or("That isn't a Den file's name.")?;
    text(kit, &name)?;
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let id = format!("{:x}{:x}", n, std::process::id());
    write(&dir(kit).join("open.json"), &open_doc(&name, &id))?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"format":"leyline-den","version":1,"name":"house","saved":"2026-10-31T22:50:37Z","room":{"look":"white","size":16},
        "paths":[{"group":1,"entry":190023,"every":4,"from":[-10,20,0],"to":[10,20,0]}],
        "props":[{"entry":19006,"x":0,"y":24,"z":0,"yaw":90},{"entry":190023,"group":1,"x":-10,"y":20,"z":0,"yaw":0}]}"#;

    #[test]
    fn names_are_words() {
        assert_eq!(clean_name("House.den.json").as_deref(), Some("house"));
        assert_eq!(clean_name("my-den_2").as_deref(), Some("my-den_2"));
        for bad in ["", "../x", "a b", "x/y", "a.b", "abcdefghijklmnopqrstuvwxyz", "naïve"] {
            assert_eq!(clean_name(bad), None, "{bad}");
        }
    }

    #[test]
    fn files_are_checked() {
        assert_eq!(check(GOOD).unwrap(), (2, "2026-10-31T22:50:37Z".into(), "house".into()));
        assert!(check("return { os.exit() }").unwrap_err().contains("can't be read"));
        assert!(check(r#"{"format":"other","version":1,"props":[]}"#).unwrap_err().contains("isn't a Den file"));
        assert!(check(r#"{"format":"leyline-den","version":2,"props":[]}"#).unwrap_err().contains("newer"));
        assert!(check(r#"{"format":"leyline-den","props":[]}"#).unwrap_err().contains("no version"));
        assert!(check(r#"{"format":"leyline-den","version":1,"props":[{"entry":"1"}]}"#).unwrap_err().contains("isn't a prop"));
        let many = format!(r#"{{"format":"leyline-den","version":1,"props":[{}]}}"#, vec![r#"{"entry":1}"#; 801].join(","));
        assert!(check(&many).unwrap_err().contains("800"));
        let big = format!(r#"{{"format":"leyline-den","version":1,"props":[],"pad":"{}"}}"#, "x".repeat(600 * 1024));
        assert!(check(&big).unwrap_err().contains("too big"));
    }

    #[test]
    fn list_add_text_open() {
        let k = tempfile::tempdir().unwrap();
        assert!(list(k.path()).is_empty(), "no folder yet: nothing");
        assert_eq!(add(k.path(), "House.den.json", GOOD).unwrap(), "house");
        assert_eq!(add(k.path(), "house.den.json", GOOD).unwrap(), "house-2", "never over another file");
        assert_eq!(add(k.path(), "", GOOD).unwrap(), "house-3", "the name inside when the file's has none");
        assert!(add(k.path(), "x.den.json", "{}").is_err());
        fs::write(dir(k.path()).join("broken.den.json"), "nope").unwrap();
        fs::write(dir(k.path()).join("notes.txt"), "hi").unwrap();
        let l = list(k.path());
        assert_eq!(l.len(), 4, "{l:?}");
        assert!(l.iter().any(|f| f.name == "broken" && f.props == 0));
        let h = l.iter().find(|f| f.name == "house").unwrap();
        assert_eq!((h.props, h.saved.as_str()), (2, "2026-10-31T22:50:37Z"));
        assert_eq!(text(k.path(), "house").unwrap(), GOOD);
        assert!(text(k.path(), "../../etc/passwd").is_err());
        assert!(text(k.path(), "broken").is_err());
        assert_eq!(open(k.path(), "house").unwrap(), "house");
        let o: Value = serde_json::from_str(&fs::read_to_string(dir(k.path()).join("open.json")).unwrap()).unwrap();
        assert_eq!(o["file"], "house");
        let id1 = o["id"].as_str().unwrap().to_string();
        assert!(!id1.is_empty() && id1.chars().all(|c| c.is_ascii_alphanumeric()));
        open(k.path(), "house").unwrap();
        let o2: Value = serde_json::from_str(&fs::read_to_string(dir(k.path()).join("open.json")).unwrap()).unwrap();
        assert_ne!(o2["id"].as_str().unwrap(), id1, "a new id each time, so the realm opens it again");
        assert!(open(k.path(), "nofile").is_err());
        assert!(open(k.path(), "broken").is_err());
    }
}
